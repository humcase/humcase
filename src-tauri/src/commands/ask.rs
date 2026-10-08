//! "Ask Humcase" from the interface (`crate::ask`).

use super::{mutation_allowed, AppState};
use crate::ask::{self, Answer, KeptSource, PreviousTurn};
use crate::meaning::MeaningState;
use crate::models::project::ProjectSpace;
use tauri::State;

/// Answers a question from the cases of `space` with the AI chosen in Settings (`provider`,
/// `model`: the interface's choice), or gives the closest cases when no AI is ready. Writes
/// nothing but the consumption journal.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn ask_cases(
    state: State<'_, AppState>,
    meaning: State<'_, MeaningState>,
    question: String,
    previous: Vec<PreviousTurn>,
    space: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<Answer, String> {
    let data_dir = crate::data_location::data_dir()?;
    ask::ask(
        &state.db,
        &state.db_path,
        &data_dir,
        Some(meaning.sense.clone()),
        &question,
        previous,
        space,
        provider,
        model,
        // Tools only when the first reading is weak (measured: `ask_eval`, `ASK_EVAL_MODE=auto`).
        ask::AskMode::Auto,
    )
    .await
}

/// Keeps an answer as a case of the Inbox of `space` (`ask::keep`); returns the new case's id.
#[tauri::command]
pub fn keep_answer(
    state: State<'_, AppState>,
    space: ProjectSpace,
    question: String,
    content: String,
    service: Option<String>,
    sources: Vec<KeptSource>,
) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    ask::keep(
        &conn,
        space,
        &question,
        &content,
        service.as_deref(),
        &sources,
    )
}
