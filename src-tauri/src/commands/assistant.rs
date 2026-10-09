//! The assistant from the interface (`crate::assistant`).

use super::AppState;
use crate::assistant::{self, Reply, Status, Turn};
use crate::meaning::MeaningState;
use tauri::State;

/// Replies to what the person wrote with the AI chosen in Settings (`provider`, `model`: the
/// interface's choice). An empty `message` with `context_id`: the person opened that case to think
/// it through. Writes nothing but the journals; the actions it proposes run on the person's click.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn assistant_reply(
    state: State<'_, AppState>,
    meaning: State<'_, MeaningState>,
    message: String,
    previous: Vec<Turn>,
    space: Option<String>,
    context_id: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<Reply, String> {
    let data_dir = crate::data_location::data_dir()?;
    assistant::reply(
        &state.db,
        &state.db_path,
        &data_dir,
        Some(meaning.sense.clone()),
        &message,
        previous,
        space,
        context_id,
        provider,
        model,
    )
    .await
}

/// The model that would answer and the counts the assistant shows (cases, to decide, to tidy).
#[tauri::command]
pub fn assistant_status(
    state: State<'_, AppState>,
    space: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<Status, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    assistant::status(&conn, space.as_deref(), provider.as_deref(), model)
}
