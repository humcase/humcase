use super::AppState;
use tauri::State;

/// Agents that opened a session with this profile, each once with its last session, most recent
/// first (Settings → Agents, Home's first steps).
#[tauri::command]
pub fn list_agents_seen(state: State<'_, AppState>) -> Vec<crate::agents_seen::AgentSeen> {
    crate::agents_seen::list(&state.db_path)
}

/// Full path of this executable, written into the commands that connect an agent (Settings →
/// Agents), so the person never looks for it.
#[tauri::command]
pub fn humcase_executable_path() -> Result<String, String> {
    std::env::current_exe()
        .map(|path| path.display().to_string())
        .map_err(|e| e.to_string())
}
