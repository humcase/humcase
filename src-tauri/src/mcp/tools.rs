use crate::dates::is_iso_day;
use crate::db::{cases, queries, relations};
use crate::models::note::{ContentKind, CreateNoteInput, NoteStatus, NoteType, UpdateNoteInput};
use crate::models::project::ProjectSpace;
use rusqlite::Connection;
use serde_json::{json, Value};

fn project_space_from_json(value: &Value) -> Result<Option<ProjectSpace>, String> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) if s == "private" => Ok(Some(ProjectSpace::Private)),
        Value::String(s) if s == "work" => Ok(Some(ProjectSpace::Work)),
        _ => Err("space must be private, work, or null".to_string()),
    }
}

/// True if `tags` is indeed an array of **strings** (the MCP schema announces it so; accepting
/// numbers would crash the interface, which calls `toLowerCase()` on every tag).
fn tags_from_json(value: Option<&Value>) -> Result<Option<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => {
            let mut tags = Vec::with_capacity(items.len());
            for item in items {
                match item.as_str() {
                    Some(s) => tags.push(s.to_string()),
                    None => {
                        return Err(
                            "tags must be an array of strings (non-text element received)"
                                .to_string(),
                        )
                    }
                }
            }
            serde_json::to_string(&tags)
                .map(Some)
                .map_err(|e| e.to_string())
        }
        Some(_) => Err("tags must be an array of strings".to_string()),
    }
}

/// Tool result carrying `value` as compact JSON text.
fn json_result(value: &Value) -> Value {
    json!({ "content": [{ "type": "text", "text": value.to_string() }] })
}

/// Tool result of a tool that declares an `outputSchema`: the typed `structuredContent`, and the
/// same JSON as text for clients that do not read it.
fn structured_result(value: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": value.to_string() }],
        "structuredContent": value
    })
}

/// Words of a title that say what it is about: 4 letters or more, lowercased.
fn title_words(title: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for word in title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4)
        .map(str::to_lowercase)
    {
        if !words.contains(&word) {
            words.push(word);
        }
    }
    words
}

/// Cases still to do in the same project whose title shares at least two words with the new
/// case's: a case created at the top level often repeats one the person dictated or another
/// session opened (05/10 sorting: work done in a new case left the original open).
fn existing_cases_like(conn: &Connection, id: &str, title: &str, project_id: &str) -> Vec<Value> {
    let words = title_words(title);
    if words.len() < 2 {
        return Vec::new();
    }
    let near = queries::find_similar_notes(conn, title, Some(id), None, queries::SIMILAR_MAX)
        .unwrap_or_default();
    near.into_iter()
        .filter(|case| {
            case.project_id == project_id
                && case.status != NoteStatus::Resolved
                && case.archived_at.is_none()
                && title_words(&case.title)
                    .iter()
                    .filter(|word| words.contains(word))
                    .count()
                    >= 2
        })
        .take(3)
        .map(|case| json!({ "id": case.id, "title": case.title, "status": case.status, "parent_id": case.parent_id }))
        .collect()
}

/// The parent of `note_id` when its last open sub-case was just resolved and the parent is not:
/// sessions resolved every step and left the case they belong to open (05/10 sorting).
fn parent_ready(conn: &Connection, note_id: &str) -> Option<Value> {
    conn.query_row(
        "SELECT p.id, p.title, p.status FROM notes c JOIN notes p ON p.id = c.parent_id
          WHERE c.id = ?1 AND p.deleted_at IS NULL AND p.status <> 'resolved'
            AND NOT EXISTS (SELECT 1 FROM notes s WHERE s.parent_id = p.id
                             AND s.deleted_at IS NULL AND s.archived_at IS NULL AND s.status <> 'resolved')",
        [note_id],
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "hint": "Every sub-case of this case is resolved. If its goal is reached, resolve it too with a resolution_comment; otherwise record a report saying what remains.",
            }))
        },
    )
    .ok()
}

fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

/// Optional string argument: absent or `null` is `None`, another type is refused.
fn optional_str<'a>(args: &'a Value, name: &str) -> Result<Option<&'a str>, String> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(format!("{name} must be a string")),
    }
}

fn optional_bool(args: &Value, name: &str) -> Result<Option<bool>, String> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(format!("{name} must be true or false")),
    }
}

/// Optional integer argument within `min..=max`.
fn optional_int(args: &Value, name: &str, min: i64, max: i64) -> Result<Option<i64>, String> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => match value.as_i64() {
            Some(n) if (min..=max).contains(&n) => Ok(Some(n)),
            _ => Err(format!("{name} must be an integer between {min} and {max}")),
        },
    }
}

/// `(offset, limit)` of a paged listing.
fn paging(args: &Value, default_limit: i64, max_limit: i64) -> Result<(i64, i64), String> {
    let limit = optional_int(args, "limit", 1, max_limit)?.unwrap_or(default_limit);
    let offset = optional_int(args, "offset", 0, i64::MAX)?.unwrap_or(0);
    Ok((offset, limit))
}

/// `response_format`: `concise` (default) or `detailed`.
fn detailed_format(args: &Value) -> Result<bool, String> {
    match optional_str(args, "response_format")? {
        None | Some("concise") => Ok(false),
        Some("detailed") => Ok(true),
        Some(other) => Err(format!(
            "invalid response_format: {other} (expected: concise|detailed)"
        )),
    }
}

/// Says how to see the rest of a truncated page; `None` when nothing is left out.
fn page_hint(shown: usize, total: i64, offset: i64) -> Option<String> {
    let next = offset + shown as i64;
    (next < total).then(|| {
        format!(
            "Showing {shown} of {total} from offset {offset}. Refine with search_query, project_id or filters, or call again with offset {next}."
        )
    })
}

/// A paged answer: `{total, offset, limit, <key>: items, hint?}`.
fn page_value(key: &str, items: Value, shown: usize, total: i64, offset: i64, limit: i64) -> Value {
    let mut value = json!({ "total": total, "offset": offset, "limit": limit });
    value[key] = items;
    if let Some(hint) = page_hint(shown, total, offset) {
        value["hint"] = json!(hint);
    }
    value
}

fn parse_status(value: &str) -> Result<NoteStatus, String> {
    match value {
        "open" => Ok(NoteStatus::Open),
        "in_progress" => Ok(NoteStatus::InProgress),
        "resolved" => Ok(NoteStatus::Resolved),
        other => Err(format!(
            "invalid status: {other} (expected: open|in_progress|resolved)"
        )),
    }
}

fn parse_note_type(value: &str) -> Result<NoteType, String> {
    NoteType::parse(value).ok_or_else(|| {
        format!(
            "invalid note_type: {value} (expected: {})",
            NoteType::expected()
        )
    })
}

fn space_filter(args: &Value) -> Result<Option<String>, String> {
    match optional_str(args, "space")? {
        None => Ok(None),
        Some(space @ ("private" | "work")) => Ok(Some(space.to_string())),
        Some(other) => Err(format!("invalid space: {other} (expected: private|work)")),
    }
}

/// Filters of `list_notes`, every value checked: an unknown status or type is an error, not
/// an empty list.
fn note_list_filter(args: &Value) -> Result<queries::NoteListFilter, String> {
    let day = |name: &str| -> Result<Option<String>, String> {
        match optional_str(args, name)? {
            None => Ok(None),
            Some(day) if is_iso_day(day.trim()) => Ok(Some(day.trim().to_string())),
            Some(day) => Err(format!("invalid {name}: {day} (expected YYYY-MM-DD)")),
        }
    };
    let note_types = match args.get("note_type") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::String(value)) => vec![parse_note_type(value)?],
        Some(Value::Array(values)) => values
            .iter()
            .map(|v| {
                v.as_str()
                    .ok_or_else(|| "note_type must be a string or an array of strings".to_string())
                    .and_then(parse_note_type)
            })
            .collect::<Result<_, _>>()?,
        Some(_) => return Err("note_type must be a string or an array of strings".to_string()),
    };
    let parent = match args.get("parent_id") {
        None => queries::ParentFilter::Any,
        Some(Value::Null) => queries::ParentFilter::TopLevel,
        Some(Value::String(id)) if id == "root" => queries::ParentFilter::TopLevel,
        Some(Value::String(id)) => queries::ParentFilter::Case(id.clone()),
        Some(_) => return Err("parent_id must be a case id, \"root\" or null".to_string()),
    };
    let updated_since = match optional_str(args, "updated_since")? {
        None => None,
        Some(instant) => {
            let instant = instant.trim();
            if !instant.get(..10).is_some_and(is_iso_day) {
                return Err(format!(
                    "invalid updated_since: {instant} (expected an ISO 8601 day or instant, e.g. 2026-10-01T08:00:00Z)"
                ));
            }
            Some(instant.to_string())
        }
    };
    Ok(queries::NoteListFilter {
        project_id: optional_str(args, "project_id")?.map(str::to_string),
        status: optional_str(args, "status")?
            .map(parse_status)
            .transpose()?,
        note_types,
        tag: optional_str(args, "tag")?
            .map(|t| t.trim().trim_start_matches('#').to_string())
            .filter(|t| !t.is_empty()),
        parent,
        due_before: day("due_before")?,
        due_after: day("due_after")?,
        updated_since,
        include_archived: optional_bool(args, "include_archived")?.unwrap_or(false),
        space: space_filter(args)?,
        search_query: optional_str(args, "search_query")?.map(str::to_string),
    })
}

/// Titles of cases by id (journal listings name the cases they speak of).
fn case_title(conn: &Connection, id: &str) -> Option<String> {
    conn.query_row("SELECT title FROM notes WHERE id = ?1", [id], |r| r.get(0))
        .ok()
}

/// A journal event as agents read it.
fn event_value(conn: &Connection, event: crate::models::note::CaseEvent, title: bool) -> Value {
    let mut value = json!({
        "kind": event.kind,
        "actor": event.actor_name.unwrap_or_else(|| event.actor_kind.clone()),
        "actor_kind": event.actor_kind,
        "fields": event.fields,
        "detail": event.detail,
        "at": event.last_at,
    });
    if title {
        value["note_id"] = json!(event.note_id);
        value["title"] = json!(case_title(conn, &event.note_id));
    }
    value
}

/// First characters of a report, on one line.
fn report_excerpt(report: Option<&str>) -> Option<String> {
    let text = report?.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    let mut excerpt: String = text.chars().take(300).collect();
    if excerpt.len() < text.len() {
        excerpt.push('…');
    }
    Some(excerpt)
}

/// A case waiting for a decision or for a rework, as `get_brief` lists it.
fn brief_case_value(conn: &Connection, case: &crate::models::note::BriefCase) -> Value {
    let note = queries::get_note_by_id(conn, &case.note_id).ok();
    let path: Vec<String> = cases::case_path(conn, &case.note_id)
        .unwrap_or_default()
        .into_iter()
        .map(|(_, title)| title)
        .collect();
    json!({
        "id": case.note_id,
        "title": note.as_ref().map(|n| n.title.clone()),
        "project_id": note.as_ref().map(|n| n.project_id.clone()),
        "path": (!path.is_empty()).then(|| path.join(" › ")),
        "status": note.as_ref().map(|n| n.status.as_str()),
        "revision": note.as_ref().map(|n| n.revision),
        "report": report_excerpt(note.as_ref().and_then(|n| n.resolution_comment.as_deref())),
        "rework_count": case.rework_count,
        "declared_by": case.declared.as_ref().map(|e| e.actor_name.clone().unwrap_or_else(|| e.actor_kind.clone())),
        "declared_at": case.declared.as_ref().map(|e| e.created_at.clone()),
        "last_decision": case.last_review,
    })
}

/// MCP speaks English to agents, whatever the interface language.
pub fn handle_tool_call(
    conn: &Connection,
    tool_name: &str,
    arguments: Option<Value>,
) -> Result<Value, String> {
    crate::i18n::with_language("en", || handle_tool_call_en(conn, tool_name, arguments))
}

fn handle_tool_call_en(
    conn: &Connection,
    tool_name: &str,
    arguments: Option<Value>,
) -> Result<Value, String> {
    let args = arguments.unwrap_or(json!({}));

    match tool_name {
        "list_projects" => {
            let projects = queries::get_projects(conn).map_err(|e| e.to_string())?;
            Ok(json_result(&to_value(&projects)?))
        }
        "create_project" => {
            let name = args
                .get("name")
                .and_then(Value::as_str)
                .ok_or("name is required")?;
            let color = optional_str(&args, "color")?;
            let space = args
                .get("space")
                .map(project_space_from_json)
                .transpose()?
                .flatten();
            let id = queries::create_project_in_space(conn, name, color, space)
                .map_err(|e| e.to_string())?;
            let project = queries::get_projects(conn)
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|p| p.id == id)
                .ok_or("project not found after creation")?;
            Ok(json_result(&json!({
                "id": project.id,
                "name": project.name,
                "color": project.color,
                "space": project.space,
            })))
        }
        "update_project" => {
            let id = args
                .get("project_id")
                .and_then(Value::as_str)
                .ok_or("project_id is required")?;
            let name = optional_str(&args, "name")?;
            let color = optional_str(&args, "color")?;
            if name.is_none() && color.is_none() {
                return Err("give a name or a color to change".to_string());
            }
            // The color is checked before the rename, so a refusal writes nothing.
            if let Some(color) = color {
                let valid = color.len() == 7
                    && color.starts_with('#')
                    && color[1..].chars().all(|c| c.is_ascii_hexdigit());
                if !valid {
                    return Err(format!("invalid project color: {color} (expected #rrggbb)"));
                }
            }
            if let Some(name) = name {
                queries::rename_project(conn, id, name)?;
            }
            if let Some(color) = color {
                queries::set_project_color(conn, id, color)?;
            }
            let project = queries::get_projects(conn)
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|p| p.id == id)
                .ok_or_else(|| format!("project not found: {id}"))?;
            Ok(json_result(&to_value(&project)?))
        }
        "update_project_space" => {
            let id = args
                .get("project_id")
                .and_then(Value::as_str)
                .ok_or("project_id is required")?;
            let space = project_space_from_json(args.get("space").ok_or("space is required")?)?;
            queries::update_project_space(conn, id, space)?;
            Ok(json_result(
                &json!({ "id": id, "space": space.map(|s| s.as_str()) }),
            ))
        }
        "list_activities" => {
            let activities = queries::get_activities(conn)?;
            Ok(json_result(&to_value(&activities)?))
        }
        "create_activity" => {
            let name = args
                .get("name")
                .and_then(Value::as_str)
                .ok_or("name is required")?;
            let space = project_space_from_json(args.get("space").ok_or("space is required")?)?
                .ok_or("space must be private or work")?;
            let activity = queries::create_activity(conn, name, space)?;
            Ok(json_result(&to_value(&activity)?))
        }
        "rename_activity" => {
            let id = args
                .get("activity_id")
                .and_then(Value::as_str)
                .ok_or("activity_id is required")?;
            let name = args
                .get("name")
                .and_then(Value::as_str)
                .ok_or("name is required")?;
            let activity = queries::rename_activity(conn, id, name)?;
            Ok(json_result(&to_value(&activity)?))
        }
        "delete_activity" => {
            let id = args
                .get("activity_id")
                .and_then(Value::as_str)
                .ok_or("activity_id is required")?;
            queries::delete_activity(conn, id)?;
            Ok(json_result(&json!({ "id": id, "deleted": true })))
        }
        "set_project_activities" => {
            let project_id = args
                .get("project_id")
                .and_then(Value::as_str)
                .ok_or("project_id is required")?;
            let ids = args
                .get("activity_ids")
                .and_then(Value::as_array)
                .ok_or("activity_ids must be an array")?;
            let ids = ids
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "activity_ids must contain only strings".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let expected = args
                .get("expected_activity_ids")
                .map(|value| {
                    let values = value
                        .as_array()
                        .ok_or("expected_activity_ids must be an array")?;
                    values
                        .iter()
                        .map(|v| {
                            v.as_str().map(str::to_owned).ok_or_else(|| {
                                "expected_activity_ids must contain only strings".to_string()
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;
            queries::set_project_activities(conn, project_id, &ids, expected.as_deref())?;
            Ok(json_result(
                &json!({ "project_id": project_id, "activity_ids": ids }),
            ))
        }
        "get_or_create_inbox" => {
            let space = project_space_from_json(args.get("space").ok_or("space is required")?)?
                .ok_or("space must be private or work")?;
            let id = queries::get_or_create_inbox(conn, space)?;
            Ok(json_result(
                &json!({ "id": id, "space": space.as_str(), "is_inbox": true }),
            ))
        }
        "move_note_to_project" => {
            let note_id = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            let project_id = args
                .get("project_id")
                .and_then(Value::as_str)
                .ok_or("project_id is required")?;
            queries::move_note_to_project(conn, note_id, project_id)?;
            let note = queries::get_note_by_id(conn, note_id).map_err(|e| e.to_string())?;
            Ok(json_result(&json!({
                "id": note.id, "project_id": note.project_id, "revision": note.revision
            })))
        }
        "list_notes" => {
            let filter = note_list_filter(&args)?;
            let (offset, limit) = paging(&args, 50, 200)?;
            let detailed = detailed_format(&args)?;
            let page = queries::list_notes_page(conn, &filter, offset, limit, detailed)?;
            let shown = page.notes.len();
            Ok(structured_result(page_value(
                "notes",
                to_value(&page.notes)?,
                shown,
                page.total,
                offset,
                limit,
            )))
        }
        "get_note" => {
            let note_id = args
                .get("note_id")
                .and_then(|v| v.as_str())
                .ok_or("note_id is required")?;
            let note = queries::get_note_by_id(conn, note_id).map_err(|e| e.to_string())?;
            // Case context (v22): path from the top-level case, direct sub-cases, provenance.
            let mut value = to_value(&note)?;
            let path: Vec<Value> = cases::case_path(conn, note_id)?
                .into_iter()
                .map(|(id, title)| json!({ "id": id, "title": title }))
                .collect();
            let sub_cases = cases::list_sub_cases(conn, note_id, Some(1))?.len();
            let provenance = cases::list_provenance(conn, note_id)?;
            value["case_path"] = json!(path);
            value["sub_case_count"] = json!(sub_cases);
            value["provenance"] = to_value(&provenance)?;
            // Decision on the result (v24): a status an agent set to resolved is only declared
            // until the person accepts it; a rework carries the person's reason.
            let review = crate::db::journal::review_state(conn, note_id)?;
            value["review"] = json!({
                "state": review.state,
                "rework_count": review.rework_count,
                "last_decision": review.last_review,
            });
            let activity: Vec<Value> = crate::db::journal::case_events(conn, note_id)?
                .into_iter()
                .take(10)
                .map(|e| {
                    json!({
                        "kind": e.kind,
                        "actor": e.actor_name.unwrap_or(e.actor_kind),
                        "fields": e.fields,
                        "detail": e.detail,
                        "at": e.last_at,
                    })
                })
                .collect();
            value["recent_activity"] = json!(activity);
            // A decision replaced by a newer one is history: read the newer one first.
            let history = crate::tidy::supersession(conn, note_id)?;
            value["superseded_by"] = match &history.superseded_by {
                Some(by) => json!({ "id": by.id, "title": by.title, "at": history.superseded_at }),
                None => Value::Null,
            };
            value["replaces"] = json!(history
                .replaces
                .iter()
                .map(|r| json!({ "id": r.id, "title": r.title }))
                .collect::<Vec<_>>());
            Ok(structured_result(value))
        }
        "find_similar_notes" => {
            let text = args
                .get("text")
                .and_then(Value::as_str)
                .filter(|t| !t.trim().is_empty())
                .ok_or("text is required")?;
            let exclude = optional_str(&args, "note_id")?;
            let limit = optional_int(&args, "limit", 1, queries::SIMILAR_MAX)?
                .unwrap_or(queries::SIMILAR_MAX);
            // Same space as the case compared against: a work case never suggests a private one.
            let space = match space_filter(&args)? {
                Some(space) => Some(space),
                None => match exclude {
                    Some(id) => queries::note_project_space(conn, id).map_err(|e| e.to_string())?,
                    None => None,
                },
            };
            let notes = queries::find_similar_notes(conn, text, exclude, space.as_deref(), limit)?;
            Ok(json_result(&json!({ "notes": notes })))
        }
        "find_by_meaning" => {
            let query = args
                .get("query")
                .and_then(Value::as_str)
                .filter(|t| !t.trim().is_empty())
                .ok_or("query is required")?;
            let limit = optional_int(&args, "limit", 1, 20)?.unwrap_or(10) as usize;
            let space = space_filter(&args)?;
            let data_dir = crate::data_location::data_dir()?;
            if !crate::meaning::enabled(&data_dir) {
                return Err("search by meaning is turned off in Humcase settings".into());
            }
            static SENSE: std::sync::OnceLock<crate::meaning::Sense> = std::sync::OnceLock::new();
            let sense = SENSE.get_or_init(Default::default);
            // More hits than asked: some may belong to another space or be in the trash.
            let hits = crate::meaning::search(sense, &data_dir, query, 100)?;
            if hits.is_empty() && !crate::meaning::has_vectors(&data_dir) {
                return Err(
                    "search by meaning is not ready: open Humcase so it reads the cases".into(),
                );
            }
            let ids: Vec<String> = hits.iter().map(|h| h.note_id.clone()).collect();
            let summaries = queries::note_summaries_by_ids(conn, &ids, space.as_deref())?;
            let notes: Vec<Value> = summaries
                .into_iter()
                .take(limit)
                .filter_map(|summary| {
                    let hit = hits.iter().find(|h| h.note_id == summary.id)?;
                    let mut value = serde_json::to_value(&summary).ok()?;
                    value["score"] = json!((hit.score * 1000.0).round() / 1000.0);
                    value["lead"] = json!((hit.lead * 100.0).round() / 100.0);
                    value["strong"] = json!(hit.strong);
                    value["very_close"] = json!(hit.very_close);
                    Some(value)
                })
                .collect();
            Ok(json_result(&json!({ "notes": notes })))
        }
        "list_nature_proposals" => {
            let limit = optional_int(&args, "limit", 1, 100)?.unwrap_or(20) as usize;
            let space = space_filter(&args)?;
            let data_dir = crate::data_location::data_dir()?;
            let review = crate::natures::review(conn, &data_dir, space.as_deref(), limit)?;
            Ok(json_result(
                &serde_json::to_value(review).map_err(|e| e.to_string())?,
            ))
        }
        "propose_sub_cases" => {
            let note_id = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            let steps: Vec<crate::germinate::Step> = serde_json::from_value(
                args.get("proposals")
                    .cloned()
                    .ok_or("proposals is required")?,
            )
            .map_err(|e| format!("proposals: {e} (each needs title, nature and passage)"))?;
            let (proposals, refused) = crate::germinate::propose_from_agent(conn, note_id, steps)?;
            Ok(json_result(
                &json!({ "proposals": proposals, "refused": refused }),
            ))
        }
        "list_proposals" => {
            let note_id = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            queries::get_note_by_id(conn, note_id)
                .map_err(|_| crate::tr!("notes.notFound", id = note_id))?;
            let proposals = crate::germinate::pending(conn, note_id)?;
            Ok(json_result(&json!({ "proposals": proposals })))
        }
        "list_tidy" => {
            let space = space_filter(&args)?;
            let items = crate::tidy::pending(conn, space.as_deref())?;
            Ok(json_result(&json!({ "items": items })))
        }
        "get_work_profile" => {
            let spaces = match space_filter(&args)? {
                Some(space) => vec![space],
                None => vec!["work".to_string(), "private".to_string()],
            };
            let mut profiles = Vec::new();
            let mut pending = Vec::new();
            for space in &spaces {
                profiles.push(crate::profile::read_profile(conn, space)?);
                pending.extend(crate::profile::pending(conn, Some(space))?);
            }
            Ok(json_result(
                &json!({ "profiles": profiles, "pending": pending }),
            ))
        }
        "propose_profile_rule" => {
            let text = args
                .get("text")
                .and_then(Value::as_str)
                .ok_or("text is required")?;
            let reason = optional_str(&args, "reason")?;
            let case_id = optional_str(&args, "case_id")?;
            let space = space_filter(&args)?;
            let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
            let result =
                crate::profile::propose_from_agent(&tx, text, reason, case_id, space.as_deref())?;
            tx.commit().map_err(crate::db::db_error)?;
            Ok(json_result(&result))
        }
        "get_project_summary" => {
            let project_id = args
                .get("project_id")
                .and_then(Value::as_str)
                .ok_or("project_id is required")?;
            let summary = crate::gather::project_summary(conn, project_id)?;
            Ok(json_result(&json!(summary)))
        }
        "propose_superseded" => {
            let older = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            let newer = args
                .get("by_id")
                .and_then(Value::as_str)
                .ok_or("by_id is required")?;
            let item = crate::tidy::propose_from_agent(conn, older, newer)?;
            Ok(json_result(&json!({ "item": item })))
        }
        "get_case_activity" => {
            let note_id = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            let limit = optional_int(&args, "limit", 1, 300)?.unwrap_or(20);
            let note = queries::get_note_by_id(conn, note_id).map_err(|e| e.to_string())?;
            let review = crate::db::journal::review_state(conn, note_id)?;
            let decisions = crate::db::journal::case_reviews(conn, note_id)?;
            let events = crate::db::journal::case_events(conn, note_id)?;
            let events_total = events.len();
            let events: Vec<Value> = events
                .into_iter()
                .take(limit as usize)
                .map(|e| event_value(conn, e, false))
                .collect();
            Ok(json_result(&json!({
                "note_id": note.id,
                "title": note.title,
                "review": {
                    "state": review.state,
                    "rework_count": review.rework_count,
                    "last_decision": review.last_review,
                },
                "decisions": decisions,
                "events": events,
                "events_total": events_total,
            })))
        }
        "get_brief" => {
            let since = optional_str(&args, "since")?.map(str::trim);
            if let Some(since) = since {
                if !since.get(..10).is_some_and(is_iso_day) {
                    return Err(format!(
                        "invalid since: {since} (expected an ISO 8601 instant, e.g. 2026-10-01T08:00:00Z)"
                    ));
                }
            }
            let limit = optional_int(&args, "limit", 1, 300)?.unwrap_or(50);
            // Reads only: nothing is marked as seen, the person's brief stays as it is.
            let brief = crate::db::journal::brief(conn)?;
            let (since_value, fallback, changes) = match since {
                Some(since) => {
                    let mut events = crate::db::journal::agent_events_since(conn, since)?;
                    events.reverse();
                    (since.to_string(), false, events)
                }
                None => (brief.since.clone(), brief.since_is_fallback, brief.changes),
            };
            let changes_total = changes.len();
            let shown: Vec<Value> = changes
                .into_iter()
                .take(limit as usize)
                .map(|e| event_value(conn, e, true))
                .collect();
            let mut value = json!({
                "since": since_value,
                "since_is_fallback": fallback,
                "to_review": brief.to_review.iter().map(|c| brief_case_value(conn, c)).collect::<Vec<_>>(),
                "reworks_pending": brief.reworks_pending.iter().map(|c| brief_case_value(conn, c)).collect::<Vec<_>>(),
                "changes_total": changes_total,
            });
            if shown.len() < changes_total {
                value["hint"] = json!(format!(
                    "Showing the {} most recent of {changes_total} changes; raise limit (at most 300) or pass a later since.",
                    shown.len()
                ));
            }
            value["changes"] = json!(shown);
            Ok(json_result(&value))
        }
        "structure_note" => {
            // Structuring rewrites the case: the same human authorization as `update_note`,
            // checked before anything else.
            if !crate::db::permissions::allow_note_edit(conn)? {
                return Err(
                    "agent note editing is disabled; ask the user to enable agent note editing in Humcase settings (structuring rewrites the case)"
                        .to_string(),
                );
            }
            let note_id = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            let expected_revision = args
                .get("expected_revision")
                .and_then(Value::as_i64)
                .ok_or("expected_revision is required (integer)")?;
            let line_of_work = match args.get("line_of_work") {
                None | Some(Value::Null) => "auto",
                Some(Value::String(value)) => value.as_str(),
                Some(_) => return Err("line_of_work must be a string".to_string()),
            };
            let provider = match args.get("provider") {
                None | Some(Value::Null) => None,
                Some(Value::String(value)) => Some(value.clone()),
                Some(_) => return Err("provider must be a string".to_string()),
            };
            let note = crate::commands::ai::structure_for_agent(
                conn,
                note_id,
                expected_revision,
                line_of_work,
                provider,
            )?;
            Ok(json_result(&to_value(&note)?))
        }
        "export_json_canvas" => {
            let project_id = args
                .get("project_id")
                .and_then(Value::as_str)
                .ok_or("project_id is required")?;
            let case_id = optional_str(&args, "case_id")?;
            let canvas = crate::db::json_canvas::export_canvas(conn, project_id, case_id)?;
            Ok(json_result(&to_value(&canvas)?))
        }
        "import_json_canvas" => {
            let (project_id, parent_id) = (
                optional_str(&args, "project_id")?,
                optional_str(&args, "parent_id")?,
            );
            let canvas = match args.get("canvas") {
                Some(Value::String(text)) => text.clone(),
                Some(document @ Value::Object(_)) => document.to_string(),
                _ => {
                    return Err("canvas is required (a JSON Canvas object or its text)".to_string())
                }
            };
            let imported =
                crate::db::json_canvas::import_canvas(conn, project_id, parent_id, &canvas)?;
            Ok(json_result(&to_value(&imported)?))
        }
        "set_case_position" => {
            let note_id = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            let coordinate = |name: &str| {
                args.get(name)
                    .and_then(Value::as_f64)
                    .ok_or(format!("{name} must be a number"))
            };
            let (x, y) = (coordinate("x")?, coordinate("y")?);
            cases::set_case_position(conn, note_id, x, y)?;
            let revision = queries::get_note_by_id(conn, note_id)
                .map_err(|e| e.to_string())?
                .revision;
            Ok(json_result(
                &json!({ "id": note_id, "x": x, "y": y, "revision": revision }),
            ))
        }
        "set_case_positions" => {
            let items = args
                .get("positions")
                .and_then(Value::as_array)
                .filter(|items| !items.is_empty())
                .ok_or("positions must be a non-empty array of {note_id, x, y}")?;
            if items.len() > 500 {
                return Err("positions holds at most 500 cases".to_string());
            }
            let positions = items
                .iter()
                .map(|item| {
                    let id = item
                        .get("note_id")
                        .and_then(Value::as_str)
                        .ok_or("every position needs a note_id")?;
                    let coordinate = |name: &str| {
                        item.get(name)
                            .and_then(Value::as_f64)
                            .ok_or(format!("{name} must be a number"))
                    };
                    Ok(cases::CasePosition {
                        id: id.to_string(),
                        x: coordinate("x")?,
                        y: coordinate("y")?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            cases::set_case_positions(conn, &positions)?;
            Ok(json_result(&json!({ "placed": positions.len() })))
        }
        "move_case" => {
            let note_id = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            // Omitted or null: back to top level. Any other non-string value is a mistake.
            let parent_id = match args.get("parent_id") {
                None | Some(Value::Null) => None,
                Some(Value::String(id)) => Some(id.as_str()),
                Some(_) => return Err("parent_id must be a string or null".to_string()),
            };
            cases::move_case(conn, note_id, parent_id)?;
            let note = queries::get_note_by_id(conn, note_id).map_err(|e| e.to_string())?;
            Ok(json_result(&json!({
                "id": note.id, "parent_id": note.parent_id, "revision": note.revision
            })))
        }
        "archive_case" => {
            let note_id = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            let archived = optional_bool(&args, "archived")?.unwrap_or(true);
            queries::set_note_archived(conn, note_id, archived)?;
            let note = queries::get_note_by_id(conn, note_id).map_err(|e| e.to_string())?;
            Ok(json_result(&json!({
                "id": note.id, "archived_at": note.archived_at, "revision": note.revision
            })))
        }
        "list_sub_cases" => {
            let note_id = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            let max_depth = match args.get("max_depth") {
                None | Some(Value::Null) => None,
                Some(value) => match value.as_i64() {
                    Some(depth) if depth >= 1 => Some(depth),
                    _ => return Err("max_depth must be an integer of at least 1".to_string()),
                },
            };
            let (offset, limit) = paging(&args, 100, 500)?;
            let detailed = detailed_format(&args)?;
            let sub_cases = cases::list_sub_cases(conn, note_id, max_depth)?;
            let total = sub_cases.len() as i64;
            let items: Vec<Value> = sub_cases
                .iter()
                .skip(offset as usize)
                .take(limit as usize)
                .map(|sub| {
                    let mut value =
                        to_value(&queries::NoteSummary::from_note(&sub.note, detailed))?;
                    value["depth"] = json!(sub.depth);
                    Ok(value)
                })
                .collect::<Result<_, String>>()?;
            let shown = items.len();
            Ok(json_result(&page_value(
                "sub_cases",
                json!(items),
                shown,
                total,
                offset,
                limit,
            )))
        }
        "add_provenance" => {
            let note_id = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            let kind = args
                .get("kind")
                .and_then(Value::as_str)
                .ok_or("kind is required")?;
            let provenance = cases::add_provenance(
                conn,
                note_id,
                kind,
                optional_str(&args, "label")?,
                optional_str(&args, "url")?,
                optional_str(&args, "excerpt")?,
                optional_str(&args, "agent")?,
            )?;
            Ok(json_result(&to_value(&provenance)?))
        }
        "get_note_relations" => {
            let note_id = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            let result = relations::get_note_relations(conn, note_id)?;
            Ok(json_result(&to_value(&result)?))
        }
        "create_linked_task" => {
            let source = args
                .get("source_note_id")
                .and_then(Value::as_str)
                .ok_or("source_note_id is required")?;
            let title = args
                .get("title")
                .and_then(Value::as_str)
                .ok_or("title is required")?;
            let content = args
                .get("content")
                .map(|v| v.as_str().ok_or("content must be a string"))
                .transpose()?;
            let id = relations::create_linked_task(conn, source, title, content)?;
            Ok(json_result(&json!({ "task_note_id": id })))
        }
        "link_note_task" => {
            let source = args
                .get("source_note_id")
                .and_then(Value::as_str)
                .ok_or("source_note_id is required")?;
            let task = args
                .get("task_note_id")
                .and_then(Value::as_str)
                .ok_or("task_note_id is required")?;
            relations::link_note_task(conn, source, task)?;
            Ok(json_result(
                &json!({ "source_note_id": source, "task_note_id": task, "linked": true }),
            ))
        }
        "unlink_note_task" => {
            let source = args
                .get("source_note_id")
                .and_then(Value::as_str)
                .ok_or("source_note_id is required")?;
            let task = args
                .get("task_note_id")
                .and_then(Value::as_str)
                .ok_or("task_note_id is required")?;
            relations::unlink_note_task(conn, source, task)?;
            Ok(json_result(
                &json!({ "source_note_id": source, "task_note_id": task, "linked": false }),
            ))
        }
        "report_codebase_bug" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .ok_or("project_id is required")?;
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or("title is required")?;
            let file_path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .ok_or("file_path is required")?;
            // The schema announces a positive integer: a decimal or a text line is refused.
            let line_num = match args.get("line_number") {
                None | Some(Value::Null) => "?".to_string(),
                Some(value) => match value.as_u64() {
                    Some(line) if line >= 1 => line.to_string(),
                    _ => return Err("line_number must be a positive integer".to_string()),
                },
            };
            // Field announced as "required" by the schema: its absence must be refused, not filled
            // by a silent default (a bug report without an error has no value).
            let error_msg = args
                .get("error_message")
                .and_then(|v| v.as_str())
                .ok_or("error_message is required")?;
            let context = args.get("context").and_then(|v| v.as_str()).unwrap_or("");

            let markdown = format!(
                "### 🐛 Affected file
{} (Line {})

### 🚨 Error
```text
{}
```

### 💡 Context / Proposed fix
{}",
                file_path, line_num, error_msg, context
            );

            let input = CreateNoteInput {
                project_id: project_id.to_string(),
                title: title.to_string(),
                content: Some(markdown),
                note_type: NoteType::Bug,
                priority: Some("P1".to_string()),
                git_branch: None,
                tags: Some("[\"bug_report\"]".to_string()),
                due_at: None,
                raw_content: None,
            };

            let id = queries::create_note(conn, input).map_err(|e| e.to_string())?;
            let note = queries::get_note_by_id(conn, &id).map_err(|e| e.to_string())?;
            Ok(json_result(&json!({
                "id": note.id,
                "title": note.title,
                "project_id": note.project_id,
                "note_type": note.note_type,
                "priority": note.priority,
                "revision": note.revision,
            })))
        }
        "add_note" => {
            // Sub-case (v22): `parent_id` places the note inside a case, in its project;
            // `project_id` may then be omitted.
            let parent_id = match args.get("parent_id") {
                None | Some(Value::Null) => None,
                Some(Value::String(id)) => Some(id.as_str()),
                Some(_) => return Err("parent_id must be a string".to_string()),
            };
            let content_kind = match args.get("content_kind") {
                None | Some(Value::Null) => None,
                Some(value) => {
                    let kind = value.as_str().and_then(ContentKind::parse).ok_or_else(|| {
                        format!(
                            "invalid content_kind: {value} (expected: {})",
                            ContentKind::ALL.map(ContentKind::as_str).join("|")
                        )
                    })?;
                    Some(kind)
                }
            };
            if content_kind.is_some() && parent_id.is_none() {
                return Err("content_kind is only accepted with parent_id".to_string());
            }
            let project_id = match (args.get("project_id").and_then(|v| v.as_str()), parent_id) {
                (Some(id), _) => id,
                (None, Some(_)) => "",
                (None, None) => return Err("project_id is required".to_string()),
            };
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or("title is required")?;
            let content = optional_str(&args, "content")?.map(str::to_string);
            // Field announced as "required" by the schema: refuse its absence rather than applying
            // a silent default (an agent's mistake would become incorrect data).
            let note_type_str = args
                .get("note_type")
                .and_then(|v| v.as_str())
                .ok_or("note_type is required (note|idea|todo|question|research|decision|bug)")?;
            // The schema announces an enumeration: an unknown type must be refused, not silently
            // turned into "note" (an agent's mistake would become incorrect data).
            let note_type = parse_note_type(note_type_str)?;

            // Priority: empty = none, otherwise the announced enumeration (P0-P3).
            let priority = match args.get("priority").and_then(|v| v.as_str()) {
                None => None,
                Some(p) if p.trim().is_empty() => None,
                Some(p) if ["P0", "P1", "P2", "P3"].contains(&p) => Some(p.to_string()),
                Some(p) => return Err(format!("invalid priority: {p} (expected: P0|P1|P2|P3)")),
            };
            // The schema announces an array of strings: actually check it, a `[42]` stored
            // as is crashes the interface when rendering tags (blank screen).
            let tags = tags_from_json(args.get("tags"))?;
            // A due day, checked like `set_due_date` (empty = none) before anything is written.
            let due_at = optional_str(&args, "due_at")?
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(|d| {
                    if is_iso_day(d) {
                        Ok(d.to_string())
                    } else {
                        Err(format!("invalid due_at: {d} (expected YYYY-MM-DD)"))
                    }
                })
                .transpose()?;

            let input = CreateNoteInput {
                project_id: project_id.to_string(),
                title: title.to_string(),
                content,
                note_type,
                priority,
                git_branch: None,
                tags,
                due_at,
                raw_content: None,
            };

            let id = match parent_id {
                Some(parent) => {
                    cases::create_sub_case(conn, parent, content_kind.unwrap_or_default(), input)?
                }
                None => queries::create_note(conn, input).map_err(|e| e.to_string())?,
            };
            let note = queries::get_note_by_id(conn, &id).map_err(|e| e.to_string())?;
            let mut result = json!({
                "id": note.id,
                "title": note.title,
                "project_id": note.project_id,
                "parent_id": note.parent_id,
                "note_type": note.note_type,
                "status": note.status,
                "due_at": note.due_at,
                "revision": note.revision,
            });
            if parent_id.is_none() {
                let existing = existing_cases_like(conn, &note.id, &note.title, &note.project_id);
                if !existing.is_empty() {
                    result["existing_cases"] = Value::Array(existing);
                    result["hint"] = json!("Open cases close to this one already exist. If one of them covers this work, track it there and propose to the person to file this case inside it (move_case) or to archive it.");
                }
            }
            Ok(structured_result(result))
        }
        "update_note_status" => {
            let note_id = args
                .get("note_id")
                .and_then(|v| v.as_str())
                .ok_or("note_id is required")?;
            // Optional: without a status, only the report is recorded (like the CLI `report`).
            let status = optional_str(&args, "status")?
                .map(parse_status)
                .transpose()?;
            let resolution_comment = args
                .get("resolution_comment")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            // Version of the report that the agent read: optimistic check in the same transaction
            // as the write (refused if someone changed it in the meantime).
            let mut expected_resolution_comment = args
                .get("expected_resolution_comment")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            // `append`: the text is added after the stored report instead of replacing it.
            let append = match args.get("append") {
                None | Some(Value::Null) => false,
                Some(value) => value.as_bool().ok_or("append must be a boolean")?,
            };
            let mut resolution_comment = resolution_comment;
            if matches!(status.as_ref(), Some(NoteStatus::Resolved))
                && resolution_comment
                    .as_deref()
                    .is_none_or(|comment| comment.trim().is_empty())
            {
                return Err("resolution_comment is required when status is resolved".to_string());
            }
            if append && resolution_comment.is_none() {
                return Err("append needs a resolution_comment to add".to_string());
            }
            // A report is never replaced blindly: an agent that did not say which report it read
            // would erase another agent's or the person's. Without `expected_resolution_comment`,
            // only an empty report (or the same text) is written; `append` adds to the stored one.
            // The report read here becomes the expected version, checked again in the write's
            // transaction.
            if let Some(new) = resolution_comment.as_deref() {
                if append || expected_resolution_comment.is_none() {
                    let stored = queries::get_note_by_id(conn, note_id)
                        .map_err(|_| crate::tr!("notes.notFound", id = note_id))?
                        .resolution_comment
                        .unwrap_or_default();
                    let (stored_text, new_text) = (stored.trim(), new.trim());
                    if append {
                        if !stored_text.is_empty() {
                            resolution_comment = Some(format!("{stored_text}\n\n{new_text}"));
                        }
                    } else if !stored_text.is_empty() && stored_text != new_text {
                        return Err("this case already has a report: pass expected_resolution_comment with the report you read to replace it, or append: true to add to it".to_string());
                    }
                    expected_resolution_comment.get_or_insert(stored);
                }
            }

            let input = UpdateNoteInput {
                status,
                content: None,
                resolution_comment,
                title: None,
                note_type: None,
                raw_content: None,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
            };

            if let Some(expected) = expected_resolution_comment {
                queries::update_note_if_report_unchanged(conn, note_id, input, Some(&expected))
                    .map_err(|e| e.to_string())?;
            } else {
                queries::update_note(conn, note_id, input).map_err(|e| e.to_string())?;
            }
            let note = queries::get_note_by_id(conn, note_id).map_err(|e| e.to_string())?;
            let mut result = json!({
                "id": note.id,
                "status": note.status,
                "resolution_comment": note.resolution_comment,
                "revision": note.revision,
            });
            if note.status == NoteStatus::Resolved {
                if let Some(parent) = parent_ready(conn, &note.id) {
                    result["parent_ready"] = parent;
                }
            }
            Ok(structured_result(result))
        }
        "update_note" => {
            // ACTUAL order of refusals (keep in sync with the `update_note` section of `docs/MCP_SPEC.md`):
            //   1. `raw_content` present (even `null`) → refusal (guardian no. 3, append-only dictation);
            //   2. authorization absent/refused → refusal, no write;
            //   3. parsing/typing of the arguments (types, note_type/priority enums, tags array of
            //      strings) → refusal;
            //   4. field to modify present and substantive validations (title, due date, JSON tags)
            //      in `apply_agent_note_edit`, before the transaction;
            //   5. missing note → error;
            //   6. `expected_revision` ≠ current revision → conflict refusal.
            // This is not the abstract "ideal" order, it is the code's: do not reorder it
            // without updating the documentation.
            // Guardian no. 3 (append-only dictation): the raw text is never modifiable by an agent, not
            // even through a present key set to `null`. Refusal BEFORE any other check.
            if args.get("raw_content").is_some() {
                return Err("raw_content is not editable by agents".to_string());
            }
            // Option B: agent editing is refused until the human has enabled the
            // authorization in the settings. No write is attempted before this check.
            if !crate::db::permissions::allow_note_edit(conn)? {
                return Err(
                    "agent note editing is disabled; ask the user to enable agent note editing in Humcase settings"
                        .to_string(),
                );
            }
            let note_id = args
                .get("note_id")
                .and_then(Value::as_str)
                .ok_or("note_id is required")?;
            // Unlike P1-02, `expected_revision` is **mandatory**: an edit without a
            // version read cannot be protected from a concurrent write.
            let expected_revision = args
                .get("expected_revision")
                .and_then(Value::as_i64)
                .ok_or("expected_revision is required (integer)")?;

            let title = match args.get("title") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_str().ok_or("title must be a string")?.to_string()),
            };
            let content = match args.get("content") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_str().ok_or("content must be a string")?.to_string()),
            };
            let note_type = match args.get("note_type") {
                None | Some(Value::Null) => None,
                Some(v) => Some(parse_note_type(
                    v.as_str().ok_or("note_type must be a string")?,
                )?),
            };
            // Priority: `null`/absent = unchanged, empty string = removal, otherwise enumeration.
            let priority = match args.get("priority") {
                None | Some(Value::Null) => None,
                Some(v) => {
                    let value = v
                        .as_str()
                        .ok_or("priority must be a string or null")?
                        .trim();
                    if value.is_empty() {
                        Some(String::new())
                    } else if ["P0", "P1", "P2", "P3"].contains(&value) {
                        Some(value.to_string())
                    } else {
                        return Err(format!("invalid priority: {value} (expected: P0|P1|P2|P3)"));
                    }
                }
            };
            let tags = tags_from_json(args.get("tags"))?;
            let due_at = match args.get("due_at") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_str().ok_or("due_at must be a string")?.to_string()),
            };

            let edit = queries::AgentNoteEdit {
                title,
                content,
                note_type,
                tags,
                priority,
                due_at,
            };
            let new_revision =
                queries::apply_agent_note_edit(conn, note_id, expected_revision, edit)?;
            Ok(json_result(
                &json!({ "id": note_id, "revision": new_revision }),
            ))
        }
        "set_due_date" => {
            let note_id = args
                .get("note_id")
                .and_then(|v| v.as_str())
                .ok_or("note_id is required")?;
            let due_at = args
                .get("due_at")
                .and_then(|v| v.as_str())
                .ok_or("due_at is required (YYYY-MM-DD, or empty string to clear)")?;
            // A due date is a day (`YYYY-MM-DD`), not an instant: formats that neither the
            // interface nor the due-date sorting could use are refused.
            if !due_at.trim().is_empty() && !is_iso_day(due_at.trim()) {
                return Err(format!(
                    "invalid due_at: {} (expected YYYY-MM-DD or empty string)",
                    due_at
                ));
            }

            let input = UpdateNoteInput {
                status: None,
                content: None,
                resolution_comment: None,
                title: None,
                note_type: None,
                raw_content: None,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: Some(due_at.to_string()),
            };
            queries::update_note(conn, note_id, input).map_err(|e| e.to_string())?;
            let note = queries::get_note_by_id(conn, note_id).map_err(|e| e.to_string())?;
            Ok(json_result(&json!({
                "id": note.id, "due_at": note.due_at, "revision": note.revision
            })))
        }
        "get_handoff" => {
            let project_id = optional_str(&args, "project_id")?;
            let brief =
                queries::build_agent_handoff(conn, project_id).map_err(|e| e.to_string())?;
            Ok(json!({ "content": [{ "type": "text", "text": brief }] }))
        }
        _ => Err(format!(
            "Tool '{tool_name}' not found. Run tools/list to see available tools."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{queries, schema};
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        schema::apply_migrations(&conn).unwrap();
        conn
    }

    fn note_input(
        project_id: &str,
        title: &str,
        note_type: crate::models::note::NoteType,
    ) -> CreateNoteInput {
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

    #[test]
    fn test_mcp_linked_task_roundtrip_returns_json_and_validates_type() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let source = queries::create_note(
            &conn,
            note_input(&project, "Thought", crate::models::note::NoteType::Idea),
        )
        .unwrap();
        let created = handle_tool_call(
            &conn,
            "create_linked_task",
            Some(json!({
                "source_note_id": source, "title": "Act", "content": "Report later"
            })),
        )
        .unwrap();
        let id_json: Value =
            serde_json::from_str(created["content"][0]["text"].as_str().unwrap()).unwrap();
        let task_id = id_json["task_note_id"].as_str().unwrap();
        let task = queries::get_note_by_id(&conn, task_id).unwrap();
        assert_eq!(task.note_type, crate::models::note::NoteType::Todo);
        let relations = handle_tool_call(
            &conn,
            "get_note_relations",
            Some(json!({ "note_id": source })),
        )
        .unwrap();
        let parsed: Value =
            serde_json::from_str(relations["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(parsed["tasks"][0]["id"], task_id);
        assert!(handle_tool_call(
            &conn,
            "link_note_task",
            Some(json!({
                "source_note_id": task_id, "task_note_id": source
            }))
        )
        .is_err());
        handle_tool_call(
            &conn,
            "unlink_note_task",
            Some(json!({
                "source_note_id": source, "task_note_id": task_id
            })),
        )
        .unwrap();
        let relations = handle_tool_call(
            &conn,
            "get_note_relations",
            Some(json!({ "note_id": source })),
        )
        .unwrap();
        let parsed: Value =
            serde_json::from_str(relations["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(parsed["tasks"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_mcp_invalid_tool() {
        let conn = setup_db();
        let res = handle_tool_call(&conn, "non_existent", None);
        assert!(res.is_err());
    }

    #[test]
    fn test_mcp_project_space_uses_english_values_and_can_clear() {
        let conn = setup_db();
        let created = handle_tool_call(
            &conn,
            "create_project",
            Some(json!({
                "name": "Workspace", "space": "work"
            })),
        )
        .unwrap();
        let created: Value =
            serde_json::from_str(created["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(created["space"], "work");
        let id = created["id"].as_str().unwrap();
        assert_eq!(
            queries::get_projects(&conn).unwrap()[0].space,
            Some(ProjectSpace::Work)
        );
        assert!(handle_tool_call(
            &conn,
            "update_project_space",
            Some(json!({
                "project_id": id, "space": "personal"
            }))
        )
        .is_err());
        handle_tool_call(
            &conn,
            "update_project_space",
            Some(json!({
                "project_id": id, "space": null
            })),
        )
        .unwrap();
        assert_eq!(queries::get_projects(&conn).unwrap()[0].space, None);
    }

    #[test]
    fn test_mcp_activities_and_inbox_validate_contract() {
        let conn = setup_db();
        let project =
            queries::create_project_in_space(&conn, "Workspace", None, Some(ProjectSpace::Work))
                .unwrap();
        let created = handle_tool_call(
            &conn,
            "create_activity",
            Some(json!({
                "name": "Development", "space": "work"
            })),
        )
        .unwrap();
        let activity: Value =
            serde_json::from_str(created["content"][0]["text"].as_str().unwrap()).unwrap();
        let activity_id = activity["id"].as_str().unwrap();
        assert_eq!(activity["space"], "work");
        assert!(handle_tool_call(
            &conn,
            "create_activity",
            Some(json!({"name":"Bad","space":"pro"}))
        )
        .is_err());
        assert!(handle_tool_call(
            &conn,
            "set_project_activities",
            Some(json!({
                "project_id": project, "activity_ids": [42]
            }))
        )
        .is_err());
        handle_tool_call(
            &conn,
            "set_project_activities",
            Some(json!({
                "project_id": project, "activity_ids": [activity_id]
            })),
        )
        .unwrap();
        let listed = handle_tool_call(&conn, "list_activities", None).unwrap();
        let items: Value =
            serde_json::from_str(listed["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(items[0]["project_ids"][0], project);
        let inbox =
            handle_tool_call(&conn, "get_or_create_inbox", Some(json!({"space":"work"}))).unwrap();
        let inbox: Value =
            serde_json::from_str(inbox["content"][0]["text"].as_str().unwrap()).unwrap();
        let inbox_id = inbox["id"].as_str().unwrap();
        assert!(queries::get_projects(&conn)
            .unwrap()
            .iter()
            .any(|p| p.id == inbox_id && p.is_inbox));
    }

    #[test]
    fn test_mcp_set_due_date_valid_invalid_and_clearing() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Deadlines", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N1", crate::models::note::NoteType::Todo),
        )
        .unwrap();

        // Format refused: a due date is a `YYYY-MM-DD` day, not a localized date.
        let bad = handle_tool_call(
            &conn,
            "set_due_date",
            Some(json!({ "note_id": n_id, "due_at": "31/12/2026" })),
        );
        assert!(bad.is_err());
        assert!(bad.unwrap_err().contains("invalid due_at"));

        let good = handle_tool_call(
            &conn,
            "set_due_date",
            Some(json!({ "note_id": n_id, "due_at": "2026-12-31" })),
        );
        assert!(good.is_ok());
        assert_eq!(
            queries::get_note_by_id(&conn, &n_id)
                .unwrap()
                .due_at
                .as_deref(),
            Some("2026-12-31")
        );

        // Empty string: the due date is removed.
        handle_tool_call(
            &conn,
            "set_due_date",
            Some(json!({ "note_id": n_id, "due_at": "" })),
        )
        .unwrap();
        assert!(queries::get_note_by_id(&conn, &n_id)
            .unwrap()
            .due_at
            .is_none());
    }

    /// #15 — the validator must refuse a nonexistent date (`2026-02-31`), not only a
    /// bad format.
    #[test]
    fn test_mcp_set_due_date_rejects_nonexistent_days() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Calendar", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N1", crate::models::note::NoteType::Todo),
        )
        .unwrap();

        for day in ["2026-02-31", "2026-04-31", "2026-13-01", "2026-02-29"] {
            let res = handle_tool_call(
                &conn,
                "set_due_date",
                Some(json!({ "note_id": n_id, "due_at": day })),
            );
            assert!(res.is_err(), "a nonexistent day was accepted: {day}");
            assert!(queries::get_note_by_id(&conn, &n_id)
                .unwrap()
                .due_at
                .is_none());
        }

        // Leap year: 29 February 2028 exists.
        let leap_year = handle_tool_call(
            &conn,
            "set_due_date",
            Some(json!({ "note_id": n_id, "due_at": "2028-02-29" })),
        );
        assert!(leap_year.is_ok());
        assert_eq!(
            queries::get_note_by_id(&conn, &n_id)
                .unwrap()
                .due_at
                .as_deref(),
            Some("2028-02-29")
        );
    }

    /// #10 — a missing note must never produce a success message.
    #[test]
    fn test_mcp_write_on_missing_note_returns_error() {
        let conn = setup_db();
        let _p_id = queries::create_project(&conn, "Ghost", None).unwrap();

        let status_result = handle_tool_call(
            &conn,
            "update_note_status",
            Some(json!({
                "note_id": "00000000-0000-0000-0000-000000000000",
                "status": "resolved",
                "resolution_comment": "resolved"
            })),
        );
        let error = status_result.unwrap_err();
        assert!(
            error.contains("not found"),
            "expected a not-found message, received: {error}"
        );

        let due_result = handle_tool_call(
            &conn,
            "set_due_date",
            Some(json!({
                "note_id": "00000000-0000-0000-0000-000000000000",
                "due_at": "2026-12-31"
            })),
        );
        assert!(due_result.is_err());
        assert!(due_result.unwrap_err().contains("not found"));
    }

    /// #10 — a call without any field to modify is an error, not a silent success.
    #[test]
    fn test_mcp_update_without_field_returns_error() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Empty", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N1", crate::models::note::NoteType::Note),
        )
        .unwrap();

        let res = handle_tool_call(
            &conn,
            "update_note_status",
            Some(json!({ "note_id": n_id })),
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("No field"));
    }

    #[test]
    fn test_mcp_get_handoff_returns_brief() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Brief", None).unwrap();
        let mut input = note_input(&p_id, "Resume here", crate::models::note::NoteType::Todo);
        input.priority = Some("P1".into());
        queries::create_note(&conn, input).unwrap();

        let res =
            handle_tool_call(&conn, "get_handoff", Some(json!({ "project_id": p_id }))).unwrap();
        let brief = res["content"][0]["text"].as_str().unwrap();
        assert!(brief.contains("# Handoff: Brief"), "{brief}");
        assert!(brief.contains("Resume here"), "{brief}");
    }

    #[test]
    fn test_mcp_invalid_status() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "T1", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N1", crate::models::note::NoteType::Note),
        )
        .unwrap();

        let args = serde_json::json!({
            "note_id": n_id,
            "status": "done"
        });

        let res = handle_tool_call(&conn, "update_note_status", Some(args));
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("invalid status: done"));
    }

    /// #11 — `tags` must be an array of strings: `[42]` is refused, not stored as is.
    #[test]
    fn test_mcp_add_note_rejects_non_string_tags() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Tags", None).unwrap();

        let res = handle_tool_call(
            &conn,
            "add_note",
            Some(json!({
                "project_id": p_id,
                "title": "Corrupted tags",
                "note_type": "note",
                "tags": [42]
            })),
        );
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .contains("tags must be an array of strings"));
        assert!(
            queries::get_notes(&conn, None, None, None)
                .unwrap()
                .is_empty(),
            "no note must be created"
        );
    }

    /// #11 — an array of strings is still accepted and stored as JSON.
    #[test]
    fn test_mcp_add_note_accepts_string_array() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Tags", None).unwrap();

        let res = handle_tool_call(
            &conn,
            "add_note",
            Some(json!({
                "project_id": p_id,
                "title": "Clean tags",
                "note_type": "note",
                "tags": ["audit", "mcp"]
            })),
        );
        assert!(res.is_ok(), "{res:?}");

        let notes = queries::get_notes(&conn, None, None, None).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].tags.as_deref(), Some("[\"audit\",\"mcp\"]"));
    }

    /// Every nature offered by the interface is accepted and read back unchanged.
    #[test]
    fn test_mcp_add_note_accepts_every_nature() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Natures", None).unwrap();
        for nature in crate::models::note::NoteType::ALL {
            handle_tool_call(
                &conn,
                "add_note",
                Some(json!({ "project_id": p_id, "title": nature.as_str(), "note_type": nature.as_str() })),
            )
            .unwrap();
        }
        let notes = queries::get_notes(&conn, None, None, None).unwrap();
        assert_eq!(notes.len(), crate::models::note::NoteType::ALL.len());
        for note in notes {
            assert_eq!(note.note_type.as_str(), note.title);
        }
    }

    /// C1 — an unknown type is refused, not silently turned into "note".
    #[test]
    fn test_mcp_add_note_rejects_unknown_type() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Types", None).unwrap();
        let res = handle_tool_call(
            &conn,
            "add_note",
            Some(json!({
                "project_id": p_id, "title": "Unknown type", "note_type": "pr"
            })),
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("invalid note_type"));
        assert!(queries::get_notes(&conn, None, None, None)
            .unwrap()
            .is_empty());
    }

    /// C1 — a priority outside the enumeration is refused (the schema announces P0-P3).
    #[test]
    fn test_mcp_add_note_rejects_invalid_priority() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Priorities", None).unwrap();
        let res = handle_tool_call(
            &conn,
            "add_note",
            Some(json!({
                "project_id": p_id, "title": "P9", "note_type": "note", "priority": "P9"
            })),
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("invalid priority"));
        assert!(queries::get_notes(&conn, None, None, None)
            .unwrap()
            .is_empty());
    }

    /// D1 — `add_note` without `note_type` is an explicit error (field announced as required), not a
    /// silent default "note".
    #[test]
    fn test_mcp_add_note_requires_note_type() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Required", None).unwrap();
        let res = handle_tool_call(
            &conn,
            "add_note",
            Some(json!({
                "project_id": p_id, "title": "No type"
            })),
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("note_type is required"));
        assert!(queries::get_notes(&conn, None, None, None)
            .unwrap()
            .is_empty());
    }

    /// D1 — `report_codebase_bug` without `error_message` is refused (no empty bug report).
    #[test]
    fn test_mcp_report_codebase_bug_requires_error_message() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Bugs", None).unwrap();
        let res = handle_tool_call(
            &conn,
            "report_codebase_bug",
            Some(json!({
                "project_id": p_id, "title": "Bug", "file_path": "src/lib.rs"
            })),
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("error_message is required"));
        assert!(queries::get_notes(&conn, None, None, None)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_mcp_report_codebase_bug_requires_file_path() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Bugs", None).unwrap();
        let res = handle_tool_call(
            &conn,
            "report_codebase_bug",
            Some(json!({ "project_id": p_id, "title": "Bug", "error_message": "boom" })),
        );
        assert!(res.unwrap_err().contains("file_path is required"));
        assert!(queries::get_notes(&conn, None, None, None)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_mcp_report_codebase_bug_puts_error_in_markdown_block() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Bugs", None).unwrap();
        handle_tool_call(
            &conn,
            "report_codebase_bug",
            Some(json!({
                "project_id": p_id,
                "title": "Compilation error",
                "file_path": "src/main.rs",
                "line_number": 42,
                "error_message": "expected expression",
                "context": "After the type change"
            })),
        )
        .unwrap();

        let notes = queries::get_notes(&conn, None, None, None).unwrap();
        assert_eq!(notes.len(), 1);
        assert!(
            notes[0]
                .content
                .contains("### 🚨 Error\n```text\nexpected expression\n```\n"),
            "{}",
            notes[0].content
        );
    }

    #[test]
    fn test_mcp_resolution_requires_non_empty_comment() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Tracking", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N1", crate::models::note::NoteType::Todo),
        )
        .unwrap();

        for comment in [None, Some(""), Some(" \t\n ")] {
            let mut args = json!({ "note_id": n_id, "status": "resolved" });
            if let Some(comment) = comment {
                args["resolution_comment"] = json!(comment);
            }
            let res = handle_tool_call(&conn, "update_note_status", Some(args));
            assert!(
                res.is_err(),
                "resolution accepted without a comment: {comment:?}"
            );
            let note = queries::get_note_by_id(&conn, &n_id).unwrap();
            assert_eq!(note.status, crate::models::note::NoteStatus::Open);
            assert!(note.resolution_comment.is_none());
        }

        handle_tool_call(
            &conn,
            "update_note_status",
            Some(json!({
                "note_id": n_id, "status": "in_progress"
            })),
        )
        .unwrap();
        assert_eq!(
            queries::get_note_by_id(&conn, &n_id).unwrap().status,
            crate::models::note::NoteStatus::InProgress
        );

        handle_tool_call(&conn, "update_note_status", Some(json!({
            "note_id": n_id, "status": "resolved", "resolution_comment": "Fixed and verified with cargo test"
        }))).unwrap();
        let note = queries::get_note_by_id(&conn, &n_id).unwrap();
        assert_eq!(note.status, crate::models::note::NoteStatus::Resolved);
        assert_eq!(
            note.resolution_comment.as_deref(),
            Some("Fixed and verified with cargo test")
        );
    }

    /// P1-02 — with `expected_resolution_comment`, the MCP refuses to overwrite a report whose
    /// expected version no longer matches. Without this field, the write stays direct (backward compat).
    #[test]
    fn test_mcp_update_note_status_rejects_stale_report() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Concurrent tracking", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N1", crate::models::note::NoteType::Todo),
        )
        .unwrap();

        // The human writes their report while the agent reads an older version.
        handle_tool_call(
            &conn,
            "update_note_status",
            Some(json!({
                "note_id": n_id, "status": "in_progress", "resolution_comment": "Human report"
            })),
        )
        .unwrap();

        let refusal = handle_tool_call(
            &conn,
            "update_note_status",
            Some(json!({
                "note_id": n_id,
                "status": "resolved",
                "resolution_comment": "Agent report",
                "expected_resolution_comment": "old version"
            })),
        );
        let error = refusal.unwrap_err();
        assert!(
            error.contains("changed"),
            "expected a refusal, received: {error}"
        );
        let note = queries::get_note_by_id(&conn, &n_id).unwrap();
        assert_eq!(note.resolution_comment.as_deref(), Some("Human report"));
        assert_eq!(note.status, crate::models::note::NoteStatus::InProgress);

        // The right expected version lets the write through.
        handle_tool_call(
            &conn,
            "update_note_status",
            Some(json!({
                "note_id": n_id,
                "status": "resolved",
                "resolution_comment": "Agent report",
                "expected_resolution_comment": "Human report"
            })),
        )
        .unwrap();
        let note = queries::get_note_by_id(&conn, &n_id).unwrap();
        assert_eq!(note.status, crate::models::note::NoteStatus::Resolved);
        assert_eq!(note.resolution_comment.as_deref(), Some("Agent report"));

        // Without `expected_resolution_comment`, an existing report is never replaced.
        let refusal = handle_tool_call(
            &conn,
            "update_note_status",
            Some(json!({
                "note_id": n_id, "status": "in_progress", "resolution_comment": "Direct write"
            })),
        )
        .unwrap_err();
        assert!(refusal.contains("already has a report"), "{refusal}");
        let note = queries::get_note_by_id(&conn, &n_id).unwrap();
        assert_eq!(note.resolution_comment.as_deref(), Some("Agent report"));
        assert_eq!(note.status, crate::models::note::NoteStatus::Resolved);
    }

    /// Lived on 13/11: adding two lines to a report replaced the whole report. Without the report
    /// read, only an empty report is written; `append` adds after the stored one.
    #[test]
    fn test_mcp_update_note_status_never_erases_a_report_blindly() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Reports", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N1", crate::models::note::NoteType::Todo),
        )
        .unwrap();
        let report = || {
            queries::get_note_by_id(&conn, &n_id)
                .unwrap()
                .resolution_comment
        };
        let call = |args: Value| handle_tool_call(&conn, "update_note_status", Some(args));

        // No report yet: written without the expected version.
        call(json!({ "note_id": n_id, "status": "resolved", "resolution_comment": "Full report" }))
            .unwrap();
        // The same text again is not a conflict.
        call(json!({ "note_id": n_id, "resolution_comment": "Full report" })).unwrap();
        // Another text without the report read: refused, nothing written.
        let error =
            call(json!({ "note_id": n_id, "resolution_comment": "Two lines" })).unwrap_err();
        assert!(error.contains("already has a report"), "{error}");
        assert_eq!(report().as_deref(), Some("Full report"));
        // `append` adds after it.
        call(json!({ "note_id": n_id, "resolution_comment": "Two lines", "append": true }))
            .unwrap();
        assert_eq!(report().as_deref(), Some("Full report\n\nTwo lines"));
        // `append` with a stale expected version is refused.
        let error = call(json!({
            "note_id": n_id, "resolution_comment": "More", "append": true,
            "expected_resolution_comment": "Full report"
        }))
        .unwrap_err();
        assert!(error.contains("changed"), "{error}");
        assert_eq!(report().as_deref(), Some("Full report\n\nTwo lines"));
        // `append` without text, or not a boolean, is refused.
        assert!(call(json!({ "note_id": n_id, "status": "resolved", "append": true })).is_err());
        assert!(
            call(json!({ "note_id": n_id, "resolution_comment": "x", "append": "yes" })).is_err()
        );
    }

    /// Option B — without authorization, `update_note` refuses and touches nothing.
    #[test]
    fn test_mcp_update_note_requires_permission_and_changes_nothing() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Perm", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "Before", crate::models::note::NoteType::Note),
        )
        .unwrap();

        let res = handle_tool_call(
            &conn,
            "update_note",
            Some(json!({
                "note_id": n_id, "expected_revision": 0, "title": "After"
            })),
        );
        let error = res.unwrap_err();
        assert!(error.contains("disabled"), "{error}");
        assert!(
            error.contains("settings"),
            "the message must say how to enable it: {error}"
        );
        let note = queries::get_note_by_id(&conn, &n_id).unwrap();
        assert_eq!(note.title, "Before");
        assert_eq!(note.revision, 0);
        assert!(crate::db::permissions::note_history(&conn, &n_id)
            .unwrap()
            .is_empty());
    }

    /// Option B — with authorization: fields applied, revision +1, snapshot attributed to the agent.
    #[test]
    fn test_mcp_update_note_applies_and_snapshots_when_allowed() {
        let conn = setup_db();
        crate::db::permissions::set_setting(
            &conn,
            crate::db::permissions::MCP_ALLOW_NOTE_EDIT,
            "1",
        )
        .unwrap();
        let p_id = queries::create_project(&conn, "Edit", None).unwrap();
        let mut input = note_input(&p_id, "Before", crate::models::note::NoteType::Idea);
        input.raw_content = Some("raw".into());
        let n_id = queries::create_note(&conn, input).unwrap();

        let res = handle_tool_call(
            &conn,
            "update_note",
            Some(json!({
                "note_id": n_id,
                "expected_revision": 0,
                "title": "After",
                "content": "new content",
                "note_type": "todo",
                "tags": ["mcp", "edit"],
                "priority": "P1",
                "due_at": "2026-12-31"
            })),
        )
        .unwrap();
        let written: Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(written, json!({ "id": n_id, "revision": 1 }));

        let note = queries::get_note_by_id(&conn, &n_id).unwrap();
        assert_eq!(note.title, "After");
        assert_eq!(note.content, "new content");
        assert_eq!(note.note_type, crate::models::note::NoteType::Todo);
        assert_eq!(note.tags.as_deref(), Some("[\"mcp\",\"edit\"]"));
        assert_eq!(note.priority.as_deref(), Some("P1"));
        assert_eq!(note.due_at.as_deref(), Some("2026-12-31"));
        assert_eq!(note.revision, 1);
        assert_eq!(
            note.raw_content.as_deref(),
            Some("raw"),
            "raw_content is never modified"
        );

        let history = crate::db::permissions::note_history(&conn, &n_id).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].source, "agent");
        assert_eq!(
            history[0].revision, 0,
            "the snapshot carries the old version"
        );
        let fields: Value = serde_json::from_str(&history[0].fields).unwrap();
        assert_eq!(fields["title"], "Before");
        assert_eq!(fields["content"], "");
        assert_eq!(fields["note_type"], "idea");
        assert_eq!(fields["priority"], Value::Null);
        assert_eq!(fields["due_at"], Value::Null);
        assert!(
            fields.get("raw_content").is_none(),
            "raw_content must never appear in the snapshot"
        );
    }

    /// Two writers — the agent reads rev N, the app writes (rev N+1): the agent write with N is
    /// refused (leaving nothing behind), then replayed successfully after re-reading.
    #[test]
    fn test_mcp_update_note_two_writers_conflict_then_retry() {
        let conn = setup_db();
        crate::db::permissions::set_setting(
            &conn,
            crate::db::permissions::MCP_ALLOW_NOTE_EDIT,
            "1",
        )
        .unwrap();
        let p_id = queries::create_project(&conn, "Two writers", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "Version N", crate::models::note::NoteType::Todo),
        )
        .unwrap();

        // The agent reads version 0. The human app writes in the meantime: revision 1.
        queries::update_note(
            &conn,
            &n_id,
            UpdateNoteInput {
                status: Some(crate::models::note::NoteStatus::InProgress),
                content: None,
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
        assert_eq!(queries::get_note_by_id(&conn, &n_id).unwrap().revision, 1);

        let refusal = handle_tool_call(
            &conn,
            "update_note",
            Some(json!({
                "note_id": n_id, "expected_revision": 0, "title": "Written by the agent"
            })),
        )
        .unwrap_err();
        assert!(refusal.contains("revision conflict"), "{refusal}");
        assert!(
            refusal.contains("revision 1"),
            "the current version must be recalled: {refusal}"
        );
        assert_eq!(
            queries::get_note_by_id(&conn, &n_id).unwrap().title,
            "Version N"
        );
        assert!(crate::db::permissions::note_history(&conn, &n_id)
            .unwrap()
            .is_empty());

        // The agent re-reads version 1 and replays: success.
        let ok = handle_tool_call(
            &conn,
            "update_note",
            Some(json!({
                "note_id": n_id, "expected_revision": 1, "title": "Written by the agent"
            })),
        )
        .unwrap();
        let written: Value =
            serde_json::from_str(ok["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(written["revision"], 2);
        let note = queries::get_note_by_id(&conn, &n_id).unwrap();
        assert_eq!(note.title, "Written by the agent");
        assert_eq!(note.revision, 2);
        assert_eq!(
            crate::db::permissions::note_history(&conn, &n_id)
                .unwrap()
                .len(),
            1
        );
    }

    /// `raw_content` is refused even when present with `null`, and before the authorization check;
    /// it stays refused when authorization is granted.
    #[test]
    fn test_mcp_update_note_rejects_raw_content_even_without_permission() {
        let conn = setup_db();
        let p_id = queries::create_project(&conn, "Raw", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N", crate::models::note::NoteType::Note),
        )
        .unwrap();

        // Without authorization: the `raw_content` refusal must precede the permission refusal.
        for raw in [json!("new raw"), json!(null)] {
            let res = handle_tool_call(
                &conn,
                "update_note",
                Some(json!({
                    "note_id": n_id, "expected_revision": 0, "raw_content": raw
                })),
            );
            let error = res.unwrap_err();
            assert!(error.contains("raw_content is not editable"), "{error}");
        }

        // With authorization: still refused, no write.
        crate::db::permissions::set_setting(
            &conn,
            crate::db::permissions::MCP_ALLOW_NOTE_EDIT,
            "1",
        )
        .unwrap();
        let error = handle_tool_call(
            &conn,
            "update_note",
            Some(json!({
                "note_id": n_id, "expected_revision": 0, "raw_content": "injected raw"
            })),
        )
        .unwrap_err();
        assert!(error.contains("raw_content is not editable"), "{error}");

        let note = queries::get_note_by_id(&conn, &n_id).unwrap();
        assert_eq!(note.raw_content, None);
        assert_eq!(note.revision, 0);
        assert!(crate::db::permissions::note_history(&conn, &n_id)
            .unwrap()
            .is_empty());
    }

    /// Validations identical to `create_note`; a refusal neither writes nor logs.
    #[test]
    fn test_mcp_update_note_validations_do_not_write() {
        let conn = setup_db();
        crate::db::permissions::set_setting(
            &conn,
            crate::db::permissions::MCP_ALLOW_NOTE_EDIT,
            "1",
        )
        .unwrap();
        let p_id = queries::create_project(&conn, "Validations", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N", crate::models::note::NoteType::Note),
        )
        .unwrap();

        let cases = [
            (json!({ "title": "" }), "title must not be empty"),
            (json!({ "due_at": "2026-02-31" }), "invalid due_at"),
            (json!({ "note_type": "pr" }), "invalid note_type"),
            (json!({ "priority": "P9" }), "invalid priority"),
            (json!({ "tags": [42] }), "tags must be an array of strings"),
        ];
        for (fields, expected_text) in cases {
            let mut args = json!({ "note_id": n_id, "expected_revision": 0 });
            for (key, value) in fields.as_object().unwrap() {
                args[key] = value.clone();
            }
            let error = handle_tool_call(&conn, "update_note", Some(args)).unwrap_err();
            assert!(
                error.contains(expected_text),
                "expected \"{expected_text}\", received \"{error}\""
            );
        }
        let note = queries::get_note_by_id(&conn, &n_id).unwrap();
        assert_eq!(note.title, "N");
        assert_eq!(note.revision, 0);
        assert!(crate::db::permissions::note_history(&conn, &n_id)
            .unwrap()
            .is_empty());
    }

    /// `expected_revision` is mandatory (unlike the tolerated P1-02 option).
    #[test]
    fn test_mcp_update_note_requires_expected_revision() {
        let conn = setup_db();
        crate::db::permissions::set_setting(
            &conn,
            crate::db::permissions::MCP_ALLOW_NOTE_EDIT,
            "1",
        )
        .unwrap();
        let p_id = queries::create_project(&conn, "Required", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N", crate::models::note::NoteType::Note),
        )
        .unwrap();
        let error = handle_tool_call(
            &conn,
            "update_note",
            Some(json!({ "note_id": n_id, "title": "X" })),
        )
        .unwrap_err();
        assert!(error.contains("expected_revision is required"), "{error}");
    }

    /// Missing note: explicit error (with authorization).
    #[test]
    fn test_mcp_update_note_missing_note_returns_error() {
        let conn = setup_db();
        crate::db::permissions::set_setting(
            &conn,
            crate::db::permissions::MCP_ALLOW_NOTE_EDIT,
            "1",
        )
        .unwrap();
        let error = handle_tool_call(
            &conn,
            "update_note",
            Some(json!({
                "note_id": "00000000-0000-0000-0000-000000000000",
                "expected_revision": 0,
                "title": "X"
            })),
        )
        .unwrap_err();
        assert!(error.contains("not found"), "{error}");
    }

    /// A call without any modifiable field is an error, not a silent success.
    #[test]
    fn test_mcp_update_note_without_field_returns_error() {
        let conn = setup_db();
        crate::db::permissions::set_setting(
            &conn,
            crate::db::permissions::MCP_ALLOW_NOTE_EDIT,
            "1",
        )
        .unwrap();
        let p_id = queries::create_project(&conn, "Empty", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N", crate::models::note::NoteType::Note),
        )
        .unwrap();
        let error = handle_tool_call(
            &conn,
            "update_note",
            Some(json!({
                "note_id": n_id, "expected_revision": 0
            })),
        )
        .unwrap_err();
        assert!(error.contains("No field"), "{error}");
    }

    /// The history is empty for a note never edited by an agent, then sorted in descending order.
    #[test]
    fn test_note_history_empty_until_agent_edit_then_ordered() {
        let conn = setup_db();
        crate::db::permissions::set_setting(
            &conn,
            crate::db::permissions::MCP_ALLOW_NOTE_EDIT,
            "1",
        )
        .unwrap();
        let p_id = queries::create_project(&conn, "History", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "Before", crate::models::note::NoteType::Note),
        )
        .unwrap();
        assert!(crate::db::permissions::note_history(&conn, &n_id)
            .unwrap()
            .is_empty());

        handle_tool_call(
            &conn,
            "update_note",
            Some(json!({
                "note_id": n_id, "expected_revision": 0, "title": "First"
            })),
        )
        .unwrap();
        handle_tool_call(
            &conn,
            "update_note",
            Some(json!({
                "note_id": n_id, "expected_revision": 1, "content": "Second"
            })),
        )
        .unwrap();

        let history = crate::db::permissions::note_history(&conn, &n_id).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].revision, 1, "the most recent first");
        assert_eq!(history[1].revision, 0);
        assert!(history.iter().all(|entry| entry.source == "agent"));
    }

    /// m9 — the flag governs ONLY `update_note`: the pre-existing dedicated tools
    /// (`update_note_status`, `set_due_date`) stay usable without authorization. This test fails
    /// if someone gates them one day without a decision.
    #[test]
    fn test_dedicated_note_tools_work_without_agent_edit_permission() {
        let conn = setup_db();
        // No agent edit authorization is set: the default value is "refused".
        assert!(!crate::db::permissions::allow_note_edit(&conn).unwrap());
        let p_id = queries::create_project(&conn, "Outside flag", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N", crate::models::note::NoteType::Todo),
        )
        .unwrap();

        handle_tool_call(
            &conn,
            "update_note_status",
            Some(json!({
                "note_id": n_id, "status": "in_progress"
            })),
        )
        .unwrap();
        handle_tool_call(
            &conn,
            "set_due_date",
            Some(json!({
                "note_id": n_id, "due_at": "2026-12-31"
            })),
        )
        .unwrap();

        let note = queries::get_note_by_id(&conn, &n_id).unwrap();
        assert_eq!(note.status, crate::models::note::NoteStatus::InProgress);
        assert_eq!(note.due_at.as_deref(), Some("2026-12-31"));
    }

    /// m9 — a non-integer `expected_revision` (decimal number or string) is refused with the same
    /// message as its absence: a truncated concurrency token would be a silent conflict.
    #[test]
    fn test_mcp_update_note_rejects_non_integer_expected_revision() {
        let conn = setup_db();
        crate::db::permissions::set_setting(
            &conn,
            crate::db::permissions::MCP_ALLOW_NOTE_EDIT,
            "1",
        )
        .unwrap();
        let p_id = queries::create_project(&conn, "Integer", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N", crate::models::note::NoteType::Note),
        )
        .unwrap();

        for token in [json!(1.5), json!(-1.5), json!("0")] {
            let error = handle_tool_call(
                &conn,
                "update_note",
                Some(json!({
                    "note_id": n_id, "expected_revision": token, "title": "X"
                })),
            )
            .unwrap_err();
            assert!(
                error.contains("expected_revision is required (integer)"),
                "token {token}: {error}"
            );
        }
        let note = queries::get_note_by_id(&conn, &n_id).unwrap();
        assert_eq!(note.title, "N");
        assert_eq!(note.revision, 0);
    }

    /// m9 — a negative `expected_revision` (integer) does not exist: conflict refusal, without a write.
    #[test]
    fn test_mcp_update_note_negative_expected_revision_conflicts() {
        let conn = setup_db();
        crate::db::permissions::set_setting(
            &conn,
            crate::db::permissions::MCP_ALLOW_NOTE_EDIT,
            "1",
        )
        .unwrap();
        let p_id = queries::create_project(&conn, "Negative", None).unwrap();
        let n_id = queries::create_note(
            &conn,
            note_input(&p_id, "N", crate::models::note::NoteType::Note),
        )
        .unwrap();

        let error = handle_tool_call(
            &conn,
            "update_note",
            Some(json!({
                "note_id": n_id, "expected_revision": -1, "title": "X"
            })),
        )
        .unwrap_err();
        assert!(error.contains("revision conflict"), "{error}");
        assert!(
            error.contains("revision 0"),
            "the current version must be recalled: {error}"
        );
        let note = queries::get_note_by_id(&conn, &n_id).unwrap();
        assert_eq!(note.title, "N");
        assert_eq!(note.revision, 0);
        assert!(crate::db::permissions::note_history(&conn, &n_id)
            .unwrap()
            .is_empty());
    }

    fn text_of(result: &Value) -> &str {
        result["content"][0]["text"].as_str().unwrap()
    }

    fn created_id(result: &Value) -> String {
        let created: Value = serde_json::from_str(text_of(result)).unwrap();
        created["id"].as_str().unwrap().to_string()
    }

    /// Sub-cases through MCP: create inside a case without project_id, move, list, record a
    /// provenance, read it all back with get_note.
    #[test]
    fn test_mcp_sub_cases_roundtrip() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let root = queries::create_note(
            &conn,
            note_input(&project, "Root", crate::models::note::NoteType::Idea),
        )
        .unwrap();
        let photo = created_id(
            &handle_tool_call(
                &conn,
                "add_note",
                Some(json!({
                    "parent_id": root, "title": "Photo", "note_type": "note", "content_kind": "image"
                })),
            )
            .unwrap(),
        );
        let detail = created_id(
            &handle_tool_call(
                &conn,
                "add_note",
                Some(json!({ "parent_id": photo, "title": "Detail", "note_type": "note" })),
            )
            .unwrap(),
        );
        let note = queries::get_note_by_id(&conn, &photo).unwrap();
        assert_eq!(note.project_id, project);
        assert_eq!(note.content_kind, ContentKind::Image);

        let listed: Value = serde_json::from_str(text_of(
            &handle_tool_call(&conn, "list_sub_cases", Some(json!({ "note_id": root }))).unwrap(),
        ))
        .unwrap();
        assert_eq!(listed["total"], 2);
        let listed = &listed["sub_cases"];
        assert_eq!(listed[0]["title"], "Photo");
        assert_eq!(listed[0]["depth"], 1);
        assert_eq!(listed[0]["content_kind"], "image");
        assert_eq!(listed[1]["title"], "Detail");
        assert_eq!(listed[1]["depth"], 2);
        let direct: Value = serde_json::from_str(text_of(
            &handle_tool_call(
                &conn,
                "list_sub_cases",
                Some(json!({ "note_id": root, "max_depth": 1 })),
            )
            .unwrap(),
        ))
        .unwrap();
        assert_eq!(direct["sub_cases"].as_array().unwrap().len(), 1);

        handle_tool_call(
            &conn,
            "add_provenance",
            Some(json!({
                "note_id": detail, "kind": "source", "url": "https://example.com", "agent": "claude"
            })),
        )
        .unwrap();
        let read: Value = serde_json::from_str(text_of(
            &handle_tool_call(&conn, "get_note", Some(json!({ "note_id": detail }))).unwrap(),
        ))
        .unwrap();
        assert_eq!(read["parent_id"], photo.as_str());
        assert_eq!(read["case_path"][0]["title"], "Root");
        assert_eq!(read["case_path"][1]["title"], "Photo");
        assert_eq!(read["sub_case_count"], 0);
        assert_eq!(read["provenance"][0]["url"], "https://example.com");
        assert_eq!(read["provenance"][0]["agent"], "claude");

        let error = handle_tool_call(
            &conn,
            "move_case",
            Some(json!({ "note_id": root, "parent_id": detail })),
        )
        .unwrap_err();
        assert!(error.contains("cannot go inside itself"), "{error}");
        handle_tool_call(
            &conn,
            "move_case",
            Some(json!({ "note_id": detail, "parent_id": root })),
        )
        .unwrap();
        assert_eq!(
            queries::get_note_by_id(&conn, &detail)
                .unwrap()
                .parent_id
                .as_deref(),
            Some(root.as_str())
        );
        handle_tool_call(
            &conn,
            "move_case",
            Some(json!({ "note_id": detail, "parent_id": null })),
        )
        .unwrap();
        assert_eq!(
            queries::get_note_by_id(&conn, &detail).unwrap().parent_id,
            None
        );
    }

    #[test]
    fn test_mcp_json_canvas_import_then_export() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let canvas = json!({ "nodes": [
            { "id": "g", "type": "group", "label": "Launch", "x": 0, "y": 0, "width": 600, "height": 400 },
            { "id": "t", "type": "text", "text": "# Pricing

Three tiers.", "x": 40, "y": 60, "width": 250, "height": 120 }
        ] });
        let result = handle_tool_call(
            &conn,
            "import_json_canvas",
            Some(json!({ "project_id": project, "canvas": canvas })),
        )
        .unwrap();
        let imported: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(imported["cases"].as_array().unwrap().len(), 2);
        let result = handle_tool_call(
            &conn,
            "export_json_canvas",
            Some(json!({ "project_id": project })),
        )
        .unwrap();
        let exported: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        let kinds: Vec<&str> = exported["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["type"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["group", "text"]);
        assert!(handle_tool_call(
            &conn,
            "import_json_canvas",
            Some(json!({ "project_id": project }))
        )
        .is_err());
    }

    #[test]
    fn test_mcp_set_case_position_places_a_case_and_refuses_bad_coordinates() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let root = queries::create_note(
            &conn,
            note_input(&project, "Root", crate::models::note::NoteType::Idea),
        )
        .unwrap();
        handle_tool_call(
            &conn,
            "set_case_position",
            Some(json!({ "note_id": root, "x": 320, "y": 48.5 })),
        )
        .unwrap();
        assert_eq!(
            cases::case_positions(&conn, Some(&project)).unwrap(),
            [cases::CasePosition {
                id: root.clone(),
                x: 320.0,
                y: 48.5
            }]
        );
        for args in [
            json!({ "note_id": root, "x": "320", "y": 0 }),
            json!({ "note_id": root, "y": 0 }),
            json!({ "x": 0, "y": 0 }),
        ] {
            assert!(handle_tool_call(&conn, "set_case_position", Some(args)).is_err());
        }
        assert_eq!(cases::case_positions(&conn, None).unwrap()[0].x, 320.0);
    }

    #[test]
    fn test_mcp_sub_case_arguments_are_validated_without_writing() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let root = queries::create_note(
            &conn,
            note_input(&project, "Root", crate::models::note::NoteType::Idea),
        )
        .unwrap();
        let count = |conn: &Connection| -> i64 {
            conn.query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
                .unwrap()
        };
        let before = count(&conn);
        for (arguments, expected) in [
            (
                json!({ "parent_id": root, "title": "x", "note_type": "note", "content_kind": "video" }),
                "invalid content_kind",
            ),
            (
                json!({ "project_id": project, "title": "x", "note_type": "note", "content_kind": "image" }),
                "only accepted with parent_id",
            ),
            (
                json!({ "title": "x", "note_type": "note" }),
                "project_id is required",
            ),
            (
                json!({ "parent_id": 42, "title": "x", "note_type": "note" }),
                "parent_id must be a string",
            ),
            (
                json!({ "parent_id": "missing", "title": "x", "note_type": "note" }),
                "Parent case not found",
            ),
        ] {
            let error = handle_tool_call(&conn, "add_note", Some(arguments)).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
        assert_eq!(count(&conn), before);
        let error = handle_tool_call(
            &conn,
            "list_sub_cases",
            Some(json!({ "note_id": root, "max_depth": 0 })),
        )
        .unwrap_err();
        assert!(error.contains("max_depth"), "{error}");
        let error = handle_tool_call(
            &conn,
            "add_provenance",
            Some(json!({ "note_id": root, "kind": "source" })),
        )
        .unwrap_err();
        assert!(error.contains("label, a url or an excerpt"), "{error}");
        let error = handle_tool_call(
            &conn,
            "move_case",
            Some(json!({ "note_id": root, "parent_id": 1 })),
        )
        .unwrap_err();
        assert!(error.contains("string or null"), "{error}");
    }

    // ───────────── Agent-sized listings, new tools and JSON returns (MCP audit) ─────────────

    fn call(conn: &Connection, tool: &str, args: Value) -> Value {
        let result = handle_tool_call(conn, tool, Some(args)).unwrap();
        serde_json::from_str(text_of(&result)).unwrap()
    }

    fn add(conn: &Connection, args: Value) -> String {
        call(conn, "add_note", args)["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    /// v28 — an agent proposes a profile line; nothing is written until the person adds it, and
    /// then agents read it (get_work_profile, get_handoff), not as a decision to repeat.
    /// 05/10 sorting: work done in a new case left the original open, and parents stayed open
    /// after their last step. A top-level case points at open cases with a close title; resolving
    /// the last open sub-case points at its parent.
    #[test]
    fn test_add_note_points_at_existing_cases_and_the_last_step_at_its_parent() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Humcase", None).unwrap();
        let other = queries::create_project(&conn, "Other", None).unwrap();
        let dictated = add(
            &conn,
            json!({ "project_id": project, "title": "Mémoire qui reste propre : classer et archiver", "note_type": "idea" }),
        );
        add(
            &conn,
            json!({ "project_id": other, "title": "Mémoire propre ailleurs : classer", "note_type": "idea" }),
        );
        let done = add(
            &conn,
            json!({ "project_id": project, "title": "Classer la mémoire propre (fait)", "note_type": "todo" }),
        );
        call(
            &conn,
            "update_note_status",
            json!({ "note_id": done, "status": "resolved", "resolution_comment": "fait" }),
        );

        let created = call(
            &conn,
            "add_note",
            json!({ "project_id": project, "title": "Lot 3 : mémoire propre, classer les cases", "note_type": "todo" }),
        );
        let existing = created["existing_cases"].as_array().unwrap();
        // Only the open case of the same project: not the resolved one, not the other project.
        assert_eq!(existing.len(), 1, "{created}");
        assert_eq!(existing[0]["id"], dictated);
        assert!(created["hint"].as_str().unwrap().contains("move_case"));
        // A sub-case is filed already; an unrelated title gets nothing.
        let step = call(
            &conn,
            "add_note",
            json!({ "parent_id": dictated, "title": "Classer la mémoire propre", "note_type": "todo" }),
        );
        assert!(step.get("existing_cases").is_none());
        let unrelated = call(
            &conn,
            "add_note",
            json!({ "project_id": project, "title": "Signature Windows", "note_type": "todo" }),
        );
        assert!(unrelated.get("existing_cases").is_none());

        // Two steps: resolving the first says nothing, the last points at the parent.
        let step = step["id"].as_str().unwrap().to_string();
        let last = add(
            &conn,
            json!({ "parent_id": dictated, "title": "Archiver", "note_type": "todo" }),
        );
        let first = call(
            &conn,
            "update_note_status",
            json!({ "note_id": step, "status": "resolved", "resolution_comment": "fait" }),
        );
        assert!(first.get("parent_ready").is_none());
        let closing = call(
            &conn,
            "update_note_status",
            json!({ "note_id": last, "status": "resolved", "resolution_comment": "fait" }),
        );
        assert_eq!(closing["parent_ready"]["id"], dictated);
        // Once the parent is resolved, nothing more.
        call(
            &conn,
            "update_note_status",
            json!({ "note_id": dictated, "status": "resolved", "resolution_comment": "fait" }),
        );
        let again = call(
            &conn,
            "update_note_status",
            json!({ "note_id": last, "status": "resolved", "resolution_comment": "refait", "append": true }),
        );
        assert!(again.get("parent_ready").is_none());
    }

    #[test]
    fn test_work_profile_proposed_by_an_agent_then_read_by_agents() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Shop", None).unwrap();
        conn.execute(
            "UPDATE projects SET space = 'work' WHERE id = ?1",
            [&project],
        )
        .unwrap();
        let proposed = call(
            &conn,
            "propose_profile_rule",
            json!({ "text": "Mockup before any visible feature", "reason": "asked twice" }),
        );
        assert_eq!(proposed["space"], "work");
        assert_eq!(proposed["already_proposed"], false);
        let again = call(
            &conn,
            "propose_profile_rule",
            json!({ "text": "mockup before any visible feature" }),
        );
        assert_eq!(again["already_proposed"], true);
        let read = call(&conn, "get_work_profile", json!({ "space": "work" }));
        assert_eq!(read["pending"].as_array().unwrap().len(), 1);
        assert_eq!(read["pending"][0]["detail"], "asked twice");
        let profile_id = read["profiles"][0]["note_id"].as_str().unwrap().to_string();
        assert!(
            !read["profiles"][0]["content"]
                .as_str()
                .unwrap()
                .contains("Mockup"),
            "nothing written before the person adds it"
        );
        let brief = crate::i18n::with_language("en", || {
            queries::build_agent_handoff(&conn, Some(&project)).unwrap()
        });
        assert!(!brief.contains("How the person works"), "{brief}");

        crate::i18n::with_language("en", || {
            crate::profile::answer(
                &conn,
                &[crate::profile::ProfileAnswer {
                    id: proposed["id"].as_str().unwrap().into(),
                    accept: true,
                }],
            )
            .unwrap();
        });
        let read = call(&conn, "get_work_profile", json!({}));
        assert_eq!(read["profiles"].as_array().unwrap().len(), 2);
        assert!(read["profiles"][0]["content"]
            .as_str()
            .unwrap()
            .contains("- Mockup before any visible feature (proposed by agent)"));
        assert_eq!(read["profiles"][1]["note_id"], Value::Null);
        let brief = crate::i18n::with_language("en", || {
            queries::build_agent_handoff(&conn, Some(&project)).unwrap()
        });
        let section = brief.split("## How the person works").nth(1).expect(&brief);
        let section = section.split("\n## ").next().unwrap();
        assert!(section.contains(&format!("[{profile_id}]")), "{brief}");
        assert!(section.contains("### Rules"), "{brief}");
        assert!(
            section.contains("- Mockup before any visible feature (proposed by agent)"),
            "{brief}"
        );
        let decisions = brief.split("## Latest decisions").nth(1).unwrap_or("");
        assert!(!decisions.contains(&profile_id), "{brief}");
        let error = handle_tool_call(&conn, "get_work_profile", Some(json!({ "space": "home" })))
            .unwrap_err();
        assert!(error.contains("invalid space"), "{error}");
    }

    /// A listing never carries the raw dictation, nor the content unless asked for; it is
    /// compact JSON with a typed copy.
    #[test]
    fn test_list_notes_is_agent_sized_and_never_returns_raw_content() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let mut input = note_input(&project, "Dictated", crate::models::note::NoteType::Idea);
        input.content = Some(format!("Start of the content. {}", "word ".repeat(400)));
        input.raw_content = Some("secret raw dictation".into());
        queries::create_note(&conn, input).unwrap();

        let result = handle_tool_call(&conn, "list_notes", Some(json!({}))).unwrap();
        let text = text_of(&result);
        assert!(!text.contains('\n'), "compact JSON expected");
        assert!(!text.contains("secret raw dictation"), "{text}");
        assert!(!text.contains("raw_content"), "{text}");
        assert_eq!(result["structuredContent"]["total"], 1);
        let note = &result["structuredContent"]["notes"][0];
        assert!(note.get("content").is_none(), "concise has no content");
        let snippet = note["snippet"].as_str().unwrap();
        assert!(snippet.starts_with("Start of the content."), "{snippet}");
        assert!(snippet.chars().count() <= 201, "{snippet}");
        assert_eq!(note["tags"], json!([]));

        let detailed = call(
            &conn,
            "list_notes",
            json!({ "response_format": "detailed" }),
        );
        let note = &detailed["notes"][0];
        assert!(note["content"]
            .as_str()
            .unwrap()
            .starts_with("Start of the content."));
        assert!(note.get("resolution_comment").is_some());
        assert!(!detailed.to_string().contains("secret raw dictation"));
        // Searching the dictation finds the case, but the passage shown is never the raw text.
        let found = call(&conn, "list_notes", json!({ "search_query": "secret" }));
        assert_eq!(found["total"], 1);
        assert!(!found.to_string().contains("secret raw dictation"));
    }

    /// `limit` and `offset` page the listing; the total and a hint say what was left out.
    #[test]
    fn test_list_notes_pages_with_total_and_hint() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        for i in 0..7 {
            queries::create_note(
                &conn,
                note_input(
                    &project,
                    &format!("Case {i}"),
                    crate::models::note::NoteType::Todo,
                ),
            )
            .unwrap();
        }
        let first = call(&conn, "list_notes", json!({ "limit": 3 }));
        assert_eq!(
            (
                first["total"].clone(),
                first["notes"].as_array().unwrap().len()
            ),
            (json!(7), 3)
        );
        assert!(
            first["hint"].as_str().unwrap().contains("offset 3"),
            "{first}"
        );
        let last = call(&conn, "list_notes", json!({ "limit": 3, "offset": 6 }));
        assert_eq!(last["notes"].as_array().unwrap().len(), 1);
        assert!(last.get("hint").is_none(), "nothing left out: {last}");
        let default = call(&conn, "list_notes", json!({}));
        assert_eq!(default["limit"], 50);
        for args in [
            json!({ "limit": 0 }),
            json!({ "limit": 201 }),
            json!({ "offset": -1 }),
        ] {
            assert!(handle_tool_call(&conn, "list_notes", Some(args)).is_err());
        }
    }

    /// An unknown status, type or format is an error, not an empty list that reads as "nothing".
    #[test]
    fn test_list_notes_refuses_unknown_filter_values() {
        let conn = setup_db();
        for (args, expected) in [
            (json!({ "status": "done" }), "invalid status"),
            (json!({ "note_type": "pr" }), "invalid note_type"),
            (json!({ "note_type": ["todo", "pr"] }), "invalid note_type"),
            (
                json!({ "response_format": "full" }),
                "invalid response_format",
            ),
            (json!({ "due_before": "31/12/2026" }), "invalid due_before"),
            (
                json!({ "updated_since": "yesterday" }),
                "invalid updated_since",
            ),
            // Not ASCII: refused, never a panic on a cut inside a character.
            (
                json!({ "updated_since": "2026-10-0é" }),
                "invalid updated_since",
            ),
            (json!({ "space": "personal" }), "invalid space"),
            (json!({ "include_archived": "yes" }), "true or false"),
        ] {
            let error = handle_tool_call(&conn, "list_notes", Some(args)).unwrap_err();
            assert!(error.contains(expected), "{expected}: {error}");
        }
    }

    #[test]
    fn test_get_brief_refuses_a_since_that_is_not_ascii() {
        let conn = setup_db();
        let error = handle_tool_call(&conn, "get_brief", Some(json!({ "since": "2026-10-0é" })))
            .unwrap_err();
        assert!(error.contains("invalid since"), "{error}");
    }

    #[test]
    fn test_list_notes_filters() {
        let conn = setup_db();
        let work = queries::create_project_in_space(&conn, "Work", None, Some(ProjectSpace::Work))
            .unwrap();
        let private =
            queries::create_project_in_space(&conn, "Home", None, Some(ProjectSpace::Private))
                .unwrap();
        let root = add(
            &conn,
            json!({ "project_id": work, "title": "Root", "note_type": "idea", "tags": ["mcp"] }),
        );
        let child = add(
            &conn,
            json!({ "parent_id": root, "title": "Child", "note_type": "todo", "due_at": "2026-05-10" }),
        );
        let bug = add(
            &conn,
            json!({ "project_id": work, "title": "Bug", "note_type": "bug", "due_at": "2026-12-01" }),
        );
        let home = add(
            &conn,
            json!({ "project_id": private, "title": "Home", "note_type": "note" }),
        );
        handle_tool_call(&conn, "archive_case", Some(json!({ "note_id": home }))).unwrap();
        let ids = |args: Value| -> Vec<String> {
            let mut ids: Vec<String> = call(&conn, "list_notes", args)["notes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n["id"].as_str().unwrap().to_string())
                .collect();
            ids.sort();
            ids
        };
        let sorted = |mut v: Vec<&String>| {
            v.sort();
            v.into_iter().cloned().collect::<Vec<_>>()
        };
        assert_eq!(ids(json!({ "note_type": "bug" })), vec![bug.clone()]);
        assert_eq!(
            ids(json!({ "note_type": ["bug", "todo"] })),
            sorted(vec![&bug, &child])
        );
        assert_eq!(ids(json!({ "tag": "#mcp" })), vec![root.clone()]);
        assert_eq!(ids(json!({ "search_query": "#mcp" })), vec![root.clone()]);
        assert_eq!(ids(json!({ "parent_id": root })), vec![child.clone()]);
        assert_eq!(
            ids(json!({ "parent_id": "root" })),
            sorted(vec![&root, &bug])
        );
        assert_eq!(
            ids(json!({ "parent_id": null })),
            ids(json!({ "parent_id": "root" }))
        );
        assert_eq!(
            ids(json!({ "due_before": "2026-06-01" })),
            vec![child.clone()]
        );
        assert_eq!(ids(json!({ "due_after": "2026-06-01" })), vec![bug.clone()]);
        assert_eq!(
            ids(json!({ "updated_since": "2999-01-01" })),
            Vec::<String>::new()
        );
        assert_eq!(ids(json!({ "updated_since": "2000-01-01" })).len(), 3);
        // Archived cases are left out unless asked for.
        assert!(!ids(json!({})).contains(&home));
        assert_eq!(
            ids(json!({ "include_archived": true, "space": "private" })),
            vec![home.clone()]
        );
        assert_eq!(ids(json!({ "space": "private" })), Vec::<String>::new());
    }

    /// With a search, the closest case comes first (a title match weighs more than a passing word).
    #[test]
    fn test_list_notes_search_ranks_by_relevance() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let mut passing = note_input(&project, "Meeting", crate::models::note::NoteType::Note);
        passing.content = Some("We also spoke about the invoice.".into());
        queries::create_note(&conn, passing).unwrap();
        let mut about = note_input(
            &project,
            "Invoice export",
            crate::models::note::NoteType::Todo,
        );
        about.content = Some("The invoice export misses the invoice number.".into());
        let about = queries::create_note(&conn, about).unwrap();
        let found = call(&conn, "list_notes", json!({ "search_query": "invoice" }));
        assert_eq!(found["total"], 2);
        assert_eq!(found["notes"][0]["id"], about.as_str());
        assert!(found["notes"][0]["snippet"]
            .as_str()
            .unwrap()
            .contains("invoice"));
    }

    #[test]
    fn test_list_sub_cases_pages_concise_summaries() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let mut input = note_input(&project, "Root", crate::models::note::NoteType::Idea);
        input.raw_content = Some("raw words".into());
        let root = queries::create_note(&conn, input).unwrap();
        for i in 0..3 {
            let mut child = note_input(
                "",
                &format!("Child {i}"),
                crate::models::note::NoteType::Todo,
            );
            child.raw_content = Some("raw child words".into());
            cases::create_sub_case(&conn, &root, ContentKind::Text, child).unwrap();
        }
        let page = call(
            &conn,
            "list_sub_cases",
            json!({ "note_id": root, "limit": 2 }),
        );
        assert_eq!(page["total"], 3);
        assert_eq!(page["sub_cases"].as_array().unwrap().len(), 2);
        assert_eq!(page["sub_cases"][0]["depth"], 1);
        assert!(page["hint"].is_string());
        assert!(!page.to_string().contains("raw child words"));
    }

    /// Writes answer with JSON carrying the identifiers and the new revision.
    #[test]
    fn test_mutations_return_json() {
        let conn = setup_db();
        let project = call(
            &conn,
            "create_project",
            json!({ "name": "Json", "space": "work" }),
        );
        let project_id = project["id"].as_str().unwrap().to_string();
        let created = handle_tool_call(
            &conn,
            "add_note",
            Some(json!({
                "project_id": project_id, "title": "Task", "note_type": "todo",
                "content": "Do it", "due_at": "2026-12-31"
            })),
        )
        .unwrap();
        let typed = &created["structuredContent"];
        let id = typed["id"].as_str().unwrap().to_string();
        assert_eq!(typed["project_id"], project_id.as_str());
        assert_eq!(typed["parent_id"], Value::Null);
        assert_eq!(typed["due_at"], "2026-12-31");
        assert_eq!(typed["status"], "open");
        assert_eq!(
            serde_json::from_str::<Value>(text_of(&created)).unwrap(),
            *typed
        );
        assert_eq!(
            queries::get_note_by_id(&conn, &id).unwrap().content,
            "Do it"
        );

        let status = handle_tool_call(
            &conn,
            "update_note_status",
            Some(json!({ "note_id": id, "status": "in_progress" })),
        )
        .unwrap();
        assert_eq!(status["structuredContent"]["status"], "in_progress");
        assert_eq!(
            status["structuredContent"]["revision"],
            queries::get_note_by_id(&conn, &id).unwrap().revision
        );
        let due = call(
            &conn,
            "set_due_date",
            json!({ "note_id": id, "due_at": "" }),
        );
        assert_eq!(due["due_at"], Value::Null);
        let moved = call(&conn, "move_case", json!({ "note_id": id }));
        assert_eq!(moved["id"], id.as_str());
        let placed = call(
            &conn,
            "set_case_position",
            json!({ "note_id": id, "x": 1, "y": 2 }),
        );
        assert_eq!(placed["x"], 1.0);
        let bug = call(
            &conn,
            "report_codebase_bug",
            json!({ "project_id": project_id, "title": "Crash", "file_path": "src/a.rs",
                    "line_number": 3, "error_message": "boom" }),
        );
        assert_eq!(bug["note_type"], "bug");
        let inbox = call(&conn, "get_or_create_inbox", json!({ "space": "work" }));
        assert_eq!(inbox["is_inbox"], true);
    }

    /// A due date given at creation is checked before anything is written.
    #[test]
    fn test_add_note_refuses_an_impossible_due_date_without_writing() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let error = handle_tool_call(
            &conn,
            "add_note",
            Some(json!({ "project_id": project, "title": "X", "note_type": "todo", "due_at": "2026-02-31" })),
        )
        .unwrap_err();
        assert!(error.contains("invalid due_at"), "{error}");
        assert!(queries::get_notes(&conn, None, None, None)
            .unwrap()
            .is_empty());
    }

    /// The report alone, without a status change (the CLI `report` through MCP).
    #[test]
    fn test_update_note_status_records_a_report_without_status() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let id = queries::create_note(
            &conn,
            note_input(&project, "Case", crate::models::note::NoteType::Todo),
        )
        .unwrap();
        let written = call(
            &conn,
            "update_note_status",
            json!({ "note_id": id, "resolution_comment": "Half done: parser written" }),
        );
        assert_eq!(written["status"], "open");
        assert_eq!(written["resolution_comment"], "Half done: parser written");
    }

    #[test]
    fn test_report_codebase_bug_line_number_is_a_positive_integer() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        for line in [json!(1.5), json!(0), json!("12")] {
            let error = handle_tool_call(
                &conn,
                "report_codebase_bug",
                Some(
                    json!({ "project_id": project, "title": "Bug", "file_path": "a.rs",
                             "error_message": "boom", "line_number": line }),
                ),
            )
            .unwrap_err();
            assert!(error.contains("line_number"), "{error}");
        }
        assert!(queries::get_notes(&conn, None, None, None)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_unknown_tool_points_to_tools_list() {
        let conn = setup_db();
        let error = handle_tool_call(&conn, "nope", None).unwrap_err();
        assert!(error.contains("Run tools/list"), "{error}");
    }

    /// Archiving sets a case aside without erasing it, and can be undone.
    #[test]
    fn test_archive_case_sets_aside_and_brings_back() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let id = queries::create_note(
            &conn,
            note_input(&project, "Case", crate::models::note::NoteType::Todo),
        )
        .unwrap();
        let archived = call(&conn, "archive_case", json!({ "note_id": id }));
        assert!(archived["archived_at"].is_string());
        assert_eq!(archived["revision"], 1);
        assert!(!queries::build_agent_handoff(&conn, None)
            .unwrap()
            .contains("Case ["));
        let back = call(
            &conn,
            "archive_case",
            json!({ "note_id": id, "archived": false }),
        );
        assert_eq!(back["archived_at"], Value::Null);
        assert!(
            handle_tool_call(&conn, "archive_case", Some(json!({ "note_id": "missing" }))).is_err()
        );
    }

    /// Renaming and recoloring a project; a bad color writes nothing, not even the name.
    #[test]
    fn test_update_project_renames_and_recolors() {
        let conn = setup_db();
        let id = queries::create_project(&conn, "Old", None).unwrap();
        let error = handle_tool_call(
            &conn,
            "update_project",
            Some(json!({ "project_id": id, "name": "New", "color": "red" })),
        )
        .unwrap_err();
        assert!(error.contains("invalid project color"), "{error}");
        assert_eq!(queries::get_projects(&conn).unwrap()[0].name, "Old");
        let project = call(
            &conn,
            "update_project",
            json!({ "project_id": id, "name": "New", "color": "#A1B2C3" }),
        );
        assert_eq!(project["name"], "New");
        assert_eq!(project["color"], "#a1b2c3");
        assert!(
            handle_tool_call(&conn, "update_project", Some(json!({ "project_id": id }))).is_err()
        );
    }

    /// Several positions at once, all or none.
    #[test]
    fn test_set_case_positions_is_all_or_none() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let a = queries::create_note(
            &conn,
            note_input(&project, "A", crate::models::note::NoteType::Idea),
        )
        .unwrap();
        let b = queries::create_note(
            &conn,
            note_input(&project, "B", crate::models::note::NoteType::Idea),
        )
        .unwrap();
        assert!(handle_tool_call(
            &conn,
            "set_case_positions",
            Some(json!({ "positions": [
                { "note_id": a, "x": 10, "y": 10 },
                { "note_id": "missing", "x": 0, "y": 0 }
            ] })),
        )
        .is_err());
        assert!(cases::case_positions(&conn, None).unwrap().is_empty());
        let placed = call(
            &conn,
            "set_case_positions",
            json!({ "positions": [
                { "note_id": a, "x": 10, "y": 10 },
                { "note_id": b, "x": 300, "y": 10 }
            ] }),
        );
        assert_eq!(placed["placed"], 2);
        assert_eq!(cases::case_positions(&conn, None).unwrap().len(), 2);
    }

    /// Duplicate check: closest first, at most five, the compared case left out, same space only.
    #[test]
    fn test_find_similar_notes_is_capped_and_stays_in_the_space() {
        let conn = setup_db();
        let work = queries::create_project_in_space(&conn, "Work", None, Some(ProjectSpace::Work))
            .unwrap();
        let home =
            queries::create_project_in_space(&conn, "Home", None, Some(ProjectSpace::Private))
                .unwrap();
        let source = add(
            &conn,
            json!({ "project_id": work, "title": "Invoice export bug", "note_type": "bug" }),
        );
        for i in 0..7 {
            add(
                &conn,
                json!({ "project_id": work, "title": format!("Invoice export item {i}"), "note_type": "todo" }),
            );
        }
        let private = add(
            &conn,
            json!({ "project_id": home, "title": "Invoice export at home", "note_type": "note" }),
        );
        let similar = call(
            &conn,
            "find_similar_notes",
            json!({ "text": "Invoice export bug", "note_id": source }),
        );
        let notes = similar["notes"].as_array().unwrap();
        assert_eq!(notes.len(), 5);
        assert!(notes
            .iter()
            .all(|n| n["id"] != source.as_str() && n["id"] != private.as_str()));
        assert!(notes.iter().all(|n| n.get("content").is_none()));
        let anywhere = call(
            &conn,
            "find_similar_notes",
            json!({ "text": "export at home", "limit": 1 }),
        );
        assert_eq!(anywhere["notes"][0]["id"], private.as_str());
        assert!(
            call(&conn, "find_similar_notes", json!({ "text": "a b" }))["notes"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(handle_tool_call(
            &conn,
            "find_similar_notes",
            Some(json!({ "text": "x", "limit": 6 }))
        )
        .is_err());
    }

    /// get_brief lists results waiting for the person and reworks with their reason, and marks
    /// nothing as seen: the person's own brief is unchanged afterwards.
    #[test]
    fn test_get_brief_reads_without_marking_anything_seen() {
        use crate::db::journal::{self, Actor, Decision};
        let conn = setup_db();
        journal::install(&conn, &Actor::Agent(Some("Codex".into()))).unwrap();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let done = add(
            &conn,
            json!({ "project_id": project, "title": "Done", "note_type": "todo" }),
        );
        let redo = add(
            &conn,
            json!({ "project_id": project, "title": "Redo", "note_type": "todo" }),
        );
        for id in [&done, &redo] {
            call(
                &conn,
                "update_note_status",
                json!({ "note_id": id, "status": "resolved", "resolution_comment": "Verified" }),
            );
        }
        journal::set_actor(&conn, &Actor::Human).unwrap();
        let revision = queries::get_note_by_id(&conn, &redo).unwrap().revision;
        journal::review_case(
            &conn,
            &redo,
            Decision::RequestRework,
            revision,
            Some("Missing the cost"),
        )
        .unwrap();
        journal::set_actor(&conn, &Actor::Agent(Some("Codex".into()))).unwrap();

        let seen_before = (
            crate::db::permissions::get_setting(&conn, journal::BRIEF_SEEN_UNTIL).unwrap(),
            conn.query_row("SELECT COUNT(*) FROM case_views", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap(),
        );
        let before = journal::brief(&conn).unwrap();
        let brief = call(&conn, "get_brief", json!({}));
        assert_eq!(brief["to_review"][0]["id"], done.as_str());
        assert_eq!(brief["to_review"][0]["declared_by"], "Codex");
        assert_eq!(brief["to_review"][0]["report"], "Verified");
        assert_eq!(brief["reworks_pending"][0]["id"], redo.as_str());
        assert_eq!(
            brief["reworks_pending"][0]["last_decision"]["comment"],
            "Missing the cost"
        );
        assert!(brief["changes_total"].as_u64().unwrap() >= 3);
        assert_eq!(brief["changes"][0]["actor"], "Codex");
        assert!(brief["changes"][0]["title"].is_string());
        let since = call(
            &conn,
            "get_brief",
            json!({ "since": "2000-01-01T00:00:00Z", "limit": 1 }),
        );
        assert_eq!(since["changes"].as_array().unwrap().len(), 1);
        assert!(since["hint"].is_string());
        assert!(
            handle_tool_call(&conn, "get_brief", Some(json!({ "since": "last week" }))).is_err()
        );

        let seen_after = (
            crate::db::permissions::get_setting(&conn, journal::BRIEF_SEEN_UNTIL).unwrap(),
            conn.query_row("SELECT COUNT(*) FROM case_views", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap(),
        );
        assert_eq!(
            seen_before, seen_after,
            "get_brief must not mark anything as seen"
        );
        // Same brief for the person (its fallback start moves with the clock, not with the read).
        let after = journal::brief(&conn).unwrap();
        assert_eq!(
            (after.to_review, after.changes, after.reworks_pending),
            (before.to_review, before.changes, before.reworks_pending)
        );

        let activity = call(
            &conn,
            "get_case_activity",
            json!({ "note_id": redo, "limit": 2 }),
        );
        assert_eq!(activity["review"]["state"], "rework_requested");
        assert_eq!(activity["decisions"][0]["comment"], "Missing the cost");
        assert_eq!(activity["events"].as_array().unwrap().len(), 2);
        assert!(activity["events_total"].as_u64().unwrap() > 2);
    }

    /// The agents' brief names every case by its id, shows where a sub-case sits, and lists at
    /// most 25 cases to resume, counting the rest; the interface's brief lists them all.
    #[test]
    fn test_agent_handoff_carries_ids_paths_and_is_capped() {
        let conn = setup_db();
        let project = queries::create_project(&conn, "Work", None).unwrap();
        let root = add(
            &conn,
            json!({ "project_id": project, "title": "Launch", "note_type": "idea", "priority": "P0" }),
        );
        let child = add(
            &conn,
            json!({ "parent_id": root, "title": "Pricing", "note_type": "todo", "priority": "P0" }),
        );
        for i in 0..30 {
            add(
                &conn,
                json!({ "project_id": project, "title": format!("Filler {i}"), "note_type": "todo" }),
            );
        }
        let brief = text_of(
            &handle_tool_call(&conn, "get_handoff", Some(json!({ "project_id": project })))
                .unwrap(),
        )
        .to_string();
        assert!(
            brief.contains(&format!("Launch › Pricing [{child}]")),
            "{brief}"
        );
        assert!(brief.contains(&format!("Launch [{root}]")), "{brief}");
        let listed = brief.lines().filter(|l| l.starts_with("- **[")).count();
        assert_eq!(listed, queries::AGENT_HANDOFF_LIMIT);
        assert!(brief.contains("7 more to resume"), "{brief}");
        assert!(brief.contains("list_notes"), "{brief}");
        let full = queries::build_handoff(&conn, Some(&project)).unwrap();
        assert_eq!(full.lines().filter(|l| l.starts_with("- **[")).count(), 32);
        assert!(!full.contains("more to resume"));
    }
}
