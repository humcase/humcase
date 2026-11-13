//! Bench of nature (and line of work) suggestions from embeddings: the nature of the nearest
//! labeled cases, the nearest class centre, a small linear classifier, and label descriptions
//! (no labeled case at all), against each other and across sets.
//!
//! Usage: nature_bench --dir <model dir> --set a.json[,b.json...] --descriptions d.json
//!        [--cache vectors.json] [--head <words>]
//!
//! A set holds labeled cases (`id`, `lang`, `nature`, `text`, optional `line_of_work`), such as
//! `src-tauri/src/recognize_cases.json` or a local export of a Humcase base (never committed).
//! Model: Granite Embedding multilingual R2 (CLS pooling, no prefix), 512 tokens.

use std::collections::{BTreeMap, HashMap};

use ort::session::{Session, SessionInputValue};
use ort::value::Tensor;
use serde::{Deserialize, Serialize};
use tokenizers::{Tokenizer, TruncationParams};

const NATURES: [&str; 7] = [
    "todo", "idea", "question", "decision", "bug", "research", "note",
];
const LINES: [&str; 5] = ["dev", "marketing", "research", "reflection", "personal"];

#[derive(Deserialize, Clone)]
struct Case {
    id: String,
    #[serde(default)]
    lang: String,
    nature: String,
    #[serde(default)]
    line_of_work: String,
    text: String,
}

/// Descriptions of each label, in the six languages: `{"nature": {"todo": ["...", ...]}, "line": {...}}`.
#[derive(Deserialize)]
struct Descriptions {
    nature: BTreeMap<String, Vec<String>>,
    line: BTreeMap<String, Vec<String>>,
}

struct Model {
    session: Session,
    tokenizer: Tokenizer,
    inputs: Vec<String>,
}

impl Model {
    fn load(dir: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut tokenizer =
            Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| e.to_string())?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: 512,
                ..Default::default()
            }))
            .map_err(|e| e.to_string())?
            .with_padding(None);
        let session = Session::builder()?
            .with_intra_threads(4)?
            .commit_from_file(dir.join("model.onnx"))?;
        let inputs = session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect();
        Ok(Model {
            session,
            tokenizer,
            inputs,
        })
    }

    fn embed(&mut self, text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let enc = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| e.to_string())?;
        let n = enc.get_ids().len();
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
        // CLS pooling.
        let mut v = data[..dim].to_vec();
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
        v.iter_mut().for_each(|x| *x /= norm);
        Ok(v)
    }
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Scores per label (higher is better); the guess is offered when the best leads the second by
/// at least `margin`.
type Scores = Vec<(String, f32)>;

fn decide(scores: &Scores, margin: f32) -> Option<&str> {
    let mut s: Vec<&(String, f32)> = scores.iter().collect();
    s.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let second = s.get(1).map_or(f32::MIN, |x| x.1);
    (s[0].1 - second >= margin).then_some(s[0].0.as_str())
}

/// Weighted vote of the k nearest labeled cases (`exclude` left out).
fn knn(v: &[f32], train: &[(&[f32], &str)], k: usize) -> Scores {
    let mut near: Vec<(f32, &str)> = train.iter().map(|(t, l)| (dot(v, t), *l)).collect();
    near.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let mut votes: HashMap<&str, f32> = HashMap::new();
    let total: f32 = near
        .iter()
        .take(k)
        .map(|(s, _)| s.max(0.0))
        .sum::<f32>()
        .max(1e-6);
    for (s, l) in near.iter().take(k) {
        *votes.entry(l).or_default() += s.max(0.0) / total;
    }
    votes.into_iter().map(|(l, s)| (l.to_string(), s)).collect()
}

/// Cosine to the centre of each label.
fn centroids(v: &[f32], train: &[(&[f32], &str)]) -> Scores {
    let mut sums: BTreeMap<&str, (Vec<f32>, usize)> = BTreeMap::new();
    for (t, l) in train {
        let e = sums.entry(l).or_insert_with(|| (vec![0.0; t.len()], 0));
        e.0.iter_mut().zip(t.iter()).for_each(|(a, x)| *a += x);
        e.1 += 1;
    }
    sums.into_iter()
        .map(|(l, (c, _))| {
            let n = c.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
            (l.to_string(), dot(v, &c) / n)
        })
        .collect()
}

/// Multinomial logistic regression with class weights (balanced), trained by gradient descent.
struct Linear {
    labels: Vec<String>,
    w: Vec<Vec<f32>>,
    b: Vec<f32>,
}

impl Linear {
    fn train(train: &[(&[f32], &str)]) -> Self {
        let mut labels: Vec<String> = train.iter().map(|(_, l)| l.to_string()).collect();
        labels.sort();
        labels.dedup();
        let dim = train[0].0.len();
        let k = labels.len();
        let idx: Vec<usize> = train
            .iter()
            .map(|(_, l)| labels.iter().position(|x| x == l).unwrap())
            .collect();
        let mut count = vec![0f32; k];
        idx.iter().for_each(|&i| count[i] += 1.0);
        let weight: Vec<f32> = count
            .iter()
            .map(|c| train.len() as f32 / (k as f32 * c))
            .collect();
        let (mut w, mut b) = (vec![vec![0f32; dim]; k], vec![0f32; k]);
        let (rate, lambda) = (0.5f32, 1e-3f32);
        for _ in 0..300 {
            let mut gw = vec![vec![0f32; dim]; k];
            let mut gb = vec![0f32; k];
            for ((x, _), &y) in train.iter().zip(&idx) {
                let z: Vec<f32> = (0..k).map(|c| dot(&w[c], x) * 8.0 + b[c]).collect();
                let m = z.iter().cloned().fold(f32::MIN, f32::max);
                let e: Vec<f32> = z.iter().map(|v| (v - m).exp()).collect();
                let s: f32 = e.iter().sum();
                for c in 0..k {
                    let g = (e[c] / s - if c == y { 1.0 } else { 0.0 }) * weight[y];
                    gb[c] += g;
                    gw[c]
                        .iter_mut()
                        .zip(x.iter())
                        .for_each(|(a, xi)| *a += g * xi * 8.0);
                }
            }
            let n = train.len() as f32;
            for c in 0..k {
                b[c] -= rate * gb[c] / n;
                for (wi, gi) in w[c].iter_mut().zip(&gw[c]) {
                    *wi -= rate * (gi / n + lambda * *wi);
                }
            }
        }
        Linear { labels, w, b }
    }

    fn scores(&self, v: &[f32]) -> Scores {
        let z: Vec<f32> = self
            .w
            .iter()
            .zip(&self.b)
            .map(|(w, b)| dot(w, v) * 8.0 + b)
            .collect();
        let m = z.iter().cloned().fold(f32::MIN, f32::max);
        let e: Vec<f32> = z.iter().map(|x| (x - m).exp()).collect();
        let s: f32 = e.iter().sum();
        self.labels
            .iter()
            .cloned()
            .zip(e.iter().map(|x| x / s))
            .collect()
    }
}

#[derive(Default)]
struct Tally {
    total: usize,
    offered: usize,
    right: usize,
}

impl Tally {
    fn add(&mut self, guess: Option<&str>, label: &str) {
        self.total += 1;
        if let Some(g) = guess {
            self.offered += 1;
            if g == label {
                self.right += 1;
            }
        }
    }
    fn line(&self) -> String {
        format!(
            "right {:>3}/{:<3} ({:>3.0} %)  offered {:>3.0} %  right when offered {:>3.0} %",
            self.right,
            self.total,
            100.0 * self.right as f64 / self.total.max(1) as f64,
            100.0 * self.offered as f64 / self.total.max(1) as f64,
            100.0 * self.right as f64 / self.offered.max(1) as f64
        )
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Cache {
    vectors: HashMap<String, Vec<f32>>,
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(arg(&args, "--dir").ok_or("--dir")?);
    let head: Option<usize> = arg(&args, "--head").map(|s| s.parse()).transpose()?;
    let descriptions: Descriptions = serde_json::from_str(&std::fs::read_to_string(
        arg(&args, "--descriptions").ok_or("--descriptions")?,
    )?)?;
    let cache_path = arg(&args, "--cache");
    let mut cache: Cache = cache_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let mut model = Model::load(&dir)?;
    let mut embed =
        |model: &mut Model, text: &str| -> Result<Vec<f32>, Box<dyn std::error::Error>> {
            let text = match head {
                Some(n) => text
                    .split_whitespace()
                    .take(n)
                    .collect::<Vec<_>>()
                    .join(" "),
                None => text.to_string(),
            };
            let key = format!("{}:{}", head.unwrap_or(0), text);
            if let Some(v) = cache.vectors.get(&key) {
                return Ok(v.clone());
            }
            let v = model.embed(&text)?;
            cache.vectors.insert(key, v.clone());
            Ok(v)
        };

    // Label descriptions, as vectors.
    let mut desc_vectors: BTreeMap<(&str, String), Vec<Vec<f32>>> = BTreeMap::new();
    for (kind, map) in [
        ("nature", &descriptions.nature),
        ("line", &descriptions.line),
    ] {
        for (label, texts) in map {
            let mut vs = Vec::new();
            for t in texts {
                vs.push(model.embed(t)?);
            }
            desc_vectors.insert((kind, label.clone()), vs);
        }
    }
    let describe = |kind: &str, v: &[f32]| -> Scores {
        desc_vectors
            .iter()
            .filter(|((k, _), _)| *k == kind)
            .map(|((_, l), vs)| {
                // Mean of the best three descriptions.
                let mut s: Vec<f32> = vs.iter().map(|d| dot(v, d)).collect();
                s.sort_by(|a, b| b.partial_cmp(a).unwrap());
                (
                    l.clone(),
                    s.iter().take(3).sum::<f32>() / s.len().min(3) as f32,
                )
            })
            .collect()
    };

    let mut sets: Vec<(String, Vec<Case>, Vec<Vec<f32>>)> = Vec::new();
    for path in arg(&args, "--set").ok_or("--set")?.split(',') {
        let cases: Vec<Case> = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        let t = std::time::Instant::now();
        let mut vs = Vec::new();
        for c in &cases {
            vs.push(embed(&mut model, &c.text)?);
        }
        eprintln!(
            "{path}: {} cases embedded in {:.1} s",
            cases.len(),
            t.elapsed().as_secs_f64()
        );
        let name = std::path::Path::new(path)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        sets.push((name, cases, vs));
    }
    if let Some(p) = &cache_path {
        std::fs::write(p, serde_json::to_string(&cache)?)?;
    }

    // The dictionary's guesses (`recognition_bench` with RECOGNIZE_DUMP), one file per set, same order.
    let dicts: Vec<HashMap<String, serde_json::Value>> = match arg(&args, "--dict") {
        Some(list) => list
            .split(',')
            .map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap())
            .collect(),
        None => Vec::new(),
    };
    // --doubtful <share>: cases of the first set whose ten nearest cases mostly hold another nature.
    if let Some(share) = arg(&args, "--doubtful") {
        let share: f32 = share.parse()?;
        let (_, cases, vs) = &sets[0];
        let mut found = Vec::new();
        for i in 0..cases.len() {
            let train: Vec<(&[f32], &str)> = (0..cases.len())
                .filter(|&o| o != i)
                .map(|o| (vs[o].as_slice(), cases[o].nature.as_str()))
                .collect();
            let votes = knn(&vs[i], &train, 10);
            let best = votes
                .iter()
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .unwrap();
            if best.0 != cases[i].nature && best.1 >= share {
                found.push(serde_json::json!({
                    "id": cases[i].id, "nature": cases[i].nature, "proposed": best.0,
                    "share": best.1, "title": cases[i].text.lines().next().unwrap_or(""),
                }));
            }
        }
        println!("{}", serde_json::to_string_pretty(&found)?);
        return Ok(());
    }
    let margins = [0.0f32, 0.1, 0.2];
    for kind in ["nature", "line"] {
        let label_of = |c: &Case| -> String {
            if kind == "nature" {
                c.nature.clone()
            } else {
                c.line_of_work.clone()
            }
        };
        let known: &[&str] = if kind == "nature" { &NATURES } else { &LINES };
        println!("\n######## {kind}");
        for (set_index, (name, cases, vs)) in sets.iter().enumerate() {
            let rows: Vec<usize> = (0..cases.len())
                .filter(|&i| known.contains(&label_of(&cases[i]).as_str()))
                .collect();
            if rows.is_empty() {
                continue;
            }
            println!(
                "\n== {name} ({} cases), each case left out in turn",
                rows.len()
            );
            let mut out: BTreeMap<String, Tally> = BTreeMap::new();
            // Five folds for the linear classifier, leave-one-out for the others.
            let folds = 5;
            let mut linear: Vec<Linear> = Vec::new();
            for f in 0..folds {
                let train: Vec<(&[f32], &str)> = rows
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| j % folds != f)
                    .map(|(_, &i)| (vs[i].as_slice(), label_str(cases, i, kind)))
                    .collect();
                linear.push(Linear::train(&train));
            }
            for (j, &i) in rows.iter().enumerate() {
                let label = label_of(&cases[i]);
                let train: Vec<(&[f32], &str)> = rows
                    .iter()
                    .filter(|&&o| o != i)
                    .map(|&o| (vs[o].as_slice(), label_str(cases, o, kind)))
                    .collect();
                let methods: Vec<(String, Scores)> = vec![
                    ("knn1".into(), knn(&vs[i], &train, 1)),
                    ("knn5".into(), knn(&vs[i], &train, 5)),
                    ("knn10".into(), knn(&vs[i], &train, 10)),
                    ("centroid".into(), centroids(&vs[i], &train)),
                    ("linear".into(), linear[j % folds].scores(&vs[i])),
                    ("descriptions".into(), describe(kind, &vs[i])),
                ];
                // The dictionary first; the nearest cases when it says nothing.
                if let Some(dict) = dicts.get(set_index) {
                    let key = if kind == "nature" { "nature" } else { "line" };
                    let d = dict
                        .get(&cases[i].id)
                        .and_then(|g| g.get(key))
                        .and_then(|v| v.as_str());
                    out.entry("dictionary alone".into())
                        .or_default()
                        .add(d, &label);
                    for &margin in &margins {
                        let k = d.or_else(|| decide(&methods[1].1, margin * 2.0));
                        out.entry(format!("dict then knn5 margin {margin:.1}"))
                            .or_default()
                            .add(k, &label);
                        let k = decide(&methods[1].1, margin * 2.0).or(d);
                        out.entry(format!("knn5 then dict margin {margin:.1}"))
                            .or_default()
                            .add(k, &label);
                    }
                }
                for (m, scores) in &methods {
                    for &margin in &margins {
                        // Margins are on different scales: knn/linear in shares, the others in cosine.
                        let mg = if m.starts_with("knn") || m == "linear" {
                            margin * 2.0
                        } else {
                            margin / 4.0
                        };
                        out.entry(format!("{m:<12} margin {margin:.1}"))
                            .or_default()
                            .add(decide(scores, mg), &label);
                        out.entry(format!("{m:<12} margin {margin:.1} {}", cases[i].lang))
                            .or_default()
                            .add(decide(scores, mg), &label);
                    }
                }
            }
            for (k, t) in &out {
                if !k.ends_with(|c: char| c.is_ascii_alphabetic())
                    || std::env::var("BY_LANG").is_ok()
                {
                    println!("{k:<34} {}", t.line());
                }
            }
        }
        // Across sets: learn on one, suggest on the other.
        for (a, ca, va) in &sets {
            for (b, cb, vb) in &sets {
                if a == b {
                    continue;
                }
                let train: Vec<(&[f32], &str)> = (0..ca.len())
                    .filter(|&i| known.contains(&label_of(&ca[i]).as_str()))
                    .map(|i| (va[i].as_slice(), label_str(ca, i, kind)))
                    .collect();
                let test: Vec<usize> = (0..cb.len())
                    .filter(|&i| known.contains(&label_of(&cb[i]).as_str()))
                    .collect();
                if train.is_empty() || test.is_empty() {
                    continue;
                }
                println!("\n== learn on {a}, suggest on {b} ({} cases)", test.len());
                let lin = Linear::train(&train);
                let mut out: BTreeMap<String, Tally> = BTreeMap::new();
                for &i in &test {
                    let label = label_of(&cb[i]);
                    for (m, scores) in [
                        ("knn10", knn(&vb[i], &train, 10)),
                        ("centroid", centroids(&vb[i], &train)),
                        ("linear", lin.scores(&vb[i])),
                    ] {
                        for &margin in &margins {
                            let mg = if m == "centroid" {
                                margin / 4.0
                            } else {
                                margin * 2.0
                            };
                            out.entry(format!("{m:<12} margin {margin:.1}"))
                                .or_default()
                                .add(decide(&scores, mg), &label);
                        }
                    }
                }
                for (k, t) in &out {
                    println!("{k:<34} {}", t.line());
                }
            }
        }
    }
    Ok(())
}

fn label_str<'a>(cases: &'a [Case], i: usize, kind: &str) -> &'a str {
    if kind == "nature" {
        &cases[i].nature
    } else {
        &cases[i].line_of_work
    }
}
