#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::{Parser, Subcommand};
use humcase_lib::{
    db, mcp,
    models::note::{CreateNoteInput, NoteStatus, NoteType, UpdateNoteInput},
    process_lease::ProcessLease,
};

/// Exit codes, printed at the end of `humcase --help`.
const EXIT_CODES: &str = "Exit codes:
  0  success
  1  error: invalid value, tool refusal, case or project not found, database unavailable
     (the message is printed on stderr)
  2  invalid command line (unknown command or option, missing argument)";

#[derive(Parser)]
#[command(
    name = "humcase",
    version,
    about = "Humcase: CLI and MCP server",
    after_help = EXIT_CODES
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP server
    Mcp,
    /// Add a new note quickly
    Add {
        /// Note title
        title: String,
        /// Optional project ID (defaults to the first project if not provided)
        #[arg(short, long)]
        project_id: Option<String>,
        /// Case nature (note, idea, todo, question, research, decision, bug)
        #[arg(short = 't', long, default_value = "note")]
        note_type: String,
        /// Priority (P0, P1, P2, P3)
        #[arg(long)]
        priority: Option<String>,
    },
    /// List cases, one page at a time (MCP list_notes); archived cases are left out
    List {
        /// Only cases with this status (open, in_progress, resolved)
        #[arg(short, long)]
        status: Option<String>,
        /// Only cases of this project
        #[arg(short, long)]
        project_id: Option<String>,
        /// Full-text search; a word starting with # filters on that tag
        #[arg(long)]
        search: Option<String>,
        /// Most cases listed (1 to 200)
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// Print the page as JSON (the list_notes result)
        #[arg(long)]
        json: bool,
    },
    /// Read one complete case as JSON, with its path, decision and recent activity (MCP get_note)
    Note { note_id: String },
    /// Read incoming sources and outgoing tasks for one note as JSON
    Relations { note_id: String },
    /// Create and link a task in the source note's project
    CreateLinkedTask {
        source_note_id: String,
        title: String,
        #[arg(long)]
        content: Option<String>,
    },
    /// Link an existing todo to a source note
    LinkTask {
        source_note_id: String,
        task_note_id: String,
    },
    /// Remove a note-to-task link
    UnlinkTask {
        source_note_id: String,
        task_note_id: String,
    },
    /// Change note status; resolving requires --resolution-comment. Pass
    /// --expected-resolution-comment with the report read beforehand to refuse the write if it changed
    Status {
        note_id: String,
        status: String,
        #[arg(long)]
        resolution_comment: Option<String>,
        /// Refuse the write if the stored report differs from this value
        #[arg(long)]
        expected_resolution_comment: Option<String>,
    },
    /// Record a human or agent result in resolution_comment without changing status
    Report { note_id: String, text: String },
    /// List the tools also exposed over MCP; `humcase call` runs any of them
    Tools {
        /// Print the full definitions (descriptions and input schemas) as JSON
        #[arg(long)]
        json: bool,
    },
    /// Run any MCP tool with the same safeguards as the MCP server, e.g.
    /// `humcase call list_notes --args '{"status":"open"}'`
    Call {
        tool: String,
        /// Tool arguments as a JSON object; `-` reads them from stdin
        #[arg(long)]
        args: Option<String>,
    },
    /// Reload working context: open work, due dates, recent decisions (MCP get_handoff)
    Handoff {
        #[arg(long)]
        project_id: Option<String>,
    },
    /// List projects as JSON (MCP list_projects)
    Projects,
    /// Run by the hooks of an agent client (Claude Code, Codex) with its JSON on stdin:
    /// reminders to track the work in Humcase. Always exits 0; prints nothing on failure
    Hook {
        #[arg(value_enum)]
        event: HookEventArg,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum HookEventArg {
    SessionStart,
    PostTool,
    Stop,
}

fn print_json<T: serde::Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(json) => println!("{json}"),
        Err(e) => fail(format!("JSON output failed: {e}")),
    }
}

fn fail(message: String) -> ! {
    eprintln!("Error: {message}");
    std::process::exit(1);
}

/// Parses `--args`: a JSON object, `-` for stdin, or nothing.
fn parse_call_args(raw: Option<&str>) -> Option<serde_json::Value> {
    let raw = match raw {
        None => return None,
        Some("-") => {
            let mut buffer = String::new();
            if let Err(e) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer) {
                fail(format!("cannot read arguments from stdin: {e}"));
            }
            buffer
        }
        Some(text) => text.to_string(),
    };
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(value) if value.is_object() => Some(value),
        Ok(_) => fail("--args must be a JSON object".to_string()),
        Err(e) => fail(format!("--args is not valid JSON: {e}")),
    }
}

/// Runs one MCP tool and prints its text result; a tool error exits with code 1.
fn run_tool(conn: &rusqlite::Connection, tool: &str, arguments: Option<serde_json::Value>) {
    match mcp::call_tool_text(conn, tool, arguments) {
        Ok(text) => println!("{text}"),
        Err(e) => fail(e),
    }
}

fn empty_update() -> UpdateNoteInput {
    UpdateNoteInput {
        status: None,
        content: None,
        resolution_comment: None,
        title: None,
        note_type: None,
        raw_content: None,
        priority: None,
        git_branch: None,
        tags: None,
        due_at: None,
    }
}

/// A release build is a Windows GUI program: started from a terminal, it has no console and the
/// CLI would print nothing. With arguments, it takes the console of the terminal that started
/// it, unless its output already goes somewhere (pipe or file: agents, MCP clients, redirections).
#[cfg(windows)]
#[cfg_attr(debug_assertions, allow(dead_code))]
fn attach_parent_console() {
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
        fn GetStdHandle(handle: u32) -> *mut std::ffi::c_void;
    }
    // SAFETY: two kernel32 calls without pointers to our memory; a failure (no parent console)
    // leaves the process as it was.
    unsafe {
        let output = GetStdHandle(STD_OUTPUT_HANDLE);
        if output.is_null() || output as isize == -1 {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

fn main() {
    #[cfg(all(windows, not(debug_assertions)))]
    if std::env::args_os().len() > 1 {
        attach_parent_console();
    }
    let cli = Cli::parse();
    // The CLI and the MCP server speak English (protocol and agents); the UI sets its own language.
    if cli.command.is_some() {
        humcase_lib::i18n::set_language("en");
    }

    match cli.command {
        Some(Commands::Tools { json }) => {
            let definitions = mcp::tool_definitions();
            if json {
                print_json(&definitions);
            } else {
                for tool in definitions.as_array().into_iter().flatten() {
                    let description = tool["description"].as_str().unwrap_or("");
                    let summary = description.split(". ").next().unwrap_or(description);
                    println!("{:<24} {}", tool["name"].as_str().unwrap_or(""), summary);
                }
            }
        }
        Some(Commands::Call { tool, args }) => {
            let arguments = parse_call_args(args.as_deref());
            let (_lease, conn) = get_db_conn();
            run_tool(&conn, &tool, arguments);
        }
        Some(Commands::Handoff { project_id }) => {
            let (_lease, conn) = get_db_conn();
            let arguments = project_id.map(|id| serde_json::json!({ "project_id": id }));
            run_tool(&conn, "get_handoff", arguments);
        }
        Some(Commands::Projects) => {
            let (_lease, conn) = get_db_conn();
            run_tool(&conn, "list_projects", None);
        }
        Some(Commands::Hook { event }) => {
            use humcase_lib::agent_hooks::{self, HookEvent};
            let event = match event {
                HookEventArg::SessionStart => HookEvent::SessionStart,
                HookEventArg::PostTool => HookEvent::PostTool,
                HookEventArg::Stop => HookEvent::Stop,
            };
            let mut stdin = String::new();
            let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut stdin);
            // No data move here: a hook must stay quick and never block the agent's session.
            if let Ok(dir) = humcase_lib::data_location::data_dir() {
                if let Some(output) = agent_hooks::run(event, &stdin, &dir) {
                    use std::io::Write;
                    let _ = writeln!(std::io::stdout(), "{output}");
                }
            }
        }
        Some(Commands::Mcp) => {
            let (_lease, conn) = get_db_conn();
            if let Err(e) = db::journal::set_actor(&conn, &db::journal::Actor::Agent(None)) {
                fail(format!("unable to prepare the activity journal: {e}"));
            }
            mcp::run_mcp_server(conn);
        }
        Some(Commands::Add {
            title,
            project_id,
            note_type,
            priority,
        }) => {
            let (_lease, conn) = get_db_conn();

            // Destination: explicit, or the first **ordinary** project (technical Inboxes
            // `__humcase_inbox_…` are never an implicit destination). The project actually
            // used is read back so it can be displayed (space + name).
            let destination = match project_id {
                Some(id) => db::queries::get_projects(&conn)
                    .unwrap_or_else(|e| fail(format!("unable to read projects: {e}")))
                    .into_iter()
                    .find(|project| project.id == id)
                    .unwrap_or_else(|| fail(humcase_lib::tr!("app.cli.projectNotFound", id = id))),
                None => db::queries::default_project(&conn).unwrap_or_else(|e| fail(e)),
            };

            // Same priorities as the MCP `add_note` (P0-P3, empty = none).
            let priority = priority.filter(|p| !p.trim().is_empty()).map(|p| {
                let p = p.trim().to_uppercase();
                if ["P0", "P1", "P2", "P3"].contains(&p.as_str()) {
                    p
                } else {
                    fail(format!("invalid priority: {p} (expected: P0|P1|P2|P3)"))
                }
            });
            let n_type = NoteType::parse(&note_type.to_lowercase()).unwrap_or_else(|| {
                eprintln!(
                    "Error: Invalid note type '{}'. Expected {}.",
                    note_type,
                    NoteType::expected()
                );
                std::process::exit(1);
            });

            let input = CreateNoteInput {
                project_id: destination.id.clone(),
                title: title.clone(),
                content: None,
                note_type: n_type,
                priority,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: None,
            };

            match db::queries::create_note(&conn, input) {
                Ok(id) => {
                    let space = destination
                        .space
                        .map(|s| s.as_str().to_string())
                        .unwrap_or_else(|| humcase_lib::tr!("app.cli.noSpace"));
                    println!(
                        "{}",
                        humcase_lib::tr!(
                            "app.cli.noteCreated",
                            title = title,
                            project = destination.name,
                            space = space,
                            id = id
                        )
                    );
                }
                Err(e) => {
                    eprintln!("Error creating note: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::List {
            status,
            project_id,
            search,
            limit,
            json,
        }) => {
            let mut arguments = serde_json::json!({ "limit": limit });
            for (key, value) in [
                ("status", status),
                ("project_id", project_id),
                ("search_query", search),
            ] {
                if let Some(value) = value {
                    arguments[key] = serde_json::json!(value);
                }
            }
            let (_lease, conn) = get_db_conn();
            let text = mcp::call_tool_text(&conn, "list_notes", Some(arguments))
                .unwrap_or_else(|e| fail(e));
            if json {
                println!("{text}");
            } else {
                let page: serde_json::Value = serde_json::from_str(&text)
                    .unwrap_or_else(|e| fail(format!("unreadable list: {e}")));
                for n in page["notes"].as_array().into_iter().flatten() {
                    println!(
                        "[{}] {} - {}  ({})",
                        n["status"].as_str().unwrap_or(""),
                        n["note_type"].as_str().unwrap_or(""),
                        n["title"].as_str().unwrap_or(""),
                        n["id"].as_str().unwrap_or("")
                    );
                }
                if let Some(hint) = page["hint"].as_str() {
                    eprintln!("{hint}");
                }
            }
        }
        Some(Commands::Note { note_id }) => {
            let (_lease, conn) = get_db_conn();
            let text = mcp::call_tool_text(
                &conn,
                "get_note",
                Some(serde_json::json!({ "note_id": note_id })),
            )
            .unwrap_or_else(|e| fail(e));
            let note: serde_json::Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| fail(format!("unreadable case: {e}")));
            print_json(&note);
        }
        Some(Commands::Relations { note_id }) => {
            let (_lease, conn) = get_db_conn();
            let result =
                db::relations::get_note_relations(&conn, &note_id).unwrap_or_else(|e| fail(e));
            print_json(&result);
        }
        Some(Commands::CreateLinkedTask {
            source_note_id,
            title,
            content,
        }) => {
            let (_lease, conn) = get_db_conn();
            let id = db::relations::create_linked_task(
                &conn,
                &source_note_id,
                &title,
                content.as_deref(),
            )
            .unwrap_or_else(|e| fail(e));
            print_json(&serde_json::json!({ "task_note_id": id }));
        }
        Some(Commands::LinkTask {
            source_note_id,
            task_note_id,
        }) => {
            let (_lease, conn) = get_db_conn();
            db::relations::link_note_task(&conn, &source_note_id, &task_note_id)
                .unwrap_or_else(|e| fail(e));
            print_json(
                &serde_json::json!({ "source_note_id": source_note_id, "task_note_id": task_note_id, "linked": true }),
            );
        }
        Some(Commands::UnlinkTask {
            source_note_id,
            task_note_id,
        }) => {
            let (_lease, conn) = get_db_conn();
            db::relations::unlink_note_task(&conn, &source_note_id, &task_note_id)
                .unwrap_or_else(|e| fail(e));
            print_json(
                &serde_json::json!({ "source_note_id": source_note_id, "task_note_id": task_note_id, "linked": false }),
            );
        }
        Some(Commands::Status {
            note_id,
            status,
            resolution_comment,
            expected_resolution_comment,
        }) => {
            let status = match status.as_str() {
                "open" => NoteStatus::Open,
                "in_progress" => NoteStatus::InProgress,
                "resolved" => NoteStatus::Resolved,
                _ => fail(format!(
                    "invalid status: {status} (expected open|in_progress|resolved)"
                )),
            };
            if status == NoteStatus::Resolved
                && resolution_comment
                    .as_deref()
                    .is_none_or(|s| s.trim().is_empty())
            {
                fail("--resolution-comment is required when status is resolved".to_string());
            }
            let (_lease, conn) = get_db_conn();
            let mut update = empty_update();
            update.status = Some(status);
            update.resolution_comment = resolution_comment;
            if let Some(expected) = expected_resolution_comment {
                // Same guarantee as `report`: refuse an overwrite if the database changed since the
                // read. The comparison and the write share a single transaction.
                match db::queries::update_note_if_report_unchanged(
                    &conn,
                    &note_id,
                    update,
                    Some(&expected),
                ) {
                    Ok(()) => {}
                    // Tell errors apart: a nonexistent note_id must not be reported as a
                    // concurrent conflict (the queries message is generic).
                    Err(e) if e == humcase_lib::tr!("notes.notFound", id = note_id) => fail(e),
                    Err(e) if e.contains("changed") => fail(
                        "resolution_comment changed concurrently; read note and retry".to_string(),
                    ),
                    Err(e) => fail(e),
                }
            } else {
                db::queries::update_note(&conn, &note_id, update).unwrap_or_else(|e| fail(e));
            }
            let note = db::queries::get_note_by_id(&conn, &note_id)
                .unwrap_or_else(|e| fail(e.to_string()));
            print_json(&note);
        }
        Some(Commands::Report { note_id, text }) => {
            if text.trim().is_empty() {
                fail("report text cannot be empty".to_string());
            }
            let (_lease, conn) = get_db_conn();
            let current = db::queries::get_note_by_id(&conn, &note_id)
                .unwrap_or_else(|e| fail(e.to_string()));
            let mut update = empty_update();
            update.resolution_comment = Some(text);
            let base = serde_json::json!({ "resolution_comment": current.resolution_comment });
            let result = db::queries::retry_note_update(&conn, &note_id, update, base)
                .unwrap_or_else(|e| fail(e));
            if result == db::queries::RetryNoteUpdateResult::Conflict {
                fail("resolution_comment changed concurrently; read note and retry".to_string());
            }
            let note = db::queries::get_note_by_id(&conn, &note_id)
                .unwrap_or_else(|e| fail(e.to_string()));
            print_json(&note);
        }
        None => {
            // Normal UI mode
            humcase_lib::run();
        }
    }
}

fn get_db_conn() -> (ProcessLease, rusqlite::Connection) {
    // Data folder `~/.humcase`, moved out of AppData if needed (verified copy) before any
    // database is opened. Never fall back to the current directory: that would create a "ghost"
    // (empty) database that the user could no longer find from the interface.
    let app_dir = match humcase_lib::data_location::prepare_on_startup(
        humcase_lib::data_location::Process::Agent,
    ) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
    };
    let lease = match ProcessLease::acquire_shared(&app_dir) {
        Ok(lease) => lease,
        Err(e) => {
            eprintln!(
                "Error: unable to acquire the Humcase session lock at {}: {e}",
                app_dir.display()
            );
            std::process::exit(1);
        }
    };
    match db::init_db(&app_dir) {
        Ok(conn) => {
            // Journal of who did what (v24): the command line writes as the CLI, named by
            // `HUMCASE_ACTOR` when an agent's shell sets it; the MCP server renames itself after
            // its client at `initialize`.
            let name = std::env::var("HUMCASE_ACTOR").ok();
            if let Err(e) = db::journal::install(&conn, &db::journal::Actor::Cli(name)) {
                eprintln!("Error: unable to prepare the activity journal: {e}");
                std::process::exit(1);
            }
            (lease, conn)
        }
        Err(e) => {
            eprintln!(
                "Error: unable to open the SQLite database at {}: {e}",
                app_dir.display()
            );
            std::process::exit(1);
        }
    }
}
