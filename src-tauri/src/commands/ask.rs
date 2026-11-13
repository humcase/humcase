//! Keeps an assistant conversation as a case (`crate::ask::keep`).

use super::{mutation_allowed, AppState};
use crate::ask::{self, KeptSource};
use crate::models::project::ProjectSpace;
use tauri::State;

/// Keeps an answer as a case of the Inbox of `space` (`ask::keep`); returns the new case's id.
#[tauri::command]
pub fn keep_answer(
    state: State<'_, AppState>,
    space: ProjectSpace,
    question: String,
    content: String,
    sources: Vec<KeptSource>,
) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    ask::keep(&conn, space, &question, &content, &sources)
}
