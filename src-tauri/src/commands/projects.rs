use super::{mutation_allowed, AppState};
use crate::db::queries;
use crate::models::activity::Activity;
use crate::models::project::{Project, ProjectSpace};
use tauri::State;

#[tauri::command]
pub fn get_projects(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::get_projects(&conn).map_err(crate::db::db_error)
}

#[tauri::command]
pub fn create_project(
    state: State<'_, AppState>,
    name: String,
    color: Option<String>,
    space: Option<ProjectSpace>,
) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::create_project_in_space(&conn, &name, color.as_deref(), space)
        .map_err(crate::db::db_error)
}

#[tauri::command]
pub fn update_project_space(
    state: State<'_, AppState>,
    id: String,
    space: Option<ProjectSpace>,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::update_project_space(&conn, &id, space)
}

#[tauri::command]
pub fn rename_project(state: State<'_, AppState>, id: String, name: String) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::rename_project(&conn, &id, &name)
}

#[tauri::command]
pub fn set_project_color(
    state: State<'_, AppState>,
    id: String,
    color: String,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::set_project_color(&conn, &id, &color)
}

#[tauri::command]
pub fn set_project_archived(
    state: State<'_, AppState>,
    id: String,
    archived: bool,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::set_project_archived(&conn, &id, archived)
}

/// Deletes a project; its notes join the Inbox of its space. Returns their number.
#[tauri::command]
pub fn delete_project(state: State<'_, AppState>, id: String) -> Result<usize, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::delete_project(&conn, &id)
}

#[tauri::command]
pub fn get_activities(state: State<'_, AppState>) -> Result<Vec<Activity>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::get_activities(&conn)
}

#[tauri::command]
pub fn create_activity(
    state: State<'_, AppState>,
    name: String,
    space: ProjectSpace,
) -> Result<Activity, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::create_activity(&conn, &name, space)
}

#[tauri::command]
pub fn rename_activity(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<Activity, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::rename_activity(&conn, &id, &name)
}

#[tauri::command]
pub fn delete_activity(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::delete_activity(&conn, &id)
}

#[tauri::command]
pub fn set_project_activities(
    state: State<'_, AppState>,
    project_id: String,
    activity_ids: Vec<String>,
    expected_activity_ids: Option<Vec<String>>,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::set_project_activities(
        &conn,
        &project_id,
        &activity_ids,
        expected_activity_ids.as_deref(),
    )
}

#[tauri::command]
pub fn get_or_create_inbox(
    state: State<'_, AppState>,
    space: ProjectSpace,
) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::get_or_create_inbox(&conn, space)
}

#[tauri::command]
pub fn move_note_to_project(
    state: State<'_, AppState>,
    note_id: String,
    project_id: String,
    parent_id: Option<String>,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::file_inbox_note(&conn, &note_id, &project_id, parent_id.as_deref())
}
