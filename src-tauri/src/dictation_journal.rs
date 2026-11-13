//! Local journal of dictations processed by the AI: what was dictated, what the AI returned, how
//! long it took and with which provider. Founder request (batch C): be able to review the path
//! of a dictation and evaluate prompts on real cases.
//!
//! JSON Lines file next to the database (`dictation-journal.jsonl`), hence inside the isolated
//! profile during tests. Never sent anywhere else, never contains an API key. Bounded size: beyond
//! [`MAX_SIZE`], only the most recent half of the lines is kept. A journal write failure never
//! blocks the dictation: it is only reported on `stderr`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const FILE_NAME: &str = "dictation-journal.jsonl";
pub const MAX_SIZE: u64 = 2 * 1024 * 1024;

/// Journal location for a given database.
pub fn path_for(db_path: &Path) -> Option<PathBuf> {
    db_path.parent().map(|folder| folder.join(FILE_NAME))
}

/// Appends a timestamped entry; never returns an error to the caller.
pub fn record(db_path: &Path, mut entry: serde_json::Value) {
    let Some(path) = path_for(db_path) else {
        return;
    };
    if let Some(object) = entry.as_object_mut() {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        object.insert("at_unix_ms".into(), serde_json::Value::from(millis));
    }
    if let Err(e) = append(&path, &entry, MAX_SIZE) {
        eprintln!("Dictation journal not written ({e})");
    }
}

pub(crate) fn append(path: &Path, entry: &serde_json::Value, max_size: u64) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{entry}")?;
    drop(file);
    if fs::metadata(path)?.len() > max_size {
        let content = fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();
        let keep = &lines[lines.len() / 2..];
        // Atomic rewrite: a power cut never leaves a journal truncated in the middle of a line.
        let temp = path.with_extension("jsonl.tmp");
        fs::write(&temp, keep.join("\n") + "\n")?;
        fs::rename(&temp, path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_appends_lines_and_stays_bounded() {
        let folder = std::env::temp_dir().join(format!("humcase-journal-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join(FILE_NAME);
        for i in 0..50 {
            append(
                &path,
                &serde_json::json!({ "n": i, "text": "x".repeat(100) }),
                2_000,
            )
            .unwrap();
        }
        let content = fs::read_to_string(&path).unwrap();
        assert!(
            fs::metadata(&path).unwrap().len() <= 2_000 + 200,
            "the journal stays bounded"
        );
        let last: serde_json::Value =
            serde_json::from_str(content.lines().last().unwrap()).unwrap();
        assert_eq!(last["n"], 49, "the most recent entries are kept");
        assert!(
            content
                .lines()
                .all(|l| serde_json::from_str::<serde_json::Value>(l).is_ok()),
            "no truncated line"
        );
        fs::remove_dir_all(&folder).unwrap();
    }
}
