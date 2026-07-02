//! `forge-core-research` — the Policy Enforcement Point (PEP) for the F14
//! research source trust model (ADR-0010).
//!
//! Candidato 1 (in `forge-core-contracts`) built the pure decision function
//! [`ResearchContract::can_admit_source`]. This crate is its enforcement
//! counterpart: it calls that PDP and appends a `SourceAdded` event
//! **atomically** under an exclusive file lock, closing the TOCTOU window
//! between decide and write (CWE-367 — atomicity at the write site; ADR-0010).
//!
//! # Architecture (mirrors ADR-0003 / `forge-core-memory`)
//!
//! - **Event log** (`research/sources.ndjson`): append-only JSONL. The source
//!   of truth. Never mutated in place ("the dataset only grows").
//! - **Projection** ([`ResearchProjection`]): the rebuildable read model
//!   (`source_id → current source`, `superseded` set), rebuilt by replaying
//!   the log (Fowler event-sourcing: discard and rebuild the projection).
//! - **Lock**: `fs4` exclusive OS file lock via
//!   `forge_core_store::acquire_effect_store_lock`, held across decide-and-write.
//!   Reused verbatim — the store crate already implements TOCTOU-safe locking.
//!
//! The PEP **never re-evaluates policy** (Cedar/OPA/XACML): a denied decision
//! is a `DeniedByGate` outcome, not an error, and appends nothing.
//!
//! # Why a separate log from memory (ADR-0010 rationale)
//!
//! Reusing the memory log (treating `ResearchSource` as a `MemoryEvent` kind)
//! would fuse two trust domains — *authority/review* (F06: "is this
//! ground-truth actionable?") and *citation provenance* (F14: "does this
//! point to a registered source?") — in a single event-sourced log. That is
//! the Model B class of bug one layer down. F14 keeps its own log, lock, and
//! projection; the two subsystems share only the `SourceId` namespace at the
//! citation-check boundary (which lives in `forge-core-validate`).

pub mod admission;
pub mod error;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use forge_core_contracts::ResearchSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use admission::{admit_source, admit_source_with_durability, AdmissionResult, AdmissionStatus};
pub use error::{ResearchAdmitError, ResearchProjectionError};

/// State-root-relative path of the append-only research source event log.
pub const RESEARCH_LOG_RELATIVE_PATH: &str = "research/sources.ndjson";

/// State-root-relative path of the exclusive lock guarding the research log.
/// Held across every decide-and-write critical section (CWE-367).
pub const RESEARCH_LOCK_RELATIVE_PATH: &str = "locks/research.sources.lock";

/// The append-only event stream that is the source of truth for the research
/// Source Ledger. One JSON object per line in `research/sources.ndjson`. Never
/// mutated in place; the projection ([`ResearchProjection`]) is the disposable
/// read model.
///
/// Variants mirror the PEP operations:
/// - [`SourceAdded`](ResearchEvent::SourceAdded) — a source entered the ledger,
///   gated by `ResearchContract::can_admit_source`.
/// - [`SourceRetired`](ResearchEvent::SourceRetired) — explicit removal. Carries
///   the FULL before-image (Debezium `before` / Postgres `REPLICA IDENTITY
///   FULL`), making the log reversible-by-replay (mirrors `MemoryEvent::Forgotten`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum ResearchEvent {
    SourceAdded {
        sequence: u64,
        at_unix: u64,
        source: ResearchSource,
    },
    SourceRetired {
        sequence: u64,
        at_unix: u64,
        /// Full prior source — the audit before-image. Replaying the log
        /// without this event restores the source.
        before: ResearchSource,
    },
}

impl ResearchEvent {
    /// The per-log monotonic sequence number of this event.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        match self {
            Self::SourceAdded { sequence, .. } | Self::SourceRetired { sequence, .. } => *sequence,
        }
    }

    /// `at_unix` timestamp (seconds since epoch).
    #[must_use]
    pub fn at_unix(&self) -> u64 {
        match self {
            Self::SourceAdded { at_unix, .. } | Self::SourceRetired { at_unix, .. } => *at_unix,
        }
    }
}

/// Severity for a projection diagnostic. Mirrors the memory PEP's
/// `MemoryProjectionSeverity` granularity (error/warning) without importing
/// the validator crate — this crate stays decoupled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResearchProjectionSeverity {
    Warning,
    Error,
}

/// A non-fatal observation produced while replaying the log (e.g. a torn final
/// line was skipped, or an out-of-order sequence was seen). The projection
/// stops at the last valid record rather than erroring, mirroring the memory
/// PEP's `RecoveryStopped` as a diagnostic, not a hard fail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResearchProjectionDiagnostic {
    pub severity: ResearchProjectionSeverity,
    /// Stable diagnostic code (e.g. `"torn_final_line_skipped"`).
    pub code: String,
    pub message: String,
}

/// The rebuildable read model: `source_id → current source` plus the
/// `superseded` set (retired ids). Rebuilt from scratch by [`replay`]; never
/// the source of truth (the event log is). Last-event-wins per `source_id`,
/// matching the memory PEP's discipline.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResearchProjection {
    /// The highest sequence number applied so far. The next event written is
    /// `sequence + 1`.
    pub sequence: u64,
    /// `source_id.0 → current ResearchSource`. Inserted by `SourceAdded`;
    /// removed by `SourceRetired`.
    pub sources: BTreeMap<String, ResearchSource>,
    /// `source_id.0` values that have been retired. Membership prevents a
    /// stale writer from re-resurrecting a retired id (defence-in-depth).
    pub superseded: BTreeSet<String>,
    /// Non-fatal observations from the last replay.
    pub diagnostics: Vec<ResearchProjectionDiagnostic>,
}

impl ResearchProjection {
    /// Apply a single event, advancing the projection. Idempotent for replay:
    /// an event whose sequence is `<= self.sequence` is ignored (recorded as a
    /// diagnostic) so a partial re-read cannot regress state.
    pub fn apply_event(&mut self, event: &ResearchEvent) {
        let seq = event.sequence();
        if seq <= self.sequence && self.sequence > 0 {
            // Out-of-order / duplicate — do not regress. Diagnose and continue.
            self.diagnostics.push(ResearchProjectionDiagnostic {
                severity: ResearchProjectionSeverity::Warning,
                code: "out_of_order_event_ignored".into(),
                message: format!(
                    "event sequence {seq} <= projection sequence {}; ignored",
                    self.sequence
                ),
            });
            return;
        }
        match event {
            ResearchEvent::SourceAdded { source, .. } => {
                let key = source.id.0.clone();
                self.sources.insert(key, source.clone());
            }
            ResearchEvent::SourceRetired { before, .. } => {
                let key = before.id.0.clone();
                self.sources.remove(&key);
                self.superseded.insert(key);
            }
        }
        self.sequence = seq;
    }

    /// Number of live (non-retired) sources.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Whether there are no live sources.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

/// Replay a stream of events into a fresh projection (Fowler: discard and
/// rebuild the read model from the event log). Events are applied in order;
/// the resulting projection's `sequence` is the max applied sequence.
///
/// This does NOT read the file — it is the pure fold used by both [`project`]
/// (cold read) and tests. File-reading + torn-line tolerance lives in
/// [`project`].
#[must_use]
pub fn replay<'a>(events: impl IntoIterator<Item = &'a ResearchEvent>) -> ResearchProjection {
    let mut projection = ResearchProjection::default();
    for event in events {
        projection.apply_event(event);
    }
    projection
}

/// The status of a [`project`] read. The `Ok` arm always carries the
/// projection (which may itself carry diagnostics); only structural I/O or
/// deserialization failures are `Err`.
pub type ProjectionResult = Result<ResearchProjection, ResearchProjectionError>;

/// Read the research log under the lock and rebuild the projection by replay.
///
/// Torn-final-line tolerance: a trailing line that fails to parse as JSON is
/// skipped with a `torn_final_line_skipped` diagnostic (mirrors the memory
/// PEP's `last_good_offset` recovery). A line that parses as JSON but fails to
/// deserialize as a [`ResearchEvent`] is a hard
/// [`ResearchProjectionError::Parse`] (schema drift, not a torn write).
///
/// `root` is the state root; the log lives at
/// `<root>/<RESEARCH_LOG_RELATIVE_PATH>`.
///
/// **Acquires the lock.** Callers already holding the research lock (the PEP
/// entry point: `admit_source`) must call [`project_locked`] instead — fs4
/// locks are NOT re-entrant, so re-acquiring would self-deadlock (`WouldBlock`).
///
/// # Errors
///
/// Returns [`ResearchProjectionError::Read`] if the lock cannot be acquired or
/// the log file cannot be read (other than `NotFound`, which yields an empty
/// projection); [`ResearchProjectionError::Parse`] if a well-formed JSON line
/// fails to deserialize as a [`ResearchEvent`] (schema drift).
pub fn project(root: impl AsRef<Path>) -> ProjectionResult {
    let root = root.as_ref();
    let _lock = forge_core_store::acquire_effect_store_lock(root, RESEARCH_LOCK_RELATIVE_PATH)
        .map_err(|source| ResearchProjectionError::Read {
            path: resolve_lock_path(root),
            source: source.to_string(),
        })?;
    project_locked(root)
}

/// Rebuild the projection by replaying the log **without** acquiring the lock.
/// For callers that already hold the research lock (the PEP entry point). Does
/// not mutate the filesystem.
///
/// # Errors
///
/// Returns [`ResearchProjectionError::Read`] if the log file cannot be read
/// (other than `NotFound`, which yields an empty projection);
/// [`ResearchProjectionError::Parse`] if a well-formed JSON line fails to
/// deserialize as a [`ResearchEvent`] (schema drift).
pub fn project_locked(root: &Path) -> ProjectionResult {
    let log_path = resolve_log_path(root);
    let bytes = match std::fs::read(&log_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // No log yet — empty projection. Not an error.
            return Ok(ResearchProjection::default());
        }
        Err(source) => {
            return Err(ResearchProjectionError::Read {
                path: log_path,
                source: source.to_string(),
            });
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let total_lines = text.lines().count();
    let mut projection = ResearchProjection::default();
    for (idx, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let event: ResearchEvent = match serde_json::from_str(line) {
            Ok(event) => event,
            Err(source) => {
                // Distinguish a torn final line (JSON incomplete) from a
                // structurally-valid-JSON-but-wrong-shape line. The former is
                // skipped with a diagnostic; the latter is a hard error.
                let is_last = idx + 1 >= total_lines;
                let looks_torn =
                    is_last && (line.is_empty() || !line.starts_with('{') || !line.ends_with('}'));
                if looks_torn {
                    projection.diagnostics.push(ResearchProjectionDiagnostic {
                        severity: ResearchProjectionSeverity::Warning,
                        code: "torn_final_line_skipped".into(),
                        message: format!("skipped incomplete final line {}: {}", idx + 1, source),
                    });
                    break;
                }
                return Err(ResearchProjectionError::Parse {
                    path: log_path.clone(),
                    line_number: idx + 1,
                    source: source.to_string(),
                });
            }
        };
        projection.apply_event(&event);
    }
    Ok(projection)
}

/// Resolve `<root>/<RESEARCH_LOG_RELATIVE_PATH>` to an absolute path for display.
fn resolve_log_path(root: &Path) -> PathBuf {
    root.join(RESEARCH_LOG_RELATIVE_PATH)
}

/// Resolve `<root>/<RESEARCH_LOCK_RELATIVE_PATH>` to an absolute path for display.
fn resolve_lock_path(root: &Path) -> PathBuf {
    root.join(RESEARCH_LOCK_RELATIVE_PATH)
}

/// The next sequence number to write, given the current projection. The PEP
/// computes this under the lock so concurrent writers cannot both pick the same
/// sequence. Sequence starts at 1 (the empty log has projection.sequence == 0).
#[must_use]
pub fn next_sequence(projection: &ResearchProjection) -> u64 {
    projection.sequence.saturating_add(1)
}

/// Best-effort wall-clock seconds since UNIX epoch. Tests override this by
/// passing `at_unix` directly to the PEP; production callers use this. Returns
/// 0 on clock failure (fail-closed to a deterministic value rather than
/// panicking — the timestamp is audit metadata, not a security boundary).
#[must_use]
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::{ResearchSourceKind, SourceId};

    fn sample_source(id: &str) -> ResearchSource {
        ResearchSource {
            id: SourceId(id.into()),
            kind: ResearchSourceKind::Paper,
            title: "A canonical source".into(),
            locator: "https://example.org/source".into(),
            fetched_at: 1_700_000_000,
            content_hash: Some("sha256:abc".into()),
            harvested_by: "agent.1".into(),
            trace_ref: Some("run.1".into()),
        }
    }

    fn added(seq: u64, id: &str) -> ResearchEvent {
        ResearchEvent::SourceAdded {
            sequence: seq,
            at_unix: 1_700_000_000 + seq,
            source: sample_source(id),
        }
    }

    fn retired(seq: u64, id: &str) -> ResearchEvent {
        ResearchEvent::SourceRetired {
            sequence: seq,
            at_unix: 1_700_000_000 + seq,
            before: sample_source(id),
        }
    }

    #[test]
    fn replay_empty_yields_empty_projection() {
        let projection = replay([]);
        assert_eq!(projection.sequence, 0);
        assert!(projection.is_empty());
        assert!(projection.diagnostics.is_empty());
    }

    #[test]
    fn replay_source_added_inserts_and_advances_sequence() {
        let events = [added(1, "s.one")];
        let projection = replay(&events);
        assert_eq!(projection.sequence, 1);
        assert_eq!(projection.len(), 1);
        assert!(projection.sources.contains_key("s.one"));
        assert!(projection.superseded.is_empty());
    }

    #[test]
    fn replay_retired_removes_source_and_marks_superseded() {
        let events = [added(1, "s.one"), retired(2, "s.one")];
        let projection = replay(&events);
        assert!(projection.sources.is_empty());
        assert!(projection.superseded.contains("s.one"));
        assert_eq!(projection.sequence, 2);
    }

    #[test]
    fn replay_ignores_out_of_order_event_without_regressing() {
        let first = added(5, "s.five");
        let mut projection = replay([&first]);
        projection.apply_event(&added(2, "s.two"));
        assert_eq!(projection.sequence, 5, "sequence must not regress");
        assert!(
            !projection.sources.contains_key("s.two"),
            "stale event must not insert"
        );
        assert!(projection
            .diagnostics
            .iter()
            .any(|d| d.code == "out_of_order_event_ignored"));
    }

    #[test]
    fn replay_last_event_wins_on_redundant_add() {
        let mut first = sample_source("s.dup");
        first.title = "first".into();
        let mut second = sample_source("s.dup");
        second.title = "second".into();
        let events = [
            ResearchEvent::SourceAdded {
                sequence: 1,
                at_unix: 1,
                source: first,
            },
            ResearchEvent::SourceAdded {
                sequence: 2,
                at_unix: 2,
                source: second,
            },
        ];
        let projection = replay(&events);
        assert_eq!(projection.sources["s.dup"].title, "second");
    }

    #[test]
    fn event_accessors_return_sequence_and_at_unix() {
        let event = added(7, "s.seven");
        assert_eq!(event.sequence(), 7);
        assert_eq!(event.at_unix(), 1_700_000_007);
    }

    #[test]
    fn next_sequence_starts_at_one_and_saturates() {
        let empty = ResearchProjection::default();
        assert_eq!(next_sequence(&empty), 1);
        let high = ResearchProjection {
            sequence: u64::MAX,
            ..ResearchProjection::default()
        };
        assert_eq!(
            next_sequence(&high),
            u64::MAX,
            "saturating add must not overflow"
        );
    }

    // --- proptest: the Fowler replay-determinism guarantee ---
    #[cfg(test)]
    mod proptests {
        use super::*;
        use forge_core_contracts::SourceId;
        use proptest::prelude::*;

        fn arb_source_id() -> impl Strategy<Value = String> {
            "[a-z]{1,3}\\.source\\.[0-9]{1,3}"
        }

        fn arb_source(id: String) -> ResearchSource {
            ResearchSource {
                id: SourceId(id),
                kind: ResearchSourceKind::Paper,
                title: "t".into(),
                locator: "https://example.org/x".into(),
                fetched_at: 1,
                content_hash: Some("sha256:0".into()),
                harvested_by: "agent.1".into(),
                trace_ref: Some("run.1".into()),
            }
        }

        fn arb_event(seq: u64, id: String) -> ResearchEvent {
            // Two event shapes, picked by seq % 2 to keep it deterministic.
            match seq % 2 {
                0 => ResearchEvent::SourceAdded {
                    sequence: seq,
                    at_unix: seq,
                    source: arb_source(id),
                },
                _ => ResearchEvent::SourceRetired {
                    sequence: seq,
                    at_unix: seq,
                    before: arb_source(id),
                },
            }
        }

        proptest! {
            /// Replay determinism: the same event stream ⇒ the same projection.
            #[test]
            fn replay_is_deterministic(events in proptest::collection::vec(arb_source_id(), 0..20)) {
                let stream: Vec<ResearchEvent> = events
                    .iter()
                    .enumerate()
                    .map(|(i, id)| arb_event((i as u64) + 1, id.clone()))
                    .collect();
                let first = replay(&stream);
                let second = replay(&stream);
                prop_assert_eq!(&first, &second, "replay must be deterministic");
                prop_assert_eq!(first.sequence, stream.len() as u64);
            }

            /// Replay advances sequence monotonically for a monotonic input stream.
            #[test]
            fn replay_advances_sequence_monotonically(events in proptest::collection::vec(arb_source_id(), 1..20)) {
                let stream: Vec<ResearchEvent> = events
                    .iter()
                    .enumerate()
                    .map(|(i, id)| arb_event((i as u64) + 1, id.clone()))
                    .collect();
                let projection = replay(&stream);
                prop_assert_eq!(
                    projection.sequence,
                    u64::try_from(stream.len()).unwrap()
                );
            }
        }
    }
}
