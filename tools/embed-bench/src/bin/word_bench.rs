//! Bench of word searches, without a model: which rule finds the expected case while keeping the
//! list of cases it returns short. Every rule runs on SQLite FTS5 like Humcase (default unicode61
//! tokenizer, title weighted 5).
//!
//! Usage: word-bench --queries queries.json --corpus corpus.json[,corpus_xx.json:xx...]
//!
//! A corpus given as `path:xx` runs only the queries in language `xx`; a corpus without a language
//! runs the queries of every language.

use std::collections::{BTreeMap, HashMap};

use rusqlite::Connection;
use serde::Deserialize;

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
    meaning: HashMap<String, String>,
    #[serde(default)]
    words: HashMap<String, String>,
}

/// The words of a query as FTS5 tokenizes them (letters and digits; anything else separates).
fn tokens(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// A crude, language-free stem: a word of 5 letters or more loses a final s, x or e (and "es"),
/// and is searched as a prefix, so "liens" finds "lien" and "perdus" finds "perdu".
fn stem(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() < 5 {
        return format!("\"{word}\"");
    }
    let mut end = chars.len();
    if chars[end - 1] == 's' || chars[end - 1] == 'x' {
        end -= 1;
    }
    if chars[end - 1] == 'e' {
        end -= 1;
    }
    let base: String = chars[..end].iter().collect();
    format!("\"{base}\"*")
}

struct Index {
    conn: Connection,
    docs: usize,
}

impl Index {
    fn new(corpus: &[Doc]) -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("CREATE VIRTUAL TABLE fts USING fts5(title, content);")?;
        {
            let mut insert =
                conn.prepare("INSERT INTO fts(rowid, title, content) VALUES (?1, ?2, ?3)")?;
            for (i, d) in corpus.iter().enumerate() {
                let content = d.text.strip_prefix(d.title.as_str()).unwrap_or(&d.text);
                insert.execute(rusqlite::params![i as i64, d.title, content])?;
            }
        }
        Ok(Index {
            conn,
            docs: corpus.len(),
        })
    }

    /// Cases matching an FTS5 query, best first, with their score (higher is better).
    fn run(&self, fts: &str) -> Vec<(usize, f64)> {
        if fts.is_empty() {
            return Vec::new();
        }
        let Ok(mut stmt) = self.conn.prepare(
            "SELECT rowid, bm25(fts, 5.0, 1.0) FROM fts WHERE fts MATCH ?1 ORDER BY bm25(fts, 5.0, 1.0)",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map([fts], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?)))
        else {
            return Vec::new();
        };
        rows.flatten().map(|(id, s)| (id as usize, -s)).collect()
    }

    /// Number of cases holding a term.
    fn df(&self, term: &str) -> usize {
        self.conn
            .query_row("SELECT count(*) FROM fts WHERE fts MATCH ?1", [term], |r| {
                r.get::<_, i64>(0)
            })
            .map(|n| n as usize)
            .unwrap_or(0)
    }

    /// The terms of a query found in at most a tenth of the cases (and at least one); all of
    /// them when none is.
    fn significant(&self, terms: Vec<String>) -> Vec<String> {
        let rare: Vec<String> = terms
            .iter()
            .filter(|t| {
                let df = self.df(t);
                df > 0 && df * 10 <= self.docs
            })
            .cloned()
            .collect();
        if rare.is_empty() {
            terms
        } else {
            rare
        }
    }
}

/// The cases a rule returns, in its order.
fn search(index: &Index, rule: &str, query: &str) -> Vec<usize> {
    let words = tokens(query);
    let exact: Vec<String> = words.iter().map(|w| format!("\"{w}\"")).collect();
    let stems: Vec<String> = words.iter().map(|w| stem(w)).collect();
    let keep_above = |hits: Vec<(usize, f64)>, ratio: f64| {
        let best = hits.first().map_or(0.0, |h| h.1);
        hits.into_iter()
            .filter(|h| h.1 >= best * ratio)
            .map(|h| h.0)
            .collect()
    };
    let ids = |hits: Vec<(usize, f64)>| hits.into_iter().map(|h| h.0).collect();
    match rule {
        // Humcase today: every word, as typed (each quoted word of `split_search` is a phrase).
        "all_exact" => {
            let today: Vec<String> = query
                .split_whitespace()
                .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
                .collect();
            ids(index.run(&today.join(" ")))
        }
        "all_stem" => ids(index.run(&stems.join(" AND "))),
        "any_exact" => ids(index.run(&index.significant(exact).join(" OR "))),
        "any_stem" => ids(index.run(&index.significant(stems).join(" OR "))),
        "any_stem_cut50" => keep_above(index.run(&index.significant(stems).join(" OR ")), 0.5),
        "any_stem_cut30" => keep_above(index.run(&index.significant(stems).join(" OR ")), 0.3),
        "any_stem_cut40" => keep_above(index.run(&index.significant(stems).join(" OR ")), 0.4),
        // Every case holding all the words, as today but with near forms, plus the cases holding
        // some of the rare words whose score reaches 40 % of the best.
        "all_or_cut40" => {
            let all: std::collections::HashSet<usize> = index
                .run(&stems.join(" AND "))
                .into_iter()
                .map(|h| h.0)
                .collect();
            let hits = index.run(&index.significant(stems).join(" OR "));
            let best = hits.first().map_or(0.0, |h| h.1);
            let mut kept: Vec<usize> = hits
                .into_iter()
                .filter(|h| all.contains(&h.0) || h.1 >= best * 0.4)
                .map(|h| h.0)
                .collect();
            for id in all {
                if !kept.contains(&id) {
                    kept.push(id);
                }
            }
            kept
        }
        _ => Vec::new(),
    }
}

const RULES: [&str; 8] = [
    "all_exact",
    "all_stem",
    "any_exact",
    "any_stem",
    "any_stem_cut50",
    "any_stem_cut30",
    "any_stem_cut40",
    "all_or_cut40",
];

#[derive(Default)]
struct Tally {
    queries: usize,
    top5: usize,
    found: usize,
    returned: Vec<usize>,
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let queries: Vec<Query> = serde_json::from_str(&std::fs::read_to_string(
        arg(&args, "--queries").ok_or("--queries")?,
    )?)?;
    // corpus label -> kind -> rule -> tally
    let mut out: BTreeMap<String, BTreeMap<&str, BTreeMap<&str, Tally>>> = BTreeMap::new();
    for spec in arg(&args, "--corpus").ok_or("--corpus")?.split(',') {
        let (path, lang) = match spec.rsplit_once(':') {
            Some((p, l)) if l.len() == 2 => (p, Some(l)),
            _ => (spec, None),
        };
        let corpus: Vec<Doc> = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        let index = Index::new(&corpus)?;
        let position: HashMap<&str, usize> = corpus
            .iter()
            .enumerate()
            .map(|(i, d)| (&d.id[..8], i))
            .collect();
        let label = format!("{} ({} cases)", lang.unwrap_or("all"), corpus.len());
        for q in &queries {
            let target = position[q.target.as_str()];
            for (kind, set) in [("meaning", &q.meaning), ("words", &q.words)] {
                for l in LANGS {
                    if lang.is_some_and(|x| x != l) {
                        continue;
                    }
                    let Some(text) = set.get(l) else { continue };
                    for rule in RULES {
                        let hits = search(&index, rule, text);
                        let t = out
                            .entry(label.clone())
                            .or_default()
                            .entry(kind)
                            .or_default()
                            .entry(rule)
                            .or_default();
                        t.queries += 1;
                        if let Some(p) = hits.iter().position(|&h| h == target) {
                            t.found += 1;
                            if p < 5 {
                                t.top5 += 1;
                            }
                        }
                        t.returned.push(hits.len());
                    }
                }
            }
        }
    }
    println!(
        "corpus | kind | rule | in first 5 | in the list | cases listed: median, 90th percentile"
    );
    for (label, kinds) in &out {
        for (kind, rules) in kinds {
            for (rule, t) in rules {
                let mut r = t.returned.clone();
                r.sort_unstable();
                println!(
                    "{label} | {kind} | {rule} | {}/{} | {}/{} | {}, {}",
                    t.top5,
                    t.queries,
                    t.found,
                    t.queries,
                    r[r.len() / 2],
                    r[r.len() * 9 / 10]
                );
            }
        }
    }
    Ok(())
}
