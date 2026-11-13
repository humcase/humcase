use super::note_update::bump_revision;
use super::notes::map_note_row;
use crate::models::note::Note;
use rusqlite::{params, Connection, OptionalExtension, Result};

pub fn delete_note(conn: &Connection, id: &str) -> Result<(), String> {
    let deleted = conn
        .execute("DELETE FROM notes WHERE id = ?1", params![id])
        .map_err(crate::db::db_error)?;
    if deleted == 0 {
        // Inconsistent with `update_note`, which detects "not found": deleting a nonexistent id
        // must not be reported as a success.
        return Err(crate::tr!("notes.notFound", id = id));
    }
    Ok(())
}

/// Time spent in the trash before automatic purge (founder's decision).
pub const TRASH_RETENTION_DAYS: i64 = 30;

/// Places a note in the trash with its whole tree: every live sub-case, at any depth, goes with
/// it at the same instant, so the tree comes back whole. Nothing is erased: links, history and
/// images stay in place, and the content is frozen by the v17 trigger. The revision of each case
/// advances (write of the note, invariant 9), not `updated_at`. Returns how many cases went to
/// the trash (the case and its sub-cases).
pub fn trash_note(conn: &Connection, id: &str) -> Result<usize, String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let count = trash_tree_in(&tx, id)?;
    tx.commit().map_err(crate::db::db_error)?;
    Ok(count)
}

/// [`trash_note`] inside the caller's transaction (Sprout's Undo).
pub(crate) fn trash_tree_in(tx: &Connection, id: &str) -> Result<usize, String> {
    let tree = tree_of(
        tx,
        id,
        "deleted_at IS NULL",
        "child.deleted_at IS NULL",
        None,
    )?;
    if tree.is_empty() {
        return Err(match note_in_trash(tx, id)? {
            Some(true) => format!("note is already in the trash: {id}"),
            _ => crate::tr!("notes.notFound", id = id),
        });
    }
    let now: String = tx
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |r| {
            r.get(0)
        })
        .map_err(crate::db::db_error)?;
    for case_id in &tree {
        tx.execute(
            "UPDATE notes SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![case_id, now],
        )
        .map_err(crate::db::db_error)?;
        bump_revision(tx, case_id)?;
    }
    Ok(tree.len())
}

/// A case left as "New case" creates it: default or empty title, no text, nothing dictated, no
/// tag, priority, due date, branch or report, still open, a text case with nothing hanging from
/// it (sub-case, link, provenance, review, proposal, replacement, agent edit). The slightest
/// trace keeps it. `n` is the alias of the note.
fn empty_case_condition() -> String {
    let titles = super::notes::DEFAULT_TITLES
        .iter()
        .map(|title| format!("'{}'", title.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "n.deleted_at IS NULL AND n.archived_at IS NULL
         AND (trim(n.title) = '' OR n.title IN ({titles}))
         AND trim(n.content) = '' AND coalesce(trim(n.raw_content), '') = ''
         AND coalesce(trim(n.tags), '') IN ('', '[]')
         AND n.priority IS NULL AND n.due_at IS NULL AND n.git_branch IS NULL
         AND n.resolution_comment IS NULL AND n.status = 'open' AND n.content_kind = 'text'
         AND NOT EXISTS (SELECT 1 FROM notes child WHERE child.parent_id = n.id)
         AND NOT EXISTS (SELECT 1 FROM dictation_fragments f WHERE f.note_id = n.id)
         AND NOT EXISTS (SELECT 1 FROM note_task_links l WHERE l.source_note_id = n.id OR l.task_note_id = n.id)
         AND NOT EXISTS (SELECT 1 FROM case_provenance p WHERE p.note_id = n.id)
         AND NOT EXISTS (SELECT 1 FROM case_reviews r WHERE r.note_id = n.id)
         AND NOT EXISTS (SELECT 1 FROM proposals q WHERE q.note_id = n.id OR q.near_id = n.id)
         AND NOT EXISTS (SELECT 1 FROM case_supersessions s WHERE s.note_id = n.id OR s.by_id = n.id)
         AND NOT EXISTS (SELECT 1 FROM note_revisions v WHERE v.note_id = n.id)"
    )
}

/// Places a case in the trash only if it is still empty ([`empty_case_condition`]), checked in
/// the same transaction as the move, so a write that landed first always keeps it. Returns
/// whether it went to the trash (founder decision 05/10: a new case left empty does not stay).
pub fn trash_if_empty(conn: &Connection, id: &str) -> Result<bool, String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let empty: bool = tx
        .query_row(
            &format!(
                "SELECT EXISTS (SELECT 1 FROM notes n WHERE n.id = ?1 AND {})",
                empty_case_condition()
            ),
            params![id],
            |r| r.get(0),
        )
        .map_err(crate::db::db_error)?;
    if empty {
        trash_tree_in(&tx, id)?;
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(empty)
}

/// Minutes an empty case is left alone before the launch sweep takes it: the one being written
/// in another window, or just created, is never swept.
pub const EMPTY_CASE_GRACE_MINUTES: i64 = 60;

/// At launch, places in the trash the empty cases ([`empty_case_condition`]) not modified for
/// `older_than_minutes`, left behind by a crash or a window closed on them. Returns how many.
pub fn trash_empty_cases(conn: &Connection, older_than_minutes: i64) -> Result<usize, String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let ids = {
        let mut stmt = tx
            .prepare(&format!(
                "SELECT n.id FROM notes n WHERE {}
                   AND n.updated_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?1)",
                empty_case_condition()
            ))
            .map_err(crate::db::db_error)?;
        let rows = stmt
            .query_map(params![format!("-{older_than_minutes} minutes")], |r| {
                r.get::<_, String>(0)
            })
            .and_then(|rows| rows.collect::<Result<Vec<_>>>())
            .map_err(crate::db::db_error)?;
        rows
    };
    for id in &ids {
        trash_tree_in(&tx, id)?;
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(ids.len())
}

/// A case and its sub-cases at any depth that meet a condition, the case first. `root_filter`
/// applies to the case itself, `child_filter` (on the alias `child`) to each sub-case; `?2` is
/// `stamp` when given.
fn tree_of(
    conn: &Connection,
    id: &str,
    root_filter: &str,
    child_filter: &str,
    stamp: Option<&str>,
) -> Result<Vec<String>, String> {
    let sql = format!(
        "WITH RECURSIVE tree(id) AS (
             SELECT id FROM notes WHERE id = ?1 AND {root_filter}
             UNION
             SELECT child.id FROM notes child JOIN tree ON child.parent_id = tree.id
              WHERE {child_filter}
         )
         SELECT id FROM tree"
    );
    let mut stmt = conn.prepare(&sql).map_err(crate::db::db_error)?;
    let read = |r: &rusqlite::Row| r.get::<_, String>(0);
    match stamp {
        Some(stamp) => stmt
            .query_map(params![id, stamp], read)
            .and_then(|rows| rows.collect::<Result<Vec<_>>>()),
        None => stmt
            .query_map(params![id], read)
            .and_then(|rows| rows.collect::<Result<Vec<_>>>()),
    }
    .map_err(crate::db::db_error)
}

/// Takes a note out of the trash, identical (same project, links and history), with the
/// sub-cases that went to the trash with it (same instant); a sub-case trashed on its own before
/// stays there. A sub-case whose parent case is still in the trash is refused (v22): the parent
/// comes back first. Returns how many cases came back.
pub fn restore_from_trash(conn: &Connection, id: &str) -> Result<usize, String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let parent_in_trash: bool = tx
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM notes child JOIN notes parent ON parent.id = child.parent_id
                            WHERE child.id = ?1 AND parent.deleted_at IS NOT NULL)",
            [id],
            |r| r.get(0),
        )
        .map_err(crate::db::db_error)?;
    if parent_in_trash {
        return Err(crate::tr!("data.restoreParentFirst"));
    }
    let stamp: Option<String> = tx
        .query_row(
            "SELECT deleted_at FROM notes WHERE id = ?1 AND deleted_at IS NOT NULL",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(crate::db::db_error)?;
    let Some(stamp) = stamp else {
        return Err(match note_in_trash(&tx, id)? {
            Some(false) => format!("note is not in the trash: {id}"),
            _ => crate::tr!("notes.notFound", id = id),
        });
    };
    let tree = tree_of(
        &tx,
        id,
        "deleted_at IS NOT NULL",
        "child.deleted_at = ?2",
        Some(&stamp),
    )?;
    for case_id in &tree {
        tx.execute(
            "UPDATE notes SET deleted_at = NULL WHERE id = ?1",
            [case_id],
        )
        .map_err(crate::db::db_error)?;
        bump_revision(&tx, case_id)?;
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(tree.len())
}

/// `None`: note not found; `Some(true)`: in the trash.
fn note_in_trash(conn: &Connection, id: &str) -> Result<Option<bool>, String> {
    conn.query_row(
        "SELECT deleted_at IS NOT NULL FROM notes WHERE id = ?1",
        [id],
        |r| r.get::<_, bool>(0),
    )
    .optional()
    .map_err(crate::db::db_error)
}

/// Notes in the trash, most recently deleted first.
pub fn get_trashed_notes(conn: &Connection) -> Result<Vec<Note>> {
    let mut stmt = conn.prepare("SELECT n.id, n.project_id, n.title, n.content, n.note_type, n.status, n.resolution_comment, n.raw_content, n.priority, n.git_branch, n.tags, n.created_at, n.updated_at, n.due_at, n.revision, n.archived_at, n.deleted_at, n.parent_id, n.content_kind FROM notes n WHERE n.deleted_at IS NOT NULL ORDER BY n.deleted_at DESC")?;
    let iter = stmt.query_map([], map_note_row)?;
    iter.collect()
}

/// What the purge permanently erases.
pub enum TrashPurge<'a> {
    /// Notes in the trash for more than `n` days (automatic purge).
    OlderThanDays(i64),
    /// The cases of the trash the person was shown ("Empty the trash"): one trashed after the
    /// confirmation stays, to be shown first.
    Listed(&'a [String]),
    /// A single note, which must be in the trash ("Delete permanently").
    One(&'a str),
}

/// Permanently erases notes **from the trash** and keeps their minimal trace (identifier,
/// project, dates, no content) in `note_tombstones`, in a single transaction. Links and
/// history go by cascade. A live note is never touched. Returns the number of
/// notes erased. A purged case's sub-cases still in the trash become top-level cases
/// (trigger `sub_case_parent_purged`, v22) and are purged in their own time.
pub fn purge_trash(conn: &Connection, purge: TrashPurge) -> Result<usize, String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    // The cutoff is read once: computed in each statement, a note crossing it between the
    // tombstone and the deletion would be erased without a trace.
    let (condition, parameter): (&str, Option<String>) = match purge {
        TrashPurge::OlderThanDays(days) => (
            "deleted_at < ?1",
            Some(
                tx.query_row(
                    "SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?1)",
                    [format!("-{days} days")],
                    |r| r.get(0),
                )
                .map_err(crate::db::db_error)?,
            ),
        ),
        TrashPurge::Listed(ids) => (
            "id IN (SELECT value FROM json_each(?1))",
            Some(serde_json::to_string(ids).map_err(|e| e.to_string())?),
        ),
        TrashPurge::One(id) => ("id = ?1", Some(id.to_string())),
    };
    let purge_params: Vec<&dyn rusqlite::ToSql> = match &parameter {
        Some(value) => vec![value],
        None => vec![],
    };
    tx.execute(
        &format!(
            "INSERT OR REPLACE INTO note_tombstones (id, project_id, deleted_at, purged_at)
             SELECT id, project_id, deleted_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now') FROM notes
             WHERE deleted_at IS NOT NULL AND {condition}"
        ),
        purge_params.as_slice(),
    )
    .map_err(crate::db::db_error)?;
    let erased = tx
        .execute(
            &format!("DELETE FROM notes WHERE deleted_at IS NOT NULL AND {condition}"),
            purge_params.as_slice(),
        )
        .map_err(crate::db::db_error)?;
    if let TrashPurge::One(id) = purge {
        if erased == 0 {
            return Err(match note_in_trash(&tx, id)? {
                Some(false) => format!("note is not in the trash: {id}"),
                _ => crate::tr!("notes.notFound", id = id),
            });
        }
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(erased)
}
