//! AI settings persisted **on the Rust side** (API keys entered in the application).
//!
//! Keys are **never** sent back to the webview: the interface only reads a state
//! ("key defined") and a provenance. The file lives in the application's data folder
//! (`%USERPROFILE%\.humcase`), outside the repository, and is never logged.
//!
//! **At rest**, keys are encrypted with **DPAPI** (protection bound to the Windows user):
//! a copy of the file is no longer enough to read them. The format stays backward compatible
//! (plaintext keys from an earlier version are still read, then migrated on the first save).
//!
//! **Serialized writes**: an inter-process lock materialized by a `<path>.lock` file
//! wraps the whole read-modify-write cycle, so that two processes (or two
//! threads) cannot overwrite each other's key. The lock carries the PID and a unique
//! token; it is released only by its owner, and a stale lock is purged only after
//! checking that the holding process is **dead** (never on age alone). The write
//! itself is **atomic** (temporary file with a unique name + rename): an interrupted
//! write cannot leave a partial file that would lose the other keys.
//!
//! **Unreadable file**: a corrupted `ai-config.json` is never silently overwritten. A
//! **read** only reports it (file left intact); it is the **write**, the only one to
//! hold the lock, that quarantines it under
//! `<path>.illisible-<millis>-<pid>-<counter>` (original content kept) before starting
//! again from an empty configuration. An **inaccessible** file (I/O error) is never moved nor
//! overwritten: we do not know what it contains. If quarantining fails, writes are
//! refused until manual intervention.
//!
//! The `*_in` functions take an explicit path: tests use a temporary file
//! and never touch the user's real configuration.

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Last reason for an unreadable `ai-config.json`. Exposed to the interface (Settings) so the
/// silent reset of a corrupted configuration does not go unnoticed.
static CONFIG_ALERT: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn config_alert() -> &'static Mutex<Option<String>> {
    CONFIG_ALERT.get_or_init(|| Mutex::new(None))
}

/// Reason for the last incident reading the configuration (`None` if all is well).
pub fn last_config_alert() -> Option<String> {
    config_alert().lock().ok().and_then(|g| g.clone())
}

fn report_unreadable_config(reason: &str) {
    if let Ok(mut g) = config_alert().lock() {
        *g = Some(reason.to_string());
    }
}

fn clear_config_alert() {
    if let Ok(mut g) = config_alert().lock() {
        *g = None;
    }
}

/// Alert message when a corrupted original was kept under a copy. Factored out so it can be
/// verified through the real static (never a reconstruction by the same function).
fn quarantine_alert(reason: &str, copy: &Path) -> String {
    crate::tr!(
        "ai.alert.quarantineCopy",
        reason = reason,
        path = copy.display()
    )
}

/// Configuration file: keys (encrypted) and server addresses entered in the application. Models are
/// not secrets and remain interface preferences.
pub(crate) const FILE_NAME: &str = "ai-config.json";

/// Stored value: encrypted with DPAPI (current format) or plaintext (old format, migrated on the
/// first write).
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum StoredKey {
    Encrypted(Vec<u8>),
    Plain(String),
}

#[derive(Default, Serialize, Deserialize)]
struct AiConfig {
    /// Stored as `cles`: the name the file has always used.
    #[serde(default, rename = "cles")]
    keys: HashMap<String, StoredKey>,
    /// Server address entered in the application (local model or custom server), by provider.
    /// Not a secret: stored in plain text. Omitted when empty, so files without addresses keep
    /// their previous shape.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    urls: HashMap<String, String>,
}

/// Where the key actually used comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyProvenance {
    /// Entered in the application (configuration file).
    App,
    /// Windows environment variable.
    Env,
    /// None.
    None,
}

/// Path of the configuration file (the folder is created if needed).
pub fn config_path() -> Result<PathBuf, String> {
    let dir = crate::data_location::data_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(FILE_NAME))
}

/// Result of a raw read of the file: distinguishes "absent" (legitimate empty config),
/// "readable", "corrupt" (content read but invalid) and "unreadable" (content not obtained).
/// Only a proven `Corrupt` allows a quarantine; `Unreadable` must never move a file
/// whose content is unknown (it could be valid).
enum ReadOutcome {
    Absent,
    Readable(AiConfig),
    Corrupt(String),
    Unreadable(String),
}

/// Result of a prepared read: the usable configuration, plus, if a write must be
/// refused to avoid risking the loss of keys, the reason (user message, localized).
struct PreparedRead {
    config: AiConfig,
    write_blocked: Option<String>,
    /// `true` if the read met no incident. A successful write after a healthy read
    /// closes the alert; a write that follows a quarantine leaves it visible (the user has
    /// not seen the incident yet).
    healthy: bool,
}

fn read_from(path: &Path) -> ReadOutcome {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return ReadOutcome::Absent,
        // Content not obtained: this is NOT proof of corruption.
        Err(e) => return ReadOutcome::Unreadable(crate::tr!("ai.reason.readFailed", error = e)),
    };
    // Invalid encoding: fixed reason, never the unexpected value.
    let content = match String::from_utf8(bytes) {
        Ok(content) => content,
        Err(_) => return ReadOutcome::Corrupt(crate::tr!("ai.reason.notText")),
    };
    match serde_json::from_str::<AiConfig>(&content) {
        Ok(config) => ReadOutcome::Readable(config),
        // The reason never echoes the file content (it may contain keys).
        Err(e) => ReadOutcome::Corrupt(crate::tr!(
            "ai.reason.invalidJson",
            line = e.line(),
            column = e.column()
        )),
    }
}

/// Usable configuration for a read. `allow_quarantine` is true only under the lock: the
/// **pure read** path (`app_key_in`) never moves the file — content seen as corrupt
/// may have been replaced in the meantime by a writer, and renaming it would destroy its fresh
/// configuration (TOCTOU). Without a possible quarantine, `Corrupt` and `Unreadable` are reported
/// and the empty configuration is returned without touching the file.
fn prepared_config(path: &Path, allow_quarantine: bool) -> PreparedRead {
    match read_from(path) {
        // No read clears the alert (N1): a file that became healthy does not negate the
        // past reset. The alert is closed only by a **successful write** on a
        // healthy configuration (see `set_key_in`). The read is simply marked as healthy.
        ReadOutcome::Readable(config) => PreparedRead {
            config,
            write_blocked: None,
            healthy: true,
        },
        // An absent file is not proof that all is well: the alert is never cleared here,
        // otherwise a quarantine in progress would go unnoticed before being displayed (M1).
        ReadOutcome::Absent => PreparedRead {
            config: AiConfig::default(),
            write_blocked: None,
            healthy: true,
        },
        ReadOutcome::Corrupt(reason) if !allow_quarantine => {
            eprintln!("humcase: ai-config.json corrupted ({reason}); file left intact (read only)");
            report_unreadable_config(&crate::tr!("ai.alert.leftIntact", reason = reason));
            PreparedRead {
                config: AiConfig::default(),
                write_blocked: None,
                healthy: false,
            }
        }
        ReadOutcome::Corrupt(reason) => match quarantine_unreadable(path) {
            Ok(Some(copy)) => {
                eprintln!(
                    "humcase: ai-config.json corrupted ({reason}); copy kept at {}; configuration reset",
                    copy.display()
                );
                report_unreadable_config(&quarantine_alert(&reason, &copy));
                PreparedRead {
                    config: AiConfig::default(),
                    write_blocked: None,
                    healthy: false,
                }
            }
            // A concurrent writer moved the file: it is no longer in place, nothing to overwrite.
            Ok(None) => {
                eprintln!(
                    "humcase: ai-config.json corrupted ({reason}); original already quarantined; configuration reset"
                );
                report_unreadable_config(&crate::tr!(
                    "ai.alert.quarantinedOriginal",
                    reason = reason
                ));
                PreparedRead {
                    config: AiConfig::default(),
                    write_blocked: None,
                    healthy: false,
                }
            }
            // The move fails: the original is still there and a write would destroy it.
            Err(e) => {
                eprintln!(
                    "humcase: ai-config.json corrupted ({reason}); original cannot be moved ({e}); writes blocked"
                );
                report_unreadable_config(&crate::tr!("ai.alert.notMovable", reason = reason));
                PreparedRead {
                    config: AiConfig::default(),
                    write_blocked: Some(crate::tr!("ai.settings.corruptNotQuarantined", error = e)),
                    healthy: false,
                }
            }
        },
        ReadOutcome::Unreadable(reason) => {
            // Content not obtained (lock, permissions, I/O): the file is never touched, otherwise we
            // could overwrite valid keys that we cannot read (M3+M4).
            eprintln!(
                "humcase: ai-config.json inaccessible ({reason}); file left intact; writes blocked"
            );
            report_unreadable_config(&crate::tr!("ai.alert.writesBlocked", reason = reason));
            PreparedRead {
                config: AiConfig::default(),
                write_blocked: Some(crate::tr!("ai.settings.unreadable")),
                healthy: false,
            }
        }
    }
}

/// `true` if the configuration still contains at least one plaintext key (old format).
fn has_plaintext_keys(config: &AiConfig) -> bool {
    config
        .keys
        .values()
        .any(|value| matches!(value, StoredKey::Plain(_)))
}

/// Encrypts in memory all keys still in plaintext, without writing to disk.
fn migrate_in_memory(config: &mut AiConfig) {
    let to_migrate: Vec<(String, String)> = config
        .keys
        .iter()
        .filter_map(|(id, value)| match value {
            StoredKey::Plain(plain) => Some((id.clone(), plain.clone())),
            StoredKey::Encrypted(_) => None,
        })
        .collect();
    for (id, plain) in to_migrate {
        if let Ok(encrypted) = encrypt(&plain) {
            config.keys.insert(id, encrypted);
        }
    }
}

/// Migration of plaintext keys on a **read path**: takes the lock, re-reads the file
/// fresh (another process may have migrated in the meantime, in which case nothing is written), migrates if
/// needed, writes, then releases. Returns the freshest configuration.
fn migrate_plaintext_keys_under_lock(path: &Path, config_read: AiConfig) -> AiConfig {
    let lock = match FileLock::acquire(path) {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!("humcase: plaintext key migration impossible ({e})");
            return config_read;
        }
    };
    let mut fresh = match read_from(path) {
        ReadOutcome::Readable(config) => config,
        // File gone, inaccessible or corrupted in the meantime: the initial read is kept.
        _ => {
            drop(lock);
            return config_read;
        }
    };
    if has_plaintext_keys(&fresh) {
        migrate_in_memory(&mut fresh);
        if let Err(e) = write_to(path, &fresh) {
            eprintln!("humcase: plaintext key migration impossible ({e})");
        }
    }
    drop(lock);
    fresh
}

/// Moves a **proven corrupted** `ai-config.json` to a unique name
/// `<path>.illisible-<millis>-<pid>-<counter>`. Called only under the lock: the pure
/// read path never moves the file.
///
/// - `Ok(Some(copy))`: original moved, content preserved;
/// - `Ok(None)`: original already absent (moved by another writer);
/// - `Err(reason)`: original still present and not movable (locked file,
///   permissions) → a write would overwrite it.
fn quarantine_unreadable(path: &Path) -> Result<Option<PathBuf>, String> {
    let copy = quarantine_path(path);
    // On Windows, `rename` **replaces** the destination (never `AlreadyExists`): the unique name
    // (pid + counter) makes any collision very unlikely and avoids overwriting the copy of a
    // previous incident (m8).
    match std::fs::rename(path, &copy) {
        Ok(()) => Ok(Some(copy)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Unique quarantine name built by `OsString` concatenation (never `display()`, lossy: m11).
fn quarantine_path(path: &Path) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(
        ".illisible-{}-{}-{counter}",
        now_millis(),
        std::process::id()
    ));
    PathBuf::from(name)
}

/// Milliseconds since the UNIX epoch (0 if the system clock is earlier, unrealistic here).
fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Age from which a lock is suspected stale (holder killed without releasing). Serves as a
/// **purge threshold**, not as proof: the PID's death is checked before any removal.
const LOCK_STALE_MS: u128 = 60_000;
/// Interval between two lock acquisition attempts.
const LOCK_RETRY_MS: u64 = 15;
/// Maximum time one holder may keep the lock while we wait, before failing. The wait starts
/// again whenever the lock changes hands: other writers making progress are not a failure.
const LOCK_TIMEOUT_MS: u128 = 5_000;

/// Monotonic counter making the lock token and the temporary file name unique, even within the
/// same millisecond.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Inter-process lock materialized by a `<path>.lock` file created atomically. The lock
/// carries the PID and a unique **token**: on drop, only the token holder removes the
/// file, which prevents destroying a successor's lock after a purge.
#[derive(Debug)]
struct FileLock {
    path: PathBuf,
    token: String,
}

impl FileLock {
    fn path(config_file: &Path) -> PathBuf {
        // OsString concatenation: never `display()` (lossy) for a sibling file name (m11).
        let mut name = config_file.as_os_str().to_os_string();
        name.push(".lock");
        PathBuf::from(name)
    }

    /// Acquires the lock with the default thresholds (testable through `acquire_with`).
    fn acquire(config_file: &Path) -> Result<Self, String> {
        Self::acquire_with(config_file, LOCK_STALE_MS, LOCK_TIMEOUT_MS)
    }

    /// Acquires the lock in a bounded loop. `stale_ms` (minimum age before suspecting the
    /// holder is dead) and `timeout_ms` (wait budget) are injectable for fast,
    /// deterministic tests. A stale lock is purged only after checking that its PID is dead.
    fn acquire_with(config_file: &Path, stale_ms: u128, timeout_ms: u128) -> Result<Self, String> {
        let path = Self::path(config_file);
        let mut start = Instant::now();
        let mut holder: Option<String> = None;
        let mut purge_attempted = false;
        loop {
            let token = new_lock_token();
            match create_lock_file(&path, &token) {
                Ok(()) => return Ok(Self { path, token }),
                Err(e)
                    if e.kind() == std::io::ErrorKind::AlreadyExists
                        || e.kind() == std::io::ErrorKind::PermissionDenied =>
                {
                    // Windows may return "Access denied" rather than "already exists" when the
                    // lock is being removed by its holder: contention in both
                    // cases, we retry.
                    // A new holder (another token) means writes go on: the budget starts again,
                    // so a writer is not starved out by a burst of others (seen on slow CI).
                    let seen = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|content| content.split_whitespace().nth(2).map(str::to_string));
                    if seen.is_some() && seen != holder {
                        holder = seen;
                        start = Instant::now();
                    }
                    if start.elapsed().as_millis() >= timeout_ms {
                        if !purge_attempted {
                            purge_attempted = true;
                            match lock_state(&path, stale_ms) {
                                LockState::Stale(pid) => match pid_liveness(pid) {
                                    PidLiveness::Dead => {
                                        // Holder dead: purge then retry.
                                        let _ = std::fs::remove_file(&path);
                                        continue;
                                    }
                                    PidLiveness::Alive => {
                                        return Err(crate::tr!("ai.lock.heldByOther", pid = pid));
                                    }
                                },
                                LockState::Unreadable => {
                                    // Poisoned lock (empty/unreadable content, lock write
                                    // failure): no holder can release it, so it is purged.
                                    let _ = std::fs::remove_file(&path);
                                    continue;
                                }
                                LockState::Gone => {
                                    // File absent: the lock may have been released in the meantime, retry.
                                    continue;
                                }
                                LockState::Active => {}
                            }
                        }
                        return Err(crate::tr!("ai.lock.locked", path = path.display()));
                    }
                    std::thread::sleep(Duration::from_millis(LOCK_RETRY_MS));
                }
                Err(e) => return Err(crate::tr!("ai.lock.failed", error = e)),
            }
        }
    }

    /// Releases the lock **only if it still carries our token**. If our lock was purged
    /// after a diagnostic error and a successor placed another one, we do not destroy
    /// theirs (no cascading lock theft).
    fn release_if_owner(&self) {
        let Ok(content) = std::fs::read_to_string(&self.path) else {
            return;
        };
        if content.split_whitespace().nth(2) == Some(self.token.as_str()) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// Builds a lock without acquisition, to test the `Drop` semantics.
    #[cfg(test)]
    fn for_test(path: PathBuf, token: &str) -> Self {
        Self {
            path,
            token: token.to_string(),
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        self.release_if_owner();
    }
}

enum LockState {
    /// Too young to be suspected stale.
    Active,
    /// Old enough, readable content: the PID is provided to check liveness.
    Stale(u32),
    /// Old enough, empty or unreadable content: nothing can release it, so it is purged (N4).
    Unreadable,
    /// File absent: retry.
    Gone,
}

fn lock_state(path: &Path, stale_ms: u128) -> LockState {
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(_) => return LockState::Gone,
    };
    let old_enough = meta
        .modified()
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map(|age| age.as_millis() >= stale_ms)
        .unwrap_or(false);
    if !old_enough {
        return LockState::Active;
    }
    // Format "pid millis token": the PID is the first field.
    match std::fs::read_to_string(path).ok().and_then(|c| {
        c.split_whitespace()
            .next()
            .and_then(|p| p.parse::<u32>().ok())
    }) {
        Some(pid) => LockState::Stale(pid),
        // Empty or unparseable content (lock write failure): a poisoned lock whose
        // holder will never find its token again. Old enough ⇒ purgeable (N4).
        None => LockState::Unreadable,
    }
}

/// Unique ownership token: PID + millisecond + monotonic counter.
fn new_lock_token() -> String {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{counter}", std::process::id(), now_millis())
}

fn create_lock_file(path: &Path, token: &str) -> std::io::Result<()> {
    // Same primitive as the placement of restored images: `create_new` fails if the
    // file already exists, with no possible race between two processes.
    let mut file = File::options().write(true).create_new(true).open(path)?;
    // Content: PID + creation date (milliseconds) + ownership token.
    let _ = file.write_all(format!("{} {} {token}\n", std::process::id(), now_millis()).as_bytes());
    let _ = file.sync_all();
    Ok(())
}

enum PidLiveness {
    Alive,
    // Built by the Windows probe and the Linux `/proc` probe. On any other system
    // `pid_liveness` always assumes the holder is alive and never builds it.
    #[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
    Dead,
}

/// Liveness probe of a PID (Windows). `OpenProcess` with `PROCESS_QUERY_LIMITED_INFORMATION`
/// fails for an invalid PID (dead process) and succeeds for a live, queryable process.
#[cfg(windows)]
mod process_probe {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(
            dw_desired_access: u32,
            b_inherit_handle: i32,
            dw_process_id: u32,
        ) -> *mut c_void;
        fn CloseHandle(h_object: *mut c_void) -> i32;
    }

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    pub fn is_alive(pid: u32) -> bool {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                false
            } else {
                CloseHandle(handle);
                true
            }
        }
    }
}

#[cfg(windows)]
fn pid_liveness(pid: u32) -> PidLiveness {
    if process_probe::is_alive(pid) {
        PidLiveness::Alive
    } else {
        PidLiveness::Dead
    }
}

/// Linux: a process exists while `/proc/<pid>` does. When `/proc` itself is not mounted the
/// probe cannot tell, so the holder is assumed alive (a lock is never stolen on a guess).
#[cfg(target_os = "linux")]
fn pid_liveness(pid: u32) -> PidLiveness {
    pid_liveness_in(Path::new("/proc"), pid)
}

#[cfg(target_os = "linux")]
fn pid_liveness_in(proc_root: &Path, pid: u32) -> PidLiveness {
    if !proc_root.join("self").exists() {
        return PidLiveness::Alive;
    }
    if proc_root.join(pid.to_string()).exists() {
        PidLiveness::Alive
    } else {
        PidLiveness::Dead
    }
}

/// Other systems (macOS, BSD): the PID cannot be probed reliably. The holder is assumed
/// **alive**, so a lock is never purged on age alone (safe fallback: better to fail than to
/// steal a lock).
#[cfg(not(any(windows, target_os = "linux")))]
fn pid_liveness(_pid: u32) -> PidLiveness {
    PidLiveness::Alive
}

/// **Atomic** write: temporary file with a unique name, then rename. A crash during
/// the write leaves the old file intact, never a truncated file. The temporary file is removed
/// on a best-effort basis if the rename fails, so no `.tmp*` leftover remains.
fn write_to(path: &Path, config: &AiConfig) -> Result<(), String> {
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    let temp_path = temporary_file_path(path);
    write_private(&temp_path, content.as_bytes())
        .map_err(|e| crate::tr!("ai.settings.writeFailed", error = e))?;
    // fsync of the temporary file before the rename: a power cut right after the rename must not
    // leave content not yet written to disk.
    if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&temp_path) {
        let _ = f.sync_all();
    }
    if let Err(e) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(crate::tr!("ai.settings.replaceFailed", error = e));
    }
    Ok(())
}

/// Writes a file readable by its owner only on Unix (mode 0600); plain write on Windows, where
/// the folder's ACL already restricts the file to the user.
fn write_private(path: &Path, content: &[u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(content)
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, content)
    }
}

/// Unique temporary file name: `<path>.tmp-<pid>-<millis>-<counter>` (OsString concatenation,
/// never lossy `display()`: m11).
fn temporary_file_path(path: &Path) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(
        ".tmp-{}-{}-{counter}",
        std::process::id(),
        now_millis()
    ));
    PathBuf::from(name)
}

/// Age beyond which an orphaned temporary file (crash during a write) is swept.
const ORPHAN_TEMP_MAX_AGE: Duration = Duration::from_secs(3600);

/// **Best-effort** sweep of orphaned temporary files `<name>.tmp-*` in the configuration folder whose
/// mtime exceeds `max_age`. Never propagates a failure (a failed cleanup must not make a
/// successful write fail). `max_age` is injectable for tests.
fn sweep_stale_temporaries(config_path: &Path, max_age: Duration) {
    let (Some(dir), Some(base)) = (config_path.parent(), config_path.file_name()) else {
        return;
    };
    let mut prefix = base.to_os_string();
    prefix.push(".tmp");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for input in entries.flatten() {
        let name = input.file_name();
        if !name
            .as_encoded_bytes()
            .starts_with(prefix.as_encoded_bytes())
        {
            continue;
        }
        let Ok(meta) = input.metadata() else {
            continue;
        };
        let old_enough = meta
            .modified()
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map(|age| age >= max_age)
            .unwrap_or(false);
        if old_enough {
            let _ = std::fs::remove_file(input.path());
        }
    }
}

fn decrypt(stored: &StoredKey) -> Option<String> {
    match stored {
        // Old format, still readable, migrated on the next write.
        StoredKey::Plain(key) => Some(key.clone()),
        StoredKey::Encrypted(bytes) => dpapi::unprotect(bytes)
            .ok()
            .and_then(|plain| String::from_utf8(plain).ok()),
    }
}

fn encrypt(key: &str) -> Result<StoredKey, String> {
    Ok(StoredKey::Encrypted(dpapi::protect(key.as_bytes())?))
}

/// Key entered in the application for this provider (never logged, never sent back to the webview).
pub fn app_key_in(path: &Path, provider_id: &str) -> Option<String> {
    // Pure read: never moves the file, even corrupted or inaccessible (content read
    // may be outdated — a writer may have published a fresh config since: TOCTOU).
    let mut config = prepared_config(path, false).config;
    // An inherited plaintext key must not stay on disk: migration under lock as soon as it is read.
    if has_plaintext_keys(&config) {
        config = migrate_plaintext_keys_under_lock(path, config);
    }
    config
        .keys
        .get(provider_id)
        .and_then(decrypt)
        .filter(|s| !s.trim().is_empty())
}

/// Saves (or erases, if `key` is empty) a provider's key.
///
/// The whole **read-modify-write** cycle is protected by the inter-process lock, so
/// that a key saved by another process during the operation is not lost.
pub fn set_key_in(path: &Path, provider_id: &str, key: &str) -> Result<(), String> {
    let encrypted = if key.trim().is_empty() {
        None
    } else {
        Some(encrypt(key)?)
    };
    update_config_in(path, |config| match encrypted {
        None => {
            config.keys.remove(provider_id);
        }
        Some(stored) => {
            config.keys.insert(provider_id.to_string(), stored);
        }
    })
}

/// Server address entered in the application for this provider (trimmed, non-empty).
pub fn app_url_in(path: &Path, provider_id: &str) -> Option<String> {
    prepared_config(path, false)
        .config
        .urls
        .get(provider_id)
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

/// Saves (or erases, if `url` is empty) a provider's server address, under the same lock
/// and safeguards as the keys.
pub fn set_url_in(path: &Path, provider_id: &str, url: &str) -> Result<(), String> {
    let url = url.trim().to_string();
    update_config_in(path, |config| {
        if url.is_empty() {
            config.urls.remove(provider_id);
        } else {
            config.urls.insert(provider_id.to_string(), url);
        }
    })
}

/// Read-modify-write of the configuration under the inter-process lock (keys and addresses).
fn update_config_in(path: &Path, change: impl FnOnce(&mut AiConfig)) -> Result<(), String> {
    let lock = FileLock::acquire(path)?;
    let result = (|| {
        // Under lock only: a proven `Corrupt` can be quarantined; `Unreadable`
        // (content not obtained) blocks the write without ever touching the file.
        let prepared = prepared_config(path, true);
        if let Some(reason) = prepared.write_blocked {
            return Err(reason);
        }
        let mut config = prepared.config;
        // Inherited plaintext keys are migrated along the way (the lock is already held).
        migrate_in_memory(&mut config);

        change(&mut config);
        write_to(path, &config)?;
        // Semantic closing of the alert (N1): only a successful write after a **healthy** read
        // clears the alert. If this write follows a quarantine, the alert stays displayed until
        // the next healthy write (the user must see the incident).
        if prepared.healthy {
            clear_config_alert();
        }
        // After a successful write, best-effort sweep of orphaned temporary files (m9).
        sweep_stale_temporaries(path, ORPHAN_TEMP_MAX_AGE);
        Ok(())
    })();
    drop(lock);
    result
}

/// Effective key: **the application's takes precedence**, otherwise the environment variable.
pub fn effective_key_in<F: Fn(&str) -> Option<String>>(
    path: &Path,
    provider_id: &str,
    key_env: &str,
    env: F,
) -> (Option<String>, KeyProvenance) {
    if let Some(key) = app_key_in(path, provider_id) {
        return (Some(key), KeyProvenance::App);
    }
    if !key_env.is_empty() {
        if let Some(value) = env(key_env).filter(|v| !v.trim().is_empty()) {
            return (Some(value), KeyProvenance::Env);
        }
    }
    (None, KeyProvenance::None)
}

/// State exposed to the interface: "key defined?" and provenance — **never the key**.
pub fn key_status_in<F: Fn(&str) -> Option<String>>(
    path: &Path,
    provider_id: &str,
    key_env: &str,
    env: F,
) -> (bool, KeyProvenance) {
    let (key, provenance) = effective_key_in(path, provider_id, key_env, env);
    (key.is_some(), provenance)
}

// --- Wrappers using the application's real file ---------------------------------------

pub fn app_key(provider_id: &str) -> Option<String> {
    config_path()
        .ok()
        .and_then(|path| app_key_in(&path, provider_id))
}

pub fn set_key(provider_id: &str, key: &str) -> Result<(), String> {
    set_key_in(&config_path()?, provider_id, key)
}

pub fn app_url(provider_id: &str) -> Option<String> {
    config_path()
        .ok()
        .and_then(|path| app_url_in(&path, provider_id))
}

pub fn set_url(provider_id: &str, url: &str) -> Result<(), String> {
    set_url_in(&config_path()?, provider_id, url)
}

pub fn effective_key<F: Fn(&str) -> Option<String>>(
    provider_id: &str,
    key_env: &str,
    env: F,
) -> (Option<String>, KeyProvenance) {
    match config_path() {
        Ok(path) => effective_key_in(&path, provider_id, key_env, env),
        Err(_) => (None, KeyProvenance::None),
    }
}

pub fn key_status<F: Fn(&str) -> Option<String>>(
    provider_id: &str,
    key_env: &str,
    env: F,
) -> (bool, KeyProvenance) {
    match config_path() {
        Ok(path) => key_status_in(&path, provider_id, key_env, env),
        Err(_) => (false, KeyProvenance::None),
    }
}

/// At-rest protection of keys, with **DPAPI** (bound to the current Windows user).
///
/// Direct FFI implementation (no extra dependency): `CryptProtectData` /
/// `CryptUnprotectData` from `crypt32.dll`. Output blobs are freed with `LocalFree`.
///
/// Known limit: without additional entropy, any process running
/// under the **same account** can decrypt these blobs. This is the DPAPI-user model; a hard-coded
/// entropy would bring only marginal protection.
#[cfg(windows)]
mod dpapi {
    use std::ffi::c_void;

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            p_data_in: *const DataBlob,
            sz_data_descr: *const u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *const c_void,
            p_prompt_struct: *const c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            p_data_in: *const DataBlob,
            ppsz_data_descr: *mut *mut u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *const c_void,
            p_prompt_struct: *const c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(hmem: *mut c_void) -> *mut c_void;
    }

    /// `CRYPTPROTECT_UI_FORBIDDEN`: never a UI prompt from a background process.
    const UI_FORBIDDEN: u32 = 0x1;

    fn blob(data: &[u8]) -> DataBlob {
        DataBlob {
            cb_data: data.len() as u32,
            pb_data: data.as_ptr() as *mut u8,
        }
    }

    /// Copies crypt32's output blob then frees the memory allocated by Windows.
    unsafe fn output_to_vec(output: &DataBlob) -> Vec<u8> {
        let bytes = std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec();
        LocalFree(output.pb_data as *mut c_void);
        bytes
    }

    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        unsafe {
            let input = blob(data);
            let mut output = DataBlob {
                cb_data: 0,
                pb_data: std::ptr::null_mut(),
            };
            let ok = CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                UI_FORBIDDEN,
                &mut output,
            );
            if ok == 0 {
                return Err(crate::tr!("ai.dpapi.encryptDenied"));
            }
            Ok(output_to_vec(&output))
        }
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        unsafe {
            let input = blob(data);
            let mut output = DataBlob {
                cb_data: 0,
                pb_data: std::ptr::null_mut(),
            };
            let mut description: *mut u16 = std::ptr::null_mut();
            let ok = CryptUnprotectData(
                &input,
                &mut description,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                UI_FORBIDDEN,
                &mut output,
            );
            // Windows allocates the description only if a pointer is passed: we free it for
            // hygiene so no allocation is left behind.
            if !description.is_null() {
                LocalFree(description as *mut c_void);
            }
            if ok == 0 {
                return Err(crate::tr!("ai.dpapi.decryptDenied"));
            }
            Ok(output_to_vec(&output))
        }
    }
}

/// Linux and other Unix systems: no DPAPI. Keys are sealed with XChaCha20-Poly1305 under a random
/// 32-byte key kept in `ai-config.key` (mode 0600) next to the settings. Each value carries its
/// own random nonce and is authenticated, so a tampered value is refused, never misread.
///
/// Known limit, the same model as DPAPI: this protects the settings file when it is copied
/// alone (backup, sync, a shared folder), not against a process running under the **same
/// account**, which can read the key file. There is no Secret Service dependency on purpose:
/// it is missing on headless machines and many window managers. The key file must stay with the
/// settings: without it the stored keys cannot be read and must be entered again.
#[cfg(unix)]
mod keyfile {
    use chacha20poly1305::aead::{Aead, KeyInit};
    use chacha20poly1305::{XChaCha20Poly1305, XNonce};
    use std::io::Read;
    use std::path::{Path, PathBuf};

    pub const KEY_FILE: &str = "ai-config.key";
    const NONCE_LEN: usize = 24;
    const KEY_LEN: usize = 32;

    fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
        let mut bytes = [0u8; N];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes))
            .map_err(|e| crate::tr!("ai.keyfile.randomFailed", error = e))?;
        Ok(bytes)
    }

    /// Reads the key file, creating it when absent and `create` is set. Creation is atomic: the
    /// key is written to a private temporary file then hard-linked into place, so two processes
    /// racing on first use end with the same key and nobody reads a half-written file.
    fn load_key(key_path: &Path, create: bool) -> Result<[u8; KEY_LEN], String> {
        match std::fs::read(key_path) {
            Ok(bytes) => {
                return <[u8; KEY_LEN]>::try_from(bytes.as_slice())
                    .map_err(|_| crate::tr!("ai.keyfile.invalid", path = key_path.display()));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && create => {}
            Err(e) => return Err(crate::tr!("ai.keyfile.unreadable", error = e)),
        }
        let key = random_bytes::<KEY_LEN>()?;
        let mut temp = key_path.as_os_str().to_os_string();
        temp.push(format!(".tmp-{}", std::process::id()));
        let temp = PathBuf::from(temp);
        super::write_private(&temp, &key)
            .map_err(|e| crate::tr!("ai.keyfile.unreadable", error = e))?;
        let linked = std::fs::hard_link(&temp, key_path);
        let _ = std::fs::remove_file(&temp);
        match linked {
            Ok(()) => Ok(key),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => load_key(key_path, false),
            Err(e) => Err(crate::tr!("ai.keyfile.unreadable", error = e)),
        }
    }

    pub fn protect_with(key_path: &Path, data: &[u8]) -> Result<Vec<u8>, String> {
        let key = load_key(key_path, true)?;
        let nonce = random_bytes::<NONCE_LEN>()?;
        let sealed = XChaCha20Poly1305::new((&key).into())
            .encrypt(XNonce::from_slice(&nonce), data)
            .map_err(|_| crate::tr!("ai.keyfile.encryptFailed"))?;
        let mut out = nonce.to_vec();
        out.extend(sealed);
        Ok(out)
    }

    pub fn unprotect_with(key_path: &Path, data: &[u8]) -> Result<Vec<u8>, String> {
        if data.len() <= NONCE_LEN {
            return Err(crate::tr!("ai.keyfile.decryptFailed"));
        }
        let key = load_key(key_path, false)?;
        let (nonce, sealed) = data.split_at(NONCE_LEN);
        XChaCha20Poly1305::new((&key).into())
            .decrypt(XNonce::from_slice(nonce), sealed)
            .map_err(|_| crate::tr!("ai.keyfile.decryptFailed"))
    }

    #[cfg(not(test))]
    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        protect_with(&crate::data_location::data_dir()?.join(KEY_FILE), data)
    }

    #[cfg(not(test))]
    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        unprotect_with(&crate::data_location::data_dir()?.join(KEY_FILE), data)
    }
}

#[cfg(all(test, unix))]
#[path = "ai_settings_unix_tests.rs"]
mod unix_tests;

/// Production encryption on Unix (see `keyfile`).
#[cfg(all(unix, not(test)))]
mod dpapi {
    pub use super::keyfile::{protect, unprotect};
}

/// Systems that are neither Windows nor Unix: refuse explicitly rather than store plaintext.
#[cfg(all(not(windows), not(unix), not(test)))]
mod dpapi {
    pub fn protect(_data: &[u8]) -> Result<Vec<u8>, String> {
        Err(crate::tr!("ai.dpapi.unavailable"))
    }

    pub fn unprotect(_data: &[u8]) -> Result<Vec<u8>, String> {
        Err(crate::tr!("ai.dpapi.unavailable"))
    }
}

/// Fake encryption, **reserved for tests outside Windows** (never compiled in production, where
/// the fallback above refuses). It allows checking under Linux the shared logic (lock, atomic
/// write, quarantine, alert, plaintext migration): the stored content is transformed, hence
/// never plaintext, and re-readable. The real DPAPI protection is still exercised by these same tests
/// under Windows, where this module does not exist.
#[cfg(all(not(windows), test))]
mod dpapi {
    const MASK: u8 = 0x5A;

    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        Ok(data.iter().map(|o| o ^ MASK).collect())
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        Ok(data.iter().map(|o| o ^ MASK).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The quarantine alert is one process-wide static, and every healthy write clears it. Run in
    /// parallel, these tests erase each other's alert faster than they can observe it (seen on
    /// Linux, where file operations are quick), so they run one at a time. Tests that spawn
    /// their own threads still exercise real concurrency inside a single test.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static SERIAL: Mutex<()> = Mutex::new(());
        SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn temporary_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "humcase-ai-test-{name}-{}.json",
            std::process::id()
        ));
        clean_up(&path);
        path
    }

    /// Removes the test file and all its leftovers (temporary file, lock, quarantine).
    fn clean_up(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(FileLock::path(path));
        for file in temp_files(path).into_iter().chain(quarantine_files(path)) {
            let _ = std::fs::remove_file(file);
        }
    }

    fn base_name(path: &Path) -> String {
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    fn neighbors(path: &Path, predicate: impl Fn(&str) -> bool) -> Vec<PathBuf> {
        let mut found = Vec::new();
        if let Some(parent) = path.parent() {
            if let Ok(entries) = std::fs::read_dir(parent) {
                for input in entries.flatten() {
                    let name = input.file_name().to_string_lossy().to_string();
                    if predicate(&name) {
                        found.push(input.path());
                    }
                }
            }
        }
        found
    }

    /// Temporary files `<path>.tmp-*` next to the test file.
    fn temp_files(path: &Path) -> Vec<PathBuf> {
        let prefix = format!("{}.tmp", base_name(path));
        neighbors(path, |name| name.starts_with(&prefix))
    }

    /// Quarantine files `<path>.illisible-*` next to the test file.
    fn quarantine_files(path: &Path) -> Vec<PathBuf> {
        let prefix = format!("{}.illisible-", base_name(path));
        neighbors(path, |name| name.starts_with(&prefix))
    }

    /// Any write leftover: temporary file or lock (quarantines are deliberate).
    fn leftovers(path: &Path) -> Vec<PathBuf> {
        let lock = format!("{}.lock", base_name(path));
        let mut found = temp_files(path);
        if let Some(parent) = path.parent() {
            let lock_path = parent.join(&lock);
            if lock_path.exists() {
                found.push(lock_path);
            }
        }
        found
    }

    /// Saving an address keeps the keys (same read-modify-write under the lock), and erasing it
    /// removes only the address.
    #[test]
    fn stores_and_clears_url_without_touching_keys() {
        let _serial = serial();
        let path = temporary_path("url-roundtrip");
        set_key_in(&path, "openai", "sk-secret").unwrap();
        set_url_in(
            &path,
            "ollama",
            "http://localhost:11434/v1/chat/completions",
        )
        .unwrap();
        assert_eq!(
            app_url_in(&path, "ollama").as_deref(),
            Some("http://localhost:11434/v1/chat/completions")
        );
        assert_eq!(app_key_in(&path, "openai").as_deref(), Some("sk-secret"));

        set_key_in(&path, "groq", "gsk-other").unwrap();
        assert!(
            app_url_in(&path, "ollama").is_some(),
            "a key write must keep the address"
        );

        set_url_in(&path, "ollama", "  ").unwrap();
        assert!(app_url_in(&path, "ollama").is_none());
        assert_eq!(app_key_in(&path, "openai").as_deref(), Some("sk-secret"));
        clean_up(&path);
    }

    /// A file written before addresses existed (keys only) is read without loss, and a file
    /// without addresses is written without an `urls` field.
    #[test]
    fn file_without_urls_stays_compatible() {
        let _serial = serial();
        let path = temporary_path("url-legacy");
        std::fs::write(&path, r#"{"cles":{"openai":"sk-plain"}}"#).unwrap();
        assert!(app_url_in(&path, "ollama").is_none());
        assert_eq!(app_key_in(&path, "openai").as_deref(), Some("sk-plain"));
        set_key_in(&path, "groq", "gsk").unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(!written.contains("urls"), "{written}");
        clean_up(&path);
    }

    #[test]
    fn stores_and_clears_key() {
        let _serial = serial();
        let path = temporary_path("roundtrip");
        assert!(app_key_in(&path, "openai").is_none());

        set_key_in(&path, "openai", "sk-secret").unwrap();
        assert_eq!(app_key_in(&path, "openai").as_deref(), Some("sk-secret"));

        set_key_in(&path, "openai", "").unwrap();
        assert!(app_key_in(&path, "openai").is_none());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn app_key_takes_precedence_over_environment() {
        let _serial = serial();
        let path = temporary_path("precedence");
        let env = |_: &str| Some("key-env".to_string());

        // Without an application key: the environment variable is used.
        let (key, provenance) = effective_key_in(&path, "openai", "OPENAI_API_KEY", env);
        assert_eq!(key.as_deref(), Some("key-env"));
        assert_eq!(provenance, KeyProvenance::Env);

        // With an application key: it wins.
        set_key_in(&path, "openai", "key-app").unwrap();
        let (key, provenance) = effective_key_in(&path, "openai", "OPENAI_API_KEY", env);
        assert_eq!(key.as_deref(), Some("key-app"));
        assert_eq!(provenance, KeyProvenance::App);

        // Neither.
        set_key_in(&path, "openai", "").unwrap();
        let (key, provenance) = effective_key_in(&path, "openai", "OPENAI_API_KEY", |_| None);
        assert!(key.is_none());
        assert_eq!(provenance, KeyProvenance::None);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corrupt_file_does_not_crash() {
        let _serial = serial();
        let path = temporary_path("corrupt");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(app_key_in(&path, "openai").is_none());
        // The next write starts again from a healthy configuration (atomically).
        set_key_in(&path, "openai", "sk-x").unwrap();
        assert_eq!(app_key_in(&path, "openai").as_deref(), Some("sk-x"));
        clean_up(&path);
    }

    /// S2 — a **read** never moves a corrupted file (file intact, no copy);
    /// it is the **write** (under lock) that quarantines it before starting again from an empty
    /// config. The copy is kept byte for byte.
    #[test]
    fn corrupt_file_is_preserved_in_quarantine() {
        let _serial = serial();
        let path = temporary_path("quarantine");
        let original: Vec<u8> = b"{ not json".to_vec();
        std::fs::write(&path, &original).unwrap();

        // Pure read: reported but file left intact (M3+M4).
        assert!(app_key_in(&path, "openai").is_none());
        assert!(
            quarantine_files(&path).is_empty(),
            "a read must never move the file"
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);

        // The write under lock quarantines, then starts again from a healthy config.
        set_key_in(&path, "openai", "sk-x").unwrap();
        let copies = quarantine_files(&path);
        assert_eq!(copies.len(), 1, "one quarantine copy expected: {copies:?}");
        assert_eq!(
            std::fs::read(&copies[0]).unwrap(),
            original,
            "the original content must be kept byte for byte"
        );
        assert_eq!(app_key_in(&path, "openai").as_deref(), Some("sk-x"));
        clean_up(&path);
    }

    /// m5 — the **real** alert (exposed static) mentions the kept copy; it is not
    /// rebuilt by the same function as the code. The static is global and shared between
    /// parallel tests: we retry on a fresh file until we observe it.
    #[test]
    fn quarantine_alert_mentions_copy() {
        let _serial = serial();
        let mut seen = false;
        for i in 0..16 {
            let path = temporary_path(&format!("alert-{i}"));
            std::fs::write(&path, "{ not json").unwrap();
            // The quarantine happens at write time (under lock).
            set_key_in(&path, "openai", "sk-x").unwrap();

            let copies = quarantine_files(&path);
            if let (Some(alert), Some(copy)) = (last_config_alert(), copies.first()) {
                if alert.contains(&base_name(copy)) {
                    seen = true;
                }
            }
            clean_up(&path);
            if seen {
                break;
            }
        }
        assert!(seen, "the real alert must mention the kept copy");
    }

    /// M1 — the alert raised by a quarantine must not be cleared by a later read
    /// that no longer sees the file (Absent). Without the fix, `config_for_read` cleared the alert
    /// on `Absent` and the incident disappeared before being displayed. The static is shared:
    /// we retry on a fresh file until we observe it.
    #[test]
    fn quarantine_alert_survives_absent_reads() {
        let _serial = serial();
        let mut seen = false;
        for i in 0..16 {
            let path = temporary_path(&format!("alert-absent-{i}"));
            std::fs::write(&path, "{ not json").unwrap();
            // A write quarantines the corrupted file (the only path allowed to move it).
            set_key_in(&path, "openai", "sk-x").unwrap();
            // The rewritten file disappears (external cleanup): reads see "Absent".
            std::fs::remove_file(&path).unwrap();

            assert!(app_key_in(&path, "openai").is_none());
            if let Some(alert) = last_config_alert() {
                if alert.contains(&crate::tr!(
                    "ai.alert.quarantineCopy",
                    reason = "",
                    path = ""
                )) {
                    seen = true;
                }
            }
            clean_up(&path);
            if seen {
                break;
            }
        }
        assert!(seen, "an Absent read must never clear the quarantine alert");
    }

    /// N1 — the quarantine alert must reach `providers_info` (UI banner) and survive a
    /// healthy read; it is a LATER successful write on a healthy configuration that
    /// closes it. The alert static is shared between parallel tests: bounded loop.
    #[test]
    fn quarantine_alert_reaches_providers_info_until_healthy_write() {
        let _serial = serial();
        let mut seen = false;
        for i in 0..16 {
            let path = temporary_path(&format!("providers-alert-{i}"));
            std::fs::write(&path, "{ not json").unwrap();

            // Write on a corrupted file: quarantine + alert, file rewritten healthy.
            set_key_in(&path, "openai", "sk-x").unwrap();
            let copies = quarantine_files(&path);
            let Some(copy) = copies.first() else {
                clean_up(&path);
                continue;
            };
            let copy_name = base_name(copy);

            // A healthy read must NOT clear the alert (otherwise the banner dies before the UI).
            assert!(app_key_in(&path, "openai").is_some());
            let info = crate::providers::providers_info_in(&path, |_| None);
            let visible = info
                .config_alert
                .as_deref()
                .map(|a| a.contains(&copy_name))
                .unwrap_or(false);
            if !visible {
                // A parallel test may have overwritten the shared static: retry.
                clean_up(&path);
                continue;
            }
            seen = true;

            // Next write on a healthy configuration: semantic closing, our alert disappears.
            set_key_in(&path, "groq", "sk-y").unwrap();
            let after = crate::providers::providers_info_in(&path, |_| None);
            let still = after
                .config_alert
                .as_deref()
                .map(|a| a.contains(&copy_name))
                .unwrap_or(false);
            assert!(!still, "a healthy write must close the quarantine alert");
            clean_up(&path);
            break;
        }
        assert!(
            seen,
            "the quarantine alert must be visible through providers_info_in"
        );
    }

    /// M3+M4 (b) — reading a corrupted file must not move it: it is still present and its
    /// content is unchanged after several reads.
    #[test]
    fn read_path_never_moves_corrupt_file() {
        let _serial = serial();
        let path = temporary_path("read-intact");
        let original: Vec<u8> = b"{ not json".to_vec();
        std::fs::write(&path, &original).unwrap();

        assert!(app_key_in(&path, "openai").is_none());
        assert!(app_key_in(&path, "groq").is_none());

        assert!(
            quarantine_files(&path).is_empty(),
            "a read must never move the corrupted file"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            original,
            "the content must stay intact"
        );
        clean_up(&path);
    }

    /// m10 — the reason for a JSON corruption never echoes the file content.
    #[test]
    fn corrupt_reason_does_not_leak_content() {
        let _serial = serial();
        let path = temporary_path("corruption-reason");
        std::fs::write(&path, "{ not json with a secret sk-xyz").unwrap();
        match read_from(&path) {
            ReadOutcome::Corrupt(reason) => {
                assert!(
                    reason.contains("JSON invalide"),
                    "unexpected reason: {reason}"
                );
                assert!(
                    !reason.contains("sk-xyz"),
                    "the reason must not leak the content: {reason}"
                );
            }
            _ => panic!("invalid JSON content must be classified as Corrupt"),
        }
        let _ = std::fs::remove_file(&path);
    }

    /// m10 — an invalid encoding gives a fixed reason, without the unexpected value.
    #[test]
    fn invalid_utf8_reason_is_fixed() {
        let _serial = serial();
        let path = temporary_path("reason-utf8");
        std::fs::write(&path, [0xff, 0xfe, 0x00, 0x01]).unwrap();
        match read_from(&path) {
            ReadOutcome::Corrupt(reason) => assert_eq!(reason, "contenu non textuel"),
            _ => panic!("non-text content must be classified as Corrupt"),
        }
        let _ = std::fs::remove_file(&path);
    }

    /// S2 — same preservation for a merely truncated JSON (quarantine at write time).
    #[test]
    fn partially_corrupt_file_is_preserved() {
        let _serial = serial();
        let path = temporary_path("truncated");
        let original: Vec<u8> = br#"{"cles":{"openai":"sk-a"},"#.to_vec();
        std::fs::write(&path, &original).unwrap();

        assert!(app_key_in(&path, "openai").is_none());
        assert!(quarantine_files(&path).is_empty());

        set_key_in(&path, "groq", "sk-b").unwrap();
        let copies = quarantine_files(&path);
        assert_eq!(copies.len(), 1, "one quarantine copy expected: {copies:?}");
        assert_eq!(std::fs::read(&copies[0]).unwrap(), original);

        assert_eq!(app_key_in(&path, "groq").as_deref(), Some("sk-b"));
        clean_up(&path);
    }

    /// m9 — the sweep removes old enough orphaned temporary files, and only those.
    #[test]
    fn sweep_removes_only_stale_temporaries() {
        let _serial = serial();
        let path = temporary_path("sweep");
        std::fs::write(&path, "{}").unwrap();
        // A fresh temporary file (mtime now) and another with a different name.
        let fresh = temporary_file_path(&path);
        std::fs::write(&fresh, "leftover").unwrap();

        // With a max age of one hour, the fresh temporary file survives.
        sweep_stale_temporaries(&path, Duration::from_secs(3600));
        assert!(fresh.exists(), "a fresh temporary file must not be swept");

        // With a zero max age, it is removed.
        sweep_stale_temporaries(&path, Duration::ZERO);
        assert!(!fresh.exists(), "a stale temporary file must be swept");
        clean_up(&path);
    }

    /// S2 — an unreadable original that cannot be moved (file opened without delete sharing) must
    /// never be overwritten: the write fails with a clear error.
    #[cfg(windows)]
    #[test]
    fn unreadable_locked_file_blocks_write() {
        let _serial = serial();
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_SHARE_WRITE: u32 = 0x0000_0002;

        let path = temporary_path("locked");
        std::fs::write(&path, "{ not json").unwrap();
        // Opened without FILE_SHARE_DELETE: the quarantine rename must fail.
        let guard = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(&path)
            .unwrap();
        let before = std::fs::read(&path).unwrap();

        let error = set_key_in(&path, "openai", "sk-x").unwrap_err();
        assert!(error.contains("quarantaine"), "unexpected error: {error}");
        assert!(quarantine_files(&path).is_empty());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "the unreadable original must not be modified"
        );

        drop(guard);
        clean_up(&path);
    }

    /// M3+M4 (c) — an ACCESS error (no read sharing) is classified `Unreadable`, never
    /// `Corrupt`: the write is refused, no quarantine is created and the original stays intact.
    #[cfg(windows)]
    #[test]
    fn inaccessible_file_blocks_write_without_quarantine() {
        let _serial = serial();
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_DELETE: u32 = 0x0000_0004;

        let path = temporary_path("access-denied");
        let original: Vec<u8> = b"{\"cles\":{}}".to_vec();
        std::fs::write(&path, &original).unwrap();

        // Handle opened sharing ONLY deletion: any later read fails with an
        // I/O error (not with invalid content).
        let guard = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_DELETE)
            .open(&path)
            .unwrap();

        assert!(
            matches!(read_from(&path), ReadOutcome::Unreadable(_)),
            "an access error must be classified Unreadable, not Corrupt"
        );
        // The read does not move the file and the alert does not lie about a kept copy.
        assert!(app_key_in(&path, "openai").is_none());
        assert!(quarantine_files(&path).is_empty());

        let error = set_key_in(&path, "openai", "sk-x").unwrap_err();
        assert!(
            error == crate::tr!("ai.settings.unreadable"),
            "unclear access error: {error}"
        );
        assert!(
            quarantine_files(&path).is_empty(),
            "a merely inaccessible file must not be quarantined"
        );

        drop(guard);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            original,
            "the inaccessible original must not be modified"
        );
        clean_up(&path);
    }

    /// M5 (a) — `Drop` does not remove a lock whose content carries another token: it
    /// would belong to a successor, and destroying it would steal its lock.
    #[test]
    fn drop_does_not_remove_foreign_lock() {
        let _serial = serial();
        let path = temporary_path("drop-foreign");
        let lock_path = FileLock::path(&path);
        std::fs::write(&lock_path, "999999 0 foreign-token\n").unwrap();

        let lock = FileLock::for_test(lock_path.clone(), "our-token");
        drop(lock);

        assert!(lock_path.exists(), "a foreign lock must not be removed");
        let _ = std::fs::remove_file(&lock_path);
    }

    /// M5 (b) — `Drop` does remove its own lock (same token).
    #[test]
    fn drop_removes_own_lock() {
        let _serial = serial();
        let path = temporary_path("drop-own");
        let lock_path = FileLock::path(&path);
        std::fs::write(&lock_path, "42 0 our-token\n").unwrap();

        let lock = FileLock::for_test(lock_path.clone(), "our-token");
        drop(lock);

        assert!(!lock_path.exists(), "our own lock must be removed");
    }

    /// M5 (c) — a "stale" lock whose PID is dead is purged, and the acquisition succeeds.
    #[cfg(windows)]
    #[test]
    fn stale_lock_of_dead_process_is_purged() {
        let _serial = serial();
        // PID almost certainly dead: a short-lived process started then awaited.
        let pid = {
            let mut child = std::process::Command::new("cmd")
                .args(["/c", "exit"])
                .spawn()
                .unwrap();
            let id = child.id();
            child.wait().unwrap();
            id
        };

        let path = temporary_path("purge-dead");
        let lock_path = FileLock::path(&path);
        std::fs::write(&lock_path, format!("{pid} 0 dead-token\n")).unwrap();

        // Stale threshold = 0 and short budget: the purge must rely on liveness, not wait.
        let lock = FileLock::acquire_with(&path, 0, 200)
            .expect("the lock of a dead process must be purged");
        drop(lock);
        assert!(
            !lock_path.exists(),
            "the purged then released lock must not remain"
        );
    }

    /// M5 (d) — a stale lock whose PID is **alive** is not purged: the acquisition fails
    /// with an error that mentions the PID, and the lock stays intact.
    #[test]
    fn live_process_lock_is_not_purged() {
        let _serial = serial();
        let path = temporary_path("purge-alive");
        let lock_path = FileLock::path(&path);
        let own_pid = std::process::id();
        std::fs::write(&lock_path, format!("{own_pid} 0 alive-token\n")).unwrap();

        let error = FileLock::acquire_with(&path, 0, 120).unwrap_err();
        assert!(
            error.contains(&own_pid.to_string()),
            "the error must mention the live PID: {error}"
        );
        assert!(
            lock_path.exists(),
            "the lock of a live process must not be purged"
        );
        let _ = std::fs::remove_file(&lock_path);
    }

    /// N4 — an empty or unparseable lock (failed write of its content) must not poison
    /// permanently: old enough, it is purged and the acquisition succeeds.
    #[test]
    fn empty_or_unparseable_lock_is_purged() {
        let _serial = serial();
        for (i, content) in ["", "not-a-pid\n", "   \n"].iter().enumerate() {
            let path = temporary_path(&format!("unreadable-lock-{i}"));
            let lock_path = FileLock::path(&path);
            std::fs::write(&lock_path, content).unwrap();

            let lock = FileLock::acquire_with(&path, 0, 200)
                .unwrap_or_else(|e| panic!("lock {content:?} not purged: {e}"));
            drop(lock);
            assert!(
                !lock_path.exists(),
                "the purged then released lock must not remain"
            );
        }
    }

    /// S2 — without an inter-process lock, two threads writing to the same file lose a key.
    /// This test fails (or becomes unstable) if the lock is removed from `set_key_in`.
    #[test]
    fn a_waiting_writer_is_not_starved_while_the_lock_changes_hands() {
        let _serial = serial();
        let path = temporary_path("fair");
        let busy = path.clone();
        let (started, ready) = std::sync::mpsc::channel();
        // Another writer takes the lock again and again for 600 ms, far beyond the 250 ms budget
        // (a loaded machine, such as a parallel Linux test run, can delay a poll by 100 ms).
        let other = std::thread::spawn(move || {
            for round in 0..30 {
                let lock = FileLock::acquire_with(&busy, 60_000, 5_000).unwrap();
                if round == 0 {
                    started.send(()).unwrap();
                }
                std::thread::sleep(Duration::from_millis(20));
                drop(lock);
            }
        });
        ready.recv().unwrap();
        let acquired = FileLock::acquire_with(&path, 60_000, 250).map(drop);
        other.join().unwrap();
        assert!(acquired.is_ok(), "{:?}", acquired.err());
        clean_up(&path);
    }

    #[test]
    fn concurrent_writes_do_not_lose_keys() {
        let _serial = serial();
        use std::sync::{Arc, Barrier};

        let path = temporary_path("concurrency");
        let barrier = Arc::new(Barrier::new(2));

        let path_a = path.clone();
        let barrier_a = Arc::clone(&barrier);
        let thread_a = std::thread::spawn(move || {
            barrier_a.wait();
            for i in 0..20 {
                set_key_in(&path_a, "openai", &format!("openai-{i}")).unwrap();
            }
            set_key_in(&path_a, "openai", "openai-final").unwrap();
        });

        let path_b = path.clone();
        let barrier_b = Arc::clone(&barrier);
        let thread_b = std::thread::spawn(move || {
            barrier_b.wait();
            for i in 0..20 {
                set_key_in(&path_b, "groq", &format!("groq-{i}")).unwrap();
            }
            set_key_in(&path_b, "groq", "groq-final").unwrap();
        });

        thread_a.join().unwrap();
        thread_b.join().unwrap();

        // No key must have been lost through mutual overwriting.
        assert_eq!(app_key_in(&path, "openai").as_deref(), Some("openai-final"));
        assert_eq!(app_key_in(&path, "groq").as_deref(), Some("groq-final"));
        // No temporary file nor lock left after successful writes.
        assert!(
            leftovers(&path).is_empty(),
            "unexpected leftovers: {:?}",
            leftovers(&path)
        );
        clean_up(&path);
    }

    /// S1 — the key must not end up in plaintext in the file (at-rest protection).
    #[test]
    fn key_is_not_plaintext_in_file() {
        let _serial = serial();
        let path = temporary_path("encryption");
        set_key_in(&path, "openai", "sk-secret-very-visible").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            !content.contains("sk-secret-very-visible"),
            "the key must not be stored in plaintext: {content}"
        );
        // ...but it stays readable by the application.
        assert_eq!(
            app_key_in(&path, "openai").as_deref(),
            Some("sk-secret-very-visible")
        );
        let _ = std::fs::remove_file(&path);
    }

    /// S1 — saving a key must not lose the others (atomic write).
    #[test]
    fn new_key_keeps_other_keys() {
        let _serial = serial();
        let path = temporary_path("other-keys");
        set_key_in(&path, "openai", "sk-a").unwrap();
        set_key_in(&path, "groq", "sk-b").unwrap();

        assert_eq!(app_key_in(&path, "openai").as_deref(), Some("sk-a"));
        assert_eq!(app_key_in(&path, "groq").as_deref(), Some("sk-b"));
        // No temporary file nor lock left.
        assert!(
            leftovers(&path).is_empty(),
            "unexpected leftovers: {:?}",
            leftovers(&path)
        );
        clean_up(&path);
    }

    /// S1 — backward compatibility: a plaintext key from an earlier version is read then migrated
    /// **as soon as it is read** (not only on the next write).
    #[test]
    fn plaintext_key_read_then_migrated() {
        let _serial = serial();
        let path = temporary_path("migration");
        std::fs::write(&path, r#"{"cles":{"openai":"sk-legacy"}}"#).unwrap();
        assert_eq!(app_key_in(&path, "openai").as_deref(), Some("sk-legacy"));

        // The mere read has already rewritten the file from plaintext to encrypted.
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            !content.contains("sk-legacy"),
            "the old key must be migrated as soon as it is read: {content}"
        );
        assert_eq!(app_key_in(&path, "openai").as_deref(), Some("sk-legacy"));
        let _ = std::fs::remove_file(&path);
    }
}
