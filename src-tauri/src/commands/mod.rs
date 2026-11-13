use rusqlite::Connection;
use std::sync::{Mutex, RwLock, RwLockReadGuard};

mod agents;
pub(crate) mod ai;
mod backup;
mod cases;
mod germinate;
mod images;
mod meaning;
mod natures;
mod notes;
mod profile;
mod projects;
mod tidy;
mod transcription;

pub use agents::*;
pub use ai::*;
pub use backup::*;
pub use cases::*;
pub use germinate::*;
pub use images::*;
pub use meaning::*;
pub use natures::*;
pub use notes::*;
pub use profile::*;
pub use projects::*;
pub use tidy::*;
pub use transcription::*;

use backup::{blocked_after_restore, quit_and_restart};

/// Language chosen in the interface: the engine texts (errors, notification-area icon menu)
/// follow it. MCP stays in English.
#[tauri::command]
pub fn set_ui_language(app: tauri::AppHandle, language: String) -> Result<(), String> {
    crate::i18n::set_language(&language);
    if let Some(tray) = app.tray_by_id(crate::TRAY_ID) {
        let menu = crate::tray_menu(&app).map_err(|e| e.to_string())?;
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// State of the mutation gate. A boolean conflated "restore in progress" and
/// "restore finished in an uncertain state": a second attempt then displayed
/// the wrong reason. The enum carries each situation all the way to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationGate {
    /// Writes allowed.
    Open,
    /// A restore is in progress: mutations are suspended for the duration of the operation.
    Restoring,
    /// A restore ended without being able to safely reopen writes
    /// (data possibly replaced): frozen until a voluntary restart.
    BlockedAfterRestore,
}

pub struct AppState {
    pub db: Mutex<Connection>,
    /// Path of the SQLite file: allows opening a **dedicated connection** (backup) without holding
    /// the lock of the main connection used by the interface and MCP.
    pub db_path: std::path::PathBuf,
    /// Write gate: serializes mutations with a restore. See [`MutationGate`].
    pub mutation_gate: RwLock<MutationGate>,
}

fn mutation_allowed(state: &AppState) -> Result<RwLockReadGuard<'_, MutationGate>, String> {
    let guard = state.mutation_gate.read().map_err(|e| e.to_string())?;
    match *guard {
        MutationGate::Open => Ok(guard),
        MutationGate::Restoring => Err(crate::tr!("ai.gate.restoring")),
        MutationGate::BlockedAfterRestore => Err(format!(
            "{} {}",
            blocked_after_restore(),
            quit_and_restart()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_gate(gate: MutationGate) -> AppState {
        AppState {
            db: Mutex::new(Connection::open_in_memory().unwrap()),
            db_path: std::path::PathBuf::from("test.db"),
            mutation_gate: RwLock::new(gate),
        }
    }

    /// m4 — the write refusal carries the **right** reason depending on the gate state, including on a
    /// second attempt after a freeze.
    #[test]
    fn mutation_allowed_reports_gate_state() {
        assert!(mutation_allowed(&state_with_gate(MutationGate::Open)).is_ok());

        let restoring = state_with_gate(MutationGate::Restoring);
        let error = crate::i18n::with_language("fr", || mutation_allowed(&restoring).unwrap_err());
        assert!(
            error.contains("restauration est déjà en cours"),
            "message: {error}"
        );

        let blocked = state_with_gate(MutationGate::BlockedAfterRestore);
        let error = mutation_allowed(&blocked).unwrap_err();
        assert!(
            error.contains("l’état des données peut avoir changé"),
            "message: {error}"
        );
        assert!(error.contains(&quit_and_restart()), "message: {error}");
    }
}
