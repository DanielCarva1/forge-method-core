//! Per-operation error enums for the research source PEP (Policy Enforcement
//! Point). Mirrors the `forge-core-memory` error convention (AGENTS.md: hand
//!-rolled enums, `#[derive(Debug, Clone, PartialEq, Eq)]`, struct variants
//! carrying `{ path, source }` with a lossy `String` source). No `anyhow`, no
//! `thiserror`. One enum per fallible operation so exhaustive matching stays
//! honest (no phantom variants a call site can never produce).
//!
//! The two operations map to two enums:
//! - [`ResearchAdmitError`] — `admission::admit_source`
//! - [`ResearchProjectionError`] — `lib::project` (the cold replay path)
//!
//! A denied admission is [`crate::AdmissionStatus::DeniedByGate`], NOT an
//! error (the PEP enforces a pure decision; it fails on storage mechanics,
//! never on policy).

use std::path::PathBuf;

/// Errors raised by [`crate::admission::admit_source`] (and its
/// `*_with_durability` twin). The PEP only enforces a pure decision; it fails
/// on the storage mechanics (lock, append, sequence), never on policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchAdmitError {
    /// The exclusive store lock could not be acquired (held by another writer,
    /// or an I/O error). The TOCTOU window cannot be closed without it.
    Lock { path: PathBuf, source: String },
    /// Appending the `SourceAdded` event to the JSONL log failed.
    Append { path: PathBuf, source: String },
    /// Serializing the [`ResearchEvent`](crate::ResearchEvent) to JSON failed.
    Serialize { source: String },
    /// Reading the existing log to compute the next sequence number failed.
    Read { path: PathBuf, source: String },
}

impl std::fmt::Display for ResearchAdmitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lock { path, source } => write!(
                formatter,
                "acquire research source lock at {} failed: {source}",
                path.display()
            ),
            Self::Append { path, source } => write!(
                formatter,
                "append research event at {} failed: {source}",
                path.display()
            ),
            Self::Serialize { source } => {
                write!(formatter, "serialize research event failed: {source}")
            }
            Self::Read { path, source } => {
                write!(
                    formatter,
                    "read research log at {} failed: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ResearchAdmitError {}

/// Errors raised by [`crate::project`] (the cold replay-on-read path). A
/// torn-write tail is NOT an error here: the projection stops at the last
/// valid record and emits a [`ResearchProjectionDiagnostic`] (mirrors the
/// memory PEP's tolerance). Only structural I/O / parse failures are errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchProjectionError {
    /// Opening or reading the research log file failed.
    Read { path: PathBuf, source: String },
    /// A JSONL line that parsed as a string but failed to deserialize into a
    /// [`ResearchEvent`](crate::ResearchEvent). (A torn final write — fails to
    /// parse as JSON at all — is skipped with a diagnostic, not an error.)
    Parse {
        path: PathBuf,
        line_number: usize,
        source: String,
    },
}

impl std::fmt::Display for ResearchProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    formatter,
                    "read research log at {} failed: {source}",
                    path.display()
                )
            }
            Self::Parse {
                path,
                line_number,
                source,
            } => write!(
                formatter,
                "parse research event at {}:{} failed: {source}",
                path.display(),
                line_number,
            ),
        }
    }
}

impl std::error::Error for ResearchProjectionError {}
