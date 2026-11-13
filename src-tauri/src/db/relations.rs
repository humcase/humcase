//! Directed links between a thought and a task of the same project.
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::db::queries;
use crate::models::note::{CreateNoteInput, Note, NoteType};

#[derive(Debug, Serialize)]
pub struct NoteRelations {
    pub sources: Vec<Note>,
    pub tasks: Vec<Note>,
}

pub fn get_note_relations(conn: &Connection, note_id: &str) -> Result<NoteRelations, String> {
    queries::get_note_by_id(conn, note_id)
        .map_err(|_| crate::tr!("notes.notFound", id = note_id))?;
    let ids = |column: &str| -> Result<Vec<String>, String> {
        let sql = if column == "source" {
            "SELECT source_note_id FROM note_task_links WHERE task_note_id = ?1 ORDER BY created_at, source_note_id"
        } else {
            "SELECT task_note_id FROM note_task_links WHERE source_note_id = ?1 ORDER BY created_at, task_note_id"
        };
        conn.prepare(sql)
            .map_err(crate::db::db_error)?
            .query_map([note_id], |row| row.get(0))
            .map_err(crate::db::db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(crate::db::db_error)
    };
    // A linked note placed in the trash is no longer displayed; the link comes back with it.
    let live = |ids: Vec<String>| -> Result<Vec<Note>, String> {
        Ok(ids
            .iter()
            .map(|id| queries::get_note_by_id(conn, id).map_err(crate::db::db_error))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|n| n.deleted_at.is_none())
            .collect())
    };
    let sources = live(ids("source")?)?;
    let tasks = live(ids("task")?)?;
    Ok(NoteRelations { sources, tasks })
}

fn validate_new_link(
    conn: &Connection,
    source_note_id: &str,
    task_note_id: &str,
) -> Result<(), String> {
    if source_note_id == task_note_id {
        return Err(crate::tr!("data.selfLink"));
    }
    let source = queries::get_note_by_id(conn, source_note_id)
        .map_err(|_| crate::tr!("data.sourceNotFound", id = source_note_id))?;
    let task = queries::get_note_by_id(conn, task_note_id)
        .map_err(|_| crate::tr!("data.taskNotFound", id = task_note_id))?;
    if source.deleted_at.is_some() || task.deleted_at.is_some() {
        return Err(crate::tr!("data.trashedCannotLink"));
    }
    if source.project_id != task.project_id {
        return Err(crate::tr!("data.linkedSameProject"));
    }
    if task.note_type != NoteType::Todo {
        return Err(crate::tr!("data.targetMustBeTask"));
    }
    Ok(())
}

pub fn link_note_task(
    conn: &Connection,
    source_note_id: &str,
    task_note_id: &str,
) -> Result<(), String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM note_task_links WHERE source_note_id = ?1 AND task_note_id = ?2)",
        params![source_note_id, task_note_id], |row| row.get(0),
    ).map_err(crate::db::db_error)?;
    if exists {
        return Ok(());
    }
    validate_new_link(&tx, source_note_id, task_note_id)?;
    tx.execute(
        "INSERT INTO note_task_links (source_note_id, task_note_id) VALUES (?1, ?2)",
        params![source_note_id, task_note_id],
    )
    .map_err(crate::db::db_error)?;
    tx.commit().map_err(crate::db::db_error)
}

pub fn unlink_note_task(
    conn: &Connection,
    source_note_id: &str,
    task_note_id: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM note_task_links WHERE source_note_id = ?1 AND task_note_id = ?2",
        params![source_note_id, task_note_id],
    )
    .map_err(crate::db::db_error)?;
    Ok(())
}

pub fn create_linked_task(
    conn: &Connection,
    source_note_id: &str,
    title: &str,
    content: Option<&str>,
) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err(crate::tr!("data.taskTitleEmpty"));
    }
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let source = queries::get_note_by_id(&tx, source_note_id)
        .map_err(|_| crate::tr!("data.sourceNotFound", id = source_note_id))?;
    if source.deleted_at.is_some() {
        return Err(crate::tr!("data.trashedCannotReceiveTask"));
    }
    let id = queries::create_note(
        &tx,
        CreateNoteInput {
            project_id: source.project_id,
            title: title.to_string(),
            content: content.map(str::to_string),
            note_type: NoteType::Todo,
            priority: None,
            git_branch: None,
            tags: None,
            due_at: None,
            raw_content: None,
        },
    )?;
    tx.execute(
        "INSERT INTO note_task_links (source_note_id, task_note_id) VALUES (?1, ?2)",
        params![source_note_id, id],
    )
    .map_err(crate::db::db_error)?;
    tx.commit().map_err(crate::db::db_error)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        schema::apply_migrations(&conn).unwrap();
        conn
    }

    fn note(conn: &Connection, project: &str, title: &str, kind: NoteType) -> String {
        queries::create_note(
            conn,
            CreateNoteInput {
                project_id: project.into(),
                title: title.into(),
                content: None,
                note_type: kind,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn new_and_v11_databases_gain_links_without_losing_notes() {
        let conn = db();
        assert_eq!(
            conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            28
        );
        let p = queries::create_project(&conn, "Journal", None).unwrap();
        let id = note(&conn, &p, "A thought", NoteType::Idea);
        conn.execute_batch(
            "
            DROP TRIGGER note_with_links_cannot_move_project;
            DROP TRIGGER note_task_links_immutable;
            DROP TRIGGER note_task_links_no_cycles_insert;
            DROP TRIGGER note_task_links_same_project_insert;
            DROP TABLE note_task_links;
            PRAGMA user_version = 11;
        ",
        )
        .unwrap();
        schema::apply_migrations(&conn).unwrap();
        assert_eq!(
            queries::get_note_by_id(&conn, &id).unwrap().title,
            "A thought"
        );
        assert_eq!(
            conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            28
        );
        assert!(get_note_relations(&conn, &id).unwrap().tasks.is_empty());
        schema::apply_migrations(&conn).unwrap();
    }

    #[test]
    fn create_and_link_validate_project_type_cycles_and_are_idempotent() {
        let conn = db();
        let p = queries::create_project(&conn, "P", None).unwrap();
        let q = queries::create_project(&conn, "Q", None).unwrap();
        let source = note(&conn, &p, "Thought", NoteType::Idea);
        let other_project = note(&conn, &q, "Other task", NoteType::Todo);
        let not_task = note(&conn, &p, "Other thought", NoteType::Idea);
        assert!(link_note_task(&conn, &source, &other_project).is_err());
        assert!(link_note_task(&conn, &source, &not_task).is_err());
        assert!(link_note_task(&conn, &source, &source).is_err());
        assert!(create_linked_task(&conn, &source, "  ", None).is_err());
        let task = create_linked_task(
            &conn,
            &source,
            " Try it out ",
            Some("Not a copy of the thought"),
        )
        .unwrap();
        let created = queries::get_note_by_id(&conn, &task).unwrap();
        assert_eq!(created.title, "Try it out");
        assert_eq!(created.content, "Not a copy of the thought");
        assert_eq!(created.note_type, NoteType::Todo);
        assert_eq!(get_note_relations(&conn, &source).unwrap().tasks.len(), 1);
        assert_eq!(get_note_relations(&conn, &task).unwrap().sources.len(), 1);
        link_note_task(&conn, &source, &task).unwrap();
        assert_eq!(get_note_relations(&conn, &source).unwrap().tasks.len(), 1);
        assert!(link_note_task(&conn, &task, &source).is_err());
        let second_task = note(&conn, &p, "Second task", NoteType::Todo);
        link_note_task(&conn, &task, &second_task).unwrap();
        assert!(
            link_note_task(&conn, &second_task, &task).is_err(),
            "a real task cycle must be refused"
        );
        // The type can change after linking: the history stays visible.
        conn.execute("UPDATE notes SET note_type='note' WHERE id=?1", [&task])
            .unwrap();
        link_note_task(&conn, &source, &task).unwrap();
        assert_eq!(get_note_relations(&conn, &source).unwrap().tasks.len(), 1);
        unlink_note_task(&conn, &source, &task).unwrap();
        unlink_note_task(&conn, &source, &task).unwrap();
    }

    #[test]
    fn create_linked_task_rolls_back_note_if_link_fails() {
        let conn = db();
        let p = queries::create_project(&conn, "P", None).unwrap();
        let source = note(&conn, &p, "Thought", NoteType::Idea);
        conn.execute_batch("CREATE TRIGGER test_reject_link BEFORE INSERT ON note_task_links BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
        let before: i64 = conn
            .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert!(create_linked_task(&conn, &source, "Task", None).is_err());
        let after: i64 = conn
            .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn trash_keeps_links_hidden_until_restore_and_purge_leaves_a_tombstone() {
        let conn = db();
        let p = queries::create_project(&conn, "P", None).unwrap();
        let source = note(&conn, &p, "Thought", NoteType::Idea);
        let task = create_linked_task(&conn, &source, "Task", None).unwrap();
        let links = |conn: &Connection| {
            conn.query_row("SELECT count(*) FROM note_task_links", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
        };

        // In the trash: the link stays in the database but the note is no longer shown nor linkable.
        queries::trash_note(&conn, &source).unwrap();
        assert_eq!(links(&conn), 1);
        assert!(get_note_relations(&conn, &task).unwrap().sources.is_empty());
        assert!(link_note_task(&conn, &source, &note(&conn, &p, "Other", NoteType::Todo)).is_err());
        assert!(create_linked_task(&conn, &source, "New", None).is_err());

        // Restored: the link comes back identical.
        queries::restore_from_trash(&conn, &source).unwrap();
        assert_eq!(get_note_relations(&conn, &source).unwrap().tasks.len(), 1);

        // Purged: links erased by cascade, minimal trace kept.
        queries::trash_note(&conn, &source).unwrap();
        assert_eq!(
            queries::purge_trash(&conn, queries::TrashPurge::One(&source)).unwrap(),
            1
        );
        assert_eq!(links(&conn), 0);
        let trace: String = conn
            .query_row(
                "SELECT project_id FROM note_tombstones WHERE id = ?1",
                [&source],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(trace, p);
        assert!(queries::get_note_by_id(&conn, &task).is_ok());
    }

    #[test]
    fn linked_note_cannot_move_to_another_project() {
        let conn = db();
        let p = queries::create_project(&conn, "P", None).unwrap();
        let q = queries::create_project(&conn, "Q", None).unwrap();
        let source = note(&conn, &p, "Thought", NoteType::Idea);
        create_linked_task(&conn, &source, "Task", None).unwrap();
        assert!(conn
            .execute(
                "UPDATE notes SET project_id=?1 WHERE id=?2",
                params![q, source]
            )
            .is_err());
        assert_eq!(
            queries::get_note_by_id(&conn, &source).unwrap().project_id,
            p
        );
    }
}
