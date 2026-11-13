use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectSpace {
    Private,
    Work,
}

impl ProjectSpace {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Work => "work",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub color: String,
    pub created_at: String,
    /// Last write of the project (ISO 8601 UTC); equal to `created_at` as long as it has not changed.
    pub updated_at: String,
    /// Concurrency version: advances on every write of the project (0 at creation).
    pub revision: i64,
    pub space: Option<ProjectSpace>,
    pub is_inbox: bool,
    /// Archive date (ISO 8601 UTC); `None` for an active project.
    pub archived_at: Option<String>,

    // Virtual fields that might be joined in queries
    pub open_bugs: Option<i64>,
    pub open_todos: Option<i64>,
}
