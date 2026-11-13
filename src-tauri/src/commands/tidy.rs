//! "To tidy" from the interface (`crate::tidy`).

use super::{mutation_allowed, AppState};
use crate::gather::{self, ProjectSummary};
use crate::meaning::{MeaningState, Sense};
use crate::tidy::{self, Supersession, TidyAnswer, TidyItem, TidyOutcome, TidySettings};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::State;

/// Longest wait for a refresh already running: Jev answers or times out (60 s) before.
const WAIT_FOR_RUNNING: Duration = Duration::from_secs(75);

/// Refreshes "To tidy" (`tidy::prepare`), asks Jev without holding the database or the write
/// gate, then keeps its answers. While another refresh runs (the Home card and the page, a
/// decision written), this one waits for it: its answer is never older than that refresh. The
/// check of a decision just written then runs on its own, in case that refresh read it before.
/// A whole refresh then looks for cases to file together and cases that go against a decision
/// (`gather`), cited with the meaning worker (`sense`).
async fn run_refresh(
    state: &State<'_, AppState>,
    newer_only: Option<&str>,
    sense: Option<Arc<Sense>>,
) -> Result<(), String> {
    let mut waited = false;
    let _running = loop {
        if let Some(running) = tidy::begin() {
            break running;
        }
        if waited {
            return Ok(());
        }
        let started = Instant::now();
        while tidy::running() && started.elapsed() < WAIT_FOR_RUNNING {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if newer_only.is_none() {
            return Ok(());
        }
        waited = true;
    };
    let data_dir = crate::data_location::data_dir()?;
    let prepared = {
        let _mutation = mutation_allowed(state)?;
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        tidy::prepare(&conn, &data_dir, newer_only)?
    };
    if let Some(prepared) = prepared {
        let judged = tidy::judge(
            &state.db_path,
            &data_dir,
            &prepared.decisions,
            prepared.pairs,
        )
        .await;
        let _mutation = mutation_allowed(state)?;
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        tidy::store_pairs(&conn, &judged)?;
    }
    if newer_only.is_some() {
        return Ok(());
    }
    let prepared = {
        let _mutation = mutation_allowed(state)?;
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        gather::prepare(&conn, &data_dir)?
    };
    let Some(prepared) = prepared else {
        return Ok(());
    };
    let judged = gather::judge(
        &state.db_path,
        &data_dir,
        &prepared.cases,
        prepared.candidates,
    )
    .await;
    let judged = gather::cite_conflicts(sense, &prepared.cases, judged).await;
    let _mutation = mutation_allowed(state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    gather::store(&conn, &judged)?;
    Ok(())
}

/// The proposals waiting in `space` (every space when `None`), after a refresh: cases that may
/// go against a decision, decisions a newer one may replace, cases to file together, cases to
/// archive (Jev judges new pairs when it may read).
#[tauri::command]
pub async fn tidy_list(
    state: State<'_, AppState>,
    meaning: State<'_, MeaningState>,
    space: Option<String>,
    refresh: Option<bool>,
) -> Result<Vec<TidyItem>, String> {
    if refresh.unwrap_or(false) {
        run_refresh(&state, None, Some(meaning.sense.clone())).await?;
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    tidy::pending(&conn, space.as_deref())
}

/// The check when a decision is written: the older decisions it may replace.
#[tauri::command]
pub async fn tidy_check_decision(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<Supersession, String> {
    run_refresh(&state, Some(&note_id), None).await?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    tidy::supersession(&conn, &note_id)
}

/// Applies the person's answers in one transaction.
#[tauri::command]
pub fn answer_tidy(
    state: State<'_, AppState>,
    answers: Vec<TidyAnswer>,
) -> Result<TidyOutcome, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    tidy::answer(&conn, &answers)
}

/// Takes answers back (Undo right after).
#[tauri::command]
pub fn undo_tidy(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    tidy::undo(&conn, &ids)
}

/// A decision's place in the history: replaced by, replaces, and what it asks.
#[tauri::command]
pub fn case_supersession(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<Supersession, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    tidy::supersession(&conn, &note_id)
}

/// The person says a decision marked as replaced still holds.
#[tauri::command]
pub fn keep_decision_in_force(state: State<'_, AppState>, note_id: String) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    tidy::keep_in_force(&conn, &note_id)
}

/// The cases a case may go against: the band of its page.
#[tauri::command]
pub fn case_conflicts(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<Vec<TidyItem>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    gather::conflicts_of(&conn, &note_id)
}

/// What holds in a project, what moves, what stays open (its "Summary" page).
#[tauri::command]
pub fn project_summary(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<ProjectSummary, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    gather::project_summary(&conn, &project_id)
}

/// Ids of the decisions marked as replaced (lists and search).
#[tauri::command]
pub fn superseded_ids(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    tidy::superseded_ids(&conn)
}

#[tauri::command]
pub fn tidy_settings(state: State<'_, AppState>) -> Result<TidySettings, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    tidy::settings(&conn)
}

#[tauri::command]
pub fn set_tidy_settings(
    state: State<'_, AppState>,
    settings: TidySettings,
) -> Result<TidySettings, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    tidy::set_settings(&conn, &settings)?;
    tidy::refresh_archive(&conn)?;
    tidy::settings(&conn)
}
