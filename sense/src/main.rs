//! Humcase's meaning worker: turns texts into vectors with a local embedding model, for search
//! by meaning and the suggestions built on it. Humcase starts it when it needs it and it stops
//! on its own after a minute without work, so it holds no memory at rest.
//!
//! It is a separate executable so that ONNX Runtime stays out of Humcase.exe: if the runtime or
//! the model is missing or refuses this machine, only meaning is lost, never the application.
//!
//! Folder (`--dir`, by default the worker's own folder):
//! - `onnxruntime.dll`: Microsoft's official ONNX Runtime (Windows 10 1809+ and Windows 11);
//! - `model/model.onnx` and `model/tokenizer.json`: Granite Embedding 97M multilingual R2.
//!
//! Protocol: one JSON object per line on stdin, one answer per line on stdout; nothing else is
//! written to stdout (diagnostics go to stderr).
//! - first line written: `{"ready":true,"model":"...","dim":384}` or `{"ready":false,"error":"..."}`;
//! - `{"id":1,"embed":["text", ...]}` answers `{"id":1,"vectors":[[...], ...]}` (unit length);
//! - any failure answers `{"id":1,"error":"..."}`.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use ort::session::{Session, SessionInputValue};
use ort::value::Tensor;
use serde::Deserialize;
use tokenizers::{Tokenizer, TruncationParams};

/// Model identity: vectors of another model are not comparable, so Humcase reindexes when it
/// changes.
const MODEL: &str = "granite-embedding-97m-multilingual-r2";
/// Longest input read by the model; Humcase cuts long cases into pieces below it.
const MAX_TOKENS: usize = 512;

struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
    inputs: Vec<String>,
    dim: usize,
}

impl Embedder {
    fn load(dir: &Path, threads: usize) -> Result<Self, String> {
        let runtime = dir.join(if cfg!(windows) {
            "onnxruntime.dll"
        } else {
            "libonnxruntime.so"
        });
        if !runtime.is_file() {
            return Err(format!("runtime not found: {}", runtime.display()));
        }
        ort::init_from(&runtime)
            .map_err(|e| format!("runtime refused: {e}"))?
            .with_name("humcase-sense")
            .commit();
        let model_dir = dir.join("model");
        let mut tokenizer = Tokenizer::from_file(model_dir.join("tokenizer.json"))
            .map_err(|e| format!("tokenizer: {e}"))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_TOKENS,
                ..Default::default()
            }))
            .map_err(|e| format!("tokenizer: {e}"))?
            .with_padding(None);
        let session = Session::builder()
            .map_err(|e| format!("session: {e}"))?
            .with_intra_threads(threads)
            .map_err(|e| format!("session: {e}"))?
            .commit_from_file(model_dir.join("model.onnx"))
            .map_err(|e| format!("model: {e}"))?;
        let inputs = session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect();
        let mut embedder = Embedder {
            session,
            tokenizer,
            inputs,
            dim: 0,
        };
        embedder.dim = embedder.embed("ready")?.len();
        Ok(embedder)
    }

    /// Unit vector of a text: the model's first token (CLS pooling), as Granite is trained.
    fn embed(&mut self, text: &str) -> Result<Vec<f32>, String> {
        let enc = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| format!("tokenizer: {e}"))?;
        let n = enc.get_ids().len();
        let column = |values: &[u32]| values.iter().map(|&x| x as i64).collect::<Vec<i64>>();
        let mut feed: Vec<(String, SessionInputValue)> = Vec::new();
        for name in &self.inputs {
            let data = match name.as_str() {
                "input_ids" => column(enc.get_ids()),
                "attention_mask" => column(enc.get_attention_mask()),
                "token_type_ids" => column(enc.get_type_ids()),
                other => return Err(format!("unexpected model input {other}")),
            };
            let tensor = Tensor::from_array(([1usize, n], data)).map_err(|e| e.to_string())?;
            feed.push((name.clone(), tensor.into()));
        }
        let outputs = self.session.run(feed).map_err(|e| e.to_string())?;
        let (shape, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        let dim = *shape.last().ok_or("empty output")? as usize;
        let mut v = data[..dim].to_vec();
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
        v.iter_mut().for_each(|x| *x /= norm);
        Ok(v)
    }
}

#[derive(Deserialize)]
struct Request {
    id: u64,
    #[serde(default)]
    embed: Vec<String>,
}

fn answer(out: &mut impl Write, value: serde_json::Value) -> std::io::Result<()> {
    writeln!(out, "{value}")?;
    out.flush()
}

/// Below-normal priority: reading every case once must not slow the person's work.
#[cfg(windows)]
fn lower_priority() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS,
    };
    // SAFETY: plain call on the current process's pseudo-handle.
    unsafe {
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
}

#[cfg(not(windows))]
fn lower_priority() {}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = arg(&args, "--dir").map(PathBuf::from).unwrap_or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_default()
    });
    let idle = Duration::from_secs(
        arg(&args, "--idle")
            .and_then(|s| s.parse().ok())
            .unwrap_or(60),
    );
    let threads = arg(&args, "--threads")
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);
    lower_priority();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut embedder = match Embedder::load(&dir, threads) {
        Ok(e) => e,
        Err(error) => {
            let _ = answer(
                &mut out,
                serde_json::json!({ "ready": false, "error": error }),
            );
            std::process::exit(2);
        }
    };
    if answer(
        &mut out,
        serde_json::json!({ "ready": true, "model": MODEL, "dim": embedder.dim }),
    )
    .is_err()
    {
        return;
    }

    // Lines are read on their own thread, so that a minute without any stops the worker.
    let (lines, receive) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if lines.send(line).is_err() {
                break;
            }
        }
    });
    while let Ok(line) = receive.recv_timeout(idle) {
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Request>(&line) {
            Ok(request) => {
                let vectors: Result<Vec<Vec<f32>>, String> =
                    request.embed.iter().map(|t| embedder.embed(t)).collect();
                match vectors {
                    Ok(vectors) => serde_json::json!({ "id": request.id, "vectors": vectors }),
                    Err(error) => serde_json::json!({ "id": request.id, "error": error }),
                }
            }
            Err(error) => serde_json::json!({ "id": 0, "error": format!("bad request: {error}") }),
        };
        if answer(&mut out, reply).is_err() {
            break;
        }
    }
}
