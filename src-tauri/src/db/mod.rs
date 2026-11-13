pub mod activities;
pub mod cases;
pub mod journal;
pub mod json_canvas;
pub mod permissions;
pub mod queries;
pub mod relations;
pub mod schema;
pub mod structure;
#[cfg(test)]
mod tests;

use rusqlite::{Connection, Result, Transaction, TransactionBehavior};
use std::path::Path;

/// Message of a database failure for the person: the failures that come from outside Humcase
/// (another program holding the file, a read-only folder, a full disk, a damaged file) are told in
/// the interface language; any other keeps SQLite's own text, which names the rule or trigger
/// that refused the write.
pub fn db_error(error: rusqlite::Error) -> String {
    use rusqlite::ErrorCode;
    match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => crate::tr!("data.dbBusy"),
        Some(ErrorCode::ReadOnly) => crate::tr!("data.dbReadOnly"),
        Some(ErrorCode::DiskFull) => crate::tr!("data.dbFull"),
        Some(ErrorCode::SystemIoFailure | ErrorCode::CannotOpen) => {
            crate::tr!("data.dbIo", error = error)
        }
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => {
            crate::tr!("data.dbCorrupt", error = error)
        }
        _ => error.to_string(),
    }
}

/// Database file in the data folder (`devnotes.db` before 0.10.0, see `data_migration`).
pub const DB_FILE: &str = "humcase.db";

pub fn init_db(app_dir: &Path) -> Result<Connection> {
    let db_path = app_dir.join(DB_FILE);

    // The folder must exist: a silent failure would make opening fail further on,
    // with a message the user cannot understand.
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(14), // SQLITE_CANTOPEN
                Some(format!("unable to create {}: {e}", parent.display())),
            )
        })?;
    }

    let conn = Connection::open(&db_path)?;
    // Check before WAL mode: this PRAGMA can modify the header of a newer database.
    schema::supported_version(&conn)?;
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA busy_timeout = 5000;
    ",
    )?;

    // Apply migrations
    schema::apply_migrations(&conn)?;

    // Image registry (v19): filled from the existing folder. The migration does not read the
    // disk; registration is idempotent. A failure never prevents startup and no
    // file is touched (stderr only: stdout is reserved for the MCP).
    if let Err(e) = crate::images::sync_registry(&conn, &app_dir.join("images")) {
        eprintln!("humcase: image registry not synchronized: {e}");
    }

    Ok(conn)
}

/// Transaction of a write path: `BEGIN IMMEDIATE` takes the write lock at once. The interface
/// and the MCP/CLI share the database from two processes in WAL mode; a deferred transaction
/// that reads and then writes fails at once (SQLITE_BUSY, `busy_timeout` does not apply) when
/// the other process committed in between, while an immediate one waits for the lock.
pub fn write_transaction(conn: &Connection) -> Result<Transaction<'_>> {
    Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
}

/// Consistent backup of the database to `destination`, on a **dedicated connection**.
///
/// `VACUUM INTO` produces a compacted, consistent copy (WAL journal included). Opening a
/// dedicated connection avoids holding the main connection's lock during the operation:
/// the interface does not freeze and the MCP does not wait for `busy_timeout`.
pub fn save_to(db_path: &Path, destination: &Path) -> Result<(), String> {
    let conn = Connection::open(db_path).map_err(|e| format!("Backup failed: {e}"))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| format!("Backup failed: {e}"))?;
    conn.execute(
        "VACUUM INTO ?1",
        rusqlite::params![destination.to_string_lossy()],
    )
    .map_err(|e| format!("Backup failed: {e}"))?;
    Ok(())
}
