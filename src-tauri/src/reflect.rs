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
    project_names, read, reading_is_strong, ready_provider, run_agent_with, without_references,
    words_of, AgentFailure, Case, PreviousTurn, Reading, Sent, Source, AGENT_READ_SENTENCES,
    JOURNAL_MAX_SIZE, PASSAGES_MAX, PREVIOUS_ANSWER_MAX, QUESTION_MAX, SENTENCES_PER_CASE,
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
const PROMPT_VERSION: &str = "reflect.v14";

/// Model calls one reply may take, the last one forced to `reply`.
const REPLY_CALLS_MAX: usize = 4;
/// An answer of up to this many significant words can be told from a question that repeats it.
const SETTLED_ANSWER_WORDS: usize = 6;
/// Links one reply carries.
const LINKS_MAX: usize = 3;
/// Turns of the conversation the AI reads (the latest ones).
const TURNS_MAX: usize = 12;
/// Extra attempts when the question digs into a point the person settled (each is one more call).
const REPEAT_RETRIES: usize = 2;
/// Cases sent with their sentences at least: when the words and the meaning of what was said
/// find fewer, the latest cases of the space fill up (a partner that only sees titles cannot
/// link, or notice a tension with, a case it has not read).
const DETAILED_MIN: usize = 6;
/// Length of a text said by the person, and of a text written by the partner.
const SAID_MAX: usize = QUESTION_MAX + 500;
const SENTENCE_MAX: usize = 400;
const ITEM_MAX: usize = 240;
/// Items in each list of the summary, actions, and cases waited on.
const LIST_MAX: usize = 6;
const ACTIONS_MAX: usize = 8;
const WAITING_MAX: usize = 5;

const REPLY_PROMPT: &str = "You are the thinking partner of a person who is working an idea, a problem or a choice through, using only what they say and the cases (their notes, tasks, decisions) given in the user message as JSON. `subject` is the case they start from when there is one: they already see it, so never cite it. Each case in `cases` has numbered excerpts, the first one being its title; `overview` lists the latest cases of the space in one line each. `conversation` holds the exchange so far, oldest first: `person` is what they said, `you` is what you replied before. When `conversation` is empty, the person has just opened the subject: restate it from its own text and ask a first question about the subject itself, not about another case: what in it is still open or uncertain for the person (the option it holds, whether it suits them, what makes them hesitate), naming its own element (\"the oak for the table\"), since the person has not yet said what they hesitate on; other cases come into your questions only once the person's words bear on them. `latest` is the last thing the person said.
Each turn you first fill `language`: the language of the person's latest message (of the subject when they have not spoken yet), by its English name (\"French\", \"English\"); every text you write after it (the restatement, the lead and the question included, with no word of another language) is in that language, even if these instructions or the cases are in another one, and you address the person as the interface does (French: \"vous\", never \"tu\", even if the person writes \"je\"). Then you do four things:
1. `restated`: one or two short sentences restating what the person just said, in their own terms. Add nothing of yours.
2. `links`: at most 3 links to what the cases already say that bears on what the person said, only when the excerpt really says it. `kind`: `supports` (the case goes the same way), `decided` (a decision already taken that bears on it), `conflicts` (it may contradict what the person said or wants). `excerpt` is the id of the excerpt that states it. Never link the subject. No link is better than a forced one: an empty list is fine.
3. `lead`: null in most turns. Only when the person is stuck or hesitates between options (not when they simply state a plan), one short lead that rests on one sentence of a case or on one precise fact the person gave, names it, and adds what the person did not say: a tension between two things they said or between what they said and a case, a constraint or an option that a case already holds and they did not mention. Never general advice (compare, list your criteria, ask for quotes, estimate, build a table, check the documents), never an option, figure, percentage, currency or fact that is in neither the cases nor the conversation, never a case applied to a situation it does not concern (a rule for new clients to an old client). No lead is far better than a generic one.
4. `question`: exactly ONE question, the one that moves the thinking forward the most, built on what the person just said. Never two questions, never a list of questions, and never a question already asked in `conversation` (even in other words) nor one the person already answered: follow where the person goes, do not bring them back to your own idea. Make it specific: it names the element it builds on (a figure, a name, an option or a date from the person's last message or from a case), so that it could not be asked unchanged about another situation (but never by naming again what the person has just settled to ask about it: not when, how or how much of what they have just decided or answered; ask about what their answer leaves open elsewhere). Forbidden, unless the conversation has led exactly there: the generic questions of any checklist (what budget, what deadline, what criteria or weights, which method, which tool, what plan, what is the next step, what price, who and how many). Choose, in this order: (a) a tension between what the person just said and a case (a figure, a date, a decision): name both; (b) the point they hesitate on or that blocks their choice; (c) an element of a case that really bears on their choice and that they did not mention (never a case that only shares a word with it); (d) what their last message leaves open; when it leaves nothing open (they decided, postponed or closed the point), go to another open element of the conversation or of the cases (a choice still waiting, a figure not yet given), never deeper into the closed one. Do not bring up again a tension or a case already pointed out in `conversation`, unless the person's last message changes it. Never ask the person to compute, estimate, subtract, rank, work out how many or how much, or choose a method (you may point out yourself a plain comparison of two figures written in the cases or said by the person, a sum, a difference or an order of dates, showing both figures). Never ask for something the person has just said they do not know or do not have. Never offer help (\"do you want me to...\") and never ask permission (the only offer you may make is the summary below, and then `question` is never null).
When the person has made a choice and named what they will do, and no hesitation they voiced is still open, set `offer_summary` to true and make your question ask whether the person wants a summary of it, instead of opening a new subject; otherwise false, and once you have asked about a summary, never ask about it again, whatever the answer.
Rules:
- Never mix languages, and never write a currency symbol or a unit the person and the cases do not use.
- Address the person directly as the interface does: \"vous\" in French, \"tú\" in Spanish, \"du\" in German, \"você\" in Portuguese, \"tu\" in Italian; keep it for the whole conversation, restatement of the subject included.
- Say that something was chosen, retained or decided only when an excerpt or the person says so; a proposal or a suggestion written in a case is not a choice (\"Stripe is proposed\", never \"Stripe, already chosen\"). A case whose state says it is a proposal not chosen has nothing chosen in it yet: never call its option chosen, retained, decided or planned (not \"the payment solution chosen\", not \"confirm the choice of X\"); say it is proposed and still to decide, or say nothing of it.
- `settled` lists the questions you asked that the person has already answered, with their answer. Those points are closed, and so is anything the person postponed (\"later\", \"not now\"): never ask about them again, nor about their details, not in other words and not by laying out their options again (now or later, with or without), and do not ask what to do about the answer itself (when, how) unless it leaves something open. Take the answer as given and ask about what is still open. If `not_this_question` is present, your previous attempt asked it and it digs into a closed point: ask a question that does not name what that point is about, in any wording (another element of the conversation or of the cases). If `do_not_use_these_words` is present, none of those words appears in your question.
- You never decide for the person. A proposal stays a proposal, what is still to decide stays open, a question stays a question. Never say who wrote, proposed or decided something (\"you proposed\") unless the excerpt names them.
- Speak as the person would: name a case only by its subject (\"the choice of the supplier\"), never as \"the case\", and say its state in plain words (\"waits for your decision\", \"in progress\"); never write the words of the data (kind, state, excerpt, case number, supports, decided, conflicts).
- Never state anything the excerpts and the conversation do not say: no outside knowledge, no guess, no advice of your own beyond the lead, no reason, cause or judgment that is not written (except a plain comparison of two figures or dates that are written), and no link between two cases that their text does not make.";

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
- `kept`: what the person kept (ideas, options, decisions): only what they chose, retained or agreed (\"I could\", \"maybe\", \"probably\" and \"I would like to\" are not enough); an option they still hesitate on, consider or may do goes nowhere in `kept` (say it is undecided in `direction` or put it in `questions`). `rejected`: what they set aside. `evolution`: how the idea changed during the conversation, each as what it was and what it became, written in the language of the conversation, not a list of the steps taken; an empty list when it did not change. `questions`: only questions raised in the conversation (by the person or by you) that nobody answered later in the conversation, rewritten as a plain question without addressing the person; never a new one of yours, never one the person answered. Short strings; an empty list when there is nothing.
- `actions`: next steps the person named, each a short task title. `create` is true only when the person said they will do it (not \"probably\", \"could\" or \"maybe\"); false for an idea for later. Never an action that only follows from a question, a doubt or an option (\"determine the quantities\" because a question was left open): only what the person said they will do. Never an action that already has a case in `cases`, `overview` or `subject`.
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

impl ReflectReply {
    /// Drops from the three texts the currency symbols nobody used (see [`without_unseen_currency`]).
    fn without_unseen_currency(&mut self, corpus: &str) {
        for text in [&mut self.restated, &mut self.lead, &mut self.question]
            .into_iter()
            .flatten()
        {
            *text = without_unseen_currency(text, corpus);
        }
    }
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
            "language": { "type": "string" },
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
        "required": ["language", "restated", "links", "lead", "question", "offer_summary"],
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

/// The text without the currency symbols (`€`, `$`, `£`) that appear nowhere in `corpus`, what
/// the person and the cases said: a partner that writes "430 $" under figures nobody gave a
/// currency to states a unit nobody said.
fn without_unseen_currency(text: &str, corpus: &str) -> String {
    let mut text = text.to_string();
    for symbol in ['€', '$', '£'] {
        if !corpus.contains(symbol) && text.contains(symbol) {
            let symbol = regex::escape(&symbol.to_string());
            // "430 $" keeps the figure; "$430" keeps the figure too.
            for (pattern, with) in [
                (format!(r"(\d)\s?{symbol}"), "$1"),
                (format!(r"{symbol}\s?"), ""),
            ] {
                text = regex::Regex::new(&pattern)
                    .expect("valid pattern")
                    .replace_all(&text, with)
                    .into_owned();
            }
        }
    }
    text
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
/// link beyond the third or naming only a case's title is dropped; every drop is counted. Two links to one case become one
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
        // A case's title alone shows nothing of what it says: a link needs one of its sentences.
        if passage == title || links.len() >= LINKS_MAX {
            dropped += 1;
            continue;
        }
        links.push(ReflectLink {
            kind,
            source: Source {
                note_id: note_id.clone(),
                title: title.clone(),
                passages: vec![passage.clone()],
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

/// Whether a question asks again about a point the person just settled: it holds most of the
/// words of a short answer (two at least). A question about what the answer leaves open holds
/// few of them; a language-free check on the words, not on a list of phrases.
fn repeats_settled(question: &str, settled: &[(String, String)]) -> bool {
    !settled_words_in(question, settled).is_empty()
}

/// The words of a settled short answer that the question holds, when it repeats that answer
/// (see [`repeats_settled`]); empty when it does not.
fn settled_words_in(question: &str, settled: &[(String, String)]) -> Vec<String> {
    let words = words_of(question);
    let mut found: Vec<String> = Vec::new();
    for (_, answer) in settled {
        let answer = words_of(answer);
        let shared: Vec<String> = words.intersection(&answer).cloned().collect();
        if !answer.is_empty()
            && answer.len() <= SETTLED_ANSWER_WORDS
            && shared.len() >= 2
            && shared.len() * 2 >= answer.len()
        {
            found.extend(shared);
        }
    }
    found.sort();
    found.dedup();
    found
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
    reading.proposals_not_chosen = true;
    if let Some(subject) = &subject {
        reading.detailed.retain(|c| c.id != subject.id);
        reading.overview.retain(|c| c.id != subject.id);
    }
    if reading.detailed.len() < DETAILED_MIN {
        let projects = project_names(conn)?;
        let words = words_of(&query);
        while reading.detailed.len() < DETAILED_MIN && !reading.overview.is_empty() {
            let next = reading.overview.remove(0);
            if let Some(full) =
                load_case(conn, &next.id, space, &projects, &words, SENTENCES_PER_CASE)?
            {
                reading.detailed.push(full);
            }
        }
    }
    Ok(Context {
        reading,
        subject,
        latest: latest_said(turns),
        query,
    })
}

/// The last question a text asks: from the start of its sentence to its `?`.
fn last_question(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let end = chars.iter().rposition(|c| *c == '?')?;
    let start = (0..end)
        .rev()
        .find(|&i| match chars[i] {
            '!' | '?' | '\n' | '¿' => true,
            '.' => chars.get(i + 1).is_none_or(|next| next.is_whitespace()),
            _ => false,
        })
        .map_or(0, |i| if chars[i] == '¿' { i } else { i + 1 });
    let question: String = chars[start..=end].iter().collect();
    let question = question.trim().to_string();
    (!question.is_empty()).then_some(question)
}

/// The questions the partner asked that the person answered next, with their answer: what is
/// settled in the conversation.
fn settled_pairs(turns: &[ReflectPreviousTurn]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut asked: Option<String> = None;
    for turn in turns {
        if let (Some(said), Some(question)) = (&turn.said, asked.take()) {
            pairs.push((question, said.clone()));
        }
        if let Some(reply) = &turn.reply {
            asked = last_question(reply);
        }
    }
    pairs
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
                "state": self.reading.state_of(c),
                "last_change": c.updated,
                "due": c.due,
                "text": c.sentences,
            })
        });
        object.insert("subject".into(), subject.unwrap_or(Value::Null));
        object.insert("conversation".into(), Value::Array(conversation(turns)));
        let settled: Vec<Value> = settled_pairs(turns)
            .into_iter()
            .map(|(asked, answer)| json!({ "you_asked": asked, "person_answered": answer }))
            .collect();
        object.insert("settled".into(), Value::Array(settled));
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

/// `reasoning_effort` sent with the reply calls: nothing (the default of the model).
const REPLY_EFFORT: Option<&str> = None;

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
    reply_with_effort(
        db,
        db_path,
        data_dir,
        sense,
        subject_id,
        turns,
        space,
        provider,
        model,
        REPLY_EFFORT,
    )
    .await
}

/// [`reply`] sending `reasoning_effort` to the model when given (the bench compares settings).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn reply_with_effort(
    db: &std::sync::Mutex<Connection>,
    db_path: &Path,
    data_dir: &Path,
    sense: Option<std::sync::Arc<crate::meaning::Sense>>,
    subject_id: Option<String>,
    turns: Vec<ReflectPreviousTurn>,
    space: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    effort: Option<&str>,
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
                    effort,
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
    let single = |message: Value| {
        crate::commands::ai::call_llm_json_effort(
            REPLY_PROMPT,
            message,
            Some(provider_id.clone()),
            model.clone(),
            "reply",
            reply_schema(),
            db_path,
            crate::ai_usage::operation::REFLECT,
            effort,
        )
    };
    let (raw, mut calls) = match outcome {
        Some(done) => (done.raw, done.calls),
        None => (parse_text(&single(message.clone()).await?, parse_reply)?, 1),
    };
    let subject_id = context.subject.as_ref().map(|c| c.id.as_str());
    let corpus = message.to_string();
    let mut reply = check_reply(raw, &context.reading, subject_id);
    reply.without_unseen_currency(&corpus);
    // A question that asks again about a point the person just settled is asked once more,
    // told not to: one extra call, only then.
    let settled = settled_pairs(&turns);
    #[cfg(test)]
    let settled = if tests::NO_RETRY.load(std::sync::atomic::Ordering::Relaxed) {
        Vec::new()
    } else {
        settled
    };
    let mut refused: Vec<String> = Vec::new();
    let mut forbidden: Vec<String> = Vec::new();
    for _ in 0..REPEAT_RETRIES {
        let Some(repeated) = reply
            .question
            .clone()
            .filter(|q| repeats_settled(q, &settled))
        else {
            break;
        };
        for word in settled_words_in(&repeated, &settled) {
            if !forbidden.contains(&word) {
                forbidden.push(word);
            }
        }
        refused.push(repeated);
        let mut again = message.clone();
        again["not_this_question"] = json!(refused.join(" | "));
        again["do_not_use_these_words"] = json!(forbidden);
        let Ok(text) = single(again).await else {
            break;
        };
        let Ok(raw) = parse_text(&text, parse_reply) else {
            break;
        };
        let mut retried = check_reply(raw, &context.reading, subject_id);
        retried.without_unseen_currency(&corpus);
        if retried.question.is_none() {
            break;
        }
        reply = retried;
        calls += 1;
    }
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
    summarize_with_effort(
        db, db_path, data_dir, sense, subject_id, turns, space, provider, model, None,
    )
    .await
}

/// [`summarize`] sending `reasoning_effort` to the model when given (the bench compares models).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn summarize_with_effort(
    db: &std::sync::Mutex<Connection>,
    db_path: &Path,
    data_dir: &Path,
    sense: Option<std::sync::Arc<crate::meaning::Sense>>,
    subject_id: Option<String>,
    turns: Vec<ReflectPreviousTurn>,
    space: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    effort: Option<&str>,
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
    let text = crate::commands::ai::call_llm_json_effort(
        SUMMARY_PROMPT,
        message,
        Some(provider_id),
        model,
        "summary",
        summary_schema(),
        db_path,
        crate::ai_usage::operation::REFLECT,
        effort,
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

    // Without it, a case shows up as a link with nothing of what it says.
    #[test]
    fn a_link_naming_only_a_title_is_dropped() {
        let reply = check_reply(
            raw(
                json!({ "restated": null, "links": [{ "kind": "supports", "excerpt": 1 }],
                "lead": null, "question": "Why?", "offer_summary": false }),
            ),
            &reading(),
            None,
        );
        assert!(reply.links.is_empty());
        assert_eq!(reply.dropped, 1);
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

    // A partner that only sees titles cannot link a case it has not read: when the words found
    // fewer than DETAILED_MIN cases, the latest ones are sent with their sentences too.
    #[test]
    fn a_small_space_is_read_in_full_even_when_no_word_matches() {
        let conn = db();
        let work = project(&conn, "Shop", crate::models::project::ProjectSpace::Work);
        let subject_id = add(&conn, &work, "Gift boxes", "Sell boxes of candles.");
        for (title, text) in [
            ("Wax stock", "Twelve kilos of wax remain."),
            ("Carrier", "The parcel carrier delivers in four days."),
            ("Returns", "Returns are accepted within fourteen days."),
        ] {
            add(&conn, &work, title, text);
        }
        let subject = load_subject(&conn, Some(&subject_id), Some("work")).unwrap();
        // Nothing the person says shares a word with the other cases.
        let turns = [turn_said("I want something for December.")];
        let query = search_text(subject.as_ref(), &turns, false);
        let context = prepare(&conn, subject, &turns, Some("work"), &[], query).unwrap();
        assert_eq!(context.reading.detailed.len(), 3, "all three are detailed");
        assert!(context.reading.overview.is_empty());
        assert!(context
            .reading
            .detailed
            .iter()
            .all(|c| c.id != subject_id && !c.sentences.is_empty()));
    }

    #[test]
    fn a_currency_nobody_used_is_left_out_of_the_reply() {
        let corpus = r#"{"cases": ["Retainer of 6 days at 430 per day"], "latest": "about 45 %"}"#;
        assert_eq!(
            without_unseen_currency("6 days at 430 $ or $520, 12 €", corpus),
            "6 days at 430 or 520, 12"
        );
        // A symbol the person or a case used stays.
        assert_eq!(
            without_unseen_currency("a fee of 50 € and 3 $", &format!("{corpus} 20 €")),
            "a fee of 50 € and 3"
        );
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

    #[test]
    fn the_questions_the_person_answered_are_settled() {
        assert_eq!(
            last_question("Vous gardez 4.5 mètres. Proposez-vous un montant libre ? Ou pas ?"),
            Some("Ou pas ?".to_string())
        );
        assert_eq!(
            last_question("Bien. ¿Quieres un monto libre o solo fijos?"),
            Some("¿Quieres un monto libre o solo fijos?".to_string())
        );
        assert_eq!(last_question("No question here."), None);
        let turns = vec![
            turn_said("Cards."),
            ReflectPreviousTurn {
                said: None,
                reply: Some("You want cards. Which amounts?".into()),
                source_ids: vec![],
            },
            turn_said("20 and 50."),
            ReflectPreviousTurn {
                said: None,
                reply: Some("Fixed amounts. A free amount too?".into()),
                source_ids: vec![],
            },
        ];
        // The last question is still waiting for its answer: not settled.
        assert_eq!(
            settled_pairs(&turns),
            vec![("Which amounts?".to_string(), "20 and 50.".to_string())]
        );
    }

    // Without this check, a question asking again about the point just answered is shown.
    #[test]
    fn the_words_a_repeating_question_shares_with_the_answer_are_named() {
        let settled = vec![(
            "Proposez-vous aussi un montant libre ?".to_string(),
            "Le montant libre plus tard.".to_string(),
        )];
        assert_eq!(
            settled_words_in("Quand ajouter le montant libre ?", &settled),
            vec!["libre".to_string(), "montant".to_string()]
        );
        assert!(settled_words_in("Quel fournisseur de paiement ?", &settled).is_empty());
    }

    #[test]
    fn a_question_holding_most_words_of_a_short_answer_repeats_it() {
        let settled = vec![(
            "Proposez-vous seulement ces montants fixes, ou aussi un montant libre ?".to_string(),
            "Le montant libre plus tard.".to_string(),
        )];
        assert!(repeats_settled(
            "Souhaitez-vous définir dès maintenant les conditions pour introduire le montant libre ?",
            &settled
        ));
        assert!(!repeats_settled(
            "Souhaitez-vous maintenant choisir la solution de paiement des cartes ?",
            &settled
        ));
        assert!(
            !repeats_settled("Un résumé du montant ?", &settled),
            "one word is not enough"
        );
        let long = vec![(
            "Q ?".to_string(),
            "Je prends le devis le moins cher sous quatre mille euros et j'appelle le vendeur cette semaine."
                .to_string(),
        )];
        assert!(
            !repeats_settled("Quel vendeur appeler cette semaine ?", &long),
            "a long answer is not told apart"
        );
    }

    // Without the plain wording, a result waiting for a decision reads as settled.
    #[test]
    fn a_result_waiting_for_a_decision_is_sent_as_a_proposal_not_chosen() {
        let conn = db();
        let ids = crate::ask::tests::build_base(&conn, &defects_base());
        let subject = load_subject(&conn, Some(&ids["cards"]), Some("work")).unwrap();
        let turns = vec![turn_said(
            "Les cartes cadeaux passent par le paiement de la boutique.",
        )];
        let query = search_text(subject.as_ref(), &turns, false);
        let mut context = prepare(&conn, subject, &turns, Some("work"), &[], query).unwrap();
        let message = context.message(&turns, false);
        let states: Vec<String> = message["cases"]
            .as_array()
            .unwrap()
            .iter()
            .chain(message["overview"].as_array().unwrap())
            .map(|c| c["state"].as_str().unwrap().to_string())
            .collect();
        assert!(
            states.iter().any(|s| s.contains("not chosen")),
            "{states:?}"
        );
        assert!(!states
            .iter()
            .any(|s| s == "result waiting for the person's decision"));
        assert_eq!(message["settled"], json!([]));
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

    // ---- Defects measured with real calls ----

    /// Set by the bench to measure the settled points without the second attempt.
    pub(super) static NO_RETRY: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    /// A small French space for the two defects: three cases holding "Proposition : X", waiting
    /// for the person's decision, and two cases the person's questions turn on.
    fn defects_base() -> Value {
        json!({ "lang": "fr", "cases": [
            { "key": "pay", "title": "Choix de la solution de paiement", "nature": "research",
              "status": "resolved", "agent_resolved": true,
              "content": "Trois solutions de paiement comparées : Stripe, PayPal et Mollie.\n\nProposition : Stripe pour le paiement, avec un bouton PayPal à côté.\n\nLe choix reste à faire." },
            { "key": "clay", "title": "Choisir le fournisseur d'argile", "nature": "research",
              "status": "resolved", "agent_resolved": true,
              "content": "Trois fournisseurs comparés : Ceradel, Solargil et Terres du Sud.\n\nProposition : Solargil, livraison en 48 h et 10 % moins cher que les autres.\n\nLe choix reste à faire." },
            { "key": "venue", "title": "Lieu du stage d'été", "nature": "research",
              "status": "resolved", "agent_resolved": true,
              "content": "Deux lieux comparés : la salle des Tilleuls et le gymnase municipal.\n\nProposition : la salle des Tilleuls, 120 € la journée.\n\nÀ décider avant fin mai." },
            { "key": "cards", "title": "Cartes cadeaux", "nature": "idea", "status": "open",
              "content": "Vendre des cartes cadeaux dans la boutique en ligne.\n\nPrévoir des montants fixes de 20, 50 et 100 €." },
            { "key": "camp", "title": "Stage d'été pour enfants", "nature": "idea", "status": "open",
              "content": "Un stage de cinq jours en juillet pour les 8 à 12 ans.\n\nIl faudrait un deuxième animateur pour encadrer le groupe." }
        ] })
    }

    /// Words next to `x` in `text` that say it was chosen, retained or decided, unless a negation
    /// or "to decide" is in the same window. A rough per-language check, printed with the reply
    /// for a human to judge.
    fn presents_as_chosen(text: &str, x: &str) -> bool {
        let text = text.to_lowercase();
        let words: Vec<&str> = text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        let chosen = [
            "choisi",
            "choisie",
            "retenu",
            "retenue",
            "décidé",
            "décidée",
            "déjà",
            "validé",
            "validée",
            "arrêté",
            "sélectionné",
            "adopté",
            "adoptée",
        ];
        let doubt = [
            "pas",
            "encore",
            "reste",
            "décider",
            "attend",
            "attente",
            "proposition",
            "proposé",
            "proposée",
            "suggéré",
            "envisagé",
            "si",
            "ou",
        ];
        words.iter().enumerate().any(|(i, w)| {
            if !w.contains(x) {
                return false;
            }
            let window = &words[i.saturating_sub(8)..(i + 9).min(words.len())];
            window.iter().any(|w| chosen.contains(w)) && !window.iter().any(|w| doubt.contains(w))
        })
    }

    /// Words (significant, from the question and the answer) the new question repeats.
    fn repeats(question: &str, asked: &str, answer: &str, subject: &str) -> (usize, usize) {
        let new = crate::ask::words_of(question);
        let subject = crate::ask::words_of(subject);
        let from_answer = new.intersection(&crate::ask::words_of(answer)).count();
        let from_asked = new
            .intersection(&crate::ask::words_of(asked))
            .filter(|w| !subject.contains(*w))
            .count();
        (from_answer, from_asked)
    }

    /// One reply of the partner against the real AI; `effort` is sent as `reasoning_effort`.
    #[allow(clippy::too_many_arguments)]
    fn play(
        db: &std::sync::Mutex<Connection>,
        db_path: &Path,
        subject: Option<String>,
        turns: Vec<ReflectPreviousTurn>,
        effort: Option<&str>,
    ) -> (ReflectReply, f64, f64, usize, usize) {
        let before = crate::ask::tests::usage_totals(db_path);
        let started = std::time::Instant::now();
        // A free tier refuses bursts (HTTP 429): wait and ask again.
        let mut result = Err(String::new());
        for wait in [0u64, 5, 10, 20, 40] {
            std::thread::sleep(std::time::Duration::from_secs(wait));
            result = tauri::async_runtime::block_on(super::reply_with_effort(
                db,
                db_path,
                db_path.parent().unwrap(),
                None,
                subject.clone(),
                turns.clone(),
                Some("work".into()),
                Some("cerebras".into()),
                None,
                effort,
            ));
            if !result.as_ref().is_err_and(|e| e.contains("429")) {
                break;
            }
        }
        let reply = result.unwrap();
        let seconds = started.elapsed().as_secs_f64();
        let after = crate::ask::tests::usage_totals(db_path);
        let calls = std::fs::read_to_string(db_path.parent().unwrap().join(JOURNAL_FILE))
            .ok()
            .and_then(|t| t.lines().last().map(str::to_string))
            .and_then(|l| serde_json::from_str::<Value>(&l).ok())
            .and_then(|e| e["calls"].as_u64())
            .unwrap_or(0) as usize;
        (
            reply,
            seconds,
            after.2 - before.2,
            (after.1 - before.1) as usize,
            calls,
        )
    }

    /// Measures the two defects seen in the real app, with real calls, `REFLECT_EVAL_RUNS` times
    /// (3) each: (a) a proposal waiting for the person's decision presented as chosen, (b) a
    /// question the person just answered, asked again. Also replays the four scenarios of
    /// `reflect_eval` once. `REFLECT_EVAL_EFFORT` (`high`; default: nothing sent, which is
    /// "medium" for gpt-oss-120b). Prints every reply in full.
    /// `cargo test reflect_defects -- --ignored --nocapture`
    #[test]
    #[ignore = "real AI calls"]
    fn reflect_defects() {
        let effort = std::env::var("REFLECT_EVAL_EFFORT").ok();
        let effort = effort.as_deref().filter(|e| *e != "default");
        let runs: usize = std::env::var("REFLECT_EVAL_RUNS")
            .ok()
            .and_then(|r| r.parse().ok())
            .unwrap_or(3);
        let conn = db();
        let ids = crate::ask::tests::build_base(&conn, &defects_base());
        let folder = tempfile::tempdir().unwrap();
        let db_path = folder.path().join("eval.db");
        let db = std::sync::Mutex::new(conn);
        let said = turn_said;
        let reply_turn = |text: &str| ReflectPreviousTurn {
            said: None,
            reply: Some(text.into()),
            source_ids: Vec::new(),
        };
        // (a) proposals: name, subject, what the person says, the proposed thing
        let proposals: Vec<(&str, Option<&str>, &str, &str)> = vec![
            ("a1 payment", Some("cards"),
             "Les cartes cadeaux doivent passer par le même paiement que la boutique. Qu'est-ce que je dois prévoir de ce côté-là ?", "stripe"),
            ("a2 clay", None,
             "Je passe commande d'argile cette semaine. On a retenu quel fournisseur déjà ?", "solargil"),
            ("a3 venue", Some("camp"),
             "Je prépare le courrier aux parents pour le stage d'été. Le lieu est déjà choisi, non ?", "tilleuls"),
        ];
        // (b) answered questions: name, subject, turns, (asked, answer)
        let answered: Vec<(&str, Option<&str>, Vec<ReflectPreviousTurn>)> = vec![
            ("b1 free amount", Some("cards"), vec![
                said("Je veux lancer des cartes cadeaux dans la boutique."),
                reply_turn("Vous voulez lancer des cartes cadeaux dans la boutique. Quels montants voulez-vous proposer ?"),
                said("Les montants fixes du cas, 20, 50 et 100 €."),
                reply_turn("Vous gardez les montants fixes de 20, 50 et 100 €. Proposez-vous seulement ces montants fixes, ou aussi un montant libre ?"),
                said("Le montant libre plus tard."),
            ]),
            ("b2 with the shop", Some("cards"), vec![
                said("Je prépare le lancement des cartes cadeaux."),
                reply_turn("Vous préparez le lancement des cartes cadeaux. Voulez-vous les lancer immédiatement avec la boutique, ou après le lancement de la boutique ?"),
                said("Avec la boutique."),
            ]),
            ("b3 twelve children", Some("camp"), vec![
                said("Je prépare le stage d'été pour les enfants."),
                reply_turn("Vous préparez le stage d'été pour les enfants. Combien d'enfants voulez-vous accueillir au maximum ?"),
                said("Douze enfants maximum."),
                reply_turn("Vous limitez le stage à douze enfants. Le deuxième animateur est-il déjà trouvé ?"),
                said("Pas encore, je m'en occupe la semaine prochaine."),
            ]),
        ];
        if std::env::var("REFLECT_EVAL_NO_RETRY").is_ok() {
            NO_RETRY.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let only = std::env::var("REFLECT_EVAL_ONLY").ok();
        let wanted = |name: &str| {
            only.as_deref()
                .is_none_or(|o| o.split(',').any(|p| name.starts_with(p)))
        };
        let proposals: Vec<_> = proposals.into_iter().filter(|p| wanted(p.0)).collect();
        let answered: Vec<_> = answered.into_iter().filter(|p| wanted(p.0)).collect();
        let (mut chosen_flags, mut again_flags, mut replies) = (0, 0, 0);
        let (mut seconds, mut cost, mut out_tokens) = (0.0, 0.0, 0);
        let mut results = Vec::new();
        for (name, subject, text, x) in &proposals {
            for run in 1..=runs {
                let (reply, secs, usd, out, calls) = play(
                    &db,
                    &db_path,
                    subject.map(|k| ids[k].clone()),
                    vec![said(text)],
                    effort,
                );
                let all = [&reply.restated, &reply.lead, &reply.question]
                    .iter()
                    .filter_map(|t| t.as_deref())
                    .collect::<Vec<_>>()
                    .join(" ");
                let flag = presents_as_chosen(&all, x);
                chosen_flags += usize::from(flag);
                replies += 1;
                seconds += secs;
                cost += usd;
                out_tokens += out;
                println!(
                    "\n--- {name} run {run}: {}  ({secs:.1} s, ${usd:.4}, {out} out tokens, {calls} calls)",
                    if flag { "FLAGGED as chosen" } else { "ok" }
                );
                println!(
                    "restated: {:?}\nlead: {:?}\nquestion: {:?}",
                    reply.restated, reply.lead, reply.question
                );
                results.push(json!({ "scenario": name, "run": run, "flag": flag, "reply": reply }));
            }
        }
        for (name, subject, turns) in &answered {
            let (asked, answer) = settled_pairs(&clean_turns(turns.clone())).pop().unwrap();
            let subject_text = subject
                .map(|k| {
                    let base = defects_base();
                    base["cases"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|c| c["key"] == k)
                        .unwrap()["title"]
                        .as_str()
                        .unwrap()
                        .to_string()
                })
                .unwrap_or_default();
            for run in 1..=runs {
                let (reply, secs, usd, out, calls) = play(
                    &db,
                    &db_path,
                    subject.map(|k| ids[k].clone()),
                    turns.clone(),
                    effort,
                );
                let question = reply.question.clone().unwrap_or_default();
                let (from_answer, from_asked) = repeats(&question, &asked, &answer, &subject_text);
                let flag = from_answer >= 2;
                again_flags += usize::from(flag);
                replies += 1;
                seconds += secs;
                cost += usd;
                out_tokens += out;
                println!("\n--- {name} run {run}: {}  (shares {from_answer} with the answer, {from_asked} with the old question; {secs:.1} s, ${usd:.4}, {out} out tokens, {calls} calls)", if flag { "FLAGGED as asked again" } else { "ok" });
                println!("asked before: {asked}\nanswer: {answer}\nrestated: {:?}\nlead: {:?}\nquestion: {:?}", reply.restated, reply.lead, reply.question);
                results.push(json!({ "scenario": name, "run": run, "flag": flag, "reply": reply }));
            }
        }
        let report = json!({
            "effort": effort.unwrap_or("default"),
            "runs_per_scenario": runs,
            "chosen_flags": format!("{chosen_flags}/{}", proposals.len() * runs),
            "asked_again_flags": format!("{again_flags}/{}", answered.len() * runs),
            "mean_seconds": seconds / replies as f64,
            "mean_cost_usd": cost / replies as f64,
            "mean_output_tokens": out_tokens / replies,
            "total_cost_usd": cost,
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
