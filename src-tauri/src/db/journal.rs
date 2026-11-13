//! Who did what on a case (v24), and the human decision on a result.
//!
//! Every connection that writes in the user database installs, with [`install`], an actor
//! (the person in the interface, the agent named by its MCP client, the CLI) and TEMP
//! triggers that record each write on a case in `case_events`. The journal is therefore kept
//! by the database itself: a write path cannot forget it, and what it shows is observed, not
//! declared. Connections that rebuild the database (archive restore, data move) do not install
//! it: their technical writes are not the actions of anyone.
//!
//! The decision on a result (`case_reviews`) is distinct from the status an agent declares:
//! `resolved` written by an agent stays a declaration until the person accepts it or asks for a
//! rework. Only the interface may record a decision ([`review_case`] refuses another actor).

use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::models::note::{Brief, BriefCase, CaseEvent, CaseReview, ReviewState};

/// Consecutive edits of the same actor on the same case within this window form one event.
const EDIT_MERGE_MINUTES: i64 = 10;
/// Brief window when the person never marked it as seen.
const BRIEF_FALLBACK_HOURS: i64 = 24;
/// Most events listed by the brief or by a case history.
const EVENTS_LIMIT: i64 = 300;
/// `app_settings` key of "mark all as seen".
pub const BRIEF_SEEN_UNTIL: &str = "brief_seen_until";

const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

/// SQL: the resolution `e` was declared after the decision `r` (a rework handed back, not the
/// result the person reviewed). Ordered by the case's revision, which the status event records:
/// two writes can share a millisecond. An event recorded without it falls back to time, a tie
/// reading as the reviewed result (a decision always follows the result it reviews).
const DECLARED_AFTER_REVIEW: &str = "CASE WHEN json_extract(e.detail, '$.revision') IS NOT NULL
     THEN json_extract(e.detail, '$.revision') > r.revision
     ELSE e.created_at > r.created_at END";

/// Who writes through a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    /// The person, in the interface.
    Human,
    /// An agent through the MCP server, named by its client when it says so.
    Agent(Option<String>),
    /// The command line (a person in a terminal or an agent's shell). `HUMCASE_ACTOR` names it.
    Cli(Option<String>),
}

impl Actor {
    fn kind(&self) -> &'static str {
        match self {
            Actor::Human => "human",
            Actor::Agent(_) => "agent",
            Actor::Cli(_) => "cli",
        }
    }

    fn name(&self) -> Option<String> {
        match self {
            Actor::Human => None,
            Actor::Agent(name) | Actor::Cli(name) => name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(|n| n.chars().take(80).collect()),
        }
    }
}

/// Readable name of an MCP client from its `clientInfo.name` (known clients only; any other
/// name is kept as given).
pub fn agent_display_name(client: &str) -> String {
    match client.trim().to_ascii_lowercase().as_str() {
        "claude-code" | "claude code" => "Claude Code".to_string(),
        "claude-ai" | "claude" | "claude desktop" => "Claude".to_string(),
        "codex" | "codex-mcp-client" | "codex_mcp_client" => "Codex".to_string(),
        "opencode" => "OpenCode".to_string(),
        "cursor" | "cursor-vscode" => "Cursor".to_string(),
        "gemini-cli" | "gemini" => "Gemini CLI".to_string(),
        _ => client.trim().to_string(),
    }
}

/// Installs the journal on `conn` for `actor`: idempotent, to call once per connection that
/// writes in the user database.
pub fn install(conn: &Connection, actor: &Actor) -> Result<(), String> {
    let edited_fields = [
        "title",
        "content",
        "raw_content",
        "note_type",
        "priority",
        "due_at",
        "tags",
    ];
    let changed = edited_fields
        .iter()
        .map(|f| format!("OLD.{f} IS NOT NEW.{f}"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let new_fields = format!(
        "(SELECT json_group_array(f) FROM ({}))",
        edited_fields
            .iter()
            .map(|f| format!("SELECT '{f}' AS f WHERE OLD.{f} IS NOT NEW.{f}"))
            .collect::<Vec<_>>()
            .join(" UNION ALL ")
    );
    let actor_kind = "COALESCE((SELECT kind FROM temp.journal_actor), 'unknown')";
    let actor_name = "(SELECT name FROM temp.journal_actor)";
    let event = |kind: &str, detail: &str| {
        format!(
            "INSERT INTO main.case_events (id, note_id, kind, actor_kind, actor_name, detail, created_at, last_at)
             VALUES (lower(hex(randomblob(16))), NEW.id, {kind}, {actor_kind}, {actor_name}, {detail}, {NOW}, {NOW});"
        )
    };
    // The edit to merge into: the case's latest event, if it is an edit of the same actor
    // made within the window.
    let merge_target = format!(
        "SELECT e.id FROM main.case_events e
         WHERE e.rowid = (SELECT max(rowid) FROM main.case_events WHERE note_id = NEW.id)
           AND e.kind = 'edited' AND e.actor_kind = {actor_kind} AND e.actor_name IS {actor_name}
           AND e.last_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-{EDIT_MERGE_MINUTES} minutes')"
    );
    // A sub-case is inserted, then attached to its parent in the same transaction: that
    // attachment belongs to its creation, not to a move. The creation event is the case's
    // latest, by the same actor, a moment ago, still without a parent.
    let attached_at_creation = format!(
        "SELECT e.id FROM main.case_events e
         WHERE e.rowid = (SELECT max(rowid) FROM main.case_events WHERE note_id = NEW.id)
           AND e.kind = 'created' AND json_extract(e.detail, '$.parent_id') IS NULL
           AND OLD.parent_id IS NULL AND NEW.parent_id IS NOT NULL AND OLD.project_id IS NEW.project_id
           AND e.actor_kind = {actor_kind} AND e.actor_name IS {actor_name}
           AND e.created_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-5 seconds')"
    );
    let moved = format!(
        "INSERT INTO main.case_events (id, note_id, kind, actor_kind, actor_name, detail, created_at, last_at)
         SELECT lower(hex(randomblob(16))), NEW.id, 'moved', {actor_kind}, {actor_name},
                json_object('from_parent', OLD.parent_id, 'to_parent', NEW.parent_id, 'from_project', OLD.project_id, 'to_project', NEW.project_id),
                {NOW}, {NOW}"
    );
    let sql = format!(
        "
        CREATE TEMP TABLE IF NOT EXISTS journal_actor (kind TEXT NOT NULL, name TEXT);
        CREATE TEMP TRIGGER IF NOT EXISTS journal_created AFTER INSERT ON main.notes BEGIN
            {created}
        END;
        CREATE TEMP TRIGGER IF NOT EXISTS journal_status AFTER UPDATE OF status ON main.notes
        WHEN OLD.status IS NOT NEW.status BEGIN
            {status}
        END;
        CREATE TEMP TRIGGER IF NOT EXISTS journal_report AFTER UPDATE OF resolution_comment ON main.notes
        WHEN OLD.resolution_comment IS NOT NEW.resolution_comment BEGIN
            {report}
        END;
        CREATE TEMP TRIGGER IF NOT EXISTS journal_moved AFTER UPDATE OF parent_id, project_id ON main.notes
        WHEN OLD.parent_id IS NOT NEW.parent_id OR OLD.project_id IS NOT NEW.project_id BEGIN
            {moved} WHERE NOT EXISTS ({attached_at_creation});
            UPDATE main.case_events SET detail = json_set(detail, '$.parent_id', NEW.parent_id)
             WHERE id = ({attached_at_creation});
        END;
        CREATE TEMP TRIGGER IF NOT EXISTS journal_trash AFTER UPDATE OF deleted_at ON main.notes
        WHEN OLD.deleted_at IS NOT NEW.deleted_at BEGIN
            {trash}
        END;
        CREATE TEMP TRIGGER IF NOT EXISTS journal_archive AFTER UPDATE OF archived_at ON main.notes
        WHEN OLD.archived_at IS NOT NEW.archived_at BEGIN
            {archive}
        END;
        CREATE TEMP TRIGGER IF NOT EXISTS journal_edited
        AFTER UPDATE OF {edited_list} ON main.notes
        WHEN {changed} BEGIN
            INSERT INTO main.case_events (id, note_id, kind, actor_kind, actor_name, created_at, last_at)
            SELECT lower(hex(randomblob(16))), NEW.id, 'edited', {actor_kind}, {actor_name}, {NOW}, {NOW}
            WHERE NOT EXISTS ({merge_target});
            UPDATE main.case_events
               SET fields = (SELECT json_group_array(value) FROM (
                       SELECT value FROM json_each(main.case_events.fields)
                       UNION SELECT value FROM json_each({new_fields}))),
                   last_at = {NOW}
             WHERE id = ({merge_target});
        END;
        ",
        created = event(
            "'created'",
            "json_object('parent_id', NEW.parent_id, 'note_type', NEW.note_type)"
        ),
        status = event(
            "'status'",
            "json_object('from', OLD.status, 'to', NEW.status, 'revision', NEW.revision)"
        ),
        report = event(
            "'report'",
            "json_object('cleared', json(CASE WHEN NEW.resolution_comment IS NULL OR trim(NEW.resolution_comment) = '' THEN 'true' ELSE 'false' END))"
        ),
        trash = event(
            "CASE WHEN NEW.deleted_at IS NULL THEN 'restored' ELSE 'trashed' END",
            "'{}'"
        ),
        archive = event(
            "CASE WHEN NEW.archived_at IS NULL THEN 'unarchived' ELSE 'archived' END",
            "'{}'"
        ),
        edited_list = edited_fields.join(", "),
    );
    conn.execute_batch(&sql).map_err(crate::db::db_error)?;
    set_actor(conn, actor)
}

/// Changes the actor of `conn` (the MCP server learns its client's name at `initialize`).
pub fn set_actor(conn: &Connection, actor: &Actor) -> Result<(), String> {
    conn.execute("DELETE FROM temp.journal_actor", [])
        .map_err(crate::db::db_error)?;
    conn.execute(
        "INSERT INTO temp.journal_actor (kind, name) VALUES (?1, ?2)",
        params![actor.kind(), actor.name()],
    )
    .map_err(crate::db::db_error)?;
    Ok(())
}

/// The actor of `conn`, `None` when the journal is not installed.
fn current_actor_kind(conn: &Connection) -> Option<String> {
    conn.query_row("SELECT kind FROM temp.journal_actor", [], |r| r.get(0))
        .optional()
        .ok()
        .flatten()
}

/// The name of the agent or command line `conn` works for (`Claude Code`), `None` for the
/// person in the interface, an unnamed actor or a connection without the journal.
pub fn current_agent_name(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT name FROM temp.journal_actor WHERE kind IN ('agent', 'cli')",
        [],
        |r| r.get(0),
    )
    .optional()
    .ok()
    .flatten()
    .flatten()
}

fn map_event(row: &rusqlite::Row) -> rusqlite::Result<CaseEvent> {
    let detail: String = row.get(5)?;
    let fields: String = row.get(6)?;
    Ok(CaseEvent {
        id: row.get(0)?,
        note_id: row.get(1)?,
        kind: row.get(2)?,
        actor_kind: row.get(3)?,
        actor_name: row.get(4)?,
        detail: serde_json::from_str(&detail).unwrap_or(serde_json::Value::Null),
        fields: serde_json::from_str(&fields).unwrap_or_default(),
        created_at: row.get(7)?,
        last_at: row.get(8)?,
    })
}

const EVENT_COLUMNS: &str =
    "e.id, e.note_id, e.kind, e.actor_kind, e.actor_name, e.detail, e.fields, e.created_at, e.last_at";

fn map_review(row: &rusqlite::Row) -> rusqlite::Result<CaseReview> {
    Ok(CaseReview {
        id: row.get(0)?,
        note_id: row.get(1)?,
        decision: row.get(2)?,
        revision: row.get(3)?,
        comment: row.get(4)?,
        rework_number: row.get(5)?,
        created_at: row.get(6)?,
    })
}

const REVIEW_COLUMNS: &str = "id, note_id, decision, revision, comment, rework_number, created_at";

/// Journal of a case, most recent first.
pub fn case_events(conn: &Connection, note_id: &str) -> Result<Vec<CaseEvent>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {EVENT_COLUMNS} FROM case_events e WHERE e.note_id = ?1
             ORDER BY e.last_at DESC, e.rowid DESC LIMIT {EVENTS_LIMIT}"
        ))
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([note_id], map_event)
        .map_err(crate::db::db_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::db::db_error)
}

/// Writes of agents and of the CLI since `since` (ISO 8601 UTC), all cases, oldest first.
/// Feeds the day timeline. Capped like the brief.
pub fn agent_events_since(conn: &Connection, since: &str) -> Result<Vec<CaseEvent>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {EVENT_COLUMNS} FROM case_events e
             WHERE e.actor_kind != 'human' AND e.last_at >= ?1
             ORDER BY e.last_at ASC, e.rowid ASC LIMIT {EVENTS_LIMIT}"
        ))
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([since], map_event)
        .map_err(crate::db::db_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::db::db_error)
}

/// Decisions on a case, most recent first.
pub fn case_reviews(conn: &Connection, note_id: &str) -> Result<Vec<CaseReview>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {REVIEW_COLUMNS} FROM case_reviews WHERE note_id = ?1
             ORDER BY created_at DESC, rowid DESC"
        ))
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([note_id], map_review)
        .map_err(crate::db::db_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::db::db_error)
}

/// The latest time a status reached `resolved` on this case, if recorded.
fn latest_resolution(conn: &Connection, note_id: &str) -> Result<Option<CaseEvent>, String> {
    conn.query_row(
        &format!(
            "SELECT {EVENT_COLUMNS} FROM case_events e
             WHERE e.note_id = ?1 AND e.kind = 'status' AND json_extract(e.detail, '$.to') = 'resolved'
             ORDER BY e.rowid DESC LIMIT 1"
        ),
        [note_id],
        map_event,
    )
    .optional()
    .map_err(crate::db::db_error)
}

/// Where the decision on a case's result stands.
///
/// - `to_review`: the case is `resolved`, an agent or the CLI declared it, and no decision was
///   made since; also the case of a rework the agent handed back.
/// - `rework_requested`: the latest decision asks for a rework and the result was not handed back.
/// - `accepted`, or `accepted_changed` when a non-human write came after the acceptance.
/// - `none`: nothing to decide (never resolved by an agent, or resolved by the person, or
///   resolved before v24 when the author was not recorded).
pub fn review_state(conn: &Connection, note_id: &str) -> Result<ReviewState, String> {
    let status: String = conn
        .query_row("SELECT status FROM notes WHERE id = ?1", [note_id], |r| {
            r.get(0)
        })
        .map_err(crate::db::db_error)?;
    let reviews = case_reviews(conn, note_id)?;
    let rework_count = reviews
        .iter()
        .filter(|r| r.decision == "rework_requested")
        .count() as i64;
    let last_review = reviews.into_iter().next();
    let resolution = latest_resolution(conn, note_id)?;

    let state = match (&resolution, &last_review) {
        (Some(res), _)
            if status == "resolved"
                && res.actor_kind != "human"
                && last_review
                    .as_ref()
                    .is_none_or(|review| declared_after(res, review)) =>
        {
            "to_review"
        }
        (_, Some(review))
            if review.decision == "rework_requested"
                && resolution
                    .as_ref()
                    .is_none_or(|res| !declared_after(res, review)) =>
        {
            "rework_requested"
        }
        (_, Some(review)) if review.decision == "accepted" && status == "resolved" => {
            let changed_after: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM case_events WHERE note_id = ?1
                       AND actor_kind != 'human' AND last_at > ?2)",
                    params![note_id, review.created_at],
                    |r| r.get(0),
                )
                .map_err(crate::db::db_error)?;
            if changed_after {
                "accepted_changed"
            } else {
                "accepted"
            }
        }
        _ => "none",
    };
    Ok(ReviewState {
        note_id: note_id.to_string(),
        state: state.to_string(),
        rework_count,
        declared: resolution,
        last_review,
    })
}

/// Rust side of [`DECLARED_AFTER_REVIEW`].
fn declared_after(resolution: &CaseEvent, review: &CaseReview) -> bool {
    match resolution.detail.get("revision").and_then(|v| v.as_i64()) {
        Some(revision) => revision > review.revision,
        None => resolution.created_at > review.created_at,
    }
}

/// The person's decision on a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Accept,
    RequestRework,
}

/// Records the person's decision on the result of a resolved case, in one transaction.
///
/// `expected_revision` is the revision the person reviewed: a later write refuses the decision
/// (the person would accept something they did not see). Asking for a rework needs a reason,
/// numbers the rework and puts the case back to `open`; accepting changes nothing else.
/// Refused outside the interface: an agent never decides on its own result.
pub fn review_case(
    conn: &Connection,
    note_id: &str,
    decision: Decision,
    expected_revision: i64,
    comment: Option<&str>,
) -> Result<CaseReview, String> {
    if current_actor_kind(conn).as_deref() != Some("human") {
        return Err("only the person can accept a result or ask for a rework".to_string());
    }
    let comment = comment.map(str::trim).filter(|c| !c.is_empty());
    if decision == Decision::RequestRework && comment.is_none() {
        return Err(crate::tr!("review.reasonRequired"));
    }
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let (status, revision, trashed): (String, i64, bool) = tx
        .query_row(
            "SELECT status, revision, deleted_at IS NOT NULL FROM notes WHERE id = ?1",
            [note_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(crate::db::db_error)?
        .ok_or_else(|| crate::tr!("review.notFound"))?;
    if trashed {
        return Err(crate::tr!("review.inTrash"));
    }
    if revision != expected_revision {
        return Err(crate::tr!("review.changed"));
    }
    if status != "resolved" {
        return Err(crate::tr!("review.notResolved"));
    }
    let rework_number: Option<i64> = match decision {
        Decision::Accept => None,
        Decision::RequestRework => Some(
            tx.query_row(
                "SELECT COUNT(*) + 1 FROM case_reviews WHERE note_id = ?1 AND decision = 'rework_requested'",
                [note_id],
                |r| r.get(0),
            )
            .map_err(crate::db::db_error)?,
        ),
    };
    let id = Uuid::new_v4().to_string();
    tx.execute(
        &format!(
            "INSERT INTO case_reviews (id, note_id, decision, revision, comment, rework_number, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, {NOW})"
        ),
        params![
            id,
            note_id,
            match decision {
                Decision::Accept => "accepted",
                Decision::RequestRework => "rework_requested",
            },
            revision,
            comment,
            rework_number
        ],
    )
    .map_err(crate::db::db_error)?;
    if decision == Decision::RequestRework {
        tx.execute(
            &format!("UPDATE notes SET status = 'open', updated_at = {NOW} WHERE id = ?1"),
            [note_id],
        )
        .map_err(crate::db::db_error)?;
        crate::db::queries::bump_revision(&tx, note_id)?;
    }
    // Deciding on a result means the person saw the case.
    tx.execute(
        &format!(
            "INSERT INTO case_views (note_id, seen_at) VALUES (?1, {NOW})
             ON CONFLICT(note_id) DO UPDATE SET seen_at = excluded.seen_at"
        ),
        [note_id],
    )
    .map_err(crate::db::db_error)?;
    let review = tx
        .query_row(
            &format!("SELECT {REVIEW_COLUMNS} FROM case_reviews WHERE id = ?1"),
            [&id],
            map_review,
        )
        .map_err(crate::db::db_error)?;
    tx.commit().map_err(crate::db::db_error)?;
    Ok(review)
}

/// Remembers that the person opened a case in the interface (the brief stops listing what
/// happened before).
pub fn mark_case_seen(conn: &Connection, note_id: &str) -> Result<(), String> {
    conn.execute(
        &format!(
            "INSERT INTO case_views (note_id, seen_at) SELECT id, {NOW} FROM notes WHERE id = ?1
             ON CONFLICT(note_id) DO UPDATE SET seen_at = excluded.seen_at"
        ),
        [note_id],
    )
    .map_err(crate::db::db_error)?;
    Ok(())
}

/// "Mark all as seen": the brief starts again from now.
pub fn mark_brief_seen(conn: &Connection) -> Result<String, String> {
    let now: String = conn
        .query_row(&format!("SELECT {NOW}"), [], |r| r.get(0))
        .map_err(crate::db::db_error)?;
    crate::db::permissions::set_setting(conn, BRIEF_SEEN_UNTIL, &now)?;
    Ok(now)
}

fn brief_cases(conn: &Connection, sql: &str) -> Result<Vec<BriefCase>, String> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(sql).map_err(crate::db::db_error)?;
        let rows = stmt
            .query_map([], |r| r.get(0))
            .map_err(crate::db::db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(crate::db::db_error)?
    };
    ids.iter()
        .map(|id| {
            let state = review_state(conn, id)?;
            Ok(BriefCase {
                note_id: id.clone(),
                rework_count: state.rework_count,
                declared: state.declared,
                last_review: state.last_review,
            })
        })
        .collect()
}

/// What changed since the person's last visit: results to review, writes of agents and of the
/// CLI not seen yet, reworks waiting for an agent. The person's own writes are not listed.
///
/// "Since" is the latest "mark all as seen", or the last 24 hours when there is none
/// (`since_is_fallback`). A write on a case opened after it is not listed; a result to review
/// stays until the person decides.
pub fn brief(conn: &Connection) -> Result<Brief, String> {
    let seen = crate::db::permissions::get_setting(conn, BRIEF_SEEN_UNTIL)?;
    let (since, since_is_fallback) = match seen {
        Some(value) => (value, false),
        None => (
            conn.query_row(
                &format!(
                    "SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-{BRIEF_FALLBACK_HOURS} hours')"
                ),
                [],
                |r| r.get(0),
            )
            .map_err(crate::db::db_error)?,
            true,
        ),
    };
    let live = "n.deleted_at IS NULL AND n.archived_at IS NULL";
    let to_review = brief_cases(
        conn,
        &format!(
            "SELECT n.id FROM notes n
             JOIN case_events e ON e.rowid = (
                 SELECT max(rowid) FROM case_events
                 WHERE note_id = n.id AND kind = 'status' AND json_extract(detail, '$.to') = 'resolved')
             WHERE {live} AND n.status = 'resolved' AND e.actor_kind != 'human'
               AND NOT EXISTS (SELECT 1 FROM case_reviews r WHERE r.note_id = n.id AND NOT ({DECLARED_AFTER_REVIEW}))
             ORDER BY e.created_at DESC"
        ),
    )?;
    let reworks_pending = brief_cases(
        conn,
        &format!(
            "SELECT n.id FROM notes n
             JOIN case_reviews r ON r.rowid = (SELECT max(rowid) FROM case_reviews WHERE note_id = n.id)
             WHERE {live} AND r.decision = 'rework_requested'
               AND NOT EXISTS (
                 SELECT 1 FROM case_events e WHERE e.note_id = n.id AND e.kind = 'status'
                   AND json_extract(e.detail, '$.to') = 'resolved' AND {DECLARED_AFTER_REVIEW})
             ORDER BY r.created_at DESC"
        ),
    )?;
    let changes = {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {EVENT_COLUMNS} FROM case_events e
                 JOIN notes n ON n.id = e.note_id
                 LEFT JOIN case_views v ON v.note_id = e.note_id
                 WHERE {live} AND e.actor_kind != 'human' AND e.last_at > ?1
                   AND (v.seen_at IS NULL OR e.last_at > v.seen_at)
                 ORDER BY e.last_at DESC, e.rowid DESC LIMIT {EVENTS_LIMIT}"
            ))
            .map_err(crate::db::db_error)?;
        let rows = stmt
            .query_map([&since], map_event)
            .map_err(crate::db::db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(crate::db::db_error)?
    };
    Ok(Brief {
        since,
        since_is_fallback,
        to_review,
        changes,
        reworks_pending,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{queries, schema};
    use crate::models::note::{CreateNoteInput, NoteStatus, NoteType, UpdateNoteInput};

    fn db(actor: Actor) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        schema::apply_migrations(&conn).unwrap();
        install(&conn, &actor).unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, space) VALUES ('p', 'Project', 'work')",
            [],
        )
        .unwrap();
        conn
    }

    fn note(conn: &Connection, title: &str) -> String {
        queries::create_note(
            conn,
            CreateNoteInput {
                project_id: "p".to_string(),
                title: title.to_string(),
                content: None,
                note_type: NoteType::Todo,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: None,
            },
        )
        .unwrap()
    }

    fn update(
        status: Option<NoteStatus>,
        content: Option<&str>,
        report: Option<&str>,
    ) -> UpdateNoteInput {
        UpdateNoteInput {
            status,
            content: content.map(str::to_string),
            resolution_comment: report.map(str::to_string),
            title: None,
            note_type: None,
            raw_content: None,
            priority: None,
            git_branch: None,
            tags: None,
            due_at: None,
        }
    }

    fn revision(conn: &Connection, id: &str) -> i64 {
        conn.query_row("SELECT revision FROM notes WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
    }

    /// Every write on a case is recorded with the actor of the connection, without the write
    /// path doing anything: creation, merged edits, status, report, move, trash.
    #[test]
    fn writes_are_journaled_with_the_session_actor() {
        let conn = db(Actor::Agent(Some("Claude Code".into())));
        let id = note(&conn, "Case");
        queries::update_note(&conn, &id, update(None, Some("one"), None)).unwrap();
        queries::update_note(&conn, &id, update(None, Some("two"), None)).unwrap();
        queries::update_note(
            &conn,
            &id,
            update(Some(NoteStatus::Resolved), None, Some("Done, tested")),
        )
        .unwrap();
        set_actor(&conn, &Actor::Human).unwrap();
        queries::update_note(&conn, &id, update(None, Some("three"), None)).unwrap();

        let events = case_events(&conn, &id).unwrap();
        let kinds: Vec<_> = events
            .iter()
            .rev()
            .map(|e| (e.kind.as_str(), e.actor_kind.as_str()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("created", "agent"),
                ("edited", "agent"),
                ("status", "agent"),
                ("report", "agent"),
                ("edited", "human"),
            ]
        );
        assert_eq!(
            events.last().unwrap().actor_name.as_deref(),
            Some("Claude Code")
        );
        let merged = &events[events.len() - 2];
        assert_eq!(merged.fields, vec!["content".to_string()]);
        assert_eq!(events[2].detail["to"], "resolved");
    }

    /// Two edits of different fields by the same actor form one event listing both fields.
    #[test]
    fn consecutive_edits_merge_their_fields() {
        let conn = db(Actor::Human);
        let id = note(&conn, "Case");
        queries::update_note(&conn, &id, update(None, Some("text"), None)).unwrap();
        conn.execute("UPDATE notes SET title = 'Renamed' WHERE id = ?1", [&id])
            .unwrap();
        let edits: Vec<_> = case_events(&conn, &id)
            .unwrap()
            .into_iter()
            .filter(|e| e.kind == "edited")
            .collect();
        assert_eq!(edits.len(), 1);
        let mut fields = edits[0].fields.clone();
        fields.sort();
        assert_eq!(fields, vec!["content".to_string(), "title".to_string()]);
    }

    /// The journal is append-only: no rewrite of who did what, no deletion while the case lives.
    #[test]
    fn events_cannot_be_rewritten_or_deleted() {
        let conn = db(Actor::Human);
        let id = note(&conn, "Case");
        assert!(conn
            .execute(
                "UPDATE case_events SET actor_kind = 'agent' WHERE note_id = ?1",
                [&id]
            )
            .is_err());
        assert!(conn
            .execute("DELETE FROM case_events WHERE note_id = ?1", [&id])
            .is_err());
        // The case's permanent erasure takes its journal with it.
        conn.execute("DELETE FROM notes WHERE id = ?1", [&id])
            .unwrap();
        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM case_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }

    /// A result declared by an agent waits for the person's decision; a rework is numbered, puts
    /// the case back to open, and the handed-back result is to review again.
    #[test]
    fn review_cycle_accept_and_rework() {
        let conn = db(Actor::Agent(Some("Codex".into())));
        let id = note(&conn, "Case");
        queries::update_note(
            &conn,
            &id,
            update(Some(NoteStatus::Resolved), None, Some("Done")),
        )
        .unwrap();
        assert_eq!(review_state(&conn, &id).unwrap().state, "to_review");

        // The agent's own connection cannot decide.
        assert!(review_case(&conn, &id, Decision::Accept, revision(&conn, &id), None).is_err());

        set_actor(&conn, &Actor::Human).unwrap();
        assert!(review_case(
            &conn,
            &id,
            Decision::RequestRework,
            revision(&conn, &id),
            Some("  ")
        )
        .is_err());
        let rework = review_case(
            &conn,
            &id,
            Decision::RequestRework,
            revision(&conn, &id),
            Some("Missing the yearly cost"),
        )
        .unwrap();
        assert_eq!(rework.rework_number, Some(1));
        let status: String = conn
            .query_row("SELECT status FROM notes WHERE id = ?1", [&id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "open");
        let state = review_state(&conn, &id).unwrap();
        assert_eq!(
            (state.state.as_str(), state.rework_count),
            ("rework_requested", 1)
        );
        assert_eq!(brief(&conn).unwrap().reworks_pending.len(), 1);

        set_actor(&conn, &Actor::Agent(Some("Codex".into()))).unwrap();
        queries::update_note(
            &conn,
            &id,
            update(Some(NoteStatus::Resolved), None, Some("Cost added")),
        )
        .unwrap();
        assert_eq!(review_state(&conn, &id).unwrap().state, "to_review");
        let brief_now = brief(&conn).unwrap();
        assert_eq!(brief_now.to_review.len(), 1);
        assert_eq!(brief_now.to_review[0].rework_count, 1);
        assert!(brief_now.reworks_pending.is_empty());

        set_actor(&conn, &Actor::Human).unwrap();
        let stale = revision(&conn, &id) - 1;
        assert!(review_case(&conn, &id, Decision::Accept, stale, None).is_err());
        review_case(&conn, &id, Decision::Accept, revision(&conn, &id), None).unwrap();
        assert_eq!(review_state(&conn, &id).unwrap().state, "accepted");
        assert!(brief(&conn).unwrap().to_review.is_empty());

        // An agent write after the acceptance makes it stale.
        set_actor(&conn, &Actor::Agent(Some("Codex".into()))).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        queries::update_note(&conn, &id, update(None, Some("more"), None)).unwrap();
        assert_eq!(review_state(&conn, &id).unwrap().state, "accepted_changed");
    }

    /// A case the person resolved or one resolved before the journal existed has nothing to review.
    #[test]
    fn nothing_to_review_without_an_agent_declaration() {
        let conn = db(Actor::Human);
        let id = note(&conn, "Mine");
        queries::update_note(
            &conn,
            &id,
            update(Some(NoteStatus::Resolved), None, Some("Done")),
        )
        .unwrap();
        assert_eq!(review_state(&conn, &id).unwrap().state, "none");
        assert!(brief(&conn).unwrap().to_review.is_empty());
    }

    /// A result, its decision and the handed-back rework can share a millisecond: the order is
    /// the case's revision, not the clock (a tie once read a rework as "nothing to decide").
    #[test]
    fn decision_and_results_in_the_same_millisecond_keep_their_order() {
        let conn = db(Actor::Agent(Some("Codex".into())));
        // Every write of this test at the same instant (the guards forbid rewriting a time).
        conn.execute_batch(
            "DROP TRIGGER case_events_only_merge_edits; DROP TRIGGER case_reviews_no_update;",
        )
        .unwrap();
        let same_instant = |conn: &Connection| {
            conn.execute_batch(
                "UPDATE case_events SET created_at = '2026-10-03T08:00:00.000Z', last_at = '2026-10-03T08:00:00.000Z';
                 UPDATE case_reviews SET created_at = '2026-10-03T08:00:00.000Z';",
            )
            .unwrap();
        };
        let state = |conn: &Connection, id: &str| review_state(conn, id).unwrap().state;
        let id = note(&conn, "Case");
        queries::update_note(
            &conn,
            &id,
            update(Some(NoteStatus::Resolved), None, Some("Done")),
        )
        .unwrap();

        set_actor(&conn, &Actor::Human).unwrap();
        review_case(
            &conn,
            &id,
            Decision::RequestRework,
            revision(&conn, &id),
            Some("Missing the cost"),
        )
        .unwrap();
        same_instant(&conn);
        assert_eq!(state(&conn, &id), "rework_requested");
        let b = brief(&conn).unwrap();
        assert_eq!((b.to_review.len(), b.reworks_pending.len()), (0, 1));

        set_actor(&conn, &Actor::Agent(Some("Codex".into()))).unwrap();
        queries::update_note(
            &conn,
            &id,
            update(Some(NoteStatus::Resolved), None, Some("Cost added")),
        )
        .unwrap();
        same_instant(&conn);
        assert_eq!(state(&conn, &id), "to_review");
        let b = brief(&conn).unwrap();
        assert_eq!((b.to_review.len(), b.reworks_pending.len()), (1, 0));

        set_actor(&conn, &Actor::Human).unwrap();
        review_case(&conn, &id, Decision::Accept, revision(&conn, &id), None).unwrap();
        same_instant(&conn);
        assert_eq!(state(&conn, &id), "accepted");
        let b = brief(&conn).unwrap();
        assert_eq!((b.to_review.len(), b.reworks_pending.len()), (0, 0));
    }

    /// The brief lists only non-human writes, after the last "seen", and forgets a case once opened.
    #[test]
    fn brief_lists_agent_writes_not_seen_yet() {
        let conn = db(Actor::Human);
        let mine = note(&conn, "Mine");
        set_actor(&conn, &Actor::Cli(None)).unwrap();
        let theirs = note(&conn, "Theirs");
        let other = note(&conn, "Other");

        let today = agent_events_since(&conn, "2000-01-01T00:00:00.000Z").unwrap();
        assert_eq!(
            today.len(),
            2,
            "the person's own write is not an agent write"
        );

        let first = brief(&conn).unwrap();
        assert!(first.since_is_fallback);
        let ids: Vec<_> = first.changes.iter().map(|e| e.note_id.clone()).collect();
        assert!(ids.contains(&theirs) && ids.contains(&other) && !ids.contains(&mine));

        mark_case_seen(&conn, &theirs).unwrap();
        let after_open: Vec<_> = brief(&conn)
            .unwrap()
            .changes
            .into_iter()
            .map(|e| e.note_id)
            .collect();
        assert_eq!(after_open, vec![other.clone()]);

        mark_brief_seen(&conn).unwrap();
        let cleared = brief(&conn).unwrap();
        assert!(!cleared.since_is_fallback);
        assert!(cleared.changes.is_empty());
    }

    /// A connection without the journal (archive restore, data move) records nothing.
    #[test]
    fn connections_without_the_journal_record_nothing() {
        let conn = Connection::open_in_memory().unwrap();
        schema::apply_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, space) VALUES ('p', 'Project', 'work')",
            [],
        )
        .unwrap();
        note(&conn, "Restored");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM case_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    /// A sub-case's creation records its parent, with no extra "moved" event; a later move is one.
    #[test]
    fn sub_case_creation_records_its_parent() {
        let conn = db(Actor::Agent(Some("Claude Code".into())));
        let parent = note(&conn, "Landing");
        let other = note(&conn, "Other");
        let child = crate::db::cases::create_sub_case(
            &conn,
            &parent,
            crate::models::note::ContentKind::Text,
            CreateNoteInput {
                project_id: String::new(),
                title: "Mockup".to_string(),
                content: None,
                note_type: NoteType::Todo,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: None,
            },
        )
        .unwrap();
        let events = case_events(&conn, &child).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "created");
        assert_eq!(events[0].detail["parent_id"], parent.as_str());

        crate::db::cases::move_case(&conn, &child, Some(&other)).unwrap();
        let events = case_events(&conn, &child).unwrap();
        assert_eq!(events[0].kind, "moved");
        assert_eq!(events[0].detail["to_parent"], other.as_str());
        // The creation event keeps its first parent.
        assert_eq!(events[1].detail["parent_id"], parent.as_str());
    }

    #[test]
    fn known_clients_get_a_readable_name() {
        assert_eq!(agent_display_name("claude-code"), "Claude Code");
        assert_eq!(agent_display_name("codex-mcp-client"), "Codex");
        assert_eq!(agent_display_name("my-agent"), "my-agent");
    }
}
