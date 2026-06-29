//! Shared filesystem primitives for governance CLI modules.
//!
//! - [`atomic_write`]: write-then-rename so a reader never sees a truncated
//!   YAML (review S4.4 bug #2 — partial-write `DoS`).
//! - [`DirLock`]: an exclusive advisory lockfile over a directory, so the
//!   load->decide->write lifecycle of a mutating command is serialized across
//!   concurrent invocations (review S4.4 bug #1 — race condition).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DIR_LOCK_STALE_AFTER: Duration = Duration::from_mins(1);
const DIR_LOCK_RETRY_ATTEMPTS: u32 = 40;
const ATOMIC_WRITE_TEMP_ATTEMPTS: u32 = 16;

/// Write `bytes` to `target` atomically and durably: stage in a temp sibling
/// file, fsync it, rename it into place, then fsync the parent directory on
/// Unix. A reader therefore never observes a half-written contract file, and a
/// crash should not lose the committed rename on filesystems that support
/// directory fsync.
///
/// # Errors
///
/// Returns any filesystem error from creating, writing, syncing, renaming, or
/// parent-directory syncing the target file.
pub fn atomic_write(target: &Path, bytes: &str) -> std::io::Result<()> {
    let parent = target_parent(target);
    let (tmp, file) = create_atomic_temp_file(target, parent)?;
    let result = write_and_rename_atomically(&tmp, target, parent, bytes, file);
    if let Err(error) = result {
        return Err(cleanup_atomic_temp_after_error(error, &tmp, parent));
    }
    Ok(())
}

fn target_parent(target: &Path) -> &Path {
    target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn create_atomic_temp_file(
    target: &Path,
    parent: &Path,
) -> std::io::Result<(PathBuf, std::fs::File)> {
    use std::fs::OpenOptions;
    use std::io::ErrorKind;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let file_name = target
        .file_name()
        .map_or_else(|| "atomic-write".into(), std::ffi::OsStr::to_string_lossy);

    for attempt in 0..ATOMIC_WRITE_TEMP_ATTEMPTS {
        let tmp = parent.join(format!(
            ".{file_name}.{}.{}.{}.tmp",
            std::process::id(),
            nonce,
            attempt
        ));
        match OpenOptions::new().create_new(true).write(true).open(&tmp) {
            Ok(file) => return Ok((tmp, file)),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }

    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        format!(
            "failed to create unique temp file for atomic write to {} after {ATOMIC_WRITE_TEMP_ATTEMPTS} attempts",
            target.display()
        ),
    ))
}

fn write_and_rename_atomically(
    tmp: &Path,
    target: &Path,
    parent: &Path,
    bytes: &str,
    mut file: std::fs::File,
) -> std::io::Result<()> {
    use std::io::Write;

    file.write_all(bytes.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(tmp, target)?;
    #[cfg(unix)]
    sync_parent_dir(parent)?;
    #[cfg(not(unix))]
    sync_parent_dir(parent);
    Ok(())
}

fn cleanup_atomic_temp_after_error(
    original: std::io::Error,
    tmp: &Path,
    parent: &Path,
) -> std::io::Error {
    #[cfg(not(unix))]
    let _ = parent;
    match std::fs::remove_file(tmp) {
        Ok(()) => {
            #[cfg(unix)]
            if let Err(cleanup_error) = sync_parent_dir(parent) {
                return std::io::Error::new(
                    original.kind(),
                    format!(
                        "atomic write failed: {original}; removed temp {} but parent sync failed: {cleanup_error}",
                        tmp.display()
                    ),
                );
            }
            original
        }
        Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => original,
        Err(cleanup_error) => std::io::Error::new(
            original.kind(),
            format!(
                "atomic write failed: {original}; additionally failed to remove temp {}: {cleanup_error}",
                tmp.display()
            ),
        ),
    }
}

#[cfg(unix)]
fn sync_parent_dir(parent: &Path) -> std::io::Result<()> {
    std::fs::File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_dir(_parent: &Path) {}

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
    LegacyOrCorruptStale { age_seconds: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirLockLiveOwner {
    pid: u32,
    acquired_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DirLockInspection {
    Reclaim(DirLockReclaimReason),
    HeldByLiveOwner(DirLockLiveOwner),
    Contended,
}

impl DirLock {
    /// Acquire the lock at `<dir>/<lockfile_name>`, retrying with backoff up to
    /// a bounded contention window. Orphan lockfiles are reclaimed when their
    /// owner pid is not alive. Legacy/corrupt lockfiles without a readable pid
    /// are reclaimed only after the stale threshold.
    ///
    /// # Errors
    ///
    /// Returns filesystem errors from creating/removing the lockfile, or
    /// `WouldBlock` when the lock is held by a live owner or remains contended.
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
                    match inspect_lock(&lock_path) {
                        DirLockInspection::Reclaim(reason) => {
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
                        DirLockInspection::HeldByLiveOwner(owner) => {
                            return Err(std::io::Error::new(
                                ErrorKind::WouldBlock,
                                live_owner_lock_message(dir, &lock_path, lockfile_name, &owner),
                            ));
                        }
                        DirLockInspection::Contended => {}
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

fn inspect_lock(lock_path: &Path) -> DirLockInspection {
    let contents = std::fs::read_to_string(lock_path).ok();
    if let Some(state) = contents.as_deref().and_then(parse_lock_state) {
        if process_may_be_alive(state.pid) {
            return DirLockInspection::HeldByLiveOwner(DirLockLiveOwner {
                pid: state.pid,
                acquired_unix: state.acquired_unix,
            });
        }
        return DirLockInspection::Reclaim(DirLockReclaimReason::DeadPid { pid: state.pid });
    }

    lockfile_modified_age(lock_path).map_or(DirLockInspection::Contended, |age| {
        let age_seconds = age.as_secs();
        if age >= DIR_LOCK_STALE_AFTER {
            DirLockInspection::Reclaim(DirLockReclaimReason::LegacyOrCorruptStale { age_seconds })
        } else {
            DirLockInspection::Contended
        }
    })
}

fn live_owner_lock_message(
    dir: &Path,
    lock_path: &Path,
    lockfile_name: &str,
    owner: &DirLockLiveOwner,
) -> String {
    format!(
        "directory lock contention on {}: {} is held by live owner pid={}, acquired_unix={}. Retry later; do not remove {} while that process is alive",
        dir.display(),
        lockfile_name,
        owner.pid,
        owner.acquired_unix,
        lock_path.display()
    )
}

fn lock_contention_message(dir: &Path, lock_path: &Path, lockfile_name: &str) -> String {
    let lock_description = std::fs::read_to_string(lock_path)
        .ok()
        .and_then(|contents| parse_lock_state(&contents))
        .map_or_else(
            || "owner unknown (legacy/corrupt lockfile)".to_owned(),
            |state| {
                format!(
                    "owner pid={}, acquired_unix={}, token={}",
                    state.pid, state.acquired_unix, state.token
                )
            },
        );
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
        .map_or(0, |d| d.as_secs())
}

fn new_owner_token() -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
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

#[cfg(unix)]
fn process_may_be_alive_platform(pid: u32) -> bool {
    use std::os::raw::c_int;

    unsafe extern "C" {
        fn kill(pid: c_int, sig: c_int) -> c_int;
    }

    let Ok(pid) = c_int::try_from(pid) else {
        return false;
    };

    // SAFETY: `kill` is called with signal 0, which performs permission and
    // existence checks without delivering a signal. `pid` was range-checked for
    // the platform C integer type above.
    if unsafe { kill(pid, 0) } == 0 {
        return true;
    }

    match std::io::Error::last_os_error().raw_os_error() {
        Some(1) => true,  // EPERM: process exists, but is not signalable.
        Some(3) => false, // ESRCH: no such process.
        _ => true,        // Unknown errors are treated conservatively as live.
    }
}

#[cfg(windows)]
fn process_may_be_alive_platform(pid: u32) -> bool {
    use std::ffi::c_void;
    type RawHandle = *mut c_void;

    const ERROR_INVALID_PARAMETER: u32 = 87;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "OpenProcess"]
        fn open_process(desired_access: u32, inherit_handle: i32, process_id: u32) -> RawHandle;
        #[link_name = "GetExitCodeProcess"]
        fn get_exit_code_process(process: RawHandle, exit_code: *mut u32) -> i32;
        #[link_name = "CloseHandle"]
        fn close_handle(object: RawHandle) -> i32;
        #[link_name = "GetLastError"]
        fn get_last_error() -> u32;
    }

    // SAFETY: The FFI calls use documented Win32 APIs. The handle returned by
    // `OpenProcess` is checked for null before use and is closed exactly once.
    unsafe {
        let handle = open_process(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return get_last_error() != ERROR_INVALID_PARAMETER;
        }

        let mut exit_code = 0;
        let alive = get_exit_code_process(handle, std::ptr::addr_of_mut!(exit_code)) != 0
            && exit_code == STILL_ACTIVE;
        let _ = close_handle(handle);
        alive
    }
}

#[cfg(not(any(unix, windows)))]
fn process_may_be_alive_platform(_pid: u32) -> bool {
    // Unsupported platforms are treated conservatively so a potentially live
    // owner is not stolen.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
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
    fn atomic_write_writes_final_content_and_removes_temp_file() {
        let dir = temp_dir("atomic-write");
        let target = dir.join("contract.yaml");

        atomic_write(&target, "status: accepted\n").expect("atomic write");

        let contents = std::fs::read_to_string(&target).expect("read target");
        assert_eq!(contents, "status: accepted\n");
        let entries = std::fs::read_dir(&dir)
            .expect("read temp dir")
            .map(|entry| entry.expect("read dir entry").file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, vec![std::ffi::OsString::from("contract.yaml")]);

        let _ = std::fs::remove_dir_all(dir);
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
    fn dir_lock_does_not_reclaim_stale_lockfile_owned_by_current_process() {
        let dir = temp_dir("stale-live-owner");
        let lock_path = dir.join(".lock");
        write_state(
            &lock_path,
            &DirLockFileState {
                pid: std::process::id(),
                acquired_unix: 1,
                token: "stale-live-owner".to_owned(),
            },
        );

        let Err(error) = DirLock::acquire(&dir, ".lock") else {
            panic!("live owner must not be reclaimed");
        };
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert!(
            error.to_string().contains("held by live owner"),
            "unexpected error: {error}"
        );
        let contents = std::fs::read_to_string(&lock_path).expect("read lock");
        let state = parse_lock_state(&contents).expect("parse lock state");
        assert_eq!(state.pid, std::process::id());
        assert_eq!(state.token, "stale-live-owner");

        let _ = std::fs::remove_file(&lock_path);
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
