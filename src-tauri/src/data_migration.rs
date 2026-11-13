//! Switch of the database file from `devnotes.db` (the product's former name) to `humcase.db`,
//! in the same folder. Run on the AppData folder of earlier versions before its data is moved to
//! `~/.humcase` (see `data_location`), and on an isolated test profile.
//!
//! Rules (user data):
//! - **copy, never move**: the former file is never modified; a marker is added next to it once
//!   the copy is verified;
//! - the copy is made under an **exclusive lock** on the former session lock, which every
//!   session of an earlier version holds while it runs: no session can write to the former
//!   database while it is being copied;
//! - the database is copied with `VACUUM INTO` (WAL included), then checked (`integrity_check`,
//!   schema version, row count of the user's tables) before being put in place **last**;
//! - a failure never creates an empty database: the caller refuses to start.

use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use rusqlite::Connection;

/// Application identifier (AppData Roaming and Local folders).
pub const APP_ID: &str = "com.humcase.desktop";

/// Database file of versions before 0.10.0.
pub(crate) const FORMER_DB_FILE: &str = "devnotes.db";
/// Session lock of versions before 0.10.0 (see `process_lease`).
const FORMER_LOCK_FILE: &str = "devnotes.session.lock";
/// Written next to the former file when the copy is verified and in place.
pub const MIGRATED_MARKER: &str = "devnotes.db-MIGRATED-TO-humcase.db.txt";
const STAGING_FILE: &str = "humcase.db.migrating";
/// User tables compared between the former and the new database.
const CHECKED_TABLES: [&str; 8] = [
    "projects",
    "notes",
    "note_revisions",
    "note_task_links",
    "activities",
    "project_activities",
    "dictation_fragments",
    "case_provenance",
];

/// What the migration found or did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// No former database: first installation.
    NothingToMigrate,
    /// The new database already exists.
    AlreadyMigrated,
    /// Copy made and verified during this call.
    Migrated,
}

/// Migrates if necessary. `wait` bounds the wait when a session of an earlier version is open.
pub fn migrate(dir: &Path, wait: Duration) -> Result<Outcome, String> {
    let new_db = dir.join(crate::db::DB_FILE);
    let deadline = Instant::now() + wait;
    loop {
        if new_db.is_file() {
            return Ok(Outcome::AlreadyMigrated);
        }
        if dir.join(MIGRATED_MARKER).is_file() {
            // The marker says "migrated" but the new database is missing: never start from an
            // empty database, the user would think their cases are lost.
            return Err(format!(
                "the data was copied to {} but this file is missing; the previous data is still in {}",
                new_db.display(),
                dir.join(FORMER_DB_FILE).display()
            ));
        }
        if !dir.join(FORMER_DB_FILE).is_file() {
            return Ok(Outcome::NothingToMigrate);
        }
        match try_lock_exclusive(dir) {
            Ok(lock) => {
                // Re-check under the lock: another session may have finished in the meantime.
                if new_db.is_file() {
                    return Ok(Outcome::AlreadyMigrated);
                }
                let result = copy_and_verify(dir);
                drop(lock);
                return result.map(|()| Outcome::Migrated);
            }
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(error) => {
                return Err(format!(
                    "another Humcase session is using {} and the data could not be migrated: {error}. \
                     Close every Humcase window and agent session, then start again.",
                    dir.join(FORMER_DB_FILE).display()
                ));
            }
        }
    }
}

fn try_lock_exclusive(dir: &Path) -> io::Result<fs::File> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(FORMER_LOCK_FILE))?;
    file.try_lock().map_err(io::Error::from)?;
    Ok(file)
}

fn copy_and_verify(dir: &Path) -> Result<(), String> {
    let staging = dir.join(STAGING_FILE);
    if staging.exists() {
        // Leftover of an interrupted attempt: never read, we start from scratch.
        fs::remove_file(&staging)
            .map_err(|e| format!("unable to clear {}: {e}", staging.display()))?;
    }
    let result = (|| {
        let source = Connection::open_with_flags(
            dir.join(FORMER_DB_FILE),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("unable to open the previous database: {e}"))?;
        source
            .execute("VACUUM INTO ?1", [staging.to_string_lossy().as_ref()])
            .map_err(|e| format!("unable to copy the database: {e}"))?;
        verify_copy(&source, &staging)?;
        drop(source);
        // Put in place last: the presence of the new file signals a complete copy.
        fs::rename(&staging, dir.join(crate::db::DB_FILE))
            .map_err(|e| format!("unable to place the database: {e}"))?;
        fs::write(
            dir.join(MIGRATED_MARKER),
            "Humcase copied devnotes.db to humcase.db and now uses humcase.db.\r\n\
             devnotes.db is kept intact and can be deleted by hand once Humcase has been verified.\r\n",
        )
        .map_err(|e| format!("unable to write the migration marker: {e}"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}

pub(crate) fn verify_copy(source: &Connection, staged_db: &Path) -> Result<(), String> {
    let copy = Connection::open(staged_db)
        .map_err(|e| format!("unable to open the copied database: {e}"))?;
    let integrity: String = copy
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| format!("integrity check failed: {e}"))?;
    if integrity != "ok" {
        return Err(format!("the copied database is damaged: {integrity}"));
    }
    let version = |c: &Connection| c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0));
    if version(source).map_err(|e| e.to_string())? != version(&copy).map_err(|e| e.to_string())? {
        return Err("the copied database has a different schema version".into());
    }
    for table in CHECKED_TABLES {
        let exists = |c: &Connection| -> Result<bool, String> {
            c.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .map_err(|e| e.to_string())
        };
        if !exists(source)? {
            continue;
        }
        let count = |c: &Connection| -> Result<i64, String> {
            c.query_row(&format!("SELECT count(*) FROM \"{table}\""), [], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())
        };
        let (before, after) = (count(source)?, count(&copy)?);
        if before != after {
            return Err(format!(
                "the copied table {table} has {after} rows instead of {before}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Representative former installation: WAL database whose last writes are not yet
    /// checkpointed. The connection is returned open, as a running session would keep it.
    fn former_install(dir: &Path) -> Connection {
        fs::create_dir_all(dir).unwrap();
        let conn = Connection::open(dir.join(FORMER_DB_FILE)).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
            .unwrap();
        crate::db::schema::apply_migrations(&conn).unwrap();
        let project = crate::db::queries::create_project(&conn, "Project", None).unwrap();
        for i in 0..5 {
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

    #[test]
    fn test_migration_copies_the_database_and_keeps_the_former_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let former = former_install(dir);
        let wal = dir.join(format!("{FORMER_DB_FILE}-wal"));
        assert!(
            fs::metadata(&wal).unwrap().len() > 0,
            "the scenario must leave pages in the WAL"
        );
        let former_before = fs::read(dir.join(FORMER_DB_FILE)).unwrap();

        assert_eq!(migrate(dir, Duration::ZERO).unwrap(), Outcome::Migrated);

        let conn = Connection::open(dir.join(crate::db::DB_FILE)).unwrap();
        let notes: i64 = conn
            .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(notes, 5, "the cases left in the WAL must be copied");
        assert!(dir.join(MIGRATED_MARKER).is_file());
        assert!(
            fs::read(dir.join(FORMER_DB_FILE)).unwrap() == former_before,
            "the former database must not change"
        );
        assert!(!dir.join(STAGING_FILE).exists());
        drop(former);

        // Relaunching does nothing.
        assert_eq!(
            migrate(dir, Duration::ZERO).unwrap(),
            Outcome::AlreadyMigrated
        );
    }

    #[test]
    fn test_fresh_install_has_nothing_to_migrate() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            migrate(tmp.path(), Duration::ZERO).unwrap(),
            Outcome::NothingToMigrate
        );
    }

    /// A session of an earlier version holds the former session lock: the copy waits, and never
    /// creates a database meanwhile.
    #[test]
    fn test_open_former_session_blocks_migration_without_creating_a_database() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        drop(former_install(dir));
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(FORMER_LOCK_FILE))
            .unwrap();
        lock.try_lock_shared().unwrap();

        let error = migrate(dir, Duration::ZERO).unwrap_err();
        assert!(error.contains("another Humcase session"), "{error}");
        assert!(!dir.join(crate::db::DB_FILE).exists());
        assert!(!dir.join(MIGRATED_MARKER).exists());
        drop(lock);

        assert_eq!(migrate(dir, Duration::ZERO).unwrap(), Outcome::Migrated);
    }

    #[test]
    fn test_marker_without_new_database_refuses_to_start_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        drop(former_install(dir));
        fs::write(dir.join(MIGRATED_MARKER), b"").unwrap();

        let error = migrate(dir, Duration::ZERO).unwrap_err();
        assert!(error.contains("this file is missing"), "{error}");
        assert!(!dir.join(crate::db::DB_FILE).exists());
    }

    /// A leftover of an interrupted copy is never used: it is replaced by a fresh copy.
    #[test]
    fn test_interrupted_copy_is_redone() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        drop(former_install(dir));
        fs::write(dir.join(STAGING_FILE), b"half a database").unwrap();

        assert_eq!(migrate(dir, Duration::ZERO).unwrap(), Outcome::Migrated);
        let conn = Connection::open(dir.join(crate::db::DB_FILE)).unwrap();
        let notes: i64 = conn
            .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(notes, 5);
    }

    /// An incomplete or different copy is refused, so it is never put in place.
    #[test]
    fn test_verify_copy_refuses_missing_rows_and_other_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let source = former_install(dir);
        let copy = dir.join("copy.db");
        source
            .execute("VACUUM INTO ?1", [copy.to_string_lossy().as_ref()])
            .unwrap();
        verify_copy(&source, &copy).unwrap();

        Connection::open(&copy)
            .unwrap()
            .execute_batch("DELETE FROM notes WHERE rowid = (SELECT max(rowid) FROM notes);")
            .unwrap();
        let error = verify_copy(&source, &copy).unwrap_err();
        assert!(error.contains("notes has 4 rows instead of 5"), "{error}");

        Connection::open(&copy)
            .unwrap()
            .execute_batch("PRAGMA user_version = 1;")
            .unwrap();
        let error = verify_copy(&source, &copy).unwrap_err();
        assert!(error.contains("different schema version"), "{error}");
    }

    /// Test on a **copy** of real data, never on AppData:
    /// `HUMCASE_MIGRATION_COPY=<copy of the data folder> cargo test -- --ignored real_copy`
    #[test]
    #[ignore]
    fn test_migration_on_real_copy() {
        let dir =
            PathBuf::from(std::env::var("HUMCASE_MIGRATION_COPY").expect("HUMCASE_MIGRATION_COPY"));
        assert!(
            !dir.starts_with(dirs::data_dir().unwrap()),
            "refused: real AppData folder"
        );
        assert_eq!(migrate(&dir, Duration::ZERO).unwrap(), Outcome::Migrated);
    }
}
