//! Import and export of cases in the JSON Canvas 1.0 format (jsoncanvas.org, MIT), an
//! open format read by other note-taking and whiteboard apps.
//!
//! JSON Canvas has no nesting: a case with sub-cases becomes a **group** holding its sub-cases
//! (and a text node for its own words), placed inside the group's rectangle; on import, a node
//! sits in the smallest group whose rectangle holds it. A text node keeps the case's title as a
//! first `#` heading. A sketch travels as a text node holding its scene in an `excalidraw` code
//! block, so a Humcase canvas comes back whole; a link case is a link node. Links between cases
//! (note → task) become edges; edges read on import are counted, not kept (Humcase has no free
//! arrows between cases).

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};

use crate::db::{cases, queries};
use crate::models::note::{is_sketch_scene, ContentKind, CreateNoteInput, NoteType};

/// Card size on the board (`BOARD_CARD_WIDTH` and the slot height of the interface).
const CARD_WIDTH: f64 = 240.0;
const CARD_HEIGHT: f64 = 204.0;
const SLOT_WIDTH: f64 = 272.0;
const SLOT_HEIGHT: f64 = 220.0;
const MARGIN: f64 = 24.0;
/// Room above the cases of a group, for its label.
const GROUP_HEADER: f64 = 48.0;
/// Limits of an imported canvas: beyond them the file is refused, nothing is written.
const MAX_CANVAS_BYTES: usize = 20 * 1024 * 1024;
const MAX_NODES: usize = 5_000;
const MAX_TITLE_CHARS: usize = 240;

/// What an import created.
#[derive(Debug, serde::Serialize, PartialEq)]
pub struct Imported {
    /// Identifiers of the cases created, parents before their sub-cases.
    pub cases: Vec<String>,
    /// Edges of the file, not kept.
    pub ignored_edges: usize,
}

struct Case {
    id: String,
    title: String,
    content: String,
    kind: ContentKind,
    position: Option<(f64, f64)>,
}

/// Live cases on the board of `parent` (the top-level board of `project_id` when `None`), with
/// their positions, oldest first like the board.
fn board(conn: &Connection, project_id: &str, parent: Option<&str>) -> Result<Vec<Case>, String> {
    let mut statement = conn
        .prepare(
            "SELECT id, title, content, content_kind, canvas_x, canvas_y FROM notes
             WHERE project_id = ?1 AND parent_id IS ?2 AND deleted_at IS NULL
             ORDER BY created_at, id",
        )
        .map_err(crate::db::db_error)?;
    let rows = statement
        .query_map(params![project_id, parent], |row| {
            let x: Option<f64> = row.get(4)?;
            let y: Option<f64> = row.get(5)?;
            Ok(Case {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                kind: row.get(3)?,
                position: x.zip(y),
            })
        })
        .map_err(crate::db::db_error)?;
    rows.collect::<Result<_, _>>().map_err(crate::db::db_error)
}

/// Text node of a case: its title as a heading, then its words; a sketch keeps its scene.
fn case_text(case: &Case) -> String {
    let title = if case.title.trim().is_empty() {
        String::new()
    } else {
        format!("# {}\n\n", case.title.trim())
    };
    match case.kind {
        ContentKind::Sketch => format!("{title}```excalidraw\n{}\n```\n", case.content.trim()),
        _ => format!("{title}{}", case.content),
    }
}

/// Places the cases of one board: stored positions as they are, the others in free slots of a
/// four-column grid below them. Returns `(case, x, y)` relative to the board's top-left corner.
fn placed(cases: Vec<Case>) -> Vec<(Case, f64, f64)> {
    let bottom = cases
        .iter()
        .filter_map(|case| case.position.map(|(_, y)| y + SLOT_HEIGHT))
        .fold(0.0_f64, f64::max);
    let mut next = 0usize;
    cases
        .into_iter()
        .map(|case| match case.position {
            Some((x, y)) => (case, x, y),
            None => {
                let (column, row) = (next % 4, next / 4);
                next += 1;
                let (x, y) = (
                    MARGIN + column as f64 * SLOT_WIDTH,
                    bottom.max(MARGIN) + row as f64 * SLOT_HEIGHT,
                );
                (case, x, y)
            }
        })
        .collect()
}

/// Nodes of a case placed at `(x, y)`; returns its width and height.
fn export_case(
    conn: &Connection,
    project_id: &str,
    case: &Case,
    (x, y): (f64, f64),
    nodes: &mut Vec<Value>,
    depth: usize,
) -> Result<(f64, f64), String> {
    let children = if depth < 64 {
        board(conn, project_id, Some(&case.id))?
    } else {
        Vec::new()
    };
    if children.is_empty() {
        let node = match (case.kind, crate::db::json_canvas::first_link(&case.content)) {
            (ContentKind::Link, Some(url)) => json!({
                "id": case.id, "type": "link", "url": url,
                "x": px(x), "y": px(y), "width": px(CARD_WIDTH), "height": px(CARD_HEIGHT),
            }),
            _ => json!({
                "id": case.id, "type": "text", "text": case_text(case),
                "x": px(x), "y": px(y), "width": px(CARD_WIDTH), "height": px(CARD_HEIGHT),
            }),
        };
        nodes.push(node);
        return Ok((CARD_WIDTH, CARD_HEIGHT));
    }
    // The group comes first: in JSON Canvas, later nodes are drawn above earlier ones.
    let group_index = nodes.len();
    nodes.push(Value::Null);
    let inner = (x + MARGIN, y + GROUP_HEADER);
    let mut offset_y = 0.0;
    if !case.content.trim().is_empty() {
        nodes.push(json!({
            "id": format!("{}-text", case.id), "type": "text", "text": case.content,
            "x": px(inner.0), "y": px(inner.1), "width": px(CARD_WIDTH), "height": px(CARD_HEIGHT),
        }));
        offset_y = SLOT_HEIGHT;
    }
    let laid = placed(children);
    let (min_x, min_y) = laid.iter().fold((f64::MAX, f64::MAX), |(a, b), (_, x, y)| {
        (a.min(*x), b.min(*y))
    });
    let (mut right, mut bottom) = (CARD_WIDTH, offset_y);
    for (child, cx, cy) in &laid {
        let at = (inner.0 + cx - min_x, inner.1 + offset_y + cy - min_y);
        let (w, h) = export_case(conn, project_id, child, at, nodes, depth + 1)?;
        right = right.max(at.0 - inner.0 + w);
        bottom = bottom.max(at.1 - inner.1 + h);
    }
    let (width, height) = (right + 2.0 * MARGIN, bottom + GROUP_HEADER + MARGIN);
    nodes[group_index] = json!({
        "id": case.id, "type": "group", "label": case.title,
        "x": px(x), "y": px(y), "width": px(width), "height": px(height),
    });
    Ok((width, height))
}

/// JSON Canvas positions and sizes are integers.
fn px(value: f64) -> i64 {
    value.round() as i64
}

/// First web address of a text: an autolink, a Markdown link or a bare address.
pub fn first_link(text: &str) -> Option<String> {
    let web = |candidate: &str| {
        let lower = candidate.to_ascii_lowercase();
        (lower.starts_with("https://") || lower.starts_with("http://"))
            .then(|| candidate.to_string())
    };
    for (index, _) in text.match_indices("http") {
        let rest = &text[index..];
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | ')' | ']' | '"'))
            .unwrap_or(rest.len());
        if let Some(url) = web(&rest[..end]) {
            return Some(url);
        }
    }
    None
}

/// The board of `root` (the top-level board of `project_id` when `None`) as a JSON Canvas
/// document: every live case and sub-case, and the note → task links between them as edges.
pub fn export_canvas(
    conn: &Connection,
    project_id: &str,
    root: Option<&str>,
) -> Result<Value, String> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
            params![project_id],
            |row| row.get(0),
        )
        .map_err(crate::db::db_error)?;
    if !exists {
        return Err(crate::tr!("projects.notFound", id = project_id));
    }
    if let Some(root) = root {
        let in_project: Option<String> = conn
            .query_row(
                "SELECT project_id FROM notes WHERE id = ?1 AND deleted_at IS NULL",
                params![root],
                |row| row.get(0),
            )
            .optional()
            .map_err(crate::db::db_error)?;
        if in_project.as_deref() != Some(project_id) {
            return Err(crate::tr!("notes.notFound", id = root));
        }
    }
    let mut nodes = Vec::new();
    let laid = placed(board(conn, project_id, root)?);
    for (case, x, y) in &laid {
        export_case(conn, project_id, case, (*x, *y), &mut nodes, 0)?;
    }
    let ids: HashSet<&str> = nodes
        .iter()
        .filter_map(|node| node["id"].as_str())
        .collect();
    let mut statement = conn
        .prepare("SELECT source_note_id, task_note_id FROM note_task_links ORDER BY source_note_id, task_note_id")
        .map_err(crate::db::db_error)?;
    let links = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(crate::db::db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(crate::db::db_error)?;
    let edges: Vec<Value> = links
        .into_iter()
        .filter(|(from, to)| ids.contains(from.as_str()) && ids.contains(to.as_str()))
        .map(|(from, to)| json!({ "id": format!("{from}-{to}"), "fromNode": from, "toNode": to, "toEnd": "arrow" }))
        .collect();
    Ok(json!({ "nodes": nodes, "edges": edges }))
}

struct Node {
    id: String,
    kind: String,
    rect: (f64, f64, f64, f64),
    text: Option<String>,
    url: Option<String>,
    file: Option<String>,
    label: Option<String>,
}

fn read_nodes(document: &Value) -> Result<Vec<Node>, String> {
    let nodes = match document.get("nodes") {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(nodes)) => nodes,
        Some(_) => return Err(crate::tr!("canvas.nodesNotArray")),
    };
    if nodes.len() > MAX_NODES {
        return Err(crate::tr!(
            "canvas.tooManyNodes",
            found = nodes.len(),
            max = MAX_NODES
        ));
    }
    let mut seen = HashSet::new();
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let node: &Map<String, Value> = node
                .as_object()
                .ok_or_else(|| crate::tr!("canvas.nodeNotObject", index = index))?;
            let text = |name: &str| node.get(name).and_then(Value::as_str).map(str::to_string);
            let number = |name: &str| {
                node.get(name)
                    .and_then(Value::as_f64)
                    .filter(|value| value.is_finite() && value.abs() <= 1_000_000.0)
                    .ok_or_else(|| crate::tr!("canvas.nodeNotNumber", index = index, field = name))
            };
            let id = text("id").ok_or_else(|| crate::tr!("canvas.nodeIdMissing", index = index))?;
            if !seen.insert(id.clone()) {
                return Err(crate::tr!("canvas.nodeDuplicateId", index = index, id = id));
            }
            Ok(Node {
                kind: text("type")
                    .ok_or_else(|| crate::tr!("canvas.nodeTypeMissing", index = index))?,
                rect: (
                    number("x")?,
                    number("y")?,
                    number("width")?.max(0.0),
                    number("height")?.max(0.0),
                ),
                text: text("text"),
                url: text("url"),
                file: text("file"),
                label: text("label"),
                id,
            })
        })
        .collect()
}

/// Smallest group holding each node (index), if any: the nesting JSON Canvas only draws.
fn containers(nodes: &[Node]) -> Vec<Option<usize>> {
    let area = |(_, _, w, h): (f64, f64, f64, f64)| w * h;
    let holds = |outer: (f64, f64, f64, f64), inner: (f64, f64, f64, f64)| {
        inner.0 >= outer.0
            && inner.1 >= outer.1
            && inner.0 + inner.2 <= outer.0 + outer.2
            && inner.1 + inner.3 <= outer.1 + outer.3
    };
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            nodes
                .iter()
                .enumerate()
                .filter(|(other, group)| {
                    *other != index
                        && group.kind == "group"
                        && holds(group.rect, node.rect)
                        // Strictly larger, or equal and earlier: never two groups holding each other.
                        && (area(group.rect) > area(node.rect)
                            || (area(group.rect) == area(node.rect) && *other < index))
                })
                .min_by(|(_, a), (_, b)| area(a.rect).total_cmp(&area(b.rect)))
                .map(|(other, _)| other)
        })
        .collect()
}

/// Title and content of a text node: its first line (without `#`) and the rest.
fn split_text(text: &str) -> (String, String) {
    let trimmed = text.trim_start();
    let (first, rest) = trimmed.split_once('\n').unwrap_or((trimmed, ""));
    let title: String = first
        .trim()
        .trim_start_matches('#')
        .trim()
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect();
    (title, rest.trim_start_matches(['\r', '\n']).to_string())
}

/// The scene of a text that is only an `excalidraw` code block, if it is a drawing.
fn sketch_of(content: &str) -> Option<String> {
    let inner = content
        .trim()
        .strip_prefix("```excalidraw")?
        .strip_suffix("```")?
        .trim();
    is_sketch_scene(inner).then(|| inner.to_string())
}

fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.trim_start_matches("www.").to_string()
}

/// Creates the cases of a JSON Canvas document, all or none, on the board of `parent_id` (inside
/// that case) or on the top-level board of `project_id`. Groups become cases holding the nodes
/// drawn inside them; positions are kept relative to their board.
pub fn import_canvas(
    conn: &Connection,
    project_id: Option<&str>,
    parent_id: Option<&str>,
    canvas: &str,
) -> Result<Imported, String> {
    if canvas.len() > MAX_CANVAS_BYTES {
        return Err(crate::tr!("canvas.tooLarge"));
    }
    let document: Value =
        serde_json::from_str(canvas).map_err(|e| crate::tr!("canvas.unreadable", error = e))?;
    if !document.is_object() {
        return Err(crate::tr!("canvas.notObject"));
    }
    let nodes = read_nodes(&document)?;
    let ignored_edges = document
        .get("edges")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let parents = containers(&nodes);
    // The words of a case exported as a group (`<group id>-text`, inside it) go back into it.
    let own_text: HashMap<usize, usize> = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            let group = parents[index]?;
            (node.kind == "text" && node.id == format!("{}-text", nodes[group].id))
                .then_some((group, index))
        })
        .collect();
    let merged: HashSet<usize> = own_text.values().copied().collect();

    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let project = match parent_id {
        Some(parent) => {
            let (project, in_trash): (String, bool) = tx
                .query_row(
                    "SELECT project_id, deleted_at IS NOT NULL FROM notes WHERE id = ?1",
                    params![parent],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(crate::db::db_error)?
                .ok_or_else(|| crate::tr!("data.parentCaseNotFound", id = parent))?;
            if in_trash {
                return Err(crate::tr!("data.trashedCannotReceiveSubCase"));
            }
            if project_id.is_some_and(|id| id != project) {
                return Err(crate::tr!("data.subCaseSameProject"));
            }
            project
        }
        None => {
            let id = project_id.ok_or("project_id or parent_id is required")?;
            let exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
                    params![id],
                    |row| row.get(0),
                )
                .map_err(crate::db::db_error)?;
            if !exists {
                return Err(crate::tr!("projects.notFound", id = id));
            }
            id.to_string()
        }
    };

    // Parents before their nodes: a node's depth is the number of groups around it.
    let depth_of = |mut index: usize| {
        let mut depth = 0;
        while let Some(parent) = parents[index] {
            depth += 1;
            index = parent;
        }
        depth
    };
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|&index| {
        (
            depth_of(index),
            nodes[index].rect.1 as i64,
            nodes[index].rect.0 as i64,
        )
    });
    // Each board starts at the margin: the smallest x and y of the nodes sharing a parent.
    let mut origins: HashMap<Option<usize>, (f64, f64)> = HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        let origin = origins
            .entry(parents[index])
            .or_insert((f64::MAX, f64::MAX));
        *origin = (origin.0.min(node.rect.0), origin.1.min(node.rect.1));
    }

    // On a board that already holds placed cases, the imported ones arrive below them.
    let below: Option<f64> = tx
        .query_row(
            "SELECT MAX(canvas_y) FROM notes WHERE project_id = ?1 AND parent_id IS ?2 AND deleted_at IS NULL",
            params![project, parent_id],
            |row| row.get(0),
        )
        .map_err(crate::db::db_error)?;
    let top_shift = below.map_or(0.0, |y| y + SLOT_HEIGHT - MARGIN);
    let mut created: HashMap<usize, String> = HashMap::new();
    let mut cases_created = Vec::new();
    for index in order {
        if merged.contains(&index) {
            continue;
        }
        let node = &nodes[index];
        let parent = match parents[index] {
            Some(group) => Some(created[&group].clone()),
            None => parent_id.map(str::to_string),
        };
        let (title, content, kind) = match node.kind.as_str() {
            "group" => {
                let words = own_text
                    .get(&index)
                    .and_then(|&text| nodes[text].text.clone());
                (
                    node.label.clone().unwrap_or_default(),
                    words.unwrap_or_default(),
                    ContentKind::Text,
                )
            }
            "link" => {
                let url = node.url.clone().unwrap_or_default();
                // A link case lives inside a case; at the top level, a text case holds it.
                let kind = if parent.is_some() {
                    ContentKind::Link
                } else {
                    ContentKind::Text
                };
                (host_of(&url), format!("<{url}>"), kind)
            }
            "file" => {
                let file = node.file.clone().unwrap_or_default();
                let name = file.rsplit(['/', '\\']).next().unwrap_or(&file).to_string();
                (name, format!("`{file}`"), ContentKind::Text)
            }
            _ => {
                let (title, rest) = split_text(node.text.as_deref().unwrap_or(""));
                match sketch_of(&rest) {
                    Some(scene) if parent.is_some() => (title, scene, ContentKind::Sketch),
                    _ => (title, rest, ContentKind::Text),
                }
            }
        };
        let title: String = title.chars().take(MAX_TITLE_CHARS).collect();
        let input = CreateNoteInput {
            project_id: project.clone(),
            title,
            content: Some(content.clone()),
            note_type: NoteType::Note,
            priority: None,
            git_branch: None,
            tags: None,
            due_at: None,
            raw_content: None,
        };
        cases::check_content(kind, &content)?;
        let id = queries::create_note(&tx, input)?;
        let origin = origins[&parents[index]];
        tx.execute(
            "UPDATE notes SET parent_id = ?2, content_kind = ?3, canvas_x = ?4, canvas_y = ?5 WHERE id = ?1",
            params![
                id,
                parent,
                kind,
                (MARGIN + node.rect.0 - origin.0).round(),
                (MARGIN + node.rect.1 - origin.1 + if parents[index].is_none() { top_shift } else { 0.0 }).round()
            ],
        )
        .map_err(crate::db::db_error)?;
        created.insert(index, id.clone());
        cases_created.push(id);
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(Imported {
        cases: cases_created,
        ignored_edges,
    })
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

    fn titles_under(conn: &Connection, parent: Option<&str>) -> Vec<(String, String)> {
        let mut statement = conn
            .prepare("SELECT title, content_kind FROM notes WHERE parent_id IS ?1 ORDER BY title")
            .unwrap();
        statement
            .query_map(params![parent], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn an_obsidian_canvas_comes_in_with_its_groups_as_cases() {
        let conn = db();
        let project = queries::create_project(&conn, "Site", None).unwrap();
        let canvas = r##"{
          "nodes": [
            {"id":"g","type":"group","label":"Home page","x":0,"y":0,"width":800,"height":500},
            {"id":"t","type":"text","text":"# Promise\n\nOne sentence.","x":40,"y":60,"width":250,"height":120},
            {"id":"l","type":"link","url":"https://www.example.com/a","x":340,"y":60,"width":250,"height":120},
            {"id":"f","type":"file","file":"Assets/hero.png","x":1000,"y":0,"width":250,"height":120},
            {"id":"loose","type":"text","text":"Loose idea","x":1000,"y":300,"width":250,"height":60}
          ],
          "edges": [{"id":"e","fromNode":"t","toNode":"l"}]
        }"##;
        let imported = import_canvas(&conn, Some(&project), None, canvas).unwrap();
        assert_eq!((imported.cases.len(), imported.ignored_edges), (5, 1));
        assert_eq!(
            titles_under(&conn, None),
            [
                ("Home page".into(), "text".into()),
                ("Loose idea".into(), "text".into()),
                ("hero.png".into(), "text".into())
            ]
        );
        let group: String = conn
            .query_row("SELECT id FROM notes WHERE title = 'Home page'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            titles_under(&conn, Some(&group)),
            [
                ("Promise".into(), "text".into()),
                ("example.com".into(), "link".into())
            ]
        );
        let (content, x, y): (String, f64, f64) = conn
            .query_row(
                "SELECT content, canvas_x, canvas_y FROM notes WHERE title = 'Promise'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((content.as_str(), x, y), ("One sentence.", 24.0, 24.0));

        // A second import lands below the cases already placed, never on them.
        let again = import_canvas(&conn, Some(&project), None, r#"{"nodes":[{"id":"n","type":"text","text":"Later","x":0,"y":0,"width":10,"height":10}]}"#).unwrap();
        let (x, y): (f64, f64) = conn
            .query_row(
                "SELECT canvas_x, canvas_y FROM notes WHERE id = ?1",
                params![again.cases[0]],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let lowest: f64 = conn
            .query_row(
                "SELECT MAX(canvas_y) FROM notes WHERE parent_id IS NULL AND id != ?1",
                params![again.cases[0]],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(x, 24.0);
        assert!(y >= lowest + 200.0, "{y} under {lowest}");
    }

    #[test]
    fn a_humcase_board_goes_out_and_comes_back_whole() {
        let conn = db();
        let project = queries::create_project(&conn, "Site", None).unwrap();
        let other = queries::create_project(&conn, "Copy", None).unwrap();
        let mut input = CreateNoteInput {
            project_id: project.clone(),
            title: "Home page".into(),
            content: Some("Rethink the top.".into()),
            note_type: NoteType::Idea,
            priority: None,
            git_branch: None,
            tags: None,
            due_at: None,
            raw_content: None,
        };
        let root = queries::create_note(&conn, input.clone()).unwrap();
        let scene = r#"{"type":"excalidraw","version":2,"elements":[{"id":"r","type":"rectangle"}],"appState":{},"files":{}}"#;
        input.project_id = String::new();
        input.title = "Layout".into();
        input.content = Some(scene.into());
        cases::create_sub_case(&conn, &root, ContentKind::Sketch, input.clone()).unwrap();
        input.title = "example.com".into();
        input.content = Some("<https://example.com/>".into());
        cases::create_sub_case(&conn, &root, ContentKind::Link, input).unwrap();

        let exported = export_canvas(&conn, &project, None).unwrap();
        let mut kinds: Vec<&str> = exported["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["type"].as_str().unwrap())
            .collect();
        kinds[1..].sort_unstable();
        assert_eq!(kinds, ["group", "link", "text", "text"]);

        import_canvas(&conn, Some(&other), None, &exported.to_string()).unwrap();
        let copy: String = conn
            .query_row(
                "SELECT id FROM notes WHERE project_id = ?1 AND parent_id IS NULL",
                params![other],
                |r| r.get(0),
            )
            .unwrap();
        let sketch: (String, String) = conn
            .query_row(
                "SELECT content, content_kind FROM notes WHERE parent_id = ?1 AND title = 'Layout'",
                params![copy],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(sketch, (scene.to_string(), "sketch".to_string()));
        let kinds = titles_under(&conn, Some(&copy));
        assert_eq!(
            kinds,
            [
                ("Layout".into(), "sketch".into()),
                ("example.com".into(), "link".into())
            ]
        );
        let words: String = conn
            .query_row(
                "SELECT content FROM notes WHERE id = ?1",
                params![copy],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(words, "Rethink the top.");
    }

    #[test]
    fn a_broken_canvas_writes_nothing() {
        let conn = db();
        let project = queries::create_project(&conn, "Site", None).unwrap();
        for canvas in [
            "not json",
            r#"{"nodes": {}}"#,
            r#"{"nodes": [{"id":"a","type":"text","x":0,"y":0,"width":10,"height":10},{"id":"b","type":"text","x":"far","y":0,"width":10,"height":10}]}"#,
            r#"{"nodes": [{"id":"a","type":"text","x":0,"y":0,"width":10,"height":10},{"id":"a","type":"text","x":0,"y":0,"width":10,"height":10}]}"#,
        ] {
            assert!(
                import_canvas(&conn, Some(&project), None, canvas).is_err(),
                "{canvas}"
            );
        }
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
