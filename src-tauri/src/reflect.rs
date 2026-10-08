//! "Think it through": a thinking partner that works from the cases of a space.
//!
//! The person thinks an idea, a problem or a choice through, alone or starting from one case
//! (the subject). At each turn the partner restates what was said, links what the cases already
//! say (at most three links, each with a sentence of the case word for word), proposes one lead
//! when the person hesitates, and asks exactly one question. At the end, `summarize` turns the
//! conversation into a summary the person corrects.
//!
//! It reuses Ask Humcase (`crate::ask`): the same reading of the space, the same registry of
//! numbered excerpts, the same read-only tools in an agent loop (offered when the first reading
//! is weak), and the same safeguard: a link stands only if the excerpt it names was really sent,
//! so a source shown is always a sentence of a case. Kept answers, other spaces, archived cases
//! and sketches are never read.

use crate::ask::{
    agent_tool, agent_tools_with, chat_turn, clip, json_object_in, load_case, meaning_hits,
    project_names, read, reading_is_strong, ready_provider, run_agent_with, state_words,
    without_references, AgentFailure, Case, PreviousTurn, Reading, Sent, Source,
    AGENT_READ_SENTENCES, JOURNAL_MAX_SIZE, PASSAGES_MAX, PREVIOUS_ANSWER_MAX, QUESTION_MAX,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;

/// Provenance of a kept summary: never read back as a source (`crate::ask::OWN_KINDS`).
pub const KEPT_KIND: &str = "reflect";
const KEPT_TITLE_MAX: usize = 200;

/// Local journal of the replies: numbers only (no text), next to the database.
pub const JOURNAL_FILE: &str = "reflect-journal.jsonl";

/// Version of the instruction below, recorded in the journal.
const PROMPT_VERSION: &str = "reflect.v8";

/// Model calls one reply may take, the last one forced to `reply`.
const REPLY_CALLS_MAX: usize = 4;
/// Links one reply carries.
const LINKS_MAX: usize = 3;
/// Turns of the conversation the AI reads (the latest ones).
const TURNS_MAX: usize = 12;
/// Length of a text said by the person, and of a text written by the partner.
const SAID_MAX: usize = QUESTION_MAX + 500;
const SENTENCE_MAX: usize = 400;
const ITEM_MAX: usize = 240;
/// Items in each list of the summary, actions, and cases waited on.
const LIST_MAX: usize = 6;
const ACTIONS_MAX: usize = 8;
const WAITING_MAX: usize = 5;

const REPLY_PROMPT: &str = "You are the thinking partner of a person who is working an idea, a problem or a choice through, using only what they say and the cases (their notes, tasks, decisions) given in the user message as JSON. `subject` is the case they start from when there is one: they already see it, so never cite it. Each case in `cases` has numbered excerpts, the first one being its title; `overview` lists the latest cases of the space in one line each. `conversation` holds the exchange so far, oldest first: `person` is what they said, `you` is what you replied before. When `conversation` is empty, the person has just opened the subject: restate it from its own text and ask a first question. `latest` is the last thing the person said.
Each turn you do four things, in the language of the person, always addressing them as the interface does (French: \"vous\", never \"tu\", even if the person writes \"je\"):
1. `restated`: one or two short sentences restating what the person just said, in their own terms. Add nothing of yours.
2. `links`: at most 3 links to what the cases already say that bears on what the person said, only when the excerpt really says it. `kind`: `supports` (the case goes the same way), `decided` (a decision already taken that bears on it), `conflicts` (it may contradict what the person said or wants). `excerpt` is the id of the excerpt that states it. Never link the subject. No link is better than a forced one: an empty list is fine.
3. `lead`: only when the person is stuck or hesitates between options (not when they simply state a plan), one short lead to follow (a way to look at it, an option to weigh) that adds something the person did not say, drawn from the conversation or the cases; otherwise null.
4. `question`: exactly ONE question, the one that moves the thinking forward the most, built on what the person just said. Never two questions, never a list of questions, and never a question already asked in `conversation` (even in other words) nor one the person already answered: follow where the person goes, do not bring them back to your own idea.
When a direction has clearly emerged from the conversation (the person made their choice, or named their next steps), set `offer_summary` to true and make your question ask whether the person wants a summary of it, instead of opening a new subject; otherwise false.
Rules:
- Address the person directly as the interface does: \"vous\" in French, \"tú\" in Spanish, \"du\" in German, \"você\" in Portuguese, \"tu\" in Italian; keep it for the whole conversation, restatement of the subject included.
- Say that something was chosen, retained or decided only when an excerpt or the person says so; a proposal or a suggestion written in a case is not a choice (\"Stripe is proposed\", never \"Stripe, already chosen\").
- You never decide for the person. A proposal stays a proposal, what is still to decide stays open, a question stays a question. Never say who wrote, proposed or decided something (\"you proposed\") unless the excerpt names them.
- Speak as the person would: name a case only by its subject (\"the choice of the supplier\"), never as \"the case\", and say its state in plain words (\"waits for your decision\", \"in progress\"); never write the words of the data (kind, state, excerpt, case number, supports, decided, conflicts).
- Never state anything the excerpts and the conversation do not say: no outside knowledge, no guess, no advice of your own beyond the lead, no reason, cause or judgment that is not written, and no link between two cases that their text does not make.";

const REPLY_TOOLS_ADDENDUM: &str = "
The cases in the user message are only those whose words are closest to what the person said and the latest ones: the space holds more. You have tools to look for the others:
- search_cases(query): the cases holding the words of `query`, with numbered excerpts. A case only matches the words it contains.
- read_case(case): all the excerpts of one case, by its number.
- list_cases(state): the latest cases that are open, in_progress or waiting_for_decision, one line each.
When the given cases may not hold what bears on what the person said, search before replying: use the words a case about that thing would contain (a synonym, a related word, the name of a document, the unit of a figure), not the person's own words, and search again with other words when a search finds nothing useful; give up only after two searches with different words found nothing. Do not search for what the given cases already cover. Excerpt ids and case numbers from the tools work exactly like those in the user message, and the rules above apply to them. To reply, call the function `reply`: it is the only way to finish, and you have only a few turns.";

const SUMMARY_PROMPT: &str = "You turn a conversation in which a person thought an idea, a problem or a choice through into a summary. Write every text in the language of the conversation (a conversation in French gives a summary entirely in French), whatever the language of these instructions. `subject` is the case they started from (null when they started from nothing). `conversation` holds the exchange, oldest first: `person` is what they said, `you` is what you replied. `cases` and `overview` are cases of their space, each with a `case` number.
Take everything ONLY from what was said in the conversation: never add a fact, reason, option, figure or action nobody said; when unsure, leave it out. Fields:
- `title`: starts with the word for \"Reflection\" followed by a colon, in the person's language (for example \"Réflexion : …\" or \"Reflection: …\"), then the subject of the thinking in a few words.
- `direction`: one or two sentences, the direction the thinking took. If none emerged, say what is still undecided. Never present as decided what the person did not decide.
- `kept`: what the person kept (ideas, options, decisions). `rejected`: what they set aside. `evolution`: how the idea changed during the conversation, each as what it was and what it became, written in the language of the conversation, not a list of the steps taken; an empty list when it did not change. `questions`: only questions raised in the conversation (by the person or by you) that nobody answered later in the conversation, rewritten as a plain question without addressing the person; never a new one of yours, never one the person answered. Short strings; an empty list when there is nothing.
- `actions`: next steps the person named, each a short task title. `create` is true only when the person agreed to do it; false for an idea for later. Never an action that already has a case in `cases`, `overview` or `subject`.
- `waiting_on`: the numbers (`case`) of existing cases the direction depends on, only when the conversation says or shows that dependency; otherwise an empty list.
Write every text as a plain note the person keeps, without a subject pronoun and without addressing the person (a note such as \"The cheapest quote under 4,000\", never \"You chose the cheapest quote\"), in the language of the conversation. Speak as the person would: never write the words of the data (kind, state, excerpt, case number) in the texts.";

/// A turn of the conversation, as the interface sends it: the person's words (`said`) or the
/// partner's previous reply as plain text (`reply`), with the cases that reply linked.
#[derive(Debug, Clone, Deserialize)]
pub struct ReflectPreviousTurn {
    #[serde(default)]
    pub said: Option<String>,
    #[serde(default)]
    pub reply: Option<String>,
    #[serde(default)]
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    Supports,
    Decided,
    Conflicts,
}

impl LinkKind {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "supports" => Some(Self::Supports),
            "decided" => Some(Self::Decided),
            "conflicts" => Some(Self::Conflicts),
            _ => None,
        }
    }
}

/// What a case already says about the thinking, with the sentences of it that say so.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReflectLink {
    pub kind: LinkKind,
    pub source: Source,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct ReflectReply {
    pub restated: Option<String>,
    pub links: Vec<ReflectLink>,
    pub lead: Option<String>,
    pub question: Option<String>,
    pub offer_summary: bool,
    pub sent: Option<Sent>,
    /// Links of the AI dropped: an excerpt that was not sent, the subject, a kind that does not
    /// exist, or beyond the third.
    pub dropped: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SummaryAction {
    pub title: String,
    pub create: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WaitingOn {
    pub note_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct ReflectSummary {
    pub title: String,
    pub direction: String,
    pub kept: Vec<String>,
    pub rejected: Vec<String>,
    pub evolution: Vec<String>,
    pub questions: Vec<String>,
    pub actions: Vec<SummaryAction>,
    pub waiting_on: Vec<WaitingOn>,
    pub sent: Option<Sent>,
}

// ---- What the AI is asked for and what it gives back ----

fn reply_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "restated": { "type": ["string", "null"] },
            "links": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "kind": { "type": "string", "enum": ["supports", "decided", "conflicts"] },
                        "excerpt": { "type": "integer" }
                    },
                    "required": ["kind", "excerpt"],
                    "additionalProperties": false
                }
            },
            "lead": { "type": ["string", "null"] },
            "question": { "type": ["string", "null"] },
            "offer_summary": { "type": "boolean" }
        },
        "required": ["restated", "links", "lead", "question", "offer_summary"],
        "additionalProperties": false
    })
}

fn summary_schema() -> Value {
    let strings = json!({ "type": "array", "items": { "type": "string" } });
    json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "direction": { "type": "string" },
            "kept": strings,
            "rejected": strings,
            "evolution": strings,
            "questions": strings,
            "actions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "create": { "type": "boolean" }
                    },
                    "required": ["title", "create"],
                    "additionalProperties": false
                }
            },
            "waiting_on": { "type": "array", "items": { "type": "integer" } }
        },
        "required": ["title", "direction", "kept", "rejected", "evolution", "questions",
            "actions", "waiting_on"],
        "additionalProperties": false
    })
}

#[derive(Debug, Deserialize)]
struct RawLink {
    kind: String,
    excerpt: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawReply {
    #[serde(default)]
    restated: Option<String>,
    #[serde(default)]
    links: Vec<RawLink>,
    #[serde(default)]
    lead: Option<String>,
    #[serde(default)]
    question: Option<String>,
    #[serde(default)]
    offer_summary: bool,
}

#[derive(Debug, Deserialize)]
struct RawAction {
    title: String,
    #[serde(default)]
    create: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawSummary {
    #[serde(default)]
    title: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    kept: Vec<String>,
    #[serde(default)]
    rejected: Vec<String>,
    #[serde(default)]
    evolution: Vec<String>,
    #[serde(default)]
    questions: Vec<String>,
    #[serde(default)]
    actions: Vec<RawAction>,
    #[serde(default)]
    waiting_on: Vec<i64>,
}

fn parse_reply(value: Value) -> Result<RawReply, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

fn parse_text<T>(text: &str, parse: impl Fn(Value) -> Result<T, String>) -> Result<T, String> {
    match json_object_in(text) {
        Some(value) => parse(value).map_err(|e| crate::tr!("ask.unreadable", error = e)),
        None => Err(crate::tr!("ask.unreadable", error = "no JSON object")),
    }
}

// ---- The checks ----

/// A sentence the partner wrote: without references to the message, short, or nothing.
fn clean(text: Option<&str>, max: usize) -> Option<String> {
    let text = clip(&without_references(text?), max);
    (!text.is_empty()).then_some(text)
}

/// The first question of a text: a partner that asks two questions asks none.
fn first_question(text: &str) -> String {
    match text.find('?') {
        Some(end) => text[..=end].trim().to_string(),
        None => text.trim().to_string(),
    }
}

/// Holds the AI's reply to what was sent. A link stands only if the excerpt it names was really
/// sent and is not a sentence of the subject (already shown); a kind that does not exist or a
/// link beyond the third is dropped; every drop is counted. Two links to one case become one
/// source with both sentences. The question is cut to its first.
fn check_reply(raw: RawReply, reading: &Reading, subject_id: Option<&str>) -> ReflectReply {
    let mut links: Vec<ReflectLink> = Vec::new();
    let mut dropped = 0;
    for link in raw.links {
        let found = link
            .excerpt
            .and_then(|id| usize::try_from(id).ok())
            .and_then(|id| reading.excerpts.get(&id));
        let (Some(kind), Some((note_id, title, passage))) = (LinkKind::parse(&link.kind), found)
        else {
            dropped += 1;
            continue;
        };
        if Some(note_id.as_str()) == subject_id {
            dropped += 1;
            continue;
        }
        if let Some(existing) = links.iter_mut().find(|l| &l.source.note_id == note_id) {
            let passages = &mut existing.source.passages;
            if passage != title && !passages.contains(passage) && passages.len() < PASSAGES_MAX {
                passages.push(passage.clone());
            }
            continue;
        }
        if links.len() >= LINKS_MAX {
            dropped += 1;
            continue;
        }
        links.push(ReflectLink {
            kind,
            source: Source {
                note_id: note_id.clone(),
                title: title.clone(),
                passages: if passage == title {
                    Vec::new()
                } else {
                    vec![passage.clone()]
                },
            },
        });
    }
    ReflectReply {
        restated: clean(raw.restated.as_deref(), SENTENCE_MAX),
        links,
        lead: clean(raw.lead.as_deref(), SENTENCE_MAX),
        question: clean(raw.question.as_deref(), SENTENCE_MAX * 2).map(|q| first_question(&q)),
        offer_summary: raw.offer_summary,
        sent: None,
        dropped,
    }
}

/// A title compared without case, accents' marks aside, or punctuation.
fn normalized(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn clean_list(items: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        if let Some(item) = clean(Some(&item), ITEM_MAX) {
            if !out.contains(&item) && out.len() < LIST_MAX {
                out.push(item);
            }
        }
    }
    out
}

/// Holds the AI's summary to the cases it was given: an action that is the title of an existing
/// case (or of the subject) is dropped, a case number nobody was given is dropped, and every
/// text is short and free of references to the message.
fn check_summary(raw: RawSummary, reading: &Reading, subject: Option<&Case>) -> ReflectSummary {
    let known: HashSet<String> = reading
        .detailed
        .iter()
        .chain(&reading.overview)
        .chain(&reading.extra)
        .chain(subject)
        .map(|c| normalized(&c.title))
        .collect();
    let mut actions: Vec<SummaryAction> = Vec::new();
    for action in raw.actions {
        let Some(title) = clean(Some(&action.title), ITEM_MAX) else {
            continue;
        };
        if known.contains(&normalized(&title))
            || actions
                .iter()
                .any(|a| normalized(&a.title) == normalized(&title))
            || actions.len() >= ACTIONS_MAX
        {
            continue;
        }
        actions.push(SummaryAction {
            title,
            create: action.create,
        });
    }
    let mut waiting_on: Vec<WaitingOn> = Vec::new();
    for number in raw.waiting_on {
        let Some(case) = usize::try_from(number)
            .ok()
            .and_then(|n| reading.case_numbered(n))
        else {
            continue;
        };
        if waiting_on.iter().all(|w| w.note_id != case.id) && waiting_on.len() < WAITING_MAX {
            waiting_on.push(WaitingOn {
                note_id: case.id.clone(),
                title: case.title.clone(),
            });
        }
    }
    ReflectSummary {
        title: clean(Some(&raw.title), ITEM_MAX).unwrap_or_default(),
        direction: clean(Some(&raw.direction), SENTENCE_MAX * 2).unwrap_or_default(),
        kept: clean_list(raw.kept),
        rejected: clean_list(raw.rejected),
        evolution: clean_list(raw.evolution),
        questions: clean_list(raw.questions),
        actions,
        waiting_on,
        sent: None,
    }
}

// ---- What the AI reads ----

/// The conversation as the interface sent it, cleaned: the latest turns, short, none empty.
fn clean_turns(turns: Vec<ReflectPreviousTurn>) -> Vec<ReflectPreviousTurn> {
    let turns: Vec<ReflectPreviousTurn> = turns
        .into_iter()
        .filter_map(|turn| {
            let said = turn
                .said
                .map(|t| clip(&t, SAID_MAX))
                .filter(|t| !t.is_empty());
            let reply = turn
                .reply
                .map(|t| clip(&t, PREVIOUS_ANSWER_MAX))
                .filter(|t| !t.is_empty());
            (said.is_some() || reply.is_some()).then_some(ReflectPreviousTurn {
                said,
                reply,
                source_ids: turn.source_ids,
            })
        })
        .collect();
    let skip = turns.len().saturating_sub(TURNS_MAX);
    turns.into_iter().skip(skip).collect()
}

/// What the person said last.
fn latest_said(turns: &[ReflectPreviousTurn]) -> String {
    turns
        .iter()
        .rev()
        .find_map(|t| t.said.clone())
        .unwrap_or_default()
}

/// Words the cases are searched with: the subject's title and what the person said last (for a
/// summary, everything they said).
fn search_text(subject: Option<&Case>, turns: &[ReflectPreviousTurn], everything: bool) -> String {
    let mut text = subject.map(|c| c.title.clone()).unwrap_or_default();
    let said: Vec<&str> = turns.iter().filter_map(|t| t.said.as_deref()).collect();
    let said = if everything {
        said.join(" ")
    } else {
        latest_said(turns)
    };
    text.push(' ');
    text.push_str(&said);
    clip(&text, QUESTION_MAX)
}

/// What the AI reads for one turn: the subject, the cases of the space and the registry of the
/// excerpts it may cite.
struct Context {
    reading: Reading,
    subject: Option<Case>,
    query: String,
    latest: String,
}

/// The subject case in full (up to 40 sentences), which must be readable like any case a
/// question may cite: not a kept answer, not archived, not a sketch, not in another space.
fn load_subject(
    conn: &Connection,
    subject_id: Option<&str>,
    space: Option<&str>,
) -> Result<Option<Case>, String> {
    let Some(id) = subject_id else {
        return Ok(None);
    };
    let projects = project_names(conn)?;
    match load_case(
        conn,
        id,
        space,
        &projects,
        &HashSet::new(),
        AGENT_READ_SENTENCES,
    )? {
        Some(case) => Ok(Some(case)),
        None => Err(crate::tr!("reflect.subjectUnavailable")),
    }
}

/// The usual reading, searched with `query`, plus the cases linked in the earlier turns; without
/// the subject, which is sent apart and never cited.
fn prepare(
    conn: &Connection,
    subject: Option<Case>,
    turns: &[ReflectPreviousTurn],
    space: Option<&str>,
    meaning_ids: &[String],
    query: String,
) -> Result<Context, String> {
    let mut linked: Vec<String> = Vec::new();
    for id in turns.iter().rev().flat_map(|t| t.source_ids.iter()) {
        if !linked.contains(id) {
            linked.push(id.clone());
        }
    }
    let earlier = [PreviousTurn {
        question: String::new(),
        answer: String::new(),
        source_ids: linked,
    }];
    let mut reading = read(conn, &query, &earlier, space, meaning_ids)?;
    if let Some(subject) = &subject {
        reading.detailed.retain(|c| c.id != subject.id);
        reading.overview.retain(|c| c.id != subject.id);
    }
    Ok(Context {
        reading,
        subject,
        latest: latest_said(turns),
        query,
    })
}

fn conversation(turns: &[ReflectPreviousTurn]) -> Vec<Value> {
    turns
        .iter()
        .flat_map(|t| {
            t.said
                .iter()
                .map(|s| json!({ "person": s }))
                .chain(t.reply.iter().map(|r| json!({ "you": r })))
        })
        .collect()
}

impl Context {
    /// The message: numbers the excerpts of the cases, and sends the subject apart, without
    /// excerpt ids (nothing in it can be cited).
    fn message(&mut self, turns: &[ReflectPreviousTurn], summary: bool) -> Value {
        let mut message = self.reading.message("", &[]);
        let object = message.as_object_mut().expect("the message is an object");
        object.remove("question");
        let subject = self.subject.as_ref().map(|c| {
            json!({
                "title": c.title,
                "project": c.project,
                "kind": c.nature,
                "state": state_words(&c.status, &c.decision),
                "last_change": c.updated,
                "due": c.due,
                "text": c.sentences,
            })
        });
        object.insert("subject".into(), subject.unwrap_or(Value::Null));
        object.insert("conversation".into(), Value::Array(conversation(turns)));
        if !summary {
            object.insert("latest".into(), json!(self.latest));
        }
        message
    }
}

// ---- The conversation ----

/// The reply by an agent loop: the usual tools plus the final function `reply`; at most
/// [`REPLY_CALLS_MAX`] model calls.
async fn converse<F, Fut>(
    db: &std::sync::Mutex<Connection>,
    space: Option<&str>,
    context: &mut Context,
    message: Value,
    call_model: F,
) -> Result<crate::ask::AgentOutcome<RawReply>, AgentFailure>
where
    F: FnMut(Vec<Value>, Value) -> Fut,
    Fut: std::future::Future<Output = Result<Value, (String, Option<u16>)>>,
{
    let system = format!("{REPLY_PROMPT}{REPLY_TOOLS_ADDENDUM}");
    run_agent_with(
        db,
        space,
        &context.query,
        &mut context.reading,
        &system,
        message,
        "reply",
        REPLY_CALLS_MAX,
        parse_reply,
        call_model,
    )
    .await
}

fn reply_tools() -> Value {
    agent_tools_with(agent_tool(
        "reply",
        "Give your reply to the person.",
        reply_schema(),
    ))
}

/// The provider to ask, or the error to show when no AI is connected.
fn require_ai(
    providers: &[crate::providers::ProviderInfo],
    chosen: Option<&str>,
) -> Result<(String, String), String> {
    ready_provider(providers, chosen).ok_or_else(|| crate::tr!("reflect.noAi"))
}

/// Records what a reply or a summary was, numbers only: how many calls, links kept and dropped.
fn record(db_path: &Path, kind: &str, calls: usize, links: usize, dropped: usize, cases: usize) {
    let Some(folder) = db_path.parent() else {
        return;
    };
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let entry = json!({
        "at_unix_ms": millis,
        "prompt": PROMPT_VERSION,
        "kind": kind,
        "calls": calls,
        "links": links,
        "dropped": dropped,
        "cases_sent": cases,
    });
    if let Err(e) =
        crate::dictation_journal::append(&folder.join(JOURNAL_FILE), &entry, JOURNAL_MAX_SIZE)
    {
        eprintln!("reflect check not recorded ({e})");
    }
}

/// The next reply of the partner. The database lock is never held during a call to the AI.
/// `Err` carries the text to show: no AI connected, or a subject that cannot be read.
#[allow(clippy::too_many_arguments)]
pub async fn reply(
    db: &std::sync::Mutex<Connection>,
    db_path: &Path,
    data_dir: &Path,
    sense: Option<std::sync::Arc<crate::meaning::Sense>>,
    subject_id: Option<String>,
    turns: Vec<ReflectPreviousTurn>,
    space: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<ReflectReply, String> {
    let providers = crate::providers::providers_info().cleanup;
    let (provider_id, label) = require_ai(&providers, provider.as_deref())?;
    let turns = clean_turns(turns);
    let subject = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        load_subject(&conn, subject_id.as_deref(), space.as_deref())?
    };
    let query = search_text(subject.as_ref(), &turns, false);
    let meaning_ids = meaning_hits(sense, data_dir, query.clone()).await;
    let mut context = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        prepare(
            &conn,
            subject,
            &turns,
            space.as_deref(),
            &meaning_ids,
            query,
        )?
    };
    let message = context.message(&turns, false);
    // Tools only when the first reading is weak, as in Ask Humcase.
    let mut outcome = None;
    if !reading_is_strong(&context.reading, &context.query, &meaning_ids) {
        let tools = reply_tools();
        match converse(
            db,
            space.as_deref(),
            &mut context,
            message.clone(),
            |messages, choice| {
                chat_turn(
                    messages,
                    choice,
                    &tools,
                    provider_id.clone(),
                    model.clone(),
                    db_path,
                    crate::ai_usage::operation::REFLECT,
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
                REPLY_PROMPT,
                message,
                Some(provider_id),
                model,
                "reply",
                reply_schema(),
                db_path,
                crate::ai_usage::operation::REFLECT,
            )
            .await?;
            (parse_text(&text, parse_reply)?, 1)
        }
    };
    let subject_id = context.subject.as_ref().map(|c| c.id.as_str());
    let mut reply = check_reply(raw, &context.reading, subject_id);
    reply.sent = Some(Sent {
        service: label,
        excerpts: context.reading.case_count(),
    });
    record(
        db_path,
        "reply",
        calls,
        reply.links.len(),
        reply.dropped,
        context.reading.case_count(),
    );
    Ok(reply)
}

/// The summary of the conversation: one call, no tools.
#[allow(clippy::too_many_arguments)]
pub async fn summarize(
    db: &std::sync::Mutex<Connection>,
    db_path: &Path,
    data_dir: &Path,
    sense: Option<std::sync::Arc<crate::meaning::Sense>>,
    subject_id: Option<String>,
    turns: Vec<ReflectPreviousTurn>,
    space: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<ReflectSummary, String> {
    let providers = crate::providers::providers_info().cleanup;
    let (provider_id, label) = require_ai(&providers, provider.as_deref())?;
    let turns = clean_turns(turns);
    let subject = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        load_subject(&conn, subject_id.as_deref(), space.as_deref())?
    };
    let query = search_text(subject.as_ref(), &turns, true);
    let meaning_ids = meaning_hits(sense, data_dir, query.clone()).await;
    let mut context = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        prepare(
            &conn,
            subject,
            &turns,
            space.as_deref(),
            &meaning_ids,
            query,
        )?
    };
    let message = context.message(&turns, true);
    let text = crate::commands::ai::call_llm_json(
        SUMMARY_PROMPT,
        message,
        Some(provider_id),
        model,
        "summary",
        summary_schema(),
        db_path,
        crate::ai_usage::operation::REFLECT,
    )
    .await?;
    let raw: RawSummary = parse_text(&text, |v| {
        serde_json::from_value(v).map_err(|e| e.to_string())
    })?;
    let mut summary = check_summary(raw, &context.reading, context.subject.as_ref());
    summary.sent = Some(Sent {
        service: label,
        excerpts: context.reading.case_count(),
    });
    record(db_path, "summary", 1, 0, 0, context.reading.case_count());
    Ok(summary)
}

/// Keeps the summary the person corrected: a sub-case of the case thought about (the Inbox of
/// `space` without one), marked as written by Humcase so it is never read back as a source. The
/// ticked actions become task sub-cases of the case thought about (of the summary without one).
#[allow(clippy::too_many_arguments)]
pub fn keep(
    conn: &Connection,
    subject_id: Option<&str>,
    space: crate::models::project::ProjectSpace,
    title: &str,
    content: &str,
    service: Option<&str>,
    actions: &[String],
) -> Result<String, String> {
    let title = clip(title.trim(), KEPT_TITLE_MAX);
    if title.is_empty() {
        return Err("a summary needs a title".into());
    }
    let input =
        |title: String, content: Option<String>, note_type| crate::models::note::CreateNoteInput {
            project_id: String::new(),
            title,
            content,
            note_type,
            priority: None,
            git_branch: None,
            tags: None,
            due_at: None,
            raw_content: None,
        };
    let note = crate::models::note::NoteType::Note;
    let id = match subject_id {
        Some(subject) => crate::db::cases::create_sub_case(
            conn,
            subject,
            crate::models::note::ContentKind::Text,
            input(title, Some(content.to_string()), note),
        )?,
        None => {
            let mut new = input(title, Some(content.to_string()), note);
            new.project_id = crate::db::queries::get_or_create_inbox(conn, space)?;
            crate::db::queries::create_note(conn, new)?
        }
    };
    let label = match service {
        Some(service) => format!("Humcase · {}", clip(service, 100)),
        None => "Humcase".to_string(),
    };
    crate::db::cases::add_provenance(conn, &id, KEPT_KIND, Some(&label), None, None, None)?;
    let parent = subject_id.unwrap_or(&id);
    for action in actions
        .iter()
        .map(|a| a.trim())
        .filter(|a| !a.is_empty())
        .take(ACTIONS_MAX)
    {
        crate::db::cases::create_sub_case(
            conn,
            parent,
            crate::models::note::ContentKind::Text,
            input(
                clip(action, KEPT_TITLE_MAX),
                None,
                crate::models::note::NoteType::Todo,
            ),
        )?;
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ask::tests::{add, db, project, tool_call, tool_reply};
    use std::collections::HashMap;

    fn case(id: &str, title: &str, sentences: &[&str]) -> Case {
        crate::ask::tests::case(id, title, sentences)
    }

    /// A reading of three cases (a, b, c) whose excerpts are numbered 1 to 6: a's title and
    /// sentence, b's title and sentence, c's title and sentence.
    fn reading() -> Reading {
        let mut reading = Reading {
            today: "2026-10-08".into(),
            detailed: vec![
                case(
                    "a",
                    "Price of the class",
                    &["Price of the class", "45 euros from November."],
                ),
                case(
                    "b",
                    "Payment providers",
                    &["Payment providers", "Three were compared."],
                ),
                case(
                    "c",
                    "Opening hours",
                    &["Opening hours", "Closed on Mondays."],
                ),
            ],
            ..Reading::default()
        };
        reading.message("", &[]);
        reading
    }

    fn raw(json: Value) -> RawReply {
        parse_reply(json).unwrap()
    }

    // Removing the registry check shows the person a link no case supports.
    #[test]
    fn a_link_with_an_unsent_excerpt_id_is_dropped() {
        let reply = check_reply(
            raw(
                json!({ "restated": "You want to lower the price.", "links": [
                { "kind": "decided", "excerpt": 2 },
                { "kind": "supports", "excerpt": 99 },
                { "kind": "conflicts", "excerpt": -3 }
            ], "lead": null, "question": "Why?", "offer_summary": false }),
            ),
            &reading(),
            None,
        );
        assert_eq!(reply.links.len(), 1);
        assert_eq!(reply.links[0].kind, LinkKind::Decided);
        assert_eq!(reply.links[0].source.note_id, "a");
        assert_eq!(
            reply.links[0].source.passages,
            vec!["45 euros from November."]
        );
        assert_eq!(reply.dropped, 2);
    }

    // Removing the subject check cites back the case the person already sees.
    #[test]
    fn a_link_to_the_subject_case_is_dropped() {
        let reply = check_reply(
            raw(json!({ "restated": null, "links": [
                { "kind": "supports", "excerpt": 2 },
                { "kind": "supports", "excerpt": 4 }
            ], "lead": null, "question": "Why?", "offer_summary": false })),
            &reading(),
            Some("a"),
        );
        assert_eq!(reply.links.len(), 1);
        assert_eq!(reply.links[0].source.note_id, "b");
        assert_eq!(reply.dropped, 1);
    }

    #[test]
    fn a_link_beyond_the_third_is_dropped() {
        let mut reading = reading();
        reading
            .detailed
            .push(case("d", "Delivery", &["Delivery", "Within 48 hours."]));
        reading.message("", &[]);
        let reply = check_reply(
            raw(json!({ "restated": null, "links": [
                { "kind": "supports", "excerpt": 2 },
                { "kind": "supports", "excerpt": 4 },
                { "kind": "supports", "excerpt": 6 },
                { "kind": "supports", "excerpt": 8 }
            ], "lead": null, "question": "Why?", "offer_summary": false })),
            &reading,
            None,
        );
        assert_eq!(reply.links.len(), LINKS_MAX);
        assert_eq!(reply.dropped, 1);
        assert!(reply.links.iter().all(|l| l.source.note_id != "d"));
    }

    #[test]
    fn two_links_to_one_case_make_one_source_and_an_unknown_kind_is_dropped() {
        let reply = check_reply(
            raw(json!({ "restated": null, "links": [
                { "kind": "supports", "excerpt": 2 },
                { "kind": "supports", "excerpt": 1 },
                { "kind": "agrees", "excerpt": 4 }
            ], "lead": null, "question": null, "offer_summary": false })),
            &reading(),
            None,
        );
        assert_eq!(reply.links.len(), 1);
        assert_eq!(reply.dropped, 1);
    }

    // Removing the cut lets the partner ask a list of questions.
    #[test]
    fn two_questions_are_cut_to_one() {
        let reply = check_reply(
            raw(json!({ "restated": null, "links": [], "lead": null,
                "question": "What is the price today? And who decides it?",
                "offer_summary": false })),
            &reading(),
            None,
        );
        assert_eq!(reply.question.as_deref(), Some("What is the price today?"));
        let spanish = check_reply(
            raw(json!({ "restated": null, "links": [], "lead": null,
                "question": "¿Cuánto cuesta hoy? ¿Y quién decide?", "offer_summary": false })),
            &reading(),
            None,
        );
        assert_eq!(spanish.question.as_deref(), Some("¿Cuánto cuesta hoy?"));
        let with_reference = check_reply(
            raw(
                json!({ "restated": "You wait (excerpt 3).", "links": [], "lead": "  ",
                "question": "What next (id 4)?", "offer_summary": true }),
            ),
            &reading(),
            None,
        );
        assert_eq!(with_reference.restated.as_deref(), Some("You wait."));
        assert_eq!(with_reference.question.as_deref(), Some("What next?"));
        assert!(with_reference.lead.is_none() && with_reference.offer_summary);
    }

    fn summary(json: Value) -> RawSummary {
        serde_json::from_value(json).unwrap()
    }

    // Removing the number check would map a number nobody was given to some case.
    #[test]
    fn waiting_on_an_unknown_case_number_is_dropped_and_known_ones_map_to_ids() {
        let subject = case("s", "Open a second workshop", &["Open a second workshop"]);
        let summary = check_summary(
            summary(json!({ "title": "Reflection: second workshop",
                "direction": "Wait for the lease.", "kept": ["Lease first", "Lease first"],
                "rejected": [], "evolution": ["Before: now. After: next spring."],
                "questions": [], "actions": [
                    { "title": "Ask the landlord", "create": true },
                    { "title": "Payment providers", "create": true },
                    { "title": "open a SECOND workshop!", "create": false },
                    { "title": "Visit the premises", "create": false }
                ], "waiting_on": [2, 99, 0, -1, 2, 3] })),
            &reading(),
            Some(&subject),
        );
        let waiting: Vec<(&str, &str)> = summary
            .waiting_on
            .iter()
            .map(|w| (w.note_id.as_str(), w.title.as_str()))
            .collect();
        assert_eq!(
            waiting,
            vec![("b", "Payment providers"), ("c", "Opening hours")]
        );
        assert_eq!(summary.kept, vec!["Lease first"], "no item twice");
        let actions: Vec<(&str, bool)> = summary
            .actions
            .iter()
            .map(|a| (a.title.as_str(), a.create))
            .collect();
        assert_eq!(
            actions,
            vec![("Ask the landlord", true), ("Visit the premises", false)],
            "an action that already has a case, or is the subject, is dropped"
        );
    }

    #[test]
    fn the_no_ai_error_is_a_translated_text() {
        let error = require_ai(&[], None).unwrap_err();
        assert_ne!(error, "reflect.noAi", "the key exists");
        assert!(error.contains("Humcase"));
    }

    fn turn_said(text: &str) -> ReflectPreviousTurn {
        ReflectPreviousTurn {
            said: Some(text.into()),
            reply: None,
            source_ids: Vec::new(),
        }
    }

    // Removing the filters of `load_case` (shared with Ask) would let a kept answer or another
    // space into the reading, or let the subject be one.
    #[test]
    fn a_kept_summary_is_never_read_back_and_its_actions_go_to_the_subject() {
        let conn = db();
        let work = project(&conn, "Shop", crate::models::project::ProjectSpace::Work);
        let gift = add(
            &conn,
            &work,
            "Gift cards",
            "A card from 20 to 100, sent by email.",
        );
        let summary = keep(
            &conn,
            Some(&gift),
            crate::models::project::ProjectSpace::Work,
            "Reflection: gift cards",
            "### Direction
Three fixed amounts.

### Conversation
**You** : gift cards for the holidays",
            Some("Cerebras"),
            &["Create the three gift cards".into(), "  ".into()],
        )
        .unwrap();
        let children = crate::db::cases::list_sub_cases(&conn, &gift, Some(1)).unwrap();
        let titles: Vec<&str> = children.iter().map(|c| c.note.title.as_str()).collect();
        assert!(titles.contains(&"Reflection: gift cards"));
        assert!(titles.contains(&"Create the three gift cards"));
        assert_eq!(children.len(), 2, "an empty action is not created");
        // Read back, the summary would quote the old conversation as if it were a case.
        let turns = vec![turn_said("gift cards for the holidays")];
        let query = search_text(None, &turns, false);
        let context = prepare(&conn, None, &turns, Some("work"), &[], query).unwrap();
        let ids: Vec<&str> = context
            .reading
            .detailed
            .iter()
            .chain(&context.reading.overview)
            .map(|c| c.id.as_str())
            .collect();
        assert!(ids.contains(&gift.as_str()));
        assert!(!ids.contains(&summary.as_str()), "a kept summary");
        let asked = crate::ask::read(
            &conn,
            "gift cards holidays",
            &[],
            Some("work"),
            std::slice::from_ref(&summary),
        )
        .unwrap();
        assert!(
            asked
                .detailed
                .iter()
                .chain(&asked.overview)
                .all(|c| c.id != summary),
            "Ask does not read it either"
        );
    }

    #[test]
    fn another_space_and_a_kept_answer_are_never_read() {
        let conn = db();
        let work = project(&conn, "Shop", crate::models::project::ProjectSpace::Work);
        let home = project(&conn, "Home", crate::models::project::ProjectSpace::Private);
        let visible = add(
            &conn,
            &work,
            "Plumber contract",
            "The plumber contract is signed.",
        );
        let private = add(&conn, &home, "Plumber at home", "Plumber contract at home.");
        let kept = crate::ask::keep(
            &conn,
            crate::models::project::ProjectSpace::Work,
            "What about the plumber contract?",
            "The plumber contract is signed. [1]",
            None,
            &[],
        )
        .unwrap();
        let turns = vec![turn_said("The plumber contract")];
        let query = search_text(None, &turns, false);
        let context = prepare(&conn, None, &turns, Some("work"), &[], query).unwrap();
        let ids: Vec<&str> = context
            .reading
            .detailed
            .iter()
            .chain(&context.reading.overview)
            .map(|c| c.id.as_str())
            .collect();
        assert!(ids.contains(&visible.as_str()));
        assert!(!ids.contains(&private.as_str()), "another space");
        assert!(!ids.contains(&kept.as_str()), "a kept answer");
        // The subject itself must be readable.
        assert!(load_subject(&conn, Some(&kept), Some("work")).is_err());
        assert!(load_subject(&conn, Some(&private), Some("work")).is_err());
        assert!(load_subject(&conn, Some(&visible), Some("work"))
            .unwrap()
            .is_some());
        // The cases linked in earlier turns come back, the subject never twice.
        let earlier = vec![ReflectPreviousTurn {
            said: None,
            reply: Some("Think about it.".into()),
            source_ids: vec![visible.clone()],
        }];
        let subject = load_subject(&conn, Some(&visible), Some("work")).unwrap();
        let context = prepare(
            &conn,
            subject,
            &earlier,
            Some("work"),
            &[],
            "contract".into(),
        )
        .unwrap();
        assert!(context.reading.detailed.iter().all(|c| c.id != visible));
        assert!(context.reading.overview.iter().all(|c| c.id != visible));
    }

    // The opening turn has a subject and no turns: the AI must be sent the subject's text.
    #[test]
    fn the_opening_turn_sends_the_subject_text() {
        let conn = db();
        let work = project(&conn, "Shop", crate::models::project::ProjectSpace::Work);
        let subject_id = add(
            &conn,
            &work,
            "Summer camp for children",
            "A five-day camp in July for ages 8 to 12. We would need a second instructor.",
        );
        let other = add(
            &conn,
            &work,
            "Camp insurance",
            "Does the insurance cover the camp?",
        );
        let subject = load_subject(&conn, Some(&subject_id), Some("work")).unwrap();
        let query = search_text(subject.as_ref(), &[], false);
        let mut context = prepare(&conn, subject, &[], Some("work"), &[], query).unwrap();
        let message = context.message(&[], false);
        assert_eq!(message["conversation"], json!([]));
        let text = message["subject"]["text"].to_string();
        assert!(text.contains("A five-day camp in July for ages 8 to 12."));
        assert!(text.contains("We would need a second instructor."));
        assert_eq!(message["subject"]["title"], "Summer camp for children");
        // The subject is in no excerpt, so nothing in it can be cited.
        assert!(context
            .reading
            .excerpts
            .values()
            .all(|(id, _, _)| *id != subject_id));
        assert!(context
            .reading
            .excerpts
            .values()
            .any(|(id, _, _)| *id == other));

        // The same message goes to the model on its first call.
        let db = std::sync::Mutex::new(conn);
        let seen: std::sync::Arc<std::sync::Mutex<Vec<Value>>> = Default::default();
        let log = seen.clone();
        let outcome = tauri::async_runtime::block_on(converse(
            &db,
            Some("work"),
            &mut context,
            message,
            |messages, _| {
                log.lock().unwrap().push(messages[1].clone());
                let reply = tool_reply(vec![tool_call(
                    "r",
                    "reply",
                    json!({ "restated": "A camp in July.", "links": [], "lead": null,
                        "question": "Who would run it?", "offer_summary": false }),
                )]);
                async move { Ok(reply) }
            },
        ))
        .unwrap();
        assert_eq!(outcome.calls, 1);
        let first = seen.lock().unwrap()[0]["content"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(first.contains("A five-day camp in July for ages 8 to 12."));
    }

    // A tool may bring the subject back; the check still drops a link to it.
    #[test]
    fn the_loop_registers_what_a_tool_returned_and_the_check_drops_the_subject() {
        let conn = db();
        let work = project(&conn, "Shop", crate::models::project::ProjectSpace::Work);
        let subject_id = add(&conn, &work, "Class price", "The class costs 45 euros.");
        let decision = add(
            &conn,
            &work,
            "Subscribers keep 40",
            "Subscribers keep 40 euros until January.",
        );
        let subject = load_subject(&conn, Some(&subject_id), Some("work")).unwrap();
        let turns = vec![turn_said("I would lower it to 35.")];
        let query = search_text(subject.as_ref(), &turns, false);
        let mut context = prepare(&conn, subject, &turns, Some("work"), &[], query).unwrap();
        let message = context.message(&turns, false);
        let db = std::sync::Mutex::new(conn);
        let mut turn = 0;
        let outcome = tauri::async_runtime::block_on(converse(
            &db,
            Some("work"),
            &mut context,
            message,
            |messages, _| {
                turn += 1;
                let reply = if turn == 1 {
                    tool_reply(vec![tool_call(
                        "s",
                        "search_cases",
                        json!({ "query": "euros" }),
                    )])
                } else {
                    let found: Value =
                        serde_json::from_str(messages.last().unwrap()["content"].as_str().unwrap())
                            .unwrap();
                    let ids: Vec<(String, i64)> = found["cases"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| {
                            (
                                c["excerpts"][0]["text"].as_str().unwrap().to_string(),
                                c["excerpts"][1]["id"].as_i64().unwrap(),
                            )
                        })
                        .collect();
                    let id_of = |title: &str| ids.iter().find(|(t, _)| t == title).unwrap().1;
                    tool_reply(vec![tool_call(
                        "r",
                        "reply",
                        json!({ "restated": "Lower to 35.", "links": [
                            { "kind": "conflicts", "excerpt": id_of("Subscribers keep 40") },
                            { "kind": "supports", "excerpt": id_of("Class price") }
                        ], "lead": null, "question": "Why 35?", "offer_summary": false }),
                    )])
                };
                async move { Ok(reply) }
            },
        ))
        .unwrap();
        assert_eq!(outcome.calls, 2);
        let reply = check_reply(outcome.raw, &context.reading, Some(&subject_id));
        assert_eq!(reply.links.len(), 1);
        assert_eq!(reply.links[0].kind, LinkKind::Conflicts);
        assert_eq!(reply.links[0].source.note_id, decision);
        assert_eq!(reply.dropped, 1, "the link to the subject");
    }

    #[test]
    fn at_most_four_calls_and_the_last_one_must_reply() {
        let conn = db();
        let db = std::sync::Mutex::new(conn);
        let mut context = prepare(&db.lock().unwrap(), None, &[], None, &[], "x".into()).unwrap();
        let message = context.message(&[], false);
        let choices: std::sync::Arc<std::sync::Mutex<Vec<Value>>> = Default::default();
        let log = choices.clone();
        let outcome = tauri::async_runtime::block_on(converse(
            &db,
            None,
            &mut context,
            message,
            |_, choice| {
                log.lock().unwrap().push(choice);
                let reply = tool_reply(vec![tool_call(
                    "s",
                    "list_cases",
                    json!({ "state": "open" }),
                )]);
                async move { Ok(reply) }
            },
        ));
        assert!(matches!(outcome, Err(AgentFailure::Failed(_))));
        let seen = choices.lock().unwrap().clone();
        assert_eq!(seen.len(), REPLY_CALLS_MAX);
        assert_eq!(seen[REPLY_CALLS_MAX - 1]["function"]["name"], "reply");
    }

    #[test]
    fn the_journal_holds_numbers_only() {
        let folder = tempfile::tempdir().unwrap();
        let db_path = folder.path().join("humcase.db");
        record(&db_path, "reply", 2, 1, 1, 9);
        let line = std::fs::read_to_string(folder.path().join(JOURNAL_FILE)).unwrap();
        let entry: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(entry["calls"], 2);
        assert_eq!(entry["links"], 1);
        assert_eq!(entry["dropped"], 1);
        assert_eq!(entry["kind"], "reply");
        assert_eq!(line.lines().count(), 1);
    }

    #[test]
    fn the_conversation_keeps_the_latest_turns_and_their_order() {
        let mut turns: Vec<ReflectPreviousTurn> =
            (0..20).map(|i| turn_said(&format!("said {i}"))).collect();
        turns.push(ReflectPreviousTurn {
            said: None,
            reply: Some("A reply.".into()),
            source_ids: Vec::new(),
        });
        turns.push(turn_said("   "));
        let turns = clean_turns(turns);
        assert_eq!(turns.len(), TURNS_MAX);
        let talk = conversation(&turns);
        assert_eq!(talk.last().unwrap()["you"], "A reply.");
        assert_eq!(talk[0]["person"], "said 9");
        assert_eq!(latest_said(&turns), "said 19");
    }

    // ---- Real check (Cerebras) ----

    fn passage_is_in(passage: &str, title: &str, content: &str) -> bool {
        let flat = |s: &str| {
            s.replace("**", "")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        let (passage, text) = (flat(passage), flat(&format!("{title}\n{content}")));
        text.contains(&passage)
    }

    /// Plays four scripted conversations on the French base of the bench with real calls: an
    /// opening on a subject, a statement that contradicts a decision of the base, a hesitation
    /// (a lead is expected), and a conversation ending with a summary. Prints each reply in full
    /// and measures: one question per reply, links word for word, a `conflicts` link on the
    /// contradiction, a lead on the hesitation, and the summary's items for a human to judge.
    /// Provider from `ASK_EVAL_PROVIDER` (Cerebras), model from `ASK_EVAL_MODEL`, results to
    /// `REFLECT_EVAL_OUT`. `cargo test reflect_eval -- --ignored --nocapture`
    #[test]
    #[ignore = "real AI calls"]
    fn reflect_eval() {
        let bench: Value = serde_json::from_str(include_str!("ask_eval_cases.json")).unwrap();
        let base = &bench["bases"][0];
        assert_eq!(base["lang"], "fr");
        let conn = db();
        let ids = crate::ask::tests::build_base(&conn, base);
        let contents: HashMap<String, (String, String)> = base["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                (
                    ids[c["key"].as_str().unwrap()].clone(),
                    (
                        c["title"].as_str().unwrap().to_string(),
                        c["content"].as_str().unwrap().to_string(),
                    ),
                )
            })
            .collect();
        let key_of = |id: &str| {
            ids.iter()
                .find(|(_, v)| v.as_str() == id)
                .map(|(k, _)| k.clone())
                .unwrap_or_else(|| id.to_string())
        };
        let folder = tempfile::tempdir().unwrap();
        let db_path = folder.path().join("eval.db");
        let data_dir = folder.path().to_path_buf();
        let db = std::sync::Mutex::new(conn);
        let provider = std::env::var("ASK_EVAL_PROVIDER")
            .ok()
            .or(Some("cerebras".into()));
        let model = std::env::var("ASK_EVAL_MODEL").ok();

        let said = |text: &str| turn_said(text);
        let scripted: Vec<(&str, Option<&str>, Vec<ReflectPreviousTurn>)> = vec![
            ("opening on a subject", Some("camp"), vec![]),
            (
                "contradiction of a decision",
                None,
                vec![said(
                    "Je pense baisser le cours du samedi à 35 € dès octobre pour attirer plus de monde.",
                )],
            ),
            (
                "hesitation",
                Some("clay"),
                vec![said(
                    "Je n'arrive pas à me décider pour l'argile. Solargil est moins cher mais je ne sais pas si je peux leur faire confiance, et Ceradel je connais déjà. Je tourne en rond.",
                )],
            ),
        ];
        let mut results = Vec::new();
        let (mut one_question, mut replies, mut links_total, mut links_valid) = (0, 0, 0, 0);
        let (mut conflicts_found, mut lead_found, mut dropped) = (false, false, 0);
        for (name, subject, turns) in &scripted {
            let subject_id = subject.map(|k| ids[k].clone());
            let reply = tauri::async_runtime::block_on(super::reply(
                &db,
                &db_path,
                &data_dir,
                None,
                subject_id,
                turns.clone(),
                Some("work".into()),
                provider.clone(),
                model.clone(),
            ))
            .unwrap();
            println!("\n=== {name}");
            print_reply(&reply, &key_of);
            replies += 1;
            one_question += usize::from(
                reply
                    .question
                    .as_deref()
                    .is_some_and(|q| q.matches('?').count() == 1),
            );
            for link in &reply.links {
                links_total += 1;
                let (title, content) = &contents[&link.source.note_id];
                links_valid += usize::from(
                    link.source
                        .passages
                        .iter()
                        .all(|p| passage_is_in(p, title, content)),
                );
            }
            dropped += reply.dropped;
            if *name == "contradiction of a decision" {
                conflicts_found = reply
                    .links
                    .iter()
                    .any(|l| l.kind == LinkKind::Conflicts && key_of(&l.source.note_id) == "price");
            }
            if *name == "hesitation" {
                lead_found = reply.lead.is_some();
            }
            results.push(json!({ "scenario": name, "reply": reply }));
        }

        // Fourth conversation: three turns, then a summary.
        let kiln = ids["kiln"].clone();
        let turns = vec![
            said("Je veux remplacer le four, l'actuel chauffe mal."),
            ReflectPreviousTurn {
                said: None,
                reply: Some("Le four actuel chauffe mal au-dessus de 1 200 °C. Qu'est-ce qui compte le plus pour toi dans le nouveau four ?".into()),
                source_ids: vec![],
            },
            said("La puissance et le prix. Je prends le devis le moins cher sous les 4 000 €, et j'appelle le vendeur cette semaine."),
            ReflectPreviousTurn {
                said: None,
                reply: Some("Tu retiens le devis le moins cher sous 4 000 € et tu appelles le vendeur cette semaine. Veux-tu que je résume ?".into()),
                source_ids: vec![],
            },
            said("Oui. Plus tard, peut-être un deuxième four pour les gros cours, mais pas maintenant."),
        ];
        let last = super::reply(
            &db,
            &db_path,
            &data_dir,
            None,
            Some(kiln.clone()),
            turns.clone(),
            Some("work".into()),
            provider.clone(),
            model.clone(),
        );
        let last = tauri::async_runtime::block_on(last).unwrap();
        println!("\n=== ending with a summary (last reply)");
        print_reply(&last, &key_of);
        replies += 1;
        one_question += usize::from(
            last.question
                .as_deref()
                .is_some_and(|q| q.matches('?').count() == 1),
        );
        let summary = tauri::async_runtime::block_on(summarize(
            &db,
            &db_path,
            &data_dir,
            None,
            Some(kiln),
            turns,
            Some("work".into()),
            provider,
            model,
        ))
        .unwrap();
        println!("\n=== summary");
        println!("title: {}\ndirection: {}", summary.title, summary.direction);
        for (label, list) in [
            ("kept", &summary.kept),
            ("rejected", &summary.rejected),
            ("evolution", &summary.evolution),
            ("questions", &summary.questions),
        ] {
            println!("{label}: {list:?}");
        }
        println!("actions: {:?}", summary.actions);
        println!(
            "waiting_on: {:?}",
            summary
                .waiting_on
                .iter()
                .map(|w| key_of(&w.note_id))
                .collect::<Vec<_>>()
        );
        results.push(json!({ "scenario": "summary", "summary": summary }));

        let usage: Vec<_> = crate::ai_usage::read(&db_path, 0)
            .into_iter()
            .filter(|u| u.ok)
            .collect();
        let cost: f64 = usage
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
        let report = json!({
            "replies": replies,
            "one_question_per_reply": format!("{one_question}/{replies}"),
            "links_valid": format!("{links_valid}/{links_total}"),
            "links_dropped_by_the_check": dropped,
            "conflicts_link_on_the_contradiction": conflicts_found,
            "lead_on_the_hesitation": lead_found,
            "model_calls": usage.len(),
            "cost_usd": cost,
        });
        println!("\n{}", serde_json::to_string_pretty(&report).unwrap());
        if let Ok(out) = std::env::var("REFLECT_EVAL_OUT") {
            std::fs::write(
                out,
                serde_json::to_string_pretty(&json!({ "report": report, "results": results }))
                    .unwrap(),
            )
            .unwrap();
        }
    }

    fn print_reply(reply: &ReflectReply, key_of: &dyn Fn(&str) -> String) {
        println!("restated: {:?}", reply.restated);
        for link in &reply.links {
            println!(
                "link {:?} -> {} «{}» {:?}",
                link.kind,
                key_of(&link.source.note_id),
                link.source.title,
                link.source.passages
            );
        }
        println!("lead: {:?}", reply.lead);
        println!("question: {:?}", reply.question);
        println!(
            "offer_summary: {}, dropped: {}",
            reply.offer_summary, reply.dropped
        );
    }
}
