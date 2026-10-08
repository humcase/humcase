//! Graded quality bench of "Think it through" (ignored test, real AI calls).
//!
//! Ten invented situations (`reflect_quality_cases.json`), each a small space of cases and a
//! hidden brief for the person. A simulated person answers the partner's latest question from
//! its brief (one call per turn, operation `reflect_eval`); after four partner turns the
//! conversation is summarized. A judge (temperature 0, strict rubric) grades every partner
//! reply and every summary item by item. Everything is printed to `REFLECT_QUALITY_OUT`.
//!
//! `cargo test reflect_quality -- --ignored --nocapture`
//! Environment: `REFLECT_QUALITY_ONLY` (comma-separated situation id prefixes),
//! `REFLECT_QUALITY_TURNS` (partner turns, 4), `REFLECT_QUALITY_PARALLEL` (situations at once, 3),
//! `REFLECT_QUALITY_JUDGE_EFFORT` (reasoning effort of the judge, `low`), `REFLECT_QUALITY_OUT`
//! (text of every conversation; the same data goes to `<out>.json`).

use crate::reflect::{
    reply_with_effort, summarize, ReflectPreviousTurn, ReflectReply, ReflectSummary,
};
use serde_json::{json, Value};
use std::path::Path;

const OPERATION: &str = "reflect_eval";

/// An item of the rubric: the judge answers yes/no flags, the score is computed here (1 when
/// every positive flag is true and no negative flag is), which keeps a reasoning judge from
/// being lenient in one overall verdict.
struct ItemSpec {
    name: &'static str,
    positive: &'static [&'static str],
    negative: &'static [&'static str],
}

const REPLY_SPECS: [ItemSpec; 5] = [
    ItemSpec {
        name: "specific",
        positive: &["uses_element_of_this_situation"],
        negative: &["generic_wording"],
    },
    ItemSpec {
        name: "forward",
        positive: &["builds_on_latest"],
        negative: &["repeats_answered_point", "changes_topic"],
    },
    ItemSpec {
        name: "useful",
        positive: &["changes_decision_or_reveals"],
        negative: &[
            "asks_person_to_think_or_compute",
            "uncued_process_or_checklist",
            "already_answered",
            "ignores_visible_tension",
            "offers_help_or_asks_permission",
        ],
    },
    ItemSpec {
        name: "lead_value",
        positive: &["ties_case_or_fact", "adds_something_new"],
        negative: &[
            "generic_method",
            "unsupported_fact",
            "misapplied_case",
            "repeats_person",
        ],
    },
    ItemSpec {
        name: "links_relevant",
        positive: &["every_passage_bears", "kind_right"],
        negative: &["background_only"],
    },
];
const SUMMARY_SPECS: [ItemSpec; 2] = [
    ItemSpec {
        name: "faithful",
        positive: &[],
        negative: &[
            "invented",
            "considered_listed_as_kept",
            "unsaid_rejection",
            "action_not_said",
        ],
    },
    ItemSpec {
        name: "actionable",
        positive: &["concrete_agreed_steps_or_open_decision_stated"],
        negative: &["vague_or_unagreed_actions", "named_steps_missing"],
    },
];

const PERSON_PROMPT: &str = "You play a real person who is thinking something through with a thinking partner (a colleague who has read their notes). You have a private brief: your goal, facts only you know, what you hesitate about, and how you talk. You answer the partner's latest message.
Rules:
- Answer the partner's latest question, always in the language given in `language` (fr: French, en: English), whatever language the partner writes in, in the first person, the way this person really types: follow `style`. One or two short sentences, 25 words at most; shorter when your style is terse.
- You do not know in advance what the partner will ask. Give a fact from `knows` only when the question or your answer calls for it; never volunteer the whole brief. A question that asks for something your brief does not cover: answer as you honestly would (\"no idea yet\", \"I haven't looked\", a doubt), and invent no figure, name, date or fact that is not in your brief; do no calculation.
- If the partner's message offers a lead or a suggestion, react to it honestly (take it, push back, or ignore it) before or while answering.
- If the partner offers a summary: accept briefly only if your hesitation has really been settled in the conversation; otherwise say what is still unsettled for you and keep thinking.
- Never ask the partner questions back unless it is a short natural doubt. Never mention the brief.";

const JUDGE_PROMPT: &str = "You are a strict reviewer of a thinking partner (an AI) that helps a person think an idea, a problem or a choice through using the person's notes (`cases`). You review ONE reply of the partner by answering yes/no flags for five items, each with a one-sentence reason (English, 25 words at most). Be strict: when in doubt, answer the way that gives the partner no credit. Do not reward politeness, length or fluency: a correct reply can still be generic.
Material: `cases` (the notes of the person's space: title and text), `subject` (the case the person started from, or null), `conversation` (what the person and the partner said before, oldest first), `person_latest` (the person's last message, null at the very beginning), and `reply` (the partner's reply: `restated`, `links` with their `kind` and cited `passages`, `lead`, `question`).
Items and flags (the flags concern the `question`, except for `lead_value` and `links_relevant`):
- specific. `uses_element_of_this_situation`: the question names a figure, name, option, constraint or date of this situation (from the person's words or from a case), beyond the topic's name, or it ties two of the person's own statements together (naming the topic again, \"these evening classes\", is not enough). `generic_wording`: the question is only about a generic category (budget, price, deadline, criteria, plan, method, who, how many, next step) with nothing of this situation attached, so that it could be asked unchanged in another situation.
- forward. `builds_on_latest`: the question takes the person's latest message as given and goes to what it opens or leaves open (at the very beginning: it builds on the subject or the opening text). `repeats_answered_point`: it asks again about a point the person already answered in the conversation, or about something the person has just said they do not know or do not have (asking for information the person has not given yet is normal and does NOT count). `changes_topic`: it ignores the latest message, goes back to an earlier topic or opens a subject the person did not.
- useful. `changes_decision_or_reveals`: the answer would change what the person decides or reveal something they have not considered, and a thoughtful colleague who knows these cases would plausibly ask it next. `asks_person_to_think_or_compute`: it asks the person to do the partner's own thinking or arithmetic (compute, estimate, subtract, choose the criteria, weights, method, percentage or tool, say which to calculate first). `uncued_process_or_checklist`: it is a process or checklist item (next step, plan, tracking, deadline, who, how many, budget, price, criteria, how to get information) that neither a case nor the conversation shows to be the bottleneck. `already_answered`: the cases or the conversation already answer it. `ignores_visible_tension`: what the person said clashes with a figure or fact of a case (or an earlier statement) and the reply neither links nor raises it. `offers_help_or_asks_permission`: it offers help (\"do you want me to...\"), asks permission, or asks whether to give a summary.
- lead_value (answer the flags even when `lead` is null; they are then ignored). `ties_case_or_fact`: the lead draws on a case's sentence or a specific fact the person gave. `adds_something_new`: it is a non-obvious angle the person did not say. `generic_method`: it is advice any consultant would give (list your criteria, look at reviews, compare the options, ask several quotes, estimate the loss, build a scoring table, check the documents). `unsupported_fact`: it contains a figure, percentage, option, currency or fact that is in neither the cases nor the conversation. `misapplied_case`: it applies a case to a situation it does not concern (a rule for new clients to an old client). `repeats_person`: it only repeats what the person said.
- links_relevant (answer the flags even when `links` is empty; they are then ignored). `every_passage_bears`: every cited passage is about the point the person just raised. `kind_right`: `supports` means the passage goes the same way as what the person said, `conflicts` that it opposes or constrains it, `decided` that it is a decision already taken on that very point; every link has the right kind. `background_only`: a passage is only background (same person, object or place) and changes nothing to the discussion.
Calibration (invented example, a florist adding weddings; the person has just said the supplier sells roses at 0.90 a stem, minimum 200 stems; a case says the van is booked every Saturday in June):
- \"What budget have you planned for the flowers?\" -> generic_wording, repeats_answered_point (the person just gave the price), uncued_process_or_checklist.
- \"With 200 stems minimum, how many wedding bouquets would that cover before the first Saturday in June?\" -> uses_element_of_this_situation, builds_on_latest, changes_decision_or_reveals.
- \"Which caterer will you work with?\" when the person never spoke of catering -> changes_topic.
- A restatement that repeats a figure without noticing that it contradicts another figure of the cases, followed by a question that moves on -> ignores_visible_tension.
- Lead \"look at what competitors charge\" -> generic_method. Lead \"the van is booked every Saturday in June, so a June wedding would need another way to deliver\" -> ties_case_or_fact, adds_something_new.
Most partner questions in practice are mediocre: expect many flags raised.
Answer with the JSON object only.";

const SUMMARY_JUDGE_PROMPT: &str = "You are a strict reviewer of the summary of a conversation in which a person thought something through with an AI partner. You review it by answering yes/no flags for two items, each with a one-sentence reason (English, 25 words at most). Be strict: when in doubt, answer the way that gives the summary no credit.
Material: `conversation` (oldest first: `person`, `partner`), `cases` (the person's notes, for reference), and `summary`. The title is only a label and need not be quoted; `waiting_on` lists cases the person's thinking depends on and is fine when a case in `cases` bears on what was said.
- faithful. `invented`: a fact, figure, option or reason appears in the summary that nobody said in the conversation. `considered_listed_as_kept`: something the person only considered (\"could\", \"might\", \"envisage\") is listed as kept or decided. `unsaid_rejection`: something is listed as rejected that nobody rejected. `action_not_said`: an action is not what the person said.
- actionable. `concrete_agreed_steps_or_open_decision_stated`: the actions are concrete next steps the person agreed to (create=true must have been agreed; an idea for later is create=false), or, when the person agreed to no step, the direction says plainly what is still to decide. `vague_or_unagreed_actions`: actions are vague (\"think about it\") or were not agreed but are marked to create. `named_steps_missing`: the person named steps that are missing.
Answer with the JSON object only.";

fn judge_schema(specs: &[ItemSpec]) -> Value {
    let properties: serde_json::Map<String, Value> = specs
        .iter()
        .map(|spec| {
            let mut fields = serde_json::Map::new();
            let mut required = Vec::new();
            for flag in spec.positive.iter().chain(spec.negative.iter()) {
                fields.insert(flag.to_string(), json!({ "type": "boolean" }));
                required.push(json!(flag));
            }
            fields.insert("reason".into(), json!({ "type": "string" }));
            required.push(json!("reason"));
            (
                spec.name.to_string(),
                json!({
                    "type": "object",
                    "properties": fields,
                    "required": required,
                    "additionalProperties": false
                }),
            )
        })
        .collect();
    json!({
        "type": "object",
        "properties": properties,
        "required": specs.iter().map(|s| s.name).collect::<Vec<_>>(),
        "additionalProperties": false
    })
}

/// What one eval call cost and took.
#[derive(Clone, Copy, Default)]
struct Spent {
    usd: f64,
    secs: f64,
}

/// One call of the simulated person or of the judge: a JSON answer, `temperature` and effort
/// sent when given; asked again on HTTP 429; recorded under the operation `reflect_eval`.
fn eval_call(
    system: &str,
    user: Value,
    schema_name: &str,
    schema: Value,
    temperature: Option<f64>,
    effort: Option<&str>,
    db_path: &Path,
) -> Result<(Value, Spent), String> {
    let resolved = crate::providers::resolve_llm("cerebras", None)?;
    let mut body = json!({
        "model": resolved.model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user.to_string() }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": { "name": schema_name, "strict": true, "schema": schema }
        }
    });
    if let Some(t) = temperature {
        body["temperature"] = json!(t);
    }
    if let Some(e) = effort {
        body["reasoning_effort"] = json!(e);
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())?;
    let mut last = String::new();
    for wait in [0u64, 5, 10, 20, 40, 60, 90] {
        std::thread::sleep(std::time::Duration::from_secs(wait));
        let started = std::time::Instant::now();
        let mut request = client.post(&resolved.url).json(&body);
        if let Some(key) = &resolved.key {
            request = request.bearer_auth(key);
        }
        let response = tauri::async_runtime::block_on(async {
            let response = request.send().await.map_err(|e| e.to_string())?;
            let status = response.status().as_u16();
            let data: Value = response.json().await.map_err(|e| e.to_string())?;
            Ok::<_, String>((status, data))
        });
        let (status, data) = match response {
            Ok(r) => r,
            Err(e) => {
                last = e;
                continue;
            }
        };
        if status == 429 {
            last = "HTTP 429".into();
            continue;
        }
        if !(200..300).contains(&status) {
            return Err(format!("HTTP {status}: {data}"));
        }
        let tokens = crate::ai_usage::TokenUsage::from_response(&data);
        let usd = crate::ai_usage::token_cost("cerebras", &resolved.model, &tokens).unwrap_or(0.0);
        let secs = started.elapsed().as_secs_f64();
        crate::ai_usage::record(
            db_path,
            &crate::ai_usage::UsageEntry {
                operation: OPERATION,
                provider: "cerebras",
                model: &resolved.model,
                language: None,
                estimated_cost_usd: Some(usd),
                tokens,
                audio_seconds: None,
                duration_ms: (secs * 1000.0) as u64,
                ok: true,
                prices_as_of: crate::ai_usage::PRICES_AS_OF,
            },
        );
        let text = data["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default();
        let value =
            crate::ask::json_object_in(text).ok_or_else(|| format!("no JSON in {text:?}"))?;
        return Ok((value, Spent { usd, secs }));
    }
    Err(format!("eval call failed: {last}"))
}

fn said(text: &str) -> ReflectPreviousTurn {
    ReflectPreviousTurn {
        said: Some(text.into()),
        reply: None,
        source_ids: Vec::new(),
    }
}

/// The partner's reply as the interface sends it back on the next turn.
fn reply_text(reply: &ReflectReply) -> String {
    [&reply.restated, &reply.lead, &reply.question]
        .iter()
        .filter_map(|t| t.as_deref())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `{ role, text }` pairs of the conversation, for the person and the judge.
fn dialog(turns: &[ReflectPreviousTurn]) -> Vec<Value> {
    turns
        .iter()
        .filter_map(|t| {
            t.said
                .as_ref()
                .map(|s| json!({ "role": "person", "text": s }))
                .or_else(|| {
                    t.reply
                        .as_ref()
                        .map(|r| json!({ "role": "partner", "text": r }))
                })
        })
        .collect()
}

fn cases_json(situation: &Value) -> Value {
    Value::Array(
        situation["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| json!({ "title": c["title"], "text": c["content"] }))
            .collect(),
    )
}

/// The grade of one item: `None` when it does not apply.
type Grades = Vec<(String, Option<i64>, String)>;

fn grades_of(value: &Value, specs: &[ItemSpec]) -> Grades {
    specs
        .iter()
        .map(|spec| {
            let item = &value[spec.name];
            let flag = |name: &str| item[name].as_bool().unwrap_or(false);
            let missing: Vec<&str> = spec.positive.iter().copied().filter(|f| !flag(f)).collect();
            let raised: Vec<&str> = spec.negative.iter().copied().filter(|f| flag(f)).collect();
            let score = missing.is_empty() && raised.is_empty();
            let mut notes: Vec<String> = raised.iter().map(|f| f.to_string()).collect();
            notes.extend(missing.iter().map(|f| format!("not {f}")));
            let reason = format!(
                "{} [{}]",
                item["reason"].as_str().unwrap_or(""),
                notes.join(", ")
            );
            (spec.name.to_string(), Some(i64::from(score)), reason)
        })
        .collect()
}

struct Config {
    turns: usize,
    judge_effort: Option<String>,
}

/// The reply of the partner against the real AI: asked again on HTTP 429.
fn partner_reply(
    db: &std::sync::Mutex<rusqlite::Connection>,
    db_path: &Path,
    subject: Option<String>,
    turns: &[ReflectPreviousTurn],
) -> (ReflectReply, Spent) {
    let before = crate::ask::tests::usage_totals(db_path);
    let started = std::time::Instant::now();
    let mut result = Err(String::new());
    for wait in [0u64, 5, 10, 20, 40, 60, 90] {
        std::thread::sleep(std::time::Duration::from_secs(wait));
        result = tauri::async_runtime::block_on(reply_with_effort(
            db,
            db_path,
            db_path.parent().unwrap(),
            None,
            subject.clone(),
            turns.to_vec(),
            Some("work".into()),
            Some("cerebras".into()),
            None,
            None,
        ));
        if !result.as_ref().is_err_and(|e| e.contains("429")) {
            break;
        }
    }
    let reply = result.expect("the partner replies");
    let after = crate::ask::tests::usage_totals(db_path);
    (
        reply,
        Spent {
            usd: after.2 - before.2,
            secs: started.elapsed().as_secs_f64(),
        },
    )
}

fn reply_json(reply: &ReflectReply) -> Value {
    json!({
        "restated": reply.restated,
        "links": reply.links.iter().map(|l| json!({
            "kind": format!("{:?}", l.kind).to_lowercase(),
            "title": l.source.title,
            "passages": l.source.passages,
        })).collect::<Vec<_>>(),
        "lead": reply.lead,
        "question": reply.question,
        "offer_summary": reply.offer_summary,
    })
}

fn summary_json(summary: &ReflectSummary) -> Value {
    json!({
        "title": summary.title,
        "direction": summary.direction,
        "kept": summary.kept,
        "rejected": summary.rejected,
        "evolution": summary.evolution,
        "questions": summary.questions,
        "actions": summary.actions.iter().map(|a| json!({ "title": a.title, "create": a.create })).collect::<Vec<_>>(),
        "waiting_on": summary.waiting_on.iter().map(|w| w.title.clone()).collect::<Vec<_>>(),
    })
}

fn judge_reply(
    situation: &Value,
    before: &[Value],
    reply: &Value,
    config: &Config,
    db_path: &Path,
) -> (Grades, Spent) {
    let latest = before
        .iter()
        .rev()
        .find(|t| t["role"] == "person")
        .map(|t| t["text"].clone())
        .unwrap_or(Value::Null);
    let subject = situation["subject"]
        .as_str()
        .and_then(|k| {
            situation["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["key"] == k)
        })
        .map(|c| json!({ "title": c["title"], "text": c["content"] }))
        .unwrap_or(Value::Null);
    let message = json!({
        "cases": cases_json(situation),
        "subject": subject,
        "conversation": before,
        "person_latest": latest,
        "reply": reply,
    });
    let (value, spent) = eval_call(
        JUDGE_PROMPT,
        message,
        "judge",
        judge_schema(&REPLY_SPECS),
        Some(0.0),
        config.judge_effort.as_deref(),
        db_path,
    )
    .expect("the judge answers");
    let mut grades = grades_of(&value, &REPLY_SPECS);
    for (item, score, _) in grades.iter_mut() {
        // Not applicable without a lead or without links, whatever the judge said.
        if (item == "lead_value" && reply["lead"].is_null())
            || (item == "links_relevant" && reply["links"].as_array().is_none_or(|l| l.is_empty()))
        {
            *score = None;
        } else if score.is_none() {
            *score = Some(0);
        }
    }
    (grades, spent)
}

fn judge_summary(
    situation: &Value,
    conversation: &[Value],
    summary: &Value,
    config: &Config,
    db_path: &Path,
) -> (Grades, Spent) {
    let message = json!({
        "conversation": conversation,
        "cases": cases_json(situation),
        "summary": summary,
    });
    let (value, spent) = eval_call(
        SUMMARY_JUDGE_PROMPT,
        message,
        "summary_judge",
        judge_schema(&SUMMARY_SPECS),
        Some(0.0),
        config.judge_effort.as_deref(),
        db_path,
    )
    .expect("the judge answers");
    let mut grades = grades_of(&value, &SUMMARY_SPECS);
    for (_, score, _) in grades.iter_mut() {
        if score.is_none() {
            *score = Some(0);
        }
    }
    (grades, spent)
}

fn person_answer(
    situation: &Value,
    turns: &[ReflectPreviousTurn],
    db_path: &Path,
) -> (String, Spent) {
    let message = json!({
        "brief": situation["brief"],
        "language": situation["lang"],
        "started_from": situation["subject"].as_str().map(|k| {
            situation["cases"].as_array().unwrap().iter()
                .find(|c| c["key"] == k).map(|c| c["title"].clone()).unwrap_or(Value::Null)
        }),
        "conversation": dialog(turns),
    });
    let (value, spent) = eval_call(
        PERSON_PROMPT,
        message,
        "person",
        json!({
            "type": "object",
            "properties": { "answer": { "type": "string" } },
            "required": ["answer"],
            "additionalProperties": false
        }),
        Some(0.7),
        Some("low"),
        db_path,
    )
    .expect("the person answers");
    (
        value["answer"].as_str().unwrap_or("").trim().to_string(),
        spent,
    )
}

/// Plays one situation; returns its record (every reply, grade, cost and time).
fn run_situation(situation: &Value, config: &Config) -> Value {
    let conn = crate::ask::tests::db();
    let base = json!({ "lang": situation["lang"], "cases": situation["cases"] });
    let ids = crate::ask::tests::build_base(&conn, &base);
    let folder = tempfile::tempdir().unwrap();
    let db_path = folder.path().join("quality.db");
    let db = std::sync::Mutex::new(conn);
    let subject = situation["subject"].as_str().map(|k| ids[k].clone());
    let mut turns: Vec<ReflectPreviousTurn> = Vec::new();
    if let Some(opening) = situation["opening"].as_str() {
        turns.push(said(opening));
    }
    let mut records = Vec::new();
    let (mut judge_cost, mut person_cost) = (0.0, 0.0);
    for index in 0..config.turns {
        let (reply, spent) = partner_reply(&db, &db_path, subject.clone(), &turns);
        let before = dialog(&turns);
        let (grades, judged) =
            judge_reply(situation, &before, &reply_json(&reply), config, &db_path);
        judge_cost += judged.usd;
        let latest = turns.iter().rev().find_map(|t| t.said.clone());
        turns.push(ReflectPreviousTurn {
            said: None,
            reply: Some(reply_text(&reply)),
            source_ids: reply
                .links
                .iter()
                .map(|l| l.source.note_id.clone())
                .collect(),
        });
        let (answer, answered) = person_answer(situation, &turns, &db_path);
        person_cost += answered.usd;
        turns.push(said(&answer));
        records.push(json!({
            "turn": index + 1,
            "person_before": latest,
            "dialog_before": before,
            "reply": reply_json(&reply),
            "grades": grades.iter().map(|(i, s, r)| json!({ "item": i, "score": s, "reason": r })).collect::<Vec<_>>(),
            "reply_usd": spent.usd,
            "reply_secs": spent.secs,
            "person_answer": answer,
        }));
    }
    let before = crate::ask::tests::usage_totals(&db_path).2;
    let started = std::time::Instant::now();
    let mut summary = Err(String::new());
    for wait in [0u64, 5, 10, 20, 40, 60, 90] {
        std::thread::sleep(std::time::Duration::from_secs(wait));
        summary = tauri::async_runtime::block_on(summarize(
            &db,
            &db_path,
            folder.path(),
            None,
            subject.clone(),
            turns.clone(),
            Some("work".into()),
            Some("cerebras".into()),
            None,
        ));
        if !summary.as_ref().is_err_and(|e| e.contains("429")) {
            break;
        }
    }
    let summary = summary.expect("the summary is made");
    let summary_usd = crate::ask::tests::usage_totals(&db_path).2 - before;
    let summary_secs = started.elapsed().as_secs_f64();
    let final_dialog = dialog(&turns);
    let (summary_grades, judged) = judge_summary(
        situation,
        &final_dialog,
        &summary_json(&summary),
        config,
        &db_path,
    );
    judge_cost += judged.usd;
    json!({
        "id": situation["id"],
        "lang": situation["lang"],
        "work": situation["work"],
        "subject": situation["subject"],
        "opening": situation["opening"],
        "brief": situation["brief"],
        "replies": records,
        "summary": summary_json(&summary),
        "dialog_final": final_dialog,
        "summary_grades": summary_grades.iter().map(|(i, s, r)| json!({ "item": i, "score": s, "reason": r })).collect::<Vec<_>>(),
        "summary_usd": summary_usd,
        "summary_secs": summary_secs,
        "judge_usd": judge_cost,
        "person_usd": person_cost,
    })
}

fn text_of(record: &Value) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "\n==================== {} ({}, {})",
        record["id"].as_str().unwrap_or(""),
        record["lang"].as_str().unwrap_or(""),
        record["work"].as_str().unwrap_or("")
    );
    let _ = writeln!(out, "subject: {}", record["subject"]);
    let _ = writeln!(out, "opening: {}", record["opening"]);
    let _ = writeln!(out, "brief: {}", record["brief"]);
    for turn in record["replies"].as_array().unwrap() {
        let _ = writeln!(out, "\n--- turn {}", turn["turn"]);
        if let Some(p) = turn["person_before"].as_str() {
            let _ = writeln!(out, "PERSON: {p}");
        }
        let r = &turn["reply"];
        let _ = writeln!(out, "PARTNER restated: {}", r["restated"]);
        for l in r["links"].as_array().unwrap() {
            let _ = writeln!(
                out,
                "  link {} -> {} {}",
                l["kind"], l["title"], l["passages"]
            );
        }
        let _ = writeln!(out, "  lead: {}", r["lead"]);
        let _ = writeln!(out, "  question: {}", r["question"]);
        let _ = writeln!(out, "  offer_summary: {}", r["offer_summary"]);
        for g in turn["grades"].as_array().unwrap() {
            let _ = writeln!(
                out,
                "  [{} {}] {}",
                g["item"].as_str().unwrap(),
                g["score"],
                g["reason"].as_str().unwrap()
            );
        }
        let _ = writeln!(
            out,
            "  (${:.4}, {:.1} s)\nPERSON answers: {}",
            turn["reply_usd"].as_f64().unwrap_or(0.0),
            turn["reply_secs"].as_f64().unwrap_or(0.0),
            turn["person_answer"].as_str().unwrap_or("")
        );
    }
    let _ = writeln!(
        out,
        "\n--- summary\n{}",
        serde_json::to_string_pretty(&record["summary"]).unwrap()
    );
    for g in record["summary_grades"].as_array().unwrap() {
        let _ = writeln!(
            out,
            "  [{} {}] {}",
            g["item"].as_str().unwrap(),
            g["score"],
            g["reason"].as_str().unwrap()
        );
    }
    out
}

/// Grades the replies and summaries of an earlier run again (`REFLECT_QUALITY_REJUDGE` is its
/// `.json` file) with the current judge: only the judge is paid, the conversations are kept.
/// Prints every grade that changed.
fn rejudge(bench: &Value, path: &str, config: &Config, only: &[Value]) -> Vec<Value> {
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let folder = tempfile::tempdir().unwrap();
    let db_path = folder.path().join("rejudge.db");
    let mut out = Vec::new();
    for record in saved["records"].as_array().unwrap() {
        let id = record["id"].as_str().unwrap();
        if !only.iter().any(|s| s["id"] == id) {
            continue;
        }
        let situation = bench["situations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == id)
            .unwrap();
        let mut record = record.clone();
        let mut judge_usd = 0.0;
        for turn in record["replies"].as_array_mut().unwrap() {
            let (grades, spent) = judge_reply(
                situation,
                turn["dialog_before"].as_array().unwrap(),
                &turn["reply"],
                config,
                &db_path,
            );
            judge_usd += spent.usd;
            for (item, score, reason) in &grades {
                let old = turn["grades"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|g| g["item"] == item.as_str())
                    .map(|g| g["score"].clone())
                    .unwrap_or(Value::Null);
                if old != json!(score) {
                    println!(
                        "{id} turn {} {item}: {old} -> {score:?} ({reason})",
                        turn["turn"]
                    );
                }
            }
            turn["grades"] = json!(grades
                .iter()
                .map(|(i, s, r)| json!({ "item": i, "score": s, "reason": r }))
                .collect::<Vec<_>>());
        }
        let (grades, spent) = judge_summary(
            situation,
            record["dialog_final"].as_array().unwrap(),
            &record["summary"],
            config,
            &db_path,
        );
        judge_usd += spent.usd;
        record["summary_grades"] = json!(grades
            .iter()
            .map(|(i, s, r)| json!({ "item": i, "score": s, "reason": r }))
            .collect::<Vec<_>>());
        record["judge_usd"] = json!(judge_usd);
        println!("{}", text_of(&record));
        out.push(record);
    }
    out
}

#[test]
#[ignore = "real AI calls"]
fn reflect_quality() {
    let bench: Value = serde_json::from_str(include_str!("reflect_quality_cases.json")).unwrap();
    let only = std::env::var("REFLECT_QUALITY_ONLY").ok();
    let situations: Vec<Value> = bench["situations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| {
            only.as_deref().is_none_or(|o| {
                o.split(',')
                    .any(|p| s["id"].as_str().unwrap().starts_with(p))
            })
        })
        .cloned()
        .collect();
    let config = std::sync::Arc::new(Config {
        turns: std::env::var("REFLECT_QUALITY_TURNS")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(4),
        judge_effort: Some(
            std::env::var("REFLECT_QUALITY_JUDGE_EFFORT").unwrap_or_else(|_| "low".into()),
        )
        .filter(|e| e != "default"),
    });
    let parallel: usize = std::env::var("REFLECT_QUALITY_PARALLEL")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or(3);
    let mut records: Vec<Value> = Vec::new();
    if let Ok(path) = std::env::var("REFLECT_QUALITY_REJUDGE") {
        records = rejudge(&bench, &path, &config, &situations);
    } else {
        for chunk in situations.chunks(parallel.max(1)) {
            let handles: Vec<_> = chunk
                .iter()
                .cloned()
                .map(|situation| {
                    let config = config.clone();
                    std::thread::spawn(move || run_situation(&situation, &config))
                })
                .collect();
            for handle in handles {
                let Ok(record) = handle.join() else {
                    println!(
                        "
A situation failed (rate limit?): it is left out."
                    );
                    continue;
                };
                println!("{}", text_of(&record));
                if let Ok(out) = std::env::var("REFLECT_QUALITY_OUT") {
                    use std::io::Write;
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(format!("{out}.partial.jsonl"))
                    {
                        let _ = writeln!(file, "{record}");
                    }
                }
                records.push(record);
            }
        }
    }

    // Rates per item.
    let mut totals: Vec<(String, i64, i64)> = Vec::new();
    let mut add = |item: &str, score: Option<i64>| {
        let Some(score) = score else {
            return;
        };
        match totals.iter_mut().find(|t| t.0 == item) {
            Some(t) => {
                t.1 += score;
                t.2 += 1;
            }
            None => totals.push((item.to_string(), score, 1)),
        }
    };
    let (mut reply_usd, mut reply_secs, mut replies) = (0.0, 0.0, 0usize);
    let (mut summary_usd, mut judge_usd, mut person_usd) = (0.0, 0.0, 0.0);
    for record in &records {
        for turn in record["replies"].as_array().unwrap() {
            for g in turn["grades"].as_array().unwrap() {
                add(g["item"].as_str().unwrap(), g["score"].as_i64());
            }
            reply_usd += turn["reply_usd"].as_f64().unwrap_or(0.0);
            reply_secs += turn["reply_secs"].as_f64().unwrap_or(0.0);
            replies += 1;
        }
        for g in record["summary_grades"].as_array().unwrap() {
            add(g["item"].as_str().unwrap(), g["score"].as_i64());
        }
        summary_usd += record["summary_usd"].as_f64().unwrap_or(0.0);
        judge_usd += record["judge_usd"].as_f64().unwrap_or(0.0);
        person_usd += record["person_usd"].as_f64().unwrap_or(0.0);
    }
    let rates: serde_json::Map<String, Value> = totals
        .iter()
        .map(|(item, sum, n)| (item.clone(), json!(format!("{sum}/{n}"))))
        .collect();
    let report = json!({
        "situations": records.len(),
        "replies": replies,
        "rates": rates,
        "mean_reply_usd": reply_usd / replies.max(1) as f64,
        "mean_reply_secs": reply_secs / replies.max(1) as f64,
        "summary_usd": summary_usd,
        "judge_usd": judge_usd,
        "person_usd": person_usd,
        "total_usd": reply_usd + summary_usd + judge_usd + person_usd,
    });
    println!("\n{}", serde_json::to_string_pretty(&report).unwrap());
    if let Ok(out) = std::env::var("REFLECT_QUALITY_OUT") {
        let mut text = serde_json::to_string_pretty(&report).unwrap();
        for record in &records {
            text.push_str(&text_of(record));
        }
        std::fs::write(&out, text).unwrap();
        std::fs::write(
            format!("{out}.json"),
            serde_json::to_string_pretty(&json!({ "report": report, "records": records })).unwrap(),
        )
        .unwrap();
    }
}
