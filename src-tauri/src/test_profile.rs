//! Redirect the whole data folder (database, backups, process lease, images, AI settings,
//! window state and WebView storage) for local debug runs.
//! Release builds always use `~/.humcase` (see `data_location`).

use std::path::PathBuf;

/// True when an isolated test profile is requested (debug only; a release build ignores it).
/// Such an instance can run alongside the installed application.
pub fn is_requested() -> bool {
    cfg!(debug_assertions) && std::env::var_os("HUMCASE_TEST_DATA_DIR").is_some()
}

pub fn app_data_dir(default_dir: PathBuf) -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(value) = std::env::var_os("HUMCASE_TEST_DATA_DIR") {
        let path = PathBuf::from(value);
        if !path.is_absolute() {
            return Err("HUMCASE_TEST_DATA_DIR must be an absolute path".into());
        }
        let path = path
            .canonicalize()
            .map_err(|error| format!("Test profile unavailable: {error}"))?;
        if !path.join(".humcase-isolated-profile").is_file() {
            return Err("Test profile marker missing; refusing to open a database".into());
        }
        return Ok(path);
    }
    Ok(default_dir)
}
