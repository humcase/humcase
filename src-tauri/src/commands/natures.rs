//! Natures to check from the interface (`crate::natures`).

use super::{mutation_allowed, AppState};
use crate::models::note::NoteType;
use crate::natures::{self, NatureState, Review, Status};
use tauri::State;

/// Natures to check in `space`, the surest first. Reads what Jev already read: nothing is sent.
#[tauri::command]
pub fn nature_review(
    state: State<'_, AppState>,
    space: Option<String>,
    limit: Option<usize>,
) -> Result<Review, String> {
    let data_dir = crate::data_location::data_dir()?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    natures::review(
        &conn,
        &data_dir,
        space.as_deref(),
        limit.unwrap_or(50).min(200),
    )
}

/// The person's answer to a proposed nature: change to `proposed`, or keep `reviewed`. False
/// when the case changed meanwhile (nothing applied).
#[tauri::command]
pub fn answer_nature(
    state: State<'_, AppState>,
    note_id: String,
    reviewed: NoteType,
    proposed: NoteType,
    change: bool,
) -> Result<bool, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    natures::answer(&conn, &note_id, reviewed, proposed, change)
}

/// Where Jev's reading of the cases stands.
#[tauri::command]
pub fn nature_reading_status(natures: State<'_, NatureState>) -> Result<Status, String> {
    natures
        .status
        .lock()
        .map(|s| s.clone())
        .map_err(|e| e.to_string())
}

/// Turns the review on or off (Jev reads the cases only while it is on).
#[tauri::command]
pub fn set_nature_reading(natures: State<'_, NatureState>, enabled: bool) -> Result<(), String> {
    natures::switch(&crate::data_location::data_dir()?, &natures, enabled)
}

/// A key was saved or cleared: the reader looks again at once.
#[tauri::command]
pub fn recheck_nature_reading(natures: State<'_, NatureState>) {
    natures
        .recheck
        .store(true, std::sync::atomic::Ordering::SeqCst);
}
