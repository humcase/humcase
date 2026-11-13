//! The work profile and the nudges from the interface (`crate::profile`).

use super::{mutation_allowed, AppState};
use crate::profile::{
    self, Nudge, Profile, ProfileAnswer, ProfileItem, ProfileOutcome, ProfileSettings,
};
use serde::Serialize;
use tauri::State;

/// A space's profile and the lines proposed for it.
#[derive(Debug, Serialize)]
pub struct ProfileView {
    pub profile: Profile,
    pub pending: Vec<ProfileItem>,
}

/// Looks for lines to propose: counted ones at once, then the decisions Jev reads, asked
/// without holding the database. One refresh at a time; a second caller reads what is there.
async fn refresh(state: &State<'_, AppState>) -> Result<(), String> {
    let Some(_running) = profile::begin() else {
        return Ok(());
    };
    let data_dir = crate::data_location::data_dir()?;
    let decisions = {
        let _mutation = mutation_allowed(state)?;
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        profile::prepare(&conn, &data_dir)?
    };
    if decisions.is_empty() {
        return Ok(());
    }
    let chances = profile::judge(&state.db_path, &data_dir, &decisions).await;
    let _mutation = mutation_allowed(state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    profile::store_rules(&conn, &decisions, &chances)?;
    Ok(())
}

/// The profile of `space` (`work` or `private`) and the lines waiting for it.
#[tauri::command]
pub async fn work_profile(
    state: State<'_, AppState>,
    space: String,
    refresh: Option<bool>,
) -> Result<ProfileView, String> {
    if refresh.unwrap_or(false) {
        self::refresh(&state).await?;
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(ProfileView {
        profile: profile::read_profile(&conn, &space)?,
        pending: profile::pending(&conn, Some(&space))?,
    })
}

/// The profile note of `space`, created if needed (to edit it).
#[tauri::command]
pub fn open_work_profile(state: State<'_, AppState>, space: String) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let tx = crate::db::write_transaction(&conn).map_err(crate::db::db_error)?;
    let id = profile::ensure_profile(&tx, &space)?;
    tx.commit().map_err(crate::db::db_error)?;
    Ok(id)
}

#[tauri::command]
pub fn answer_profile(
    state: State<'_, AppState>,
    answers: Vec<ProfileAnswer>,
) -> Result<ProfileOutcome, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    profile::answer(&conn, &answers)
}

#[tauri::command]
pub fn undo_profile(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    profile::undo(&conn, &ids)
}

/// The cases waiting for a next step (chosen once a day).
#[tauri::command]
pub fn nudges(state: State<'_, AppState>) -> Result<Vec<Nudge>, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    profile::nudges(&conn)
}

#[tauri::command]
pub fn answer_nudge(state: State<'_, AppState>, id: String, answer: String) -> Result<(), String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    profile::answer_nudge(&conn, &id, &answer)
}

#[tauri::command]
pub fn profile_settings(state: State<'_, AppState>) -> Result<ProfileSettings, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    profile::settings(&conn)
}

#[tauri::command]
pub fn set_profile_settings(
    state: State<'_, AppState>,
    settings: ProfileSettings,
) -> Result<ProfileSettings, String> {
    let _mutation = mutation_allowed(&state)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    profile::set_settings(&conn, &settings)?;
    profile::settings(&conn)
}
