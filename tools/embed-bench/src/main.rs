//! Bench of a local embedding model for Humcase's search by meaning.
//!
//! Measures what the founder asked for: time until the model is ready, memory while it works and
//! after it is released, time per case and per query on one thread at low priority, and how often
//! the expected case comes back in the first five results, per language and per kind of query
//! (`meaning`: said without the case's words; `words`: typed as in a search box). It compares the
//! model with Humcase's current search (the same SQLite FTS5 query: every word required, title
//! weighted 5), with a word search where any word counts (BM25 without the words found in more
//! than a tenth of the cases), and with the fusion of the model and each of them.
//!
//! Usage: embed-bench --model granite|e5 --dir <folder with model.onnx and tokenizer.json>
//!        --corpus corpus.json --queries queries.json [--langs en,fr] [--max-tokens N]
//!        [--chunk-words N] [--threads N] [--title] [--no-memory-pattern] [--opt 0-3]
//!        [--out result.json]

use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

use ort::session::builder::GraphOptimizationLevel;
use ort::session::{Session, SessionInputValue};
use ort::value::Tensor;
use serde::{Deserialize, Serialize};
use tokenizers::{Tokenizer, TruncationParams};

const LANGS: [&str; 6] = ["en", "fr", "es", "de", "it", "pt"];

#[derive(Deserialize)]
struct Doc {
    id: String,
    title: String,
    text: String,
}

#[derive(Deserialize)]
struct Query {
    target: String,
    #[serde(alias = "q")]
    meaning: HashMap<String, String>,
    #[serde(default)]
    words: HashMap<String, String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Pooling {
    Cls,
    Mean,
}

struct Model {
    session: Session,
    tokenizer: Tokenizer,
    inputs: Vec<String>,
    pooling: Pooling,
    query_prefix: &'static str,
    passage_prefix: &'static str,
    max_tokens: usize,
    truncated: usize,
}

impl Model {
    fn embed(&mut self, text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let enc = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| e.to_string())?;
        let n = enc.get_ids().len();
        if n >= self.max_tokens {
            self.truncated += 1;
        }
        let ids: Vec<i64> = enc.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = enc.get_attention_mask().iter().map(|&x| x as i64).collect();
        let types: Vec<i64> = enc.get_type_ids().iter().map(|&x| x as i64).collect();
        let mut feed: Vec<(String, SessionInputValue)> = Vec::new();
        for name in &self.inputs {
            let data = match name.as_str() {
                "input_ids" => ids.clone(),
                "attention_mask" => mask.clone(),
                "token_type_ids" => types.clone(),
                other => return Err(format!("unexpected model input {other}").into()),
            };
            feed.push((
                name.clone(),
                Tensor::from_array(([1usize, n], data))?.into(),
            ));
        }
        let outputs = self.session.run(feed)?;
        let (shape, data) = outputs[0].try_extract_tensor::<f32>()?;
        let dim = *shape.last().unwrap() as usize;
        let mut v = match (shape.len(), self.pooling) {
            (2, _) => data[..dim].to_vec(),
            (_, Pooling::Cls) => data[..dim].to_vec(),
            (_, Pooling::Mean) => {
                let mut acc = vec![0f32; dim];
                for t in 0..n {
                    for (a, x) in acc.iter_mut().zip(&data[t * dim..(t + 1) * dim]) {
                        *a += x;
                    }
                }
                acc.iter_mut().for_each(|a| *a /= n as f32);
                acc
            }
        };
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
        v.iter_mut().for_each(|x| *x /= norm);
        Ok(v)
    }
}

/// Working set and peak working set of this process, in MB.
#[cfg(windows)]
fn memory_mb() -> (f64, f64) {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut c: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb);
        let mb = |b: usize| b as f64 / 1_048_576.0;
        (mb(c.WorkingSetSize), mb(c.PeakWorkingSetSize))
    }
}

#[cfg(not(windows))]
fn memory_mb() -> (f64, f64) {
    (0.0, 0.0)
}

#[cfg(windows)]
fn lower_priority() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS,
    };
    unsafe {
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
}

#[cfg(not(windows))]
fn lower_priority() {}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

/// Splits a long text into pieces of about `size` words; 0 keeps it whole.
fn chunks(text: &str, size: usize) -> Vec<String> {
    let ws: Vec<&str> = text.split_whitespace().collect();
    if size == 0 || ws.len() <= size {
        return vec![text.to_string()];
    }
    ws.chunks(size).map(|c| c.join(" ")).collect()
}

struct Bm25 {
    docs: Vec<HashMap<String, f32>>,
    lens: Vec<f32>,
    avg: f32,
    df: HashMap<String, f32>,
}

impl Bm25 {
    fn new(texts: &[&str]) -> Self {
        let mut docs = Vec::new();
        let mut lens = Vec::new();
        let mut df: HashMap<String, f32> = HashMap::new();
        for t in texts {
            let ws = words(t);
            lens.push(ws.len() as f32);
            let mut tf: HashMap<String, f32> = HashMap::new();
            for w in ws {
                *tf.entry(w).or_default() += 1.0;
            }
            for w in tf.keys() {
                *df.entry(w.clone()).or_default() += 1.0;
            }
            docs.push(tf);
        }
        let avg = lens.iter().sum::<f32>() / lens.len().max(1) as f32;
        Bm25 {
            docs,
            lens,
            avg,
            df,
        }
    }

    /// Scores of every case; words found in more than a tenth of the cases are ignored, so that
    /// "the", "de" or "la" do not bring unrelated cases up.
    fn scores(&self, query: &str) -> Vec<f32> {
        let n = self.docs.len() as f32;
        let (k1, b) = (1.2f32, 0.75f32);
        let q: Vec<String> = words(query)
            .into_iter()
            .filter(|w| self.df.get(w).is_some_and(|&df| df <= n / 10.0))
            .collect();
        self.docs
            .iter()
            .zip(&self.lens)
            .map(|(tf, len)| {
                q.iter()
                    .map(|w| {
                        let f = *tf.get(w).unwrap_or(&0.0);
                        if f == 0.0 {
                            return 0.0;
                        }
                        let df = self.df[w];
                        let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                        idf * f * (k1 + 1.0) / (f + k1 * (1.0 - b + b * len / self.avg))
                    })
                    .sum()
            })
            .collect()
    }
}

/// Humcase's current search: an FTS5 table over title and content, the query split as
/// `split_search` does (every word quoted, all required), ranked by `bm25(5.0, 1.0)`.
struct CurrentSearch {
    conn: rusqlite::Connection,
}

impl CurrentSearch {
    fn new(corpus: &[Doc]) -> rusqlite::Result<Self> {
        let conn = rusqlite::Connection::open_in_memory()?;
        conn.execute_batch("CREATE VIRTUAL TABLE fts USING fts5(title, content);")?;
        {
            let mut insert =
                conn.prepare("INSERT INTO fts(rowid, title, content) VALUES (?1, ?2, ?3)")?;
            for (i, d) in corpus.iter().enumerate() {
                let content = d.text.strip_prefix(d.title.as_str()).unwrap_or(&d.text);
                insert.execute(rusqlite::params![i as i64, d.title, content])?;
            }
        }
        Ok(CurrentSearch { conn })
    }

    /// Scores of every case: higher is better; a case the search does not return gets the lowest.
    fn scores(&self, query: &str, docs: usize) -> Vec<f32> {
        let mut scores = vec![f32::MIN; docs];
        let fts: Vec<String> = query
            .split_whitespace()
            .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
            .collect();
        if fts.is_empty() {
            return scores;
        }
        let Ok(mut stmt) = self
            .conn
            .prepare("SELECT rowid, bm25(fts, 5.0, 1.0) FROM fts WHERE fts MATCH ?1")
        else {
            return scores;
        };
        let rows = stmt.query_map([fts.join(" ")], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
        });
        if let Ok(rows) = rows {
            for (id, rank) in rows.flatten() {
                scores[id as usize] = -rank as f32;
            }
        }
        scores
    }
}

/// 1-based rank of the target among the scores; ties count against it, so a search that finds
/// nothing does not rank the target first.
fn rank_of(scores: &[f32], target: usize) -> usize {
    let s = scores[target];
    scores.iter().filter(|&&x| x >= s).count()
}

/// Short labels of the three best cases, to read what came back instead of the target.
fn top3(scores: &[f32], corpus: &[Doc]) -> String {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| scores[b].partial_cmp(&scores[a]).unwrap());
    order[..3]
        .iter()
        .map(|&i| {
            format!(
                "{} {}",
                &corpus[i].id[..8],
                corpus[i].title.chars().take(50).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Reciprocal rank fusion of the model and the word search: what an app would show when it
/// combines both. A case the word search does not match gets nothing from it.
fn fuse(model: &[f32], words: &[f32]) -> Vec<f32> {
    let position = |scores: &[f32]| {
        let mut order: Vec<usize> = (0..scores.len()).collect();
        order.sort_by(|&a, &b| scores[b].partial_cmp(&scores[a]).unwrap());
        let mut pos = vec![0usize; scores.len()];
        for (r, &i) in order.iter().enumerate() {
            pos[i] = r + 1;
        }
        pos
    };
    let (pm, pw) = (position(model), position(words));
    (0..model.len())
        .map(|i| {
            let w = if words[i] > 0.0 && words[i] != f32::MIN {
                1.0 / (60.0 + pw[i] as f32)
            } else {
                0.0
            };
            1.0 / (60.0 + pm[i] as f32) + w
        })
        .collect()
}

#[derive(Default, Serialize, Clone)]
struct LangScore {
    queries: usize,
    top1: usize,
    top5: usize,
    mrr: f64,
}

impl LangScore {
    fn add(&mut self, rank: usize) {
        self.queries += 1;
        if rank == 1 {
            self.top1 += 1;
        }
        if rank <= 5 {
            self.top5 += 1;
        }
        self.mrr += 1.0 / rank as f64;
    }
}

type Scores = BTreeMap<String, BTreeMap<String, BTreeMap<String, LangScore>>>;

#[derive(Serialize)]
struct Report {
    model: String,
    threads: usize,
    max_tokens: usize,
    chunk_words: usize,
    title_vector: bool,
    memory_pattern: bool,
    opt: u8,
    corpus_lang: Option<String>,
    docs: usize,
    pieces: usize,
    truncated_pieces: usize,
    load_ms: f64,
    first_embed_ms: f64,
    ready_ms: f64,
    mem_before_mb: f64,
    mem_after_load_mb: f64,
    mem_peak_mb: f64,
    mem_after_release_mb: f64,
    index_total_s: f64,
    ms_per_case: f64,
    query_ms_median: f64,
    query_ms_max: f64,
    search_ms_per_query: f64,
    scores: Scores,
    misses: Vec<String>,
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let kind = arg(&args, "--model").ok_or("--model granite|e5")?;
    let dir = std::path::PathBuf::from(arg(&args, "--dir").ok_or("--dir")?);
    let corpus: Vec<Doc> = serde_json::from_str(&std::fs::read_to_string(
        arg(&args, "--corpus").ok_or("--corpus")?,
    )?)?;
    let queries: Vec<Query> = serde_json::from_str(&std::fs::read_to_string(
        arg(&args, "--queries").ok_or("--queries")?,
    )?)?;
    let threads: usize = arg(&args, "--threads").map_or(Ok(1), |s| s.parse())?;
    let (pooling, qp, pp, default_max) = match kind.as_str() {
        "granite" => (Pooling::Cls, "", "", 2048),
        "e5" => (Pooling::Mean, "query: ", "passage: ", 512),
        other => return Err(format!("unknown model {other}").into()),
    };
    let max_tokens: usize = arg(&args, "--max-tokens").map_or(Ok(default_max), |s| s.parse())?;
    let chunk_words: usize = arg(&args, "--chunk-words").map_or(Ok(0), |s| s.parse())?;
    let langs: Vec<String> = arg(&args, "--langs").map_or_else(
        || LANGS.iter().map(|l| l.to_string()).collect(),
        |s| s.split(',').map(str::to_string).collect(),
    );
    let corpus_lang = arg(&args, "--corpus-lang");
    let opt: u8 = arg(&args, "--opt").map_or(Ok(3), |s| s.parse())?;
    let title_vector = args.iter().any(|a| a == "--title");
    let memory_pattern = !args.iter().any(|a| a == "--no-memory-pattern");

    lower_priority();
    let (mem_before, _) = memory_mb();

    // Time until the first vector: what the person waits for on the first search.
    let t = Instant::now();
    let mut tokenizer =
        Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| e.to_string())?;
    tokenizer
        .with_truncation(Some(TruncationParams {
            max_length: max_tokens,
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?
        .with_padding(None);
    let session = Session::builder()?
        .with_optimization_level(match opt {
            0 => GraphOptimizationLevel::Disable,
            1 => GraphOptimizationLevel::Level1,
            2 => GraphOptimizationLevel::Level2,
            _ => GraphOptimizationLevel::Level3,
        })?
        .with_intra_threads(threads)?
        .with_memory_pattern(memory_pattern)?
        .commit_from_file(dir.join("model.onnx"))?;
    let inputs: Vec<String> = session
        .inputs()
        .iter()
        .map(|i| i.name().to_string())
        .collect();
    let outputs: Vec<String> = session
        .outputs()
        .iter()
        .map(|o| o.name().to_string())
        .collect();
    eprintln!("inputs {inputs:?} outputs {outputs:?}");
    let load_ms = t.elapsed().as_secs_f64() * 1e3;
    let mut model = Model {
        session,
        tokenizer,
        inputs,
        pooling,
        query_prefix: qp,
        passage_prefix: pp,
        max_tokens,
        truncated: 0,
    };
    let t1 = Instant::now();
    model.embed(&format!("{qp}hello"))?;
    let first_embed_ms = t1.elapsed().as_secs_f64() * 1e3;
    let ready_ms = t.elapsed().as_secs_f64() * 1e3;
    let (mem_after_load, _) = memory_mb();

    // Index the corpus: one vector per piece, a case scores as its best piece.
    let t = Instant::now();
    let mut piece_doc: Vec<usize> = Vec::new();
    let mut vectors: Vec<Vec<f32>> = Vec::new();
    for (i, d) in corpus.iter().enumerate() {
        let mut pieces = chunks(&d.text, chunk_words);
        if title_vector && !d.title.is_empty() && d.title != d.text {
            pieces.push(d.title.clone());
        }
        for c in pieces {
            vectors.push(model.embed(&format!("{}{c}", model.passage_prefix))?);
            piece_doc.push(i);
        }
    }
    let index_total_s = t.elapsed().as_secs_f64();
    let truncated_pieces = model.truncated;

    let index: HashMap<&str, usize> = corpus
        .iter()
        .enumerate()
        .map(|(i, d)| (&d.id[..8], i))
        .collect();
    let bm25 = Bm25::new(&corpus.iter().map(|d| d.text.as_str()).collect::<Vec<_>>());
    let current = CurrentSearch::new(&corpus)?;
    // method -> kind of query -> language -> score
    let mut scores_by: Scores = BTreeMap::new();
    let mut add = |method: &str, kind: &str, lang: &str, rank: usize| {
        scores_by
            .entry(method.into())
            .or_default()
            .entry(kind.into())
            .or_default()
            .entry(lang.into())
            .or_default()
            .add(rank)
    };
    let mut query_ms = Vec::new();
    let mut search_ms = 0.0;
    let mut misses = Vec::new();
    for q in &queries {
        let target = *index
            .get(q.target.as_str())
            .ok_or(format!("target {} not in corpus", q.target))?;
        for (kind, set) in [("meaning", &q.meaning), ("words", &q.words)] {
            for lang in &langs {
                let Some(text) = set.get(lang) else { continue };
                let t = Instant::now();
                let v = model.embed(&format!("{}{text}", model.query_prefix))?;
                query_ms.push(t.elapsed().as_secs_f64() * 1e3);
                let t = Instant::now();
                let mut scores = vec![f32::MIN; corpus.len()];
                for (p, pv) in vectors.iter().enumerate() {
                    let s: f32 = pv.iter().zip(&v).map(|(a, b)| a * b).sum();
                    let d = piece_doc[p];
                    if s > scores[d] {
                        scores[d] = s;
                    }
                }
                let r = rank_of(&scores, target);
                search_ms += t.elapsed().as_secs_f64() * 1e3;
                add("model", kind, lang, r);
                if r > 5 {
                    misses.push(format!(
                        "{} {kind} {lang} rank {r}: {text} -> {}",
                        q.target,
                        top3(&scores, &corpus)
                    ));
                }
                let now = current.scores(text, corpus.len());
                add("current", kind, lang, rank_of(&now, target));
                add(
                    "model+current",
                    kind,
                    lang,
                    rank_of(&fuse(&scores, &now), target),
                );
                let any = bm25.scores(text);
                add("any_word", kind, lang, rank_of(&any, target));
                add(
                    "model+any_word",
                    kind,
                    lang,
                    rank_of(&fuse(&scores, &any), target),
                );
            }
        }
    }
    let (_, mem_peak) = memory_mb();
    drop(model);
    drop(vectors);
    let (mem_after_release, _) = memory_mb();

    query_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let report = Report {
        model: kind,
        threads,
        max_tokens,
        chunk_words,
        title_vector,
        memory_pattern,
        opt,
        corpus_lang,
        docs: corpus.len(),
        pieces: piece_doc.len(),
        truncated_pieces,
        load_ms,
        first_embed_ms,
        ready_ms,
        mem_before_mb: mem_before,
        mem_after_load_mb: mem_after_load,
        mem_peak_mb: mem_peak,
        mem_after_release_mb: mem_after_release,
        index_total_s,
        ms_per_case: index_total_s * 1e3 / corpus.len() as f64,
        query_ms_median: query_ms[query_ms.len() / 2],
        query_ms_max: *query_ms.last().unwrap(),
        search_ms_per_query: search_ms / query_ms.len() as f64,
        scores: scores_by,
        misses,
    };
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(out) = arg(&args, "--out") {
        std::fs::write(out, &json)?;
    }
    println!("{json}");
    Ok(())
}
