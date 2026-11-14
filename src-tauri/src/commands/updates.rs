//! Updates of the installed application: a check at startup, then a download and an install
//! the person starts from the interface.
//!
//! Only builds made by the release workflow look for updates: it sets `HUMCASE_UPDATES=1` at
//! compile time. A local build (`npm run update-exe`, `tauri dev`, the tests) never calls the
//! network for this and never offers to replace itself with an installed copy. The person can
//! also turn the check off in Settings (`update_check` = `"0"` in `app_settings`).
//!
//! The update is the installer of the release (NSIS, or MSI for a copy the MSI installed), signed
//! with the project's updater key: the plugin refuses an installer whose signature does not match
//! the public key in `tauri.conf.json`, or whose signed version differs from the announced one
//! (`requireSignedVersion`). On Windows the NSIS installer kills by name every `humcase-sense.exe`
//! (`windows/installer-hooks.nsh`) and every `Humcase.exe` of the current user, whatever their
//! folder (interface and MCP servers; a copy built elsewhere too, without its savers), installs,
//! then starts Humcase again.

use std::sync::Mutex;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use super::AppState;
use crate::db::permissions::{get_setting, set_setting};

/// `app_settings` key: `"0"` turns the check off; absent or any other value leaves it on.
pub const UPDATE_CHECK: &str = "update_check";

/// True for builds of the release workflow, the only ones that can update themselves.
pub const BUILT_FOR_UPDATES: bool = match option_env!("HUMCASE_UPDATES") {
    Some(value) => matches!(value.as_bytes(), b"1"),
    None => false,
};

/// The update found by the last check, then its downloaded installer.
#[derive(Default)]
pub struct UpdateState {
    found: Mutex<Option<Update>>,
    downloaded: Mutex<Option<(Update, Vec<u8>)>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UpdateSettings {
    /// This build can update itself (release build).
    pub supported: bool,
    /// The person leaves the check on.
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AvailableUpdate {
    pub version: String,
    pub current_version: String,
}

/// The check is on unless the person turned it off.
pub fn check_enabled(value: Option<&str>) -> bool {
    value != Some("0")
}

fn settings(state: &AppState) -> Result<UpdateSettings, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(UpdateSettings {
        supported: BUILT_FOR_UPDATES,
        enabled: check_enabled(get_setting(&conn, UPDATE_CHECK)?.as_deref()),
    })
}

#[tauri::command]
pub fn get_update_settings(state: State<'_, AppState>) -> Result<UpdateSettings, String> {
    settings(&state)
}

#[tauri::command]
pub fn set_update_check(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    set_setting(&conn, UPDATE_CHECK, if enabled { "1" } else { "0" })
}

/// Looks for a newer release. `None` when this build cannot update itself, when the person
/// turned the check off, or when Humcase is up to date.
#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    state: State<'_, AppState>,
    updates: State<'_, UpdateState>,
) -> Result<Option<AvailableUpdate>, String> {
    let current = settings(&state)?;
    if !current.supported || !current.enabled {
        return Ok(None);
    }
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let available = update.as_ref().map(|u| AvailableUpdate {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
    });
    *updates.found.lock().map_err(|e| e.to_string())? = update;
    Ok(available)
}

/// Downloads the installer of the update found by the last check and verifies its signature.
/// `progress` receives the percentage downloaded (0 to 100) when the size is known.
#[tauri::command]
pub async fn download_update(
    updates: State<'_, UpdateState>,
    progress: Channel<u8>,
) -> Result<(), String> {
    let update = updates
        .found
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| crate::tr!("updates.noneFound"))?;
    let mut received: u64 = 0;
    let mut last_sent: Option<u8> = None;
    let bytes = update
        .download(
            |chunk, total| {
                received += chunk as u64;
                if let Some(total) = total.filter(|t| *t > 0) {
                    let percent = (received.saturating_mul(100) / total).min(100) as u8;
                    if last_sent != Some(percent) {
                        last_sent = Some(percent);
                        let _ = progress.send(percent);
                    }
                }
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    *updates.downloaded.lock().map_err(|e| e.to_string())? = Some((update, bytes));
    Ok(())
}

/// Runs the downloaded installer. On Windows the installer closes Humcase, installs and starts
/// it again; elsewhere Humcase restarts itself. The interface saves what it holds before calling.
#[tauri::command]
pub fn install_update(app: AppHandle, updates: State<'_, UpdateState>) -> Result<(), String> {
    let (update, bytes) = updates
        .downloaded
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or_else(|| crate::tr!("updates.notDownloaded"))?;
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_check_is_on_unless_turned_off() {
        assert!(check_enabled(None));
        assert!(check_enabled(Some("1")));
        assert!(check_enabled(Some("unexpected")));
        assert!(!check_enabled(Some("0")));
    }
}
