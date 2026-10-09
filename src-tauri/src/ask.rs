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
pub(crate) const OWN_KINDS: [&str; 2] = [KEPT_KIND, REFLECT_KIND];
/// The provenance of a summary kept by "Think it through", before the assistant replaced it.
pub(crate) const REFLECT_KIND: &str = "reflect";
const KEPT_TITLE_MAX: usize = 200;

/// Cases whose sentences are sent in full.
const DETAILED_MAX: usize = 7;
/// Sentences of one case sent, its title included.
pub(crate) const SENTENCES_PER_CASE: usize = 9;
/// Latest cases of the space sent as one line each (for "what is waiting", "this week").
const OVERVIEW_MAX: usize = 25;
pub(crate) const QUESTION_MAX: usize = 1000;

/// Size at which a local journal of checks (numbers only) starts again.
pub(crate) const JOURNAL_MAX_SIZE: u64 = 1024 * 1024;

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

/// A piece of the answer; `cite` is the 1-based number of its source.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Part {
    pub text: String,
    pub cite: Option<usize>,
}

/// Passages shown under one source.
pub(crate) const PASSAGES_MAX: usize = 3;

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
    let mut text = reference.replace_all(text, "").into_owned();
    // A brace or bracket closing nothing, left at the end by the JSON around the text
    // ("Vous demandez quel modèle répond ici.}", seen with Claude Haiku 5.5).
    loop {
        let trimmed = text.trim_end();
        let (open, close) = match trimmed.chars().last() {
            Some('}') => ('{', '}'),
            Some(']') => ('[', ']'),
            _ => break,
        };
        if trimmed.matches(open).count() >= trimmed.matches(close).count() {
            break;
        }
        let end = trimmed.len() - close.len_utf8();
        text.truncate(end);
    }
    text
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
    crate::db::cases::add_provenance(
        conn,
        &id,
        KEPT_KIND,
        Some("Humcase"),
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

pub(crate) fn decision_of(conn: &Connection, note_id: &str, status: &str) -> String {
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
pub(crate) fn in_space(
    conn: &Connection,
    space: Option<&str>,
    project_id: &str,
) -> Result<bool, String> {
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
    load_case_as(
        conn,
        id,
        space,
        projects,
        question_words,
        max_sentences,
        false,
    )
}

/// [`load_case`], or with `opened` the case the person has open: read even when it is archived,
/// a kept answer or outside the space read, since they chose it (never in the trash, never a
/// drawing).
pub(crate) fn load_case_as(
    conn: &Connection,
    id: &str,
    space: Option<&str>,
    projects: &HashMap<String, String>,
    question_words: &HashSet<String>,
    max_sentences: usize,
    opened: bool,
) -> Result<Option<Case>, String> {
    let Ok(note) = queries::get_note_by_id(conn, id) else {
        return Ok(None);
    };
    if note.deleted_at.is_some() || note.content_kind == crate::models::note::ContentKind::Sketch {
        return Ok(None);
    }
    if !opened
        && (note.archived_at.is_some()
            || is_kept_answer(conn, &note.id)?
            || !in_space(conn, space, &note.project_id)?)
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

/// Cases `search_cases` returns, and lines `list_cases` returns.
const AGENT_SEARCH_MAX: usize = 5;
const AGENT_LIST_MAX: usize = 15;
/// Sentences of a case `read_case` returns (the title included): all a case offers.
pub(crate) const AGENT_READ_SENTENCES: usize = 40;

pub(crate) fn agent_tool(name: &str, description: &str, parameters: Value) -> Value {
    json!({ "type": "function", "function": {
        "name": name, "description": description, "parameters": parameters } })
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
pub(crate) fn case_for_tool(reading: &mut Reading, case: &Case) -> Value {
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

/// The loop of [`run_agent`] for any final answer: the model finishes by calling the function
/// `final_tool`, whose arguments `parse` reads (the same reader is tried on a JSON object
/// written as plain text); at most `max_calls` model calls, the last forced to `final_tool`.
/// `query` is what the question words of the search tools are taken from. `extra_tool` runs the
/// tools of the caller beyond the shared read-only ones (`None`: not one of its tools).
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
    extra_tool: impl Fn(&Connection, &mut Reading, &str, &Value) -> Option<Result<Value, String>>,
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
                extra_tool(&conn, reading, name, &args)
                    .unwrap_or_else(|| {
                        run_tool(&conn, space, &question_words, reading, name, &args)
                    })
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
        assert_eq!(
            without_references("Vous demandez quel modèle répond ici.}"),
            "Vous demandez quel modèle répond ici."
        );
        assert_eq!(without_references("Done.}] "), "Done.");
        assert_eq!(
            without_references("The set {a, b} stays [as written]"),
            "The set {a, b} stays [as written]"
        );
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

    fn empty_reading() -> Reading {
        let mut reading = Reading {
            today: "2026-10-08".into(),
            ..Reading::default()
        };
        reading.message("Question?", &[]);
        reading
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
}
