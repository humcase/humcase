//! Claude through Anthropic's native Messages API.
//!
//! Anthropic's OpenAI-compatible endpoint ignores `response_format` and `reasoning_effort` and is
//! meant for tests only, so Humcase speaks the native API: the rest of the code keeps building
//! OpenAI-shaped messages, [`request`] turns them into a Messages request and [`to_openai`] turns
//! the answer back into a chat completion (docs read on 2026-10-08:
//! platform.claude.com/docs/en/build-with-claude/structured-outputs, /effort, /thinking).

use serde_json::{json, Value};

/// Provider id in [`crate::providers::LLM_PROVIDERS`].
pub const ID: &str = "anthropic";
/// Value of the `anthropic-version` header.
pub const VERSION: &str = "2023-06-01";
/// Room for the thinking and the answer: thinking counts toward `max_tokens`, and the tokens
/// are billed only when spent.
const MAX_TOKENS: u64 = 16_000;
/// The assistant's content blocks as Anthropic returned them, kept on the OpenAI-shaped message:
/// thinking blocks must be sent back unchanged with the tool results that follow them.
pub const RAW_CONTENT: &str = "anthropic_content";

/// A Messages request from OpenAI-shaped `messages` (system, user, assistant with `tool_calls`,
/// tool results), `tools`, `tool_choice`, a reasoning effort and a `response_format`.
pub fn request(
    model: &str,
    messages: &[Value],
    tools: Option<&Value>,
    tool_choice: Option<&Value>,
    effort: Option<&str>,
    response_format: Option<&Value>,
) -> Value {
    let mut system: Vec<String> = Vec::new();
    let mut turns: Vec<Value> = Vec::new();
    for message in messages {
        match message["role"].as_str().unwrap_or_default() {
            "system" | "developer" => system.push(plain_text(&message["content"])),
            "assistant" => {
                let blocks = match message.get(RAW_CONTENT).and_then(Value::as_array) {
                    Some(raw) => raw.clone(),
                    None => assistant_blocks(message),
                };
                push_turn(&mut turns, "assistant", blocks);
            }
            "tool" => push_turn(
                &mut turns,
                "user",
                vec![json!({
                    "type": "tool_result",
                    "tool_use_id": message["tool_call_id"],
                    "content": plain_text(&message["content"]),
                })],
            ),
            _ => push_turn(&mut turns, "user", text_blocks(&message["content"])),
        }
    }
    let mut body = json!({ "model": model, "max_tokens": MAX_TOKENS, "messages": turns });
    let system = system.join("\n\n");
    if !system.trim().is_empty() {
        body["system"] = json!(system);
    }
    if let Some(tools) = tools.and_then(Value::as_array) {
        body["tools"] = tools
            .iter()
            .map(|tool| {
                let function = &tool["function"];
                let schema = if function["parameters"].is_object() {
                    function["parameters"].clone()
                } else {
                    json!({ "type": "object", "properties": {} })
                };
                json!({
                    "name": function["name"],
                    "description": function["description"].as_str().unwrap_or_default(),
                    "input_schema": schema,
                })
            })
            .collect();
        if let Some(choice) = tool_choice {
            body["tool_choice"] = native_tool_choice(choice);
        }
    }
    if let Some(effort) = effort {
        // `none` and `minimal` (OpenAI's words) have no level here: the lowest is `low`.
        let level = match effort {
            "none" | "minimal" => "low",
            other => other,
        };
        body["output_config"]["effort"] = json!(level);
    }
    if let Some(format) = response_format {
        if format["type"] == "json_schema" {
            body["output_config"]["format"] =
                json!({ "type": "json_schema", "schema": format["json_schema"]["schema"] });
        }
    }
    body
}

/// Whether the request forces a tool (`tool_choice` `any` or a named tool), which Claude Opus 5.5
/// and Sonnet 5.5 refuse with HTTP 400 when thinking is on.
pub fn forces_tool(body: &Value) -> bool {
    matches!(body["tool_choice"]["type"].as_str(), Some("any" | "tool"))
}

/// The same request leaving the choice of the tool to the model.
pub fn without_forced_tool(mut body: Value) -> Value {
    body["tool_choice"] = json!({ "type": "auto" });
    body
}

/// A Messages answer as an OpenAI chat completion: `choices[0].message` with its text, its
/// `tool_calls` and the raw blocks ([`RAW_CONTENT`]), and `usage` in OpenAI's fields.
pub fn to_openai(data: &Value) -> Value {
    let blocks = data["content"].as_array().cloned().unwrap_or_default();
    let text: String = blocks
        .iter()
        .filter(|block| block["type"] == "text")
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    let tool_calls: Vec<Value> = blocks
        .iter()
        .filter(|block| block["type"] == "tool_use")
        .map(|block| {
            json!({
                "id": block["id"],
                "type": "function",
                "function": { "name": block["name"], "arguments": block["input"].to_string() },
            })
        })
        .collect();
    let mut message = json!({
        "role": "assistant",
        "content": if text.is_empty() { Value::Null } else { json!(text) },
        RAW_CONTENT: blocks,
    });
    if !tool_calls.is_empty() {
        message["tool_calls"] = json!(tool_calls);
    }
    let usage = &data["usage"];
    let count = |key: &str| usage[key].as_u64().unwrap_or(0);
    let cached = count("cache_read_input_tokens");
    json!({
        "model": data["model"],
        "choices": [{ "index": 0, "message": message, "finish_reason": data["stop_reason"] }],
        "usage": {
            "prompt_tokens": count("input_tokens") + count("cache_creation_input_tokens") + cached,
            "completion_tokens": count("output_tokens"),
            "prompt_tokens_details": { "cached_tokens": cached },
        },
    })
}

/// Appends blocks to the last turn when it has the same role (tool results following each other
/// form one user turn), otherwise starts a turn. Empty texts are dropped: the API refuses them.
fn push_turn(turns: &mut Vec<Value>, role: &str, blocks: Vec<Value>) {
    let blocks: Vec<Value> = blocks
        .into_iter()
        .filter(|b| {
            !(b["type"] == "text" && b["text"].as_str().is_none_or(|t| t.trim().is_empty()))
        })
        .collect();
    if blocks.is_empty() {
        return;
    }
    if let Some(last) = turns.last_mut() {
        if last["role"] == role {
            if let Some(content) = last["content"].as_array_mut() {
                content.extend(blocks);
                return;
            }
        }
    }
    turns.push(json!({ "role": role, "content": blocks }));
}

/// The blocks of an assistant message written in OpenAI's shape: its text, then its tool calls.
fn assistant_blocks(message: &Value) -> Vec<Value> {
    let mut blocks = text_blocks(&message["content"]);
    for call in message["tool_calls"].as_array().into_iter().flatten() {
        let input = call["function"]["arguments"]
            .as_str()
            .and_then(|a| serde_json::from_str::<Value>(a).ok())
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        blocks.push(json!({
            "type": "tool_use",
            "id": call["id"],
            "name": call["function"]["name"],
            "input": input,
        }));
    }
    blocks
}

fn text_blocks(content: &Value) -> Vec<Value> {
    vec![json!({ "type": "text", "text": plain_text(content) })]
}

/// The text of an OpenAI `content`: a string, or the text of its parts.
fn plain_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        _ => crate::providers::content_text(content),
    }
}

/// OpenAI's `tool_choice` in Anthropic's words.
fn native_tool_choice(choice: &Value) -> Value {
    match choice {
        Value::String(word) => match word.as_str() {
            "required" => json!({ "type": "any" }),
            "none" => json!({ "type": "none" }),
            _ => json!({ "type": "auto" }),
        },
        _ => match choice["function"]["name"].as_str() {
            Some(name) => json!({ "type": "tool", "name": name }),
            None => json!({ "type": "auto" }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_structured_call_becomes_a_messages_request() {
        let format = json!({ "type": "json_schema", "json_schema": { "name": "reply", "strict": true,
            "schema": { "type": "object", "properties": { "q": { "type": "string" } },
                        "required": ["q"], "additionalProperties": false } } });
        let body = request(
            "claude-haiku-5-5",
            &[
                json!({ "role": "system", "content": "Be brief." }),
                json!({ "role": "user", "content": "{\"latest\":\"hi\"}" }),
            ],
            None,
            None,
            Some("low"),
            Some(&format),
        );
        assert_eq!(body["system"], "Be brief.");
        assert_eq!(
            body["messages"],
            json!([{ "role": "user", "content": [{ "type": "text", "text": "{\"latest\":\"hi\"}" }] }])
        );
        assert_eq!(body["output_config"]["effort"], "low");
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert_eq!(
            body["output_config"]["format"]["schema"]["required"],
            json!(["q"])
        );
        assert!(body["max_tokens"].as_u64().unwrap() > 0);
        assert!(body.get("tools").is_none() && body.get("response_format").is_none());
    }

    #[test]
    fn a_tool_loop_keeps_the_thinking_blocks_and_groups_the_results() {
        // An answer of Claude: thinking, then two tool calls.
        let answer = json!({ "model": "claude-haiku-5-5", "stop_reason": "tool_use",
            "content": [
                { "type": "thinking", "thinking": "…", "signature": "sig" },
                { "type": "tool_use", "id": "t1", "name": "search_cases", "input": { "query": "oak" } },
                { "type": "tool_use", "id": "t2", "name": "read_case", "input": { "case": 3 } }
            ],
            "usage": { "input_tokens": 100, "output_tokens": 40, "cache_read_input_tokens": 20 } });
        let completion = to_openai(&answer);
        let message = &completion["choices"][0]["message"];
        assert!(message["content"].is_null());
        assert_eq!(message["tool_calls"][0]["function"]["name"], "search_cases");
        assert_eq!(
            message["tool_calls"][0]["function"]["arguments"],
            "{\"query\":\"oak\"}"
        );
        assert_eq!(completion["usage"]["prompt_tokens"], 120);
        assert_eq!(completion["usage"]["completion_tokens"], 40);

        // The loop sends it back with both results, as `ask::run_agent` builds them.
        let tools = json!([{ "type": "function", "function": { "name": "answer",
            "description": "Final answer", "parameters": { "type": "object", "properties": {} } } }]);
        let choice = json!({ "type": "function", "function": { "name": "answer" } });
        let body = request(
            "claude-haiku-5-5",
            &[
                json!({ "role": "system", "content": "S" }),
                json!({ "role": "user", "content": "Q" }),
                message.clone(),
                json!({ "role": "tool", "tool_call_id": "t1", "content": "[1]" }),
                json!({ "role": "tool", "tool_call_id": "t2", "content": "[2]" }),
            ],
            Some(&tools),
            Some(&choice),
            None,
            None,
        );
        let turns = body["messages"].as_array().unwrap();
        assert_eq!(turns.len(), 3, "user, assistant, one user turn of results");
        assert_eq!(
            turns[1]["content"][0]["signature"], "sig",
            "thinking unchanged"
        );
        assert_eq!(turns[2]["content"][1]["tool_use_id"], "t2");
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
        assert_eq!(
            body["tool_choice"],
            json!({ "type": "tool", "name": "answer" })
        );
        assert!(forces_tool(&body));
        assert_eq!(without_forced_tool(body)["tool_choice"]["type"], "auto");
    }

    #[test]
    fn an_assistant_turn_of_another_provider_is_rebuilt() {
        let body = request(
            "claude-haiku-5-5",
            &[
                json!({ "role": "user", "content": "Q" }),
                json!({ "role": "assistant", "content": "", "tool_calls": [{ "id": "c1",
                    "type": "function", "function": { "name": "search_cases", "arguments": "{\"query\":\"x\"}" } }] }),
                json!({ "role": "tool", "tool_call_id": "c1", "content": "[]" }),
                json!({ "role": "user", "content": "Reply by calling the function `answer`." }),
            ],
            None,
            Some(&json!("auto")),
            Some("none"),
            None,
        );
        let turns = body["messages"].as_array().unwrap();
        assert_eq!(
            turns[1]["content"],
            json!([{ "type": "tool_use", "id": "c1",
            "name": "search_cases", "input": { "query": "x" } }]),
            "no empty text block"
        );
        assert_eq!(
            turns[2]["content"].as_array().unwrap().len(),
            2,
            "result and the reminder"
        );
        assert_eq!(body["output_config"]["effort"], "low");
        assert!(body.get("tool_choice").is_none(), "no tools, no choice");
    }

    #[test]
    fn the_text_of_a_structured_answer_is_the_content() {
        let completion = to_openai(&json!({ "content": [
            { "type": "thinking", "thinking": "", "signature": "s" },
            { "type": "text", "text": "{\"q\":\"Why?\"}" }
        ], "usage": { "input_tokens": 5, "output_tokens": 7 } }));
        assert_eq!(
            crate::providers::extract_content(&completion).unwrap(),
            "{\"q\":\"Why?\"}"
        );
    }
}
