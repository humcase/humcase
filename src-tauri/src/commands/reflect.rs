//! "Think it through" from the interface (`crate::reflect`).

use super::{mutation_allowed, AppState};
use crate::meaning::MeaningState;
use crate::models::project::ProjectSpace;
use crate::reflect::{self, ReflectPreviousTurn, ReflectReply, ReflectSummary};
use tauri::State;

/// The next reply of the thinking partner: restates what the person said, links what the cases
/// already say (with their sentences), may propose a lead, and asks one question. `subject_id`:
/// the case the person starts from; `space` as for `ask_cases`. Writes nothing but the
/// consumption journal.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn reflect_reply(
    state: State<'_, AppState>,
    meaning: State<'_, MeaningState>,
    subject_id: Option<String>,
    turns: Vec<ReflectPreviousTurn>,
    space: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<ReflectReply, String> {
    let data_dir = crate::data_location::data_dir()?;
    reflect::reply(
        &state.db,
        &state.db_path,
        &data_dir,
        Some(meaning.sense.clone()),
        subject_id,
        turns,
        space,
        provider,
        model,
    )
    .await
}

/// The summary of the conversation, for the person to correct. Writes nothing but the
/// consumption journal.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn reflect_summarize(
    state: State<'_, AppState>,
    meaning: State<'_, MeaningState>,
    subject_id: Option<String>,
    turns: Vec<ReflectPreviousTurn>,
    space: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<ReflectSummary, String> {
    let data_dir = crate::data_location::data_dir()?;
    reflect::summarize(
        &state.db,
        &state.db_path,
        &data_dir,
        Some(meaning.sense.clone()),
        subject_id,
        turns,
        space,
        provider,
        model,
    )
    .await
}

/// Keeps the summary the person corrected, with the ticked actions as sub-cases
/// (`crate::reflect::keep`).
#[tauri::command]
pub fn keep_reflection(
    state: State<'_, AppState>,
    subject_id: Option<String>,
    space: ProjectSpace,
    title: String,
    content: String,
    actions: Vec<String>,
) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    reflect::keep(
        &conn,
        subject_id.as_deref(),
        space,
        &title,
        &content,
        &actions,
    )
}
