//! Agent-sized listings (MCP `list_notes`, `list_sub_cases`, `find_similar_notes`, CLI `list`).
//!
//! A listing returns short summaries, never `raw_content` (the dictation stays readable only
//! through `get_note`): a whole workspace listed with every content was close to a megabyte, more
//! than an agent can read. A page is bounded by `limit` and says how many cases matched.

use super::notes::split_search;
use super::search::{plan_word_search, SEARCH_RANK};
use crate::models::note::{ContentKind, Note, NoteStatus, NoteType};
use rusqlite::{Connection, ToSql};
use serde::Serialize;

/// Longest snippet of a summary, in characters.
pub const SNIPPET_CHARS: usize = 200;

/// Which parent the listed cases must have.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum ParentFilter {
    /// Any case, top-level or not.
    #[default]
    Any,
    /// Top-level cases only.
    TopLevel,
    /// Direct sub-cases of this case.
    Case(String),
}

/// Filters of an agent listing. Values are validated by the caller (MCP arguments).
#[derive(Debug, Default, Clone)]
pub struct NoteListFilter {
    pub project_id: Option<String>,
    pub status: Option<NoteStatus>,
    pub note_types: Vec<NoteType>,
    pub tag: Option<String>,
    pub parent: ParentFilter,
    /// `YYYY-MM-DD`: due on or before this day.
    pub due_before: Option<String>,
    /// `YYYY-MM-DD`: due on or after this day.
    pub due_after: Option<String>,
    /// ISO 8601: modified at or after this instant.
    pub updated_since: Option<String>,
    pub include_archived: bool,
    /// `private` or `work`: cases of the projects of this space.
    pub space: Option<String>,
    /// Words searched as `plan_word_search` does; a word starting with `#` filters on that tag.
    pub search_query: Option<String>,
}

/// Content and report of a detailed summary (`null` when absent).
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct NoteDetails {
    /// The Markdown content; `null` for a sketch (its scene is read through `get_note`).
    pub content: Option<String>,
    pub resolution_comment: Option<String>,
}

/// A case as listed for an agent: what it needs to choose, never the raw dictation.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub note_type: NoteType,
    pub status: NoteStatus,
    pub priority: Option<String>,
    pub tags: Vec<String>,
    pub due_at: Option<String>,
    pub parent_id: Option<String>,
    pub project_id: String,
    pub updated_at: String,
    pub revision: i64,
    pub archived_at: Option<String>,
    pub content_kind: ContentKind,
    /// Start of the content, or the passage that matched a search; `null` when empty or a sketch.
    pub snippet: Option<String>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub details: Option<NoteDetails>,
}

/// One page of a listing.
#[derive(Debug, Clone)]
pub struct NotePage {
    /// Cases matching the filters, all pages together.
    pub total: i64,
    pub notes: Vec<NoteSummary>,
}

fn tags_of(tags: Option<&str>) -> Vec<String> {
    tags.and_then(|t| serde_json::from_str::<Vec<String>>(t).ok())
        .unwrap_or_default()
}

/// One line of at most `max` characters: whitespace collapsed, an ellipsis when cut.
fn one_line(text: &str, max: usize) -> Option<String> {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    if collapsed.chars().count() <= max {
        return Some(collapsed);
    }
    let mut cut: String = collapsed.chars().take(max).collect();
    cut.push('…');
    Some(cut)
}

/// Snippet of a case's content: its first characters, nothing for a sketch (a JSON scene).
pub fn content_snippet(content: &str, kind: ContentKind) -> Option<String> {
    if kind == ContentKind::Sketch {
        return None;
    }
    one_line(content, SNIPPET_CHARS)
}

impl NoteSummary {
    /// Summary of a note already read in full (sub-case listing).
    pub fn from_note(note: &Note, detailed: bool) -> Self {
        NoteSummary {
            id: note.id.clone(),
            title: note.title.clone(),
            note_type: note.note_type,
            status: note.status.clone(),
            priority: note.priority.clone(),
            tags: tags_of(note.tags.as_deref()),
            due_at: note.due_at.clone(),
            parent_id: note.parent_id.clone(),
            project_id: note.project_id.clone(),
            updated_at: note.updated_at.clone(),
            revision: note.revision,
            archived_at: note.archived_at.clone(),
            content_kind: note.content_kind,
            snippet: content_snippet(&note.content, note.content_kind),
            details: detailed.then(|| NoteDetails {
                content: (note.content_kind != ContentKind::Sketch).then(|| note.content.clone()),
                resolution_comment: note.resolution_comment.clone(),
            }),
        }
    }
}

/// Columns read for a summary. `content_expr` is the full content (detailed) or its start;
/// `snippet_expr` the FTS5 passage of a search, or `NULL`.
fn summary_columns(content_expr: &str, snippet_expr: &str) -> String {
    format!(
        "n.id, n.title, n.note_type, n.status, n.priority, n.tags, n.due_at, n.parent_id,
         n.project_id, n.updated_at, n.revision, n.archived_at, n.content_kind,
         {content_expr}, n.resolution_comment, {snippet_expr}"
    )
}

fn map_summary(row: &rusqlite::Row, detailed: bool) -> rusqlite::Result<NoteSummary> {
    let kind: ContentKind = row.get(12)?;
    let content: Option<String> = row.get(13)?;
    let content = content.unwrap_or_default();
    let matched: Option<String> = row.get(15)?;
    let snippet = if kind == ContentKind::Sketch {
        None
    } else {
        matched
            .and_then(|m| one_line(&m, SNIPPET_CHARS + 40))
            .or_else(|| content_snippet(&content, kind))
    };
    let tags: Option<String> = row.get(5)?;
    Ok(NoteSummary {
        id: row.get(0)?,
        title: row.get(1)?,
        note_type: row.get(2)?,
        status: row.get(3)?,
        priority: row.get(4)?,
        tags: tags_of(tags.as_deref()),
        due_at: row.get(6)?,
        parent_id: row.get(7)?,
        project_id: row.get(8)?,
        updated_at: row.get(9)?,
        revision: row.get(10)?,
        archived_at: row.get(11)?,
        content_kind: kind,
        snippet,
        details: if detailed {
            Some(NoteDetails {
                content: (kind != ContentKind::Sketch).then_some(content),
                resolution_comment: row.get(14)?,
            })
        } else {
            None
        },
    })
}

/// FTS5 passage of the content column (never the raw dictation, column 2).
const SEARCH_SNIPPET: &str = "snippet(notes_fts, 1, '', '', '…', 32)";

/// A page of cases matching `filter`, most relevant first when searching, otherwise by
/// priority then most recent. Cases in the trash are never listed; archived ones only with
/// `include_archived`.
pub fn list_notes_page(
    conn: &Connection,
    filter: &NoteListFilter,
    offset: i64,
    limit: i64,
    detailed: bool,
) -> Result<NotePage, String> {
    let mut joins = String::new();
    let mut clauses: Vec<String> = vec!["n.deleted_at IS NULL".to_string()];
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    let mut searching = false;
    if let Some(query) = filter.search_query.as_deref().map(str::trim) {
        let (text, tags) = split_search(query, true);
        if !text.is_empty() {
            match plan_word_search(conn, &text).map_err(crate::db::db_error)? {
                Some(plan) => {
                    searching = true;
                    joins.push_str(" JOIN notes_fts ON notes_fts.rowid = n.rowid");
                    clauses.push("notes_fts MATCH ?".to_string());
                    params.push(Box::new(plan.fts));
                    clauses.push("n.rowid IN (SELECT value FROM json_each(?))".to_string());
                    params.push(Box::new(plan.kept));
                }
                // Nothing but separators: nothing to find.
                None => clauses.push("0".to_string()),
            }
        }
        for tag in tags {
            clauses.push("EXISTS (SELECT 1 FROM json_each(n.tags) WHERE value = ?)".to_string());
            params.push(Box::new(tag));
        }
    }
    if !filter.include_archived {
        clauses.push("n.archived_at IS NULL".to_string());
    }
    if let Some(project) = &filter.project_id {
        clauses.push("n.project_id = ?".to_string());
        params.push(Box::new(project.clone()));
    }
    if let Some(space) = &filter.space {
        clauses.push("n.project_id IN (SELECT id FROM projects WHERE space = ?)".to_string());
        params.push(Box::new(space.clone()));
    }
    if let Some(status) = &filter.status {
        clauses.push("n.status = ?".to_string());
        params.push(Box::new(status.as_str()));
    }
    if !filter.note_types.is_empty() {
        let marks = vec!["?"; filter.note_types.len()].join(", ");
        clauses.push(format!("n.note_type IN ({marks})"));
        for kind in &filter.note_types {
            params.push(Box::new(kind.as_str()));
        }
    }
    if let Some(tag) = &filter.tag {
        clauses.push("EXISTS (SELECT 1 FROM json_each(n.tags) WHERE value = ?)".to_string());
        params.push(Box::new(tag.clone()));
    }
    match &filter.parent {
        ParentFilter::Any => {}
        ParentFilter::TopLevel => clauses.push("n.parent_id IS NULL".to_string()),
        ParentFilter::Case(id) => {
            clauses.push("n.parent_id = ?".to_string());
            params.push(Box::new(id.clone()));
        }
    }
    if let Some(day) = &filter.due_before {
        clauses.push("n.due_at IS NOT NULL AND n.due_at <= ?".to_string());
        params.push(Box::new(day.clone()));
    }
    if let Some(day) = &filter.due_after {
        clauses.push("n.due_at IS NOT NULL AND n.due_at >= ?".to_string());
        params.push(Box::new(day.clone()));
    }
    if let Some(instant) = &filter.updated_since {
        clauses.push("n.updated_at >= ?".to_string());
        params.push(Box::new(instant.clone()));
    }

    let from_where = format!(" FROM notes n{joins} WHERE {}", clauses.join(" AND "));
    let values: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let total: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*){from_where}"),
            values.as_slice(),
            |r| r.get(0),
        )
        .map_err(crate::db::db_error)?;

    let content_expr = if detailed {
        "n.content"
    } else {
        "substr(n.content, 1, 600)"
    };
    let (snippet_expr, order) = if searching {
        (SEARCH_SNIPPET, format!("{SEARCH_RANK}, n.updated_at DESC"))
    } else {
        (
            "NULL",
            "n.priority ASC NULLS LAST, n.created_at DESC".to_string(),
        )
    };
    let sql = format!(
        "SELECT {}{from_where} ORDER BY {order} LIMIT ? OFFSET ?",
        summary_columns(content_expr, snippet_expr)
    );
    let mut values = values;
    values.push(&limit);
    values.push(&offset);
    let mut stmt = conn.prepare(&sql).map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map(values.as_slice(), |row| map_summary(row, detailed))
        .map_err(crate::db::db_error)?;
    let mut notes = Vec::new();
    for row in rows {
        // A corrupted row must not make the whole page unusable (same rule as `get_notes`).
        match row {
            Ok(note) => notes.push(note),
            Err(e) => eprintln!("humcase: note skipped (unreadable row): {e}"),
        }
    }
    Ok(NotePage { total, notes })
}

/// Most cases a similarity check returns.
pub const SIMILAR_MAX: i64 = 5;

/// Fewest words a case shares with the checked text to be returned as similar.
const SIMILAR_MIN_WORDS: usize = 2;

/// Cases that look like `text` (a title or a few lines), to check for a duplicate before
/// creating: they share at least [`SIMILAR_MIN_WORDS`] of its words (all of them when it has
/// fewer), common ones left out ("pour", "with": `common_words`), the closest first (title
/// matches weigh more). One shared word, such as a weekday, does not make a duplicate. Restricted
/// to `space` when given; `exclude_id` is left out. Live cases only, archived ones included.
pub fn find_similar_notes(
    conn: &Connection,
    text: &str,
    exclude_id: Option<&str>,
    space: Option<&str>,
    limit: i64,
) -> Result<Vec<NoteSummary>, String> {
    let mut words: Vec<String> = Vec::new();
    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3)
        .map(str::to_lowercase)
        .filter(|w| !crate::common_words::is_common(w))
    {
        if !words.contains(&word) {
            words.push(word);
        }
        if words.len() == 24 {
            break;
        }
    }
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let terms: Vec<String> = words.iter().map(|w| format!("\"{w}\"")).collect();
    // Cases holding enough of the words, counted word by word.
    let mut shared: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    let mut by_word = conn
        .prepare("SELECT rowid FROM notes_fts WHERE notes_fts MATCH ?1")
        .map_err(crate::db::db_error)?;
    for term in &terms {
        let rowids = by_word
            .query_map([term], |r| r.get::<_, i64>(0))
            .map_err(crate::db::db_error)?;
        for rowid in rowids.filter_map(Result::ok) {
            *shared.entry(rowid).or_default() += 1;
        }
    }
    let needed = SIMILAR_MIN_WORDS.min(words.len());
    let kept: Vec<i64> = shared
        .into_iter()
        .filter(|&(_, count)| count >= needed)
        .map(|(rowid, _)| rowid)
        .collect();
    if kept.is_empty() {
        return Ok(Vec::new());
    }
    let kept = serde_json::to_string(&kept).unwrap_or_else(|_| "[]".into());
    let query = terms.join(" OR ");
    let clauses = [
        "notes_fts MATCH ?1".to_string(),
        "n.rowid IN (SELECT value FROM json_each(?5))".to_string(),
        "n.deleted_at IS NULL".to_string(),
        "(?2 IS NULL OR n.id <> ?2)".to_string(),
        "(?3 IS NULL OR n.project_id IN (SELECT id FROM projects WHERE space = ?3))".to_string(),
    ];
    let sql = format!(
        "SELECT {} FROM notes n JOIN notes_fts ON notes_fts.rowid = n.rowid
          WHERE {} ORDER BY {SEARCH_RANK}, n.updated_at DESC LIMIT ?4",
        summary_columns("substr(n.content, 1, 600)", SEARCH_SNIPPET),
        clauses.join(" AND ")
    );
    let mut stmt = conn.prepare(&sql).map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map(
            rusqlite::params![query, exclude_id, space, limit.clamp(1, SIMILAR_MAX), kept],
            |row| map_summary(row, false),
        )
        .map_err(crate::db::db_error)?;
    Ok(rows.filter_map(Result::ok).collect())
}

/// Summaries of the live cases `ids`, in that order (search by meaning), only those of `space`
/// when given.
pub fn note_summaries_by_ids(
    conn: &Connection,
    ids: &[String],
    space: Option<&str>,
) -> Result<Vec<NoteSummary>, String> {
    let sql = format!(
        "SELECT {} FROM notes n
          WHERE n.id = ?1 AND n.deleted_at IS NULL
            AND (?2 IS NULL OR n.project_id IN (SELECT id FROM projects WHERE space = ?2))",
        summary_columns("substr(n.content, 1, 600)", "substr(n.content, 1, 200)")
    );
    let mut stmt = conn.prepare(&sql).map_err(crate::db::db_error)?;
    let mut out = Vec::new();
    for id in ids {
        let mut rows = stmt
            .query_map(rusqlite::params![id, space], |row| map_summary(row, false))
            .map_err(crate::db::db_error)?;
        if let Some(Ok(summary)) = rows.next() {
            out.push(summary);
        }
    }
    Ok(out)
}
