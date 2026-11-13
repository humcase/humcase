//! The assistant: one conversation that answers, discusses and proposes actions.
//!
//! It reads the cases as "Ask Humcase" does (words, meaning, the latest cases, the case open in the
//! interface first) and may look further with read-only tools: the shared ones of `ask`, the cases
//! changed in a period, what "To tidy" found, and Humcase's own documentation. The AI replies in
//! sentences; a sentence that names an excerpt is kept only when that excerpt was really sent, so a
//! source shown is always a sentence of a case (or of the documentation) word for word. Actions are
//! proposals checked against the database; nothing is written until the person clicks, and the
//! interface runs them with the existing commands.

use crate::ask::{self, Case, Part, Reading};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;

/// Version of the instruction below, recorded with each reply.
const PROMPT_VERSION: &str = "assistant.v5";
/// Local journal of the replies: numbers only (no text), next to the database.
pub const JOURNAL_FILE: &str = "assistant-journal.jsonl";

/// Earlier exchanges the AI reads.
const PREVIOUS_MAX: usize = 8;
const MESSAGE_MAX: usize = 2000;
const PREVIOUS_REPLY_MAX: usize = 1500;
/// Sentences of the open case sent (its title included).
const CONTEXT_SENTENCES: usize = 20;
/// Model calls a reply may take, the last one forced to `reply`.
const CALLS_MAX: usize = 6;
/// The whole reply (every model call and the fallback) stops after this long, whatever the
/// number of calls and the limit of each.
const REPLY_DEADLINE: std::time::Duration = std::time::Duration::from_secs(120);
const ACTIONS_MAX: usize = 3;
const SUB_CASE_TITLE_MAX: usize = 120;
/// Cases `list_changed` returns, and items `list_tidy` returns.
const CHANGED_MAX: usize = 40;
const TIDY_MAX: usize = 12;
/// Help sections `help` returns, and sentences of each.
const HELP_SECTIONS_MAX: usize = 3;
const HELP_SENTENCES_MAX: usize = 6;
/// Prefix of the id of a help section: never a case, so never the target of an action.
const HELP_ID: &str = "help:";

/// Humcase's documentation, compiled in: one section per bullet, "**Title**: text".
const HELP: &str = include_str!("../../docs/FEATURES.md");

const SYSTEM_PROMPT: &str = "You are the assistant inside Humcase, the app where a person keeps their work as cases (notes, tasks, questions, research, decisions) in projects, alone or with AI agents. You talk with that person. `you_are` names the AI model you run on: say it when asked.
The user message is JSON: `today`; `latest`, what the person just wrote (empty: they just opened the case `context` to think it through); `conversation`, the exchange so far, oldest first (`person`, `you`); `context`, the number of the case the person has open, or null; `cases`, the cases whose words are closest to the conversation, each with its number, project, kind, state, last change, due day and numbered excerpts (the first one is its title); `overview`, the latest cases of the space, one line each; `projects`, the names of the projects of the space.
First fill `language`: the language of `latest` (of the conversation when `latest` is empty, of the context case when both are empty), by its English name. Write every text in that language, with no word of another one, and address the person as the interface does: \"vous\" in French, \"tú\" in Spanish, \"du\" in German, \"você\" in Portuguese, \"tu\" in Italian.
How to reply:
1. `sentences`: your reply, one to six short sentences, plain and direct; answer what the person asked first. A sentence that states something about the person's work (a fact, a figure, a date, a name, a state, a decision, what a case says) names in `excerpt` the id of the excerpt that states it, and a sentence about how Humcase works names the excerpt of the documentation it comes from. A sentence naming an excerpt that was not given is removed. A sentence with `excerpt` null may only restate what the person said, give your own view or suggestion as yours (\"I would start with…\"), or say what you did not find; it never states a fact about the work or about Humcase.
2. Never state what the excerpts and the person do not say: no outside knowledge about their work, no guess. When nothing answers, say so in one sentence and say what you looked for. Keep each case's certainty: a proposal stays a proposal, what is open stays open, and a result waiting for the person's decision is not decided. Never say who wrote, proposed or decided something unless the excerpt names them.
3. When the person hesitates, is stuck or asks for your opinion, give one clear view built on the excerpts you cite, as yours, and leave the decision to them.
4. `question`: null unless one question really moves the person forward: the point they hesitate on, or what their last message leaves open. Never a question they already answered, never one that repeats what they just said, never a yes-or-no question whose answer their words already give, never a generic one (budget, deadline, criteria, next step), never several. When `latest` is empty, restate the context case in one or two sentences from its own text, then ask one first question about what in it is still open for the person.
5. `actions`: what you can do for the person, done only when they click; at most three, usually none. Propose one only when the person asks for it, says they will do it or agrees to it, or when an item of list_tidy answers their request. When the person asks what to do now after a plan was made, propose the actions of that plan.
   - create_sub_case: a new task `title` (short, in their language) inside the case numbered `case`;
   - germinate: open Sprout on the case numbered `case`: it proposes the case's sub-cases from its own text, each with its passage, for the person to tick; the action when they ask to split a case, break it down or find its steps (not several create_sub_case);
   - archive: archive the case numbered `case` (done or out of date);
   - move: file the case numbered `case` in the project named `project` (a name of `projects`, written exactly);
   - tidy: accept the item `tidy` of list_tidy (a case to archive, an older decision a newer one replaces, a case to file inside another, a case that goes against a decision).
   Never propose to create what already has a case, nor an action the person did not ask for or agree to.
6. Speak as the person would: name a case by its subject, never as \"the case\", and say states in plain words (\"waits for your decision\", \"in progress\"); never write the words of the data (excerpt, case number, kind, state, tidy, id). When you need the word, say it as the interface does: \"case\" in English and French (\"une case\", never \"dossier\" or \"fiche\"), \"caso\" in Spanish, Portuguese and Italian, \"Fall\" in German. Name Humcase's parts as its interface does in that language, never by their English names from the documentation: Sprout, Structure, To tidy, Inbox, Settings are \"Faire germer\", \"Structurer\", \"À ranger\", \"Réception\", \"Paramètres\" in French; \"Germinar\", \"Estructurar\", \"Por ordenar\", \"Bandeja de entrada\", \"Ajustes\" in Spanish; \"Entfalten\", \"Strukturieren\", \"Aufräumen\", \"Eingang\", \"Einstellungen\" in German; \"Germinar\", \"Estruturar\", \"A arrumar\", \"Caixa de entrada\", \"Configurações\" in Portuguese; \"Fai germogliare\", \"Struttura\", \"Da riordinare\", \"Inbox\", \"Impostazioni\" in Italian. No heading. A short list only when the person asks for a summary or several items.
Tools: the given cases are only part of the space.
- search_cases(query): the cases holding the words of `query` (use the words a case about it would contain, a synonym; search again with other words before saying nothing exists).
- read_case(case): all the excerpts of one case, by its number.
- list_cases(state): the latest open, in-progress or waiting cases.
- list_changed(since, until): the cases changed between two days (YYYY-MM-DD, both included), with counts; for a question about a period (\"this week\", \"yesterday\"), the days computed from `today`.
- list_tidy(): what Humcase found to tidy, each item with its cases and passages: cases to archive, older decisions replaced by newer ones, cases to file together, cases that go against a decision. Use it for questions about contradictions, duplicates, outdated or messy cases.
- help(query): Humcase's documentation, for questions about how Humcase works; write `query` in English words.
Excerpt ids and case numbers from the tools work like those in the message. Finish by calling `reply`; you have a few turns.";

/// An earlier exchange, as the interface sends it.
#[derive(Debug, Clone, Deserialize)]
pub struct Turn {
    /// What the person wrote (empty for the opening of a case).
    #[serde(default)]
    pub said: String,
    /// The assistant's reply as plain text, its question included.
    #[serde(default)]
    pub reply: String,
    /// Cases the reply cited.
    #[serde(default)]
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReplyKind {
    Reply,
    NoAi,
}

/// A case (or a section of the documentation) the reply relies on, with the sentences it cites.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Source {
    /// `None` for a section of the documentation.
    pub note_id: Option<String>,
    pub title: String,
    pub passages: Vec<String>,
    pub help: bool,
}

/// An action the reply proposes; the interface runs it when the person clicks.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    CreateSubCase {
        case_id: String,
        case_title: String,
        title: String,
    },
    /// Opens Sprout on the case; its proposals wait there, the click writes nothing.
    Germinate {
        case_id: String,
        case_title: String,
    },
    Archive {
        case_id: String,
        case_title: String,
    },
    Move {
        case_id: String,
        case_title: String,
        project_id: String,
        project_name: String,
        from_project_id: String,
    },
    Tidy {
        tidy_id: String,
        tidy_kind: crate::tidy::TidyKind,
        case_id: String,
        case_title: String,
        other_id: Option<String>,
        other_title: Option<String>,
        passage: Option<String>,
        other_passage: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Reply {
    pub kind: ReplyKind,
    pub parts: Vec<Part>,
    pub question: Option<String>,
    pub sources: Vec<Source>,
    pub actions: Vec<Action>,
    /// The model that answered, as the person reads it ("Claude Haiku 5.5").
    pub model_label: Option<String>,
    /// Sentences dropped because the excerpt they named was not sent.
    pub dropped: usize,
}

impl Reply {
    fn empty(kind: ReplyKind, model_label: Option<String>) -> Self {
        Reply {
            kind,
            parts: Vec::new(),
            question: None,
            sources: Vec::new(),
            actions: Vec::new(),
            model_label,
            dropped: 0,
        }
    }
}

/// What the header and the empty conversation show.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Status {
    pub model_label: Option<String>,
    pub cases: usize,
    pub to_decide: usize,
    /// Items of "To tidy" where two cases disagree: a case against a decision, or a decision a
    /// newer one may replace.
    pub conflicts: usize,
    /// The other items of "To tidy": cases to archive, cases to file together.
    pub to_tidy: usize,
}

/// A model id as the person reads it: "claude-haiku-5-5" → "Claude Haiku 5.5".
pub(crate) fn model_label(model: &str) -> String {
    let model = model.rsplit('/').next().unwrap_or(model);
    let mut words: Vec<String> = Vec::new();
    let mut previous_number = false;
    for piece in model.split(['-', '_', ' ']).filter(|p| !p.is_empty()) {
        let number = piece.chars().all(|c| c.is_ascii_digit() || c == '.');
        if number && previous_number {
            if let Some(last) = words.last_mut() {
                last.push('.');
                last.push_str(piece);
                continue;
            }
        }
        previous_number = number;
        let word = match piece {
            "gpt" | "oss" => piece.to_uppercase(),
            _ => {
                let mut chars = piece.chars();
                chars
                    .next()
                    .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            }
        };
        words.push(word);
    }
    words.join(" ")
}

/// The provider to ask and the label of the model it runs: the chosen one when ready, else the
/// first ready one only when none is chosen (`ask::provider_to_ask`): a chosen AI without a key
/// is an error, never replaced by another; a model chosen for another provider is dropped.
fn provider_and_model(
    providers: &[crate::providers::ProviderInfo],
    provider: Option<&str>,
    model: Option<String>,
) -> Result<Option<(String, Option<String>, String)>, String> {
    let Some((id, label)) = ask::provider_to_ask(providers, provider)? else {
        return Ok(None);
    };
    let model = model.filter(|m| !m.trim().is_empty() && provider == Some(id.as_str()));
    let default = providers
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.default_model.clone())
        .unwrap_or_default();
    let shown = model.clone().unwrap_or(default);
    let shown = if shown.is_empty() {
        label
    } else {
        model_label(&shown)
    };
    Ok(Some((id, model, shown)))
}

// ---- The tools of the assistant -------------------------------------------------------------

/// A section of the documentation as a case the reading can register excerpts of.
fn help_case(index: usize, title: &str) -> Case {
    Case {
        id: format!("{HELP_ID}{index}"),
        title: title.to_string(),
        project: String::new(),
        nature: String::new(),
        status: String::new(),
        decision: String::new(),
        updated: String::new(),
        due: None,
        sentences: Vec::new(),
    }
}

/// The sections of the documentation: each bullet "- **Title**: text", plus the "##" sections.
fn help_sections() -> &'static [(String, String)] {
    static SECTIONS: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    SECTIONS.get_or_init(|| {
        let mut sections: Vec<(String, String)> = Vec::new();
        let mut heading: Option<(String, String)> = None;
        for line in HELP.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("- **") {
                if let Some((title, text)) = rest.split_once("**") {
                    let text = text.trim_start_matches(':').trim();
                    sections.push((title.trim().to_string(), text.to_string()));
                }
            } else if let Some(title) = line.strip_prefix("## ") {
                if let Some(done) = heading.take() {
                    sections.push(done);
                }
                heading = Some((title.trim().to_string(), String::new()));
            } else if let Some((_, text)) = heading.as_mut() {
                if !line.is_empty() {
                    text.push(' ');
                    text.push_str(line);
                }
            }
        }
        sections.extend(heading);
        sections
    })
}

/// The start of a word, so near forms meet ("archived", "archive").
fn stem(word: &str) -> String {
    word.chars().take(5).collect()
}

fn stems(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3)
        .map(|w| stem(&w.to_lowercase()))
        .filter(|w| !crate::common_words::is_common(w))
        .collect()
}

/// The sections of the documentation that hold the words of `query`, best first; their
/// sentences are registered so a reply may cite them.
fn help(reading: &mut Reading, query: &str) -> Value {
    let wanted = stems(query);
    let mut scored: Vec<(usize, usize)> = help_sections()
        .iter()
        .enumerate()
        .map(|(i, (title, text))| {
            let in_title = stems(title).intersection(&wanted).count();
            let in_text = stems(text).intersection(&wanted).count();
            (i, in_title * 3 + in_text)
        })
        .filter(|(_, score)| *score > 0)
        .collect();
    scored.sort_by_key(|s| std::cmp::Reverse(s.1));
    if scored.is_empty() {
        let titles: Vec<&str> = help_sections().iter().map(|(t, _)| t.as_str()).collect();
        return json!({ "sections": [], "note": "no section holds these words; the titles are",
            "titles": titles });
    }
    let sections: Vec<Value> = scored
        .into_iter()
        .take(HELP_SECTIONS_MAX)
        .map(|(i, _)| {
            let (title, text) = &help_sections()[i];
            let case = help_case(i, title);
            let excerpts: Vec<Value> = crate::gather::sentences(title, text)
                .into_iter()
                .take(HELP_SENTENCES_MAX)
                .map(|s| json!({ "id": reading.register(&case, &s), "text": s }))
                .collect();
            json!({ "section": title, "excerpts": excerpts })
        })
        .collect();
    json!({ "sections": sections })
}

fn valid_day(day: &str) -> bool {
    let b = day.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && day
            .chars()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// The cases of the space changed between two local days, one line each, with counts.
fn list_changed(
    conn: &Connection,
    space: Option<&str>,
    reading: &mut Reading,
    args: &Value,
) -> Result<Value, String> {
    let since = args["since"].as_str().unwrap_or_default();
    let until = args["until"].as_str().unwrap_or_default();
    if !valid_day(since) || !valid_day(until) {
        return Ok(json!({ "error": "since and until are days written YYYY-MM-DD" }));
    }
    let projects = ask::project_names(conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT n.id, n.title, n.note_type, n.status, n.updated_at, n.due_at, n.project_id,
                    date(n.created_at, 'localtime') BETWEEN ?2 AND ?3
               FROM notes n JOIN projects p ON p.id = n.project_id
              WHERE n.deleted_at IS NULL AND n.archived_at IS NULL
                AND n.content_kind != 'sketch'
                AND date(n.updated_at, 'localtime') BETWEEN ?2 AND ?3
                AND NOT EXISTS (SELECT 1 FROM case_provenance cp
                                 WHERE cp.note_id = n.id AND cp.kind IN ('ask', 'reflect'))
                AND (?1 IS NULL OR p.space = ?1 OR (?1 = 'unassigned' AND p.space IS NULL))
              ORDER BY n.updated_at DESC",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map(params![space, since, until], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, bool>(7)?,
            ))
        })
        .map_err(crate::db::db_error)?;
    let (mut changed, mut created, mut done) = (0usize, 0usize, 0usize);
    let mut lines = Vec::new();
    for row in rows {
        let (id, title, nature, status, updated, due, project_id, new) =
            row.map_err(crate::db::db_error)?;
        changed += 1;
        created += usize::from(new);
        done += usize::from(status == "resolved");
        if lines.len() < CHANGED_MAX {
            let case = Case {
                decision: ask::decision_of(conn, &id, &status),
                project: projects.get(&project_id).cloned().unwrap_or_default(),
                nature,
                status,
                updated: updated.chars().take(10).collect(),
                due,
                sentences: vec![title.clone()],
                id,
                title,
            };
            lines.push(ask::case_for_tool(reading, &case));
        }
    }
    Ok(
        json!({ "since": since, "until": until, "changed": changed, "created": created,
        "done": done, "cases": lines }),
    )
}

/// A case of "To tidy" as the reading registers it: its title and the passage the item names.
fn tidy_case(reading: &mut Reading, case: &crate::tidy::CaseRef, passage: Option<&str>) -> Value {
    let mut sentences = vec![case.title.clone()];
    if let Some(p) = passage.filter(|p| !p.trim().is_empty()) {
        sentences.push(p.to_string());
    } else if !case.excerpt.is_empty() {
        sentences.push(case.excerpt.clone());
    }
    let case = Case {
        id: case.id.clone(),
        title: case.title.clone(),
        project: case.project_name.clone(),
        nature: case.nature.as_str().to_string(),
        status: String::new(),
        decision: String::new(),
        updated: case.date.chars().take(10).collect(),
        due: None,
        sentences,
    };
    let number = reading.number_of(&case);
    let excerpts: Vec<Value> = case
        .sentences
        .iter()
        .map(|s| json!({ "id": reading.register(&case, s), "text": s }))
        .collect();
    json!({ "case": number, "project": case.project, "date": case.updated, "excerpts": excerpts })
}

/// What "To tidy" proposes in the space, each item with its cases registered for citation.
fn list_tidy(
    conn: &Connection,
    space: Option<&str>,
    reading: &mut Reading,
) -> Result<Value, String> {
    let items = crate::tidy::pending(conn, space)?;
    let total = items.len();
    let listed: Vec<Value> = items
        .iter()
        .take(TIDY_MAX)
        .map(|item| {
            let what = match item.kind {
                crate::tidy::TidyKind::Archive => "archive: done and quiet for weeks",
                crate::tidy::TidyKind::Supersede => "supersede: `case` is an older decision `other` may replace",
                crate::tidy::TidyKind::Group => "group: file `case` inside `other`",
                crate::tidy::TidyKind::Conflict => "conflict: `case` may go against the decision `other`",
            };
            json!({
                "tidy": item.id,
                "what": what,
                "case": tidy_case(reading, &item.case, item.passage.as_deref()),
                "other": item.other.as_ref().map(|o| tidy_case(reading, o, item.other_passage.as_deref())),
                "to_check": item.doubt,
            })
        })
        .collect();
    Ok(json!({ "total": total, "items": listed }))
}

/// Runs a tool of the assistant, or `None` for a shared tool of `ask`.
fn run_tool(
    conn: &Connection,
    space: Option<&str>,
    reading: &mut Reading,
    name: &str,
    args: &Value,
) -> Option<Result<Value, String>> {
    match name {
        "list_changed" => Some(list_changed(conn, space, reading, args)),
        "list_tidy" => Some(list_tidy(conn, space, reading)),
        "help" => Some(Ok(help(
            reading,
            args["query"].as_str().unwrap_or_default(),
        ))),
        _ => None,
    }
}

fn reply_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "language": { "type": "string" },
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
            "question": { "type": ["string", "null"] },
            "actions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "kind": { "type": "string",
                            "enum": ["create_sub_case", "germinate", "archive", "move", "tidy"] },
                        "case": { "type": ["integer", "null"] },
                        "title": { "type": ["string", "null"] },
                        "project": { "type": ["string", "null"] },
                        "tidy": { "type": ["string", "null"] }
                    },
                    "required": ["kind", "case", "title", "project", "tidy"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["language", "sentences", "question", "actions"],
        "additionalProperties": false
    })
}

fn tools() -> Value {
    let mut tools = ask::agent_tools_with(ask::agent_tool(
        "reply",
        "Give your reply to the person.",
        reply_schema(),
    ));
    let extra = [
        ask::agent_tool(
            "list_changed",
            "List the cases changed between two days, both included, with counts.",
            json!({ "type": "object",
                "properties": { "since": { "type": "string" }, "until": { "type": "string" } },
                "required": ["since", "until"], "additionalProperties": false }),
        ),
        ask::agent_tool(
            "list_tidy",
            "List what Humcase found to tidy: cases to archive, replaced decisions, cases to file together, cases against a decision.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        ask::agent_tool(
            "help",
            "Read Humcase's documentation on a subject (query in English words).",
            json!({ "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"], "additionalProperties": false }),
        ),
    ];
    if let Some(list) = tools.as_array_mut() {
        // The final `reply` stays last.
        let at = list.len().saturating_sub(1);
        for (i, tool) in extra.into_iter().enumerate() {
            list.insert(at + i, tool);
        }
    }
    tools
}

// ---- The reply and its check ----------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawSentence {
    text: String,
    excerpt: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawAction {
    kind: String,
    #[serde(default)]
    case: Option<i64>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    tidy: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawReply {
    #[serde(default)]
    sentences: Vec<RawSentence>,
    #[serde(default)]
    question: Option<String>,
    #[serde(default)]
    actions: Vec<RawAction>,
}

fn parse_reply(value: Value) -> Result<RawReply, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Text the AI wrote, cleaned of references to the message, or nothing.
fn clean(text: &str, max: usize) -> Option<String> {
    let text = ask::clip(&ask::without_references(text), max);
    (!text.is_empty()).then_some(text)
}

/// The case numbered `number`, when it is one the reply may act on (not a help section).
fn acted_case(reading: &Reading, number: Option<i64>) -> Option<&Case> {
    let case = reading.case_numbered(usize::try_from(number?).ok()?)?;
    (!case.id.starts_with(HELP_ID)).then_some(case)
}

/// The project named `name` (exact, case aside) in the space, not archived.
/// The names of the live projects of the space, for `move`.
fn project_list(conn: &Connection, space: Option<&str>) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for project in crate::db::queries::get_projects(conn).map_err(crate::db::db_error)? {
        if project.archived_at.is_none() && ask::in_space(conn, space, &project.id)? {
            names.push(project.name);
        }
    }
    Ok(names)
}

fn project_named(
    conn: &Connection,
    space: Option<&str>,
    name: &str,
) -> Result<Option<(String, String)>, String> {
    let name = name.trim().to_lowercase();
    if name.is_empty() {
        return Ok(None);
    }
    for project in crate::db::queries::get_projects(conn).map_err(crate::db::db_error)? {
        if project.archived_at.is_none()
            && project.name.trim().to_lowercase() == name
            && ask::in_space(conn, space, &project.id)?
        {
            return Ok(Some((project.id, project.name)));
        }
    }
    Ok(None)
}

/// Holds an action to what exists: the case must have been shown, the project and the item of
/// "To tidy" must exist; anything else is dropped.
fn check_action(
    conn: &Connection,
    space: Option<&str>,
    reading: &Reading,
    tidy: &[crate::tidy::TidyItem],
    raw: RawAction,
) -> Result<Option<Action>, String> {
    Ok(match raw.kind.as_str() {
        "create_sub_case" => {
            let Some(case) = acted_case(reading, raw.case) else {
                return Ok(None);
            };
            let Some(title) = raw
                .title
                .as_deref()
                .and_then(|t| clean(t, SUB_CASE_TITLE_MAX))
            else {
                return Ok(None);
            };
            Some(Action::CreateSubCase {
                case_id: case.id.clone(),
                case_title: case.title.clone(),
                title,
            })
        }
        "germinate" => acted_case(reading, raw.case).map(|case| Action::Germinate {
            case_id: case.id.clone(),
            case_title: case.title.clone(),
        }),
        "archive" => acted_case(reading, raw.case).map(|case| Action::Archive {
            case_id: case.id.clone(),
            case_title: case.title.clone(),
        }),
        "move" => {
            let Some(case) = acted_case(reading, raw.case) else {
                return Ok(None);
            };
            let Ok(note) = crate::db::queries::get_note_by_id(conn, &case.id) else {
                return Ok(None);
            };
            match project_named(conn, space, raw.project.as_deref().unwrap_or_default())? {
                Some((project_id, project_name)) if project_id != note.project_id => {
                    Some(Action::Move {
                        case_id: case.id.clone(),
                        case_title: case.title.clone(),
                        project_id,
                        project_name,
                        from_project_id: note.project_id,
                    })
                }
                _ => None,
            }
        }
        "tidy" => {
            let wanted = raw.tidy.as_deref().unwrap_or_default();
            tidy.iter()
                .find(|item| item.id == wanted)
                .map(|item| Action::Tidy {
                    tidy_id: item.id.clone(),
                    tidy_kind: item.kind,
                    case_id: item.case.id.clone(),
                    case_title: item.case.title.clone(),
                    other_id: item.other.as_ref().map(|o| o.id.clone()),
                    other_title: item.other.as_ref().map(|o| o.title.clone()),
                    passage: item.passage.clone(),
                    other_passage: item.other_passage.clone(),
                })
        }
        _ => None,
    })
}

/// Holds the reply to what was sent: a sentence naming an excerpt not sent is dropped (every
/// source shown can be checked against its case), the actions to what exists.
fn check(
    raw: RawReply,
    reading: &Reading,
    conn: &Connection,
    space: Option<&str>,
    model_label: Option<String>,
) -> Result<Reply, String> {
    let mut sources: Vec<Source> = Vec::new();
    let mut parts: Vec<Part> = Vec::new();
    let mut dropped = 0;
    for sentence in raw.sentences {
        let Some(text) = clean(&sentence.text, 600) else {
            continue;
        };
        let cite = match sentence.excerpt {
            None => None,
            Some(id) => {
                let found = usize::try_from(id)
                    .ok()
                    .and_then(|id| reading.excerpts.get(&id));
                let Some((note_id, title, passage)) = found else {
                    dropped += 1;
                    continue;
                };
                let help = note_id.starts_with(HELP_ID);
                let key = (!help).then(|| note_id.clone());
                let index = match sources
                    .iter()
                    .position(|s| s.note_id == key && s.title == *title)
                {
                    Some(i) => i,
                    None => {
                        sources.push(Source {
                            note_id: key,
                            title: title.clone(),
                            passages: Vec::new(),
                            help,
                        });
                        sources.len() - 1
                    }
                };
                let source = &mut sources[index];
                if passage != title
                    && !source.passages.contains(passage)
                    && source.passages.len() < ask::PASSAGES_MAX
                {
                    source.passages.push(passage.clone());
                }
                Some(index + 1)
            }
        };
        let text = if parts.is_empty() {
            text
        } else {
            format!(" {text}")
        };
        parts.push(Part { text, cite });
    }
    let question = raw.question.as_deref().and_then(|q| clean(q, 400));
    let tidy = if raw.actions.iter().any(|a| a.kind == "tidy") {
        crate::tidy::pending(conn, space)?
    } else {
        Vec::new()
    };
    let mut actions: Vec<Action> = Vec::new();
    for raw_action in raw.actions {
        if actions.len() >= ACTIONS_MAX {
            break;
        }
        if let Some(action) = check_action(conn, space, reading, &tidy, raw_action)? {
            if !actions.contains(&action) {
                actions.push(action);
            }
        }
    }
    Ok(Reply {
        kind: ReplyKind::Reply,
        parts,
        question,
        sources,
        actions,
        model_label,
        dropped,
    })
}

// ---- The reading and the reply --------------------------------------------------------------

/// What the AI reads: the case open in the interface first (all its sentences), then the cases
/// found for the conversation, then the latest ones.
fn read(
    conn: &Connection,
    message: &str,
    previous: &[ask::PreviousTurn],
    space: Option<&str>,
    context_id: Option<&str>,
    meaning_ids: &[String],
) -> Result<(Reading, Option<usize>), String> {
    let projects = ask::project_names(conn)?;
    let context = match context_id {
        Some(id) => ask::load_case_as(
            conn,
            id,
            space,
            &projects,
            &ask::words_of(message),
            CONTEXT_SENTENCES,
            true,
        )?,
        None => None,
    };
    let query = match (&context, message.trim().is_empty()) {
        (Some(case), true) => case.title.clone(),
        _ => message.to_string(),
    };
    let mut reading = ask::read(conn, &query, previous, space, meaning_ids)?;
    let number = context.map(|case| {
        reading.detailed.retain(|c| c.id != case.id);
        reading.overview.retain(|c| c.id != case.id);
        reading.detailed.insert(0, case);
        1
    });
    Ok((reading, number))
}

/// The message the AI reads.
fn message(
    reading: &mut Reading,
    latest: &str,
    previous: &[Turn],
    context: Option<usize>,
    you_are: &str,
    projects: &[String],
) -> Value {
    let mut message = reading.message(latest, &[]);
    let conversation: Vec<Value> = previous
        .iter()
        .map(|turn| json!({ "person": turn.said, "you": turn.reply }))
        .collect();
    if let Some(object) = message.as_object_mut() {
        object.remove("question");
        object.insert("latest".into(), json!(latest));
        object.insert("conversation".into(), json!(conversation));
        object.insert("context".into(), json!(context));
        object.insert("you_are".into(), json!(you_are));
        object.insert("projects".into(), json!(projects));
    }
    message
}

/// Records what a reply was, numbers only (the person reads the file; nothing leaves).
fn record(db_path: &Path, reply: &Reply, cases_sent: usize, calls: usize, tools: &[String]) {
    let Some(folder) = db_path.parent() else {
        return;
    };
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let tool_names: Vec<&str> = tools
        .iter()
        .map(|t| t.split_whitespace().next().unwrap_or_default())
        .collect();
    let entry = json!({
        "at_unix_ms": millis,
        "prompt": PROMPT_VERSION,
        "calls": calls,
        "tools": tool_names,
        "sentences": reply.parts.len(),
        "cited": reply.parts.iter().filter(|p| p.cite.is_some()).count(),
        "dropped": reply.dropped,
        "sources": reply.sources.len(),
        "question": reply.question.is_some(),
        "actions": reply.actions.len(),
        "cases_sent": cases_sent,
    });
    if let Err(e) =
        crate::dictation_journal::append(&folder.join(JOURNAL_FILE), &entry, ask::JOURNAL_MAX_SIZE)
    {
        eprintln!("assistant reply not recorded ({e})");
    }
}

/// Replies to `message` (empty with `context_id`: the person opened that case to think it
/// through). The database lock is never held during a call to the AI.
#[allow(clippy::too_many_arguments)]
pub async fn reply(
    db: &std::sync::Mutex<Connection>,
    db_path: &Path,
    data_dir: &Path,
    sense: Option<std::sync::Arc<crate::meaning::Sense>>,
    message: &str,
    previous: Vec<Turn>,
    space: Option<String>,
    context_id: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<Reply, String> {
    let latest = ask::clip(message, MESSAGE_MAX);
    let skip = previous.len().saturating_sub(PREVIOUS_MAX);
    let previous: Vec<Turn> = previous
        .into_iter()
        .skip(skip)
        .map(|turn| Turn {
            said: ask::clip(&turn.said, MESSAGE_MAX),
            reply: ask::clip(&turn.reply, PREVIOUS_REPLY_MAX),
            source_ids: turn.source_ids,
        })
        .collect();
    // The reading searches with the last exchange too, as "Ask Humcase" does.
    let as_ask: Vec<ask::PreviousTurn> = previous
        .iter()
        .map(|turn| ask::PreviousTurn {
            question: turn.said.clone(),
            answer: turn.reply.clone(),
            source_ids: turn.source_ids.clone(),
        })
        .collect();
    let providers = crate::providers::providers_info().cleanup;
    let chosen = provider_and_model(&providers, provider.as_deref(), model)?;
    let Some((provider_id, model, label)) = chosen else {
        return Ok(Reply::empty(ReplyKind::NoAi, None));
    };
    if latest.is_empty() && context_id.is_none() {
        return Ok(Reply::empty(ReplyKind::Reply, Some(label)));
    }
    let meaning_query = match as_ask.last() {
        Some(last) => format!("{} {latest}", last.question),
        None => latest.clone(),
    };
    let meaning_ids = ask::meaning_hits(sense, data_dir, meaning_query).await;
    let (mut reading, context, projects) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let (reading, context) = read(
            &conn,
            &latest,
            &as_ask,
            space.as_deref(),
            context_id.as_deref(),
            &meaning_ids,
        )?;
        (reading, context, project_list(&conn, space.as_deref())?)
    };
    // The case opened to think it through is gone (trash, drawing): nothing to restate.
    if latest.is_empty() && context.is_none() {
        return Ok(Reply::empty(ReplyKind::Reply, Some(label)));
    }
    let first = self::message(&mut reading, &latest, &previous, context, &label, &projects);
    let tools = tools();
    let query = format!(
        "{latest} {}",
        previous.last().map(|t| t.said.as_str()).unwrap_or_default()
    );
    let started = std::time::Instant::now();
    let outcome = ask::run_agent_with(
        db,
        space.as_deref(),
        &query,
        &mut reading,
        SYSTEM_PROMPT,
        first.clone(),
        "reply",
        CALLS_MAX,
        REPLY_DEADLINE,
        parse_reply,
        |conn, reading, name, args| run_tool(conn, space.as_deref(), reading, name, args),
        |messages, choice| {
            ask::chat_turn(
                messages,
                choice,
                &tools,
                provider_id.clone(),
                model.clone(),
                db_path,
                crate::ai_usage::operation::ASSISTANT,
                None,
            )
        },
    )
    .await;
    let (raw, calls, used) = match outcome {
        Ok(done) => (done.raw, done.calls, done.tools),
        Err(ask::AgentFailure::Failed(error)) => return Err(error),
        // A provider without tools: one call on the first reading.
        Err(ask::AgentFailure::ToolsRefused) => {
            let left = REPLY_DEADLINE.saturating_sub(started.elapsed());
            let text = tokio::time::timeout(
                left,
                crate::commands::ai::call_llm_json(
                    SYSTEM_PROMPT,
                    first,
                    Some(provider_id),
                    model,
                    "reply",
                    reply_schema(),
                    db_path,
                    crate::ai_usage::operation::ASSISTANT,
                ),
            )
            .await
            .map_err(|_| ask::deadline_error(REPLY_DEADLINE))??;
            let value = ask::json_object_in(&text)
                .ok_or_else(|| crate::tr!("ask.unreadable", error = "no JSON object"))?;
            (
                parse_reply(value).map_err(|e| crate::tr!("ask.unreadable", error = e))?,
                1,
                Vec::new(),
            )
        }
    };
    let reply = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        check(raw, &reading, &conn, space.as_deref(), Some(label))?
    };
    record(db_path, &reply, reading.case_count(), calls, &used);
    Ok(reply)
}

/// The model that would answer, the cases of the space, the results waiting for a decision and
/// the proposals of "To tidy".
pub fn status(
    conn: &Connection,
    space: Option<&str>,
    provider: Option<&str>,
    model: Option<String>,
) -> Result<Status, String> {
    let providers = crate::providers::providers_info().cleanup;
    // A chosen AI without a key shows no model (the reply itself says why).
    let model_label = provider_and_model(&providers, provider, model)
        .ok()
        .flatten()
        .map(|(_, _, label)| label);
    let mut stmt = conn
        .prepare(
            "SELECT n.id, n.status FROM notes n JOIN projects p ON p.id = n.project_id
              WHERE n.deleted_at IS NULL AND n.archived_at IS NULL
                AND NOT EXISTS (SELECT 1 FROM case_provenance cp
                                 WHERE cp.note_id = n.id AND cp.kind IN ('ask', 'reflect'))
                AND (?1 IS NULL OR p.space = ?1 OR (?1 = 'unassigned' AND p.space IS NULL))",
        )
        .map_err(crate::db::db_error)?;
    let rows: Vec<(String, String)> = stmt
        .query_map([space], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(crate::db::db_error)?
        .collect::<Result<_, _>>()
        .map_err(crate::db::db_error)?;
    let to_decide = rows
        .iter()
        .filter(|(id, status)| {
            status == "resolved" && ask::decision_of(conn, id, status) == "to_review"
        })
        .count();
    let tidy = crate::tidy::pending(conn, space)?;
    let conflicts = tidy
        .iter()
        .filter(|item| {
            matches!(
                item.kind,
                crate::tidy::TidyKind::Conflict | crate::tidy::TidyKind::Supersede
            )
        })
        .count();
    Ok(Status {
        model_label,
        cases: rows.len(),
        to_decide,
        conflicts,
        to_tidy: tidy.len() - conflicts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ask::tests::{add, db, project};
    use crate::models::project::ProjectSpace;

    fn raw(value: Value) -> RawReply {
        parse_reply(value).unwrap()
    }

    #[test]
    fn a_model_id_reads_as_its_name() {
        assert_eq!(model_label("claude-haiku-5-5"), "Claude Haiku 5.5");
        assert_eq!(model_label("mistral-large-4"), "Mistral Large 4");
        assert_eq!(model_label("openai/gpt-4o-mini"), "GPT 4o Mini");
        assert_eq!(model_label("gpt-oss-120b"), "GPT OSS 120b");
    }

    #[test]
    fn the_documentation_is_read_by_sections_and_cited_word_for_word() {
        assert!(help_sections().len() > 10);
        assert!(help_sections().iter().any(|(t, _)| t == "Inbox"));
        let mut reading = Reading::default();
        let found = help(&mut reading, "inbox file a thought in a project");
        assert_eq!(found["sections"][0]["section"], "Inbox");
        let first = found["sections"][0]["excerpts"][1]["id"].as_u64().unwrap() as usize;
        let (id, title, _) = &reading.excerpts[&first];
        assert!(id.starts_with(HELP_ID));
        assert_eq!(title, "Inbox");
        let none = help(&mut reading, "zzqx");
        assert!(none["titles"].as_array().unwrap().len() > 10);
    }

    // Removing the excerpt check would show a source the AI invented.
    #[test]
    fn a_sentence_citing_an_excerpt_not_sent_is_dropped_and_help_is_its_own_source() {
        let conn = db();
        let work = project(&conn, "Shop", ProjectSpace::Work);
        let checkout = add(
            &conn,
            &work,
            "Checkout",
            "Payment happens on a single page.",
        );
        let mut reading = ask::read(&conn, "payment page", &[], Some("work"), &[]).unwrap();
        reading.message("payment page", &[]);
        let help_found = help(&mut reading, "inbox");
        let help_id = help_found["sections"][0]["excerpts"][1]["id"]
            .as_i64()
            .unwrap();
        let payment = reading
            .excerpts
            .iter()
            .find(|(_, (_, _, text))| text == "Payment happens on a single page.")
            .map(|(id, _)| *id as i64)
            .unwrap();
        let reply = check(
            raw(json!({ "sentences": [
                { "text": "Payment is on one page.", "excerpt": payment },
                { "text": "I would keep it.", "excerpt": null },
                { "text": "Invented (excerpt 99).", "excerpt": 999 },
                { "text": "The Inbox holds thoughts.", "excerpt": help_id }
            ], "question": "Which provider?", "actions": [] })),
            &reading,
            &conn,
            Some("work"),
            None,
        )
        .unwrap();
        assert_eq!(reply.dropped, 1);
        assert_eq!(reply.parts.len(), 3);
        assert_eq!(reply.parts[0].cite, Some(1));
        assert_eq!(reply.parts[1].cite, None);
        assert_eq!(reply.sources[0].note_id.as_deref(), Some(checkout.as_str()));
        assert_eq!(
            reply.sources[0].passages,
            ["Payment happens on a single page."]
        );
        assert!(reply.sources[1].help && reply.sources[1].note_id.is_none());
        assert_eq!(reply.question.as_deref(), Some("Which provider?"));
    }

    // Removing the action check would let the AI act on a case it was never shown, or file a
    // case in a project that does not exist or belongs to another space.
    #[test]
    fn actions_hold_only_to_cases_shown_and_projects_that_exist() {
        let conn = db();
        let work = project(&conn, "Shop", ProjectSpace::Work);
        let other = project(&conn, "Marketing", ProjectSpace::Work);
        project(&conn, "Home", ProjectSpace::Private);
        let checkout = add(
            &conn,
            &work,
            "Checkout",
            "Payment happens on a single page.",
        );
        let mut reading = ask::read(&conn, "checkout payment", &[], Some("work"), &[]).unwrap();
        reading.message("checkout payment", &[]);
        help(&mut reading, "inbox");
        let shown = reading
            .detailed
            .iter()
            .position(|c| c.id == checkout)
            .unwrap() as i64
            + 1;
        let reply = check(
            raw(json!({ "sentences": [], "question": null, "actions": [
                { "kind": "create_sub_case", "case": shown, "title": "Pick the provider", "project": null, "tidy": null },
                { "kind": "create_sub_case", "case": 99, "title": "Nowhere", "project": null, "tidy": null },
                { "kind": "germinate", "case": shown, "title": null, "project": null, "tidy": null },
                { "kind": "move", "case": shown, "title": null, "project": "marketing", "tidy": null },
                { "kind": "move", "case": shown, "title": null, "project": "Home", "tidy": null },
                { "kind": "tidy", "case": null, "title": null, "project": null, "tidy": "nothing" }
            ] })),
            &reading,
            &conn,
            Some("work"),
            None,
        )
        .unwrap();
        assert_eq!(
            reply.actions,
            vec![
                Action::CreateSubCase {
                    case_id: checkout.clone(),
                    case_title: "Checkout".into(),
                    title: "Pick the provider".into()
                },
                Action::Germinate {
                    case_id: checkout.clone(),
                    case_title: "Checkout".into()
                },
                Action::Move {
                    case_id: checkout.clone(),
                    case_title: "Checkout".into(),
                    project_id: other,
                    project_name: "Marketing".into(),
                    from_project_id: work
                }
            ]
        );
    }

    #[test]
    fn the_open_case_comes_first_with_all_its_sentences() {
        let conn = db();
        let work = project(&conn, "Shop", ProjectSpace::Work);
        add(
            &conn,
            &work,
            "Payment provider",
            "Stripe or Mollie for payment.",
        );
        let open = add(
            &conn,
            &work,
            "Gift cards",
            "Three amounts. Sent by email. Valid one year.",
        );
        let (reading, context) = read(&conn, "", &[], Some("work"), Some(&open), &[]).unwrap();
        assert_eq!(context, Some(1));
        assert_eq!(reading.detailed[0].id, open);
        assert!(reading.detailed[0].sentences.len() >= 3);
        assert_eq!(reading.detailed.iter().filter(|c| c.id == open).count(), 1);
    }

    // The person chose the case: "Think it through" on an archived case, a kept answer or a case
    // of another space restated nothing ("no case is open") before.
    #[test]
    fn the_open_case_is_read_even_archived_kept_or_in_another_space() {
        let conn = db();
        let work = project(&conn, "Shop", ProjectSpace::Work);
        let home = project(&conn, "Home", ProjectSpace::Private);
        let archived = add(&conn, &work, "Old supplier", "We left Ceradel in May.");
        conn.execute(
            "UPDATE notes SET archived_at = datetime('now') WHERE id = ?1",
            [&archived],
        )
        .unwrap();
        let kept = add(&conn, &work, "Answer kept", "Payment is on one page.");
        conn.execute(
            "INSERT INTO case_provenance (id, note_id, kind, label, created_at)
             VALUES ('p1', ?1, 'ask', 'Humcase', datetime('now'))",
            [&kept],
        )
        .unwrap();
        let private = add(&conn, &home, "Plumber", "Friday morning.");
        for id in [&archived, &kept, &private] {
            let (reading, context) = read(&conn, "", &[], Some("work"), Some(id), &[]).unwrap();
            assert_eq!(context, Some(1), "{id}");
            assert_eq!(&reading.detailed[0].id, id);
        }
    }

    #[test]
    fn a_period_lists_the_cases_changed_in_it_with_counts() {
        let conn = db();
        let work = project(&conn, "Shop", ProjectSpace::Work);
        let home = project(&conn, "Home", ProjectSpace::Private);
        add(&conn, &work, "Checkout", "One page.");
        add(&conn, &home, "Plumber", "Friday.");
        let today: String = conn
            .query_row("SELECT date('now', 'localtime')", [], |r| r.get(0))
            .unwrap();
        let mut reading = Reading::default();
        let listed = list_changed(
            &conn,
            Some("work"),
            &mut reading,
            &json!({ "since": today, "until": today }),
        )
        .unwrap();
        assert_eq!(listed["changed"], 1);
        assert_eq!(listed["created"], 1);
        assert_eq!(listed["cases"][0]["excerpts"][0]["text"], "Checkout");
        let bad = list_changed(
            &conn,
            None,
            &mut reading,
            &json!({ "since": "lundi", "until": today }),
        )
        .unwrap();
        assert!(bad["error"].is_string());
    }

    // `list_changed` has its own SQL: it must refuse a drawing as `ask::load_case` does.
    #[test]
    fn a_period_never_lists_a_sketch() {
        let conn = db();
        let work = project(&conn, "Shop", ProjectSpace::Work);
        add(&conn, &work, "Checkout", "One page.");
        let drawing = add(&conn, &work, "Wireframe", "{}");
        conn.execute(
            "UPDATE notes SET content_kind = 'sketch' WHERE id = ?1",
            [&drawing],
        )
        .unwrap();
        let today: String = conn
            .query_row("SELECT date('now', 'localtime')", [], |r| r.get(0))
            .unwrap();
        let mut reading = Reading::default();
        let listed = list_changed(
            &conn,
            Some("work"),
            &mut reading,
            &json!({ "since": today, "until": today }),
        )
        .unwrap();
        assert_eq!(listed["changed"], 1);
        assert_eq!(listed["cases"].as_array().unwrap().len(), 1);
        assert_eq!(listed["cases"][0]["excerpts"][0]["text"], "Checkout");
    }

    #[test]
    fn a_chosen_ai_without_a_key_is_never_replaced_by_another() {
        let info = |id: &str, ready: bool| crate::providers::ProviderInfo {
            id: id.into(),
            label: id.to_uppercase(),
            key_env: String::new(),
            default_model: String::new(),
            url_env: String::new(),
            model_env: String::new(),
            needs_key: true,
            key_defined: ready,
            key_source: crate::ai_settings::KeyProvenance::None,
            url_editable: false,
            url: String::new(),
            ready,
        };
        let providers = [info("cerebras", false), info("groq", true)];
        assert!(provider_and_model(&providers, Some("cerebras"), None).is_err());
        let (id, _, _) = provider_and_model(&providers, None, None).unwrap().unwrap();
        assert_eq!(id, "groq");
    }

    #[test]
    fn status_counts_the_cases_of_the_space() {
        let conn = db();
        let work = project(&conn, "Shop", ProjectSpace::Work);
        let home = project(&conn, "Home", ProjectSpace::Private);
        add(&conn, &work, "Checkout", "One page.");
        add(&conn, &work, "Photos", "1600 px.");
        add(&conn, &home, "Plumber", "Friday.");
        let status = status(&conn, Some("work"), None, None).unwrap();
        assert_eq!(status.cases, 2);
        assert_eq!(status.to_decide, 0);
        assert_eq!(status.conflicts, 0);
        assert_eq!(status.to_tidy, 0);
    }

    #[test]
    fn status_counts_the_disagreements_apart_from_the_rest_of_to_tidy() {
        use crate::gather::{store, Candidate, Check, Judged};
        let conn = db();
        let work = project(&conn, "Shop", ProjectSpace::Work);
        let home = project(&conn, "Home", ProjectSpace::Private);
        let decision = add(&conn, &work, "Never show a price", "No price.");
        let task = add(&conn, &work, "Show the cost in dollars", "In dollars.");
        let near = add(&conn, &work, "Cost page", "The cost.");
        let elsewhere = add(&conn, &home, "Plumber", "Friday.");
        let other = add(&conn, &home, "Plumber again", "Friday.");
        let judged = |check, note_id: &str, near_id: &str| Judged {
            candidate: Candidate {
                check,
                note_id: note_id.into(),
                near_id: near_id.into(),
                duplicate: false,
                closeness: 0.9,
            },
            chance: Some(0.9),
            passages: None,
        };
        store(
            &conn,
            &[
                judged(Check::Conflict, &task, &decision),
                judged(Check::Together, &near, &task),
                judged(Check::Conflict, &other, &elsewhere),
            ],
        )
        .unwrap();
        let work_status = status(&conn, Some("work"), None, None).unwrap();
        assert_eq!((work_status.conflicts, work_status.to_tidy), (1, 1));
        let home_status = status(&conn, Some("private"), None, None).unwrap();
        assert_eq!((home_status.conflicts, home_status.to_tidy), (1, 0));
    }

    #[test]
    fn the_tools_end_with_reply() {
        let tools = tools();
        let names: Vec<&str> = tools
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(names.last(), Some(&"reply"));
        for name in [
            "search_cases",
            "read_case",
            "list_cases",
            "list_changed",
            "list_tidy",
            "help",
        ] {
            assert!(names.contains(&name), "{name}");
        }
    }

    /// Words of the informal French address, which the interface never uses.
    const FRENCH_INFORMAL: [&str; 6] = ["tu", "ton", "ta", "tes", "toi", "te"];

    /// Words of the data the reply must never show ("excerpt 3", "(2)", "#1").
    fn internal_words(lower: &str) -> Vec<String> {
        let mut found: Vec<String> = [
            "excerpt",
            "extrait",
            "case number",
            "numéro du cas",
            "note_id",
        ]
        .iter()
        .filter(|w| lower.contains(*w))
        .map(|w| w.to_string())
        .collect();
        let chars: Vec<char> = lower.chars().collect();
        for pair in chars.windows(2) {
            if matches!(pair[0], '(' | '[' | '#') && pair[1].is_ascii_digit() {
                found.push(format!("{}{}", pair[0], pair[1]));
            }
        }
        found
    }

    /// Measures the assistant with real calls on the bases of `ask_eval_cases.json`: their
    /// questions, plus the situations of `assistant_eval_cases.json` (the model it runs on, the
    /// documentation, a period, the opening of a case, actions asked for or agreed to). Per
    /// reply: cases cited against the expected ones, facts held, phrases that would turn a
    /// proposal into a decision, words of the data, informal French, the documentation and the
    /// tool used when expected, the action proposed when asked and none otherwise, the question
    /// at the opening of a case, sentences dropped by the check, calls, seconds, tokens and
    /// estimated cost. Provider `ASSISTANT_EVAL_PROVIDER` (Anthropic), model
    /// `ASSISTANT_EVAL_MODEL` (Claude Haiku 5.5 for Anthropic); the bench stops once
    /// `ASSISTANT_EVAL_MAX_USD` (2) is spent; `ASSISTANT_EVAL_BASES` keeps only those bases;
    /// results to `ASSISTANT_EVAL_OUT`. Search by meaning is not used.
    /// `cargo test assistant_eval -- --ignored --nocapture`
    #[test]
    #[ignore = "real AI calls"]
    fn assistant_eval() {
        use crate::ask::tests::{build_base, usage_totals};
        let bench: Value = serde_json::from_str(include_str!("ask_eval_cases.json")).unwrap();
        let situations: Value =
            serde_json::from_str(include_str!("assistant_eval_cases.json")).unwrap();
        let provider =
            std::env::var("ASSISTANT_EVAL_PROVIDER").unwrap_or_else(|_| "anthropic".into());
        let model = std::env::var("ASSISTANT_EVAL_MODEL")
            .ok()
            .or_else(|| (provider == "anthropic").then(|| "claude-haiku-5-5".into()));
        let max_usd: f64 = std::env::var("ASSISTANT_EVAL_MAX_USD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2.0);
        let folder = tempfile::tempdir().unwrap();
        let db_path = folder.path().join("eval.db");
        let journal = folder.path().join(JOURNAL_FILE);
        let mut results = Vec::new();
        let mut totals: std::collections::HashMap<&str, f64> = std::collections::HashMap::new();
        let mut count = |name: &'static str, value: f64| *totals.entry(name).or_default() += value;
        let mut spent = 0.0;
        'bases: for base in bench["bases"].as_array().unwrap() {
            let name = base["name"].as_str().or(base["lang"].as_str()).unwrap();
            if let Ok(only) = std::env::var("ASSISTANT_EVAL_BASES") {
                if !only.split(',').any(|n| n == name) {
                    continue;
                }
            }
            let french = base["lang"] == "fr";
            let extra = situations["scenarios"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["base"] == name);
            let conn = db();
            let ids = build_base(&conn, base);
            for project_name in extra
                .and_then(|s| s["projects"].as_array())
                .into_iter()
                .flatten()
            {
                project(&conn, project_name.as_str().unwrap(), ProjectSpace::Work);
            }
            let conn = std::sync::Mutex::new(conn);
            let key_of = |id: &str| {
                ids.iter()
                    .find(|(_, v)| v.as_str() == id)
                    .map(|(k, _)| k.clone())
                    .unwrap_or_else(|| id.to_string())
            };
            let items: Vec<&Value> = base["questions"]
                .as_array()
                .unwrap()
                .iter()
                .chain(
                    extra
                        .and_then(|s| s["items"].as_array())
                        .into_iter()
                        .flatten(),
                )
                .collect();
            for item in items {
                if spent >= max_usd {
                    println!("\nstopped: {spent:.3} $ spent");
                    break 'bases;
                }
                let said = item["q"].as_str().unwrap();
                let previous: Vec<Turn> = item["previous"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| Turn {
                        said: p["said"]
                            .as_str()
                            .or(p["question"].as_str())
                            .unwrap()
                            .into(),
                        reply: p["reply"].as_str().or(p["answer"].as_str()).unwrap().into(),
                        source_ids: p["cites"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|k| ids[k.as_str().unwrap()].clone())
                            .collect(),
                    })
                    .collect();
                let context_id = item["context"].as_str().map(|k| ids[k].clone());
                let before = usage_totals(&db_path);
                let started = std::time::Instant::now();
                // A burst refused (HTTP 429): wait and ask again, never count it.
                let mut outcome = Err(String::new());
                for wait in [0u64, 5, 10, 20, 40] {
                    std::thread::sleep(std::time::Duration::from_secs(wait));
                    outcome = tauri::async_runtime::block_on(reply(
                        &conn,
                        &db_path,
                        folder.path(),
                        None,
                        said,
                        previous.clone(),
                        Some("work".into()),
                        context_id.clone(),
                        Some(provider.clone()),
                        model.clone(),
                    ));
                    if !outcome.as_ref().is_err_and(|e| e.contains("429")) {
                        break;
                    }
                }
                let seconds = started.elapsed().as_secs_f64();
                let after = usage_totals(&db_path);
                let cost = after.2 - before.2;
                spent += cost;
                count("replies", 1.0);
                count("seconds", seconds);
                count("input_tokens", (after.0 - before.0) as f64);
                count("output_tokens", (after.1 - before.1) as f64);
                count("cost_usd", cost);
                let answer = match outcome {
                    Ok(answer) => answer,
                    Err(e) => {
                        println!("\n== {name} «{said}»: ERROR {e}");
                        count("errors", 1.0);
                        continue;
                    }
                };
                // The tools of this reply, from the journal it just wrote.
                let line: Value = std::fs::read_to_string(&journal)
                    .ok()
                    .and_then(|text| text.lines().last().map(str::to_string))
                    .and_then(|line| serde_json::from_str(&line).ok())
                    .unwrap_or(Value::Null);
                let tools: Vec<String> = line["tools"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|t| t.as_str().map(str::to_string))
                    .collect();
                count("calls", line["calls"].as_f64().unwrap_or(0.0));
                let mut text: String = answer
                    .parts
                    .iter()
                    .map(|p| p.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Some(question) = &answer.question {
                    text.push_str(" ? ");
                    text.push_str(question);
                }
                let lower = text.to_lowercase().replace(['\u{a0}', '\u{202f}'], " ");
                let mut faults: Vec<String> = Vec::new();
                let cited: Vec<String> = answer
                    .sources
                    .iter()
                    .filter_map(|s| s.note_id.as_deref())
                    .map(key_of)
                    .collect();
                let expect: Vec<String> = item["expect"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|k| k.as_str().unwrap().to_string())
                    .collect();
                let missed: Vec<&String> = expect.iter().filter(|k| !cited.contains(k)).collect();
                if !missed.is_empty() {
                    faults.push(format!("missed {missed:?}"));
                }
                if item["not_found"].as_bool() == Some(true) && !cited.is_empty() {
                    faults.push("cites a case for nothing".into());
                }
                if let Some(min) = item["min_sources"].as_u64() {
                    if (cited.len() as u64) < min {
                        faults.push(format!("{} cases cited, {min} expected", cited.len()));
                    }
                }
                for fact in item["must"].as_array().into_iter().flatten() {
                    // `{model}`: the label of the model that answered, whichever was benched.
                    let model = answer.model_label.as_deref().unwrap_or("").to_lowercase();
                    let spellings: Vec<&str> = fact
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|f| f.as_str())
                        .map(|f| if f == "{model}" { model.as_str() } else { f })
                        .filter(|f| !f.is_empty())
                        .collect();
                    let spellings = if spellings.is_empty() {
                        vec!["{model}"]
                    } else {
                        spellings
                    };
                    if !spellings.iter().any(|f| lower.contains(f)) {
                        faults.push(format!("fact missing {}", spellings[0]));
                    }
                }
                let base_phrases = extra.and_then(|s| s["forbid"].as_array());
                let phrases = item["forbid"]
                    .as_array()
                    .into_iter()
                    .chain(base_phrases)
                    .flatten();
                let jargon = base["jargon"].as_array().into_iter().flatten();
                for phrase in phrases.chain(jargon).filter_map(|f| f.as_str()) {
                    if lower.contains(phrase) {
                        faults.push(format!("says «{phrase}»"));
                    }
                }
                for word in internal_words(&lower) {
                    faults.push(format!("data word «{word}»"));
                }
                if french {
                    let informal: Vec<&str> = lower
                        .split(|c: char| !c.is_alphanumeric())
                        .filter(|w| FRENCH_INFORMAL.contains(w))
                        .collect();
                    if !informal.is_empty() {
                        faults.push(format!("informal {informal:?}"));
                    }
                }
                if item["help"].as_bool() == Some(true) && !answer.sources.iter().any(|s| s.help) {
                    faults.push("documentation not cited".into());
                }
                if let Some(tool) = item["tool"].as_str() {
                    if !tools.iter().any(|t| t == tool) {
                        faults.push(format!("{tool} not used"));
                    }
                }
                if item["question"].as_bool() == Some(true) && answer.question.is_none() {
                    faults.push("no question".into());
                }
                let actions: Vec<Value> = answer
                    .actions
                    .iter()
                    .map(|a| serde_json::to_value(a).unwrap())
                    .collect();
                if let Some(wanted) = item.get("action").filter(|a| a.is_object()) {
                    let case = &ids[wanted["case"].as_str().unwrap()];
                    if !actions
                        .iter()
                        .any(|a| a["kind"] == wanted["kind"] && a["case_id"] == case.as_str())
                    {
                        faults.push(format!("no {} on {}", wanted["kind"], wanted["case"]));
                    }
                } else if !actions.is_empty() {
                    faults.push(format!("{} actions not asked for", actions.len()));
                }
                count("dropped", answer.dropped as f64);
                count("sentences", answer.parts.len() as f64);
                if faults.is_empty() {
                    count("clean", 1.0);
                }
                for fault in &faults {
                    count(
                        match fault.split_whitespace().next().unwrap_or_default() {
                            "missed" => "missed_cases",
                            "fact" => "facts_missing",
                            "says" => "phrases",
                            "data" => "data_words",
                            "informal" => "informal",
                            "no" if fault.starts_with("no question") => "questions_missing",
                            "no" => "actions_missing",
                            "documentation" => "documentation_missing",
                            _ if fault.contains("not used") => "tools_missing",
                            _ if fault.contains("not asked") => "actions_unasked",
                            _ => "other",
                        },
                        1.0,
                    );
                }
                println!(
                    "\n== {name} «{said}» ({seconds:.1} s, {} calls) {}",
                    line["calls"],
                    if faults.is_empty() {
                        "ok".to_string()
                    } else {
                        faults.join("; ")
                    }
                );
                println!("   tools {tools:?}");
                println!("   {text}");
                println!(
                    "   cited {cited:?}, actions {actions:?}, dropped {}",
                    answer.dropped
                );
                results.push(json!({
                    "base": name, "said": said, "context": item["context"], "seconds": seconds,
                    "calls": line["calls"], "tools": tools, "text": text, "cited": cited,
                    "help": answer.sources.iter().any(|s| s.help), "actions": actions,
                    "dropped": answer.dropped, "cost_usd": cost, "faults": faults,
                }));
            }
        }
        let mut summary: serde_json::Map<String, Value> = totals
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect();
        summary.insert("prompt".into(), json!(PROMPT_VERSION));
        summary.insert("provider".into(), json!(provider));
        summary.insert("model".into(), json!(model));
        println!("\n{}", serde_json::to_string_pretty(&summary).unwrap());
        if let Ok(out) = std::env::var("ASSISTANT_EVAL_OUT") {
            std::fs::write(
                out,
                serde_json::to_string_pretty(&json!({ "summary": summary, "results": results }))
                    .unwrap(),
            )
            .unwrap();
        }
    }
}
