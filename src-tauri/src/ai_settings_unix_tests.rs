//! Tests of the Unix-only parts of `ai_settings`: the key file that seals AI keys, private file
//! modes and the Linux `/proc` PID probe.

use super::keyfile::{protect_with, unprotect_with, KEY_FILE};
use std::os::unix::fs::PermissionsExt;

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("humcase-keyfile-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn round_trip_is_not_plaintext_and_key_file_is_private() {
    let d = dir("roundtrip");
    let path = d.join(KEY_FILE);
    let sealed = protect_with(&path, b"sk-secret-value").unwrap();
    assert!(!String::from_utf8_lossy(&sealed).contains("sk-secret-value"));
    assert_eq!(unprotect_with(&path, &sealed).unwrap(), b"sk-secret-value");
    assert_eq!(mode(&path), 0o600);
    // Two seals of the same value differ (random nonce); the same key file serves both.
    let again = protect_with(&path, b"sk-secret-value").unwrap();
    assert_ne!(sealed, again);
    assert_eq!(unprotect_with(&path, &again).unwrap(), b"sk-secret-value");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn tampered_or_foreign_values_are_refused() {
    let d = dir("tamper");
    let path = d.join(KEY_FILE);
    let good = protect_with(&path, b"abc").unwrap();
    let mut tampered = good.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    assert!(unprotect_with(&path, &tampered).is_err());
    assert!(unprotect_with(&path, b"short").is_err());
    // Another installation's key cannot read it.
    let other = d.join("other.key");
    protect_with(&other, b"x").unwrap();
    assert!(unprotect_with(&other, &good).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_read_never_invents_a_key_and_an_invalid_key_file_is_not_overwritten() {
    let d = dir("missing");
    let path = d.join(KEY_FILE);
    let sealed = protect_with(&path, b"abc").unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(unprotect_with(&path, &sealed).is_err());
    assert!(!path.exists(), "a read must not create a new key");
    std::fs::write(&path, b"garbage").unwrap();
    assert!(protect_with(&path, b"abc").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"garbage");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn settings_file_is_written_private() {
    let d = dir("config");
    let config = d.join("ai-config.json");
    super::write_private(&config, b"{}").unwrap();
    assert_eq!(mode(&config), 0o600);
    let _ = std::fs::remove_dir_all(&d);
}

#[cfg(target_os = "linux")]
mod pid_probe {
    use super::super::{pid_liveness, pid_liveness_in, PidLiveness};
    use std::path::Path;

    #[test]
    fn current_process_is_alive_and_an_impossible_pid_is_dead() {
        assert!(matches!(
            pid_liveness(std::process::id()),
            PidLiveness::Alive
        ));
        assert!(matches!(pid_liveness(u32::MAX - 1), PidLiveness::Dead));
    }

    #[test]
    fn an_exited_child_is_dead() {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        assert!(matches!(pid_liveness(pid), PidLiveness::Dead));
    }

    #[test]
    fn without_proc_the_holder_is_assumed_alive() {
        assert!(matches!(
            pid_liveness_in(Path::new("/nonexistent-proc"), 1),
            PidLiveness::Alive
        ));
    }
}
