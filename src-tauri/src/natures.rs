//! Natures to check: cases whose nature reads like another one. Humcase proposes, the person
//! decides; nothing changes without them.
//!
//! - The reader is Jev (`crate::jev`), when the person turns the review on and a TypeSafe key is
//!   set. On the computer, neither the wording (`recognize`) nor the cases closest in meaning
//!   proposed right on a real base (measured on 04/10 with `tools/nature-review`: Jev agreed with
//!   about one proposal in six), while Jev agrees with the known natures of a multilingual
//!   corpus 95% of the time.
//! - A case is read once per version of what is sent (title and first 300 words) and of the
//!   question. The readings live in `nature-readings.sqlite` next to the database: a cache that
//!   archives do not carry. A case edited in the last two minutes waits.
//! - A nature is proposed only when both orders of the options give it with a confidence of at
//!   least `MIN_CONFIDENCE`. A nature the person chose is never questioned, nor sent: one they
//!   set in the interface (journal, v24) or kept or chose in this review (`nature_checks`, v25).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::jev::{self, Failure};
use crate::models::note::NoteType;
use crate::recognize::{self, Evidence};

/// Least confidence of Jev's reading for a proposal. On the multilingual corpus
/// (`recognize_cases.json`, 360 cases whose nature is known), 6 cases are read wrong at this
/// level against 10 at 0.7; on a real base of 450 cases, 40 proposals.
pub const MIN_CONFIDENCE: f32 = 0.9;
/// A case edited more recently than this is read later (it is still being written).
const QUIET: &str = "-120 seconds";
/// Cases read in one pass, before the database is looked at again.
const PASS_LIMIT: usize = 100;
pub const READINGS_FILE: &str = "nature-readings.sqlite";
const SETTINGS_FILE: &str = "nature-reading.json";
/// Words of the text the wording evidence is looked for in (the opening carries the nature).
const OPENING_WORDS: usize = 60;

/// Why a nature is proposed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reason {
    /// Jev reads the case as the proposed nature, with this confidence (0 to 1).
    Reading { confidence: f32 },
    /// Words of the case that say it too (see `recognize::Evidence`).
    Wording { evidence: Evidence },
}

/// A case whose nature reads like another one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Proposal {
    pub note_id: String,
    pub title: String,
    pub project_id: String,
    pub revision: i64,
    /// Its nature now.
    pub nature: NoteType,
    pub proposed: NoteType,
    pub reasons: Vec<Reason>,
    pub confidence: f32,
}

/// The person's answer to a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Kept,
    Changed,
}

impl Answer {
    fn as_str(self) -> &'static str {
        match self {
            Answer::Kept => "kept",
            Answer::Changed => "changed",
        }
    }
}

/// Records the person's answer: the nature the case keeps (`note_type`) after `proposed` was
/// offered. The latest answer replaces an earlier one.
pub fn record_answer(
    conn: &Connection,
    note_id: &str,
    note_type: NoteType,
    proposed: NoteType,
    answer: Answer,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO nature_checks (note_id, note_type, proposed, answer, checked_at)
         VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(note_id) DO UPDATE SET note_type = excluded.note_type,
           proposed = excluded.proposed, answer = excluded.answer, checked_at = excluded.checked_at",
        params![
            note_id,
            note_type.as_str(),
            proposed.as_str(),
            answer.as_str()
        ],
    )
    .map_err(crate::db::db_error)?;
    Ok(())
}

/// Cases whose nature the person chose, with that nature: set in the interface (journal) or
/// kept or chosen in the review. The person set it when the latest write of the nature is
/// theirs: a change of nature in the interface, or a creation with a nature other than the
/// default thought.
pub fn chosen_natures(conn: &Connection) -> Result<HashMap<String, NoteType>, String> {
    let mut chosen = HashMap::new();
    let mut stmt = conn
        .prepare(
            "WITH nature_writes AS (
                SELECT e.note_id, e.kind, e.actor_kind, e.detail,
                       ROW_NUMBER() OVER (PARTITION BY e.note_id
                                          ORDER BY e.last_at DESC, e.rowid DESC) AS rank
                  FROM case_events e
                 WHERE e.kind = 'created'
                    OR (e.kind = 'edited'
                        AND EXISTS (SELECT 1 FROM json_each(e.fields) f
                                     WHERE f.value = 'note_type')))
             SELECT n.id, n.note_type FROM notes n
              WHERE EXISTS (SELECT 1 FROM nature_checks c
                             WHERE c.note_id = n.id AND c.note_type = n.note_type)
                 OR EXISTS (SELECT 1 FROM nature_writes w
                             WHERE w.note_id = n.id AND w.rank = 1 AND w.actor_kind = 'human'
                               AND (w.kind = 'edited'
                                    OR json_extract(w.detail, '$.note_type') IS NOT 'note'))",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, NoteType>(1)?))
        })
        .map_err(crate::db::db_error)?;
    for row in rows {
        let (id, nature) = row.map_err(crate::db::db_error)?;
        chosen.insert(id, nature);
    }
    Ok(chosen)
}

// --- Settings -------------------------------------------------------------------------------

/// Whether the person turned the review on (off unless they did: it sends cases to TypeSafe).
pub fn enabled(data_dir: &Path) -> bool {
    std::fs::read_to_string(data_dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v["enabled"].as_bool())
        .unwrap_or(false)
}

fn write_enabled(data_dir: &Path, enabled: bool) -> Result<(), String> {
    std::fs::write(
        data_dir.join(SETTINGS_FILE),
        serde_json::json!({ "enabled": enabled }).to_string(),
    )
    .map_err(|e| e.to_string())
}

// --- Readings -------------------------------------------------------------------------------

/// Opens (and creates) the readings of `data_dir`.
pub fn open_readings(data_dir: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(data_dir.join(READINGS_FILE))?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE IF NOT EXISTS readings (
             note_id TEXT PRIMARY KEY,
             hash TEXT NOT NULL,
             nature TEXT NOT NULL,
             confidence REAL NOT NULL,
             agree INTEGER NOT NULL,
             model TEXT NOT NULL,
             read_at TEXT NOT NULL
         );",
    )?;
    Ok(conn)
}

/// Fingerprint of what Jev reads of a case with the current question: a new one reads it again.
pub fn fingerprint(title: &str, content: &str) -> String {
    let state = jev::state_of(title, content);
    let mut hash = Sha256::new();
    hash.update(jev::question_version().as_bytes());
    hash.update([0]);
    hash.update(jev::MODEL.as_bytes());
    hash.update([0]);
    hash.update(state.to_string().as_bytes());
    hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// A reading as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct Stored {
    pub hash: String,
    pub nature: NoteType,
    pub confidence: f32,
    pub agree: bool,
}

pub fn stored_readings(readings: &Connection) -> Result<HashMap<String, Stored>, String> {
    let mut stmt = readings
        .prepare("SELECT note_id, hash, nature, confidence, agree FROM readings")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Stored {
                    hash: r.get(1)?,
                    nature: r.get(2)?,
                    confidence: r.get::<_, f64>(3)? as f32,
                    agree: r.get(4)?,
                },
            ))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

fn store(
    readings: &Connection,
    note_id: &str,
    hash: &str,
    reading: &jev::NatureReading,
) -> Result<(), String> {
    readings
        .execute(
            "INSERT INTO readings (note_id, hash, nature, confidence, agree, model, read_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(note_id) DO UPDATE SET hash = excluded.hash, nature = excluded.nature,
               confidence = excluded.confidence, agree = excluded.agree, model = excluded.model,
               read_at = excluded.read_at",
            params![
                note_id,
                hash,
                reading.nature.as_str(),
                f64::from(reading.confidence),
                reading.agree,
                reading.model
            ],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

// --- Cases and proposals --------------------------------------------------------------------

/// A case as the review reads it.
pub struct Case {
    pub id: String,
    pub title: String,
    pub content: String,
    pub project_id: String,
    pub revision: i64,
    pub nature: NoteType,
    /// Not edited for two minutes: it can be read.
    pub quiet: bool,
}

/// The live text cases of `space` (all spaces when `None`); the work profiles are left out.
pub fn cases(db: &Connection, space: Option<&str>) -> Result<Vec<Case>, String> {
    let mut stmt = db
        .prepare(&format!(
            "SELECT n.id, n.title, COALESCE(n.content, ''), n.project_id, n.revision, n.note_type,
                    n.updated_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '{QUIET}')
               FROM notes n
              WHERE n.deleted_at IS NULL AND n.archived_at IS NULL AND n.content_kind = 'text'
                AND n.id NOT IN (SELECT note_id FROM work_profiles)
                AND (?1 IS NULL OR n.project_id IN (SELECT id FROM projects WHERE space = ?1))"
        ))
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([space], |r| {
            Ok(Case {
                id: r.get(0)?,
                title: r.get(1)?,
                content: r.get(2)?,
                project_id: r.get(3)?,
                revision: r.get(4)?,
                nature: r.get(5)?,
                quiet: r.get(6)?,
            })
        })
        .map_err(crate::db::db_error)?;
    rows.collect::<Result<_, _>>().map_err(crate::db::db_error)
}

/// The wording the evidence is looked for in: the title and the opening of the text.
pub fn opening(title: &str, content: &str) -> String {
    let words: Vec<&str> = content.split_whitespace().take(OPENING_WORDS).collect();
    format!("{}\n\n{}", title.trim(), words.join(" "))
}

/// Proposals among `cases`, the surest first: a current reading (same fingerprint) of another
/// nature, both orders agreeing, confident enough, on a nature the person did not choose.
pub fn propose(
    cases: &[Case],
    chosen: &HashMap<String, NoteType>,
    readings: &HashMap<String, Stored>,
) -> Vec<Proposal> {
    let mut proposals: Vec<Proposal> = cases
        .iter()
        .filter(|case| !chosen.contains_key(&case.id))
        .filter_map(|case| {
            let reading = readings.get(&case.id)?;
            if !reading.agree
                || reading.confidence < MIN_CONFIDENCE
                || reading.nature == case.nature
                || reading.hash != fingerprint(&case.title, &case.content)
            {
                return None;
            }
            let mut reasons = vec![Reason::Reading {
                confidence: reading.confidence,
            }];
            if let Some(evidence) =
                recognize::evidence(&opening(&case.title, &case.content), reading.nature)
            {
                reasons.push(Reason::Wording { evidence });
            }
            Some(Proposal {
                note_id: case.id.clone(),
                title: case.title.clone(),
                project_id: case.project_id.clone(),
                revision: case.revision,
                nature: case.nature,
                proposed: reading.nature,
                reasons,
                confidence: reading.confidence,
            })
        })
        .collect();
    proposals.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    proposals
}

/// Natures to check: the surest proposals first, at most `limit`, and where the reading stands.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Review {
    pub proposals: Vec<Proposal>,
    pub total: usize,
    /// The person turned the review on.
    pub enabled: bool,
    /// Cases Jev can read (live text cases whose nature the person did not choose)...
    pub readable: usize,
    /// ...and those it read as they are now.
    pub read: usize,
}

/// The review of `space`. Reads the database and the stored readings only: nothing is sent.
pub fn review(
    db: &Connection,
    data_dir: &Path,
    space: Option<&str>,
    limit: usize,
) -> Result<Review, String> {
    let cases = cases(db, space)?;
    let chosen = chosen_natures(db)?;
    let readings = if data_dir.join(READINGS_FILE).is_file() {
        stored_readings(&open_readings(data_dir).map_err(|e| e.to_string())?)?
    } else {
        HashMap::new()
    };
    let readable: Vec<&Case> = cases
        .iter()
        .filter(|c| !chosen.contains_key(&c.id))
        .collect();
    let read = readable
        .iter()
        .filter(|c| {
            readings
                .get(&c.id)
                .is_some_and(|r| r.hash == fingerprint(&c.title, &c.content))
        })
        .count();
    let mut proposals = propose(&cases, &chosen, &readings);
    let total = proposals.len();
    proposals.truncate(limit);
    Ok(Review {
        proposals,
        total,
        enabled: enabled(data_dir),
        readable: readable.len(),
        read,
    })
}

/// Applies the person's answer: `change` sets the proposed nature if the case still has the
/// nature that was reviewed; either way the answer is remembered.
pub fn answer(
    conn: &Connection,
    note_id: &str,
    reviewed: NoteType,
    proposed: NoteType,
    change: bool,
) -> Result<bool, String> {
    if change {
        let input = crate::models::note::UpdateNoteInput {
            status: None,
            content: None,
            resolution_comment: None,
            title: None,
            note_type: Some(proposed),
            raw_content: None,
            priority: None,
            git_branch: None,
            tags: None,
            due_at: None,
        };
        let base = serde_json::json!({ "note_type": reviewed.as_str() });
        match crate::db::queries::retry_note_update(conn, note_id, input, base)? {
            crate::db::queries::RetryNoteUpdateResult::Applied
            | crate::db::queries::RetryNoteUpdateResult::AlreadyApplied => {}
            // Changed meanwhile, or gone: the person's answer was about another state.
            _ => return Ok(false),
        }
        record_answer(conn, note_id, proposed, proposed, Answer::Changed)?;
    } else {
        let current: Option<NoteType> = conn
            .query_row(
                "SELECT note_type FROM notes WHERE id = ?1",
                [note_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(crate::db::db_error)?;
        if current != Some(reviewed) {
            return Ok(false);
        }
        record_answer(conn, note_id, reviewed, proposed, Answer::Kept)?;
    }
    Ok(true)
}

// --- Background reader ----------------------------------------------------------------------

/// Where the reading stands, for Settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Status {
    /// `off`, `no_key`, `reading`, `ready`, `key_refused` or `failed`.
    pub state: &'static str,
    pub read: usize,
    pub total: usize,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct NatureState {
    pub status: Arc<Mutex<Status>>,
    /// Set by the switch and by a new key: the reader looks again at once.
    pub recheck: Arc<AtomicBool>,
}

fn set_status(status: &Mutex<Status>, value: Status) {
    if let Ok(mut s) = status.lock() {
        *s = value;
    }
}

/// Turns the review on or off; the status follows at once, the reader within seconds.
pub fn switch(data_dir: &Path, state: &NatureState, on: bool) -> Result<(), String> {
    write_enabled(data_dir, on)?;
    set_status(
        &state.status,
        Status {
            state: if on { "reading" } else { "off" },
            ..Status::default()
        },
    );
    state.recheck.store(true, Ordering::SeqCst);
    Ok(())
}

pub(crate) fn key() -> Option<String> {
    crate::ai_settings::effective_key(jev::PROVIDER, jev::KEY_ENV, |k| std::env::var(k).ok()).0
}

fn key_fingerprint(key: &str) -> String {
    Sha256::digest(key.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// What is left after a pass.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Left {
    Nothing,
    /// More cases than one pass reads: the next pass starts at once.
    More,
    /// Cases edited in the last two minutes: read once they are quiet.
    Edited,
}

/// Cases failing in a row before a pass stops (the service, not one case, is failing).
const FAILURES_IN_A_ROW: usize = 3;

/// Where a pass reads, and what may stop it.
struct Pass<'a> {
    db: &'a Connection,
    db_path: &'a Path,
    data_dir: &'a Path,
    readings: &'a Connection,
    key: &'a str,
    status: &'a Mutex<Status>,
    /// Set when the person turns the review off or on, or sets a new key: the pass stops.
    recheck: &'a AtomicBool,
    /// Cases (id, fingerprint) Jev failed to read in this session: not asked again until they
    /// change, so one case can neither stop the reader nor cost a call at every pass.
    failed: &'a mut HashSet<(String, String)>,
}

/// One pass: reads the cases whose reading is missing or out of date, at most `PASS_LIMIT`.
/// It stops at once when the person turns the review off: no case is sent after that.
fn read_pass(pass: Pass<'_>) -> Result<Left, Failure> {
    let Pass {
        db,
        db_path,
        data_dir,
        readings,
        key,
        status,
        recheck,
        failed,
    } = pass;
    let cases = cases(db, None).map_err(Failure::Passing)?;
    let chosen = chosen_natures(db).map_err(Failure::Passing)?;
    let stored = stored_readings(readings).map_err(Failure::Passing)?;
    let readable: Vec<&Case> = cases
        .iter()
        .filter(|c| !chosen.contains_key(&c.id))
        .collect();
    let mut to_read: Vec<(&Case, String)> = readable
        .iter()
        .map(|c| (*c, fingerprint(&c.title, &c.content)))
        .filter(|(c, hash)| stored.get(&c.id).is_none_or(|r| &r.hash != hash))
        .filter(|(c, hash)| !failed.contains(&(c.id.clone(), hash.clone())))
        .collect();
    let total = readable.len();
    let mut read = total - to_read.len();
    let edited = to_read.iter().any(|(c, _)| !c.quiet);
    to_read.retain(|(c, _)| c.quiet);
    let more = to_read.len() > PASS_LIMIT;
    to_read.truncate(PASS_LIMIT);
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| Failure::Passing(e.to_string()))?;
    let mut failing = 0;
    for (case, hash) in to_read {
        if recheck.load(Ordering::SeqCst) || !enabled(data_dir) {
            return Ok(Left::More);
        }
        set_status(
            status,
            Status {
                state: "reading",
                read,
                total,
                error: None,
            },
        );
        let started = Instant::now();
        let (reading, tokens) = tauri::async_runtime::block_on(jev::read_nature(
            &client,
            key,
            &case.title,
            &case.content,
        ));
        let model = reading
            .as_ref()
            .map(|r| r.model.clone())
            .unwrap_or_else(|_| jev::MODEL.into());
        crate::ai_usage::record(
            db_path,
            &crate::ai_usage::UsageEntry {
                operation: crate::ai_usage::operation::CHECK_NATURE,
                provider: jev::PROVIDER,
                model: &model,
                language: None,
                estimated_cost_usd: crate::ai_usage::token_cost(jev::PROVIDER, &model, &tokens),
                tokens,
                audio_seconds: None,
                duration_ms: started.elapsed().as_millis() as u64,
                ok: reading.is_ok(),
                prices_as_of: crate::ai_usage::PRICES_AS_OF,
            },
        );
        match reading {
            Ok(reading) => {
                store(readings, &case.id, &hash, &reading).map_err(Failure::Passing)?;
                read += 1;
                failing = 0;
            }
            Err(Failure::Key) => return Err(Failure::Key),
            Err(Failure::Passing(error)) => {
                eprintln!("humcase: nature review: case {}: {error}", case.id);
                failed.insert((case.id.clone(), hash));
                failing += 1;
                if failing >= FAILURES_IN_A_ROW {
                    return Err(Failure::Passing(error));
                }
            }
        }
    }
    if recheck.load(Ordering::SeqCst) || !enabled(data_dir) {
        return Ok(Left::More);
    }
    set_status(
        status,
        Status {
            state: if read == total { "ready" } else { "reading" },
            read,
            total,
            error: None,
        },
    );
    Ok(if more {
        Left::More
    } else if edited {
        Left::Edited
    } else {
        Left::Nothing
    })
}

/// Waits `seconds`, or less when asked to look again.
fn pause(seconds: u64, recheck: &AtomicBool) {
    for _ in 0..seconds {
        if recheck.load(Ordering::SeqCst) {
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Starts the interface's background reader: when the review is on and a key is set, now, then
/// whenever the database changes (and every two minutes while cases wait to be read).
pub fn start_reader(db_path: PathBuf, data_dir: PathBuf, state: &NatureState) {
    let status = state.status.clone();
    let recheck = state.recheck.clone();
    std::thread::spawn(move || {
        let mut db: Option<Connection> = None;
        let mut seen_version: Option<i64> = None;
        let mut waiting_since: Option<Instant> = None;
        let mut refused_key: Option<String> = None;
        let mut failed: HashSet<(String, String)> = HashSet::new();
        loop {
            if recheck.swap(false, Ordering::SeqCst) {
                seen_version = None;
                refused_key = None;
            }
            if !enabled(&data_dir) {
                set_status(
                    &status,
                    Status {
                        state: "off",
                        ..Status::default()
                    },
                );
                pause(3, &recheck);
                continue;
            }
            let Some(key) = key() else {
                set_status(
                    &status,
                    Status {
                        state: "no_key",
                        ..Status::default()
                    },
                );
                pause(5, &recheck);
                continue;
            };
            if refused_key.as_deref() == Some(key_fingerprint(&key).as_str()) {
                pause(5, &recheck);
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
                pause(10, &recheck);
                continue;
            };
            // `data_version` changes when another connection commits: a pass only then, or when
            // cases left waiting may be read now.
            let version: Option<i64> = conn.query_row("PRAGMA data_version", [], |r| r.get(0)).ok();
            let waited = waiting_since.is_some_and(|t| t.elapsed() >= Duration::from_secs(120));
            if version.is_some() && version == seen_version && !waited {
                pause(3, &recheck);
                continue;
            }
            let result = open_readings(&data_dir)
                .map_err(|e| Failure::Passing(e.to_string()))
                .and_then(|readings| {
                    read_pass(Pass {
                        db: conn,
                        db_path: &db_path,
                        data_dir: &data_dir,
                        readings: &readings,
                        key: &key,
                        status: &status,
                        recheck: &recheck,
                        failed: &mut failed,
                    })
                });
            match result {
                Ok(left) => {
                    seen_version = (left != Left::More).then_some(version).flatten();
                    waiting_since = (left == Left::Edited).then(Instant::now);
                }
                Err(Failure::Key) => {
                    refused_key = Some(key_fingerprint(&key));
                    set_status(
                        &status,
                        Status {
                            state: "key_refused",
                            ..Status::default()
                        },
                    );
                }
                Err(Failure::Passing(error)) => {
                    eprintln!("humcase: nature review: {error}");
                    set_status(
                        &status,
                        Status {
                            state: "failed",
                            error: Some(error),
                            ..Status::default()
                        },
                    );
                    pause(300, &recheck);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(id: &str, title: &str, nature: NoteType) -> Case {
        Case {
            id: id.into(),
            title: title.into(),
            content: String::new(),
            project_id: "p".into(),
            revision: 1,
            nature,
            quiet: true,
        }
    }

    fn reading(case: &Case, nature: NoteType, confidence: f32, agree: bool) -> Stored {
        Stored {
            hash: fingerprint(&case.title, &case.content),
            nature,
            confidence,
            agree,
        }
    }

    #[test]
    fn a_sure_reading_of_another_nature_is_proposed_with_its_words() {
        let cases = vec![
            case("d", "Décision : on garde SQLite", NoteType::Note),
            case("t", "Appeler le plombier", NoteType::Todo),
        ];
        let readings = HashMap::from([
            (
                "d".to_string(),
                reading(&cases[0], NoteType::Decision, 0.97, true),
            ),
            (
                "t".to_string(),
                reading(&cases[1], NoteType::Todo, 0.99, true),
            ),
        ]);
        let proposals = propose(&cases, &HashMap::new(), &readings);
        assert_eq!(proposals.len(), 1, "the same nature is not proposed");
        assert_eq!(
            (proposals[0].note_id.as_str(), proposals[0].proposed),
            ("d", NoteType::Decision)
        );
        assert_eq!(
            proposals[0].reasons,
            vec![
                Reason::Reading { confidence: 0.97 },
                Reason::Wording {
                    evidence: Evidence::Words {
                        words: "Décision".into()
                    }
                }
            ]
        );
    }

    #[test]
    fn unsure_split_stale_or_chosen_readings_propose_nothing() {
        let c = case("d", "Décision : on garde SQLite", NoteType::Note);
        let none = |stored: Stored, chosen: HashMap<String, NoteType>| {
            propose(
                std::slice::from_ref(&c),
                &chosen,
                &HashMap::from([("d".to_string(), stored)]),
            )
            .is_empty()
        };
        assert!(none(
            reading(&c, NoteType::Decision, 0.85, true),
            HashMap::new()
        ));
        assert!(none(
            reading(&c, NoteType::Decision, 0.97, false),
            HashMap::new()
        ));
        let mut stale = reading(&c, NoteType::Decision, 0.97, true);
        stale.hash = fingerprint("Ancien titre", "");
        assert!(none(stale, HashMap::new()));
        assert!(none(
            reading(&c, NoteType::Decision, 0.97, true),
            HashMap::from([("d".to_string(), NoteType::Note)])
        ));
    }

    #[test]
    fn the_review_is_off_until_the_person_turns_it_on() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!enabled(dir.path()));
        let state = NatureState::default();
        switch(dir.path(), &state, true).unwrap();
        assert!(enabled(dir.path()));
        assert_eq!(state.status.lock().unwrap().state, "reading");
        switch(dir.path(), &state, false).unwrap();
        assert!(!enabled(dir.path()));
        assert_eq!(state.status.lock().unwrap().state, "off");
    }

    #[test]
    fn readings_are_stored_once_and_read_again_when_the_case_changes() {
        let dir = tempfile::tempdir().unwrap();
        let readings = open_readings(dir.path()).unwrap();
        let hash = fingerprint("Titre", "texte");
        let jev_reading = jev::NatureReading {
            nature: NoteType::Bug,
            confidence: 0.93,
            agree: true,
            model: "jev-1.13.0".into(),
        };
        store(&readings, "n", &hash, &jev_reading).unwrap();
        store(&readings, "n", &hash, &jev_reading).unwrap();
        let stored = stored_readings(&readings).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored["n"].nature, NoteType::Bug);
        assert_ne!(fingerprint("Titre", "texte modifié"), hash);
    }

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::schema::apply_migrations(&conn).unwrap();
        conn
    }

    fn create(conn: &Connection, project: &str, title: &str, nature: NoteType) -> String {
        crate::db::queries::create_note(
            conn,
            crate::models::note::CreateNoteInput {
                project_id: project.into(),
                title: title.into(),
                content: None,
                note_type: nature,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: None,
            },
        )
        .unwrap()
    }

    fn nature_of(conn: &Connection, id: &str) -> NoteType {
        conn.query_row("SELECT note_type FROM notes WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
    }

    /// The review reads the stored readings only, and a case the person chose is neither
    /// proposed nor counted as readable.
    #[test]
    fn the_review_reads_stored_readings_of_cases_the_person_did_not_choose() {
        use crate::db::journal::{install, Actor};
        let dir = tempfile::tempdir().unwrap();
        let conn = db();
        let p = crate::db::queries::create_project(&conn, "P", None).unwrap();
        let open = create(&conn, &p, "Décision : on garde SQLite", NoteType::Note);
        install(&conn, &Actor::Human).unwrap();
        let chosen = create(&conn, &p, "Une tâche choisie", NoteType::Todo);
        let readings = open_readings(dir.path()).unwrap();
        for (id, title) in [
            (&open, "Décision : on garde SQLite"),
            (&chosen, "Une tâche choisie"),
        ] {
            let read = jev::NatureReading {
                nature: NoteType::Decision,
                confidence: 0.95,
                agree: true,
                model: "jev-1.13.0".into(),
            };
            store(&readings, id, &fingerprint(title, ""), &read).unwrap();
        }
        let review = review(&conn, dir.path(), None, 10).unwrap();
        assert_eq!((review.readable, review.read, review.total), (1, 1, 1));
        assert_eq!(review.proposals[0].note_id, open);
        assert!(!review.enabled);
    }

    /// Turned off while a pass is under way: no case is sent after that, and the status the
    /// person set is not turned back to "reading". Fails if the pass stops checking the switch.
    #[test]
    fn a_pass_sends_nothing_once_the_review_is_off() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db();
        let p = crate::db::queries::create_project(&conn, "P", None).unwrap();
        let id = create(&conn, &p, "Une idée ancienne", NoteType::Note);
        conn.execute(
            "UPDATE notes SET updated_at = '2026-01-01T00:00:00.000Z' WHERE id = ?1",
            [&id],
        )
        .unwrap();
        let readings = open_readings(dir.path()).unwrap();
        let state = NatureState::default();
        switch(dir.path(), &state, false).unwrap();
        let mut failed = HashSet::new();
        let left = read_pass(Pass {
            db: &conn,
            db_path: &dir.path().join("humcase.db"),
            data_dir: dir.path(),
            readings: &readings,
            key: "not-a-key",
            status: &state.status,
            recheck: &state.recheck,
            failed: &mut failed,
        });
        assert_eq!(left, Ok(Left::More));
        assert!(stored_readings(&readings).unwrap().is_empty());
        assert_eq!(state.status.lock().unwrap().state, "off");
    }

    /// A case edited in the last two minutes is still being written: it waits to be read.
    #[test]
    fn a_case_being_written_waits() {
        let conn = db();
        let p = crate::db::queries::create_project(&conn, "P", None).unwrap();
        let fresh = create(&conn, &p, "En cours", NoteType::Note);
        let old = create(&conn, &p, "Ancienne", NoteType::Note);
        conn.execute(
            "UPDATE notes SET updated_at = '2026-01-01T00:00:00.000Z' WHERE id = ?1",
            [&old],
        )
        .unwrap();
        let quiet: HashMap<String, bool> = cases(&conn, None)
            .unwrap()
            .into_iter()
            .map(|c| (c.id, c.quiet))
            .collect();
        assert_eq!((quiet[&fresh], quiet[&old]), (false, true));
    }

    /// The natures the person chose come from their own writes and answers only: an agent's
    /// creation or change, or a thought created by default, chooses nothing.
    #[test]
    fn chosen_natures_are_the_persons_latest_writes_and_answers() {
        use crate::db::journal::{install, set_actor, Actor};
        let conn = db();
        let p = crate::db::queries::create_project(&conn, "P", None).unwrap();
        install(&conn, &Actor::Human).unwrap();
        let by_default = create(&conn, &p, "Thought", NoteType::Note);
        let set_at_creation = create(&conn, &p, "Bug", NoteType::Bug);
        let changed = create(&conn, &p, "Later a task", NoteType::Note);
        conn.execute(
            "UPDATE notes SET note_type = 'todo' WHERE id = ?1",
            [&changed],
        )
        .unwrap();
        let retyped_by_agent = create(&conn, &p, "Idea", NoteType::Idea);
        set_actor(&conn, &Actor::Agent(Some("Codex".into()))).unwrap();
        let by_agent = create(&conn, &p, "Decision", NoteType::Decision);
        conn.execute(
            "UPDATE notes SET note_type = 'question' WHERE id = ?1",
            [&retyped_by_agent],
        )
        .unwrap();
        let kept = create(&conn, &p, "Kept", NoteType::Note);
        record_answer(&conn, &kept, NoteType::Note, NoteType::Todo, Answer::Kept).unwrap();

        let chosen = chosen_natures(&conn).unwrap();
        let mut ids: Vec<&str> = chosen.keys().map(String::as_str).collect();
        ids.sort();
        let mut expected = vec![set_at_creation.as_str(), changed.as_str(), kept.as_str()];
        expected.sort();
        assert_eq!(ids, expected);
        assert_eq!(chosen[&changed], NoteType::Todo);
        for other in [&by_default, &retyped_by_agent, &by_agent] {
            assert!(!chosen.contains_key(other.as_str()));
        }
        // A kept nature changed since by an agent is no longer the one the person kept.
        conn.execute("UPDATE notes SET note_type = 'idea' WHERE id = ?1", [&kept])
            .unwrap();
        assert!(!chosen_natures(&conn).unwrap().contains_key(&kept));
    }

    /// Changing applies the proposal only if the case still has the nature the person saw, and
    /// an answer is remembered either way.
    #[test]
    fn an_answer_applies_to_the_nature_the_person_saw() {
        let conn = db();
        let p = crate::db::queries::create_project(&conn, "P", None).unwrap();
        let a = create(&conn, &p, "Appeler le plombier", NoteType::Note);
        assert!(answer(&conn, &a, NoteType::Note, NoteType::Todo, true).unwrap());
        assert_eq!(nature_of(&conn, &a), NoteType::Todo);

        let b = create(&conn, &p, "Postgres ou SQLite ?", NoteType::Note);
        // Changed elsewhere meanwhile: neither the change nor the answer is applied.
        conn.execute("UPDATE notes SET note_type = 'idea' WHERE id = ?1", [&b])
            .unwrap();
        assert!(!answer(&conn, &b, NoteType::Note, NoteType::Question, true).unwrap());
        assert!(!answer(&conn, &b, NoteType::Note, NoteType::Question, false).unwrap());
        assert_eq!(nature_of(&conn, &b), NoteType::Idea);
        assert!(answer(&conn, &b, NoteType::Idea, NoteType::Question, false).unwrap());

        let chosen = chosen_natures(&conn).unwrap();
        assert_eq!(
            (chosen.get(&a), chosen.get(&b)),
            (Some(&NoteType::Todo), Some(&NoteType::Idea))
        );
    }
}
