//! Internal restore. The caller must prevent concurrent image writes during
//! the operation; the SQLite lock protects the database's own writes.
use super::*;
#[cfg(test)]
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use rusqlite::params;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct RestoreSummary {
    pub project_count: u64,
    pub note_count: u64,
    pub image_count: u64,
    pub safety_archive_path: PathBuf,
}

/// Failure of [`restore_archive_into`], distinguished by whether the COMMIT may have been published.
///
/// An engine `Err` does not prove by itself that nothing was committed: after
/// a COMMIT I/O error, SQLite may have published the transaction. The caller
/// must therefore know whether the data state is **safe** (transaction cancelled by the
/// ROLLBACK) or **ambiguous** (COMMIT possibly published), so as never to reopen
/// writes on a potentially replaced database.
#[derive(Debug, PartialEq, Eq)]
pub enum RestoreFailure {
    /// Failure that occurred **before** any COMMIT attempt: the open transaction
    /// is cancelled by the exit ROLLBACK, nothing is committed.
    BeforeCommit(String),
    /// Failure that occurred **during or after the COMMIT**: SQLite may have published
    /// the transaction, the data state is uncertain.
    CommitAmbiguous(String),
}

impl RestoreFailure {
    /// Message for the user, without indicating the class (the caller decides
    /// the gate exit policy according to the variant).
    pub fn message(&self) -> &str {
        match self {
            RestoreFailure::BeforeCommit(message) | RestoreFailure::CommitAmbiguous(message) => {
                message
            }
        }
    }
}

impl std::fmt::Display for RestoreFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

struct StagedImages {
    paths: Vec<PathBuf>,
    committed: bool,
}

impl StagedImages {
    fn new() -> Self {
        Self {
            paths: Vec::new(),
            committed: false,
        }
    }
}

impl Drop for StagedImages {
    fn drop(&mut self) {
        if !self.committed {
            for path in &self.paths {
                let _ = fs::remove_file(path);
            }
        }
    }
}

#[cfg(test)]
pub(super) fn local_asset_url(path: &Path) -> String {
    // Same encoding as Tauri 2 core.js's `encodeURIComponent(filePath)` on Windows.
    const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'!')
        .remove(b'~')
        .remove(b'*')
        .remove(b'\'')
        .remove(b'(')
        .remove(b')');
    let path_string = path.to_string_lossy();
    let encoded = utf8_percent_encode(&path_string, COMPONENT);
    format!("http://asset.localhost/{encoded}")
}

/// True if the original name can be recreated as is in the images folder. A name that Windows
/// would treat as a device (`NUL.png`, `COM1`...) or whose trailing dot it would strip
/// would write elsewhere, or nowhere: these names, which no image saved by Humcase has
/// (UUID), go through renaming.
pub(super) fn name_can_be_kept(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|digit| {
                digit.len() == 1 && digit != "0" && digit.bytes().all(|b| b.is_ascii_digit())
            })
        });
    !device && !name.ends_with('.')
}

/// Is the file existing under this name exactly the archive's image (same size, same
/// fingerprint)? Symbolic link, folder, unreadable file or different size: no, without
/// ever following or reading beyond the expected size.
fn existing_image_matches(path: &Path, expected: &Entry) -> bool {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.len() == expected.size => {
            crate::images::hash_file(path)
                .is_ok_and(|(size, sha)| size == expected.size && sha == expected.sha256)
        }
        _ => false,
    }
}

/// Image of the archive as it exists in the folder after the restore.
struct RestoredImage {
    name: String,
    sha256: String,
    size: u64,
}

fn unique_image_path(images_dir: &Path, original: &str) -> PathBuf {
    let extension = original
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty() && ext.len() <= 16)
        .unwrap_or("bin");
    images_dir.join(format!("{}.{}", Uuid::new_v4(), extension))
}

fn extract_verified<R: Read, W: Write>(
    source: &mut R,
    destination: &mut W,
    expected: &Entry,
    limit: u64,
) -> Result<(), String> {
    let (size, sha) = sha_copy(source, destination, limit)?;
    if size != expected.size || sha != expected.sha256 {
        return Err(crate::tr!(
            "backup.fingerprintInvalid",
            path = expected.path
        ));
    }
    Ok(())
}

/// State prepared **outside COMMIT**: validated frozen copy, imported database extracted, live
/// transaction open. Any preparation error is of class `BeforeCommit`: nothing is published.
struct PreparedRestore {
    /// Keeps the temporary files alive during the whole restore.
    _work: tempfile::TempDir,
    live: Connection,
    zip: ZipArchive<File>,
    entries: BTreeMap<String, Entry>,
    import_db: PathBuf,
    preview: ArchivePreview,
}

fn prepare_restore(
    db_path: &Path,
    archive_path: &Path,
    expected_sha256: &str,
) -> Result<PreparedRestore, String> {
    // A private copy makes validation and extraction insensitive to the replacement of the ZIP
    // supplied by another process between these two steps.
    let work = tempfile::tempdir().map_err(|e| e.to_string())?;
    let frozen_archive = work.path().join("input.humcase.zip");
    let mut source = File::open(archive_path).map_err(|e| format!("Archive inaccessible: {e}"))?;
    let max_zip_size = MAX_TOTAL + 16 * 1024 * 1024;
    if source.metadata().map_err(|e| e.to_string())?.len() > max_zip_size {
        return Err(crate::tr!("backup.archiveTooLarge"));
    }
    let mut target = File::create(&frozen_archive).map_err(|e| e.to_string())?;
    let (_, frozen_hash) = sha_copy(&mut source, &mut target, max_zip_size)?;
    target.sync_all().map_err(|e| e.to_string())?;
    if frozen_hash != expected_sha256 {
        return Err(crate::tr!("backup.archiveChanged"));
    }
    let preview = inspect_archive(&frozen_archive)?;
    if preview.archive_sha256 != expected_sha256 {
        return Err(crate::tr!("backup.archiveChanged"));
    }
    let mut zip = ZipArchive::new(File::open(&frozen_archive).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let manifest: Manifest = {
        let mut entry = zip.by_name("manifest.json").map_err(|e| e.to_string())?;
        serde_json::from_reader(&mut entry).map_err(|e| e.to_string())?
    };
    let entries: BTreeMap<String, Entry> = manifest
        .entries
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect();
    let import_db = work.path().join("import.db");
    {
        // Current or former name of the database entry (checked unique by `inspect_archive`).
        let (db_entry, expected) = entries
            .iter()
            .find(|(name, _)| super::is_archive_db(name))
            .ok_or("Database missing")?;
        let mut source = zip.by_name(db_entry).map_err(|e| e.to_string())?;
        let mut target = File::create(&import_db).map_err(|e| e.to_string())?;
        extract_verified(&mut source, &mut target, expected, MAX_DB)?;
        target.sync_all().map_err(|e| e.to_string())?;
    }

    let live = Connection::open(db_path).map_err(crate::db::db_error)?;
    live.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(crate::db::db_error)?;
    live.execute_batch("PRAGMA foreign_keys=ON; BEGIN IMMEDIATE;")
        .map_err(|e| format!("Could not lock the database: {e}"))?;
    Ok(PreparedRestore {
        _work: work,
        live,
        zip,
        entries,
        import_db,
        preview,
    })
}

fn restore_archive_into_impl(
    db_path: &Path,
    images_dir: &Path,
    archive_path: &Path,
    safety_archive_path: &Path,
    expected_sha256: &str,
    #[cfg(test)] fail_before_commit: bool,
    #[cfg(test)] fail_on_commit: bool,
) -> Result<RestoreSummary, RestoreFailure> {
    // Any preparation error precedes the COMMIT: the transaction is cancelled, nothing is published.
    let prepared = prepare_restore(db_path, archive_path, expected_sha256)
        .map_err(RestoreFailure::BeforeCommit)?;
    let PreparedRestore {
        _work,
        live,
        mut zip,
        entries,
        import_db,
        preview,
    } = prepared;
    let mut staged = StagedImages::new();
    let pre_commit: Result<(), String> = (|| {
        check_db(&live)?;
        // The safety backup is published and validated before any live modification.
        create_archive(db_path, images_dir, safety_archive_path)?;
        inspect_archive(safety_archive_path)?;

        fs::create_dir_all(images_dir).map_err(|e| e.to_string())?;
        let mut urls = BTreeMap::new();
        let mut restored_images = Vec::new();
        for (name, expected) in entries
            .iter()
            .filter_map(|(path, entry)| path.strip_prefix("images/").map(|name| (name, entry)))
        {
            let mut source = zip.by_name(&expected.path).map_err(|e| e.to_string())?;
            // Original name kept (lot S3): free (`create_new`, no race with another
            // writer) or already occupied by the same image (same fingerprint) — in that case nothing is
            // written, and the existing file never enters `staged`: a failure before COMMIT must
            // not delete an image that the restore did not create. Only a real collision
            // (same name, different content) renames, and only its references will be rewritten.
            let original = images_dir.join(name);
            let mut kept = None;
            if name_can_be_kept(name) {
                match File::options().write(true).create_new(true).open(&original) {
                    Ok(file) => {
                        staged.paths.push(original.clone());
                        kept = Some((original.clone(), Some(file)));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        if existing_image_matches(&original, expected) {
                            kept = Some((original.clone(), None));
                        }
                    }
                    Err(e) => return Err(e.to_string()),
                }
            }
            let (path, created) = match kept {
                Some(kept) => kept,
                None => {
                    let (path, file) = loop {
                        let path = unique_image_path(images_dir, name);
                        match File::options().write(true).create_new(true).open(&path) {
                            Ok(file) => break (path, file),
                            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                            Err(e) => return Err(e.to_string()),
                        }
                    };
                    staged.paths.push(path.clone());
                    (path, Some(file))
                }
            };
            if let Some(mut target) = created {
                extract_verified(&mut source, &mut target, expected, MAX_IMAGE)?;
                target.sync_all().map_err(|e| e.to_string())?;
            }
            // Portable reference (0.9.81): the restored content no longer depends on the local path.
            let final_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("Invalid image name")?;
            urls.insert(name.to_string(), crate::images::reference(final_name));
            restored_images.push(RestoredImage {
                name: final_name.to_string(),
                sha256: expected.sha256.clone(),
                size: expected.size,
            });
        }

        {
            let mut imported = Connection::open(&import_db).map_err(crate::db::db_error)?;
            let imported_version: i32 = imported
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .map_err(crate::db::db_error)?;
            if imported_version < DB_SCHEMA_VERSION {
                crate::db::schema::apply_migrations(&imported)
                    .map_err(|e| format!("Archive migration impossible: {e}"))?;
            }
            // Working copy only: the image rewrite must be able to touch a trashed note,
            // whose v17 trigger freezes the content. Precaution: an archive produced
            // by `create_archive` no longer has this trigger; without this removal, a failure here
            // would cancel the restore without loss. The live database keeps it.
            // Same for dictation fragments (v20, append-only on the live database): their image
            // references follow those of `raw_content`.
            imported
                .execute_batch(
                    "PRAGMA foreign_keys=ON;
                     DROP TRIGGER IF EXISTS trashed_note_is_read_only;
                     DROP TRIGGER IF EXISTS dictation_fragments_no_update;",
                )
                .map_err(crate::db::db_error)?;
            let rows: Vec<(String, String, String)> = {
                let mut statement = imported
                    .prepare(NOTE_TEXTS_WITH_IMAGES)
                    .map_err(crate::db::db_error)?;
                let rows = statement
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                    .map_err(crate::db::db_error)?
                    .collect::<Result<_, _>>()
                    .map_err(crate::db::db_error)?;
                rows
            };
            let tx = imported.transaction().map_err(crate::db::db_error)?;
            for (id, column, content) in rows {
                let rewritten = rewrite_archive_images(&content, &urls)?;
                if rewritten != content {
                    tx.execute(update_note_text_sql(&column), params![rewritten, id])
                        .map_err(crate::db::db_error)?;
                }
            }
            tx.commit().map_err(crate::db::db_error)?;
            check_db(&imported)?;
            imported
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")
                .map_err(crate::db::db_error)?;
        }

        live.execute(
            "ATTACH DATABASE ?1 AS incoming",
            [import_db.to_string_lossy().as_ref()],
        )
        .map_err(crate::db::db_error)?;
        // The transaction replaces projects, notes and links with those of the archive. It also copies
        // the attributed journal `note_revisions` (previous values of agent edits): without this,
        // `DELETE FROM notes` would empty it by cascade and the restore would destroy the history.
        // `app_settings` is **deliberately not** copied: the agent edit authorization stays
        // that of the live profile — an archive must never silently re-enable editing.
        // Deletion traces (v21): the `DELETE`s below (projects, activities, links,
        // attachments, including by cascade) trigger traces, and the `INSERT`s erase
        // some of them. These are **technical** erasures, not user deletions:
        // at the end of the batch, `deletions` is emptied then copied from the archive, so the result
        // is exactly the archive's state (synchronization decision §8-6: lot S8). These two
        // statements must stay **after** all the other erasures and insertions.
        // Sub-cases (v22): notes are inserted without their parent, then attached in a second
        // statement, so the order of the rows never meets a parent that is not there yet (the
        // same-project and cycle triggers check each attachment against complete rows).
        // Provenance leaves with `DELETE FROM notes` by cascade and is copied back, and so do
        // the journal of who did what, the decisions on results and the opened cases (v24), and the
        // answers to the review of natures (v25), in
        // their original order (the brief and the decision state read the latest row). This
        // connection has no journal installed: the restore itself is nobody's action.
        // Revisions never move backwards (invariant 9): a note whose restored row differs from the
        // live one takes one more than the greater of its archive and live revisions, so an agent
        // holding a revision read before the restore gets a conflict instead of writing over the
        // restored content. A note restored identical keeps the greater of the two (restoring the
        // same archive twice changes nothing, S3), and a note absent from the live database keeps
        // the archive's.
        live.execute_batch(
            "DROP TABLE IF EXISTS temp.restore_live_notes;
             CREATE TEMP TABLE restore_live_notes AS SELECT * FROM notes;
             DELETE FROM note_revisions;
             DELETE FROM note_task_links;
             DELETE FROM note_tombstones;
             DELETE FROM notes;
             DELETE FROM project_activities;
             DELETE FROM projects;
             DELETE FROM activities;
             INSERT INTO projects (id, name, color, created_at, space, is_inbox, archived_at, updated_at, revision)
               SELECT id, name, color, created_at, space, is_inbox, archived_at, updated_at, revision FROM incoming.projects;
             INSERT INTO activities (id, name, space, created_at, updated_at, revision)
               SELECT id, name, space, created_at, updated_at, revision FROM incoming.activities;
             INSERT INTO project_activities (project_id, activity_id)
               SELECT project_id, activity_id FROM incoming.project_activities;
             INSERT INTO notes (id, project_id, title, content, note_type, status,
                 resolution_comment, raw_content, priority, git_branch, tags,
                 created_at, updated_at, due_at, revision, archived_at, deleted_at,
                 content_kind, canvas_x, canvas_y, canvas_w, canvas_h, order_key)
               SELECT i.id, i.project_id, i.title, i.content, i.note_type, i.status,
                 i.resolution_comment, i.raw_content, i.priority, i.git_branch, i.tags,
                 i.created_at, i.updated_at, i.due_at,
                 CASE
                   WHEN l.id IS NULL THEN i.revision
                   WHEN l.project_id IS i.project_id
                     AND l.title IS i.title
                     AND l.content IS i.content
                     AND l.note_type IS i.note_type
                     AND l.status IS i.status
                     AND l.resolution_comment IS i.resolution_comment
                     AND l.raw_content IS i.raw_content
                     AND l.priority IS i.priority
                     AND l.git_branch IS i.git_branch
                     AND l.tags IS i.tags
                     AND l.created_at IS i.created_at
                     AND l.updated_at IS i.updated_at
                     AND l.due_at IS i.due_at
                     AND l.archived_at IS i.archived_at
                     AND l.deleted_at IS i.deleted_at
                     AND l.content_kind IS i.content_kind
                     AND l.canvas_x IS i.canvas_x
                     AND l.canvas_y IS i.canvas_y
                     AND l.canvas_w IS i.canvas_w
                     AND l.canvas_h IS i.canvas_h
                     AND l.order_key IS i.order_key
                     AND l.parent_id IS i.parent_id
                     THEN MAX(i.revision, l.revision)
                   ELSE MAX(i.revision, l.revision) + 1
                 END,
                 i.archived_at, i.deleted_at,
                 i.content_kind, i.canvas_x, i.canvas_y, i.canvas_w, i.canvas_h, i.order_key
               FROM incoming.notes i LEFT JOIN temp.restore_live_notes l ON l.id = i.id;
             UPDATE notes SET parent_id = (SELECT i.parent_id FROM incoming.notes i WHERE i.id = notes.id)
               WHERE id IN (SELECT id FROM incoming.notes WHERE parent_id IS NOT NULL);
             INSERT INTO case_provenance (id, note_id, kind, label, url, excerpt, agent, created_at)
               SELECT id, note_id, kind, label, url, excerpt, agent, created_at FROM incoming.case_provenance;
             INSERT INTO dictation_fragments (id, note_id, text, created_at)
               SELECT id, note_id, text, created_at FROM incoming.dictation_fragments;
             INSERT INTO note_tombstones (id, project_id, deleted_at, purged_at)
               SELECT id, project_id, deleted_at, purged_at FROM incoming.note_tombstones;
             INSERT INTO note_revisions (id, note_id, revision, source, fields, created_at)
               SELECT id, note_id, revision, source, fields, created_at FROM incoming.note_revisions;
             INSERT INTO note_task_links (source_note_id, task_note_id, created_at)
               SELECT source_note_id, task_note_id, created_at FROM incoming.note_task_links;
             INSERT INTO case_events (id, note_id, kind, actor_kind, actor_name, detail, fields, created_at, last_at)
               SELECT id, note_id, kind, actor_kind, actor_name, detail, fields, created_at, last_at
               FROM incoming.case_events ORDER BY rowid;
             INSERT INTO case_reviews (id, note_id, decision, revision, comment, rework_number, created_at)
               SELECT id, note_id, decision, revision, comment, rework_number, created_at
               FROM incoming.case_reviews ORDER BY rowid;
             INSERT INTO case_views (note_id, seen_at)
               SELECT note_id, seen_at FROM incoming.case_views;
             INSERT INTO nature_checks (note_id, note_type, proposed, answer, checked_at)
               SELECT note_id, note_type, proposed, answer, checked_at FROM incoming.nature_checks;
             INSERT INTO proposals (id, note_id, kind, nature, title, passage, reason, proposed_by,
                                    near_id, follows, covers, revision, position, created_at,
                                    answer, answer_nature, answer_title, result_id, answered_at)
               SELECT id, note_id, kind, nature, title, passage, reason, proposed_by,
                      near_id, follows, covers, revision, position, created_at,
                      answer, answer_nature, answer_title, result_id, answered_at
               FROM incoming.proposals;
             INSERT INTO case_supersessions (note_id, by_id, superseded_at)
               SELECT note_id, by_id, superseded_at FROM incoming.case_supersessions;
             INSERT INTO work_profiles (space, note_id, created_at)
               SELECT space, note_id, created_at FROM incoming.work_profiles;
             DELETE FROM deletions;
             INSERT INTO deletions (entity, entity_id, deleted_at)
               SELECT entity, entity_id, deleted_at FROM incoming.deletions;
             INSERT INTO notes_fts(notes_fts) VALUES('rebuild');
             DROP TABLE temp.restore_live_notes;",
        )
        .map_err(crate::db::db_error)?;
        // Image registry (v19), in the same transaction as the rest: it describes the folder
        // (files from before the restore remain, so do its rows) and receives each
        // archive image under its final name. Cancelled with the transaction if the restore
        // fails before COMMIT; `sync_registry` catches up a missing row at the next startup.
        for image in &restored_images {
            crate::images::register_image(&live, &image.name, &image.sha256, image.size)
                .map_err(crate::db::db_error)?;
        }
        let (_, project_count, note_count) = check_db(&live)?;
        if project_count != preview.project_count || note_count != preview.note_count {
            return Err(crate::tr!("backup.countsInvalid"));
        }
        // The copied fragments exactly recompose the copied raw dictation, note by note. An
        // archive may lie (a v9 to v19 archive gets its own from the migration, identically);
        // the check applies to the result, so it also covers an incomplete copy.
        check_dictation_fragments(&live)?;
        live.execute(
            "INSERT INTO notes_fts(notes_fts, rank) VALUES('integrity-check', 1)",
            [],
        )
        .map_err(|e| crate::tr!("backup.ftsInvalid", error = e))?;
        #[cfg(test)]
        if fail_before_commit {
            return Err("Injected failure before COMMIT".to_string());
        }
        Ok(())
    })();

    if let Err(message) = pre_commit {
        // No COMMIT was attempted: the ROLLBACK cancels the open transaction, nothing is published.
        let _ = live.execute_batch("ROLLBACK");
        return Err(RestoreFailure::BeforeCommit(message));
    }

    // From here on, a COMMIT failure may mean SQLite published the transaction;
    // keeping the images is then safer than risking broken references.
    staged.committed = true;
    // Test injection: produces an error at the exact same point as the real COMMIT call,
    // so as to take the same exit path (safety ROLLBACK, images kept).
    #[cfg(test)]
    let commit_result = if fail_on_commit {
        Err(rusqlite::Error::InvalidQuery)
    } else {
        live.execute_batch("COMMIT")
    };
    #[cfg(not(test))]
    let commit_result = live.execute_batch("COMMIT");
    match commit_result {
        Ok(()) => Ok(RestoreSummary {
            project_count: preview.project_count,
            note_count: preview.note_count,
            image_count: preview.image_count,
            safety_archive_path: safety_archive_path.to_path_buf(),
        }),
        Err(error) => {
            // Safe ROLLBACK: no-op if the transaction was already published, cancellation otherwise.
            let _ = live.execute_batch("ROLLBACK");
            Err(RestoreFailure::CommitAmbiguous(crate::tr!(
                "backup.commitFailed",
                error = error
            )))
        }
    }
}

/// Replaces `humcase-archive://images/…` references (or the former `devnotes-archive://`) with the restored images. A missing
/// image returns an error (before any COMMIT) rather than aborting the process:
/// `inspect_archive` normally already refused it.
pub(super) fn rewrite_archive_images(
    content: &str,
    urls: &BTreeMap<String, String>,
) -> Result<String, String> {
    let mut missing = None;
    let rewritten = portable_regex().replace_all(content, |caps: &regex::Captures<'_>| {
        let link = &caps[0];
        let name = super::archive_image_name(link).unwrap_or(link);
        match urls.get(name) {
            Some(url) => url.clone(),
            None => {
                missing.get_or_insert_with(|| name.to_string());
                link.to_string()
            }
        }
    });
    match missing {
        Some(name) => Err(crate::tr!("backup.imageMissing", name = name)),
        None => Ok(rewritten.into_owned()),
    }
}

/// Replaces notes, projects, activities and their links in a single COMMIT, after a complete safety archive.
/// The caller must choose a new `safety_archive_path` and serialize access
/// to images with the other components of the application. `expected_sha256` comes from
/// `inspect_archive` during the preview and protects the choice confirmed by the user.
pub fn restore_archive_into(
    db_path: &Path,
    images_dir: &Path,
    archive_path: &Path,
    safety_archive_path: &Path,
    expected_sha256: &str,
) -> Result<RestoreSummary, RestoreFailure> {
    restore_archive_into_impl(
        db_path,
        images_dir,
        archive_path,
        safety_archive_path,
        expected_sha256,
        #[cfg(test)]
        false,
        #[cfg(test)]
        false,
    )
}

#[cfg(test)]
pub(super) fn restore_with_failure(
    db_path: &Path,
    images_dir: &Path,
    archive_path: &Path,
    safety_archive_path: &Path,
    expected_sha256: &str,
    fail_before_commit: bool,
    fail_on_commit: bool,
) -> Result<RestoreSummary, RestoreFailure> {
    restore_archive_into_impl(
        db_path,
        images_dir,
        archive_path,
        safety_archive_path,
        expected_sha256,
        fail_before_commit,
        fail_on_commit,
    )
}
