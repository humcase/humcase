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
/// Provenance kinds of what Humcase wrote itself (a kept answer, a kept summary of "Think it
/// through"): never read back as a source, or a conversation would quote its own past words.
pub(crate) const OWN_KINDS: [&str; 2] = [KEPT_KIND, crate::reflect::KEPT_KIND];
const KEPT_TITLE_MAX: usize = 200;

/// Cases whose sentences are sent in full.
const DETAILED_MAX: usize = 7;
/// Sentences of one case sent, its title included.
pub(crate) const SENTENCES_PER_CASE: usize = 9;
/// Latest cases of the space sent as one line each (for "what is waiting", "this week").
const OVERVIEW_MAX: usize = 25;
/// Previous exchanges the AI reads to understand a follow-up.
const PREVIOUS_MAX: usize = 3;
pub(crate) const QUESTION_MAX: usize = 1000;
pub(crate) const PREVIOUS_ANSWER_MAX: usize = 1200;
const SUB_CASE_TITLE_MAX: usize = 120;

/// Version of the instruction below, recorded with each answer and in the bench's results.
const PROMPT_VERSION: &str = "ask.v7";
/// Version of the instruction in agent mode (the one above plus the tools' addendum).
const AGENT_PROMPT_VERSION: &str = "ask.v7+agent1";

/// Local journal of the checks: numbers only (no question, no text), next to the database.
pub const JOURNAL_FILE: &str = "ask-journal.jsonl";
pub(crate) const JOURNAL_MAX_SIZE: u64 = 1024 * 1024;

const SYSTEM_PROMPT: &str = "You answer a person's question about their own work, using only the cases (their notes, tasks, decisions) given in the user message as JSON. Each case has numbered excerpts, the first one being its title; `overview` lists the latest cases of the space in one line each (title excerpt, kind, state, last change, due day). `conversation` holds the previous exchanges: use it to understand a follow-up question.
Rules:
1. Answer in the language of the question, in one to five short sentences.
2. Give the substance: the proposal, figure, date, name or reason the cases state, not only where things stand. When several cases answer, use each of them.
3. If the cases answer only part of the question, answer that part; set `found` to false only when they answer nothing of it.
4. Keep the cases' own certainty: a proposal stays a proposal, what is still to decide stays open, a question stays a question. Never say who wrote, proposed or decided something (\"you proposed\") unless the excerpt names them.
5. Speak as the person would: name a case only by its subject (\"the choice of the supplier\"), never as \"the case\", and say its state in plain words (\"waits for your decision\", \"in progress\"); never write the words of the data (kind, state, excerpt, case number). A case without a due day has no deadline.
6. Every sentence names in `excerpt` the id of the excerpt that states that very fact; a fact read from a case's kind, state, last change or due day names that case's title excerpt. A sentence without an excerpt is not shown, so do not write one.
7. Never state anything the excerpts do not say: no outside knowledge, no guess, no advice, no reason, cause or judgment that is not written in the cases, and no link between two cases that their text does not make.
8. `sub_case`: when the question asks what is missing, what remains or what to do next, and your answer names an action that has no case of its own yet, propose that action (the person decides with one click, so propose it rather than leave it out): a short title in the language of the question and the number of the case it belongs in. Otherwise null: never for a question about facts, prices, dates or where things stand, and never for a decision the person takes on an existing case.";

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
pub(crate) const PASSAGES_MAX: usize = 3;

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
pub(crate) struct Case {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) project: String,
    pub(crate) nature: String,
    pub(crate) status: String,
    pub(crate) decision: String,
    pub(crate) updated: String,
    pub(crate) due: Option<String>,
    /// Sentences sent in full; empty for a case of the overview (its title is its excerpt).
    pub(crate) sentences: Vec<String>,
}

/// What the AI reads, with each excerpt's case and text to check the answer against.
#[derive(Debug, Default)]
pub(crate) struct Reading {
    pub(crate) today: String,
    pub(crate) detailed: Vec<Case>,
    pub(crate) overview: Vec<Case>,
    /// Cases a tool brought in after the first message (agent mode), numbered after the others.
    pub(crate) extra: Vec<Case>,
    /// Excerpt id → (case id, case title, text).
    pub(crate) excerpts: HashMap<usize, (String, String, String)>,
    /// Next excerpt id to give: tools continue the numbering of the first message.
    pub(crate) next_id: usize,
    /// Say a result waiting for the person's decision as a proposal not chosen (the thinking
    /// partner: a proposal told as "waits for the decision" was spoken of as chosen).
    pub(crate) proposals_not_chosen: bool,
}

/// How a result waiting for the person's decision is told when it must not read as chosen.
const PROPOSAL_NOT_CHOSEN: &str =
    "a proposal, not chosen: it waits for the person's decision, nothing in it is decided";

impl Reading {
    /// The state of a case in plain words, as sent.
    pub(crate) fn state_of(&self, case: &Case) -> &'static str {
        if self.proposals_not_chosen && case.decision == "to_review" {
            PROPOSAL_NOT_CHOSEN
        } else {
            state_words(&case.status, &case.decision)
        }
    }

    pub(crate) fn case_count(&self) -> usize {
        let ids: HashSet<&str> = self
            .detailed
            .iter()
            .chain(&self.overview)
            .chain(&self.extra)
            .map(|c| c.id.as_str())
            .collect();
        ids.len()
    }

    /// Gives `text`, a sentence of `case`, the next excerpt id and registers it: this is the only
    /// way an excerpt becomes citable, so `check` accepts exactly what the AI was shown.
    pub(crate) fn register(&mut self, case: &Case, text: &str) -> usize {
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        self.excerpts
            .insert(id, (case.id.clone(), case.title.clone(), text.to_string()));
        id
    }

    /// The number of the case in the conversation, adding it after the others when it is new.
    pub(crate) fn number_of(&mut self, case: &Case) -> usize {
        let known = self
            .detailed
            .iter()
            .chain(&self.overview)
            .chain(&self.extra)
            .position(|c| c.id == case.id);
        match known {
            Some(i) => i + 1,
            None => {
                self.extra.push(case.clone());
                self.detailed.len() + self.overview.len() + self.extra.len()
            }
        }
    }

    /// Numbers every excerpt: detailed cases first, then the overview's titles. A case number
    /// (`case`) is its position in `detailed` + 1, or after them for the overview.
    pub(crate) fn message(&mut self, question: &str, previous: &[PreviousTurn]) -> Value {
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
                    "kind": case.nature,
                    "state": self.state_of(case),
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
                    "kind": case.nature,
                    "state": self.state_of(case),
                    "last_change": case.updated,
                    "due": case.due,
                })
            })
            .collect();
        self.next_id = next;
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
    pub(crate) fn case_numbered(&self, number: usize) -> Option<&Case> {
        let index = number.checked_sub(1)?;
        self.detailed
            .get(index)
            .or_else(|| self.overview.get(index.checked_sub(self.detailed.len())?))
            .or_else(|| {
                self.extra
                    .get(index.checked_sub(self.detailed.len() + self.overview.len())?)
            })
    }
}

/// A reference to the message the AI sometimes leaves in its text ("(id 4)", "(excerpt 11)",
/// "(extrait 3, 5)"): meaningless to the person, the citation is shown beside the sentence.
pub(crate) fn without_references(text: &str) -> String {
    static REFERENCE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let reference = REFERENCE.get_or_init(|| {
        // A parenthesis of references: "(case 4)", "(excerpt 11, 12)", "(case 1, excerpt 3)",
        // "(case 2 excerpt 4)".
        let word = r"(?:ids?|excerpts?|extraits?|extractos?|trechos?|estratt[oi]|auszug|auszüge|cases?|cas|casos?|cas[oi]|fall|fälle)\s*[#:n°]*\s*";
        regex::Regex::new(&format!(
            r"(?i)\s*[(\[]\s*(?:{word}\d+(?:(?:\s*(?:,|;|and|et|y|e|und|du|de|of|del)\s*(?:{word})?|\s+{word})\d+)*|to_review|rework_requested|accepted_changed|in_progress)\s*[)\]]"
        ))
        .expect("valid pattern")
    });
    reference.replace_all(text, "").into_owned()
}

/// The state of a case as the person says it: the AI copied raw words ("resolved",
/// "to_review") into its answers when given the status and the decision state.
pub(crate) fn state_words(status: &str, decision: &str) -> &'static str {
    match (status, decision) {
        (_, "to_review") => "result waiting for the person's decision",
        (_, "rework_requested") => "the person asked for it to be redone",
        (_, "accepted" | "accepted_changed") => "done, accepted by the person",
        ("resolved", _) => "done",
        ("in_progress", _) => "in progress",
        _ => "to do",
    }
}

pub(crate) fn clip(text: &str, max: usize) -> String {
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
pub(crate) fn search_words(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3 && !crate::common_words::is_common(w))
        .map(str::to_lowercase)
        .filter(|w| seen.insert(w.clone()))
        .take(8)
        .collect()
}

/// A word of this length found nowhere is searched by its first `COMPOUND_START` letters.
const COMPOUND_MIN: usize = 10;
const COMPOUND_START: usize = 7;

/// Scope of the projects read: `work`, `private`, or `unassigned` for the projects without a
/// space; all of them when `None`.
pub(crate) const UNASSIGNED: &str = "unassigned";

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
        let mut page = queries::list_notes_page(conn, &filter, 0, 30, false)?;
        // A long word found nowhere may be a compound ("Beratungsstunde"): its start is searched.
        if page.notes.is_empty() && word.chars().count() >= COMPOUND_MIN {
            let start: String = word.chars().take(COMPOUND_START).collect();
            let filter = NoteListFilter {
                search_query: Some(start),
                ..filter
            };
            page = queries::list_notes_page(conn, &filter, 0, 30, false)?;
        }
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

/// The sentences of a case most related to the question, in their order, its title first; at
/// most `max`, the title included.
pub(crate) fn pick_sentences(
    title: &str,
    content: &str,
    question_words: &HashSet<String>,
    max: usize,
) -> Vec<String> {
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
        .take(max.saturating_sub(1))
        .map(|(i, _)| i)
        .collect();
    keep.sort_unstable();
    std::iter::once(title)
        .chain(keep.into_iter().map(|i| rest[i].clone()))
        .collect()
}

fn is_kept_answer(conn: &Connection, note_id: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM case_provenance WHERE note_id = ?1 AND kind IN (?2, ?3))",
        params![note_id, OWN_KINDS[0], OWN_KINDS[1]],
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

pub(crate) fn project_names(conn: &Connection) -> Result<HashMap<String, String>, String> {
    Ok(queries::get_projects(conn)
        .map_err(crate::db::db_error)?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect())
}

/// Whether the project belongs to the scope read (`space`, see [`UNASSIGNED`]).
fn in_space(conn: &Connection, space: Option<&str>, project_id: &str) -> Result<bool, String> {
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
}

/// A case with its sentences closest to the question, or `None` when it may never be read as a
/// source: deleted, archived, a kept answer, a sketch, or in another space. Every path that
/// brings a case to the AI (the first reading and each tool) goes through here.
pub(crate) fn load_case(
    conn: &Connection,
    id: &str,
    space: Option<&str>,
    projects: &HashMap<String, String>,
    question_words: &HashSet<String>,
    max_sentences: usize,
) -> Result<Option<Case>, String> {
    let Ok(note) = queries::get_note_by_id(conn, id) else {
        return Ok(None);
    };
    if note.deleted_at.is_some()
        || note.archived_at.is_some()
        || is_kept_answer(conn, &note.id)?
        || note.content_kind == crate::models::note::ContentKind::Sketch
        || !in_space(conn, space, &note.project_id)?
    {
        return Ok(None);
    }
    let status = note.status.as_str().to_string();
    Ok(Some(Case {
        decision: decision_of(conn, &note.id, &status),
        project: projects.get(&note.project_id).cloned().unwrap_or_default(),
        nature: note.note_type.as_str().to_string(),
        status,
        updated: note.updated_at.chars().take(10).collect(),
        due: note.due_at.clone(),
        sentences: pick_sentences(&note.title, &note.content, question_words, max_sentences),
        id: note.id,
        title: note.title,
    }))
}

/// The latest `limit` cases of the space as one line each (no sentences), a kept answer, an
/// archived case or a sketch never among them.
fn latest_cases(
    conn: &Connection,
    space: Option<&str>,
    limit: usize,
    projects: &HashMap<String, String>,
) -> Result<Vec<Case>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT n.id, n.title, n.note_type, n.status, n.updated_at, n.due_at, n.project_id
               FROM notes n JOIN projects p ON p.id = n.project_id
              WHERE n.deleted_at IS NULL AND n.archived_at IS NULL
                AND n.content_kind != 'sketch'
                AND NOT EXISTS (SELECT 1 FROM case_provenance cp
                                 WHERE cp.note_id = n.id AND cp.kind IN ('ask', 'reflect'))
                AND (?1 IS NULL OR p.space = ?1 OR (?1 = 'unassigned' AND p.space IS NULL))
              ORDER BY n.updated_at DESC LIMIT ?2",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map(params![space, limit as i64], |r| {
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
    let mut cases = Vec::new();
    for row in rows {
        let (id, title, nature, status, updated, due, project_id) =
            row.map_err(crate::db::db_error)?;
        cases.push(Case {
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
    Ok(cases)
}

/// Reads the cases the AI will answer from. `meaning_ids`: cases found by meaning, best first.
pub(crate) fn read(
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
    let question_words = words_of(&query);
    let mut detailed = Vec::new();
    for id in ids {
        if detailed.len() >= DETAILED_MAX {
            break;
        }
        if let Some(case) = load_case(
            conn,
            &id,
            space,
            &projects,
            &question_words,
            SENTENCES_PER_CASE,
        )? {
            detailed.push(case);
        }
    }
    let shown: HashSet<String> = detailed.iter().map(|c| c.id.clone()).collect();
    let overview = latest_cases(conn, space, OVERVIEW_MAX + DETAILED_MAX, &projects)?
        .into_iter()
        .filter(|c| !shown.contains(&c.id))
        .take(OVERVIEW_MAX)
        .collect();
    Ok(Reading {
        today,
        detailed,
        overview,
        ..Reading::default()
    })
}

/// The words of a text that tell its sentences apart (common words left out).
pub(crate) fn words_of(text: &str) -> HashSet<String> {
    crate::germinate::words(text)
        .into_iter()
        .filter(|w| !crate::common_words::is_common(w))
        .collect()
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
pub(crate) fn ready_provider(
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

/// Records what the check kept of an answer, numbers only: how many sentences the AI wrote and
/// how many were dropped tell whether it invents (the person reads the file; nothing leaves).
fn record_check(db_path: &Path, answer: &Answer, cases_sent: usize, calls: usize) {
    let Some(folder) = db_path.parent() else {
        return;
    };
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let entry = json!({
        "at_unix_ms": millis,
        "prompt": if calls > 1 { AGENT_PROMPT_VERSION } else { PROMPT_VERSION },
        "calls": calls,
        "kind": answer.kind,
        "sentences": answer.parts.len(),
        "dropped": answer.dropped,
        "sources": answer.sources.len(),
        "passages": answer.sources.iter().map(|s| s.passages.len()).sum::<usize>(),
        "cases_sent": cases_sent,
        "sub_case": answer.action.is_some(),
    });
    if let Err(e) =
        crate::dictation_journal::append(&folder.join(JOURNAL_FILE), &entry, JOURNAL_MAX_SIZE)
    {
        eprintln!("ask check not recorded ({e})");
    }
}

/// Model calls an agent-mode question may take, the last one forced to `answer`.
const AGENT_CALLS_MAX: usize = 5;
/// Cases `search_cases` returns, and lines `list_cases` returns.
const AGENT_SEARCH_MAX: usize = 5;
const AGENT_LIST_MAX: usize = 15;
/// Sentences of a case `read_case` returns (the title included): all a case offers.
pub(crate) const AGENT_READ_SENTENCES: usize = 40;

/// What the instruction says of the tools, after the rules above.
const AGENT_ADDENDUM: &str = "
The cases in the user message are only those whose words are closest to the question and the latest ones: the space holds more. You have tools to look for the others:
- search_cases(query): the cases holding the words of `query`, with numbered excerpts. A case only matches the words it contains.
- read_case(case): all the excerpts of one case, by its number.
- list_cases(state): the latest cases that are open, in_progress or waiting_for_decision, one line each.
When the given cases do not answer the question, or answer only part of it, search before concluding: use the words a case about that thing would contain (a synonym, a related word, the name of a document, the unit of a figure), not the question's own words, and search again with other words when a search finds nothing useful; give up only after two searches with different words found nothing. Do not search for what the given cases already answer. Excerpt ids and case numbers from the tools work exactly like those in the user message, and rules 1 to 8 apply to them. To reply, call the function `answer`: it is the only way to finish, and you have only a few turns.";

pub(crate) fn agent_tool(name: &str, description: &str, parameters: Value) -> Value {
    json!({ "type": "function", "function": {
        "name": name, "description": description, "parameters": parameters } })
}

/// The read-only tools and `answer`, whose parameters are the answer the AI is held to.
fn agent_tools() -> Value {
    agent_tools_with(agent_tool(
        "answer",
        "Give the final answer.",
        answer_schema(),
    ))
}

/// The read-only tools and `final_tool`, the function that ends the conversation.
pub(crate) fn agent_tools_with(final_tool: Value) -> Value {
    json!([
        agent_tool(
            "search_cases",
            "Find the cases of the space holding the words of the query.",
            json!({ "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"], "additionalProperties": false }),
        ),
        agent_tool(
            "read_case",
            "Read all the excerpts of the case with this number.",
            json!({ "type": "object",
                "properties": { "case": { "type": "integer" } },
                "required": ["case"], "additionalProperties": false }),
        ),
        agent_tool(
            "list_cases",
            "List the latest cases in a state.",
            json!({ "type": "object",
                "properties": { "state": { "type": "string",
                    "enum": ["open", "in_progress", "waiting_for_decision"] } },
                "required": ["state"], "additionalProperties": false }),
        ),
        final_tool,
    ])
}

pub(crate) fn named_tool_choice(name: &str) -> Value {
    json!({ "type": "function", "function": { "name": name } })
}

/// A case as a tool returns it: its number, its facts and its excerpts, each with a new id that
/// is registered so the answer may cite it (and nothing else).
fn case_for_tool(reading: &mut Reading, case: &Case) -> Value {
    let number = reading.number_of(case);
    let excerpts: Vec<Value> = case
        .sentences
        .iter()
        .map(|s| json!({ "id": reading.register(case, s), "text": s }))
        .collect();
    json!({
        "case": number,
        "project": case.project,
        "kind": case.nature,
        "state": reading.state_of(case),
        "last_change": case.updated,
        "due": case.due,
        "excerpts": excerpts,
    })
}

/// Runs one read-only tool and returns what the AI reads. Same rules as the first reading:
/// the space, no archived case, no kept answer, no sketch.
pub(crate) fn run_tool(
    conn: &Connection,
    space: Option<&str>,
    question_words: &HashSet<String>,
    reading: &mut Reading,
    name: &str,
    args: &Value,
) -> Result<Value, String> {
    let projects = project_names(conn)?;
    match name {
        "search_cases" => {
            let query = args["query"].as_str().unwrap_or_default();
            let mut words = question_words.clone();
            words.extend(words_of(query));
            let mut cases = Vec::new();
            for id in by_words(conn, query, space)? {
                if cases.len() >= AGENT_SEARCH_MAX {
                    break;
                }
                if let Some(case) =
                    load_case(conn, &id, space, &projects, &words, SENTENCES_PER_CASE)?
                {
                    cases.push(case_for_tool(reading, &case));
                }
            }
            if cases.is_empty() {
                return Ok(
                    json!({ "cases": [], "note": "no case holds these words; try other words" }),
                );
            }
            Ok(json!({ "cases": cases }))
        }
        "read_case" => {
            let Some(case) = args["case"]
                .as_u64()
                .and_then(|n| reading.case_numbered(n as usize))
                .map(|c| c.id.clone())
            else {
                return Ok(json!({ "error": "no case has this number" }));
            };
            match load_case(
                conn,
                &case,
                space,
                &projects,
                question_words,
                AGENT_READ_SENTENCES,
            )? {
                Some(case) => Ok(case_for_tool(reading, &case)),
                None => Ok(json!({ "error": "this case cannot be read" })),
            }
        }
        "list_cases" => {
            let wanted = match args["state"].as_str() {
                Some("open") => "to do",
                Some("in_progress") => "in progress",
                Some("waiting_for_decision") => "result waiting for the person's decision",
                _ => {
                    return Ok(
                        json!({ "error": "state must be open, in_progress or waiting_for_decision" }),
                    )
                }
            };
            let mut lines = Vec::new();
            for mut case in latest_cases(conn, space, 200, &projects)? {
                if lines.len() >= AGENT_LIST_MAX {
                    break;
                }
                if state_words(&case.status, &case.decision) == wanted {
                    case.sentences = vec![case.title.clone()];
                    lines.push(case_for_tool(reading, &case));
                }
            }
            Ok(json!({ "cases": lines }))
        }
        _ => Ok(json!({ "error": format!("unknown tool {name}") })),
    }
}

/// Why the agent loop gave nothing: the provider refuses tools (the single call is the
/// fallback), or the loop failed.
#[derive(Debug)]
pub(crate) enum AgentFailure {
    ToolsRefused,
    Failed(String),
}

#[derive(Debug)]
pub(crate) struct AgentOutcome<T> {
    pub(crate) raw: T,
    /// Model calls taken, 1 when the first reading was enough.
    pub(crate) calls: usize,
    /// The tools the model used, with their arguments (the bench prints them).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) tools: Vec<String>,
}

/// The assistant's message as sent back to the model.
pub(crate) fn assistant_turn(reply: &Value) -> Value {
    let mut turn = reply.clone();
    turn["role"] = json!("assistant");
    turn
}

/// Answers by a loop: the first message is the same reading as the single call, the model may
/// search or read more cases (every excerpt a tool returns is registered in `reading`), and
/// finishes by calling `answer`; at most [`AGENT_CALLS_MAX`] model calls, the last forced to
/// `answer`. `call_model` sends the messages and the `tool_choice` and returns the assistant's
/// message. The database lock is held only while a tool runs.
#[allow(clippy::too_many_arguments)]
async fn run_agent<F, Fut>(
    db: &std::sync::Mutex<Connection>,
    space: Option<&str>,
    question: &str,
    previous: &[PreviousTurn],
    reading: &mut Reading,
    system: &str,
    first: Value,
    call_model: F,
) -> Result<AgentOutcome<RawAnswer>, AgentFailure>
where
    F: FnMut(Vec<Value>, Value) -> Fut,
    Fut: std::future::Future<Output = Result<Value, (String, Option<u16>)>>,
{
    let mut query = question.to_string();
    if let Some(last) = previous.last() {
        query.push(' ');
        query.push_str(&last.question);
    }
    run_agent_with(
        db,
        space,
        &query,
        reading,
        system,
        first,
        "answer",
        AGENT_CALLS_MAX,
        |args| serde_json::from_value::<RawAnswer>(args).map_err(|e| e.to_string()),
        call_model,
    )
    .await
}

/// The loop of [`run_agent`] for any final answer: the model finishes by calling the function
/// `final_tool`, whose arguments `parse` reads (the same reader is tried on a JSON object
/// written as plain text); at most `max_calls` model calls, the last forced to `final_tool`.
/// `query` is what the question words of the search tools are taken from.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_agent_with<T, F, Fut>(
    db: &std::sync::Mutex<Connection>,
    space: Option<&str>,
    query: &str,
    reading: &mut Reading,
    system: &str,
    first: Value,
    final_tool: &str,
    max_calls: usize,
    parse: impl Fn(Value) -> Result<T, String>,
    mut call_model: F,
) -> Result<AgentOutcome<T>, AgentFailure>
where
    F: FnMut(Vec<Value>, Value) -> Fut,
    Fut: std::future::Future<Output = Result<Value, (String, Option<u16>)>>,
{
    let question_words = words_of(query);
    let mut tools_used = Vec::new();
    let mut messages = vec![
        json!({ "role": "system", "content": system }),
        json!({ "role": "user", "content": first.to_string() }),
    ];
    for call in 1..=max_calls {
        let choice = if call == max_calls {
            named_tool_choice(final_tool)
        } else {
            json!("auto")
        };
        let reply = match call_model(messages.clone(), choice).await {
            Ok(reply) => reply,
            Err((_, Some(400 | 422))) if call == 1 => return Err(AgentFailure::ToolsRefused),
            Err((error, _)) => return Err(AgentFailure::Failed(error)),
        };
        let tool_calls = reply["tool_calls"].as_array().cloned().unwrap_or_default();
        if tool_calls.is_empty() {
            // The model wrote its answer as text instead of calling the final function.
            let text = crate::providers::content_text(&reply["content"]);
            if let Some(Ok(raw)) = json_object_in(&text).map(&parse) {
                return Ok(AgentOutcome {
                    raw,
                    calls: call,
                    tools: tools_used,
                });
            }
            messages.push(assistant_turn(&reply));
            messages.push(json!({ "role": "user",
                "content": format!("Reply by calling the function `{final_tool}`.") }));
            continue;
        }
        messages.push(assistant_turn(&reply));
        for tool_call in tool_calls {
            let name = tool_call["function"]["name"].as_str().unwrap_or_default();
            let args: Value = tool_call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or(Value::Null);
            let content = if name == final_tool {
                match parse(args) {
                    Ok(raw) => {
                        return Ok(AgentOutcome {
                            raw,
                            calls: call,
                            tools: tools_used,
                        })
                    }
                    Err(e) => json!({ "error": format!("unreadable answer: {e}") }),
                }
            } else {
                tools_used.push(format!("{name} {args}"));
                let conn = db.lock().map_err(|e| AgentFailure::Failed(e.to_string()))?;
                run_tool(&conn, space, &question_words, reading, name, &args)
                    .map_err(AgentFailure::Failed)?
            };
            messages.push(json!({
                "role": "tool",
                "tool_call_id": tool_call["id"],
                "content": content.to_string(),
            }));
        }
    }
    Err(AgentFailure::Failed(crate::tr!(
        "ask.unreadable",
        error = "no answer after the tools"
    )))
}

/// The JSON object a text holds, even wrapped in words or a code fence.
pub(crate) fn json_object_in(text: &str) -> Option<Value> {
    let (start, end) = (text.find('{')?, text.rfind('}')?);
    if end <= start {
        return None;
    }
    serde_json::from_str(&text[start..=end]).ok()
}

/// One turn of the conversation with the AI chosen in Settings.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn chat_turn(
    messages: Vec<Value>,
    tool_choice: Value,
    tools: &Value,
    provider: String,
    model: Option<String>,
    db_path: &Path,
    operation: &str,
    reasoning_effort: Option<&str>,
) -> Result<Value, (String, Option<u16>)> {
    crate::commands::ai::call_llm_chat(
        &messages,
        tools,
        &tool_choice,
        Some(provider),
        model,
        db_path,
        operation,
        reasoning_effort,
    )
    .await
}

/// Cases found by meaning for `query`, best first (strong hits only); empty when the search by
/// meaning is off or unavailable.
pub(crate) async fn meaning_hits(
    sense: Option<std::sync::Arc<crate::meaning::Sense>>,
    data_dir: &Path,
    query: String,
) -> Vec<String> {
    match sense {
        Some(sense) if crate::meaning::enabled(data_dir) => {
            let dir = data_dir.to_path_buf();
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
    }
}

/// How a question is answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskMode {
    /// One call on the first reading.
    Single,
    /// The AI may search and read more cases before answering (see [`run_agent`]).
    Agent,
    /// One call when the first reading is strong, the agent loop when it is weak
    /// ([`reading_is_strong`]): the tools cost twice the tokens and time, so they are offered
    /// only when the first reading probably misses the case.
    Auto,
}

/// The `words` that `case` holds in its title or its picked sentences (a long word also by its
/// start, as the search does).
fn words_held<'a>(case: &Case, words: &'a [String]) -> Vec<&'a String> {
    let text = case.sentences.join(" ").to_lowercase();
    words
        .iter()
        .filter(|w| {
            text.contains(w.as_str())
                || (w.chars().count() >= COMPOUND_MIN
                    && text.contains(&w.chars().take(COMPOUND_START).collect::<String>()))
        })
        .collect()
}

/// A word this long is specific enough to be a hit by itself ("furgone", "hosting").
const SPECIFIC_WORD: usize = 6;

/// Whether the first reading probably holds what the question asks, decided without an AI
/// call. Strong: a detailed case found by meaning, or one holding two of the question's
/// significant words in its title or sentences, or one holding a single specific (6 letters or
/// more) word, or the only word there is. Weak: the cases found share nothing specific with
/// the question, so the case it asks for is probably not among them. (More than half of the
/// words was tried first and was too strict: the stop-word list leaves "what", "combien" or
/// "quelle" in, and the questions about state — "what is waiting?" — have words no case holds.)
/// A question with no significant word has nothing to search with: the reading is all there
/// is. `query` is the question with the one before it, as `read` searched.
pub(crate) fn reading_is_strong(reading: &Reading, query: &str, meaning_ids: &[String]) -> bool {
    if reading.detailed.iter().any(|c| meaning_ids.contains(&c.id)) {
        return true;
    }
    let words = search_words(query);
    if words.is_empty() {
        return true;
    }
    reading.detailed.iter().any(|case| {
        let held = words_held(case, &words);
        held.len() >= 2
            || (held.len() == 1 && (words.len() == 1 || held[0].chars().count() >= SPECIFIC_WORD))
    })
}

/// Answers `question` from the cases of `space`. The database lock is never held during the
/// call to the AI. `mode`: see [`AskMode`]; the single call stays the fallback when the provider
/// refuses tools.
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
    mode: AskMode,
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
    let meaning_query = match previous.last() {
        Some(last) => format!("{} {question}", last.question),
        None => question.clone(),
    };
    let meaning_ids = meaning_hits(sense, data_dir, meaning_query).await;
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
    let agent = match mode {
        AskMode::Single => false,
        AskMode::Agent => true,
        AskMode::Auto => {
            let mut query = question.clone();
            if let Some(last) = previous.last() {
                query.push(' ');
                query.push_str(&last.question);
            }
            !reading_is_strong(&reading, &query, &meaning_ids)
        }
    };
    let message = reading.message(&question, &previous);
    let mut outcome = None;
    if agent {
        let system = format!("{SYSTEM_PROMPT}{AGENT_ADDENDUM}");
        let tools = agent_tools();
        match run_agent(
            db,
            space.as_deref(),
            &question,
            &previous,
            &mut reading,
            &system,
            message.clone(),
            |messages, choice| {
                chat_turn(
                    messages,
                    choice,
                    &tools,
                    provider_id.clone(),
                    model.clone(),
                    db_path,
                    crate::ai_usage::operation::ASK,
                    None,
                )
            },
        )
        .await
        {
            Ok(done) => outcome = Some(done),
            Err(AgentFailure::ToolsRefused) => {}
            Err(AgentFailure::Failed(error)) => return Err(error),
        }
    }
    let (raw, calls) = match outcome {
        Some(done) => (done.raw, done.calls),
        None => {
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
            (parse_answer(&text)?, 1)
        }
    };
    let mut answer = check(raw, &reading);
    answer.sent = Some(Sent {
        service: label,
        excerpts: reading.case_count(),
    });
    record_check(db_path, &answer, reading.case_count(), calls);
    Ok(answer)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn case(id: &str, title: &str, sentences: &[&str]) -> Case {
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
            ..Reading::default()
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
    fn the_state_is_sent_in_plain_words() {
        assert_eq!(
            state_words("resolved", "to_review"),
            "result waiting for the person's decision"
        );
        assert_eq!(state_words("resolved", "none"), "done");
        assert_eq!(
            state_words("open", "rework_requested"),
            "the person asked for it to be redone"
        );
        assert_eq!(state_words("in_progress", "none"), "in progress");
        assert_eq!(state_words("open", "none"), "to do");
        let message = reading().message("What is open?", &[]);
        assert!(
            !message.to_string().contains("\"status\""),
            "no raw status sent"
        );
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
            without_references("The renewal (case 1, excerpt 3) comes first; (cas 2 et 3) too."),
            "The renewal comes first; too."
        );
        assert_eq!(
            without_references(
                "Since (case 2 excerpt 4), the bathroom is closed (excerpt 3, case 4)."
            ),
            "Since, the bathroom is closed."
        );
        assert_eq!(
            without_references("Entre Ceradel (extrait 2 du cas 1) et Solargil (case 3 of 4)."),
            "Entre Ceradel et Solargil."
        );
        assert_eq!(
            without_references("Step (page 4) stays."),
            "Step (page 4) stays."
        );
        assert_eq!(
            without_references("It awaits your decision (to_review)."),
            "It awaits your decision."
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
        let picked = pick_sentences("Checkout", &content, &words, SENTENCES_PER_CASE);
        let boxes = pick_sentences(
            "Shop",
            "- [ ] Payment on one page
- [x] Product photos resized",
            &words,
            SENTENCES_PER_CASE,
        );
        assert_eq!(
            boxes[1..],
            ["Payment on one page", "Product photos resized"]
        );
        assert_eq!(picked[0], "Checkout");
        assert!(picked.contains(&"Payment happens on a single page.".to_string()));
        assert!(picked.len() <= SENTENCES_PER_CASE);
    }

    pub(crate) fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        crate::db::schema::apply_migrations(&conn).unwrap();
        conn
    }

    pub(crate) fn project(
        conn: &Connection,
        name: &str,
        space: crate::models::project::ProjectSpace,
    ) -> String {
        queries::create_project_in_space(conn, name, None, Some(space)).unwrap()
    }

    pub(crate) fn add(conn: &Connection, project: &str, title: &str, content: &str) -> String {
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
    fn a_compound_word_finds_the_case_holding_its_start() {
        let conn = db();
        let work = project(&conn, "Office", crate::models::project::ProjectSpace::Work);
        let fee = add(&conn, &work, "Stundensatz für Beratung", "Ab Februar 95 €.");
        let found = by_words(&conn, "Was kostet eine Beratungsstunde?", Some("work")).unwrap();
        assert_eq!(found, vec![fee]);
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

    #[test]
    fn a_check_is_journaled_as_numbers_only() {
        let folder = tempfile::tempdir().unwrap();
        let db_path = folder.path().join("humcase.db");
        let answer = check(
            raw(r#"{"found": true, "sentences": [
                {"text": "Address and payment on one page.", "excerpt": 2},
                {"text": "Invented.", "excerpt": null}
            ], "sub_case": null}"#),
            &reading(),
        );
        record_check(&db_path, &answer, 3, 1);
        let line = std::fs::read_to_string(folder.path().join(JOURNAL_FILE)).unwrap();
        let entry: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(entry["sentences"], 1);
        assert_eq!(entry["dropped"], 1);
        assert_eq!(entry["cases_sent"], 3);
        assert!(
            !line.contains("Address"),
            "no text of the cases nor of the answer"
        );
    }

    // -- Agent mode ---------------------------------------------------------------------------

    pub(crate) fn tool_call(id: &str, name: &str, args: Value) -> Value {
        json!({ "id": id, "type": "function",
            "function": { "name": name, "arguments": args.to_string() } })
    }

    pub(crate) fn tool_reply(calls: Vec<Value>) -> Value {
        json!({ "role": "assistant", "content": null, "tool_calls": calls })
    }

    fn empty_reading() -> Reading {
        let mut reading = Reading {
            today: "2026-10-08".into(),
            ..Reading::default()
        };
        reading.message("Question?", &[]);
        reading
    }

    /// The last message of the conversation, which a tool answered, as JSON.
    fn last_tool_result(messages: &[Value]) -> Value {
        let last = messages.last().unwrap();
        assert_eq!(last["role"], "tool");
        serde_json::from_str(last["content"].as_str().unwrap()).unwrap()
    }

    // Removing the registration in `case_for_tool` makes the cited excerpt unknown to `check`;
    // removing `check` shows a source nothing supports.
    #[test]
    fn an_excerpt_a_tool_returned_is_accepted_and_one_never_returned_is_dropped() {
        let conn = db();
        let work = project(&conn, "Shop", crate::models::project::ProjectSpace::Work);
        let contract = add(
            &conn,
            &work,
            "Contrat RC Pro",
            "Prime annuelle 380 euros. Renouvellement en mars.",
        );
        let db = std::sync::Mutex::new(conn);
        let mut reading = empty_reading();
        let mut turn = 0;
        let outcome = tauri::async_runtime::block_on(run_agent(
            &db,
            Some("work"),
            "What do I pay for insurance?",
            &[],
            &mut reading,
            "system",
            json!({}),
            |messages, _choice| {
                turn += 1;
                let reply = if turn == 1 {
                    tool_reply(vec![tool_call(
                        "t1",
                        "search_cases",
                        json!({ "query": "prime" }),
                    )])
                } else {
                    let found = last_tool_result(&messages);
                    let case = &found["cases"][0];
                    let seen = case["excerpts"][1]["id"].clone();
                    let answer = json!({ "found": true, "sub_case": {
                            "title": "Renew the contract", "case": case["case"] },
                        "sentences": [
                            { "text": "It costs 380 euros a year.", "excerpt": seen },
                            { "text": "It is paid in advance.", "excerpt": 9999 }] });
                    tool_reply(vec![tool_call("t2", "answer", answer)])
                };
                async move { Ok(reply) }
            },
        ))
        .unwrap();
        assert_eq!(outcome.calls, 2);
        assert_eq!(outcome.tools.len(), 1);
        let answer = check(outcome.raw, &reading);
        assert_eq!(answer.kind, AnswerKind::Answer);
        assert_eq!(answer.parts.len(), 1, "the excerpt the tool returned");
        assert_eq!(answer.dropped, 1, "the id no tool returned");
        assert_eq!(answer.sources[0].note_id, contract);
        assert_eq!(
            answer.sources[0].passages,
            vec!["Prime annuelle 380 euros."]
        );
        let action = answer.action.expect("a case a tool brought has a number");
        assert_eq!(action.parent_id, contract);
        assert_eq!(reading.case_count(), 1);
    }

    // Removing `load_case`'s checks would let the tools hand the AI what the first reading
    // never shows: another space, an archived case, a kept answer.
    #[test]
    fn the_tools_never_return_a_kept_answer_an_archived_case_or_another_space() {
        let conn = db();
        let work = project(&conn, "Shop", crate::models::project::ProjectSpace::Work);
        let home = project(&conn, "Home", crate::models::project::ProjectSpace::Private);
        let visible = add(&conn, &work, "Visible contract", "The contract is signed.");
        let archived = add(&conn, &work, "Archived contract", "An old contract.");
        conn.execute(
            "UPDATE notes SET archived_at = '2026-10-05T00:00:00Z' WHERE id = ?1",
            [&archived],
        )
        .unwrap();
        add(
            &conn,
            &home,
            "Private contract",
            "Contract with the plumber.",
        );
        keep(
            &conn,
            crate::models::project::ProjectSpace::Work,
            "What about the contract?",
            "The contract is signed. [1]",
            None,
            &[],
        )
        .unwrap();
        let mut reading = empty_reading();
        let found = run_tool(
            &conn,
            Some("work"),
            &HashSet::new(),
            &mut reading,
            "search_cases",
            &json!({ "query": "contract" }),
        )
        .unwrap();
        let titles: Vec<&str> = found["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["excerpts"][0]["text"].as_str().unwrap())
            .collect();
        assert_eq!(titles, vec!["Visible contract"]);
        assert!(reading.excerpts.values().all(|(id, _, _)| *id == visible));
        // The latest cases and a case read by number follow the same rules.
        let listed = run_tool(
            &conn,
            Some("work"),
            &HashSet::new(),
            &mut reading,
            "list_cases",
            &json!({ "state": "open" }),
        )
        .unwrap();
        assert_eq!(listed["cases"].as_array().unwrap().len(), 1);
        let none = run_tool(
            &conn,
            Some("work"),
            &HashSet::new(),
            &mut reading,
            "search_cases",
            &json!({ "query": "plumber" }),
        )
        .unwrap();
        assert!(none["cases"].as_array().unwrap().is_empty());
        assert!(none["note"].is_string());
    }

    #[test]
    fn read_case_gives_the_whole_case_and_list_cases_the_state_asked() {
        let conn = db();
        let work = project(&conn, "Shop", crate::models::project::ProjectSpace::Work);
        let long = (1..=30)
            .map(|i| format!("Line number {i} about the new shipping rules."))
            .collect::<Vec<_>>()
            .join("\n");
        let shipping = add(&conn, &work, "Shipping rules", &long);
        let busy = add(&conn, &work, "Busy case", "Something under way.");
        conn.execute(
            "UPDATE notes SET status = 'in_progress' WHERE id = ?1",
            [&busy],
        )
        .unwrap();
        let mut reading = empty_reading();
        let list = run_tool(
            &conn,
            Some("work"),
            &HashSet::new(),
            &mut reading,
            "list_cases",
            &json!({ "state": "in_progress" }),
        )
        .unwrap();
        assert_eq!(list["cases"].as_array().unwrap().len(), 1);
        assert_eq!(list["cases"][0]["excerpts"].as_array().unwrap().len(), 1);
        let all = run_tool(
            &conn,
            Some("work"),
            &HashSet::new(),
            &mut reading,
            "search_cases",
            &json!({ "query": "shipping" }),
        )
        .unwrap();
        assert_eq!(
            all["cases"][0]["excerpts"].as_array().unwrap().len(),
            SENTENCES_PER_CASE
        );
        let number = all["cases"][0]["case"].as_u64().unwrap();
        let whole = run_tool(
            &conn,
            Some("work"),
            &HashSet::new(),
            &mut reading,
            "read_case",
            &json!({ "case": number }),
        )
        .unwrap();
        assert_eq!(whole["case"], number, "same case, same number");
        assert_eq!(whole["excerpts"].as_array().unwrap().len(), 31);
        let ids: HashSet<u64> = whole["excerpts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect();
        assert_eq!(ids.len(), 31, "new ids, none reused");
        assert!(reading.excerpts.values().any(|(id, _, _)| *id == shipping));
        let missing = run_tool(
            &conn,
            Some("work"),
            &HashSet::new(),
            &mut reading,
            "read_case",
            &json!({ "case": 99 }),
        )
        .unwrap();
        assert!(missing["error"].is_string());
    }

    #[test]
    fn the_loop_stops_at_five_calls_and_the_last_one_must_answer() {
        let db = std::sync::Mutex::new(db());
        let choices: std::sync::Arc<std::sync::Mutex<Vec<Value>>> = Default::default();
        // A model that never answers.
        let mut reading = empty_reading();
        let log = choices.clone();
        let outcome = tauri::async_runtime::block_on(run_agent(
            &db,
            Some("work"),
            "Anything?",
            &[],
            &mut reading,
            "system",
            json!({}),
            |_messages, choice| {
                log.lock().unwrap().push(choice);
                let reply = tool_reply(vec![tool_call(
                    "t",
                    "search_cases",
                    json!({ "query": "anything" }),
                )]);
                async move { Ok(reply) }
            },
        ));
        assert!(matches!(outcome, Err(AgentFailure::Failed(_))));
        let seen = choices.lock().unwrap().clone();
        assert_eq!(seen.len(), AGENT_CALLS_MAX, "never a sixth call");
        assert!(seen[..4].iter().all(|c| c == "auto"));
        assert_eq!(
            seen[4]["function"]["name"], "answer",
            "the last one is forced"
        );
        // A model that answers only when forced.
        choices.lock().unwrap().clear();
        let log = choices.clone();
        let outcome = tauri::async_runtime::block_on(run_agent(
            &db,
            Some("work"),
            "Anything?",
            &[],
            &mut reading,
            "system",
            json!({}),
            |_messages, choice| {
                let forced = choice["function"]["name"] == "answer";
                log.lock().unwrap().push(choice);
                let reply = if forced {
                    tool_reply(vec![tool_call(
                        "t",
                        "answer",
                        json!({ "found": false, "sentences": [], "sub_case": null }),
                    )])
                } else {
                    tool_reply(vec![tool_call(
                        "t",
                        "list_cases",
                        json!({ "state": "open" }),
                    )])
                };
                async move { Ok(reply) }
            },
        ))
        .unwrap();
        assert_eq!(outcome.calls, AGENT_CALLS_MAX);
        assert!(!outcome.raw.found);
    }

    #[test]
    fn a_plain_text_answer_is_read_and_a_provider_refusing_tools_falls_back() {
        let db = std::sync::Mutex::new(db());
        let mut reading = empty_reading();
        let outcome = tauri::async_runtime::block_on(run_agent(
            &db,
            None,
            "Anything?",
            &[],
            &mut reading,
            "system",
            json!({}),
            |_messages, _choice| async {
                Ok(json!({ "role": "assistant", "content":
                    "{\"found\": false, \"sentences\": [], \"sub_case\": null}" }))
            },
        ))
        .unwrap();
        assert_eq!(outcome.calls, 1);
        let refused = tauri::async_runtime::block_on(run_agent(
            &db,
            None,
            "Anything?",
            &[],
            &mut reading,
            "system",
            json!({}),
            |_messages, _choice| async { Err(("refused".to_string(), Some(400))) },
        ));
        assert!(matches!(refused, Err(AgentFailure::ToolsRefused)));
        // Later failures are errors, not a silent fallback.
        let mut turn = 0;
        let later = tauri::async_runtime::block_on(run_agent(
            &db,
            None,
            "Anything?",
            &[],
            &mut reading,
            "system",
            json!({}),
            |_messages, _choice| {
                turn += 1;
                let reply = if turn == 1 {
                    Ok(tool_reply(vec![tool_call(
                        "t",
                        "list_cases",
                        json!({ "state": "open" }),
                    )]))
                } else {
                    Err(("refused".to_string(), Some(400)))
                };
                async move { reply }
            },
        ));
        assert!(matches!(later, Err(AgentFailure::Failed(_))));
    }

    #[test]
    fn the_journal_counts_the_calls() {
        let folder = tempfile::tempdir().unwrap();
        let db_path = folder.path().join("humcase.db");
        let answer = check(
            raw(r#"{"found": true, "sentences": [
                {"text": "Address and payment on one page.", "excerpt": 2}
            ], "sub_case": null}"#),
            &reading(),
        );
        record_check(&db_path, &answer, 3, 3);
        let line = std::fs::read_to_string(folder.path().join(JOURNAL_FILE)).unwrap();
        let entry: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(entry["calls"], 3);
        assert_eq!(entry["prompt"], AGENT_PROMPT_VERSION);
    }

    // -- Bench ----------------------------------------------------------------------------------

    /// Dull, distinct subjects of the filler cases: no word of a bench question.
    const FILLER_FR: [&str; 35] = [
        "le nettoyage des vitres",
        "la machine à café",
        "le tri des déchets",
        "l'éclairage du couloir",
        "la salle de pause",
        "le renouvellement des badges",
        "la peinture de l'escalier",
        "le local à vélos",
        "l'affichage dans le hall",
        "les plantes vertes",
        "la climatisation du deuxième étage",
        "le contrôle des extincteurs",
        "la sonnette de l'entrée",
        "le tapis de l'accueil",
        "les stores de la salle ronde",
        "le calendrier des arrosages",
        "la réparation du portail",
        "le menu de la cantine",
        "la boîte aux lettres",
        "le distributeur d'eau",
        "l'ascenseur en panne",
        "les prises électriques",
        "la signalétique des issues",
        "le rangement des archives",
        "le nettoyage du four",
        "la fontaine du hall",
        "les horaires du gardien",
        "la réserve de papier",
        "le fauteuil cassé",
        "la fuite du robinet",
        "le tableau blanc",
        "les clés du local",
        "la porte du garage",
        "le nouveau micro-ondes",
        "la poubelle du fond",
    ];
    const FILLER_EN: [&str; 35] = [
        "the window washing rota",
        "the coffee machine",
        "the recycling bins",
        "the corridor lighting",
        "the break room fridge",
        "the visitor badges",
        "the stairwell paint",
        "the bike shed",
        "the lobby noticeboard",
        "the potted plants",
        "the second floor heating",
        "the fire extinguisher checks",
        "the front door buzzer",
        "the reception rug",
        "the blinds in the round room",
        "the watering calendar",
        "the broken gate",
        "the canteen menu",
        "the mailbox",
        "the water cooler",
        "the lift fault",
        "the power sockets",
        "the exit signs",
        "the archive shelving",
        "the oven descaling",
        "the lobby fountain",
        "the caretaker's hours",
        "the paper store",
        "the wobbly chair",
        "the dripping tap",
        "the whiteboard",
        "the storeroom keys",
        "the garage door",
        "the microwave",
        "the back bin",
    ];

    /// The cases of a base, its `filler` recent dull cases included.
    fn base_cases(base: &Value) -> Vec<Value> {
        let mut cases = base["cases"].as_array().unwrap().clone();
        let french = base["lang"] == "fr";
        let topics: &[&str] = if french { &FILLER_FR } else { &FILLER_EN };
        for i in 0..base["filler"].as_u64().unwrap_or(0) as usize {
            let n = i + 1;
            let topic = topics[i % topics.len()];
            let (title, content) = if french {
                (
                    format!("Notes de réunion {n}"),
                    format!("Réunion de la semaine. Sujet : {topic}. Rien d'autre à signaler."),
                )
            } else {
                (
                    format!("Meeting notes {n}"),
                    format!("Weekly meeting. Topic: {topic}. Nothing else to report."),
                )
            };
            cases.push(
                json!({ "key": format!("filler{n}"), "title": title, "nature": "note",
                "status": "open", "days_ago": 0, "content": content }),
            );
        }
        cases
    }

    /// Writes the cases of a bench base into `conn`; returns each case's id by key.
    pub(crate) fn build_base(conn: &Connection, base: &Value) -> HashMap<String, String> {
        use crate::db::journal::{self, Actor};
        journal::install(conn, &Actor::Human).unwrap();
        let work = project(conn, "Work", crate::models::project::ProjectSpace::Work);
        let cases = base_cases(base);
        let mut ids: HashMap<String, String> = HashMap::new();
        for case in &cases {
            let id = queries::create_note(
                conn,
                crate::models::note::CreateNoteInput {
                    project_id: work.clone(),
                    title: case["title"].as_str().unwrap().into(),
                    content: Some(case["content"].as_str().unwrap().into()),
                    note_type: serde_json::from_value(case["nature"].clone()).unwrap(),
                    priority: None,
                    git_branch: None,
                    tags: None,
                    due_at: None,
                    raw_content: None,
                },
            )
            .unwrap();
            let status = case["status"].as_str().unwrap();
            if case["agent_resolved"].as_bool() == Some(true) {
                journal::set_actor(conn, &Actor::Agent(Some("Codex".into()))).unwrap();
            }
            conn.execute(
                "UPDATE notes SET status = ?2 WHERE id = ?1",
                params![id, status],
            )
            .unwrap();
            journal::set_actor(conn, &Actor::Human).unwrap();
            if let Some(days) = case["due_in_days"].as_i64() {
                conn.execute(
                    "UPDATE notes SET due_at = date('now', 'localtime', ?2) WHERE id = ?1",
                    params![id, format!("+{days} days")],
                )
                .unwrap();
            }
            ids.insert(case["key"].as_str().unwrap().into(), id);
        }
        // Dates last: every write above moved them to now. A filler is dated a few minutes apart
        // so that the order of the overview is the same on every run.
        for case in &cases {
            let key = case["key"].as_str().unwrap();
            let days = case["days_ago"].as_i64().unwrap_or(0);
            let minutes = key
                .strip_prefix("filler")
                .and_then(|n| n.parse::<i64>().ok())
                .unwrap_or(0);
            conn.execute(
                "UPDATE notes SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2, ?3),
                                  created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2, ?3)
                  WHERE id = ?1",
                params![
                    ids[key],
                    format!("-{days} days"),
                    format!("-{minutes} minutes")
                ],
            )
            .unwrap();
        }
        ids
    }

    // The hard base is hard: the target case is in neither part of the first reading, and a
    // search with the case's own words finds it. If the first reading found it, the bench would
    // measure nothing about the tools.
    #[test]
    fn the_hard_bases_hide_their_targets_from_the_first_reading() {
        let bench: Value = serde_json::from_str(include_str!("ask_eval_cases.json")).unwrap();
        let mut checked = 0;
        for base in bench["bases"].as_array().unwrap() {
            if base["hard"].as_bool() != Some(true) {
                continue;
            }
            let conn = db();
            let ids = build_base(&conn, base);
            for item in base["questions"].as_array().unwrap() {
                let reading =
                    read(&conn, item["q"].as_str().unwrap(), &[], Some("work"), &[]).unwrap();
                let shown: Vec<&str> = reading
                    .detailed
                    .iter()
                    .chain(&reading.overview)
                    .map(|c| c.id.as_str())
                    .collect();
                for key in item["expect"].as_array().unwrap() {
                    let key = key.as_str().unwrap();
                    if key == "fair" {
                        assert!(shown.contains(&ids[key].as_str()), "the control question");
                    } else {
                        assert!(
                            !shown.contains(&ids[key].as_str()),
                            "«{}» finds {key} in the first reading",
                            item["q"]
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked >= 6);
        // ... and a search with the words the case itself uses finds it.
        for (base, key, word) in [
            ("fr-hard", "insurance", "prime"),
            ("fr-hard", "move", "loyer"),
            ("fr-hard", "accountant", "comptable"),
            ("fr-hard", "leave", "congés"),
            ("en-hard", "cleaning", "cleaning"),
            ("en-hard", "desks", "desks"),
        ] {
            let base = bench["bases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|b| b["name"] == base)
                .unwrap();
            let conn = db();
            let ids = build_base(&conn, base);
            let found = by_words(&conn, word, Some("work")).unwrap();
            assert_eq!(found, vec![ids[key].clone()], "{word} finds {key}");
        }
    }

    fn guard_reading(cases: Vec<Case>) -> Reading {
        Reading {
            detailed: cases,
            ..Reading::default()
        }
    }

    #[test]
    fn a_reading_holding_the_words_of_the_question_is_strong() {
        let reading = guard_reading(vec![case(
            "a",
            "Checkout on one page",
            &["Checkout on one page", "Address and payment on one page."],
        )]);
        // Two significant words held.
        assert!(reading_is_strong(&reading, "payment on one page", &[]));
        // One specific word is enough ...
        let hosting = guard_reading(vec![case(
            "h",
            "Hosting plan",
            &["Hosting plan", "Cheaper plan at the end of the year."],
        )]);
        assert!(reading_is_strong(&hosting, "Where are we on hosting?", &[]));
        // ... or a case found by meaning, or a question with no word to search.
        let other = guard_reading(vec![case("m", "Unrelated", &["Unrelated"])]);
        assert!(reading_is_strong(
            &other,
            "insurance premium",
            &["m".into()]
        ));
        assert!(!reading_is_strong(&other, "insurance premium", &[]));
        assert!(reading_is_strong(&other, "and the", &[]));
    }

    // Offering the tools only to a weak reading is what keeps the easy questions on one call:
    // always answering "strong" would send the hidden cases of the hard base to the single call.
    #[test]
    fn a_reading_sharing_nothing_specific_with_the_question_is_weak() {
        let reading = guard_reading(vec![case(
            "a",
            "Checkout on one page",
            &["Checkout on one page", "Address and payment on one page."],
        )]);
        assert!(!reading_is_strong(
            &reading,
            "How much is my insurance?",
            &[]
        ));
        assert!(!reading_is_strong(
            &guard_reading(Vec::new()),
            "insurance premium",
            &[]
        ));
        // One short word among several is a coincidence, not a hit.
        assert!(!reading_is_strong(
            &reading,
            "Which page costs the insurance premium?",
            &[]
        ));
    }

    /// Which bench questions the guard offers the tools to, without any AI call.
    #[test]
    fn the_guard_routes_the_bench_questions() {
        let bench: Value = serde_json::from_str(include_str!("ask_eval_cases.json")).unwrap();
        let (mut easy_to_agent, mut hard_to_single) = (Vec::new(), Vec::new());
        for base in bench["bases"].as_array().unwrap() {
            let conn = db();
            let ids = build_base(&conn, base);
            let hard_base = base["hard"].as_bool() == Some(true);
            for item in base["questions"].as_array().unwrap() {
                let question = item["q"].as_str().unwrap();
                let previous: Vec<PreviousTurn> = item["previous"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| PreviousTurn {
                        question: p["question"].as_str().unwrap().into(),
                        answer: String::new(),
                        source_ids: p["cites"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|k| ids[k.as_str().unwrap()].clone())
                            .collect(),
                    })
                    .collect();
                let reading = read(&conn, question, &previous, Some("work"), &[]).unwrap();
                let mut query = question.to_string();
                if let Some(last) = previous.last() {
                    query.push(' ');
                    query.push_str(&last.question);
                }
                let agent = !reading_is_strong(&reading, &query, &[]);
                let expect = item["expect"].as_array().unwrap();
                let target = hard_base && !expect.is_empty() && !expect.iter().any(|k| k == "fair");
                let neutral = item["not_found"].as_bool() == Some(true);
                println!("{} {question}", if agent { "AGENT " } else { "single" });
                if agent && !target && !neutral {
                    easy_to_agent.push(question.to_string());
                }
                if !agent && target {
                    hard_to_single.push(question.to_string());
                }
            }
        }
        println!("answerable questions given the tools needlessly: {easy_to_agent:?}");
        assert!(
            hard_to_single.is_empty(),
            "questions the tools are needed for stay on the single call: {hard_to_single:?}"
        );
        // Questions about state ("what has been stalling?") have words no case holds, so a few
        // answerable ones still get the tools: that costs time, not correctness.
        assert!(easy_to_agent.len() <= 5, "{easy_to_agent:?}");
    }

    /// Tokens and cost of the calls recorded so far.
    pub(crate) fn usage_totals(path: &Path) -> (u64, u64, f64) {
        let usage: Vec<_> = crate::ai_usage::read(path, 0)
            .into_iter()
            .filter(|u| u.ok)
            .collect();
        let cost = usage
            .iter()
            .filter_map(|u| {
                let tokens = crate::ai_usage::TokenUsage {
                    input_tokens: u.input_tokens,
                    output_tokens: u.output_tokens,
                    ..Default::default()
                };
                crate::ai_usage::token_cost(&u.provider, &u.model, &tokens)
            })
            .sum();
        (
            usage.iter().filter_map(|u| u.input_tokens).sum(),
            usage.iter().filter_map(|u| u.output_tokens).sum(),
            cost,
        )
    }

    /// What the bench counts, overall and per base.
    #[derive(Default, Clone)]
    struct Tally {
        questions: usize,
        errors: usize,
        right_kind: usize,
        full_recall: usize,
        recall_sum: f64,
        forbidden: usize,
        dropped: usize,
        sentences: usize,
        sub_expected: usize,
        sub_proposed: usize,
        sub_unexpected: usize,
        facts_expected: usize,
        facts_missing: usize,
        jargon: usize,
        model_calls: usize,
        seconds: f64,
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
        /// Questions given the tools, those that did not need them, and those that needed them
        /// but were answered by the single call.
        routed_agent: usize,
        easy_to_agent: usize,
        hard_to_single: usize,
    }

    impl Tally {
        fn add(&mut self, o: &Tally) {
            self.questions += o.questions;
            self.errors += o.errors;
            self.right_kind += o.right_kind;
            self.full_recall += o.full_recall;
            self.recall_sum += o.recall_sum;
            self.forbidden += o.forbidden;
            self.dropped += o.dropped;
            self.sentences += o.sentences;
            self.sub_expected += o.sub_expected;
            self.sub_proposed += o.sub_proposed;
            self.sub_unexpected += o.sub_unexpected;
            self.facts_expected += o.facts_expected;
            self.facts_missing += o.facts_missing;
            self.jargon += o.jargon;
            self.model_calls += o.model_calls;
            self.seconds += o.seconds;
            self.input_tokens += o.input_tokens;
            self.output_tokens += o.output_tokens;
            self.cost_usd += o.cost_usd;
            self.routed_agent += o.routed_agent;
            self.easy_to_agent += o.easy_to_agent;
            self.hard_to_single += o.hard_to_single;
        }

        fn json(&self) -> Value {
            let n = self.questions.max(1) as f64;
            json!({
                "questions": self.questions, "errors": self.errors,
                "right_kind": self.right_kind, "full_recall": self.full_recall,
                "mean_recall": self.recall_sum / n, "forbidden": self.forbidden,
                "facts_expected": self.facts_expected, "facts_missing": self.facts_missing,
                "jargon": self.jargon, "sentences": self.sentences, "dropped": self.dropped,
                "sub_case_expected": self.sub_expected, "sub_case_proposed": self.sub_proposed,
                "sub_case_unexpected": self.sub_unexpected,
                "model_calls": self.model_calls, "mean_calls": self.model_calls as f64 / n,
                "seconds": self.seconds, "mean_seconds": self.seconds / n,
                "input_tokens": self.input_tokens, "output_tokens": self.output_tokens,
                "cost_usd": self.cost_usd,
                "routed_to_agent": self.routed_agent,
                "easy_wrongly_to_agent": self.easy_to_agent,
                "hard_wrongly_single": self.hard_to_single,
            })
        }
    }

    /// Measures Ask Humcase on `ask_eval_cases.json` (six languages, plus the hard bases whose
    /// target case is outside the first reading) with real calls: per question the cases cited
    /// against the expected ones, phrases that would turn a proposal into a decision, "not found"
    /// when it should be, sub-cases proposed, sentences dropped by the check, model calls,
    /// seconds, tokens and estimated cost. Provider from `ASK_EVAL_PROVIDER` (Cerebras), model
    /// from `ASK_EVAL_MODEL`, instruction from the file `ASK_EVAL_PROMPT` to compare another
    /// version, `ASK_EVAL_MODE` = `single` (default, one call) or `agent` (the tools of
    /// `run_agent`); the results go to `ASK_EVAL_OUT`. Search by meaning is not used.
    /// `ASK_EVAL_BASES` (names or languages, comma separated) keeps only those bases.
    /// `cargo test ask_eval -- --ignored --nocapture`
    #[test]
    #[ignore = "real AI calls"]
    fn ask_eval() {
        let bench: Value = serde_json::from_str(include_str!("ask_eval_cases.json")).unwrap();
        let provider = std::env::var("ASK_EVAL_PROVIDER").unwrap_or_else(|_| "cerebras".into());
        let model = std::env::var("ASK_EVAL_MODEL").ok();
        let mode = match std::env::var("ASK_EVAL_MODE").as_deref() {
            Ok("agent") => AskMode::Agent,
            Ok("auto") => AskMode::Auto,
            _ => AskMode::Single,
        };
        // Another instruction to compare with, from a file (`ASK_EVAL_PROMPT`).
        let (prompt, version) = match std::env::var("ASK_EVAL_PROMPT") {
            Ok(path) => (std::fs::read_to_string(&path).unwrap(), path),
            Err(_) => (SYSTEM_PROMPT.to_string(), PROMPT_VERSION.to_string()),
        };
        let folder = tempfile::tempdir().unwrap();
        let usage_path = folder.path().join("eval.db");
        let usage_ref: &Path = &usage_path;
        let chat_tools_value = agent_tools();
        let chat_tools: &Value = &chat_tools_value;
        let mut results = Vec::new();
        let mut overall = Tally::default();
        let mut hard = Tally::default();
        let mut per_base: Vec<(String, Tally)> = Vec::new();
        for base in bench["bases"].as_array().unwrap() {
            let name = base["name"]
                .as_str()
                .or(base["lang"].as_str())
                .unwrap()
                .to_string();
            if let Ok(only) = std::env::var("ASK_EVAL_BASES") {
                if !only.split(',').any(|n| n == name) {
                    continue;
                }
            }
            let conn = db();
            let ids = build_base(&conn, base);
            let conn = std::sync::Mutex::new(conn);
            let key_of = |id: &str| {
                ids.iter()
                    .find(|(_, v)| v.as_str() == id)
                    .map(|(k, _)| k.clone())
                    .unwrap_or_else(|| id.to_string())
            };
            let mut tally = Tally::default();
            for item in base["questions"].as_array().unwrap() {
                let question = item["q"].as_str().unwrap();
                let previous: Vec<PreviousTurn> = item["previous"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| PreviousTurn {
                        question: p["question"].as_str().unwrap().into(),
                        answer: p["answer"].as_str().unwrap().into(),
                        source_ids: p["cites"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|k| ids[k.as_str().unwrap()].clone())
                            .collect(),
                    })
                    .collect();
                let mut reading = {
                    let conn = conn.lock().unwrap();
                    read(&conn, question, &previous, Some("work"), &[]).unwrap()
                };
                let agent = match mode {
                    AskMode::Single => false,
                    AskMode::Agent => true,
                    AskMode::Auto => {
                        let mut query = question.to_string();
                        if let Some(last) = previous.last() {
                            query.push(' ');
                            query.push_str(&last.question);
                        }
                        !reading_is_strong(&reading, &query, &[])
                    }
                };
                // Questions whose case the first reading cannot hold (`hard` bases, except the
                // control and the unanswerable one) must go to the tools, the others need not.
                let expected_keys = item["expect"].as_array().unwrap();
                let hard_target = base["hard"].as_bool() == Some(true)
                    && !expected_keys.is_empty()
                    && !expected_keys.iter().any(|k| k == "fair");
                let neutral = item["not_found"].as_bool() == Some(true);
                let message = reading.message(question, &previous);
                let before = usage_totals(usage_ref);
                let started = std::time::Instant::now();
                let (raw, calls, tools): (Result<RawAnswer, String>, usize, Vec<String>) = if agent
                {
                    let system = format!("{prompt}{AGENT_ADDENDUM}");
                    let outcome = tauri::async_runtime::block_on(run_agent(
                        &conn,
                        Some("work"),
                        question,
                        &previous,
                        &mut reading,
                        &system,
                        message,
                        |messages, choice| {
                            let (provider, model) = (provider.clone(), model.clone());
                            async move {
                                // A free tier refuses bursts (HTTP 429): wait and ask again.
                                let mut reply = Err((String::new(), None));
                                for wait in [0u64, 5, 10, 20, 40] {
                                    tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                                    reply = chat_turn(
                                        messages.clone(),
                                        choice.clone(),
                                        chat_tools,
                                        provider.clone(),
                                        model.clone(),
                                        usage_ref,
                                        crate::ai_usage::operation::ASK,
                                        None,
                                    )
                                    .await;
                                    if !reply.as_ref().is_err_and(|(e, _)| e.contains("429")) {
                                        break;
                                    }
                                }
                                reply
                            }
                        },
                    ));
                    match outcome {
                        Ok(done) => (Ok(done.raw), done.calls, done.tools),
                        Err(AgentFailure::ToolsRefused) => {
                            (Err("tools refused".into()), 1, Vec::new())
                        }
                        Err(AgentFailure::Failed(e)) => (Err(e), 0, Vec::new()),
                    }
                } else {
                    // A free tier refuses bursts (HTTP 429): wait and ask again, never count it.
                    let mut text = Err(String::new());
                    for wait in [0, 5, 10, 20, 40] {
                        std::thread::sleep(std::time::Duration::from_secs(wait));
                        text = tauri::async_runtime::block_on(crate::commands::ai::call_llm_json(
                            &prompt,
                            message.clone(),
                            Some(provider.clone()),
                            model.clone(),
                            "answer",
                            answer_schema(),
                            usage_ref,
                            crate::ai_usage::operation::ASK,
                        ));
                        if !text.as_ref().is_err_and(|e| e.contains("429")) {
                            break;
                        }
                    }
                    (text.and_then(|t| parse_answer(&t)), 1, Vec::new())
                };
                let seconds = started.elapsed().as_secs_f32();
                let after = usage_totals(usage_ref);
                let mut q = Tally {
                    questions: 1,
                    model_calls: calls,
                    seconds: seconds as f64,
                    input_tokens: after.0 - before.0,
                    output_tokens: after.1 - before.1,
                    cost_usd: after.2 - before.2,
                    routed_agent: usize::from(agent),
                    easy_to_agent: usize::from(agent && !hard_target && !neutral),
                    hard_to_single: usize::from(!agent && hard_target),
                    ..Tally::default()
                };
                let answer = match raw {
                    Ok(raw) => check(raw, &reading),
                    Err(e) => {
                        println!("{name} «{question}»: {e}");
                        q.errors = 1;
                        q.questions = 0;
                        tally.add(&q);
                        continue;
                    }
                };
                let expect: Vec<String> = item["expect"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|k| k.as_str().unwrap().to_string())
                    .collect();
                let cited: Vec<String> =
                    answer.sources.iter().map(|s| key_of(&s.note_id)).collect();
                let not_found = item["not_found"].as_bool() == Some(true);
                let right_kind = if not_found {
                    answer.kind == AnswerKind::NotFound
                } else {
                    answer.kind == AnswerKind::Answer
                };
                let found = expect.iter().filter(|k| cited.contains(k)).count();
                let recall = if expect.is_empty() {
                    1.0
                } else {
                    found as f64 / expect.len() as f64
                };
                let text: String = answer.parts.iter().map(|p| p.text.as_str()).collect();
                // No-break spaces read as spaces ("4 000 €").
                let lower = text.to_lowercase().replace(['\u{a0}', '\u{202f}'], " ");
                let hits: Vec<&str> = item["forbid"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|f| f.as_str())
                    .filter(|f| lower.contains(f))
                    .collect();
                let missing: Vec<String> = item["must"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|fact| fact.as_array())
                    .filter(|spellings| {
                        !spellings
                            .iter()
                            .filter_map(|f| f.as_str())
                            .any(|f| lower.contains(f))
                    })
                    .map(|spellings| spellings[0].as_str().unwrap_or_default().to_string())
                    .collect();
                let jargon: Vec<&str> = base["jargon"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|f| f.as_str())
                    .filter(|f| lower.contains(f))
                    .collect();
                let wants_sub = item["sub_case"].as_bool() == Some(true);
                q.facts_expected = item["must"].as_array().map_or(0, Vec::len);
                q.facts_missing = missing.len();
                q.jargon = jargon.len();
                q.right_kind = usize::from(right_kind);
                q.full_recall = usize::from(recall == 1.0);
                q.recall_sum = recall;
                q.forbidden = hits.len();
                q.dropped = answer.dropped;
                q.sentences = answer.parts.len();
                q.sub_expected = usize::from(wants_sub);
                q.sub_proposed = usize::from(wants_sub && answer.action.is_some());
                q.sub_unexpected = usize::from(!wants_sub && answer.action.is_some());
                tally.add(&q);
                let extra: Vec<&String> = cited.iter().filter(|k| !expect.contains(k)).collect();
                let missed = if recall < 1.0 {
                    format!("MISSED {}/{} ", expect.len() - found, expect.len())
                } else {
                    String::new()
                };
                println!(
                    "\n== {name} «{question}» ({seconds:.1} s, {calls} calls) {}{missed}",
                    if right_kind { "" } else { "WRONG KIND " },
                );
                for tool in &tools {
                    println!("   tool {tool}");
                }
                println!("   {text}");
                println!(
                    "   cited {cited:?}, extra {extra:?}, dropped {}, forbidden {hits:?}, facts missing {missing:?}, jargon {jargon:?}, sub-case {}",
                    answer.dropped,
                    answer.action.as_ref().map_or("-".into(), |a| format!(
                        "«{}» in {}",
                        a.title,
                        key_of(&a.parent_id)
                    ))
                );
                results.push(json!({
                    "base": name, "question": question, "seconds": seconds, "calls": calls,
                    "agent": agent,
                    "tools": tools,
                    "kind": answer.kind, "right_kind": right_kind, "expect": expect, "cited": cited,
                    "recall": recall, "forbidden": hits, "facts_missing": missing, "jargon": jargon, "dropped": answer.dropped, "text": text,
                    "sub_case": answer.action.as_ref().map(|a| json!({ "title": a.title, "in": key_of(&a.parent_id) })),
                }));
            }
            overall.add(&tally);
            if base["hard"].as_bool() == Some(true) {
                hard.add(&tally);
            }
            per_base.push((name, tally));
        }
        let summary = json!({
            "mode": match mode { AskMode::Single => "single", AskMode::Agent => "agent", AskMode::Auto => "auto" },
            "prompt": if mode == AskMode::Single { version } else { AGENT_PROMPT_VERSION.to_string() },
            "provider": provider,
            "overall": overall.json(),
            "hard": hard.json(),
            "per_base": per_base.iter().map(|(n, t)| (n.clone(), t.json())).collect::<serde_json::Map<_, _>>(),
        });
        println!("\n{}", serde_json::to_string_pretty(&summary).unwrap());
        if let Ok(out) = std::env::var("ASK_EVAL_OUT") {
            std::fs::write(
                out,
                serde_json::to_string_pretty(&json!({ "summary": summary, "results": results }))
                    .unwrap(),
            )
            .unwrap();
        }
    }
}
