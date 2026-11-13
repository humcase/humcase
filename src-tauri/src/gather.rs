//! Gathering and checking, the second part of "To tidy": two close cases of a project Humcase
//! proposes to file one inside the other (`group`), a case that may go against a decision in
//! force (`conflict`), and the summary of a project (decisions in force, work in progress, open
//! questions, what to check).
//!
//! Both kinds live in the `proposals` table beside the others. A group pair: `note_id` is the
//! case to file, `near_id` the case to file it in (the older one; the person may swap them),
//! `reason` is `duplicate` for two cases that say nearly the same thing. A conflict: `note_id`
//! is the case, `near_id` the decision, `passage` and `reason` the closest sentences of each.
//! Candidates are found on the computer, by meaning (the index) and by words; Jev (when the
//! person lets it read their cases) judges each pair once. Duplicates need no AI: the words
//! say it.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::permissions;
use crate::tidy::{case_ref, closest, vectors_of, CaseRef, TidyItem, TidyKind};

/// Propose to file close cases one inside the other (`0`: off).
pub const LOOK_FOR_GROUPS: &str = "tidy_group";
/// Closest top-level cases of the same project each case is compared with.
const NEIGHBOURS: usize = 2;
/// Closest unresolved cases of the same space each decision in force is compared with.
const CONFLICT_NEIGHBOURS: usize = 3;
/// Questions sent to Jev in one request.
const QUESTIONS_PER_CHECK: usize = 12;
/// Jev's chance that two cases belong together: from here the pair is proposed. Measured by
/// `gather_eval` (12 pairs, six languages): together 0.81 to 0.95, apart 0.14 or less.
pub const TOGETHER_MIN: f64 = 0.7;
/// Below this chance the pair is not proposed; between the two it is proposed "to check".
pub const TOGETHER_DOUBT: f64 = 0.4;
/// Jev's chance that a case goes against a decision (`gather_eval`, 10 pairs: conflicts 0.91 to
/// 0.97, the others 0.14 or less).
pub const CONFLICT_MIN: f64 = 0.7;
pub const CONFLICT_DOUBT: f64 = 0.4;
/// Share of words two cases have in common (Jaccard) from which they are read as duplicates.
pub const DUPLICATE_MIN: f64 = 0.8;
/// Fewest distinct words a case needs to be read as a duplicate: two short titles alike say
/// little.
const DUPLICATE_WORDS: usize = 4;
/// Top-level cases of one project compared word by word, the most recent first.
const DUPLICATE_SCAN: usize = 400;
/// Sentences of a case compared to cite a conflict.
const SENTENCES_MAX: usize = 40;
/// Characters of a cited sentence.
const SENTENCE_MAX: usize = 240;
/// Characters of a case compared (its words, its sentences for Jev).
const CONTENT_MAX: usize = 20_000;
/// Share of the title words two duplicates must have in common: cases written from one
/// template share most of their words, not their subject.
const DUPLICATE_TITLE_MIN: f64 = 0.5;
/// Jev's questions, versioned with the code.
const JEV_QUESTIONS: &str = include_str!("jev_gather.v1.json");

pub fn group_enabled(conn: &Connection) -> Result<bool, String> {
    Ok(permissions::get_setting(conn, LOOK_FOR_GROUPS)?.as_deref() != Some("0"))
}

pub fn set_group_enabled(conn: &Connection, enabled: bool) -> Result<(), String> {
    permissions::set_setting(conn, LOOK_FOR_GROUPS, if enabled { "1" } else { "0" })
}

// --- Candidates ------------------------------------------------------------------------------

/// A live case as compared with the others.
#[derive(Debug, Clone)]
pub struct Case {
    pub id: String,
    pub title: String,
    pub content: String,
    pub project_id: String,
    pub space: Option<String>,
    pub parent_id: Option<String>,
    pub nature: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Live cases: not in the trash, not archived, in a project not archived; the work profiles
/// (`profile.rs`) are left out. The content is the written text only (a sketch, an image, a
/// link or a file holds data, not words), clipped to [`CONTENT_MAX`].
pub fn live_cases(conn: &Connection) -> Result<Vec<Case>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT n.id, n.title,
                    CASE WHEN n.content_kind IN ('text', 'research')
                         THEN substr(COALESCE(n.content, ''), 1, ?1) ELSE '' END,
                    n.project_id, p.space, n.parent_id,
                    n.note_type, n.status, n.created_at, n.updated_at
               FROM notes n JOIN projects p ON p.id = n.project_id
              WHERE n.deleted_at IS NULL AND n.archived_at IS NULL AND p.archived_at IS NULL
                AND n.id NOT IN (SELECT note_id FROM work_profiles)
              ORDER BY n.created_at",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([CONTENT_MAX as i64], |r| {
            Ok(Case {
                id: r.get(0)?,
                title: r.get(1)?,
                content: r.get(2)?,
                project_id: r.get(3)?,
                space: r.get(4)?,
                parent_id: r.get(5)?,
                nature: r.get(6)?,
                status: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
            })
        })
        .map_err(crate::db::db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(crate::db::db_error)?;
    Ok(rows)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Check {
    /// Do `note_id` (the case to file) and `near_id` (where) belong together?
    Together,
    /// Does `note_id` go against the decision `near_id`?
    Conflict,
}

impl Check {
    fn kind(self) -> &'static str {
        match self {
            Check::Together => "group",
            Check::Conflict => "conflict",
        }
    }
}

/// A pair to judge, before Jev.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub check: Check,
    pub note_id: String,
    pub near_id: String,
    /// Nearly the same words: proposed without Jev.
    pub duplicate: bool,
    /// Closeness by meaning, to judge the closest first.
    pub closeness: f32,
}

/// Jaccard share of the words two cases have in common.
fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    let union = a.union(b).count();
    if union == 0 {
        return 0.0;
    }
    a.intersection(b).count() as f64 / union as f64
}

/// Words read to find duplicates: the words of four letters or more, and every token holding a
/// digit, kept whole ("v1.4.0", "4,90"): two reports that differ by a version or an amount are
/// not the same case (on a real space, "lot v1.4.0" and "lot v1.5.0" read as duplicates when
/// the numbers were dropped).
/// The headings of a template ("Observed behavior", "Steps to reproduce") are left out, and
/// the words too common to tell cases apart.
fn duplicate_words(case: &Case) -> Words {
    let body: Vec<&str> = case
        .content
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect();
    Words {
        all: tokens(&format!("{} {}", case.title, body.join("\n"))),
        title: tokens(&case.title),
    }
}

fn tokens(text: &str) -> HashSet<String> {
    crate::germinate::normalize(text)
        .to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '.' || c == ','))
        .map(|w| w.trim_matches(['.', ',']))
        .filter(|w| w.chars().count() >= 4 || w.chars().any(|c| c.is_ascii_digit()))
        .filter(|w| !crate::common_words::is_common(w))
        .map(str::to_string)
        .collect()
}

/// The words of a case, and those of its title.
struct Words {
    all: HashSet<String>,
    title: HashSet<String>,
}

/// Two cases that say nearly the same thing, filed together without AI: enough words, most of
/// them shared, titles on one subject, neither a decision (compared with cases instead), not
/// both resolved (archived rather than filed).
fn duplicates(a: &Case, a_words: &Words, b: &Case, b_words: &Words) -> bool {
    a_words.all.len() >= DUPLICATE_WORDS
        && b_words.all.len() >= DUPLICATE_WORDS
        && a.nature != "decision"
        && b.nature != "decision"
        && !(a.status == "resolved" && b.status == "resolved")
        && jaccard(&a_words.title, &b_words.title) >= DUPLICATE_TITLE_MIN
        && jaccard(&a_words.all, &b_words.all) >= DUPLICATE_MIN
}

/// Pairs already asked, waiting or answered, in both orders for groups.
fn asked(conn: &Connection) -> Result<HashSet<(Check, String, String)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT kind, note_id, near_id FROM proposals
              WHERE kind IN ('group', 'conflict') AND near_id IS NOT NULL",
        )
        .map_err(crate::db::db_error)?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(crate::db::db_error)?
        .collect::<Result<_, _>>()
        .map_err(crate::db::db_error)?;
    let mut out = HashSet::new();
    for (kind, note, near) in rows {
        if kind == "group" {
            out.insert((Check::Together, near.clone(), note.clone()));
            out.insert((Check::Together, note, near));
        } else {
            out.insert((Check::Conflict, note, near));
        }
    }
    Ok(out)
}

fn superseded(conn: &Connection) -> Result<HashSet<String>, String> {
    Ok(crate::tidy::superseded_ids(conn)?.into_iter().collect())
}

/// The `limit` cases of `others` closest in meaning to `case` (both read by the index).
fn nearest<'a>(
    case: &Case,
    others: impl Iterator<Item = &'a Case>,
    vectors: &HashMap<String, Vec<Vec<f32>>>,
    limit: usize,
) -> Vec<(&'a Case, f32)> {
    let Some(mine) = vectors.get(&case.id) else {
        return Vec::new();
    };
    let mut scored: Vec<(&Case, f32)> = others
        .filter(|other| other.id != case.id)
        .filter_map(|other| Some((other, closest(mine, vectors.get(&other.id)?))))
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.truncate(limit);
    scored
}

/// Pairs to judge: top-level cases of one project close in meaning (or nearly the same words),
/// and decisions in force with the unresolved cases closest to them, never asked before.
/// Duplicates come first, then the closest pairs.
pub fn candidates(
    conn: &Connection,
    data_dir: Option<&Path>,
    groups: bool,
    conflicts: bool,
) -> Result<Vec<Candidate>, String> {
    let cases = live_cases(conn)?;
    let asked = asked(conn)?;
    let replaced = superseded(conn)?;
    let vectors = data_dir
        .filter(|dir| crate::meaning::enabled(dir) && crate::meaning::has_vectors(dir))
        .and_then(|dir| crate::meaning::open_index(dir).ok())
        .map(|index| {
            let ids: Vec<String> = cases.iter().map(|c| c.id.clone()).collect();
            vectors_of(&index, &ids)
        })
        .unwrap_or_default();
    let mut out: Vec<Candidate> = Vec::new();
    let mut seen: HashSet<(Check, String, String)> = HashSet::new();
    let mut push = |out: &mut Vec<Candidate>, candidate: Candidate| {
        let key = (
            candidate.check,
            candidate.note_id.clone(),
            candidate.near_id.clone(),
        );
        let swapped = (
            candidate.check,
            candidate.near_id.clone(),
            candidate.note_id.clone(),
        );
        if asked.contains(&key) || seen.contains(&key) || seen.contains(&swapped) {
            return;
        }
        seen.insert(key);
        out.push(candidate);
    };

    if groups {
        let mut by_project: HashMap<&str, Vec<&Case>> = HashMap::new();
        for case in cases.iter().filter(|c| c.parent_id.is_none()) {
            by_project.entry(&case.project_id).or_default().push(case);
        }
        // The case to file is the newer one: the older usually holds the subject.
        let pair = |a: &Case, b: &Case| -> (String, String) {
            if a.created_at <= b.created_at {
                (b.id.clone(), a.id.clone())
            } else {
                (a.id.clone(), b.id.clone())
            }
        };
        // A decision holds for many cases: it is compared with them (replaced, contradicted),
        // not filed with one (on a test space, every pair with a decision Jev judged "together"
        // filed a task inside the rule it applies). Two resolved cases are archived rather than
        // filed.
        let fits = |a: &Case, b: &Case| {
            a.nature != "decision"
                && b.nature != "decision"
                && !(a.status == "resolved" && b.status == "resolved")
        };
        for project in by_project.values() {
            let recent: Vec<&Case> = {
                let mut list = project.clone();
                list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                list.truncate(DUPLICATE_SCAN);
                list
            };
            let words: Vec<Words> = recent.iter().map(|c| duplicate_words(c)).collect();
            for i in 0..recent.len() {
                for j in (i + 1)..recent.len() {
                    if duplicates(recent[i], &words[i], recent[j], &words[j]) {
                        let (note_id, near_id) = pair(recent[i], recent[j]);
                        push(
                            &mut out,
                            Candidate {
                                check: Check::Together,
                                note_id,
                                near_id,
                                duplicate: true,
                                closeness: 1.0,
                            },
                        );
                    }
                }
            }
            for case in &recent {
                let near = nearest(
                    case,
                    recent.iter().copied().filter(|other| fits(case, other)),
                    &vectors,
                    NEIGHBOURS,
                );
                for (other, closeness) in near {
                    let (note_id, near_id) = pair(case, other);
                    push(
                        &mut out,
                        Candidate {
                            check: Check::Together,
                            note_id,
                            near_id,
                            duplicate: false,
                            closeness,
                        },
                    );
                }
            }
        }
    }

    if conflicts {
        let in_force: Vec<&Case> = cases
            .iter()
            .filter(|c| c.nature == "decision" && !replaced.contains(&c.id))
            .collect();
        for decision in in_force {
            let others = cases.iter().filter(|c| {
                c.nature != "decision"
                    && c.status != "resolved"
                    && c.space == decision.space
                    && c.parent_id.as_deref() != Some(decision.id.as_str())
                    && decision.parent_id.as_deref() != Some(c.id.as_str())
            });
            for (other, closeness) in nearest(decision, others, &vectors, CONFLICT_NEIGHBOURS) {
                push(
                    &mut out,
                    Candidate {
                        check: Check::Conflict,
                        note_id: other.id.clone(),
                        near_id: decision.id.clone(),
                        duplicate: false,
                        closeness,
                    },
                );
            }
        }
    }

    out.sort_by(|a, b| {
        b.duplicate
            .cmp(&a.duplicate)
            .then(b.closeness.total_cmp(&a.closeness))
    });
    Ok(out)
}

// --- Jev -------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct JevQuestions {
    version: String,
    model: String,
    together: JevQuestion,
    conflict: JevQuestion,
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

/// Version of Jev's questions (`gather.v1`).
pub fn jev_version() -> &'static str {
    &jev_questions().version
}

/// Jev's request: one yes/no question per pair, titles and texts redacted before they leave.
fn jev_request(pairs: &[(&Candidate, &Case, &Case)]) -> String {
    let questions = jev_questions();
    let mut asked = serde_json::Map::new();
    for (i, (candidate, case, near)) in pairs.iter().enumerate() {
        let (mut instructions, question, key) = match candidate.check {
            Check::Together => (
                json!({
                    "a": crate::jev::state_of(&near.title, &near.content),
                    "b": crate::jev::state_of(&case.title, &case.content),
                }),
                &questions.together,
                "together",
            ),
            Check::Conflict => (
                json!({
                    "decision": crate::jev::state_of(&near.title, &near.content),
                    "case": crate::jev::state_of(&case.title, &case.content),
                }),
                &questions.conflict,
                "conflict",
            ),
        };
        instructions["question"] = json!(question.instructions);
        asked.insert(
            format!("{key}_{i}"),
            json!({ "type": "noul", "instructions": instructions, "criteria": question.criteria }),
        );
    }
    json!({
        "state": { "notes": "cases from one person's notes" },
        "model": questions.model,
        "questions": asked,
    })
    .to_string()
}

fn jev_answers(reply: &Value, pairs: &[(&Candidate, &Case, &Case)]) -> Vec<Option<f64>> {
    pairs
        .iter()
        .enumerate()
        .map(|(i, (candidate, _, _))| {
            let key = match candidate.check {
                Check::Together => "together",
                Check::Conflict => "conflict",
            };
            reply["answers"][format!("{key}_{i}")]["noul"].as_f64()
        })
        .collect()
}

/// A pair as judged, ready to store.
#[derive(Debug, Clone)]
pub struct Judged {
    pub candidate: Candidate,
    /// Jev's chance (together, or goes against); `None` when nobody judged it.
    pub chance: Option<f64>,
    /// The closest sentences of the case and of the decision (conflicts).
    pub passages: Option<(String, String)>,
}

/// Asks Jev about the pairs that need it (duplicates do not); a pair stays unjudged when Jev
/// may not read, has no key, or fails.
pub async fn judge(
    db_path: &Path,
    data_dir: &Path,
    cases: &[Case],
    candidates: Vec<Candidate>,
) -> Vec<Judged> {
    let (duplicates, to_ask): (Vec<Candidate>, Vec<Candidate>) =
        candidates.into_iter().partition(|c| c.duplicate);
    let mut out: Vec<Judged> = duplicates
        .into_iter()
        .map(|candidate| Judged {
            candidate,
            chance: None,
            passages: None,
        })
        .collect();
    let to_ask: Vec<Candidate> = to_ask.into_iter().take(QUESTIONS_PER_CHECK).collect();
    let key = crate::natures::enabled(data_dir)
        .then(crate::natures::key)
        .flatten();
    let by_id: HashMap<&str, &Case> = cases.iter().map(|c| (c.id.as_str(), c)).collect();
    let pairs: Vec<(&Candidate, &Case, &Case)> = to_ask
        .iter()
        .filter_map(|c| {
            Some((
                c,
                *by_id.get(c.note_id.as_str())?,
                *by_id.get(c.near_id.as_str())?,
            ))
        })
        .collect();
    let answers = match key {
        Some(key) if !pairs.is_empty() => ask(db_path, &key, &pairs).await,
        _ => vec![None; pairs.len()],
    };
    for ((candidate, _, _), chance) in pairs.iter().zip(answers) {
        out.push(Judged {
            candidate: (*candidate).clone(),
            chance,
            passages: None,
        });
    }
    out
}

async fn ask(db_path: &Path, key: &str, pairs: &[(&Candidate, &Case, &Case)]) -> Vec<Option<f64>> {
    let none = vec![None; pairs.len()];
    let Ok(client) = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
    else {
        return none;
    };
    let started = Instant::now();
    let (reply, tokens) = crate::jev::ask(&client, key, jev_request(pairs)).await;
    let model = reply
        .as_ref()
        .map(crate::jev::model_of)
        .unwrap_or_else(|_| jev_questions().model.clone());
    crate::ai_usage::record(
        db_path,
        &crate::ai_usage::UsageEntry {
            operation: crate::ai_usage::operation::CHECK_GATHER,
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
    match reply {
        Ok(reply) => jev_answers(&reply, pairs),
        Err(_) => none,
    }
}

/// Sentences of a case worth citing: its title, then the lines of its text cut at the end of
/// each sentence, without Markdown marks.
pub(crate) fn sentences(title: &str, content: &str) -> Vec<String> {
    let mut out = vec![title.trim().to_string()];
    for line in content.lines() {
        let line = line
            .trim()
            .trim_start_matches(['#', '-', '*', '>', ' '])
            .replace("**", "")
            .replace('`', "");
        if line.starts_with("```") {
            continue;
        }
        let mut current = String::new();
        let chars: Vec<char> = line.chars().collect();
        for (i, c) in chars.iter().enumerate() {
            current.push(*c);
            let ends = matches!(c, '.' | '!' | '?' | ';')
                && chars.get(i + 1).is_none_or(|next| next.is_whitespace());
            if ends {
                out.push(current.trim().to_string());
                current.clear();
            }
        }
        out.push(current.trim().to_string());
    }
    let mut seen = HashSet::new();
    out.into_iter()
        .filter(|s| s.chars().filter(|c| c.is_alphanumeric()).count() >= 8)
        .map(|s| {
            if s.chars().count() > SENTENCE_MAX {
                s.chars()
                    .take(SENTENCE_MAX)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
                    + "…"
            } else {
                s
            }
        })
        .filter(|s| seen.insert(s.clone()))
        .take(SENTENCES_MAX)
        .collect()
}

/// The sentence of the case and the sentence of the decision closest in meaning: what each
/// says on the point where they may disagree. `None` without the meaning worker.
pub fn cite(
    sense: &crate::meaning::Sense,
    case: &Case,
    decision: &Case,
) -> Option<(String, String)> {
    let mine = sentences(&case.title, &case.content);
    let theirs = sentences(&decision.title, &decision.content);
    if mine.is_empty() || theirs.is_empty() {
        return None;
    }
    let all: Vec<String> = mine.iter().chain(theirs.iter()).cloned().collect();
    let (vectors, _) = sense.embed(&all).ok()?;
    let (a, b) = vectors.split_at(mine.len());
    let mut best = (f32::MIN, 0, 0);
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            let score: f32 = x.iter().zip(y).map(|(p, q)| p * q).sum();
            if score > best.0 {
                best = (score, i, j);
            }
        }
    }
    Some((mine[best.1].clone(), theirs[best.2].clone()))
}

/// Cites the conflicts Jev reads (from [`CONFLICT_DOUBT`]): the worker runs off the async
/// threads.
pub async fn cite_conflicts(
    sense: Option<Arc<crate::meaning::Sense>>,
    cases: &[Case],
    judged: Vec<Judged>,
) -> Vec<Judged> {
    let Some(sense) = sense else {
        return judged;
    };
    if crate::meaning::worker_dir().is_none() {
        return judged;
    }
    let by_id: HashMap<String, Case> = cases.iter().map(|c| (c.id.clone(), c.clone())).collect();
    let task = tokio::task::spawn_blocking(move || {
        judged
            .into_iter()
            .map(|mut item| {
                if item.candidate.check == Check::Conflict
                    && item.chance.is_some_and(|c| c >= CONFLICT_DOUBT)
                {
                    if let (Some(case), Some(decision)) = (
                        by_id.get(&item.candidate.note_id),
                        by_id.get(&item.candidate.near_id),
                    ) {
                        item.passages = cite(&sense, case, decision);
                    }
                }
                item
            })
            .collect::<Vec<_>>()
    });
    task.await.unwrap_or_default()
}

// --- Storing and withdrawing -----------------------------------------------------------------

/// Stores judged pairs: a duplicate is proposed; a pair Jev judged is proposed from the doubt
/// threshold ("to check" below the sure one) and `dismissed` below, so it is not judged again.
/// A pair nobody judged is not stored: it is judged once Jev can read. Returns the count of
/// proposals added.
pub fn store(conn: &Connection, judged: &[Judged]) -> Result<usize, String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let now: String = tx
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |r| {
            r.get(0)
        })
        .map_err(crate::db::db_error)?;
    let mut added = 0;
    for (position, item) in judged.iter().enumerate() {
        let doubt = match item.candidate.check {
            Check::Together => TOGETHER_DOUBT,
            Check::Conflict => CONFLICT_DOUBT,
        };
        let answer = match (item.candidate.duplicate, item.chance) {
            (true, _) => None,
            (false, Some(chance)) if chance >= doubt => None,
            (false, Some(_)) => Some("dismissed"),
            (false, None) => continue,
        };
        let (passage, reason) = match (&item.passages, item.candidate.duplicate) {
            (Some((mine, theirs)), _) => (mine.clone(), Some(theirs.clone())),
            (None, true) => (String::new(), Some("duplicate".to_string())),
            (None, false) => (String::new(), None),
        };
        let inserted = tx
            .execute(
                "INSERT INTO proposals (id, note_id, kind, nature, title, passage, reason, near_id,
                                        follows, revision, position, created_at, answer, answered_at)
                 SELECT ?1, n.id, ?3, n.note_type, n.title, ?4, ?5, ?6, ?7, n.revision, ?8, ?9, ?10,
                        CASE WHEN ?10 IS NULL THEN NULL ELSE ?9 END
                   FROM notes n WHERE n.id = ?2
                    AND NOT EXISTS (SELECT 1 FROM proposals p WHERE p.kind = ?3
                                       AND ((p.note_id = ?2 AND p.near_id = ?6)
                                            OR (?3 = 'group' AND p.note_id = ?6 AND p.near_id = ?2)))",
                params![
                    Uuid::new_v4().to_string(),
                    item.candidate.note_id,
                    item.candidate.check.kind(),
                    passage,
                    reason,
                    item.candidate.near_id,
                    item.chance,
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

/// Withdraws (`replaced`) the waiting pairs that no longer hold: a case filed meanwhile, moved
/// to another project, archived or trashed; a decision replaced or set aside, a case resolved.
pub fn withdraw_stale(conn: &Connection) -> Result<usize, String> {
    conn.execute(
        "UPDATE proposals SET answer = 'replaced', answered_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE answer IS NULL AND (
            (kind = 'group' AND (
                near_id IS NULL
                OR EXISTS (SELECT 1 FROM notes a, notes b
                            WHERE a.id = proposals.note_id AND b.id = proposals.near_id
                              AND (a.parent_id IS NOT NULL OR b.parent_id IS NOT NULL
                                   OR a.project_id <> b.project_id
                                   OR a.archived_at IS NOT NULL OR b.archived_at IS NOT NULL
                                   OR a.deleted_at IS NOT NULL OR b.deleted_at IS NOT NULL))))
            OR (kind = 'conflict' AND (
                near_id IS NULL
                OR EXISTS (SELECT 1 FROM case_supersessions s WHERE s.note_id = proposals.near_id)
                OR EXISTS (SELECT 1 FROM notes a, notes d
                            WHERE a.id = proposals.note_id AND d.id = proposals.near_id
                              AND (a.status = 'resolved' OR a.archived_at IS NOT NULL
                                   OR a.deleted_at IS NOT NULL OR d.archived_at IS NOT NULL
                                   OR d.deleted_at IS NOT NULL)))))",
        [],
    )
    .map_err(crate::db::db_error)
}

/// Withdraws (`replaced`) the waiting duplicates that the words no longer read as duplicates
/// (a case changed, or a pair stored by an earlier rule).
pub fn recheck_duplicates(conn: &Connection) -> Result<usize, String> {
    let waiting: Vec<(String, String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT id, note_id, near_id FROM proposals
                  WHERE kind = 'group' AND answer IS NULL AND reason = 'duplicate'
                    AND near_id IS NOT NULL",
            )
            .map_err(crate::db::db_error)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(crate::db::db_error)?
            .collect::<Result<_, _>>()
            .map_err(crate::db::db_error)?;
        rows
    };
    if waiting.is_empty() {
        return Ok(0);
    }
    let cases: HashMap<String, Case> = live_cases(conn)?
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();
    let mut withdrawn = 0;
    for (id, note_id, near_id) in waiting {
        let still = match (cases.get(&note_id), cases.get(&near_id)) {
            (Some(a), Some(b)) => duplicates(a, &duplicate_words(a), b, &duplicate_words(b)),
            _ => false,
        };
        if !still {
            withdrawn += conn
                .execute(
                    "UPDATE proposals SET answer = 'replaced',
                            answered_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                      WHERE id = ?1 AND answer IS NULL",
                    [&id],
                )
                .map_err(crate::db::db_error)?;
        }
    }
    Ok(withdrawn)
}

/// What a refresh asks Jev about.
pub struct Prepared {
    pub candidates: Vec<Candidate>,
    pub cases: Vec<Case>,
}

/// First step of a refresh, under the database lock: stale pairs withdrawn, then the pairs to
/// judge. Without Jev only duplicates are looked for: closeness alone does not tell two cases
/// of one subject from two cases of one field (measured on a real space: 0.90 to 0.96 for
/// both).
pub fn prepare(conn: &Connection, data_dir: &Path) -> Result<Option<Prepared>, String> {
    withdraw_stale(conn)?;
    recheck_duplicates(conn)?;
    let settings = crate::tidy::settings(conn)?;
    let jev = crate::natures::enabled(data_dir) && crate::natures::key().is_some();
    let groups = settings.group;
    let conflicts = settings.supersede && jev;
    if !groups && !conflicts {
        return Ok(None);
    }
    static LAST: crate::tidy::LastLook = std::sync::Mutex::new(None);
    let extra = format!("{jev}:{}", crate::meaning::has_vectors(data_dir));
    if crate::tidy::nothing_changed(conn, &LAST, &extra) {
        return Ok(None);
    }
    let mut candidates = candidates(conn, Some(data_dir), groups, conflicts)?;
    if !jev {
        candidates.retain(|c| c.duplicate);
    }
    if candidates.is_empty() {
        return Ok(None);
    }
    Ok(Some(Prepared {
        candidates,
        cases: live_cases(conn)?,
    }))
}

// --- A case's conflicts and a project's summary ----------------------------------------------

/// The waiting conflicts where `note_id` is the case (the band of its page).
pub fn conflicts_of(conn: &Connection, note_id: &str) -> Result<Vec<TidyItem>, String> {
    Ok(crate::tidy::pending(conn, None)?
        .into_iter()
        .filter(|item| item.kind == TidyKind::Conflict && item.case.id == note_id)
        .collect())
}

/// What holds today in a project, what moves, what stays open: read from its cases each time.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub project_id: String,
    pub project_name: String,
    /// Live decisions no newer one replaced, the newest first.
    pub decisions: Vec<CaseRef>,
    /// Decisions marked as replaced, kept as history.
    pub replaced_count: usize,
    pub in_progress: Vec<CaseRef>,
    /// Questions not answered yet.
    pub open_questions: Vec<CaseRef>,
    /// Cases that may go against a decision in force (waiting in "To tidy").
    pub to_check: Vec<TidyItem>,
    /// Resolved in the last two weeks.
    pub recently_resolved: Vec<CaseRef>,
}

fn refs(
    conn: &Connection,
    sql: &str,
    project_id: &str,
    date: &str,
) -> Result<Vec<CaseRef>, String> {
    let mut stmt = conn.prepare(sql).map_err(crate::db::db_error)?;
    let ids: Vec<String> = stmt
        .query_map([project_id], |r| r.get(0))
        .map_err(crate::db::db_error)?
        .collect::<Result<_, _>>()
        .map_err(crate::db::db_error)?;
    let mut out = Vec::new();
    for id in ids {
        if let Some(case) = case_ref(conn, &id, date)? {
            out.push(case);
        }
    }
    Ok(out)
}

pub fn project_summary(conn: &Connection, project_id: &str) -> Result<ProjectSummary, String> {
    let project_name: String = conn
        .query_row(
            "SELECT name FROM projects WHERE id = ?1",
            [project_id],
            |r| r.get(0),
        )
        .map_err(|_| format!("project {project_id} not found"))?;
    let live = "n.project_id = ?1 AND n.deleted_at IS NULL AND n.archived_at IS NULL";
    let decisions = refs(
        conn,
        &format!(
            "SELECT n.id FROM notes n WHERE {live} AND n.note_type = 'decision'
               AND NOT EXISTS (SELECT 1 FROM case_supersessions s
                                JOIN notes b ON b.id = s.by_id AND b.deleted_at IS NULL
                               WHERE s.note_id = n.id)
             ORDER BY n.created_at DESC"
        ),
        project_id,
        "created_at",
    )?;
    let replaced_count: usize = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM notes n WHERE {live} AND n.note_type = 'decision'
                   AND EXISTS (SELECT 1 FROM case_supersessions s
                                JOIN notes b ON b.id = s.by_id AND b.deleted_at IS NULL
                               WHERE s.note_id = n.id)"
            ),
            [project_id],
            |r| r.get::<_, i64>(0),
        )
        .map_err(crate::db::db_error)? as usize;
    let in_progress = refs(
        conn,
        &format!(
            "SELECT n.id FROM notes n WHERE {live} AND n.status = 'in_progress'
             ORDER BY n.updated_at DESC LIMIT 30"
        ),
        project_id,
        "updated_at",
    )?;
    let open_questions = refs(
        conn,
        &format!(
            "SELECT n.id FROM notes n WHERE {live} AND n.note_type = 'question'
               AND n.status <> 'resolved'
             ORDER BY n.updated_at DESC LIMIT 30"
        ),
        project_id,
        "updated_at",
    )?;
    let recently_resolved = refs(
        conn,
        &format!(
            "SELECT n.id FROM notes n WHERE {live} AND n.status = 'resolved'
               AND n.note_type <> 'decision'
               AND n.updated_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-14 days')
             ORDER BY n.updated_at DESC LIMIT 10"
        ),
        project_id,
        "updated_at",
    )?;
    let in_project: HashSet<String> = {
        let mut stmt = conn
            .prepare("SELECT id FROM notes WHERE project_id = ?1")
            .map_err(crate::db::db_error)?;
        let rows = stmt
            .query_map([project_id], |r| r.get(0))
            .map_err(crate::db::db_error)?
            .collect::<Result<_, _>>()
            .map_err(crate::db::db_error)?;
        rows
    };
    let to_check = crate::tidy::pending(conn, None)?
        .into_iter()
        .filter(|item| item.kind == TidyKind::Conflict && in_project.contains(&item.case.id))
        .collect();
    Ok(ProjectSummary {
        project_id: project_id.to_string(),
        project_name,
        decisions,
        replaced_count,
        in_progress,
        open_questions,
        to_check,
        recently_resolved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::note::{CreateNoteInput, NoteType};
    use crate::tidy::{TidyAnswer, TidyKind};

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::schema::apply_migrations(&conn).unwrap();
        conn
    }

    fn project(conn: &Connection, name: &str) -> String {
        crate::db::queries::create_project(conn, name, None).unwrap()
    }

    fn case(conn: &Connection, project: &str, title: &str, nature: NoteType, text: &str) -> String {
        crate::db::queries::create_note(
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

    fn written(conn: &Connection, id: &str, at: &str) {
        conn.execute(
            "UPDATE notes SET created_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![id, at],
        )
        .unwrap();
    }

    fn parent_of(conn: &Connection, id: &str) -> Option<String> {
        conn.query_row("SELECT parent_id FROM notes WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
    }

    const EXPORT: &str =
        "Quand je clique sur Exporter dans une case, rien ne se passe, aucun fichier n’est créé.";

    #[test]
    fn duplicates_are_found_by_words_and_proposed_without_jev() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let older = case(
            &conn,
            &p,
            "Le bouton Exporter ne fait rien",
            NoteType::Bug,
            EXPORT,
        );
        let newer = case(
            &conn,
            &p,
            "Le bouton Exporter ne fait rien",
            NoteType::Bug,
            EXPORT,
        );
        written(&conn, &older, "2026-09-01T10:00:00.000Z");
        written(&conn, &newer, "2026-09-02T10:00:00.000Z");
        let other = case(
            &conn,
            &p,
            "Synchronisation chiffrée",
            NoteType::Idea,
            "Plan de la synchronisation chiffrée de bout en bout entre deux ordinateurs.",
        );
        let found = candidates(&conn, None, true, true).unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].note_id, newer,
            "the newer case is filed in the older"
        );
        assert_eq!(found[0].near_id, older);
        assert!(found[0].duplicate);
        assert!(found
            .iter()
            .all(|c| c.note_id != other && c.near_id != other));

        let judged: Vec<Judged> = found
            .into_iter()
            .map(|candidate| Judged {
                candidate,
                chance: None,
                passages: None,
            })
            .collect();
        assert_eq!(store(&conn, &judged).unwrap(), 1);
        let items = crate::tidy::pending(&conn, None).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, TidyKind::Group);
        assert!(items[0].duplicate);
        assert_eq!(items[0].other.as_ref().unwrap().id, older);
        // Asked once: not found again, in either order.
        assert!(candidates(&conn, None, true, true).unwrap().is_empty());
        assert_eq!(store(&conn, &judged).unwrap(), 0);
    }

    /// Two bugs written from one template, on two subjects, and two sketches: none is a
    /// duplicate. Fails if template headings count, or if titles are not compared.
    #[test]
    fn cases_from_one_template_or_sketches_are_not_duplicates() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let template = |detail: &str| {
            format!(
                "### Observed behavior\n\nThe window does nothing when the button is pressed.\n\n\
                 ### Expected behavior\n\nThe window reacts at once to the button.\n\n\
                 ### Steps to reproduce\n\nOpen Humcase, choose a project, open a case, \
                 press the button twice, wait several seconds, look again at the window.\n\n\
                 ### Environment\n\nWindows 11, release build, laptop screen, French locale, \
                 version 0.9.2 installed from the official installer.\n\n{detail}"
            )
        };
        case(
            &conn,
            &p,
            "Export button ignored",
            NoteType::Bug,
            &template("Export."),
        );
        case(
            &conn,
            &p,
            "Dictation microphone silent",
            NoteType::Bug,
            &template("Dictation."),
        );
        for title in ["Plan A", "Plan B"] {
            let id = case(&conn, &p, title, NoteType::Idea, "");
            conn.execute(
                "UPDATE notes SET content_kind = 'sketch',
                        content = '{\"type\":\"excalidraw\",\"elements\":[{\"type\":\"rectangle\",\"strokeColor\":\"#1e1e1e\"}]}'
                  WHERE id = ?1",
                [&id],
            )
            .unwrap();
        }
        let found = candidates(&conn, None, true, true).unwrap();
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn reports_that_differ_by_a_version_or_are_both_resolved_are_not_duplicates() {
        let conn = db();
        let p = project(&conn, "Disk Cleaner");
        let report = |version: &str| {
            format!("Tâche liée au lot {version}. Résolue après vérification orchestrator.")
        };
        let a = case(
            &conn,
            &p,
            "Corrections audit 2 (v1.4.0)",
            NoteType::Todo,
            &report("v1.4.0"),
        );
        let b = case(
            &conn,
            &p,
            "Corrections audit 3 (v1.5.0)",
            NoteType::Todo,
            &report("v1.5.0"),
        );
        assert!(
            candidates(&conn, None, true, true).unwrap().is_empty(),
            "versions differ"
        );
        // A pair stored by the earlier rule (numbers dropped) is withdrawn at the next look.
        conn.execute(
            "INSERT INTO proposals (id, note_id, kind, nature, title, passage, reason, near_id,
                                    revision, position, created_at)
             VALUES ('old', ?1, 'group', 'todo', 't', '', 'duplicate', ?2, 0, 0, 'x')",
            params![b, a],
        )
        .unwrap();
        assert_eq!(recheck_duplicates(&conn).unwrap(), 1);
        assert!(crate::tidy::pending(&conn, None).unwrap().is_empty());

        // The same text twice is a duplicate, until both are resolved.
        let c = case(
            &conn,
            &p,
            "Corrections audit 4 (v1.6.0)",
            NoteType::Todo,
            &report("v1.6.0"),
        );
        let d = case(
            &conn,
            &p,
            "Corrections audit 4 (v1.6.0)",
            NoteType::Todo,
            &report("v1.6.0"),
        );
        assert_eq!(candidates(&conn, None, true, true).unwrap().len(), 1);
        for id in [&c, &d] {
            conn.execute("UPDATE notes SET status = 'resolved' WHERE id = ?1", [id])
                .unwrap();
        }
        assert!(
            candidates(&conn, None, true, true).unwrap().is_empty(),
            "both resolved"
        );
    }

    #[test]
    fn a_pair_nobody_judged_is_not_stored_and_jev_below_doubt_is_dismissed() {
        let conn = db();
        let p = project(&conn, "Maison");
        let a = case(
            &conn,
            &p,
            "Umzug nach Berlin",
            NoteType::Todo,
            "Umzug Ende Juni.",
        );
        let b = case(
            &conn,
            &p,
            "Kartons bestellen",
            NoteType::Todo,
            "40 Kartons bestellen.",
        );
        let c = case(
            &conn,
            &p,
            "Steuererklärung",
            NoteType::Todo,
            "Belege sammeln.",
        );
        let pair = |note: &str, near: &str, chance: Option<f64>| Judged {
            candidate: Candidate {
                check: Check::Together,
                note_id: note.to_string(),
                near_id: near.to_string(),
                duplicate: false,
                closeness: 0.9,
            },
            chance,
            passages: None,
        };
        let added = store(
            &conn,
            &[
                pair(&b, &a, Some(0.9)),
                pair(&c, &a, Some(0.1)),
                pair(&c, &b, None),
            ],
        )
        .unwrap();
        assert_eq!(added, 1);
        let items = crate::tidy::pending(&conn, None).unwrap();
        assert_eq!(items.len(), 1);
        assert!(!items[0].doubt);
        assert!(items[0].checked);
        let asked = asked(&conn).unwrap();
        assert!(
            asked.contains(&(Check::Together, c.clone(), a.clone())),
            "dismissed is asked"
        );
        assert!(
            !asked.contains(&(Check::Together, c, b)),
            "unjudged is judged later"
        );
    }

    #[test]
    fn grouping_files_the_case_and_undo_takes_it_out() {
        let conn = db();
        let p = project(&conn, "Voyage");
        let trip = case(
            &conn,
            &p,
            "Viaggio in Giappone",
            NoteType::Idea,
            "Tokyo, Kyoto, Osaka.",
        );
        let ryokan = case(
            &conn,
            &p,
            "Prenotare un ryokan",
            NoteType::Todo,
            "Tre notti a Kyoto.",
        );
        let judged = Judged {
            candidate: Candidate {
                check: Check::Together,
                note_id: ryokan.clone(),
                near_id: trip.clone(),
                duplicate: false,
                closeness: 0.9,
            },
            chance: Some(0.9),
            passages: None,
        };
        store(&conn, &[judged]).unwrap();
        let id = crate::tidy::pending(&conn, None).unwrap()[0].id.clone();
        let outcome = crate::tidy::answer(
            &conn,
            &[TidyAnswer {
                id: id.clone(),
                accept: true,
                swap: false,
            }],
        )
        .unwrap();
        assert_eq!(outcome.grouped, 1);
        assert_eq!(parent_of(&conn, &ryokan).as_deref(), Some(trip.as_str()));
        crate::tidy::undo(&conn, std::slice::from_ref(&id)).unwrap();
        assert_eq!(parent_of(&conn, &ryokan), None);

        // Swapped: the older one goes in the newer.
        crate::tidy::answer(
            &conn,
            &[TidyAnswer {
                id: id.clone(),
                accept: true,
                swap: true,
            }],
        )
        .unwrap();
        assert_eq!(parent_of(&conn, &trip).as_deref(), Some(ryokan.as_str()));
        assert_eq!(parent_of(&conn, &ryokan), None);
        crate::tidy::undo(&conn, &[id]).unwrap();
        assert_eq!(parent_of(&conn, &trip), None);
    }

    #[test]
    fn a_pair_that_no_longer_holds_is_withdrawn() {
        let conn = db();
        let p = project(&conn, "Boutique");
        let a = case(
            &conn,
            &p,
            "Migrar a loja",
            NoteType::Todo,
            "Shopify para Nuvemshop.",
        );
        let b = case(
            &conn,
            &p,
            "Frete dos Correios",
            NoteType::Todo,
            "Ativar o frete.",
        );
        let decision = case(
            &conn,
            &p,
            "Fotos com fundo branco",
            NoteType::Decision,
            "Sempre.",
        );
        let photos = case(
            &conn,
            &p,
            "Fotos sobre madeira",
            NoteType::Todo,
            "Madeira escura.",
        );
        let judged = |check, note: &str, near: &str| Judged {
            candidate: Candidate {
                check,
                note_id: note.to_string(),
                near_id: near.to_string(),
                duplicate: false,
                closeness: 0.9,
            },
            chance: Some(0.95),
            passages: Some(("Madeira escura.".into(), "Sempre.".into())),
        };
        store(
            &conn,
            &[
                judged(Check::Together, &b, &a),
                judged(Check::Conflict, &photos, &decision),
            ],
        )
        .unwrap();
        assert_eq!(crate::tidy::pending(&conn, None).unwrap().len(), 2);
        let conflict = conflicts_of(&conn, &photos).unwrap();
        assert_eq!(conflict.len(), 1);
        assert_eq!(conflict[0].passage.as_deref(), Some("Madeira escura."));
        assert_eq!(conflict[0].other_passage.as_deref(), Some("Sempre."));

        crate::db::cases::move_case(&conn, &b, Some(&a)).unwrap();
        conn.execute(
            "UPDATE notes SET status = 'resolved' WHERE id = ?1",
            [&photos],
        )
        .unwrap();
        assert_eq!(withdraw_stale(&conn).unwrap(), 2);
        assert!(crate::tidy::pending(&conn, None).unwrap().is_empty());
    }

    #[test]
    fn a_conflict_is_acknowledged_or_refused_without_changing_the_cases() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let decision = case(
            &conn,
            &p,
            "Jamais de prix affiché",
            NoteType::Decision,
            "Aucun prix.",
        );
        let task = case(
            &conn,
            &p,
            "Afficher le coût en dollars",
            NoteType::Todo,
            "En dollars.",
        );
        let revision = |id: &str| -> i64 {
            conn.query_row("SELECT revision FROM notes WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
        };
        let before = (revision(&decision), revision(&task));
        store(
            &conn,
            &[Judged {
                candidate: Candidate {
                    check: Check::Conflict,
                    note_id: task.clone(),
                    near_id: decision.clone(),
                    duplicate: false,
                    closeness: 0.9,
                },
                chance: Some(0.5),
                passages: None,
            }],
        )
        .unwrap();
        let item = &crate::tidy::pending(&conn, None).unwrap()[0];
        assert_eq!(item.kind, TidyKind::Conflict);
        assert!(item.doubt, "between doubt and sure: to check");
        let summary = project_summary(&conn, &p).unwrap();
        assert_eq!(summary.to_check.len(), 1);
        assert_eq!(summary.decisions.len(), 1);
        let outcome = crate::tidy::answer(
            &conn,
            &[TidyAnswer {
                id: item.id.clone(),
                accept: true,
                swap: false,
            }],
        )
        .unwrap();
        assert_eq!(outcome.to_fix, 1);
        assert!(conflicts_of(&conn, &task).unwrap().is_empty());
        assert_eq!((revision(&decision), revision(&task)), before);
    }

    /// The brief of a project names its case that may go against a decision of another
    /// project, even when no decision of its own is in force.
    #[test]
    fn the_brief_names_a_conflict_with_another_projects_decision() {
        let conn = db();
        let rules = project(&conn, "Règles");
        let app = project(&conn, "App");
        let decision = case(
            &conn,
            &rules,
            "Jamais de prix affiché",
            NoteType::Decision,
            "Aucun prix.",
        );
        let task = case(
            &conn,
            &app,
            "Afficher le coût en dollars",
            NoteType::Todo,
            "En dollars.",
        );
        store(
            &conn,
            &[Judged {
                candidate: Candidate {
                    check: Check::Conflict,
                    note_id: task.clone(),
                    near_id: decision.clone(),
                    duplicate: false,
                    closeness: 0.9,
                },
                chance: Some(0.9),
                passages: None,
            }],
        )
        .unwrap();
        let brief = crate::db::queries::build_agent_handoff(&conn, Some(&app)).unwrap();
        assert!(
            brief.contains(&decision) && brief.contains(&task),
            "{brief}"
        );
    }

    #[test]
    fn the_summary_reads_what_holds_moves_and_stays_open() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let old = case(
            &conn,
            &p,
            "Supabase",
            NoteType::Decision,
            "Supabase pour le cloud.",
        );
        let new = case(
            &conn,
            &p,
            "Cloudflare",
            NoteType::Decision,
            "Cloudflare pour le cloud.",
        );
        conn.execute(
            "INSERT INTO case_supersessions (note_id, by_id, superseded_at) VALUES (?1, ?2, 'x')",
            params![old, new],
        )
        .unwrap();
        let q = case(
            &conn,
            &p,
            "Un profil par espace ?",
            NoteType::Question,
            "À voir.",
        );
        let t = case(&conn, &p, "Lot 5", NoteType::Todo, "Regrouper.");
        conn.execute(
            "UPDATE notes SET status = 'in_progress' WHERE id = ?1",
            [&t],
        )
        .unwrap();
        let summary = project_summary(&conn, &p).unwrap();
        assert_eq!(
            summary
                .decisions
                .iter()
                .map(|d| d.id.as_str())
                .collect::<Vec<_>>(),
            [new.as_str()]
        );
        assert_eq!(summary.replaced_count, 1);
        assert_eq!(summary.open_questions[0].id, q);
        assert_eq!(summary.in_progress[0].id, t);
    }

    #[test]
    fn the_brief_lists_decisions_in_force_and_cases_against_them() {
        let conn = db();
        let p = project(&conn, "Humcase");
        let old = case(&conn, &p, "Supabase", NoteType::Decision, "Supabase.");
        let new = case(
            &conn,
            &p,
            "No prices shown",
            NoteType::Decision,
            "No price.",
        );
        conn.execute(
            "INSERT INTO case_supersessions (note_id, by_id, superseded_at) VALUES (?1, ?2, 'x')",
            params![old, new],
        )
        .unwrap();
        let task = case(
            &conn,
            &p,
            "Show the cost in dollars",
            NoteType::Todo,
            "Dollars.",
        );
        store(
            &conn,
            &[Judged {
                candidate: Candidate {
                    check: Check::Conflict,
                    note_id: task.clone(),
                    near_id: new.clone(),
                    duplicate: false,
                    closeness: 0.9,
                },
                chance: Some(0.95),
                passages: None,
            }],
        )
        .unwrap();
        let brief = crate::i18n::with_language("en", || {
            crate::db::queries::build_agent_handoff(&conn, Some(&p)).unwrap()
        });
        let in_force = brief.split("## Decisions in force").nth(1).expect(&brief);
        let in_force = in_force.split("\n## ").next().unwrap();
        assert!(
            in_force.contains(&format!("No prices shown [{new}]")),
            "{brief}"
        );
        assert!(
            !in_force.contains(&old),
            "a replaced decision is not in force: {brief}"
        );
        assert!(
            in_force.contains(&format!(
                "Show the cost in dollars [{task}] may go against the decision No prices shown [{new}]"
            )),
            "{brief}"
        );
    }

    #[test]
    fn sentences_cut_lines_and_skip_marks() {
        let found = sentences(
            "Jamais de prix",
            "### Décision\n- On n’affiche aucun prix. Seulement des quantités !\n```\ncode\n```",
        );
        assert_eq!(
            found,
            [
                "Jamais de prix",
                "Décision",
                "On n’affiche aucun prix.",
                "Seulement des quantités !"
            ]
        );
    }

    #[test]
    fn jevs_request_redacts_and_names_each_question() {
        let a = Case {
            id: "a".into(),
            title: "Write to bob@example.com".into(),
            content: "Plan".into(),
            project_id: "p".into(),
            space: None,
            parent_id: None,
            nature: "todo".into(),
            status: "open".into(),
            created_at: "1".into(),
            updated_at: "1".into(),
        };
        let b = Case {
            id: "b".into(),
            ..a.clone()
        };
        let together = Candidate {
            check: Check::Together,
            note_id: "b".into(),
            near_id: "a".into(),
            duplicate: false,
            closeness: 0.9,
        };
        let conflict = Candidate {
            check: Check::Conflict,
            ..together.clone()
        };
        let body = jev_request(&[(&together, &b, &a), (&conflict, &b, &a)]);
        assert!(!body.contains("bob@example.com"));
        let body: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["model"], "jev-1.13.0");
        assert!(body["questions"]["together_0"]["instructions"]["a"].is_object());
        assert!(body["questions"]["conflict_1"]["instructions"]["decision"].is_object());
    }

    /// Jev on the synthetic bench (no real case is sent): `cargo test gather_eval -- --ignored
    /// --nocapture` with TYPESAFE_API_KEY set.
    #[tokio::test]
    #[ignore]
    async fn gather_eval() {
        let key = std::env::var(crate::jev::KEY_ENV).expect("TYPESAFE_API_KEY");
        let bench: Value =
            serde_json::from_str(include_str!("gather_eval_cases.json")).expect("valid bench");
        let to_case = |id: &str, v: &Value| Case {
            id: id.to_string(),
            title: v["title"].as_str().unwrap().to_string(),
            content: v["text"].as_str().unwrap().to_string(),
            project_id: "p".into(),
            space: None,
            parent_id: None,
            nature: "todo".into(),
            status: "open".into(),
            created_at: "1".into(),
            updated_at: "1".into(),
        };
        let dir = std::env::temp_dir().join(format!("humcase-gather-eval-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("humcase.db");
        let mut failures = 0;
        for (list, check, yes) in [
            ("together", Check::Together, "together"),
            ("conflicts", Check::Conflict, "conflict"),
        ] {
            let items = bench[list].as_array().unwrap();
            let mut owned = Vec::new();
            for item in items {
                let (case, near) = match check {
                    Check::Together => (to_case("b", &item["b"]), to_case("a", &item["a"])),
                    Check::Conflict => {
                        (to_case("c", &item["case"]), to_case("d", &item["decision"]))
                    }
                };
                owned.push((
                    Candidate {
                        check,
                        note_id: case.id.clone(),
                        near_id: near.id.clone(),
                        duplicate: false,
                        closeness: 0.9,
                    },
                    case,
                    near,
                ));
            }
            for chunk in owned.chunks(QUESTIONS_PER_CHECK) {
                let pairs: Vec<(&Candidate, &Case, &Case)> =
                    chunk.iter().map(|(c, a, b)| (c, a, b)).collect();
                let answers = ask(&db_path, &key, &pairs).await;
                for ((_, case, _), chance) in chunk.iter().zip(answers) {
                    let item = items
                        .iter()
                        .find(|i| {
                            let title = match check {
                                Check::Together => &i["b"]["title"],
                                Check::Conflict => &i["case"]["title"],
                            };
                            title.as_str() == Some(case.title.as_str())
                        })
                        .unwrap();
                    let expected = item["expected"].as_str() == Some(yes);
                    let min = match check {
                        Check::Together => TOGETHER_MIN,
                        Check::Conflict => CONFLICT_MIN,
                    };
                    let doubt = match check {
                        Check::Together => TOGETHER_DOUBT,
                        Check::Conflict => CONFLICT_DOUBT,
                    };
                    let ok = match chance {
                        Some(c) if expected => c >= min,
                        Some(c) => c < doubt,
                        None => false,
                    };
                    if !ok {
                        failures += 1;
                    }
                    println!(
                        "{:16} {:9} {:?} {}",
                        item["id"],
                        item["expected"],
                        chance,
                        if ok { "ok" } else { "FAIL" }
                    );
                }
            }
        }
        let usage = std::fs::read_to_string(dir.join("ai-usage.jsonl")).unwrap_or_default();
        println!("usage:\n{usage}");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(failures, 0);
    }
}
