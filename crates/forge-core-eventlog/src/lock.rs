//! The RAII exclusive lock guarding an event log.
//!
//! Wraps [`forge_core_store::acquire_effect_store_lock`] +
//! [`forge_core_store::EffectStoreLock`] into a domain-agnostic guard. The
//! underlying implementation (fs4 advisory lock + `Drop` releases) is already
//! correct in the store crate; this module just re-exposes it cleanly as a
//! reusable capability, so each PEP crate (memory/research/governance/…)
//! doesn't hand-roll the same `acquire_effect_store_lock → map_err → hold`
//! dance.
//!
//! fs4 locks are **NOT re-entrant**: a path already held by the current
//! process will self-deadlock (the store's `acquire_effect_store_lock` blocks).
//! Callers that hold this guard across a cold read must call
//! [`project_locked`](crate::project_locked), NOT
//! [`project`](crate::project) (which would re-acquire).

use std::path::{Path, PathBuf};

use forge_core_store::{acquire_effect_store_lock, EffectStoreLock};

use crate::EventLogError;

/// An RAII guard for the exclusive OS file lock on an event log.
///
/// Acquire with [`EventLogLock::acquire`]; release is automatic on `Drop`
/// (delegated to the inner [`EffectStoreLock`], which calls `File::unlock`).
/// Holding this guard witnesses that the caller owns the
/// read-sequence-then-write critical section (CWE-367: atomicity at the write
/// site, not check-fusion — ADR-0002 Decision 1).
///
/// Note that [`crate::append_event`] routes the actual write through
/// `append_json_line_with_durability`, which takes its **own** separate
/// per-path lock internally. The two locks compose: this guard serializes the
/// read-sequence-then-write window so two writers cannot both observe seq=N;
/// the store's internal lock serializes the byte append for torn-write safety.
pub struct EventLogLock {
    inner: EffectStoreLock,
}

impl EventLogLock {
    /// Block until the exclusive lock on `<root>/<lock_relative_path>` is
    /// acquired. Creates the parent dir and the lock file if absent. The lock
    /// is released when the returned guard drops.
    ///
    /// # Errors
    ///
    /// Returns [`EventLogError::Lock`] if the lock path is invalid, its parent
    /// directory cannot be created, the lock file cannot be opened, or the lock
    /// is already held.
    pub fn acquire<D: Clone>(
        root: &Path,
        lock_relative_path: &str,
    ) -> Result<Self, EventLogError<D>> {
        let inner = acquire_effect_store_lock(root, lock_relative_path).map_err(|source| {
            EventLogError::Lock {
                path: root.join(lock_relative_path),
                source: source.to_string(),
            }
        })?;
        Ok(Self { inner })
    }

    /// The absolute path of the lock file this guard holds.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.inner.path()
    }
}

// `EffectStoreLock`'s `Drop` already calls `File::unlock`; nothing extra to do.
// We do NOT impl a manual `Drop` here that would risk a double-unlock.

/// Resolve `<root>/<lock_relative_path>` for display/`path()` of a not-yet-held
/// lock. Used by callers that need the path before acquiring (e.g. for an
/// error variant built around a failed acquire).
#[must_use]
pub fn resolve_lock_path(root: &Path, lock_relative_path: &str) -> PathBuf {
    root.join(lock_relative_path)
}
