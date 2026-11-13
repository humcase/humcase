//! Search by meaning: each case is turned into vectors by Humcase's meaning worker
//! (`humcase-sense.exe`, folder `meaning/` next to Humcase.exe), kept in an index next to the
//! database, and a search compares its own vector with them. Everything stays on the computer.
//!
//! - The worker is a separate program (ONNX Runtime and the model stay out of Humcase.exe): when
//!   it is missing or refuses this machine, only meaning is lost. It stops after a minute idle.
//! - The index (`meaning-index.sqlite`) holds only vectors derived from the cases: it can be
//!   deleted at any time and is rebuilt; it is not part of backups.
//! - The interface reads the cases in the background (lowest priority, in the worker): all of
//!   them the first time, then only those whose text changed. Agents search the same index.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Worker program, in the `meaning` folder next to Humcase.exe (built from `sense/`).
#[cfg(windows)]
pub const WORKER_EXE: &str = "humcase-sense.exe";
#[cfg(not(windows))]
pub const WORKER_EXE: &str = "humcase-sense";
/// Index of vectors, in the data folder.
pub const INDEX_FILE: &str = "meaning-index.sqlite";
/// Settings of search by meaning, in the data folder (absent: on).
const SETTINGS_FILE: &str = "meaning-settings.json";
/// Words per piece of a long case (pieces stay under the model's 512 tokens).
const PIECE_WORDS: usize = 300;
/// How far a case must stand out from the others to be returned, in standard deviations of the
/// similarities of every case to the search (Granite's raw similarities sit high even for an
/// unrelated search, so a fixed threshold sorts badly). Set on the six-language bench of
/// `tools/embed-bench` (150 cases per language, 288 searches, 60 unrelated searches):
/// - `MIN_LEAD` (2.5): the target is shown for 78 of 144 searches worded without its words, and an
///   unrelated search shows something 50 times out of 60, so it is meant for when words found
///   nothing;
/// - `STRONG_LEAD` (3.0): the target is shown for 57 of 144 and an unrelated search shows
///   something 20 times out of 60, for when words already found cases. Word search finds some
///   case for almost any phrase (near forms, partial matches), so a stricter 3.5 (36 of 144, 3
///   of 60) hid meaning almost always in the app;
/// - `VERY_CLOSE_LEAD` (4.5): "very close" is the target 123 times out of 124.
pub const MIN_LEAD: f32 = 2.5;
pub const STRONG_LEAD: f32 = 3.0;
pub const VERY_CLOSE_LEAD: f32 = 4.5;
/// Below this many cases, the spread of similarities says little: a fixed similarity is used.
const FEW_CASES: usize = 20;
const FEW_CASES_SCORE: f32 = 0.8;

/// Folder of the worker: `HUMCASE_SENSE_DIR`, otherwise `meaning` next to the executable (and,
/// in debug builds, the repository's `meaning/` built by `npm run build-meaning`).
pub fn worker_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    match std::env::var_os("HUMCASE_SENSE_DIR") {
        Some(dir) => candidates.push(PathBuf::from(dir)),
        None => {
            if let Some(exe_dir) = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(Path::to_path_buf))
            {
                candidates.push(exe_dir.join("meaning"));
            }
            #[cfg(debug_assertions)]
            candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../meaning"));
        }
    }
    candidates
        .into_iter()
        .find(|dir| dir.join(WORKER_EXE).is_file())
}

/// A running worker.
struct Worker {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    model: String,
}

impl Worker {
    fn start(dir: &Path) -> Result<Self, String> {
        let mut command = Command::new(dir.join(WORKER_EXE));
        command
            .arg("--dir")
            .arg(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("meaning worker: {e}"))?;
        let stdin = child.stdin.take().ok_or("meaning worker: no input")?;
        let mut stdout = BufReader::new(child.stdout.take().ok_or("meaning worker: no output")?);
        let mut line = String::new();
        stdout
            .read_line(&mut line)
            .map_err(|e| format!("meaning worker: {e}"))?;
        let ready: serde_json::Value =
            serde_json::from_str(&line).map_err(|_| "meaning worker: no answer".to_string())?;
        if ready["ready"] != true {
            let _ = child.wait();
            return Err(format!(
                "meaning worker: {}",
                ready["error"].as_str().unwrap_or("not ready")
            ));
        }
        Ok(Worker {
            child,
            stdin,
            stdout,
            next_id: 1,
            model: ready["model"].as_str().unwrap_or_default().to_string(),
        })
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let id = self.next_id;
        self.next_id += 1;
        let request = serde_json::json!({ "id": id, "embed": texts });
        writeln!(self.stdin, "{request}")
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("meaning worker: {e}"))?;
        let mut line = String::new();
        if self
            .stdout
            .read_line(&mut line)
            .map_err(|e| format!("meaning worker: {e}"))?
            == 0
        {
            return Err("meaning worker: stopped".into());
        }
        let answer: serde_json::Value = serde_json::from_str(&line)
            .map_err(|_| "meaning worker: unreadable answer".to_string())?;
        if let Some(error) = answer["error"].as_str() {
            return Err(format!("meaning worker: {error}"));
        }
        serde_json::from_value(answer["vectors"].clone())
            .map_err(|_| "meaning worker: unreadable vectors".to_string())
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The worker of this process, started when needed and started again after it stopped idle.
#[derive(Default)]
pub struct Sense {
    worker: Mutex<Option<Worker>>,
}

impl Sense {
    /// Vectors of `texts`, and the model that made them.
    pub fn embed(&self, texts: &[String]) -> Result<(Vec<Vec<f32>>, String), String> {
        let dir = worker_dir().ok_or("meaning worker not installed")?;
        let mut slot = self.worker.lock().map_err(|e| e.to_string())?;
        // A worker that stopped after a minute idle fails once: it is started again.
        for attempt in 0..2 {
            if slot.is_none() {
                *slot = Some(Worker::start(&dir)?);
            }
            let worker = slot.as_mut().expect("started above");
            match worker.embed(texts) {
                Ok(vectors) => return Ok((vectors, worker.model.clone())),
                Err(error) if attempt == 1 => return Err(error),
                Err(_) => *slot = None,
            }
        }
        unreachable!("the second attempt returns")
    }
}

/// Text read for a case: its title and its text, without link and image targets (paths and
/// addresses carry no meaning and use up the model's length). Sketches, images and files are
/// read by their title only.
fn case_text(title: &str, content: &str, content_kind: &str) -> String {
    if content_kind != "text" && content_kind != "research" {
        return title.trim().to_string();
    }
    static TARGETS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let targets = TARGETS.get_or_init(|| regex::Regex::new(r"\]\([^)]*\)").expect("valid regex"));
    let content = targets.replace_all(content, "]");
    format!("{}\n\n{}", title.trim(), content.trim())
        .trim()
        .to_string()
}

/// Pieces of a case read separately: its text in pieces of about 300 words, and its title
/// alone (a short title says a lot that a long text dilutes).
pub fn pieces(title: &str, content: &str, content_kind: &str) -> Vec<String> {
    let text = case_text(title, content, content_kind);
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut pieces: Vec<String> = words.chunks(PIECE_WORDS).map(|c| c.join(" ")).collect();
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if !title.is_empty() && title != pieces[0] {
        pieces.push(title);
    }
    pieces
}

fn fingerprint(model: &str, piece: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(model.as_bytes());
    hash.update([0]);
    hash.update(piece.as_bytes());
    hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Opens (and creates) the index of `data_dir`.
pub fn open_index(data_dir: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(data_dir.join(INDEX_FILE))?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE IF NOT EXISTS pieces (
             note_id TEXT NOT NULL,
             piece INTEGER NOT NULL,
             hash TEXT NOT NULL,
             vector BLOB NOT NULL,
             PRIMARY KEY (note_id, piece)
         );",
    )?;
    Ok(conn)
}

fn to_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// A vector as the index stores it (little-endian f32).
pub fn from_blob(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

/// Cases whose pieces are not in the index as they are now.
pub struct Plan {
    /// Live cases with something to read.
    pub total: usize,
    /// Cases to read again: id and pieces.
    pub stale: Vec<(String, Vec<String>)>,
    /// Cases gone (erased or in the trash), whose vectors are dropped.
    pub gone: Vec<String>,
}

/// Compares the cases of the database with the index, for vectors made by `model`.
pub fn plan(db: &Connection, index: &Connection, model: &str) -> rusqlite::Result<Plan> {
    let mut stored: HashMap<String, Vec<String>> = HashMap::new();
    {
        let mut stmt = index.prepare("SELECT note_id, hash FROM pieces ORDER BY note_id, piece")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, hash) = row?;
            stored.entry(id).or_default().push(hash);
        }
    }
    let mut stmt = db.prepare(
        "SELECT id, title, COALESCE(content, ''), content_kind FROM notes WHERE deleted_at IS NULL",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut live = HashSet::new();
    let mut total = 0;
    let mut stale = Vec::new();
    for row in rows {
        let (id, title, content, kind) = row?;
        let pieces = pieces(&title, &content, &kind);
        if pieces.is_empty() {
            continue;
        }
        total += 1;
        let hashes: Vec<String> = pieces.iter().map(|p| fingerprint(model, p)).collect();
        if stored.get(&id) != Some(&hashes) {
            stale.push((id.clone(), pieces));
        }
        live.insert(id);
    }
    let gone = stored.into_keys().filter(|id| !live.contains(id)).collect();
    Ok(Plan { total, stale, gone })
}

/// Replaces the vectors of a case.
pub fn store(
    index: &mut Connection,
    note_id: &str,
    model: &str,
    pieces: &[String],
    vectors: &[Vec<f32>],
) -> rusqlite::Result<()> {
    let tx = index.transaction()?;
    tx.execute("DELETE FROM pieces WHERE note_id = ?1", [note_id])?;
    for (i, (piece, vector)) in pieces.iter().zip(vectors).enumerate() {
        tx.execute(
            "INSERT INTO pieces (note_id, piece, hash, vector) VALUES (?1, ?2, ?3, ?4)",
            params![
                note_id,
                i as i64,
                fingerprint(model, piece),
                to_blob(vector)
            ],
        )?;
    }
    tx.commit()
}

pub fn forget(index: &Connection, note_ids: &[String]) -> rusqlite::Result<()> {
    for id in note_ids {
        index.execute("DELETE FROM pieces WHERE note_id = ?1", [id])?;
    }
    Ok(())
}

/// A case found by meaning.
#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub note_id: String,
    /// Cosine similarity of its closest piece.
    pub score: f32,
    /// How far it stands out from the other cases (standard deviations), see [`MIN_LEAD`].
    pub lead: f32,
    /// Stands out enough to be shown next to cases the words found.
    pub strong: bool,
    pub very_close: bool,
}

/// Cases that stand out as closest to `query` vector, best first.
pub fn nearest(index: &Connection, query: &[f32], limit: usize) -> rusqlite::Result<Vec<Hit>> {
    let mut best: HashMap<String, f32> = HashMap::new();
    let mut stmt = index.prepare("SELECT note_id, vector FROM pieces")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
    })?;
    for row in rows {
        let (id, blob) = row?;
        let vector = from_blob(&blob);
        let score: f32 = vector.iter().zip(query).map(|(a, b)| a * b).sum();
        let entry = best.entry(id).or_insert(f32::MIN);
        *entry = entry.max(score);
    }
    let n = best.len() as f32;
    let mean = best.values().sum::<f32>() / n.max(1.0);
    let spread = (best.values().map(|s| (s - mean).powi(2)).sum::<f32>() / n.max(1.0))
        .sqrt()
        .max(1e-6);
    let few = best.len() < FEW_CASES;
    let mut hits: Vec<Hit> = best
        .into_iter()
        .map(|(note_id, score)| {
            let lead = (score - mean) / spread;
            let (shown, strong, very_close) = if few {
                let close = score >= FEW_CASES_SCORE;
                (close, close, close)
            } else {
                (
                    lead >= MIN_LEAD,
                    lead >= STRONG_LEAD,
                    lead >= VERY_CLOSE_LEAD,
                )
            };
            (
                shown,
                Hit {
                    note_id,
                    score,
                    lead,
                    strong,
                    very_close,
                },
            )
        })
        .filter(|(shown, _)| *shown)
        .map(|(_, hit)| hit)
        .collect();
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    hits.truncate(limit);
    Ok(hits)
}

/// Searches the index of `data_dir` by meaning.
pub fn search(
    sense: &Sense,
    data_dir: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<Hit>, String> {
    let query = query.split_whitespace().collect::<Vec<_>>().join(" ");
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let (vectors, _) = sense.embed(&[query])?;
    let index = open_index(data_dir).map_err(|e| e.to_string())?;
    nearest(&index, &vectors[0], limit).map_err(|e| e.to_string())
}

/// Whether the index of `data_dir` holds any vector (the interface read the cases at least once).
pub fn has_vectors(data_dir: &Path) -> bool {
    open_index(data_dir)
        .and_then(|i| i.query_row("SELECT EXISTS (SELECT 1 FROM pieces)", [], |r| r.get(0)))
        .unwrap_or(false)
}

/// Whether search by meaning is on (it is unless the person turned it off).
pub fn enabled(data_dir: &Path) -> bool {
    std::fs::read_to_string(data_dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v["enabled"].as_bool())
        .unwrap_or(true)
}

pub fn set_enabled(data_dir: &Path, enabled: bool) -> Result<(), String> {
    std::fs::write(
        data_dir.join(SETTINGS_FILE),
        serde_json::json!({ "enabled": enabled }).to_string(),
    )
    .map_err(|e| e.to_string())
}

/// Where the reading of the cases stands, for the interface.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Turned off by the person.
    Off,
    /// The worker is missing or refused this machine (`error` says why).
    Unavailable,
    /// Cases are being read (`done` of `total`).
    Reading,
    /// Every case is read.
    Ready,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub state: State,
    pub done: usize,
    pub total: usize,
    pub error: Option<String>,
}

impl Default for Status {
    fn default() -> Self {
        Status {
            state: State::Reading,
            done: 0,
            total: 0,
            error: None,
        }
    }
}

/// State shared by the interface's commands and its background reader.
#[derive(Default)]
pub struct MeaningState {
    pub sense: Arc<Sense>,
    pub status: Arc<Mutex<Status>>,
    /// Set when search by meaning is turned back on: the reader then makes a pass even though
    /// no case changed, so the state it shows is its own again.
    pub recheck: Arc<AtomicBool>,
}

/// Turns search by meaning on or off and shows it at once (the background reader takes over
/// within seconds): the interface asking right after the switch must not see the old state.
pub fn switch(data_dir: &Path, state: &MeaningState, enabled: bool) -> Result<(), String> {
    set_enabled(data_dir, enabled)?;
    if let Ok(mut status) = state.status.lock() {
        if !enabled {
            *status = Status {
                state: State::Off,
                ..Status::default()
            };
        } else if status.state == State::Off {
            *status = Status::default();
        }
    }
    if enabled {
        state.recheck.store(true, Ordering::SeqCst);
    }
    Ok(())
}

fn set_status(status: &Mutex<Status>, value: Status) {
    if let Ok(mut s) = status.lock() {
        *s = value;
    }
}

/// Shows "off" and tells the pass to stop when the person turned search by meaning off: the
/// state they chose is never turned back to "reading" or "ready" by a pass under way.
fn turned_off(data_dir: &Path, status: &Mutex<Status>) -> bool {
    if enabled(data_dir) {
        return false;
    }
    set_status(
        status,
        Status {
            state: State::Off,
            ..Status::default()
        },
    );
    true
}

/// One pass of the background reader: reads every stale case, drops the gone ones. Returns the
/// model used, or the worker's error. It stops when the person turns search by meaning off.
fn read_pass(
    sense: &Sense,
    db: &Connection,
    index: &mut Connection,
    data_dir: &Path,
    status: &Mutex<Status>,
    model: &mut Option<String>,
) -> Result<(), String> {
    // The model is known once the worker answered; until then, ask it with a first case.
    let current = match model.clone() {
        Some(m) => m,
        None => {
            let (_, m) = sense.embed(&["humcase".to_string()])?;
            *model = Some(m.clone());
            m
        }
    };
    let plan = plan(db, index, &current).map_err(|e| e.to_string())?;
    forget(index, &plan.gone).map_err(|e| e.to_string())?;
    let mut done = plan.total - plan.stale.len();
    if turned_off(data_dir, status) {
        return Ok(());
    }
    if plan.stale.is_empty() {
        set_status(
            status,
            Status {
                state: State::Ready,
                done,
                total: plan.total,
                error: None,
            },
        );
        return Ok(());
    }
    for (note_id, pieces) in &plan.stale {
        if turned_off(data_dir, status) {
            return Ok(());
        }
        set_status(
            status,
            Status {
                state: State::Reading,
                done,
                total: plan.total,
                error: None,
            },
        );
        let (vectors, used) = sense.embed(pieces)?;
        if used != current {
            // Another model was installed: every vector is read again on the next pass.
            *model = Some(used);
            return Ok(());
        }
        store(index, note_id, &current, pieces, &vectors).map_err(|e| e.to_string())?;
        done += 1;
    }
    if turned_off(data_dir, status) {
        return Ok(());
    }
    set_status(
        status,
        Status {
            state: State::Ready,
            done,
            total: plan.total,
            error: None,
        },
    );
    Ok(())
}

/// Starts the interface's background reader: on now, then whenever the database changes.
pub fn start_reader(db_path: PathBuf, data_dir: PathBuf, state: &MeaningState) {
    let sense = state.sense.clone();
    let status = state.status.clone();
    let recheck = state.recheck.clone();
    std::thread::spawn(move || {
        let mut model: Option<String> = None;
        let mut seen_version: Option<i64> = None;
        let mut db: Option<Connection> = None;
        loop {
            if !enabled(&data_dir) {
                set_status(
                    &status,
                    Status {
                        state: State::Off,
                        ..Status::default()
                    },
                );
                seen_version = None;
                std::thread::sleep(Duration::from_secs(3));
                continue;
            }
            if worker_dir().is_none() {
                set_status(
                    &status,
                    Status {
                        state: State::Unavailable,
                        error: Some("meaning worker not installed".into()),
                        ..Status::default()
                    },
                );
                std::thread::sleep(Duration::from_secs(60));
                continue;
            }
            if db.is_none() {
                db = Connection::open_with_flags(
                    &db_path,
                    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )
                .ok();
            }
            let Some(conn) = db.as_ref() else {
                std::thread::sleep(Duration::from_secs(10));
                continue;
            };
            if recheck.swap(false, Ordering::SeqCst) {
                seen_version = None;
            }
            // `data_version` changes when another connection commits: a pass only then.
            let version: Option<i64> = conn.query_row("PRAGMA data_version", [], |r| r.get(0)).ok();
            if version.is_some() && version == seen_version {
                std::thread::sleep(Duration::from_secs(3));
                continue;
            }
            let result = open_index(&data_dir)
                .map_err(|e| e.to_string())
                .and_then(|mut index| {
                    read_pass(&sense, conn, &mut index, &data_dir, &status, &mut model)
                });
            match result {
                Ok(()) => seen_version = version,
                Err(_) if turned_off(&data_dir, &status) => {}
                Err(error) => {
                    eprintln!("humcase: search by meaning unavailable: {error}");
                    set_status(
                        &status,
                        Status {
                            state: State::Unavailable,
                            error: Some(error),
                            ..Status::default()
                        },
                    );
                    std::thread::sleep(Duration::from_secs(60));
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_shows_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let state = MeaningState::default();
        set_status(
            &state.status,
            Status {
                state: State::Ready,
                done: 3,
                total: 3,
                error: None,
            },
        );
        switch(dir.path(), &state, false).unwrap();
        assert!(!enabled(dir.path()));
        assert_eq!(state.status.lock().unwrap().state, State::Off);
        switch(dir.path(), &state, true).unwrap();
        assert!(enabled(dir.path()));
        assert_eq!(state.status.lock().unwrap().state, State::Reading);
        // Turned back on before the reader noticed it was off: it must still read again.
        assert!(state.recheck.load(Ordering::SeqCst));
    }

    /// Turned off while cases wait to be read: the pass reads none and the state stays "off".
    /// Fails if the pass stops checking the switch (it would ask the worker, missing here).
    #[test]
    fn a_pass_reads_nothing_once_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let db = Connection::open_in_memory().unwrap();
        crate::db::schema::apply_migrations(&db).unwrap();
        let p = crate::db::queries::create_project(&db, "P", None).unwrap();
        crate::db::queries::create_note(
            &db,
            crate::models::note::CreateNoteInput {
                project_id: p,
                title: "Une idée".into(),
                content: Some("À lire.".into()),
                note_type: crate::models::note::NoteType::Idea,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: None,
            },
        )
        .unwrap();
        let mut index = open_index(dir.path()).unwrap();
        let state = MeaningState::default();
        switch(dir.path(), &state, false).unwrap();
        let mut model = Some("model".to_string());
        let pass = read_pass(
            &state.sense,
            &db,
            &mut index,
            dir.path(),
            &state.status,
            &mut model,
        );
        assert_eq!(pass, Ok(()));
        assert_eq!(state.status.lock().unwrap().state, State::Off);
    }

    #[test]
    fn a_case_is_read_in_pieces_with_its_title() {
        assert!(pieces("", "", "text").is_empty());
        assert_eq!(pieces("Idée", "", "text"), vec!["Idée"]);
        let long = "mot ".repeat(650);
        let p = pieces("Titre", &long, "text");
        // 651 words: three pieces of text, then the title alone.
        assert_eq!(p.len(), 4);
        assert_eq!(p[3], "Titre");
        // Link and image targets are not read; a sketch is read by its title.
        assert_eq!(
            pieces("T", "voir [doc](https://x.y/z) ![img](C:/a.png)", "text")[0],
            "T voir [doc] ![img]"
        );
        assert_eq!(pieces("Plan", "{\"elements\":[]}", "sketch"), vec!["Plan"]);
    }

    #[test]
    fn the_index_keeps_one_vector_per_piece_and_finds_the_closest_case() {
        let dir = tempfile::tempdir().unwrap();
        let mut index = open_index(dir.path()).unwrap();
        let p = vec!["a".to_string(), "b".to_string()];
        store(&mut index, "n1", "m", &p, &[vec![1.0, 0.0], vec![0.0, 1.0]]).unwrap();
        store(&mut index, "n2", "m", &p[..1], &[vec![0.8, 0.6]]).unwrap();
        // Few cases: a fixed similarity decides; a case scores as its closest piece.
        let hits = nearest(&index, &[0.0, 1.0], 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].note_id, "n1");
        assert!(hits[0].very_close);
        assert!(nearest(&index, &[-1.0, 0.0], 10).unwrap().is_empty());
        forget(&index, &["n1".into()]).unwrap();
        assert!(nearest(&index, &[0.0, 1.0], 10).unwrap().is_empty());
    }

    #[test]
    fn a_case_is_returned_when_it_stands_out_from_the_others() {
        let dir = tempfile::tempdir().unwrap();
        let mut index = open_index(dir.path()).unwrap();
        let unit = |angle: f32| vec![angle.cos(), angle.sin()];
        // Forty cases close together around 0.52 rad, one right on the search.
        for i in 0..40 {
            let piece = vec![format!("c{i}")];
            store(
                &mut index,
                &format!("c{i}"),
                "m",
                &piece,
                &[unit(0.5 + i as f32 * 0.001)],
            )
            .unwrap();
        }
        store(&mut index, "target", "m", &["t".to_string()], &[unit(0.0)]).unwrap();
        let hits = nearest(&index, &unit(0.0), 10).unwrap();
        assert_eq!(hits[0].note_id, "target");
        assert!(hits[0].strong && hits[0].very_close);
        assert!(hits.iter().all(|h| h.lead >= MIN_LEAD));
        // A search close to nobody in particular returns nothing.
        assert!(nearest(&index, &unit(1.6), 10)
            .unwrap()
            .iter()
            .all(|h| h.note_id != "target"));
    }

    #[test]
    fn only_changed_and_new_cases_are_read_again() {
        let dir = tempfile::tempdir().unwrap();
        let db = Connection::open_in_memory().unwrap();
        crate::db::schema::apply_migrations(&db).unwrap();
        let project = crate::db::queries::create_project(&db, "P", None).unwrap();
        let note = crate::db::queries::create_note(
            &db,
            crate::models::note::CreateNoteInput {
                project_id: project,
                title: "Sauvegarde".into(),
                content: Some("Les liens se perdent".into()),
                note_type: crate::models::note::NoteType::Bug,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: None,
            },
        )
        .unwrap();
        let mut index = open_index(dir.path()).unwrap();
        let first = plan(&db, &index, "m").unwrap();
        assert_eq!((first.total, first.stale.len()), (1, 1));
        let (id, pieces) = &first.stale[0];
        let vectors: Vec<Vec<f32>> = pieces.iter().map(|_| vec![1.0]).collect();
        store(&mut index, id, "m", pieces, &vectors).unwrap();
        assert!(plan(&db, &index, "m").unwrap().stale.is_empty());
        // Another model reads everything again.
        assert_eq!(plan(&db, &index, "other").unwrap().stale.len(), 1);
        // A case in the trash is dropped from the index.
        crate::db::queries::trash_note(&db, &note).unwrap();
        assert_eq!(plan(&db, &index, "m").unwrap().gone, vec![note]);
    }

    #[test]
    fn search_by_meaning_is_on_until_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        assert!(enabled(dir.path()));
        set_enabled(dir.path(), false).unwrap();
        assert!(!enabled(dir.path()));
        set_enabled(dir.path(), true).unwrap();
        assert!(enabled(dir.path()));
    }
}
