use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::db::queries;
use crate::models::activity::Activity;
use crate::models::project::ProjectSpace;

fn activity_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(crate::tr!("activities.nameLength"));
    }
    Ok(name)
}

pub fn get_activities(conn: &Connection) -> Result<Vec<Activity>, String> {
    let mut stmt = conn.prepare("SELECT id, name, space, created_at, updated_at, revision FROM activities ORDER BY space, name COLLATE NOCASE")
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([], |row| {
            let space: String = row.get(2)?;
            let space = match space.as_str() {
                "private" => ProjectSpace::Private,
                "work" => ProjectSpace::Work,
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            Ok(Activity {
                id: row.get(0)?,
                name: row.get(1)?,
                space,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                revision: row.get(5)?,
                project_ids: Vec::new(),
            })
        })
        .map_err(crate::db::db_error)?;
    let mut activities = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::db::db_error)?;
    let mut links = conn
        .prepare(
            "SELECT project_id FROM project_activities WHERE activity_id = ?1 ORDER BY project_id",
        )
        .map_err(crate::db::db_error)?;
    for activity in &mut activities {
        activity.project_ids = links
            .query_map([&activity.id], |row| row.get(0))
            .map_err(crate::db::db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(crate::db::db_error)?;
    }
    Ok(activities)
}

/// Advances an activity's version (`revision`, `updated_at`): single point called inside the
/// transaction of every write that touches the activity without going through an `UPDATE` of its fields
/// (attachments, deletion of an attached project). No effect if the activity does not exist.
pub fn bump_activity_revision(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE activities SET revision = revision + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
        [id],
    ).map_err(crate::db::db_error)?;
    Ok(())
}

pub fn create_activity(
    conn: &Connection,
    name: &str,
    space: ProjectSpace,
) -> Result<Activity, String> {
    let name = activity_name(name)?;
    let id = Uuid::new_v4().to_string();
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    // A single instant for `created_at` and `updated_at`; a new activity's version is 0.
    tx.execute(
        "INSERT INTO activities (id, name, space, created_at, updated_at)
         SELECT ?1, ?2, ?3, t, t FROM (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS t)",
        params![id, name, space.as_str()],
    )
    .map_err(crate::db::db_error)?;
    let activity = get_activities(&tx)?
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| "new activity could not be read".to_string())?;
    tx.commit().map_err(crate::db::db_error)?;
    Ok(activity)
}

pub fn rename_activity(conn: &Connection, id: &str, name: &str) -> Result<Activity, String> {
    let name = activity_name(name)?;
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let changed = tx
        .execute(
            "UPDATE activities SET name = ?1, revision = revision + 1,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![name, id],
        )
        .map_err(crate::db::db_error)?;
    if changed == 0 {
        return Err(crate::tr!("activities.notFound", id = id));
    }
    let activity = get_activities(&tx)?
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| "renamed activity could not be read".to_string())?;
    tx.commit().map_err(crate::db::db_error)?;
    Ok(activity)
}

pub fn delete_activity(conn: &Connection, id: &str) -> Result<(), String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    // Deletion removes the activity's attachments by cascade: every project that
    // contained it changes, its version advances in the same transaction.
    let linked_projects: Vec<String> = {
        let mut stmt = tx
            .prepare("SELECT project_id FROM project_activities WHERE activity_id = ?1")
            .map_err(crate::db::db_error)?;
        let ids = stmt
            .query_map([id], |r| r.get::<_, String>(0))
            .map_err(crate::db::db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(crate::db::db_error)?;
        ids
    };
    for project_id in &linked_projects {
        queries::bump_project_revision(&tx, project_id)?;
    }
    let changed = tx
        .execute("DELETE FROM activities WHERE id = ?1", [id])
        .map_err(crate::db::db_error)?;
    if changed == 0 {
        return Err(crate::tr!("activities.notFound", id = id));
    }
    tx.commit().map_err(crate::db::db_error)
}

pub fn set_project_activities(
    conn: &Connection,
    project_id: &str,
    activity_ids: &[String],
    expected_activity_ids: Option<&[String]>,
) -> Result<(), String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let project: Option<(Option<String>, bool)> = tx
        .query_row(
            "SELECT space, is_inbox FROM projects WHERE id = ?1",
            [project_id],
            |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)),
        )
        .optional()
        .map_err(crate::db::db_error)?;
    let (space, is_inbox) =
        project.ok_or_else(|| crate::tr!("projects.notFound", id = project_id))?;
    if is_inbox {
        return Err(crate::tr!("activities.inboxHasNone"));
    }
    if let Some(expected) = expected_activity_ids {
        let mut stmt = tx
            .prepare("SELECT activity_id FROM project_activities WHERE project_id = ?1")
            .map_err(crate::db::db_error)?;
        let current = stmt
            .query_map([project_id], |r| r.get::<_, String>(0))
            .map_err(crate::db::db_error)?
            .collect::<rusqlite::Result<HashSet<_>>>()
            .map_err(crate::db::db_error)?;
        let expected: HashSet<&String> = expected.iter().collect();
        if current.len() != expected.len() || !current.iter().all(|id| expected.contains(id)) {
            return Err(crate::tr!("activities.changedExternally"));
        }
    }
    if !activity_ids.is_empty() && space.is_none() {
        return Err(crate::tr!("activities.needsSpace"));
    }
    let mut unique = HashSet::new();
    for id in activity_ids {
        if !unique.insert(id) {
            continue;
        }
        let activity_space: Option<String> = tx
            .query_row("SELECT space FROM activities WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(crate::db::db_error)?;
        match activity_space {
            None => return Err(crate::tr!("activities.notFound", id = id)),
            Some(value) if Some(value.as_str()) != space.as_deref() => {
                return Err(crate::tr!("activities.otherSpace", id = id))
            }
            Some(_) => {}
        }
    }
    // Only the difference is written: the project and the activities whose attachment changes
    // see their version advance (both sides), a save without changes advances nothing.
    let current: HashSet<String> = {
        let mut stmt = tx
            .prepare("SELECT activity_id FROM project_activities WHERE project_id = ?1")
            .map_err(crate::db::db_error)?;
        let ids = stmt
            .query_map([project_id], |r| r.get::<_, String>(0))
            .map_err(crate::db::db_error)?
            .collect::<rusqlite::Result<HashSet<_>>>()
            .map_err(crate::db::db_error)?;
        ids
    };
    let removed: Vec<&String> = current.iter().filter(|id| !unique.contains(*id)).collect();
    let added: Vec<&String> = unique
        .iter()
        .copied()
        .filter(|id| !current.contains(*id))
        .collect();
    if removed.is_empty() && added.is_empty() {
        return tx.commit().map_err(crate::db::db_error);
    }
    for id in &removed {
        tx.execute(
            "DELETE FROM project_activities WHERE project_id = ?1 AND activity_id = ?2",
            params![project_id, id],
        )
        .map_err(crate::db::db_error)?;
    }
    for id in &added {
        tx.execute(
            "INSERT INTO project_activities (project_id, activity_id) VALUES (?1, ?2)",
            params![project_id, id],
        )
        .map_err(crate::db::db_error)?;
    }
    queries::bump_project_revision(&tx, project_id)?;
    for id in removed.iter().chain(added.iter()) {
        bump_activity_revision(&tx, id)?;
    }
    tx.commit().map_err(crate::db::db_error)
}

pub fn get_or_create_inbox(conn: &Connection, space: ProjectSpace) -> Result<String, String> {
    let space = space.as_str();
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM projects WHERE is_inbox = 1 AND space = ?1",
            [space],
            |r| r.get(0),
        )
        .optional()
        .map_err(crate::db::db_error)?
    {
        return Ok(id);
    }
    // Stable semantic identity is the space; UUID in the internal name avoids collisions with
    // an existing user project. The UI displays a localized inbox label instead of this name.
    let id = Uuid::new_v4().to_string();
    let name = format!("__humcase_inbox_{space}_{id}");
    conn.execute(
        "INSERT OR IGNORE INTO projects (id, name, color, space, is_inbox, created_at, updated_at)
         SELECT ?1, ?2, '#64748b', ?3, 1, t, t FROM (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS t)",
        params![id, name, space],
    ).map_err(crate::db::db_error)?;
    conn.query_row(
        "SELECT id FROM projects WHERE is_inbox = 1 AND space = ?1",
        [space],
        |r| r.get(0),
    )
    .map_err(crate::db::db_error)
}

/// Identifiers of the connected component (**undirected** graph) of a note in the
/// note↔task links and the case tree (v22: parent ↔ sub-case), the note included. Both kinds of
/// edge join two notes of the same project: the component is thus the minimal unit that must
/// move together to preserve the common-project invariants.
fn component_ids(conn: &Connection, note_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE component(id) AS (
             SELECT ?1
             UNION
             SELECT links.task_note_id FROM note_task_links links
             JOIN component ON links.source_note_id = component.id
             UNION
             SELECT links.source_note_id FROM note_task_links links
             JOIN component ON links.task_note_id = component.id
             UNION
             SELECT notes.id FROM notes JOIN component ON notes.parent_id = component.id
             UNION
             SELECT notes.parent_id FROM notes JOIN component ON notes.id = component.id
             WHERE notes.parent_id IS NOT NULL
         )
         SELECT id FROM component",
        )
        .map_err(crate::db::db_error)?;
    let ids = stmt
        .query_map([note_id], |row| row.get::<_, String>(0))
        .map_err(crate::db::db_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::db::db_error);
    ids
}

/// Files a note from the Inbox into an ordinary project of the **same space**, with its
/// sub-cases (v22); a sub-case alone is refused, its top-level case is filed instead.
///
/// The trigger `note_with_links_cannot_move_project` forbids moving a linked note: it
/// deliberately stays in place. To file a captured note that already has linked tasks, the whole
/// connected component of links is moved in the **same transaction** (the note and its
/// tasks, sub-tasks included): the edges are temporarily removed, all notes
/// join the same project, then the edges are restored identically. The single transaction
/// (DEFERRED) guarantees atomicity by rollback: no intermediate state is committed, and
/// the "linked notes share a project" invariant is true before the commit. Protection against
/// concurrent writes relies on the connection's SQLite `busy_timeout` / lock, not on a
/// total serialization of readers.
pub fn move_note_to_project(
    conn: &Connection,
    note_id: &str,
    project_id: &str,
) -> Result<(), String> {
    file_inbox_note(conn, note_id, project_id, None)
}

/// Files a note from the Inbox into a project, and inside one of its cases when `parent_id` is
/// given, in **one** transaction: should the case refuse it, the note stays in the Inbox
/// instead of landing at the top of the project.
pub fn file_inbox_note(
    conn: &Connection,
    note_id: &str,
    project_id: &str,
    parent_id: Option<&str>,
) -> Result<(), String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    move_note_to_project_in(&tx, note_id, project_id)?;
    if let Some(parent_id) = parent_id {
        crate::db::cases::move_case_in(&tx, note_id, Some(parent_id))?;
    }
    tx.commit().map_err(crate::db::db_error)
}

fn move_note_to_project_in(tx: &Connection, note_id: &str, project_id: &str) -> Result<(), String> {
    let in_trash: Option<bool> = tx
        .query_row(
            "SELECT deleted_at IS NOT NULL FROM notes WHERE id = ?1",
            [note_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(crate::db::db_error)?;
    if in_trash == Some(true) {
        return Err(crate::tr!("data.trashedCannotFile"));
    }

    // A sub-case follows its parent: the top-level case is filed, with its whole tree.
    let parent: Option<String> = tx
        .query_row(
            "SELECT parent_id FROM notes WHERE id = ?1",
            [note_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(crate::db::db_error)?
        .flatten();
    if parent.is_some() {
        return Err(crate::tr!("data.subCaseFollowsParent"));
    }

    // 1) The source note must belong to an Inbox.
    let source: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT p.id, p.space FROM notes n JOIN projects p ON p.id = n.project_id
         WHERE n.id = ?1 AND p.is_inbox = 1",
            [note_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(crate::db::db_error)?;
    let (source_project, source_space) =
        source.ok_or_else(|| crate::tr!("data.onlyInboxCanBeFiled"))?;

    // 2) The target must be an ordinary project of the same space.
    let target: Option<(bool, Option<String>)> = tx
        .query_row(
            "SELECT is_inbox, space FROM projects WHERE id = ?1",
            [project_id],
            |r| Ok((r.get::<_, i64>(0)? != 0, r.get(1)?)),
        )
        .optional()
        .map_err(crate::db::db_error)?;
    let (target_is_inbox, target_space) =
        target.ok_or_else(|| crate::tr!("data.destinationNotFound"))?;
    if target_is_inbox || target_space != source_space {
        return Err(crate::tr!("data.destinationMustBeRegular"));
    }

    // 3) Connected component of links, then defensive check of the project invariant.
    let members = component_ids(tx, note_id)?;
    for member in &members {
        let project: String = tx
            .query_row(
                "SELECT project_id FROM notes WHERE id = ?1",
                [member],
                |r| r.get(0),
            )
            .map_err(|_| crate::tr!("notes.notFound", id = member))?;
        if project != source_project {
            return Err(crate::tr!("data.linkedOtherProject"));
        }
    }

    // 4) Snapshot of the internal edges: each link has its source in the component (closure).
    let mut links: Vec<(String, String, String)> = Vec::new();
    {
        let mut stmt = tx.prepare(
            "SELECT source_note_id, task_note_id, created_at FROM note_task_links WHERE source_note_id = ?1",
        ).map_err(crate::db::db_error)?;
        for member in &members {
            let rows = stmt
                .query_map([member], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(crate::db::db_error)?
                .collect::<rusqlite::Result<Vec<(String, String, String)>>>()
                .map_err(crate::db::db_error)?;
            links.extend(rows);
        }
    }

    // 5) Remove the edges (the protection trigger no longer blocks the move), move
    //    all notes of the component, then restore the edges identically.
    for (src, task, _) in &links {
        tx.execute(
            "DELETE FROM note_task_links WHERE source_note_id = ?1 AND task_note_id = ?2",
            params![src, task],
        )
        .map_err(crate::db::db_error)?;
    }
    for member in &members {
        tx.execute(
            "UPDATE notes SET project_id = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![member, project_id],
        )
        .map_err(crate::db::db_error)?;
        // A move is a write of the note: the version must advance so that the MCP's
        // optimistic check does not let a write through on a moved note.
        queries::bump_revision(tx, member)?;
    }
    for (src, task, created_at) in &links {
        tx.execute(
            "INSERT INTO note_task_links (source_note_id, task_note_id, created_at) VALUES (?1, ?2, ?3)",
            params![src, task, created_at],
        ).map_err(crate::db::db_error)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{queries, relations, schema};
    use crate::models::note::{CreateNoteInput, NoteType};

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        schema::apply_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn activities_link_multiple_projects_without_duplication_and_validate_atomically() {
        let conn = db();
        let p1 = queries::create_project_in_space(&conn, "Work 1", None, Some(ProjectSpace::Work))
            .unwrap();
        let p2 = queries::create_project_in_space(&conn, "Work 2", None, Some(ProjectSpace::Work))
            .unwrap();
        let private =
            queries::create_project_in_space(&conn, "Private", None, Some(ProjectSpace::Private))
                .unwrap();
        let a = create_activity(&conn, "Development", ProjectSpace::Work).unwrap();
        let b = create_activity(&conn, "Marketing", ProjectSpace::Work).unwrap();
        let wrong = create_activity(&conn, "Travel", ProjectSpace::Private).unwrap();
        set_project_activities(&conn, &p1, &[a.id.clone(), b.id.clone()], None).unwrap();
        set_project_activities(&conn, &p2, std::slice::from_ref(&a.id), None).unwrap();
        let stale_baseline = vec![a.id.clone()];
        let current_links = vec![a.id.clone(), b.id.clone()];
        assert!(set_project_activities(&conn, &p1, &[], Some(&stale_baseline)).is_err());
        assert!(set_project_activities(&conn, &p1, &current_links, Some(&current_links)).is_ok());
        let listed = get_activities(&conn).unwrap();
        assert_eq!(
            listed
                .iter()
                .find(|item| item.id == a.id)
                .unwrap()
                .project_ids
                .len(),
            2
        );
        assert_eq!(
            listed
                .iter()
                .find(|item| item.id == b.id)
                .unwrap()
                .project_ids,
            vec![p1.clone()]
        );
        assert!(set_project_activities(&conn, &p1, std::slice::from_ref(&wrong.id), None).is_err());
        assert_eq!(
            get_activities(&conn)
                .unwrap()
                .iter()
                .find(|item| item.id == b.id)
                .unwrap()
                .project_ids,
            vec![p1.clone()]
        );
        assert!(
            set_project_activities(&conn, &private, std::slice::from_ref(&a.id), None).is_err()
        );
        assert!(conn
            .execute(
                "INSERT INTO project_activities VALUES (?1, ?2)",
                params![private, a.id]
            )
            .is_err());

        queries::update_project_space(&conn, &p1, Some(ProjectSpace::Private)).unwrap();
        assert!(!get_activities(&conn)
            .unwrap()
            .iter()
            .any(|item| item.project_ids.contains(&p1)));
        rename_activity(&conn, &b.id, "Campaigns").unwrap();
        assert_eq!(
            get_activities(&conn)
                .unwrap()
                .iter()
                .find(|item| item.id == b.id)
                .unwrap()
                .name,
            "Campaigns"
        );
        delete_activity(&conn, &b.id).unwrap();
        assert!(get_activities(&conn)
            .unwrap()
            .iter()
            .all(|item| item.id != b.id));
        assert_eq!(queries::get_projects(&conn).unwrap().len(), 3);
    }

    #[test]
    fn inbox_is_lazy_unique_and_note_move_preserves_content() {
        let conn = db();
        assert!(queries::get_projects(&conn).unwrap().is_empty());
        let inbox = get_or_create_inbox(&conn, ProjectSpace::Work).unwrap();
        assert_eq!(
            get_or_create_inbox(&conn, ProjectSpace::Work).unwrap(),
            inbox
        );
        let project =
            queries::create_project_in_space(&conn, "Destination", None, Some(ProjectSpace::Work))
                .unwrap();
        let other = queries::create_project_in_space(
            &conn,
            "Other space",
            None,
            Some(ProjectSpace::Private),
        )
        .unwrap();
        let activity = create_activity(&conn, "Development", ProjectSpace::Work).unwrap();
        assert!(set_project_activities(&conn, &inbox, &[activity.id], None).is_err());
        assert!(queries::update_project_space(&conn, &inbox, None).is_err());
        assert!(conn
            .execute("UPDATE projects SET is_inbox = 0 WHERE id = ?1", [&inbox])
            .is_err());
        assert!(conn
            .execute("UPDATE projects SET is_inbox = 1 WHERE id = ?1", [&project])
            .is_err());
        let note_id = queries::create_note(
            &conn,
            CreateNoteInput {
                project_id: inbox.clone(),
                title: "Captured".into(),
                content: Some("Original text".into()),
                note_type: NoteType::Note,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: Some("Raw voice".into()),
            },
        )
        .unwrap();
        assert!(move_note_to_project(&conn, &note_id, &other).is_err());
        move_note_to_project(&conn, &note_id, &project).unwrap();
        let note = queries::get_note_by_id(&conn, &note_id).unwrap();
        assert_eq!(note.project_id, project);
        assert_eq!(note.content, "Original text");
        assert_eq!(note.raw_content.as_deref(), Some("Raw voice"));
        assert!(move_note_to_project(&conn, &note_id, &other).is_err());
    }

    #[test]
    fn move_note_with_linked_tasks_moves_the_whole_graph_in_one_transaction() {
        let conn = db();
        let inbox = get_or_create_inbox(&conn, ProjectSpace::Work).unwrap();
        let target =
            queries::create_project_in_space(&conn, "Destination", None, Some(ProjectSpace::Work))
                .unwrap();
        let other_space =
            queries::create_project_in_space(&conn, "Private", None, Some(ProjectSpace::Private))
                .unwrap();
        let source = queries::create_note(
            &conn,
            CreateNoteInput {
                project_id: inbox.clone(),
                title: "Thought in Inbox".into(),
                content: None,
                note_type: NoteType::Idea,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: None,
            },
        )
        .unwrap();
        let task = relations::create_linked_task(&conn, &source, "Linked task", None).unwrap();
        // Sub-task: the connected component must be moved as a whole, otherwise a link
        // would cross two projects (broken invariant).
        let sub_task = relations::create_linked_task(&conn, &task, "Sub-task", None).unwrap();

        // A project from another space is refused cleanly, without moving anything.
        assert!(move_note_to_project(&conn, &source, &other_space).is_err());
        assert_eq!(
            queries::get_note_by_id(&conn, &source).unwrap().project_id,
            inbox
        );

        move_note_to_project(&conn, &source, &target).unwrap();

        for id in [&source, &task, &sub_task] {
            assert_eq!(
                queries::get_note_by_id(&conn, id).unwrap().project_id,
                target,
                "{id}"
            );
        }
        // The links are intact (same endpoints) and the graph has no cycle.
        let source_relations = relations::get_note_relations(&conn, &source).unwrap();
        assert_eq!(
            source_relations
                .tasks
                .iter()
                .map(|n| n.id.clone())
                .collect::<Vec<_>>(),
            vec![task.clone()]
        );
        let task_relations = relations::get_note_relations(&conn, &task).unwrap();
        assert_eq!(
            task_relations
                .sources
                .iter()
                .map(|n| n.id.clone())
                .collect::<Vec<_>>(),
            vec![source.clone()]
        );
        assert_eq!(
            task_relations
                .tasks
                .iter()
                .map(|n| n.id.clone())
                .collect::<Vec<_>>(),
            vec![sub_task.clone()]
        );
        // A note that is no longer in the Inbox can no longer be filed this way.
        assert!(move_note_to_project(&conn, &source, &target).is_err());
    }

    #[test]
    fn migration_from_v10_preserves_existing_project_and_space() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE projects (
            id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, color TEXT, created_at TEXT,
            space TEXT CHECK (space IS NULL OR space IN ('private', 'work'))
        );
        CREATE TABLE notes (id TEXT PRIMARY KEY, project_id TEXT NOT NULL, title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '', resolution_comment TEXT, raw_content TEXT, note_type TEXT NOT NULL, status TEXT NOT NULL);
        INSERT INTO projects VALUES ('old', 'Existing', '#123456', '2026-09-24T00:00:00.000Z', 'work');
        INSERT INTO notes (id, project_id, note_type, status) VALUES ('note-old', 'old', 'idea', 'open');
        PRAGMA user_version = 10;").unwrap();
        schema::apply_migrations(&conn).unwrap();
        let project = queries::get_projects(&conn).unwrap();
        assert_eq!(project.len(), 1);
        assert_eq!(project[0].open_bugs, Some(0));
        let (name, space, is_inbox): (String, String, i64) = conn
            .query_row(
                "SELECT name, space, is_inbox FROM projects WHERE id = 'old'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            (name.as_str(), space.as_str(), is_inbox),
            ("Existing", "work", 0)
        );
        let note_project: String = conn
            .query_row(
                "SELECT project_id FROM notes WHERE id = 'note-old'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(note_project, "old");
        assert!(get_activities(&conn).unwrap().is_empty());
        schema::apply_migrations(&conn).unwrap();
    }
}
