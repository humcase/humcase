//! MCP prompts: short workflows a person can start from their client (a slash command, a menu),
//! for clients that do not load the Humcase plugin's skills. A prompt only builds a message; it
//! reads and writes nothing.

use serde_json::{json, Value};

/// Prompts offered by `prompts/list`.
pub fn prompt_definitions() -> Value {
    json!([
        {
            "name": "resume-work",
            "title": "Resume work from Humcase",
            "description": "Reload the context from Humcase (brief, reworks, results waiting for a decision) and say where things stand before acting.",
            "arguments": [
                { "name": "project_id", "description": "Project to resume (from list_projects); every project otherwise", "required": false }
            ]
        },
        {
            "name": "write-note",
            "title": "Record a case in Humcase",
            "description": "Record something in Humcase as a well-shaped case, filed inside the case it belongs to.",
            "arguments": [
                { "name": "subject", "description": "What to record", "required": false }
            ]
        }
    ])
}

fn argument<'a>(arguments: Option<&'a Value>, name: &str) -> Result<Option<&'a str>, String> {
    match arguments.and_then(|a| a.get(name)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.trim().is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.trim())),
        Some(_) => Err(format!("prompt argument {name} must be a string")),
    }
}

/// Messages of a prompt (`prompts/get`). An unknown prompt is an error.
pub fn get_prompt(name: &str, arguments: Option<&Value>) -> Result<Value, String> {
    let (description, text) = match name {
        "resume-work" => {
            let scope = match argument(arguments, "project_id")? {
                Some(id) => format!("get_handoff with project_id \"{id}\""),
                None => "get_handoff".to_string(),
            };
            (
                "Resume work from Humcase",
                format!(
                    "Resume my work from Humcase before doing anything else.
1. Call {scope}, then get_brief.
2. Reworks come first: for each one, read my reason (get_note: review.last_decision.comment).
3. Tell me in a few lines where things stand: reworks to do, cases in progress, results waiting for my decision, what is overdue.
4. Propose what to do next and wait for my answer. When you start a case, set it in_progress; resolve it only after verifying the result, with a report of what was done, how it was verified and what remains."
                ),
            )
        }
        "write-note" => {
            let subject = match argument(arguments, "subject")? {
                Some(subject) => format!("Record this in Humcase: {subject}"),
                None => "Record in Humcase what we just discussed.".to_string(),
            };
            (
                "Record a case in Humcase",
                format!(
                    "{subject}
1. Check it does not exist yet: find_similar_notes with its title.
2. Find the case it belongs to (list_notes with search_query, get_note, list_sub_cases) and create it inside with add_note and parent_id; a top-level case only when none fits.
3. Shape it as the add_note description says: short title, Markdown in my language, every fact kept and none added, the type that fits (todo, question, bug, decision...).
4. Tell me its title and where you filed it."
                ),
            )
        }
        other => {
            return Err(format!(
                "Unknown prompt '{other}'. Run prompts/list to see available prompts."
            ))
        }
    };
    Ok(json!({
        "description": description,
        "messages": [{ "role": "user", "content": { "type": "text", "text": text } }]
    }))
}
