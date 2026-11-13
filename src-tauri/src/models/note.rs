use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

/// What a case is (its nature): a thought, an idea, a task... `note` is shown as "Thought".
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NoteType {
    Bug,
    Todo,
    Idea,
    Note,
    Question,
    Research,
    Decision,
}

impl NoteType {
    pub const ALL: [NoteType; 7] = [
        NoteType::Note,
        NoteType::Idea,
        NoteType::Todo,
        NoteType::Question,
        NoteType::Research,
        NoteType::Decision,
        NoteType::Bug,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            NoteType::Bug => "bug",
            NoteType::Todo => "todo",
            NoteType::Idea => "idea",
            NoteType::Note => "note",
            NoteType::Question => "question",
            NoteType::Research => "research",
            NoteType::Decision => "decision",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }

    /// The accepted values, for error messages and tool schemas: "note|idea|...".
    pub fn expected() -> String {
        Self::ALL.map(NoteType::as_str).join("|")
    }
}

impl ToSql for NoteType {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for NoteType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value
            .as_str()
            .and_then(|s| NoteType::parse(s).ok_or(rusqlite::types::FromSqlError::InvalidType))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NoteStatus {
    Open,
    InProgress,
    Resolved,
}

impl NoteStatus {
    /// Value stored, sent over MCP and printed by the CLI.
    pub fn as_str(&self) -> &'static str {
        match self {
            NoteStatus::Open => "open",
            NoteStatus::InProgress => "in_progress",
            NoteStatus::Resolved => "resolved",
        }
    }
}

impl ToSql for NoteStatus {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for NoteStatus {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().and_then(|s| match s {
            "open" => Ok(NoteStatus::Open),
            "in_progress" => Ok(NoteStatus::InProgress),
            "resolved" => Ok(NoteStatus::Resolved),
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        })
    }
}

/// What a sub-case holds (v22). A top-level case is `Text`; the other kinds are chosen when a
/// sub-case is created (image, link, file, sketch, agent research).
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    #[default]
    Text,
    Image,
    Link,
    File,
    Sketch,
    Research,
}

impl ContentKind {
    pub const ALL: [ContentKind; 6] = [
        ContentKind::Text,
        ContentKind::Image,
        ContentKind::Link,
        ContentKind::File,
        ContentKind::Sketch,
        ContentKind::Research,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ContentKind::Text => "text",
            ContentKind::Image => "image",
            ContentKind::Link => "link",
            ContentKind::File => "file",
            ContentKind::Sketch => "sketch",
            ContentKind::Research => "research",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

/// Scene of an empty sketch, in the Excalidraw file format (`.excalidraw`) a sketch case keeps.
pub const EMPTY_SKETCH: &str = r#"{"type":"excalidraw","version":2,"source":"humcase","elements":[],"appState":{},"files":{}}"#;

/// Largest scene a sketch case keeps (5 MB): a drawing is far below it, a pasted photo is not
/// (images go in an image case).
pub const MAX_SKETCH_BYTES: usize = 5 * 1024 * 1024;

/// A sketch case holds an Excalidraw scene and nothing else: a JSON object of type `excalidraw`
/// with its list of elements. Any other text written in its place would lose the drawing.
pub fn is_sketch_scene(content: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(content).is_ok_and(|scene| {
        scene.get("type").and_then(|v| v.as_str()) == Some("excalidraw")
            && scene.get("elements").is_some_and(|v| v.is_array())
    })
}

impl ToSql for ContentKind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for ContentKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value
            .as_str()
            .and_then(|s| Self::parse(s).ok_or(rusqlite::types::FromSqlError::InvalidType))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Note {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub content: String,
    pub note_type: NoteType,
    pub status: NoteStatus,
    pub resolution_comment: Option<String>,
    pub raw_content: Option<String>,
    pub priority: Option<String>,
    pub git_branch: Option<String>,
    pub tags: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Optional due date, in `YYYY-MM-DD` format (local day, not an instant) — `None` = none.
    pub due_at: Option<String>,
    /// Optimistic concurrency version: incremented on every write of the note (app or agent).
    /// An MCP agent reads this value then passes it as `expected_revision`; any mismatch
    /// (human write in the meantime) refuses the write.
    pub revision: i64,
    /// Archive date (ISO 8601 UTC); `None` for an active note. Absent from old
    /// snapshots (undo of a deletion): then read as active.
    #[serde(default)]
    pub archived_at: Option<String>,
    /// Date the note was moved to the trash (ISO 8601 UTC); `None` for a live note. A note in
    /// the trash appears in no list and its content is frozen (trigger v17).
    #[serde(default)]
    pub deleted_at: Option<String>,
    /// Parent case (v22); `None` for a top-level case. Absent from old snapshots: top level.
    #[serde(default)]
    pub parent_id: Option<String>,
    /// What the case holds (v22); `Text` for old snapshots.
    #[serde(default)]
    pub content_kind: ContentKind,
}

/// Where a case comes from (v22): a source, a link, an agent's research. Append-only.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CaseProvenance {
    pub id: String,
    pub note_id: String,
    /// Short word chosen by the writer (`source`, `link`, `research`, `agent`...).
    pub kind: String,
    pub label: Option<String>,
    pub url: Option<String>,
    pub excerpt: Option<String>,
    /// Name of the agent that recorded it; `None` when a person did.
    pub agent: Option<String>,
    pub created_at: String,
}

/// Snapshot of a note's **previous** values before an AGENT write.
///
/// `fields` is a JSON object of the fields actually modified (title/content/note_type/status/
/// tags/priority/due_at) — `raw_content` never appears in it. `revision` is the old version.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteRevision {
    pub id: String,
    pub note_id: String,
    pub revision: i64,
    pub source: String,
    /// JSON object: field values before the attributed write.
    pub fields: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateNoteInput {
    pub project_id: String,
    pub title: String,
    pub content: Option<String>,
    pub note_type: NoteType,
    pub priority: Option<String>,
    pub git_branch: Option<String>,
    pub tags: Option<String>,
    /// Due date in `YYYY-MM-DD` format (local day) — `None` = none.
    pub due_at: Option<String>,
    /// Original raw dictation, if the note carries one (also recorded as the first fragment, v20).
    pub raw_content: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateNoteInput {
    pub status: Option<NoteStatus>,
    pub content: Option<String>,
    pub resolution_comment: Option<String>,
    pub title: Option<String>,
    pub note_type: Option<NoteType>,
    /// Always refused if provided (batch S6): the raw dictation is never replaced, it is
    /// added to via `append_raw_content`. The field stays declared so that the refusal is explicit
    /// (an unknown field would be silently ignored by serde).
    pub raw_content: Option<String>,
    pub priority: Option<String>,
    pub git_branch: Option<String>,
    pub tags: Option<String>,
    /// Due date in `YYYY-MM-DD` format; an empty string clears the existing due date.
    pub due_at: Option<String>,
}

/// One write on a case, recorded by the database with the actor of the connection (v24).
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct CaseEvent {
    pub id: String,
    pub note_id: String,
    /// `created`, `edited`, `status`, `report`, `moved`, `trashed`, `restored`, `archived`,
    /// `unarchived`.
    pub kind: String,
    /// `human` (the interface), `agent` (MCP), `cli`, `unknown` (connection without an actor).
    pub actor_kind: String,
    /// Agent or CLI name when known; `None` for the person.
    pub actor_name: Option<String>,
    /// Kind-specific facts: `{from, to}` of a status, the parents of a move...
    pub detail: serde_json::Value,
    /// Fields changed by an `edited` event (consecutive edits of one actor are merged).
    pub fields: Vec<String>,
    pub created_at: String,
    /// Latest write merged into this event (`created_at` when not merged).
    pub last_at: String,
}

/// The person's decision on a case's result (v24), distinct from the declared status.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct CaseReview {
    pub id: String,
    pub note_id: String,
    /// `accepted` or `rework_requested`.
    pub decision: String,
    /// Revision of the case the person reviewed.
    pub revision: i64,
    /// Reason of a rework, as the person wrote it.
    pub comment: Option<String>,
    /// 1 for the first rework of the case, 2 for the second...
    pub rework_number: Option<i64>,
    pub created_at: String,
}

/// Where the decision on a case's result stands (see `db::journal::review_state`).
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct ReviewState {
    pub note_id: String,
    /// `none`, `to_review`, `rework_requested`, `accepted`, `accepted_changed`.
    pub state: String,
    pub rework_count: i64,
    /// Latest recorded move to `resolved` (who declared the result, when).
    pub declared: Option<CaseEvent>,
    pub last_review: Option<CaseReview>,
}

/// A case listed by the brief, with what the person needs to decide.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct BriefCase {
    pub note_id: String,
    pub rework_count: i64,
    pub declared: Option<CaseEvent>,
    pub last_review: Option<CaseReview>,
}

/// What changed since the person's last visit (see `db::journal::brief`).
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Brief {
    /// Start of the period (ISO 8601 UTC).
    pub since: String,
    /// `true` when the person never marked the brief as seen: the period is the last 24 hours.
    pub since_is_fallback: bool,
    pub to_review: Vec<BriefCase>,
    /// Non-human writes not seen yet, most recent first.
    pub changes: Vec<CaseEvent>,
    pub reworks_pending: Vec<BriefCase>,
}
