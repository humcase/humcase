//! Which agents have connected to Humcase, and when last: Settings → Agents says "Claude Code is
//! connected" from it, and Home's first steps tick "Connect an agent". Recorded when an MCP client
//! opens its session (`initialize`), so an agent that only reads counts too, which the journal of
//! writes cannot tell.
//!
//! JSON Lines file next to the database (`agents-seen.jsonl`), so inside the isolated profile during
//! tests: a name and a time per session, nothing else. Bounded like the other journals; a write
//! failure never blocks the agent.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const FILE_NAME: &str = "agents-seen.jsonl";
const MAX_SIZE: u64 = 64 * 1024;

/// One agent and the last time it opened a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSeen {
    pub name: String,
    pub at_unix_ms: u64,
}

/// Records that the agent `name` opened a session, beside the database at `db_path`.
pub fn record(db_path: &Path, name: &str) {
    let Some(folder) = db_path.parent() else {
        return;
    };
    let at_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let entry = AgentSeen {
        name: name.to_string(),
        at_unix_ms,
    };
    let Ok(value) = serde_json::to_value(&entry) else {
        return;
    };
    if let Err(e) = crate::dictation_journal::append(&folder.join(FILE_NAME), &value, MAX_SIZE) {
        eprintln!("humcase: agent session not recorded ({e})");
    }
}

/// Every agent seen beside the database at `db_path`, once each with its last session, most
/// recent first. Unreadable lines are skipped; no file means no agent yet.
pub fn list(db_path: &Path) -> Vec<AgentSeen> {
    let Some(folder) = db_path.parent() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(folder.join(FILE_NAME)) else {
        return Vec::new();
    };
    let mut last: BTreeMap<String, u64> = BTreeMap::new();
    for seen in content
        .lines()
        .filter_map(|line| serde_json::from_str::<AgentSeen>(line).ok())
    {
        let at = last.entry(seen.name).or_insert(0);
        *at = (*at).max(seen.at_unix_ms);
    }
    let mut agents: Vec<AgentSeen> = last
        .into_iter()
        .map(|(name, at_unix_ms)| AgentSeen { name, at_unix_ms })
        .collect();
    agents.sort_by_key(|agent| std::cmp::Reverse(agent.at_unix_ms));
    agents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_each_agent_once_with_its_last_session() {
        let folder = std::env::temp_dir().join(format!("humcase-agents-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&folder).unwrap();
        let db = folder.join("humcase.db");
        assert!(list(&db).is_empty());
        std::fs::write(
            folder.join(FILE_NAME),
            "{\"name\":\"Codex\",\"at_unix_ms\":10}\nnot json\n{\"name\":\"Claude Code\",\"at_unix_ms\":20}\n{\"name\":\"Codex\",\"at_unix_ms\":30}\n",
        )
        .unwrap();
        record(&db, "Cursor");
        let agents = list(&db);
        assert_eq!(
            agents.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
            ["Cursor", "Codex", "Claude Code"]
        );
        assert_eq!(agents[1].at_unix_ms, 30);
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
