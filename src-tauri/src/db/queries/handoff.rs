use super::notes::map_note_row;
use crate::models::note::{Note, NoteStatus, NoteType};
use rusqlite::{params, Connection, Result};
use std::collections::{HashMap, HashSet};

// Only notes touched in the last 24 h. The bound is expressed in the same ISO 8601 UTC format
// as the columns (comparing the strings "2026-09-21T14:50:57.123Z" vs "2026-09-20 14:50:57"
// would mix two formats and invert the order).
pub fn get_daily_standup(conn: &Connection, project_id: &str) -> Result<Vec<Note>> {
    let mut stmt = conn.prepare("SELECT n.id, n.project_id, n.title, n.content, n.note_type, n.status, n.resolution_comment, n.raw_content, n.priority, n.git_branch, n.tags, n.created_at, n.updated_at, n.due_at, n.revision, n.archived_at, n.deleted_at, n.parent_id, n.content_kind FROM notes n WHERE n.project_id = ?1 AND n.deleted_at IS NULL AND n.updated_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 day') ORDER BY n.status DESC, n.updated_at DESC")?;
    let iter = stmt.query_map(params![project_id], map_note_row)?;
    let mut notes = Vec::new();
    for n in iter {
        notes.push(n?);
    }
    Ok(notes)
}

/// Numeric rank of a priority (`P0`…`P3`). An unknown priority comes after the known ones, and
/// no priority comes last. Avoids lexical sorting where "P10" would precede "P2".
fn priority_rank(priority: Option<&str>) -> u8 {
    match priority {
        Some("P0") => 0,
        Some("P1") => 1,
        Some("P2") => 2,
        Some("P3") => 3,
        Some(_) => 8,
        None => 9,
    }
}

/// Readable label of a status (the JSON already serializes in snake_case; this is for the brief's text).
fn status_label(status: &NoteStatus) -> String {
    match status {
        NoteStatus::Open => crate::tr!("handoff.status.open"),
        NoteStatus::InProgress => crate::tr!("handoff.status.inProgress"),
        NoteStatus::Resolved => crate::tr!("handoff.status.resolved"),
    }
}

fn type_label(note_type: &NoteType) -> String {
    match note_type {
        NoteType::Bug => crate::tr!("handoff.type.bug"),
        NoteType::Todo => crate::tr!("handoff.type.todo"),
        NoteType::Idea => crate::tr!("handoff.type.idea"),
        NoteType::Note => crate::tr!("handoff.type.note"),
        NoteType::Question => crate::tr!("handoff.type.question"),
        NoteType::Research => crate::tr!("handoff.type.research"),
        NoteType::Decision => crate::tr!("handoff.type.decision"),
    }
}

/// Qualifies a due date (`YYYY-MM-DD`) relative to the current day and the end of the week.
fn qualify_due_date(due_at: &str, today: &str, week_end: &str) -> String {
    if due_at < today {
        crate::tr!("handoff.due.overdue")
    } else if due_at == today {
        crate::tr!("handoff.due.today")
    } else if due_at <= week_end {
        crate::tr!("handoff.due.thisWeek")
    } else {
        crate::tr!("handoff.due.upcoming")
    }
}

/// A case as the brief reads it: no content, no raw dictation.
struct HandoffCase {
    id: String,
    project_id: String,
    title: String,
    note_type: NoteType,
    status: NoteStatus,
    resolution_comment: Option<String>,
    priority: Option<String>,
    git_branch: Option<String>,
    tags: Option<String>,
    updated_at: String,
    due_at: Option<String>,
    archived_at: Option<String>,
    parent_id: Option<String>,
}

/// Live cases of a project (or of all projects), without their content: the brief needs titles,
/// statuses and the first line of a report, not whole notes.
fn handoff_cases(conn: &Connection, project_id: Option<&str>) -> Result<Vec<HandoffCase>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, title, note_type, status, substr(resolution_comment, 1, 600),
                priority, git_branch, tags, updated_at, due_at, archived_at, parent_id
           FROM notes WHERE deleted_at IS NULL AND (?1 IS NULL OR project_id = ?1)
          ORDER BY priority ASC NULLS LAST, created_at DESC",
    )?;
    let rows = stmt.query_map(params![project_id], |row| {
        Ok(HandoffCase {
            id: row.get(0)?,
            project_id: row.get(1)?,
            title: row.get(2)?,
            note_type: row.get(3)?,
            status: row.get(4)?,
            resolution_comment: row.get(5)?,
            priority: row.get(6)?,
            git_branch: row.get(7)?,
            tags: row.get(8)?,
            updated_at: row.get(9)?,
            due_at: row.get(10)?,
            archived_at: row.get(11)?,
            parent_id: row.get(12)?,
        })
    })?;
    let mut cases = Vec::new();
    for row in rows {
        // A corrupted row (e.g. unknown `note_type`) must not make the whole brief unusable.
        match row {
            Ok(case) => cases.push(case),
            Err(e) => eprintln!("humcase: note skipped (unreadable row): {e}"),
        }
    }
    Ok(cases)
}

/// Title of a case preceded by the cases it sits in: `Launch › Pricing › Yearly plan`.
fn titled_path(case: &HandoffCase, by_id: &HashMap<&str, &HandoffCase>) -> String {
    let mut titles = vec![case.title.as_str()];
    let mut parent = case.parent_id.as_deref();
    // The tree is strict (checked by the database); the bound only guards a damaged file.
    while let Some(id) = parent.filter(|_| titles.len() < 64) {
        match by_id.get(id) {
            Some(ancestor) => {
                titles.push(ancestor.title.as_str());
                parent = ancestor.parent_id.as_deref();
            }
            None => break,
        }
    }
    titles.reverse();
    titles.join(" › ")
}

/// Assembles the "handoff" brief of a project (or of all projects if `project_id` is `None`).
///
/// The brief states **only** what is actually in the database — notes to resume (open / in progress,
/// sorted by priority then due date), latest decisions (resolved notes, with their resolution
/// comment), overdue items and the last git branch seen. No deduction, no invented
/// data: an agent reading it must be able to infer the real state of the work. Every case
/// carries its identifier in brackets, and a sub-case the titles of the cases it sits in.
///
/// Used by the interface's "Handoff" button, which lists every case to resume.
pub fn build_handoff(conn: &Connection, project_id: Option<&str>) -> Result<String> {
    build_handoff_with(conn, project_id, None)
}

/// Decisions in force listed by the brief, the most recently changed first.
const IN_FORCE_LIMIT: usize = 10;

/// Cases to resume listed by the agents' brief (MCP and CLI `get_handoff`); the others are counted.
pub const AGENT_HANDOFF_LIMIT: usize = 25;

/// The brief for an agent: the same, with at most [`AGENT_HANDOFF_LIMIT`] cases to resume.
pub fn build_agent_handoff(conn: &Connection, project_id: Option<&str>) -> Result<String> {
    build_handoff_with(conn, project_id, Some(AGENT_HANDOFF_LIMIT))
}

fn ids_of(conn: &Connection, sql: &str) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare(sql)?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<_>>()?;
    Ok(ids)
}

fn build_handoff_with(
    conn: &Connection,
    project_id: Option<&str>,
    to_resume_limit: Option<usize>,
) -> Result<String> {
    let (today, week_end): (String, String) = conn.query_row(
        "SELECT date('now','localtime'), date('now','localtime','+7 day')",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let notes = handoff_cases(conn, project_id)?;
    let by_id: HashMap<&str, &HandoffCase> = notes.iter().map(|n| (n.id.as_str(), n)).collect();

    let project_title = match project_id {
        Some(pid) => conn
            .query_row(
                "SELECT CASE WHEN is_inbox = 1 THEN
                CASE space WHEN 'work' THEN 'Inbox — Work' ELSE 'Inbox — Private' END
                ELSE name END FROM projects WHERE id = ?1",
                params![pid],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_else(|_| crate::tr!("handoff.unknownProject")),
        None => crate::tr!("handoff.allProjects"),
    };

    // An archived note, or one filed in an archived project, has been set aside: not "to resume".
    let archived_projects = ids_of(
        conn,
        "SELECT id FROM projects WHERE archived_at IS NOT NULL",
    )?;
    let mut to_resume: Vec<&HandoffCase> = notes
        .iter()
        .filter(|n| {
            n.status != NoteStatus::Resolved
                && n.archived_at.is_none()
                && !archived_projects.contains(&n.project_id)
        })
        .collect();
    to_resume.sort_by(|a, b| {
        // **Numeric** sort by priority: a lexical sort would place "P10" before "P2".
        priority_rank(a.priority.as_deref())
            .cmp(&priority_rank(b.priority.as_deref()))
            .then_with(|| {
                a.due_at
                    .as_deref()
                    .unwrap_or("9999-12-31")
                    .cmp(b.due_at.as_deref().unwrap_or("9999-12-31"))
            })
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });

    // The work profiles (v28) have their own section, not a decision to repeat.
    let profiles = ids_of(conn, "SELECT note_id FROM work_profiles")?;
    let mut resolved: Vec<&HandoffCase> = notes
        .iter()
        .filter(|n| n.status == NoteStatus::Resolved && !profiles.contains(&n.id))
        .collect();
    resolved.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    let overdue = to_resume
        .iter()
        .filter(|n| matches!(&n.due_at, Some(d) if d.as_str() < today.as_str()))
        .count();

    let mut out = String::new();
    out.push_str(&format!(
        "{}\n",
        crate::tr!("handoff.title", project = project_title)
    ));
    out.push_str(&format!(
        "{}\n\n",
        crate::tr!(
            "handoff.summary",
            date = today,
            total = notes.len(),
            toResume = to_resume.len(),
            resolved = resolved.len(),
            overdue = overdue
        )
    ));

    // Reworks the person asked for come first, with their reason: the result was not accepted.
    // Read from the journal of decisions (v24), only for the cases that have a decision.
    let reviewed = ids_of(conn, "SELECT DISTINCT note_id FROM case_reviews")?;
    let reworks: Vec<(&HandoffCase, crate::models::note::ReviewState)> = to_resume
        .iter()
        .filter(|n| reviewed.contains(&n.id))
        .filter_map(|n| {
            crate::db::journal::review_state(conn, &n.id)
                .ok()
                .filter(|state| state.state == "rework_requested")
                .map(|state| (*n, state))
        })
        .collect();
    if !reworks.is_empty() {
        out.push_str(&format!("## {}\n\n", crate::tr!("handoff.reworkHeading")));
        for (n, state) in &reworks {
            let review = state.last_review.as_ref();
            out.push_str(&format!(
                "- {}\n",
                crate::tr!(
                    "handoff.reworkEntry",
                    title = titled_path(n, &by_id),
                    id = n.id,
                    number = review
                        .and_then(|r| r.rework_number)
                        .unwrap_or(state.rework_count),
                    reason = review.and_then(|r| r.comment.clone()).unwrap_or_default()
                )
            ));
        }
        out.push('\n');
    }

    out.push_str(&format!("## {}\n\n", crate::tr!("handoff.toResumeHeading")));
    if to_resume.is_empty() {
        out.push_str(&format!("- {}\n", crate::tr!("handoff.noneOpen")));
    }
    let shown = to_resume_limit.unwrap_or(usize::MAX).min(to_resume.len());
    for n in &to_resume[..shown] {
        let mut line = format!(
            "- **[{}]** {} [{}] — {}, {}",
            n.priority.clone().unwrap_or_else(|| "—".to_string()),
            titled_path(n, &by_id),
            n.id,
            status_label(&n.status),
            type_label(&n.note_type)
        );
        if let Some(d) = &n.due_at {
            line.push_str(&format!(
                ", {}",
                crate::tr!(
                    "handoff.dueEntry",
                    date = d,
                    when = qualify_due_date(d, &today, &week_end)
                )
            ));
        }
        if let Some(tags) = &n.tags {
            if let Ok(list) = serde_json::from_str::<Vec<String>>(tags) {
                for tag in list {
                    line.push_str(&format!(" #{}", tag));
                }
            }
        }
        out.push_str(&line);
        out.push('\n');
    }
    // Only the agents' brief is capped, and it speaks English like the rest of the MCP.
    if shown < to_resume.len() {
        out.push_str(&format!(
            "- … {} more to resume, not listed: use list_notes (status, project_id, search_query) to see them\n",
            to_resume.len() - shown
        ));
    }

    // Decisions in force (v27: no newer one replaced them) and the cases that may go against
    // one: what an agent must respect, and what it must not settle alone.
    let replaced = ids_of(
        conn,
        "SELECT s.note_id FROM case_supersessions s
           JOIN notes b ON b.id = s.by_id AND b.deleted_at IS NULL",
    )?;
    let mut in_force: Vec<&HandoffCase> = notes
        .iter()
        .filter(|n| {
            n.note_type == NoteType::Decision
                && n.archived_at.is_none()
                && !archived_projects.contains(&n.project_id)
                && !replaced.contains(&n.id)
        })
        .collect();
    in_force.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let conflicts: Vec<crate::tidy::TidyItem> = crate::tidy::pending(conn, None)
        .unwrap_or_default()
        .into_iter()
        .filter(|item| {
            item.kind == crate::tidy::TidyKind::Conflict
                && by_id.contains_key(item.case.id.as_str())
        })
        .collect();
    // A case of this scope may go against a decision of another project: shown even when no
    // decision of the scope is in force.
    if !in_force.is_empty() || !conflicts.is_empty() {
        out.push_str(&format!(
            "\n## {}\n\n",
            crate::tr!("handoff.inForceHeading")
        ));
        for n in in_force.iter().take(IN_FORCE_LIMIT) {
            out.push_str(&format!("- {} [{}]\n", titled_path(n, &by_id), n.id));
        }
        if in_force.len() > IN_FORCE_LIMIT {
            out.push_str(&format!(
                "- {}\n",
                crate::tr!(
                    "handoff.inForceMore",
                    count = in_force.len() - IN_FORCE_LIMIT
                )
            ));
        }
        for item in &conflicts {
            let Some(decision) = &item.other else {
                continue;
            };
            out.push_str(&format!(
                "- {}\n",
                crate::tr!(
                    "handoff.conflictEntry",
                    case = item.case.title,
                    caseId = item.case.id,
                    decision = decision.title,
                    decisionId = decision.id
                )
            ));
        }
    }

    // How the person works (v28): the profile of the project's space, or every profile.
    let spaces: Vec<String> = match project_id {
        Some(pid) => conn
            .query_row(
                "SELECT COALESCE(space, 'work') FROM projects WHERE id = ?1",
                params![pid],
                |row| row.get(0),
            )
            .map(|space| vec![space])
            .unwrap_or_default(),
        None => vec!["work".into(), "private".into()],
    };
    let mut profile_lines = Vec::new();
    for space in &spaces {
        if let Ok(profile) = crate::profile::read_profile(conn, space) {
            if let Some(id) = &profile.note_id {
                let lines = crate::profile::brief_lines(&profile.content);
                if lines.iter().any(|l| l.starts_with("- ")) {
                    profile_lines.push(format!("[{id}]"));
                    profile_lines.extend(lines);
                }
            }
        }
    }
    if !profile_lines.is_empty() {
        out.push_str(&format!(
            "\n## {}\n\n",
            crate::tr!("handoff.profileHeading")
        ));
        for line in profile_lines {
            out.push_str(&line);
            out.push('\n');
        }
    }

    out.push_str(&format!(
        "\n## {}\n\n",
        crate::tr!("handoff.decisionsHeading")
    ));
    if resolved.is_empty() {
        out.push_str(&format!("- {}\n", crate::tr!("handoff.noneResolved")));
    }
    // A decision a newer one replaced is history (v27): tagged with the decision in force.
    let replaced_by: std::collections::HashMap<String, String> = {
        let mut stmt = conn.prepare(
            "SELECT s.note_id, s.by_id FROM case_supersessions s
                   JOIN notes b ON b.id = s.by_id AND b.deleted_at IS NULL",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        rows
    };
    for n in resolved.iter().take(5) {
        let comment = n
            .resolution_comment
            .as_ref()
            .map(|c| {
                c.lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .chars()
                    .take(160)
                    .collect::<String>()
            })
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| crate::tr!("handoff.resolvedNoComment"));
        // A result an agent declared is not a decision the person made (v24).
        let review = match crate::db::journal::review_state(conn, &n.id)
            .map(|state| state.state)
            .as_deref()
        {
            Ok("to_review") => format!(" _({})_", crate::tr!("handoff.awaitingReview")),
            Ok("accepted") => format!(" _({})_", crate::tr!("handoff.accepted")),
            Ok("accepted_changed") => format!(" _({})_", crate::tr!("handoff.acceptedChanged")),
            _ => String::new(),
        };
        let replaced = replaced_by
            .get(&n.id)
            .map(|by| format!(" _({})_", crate::tr!("handoff.replacedBy", id = by)))
            .unwrap_or_default();
        out.push_str(&format!(
            "- {} [{}] → {}{}{}\n",
            titled_path(n, &by_id),
            n.id,
            comment,
            review,
            replaced
        ));
    }

    // "Last git branch seen" = that of the note carrying a branch **most recently
    // modified**, not the first of a list sorted by priority/creation (it could present
    // an old branch as the current context).
    let last_branch = notes
        .iter()
        .filter(|n| n.git_branch.as_ref().is_some_and(|b| !b.trim().is_empty()))
        .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
        .and_then(|n| n.git_branch.as_deref());
    if let Some(branch) = last_branch {
        out.push_str(&format!(
            "\n## {}\n\n- {}\n",
            crate::tr!("handoff.contextHeading"),
            crate::tr!("handoff.lastBranch", branch = branch)
        ));
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_rank_orders_known_then_unknown_then_absent() {
        assert_eq!(priority_rank(Some("P0")), 0);
        assert_eq!(priority_rank(Some("P1")), 1);
        assert_eq!(priority_rank(Some("P2")), 2);
        assert_eq!(priority_rank(Some("P3")), 3);
        // No lexical sort: "P10" (unknown) comes after "P3".
        assert!(priority_rank(Some("P10")) > priority_rank(Some("P3")));
        assert!(priority_rank(None) > priority_rank(Some("P10")));
    }

    #[test]
    fn qualify_due_date_boundaries() {
        let (today, week_end) = ("2026-09-29", "2026-10-04");
        crate::i18n::with_language("fr", || {
            assert_eq!(qualify_due_date("2026-09-28", today, week_end), "en retard");
            assert_eq!(
                qualify_due_date("2026-09-29", today, week_end),
                "aujourd’hui"
            );
            assert_eq!(
                qualify_due_date("2026-09-30", today, week_end),
                "cette semaine"
            );
            assert_eq!(
                qualify_due_date("2026-10-04", today, week_end),
                "cette semaine"
            );
            assert_eq!(qualify_due_date("2026-10-05", today, week_end), "à venir");
        });
    }
}
