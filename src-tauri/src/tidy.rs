//! Keeping the memory tidy ("To tidy"): Humcase proposes to archive cases resolved long ago and
//! to mark an older decision as replaced by a newer one; the person answers, nothing is removed.
//!
//! Both kinds live in the `proposals` table beside the sub-cases of Sprout (`kind` `archive` and
//! `supersede`). Archiving needs no AI: a top-level case resolved for some weeks, with nothing
//! open under it. Decisions that may replace one another are found by the words of their titles
//! and, when the index is ready, by the meaning of what they say; Jev (when the person lets it
//! read their cases) judges each pair once: same subject, and does the newer replace the older.
//! A replaced decision stays readable, marked as history for the person and for agents
//! (`case_supersessions`). Cases to file one inside the other and cases that may go against a
//! decision (`group`, `conflict`) are found by `crate::gather` and listed here with the rest.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::{permissions, queries};
use crate::models::note::NoteType;

/// Weeks a top-level case stays resolved before Humcase proposes to archive it (`0`: never).
pub const ARCHIVE_WEEKS: &str = "tidy_archive_weeks";
/// Look for the decisions a newer decision replaces (`0`: off).
pub const LOOK_FOR_SUPERSEDED: &str = "tidy_supersede";
/// The delays the person can choose.
pub const WEEK_CHOICES: [u32; 4] = [0, 2, 4, 8];
const DEFAULT_WEEKS: u32 = 4;
/// Archive proposals shown at once, oldest first.
const ARCHIVE_LIMIT: usize = 100;
/// Pairs of decisions sent to Jev in one request.
const PAIRS_PER_CHECK: usize = 12;
/// Jev's chance that the newer decision replaces the older: from here the pair is proposed.
/// Measured by `tidy_eval` (15 pairs, six languages): replacements 0.88 to 0.97, a decision that
/// adds to or confirms the older one 0.06 to 0.12, another subject 0.03 to 0.04.
pub const REPLACES_MIN: f64 = 0.7;
/// Below this chance the pair is not proposed; between the two it is proposed "to check".
pub const REPLACES_DOUBT: f64 = 0.4;
/// Cosine similarity of two decisions' closest pieces from which they are read as close in
/// meaning (Granite R2, `tidy_eval`): every replacement 0.81 or more, another subject 0.76 or
/// less. Pairs on the same subject that both hold are close too (0.86 to 0.91), and so are their
/// titles' words: only Jev tells a replacement from an addition, so Humcase proposes a pair only
/// once Jev judged it (the first pairs proposed without it, on a real space, were all additions).
pub const MEANING_CLOSE: f32 = 0.80;
/// Characters of a case shown as its excerpt.
const EXCERPT_MAX: usize = 220;

/// Jev's questions, versioned with the code.
const JEV_QUESTIONS: &str = include_str!("jev_tidy.v1.json");

// --- Settings -------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TidySettings {
    /// Weeks before archiving is proposed, `0` for never.
    pub archive_weeks: u32,
    /// Compare decisions: the ones a newer decision replaces, the cases that go against one.
    pub supersede: bool,
    /// Propose to file close cases one inside the other.
    #[serde(default = "yes")]
    pub group: bool,
}

fn yes() -> bool {
    true
}

pub fn settings(conn: &Connection) -> Result<TidySettings, String> {
    let weeks = permissions::get_setting(conn, ARCHIVE_WEEKS)?
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|w| WEEK_CHOICES.contains(w))
        .unwrap_or(DEFAULT_WEEKS);
    let supersede = permissions::get_setting(conn, LOOK_FOR_SUPERSEDED)?.as_deref() != Some("0");
    Ok(TidySettings {
        archive_weeks: weeks,
        supersede,
        group: crate::gather::group_enabled(conn)?,
    })
}

pub fn set_settings(conn: &Connection, settings: &TidySettings) -> Result<(), String> {
    if !WEEK_CHOICES.contains(&settings.archive_weeks) {
        return Err(crate::tr!("tidy.weeks", weeks = settings.archive_weeks));
    }
    permissions::set_setting(conn, ARCHIVE_WEEKS, &settings.archive_weeks.to_string())?;
    permissions::set_setting(
        conn,
        LOOK_FOR_SUPERSEDED,
        if settings.supersede { "1" } else { "0" },
    )?;
    crate::gather::set_group_enabled(conn, settings.group)
}

// --- What the person sees -------------------------------------------------------------------

/// A case as the list shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CaseRef {
    pub id: String,
    pub title: String,
    pub nature: NoteType,
    pub project_name: String,
    /// The space of the Inbox when the case is in one (`project_name` is then technical).
    pub inbox_space: Option<String>,
    /// When it was written (`created_at`) for a decision, when it last changed for a case to
    /// archive (`updated_at`).
    pub date: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TidyKind {
    Archive,
    Supersede,
    /// File `case` inside `other`.
    Group,
    /// `case` may go against the decision `other`.
    Conflict,
}

/// A proposal of "To tidy".
#[derive(Debug, Clone, Serialize)]
pub struct TidyItem {
    pub id: String,
    pub kind: TidyKind,
    /// The case to archive, the older decision, the case to file, or the case that may go
    /// against a decision.
    pub case: CaseRef,
    /// The newer decision that may replace `case`, the case to file it in, or the decision.
    pub other: Option<CaseRef>,
    /// Conflict: what `case` says on the point, and what the decision says.
    pub passage: Option<String>,
    pub other_passage: Option<String>,
    /// Group: the two cases say nearly the same thing (found by words, without AI).
    pub duplicate: bool,
    /// Jev doubts, or nobody checked: the person looks closer.
    pub doubt: bool,
    /// Jev judged the pair.
    pub checked: bool,
    /// The agent that proposed it, `None` for Humcase.
    pub proposed_by: Option<String>,
    pub created_at: String,
}

/// The first line of text of a case, without Markdown headings and marks, clipped.
pub(crate) fn excerpt(content: &str) -> String {
    let line = content
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("```"))
        .unwrap_or("");
    let line = line
        .trim_start_matches(['-', '*', '>', ' '])
        .replace("**", "")
        .replace('`', "");
    let mut out: String = line.chars().take(EXCERPT_MAX).collect();
    if line.chars().count() > EXCERPT_MAX {
        out = out.trim_end().to_string() + "…";
    }
    out
}

pub(crate) fn case_ref(
    conn: &Connection,
    id: &str,
    date_column: &str,
) -> Result<Option<CaseRef>, String> {
    let sql = format!(
        "SELECT n.id, n.title, n.note_type, p.name, n.{date_column}, COALESCE(n.content, ''),
                CASE WHEN p.is_inbox THEN COALESCE(p.space, 'work') END
           FROM notes n JOIN projects p ON p.id = n.project_id
          WHERE n.id = ?1 AND n.deleted_at IS NULL"
    );
    conn.query_row(&sql, [id], |r| {
        Ok(CaseRef {
            id: r.get(0)?,
            title: r.get(1)?,
            nature: r.get(2)?,
            project_name: r.get(3)?,
            inbox_space: r.get(6)?,
            date: r.get(4)?,
            excerpt: excerpt(&r.get::<_, String>(5)?),
        })
    })
    .optional()
    .map_err(crate::db::db_error)
}

/// Proposals waiting in `space` (`work` or `private`; every space when `None`), decisions first,
/// then the oldest cases to archive. Archive proposals are left out when the person chose
/// "never".
pub fn pending(conn: &Connection, space: Option<&str>) -> Result<Vec<TidyItem>, String> {
    let settings = settings(conn)?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT pr.id, pr.kind, pr.note_id, pr.near_id, pr.follows, pr.proposed_by, pr.created_at,
                    pr.passage, pr.reason
               FROM proposals pr
               JOIN notes n ON n.id = pr.note_id
               JOIN projects p ON p.id = n.project_id
              WHERE pr.answer IS NULL AND pr.kind IN ('archive', 'supersede', 'group', 'conflict')
                AND NOT (pr.kind = 'supersede' AND pr.follows IS NULL AND pr.proposed_by IS NULL)
                AND (?1 IS NULL OR p.space = ?1) AND n.deleted_at IS NULL
                AND NOT EXISTS (SELECT 1 FROM proposals stale WHERE stale.id = pr.id AND {STALE})
              ORDER BY CASE pr.kind WHEN 'conflict' THEN 0 WHEN 'supersede' THEN 1
                                    WHEN 'group' THEN 2 ELSE 3 END,
                       n.updated_at, pr.created_at"
        ))
        .map_err(crate::db::db_error)?;
    type Row = (
        String,
        String,
        String,
        Option<String>,
        Option<f64>,
        Option<String>,
        String,
        String,
        Option<String>,
    );
    let rows: Vec<Row> = stmt
        .query_map([space], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
            ))
        })
        .map_err(crate::db::db_error)?
        .collect::<Result<_, _>>()
        .map_err(crate::db::db_error)?;
    let mut items = Vec::new();
    for (id, kind, note_id, near_id, chance, proposed_by, created_at, passage, reason) in rows {
        if kind == "group" || kind == "conflict" {
            let group = kind == "group";
            if (group && !settings.group) || (!group && !settings.supersede) {
                continue;
            }
            let date = if group { "updated_at" } else { "created_at" };
            let Some(case) = case_ref(conn, &note_id, date)? else {
                continue;
            };
            let Some(other) = near_id
                .as_deref()
                .map(|n| case_ref(conn, n, "created_at"))
                .transpose()?
                .flatten()
            else {
                continue;
            };
            let duplicate = group && reason.as_deref() == Some("duplicate");
            let (min, cited) = if group {
                (crate::gather::TOGETHER_MIN, (None, None))
            } else {
                let passage = Some(passage).filter(|p| !p.is_empty());
                (crate::gather::CONFLICT_MIN, (passage, reason))
            };
            items.push(TidyItem {
                id,
                kind: if group {
                    TidyKind::Group
                } else {
                    TidyKind::Conflict
                },
                case,
                other: Some(other),
                passage: cited.0,
                other_passage: cited.1,
                duplicate,
                // Found by words alone: the person looks before filing (templated cases share
                // most of their words).
                doubt: chance.is_none_or(|c| c < min),
                checked: chance.is_some(),
                proposed_by,
                created_at,
            });
            continue;
        }
        let replaces = chance;
        if kind == "archive" {
            if settings.archive_weeks == 0 {
                continue;
            }
            let Some(case) = case_ref(conn, &note_id, "updated_at")? else {
                continue;
            };
            items.push(TidyItem {
                id,
                kind: TidyKind::Archive,
                case,
                other: None,
                passage: None,
                other_passage: None,
                duplicate: false,
                doubt: false,
                checked: false,
                proposed_by,
                created_at,
            });
        } else {
            let Some(case) = case_ref(conn, &note_id, "created_at")? else {
                continue;
            };
            let Some(newer) = near_id
                .as_deref()
                .map(|n| case_ref(conn, n, "created_at"))
                .transpose()?
                .flatten()
            else {
                continue;
            };
            items.push(TidyItem {
                id,
                kind: TidyKind::Supersede,
                case,
                other: Some(newer),
                passage: None,
                other_passage: None,
                duplicate: false,
                doubt: replaces.is_none_or(|r| r < REPLACES_MIN),
                checked: replaces.is_some(),
                proposed_by,
                created_at,
            });
        }
    }
    Ok(items)
}

// --- Archiving ------------------------------------------------------------------------------

/// Proposes to archive the top-level cases of the database resolved for the chosen weeks, with
/// nothing open under them and nothing in force: a decision that no newer one replaced stays.
/// A case the person refused, or archived then took back, is not proposed again; a waiting
/// proposal whose case no longer qualifies is withdrawn (`replaced`). Returns the count added.
/// A result an agent declared that the person has not accepted yet, or sent back for a rework.
fn awaits_decision(conn: &Connection, note_id: &str) -> Result<bool, String> {
    let state = crate::db::journal::review_state(conn, note_id)?.state;
    Ok(state == "to_review" || state == "rework_requested")
}

pub fn refresh_archive(conn: &Connection) -> Result<usize, String> {
    let weeks = settings(conn)?.archive_weeks;
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let now: String = tx
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |r| {
            r.get(0)
        })
        .map_err(crate::db::db_error)?;
    // At most ARCHIVE_LIMIT proposals wait at once: each refresh only fills the room left, so the
    // count does not climb on its own from one visit to the next.
    let waiting_now: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM proposals WHERE kind = 'archive' AND answer IS NULL",
            [],
            |r| r.get(0),
        )
        .map_err(crate::db::db_error)?;
    let room = ARCHIVE_LIMIT.saturating_sub(waiting_now.max(0) as usize);
    // The cases that qualify now. Sub-cases are tidied with their top-level case.
    let eligible: Vec<(String, String, String)> = if weeks == 0 || room == 0 {
        Vec::new()
    } else {
        let mut stmt = tx
            .prepare(
                "WITH RECURSIVE below(root, id) AS (
                     SELECT id, id FROM notes WHERE parent_id IS NULL
                     UNION ALL
                     SELECT below.root, n.id FROM notes n JOIN below ON n.parent_id = below.id
                 )
                 SELECT n.id, n.note_type, n.title FROM notes n
                  WHERE n.parent_id IS NULL AND n.deleted_at IS NULL AND n.archived_at IS NULL
                    AND n.status = 'resolved'
                    AND n.id NOT IN (SELECT note_id FROM work_profiles)
                    AND n.updated_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?1)
                    AND (n.note_type <> 'decision'
                         OR EXISTS (SELECT 1 FROM case_supersessions s
                                     JOIN notes b ON b.id = s.by_id AND b.deleted_at IS NULL
                                    WHERE s.note_id = n.id))
                    AND NOT EXISTS (
                        SELECT 1 FROM below JOIN notes c ON c.id = below.id
                         WHERE below.root = n.id AND c.id <> n.id AND c.deleted_at IS NULL
                           AND c.status <> 'resolved')
                    AND NOT EXISTS (
                        SELECT 1 FROM proposals pr
                         WHERE pr.note_id = n.id AND pr.kind = 'archive'
                           AND (pr.answer IS NULL OR pr.answer IN ('refused', 'accepted')))
                  ORDER BY n.updated_at
                  LIMIT ?2",
            )
            .map_err(crate::db::db_error)?;
        let rows = stmt
            .query_map(params![format!("-{} days", weeks * 7), room as i64], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .map_err(crate::db::db_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(crate::db::db_error)?;
        rows
    };
    // A result still waiting for the person's decision is never archived behind their back.
    let mut eligible_now = Vec::with_capacity(eligible.len());
    for row in eligible {
        if !awaits_decision(&tx, &row.0)? {
            eligible_now.push(row);
        }
    }
    let eligible = eligible_now;
    // Waiting proposals whose case changed since: reopened, archived, trashed, given work, or
    // now a result waiting for the person's decision.
    let waiting: Vec<String> = {
        let mut stmt = tx
            .prepare(
                "SELECT pr.id, pr.note_id,
                        (n.status <> 'resolved' OR n.archived_at IS NOT NULL
                         OR n.deleted_at IS NOT NULL OR n.parent_id IS NOT NULL
                         OR ?1 = 0
                         OR n.updated_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2))
                   FROM proposals pr JOIN notes n ON n.id = pr.note_id
                  WHERE pr.kind = 'archive' AND pr.answer IS NULL",
            )
            .map_err(crate::db::db_error)?;
        let rows: Vec<(String, String, bool)> = stmt
            .query_map(params![weeks, format!("-{} days", weeks * 7)], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .map_err(crate::db::db_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(crate::db::db_error)?;
        let mut withdrawn = Vec::new();
        for (id, note_id, changed) in rows {
            if changed || awaits_decision(&tx, &note_id)? {
                withdrawn.push(id);
            }
        }
        withdrawn
    };
    for id in &waiting {
        tx.execute(
            "UPDATE proposals SET answer = 'replaced', answered_at = ?2 WHERE id = ?1",
            params![id, now],
        )
        .map_err(crate::db::db_error)?;
    }
    for (position, (note_id, nature, title)) in eligible.iter().enumerate() {
        tx.execute(
            "INSERT INTO proposals (id, note_id, kind, nature, title, passage, revision, position, created_at)
             VALUES (?1, ?2, 'archive', ?3, ?4, '', (SELECT revision FROM notes WHERE id = ?2), ?5, ?6)",
            params![Uuid::new_v4().to_string(), note_id, nature, title, position as i64, now],
        )
        .map_err(crate::db::db_error)?;
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(eligible.len())
}

// --- Decisions that may replace one another --------------------------------------------------

/// A decision as compared with the others.
#[derive(Debug, Clone)]
pub struct Decision {
    pub id: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub space: Option<String>,
}

pub fn decisions(conn: &Connection) -> Result<Vec<Decision>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT n.id, n.title, COALESCE(n.content, ''), n.created_at, p.space
               FROM notes n JOIN projects p ON p.id = n.project_id
              WHERE n.note_type = 'decision' AND n.deleted_at IS NULL
              ORDER BY n.created_at",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Decision {
                id: r.get(0)?,
                title: r.get(1)?,
                content: r.get(2)?,
                created_at: r.get(3)?,
                space: r.get(4)?,
            })
        })
        .map_err(crate::db::db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(crate::db::db_error)?;
    Ok(rows)
}

/// Two decisions that may replace one another, before Jev.
#[derive(Debug, Clone, PartialEq)]
pub struct Pair {
    pub older: String,
    pub newer: String,
}

/// Cosine similarity of the closest pieces of two cases (vectors are normalized).
pub(crate) fn closest(a: &[Vec<f32>], b: &[Vec<f32>]) -> f32 {
    let mut best = f32::MIN;
    for x in a {
        for y in b {
            best = best.max(x.iter().zip(y).map(|(p, q)| p * q).sum());
        }
    }
    best
}

/// The vectors of each case in the index, by case.
pub(crate) fn vectors_of(index: &Connection, ids: &[String]) -> HashMap<String, Vec<Vec<f32>>> {
    let mut by_case: HashMap<String, Vec<Vec<f32>>> = HashMap::new();
    let Ok(mut stmt) = index.prepare("SELECT vector FROM pieces WHERE note_id = ?1") else {
        return by_case;
    };
    for id in ids {
        let Ok(rows) = stmt.query_map([id], |r| r.get::<_, Vec<u8>>(0)) else {
            continue;
        };
        let vectors: Vec<Vec<f32>> = rows
            .flatten()
            .map(|b| crate::meaning::from_blob(&b))
            .collect();
        if !vectors.is_empty() {
            by_case.insert(id.clone(), vectors);
        }
    }
    by_case
}

/// Pairs of decisions of one space close by the words of their titles or by meaning, the older
/// one not replaced yet, never judged before. `newer_only`: the pairs where this decision is the
/// newer one (the check when it is written).
pub fn candidate_pairs(
    conn: &Connection,
    data_dir: Option<&Path>,
    newer_only: Option<&str>,
) -> Result<Vec<Pair>, String> {
    let all = decisions(conn)?;
    let replaced: HashSet<String> = {
        let mut stmt = conn
            .prepare("SELECT note_id FROM case_supersessions")
            .map_err(crate::db::db_error)?;
        let rows = stmt
            .query_map([], |r| r.get(0))
            .map_err(crate::db::db_error)?
            .collect::<Result<_, _>>()
            .map_err(crate::db::db_error)?;
        rows
    };
    let asked: HashSet<(String, String)> = {
        let mut stmt = conn
            // A pair waiting unchecked (stored by the first version, which proposed very close
            // pairs without Jev) is judged again once Jev may read; until then it is not shown.
            .prepare(
                "SELECT note_id, near_id FROM proposals
                  WHERE kind = 'supersede' AND near_id IS NOT NULL
                    AND NOT (answer IS NULL AND follows IS NULL AND proposed_by IS NULL)",
            )
            .map_err(crate::db::db_error)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(crate::db::db_error)?
            .collect::<Result<_, _>>()
            .map_err(crate::db::db_error)?;
        rows
    };
    let vectors = data_dir
        .filter(|dir| crate::meaning::enabled(dir) && crate::meaning::has_vectors(dir))
        .and_then(|dir| crate::meaning::open_index(dir).ok())
        .map(|index| {
            let ids: Vec<String> = all.iter().map(|d| d.id.clone()).collect();
            vectors_of(&index, &ids)
        })
        .unwrap_or_default();
    let words: Vec<HashSet<String>> = all
        .iter()
        .map(|d| crate::germinate::words(&d.title))
        .collect();
    let mut pairs = Vec::new();
    for (j, newer) in all.iter().enumerate() {
        if newer_only.is_some_and(|id| id != newer.id) {
            continue;
        }
        for (i, older) in all.iter().enumerate() {
            if older.id == newer.id
                || older.space != newer.space
                || older.created_at >= newer.created_at
                || replaced.contains(&older.id)
                || asked.contains(&(older.id.clone(), newer.id.clone()))
            {
                continue;
            }
            let (share, shared) = crate::germinate::overlap(&words[i], &words[j]);
            let by_words = share >= 0.5 && shared >= 2;
            let by_meaning = match (vectors.get(&older.id), vectors.get(&newer.id)) {
                (Some(a), Some(b)) => closest(a, b) >= MEANING_CLOSE,
                _ => false,
            };
            if by_words || by_meaning {
                pairs.push(Pair {
                    older: older.id.clone(),
                    newer: newer.id.clone(),
                });
            }
        }
    }
    Ok(pairs)
}

#[derive(Deserialize)]
struct JevQuestions {
    version: String,
    model: String,
    same_subject: JevQuestion,
    replaces: JevQuestion,
}

#[derive(Deserialize, Serialize, Clone)]
struct JevQuestion {
    instructions: String,
    criteria: Value,
}

fn jev_questions() -> &'static JevQuestions {
    static QUESTIONS: std::sync::OnceLock<JevQuestions> = std::sync::OnceLock::new();
    QUESTIONS.get_or_init(|| serde_json::from_str(JEV_QUESTIONS).expect("valid Jev questions"))
}

/// Version of Jev's questions (`tidy.v1`).
pub fn jev_version() -> &'static str {
    &jev_questions().version
}

/// Jev's request for pairs of decisions: for each, same subject and does the newer replace the
/// older. Titles and texts are redacted before they leave.
fn jev_request(pairs: &[(&Decision, &Decision)]) -> String {
    let questions = jev_questions();
    let mut asked = serde_json::Map::new();
    for (i, (older, newer)) in pairs.iter().enumerate() {
        let decisions = json!({
            "older": crate::jev::state_of(&older.title, &older.content),
            "newer": crate::jev::state_of(&newer.title, &newer.content),
        });
        for (key, question) in [
            ("same", &questions.same_subject),
            ("replaces", &questions.replaces),
        ] {
            let mut instructions = decisions.clone();
            instructions["question"] = json!(question.instructions);
            asked.insert(
                format!("{key}_{i}"),
                json!({ "type": "noul", "instructions": instructions, "criteria": question.criteria }),
            );
        }
    }
    json!({
        "state": { "notes": "decisions from one person's notes, each pair older then newer" },
        "model": questions.model,
        "questions": asked,
    })
    .to_string()
}

/// Jev's answers per pair: same subject, replaces.
fn jev_answers(reply: &Value, count: usize) -> Vec<(Option<f64>, Option<f64>)> {
    let noul = |id: String| reply["answers"][id]["noul"].as_f64();
    (0..count)
        .map(|i| (noul(format!("same_{i}")), noul(format!("replaces_{i}"))))
        .collect()
}

/// A pair as judged, ready to store.
#[derive(Debug, Clone)]
pub struct Judged {
    pub pair: Pair,
    pub same: Option<f64>,
    pub replaces: Option<f64>,
}

/// Asks Jev about the pairs (`all`: the decisions they come from) (when the person lets it read their cases and a key is set); `None`
/// for each pair otherwise or when the call fails.
pub async fn judge(
    db_path: &Path,
    data_dir: &Path,
    all: &[Decision],
    pairs: Vec<Pair>,
) -> Vec<Judged> {
    let unjudged = |pairs: Vec<Pair>| {
        pairs
            .into_iter()
            .map(|pair| Judged {
                pair,
                same: None,
                replaces: None,
            })
            .collect::<Vec<_>>()
    };
    if pairs.is_empty() || !crate::natures::enabled(data_dir) {
        return unjudged(pairs);
    }
    let Some(key) = crate::natures::key() else {
        return unjudged(pairs);
    };
    let by_id: HashMap<&str, &Decision> = all.iter().map(|d| (d.id.as_str(), d)).collect();
    let pairs: Vec<Pair> = pairs.into_iter().take(PAIRS_PER_CHECK).collect();
    let both: Vec<(&Decision, &Decision)> = pairs
        .iter()
        .filter_map(|p| Some((*by_id.get(p.older.as_str())?, *by_id.get(p.newer.as_str())?)))
        .collect();
    if both.len() != pairs.len() {
        return unjudged(pairs);
    }
    let body = jev_request(&both);
    let Ok(client) = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
    else {
        return unjudged(pairs);
    };
    let started = Instant::now();
    let (reply, tokens) = crate::jev::ask(&client, &key, body).await;
    let model = reply
        .as_ref()
        .map(crate::jev::model_of)
        .unwrap_or_else(|_| jev_questions().model.clone());
    crate::ai_usage::record(
        db_path,
        &crate::ai_usage::UsageEntry {
            operation: crate::ai_usage::operation::CHECK_DECISIONS,
            provider: crate::jev::PROVIDER,
            model: &model,
            language: None,
            estimated_cost_usd: crate::ai_usage::token_cost(crate::jev::PROVIDER, &model, &tokens),
            tokens,
            audio_seconds: None,
            duration_ms: started.elapsed().as_millis() as u64,
            ok: reply.is_ok(),
            prices_as_of: crate::ai_usage::PRICES_AS_OF,
        },
    );
    let Ok(reply) = reply else {
        return unjudged(pairs);
    };
    let answers = jev_answers(&reply, pairs.len());
    pairs
        .into_iter()
        .zip(answers)
        .map(|(pair, (same, replaces))| Judged {
            pair,
            same,
            replaces,
        })
        .collect()
}

/// One refresh at a time: two would ask Jev about the same pairs.
static REFRESHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Held while a refresh runs; another one gets `None` from [`begin`] and waits for it.
pub struct Refreshing(());

impl Drop for Refreshing {
    fn drop(&mut self) {
        REFRESHING.store(false, std::sync::atomic::Ordering::Release);
    }
}

pub fn begin() -> Option<Refreshing> {
    // `then`, not `then_some`: a guard built for nothing would clear the flag when dropped.
    (!REFRESHING.swap(true, std::sync::atomic::Ordering::AcqRel)).then(|| Refreshing(()))
}

/// Whether a refresh runs now.
pub fn running() -> bool {
    REFRESHING.load(std::sync::atomic::Ordering::Acquire)
}

/// How long a look at every pair is skipped while nothing it reads changed.
const LOOK_AGAIN: Duration = Duration::from_secs(30 * 60);

/// The last look at every pair of a database: what it read, and when.
pub(crate) type LastLook = std::sync::Mutex<Option<(String, Instant)>>;

/// Whether nothing a look at every pair reads changed since the last one (cases, proposals,
/// replacements, settings, projects, and `extra` from the caller), for less than
/// [`LOOK_AGAIN`]. That look compares cases two by two under the database lock: a refresh
/// every two minutes then costs one small query. An in-memory database is always looked at.
pub(crate) fn nothing_changed(conn: &Connection, last: &LastLook, extra: &str) -> bool {
    if conn.path().is_none_or(str::is_empty) {
        return false;
    }
    let Ok(read) = conn.query_row(
        "SELECT (SELECT count(*) || ':' || COALESCE(max(updated_at), '') || ':'
                        || COALESCE(sum(revision), 0) FROM notes)
                || '|' || (SELECT count(*) || ':' || count(answer) || ':'
                                  || COALESCE(max(answered_at), '') FROM proposals)
                || '|' || (SELECT count(*) FROM case_supersessions)
                || '|' || (SELECT COALESCE(group_concat(key || '=' || value), '') FROM app_settings)
                || '|' || (SELECT COALESCE(group_concat(id || ':' || COALESCE(space, '') || ':'
                                                        || COALESCE(archived_at, '')), '')
                             FROM projects)",
        [],
        |r| r.get::<_, String>(0),
    ) else {
        return false;
    };
    let read = format!("{}|{read}|{extra}", conn.path().unwrap_or_default());
    let Ok(mut last) = last.lock() else {
        return false;
    };
    let same = last
        .as_ref()
        .is_some_and(|(seen, at)| *seen == read && at.elapsed() < LOOK_AGAIN);
    if !same {
        *last = Some((read, Instant::now()));
    }
    same
}

/// What a refresh asks Jev about.
pub struct Prepared {
    pub pairs: Vec<Pair>,
    pub decisions: Vec<Decision>,
}

/// First step of a refresh of "To tidy", under the database lock: the cases to archive (unless
/// `newer_only`), then the pairs of decisions to judge (only those where `newer_only` is the
/// newer one when given: the check when a decision is written). `None` when there is nothing to
/// judge, or Jev may not read. Jev is asked by [`judge`] without the lock, then [`store_pairs`] keeps the answers.
pub fn prepare(
    conn: &Connection,
    data_dir: &Path,
    newer_only: Option<&str>,
) -> Result<Option<Prepared>, String> {
    if newer_only.is_none() {
        refresh_archive(conn)?;
    }
    // Only Jev tells a replacement from an addition: without it, no pair is looked for.
    if !settings(conn)?.supersede
        || !crate::natures::enabled(data_dir)
        || crate::natures::key().is_none()
    {
        return Ok(None);
    }
    static LAST: LastLook = std::sync::Mutex::new(None);
    let extra = crate::meaning::has_vectors(data_dir).to_string();
    if newer_only.is_none() && nothing_changed(conn, &LAST, &extra) {
        return Ok(None);
    }
    let pairs = candidate_pairs(conn, Some(data_dir), newer_only)?;
    if pairs.is_empty() {
        return Ok(None);
    }
    Ok(Some(Prepared {
        pairs,
        decisions: decisions(conn)?,
    }))
}

/// Stores judged pairs: proposed when Jev reads a replacement (from [`REPLACES_DOUBT`], "to
/// check" below [`REPLACES_MIN`]); `dismissed` when Jev reads no replacement, so the pair is not
/// judged again. A pair nobody judged is not stored: it is judged once Jev can read. Returns the
/// count of proposals added.
pub fn store_pairs(conn: &Connection, judged: &[Judged]) -> Result<usize, String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let now: String = tx
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |r| {
            r.get(0)
        })
        .map_err(crate::db::db_error)?;
    let mut added = 0;
    for (position, item) in judged.iter().enumerate() {
        let answer = match item.replaces {
            Some(r) if r >= REPLACES_DOUBT => None,
            Some(_) => Some("dismissed"),
            None => continue,
        };
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM proposals WHERE kind = 'supersede'
                                   AND note_id = ?1 AND near_id = ?2)",
                params![item.pair.older, item.pair.newer],
                |r| r.get(0),
            )
            .map_err(crate::db::db_error)?;
        if exists {
            // Jev judges a pair kept unchecked: shown or dismissed from its answer.
            if let Some(replaces) = item.replaces {
                tx.execute(
                    "UPDATE proposals SET follows = ?3, covers = ?4, answer = ?5,
                            answered_at = CASE WHEN ?5 IS NULL THEN NULL ELSE ?6 END
                      WHERE kind = 'supersede' AND note_id = ?1 AND near_id = ?2
                        AND answer IS NULL AND follows IS NULL AND proposed_by IS NULL",
                    params![
                        item.pair.older,
                        item.pair.newer,
                        replaces,
                        item.same,
                        answer,
                        now
                    ],
                )
                .map_err(crate::db::db_error)?;
            }
            continue;
        }
        let inserted = tx
            .execute(
                "INSERT INTO proposals (id, note_id, kind, nature, title, passage, near_id, follows,
                                        covers, revision, position, created_at, answer, answered_at)
                 SELECT ?1, n.id, 'supersede', n.note_type, n.title, '', ?3, ?4, ?5, n.revision, ?6,
                        ?7, ?8, CASE WHEN ?8 IS NULL THEN NULL ELSE ?7 END
                   FROM notes n WHERE n.id = ?2",
                params![
                    Uuid::new_v4().to_string(),
                    item.pair.older,
                    item.pair.newer,
                    item.replaces,
                    item.same,
                    position as i64,
                    now,
                    answer
                ],
            )
            .map_err(crate::db::db_error)?;
        if answer.is_none() {
            added += inserted;
        }
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(added)
}

/// An agent proposes that a newer decision replaces an older one (MCP `propose_superseded`).
/// Both must be live decisions of the same space, the older one written first and not replaced
/// yet; only the person answers.
pub fn propose_from_agent(
    conn: &Connection,
    older: &str,
    newer: &str,
) -> Result<Option<TidyItem>, String> {
    let all = decisions(conn)?;
    let find = |id: &str| all.iter().find(|d| d.id == id).cloned();
    let (Some(o), Some(n)) = (find(older), find(newer)) else {
        return Err("both cases must be live decisions".into());
    };
    if o.id == n.id || o.space != n.space {
        return Err("the two decisions must be different and in the same space".into());
    }
    if o.created_at >= n.created_at {
        return Err("the replaced decision must be the older one".into());
    }
    let replaced: bool = conn
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM case_supersessions WHERE note_id = ?1)",
            [older],
            |r| r.get(0),
        )
        .map_err(crate::db::db_error)?;
    if replaced {
        return Err("the older decision is already marked as replaced".into());
    }
    let agent = crate::db::journal::current_agent_name(conn).unwrap_or_else(|| "agent".to_string());
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let waiting: Option<String> = tx
        .query_row(
            "SELECT id FROM proposals WHERE kind = 'supersede' AND note_id = ?1 AND near_id = ?2
                AND answer IS NULL",
            params![older, newer],
            |r| r.get(0),
        )
        .optional()
        .map_err(crate::db::db_error)?;
    if waiting.is_none() {
        tx.execute(
            "INSERT INTO proposals (id, note_id, kind, nature, title, passage, proposed_by, near_id,
                                    revision, position, created_at)
             SELECT ?1, n.id, 'supersede', n.note_type, n.title, '', ?3, ?4, n.revision, 0,
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
               FROM notes n WHERE n.id = ?2",
            params![Uuid::new_v4().to_string(), older, agent, newer],
        )
        .map_err(crate::db::db_error)?;
        // The interface follows agents' writes through the revision of the case.
        queries::bump_revision(&tx, newer)?;
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(pending(conn, o.space.as_deref())?.into_iter().find(|item| {
        item.kind == TidyKind::Supersede
            && item.case.id == older
            && item.other.as_ref().is_some_and(|n| n.id == newer)
    }))
}

// --- Answers --------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct TidyAnswer {
    pub id: String,
    pub accept: bool,
    /// Group: file `other` inside `case` instead.
    #[serde(default)]
    pub swap: bool,
}

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct TidyOutcome {
    pub archived: usize,
    pub superseded: usize,
    /// Cases filed inside another.
    pub grouped: usize,
    /// Cases the person will correct (they go against a decision).
    pub to_fix: usize,
    pub refused: usize,
    /// Accepted proposals that no longer held when applied (nothing changed).
    pub skipped: usize,
}

/// Marks `older` as replaced by `newer` in an open transaction (the older case's revision
/// advances: agents read it as history from now on).
fn mark_replaced(conn: &Connection, older: &str, newer: &str, now: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO case_supersessions (note_id, by_id, superseded_at) VALUES (?1, ?2, ?3)",
        params![older, newer, now],
    )
    .map_err(crate::db::db_error)?;
    queries::bump_revision(conn, older)?;
    Ok(())
}

fn set_archived(conn: &Connection, id: &str, archived: bool) -> Result<(), String> {
    let sql = if archived {
        "UPDATE notes SET archived_at = COALESCE(archived_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
          WHERE id = ?1 AND deleted_at IS NULL"
    } else {
        "UPDATE notes SET archived_at = NULL WHERE id = ?1"
    };
    if conn.execute(sql, [id]).map_err(crate::db::db_error)? > 0 {
        queries::bump_revision(conn, id)?;
    }
    Ok(())
}

/// The condition (on `proposals`, aliased `stale`) under which a waiting proposal no longer
/// holds: its cases moved, closed or went, a decision is already replaced. Shared by
/// [`pending`] (not shown) and [`answer`] (not applied).
const STALE: &str = "(
    (stale.kind = 'archive' AND EXISTS (
        SELECT 1 FROM notes a WHERE a.id = stale.note_id
           AND (a.status <> 'resolved' OR a.archived_at IS NOT NULL OR a.deleted_at IS NOT NULL
                OR a.parent_id IS NOT NULL
                OR (a.note_type = 'decision'
                    AND NOT EXISTS (SELECT 1 FROM case_supersessions s
                                     JOIN notes b ON b.id = s.by_id AND b.deleted_at IS NULL
                                    WHERE s.note_id = a.id))
                OR EXISTS (WITH RECURSIVE below(id) AS (
                               SELECT id FROM notes WHERE parent_id = a.id
                               UNION ALL
                               SELECT n.id FROM notes n JOIN below ON n.parent_id = below.id)
                           SELECT 1 FROM below JOIN notes c ON c.id = below.id
                            WHERE c.deleted_at IS NULL AND c.status <> 'resolved'))))
    OR (stale.kind = 'supersede' AND (
        stale.near_id IS NULL
        OR EXISTS (SELECT 1 FROM case_supersessions s WHERE s.note_id = stale.note_id)
        OR NOT EXISTS (SELECT 1 FROM notes b WHERE b.id = stale.near_id AND b.deleted_at IS NULL)))
    OR (stale.kind = 'group' AND (
        stale.near_id IS NULL
        OR NOT EXISTS (SELECT 1 FROM notes a, notes b
                        WHERE a.id = stale.note_id AND b.id = stale.near_id
                          AND a.parent_id IS NULL AND b.parent_id IS NULL
                          AND a.project_id = b.project_id
                          AND a.archived_at IS NULL AND b.archived_at IS NULL
                          AND a.deleted_at IS NULL AND b.deleted_at IS NULL)))
    OR (stale.kind = 'conflict' AND (
        stale.near_id IS NULL
        OR EXISTS (SELECT 1 FROM case_supersessions s WHERE s.note_id = stale.near_id)
        OR NOT EXISTS (SELECT 1 FROM notes a, notes d
                        WHERE a.id = stale.note_id AND d.id = stale.near_id
                          AND a.status <> 'resolved' AND a.archived_at IS NULL
                          AND a.deleted_at IS NULL AND d.archived_at IS NULL
                          AND d.deleted_at IS NULL)))
)";

/// Whether a waiting proposal no longer holds ([`STALE`]).
fn stale(conn: &Connection, id: &str) -> Result<bool, String> {
    conn.query_row(
        &format!("SELECT EXISTS (SELECT 1 FROM proposals stale WHERE stale.id = ?1 AND {STALE})"),
        [id],
        |r| r.get(0),
    )
    .map_err(crate::db::db_error)
}

/// Applies the person's answers in one transaction. An accepted proposal that no longer holds
/// is left waiting instead (`skipped`): a decision already replaced by another, a case to archive
/// that has open work again (filed under it by the same batch), a case to file that went.
pub fn answer(conn: &Connection, answers: &[TidyAnswer]) -> Result<TidyOutcome, String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let now: String = tx
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |r| {
            r.get(0)
        })
        .map_err(crate::db::db_error)?;
    let mut outcome = TidyOutcome::default();
    for input in answers {
        let row: Option<(String, String, Option<String>)> = tx
            .query_row(
                "SELECT kind, note_id, near_id FROM proposals
                  WHERE id = ?1 AND answer IS NULL
                    AND kind IN ('archive', 'supersede', 'group', 'conflict')",
                [&input.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(crate::db::db_error)?;
        let Some((kind, note_id, near_id)) = row else {
            return Err(crate::tr!("tidy.alreadyAnswered"));
        };
        let mut result = Some(note_id.clone());
        if input.accept && stale(&tx, &input.id)? {
            // Left waiting, so it is not shown while it does not hold, and comes back if what
            // made it stale is undone (the other decision's answer taken back).
            outcome.skipped += 1;
            continue;
        }
        let answer = if !input.accept {
            outcome.refused += 1;
            "refused"
        } else if kind == "archive" {
            set_archived(&tx, &note_id, true)?;
            outcome.archived += 1;
            "accepted"
        } else if kind == "group" {
            let near = near_id.ok_or_else(|| crate::tr!("tidy.otherGone"))?;
            let (child, parent) = if input.swap {
                (near, note_id.clone())
            } else {
                (note_id.clone(), near)
            };
            crate::db::cases::move_case_in(&tx, &child, Some(&parent))?;
            outcome.grouped += 1;
            // The case moved: undo takes it out again.
            result = Some(child);
            "accepted"
        } else if kind == "conflict" {
            outcome.to_fix += 1;
            "accepted"
        } else {
            let newer = near_id.ok_or_else(|| crate::tr!("tidy.otherGone"))?;
            mark_replaced(&tx, &note_id, &newer, &now)?;
            outcome.superseded += 1;
            "accepted"
        };
        tx.execute(
            "UPDATE proposals SET answer = ?2, answered_at = ?3, result_id = ?4 WHERE id = ?1",
            params![
                input.id,
                answer,
                now,
                result.filter(|_| answer == "accepted")
            ],
        )
        .map_err(crate::db::db_error)?;
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(outcome)
}

/// Takes answers back (Undo right after): an archived case comes back, a decision marked as
/// replaced is in force again, and the proposals wait again.
pub fn undo(conn: &Connection, ids: &[String]) -> Result<(), String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    for id in ids {
        type Row = (String, String, Option<String>, String, Option<String>);
        let row: Option<Row> = tx
            .query_row(
                "SELECT kind, note_id, near_id, answer, result_id FROM proposals
                  WHERE id = ?1 AND kind IN ('archive', 'supersede', 'group', 'conflict')
                    AND answer IN ('accepted', 'refused')",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()
            .map_err(crate::db::db_error)?;
        let Some((kind, note_id, near_id, answer, moved)) = row else {
            continue;
        };
        if answer == "accepted" {
            if kind == "group" {
                // The case filed comes out again, if it is still where it was filed.
                if let (Some(moved), Some(near)) = (moved, near_id) {
                    let parent = if moved == note_id {
                        near
                    } else {
                        note_id.clone()
                    };
                    let still: bool = tx
                        .query_row(
                            "SELECT EXISTS (SELECT 1 FROM notes WHERE id = ?1 AND parent_id = ?2)",
                            params![moved, parent],
                            |r| r.get(0),
                        )
                        .map_err(crate::db::db_error)?;
                    if still {
                        crate::db::cases::move_case_in(&tx, &moved, None)?;
                    }
                }
            } else if kind == "archive" {
                set_archived(&tx, &note_id, false)?;
            } else if kind == "supersede"
                && tx
                    .execute(
                        "DELETE FROM case_supersessions WHERE note_id = ?1 AND by_id IS ?2",
                        params![note_id, near_id],
                    )
                    .map_err(crate::db::db_error)?
                    > 0
            {
                queries::bump_revision(&tx, &note_id)?;
            }
        }
        tx.execute(
            "UPDATE proposals SET answer = NULL, answered_at = NULL, result_id = NULL WHERE id = ?1",
            [id],
        )
        .map_err(crate::db::db_error)?;
    }
    tx.commit().map_err(crate::db::db_error)
}

// --- A case's place in the history ----------------------------------------------------------

/// For a decision: the newer one that replaced it, and the older ones it replaced.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Supersession {
    pub superseded_by: Option<CaseRef>,
    pub superseded_at: Option<String>,
    pub replaces: Vec<CaseRef>,
    /// Proposals waiting where this decision is the newer one ("Does it replace…?").
    pub asking: Vec<TidyItem>,
}

pub fn supersession(conn: &Connection, note_id: &str) -> Result<Supersession, String> {
    let by: Option<(String, String)> = conn
        .query_row(
            "SELECT s.by_id, s.superseded_at FROM case_supersessions s
               JOIN notes b ON b.id = s.by_id AND b.deleted_at IS NULL
              WHERE s.note_id = ?1",
            [note_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(crate::db::db_error)?;
    let (superseded_by, superseded_at) = match by {
        Some((id, at)) => (case_ref(conn, &id, "created_at")?, Some(at)),
        None => (None, None),
    };
    let older: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT note_id FROM case_supersessions WHERE by_id = ?1 ORDER BY superseded_at",
            )
            .map_err(crate::db::db_error)?;
        let rows = stmt
            .query_map([note_id], |r| r.get(0))
            .map_err(crate::db::db_error)?
            .collect::<Result<_, _>>()
            .map_err(crate::db::db_error)?;
        rows
    };
    let mut replaces = Vec::new();
    for id in older {
        if let Some(case) = case_ref(conn, &id, "created_at")? {
            replaces.push(case);
        }
    }
    let asking = pending(conn, None)?
        .into_iter()
        .filter(|item| {
            item.kind == TidyKind::Supersede && item.other.as_ref().is_some_and(|n| n.id == note_id)
        })
        .collect();
    Ok(Supersession {
        superseded_by,
        superseded_at,
        replaces,
        asking,
    })
}

/// The person says a decision marked as replaced is still in force: the mark goes, and the
/// proposal that led to it (if any) counts as refused so it is not asked again.
pub fn keep_in_force(conn: &Connection, note_id: &str) -> Result<(), String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let by: Option<String> = tx
        .query_row(
            "SELECT by_id FROM case_supersessions WHERE note_id = ?1",
            [note_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(crate::db::db_error)?;
    if let Some(by) = by {
        tx.execute(
            "DELETE FROM case_supersessions WHERE note_id = ?1",
            [note_id],
        )
        .map_err(crate::db::db_error)?;
        tx.execute(
            "UPDATE proposals SET answer = 'refused',
                    answered_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), result_id = NULL
              WHERE kind = 'supersede' AND note_id = ?1 AND near_id = ?2",
            params![note_id, by],
        )
        .map_err(crate::db::db_error)?;
        queries::bump_revision(&tx, note_id)?;
    }
    tx.commit().map_err(crate::db::db_error)
}

/// Ids of the decisions marked as replaced (by a live decision), for lists and search.
pub fn superseded_ids(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT s.note_id FROM case_supersessions s
               JOIN notes b ON b.id = s.by_id AND b.deleted_at IS NULL",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([], |r| r.get(0))
        .map_err(crate::db::db_error)?
        .collect::<Result<_, _>>()
        .map_err(crate::db::db_error)?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::note::CreateNoteInput;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::schema::apply_migrations(&conn).unwrap();
        conn
    }

    fn project(conn: &Connection, name: &str) -> String {
        queries::create_project(conn, name, None).unwrap()
    }

    fn case(conn: &Connection, project: &str, title: &str, nature: NoteType, text: &str) -> String {
        queries::create_note(
            conn,
            CreateNoteInput {
                project_id: project.to_string(),
                title: title.to_string(),
                content: Some(text.to_string()),
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

    /// Resolved and untouched for `days`.
    fn resolved_since(conn: &Connection, id: &str, days: i64) {
        conn.execute(
            "UPDATE notes SET status = ?2,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?3) WHERE id = ?1",
            params![id, "resolved", format!("-{days} days")],
        )
        .unwrap();
    }

    fn written(conn: &Connection, id: &str, at: &str) {
        conn.execute(
            "UPDATE notes SET created_at = ?2 WHERE id = ?1",
            params![id, at],
        )
        .unwrap();
    }

    #[test]
    fn archiving_is_proposed_for_old_resolved_top_level_cases_only() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let old = case(&conn, &p, "Export bug", NoteType::Bug, "Fixed.");
        let recent = case(&conn, &p, "Recent fix", NoteType::Bug, "Fixed.");
        let open = case(&conn, &p, "Still open", NoteType::Todo, "To do.");
        let in_force = case(
            &conn,
            &p,
            "Use SQLite",
            NoteType::Decision,
            "We use SQLite.",
        );
        let parent = case(&conn, &p, "Box", NoteType::Note, "A box.");
        let child = case(&conn, &p, "Open child", NoteType::Todo, "Open.");
        conn.execute(
            "UPDATE notes SET parent_id = ?2 WHERE id = ?1",
            params![child, parent],
        )
        .unwrap();
        resolved_since(&conn, &old, 40);
        resolved_since(&conn, &recent, 3);
        resolved_since(&conn, &in_force, 40);
        resolved_since(&conn, &parent, 40);
        let _ = open;

        assert_eq!(refresh_archive(&conn).unwrap(), 1);
        // The test projects have no space: every space is listed.
        let space: Option<&str> = None;
        let items = pending(&conn, space).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].case.id, old);
        // The Sprout panel of the case shows sub-cases only.
        assert!(crate::germinate::pending(&conn, &old).unwrap().is_empty());
        // Asked again, nothing doubles.
        assert_eq!(refresh_archive(&conn).unwrap(), 0);

        // Refused: never proposed again. Reopened: the waiting proposal is withdrawn.
        answer(
            &conn,
            &[TidyAnswer {
                id: items[0].id.clone(),
                accept: false,
                swap: false,
            }],
        )
        .unwrap();
        assert_eq!(refresh_archive(&conn).unwrap(), 0);
        let other = case(&conn, &p, "Other bug", NoteType::Bug, "Fixed.");
        resolved_since(&conn, &other, 40);
        assert_eq!(refresh_archive(&conn).unwrap(), 1);
        conn.execute("UPDATE notes SET status = 'open' WHERE id = ?1", [&other])
            .unwrap();
        refresh_archive(&conn).unwrap();
        assert!(pending(&conn, space).unwrap().is_empty());

        // "Never": nothing is proposed.
        set_settings(
            &conn,
            &TidySettings {
                archive_weeks: 0,
                supersede: true,
                group: true,
            },
        )
        .unwrap();
        resolved_since(&conn, &other, 40);
        assert_eq!(refresh_archive(&conn).unwrap(), 0);
        assert!(set_settings(
            &conn,
            &TidySettings {
                archive_weeks: 3,
                supersede: true,
                group: true,
            }
        )
        .is_err());
    }

    /// No more than ARCHIVE_LIMIT archive proposals wait at once: a second refresh adds none.
    #[test]
    fn archive_proposals_do_not_pile_up_from_one_refresh_to_the_next() {
        let conn = db();
        let p = project(&conn, "Humcase");
        for i in 0..ARCHIVE_LIMIT + 5 {
            let id = case(&conn, &p, &format!("Done {i}"), NoteType::Todo, "Done.");
            resolved_since(&conn, &id, 40);
        }
        assert_eq!(refresh_archive(&conn).unwrap(), ARCHIVE_LIMIT);
        assert_eq!(refresh_archive(&conn).unwrap(), 0);
        assert_eq!(pending(&conn, None).unwrap().len(), ARCHIVE_LIMIT);
    }

    /// An agent's result the person has not accepted is never proposed for archiving; once
    /// accepted, it is.
    #[test]
    fn a_result_waiting_for_the_persons_decision_is_not_proposed_for_archiving() {
        use crate::db::journal::{self, Actor};
        let conn = db();
        journal::install(&conn, &Actor::Agent(Some("Codex".into()))).unwrap();
        let p = project(&conn, "Humcase");
        let result = case(&conn, &p, "Case study", NoteType::Todo, "Draft.");
        queries::update_note(
            &conn,
            &result,
            crate::models::note::UpdateNoteInput {
                status: Some(crate::models::note::NoteStatus::Resolved),
                content: None,
                resolution_comment: Some("Written, to check with Mathilde.".into()),
                title: None,
                note_type: None,
                raw_content: None,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
            },
        )
        .unwrap();
        resolved_since(&conn, &result, 40);
        assert_eq!(
            journal::review_state(&conn, &result).unwrap().state,
            "to_review"
        );
        assert_eq!(refresh_archive(&conn).unwrap(), 0);

        journal::set_actor(&conn, &Actor::Human).unwrap();
        let revision: i64 = conn
            .query_row("SELECT revision FROM notes WHERE id = ?1", [&result], |r| {
                r.get(0)
            })
            .unwrap();
        journal::review_case(&conn, &result, journal::Decision::Accept, revision, None).unwrap();
        resolved_since(&conn, &result, 40);
        assert_eq!(refresh_archive(&conn).unwrap(), 1);
    }

    #[test]
    fn accepting_archives_and_undo_brings_back() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let old = case(&conn, &p, "Export bug", NoteType::Bug, "Fixed.");
        resolved_since(&conn, &old, 40);
        refresh_archive(&conn).unwrap();
        // The test projects have no space: every space is listed.
        let space: Option<&str> = None;
        let item = pending(&conn, space).unwrap().remove(0);
        let before = queries::get_note_by_id(&conn, &old).unwrap();
        let outcome = answer(
            &conn,
            &[TidyAnswer {
                id: item.id.clone(),
                accept: true,
                swap: false,
            }],
        )
        .unwrap();
        assert_eq!(outcome.archived, 1);
        let archived = queries::get_note_by_id(&conn, &old).unwrap();
        assert!(archived.archived_at.is_some());
        assert!(archived.revision > before.revision);
        assert_eq!(
            archived.updated_at, before.updated_at,
            "not brought back up"
        );
        // Answered twice: refused.
        assert!(answer(
            &conn,
            &[TidyAnswer {
                id: item.id.clone(),
                accept: true,
                swap: false,
            }]
        )
        .is_err());
        undo(&conn, std::slice::from_ref(&item.id)).unwrap();
        assert!(queries::get_note_by_id(&conn, &old)
            .unwrap()
            .archived_at
            .is_none());
        assert_eq!(pending(&conn, space).unwrap().len(), 1);
        // Taken back by hand after accepting: not proposed again.
        answer(
            &conn,
            &[TidyAnswer {
                id: item.id,
                accept: true,
                swap: false,
            }],
        )
        .unwrap();
        queries::set_note_archived(&conn, &old, false).unwrap();
        assert_eq!(refresh_archive(&conn).unwrap(), 0);
    }

    #[test]
    fn decisions_close_by_words_are_paired_older_first_once() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let older = case(
            &conn,
            &p,
            "Héberger Humcase en ligne sur Supabase",
            NoteType::Decision,
            "On part sur Supabase.",
        );
        let newer = case(
            &conn,
            &p,
            "Abandonner Supabase : Cloudflare pour la version en ligne",
            NoteType::Decision,
            "On abandonne Supabase.",
        );
        let other = case(
            &conn,
            &p,
            "Licence AGPL pour le dépôt",
            NoteType::Decision,
            "AGPL-3.0.",
        );
        written(&conn, &older, "2026-09-28T10:00:00.000Z");
        written(&conn, &newer, "2026-09-30T10:00:00.000Z");
        written(&conn, &other, "2026-09-29T10:00:00.000Z");
        let pairs = candidate_pairs(&conn, None, None).unwrap();
        assert_eq!(
            pairs,
            vec![Pair {
                older: older.clone(),
                newer: newer.clone(),
            }]
        );
        assert!(candidate_pairs(&conn, None, Some(&older))
            .unwrap()
            .is_empty());

        // Nobody judged the pair: nothing stored, judged once Jev may read.
        let unjudged = Judged {
            pair: pairs[0].clone(),
            same: None,
            replaces: None,
        };
        assert_eq!(
            store_pairs(&conn, std::slice::from_ref(&unjudged)).unwrap(),
            0
        );
        assert_eq!(candidate_pairs(&conn, None, None).unwrap().len(), 1);
        // Jev reads no replacement: dismissed, never asked again.
        let no = Judged {
            replaces: Some(0.1),
            same: Some(0.9),
            ..unjudged.clone()
        };
        assert_eq!(store_pairs(&conn, &[no]).unwrap(), 0);
        assert!(candidate_pairs(&conn, None, None).unwrap().is_empty());
    }

    #[test]
    fn one_refresh_at_a_time() {
        let first = begin().expect("no refresh runs yet");
        assert!(begin().is_none());
        // The refused attempt leaves the running refresh marked.
        assert!(running());
        drop(first);
        assert!(!running());
        drop(begin().expect("free again"));
    }

    #[test]
    fn without_jev_no_pair_is_proposed_and_an_old_unchecked_one_waits_for_it() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let older = case(
            &conn,
            &p,
            "Licence AGPL pour le dépôt public",
            NoteType::Decision,
            "AGPL-3.0.",
        );
        let newer = case(
            &conn,
            &p,
            "Licence AGPL pour le dépôt public, contributions sous CLA",
            NoteType::Decision,
            "En plus de l'AGPL, un CLA.",
        );
        written(&conn, &older, "2026-09-28T10:00:00.000Z");
        written(&conn, &newer, "2026-09-30T10:00:00.000Z");
        let pairs = candidate_pairs(&conn, None, None).unwrap();
        assert_eq!(pairs.len(), 1);
        // Nearly every word shared, but nobody could judge: nothing proposed.
        let unjudged = Judged {
            pair: pairs[0].clone(),
            same: None,
            replaces: None,
        };
        assert_eq!(
            store_pairs(&conn, std::slice::from_ref(&unjudged)).unwrap(),
            0
        );
        assert!(pending(&conn, None).unwrap().is_empty());
        // A pair stored unchecked by the first version is not shown, and is judged again.
        conn.execute(
            "INSERT INTO proposals (id, note_id, kind, nature, title, passage, near_id, revision,
                                    position, created_at)
             VALUES ('old', ?1, 'supersede', 'decision', '', '', ?2, 0, 0, '2026-10-01T10:00:00Z')",
            params![older, newer],
        )
        .unwrap();
        assert!(pending(&conn, None).unwrap().is_empty());
        assert_eq!(candidate_pairs(&conn, None, None).unwrap(), pairs);
        let replaces = Judged {
            replaces: Some(0.9),
            same: Some(0.9),
            ..unjudged
        };
        store_pairs(&conn, &[replaces]).unwrap();
        let shown = pending(&conn, None).unwrap();
        assert_eq!(shown.len(), 1);
        assert!(shown[0].checked && !shown[0].doubt);
        assert!(candidate_pairs(&conn, None, None).unwrap().is_empty());
    }

    #[test]
    fn a_replacement_is_proposed_marked_and_kept_readable() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let older = case(
            &conn,
            &p,
            "Héberger sur Supabase",
            NoteType::Decision,
            "Supabase.",
        );
        let newer = case(
            &conn,
            &p,
            "Abandonner Supabase",
            NoteType::Decision,
            "Cloudflare.",
        );
        written(&conn, &older, "2026-09-28T10:00:00.000Z");
        written(&conn, &newer, "2026-09-30T10:00:00.000Z");
        let pair = Pair {
            older: older.clone(),
            newer: newer.clone(),
        };
        let doubt = Judged {
            pair: pair.clone(),
            same: Some(0.9),
            replaces: Some(0.55),
        };
        assert_eq!(store_pairs(&conn, &[doubt]).unwrap(), 1);
        // The test projects have no space: every space is listed.
        let space: Option<&str> = None;
        let items = pending(&conn, space).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].doubt && items[0].checked);
        assert_eq!(items[0].other.as_ref().unwrap().id, newer);
        // The newer decision asks.
        assert_eq!(supersession(&conn, &newer).unwrap().asking.len(), 1);

        let before = queries::get_note_by_id(&conn, &older).unwrap().revision;
        answer(
            &conn,
            &[TidyAnswer {
                id: items[0].id.clone(),
                accept: true,
                swap: false,
            }],
        )
        .unwrap();
        let history = supersession(&conn, &older).unwrap();
        assert_eq!(history.superseded_by.unwrap().id, newer);
        assert!(queries::get_note_by_id(&conn, &older).unwrap().revision > before);
        assert_eq!(supersession(&conn, &newer).unwrap().replaces[0].id, older);
        assert_eq!(superseded_ids(&conn).unwrap(), vec![older.clone()]);
        // A replaced decision may now be archived, once resolved long ago.
        resolved_since(&conn, &older, 40);
        assert_eq!(refresh_archive(&conn).unwrap(), 1);

        // "Still in force": the mark goes and the pair is not asked again.
        keep_in_force(&conn, &older).unwrap();
        assert!(supersession(&conn, &older).unwrap().superseded_by.is_none());
        assert!(candidate_pairs(&conn, None, None).unwrap().is_empty());
    }

    #[test]
    fn undo_takes_a_replacement_back() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let older = case(
            &conn,
            &p,
            "Héberger sur Supabase",
            NoteType::Decision,
            "Supabase.",
        );
        let newer = case(
            &conn,
            &p,
            "Abandonner Supabase",
            NoteType::Decision,
            "Cloudflare.",
        );
        written(&conn, &older, "2026-09-28T10:00:00.000Z");
        written(&conn, &newer, "2026-09-30T10:00:00.000Z");
        let item = propose_from_agent(&conn, &older, &newer).unwrap().unwrap();
        assert_eq!(item.proposed_by.as_deref(), Some("agent"));
        assert!(item.doubt && !item.checked);
        assert!(propose_from_agent(&conn, &newer, &older).is_err());
        answer(
            &conn,
            &[TidyAnswer {
                id: item.id.clone(),
                accept: true,
                swap: false,
            }],
        )
        .unwrap();
        assert!(propose_from_agent(&conn, &older, &newer).is_err());
        undo(&conn, &[item.id]).unwrap();
        assert!(superseded_ids(&conn).unwrap().is_empty());
        assert!(supersession(&conn, &older).unwrap().superseded_by.is_none());
    }

    /// A waiting proposal written as the stores write it.
    fn waiting(conn: &Connection, kind: &str, note: &str, near: Option<&str>) -> String {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO proposals (id, note_id, kind, nature, title, passage, near_id, follows,
                                    revision, position, created_at)
             SELECT ?1, n.id, ?2, n.note_type, n.title, '', ?3, 0.9, n.revision, 0,
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
               FROM notes n WHERE n.id = ?4",
            params![id, kind, near, note],
        )
        .unwrap();
        id
    }

    fn yes(id: &str) -> TidyAnswer {
        TidyAnswer {
            id: id.to_string(),
            accept: true,
            swap: false,
        }
    }

    /// Two newer decisions proposed for the same older one, both accepted: the first holds, the
    /// second never overwrites it (Undo would lose the first), and comes back once the first is
    /// taken back. Fails if accepting overwrites a replacement.
    #[test]
    fn a_decision_already_replaced_is_never_replaced_again() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let older = case(&conn, &p, "Supabase", NoteType::Decision, "Supabase.");
        let b = case(&conn, &p, "Cloudflare", NoteType::Decision, "Cloudflare.");
        let c = case(&conn, &p, "Fly", NoteType::Decision, "Fly.");
        let by_b = waiting(&conn, "supersede", &older, Some(&b));
        let by_c = waiting(&conn, "supersede", &older, Some(&c));
        let outcome = answer(&conn, &[yes(&by_b), yes(&by_c)]).unwrap();
        assert_eq!((outcome.superseded, outcome.skipped), (1, 1));
        let by: String = conn
            .query_row(
                "SELECT by_id FROM case_supersessions WHERE note_id = ?1",
                [&older],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(by, b);
        assert!(pending(&conn, None).unwrap().is_empty());
        undo(&conn, &[by_b.clone(), by_c.clone()]).unwrap();
        assert!(superseded_ids(&conn).unwrap().is_empty());
        assert_eq!(pending(&conn, None).unwrap().len(), 2);
    }

    /// Filing an open case under a case to archive, in the same batch, in either order: the
    /// archive is not applied over open work, nor a case filed under an archived one. Fails if
    /// `answer` stops re-checking a proposal when it applies it.
    #[test]
    fn filing_and_archiving_in_one_batch_never_hide_open_work() {
        for archive_first in [false, true] {
            let conn = db();
            let p = project(&conn, "Humcase");
            let done = case(&conn, &p, "Export", NoteType::Bug, "Fixed.");
            resolved_since(&conn, &done, 60);
            let open = case(&conn, &p, "Export again", NoteType::Todo, "To do.");
            let archive = waiting(&conn, "archive", &done, None);
            let group = waiting(&conn, "group", &open, Some(&done));
            let batch = if archive_first {
                [yes(&archive), yes(&group)]
            } else {
                [yes(&group), yes(&archive)]
            };
            let outcome = answer(&conn, &batch).unwrap();
            assert_eq!(outcome.skipped, 1, "archive first: {archive_first}");
            let parent = queries::get_note_by_id(&conn, &done).unwrap();
            let child = queries::get_note_by_id(&conn, &open).unwrap();
            let hidden = parent.archived_at.is_some() && child.parent_id.as_deref() == Some(&done);
            assert!(!hidden, "archive first: {archive_first}");
        }
    }

    /// A proposal that no longer holds is not shown, before any refresh withdraws it.
    #[test]
    fn a_proposal_that_no_longer_holds_is_not_shown() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let done = case(&conn, &p, "Export", NoteType::Bug, "Fixed.");
        resolved_since(&conn, &done, 60);
        let other = case(&conn, &p, "Import", NoteType::Todo, "To do.");
        waiting(&conn, "archive", &done, None);
        waiting(&conn, "group", &other, Some(&done));
        assert_eq!(pending(&conn, None).unwrap().len(), 2);
        conn.execute("UPDATE notes SET status = 'open' WHERE id = ?1", [&done])
            .unwrap();
        conn.execute(
            "UPDATE notes SET archived_at = '2026-10-01T00:00:00.000Z' WHERE id = ?1",
            [&other],
        )
        .unwrap();
        assert!(pending(&conn, None).unwrap().is_empty());
    }

    /// The look at every pair runs again as soon as a case, a proposal or a setting changes,
    /// and only then.
    #[test]
    fn the_look_at_every_pair_waits_for_a_change() {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open(dir.path().join("humcase.db")).unwrap();
        crate::db::schema::apply_migrations(&conn).unwrap();
        let p = project(&conn, "Humcase");
        let id = case(&conn, &p, "Export", NoteType::Bug, "Text.");
        let last: LastLook = std::sync::Mutex::new(None);
        assert!(!nothing_changed(&conn, &last, ""));
        assert!(nothing_changed(&conn, &last, ""));
        assert!(
            !nothing_changed(&conn, &last, "vectors"),
            "the caller's part"
        );
        queries::bump_revision(&conn, &id).unwrap();
        assert!(!nothing_changed(&conn, &last, "vectors"), "a case changed");
        permissions::set_setting(&conn, ARCHIVE_WEEKS, "8").unwrap();
        assert!(
            !nothing_changed(&conn, &last, "vectors"),
            "a setting changed"
        );
        assert!(nothing_changed(&conn, &last, "vectors"));
        let memory = db();
        assert!(!nothing_changed(&memory, &last, ""));
        assert!(!nothing_changed(&memory, &last, ""), "never kept in memory");
    }

    #[test]
    fn jevs_request_names_both_decisions_redacted() {
        let a = Decision {
            id: "a".into(),
            title: "Supabase".into(),
            content: "write to jean@example.com".into(),
            created_at: "1".into(),
            space: Some("work".into()),
        };
        let b = Decision {
            title: "Cloudflare".into(),
            id: "b".into(),
            ..a.clone()
        };
        let body: Value = serde_json::from_str(&jev_request(&[(&a, &b)])).unwrap();
        assert_eq!(
            body["questions"]["replaces_0"]["instructions"]["older"]["title"],
            "Supabase"
        );
        assert_eq!(
            body["questions"]["same_0"]["instructions"]["newer"]["title"],
            "Cloudflare"
        );
        assert!(!body.to_string().contains("jean@example.com"));
        let reply =
            json!({ "answers": { "same_0": { "noul": 0.9 }, "replaces_0": { "noul": 0.8 } } });
        assert_eq!(jev_answers(&reply, 1), vec![(Some(0.9), Some(0.8))]);
    }

    /// Measures the decision pairs of `tidy_eval_cases.json` (six languages) with real calls:
    /// closeness by words and by meaning (the meaning worker), and Jev's answers when
    /// `TIDY_EVAL_JEV=1` and a TypeSafe key is set. Results to `TIDY_EVAL_OUT`.
    /// `cargo test tidy_eval -- --ignored --nocapture`
    #[test]
    #[ignore = "real calls"]
    fn tidy_eval() {
        let cases: Vec<Value> = serde_json::from_str(include_str!("tidy_eval_cases.json")).unwrap();
        let sense = crate::meaning::Sense::default();
        let decision = |v: &Value, id: &str| Decision {
            id: id.to_string(),
            title: v["title"].as_str().unwrap().to_string(),
            content: v["text"].as_str().unwrap().to_string(),
            created_at: String::new(),
            space: None,
        };
        let pairs: Vec<(Decision, Decision, String, String)> = cases
            .iter()
            .map(|c| {
                let id = c["id"].as_str().unwrap();
                (
                    decision(&c["older"], &format!("{id}-older")),
                    decision(&c["newer"], &format!("{id}-newer")),
                    id.to_string(),
                    c["expected"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let mut meaning = Vec::new();
        for (older, newer, _, _) in &pairs {
            let a = crate::meaning::pieces(&older.title, &older.content, "text");
            let b = crate::meaning::pieces(&newer.title, &newer.content, "text");
            let (va, _) = sense.embed(&a).expect("meaning worker");
            let (vb, _) = sense.embed(&b).expect("meaning worker");
            meaning.push(closest(&va, &vb));
        }
        let jev = (std::env::var("TIDY_EVAL_JEV").as_deref() == Ok("1"))
            .then(crate::natures::key)
            .flatten();
        let mut answers: Vec<(Option<f64>, Option<f64>)> = vec![(None, None); pairs.len()];
        if let Some(key) = jev {
            let client = reqwest::Client::new();
            for (start, chunk) in pairs.chunks(PAIRS_PER_CHECK).enumerate() {
                let both: Vec<(&Decision, &Decision)> =
                    chunk.iter().map(|(o, n, _, _)| (o, n)).collect();
                let (reply, _) = tauri::async_runtime::block_on(crate::jev::ask(
                    &client,
                    &key,
                    jev_request(&both),
                ));
                let got = match reply {
                    Ok(r) => jev_answers(&r, both.len()),
                    Err(e) => {
                        println!("Jev: {e:?}");
                        Vec::new()
                    }
                };
                for (i, a) in got.into_iter().enumerate() {
                    answers[start * PAIRS_PER_CHECK + i] = a;
                }
            }
        }
        let mut results = Vec::new();
        for (i, (older, newer, id, expected)) in pairs.iter().enumerate() {
            let (share, shared) = crate::germinate::overlap(
                &crate::germinate::words(&older.title),
                &crate::germinate::words(&newer.title),
            );
            let (same, replaces) = answers[i];
            println!(
                "{id:14} {expected:8} words {share:.2}/{shared} meaning {:.3} same {} replaces {}",
                meaning[i],
                same.map_or("-".into(), |v| format!("{v:.2}")),
                replaces.map_or("-".into(), |v| format!("{v:.2}")),
            );
            results.push(json!({ "id": id, "expected": expected, "words_share": share,
                "words_shared": shared, "meaning": meaning[i], "same": same, "replaces": replaces }));
        }
        if let Ok(out) = std::env::var("TIDY_EVAL_OUT") {
            std::fs::write(
                out,
                serde_json::to_string_pretty(&json!({
                    "questions": jev_version(), "results": results,
                }))
                .unwrap(),
            )
            .unwrap();
        }
    }
}
