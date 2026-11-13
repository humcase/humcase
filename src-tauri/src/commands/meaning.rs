//! Search by meaning from the interface (`crate::meaning`).

use super::AppState;
use crate::meaning::{self, Hit, MeaningState, Status};
use tauri::State;

/// Where the reading of the cases stands.
#[tauri::command]
pub fn meaning_status(meaning: State<'_, MeaningState>) -> Result<Status, String> {
    meaning
        .status
        .lock()
        .map(|s| s.clone())
        .map_err(|e| e.to_string())
}

/// Turns search by meaning on or off (the background reader follows within seconds).
#[tauri::command]
pub fn set_meaning_enabled(meaning: State<'_, MeaningState>, enabled: bool) -> Result<(), String> {
    meaning::switch(&crate::data_location::data_dir()?, &meaning, enabled)
}

/// Cases closest in meaning to `query`, best first. Empty when search by meaning is off.
#[tauri::command]
pub async fn search_by_meaning(
    state: State<'_, AppState>,
    meaning: State<'_, MeaningState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<Hit>, String> {
    let data_dir = crate::data_location::data_dir()?;
    if !meaning::enabled(&data_dir) {
        return Ok(Vec::new());
    }
    let sense = meaning.sense.clone();
    let limit = limit.unwrap_or(20).min(100);
    let hits = tauri::async_runtime::spawn_blocking(move || {
        meaning::search(&sense, &data_dir, &query, limit)
    })
    .await
    .map_err(|e| e.to_string())??;
    // Only live cases: the index may lag a few seconds behind a case put in the trash.
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut live = conn
        .prepare("SELECT 1 FROM notes WHERE id = ?1 AND deleted_at IS NULL")
        .map_err(|e| e.to_string())?;
    Ok(hits
        .into_iter()
        .filter(|h| live.exists([&h.note_id]).unwrap_or(false))
        .collect())
}
