use super::note_update::bump_revision;
use crate::models::project::{Project, ProjectSpace};
use rusqlite::{params, Connection, OptionalExtension, Result};
use uuid::Uuid;

pub fn get_projects(conn: &Connection) -> Result<Vec<Project>> {
    let mut stmt = conn.prepare("
        SELECT p.id, p.name, p.color, p.created_at, p.space, p.is_inbox, p.archived_at, p.updated_at, p.revision,
               SUM(CASE WHEN n.note_type = 'bug' AND n.status = 'open' AND n.archived_at IS NULL THEN 1 ELSE 0 END) as open_bugs,
               SUM(CASE WHEN n.note_type = 'todo' AND n.status = 'open' AND n.archived_at IS NULL THEN 1 ELSE 0 END) as open_todos
        FROM projects p
        LEFT JOIN notes n ON p.id = n.project_id AND n.deleted_at IS NULL
        GROUP BY p.id
        ORDER BY p.name ASC
    ")?;

    let iter = stmt.query_map([], |row| {
        Ok(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            created_at: row.get(3)?,
            space: match row.get::<_, Option<String>>(4)?.as_deref() {
                None => None,
                Some("private") => Some(ProjectSpace::Private),
                Some("work") => Some(ProjectSpace::Work),
                Some(value) => {
                    return Err(rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        format!("invalid project space: {value}").into(),
                    ))
                }
            },
            is_inbox: row.get::<_, i64>(5)? != 0,
            archived_at: row.get(6)?,
            updated_at: row.get(7)?,
            revision: row.get(8)?,
            open_bugs: row.get(9)?,
            open_todos: row.get(10)?,
        })
    })?;

    let mut projects = Vec::new();
    for p in iter {
        projects.push(p?);
    }

    Ok(projects)
}

/// Implicit destination project of the CLI (`humcase add` without `--project-id`).
///
/// Technical Inbox containers (`is_inbox = 1`, names `__humcase_inbox_…`) are excluded:
/// they are not filing destinations and, sorted by name, they came ahead of real
/// projects. The order stays that of [`get_projects`] (name ascending), hence the first ordinary project.
/// When only Inboxes exist, the caller must ask for an explicit destination.
pub fn default_project(conn: &Connection) -> Result<Project, String> {
    get_projects(conn)
        .map_err(crate::db::db_error)?
        .into_iter()
        .find(|project| !project.is_inbox)
        .ok_or_else(|| {
            "no destination project: create one in the UI, or pass --project-id".to_string()
        })
}

/// Advances a project's version (`revision`, `updated_at`): single point called inside the
/// transaction of every write that touches the project without going through an `UPDATE` of its fields
/// (attachments to activities). No effect if the project does not exist.
pub fn bump_project_revision(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE projects SET revision = revision + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
        [id],
    ).map_err(crate::db::db_error)?;
    Ok(())
}

pub fn create_project(conn: &Connection, name: &str, color: Option<&str>) -> Result<String> {
    create_project_in_space(conn, name, color, None)
}

pub fn create_project_in_space(
    conn: &Connection,
    name: &str,
    color: Option<&str>,
    space: Option<ProjectSpace>,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let col = color.unwrap_or("#3b82f6");

    // A single instant for `created_at` and `updated_at`; a new project's version is 0.
    conn.execute(
        "INSERT INTO projects (id, name, color, space, created_at, updated_at)
         SELECT ?1, ?2, ?3, ?4, t, t FROM (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS t)",
        params![id, name, col, space.map(ProjectSpace::as_str)],
    )?;

    Ok(id)
}

pub fn update_project_space(
    conn: &Connection,
    id: &str,
    space: Option<ProjectSpace>,
) -> Result<(), String> {
    let changed = conn
        .execute(
            "UPDATE projects SET space = ?1, revision = revision + 1,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![space.map(ProjectSpace::as_str), id],
        )
        .map_err(crate::db::db_error)?;
    if changed == 0 {
        return Err(crate::tr!("projects.notFound", id = id));
    }
    Ok(())
}

/// Ordinary project (not the Inbox): its space, or an explicit error.
fn ordinary_project_space(conn: &Connection, id: &str) -> Result<Option<String>, String> {
    let row: Option<(Option<String>, bool)> = conn
        .query_row(
            "SELECT space, is_inbox FROM projects WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)),
        )
        .optional()
        .map_err(crate::db::db_error)?;
    let (space, is_inbox) = row.ok_or_else(|| crate::tr!("projects.notFound", id = id))?;
    if is_inbox {
        return Err(crate::tr!("projects.inboxLocked"));
    }
    Ok(space)
}

/// Renames an ordinary project. The name is unique; the identifier, hence the notes and links,
/// does not change.
pub fn rename_project(conn: &Connection, id: &str, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(crate::tr!("projects.nameEmpty"));
    }
    if name.chars().count() > 120 {
        return Err(crate::tr!("projects.nameTooLong"));
    }
    ordinary_project_space(conn, id)?;
    let taken: bool = conn
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM projects WHERE name = ?1 AND id <> ?2)",
            params![name, id],
            |r| r.get(0),
        )
        .map_err(crate::db::db_error)?;
    if taken {
        return Err(crate::tr!("projects.duplicateName"));
    }
    conn.execute("UPDATE projects SET name = ?1, revision = revision + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2", params![name, id])
        .map_err(crate::db::db_error)?;
    Ok(())
}

/// Changes the color of an ordinary project (`#rrggbb`).
pub fn set_project_color(conn: &Connection, id: &str, color: &str) -> Result<(), String> {
    let valid = color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        return Err(crate::tr!("projects.invalidColor", color = color));
    }
    ordinary_project_space(conn, id)?;
    conn.execute("UPDATE projects SET color = ?1, revision = revision + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2", params![color.to_ascii_lowercase(), id])
        .map_err(crate::db::db_error)?;
    Ok(())
}

/// Archives (hides) or unarchives an ordinary project. Its notes are not modified.
pub fn set_project_archived(conn: &Connection, id: &str, archived: bool) -> Result<(), String> {
    ordinary_project_space(conn, id)?;
    let sql = if archived {
        "UPDATE projects SET archived_at = COALESCE(archived_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                revision = revision + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1"
    } else {
        "UPDATE projects SET archived_at = NULL, revision = revision + 1,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1"
    };
    conn.execute(sql, [id]).map_err(crate::db::db_error)?;
    Ok(())
}

/// Deletes an ordinary project **without erasing any note**: in a single transaction, its notes
/// join the Inbox of its space (note → task links restored identically, revision of
/// each note advanced), then the project is deleted. Returns the number of notes moved.
///
/// A project without a space is refused: its destination Inbox would be undetermined.
pub fn delete_project(conn: &Connection, id: &str) -> Result<usize, String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let space = match ordinary_project_space(&tx, id)?.as_deref() {
        Some("private") => ProjectSpace::Private,
        Some("work") => ProjectSpace::Work,
        _ => return Err(crate::tr!("projects.needsSpaceToDelete")),
    };
    let inbox = crate::db::activities::get_or_create_inbox(&tx, space)?;

    let note_ids: Vec<String> = {
        let mut stmt = tx
            .prepare("SELECT id FROM notes WHERE project_id = ?1")
            .map_err(crate::db::db_error)?;
        let ids = stmt
            .query_map([id], |r| r.get::<_, String>(0))
            .map_err(crate::db::db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(crate::db::db_error)?;
        ids
    };
    // A link joins two notes of the same project: all of them leave together. The trigger
    // `note_with_links_cannot_move_project` requires removing the links during the move.
    let links: Vec<(String, String, String)> = {
        let mut stmt = tx
            .prepare(
                "SELECT l.source_note_id, l.task_note_id, l.created_at FROM note_task_links l
             JOIN notes n ON n.id = l.source_note_id WHERE n.project_id = ?1",
            )
            .map_err(crate::db::db_error)?;
        let rows = stmt
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(crate::db::db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(crate::db::db_error)?;
        rows
    };
    for (source, task, _) in &links {
        tx.execute(
            "DELETE FROM note_task_links WHERE source_note_id = ?1 AND task_note_id = ?2",
            params![source, task],
        )
        .map_err(crate::db::db_error)?;
    }
    for note_id in &note_ids {
        tx.execute(
            "UPDATE notes SET project_id = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
            params![note_id, inbox],
        ).map_err(crate::db::db_error)?;
        bump_revision(&tx, note_id)?;
    }
    for (source, task, created_at) in &links {
        tx.execute(
            "INSERT INTO note_task_links (source_note_id, task_note_id, created_at) VALUES (?1, ?2, ?3)",
            params![source, task, created_at],
        ).map_err(crate::db::db_error)?;
    }
    // Deletion removes the project's attachments by cascade: every activity that
    // contained it changes, its version advances in the same transaction.
    let linked_activities: Vec<String> = {
        let mut stmt = tx
            .prepare("SELECT activity_id FROM project_activities WHERE project_id = ?1")
            .map_err(crate::db::db_error)?;
        let ids = stmt
            .query_map([id], |r| r.get::<_, String>(0))
            .map_err(crate::db::db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(crate::db::db_error)?;
        ids
    };
    for activity_id in &linked_activities {
        crate::db::activities::bump_activity_revision(&tx, activity_id)?;
    }
    tx.execute("DELETE FROM projects WHERE id = ?1", [id])
        .map_err(crate::db::db_error)?;
    tx.commit().map_err(crate::db::db_error)?;
    Ok(note_ids.len())
}
