//! Per-operation error enums for the governance arbitration PEP (Policy
//! Enforcement Point).
//!
//! Mirrors the `ClaimWal*Error` / memory-PEP convention: each fallible operation
//! gets its own enum, `#[derive(Debug, Clone, PartialEq, Eq)]`, struct variants
//! carrying `{ path, source }` (the source is a lossy `String` for errors
//! stringified at a crate boundary, since the concrete type lives in another
//! crate and these enums derive `Clone`). No `anyhow`, no `thiserror` (neither
//! is a workspace dep — AGENTS.md forbids them). One mega-enum is deliberately
//! avoided: it accumulates phantom variants a given call site can never produce,
//! defeating exhaustive matching.
//!
//! The four operations map to four enums:
//! - [`RecordError`] — `record::record`
//! - [`ArbitrateError`] — `arbitrate::arbitrate`
//! - [`EscalateError`] — `escalate::escalate`
//! - [`ArbitrationProjectionError`] — `lib::project` (the cold replay path)
//!
//! As in the memory PEP, the PEP only enforces a pure decision; it fails on the
//! storage mechanics (lock, append, sequence), never on policy (a denied
//! arbitration is `DeniedByGate`, not an error).

use std::path::PathBuf;

/// Errors raised by [`crate::record::record`] (and its `*_with_durability`
/// twin). The PEP only enforces a pure idempotency check; it fails on the
/// storage mechanics, never on policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordError {
    /// The exclusive store lock could not be acquired (held by another writer,
    /// or an I/O error). The TOCTOU window cannot be closed without it.
    Lock { path: PathBuf, source: String },
    /// Appending the `Detected` event to the JSONL log failed.
    Append { path: PathBuf, source: String },
    /// Serializing the [`GovernanceEvent`](crate::GovernanceEvent) to JSON failed.
    Serialize { source: String },
    /// Reading the existing log to compute the next sequence / check idempotency
    /// failed.
    Read { path: PathBuf, source: String },
}

impl std::fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lock { path, source } => write!(
                formatter,
                "acquire governance store lock at {} failed: {source}",
                path.display()
            ),
            Self::Append { path, source } => write!(
                formatter,
                "append governance event at {} failed: {source}",
                path.display()
            ),
            Self::Serialize { source } => {
                write!(formatter, "serialize governance event failed: {source}")
            }
            Self::Read { path, source } => write!(
                formatter,
                "read governance log at {} failed: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for RecordError {}

/// Errors raised by [`crate::arbitrate::arbitrate`]. A denied arbitration is
/// [`crate::ArbitrateStatus::DeniedByGate`], not an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArbitrateError {
    /// The exclusive store lock could not be acquired.
    Lock { path: PathBuf, source: String },
    /// Appending the `Resolved` event failed.
    Append { path: PathBuf, source: String },
    /// Serializing the [`GovernanceEvent`](crate::GovernanceEvent) failed.
    Serialize { source: String },
    /// Reading the log to compute the next sequence / locate the conflict failed.
    Read { path: PathBuf, source: String },
}

impl std::fmt::Display for ArbitrateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lock { path, source } => write!(
                formatter,
                "acquire governance store lock at {} failed: {source}",
                path.display()
            ),
            Self::Append { path, source } => write!(
                formatter,
                "append arbitrate event at {} failed: {source}",
                path.display()
            ),
            Self::Serialize { source } => {
                write!(formatter, "serialize arbitrate event failed: {source}")
            }
            Self::Read { path, source } => write!(
                formatter,
                "read governance log at {} failed: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ArbitrateError {}

/// Errors raised by [`crate::escalate::escalate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscalateError {
    /// The exclusive store lock could not be acquired.
    Lock { path: PathBuf, source: String },
    /// Appending the `Escalated` event failed.
    Append { path: PathBuf, source: String },
    /// Serializing the [`GovernanceEvent`](crate::GovernanceEvent) failed.
    Serialize { source: String },
    /// Reading the log to compute the next sequence / locate the conflict failed.
    Read { path: PathBuf, source: String },
}

impl std::fmt::Display for EscalateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lock { path, source } => write!(
                formatter,
                "acquire governance store lock at {} failed: {source}",
                path.display()
            ),
            Self::Append { path, source } => write!(
                formatter,
                "append escalate event at {} failed: {source}",
                path.display()
            ),
            Self::Serialize { source } => {
                write!(formatter, "serialize escalate event failed: {source}")
            }
            Self::Read { path, source } => write!(
                formatter,
                "read governance log at {} failed: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for EscalateError {}

/// Errors raised by [`crate::project`] (the cold replay-on-read path).
///
/// A torn-write tail is NOT an error here: the projection stops at the last
/// valid record and emits a [`crate::ArbitrationProjectionDiagnostic`] (mirrors
/// `ClaimWalProjectionError::RecoveryStopped` and the memory PEP). Only
/// structural I/O / parse failures are errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArbitrationProjectionError {
    /// Opening or reading the governance log file failed.
    Read { path: PathBuf, source: String },
    /// A JSONL line that parsed as a string but failed to deserialize into a
    /// [`GovernanceEvent`](crate::GovernanceEvent). (A line that fails to parse
    /// as JSON at all — a torn final write — is skipped with a diagnostic, not
    /// an error.)
    Parse {
        path: PathBuf,
        line_number: usize,
        source: String,
    },
}

impl std::fmt::Display for ArbitrationProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, source } => write!(
                formatter,
                "read governance log at {} failed: {source}",
                path.display()
            ),
            Self::Parse {
                path,
                line_number,
                source,
            } => write!(
                formatter,
                "parse governance event at {}:{} failed: {source}",
                path.display(),
                line_number,
            ),
        }
    }
}

impl std::error::Error for ArbitrationProjectionError {}
