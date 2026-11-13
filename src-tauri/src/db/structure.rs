//! Minimal context of a note and conditional application of a structuring proposal.
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::queries;
use crate::models::note::Note;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct StructureContext {
    pub space: Option<String>,
    pub project_name: Option<String>,
    pub activities: Vec<String>,
    pub is_inbox: bool,
}

/// Reads neither the content nor the metadata of another note. The technical name of the inbox is
/// never presented to the provider.
pub fn context_for_note(conn: &Connection, note_id: &str) -> Result<StructureContext, String> {
    let project: Option<(String, Option<String>, bool)> = conn.query_row(
        "SELECT p.name, p.space, p.is_inbox FROM notes n JOIN projects p ON p.id = n.project_id WHERE n.id = ?1",
        [note_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? != 0)),
    ).optional().map_err(crate::db::db_error)?;
    let (name, space, is_inbox) =
        project.ok_or_else(|| crate::tr!("notes.notFound", id = note_id))?;
    let mut activities = Vec::new();
    if !is_inbox {
        let mut stmt = conn.prepare(
            "SELECT a.name FROM notes n JOIN project_activities pa ON pa.project_id = n.project_id \
             JOIN activities a ON a.id = pa.activity_id WHERE n.id = ?1 ORDER BY a.name COLLATE NOCASE",
        ).map_err(crate::db::db_error)?;
        activities = stmt
            .query_map([note_id], |row| row.get(0))
            .map_err(crate::db::db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(crate::db::db_error)?;
    }
    Ok(StructureContext {
        space,
        project_name: (!is_inbox).then_some(name),
        activities,
        is_inbox,
    })
}

/// Origin of the previous versions recorded by a structuring in `note_revisions`.
pub const STRUCTURE_SOURCE: &str = "structure";

/// The version check, the archiving of the original and the replacement are indivisible.
///
/// Since 0.9.78, the replaced version is kept in the note's history
/// (`note_revisions`, source `structure`) and no longer copied at the end of the raw text:
/// restructuring several times otherwise stacked a full copy of each version. The raw text stays
/// reserved for dictation (never erased or rewritten here). Going back to the previous version
/// goes through this same function, in the other direction.
pub fn apply_structured_note(
    conn: &Connection,
    note_id: &str,
    expected_content: &str,
    content: &str,
) -> Result<Note, String> {
    if expected_content.trim().is_empty() || content.trim().is_empty() {
        return Err(crate::tr!("data.structureEmpty"));
    }
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let current = queries::get_note_by_id(&tx, note_id)
        .map_err(|_| crate::tr!("notes.notFound", id = note_id))?;
    if current.content != expected_content {
        return Err(crate::tr!("data.contentChanged"));
    }
    // A sketch is a drawing: structuring would replace it with text.
    if current.content_kind == crate::models::note::ContentKind::Sketch {
        return Err(crate::tr!("data.sketchNotStructured"));
    }
    if current.content == content {
        return Ok(current);
    }
    let rows = tx
        .execute(
            "UPDATE notes SET content = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ?2 AND content = ?3",
            params![content, note_id, expected_content],
        )
        .map_err(crate::db::db_error)?;
    // The CAS `content = ?4` may touch nothing if the note changed between the read and the write
    // (impossible in this transaction, but we do not infer it): then refuse clearly, so as
    // not to increment the revision of a note we believe we structured.
    if rows != 1 {
        return Err(crate::tr!("data.contentChanged"));
    }
    // Replaced version kept in the history, in the same transaction as the write.
    let previous = serde_json::json!({ "content": expected_content }).to_string();
    crate::db::permissions::insert_note_revision(
        &tx,
        note_id,
        current.revision,
        STRUCTURE_SOURCE,
        &previous,
    )?;
    // Structuring modifies the content: it is a write of the note, the version must advance
    // so as not to let an agent write on a stale base.
    queries::bump_revision(&tx, note_id)?;
    let updated = queries::get_note_by_id(&tx, note_id).map_err(crate::db::db_error)?;
    tx.commit().map_err(crate::db::db_error)?;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema;
    use crate::models::{
        note::{CreateNoteInput, NoteType, UpdateNoteInput},
        project::ProjectSpace,
    };

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        schema::apply_migrations(&conn).unwrap();
        conn
    }

    fn note(conn: &Connection, project_id: &str, content: &str) -> String {
        queries::create_note(
            conn,
            CreateNoteInput {
                project_id: project_id.into(),
                title: "Title".into(),
                content: Some(content.into()),
                note_type: NoteType::Idea,
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
    fn context_only_contains_selected_notes_project() {
        let conn = db();
        let p =
            queries::create_project_in_space(&conn, "Travel", None, Some(ProjectSpace::Private))
                .unwrap();
        let other =
            queries::create_project_in_space(&conn, "Secret", None, Some(ProjectSpace::Work))
                .unwrap();
        let travel =
            crate::db::activities::create_activity(&conn, "Trips", ProjectSpace::Private).unwrap();
        crate::db::activities::set_project_activities(&conn, &p, &[travel.id], None).unwrap();
        let id = note(&conn, &p, "Walk to Rome");
        note(&conn, &other, "Do not send");
        let context = context_for_note(&conn, &id).unwrap();
        let serialized = serde_json::to_string(&context).unwrap();
        assert_eq!(context.project_name.as_deref(), Some("Travel"));
        assert_eq!(context.space.as_deref(), Some("private"));
        assert_eq!(context.activities, vec!["Trips"]);
        assert!(!serialized.contains("Secret"));
        assert!(!serialized.contains("Do not send"));
        let inbox =
            crate::db::activities::get_or_create_inbox(&conn, ProjectSpace::Private).unwrap();
        let inbox_note = note(&conn, &inbox, "No filing");
        let context = context_for_note(&conn, &inbox_note).unwrap();
        assert!(context.is_inbox);
        assert!(context.project_name.is_none());
        assert!(context.activities.is_empty());
    }

    #[test]
    fn apply_keeps_original_and_other_fields_and_rejects_retry() {
        let conn = db();
        let p = queries::create_project(&conn, "Journal", None).unwrap();
        let id = note(&conn, &p, "Rough idea, maybe.");
        queries::append_raw_content(&conn, &id, "Pre-existing dictation").unwrap();
        let before = queries::get_note_by_id(&conn, &id).unwrap();
        let after = apply_structured_note(&conn, &id, &before.content, "An idea, maybe.").unwrap();
        assert_eq!(after.content, "An idea, maybe.");
        // The raw text (dictation) is neither rewritten nor completed; the original goes into the history.
        assert_eq!(after.raw_content.as_deref(), Some("Pre-existing dictation"));
        let history = crate::db::permissions::note_history(&conn, &id).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].source, STRUCTURE_SOURCE);
        assert_eq!(history[0].revision, before.revision);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&history[0].fields).unwrap()["content"],
            "Rough idea, maybe."
        );
        assert_eq!(after.project_id, before.project_id);
        assert_eq!(after.title, before.title);
        assert_eq!(after.note_type, before.note_type);
        assert_eq!(after.status, before.status);
        assert!(apply_structured_note(&conn, &id, &before.content, &after.content).is_err());
        assert_eq!(
            queries::get_note_by_id(&conn, &id).unwrap().raw_content,
            after.raw_content
        );
    }

    #[test]
    fn apply_rejects_conflict_without_writing_and_does_not_erase_source() {
        let conn = db();
        let p = queries::create_project(&conn, "Journal", None).unwrap();
        let id = note(&conn, &p, "Original");
        queries::update_note(
            &conn,
            &id,
            UpdateNoteInput {
                status: None,
                content: Some("Concurrent modification".into()),
                resolution_comment: None,
                title: None,
                note_type: None,
                raw_content: None,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
            },
        )
        .unwrap();
        let before = queries::get_note_by_id(&conn, &id).unwrap();
        queries::append_raw_content(&conn, &id, "New dictation fragment").unwrap();
        assert!(apply_structured_note(&conn, &id, "Original", "Proposal").is_err());
        let after = queries::get_note_by_id(&conn, &id).unwrap();
        assert_eq!(after.content, before.content);
        assert_eq!(after.raw_content.as_deref(), Some("New dictation fragment"));
        assert!(apply_structured_note(&conn, &id, " ", "Proposal").is_err());
        assert!(apply_structured_note(&conn, &id, "Original", " ").is_err());
    }

    #[test]
    fn apply_preserves_dictation_appended_after_proposal() {
        let conn = db();
        let p = queries::create_project(&conn, "Journal", None).unwrap();
        let id = note(&conn, &p, "Free text");
        // A dictation can arrive while the provider computes the proposal.
        queries::append_raw_content(&conn, &id, "Fragment that arrived later").unwrap();
        let after = apply_structured_note(&conn, &id, "Free text", "Structured text").unwrap();
        assert_eq!(
            after.raw_content.as_deref(),
            Some("Fragment that arrived later")
        );
    }

    /// Bug of 28/09/2026: five structurings stacked five copies in the raw text.
    #[test]
    fn restructuring_never_stacks_versions_in_raw_and_can_be_undone() {
        let conn = db();
        let p = queries::create_project(&conn, "Journal", None).unwrap();
        let id = note(&conn, &p, "v0");
        queries::append_raw_content(&conn, &id, "Dictation").unwrap();
        let mut current = "v0".to_string();
        for i in 1..=5 {
            let next = format!("v{i}");
            apply_structured_note(&conn, &id, &current, &next).unwrap();
            current = next;
        }
        let final_note = queries::get_note_by_id(&conn, &id).unwrap();
        assert_eq!(final_note.raw_content.as_deref(), Some("Dictation"));
        assert_eq!(
            crate::db::permissions::note_history(&conn, &id)
                .unwrap()
                .len(),
            5
        );
        // Undo = apply the previous version against the current version.
        let undone = apply_structured_note(&conn, &id, "v5", "v4").unwrap();
        assert_eq!(undone.content, "v4");
        assert_eq!(undone.raw_content.as_deref(), Some("Dictation"));
    }

    /// Structuring versions do not count as agent writes (day timeline).
    #[test]
    fn structure_history_is_not_reported_as_agent_writes() {
        let conn = db();
        let p = queries::create_project(&conn, "Journal", None).unwrap();
        let id = note(&conn, &p, "Raw");
        apply_structured_note(&conn, &id, "Raw", "Structured").unwrap();
        assert!(
            crate::db::permissions::revisions_since(&conn, "2000-01-01T00:00:00Z")
                .unwrap()
                .is_empty()
        );
    }
}
