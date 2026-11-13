//! Fills an isolated test profile from a JSON file, through the application's own writes
//! (projects, cases, sub-cases, the journal of who did what), for audits and measures on large
//! profiles. Never the user's data: the folder must be new or already marked as a test profile.
//!
//! `HUMCASE_SEED_FILE=profile.json HUMCASE_SEED_DIR=C:\tmp\profile cargo test seed_profile -- --ignored`
//!
//! The file: `{ "projects": [{ "key", "name", "space": "work" | "private", "color"? }],
//! "cases": [{ "key", "project", "parent"?, "title", "nature", "status", "content", "days_ago",
//! "updated_days_ago"?, "due_in_days"?, "priority"?, "tags"?, "agent"? }] }`. A parent comes
//! before its sub-cases; `agent` names the agent that declared the status (a resolved case then
//! waits for the person's decision).

#[cfg(test)]
mod tests {
    use crate::db::cases::create_sub_case;
    use crate::db::journal::{self, Actor};
    use crate::db::queries;
    use crate::models::note::{ContentKind, CreateNoteInput};
    use crate::models::project::ProjectSpace;
    use rusqlite::params;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::path::Path;

    const MARKER: &str = ".humcase-isolated-profile";

    #[test]
    #[ignore = "writes a test profile on disk"]
    fn seed_profile() {
        let file = std::env::var("HUMCASE_SEED_FILE").expect("HUMCASE_SEED_FILE");
        let dir = std::env::var("HUMCASE_SEED_DIR").expect("HUMCASE_SEED_DIR");
        let dir = Path::new(&dir);
        assert!(dir.is_absolute(), "HUMCASE_SEED_DIR must be absolute");
        let home = dirs_home();
        assert!(
            !dir.starts_with(home.join(".humcase")),
            "never the user's data folder"
        );
        if dir.exists() && dir.read_dir().unwrap().next().is_some() {
            assert!(
                dir.join(MARKER).is_file(),
                "the folder is not a test profile"
            );
            assert!(
                !dir.join(crate::db::DB_FILE).exists(),
                "the profile already has a database"
            );
        }
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(MARKER), "").unwrap();
        let profile: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let conn = crate::db::init_db(dir).unwrap();
        let counts = seed(&conn, &profile);
        println!(
            "seeded {} projects, {} cases in {}",
            counts.0,
            counts.1,
            dir.display()
        );
    }

    fn dirs_home() -> std::path::PathBuf {
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(Into::into)
            .unwrap_or_default()
    }

    /// Writes the profile; returns the projects and cases created.
    fn seed(conn: &rusqlite::Connection, profile: &Value) -> (usize, usize) {
        journal::install(conn, &Actor::Human).unwrap();
        let mut projects: HashMap<String, String> = HashMap::new();
        for project in profile["projects"].as_array().unwrap() {
            let space = match project["space"].as_str() {
                Some("private") => Some(ProjectSpace::Private),
                Some("work") => Some(ProjectSpace::Work),
                _ => None,
            };
            let id = queries::create_project_in_space(
                conn,
                project["name"].as_str().unwrap(),
                project["color"].as_str(),
                space,
            )
            .unwrap();
            projects.insert(project["key"].as_str().unwrap().into(), id);
        }
        let cases = profile["cases"].as_array().unwrap();
        let mut ids: HashMap<String, String> = HashMap::new();
        // One transaction per case: `create_sub_case` opens its own.
        for case in cases {
            let key = case["key"].as_str().unwrap();
            let tags = case["tags"]
                .as_array()
                .filter(|t| !t.is_empty())
                .map(|t| Value::Array(t.clone()).to_string());
            let nature = case["nature"].as_str().unwrap_or("note");
            let input = CreateNoteInput {
                project_id: projects[case["project"].as_str().unwrap()].clone(),
                title: case["title"].as_str().unwrap().into(),
                content: case["content"].as_str().map(Into::into),
                note_type: serde_json::from_value(Value::String(nature.into()))
                    .unwrap_or(crate::models::note::NoteType::Note),
                priority: case["priority"].as_str().map(Into::into),
                git_branch: None,
                tags,
                due_at: None,
                raw_content: None,
            };
            let id = match case["parent"].as_str().and_then(|p| ids.get(p)) {
                Some(parent) => {
                    let mut input = input;
                    input.project_id = String::new();
                    create_sub_case(conn, parent, ContentKind::Text, input).unwrap()
                }
                None => queries::create_note(conn, input).unwrap(),
            };
            let status = case["status"].as_str().unwrap_or("open");
            if status != "open" {
                if let Some(agent) = case["agent"].as_str() {
                    journal::set_actor(conn, &Actor::Agent(Some(agent.into()))).unwrap();
                }
                conn.execute(
                    "UPDATE notes SET status = ?2 WHERE id = ?1",
                    params![id, status],
                )
                .unwrap();
                journal::set_actor(conn, &Actor::Human).unwrap();
            }
            if let Some(days) = case["due_in_days"].as_i64() {
                conn.execute(
                    "UPDATE notes SET due_at = date('now', 'localtime', ?2) WHERE id = ?1",
                    params![id, format!("{days:+} days")],
                )
                .unwrap();
            }
            ids.insert(key.into(), id);
        }
        let tx = conn.unchecked_transaction().unwrap();
        // Dates last: every write above moved them to now.
        for case in cases {
            let created = case["days_ago"].as_i64().unwrap_or(0).max(0);
            let updated = case["updated_days_ago"]
                .as_i64()
                .unwrap_or(created)
                .clamp(0, created);
            tx.execute(
                "UPDATE notes SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2),
                                  updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?3)
                  WHERE id = ?1",
                params![
                    ids[case["key"].as_str().unwrap()],
                    format!("-{created} days"),
                    format!("-{updated} days")
                ],
            )
            .unwrap();
        }
        // The journal follows: a creation on the case's first day, the other writes on its last
        // change. Its guard against rewrites is lifted for this test profile only, then put back
        // as it was.
        let guard: String = tx
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'case_events_only_merge_edits'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        tx.execute_batch("DROP TRIGGER case_events_only_merge_edits")
            .unwrap();
        tx.execute(
            "UPDATE case_events SET
                 created_at = (SELECT CASE WHEN case_events.kind = 'created' THEN n.created_at ELSE n.updated_at END
                                 FROM notes n WHERE n.id = case_events.note_id),
                 last_at = (SELECT CASE WHEN case_events.kind = 'created' THEN n.created_at ELSE n.updated_at END
                              FROM notes n WHERE n.id = case_events.note_id)
              WHERE EXISTS (SELECT 1 FROM notes n WHERE n.id = case_events.note_id)",
            [],
        )
        .unwrap();
        tx.execute_batch(&guard).unwrap();
        tx.commit().unwrap();
        (projects.len(), ids.len())
    }

    #[test]
    fn a_profile_is_seeded_with_sub_cases_dates_and_an_agent_result() {
        let conn = crate::ask::tests::db();
        let profile = serde_json::json!({
            "projects": [{ "key": "shop", "name": "Shop", "space": "work" }],
            "cases": [
                { "key": "a", "project": "shop", "title": "Redesign", "nature": "todo",
                  "status": "in_progress", "content": "The shop.", "days_ago": 40, "updated_days_ago": 3 },
                { "key": "b", "project": "shop", "parent": "a", "title": "Photos", "nature": "todo",
                  "status": "resolved", "content": "1600 px.", "days_ago": 10, "agent": "Claude Code",
                  "due_in_days": -2 }
            ]
        });
        assert_eq!(seed(&conn, &profile), (1, 2));
        let (parent, status, overdue): (Option<String>, String, bool) = conn
            .query_row(
                "SELECT parent_id, status, due_at < date('now', 'localtime') FROM notes WHERE title = 'Photos'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert!(parent.is_some());
        assert_eq!(status, "resolved");
        assert!(overdue);
        let photos: String = conn
            .query_row("SELECT id FROM notes WHERE title = 'Photos'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            journal::review_state(&conn, &photos).unwrap().state,
            "to_review"
        );
        let old: bool = conn
            .query_row(
                "SELECT updated_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 days') FROM notes WHERE title = 'Redesign'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(old);
        let journal_today: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM case_events WHERE created_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 days')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(journal_today, 0, "the journal follows the cases' dates");
        let guard: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'case_events_only_merge_edits'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(guard, 1, "the journal's guard is back");
    }
}
