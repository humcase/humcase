use super::{mutation_allowed, AppState};
use crate::db::cases;
use crate::models::note::{CaseProvenance, ContentKind, CreateNoteInput, NoteType};
use serde::Serialize;
use tauri::State;

/// An ancestor of a case, from the top-level case down to its direct parent.
#[derive(Debug, Serialize)]
pub struct CasePathItem {
    pub id: String,
    pub title: String,
}

/// What the open case shows around its content: where it sits and where it comes from.
#[derive(Debug, Serialize)]
pub struct CaseContext {
    pub path: Vec<CasePathItem>,
    pub provenance: Vec<CaseProvenance>,
}

#[tauri::command]
pub fn get_case_context(state: State<'_, AppState>, id: String) -> Result<CaseContext, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let path = cases::case_path(&conn, &id)?
        .into_iter()
        .map(|(id, title)| CasePathItem { id, title })
        .collect();
    let provenance = cases::list_provenance(&conn, &id)?;
    Ok(CaseContext { path, provenance })
}

/// Creates a sub-case inside `parent_id`, in the parent's project, of the given nature (a thought
/// when absent) and content kind (text when absent; an empty sketch when the content is).
/// Returns its identifier.
#[tauri::command]
pub fn create_sub_case(
    state: State<'_, AppState>,
    parent_id: String,
    title: String,
    note_type: Option<NoteType>,
    content_kind: Option<ContentKind>,
    content: Option<String>,
) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let input = CreateNoteInput {
        project_id: String::new(),
        title,
        content,
        note_type: note_type.unwrap_or(NoteType::Note),
        priority: None,
        git_branch: None,
        tags: None,
        due_at: None,
        raw_content: None,
    };
    cases::create_sub_case(&conn, &parent_id, content_kind.unwrap_or_default(), input)
}

/// Moves a case inside `parent_id`, or back to top level when it is absent.
#[tauri::command]
pub fn move_case(
    state: State<'_, AppState>,
    id: String,
    parent_id: Option<String>,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    cases::move_case(&conn, &id, parent_id.as_deref())
}

/// Positions of the cases placed on a board, in one project or in all of them.
#[tauri::command]
pub fn get_case_positions(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<Vec<cases::CasePosition>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    cases::case_positions(&conn, project_id.as_deref())
}

/// Places a case on its parent's board.
#[tauri::command]
pub fn set_case_position(
    state: State<'_, AppState>,
    id: String,
    x: f64,
    y: f64,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    cases::set_case_position(&conn, &id, x, y)
}

/// Places several cases at once, all or none.
#[tauri::command]
pub fn set_case_positions(
    state: State<'_, AppState>,
    positions: Vec<cases::CasePosition>,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    cases::set_case_positions(&conn, &positions)
}

/// The board of `case_id` (the top-level board of `project_id` when absent) as a JSON Canvas
/// document, as text.
#[tauri::command]
pub fn export_json_canvas(
    state: State<'_, AppState>,
    project_id: String,
    case_id: Option<String>,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let canvas = crate::db::json_canvas::export_canvas(&conn, &project_id, case_id.as_deref())?;
    serde_json::to_string_pretty(&canvas).map_err(|e| e.to_string())
}

/// Creates the cases of a JSON Canvas document, all or none, inside `parent_id` or on the
/// top-level board of `project_id`. Returns how many cases were created.
#[tauri::command]
pub fn import_json_canvas(
    state: State<'_, AppState>,
    project_id: Option<String>,
    parent_id: Option<String>,
    canvas: String,
) -> Result<crate::db::json_canvas::Imported, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::db::json_canvas::import_canvas(
        &conn,
        project_id.as_deref(),
        parent_id.as_deref(),
        &canvas,
    )
}

/// Whether `url` is a web address a link case may open: `http` or `https` with a host. Anything
/// else (`file:`, `javascript:`, a program path) is refused, so a link written by an agent
/// never starts a program.
fn is_web_address(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"));
    rest.is_some_and(|rest| {
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        !host.is_empty() && !url.chars().any(char::is_whitespace)
    })
}

/// Opens the address of a link case in the default browser.
#[tauri::command]
pub fn open_link(url: String) -> Result<(), String> {
    if !is_web_address(&url) {
        return Err(crate::tr!("data.linkNotWeb"));
    }
    tauri_plugin_opener::open_url(url.trim(), None::<&str>).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::is_web_address;

    #[test]
    fn only_web_addresses_open() {
        assert!(is_web_address("https://example.com/page?q=1"));
        assert!(is_web_address("http://localhost:3000"));
        for refused in [
            "file:///C:/Windows/System32/calc.exe",
            "javascript:alert(1)",
            r"C:\Windows\notepad.exe",
            "https://",
            "https:///path",
            "https://example.com/a b",
            "mailto:someone@example.com",
        ] {
            assert!(!is_web_address(refused), "{refused}");
        }
    }
}
