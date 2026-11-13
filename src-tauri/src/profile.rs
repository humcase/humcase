//! The work profile and the nudges: what Humcase learns about how the person works, and the cases
//! that wait for a next step.
//!
//! The profile of a space is an ordinary case of its Inbox (`work_profiles`, v28, says which):
//! the person reads and edits it like any other, agents read it (MCP `get_work_profile`,
//! `get_handoff`). Humcase never writes a line alone: it proposes lines (`proposals`, kind
//! `profile`) and the person adds them. A line comes from a decision in force that Jev reads as a
//! lasting rule (only when the person lets Jev read their cases), from the lines of work their
//! cases show, from what they often refuse in Sprout (both counted on the computer, without AI),
//! or from an agent (`propose_profile_rule`). `reason` says where a proposal comes from: `rule`,
//! `work:<line>`, `refused:<nature>` or `agent`.
//!
//! A nudge (kind `nudge`) points to an open question, idea or decision to take that nobody has
//! touched for days: at most three per space, chosen once a day, without AI. "Later" brings it back
//! after a week; "Never" never.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::permissions;
use crate::models::project::ProjectSpace;
use crate::tidy::{case_ref, excerpt, CaseRef};

/// Propose lines to add to the profile (`0`: off).
pub const PROPOSE_LINES: &str = "profile_propose";
/// Show on Home the cases that wait for a next step (`0`: off).
pub const SHOW_NUDGES: &str = "home_nudges";
/// Day (local) the nudges were last chosen.
const NUDGES_CHOSEN_ON: &str = "nudges_chosen_on";
/// Version of Jev's question the waiting rules were asked with (`jev_profile.v*.json`).
const RULE_QUESTION: &str = "profile_rule_question";

/// A decision waiting to be marked as replaced by a newer one, as "To tidy" shows it (judged by
/// Jev or proposed by an agent): it is not read as a rule while the person has not answered.
const WAITS_FOR_A_NEWER: &str = "EXISTS (SELECT 1 FROM proposals s
                    WHERE s.kind = 'supersede' AND s.answer IS NULL AND s.note_id = {decision}
                      AND (s.follows IS NOT NULL OR s.proposed_by IS NOT NULL))";
/// Jev's chance that a decision is a lasting rule: from here it is proposed. Measured by
/// `profile_eval` (22 decisions, six languages, `profile.v2`): rules 0.80 to 0.95 with one missed
/// (0.6), one-off choices and setups of a system 0.5 or less.
pub const RULE_MIN: f64 = 0.7;
/// Decisions sent to Jev in one refresh.
const RULES_PER_CHECK: usize = 12;
/// A line of work is proposed when this many cases of the space show it…
const WORK_MIN_CASES: usize = 5;
/// …and they are at least this share of the space's cases.
const WORK_MIN_SHARE: f64 = 0.15;
/// A nature of Sprout's proposals is "often refused" from this many refusals…
const REFUSED_MIN: i64 = 5;
/// …that are at least this share of the answers.
const REFUSED_MIN_SHARE: f64 = 0.6;
/// Nudges waiting at once in a space.
pub const NUDGES_MAX: usize = 3;
/// Days without a change before a case is nudged.
pub const NUDGE_IDLE_DAYS: i64 = 5;
/// Days before a nudge answered "Later" can come back.
pub const NUDGE_AGAIN_DAYS: i64 = 7;
/// Lines of a profile an agent's brief carries.
pub const BRIEF_LINES: usize = 40;
/// Characters of a case the counts read (its first words say its line of work).
const COUNT_TEXT_MAX: usize = 4000;
/// Counted lines are looked for at most this often (each look reads every case).
const COUNT_EVERY: Duration = Duration::from_secs(10 * 60);

/// Jev's question, versioned with the code.
const JEV_QUESTIONS: &str = include_str!("jev_profile.v2.json");

static REFRESHING: AtomicBool = AtomicBool::new(false);

fn now(conn: &Connection) -> Result<String, String> {
    conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |r| {
        r.get(0)
    })
    .map_err(crate::db::db_error)
}

// --- Settings --------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSettings {
    /// Humcase proposes lines to add to the profile.
    pub propose: bool,
    /// Home shows the cases that wait for a next step.
    pub nudges: bool,
}

pub fn settings(conn: &Connection) -> Result<ProfileSettings, String> {
    Ok(ProfileSettings {
        propose: permissions::get_setting(conn, PROPOSE_LINES)?.as_deref() != Some("0"),
        nudges: permissions::get_setting(conn, SHOW_NUDGES)?.as_deref() != Some("0"),
    })
}

pub fn set_settings(conn: &Connection, settings: &ProfileSettings) -> Result<(), String> {
    let flag = |on: bool| if on { "1" } else { "0" };
    permissions::set_setting(conn, PROPOSE_LINES, flag(settings.propose))?;
    permissions::set_setting(conn, SHOW_NUDGES, flag(settings.nudges))?;
    if settings.nudges {
        // Chosen again at once when the person turns them back on.
        conn.execute(
            "DELETE FROM app_settings WHERE key = ?1",
            [NUDGES_CHOSEN_ON],
        )
        .map_err(crate::db::db_error)?;
    }
    Ok(())
}

// --- The profile note ------------------------------------------------------------------------

fn parse_space(space: &str) -> Result<ProjectSpace, String> {
    match space {
        "work" => Ok(ProjectSpace::Work),
        "private" => Ok(ProjectSpace::Private),
        other => Err(format!("space must be work or private, not {other}")),
    }
}

/// The live profile note of `space`, if there is one.
pub fn profile_id(conn: &Connection, space: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT w.note_id FROM work_profiles w JOIN notes n ON n.id = w.note_id
          WHERE w.space = ?1 AND n.deleted_at IS NULL",
        [space],
        |r| r.get(0),
    )
    .optional()
    .map_err(crate::db::db_error)
}

/// The profile note of `space`, created in its Inbox the first time (or again when the person
/// put the previous one in the trash).
pub fn ensure_profile(conn: &Connection, space: &str) -> Result<String, String> {
    if let Some(id) = profile_id(conn, space)? {
        return Ok(id);
    }
    let inbox = crate::db::activities::get_or_create_inbox(conn, parse_space(space)?)?;
    let id = crate::db::queries::create_note(
        conn,
        crate::models::note::CreateNoteInput {
            project_id: inbox,
            title: crate::tr!("profile.title"),
            content: Some(String::new()),
            note_type: crate::models::note::NoteType::Note,
            priority: None,
            git_branch: None,
            tags: None,
            due_at: None,
            raw_content: None,
        },
    )?;
    // Resolved: a reference, not work to resume (the Tidy archive leaves it out).
    conn.execute("UPDATE notes SET status = 'resolved' WHERE id = ?1", [&id])
        .map_err(crate::db::db_error)?;
    let created = now(conn)?;
    conn.execute(
        "INSERT INTO work_profiles (space, note_id, created_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(space) DO UPDATE SET note_id = excluded.note_id, created_at = excluded.created_at",
        params![space, id, created],
    )
    .map_err(crate::db::db_error)?;
    Ok(id)
}

/// The profile of a space as agents and the profile page read it.
#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    pub space: String,
    pub note_id: Option<String>,
    pub content: String,
    pub revision: Option<i64>,
}

pub fn read_profile(conn: &Connection, space: &str) -> Result<Profile, String> {
    parse_space(space)?;
    let Some(id) = profile_id(conn, space)? else {
        return Ok(Profile {
            space: space.to_string(),
            note_id: None,
            content: String::new(),
            revision: None,
        });
    };
    let (content, revision): (String, i64) = conn
        .query_row(
            "SELECT COALESCE(content, ''), revision FROM notes WHERE id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(crate::db::db_error)?;
    Ok(Profile {
        space: space.to_string(),
        note_id: Some(id),
        content,
        revision: Some(revision),
    })
}

/// Ids of every profile note (left out of Tidy, Gather and the nudges).
pub fn profile_ids(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT note_id FROM work_profiles")
        .map_err(crate::db::db_error)?;
    let ids = stmt
        .query_map([], |r| r.get(0))
        .map_err(crate::db::db_error)?
        .collect::<Result<_, _>>()
        .map_err(crate::db::db_error)?;
    Ok(ids)
}

/// The profile's lines an agent's brief carries: its headings one level down, at most
/// [`BRIEF_LINES`] lines.
pub fn brief_lines(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .map(|l| match l.strip_prefix("## ") {
            Some(heading) => format!("### {heading}"),
            None => l.to_string(),
        })
        .take(BRIEF_LINES)
        .collect()
}

// --- Lines -----------------------------------------------------------------------------------

/// Where a line goes in the profile.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Rules,
    Work,
    Refused,
}

impl Section {
    fn heading(self) -> String {
        match self {
            Section::Rules => crate::tr!("profile.section.rules"),
            Section::Work => crate::tr!("profile.section.work"),
            Section::Refused => crate::tr!("profile.section.refused"),
        }
    }

    /// The heading in every language the engine speaks: a profile written before the person
    /// changed language keeps one section per kind.
    fn headings(self) -> Vec<String> {
        crate::i18n::languages()
            .map(|code| crate::i18n::with_language(code, || self.heading()))
            .collect()
    }

    fn of_reason(reason: &str) -> Section {
        if reason.starts_with("work:") {
            Section::Work
        } else if reason.starts_with("refused:") {
            Section::Refused
        } else {
            Section::Rules
        }
    }
}

/// The line where `section` starts, under its heading in any language.
fn section_start(lines: &[&str], section: Section) -> Option<usize> {
    let titles: Vec<String> = section
        .headings()
        .iter()
        .map(|h| format!("## {h}"))
        .collect();
    lines
        .iter()
        .position(|l| titles.iter().any(|t| l.trim() == t))
}

/// `content` with `line` at the end of `section` (the section is added at the end when the
/// person removed it).
fn with_line(content: &str, section: Section, line: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let title = format!("## {}", section.heading());
    let Some(start) = section_start(&lines, section) else {
        let base = content.trim_end();
        let gap = if base.is_empty() { "" } else { "\n\n" };
        return format!("{base}{gap}{title}\n\n{line}\n");
    };
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.starts_with("## ") || l.starts_with("# "))
        .map(|i| start + 1 + i)
        .unwrap_or(lines.len());
    // After the section's last non-empty line.
    let at = (start + 1..end)
        .rev()
        .find(|&i| !lines[i].trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(start + 1);
    let mut out: Vec<String> = lines[..at].iter().map(|l| l.to_string()).collect();
    if at == start + 1 {
        out.push(String::new());
    }
    out.push(line.to_string());
    if at < lines.len() && !lines[at].trim().is_empty() {
        out.push(String::new());
    }
    out.extend(lines[at..].iter().map(|l| l.to_string()));
    let mut text = out.join("\n");
    text.push('\n');
    text
}

/// `content` without `line`, as [`with_line`] added it to `section`: the last identical line
/// (an earlier one is the person's), then the section's heading when nothing is left under it.
/// `None` when the person changed or removed the line meanwhile.
fn without_line(content: &str, section: Section, line: &str) -> Option<String> {
    let mut lines: Vec<&str> = content.lines().collect();
    let at = lines.iter().rposition(|l| l.trim_end() == line)?;
    lines.remove(at);
    if let Some(start) = section_start(&lines, section) {
        let end = lines[start + 1..]
            .iter()
            .position(|l| l.starts_with("## ") || l.starts_with("# "))
            .map(|i| start + 1 + i)
            .unwrap_or(lines.len());
        if lines[start + 1..end].iter().all(|l| l.trim().is_empty()) {
            lines.drain(start..end);
            // One blank line stays between the sections around it.
            if start > 0
                && lines[start - 1].trim().is_empty()
                && lines.get(start).is_none_or(|l| l.trim().is_empty())
            {
                lines.remove(start - 1);
            }
        }
    }
    let mut text = lines.join("\n").trim_end().to_string();
    if !text.is_empty() {
        text.push('\n');
    }
    Some(text)
}

/// Whether the profile of `space` already holds the line `text` (with or without its source):
/// a line is never proposed twice, even when what it was found on is gone.
fn profile_has(conn: &Connection, space: &str, text: &str) -> Result<bool, String> {
    let Some(id) = profile_id(conn, space)? else {
        return Ok(false);
    };
    let content: String = conn
        .query_row(
            "SELECT COALESCE(content, '') FROM notes WHERE id = ?1",
            [&id],
            |r| r.get(0),
        )
        .map_err(crate::db::db_error)?;
    let wanted = text.trim().to_lowercase();
    Ok(content.lines().any(|l| {
        let l = l
            .trim()
            .trim_start_matches(['-', '*', '+'])
            .trim()
            .to_lowercase();
        l == wanted
            || l.strip_prefix(&wanted)
                .is_some_and(|rest| rest.starts_with(" ("))
    }))
}

fn write_content(conn: &Connection, id: &str, content: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE notes SET content = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE id = ?1 AND deleted_at IS NULL",
        params![id, content],
    )
    .map_err(crate::db::db_error)?;
    crate::db::queries::bump_revision(conn, id)?;
    Ok(())
}

// --- Proposals -------------------------------------------------------------------------------

/// A line Humcase (or an agent) proposes to add to the profile.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileItem {
    pub id: String,
    pub space: String,
    pub section: Section,
    /// `rule` (a decision Jev reads as a lasting rule), `work`, `refused` or `agent`.
    pub source: String,
    /// The line as it would be added.
    pub text: String,
    /// The decision or case it comes from.
    pub case: Option<CaseRef>,
    /// Why (an agent's reason) or what it was counted on.
    pub detail: String,
    pub proposed_by: Option<String>,
    pub chance: Option<f64>,
    pub created_at: String,
}

fn source_of(reason: &str) -> &'static str {
    match reason.split(':').next().unwrap_or("") {
        "work" => "work",
        "refused" => "refused",
        "agent" => "agent",
        _ => "rule",
    }
}

/// A day (`YYYY-MM-DD`) as the profile writes it: `DD/MM/YYYY` in French, Spanish, Portuguese and
/// Italian, `DD.MM.YYYY` in German, as is in English.
fn day_label(day: &str) -> String {
    match (
        crate::i18n::language(),
        day.split('-').collect::<Vec<_>>().as_slice(),
    ) {
        ("fr" | "es" | "pt-BR" | "it", [y, m, d]) => format!("{d}/{m}/{y}"),
        ("de", [y, m, d]) => format!("{d}.{m}.{y}"),
        _ => day.to_string(),
    }
}

/// The line written in the profile: the text, then where it comes from (a decision by its day:
/// the line already says its title).
fn line_of(source: &str, text: &str, decided_on: &str, by: Option<&str>) -> String {
    let from = match source {
        "rule" => crate::tr!("profile.from.decision", date = day_label(decided_on)),
        "work" => crate::tr!("profile.from.count"),
        "refused" => crate::tr!("profile.from.sprout"),
        _ => crate::tr!("profile.from.agent", agent = by.unwrap_or("agent")),
    };
    format!("- {text} ({from})")
}

/// The proposals waiting in `space` (every space when `None`), rules first.
pub fn pending(conn: &Connection, space: Option<&str>) -> Result<Vec<ProfileItem>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT q.id, COALESCE(p.space, 'work'), q.reason, q.title, q.note_id, q.passage,
                    q.proposed_by, q.follows, q.created_at, n.id IN (SELECT note_id FROM work_profiles)
               FROM proposals q JOIN notes n ON n.id = q.note_id JOIN projects p ON p.id = n.project_id
              WHERE q.kind = 'profile' AND q.answer IS NULL AND n.deleted_at IS NULL
                AND (?1 IS NULL OR COALESCE(p.space, 'work') = ?1)
              ORDER BY CASE WHEN q.reason = 'rule' OR q.reason = 'agent' THEN 0
                            WHEN q.reason LIKE 'work:%' THEN 1 ELSE 2 END,
                       q.created_at, q.position",
        )
        .map_err(crate::db::db_error)?;
    type Row = (
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<f64>,
        String,
        bool,
    );
    let rows: Vec<Row> = stmt
        .query_map([space], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
            ))
        })
        .map_err(crate::db::db_error)?
        .collect::<Result<_, _>>()
        .map_err(crate::db::db_error)?;
    let mut out = Vec::new();
    for (id, space, reason, text, note_id, detail, proposed_by, chance, created_at, on_profile) in
        rows
    {
        let case = if on_profile {
            None
        } else {
            case_ref(conn, &note_id, "created_at")?
        };
        out.push(ProfileItem {
            id,
            space,
            section: Section::of_reason(&reason),
            source: source_of(&reason).to_string(),
            text,
            case,
            detail,
            proposed_by,
            chance,
            created_at,
        });
    }
    Ok(out)
}

/// Adds a proposal unless one with the same origin exists in the space (answered or not):
/// a line refused is not proposed again.
#[allow(clippy::too_many_arguments)]
fn propose(
    conn: &Connection,
    space: &str,
    note_id: &str,
    reason: &str,
    text: &str,
    detail: &str,
    proposed_by: Option<&str>,
    chance: Option<f64>,
    answer: Option<&str>,
) -> Result<Option<String>, String> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS (
                SELECT 1 FROM proposals q JOIN notes n ON n.id = q.note_id
                  JOIN projects p ON p.id = n.project_id
                 WHERE q.kind = 'profile' AND COALESCE(p.space, 'work') = ?1
                   AND CASE WHEN ?2 = 'rule' THEN q.reason = 'rule' AND q.note_id = ?3
                                                  AND q.answer IS NOT 'replaced'
                            WHEN ?2 = 'agent' THEN lower(q.title) = lower(?4)
                            ELSE q.reason = ?2 END)",
            params![space, reason, note_id, text],
            |r| r.get(0),
        )
        .map_err(crate::db::db_error)?;
    if exists || profile_has(conn, space, text)? {
        return Ok(None);
    }
    let id = Uuid::new_v4().to_string();
    let created = now(conn)?;
    conn.execute(
        "INSERT INTO proposals (id, note_id, kind, nature, title, passage, reason, proposed_by,
                                follows, revision, position, created_at, answer, answered_at)
         SELECT ?1, n.id, 'profile', n.note_type, ?3, ?4, ?5, ?6, ?7, n.revision, 0, ?8, ?9,
                CASE WHEN ?9 IS NULL THEN NULL ELSE ?8 END
           FROM notes n WHERE n.id = ?2",
        params![
            id,
            note_id,
            text,
            detail,
            reason,
            proposed_by,
            chance,
            created,
            answer
        ],
    )
    .map_err(crate::db::db_error)?;
    Ok(Some(id))
}

/// Space of a case (`work` for a project not in a space).
fn space_of(conn: &Connection, note_id: &str) -> Result<String, String> {
    conn.query_row(
        "SELECT COALESCE(p.space, 'work') FROM notes n JOIN projects p ON p.id = n.project_id
          WHERE n.id = ?1 AND n.deleted_at IS NULL",
        [note_id],
        |r| r.get(0),
    )
    .optional()
    .map_err(crate::db::db_error)?
    .ok_or_else(|| crate::tr!("notes.notFound", id = note_id))
}

/// Withdraws (`replaced`) the waiting rules whose decision was replaced, archived or trashed,
/// or waits to be marked as replaced; and, once Jev's question changed, every waiting rule, so
/// the new question reads its decision again.
pub fn withdraw_stale(conn: &Connection) -> Result<usize, String> {
    let waits = WAITS_FOR_A_NEWER.replace("{decision}", "proposals.note_id");
    let mut withdrawn = conn
        .execute(
            &format!(
                "UPDATE proposals SET answer = 'replaced', answered_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                  WHERE kind = 'profile' AND answer IS NULL AND reason = 'rule'
                    AND (EXISTS (SELECT 1 FROM case_supersessions s
                                   JOIN notes b ON b.id = s.by_id AND b.deleted_at IS NULL
                                  WHERE s.note_id = proposals.note_id)
                         OR EXISTS (SELECT 1 FROM notes n WHERE n.id = proposals.note_id
                                     AND (n.archived_at IS NOT NULL OR n.deleted_at IS NOT NULL))
                         OR {waits})"
            ),
            [],
        )
        .map_err(crate::db::db_error)?;
    if permissions::get_setting(conn, RULE_QUESTION)?.as_deref() != Some(jev_version()) {
        withdrawn += conn
            .execute(
                "UPDATE proposals SET answer = 'replaced',
                        answered_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                  WHERE kind = 'profile' AND answer IS NULL AND reason = 'rule'",
                [],
            )
            .map_err(crate::db::db_error)?;
        permissions::set_setting(conn, RULE_QUESTION, jev_version())?;
    }
    Ok(withdrawn)
}

/// A live case as the counts read it.
struct Live {
    id: String,
    space: String,
    text: String,
}

fn live_cases(conn: &Connection) -> Result<Vec<Live>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT n.id, p.space, n.title || '\n' || substr(COALESCE(n.content, ''), 1, ?1)
               FROM notes n JOIN projects p ON p.id = n.project_id
              WHERE n.deleted_at IS NULL AND n.archived_at IS NULL AND p.archived_at IS NULL
                AND p.space IS NOT NULL AND n.content_kind = 'text'
                AND n.id NOT IN (SELECT note_id FROM work_profiles)
              ORDER BY n.updated_at DESC",
        )
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([COUNT_TEXT_MAX as i64], |r| {
            Ok(Live {
                id: r.get(0)?,
                space: r.get(1)?,
                text: r.get(2)?,
            })
        })
        .map_err(crate::db::db_error)?
        .collect::<Result<_, _>>()
        .map_err(crate::db::db_error)?;
    Ok(rows)
}

/// Lines of work the cases of each space show clearly enough, counted from their wording
/// (`recognize`). Proposed once per space and line.
pub fn propose_work(conn: &Connection) -> Result<usize, String> {
    let cases = live_cases(conn)?;
    let mut totals: HashMap<&str, usize> = HashMap::new();
    // space → line → (count, most recent case)
    let mut lines: HashMap<&str, HashMap<&'static str, (usize, &str)>> = HashMap::new();
    for case in &cases {
        *totals.entry(case.space.as_str()).or_default() += 1;
        if let Some(guess) = crate::recognize::recognize(&case.text).line_of_work {
            let entry = lines
                .entry(case.space.as_str())
                .or_default()
                .entry(guess.value)
                .or_insert((0, case.id.as_str()));
            entry.0 += 1;
        }
    }
    let mut added = 0;
    for (space, counts) in lines {
        let total = totals.get(space).copied().unwrap_or(0);
        let mut counts: Vec<_> = counts.into_iter().collect();
        counts.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(b.0)));
        for (line, (count, example)) in counts {
            if count < WORK_MIN_CASES || (count as f64) < WORK_MIN_SHARE * total as f64 {
                continue;
            }
            let text = crate::tr!(&format!("profile.work.{line}"));
            let detail = crate::tr!("profile.workDetail", count = count, total = total);
            let anchor = profile_id(conn, space)?;
            if propose(
                conn,
                space,
                anchor.as_deref().unwrap_or(example),
                &format!("work:{line}"),
                &text,
                &detail,
                None,
                None,
                None,
            )?
            .is_some()
            {
                added += 1;
            }
        }
    }
    Ok(added)
}

/// Natures of Sprout's proposals the person refuses most of the time, per space.
pub fn propose_refused(conn: &Connection) -> Result<usize, String> {
    let rows: Vec<(String, String, i64, i64, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT p.space, q.nature, count(*), sum(q.answer = 'refused'),
                        (SELECT q2.note_id FROM proposals q2 JOIN notes n2 ON n2.id = q2.note_id
                           JOIN projects p2 ON p2.id = n2.project_id
                          WHERE q2.kind = 'sub_case' AND q2.answer = 'refused'
                            AND q2.nature = q.nature AND p2.space = p.space
                            AND n2.deleted_at IS NULL
                          ORDER BY q2.answered_at DESC LIMIT 1)
                   FROM proposals q JOIN notes n ON n.id = q.note_id
                   JOIN projects p ON p.id = n.project_id
                  WHERE q.kind = 'sub_case' AND q.proposed_by IS NULL
                    AND q.answer IN ('created', 'placed', 'refused') AND p.space IS NOT NULL
                  GROUP BY p.space, q.nature",
            )
            .map_err(crate::db::db_error)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                ))
            })
            .map_err(crate::db::db_error)?
            .collect::<Result<_, _>>()
            .map_err(crate::db::db_error)?;
        rows
    };
    let mut added = 0;
    for (space, nature, total, refused, example) in rows {
        if refused < REFUSED_MIN
            || (refused as f64) < REFUSED_MIN_SHARE * total as f64
            || example.is_empty()
        {
            continue;
        }
        let text = crate::tr!(
            "profile.refusedLine",
            natures = crate::tr!(&format!("profile.natures.{nature}"))
        );
        let detail = crate::tr!("profile.refusedDetail", refused = refused, total = total);
        let anchor = profile_id(conn, &space)?;
        if propose(
            conn,
            &space,
            anchor.as_deref().unwrap_or(&example),
            &format!("refused:{nature}"),
            &text,
            &detail,
            None,
            None,
            None,
        )?
        .is_some()
        {
            added += 1;
        }
    }
    Ok(added)
}

/// An agent proposes a line (MCP `propose_profile_rule`): it waits for the person. It comes from
/// `case_id` when given (its space), else from the profile of `space`.
pub fn propose_from_agent(
    conn: &Connection,
    text: &str,
    reason: Option<&str>,
    case_id: Option<&str>,
    space: Option<&str>,
) -> Result<Value, String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return Err("text is required".into());
    }
    if text.chars().count() > 200 {
        return Err("text is longer than 200 characters: one short rule per call".into());
    }
    if !settings(conn)?.propose {
        return Err("the person turned off line proposals for the work profile".into());
    }
    let (space, note_id) = match case_id {
        Some(id) => (space_of(conn, id)?, id.to_string()),
        None => {
            let space = space.unwrap_or("work");
            (space.to_string(), ensure_profile(conn, space)?)
        }
    };
    let agent = crate::db::journal::current_agent_name(conn).unwrap_or_else(|| "agent".into());
    let detail: String = reason.unwrap_or("").trim().chars().take(300).collect();
    let id = propose(
        conn,
        &space,
        &note_id,
        "agent",
        &text,
        &detail,
        Some(&agent),
        None,
        None,
    )?;
    Ok(json!({ "id": id, "space": space, "already_proposed": id.is_none() }))
}

// --- Jev: is a decision a lasting rule? ------------------------------------------------------

#[derive(Deserialize)]
struct JevQuestions {
    version: String,
    model: String,
    rule: JevQuestion,
}

#[derive(Deserialize, Serialize, Clone)]
struct JevQuestion {
    instructions: String,
    criteria: Value,
}

fn jev_questions() -> &'static JevQuestions {
    static QUESTIONS: std::sync::OnceLock<JevQuestions> = std::sync::OnceLock::new();
    QUESTIONS.get_or_init(|| serde_json::from_str(JEV_QUESTIONS).expect("valid Jev questions"))
}

/// Version of Jev's question (`profile.v2`).
pub fn jev_version() -> &'static str {
    &jev_questions().version
}

/// A decision to ask Jev about.
#[derive(Debug, Clone)]
pub struct Decision {
    pub id: String,
    pub space: String,
    pub title: String,
    pub content: String,
}

/// Decisions in force not asked yet (a decision no newer one replaced, live, in a space).
pub fn decisions_to_ask(conn: &Connection) -> Result<Vec<Decision>, String> {
    let waits = WAITS_FOR_A_NEWER.replace("{decision}", "n.id");
    let mut stmt = conn
        .prepare(&format!(
            "SELECT n.id, p.space, n.title, COALESCE(n.content, '')
               FROM notes n JOIN projects p ON p.id = n.project_id
              WHERE n.note_type = 'decision' AND n.deleted_at IS NULL AND n.archived_at IS NULL
                AND p.archived_at IS NULL AND p.space IS NOT NULL
                AND NOT EXISTS (SELECT 1 FROM case_supersessions s
                                  JOIN notes b ON b.id = s.by_id AND b.deleted_at IS NULL
                                 WHERE s.note_id = n.id)
                AND NOT EXISTS (SELECT 1 FROM proposals q
                                 WHERE q.kind = 'profile' AND q.reason = 'rule' AND q.note_id = n.id
                                   AND q.answer IS NOT 'replaced')
                AND NOT {waits}
              ORDER BY n.updated_at DESC
              LIMIT ?1"
        ))
        .map_err(crate::db::db_error)?;
    let rows = stmt
        .query_map([RULES_PER_CHECK as i64], |r| {
            Ok(Decision {
                id: r.get(0)?,
                space: r.get(1)?,
                title: r.get(2)?,
                content: r.get(3)?,
            })
        })
        .map_err(crate::db::db_error)?
        .collect::<Result<_, _>>()
        .map_err(crate::db::db_error)?;
    Ok(rows)
}

fn jev_request(decisions: &[Decision]) -> String {
    let questions = jev_questions();
    let mut asked = serde_json::Map::new();
    for (i, decision) in decisions.iter().enumerate() {
        asked.insert(
            format!("rule_{i}"),
            json!({
                "type": "noul",
                "instructions": {
                    "decision": crate::jev::state_of(&decision.title, &decision.content),
                    "question": questions.rule.instructions,
                },
                "criteria": questions.rule.criteria,
            }),
        );
    }
    json!({
        "state": { "notes": "decisions from one person's notes" },
        "model": questions.model,
        "questions": asked,
    })
    .to_string()
}

/// Jev's chance that each decision is a lasting rule; `None` when Jev may not read, has no key
/// or fails.
pub async fn judge(db_path: &Path, data_dir: &Path, decisions: &[Decision]) -> Vec<Option<f64>> {
    let none = vec![None; decisions.len()];
    if decisions.is_empty() {
        return none;
    }
    let Some(key) = crate::natures::enabled(data_dir)
        .then(crate::natures::key)
        .flatten()
    else {
        return none;
    };
    let Ok(client) = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
    else {
        return none;
    };
    let started = Instant::now();
    let (reply, tokens) = crate::jev::ask(&client, &key, jev_request(decisions)).await;
    let model = reply
        .as_ref()
        .map(crate::jev::model_of)
        .unwrap_or_else(|_| jev_questions().model.clone());
    crate::ai_usage::record(
        db_path,
        &crate::ai_usage::UsageEntry {
            operation: crate::ai_usage::operation::CHECK_PROFILE,
            provider: crate::jev::PROVIDER,
            model: &model,
            language: None,
            estimated_cost_usd: crate::ai_usage::token_cost(crate::jev::PROVIDER, &model, &tokens),
            tokens,
            audio_seconds: None,
            duration_ms: started.elapsed().as_millis() as u64,
            ok: reply.is_ok(),
            prices_as_of: crate::ai_usage::PRICES_AS_OF,
        },
    );
    match reply {
        Ok(reply) => (0..decisions.len())
            .map(|i| reply["answers"][format!("rule_{i}")]["noul"].as_f64())
            .collect(),
        Err(_) => none,
    }
}

/// Keeps Jev's answers: a rule is proposed, a one-off choice is stored `dismissed` so it is not
/// asked again. A decision nobody judged is asked at a later refresh.
pub fn store_rules(
    conn: &Connection,
    decisions: &[Decision],
    chances: &[Option<f64>],
) -> Result<usize, String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let mut added = 0;
    for (decision, chance) in decisions.iter().zip(chances) {
        let Some(chance) = *chance else { continue };
        let answer = (chance < RULE_MIN).then_some("dismissed");
        let stored = propose(
            &tx,
            &decision.space,
            &decision.id,
            "rule",
            decision.title.trim(),
            &excerpt(&decision.content),
            None,
            Some(chance),
            answer,
        )?;
        if stored.is_some() && answer.is_none() {
            added += 1;
        }
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(added)
}

/// One refresh at a time (the Home card and the profile page).
pub struct Refreshing;

pub fn begin() -> Option<Refreshing> {
    REFRESHING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| Refreshing)
}

impl Drop for Refreshing {
    fn drop(&mut self) {
        REFRESHING.store(false, Ordering::Release);
    }
}

/// First step of a refresh, under the database lock: stale rules withdrawn, counted lines
/// proposed, then the decisions Jev should read (none when the person turned proposals off or
/// Jev may not read).
pub fn prepare(conn: &Connection, data_dir: &Path) -> Result<Vec<Decision>, String> {
    withdraw_stale(conn)?;
    if !settings(conn)?.propose {
        return Ok(Vec::new());
    }
    static COUNTED: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);
    let due = COUNTED
        .lock()
        .map(|mut last| {
            let due = last.is_none_or(|at| at.elapsed() >= COUNT_EVERY);
            if due {
                *last = Some(Instant::now());
            }
            due
        })
        .unwrap_or(true);
    if due {
        propose_work(conn)?;
        propose_refused(conn)?;
    }
    if !(crate::natures::enabled(data_dir) && crate::natures::key().is_some()) {
        return Ok(Vec::new());
    }
    decisions_to_ask(conn)
}

// --- Answers ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ProfileAnswer {
    pub id: String,
    pub accept: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ProfileOutcome {
    pub added: usize,
    pub refused: usize,
    /// Proposals answered (for Undo).
    pub ids: Vec<String>,
    /// Profile notes changed.
    pub profiles: Vec<String>,
}

/// Applies the answers in one transaction: an accepted line goes at the end of its section of
/// the profile of its space (created if needed).
pub fn answer(conn: &Connection, answers: &[ProfileAnswer]) -> Result<ProfileOutcome, String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    let mut outcome = ProfileOutcome::default();
    for item in answers {
        let row: Option<(String, String, String, Option<String>, String)> = tx
            .query_row(
                "SELECT q.note_id, COALESCE(q.reason, ''), q.title, q.proposed_by,
                        date(n.created_at, 'localtime')
                   FROM proposals q JOIN notes n ON n.id = q.note_id
                  WHERE q.id = ?1 AND q.kind = 'profile' AND q.answer IS NULL",
                [&item.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()
            .map_err(crate::db::db_error)?;
        let Some((note_id, reason, text, by, decided_on)) = row else {
            continue;
        };
        let answered = now(&tx)?;
        if !item.accept {
            tx.execute(
                "UPDATE proposals SET answer = 'refused', answered_at = ?2 WHERE id = ?1",
                params![item.id, answered],
            )
            .map_err(crate::db::db_error)?;
            outcome.refused += 1;
            outcome.ids.push(item.id.clone());
            continue;
        }
        let space = space_of(&tx, &note_id)?;
        let profile = ensure_profile(&tx, &space)?;
        let source = source_of(&reason);
        let line = line_of(source, &text, &decided_on, by.as_deref());
        let content: String = tx
            .query_row(
                "SELECT COALESCE(content, '') FROM notes WHERE id = ?1",
                [&profile],
                |r| r.get(0),
            )
            .map_err(crate::db::db_error)?;
        write_content(
            &tx,
            &profile,
            &with_line(&content, Section::of_reason(&reason), &line),
        )?;
        tx.execute(
            "UPDATE proposals SET answer = 'accepted', answered_at = ?2, answer_title = ?3,
                    result_id = ?4
              WHERE id = ?1",
            params![item.id, answered, line, profile],
        )
        .map_err(crate::db::db_error)?;
        outcome.added += 1;
        outcome.ids.push(item.id.clone());
        if !outcome.profiles.contains(&profile) {
            outcome.profiles.push(profile);
        }
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(outcome)
}

/// Takes answers back: an added line leaves the profile (unless the person changed it since),
/// and the proposal waits again.
pub fn undo(conn: &Connection, ids: &[String]) -> Result<(), String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    for id in ids {
        type Row = (Option<String>, Option<String>, Option<String>, String);
        let row: Option<Row> = tx
            .query_row(
                "SELECT answer, answer_title, result_id, COALESCE(reason, '') FROM proposals
                  WHERE id = ?1 AND kind = 'profile'",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(crate::db::db_error)?;
        let Some((answer, line, profile, reason)) = row else {
            continue;
        };
        if let (Some("accepted"), Some(line), Some(profile)) =
            (answer.as_deref(), line.as_deref(), profile.as_deref())
        {
            let content: Option<String> = tx
                .query_row(
                    "SELECT COALESCE(content, '') FROM notes WHERE id = ?1 AND deleted_at IS NULL",
                    [profile],
                    |r| r.get(0),
                )
                .optional()
                .map_err(crate::db::db_error)?;
            match content.and_then(|c| without_line(&c, Section::of_reason(&reason), line)) {
                Some(updated) => write_content(&tx, profile, &updated)?,
                // The person changed the line since: it stays, and so does the answer (waiting
                // again, it would be added a second time).
                None => continue,
            }
        }
        tx.execute(
            "UPDATE proposals SET answer = NULL, answered_at = NULL, answer_title = NULL,
                    result_id = NULL
              WHERE id = ?1 AND answer IN ('accepted', 'refused')",
            [id],
        )
        .map_err(crate::db::db_error)?;
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(())
}

// --- Nudges ----------------------------------------------------------------------------------

/// A case waiting for a next step.
#[derive(Debug, Clone, Serialize)]
pub struct Nudge {
    pub id: String,
    pub space: Option<String>,
    pub case: CaseRef,
    /// Whole days since the case last changed.
    pub idle_days: i64,
    /// Its text is long enough for Sprout (otherwise it is only opened).
    pub can_sprout: bool,
}

/// Withdraws (`replaced`) the waiting nudges whose case moved on: changed, answered, archived,
/// trashed, or given a sub-case.
fn withdraw_stale_nudges(conn: &Connection) -> Result<usize, String> {
    conn.execute(
        "UPDATE proposals SET answer = 'replaced', answered_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE kind = 'nudge' AND answer IS NULL AND (
            NOT EXISTS (SELECT 1 FROM notes n JOIN projects p ON p.id = n.project_id
                         WHERE n.id = proposals.note_id AND n.status = 'open'
                           AND n.deleted_at IS NULL AND n.archived_at IS NULL
                           AND p.archived_at IS NULL AND n.updated_at <= proposals.created_at)
            OR EXISTS (SELECT 1 FROM notes c WHERE c.parent_id = proposals.note_id
                        AND c.deleted_at IS NULL))",
        [],
    )
    .map_err(crate::db::db_error)
}

/// Chooses the nudges of the day: in each space, up to [`NUDGES_MAX`] waiting, the most
/// pressing first (priority, then the oldest change). An open question or idea, or an open
/// decision written as a question, that nobody changed for [`NUDGE_IDLE_DAYS`] days and that
/// has no sub-case yet.
pub fn choose_nudges(conn: &Connection) -> Result<usize, String> {
    let tx = crate::db::write_transaction(conn).map_err(crate::db::db_error)?;
    withdraw_stale_nudges(&tx)?;
    let created = now(&tx)?;
    let mut added = 0;
    for space in [Some("work"), Some("private"), None] {
        let waiting: i64 = tx
            .query_row(
                "SELECT count(*) FROM proposals q JOIN notes n ON n.id = q.note_id
                   JOIN projects p ON p.id = n.project_id
                  WHERE q.kind = 'nudge' AND q.answer IS NULL AND p.space IS ?1",
                [space],
                |r| r.get(0),
            )
            .map_err(crate::db::db_error)?;
        let room = NUDGES_MAX.saturating_sub(waiting as usize);
        if room == 0 {
            continue;
        }
        let ids: Vec<String> = {
            let mut stmt = tx
                .prepare(
                    "SELECT n.id FROM notes n JOIN projects p ON p.id = n.project_id
                      WHERE p.space IS ?1 AND n.status = 'open'
                        AND n.deleted_at IS NULL AND n.archived_at IS NULL AND p.archived_at IS NULL
                        AND (n.note_type IN ('question', 'idea')
                             OR (n.note_type = 'decision' AND instr(n.title, '?') > 0))
                        AND n.updated_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2)
                        AND NOT EXISTS (SELECT 1 FROM notes c WHERE c.parent_id = n.id
                                         AND c.deleted_at IS NULL)
                        AND n.id NOT IN (SELECT note_id FROM work_profiles)
                        AND NOT EXISTS (
                            SELECT 1 FROM proposals q WHERE q.kind = 'nudge' AND q.note_id = n.id
                               AND (q.answer IS NULL OR q.answer = 'refused'
                                    OR q.answered_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?3)))
                      ORDER BY CASE n.priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2
                                               WHEN 'P3' THEN 3 ELSE 4 END,
                               n.updated_at
                      LIMIT ?4",
                )
                .map_err(crate::db::db_error)?;
            let rows = stmt
                .query_map(
                    params![
                        space,
                        format!("-{NUDGE_IDLE_DAYS} days"),
                        format!("-{NUDGE_AGAIN_DAYS} days"),
                        room as i64
                    ],
                    |r| r.get(0),
                )
                .map_err(crate::db::db_error)?
                .collect::<Result<_, _>>()
                .map_err(crate::db::db_error)?;
            rows
        };
        for (position, id) in ids.iter().enumerate() {
            added += tx
                .execute(
                    "INSERT INTO proposals (id, note_id, kind, nature, title, passage, reason,
                                            revision, position, created_at)
                     SELECT ?1, n.id, 'nudge', n.note_type, n.title, '', 'idle', n.revision, ?3, ?4
                       FROM notes n WHERE n.id = ?2",
                    params![Uuid::new_v4().to_string(), id, position as i64, created],
                )
                .map_err(crate::db::db_error)?;
        }
    }
    tx.commit().map_err(crate::db::db_error)?;
    Ok(added)
}

/// The nudges waiting, chosen again once a day (none when the person turned them off).
pub fn nudges(conn: &Connection) -> Result<Vec<Nudge>, String> {
    if !settings(conn)?.nudges {
        return Ok(Vec::new());
    }
    let today: String = conn
        .query_row("SELECT date('now', 'localtime')", [], |r| r.get(0))
        .map_err(crate::db::db_error)?;
    if permissions::get_setting(conn, NUDGES_CHOSEN_ON)?.as_deref() != Some(today.as_str()) {
        choose_nudges(conn)?;
        permissions::set_setting(conn, NUDGES_CHOSEN_ON, &today)?;
    } else {
        withdraw_stale_nudges(conn)?;
    }
    let rows: Vec<(String, String, Option<String>, i64, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT q.id, q.note_id, p.space,
                        CAST(julianday('now') - julianday(n.updated_at) AS INTEGER),
                        COALESCE(n.content, '')
                   FROM proposals q JOIN notes n ON n.id = q.note_id
                   JOIN projects p ON p.id = n.project_id
                  WHERE q.kind = 'nudge' AND q.answer IS NULL
                  ORDER BY q.created_at, q.position",
            )
            .map_err(crate::db::db_error)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .map_err(crate::db::db_error)?
            .collect::<Result<_, _>>()
            .map_err(crate::db::db_error)?;
        rows
    };
    let mut out = Vec::new();
    for (id, note_id, space, idle_days, content) in rows {
        if let Some(case) = case_ref(conn, &note_id, "updated_at")? {
            out.push(Nudge {
                id,
                space,
                case,
                idle_days,
                can_sprout: content.split_whitespace().count() >= crate::germinate::MIN_WORDS,
            });
        }
    }
    Ok(out)
}

/// The person's answer to a nudge: `accepted` (they open it to take it further), `skipped`
/// (later: it may come back in a week) or `refused` (never for this case).
pub fn answer_nudge(conn: &Connection, id: &str, answer: &str) -> Result<(), String> {
    if !["accepted", "skipped", "refused"].contains(&answer) {
        return Err("answer must be accepted, skipped or refused".into());
    }
    conn.execute(
        "UPDATE proposals SET answer = ?2, answered_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE id = ?1 AND kind = 'nudge' AND answer IS NULL",
        params![id, answer],
    )
    .map_err(crate::db::db_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries;
    use crate::models::note::{CreateNoteInput, NoteType};

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        crate::db::schema::apply_migrations(&conn).unwrap();
        conn
    }

    fn project(conn: &Connection, name: &str, space: &str) -> String {
        let id = queries::create_project(conn, name, None).unwrap();
        conn.execute("UPDATE projects SET space = ?2 WHERE id = ?1", [&id, space])
            .unwrap();
        id
    }

    fn case(conn: &Connection, project: &str, title: &str, text: &str, nature: NoteType) -> String {
        queries::create_note(
            conn,
            CreateNoteInput {
                project_id: project.into(),
                title: title.into(),
                content: Some(text.into()),
                note_type: nature,
                priority: None,
                git_branch: None,
                tags: None,
                due_at: None,
                raw_content: None,
            },
        )
        .unwrap()
    }

    fn content(conn: &Connection, id: &str) -> String {
        conn.query_row("SELECT content FROM notes WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
    }

    fn revision(conn: &Connection, id: &str) -> i64 {
        conn.query_row("SELECT revision FROM notes WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
    }

    fn age(conn: &Connection, id: &str, days: i64) {
        conn.execute(
            "UPDATE notes SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2) WHERE id = ?1",
            params![id, format!("-{days} days")],
        )
        .unwrap();
    }

    #[test]
    fn a_decision_is_dated_in_the_language_of_the_profile() {
        crate::i18n::with_language("fr", || {
            assert_eq!(
                line_of("rule", "Une PR par lot", "2026-09-30", None),
                "- Une PR par lot (décision du 30/09/2026)"
            );
        });
        crate::i18n::with_language("en", || {
            assert_eq!(
                line_of("rule", "One PR per lot", "2026-09-30", None),
                "- One PR per lot (decision of 2026-09-30)"
            );
        });
    }

    #[test]
    fn a_line_goes_at_the_end_of_its_section() {
        crate::i18n::with_language("en", || {
            let text = "Intro.\n\n## Rules\n\n- One\n\n## Lines of work\n\n- Dev\n";
            assert_eq!(
                with_line(text, Section::Rules, "- Two"),
                "Intro.\n\n## Rules\n\n- One\n- Two\n\n## Lines of work\n\n- Dev\n"
            );
            assert_eq!(
                with_line(text, Section::Work, "- Writing"),
                "Intro.\n\n## Rules\n\n- One\n\n## Lines of work\n\n- Dev\n- Writing\n"
            );
            assert_eq!(
                with_line("Intro.\n", Section::Refused, "- Research"),
                "Intro.\n\n## Often refused\n\n- Research\n"
            );
            assert_eq!(
                with_line(
                    "## Rules\n## Lines of work\n- Dev\n",
                    Section::Rules,
                    "- One"
                ),
                "## Rules\n\n- One\n\n## Lines of work\n- Dev\n"
            );
            assert_eq!(
                without_line("a\n- Two\nb\n", Section::Rules, "- Two").unwrap(),
                "a\nb\n"
            );
            assert_eq!(
                without_line("a\n- Two, changed\n", Section::Rules, "- Two"),
                None
            );
        });
    }

    /// A profile written in French, read after the person chose English: a line joins the
    /// French section instead of adding a second one. Fails if a heading is looked for in the
    /// current language only.
    #[test]
    fn a_section_is_found_whatever_language_wrote_it() {
        crate::i18n::with_language("en", || {
            let text = "## Règles\n\n- Une PR par lot\n";
            assert_eq!(
                with_line(text, Section::Rules, "- Docs first"),
                "## Règles\n\n- Une PR par lot\n- Docs first\n"
            );
        });
    }

    /// Undo leaves the profile as it was: the line it added (not an identical one the person
    /// wrote earlier) and the section heading it created. Fails if Undo removes the first
    /// identical line or keeps an empty section.
    #[test]
    fn undo_leaves_the_profile_as_it_was() {
        crate::i18n::with_language("en", || {
            let before = "## Rules\n\n- One\n";
            let added = with_line(before, Section::Work, "- Writing (counted)");
            assert_eq!(
                without_line(&added, Section::Work, "- Writing (counted)").unwrap(),
                before
            );
            let mine = "## Rules\n\n- Same\n- Other\n";
            let added = with_line(mine, Section::Rules, "- Same");
            assert_eq!(
                without_line(&added, Section::Rules, "- Same").unwrap(),
                mine
            );
            let middle = "## Rules\n\n- One\n\n## Often refused\n\n- Research\n";
            let added = with_line(middle, Section::Work, "- Dev");
            assert_eq!(
                without_line(&added, Section::Work, "- Dev").unwrap(),
                middle
            );
        });
    }

    #[test]
    fn an_accepted_rule_is_written_in_the_profile_and_undo_takes_it_back() {
        crate::i18n::with_language("en", || {
            let conn = conn();
            let p = project(&conn, "Shop", "work");
            let d = case(
                &conn,
                &p,
                "Photos on white",
                "Every product photo on white.",
                NoteType::Decision,
            );
            let decisions = decisions_to_ask(&conn).unwrap();
            assert_eq!(decisions.len(), 1);
            assert_eq!(store_rules(&conn, &decisions, &[Some(0.9)]).unwrap(), 1);
            // Asked once: a decision Jev read is not asked again.
            assert!(decisions_to_ask(&conn).unwrap().is_empty());
            let items = pending(&conn, Some("work")).unwrap();
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].source, "rule");
            assert_eq!(items[0].case.as_ref().unwrap().id, d);
            assert!(pending(&conn, Some("private")).unwrap().is_empty());

            let outcome = answer(
                &conn,
                &[ProfileAnswer {
                    id: items[0].id.clone(),
                    accept: true,
                }],
            )
            .unwrap();
            assert_eq!(outcome.added, 1);
            let profile = profile_id(&conn, "work").unwrap().expect("profile created");
            assert_eq!(outcome.profiles, vec![profile.clone()]);
            let text = content(&conn, &profile);
            assert!(
                text.starts_with("## Rules\n\n- Photos on white (decision of 20"),
                "{text}"
            );
            let status: String = conn
                .query_row("SELECT status FROM notes WHERE id = ?1", [&profile], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(status, "resolved");
            let before = revision(&conn, &profile);
            assert!(pending(&conn, None).unwrap().is_empty());

            undo(&conn, &outcome.ids).unwrap();
            assert!(!content(&conn, &profile).contains("Photos on white"));
            assert!(
                revision(&conn, &profile) > before,
                "the undo is a new revision"
            );
            assert_eq!(pending(&conn, None).unwrap().len(), 1);
        });
    }

    #[test]
    fn a_one_off_choice_is_not_proposed_nor_asked_again() {
        let conn = conn();
        let p = project(&conn, "Shop", "work");
        case(
            &conn,
            &p,
            "Buy the Rational oven",
            "Second hand.",
            NoteType::Decision,
        );
        let decisions = decisions_to_ask(&conn).unwrap();
        assert_eq!(store_rules(&conn, &decisions, &[Some(0.2)]).unwrap(), 0);
        assert!(pending(&conn, None).unwrap().is_empty());
        assert!(decisions_to_ask(&conn).unwrap().is_empty());
        // Nobody judged it: asked again later.
        let q = case(
            &conn,
            &p,
            "Close on Sundays",
            "Every Sunday.",
            NoteType::Decision,
        );
        let decisions = decisions_to_ask(&conn).unwrap();
        assert_eq!(store_rules(&conn, &decisions, &[None]).unwrap(), 0);
        assert_eq!(decisions_to_ask(&conn).unwrap()[0].id, q);
    }

    #[test]
    fn a_refused_line_is_not_proposed_again_and_a_replaced_decision_leaves() {
        let conn = conn();
        let p = project(&conn, "Shop", "work");
        let a = case(&conn, &p, "Older rule", "Always A.", NoteType::Decision);
        let b = case(&conn, &p, "Newer rule", "Always B.", NoteType::Decision);
        let decisions = decisions_to_ask(&conn).unwrap();
        store_rules(&conn, &decisions, &[Some(0.9), Some(0.9)]).unwrap();
        let items = pending(&conn, None).unwrap();
        let of = |id: &str| {
            items
                .iter()
                .find(|i| i.case.as_ref().unwrap().id == id)
                .unwrap()
                .id
                .clone()
        };
        answer(
            &conn,
            &[ProfileAnswer {
                id: of(&b),
                accept: false,
            }],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO case_supersessions (note_id, by_id, superseded_at) VALUES (?1, ?2, 'now')",
            [&a, &b],
        )
        .unwrap();
        assert_eq!(withdraw_stale(&conn).unwrap(), 1);
        assert!(pending(&conn, None).unwrap().is_empty());
        assert!(decisions_to_ask(&conn).unwrap().is_empty());
        assert_eq!(
            store_rules(
                &conn,
                &[Decision {
                    id: b.clone(),
                    space: "work".into(),
                    title: "Newer rule".into(),
                    content: String::new()
                }],
                &[Some(0.95)]
            )
            .unwrap(),
            0,
            "refused once, never proposed again"
        );
    }

    /// A decision "To tidy" shows as maybe replaced is not read as a rule, and its waiting rule
    /// leaves, until the person answers. On a real base, one decision was proposed both as a
    /// rule and as replaced. Fails if the filter is removed.
    #[test]
    fn a_decision_maybe_replaced_is_not_a_rule() {
        let conn = conn();
        withdraw_stale(&conn).unwrap();
        let p = project(&conn, "Shop", "work");
        let a = case(&conn, &p, "Older rule", "Always A.", NoteType::Decision);
        let b = case(&conn, &p, "Newer rule", "Always B.", NoteType::Decision);
        store_rules(
            &conn,
            &decisions_to_ask(&conn).unwrap(),
            &[Some(0.9), Some(0.9)],
        )
        .unwrap();
        assert_eq!(pending(&conn, None).unwrap().len(), 2);
        conn.execute(
            "INSERT INTO proposals (id, note_id, kind, nature, title, passage, near_id, follows,
                                    revision, position, created_at)
             VALUES ('s', ?1, 'supersede', 'decision', 'Older rule', '', ?2, 0.9, 1, 0, 'now')",
            [&a, &b],
        )
        .unwrap();
        assert_eq!(withdraw_stale(&conn).unwrap(), 1);
        let left = pending(&conn, None).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].case.as_ref().unwrap().id, b);
        assert!(decisions_to_ask(&conn).unwrap().is_empty());
        // The person keeps both: the older one is asked again.
        conn.execute("UPDATE proposals SET answer = 'refused' WHERE id = 's'", [])
            .unwrap();
        let again = decisions_to_ask(&conn).unwrap();
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].id, a);
    }

    /// Once Jev's question changes, the rules waiting from the earlier one leave and their
    /// decisions are read again; an accepted line stays.
    #[test]
    fn a_new_question_reads_the_waiting_rules_again() {
        let conn = conn();
        withdraw_stale(&conn).unwrap();
        let p = project(&conn, "Shop", "work");
        case(&conn, &p, "Rule one", "Always A.", NoteType::Decision);
        case(&conn, &p, "Rule two", "Always B.", NoteType::Decision);
        store_rules(
            &conn,
            &decisions_to_ask(&conn).unwrap(),
            &[Some(0.9), Some(0.9)],
        )
        .unwrap();
        let first = pending(&conn, None).unwrap();
        answer(
            &conn,
            &[ProfileAnswer {
                id: first[0].id.clone(),
                accept: true,
            }],
        )
        .unwrap();
        assert_eq!(withdraw_stale(&conn).unwrap(), 0, "same question");
        permissions::set_setting(&conn, RULE_QUESTION, "profile.v1").unwrap();
        assert_eq!(withdraw_stale(&conn).unwrap(), 1);
        assert!(pending(&conn, None).unwrap().is_empty());
        assert_eq!(decisions_to_ask(&conn).unwrap().len(), 1);
        assert_eq!(
            permissions::get_setting(&conn, RULE_QUESTION)
                .unwrap()
                .as_deref(),
            Some(jev_version())
        );
    }

    #[test]
    fn lines_of_work_are_counted_per_space_without_ai() {
        crate::i18n::with_language("en", || {
            let conn = conn();
            let p = project(&conn, "App", "work");
            for i in 0..6 {
                case(
                    &conn,
                    &p,
                    &format!("Fix the login bug {i}"),
                    "The API returns a 500 error after the deploy; check the Rust backend and the SQL migration.",
                    NoteType::Bug,
                );
            }
            let home = project(&conn, "Home", "private");
            case(&conn, &home, "Groceries", "Milk.", NoteType::Todo);
            assert_eq!(propose_work(&conn).unwrap(), 1);
            let items = pending(&conn, None).unwrap();
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].space, "work");
            assert_eq!(items[0].section, Section::Work);
            assert_eq!(items[0].text, "Software development");
            assert_eq!(items[0].detail, "6 of your 6 cases");
            assert_eq!(propose_work(&conn).unwrap(), 0, "proposed once");
        });
    }

    /// A line in the profile is not proposed again once what it was counted on is gone (the
    /// case purged from the trash takes its proposals with it). Fails if `propose` stops
    /// reading the profile.
    #[test]
    fn a_line_in_the_profile_is_not_proposed_again_after_a_purge() {
        crate::i18n::with_language("en", || {
            let conn = conn();
            let p = project(&conn, "App", "work");
            for i in 0..6 {
                case(
                    &conn,
                    &p,
                    &format!("Fix the login bug {i}"),
                    "The API returns a 500 error after the deploy; check the Rust backend and the SQL migration.",
                    NoteType::Bug,
                );
            }
            assert_eq!(propose_work(&conn).unwrap(), 1);
            let item = pending(&conn, None).unwrap().remove(0);
            answer(
                &conn,
                &[ProfileAnswer {
                    id: item.id,
                    accept: true,
                }],
            )
            .unwrap();
            conn.execute("DELETE FROM proposals WHERE kind = 'profile'", [])
                .unwrap();
            assert_eq!(propose_work(&conn).unwrap(), 0);
            assert!(pending(&conn, None).unwrap().is_empty());
        });
    }

    /// The person turned proposals off: an agent's line is refused, and no profile is created
    /// for it.
    #[test]
    fn an_agent_line_is_refused_when_proposals_are_off() {
        let conn = conn();
        set_settings(
            &conn,
            &ProfileSettings {
                propose: false,
                nudges: true,
            },
        )
        .unwrap();
        assert!(propose_from_agent(&conn, "Docs first", None, None, Some("work")).is_err());
        assert_eq!(profile_id(&conn, "work").unwrap(), None);
    }

    #[test]
    fn natures_often_refused_in_sprout_are_proposed() {
        crate::i18n::with_language("en", || {
            let conn = conn();
            let p = project(&conn, "Trip", "private");
            let n = case(
                &conn,
                &p,
                "Plan the trip",
                "Somewhere warm.",
                NoteType::Idea,
            );
            for i in 0..7 {
                let answer = if i < 5 { "refused" } else { "created" };
                conn.execute(
                    "INSERT INTO proposals (id, note_id, kind, nature, title, passage, revision,
                                            position, created_at, answer, answered_at)
                     VALUES (?1, ?2, 'sub_case', 'research', 'Compare', 'p', 1, 0, 'now', ?3, 'now')",
                    params![format!("s{i}"), n, answer],
                )
                .unwrap();
            }
            // An agent's refused proposals do not count.
            conn.execute(
                "INSERT INTO proposals (id, note_id, kind, nature, title, passage, proposed_by,
                                        revision, position, created_at, answer, answered_at)
                 VALUES ('a', ?1, 'sub_case', 'todo', 'Do', 'p', 'Codex', 1, 0, 'now', 'refused', 'now')",
                [&n],
            )
            .unwrap();
            assert_eq!(propose_refused(&conn).unwrap(), 1);
            let items = pending(&conn, Some("private")).unwrap();
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].section, Section::Refused);
            assert_eq!(
                items[0].text,
                "Research steps proposed by Sprout: often refused"
            );
            assert_eq!(items[0].detail, "5 refused out of 7");
        });
    }

    #[test]
    fn an_agent_proposal_waits_for_the_person_and_is_not_duplicated() {
        let conn = conn();
        let p = project(&conn, "App", "work");
        let n = case(&conn, &p, "Review", "Text.", NoteType::Note);
        let first = propose_from_agent(
            &conn,
            "Mockup before UI",
            Some("seen twice"),
            Some(&n),
            None,
        )
        .unwrap();
        assert_eq!(first["already_proposed"], false);
        let again =
            propose_from_agent(&conn, "mockup before  UI", None, None, Some("work")).unwrap();
        assert_eq!(again["already_proposed"], true);
        let items = pending(&conn, None).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "agent");
        assert_eq!(items[0].proposed_by.as_deref(), Some("agent"));
        assert!(propose_from_agent(&conn, " ", None, None, None).is_err());
    }

    #[test]
    fn the_profile_is_left_out_of_the_tidy_archive() {
        let conn = conn();
        let profile = ensure_profile(&conn, "work").unwrap();
        assert_eq!(
            ensure_profile(&conn, "work").unwrap(),
            profile,
            "one per space"
        );
        age(&conn, &profile, 400);
        crate::tidy::refresh_archive(&conn).unwrap();
        let archived: i64 = conn
            .query_row(
                "SELECT count(*) FROM proposals WHERE kind = 'archive' AND note_id = ?1",
                [&profile],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(archived, 0, "the profile is never proposed for the archive");
        // In the trash: a new one is made.
        conn.execute(
            "UPDATE notes SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
            [&profile],
        )
        .unwrap();
        assert_ne!(ensure_profile(&conn, "work").unwrap(), profile);
    }

    #[test]
    fn nudges_pick_idle_open_questions_and_respect_the_answers() {
        let conn = conn();
        let p = project(&conn, "App", "work");
        let q1 = case(
            &conn,
            &p,
            "Which host?",
            "Cloudflare or Supabase.",
            NoteType::Question,
        );
        let q2 = case(&conn, &p, "One profile per space?", "", NoteType::Decision);
        let fresh = case(&conn, &p, "New idea", "", NoteType::Idea);
        let task = case(&conn, &p, "Old task", "", NoteType::Todo);
        let settled = case(&conn, &p, "Use Stripe", "", NoteType::Decision);
        let parent = case(&conn, &p, "Idea with steps", "", NoteType::Idea);
        let child = case(&conn, &p, "Step", "", NoteType::Todo);
        conn.execute(
            "UPDATE notes SET parent_id = ?2 WHERE id = ?1",
            [&child, &parent],
        )
        .unwrap();
        for id in [&q1, &q2, &task, &settled, &parent] {
            age(&conn, id, 9);
        }
        age(&conn, &fresh, 1);
        let shown = nudges(&conn).unwrap();
        assert_eq!(
            shown.iter().map(|n| n.case.id.clone()).collect::<Vec<_>>(),
            [q1.clone(), q2.clone()]
        );
        assert!(!shown[1].can_sprout, "no text: nothing to sprout from");
        assert!(!shown[0].can_sprout, "three words: too short");

        let ids: Vec<String> = nudges(&conn).unwrap().into_iter().map(|n| n.id).collect();
        assert_eq!(ids.len(), 2, "chosen once a day");
        answer_nudge(&conn, &ids[0], "refused").unwrap();
        answer_nudge(&conn, &ids[1], "skipped").unwrap();
        choose_nudges(&conn).unwrap();
        assert!(
            nudges(&conn).unwrap().is_empty(),
            "never, and not before a week"
        );
        conn.execute(
            "UPDATE proposals SET answered_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-8 days')
              WHERE id = ?1",
            [&ids[1]],
        )
        .unwrap();
        choose_nudges(&conn).unwrap();
        let back: Vec<String> = nudges(&conn)
            .unwrap()
            .into_iter()
            .map(|n| n.case.id)
            .collect();
        assert_eq!(back, vec![q2.clone()], "later comes back after a week");

        // The case moves on: the nudge leaves.
        conn.execute(
            "UPDATE notes SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+1 second') WHERE id = ?1",
            [&q2],
        )
        .unwrap();
        assert!(nudges(&conn).unwrap().is_empty());

        set_settings(
            &conn,
            &ProfileSettings {
                propose: true,
                nudges: false,
            },
        )
        .unwrap();
        age(&conn, &q2, 30);
        assert!(nudges(&conn).unwrap().is_empty(), "off");
    }

    /// Jev's rule question on the synthetic bench (`profile_eval_cases.json`), with real calls.
    /// `cargo test profile_eval -- --ignored --nocapture` (TYPESAFE_API_KEY in the environment).
    #[tokio::test]
    #[ignore]
    async fn profile_eval() {
        #[derive(Deserialize)]
        struct Bench {
            rules: Vec<Sample>,
        }
        #[derive(Deserialize)]
        struct Sample {
            id: String,
            expected: String,
            title: String,
            text: String,
        }
        let bench: Bench = serde_json::from_str(include_str!("profile_eval_cases.json")).unwrap();
        let key = std::env::var(crate::jev::KEY_ENV).expect("TYPESAFE_API_KEY");
        let decisions: Vec<Decision> = bench
            .rules
            .iter()
            .map(|s| Decision {
                id: s.id.clone(),
                space: "work".into(),
                title: s.title.clone(),
                content: s.text.clone(),
            })
            .collect();
        let client = reqwest::Client::new();
        let (reply, tokens) = crate::jev::ask(&client, &key, jev_request(&decisions)).await;
        let reply = reply.expect("Jev answers");
        println!("model {} tokens {:?}", crate::jev::model_of(&reply), tokens);
        let mut right = 0;
        let mut false_rules = 0;
        for (i, sample) in bench.rules.iter().enumerate() {
            let chance = reply["answers"][format!("rule_{i}")]["noul"]
                .as_f64()
                .unwrap_or(-1.0);
            let ok = (chance >= RULE_MIN) == (sample.expected == "rule");
            right += ok as usize;
            false_rules += (chance >= RULE_MIN && sample.expected != "rule") as usize;
            println!(
                "{:12} {:5} {chance:.2} {}",
                sample.id,
                sample.expected,
                if ok { "ok" } else { "WRONG" }
            );
        }
        println!("{right}/{}", bench.rules.len());
        // A one-off choice read as a rule puts a false line before the person: never. A rule
        // missed costs less (the person or an agent adds it): one is known (`fr-price`, 0.54 to
        // 0.62 with `profile.v2`), so at most one.
        assert_eq!(false_rules, 0, "one-off choices read as rules");
        assert!(bench.rules.len() - right <= 1, "rules missed");
    }
}
