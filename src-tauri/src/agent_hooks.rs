//! Agent hooks: reminders so an agent working with Humcase tracks its work. Agent clients
//! (Claude Code, Codex) run `humcase hook <event>` at moments of their session, with their JSON
//! on stdin (hooks declared in the user's settings, docs/AGENT_SETUP.md):
//!
//! - `session-start`: the tracking rules, and the brief (`get_handoff`) on a new session;
//! - `post-tool`: a file edit is noted; a commit or a merge gets a reminder to report;
//! - `stop`: when files changed during the turn and no agent or CLI wrote a case since the
//!   session started, the agent is asked once to update its case (or to say the work is not
//!   tracked).
//!
//! Humcase never writes in the agent's place: a reminder, not a report. A hook never breaks the
//! session either: any failure prints nothing. The state of a session (`agent-sessions/`, in the
//! data folder) holds no content, only times and fingerprints.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const SESSIONS_DIR: &str = "agent-sessions";
/// Session states untouched for this long are removed at the next session start.
const STATE_MAX_AGE: Duration = Duration::from_secs(14 * 24 * 3600);

pub const RULES: &str = "Humcase tracks the work of this session (Humcase hook).
- Before changing files for a task: find its case (list_notes with search_query), including the ones the person dictated, and continue in it; create one only if none covers the task, inside the case it belongs to. Then set it in_progress.
- Once the work is verified: update_note_status resolved with a resolution_comment (what was done, how it was checked, limits). Never resolve because it compiles.
- When every sub-case of a case is resolved, resolve that case too if its goal is reached, or report what remains in it.
- Work the person did not ask to track in Humcase needs nothing.";

pub const STOP_REASON: &str = "Humcase: files changed in this turn, but no case was written in this session. If this work belongs to a Humcase case, update it now: in_progress while working, resolved with a resolution_comment once verified. If it is not tracked in Humcase, say so in one line and stop.";

pub const DELIVERY_REMINDER: &str = "Humcase: if this delivers a tracked case, resolve it with a resolution_comment once the result is verified (not before).";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookEvent {
    SessionStart,
    PostTool,
    Stop,
}

/// Fields of the client's JSON that the hooks read; the rest is ignored.
#[derive(Deserialize, Default, Debug)]
pub struct HookInput {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub cwd: Option<String>,
    /// SessionStart: `startup`, `resume`, `clear`, `compact`.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub stop_hook_active: bool,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_input: serde_json::Value,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct SessionState {
    /// Case writes of agents and of the CLI count from here.
    pub started_ms: u64,
    /// The git state of the working folder at the start of the turn.
    pub git: Option<GitSnapshot>,
    /// A file edit tool ran during the turn.
    pub edited: bool,
    /// The stop reminder was already given in this session.
    pub reminded: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GitSnapshot {
    pub head: String,
    /// Fingerprint of `git status --porcelain`.
    pub status: u64,
}

pub fn is_edit_tool(name: &str) -> bool {
    matches!(
        name,
        "Edit"
            | "Write"
            | "MultiEdit"
            | "NotebookEdit"
            | "apply_patch"
            | "edit"
            | "write"
            | "patch"
    )
}

fn shell_command(input: &HookInput) -> Option<&str> {
    match input.tool_name.as_deref() {
        Some("Bash" | "PowerShell" | "bash" | "shell") => input.tool_input["command"].as_str(),
        _ => None,
    }
}

pub fn is_delivery_command(command: &str) -> bool {
    let words = command.split_whitespace().collect::<Vec<_>>();
    words
        .windows(2)
        .any(|w| w == ["git", "commit"] || w == ["git", "merge"])
        || words.windows(3).any(|w| w == ["gh", "pr", "merge"])
}

/// The stop reminder: once per session, when the turn changed files and nobody but the person
/// wrote a case since the session started.
pub fn should_remind(
    state: &SessionState,
    git_now: Option<&GitSnapshot>,
    agent_wrote: bool,
) -> bool {
    let changed = state.edited || state.git.as_ref() != git_now;
    !state.reminded && changed && !agent_wrote
}

fn context_output(event: &str, text: &str) -> String {
    serde_json::json!({
        "hookSpecificOutput": { "hookEventName": event, "additionalContext": text }
    })
    .to_string()
}

fn block_output(reason: &str) -> String {
    serde_json::json!({ "decision": "block", "reason": reason }).to_string()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// File of a session: its identifier kept to safe characters.
fn state_path(data_dir: &Path, session_id: &str) -> PathBuf {
    let mut name: String = session_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(100)
        .collect();
    if name.is_empty() {
        name = "unknown".to_string();
    }
    data_dir.join(SESSIONS_DIR).join(format!("{name}.json"))
}

fn load_state(path: &Path) -> Option<SessionState> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn save_state(path: &Path, state: &SessionState) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(state) {
        let _ = std::fs::write(path, bytes);
    }
}

fn remove_old_states(data_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(data_dir.join(SESSIONS_DIR)) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > STATE_MAX_AGE);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn git(cwd: &str, args: &[&str]) -> Option<String> {
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(cwd).args(args);
    command.stdin(std::process::Stdio::null());
    command.stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        // A release build has no console: without this flag, git would open a window.
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Commit and working tree of `cwd`; none outside a git repository.
fn git_snapshot(cwd: Option<&str>) -> Option<GitSnapshot> {
    let cwd = cwd?;
    let status = git(cwd, &["status", "--porcelain"])?;
    let head = git(cwd, &["rev-parse", "HEAD"]).unwrap_or_default();
    Some(GitSnapshot {
        head: head.trim().to_string(),
        status: fnv1a(status.as_bytes()),
    })
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The database, read only by the hooks; none when Humcase is unavailable (restore in progress,
/// no data yet).
fn open_db(data_dir: &Path) -> Option<(crate::process_lease::ProcessLease, rusqlite::Connection)> {
    if !data_dir.join("humcase.db").exists() {
        return None;
    }
    let lease = crate::process_lease::ProcessLease::acquire_shared(data_dir).ok()?;
    let conn = crate::db::init_db(data_dir).ok()?;
    Some((lease, conn))
}

/// True when an agent or the CLI wrote a case since `since_ms`.
fn agent_wrote_since(conn: &rusqlite::Connection, since_ms: u64) -> Option<bool> {
    let since: String = conn
        .query_row(
            "SELECT strftime('%Y-%m-%dT%H:%M:%fZ', ?1 / 1000.0, 'unixepoch')",
            [since_ms as i64],
            |row| row.get(0),
        )
        .ok()?;
    crate::db::journal::agent_events_since(conn, &since)
        .ok()
        .map(|events| !events.is_empty())
}

/// Runs one hook and returns what to print on stdout (nothing when there is nothing to say).
pub fn run(event: HookEvent, stdin: &str, data_dir: &Path) -> Option<String> {
    let input: HookInput = serde_json::from_str(stdin).unwrap_or_default();
    let path = state_path(data_dir, &input.session_id);
    match event {
        HookEvent::SessionStart => {
            remove_old_states(data_dir);
            let source = input.source.as_deref().unwrap_or("startup");
            // A resumed or compacted session keeps its writes counted from its real start.
            let previous = load_state(&path).filter(|_| matches!(source, "resume" | "compact"));
            save_state(
                &path,
                &SessionState {
                    started_ms: previous.as_ref().map_or_else(now_ms, |s| s.started_ms),
                    git: git_snapshot(input.cwd.as_deref()),
                    edited: false,
                    reminded: previous.is_some_and(|s| s.reminded),
                },
            );
            let mut text = RULES.to_string();
            if matches!(source, "startup" | "clear") {
                if let Some((_lease, conn)) = open_db(data_dir) {
                    if let Ok(handoff) = crate::mcp::call_tool_text(&conn, "get_handoff", None) {
                        text.push_str("\n\n");
                        text.push_str(&handoff);
                    }
                }
            }
            Some(context_output("SessionStart", &text))
        }
        HookEvent::PostTool => {
            let edit = input.tool_name.as_deref().is_some_and(is_edit_tool);
            let delivery = shell_command(&input).is_some_and(is_delivery_command);
            if edit || delivery {
                let mut state = load_state(&path).unwrap_or_else(|| SessionState {
                    started_ms: now_ms(),
                    ..SessionState::default()
                });
                if !state.edited {
                    state.edited = true;
                    save_state(&path, &state);
                }
            }
            delivery.then(|| context_output("PostToolUse", DELIVERY_REMINDER))
        }
        HookEvent::Stop => {
            let git_now = git_snapshot(input.cwd.as_deref());
            let Some(mut state) = load_state(&path) else {
                // Hooks installed during the session: count from now on.
                save_state(
                    &path,
                    &SessionState {
                        started_ms: now_ms(),
                        git: git_now,
                        ..SessionState::default()
                    },
                );
                return None;
            };
            let remind = !input.stop_hook_active
                && should_remind(&state, git_now.as_ref(), false)
                && open_db(data_dir)
                    .and_then(|(_lease, conn)| agent_wrote_since(&conn, state.started_ms))
                    == Some(false);
            // The next turn starts here, whatever the answer.
            state.git = git_now;
            state.edited = false;
            state.reminded |= remind;
            save_state(&path, &state);
            remind.then(|| block_output(STOP_REASON))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(head: &str, status: u64) -> GitSnapshot {
        GitSnapshot {
            head: head.to_string(),
            status,
        }
    }

    #[test]
    fn the_stop_reminder_needs_work_and_no_case_write() {
        let state = SessionState {
            started_ms: 1,
            git: Some(snapshot("a", 1)),
            edited: false,
            reminded: false,
        };
        // Nothing changed: no reminder.
        assert!(!should_remind(&state, Some(&snapshot("a", 1)), false));
        // A commit or a modified file: reminder, unless a case was written.
        assert!(should_remind(&state, Some(&snapshot("b", 1)), false));
        assert!(should_remind(&state, Some(&snapshot("a", 2)), false));
        assert!(!should_remind(&state, Some(&snapshot("a", 2)), true));
        // An edit tool outside git counts too.
        let edited = SessionState {
            git: None,
            edited: true,
            ..state.clone()
        };
        assert!(should_remind(&edited, None, false));
        // Once per session.
        let reminded = SessionState {
            reminded: true,
            ..edited
        };
        assert!(!should_remind(&reminded, None, false));
    }

    #[test]
    fn deliveries_and_edits_are_recognized() {
        assert!(is_delivery_command("git add -A && git commit -m 'x'"));
        assert!(is_delivery_command("gh pr merge 76 --merge"));
        assert!(is_delivery_command("git merge --no-ff feat/x"));
        assert!(!is_delivery_command("git status"));
        assert!(!is_delivery_command("echo commit"));
        assert!(is_edit_tool("Edit") && is_edit_tool("apply_patch") && is_edit_tool("write"));
        assert!(!is_edit_tool("Read") && !is_edit_tool("Bash"));
    }

    #[test]
    fn a_session_file_keeps_safe_characters_only() {
        let dir = Path::new("data");
        assert_eq!(
            state_path(dir, "../../evil\\name"),
            dir.join(SESSIONS_DIR).join("evilname.json")
        );
        assert_eq!(
            state_path(dir, ""),
            dir.join(SESSIONS_DIR).join("unknown.json")
        );
    }

    #[test]
    fn a_session_is_reminded_once_when_it_edits_without_writing_a_case() {
        let dir = tempfile::tempdir().unwrap();
        let start = r#"{"session_id":"s1","source":"startup"}"#;
        let out = run(HookEvent::SessionStart, start, dir.path()).unwrap();
        let context: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(context["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .starts_with("Humcase tracks"));

        // A turn without edits: nothing.
        assert_eq!(
            run(HookEvent::Stop, r#"{"session_id":"s1"}"#, dir.path()),
            None
        );

        // An edit, then the stop: without a database to read, no reminder is invented.
        let edit = r#"{"session_id":"s1","tool_name":"Edit","tool_input":{"file_path":"a"}}"#;
        assert_eq!(run(HookEvent::PostTool, edit, dir.path()), None);
        assert!(load_state(&state_path(dir.path(), "s1")).unwrap().edited);
        assert_eq!(
            run(HookEvent::Stop, r#"{"session_id":"s1"}"#, dir.path()),
            None
        );
        assert!(!load_state(&state_path(dir.path(), "s1")).unwrap().edited);

        // A commit gets the delivery reminder.
        let commit =
            r#"{"session_id":"s1","tool_name":"Bash","tool_input":{"command":"git commit -m x"}}"#;
        let out = run(HookEvent::PostTool, commit, dir.path()).unwrap();
        assert!(out.contains(DELIVERY_REMINDER));
    }

    #[test]
    fn the_stop_reminder_reads_the_journal() {
        let dir = tempfile::tempdir().unwrap();
        {
            let conn = crate::db::init_db(dir.path()).unwrap();
            crate::db::journal::install(&conn, &crate::db::journal::Actor::Cli(None)).unwrap();
        }
        run(HookEvent::SessionStart, r#"{"session_id":"s"}"#, dir.path());
        let edit = r#"{"session_id":"s","tool_name":"Write","tool_input":{}}"#;

        run(HookEvent::PostTool, edit, dir.path());
        let out = run(HookEvent::Stop, r#"{"session_id":"s"}"#, dir.path()).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&out).unwrap(),
            serde_json::json!({ "decision": "block", "reason": STOP_REASON })
        );
        // The agent answers; its next stop is let through, and the reminder is not repeated.
        assert_eq!(
            run(
                HookEvent::Stop,
                r#"{"session_id":"s","stop_hook_active":true}"#,
                dir.path()
            ),
            None
        );
        run(HookEvent::PostTool, edit, dir.path());
        assert_eq!(
            run(HookEvent::Stop, r#"{"session_id":"s"}"#, dir.path()),
            None
        );

        // Another session that writes a case through the CLI before stopping: no reminder.
        run(HookEvent::SessionStart, r#"{"session_id":"t"}"#, dir.path());
        {
            let conn = crate::db::init_db(dir.path()).unwrap();
            crate::db::journal::install(&conn, &crate::db::journal::Actor::Cli(None)).unwrap();
            let project_id = crate::db::queries::create_project(&conn, "Project", None).unwrap();
            crate::db::queries::create_note(
                &conn,
                crate::models::note::CreateNoteInput {
                    project_id,
                    title: "Tracked".to_string(),
                    content: None,
                    note_type: crate::models::note::NoteType::Todo,
                    priority: None,
                    git_branch: None,
                    tags: None,
                    due_at: None,
                    raw_content: None,
                },
            )
            .unwrap();
        }
        run(
            HookEvent::PostTool,
            r#"{"session_id":"t","tool_name":"Edit"}"#,
            dir.path(),
        );
        assert_eq!(
            run(HookEvent::Stop, r#"{"session_id":"t"}"#, dir.path()),
            None
        );
    }
}
