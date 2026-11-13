use super::{mutation_allowed, AppState, MutationGate};
use crate::backup::RestoreFailure;
use crate::process_lease::ExclusiveOutcome;
use std::sync::Mutex;
use tauri::{AppHandle, State};

/// Opens the application's data folder in Windows Explorer.
#[tauri::command]
pub fn open_data_dir() -> Result<String, String> {
    let dir = crate::data_location::data_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map_err(|e| crate::tr!("backup.explorerOpenFailed", error = e))?;
    Ok(dir.to_string_lossy().to_string())
}

/// Candidate folders for backups, in the order they are tried.
///
/// In an isolated test profile (`isolated_data_dir`), a single candidate: `backups` in the profile
/// folder. No fallback to Documents or the Desktop, so a manual test never writes into the user's
/// real backups.
fn backup_candidates(isolated_data_dir: Option<&std::path::Path>) -> Vec<std::path::PathBuf> {
    if let Some(dir) = isolated_data_dir {
        return vec![dir.join("backups")];
    }
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(documents) = dirs::document_dir() {
        candidates.push(documents.join("Humcase Backups"));
    }
    if let Some(desktop) = dirs::desktop_dir() {
        candidates.push(desktop.join("Humcase Backups"));
    }
    if let Ok(data) = crate::data_location::data_dir() {
        candidates.push(data.join("backups"));
    }
    candidates
}

/// Destination folder of the backup.
///
/// We do not rely on `dirs::document_dir()` alone: on some Windows profiles, "Documents"
/// is listable but not writable (redirection/junction), and `create_dir_all` then fails
/// with a misleading "file not found". So writing is actually tested, then we fall back
/// to the Desktop, then to the application's data folder. In an isolated profile, only the
/// profile folder (the one of the database) is used.
fn backup_directory(db_path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let isolated_data_dir = if crate::test_profile::is_requested() {
        db_path.parent()
    } else {
        None
    };
    let candidates = backup_candidates(isolated_data_dir);

    let mut last_error = crate::tr!("backup.noCandidateLocation");
    for candidate in candidates {
        if let Err(e) = std::fs::create_dir_all(&candidate) {
            last_error = format!("{} : {e}", candidate.display());
            continue;
        }
        let probe = candidate.join(".humcase-write-test");
        match std::fs::write(&probe, b"ok") {
            Ok(()) => {
                let _ = std::fs::remove_file(&probe);
                return Ok(candidate);
            }
            Err(e) => last_error = format!("{} : {e}", candidate.display()),
        }
    }
    Err(crate::tr!("backup.noWritableFolder", error = last_error))
}

/// Consistent backup of the database (folder resolved by `backup_directory`).
///
/// `VACUUM INTO` is used rather than a file copy: the copy includes transactions
/// still in the WAL journal and produces a compacted file, openable as is.
#[tauri::command]
pub fn backup_database(state: State<'_, AppState>, file_name: String) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let backup_dir = backup_directory(&state.db_path)?;

    // Sanitized file name (the name comes from the frontend, the destination is imposed on the Rust side).
    let safe_name: String = file_name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    let safe_name = if safe_name.is_empty() || !safe_name.ends_with(".db") {
        "humcase-backup.db".to_string()
    } else {
        safe_name
    };

    let destination = backup_dir.join(safe_name);
    if destination.exists() {
        return Err(crate::tr!(
            "backup.alreadyExistsChooseName",
            path = destination.display()
        ));
    }

    // Backup on a **dedicated connection**: the main connection's lock (interface and
    // MCP) is not held during the `VACUUM`, so the interface no longer freezes.
    crate::db::save_to(&state.db_path, &destination)?;

    // Reveal the file in Explorer (non-blocking failure).
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", destination.display()))
        .spawn();

    Ok(destination.to_string_lossy().to_string())
}

/// Portable archive: the destination stays imposed on the Rust side and never overwrites a file.
#[tauri::command]
pub fn create_backup_archive(
    state: State<'_, AppState>,
    file_name: String,
) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let backup_dir = backup_directory(&state.db_path)?;
    let safe_name = crate::backup::safe_archive_name(&file_name);
    let destination = backup_dir.join(safe_name);
    let images_dir = state
        .db_path
        .parent()
        .ok_or_else(|| crate::tr!("backup.dataFolderNotFound"))?
        .join("images");
    crate::backup::create_archive(&state.db_path, &images_dir, &destination)?;
    Ok(destination.to_string_lossy().to_string())
}

/// Validates an archive and returns its preview without modifying the user's database.
#[tauri::command]
pub fn inspect_backup_archive(path: String) -> Result<crate::backup::ArchivePreview, String> {
    crate::backup::inspect_archive(std::path::Path::new(&path))
}

/// Decision on how to handle the write gate (`mutation_gate`) after a restore
/// attempt.
///
/// Factored out of the Tauri command to be testable without launching the application.
/// The delicate point is the [`RestoreResolution::Blocked`] branch: the database may
/// already have been replaced on disk, so reopening writes from the old interface
/// would produce mutations on an inconsistent database.
#[derive(Debug, PartialEq, Eq)]
enum RestoreResolution {
    /// Restore committed and shared lock retaken: restart the process.
    Restart,
    /// Nothing was committed (failure before COMMIT, or operation not executed): reopen
    /// writes and display the error.
    Reopen(String),
    /// The data state is uncertain: **keep the gate frozen** and ask for a
    /// voluntary restart. NEVER reopen writes in this case.
    Blocked(String),
    /// Like `Blocked`, but the message is already complete and ready for the user (it
    /// starts with the state of the restore): it is not wrapped in "Restore
    /// failed".
    BlockedReported(String),
}

/// Result of applying a resolution to the write gate.
#[derive(Debug, PartialEq, Eq)]
struct RestoreApplication {
    /// `true` if the process must restart (complete success).
    restart: bool,
    /// Error to report to the frontend, if any.
    error: Option<String>,
    /// `true` if `error` is a complete message to display as is; otherwise the error is a
    /// raw cause that the command wraps with `backup.restoreFailed`.
    reported: bool,
}

/// Adds the reminder of the safety archive when it was actually written.
fn add_safety_hint(error: String, safety_path: &std::path::Path) -> String {
    if safety_path.exists() {
        crate::tr!(
            "backup.safetyArchiveKept",
            error = error,
            path = safety_path.display()
        )
    } else {
        error
    }
}

/// Exit procedure when a restore leaves the gate frozen.
///
/// Closing the window only hides it in the notification area (`lib.rs`): only the
/// "Quit" item of the icon menu actually stops the process and releases the session
/// lock. `app.restart()` is **not** used in the frozen branches: if the exclusive file
/// lock stayed held, the new process would not start and the user
/// would lose the whole interface.
pub(super) fn quit_and_restart() -> String {
    crate::tr!("backup.quitAndRestart")
}

/// Neutral message of the freeze after a restore: true whether the data was replaced,
/// partially replaced, or proven unchanged. Never assert an unverified state.
pub(super) fn blocked_after_restore() -> String {
    crate::tr!("backup.blockedAfterRestore")
}

/// Reason for refusing a **new** restore depending on the gate state. Pure function:
/// `Open` allows (`None`), `Restoring` and `BlockedAfterRestore` return their reason.
fn restore_entry_error(gate: &MutationGate) -> Option<String> {
    match gate {
        MutationGate::Open => None,
        MutationGate::Restoring => Some(crate::tr!("backup.restoreInProgress")),
        MutationGate::BlockedAfterRestore => Some(format!(
            "{} {}",
            blocked_after_restore(),
            quit_and_restart()
        )),
    }
}

/// Gate exit policy. Pure function: takes the outcome of the exclusive lock and the
/// path of the safety archive, touches neither Tauri nor any global state.
///
/// The decisive distinction is the **COMMIT state**: a `BeforeCommit` failure proves that
/// the transaction was cancelled by the ROLLBACK (data unchanged), a
/// `CommitAmbiguous` failure means SQLite may have published despite the error. Only the first
/// allows reopening writes.
fn resolve_restore_outcome<S>(
    outcome: ExclusiveOutcome<Result<S, RestoreFailure>>,
    safety_path: &std::path::Path,
) -> RestoreResolution {
    match outcome {
        // Invalid internal lock state: the operation was NOT executed, no data
        // was replaced. Writes are reopened, but since the session no longer holds its
        // shared lock, a restart is requested (message distinct from contention).
        ExclusiveOutcome::NotShared(error) => RestoreResolution::Reopen(format!(
            "{} {}",
            crate::tr!("backup.lockStateInvalid", error = error),
            quit_and_restart()
        )),
        // (a) The exclusive lock was not available: the operation did not run, nothing
        // is committed. The message stays focused on contention between sessions.
        ExclusiveOutcome::LockUnavailable(error) => {
            RestoreResolution::Reopen(crate::tr!("backup.lockUnavailable", error = error))
        }
        // (b) The operation ran and returned its result, lock retaken.
        ExclusiveOutcome::Completed(Ok(_)) => RestoreResolution::Restart,
        // Failure before COMMIT: transaction cancelled, data unchanged → reopen.
        ExclusiveOutcome::Completed(Err(RestoreFailure::BeforeCommit(message))) => {
            RestoreResolution::Reopen(add_safety_hint(message, safety_path))
        }
        // Failure at COMMIT: the state is ambiguous, never reopen writes.
        ExclusiveOutcome::Completed(Err(RestoreFailure::CommitAmbiguous(message))) => {
            RestoreResolution::Blocked(add_safety_hint(
                format!(
                    "{} {}",
                    crate::tr!("backup.commitAmbiguous", error = message),
                    quit_and_restart()
                ),
                safety_path,
            ))
        }
        // (c) The operation ran but retaking the shared lock failed: NEVER
        // reopen writes. The message depends on the COMMIT state.
        ExclusiveOutcome::LockRestoreFailed { value, error } => {
            let detail = match value {
                Ok(_) => crate::tr!("backup.restoreAppliedLockFailed", error = error),
                Err(RestoreFailure::BeforeCommit(_)) => {
                    crate::tr!("backup.restoreNotAppliedLockFailed", error = error)
                }
                Err(RestoreFailure::CommitAmbiguous(_)) => {
                    crate::tr!("backup.restoreUncertainLockFailed", error = error)
                }
            };
            // Complete message ready for the user: the frontend displays it as is.
            RestoreResolution::BlockedReported(add_safety_hint(
                format!("{detail} {}", quit_and_restart()),
                safety_path,
            ))
        }
    }
}

/// Applies the resolution to the write gate. Testable function: the gate goes to
/// [`MutationGate::BlockedAfterRestore`] in the blocked branch, which keeps
/// [`mutation_allowed`] refusing for the old UI.
fn apply_restore_resolution(
    gate: &mut MutationGate,
    resolution: RestoreResolution,
) -> RestoreApplication {
    match resolution {
        RestoreResolution::Restart => RestoreApplication {
            restart: true,
            error: None,
            reported: false,
        },
        RestoreResolution::Reopen(error) => {
            *gate = MutationGate::Open;
            RestoreApplication {
                restart: false,
                error: Some(error),
                reported: false,
            }
        }
        RestoreResolution::Blocked(error) => {
            *gate = MutationGate::BlockedAfterRestore;
            RestoreApplication {
                restart: false,
                error: Some(error),
                reported: false,
            }
        }
        RestoreResolution::BlockedReported(error) => {
            *gate = MutationGate::BlockedAfterRestore;
            RestoreApplication {
                restart: false,
                error: Some(error),
                reported: true,
            }
        }
    }
}

/// Replaces all notes and all projects after freezing local writes and closing
/// the other Humcase sessions. The engine keeps a safety archive of the old state.
/// Success restarts the process: no old view nor Quick Capture session survives.
#[tauri::command]
pub fn restore_backup_archive(
    app: AppHandle,
    state: State<'_, AppState>,
    lease_state: State<'_, Mutex<crate::process_lease::ProcessLease>>,
    path: String,
    expected_sha256: String,
) -> Result<(), String> {
    // Any returned error is a complete message, ready to display (the frontend does not interpret it).
    let mut gate = state
        .mutation_gate
        .write()
        .map_err(|e| crate::tr!("backup.restoreFailed", error = e))?;
    if let Some(error) = restore_entry_error(&gate) {
        return Err(crate::tr!("backup.restoreFailed", error = error));
    }
    *gate = MutationGate::Restoring;

    // Preparation outside the exclusive lock: no database mutation. An error here
    // must reopen writes.
    let prepared = (|| -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
        let safety_dir = backup_directory(&state.db_path)?;
        let safety_path = safety_dir.join(format!(
            "humcase-avant-restauration-{}.humcase.zip",
            uuid::Uuid::new_v4()
        ));
        let images_dir = state
            .db_path
            .parent()
            .ok_or_else(|| crate::tr!("backup.dataFolderNotFound"))?
            .join("images");
        Ok((safety_path, images_dir))
    })();

    let resolution = match prepared {
        Err(error) => RestoreResolution::Reopen(error),
        Ok((safety_path, images_dir)) => match lease_state.lock() {
            Ok(mut lease) => {
                let outcome = lease.with_exclusive(|| {
                    crate::backup::restore::restore_archive_into(
                        &state.db_path,
                        &images_dir,
                        std::path::Path::new(&path),
                        &safety_path,
                        &expected_sha256,
                    )
                });
                resolve_restore_outcome(outcome, &safety_path)
            }
            // Poisoned lock: nothing was attempted, reopen writes. The technical detail
            // (poisoning) stays in parentheses.
            Err(error) => {
                RestoreResolution::Reopen(crate::tr!("backup.lockPoisoned", error = error))
            }
        },
    };

    let application = apply_restore_resolution(&mut gate, resolution);
    if application.restart {
        // The shared read is retaken before the restart, so the new
        // process can open the database; the old UI's writes stay frozen.
        drop(gate);
        // Release the single-instance lock before relaunching: otherwise the new process
        // would find the old one still alive, hand over to it and stop.
        tauri_plugin_single_instance::destroy(&app);
        app.restart();
    }
    match application.error {
        Some(error) if application.reported => Err(error),
        Some(error) => Err(crate::tr!("backup.restoreFailed", error = error)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_profile_backups_stay_in_profile_folder() {
        let profile = std::path::Path::new("C:/test-profile");
        assert_eq!(
            backup_candidates(Some(profile)),
            vec![profile.join("backups")]
        );
        // Outside an isolated profile, the usual folders remain candidates (Documents, Desktop...).
        assert!(!backup_candidates(None)
            .iter()
            .any(|c| c.starts_with(profile)));
    }

    // The assertions target the French messages: thread language forced to French
    // (the global language may be changed by another test in parallel).
    fn quit_and_restart() -> String {
        crate::i18n::with_language("fr", super::quit_and_restart)
    }

    fn resolve_restore_outcome<S>(
        outcome: ExclusiveOutcome<Result<S, RestoreFailure>>,
        safety_path: &std::path::Path,
    ) -> RestoreResolution {
        crate::i18n::with_language("fr", || {
            super::resolve_restore_outcome(outcome, safety_path)
        })
    }

    fn restore_entry_error(gate: &MutationGate) -> Option<String> {
        crate::i18n::with_language("fr", || super::restore_entry_error(gate))
    }

    /// (d) Restore succeeded and lock retaken: restart, no error.
    #[test]
    fn restore_gate_policy_completed_ok_restarts() {
        let mut gate = MutationGate::Restoring;
        let application = apply_restore_resolution(
            &mut gate,
            resolve_restore_outcome::<()>(
                ExclusiveOutcome::Completed(Ok(())),
                std::path::Path::new("missing-archive.humcase.zip"),
            ),
        );

        assert!(
            application.restart,
            "a complete success must restart the process"
        );
        assert_eq!(
            application.error, None,
            "no error must be reported on success"
        );
    }

    /// Failure **before COMMIT**: the transaction was cancelled, writes are reopened.
    /// (historical variant "case b")
    #[test]
    fn restore_gate_policy_case_b_reopens_gate() {
        let mut gate = MutationGate::Restoring;
        let application = apply_restore_resolution(
            &mut gate,
            resolve_restore_outcome::<()>(
                ExclusiveOutcome::Completed(Err(RestoreFailure::BeforeCommit(
                    "restore failure".to_string(),
                ))),
                std::path::Path::new("missing-archive.humcase.zip"),
            ),
        );

        assert_eq!(
            gate,
            MutationGate::Open,
            "the gate must be reopened when nothing was committed"
        );
        assert!(!application.restart);
        assert_eq!(application.error.as_deref(), Some("restore failure"));
    }

    /// A failure before COMMIT recalls the safety archive **actually present** on disk.
    #[test]
    fn restore_gate_policy_before_commit_recalls_real_safety_archive() {
        let directory = tempfile::tempdir().unwrap();
        let safety = directory.path().join("safety.humcase.zip");
        std::fs::write(&safety, b"content").unwrap();

        let mut gate = MutationGate::Restoring;
        let application = apply_restore_resolution(
            &mut gate,
            resolve_restore_outcome::<()>(
                ExclusiveOutcome::Completed(Err(RestoreFailure::BeforeCommit(
                    "failure before COMMIT".to_string(),
                ))),
                &safety,
            ),
        );

        assert_eq!(gate, MutationGate::Open);
        let message = application.error.expect("message expected");
        assert!(
            message.contains("failure before COMMIT"),
            "message: {message}"
        );
        assert!(
            message.contains(safety.to_string_lossy().as_ref()),
            "the real safety archive must be mentioned: {message}"
        );
    }

    /// Failure **during COMMIT**: the state is ambiguous, the gate stays frozen.
    #[test]
    fn restore_gate_policy_commit_ambiguous_blocks_gate() {
        let mut gate = MutationGate::Restoring;
        let application = apply_restore_resolution(
            &mut gate,
            resolve_restore_outcome::<()>(
                ExclusiveOutcome::Completed(Err(RestoreFailure::CommitAmbiguous(
                    "disk full".to_string(),
                ))),
                std::path::Path::new("missing-archive.humcase.zip"),
            ),
        );

        assert_eq!(
            gate,
            MutationGate::BlockedAfterRestore,
            "a failure at COMMIT must freeze the gate"
        );
        assert!(!application.restart);
        let message = application.error.expect("message expected");
        assert!(
            message.contains("l’état des données est incertain"),
            "message: {message}"
        );
        assert!(
            message.contains(&quit_and_restart()),
            "exit procedure missing: {message}"
        );
    }

    /// (a) Exclusive lock unavailable: nothing was committed, writes are reopened
    /// with the historical contention message.
    #[test]
    fn restore_gate_policy_case_a_reopens_gate() {
        let mut gate = MutationGate::Restoring;
        let application = apply_restore_resolution(
            &mut gate,
            resolve_restore_outcome::<()>(
                ExclusiveOutcome::LockUnavailable(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "lock busy",
                )),
                std::path::Path::new("missing-archive.humcase.zip"),
            ),
        );

        assert_eq!(
            gate,
            MutationGate::Open,
            "no commit: the gate must be reopened"
        );
        let message = application.error.expect("message expected");
        assert!(
            message.contains("Fermez les autres sessions Humcase"),
            "message expected: {message}"
        );
    }

    /// m2 — an invalid internal lock state has its **own** message: it must not be
    /// confused with contention between sessions ("Close the other sessions").
    #[test]
    fn restore_gate_policy_not_shared_has_own_message() {
        let mut gate = MutationGate::Restoring;
        let application = apply_restore_resolution(
            &mut gate,
            resolve_restore_outcome::<()>(
                ExclusiveOutcome::NotShared(std::io::Error::other("shared lock missing")),
                std::path::Path::new("missing-archive.humcase.zip"),
            ),
        );

        assert_eq!(
            gate,
            MutationGate::Open,
            "no data changed: the gate reopens"
        );
        let message = application.error.expect("message expected");
        assert!(
            message.contains("verrou de session invalide"),
            "message: {message}"
        );
        assert!(
            !message.contains("Fermez les autres sessions"),
            "the message must not blame the other sessions: {message}"
        );
        assert!(
            message.contains(&quit_and_restart()),
            "exit procedure missing: {message}"
        );
    }

    /// Critical safeguard (case c): a committed restore whose retaking of the shared
    /// lock failed must leave the gate frozen and request the **real** exit procedure.
    ///
    /// This test FAILS if the freeze is removed from the blocked branch: it explicitly checks
    /// the gate state after the policy is applied.
    #[test]
    fn restore_gate_policy_case_c_keeps_gate_closed() {
        let mut gate = MutationGate::Restoring;
        let application = apply_restore_resolution(
            &mut gate,
            resolve_restore_outcome(
                ExclusiveOutcome::LockRestoreFailed {
                    value: Ok::<(), RestoreFailure>(()),
                    error: std::io::Error::other("lock lost"),
                },
                std::path::Path::new("missing-archive.humcase.zip"),
            ),
        );

        assert_eq!(
            gate,
            MutationGate::BlockedAfterRestore,
            "the gate must STAY frozen after a committed restore"
        );
        assert!(!application.restart, "no automatic restart in this case");
        let message = application.error.expect("an explanatory error is expected");
        assert!(
            message.contains("Restauration appliquée"),
            "neutral marker expected: {message}"
        );
        assert!(
            message.contains(&quit_and_restart()),
            "exit procedure missing: {message}"
        );
    }

    /// (c) + failure **before COMMIT**: nothing was published, but the lock could not be retaken.
    /// The gate stays frozen and the message tells the truth (data unchanged).
    #[test]
    fn restore_gate_policy_lock_restore_failed_before_commit_blocks_gate() {
        let mut gate = MutationGate::Restoring;
        let application = apply_restore_resolution(
            &mut gate,
            resolve_restore_outcome::<()>(
                ExclusiveOutcome::LockRestoreFailed {
                    value: Err(RestoreFailure::BeforeCommit(
                        "failure before COMMIT".to_string(),
                    )),
                    error: std::io::Error::other("lock lost"),
                },
                std::path::Path::new("missing-archive.humcase.zip"),
            ),
        );

        assert_eq!(gate, MutationGate::BlockedAfterRestore);
        assert!(!application.restart);
        let message = application.error.expect("message expected");
        assert!(
            message.contains("Restauration non appliquée"),
            "message: {message}"
        );
        assert!(
            message.contains("les données sont inchangées"),
            "message: {message}"
        );
        assert!(
            message.contains(&quit_and_restart()),
            "exit procedure missing: {message}"
        );
    }

    /// (c) + failure **during COMMIT**: the state is ambiguous, the gate stays frozen and the message
    /// does not claim the data is intact.
    #[test]
    fn restore_gate_policy_lock_restore_failed_commit_ambiguous_blocks_gate() {
        let mut gate = MutationGate::Restoring;
        let application = apply_restore_resolution(
            &mut gate,
            resolve_restore_outcome::<()>(
                ExclusiveOutcome::LockRestoreFailed {
                    value: Err(RestoreFailure::CommitAmbiguous("disk full".to_string())),
                    error: std::io::Error::other("lock lost"),
                },
                std::path::Path::new("missing-archive.humcase.zip"),
            ),
        );

        assert_eq!(gate, MutationGate::BlockedAfterRestore);
        assert!(!application.restart);
        let message = application.error.expect("message expected");
        assert!(
            message.contains("Restauration en état incertain"),
            "message: {message}"
        );
        assert!(
            message.contains("les données ont pu être remplacées"),
            "message: {message}"
        );
        assert!(
            message.contains(&quit_and_restart()),
            "exit procedure missing: {message}"
        );
    }

    /// F5 — entry of a new restore is refused depending on the gate state, through the
    /// pure function `restore_entry_error` (also called by the command).
    #[test]
    fn restore_entry_error_open_allows() {
        assert_eq!(restore_entry_error(&MutationGate::Open), None);
    }

    #[test]
    fn restore_entry_error_restoring_reports_in_progress() {
        let error = restore_entry_error(&MutationGate::Restoring).expect("refusal expected");
        assert!(error.contains("déjà en cours"), "message: {error}");
    }

    #[test]
    fn restore_entry_error_blocked_reports_neutral_exit() {
        let error =
            restore_entry_error(&MutationGate::BlockedAfterRestore).expect("refusal expected");
        assert!(
            error.contains("l’état des données peut avoir changé"),
            "message: {error}"
        );
        assert!(
            error.contains(&quit_and_restart()),
            "exit procedure missing: {error}"
        );
    }
}
