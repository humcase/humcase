use super::note_update::bump_revision;
use crate::models::note::{CreateNoteInput, Note};
use rusqlite::{params, Connection, OptionalExtension, Result};
use uuid::Uuid;

/// Cheap fingerprint of the workspace state: notes, projects, activities and
/// attachments. It changes as soon as one of these objects is created, deleted or written. The interface
/// compares it periodically to reload what another process (MCP agent, CLI, quick capture) has
/// modified.
///
/// - `notes`: every write advances `revision` (invariant 9); archiving does not touch
///   `updated_at`, the sum of revisions sees it.
/// - `projects`, `activities`: `revision` and `updated_at` (schema v18) advance on every write.
/// - `project_activities`: no version of its own. An attachment removed or added advances
///   both sides (v18): a swap with an equal count is seen through these revisions. The row count
///   is the safety net for a deletion that would forget them (the `rowid` is reassigned by SQLite, it
///   proves nothing).
///
/// A single aggregated query, without reading any content. The format is opaque: only equality matters.
pub fn notes_change_marker(conn: &Connection) -> Result<String> {
    conn.query_row(
        "SELECT
            (SELECT COUNT(*) || ':' || COALESCE(SUM(revision), 0) || ':' || COALESCE(MAX(updated_at), '') FROM notes),
            (SELECT COUNT(*) || ':' || COALESCE(SUM(revision), 0) || ':' || COALESCE(MAX(updated_at), '') FROM projects),
            (SELECT COUNT(*) || ':' || COALESCE(SUM(revision), 0) || ':' || COALESCE(MAX(updated_at), '') FROM activities),
            (SELECT COUNT(*) FROM project_activities),
            (SELECT COUNT(*) FROM case_reviews)",
        [],
        |row| Ok(format!(
            "{}|{}|{}|{}|{}",
            row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
        )),
    )
}

/// Archives (hides) or unarchives a note. Nothing is erased; the status does not change.
/// This is a write of the note: its revision advances (agents' optimistic check), but not
/// `updated_at`, so that archiving does not bring it back up among recent notes.
pub fn set_note_archived(conn: &Connection, id: &str, archived: bool) -> Result<(), String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let sql = if archived {
        "UPDATE notes SET archived_at = COALESCE(archived_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) WHERE id = ?1"
    } else {
        "UPDATE notes SET archived_at = NULL WHERE id = ?1"
    };
    if tx.execute(sql, [id]).map_err(crate::db::db_error)? == 0 {
        return Err(crate::tr!("notes.notFound", id = id));
    }
    bump_revision(&tx, id)?;
    tx.commit().map_err(crate::db::db_error)
}

pub fn get_notes(
    conn: &Connection,
    project_id: Option<&str>,
    status: Option<&str>,
    search_query: Option<&str>,
) -> Result<Vec<Note>> {
    get_notes_impl(conn, project_id, status, search_query, true, None)
}

/// Space (Work/Private) of the project a note belongs to.
///
/// `Ok(None)` when the note or its project does not exist, or when the project has no space
/// ("to be sorted"): the caller then treats it as no limit, as before this function existed.
pub fn note_project_space(conn: &Connection, note_id: &str) -> Result<Option<String>> {
    let space: Option<Option<String>> = conn
        .query_row(
            "SELECT p.space FROM notes n JOIN projects p ON p.id = n.project_id WHERE n.id = ?1",
            params![note_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(space.flatten())
}

/// Search **without tag syntax**: `#` is never interpreted as a filter.
///
/// Used by duplicate detection, which passes a note's full Markdown (headings
/// `### …` included): seeing tags there turned a Markdown heading into a `##` requirement and eliminated
/// notes that were in fact similar.
pub fn get_notes_without_tags(
    conn: &Connection,
    project_id: Option<&str>,
    status: Option<&str>,
    search_query: Option<&str>,
) -> Result<Vec<Note>> {
    get_notes_impl(conn, project_id, status, search_query, false, None)
}

/// Like `get_notes_without_tags`, but restricted to notes whose project carries `space`.
///
/// `space = None` = no restriction (historical behavior). Used by duplicate
/// detection: a Work space note must never suggest a Private space note.
pub fn get_notes_without_tags_in_space(
    conn: &Connection,
    project_id: Option<&str>,
    status: Option<&str>,
    search_query: Option<&str>,
    space: Option<&str>,
) -> Result<Vec<Note>> {
    get_notes_impl(conn, project_id, status, search_query, false, space)
}

/// Splits a search into its text (searched with `plan_word_search`) and the tags to filter on:
/// with `tags_allowed`, a word starting with `#` is a tag; otherwise the hash is dropped and the
/// rest is searched as text.
pub(super) fn split_search(query: &str, tags_allowed: bool) -> (String, Vec<String>) {
    let mut text_parts: Vec<String> = Vec::new();
    let mut tags = Vec::new();
    for word in query.split_whitespace() {
        if tags_allowed {
            if let Some(tag) = word.strip_prefix('#') {
                tags.push(tag.to_string());
                continue;
            }
        }
        let term = if tags_allowed {
            word
        } else {
            word.trim_start_matches('#')
        };
        if term.is_empty() {
            continue;
        }
        text_parts.push(term.to_string());
    }
    (text_parts.join(" "), tags)
}

fn get_notes_impl(
    conn: &Connection,
    project_id: Option<&str>,
    status: Option<&str>,
    search_query: Option<&str>,
    tags_allowed: bool,
    space_filter: Option<&str>,
) -> Result<Vec<Note>> {
    let mut query = String::from("SELECT n.id, n.project_id, n.title, n.content, n.note_type, n.status, n.resolution_comment, n.raw_content, n.priority, n.git_branch, n.tags, n.created_at, n.updated_at, n.due_at, n.revision, n.archived_at, n.deleted_at, n.parent_id, n.content_kind FROM notes n");

    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut param_index = 1;
    // A search returns its cases best first (`kept` is in that order).
    let mut relevance: Option<usize> = None;

    if let Some(q) = search_query {
        let q = q.trim();
        if !q.is_empty() {
            let (text_query, tags) = split_search(q, tags_allowed);

            if !text_query.is_empty() {
                match super::search::plan_word_search(conn, &text_query)? {
                    Some(plan) => {
                        query.push_str(&format!(
                            " WHERE n.rowid IN (SELECT value FROM json_each(?{}))",
                            param_index
                        ));
                        params.push(Box::new(plan.kept));
                        relevance = Some(param_index);
                        param_index += 1;
                    }
                    // Nothing but separators: nothing to find.
                    None => query.push_str(" WHERE 0"),
                }
            } else {
                query.push_str(" WHERE 1=1");
            }

            for tag in tags {
                query.push_str(&format!(
                    " AND EXISTS (SELECT 1 FROM json_each(n.tags) WHERE value = ?{})",
                    param_index
                ));
                params.push(Box::new(tag));
                param_index += 1;
            }
        } else {
            query.push_str(" WHERE 1=1");
        }
    } else {
        query.push_str(" WHERE 1=1");
    }

    // The trash appears in no list or search (see `get_trashed_notes`).
    query.push_str(" AND n.deleted_at IS NULL");

    if let Some(p_id) = project_id {
        query.push_str(&format!(" AND n.project_id = ?{}", param_index));
        params.push(Box::new(p_id.to_string()));
        param_index += 1;
    }

    // Space restriction: keeps only notes whose project carries exactly this space.
    // A "to be sorted" note (project without space) is therefore excluded as soon as a limit is requested;
    // a caller that has no space to impose passes `None` and gets the original behavior.
    if let Some(space) = space_filter {
        query.push_str(&format!(
            " AND n.project_id IN (SELECT id FROM projects WHERE space = ?{})",
            param_index
        ));
        params.push(Box::new(space.to_string()));
        param_index += 1;
    }

    if let Some(stat) = status {
        query.push_str(&format!(" AND n.status = ?{}", param_index));
        params.push(Box::new(stat.to_string()));
    }

    match relevance {
        Some(kept) => query.push_str(&format!(
            " ORDER BY (SELECT key FROM json_each(?{kept}) WHERE value = n.rowid)"
        )),
        None => query.push_str(" ORDER BY n.priority ASC NULLS LAST, n.created_at DESC"),
    }

    let mut stmt = conn.prepare(&query)?;

    let sql_params: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let iter = stmt.query_map(sql_params.as_slice(), map_note_row)?;

    let mut notes = Vec::new();
    for n in iter {
        // A corrupted row (e.g. unknown `note_type`) must not make the **whole** list
        // unusable: it is skipped with a trace, the others stay readable.
        match n {
            Ok(note) => notes.push(note),
            Err(e) => eprintln!("humcase: note skipped (unreadable row): {e}"),
        }
    }
    Ok(notes)
}

/// Validates that a `tags` string is indeed a **JSON array of strings** (IPC path).
///
/// The MCP already validates via `tags_from_json`; IPC accepted any string
/// (`"abc"`, `"[42]"`), which crashed rendering (a `toLowerCase()` on a number).
pub fn validate_tags_json(tags: &str) -> Result<(), String> {
    let parsed: serde_json::Value = serde_json::from_str(tags)
        .map_err(|_| "tags must be a JSON array of strings".to_string())?;
    match parsed {
        serde_json::Value::Array(items) if items.iter().all(|v| v.is_string()) => Ok(()),
        serde_json::Value::Array(_) => {
            Err("tags must be an array of strings (non-text element received)".to_string())
        }
        _ => Err("tags must be an array of strings".to_string()),
    }
}

pub fn get_note_by_id(conn: &Connection, id: &str) -> Result<Note> {
    conn.query_row(
        "SELECT id, project_id, title, content, note_type, status, resolution_comment, raw_content, priority, git_branch, tags, created_at, updated_at, due_at, revision, archived_at, deleted_at, parent_id, content_kind FROM notes WHERE id = ?1",
        params![id],
        map_note_row
    )
}

/// Validates a due date (`YYYY-MM-DD`; empty string = none). Shared by creation and
/// modification: no write path must accept a nonexistent day (`2026-02-31`).
pub fn validate_due_at(due_at: &str) -> Result<(), String> {
    let day = due_at.trim();
    if !day.is_empty() && !crate::dates::is_iso_day(day) {
        return Err(format!(
            "invalid due_at: {due_at} (expected a YYYY-MM-DD day)"
        ));
    }
    Ok(())
}

pub fn create_note(conn: &Connection, input: CreateNoteInput) -> Result<String, String> {
    // Same validation as `update_note` (before, creation accepted an impossible due date).
    if let Some(due_at) = &input.due_at {
        validate_due_at(due_at)?;
    }

    let id = Uuid::new_v4().to_string();
    let content = input.content.unwrap_or_default();

    let tags_val = input.tags.unwrap_or_else(|| "[]".to_string());
    // The IPC path must not store a non-conforming `tags` (the MCP already validates it).
    validate_tags_json(&tags_val)?;
    // "Empty string = no value" convention (like the due date): an empty priority must
    // not be stored as an empty string, otherwise the interface believes a priority exists.
    let priority = input.priority.filter(|p| !p.trim().is_empty());
    let due_at = input.due_at.filter(|d| !d.trim().is_empty());
    let raw_content = input.raw_content.filter(|r| !r.is_empty());
    // A note created with a raw dictation gets its first fragment (v20) in the same
    // transaction. Called from an existing transaction (note → task links), the
    // caller's is enough: SQLite refuses a nested transaction.
    let tx = if conn.is_autocommit() {
        Some(crate::db::write_transaction(conn).map_err(crate::db::db_error)?)
    } else {
        None
    };
    let conn: &Connection = tx.as_deref().unwrap_or(conn);
    conn.execute(
        "INSERT INTO notes (id, project_id, title, content, note_type, status, priority, git_branch, tags, due_at, raw_content, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7, ?8, ?9, ?10,
                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![id, input.project_id, input.title, content, input.note_type, priority, input.git_branch, tags_val, due_at, raw_content],
    )
    .map_err(crate::db::db_error)?;
    if let Some(raw) = &raw_content {
        insert_dictation_fragment(conn, &id, raw)?;
    }
    if let Some(tx) = tx {
        tx.commit().map_err(crate::db::db_error)?;
    }

    Ok(id)
}

/// Appends content to a note **atomically**, without a read-modify-write on the
/// caller's side.
///
/// A dictation processed while the note is no longer displayed must not overwrite a
/// concurrent write (MCP agent) on the same note: the `UPDATE` reads and writes the same row in a single
/// SQLite statement, so no race window exists. The title is only replaced if it is
/// still empty or left at a default title ([`DEFAULT_TITLES`]), as the interface would do.
/// Recognized default titles (all languages, old versions included); the interface keeps the
/// same list (`utils/title.ts`, a test compares them).
pub const DEFAULT_TITLES: [&str; 8] = [
    "Nouvelle note",
    "Nouvelle case",
    "New note",
    "New case",
    "Nuevo caso",
    "Neuer Fall",
    "Novo caso",
    "Nuovo caso",
];

pub fn append_note_content(
    conn: &Connection,
    id: &str,
    text: &str,
    auto_title: Option<&str>,
) -> Result<(), String> {
    let changed = conn
        .execute(
            "UPDATE notes SET
                content = CASE
                    WHEN content IS NULL OR content = '' THEN ?2
                    ELSE content || char(10) || char(10) || ?2
                END,
                title = CASE
                    WHEN ?3 IS NOT NULL AND (title IS NULL OR trim(title) = '' OR title IN (SELECT value FROM json_each(?4)))
                        THEN ?3
                    ELSE title
                END,
                revision = revision + 1,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![
                id,
                text,
                auto_title,
                serde_json::to_string(&DEFAULT_TITLES).map_err(|e| e.to_string())?
            ],
        )
        .map_err(crate::db::db_error)?;
    if changed == 0 {
        return Err(crate::tr!("notes.notFound", id = id));
    }
    Ok(())
}

/// Appends a raw dictation fragment in the same SQLite statement that reads the existing raw text.
/// An MCP write between two fragments can thus not be replaced by a stale read.
///
/// Since v20, the fragment is also recorded in `dictation_fragments` (append-only), in the
/// **same transaction** as the `raw_content` update: both succeed or neither does. The
/// update goes first to keep exactly the previous refusals (note not found,
/// trash trigger); a refusal cancels the transaction, no fragment remains.
pub fn append_raw_content(conn: &Connection, id: &str, text: &str) -> Result<(), String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let changed = tx
        .execute(
            "UPDATE notes SET
            raw_content = CASE
                WHEN raw_content IS NULL OR raw_content = '' THEN ?2
                ELSE raw_content || char(10) || char(10) || ?2
            END,
            revision = revision + 1,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1",
            params![id, text],
        )
        .map_err(crate::db::db_error)?;
    if changed == 0 {
        return Err(crate::tr!("notes.notFound", id = id));
    }
    insert_dictation_fragment(&tx, id, text)?;
    tx.commit().map_err(crate::db::db_error)
}

/// Records a dictation fragment (v20). To be called in the transaction that writes `raw_content`.
///
/// `created_at` is the current instant, but always **strictly later** (by at least 1 ms)
/// than the note's last fragment: two appends in the same millisecond, or a wall clock
/// that goes backwards, can never invert the read order (`created_at` then `id`).
fn insert_dictation_fragment(conn: &Connection, note_id: &str, text: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO dictation_fragments (id, note_id, text, created_at)
         VALUES (?1, ?2, ?3, max(
             strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ',
                 (SELECT max(created_at) FROM dictation_fragments WHERE note_id = ?2), '+0.001 seconds'), '')
         ))",
        params![Uuid::new_v4().to_string(), note_id, text],
    )
    .map_err(crate::db::db_error)?;
    Ok(())
}

/// Dictation fragments of a note `(id, text, created_at)`, in read order.
pub fn dictation_fragments(
    conn: &Connection,
    note_id: &str,
) -> Result<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, text, created_at FROM dictation_fragments WHERE note_id = ?1 ORDER BY created_at, id",
    )?;
    let rows = stmt.query_map([note_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    rows.collect()
}

/// Recomposes the raw text from the fragments, with **the same rule** as `append_raw_content`: a
/// text that follows a still-empty raw text is taken as is, otherwise it is preceded by two line breaks.
pub fn raw_content_from_fragments<'a>(texts: impl IntoIterator<Item = &'a str>) -> String {
    texts.into_iter().fold(String::new(), |mut raw, text| {
        if !raw.is_empty() {
            raw.push_str("\n\n");
        }
        raw.push_str(text);
        raw
    })
}

/// Identifiers of the notes whose fragments do not exactly recompose `raw_content` (NULL and
/// empty are equivalent). Archives use it: no archiving of an inconsistent database, and
/// refusal before COMMIT of a restore that would produce one.
pub fn notes_with_inconsistent_fragments(conn: &Connection) -> Result<Vec<String>> {
    let mut fragments: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT note_id, text FROM dictation_fragments ORDER BY note_id, created_at, id",
    )?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (note_id, text) = row?;
        fragments.entry(note_id).or_default().push(text);
    }
    let mut stmt = conn.prepare("SELECT id, COALESCE(raw_content, '') FROM notes ORDER BY id")?;
    let mut inconsistent = Vec::new();
    for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (id, raw) = row?;
        let texts = fragments.remove(&id).unwrap_or_default();
        if raw_content_from_fragments(texts.iter().map(String::as_str)) != raw {
            inconsistent.push(id);
        }
    }
    // Fragments without a note: `foreign_key_check` already refuses them; reported here too.
    inconsistent.extend(fragments.into_keys());
    Ok(inconsistent)
}

pub(super) fn map_note_row(row: &rusqlite::Row) -> Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        content: row.get(3)?,
        note_type: row.get(4)?,
        status: row.get(5)?,
        resolution_comment: row.get(6)?,
        raw_content: row.get(7)?,
        priority: row.get(8)?,
        git_branch: row.get(9)?,
        tags: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        due_at: row.get(13)?,
        revision: row.get(14)?,
        archived_at: row.get(15)?,
        deleted_at: row.get(16)?,
        parent_id: row.get(17)?,
        content_kind: row.get(18)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_due_at_accepts_empty_and_real_days_only() {
        assert!(validate_due_at("").is_ok());
        assert!(validate_due_at("2026-09-29").is_ok());
        assert!(validate_due_at("2026-02-31").is_err());
        assert!(validate_due_at("tomorrow").is_err());
        assert!(validate_due_at("2026-9-1").is_err());
    }
}
