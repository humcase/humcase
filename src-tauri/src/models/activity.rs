use super::project::ProjectSpace;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Activity {
    pub id: String,
    pub name: String,
    pub space: ProjectSpace,
    pub created_at: String,
    /// Last write of the activity (ISO 8601 UTC); equal to `created_at` as long as it has not changed.
    pub updated_at: String,
    /// Concurrency version: advances on every write of the activity, attachments included.
    pub revision: i64,
    pub project_ids: Vec<String>,
}
