//! SQLite queries, split by theme. The `queries::xxx` paths remain unchanged: each
//! submodule is re-exported here.

pub use crate::db::activities::{
    create_activity, delete_activity, file_inbox_note, get_activities, get_or_create_inbox,
    move_note_to_project, rename_activity, set_project_activities,
};

mod handoff;
mod listing;
mod note_update;
mod notes;
mod projects;
mod search;
mod trash;

pub use handoff::*;
pub use listing::*;
pub use note_update::*;
pub use notes::*;
pub use projects::*;
pub use search::*;
pub use trash::*;
