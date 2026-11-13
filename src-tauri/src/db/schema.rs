use rusqlite::{Connection, Result};

fn add_column_if_not_exists(conn: &Connection, table: &str, col_def: &str) -> Result<()> {
    // **Deterministic** detection via `PRAGMA table_info`: no longer depends on the exact wording of the
    // SQLite error message ("duplicate column name"), which can change between versions.
    let column = col_def.split_whitespace().next().unwrap_or("");
    let exists = conn
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(Result::ok)
        .any(|c| c == column);
    if exists {
        return Ok(());
    }
    conn.execute(&format!("ALTER TABLE {} ADD COLUMN {}", table, col_def), [])?;
    Ok(())
}

pub(crate) fn supported_version(conn: &Connection) -> Result<i32> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    const SUPPORTED_VERSION: i32 = 28;
    if version > SUPPORTED_VERSION {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
            Some(format!(
                "database version {version} is newer than supported version {SUPPORTED_VERSION}"
            )),
        ));
    }
    Ok(version)
}

pub fn apply_migrations(conn: &Connection) -> Result<()> {
    let version = supported_version(conn)?;

    if version < 1 {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                color TEXT DEFAULT '#3b82f6',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                note_type TEXT NOT NULL,
                status TEXT NOT NULL,
                resolution_comment TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_notes_project ON notes(project_id);
            CREATE INDEX IF NOT EXISTS idx_notes_status ON notes(status);
            ",
        )?;
        conn.execute("PRAGMA user_version = 1", [])?;
    }

    if version < 2 {
        add_column_if_not_exists(conn, "notes", "raw_content TEXT")?;
        conn.execute("PRAGMA user_version = 2", [])?;
    }

    if version < 3 {
        add_column_if_not_exists(conn, "notes", "priority TEXT")?;
        conn.execute("PRAGMA user_version = 3", [])?;
    }

    if version < 4 {
        add_column_if_not_exists(conn, "notes", "git_branch TEXT")?;
        conn.execute("PRAGMA user_version = 4", [])?;
    }

    if version < 5 {
        conn.execute_batch("
            CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(title, content, raw_content, content='notes', content_rowid='rowid');
            
            DROP TRIGGER IF EXISTS notes_ai;
            CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
                INSERT INTO notes_fts(rowid, title, content, raw_content) VALUES (new.rowid, new.title, new.content, new.raw_content);
            END;
            
            DROP TRIGGER IF EXISTS notes_ad;
            CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
                INSERT INTO notes_fts(notes_fts, rowid, title, content, raw_content) VALUES('delete', old.rowid, old.title, old.content, old.raw_content);
            END;
            
            DROP TRIGGER IF EXISTS notes_au;
            CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
                INSERT INTO notes_fts(notes_fts, rowid, title, content, raw_content) VALUES('delete', old.rowid, old.title, old.content, old.raw_content);
                INSERT INTO notes_fts(rowid, title, content, raw_content) VALUES (new.rowid, new.title, new.content, new.raw_content);
            END;
        ")?;
        conn.execute("INSERT INTO notes_fts(notes_fts) VALUES('rebuild')", [])?;
        conn.execute("PRAGMA user_version = 5", [])?;
    }

    if version < 6 {
        add_column_if_not_exists(conn, "notes", "tags TEXT DEFAULT '[]'")?;
        conn.execute("PRAGMA user_version = 6", [])?;
    }

    if version < 7 {
        // v7 — Explicit ISO 8601 UTC timestamps (ticket B8).
        //
        // Historical values were produced by `DEFAULT CURRENT_TIMESTAMP`, i.e. UTC
        // in the "YYYY-MM-DD HH:MM:SS" format (space, no time zone indication). On the webview side,
        // `new Date("2026-09-21 14:50:57")` is interpreted as *local* time (shifted by
        // the offset, +2 h in summer in Paris) and `split('T')[0]` split nothing.
        //
        // SQLite cannot change the DEFAULT of an existing column: the table is therefore
        // rebuilt (official procedure "making other kinds of table schema changes") and the
        // existing values are converted to ISO 8601 UTC ending with 'Z'. The indexes, the FTS5
        // triggers and the FTS index are recreated/rebuilt afterwards.
        conn.execute_batch(
            "
            BEGIN IMMEDIATE;

            CREATE TABLE notes_new (
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
                created_at DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                updated_at DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );

            INSERT INTO notes_new (id, project_id, title, content, note_type, status,
                                   resolution_comment, raw_content, priority, git_branch, tags,
                                   created_at, updated_at)
            SELECT id, project_id, title, content, note_type, status,
                   resolution_comment, raw_content, priority, git_branch, tags,
                   COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', created_at), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                   COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', updated_at), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            FROM notes;

            DROP TABLE notes;
            ALTER TABLE notes_new RENAME TO notes;

            CREATE INDEX IF NOT EXISTS idx_notes_project ON notes(project_id);
            CREATE INDEX IF NOT EXISTS idx_notes_status ON notes(status);

            DROP TRIGGER IF EXISTS notes_ai;
            CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
                INSERT INTO notes_fts(rowid, title, content, raw_content) VALUES (new.rowid, new.title, new.content, new.raw_content);
            END;

            DROP TRIGGER IF EXISTS notes_ad;
            CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
                INSERT INTO notes_fts(notes_fts, rowid, title, content, raw_content) VALUES('delete', old.rowid, old.title, old.content, old.raw_content);
            END;

            DROP TRIGGER IF EXISTS notes_au;
            CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
                INSERT INTO notes_fts(notes_fts, rowid, title, content, raw_content) VALUES('delete', old.rowid, old.title, old.content, old.raw_content);
                INSERT INTO notes_fts(rowid, title, content, raw_content) VALUES (new.rowid, new.title, new.content, new.raw_content);
            END;

            INSERT INTO notes_fts(notes_fts) VALUES('rebuild');

            PRAGMA user_version = 7;
            COMMIT;
            ",
        )?;

        // Projects also carry a timestamp: same normalization (no table to rebuild,
        // the original DEFAULT stays in place but no write path relies on it anymore).
        conn.execute(
            "UPDATE projects
                SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ', created_at)
              WHERE created_at IS NOT NULL AND created_at NOT LIKE '%T%'",
            [],
        )?;
    }

    if version < 8 {
        // v8 — Due dates (ticket "Due dates & reminders").
        //
        // `due_at` stores a **day** (`YYYY-MM-DD`), not an instant: a due date is thought of as a
        // local day ("due by Friday"), and comparing days avoids any time zone shift
        // between writing (UTC) and display (local). NULL = no due date.
        add_column_if_not_exists(conn, "notes", "due_at TEXT")?;
        conn.execute("PRAGMA user_version = 8", [])?;
    }

    if version < 9 {
        // v9 — Sort/filter indexes: all lists sort by `created_at DESC` and the standup
        // filters on `updated_at`. Without an index, every read scans the table.
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_notes_created_at ON notes(created_at DESC);
             CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at);",
        )?;
        conn.execute("PRAGMA user_version = 9", [])?;
    }

    if version < 10 {
        // Historical projects explicitly stay unclassified. A space is never
        // inferred from a project's name or notes.
        add_column_if_not_exists(
            conn,
            "projects",
            "space TEXT CHECK (space IS NULL OR space IN ('private', 'work'))",
        )?;
        conn.execute("PRAGMA user_version = 10", [])?;
    }

    if version < 11 {
        // Activities group projects without moving or duplicating their notes. Existing projects
        // remain unclassified; the migration never infers a space or an activity for them.
        add_column_if_not_exists(
            conn,
            "projects",
            "is_inbox INTEGER NOT NULL DEFAULT 0 CHECK (is_inbox IN (0, 1))",
        )?;
        conn.execute_batch("
            BEGIN IMMEDIATE;
            CREATE UNIQUE INDEX IF NOT EXISTS idx_projects_one_inbox_per_space ON projects(space) WHERE is_inbox = 1;
            CREATE TRIGGER IF NOT EXISTS inbox_requires_space_insert
            BEFORE INSERT ON projects WHEN NEW.is_inbox = 1 AND NEW.space IS NULL
            BEGIN SELECT RAISE(ABORT, 'inbox requires a space'); END;
            CREATE TRIGGER IF NOT EXISTS inbox_requires_space_update
            BEFORE UPDATE OF is_inbox, space ON projects
            WHEN NEW.is_inbox = 1 AND (NEW.space IS NULL OR (OLD.is_inbox = 1 AND NEW.space IS NOT OLD.space))
            BEGIN SELECT RAISE(ABORT, 'inbox space cannot be changed or cleared'); END;
            CREATE TRIGGER IF NOT EXISTS inbox_flag_immutable
            BEFORE UPDATE OF is_inbox ON projects WHEN NEW.is_inbox IS NOT OLD.is_inbox
            BEGIN SELECT RAISE(ABORT, 'inbox flag cannot be changed'); END;
            CREATE TABLE IF NOT EXISTS activities (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL CHECK (length(trim(name)) > 0),
                space TEXT NOT NULL CHECK (space IN ('private', 'work')),
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                UNIQUE (space, name COLLATE NOCASE)
            );
            CREATE TABLE IF NOT EXISTS project_activities (
                project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                activity_id TEXT NOT NULL REFERENCES activities(id) ON DELETE CASCADE,
                PRIMARY KEY (project_id, activity_id)
            );
            CREATE INDEX IF NOT EXISTS idx_project_activities_activity ON project_activities(activity_id);
            CREATE TRIGGER IF NOT EXISTS project_activities_same_space_insert
            BEFORE INSERT ON project_activities
            WHEN NOT EXISTS (
                SELECT 1 FROM projects p JOIN activities a ON a.id = NEW.activity_id
                WHERE p.id = NEW.project_id AND p.space = a.space AND p.is_inbox = 0
            )
            BEGIN SELECT RAISE(ABORT, 'project and activity must have the same space'); END;
            CREATE TRIGGER IF NOT EXISTS project_activities_same_space_update
            BEFORE UPDATE ON project_activities
            WHEN NOT EXISTS (
                SELECT 1 FROM projects p JOIN activities a ON a.id = NEW.activity_id
                WHERE p.id = NEW.project_id AND p.space = a.space AND p.is_inbox = 0
            )
            BEGIN SELECT RAISE(ABORT, 'project and activity must have the same space'); END;
            CREATE TRIGGER IF NOT EXISTS project_space_unlink_activities
            AFTER UPDATE OF space ON projects
            BEGIN
                DELETE FROM project_activities
                WHERE project_id = NEW.id AND activity_id NOT IN (
                    SELECT id FROM activities WHERE space = NEW.space
                );
            END;
            CREATE TRIGGER IF NOT EXISTS activity_space_unlink_projects
            AFTER UPDATE OF space ON activities
            BEGIN
                DELETE FROM project_activities
                WHERE activity_id = NEW.id AND project_id NOT IN (
                    SELECT id FROM projects WHERE space = NEW.space
                );
            END;
            PRAGMA user_version = 11;
            COMMIT;
        ")?;
    }

    if version < 12 {
        // A directed edge keeps its provenance and follows the deletion of a note. Undo
        // receives a snapshot of the edges through IPC, without persistent phantom history.
        conn.execute_batch(
            "
            BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS note_task_links (
                source_note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                task_note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                PRIMARY KEY (source_note_id, task_note_id),
                CHECK (source_note_id <> task_note_id)
            );
            CREATE INDEX IF NOT EXISTS idx_note_task_links_task ON note_task_links(task_note_id);
            CREATE TRIGGER IF NOT EXISTS note_task_links_same_project_insert
            BEFORE INSERT ON note_task_links
            WHEN NOT EXISTS (
                SELECT 1 FROM notes source JOIN notes task ON task.id = NEW.task_note_id
                WHERE source.id = NEW.source_note_id AND source.project_id = task.project_id
            )
            BEGIN SELECT RAISE(ABORT, 'linked notes must belong to the same project'); END;
            CREATE TRIGGER IF NOT EXISTS note_task_links_no_cycles_insert
            BEFORE INSERT ON note_task_links
            WHEN EXISTS (
                WITH RECURSIVE descendants(id) AS (
                    SELECT NEW.task_note_id
                    UNION
                    SELECT links.task_note_id FROM note_task_links links
                    JOIN descendants ON links.source_note_id = descendants.id
                ) SELECT 1 FROM descendants WHERE id = NEW.source_note_id
            )
            BEGIN SELECT RAISE(ABORT, 'note task links cannot form a cycle'); END;
            CREATE TRIGGER IF NOT EXISTS note_task_links_immutable
            BEFORE UPDATE ON note_task_links
            BEGIN SELECT RAISE(ABORT, 'note task link endpoints are immutable'); END;
            CREATE TRIGGER IF NOT EXISTS note_with_links_cannot_move_project
            BEFORE UPDATE OF project_id ON notes
            WHEN NEW.project_id <> OLD.project_id AND EXISTS (
                SELECT 1 FROM note_task_links
                WHERE source_note_id = OLD.id OR task_note_id = OLD.id
            )
            BEGIN SELECT RAISE(ABORT, 'unlink note task relations before moving this note'); END;
            PRAGMA user_version = 12;
            COMMIT;
        ",
        )?;
    }

    if version < 13 {
        // v13 — the FTS index must cover the report (`resolution_comment`).
        //
        // Without it, a term recorded only in a report (bug resolution,
        // agent result) was never found by search. FTS5 with external
        // content maps its columns by **name** onto the `notes` table: adding the
        // column to the index requires recreating the virtual table and thus the three triggers
        // that keep it up to date, then rebuilding the index for existing data.
        conn.execute_batch("
            BEGIN IMMEDIATE;
            DROP TRIGGER IF EXISTS notes_ai;
            DROP TRIGGER IF EXISTS notes_ad;
            DROP TRIGGER IF EXISTS notes_au;
            DROP TABLE IF EXISTS notes_fts;
            CREATE VIRTUAL TABLE notes_fts USING fts5(title, content, raw_content, resolution_comment, content='notes', content_rowid='rowid');
            CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
                INSERT INTO notes_fts(rowid, title, content, raw_content, resolution_comment) VALUES (new.rowid, new.title, new.content, new.raw_content, new.resolution_comment);
            END;
            CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
                INSERT INTO notes_fts(notes_fts, rowid, title, content, raw_content, resolution_comment) VALUES('delete', old.rowid, old.title, old.content, old.raw_content, old.resolution_comment);
            END;
            CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
                INSERT INTO notes_fts(notes_fts, rowid, title, content, raw_content, resolution_comment) VALUES('delete', old.rowid, old.title, old.content, old.raw_content, old.resolution_comment);
                INSERT INTO notes_fts(rowid, title, content, raw_content, resolution_comment) VALUES (new.rowid, new.title, new.content, new.raw_content, new.resolution_comment);
            END;
            INSERT INTO notes_fts(notes_fts) VALUES('rebuild');
            PRAGMA user_version = 13;
            COMMIT;
        ")?;
    }

    if version < 14 {
        // v14 — note editing by an MCP agent: concurrency version + attributed journal.
        //
        // `revision` is incremented on **every** write of a note (human app or agent), which
        // lets an agent refuse to overwrite a change made since its read
        // (`expected_revision`). `app_settings` carries the permission flag
        // `mcp_allow_note_edit`: absent = agent editing refused. `note_revisions` keeps, for
        // each AGENT write, a JSON snapshot of the **previous** values of the modified fields
        // (never `raw_content`: append-only dictation invariant).
        add_column_if_not_exists(conn, "notes", "revision INTEGER NOT NULL DEFAULT 0")?;
        conn.execute_batch("
            BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS note_revisions (
                id TEXT PRIMARY KEY,
                note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                revision INTEGER NOT NULL,
                source TEXT NOT NULL,
                fields TEXT NOT NULL,
                -- ISO 8601 UTC timestamp, like the v11/v12 tables: `CURRENT_TIMESTAMP`
                -- would produce YYYY-MM-DD HH:MM:SS (no time zone), which the webview reads
                -- as local time (bug fixed by migration v7).
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            );
            CREATE INDEX IF NOT EXISTS idx_note_revisions_note ON note_revisions(note_id, revision DESC);
            PRAGMA user_version = 14;
            COMMIT;
        ")?;
    }

    if version < 15 {
        // v15 — archiving and deletion of projects, archiving of notes.
        //
        // `archived_at` (ISO 8601 UTC, NULL = active) hides a project or a note without erasing anything.
        // Deleting a project erases its notes by cascade (`ON DELETE CASCADE`): the application
        // first moves its notes to the space's Inbox, and the trigger below refuses
        // as a last resort the deletion of a project that still has notes. Archive restore
        // empties `notes` before `projects` and is therefore not blocked.
        add_column_if_not_exists(conn, "projects", "archived_at TEXT")?;
        add_column_if_not_exists(conn, "notes", "archived_at TEXT")?;
        conn.execute_batch(
            "
            BEGIN IMMEDIATE;
            CREATE TRIGGER IF NOT EXISTS project_with_notes_cannot_be_deleted
            BEFORE DELETE ON projects
            WHEN EXISTS (SELECT 1 FROM notes WHERE project_id = OLD.id)
            BEGIN SELECT RAISE(ABORT, 'move this project notes before deleting it'); END;
            PRAGMA user_version = 15;
            COMMIT;
        ",
        )?;
    }

    if version < 16 {
        // v16 — portable images. The local URLs `http://asset.localhost/<absolute path>` of internal
        // images become `humcase-image://<name>` (see `crate::images`) in the content, the raw text
        // and the report. No other data is touched; the revision advances for each note
        // rewritten (invariant 9), the modification date does not change.
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        let resultat = (|| -> Result<()> {
            let lignes: Vec<(String, String, Option<String>, Option<String>)> = {
                let mut stmt = conn.prepare(
                    "SELECT id, content, raw_content, resolution_comment FROM notes
                     WHERE content LIKE '%asset%' OR raw_content LIKE '%asset%' OR resolution_comment LIKE '%asset%'",
                )?;
                let lignes = stmt
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                    .collect::<Result<_>>()?;
                lignes
            };
            for (id, contenu, brut, compte_rendu) in lignes {
                let portable = |t: &str| crate::images::make_portable(t);
                let nouveau_contenu = portable(&contenu);
                let nouveau_brut = brut.as_deref().map(portable);
                let nouveau_compte_rendu = compte_rendu.as_deref().map(portable);
                if nouveau_contenu != contenu
                    || nouveau_brut != brut
                    || nouveau_compte_rendu != compte_rendu
                {
                    conn.execute(
                        "UPDATE notes SET content = ?1, raw_content = ?2, resolution_comment = ?3,
                         revision = revision + 1 WHERE id = ?4",
                        rusqlite::params![nouveau_contenu, nouveau_brut, nouveau_compte_rendu, id],
                    )?;
                }
            }
            conn.execute("PRAGMA user_version = 16", [])?;
            Ok(())
        })();
        match resultat {
            Ok(()) => conn.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
    }

    if version < 17 {
        // v17 — trash. `deleted_at` (ISO 8601 UTC, NULL = live) places a note in the
        // trash without erasing anything: links, history and images stay in place, and the note
        // comes back identical on restore. Reads ignore it (see `queries`).
        //
        // The trigger freezes the content of a note in the trash: only its project (project
        // deletion, move of linked notes), its revision, its modification date and its exit from the
        // trash can change. Any ordinary write (editor, MCP, dictation, structuring,
        // escrow) is refused by SQLite, whatever its path. A maintenance rewrite
        // (archives) drops this trigger on its working **copy** only.
        //
        // `note_tombstones` keeps the minimal trace of a purged note (without content) for the future
        // synchronization.
        add_column_if_not_exists(conn, "notes", "deleted_at TEXT")?;
        conn.execute_batch(
            "
            BEGIN IMMEDIATE;
            CREATE INDEX IF NOT EXISTS idx_notes_deleted_at ON notes(deleted_at);
            CREATE TABLE IF NOT EXISTS note_tombstones (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                deleted_at TEXT NOT NULL,
                purged_at TEXT NOT NULL
            );
            CREATE TRIGGER IF NOT EXISTS trashed_note_is_read_only
            BEFORE UPDATE ON notes
            WHEN OLD.deleted_at IS NOT NULL AND NEW.deleted_at IS NOT NULL AND (
                NEW.title IS NOT OLD.title OR NEW.content IS NOT OLD.content
                OR NEW.note_type IS NOT OLD.note_type OR NEW.status IS NOT OLD.status
                OR NEW.resolution_comment IS NOT OLD.resolution_comment
                OR NEW.raw_content IS NOT OLD.raw_content OR NEW.priority IS NOT OLD.priority
                OR NEW.git_branch IS NOT OLD.git_branch OR NEW.tags IS NOT OLD.tags
                OR NEW.due_at IS NOT OLD.due_at OR NEW.archived_at IS NOT OLD.archived_at
                OR NEW.created_at IS NOT OLD.created_at OR NEW.deleted_at IS NOT OLD.deleted_at
            )
            BEGIN SELECT RAISE(ABORT, 'note is in the trash'); END;
            PRAGMA user_version = 17;
            COMMIT;
        ",
        )?;
    }

    if version < 18 {
        // v18 — concurrency version of projects and activities (batch S1 of the synchronization
        // preparation), modeled on `notes.revision` / `notes.updated_at`.
        //
        // `revision` (0 for existing rows) advances on every write of the project or
        // activity, their attachments included; `updated_at` (ISO 8601 UTC ending with `Z`)
        // is initialized to `created_at`. SQLite refuses a non-constant DEFAULT in an
        // `ALTER TABLE ADD COLUMN`: the column is born as '' then filled, and an insert
        // trigger guarantees that a row created without `updated_at` gets `created_at`.
        //
        // The two triggers that remove attachments when a space changes are
        // recreated: they also advance the version of the other side of the removed attachment,
        // where the deletion takes place (no application code sees it).
        add_column_if_not_exists(conn, "projects", "revision INTEGER NOT NULL DEFAULT 0")?;
        add_column_if_not_exists(conn, "projects", "updated_at TEXT NOT NULL DEFAULT ''")?;
        add_column_if_not_exists(conn, "activities", "revision INTEGER NOT NULL DEFAULT 0")?;
        add_column_if_not_exists(conn, "activities", "updated_at TEXT NOT NULL DEFAULT ''")?;
        conn.execute_batch("
            BEGIN IMMEDIATE;
            UPDATE projects
               SET updated_at = COALESCE(NULLIF(created_at, ''), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             WHERE updated_at = '';
            UPDATE activities
               SET updated_at = COALESCE(NULLIF(created_at, ''), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             WHERE updated_at = '';
            CREATE TRIGGER IF NOT EXISTS projects_default_updated_at
            AFTER INSERT ON projects WHEN NEW.updated_at = ''
            BEGIN
                UPDATE projects
                   SET updated_at = COALESCE(NULLIF(NEW.created_at, ''), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 WHERE id = NEW.id;
            END;
            CREATE TRIGGER IF NOT EXISTS activities_default_updated_at
            AFTER INSERT ON activities WHEN NEW.updated_at = ''
            BEGIN
                UPDATE activities
                   SET updated_at = COALESCE(NULLIF(NEW.created_at, ''), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 WHERE id = NEW.id;
            END;
            DROP TRIGGER IF EXISTS project_space_unlink_activities;
            CREATE TRIGGER project_space_unlink_activities
            AFTER UPDATE OF space ON projects
            BEGIN
                UPDATE activities
                   SET revision = revision + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id IN (
                     SELECT activity_id FROM project_activities
                     WHERE project_id = NEW.id AND activity_id NOT IN (
                         SELECT id FROM activities WHERE space = NEW.space
                     )
                 );
                DELETE FROM project_activities
                WHERE project_id = NEW.id AND activity_id NOT IN (
                    SELECT id FROM activities WHERE space = NEW.space
                );
            END;
            DROP TRIGGER IF EXISTS activity_space_unlink_projects;
            CREATE TRIGGER activity_space_unlink_projects
            AFTER UPDATE OF space ON activities
            BEGIN
                UPDATE projects
                   SET revision = revision + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id IN (
                     SELECT project_id FROM project_activities
                     WHERE activity_id = NEW.id AND project_id NOT IN (
                         SELECT id FROM projects WHERE space = NEW.space
                     )
                 );
                DELETE FROM project_activities
                WHERE activity_id = NEW.id AND project_id NOT IN (
                    SELECT id FROM projects WHERE space = NEW.space
                );
            END;
            PRAGMA user_version = 18;
            COMMIT;
        ")?;
    }

    if version < 19 {
        // v19 — image registry (batch S3 of the synchronization preparation): one row per
        // file of the `images` folder, with its SHA-256 fingerprint and its size.
        //
        // The migration cannot read the disk: the table starts empty. It is filled outside the
        // migration, at startup (`crate::images::sync_registry`, idempotent), then on each
        // image save and each archive restore. The registry describes the folder,
        // not the notes: no foreign key, no automatic deletion.
        conn.execute_batch(
            "
            BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS images (
                name TEXT PRIMARY KEY,
                sha256 TEXT NOT NULL,
                size INTEGER NOT NULL,
                created_at TEXT NOT NULL
            );
            PRAGMA user_version = 19;
            COMMIT;
        ",
        )?;
    }

    if version < 20 {
        // v20 — raw dictation in fragments (batch S6 of the synchronization preparation).
        //
        // `raw_content` remains the read of the raw dictation, unchanged. Each append
        // (`append_raw_content`) also writes, in the same transaction, an **append-only** fragment:
        // a future synchronization will take the union of the fragments instead of a "last write wins"
        // on a field that would lose a dictation (invariant 3).
        //
        // Carrying over existing data: a single fragment per note whose raw text is not empty, with text
        // **exactly** equal to `raw_content` (no splitting), dated from the note's `updated_at`
        // (normalized ISO format, like all dates since v7). No other data changes:
        // neither `raw_content`, nor `revision`, nor `updated_at`.
        //
        // The triggers refuse any modification and any direct deletion of a fragment.
        // Only the deletion of the note itself removes them (cascade: permanent erasure from
        // the trash, traced by `note_tombstones`, or replacement by an archive restore):
        // when the cascade runs, the note no longer exists, the trigger's condition is
        // false. A maintenance rewrite (archives) drops these triggers on its working
        // **copy** only.
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        let resultat = (|| -> Result<()> {
            conn.execute_batch("
                CREATE TABLE IF NOT EXISTS dictation_fragments (
                    id TEXT PRIMARY KEY,
                    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                    text TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_dictation_fragments_note ON dictation_fragments(note_id, created_at);
            ")?;
            // Fragment date: `updated_at`, failing that `created_at`, failing that the current instant. A
            // very old database may not have these columns: the query only uses those that
            // exist, so as never to make the migration fail.
            let colonnes: Vec<String> = conn
                .prepare("SELECT name FROM pragma_table_info('notes')")?
                .query_map([], |r| r.get(0))?
                .collect::<Result<_>>()?;
            let date = ["updated_at", "created_at", "'now'"]
                .iter()
                .filter(|c| c.starts_with('\'') || colonnes.iter().any(|n| n == *c))
                .map(|c| format!("strftime('%Y-%m-%dT%H:%M:%fZ', {c})"))
                .collect::<Vec<_>>();
            let date = match date.as_slice() {
                [seule] => seule.clone(),
                _ => format!("COALESCE({})", date.join(", ")),
            };
            let lignes: Vec<(String, String, String)> = {
                let mut stmt = conn.prepare(&format!(
                    "SELECT id, raw_content, {date}
                       FROM notes
                      WHERE raw_content IS NOT NULL AND raw_content <> ''
                        AND NOT EXISTS (SELECT 1 FROM dictation_fragments f WHERE f.note_id = notes.id)"
                ))?;
                let lignes = stmt
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                    .collect::<Result<_>>()?;
                lignes
            };
            for (note_id, brut, date) in lignes {
                conn.execute(
                    "INSERT INTO dictation_fragments (id, note_id, text, created_at) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![uuid::Uuid::new_v4().to_string(), note_id, brut, date],
                )?;
            }
            conn.execute_batch(
                "
                CREATE TRIGGER IF NOT EXISTS dictation_fragments_no_update
                BEFORE UPDATE ON dictation_fragments
                BEGIN SELECT RAISE(ABORT, 'dictation fragments are append-only'); END;
                CREATE TRIGGER IF NOT EXISTS dictation_fragments_no_delete
                BEFORE DELETE ON dictation_fragments
                WHEN EXISTS (SELECT 1 FROM notes WHERE id = OLD.note_id)
                BEGIN SELECT RAISE(ABORT, 'dictation fragments are append-only'); END;
                PRAGMA user_version = 20;
            ",
            )?;
            Ok(())
        })();
        match resultat {
            Ok(()) => conn.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
    }

    if version < 21 {
        // v21 — general deletion traces (batch S2 of the synchronization preparation):
        // no deletion without a trace, including by cascade or by trigger, so that a future
        // device that did not see the deletion does not resurrect the item.
        //
        // `deletions(entity, entity_id, deleted_at)` keeps one row per deleted item. The
        // `AFTER DELETE` triggers fill it: SQLite also runs them for rows
        // removed by a foreign key cascade or by another trigger, which the application
        // code never sees. The `AFTER INSERT` triggers erase the trace of a recreated item (links
        // removed then restored during a linked component move, attachment
        // restored).
        //
        // Traced entities (`entity`) and serialization of `entity_id`:
        //   - `project`, `activity`      : the identifier as is;
        //   - `note_task_link`           : JSON array `["<source_note_id>","<task_note_id>"]`;
        //   - `project_activity`         : JSON array `["<project_id>","<activity_id>"]`.
        // A JSON array (`json_array`) escapes each component: two distinct pairs never
        // give the same text, whatever the characters of the identifiers (a simple
        // separator would be ambiguous if an identifier contained it). The order is that of the
        // table's primary key.
        //
        // `note_tombstones` (purged notes) is unchanged, and dictation fragments are not
        // traced: they leave with their case, already traced. `deleted_at` is ISO 8601 UTC like
        // the other dates; a new deletion of the same item updates the date.
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        let resultat = (|| -> Result<()> {
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS deletions (
                    entity TEXT NOT NULL,
                    entity_id TEXT NOT NULL,
                    deleted_at TEXT NOT NULL,
                    PRIMARY KEY (entity, entity_id)
                );
                CREATE TRIGGER IF NOT EXISTS projects_trace_delete
                AFTER DELETE ON projects
                BEGIN
                    INSERT INTO deletions (entity, entity_id, deleted_at)
                    VALUES ('project', OLD.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                    ON CONFLICT (entity, entity_id) DO UPDATE SET deleted_at = excluded.deleted_at;
                END;
                CREATE TRIGGER IF NOT EXISTS projects_untrace_insert
                AFTER INSERT ON projects
                BEGIN
                    DELETE FROM deletions WHERE entity = 'project' AND entity_id = NEW.id;
                END;
                CREATE TRIGGER IF NOT EXISTS activities_trace_delete
                AFTER DELETE ON activities
                BEGIN
                    INSERT INTO deletions (entity, entity_id, deleted_at)
                    VALUES ('activity', OLD.id, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                    ON CONFLICT (entity, entity_id) DO UPDATE SET deleted_at = excluded.deleted_at;
                END;
                CREATE TRIGGER IF NOT EXISTS activities_untrace_insert
                AFTER INSERT ON activities
                BEGIN
                    DELETE FROM deletions WHERE entity = 'activity' AND entity_id = NEW.id;
                END;
                CREATE TRIGGER IF NOT EXISTS note_task_links_trace_delete
                AFTER DELETE ON note_task_links
                BEGIN
                    INSERT INTO deletions (entity, entity_id, deleted_at)
                    VALUES ('note_task_link', json_array(OLD.source_note_id, OLD.task_note_id),
                            strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                    ON CONFLICT (entity, entity_id) DO UPDATE SET deleted_at = excluded.deleted_at;
                END;
                CREATE TRIGGER IF NOT EXISTS note_task_links_untrace_insert
                AFTER INSERT ON note_task_links
                BEGIN
                    DELETE FROM deletions
                    WHERE entity = 'note_task_link'
                      AND entity_id = json_array(NEW.source_note_id, NEW.task_note_id);
                END;
                CREATE TRIGGER IF NOT EXISTS project_activities_trace_delete
                AFTER DELETE ON project_activities
                BEGIN
                    INSERT INTO deletions (entity, entity_id, deleted_at)
                    VALUES ('project_activity', json_array(OLD.project_id, OLD.activity_id),
                            strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                    ON CONFLICT (entity, entity_id) DO UPDATE SET deleted_at = excluded.deleted_at;
                END;
                CREATE TRIGGER IF NOT EXISTS project_activities_untrace_insert
                AFTER INSERT ON project_activities
                BEGIN
                    DELETE FROM deletions
                    WHERE entity = 'project_activity'
                      AND entity_id = json_array(NEW.project_id, NEW.activity_id);
                END;
                PRAGMA user_version = 21;
            ",
            )?;
            Ok(())
        })();
        match resultat {
            Ok(()) => conn.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
    }

    if version < 22 {
        // v22 — sub-cases (lot 5, step 1). Every case is a note: a top-level case has no
        // parent, a sub-case has one (`parent_id`) and a content kind (`content_kind`: text,
        // image, link, file, sketch, research; validated on the Rust side). Depth is unlimited
        // and the structure is a strict tree:
        //   - a sub-case lives in its parent's project (checked on insert and on every change of
        //     parent; a project change moves a whole tree, done by the Rust code);
        //   - a case never goes inside itself or one of its descendants (cycle refused by
        //     walking up the new parent's ancestors, like the v12 link triggers);
        //   - purging a parent from the trash promotes its remaining sub-cases to top level
        //     (trigger `sub_case_parent_purged`): nothing is erased by cascade. Trashing a case
        //     that still has live sub-cases is refused by the Rust code.
        // `parent_id` is kept by triggers rather than a foreign key: the parent must exist on
        // every insert or change of parent, and its deletion detaches its sub-cases. A restore
        // can insert the rows in any order then attach them.
        // `canvas_x/y/w/h` (position and size on the parent's canvas) and `order_key`
        // (fractional order among siblings) are reserved for the canvas (step 2).
        //
        // `case_provenance` records where a case comes from (source, link, agent research),
        // append-only like the dictation fragments (v20): no update, no deletion while the
        // case exists; the rows leave with the case when it is purged.
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        let resultat = (|| -> Result<()> {
            add_column_if_not_exists(conn, "notes", "parent_id TEXT")?;
            add_column_if_not_exists(conn, "notes", "content_kind TEXT NOT NULL DEFAULT 'text'")?;
            add_column_if_not_exists(conn, "notes", "canvas_x REAL")?;
            add_column_if_not_exists(conn, "notes", "canvas_y REAL")?;
            add_column_if_not_exists(conn, "notes", "canvas_w REAL")?;
            add_column_if_not_exists(conn, "notes", "canvas_h REAL")?;
            add_column_if_not_exists(conn, "notes", "order_key TEXT")?;
            conn.execute_batch(
                "
                CREATE INDEX IF NOT EXISTS idx_notes_parent ON notes(parent_id);
                CREATE TRIGGER IF NOT EXISTS sub_case_same_project_insert
                BEFORE INSERT ON notes
                WHEN NEW.parent_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM notes parent
                    WHERE parent.id = NEW.parent_id AND parent.project_id = NEW.project_id
                )
                BEGIN SELECT RAISE(ABORT, 'a sub-case must belong to its parent case project'); END;
                CREATE TRIGGER IF NOT EXISTS sub_case_same_project_update
                BEFORE UPDATE OF parent_id ON notes
                WHEN NEW.parent_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM notes parent
                    WHERE parent.id = NEW.parent_id AND parent.project_id = NEW.project_id
                )
                BEGIN SELECT RAISE(ABORT, 'a sub-case must belong to its parent case project'); END;
                CREATE TRIGGER IF NOT EXISTS sub_case_parent_purged
                AFTER DELETE ON notes
                BEGIN UPDATE notes SET parent_id = NULL WHERE parent_id = OLD.id; END;
                CREATE TRIGGER IF NOT EXISTS sub_case_no_cycle
                BEFORE UPDATE OF parent_id ON notes
                WHEN NEW.parent_id IS NOT NULL AND EXISTS (
                    WITH RECURSIVE ancestors(id) AS (
                        SELECT NEW.parent_id
                        UNION
                        SELECT notes.parent_id FROM notes
                        JOIN ancestors ON notes.id = ancestors.id
                        WHERE notes.parent_id IS NOT NULL
                    ) SELECT 1 FROM ancestors WHERE id = NEW.id
                )
                BEGIN SELECT RAISE(ABORT, 'a case cannot go inside itself or its sub-cases'); END;
                CREATE TABLE IF NOT EXISTS case_provenance (
                    id TEXT PRIMARY KEY,
                    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                    kind TEXT NOT NULL,
                    label TEXT,
                    url TEXT,
                    excerpt TEXT,
                    agent TEXT,
                    created_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_case_provenance_note ON case_provenance(note_id, created_at);
                CREATE TRIGGER IF NOT EXISTS case_provenance_no_update
                BEFORE UPDATE ON case_provenance
                BEGIN SELECT RAISE(ABORT, 'case provenance is append-only'); END;
                CREATE TRIGGER IF NOT EXISTS case_provenance_no_delete
                BEFORE DELETE ON case_provenance
                WHEN EXISTS (SELECT 1 FROM notes WHERE id = OLD.note_id)
                BEGIN SELECT RAISE(ABORT, 'case provenance is append-only'); END;
                PRAGMA user_version = 22;
            ",
            )?;
            Ok(())
        })();
        match resultat {
            Ok(()) => conn.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
    }

    if version < 23 {
        // v23 — the technical name of the Inbox containers follows the product's name:
        // `__devnotes_inbox_<space>_<id>` becomes `__humcase_inbox_<space>_<id>`. The name is
        // never shown in the interface nor used to find an Inbox (`is_inbox` is); agents see
        // it in `list_projects`. The rename is a write of the project: its version advances.
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        let resultat = conn.execute_batch(
            "
            UPDATE projects
               SET name = '__humcase_inbox_' || substr(name, 18),
                   revision = revision + 1,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE is_inbox = 1 AND substr(name, 1, 17) = '__devnotes_inbox_';
            PRAGMA user_version = 23;
        ",
        );
        match resultat {
            Ok(()) => conn.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
    }

    if version < 24 {
        // v24 — who did what on a case, and the human's decision on a result.
        //
        // `case_events` is the attributed journal of the writes on a case (created, edited,
        // status, report, moved, trashed, archived). It is filled by the per-connection
        // triggers of `db::journal`, with the actor of the session (the person in the
        // interface, the agent named by its MCP client, the CLI), never by the callers: no
        // write path can forget it. Consecutive edits of the same actor are merged into one
        // row (`fields`, `last_at`), and a creation receives the parent its sub-case is
        // attached to in the same transaction; nothing else in a row ever changes and a row is never
        // deleted while its case exists. No history is invented for the writes made before v24.
        //
        // `case_reviews` holds the human decision on a result (accepted, rework requested),
        // distinct from the status an agent declares: the reviewed revision, the reason of a
        // rework and its number. Append-only; written only by the interface (`db::journal`
        // refuses another actor). `case_views` remembers when a case was last opened in the
        // interface, for the brief; `app_settings.brief_seen_until` is "mark all as seen".
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        let resultat = conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS case_events (
                id TEXT PRIMARY KEY,
                note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                kind TEXT NOT NULL,
                actor_kind TEXT NOT NULL,
                actor_name TEXT,
                detail TEXT NOT NULL DEFAULT '{}',
                fields TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                last_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_case_events_note ON case_events(note_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_case_events_last ON case_events(last_at);
            CREATE TRIGGER IF NOT EXISTS case_events_only_merge_edits
            BEFORE UPDATE ON case_events
            WHEN NEW.id IS NOT OLD.id OR NEW.note_id IS NOT OLD.note_id
              OR NEW.kind IS NOT OLD.kind OR NEW.actor_kind IS NOT OLD.actor_kind
              OR NEW.actor_name IS NOT OLD.actor_name OR NEW.created_at IS NOT OLD.created_at
              OR (OLD.kind != 'edited' AND (NEW.fields IS NOT OLD.fields OR NEW.last_at IS NOT OLD.last_at))
              OR (NEW.detail IS NOT OLD.detail AND NOT (
                    OLD.kind = 'created' AND json_extract(OLD.detail, '$.parent_id') IS NULL
                    AND json_remove(NEW.detail, '$.parent_id') IS json_remove(OLD.detail, '$.parent_id')))
            BEGIN SELECT RAISE(ABORT, 'case events are append-only'); END;
            CREATE TRIGGER IF NOT EXISTS case_events_no_delete
            BEFORE DELETE ON case_events
            WHEN EXISTS (SELECT 1 FROM notes WHERE id = OLD.note_id)
            BEGIN SELECT RAISE(ABORT, 'case events are append-only'); END;
            CREATE TABLE IF NOT EXISTS case_reviews (
                id TEXT PRIMARY KEY,
                note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                decision TEXT NOT NULL CHECK (decision IN ('accepted', 'rework_requested')),
                revision INTEGER NOT NULL,
                comment TEXT,
                rework_number INTEGER,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_case_reviews_note ON case_reviews(note_id, created_at);
            CREATE TRIGGER IF NOT EXISTS case_reviews_no_update
            BEFORE UPDATE ON case_reviews
            BEGIN SELECT RAISE(ABORT, 'case reviews are append-only'); END;
            CREATE TRIGGER IF NOT EXISTS case_reviews_no_delete
            BEFORE DELETE ON case_reviews
            WHEN EXISTS (SELECT 1 FROM notes WHERE id = OLD.note_id)
            BEGIN SELECT RAISE(ABORT, 'case reviews are append-only'); END;
            CREATE TABLE IF NOT EXISTS case_views (
                note_id TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
                seen_at TEXT NOT NULL
            );
            PRAGMA user_version = 24;
        ",
        );
        match resultat {
            Ok(()) => conn.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
    }

    if version < 25 {
        // `nature_checks` remembers the person's answers to the review of doubtful natures: the
        // nature proposed, and the nature they kept or chose instead. A kept nature is not
        // proposed again while the case keeps it, and the answers weigh more than other cases
        // when Humcase reads a nature from the nearest cases (`natures.rs`). One row per case,
        // the latest answer; it leaves with its case.
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        let resultat = conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS nature_checks (
                note_id TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
                note_type TEXT NOT NULL,
                proposed TEXT NOT NULL,
                answer TEXT NOT NULL CHECK (answer IN ('kept', 'changed')),
                checked_at TEXT NOT NULL
            );
            PRAGMA user_version = 25;
        ",
        );
        match resultat {
            Ok(()) => conn.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
    }

    if version < 26 {
        // v26 — what Humcase and agents propose for a case, and the person's answer
        // (`germinate.rs`). One table for every kind of proposal; the first kind is `sub_case`:
        // a next step of the case, with the passage of the case it comes from, the near case
        // that may already cover it, and Jev's checks (`follows`, `covers`: probabilities).
        // `answer` stays NULL while the proposal waits: `created`, `placed` (the near case moved
        // under this case), `skipped` (covered by the near case), `refused`, or `replaced` by a
        // newer germination. The answers feed the work profile later; they leave with the case.
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        let resultat = conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS proposals (
                id TEXT PRIMARY KEY,
                note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                kind TEXT NOT NULL,
                nature TEXT NOT NULL,
                title TEXT NOT NULL,
                passage TEXT NOT NULL,
                reason TEXT,
                proposed_by TEXT,
                near_id TEXT REFERENCES notes(id) ON DELETE SET NULL,
                follows REAL,
                covers REAL,
                revision INTEGER NOT NULL,
                position INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                answer TEXT CHECK (answer IN ('created', 'placed', 'skipped', 'refused', 'replaced')),
                answer_nature TEXT,
                answer_title TEXT,
                result_id TEXT REFERENCES notes(id) ON DELETE SET NULL,
                answered_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_proposals_note ON proposals(note_id, answer, created_at);
            PRAGMA user_version = 26;
        ",
        );
        match resultat {
            Ok(()) => conn.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
    }

    if version < 27 {
        // v27 — keeping the memory tidy (`tidy.rs`). `proposals` gets two kinds: `archive` (a case
        // resolved long ago) and `supersede` (an older decision the newer one, `near_id`, may
        // replace). Two answers join the CHECK, so the table is rebuilt with its rows: `accepted`
        // (the person said yes) and `dismissed` (Humcase judged the pair not worth asking, so it
        // is not judged again). `case_supersessions` records that a decision (`note_id`) was
        // replaced by a newer one (`by_id`): the older stays readable, marked as history.
        // The legacy rename leaves the other objects alone: nothing refers to `proposals`, and
        // the default rename would check every trigger of the schema again.
        conn.execute_batch("PRAGMA legacy_alter_table = ON; BEGIN IMMEDIATE;")?;
        let resultat = conn.execute_batch(
            "
            CREATE TABLE proposals_v27 (
                id TEXT PRIMARY KEY,
                note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                kind TEXT NOT NULL,
                nature TEXT NOT NULL,
                title TEXT NOT NULL,
                passage TEXT NOT NULL,
                reason TEXT,
                proposed_by TEXT,
                near_id TEXT REFERENCES notes(id) ON DELETE SET NULL,
                follows REAL,
                covers REAL,
                revision INTEGER NOT NULL,
                position INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                answer TEXT CHECK (answer IN ('created', 'placed', 'skipped', 'refused', 'replaced', 'accepted', 'dismissed')),
                answer_nature TEXT,
                answer_title TEXT,
                result_id TEXT REFERENCES notes(id) ON DELETE SET NULL,
                answered_at TEXT
            );
            INSERT INTO proposals_v27 (id, note_id, kind, nature, title, passage, reason, proposed_by,
                near_id, follows, covers, revision, position, created_at, answer, answer_nature,
                answer_title, result_id, answered_at)
            SELECT id, note_id, kind, nature, title, passage, reason, proposed_by, near_id, follows,
                covers, revision, position, created_at, answer, answer_nature, answer_title,
                result_id, answered_at
            FROM proposals;
            DROP TABLE proposals;
            ALTER TABLE proposals_v27 RENAME TO proposals;
            CREATE INDEX IF NOT EXISTS idx_proposals_note ON proposals(note_id, answer, created_at);
            CREATE INDEX IF NOT EXISTS idx_proposals_kind ON proposals(kind, answer);
            CREATE TABLE IF NOT EXISTS case_supersessions (
                note_id TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
                by_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                superseded_at TEXT NOT NULL,
                CHECK (note_id <> by_id)
            );
            CREATE INDEX IF NOT EXISTS idx_case_supersessions_by ON case_supersessions(by_id);
            PRAGMA user_version = 27;
        ",
        );
        let outcome = match resultat {
            Ok(()) => conn.execute_batch("COMMIT;"),
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                Err(e)
            }
        };
        let _ = conn.execute_batch("PRAGMA legacy_alter_table = OFF;");
        outcome?;
    }

    if version < 28 {
        // v28 — the work profile (`profile.rs`): one note per space says how the person works
        // (rules, lines of work, what they often refuse). The note is an ordinary case of the
        // space's Inbox, so the person reads and edits it like any other; this table only says
        // which note it is. Proposals to add a line and the nudges live in `proposals`
        // (kinds `profile` and `nudge`), whose kind has no CHECK.
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        let resultat = conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS work_profiles (
                space TEXT PRIMARY KEY CHECK (space IN ('private', 'work')),
                note_id TEXT NOT NULL UNIQUE REFERENCES notes(id) ON DELETE CASCADE,
                created_at TEXT NOT NULL
            );
            PRAGMA user_version = 28;
        ",
        );
        match resultat {
            Ok(()) => conn.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
    }

    Ok(())
}
