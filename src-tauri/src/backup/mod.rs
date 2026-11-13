//! Portable archive format v1. No path coming from the ZIP is extracted into the user folder.
use percent_encoding::percent_decode_str;
use regex::Regex;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const FORMAT_VERSION: u32 = 1;
const DB_SCHEMA_VERSION: i32 = 28;
const LEGACY_DB_SCHEMA_VERSION: i32 = 9;
const MAX_DB: u64 = 1024 * 1024 * 1024;
const MAX_IMAGE: u64 = 20 * 1024 * 1024;
const MAX_TOTAL: u64 = 2 * 1024 * 1024 * 1024;
const MAX_IMAGES: usize = 4096;
const MAX_MANIFEST: u64 = 1024 * 1024;
/// Database entry of an archive.
const ARCHIVE_DB: &str = "humcase.db";
/// Database entry of archives made before 0.10.0 (the product's former name), still restorable.
const FORMER_ARCHIVE_DB: &str = "devnotes.db";
/// Portable image reference inside an archive.
const ARCHIVE_IMAGES: &str = "humcase-archive://images/";
/// Same, in archives made before 0.10.0.
const FORMER_ARCHIVE_IMAGES: &str = "devnotes-archive://images/";

fn is_archive_db(name: &str) -> bool {
    name == ARCHIVE_DB || name == FORMER_ARCHIVE_DB
}

/// Image name of a portable archive reference, current or former prefix.
pub(super) fn archive_image_name(url: &str) -> Option<&str> {
    url.strip_prefix(ARCHIVE_IMAGES)
        .or_else(|| url.strip_prefix(FORMER_ARCHIVE_IMAGES))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format_version: u32,
    created_at: String,
    database_schema_version: i32,
    entries: Vec<Entry>,
}

#[derive(Debug, Serialize)]
pub struct ArchivePreview {
    pub archive_sha256: String,
    pub format_version: u32,
    pub created_at: String,
    /// Schema version **of the archive's origin** (that of its content), not the current
    /// version after migration: this is what the user previews.
    pub database_schema_version: i32,
    pub project_count: u64,
    pub note_count: u64,
    pub image_count: u64,
    pub total_uncompressed_bytes: u64,
}

pub fn safe_archive_name(input: &str) -> String {
    let safe: String = input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    if safe.is_empty() || safe.starts_with('.') || !safe.ends_with(".humcase.zip") {
        "humcase-backup.humcase.zip".to_string()
    } else {
        safe
    }
}

/// Lists the raw file names of the images folder, WITHOUT any validation: this
/// listing only serves to build `known` for the portable rewrite. The memory
/// bound applies to names (O(number of names)); no metadata is read for a
/// file until it is referenced by the SQLite copy. Entries that cannot be
/// converted to UTF-8 are logged on stderr (never stdout, reserved for the MCP).
fn list_image_dir_names(images_dir: &Path) -> Result<HashSet<String>, String> {
    let mut names = HashSet::new();
    if images_dir.exists() {
        for item in fs::read_dir(images_dir).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            match item.file_name().into_string() {
                Ok(name) => {
                    names.insert(name);
                }
                Err(_) => eprintln!(
                    "skipping orphan image with non-UTF-8 name in {}: {:?}",
                    images_dir.display(),
                    item.file_name()
                ),
            }
        }
    }
    Ok(names)
}

fn image_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn asset_regex() -> Regex {
    Regex::new(r#"(?i)(?:https?://asset\.localhost|asset://localhost)/[^\s<>\[\)"']+"#).unwrap()
}

fn portable_regex() -> Regex {
    Regex::new(r#"(?:humcase|devnotes)-archive://images/[^\s<>\[\)"']+"#).unwrap()
}

/// Path (lowercase, `/`) of an internal image, old or new data folder.
fn is_internal_image_path(lower: &str) -> bool {
    crate::images::images_segment_end(lower).is_some()
}

fn normalise_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches('/').to_string()
}

/// Only rewrites URLs of the exact application `images` folder.
fn make_portable(
    content: &str,
    images_dir: &Path,
    known_images: &HashSet<String>,
) -> Result<String, String> {
    let root = normalise_path(&images_dir.to_string_lossy()).to_ascii_lowercase() + "/";
    let mut error = None;
    let rewritten = asset_regex().replace_all(content, |caps: &regex::Captures<'_>| {
        let whole = caps.get(0).unwrap().as_str();
        let path = whole
            .split_once("://")
            .unwrap()
            .1
            .split_once('/')
            .map(|(_, p)| p)
            .unwrap_or("");
        let decoded = normalise_path(&percent_decode_str(path).decode_utf8_lossy());
        let lower = decoded.to_ascii_lowercase();
        if !lower.starts_with(&root) {
            if is_internal_image_path(&lower) {
                error = Some(crate::tr!("backup.imageOutsideFolder"));
            }
            return whole.to_string();
        }
        let name = &decoded[root.len()..];
        if !image_name(name) || !known_images.contains(name) {
            error = Some(crate::tr!("backup.imageMissingOrInvalid", name = name));
            return whole.to_string();
        }
        format!("{ARCHIVE_IMAGES}{name}")
    });
    if let Some(err) = error {
        return Err(err);
    }
    // Portable references (0.9.81): same treatment, the name must exist in the folder.
    let mut error = None;
    let rewritten = portable_image_regex().replace_all(&rewritten, |caps: &regex::Captures<'_>| {
        let name = &caps[1];
        if !image_name(name) || !known_images.contains(name) {
            error = Some(crate::tr!("backup.imageMissingOrInvalid", name = name));
            return caps[0].to_string();
        }
        format!("{ARCHIVE_IMAGES}{name}")
    });
    match error {
        Some(err) => Err(err),
        None => Ok(rewritten.into_owned()),
    }
}

fn portable_image_regex() -> Regex {
    Regex::new(r#"humcase-image://([^\s<>\[\)"']+)"#).unwrap()
}

/// Texts of a database that may cite images: `(row identifier, column, text)`.
/// Dictation fragments (v20) are rewritten like `raw_content`: a reference never crosses
/// the separator (two line breaks, which the URL patterns exclude), so rewriting each
/// fragment or the whole raw text gives the same recomposed text.
const NOTE_TEXTS_WITH_IMAGES: &str = "SELECT id, 'content', content FROM notes
    UNION ALL SELECT id, 'raw_content', raw_content FROM notes WHERE raw_content IS NOT NULL
    UNION ALL SELECT id, 'resolution_comment', resolution_comment FROM notes WHERE resolution_comment IS NOT NULL
    UNION ALL SELECT id, 'dictation_fragment', text FROM dictation_fragments";

/// Rewrite query for a text read by [`NOTE_TEXTS_WITH_IMAGES`] (`?1` text, `?2` identifier).
fn update_note_text_sql(column: &str) -> &'static str {
    match column {
        "raw_content" => "UPDATE notes SET raw_content = ?1 WHERE id = ?2",
        "resolution_comment" => "UPDATE notes SET resolution_comment = ?1 WHERE id = ?2",
        "dictation_fragment" => "UPDATE dictation_fragments SET text = ?1 WHERE id = ?2",
        _ => "UPDATE notes SET content = ?1 WHERE id = ?2",
    }
}

/// Topological traversal in O(notes + links). An archive may contain a long acyclic
/// chain: computing all reachable pairs with a recursive CTE becomes prohibitive.
fn note_task_links_have_cycle(conn: &Connection) -> Result<bool, String> {
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut indegree: HashMap<String, usize> = HashMap::new();
    let mut statement = conn
        .prepare("SELECT source_note_id, task_note_id FROM note_task_links")
        .map_err(|e| crate::tr!("backup.noteLinksUnreadable", error = e))?;
    let links = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| crate::tr!("backup.noteLinksUnreadable", error = e))?;
    for link in links {
        let (source, target) =
            link.map_err(|e| crate::tr!("backup.noteLinksUnreadable", error = e))?;
        indegree.entry(source.clone()).or_insert(0);
        *indegree.entry(target.clone()).or_insert(0) += 1;
        children.entry(source).or_default().push(target);
    }

    let total = indegree.len();
    let mut ready: VecDeque<String> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect();
    let mut visited = 0;
    while let Some(id) = ready.pop_front() {
        visited += 1;
        if let Some(targets) = children.get(&id) {
            for target in targets {
                let degree = indegree.get_mut(target).expect("target listed in indegree");
                *degree -= 1;
                if *degree == 0 {
                    ready.push_back(target.clone());
                }
            }
        }
    }
    Ok(visited != total)
}

/// Sub-case tree (v22): each case has at most one parent, so following parents from every case
/// either reaches a top-level case or loops. O(notes).
fn sub_cases_have_cycle(conn: &Connection) -> Result<bool, String> {
    let mut statement = conn
        .prepare("SELECT id, parent_id FROM notes WHERE parent_id IS NOT NULL")
        .map_err(|e| crate::tr!("backup.subCasesInvalidWithError", error = e))?;
    let parents: HashMap<String, String> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| crate::tr!("backup.subCasesInvalidWithError", error = e))?
        .collect::<Result<_, _>>()
        .map_err(|e| crate::tr!("backup.subCasesInvalidWithError", error = e))?;
    // 0 = not seen, 1 = on the current walk, 2 = reaches the top level.
    let mut state: HashMap<&str, u8> = HashMap::new();
    for start in parents.keys() {
        let mut walk: Vec<&str> = Vec::new();
        let mut current = Some(start.as_str());
        while let Some(id) = current {
            match state.get(id).copied().unwrap_or(0) {
                1 => return Ok(true),
                2 => break,
                _ => {}
            }
            state.insert(id, 1);
            walk.push(id);
            current = parents.get(id).map(String::as_str);
        }
        for id in walk {
            state.insert(id, 2);
        }
    }
    Ok(false)
}

fn check_db(conn: &Connection) -> Result<(i32, u64, u64), String> {
    let check: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| crate::tr!("backup.invalidDatabase", error = e))?;
    if check != "ok" {
        return Err(crate::tr!("backup.invalidDatabase", error = check));
    }
    let has_bad_foreign_key = conn
        .prepare("PRAGMA foreign_key_check")
        .and_then(|mut statement| statement.exists([]))
        .map_err(|e| crate::tr!("backup.foreignKeysInvalidWithError", error = e))?;
    if has_bad_foreign_key {
        return Err(crate::tr!("backup.foreignKeysInvalid"));
    }
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(crate::db::db_error)?;
    if version != DB_SCHEMA_VERSION {
        return Err(crate::tr!(
            "backup.schemaIncompatible",
            version = version,
            expected = DB_SCHEMA_VERSION
        ));
    }
    let invalid_spaces: i64 = conn
        .query_row(
            "SELECT count(*) FROM projects WHERE space IS NOT NULL AND space NOT IN ('private', 'work')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| crate::tr!("backup.projectSpacesInvalidWithError", error = e))?;
    if invalid_spaces != 0 {
        return Err(crate::tr!("backup.projectSpacesInvalid"));
    }
    let invalid_inboxes: i64 = conn
        .query_row(
            "SELECT count(*) FROM projects
             WHERE is_inbox IS NULL OR is_inbox NOT IN (0, 1) OR (is_inbox = 1 AND space IS NULL)",
            [],
            |r| r.get(0),
        )
        .map_err(|e| crate::tr!("backup.inboxesInvalidWithError", error = e))?;
    if invalid_inboxes != 0 {
        return Err(crate::tr!("backup.inboxesInvalid"));
    }
    let duplicate_inboxes: i64 = conn
        .query_row(
            "SELECT count(*) FROM (SELECT space FROM projects WHERE is_inbox = 1 GROUP BY space HAVING count(*) > 1)",
            [],
            |r| r.get(0),
        )
        .map_err(|e| crate::tr!("backup.inboxesInvalidWithError", error = e))?;
    if duplicate_inboxes != 0 {
        return Err(crate::tr!("backup.inboxesMultiple"));
    }
    let invalid_activity_spaces: i64 = conn
        .query_row(
            "SELECT count(*) FROM activities WHERE space IS NULL OR space NOT IN ('private', 'work')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| crate::tr!("backup.activitySpacesInvalidWithError", error = e))?;
    if invalid_activity_spaces != 0 {
        return Err(crate::tr!("backup.activitySpacesInvalid"));
    }
    let invalid_activity_links: i64 = conn
        .query_row(
            "SELECT count(*) FROM project_activities pa
             JOIN projects p ON p.id = pa.project_id
             JOIN activities a ON a.id = pa.activity_id
             WHERE p.space IS NULL OR p.space != a.space OR p.is_inbox = 1",
            [],
            |r| r.get(0),
        )
        .map_err(|e| crate::tr!("backup.activityLinksInvalidWithError", error = e))?;
    if invalid_activity_links != 0 {
        return Err(crate::tr!("backup.activityLinksInvalid"));
    }
    let invalid_note_task_links: i64 = conn
        .query_row(
            "SELECT count(*) FROM note_task_links link
             JOIN notes source ON source.id = link.source_note_id
             JOIN notes task ON task.id = link.task_note_id
             WHERE source.id = task.id OR source.project_id != task.project_id",
            [],
            |r| r.get(0),
        )
        .map_err(|e| crate::tr!("backup.noteLinksInvalidWithError", error = e))?;
    if invalid_note_task_links != 0 {
        return Err(crate::tr!("backup.noteLinksInvalid"));
    }
    // The current type of the task is not checked here: a valid link is kept
    // if the task changes type after being attached.
    if note_task_links_have_cycle(conn)? {
        return Err(crate::tr!("backup.noteLinksCycle"));
    }
    // Sub-cases (v22): the parent exists and shares the project, the content kind is known.
    let kinds = crate::models::note::ContentKind::ALL
        .map(|kind| format!("'{}'", kind.as_str()))
        .join(", ");
    let invalid_sub_cases: i64 = conn
        .query_row(
            &format!(
                "SELECT count(*) FROM notes child
                 LEFT JOIN notes parent ON parent.id = child.parent_id
                 WHERE (child.parent_id IS NOT NULL
                        AND (parent.id IS NULL OR parent.project_id != child.project_id
                             OR parent.id = child.id))
                    OR child.content_kind NOT IN ({kinds})"
            ),
            [],
            |r| r.get(0),
        )
        .map_err(|e| crate::tr!("backup.subCasesInvalidWithError", error = e))?;
    if invalid_sub_cases != 0 {
        return Err(crate::tr!("backup.subCasesInvalid"));
    }
    if sub_cases_have_cycle(conn)? {
        return Err(crate::tr!("backup.subCasesCycle"));
    }
    let projects: i64 = conn
        .query_row(
            "SELECT count(*) FROM projects WHERE is_inbox = 0",
            [],
            |r| r.get(0),
        )
        .map_err(|e| crate::tr!("backup.projectsTableInvalid", error = e))?;
    let notes: i64 = conn
        .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
        .map_err(|e| crate::tr!("backup.notesTableInvalid", error = e))?;
    Ok((version, projects as u64, notes as u64))
}

/// Refuses a database whose dictation fragments (v20) do not exactly recompose `raw_content`.
/// At archive time, the error appears right at backup rather than at an impossible restore; at
/// restore time, it refuses a forged archive before COMMIT.
fn check_dictation_fragments(conn: &Connection) -> Result<(), String> {
    let inconsistent = crate::db::queries::notes_with_inconsistent_fragments(conn)
        .map_err(|e| crate::tr!("backup.dictationFragmentsUnreadable", error = e))?;
    match inconsistent.first() {
        Some(id) => Err(crate::tr!("backup.dictationFragmentsInvalid", id = id)),
        None => Ok(()),
    }
}

fn sha_copy<R: Read, W: Write>(
    source: &mut R,
    dest: &mut W,
    limit: u64,
) -> Result<(u64, String), String> {
    let mut hasher = Sha256::new();
    let mut length = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = source.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        length = length
            .checked_add(count as u64)
            .ok_or_else(|| crate::tr!("backup.sizeExceeded"))?;
        if length > limit {
            return Err(crate::tr!("backup.entryTooLarge"));
        }
        hasher.update(&buffer[..count]);
        dest.write_all(&buffer[..count])
            .map_err(|e| e.to_string())?;
    }
    Ok((length, crate::images::lower_hex(&hasher.finalize())))
}

fn zip_options() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
}

/// Validation of a file referenced by the SQLite copy: name, type and size applicable
/// only to an image that enters the archive (never called on an orphan).
/// Errors are localized (shown to the user); the file name stays identified.
fn validate_referenced_image(images_dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !image_name(name) {
        return Err(crate::tr!("backup.imageInvalidName"));
    }
    let path = images_dir.join(name);
    // symlink_metadata does NOT follow symbolic links: a referenced link is rejected
    // as any non-file entry was in the old scan (no widening of the perimeter of
    // the archive content beyond the images folder).
    let file_type =
        fs::symlink_metadata(&path).map_err(|e| crate::tr!("backup.imageUnreadable", error = e))?;
    if !file_type.is_file() {
        return Err(crate::tr!("backup.imageUnsupportedEntry"));
    }
    if file_type.len() > MAX_IMAGE {
        return Err(crate::tr!(
            "backup.imageTooLarge",
            size = file_type.len(),
            max = MAX_IMAGE
        ));
    }
    Ok(path)
}

/// Produces a complete archive on the same volume, then publishes it without overwriting an existing archive.
pub fn create_archive(db_path: &Path, images_dir: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(crate::tr!(
            "backup.alreadyExists",
            path = destination.display()
        ));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| crate::tr!("backup.invalidDestination"))?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let work = tempfile::Builder::new()
        .prefix(".humcase-archive-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    let copy_path = work.path().join(ARCHIVE_DB);
    crate::db::save_to(db_path, &copy_path)?;

    let mut images: BTreeMap<String, PathBuf> = BTreeMap::new();
    // 1) Raw listing of the images folder names, without validation: `known` must reflect
    //    the actual content of the folder for the portable rewrite (distinct from external URLs,
    //    with a clean failure on a missing referenced image).
    let known: HashSet<String> = list_image_dir_names(images_dir)?;
    let mut referenced: HashSet<String> = HashSet::new();
    {
        let mut conn = Connection::open(&copy_path).map_err(crate::db::db_error)?;
        check_db(&conn)?;
        // Working copy only: images of a trashed note (content frozen by the
        // v17 trigger) must also become portable. The live database is not touched.
        // Same for dictation fragments (v20): their image references become portable
        // exactly like those of `raw_content`, so fragments always recompose the
        // raw text. Append-only on the live database, not on this copy.
        conn.execute_batch(
            "DROP TRIGGER IF EXISTS trashed_note_is_read_only;
             DROP TRIGGER IF EXISTS dictation_fragments_no_update;",
        )
        .map_err(crate::db::db_error)?;
        let rows: Vec<(String, String, String)> = {
            let mut stmt = conn
                .prepare(NOTE_TEXTS_WITH_IMAGES)
                .map_err(crate::db::db_error)?;
            let result = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(crate::db::db_error)?
                .collect::<Result<_, _>>()
                .map_err(crate::db::db_error)?;
            result
        };
        let tx = conn.transaction().map_err(crate::db::db_error)?;
        for (id, column, content) in rows {
            let portable = make_portable(&content, images_dir, &known)?;
            for url in portable_regex().find_iter(&portable) {
                referenced.insert(
                    archive_image_name(url.as_str())
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            if portable != content {
                tx.execute(update_note_text_sql(&column), (&portable, &id))
                    .map_err(crate::db::db_error)?;
            }
        }
        tx.commit().map_err(crate::db::db_error)?;
        check_dictation_fragments(&conn)?;
        // The copy must be self-contained: no temporary WAL enters the ZIP.
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")
            .map_err(crate::db::db_error)?;
        check_db(&conn)?;
    }
    // 2) Only images referenced in the embedded copy are exported; orphan
    //    files of the folder are not deleted from disk, only left out of the archive.
    //    The checks (name/type/size) now only apply to referenced files:
    //    an invalid orphan is logged on stderr (never stdout, reserved for the MCP)
    //    in list_image_dir_names and left out, instead of making an export fail that does not
    //    include it.
    for name in referenced {
        // `humcase-archive://images/...` (or former `devnotes-archive://`) URLs already present in the content (coming from an
        // earlier restore) do NOT go through make_portable's `known` check:
        // this guard covers them before any metadata read. It is load-bearing, do not
        // remove it (see the test portable_reference_without_image_is_rejected).
        if !known.contains(name.as_str()) {
            return Err(crate::tr!("backup.imageMissingFromFolder", name = name));
        }
        match validate_referenced_image(images_dir, &name) {
            Ok(path) => {
                images.insert(name.clone(), path);
            }
            Err(reason) => {
                return Err(crate::tr!(
                    "backup.imageInvalid",
                    name = name,
                    reason = reason
                ));
            }
        }
    }
    if images.len() > MAX_IMAGES {
        return Err(crate::tr!("backup.tooManyImages"));
    }
    if fs::metadata(&copy_path).map_err(|e| e.to_string())?.len() > MAX_DB {
        return Err(crate::tr!("backup.databaseTooLarge"));
    }

    let mut output = tempfile::Builder::new()
        .prefix(".humcase-zip-")
        .tempfile_in(parent)
        .map_err(|e| e.to_string())?;
    {
        let mut zip = ZipWriter::new(output.as_file_mut());
        let mut entries = Vec::new();
        let mut total = 0u64;
        let files = std::iter::once((ARCHIVE_DB.to_string(), copy_path)).chain(
            images
                .into_iter()
                .map(|(name, path)| (format!("images/{name}"), path)),
        );
        for (path, source) in files {
            let limit = if path == ARCHIVE_DB {
                MAX_DB
            } else {
                MAX_IMAGE
            };
            zip.start_file(&path, zip_options())
                .map_err(|e| e.to_string())?;
            let (size, sha256) = sha_copy(
                &mut File::open(&source).map_err(|e| e.to_string())?,
                &mut zip,
                limit,
            )?;
            total = total
                .checked_add(size)
                .ok_or_else(|| crate::tr!("backup.sizeExceeded"))?;
            if total > MAX_TOTAL {
                return Err(crate::tr!("backup.archiveTooLarge"));
            }
            entries.push(Entry { path, size, sha256 });
        }
        let conn = Connection::open_with_flags(
            work.path().join(ARCHIVE_DB),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(crate::db::db_error)?;
        let (database_schema_version, _, _) = check_db(&conn)?;
        let created_at: String = conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |r| {
                r.get(0)
            })
            .map_err(crate::db::db_error)?;
        let manifest = Manifest {
            format_version: FORMAT_VERSION,
            created_at,
            database_schema_version,
            entries,
        };
        let json = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
        if json.len() as u64 > MAX_MANIFEST {
            return Err(crate::tr!("backup.manifestTooLarge"));
        }
        zip.start_file("manifest.json", zip_options())
            .map_err(|e| e.to_string())?;
        zip.write_all(&json).map_err(|e| e.to_string())?;
        zip.finish().map_err(|e| e.to_string())?;
    }
    output.as_file().sync_all().map_err(|e| e.to_string())?;
    inspect_archive(output.path())?;
    // persist_noclobber is atomic and refuses a destination that appeared in the meantime.
    output
        .persist_noclobber(destination)
        .map_err(|e| e.error.to_string())?;
    Ok(())
}

fn valid_zip_path(path: &str) -> bool {
    if path == "manifest.json" || is_archive_db(path) {
        return true;
    }
    path.strip_prefix("images/")
        .map(image_name)
        .unwrap_or(false)
}

/// Validation reusable by the restore: reading and extraction of the DB into a temp folder only.
pub fn inspect_archive(path: &Path) -> Result<ArchivePreview, String> {
    let mut source =
        File::open(path).map_err(|e| crate::tr!("backup.archiveUnreadable", error = e))?;
    if source.metadata().map_err(|e| e.to_string())?.len() > MAX_TOTAL + 16 * 1024 * 1024 {
        return Err(crate::tr!("backup.archiveTooLarge"));
    }
    let (_, archive_sha256) = sha_copy(
        &mut source,
        &mut std::io::sink(),
        MAX_TOTAL + 16 * 1024 * 1024,
    )?;
    source.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut zip =
        ZipArchive::new(source).map_err(|e| crate::tr!("backup.zipInvalid", error = e))?;
    // The reader keeps one entry per name and reads names from the central directory only: an
    // archive with two entries of the same name, or whose local headers name other files than
    // the central directory, is refused before anything is read.
    zip.validate_header_consistency()
        .map_err(|_| crate::tr!("backup.zipInconsistent"))?;
    if zip.len() < 2 || zip.len() > MAX_IMAGES + 2 {
        return Err(crate::tr!("backup.entryCountInvalid"));
    }
    let mut seen = HashSet::new();
    let mut sizes = BTreeMap::new();
    let mut total = 0u64;
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let name = entry.name().map_err(|e| e.to_string())?;
        let name = name.as_ref();
        let invalid_unix_mode = entry
            .unix_mode()
            .map(|mode| mode & 0o170000 != 0 && mode & 0o170000 != 0o100000)
            .unwrap_or(false);
        if !valid_zip_path(name)
            || entry.is_dir()
            || name.contains('\\')
            || invalid_unix_mode
            || !seen.insert(name.to_ascii_lowercase())
        {
            return Err(crate::tr!("backup.zipPathInvalid", name = name));
        }
        let limit = if name == "manifest.json" {
            MAX_MANIFEST
        } else if is_archive_db(name) {
            MAX_DB
        } else {
            MAX_IMAGE
        };
        if entry.size() > limit {
            return Err(crate::tr!("backup.zipEntryTooLarge", name = name));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| crate::tr!("backup.sizeExceeded"))?;
        if total > MAX_TOTAL {
            return Err(crate::tr!("backup.decompressedTooLarge"));
        }
        sizes.insert(name.to_string(), entry.size());
    }
    // Exactly one database entry, under its current or former name.
    if !sizes.contains_key("manifest.json")
        || sizes.keys().filter(|n| is_archive_db(n)).count() != 1
    {
        return Err(crate::tr!("backup.manifestOrDatabaseMissing"));
    }
    let manifest: Manifest = {
        let entry = zip.by_name("manifest.json").map_err(|e| e.to_string())?;
        let mut json = Vec::new();
        entry
            .take(MAX_MANIFEST + 1)
            .read_to_end(&mut json)
            .map_err(|e| e.to_string())?;
        if json.len() as u64 > MAX_MANIFEST {
            return Err(crate::tr!("backup.manifestTooLarge"));
        }
        serde_json::from_slice(&json)
            .map_err(|e| crate::tr!("backup.manifestInvalid", error = e))?
    };
    if manifest.format_version != FORMAT_VERSION
        || !(LEGACY_DB_SCHEMA_VERSION..=DB_SCHEMA_VERSION)
            .contains(&manifest.database_schema_version)
    {
        return Err(crate::tr!("backup.versionIncompatible"));
    }
    let mut expected = BTreeMap::new();
    for item in &manifest.entries {
        if item.path == "manifest.json"
            || !valid_zip_path(&item.path)
            || item.sha256.len() != 64
            || !item
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || expected
                .insert(item.path.clone(), (item.size, item.sha256.clone()))
                .is_some()
        {
            return Err(crate::tr!("backup.manifestEntryInvalid"));
        }
    }
    if expected.len() + 1 != sizes.len() || expected.keys().any(|name| !sizes.contains_key(name)) {
        return Err(crate::tr!("backup.manifestZipMismatch"));
    }
    let work = tempfile::tempdir().map_err(|e| e.to_string())?;
    let db_path = work.path().join(ARCHIVE_DB);
    for (name, (size, hash)) in &expected {
        if sizes[name] != *size {
            return Err(crate::tr!("backup.sizeInvalid", name = name));
        }
        let mut entry = zip.by_name(name).map_err(|e| e.to_string())?;
        let limit = if is_archive_db(name) {
            MAX_DB
        } else {
            MAX_IMAGE
        };
        let (actual_size, actual_hash) = if is_archive_db(name) {
            sha_copy(
                &mut entry,
                &mut File::create(&db_path).map_err(|e| e.to_string())?,
                limit,
            )?
        } else {
            sha_copy(&mut entry, &mut std::io::sink(), limit)?
        };
        if actual_size != *size || actual_hash != *hash {
            return Err(crate::tr!("backup.hashInvalid", name = name));
        }
    }
    let conn =
        Connection::open(&db_path).map_err(|e| crate::tr!("backup.invalidDatabase", error = e))?;
    let original_version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| crate::tr!("backup.invalidDatabase", error = e))?;
    if original_version != manifest.database_schema_version {
        return Err(crate::tr!("backup.versionMismatch"));
    }
    // The extracted copy is temporary: migrating it to the current version never affects the
    // user's database. This is the official `apply_migrations` path, identical to the
    // restore (restore.rs), which makes it possible to inspect then restore old archives
    // (schema 9 to 15) instead of refusing them.
    if original_version < DB_SCHEMA_VERSION {
        crate::db::schema::apply_migrations(&conn)
            .map_err(|e| crate::tr!("backup.archiveMigrationFailed", error = e))?;
        let migrated: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| crate::tr!("backup.invalidDatabase", error = e))?;
        if migrated != DB_SCHEMA_VERSION {
            return Err(crate::tr!(
                "backup.archiveMigrationIncomplete",
                version = migrated,
                expected = DB_SCHEMA_VERSION
            ));
        }
    }
    let (_migrated_schema, project_count, note_count) = check_db(&conn)?;
    let image_names: HashSet<&str> = expected
        .keys()
        .filter_map(|p| p.strip_prefix("images/"))
        .collect();
    let mut stmt = conn
        .prepare(NOTE_TEXTS_WITH_IMAGES)
        .map_err(crate::db::db_error)?;
    let contents = stmt
        .query_map([], |r| r.get::<_, String>(2))
        .map_err(crate::db::db_error)?;
    for content in contents {
        let content = content.map_err(crate::db::db_error)?;
        for image_url in portable_regex().find_iter(&content) {
            let name = archive_image_name(image_url.as_str()).unwrap();
            if !image_name(name) || !image_names.contains(name) {
                return Err(crate::tr!("backup.imageMissingOrInvalid", name = name));
            }
        }
        if asset_regex().find_iter(&content).any(|url| {
            let path = url.as_str().split_once("://").unwrap().1;
            let decoded = percent_decode_str(path)
                .decode_utf8_lossy()
                .replace('\\', "/");
            is_internal_image_path(&decoded.to_ascii_lowercase())
        }) {
            return Err(crate::tr!("backup.imageUrlNotPortable"));
        }
    }
    Ok(ArchivePreview {
        archive_sha256,
        format_version: manifest.format_version,
        created_at: manifest.created_at,
        database_schema_version: original_version,
        project_count,
        note_count,
        image_count: image_names.len() as u64,
        total_uncompressed_bytes: total,
    })
}

pub mod restore;
pub use restore::{restore_archive_into, RestoreFailure, RestoreSummary};

#[cfg(test)]
mod tests;
