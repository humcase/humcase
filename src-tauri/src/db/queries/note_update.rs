use super::notes::{get_note_by_id, validate_due_at, validate_tags_json};
use crate::models::note::{NoteType, UpdateNoteInput};
use rusqlite::{params, Connection, Result};
use serde_json::json;

#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryNoteUpdateResult {
    Applied,
    AlreadyApplied,
    Conflict,
    /// The note no longer exists: the draft remains to be recovered by hand. Explicit result rather than
    /// an error message the interface would have to recognize (translated text, hence fragile).
    NotFound,
}

pub fn update_note(conn: &Connection, id: &str, input: UpdateNoteInput) -> Result<(), String> {
    update_note_impl(conn, id, input, None, None).map(|_| ())
}

/// Writes a note **only if** the `resolution_comment` in the database is still the expected one.
///
/// Optimistic check requested by the MCP: an agent that read a report before a human
/// modified it must not silently overwrite that report. The comparison and the write
/// share the same transaction: no write takes place in case of conflict.
///
/// `expected = None` falls back to the behavior of `update_note` (CLI/IPC compatibility); an
/// empty string is treated as "no report".
pub fn update_note_if_report_unchanged(
    conn: &Connection,
    id: &str,
    input: UpdateNoteInput,
    expected: Option<&str>,
) -> Result<(), String> {
    update_note_impl(conn, id, input, None, expected).map(|_| ())
}

/// Replays a draft only if the targeted fields still have their reference value.
/// The comparison and all writes share a SQLite transaction: an MCP agent can no
/// longer modify a field between the check and the write. A conflict leaves the draft
/// intact; an already-applied value is recognized inside the same transaction.
pub fn retry_note_update(
    conn: &Connection,
    id: &str,
    input: UpdateNoteInput,
    base: serde_json::Value,
) -> Result<RetryNoteUpdateResult, String> {
    refuse_raw_content(&input)?;
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM notes WHERE id = ?1)",
            params![id],
            |r| r.get(0),
        )
        .map_err(crate::db::db_error)?;
    if !exists {
        return Ok(RetryNoteUpdateResult::NotFound);
    }
    update_note_impl(conn, id, input, Some(&base), None)
}

/// Raw dictation is never replaced (invariant 3, batch S6): `update_note`, its
/// conditional variant and the escrow replay refuse `raw_content`, before any read or write.
/// The escrow replays a dictation through `append_raw_content`, which also writes its fragment.
fn refuse_raw_content(input: &UpdateNoteInput) -> Result<(), String> {
    if input.raw_content.is_some() {
        return Err(
            "raw_content cannot be replaced; dictation is append-only (use append_raw_content)"
                .to_string(),
        );
    }
    Ok(())
}

fn compared_value(field: &str, value: &serde_json::Value) -> serde_json::Value {
    match field {
        // The editor uses an empty string for the absence of priority or due date.
        "priority" | "due_at" if value.is_null() => serde_json::Value::String(String::new()),
        // Tags are compared in the JSON form produced by `JSON.stringify(readTags(...))`.
        "tags" => {
            let tags = value
                .as_str()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok());
            serde_json::Value::String(serde_json::to_string(&tags.unwrap_or_default()).unwrap())
        }
        _ => value.clone(),
    }
}

/// Increments a note's concurrency version and returns the new value.
///
/// Single point called inside the transaction of every write: it guarantees that no write
/// path escapes the MCP's optimistic check (`expected_revision`).
pub fn bump_revision(conn: &Connection, id: &str) -> Result<i64, String> {
    conn.execute(
        "UPDATE notes SET revision = revision + 1 WHERE id = ?1",
        params![id],
    )
    .map_err(crate::db::db_error)?;
    conn.query_row(
        "SELECT revision FROM notes WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )
    .map_err(crate::db::db_error)
}

/// Applies the provided fields on `conn` (the caller already provides the transaction). Returns the
/// number of rows touched. Common path of all field writes of a note.
fn apply_note_fields(
    conn: &Connection,
    id: &str,
    input: &UpdateNoteInput,
) -> Result<usize, String> {
    let mut changed = 0usize;
    if let Some(status) = &input.status {
        changed += conn.execute(
            "UPDATE notes SET status = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![status, id],
        ).map_err(crate::db::db_error)?;
    }
    if let Some(content) = &input.content {
        let kind: crate::models::note::ContentKind = conn
            .query_row(
                "SELECT content_kind FROM notes WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(crate::db::db_error)?;
        crate::db::cases::check_content(kind, content)?;
        changed += conn.execute(
            "UPDATE notes SET content = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![content, id],
        ).map_err(crate::db::db_error)?;
    }
    if let Some(res) = &input.resolution_comment {
        changed += conn.execute(
            "UPDATE notes SET resolution_comment = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![res, id],
        ).map_err(crate::db::db_error)?;
    }
    if let Some(title) = &input.title {
        changed += conn.execute(
            "UPDATE notes SET title = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![title, id],
        ).map_err(crate::db::db_error)?;
    }
    if let Some(note_type) = &input.note_type {
        changed += conn.execute(
            "UPDATE notes SET note_type = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![note_type, id],
        ).map_err(crate::db::db_error)?;
    }
    // `raw_content` is never replaced here (see `refuse_raw_content`): only
    // `append_raw_content` writes it, fragment included.
    if let Some(tags) = &input.tags {
        changed += conn.execute(
            "UPDATE notes SET tags = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![tags, id],
        ).map_err(crate::db::db_error)?;
    }
    if let Some(priority) = &input.priority {
        // "Empty = removal" convention (like the due date): the interface sends '' for
        // "no priority". Without it, `priority: null` was ignored and the priority stayed
        // in the database although the save was announced.
        let value = priority.trim();
        let value = if value.is_empty() { None } else { Some(value) };
        changed += conn.execute(
            "UPDATE notes SET priority = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![value, id],
        ).map_err(crate::db::db_error)?;
    }
    if let Some(git_branch) = &input.git_branch {
        changed += conn.execute(
            "UPDATE notes SET git_branch = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![git_branch, id],
        ).map_err(crate::db::db_error)?;
    }
    if let Some(due_at) = &input.due_at {
        // Empty string = remove the due date (the UI sends '' when the date field is cleared).
        let value = due_at.trim();
        let value = if value.is_empty() { None } else { Some(value) };
        changed += conn.execute(
            "UPDATE notes SET due_at = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![value, id],
        ).map_err(crate::db::db_error)?;
    }
    Ok(changed)
}

fn update_note_impl(
    conn: &Connection,
    id: &str,
    mut input: UpdateNoteInput,
    base: Option<&serde_json::Value>,
    expected_resolution_comment: Option<&str>,
) -> Result<RetryNoteUpdateResult, String> {
    refuse_raw_content(&input)?;
    // Refuse a call without any field: otherwise no `UPDATE` is executed and the caller
    // (MCP, IPC) would nonetheless believe in a success.
    let has_field = input.status.is_some()
        || input.content.is_some()
        || input.resolution_comment.is_some()
        || input.title.is_some()
        || input.note_type.is_some()
        || input.priority.is_some()
        || input.git_branch.is_some()
        || input.tags.is_some()
        || input.due_at.is_some();
    if !has_field {
        return Err(crate::tr!("notes.noFieldToUpdate"));
    }

    // The due date is validated here too: the interface path does not go through the MCP, and a
    // nonexistent day ("2026-02-31") must be accepted by no write path.
    if let Some(due_at) = &input.due_at {
        validate_due_at(due_at)?;
    }
    // Same validation as creation: the IPC path must not store non-conforming tags.
    if let Some(tags) = &input.tags {
        validate_tags_json(tags)?;
    }

    // Transaction with a Drop guard: a `panic` (or an early return) between the start and the end
    // rolls back automatically instead of leaving the connection locked on a manual `BEGIN`.
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;

    // Optimistic check of the report: if the caller provided the version it read,
    // compare it to the database **in the same transaction** as the write. Any mismatch refuses
    // the call without writing anything (the transaction's `Drop` rolls back), which protects a
    // human report from an agent stuck on a stale base.
    if let Some(expected) = expected_resolution_comment {
        let stored: Option<String> = tx
            .query_row(
                "SELECT resolution_comment FROM notes WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|_| crate::tr!("notes.notFound", id = id))?;
        // An empty string and `NULL` denote the same absence of report; surrounding
        // whitespace is ignored on both sides (otherwise a text stored with blanks would produce
        // a permanent false conflict).
        let stored = stored
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty());
        let expected = expected.trim();
        let expected = if expected.is_empty() {
            None
        } else {
            Some(expected)
        };
        if stored.as_deref() != expected {
            return Err(crate::tr!("notes.resolutionChanged"));
        }
    }

    if let Some(base) = base {
        let current_note =
            get_note_by_id(&tx, id).map_err(|_| crate::tr!("notes.notFound", id = id))?;
        let current_note = serde_json::to_value(current_note).map_err(|e| e.to_string())?;
        let mut payload = serde_json::to_value(&input).map_err(|e| e.to_string())?;
        let base = base.as_object().ok_or("Invalid draft reference")?;
        let fields = payload.as_object().ok_or("Invalid draft payload")?.clone();
        let mut already_applied = Vec::new();
        for (field, wanted) in &fields {
            if wanted.is_null() {
                continue;
            }
            let Some(current_value) = current_note.get(field) else {
                return Ok(RetryNoteUpdateResult::Conflict);
            };
            if compared_value(field, wanted) == compared_value(field, current_value) {
                already_applied.push(field.clone());
                continue;
            }
            let Some(expected) = base.get(field) else {
                return Ok(RetryNoteUpdateResult::Conflict);
            };
            if compared_value(field, expected) != compared_value(field, current_value) {
                return Ok(RetryNoteUpdateResult::Conflict);
            }
        }
        for field in already_applied {
            payload
                .as_object_mut()
                .unwrap()
                .insert(field, serde_json::Value::Null);
        }
        if payload
            .as_object()
            .unwrap()
            .values()
            .all(serde_json::Value::is_null)
        {
            // All values are already in the database at the atomic moment of the check.
            return Ok(RetryNoteUpdateResult::AlreadyApplied);
        }
        input = serde_json::from_value(payload).map_err(|e| e.to_string())?;
    }

    let result = {
        // `Transaction` dereferences to `Connection`: existing queries go through the
        // transaction without being rewritten, and the block limits the borrow of `tx`.
        let conn = &*tx;
        (|| -> Result<usize, String> {
            let changed = apply_note_fields(conn, id, &input)?;
            // Every real write increments the version: the MCP's optimistic check
            // (`expected_revision`) thus covers **all** paths, the human app included.
            if changed > 0 {
                bump_revision(conn, id)?;
            }
            Ok(changed)
        })()
    };

    match result {
        // No row touched: no note carries this identifier (`rows_affected == 0`).
        // The transaction's `Drop` rolls back.
        Ok(0) => Err(crate::tr!("notes.notFound", id = id)),
        Ok(_) => tx
            .commit()
            .map(|_| RetryNoteUpdateResult::Applied)
            .map_err(crate::db::db_error),
        // The transaction's `Drop` rolls back, including after a panic.
        Err(e) => Err(e),
    }
}

/// Edit of a note by an **authorized MCP agent** (`update_note` tool).
///
/// Fields an agent may modify: only these. `raw_content` (append-only dictation
/// invariant), `project_id` (move via `move_note_to_project`), `created_at`, `id` and
/// `resolution_comment` (dedicated `update_note_status` tool) are not part of it.
#[derive(Debug, Clone, Default)]
pub struct AgentNoteEdit {
    pub title: Option<String>,
    pub content: Option<String>,
    pub note_type: Option<NoteType>,
    pub tags: Option<String>,
    pub priority: Option<String>,
    pub due_at: Option<String>,
}

impl AgentNoteEdit {
    fn has_field(&self) -> bool {
        self.title.is_some()
            || self.content.is_some()
            || self.note_type.is_some()
            || self.tags.is_some()
            || self.priority.is_some()
            || self.due_at.is_some()
    }
}

/// Applies an agent edit in **a single transaction**: version check, snapshot of the
/// previous values, field write and `revision` increment.
///
/// `expected_revision` is the old version read by the agent. Any mismatch (human write
/// in the meantime) refuses the call without writing anything and reminds of the current version, so the agent
/// can reload and replay. Returns the new version.
pub fn apply_agent_note_edit(
    conn: &Connection,
    id: &str,
    expected_revision: i64,
    edit: AgentNoteEdit,
) -> Result<i64, String> {
    if !edit.has_field() {
        return Err(crate::tr!("notes.noFieldToUpdate"));
    }
    // Argument validations shared with `create_note`, before any write.
    if let Some(title) = &edit.title {
        if title.trim().is_empty() {
            return Err(crate::tr!("notes.titleEmpty"));
        }
    }
    if let Some(due_at) = &edit.due_at {
        validate_due_at(due_at)?;
    }
    if let Some(tags) = &edit.tags {
        validate_tags_json(tags)?;
    }

    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let current = get_note_by_id(&tx, id).map_err(|_| crate::tr!("notes.notFound", id = id))?;
    if current.revision != expected_revision {
        // Actionable message: the agent knows the current version and can reload then replay.
        return Err(format!(
            "revision conflict: note is at revision {}, expected {expected_revision}; reload and retry",
            current.revision
        ));
    }

    // Snapshot of the **previous** values of the fields actually modified, before the write.
    // `raw_content` never enters this snapshot.
    let mut snapshot = serde_json::Map::new();
    if edit.title.is_some() {
        snapshot.insert("title".to_string(), json!(current.title));
    }
    if edit.content.is_some() {
        snapshot.insert("content".to_string(), json!(current.content));
    }
    if edit.note_type.is_some() {
        snapshot.insert(
            "note_type".to_string(),
            serde_json::to_value(current.note_type).map_err(|e| e.to_string())?,
        );
    }
    if edit.tags.is_some() {
        snapshot.insert("tags".to_string(), json!(current.tags));
    }
    if edit.priority.is_some() {
        snapshot.insert("priority".to_string(), json!(current.priority));
    }
    if edit.due_at.is_some() {
        snapshot.insert("due_at".to_string(), json!(current.due_at));
    }
    let snapshot = serde_json::Value::Object(snapshot).to_string();
    crate::db::permissions::insert_note_revision(&tx, id, current.revision, "agent", &snapshot)?;

    let input = UpdateNoteInput {
        status: None,
        content: edit.content,
        resolution_comment: None,
        title: edit.title,
        note_type: edit.note_type,
        raw_content: None,
        priority: edit.priority,
        git_branch: None,
        tags: edit.tags,
        due_at: edit.due_at,
    };
    let changed = apply_note_fields(&tx, id, &input)?;
    if changed == 0 {
        return Err(crate::tr!("notes.notFound", id = id));
    }
    let new_revision = bump_revision(&tx, id)?;
    tx.commit().map_err(crate::db::db_error)?;
    Ok(new_revision)
}
