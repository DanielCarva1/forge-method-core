//! Shared filesystem primitives for governance CLI modules.
//!
//! - [`atomic_write`]: write-then-rename so a reader never sees a truncated
//!   YAML (review S4.4 bug #2 — partial-write DoS).
//! - [`DirLock`]: an exclusive advisory lockfile over a directory, so the
//!   load->decide->write lifecycle of a mutating command is serialized across
//!   concurrent invocations (review S4.4 bug #1 — race condition).

use std::path::{Path, PathBuf};

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
/// `create_new`; removed on drop. Serializes lifecycle transitions so two
/// racing mutating commands cannot both win.
///
/// Note: a process that crashes mid-operation leaves an orphan lockfile; the
/// retry timeout bounds the wait, and a human can remove it. This is acceptable
/// for short load->decide->write CLI invocations. Full transactional CAS is
/// future work.
pub struct DirLock {
    path: PathBuf,
}

impl DirLock {
    /// Acquire the lock at `<dir>/<lockfile_name>`, retrying with backoff up to
    /// ~1s of contention.
    pub fn acquire(dir: &Path, lockfile_name: &str) -> std::io::Result<Self> {
        use std::fs::OpenOptions;
        use std::io::ErrorKind;
        use std::time::Duration;
        std::fs::create_dir_all(dir)?;
        let lock_path = dir.join(lockfile_name);
        for attempt in 0..40u32 {
            match OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&lock_path)
            {
                Ok(mut f) => {
                    use std::io::Write;
                    let _ = writeln!(f, "{}", std::process::id());
                    return Ok(DirLock { path: lock_path });
                }
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
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
            format!(
                "directory lock contention on {} (another process holds it; or an orphan {lockfile_name} was left by a crash — remove it)",
                dir.display()
            ),
        ))
    }
}

impl Drop for DirLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
