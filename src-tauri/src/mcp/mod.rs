pub mod definitions;
pub mod prompts;
pub mod protocol;
pub mod tools;

pub use definitions::tool_definitions;
use protocol::{JsonRpcRequest, JsonRpcResponse};
use rusqlite::Connection;
use serde_json::json;
use std::io::{self, BufRead, Write};

/// Instructions sent to clients in `initialize.instructions`: they apply to any MCP client
/// (Claude Code, Codex, opencode...), with or without the Humcase plugin. Text for agents, hence
/// in English. Kept well under 2,048 characters, where Claude Code cuts server instructions: the
/// shape of a note lives in the `add_note` description, the details in the plugin's skills.
pub const SERVER_INSTRUCTIONS: &str = "Humcase is the person's local workspace of cases (called notes in tool names), shared with agents: the source of truth for tracking work.

Start: call get_handoff (with project_id when known). Before creating a case, look for an existing one, including the cases the person dictated (find_similar_notes, or list_notes with search_query), and continue in it. No project fits: get_or_create_inbox.

Filing: what belongs to one subject goes inside its case. Pass that case's id as parent_id to add_note to create a sub-case (a task, question, incident...); a top-level case only when none fits. Links (create_linked_task, link_note_task) only cross-reference different subjects. Propose moves (move_case) and archiving (archive_case) to the person before tidying.

Status: set in_progress when you start. Set resolved only after verifying the result, with a resolution_comment saying what was done, how it was verified and what remains. resolved declares a result: only the person accepts it or asks for a rework. When you resolve the last open sub-case, resolve its parent too if its goal is reached, or report what remains in it. Never report progress you did not observe, nor a status change that failed.

Rework: the person asked for the work to be redone. Read their reason (get_note review.last_decision.comment; also in get_handoff and get_brief), fix it, then resolve again with a report of what changed (append: true keeps the previous report above it).

Editing: update_note and structure_note need the person's authorization in Humcase settings and expected_revision from get_note; raw_content is never editable. Otherwise add a report or a sub-case.";

/// Protocol versions the server speaks, newest first.
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 4] =
    ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Runs one tool for the CLI through the same code as `tools/call`, with the same safeguards,
/// and returns its text content. A tool error is returned as `Err` with the tool's message.
pub fn call_tool_text(
    conn: &Connection,
    tool_name: &str,
    arguments: Option<serde_json::Value>,
) -> Result<String, String> {
    let known = tool_definitions()
        .as_array()
        .is_some_and(|tools| tools.iter().any(|t| t["name"] == tool_name));
    if !known {
        return Err(format!(
            "Unknown tool '{tool_name}'. Run `humcase tools` to list them."
        ));
    }
    let result = tools::handle_tool_call(conn, tool_name, arguments)?;
    let text = result["content"]
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join(
                    "
",
                )
        })
        .unwrap_or_default();
    Ok(text)
}

/// Handles **one line** of JSON-RPC and returns the response to write, or `None` if there is nothing
/// to answer (notification without `id`).
///
/// Pure function with respect to I/O: it makes the framing testable (JSON-RPC codes, version
/// negotiation, notifications, tool errors) without launching the server or a real `stdin`. The empty
/// line is filtered by the caller.
pub fn handle_line(conn: &Connection, line: &str) -> Option<JsonRpcResponse> {
    let req: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        // Parse error: JSON-RPC requires -32700 with a null `id`.
        Err(_) => return Some(JsonRpcResponse::error(json!(null), -32700, "Parse error")),
    };

    // Ignore notifications (requests without an ID): they expect no response.
    req.id.as_ref()?;

    let id = req.id.clone().unwrap_or(json!(null));

    let res = match req.method.as_str() {
        "ping" => JsonRpcResponse::success(id, json!({})),
        "initialize" => {
            // The MCP protocol requires answering with a version the server supports: a known
            // requested version is echoed back, otherwise the newest one is offered.
            let requested = req
                .params
                .as_ref()
                .and_then(|p| p.get("protocolVersion"))
                .and_then(|v| v.as_str());
            let protocol_version = match requested {
                Some(v) if SUPPORTED_PROTOCOL_VERSIONS.contains(&v) => v,
                _ => SUPPORTED_PROTOCOL_VERSIONS[0],
            };
            // The journal of who did what names the agent after its client (Claude Code,
            // Codex...). A connection without the journal (tests) is left as it is.
            let client = req
                .params
                .as_ref()
                .and_then(|p| p.pointer("/clientInfo/name"))
                .and_then(|v| v.as_str());
            if let Some(client) = client {
                let actor = crate::db::journal::Actor::Agent(Some(
                    crate::db::journal::agent_display_name(client),
                ));
                if let Err(e) = crate::db::journal::set_actor(conn, &actor) {
                    eprintln!("humcase: agent name not recorded: {e}");
                }
            }
            // Settings → Agents and Home's first steps tell an agent is connected from this,
            // reads included. A connection without a database file (tests) records nothing.
            if let Some(db_path) = conn.path().filter(|path| !path.is_empty()) {
                let name = client
                    .map_or_else(|| "MCP".to_string(), crate::db::journal::agent_display_name);
                crate::agents_seen::record(std::path::Path::new(db_path), &name);
            }
            JsonRpcResponse::success(
                id,
                json!({
                    "protocolVersion": protocol_version,
                    "serverInfo": { "name": "Humcase", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": SERVER_INSTRUCTIONS,
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "prompts": { "listChanged": false }
                    }
                }),
            )
        }
        "tools/list" => JsonRpcResponse::success(id, json!({ "tools": tool_definitions() })),
        "prompts/list" => {
            JsonRpcResponse::success(id, json!({ "prompts": prompts::prompt_definitions() }))
        }
        "prompts/get" => {
            let params = req.params.unwrap_or(json!({}));
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            match prompts::get_prompt(name, params.get("arguments")) {
                Ok(result) => JsonRpcResponse::success(id, result),
                // An unknown prompt or a bad argument is an invalid request parameter.
                Err(e) => JsonRpcResponse::error(id, -32602, &e),
            }
        }
        "tools/call" => {
            let params = req.params.unwrap_or(json!({}));
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned();

            match tools::handle_tool_call(conn, tool_name, arguments) {
                Ok(result) => JsonRpcResponse::success(id, result),
                // A tool error is a JSON-RPC **result** (not a protocol error):
                // it must be flagged with `isError: true` so the agent sees it.
                Err(e) => JsonRpcResponse::success(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": e }],
                        "isError": true
                    }),
                ),
            }
        }
        _ => JsonRpcResponse::error(id, -32601, "Method not found"),
    };

    Some(res)
}

pub fn run_mcp_server(conn: Connection) {
    serve(io::stdin().lock(), io::stdout(), |line| {
        handle_line(&conn, line)
    });
}

/// Protocol loop: one request per line, until the end of the input. A line that is not UTF-8
/// gets a parse error and a handler that panics an internal error, for that request only: the
/// session goes on, the agent keeps its tools.
fn serve(
    mut input: impl BufRead,
    mut output: impl Write,
    handle: impl Fn(&str) -> Option<JsonRpcResponse>,
) {
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match input.read_until(b'\n', &mut buffer) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                eprintln!("humcase: MCP input closed: {e}");
                break;
            }
        }
        let response = match std::str::from_utf8(&buffer) {
            Err(_) => Some(JsonRpcResponse::error(json!(null), -32700, "Parse error")),
            Ok(line) if line.trim().is_empty() => None,
            Ok(line) => {
                let line = line.trim_end_matches(['\r', '\n']);
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle(line)))
                    .unwrap_or_else(|_| {
                        eprintln!("humcase: MCP request failed with an internal error");
                        let id = serde_json::from_str::<serde_json::Value>(line)
                            .ok()
                            .and_then(|request| request.get("id").cloned());
                        // A notification (no id) gets no response, even on failure.
                        id.map(|id| JsonRpcResponse::error(id, -32603, "Internal error"))
                    })
            }
        };
        if let Some(response) = response {
            if let Ok(text) = serde_json::to_string(&response) {
                let _ = writeln!(output, "{text}");
                let _ = output.flush();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        schema::apply_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn the_session_survives_a_line_that_is_not_utf8_and_a_failing_request() {
        let conn = setup_db();
        let mut input = Vec::new();
        input.extend_from_slice(b"\xff\xfe not text\n");
        input.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"boom\"}\n");
        input.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"ping\"}\r\n");
        let mut output = Vec::new();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        serve(&input[..], &mut output, |line| {
            if line.contains("boom") {
                panic!("handler failure");
            }
            handle_line(&conn, line)
        });
        std::panic::set_hook(previous);
        let replies: Vec<serde_json::Value> = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(replies.len(), 3);
        assert_eq!(replies[0]["error"]["code"], -32700);
        assert_eq!(replies[1]["id"], 7);
        assert_eq!(replies[1]["error"]["code"], -32603);
        assert_eq!(replies[2]["id"], 8);
        assert!(replies[2].get("result").is_some(), "the session goes on");
    }

    /// Audit 4 #10 — an unreadable line must produce exactly -32700, with a null `id`.
    #[test]
    fn test_parse_error_returns_32700_with_null_id() {
        let conn = setup_db();
        let res = handle_line(&conn, "{ this is not json")
            .expect("a parse error must produce a response");
        assert_eq!(res.id, json!(null));
        let err = res.error.expect("a -32700 error is expected");
        assert_eq!(err.code, -32700);
        assert_eq!(err.message, "Parse error");
    }

    /// v24 — the journal names the agent after its MCP client: a case it creates is attributed
    /// to "Claude Code", not to the person, and `get_note` tells the agent where the decision on
    /// its result stands.
    #[test]
    fn test_initialize_names_the_agent_in_the_journal() {
        let conn = setup_db();
        crate::db::journal::install(&conn, &crate::db::journal::Actor::Agent(None)).unwrap();
        handle_line(
            &conn,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{"name":"claude-code","version":"2"}}}"#,
        )
        .unwrap();
        let project = crate::db::queries::create_project(&conn, "Journal", None).unwrap();
        let created = tools::handle_tool_call(
            &conn,
            "add_note",
            Some(json!({ "project_id": project, "title": "Research", "note_type": "research" })),
        )
        .unwrap();
        let id = created["structuredContent"]["id"].as_str().unwrap();
        let events = crate::db::journal::case_events(&conn, id).unwrap();
        assert_eq!(events[0].actor_kind, "agent");
        assert_eq!(events[0].actor_name.as_deref(), Some("Claude Code"));
        let read =
            tools::handle_tool_call(&conn, "get_note", Some(json!({ "note_id": id }))).unwrap();
        let note: serde_json::Value =
            serde_json::from_str(read["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(note["review"]["state"], "none");
        assert_eq!(note["recent_activity"][0]["actor"], "Claude Code");
    }

    /// Audit 4 #10 — `initialize` returns a supported version: echo if it is known, fallback otherwise.
    #[test]
    fn test_initialize_negotiates_version() {
        let conn = setup_db();

        let known = handle_line(
            &conn,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#,
        )
        .unwrap();
        assert_eq!(known.id, json!(1));
        assert_eq!(known.result.unwrap()["protocolVersion"], "2025-03-26");

        let unknown = handle_line(
            &conn,
            r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#,
        )
        .unwrap();
        // An unknown version gets the newest one the server speaks.
        assert_eq!(unknown.result.unwrap()["protocolVersion"], "2025-11-25");

        let newest = handle_line(
            &conn,
            r#"{"jsonrpc":"2.0","id":3,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
        )
        .unwrap();
        assert_eq!(newest.result.unwrap()["protocolVersion"], "2025-11-25");
    }

    /// Humcase plugin (0.9.71) — the server presents itself under the name Humcase and sends its writing
    /// instructions to any client, so agents write structured and linked notes.
    #[test]
    fn test_initialize_announces_humcase_and_instructions() {
        let conn = setup_db();
        let res = handle_line(
            &conn,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
        )
        .unwrap();
        let result = res.result.unwrap();
        assert_eq!(result["serverInfo"]["name"], "Humcase");
        let instructions = result["instructions"]
            .as_str()
            .expect("instructions missing");
        for expected in [
            "get_handoff",
            "parent_id",
            "create_linked_task",
            "in_progress",
            "resolution_comment",
            "rework",
            "expected_revision",
            "raw_content",
        ] {
            assert!(
                instructions.contains(expected),
                "instruction missing: {expected}"
            );
        }
        // Every tool cited in the instructions actually exists: any snake_case word that is not
        // a known argument or value must be a tool.
        let list = handle_line(&conn, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        let tools = list.result.unwrap()["tools"].as_array().unwrap().clone();
        let names: Vec<String> = tools
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        let not_tools = [
            "project_id",
            "search_query",
            "parent_id",
            "in_progress",
            "resolution_comment",
            "last_decision",
            "expected_revision",
            "raw_content",
        ];
        let cited: Vec<&str> = instructions
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|w| w.contains('_') && !not_tools.contains(w))
            .collect();
        assert!(cited.len() >= 10, "tools cited: {cited:?}");
        for cited in cited {
            assert!(
                names.iter().any(|n| n == cited),
                "tool cited but missing: {cited}"
            );
        }
    }

    /// Claude Code cuts server instructions at 2,048 characters: past that, the rules on status,
    /// reworks and editing would never reach agents.
    #[test]
    fn server_instructions_fit_in_what_clients_read() {
        assert!(
            SERVER_INSTRUCTIONS.chars().count() < 2000,
            "instructions too long: {} characters",
            SERVER_INSTRUCTIONS.chars().count()
        );
        // The last rule (editing) is what a cut would lose first.
        assert!(SERVER_INSTRUCTIONS.contains("raw_content is never editable"));
    }

    /// Every tool says what it does to the person's data, with a title and the four hints, and
    /// describes every parameter; a tool without parameters refuses extra ones.
    #[test]
    fn every_tool_has_a_title_annotations_and_described_parameters() {
        let definitions = tool_definitions();
        for tool in definitions.as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            assert!(tool["title"].is_string(), "{name}: title missing");
            let hints = &tool["annotations"];
            for hint in [
                "readOnlyHint",
                "destructiveHint",
                "idempotentHint",
                "openWorldHint",
            ] {
                assert!(hints[hint].is_boolean(), "{name}: {hint} missing");
            }
            let read_only = hints["readOnlyHint"].as_bool().unwrap();
            assert_eq!(
                read_only,
                name.starts_with("list_")
                    || name.starts_with("get_") && name != "get_or_create_inbox"
                    || name.starts_with("find_")
                    || name == "export_json_canvas",
                "{name}: readOnlyHint"
            );
            let properties = tool["inputSchema"]["properties"].as_object().unwrap();
            if properties.is_empty() {
                assert_eq!(
                    tool["inputSchema"]["additionalProperties"],
                    json!(false),
                    "{name}: a tool without parameters refuses extra ones"
                );
            }
            for (parameter, schema) in properties {
                assert!(
                    schema["description"].is_string(),
                    "{name}.{parameter}: description missing"
                );
            }
        }
        let hint = |name: &str, hint: &str| {
            definitions
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["name"] == name)
                .unwrap()["annotations"][hint]
                .as_bool()
                .unwrap()
        };
        for destructive in [
            "delete_activity",
            "set_project_activities",
            "unlink_note_task",
            "update_note",
            "structure_note",
        ] {
            assert!(hint(destructive, "destructiveHint"), "{destructive}");
        }
        assert!(hint("structure_note", "openWorldHint"));
        assert!(!hint("add_note", "openWorldHint"));
        for idempotent in ["set_due_date", "set_case_position", "move_case"] {
            assert!(hint(idempotent, "idempotentHint"), "{idempotent}");
        }
    }

    /// Deliberate exclusions: an agent never decides on its own result, marks the brief as
    /// seen, deletes or purges, restores an archive, reads AI keys or deletes a project.
    #[test]
    fn excluded_capabilities_stay_out_of_the_tool_list() {
        let definitions = tool_definitions();
        let names: Vec<&str> = definitions
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        for name in &names {
            for forbidden in [
                "review",
                "seen",
                "delete_note",
                "trash",
                "purge",
                "restore",
                "backup",
                "key",
                "setting",
                "delete_project",
            ] {
                assert!(
                    !name.contains(forbidden),
                    "{name} looks like an excluded capability ({forbidden})"
                );
            }
        }
    }

    /// The prompts capability is declared and its prompts can be read; an unknown prompt is an
    /// invalid parameter.
    #[test]
    fn prompts_are_listed_and_built() {
        let conn = setup_db();
        let init = handle_line(
            &conn,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
        )
        .unwrap();
        assert!(init.result.unwrap()["capabilities"]["prompts"].is_object());
        let list = handle_line(&conn, r#"{"jsonrpc":"2.0","id":2,"method":"prompts/list"}"#)
            .unwrap()
            .result
            .unwrap();
        let names: Vec<&str> = list["prompts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["resume-work", "write-note"]);
        let prompt = handle_line(
            &conn,
            r#"{"jsonrpc":"2.0","id":3,"method":"prompts/get","params":{"name":"resume-work","arguments":{"project_id":"p1"}}}"#,
        )
        .unwrap()
        .result
        .unwrap();
        let text = prompt["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(
            text.contains("get_handoff with project_id \"p1\""),
            "{text}"
        );
        assert!(text.contains("get_brief"), "{text}");
        let unknown = handle_line(
            &conn,
            r#"{"jsonrpc":"2.0","id":4,"method":"prompts/get","params":{"name":"nope"}}"#,
        )
        .unwrap();
        assert_eq!(unknown.error.unwrap().code, -32602);
    }

    /// Audit 4 #10 — `tools/list` exposes the tools expected by agents.
    #[test]
    fn test_tools_list_exposes_tools() {
        let conn = setup_db();
        let res = handle_line(&conn, r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#).unwrap();
        let tools = res.result.unwrap()["tools"].as_array().unwrap().clone();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for expected in [
            "list_projects",
            "list_notes",
            "get_note",
            "add_note",
            "update_note_status",
            "update_note",
            "get_handoff",
            "get_note_relations",
            "create_linked_task",
            "link_note_task",
            "unlink_note_task",
        ] {
            assert!(
                names.contains(&expected),
                "tool missing: {expected} (present: {names:?})"
            );
        }
        // Safeguard: deletion is never exposed to an agent.
        assert!(
            !names.contains(&"delete_note"),
            "delete_note must not be exposed to agents: {names:?}"
        );
    }

    /// Audit 4 #10 — a notification (without `id`) must produce no response.
    #[test]
    fn test_notification_without_id_ignored() {
        let conn = setup_db();
        assert!(handle_line(
            &conn,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
        )
        .is_none());
        assert!(handle_line(&conn, r#"{"jsonrpc":"2.0","method":"tools/list"}"#).is_none());
    }

    /// Audit 4 #10 — a tool error is an `isError: true` **result**, not a JSON-RPC error.
    #[test]
    fn test_tool_error_becomes_iserror() {
        let conn = setup_db();
        let res = handle_line(
            &conn,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"nonexistent_tool","arguments":{}}}"#,
        )
        .unwrap();
        assert!(
            res.error.is_none(),
            "a tool error must not be a protocol error"
        );
        let result = res.result.unwrap();
        assert_eq!(result["isError"], json!(true));
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not found"));
    }

    /// Audit 4 #10 — an unknown method stays a -32601 protocol error.
    #[test]
    fn test_unknown_method_returns_32601() {
        let conn = setup_db();
        let res = handle_line(&conn, r#"{"jsonrpc":"2.0","id":5,"method":"nonexistent"}"#).unwrap();
        assert_eq!(res.error.unwrap().code, -32601);
    }

    /// CLI parity: every tool listed by `tools/list` is dispatched by the shared handler,
    /// so `humcase call` can run it. Fails if a definition has no implementation.
    #[test]
    fn every_listed_tool_is_callable() {
        let conn = setup_db();
        let definitions = tool_definitions();
        let tools = definitions.as_array().unwrap();
        assert!(tools.len() >= 22, "unexpected tool count: {}", tools.len());
        for tool in tools {
            let name = tool["name"].as_str().unwrap();
            if let Err(e) = tools::handle_tool_call(&conn, name, Some(json!({}))) {
                assert!(
                    !e.contains("not found"),
                    "tool {name} is listed but not dispatched: {e}"
                );
            }
        }
    }

    #[test]
    fn cli_call_rejects_unknown_tool() {
        let conn = setup_db();
        let error = call_tool_text(&conn, "delete_note", None).unwrap_err();
        assert!(error.contains("Unknown tool"), "{error}");
    }

    /// The CLI goes through the same safeguards: without the human permission,
    /// update_note is refused and the note is unchanged.
    #[test]
    fn cli_call_keeps_update_note_permission_guard() {
        let conn = setup_db();
        let project: serde_json::Value = serde_json::from_str(
            &call_tool_text(&conn, "create_project", Some(json!({ "name": "CLI" }))).unwrap(),
        )
        .unwrap();
        let note: serde_json::Value = serde_json::from_str(
            &call_tool_text(
                &conn,
                "add_note",
                Some(
                    json!({ "project_id": project["id"], "title": "Before", "note_type": "note" }),
                ),
            )
            .unwrap(),
        )
        .unwrap();
        let note_id = note["id"].as_str().unwrap().to_string();
        let error = call_tool_text(
            &conn,
            "update_note",
            Some(json!({ "note_id": note_id, "expected_revision": 0, "title": "After" })),
        )
        .unwrap_err();
        assert!(error.contains("disabled"), "{error}");
        let note = crate::db::queries::get_note_by_id(&conn, &note_id).unwrap();
        assert_eq!(note.title, "Before");
    }

    #[test]
    fn cli_call_returns_tool_text() {
        let conn = setup_db();
        call_tool_text(&conn, "create_project", Some(json!({ "name": "Listed" }))).unwrap();
        let text = call_tool_text(&conn, "list_projects", None).unwrap();
        let projects: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(projects
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == "Listed"));
    }
}
