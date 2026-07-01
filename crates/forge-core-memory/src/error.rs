//! Per-operation error enums for the memory PEP (Policy Enforcement Point).
//!
//! Mirrors the `ClaimWal*Error` convention in `forge-core-store`: each
//! fallible operation gets its own enum, `#[derive(Debug, Clone, PartialEq, Eq)]`,
//! struct variants carrying `{ path, source }` (the source is a lossy `String`
//! for errors stringified at a crate boundary, since the concrete type lives in
//! another crate and these enums derive `Clone`). No `anyhow`, no `thiserror`
//! (neither is a workspace dep — AGENTS.md forbids them). One mega-enum is
//! deliberately avoided: it accumulates phantom variants a given call site can
//! never produce, defeating exhaustive matching (redb keeps per-operation enums
//! for the same reason).
//!
//! The four operations map to four enums:
//! - [`AdmitError`] — `admission::admit`
//! - [`PromoteError`] — `promote::promote`
//! - [`ForgetError`] — `retention::forget`
//! - [`MemoryProjectionError`] — `lib::project` (the cold replay path)
//!
//! `Lock`/`Append`/`Serialize` variants stringify the source via `.to_string()`
//! at the boundary, matching `AppendJsonLineError::Lock { source: String }`.

use std::path::PathBuf;

/// Errors raised by [`crate::admission::admit`] (and its `*_with_durability`
/// twin). The PEP only enforces a pure decision; it fails on the storage
/// mechanics (lock, append, sequence), never on policy (a denied admission is
/// [`crate::AdmissionStatus::DeniedByGate`], not an error).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmitError {
    /// The exclusive store lock could not be acquired (held by another writer,
    /// or an I/O error). The TOCTOU window cannot be closed without it.
    Lock { path: PathBuf, source: String },
    /// Appending the `Admitted` event to the JSONL log failed.
    Append { path: PathBuf, source: String },
    /// Serializing the [`MemoryEvent`](crate::MemoryEvent) to JSON failed.
    Serialize { source: String },
    /// Reading the existing log to compute the next sequence number failed.
    Read { path: PathBuf, source: String },
}

impl std::fmt::Display for AdmitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lock { path, source } => {
                write!(
                    formatter,
                    "acquire memory store lock at {} failed: {source}",
                    path.display()
                )
            }
            Self::Append { path, source } => {
                write!(
                    formatter,
                    "append memory event at {} failed: {source}",
                    path.display()
                )
            }
            Self::Serialize { source } => {
                write!(formatter, "serialize memory event failed: {source}")
            }
            Self::Read { path, source } => {
                write!(
                    formatter,
                    "read memory log at {} failed: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for AdmitError {}

/// Errors raised by [`crate::promote::promote`]. A denied promotion is
/// [`crate::PromoteStatus::DeniedByGate`], not an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromoteError {
    /// The exclusive store lock could not be acquired.
    Lock { path: PathBuf, source: String },
    /// Appending the `Promoted` event failed.
    Append { path: PathBuf, source: String },
    /// Serializing the [`MemoryEvent`](crate::MemoryEvent) failed.
    Serialize { source: String },
    /// Reading the log to compute the next sequence / find the entry failed.
    Read { path: PathBuf, source: String },
}

impl std::fmt::Display for PromoteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lock { path, source } => {
                write!(
                    formatter,
                    "acquire memory store lock at {} failed: {source}",
                    path.display()
                )
            }
            Self::Append { path, source } => {
                write!(
                    formatter,
                    "append memory event at {} failed: {source}",
                    path.display()
                )
            }
            Self::Serialize { source } => {
                write!(formatter, "serialize memory event failed: {source}")
            }
            Self::Read { path, source } => {
                write!(
                    formatter,
                    "read memory log at {} failed: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for PromoteError {}

/// Errors raised by [`crate::retention::forget`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgetError {
    /// The exclusive store lock could not be acquired.
    Lock { path: PathBuf, source: String },
    /// Appending the `Forgotten` before-image event failed.
    Append { path: PathBuf, source: String },
    /// Serializing the [`MemoryEvent`](crate::MemoryEvent) (with its before-image) failed.
    Serialize { source: String },
    /// Reading the log to compute the next sequence / locate the prior entry failed.
    Read { path: PathBuf, source: String },
}

impl std::fmt::Display for ForgetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lock { path, source } => {
                write!(
                    formatter,
                    "acquire memory store lock at {} failed: {source}",
                    path.display()
                )
            }
            Self::Append { path, source } => {
                write!(
                    formatter,
                    "append forget event at {} failed: {source}",
                    path.display()
                )
            }
            Self::Serialize { source } => {
                write!(formatter, "serialize forget event failed: {source}")
            }
            Self::Read { path, source } => {
                write!(
                    formatter,
                    "read memory log at {} failed: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ForgetError {}

/// Errors raised by [`crate::project`] (the cold replay-on-read path) and by
/// the list/sweep path in [`crate::retention::list_now`] when it must rebuild.
///
/// A torn-write tail is NOT an error here: the projection stops at the last
/// valid record and emits a [`MemoryProjectionDiagnostic`] (mirrors
/// `ClaimWalProjectionError::RecoveryStopped`). Only structural I/O / parse
/// failures are errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryProjectionError {
    /// Opening or reading the memory log file failed.
    Read { path: PathBuf, source: String },
    /// A JSONL line that parsed as a string but failed to deserialize into a
    /// [`MemoryEvent`](crate::MemoryEvent). (A line that fails to parse as JSON
    /// at all — a torn final write — is skipped with a diagnostic, not an error.)
    Parse {
        path: PathBuf,
        line_number: usize,
        source: String,
    },
}

impl std::fmt::Display for MemoryProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    formatter,
                    "read memory log at {} failed: {source}",
                    path.display()
                )
            }
            Self::Parse {
                path,
                line_number,
                source,
            } => write!(
                formatter,
                "parse memory event at {}:{} failed: {source}",
                path.display(),
                line_number,
            ),
        }
    }
}

impl std::error::Error for MemoryProjectionError {}
