//! "Ask Humcase": a question answered from the cases of a space.
//!
//! Humcase finds the cases on the computer (words, then meaning when it is on), sends the AI
//! chosen in Settings their numbered sentences and a one-line view of the latest cases, and the
//! AI answers sentence by sentence, each naming the excerpt it relies on. The code then keeps
//! only the sentences whose excerpt was really sent: a source shown is always a sentence of the
//! case, word for word. Without an AI, the closest cases only. The conversation lives in the
//! interface for the session; a follow-up is searched with the previous question and the cases
//! the previous answers cited.

use crate::db::queries::{self, NoteListFilter};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Provenance kind of a case kept from an answer. Such a case is never read as a source: an
/// answer of the AI is not evidence, the cases it cites are.
pub const KEPT_KIND: &str = "ask";
const KEPT_TITLE_MAX: usize = 200;

/// Cases whose sentences are sent in full.
const DETAILED_MAX: usize = 7;
/// Sentences of one case sent, its title included.
const SENTENCES_PER_CASE: usize = 9;
/// Latest cases of the space sent as one line each (for "what is waiting", "this week").
const OVERVIEW_MAX: usize = 25;
/// Previous exchanges the AI reads to understand a follow-up.
const PREVIOUS_MAX: usize = 3;
const QUESTION_MAX: usize = 1000;
const PREVIOUS_ANSWER_MAX: usize = 1200;
const SUB_CASE_TITLE_MAX: usize = 120;

const SYSTEM_PROMPT: &str = "You answer a person's question about their own work, using only the cases (their notes, tasks, decisions) given in the user message as JSON. Each case has numbered excerpts, the first one being its title; `overview` lists the latest cases of the space in one line each (title excerpt, nature, status, decision state, last change, due day). `conversation` holds the previous exchanges: use it to understand a follow-up question. Rules: answer in the language of the question, briefly (one to four sentences). Every sentence names in `excerpt` the id of the excerpt that states that very fact; a fact read from a case's nature, status, decision state, last change or due day names that case's title excerpt. A sentence without an excerpt is not shown, so do not write one. A case without a due day has no deadline. Never state anything the excerpts do not say: no outside knowledge, no guess, no advice, and no reason, cause or judgment that is not written in the cases. Never write excerpt ids or case numbers in `text`. A case with status resolved and decision `to_review` is a result waiting for the person's decision; `rework_requested` means the person asked for it to be redone. If the cases do not answer the question, set `found` to false and `sentences` to an empty list. `sub_case` proposes one next step only when the question or the cases clearly call for an action that has no case yet: a short title and the number of the case it belongs in; otherwise null.";

/// A previous exchange of the conversation, as the interface sends it.
#[derive(Debug, Clone, Deserialize)]
pub struct PreviousTurn {
    pub question: String,
    /// The answer as plain text (empty when there was none).
    #[serde(default)]
    pub answer: String,
    /// Cases the answer cited.
    #[serde(default)]
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnswerKind {
    Answer,
    NotFound,
    NoAi,
}

/// A piece of the answer; `cite` is the 1-based number of its source.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Part {
    pub text: String,
    pub cite: Option<usize>,
}

/// A case the answer relies on, with the sentences of it the answer cites (its title is not
/// repeated as a passage).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Source {
    pub note_id: String,
    pub title: String,
    pub passages: Vec<String>,
}

/// Passages shown under one source.
const PASSAGES_MAX: usize = 3;

/// A next step the answer proposes; nothing is written until the person clicks.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Action {
    pub kind: &'static str,
    pub title: String,
    pub parent_id: String,
    pub parent_title: String,
}

/// What left the computer: the AI service and how many cases it read.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Sent {
    pub service: String,
    pub excerpts: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Answer {
    pub kind: AnswerKind,
    pub parts: Vec<Part>,
    pub sources: Vec<Source>,
    pub action: Option<Action>,
    pub sent: Option<Sent>,
    /// Sentences of the AI dropped because the excerpt they named was not sent.
    pub dropped: usize,
}

impl Answer {
    fn empty(kind: AnswerKind) -> Self {
        Answer {
            kind,
            parts: Vec::new(),
            sources: Vec::new(),
            action: None,
            sent: None,
            dropped: 0,
        }
    }
}

/// A case sent to the AI.
#[derive(Debug, Clone)]
struct Case {
    id: String,
    title: String,
    project: String,
    nature: String,
    status: String,
    decision: String,
    updated: String,
    due: Option<String>,
    /// Sentences sent in full; empty for a case of the overview (its title is its excerpt).
    sentences: Vec<String>,
}

/// What the AI reads, with each excerpt's case and text to check the answer against.
#[derive(Debug, Default)]
struct Reading {
    today: String,
    detailed: Vec<Case>,
    overview: Vec<Case>,
    /// Excerpt id → (case id, case title, text).
    excerpts: HashMap<usize, (String, String, String)>,
}

impl Reading {
    fn case_count(&self) -> usize {
        let ids: HashSet<&str> = self
            .detailed
            .iter()
            .chain(&self.overview)
            .map(|c| c.id.as_str())
            .collect();
        ids.len()
    }

    /// Numbers every excerpt: detailed cases first, then the overview's titles. A case number
    /// (`case`) is its position in `detailed` + 1, or after them for the overview.
    fn message(&mut self, question: &str, previous: &[PreviousTurn]) -> Value {
        let mut next = 1usize;
        let mut excerpts = HashMap::new();
        let mut number = |case: &Case, text: &str| {
            let id = next;
            next += 1;
            excerpts.insert(id, (case.id.clone(), case.title.clone(), text.to_string()));
            id
        };
        let detailed: Vec<Value> = self
            .detailed
            .iter()
            .enumerate()
            .map(|(i, case)| {
                let lines: Vec<Value> = case
                    .sentences
                    .iter()
                    .map(|s| json!({ "id": number(case, s), "text": s }))
                    .collect();
                json!({
                    "case": i + 1,
                    "project": case.project,
                    "nature": case.nature,
                    "status": case.status,
                    "decision": case.decision,
                    "last_change": case.updated,
                    "due": case.due,
                    "excerpts": lines,
                })
            })
            .collect();
        let offset = self.detailed.len();
        let overview: Vec<Value> = self
            .overview
            .iter()
            .enumerate()
            .map(|(i, case)| {
                json!({
                    "case": offset + i + 1,
                    "title": { "id": number(case, &case.title), "text": case.title },
                    "project": case.project,
                    "nature": case.nature,
                    "status": case.status,
                    "decision": case.decision,
                    "last_change": case.updated,
                    "due": case.due,
                })
            })
            .collect();
        self.excerpts = excerpts;
        let conversation: Vec<Value> = previous
            .iter()
            .map(|turn| json!({ "question": turn.question, "answer": turn.answer }))
            .collect();
        json!({
            "today": self.today,
            "question": question,
            "conversation": conversation,
            "cases": detailed,
            "overview": overview,
        })
    }

    /// The case numbered `number` in the message.
    fn case_numbered(&self, number: usize) -> Option<&Case> {
        let index = number.checked_sub(1)?;
        self.detailed
            .get(index)
            .or_else(|| self.overview.get(index.checked_sub(self.detailed.len())?))
    }
}

/// A reference to the message the AI sometimes leaves in its text ("(id 4)", "(excerpt 11)",
/// "(extrait 3, 5)"): meaningless to the person, the citation is shown beside the sentence.
fn without_references(text: &str) -> String {
    static REFERENCE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let reference = REFERENCE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)\s*[(\[]\s*(?:ids?|excerpts?|extraits?|extractos?|trechos?|estratt[oi]|auszug|auszüge|cases?|cas|casos?|cas[oi]|fall|fälle)\s*[#:n°]*\s*\d+(?:\s*(?:,|and|et|y|e|und)\s*\d+)*\s*[)\]]",
        )
        .expect("valid pattern")
    });
    reference.replace_all(text, "").into_owned()
}

fn clip(text: &str, max: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max {
        return text;
    }
    text.chars()
        .take(max)
        .collect::<String>()
        .trim_end()
        .to_string()
        + "…"
}

/// Words of the question worth searching: three letters or more, common words left out.
fn search_words(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3 && !crate::common_words::is_common(w))
        .map(str::to_lowercase)
        .filter(|w| seen.insert(w.clone()))
        .take(8)
        .collect()
}

/// Scope of the projects read: `work`, `private`, or `unassigned` for the projects without a
/// space; all of them when `None`.
const UNASSIGNED: &str = "unassigned";

/// Cases sharing words with `query`: the more words a case holds, the higher (a word in its
/// title counts a little more), the order of the search breaking ties.
fn by_words(conn: &Connection, query: &str, space: Option<&str>) -> Result<Vec<String>, String> {
    let mut score: HashMap<String, (f64, usize)> = HashMap::new();
    for word in search_words(query) {
        let filter = NoteListFilter {
            // The projects without a space are kept by the caller.
            space: space.filter(|s| *s != UNASSIGNED).map(str::to_string),
            search_query: Some(word.clone()),
            ..Default::default()
        };
        let page = queries::list_notes_page(conn, &filter, 0, 30, false)?;
        for (rank, note) in page.notes.into_iter().enumerate() {
            let in_title = note.title.to_lowercase().contains(&word);
            let entry = score.entry(note.id).or_insert((0.0, rank));
            entry.0 += if in_title { 1.5 } else { 1.0 };
            entry.1 = entry.1.min(rank);
        }
    }
    let mut ranked: Vec<(String, (f64, usize))> = score.into_iter().collect();
    ranked.sort_by(|a, b| b.1 .0.total_cmp(&a.1 .0).then(a.1 .1.cmp(&b.1 .1)));
    Ok(ranked.into_iter().map(|(id, _)| id).collect())
}

/// The sentences of a case most related to the question, in their order, its title first.
fn pick_sentences(title: &str, content: &str, question_words: &HashSet<String>) -> Vec<String> {
    let title = title.trim().to_string();
    // `sentences` starts with the title, unless it is too short to be cited; a checklist item
    // reads as its words, without its box.
    let rest: Vec<String> = crate::gather::sentences(&title, content)
        .into_iter()
        .filter(|s| *s != title)
        .map(|s| {
            s.strip_prefix("[ ] ")
                .or_else(|| s.strip_prefix("[x] "))
                .or_else(|| s.strip_prefix("[X] "))
                .map_or(s.clone(), str::to_string)
        })
        .collect();
    let mut scored: Vec<(usize, usize)> = rest
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let shared = crate::germinate::words(s)
                .intersection(question_words)
                .count();
            (i, shared)
        })
        .collect();
    let any = scored.iter().any(|(_, shared)| *shared > 0);
    if any {
        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    }
    let mut keep: Vec<usize> = scored
        .into_iter()
        .take(SENTENCES_PER_CASE - 1)
        .map(|(i, _)| i)
        .collect();
    keep.sort_unstable();
    std::iter::once(title)
        .chain(keep.into_iter().map(|i| rest[i].clone()))
        .collect()
}

fn is_kept_answer(conn: &Connection, note_id: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM case_provenance WHERE note_id = ?1 AND kind = ?2)",
        params![note_id, KEPT_KIND],
        |r| r.get(0),
    )
    .map_err(crate::db::db_error)
}

/// A source of a kept answer, as the interface sends it back.
#[derive(Debug, Clone, Deserialize)]
pub struct KeptSource {
    pub title: String,
    #[serde(default)]
    pub passages: Vec<String>,
}

/// Keeps an answer as a case of the Inbox of `space`: the question as its title, `content` (the
/// answer and its sources, written by the interface in its language) as its text, marked by a
/// provenance so it is never read as a source, and one provenance per case it cites.
pub fn keep(
    conn: &Connection,
    space: crate::models::project::ProjectSpace,
    question: &str,
    content: &str,
    service: Option<&str>,
    sources: &[KeptSource],
) -> Result<String, String> {
    let inbox = queries::get_or_create_inbox(conn, space)?;
    let id = queries::create_note(
        conn,
        crate::models::note::CreateNoteInput {
            project_id: inbox,
            title: clip(question, KEPT_TITLE_MAX),
            content: Some(content.to_string()),
            note_type: crate::models::note::NoteType::Note,
            priority: None,
            git_branch: None,
            tags: None,
            due_at: None,
            raw_content: None,
        },
    )?;
    let label = match service {
        Some(service) => format!("Humcase · {}", clip(service, 100)),
        None => "Humcase".to_string(),
    };
    crate::db::cases::add_provenance(
        conn,
        &id,
        KEPT_KIND,
        Some(&label),
        None,
        Some(&clip(question, QUESTION_MAX)),
        None,
    )?;
    for source in sources {
        let excerpt = source.passages.join(" … ");
        crate::db::cases::add_provenance(
            conn,
            &id,
            "source",
            Some(&clip(&source.title, 400)),
            None,
            Some(&clip(&excerpt, 5000)),
            None,
        )?;
    }
    Ok(id)
}

fn decision_of(conn: &Connection, note_id: &str, status: &str) -> String {
    if status != "resolved" && status != "open" && status != "in_progress" {
        return "none".into();
    }
    crate::db::journal::review_state(conn, note_id)
        .map(|r| r.state)
        .unwrap_or_else(|_| "none".into())
}

fn project_names(conn: &Connection) -> Result<HashMap<String, String>, String> {
    Ok(queries::get_projects(conn)
        .map_err(crate::db::db_error)?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect())
}

/// Reads the cases the AI will answer from. `meaning_ids`: cases found by meaning, best first.
fn read(
    conn: &Connection,
    question: &str,
    previous: &[PreviousTurn],
    space: Option<&str>,
    meaning_ids: &[String],
) -> Result<Reading, String> {
    let projects = project_names(conn)?;
    let today: String = conn
        .query_row("SELECT date('now', 'localtime')", [], |r| r.get(0))
        .map_err(crate::db::db_error)?;
    // A follow-up is searched with the question before it.
    let mut query = question.to_string();
    if let Some(last) = previous.last() {
        query.push(' ');
        query.push_str(&last.question);
    }
    let words_found = by_words(conn, &query, space)?;
    let earlier: Vec<String> = previous
        .iter()
        .rev()
        .flat_map(|turn| turn.source_ids.iter().cloned())
        .collect();
    // Words first, then meaning, then the cases the conversation already cited.
    let mut ids: Vec<String> = Vec::new();
    for id in words_found
        .iter()
        .take(4)
        .chain(meaning_ids.iter().take(3))
        .chain(earlier.iter().take(3))
        .chain(words_found.iter().skip(4))
    {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    let question_words: HashSet<String> = crate::germinate::words(&query)
        .into_iter()
        .filter(|w| !crate::common_words::is_common(w))
        .collect();
    let in_space = |project_id: &str| -> Result<bool, String> {
        let Some(space) = space else { return Ok(true) };
        let found: Option<String> = conn
            .query_row(
                "SELECT space FROM projects WHERE id = ?1",
                [project_id],
                |r| r.get(0),
            )
            .map_err(crate::db::db_error)?;
        Ok(match found.as_deref() {
            None => space == UNASSIGNED,
            Some(found) => found == space,
        })
    };
    let mut detailed = Vec::new();
    for id in ids {
        if detailed.len() >= DETAILED_MAX {
            break;
        }
        let Ok(note) = queries::get_note_by_id(conn, &id) else {
            continue;
        };
        if note.deleted_at.is_some()
            || note.archived_at.is_some()
            || is_kept_answer(conn, &note.id)?
            || note.content_kind == crate::models::note::ContentKind::Sketch
            || !in_space(&note.project_id)?
        {
            continue;
        }
        let status = note.status.as_str().to_string();
        detailed.push(Case {
            decision: decision_of(conn, &note.id, &status),
            project: projects.get(&note.project_id).cloned().unwrap_or_default(),
            nature: note.note_type.as_str().to_string(),
            status,
            updated: note.updated_at.chars().take(10).collect(),
            due: note.due_at.clone(),
            sentences: pick_sentences(&note.title, &note.content, &question_words),
            id: note.id,
            title: note.title,
        });
    }
    let shown: HashSet<String> = detailed.iter().map(|c| c.id.clone()).collect();
    let mut stmt = conn
        .prepare(
            "SELECT n.id, n.title, n.note_type, n.status, n.updated_at, n.due_at, n.project_id
               FROM notes n JOIN projects p ON p.id = n.project_id
              WHERE n.deleted_at IS NULL AND n.archived_at IS NULL
                AND n.content_kind != 'sketch'
                AND NOT EXISTS (SELECT 1 FROM case_provenance cp
                                 WHERE cp.note_id = n.id AND cp.kind = 'ask')
                AND (?1 IS NULL OR p.space = ?1 OR (?1 = 'unassigned' AND p.space IS NULL))
              ORDER BY n.updated_at DESC LIMIT ?2",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map(params![space, (OVERVIEW_MAX + DETAILED_MAX) as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
            ))
        })
        .map_err(crate::db::db_error)?;
    let mut overview = Vec::new();
    for row in rows {
        let (id, title, nature, status, updated, due, project_id) =
            row.map_err(crate::db::db_error)?;
        if shown.contains(&id) || overview.len() >= OVERVIEW_MAX {
            continue;
        }
        overview.push(Case {
            decision: decision_of(conn, &id, &status),
            project: projects.get(&project_id).cloned().unwrap_or_default(),
            nature,
            status,
            updated: updated.chars().take(10).collect(),
            due,
            sentences: Vec::new(),
            id,
            title,
        });
    }
    Ok(Reading {
        today,
        detailed,
        overview,
        excerpts: HashMap::new(),
    })
}

/// The answer the AI is held to.
fn answer_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "found": { "type": "boolean" },
            "sentences": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" },
                        "excerpt": { "type": ["integer", "null"] }
                    },
                    "required": ["text", "excerpt"],
                    "additionalProperties": false
                }
            },
            "sub_case": {
                "anyOf": [
                    {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "case": { "type": "integer" }
                        },
                        "required": ["title", "case"],
                        "additionalProperties": false
                    },
                    { "type": "null" }
                ]
            }
        },
        "required": ["found", "sentences", "sub_case"],
        "additionalProperties": false
    })
}

#[derive(Debug, Deserialize)]
struct RawSentence {
    text: String,
    excerpt: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawSubCase {
    title: String,
    case: i64,
}

#[derive(Debug, Deserialize)]
struct RawAnswer {
    found: bool,
    #[serde(default)]
    sentences: Vec<RawSentence>,
    #[serde(default)]
    sub_case: Option<RawSubCase>,
}

fn parse_answer(text: &str) -> Result<RawAnswer, String> {
    let start = text.find('{');
    let end = text.rfind('}');
    let json = match (start, end) {
        (Some(start), Some(end)) if end > start => &text[start..=end],
        _ => return Err(crate::tr!("ask.unreadable", error = "no JSON object")),
    };
    serde_json::from_str(json).map_err(|e| crate::tr!("ask.unreadable", error = e))
}

/// Holds the AI's answer to what was sent: a sentence naming no excerpt, or one that was not
/// sent, is dropped (every sentence shown can be checked against its case), and an answer left
/// without a sentence is "not found".
fn check(raw: RawAnswer, reading: &Reading) -> Answer {
    if !raw.found {
        return Answer::empty(AnswerKind::NotFound);
    }
    let mut sources: Vec<Source> = Vec::new();
    let mut parts = Vec::new();
    let mut dropped = 0;
    for sentence in raw.sentences {
        let text = clip(&without_references(&sentence.text), 600);
        if text.is_empty() {
            continue;
        }
        let found = sentence
            .excerpt
            .and_then(|id| usize::try_from(id).ok())
            .and_then(|id| reading.excerpts.get(&id));
        let Some((note_id, title, passage)) = found else {
            dropped += 1;
            continue;
        };
        let index = match sources.iter().position(|s| &s.note_id == note_id) {
            Some(i) => i,
            None => {
                sources.push(Source {
                    note_id: note_id.clone(),
                    title: title.clone(),
                    passages: Vec::new(),
                });
                sources.len() - 1
            }
        };
        let source = &mut sources[index];
        // The title is already shown: no passage repeating it.
        if passage != title
            && !source.passages.contains(passage)
            && source.passages.len() < PASSAGES_MAX
        {
            source.passages.push(passage.clone());
        }
        // Pieces follow each other in one paragraph.
        let text = if parts.is_empty() {
            text
        } else {
            format!(" {text}")
        };
        parts.push(Part {
            text,
            cite: Some(index + 1),
        });
    }
    if sources.is_empty() {
        return Answer {
            dropped,
            ..Answer::empty(AnswerKind::NotFound)
        };
    }
    let action = raw.sub_case.and_then(|sub| {
        let title = clip(&sub.title, SUB_CASE_TITLE_MAX);
        let case = reading.case_numbered(usize::try_from(sub.case).ok()?)?;
        (!title.is_empty()).then(|| Action {
            kind: "create_sub_case",
            title,
            parent_id: case.id.clone(),
            parent_title: case.title.clone(),
        })
    });
    Answer {
        kind: AnswerKind::Answer,
        parts,
        sources,
        action,
        sent: None,
        dropped,
    }
}

/// The closest cases, when no AI is connected: each with its sentence closest to the question.
fn closest(reading: &Reading, question: &str) -> Answer {
    let words: HashSet<String> = crate::germinate::words(question);
    let sources = reading
        .detailed
        .iter()
        .take(5)
        .map(|case| {
            let passage = case
                .sentences
                .iter()
                .skip(1)
                .max_by_key(|s| crate::germinate::words(s).intersection(&words).count());
            Source {
                note_id: case.id.clone(),
                title: case.title.clone(),
                passages: passage.into_iter().cloned().collect(),
            }
        })
        .collect();
    Answer {
        sources,
        ..Answer::empty(AnswerKind::NoAi)
    }
}

/// A local server Humcase knows the address of without the person setting it up: it counts as
/// ready even when nothing runs there, so it is used only when chosen.
const LOCAL_BY_DEFAULT: &str = "ollama";

/// The provider to ask: the one chosen in Settings when it is ready, else the first other ready
/// one (never the local server by default); `None` when no AI is connected.
fn ready_provider(
    providers: &[crate::providers::ProviderInfo],
    chosen: Option<&str>,
) -> Option<(String, String)> {
    chosen
        .and_then(|id| providers.iter().find(|p| p.id == id && p.ready))
        .or_else(|| {
            providers
                .iter()
                .find(|p| p.ready && p.id != LOCAL_BY_DEFAULT)
        })
        .map(|p| (p.id.clone(), p.label.clone()))
}

/// Answers `question` from the cases of `space`. The database lock is never held during the
/// call to the AI.
#[allow(clippy::too_many_arguments)]
pub async fn ask(
    db: &std::sync::Mutex<Connection>,
    db_path: &Path,
    data_dir: &Path,
    sense: Option<std::sync::Arc<crate::meaning::Sense>>,
    question: &str,
    previous: Vec<PreviousTurn>,
    space: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<Answer, String> {
    let question = clip(question, QUESTION_MAX);
    if question.is_empty() {
        return Ok(Answer::empty(AnswerKind::NotFound));
    }
    let skip = previous.len().saturating_sub(PREVIOUS_MAX);
    let previous: Vec<PreviousTurn> = previous
        .into_iter()
        .skip(skip)
        .map(|turn| PreviousTurn {
            question: clip(&turn.question, QUESTION_MAX),
            answer: clip(&turn.answer, PREVIOUS_ANSWER_MAX),
            source_ids: turn.source_ids,
        })
        .collect();
    let meaning_ids = match sense {
        Some(sense) if crate::meaning::enabled(data_dir) => {
            let dir = data_dir.to_path_buf();
            let query = match previous.last() {
                Some(last) => format!("{} {question}", last.question),
                None => question.clone(),
            };
            // The meaning worker may take a second to start: off the async threads.
            tauri::async_runtime::spawn_blocking(move || {
                crate::meaning::search(&sense, &dir, &query, 20)
                    .map(|hits| {
                        hits.into_iter()
                            .filter(|h| h.strong)
                            .map(|h| h.note_id)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .await
            .unwrap_or_default()
        }
        _ => Vec::new(),
    };
    let mut reading = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        read(&conn, &question, &previous, space.as_deref(), &meaning_ids)?
    };
    let providers = crate::providers::providers_info().cleanup;
    let Some((provider_id, label)) = ready_provider(&providers, provider.as_deref()) else {
        return Ok(closest(&reading, &question));
    };
    if reading.detailed.is_empty() && reading.overview.is_empty() {
        return Ok(Answer::empty(AnswerKind::NotFound));
    }
    let message = reading.message(&question, &previous);
    let text = crate::commands::ai::call_llm_json(
        SYSTEM_PROMPT,
        message,
        Some(provider_id),
        model,
        "answer",
        answer_schema(),
        db_path,
        crate::ai_usage::operation::ASK,
    )
    .await?;
    let mut answer = check(parse_answer(&text)?, &reading);
    answer.sent = Some(Sent {
        service: label,
        excerpts: reading.case_count(),
    });
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(id: &str, title: &str, sentences: &[&str]) -> Case {
        Case {
            id: id.into(),
            title: title.into(),
            project: "Shop".into(),
            nature: "decision".into(),
            status: "open".into(),
            decision: "none".into(),
            updated: "2026-10-08".into(),
            due: None,
            sentences: sentences.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn reading() -> Reading {
        let mut reading = Reading {
            today: "2026-10-08".into(),
            detailed: vec![
                case(
                    "a",
                    "Checkout on one page",
                    &["Checkout on one page", "Address and payment on one page."],
                ),
                case(
                    "b",
                    "Compare payment providers",
                    &[
                        "Compare payment providers",
                        "All three have no monthly fee.",
                    ],
                ),
            ],
            overview: vec![case("c", "Shop redesign", &[])],
            excerpts: HashMap::new(),
        };
        reading.message("What did we decide?", &[]);
        reading
    }

    fn raw(json: &str) -> RawAnswer {
        parse_answer(json).unwrap()
    }

    #[test]
    fn excerpts_are_numbered_across_cases_and_overview() {
        let reading = reading();
        assert_eq!(reading.excerpts.len(), 5);
        assert_eq!(reading.excerpts[&2].2, "Address and payment on one page.");
        assert_eq!(reading.excerpts[&5].0, "c");
        assert_eq!(reading.case_numbered(3).unwrap().id, "c");
        assert!(reading.case_numbered(0).is_none());
        assert!(reading.case_numbered(4).is_none());
        assert_eq!(reading.case_count(), 3);
    }

    #[test]
    fn a_source_is_the_sentence_sent_word_for_word() {
        let answer = check(
            raw(r#"{"found": true, "sentences": [
                {"text": "Checkout is on one page.", "excerpt": 2},
                {"text": "No provider charges a monthly fee.", "excerpt": 4},
                {"text": "The page also shows the address.", "excerpt": 1}
            ], "sub_case": null}"#),
            &reading(),
        );
        assert_eq!(answer.kind, AnswerKind::Answer);
        assert_eq!(answer.sources.len(), 2, "one source per case");
        assert_eq!(
            answer.sources[0].passages,
            vec!["Address and payment on one page."]
        );
        assert_eq!(answer.sources[1].note_id, "b");
        let cites: Vec<Option<usize>> = answer.parts.iter().map(|p| p.cite).collect();
        assert_eq!(cites, vec![Some(1), Some(2), Some(1)]);
        let title_only = check(
            raw(
                r#"{"found": true, "sentences": [{"text": "A redesign.", "excerpt": 5}], "sub_case": null}"#,
            ),
            &reading(),
        );
        assert!(
            title_only.sources[0].passages.is_empty(),
            "a title is not repeated as its passage"
        );
        let two = check(
            raw(r#"{"found": true, "sentences": [
                {"text": "One page.", "excerpt": 2},
                {"text": "Checkout.", "excerpt": 1},
                {"text": "No fee.", "excerpt": 4},
                {"text": "Again one page.", "excerpt": 2}
            ], "sub_case": null}"#),
            &reading(),
        );
        assert_eq!(
            two.sources[0].passages,
            vec!["Address and payment on one page."]
        );
        assert_eq!(
            two.sources[1].passages,
            vec!["All three have no monthly fee."]
        );
        assert_eq!(answer.parts[1].text, " No provider charges a monthly fee.");
    }

    // Removing this check would show the person a source the AI invented, or a sentence nothing
    // supports ("planned for today" on a case without a due day).
    #[test]
    fn a_sentence_naming_no_excerpt_or_one_not_sent_is_dropped() {
        let answer = check(
            raw(r#"{"found": true, "sentences": [
                {"text": "Stripe was chosen.", "excerpt": 99},
                {"text": "It is planned for today.", "excerpt": null},
                {"text": "Checkout is on one page.", "excerpt": 2}
            ], "sub_case": null}"#),
            &reading(),
        );
        assert_eq!(answer.dropped, 2);
        assert_eq!(answer.parts.len(), 1);
        assert_eq!(answer.parts[0].text, "Checkout is on one page.");
    }

    #[test]
    fn an_answer_without_a_cited_sentence_is_not_found() {
        let answer = check(
            raw(r#"{"found": true, "sentences": [
                {"text": "You probably chose Stripe.", "excerpt": null},
                {"text": "Made up.", "excerpt": 42}
            ], "sub_case": {"title": "Pick", "case": 1}}"#),
            &reading(),
        );
        assert_eq!(answer.kind, AnswerKind::NotFound);
        assert!(answer.parts.is_empty() && answer.action.is_none());
        let answer = check(
            raw(r#"{"found": false, "sentences": [], "sub_case": null}"#),
            &reading(),
        );
        assert_eq!(answer.kind, AnswerKind::NotFound);
    }

    #[test]
    fn a_proposed_sub_case_needs_a_case_that_was_sent() {
        let ok = check(
            raw(
                r#"{"found": true, "sentences": [{"text": "On one page.", "excerpt": 2}],
                "sub_case": {"title": "Choose the payment provider", "case": 3}}"#,
            ),
            &reading(),
        );
        let action = ok.action.unwrap();
        assert_eq!(action.parent_id, "c");
        assert_eq!(action.parent_title, "Shop redesign");
        let unknown = check(
            raw(
                r#"{"found": true, "sentences": [{"text": "On one page.", "excerpt": 2}],
                "sub_case": {"title": "Choose", "case": 9}}"#,
            ),
            &reading(),
        );
        assert!(unknown.action.is_none());
    }

    #[test]
    fn without_ai_the_closest_cases_come_with_their_closest_sentence() {
        let answer = closest(&reading(), "monthly fee");
        assert_eq!(answer.kind, AnswerKind::NoAi);
        assert!(answer.parts.is_empty() && answer.sent.is_none());
        assert_eq!(
            answer.sources.len(),
            2,
            "the detailed cases, not the overview"
        );
        assert_eq!(
            answer.sources[1].passages,
            vec!["All three have no monthly fee."]
        );
    }

    fn provider(id: &str, ready: bool) -> crate::providers::ProviderInfo {
        crate::providers::ProviderInfo {
            id: id.into(),
            label: id.to_uppercase(),
            key_env: String::new(),
            default_model: String::new(),
            url_env: String::new(),
            model_env: String::new(),
            needs_key: id != LOCAL_BY_DEFAULT,
            key_defined: ready,
            key_source: crate::ai_settings::KeyProvenance::None,
            url_editable: false,
            url: String::new(),
            ready,
        }
    }

    // Without this rule, a person with no key gets a connection error instead of the closest cases.
    #[test]
    fn no_key_means_no_ai_even_though_the_local_server_looks_ready() {
        let none = [provider("cerebras", false), provider("ollama", true)];
        assert_eq!(ready_provider(&none, Some("cerebras")), None);
        assert_eq!(
            ready_provider(&none, Some("ollama")),
            Some(("ollama".into(), "OLLAMA".into())),
            "chosen in Settings"
        );
        let groq = [
            provider("cerebras", false),
            provider("ollama", true),
            provider("groq", true),
        ];
        assert_eq!(ready_provider(&groq, Some("cerebras")).unwrap().0, "groq");
        assert_eq!(ready_provider(&groq, None).unwrap().0, "groq");
    }

    #[test]
    fn references_to_the_message_are_left_out_of_the_text() {
        assert_eq!(
            without_references("The comparison (id 4) waits for you (excerpt 11, 12)."),
            "The comparison waits for you."
        );
        assert_eq!(
            without_references("Le choix (extrait 3) reste ouvert."),
            "Le choix reste ouvert."
        );
        assert_eq!(
            without_references("Step (page 4) stays."),
            "Step (page 4) stays."
        );
    }

    #[test]
    fn the_answer_is_read_even_wrapped_in_words() {
        let raw = parse_answer(
            "Here it is: ```json\n{\"found\": false, \"sentences\": [], \"sub_case\": null}\n```",
        )
        .unwrap();
        assert!(!raw.found);
        assert!(parse_answer("no json").is_err());
    }

    #[test]
    fn search_words_leave_out_common_and_short_words() {
        let words = search_words("Qu’avait-on décidé pour le paiement de la boutique ?");
        assert!(words.contains(&"paiement".to_string()));
        assert!(words.contains(&"boutique".to_string()));
        assert!(!words.iter().any(|w| w == "pour" || w == "le" || w == "la"));
    }

    #[test]
    fn sentences_closest_to_the_question_are_kept_title_first() {
        let content = (1..=15)
            .map(|i| format!("Line number {i} about shipping."))
            .chain(["Payment happens on a single page.".to_string()])
            .collect::<Vec<_>>()
            .join("\n");
        let words = crate::germinate::words("payment page");
        let picked = pick_sentences("Checkout", &content, &words);
        let boxes = pick_sentences(
            "Shop",
            "- [ ] Payment on one page
- [x] Product photos resized",
            &words,
        );
        assert_eq!(
            boxes[1..],
            ["Payment on one page", "Product photos resized"]
        );
        assert_eq!(picked[0], "Checkout");
        assert!(picked.contains(&"Payment happens on a single page.".to_string()));
        assert!(picked.len() <= SENTENCES_PER_CASE);
    }

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        crate::db::schema::apply_migrations(&conn).unwrap();
        conn
    }

    fn project(
        conn: &Connection,
        name: &str,
        space: crate::models::project::ProjectSpace,
    ) -> String {
        queries::create_project_in_space(conn, name, None, Some(space)).unwrap()
    }

    fn add(conn: &Connection, project: &str, title: &str, content: &str) -> String {
        queries::create_note(
            conn,
            crate::models::note::CreateNoteInput {
                project_id: project.into(),
                title: title.into(),
                content: Some(content.into()),
                note_type: crate::models::note::NoteType::Note,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn reading_stays_in_the_space_and_finds_cases_by_words() {
        let conn = db();
        let work = project(&conn, "Shop", crate::models::project::ProjectSpace::Work);
        let home = project(&conn, "Home", crate::models::project::ProjectSpace::Private);
        let checkout = add(
            &conn,
            &work,
            "Checkout on one page",
            "Address, delivery and payment on one page.",
        );
        add(
            &conn,
            &work,
            "Product photos",
            "Resize every photo to 1600 px.",
        );
        let private = add(&conn, &home, "Pay the plumber", "Payment by Friday.");
        let reading = read(&conn, "What about the payment?", &[], Some("work"), &[]).unwrap();
        assert_eq!(reading.detailed[0].id, checkout);
        let all: Vec<&str> = reading
            .detailed
            .iter()
            .chain(&reading.overview)
            .map(|c| c.id.as_str())
            .collect();
        assert!(
            !all.contains(&private.as_str()),
            "another space never leaves"
        );
        assert!(
            reading.overview.iter().all(|c| c.id != checkout),
            "no case twice"
        );
        assert_eq!(reading.today.len(), 10);

        let loose = queries::create_project_in_space(&conn, "Loose", None, None).unwrap();
        let unfiled = add(&conn, &loose, "Payment terms", "Payment within 30 days.");
        let reading = read(&conn, "payment", &[], Some(UNASSIGNED), &[]).unwrap();
        let ids: Vec<&str> = reading
            .detailed
            .iter()
            .chain(&reading.overview)
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(ids, vec![unfiled.as_str()], "projects without a space only");
    }

    // Removing this rule would let a kept answer, with whatever the AI got wrong, be cited later
    // as if it were a case of the person.
    #[test]
    fn a_kept_answer_is_never_read_as_a_source() {
        let conn = db();
        let work = project(&conn, "Shop", crate::models::project::ProjectSpace::Work);
        let photos = add(
            &conn,
            &work,
            "Resize product photos",
            "Every photo at 1600 px.",
        );
        let kept = keep(
            &conn,
            crate::models::project::ProjectSpace::Work,
            "What is next for the photos?",
            "Resize product photos, planned for today. [1]",
            Some("Cerebras"),
            &[KeptSource {
                title: "Resize product photos".into(),
                passages: vec!["Every photo at 1600 px.".into()],
            }],
        )
        .unwrap();
        let provenance = crate::db::cases::list_provenance(&conn, &kept).unwrap();
        assert_eq!(provenance[0].kind, KEPT_KIND);
        assert_eq!(
            provenance[1].label.as_deref(),
            Some("Resize product photos")
        );
        let reading = read(
            &conn,
            "photos planned today",
            &[],
            Some("work"),
            std::slice::from_ref(&kept),
        )
        .unwrap();
        let ids: Vec<&str> = reading
            .detailed
            .iter()
            .chain(&reading.overview)
            .map(|c| c.id.as_str())
            .collect();
        assert!(ids.contains(&photos.as_str()));
        assert!(
            !ids.contains(&kept.as_str()),
            "not by words, meaning nor the overview"
        );
    }

    #[test]
    fn a_follow_up_is_searched_with_the_previous_question_and_cited_cases() {
        let conn = db();
        let work = project(&conn, "Shop", crate::models::project::ProjectSpace::Work);
        let providers = add(
            &conn,
            &work,
            "Compare payment providers",
            "Stripe, PayPal and Mollie compared.",
        );
        let cited = add(&conn, &work, "Unrelated wording", "Nothing in common.");
        let previous = vec![PreviousTurn {
            question: "What about payment providers?".into(),
            answer: "Three were compared.".into(),
            source_ids: vec![cited.clone()],
        }];
        let reading = read(&conn, "And why not that one?", &previous, Some("work"), &[]).unwrap();
        let ids: Vec<&str> = reading.detailed.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&providers.as_str()));
        assert!(ids.contains(&cited.as_str()));
    }
}
