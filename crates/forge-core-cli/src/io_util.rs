//! Shared filesystem primitives for governance CLI modules.
//!
//! - [`atomic_write`]: write-then-rename so a reader never sees a truncated
//!   YAML (review S4.4 bug #2 — partial-write DoS).
//! - [`DirLock`]: an exclusive advisory lockfile over a directory, so the
//!   load->decide->write lifecycle of a mutating command is serialized across
//!   concurrent invocations (review S4.4 bug #1 — race condition).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DIR_LOCK_STALE_AFTER: Duration = Duration::from_secs(60);
const DIR_LOCK_RETRY_ATTEMPTS: u32 = 40;

/// Write `bytes` to `target` atomically: stage in a temp sibling file, then
/// `rename` (atomic on the same filesystem). A reader therefore never observes
/// a half-written contract file.
pub fn atomic_write(target: &Path, bytes: &str) -> std::io::Result<()> {
    let mut tmp = target.to_path_buf();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    tmp.set_extension(format!("tmp.{}.{}", std::process::id(), nonce));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, target)
}

/// An exclusive advisory lock over a directory, materialized as a lockfile
/// named `lockfile_name` inside that directory. Acquired via atomic
/// `create_new`; removed on drop only when the on-disk ownership token still
/// matches this lock instance. Serializes lifecycle transitions so two racing
/// mutating commands cannot both win.
pub struct DirLock {
    path: PathBuf,
    owner_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirLockFileState {
    pid: u32,
    acquired_unix: u64,
    token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DirLockReclaimReason {
    DeadPid { pid: u32 },
    StaleAge { age_seconds: u64 },
    LegacyOrCorruptStale { age_seconds: u64 },
}

impl DirLock {
    /// Acquire the lock at `<dir>/<lockfile_name>`, retrying with backoff up to
    /// a bounded contention window. Orphan lockfiles are reclaimed when their
    /// owner pid is not alive or the recorded lock age exceeds the stale
    /// threshold.
    pub fn acquire(dir: &Path, lockfile_name: &str) -> std::io::Result<Self> {
        use std::fs::OpenOptions;
        use std::io::ErrorKind;
        std::fs::create_dir_all(dir)?;
        let lock_path = dir.join(lockfile_name);
        for attempt in 0..DIR_LOCK_RETRY_ATTEMPTS {
            let owner_token = new_owner_token();
            match OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&lock_path)
            {
                Ok(mut f) => {
                    let state = DirLockFileState {
                        pid: std::process::id(),
                        acquired_unix: now_unix_seconds(),
                        token: owner_token.clone(),
                    };
                    if let Err(e) = write_lock_state(&mut f, &state) {
                        let _ = std::fs::remove_file(&lock_path);
                        return Err(e);
                    }
                    return Ok(DirLock {
                        path: lock_path,
                        owner_token,
                    });
                }
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    if let Some(reason) = reclaim_reason(&lock_path) {
                        match std::fs::remove_file(&lock_path) {
                            Ok(()) => continue,
                            Err(remove_error) if remove_error.kind() == ErrorKind::NotFound => {
                                continue;
                            }
                            Err(remove_error) => {
                                return Err(std::io::Error::new(
                                    remove_error.kind(),
                                    format!(
                                        "failed to reclaim stale directory lock {} ({}) because removal failed: {}",
                                        lock_path.display(),
                                        reason.describe(),
                                        remove_error
                                    ),
                                ));
                            }
                        }
                    }
                    // exponential backoff: 2ms, 4ms, ... capped at ~64ms
                    let shift = attempt.min(5);
                    let backoff_ms = 2_u64.checked_shl(shift).unwrap_or(64);
                    std::thread::sleep(Duration::from_millis(backoff_ms));
                }
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::new(
            ErrorKind::WouldBlock,
            lock_contention_message(dir, &lock_path, lockfile_name),
        ))
    }
}

impl Drop for DirLock {
    fn drop(&mut self) {
        let Ok(contents) = std::fs::read_to_string(&self.path) else {
            return;
        };
        let Some(state) = parse_lock_state(&contents) else {
            return;
        };
        if state.token == self.owner_token {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl DirLockReclaimReason {
    fn describe(&self) -> String {
        match self {
            DirLockReclaimReason::DeadPid { pid } => format!("owner pid {pid} is not alive"),
            DirLockReclaimReason::StaleAge { age_seconds } => {
                format!("lock age {age_seconds}s exceeds stale threshold")
            }
            DirLockReclaimReason::LegacyOrCorruptStale { age_seconds } => {
                format!("legacy/corrupt lock age {age_seconds}s exceeds stale threshold")
            }
        }
    }
}

fn write_lock_state(f: &mut std::fs::File, state: &DirLockFileState) -> std::io::Result<()> {
    use std::io::Write;
    writeln!(f, "pid={}", state.pid)?;
    writeln!(f, "acquired_unix={}", state.acquired_unix)?;
    writeln!(f, "token={}", state.token)?;
    f.sync_all()
}

fn parse_lock_state(contents: &str) -> Option<DirLockFileState> {
    let mut pid = None;
    let mut acquired_unix = None;
    let mut token = None;
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("pid=") {
            pid = value.parse::<u32>().ok();
        } else if let Some(value) = line.strip_prefix("acquired_unix=") {
            acquired_unix = value.parse::<u64>().ok();
        } else if let Some(value) = line.strip_prefix("token=") {
            if !value.is_empty() {
                token = Some(value.to_owned());
            }
        }
    }
    Some(DirLockFileState {
        pid: pid?,
        acquired_unix: acquired_unix?,
        token: token?,
    })
}

fn reclaim_reason(lock_path: &Path) -> Option<DirLockReclaimReason> {
    let contents = std::fs::read_to_string(lock_path).ok();
    if let Some(state) = contents.as_deref().and_then(parse_lock_state) {
        if !process_may_be_alive(state.pid) {
            return Some(DirLockReclaimReason::DeadPid { pid: state.pid });
        }
        let age_seconds = now_unix_seconds().saturating_sub(state.acquired_unix);
        if age_seconds >= DIR_LOCK_STALE_AFTER.as_secs() {
            return Some(DirLockReclaimReason::StaleAge { age_seconds });
        }
        return None;
    }

    lockfile_modified_age(lock_path).and_then(|age| {
        let age_seconds = age.as_secs();
        if age >= DIR_LOCK_STALE_AFTER {
            Some(DirLockReclaimReason::LegacyOrCorruptStale { age_seconds })
        } else {
            None
        }
    })
}

fn lock_contention_message(dir: &Path, lock_path: &Path, lockfile_name: &str) -> String {
    let lock_description = std::fs::read_to_string(lock_path)
        .ok()
        .and_then(|contents| parse_lock_state(&contents))
        .map(|state| {
            format!(
                "owner pid={}, acquired_unix={}, token={}",
                state.pid, state.acquired_unix, state.token
            )
        })
        .unwrap_or_else(|| "owner unknown (legacy/corrupt lockfile)".to_owned());
    format!(
        "directory lock contention on {}: {} holds {}. Retry later, confirm the owner process is alive, or force-remove {} only after verifying it is orphaned",
        dir.display(),
        lock_description,
        lockfile_name,
        lock_path.display()
    )
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn new_owner_token() -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{nonce}", std::process::id())
}

fn lockfile_modified_age(lock_path: &Path) -> Option<Duration> {
    std::fs::metadata(lock_path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
}

fn process_may_be_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if pid == std::process::id() {
        return true;
    }
    process_may_be_alive_platform(pid)
}

#[cfg(target_family = "unix")]
fn process_may_be_alive_platform(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

#[cfg(not(target_family = "unix"))]
fn process_may_be_alive_platform(_pid: u32) -> bool {
    // Without adding a platform-specific dependency, keep non-current pids as
    // possibly alive on non-Unix targets and rely on the stale-age guard to
    // reclaim orphaned locks safely.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "forge-core-cli-io-util-{name}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn write_state(path: &Path, state: &DirLockFileState) {
        let mut file = std::fs::File::create(path).expect("create lockfile");
        write_lock_state(&mut file, state).expect("write lock state");
    }

    #[test]
    fn dir_lock_records_pid_timestamp_and_token() {
        let dir = temp_dir("records");
        let lock_path = dir.join(".lock");
        let lock = DirLock::acquire(&dir, ".lock").expect("acquire lock");

        let contents = std::fs::read_to_string(&lock_path).expect("read lockfile");
        let state = parse_lock_state(&contents).expect("parse lock state");
        assert_eq!(state.pid, std::process::id());
        assert!(state.acquired_unix > 0);
        assert!(!state.token.is_empty());

        drop(lock);
        assert!(!lock_path.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dir_lock_reclaims_dead_pid_lockfile() {
        let dir = temp_dir("dead-pid");
        let lock_path = dir.join(".lock");
        write_state(
            &lock_path,
            &DirLockFileState {
                pid: 0,
                acquired_unix: now_unix_seconds(),
                token: "dead-owner".to_owned(),
            },
        );

        let lock = DirLock::acquire(&dir, ".lock").expect("reclaim dead pid lock");
        let contents = std::fs::read_to_string(&lock_path).expect("read new lock");
        let state = parse_lock_state(&contents).expect("parse new lock");
        assert_eq!(state.pid, std::process::id());
        assert_ne!(state.token, "dead-owner");

        drop(lock);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dir_lock_reclaims_stale_lockfile() {
        let dir = temp_dir("stale");
        let lock_path = dir.join(".lock");
        write_state(
            &lock_path,
            &DirLockFileState {
                pid: std::process::id(),
                acquired_unix: 1,
                token: "stale-owner".to_owned(),
            },
        );

        let lock = DirLock::acquire(&dir, ".lock").expect("reclaim stale lock");
        let contents = std::fs::read_to_string(&lock_path).expect("read new lock");
        let state = parse_lock_state(&contents).expect("parse new lock");
        assert_eq!(state.pid, std::process::id());
        assert_ne!(state.token, "stale-owner");

        drop(lock);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dir_lock_drop_only_removes_matching_owner_token() {
        let dir = temp_dir("drop-token");
        let lock_path = dir.join(".lock");
        let lock = DirLock::acquire(&dir, ".lock").expect("acquire lock");

        write_state(
            &lock_path,
            &DirLockFileState {
                pid: std::process::id(),
                acquired_unix: now_unix_seconds(),
                token: "different-owner".to_owned(),
            },
        );

        drop(lock);
        assert!(lock_path.exists());
        let _ = std::fs::remove_file(&lock_path);
        let _ = std::fs::remove_dir_all(dir);
    }
}
