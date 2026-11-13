// SQLite test database. The file is already declared as `mod tests` in `db/mod.rs`
// (no inner `mod tests`: clippy `module_inception`).
use crate::db::{permissions, queries, schema};
use crate::models::note::{CreateNoteInput, NoteStatus, NoteType, UpdateNoteInput};
use rusqlite::Connection;

#[test]
fn test_future_database_version_is_rejected_without_mutation() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE future_marker (id INTEGER PRIMARY KEY, value TEXT); INSERT INTO future_marker(value) VALUES ('keep'); PRAGMA user_version = 29;").unwrap();
    let error = schema::apply_migrations(&conn).unwrap_err().to_string();
    assert!(error.contains("newer than supported version 28"), "{error}");
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 29);
    let value: String = conn
        .query_row("SELECT value FROM future_marker", [], |r| r.get(0))
        .unwrap();
    assert_eq!(value, "keep");
    let legacy_tables: i64 = conn.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('projects','notes','note_task_links')", [], |r| r.get(0)).unwrap();
    assert_eq!(legacy_tables, 0);
}

#[test]
fn test_init_db_rejects_future_file_before_switching_to_wal() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("humcase.db");
    {
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch("CREATE TABLE future_marker (value TEXT); INSERT INTO future_marker VALUES ('keep'); PRAGMA user_version = 29;").unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "delete");
    }
    let before = std::fs::read(&db_path).unwrap();
    let error = crate::db::init_db(dir.path()).unwrap_err().to_string();
    assert!(error.contains("newer than supported version 28"), "{error}");
    assert_eq!(
        std::fs::read(&db_path).unwrap(),
        before,
        "the future file must not change"
    );
    assert!(!dir.path().join("humcase.db-wal").exists());
    assert!(!dir.path().join("humcase.db-shm").exists());
}

#[test]
fn test_human_report_retry_is_conditional_and_preserves_agent_report_and_status() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Report", None).unwrap();
    let id = queries::create_note(&conn, note_input(&project_id, "Task", NoteType::Todo)).unwrap();
    let report = |text: &str| UpdateNoteInput {
        resolution_comment: Some(text.into()),
        ..update_note_input(None, None, None, None)
    };
    let applied = queries::retry_note_update(
        &conn,
        &id,
        report("Trial done"),
        serde_json::json!({ "resolution_comment": null }),
    )
    .unwrap();
    assert_eq!(applied, queries::RetryNoteUpdateResult::Applied);
    queries::update_note(
        &conn,
        &id,
        UpdateNoteInput {
            status: Some(NoteStatus::InProgress),
            resolution_comment: Some("Agent: more recent result".into()),
            ..update_note_input(None, None, None, None)
        },
    )
    .unwrap();
    let conflict = queries::retry_note_update(
        &conn,
        &id,
        report("Local overwrite"),
        serde_json::json!({ "resolution_comment": "Trial done" }),
    )
    .unwrap();
    assert_eq!(conflict, queries::RetryNoteUpdateResult::Conflict);
    let note = queries::get_note_by_id(&conn, &id).unwrap();
    assert_eq!(
        note.resolution_comment.as_deref(),
        Some("Agent: more recent result")
    );
    assert_eq!(note.status, NoteStatus::InProgress);
}

/// P1-02 — an agent stuck on a stale base must not overwrite a human report.
///
/// Reproduction on **two real WAL connections** (`init_db`), like the application and an MCP
/// server side by side: the optimistic check and the write share the same SQLite transaction.
#[test]
fn test_conditional_report_write_refuses_stale_base() {
    let dir = std::env::temp_dir().join(format!("humcase-test-report-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let conn_human = crate::db::init_db(&dir).unwrap();
    let conn_agent = crate::db::init_db(&dir).unwrap();

    let project_id = queries::create_project(&conn_human, "Follow-up", None).unwrap();
    let id =
        queries::create_note(&conn_human, note_input(&project_id, "Task", NoteType::Todo)).unwrap();

    // The human writes their report.
    queries::update_note(
        &conn_human,
        &id,
        update_note_input(
            Some(NoteStatus::InProgress),
            None,
            None,
            Some("Human report"),
        ),
    )
    .unwrap();

    let agent_write =
        || update_note_input(Some(NoteStatus::Resolved), None, None, Some("Agent report"));

    // The agent, stuck on the old version, tries to write: explicit refusal.
    let refusal =
        queries::update_note_if_report_unchanged(&conn_agent, &id, agent_write(), Some("Old text"));
    assert_eq!(refusal.unwrap_err(), crate::tr!("notes.resolutionChanged"));

    // No partial write: neither the report nor the status moved.
    let note = queries::get_note_by_id(&conn_agent, &id).unwrap();
    assert_eq!(note.resolution_comment.as_deref(), Some("Human report"));
    assert_eq!(note.status, NoteStatus::InProgress);

    // With the right expected version, the write goes through normally.
    queries::update_note_if_report_unchanged(&conn_agent, &id, agent_write(), Some("Human report"))
        .unwrap();
    let note = queries::get_note_by_id(&conn_agent, &id).unwrap();
    assert_eq!(note.resolution_comment.as_deref(), Some("Agent report"));
    assert_eq!(note.status, NoteStatus::Resolved);

    // Without an expected version (`None`), the historical behavior is kept: direct write.
    queries::update_note(
        &conn_agent,
        &id,
        update_note_input(None, None, None, Some("Direct write")),
    )
    .unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn_agent, &id)
            .unwrap()
            .resolution_comment
            .as_deref(),
        Some("Direct write")
    );

    drop(conn_human);
    drop(conn_agent);
    let _ = std::fs::remove_dir_all(&dir);
}

/// F6 — the comparison of the expected report ignores surrounding whitespace on both sides.
#[test]
fn test_expected_report_ignores_surrounding_whitespace() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Trim", None).unwrap();
    let id = queries::create_note(&conn, note_input(&project_id, "N", NoteType::Todo)).unwrap();

    // The database holds a text padded with spaces; the expected value is the trimmed version.
    queries::update_note(
        &conn,
        &id,
        update_note_input(None, None, None, Some("  Report  ")),
    )
    .unwrap();
    queries::update_note_if_report_unchanged(
        &conn,
        &id,
        update_note_input(None, None, None, Some("Next")),
        Some("Report"),
    )
    .expect("the whitespace difference must not produce a false conflict");
    assert_eq!(
        queries::get_note_by_id(&conn, &id)
            .unwrap()
            .resolution_comment
            .as_deref(),
        Some("Next")
    );
}

/// P1-01 — duplicate detection never crosses the Work/Private boundary.
#[test]
fn test_similar_notes_stay_within_space_boundaries() {
    use crate::models::project::ProjectSpace;

    let conn = setup_test_db();
    let work =
        queries::create_project_in_space(&conn, "Work project", None, Some(ProjectSpace::Work))
            .unwrap();
    let private = queries::create_project_in_space(
        &conn,
        "Private project",
        None,
        Some(ProjectSpace::Private),
    )
    .unwrap();
    let no_space = queries::create_project(&conn, "To sort", None).unwrap();

    let mut a = note_input(&work, "Identical bug", NoteType::Note);
    a.content = Some("the identical crash bug reproduces".into());
    let mut b = note_input(&private, "Identical bug", NoteType::Note);
    b.content = Some("the identical crash bug reproduces".into());
    let mut c = note_input(&no_space, "Identical bug", NoteType::Note);
    c.content = Some("the identical crash bug reproduces".into());
    let id_work = queries::create_note(&conn, a).unwrap();
    let id_private = queries::create_note(&conn, b).unwrap();
    queries::create_note(&conn, c).unwrap();

    // The space is indeed derived from the source note's project.
    assert_eq!(
        queries::note_project_space(&conn, &id_work)
            .unwrap()
            .as_deref(),
        Some("work")
    );
    assert_eq!(
        queries::note_project_space(&conn, &id_private)
            .unwrap()
            .as_deref(),
        Some("private")
    );
    assert_eq!(queries::note_project_space(&conn, "unknown").unwrap(), None);

    let query = "identical bug";
    let in_work =
        queries::get_notes_without_tags_in_space(&conn, None, None, Some(query), Some("work"))
            .unwrap();
    assert!(!in_work.is_empty());
    assert!(
        in_work.iter().all(|n| n.project_id == work),
        "the Work space must only see the Work project"
    );

    let in_private =
        queries::get_notes_without_tags_in_space(&conn, None, None, Some(query), Some("private"))
            .unwrap();
    assert!(!in_private.is_empty());
    assert!(
        in_private.iter().all(|n| n.project_id == private),
        "the Private space must only see the Private project"
    );

    // Note without a space (no limit): all three notes are still found, behavior unchanged.
    let unrestricted =
        queries::get_notes_without_tags_in_space(&conn, None, None, Some(query), None).unwrap();
    assert_eq!(unrestricted.len(), 3);
}

#[test]
fn test_retry_note_update_refuses_intervening_change_without_partial_write() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Drafts", None).unwrap();
    let id = queries::create_note(
        &conn,
        note_input(&project_id, "Initial title", NoteType::Note),
    )
    .unwrap();

    // The draft knows two base values, then the MCP changes only the content.
    queries::update_note(
        &conn,
        &id,
        UpdateNoteInput {
            content: Some("MCP text".into()),
            ..update_note_input(None, None, None, None)
        },
    )
    .unwrap();
    let accepted = queries::retry_note_update(
        &conn,
        &id,
        UpdateNoteInput {
            content: Some("local draft".into()),
            title: Some("Local title".into()),
            ..update_note_input(None, None, None, None)
        },
        serde_json::json!({ "content": "", "title": "Initial title" }),
    )
    .unwrap();
    assert_eq!(accepted, queries::RetryNoteUpdateResult::Conflict);
    let note = queries::get_note_by_id(&conn, &id).unwrap();
    assert_eq!(note.content, "MCP text");
    assert_eq!(
        note.title, "Initial title",
        "no other field must be written"
    );

    // Without a conflict, both fields go through together.
    let accepted = queries::retry_note_update(
        &conn,
        &id,
        UpdateNoteInput {
            content: Some("local draft".into()),
            title: Some("Local title".into()),
            ..update_note_input(None, None, None, None)
        },
        serde_json::json!({ "content": "MCP text", "title": "Initial title" }),
    )
    .unwrap();
    assert_eq!(accepted, queries::RetryNoteUpdateResult::Applied);
    let note = queries::get_note_by_id(&conn, &id).unwrap();
    assert_eq!(note.content, "local draft");
    assert_eq!(note.title, "Local title");
}

#[test]
fn test_retry_note_update_checks_already_applied_inside_transaction() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Drafts", None).unwrap();
    let id = queries::create_note(&conn, note_input(&project_id, "Title", NoteType::Note)).unwrap();
    let draft = || UpdateNoteInput {
        content: Some("draft".into()),
        ..update_note_input(None, None, None, None)
    };

    queries::update_note(&conn, &id, draft()).unwrap();
    let already = queries::retry_note_update(&conn, &id, draft(), serde_json::json!({})).unwrap();
    assert_eq!(already, queries::RetryNoteUpdateResult::AlreadyApplied);

    // After a read that would have seen "draft", another writer replaces the field.
    queries::update_note(
        &conn,
        &id,
        UpdateNoteInput {
            content: Some("more recent MCP text".into()),
            ..update_note_input(None, None, None, None)
        },
    )
    .unwrap();
    let conflict =
        queries::retry_note_update(&conn, &id, draft(), serde_json::json!({ "content": "" }))
            .unwrap();
    assert_eq!(conflict, queries::RetryNoteUpdateResult::Conflict);
    assert_eq!(
        queries::get_note_by_id(&conn, &id).unwrap().content,
        "more recent MCP text"
    );
}

#[test]
fn test_retry_note_update_normalizes_empty_optional_fields_and_tags() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Drafts", None).unwrap();
    let id = queries::create_note(&conn, note_input(&project_id, "Title", NoteType::Note)).unwrap();
    let result = queries::retry_note_update(
        &conn,
        &id,
        UpdateNoteInput {
            priority: Some("P1".into()),
            due_at: Some("2026-12-31".into()),
            tags: Some("[\"alpha\"]".into()),
            ..update_note_input(None, None, None, None)
        },
        serde_json::json!({ "priority": "", "due_at": "", "tags": "[]" }),
    )
    .unwrap();
    assert_eq!(result, queries::RetryNoteUpdateResult::Applied);
    let note = queries::get_note_by_id(&conn, &id).unwrap();
    assert_eq!(note.priority.as_deref(), Some("P1"));
    assert_eq!(note.due_at.as_deref(), Some("2026-12-31"));
    assert_eq!(note.tags.as_deref(), Some("[\"alpha\"]"));

    let result = queries::retry_note_update(
        &conn,
        &id,
        UpdateNoteInput {
            priority: Some(String::new()),
            due_at: Some(String::new()),
            tags: Some("[]".into()),
            ..update_note_input(None, None, None, None)
        },
        serde_json::json!({ "priority": "P1", "due_at": "2026-12-31", "tags": "[\"alpha\"]" }),
    )
    .unwrap();
    assert_eq!(result, queries::RetryNoteUpdateResult::Applied);
    let note = queries::get_note_by_id(&conn, &id).unwrap();
    assert_eq!(note.priority, None);
    assert_eq!(note.due_at, None);
    assert_eq!(note.tags.as_deref(), Some("[]"));
}

#[test]
fn test_append_raw_content_keeps_existing_and_new_fragments() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Dictation", None).unwrap();
    let id = queries::create_note(&conn, note_input(&project_id, "Title", NoteType::Note)).unwrap();
    queries::append_raw_content(&conn, &id, "first fragment").unwrap();
    // Simulates a write that arrived between two interface calls (another process): the append reads
    // the current value in the database, not an old copy kept on the JavaScript side. Since v20, the raw text
    // is no longer replaced (`update_note` refuses it): the interleaved write is another append.
    queries::append_raw_content(&conn, &id, "text from another window").unwrap();
    queries::append_raw_content(&conn, &id, "last fragment").unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &id)
            .unwrap()
            .raw_content
            .as_deref(),
        Some("first fragment\n\ntext from another window\n\nlast fragment")
    );
    assert!(queries::append_raw_content(&conn, "missing note", "fragment").is_err());
}

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    schema::apply_migrations(&conn).unwrap();
    conn
}

/// P2-06 — the implicit destination of `humcase add` ignores technical Inboxes and fails
/// explicitly when no ordinary project exists.
#[test]
fn test_default_project_ignores_inboxes() {
    use crate::models::project::ProjectSpace;
    let conn = setup_test_db();
    let inbox_work = queries::get_or_create_inbox(&conn, ProjectSpace::Work).unwrap();
    let inbox_private = queries::get_or_create_inbox(&conn, ProjectSpace::Private).unwrap();

    // Only Inboxes: no implicit filing possible, explicit error.
    let error = queries::default_project(&conn).unwrap_err();
    assert!(error.contains("no destination project"), "{error}");

    // A real project (highest name outside the Inbox) is chosen, never a technical container.
    let project =
        queries::create_project_in_space(&conn, "zProject", None, Some(ProjectSpace::Work))
            .unwrap();
    let destination = queries::default_project(&conn).unwrap();
    assert_eq!(destination.id, project);
    assert_eq!(destination.name, "zProject");
    assert!(!destination.is_inbox);
    assert_ne!(destination.id, inbox_work);
    assert_ne!(destination.id, inbox_private);
}

fn note_input(project_id: &str, title: &str, note_type: NoteType) -> CreateNoteInput {
    CreateNoteInput {
        project_id: project_id.to_string(),
        title: title.to_string(),
        content: None,
        note_type,
        priority: None,
        git_branch: None,
        tags: None,
        due_at: None,
        raw_content: None,
    }
}

/// `2026-09-21T15:38:27.596Z`: ISO 8601, `T` separator, explicit UTC time zone (ticket B8).
fn assert_iso_utc(value: &str) {
    assert!(
        value.len() == 24 && value.ends_with('Z') && !value.contains(' '),
        "timestamp is not ISO 8601 UTC: {value:?}"
    );
    let bytes = value.as_bytes();
    for (idx, expected) in [
        (4, b'-'),
        (7, b'-'),
        (10, b'T'),
        (13, b':'),
        (16, b':'),
        (19, b'.'),
    ] {
        assert_eq!(
            bytes[idx], expected,
            "timestamp is not ISO 8601 UTC: {value:?}"
        );
    }
    assert!(
        value
            .split_at(19)
            .0
            .chars()
            .filter(char::is_ascii_digit)
            .count()
            == 14,
        "invalid date/time part: {value:?}"
    );
}

fn sqlite_now_prefix(conn: &Connection) -> String {
    // Same time source as the writes: the current minute must match.
    let now: String = conn
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M', 'now')", [], |r| r.get(0))
        .unwrap();
    now
}

#[test]
fn test_projects_and_notes_crud() {
    let conn = setup_test_db();

    // 1. Create a project
    let project_id = queries::create_project(&conn, "Test Project", Some("#ff0000")).unwrap();
    assert!(!project_id.is_empty());

    // 2. Fetch projects
    let projects = queries::get_projects(&conn).unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "Test Project");
    assert_eq!(projects[0].open_bugs.unwrap_or(0), 0);

    // 3. Create a bug note
    let note_id =
        queries::create_note(&conn, note_input(&project_id, "Test Bug", NoteType::Bug)).unwrap();
    assert!(!note_id.is_empty());

    // 4. Fetch projects again to check counters
    let projects = queries::get_projects(&conn).unwrap();
    assert_eq!(projects[0].open_bugs.unwrap_or(0), 1);

    // 5. Update note status
    queries::update_note(
        &conn,
        &note_id,
        UpdateNoteInput {
            status: Some(NoteStatus::Resolved),
            content: None,
            resolution_comment: Some("Fixed it!".to_string()),
            title: None,
            note_type: None,
            due_at: None,
            raw_content: None,
            priority: None,
            git_branch: None,
            tags: None,
        },
    )
    .unwrap();

    // 6. Fetch notes
    let notes = queries::get_notes(&conn, Some(&project_id), Some("resolved"), None).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].status, NoteStatus::Resolved);
    assert_eq!(notes[0].resolution_comment, Some("Fixed it!".to_string()));

    // 7. Check counter is back to 0
    let projects = queries::get_projects(&conn).unwrap();
    assert_eq!(projects[0].open_bugs.unwrap_or(0), 0);
}

#[test]
fn project_spaces_are_explicit_and_checked() {
    let conn = setup_test_db();
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);

    let legacy_style = queries::create_project(&conn, "Unassigned", None).unwrap();
    let private = queries::create_project_in_space(
        &conn,
        "Personal",
        None,
        Some(crate::models::project::ProjectSpace::Private),
    )
    .unwrap();
    let projects = queries::get_projects(&conn).unwrap();
    assert_eq!(
        projects
            .iter()
            .find(|p| p.id == legacy_style)
            .unwrap()
            .space,
        None
    );
    assert_eq!(
        projects.iter().find(|p| p.id == private).unwrap().space,
        Some(crate::models::project::ProjectSpace::Private)
    );

    queries::update_project_space(
        &conn,
        &legacy_style,
        Some(crate::models::project::ProjectSpace::Work),
    )
    .unwrap();
    queries::update_project_space(&conn, &private, None).unwrap();
    let projects = queries::get_projects(&conn).unwrap();
    assert_eq!(
        projects
            .iter()
            .find(|p| p.id == legacy_style)
            .unwrap()
            .space,
        Some(crate::models::project::ProjectSpace::Work)
    );
    assert_eq!(
        projects.iter().find(|p| p.id == private).unwrap().space,
        None
    );
    assert!(queries::update_project_space(&conn, "missing", None).is_err());
    assert!(conn
        .execute(
            "UPDATE projects SET space = 'unknown' WHERE id = ?1",
            [&legacy_style]
        )
        .is_err());
}

#[test]
fn test_delete_and_get_note() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Test Delete", None).unwrap();

    let note_id =
        queries::create_note(&conn, note_input(&project_id, "To Delete", NoteType::Note)).unwrap();

    // Test get_note_by_id
    let note = queries::get_note_by_id(&conn, &note_id).unwrap();
    assert_eq!(note.title, "To Delete");

    // Test delete
    queries::delete_note(&conn, &note_id).unwrap();

    // Verify deletion
    let notes = queries::get_notes(&conn, None, None, None).unwrap();
    assert_eq!(notes.len(), 0);

    let err = queries::get_note_by_id(&conn, &note_id).unwrap_err();
    assert!(
        err.to_string().contains("not found") || err.to_string().contains("Query returned no rows")
    );
}

#[test]
fn test_get_notes_filters() {
    let conn = setup_test_db();
    let p1 = queries::create_project(&conn, "P1", None).unwrap();
    let p2 = queries::create_project(&conn, "P2", None).unwrap();

    queries::create_note(&conn, note_input(&p1, "N1", NoteType::Bug)).unwrap();
    queries::create_note(&conn, note_input(&p2, "N2", NoteType::Idea)).unwrap();

    let notes_p1 = queries::get_notes(&conn, Some(&p1), None, None).unwrap();
    assert_eq!(notes_p1.len(), 1);
    assert_eq!(notes_p1[0].title, "N1");

    let notes_open = queries::get_notes(&conn, None, Some("open"), None).unwrap();
    assert_eq!(notes_open.len(), 2);
}

#[test]
fn test_fts5_special_chars() {
    let conn = setup_test_db();
    let p1 = queries::create_project(&conn, "P1", None).unwrap();

    let mut input = note_input(&p1, "Test FTS5", NoteType::Bug);
    input.content = Some("This is a bug\" and error(".to_string());
    queries::create_note(&conn, input).unwrap();

    // 1. Search with an unclosed quote
    let notes1 = queries::get_notes(&conn, None, None, Some("bug\"")).unwrap();
    assert_eq!(notes1.len(), 1);

    // 2. Search with a parenthesis
    let notes2 = queries::get_notes(&conn, None, None, Some("error(")).unwrap();
    assert_eq!(notes2.len(), 1);

    // 3. Multi-word search with special characters
    let notes3 = queries::get_notes(&conn, None, None, Some("bug\" error(")).unwrap();
    assert_eq!(notes3.len(), 1);
}

// ───────────────────────── P2-07 — the report is indexed by search ─────────────────────────

/// Fresh database: a term present **only** in the report must be found.
#[test]
fn test_search_finds_report_on_fresh_db() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Reports", None).unwrap();
    let id = queries::create_note(&conn, note_input(&p, "Task", NoteType::Todo)).unwrap();

    queries::update_note(
        &conn,
        &id,
        update_note_input(
            Some(NoteStatus::Resolved),
            None,
            None,
            Some("conclusion conclusive after audit"),
        ),
    )
    .unwrap();

    let found = queries::get_notes(&conn, None, None, Some("conclusive")).unwrap();
    assert_eq!(found.len(), 1, "a report term must be indexed");
    assert_eq!(found[0].id, id);

    // The title and content remain findable (no cross-column regression).
    assert_eq!(
        queries::get_notes(&conn, None, None, Some("Task"))
            .unwrap()
            .len(),
        1
    );
}

/// Migration of an "old" database (index without report): data is preserved and the
/// report of an existing note becomes findable after `apply_migrations`.
#[test]
fn test_migration_v13_indexes_existing_report() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Report migration", None).unwrap();
    let id = queries::create_note(&conn, note_input(&p, "Migrated task", NoteType::Todo)).unwrap();
    queries::update_note(
        &conn,
        &id,
        update_note_input(
            Some(NoteStatus::InProgress),
            None,
            None,
            Some("applied fix conclusive"),
        ),
    )
    .unwrap();

    // Simulates a database left at v12: FTS index limited to title/content/raw.
    conn.execute_batch("
        DROP TRIGGER IF EXISTS notes_ai;
        DROP TRIGGER IF EXISTS notes_ad;
        DROP TRIGGER IF EXISTS notes_au;
        DROP TABLE IF EXISTS notes_fts;
        CREATE VIRTUAL TABLE notes_fts USING fts5(title, content, raw_content, content='notes', content_rowid='rowid');
        CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
            INSERT INTO notes_fts(rowid, title, content, raw_content) VALUES (new.rowid, new.title, new.content, new.raw_content);
        END;
        CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, content, raw_content) VALUES('delete', old.rowid, old.title, old.content, old.raw_content);
        END;
        CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, content, raw_content) VALUES('delete', old.rowid, old.title, old.content, old.raw_content);
            INSERT INTO notes_fts(rowid, title, content, raw_content) VALUES (new.rowid, new.title, new.content, new.raw_content);
        END;
        INSERT INTO notes_fts(notes_fts) VALUES('rebuild');
        PRAGMA user_version = 12;
    ").unwrap();

    // Before the migration, the report term is not indexed.
    assert!(
        queries::get_notes(&conn, None, None, Some("conclusive"))
            .unwrap()
            .is_empty(),
        "the old index must not cover the report"
    );

    schema::apply_migrations(&conn).unwrap();

    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);

    // Data preserved.
    let note = queries::get_note_by_id(&conn, &id).unwrap();
    assert_eq!(note.title, "Migrated task");
    assert_eq!(
        note.resolution_comment.as_deref(),
        Some("applied fix conclusive")
    );

    // The report becomes findable thanks to the migration's rebuild.
    let found = queries::get_notes(&conn, None, None, Some("conclusive")).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, id);
}

/// The three FTS triggers cover the report: insertion, modification and deletion.
#[test]
fn test_fts_triggers_cover_report() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Triggers", None).unwrap();
    let id = queries::create_note(&conn, note_input(&p, "Task", NoteType::Todo)).unwrap();

    // `notes_au`: writing a report indexes it immediately.
    queries::update_note(
        &conn,
        &id,
        update_note_input(None, None, None, Some("first fix")),
    )
    .unwrap();
    assert_eq!(
        queries::get_notes(&conn, None, None, Some("fix"))
            .unwrap()
            .len(),
        1
    );

    // A new value replaces the old one in the index (no phantom duplicate).
    queries::update_note(
        &conn,
        &id,
        update_note_input(None, None, None, Some("second correction")),
    )
    .unwrap();
    assert!(queries::get_notes(&conn, None, None, Some("fix"))
        .unwrap()
        .is_empty());
    assert_eq!(
        queries::get_notes(&conn, None, None, Some("correction"))
            .unwrap()
            .len(),
        1
    );

    // `notes_ai`: a direct insert carrying a report is indexed.
    conn.execute(
        "INSERT INTO notes (id, project_id, title, note_type, status, resolution_comment)
         VALUES ('cr-direct', ?1, 'Direct', 'note', 'resolved', 'direct diagnostic')",
        rusqlite::params![p],
    )
    .unwrap();
    assert_eq!(
        queries::get_notes(&conn, None, None, Some("diagnostic"))
            .unwrap()
            .len(),
        1
    );

    // `notes_ad`: deleting the note removes its index entry.
    queries::delete_note(&conn, &id).unwrap();
    assert!(queries::get_notes(&conn, None, None, Some("correction"))
        .unwrap()
        .is_empty());
    conn.execute("DELETE FROM notes WHERE id = 'cr-direct'", [])
        .unwrap();
    assert!(queries::get_notes(&conn, None, None, Some("diagnostic"))
        .unwrap()
        .is_empty());
}

// ---------------------------------------------------------------------------
// B8 — Timestamps: explicit ISO 8601 UTC
// ---------------------------------------------------------------------------

#[test]
fn test_timestamps_are_iso_utc_on_insert_and_update() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Timestamps", None).unwrap();
    let note_id = queries::create_note(
        &conn,
        note_input(&project_id, "Timestamped note", NoteType::Note),
    )
    .unwrap();

    let created = queries::get_note_by_id(&conn, &note_id).unwrap();
    assert_iso_utc(&created.created_at);
    assert_iso_utc(&created.updated_at);
    assert_eq!(
        created.created_at.split_at(16).0,
        sqlite_now_prefix(&conn),
        "the stored timestamp must match the current UTC time"
    );

    queries::update_note(
        &conn,
        &note_id,
        UpdateNoteInput {
            status: Some(NoteStatus::InProgress),
            content: Some("content".to_string()),
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

    let updated = queries::get_note_by_id(&conn, &note_id).unwrap();
    assert_iso_utc(&updated.updated_at);
    assert_eq!(
        updated.created_at, created.created_at,
        "updated_at must not rewrite created_at"
    );
}

#[test]
fn test_notes_default_timestamp_is_iso_utc() {
    // The table's DEFAULT (and not an explicit INSERT) must produce the right format,
    // otherwise any new write path reintroduces the old space format.
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Default", None).unwrap();
    conn.execute(
        "INSERT INTO notes (id, project_id, title, note_type, status) VALUES ('raw-id', ?1, 'raw', 'note', 'open')",
        rusqlite::params![project_id],
    ).unwrap();

    let note = queries::get_note_by_id(&conn, "raw-id").unwrap();
    assert_iso_utc(&note.created_at);
    assert_iso_utc(&note.updated_at);
}

/// Representative "old" database (v6): timestamps in the `YYYY-MM-DD HH:MM:SS` format (UTC).
fn legacy_v6_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("
        CREATE TABLE projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            color TEXT DEFAULT '#3b82f6',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE notes (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            note_type TEXT NOT NULL,
            status TEXT NOT NULL,
            resolution_comment TEXT,
            raw_content TEXT,
            priority TEXT,
            git_branch TEXT,
            tags TEXT DEFAULT '[]',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        );
        CREATE INDEX idx_notes_project ON notes(project_id);
        CREATE INDEX idx_notes_status ON notes(status);
        CREATE VIRTUAL TABLE notes_fts USING fts5(title, content, raw_content, content='notes', content_rowid='rowid');
        CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
            INSERT INTO notes_fts(rowid, title, content, raw_content) VALUES (new.rowid, new.title, new.content, new.raw_content);
        END;
        CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, content, raw_content) VALUES('delete', old.rowid, old.title, old.content, old.raw_content);
        END;
        CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, content, raw_content) VALUES('delete', old.rowid, old.title, old.content, old.raw_content);
            INSERT INTO notes_fts(rowid, title, content, raw_content) VALUES (new.rowid, new.title, new.content, new.raw_content);
        END;

        INSERT INTO projects (id, name, created_at) VALUES ('p-legacy', 'Old project', '2026-09-20 20:12:42');
        INSERT INTO notes (id, project_id, title, content, note_type, status, priority, tags, created_at, updated_at)
            VALUES ('n-legacy', 'p-legacy', 'Historical bug', 'search content', 'bug', 'open', 'P1', '[\"audit\"]', '2026-09-20 20:12:42', '2026-09-21 09:05:10');
        INSERT INTO notes (id, project_id, title, content, note_type, status, created_at, updated_at)
            VALUES ('n-null', 'p-legacy', 'Note without timestamp', '', 'note', 'open', NULL, NULL);
        PRAGMA user_version = 6;
    ").unwrap();
    conn
}

#[test]
fn test_migration_v6_to_v7_converts_legacy_timestamps() {
    let conn = legacy_v6_db();
    schema::apply_migrations(&conn).unwrap();

    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    // The conversion is what this test checks: the current schema version may advance
    // (v8 = due dates), the absence of regression on timestamps remains the invariant.
    assert!(
        version >= 7,
        "the database must stay at v7 or more recent (current: {version})"
    );

    let note = queries::get_note_by_id(&conn, "n-legacy").unwrap();
    assert_eq!(note.created_at, "2026-09-20T20:12:42.000Z");
    assert_eq!(note.updated_at, "2026-09-21T09:05:10.000Z");
    // No data lost
    assert_eq!(note.title, "Historical bug");
    assert_eq!(note.content, "search content");
    assert_eq!(note.priority, Some("P1".to_string()));
    assert_eq!(note.tags, Some("[\"audit\"]".to_string()));

    // A NULL timestamp is replaced by the migration date (never an invalid value)
    let null_note = queries::get_note_by_id(&conn, "n-null").unwrap();
    assert_iso_utc(&null_note.created_at);
    assert_iso_utc(&null_note.updated_at);

    // Projects are normalized too
    let projects = queries::get_projects(&conn).unwrap();
    assert_eq!(projects[0].created_at, "2026-09-20T20:12:42.000Z");
    assert_eq!(
        projects[0].space, None,
        "an old project must stay unclassified"
    );

    // FTS index rebuilt: search must find the migrated note
    let found = queries::get_notes(&conn, None, None, Some("search")).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "n-legacy");

    // And the FTS triggers still work after the table rebuild
    queries::update_note(
        &conn,
        "n-legacy",
        UpdateNoteInput {
            title: Some("Historical bug renamed".to_string()),
            resolution_comment: None,
            status: None,
            content: None,
            note_type: None,
            raw_content: None,
            priority: None,
            git_branch: None,
            tags: None,
            due_at: None,
        },
    )
    .unwrap();
    assert_eq!(
        queries::get_notes(&conn, None, None, Some("renamed"))
            .unwrap()
            .len(),
        1
    );
    queries::delete_note(&conn, "n-legacy").unwrap();
    assert_eq!(
        queries::get_notes(&conn, None, None, Some("renamed"))
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn test_migrations_are_idempotent() {
    // Second pass: v7 must neither reconvert nor fail (NOT LIKE '%T%' guard).
    let conn = legacy_v6_db();
    schema::apply_migrations(&conn).unwrap();
    schema::apply_migrations(&conn).unwrap();
    let note = queries::get_note_by_id(&conn, "n-legacy").unwrap();
    assert_eq!(note.created_at, "2026-09-20T20:12:42.000Z");
    assert_eq!(note.updated_at, "2026-09-21T09:05:10.000Z");
    assert_eq!(
        queries::get_notes(&conn, None, None, None).unwrap().len(),
        2
    );
}

#[test]
fn test_daily_standup_uses_iso_boundaries() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Standup", None).unwrap();
    queries::create_note(&conn, note_input(&project_id, "Fresh", NoteType::Todo)).unwrap();

    // Old note (outside the 24 h) written directly
    conn.execute(
        "INSERT INTO notes (id, project_id, title, note_type, status, created_at, updated_at)
         VALUES ('old-note', ?1, 'Old', 'todo', 'open', '2020-01-01T08:00:00.000Z', '2020-01-01T08:00:00.000Z')",
        rusqlite::params![project_id],
    ).unwrap();

    let standup = queries::get_daily_standup(&conn, &project_id).unwrap();
    assert_eq!(standup.len(), 1);
    assert_eq!(standup[0].title, "Fresh");
}

/// Builds a partial `UpdateNoteInput` for the due date and status tests.
fn update_note_input(
    status: Option<NoteStatus>,
    priority: Option<&str>,
    due_at: Option<&str>,
    comment: Option<&str>,
) -> UpdateNoteInput {
    UpdateNoteInput {
        status,
        content: None,
        resolution_comment: comment.map(|c| c.to_string()),
        title: None,
        note_type: None,
        raw_content: None,
        priority: priority.map(|p| p.to_string()),
        git_branch: None,
        tags: None,
        due_at: due_at.map(|d| d.to_string()),
    }
}

// ───────────────────────── Due dates & handoff ("ideas" batch) ─────────────────────────

/// The v8 migration must add the `due_at` column to an already populated database without losing a note.
#[test]
fn test_migration_v8_adds_due_date_without_losing_note() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Migration", None).unwrap();
    let note_id = queries::create_note(
        &conn,
        note_input(&project_id, "Existing note", NoteType::Todo),
    )
    .unwrap();

    // Simulates a database left at v7: the column does not exist yet (nor the v17 trigger that cites it).
    conn.execute("DROP TRIGGER trashed_note_is_read_only", [])
        .unwrap();
    conn.execute("ALTER TABLE notes DROP COLUMN due_at", [])
        .unwrap();
    conn.execute("PRAGMA user_version = 7", []).unwrap();

    schema::apply_migrations(&conn).unwrap();

    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert!(
        version >= 8,
        "the migration must bring the database to v8 or more (current: {version})"
    );

    let columns: Vec<String> = conn
        .prepare("PRAGMA table_info(notes)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .map(|c| c.unwrap())
        .collect();
    assert!(
        columns.iter().any(|c| c == "due_at"),
        "due_at column missing: {columns:?}"
    );

    let note = queries::get_note_by_id(&conn, &note_id).unwrap();
    assert_eq!(note.title, "Existing note");
    assert!(note.due_at.is_none());
}

/// A due date is set, read, then cleared (empty string = field cleared in the interface).
#[test]
fn test_due_date_saved_then_cleared() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Due dates", None).unwrap();
    let note_id =
        queries::create_note(&conn, note_input(&project_id, "To finish", NoteType::Todo)).unwrap();

    queries::update_note(
        &conn,
        &note_id,
        update_note_input(None, None, Some("2026-12-31"), None),
    )
    .unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &note_id)
            .unwrap()
            .due_at
            .as_deref(),
        Some("2026-12-31")
    );

    // The date is also returned in the list (it travels with the note, no dedicated query).
    let list = queries::get_notes(&conn, Some(&project_id), None, None).unwrap();
    assert_eq!(list[0].due_at.as_deref(), Some("2026-12-31"));

    queries::update_note(
        &conn,
        &note_id,
        update_note_input(None, None, Some(""), None),
    )
    .unwrap();
    assert!(queries::get_note_by_id(&conn, &note_id)
        .unwrap()
        .due_at
        .is_none());
}

/// The handoff brief must reflect the database: notes to resume (with priority and real state),
/// recent resolutions, qualified due dates — and invent nothing (resolved notes do not
/// reappear in "To resume").
#[test]
fn test_handoff_contains_notes_to_resume_and_decisions() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Handoff", None).unwrap();

    let in_progress = queries::create_note(
        &conn,
        note_input(&project_id, "In-progress task", NoteType::Todo),
    )
    .unwrap();
    let open =
        queries::create_note(&conn, note_input(&project_id, "Open task", NoteType::Bug)).unwrap();
    let finished = queries::create_note(
        &conn,
        note_input(&project_id, "Finished task", NoteType::Bug),
    )
    .unwrap();

    queries::update_note(
        &conn,
        &in_progress,
        update_note_input(
            Some(NoteStatus::InProgress),
            Some("P1"),
            Some("2020-01-01"),
            None,
        ),
    )
    .unwrap();
    queries::update_note(
        &conn,
        &open,
        update_note_input(None, Some("P2"), None, None),
    )
    .unwrap();
    queries::update_note(
        &conn,
        &finished,
        update_note_input(
            Some(NoteStatus::Resolved),
            None,
            None,
            Some("Resolved with comment"),
        ),
    )
    .unwrap();

    // French locale output is asserted on purpose: labels come from the `fr` engine.json.
    let brief =
        crate::i18n::with_language("fr", || queries::build_handoff(&conn, Some(&project_id)))
            .unwrap();

    assert!(
        brief.contains("# Brief de session : Handoff"),
        "project title missing:\n{brief}"
    );
    // Every case line carries the case id in brackets, for an agent's next call.
    assert!(
        brief.contains(&format!(
            "**[P1]** In-progress task [{in_progress}] — en cours, tâche"
        )),
        "in-progress note line:\n{brief}"
    );
    assert!(
        brief.contains(&format!("**[P2]** Open task [{open}] — ouvert, incident")),
        "open note line:\n{brief}"
    );
    assert!(
        brief.contains("échéance 2020-01-01 (en retard)"),
        "due date not qualified:\n{brief}"
    );
    assert!(
        brief.contains(&format!(
            "Finished task [{finished}] → Resolved with comment"
        )),
        "decision missing:\n{brief}"
    );

    // "Nothing invented": the resolved note must not appear in the to-resume part.
    let to_resume = brief.split("## Dernières décisions").next().unwrap();
    assert!(
        !to_resume.contains("Finished task"),
        "a resolved note appears as to resume:\n{to_resume}"
    );
    assert_eq!(
        to_resume.matches("- **[").count(),
        2,
        "two notes expected to resume:\n{to_resume}"
    );

    // Same brief in English (language of the MCP and the CLI).
    let en = crate::i18n::with_language("en", || queries::build_handoff(&conn, Some(&project_id)))
        .unwrap();
    assert!(
        en.contains("in progress, task, due 2020-01-01 (overdue)"),
        "{en}"
    );
    assert!(en.contains("## Latest decisions (do not redo)"), "{en}");
}

// ───────────────────────── Priority, undo of a deletion, git branch ─────────────────────────

/// #6 — removing a priority: same convention as the due date, empty string = NULL.
#[test]
fn test_priority_removed_by_empty_string() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Priorities", None).unwrap();
    let note_id =
        queries::create_note(&conn, note_input(&project_id, "To sort", NoteType::Todo)).unwrap();

    queries::update_note(
        &conn,
        &note_id,
        update_note_input(None, Some("P1"), None, None),
    )
    .unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &note_id)
            .unwrap()
            .priority
            .as_deref(),
        Some("P1")
    );

    queries::update_note(
        &conn,
        &note_id,
        update_note_input(None, Some(""), None, None),
    )
    .unwrap();
    assert!(
        queries::get_note_by_id(&conn, &note_id)
            .unwrap()
            .priority
            .is_none(),
        "an empty priority must be removed, not kept"
    );

    // An empty priority at creation is not stored as an empty string.
    let mut input = note_input(&project_id, "Without priority", NoteType::Todo);
    input.priority = Some(String::new());
    let other = queries::create_note(&conn, input).unwrap();
    assert!(queries::get_note_by_id(&conn, &other)
        .unwrap()
        .priority
        .is_none());
}

/// #7 — undoing a deletion must restore the **whole** note: same identifier, due date and
/// raw dictation included, without going through a recreation that changes everything.
#[test]
fn test_creation_preserves_due_date_and_raw_dictation() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Creation", None).unwrap();

    let mut input = note_input(&project_id, "Dictation", NoteType::Note);
    input.due_at = Some("2026-10-05".into());
    input.raw_content = Some("raw text".into());
    let note_id = queries::create_note(&conn, input).unwrap();

    let note = queries::get_note_by_id(&conn, &note_id).unwrap();
    assert_eq!(note.due_at.as_deref(), Some("2026-10-05"));
    assert_eq!(note.raw_content.as_deref(), Some("raw text"));
}

/// #16 — the "last git branch seen" is that of the **most recently modified** note, not
/// the first of a list sorted by priority.
#[test]
fn test_handoff_reports_most_recently_modified_branch() {
    let conn = setup_test_db();
    let project_id = queries::create_project(&conn, "Branches", None).unwrap();

    let prioritized =
        queries::create_note(&conn, note_input(&project_id, "Old but P0", NoteType::Bug)).unwrap();
    let recent = queries::create_note(
        &conn,
        note_input(&project_id, "Recent but P3", NoteType::Todo),
    )
    .unwrap();

    queries::update_note(
        &conn,
        &prioritized,
        UpdateNoteInput {
            priority: Some("P0".into()),
            git_branch: Some("feature/old".into()),
            ..update_note_input(None, None, None, None)
        },
    )
    .unwrap();
    // Forced old timestamp: without it, both notes share the same millisecond.
    conn.execute(
        "UPDATE notes SET updated_at = '2020-01-01T00:00:00.000Z' WHERE id = ?1",
        rusqlite::params![prioritized],
    )
    .unwrap();

    queries::update_note(
        &conn,
        &recent,
        UpdateNoteInput {
            priority: Some("P3".into()),
            git_branch: Some("feature/recent".into()),
            ..update_note_input(None, None, None, None)
        },
    )
    .unwrap();

    let brief = queries::build_handoff(&conn, Some(&project_id)).unwrap();
    assert!(
        brief.contains("feature/recent"),
        "the most recent branch must be announced:\n{brief}"
    );
    assert!(
        !brief.contains("feature/old"),
        "an older branch must not be presented as current:\n{brief}"
    );
}

/// The in-memory helpers (`setup_test_db`) do not reproduce the production initialization:
/// neither `journal_mode=WAL`, nor `foreign_keys`, nor `busy_timeout`. This test goes through `init_db` on a
/// **temporary file** database (never the user's database) and checks the real behavior with
/// **two simultaneous connections**.
#[test]
fn test_production_pragmas_and_multi_connection() {
    let dir = std::env::temp_dir().join(format!("humcase-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let conn1 = crate::db::init_db(&dir).unwrap();

    let journal: String = conn1
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(journal.to_lowercase(), "wal", "production must use WAL");
    let foreign_keys: i64 = conn1
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(foreign_keys, 1, "foreign_keys must be enabled");
    let busy_timeout: i64 = conn1
        .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
        .unwrap();
    assert_eq!(busy_timeout, 5000, "busy_timeout must be applied");

    // Second connection on the same database (like the application and the MCP server side by side).
    let conn2 = crate::db::init_db(&dir).unwrap();

    // Write on one, read on the other: WAL must make the data visible immediately.
    let project_id = queries::create_project(&conn1, "WAL test", None).unwrap();
    let note_id = queries::create_note(
        &conn1,
        note_input(&project_id, "From conn1", NoteType::Note),
    )
    .unwrap();
    let notes = queries::get_notes(&conn2, Some(&project_id), None, None).unwrap();
    assert_eq!(
        notes.len(),
        1,
        "the second connection must see the first one's write"
    );
    assert_eq!(notes[0].id, note_id);

    // `foreign_keys` is actually applied on **every** connection: an orphan note is refused.
    let orphan = queries::create_note(
        &conn2,
        note_input("nonexistent-project", "Orphan", NoteType::Note),
    );
    assert!(
        orphan.is_err(),
        "foreign_keys not applied: orphan note accepted"
    );

    drop(conn1);
    drop(conn2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Tour of 08/10 — a case is similar when it shares two words of the text (the only one when
/// the text has one), common words ("pour") left out: "pour" alone or "vendredi" alone is not a
/// duplicate.
#[test]
fn test_find_similar_skips_common_words_and_needs_two_shared_words() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "House", None).unwrap();
    let mut kitchen = note_input(
        &p,
        "Préparer le devis pour la rénovation de la cuisine",
        NoteType::Note,
    );
    kitchen.content = Some("Mesurer la pièce avant vendredi.".into());
    let mut plumber = note_input(&p, "Trois plombiers disponibles", NoteType::Note);
    plumber.content = Some("Dupont passe jeudi pour la fuite.".into());
    queries::create_note(&conn, kitchen).unwrap();
    let plumber_id = queries::create_note(&conn, plumber).unwrap();

    let found = |text: &str| {
        queries::find_similar_notes(&conn, text, None, None, queries::SIMILAR_MAX)
            .unwrap()
            .into_iter()
            .map(|case| case.id)
            .collect::<Vec<_>>()
    };
    assert!(found("Appeler le plombier pour la fuite avant vendredi").is_empty());
    assert_eq!(found("Dupont pour la fuite"), vec![plumber_id.clone()]);
    assert_eq!(found("fuite"), vec![plumber_id]);
}

/// Audit 3 #11 — duplicate detection must not interpret Markdown (`### …`) as a tag.
#[test]
fn test_find_similar_ignores_markdown_as_tag() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Duplicates", None).unwrap();

    let mut a = note_input(&p, "### Reproduce", NoteType::Bug);
    a.content = Some("### Reproduce\nsame bug content".into());
    let mut b = note_input(&p, "### Reproduce", NoteType::Bug);
    b.content = Some("### Reproduce\nsame bug content".into());
    queries::create_note(&conn, a).unwrap();
    queries::create_note(&conn, b).unwrap();

    // Query as built by `find_similar_notes` (title + content).
    let query = "### Reproduce ### Reproduce\nsame bug content";

    // Without the fix, "###" became a "##" tag requirement → zero results.
    let found = queries::get_notes_without_tags(&conn, None, None, Some(query)).unwrap();
    assert_eq!(found.len(), 2, "both identical notes must be found");
}

// ───────────────────────── Database/commands robustness (audit 4 P2-R1) ─────────────────────────

/// P2-R1 — the IPC path refuses non-conforming `tags` (the MCP already validates them).
#[test]
fn test_ipc_tags_rejects_nonconforming() {
    assert!(queries::validate_tags_json("[42]").is_err());
    assert!(queries::validate_tags_json("\"abc\"").is_err());
    assert!(queries::validate_tags_json("not json").is_err());
    assert!(queries::validate_tags_json("[\"audit\",\"ui\"]").is_ok());
    assert!(queries::validate_tags_json("[]").is_ok());

    let conn = setup_test_db();
    let p = queries::create_project(&conn, "TagsIPC", None).unwrap();
    let mut input = note_input(&p, "Bad tags", NoteType::Note);
    input.tags = Some("[42]".into());
    assert!(
        queries::create_note(&conn, input).is_err(),
        "IPC create_note must refuse [42]"
    );
    assert!(queries::get_notes(&conn, None, None, None)
        .unwrap()
        .is_empty());
}

/// P2-R1 — deleting a nonexistent identifier returns an error, not a silent success.
#[test]
fn test_delete_missing_note_returns_error() {
    let conn = setup_test_db();
    assert!(queries::delete_note(&conn, "00000000-0000-0000-0000-000000000000").is_err());
}

/// P2-R1 — a corrupted row does not prevent reading the others.
#[test]
fn test_corrupt_row_isolated_in_list() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Corrupted", None).unwrap();
    let good = queries::create_note(&conn, note_input(&p, "Good", NoteType::Note)).unwrap();
    let bad = queries::create_note(&conn, note_input(&p, "Bad", NoteType::Note)).unwrap();
    // Unknown note_type injected directly (no validated path allows it).
    conn.execute(
        "UPDATE notes SET note_type = 'unknown' WHERE id = ?1",
        rusqlite::params![bad],
    )
    .unwrap();

    let notes = queries::get_notes(&conn, Some(&p), None, None).unwrap();
    assert_eq!(
        notes.len(),
        1,
        "the corrupted row must be skipped, the others read"
    );
    assert_eq!(notes[0].id, good);
}

/// P2-R1 — priority sorting is **numeric**: "P2" must precede "P10".
#[test]
fn test_handoff_sorts_priority_numerically() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Sorting", None).unwrap();
    // "P10" is impossible through validated paths: written directly to exercise the comparator.
    let p10 = queries::create_note(&conn, note_input(&p, "Item ten", NoteType::Todo)).unwrap();
    let p2 = queries::create_note(&conn, note_input(&p, "Item two", NoteType::Todo)).unwrap();
    conn.execute(
        "UPDATE notes SET priority = 'P10' WHERE id = ?1",
        rusqlite::params![p10],
    )
    .unwrap();
    conn.execute(
        "UPDATE notes SET priority = 'P2' WHERE id = ?1",
        rusqlite::params![p2],
    )
    .unwrap();

    let brief = queries::build_handoff(&conn, Some(&p)).unwrap();
    let i2 = brief.find("Item two").unwrap();
    let i10 = brief.find("Item ten").unwrap();
    assert!(i2 < i10, "P2 must come before P10:\n{brief}");
}

/// P2-R1 — the v9 migration adds the sort indexes without losing a note.
#[test]
fn test_migration_v9_adds_indexes() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Indexes", None).unwrap();
    queries::create_note(&conn, note_input(&p, "Existing", NoteType::Note)).unwrap();
    conn.execute("DROP INDEX IF EXISTS idx_notes_created_at", [])
        .unwrap();
    conn.execute("DROP INDEX IF EXISTS idx_notes_updated_at", [])
        .unwrap();
    conn.execute("PRAGMA user_version = 8", []).unwrap();

    schema::apply_migrations(&conn).unwrap();
    let names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_notes_%'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        names.contains(&"idx_notes_created_at".to_string()),
        "{names:?}"
    );
    assert!(
        names.contains(&"idx_notes_updated_at".to_string()),
        "{names:?}"
    );
    assert_eq!(
        queries::get_notes(&conn, None, None, None).unwrap().len(),
        1
    );
}

/// Follow-up #1 — the backup runs on a dedicated connection and produces a usable copy.
#[test]
fn test_save_to_usable_copy() {
    let dir = std::env::temp_dir().join(format!("humcase-backup-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let conn = crate::db::init_db(&dir).unwrap();
    let p = queries::create_project(&conn, "Backup", None).unwrap();
    queries::create_note(&conn, note_input(&p, "To back up", NoteType::Note)).unwrap();
    drop(conn);

    let source = dir.join("humcase.db");
    let destination = dir.join("copy.db");
    crate::db::save_to(&source, &destination).unwrap();
    assert!(destination.exists(), "the copy must exist");

    // The copy is a valid database and contains the note (VACUUM INTO, not a raw copy).
    let copy = Connection::open(&destination).unwrap();
    let notes = queries::get_notes(&copy, None, None, None).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].title, "To back up");

    drop(copy);
    let _ = std::fs::remove_dir_all(&dir);
}

// ───────────────────────── Atomic append (dictation on a note not displayed) ─────────────────────────

/// Audit 4 #4 — appending content is an atomic `UPDATE` (no read-modify-write):
/// a concurrent write is kept, and the title is only replaced if it was left at the default.
#[test]
fn test_append_note_content_atomic_and_default_title() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Append", None).unwrap();

    // Empty content: the text becomes the content, and the default title is replaced.
    let empty =
        queries::create_note(&conn, note_input(&p, "Nouvelle note", NoteType::Note)).unwrap();
    queries::append_note_content(&conn, &empty, "First block", Some("First block")).unwrap();
    let n = queries::get_note_by_id(&conn, &empty).unwrap();
    assert_eq!(n.content, "First block");
    assert_eq!(n.title, "First block");

    // Concurrent write (MCP) on the same note: it is kept, the text is appended.
    conn.execute(
        "UPDATE notes SET content = 'Written by MCP' WHERE id = ?1",
        rusqlite::params![empty],
    )
    .unwrap();
    queries::append_note_content(&conn, &empty, "Second block", Some("Second block")).unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &empty).unwrap().content,
        "Written by MCP\n\nSecond block"
    );

    // Custom title: never replaced.
    let custom = queries::create_note(&conn, note_input(&p, "My title", NoteType::Note)).unwrap();
    queries::append_note_content(&conn, &custom, "Text", Some("Derived title")).unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &custom).unwrap().title,
        "My title"
    );

    // Nonexistent note: explicit error, never a silent success.
    assert!(
        queries::append_note_content(&conn, "00000000-0000-0000-0000-000000000000", "x", None)
            .is_err()
    );
}

/// Every default title (old versions, French, English) stays replaceable: the title is
/// a stored piece of data, created in the interface language of the moment.
#[test]
fn test_append_note_content_recognizes_every_default_title() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Titles", None).unwrap();
    for title in queries::DEFAULT_TITLES {
        let id = queries::create_note(&conn, note_input(&p, title, NoteType::Note)).unwrap();
        queries::append_note_content(&conn, &id, "Block", Some("Derived title")).unwrap();
        assert_eq!(
            queries::get_note_by_id(&conn, &id).unwrap().title,
            "Derived title",
            "{title}"
        );
    }
}

/// C1 — creation validates the due date like modification (nonexistent day refused).
#[test]
fn test_create_note_rejects_impossible_due_date() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Due dates", None).unwrap();

    let mut input = note_input(&p, "Impossible", NoteType::Todo);
    input.due_at = Some("2026-02-31".into());
    assert!(
        queries::create_note(&conn, input).is_err(),
        "2026-02-31 must not be accepted"
    );
    assert!(queries::get_notes(&conn, None, None, None)
        .unwrap()
        .is_empty());
}

// ───────────────────────── v14 — revision & journal of agent edits ─────────────────────────

/// v13 → v14: `revision` column and `app_settings` / `note_revisions` tables added without losing
/// existing notes; a fresh database starts directly at v14.
#[test]
fn test_migration_v14_adds_revision_and_preserves_notes() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Migration v14", None).unwrap();
    let mut input = note_input(&p, "Kept title", NoteType::Bug);
    input.content = Some("kept content".into());
    input.tags = Some("[\"audit\"]".into());
    let id = queries::create_note(&conn, input).unwrap();

    // Simulates a database left at v13: the v14 column and tables are removed.
    conn.execute_batch(
        "
        DROP TABLE IF EXISTS note_revisions;
        DROP TABLE IF EXISTS app_settings;
        ALTER TABLE notes DROP COLUMN revision;
        PRAGMA user_version = 13;
    ",
    )
    .unwrap();

    schema::apply_migrations(&conn).unwrap();

    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);
    // v15: empty archive columns and project deletion guard present.
    let (archived, guard): (i64, i64) = conn.query_row(
        "SELECT (SELECT count(*) FROM notes WHERE archived_at IS NOT NULL) + (SELECT count(*) FROM projects WHERE archived_at IS NOT NULL),
                (SELECT count(*) FROM sqlite_master WHERE type = 'trigger' AND name = 'project_with_notes_cannot_be_deleted')",
        [], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();
    assert_eq!((archived, guard), (0, 1));
    let note = queries::get_note_by_id(&conn, &id).unwrap();
    assert_eq!(note.title, "Kept title");
    assert_eq!(note.content, "kept content");
    assert_eq!(note.tags.as_deref(), Some("[\"audit\"]"));
    assert_eq!(note.revision, 0, "existing notes restart at revision 0");

    // The new tables are usable and permission is refused by default.
    assert!(permissions::note_history(&conn, &id).unwrap().is_empty());
    assert!(!permissions::allow_note_edit(&conn).unwrap());
    permissions::set_setting(&conn, permissions::MCP_ALLOW_NOTE_EDIT, "1").unwrap();
    assert!(permissions::allow_note_edit(&conn).unwrap());
}

/// Every write path of a note increments its version. If one of them is removed from the
/// mechanism (`revision = revision + 1` or `bump_revision`), this test fails.
#[test]
fn test_every_note_write_path_bumps_revision() {
    use crate::models::project::ProjectSpace;
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Revisions", None).unwrap();

    let id = queries::create_note(&conn, note_input(&p, "N", NoteType::Note)).unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &id).unwrap().revision,
        0,
        "creation → revision 0"
    );

    // `update_note` also covers `update_note_status` (MCP) and `set_due_date` (MCP).
    queries::update_note(
        &conn,
        &id,
        update_note_input(
            Some(NoteStatus::InProgress),
            Some("P1"),
            Some("2026-12-31"),
            None,
        ),
    )
    .unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &id).unwrap().revision,
        1,
        "update_note"
    );

    queries::append_note_content(&conn, &id, "addition", None).unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &id).unwrap().revision,
        2,
        "append_note_content"
    );

    queries::append_raw_content(&conn, &id, "raw fragment").unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &id).unwrap().revision,
        3,
        "append_raw_content"
    );

    let mut structure_input = note_input(&p, "Free text", NoteType::Idea);
    structure_input.content = Some("original".into());
    let structured = queries::create_note(&conn, structure_input).unwrap();
    crate::db::structure::apply_structured_note(&conn, &structured, "original", "structured")
        .unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &structured)
            .unwrap()
            .revision,
        1,
        "apply_structured_note"
    );

    let inbox = queries::get_or_create_inbox(&conn, ProjectSpace::Work).unwrap();
    let target =
        queries::create_project_in_space(&conn, "Target", None, Some(ProjectSpace::Work)).unwrap();
    let mut captured = note_input(&inbox, "Captured", NoteType::Note);
    captured.content = Some("text".into());
    let moved = queries::create_note(&conn, captured).unwrap();
    assert_eq!(queries::get_note_by_id(&conn, &moved).unwrap().revision, 0);
    queries::move_note_to_project(&conn, &moved, &target).unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &moved).unwrap().revision,
        1,
        "move_note_to_project"
    );
}

/// Undoing a deletion restores the note's version, it does not reset it to 0.
#[test]
fn test_apply_agent_note_edit_snapshots_and_refuses_stale_revision() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Agent", None).unwrap();
    let mut input = note_input(&p, "Before", NoteType::Note);
    input.content = Some("c0".into());
    input.raw_content = Some("raw dictation".into());
    let id = queries::create_note(&conn, input).unwrap();

    // The human app writes in the meantime: the note moves to revision 1.
    queries::update_note(
        &conn,
        &id,
        update_note_input(Some(NoteStatus::InProgress), None, None, None),
    )
    .unwrap();

    // The agent, stuck on revision 0, is refused: no write or snapshot.
    let conflict = queries::apply_agent_note_edit(
        &conn,
        &id,
        0,
        queries::AgentNoteEdit {
            title: Some("After".into()),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(conflict.contains("revision conflict"), "{conflict}");
    assert!(
        conflict.contains('1'),
        "the message must recall the current version: {conflict}"
    );
    assert_eq!(queries::get_note_by_id(&conn, &id).unwrap().title, "Before");
    assert!(
        permissions::note_history(&conn, &id).unwrap().is_empty(),
        "a conflict writes no snapshot"
    );

    // Replay with the re-read version.
    let new_revision = queries::apply_agent_note_edit(
        &conn,
        &id,
        1,
        queries::AgentNoteEdit {
            title: Some("After".into()),
            tags: Some("[\"mcp\"]".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(new_revision, 2);
    let note = queries::get_note_by_id(&conn, &id).unwrap();
    assert_eq!(note.title, "After");
    assert_eq!(note.tags.as_deref(), Some("[\"mcp\"]"));
    assert_eq!(note.revision, 2);
    assert_eq!(
        note.raw_content.as_deref(),
        Some("raw dictation"),
        "raw_content is never modified"
    );

    let history = permissions::note_history(&conn, &id).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].source, "agent");
    assert_eq!(
        history[0].revision, 1,
        "the snapshot carries the old version"
    );
    let fields: serde_json::Value = serde_json::from_str(&history[0].fields).unwrap();
    assert_eq!(fields["title"], "Before", "previous values");
    assert_eq!(fields["tags"], "[]");
    assert!(
        fields.get("raw_content").is_none(),
        "raw_content must never appear in the snapshot"
    );
}

// --- Project management (v15) ---

fn project(conn: &Connection, id: &str) -> Option<crate::models::project::Project> {
    queries::get_projects(conn)
        .unwrap()
        .into_iter()
        .find(|p| p.id == id)
}

#[test]
fn test_rename_project_keeps_notes_and_refuses_duplicates_and_inbox() {
    use crate::models::project::ProjectSpace;
    let conn = setup_test_db();
    let a =
        queries::create_project_in_space(&conn, "Alpha", None, Some(ProjectSpace::Work)).unwrap();
    queries::create_project_in_space(&conn, "Beta", None, Some(ProjectSpace::Work)).unwrap();
    let note = queries::create_note(&conn, note_input(&a, "N", NoteType::Todo)).unwrap();

    queries::rename_project(&conn, &a, "  Humcase  ").unwrap();
    assert_eq!(project(&conn, &a).unwrap().name, "Humcase");
    assert_eq!(queries::get_note_by_id(&conn, &note).unwrap().project_id, a);

    assert_eq!(
        queries::rename_project(&conn, &a, "Beta").unwrap_err(),
        crate::tr!("projects.duplicateName")
    );
    assert_eq!(
        queries::rename_project(&conn, &a, "   ").unwrap_err(),
        crate::tr!("projects.nameEmpty")
    );
    assert_eq!(
        queries::rename_project(&conn, "missing", "X").unwrap_err(),
        crate::tr!("projects.notFound", id = "missing")
    );
    let inbox = queries::get_or_create_inbox(&conn, ProjectSpace::Work).unwrap();
    assert_eq!(
        queries::rename_project(&conn, &inbox, "X").unwrap_err(),
        crate::tr!("projects.inboxLocked")
    );
    assert_eq!(project(&conn, &a).unwrap().name, "Humcase");
}

#[test]
fn test_set_project_color_validates_hex() {
    let conn = setup_test_db();
    let a = queries::create_project(&conn, "Alpha", None).unwrap();
    queries::set_project_color(&conn, &a, "#A1B2C3").unwrap();
    assert_eq!(project(&conn, &a).unwrap().color, "#a1b2c3");
    for invalid in ["a1b2c3", "#a1b2c", "#a1b2cz", "red"] {
        assert!(
            queries::set_project_color(&conn, &a, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(project(&conn, &a).unwrap().color, "#a1b2c3");
}

#[test]
fn test_archive_project_hides_nothing_in_database_and_is_reversible() {
    use crate::models::project::ProjectSpace;
    let conn = setup_test_db();
    let a = queries::create_project_in_space(&conn, "Alpha", None, Some(ProjectSpace::Private))
        .unwrap();
    let note = queries::create_note(&conn, note_input(&a, "N", NoteType::Idea)).unwrap();
    let revision_before = queries::get_note_by_id(&conn, &note).unwrap().revision;

    queries::set_project_archived(&conn, &a, true).unwrap();
    let archived_at = project(&conn, &a).unwrap().archived_at.expect("archived");
    assert_iso_utc(&archived_at);
    // Archiving twice keeps the first date.
    queries::set_project_archived(&conn, &a, true).unwrap();
    assert_eq!(
        project(&conn, &a).unwrap().archived_at.as_deref(),
        Some(archived_at.as_str())
    );
    // Notes are neither moved nor rewritten.
    let n = queries::get_note_by_id(&conn, &note).unwrap();
    assert_eq!(n.project_id, a);
    assert_eq!(n.revision, revision_before);

    queries::set_project_archived(&conn, &a, false).unwrap();
    assert!(project(&conn, &a).unwrap().archived_at.is_none());
    let inbox = queries::get_or_create_inbox(&conn, ProjectSpace::Private).unwrap();
    assert!(queries::set_project_archived(&conn, &inbox, true).is_err());
}

#[test]
fn test_delete_project_moves_notes_and_links_to_inbox() {
    use crate::models::project::ProjectSpace;
    let conn = setup_test_db();
    let a =
        queries::create_project_in_space(&conn, "Alpha", None, Some(ProjectSpace::Work)).unwrap();
    let other =
        queries::create_project_in_space(&conn, "Other", None, Some(ProjectSpace::Work)).unwrap();
    let source = queries::create_note(&conn, note_input(&a, "Source", NoteType::Idea)).unwrap();
    let task = queries::create_note(&conn, note_input(&a, "Task", NoteType::Todo)).unwrap();
    let witness =
        queries::create_note(&conn, note_input(&other, "Witness", NoteType::Todo)).unwrap();
    crate::db::relations::link_note_task(&conn, &source, &task).unwrap();
    let revision_before = queries::get_note_by_id(&conn, &source).unwrap().revision;

    assert_eq!(queries::delete_project(&conn, &a).unwrap(), 2);

    assert!(project(&conn, &a).is_none());
    let inbox = queries::get_or_create_inbox(&conn, ProjectSpace::Work).unwrap();
    for id in [&source, &task] {
        assert_eq!(
            queries::get_note_by_id(&conn, id).unwrap().project_id,
            inbox
        );
    }
    assert!(queries::get_note_by_id(&conn, &source).unwrap().revision > revision_before);
    let links: i64 = conn
        .query_row(
            "SELECT count(*) FROM note_task_links WHERE source_note_id = ?1 AND task_note_id = ?2",
            [&source, &task],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(links, 1, "the note → task link is kept");
    assert_eq!(
        queries::get_note_by_id(&conn, &witness).unwrap().project_id,
        other
    );
}

#[test]
fn test_delete_project_refuses_inbox_and_projects_without_space() {
    use crate::models::project::ProjectSpace;
    let conn = setup_test_db();
    let no_space = queries::create_project(&conn, "No space", None).unwrap();
    let note = queries::create_note(&conn, note_input(&no_space, "N", NoteType::Todo)).unwrap();
    assert_eq!(
        queries::delete_project(&conn, &no_space).unwrap_err(),
        crate::tr!("projects.needsSpaceToDelete")
    );
    assert!(project(&conn, &no_space).is_some());
    assert_eq!(
        queries::get_note_by_id(&conn, &note).unwrap().project_id,
        no_space
    );

    let inbox = queries::get_or_create_inbox(&conn, ProjectSpace::Work).unwrap();
    assert_eq!(
        queries::delete_project(&conn, &inbox).unwrap_err(),
        crate::tr!("projects.inboxLocked")
    );
}

/// Last-resort guard: without the v15 trigger, `ON DELETE CASCADE` would erase the notes.
#[test]
fn test_deleting_project_with_notes_directly_is_refused() {
    let conn = setup_test_db();
    let a = queries::create_project(&conn, "Alpha", None).unwrap();
    let note = queries::create_note(&conn, note_input(&a, "N", NoteType::Todo)).unwrap();
    assert!(conn
        .execute("DELETE FROM projects WHERE id = ?1", [&a])
        .is_err());
    assert!(queries::get_note_by_id(&conn, &note).is_ok());
    // An empty project is deleted normally.
    let empty = queries::create_project(&conn, "Empty", None).unwrap();
    conn.execute("DELETE FROM projects WHERE id = ?1", [&empty])
        .unwrap();
}

// --- Note archiving (0.9.77) ---

#[test]
fn test_archive_note_keeps_status_and_date_and_is_reversible() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Alpha", None).unwrap();
    let id = queries::create_note(&conn, note_input(&p, "N", NoteType::Todo)).unwrap();
    let before = queries::get_note_by_id(&conn, &id).unwrap();
    assert!(before.archived_at.is_none());

    queries::set_note_archived(&conn, &id, true).unwrap();
    let archived = queries::get_note_by_id(&conn, &id).unwrap();
    assert_iso_utc(archived.archived_at.as_deref().unwrap());
    assert_eq!(archived.status, before.status);
    assert_eq!(
        archived.updated_at, before.updated_at,
        "archiving does not bring the note back up"
    );
    assert!(
        archived.revision > before.revision,
        "archiving is a write: the revision advances"
    );
    // Still listed (search and the MCP see it) with its archive date.
    let listed = queries::get_notes(&conn, Some(&p), None, None).unwrap();
    assert_eq!(listed[0].archived_at, archived.archived_at);

    // The sidebar counters ignore an archived note.
    let count = |conn: &Connection| {
        queries::get_projects(conn)
            .unwrap()
            .into_iter()
            .find(|x| x.id == p)
            .unwrap()
            .open_todos
    };
    assert_eq!(count(&conn), Some(0));

    queries::set_note_archived(&conn, &id, false).unwrap();
    assert!(queries::get_note_by_id(&conn, &id)
        .unwrap()
        .archived_at
        .is_none());
    assert_eq!(count(&conn), Some(1));
    assert_eq!(
        queries::set_note_archived(&conn, "missing", true).unwrap_err(),
        crate::tr!("notes.notFound", id = "missing")
    );
}

#[test]
fn test_handoff_skips_archived_notes_and_projects_in_to_resume() {
    use crate::models::project::ProjectSpace;
    let conn = setup_test_db();
    let p =
        queries::create_project_in_space(&conn, "Alpha", None, Some(ProjectSpace::Work)).unwrap();
    let filed =
        queries::create_project_in_space(&conn, "Filed", None, Some(ProjectSpace::Work)).unwrap();
    queries::create_note(&conn, note_input(&p, "Active", NoteType::Todo)).unwrap();
    let archived =
        queries::create_note(&conn, note_input(&p, "Set aside", NoteType::Todo)).unwrap();
    queries::create_note(
        &conn,
        note_input(&filed, "In archived project", NoteType::Todo),
    )
    .unwrap();
    queries::set_note_archived(&conn, &archived, true).unwrap();
    queries::set_project_archived(&conn, &filed, true).unwrap();

    let handoff = queries::build_handoff(&conn, None).unwrap();
    assert!(handoff.contains("Active"));
    assert!(!handoff.contains("Set aside"), "{handoff}");
    assert!(!handoff.contains("In archived project"), "{handoff}");
}

// --- Interface refresh (writes from another process) ---

/// The fingerprint must change for every write an agent can make: otherwise the open interface
/// keeps a stale state (bug of 28/09/2026: note resolved by MCP displayed "In progress").
#[test]
fn test_notes_change_marker_follows_every_note_write() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Alpha", None).unwrap();
    let mut previous = queries::notes_change_marker(&conn).unwrap();
    let mut change = |conn: &Connection, what: &str| {
        let current = queries::notes_change_marker(conn).unwrap();
        assert_ne!(
            current, previous,
            "the fingerprint must change after: {what}"
        );
        previous = current;
    };

    let id = queries::create_note(&conn, note_input(&p, "N", NoteType::Todo)).unwrap();
    change(&conn, "creation");
    queries::update_note(
        &conn,
        &id,
        UpdateNoteInput {
            status: Some(NoteStatus::Resolved),
            ..update_note_input(None, None, None, None)
        },
    )
    .unwrap();
    change(&conn, "status change");
    queries::set_note_archived(&conn, &id, true).unwrap();
    change(&conn, "archiving (without date change)");
    queries::delete_note(&conn, &id).unwrap();
    change(&conn, "deletion");

    // No write: stable fingerprint, no needless reload.
    assert_eq!(queries::notes_change_marker(&conn).unwrap(), previous);
}

/// Checks that the fingerprint changed since `previous`, then memorizes it.
fn marker_changed(conn: &Connection, previous: &mut String, what: &str) {
    let current = queries::notes_change_marker(conn).unwrap();
    assert_ne!(
        &current, previous,
        "the fingerprint must change after: {what}"
    );
    *previous = current;
}

/// The counterpart for projects, activities and their attachments: an MCP agent or the CLI
/// (another process) writing them must change the fingerprint, otherwise the open interface keeps a
/// stale list. Every write path is exercised; reads change nothing.
#[test]
fn test_notes_change_marker_follows_every_project_and_activity_write() {
    use crate::models::project::ProjectSpace::{Private, Work};
    let conn = setup_test_db();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    let mut previous = queries::notes_change_marker(&conn).unwrap();

    // Projects. `old_project` is created first: renaming it changes neither the count nor the maximum date
    // of projects, only its revision betrays the write.
    let old_project = queries::create_project_in_space(&conn, "Old", None, Some(Work)).unwrap();
    marker_changed(&conn, &mut previous, "creation of a project");
    let p = queries::create_project_in_space(&conn, "Alpha", None, Some(Work)).unwrap();
    let no_space = queries::create_project(&conn, "No space", None).unwrap();
    marker_changed(
        &conn,
        &mut previous,
        "creation of a project without a space",
    );
    queries::get_or_create_inbox(&conn, Private).unwrap();
    marker_changed(&conn, &mut previous, "creation of the Inbox (a project)");
    queries::get_or_create_inbox(&conn, Private).unwrap();
    assert_eq!(
        queries::notes_change_marker(&conn).unwrap(),
        previous,
        "existing Inbox: nothing is written"
    );
    queries::rename_project(&conn, &old_project, "Old 2").unwrap();
    marker_changed(&conn, &mut previous, "renaming of a project");
    queries::set_project_color(&conn, &old_project, "#112233").unwrap();
    marker_changed(&conn, &mut previous, "color of a project");
    queries::set_project_archived(&conn, &old_project, true).unwrap();
    marker_changed(&conn, &mut previous, "archiving of a project");
    queries::set_project_archived(&conn, &old_project, false).unwrap();
    marker_changed(&conn, &mut previous, "unarchiving of a project");
    queries::update_project_space(&conn, &no_space, Some(Work)).unwrap();
    marker_changed(&conn, &mut previous, "space of a project");

    // Activities.
    let a = queries::create_activity(&conn, "Dev", Work).unwrap();
    marker_changed(&conn, &mut previous, "creation of an activity");
    let b = queries::create_activity(&conn, "Marketing", Work).unwrap();
    marker_changed(&conn, &mut previous, "creation of a second activity");
    queries::rename_activity(&conn, &a.id, "Development").unwrap();
    marker_changed(&conn, &mut previous, "renaming of an activity (the oldest)");

    // Attachments.
    queries::set_project_activities(&conn, &p, &[a.id.clone(), b.id.clone()], None).unwrap();
    marker_changed(&conn, &mut previous, "attachment of two activities");
    queries::set_project_activities(&conn, &p, std::slice::from_ref(&a.id), None).unwrap();
    marker_changed(&conn, &mut previous, "removal of a single attachment");
    queries::set_project_activities(&conn, &p, std::slice::from_ref(&b.id), None).unwrap();
    marker_changed(
        &conn,
        &mut previous,
        "swap of an attachment (same number of links)",
    );
    queries::update_project_space(&conn, &p, Some(Private)).unwrap();
    marker_changed(
        &conn,
        &mut previous,
        "space changed: the trigger removes the attachment",
    );

    // Deletions.
    queries::update_project_space(&conn, &p, Some(Work)).unwrap();
    queries::set_project_activities(&conn, &p, std::slice::from_ref(&a.id), None).unwrap();
    previous = queries::notes_change_marker(&conn).unwrap();
    queries::delete_activity(&conn, &a.id).unwrap();
    marker_changed(&conn, &mut previous, "deletion of an attached activity");
    queries::set_project_activities(&conn, &p, std::slice::from_ref(&b.id), None).unwrap();
    previous = queries::notes_change_marker(&conn).unwrap();
    queries::delete_project(&conn, &p).unwrap();
    marker_changed(&conn, &mut previous, "deletion of an attached project");
    queries::delete_project(&conn, &old_project).unwrap();
    marker_changed(&conn, &mut previous, "deletion of a project");
    queries::delete_activity(&conn, &b.id).unwrap();
    marker_changed(&conn, &mut previous, "deletion of an activity");

    // Reads: no write, stable fingerprint (no needless reload on the interface side).
    queries::get_projects(&conn).unwrap();
    queries::get_activities(&conn).unwrap();
    queries::get_notes(&conn, None, None, None).unwrap();
    queries::default_project(&conn).unwrap();
    assert_eq!(queries::notes_change_marker(&conn).unwrap(), previous);
    assert_eq!(queries::notes_change_marker(&conn).unwrap(), previous);
}

/// Each component of the fingerprint counts on its own: a write path that forgot to
/// advance the version (or a future import, a restore) remains visible through the count, the
/// date or the number of attachments. Direct writes: they deliberately bypass the
/// `queries` functions, which all advance the version.
#[test]
fn test_notes_change_marker_each_project_and_link_component_counts() {
    use crate::models::project::ProjectSpace::Work;
    let conn = setup_test_db();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    let p1 = queries::create_project_in_space(&conn, "One", None, Some(Work)).unwrap();
    let _p2 = queries::create_project_in_space(&conn, "Two", None, Some(Work)).unwrap();
    pause();
    let a = queries::create_activity(&conn, "A", Work).unwrap();
    let b = queries::create_activity(&conn, "B", Work).unwrap();
    queries::set_project_activities(&conn, &p1, &[a.id.clone(), b.id.clone()], None).unwrap();

    let mut previous = queries::notes_change_marker(&conn).unwrap();

    // Projects: date alone, then revision alone on a row that does not have the maximum date.
    conn.execute(
        "UPDATE projects SET updated_at = '2099-01-01T00:00:00.000Z' WHERE id = ?1",
        [&p1],
    )
    .unwrap();
    marker_changed(&conn, &mut previous, "projects: updated_at alone");
    conn.execute(
        "UPDATE projects SET revision = revision + 1 WHERE name = 'Two'",
        [],
    )
    .unwrap();
    marker_changed(
        &conn,
        &mut previous,
        "projects: revision alone (row that does not have the maximum date)",
    );
    // Activities: the same two cases.
    conn.execute(
        "UPDATE activities SET updated_at = '2099-01-01T00:00:00.000Z' WHERE id = ?1",
        [&a.id],
    )
    .unwrap();
    marker_changed(&conn, &mut previous, "activities: updated_at alone");
    conn.execute(
        "UPDATE activities SET revision = revision + 1 WHERE id = ?1",
        [&b.id],
    )
    .unwrap();
    marker_changed(
        &conn,
        &mut previous,
        "activities: revision alone (row that does not have the maximum date)",
    );

    // Attachments deleted alone (without a version advancing): the link count betrays them.
    conn.execute(
        "DELETE FROM project_activities WHERE project_id = ?1 AND activity_id = ?2",
        [&p1, &b.id],
    )
    .unwrap();
    marker_changed(
        &conn,
        &mut previous,
        "attachments: deletion of a single link",
    );
    conn.execute("DELETE FROM project_activities", []).unwrap();
    marker_changed(&conn, &mut previous, "attachments: last link deleted");
    // Direct deletion of an activity or a project, without any other write.
    conn.execute("DELETE FROM activities WHERE id = ?1", [&b.id])
        .unwrap();
    marker_changed(&conn, &mut previous, "activities: deletion alone");
    conn.execute("DELETE FROM projects WHERE name = 'Two'", [])
        .unwrap();
    marker_changed(&conn, &mut previous, "projects: deletion alone");
}

// --- Portable images (0.9.81) ---

#[test]
fn test_migration_v16_makes_internal_image_urls_portable() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "Images", None).unwrap();
    let url = "http://asset.localhost/C%3A%5CUsers%5Cx%5CAppData%5CRoaming%5Ccom.humcase.desktop%5Cimages%5Cabc.png";
    let external = "![e](http://asset.localhost/C%3A%5CPhotos%5Cvacations.png)";
    let id = queries::create_note(&conn, note_input(&p, "With image", NoteType::Note)).unwrap();
    let without =
        queries::create_note(&conn, note_input(&p, "Without image", NoteType::Note)).unwrap();
    conn.execute(
        "UPDATE notes SET content = ?1, raw_content = ?2, resolution_comment = ?3 WHERE id = ?4",
        rusqlite::params![
            format!("Text ![a]({url}) {external}"),
            format!("Raw ![a]({url})"),
            format!("Report ![a]({url})"),
            id
        ],
    )
    .unwrap();
    let before = queries::get_note_by_id(&conn, &id).unwrap();
    let without_before = queries::get_note_by_id(&conn, &without).unwrap();
    conn.execute_batch("PRAGMA user_version = 15;").unwrap();

    schema::apply_migrations(&conn).unwrap();

    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);
    let after = queries::get_note_by_id(&conn, &id).unwrap();
    assert_eq!(
        after.content,
        format!("Text ![a](humcase-image://abc.png) {external}"),
        "the image outside the Humcase folder is intact"
    );
    assert_eq!(
        after.raw_content.as_deref(),
        Some("Raw ![a](humcase-image://abc.png)")
    );
    assert_eq!(
        after.resolution_comment.as_deref(),
        Some("Report ![a](humcase-image://abc.png)")
    );
    assert_eq!(
        after.revision,
        before.revision + 1,
        "rewrite = write: the revision advances"
    );
    assert_eq!(
        after.updated_at, before.updated_at,
        "the note does not come back up among recent ones"
    );
    let without_after = queries::get_note_by_id(&conn, &without).unwrap();
    assert_eq!(
        without_after.revision, without_before.revision,
        "a note without an image is not touched"
    );
}

/// Trash (v17): a deleted note disappears from reads, keeps its content identical and
/// comes back as it was; only its revision and its trash date move.
#[test]
fn test_trash_hides_note_and_restores_it_identically() {
    let conn = setup_test_db();
    let project = queries::create_project(&conn, "P", None).unwrap();
    let id = queries::create_note(
        &conn,
        CreateNoteInput {
            content: Some("unique-trash-word".into()),
            ..note_input(&project, "To throw away", NoteType::Bug)
        },
    )
    .unwrap();
    queries::update_note(
        &conn,
        &id,
        update_note_input(
            Some(NoteStatus::InProgress),
            Some("P2"),
            Some("2026-12-31"),
            Some("in progress"),
        ),
    )
    .unwrap();
    let before = queries::get_note_by_id(&conn, &id).unwrap();

    queries::trash_note(&conn, &id).unwrap();
    assert!(queries::get_notes(&conn, None, None, None)
        .unwrap()
        .iter()
        .all(|n| n.id != id));
    assert!(
        queries::get_notes(&conn, None, None, Some("unique-trash-word"))
            .unwrap()
            .is_empty()
    );
    assert!(queries::get_daily_standup(&conn, &project)
        .unwrap()
        .is_empty());
    assert!(!queries::build_handoff(&conn, None)
        .unwrap()
        .contains("To throw away"));
    let project_read = queries::get_projects(&conn)
        .unwrap()
        .into_iter()
        .find(|p| p.id == project)
        .unwrap();
    assert_eq!(project_read.open_bugs, Some(0));
    let trash = queries::get_trashed_notes(&conn).unwrap();
    assert_eq!(trash.len(), 1);
    assert!(trash[0].deleted_at.is_some());
    assert!(queries::trash_note(&conn, &id)
        .unwrap_err()
        .contains("already in the trash"));

    queries::restore_from_trash(&conn, &id).unwrap();
    let after = queries::get_note_by_id(&conn, &id).unwrap();
    assert_eq!(after.deleted_at, None);
    assert_eq!(
        after.revision,
        before.revision + 2,
        "moving to the trash and restoring are writes"
    );
    assert_eq!(
        (
            after.title,
            after.content,
            after.status,
            after.priority,
            after.due_at,
            after.resolution_comment,
            after.updated_at
        ),
        (
            before.title,
            before.content,
            before.status,
            before.priority,
            before.due_at,
            before.resolution_comment,
            before.updated_at
        )
    );
    assert!(queries::restore_from_trash(&conn, &id)
        .unwrap_err()
        .contains("not in the trash"));
}

/// The v17 trigger refuses any ordinary write on a note in the trash (editor, MCP,
/// status, archiving), but lets project deletion file it into the Inbox.
#[test]
fn test_trashed_note_content_is_read_only_but_can_follow_project_deletion() {
    use crate::models::project::ProjectSpace;
    let conn = setup_test_db();
    let project =
        queries::create_project_in_space(&conn, "P", None, Some(ProjectSpace::Work)).unwrap();
    let id = queries::create_note(&conn, note_input(&project, "Frozen", NoteType::Todo)).unwrap();
    queries::trash_note(&conn, &id).unwrap();

    let refuse = |r: Result<(), String>| assert!(r.unwrap_err().contains("note is in the trash"));
    refuse(queries::update_note(
        &conn,
        &id,
        UpdateNoteInput {
            content: Some("modified".into()),
            ..update_note_input(None, None, None, None)
        },
    ));
    refuse(queries::update_note(
        &conn,
        &id,
        update_note_input(Some(NoteStatus::Resolved), None, None, Some("done")),
    ));
    refuse(queries::set_note_archived(&conn, &id, true));
    assert!(queries::get_note_by_id(&conn, &id)
        .unwrap()
        .content
        .is_empty());

    assert_eq!(queries::delete_project(&conn, &project).unwrap(), 1);
    let note = queries::get_note_by_id(&conn, &id).unwrap();
    assert!(
        note.deleted_at.is_some(),
        "the note stays in the trash, filed in the Inbox"
    );
    assert_ne!(note.project_id, project);
}

/// Purge: only notes in the trash for more than 30 days leave, with a minimal trace;
/// a live note is never touched, even by "Empty the trash".
#[test]
fn test_an_empty_new_case_goes_to_the_trash_and_any_trace_keeps_it() {
    let conn = setup_test_db();
    let project = queries::create_project(&conn, "P", None).unwrap();
    let fresh = || {
        queries::create_note(&conn, note_input(&project, "Nouvelle case", NoteType::Note)).unwrap()
    };
    let in_trash = |id: &str| -> bool {
        conn.query_row(
            "SELECT deleted_at IS NOT NULL FROM notes WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
    };

    // Left as created, of any nature: it goes to the trash, never erased.
    let empty = fresh();
    assert!(queries::trash_if_empty(&conn, &empty).unwrap());
    assert!(in_trash(&empty));
    let task =
        queries::create_note(&conn, note_input(&project, "New case", NoteType::Todo)).unwrap();
    assert!(queries::trash_if_empty(&conn, &task).unwrap());

    // The slightest trace keeps it.
    let other = fresh();
    let traces: Vec<(&str, String)> = vec![
        ("title", "UPDATE notes SET title = 'Idée' WHERE id = ?1".into()),
        ("text", "UPDATE notes SET content = 'x' WHERE id = ?1".into()),
        ("dictation", "UPDATE notes SET raw_content = 'dit' WHERE id = ?1".into()),
        ("fragment", "INSERT INTO dictation_fragments (id, note_id, text, created_at) VALUES ('f-' || ?1, ?1, 'dit', '2026-10-05T00:00:00Z')".into()),
        ("tag", "UPDATE notes SET tags = '[\"a\"]' WHERE id = ?1".into()),
        ("priority", "UPDATE notes SET priority = 'P1' WHERE id = ?1".into()),
        ("due date", "UPDATE notes SET due_at = '2026-10-10' WHERE id = ?1".into()),
        ("status", "UPDATE notes SET status = 'in_progress' WHERE id = ?1".into()),
        ("drawing", "UPDATE notes SET content_kind = 'sketch' WHERE id = ?1".into()),
        ("archived", "UPDATE notes SET archived_at = '2026-10-05T00:00:00Z' WHERE id = ?1".into()),
        ("sub-case", format!("UPDATE notes SET parent_id = ?1 WHERE id = '{other}'")),
        ("link", format!("INSERT INTO note_task_links (source_note_id, task_note_id) VALUES (?1, '{other}')")),
    ];
    for (trace, sql) in traces {
        let id = fresh();
        conn.execute(&sql, [&id])
            .unwrap_or_else(|e| panic!("{trace}: {e}"));
        assert!(
            !queries::trash_if_empty(&conn, &id).unwrap(),
            "{trace} must keep the case"
        );
        assert!(!in_trash(&id), "{trace}");
        conn.execute("UPDATE notes SET parent_id = NULL WHERE id = ?1", [&other])
            .unwrap();
        conn.execute("DELETE FROM note_task_links", []).unwrap();
    }
}

#[test]
fn test_the_launch_sweep_only_takes_empty_cases_left_for_an_hour() {
    let conn = setup_test_db();
    let project = queries::create_project(&conn, "P", None).unwrap();
    let make = |title: &str, content: Option<&str>| {
        let mut input = note_input(&project, title, NoteType::Note);
        input.content = content.map(str::to_string);
        queries::create_note(&conn, input).unwrap()
    };
    let old_empty = make("Nouvelle case", None);
    let old_written = make("Nouvelle case", Some("une idée"));
    let recent_empty = make("Nouvelle case", None);
    for id in [&old_empty, &old_written] {
        conn.execute("UPDATE notes SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 hours') WHERE id = ?1", [id]).unwrap();
    }
    assert_eq!(
        queries::trash_empty_cases(&conn, queries::EMPTY_CASE_GRACE_MINUTES).unwrap(),
        1
    );
    let deleted = |id: &str| -> bool {
        conn.query_row(
            "SELECT deleted_at IS NOT NULL FROM notes WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert!(deleted(&old_empty));
    assert!(!deleted(&old_written));
    assert!(!deleted(&recent_empty));
}

#[test]
fn test_purge_only_erases_old_trash_and_keeps_tombstones() {
    let conn = setup_test_db();
    let project = queries::create_project(&conn, "P", None).unwrap();
    let live = queries::create_note(&conn, note_input(&project, "Live", NoteType::Note)).unwrap();
    let old = queries::create_note(&conn, note_input(&project, "Old", NoteType::Note)).unwrap();
    let recent =
        queries::create_note(&conn, note_input(&project, "Recent", NoteType::Note)).unwrap();
    conn.execute("UPDATE notes SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-31 days') WHERE id = ?1", [&old]).unwrap();
    queries::trash_note(&conn, &recent).unwrap();

    assert_eq!(
        queries::purge_trash(
            &conn,
            queries::TrashPurge::OlderThanDays(queries::TRASH_RETENTION_DAYS)
        )
        .unwrap(),
        1
    );
    assert!(queries::get_note_by_id(&conn, &old).is_err());
    assert!(queries::get_note_by_id(&conn, &recent).is_ok());
    let traces: Vec<String> = conn
        .prepare("SELECT id FROM note_tombstones")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(traces, vec![old.clone()]);

    assert!(queries::purge_trash(&conn, queries::TrashPurge::One(&live))
        .unwrap_err()
        .contains("not in the trash"));
    // "Empty the trash" erases what was shown; a live note listed by mistake is never touched,
    // and a case trashed after the confirmation stays.
    let late = queries::create_note(&conn, note_input(&project, "Late", NoteType::Note)).unwrap();
    queries::trash_note(&conn, &late).unwrap();
    assert_eq!(
        queries::purge_trash(
            &conn,
            queries::TrashPurge::Listed(&[recent.clone(), live.clone()])
        )
        .unwrap(),
        1
    );
    assert!(queries::get_note_by_id(&conn, &live).is_ok());
    assert!(queries::get_note_by_id(&conn, &recent).is_err());
    assert!(queries::get_note_by_id(&conn, &late).is_ok());
}

/// A vanished note (erased between the failure and the retry) gives an explicit result: the interface
/// no longer depends on an error text, translated and therefore fragile.
#[test]
fn test_retry_note_update_reports_missing_note_as_result() {
    let conn = setup_test_db();
    let result = queries::retry_note_update(
        &conn,
        "missing-note",
        UpdateNoteInput {
            content: Some("draft".into()),
            ..update_note_input(None, None, None, None)
        },
        serde_json::json!({ "content": "" }),
    )
    .unwrap();
    assert_eq!(result, queries::RetryNoteUpdateResult::NotFound);
}

// --- Concurrency version of projects and activities (v18, batch S1) ---

/// (revision, updated_at) of a project or an activity, read directly from the database.
fn versions(conn: &Connection, table: &str, id: &str) -> (i64, String) {
    conn.query_row(
        &format!("SELECT revision, updated_at FROM {table} WHERE id = ?1"),
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
}

/// Checks that a write advanced the version of `id` by exactly 1, and `updated_at`
/// strictly (a 2 ms pause separates two writes: the resolution is the millisecond).
fn assert_bumped(
    conn: &Connection,
    table: &str,
    id: &str,
    before: &(i64, String),
    what: &str,
) -> (i64, String) {
    let after = versions(conn, table, id);
    assert_eq!(
        after.0,
        before.0 + 1,
        "{what}: {table}.revision must advance by 1"
    );
    assert!(
        after.1 > before.1,
        "{what}: {table}.updated_at must advance ({} → {})",
        before.1,
        after.1
    );
    assert_iso_utc(&after.1);
    after
}

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(2));
}

/// Every write path of a project or an activity advances its version, in the same
/// transaction, including the "other" side of an attachment and the effects of triggers. If
/// one of these paths loses its increment, this test fails.
#[test]
fn test_every_project_and_activity_write_path_bumps_revision() {
    use crate::models::project::ProjectSpace::{Private, Work};
    let conn = setup_test_db();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();

    // Creations: version 0, `updated_at` = `created_at` (ISO 8601 UTC).
    let p = queries::create_project_in_space(&conn, "Alpha", None, Some(Work)).unwrap();
    let p2 = queries::create_project_in_space(&conn, "Beta", None, Some(Work)).unwrap();
    let p3 = queries::create_project(&conn, "Gamma", None).unwrap();
    let inbox = queries::get_or_create_inbox(&conn, Work).unwrap();
    for id in [&p, &p2, &p3, &inbox] {
        let (rev, updated) = versions(&conn, "projects", id);
        let created: String = conn
            .query_row("SELECT created_at FROM projects WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(rev, 0, "creation of a project → revision 0");
        assert_eq!(
            updated, created,
            "creation of a project → updated_at = created_at"
        );
        assert_iso_utc(&updated);
    }
    let a = queries::create_activity(&conn, "Dev", Work).unwrap();
    let b = queries::create_activity(&conn, "Marketing", Work).unwrap();
    let c = queries::create_activity(&conn, "Travel", Private).unwrap();
    for act in [&a, &b, &c] {
        assert_eq!(act.revision, 0, "creation of an activity → revision 0");
        assert_eq!(
            act.updated_at, act.created_at,
            "creation of an activity → updated_at = created_at"
        );
        assert_iso_utc(&act.updated_at);
    }
    // The exposed models carry the same values as the database.
    let listed = queries::get_projects(&conn).unwrap();
    let alpha = listed.iter().find(|x| x.id == p).unwrap();
    assert_eq!(
        (alpha.revision, alpha.updated_at.clone()),
        versions(&conn, "projects", &p)
    );

    // Renaming, color, archiving, unarchiving, space: each advances the project.
    let mut v = versions(&conn, "projects", &p);
    pause();
    queries::rename_project(&conn, &p, "Alpha 2").unwrap();
    v = assert_bumped(&conn, "projects", &p, &v, "rename_project");
    pause();
    queries::set_project_color(&conn, &p, "#112233").unwrap();
    v = assert_bumped(&conn, "projects", &p, &v, "set_project_color");
    pause();
    queries::set_project_archived(&conn, &p, true).unwrap();
    v = assert_bumped(&conn, "projects", &p, &v, "set_project_archived(true)");
    pause();
    queries::set_project_archived(&conn, &p, false).unwrap();
    v = assert_bumped(&conn, "projects", &p, &v, "set_project_archived(false)");
    pause();
    queries::update_project_space(&conn, &p3, Some(Work)).unwrap();
    let v3 = versions(&conn, "projects", &p3);
    assert_eq!(v3.0, 1, "update_project_space (no space → Work)");

    // Attachment: BOTH sides advance (the project once, each touched activity).
    let (pa, pb, pp2) = (
        versions(&conn, "activities", &a.id),
        versions(&conn, "activities", &b.id),
        versions(&conn, "projects", &p2),
    );
    pause();
    queries::set_project_activities(&conn, &p, &[a.id.clone(), b.id.clone()], None).unwrap();
    v = assert_bumped(
        &conn,
        "projects",
        &p,
        &v,
        "set_project_activities (project)",
    );
    let mut va = assert_bumped(
        &conn,
        "activities",
        &a.id,
        &pa,
        "set_project_activities (activity added a)",
    );
    let mut vb = assert_bumped(
        &conn,
        "activities",
        &b.id,
        &pb,
        "set_project_activities (activity added b)",
    );
    assert_eq!(
        versions(&conn, "projects", &p2),
        pp2,
        "an unaffected project does not move"
    );

    // Removal of an attachment: the project and the removed activity advance, the other activity does not.
    pause();
    queries::set_project_activities(&conn, &p, std::slice::from_ref(&a.id), None).unwrap();
    v = assert_bumped(
        &conn,
        "projects",
        &p,
        &v,
        "set_project_activities (removal, project)",
    );
    vb = assert_bumped(
        &conn,
        "activities",
        &b.id,
        &vb,
        "set_project_activities (removal, removed activity)",
    );
    assert_eq!(
        versions(&conn, "activities", &a.id),
        va,
        "a kept activity does not move"
    );

    // A save that changes nothing advances nothing (no artificial synchronization conflict).
    pause();
    queries::set_project_activities(&conn, &p, std::slice::from_ref(&a.id), None).unwrap();
    assert_eq!(
        versions(&conn, "projects", &p),
        v,
        "unchanged attachments → unchanged project"
    );
    assert_eq!(
        versions(&conn, "activities", &a.id),
        va,
        "unchanged attachments → unchanged activity"
    );
    // A refused write advances nothing either.
    assert!(queries::set_project_activities(&conn, &p, std::slice::from_ref(&c.id), None).is_err());
    assert_eq!(versions(&conn, "projects", &p), v);
    assert_eq!(
        versions(&conn, "activities", &c.id),
        (0, c.created_at.clone())
    );

    // Activity renaming.
    pause();
    queries::rename_activity(&conn, &b.id, "Communication").unwrap();
    assert_bumped(&conn, "activities", &b.id, &vb, "rename_activity");

    // Trigger effect: changing the project's space removes its attachments that became
    // incompatible; the project AND the activity whose attachment disappears advance.
    queries::set_project_activities(&conn, &p, &[a.id.clone(), b.id.clone()], None).unwrap();
    v = versions(&conn, "projects", &p);
    va = versions(&conn, "activities", &a.id);
    vb = versions(&conn, "activities", &b.id);
    pause();
    queries::update_project_space(&conn, &p, Some(Private)).unwrap();
    assert_bumped(&conn, "projects", &p, &v, "update_project_space (project)");
    assert_bumped(
        &conn,
        "activities",
        &a.id,
        &va,
        "trigger project_space_unlink_activities (a)",
    );
    assert_bumped(
        &conn,
        "activities",
        &b.id,
        &vb,
        "trigger project_space_unlink_activities (b)",
    );
    let links: i64 = conn
        .query_row(
            "SELECT count(*) FROM project_activities WHERE project_id = ?1",
            [&p],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(links, 0, "the trigger did remove the attachments");
    // An unattached activity is not touched by this trigger.
    assert_eq!(
        versions(&conn, "activities", &c.id),
        (0, c.created_at.clone())
    );
    queries::update_project_space(&conn, &p, Some(Work)).unwrap();

    // Deletion of an activity: every project that was attached to it advances.
    queries::set_project_activities(&conn, &p, std::slice::from_ref(&a.id), None).unwrap();
    queries::set_project_activities(&conn, &p2, std::slice::from_ref(&a.id), None).unwrap();
    let (vp, vp2) = (
        versions(&conn, "projects", &p),
        versions(&conn, "projects", &p2),
    );
    let vp3 = versions(&conn, "projects", &p3);
    pause();
    queries::delete_activity(&conn, &a.id).unwrap();
    assert_bumped(&conn, "projects", &p, &vp, "delete_activity (project p)");
    assert_bumped(&conn, "projects", &p2, &vp2, "delete_activity (project p2)");
    assert_eq!(
        versions(&conn, "projects", &p3),
        vp3,
        "an unattached project does not move"
    );
    assert!(
        queries::delete_activity(&conn, &a.id).is_err(),
        "unknown activity → error"
    );

    // Deletion of a project: every activity that contained it advances. (A project without notes or
    // space is refused: `p2` is in Work, its Inbox already exists.)
    queries::set_project_activities(&conn, &p2, std::slice::from_ref(&b.id), None).unwrap();
    let vb = versions(&conn, "activities", &b.id);
    pause();
    queries::delete_project(&conn, &p2).unwrap();
    assert_bumped(&conn, "activities", &b.id, &vb, "delete_project");
    assert_eq!(
        versions(&conn, "activities", &c.id),
        (0, c.created_at.clone())
    );
}

/// The two removed-attachment triggers (project space, activity space) advance
/// the other side **at the moment of deletion**, whatever path wrote. No application
/// code changes an activity's space: only a direct `UPDATE` reaches it.
#[test]
fn test_space_triggers_bump_the_other_side_of_removed_links() {
    use crate::models::project::ProjectSpace::Work;
    let conn = setup_test_db();
    let p = queries::create_project_in_space(&conn, "Alpha", None, Some(Work)).unwrap();
    let a = queries::create_activity(&conn, "Dev", Work).unwrap();
    queries::set_project_activities(&conn, &p, std::slice::from_ref(&a.id), None).unwrap();
    let (vp, va) = (
        versions(&conn, "projects", &p),
        versions(&conn, "activities", &a.id),
    );
    pause();
    conn.execute(
        "UPDATE activities SET space = 'private' WHERE id = ?1",
        [&a.id],
    )
    .unwrap();
    assert_bumped(&conn, "projects", &p, &vp, "activity_space_unlink_projects");
    assert_eq!(
        versions(&conn, "activities", &a.id),
        va,
        "the trigger only touches the other side"
    );
    let links: i64 = conn
        .query_row("SELECT count(*) FROM project_activities", [], |r| r.get(0))
        .unwrap();
    assert_eq!(links, 0);
}

/// A row inserted without `updated_at` (test, external tool) gets `created_at`: the column is
/// never empty, even if an insertion path forgets to fill it in.
#[test]
fn test_insert_without_updated_at_defaults_to_created_at() {
    let conn = setup_test_db();
    conn.execute(
        "INSERT INTO projects (id, name, created_at) VALUES ('p', 'P', '2026-01-02T03:04:05.006Z')",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO activities (id, name, space, created_at) VALUES ('a', 'A', 'work', '2026-02-03T04:05:06.007Z')", []).unwrap();
    assert_eq!(
        versions(&conn, "projects", "p"),
        (0, "2026-01-02T03:04:05.006Z".to_string())
    );
    assert_eq!(
        versions(&conn, "activities", "a"),
        (0, "2026-02-03T04:05:06.007Z".to_string())
    );
}

/// Brings a current database back to the **real v17** state: without `revision` or `updated_at` on projects
/// and activities, with the two original attachment triggers (v11).
fn downgrade_to_v17(conn: &Connection) {
    downgrade_to_v19(conn);
    conn.execute_batch(
        "DROP TRIGGER projects_default_updated_at;
         DROP TRIGGER activities_default_updated_at;
         DROP TRIGGER project_space_unlink_activities;
         DROP TRIGGER activity_space_unlink_projects;
         CREATE TRIGGER project_space_unlink_activities
         AFTER UPDATE OF space ON projects
         BEGIN
             DELETE FROM project_activities
             WHERE project_id = NEW.id AND activity_id NOT IN (
                 SELECT id FROM activities WHERE space = NEW.space
             );
         END;
         CREATE TRIGGER activity_space_unlink_projects
         AFTER UPDATE OF space ON activities
         BEGIN
             DELETE FROM project_activities
             WHERE activity_id = NEW.id AND project_id NOT IN (
                 SELECT id FROM projects WHERE space = NEW.space
             );
         END;
         ALTER TABLE projects DROP COLUMN revision;
         ALTER TABLE projects DROP COLUMN updated_at;
         ALTER TABLE activities DROP COLUMN revision;
         ALTER TABLE activities DROP COLUMN updated_at;
         PRAGMA user_version = 17;",
    )
    .unwrap();
}

/// (id, name, color, space, is_inbox, archived_at, created_at) — excluding the "legacy" row.
type ProjectRow = (
    String,
    String,
    String,
    Option<String>,
    i64,
    Option<String>,
    String,
);

/// Content (excluding versions) of projects, activities and attachments, to compare before/after.
struct Snapshot {
    projects: Vec<ProjectRow>,
    /// (id, name, space, created_at)
    activities: Vec<(String, String, String, String)>,
    links: Vec<(String, String)>,
}

fn snapshot(conn: &Connection) -> Snapshot {
    let projects = conn.prepare("SELECT id, name, color, space, is_inbox, archived_at, created_at FROM projects WHERE id <> 'legacy' ORDER BY id").unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))).unwrap()
        .collect::<Result<_, _>>().unwrap();
    let activities = conn
        .prepare("SELECT id, name, space, created_at FROM activities ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let links = conn
        .prepare("SELECT project_id, activity_id FROM project_activities ORDER BY 1, 2")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    Snapshot {
        projects,
        activities,
        links,
    }
}

/// Migration v17 → v18 of a representative database: all values are kept, the version
/// restarts at 0 and `updated_at` takes `created_at`; the migration is idempotent.
#[test]
fn test_migration_v18_versions_projects_and_activities_and_keeps_data() {
    use crate::models::project::ProjectSpace::{Private, Work};
    let conn = setup_test_db();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    let p1 =
        queries::create_project_in_space(&conn, "Website", Some("#ff0000"), Some(Work)).unwrap();
    let p2 = queries::create_project_in_space(&conn, "Holidays", None, Some(Private)).unwrap();
    let p3 = queries::create_project(&conn, "To sort", None).unwrap();
    let inbox = queries::get_or_create_inbox(&conn, Work).unwrap();
    let a1 = queries::create_activity(&conn, "Development", Work).unwrap();
    let a2 = queries::create_activity(&conn, "Hobbies", Private).unwrap();
    queries::set_project_activities(&conn, &p1, std::slice::from_ref(&a1.id), None).unwrap();
    queries::set_project_activities(&conn, &p2, std::slice::from_ref(&a2.id), None).unwrap();
    queries::set_project_archived(&conn, &p3, true).unwrap();
    let mut note = note_input(&p1, "A note", NoteType::Todo);
    note.content = Some("kept content".into());
    let note_id = queries::create_note(&conn, note).unwrap();
    // Distinct, old creation dates: the migration must keep them as they are.
    conn.execute_batch(
        "UPDATE projects SET created_at = '2025-03-04T05:06:07.008Z' WHERE name = 'Website';
         UPDATE projects SET created_at = '2025-04-05T06:07:08.009Z' WHERE name = 'Holidays';
         UPDATE projects SET created_at = '2025-05-06T07:08:09.010Z' WHERE name = 'To sort';
         UPDATE activities SET created_at = '2025-06-07T08:09:10.011Z' WHERE name = 'Development';
         UPDATE activities SET created_at = '2025-07-08T09:10:11.012Z' WHERE name = 'Hobbies';",
    )
    .unwrap();
    // Historical row whose `created_at` is empty: `updated_at` falls back to the migration instant.
    conn.execute(
        "INSERT INTO projects (id, name, created_at) VALUES ('legacy', 'No date', NULL)",
        [],
    )
    .unwrap();
    let before = snapshot(&conn);
    downgrade_to_v17(&conn);
    assert!(
        conn.prepare("SELECT revision FROM projects").is_err(),
        "the forged database is indeed at v17"
    );
    assert!(conn.prepare("SELECT updated_at FROM activities").is_err());

    schema::apply_migrations(&conn).unwrap();
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);

    // Nothing changed, except the two added columns.
    let after = snapshot(&conn);
    assert_eq!(after.projects, before.projects, "projects kept identical");
    assert_eq!(after.links, before.links, "attachments kept");
    assert_eq!(
        after.activities, before.activities,
        "activities kept identical"
    );
    assert_eq!(
        queries::get_note_by_id(&conn, &note_id).unwrap().content,
        "kept content"
    );

    // Version 0 and `updated_at = created_at` everywhere.
    for (id, .., created_at) in &after.projects {
        assert_eq!(
            versions(&conn, "projects", id),
            (0, created_at.clone()),
            "project {id}"
        );
    }
    for (id, .., created_at) in &after.activities {
        assert_eq!(
            versions(&conn, "activities", id),
            (0, created_at.clone()),
            "activity {id}"
        );
    }
    let (rev, updated) = versions(&conn, "projects", "legacy");
    assert_eq!(rev, 0);
    assert_iso_utc(&updated);
    assert!(
        after.projects.iter().any(|(id, ..)| id == &inbox),
        "the Inbox is kept"
    );
    assert_eq!(after.projects.len(), 4);

    // The models can be read again (excluding the undated historical row, which exists in no real database).
    conn.execute("DELETE FROM projects WHERE id = 'legacy'", [])
        .unwrap();
    assert!(queries::get_projects(&conn)
        .unwrap()
        .iter()
        .all(|p| p.revision == 0 && p.updated_at == p.created_at));
    assert!(queries::get_activities(&conn)
        .unwrap()
        .iter()
        .all(|a| a.revision == 0 && a.updated_at == a.created_at));

    // Idempotence: a second pass touches nothing.
    schema::apply_migrations(&conn).unwrap();
    assert_eq!(
        versions(&conn, "projects", &p1),
        (0, "2025-03-04T05:06:07.008Z".to_string())
    );

    // The migrated database does version its subsequent writes (recreated triggers included).
    let v = versions(&conn, "projects", &p1);
    pause();
    queries::rename_project(&conn, &p1, "Website 2").unwrap();
    assert_bumped(&conn, "projects", &p1, &v, "write after migration");
    let va = versions(&conn, "activities", &a1.id);
    pause();
    queries::update_project_space(&conn, &p1, Some(Private)).unwrap();
    assert_bumped(
        &conn,
        "activities",
        &a1.id,
        &va,
        "trigger recreated by the migration",
    );
}

// --- Image registry (v19, batch S3) ---

fn sha_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    crate::images::lower_hex(&Sha256::digest(bytes))
}

/// Content of all tables (excluding the `images` registry and the FTS index), value by value, and schema
/// objects (triggers, indexes): enough to prove that a migration changed nothing else.
const V22_NOTE_COLUMNS: [&str; 7] = [
    "parent_id",
    "content_kind",
    "canvas_x",
    "canvas_y",
    "canvas_w",
    "canvas_h",
    "order_key",
];

fn dump_database(conn: &Connection) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut dump = std::collections::BTreeMap::new();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'notes_fts%' AND name <> 'images' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for table in tables {
        // The v22 note columns are left out: tests of older migrations compare before/after
        // without them, and the v22 test checks them itself.
        let columns: Vec<String> = conn
            .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .into_iter()
            .filter(|c| table != "notes" || !V22_NOTE_COLUMNS.contains(&c.as_str()))
            .map(|c| format!("\"{c}\""))
            .collect();
        let mut statement = conn
            .prepare(&format!("SELECT {} FROM \"{table}\"", columns.join(", ")))
            .unwrap();
        let columns = statement.column_count();
        let mut rows: Vec<String> = statement
            .query_map([], |row| {
                Ok((0..columns)
                    .map(|i| format!("{:?}", row.get_ref(i).unwrap()))
                    .collect::<Vec<_>>()
                    .join("|"))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        rows.sort();
        dump.insert(table, rows);
    }
    let objects: Vec<String> = conn
        .prepare("SELECT type || ' ' || name || ' ' || COALESCE(sql, '') FROM sqlite_master WHERE type IN ('trigger', 'index') AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    dump.insert("<schema objects>".to_string(), objects);
    dump
}

/// Migration v18 → v19 of a representative database: the registry starts empty, no other data nor
/// any schema object moves, the migration is idempotent.
#[test]
fn test_migration_v19_creates_empty_image_registry_and_keeps_data() {
    use crate::models::project::ProjectSpace::{Private, Work};
    let conn = setup_test_db();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    let p1 =
        queries::create_project_in_space(&conn, "Website", Some("#ff0000"), Some(Work)).unwrap();
    let p2 = queries::create_project_in_space(&conn, "Holidays", None, Some(Private)).unwrap();
    queries::get_or_create_inbox(&conn, Work).unwrap();
    let activity = queries::create_activity(&conn, "Development", Work).unwrap();
    queries::set_project_activities(&conn, &p1, std::slice::from_ref(&activity.id), None).unwrap();
    let mut illustrated = note_input(&p1, "With image", NoteType::Todo);
    illustrated.content = Some("![screenshot](humcase-image://abc.png) text".into());
    illustrated.raw_content = Some("raw dictation".into());
    let illustrated_id = queries::create_note(&conn, illustrated).unwrap();
    let source_id = queries::create_note(&conn, note_input(&p1, "Source", NoteType::Note)).unwrap();
    crate::db::relations::link_note_task(&conn, &source_id, &illustrated_id).unwrap();
    let trashed =
        queries::create_note(&conn, note_input(&p2, "In the trash", NoteType::Idea)).unwrap();
    queries::trash_note(&conn, &trashed).unwrap();
    queries::update_note(
        &conn,
        &illustrated_id,
        UpdateNoteInput {
            content: Some("![screenshot](humcase-image://abc.png) modified text".into()),
            ..update_note_input(None, None, None, None)
        },
    )
    .unwrap();

    downgrade_to_v19(&conn);
    conn.execute_batch("DROP TABLE images; PRAGMA user_version = 18;")
        .unwrap();
    assert!(
        conn.prepare("SELECT name FROM images").is_err(),
        "the forged database is indeed at v18"
    );
    let before = dump_database(&conn);
    assert_eq!(before["notes"].len(), 3, "representative database");
    assert_eq!(before["note_task_links"].len(), 1);

    schema::apply_migrations(&conn).unwrap();

    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);
    // The v20 migration that follows adds its own objects (checked by its test): they are excluded.
    let before = without_v20(before);
    assert_eq!(
        without_v20(dump_database(&conn)),
        before,
        "data and schema objects identical"
    );
    let columns: Vec<(String, String, i64, i64)> = conn
        .prepare("SELECT name, type, \"notnull\", pk FROM pragma_table_info('images') ORDER BY cid")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        columns,
        vec![
            ("name".to_string(), "TEXT".to_string(), 0, 1),
            ("sha256".to_string(), "TEXT".to_string(), 1, 0),
            ("size".to_string(), "INTEGER".to_string(), 1, 0),
            ("created_at".to_string(), "TEXT".to_string(), 1, 0),
        ]
    );
    let rows: i64 = conn
        .query_row("SELECT count(*) FROM images", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        rows, 0,
        "the migration does not read the disk: the registry starts empty"
    );
    assert_eq!(
        queries::get_note_by_id(&conn, &illustrated_id)
            .unwrap()
            .content,
        "![screenshot](humcase-image://abc.png) modified text"
    );

    schema::apply_migrations(&conn).unwrap();
    assert_eq!(without_v20(dump_database(&conn)), before, "idempotent");
}

fn registry_rows(conn: &Connection) -> Vec<(String, String, i64, String)> {
    conn.prepare("SELECT name, sha256, size, created_at FROM images ORDER BY name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// A saved image is recorded in the registry with the fingerprint and size of the written file.
#[test]
fn test_saved_image_is_registered_with_hash_and_size() {
    let dir = tempfile::tempdir().unwrap();
    let images = dir.path().join("images");
    let db = std::sync::Mutex::new(crate::db::init_db(dir.path()).unwrap());
    let prepared = crate::images::PreparedImage {
        bytes: b"image bytes 123".to_vec(),
        extension: "png",
    };

    let reference = crate::images::store_image(&db, &images, &prepared).unwrap();

    let name = reference.strip_prefix(crate::images::SCHEME).unwrap();
    assert_eq!(
        std::fs::read(images.join(name)).unwrap(),
        b"image bytes 123"
    );
    let rows = registry_rows(&db.lock().unwrap());
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].0.as_str(), rows[0].1.as_str(), rows[0].2),
        (name, sha_hex(b"image bytes 123").as_str(), 15)
    );
    assert!(
        rows[0].3.ends_with('Z') && rows[0].3.contains('T'),
        "ISO UTC date: {}",
        rows[0].3
    );
    let leftovers: Vec<_> = std::fs::read_dir(&images)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(leftovers.len(), 1, "no temporary file: {leftovers:?}");

    // A second image, a second record: never the same name.
    let second = crate::images::store_image(&db, &images, &prepared).unwrap();
    assert_ne!(second, reference);
    assert_eq!(registry_rows(&db.lock().unwrap()).len(), 2);
}

/// At startup (`init_db`), the registry is filled from the existing folder; what cannot
/// be recorded is ignored and **kept**; a second pass changes nothing.
#[test]
fn test_startup_fills_registry_from_existing_folder_and_keeps_unreadable_entries() {
    let dir = tempfile::tempdir().unwrap();
    let images = dir.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    std::fs::write(images.join("aaa.png"), b"first").unwrap();
    std::fs::write(images.join("bbb.webp"), b"second image").unwrap();
    // Known modification date: it becomes the registration date of an existing file.
    std::fs::File::options()
        .write(true)
        .open(images.join("aaa.png"))
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_577_934_245))
        .unwrap();
    // Entries to ignore without ever touching them: folder, non-conforming name, write in progress.
    std::fs::create_dir(images.join("folder.png")).unwrap();
    std::fs::write(images.join("folder.png").join("inside.png"), b"inside").unwrap();
    std::fs::write(images.join("name with space.png"), b"space").unwrap();
    std::fs::write(images.join("in-progress.tmp"), b"partial").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::{symlink, PermissionsExt};
        std::fs::write(dir.path().join("outside.png"), b"outside").unwrap();
        symlink(dir.path().join("outside.png"), images.join("link.png")).unwrap();
        std::fs::write(images.join("secret.png"), b"locked").unwrap();
        std::fs::set_permissions(
            images.join("secret.png"),
            std::fs::Permissions::from_mode(0o000),
        )
        .unwrap();
    }
    // Under an account that ignores permissions (root), `secret.png` is readable and legitimately registered.
    let secret_readable = std::fs::read(images.join("secret.png")).is_ok();

    let conn = crate::db::init_db(dir.path()).unwrap();

    let rows = registry_rows(&conn);
    let mut expected = vec!["aaa.png", "bbb.webp"];
    if secret_readable {
        expected.push("secret.png");
    }
    assert_eq!(
        rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
        expected,
        "{rows:?}"
    );
    assert_eq!(
        (rows[0].1.as_str(), rows[0].2),
        (sha_hex(b"first").as_str(), 5)
    );
    assert_eq!(
        rows[0].3, "2020-01-02T03:04:05.000Z",
        "file modification date"
    );
    assert_eq!(
        (rows[1].1.as_str(), rows[1].2),
        (sha_hex(b"second image").as_str(), 12)
    );
    // Nothing was deleted, moved or modified.
    for (name, content) in [
        ("aaa.png", &b"first"[..]),
        ("bbb.webp", b"second image"),
        ("name with space.png", b"space"),
        ("in-progress.tmp", b"partial"),
    ] {
        assert_eq!(std::fs::read(images.join(name)).unwrap(), content, "{name}");
    }
    assert_eq!(
        std::fs::read(images.join("folder.png").join("inside.png")).unwrap(),
        b"inside"
    );
    #[cfg(unix)]
    assert!(images
        .join("link.png")
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());

    // Idempotent: a second pass adds nothing and does not re-read what is already registered.
    assert_eq!(crate::images::sync_registry(&conn, &images).unwrap(), 0);
    assert_eq!(registry_rows(&conn), rows);
    // An image added between two startups is caught up, the existing one keeps its row.
    std::fs::write(images.join("ccc.png"), b"third").unwrap();
    assert_eq!(crate::images::sync_registry(&conn, &images).unwrap(), 1);
    let after = registry_rows(&conn);
    assert_eq!(after.iter().filter(|r| r.0 == "ccc.png").count(), 1);
    assert_eq!(&after[0], &rows[0]);
    // The unreadable file is still there, intact, and never registered as long as it cannot be read.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(images.join("secret.png"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0
        );
        std::fs::set_permissions(
            images.join("secret.png"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        assert_eq!(std::fs::read(images.join("secret.png")).unwrap(), b"locked");
        assert_eq!(after.iter().any(|r| r.0 == "secret.png"), secret_readable);
    }
}

/// A missing folder is not an error (first use) and is not created by the synchronization.
#[test]
fn test_startup_without_image_folder_is_a_noop() {
    let dir = tempfile::tempdir().unwrap();
    let conn = crate::db::init_db(dir.path()).unwrap();
    assert!(registry_rows(&conn).is_empty());
    assert!(!dir.path().join("images").exists());
}

// --- Raw dictation in fragments (v20, batch S6) ---

/// Brings a current database back to the **real v20** state: without the deletion trace table nor
/// its triggers (v21).
fn downgrade_to_v20(conn: &Connection) {
    downgrade_to_v21(conn);
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS projects_trace_delete;
         DROP TRIGGER IF EXISTS projects_untrace_insert;
         DROP TRIGGER IF EXISTS activities_trace_delete;
         DROP TRIGGER IF EXISTS activities_untrace_insert;
         DROP TRIGGER IF EXISTS note_task_links_trace_delete;
         DROP TRIGGER IF EXISTS note_task_links_untrace_insert;
         DROP TRIGGER IF EXISTS project_activities_trace_delete;
         DROP TRIGGER IF EXISTS project_activities_untrace_insert;
         DROP TABLE IF EXISTS deletions;
         PRAGMA user_version = 20;",
    )
    .unwrap();
}

/// Brings a current database back to the **real v22** state: Inbox containers under their
/// former technical name.
fn downgrade_to_v22(conn: &Connection) {
    conn.execute_batch(
        "UPDATE projects SET name = '__devnotes_inbox_' || substr(name, 17)
          WHERE is_inbox = 1 AND substr(name, 1, 16) = '__humcase_inbox_';
         PRAGMA user_version = 22;",
    )
    .unwrap();
}

/// Brings a current database back to the **real v21** state: without sub-case columns, triggers
/// or provenance (v22).
fn downgrade_to_v21(conn: &Connection) {
    // Inbox names are left current: the tests of older migrations compare before/after
    // snapshots, and v23 has its own test.
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS sub_case_same_project_insert;
         DROP TRIGGER IF EXISTS sub_case_same_project_update;
         DROP TRIGGER IF EXISTS sub_case_parent_purged;
         DROP TRIGGER IF EXISTS sub_case_no_cycle;
         DROP TRIGGER IF EXISTS case_provenance_no_update;
         DROP TRIGGER IF EXISTS case_provenance_no_delete;
         DROP TABLE IF EXISTS case_provenance;
         DROP INDEX IF EXISTS idx_notes_parent;
         ALTER TABLE notes DROP COLUMN parent_id;
         ALTER TABLE notes DROP COLUMN content_kind;
         ALTER TABLE notes DROP COLUMN canvas_x;
         ALTER TABLE notes DROP COLUMN canvas_y;
         ALTER TABLE notes DROP COLUMN canvas_w;
         ALTER TABLE notes DROP COLUMN canvas_h;
         ALTER TABLE notes DROP COLUMN order_key;
         PRAGMA user_version = 21;",
    )
    .unwrap();
}

/// Brings a current database back to the **real v19** state: without fragments table or triggers (and,
/// before that, without the v21 objects).
fn downgrade_to_v19(conn: &Connection) {
    downgrade_to_v20(conn);
    conn.execute_batch(
        "DROP TRIGGER dictation_fragments_no_update;
         DROP TRIGGER dictation_fragments_no_delete;
         DROP TABLE dictation_fragments;
         PRAGMA user_version = 19;",
    )
    .unwrap();
}

/// Removes from a snapshot what the v21 migration adds (trace table and its triggers).
fn without_v21(
    mut dump: std::collections::BTreeMap<String, Vec<String>>,
) -> std::collections::BTreeMap<String, Vec<String>> {
    dump.remove("deletions");
    if let Some(objects) = dump.get_mut("<schema objects>") {
        objects.retain(|o| !o.contains("deletions"));
    }
    without_v22(dump)
}

/// Removes from a snapshot the v22 objects (provenance table, sub-case triggers and index);
/// the v22 note columns never enter a snapshot (`V22_NOTE_COLUMNS`).
fn without_v22(
    mut dump: std::collections::BTreeMap<String, Vec<String>>,
) -> std::collections::BTreeMap<String, Vec<String>> {
    dump.remove("case_provenance");
    if let Some(objects) = dump.get_mut("<schema objects>") {
        objects.retain(|o| {
            !o.contains("case_provenance")
                && !o.contains("sub_case_")
                && !o.contains("idx_notes_parent")
        });
    }
    dump
}

/// Removes from a snapshot what the v20 migrations (fragments table and its objects) then v21
/// add: tests of older migrations compare before/after without these objects, which
/// their own test checks.
fn without_v20(
    mut dump: std::collections::BTreeMap<String, Vec<String>>,
) -> std::collections::BTreeMap<String, Vec<String>> {
    dump.remove("dictation_fragments");
    if let Some(objects) = dump.get_mut("<schema objects>") {
        objects.retain(|o| !o.contains("dictation_fragments"));
    }
    without_v21(dump)
}

fn fragment_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM dictation_fragments", [], |r| r.get(0))
        .unwrap()
}

fn fragment_texts(conn: &Connection, note_id: &str) -> Vec<String> {
    queries::dictation_fragments(conn, note_id)
        .unwrap()
        .into_iter()
        .map(|(_, text, _)| text)
        .collect()
}

/// A note's fragments, in read order, exactly recompose its `raw_content`.
fn assert_fragments_match_raw(conn: &Connection, note_id: &str) {
    let raw = queries::get_note_by_id(conn, note_id)
        .unwrap()
        .raw_content
        .unwrap_or_default();
    let texts = fragment_texts(conn, note_id);
    assert_eq!(
        queries::raw_content_from_fragments(texts.iter().map(String::as_str)),
        raw,
        "note {note_id}"
    );
}

/// Each append writes a fragment **and** the same `raw_content` as before v20: separator of two
/// line breaks, first append taken as is (raw text absent or empty), one revision per append.
#[test]
fn test_append_raw_content_writes_one_fragment_and_the_same_raw_content_as_before() {
    let conn = setup_test_db();
    let project = queries::create_project(&conn, "Dictation", None).unwrap();
    let id = queries::create_note(&conn, note_input(&project, "Note", NoteType::Note)).unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &id).unwrap().raw_content,
        None
    );

    let expected = [
        ("first", "first"),
        ("second", "first\n\nsecond"),
        ("", "first\n\nsecond\n\n"),
        (
            "after an empty one",
            "first\n\nsecond\n\n\n\nafter an empty one",
        ),
    ];
    for (revision, (text, raw)) in expected.iter().enumerate() {
        queries::append_raw_content(&conn, &id, text).unwrap();
        let note = queries::get_note_by_id(&conn, &id).unwrap();
        assert_eq!(note.raw_content.as_deref(), Some(*raw));
        assert_eq!(
            note.revision,
            revision as i64 + 1,
            "one revision per append"
        );
        assert_eq!(
            fragment_texts(&conn, &id).last().map(String::as_str),
            Some(*text)
        );
        assert_fragments_match_raw(&conn, &id);
    }
    assert_eq!(
        fragment_texts(&conn, &id),
        vec!["first", "second", "", "after an empty one"]
    );

    // First append on an empty (not absent) raw text: taken as is, as before.
    let empty = queries::create_note(&conn, note_input(&project, "Empty", NoteType::Note)).unwrap();
    conn.execute("UPDATE notes SET raw_content = '' WHERE id = ?1", [&empty])
        .unwrap();
    queries::append_raw_content(&conn, &empty, "alone").unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &empty)
            .unwrap()
            .raw_content
            .as_deref(),
        Some("alone")
    );
    assert_fragments_match_raw(&conn, &empty);

    // Fragments are dated in the database's ISO format.
    for (_, _, date) in queries::dictation_fragments(&conn, &id).unwrap() {
        assert_iso_utc(&date);
    }
}

/// Closely spaced appends (same millisecond) and a clock that goes backwards never reorder the
/// read: `created_at` is strictly increasing per note, the recomposition stays exact.
#[test]
fn test_fragments_keep_append_order_within_a_millisecond_and_after_clock_regression() {
    let conn = setup_test_db();
    let project = queries::create_project(&conn, "Order", None).unwrap();
    let id = queries::create_note(&conn, note_input(&project, "Burst", NoteType::Note)).unwrap();
    for i in 0..200 {
        queries::append_raw_content(&conn, &id, &format!("fragment {i}")).unwrap();
    }
    let fragments = queries::dictation_fragments(&conn, &id).unwrap();
    let texts: Vec<String> = fragments.iter().map(|(_, t, _)| t.clone()).collect();
    assert_eq!(
        texts,
        (0..200)
            .map(|i| format!("fragment {i}"))
            .collect::<Vec<_>>(),
        "append order"
    );
    assert!(
        fragments.windows(2).all(|w| w[0].2 < w[1].2),
        "created_at strictly increasing"
    );
    assert_fragments_match_raw(&conn, &id);

    // A fragment dated in the future (clock from another moment, or that has gone back since): the
    // next append is placed right after it, never before.
    let other = queries::create_note(&conn, note_input(&project, "Clock", NoteType::Note)).unwrap();
    queries::append_raw_content(&conn, &other, "first").unwrap();
    conn.execute(
        "INSERT INTO dictation_fragments (id, note_id, text, created_at) VALUES ('future', ?1, 'future', '2999-12-31T23:59:59.999Z')",
        [&other],
    ).unwrap();
    conn.execute("UPDATE notes SET raw_content = raw_content || char(10) || char(10) || 'future' WHERE id = ?1", [&other]).unwrap();
    queries::append_raw_content(&conn, &other, "then").unwrap();
    let fragments = queries::dictation_fragments(&conn, &other).unwrap();
    assert_eq!(fragments.last().unwrap().1, "then");
    assert_eq!(fragments.last().unwrap().2, "3000-01-01T00:00:00.000Z");
    assert_fragments_match_raw(&conn, &other);
}

/// Append-only: directly modifying or deleting a fragment is refused by SQLite; only
/// the permanent erasure of the note (trash, with trace) takes them away, by cascade.
#[test]
fn test_fragments_are_append_only_and_leave_only_with_their_purged_note() {
    let conn = setup_test_db();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    let project = queries::create_project(&conn, "Append-only", None).unwrap();
    let id =
        queries::create_note(&conn, note_input(&project, "Dictation", NoteType::Note)).unwrap();
    let neighbor =
        queries::create_note(&conn, note_input(&project, "Neighbor", NoteType::Note)).unwrap();
    queries::append_raw_content(&conn, &id, "one").unwrap();
    queries::append_raw_content(&conn, &id, "two").unwrap();
    queries::append_raw_content(&conn, &neighbor, "to keep").unwrap();

    for sql in [
        "UPDATE dictation_fragments SET text = 'replaced'",
        "UPDATE dictation_fragments SET created_at = '2000-01-01T00:00:00.000Z'",
        "UPDATE dictation_fragments SET note_id = note_id",
        "DELETE FROM dictation_fragments",
        "DELETE FROM dictation_fragments WHERE text = 'one'",
    ] {
        let error = conn.execute(sql, []).unwrap_err().to_string();
        assert!(
            error.contains("dictation fragments are append-only"),
            "{sql}: {error}"
        );
    }
    assert_eq!(fragment_texts(&conn, &id), vec!["one", "two"]);
    assert_fragments_match_raw(&conn, &id);

    // Moving to the trash: nothing leaves. Permanent erasure: fragments taken away, trace kept.
    queries::trash_note(&conn, &id).unwrap();
    assert_eq!(fragment_count(&conn), 3);
    assert_eq!(
        queries::purge_trash(&conn, queries::TrashPurge::One(&id)).unwrap(),
        1
    );
    assert!(
        fragment_texts(&conn, &id).is_empty(),
        "the cascade is not blocked by the triggers"
    );
    let trace: i64 = conn
        .query_row(
            "SELECT count(*) FROM note_tombstones WHERE id = ?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(trace, 1, "permanent erasure stays traced");
    assert_eq!(
        fragment_texts(&conn, &neighbor),
        vec!["to keep"],
        "another note's fragments remain"
    );
}

/// Note in the trash: the append is refused as before (v17 trigger), with no fragment or revision.
/// Nonexistent note: same error as before (`notes.notFound`), no orphan fragment.
#[test]
fn test_append_refused_on_trashed_or_missing_note_leaves_no_fragment() {
    let conn = setup_test_db();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    let project = queries::create_project(&conn, "Refusal", None).unwrap();
    let id = queries::create_note(&conn, note_input(&project, "Frozen", NoteType::Note)).unwrap();
    queries::append_raw_content(&conn, &id, "before the trash").unwrap();
    queries::trash_note(&conn, &id).unwrap();
    let before = queries::get_note_by_id(&conn, &id).unwrap();

    let error = queries::append_raw_content(&conn, &id, "refused").unwrap_err();
    assert!(error.contains("note is in the trash"), "{error}");
    let after = queries::get_note_by_id(&conn, &id).unwrap();
    assert_eq!(after.raw_content, before.raw_content);
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.updated_at, before.updated_at);
    assert_eq!(
        fragment_texts(&conn, &id),
        vec!["before the trash"],
        "no fragment added"
    );

    let error = crate::i18n::with_language("fr", || {
        queries::append_raw_content(&conn, "missing-note", "lost?")
    });
    assert_eq!(
        error.unwrap_err(),
        crate::i18n::with_language("fr", || crate::tr!("notes.notFound", id = "missing-note"))
    );
    assert_eq!(fragment_count(&conn), 1, "no orphan fragment");

    // Same guarantee without foreign key checking: the transaction rolls back the append.
    conn.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
    assert!(queries::append_raw_content(&conn, "missing-note", "lost?").is_err());
    assert_eq!(fragment_count(&conn), 1);
}

/// `update_note` (IPC), its conditional variant and the escrow replay refuse `raw_content`
/// without writing anything, even accompanied by another field.
#[test]
fn test_update_note_refuses_raw_content_without_writing() {
    let conn = setup_test_db();
    let project = queries::create_project(&conn, "Refusal", None).unwrap();
    let id = queries::create_note(&conn, note_input(&project, "Title", NoteType::Note)).unwrap();
    queries::append_raw_content(&conn, &id, "original dictation").unwrap();
    let before = queries::get_note_by_id(&conn, &id).unwrap();
    let replacement = || UpdateNoteInput {
        raw_content: Some("overwritten".into()),
        title: Some("New title".into()),
        ..update_note_input(None, None, None, None)
    };

    let refuse =
        |r: Result<(), String>| assert!(r.unwrap_err().contains("raw_content cannot be replaced"));
    refuse(queries::update_note(&conn, &id, replacement()));
    refuse(queries::update_note(
        &conn,
        &id,
        UpdateNoteInput {
            raw_content: Some(String::new()),
            ..update_note_input(None, None, None, None)
        },
    ));
    refuse(queries::update_note_if_report_unchanged(
        &conn,
        &id,
        replacement(),
        Some(""),
    ));
    let replay = queries::retry_note_update(
        &conn,
        &id,
        replacement(),
        serde_json::json!({ "raw_content": "original dictation", "title": "Title" }),
    );
    assert!(replay
        .unwrap_err()
        .contains("raw_content cannot be replaced"));
    let replay =
        queries::retry_note_update(&conn, "missing-note", replacement(), serde_json::json!({}));
    assert!(
        replay
            .unwrap_err()
            .contains("raw_content cannot be replaced"),
        "refusal before looking up the note"
    );

    let after = queries::get_note_by_id(&conn, &id).unwrap();
    assert_eq!(after.raw_content.as_deref(), Some("original dictation"));
    assert_eq!(after.title, "Title", "no field is written");
    assert_eq!(after.revision, before.revision);
    assert_eq!(fragment_texts(&conn, &id), vec!["original dictation"]);
}

/// A note created with a raw dictation (direct creation, or inside the transaction of a linked
/// task) gets its first fragment; without dictation, no fragment.
#[test]
fn test_create_note_with_raw_content_writes_its_first_fragment() {
    let conn = setup_test_db();
    let project = queries::create_project(&conn, "Creation", None).unwrap();
    let mut input = note_input(&project, "Dictation", NoteType::Note);
    input.raw_content = Some("dictated at creation".into());
    let id = queries::create_note(&conn, input).unwrap();
    assert_eq!(fragment_texts(&conn, &id), vec!["dictated at creation"]);
    queries::append_raw_content(&conn, &id, "then appended").unwrap();
    assert_fragments_match_raw(&conn, &id);

    let mut empty = note_input(&project, "Empty", NoteType::Note);
    empty.raw_content = Some(String::new());
    let empty = queries::create_note(&conn, empty).unwrap();
    assert!(fragment_texts(&conn, &empty).is_empty());
    assert_fragments_match_raw(&conn, &empty);

    // Inside an existing transaction (linked task path): no nested transaction.
    let tx = conn.unchecked_transaction().unwrap();
    let mut in_tx = note_input(&project, "In a transaction", NoteType::Todo);
    in_tx.raw_content = Some("transactional raw text".into());
    let in_tx = queries::create_note(&tx, in_tx).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        fragment_texts(&conn, &in_tx),
        vec!["transactional raw text"]
    );
    let task = crate::db::relations::create_linked_task(&conn, &id, "Task", None).unwrap();
    assert!(fragment_texts(&conn, &task).is_empty());
    assert!(queries::notes_with_inconsistent_fragments(&conn)
        .unwrap()
        .is_empty());
}

/// Migration v19 → v20 of a representative database: one fragment per non-empty `raw_content`, with identical
/// text and dated from `updated_at`; no other data nor any other object changes; the migrated
/// database continues append-only; the migration is idempotent.
#[test]
fn test_migration_v20_turns_each_raw_content_into_one_identical_fragment() {
    use crate::models::project::ProjectSpace::{Private, Work};
    let conn = setup_test_db();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    let p1 =
        queries::create_project_in_space(&conn, "Website", Some("#ff0000"), Some(Work)).unwrap();
    let p2 = queries::create_project_in_space(&conn, "Holidays", None, Some(Private)).unwrap();
    let activity = queries::create_activity(&conn, "Development", Work).unwrap();
    queries::set_project_activities(&conn, &p1, std::slice::from_ref(&activity.id), None).unwrap();
    let dictated = queries::create_note(
        &conn,
        note_input(&p1, "Dictation in three parts", NoteType::Todo),
    )
    .unwrap();
    for text in [
        "first draft",
        "  second, with spaces  ",
        "![view](humcase-image://abc.png) third",
    ] {
        queries::append_raw_content(&conn, &dictated, text).unwrap();
    }
    let no_raw =
        queries::create_note(&conn, note_input(&p1, "Without dictation", NoteType::Note)).unwrap();
    crate::db::relations::link_note_task(&conn, &no_raw, &dictated).unwrap();
    let empty_raw =
        queries::create_note(&conn, note_input(&p2, "Empty raw text", NoteType::Idea)).unwrap();
    conn.execute(
        "UPDATE notes SET raw_content = '' WHERE id = ?1",
        [&empty_raw],
    )
    .unwrap();
    let in_trash =
        queries::create_note(&conn, note_input(&p2, "In the trash", NoteType::Note)).unwrap();
    queries::append_raw_content(&conn, &in_trash, "dictation of a discarded note").unwrap();
    queries::trash_note(&conn, &in_trash).unwrap();
    // Historical raw text with lines and separators: never split.
    let historical =
        queries::create_note(&conn, note_input(&p2, "Historical", NoteType::Note)).unwrap();
    conn.execute(
        "UPDATE notes SET raw_content = ?1, updated_at = '2025-01-02T03:04:05.006Z' WHERE id = ?2",
        rusqlite::params!["line 1\nline 2\n\nparagraph\n\n\n", historical],
    )
    .unwrap();

    downgrade_to_v19(&conn);
    assert!(
        conn.prepare("SELECT id FROM dictation_fragments").is_err(),
        "the forged database is indeed at v19"
    );
    let before = dump_database(&conn);
    assert_eq!(before["notes"].len(), 5, "representative database");

    schema::apply_migrations(&conn).unwrap();
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);
    assert_eq!(
        without_v20(dump_database(&conn)),
        before,
        "no other data nor any other object changes"
    );

    let fragments: Vec<(String, String, String)> = conn
        .prepare("SELECT note_id, text, created_at FROM dictation_fragments ORDER BY note_id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut expected: Vec<(String, String, String)> = conn
        .prepare(
            "SELECT id, raw_content, updated_at FROM notes WHERE raw_content <> '' ORDER BY id",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    expected.sort();
    assert_eq!(
        expected.len(),
        3,
        "dictation, trash and historical; neither without raw text nor empty raw text"
    );
    assert_eq!(
        fragments, expected,
        "one fragment per non-empty raw text, identical text, dated from updated_at"
    );
    assert!(queries::notes_with_inconsistent_fragments(&conn)
        .unwrap()
        .is_empty());

    // The migrated database is protected and continues by appending, in order.
    assert!(conn
        .execute("UPDATE dictation_fragments SET text = 'x'", [])
        .is_err());
    assert!(conn.execute("DELETE FROM dictation_fragments", []).is_err());
    queries::append_raw_content(&conn, &historical, "append after migration").unwrap();
    assert_eq!(fragment_texts(&conn, &historical).len(), 2);
    assert_fragments_match_raw(&conn, &historical);

    let after = dump_database(&conn);
    schema::apply_migrations(&conn).unwrap();
    assert_eq!(dump_database(&conn), after, "idempotent");
}

/// For any note written through the application's paths (creation, appends, editing, status,
/// structuring, trash, move), the fragments exactly recompose `raw_content`.
#[test]
fn test_fragments_always_recompose_raw_content_through_application_writes() {
    use crate::models::project::ProjectSpace;
    let conn = setup_test_db();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    let inbox = queries::get_or_create_inbox(&conn, ProjectSpace::Work).unwrap();
    let target =
        queries::create_project_in_space(&conn, "Target", None, Some(ProjectSpace::Work)).unwrap();
    let mut ids = Vec::new();
    for n in 0..5 {
        let mut input = note_input(&inbox, &format!("Note {n}"), NoteType::Idea);
        input.content = Some("original".into());
        if n % 2 == 0 {
            input.raw_content = Some(format!("creation {n}"));
        }
        let id = queries::create_note(&conn, input).unwrap();
        for k in 0..n {
            queries::append_raw_content(&conn, &id, &format!("note {n}, append {k}\non two lines"))
                .unwrap();
        }
        ids.push(id);
    }
    queries::update_note(
        &conn,
        &ids[1],
        update_note_input(Some(NoteStatus::Resolved), Some("P1"), None, Some("done")),
    )
    .unwrap();
    queries::append_note_content(&conn, &ids[2], "added content", None).unwrap();
    crate::db::structure::apply_structured_note(&conn, &ids[3], "original", "structured").unwrap();
    queries::move_note_to_project(&conn, &ids[4], &target).unwrap();
    queries::append_raw_content(&conn, &ids[4], "after move").unwrap();
    queries::trash_note(&conn, &ids[0]).unwrap();
    queries::restore_from_trash(&conn, &ids[0]).unwrap();
    queries::append_raw_content(&conn, &ids[0], "after the trash").unwrap();

    for id in &ids {
        assert_fragments_match_raw(&conn, id);
    }
    assert!(queries::notes_with_inconsistent_fragments(&conn)
        .unwrap()
        .is_empty());
    // And the check does detect a discrepancy (guard used by archive restore).
    conn.execute(
        "UPDATE notes SET raw_content = raw_content || '!' WHERE id = ?1",
        [&ids[2]],
    )
    .unwrap();
    assert_eq!(
        queries::notes_with_inconsistent_fragments(&conn).unwrap(),
        vec![ids[2].clone()]
    );
}

// --- General deletion traces (v21, batch S2) ---

/// All `(entity, identifier)` traces, sorted.
fn traces(conn: &Connection) -> Vec<(String, String)> {
    conn.prepare("SELECT entity, entity_id FROM deletions ORDER BY entity, entity_id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// Composite key of a trace: JSON array of the primary key, in the table's order.
fn key(a: &str, b: &str) -> String {
    serde_json::to_string(&[a, b]).unwrap()
}

fn trace(entity: &str, id: &str) -> (String, String) {
    (entity.to_string(), id.to_string())
}

fn sorted(mut traces: Vec<(String, String)>) -> Vec<(String, String)> {
    traces.sort();
    traces
}

fn deletions_conn() -> Connection {
    let conn = setup_test_db();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    conn
}

/// Deleting a project leaves **one** project trace; its linked notes join the Inbox
/// (links removed then restored in the same transaction) without leaving a link trace.
#[test]
fn test_deleting_a_project_leaves_one_trace_and_no_link_trace() {
    use crate::models::project::ProjectSpace::Work;
    let conn = deletions_conn();
    let p = queries::create_project_in_space(&conn, "Alpha", None, Some(Work)).unwrap();
    let a = queries::create_note(&conn, note_input(&p, "Source", NoteType::Note)).unwrap();
    let b = queries::create_note(&conn, note_input(&p, "Task", NoteType::Todo)).unwrap();
    crate::db::relations::link_note_task(&conn, &a, &b).unwrap();
    assert!(traces(&conn).is_empty(), "nothing has been deleted");

    assert_eq!(queries::delete_project(&conn, &p).unwrap(), 2);

    assert_eq!(traces(&conn), vec![trace("project", &p)]);
    let links: i64 = conn
        .query_row("SELECT count(*) FROM note_task_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(links, 1, "the link was restored in the Inbox");
}

/// Deleting a project attached to activities also traces each attachment removed by cascade.
#[test]
fn test_deleting_a_project_traces_its_attachments_removed_by_cascade() {
    use crate::models::project::ProjectSpace::Work;
    let conn = deletions_conn();
    let p = queries::create_project_in_space(&conn, "Alpha", None, Some(Work)).unwrap();
    let a1 = queries::create_activity(&conn, "One", Work).unwrap();
    let a2 = queries::create_activity(&conn, "Two", Work).unwrap();
    let other = queries::create_project_in_space(&conn, "Beta", None, Some(Work)).unwrap();
    queries::set_project_activities(&conn, &p, &[a1.id.clone(), a2.id.clone()], None).unwrap();
    queries::set_project_activities(&conn, &other, std::slice::from_ref(&a1.id), None).unwrap();

    queries::delete_project(&conn, &p).unwrap();

    assert_eq!(
        traces(&conn),
        sorted(vec![
            trace("project", &p),
            trace("project_activity", &key(&p, &a1.id)),
            trace("project_activity", &key(&p, &a2.id)),
        ]),
        "the other project's attachment is not traced"
    );
}

/// Deleting an activity leaves its trace and **one trace per removed attachment** (cascade).
#[test]
fn test_deleting_an_activity_leaves_one_trace_per_removed_attachment() {
    use crate::models::project::ProjectSpace::Work;
    let conn = deletions_conn();
    let p1 = queries::create_project_in_space(&conn, "Alpha", None, Some(Work)).unwrap();
    let p2 = queries::create_project_in_space(&conn, "Beta", None, Some(Work)).unwrap();
    let target = queries::create_activity(&conn, "To delete", Work).unwrap();
    let kept = queries::create_activity(&conn, "Kept", Work).unwrap();
    queries::set_project_activities(&conn, &p1, &[target.id.clone(), kept.id.clone()], None)
        .unwrap();
    queries::set_project_activities(&conn, &p2, std::slice::from_ref(&target.id), None).unwrap();

    queries::delete_activity(&conn, &target.id).unwrap();

    assert_eq!(
        traces(&conn),
        sorted(vec![
            trace("activity", &target.id),
            trace("project_activity", &key(&p1, &target.id)),
            trace("project_activity", &key(&p2, &target.id)),
        ])
    );
}

/// Removing a note → task link traces it by its composite key; restoring it erases the trace.
#[test]
fn test_unlinking_notes_traces_the_link_and_relinking_erases_the_trace() {
    let conn = deletions_conn();
    let p = queries::create_project(&conn, "Project", None).unwrap();
    let a = queries::create_note(&conn, note_input(&p, "Source", NoteType::Note)).unwrap();
    let b = queries::create_note(&conn, note_input(&p, "Task", NoteType::Todo)).unwrap();
    let c = queries::create_note(&conn, note_input(&p, "Other task", NoteType::Todo)).unwrap();
    crate::db::relations::link_note_task(&conn, &a, &b).unwrap();
    crate::db::relations::link_note_task(&conn, &a, &c).unwrap();

    crate::db::relations::unlink_note_task(&conn, &a, &b).unwrap();
    assert_eq!(
        traces(&conn),
        vec![trace("note_task_link", &key(&a, &b))],
        "the link's direction matters"
    );

    // The reverse link is refused (cycle): it is another item, the first one's trace remains.
    crate::db::relations::link_note_task(&conn, &b, &a).unwrap_err();
    assert_eq!(traces(&conn), vec![trace("note_task_link", &key(&a, &b))]);

    crate::db::relations::link_note_task(&conn, &a, &b).unwrap();
    assert!(
        traces(&conn).is_empty(),
        "the recreated link is no longer deleted"
    );
}

/// Removing then restoring a project ↔ activity attachment: trace, then no trace. A save
/// without changes leaves none.
#[test]
fn test_removing_an_attachment_traces_it_and_restoring_it_erases_the_trace() {
    use crate::models::project::ProjectSpace::Work;
    let conn = deletions_conn();
    let p = queries::create_project_in_space(&conn, "Alpha", None, Some(Work)).unwrap();
    let a1 = queries::create_activity(&conn, "One", Work).unwrap();
    let a2 = queries::create_activity(&conn, "Two", Work).unwrap();
    queries::set_project_activities(&conn, &p, &[a1.id.clone(), a2.id.clone()], None).unwrap();
    queries::set_project_activities(&conn, &p, &[a1.id.clone(), a2.id.clone()], None).unwrap();
    assert!(
        traces(&conn).is_empty(),
        "a save without changes deletes nothing"
    );

    queries::set_project_activities(&conn, &p, std::slice::from_ref(&a1.id), None).unwrap();
    assert_eq!(
        traces(&conn),
        vec![trace("project_activity", &key(&p, &a2.id))]
    );

    queries::set_project_activities(&conn, &p, &[a1.id.clone(), a2.id.clone()], None).unwrap();
    assert!(
        traces(&conn).is_empty(),
        "the restored attachment is no longer deleted"
    );
}

/// A project or activity recreated under the same identifier (undo, import) is no longer
/// marked deleted.
#[test]
fn test_recreated_project_and_activity_lose_their_trace() {
    let conn = deletions_conn();
    conn.execute_batch(
        "INSERT INTO projects (id, name, created_at, updated_at) VALUES ('p', 'Project', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');
         INSERT INTO activities (id, name, space, created_at, updated_at) VALUES ('a', 'Activity', 'work', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z');
         DELETE FROM projects WHERE id = 'p';
         DELETE FROM activities WHERE id = 'a';",
    ).unwrap();
    assert_eq!(
        traces(&conn),
        vec![trace("activity", "a"), trace("project", "p")]
    );

    conn.execute_batch("INSERT INTO projects (id, name) VALUES ('p', 'Project');")
        .unwrap();
    assert_eq!(
        traces(&conn),
        vec![trace("activity", "a")],
        "only the recreated project's trace disappears"
    );
    conn.execute_batch(
        "INSERT INTO activities (id, name, space) VALUES ('a', 'Activity', 'work');",
    )
    .unwrap();
    assert!(traces(&conn).is_empty());
}

/// Changing a project's or an activity's space removes attachments **by trigger**:
/// they are traced, and only those.
#[test]
fn test_space_change_traces_attachments_removed_by_triggers() {
    use crate::models::project::ProjectSpace::{Private, Work};
    let conn = deletions_conn();
    let p = queries::create_project_in_space(&conn, "Alpha", None, Some(Work)).unwrap();
    let q = queries::create_project_in_space(&conn, "Beta", None, Some(Work)).unwrap();
    let work = queries::create_activity(&conn, "Work", Work).unwrap();
    let other = queries::create_activity(&conn, "Other", Work).unwrap();
    queries::set_project_activities(&conn, &p, &[work.id.clone(), other.id.clone()], None).unwrap();
    queries::set_project_activities(&conn, &q, std::slice::from_ref(&work.id), None).unwrap();

    // The project becomes private: its two attachments go, not the other project's.
    queries::update_project_space(&conn, &p, Some(Private)).unwrap();
    assert_eq!(
        traces(&conn),
        sorted(vec![
            trace("project_activity", &key(&p, &work.id)),
            trace("project_activity", &key(&p, &other.id)),
        ])
    );

    // The activity becomes private (no application code does this: direct `UPDATE`): the
    // attachment of `q` goes in turn.
    conn.execute(
        "UPDATE activities SET space = 'private' WHERE id = ?1",
        [&work.id],
    )
    .unwrap();
    assert_eq!(
        traces(&conn),
        sorted(vec![
            trace("project_activity", &key(&p, &work.id)),
            trace("project_activity", &key(&p, &other.id)),
            trace("project_activity", &key(&q, &work.id)),
        ])
    );
    let remaining: i64 = conn
        .query_row("SELECT count(*) FROM project_activities", [], |r| r.get(0))
        .unwrap();
    assert_eq!(remaining, 0, "every removed attachment is traced");
}

/// Permanently erasing a note from the trash traces the links removed by cascade (in both
/// directions), in addition to `note_tombstones`, which does not change; neither the note nor its dictation
/// fragments receive a trace here.
#[test]
fn test_purging_a_trashed_note_traces_its_links_removed_by_cascade() {
    let conn = deletions_conn();
    let p = queries::create_project(&conn, "Project", None).unwrap();
    let a = queries::create_note(&conn, note_input(&p, "Source", NoteType::Note)).unwrap();
    let b = queries::create_note(&conn, note_input(&p, "Discarded", NoteType::Todo)).unwrap();
    let c = queries::create_note(&conn, note_input(&p, "Next", NoteType::Todo)).unwrap();
    let d = queries::create_note(&conn, note_input(&p, "Elsewhere", NoteType::Todo)).unwrap();
    crate::db::relations::link_note_task(&conn, &a, &b).unwrap();
    crate::db::relations::link_note_task(&conn, &b, &c).unwrap();
    crate::db::relations::link_note_task(&conn, &a, &d).unwrap();
    queries::append_raw_content(&conn, &b, "dictation of the discarded note").unwrap();
    queries::trash_note(&conn, &b).unwrap();
    assert!(traces(&conn).is_empty(), "the trash erases nothing");

    queries::purge_trash(&conn, queries::TrashPurge::One(&b)).unwrap();

    assert_eq!(
        traces(&conn),
        sorted(vec![
            trace("note_task_link", &key(&a, &b)),
            trace("note_task_link", &key(&b, &c)),
        ]),
        "one link by cascade and one per outgoing link; the `a → d` link remains"
    );
    let tombstones: Vec<String> = conn
        .prepare("SELECT id FROM note_tombstones")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(tombstones, vec![b.clone()], "note_tombstones unchanged");
    assert_eq!(
        fragment_count(&conn),
        0,
        "the fragments left with the note, without a trace of their own"
    );
}

/// Directly deleting a live note (CLI `delete_note`) also traces its links.
#[test]
fn test_deleting_a_live_note_traces_its_links() {
    let conn = deletions_conn();
    let p = queries::create_project(&conn, "Project", None).unwrap();
    let a = queries::create_note(&conn, note_input(&p, "Source", NoteType::Note)).unwrap();
    let b = queries::create_note(&conn, note_input(&p, "Task", NoteType::Todo)).unwrap();
    crate::db::relations::link_note_task(&conn, &a, &b).unwrap();

    queries::delete_note(&conn, &b).unwrap();

    assert_eq!(traces(&conn), vec![trace("note_task_link", &key(&a, &b))]);
}

/// Moving a linked component removes then recreates its links in the same transaction: no
/// link trace remains, and an unrelated trace is not touched.
#[test]
fn test_moving_a_linked_component_leaves_no_link_trace() {
    use crate::models::project::ProjectSpace::Work;
    let conn = deletions_conn();
    let inbox = queries::get_or_create_inbox(&conn, Work).unwrap();
    let target = queries::create_project_in_space(&conn, "Target", None, Some(Work)).unwrap();
    let n1 = queries::create_note(&conn, note_input(&inbox, "One", NoteType::Note)).unwrap();
    let n2 = queries::create_note(&conn, note_input(&inbox, "Two", NoteType::Todo)).unwrap();
    let n3 = queries::create_note(&conn, note_input(&inbox, "Three", NoteType::Todo)).unwrap();
    let n4 = queries::create_note(&conn, note_input(&inbox, "Four", NoteType::Todo)).unwrap();
    crate::db::relations::link_note_task(&conn, &n1, &n2).unwrap();
    crate::db::relations::link_note_task(&conn, &n2, &n3).unwrap();
    crate::db::relations::link_note_task(&conn, &n1, &n4).unwrap();
    crate::db::relations::unlink_note_task(&conn, &n1, &n4).unwrap();
    let before = traces(&conn);
    assert_eq!(before, vec![trace("note_task_link", &key(&n1, &n4))]);

    queries::move_note_to_project(&conn, &n1, &target).unwrap();

    assert_eq!(
        traces(&conn),
        before,
        "no trace for the recreated links; the other trace is intact"
    );
    let links: i64 = conn
        .query_row("SELECT count(*) FROM note_task_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(links, 2, "links restored");
}

/// A new deletion of the same item updates the date (never an error nor a frozen date).
#[test]
fn test_deleting_an_element_again_refreshes_its_trace_date() {
    let conn = deletions_conn();
    conn.execute_batch(
        "INSERT INTO projects (id, name) VALUES ('p', 'Project'), ('q', 'Project 2');
         INSERT INTO activities (id, name, space) VALUES ('a', 'Activity', 'work');
         INSERT INTO notes (id, project_id, title, note_type, status) VALUES ('n1', 'q', 'One', 'note', 'open'), ('n2', 'q', 'Two', 'todo', 'open');
         UPDATE projects SET space = 'work' WHERE id = 'p';
         INSERT INTO project_activities (project_id, activity_id) VALUES ('p', 'a');
         INSERT INTO note_task_links (source_note_id, task_note_id) VALUES ('n1', 'n2');",
    ).unwrap();
    // Stale traces placed by hand **after** creation (creation erases the trace).
    let stale = "2000-01-01T00:00:00.000Z";
    for (entity, id) in [
        ("project", "p".to_string()),
        ("activity", "a".to_string()),
        ("project_activity", key("p", "a")),
        ("note_task_link", key("n1", "n2")),
    ] {
        conn.execute(
            "INSERT INTO deletions (entity, entity_id, deleted_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![entity, id, stale],
        )
        .unwrap();
    }

    conn.execute_batch(
        "DELETE FROM note_task_links;
         DELETE FROM project_activities;
         DELETE FROM activities;
         DELETE FROM projects WHERE id = 'p';",
    )
    .unwrap();

    let dates: Vec<(String, String, String)> = conn
        .prepare("SELECT entity, entity_id, deleted_at FROM deletions ORDER BY entity")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(dates.len(), 4, "a single row per item: {dates:?}");
    for (entity, _, date) in &dates {
        assert!(
            date.as_str() > stale,
            "{entity}: the date must be refreshed ({date})"
        );
        assert_iso_utc(date);
    }
}

/// Composite keys are JSON arrays: two distinct pairs never give the same
/// key, even if the identifiers contain commas, quotes or separators.
#[test]
fn test_composite_trace_keys_are_unambiguous_json_arrays() {
    let conn = deletions_conn();
    conn.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
    // Deliberately hostile identifiers (an archive or a future device may provide some).
    let ids = [
        "a,b", "c", "a", "b,c", "x\",\"y", "z", "x", "y\",\"z", "p\\q", "r",
    ];
    conn.execute_batch(
        "INSERT INTO projects (id, name, space) VALUES ('proj', 'Project', 'work');",
    )
    .unwrap();
    for id in ids {
        conn.execute("INSERT INTO notes (id, project_id, title, note_type, status) VALUES (?1, 'proj', 't', 'note', 'open')", [id]).unwrap();
    }
    let pairs = [
        ("a,b", "c"),
        ("a", "b,c"),
        ("x\",\"y", "z"),
        ("x", "y\",\"z"),
        ("p\\q", "r"),
    ];
    for (source, task) in pairs {
        conn.execute(
            "INSERT INTO note_task_links (source_note_id, task_note_id) VALUES (?1, ?2)",
            [source, task],
        )
        .unwrap();
    }
    conn.execute_batch("DELETE FROM note_task_links;").unwrap();

    let mut expected: Vec<(String, String)> = pairs
        .iter()
        .map(|(a, b)| trace("note_task_link", &key(a, b)))
        .collect();
    expected.sort();
    assert_eq!(
        traces(&conn),
        expected,
        "five distinct pairs, five traces, documented format"
    );
    assert_eq!(
        key("x\",\"y", "z"),
        r#"["x\",\"y","z"]"#,
        "format: compact JSON array"
    );

    // Same serialization for attachments.
    conn.execute_batch(
        "INSERT INTO activities (id, name, space) VALUES ('a,b', 'One', 'work'), ('c', 'Two', 'work'), ('b,c', 'Three', 'work');
         INSERT INTO projects (id, name, space) VALUES ('a', 'Other', 'work'), ('a,b', 'Comma', 'work');
         INSERT INTO project_activities (project_id, activity_id) VALUES ('a,b', 'c'), ('a', 'b,c');
         DELETE FROM project_activities;",
    ).unwrap();
    let attachments: Vec<(String, String)> = traces(&conn)
        .into_iter()
        .filter(|(e, _)| e == "project_activity")
        .collect();
    assert_eq!(
        attachments,
        sorted(vec![
            trace("project_activity", &key("a,b", "c")),
            trace("project_activity", &key("a", "b,c"))
        ])
    );
}

/// A refused deletion (`BEFORE` trigger) or an impossible one leaves no trace: the existing
/// triggers keep their behavior.
#[test]
fn test_refused_writes_leave_no_trace() {
    use crate::models::project::ProjectSpace::Work;
    let conn = deletions_conn();
    let p = queries::create_project_in_space(&conn, "Alpha", None, Some(Work)).unwrap();
    let a = queries::create_note(&conn, note_input(&p, "Source", NoteType::Note)).unwrap();
    let b = queries::create_note(&conn, note_input(&p, "Task", NoteType::Todo)).unwrap();
    crate::db::relations::link_note_task(&conn, &a, &b).unwrap();

    // `project_with_notes_cannot_be_deleted`
    let error = conn
        .execute("DELETE FROM projects WHERE id = ?1", [&p])
        .unwrap_err()
        .to_string();
    assert!(error.contains("move this project notes"), "{error}");
    // `note_task_links_immutable`
    let error = conn
        .execute(
            "UPDATE note_task_links SET task_note_id = source_note_id",
            [],
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("immutable"), "{error}");
    // `note_with_links_cannot_move_project`
    let other = queries::create_project_in_space(&conn, "Beta", None, Some(Work)).unwrap();
    assert!(conn
        .execute(
            "UPDATE notes SET project_id = ?1 WHERE id = ?2",
            [&other, &a]
        )
        .is_err());
    // Deletion of a nonexistent identifier: no row, no trace.
    assert!(queries::delete_activity(&conn, "missing").is_err());
    assert!(queries::delete_project(&conn, "missing").is_err());

    assert!(traces(&conn).is_empty(), "{:?}", traces(&conn));
}

/// Migration v20 → v21 of a representative database: all data and all other schema objects are
/// identical, the trace table starts empty, the triggers work, and the
/// migration is idempotent.
#[test]
fn test_migration_v21_creates_empty_deletions_table_and_keeps_data() {
    use crate::models::project::ProjectSpace::{Private, Work};
    let conn = deletions_conn();
    let p1 =
        queries::create_project_in_space(&conn, "Website", Some("#ff0000"), Some(Work)).unwrap();
    let p2 = queries::create_project_in_space(&conn, "Holidays", None, Some(Private)).unwrap();
    let archive = queries::create_project_in_space(&conn, "Old", None, Some(Work)).unwrap();
    queries::set_project_archived(&conn, &archive, true).unwrap();
    let work = queries::create_activity(&conn, "Development", Work).unwrap();
    let personal = queries::create_activity(&conn, "Family", Private).unwrap();
    queries::set_project_activities(&conn, &p1, std::slice::from_ref(&work.id), None).unwrap();
    queries::set_project_activities(&conn, &p2, std::slice::from_ref(&personal.id), None).unwrap();
    let a = queries::create_note(&conn, note_input(&p1, "Source", NoteType::Note)).unwrap();
    let b = queries::create_note(&conn, note_input(&p1, "Task", NoteType::Todo)).unwrap();
    crate::db::relations::link_note_task(&conn, &a, &b).unwrap();
    queries::append_raw_content(&conn, &a, "dictation").unwrap();
    let trashed =
        queries::create_note(&conn, note_input(&p2, "Discarded", NoteType::Idea)).unwrap();
    queries::trash_note(&conn, &trashed).unwrap();
    let purged = queries::create_note(&conn, note_input(&p2, "Purged", NoteType::Idea)).unwrap();
    queries::trash_note(&conn, &purged).unwrap();
    queries::purge_trash(&conn, queries::TrashPurge::One(&purged)).unwrap();

    downgrade_to_v20(&conn);
    assert!(
        conn.prepare("SELECT entity FROM deletions").is_err(),
        "the forged database is indeed at v20"
    );
    let before = dump_database(&conn);
    assert_eq!(before["notes"].len(), 3, "representative database");
    assert_eq!(before["note_tombstones"].len(), 1);
    assert_eq!(before["project_activities"].len(), 2);

    schema::apply_migrations(&conn).unwrap();

    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);
    assert_eq!(
        without_v21(dump_database(&conn)),
        before,
        "data and schema objects identical"
    );
    let columns: Vec<(String, String, i64, i64)> = conn
        .prepare(
            "SELECT name, type, \"notnull\", pk FROM pragma_table_info('deletions') ORDER BY cid",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        columns,
        vec![
            ("entity".to_string(), "TEXT".to_string(), 1, 1),
            ("entity_id".to_string(), "TEXT".to_string(), 1, 2),
            ("deleted_at".to_string(), "TEXT".to_string(), 1, 0),
        ]
    );
    assert!(
        traces(&conn).is_empty(),
        "no past deletion is invented: the table starts empty"
    );
    let triggers: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'trigger' AND sql LIKE '%deletions%' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        triggers,
        [
            "activities_trace_delete",
            "activities_untrace_insert",
            "note_task_links_trace_delete",
            "note_task_links_untrace_insert",
            "project_activities_trace_delete",
            "project_activities_untrace_insert",
            "projects_trace_delete",
            "projects_untrace_insert",
        ]
    );

    // The migrated database traces its deletions from the very first one.
    queries::delete_activity(&conn, &personal.id).unwrap();
    assert_eq!(
        traces(&conn),
        sorted(vec![
            trace("activity", &personal.id),
            trace("project_activity", &key(&p2, &personal.id))
        ])
    );

    let after = dump_database(&conn);
    schema::apply_migrations(&conn).unwrap();
    assert_eq!(dump_database(&conn), after, "idempotent");
}

/// A v21 migration that fails midway is rolled back entirely: version unchanged, no
/// half-created table or trigger, data intact.
#[test]
fn test_failed_migration_v21_rolls_back_completely() {
    let conn = deletions_conn();
    let p = queries::create_project(&conn, "Project", None).unwrap();
    downgrade_to_v20(&conn);
    // Without the links table, creating their triggers fails **after** the creation of the trace
    // table and of the first triggers.
    conn.execute_batch("DROP TABLE note_task_links;").unwrap();
    let before = dump_database(&conn);

    let error = schema::apply_migrations(&conn).unwrap_err().to_string();
    assert!(error.contains("note_task_links"), "{error}");

    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 20);
    assert!(
        conn.is_autocommit(),
        "the migration's transaction is rolled back"
    );
    assert_eq!(
        dump_database(&conn),
        before,
        "no `deletions` table nor trigger left behind"
    );
    assert!(queries::get_projects(&conn)
        .unwrap()
        .iter()
        .any(|project| project.id == p));
}

/// Migration v21 → v22 of a representative database: every case stays at top level as text, no
/// other data nor schema object moves, the provenance table starts empty, and the migration is
/// idempotent.
#[test]
fn test_migration_v22_keeps_every_case_at_top_level_and_data_intact() {
    use crate::models::project::ProjectSpace::{Private, Work};
    let conn = deletions_conn();
    let p1 =
        queries::create_project_in_space(&conn, "Website", Some("#ff0000"), Some(Work)).unwrap();
    let p2 = queries::create_project_in_space(&conn, "Holidays", None, Some(Private)).unwrap();
    let work = queries::create_activity(&conn, "Development", Work).unwrap();
    queries::set_project_activities(&conn, &p1, std::slice::from_ref(&work.id), None).unwrap();
    let a = queries::create_note(&conn, note_input(&p1, "Source", NoteType::Note)).unwrap();
    let b = queries::create_note(&conn, note_input(&p1, "Task", NoteType::Todo)).unwrap();
    crate::db::relations::link_note_task(&conn, &a, &b).unwrap();
    queries::append_raw_content(&conn, &a, "dictation").unwrap();
    let trashed =
        queries::create_note(&conn, note_input(&p2, "Discarded", NoteType::Idea)).unwrap();
    queries::trash_note(&conn, &trashed).unwrap();
    let purged = queries::create_note(&conn, note_input(&p2, "Purged", NoteType::Idea)).unwrap();
    queries::trash_note(&conn, &purged).unwrap();
    queries::purge_trash(&conn, queries::TrashPurge::One(&purged)).unwrap();

    downgrade_to_v21(&conn);
    assert!(
        conn.prepare("SELECT parent_id FROM notes").is_err(),
        "the forged database is indeed at v21"
    );
    let before = dump_database(&conn);
    assert_eq!(before["notes"].len(), 3, "representative database");

    schema::apply_migrations(&conn).unwrap();
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);
    assert_eq!(
        without_v22(dump_database(&conn)),
        before,
        "data and schema objects identical"
    );
    let (sub_cases, not_text, placed): (i64, i64, i64) = conn
        .query_row(
            "SELECT count(parent_id), count(*) FILTER (WHERE content_kind <> 'text'),
                    count(canvas_x) + count(canvas_y) + count(canvas_w) + count(canvas_h) + count(order_key)
               FROM notes",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((sub_cases, not_text, placed), (0, 0, 0));
    let provenance: i64 = conn
        .query_row("SELECT count(*) FROM case_provenance", [], |r| r.get(0))
        .unwrap();
    assert_eq!(provenance, 0, "no provenance is invented");

    // Idempotent, and the migrated database takes sub-cases at once.
    schema::apply_migrations(&conn).unwrap();
    let child = crate::db::cases::create_sub_case(
        &conn,
        &a,
        crate::models::note::ContentKind::Link,
        note_input("", "Link", NoteType::Note),
    )
    .unwrap();
    assert_eq!(
        queries::get_note_by_id(&conn, &child)
            .unwrap()
            .parent_id
            .as_deref(),
        Some(a.as_str())
    );
}

/// A v22 migration that fails midway leaves the database exactly at v21: no added column,
/// trigger or table survives.
#[test]
fn test_failed_migration_v22_rolls_back_completely() {
    let conn = deletions_conn();
    let p = queries::create_project(&conn, "Project", None).unwrap();
    queries::create_note(&conn, note_input(&p, "Case", NoteType::Note)).unwrap();
    downgrade_to_v21(&conn);
    // An unrelated `case_provenance` table makes the index creation fail **after** the new
    // columns and the first triggers.
    conn.execute_batch("CREATE TABLE case_provenance (other TEXT);")
        .unwrap();
    let before = dump_database(&conn);
    let columns = |conn: &Connection| -> i64 {
        conn.query_row("SELECT count(*) FROM pragma_table_info('notes')", [], |r| {
            r.get(0)
        })
        .unwrap()
    };
    let column_count = columns(&conn);

    let error = schema::apply_migrations(&conn).unwrap_err().to_string();
    assert!(error.contains("note_id"), "{error}");

    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 21);
    assert!(
        conn.is_autocommit(),
        "the migration's transaction is rolled back"
    );
    assert_eq!(columns(&conn), column_count, "no column left behind");
    assert_eq!(
        dump_database(&conn),
        before,
        "no trigger nor index left behind"
    );
}

/// Migration v22 → v23: the Inbox containers take their `__humcase_inbox_` technical name and
/// advance their version; nothing else changes, and the migration is idempotent.
#[test]
fn test_migration_v23_renames_inbox_containers_only() {
    use crate::models::project::ProjectSpace::{Private, Work};
    let conn = deletions_conn();
    let work = queries::get_or_create_inbox(&conn, Work).unwrap();
    let private = queries::get_or_create_inbox(&conn, Private).unwrap();
    let project = queries::create_project_in_space(&conn, "Website", None, Some(Work)).unwrap();
    queries::create_note(&conn, note_input(&work, "Captured", NoteType::Note)).unwrap();
    downgrade_to_v22(&conn);
    let name = |id: &str| -> (String, i64) {
        conn.query_row(
            "SELECT name, revision FROM projects WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    };
    let (former, revision) = name(&work);
    assert_eq!(former, format!("__devnotes_inbox_work_{work}"));
    let untouched = name(&project);

    schema::apply_migrations(&conn).unwrap();
    assert_eq!(
        name(&work),
        (format!("__humcase_inbox_work_{work}"), revision + 1)
    );
    assert_eq!(
        name(&private).0,
        format!("__humcase_inbox_private_{private}")
    );
    assert_eq!(
        name(&project),
        untouched,
        "a regular project is not touched"
    );
    assert_eq!(queries::get_or_create_inbox(&conn, Work).unwrap(), work);
    schema::apply_migrations(&conn).unwrap();
    assert_eq!(name(&work).1, revision + 1, "idempotent");
}

#[test]
fn a_write_transaction_takes_the_write_lock_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let first = crate::db::init_db(dir.path()).unwrap();
    let second = crate::db::init_db(dir.path()).unwrap();
    second
        .busy_timeout(std::time::Duration::from_millis(0))
        .unwrap();
    let tx = crate::db::write_transaction(&first).unwrap();
    // The other process cannot slip a write between this transaction's read and its write.
    assert!(second
        .execute("INSERT INTO projects (id, name) VALUES ('x', 'X')", [])
        .is_err());
    tx.rollback().unwrap();
    second
        .execute("INSERT INTO projects (id, name) VALUES ('x', 'X')", [])
        .unwrap();
}

#[test]
fn write_paths_never_open_a_deferred_transaction() {
    // A deferred transaction that reads then writes fails when the MCP process commits in
    // between: every write path goes through `write_transaction`.
    fn visit(dir: &std::path::Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(&path, found);
            } else if path.extension().is_some_and(|e| e == "rs")
                && !path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains("tests")
                && std::fs::read_to_string(&path)
                    .unwrap()
                    .contains(".unchecked_transaction()")
            {
                found.push(path.display().to_string());
            }
        }
    }
    let mut found = Vec::new();
    visit(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/db"),
        &mut found,
    );
    assert!(found.is_empty(), "deferred transactions in {found:?}");
}

/// v23 → v24: the journal of who did what, the decisions and the opened cases are added empty;
/// every existing row and schema object stays identical, no history is invented for earlier
/// writes, and the migrated database records the next writes at once.
#[test]
fn test_migration_v24_adds_an_empty_journal_and_keeps_data_intact() {
    use crate::models::project::ProjectSpace::Work;
    let conn = deletions_conn();
    let p = queries::create_project_in_space(&conn, "Website", None, Some(Work)).unwrap();
    let a = queries::create_note(&conn, note_input(&p, "Source", NoteType::Note)).unwrap();
    let b = queries::create_note(&conn, note_input(&p, "Task", NoteType::Todo)).unwrap();
    crate::db::relations::link_note_task(&conn, &a, &b).unwrap();
    queries::append_raw_content(&conn, &a, "dictation").unwrap();
    conn.execute(
        "UPDATE notes SET status = 'resolved', resolution_comment = 'Done' WHERE id = ?1",
        [&b],
    )
    .unwrap();
    conn.execute_batch(
        "DROP TRIGGER case_events_only_merge_edits;
         DROP TRIGGER case_events_no_delete;
         DROP TRIGGER case_reviews_no_update;
         DROP TRIGGER case_reviews_no_delete;
         DROP TABLE case_events;
         DROP TABLE case_reviews;
         DROP TABLE case_views;
         DROP TABLE nature_checks;
         PRAGMA user_version = 23;",
    )
    .unwrap();
    let before = dump_database(&conn);
    assert_eq!(before["notes"].len(), 2, "representative database");

    schema::apply_migrations(&conn).unwrap();
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);
    let mut after = dump_database(&conn);
    for table in ["case_events", "case_reviews", "case_views", "nature_checks"] {
        assert_eq!(
            after.remove(table).map(|rows| rows.len()),
            Some(0),
            "{table} starts empty"
        );
    }
    if let Some(objects) = after.get_mut("<schema objects>") {
        objects.retain(|o| {
            !o.contains("case_events")
                && !o.contains("case_reviews")
                && !o.contains("case_views")
                && !o.contains("nature_checks")
        });
    }
    assert_eq!(after, before, "data and schema objects identical");
    // A result resolved before v24 has no recorded author: nothing to review is invented.
    assert_eq!(
        crate::db::journal::review_state(&conn, &b).unwrap().state,
        "none"
    );

    // Idempotent, and the migrated database journals the next write.
    schema::apply_migrations(&conn).unwrap();
    crate::db::journal::install(
        &conn,
        &crate::db::journal::Actor::Agent(Some("Codex".into())),
    )
    .unwrap();
    conn.execute("UPDATE notes SET title = 'Renamed' WHERE id = ?1", [&a])
        .unwrap();
    let events = crate::db::journal::case_events(&conn, &a).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        (events[0].kind.as_str(), events[0].actor_name.as_deref()),
        ("edited", Some("Codex"))
    );
}

/// v25 — answers on natures: an old database gains an empty `nature_checks`, its data intact,
/// and an answer disappears with its case.
#[test]
fn test_migration_v25_adds_empty_nature_checks_and_keeps_data_intact() {
    let conn = deletions_conn();
    let p = queries::create_project(&conn, "Website", None).unwrap();
    let a = queries::create_note(&conn, note_input(&p, "Source", NoteType::Note)).unwrap();
    queries::append_raw_content(&conn, &a, "dictation").unwrap();
    conn.execute_batch("DROP TABLE nature_checks; PRAGMA user_version = 24;")
        .unwrap();
    let before = dump_database(&conn);

    schema::apply_migrations(&conn).unwrap();
    schema::apply_migrations(&conn).unwrap();
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);
    let mut after = dump_database(&conn);
    assert_eq!(after.remove("nature_checks").map(|r| r.len()), Some(0));
    if let Some(objects) = after.get_mut("<schema objects>") {
        objects.retain(|o| !o.contains("nature_checks"));
    }
    assert_eq!(after, before, "data and schema objects identical");

    crate::natures::record_answer(
        &conn,
        &a,
        NoteType::Note,
        NoteType::Todo,
        crate::natures::Answer::Kept,
    )
    .unwrap();
    conn.execute("DELETE FROM notes WHERE id = ?1", [&a])
        .unwrap();
    let left: i64 = conn
        .query_row("SELECT COUNT(*) FROM nature_checks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0, "an answer leaves with its case");
}

/// A failure that comes from outside Humcase is told in the interface language; a write
/// refused by a rule of the schema keeps SQLite's text, which names that rule.
#[test]
fn database_failures_from_outside_are_told_to_the_person() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("busy.db");
    let holder = Connection::open(&path).unwrap();
    holder
        .execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY); BEGIN IMMEDIATE;")
        .unwrap();
    let other = Connection::open(&path).unwrap();
    other.busy_timeout(std::time::Duration::ZERO).unwrap();
    let busy = other.execute("INSERT INTO t VALUES (1)", []).unwrap_err();
    assert_eq!(super::db_error(busy), crate::tr!("data.dbBusy"));
    holder.execute_batch("COMMIT").unwrap();

    let read_only =
        Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let refused = read_only
        .execute("INSERT INTO t VALUES (2)", [])
        .unwrap_err();
    assert_eq!(super::db_error(refused), crate::tr!("data.dbReadOnly"));

    other.execute("INSERT INTO t VALUES (3)", []).unwrap();
    let duplicate = other.execute("INSERT INTO t VALUES (3)", []).unwrap_err();
    let text = duplicate.to_string();
    assert_eq!(super::db_error(duplicate), text);
    assert!(text.contains("UNIQUE"), "{text}");
}

// ───────────────────────── Word search: near forms, partial matches ─────────────────────────

fn searched_titles(conn: &Connection, query: &str) -> Vec<String> {
    let mut titles: Vec<String> = queries::get_notes(conn, None, None, Some(query))
        .unwrap()
        .into_iter()
        .map(|n| n.title)
        .collect();
    titles.sort();
    titles
}

fn note_with(conn: &Connection, project: &str, title: &str, content: &str) -> String {
    let mut input = note_input(project, title, NoteType::Bug);
    input.content = Some(content.to_string());
    queries::create_note(conn, input).unwrap()
}

/// A word finds its near forms: the former search wanted "liens", the case says "lien".
#[test]
fn test_search_finds_near_forms() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "P", None).unwrap();
    note_with(
        &conn,
        &p,
        "Extension Link absente",
        "tout lien Markdown est perdu",
    );
    note_with(&conn, &p, "Autre", "rien à voir");
    assert_eq!(
        searched_titles(&conn, "liens perdus"),
        ["Extension Link absente"]
    );
}

/// A case holding some of the words is found when it is the best answer: the former search
/// returned nothing as soon as one word was missing.
#[test]
fn test_search_does_not_require_every_word() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "P", None).unwrap();
    note_with(
        &conn,
        &p,
        "Extension Link absente",
        "tout lien Markdown est perdu à la réécriture",
    );
    note_with(&conn, &p, "Sauvegarde portable", "archive et restauration");
    note_with(&conn, &p, "Autre", "rien à voir");
    let found = searched_titles(&conn, "lien perdu sauvegarde");
    assert!(
        found.contains(&"Extension Link absente".to_string()),
        "{found:?}"
    );
    assert!(!found.contains(&"Autre".to_string()), "{found:?}");
}

/// One word returns every case holding it, as before: no case is cut for scoring lower.
#[test]
fn test_search_one_word_keeps_every_match() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "P", None).unwrap();
    note_with(&conn, &p, "Micro", "micro micro micro");
    note_with(&conn, &p, "Réglages", "une longue note sur les réglages, les thèmes, les polices, les langues, et une seule fois le micro, perdu au milieu de beaucoup d'autres mots qui diluent le score");
    note_with(&conn, &p, "Autre", "rien à voir");
    assert_eq!(searched_titles(&conn, "micro"), ["Micro", "Réglages"]);
}

/// Every case holding all the words is returned, even with a low score, so nothing the former
/// search found is lost.
#[test]
fn test_search_keeps_every_complete_match() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "P", None).unwrap();
    note_with(&conn, &p, "Kanban zèbre", "kanban zèbre kanban zèbre");
    note_with(&conn, &p, "Long", "une très longue note qui parle de tout et de rien, de projets, de réunions, de planning, et qui cite une fois le kanban et une fois le zèbre parmi beaucoup d'autres mots encore");
    assert_eq!(
        searched_titles(&conn, "kanban zèbre"),
        ["Kanban zèbre", "Long"]
    );
}

/// A word found in most cases does not bring unrelated cases up.
#[test]
fn test_search_common_words_do_not_match_alone() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "P", None).unwrap();
    for i in 0..12 {
        note_with(&conn, &p, &format!("Note {i}"), "the usual text");
    }
    note_with(&conn, &p, "Zebra", "the zebra crossing");
    assert_eq!(searched_titles(&conn, "the zebra"), ["Zebra"]);
}

/// Separators alone find nothing, and a case in the trash is never found.
#[test]
fn test_search_separators_and_trash() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "P", None).unwrap();
    let gone = note_with(&conn, &p, "Corbeille", "girafe");
    queries::trash_note(&conn, &gone).unwrap();
    assert!(searched_titles(&conn, "((( \"").is_empty());
    assert!(searched_titles(&conn, "girafe").is_empty());
}

/// The listing of agents (MCP `list_notes`) follows the same rule, best first.
#[test]
fn test_listing_search_follows_the_same_rule() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "P", None).unwrap();
    note_with(
        &conn,
        &p,
        "Extension Link absente",
        "tout lien Markdown est perdu",
    );
    note_with(&conn, &p, "Autre", "rien à voir");
    let filter = queries::NoteListFilter {
        search_query: Some("liens perdus sauvegarde".into()),
        ..Default::default()
    };
    let page = queries::list_notes_page(&conn, &filter, 0, 10, false).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.notes[0].title, "Extension Link absente");
}

/// The interface's search answer comes best match first: a title match before a passing mention.
#[test]
fn test_search_returns_best_match_first() {
    let conn = setup_test_db();
    let p = queries::create_project(&conn, "P", None).unwrap();
    note_with(
        &conn,
        &p,
        "Réunion d'équipe",
        "ordre du jour, et le calendrier en passant",
    );
    note_with(&conn, &p, "Calendrier partagé", "le calendrier de l'équipe");
    let found: Vec<String> = queries::get_notes(&conn, None, None, Some("calendrier"))
        .unwrap()
        .into_iter()
        .map(|n| n.title)
        .collect();
    assert_eq!(found, ["Calendrier partagé", "Réunion d'équipe"]);
}

/// v26 — proposals: an old database gains an empty `proposals`, its data intact, and the
/// proposals of a case disappear with it.
#[test]
fn test_migration_v26_adds_empty_proposals_and_keeps_data_intact() {
    let conn = deletions_conn();
    let p = queries::create_project(&conn, "Website", None).unwrap();
    let a = queries::create_note(&conn, note_input(&p, "Source", NoteType::Idea)).unwrap();
    conn.execute_batch("DROP TABLE proposals; PRAGMA user_version = 25;")
        .unwrap();
    let before = dump_database(&conn);

    schema::apply_migrations(&conn).unwrap();
    schema::apply_migrations(&conn).unwrap();
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);
    let mut after = dump_database(&conn);
    assert_eq!(after.remove("proposals").map(|r| r.len()), Some(0));
    if let Some(objects) = after.get_mut("<schema objects>") {
        objects.retain(|o| !o.contains("proposals"));
    }
    assert_eq!(after, before, "data and schema objects identical");

    conn.execute(
        "INSERT INTO proposals (id, note_id, kind, nature, title, passage, revision, position, created_at)
         VALUES ('x', ?1, 'sub_case', 'todo', 'Step', 'a passage', 0, 0, '2026-10-04T10:00:00Z')",
        [&a],
    )
    .unwrap();
    assert!(conn
        .execute("UPDATE proposals SET answer = 'maybe' WHERE id = 'x'", [],)
        .is_err());
    conn.execute("DELETE FROM notes WHERE id = ?1", [&a])
        .unwrap();
    let left: i64 = conn
        .query_row("SELECT COUNT(*) FROM proposals", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);
}

#[test]
fn test_migration_v27_keeps_proposals_and_adds_supersessions() {
    let conn = deletions_conn();
    let p = queries::create_project(&conn, "Website", None).unwrap();
    let a = queries::create_note(&conn, note_input(&p, "Older", NoteType::Decision)).unwrap();
    let b = queries::create_note(&conn, note_input(&p, "Newer", NoteType::Decision)).unwrap();
    // A v26 database: the proposals table as v26 made it, with an answered row.
    conn.execute_batch(
        "DROP TABLE case_supersessions;
         DROP TABLE proposals;
         CREATE TABLE proposals (
             id TEXT PRIMARY KEY,
             note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
             kind TEXT NOT NULL, nature TEXT NOT NULL, title TEXT NOT NULL, passage TEXT NOT NULL,
             reason TEXT, proposed_by TEXT,
             near_id TEXT REFERENCES notes(id) ON DELETE SET NULL,
             follows REAL, covers REAL, revision INTEGER NOT NULL, position INTEGER NOT NULL,
             created_at TEXT NOT NULL,
             answer TEXT CHECK (answer IN ('created', 'placed', 'skipped', 'refused', 'replaced')),
             answer_nature TEXT, answer_title TEXT,
             result_id TEXT REFERENCES notes(id) ON DELETE SET NULL, answered_at TEXT
         );
         CREATE INDEX idx_proposals_note ON proposals(note_id, answer, created_at);
         PRAGMA user_version = 26;",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO proposals (id, note_id, kind, nature, title, passage, proposed_by, near_id,
                                follows, revision, position, created_at, answer, answered_at)
         VALUES ('x', ?1, 'sub_case', 'todo', 'Step', 'a passage', 'Claude Code', ?2, 0.8, 3, 1,
                 '2026-10-04T10:00:00Z', 'refused', '2026-10-04T10:05:00Z')",
        [&a, &b],
    )
    .unwrap();
    let before = dump_database(&conn);

    schema::apply_migrations(&conn).unwrap();
    schema::apply_migrations(&conn).unwrap();
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);
    let mut after = dump_database(&conn);
    assert_eq!(after.get("proposals"), before.get("proposals"), "rows kept");
    assert_eq!(after.remove("case_supersessions").map(|r| r.len()), Some(0));
    after.remove("<schema objects>");
    let mut before = before;
    before.remove("<schema objects>");
    assert_eq!(after, before, "data identical");

    // The new answers are accepted, an unknown one still refused.
    conn.execute(
        "UPDATE proposals SET answer = 'accepted' WHERE id = 'x'",
        [],
    )
    .unwrap();
    assert!(conn
        .execute("UPDATE proposals SET answer = 'maybe' WHERE id = 'x'", [])
        .is_err());
    // A decision cannot replace itself; a supersession leaves with either decision.
    assert!(conn
        .execute(
            "INSERT INTO case_supersessions (note_id, by_id, superseded_at) VALUES (?1, ?1, 'now')",
            [&a],
        )
        .is_err());
    conn.execute(
        "INSERT INTO case_supersessions (note_id, by_id, superseded_at) VALUES (?1, ?2, 'now')",
        [&a, &b],
    )
    .unwrap();
    conn.execute("DELETE FROM notes WHERE id = ?1", [&b])
        .unwrap();
    let left: i64 = conn
        .query_row("SELECT COUNT(*) FROM case_supersessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);
}

#[test]
fn test_migration_v28_adds_empty_work_profiles_and_keeps_data_intact() {
    let conn = deletions_conn();
    let p = queries::create_project(&conn, "Website", None).unwrap();
    let a = queries::create_note(&conn, note_input(&p, "How I work", NoteType::Note)).unwrap();
    conn.execute_batch("DROP TABLE work_profiles; PRAGMA user_version = 27;")
        .unwrap();
    let before = dump_database(&conn);

    schema::apply_migrations(&conn).unwrap();
    schema::apply_migrations(&conn).unwrap();
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 28);
    let mut after = dump_database(&conn);
    assert_eq!(after.remove("work_profiles").map(|r| r.len()), Some(0));
    if let Some(objects) = after.get_mut("<schema objects>") {
        objects.retain(|o| !o.contains("work_profiles"));
    }
    assert_eq!(after, before, "data and schema objects identical");

    assert!(conn
        .execute(
            "INSERT INTO work_profiles (space, note_id, created_at) VALUES ('home', ?1, 'x')",
            [&a],
        )
        .is_err());
    conn.execute(
        "INSERT INTO work_profiles (space, note_id, created_at) VALUES ('work', ?1, 'x')",
        [&a],
    )
    .unwrap();
    conn.execute("DELETE FROM notes WHERE id = ?1", [&a])
        .unwrap();
    let left: i64 = conn
        .query_row("SELECT COUNT(*) FROM work_profiles", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);
}
