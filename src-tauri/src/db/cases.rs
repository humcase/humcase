//! Sub-cases (v22): every case is a note; a sub-case has a parent case and a content kind.
//!
//! The tree is strict and of unlimited depth. The database refuses a cycle and a parent in
//! another project (triggers `sub_case_*`); this module adds the rules the triggers cannot see
//! (trash, clear messages) and keeps every write in one transaction that advances the revision
//! of the note it changes (invariant 9).

use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::db::queries;
use crate::models::note::{
    is_sketch_scene, CaseProvenance, ContentKind, CreateNoteInput, Note, EMPTY_SKETCH,
    MAX_SKETCH_BYTES,
};

/// Longest provenance `kind` accepted (a short word, not a sentence).
const PROVENANCE_KIND_MAX: usize = 40;
/// Longest label, url and excerpt of a provenance, in characters: an excerpt is a quote, not a
/// copy of the source.
const PROVENANCE_LABEL_MAX: usize = 500;
const PROVENANCE_URL_MAX: usize = 2048;
const PROVENANCE_EXCERPT_MAX: usize = 10_000;

/// `(project_id, in_trash)` of a note, `None` when it does not exist.
pub(crate) fn project_and_trash(
    conn: &Connection,
    id: &str,
) -> Result<Option<(String, bool)>, String> {
    conn.query_row(
        "SELECT project_id, deleted_at IS NOT NULL FROM notes WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(crate::db::db_error)
}

/// Turns a refusal from the v22 triggers into a readable message; other errors pass through.
pub(crate) fn trigger_error(error: rusqlite::Error) -> String {
    let text = error.to_string();
    if text.contains("cannot go inside itself") {
        crate::tr!("data.caseCycle")
    } else if text.contains("must belong to its parent case project") {
        crate::tr!("data.subCaseSameProject")
    } else {
        text
    }
}

/// Creates a sub-case inside `parent_id`, in the parent's project. `input.project_id` may be
/// empty; when given it must be the parent's project. Returns the new identifier.
/// Checks that `content` may be the content of a case of this kind: a sketch keeps a valid
/// Excalidraw scene of reasonable size, so no write (editor, agent, structuring) replaces the
/// drawing with text. Other kinds take any text.
pub fn check_content(kind: ContentKind, content: &str) -> Result<(), String> {
    if kind != ContentKind::Sketch {
        return Ok(());
    }
    if content.len() > MAX_SKETCH_BYTES {
        return Err(crate::tr!("data.sketchTooLarge"));
    }
    if !is_sketch_scene(content) {
        return Err(crate::tr!("data.sketchInvalid"));
    }
    Ok(())
}

pub fn create_sub_case(
    conn: &Connection,
    parent_id: &str,
    kind: ContentKind,
    mut input: CreateNoteInput,
) -> Result<String, String> {
    if kind == ContentKind::Sketch {
        match input.content.as_deref() {
            None | Some("") => input.content = Some(EMPTY_SKETCH.to_string()),
            Some(content) => check_content(kind, content)?,
        }
    }
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let (project_id, parent_in_trash) = project_and_trash(&tx, parent_id)?
        .ok_or_else(|| crate::tr!("data.parentCaseNotFound", id = parent_id))?;
    if parent_in_trash {
        return Err(crate::tr!("data.trashedCannotReceiveSubCase"));
    }
    if !input.project_id.is_empty() && input.project_id != project_id {
        return Err(crate::tr!("data.subCaseSameProject"));
    }
    input.project_id = project_id;
    let id = queries::create_note(&tx, input)?;
    tx.execute(
        "UPDATE notes SET parent_id = ?2, content_kind = ?3 WHERE id = ?1",
        params![id, parent_id, kind],
    )
    .map_err(trigger_error)?;
    tx.commit().map_err(crate::db::db_error)?;
    Ok(id)
}

/// Moves a case inside `new_parent_id`, or back to top level with `None`. The case keeps its
/// sub-cases. Refused: a case in the trash, a parent in the trash or in another project, and
/// a parent that is the case itself or one of its descendants.
pub fn move_case(conn: &Connection, id: &str, new_parent_id: Option<&str>) -> Result<(), String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    move_case_in(&tx, id, new_parent_id)?;
    tx.commit().map_err(crate::db::db_error)
}

/// `move_case` inside a transaction the caller opened and commits.
pub(crate) fn move_case_in(
    tx: &Connection,
    id: &str,
    new_parent_id: Option<&str>,
) -> Result<(), String> {
    let (project_id, in_trash) =
        project_and_trash(tx, id)?.ok_or_else(|| crate::tr!("notes.notFound", id = id))?;
    if in_trash {
        return Err(crate::tr!("data.trashedCannotMove"));
    }
    if let Some(parent_id) = new_parent_id {
        let (parent_project, parent_in_trash) = project_and_trash(tx, parent_id)?
            .ok_or_else(|| crate::tr!("data.parentCaseNotFound", id = parent_id))?;
        if parent_in_trash {
            return Err(crate::tr!("data.trashedCannotReceiveSubCase"));
        }
        if parent_project != project_id {
            return Err(crate::tr!("data.subCaseSameProject"));
        }
    }
    let current: Option<String> = tx
        .query_row("SELECT parent_id FROM notes WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .map_err(crate::db::db_error)?;
    if current.as_deref() == new_parent_id {
        return Ok(());
    }
    tx.execute(
        // A position belongs to the parent's board: on another board it would land anywhere,
        // so the case is placed again where it arrives.
        "UPDATE notes SET parent_id = ?2, canvas_x = NULL, canvas_y = NULL,
                          updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1",
        params![id, new_parent_id],
    )
    .map_err(trigger_error)?;
    queries::bump_revision(tx, id)?;
    Ok(())
}

/// A live sub-case and its depth below the case that was asked for (1 = direct sub-case).
#[derive(Debug, serde::Serialize, Clone)]
pub struct SubCase {
    pub depth: i64,
    #[serde(flatten)]
    pub note: Note,
}

/// Live sub-cases of a case, in tree order (each case followed by its own sub-cases,
/// siblings by `order_key` then creation). `max_depth` 1 gives the direct sub-cases only.
/// A sub-case in the trash is left out with everything below it.
pub fn list_sub_cases(
    conn: &Connection,
    id: &str,
    max_depth: Option<i64>,
) -> Result<Vec<SubCase>, String> {
    if project_and_trash(conn, id)?.is_none() {
        return Err(crate::tr!("notes.notFound", id = id));
    }
    let limit = max_depth.unwrap_or(i64::MAX).max(1);
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE tree(id, depth, path) AS (
                 SELECT n.id, 1, printf('%s|%s|%s', COALESCE(n.order_key, ''), n.created_at, n.id)
                   FROM notes n WHERE n.parent_id = ?1 AND n.deleted_at IS NULL
                 UNION ALL
                 SELECT n.id, tree.depth + 1,
                        tree.path || '/' || printf('%s|%s|%s', COALESCE(n.order_key, ''), n.created_at, n.id)
                   FROM notes n JOIN tree ON n.parent_id = tree.id
                  WHERE n.deleted_at IS NULL AND tree.depth < ?2
             )
             SELECT tree.depth, n.id, n.project_id, n.title, n.content, n.note_type, n.status,
                    n.resolution_comment, n.raw_content, n.priority, n.git_branch, n.tags,
                    n.created_at, n.updated_at, n.due_at, n.revision, n.archived_at, n.deleted_at,
                    n.parent_id, n.content_kind
               FROM tree JOIN notes n ON n.id = tree.id
              ORDER BY tree.path",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map(params![id, limit], |row| {
            Ok(SubCase {
                depth: row.get(0)?,
                note: Note {
                    id: row.get(1)?,
                    project_id: row.get(2)?,
                    title: row.get(3)?,
                    content: row.get(4)?,
                    note_type: row.get(5)?,
                    status: row.get(6)?,
                    resolution_comment: row.get(7)?,
                    raw_content: row.get(8)?,
                    priority: row.get(9)?,
                    git_branch: row.get(10)?,
                    tags: row.get(11)?,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                    due_at: row.get(14)?,
                    revision: row.get(15)?,
                    archived_at: row.get(16)?,
                    deleted_at: row.get(17)?,
                    parent_id: row.get(18)?,
                    content_kind: row.get(19)?,
                },
            })
        })
        .map_err(crate::db::db_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::db::db_error);
    rows
}

/// Ancestors of a case from the top-level case down to its direct parent: `(id, title)`.
pub fn case_path(conn: &Connection, id: &str) -> Result<Vec<(String, String)>, String> {
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE ancestors(id, depth) AS (
                 SELECT parent_id, 1 FROM notes WHERE id = ?1 AND parent_id IS NOT NULL
                 UNION ALL
                 SELECT notes.parent_id, ancestors.depth + 1 FROM notes
                   JOIN ancestors ON notes.id = ancestors.id
                  WHERE notes.parent_id IS NOT NULL
             )
             SELECT n.id, n.title FROM ancestors JOIN notes n ON n.id = ancestors.id
              ORDER BY ancestors.depth DESC",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(crate::db::db_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::db::db_error);
    rows
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// Records where a case comes from. Append-only: a provenance is never changed or removed
/// while its case exists. At least one of `label`, `url` or `excerpt` is required.
pub fn add_provenance(
    conn: &Connection,
    note_id: &str,
    kind: &str,
    label: Option<&str>,
    url: Option<&str>,
    excerpt: Option<&str>,
    agent: Option<&str>,
) -> Result<CaseProvenance, String> {
    let kind = kind.trim();
    if kind.is_empty() || kind.chars().count() > PROVENANCE_KIND_MAX {
        return Err(format!(
            "provenance kind must contain 1 to {PROVENANCE_KIND_MAX} characters"
        ));
    }
    let (label, url, excerpt, agent) = (
        non_empty(label),
        non_empty(url),
        non_empty(excerpt),
        non_empty(agent),
    );
    if label.is_none() && url.is_none() && excerpt.is_none() {
        return Err("provenance needs a label, a url or an excerpt".to_string());
    }
    for (field, value, max) in [
        ("label", label.as_deref(), PROVENANCE_LABEL_MAX),
        ("url", url.as_deref(), PROVENANCE_URL_MAX),
        ("excerpt", excerpt.as_deref(), PROVENANCE_EXCERPT_MAX),
        ("agent", agent.as_deref(), PROVENANCE_LABEL_MAX),
    ] {
        if value.is_some_and(|v| v.chars().count() > max) {
            return Err(format!(
                "provenance {field} must contain at most {max} characters"
            ));
        }
    }
    // A provenance is shown with its case: adding one is a write of the case (invariant 9), so
    // the interface and agents see it through the revision.
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let (_, in_trash) = project_and_trash(&tx, note_id)?
        .ok_or_else(|| crate::tr!("notes.notFound", id = note_id))?;
    if in_trash {
        return Err(crate::tr!("data.trashedCannotAnnotate"));
    }
    let id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO case_provenance (id, note_id, kind, label, url, excerpt, agent, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![id, note_id, kind, label, url, excerpt, agent],
    )
    .map_err(crate::db::db_error)?;
    queries::bump_revision(&tx, note_id)?;
    tx.commit().map_err(crate::db::db_error)?;
    list_provenance(conn, note_id)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "provenance not found after insert".to_string())
}

/// Provenance of a case, oldest first.
pub fn list_provenance(conn: &Connection, note_id: &str) -> Result<Vec<CaseProvenance>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, note_id, kind, label, url, excerpt, agent, created_at
               FROM case_provenance WHERE note_id = ?1 ORDER BY created_at, rowid",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([note_id], |r| {
            Ok(CaseProvenance {
                id: r.get(0)?,
                note_id: r.get(1)?,
                kind: r.get(2)?,
                label: r.get(3)?,
                url: r.get(4)?,
                excerpt: r.get(5)?,
                agent: r.get(6)?,
                created_at: r.get(7)?,
            })
        })
        .map_err(crate::db::db_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::db::db_error);
    rows
}

/// Position of a case on its parent's board (the top-level board for a top-level case).
#[derive(Debug, serde::Serialize, Clone, PartialEq, serde::Deserialize)]
pub struct CasePosition {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

/// Largest coordinate accepted on a board, far beyond any screen.
const BOARD_LIMIT: f64 = 1_000_000.0;

/// Positions of the live cases placed on a board, in one project or in all of them. A case
/// never placed has no position: the interface lays it out until it is moved.
pub fn case_positions(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<Vec<CasePosition>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, canvas_x, canvas_y FROM notes
              WHERE deleted_at IS NULL AND canvas_x IS NOT NULL AND canvas_y IS NOT NULL
                AND (?1 IS NULL OR project_id = ?1)",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([project_id], |r| {
            Ok(CasePosition {
                id: r.get(0)?,
                x: r.get(1)?,
                y: r.get(2)?,
            })
        })
        .map_err(crate::db::db_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::db::db_error);
    rows
}

/// Places a case on its parent's board. A position is a write of the case: it advances its
/// revision (invariant 9) but not its modification date, which dates the content.
pub fn set_case_position(conn: &Connection, id: &str, x: f64, y: f64) -> Result<(), String> {
    set_case_positions(
        conn,
        &[CasePosition {
            id: id.to_string(),
            x,
            y,
        }],
    )
}

/// Places several cases at once, all or none: the board pins the cases that would otherwise
/// slide into the room left by cases put away elsewhere.
pub fn set_case_positions(conn: &Connection, positions: &[CasePosition]) -> Result<(), String> {
    for CasePosition { x, y, .. } in positions {
        if !(x.is_finite() && y.is_finite() && x.abs() <= BOARD_LIMIT && y.abs() <= BOARD_LIMIT) {
            return Err(format!(
                "position must be finite numbers within {BOARD_LIMIT}"
            ));
        }
    }
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    for CasePosition { id, x, y } in positions {
        let (_, in_trash) =
            project_and_trash(&tx, id)?.ok_or_else(|| crate::tr!("notes.notFound", id = id))?;
        if in_trash {
            return Err(crate::tr!("data.trashedCannotMove"));
        }
        tx.execute(
            "UPDATE notes SET canvas_x = ?2, canvas_y = ?3 WHERE id = ?1",
            params![id, x, y],
        )
        .map_err(crate::db::db_error)?;
        queries::bump_revision(&tx, id)?;
    }
    tx.commit().map_err(crate::db::db_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{activities, schema};
    use crate::models::note::NoteType;
    use crate::models::project::ProjectSpace;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        schema::apply_migrations(&conn).unwrap();
        conn
    }

    fn input(project: &str, title: &str) -> CreateNoteInput {
        CreateNoteInput {
            project_id: project.into(),
            title: title.into(),
            content: None,
            note_type: NoteType::Note,
            priority: None,
            git_branch: None,
            tags: None,
            due_at: None,
            raw_content: None,
        }
    }

    fn case(conn: &Connection, project: &str, title: &str) -> String {
        queries::create_note(conn, input(project, title)).unwrap()
    }

    fn sub(conn: &Connection, parent: &str, title: &str, kind: ContentKind) -> String {
        create_sub_case(conn, parent, kind, input("", title)).unwrap()
    }

    fn content_only(content: &str) -> crate::models::note::UpdateNoteInput {
        crate::models::note::UpdateNoteInput {
            status: None,
            content: Some(content.into()),
            resolution_comment: None,
            title: None,
            note_type: None,
            raw_content: None,
            priority: None,
            git_branch: None,
            tags: None,
            due_at: None,
        }
    }

    #[test]
    fn a_sketch_keeps_a_drawing_whatever_writes_it() {
        let conn = db();
        let p = queries::create_project(&conn, "Project", None).unwrap();
        let root = case(&conn, &p, "Home page");
        let sketch = sub(&conn, &root, "Layout", ContentKind::Sketch);
        let created = queries::get_note_by_id(&conn, &sketch).unwrap();
        assert_eq!(
            created.content, EMPTY_SKETCH,
            "a new sketch starts as an empty scene"
        );

        // Text written in place of the drawing (an editor, an agent) is refused, nothing changes.
        assert!(queries::update_note(
            &conn,
            &sketch,
            content_only(
                "# Layout

A header"
            )
        )
        .is_err());
        let huge = format!(
            r#"{{"type":"excalidraw","elements":[],"files":{{"f":"{}"}}}}"#,
            "a".repeat(MAX_SKETCH_BYTES)
        );
        assert!(queries::update_note(&conn, &sketch, content_only(&huge)).is_err());
        assert!(crate::db::structure::apply_structured_note(
            &conn,
            &sketch,
            EMPTY_SKETCH,
            "### Layout"
        )
        .is_err());
        let unchanged = queries::get_note_by_id(&conn, &sketch).unwrap();
        assert_eq!(
            (unchanged.content.as_str(), unchanged.revision),
            (EMPTY_SKETCH, created.revision)
        );

        // A drawing replaces a drawing.
        let drawn = r#"{"type":"excalidraw","version":2,"elements":[{"id":"a","type":"rectangle"}],"appState":{},"files":{}}"#;
        queries::update_note(&conn, &sketch, content_only(drawn)).unwrap();
        let saved = queries::get_note_by_id(&conn, &sketch).unwrap();
        assert_eq!(saved.content, drawn);
        assert!(saved.revision > created.revision);

        // A sketch created with a content needs a drawing; other kinds take any text.
        let mut with_text = input("", "Not a drawing");
        with_text.content = Some("just words".into());
        assert!(create_sub_case(&conn, &root, ContentKind::Sketch, with_text.clone()).is_err());
        let link = create_sub_case(&conn, &root, ContentKind::Link, with_text).unwrap();
        queries::update_note(&conn, &link, content_only("<https://example.com>")).unwrap();
    }

    fn parent_of(conn: &Connection, id: &str) -> Option<String> {
        queries::get_note_by_id(conn, id).unwrap().parent_id
    }

    fn titles(sub_cases: &[SubCase]) -> Vec<(i64, String)> {
        sub_cases
            .iter()
            .map(|s| (s.depth, s.note.title.clone()))
            .collect()
    }

    #[test]
    fn sub_cases_nest_without_limit_and_list_in_tree_order() {
        let conn = db();
        let p = queries::create_project(&conn, "Project", None).unwrap();
        let root = case(&conn, &p, "Root");
        let photo = sub(&conn, &root, "Photo", ContentKind::Image);
        let detail = sub(&conn, &photo, "Detail", ContentKind::Text);
        let deeper = sub(&conn, &detail, "Deeper", ContentKind::Sketch);
        // Siblings are ordered by creation: a later millisecond, or their ids decide at random.
        std::thread::sleep(std::time::Duration::from_millis(2));
        sub(&conn, &root, "Link", ContentKind::Link);

        let note = queries::get_note_by_id(&conn, &photo).unwrap();
        assert_eq!(
            note.project_id, p,
            "a sub-case lives in its parent's project"
        );
        assert_eq!(note.parent_id.as_deref(), Some(root.as_str()));
        assert_eq!(note.content_kind, ContentKind::Image);
        assert_eq!(
            queries::get_note_by_id(&conn, &root).unwrap().content_kind,
            ContentKind::Text
        );
        assert_eq!(
            titles(&list_sub_cases(&conn, &root, None).unwrap()),
            [
                (1, "Photo".to_string()),
                (2, "Detail".to_string()),
                (3, "Deeper".to_string()),
                (1, "Link".to_string()),
            ]
        );
        assert_eq!(
            titles(&list_sub_cases(&conn, &root, Some(1)).unwrap()),
            [(1, "Photo".to_string()), (1, "Link".to_string())]
        );
        let path: Vec<String> = case_path(&conn, &deeper)
            .unwrap()
            .into_iter()
            .map(|(_, title)| title)
            .collect();
        assert_eq!(path, ["Root", "Photo", "Detail"]);
        assert!(case_path(&conn, &root).unwrap().is_empty());
    }

    #[test]
    fn a_case_never_goes_inside_itself_or_its_sub_cases() {
        let conn = db();
        let p = queries::create_project(&conn, "Project", None).unwrap();
        let root = case(&conn, &p, "Root");
        let child = sub(&conn, &root, "Child", ContentKind::Text);
        let grandchild = sub(&conn, &child, "Grandchild", ContentKind::Text);
        let revision = queries::get_note_by_id(&conn, &root).unwrap().revision;

        for target in [&root, &child, &grandchild] {
            let error = move_case(&conn, &root, Some(target)).unwrap_err();
            assert_eq!(error, crate::tr!("data.caseCycle"));
        }
        assert_eq!(parent_of(&conn, &root), None);
        assert_eq!(
            queries::get_note_by_id(&conn, &root).unwrap().revision,
            revision,
            "a refused move writes nothing"
        );
        // The database itself refuses the cycle, whatever the caller.
        let direct = conn.execute(
            "UPDATE notes SET parent_id = ?1 WHERE id = ?2",
            params![grandchild, root],
        );
        assert!(
            direct
                .unwrap_err()
                .to_string()
                .contains("cannot go inside itself"),
            "trigger sub_case_no_cycle"
        );
    }

    #[test]
    fn moving_a_case_keeps_its_sub_cases_and_advances_its_revision_only() {
        let conn = db();
        let p = queries::create_project(&conn, "Project", None).unwrap();
        let first = case(&conn, &p, "First");
        let second = case(&conn, &p, "Second");
        let child = sub(&conn, &first, "Child", ContentKind::Text);
        let grandchild = sub(&conn, &child, "Grandchild", ContentKind::Text);
        let revision = |id: &str| queries::get_note_by_id(&conn, id).unwrap().revision;
        let (r_child, r_first, r_second) = (revision(&child), revision(&first), revision(&second));

        move_case(&conn, &child, Some(&second)).unwrap();
        assert_eq!(parent_of(&conn, &child).as_deref(), Some(second.as_str()));
        assert_eq!(
            parent_of(&conn, &grandchild).as_deref(),
            Some(child.as_str())
        );
        assert_eq!(revision(&child), r_child + 1);
        assert_eq!((revision(&first), revision(&second)), (r_first, r_second));

        move_case(&conn, &child, None).unwrap();
        assert_eq!(parent_of(&conn, &child), None, "back to top level");
        assert_eq!(revision(&child), r_child + 2);
        // Same place again: nothing to write.
        move_case(&conn, &child, None).unwrap();
        assert_eq!(revision(&child), r_child + 2);
    }

    #[test]
    fn a_sub_case_stays_in_its_parent_project() {
        let conn = db();
        let p = queries::create_project(&conn, "Project", None).unwrap();
        let other = queries::create_project(&conn, "Other", None).unwrap();
        let root = case(&conn, &p, "Root");
        let elsewhere = case(&conn, &other, "Elsewhere");

        let error =
            create_sub_case(&conn, &root, ContentKind::Text, input(&other, "Wrong")).unwrap_err();
        assert_eq!(error, crate::tr!("data.subCaseSameProject"));
        let error = move_case(&conn, &elsewhere, Some(&root)).unwrap_err();
        assert_eq!(error, crate::tr!("data.subCaseSameProject"));
        let error =
            create_sub_case(&conn, "missing", ContentKind::Text, input("", "Orphan")).unwrap_err();
        assert_eq!(error, crate::tr!("data.parentCaseNotFound", id = "missing"));

        // The database refuses a parent from another project or a missing parent.
        for (project, parent) in [(other.as_str(), root.as_str()), (p.as_str(), "missing")] {
            let direct = conn.execute(
                "INSERT INTO notes (id, project_id, title, content, note_type, status, parent_id)
                 VALUES ('x', ?1, 'x', '', 'note', 'open', ?2)",
                params![project, parent],
            );
            assert!(direct
                .unwrap_err()
                .to_string()
                .contains("must belong to its parent case project"));
        }
        let direct = conn.execute(
            "UPDATE notes SET parent_id = ?1 WHERE id = ?2",
            params![root, elsewhere],
        );
        assert!(direct
            .unwrap_err()
            .to_string()
            .contains("must belong to its parent case project"));
    }

    #[test]
    fn trash_keeps_the_tree_whole() {
        let conn = db();
        let p = queries::create_project(&conn, "Project", None).unwrap();
        let root = case(&conn, &p, "Root");
        let child = sub(&conn, &root, "Child", ContentKind::Text);
        let grandchild = sub(&conn, &child, "Grandchild", ContentKind::Text);
        let sibling = sub(&conn, &root, "Sibling", ContentKind::Text);

        // A sub-case trashed on its own stays in the trash when its parent comes back.
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert_eq!(queries::trash_note(&conn, &sibling).unwrap(), 1);
        std::thread::sleep(std::time::Duration::from_millis(2));
        let before = queries::get_note_by_id(&conn, &grandchild)
            .unwrap()
            .revision;
        assert_eq!(
            queries::trash_note(&conn, &root).unwrap(),
            3,
            "the case goes with its live sub-cases, at any depth"
        );
        let in_trash = |id: &str| queries::get_note_by_id(&conn, id).unwrap().deleted_at;
        assert!(in_trash(&child).is_some() && in_trash(&grandchild).is_some());
        assert_eq!(
            in_trash(&root),
            in_trash(&grandchild),
            "one instant for the tree"
        );
        assert_eq!(
            queries::get_note_by_id(&conn, &grandchild)
                .unwrap()
                .revision,
            before + 1
        );

        assert_eq!(
            queries::restore_from_trash(&conn, &root).unwrap(),
            3,
            "the tree comes back whole"
        );
        assert!(in_trash(&grandchild).is_none());
        assert!(in_trash(&sibling).is_some(), "trashed on its own before");
        assert_eq!(queries::trash_note(&conn, &root).unwrap(), 3);
        assert_eq!(
            move_case(&conn, &sibling, None).unwrap_err(),
            crate::tr!("data.trashedCannotMove")
        );
        let live = case(&conn, &p, "Live");
        assert_eq!(
            create_sub_case(&conn, &root, ContentKind::Text, input("", "x")).unwrap_err(),
            crate::tr!("data.trashedCannotReceiveSubCase")
        );
        assert_eq!(
            move_case(&conn, &live, Some(&root)).unwrap_err(),
            crate::tr!("data.trashedCannotReceiveSubCase")
        );
        assert_eq!(
            queries::restore_from_trash(&conn, &child).unwrap_err(),
            crate::tr!("data.restoreParentFirst")
        );

        // Purging the parent erases nothing else: its sub-cases stay in the trash, at top level.
        queries::purge_trash(&conn, queries::TrashPurge::One(&root)).unwrap();
        assert_eq!(parent_of(&conn, &child), None);
        assert_eq!(parent_of(&conn, &sibling), None);
        assert_eq!(
            parent_of(&conn, &grandchild).as_deref(),
            Some(child.as_str())
        );
        assert!(queries::get_note_by_id(&conn, &child)
            .unwrap()
            .deleted_at
            .is_some());
        // Its own sub-cases, trashed at the same instant, come back with it.
        assert_eq!(queries::restore_from_trash(&conn, &child).unwrap(), 2);
        assert_eq!(
            titles(&list_sub_cases(&conn, &child, None).unwrap()),
            [(1, "Grandchild".to_string())]
        );
    }

    #[test]
    fn a_position_is_a_write_of_the_case_but_not_a_content_change() {
        let conn = db();
        let p = queries::create_project(&conn, "Project", None).unwrap();
        let other = queries::create_project(&conn, "Other", None).unwrap();
        let root = case(&conn, &p, "Root");
        let child = sub(&conn, &root, "Child", ContentKind::Text);
        case(&conn, &other, "Elsewhere");
        let before = queries::get_note_by_id(&conn, &child).unwrap();
        assert!(
            case_positions(&conn, Some(&p)).unwrap().is_empty(),
            "never placed"
        );

        set_case_position(&conn, &child, 120.5, -40.0).unwrap();
        let after = queries::get_note_by_id(&conn, &child).unwrap();
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(
            after.updated_at, before.updated_at,
            "the content did not change"
        );
        assert_eq!(
            case_positions(&conn, Some(&p)).unwrap(),
            [CasePosition {
                id: child.clone(),
                x: 120.5,
                y: -40.0
            }]
        );
        assert!(case_positions(&conn, Some(&other)).unwrap().is_empty());
        assert_eq!(case_positions(&conn, None).unwrap().len(), 1);

        for (x, y) in [(f64::NAN, 0.0), (0.0, f64::INFINITY), (2e6, 0.0)] {
            assert!(set_case_position(&conn, &child, x, y).is_err());
        }
        assert!(set_case_position(&conn, "missing", 0.0, 0.0).is_err());
        assert_eq!(
            queries::get_note_by_id(&conn, &child).unwrap().revision,
            after.revision
        );

        queries::trash_note(&conn, &child).unwrap();
        assert_eq!(
            set_case_position(&conn, &child, 0.0, 0.0).unwrap_err(),
            crate::tr!("data.trashedCannotMove")
        );
        assert!(
            case_positions(&conn, Some(&p)).unwrap().is_empty(),
            "trash leaves the board"
        );
    }

    #[test]
    fn several_positions_are_written_together_or_not_at_all() {
        let conn = db();
        let p = queries::create_project(&conn, "Project", None).unwrap();
        let a = case(&conn, &p, "A");
        let b = case(&conn, &p, "B");
        let at = |id: &str, x: f64| CasePosition {
            id: id.to_string(),
            x,
            y: 24.0,
        };
        assert!(set_case_positions(&conn, &[at(&a, 24.0), at("missing", 296.0)]).is_err());
        assert!(
            case_positions(&conn, Some(&p)).unwrap().is_empty(),
            "a refused position leaves every case unplaced"
        );
        set_case_positions(&conn, &[at(&a, 24.0), at(&b, 296.0)]).unwrap();
        assert_eq!(case_positions(&conn, Some(&p)).unwrap().len(), 2);
    }

    #[test]
    fn a_moved_case_leaves_its_position_on_the_board_it_left() {
        let conn = db();
        let p = queries::create_project(&conn, "Project", None).unwrap();
        let group = case(&conn, &p, "Group");
        let loose = case(&conn, &p, "Loose");
        set_case_position(&conn, &loose, 300.0, 40.0).unwrap();
        set_case_position(&conn, &group, 0.0, 0.0).unwrap();

        move_case(&conn, &loose, Some(&group)).unwrap();
        let positions = case_positions(&conn, Some(&p)).unwrap();
        assert_eq!(positions.len(), 1, "only the group keeps its place");
        assert_eq!(positions[0].id, group);

        // Moving to the same parent changes nothing, the new place stays.
        set_case_position(&conn, &loose, 24.0, 24.0).unwrap();
        move_case(&conn, &loose, Some(&group)).unwrap();
        assert_eq!(case_positions(&conn, Some(&p)).unwrap().len(), 2);
    }

    #[test]
    fn a_provenance_is_a_write_of_its_case_and_stays_a_quote() {
        let conn = db();
        let p = queries::create_project(&conn, "Project", None).unwrap();
        let root = case(&conn, &p, "Root");
        let before = queries::get_note_by_id(&conn, &root).unwrap().revision;
        add_provenance(
            &conn,
            &root,
            "web",
            Some("Source"),
            None,
            None,
            Some("agent"),
        )
        .unwrap();
        assert_eq!(
            queries::get_note_by_id(&conn, &root).unwrap().revision,
            before + 1
        );
        let long = "x".repeat(PROVENANCE_EXCERPT_MAX + 1);
        assert!(
            add_provenance(&conn, &root, "web", None, None, Some(&long), None)
                .unwrap_err()
                .contains("excerpt")
        );
        assert_eq!(list_provenance(&conn, &root).unwrap().len(), 1);
    }

    #[test]
    fn provenance_is_append_only_and_leaves_with_its_case() {
        let conn = db();
        let p = queries::create_project(&conn, "Project", None).unwrap();
        let root = case(&conn, &p, "Root");
        let first = add_provenance(
            &conn,
            &root,
            "source",
            Some("Interview"),
            Some("https://example.com/a"),
            None,
            Some("claude"),
        )
        .unwrap();
        add_provenance(&conn, &root, "research", None, None, Some("excerpt"), None).unwrap();
        let listed = list_provenance(&conn, &root).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, first.id);
        assert_eq!(listed[0].agent.as_deref(), Some("claude"));
        assert_eq!(listed[1].agent, None);

        assert!(add_provenance(&conn, &root, " ", Some("x"), None, None, None).is_err());
        assert!(add_provenance(&conn, &root, "source", Some(" "), None, None, None).is_err());
        assert!(add_provenance(&conn, "missing", "source", Some("x"), None, None, None).is_err());

        let update = conn.execute("UPDATE case_provenance SET label = 'changed'", []);
        assert!(update.unwrap_err().to_string().contains("append-only"));
        let delete = conn.execute("DELETE FROM case_provenance", []);
        assert!(delete.unwrap_err().to_string().contains("append-only"));
        assert_eq!(list_provenance(&conn, &root).unwrap().len(), 2);

        queries::trash_note(&conn, &root).unwrap();
        assert_eq!(
            add_provenance(&conn, &root, "source", Some("x"), None, None, None).unwrap_err(),
            crate::tr!("data.trashedCannotAnnotate")
        );
        queries::purge_trash(&conn, queries::TrashPurge::One(&root)).unwrap();
        let left: i64 = conn
            .query_row("SELECT count(*) FROM case_provenance", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0, "provenance leaves with its purged case");
    }

    #[test]
    fn filing_from_the_inbox_into_a_case_is_all_or_nothing() {
        let conn = db();
        let inbox = activities::get_or_create_inbox(&conn, ProjectSpace::Work).unwrap();
        let target =
            queries::create_project_in_space(&conn, "Target", None, Some(ProjectSpace::Work))
                .unwrap();
        let other =
            queries::create_project_in_space(&conn, "Other", None, Some(ProjectSpace::Work))
                .unwrap();
        let captured = case(&conn, &inbox, "Captured");
        let elsewhere = case(&conn, &other, "Elsewhere");
        let trashed = case(&conn, &target, "Trashed");
        queries::trash_note(&conn, &trashed).unwrap();

        // The case refuses the note: it stays in the Inbox, not at the top of the project.
        for parent in [&elsewhere, &trashed] {
            assert!(activities::file_inbox_note(&conn, &captured, &target, Some(parent)).is_err());
            let note = queries::get_note_by_id(&conn, &captured).unwrap();
            assert_eq!(note.project_id, inbox);
            assert_eq!(note.parent_id, None);
        }

        let parent = case(&conn, &target, "Parent");
        activities::file_inbox_note(&conn, &captured, &target, Some(&parent)).unwrap();
        let note = queries::get_note_by_id(&conn, &captured).unwrap();
        assert_eq!(note.project_id, target);
        assert_eq!(note.parent_id.as_deref(), Some(parent.as_str()));
    }

    #[test]
    fn filing_from_the_inbox_moves_the_whole_tree() {
        let conn = db();
        let inbox = activities::get_or_create_inbox(&conn, ProjectSpace::Work).unwrap();
        let target =
            queries::create_project_in_space(&conn, "Target", None, Some(ProjectSpace::Work))
                .unwrap();
        let root = case(&conn, &inbox, "Captured");
        let child = sub(&conn, &root, "Child", ContentKind::Text);
        let grandchild = sub(&conn, &child, "Grandchild", ContentKind::Text);

        assert_eq!(
            activities::move_note_to_project(&conn, &child, &target).unwrap_err(),
            crate::tr!("data.subCaseFollowsParent")
        );
        activities::move_note_to_project(&conn, &root, &target).unwrap();
        for id in [&root, &child, &grandchild] {
            assert_eq!(
                queries::get_note_by_id(&conn, id).unwrap().project_id,
                target
            );
        }
        assert_eq!(
            parent_of(&conn, &grandchild).as_deref(),
            Some(child.as_str())
        );
    }

    #[test]
    fn deleting_a_project_moves_whole_trees_to_the_inbox() {
        let conn = db();
        let project =
            queries::create_project_in_space(&conn, "Doomed", None, Some(ProjectSpace::Work))
                .unwrap();
        let root = case(&conn, &project, "Root");
        let child = sub(&conn, &root, "Child", ContentKind::Text);
        queries::delete_project(&conn, &project).unwrap();
        let inbox = activities::get_or_create_inbox(&conn, ProjectSpace::Work).unwrap();
        for id in [&root, &child] {
            assert_eq!(
                queries::get_note_by_id(&conn, id).unwrap().project_id,
                inbox
            );
        }
        assert_eq!(parent_of(&conn, &child).as_deref(), Some(root.as_str()));
    }
}
