use super::{mutation_allowed, AppState};
use crate::db::{journal, permissions, queries, relations};
use crate::models::note::{
    Brief, CaseEvent, CaseReview, CreateNoteInput, Note, NoteRevision, ReviewState, UpdateNoteInput,
};
use tauri::State;

#[tauri::command]
pub fn set_note_archived(
    state: State<'_, AppState>,
    id: String,
    archived: bool,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::set_note_archived(&conn, &id, archived)
}

#[tauri::command]
pub fn get_notes(
    state: State<'_, AppState>,
    project_id: Option<String>,
    status: Option<String>,
    search_query: Option<String>,
) -> Result<Vec<Note>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::get_notes(
        &conn,
        project_id.as_deref(),
        status.as_deref(),
        search_query.as_deref(),
    )
    .map_err(crate::db::db_error)
}

/// Fingerprint of the state of notes, projects, activities and attachments, re-read periodically by
/// the interface (see `queries::notes_change_marker`).
#[tauri::command]
pub fn notes_change_marker(state: State<'_, AppState>) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::notes_change_marker(&conn).map_err(crate::db::db_error)
}

#[tauri::command]
pub fn get_note_by_id(state: State<'_, AppState>, id: String) -> Result<Note, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::get_note_by_id(&conn, &id).map_err(crate::db::db_error)
}

/// MCP permissions visible to the interface (no key, only flags).
#[derive(Debug, serde::Serialize)]
pub struct McpPermissions {
    pub allow_note_edit: bool,
}

#[tauri::command]
pub fn get_mcp_permissions(state: State<'_, AppState>) -> Result<McpPermissions, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(McpPermissions {
        allow_note_edit: permissions::allow_note_edit(&conn)?,
    })
}

/// Enables or disables note editing by an MCP agent. The flag is the only authorization:
/// absent or `"0"` = refused (see `db/permissions.rs`).
#[tauri::command]
pub fn set_mcp_allow_note_edit(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    permissions::set_setting(
        &conn,
        permissions::MCP_ALLOW_NOTE_EDIT,
        if enabled { "1" } else { "0" },
    )
}

/// Attributed history of a note's edits (most recent first).
#[tauri::command]
pub fn get_note_history(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<Vec<NoteRevision>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    permissions::note_history(&conn, &note_id)
}

/// Agent writes recorded since `since` (ISO 8601 UTC), all notes combined.
#[tauri::command]
pub fn list_agent_revisions_since(
    state: State<'_, AppState>,
    since: String,
) -> Result<Vec<NoteRevision>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    permissions::revisions_since(&conn, &since)
}

/// What changed since the person's last visit (results to review, agent writes, reworks).
#[tauri::command]
pub fn get_brief(state: State<'_, AppState>) -> Result<Brief, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    journal::brief(&conn)
}

/// Writes of agents and of the CLI since `since` (ISO 8601 UTC), for the day timeline.
#[tauri::command]
pub fn list_agent_events_since(
    state: State<'_, AppState>,
    since: String,
) -> Result<Vec<CaseEvent>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    journal::agent_events_since(&conn, &since)
}

/// "Mark all as seen": the brief starts again from now. Returns the new start.
#[tauri::command]
pub fn mark_brief_seen(state: State<'_, AppState>) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    journal::mark_brief_seen(&conn)
}

/// The person opened a case: the brief stops listing what happened on it before.
#[tauri::command]
pub fn mark_case_seen(state: State<'_, AppState>, note_id: String) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    journal::mark_case_seen(&conn, &note_id)
}

/// Who did what on a case, the decisions on its result and where they stand.
#[derive(Debug, serde::Serialize)]
pub struct CaseJournal {
    pub events: Vec<CaseEvent>,
    pub reviews: Vec<CaseReview>,
    pub review: ReviewState,
}

#[tauri::command]
pub fn get_case_journal(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<CaseJournal, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(CaseJournal {
        events: journal::case_events(&conn, &note_id)?,
        reviews: journal::case_reviews(&conn, &note_id)?,
        review: journal::review_state(&conn, &note_id)?,
    })
}

/// The person accepts the reviewed revision of a result or asks for a rework (with a reason).
#[tauri::command]
pub fn review_case(
    state: State<'_, AppState>,
    note_id: String,
    decision: String,
    expected_revision: i64,
    comment: Option<String>,
) -> Result<CaseReview, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let decision = match decision.as_str() {
        "accepted" => journal::Decision::Accept,
        "rework_requested" => journal::Decision::RequestRework,
        other => return Err(format!("unknown decision '{other}'")),
    };
    journal::review_case(
        &conn,
        &note_id,
        decision,
        expected_revision,
        comment.as_deref(),
    )
}

#[tauri::command]
pub fn create_note(state: State<'_, AppState>, input: CreateNoteInput) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    queries::create_note(&conn, input).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_note(
    state: State<'_, AppState>,
    id: String,
    input: UpdateNoteInput,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::update_note(&conn, &id, input)
}

/// Conditional write reserved for recovering a draft after a failure.
#[tauri::command]
pub fn retry_note_update(
    state: State<'_, AppState>,
    id: String,
    input: UpdateNoteInput,
    base: serde_json::Value,
) -> Result<queries::RetryNoteUpdateResult, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::retry_note_update(&conn, &id, input, base)
}

/// Deleting a note places it in the trash (restorable for 30 days), with its sub-cases. Returns
/// how many cases went to the trash.
#[tauri::command]
pub fn delete_note(state: State<'_, AppState>, id: String) -> Result<usize, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::trash_note(&conn, &id)
}

/// Places a case in the trash only if it is still empty (a new case left without anything).
/// Returns whether it went.
#[tauri::command]
pub fn trash_if_empty(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::trash_if_empty(&conn, &id)
}

#[tauri::command]
pub fn get_trashed_notes(state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::get_trashed_notes(&conn).map_err(crate::db::db_error)
}

#[tauri::command]
pub fn restore_from_trash(state: State<'_, AppState>, id: String) -> Result<usize, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::restore_from_trash(&conn, &id)
}

/// Permanently erases a note **from the trash** (confirmed in the interface).
#[tauri::command]
pub fn purge_note(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::purge_trash(&conn, queries::TrashPurge::One(&id)).map(|_| ())
}

/// Empties the trash of the cases the person confirmed (`ids`): a case trashed since stays.
/// Returns the number of notes erased.
#[tauri::command]
pub fn empty_trash(state: State<'_, AppState>, ids: Vec<String>) -> Result<usize, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::purge_trash(&conn, queries::TrashPurge::Listed(&ids))
}

#[tauri::command]
pub fn get_note_relations(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<relations::NoteRelations, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    relations::get_note_relations(&conn, &note_id)
}

#[tauri::command]
pub fn create_linked_task(
    state: State<'_, AppState>,
    source_note_id: String,
    title: String,
    content: Option<String>,
) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    relations::create_linked_task(&conn, &source_note_id, &title, content.as_deref())
}

#[tauri::command]
pub fn link_note_task(
    state: State<'_, AppState>,
    source_note_id: String,
    task_note_id: String,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    relations::link_note_task(&conn, &source_note_id, &task_note_id)
}

#[tauri::command]
pub fn unlink_note_task(
    state: State<'_, AppState>,
    source_note_id: String,
    task_note_id: String,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    relations::unlink_note_task(&conn, &source_note_id, &task_note_id)
}

/// Appends text to a note's content **atomically** (append in the database, no
/// read-modify-write): a concurrent write (MCP agent) cannot be overwritten.
/// Used by dictation processed while the target note is no longer displayed.
#[tauri::command]
pub fn append_note_content(
    state: State<'_, AppState>,
    id: String,
    text: String,
    auto_title: Option<String>,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::append_note_content(&conn, &id, &text, auto_title.as_deref())
}

#[tauri::command]
pub fn append_raw_content(
    state: State<'_, AppState>,
    id: String,
    text: String,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::append_raw_content(&conn, &id, &text)
}

/// The nature and line of work the wording of a case says, without AI (`crate::recognize`).
#[tauri::command]
pub fn recognize_case(text: String) -> crate::recognize::Recognition {
    crate::recognize::recognize(&text)
}

#[tauri::command]
pub fn find_similar_notes(
    state: State<'_, AppState>,
    query: String,
    exclude_id: Option<String>,
) -> Result<Vec<Note>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    // Keep the cause of the poisoning instead of replacing it with a generic message.
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    // The allowed space is deduced from the source note (the one being excluded): a note from the
    // Work space must never suggest a note from the Private space, and vice versa. A note without
    // a space (or not found) imposes no limit: historical behavior kept.
    let space = exclude_id
        .as_deref()
        .and_then(|id| queries::note_project_space(&conn, id).ok().flatten());
    // The same check as MCP's `find_similar_notes`: shared words, common ones left out.
    let near =
        queries::find_similar_notes(&conn, &query, exclude_id.as_deref(), space.as_deref(), 3)?;
    Ok(near
        .iter()
        .filter_map(|case| queries::get_note_by_id(&conn, &case.id).ok())
        .collect())
}

#[tauri::command]
pub fn get_daily_standup(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<Note>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::get_daily_standup(&conn, &project_id).map_err(crate::db::db_error)
}

#[tauri::command]
pub fn get_handoff(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::build_handoff(&conn, project_id.as_deref()).map_err(crate::db::db_error)
}
