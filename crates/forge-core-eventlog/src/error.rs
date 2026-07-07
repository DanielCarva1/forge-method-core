//! The generic error enum for event-log mechanics.
//!
//! Collapses the `{Lock, Append, Serialize, Read}` quartet that was copied
//! verbatim across `forge-core-memory`, `forge-core-research`,
//! `forge-core-governance`, and the JSONL half of `forge-core-store` (7
//! near-identical copies). Hand-rolled — NO `anyhow`, NO `thiserror` (neither
//! is a workspace dep). `Debug, Clone, PartialEq, Eq` are derived;
//! `Display`/`std::error::Error` are implemented by hand below.
//!
//! The enum is generic over the projection's `Diagnostic` type so a domain can
//! surface its own diagnostic vocabulary alongside a successful read
//! ([`EventLogError::ProjectionDiagnostic`]). `D` defaults to [`String`] so a
//! domain that only needs the structural quartet can write `EventLogError` with
//! no type parameter. `D: Clone` (not `D: 'static + Clone`) is the bound
//! throughout so domain diagnostics are free to borrow.
//!
//! At a crate boundary the concrete error type of the source lives in another
//! crate, so — matching the existing `*ProjectionError` / `AdmitError`
//! convention — each struct variant carries a lossy `String` for the source.

use std::path::PathBuf;

/// Errors raised by the event-log mechanics in this crate: the cold-read replay
/// path ([`projection::project_locked`](crate::project_locked)), the lock
/// acquire ([`lock::EventLogLock::acquire`](crate::EventLogLock::acquire)), and
/// the append shim ([`append_event`](crate::append_event)).
///
/// A torn-write tail is NOT an error here: the projection stops at the last
/// valid record and emits a [`ProjectionDiagnostic`](Self::ProjectionDiagnostic)
/// (mirrors `ClaimWalProjectionError::RecoveryStopped` as a diagnostic, not a
/// hard fail — see the existing `forge-core-memory`/`-research` PEPs).
///
/// `D` is the domain's projection-diagnostic type; it defaults to [`String`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventLogError<D: Clone = String> {
    /// The exclusive store lock could not be acquired (held by another writer,
    /// or an I/O error). The read-sequence-then-write TOCTOU window cannot be
    /// closed without it (CWE-367).
    Lock {
        /// Absolute path of the lock file.
        path: PathBuf,
        /// Lossy stringified source from `EffectStoreLockError`.
        source: String,
    },
    /// Appending the serialized event to the JSONL log failed.
    Append {
        /// Absolute path of the log file.
        path: PathBuf,
        /// Lossy stringified source from `AppendJsonLineError`.
        source: String,
    },
    /// Serializing the event to JSON failed.
    Serialize {
        /// Lossy stringified `serde_json::Error`.
        source: String,
    },
    /// Reading the existing log to rebuild the projection failed.
    Read {
        /// Absolute path of the log file.
        path: PathBuf,
        /// Lossy stringified `io::Error`.
        source: String,
    },
    /// A JSONL line that parsed as a string but failed to deserialize into the
    /// domain's `Event` type. (A line that fails to parse as JSON at all — a
    /// torn final write — is skipped with a diagnostic, not an error.) This
    /// indicates schema drift, which is a hard fail.
    Parse {
        /// Absolute path of the log file.
        path: PathBuf,
        /// 1-based line number of the offending line.
        line_number: usize,
        /// Lossy stringified `serde_json::Error`.
        source: String,
    },
    /// A projection-level warning carried alongside a (still-`Ok`) read — e.g.
    /// an out-of-order event that was ignored, or a torn final line that was
    /// skipped. The projection returned to the caller is valid up to the last
    /// good record.
    ProjectionDiagnostic(D),
}

impl<D: Clone + std::fmt::Debug> std::fmt::Display for EventLogError<D> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lock { path, source } => {
                write!(
                    formatter,
                    "acquire event-log lock at {} failed: {source}",
                    path.display()
                )
            }
            Self::Append { path, source } => {
                write!(
                    formatter,
                    "append event to {} failed: {source}",
                    path.display()
                )
            }
            Self::Serialize { source } => {
                write!(formatter, "serialize event failed: {source}")
            }
            Self::Read { path, source } => {
                write!(
                    formatter,
                    "read event log at {} failed: {source}",
                    path.display()
                )
            }
            Self::Parse {
                path,
                line_number,
                source,
            } => write!(
                formatter,
                "parse event at {}:{line_number} failed: {source}",
                path.display()
            ),
            Self::ProjectionDiagnostic(diagnostic) => {
                write!(formatter, "projection diagnostic: {diagnostic:?}")
            }
        }
    }
}

impl<D: Clone + std::fmt::Debug> std::error::Error for EventLogError<D> {}
