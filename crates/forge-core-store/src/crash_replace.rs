//! Typed results for Store-owned descriptor-relative crash replacement.
//!
//! Production crash recovery and publication live in [`crate::retained_crash_replace`].
//! This module intentionally exposes no ambient pathname mutation entrypoint.

use forge_core_contracts::ReservedStatePath;
use std::fmt;
use std::path::PathBuf;

/// Durable phases exposed only so focused tests can simulate process loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CrashReplacePhase {
    NextSynced,
    TransactionSynced,
    PreviousInstalled,
    TargetInstalled,
}

/// The deterministic action taken while reconciling replacement sidecars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CrashReplaceRecoveryAction {
    Noop,
    RemovedUncommittedNext,
    AbortedToPrevious,
    RestoredPrevious,
    CommittedInitial,
    CleanedCommitted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashReplaceRecovery {
    pub action: CrashReplaceRecoveryAction,
    pub target_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashReplaceResult {
    pub previous_digest: Option<String>,
    pub installed_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CrashReplaceError {
    InvalidArgument {
        field: &'static str,
        reason: String,
    },
    InvalidPath {
        field: &'static str,
        path: String,
    },
    ReservedStatePath {
        field: &'static str,
        path: String,
        reserved: ReservedStatePath,
    },
    LockScopeMismatch {
        expected: PathBuf,
        actual: PathBuf,
    },
    CompareAndSwapMismatch {
        expected: Option<String>,
        actual: Option<String>,
    },
    SizeLimit {
        path: PathBuf,
        found: u64,
        maximum: u64,
    },
    Protocol {
        reason: String,
    },
    Io {
        path: PathBuf,
        source: String,
    },
    InjectedFault {
        phase: CrashReplacePhase,
    },
}

impl fmt::Display for CrashReplaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument { field, reason } => {
                write!(formatter, "invalid crash-replace {field}: {reason}")
            }
            Self::InvalidPath { field, path } => {
                write!(formatter, "invalid crash-replace {field} path {path}")
            }
            Self::ReservedStatePath {
                field,
                path,
                reserved,
            } => write!(
                formatter,
                "crash-replace {field} path {path} is reserved for EventLog TCB: {reserved:?}"
            ),
            Self::LockScopeMismatch { expected, actual } => write!(
                formatter,
                "crash-replace lock scope mismatch: expected {}, actual {}",
                expected.display(),
                actual.display()
            ),
            Self::CompareAndSwapMismatch { expected, actual } => write!(
                formatter,
                "crash-replace compare-and-swap mismatch: expected {expected:?}, actual {actual:?}"
            ),
            Self::SizeLimit {
                path,
                found,
                maximum,
            } => write!(
                formatter,
                "crash-replace file {} exceeds size limit: {found} > {maximum}",
                path.display()
            ),
            Self::Protocol { reason } => {
                write!(formatter, "crash-replace protocol error: {reason}")
            }
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "crash-replace I/O {} failed: {source}",
                    path.display()
                )
            }
            Self::InjectedFault { phase } => {
                write!(formatter, "injected crash-replace fault after {phase:?}")
            }
        }
    }
}

impl std::error::Error for CrashReplaceError {}
