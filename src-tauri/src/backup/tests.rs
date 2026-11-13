use super::*;

// The assertions of these tests target the French messages: the tested functions are
// called with the thread language forced to French (the global language may be changed
// by another test in parallel).
fn create_archive(db_path: &Path, images_dir: &Path, destination: &Path) -> Result<(), String> {
    crate::i18n::with_language("fr", || {
        super::create_archive(db_path, images_dir, destination)
    })
}

fn inspect_archive(path: &Path) -> Result<ArchivePreview, String> {
    crate::i18n::with_language("fr", || super::inspect_archive(path))
}

fn fixture(content: &str, with_image: bool) -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("com.humcase.desktop");
    let images = data.join("images");
    fs::create_dir_all(&images).unwrap();
    let db = data.join("humcase.db");
    let conn = crate::db::init_db(&data).unwrap();
    conn.execute(
        "INSERT INTO projects (id, name) VALUES ('p', 'Project')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO notes (id, project_id, title, content, note_type, status) VALUES ('n', 'p', 'Note', ?1, 'note', 'open')",
        [content],
    ).unwrap();
    drop(conn);
    if with_image {
        fs::write(images.join("image.png"), b"image bytes").unwrap();
    }
    let output = root.path().join("backup.humcase.zip");
    (root, db, images, output)
}

fn set_note_content(db: &Path, content: &str) {
    Connection::open(db)
        .unwrap()
        .execute("UPDATE notes SET content = ?1 WHERE id = 'n'", [content])
        .unwrap();
}

#[test]
fn raw_source_image_survives_portable_archive_and_restore() {
    let (_source_root, db, images, output) = fixture("Structured text", true);
    let raw = format!(
        "Before restructuring: ![source]({})",
        super::restore::local_asset_url(&images.join("image.png"))
    );
    // Real write path of the raw text (v20): the fragment carries the same image reference.
    crate::db::queries::append_raw_content(&Connection::open(&db).unwrap(), "n", &raw).unwrap();
    create_archive(&db, &images, &output).unwrap();
    let preview = inspect_archive(&output).unwrap();
    let (target_root, target_db, target_images, _) = fixture("Previous target", false);
    let safety = target_root.path().join("safety.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &output,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();
    let conn = Connection::open(&target_db).unwrap();
    let (content, restored_raw): (String, String) = conn
        .query_row(
            "SELECT content,raw_content FROM notes WHERE id='n'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(content, "Structured text");
    let restored_image = fs::read_dir(&target_images)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(
        restored_raw.contains(&crate::images::reference(
            restored_image.file_name().unwrap().to_str().unwrap()
        )),
        "the restore writes the portable reference"
    );
    assert!(!restored_raw.contains(&super::restore::local_asset_url(&images.join("image.png"))));
    assert!(restored_raw.starts_with("Before restructuring: "));
    // The fragment follows the same rewrite: it still recomposes the restored raw text.
    let fragments: Vec<String> = conn
        .prepare("SELECT text FROM dictation_fragments WHERE note_id='n'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(fragments, vec![restored_raw.clone()]);
    let again = target_root.path().join("again.humcase.zip");
    create_archive(&target_db, &target_images, &again).unwrap();
    assert_eq!(inspect_archive(&again).unwrap().image_count, 1);
}

/// M2 — the archive restore must not destroy the attributed journal `note_revisions`, and the
/// agent edit authorization flag of the **live** profile must be kept (an archive never
/// silently re-enables editing). Fails if the copy of `note_revisions` disappears from
/// `restore_archive_into_impl`.
#[test]
fn restore_preserves_agent_history_and_keeps_live_agent_permission() {
    // Source: a note edited by the agent (via MCP), hence an attributed revision.
    let (source_root, source_db, source_images, _) = fixture("Initial target", false);
    {
        let conn = Connection::open(&source_db).unwrap();
        crate::db::permissions::set_setting(
            &conn,
            crate::db::permissions::MCP_ALLOW_NOTE_EDIT,
            "1",
        )
        .unwrap();
        conn.execute("UPDATE notes SET title = 'Before agent' WHERE id = 'n'", [])
            .unwrap();
        crate::mcp::tools::handle_tool_call(
            &conn,
            "update_note",
            Some(serde_json::json!({ "note_id": "n", "expected_revision": 0, "title": "After agent" })),
        )
        .unwrap();
        assert_eq!(
            crate::db::permissions::note_history(&conn, "n")
                .unwrap()
                .len(),
            1,
            "the agent edit must create an attributed snapshot"
        );
    }
    let archive = source_root.path().join("journal.humcase.zip");
    create_archive(&source_db, &source_images, &archive).unwrap();
    let preview = inspect_archive(&archive).unwrap();

    // Target: authorization DISABLED, with a stray snapshot that must be replaced.
    let (target_root, target_db, target_images, _) = fixture("Target", false);
    {
        let conn = Connection::open(&target_db).unwrap();
        crate::db::permissions::set_setting(
            &conn,
            crate::db::permissions::MCP_ALLOW_NOTE_EDIT,
            "0",
        )
        .unwrap();
        crate::db::permissions::insert_note_revision(
            &conn,
            "n",
            0,
            "agent",
            "{\"title\":\"parasite\"}",
        )
        .unwrap();
    }
    let safety = target_root.path().join("safety.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();

    let target = Connection::open(&target_db).unwrap();
    assert_eq!(
        crate::db::queries::get_note_by_id(&target, "n")
            .unwrap()
            .title,
        "After agent"
    );
    let history = crate::db::permissions::note_history(&target, "n").unwrap();
    assert_eq!(
        history.len(),
        1,
        "the archive's journal must survive the restore"
    );
    assert_eq!(history[0].source, "agent");
    let fields: serde_json::Value = serde_json::from_str(&history[0].fields).unwrap();
    assert_eq!(
        fields["title"], "Before agent",
        "the restored snapshot must be the archive's, not the live profile's"
    );
    // The flag stays the live one (disabled), not the archive's (enabled).
    assert!(
        !crate::db::permissions::allow_note_edit(&target).unwrap(),
        "an archive must not re-enable agent editing"
    );
}

#[test]
fn restore_never_moves_a_revision_backwards() {
    // The archive holds note n at revision 5; the live note went on to revision 8. An agent that
    // read revision 5 before the restore must get a conflict, not write over the restored text.
    let (source_root, source_db, source_images, _) = fixture("Archived text", false);
    Connection::open(&source_db)
        .unwrap()
        .execute_batch(
            "UPDATE notes SET revision = 5 WHERE id = 'n';
             INSERT INTO notes (id, project_id, title, content, note_type, status, revision)
               VALUES ('only-archived', 'p', 'Gone since', '', 'note', 'open', 3);",
        )
        .unwrap();
    let archive = source_root.path().join("revisions.humcase.zip");
    create_archive(&source_db, &source_images, &archive).unwrap();
    let preview = inspect_archive(&archive).unwrap();

    let (target_root, target_db, target_images, _) = fixture("Live text", false);
    Connection::open(&target_db)
        .unwrap()
        .execute("UPDATE notes SET revision = 8 WHERE id = 'n'", [])
        .unwrap();
    let safety = target_root.path().join("safety-revisions.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();

    let target = Connection::open(&target_db).unwrap();
    let revision = |id: &str| -> i64 {
        target
            .query_row("SELECT revision FROM notes WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
    };
    assert_eq!(
        revision("n"),
        9,
        "past both the archive and the live revision"
    );
    assert_eq!(
        revision("only-archived"),
        3,
        "absent from the live base: the archive's"
    );
    assert!(crate::mcp::tools::handle_tool_call(
        &target,
        "update_note",
        Some(serde_json::json!({ "note_id": "n", "expected_revision": 5, "title": "Stale" })),
    )
    .is_err());
}

fn asset_url(path: &Path) -> String {
    format!(
        "http://asset.localhost/{}",
        path.to_string_lossy()
            .replace('\\', "/")
            .replace(':', "%3A")
    )
}

fn archive_db_for_test(db: &Path, output: &Path, schema_version: i32) {
    let bytes = fs::read(db).unwrap();
    let manifest = Manifest {
        format_version: FORMAT_VERSION,
        created_at: "2026-09-24T12:00:00.000Z".to_string(),
        database_schema_version: schema_version,
        entries: vec![Entry {
            path: "humcase.db".to_string(),
            size: bytes.len() as u64,
            sha256: crate::images::lower_hex(&Sha256::digest(&bytes)),
        }],
    };
    let mut zip = ZipWriter::new(File::create(output).unwrap());
    zip.start_file("humcase.db", zip_options()).unwrap();
    zip.write_all(&bytes).unwrap();
    zip.start_file("manifest.json", zip_options()).unwrap();
    zip.write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    zip.finish().unwrap();
}

/// Removes what the v21 migration created (deletion traces): a v20 or older
/// archive does not have it.
const DROP_V21: &str = "DROP TRIGGER IF EXISTS projects_trace_delete;
     DROP TRIGGER IF EXISTS projects_untrace_insert;
     DROP TRIGGER IF EXISTS activities_trace_delete;
     DROP TRIGGER IF EXISTS activities_untrace_insert;
     DROP TRIGGER IF EXISTS note_task_links_trace_delete;
     DROP TRIGGER IF EXISTS note_task_links_untrace_insert;
     DROP TRIGGER IF EXISTS project_activities_trace_delete;
     DROP TRIGGER IF EXISTS project_activities_untrace_insert;
     DROP TABLE IF EXISTS deletions;";

/// Removes what the v20 migration created (dictation fragments): an older archive does not have it.
const DROP_V20: &str = "DROP TRIGGER IF EXISTS dictation_fragments_no_update;
     DROP TRIGGER IF EXISTS dictation_fragments_no_delete;
     DROP TABLE IF EXISTS dictation_fragments;";

/// Removes what the v20 and v21 migrations created, to forge a v19 or older archive.
fn drop_v20_and_v21(conn: &Connection) {
    conn.execute_batch(DROP_V21).unwrap();
    conn.execute_batch(DROP_V20).unwrap();
}

fn downgrade_fixture(db: &Path, version: i32) {
    let conn = Connection::open(db).unwrap();
    drop_v20_and_v21(&conn);
    conn.execute_batch(
        "DROP TRIGGER note_task_links_same_project_insert;
         DROP TRIGGER note_task_links_no_cycles_insert;
         DROP TRIGGER note_task_links_immutable;
         DROP TRIGGER note_with_links_cannot_move_project;
         DROP TABLE note_task_links;",
    )
    .unwrap();
    if version == 11 {
        conn.pragma_update(None, "user_version", version).unwrap();
        return;
    }
    conn.execute_batch(
        "DROP TRIGGER inbox_requires_space_insert;
         DROP TRIGGER inbox_requires_space_update;
         DROP TRIGGER inbox_flag_immutable;
         DROP TRIGGER project_activities_same_space_insert;
         DROP TRIGGER project_activities_same_space_update;
         DROP TRIGGER project_space_unlink_activities;
         DROP TRIGGER activity_space_unlink_projects;
         DROP TABLE project_activities;
         DROP TABLE activities;
         DROP INDEX idx_projects_one_inbox_per_space;
         ALTER TABLE projects DROP COLUMN is_inbox;",
    )
    .unwrap();
    if version == 9 {
        conn.execute_batch("ALTER TABLE projects DROP COLUMN space;")
            .unwrap();
    }
    conn.pragma_update(None, "user_version", version).unwrap();
}

/// Brings a current database (v13) back to the **real v12** state: FTS index without a report and
/// `user_version = 12`. The v12 tables (`note_task_links`, activities...) stay in place, as
/// in a database produced by 0.9.25→0.9.28. Used to forge an old archive consistent with
/// its manifest (same logic as `downgrade_fixture` for v9/v10/v11).
fn downgrade_to_v12(db: &Path) {
    let conn = Connection::open(db).unwrap();
    drop_v20_and_v21(&conn);
    conn.execute_batch(
        "DROP TRIGGER notes_ai;
         DROP TRIGGER notes_ad;
         DROP TRIGGER notes_au;
         DROP TABLE notes_fts;
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
         PRAGMA user_version = 12;",
    )
    .unwrap();
}

/// MAJOR regression — a v12 archive (0.9.25→0.9.28) must become **inspectable** again: the extracted
/// copy is migrated to the current version, without ever touching the user database. The
/// preview reports the archive's **original** version, not the migrated one.
#[test]
fn v12_archive_is_inspectable_and_reports_original_schema() {
    let (_root, db, _images, output) = fixture("content v12", false);
    Connection::open(&db)
        .unwrap()
        .execute(
            "UPDATE notes SET resolution_comment='report present in v12' WHERE id='n'",
            [],
        )
        .unwrap();
    downgrade_to_v12(&db);
    archive_db_for_test(&db, &output, 12);

    let preview = inspect_archive(&output).expect("a v12 archive must stay inspectable");
    assert_eq!(
        preview.database_schema_version, 12,
        "the preview reports the archive's original version"
    );
    assert_eq!(preview.project_count, 1);
    assert_eq!(preview.note_count, 1);
}

/// A v12 archive restored on a blank profile keeps its data, including the report
/// (column present since v12) that the v13 migration must index correctly.
#[test]
fn v12_archive_restores_and_keeps_resolution_comment() {
    let (_source_root, source_db, _source_images, archive) = fixture("source v12", false);
    Connection::open(&source_db)
        .unwrap()
        .execute(
            "UPDATE notes SET resolution_comment='v12 report to keep' WHERE id='n'",
            [],
        )
        .unwrap();
    downgrade_to_v12(&source_db);
    archive_db_for_test(&source_db, &archive, 12);
    let preview = inspect_archive(&archive).unwrap();
    assert_eq!(preview.database_schema_version, 12);

    let (target_root, target_db, target_images, _) = fixture("target", false);
    let safety = target_root.path().join("safety-v12.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();

    let target = Connection::open(&target_db).unwrap();
    let (content, comment): (String, Option<String>) = target
        .query_row(
            "SELECT content, resolution_comment FROM notes WHERE id='n'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(content, "source v12");
    assert_eq!(comment.as_deref(), Some("v12 report to keep"));
    assert_eq!(
        check_db(&target).unwrap().0,
        DB_SCHEMA_VERSION,
        "the restored database is migrated to the current version"
    );
}

/// Brings a current database back to the **real v17** state (without `revision` nor `updated_at` on projects
/// and activities, original attachment triggers), like an archive from 0.9.88 or earlier.
fn downgrade_to_v17(db: &Path) {
    let conn = Connection::open(db).unwrap();
    drop_v20_and_v21(&conn);
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

/// A v17 archive (produced before projects and activities were versioned) stays
/// inspectable and restorable: the working copy is migrated to v18 (version 0, `updated_at` =
/// `created_at`) and the restored database is at v18.
#[test]
fn v17_archive_restores_with_zero_revisions_and_created_at_as_updated_at() {
    let (_source_root, source_db, _source_images, archive) = fixture("source v17", false);
    Connection::open(&source_db)
        .unwrap()
        .execute_batch(
            "UPDATE projects SET created_at='2025-03-04T05:06:07.008Z', space='work' WHERE id='p';
             INSERT INTO activities (id, name, space, created_at) VALUES ('a', 'Dev', 'work', '2025-04-05T06:07:08.009Z');
             INSERT INTO project_activities VALUES ('p', 'a');",
        )
        .unwrap();
    downgrade_to_v17(&source_db);
    archive_db_for_test(&source_db, &archive, 17);
    let preview = inspect_archive(&archive).unwrap();
    assert_eq!(
        preview.database_schema_version, 17,
        "the preview reports the original version"
    );

    let (target_root, target_db, target_images, _) = fixture("target", false);
    let safety = target_root.path().join("safety-v17.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();

    let target = Connection::open(&target_db).unwrap();
    assert_eq!(check_db(&target).unwrap().0, DB_SCHEMA_VERSION);
    let project: (i64, String) = target
        .query_row(
            "SELECT revision, updated_at FROM projects WHERE id='p'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(project, (0, "2025-03-04T05:06:07.008Z".to_string()));
    let activity: (i64, String) = target
        .query_row(
            "SELECT revision, updated_at FROM activities WHERE id='a'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(activity, (0, "2025-04-05T06:07:08.009Z".to_string()));
    let content: String = target
        .query_row("SELECT content FROM notes WHERE id='n'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(content, "source v17");
}

#[test]
fn v9_archive_restores_projects_as_unassigned() {
    let (_old_root, old_db, _old_images, old_archive) = fixture("original", false);
    downgrade_fixture(&old_db, 9);
    archive_db_for_test(&old_db, &old_archive, 9);
    let preview = inspect_archive(&old_archive).unwrap();
    assert_eq!(
        preview.database_schema_version, 9,
        "the preview must report the archive's original version"
    );

    let (target_root, target_db, target_images, _) = fixture("replacement", false);
    Connection::open(&target_db)
        .unwrap()
        .execute(
            "INSERT INTO activities (id, name, space) VALUES ('obsolete', 'Old', 'work')",
            [],
        )
        .unwrap();
    let safety = target_root.path().join("safety-v9.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &old_archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();
    let target = Connection::open(&target_db).unwrap();
    let space: Option<String> = target
        .query_row("SELECT space FROM projects WHERE id='p'", [], |r| r.get(0))
        .unwrap();
    let content: String = target
        .query_row("SELECT content FROM notes WHERE id='n'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(space, None);
    assert_eq!(content, "original");
    assert_eq!(check_db(&target).unwrap().0, DB_SCHEMA_VERSION);
    let activity_count: i64 = target
        .query_row("SELECT count(*) FROM activities", [], |r| r.get(0))
        .unwrap();
    assert_eq!(activity_count, 0);
}

#[test]
fn v10_archive_restores_and_clears_existing_activities() {
    let (_old_root, old_db, _old_images, old_archive) = fixture("v10", false);
    downgrade_fixture(&old_db, 10);
    archive_db_for_test(&old_db, &old_archive, 10);
    let preview = inspect_archive(&old_archive).unwrap();
    assert_eq!(
        preview.database_schema_version, 10,
        "the preview must report the archive's original version"
    );

    let (target_root, target_db, target_images, _) = fixture("newer", false);
    let target = Connection::open(&target_db).unwrap();
    target
        .execute("UPDATE projects SET space='work' WHERE id='p'", [])
        .unwrap();
    target
        .execute(
            "INSERT INTO activities (id, name, space) VALUES ('old', 'Old', 'work')",
            [],
        )
        .unwrap();
    target
        .execute("INSERT INTO project_activities VALUES ('p', 'old')", [])
        .unwrap();
    drop(target);
    let safety = target_root.path().join("safety-v10.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &old_archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();
    let target = Connection::open(&target_db).unwrap();
    let activity_count: i64 = target
        .query_row("SELECT count(*) FROM activities", [], |r| r.get(0))
        .unwrap();
    let link_count: i64 = target
        .query_row("SELECT count(*) FROM project_activities", [], |r| r.get(0))
        .unwrap();
    assert_eq!((activity_count, link_count), (0, 0));
    let content: String = target
        .query_row("SELECT content FROM notes WHERE id='n'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(content, "v10");
}

#[test]
fn v11_archive_restores_and_clears_existing_note_task_links() {
    let (_old_root, old_db, _old_images, old_archive) = fixture("v11", false);
    downgrade_fixture(&old_db, 11);
    archive_db_for_test(&old_db, &old_archive, 11);
    let preview = inspect_archive(&old_archive).unwrap();
    assert_eq!(
        preview.database_schema_version, 11,
        "the preview must report the archive's original version"
    );

    let (target_root, target_db, target_images, _) = fixture("newer", false);
    let target = Connection::open(&target_db).unwrap();
    target
        .execute_batch(
            "INSERT INTO notes (id,project_id,title,content,note_type,status)
           VALUES ('old-task','p','Old task','','todo','open');
         INSERT INTO note_task_links (source_note_id,task_note_id)
           VALUES ('n','old-task');",
        )
        .unwrap();
    drop(target);
    let safety = target_root.path().join("safety-v11.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &old_archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();
    let target = Connection::open(&target_db).unwrap();
    let link_count: i64 = target
        .query_row("SELECT count(*) FROM note_task_links", [], |r| r.get(0))
        .unwrap();
    let note_count: i64 = target
        .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
        .unwrap();
    assert_eq!((link_count, note_count), (0, 1));
    assert_eq!(check_db(&target).unwrap().0, DB_SCHEMA_VERSION);
}

#[test]
fn v12_archive_restores_note_task_links_and_their_dates() {
    let (_source_root, source_db, source_images, archive) = fixture("source", false);
    let source = Connection::open(&source_db).unwrap();
    source
        .execute_batch(
            "INSERT INTO notes (id,project_id,title,content,note_type,status)
           VALUES ('task','p','Task','','todo','open');
         INSERT INTO note_task_links (source_note_id,task_note_id,created_at)
           VALUES ('n','task','2026-09-24T10:00:00.000Z');
         UPDATE notes SET note_type='note' WHERE id='task';",
        )
        .unwrap();
    drop(source);
    create_archive(&source_db, &source_images, &archive).unwrap();
    let preview = inspect_archive(&archive).unwrap();
    assert_eq!(preview.database_schema_version, DB_SCHEMA_VERSION);

    let (target_root, target_db, target_images, _) = fixture("target", false);
    let target = Connection::open(&target_db).unwrap();
    target
        .execute_batch(
            "INSERT INTO notes (id,project_id,title,content,note_type,status)
           VALUES ('obsolete','p','Old','','todo','open');
         INSERT INTO note_task_links (source_note_id,task_note_id)
           VALUES ('n','obsolete');",
        )
        .unwrap();
    drop(target);
    let safety = target_root.path().join("safety-v12.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();
    let target = Connection::open(&target_db).unwrap();
    let links: Vec<(String, String, String)> = target
        .prepare("SELECT source_note_id,task_note_id,created_at FROM note_task_links")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        links,
        vec![("n".into(), "task".into(), "2026-09-24T10:00:00.000Z".into())]
    );
    let task_type: String = target
        .query_row("SELECT note_type FROM notes WHERE id='task'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(task_type, "note");
    let obsolete_count: i64 = target
        .query_row("SELECT count(*) FROM notes WHERE id='obsolete'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(obsolete_count, 0);
    drop(target);
    let roundtrip = target_root.path().join("roundtrip-v12.humcase.zip");
    create_archive(&target_db, &target_images, &roundtrip).unwrap();
    assert_eq!(
        inspect_archive(&roundtrip).unwrap().database_schema_version,
        DB_SCHEMA_VERSION
    );
}

#[test]
fn archive_with_note_task_link_across_projects_is_rejected() {
    let (_root, db, _images, output) = fixture("", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO projects (id,name) VALUES ('other','Other');
         INSERT INTO notes (id,project_id,title,content,note_type,status)
           VALUES ('task','other','Task','','todo','open');
         DROP TRIGGER note_task_links_same_project_insert;
         INSERT INTO note_task_links (source_note_id,task_note_id) VALUES ('n','task');",
    )
    .unwrap();
    drop(conn);
    archive_db_for_test(&db, &output, DB_SCHEMA_VERSION);
    assert!(inspect_archive(&output)
        .unwrap_err()
        .contains("Liens pensée-tâche invalides"));
}

#[test]
fn archive_with_note_task_cycle_is_rejected() {
    let (_root, db, _images, output) = fixture("", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO notes (id,project_id,title,content,note_type,status) VALUES
           ('a','p','Task A','','todo','open'),
           ('b','p','Task B','','todo','open');
         INSERT INTO note_task_links (source_note_id,task_note_id) VALUES ('n','a'), ('a','b');
         DROP TRIGGER note_task_links_no_cycles_insert;
         INSERT INTO note_task_links (source_note_id,task_note_id) VALUES ('b','n');",
    )
    .unwrap();
    drop(conn);
    archive_db_for_test(&db, &output, DB_SCHEMA_VERSION);
    assert!(inspect_archive(&output)
        .unwrap_err()
        .contains("Cycle de liens pensée-tâche"));
}

#[test]
fn long_acyclic_note_task_chain_is_valid_for_archive() {
    let (_root, db, _images, _output) = fixture("", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON; BEGIN IMMEDIATE;")
        .unwrap();
    let mut source = "n".to_string();
    for index in 0..2000 {
        let task = format!("task-{index}");
        conn.execute(
            "INSERT INTO notes (id,project_id,title,content,note_type,status)
             VALUES (?1,'p',?1,'','todo','open')",
            [&task],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO note_task_links (source_note_id,task_note_id) VALUES (?1,?2)",
            (&source, &task),
        )
        .unwrap();
        source = task;
    }
    conn.execute_batch("COMMIT;").unwrap();
    assert_eq!(check_db(&conn).unwrap().2, 2001);
}

#[test]
fn archive_with_invalid_project_space_is_rejected() {
    let (_root, db, _images, output) = fixture("", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "PRAGMA ignore_check_constraints=ON; UPDATE projects SET space='unknown' WHERE id='p';",
    )
    .unwrap();
    drop(conn);
    archive_db_for_test(&db, &output, DB_SCHEMA_VERSION);
    assert!(inspect_archive(&output).unwrap_err().contains("invalide"));
}

#[test]
fn archive_with_cross_space_activity_link_is_rejected() {
    let (_root, db, _images, output) = fixture("", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "UPDATE projects SET space='work' WHERE id='p';
         INSERT INTO activities (id, name, space) VALUES ('a', 'Private', 'private');
         DROP TRIGGER project_activities_same_space_insert;
         INSERT INTO project_activities VALUES ('p', 'a');",
    )
    .unwrap();
    drop(conn);
    archive_db_for_test(&db, &output, DB_SCHEMA_VERSION);
    assert!(inspect_archive(&output)
        .unwrap_err()
        .contains("Liens d’activité invalides"));
}

#[test]
fn archive_with_activity_link_to_inbox_is_rejected() {
    let (_root, db, _images, output) = fixture("", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "DROP TRIGGER inbox_flag_immutable;
         UPDATE projects SET space='private', is_inbox=1 WHERE id='p';
         INSERT INTO activities (id, name, space) VALUES ('a', 'Private', 'private');
         DROP TRIGGER project_activities_same_space_insert;
         INSERT INTO project_activities VALUES ('p', 'a');",
    )
    .unwrap();
    drop(conn);
    archive_db_for_test(&db, &output, DB_SCHEMA_VERSION);
    assert!(inspect_archive(&output)
        .unwrap_err()
        .contains("Liens d’activité invalides"));
}

#[test]
fn archive_with_invalid_inbox_flag_is_rejected() {
    let (_root, db, _images, output) = fixture("", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch("DROP TRIGGER inbox_flag_immutable; PRAGMA ignore_check_constraints=ON; UPDATE projects SET is_inbox=2 WHERE id='p';").unwrap();
    drop(conn);
    archive_db_for_test(&db, &output, DB_SCHEMA_VERSION);
    assert!(inspect_archive(&output).is_err());
}

#[test]
fn archive_with_unassigned_inbox_is_rejected() {
    let (_root, db, _images, output) = fixture("", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "DROP TRIGGER inbox_requires_space_update;
         DROP TRIGGER inbox_flag_immutable;
         UPDATE projects SET is_inbox=1 WHERE id='p';",
    )
    .unwrap();
    drop(conn);
    archive_db_for_test(&db, &output, DB_SCHEMA_VERSION);
    assert!(inspect_archive(&output)
        .unwrap_err()
        .contains("Réceptions invalides"));
}

#[test]
fn archive_with_duplicate_inboxes_is_rejected() {
    let (_root, db, _images, output) = fixture("", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "DROP INDEX idx_projects_one_inbox_per_space;
         DROP TRIGGER inbox_flag_immutable;
         UPDATE projects SET space='private', is_inbox=1 WHERE id='p';
         INSERT INTO projects (id, name, space, is_inbox) VALUES ('other', 'Other inbox', 'private', 1);",
    ).unwrap();
    drop(conn);
    archive_db_for_test(&db, &output, DB_SCHEMA_VERSION);
    assert!(inspect_archive(&output)
        .unwrap_err()
        .contains("Réceptions multiples"));
}

#[test]
fn export_rewrites_copy_only_and_inspects() {
    let (_root, db, images, output) = fixture("", true);
    let url = asset_url(&images.join("image.png"));
    let asset_scheme = url.replacen("http://asset.localhost/", "asset://localhost/", 1);
    let content = format!("![capture]({url}) ![second]({asset_scheme})");
    set_note_content(&db, &content);
    fs::write(db.parent().unwrap().join("ai-config.json"), b"secret").unwrap();
    create_archive(&db, &images, &output).unwrap();
    let preview = inspect_archive(&output).unwrap();
    assert_eq!(preview.note_count, 1);
    assert_eq!(preview.image_count, 1);
    let live = Connection::open(&db).unwrap();
    let live_content: String = live
        .query_row("SELECT content FROM notes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(live_content, content);
    let mut zip = ZipArchive::new(File::open(&output).unwrap()).unwrap();
    assert!(zip.by_name("ai-config.json").is_err());
    let mut bytes = Vec::new();
    zip.by_name("humcase.db")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let extracted = output.with_extension("extracted.db");
    fs::write(&extracted, bytes).unwrap();
    let archived =
        Connection::open_with_flags(&extracted, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(check_db(&archived).unwrap().2, 1);
    assert!(!extracted
        .with_file_name(format!(
            "{}-wal",
            extracted.file_name().unwrap().to_string_lossy()
        ))
        .exists());
    let archived_content: String = archived
        .query_row("SELECT content FROM notes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        archived_content,
        "![capture](humcase-archive://images/image.png) ![second](humcase-archive://images/image.png)"
    );
}

#[test]
fn resolution_comment_image_survives_portable_archive_and_restore() {
    let (_source_root, db, images, output) = fixture("Report with proof", true);
    let source_url = asset_url(&images.join("image.png"));
    Connection::open(&db)
        .unwrap()
        .execute(
            "UPDATE notes SET resolution_comment=?1 WHERE id='n'",
            [format!("![proof]({source_url})")],
        )
        .unwrap();
    create_archive(&db, &images, &output).unwrap();

    // The image is referenced only by the report: it must be made portable and embedded.
    let mut zip = ZipArchive::new(File::open(&output).unwrap()).unwrap();
    assert!(zip.by_name("images/image.png").is_ok());
    let mut bytes = Vec::new();
    zip.by_name("humcase.db")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    drop(zip);
    let extracted = output.with_extension("comment.db");
    fs::write(&extracted, bytes).unwrap();
    let archived =
        Connection::open_with_flags(&extracted, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let archived_comment: String = archived
        .query_row(
            "SELECT resolution_comment FROM notes WHERE id='n'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!archived_comment.contains("http://asset.localhost/"));
    assert!(archived_comment.contains("humcase-archive://images/image.png"));
    drop(archived);

    let (target_root, target_db, target_images, _) = fixture("Target", false);
    let preview = inspect_archive(&output).unwrap();
    let safety = target_root.path().join("safety-comment.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &output,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();
    let restored: String = Connection::open(&target_db)
        .unwrap()
        .query_row(
            "SELECT resolution_comment FROM notes WHERE id='n'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let restored_image = fs::read_dir(&target_images)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(restored.contains(&crate::images::reference(
        restored_image.file_name().unwrap().to_str().unwrap()
    )));
    assert!(!restored.contains("humcase-archive://"));
    assert!(!restored.contains(&source_url));
}

#[test]
fn export_excludes_orphan_images_but_keeps_resolution_comment_images() {
    let (_root, db, images, output) = fixture("Text without image", true);
    fs::write(images.join("orphan.png"), b"orphan").unwrap();
    let url = asset_url(&images.join("image.png"));
    Connection::open(&db)
        .unwrap()
        .execute(
            "UPDATE notes SET resolution_comment=?1 WHERE id='n'",
            [format!("![proof]({url})")],
        )
        .unwrap();
    create_archive(&db, &images, &output).unwrap();
    let preview = inspect_archive(&output).unwrap();
    assert_eq!(preview.image_count, 1);
    let mut zip = ZipArchive::new(File::open(&output).unwrap()).unwrap();
    assert!(zip.by_name("images/image.png").is_ok());
    assert!(zip.by_name("images/orphan.png").is_err());
    drop(zip);
    // The disk cleanup policy is out of scope: the orphan file stays.
    assert!(images.join("orphan.png").exists());
}

/// R2 — an INVALID orphan (forbidden name, directory, too large) must no longer
/// make the export fail: the checks only apply to referenced images. Without the
/// reorganization (validation of the whole folder before the filter), this test fails on the
/// first invalid orphan.
#[test]
fn export_succeeds_with_invalid_orphan_images() {
    let (_root, db, images, output) = fixture("Text without image", true);
    // Orphan with a forbidden name (leading dot) — rejected by image_name().
    fs::write(images.join(".hidden"), b"x").unwrap();
    // "Directory" orphan — rejected by is_file().
    fs::create_dir(images.join("orphan_dir")).unwrap();
    // Orphan that is too large (> MAX_IMAGE) — refused by the size bound.
    let big = images.join("toobig.bin");
    let file = File::create(&big).unwrap();
    file.set_len(MAX_IMAGE + 1).unwrap();
    drop(file);
    let url = asset_url(&images.join("image.png"));
    Connection::open(&db)
        .unwrap()
        .execute(
            "UPDATE notes SET resolution_comment=?1 WHERE id='n'",
            [format!("![proof]({url})")],
        )
        .unwrap();

    // R2: the export must succeed despite these invalid orphans.
    create_archive(&db, &images, &output).unwrap();

    let preview = inspect_archive(&output).unwrap();
    assert_eq!(preview.image_count, 1);
    let mut zip = ZipArchive::new(File::open(&output).unwrap()).unwrap();
    assert!(zip.by_name("images/image.png").is_ok());
    assert!(zip.by_name("images/.hidden").is_err());
    assert!(zip.by_name("images/toobig.bin").is_err());
    assert!(zip.by_name("images/orphan_dir").is_err());
    drop(zip);
}

/// R2 — the safeguards on a REFERENCED image do not weaken: the scan →
/// references → bounded validation path must keep "an invalid referenced file always
/// fails the export" (guardian no. 5 of the invariants).
#[test]
fn referenced_image_too_large_still_rejects_export() {
    let (_root, db, images, output) = fixture("Text", true);
    let oversized = images.join("oversized.png");
    let file = File::create(&oversized).unwrap();
    file.set_len(MAX_IMAGE + 1).unwrap();
    drop(file);
    let url = asset_url(&oversized);
    set_note_content(&db, &format!("![x]({url})"));
    let error = create_archive(&db, &images, &output).unwrap_err();
    // Errors shown to the user stay localized (French here).
    assert!(error.contains("volumineux"), "unexpected message: {error}");
    assert!(!output.exists());
}

/// The same folder may contain a healthy referenced image and another referenced one that is
/// too large: only the second makes the export fail, the first is not exported.
#[test]
fn referenced_image_too_large_rejects_even_with_valid_referenced_image() {
    let (_root, db, images, output) = fixture("Text without image", true);
    let oversized = images.join("oversized.png");
    let file = File::create(&oversized).unwrap();
    file.set_len(MAX_IMAGE + 1).unwrap();
    drop(file);
    let url_oversized = asset_url(&oversized);
    let url_ok = asset_url(&images.join("image.png"));
    set_note_content(
        &db,
        &format!("![large]({url_oversized}) ![valid]({url_ok})"),
    );
    let error = create_archive(&db, &images, &output).unwrap_err();
    assert!(
        error.contains("oversized.png"),
        "unexpected message: {error}"
    );
    assert!(!output.exists());
}

/// A referenced image replaced by a directory is rejected (the old folder scan
/// already failed on an unreferenced directory; the new check only applies to
/// referenced ones, but a referenced non-file stays invalid).
#[test]
fn referenced_image_replaced_by_directory_still_rejects_export() {
    let (_root, db, images, output) = fixture("Text", true);
    fs::remove_file(images.join("image.png")).unwrap();
    fs::create_dir(images.join("image.png")).unwrap();
    let url = asset_url(&images.join("image.png"));
    set_note_content(&db, &format!("![x]({url})"));
    let error = create_archive(&db, &images, &output).unwrap_err();
    assert!(error.contains("image.png"), "unexpected message: {error}");
    assert!(!output.exists());
}

/// A referenced symbolic link is rejected like a non-file entry: the archive must
/// never embed the content of a target outside the images folder. Creating a Windows
/// symlink requires developer mode or a privilege: without it, the test bows out
/// (the lack of coverage is then logged, reading the code is authoritative).
// Windows only: the test creates the link with the Windows API (cloud sessions on Linux).
#[cfg(windows)]
#[test]
fn referenced_symlink_is_rejected() {
    let (_root, db, images, output) = fixture("Text", true);
    let target = images.join("target-outside-folder.txt");
    fs::write(&target, b"outside content").unwrap();
    let link = images.join("image.png");
    fs::remove_file(&link).unwrap();
    match std::os::windows::fs::symlink_file(&target, &link) {
        Ok(()) => {
            let url = asset_url(&link);
            set_note_content(&db, &format!("![x]({url})"));
            let error = create_archive(&db, &images, &output).unwrap_err();
            assert!(error.contains("image.png"), "unexpected message: {error}");
            assert!(!output.exists());
        }
        Err(_) => {
            eprintln!("humcase: symlink not created (missing privilege), test not applicable")
        }
    }
}

/// Unix twin of the test above: symbolic links need no privilege there.
#[cfg(unix)]
#[test]
fn referenced_symlink_is_rejected() {
    let (_root, db, images, output) = fixture("Text", true);
    let target = images.join("target-outside-folder.txt");
    fs::write(&target, b"outside content").unwrap();
    let link = images.join("image.png");
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let url = asset_url(&link);
    set_note_content(&db, &format!("![x]({url})"));
    let error = create_archive(&db, &images, &output).unwrap_err();
    assert!(error.contains("image.png"), "unexpected message: {error}");
    assert!(!output.exists());
}

/// A referenced junction (reparse point, creatable without privilege via mklink /J) is
/// rejected: it really exercises the `symlink_metadata` path that the symlink test
/// above cannot always reach for lack of privilege. The target is a file
/// of normal size: it is the only case that distinguishes following the link (bad) from rejecting.
#[test]
fn referenced_junction_is_rejected() {
    let (_root, db, images, output) = fixture("Text", true);
    let outside = _root.path().join("outside-folder.txt");
    fs::write(&outside, b"outside content").unwrap();
    let link = images.join("image.png");
    fs::remove_file(&link).unwrap();
    // mklink /J creates a junction without elevation; if the tooling is missing, the test bows out
    // (the lack of coverage is then logged, reading the code is authoritative).
    let created = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&link)
        .arg(&outside)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !created {
        eprintln!("humcase: junction not created (mklink unavailable), test not applicable");
        return;
    }
    let url = asset_url(&link);
    set_note_content(&db, &format!("![x]({url})"));
    let error = create_archive(&db, &images, &output).unwrap_err();
    // Specific message: the rejection must come from the TYPE check on the reparse point
    // ("unsupported entry"), not from a traversal error or worse, a silent
    // embedding of the target.
    assert!(
        error.contains("non prise en charge"),
        "unexpected message: {error}"
    );
    assert!(!output.exists());
}

#[test]
fn missing_internal_image_rejects_export() {
    let (_root, db, images, output) = fixture("", false);
    let content = format!("![x]({})", asset_url(&images.join("missing.png")));
    set_note_content(&db, &content);
    assert!(create_archive(&db, &images, &output)
        .unwrap_err()
        .contains("absente"));
    assert!(!output.exists());
}

#[test]
fn foreign_asset_path_is_not_rebased_to_current_image() {
    let (_root, db, images, output) = fixture("", true);
    set_note_content(
        &db,
        "![x](asset://localhost/C:/Other/com.example.devnotes/images/image.png)",
    );
    assert!(create_archive(&db, &images, &output)
        .unwrap_err()
        .contains("hors du dossier courant"));
}

#[test]
fn foreign_key_violation_rejects_export() {
    let (_root, db, images, output) = fixture("text", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    conn.execute("UPDATE notes SET project_id = 'missing' WHERE id = 'n'", [])
        .unwrap();
    drop(conn);
    assert!(create_archive(&db, &images, &output)
        .unwrap_err()
        .contains("Clés étrangères invalides"));
    assert!(!output.exists());
}

fn malformed_zip(names: &[&str]) -> (tempfile::TempDir, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("bad.humcase.zip");
    let file = File::create(&path).unwrap();
    let mut zip = ZipWriter::new(file);
    for name in names {
        zip.start_file(*name, zip_options()).unwrap();
        zip.write_all(b"x").unwrap();
    }
    zip.finish().unwrap();
    (root, path)
}

/// Renames an entry by rewriting its name in the bytes: in its local header only (`all` false),
/// or in its local header and the central directory. The writer refuses these archives itself.
fn rename_in_bytes(path: &Path, from: &str, to: &str, all: bool) {
    assert_eq!(from.len(), to.len());
    let mut bytes = fs::read(path).unwrap();
    let mut found = 0;
    let mut at = 0;
    while let Some(i) = bytes[at..]
        .windows(from.len())
        .position(|w| w == from.as_bytes())
    {
        bytes[at + i..at + i + from.len()].copy_from_slice(to.as_bytes());
        found += 1;
        at += i + from.len();
        if !all {
            break;
        }
    }
    assert!(found > 0);
    fs::write(path, bytes).unwrap();
}

#[test]
fn duplicate_entry_is_rejected() {
    // The reader keeps one entry per name: without the consistency check, the second database
    // would go unseen.
    let (_root, path) = malformed_zip(&["manifest.json", "humcase.db", "humcase.dc"]);
    rename_in_bytes(&path, "humcase.dc", "humcase.db", true);
    assert!(inspect_archive(&path).unwrap_err().contains("dupliqué"));
}

#[test]
fn local_header_naming_another_file_is_rejected() {
    // A local header that names another file than the central directory ("zip cloaking").
    let (_root, path) = malformed_zip(&["manifest.json", "humcase.db", "images/a.png"]);
    rename_in_bytes(&path, "images/a.png", "images/b.png", false);
    assert!(inspect_archive(&path).unwrap_err().contains("incohérente"));
}

#[test]
fn traversal_entry_is_rejected() {
    let (_root, path) = malformed_zip(&["manifest.json", "humcase.db", "images/../evil.png"]);
    assert!(inspect_archive(&path).unwrap_err().contains("invalide"));
}

#[test]
fn existing_archive_is_not_replaced() {
    let (_root, db, images, output) = fixture("text", false);
    fs::write(&output, b"original").unwrap();
    assert!(create_archive(&db, &images, &output).is_err());
    assert_eq!(fs::read(&output).unwrap(), b"original");
}

#[test]
fn altered_image_fails_hash_validation() {
    let (_root, db, images, output) = fixture("text", true);
    let url = asset_url(&images.join("image.png"));
    set_note_content(&db, &format!("![x]({url})"));
    create_archive(&db, &images, &output).unwrap();
    let bad = output.with_file_name("altered.humcase.zip");
    let mut original = ZipArchive::new(File::open(&output).unwrap()).unwrap();
    let mut new_zip = ZipWriter::new(File::create(&bad).unwrap());
    for index in 0..original.len() {
        let mut item = original.by_index(index).unwrap();
        let name = item.name().unwrap().to_string();
        let mut data = Vec::new();
        item.read_to_end(&mut data).unwrap();
        if name == "images/image.png" {
            data[0] ^= 1;
        }
        new_zip.start_file(name, zip_options()).unwrap();
        new_zip.write_all(&data).unwrap();
    }
    new_zip.finish().unwrap();
    assert!(inspect_archive(&bad)
        .unwrap_err()
        .contains("Empreinte invalide"));
}

#[test]
fn portable_reference_without_image_is_rejected() {
    let (_root, db, images, output) = fixture("![x](humcase-archive://images/missing.png)", false);
    assert!(create_archive(&db, &images, &output)
        .unwrap_err()
        .contains("Image interne absente"));
    assert!(!output.exists());
}

#[test]
fn invalid_sqlite_is_rejected_even_with_matching_hash() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("invalid.humcase.zip");
    let db = b"not a sqlite database";
    let manifest = Manifest {
        format_version: FORMAT_VERSION,
        created_at: "2026-09-23T12:00:00.000Z".to_string(),
        database_schema_version: DB_SCHEMA_VERSION,
        entries: vec![Entry {
            path: "humcase.db".to_string(),
            size: db.len() as u64,
            sha256: crate::images::lower_hex(&Sha256::digest(db)),
        }],
    };
    let mut zip = ZipWriter::new(File::create(&path).unwrap());
    zip.start_file("humcase.db", zip_options()).unwrap();
    zip.write_all(db).unwrap();
    zip.start_file("manifest.json", zip_options()).unwrap();
    zip.write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    zip.finish().unwrap();
    assert!(inspect_archive(&path)
        .unwrap_err()
        .contains("SQLite invalide"));
}

#[test]
fn restore_replaces_complete_rows_and_keeps_old_images_with_safety_archive() {
    let (_source_root, source_db, source_images, archive) = fixture("", true);
    fs::write(source_images.join("image.png"), b"new image").unwrap();
    let new_url = asset_url(&source_images.join("image.png"));
    let source = Connection::open(&source_db).unwrap();
    source.execute("UPDATE projects SET color='#123456', created_at='2020-01-01T00:00:00.000Z', space='private', revision=7, updated_at='2023-05-06T07:08:09.010Z' WHERE id='p'", []).unwrap();
    crate::db::queries::append_raw_content(&source, "n", "Dictation").unwrap();
    source
        .execute(
            "UPDATE notes SET title='New', content=?1, note_type='todo', status='resolved',
         resolution_comment='Done', priority='high', git_branch='feature/x',
         tags='[\"rust\"]', created_at='2021-01-01T00:00:00.000Z',
         updated_at='2022-01-01T00:00:00.000Z', due_at='2026-10-01' WHERE id='n'",
            [format!("![new]({new_url}) unique-restored-token")],
        )
        .unwrap();
    source
        .execute(
            "INSERT INTO projects (id,name,color,space) VALUES ('p2','Second','#abcdef','work')",
            [],
        )
        .unwrap();
    source.execute("INSERT INTO notes (id,project_id,title,content,note_type,status) VALUES ('n2','p2','Two','other','note','open')", []).unwrap();
    source
        .execute_batch(
            "INSERT INTO activities (id, name, space, created_at) VALUES
           ('a1', 'Personal', 'private', '2020-01-02T00:00:00.000Z'),
           ('a2', 'Work', 'work', '2020-01-03T00:00:00.000Z');
         INSERT INTO project_activities VALUES ('p', 'a1'), ('p2', 'a2');
         UPDATE activities SET revision=3, updated_at='2024-01-02T03:04:05.006Z' WHERE id='a1';",
        )
        .unwrap();
    drop(source);
    create_archive(&source_db, &source_images, &archive).unwrap();
    let expected_hash = inspect_archive(&archive).unwrap().archive_sha256;

    let (target_root, target_db, target_images, _) = fixture("", true);
    fs::write(target_images.join("image.png"), b"old image").unwrap();
    let old_url = asset_url(&target_images.join("image.png"));
    let target = Connection::open(&target_db).unwrap();
    target
        .execute(
            "UPDATE notes SET id='old', title='Former', content=?1 WHERE id='n'",
            [format!("![old]({old_url})")],
        )
        .unwrap();
    target
        .execute_batch(
            "UPDATE projects SET space='work' WHERE id='p';
         INSERT INTO activities (id, name, space) VALUES ('obsolete', 'Old', 'work');
         INSERT INTO project_activities VALUES ('p', 'obsolete');",
        )
        .unwrap();
    drop(target);
    let safety = target_root.path().join("safety.humcase.zip");
    let summary = restore_archive_into(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &expected_hash,
    )
    .unwrap();
    assert_eq!(
        (
            summary.project_count,
            summary.note_count,
            summary.image_count
        ),
        (2, 2, 1)
    );
    assert_eq!(summary.safety_archive_path, safety);
    let safety_preview = inspect_archive(&safety).unwrap();
    assert_eq!(
        (
            safety_preview.project_count,
            safety_preview.note_count,
            safety_preview.image_count
        ),
        (1, 1, 1)
    );
    assert_eq!(
        fs::read(target_images.join("image.png")).unwrap(),
        b"old image"
    );
    let imported = Connection::open(&target_db).unwrap();
    assert_eq!(check_db(&imported).unwrap().2, 2);
    let old_count: i64 = imported
        .query_row("SELECT count(*) FROM notes WHERE id='old'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(old_count, 0);
    let row: (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ) = imported
        .query_row(
            "SELECT title, content, note_type, status, resolution_comment, raw_content, priority,
         git_branch, tags, created_at, updated_at, due_at FROM notes WHERE id='n'",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(row.0, "New");
    assert_eq!(row.2, "todo");
    assert_eq!(row.3, "resolved");
    assert_eq!(row.4, "Done");
    assert_eq!(row.5, "Dictation");
    assert_eq!(row.6, "high");
    assert_eq!(row.7, "feature/x");
    assert_eq!(row.8, "[\"rust\"]");
    assert_eq!(row.9, "2021-01-01T00:00:00.000Z");
    assert_eq!(row.10, "2022-01-01T00:00:00.000Z");
    assert_eq!(row.11, "2026-10-01");
    assert!(!row.1.contains("humcase-archive://"));
    assert!(
        row.1.contains("humcase-image://"),
        "portable reference after restore"
    );
    let imported_image = fs::read_dir(&target_images)
        .unwrap()
        .map(|item| item.unwrap().path())
        .find(|path| path.file_name().unwrap() != "image.png")
        .unwrap();
    assert_eq!(fs::read(&imported_image).unwrap(), b"new image");
    assert!(row.1.contains(&crate::images::reference(
        imported_image.file_name().unwrap().to_str().unwrap()
    )));
    assert!(
        !row.1.contains("asset.localhost"),
        "no local path left in the restored content"
    );
    let fts_count: i64 = imported
        .query_row(
            "SELECT count(*) FROM notes_fts WHERE notes_fts MATCH '\"unique-restored-token\"'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fts_count, 1);
    let project: (String, String) = imported
        .query_row(
            "SELECT color, created_at FROM projects WHERE id='p'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        project,
        (
            "#123456".to_string(),
            "2020-01-01T00:00:00.000Z".to_string()
        )
    );
    // The restore copies the concurrency version of the archive's projects and activities.
    let versions: (i64, String, i64, String) = imported
        .query_row(
            "SELECT p.revision, p.updated_at, a.revision, a.updated_at FROM projects p, activities a WHERE p.id='p' AND a.id='a1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        versions,
        (
            7,
            "2023-05-06T07:08:09.010Z".to_string(),
            3,
            "2024-01-02T03:04:05.006Z".to_string()
        )
    );
    let spaces: Vec<(String, Option<String>)> = imported
        .prepare("SELECT id, space FROM projects ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        spaces,
        vec![
            ("p".to_string(), Some("private".to_string())),
            ("p2".to_string(), Some("work".to_string()))
        ]
    );
    let activities: Vec<(String, String, String, String)> = imported
        .prepare("SELECT id, name, space, created_at FROM activities ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        activities,
        vec![
            (
                "a1".into(),
                "Personal".into(),
                "private".into(),
                "2020-01-02T00:00:00.000Z".into()
            ),
            (
                "a2".into(),
                "Work".into(),
                "work".into(),
                "2020-01-03T00:00:00.000Z".into()
            ),
        ]
    );
    let links: Vec<(String, String)> = imported
        .prepare("SELECT project_id, activity_id FROM project_activities ORDER BY project_id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        links,
        vec![("p".into(), "a1".into()), ("p2".into(), "a2".into())]
    );
    drop(imported);
    let roundtrip = target_root.path().join("roundtrip.humcase.zip");
    create_archive(&target_db, &target_images, &roundtrip).unwrap();
    // The target's old image is no longer referenced after the restore: orphan,
    // it stays on disk but does not enter the new archive.
    assert_eq!(inspect_archive(&roundtrip).unwrap().image_count, 1);
}

#[test]
fn restore_preserves_inbox_and_replaces_previous_inbox() {
    let (_source_root, source_db, source_images, archive) = fixture("source", false);
    let source = Connection::open(&source_db).unwrap();
    source.execute("INSERT INTO projects (id, name, space, is_inbox) VALUES ('inbox-private', 'Private inbox', 'private', 1)", []).unwrap();
    drop(source);
    create_archive(&source_db, &source_images, &archive).unwrap();
    let expected_hash = inspect_archive(&archive).unwrap().archive_sha256;

    let (target_root, target_db, target_images, _) = fixture("target", false);
    let target = Connection::open(&target_db).unwrap();
    target.execute("INSERT INTO projects (id, name, space, is_inbox) VALUES ('inbox-work', 'Work inbox', 'work', 1)", []).unwrap();
    drop(target);
    let safety = target_root.path().join("safety-inbox.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &expected_hash,
    )
    .unwrap();
    let target = Connection::open(&target_db).unwrap();
    let inboxes: Vec<(String, String)> = target
        .prepare("SELECT id, space FROM projects WHERE is_inbox = 1")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(inboxes, vec![("inbox-private".into(), "private".into())]);
    assert_eq!(check_db(&target).unwrap().0, DB_SCHEMA_VERSION);
}

#[test]
fn injected_failure_rolls_back_and_removes_staged_images() {
    let (_source_root, source_db, source_images, archive) = fixture("", true);
    let url = asset_url(&source_images.join("image.png"));
    set_note_content(&source_db, &format!("![x]({url})"));
    Connection::open(&source_db)
        .unwrap()
        .execute_batch(
            "UPDATE projects SET space='private' WHERE id='p';
         INSERT INTO activities (id, name, space) VALUES ('new', 'New one', 'private');
         INSERT INTO project_activities VALUES ('p', 'new');
         INSERT INTO notes (id,project_id,title,content,note_type,status)
           VALUES ('new-task','p','New task','','todo','open');
         INSERT INTO note_task_links (source_note_id,task_note_id)
           VALUES ('n','new-task');",
        )
        .unwrap();
    create_archive(&source_db, &source_images, &archive).unwrap();
    let expected_hash = inspect_archive(&archive).unwrap().archive_sha256;
    let (target_root, target_db, target_images, _) = fixture("old text", true);
    // Same name, different content: real collision, the archive image is written under a
    // new name ("staged"). With the same content it would be reused and there would be nothing to remove.
    fs::write(target_images.join("image.png"), b"local image").unwrap();
    Connection::open(&target_db)
        .unwrap()
        .execute_batch(
            "UPDATE projects SET space='work' WHERE id='p';
         INSERT INTO activities (id, name, space) VALUES ('old', 'Old', 'work');
         INSERT INTO project_activities VALUES ('p', 'old');
         INSERT INTO notes (id,project_id,title,content,note_type,status)
           VALUES ('old-task','p','Old task','','todo','open');
         INSERT INTO note_task_links (source_note_id,task_note_id)
           VALUES ('n','old-task');",
        )
        .unwrap();
    let safety = target_root.path().join("safety.humcase.zip");
    let error = super::restore::restore_with_failure(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &expected_hash,
        true,
        false,
    )
    .unwrap_err();
    assert!(
        matches!(error, RestoreFailure::BeforeCommit(_)),
        "a failure before COMMIT must stay classified BeforeCommit, received: {error:?}"
    );
    assert!(error.message().contains("Injected failure"));
    let conn = Connection::open(&target_db).unwrap();
    let content: String = conn
        .query_row("SELECT content FROM notes WHERE id='n'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(content, "old text");
    let links: Vec<(String, String)> = conn
        .prepare("SELECT project_id, activity_id FROM project_activities")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(links, vec![("p".into(), "old".into())]);
    let activity_count: i64 = conn
        .query_row("SELECT count(*) FROM activities", [], |r| r.get(0))
        .unwrap();
    assert_eq!(activity_count, 1);
    let note_links: Vec<(String, String)> = conn
        .prepare("SELECT source_note_id,task_note_id FROM note_task_links")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(note_links, vec![("n".into(), "old-task".into())]);
    assert_eq!(fs::read_dir(&target_images).unwrap().count(), 1);
    assert_eq!(inspect_archive(&safety).unwrap().note_count, 2);
}

#[test]
fn commit_failure_is_ambiguous_and_keeps_staged_images() {
    let (_source_root, source_db, source_images, archive) = fixture("New version", true);
    let url = asset_url(&source_images.join("image.png"));
    set_note_content(&source_db, &format!("![x]({url})"));
    create_archive(&source_db, &source_images, &archive).unwrap();
    let expected_hash = inspect_archive(&archive).unwrap().archive_sha256;
    let (target_root, target_db, target_images, _) = fixture("old text", true);
    // Real collision (same name, different content): the archive image is written under a new name.
    fs::write(target_images.join("image.png"), b"local image").unwrap();
    let safety = target_root.path().join("safety.humcase.zip");
    let error = super::restore::restore_with_failure(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &expected_hash,
        false,
        true,
    )
    .unwrap_err();
    // A COMMIT failure may mean SQLite published: the class must stay
    // CommitAmbiguous. Moving the COMMIT into the pre-commit closure would make this test
    // become BeforeCommit and fail.
    assert!(
        matches!(error, RestoreFailure::CommitAmbiguous(_)),
        "a COMMIT failure must be classified CommitAmbiguous, received: {error:?}"
    );
    assert!(
        error.message().contains("Échec pendant la publication"),
        "message: {}",
        error.message()
    );
    // Mirror of the `fail_before_commit` test: here the prepared images stay on disk
    // (they may be referenced if the transaction was published).
    assert_eq!(
        fs::read_dir(&target_images).unwrap().count(),
        2,
        "the prepared images must be kept after a COMMIT failure"
    );
    // The safety archive of the old state was published before any live modification.
    assert!(safety.exists(), "the safety archive must exist");
    assert_eq!(inspect_archive(&safety).unwrap().note_count, 1);
}

#[test]
fn corrupt_archive_rejects_before_any_mutation_or_safety_archive() {
    let (_source_root, source_db, source_images, archive) = fixture("new", true);
    create_archive(&source_db, &source_images, &archive).unwrap();
    let expected_hash = inspect_archive(&archive).unwrap().archive_sha256;
    let bad = archive.with_file_name("bad.humcase.zip");
    fs::write(&bad, b"bad zip").unwrap();
    let (target_root, target_db, target_images, _) = fixture("old", true);
    let safety = target_root.path().join("safety.humcase.zip");
    assert!(
        restore_archive_into(&target_db, &target_images, &bad, &safety, &expected_hash).is_err()
    );
    assert!(!safety.exists());
    let conn = Connection::open(&target_db).unwrap();
    let content: String = conn
        .query_row("SELECT content FROM notes WHERE id='n'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(content, "old");
    assert_eq!(fs::read_dir(&target_images).unwrap().count(), 1);
}

#[test]
fn changed_valid_archive_is_rejected_before_mutation() {
    let (_source_root, source_db, source_images, first) = fixture("version one", false);
    create_archive(&source_db, &source_images, &first).unwrap();
    let expected_hash = inspect_archive(&first).unwrap().archive_sha256;
    set_note_content(&source_db, "version two");
    let second = first.with_file_name("second.humcase.zip");
    create_archive(&source_db, &source_images, &second).unwrap();
    let (target_root, target_db, target_images, _) = fixture("old", false);
    let safety = target_root.path().join("safety.humcase.zip");
    let error = restore_archive_into(&target_db, &target_images, &second, &safety, &expected_hash)
        .unwrap_err();
    assert!(
        matches!(error, RestoreFailure::BeforeCommit(_)),
        "a preparation rejection must be classified BeforeCommit, received: {error:?}"
    );
    assert!(error.message().contains("changé"));
    assert!(!safety.exists());
    let content: String = Connection::open(&target_db)
        .unwrap()
        .query_row("SELECT content FROM notes WHERE id='n'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(content, "old");
}

/// New archives are always named `.humcase.zip`.
#[test]
fn test_safe_archive_name_forces_the_humcase_extension() {
    assert_eq!(
        safe_archive_name("humcase-2026.humcase.zip"),
        "humcase-2026.humcase.zip"
    );
    assert_eq!(
        safe_archive_name("notes.devnotes.zip"),
        "humcase-backup.humcase.zip"
    );
    assert_eq!(safe_archive_name("notes.zip"), "humcase-backup.humcase.zip");
    // Relative path neutralized: the name would start with ".", we fall back to the default name.
    assert_eq!(
        safe_archive_name("../x.humcase.zip"),
        "humcase-backup.humcase.zip"
    );
}

/// 0.9.81: a note that references its image as `humcase-image://` exports and restores; the
/// restore writes the portable reference of the renamed file, never a local path.
#[test]
fn portable_image_reference_roundtrips_through_archive() {
    let (_root, db, images, output) = fixture("![capture](humcase-image://image.png)", true);
    create_archive(&db, &images, &output).unwrap();
    let preview = inspect_archive(&output).unwrap();
    let (target_root, target_db, target_images, _) = fixture("Before", false);
    let safety = target_root.path().join("safety.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &output,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();
    let content: String = Connection::open(&target_db)
        .unwrap()
        .query_row("SELECT content FROM notes WHERE id='n'", [], |r| r.get(0))
        .unwrap();
    let restored = fs::read_dir(&target_images)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        content,
        format!(
            "![capture]({})",
            crate::images::reference(restored.file_name().unwrap().to_str().unwrap())
        )
    );
    assert_eq!(fs::read(&restored).unwrap(), b"image bytes");
}

#[test]
fn portable_image_reference_without_file_is_rejected() {
    let (_root, db, images, output) = fixture("![x](humcase-image://missing-file.png)", true);
    assert!(create_archive(&db, &images, &output)
        .unwrap_err()
        .contains("missing-file.png"));
}

#[test]
fn trashed_note_with_image_roundtrips_through_archive_and_stays_in_trash() {
    let (_root, db, images, output) = fixture("![capture](humcase-image://image.png)", true);
    crate::db::queries::trash_note(&Connection::open(&db).unwrap(), "n").unwrap();
    create_archive(&db, &images, &output).unwrap();
    let preview = inspect_archive(&output).unwrap();
    let (target_root, target_db, target_images, _) = fixture("Before", false);
    let safety = target_root.path().join("safety.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &output,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();
    let conn = Connection::open(&target_db).unwrap();
    let (content, deleted_at): (String, Option<String>) = conn
        .query_row(
            "SELECT content, deleted_at FROM notes WHERE id='n'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let restored = fs::read_dir(&target_images)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        content,
        format!(
            "![capture]({})",
            crate::images::reference(restored.file_name().unwrap().to_str().unwrap())
        )
    );
    assert!(deleted_at.is_some(), "the restored note stays in the trash");
    // The live database keeps the trigger that freezes the content of a trashed note.
    let error = conn
        .execute("UPDATE notes SET content = 'x' WHERE id = 'n'", [])
        .unwrap_err()
        .to_string();
    assert!(error.contains("note is in the trash"), "{error}");
}

/// A referenced image that is missing from the restored images must no longer abort the process
/// (old `expect`): the restore fails cleanly, before any COMMIT.
#[test]
fn missing_archive_image_is_an_error_not_a_panic() {
    let mut urls = BTreeMap::new();
    urls.insert("a.png".to_string(), "humcase-image://a-1.png".to_string());
    assert_eq!(
        super::restore::rewrite_archive_images("![](humcase-archive://images/a.png)", &urls)
            .unwrap(),
        "![](humcase-image://a-1.png)"
    );
    let error = super::restore::rewrite_archive_images(
        "![](humcase-archive://images/a.png) ![](humcase-archive://images/b.png)",
        &urls,
    )
    .unwrap_err();
    assert!(error.contains("b.png"), "{error}");
}

// --- Image registry and restore without renaming (v19, lot S3) ---

fn sha_hex(bytes: &[u8]) -> String {
    crate::images::lower_hex(&Sha256::digest(bytes))
}

fn write_image(images: &Path, name: &str, bytes: &[u8]) {
    fs::write(images.join(name), bytes).unwrap();
}

/// Content (name → bytes) of the images folder.
fn image_files(images: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(images)
        .unwrap()
        .map(|item| {
            let item = item.unwrap();
            (
                item.file_name().into_string().unwrap(),
                fs::read(item.path()).unwrap(),
            )
        })
        .collect()
}

/// Registry rows: (name, sha256, size, registration date).
fn registry(db: &Path) -> Vec<(String, String, i64, String)> {
    let conn = Connection::open(db).unwrap();
    let mut statement = conn
        .prepare("SELECT name, sha256, size, created_at FROM images ORDER BY name")
        .unwrap();
    let rows = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    rows
}

/// (id, content, revision, updated_at) of all notes.
fn note_rows(db: &Path) -> Vec<(String, String, i64, String)> {
    let conn = Connection::open(db).unwrap();
    let mut statement = conn
        .prepare("SELECT id, content, revision, updated_at FROM notes ORDER BY id")
        .unwrap();
    let rows = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    rows
}

fn add_note(db: &Path, id: &str, content: &str) {
    Connection::open(db)
        .unwrap()
        .execute(
            "INSERT INTO notes (id, project_id, title, content, note_type, status) VALUES (?1, 'p', ?1, ?2, 'note', 'open')",
            [id, content],
        )
        .unwrap();
}

/// Registers the images already present, as the application startup does.
fn sync_registry_like_startup(db: &Path, images: &Path) {
    let conn = Connection::open(db).unwrap();
    crate::images::sync_registry(&conn, images).unwrap();
}

fn restore_into(
    root: &tempfile::TempDir,
    db: &Path,
    images: &Path,
    archive: &Path,
    safety: &str,
) -> RestoreSummary {
    let hash = inspect_archive(archive).unwrap().archive_sha256;
    restore_archive_into(db, images, archive, &root.path().join(safety), &hash).unwrap()
}

#[test]
fn restoring_the_same_archive_twice_creates_no_image_and_keeps_every_note() {
    let (_source_root, source_db, source_images, archive) =
        fixture("![a](humcase-image://a.png) text", false);
    write_image(&source_images, "a.png", b"AAA");
    write_image(&source_images, "b.webp", b"BBBB");
    add_note(
        &source_db,
        "n2",
        "![b](humcase-image://b.webp) ![a](humcase-image://a.png)",
    );
    create_archive(&source_db, &source_images, &archive).unwrap();

    let (target_root, target_db, target_images, _) = fixture("", false);
    sync_registry_like_startup(&target_db, &target_images);

    let first = restore_into(
        &target_root,
        &target_db,
        &target_images,
        &archive,
        "safety-1.zip",
    );
    assert_eq!(first.image_count, 2);
    // Original names are kept: no UUID nor duplicate.
    let files = image_files(&target_images);
    assert_eq!(
        files,
        BTreeMap::from([
            ("a.png".to_string(), b"AAA".to_vec()),
            ("b.webp".to_string(), b"BBBB".to_vec())
        ])
    );
    let notes = note_rows(&target_db);
    assert_eq!(notes[0].1, "![a](humcase-image://a.png) text");
    assert_eq!(
        notes[1].1,
        "![b](humcase-image://b.webp) ![a](humcase-image://a.png)"
    );
    let names: Vec<_> = registry(&target_db)
        .into_iter()
        .map(|(name, sha, size, _)| (name, sha, size))
        .collect();
    assert_eq!(
        names,
        vec![
            ("a.png".to_string(), sha_hex(b"AAA"), 3),
            ("b.webp".to_string(), sha_hex(b"BBBB"), 4),
        ]
    );

    let modified = |name: &str| {
        fs::metadata(target_images.join(name))
            .unwrap()
            .modified()
            .unwrap()
    };
    let (files_before, notes_before, registry_before) = (
        image_files(&target_images),
        note_rows(&target_db),
        registry(&target_db),
    );
    let (mtime_a, mtime_b) = (modified("a.png"), modified("b.webp"));

    restore_into(
        &target_root,
        &target_db,
        &target_images,
        &archive,
        "safety-2.zip",
    );

    assert_eq!(
        image_files(&target_images),
        files_before,
        "no image file created, deleted nor modified"
    );
    assert_eq!(
        (modified("a.png"), modified("b.webp")),
        (mtime_a, mtime_b),
        "files not rewritten"
    );
    assert_eq!(
        note_rows(&target_db),
        notes_before,
        "content, revision and updated_at of each note unchanged"
    );
    assert_eq!(
        registry(&target_db),
        registry_before,
        "registry unchanged (registration dates included)"
    );
}

#[test]
fn name_collision_with_different_content_renames_and_rewrites_only_its_references() {
    let (_source_root, source_db, source_images, archive) = fixture(
        "![free](humcase-image://free.png) ![conflict](humcase-image://clash.png)",
        false,
    );
    write_image(&source_images, "free.png", b"free");
    write_image(&source_images, "clash.png", b"archive version");
    write_image(&source_images, "same.png", b"identical");
    // Same size as the local version, different content: size alone is not enough.
    write_image(&source_images, "samesize.png", b"AAAA");
    add_note(&source_db, "n2", "![same](humcase-image://same.png)");
    add_note(
        &source_db,
        "n3",
        "![c](humcase-image://clash.png) ![s](humcase-image://samesize.png)",
    );
    create_archive(&source_db, &source_images, &archive).unwrap();

    let (target_root, target_db, target_images, _) = fixture("", false);
    write_image(&target_images, "clash.png", b"local version, different");
    write_image(&target_images, "same.png", b"identical");
    write_image(&target_images, "samesize.png", b"BBBB");
    sync_registry_like_startup(&target_db, &target_images);
    // Stale row of a vanished file: the image restored under this name corrects it (upsert).
    Connection::open(&target_db)
        .unwrap()
        .execute("INSERT INTO images (name, sha256, size, created_at) VALUES ('free.png', 'stale', 99, '2020-01-01T00:00:00.000Z')", [])
        .unwrap();
    let local_clash = registry(&target_db)
        .into_iter()
        .find(|row| row.0 == "clash.png")
        .unwrap();

    restore_into(
        &target_root,
        &target_db,
        &target_images,
        &archive,
        "safety.zip",
    );

    let files = image_files(&target_images);
    assert_eq!(
        files.len(),
        6,
        "free + identical + 2 renamed collisions + 2 local files: {:?}",
        files.keys()
    );
    assert_eq!(
        files["clash.png"], b"local version, different",
        "the local file is never overwritten"
    );
    assert_eq!(files["samesize.png"], b"BBBB");
    assert_eq!(files["same.png"], b"identical");
    assert_eq!(files["free.png"], b"free");
    let renamed = |content: &[u8]| -> String {
        let found: Vec<_> = files
            .iter()
            .filter(|(name, bytes)| {
                bytes.as_slice() == content
                    && !name.starts_with("clash")
                    && !name.starts_with("samesize")
            })
            .map(|(name, _)| name.clone())
            .collect();
        assert_eq!(found.len(), 1, "a single copy of {content:?}");
        found[0].clone()
    };
    let clash = renamed(b"archive version");
    let samesize = renamed(b"AAAA");
    assert!(
        clash.ends_with(".png") && uuid::Uuid::parse_str(clash.trim_end_matches(".png")).is_ok(),
        "{clash}"
    );

    let notes: BTreeMap<_, _> = note_rows(&target_db)
        .into_iter()
        .map(|(id, content, ..)| (id, content))
        .collect();
    assert_eq!(
        notes["n"],
        format!("![free](humcase-image://free.png) ![conflict](humcase-image://{clash})")
    );
    assert_eq!(
        notes["n2"], "![same](humcase-image://same.png)",
        "identical image: original name"
    );
    assert_eq!(
        notes["n3"],
        format!("![c](humcase-image://{clash}) ![s](humcase-image://{samesize})"),
        "each reference to a renamed image follows its file"
    );

    let rows: BTreeMap<_, _> = registry(&target_db)
        .into_iter()
        .map(|(name, sha, size, _)| (name, (sha, size)))
        .collect();
    assert_eq!(rows[&clash], (sha_hex(b"archive version"), 15));
    assert_eq!(rows[&samesize], (sha_hex(b"AAAA"), 4));
    assert_eq!(rows["free.png"], (sha_hex(b"free"), 4));
    assert_eq!(rows["same.png"], (sha_hex(b"identical"), 9));
    let clash_now = registry(&target_db)
        .into_iter()
        .find(|row| row.0 == "clash.png")
        .unwrap();
    assert_eq!(
        clash_now, local_clash,
        "the local file's row is not overwritten by the archive image"
    );
    assert_eq!(rows["samesize.png"].0, sha_hex(b"BBBB"));
}

#[test]
fn failure_before_commit_keeps_reused_image_removes_created_ones_and_leaves_registry_untouched() {
    let (_source_root, source_db, source_images, archive) = fixture(
        "![s](humcase-image://same.png) ![f](humcase-image://free.png)",
        false,
    );
    write_image(&source_images, "same.png", b"identical");
    write_image(&source_images, "free.png", b"free");
    create_archive(&source_db, &source_images, &archive).unwrap();
    let hash = inspect_archive(&archive).unwrap().archive_sha256;

    let (target_root, target_db, target_images, _) = fixture("old", false);
    write_image(&target_images, "same.png", b"identical");
    sync_registry_like_startup(&target_db, &target_images);
    let registry_before = registry(&target_db);
    let notes_before = note_rows(&target_db);

    let error = super::restore::restore_with_failure(
        &target_db,
        &target_images,
        &archive,
        &target_root.path().join("safety.zip"),
        &hash,
        true,
        false,
    )
    .unwrap_err();
    assert!(
        matches!(error, RestoreFailure::BeforeCommit(_)),
        "{error:?}"
    );
    assert_eq!(
        image_files(&target_images),
        BTreeMap::from([("same.png".to_string(), b"identical".to_vec())]),
        "the image created by the restore is removed, the one that already existed is kept"
    );
    assert_eq!(
        registry(&target_db),
        registry_before,
        "the registry is cancelled with the transaction"
    );
    assert_eq!(note_rows(&target_db), notes_before);
}

#[test]
fn commit_failure_leaves_registry_with_the_transaction_and_startup_repairs_it() {
    let (_source_root, source_db, source_images, archive) =
        fixture("![f](humcase-image://free.png)", false);
    write_image(&source_images, "free.png", b"free");
    create_archive(&source_db, &source_images, &archive).unwrap();
    let hash = inspect_archive(&archive).unwrap().archive_sha256;
    let (target_root, target_db, target_images, _) = fixture("old", false);

    let error = super::restore::restore_with_failure(
        &target_db,
        &target_images,
        &archive,
        &target_root.path().join("safety.zip"),
        &hash,
        false,
        true,
    )
    .unwrap_err();
    assert!(
        matches!(error, RestoreFailure::CommitAmbiguous(_)),
        "{error:?}"
    );
    // The registry follows the database: COMMIT not published, no row; the image is kept.
    assert_eq!(registry(&target_db), vec![]);
    assert!(target_images.join("free.png").exists());
    // The next startup finds it again.
    sync_registry_like_startup(&target_db, &target_images);
    assert_eq!(registry(&target_db)[0].0, "free.png");
}

/// Rewrites a current archive as if it dated from schema `version`: `downgrade` modifies the
/// extracted copy of the database, then the size, fingerprint and manifest version are redone.
fn archive_as_older_schema(
    archive: &Path,
    output: &Path,
    version: i32,
    downgrade: impl Fn(&Connection),
) {
    let mut zip = ZipArchive::new(File::open(archive).unwrap()).unwrap();
    let work = tempfile::tempdir().unwrap();
    let db_copy = work.path().join("humcase.db");
    std::io::copy(
        &mut zip.by_name("humcase.db").unwrap(),
        &mut File::create(&db_copy).unwrap(),
    )
    .unwrap();
    downgrade(&Connection::open(&db_copy).unwrap());
    let mut files: Vec<(String, Vec<u8>)> =
        vec![("humcase.db".to_string(), fs::read(&db_copy).unwrap())];
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).unwrap();
        if entry.name().unwrap().starts_with("images/") {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            files.push((entry.name().unwrap().to_string(), bytes));
        }
    }
    let manifest = Manifest {
        format_version: FORMAT_VERSION,
        created_at: "2026-09-24T12:00:00.000Z".to_string(),
        database_schema_version: version,
        entries: files
            .iter()
            .map(|(path, bytes)| Entry {
                path: path.clone(),
                size: bytes.len() as u64,
                sha256: sha_hex(bytes),
            })
            .collect(),
    };
    let mut writer = ZipWriter::new(File::create(output).unwrap());
    for (path, bytes) in &files {
        writer.start_file(path, zip_options()).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.start_file("manifest.json", zip_options()).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer.finish().unwrap();
}

#[test]
fn v18_archive_with_images_restores_and_rebuilds_the_image_registry() {
    let (source_root, source_db, source_images, current) =
        fixture("![a](humcase-image://a.png)", false);
    write_image(&source_images, "a.png", b"AAA");
    create_archive(&source_db, &source_images, &current).unwrap();
    // A v18 archive has no `images` table: the real version 18 is the one before the migration.
    let old = source_root.path().join("v18.humcase.zip");
    archive_as_older_schema(&current, &old, 18, |conn| {
        drop_v20_and_v21(conn);
        conn.execute_batch("DROP TABLE images; PRAGMA user_version = 18;")
            .unwrap();
    });
    let preview = inspect_archive(&old).unwrap();
    assert_eq!(
        (preview.database_schema_version, preview.image_count),
        (18, 1)
    );

    let (target_root, target_db, target_images, _) = fixture("old", false);
    restore_into(&target_root, &target_db, &target_images, &old, "safety.zip");

    assert_eq!(
        check_db(&Connection::open(&target_db).unwrap()).unwrap().0,
        DB_SCHEMA_VERSION
    );
    assert_eq!(
        image_files(&target_images),
        BTreeMap::from([("a.png".to_string(), b"AAA".to_vec())])
    );
    assert_eq!(note_rows(&target_db)[0].1, "![a](humcase-image://a.png)");
    let rows = registry(&target_db);
    assert_eq!(
        (rows[0].0.as_str(), rows[0].1.as_str(), rows[0].2),
        ("a.png", sha_hex(b"AAA").as_str(), 3)
    );
    assert_eq!(rows.len(), 1);
}

#[test]
fn archive_registry_rows_are_ignored_and_rebuilt_from_the_archive_images() {
    // A v19 archive embeds its author's registry: the restore does not read it, it
    // registers the images actually present in the archive with the manifest's fingerprint.
    let (_source_root, source_db, source_images, archive) =
        fixture("![a](humcase-image://a.png)", false);
    write_image(&source_images, "a.png", b"AAA");
    Connection::open(&source_db)
        .unwrap()
        .execute(
            "INSERT INTO images (name, sha256, size, created_at) VALUES ('ghost.png', 'deadbeef', 9, '2020-01-01T00:00:00.000Z'), ('a.png', 'liar', 1, '2020-01-01T00:00:00.000Z')",
            [],
        )
        .unwrap();
    create_archive(&source_db, &source_images, &archive).unwrap();
    let (target_root, target_db, target_images, _) = fixture("old", false);

    restore_into(
        &target_root,
        &target_db,
        &target_images,
        &archive,
        "safety.zip",
    );

    let rows = registry(&target_db);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(
        (rows[0].0.as_str(), rows[0].1.as_str(), rows[0].2),
        ("a.png", sha_hex(b"AAA").as_str(), 3)
    );
}

#[test]
fn only_safe_original_names_are_kept() {
    for kept in [
        "image.png",
        "0a1b.webp",
        "COM0.png",
        "COM10.png",
        "COMX",
        "nullable.png",
        "LPT.png",
        "a.b.c",
    ] {
        assert!(super::restore::name_can_be_kept(kept), "{kept}");
    }
    for renamed in [
        "NUL",
        "nul.png",
        "CON.txt",
        "Prn.a.b",
        "AUX",
        "com1.png",
        "LPT9.bin",
        "trailing.",
    ] {
        assert!(!super::restore::name_can_be_kept(renamed), "{renamed}");
    }

    // End to end: a Windows reserved name is renamed and its reference follows.
    let (_source_root, source_db, source_images, archive) =
        fixture("![p](humcase-image://NUL.png)", false);
    write_image(&source_images, "NUL.png", b"periph");
    // On Windows the source file cannot carry this name: the test only applies where it exists.
    if !source_images.join("NUL.png").is_file() {
        return;
    }
    create_archive(&source_db, &source_images, &archive).unwrap();
    let (target_root, target_db, target_images, _) = fixture("old", false);
    restore_into(
        &target_root,
        &target_db,
        &target_images,
        &archive,
        "safety.zip",
    );
    let files = image_files(&target_images);
    assert!(!files.contains_key("NUL.png"), "{:?}", files.keys());
    let (name, bytes) = files.iter().next().unwrap();
    assert_eq!(bytes.as_slice(), b"periph");
    assert_eq!(
        note_rows(&target_db)[0].1,
        format!("![p](humcase-image://{name})")
    );
}

// --- Raw dictation in fragments (v20, lot S6) ---

/// Fragments of the whole database: (id, note, text, date), in reading order.
fn fragment_rows(db: &Path) -> Vec<(String, String, String, String)> {
    let conn = Connection::open(db).unwrap();
    let mut statement = conn
        .prepare("SELECT id, note_id, text, created_at FROM dictation_fragments ORDER BY note_id, created_at, id")
        .unwrap();
    let rows = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    rows
}

/// (id, raw, revision, updated_at) of each note.
fn raw_rows(db: &Path) -> Vec<(String, Option<String>, i64, String)> {
    let conn = Connection::open(db).unwrap();
    let mut statement = conn
        .prepare("SELECT id, raw_content, revision, updated_at FROM notes ORDER BY id")
        .unwrap();
    let rows = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    rows
}

/// Raw text and date as in the archive; the revision never below the archive's (it moves past
/// the live one when the live note differed, invariant 9).
fn assert_restored_raw(target: &Path, source: &Path) {
    let (target, source) = (raw_rows(target), raw_rows(source));
    let strip = |rows: &[(String, Option<String>, i64, String)]| -> Vec<_> {
        rows.iter()
            .map(|(id, raw, _, updated)| (id.clone(), raw.clone(), updated.clone()))
            .collect()
    };
    assert_eq!(
        strip(&target),
        strip(&source),
        "raw text and date unchanged"
    );
    for (restored, archived) in target.iter().zip(&source) {
        assert!(
            restored.2 >= archived.2,
            "revision of {} moved backwards",
            restored.0
        );
    }
}

fn assert_fragments_recompose_raw(db: &Path) {
    let conn = Connection::open(db).unwrap();
    assert_eq!(
        crate::db::queries::notes_with_inconsistent_fragments(&conn).unwrap(),
        Vec::<String>::new()
    );
}

/// Representative source: dictation in several appends (including an image), note without dictation, trashed
/// note with dictation.
fn fixture_with_dictation() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let (root, db, images, archive) = fixture("![a](humcase-image://a.png) content", false);
    write_image(&images, "a.png", b"AAA");
    let conn = Connection::open(&db).unwrap();
    for text in [
        "first draft",
        "see ![a](humcase-image://a.png) here",
        "end\non two lines",
    ] {
        crate::db::queries::append_raw_content(&conn, "n", text).unwrap();
    }
    conn.execute_batch(
        "INSERT INTO notes (id, project_id, title, content, note_type, status) VALUES ('mute', 'p', 'No dictation', '', 'note', 'open');
         INSERT INTO notes (id, project_id, title, content, note_type, status) VALUES ('bin', 'p', 'Trashed', '', 'idea', 'open');",
    ).unwrap();
    crate::db::queries::append_raw_content(&conn, "bin", "dictation of a trashed note").unwrap();
    crate::db::queries::trash_note(&conn, "bin").unwrap();
    drop(conn);
    (root, db, images, archive)
}

/// Target that already has its own fragments: the restore replaces them, like the notes.
fn target_with_dictation() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let (root, db, images, output) = fixture("old text", false);
    crate::db::queries::append_raw_content(&Connection::open(&db).unwrap(), "n", "local dictation")
        .unwrap();
    (root, db, images, output)
}

/// v20 archive: fragments are restored **identically** (identifiers, texts, dates),
/// with the notes, in the same transaction; restoring twice changes nothing; the dictation
/// then continues by appending, in order.
#[test]
fn v20_archive_restores_dictation_fragments_identically() {
    let (_source_root, source_db, source_images, archive) = fixture_with_dictation();
    let source_fragments = fragment_rows(&source_db);
    assert_eq!(source_fragments.len(), 4);
    create_archive(&source_db, &source_images, &archive).unwrap();
    assert_eq!(
        fragment_rows(&source_db),
        source_fragments,
        "archiving does not touch the live database"
    );

    let (target_root, target_db, target_images, _) = target_with_dictation();
    restore_into(
        &target_root,
        &target_db,
        &target_images,
        &archive,
        "safety-1.zip",
    );

    assert_eq!(
        fragment_rows(&target_db),
        source_fragments,
        "fragments restored identically"
    );
    assert_restored_raw(&target_db, &source_db);
    assert_fragments_recompose_raw(&target_db);
    let before = (fragment_rows(&target_db), raw_rows(&target_db));
    restore_into(
        &target_root,
        &target_db,
        &target_images,
        &archive,
        "safety-2.zip",
    );
    assert_eq!(
        (fragment_rows(&target_db), raw_rows(&target_db)),
        before,
        "restoring twice changes nothing"
    );

    // The restored database stays append-only and continues after the last restored fragment.
    let conn = Connection::open(&target_db).unwrap();
    assert!(conn
        .execute("UPDATE dictation_fragments SET text = 'x'", [])
        .is_err());
    assert!(conn
        .execute("DELETE FROM dictation_fragments WHERE note_id = 'n'", [])
        .is_err());
    crate::db::queries::append_raw_content(&conn, "n", "after restore").unwrap();
    let texts: Vec<String> = crate::db::queries::dictation_fragments(&conn, "n")
        .unwrap()
        .into_iter()
        .map(|f| f.1)
        .collect();
    assert_eq!(texts.last().map(String::as_str), Some("after restore"));
    assert_eq!(texts.len(), 4);
    drop(conn);
    assert_fragments_recompose_raw(&target_db);
}

/// v19 archive (before fragments): the working copy is migrated, each note with non-empty raw text
/// receives an identical text fragment, dated with its `updated_at`; the target's fragments are
/// replaced.
#[test]
fn v19_archive_restores_with_one_fragment_per_raw_content() {
    let (source_root, source_db, source_images, current) = fixture_with_dictation();
    create_archive(&source_db, &source_images, &current).unwrap();
    let old = source_root.path().join("v19.humcase.zip");
    archive_as_older_schema(&current, &old, 19, |conn| {
        drop_v20_and_v21(conn);
        conn.execute_batch("PRAGMA user_version = 19;").unwrap();
    });
    let preview = inspect_archive(&old).unwrap();
    assert_eq!(preview.database_schema_version, 19);

    let (target_root, target_db, target_images, _) = target_with_dictation();
    restore_into(&target_root, &target_db, &target_images, &old, "safety.zip");

    assert_eq!(
        check_db(&Connection::open(&target_db).unwrap()).unwrap().0,
        DB_SCHEMA_VERSION
    );
    assert_restored_raw(&target_db, &source_db);
    let fragments: Vec<(String, String, String)> = fragment_rows(&target_db)
        .into_iter()
        .map(|(_, note, text, date)| (note, text, date))
        .collect();
    let expected_rows: Vec<(String, String, String)> = raw_rows(&source_db)
        .into_iter()
        .filter_map(|(id, raw, _, updated_at)| {
            raw.filter(|b| !b.is_empty()).map(|b| (id, b, updated_at))
        })
        .collect();
    assert_eq!(
        expected_rows.len(),
        2,
        "the dictation in three appends and the trashed note"
    );
    assert_eq!(
        fragments, expected_rows,
        "one fragment per non-empty raw text, identical text (image included)"
    );
    assert!(
        fragments[1].1.contains("humcase-image://a.png"),
        "{fragments:?}"
    );
    assert_fragments_recompose_raw(&target_db);
}

/// An archive whose fragments do not recompose the raw dictation (forged archive) is refused
/// before COMMIT, without changing anything in the live database.
#[test]
fn archive_with_fragments_that_do_not_recompose_raw_content_is_refused() {
    let (source_root, source_db, source_images, current) = fixture_with_dictation();
    create_archive(&source_db, &source_images, &current).unwrap();
    let forged = source_root.path().join("forged.humcase.zip");
    archive_as_older_schema(&current, &forged, DB_SCHEMA_VERSION, |conn| {
        conn.execute_batch(
            "UPDATE dictation_fragments SET text = 'something else' WHERE text = 'first draft';",
        )
        .unwrap();
    });
    let hash = inspect_archive(&forged).unwrap().archive_sha256;

    let (target_root, target_db, target_images, _) = target_with_dictation();
    let before = (fragment_rows(&target_db), raw_rows(&target_db));
    let error = crate::i18n::with_language("fr", || {
        restore_archive_into(
            &target_db,
            &target_images,
            &forged,
            &target_root.path().join("safety.zip"),
            &hash,
        )
    })
    .unwrap_err();
    assert!(
        matches!(error, RestoreFailure::BeforeCommit(_)),
        "{error:?}"
    );
    assert!(
        error.message().contains("Dictée brute incohérente"),
        "{}",
        error.message()
    );
    assert_eq!((fragment_rows(&target_db), raw_rows(&target_db)), before);
}

/// A fragment that cites an image missing from the archive is refused at inspection, like the raw text.
#[test]
fn fragment_citing_an_image_missing_from_the_archive_is_rejected() {
    let (source_root, source_db, source_images, current) = fixture_with_dictation();
    create_archive(&source_db, &source_images, &current).unwrap();
    let forged = source_root.path().join("forged.humcase.zip");
    archive_as_older_schema(&current, &forged, DB_SCHEMA_VERSION, |conn| {
        conn.execute_batch(
            "INSERT INTO dictation_fragments (id, note_id, text, created_at)
             VALUES ('f', 'mute', '![x](humcase-archive://images/missing-file.png)', '2026-01-01T00:00:00.000Z');",
        )
        .unwrap();
    });
    let error = inspect_archive(&forged).unwrap_err();
    assert!(error.contains("missing-file.png"), "{error}");
}

/// Failure injected before COMMIT then failure at COMMIT: the live database's fragments stay the previous
/// ones (cancelled with the transaction), and the classification does not change.
#[test]
fn injected_failures_keep_live_dictation_fragments() {
    let (_source_root, source_db, source_images, archive) = fixture_with_dictation();
    create_archive(&source_db, &source_images, &archive).unwrap();
    let hash = inspect_archive(&archive).unwrap().archive_sha256;

    for (before_commit, on_commit) in [(true, false), (false, true)] {
        let (target_root, target_db, target_images, _) = target_with_dictation();
        let before = (fragment_rows(&target_db), raw_rows(&target_db));
        assert_eq!(before.0.len(), 1);
        let error = super::restore::restore_with_failure(
            &target_db,
            &target_images,
            &archive,
            &target_root.path().join("safety.zip"),
            &hash,
            before_commit,
            on_commit,
        )
        .unwrap_err();
        if before_commit {
            assert!(
                matches!(error, RestoreFailure::BeforeCommit(_)),
                "{error:?}"
            );
        } else {
            assert!(
                matches!(error, RestoreFailure::CommitAmbiguous(_)),
                "{error:?}"
            );
        }
        assert_eq!(
            (fragment_rows(&target_db), raw_rows(&target_db)),
            before,
            "{error:?}"
        );
        assert_fragments_recompose_raw(&target_db);
    }
}

/// A database whose raw text no longer matches its fragments (SQL write outside the application) is not
/// archived: the error appears at backup time, not during an impossible restore.
#[test]
fn export_refuses_a_database_whose_fragments_do_not_recompose_raw_content() {
    let (_root, db, images, archive) = fixture_with_dictation();
    Connection::open(&db)
        .unwrap()
        .execute(
            "UPDATE notes SET raw_content = 'replaced outside the application' WHERE id = 'n'",
            [],
        )
        .unwrap();
    let error = create_archive(&db, &images, &archive).unwrap_err();
    assert!(
        error.contains("Dictée brute incohérente (case n)"),
        "{error}"
    );
    assert!(!archive.exists(), "no archive published");
}

// --- General deletion traces (v21, lot S2) ---

/// Deletion traces of the whole database: (entity, identifier, date), sorted.
fn deletion_rows(db: &Path) -> Vec<(String, String, String)> {
    let conn = Connection::open(db).unwrap();
    let mut statement = conn
        .prepare("SELECT entity, entity_id, deleted_at FROM deletions ORDER BY entity, entity_id")
        .unwrap();
    let rows = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    rows
}

/// Composite key of a trace (JSON array, primary key order).
fn trace_key(a: &str, b: &str) -> String {
    serde_json::to_string(&[a, b]).unwrap()
}

/// Identifiers of a table, sorted.
fn ids_of(db: &Path, table: &str) -> Vec<String> {
    let conn = Connection::open(db).unwrap();
    let mut statement = conn
        .prepare(&format!("SELECT id FROM {table} ORDER BY id"))
        .unwrap();
    let rows = statement
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    rows
}

fn link_rows(db: &Path) -> Vec<(String, String)> {
    let conn = Connection::open(db).unwrap();
    let mut statement = conn
        .prepare("SELECT source_note_id, task_note_id FROM note_task_links ORDER BY 1, 2")
        .unwrap();
    let rows = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    rows
}

fn attachment_rows(db: &Path) -> Vec<(String, String)> {
    let conn = Connection::open(db).unwrap();
    let mut statement = conn
        .prepare("SELECT project_id, activity_id FROM project_activities ORDER BY 1, 2")
        .unwrap();
    let rows = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    rows
}

/// Representative source: projects, activities, attachments, linked notes. With `with_traces`, it
/// deleted a link, an activity (hence an attachment by cascade) and a project: the four
/// traced entities each have a trace.
fn source_with_deletions(with_traces: bool) -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let (root, db, images, archive) = fixture("source content", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         UPDATE projects SET space = 'work' WHERE id = 'p';
         INSERT INTO projects (id, name, space) VALUES ('q', 'Other', 'work'), ('gone', 'Deleted', 'work');
         INSERT INTO activities (id, name, space) VALUES ('a1', 'One', 'work'), ('a2', 'Two', 'work'), ('gone-act', 'Lost', 'work');
         INSERT INTO project_activities (project_id, activity_id)
           VALUES ('p', 'a1'), ('p', 'a2'), ('q', 'a2'), ('q', 'gone-act');
         INSERT INTO notes (id, project_id, title, content, note_type, status)
           VALUES ('t1', 'p', 'Task 1', '', 'todo', 'open'), ('t2', 'p', 'Task 2', '', 'todo', 'open');
         INSERT INTO note_task_links (source_note_id, task_note_id) VALUES ('n', 't1'), ('n', 't2');",
    )
    .unwrap();
    if with_traces {
        conn.execute_batch(
            "DELETE FROM note_task_links WHERE task_note_id = 't2';
             DELETE FROM activities WHERE id = 'gone-act';
             DELETE FROM projects WHERE id = 'gone';",
        )
        .unwrap();
    }
    (root, db, images, archive)
}

/// Target different from the source, with its own data and, if requested, its own traces: the
/// restore erases all its rows (projects, activities, links, attachments) before
/// copying the archive.
fn target_with_deletions(with_traces: bool) -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let (root, db, images, output) = fixture("old content", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         UPDATE projects SET space = 'work' WHERE id = 'p';
         INSERT INTO projects (id, name, space) VALUES ('old-q', 'Old project', 'work'), ('old-gone', 'Old deleted', 'work');
         INSERT INTO activities (id, name, space) VALUES ('old-a', 'Old', 'work'), ('old-gone-act', 'Old lost', 'work');
         INSERT INTO project_activities (project_id, activity_id)
           VALUES ('p', 'old-a'), ('old-q', 'old-a'), ('old-q', 'old-gone-act');
         INSERT INTO notes (id, project_id, title, content, note_type, status)
           VALUES ('old-t', 'p', 'Old task', '', 'todo', 'open');
         INSERT INTO note_task_links (source_note_id, task_note_id) VALUES ('n', 'old-t');",
    )
    .unwrap();
    if with_traces {
        conn.execute_batch(
            "DELETE FROM activities WHERE id = 'old-gone-act';
             DELETE FROM projects WHERE id = 'old-gone';",
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO notes (id, project_id, title, content, note_type, status)
               VALUES ('old-t2', 'p', 'Other task', '', 'todo', 'open');
             INSERT INTO note_task_links (source_note_id, task_note_id) VALUES ('n', 'old-t2');
             DELETE FROM note_task_links WHERE task_note_id = 'old-t2';",
        )
        .unwrap();
    }
    (root, db, images, output)
}

/// Content of the archive's database (extracted `humcase.db`), to read what it embeds.
fn archived_db(archive: &Path) -> (tempfile::TempDir, PathBuf) {
    let work = tempfile::tempdir().unwrap();
    let path = work.path().join("humcase.db");
    let mut zip = ZipArchive::new(File::open(archive).unwrap()).unwrap();
    std::io::copy(
        &mut zip.by_name("humcase.db").unwrap(),
        &mut File::create(&path).unwrap(),
    )
    .unwrap();
    (work, path)
}

/// The `deletions` table travels in the archive; restoring keeps **exactly** the archive's
/// traces: those of the previous target disappear, the restore's technical erasures
/// (projects, activities, links, attachments of the target) leave none, and
/// restoring twice changes nothing.
#[test]
fn archive_carries_deletion_traces_and_restore_keeps_exactly_those() {
    let (_source_root, source_db, source_images, archive) = source_with_deletions(true);
    let source_traces = deletion_rows(&source_db);
    let mut expected_traces: Vec<(String, String)> = vec![
        ("activity".into(), "gone-act".into()),
        ("note_task_link".into(), trace_key("n", "t2")),
        ("project".into(), "gone".into()),
        ("project_activity".into(), trace_key("q", "gone-act")),
    ];
    expected_traces.sort();
    assert_eq!(
        source_traces
            .iter()
            .map(|(e, i, _)| (e.clone(), i.clone()))
            .collect::<Vec<_>>(),
        expected_traces,
        "the four traced entities each have a trace"
    );

    create_archive(&source_db, &source_images, &archive).unwrap();
    assert_eq!(
        deletion_rows(&source_db),
        source_traces,
        "archiving does not touch the live database"
    );
    let (_work, embedded) = archived_db(&archive);
    assert_eq!(
        deletion_rows(&embedded),
        source_traces,
        "the `deletions` table travels in the archive, dates included"
    );

    let (target_root, target_db, target_images, _) = target_with_deletions(true);
    let target_traces = deletion_rows(&target_db);
    assert!(
        target_traces.len() >= 3,
        "the target has its own traces: {target_traces:?}"
    );
    restore_into(
        &target_root,
        &target_db,
        &target_images,
        &archive,
        "safety-1.zip",
    );

    assert_eq!(
        deletion_rows(&target_db),
        source_traces,
        "exactly the archive's traces"
    );
    assert_eq!(
        ids_of(&target_db, "projects"),
        ids_of(&source_db, "projects")
    );
    assert_eq!(
        ids_of(&target_db, "activities"),
        ids_of(&source_db, "activities")
    );
    assert_eq!(attachment_rows(&target_db), attachment_rows(&source_db));
    assert_eq!(link_rows(&target_db), link_rows(&source_db));
    assert_eq!(
        check_db(&Connection::open(&target_db).unwrap()).unwrap().0,
        DB_SCHEMA_VERSION
    );

    restore_into(
        &target_root,
        &target_db,
        &target_images,
        &archive,
        "safety-2.zip",
    );
    assert_eq!(
        deletion_rows(&target_db),
        source_traces,
        "restoring twice changes nothing"
    );
    // The safety archive, made before the restore, contains the previous target's traces.
    let (_safety_work, safety_db) = archived_db(&target_root.path().join("safety-1.zip"));
    assert_eq!(deletion_rows(&safety_db), target_traces);
}

/// The restore's technical erasures leave **no** trace: an archive without
/// traces restored onto a rich target (with or without its own traces) gives an empty table, not a
/// trace per replaced project, activity, link or attachment.
#[test]
fn restore_adds_no_trace_for_its_own_erasures() {
    let (_source_root, source_db, source_images, archive) = source_with_deletions(false);
    assert!(deletion_rows(&source_db).is_empty());
    create_archive(&source_db, &source_images, &archive).unwrap();

    for with_traces in [false, true] {
        let (target_root, target_db, target_images, _) = target_with_deletions(with_traces);
        assert_eq!(deletion_rows(&target_db).is_empty(), !with_traces);
        assert!(
            ids_of(&target_db, "projects").contains(&"old-q".to_string()),
            "the target has rows to replace"
        );
        restore_into(
            &target_root,
            &target_db,
            &target_images,
            &archive,
            "safety.zip",
        );

        assert!(
            deletion_rows(&target_db).is_empty(),
            "target with traces = {with_traces}: {:?}",
            deletion_rows(&target_db)
        );
        assert_eq!(
            ids_of(&target_db, "projects"),
            ids_of(&source_db, "projects"),
            "the state is indeed the archive's"
        );
        assert_eq!(attachment_rows(&target_db), attachment_rows(&source_db));
        assert_eq!(link_rows(&target_db), link_rows(&source_db));
    }
}

/// A v20 archive (before traces) is restorable: the migrated working copy receives an empty
/// table, so the target has no trace left, and its data is replaced as usual.
#[test]
fn v20_archive_restores_with_an_empty_trace_table() {
    let (source_root, source_db, source_images, current) = source_with_deletions(true);
    create_archive(&source_db, &source_images, &current).unwrap();
    let old = source_root.path().join("v20.humcase.zip");
    archive_as_older_schema(&current, &old, 20, |conn| {
        conn.execute_batch(DROP_V21).unwrap();
        conn.execute_batch("PRAGMA user_version = 20;").unwrap();
    });
    let preview = inspect_archive(&old).unwrap();
    assert_eq!(preview.database_schema_version, 20);
    let (_work, embedded) = archived_db(&old);
    assert!(
        Connection::open(&embedded)
            .unwrap()
            .prepare("SELECT entity FROM deletions")
            .is_err(),
        "the v20 archive does not have the table"
    );

    let (target_root, target_db, target_images, _) = target_with_deletions(true);
    assert!(!deletion_rows(&target_db).is_empty());
    restore_into(&target_root, &target_db, &target_images, &old, "safety.zip");

    assert_eq!(
        check_db(&Connection::open(&target_db).unwrap()).unwrap().0,
        DB_SCHEMA_VERSION
    );
    assert!(
        deletion_rows(&target_db).is_empty(),
        "a v20 archive receives an empty table from the migration"
    );
    assert_eq!(
        ids_of(&target_db, "projects"),
        ids_of(&source_db, "projects")
    );
    assert_eq!(attachment_rows(&target_db), attachment_rows(&source_db));
    assert_eq!(link_rows(&target_db), link_rows(&source_db));
}

/// Failure injected before COMMIT then failure at COMMIT, target and archive **with** traces: the live
/// database's traces stay the previous ones (cancelled with the transaction), like its data, and
/// the classification does not change.
#[test]
fn injected_failures_keep_live_deletion_traces() {
    let (_source_root, source_db, source_images, archive) = source_with_deletions(true);
    create_archive(&source_db, &source_images, &archive).unwrap();
    let hash = inspect_archive(&archive).unwrap().archive_sha256;

    for (before_commit, on_commit) in [(true, false), (false, true)] {
        let (target_root, target_db, target_images, _) = target_with_deletions(true);
        let traces_before = deletion_rows(&target_db);
        let data_before = (
            ids_of(&target_db, "projects"),
            attachment_rows(&target_db),
            link_rows(&target_db),
        );
        assert_ne!(traces_before, deletion_rows(&source_db));
        let error = super::restore::restore_with_failure(
            &target_db,
            &target_images,
            &archive,
            &target_root.path().join("safety.zip"),
            &hash,
            before_commit,
            on_commit,
        )
        .unwrap_err();
        if before_commit {
            assert!(
                matches!(error, RestoreFailure::BeforeCommit(_)),
                "{error:?}"
            );
        } else {
            assert!(
                matches!(error, RestoreFailure::CommitAmbiguous(_)),
                "{error:?}"
            );
        }
        assert_eq!(deletion_rows(&target_db), traces_before, "{error:?}");
        assert_eq!(
            (
                ids_of(&target_db, "projects"),
                attachment_rows(&target_db),
                link_rows(&target_db)
            ),
            data_before
        );
    }
}

/// An archive that claims to be v21 but lacks the `deletions` table is refused **before** COMMIT, without
/// changing anything in the live database (neither data nor traces).
#[test]
fn v21_archive_without_the_trace_table_is_refused_before_commit() {
    let (source_root, source_db, source_images, current) = source_with_deletions(true);
    create_archive(&source_db, &source_images, &current).unwrap();
    let forged = source_root.path().join("forged.humcase.zip");
    archive_as_older_schema(&current, &forged, DB_SCHEMA_VERSION, |conn| {
        conn.execute_batch(DROP_V21).unwrap();
        conn.execute_batch(&format!("PRAGMA user_version = {DB_SCHEMA_VERSION};"))
            .unwrap();
    });
    let hash = inspect_archive(&forged).unwrap().archive_sha256;

    let (target_root, target_db, target_images, _) = target_with_deletions(true);
    let traces_before = deletion_rows(&target_db);
    let data_before = (
        ids_of(&target_db, "projects"),
        attachment_rows(&target_db),
        link_rows(&target_db),
    );
    let error = restore_archive_into(
        &target_db,
        &target_images,
        &forged,
        &target_root.path().join("safety.zip"),
        &hash,
    )
    .unwrap_err();
    assert!(
        matches!(error, RestoreFailure::BeforeCommit(_)),
        "{error:?}"
    );
    assert_eq!(deletion_rows(&target_db), traces_before);
    assert_eq!(
        (
            ids_of(&target_db, "projects"),
            attachment_rows(&target_db),
            link_rows(&target_db)
        ),
        data_before
    );
}

/// Sub-cases (v22): the tree, the content kinds and the provenance survive an archive and its
/// restore, even when a parent comes after its sub-case in the rows. Fails if the restore stops
/// attaching the parents or copying the provenance.
#[test]
fn sub_cases_and_provenance_survive_archive_and_restore() {
    use crate::db::cases;
    use crate::models::note::ContentKind;
    let (_source_root, db, images, output) = fixture("Root case", false);
    {
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "INSERT INTO notes (id, project_id, title, content, note_type, status) VALUES
               ('early', 'p', 'Created first', '', 'note', 'open'),
               ('late', 'p', 'Created last', '', 'note', 'open');",
        )
        .unwrap();
        // `early` comes before its parent `late` in the rows.
        cases::move_case(&conn, "early", Some("late")).unwrap();
        cases::move_case(&conn, "late", Some("n")).unwrap();
        conn.execute_batch("UPDATE notes SET content_kind = 'image' WHERE id = 'early'")
            .unwrap();
        cases::add_provenance(
            &conn,
            "late",
            "source",
            Some("Interview"),
            None,
            None,
            Some("agent"),
        )
        .unwrap();
    }
    create_archive(&db, &images, &output).unwrap();
    let preview = inspect_archive(&output).unwrap();
    let (target_root, target_db, target_images, _) = fixture("Previous target", false);
    let safety = target_root.path().join("safety.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &output,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();

    let conn = Connection::open(&target_db).unwrap();
    let rows: Vec<(String, Option<String>, ContentKind)> = conn
        .prepare("SELECT id, parent_id, content_kind FROM notes ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        rows,
        vec![
            (
                "early".to_string(),
                Some("late".to_string()),
                ContentKind::Image
            ),
            ("late".to_string(), Some("n".to_string()), ContentKind::Text),
            ("n".to_string(), None, ContentKind::Text),
        ]
    );
    let provenance = cases::list_provenance(&conn, "late").unwrap();
    assert_eq!(provenance.len(), 1);
    assert_eq!(provenance[0].label.as_deref(), Some("Interview"));
    assert_eq!(provenance[0].agent.as_deref(), Some("agent"));
}

/// A v21 archive (before sub-cases) restores: every case comes back at top level, as text.
#[test]
fn v21_archive_restores_every_case_at_top_level() {
    let (_root, db, _images, output) = fixture("Before sub-cases", false);
    Connection::open(&db)
        .unwrap()
        .execute_batch(
            "DROP TRIGGER sub_case_same_project_insert;
             DROP TRIGGER sub_case_same_project_update;
             DROP TRIGGER sub_case_parent_purged;
             DROP TRIGGER sub_case_no_cycle;
             DROP TRIGGER case_provenance_no_update;
             DROP TRIGGER case_provenance_no_delete;
             DROP TABLE case_provenance;
             DROP INDEX idx_notes_parent;
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
    archive_db_for_test(&db, &output, 21);
    let preview = inspect_archive(&output).unwrap();
    let (target_root, target_db, target_images, _) = fixture("Previous target", false);
    let safety = target_root.path().join("safety.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &output,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();
    let conn = Connection::open(&target_db).unwrap();
    let (content, parent, kind): (String, Option<String>, String) = conn
        .query_row(
            "SELECT content, parent_id, content_kind FROM notes WHERE id = 'n'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (content.as_str(), parent, kind.as_str()),
        ("Before sub-cases", None, "text")
    );
}

#[test]
fn archive_with_sub_case_cycle_is_rejected() {
    let (_root, db, _images, output) = fixture("", false);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO notes (id, project_id, title, content, note_type, status, parent_id)
           VALUES ('a', 'p', 'A', '', 'note', 'open', 'n');
         DROP TRIGGER sub_case_no_cycle;
         UPDATE notes SET parent_id = 'a' WHERE id = 'n';",
    )
    .unwrap();
    drop(conn);
    archive_db_for_test(&db, &output, DB_SCHEMA_VERSION);
    assert!(inspect_archive(&output)
        .unwrap_err()
        .contains("une de ses propres sous-cases"));
}

#[test]
fn archive_with_sub_case_in_another_project_or_unknown_kind_is_rejected() {
    for corruption in [
        "INSERT INTO projects (id, name) VALUES ('q', 'Other');
         DROP TRIGGER sub_case_same_project_insert;
         INSERT INTO notes (id, project_id, title, content, note_type, status, parent_id)
           VALUES ('a', 'q', 'A', '', 'note', 'open', 'n');",
        "DROP TRIGGER sub_case_same_project_insert;
         INSERT INTO notes (id, project_id, title, content, note_type, status, parent_id)
           VALUES ('a', 'p', 'A', '', 'note', 'open', 'missing');",
        "UPDATE notes SET content_kind = 'video' WHERE id = 'n';",
    ] {
        let (_root, db, _images, output) = fixture("", false);
        Connection::open(&db)
            .unwrap()
            .execute_batch(corruption)
            .unwrap();
        archive_db_for_test(&db, &output, DB_SCHEMA_VERSION);
        assert!(
            inspect_archive(&output)
                .unwrap_err()
                .contains("case parente manque"),
            "{corruption}"
        );
    }
}

/// Archives made before 0.10.0 name their database `devnotes.db` and their images
/// `devnotes-archive://images/…`: they stay inspectable and restorable, images included. An
/// archive carrying both database names is refused.
#[test]
fn former_archive_names_still_restore() {
    let (_source_root, db, images, output) = fixture("Before", true);
    set_note_content(&db, "![view](humcase-image://image.png)");
    create_archive(&db, &images, &output).unwrap();
    // Rebuild the same archive under the former names.
    let mut zip = ZipArchive::new(File::open(&output).unwrap()).unwrap();
    let work = tempfile::tempdir().unwrap();
    let db_copy = work.path().join("copy.db");
    std::io::copy(
        &mut zip.by_name("humcase.db").unwrap(),
        &mut File::create(&db_copy).unwrap(),
    )
    .unwrap();
    let mut image = Vec::new();
    zip.by_name("images/image.png")
        .unwrap()
        .read_to_end(&mut image)
        .unwrap();
    drop(zip);
    Connection::open(&db_copy)
        .unwrap()
        .execute(
            "UPDATE notes SET content = replace(content, 'humcase-archive://', 'devnotes-archive://')",
            [],
        )
        .unwrap();
    let former = output.with_file_name("former.devnotes.zip");
    let write_archive = |path: &Path, db_names: &[&str]| {
        let db_bytes = fs::read(&db_copy).unwrap();
        let mut entries: Vec<Entry> = db_names
            .iter()
            .map(|name| Entry {
                path: name.to_string(),
                size: db_bytes.len() as u64,
                sha256: crate::images::lower_hex(&Sha256::digest(&db_bytes)),
            })
            .collect();
        entries.push(Entry {
            path: "images/image.png".to_string(),
            size: image.len() as u64,
            sha256: crate::images::lower_hex(&Sha256::digest(&image)),
        });
        let manifest = Manifest {
            format_version: FORMAT_VERSION,
            created_at: "2026-09-24T12:00:00.000Z".to_string(),
            database_schema_version: DB_SCHEMA_VERSION,
            entries,
        };
        let mut zip = ZipWriter::new(File::create(path).unwrap());
        for name in db_names {
            zip.start_file(*name, zip_options()).unwrap();
            zip.write_all(&db_bytes).unwrap();
        }
        zip.start_file("images/image.png", zip_options()).unwrap();
        zip.write_all(&image).unwrap();
        zip.start_file("manifest.json", zip_options()).unwrap();
        zip.write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        zip.finish().unwrap();
    };
    write_archive(&former, &["devnotes.db"]);
    let both = output.with_file_name("both.humcase.zip");
    write_archive(&both, &["devnotes.db", "humcase.db"]);
    assert!(inspect_archive(&both).is_err(), "two databases are refused");

    let preview = inspect_archive(&former).unwrap();
    assert_eq!(preview.image_count, 1);
    let (target_root, target_db, target_images, _) = fixture("Previous target", false);
    let safety = target_root.path().join("safety.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &former,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();
    let content: String = Connection::open(&target_db)
        .unwrap()
        .query_row("SELECT content FROM notes WHERE id = 'n'", [], |r| r.get(0))
        .unwrap();
    assert!(!content.contains("-archive://"), "{content}");
    assert!(content.contains("humcase-image://"), "{content}");
}

/// v24 — an archive carries the journal of who did what and the decisions on results: the
/// restore puts them back identically (the brief and the decision state read them), and the
/// restore itself is nobody's action (no event is added). Fails if the copy of `case_events`
/// or `case_reviews` disappears from the restore.
#[test]
fn restore_keeps_the_journal_and_decisions_without_adding_events() {
    use crate::db::journal::{self, Actor, Decision};
    let (source_root, source_db, source_images, _) = fixture("Initial target", false);
    {
        let conn = Connection::open(&source_db).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        journal::install(&conn, &Actor::Agent(Some("Codex".into()))).unwrap();
        conn.execute(
            "UPDATE notes SET status = 'resolved', resolution_comment = 'Done' WHERE id = 'n'",
            [],
        )
        .unwrap();
        journal::set_actor(&conn, &Actor::Human).unwrap();
        let revision: i64 = conn
            .query_row("SELECT revision FROM notes WHERE id = 'n'", [], |r| {
                r.get(0)
            })
            .unwrap();
        journal::review_case(
            &conn,
            "n",
            Decision::RequestRework,
            revision,
            Some("Add the cost"),
        )
        .unwrap();
    }
    let source = Connection::open(&source_db).unwrap();
    let events = journal::case_events(&source, "n").unwrap();
    let reviews = journal::case_reviews(&source, "n").unwrap();
    assert!(
        events.len() >= 3 && reviews.len() == 1,
        "representative journal"
    );
    drop(source);
    let archive = source_root.path().join("journal-v24.humcase.zip");
    create_archive(&source_db, &source_images, &archive).unwrap();
    let preview = inspect_archive(&archive).unwrap();

    let (target_root, target_db, target_images, _) = fixture("Target", false);
    let safety = target_root.path().join("safety.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();

    let target = Connection::open(&target_db).unwrap();
    assert_eq!(journal::case_events(&target, "n").unwrap(), events);
    assert_eq!(journal::case_reviews(&target, "n").unwrap(), reviews);
    let state = journal::review_state(&target, "n").unwrap();
    assert_eq!(
        (state.state.as_str(), state.rework_count),
        ("rework_requested", 1)
    );
}

/// v25 — the person's answers on natures travel with the archive: a nature they kept is not
/// proposed again after a restore. Fails if the copy of `nature_checks` disappears.
#[test]
fn restore_keeps_the_answers_on_natures() {
    use crate::models::note::NoteType;
    use crate::natures::{self, Answer};
    let (source_root, source_db, source_images, _) = fixture("Initial target", false);
    {
        let conn = Connection::open(&source_db).unwrap();
        natures::record_answer(&conn, "n", NoteType::Note, NoteType::Todo, Answer::Kept).unwrap();
    }
    let archive = source_root.path().join("natures-v25.humcase.zip");
    create_archive(&source_db, &source_images, &archive).unwrap();
    let preview = inspect_archive(&archive).unwrap();

    let (target_root, target_db, target_images, _) = fixture("Target", false);
    let safety = target_root.path().join("safety.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();

    let target = Connection::open(&target_db).unwrap();
    let answer: (String, String, String) = target
        .query_row(
            "SELECT note_type, proposed, answer FROM nature_checks WHERE note_id = 'n'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(answer, ("note".into(), "todo".into(), "kept".into()));
}

/// v26 — what Humcase proposed for a case and the person's answers travel with the archive.
/// Fails if the copy of `proposals` disappears.
#[test]
fn restore_keeps_the_proposals_and_their_answers() {
    let (source_root, source_db, source_images, _) = fixture("Initial target", false);
    {
        let conn = Connection::open(&source_db).unwrap();
        conn.execute_batch(
            "INSERT INTO proposals (id, note_id, kind, nature, title, passage, revision, position,
                                    created_at, answer, answered_at)
             VALUES ('p1', 'n', 'sub_case', 'todo', 'Call the plumber', 'call the plumber', 1, 0,
                     '2026-10-04T10:00:00Z', 'refused', '2026-10-04T10:01:00Z'),
                    ('p2', 'n', 'sub_case', 'question', 'Which day?', 'which day', 1, 1,
                     '2026-10-04T10:00:00Z', NULL, NULL);",
        )
        .unwrap();
    }
    let archive = source_root.path().join("proposals-v26.humcase.zip");
    create_archive(&source_db, &source_images, &archive).unwrap();
    let preview = inspect_archive(&archive).unwrap();

    let (target_root, target_db, target_images, _) = fixture("Target", false);
    let safety = target_root.path().join("safety.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();

    let target = Connection::open(&target_db).unwrap();
    let rows: Vec<(String, Option<String>)> = target
        .prepare("SELECT id, answer FROM proposals ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        rows,
        [("p1".into(), Some("refused".into())), ("p2".into(), None)]
    );
}

#[test]
fn restore_keeps_the_decisions_marked_as_replaced() {
    let (source_root, source_db, source_images, _) = fixture("Initial target", false);
    {
        let conn = Connection::open(&source_db).unwrap();
        conn.execute_batch(
            "INSERT INTO notes (id, project_id, title, content, note_type, status, created_at, updated_at)
             SELECT 'newer', project_id, 'Newer decision', 'Cloudflare.', 'decision', 'open',
                    created_at, updated_at FROM notes WHERE id = 'n';
             INSERT INTO case_supersessions (note_id, by_id, superseded_at)
             VALUES ('n', 'newer', '2026-10-04T10:00:00Z');",
        )
        .unwrap();
    }
    let archive = source_root.path().join("supersessions-v27.humcase.zip");
    create_archive(&source_db, &source_images, &archive).unwrap();
    let preview = inspect_archive(&archive).unwrap();

    let (target_root, target_db, target_images, _) = fixture("Target", false);
    let safety = target_root.path().join("safety.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();

    let target = Connection::open(&target_db).unwrap();
    let rows: Vec<(String, String)> = target
        .prepare("SELECT note_id, by_id FROM case_supersessions")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, [("n".into(), "newer".into())]);
}

/// v28 — which case is the work profile of a space travels with the archive. Fails if the copy
/// of `work_profiles` disappears (the restored profile would be an ordinary case, and a second
/// profile would be made beside it).
#[test]
fn restore_keeps_the_work_profiles() {
    let (source_root, source_db, source_images, _) = fixture("Initial target", false);
    {
        let conn = Connection::open(&source_db).unwrap();
        conn.execute_batch(
            "INSERT INTO work_profiles (space, note_id, created_at)
             VALUES ('work', 'n', '2026-10-04T10:00:00Z');",
        )
        .unwrap();
    }
    let archive = source_root.path().join("profiles-v28.humcase.zip");
    create_archive(&source_db, &source_images, &archive).unwrap();
    let preview = inspect_archive(&archive).unwrap();

    let (target_root, target_db, target_images, _) = fixture("Target", false);
    let safety = target_root.path().join("safety.humcase.zip");
    restore_archive_into(
        &target_db,
        &target_images,
        &archive,
        &safety,
        &preview.archive_sha256,
    )
    .unwrap();

    let target = Connection::open(&target_db).unwrap();
    let rows: Vec<(String, String)> = target
        .prepare("SELECT space, note_id FROM work_profiles")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, [("work".into(), "n".into())]);
}
