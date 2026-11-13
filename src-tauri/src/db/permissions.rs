//! Permission for an MCP agent to edit notes, and the journal of attributed revisions.
//!
//! Two responsibilities deliberately kept together because they serve the same invariant: an agent
//! can only modify a note if the human explicitly allowed it, and every allowed modification
//! leaves an attributed trace (`source = 'agent'`) of the previous values.

use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::models::note::NoteRevision;

/// Key of the permission flag in `app_settings`. Absent = agent editing refused.
pub const MCP_ALLOW_NOTE_EDIT: &str = "mcp_allow_note_edit";

/// Reads a key from `app_settings`. `Ok(None)` = key absent (never an error).
pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(crate::db::db_error)
}

/// Writes (or replaces) a key in `app_settings`.
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(crate::db::db_error)?;
    Ok(())
}

/// Agent editing is only allowed on an explicitly enabled value (`"1"`).
/// Any other value (absent, `"0"`, unexpected content) means "refused".
pub fn allow_note_edit(conn: &Connection) -> Result<bool, String> {
    Ok(get_setting(conn, MCP_ALLOW_NOTE_EDIT)?.as_deref() == Some("1"))
}

/// Inserts the snapshot of the previous values of an agent write. `revision` is the **old**
/// version of the note (the one the agent had read). Called inside the write's transaction.
pub fn insert_note_revision(
    conn: &Connection,
    note_id: &str,
    revision: i64,
    source: &str,
    fields_json: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO note_revisions (id, note_id, revision, source, fields)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            Uuid::new_v4().to_string(),
            note_id,
            revision,
            source,
            fields_json
        ],
    )
    .map_err(crate::db::db_error)?;
    Ok(())
}

/// Revision history of a note, from most recent to oldest. Empty when the note
/// has never been edited by an agent.
pub fn note_history(conn: &Connection, note_id: &str) -> Result<Vec<NoteRevision>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, note_id, revision, source, fields, created_at FROM note_revisions
             WHERE note_id = ?1 ORDER BY revision DESC, created_at DESC",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map(params![note_id], |row| {
            Ok(NoteRevision {
                id: row.get(0)?,
                note_id: row.get(1)?,
                revision: row.get(2)?,
                source: row.get(3)?,
                fields: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(crate::db::db_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::db::db_error)
}

/// Agent revisions recorded since `since` (ISO 8601 UTC, bounds included), across all notes,
/// from oldest to most recent. Read-only: feeds the day timeline,
/// which only shows agent writes actually recorded. Capped at 500 entries.
pub fn revisions_since(conn: &Connection, since: &str) -> Result<Vec<NoteRevision>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, note_id, revision, source, fields, created_at FROM note_revisions
             WHERE created_at >= ?1 AND source = 'agent' ORDER BY created_at ASC, revision ASC LIMIT 500",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map(params![since], |row| {
            Ok(NoteRevision {
                id: row.get(0)?,
                note_id: row.get(1)?,
                revision: row.get(2)?,
                source: row.get(3)?,
                fields: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(crate::db::db_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::db::db_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{queries, schema};

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        schema::apply_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn setting_roundtrips_and_absent_defaults_to_refused() {
        let conn = db();
        assert_eq!(get_setting(&conn, MCP_ALLOW_NOTE_EDIT).unwrap(), None);
        assert!(!allow_note_edit(&conn).unwrap(), "absent = refused");

        set_setting(&conn, MCP_ALLOW_NOTE_EDIT, "1").unwrap();
        assert!(allow_note_edit(&conn).unwrap());

        // An unexpected value does not allow.
        set_setting(&conn, MCP_ALLOW_NOTE_EDIT, "true").unwrap();
        assert!(!allow_note_edit(&conn).unwrap());

        set_setting(&conn, MCP_ALLOW_NOTE_EDIT, "0").unwrap();
        assert!(!allow_note_edit(&conn).unwrap());
        assert_eq!(
            get_setting(&conn, MCP_ALLOW_NOTE_EDIT).unwrap().as_deref(),
            Some("0")
        );
    }

    #[test]
    fn history_is_attributed_and_ordered_most_recent_first() {
        let conn = db();
        let p = queries::create_project(&conn, "Journal", None).unwrap();
        let note = queries::create_note(
            &conn,
            crate::models::note::CreateNoteInput {
                project_id: p,
                title: "N".into(),
                content: None,
                note_type: crate::models::note::NoteType::Note,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: None,
            },
        )
        .unwrap();

        // No agent edit: empty history.
        assert!(note_history(&conn, &note).unwrap().is_empty());

        insert_note_revision(&conn, &note, 0, "agent", "{\"title\":\"Before\"}").unwrap();
        insert_note_revision(&conn, &note, 1, "agent", "{\"content\":\"v1\"}").unwrap();
        let history = note_history(&conn, &note).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].revision, 1, "most recent first");
        assert_eq!(history[1].revision, 0);
        assert!(history.iter().all(|entry| entry.source == "agent"));
        assert_eq!(history[1].fields, "{\"title\":\"Before\"}");
        // ISO 8601 UTC timestamp (invariant of migration v7): without it, the webview
        // would interpret "YYYY-MM-DD HH:MM:SS" as local time.
        assert!(
            history[0].created_at.contains('T') && history[0].created_at.ends_with('Z'),
            "created_at must be ISO 8601 UTC: {}",
            history[0].created_at
        );
    }

    #[test]
    fn history_cascades_when_note_is_deleted() {
        let conn = db();
        let p = queries::create_project(&conn, "Journal", None).unwrap();
        let note = queries::create_note(
            &conn,
            crate::models::note::CreateNoteInput {
                project_id: p,
                title: "N".into(),
                content: None,
                note_type: crate::models::note::NoteType::Note,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: None,
            },
        )
        .unwrap();
        insert_note_revision(&conn, &note, 0, "agent", "{}").unwrap();
        queries::delete_note(&conn, &note).unwrap();
        assert!(note_history(&conn, &note).unwrap().is_empty());
    }

    #[test]
    fn revisions_since_filters_by_timestamp_across_notes() {
        let conn = db();
        let p = queries::create_project(&conn, "Journal", None).unwrap();
        let make = |title: &str| {
            queries::create_note(
                &conn,
                crate::models::note::CreateNoteInput {
                    project_id: p.clone(),
                    title: title.into(),
                    content: None,
                    note_type: crate::models::note::NoteType::Note,
                    priority: None,
                    git_branch: None,
                    tags: None,
                    due_at: None,
                    raw_content: None,
                },
            )
            .unwrap()
        };
        let a = make("A");
        let b = make("B");
        insert_note_revision(&conn, &a, 0, "agent", "{}").unwrap();
        insert_note_revision(&conn, &b, 0, "agent", "{}").unwrap();
        insert_note_revision(&conn, &a, 1, "agent", "{}").unwrap();
        conn.execute("UPDATE note_revisions SET created_at = '2026-09-26T08:00:00.000Z' WHERE note_id = ?1 AND revision = 0", params![a]).unwrap();
        conn.execute(
            "UPDATE note_revisions SET created_at = '2026-09-27T09:30:00.000Z' WHERE note_id = ?1",
            params![b],
        )
        .unwrap();
        conn.execute("UPDATE note_revisions SET created_at = '2026-09-27T11:00:00.000Z' WHERE note_id = ?1 AND revision = 1", params![a]).unwrap();

        let today = revisions_since(&conn, "2026-09-27T00:00:00.000Z").unwrap();
        assert_eq!(today.len(), 2, "the previous day's revision is excluded");
        assert_eq!(today[0].note_id, b, "chronological order");
        assert_eq!(today[1].note_id, a);
        assert!(revisions_since(&conn, "2026-09-28T00:00:00.000Z")
            .unwrap()
            .is_empty());
    }
}
