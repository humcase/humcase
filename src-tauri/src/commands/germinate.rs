//! Make a case sprout from the interface (`crate::germinate`).

use super::{mutation_allowed, AppState};
use crate::germinate::{self, AnswerInput, Germination, Outcome, Proposal};
use crate::meaning::MeaningState;
use tauri::State;

/// Asks the AI chosen in Settings (`provider`, `model`: the interface's choice) for the next steps of a case and keeps them as proposals.
#[tauri::command]
pub async fn germinate_case(
    state: State<'_, AppState>,
    meaning: State<'_, MeaningState>,
    note_id: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<Germination, String> {
    let data_dir = crate::data_location::data_dir()?;
    let prepared = germinate::germinate(
        &state.db,
        &state.db_path,
        &data_dir,
        Some(meaning.sense.clone()),
        &note_id,
        provider,
        model,
    )
    .await?;
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    germinate::keep(&conn, prepared)
}

/// Proposals of a case waiting for the person's answer (Humcase's and the agents').
#[tauri::command]
pub fn list_proposals(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<Vec<Proposal>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    germinate::pending(&conn, &note_id)
}

/// Applies the person's answers in one transaction.
#[tauri::command]
pub fn answer_proposals(
    state: State<'_, AppState>,
    note_id: String,
    answers: Vec<AnswerInput>,
) -> Result<Outcome, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    germinate::answer(&conn, &note_id, &answers)
}

/// Takes the answers back (Undo right after answering).
#[tauri::command]
pub fn undo_proposals(
    state: State<'_, AppState>,
    note_id: String,
    ids: Vec<String>,
) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    germinate::undo(&conn, &note_id, &ids)
}
