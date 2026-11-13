//! Where Humcase keeps its data: `%USERPROFILE%\.humcase` (`~/.humcase`), never AppData.
//!
//! Why not AppData: the Claude desktop app for Windows is an MSIX package, and Windows redirects
//! every write its child processes (agent shells, the Humcase MCP server, the CLI) make under
//! `%APPDATA%` or `%LOCALAPPDATA%` to a private copy inside the package. An agent then writes to a
//! database the application never sees. The user profile root is not redirected.
//!
//! Everything lives in this one folder: database, images, session lock, AI settings, window state
//! and the WebView2 profile (local storage of the interface, including the draft escrow).
//!
//! Moving from AppData (versions before 0.10.0, `%APPDATA%\com.humcase.desktop`) follows the
//! rules of user data:
//! - **copy, never move**: the AppData files are never modified; a marker is added there once the
//!   copy is verified;
//! - the copy runs under an **exclusive lock** on the former session locks, which every running
//!   session of an earlier version holds;
//! - the database is copied with `VACUUM INTO` (WAL included) and checked before being put in
//!   place **last**: its presence means the move is complete;
//! - a process running inside an MSIX package (an agent launched by the Claude app) never moves
//!   data: it cannot see the real AppData reliably, so it asks the user to open Humcase once;
//! - a failure never creates an empty database: the caller refuses to start.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use rusqlite::Connection;

/// Name of the data folder, in the user profile.
pub const DIR_NAME: &str = ".humcase";
/// Pasted images.
pub const IMAGES_DIR: &str = "images";
/// WebView2 profile of the interface windows (local storage, cache).
pub const WEBVIEW_DIR: &str = "webview";
/// Window size and position (`tauri-plugin-window-state`).
pub const WINDOW_STATE_FILE: &str = ".window-state.json";
/// Files carried over from AppData as they are, when present.
const CARRIED_FILES: [&str; 2] = [crate::ai_settings::FILE_NAME, WINDOW_STATE_FILE];
/// Written in the AppData folder once the copy is verified and in place.
pub const MOVED_MARKER: &str = "humcase.db-MOVED-TO-USERPROFILE-humcase.txt";
const STAGING_DB: &str = "humcase.db.moving";
const STAGING_WEBVIEW: &str = "webview.moving";
/// Session locks held by earlier versions while they run.
const LEGACY_LOCKS: [&str; 2] = [crate::process_lease::LOCK_FILE, "devnotes.session.lock"];
/// Folder WebView2 creates inside its user data folder.
const EBWEBVIEW: &str = "EBWebView";

static RESOLVED: OnceLock<PathBuf> = OnceLock::new();

/// Who is starting: the interface also carries the WebView2 profile over.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Process {
    Interface,
    Agent,
}

/// What the move found or did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Data already in the new folder.
    AlreadyMoved,
    /// Nothing in AppData: first installation.
    Fresh,
    /// Copy made and verified during this call.
    Moved,
}

/// Default data folder: `~/.humcase`.
pub fn default_dir() -> Result<PathBuf, String> {
    Ok(dirs::home_dir()
        .ok_or_else(|| crate::tr!("dataFolder.homeUnavailable"))?
        .join(DIR_NAME))
}

/// Data folder of this process: the one prepared at startup, otherwise the default one (or the
/// isolated test profile in a debug build).
pub fn data_dir() -> Result<PathBuf, String> {
    if let Some(dir) = RESOLVED.get() {
        return Ok(dir.clone());
    }
    crate::test_profile::app_data_dir(default_dir()?)
}

/// AppData folder of versions before 0.10.0.
pub fn legacy_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(crate::data_migration::APP_ID))
}

/// Local AppData folder where WebView2 kept the interface profile before 0.10.0.
fn legacy_webview_parent() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join(crate::data_migration::APP_ID))
}

/// Entry point of every process (interface, MCP, CLI), before any database is opened: resolves
/// the data folder, moves the data out of AppData if needed, and remembers the folder.
pub fn prepare_on_startup(process: Process) -> Result<PathBuf, String> {
    let dir = data_dir()?;
    if crate::test_profile::is_requested() {
        // Isolated profile: its own folder, never AppData.
        crate::data_migration::migrate(&dir, Duration::from_secs(20))?;
    } else {
        move_from_legacy(
            &dir,
            legacy_dir().as_deref(),
            running_in_package(),
            Duration::from_secs(20),
        )?;
        if process == Process::Interface {
            move_webview_profile(&dir, legacy_webview_parent().as_deref())?;
        }
    }
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // Unix: the data folder holds the database and the AI settings, for its owner only.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    let _ = RESOLVED.set(dir.clone());
    Ok(dir)
}

/// Moves the database, images, AI settings and window state from AppData to `new_dir`.
pub fn move_from_legacy(
    new_dir: &Path,
    legacy: Option<&Path>,
    packaged: bool,
    wait: Duration,
) -> Result<Outcome, String> {
    let new_db = new_dir.join(crate::db::DB_FILE);
    if new_db.is_file() {
        return Ok(Outcome::AlreadyMoved);
    }
    let Some(legacy) = legacy else {
        return Ok(Outcome::Fresh);
    };
    if legacy.join(MOVED_MARKER).is_file() {
        // The marker says "moved" but the new database is missing: never start from an empty
        // database, the user would think their cases are lost.
        return Err(crate::tr!(
            "dataFolder.movedButMissing",
            path = new_db.display(),
            legacy = legacy.display()
        ));
    }
    let has_data = legacy.join(crate::db::DB_FILE).is_file()
        || legacy.join(crate::data_migration::FORMER_DB_FILE).is_file();
    if !has_data {
        return Ok(Outcome::Fresh);
    }
    if packaged {
        return Err(crate::tr!(
            "dataFolder.openAppFirst",
            path = new_dir.display(),
            legacy = legacy.display()
        ));
    }
    // devnotes.db → humcase.db, in the AppData folder, for installations older than 0.10.0.
    crate::data_migration::migrate(legacy, wait)?;

    let deadline = Instant::now() + wait;
    loop {
        match lock_legacy(legacy) {
            Ok(locks) => {
                // Re-check under the locks: another process may have finished in the meantime.
                if new_db.is_file() {
                    return Ok(Outcome::AlreadyMoved);
                }
                let result = copy_and_verify(legacy, new_dir);
                drop(locks);
                return result.map(|()| Outcome::Moved);
            }
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(error) => {
                return Err(crate::tr!(
                    "dataFolder.legacyInUse",
                    path = legacy.display(),
                    error = error
                ));
            }
        }
    }
}

fn lock_legacy(legacy: &Path) -> io::Result<Vec<fs::File>> {
    LEGACY_LOCKS
        .iter()
        .map(|name| {
            let file = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(legacy.join(name))?;
            file.try_lock().map_err(io::Error::from)?;
            Ok(file)
        })
        .collect()
}

fn copy_and_verify(legacy: &Path, new_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(new_dir).map_err(|e| format!("{}: {e}", new_dir.display()))?;
    copy_images(&legacy.join(IMAGES_DIR), &new_dir.join(IMAGES_DIR))?;
    for name in CARRIED_FILES {
        let (from, to) = (legacy.join(name), new_dir.join(name));
        if from.is_file() && !to.exists() {
            fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
        }
    }

    let staging = new_dir.join(STAGING_DB);
    if staging.exists() {
        // Leftover of an interrupted attempt: never read, we start from scratch.
        fs::remove_file(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    }
    let result = (|| {
        let source = Connection::open_with_flags(
            legacy.join(crate::db::DB_FILE),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("unable to open the AppData database: {e}"))?;
        source
            .execute("VACUUM INTO ?1", [staging.to_string_lossy().as_ref()])
            .map_err(|e| format!("unable to copy the database: {e}"))?;
        crate::data_migration::verify_copy(&source, &staging)?;
        drop(source);
        // Put in place last: the presence of the database signals a complete move.
        fs::rename(&staging, new_dir.join(crate::db::DB_FILE))
            .map_err(|e| format!("unable to place the database: {e}"))?;
        fs::write(
            legacy.join(MOVED_MARKER),
            format!(
                "Humcase now keeps its data in {}.\r\n\
                 The files of this folder are kept intact and can be deleted by hand once Humcase \
                 has been verified.\r\n",
                new_dir.display()
            ),
        )
        .map_err(|e| format!("unable to write the move marker: {e}"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}

/// Copies every image (flat folder) and checks each one arrived with the same size. Images are
/// never modified once written, so a file already present with the same size is kept.
fn copy_images(from: &Path, to: &Path) -> Result<(), String> {
    if !from.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let source = entry.path();
        if !source.is_file() {
            continue;
        }
        let target = to.join(entry.file_name());
        let size = entry.metadata().map_err(|e| e.to_string())?.len();
        let present = fs::metadata(&target)
            .map(|m| m.len() == size)
            .unwrap_or(false);
        if !present {
            fs::copy(&source, &target).map_err(|e| format!("{}: {e}", source.display()))?;
        }
        let copied = fs::metadata(&target).map_err(|e| e.to_string())?.len();
        if copied != size {
            return Err(format!(
                "the image {} was not copied completely",
                target.display()
            ));
        }
    }
    Ok(())
}

/// Carries the interface's WebView2 profile over (local storage: draft escrow, spaces, theme).
/// Copied whole, then checked file by file, before being put in place.
pub fn move_webview_profile(new_dir: &Path, legacy_parent: Option<&Path>) -> Result<(), String> {
    let target = new_dir.join(WEBVIEW_DIR);
    if target.exists() {
        return Ok(());
    }
    let Some(source) = legacy_parent
        .map(|p| p.join(EBWEBVIEW))
        .filter(|p| p.is_dir())
    else {
        return Ok(());
    };
    let staging = new_dir.join(STAGING_WEBVIEW);
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    }
    let result = (|| {
        let copied = staging.join(EBWEBVIEW);
        copy_tree(&source, &copied)?;
        let (expected, got) = (tree_summary(&source)?, tree_summary(&copied)?);
        if expected != got {
            return Err(format!(
                "the interface profile was not copied completely ({} files, {} bytes instead of {} files, {} bytes)",
                got.0, got.1, expected.0, expected.1
            ));
        }
        fs::rename(&staging, &target).map_err(|e| format!("{}: {e}", target.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result.map_err(|error| crate::tr!("dataFolder.webviewCopyFailed", error = error))
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&source, &target)?;
        } else {
            fs::copy(&source, &target).map_err(|e| format!("{}: {e}", source.display()))?;
        }
    }
    Ok(())
}

/// Number of files and total size of a folder tree.
fn tree_summary(dir: &Path) -> Result<(u64, u64), String> {
    let mut summary = (0, 0);
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            let (files, bytes) = tree_summary(&entry.path())?;
            summary = (summary.0 + files, summary.1 + bytes);
        } else {
            summary = (
                summary.0 + 1,
                summary.1 + entry.metadata().map_err(|e| e.to_string())?.len(),
            );
        }
    }
    Ok(summary)
}

/// True when this process runs inside an MSIX package (for instance launched by the Claude
/// desktop app): its writes under AppData are redirected and its view of AppData is unreliable.
#[cfg(windows)]
pub fn running_in_package() -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentPackageFullName(length: *mut u32, name: *mut u16) -> i32;
    }
    const APPMODEL_ERROR_NO_PACKAGE: i32 = 15700;
    let mut length = 0u32;
    // SAFETY: a zero length with a null buffer only asks for the required size.
    let code = unsafe { GetCurrentPackageFullName(&mut length, std::ptr::null_mut()) };
    code != APPMODEL_ERROR_NO_PACKAGE
}

#[cfg(not(windows))]
pub fn running_in_package() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Representative AppData installation: WAL database with writes not yet checkpointed,
    /// images, AI settings and window state. The connection stays open, as a running session.
    fn legacy_install(dir: &Path) -> Connection {
        fs::create_dir_all(dir.join(IMAGES_DIR)).unwrap();
        fs::write(dir.join(IMAGES_DIR).join("a.webp"), b"image a").unwrap();
        fs::write(dir.join(IMAGES_DIR).join("b.jpg"), b"image b, longer").unwrap();
        fs::write(dir.join(crate::ai_settings::FILE_NAME), b"{\"keys\":{}}").unwrap();
        fs::write(dir.join(WINDOW_STATE_FILE), b"{}").unwrap();
        let conn = Connection::open(dir.join(crate::db::DB_FILE)).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
            .unwrap();
        crate::db::schema::apply_migrations(&conn).unwrap();
        let project = crate::db::queries::create_project(&conn, "Project", None).unwrap();
        for i in 0..3 {
            crate::db::queries::create_note(
                &conn,
                crate::models::note::CreateNoteInput {
                    project_id: project.clone(),
                    title: format!("Case {i}"),
                    content: Some("content".into()),
                    note_type: crate::models::note::NoteType::Todo,
                    priority: None,
                    git_branch: None,
                    tags: None,
                    due_at: None,
                    raw_content: None,
                },
            )
            .unwrap();
        }
        conn
    }

    fn file_bytes(path: &Path) -> Vec<u8> {
        fs::read(path).unwrap()
    }

    #[test]
    fn move_copies_everything_and_keeps_appdata_intact() {
        let tmp = tempfile::tempdir().unwrap();
        let (legacy, new_dir) = (tmp.path().join("appdata"), tmp.path().join(".humcase"));
        let session = legacy_install(&legacy);
        let before = file_bytes(&legacy.join(crate::db::DB_FILE));

        assert_eq!(
            move_from_legacy(&new_dir, Some(&legacy), false, Duration::ZERO).unwrap(),
            Outcome::Moved
        );

        let conn = Connection::open(new_dir.join(crate::db::DB_FILE)).unwrap();
        let notes: i64 = conn
            .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(notes, 3, "the cases left in the WAL must be copied");
        for name in ["a.webp", "b.jpg"] {
            assert_eq!(
                file_bytes(&new_dir.join(IMAGES_DIR).join(name)),
                file_bytes(&legacy.join(IMAGES_DIR).join(name))
            );
        }
        assert!(new_dir.join(crate::ai_settings::FILE_NAME).is_file());
        assert!(new_dir.join(WINDOW_STATE_FILE).is_file());
        assert!(legacy.join(MOVED_MARKER).is_file());
        assert!(!new_dir.join(STAGING_DB).exists());
        assert_eq!(
            file_bytes(&legacy.join(crate::db::DB_FILE)),
            before,
            "the AppData database must not change"
        );
        assert!(legacy.join(IMAGES_DIR).join("a.webp").is_file());
        drop(session);

        assert_eq!(
            move_from_legacy(&new_dir, Some(&legacy), false, Duration::ZERO).unwrap(),
            Outcome::AlreadyMoved
        );
    }

    /// An agent launched by the Claude app (MSIX) sees a redirected AppData: it must never move
    /// data, nor create an empty database in the new folder.
    #[test]
    fn packaged_process_never_moves_data() {
        let tmp = tempfile::tempdir().unwrap();
        let (legacy, new_dir) = (tmp.path().join("appdata"), tmp.path().join(".humcase"));
        drop(legacy_install(&legacy));

        let error = move_from_legacy(&new_dir, Some(&legacy), true, Duration::ZERO).unwrap_err();
        assert!(
            error.contains(&new_dir.display().to_string()),
            "the message must name the new folder: {error}"
        );
        assert!(!new_dir.join(crate::db::DB_FILE).exists());
        assert!(!legacy.join(MOVED_MARKER).exists());
    }

    #[test]
    fn fresh_install_and_empty_appdata_move_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let new_dir = tmp.path().join(".humcase");
        assert_eq!(
            move_from_legacy(&new_dir, None, false, Duration::ZERO).unwrap(),
            Outcome::Fresh
        );
        let empty = tmp.path().join("appdata");
        fs::create_dir_all(&empty).unwrap();
        assert_eq!(
            move_from_legacy(&new_dir, Some(&empty), true, Duration::ZERO).unwrap(),
            Outcome::Fresh
        );
    }

    /// A session of an earlier version holds its session lock: the move waits and never creates
    /// a database meanwhile.
    #[test]
    fn running_earlier_session_blocks_the_move() {
        let tmp = tempfile::tempdir().unwrap();
        let (legacy, new_dir) = (tmp.path().join("appdata"), tmp.path().join(".humcase"));
        drop(legacy_install(&legacy));
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(legacy.join("humcase.session.lock"))
            .unwrap();
        lock.try_lock_shared().unwrap();

        assert!(move_from_legacy(&new_dir, Some(&legacy), false, Duration::ZERO).is_err());
        assert!(!new_dir.join(crate::db::DB_FILE).exists());
        assert!(!legacy.join(MOVED_MARKER).exists());
        drop(lock);

        assert_eq!(
            move_from_legacy(&new_dir, Some(&legacy), false, Duration::ZERO).unwrap(),
            Outcome::Moved
        );
    }

    #[test]
    fn marker_without_database_refuses_to_start_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let (legacy, new_dir) = (tmp.path().join("appdata"), tmp.path().join(".humcase"));
        drop(legacy_install(&legacy));
        fs::write(legacy.join(MOVED_MARKER), b"").unwrap();

        assert!(move_from_legacy(&new_dir, Some(&legacy), false, Duration::ZERO).is_err());
        assert!(!new_dir.join(crate::db::DB_FILE).exists());
    }

    /// Installation older than 0.10.0 (`devnotes.db` in AppData): renamed in AppData, then moved.
    #[test]
    fn devnotes_install_is_renamed_then_moved() {
        let tmp = tempfile::tempdir().unwrap();
        let (legacy, new_dir) = (tmp.path().join("appdata"), tmp.path().join(".humcase"));
        drop(legacy_install(&legacy));
        fs::rename(
            legacy.join(crate::db::DB_FILE),
            legacy.join(crate::data_migration::FORMER_DB_FILE),
        )
        .unwrap();

        assert_eq!(
            move_from_legacy(&new_dir, Some(&legacy), false, Duration::ZERO).unwrap(),
            Outcome::Moved
        );
        let conn = Connection::open(new_dir.join(crate::db::DB_FILE)).unwrap();
        let notes: i64 = conn
            .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(notes, 3);
    }

    /// A leftover of an interrupted copy is never used.
    #[test]
    fn interrupted_copy_is_redone() {
        let tmp = tempfile::tempdir().unwrap();
        let (legacy, new_dir) = (tmp.path().join("appdata"), tmp.path().join(".humcase"));
        drop(legacy_install(&legacy));
        fs::create_dir_all(&new_dir).unwrap();
        fs::write(new_dir.join(STAGING_DB), b"half a database").unwrap();

        assert_eq!(
            move_from_legacy(&new_dir, Some(&legacy), false, Duration::ZERO).unwrap(),
            Outcome::Moved
        );
        assert!(!new_dir.join(STAGING_DB).exists());
    }

    #[test]
    fn webview_profile_is_copied_whole_and_kept_in_appdata() {
        let tmp = tempfile::tempdir().unwrap();
        let (local, new_dir) = (tmp.path().join("local"), tmp.path().join(".humcase"));
        let storage = local
            .join(EBWEBVIEW)
            .join("Default")
            .join("Local Storage")
            .join("leveldb");
        fs::create_dir_all(&storage).unwrap();
        fs::write(storage.join("000003.log"), b"escrowed draft").unwrap();
        fs::write(storage.join("CURRENT"), b"MANIFEST-000001").unwrap();

        move_webview_profile(&new_dir, Some(&local)).unwrap();

        let copied = new_dir
            .join(WEBVIEW_DIR)
            .join(EBWEBVIEW)
            .join("Default")
            .join("Local Storage")
            .join("leveldb");
        assert_eq!(file_bytes(&copied.join("000003.log")), b"escrowed draft");
        assert!(storage.join("000003.log").is_file(), "AppData stays intact");
        assert!(!new_dir.join(STAGING_WEBVIEW).exists());

        // Once moved, the profile in use is never overwritten again.
        fs::write(copied.join("000003.log"), b"newer draft").unwrap();
        move_webview_profile(&new_dir, Some(&local)).unwrap();
        assert_eq!(file_bytes(&copied.join("000003.log")), b"newer draft");
    }

    /// The data folder is never under AppData (the reason for this module).
    #[test]
    fn default_dir_is_outside_appdata() {
        let dir = default_dir().unwrap();
        assert!(dir.ends_with(DIR_NAME));
        for appdata in [dirs::data_dir(), dirs::data_local_dir()]
            .into_iter()
            .flatten()
        {
            assert!(
                !dir.starts_with(&appdata),
                "{} is under AppData",
                dir.display()
            );
        }
    }
}
