//! `forge-core-governance` — the Policy Enforcement Point (PEP) for the F07
//! multi-principal governance arbitration ledger (ADR-0007).
//!
//! The claim engine (`forge-core-engine::claim_engine`) is a pure state machine
//! (DD16): on a path-overlap acquire it returns a
//! [`ConflictContract`](forge_core_contracts::ConflictContract) inside the
//! rejection. But the engine touches no filesystem — emitting the conflict to a
//! durable, queryable store is a *separate* concern, and that is what this
//! crate is. It mirrors the F06 split exactly:
//! `forge-core-contracts`/`MemoryContract::can_admit` = the pure PDP;
//! `forge-core-memory` = the PEP that persists the decision under a lock.
//! Here: `GovernancePolicy::can_arbitrate` = the pure PDP;
//! `arbitrate_with_durability` = the PEP.
//!
//! # Architecture (mirrors `forge-core-memory` / ADR-0003)
//!
//! - **Event log** (`governance/conflicts.ndjson`): append-only JSONL. The
//!   source of truth. Never mutated in place ("the dataset only grows" —
//!   rerun.io).
//! - **Projection** ([`ArbitrationProjection`]): the rebuildable read model
//!   (`conflict_id → current ConflictContract`), rebuilt by replaying the log
//!   (Fowler event-sourcing: discard and rebuild the projection).
//! - **Lock**: exclusive OS file lock via
//!   `forge_core_store::acquire_effect_store_lock`, held across
//!   decide-and-write. Reused verbatim — the store crate already implements
//!   TOCTOU-safe locking (CWE-367 — atomicity at the write site).
//!
//! The PEP **never re-evaluates policy** (Cedar/OPA/XACML): a denied decision
//! is a `DeniedByGate` outcome, not an error, and appends nothing.

pub mod arbitrate;
pub mod error;
pub mod escalate;
pub mod record;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use forge_core_contracts::{
    ConflictContract, ConflictResolutionState, PrincipalId, ResolutionDecision, StableId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use arbitrate::{arbitrate, arbitrate_with_durability, ArbitrateResult, ArbitrateStatus};
pub use error::{ArbitrateError, ArbitrationProjectionError, EscalateError, RecordError};
pub use escalate::{escalate, escalate_with_durability, EscalateResult, EscalateStatus};
pub use record::{record, record_with_durability, RecordResult, RecordStatus};

/// State-root-relative path of the append-only governance arbitration event log.
pub const GOVERNANCE_LOG_RELATIVE_PATH: &str = "governance/conflicts.ndjson";

/// State-root-relative path of the exclusive lock guarding the governance log.
/// Held across every decide-and-write critical section (CWE-367).
pub const GOVERNANCE_LOCK_RELATIVE_PATH: &str = "locks/governance.conflicts.lock";

/// The append-only event stream that is the source of truth for the arbitration
/// ledger. One JSON object per line in `governance/conflicts.ndjson`. Never
/// mutated in place; the projection ([`ArbitrationProjection`]) is the
/// disposable read model.
///
/// Variants mirror the conflict lifecycle:
/// - [`Detected`](GovernanceEvent::Detected) — a conflict entered the ledger at
///   `Pending` resolution (the carrying [`ConflictContract`] is the full
///   attribution record). Emitted by [`record`].
/// - [`Resolved`](GovernanceEvent::Resolved) — an authorized arbiter moved a
///   `Pending` conflict to `Resolved`. Emitted by [`arbitrate`].
/// - [`Escalated`](GovernanceEvent::Escalated) — an authorized arbiter moved a
///   `Pending` conflict to `Escalated`. Emitted by [`escalate`].
///
/// `Resolved`/`Escalated` are transition events: they reference a `conflict_id`
/// rather than re-stating the whole contract, and the projection folds them
/// into the existing conflict's `resolution` field (last-writer-wins on the
/// resolution axis, under the lock).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum GovernanceEvent {
    Detected {
        sequence: u64,
        at_unix: u64,
        conflict: ConflictContract,
    },
    Resolved {
        sequence: u64,
        at_unix: u64,
        conflict_id: StableId,
        arbiter: PrincipalId,
        decision: ResolutionDecision,
    },
    Escalated {
        sequence: u64,
        at_unix: u64,
        conflict_id: StableId,
    },
}

impl GovernanceEvent {
    /// The per-log monotonic sequence number of this event.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        match self {
            Self::Detected { sequence, .. }
            | Self::Resolved { sequence, .. }
            | Self::Escalated { sequence, .. } => *sequence,
        }
    }

    /// `at_unix` timestamp (seconds since epoch).
    #[must_use]
    pub fn at_unix(&self) -> u64 {
        match self {
            Self::Detected { at_unix, .. }
            | Self::Resolved { at_unix, .. }
            | Self::Escalated { at_unix, .. } => *at_unix,
        }
    }
}

/// Severity for a projection diagnostic. Mirrors the memory PEP's
/// `MemoryProjectionSeverity` (and the validate crate's `DiagnosticSeverity`
/// granularity) without importing the validator — this crate stays decoupled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArbitrationProjectionSeverity {
    Warning,
    Error,
}

/// A non-fatal observation produced while replaying the log (e.g. a torn final
/// line was skipped, an out-of-order sequence was seen, or a `Resolved`/
/// `Escalated` event targeted a conflict that was never `Detected`). The
/// projection stops at the last valid record rather than erroring, mirroring
/// `ClaimWalProjectionError::RecoveryStopped` as a diagnostic, not a hard fail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArbitrationProjectionDiagnostic {
    pub severity: ArbitrationProjectionSeverity,
    /// Stable diagnostic code (e.g. `"torn_final_line_skipped"`).
    pub code: String,
    pub message: String,
}

/// The rebuildable read model: `conflict_id → current ConflictContract`. Rebuilt
/// from scratch by [`replay`]; never the source of truth (the event log is).
/// The conflict's `resolution` is folded forward by `Resolved`/`Escalated`
/// events (last-writer-wins under the lock); every other field is set once at
/// `Detected` and never mutated.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArbitrationProjection {
    /// The highest sequence number applied so far. The next event written is
    /// `sequence + 1`.
    pub sequence: u64,
    /// `conflict_id.0 → current ConflictContract`. Inserted by `Detected`;
    /// resolution folded forward by `Resolved`/`Escalated`.
    pub conflicts: BTreeMap<String, ConflictContract>,
    /// Non-fatal observations from the last replay.
    pub diagnostics: Vec<ArbitrationProjectionDiagnostic>,
}

impl ArbitrationProjection {
    /// Apply a single event, advancing the projection. Idempotent for replay:
    /// an event whose sequence is `<= self.sequence` is ignored (recorded as a
    /// diagnostic) so a partial re-read cannot regress state.
    pub fn apply_event(&mut self, event: &GovernanceEvent) {
        let seq = event.sequence();
        if seq <= self.sequence && self.sequence > 0 {
            // Out-of-order / duplicate — do not regress. Diagnose and continue.
            self.diagnostics.push(ArbitrationProjectionDiagnostic {
                severity: ArbitrationProjectionSeverity::Warning,
                code: "out_of_order_event_ignored".into(),
                message: format!(
                    "event sequence {seq} <= projection sequence {}; ignored",
                    self.sequence
                ),
            });
            return;
        }
        match event {
            GovernanceEvent::Detected { conflict, .. } => {
                // Last-Detected-wins on a redundant detect (matches the memory
                // PEP's redundant-Admit rule). The conflict_id is deterministic
                // and ordering-independent (claim_engine::build_conflict sorts
                // principals), so a re-emit carries the same id.
                self.conflicts
                    .insert(conflict.conflict_id.0.clone(), conflict.clone());
            }
            GovernanceEvent::Resolved {
                conflict_id,
                arbiter,
                decision,
                ..
            } => match self.conflicts.get_mut(&conflict_id.0) {
                Some(conflict) => {
                    conflict.resolution = ConflictResolutionState::Resolved {
                        arbiter: arbiter.clone(),
                        decided_at: event.at_unix(),
                        decision: decision.clone(),
                    };
                }
                None => self.diagnostics.push(ArbitrationProjectionDiagnostic {
                    severity: ArbitrationProjectionSeverity::Warning,
                    code: "resolution_target_missing".into(),
                    message: format!("Resolved event targeted unknown conflict {}", conflict_id.0),
                }),
            },
            GovernanceEvent::Escalated { conflict_id, .. } => {
                match self.conflicts.get_mut(&conflict_id.0) {
                    Some(conflict) => {
                        conflict.resolution = ConflictResolutionState::Escalated;
                    }
                    None => self.diagnostics.push(ArbitrationProjectionDiagnostic {
                        severity: ArbitrationProjectionSeverity::Warning,
                        code: "resolution_target_missing".into(),
                        message: format!(
                            "Escalated event targeted unknown conflict {}",
                            conflict_id.0
                        ),
                    }),
                }
            }
        }
        self.sequence = seq;
    }

    /// Number of conflicts in the ledger.
    #[must_use]
    pub fn len(&self) -> usize {
        self.conflicts.len()
    }

    /// Whether the ledger is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.conflicts.is_empty()
    }
}

/// Replay a stream of events into a fresh projection (Fowler: discard and
/// rebuild the read model from the event log). Events are applied in order; the
/// resulting projection's `sequence` is the max applied sequence.
///
/// This does NOT read the file — it is the pure fold used by both
/// [`project`] (cold read) and tests. File-reading + torn-line tolerance lives
/// in [`project`].
#[must_use]
pub fn replay<'a>(events: impl IntoIterator<Item = &'a GovernanceEvent>) -> ArbitrationProjection {
    let mut projection = ArbitrationProjection::default();
    for event in events {
        projection.apply_event(event);
    }
    projection
}

/// The status of an [`project`] read. The `Ok` arm always carries the
/// projection (which may itself carry diagnostics); only structural I/O or
/// deserialization failures are `Err`.
pub type ProjectionResult = Result<ArbitrationProjection, ArbitrationProjectionError>;

/// Read the governance log under the lock and rebuild the projection by replay.
///
/// Torn-final-line tolerance: a trailing line that fails to parse as JSON is
/// skipped with a `torn_final_line_skipped` diagnostic (mirrors `claim_wal.rs`'s
/// `last_good_offset` recovery). A line that parses as JSON but fails to
/// deserialize as a [`GovernanceEvent`] is a hard
/// [`ArbitrationProjectionError::Parse`] (it indicates schema drift, not a torn
/// write).
///
/// `root` is the state root; the log lives at
/// `<root>/<GOVERNANCE_LOG_RELATIVE_PATH>`.
///
/// **Acquires the lock.** Callers already holding the governance lock (the PEP
/// entry points: `record`, `arbitrate`, `escalate`, `list`) must call
/// [`project_locked`] instead — the store lock is NOT re-entrant, so
/// re-acquiring would self-deadlock (return `WouldBlock`).
///
/// # Errors
///
/// Returns [`ArbitrationProjectionError::Read`] if the lock cannot be acquired
/// or the log file cannot be read (other than `NotFound`, which yields an empty
/// projection); [`ArbitrationProjectionError::Parse`] if a well-formed JSON line
/// fails to deserialize as a [`GovernanceEvent`] (schema drift).
pub fn project(root: impl AsRef<Path>) -> ProjectionResult {
    let root = root.as_ref();
    let _lock = forge_core_store::acquire_effect_store_lock(root, GOVERNANCE_LOCK_RELATIVE_PATH)
        .map_err(|source| ArbitrationProjectionError::Read {
            path: resolve_lock_path(root),
            source: source.to_string(),
        })?;
    project_locked(root)
}

/// Rebuild the projection by replaying the log **without** acquiring the lock.
/// For callers that already hold the governance lock (the PEP entry points).
/// Does not mutate the filesystem.
///
/// # Errors
///
/// Returns [`ArbitrationProjectionError::Read`] if the log file cannot be read
/// (other than `NotFound`, which yields an empty projection);
/// [`ArbitrationProjectionError::Parse`] if a well-formed JSON line fails to
/// deserialize as a [`GovernanceEvent`] (schema drift).
pub fn project_locked(root: &Path) -> ProjectionResult {
    let log_path = resolve_log_path(root);
    let bytes = match std::fs::read(&log_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // No log yet — empty projection. Not an error.
            return Ok(ArbitrationProjection::default());
        }
        Err(source) => {
            return Err(ArbitrationProjectionError::Read {
                path: log_path,
                source: source.to_string(),
            });
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut projection = ArbitrationProjection::default();
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();
    for (idx, raw_line) in lines.iter().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let event: GovernanceEvent = match serde_json::from_str(line) {
            Ok(event) => event,
            Err(source) => {
                // Distinguish a torn final line (JSON incomplete) from a
                // structurally-valid-JSON-but-wrong-shape line. The former is
                // skipped with a diagnostic; the latter is a hard error.
                let is_last = idx + 1 >= total;
                let looks_torn =
                    is_last && (line.is_empty() || !line.starts_with('{') || !line.ends_with('}'));
                if looks_torn {
                    projection
                        .diagnostics
                        .push(ArbitrationProjectionDiagnostic {
                            severity: ArbitrationProjectionSeverity::Warning,
                            code: "torn_final_line_skipped".into(),
                            message: format!(
                                "skipped incomplete final line {}: {}",
                                idx + 1,
                                source
                            ),
                        });
                    break;
                }
                return Err(ArbitrationProjectionError::Parse {
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

/// Resolve `<root>/<GOVERNANCE_LOG_RELATIVE_PATH>` for display.
fn resolve_log_path(root: &Path) -> PathBuf {
    root.join(GOVERNANCE_LOG_RELATIVE_PATH)
}

/// Resolve `<root>/<GOVERNANCE_LOCK_RELATIVE_PATH>` for display.
fn resolve_lock_path(root: &Path) -> PathBuf {
    root.join(GOVERNANCE_LOCK_RELATIVE_PATH)
}

/// The next sequence number to write, given the current projection. The PEP
/// computes this under the lock so concurrent writers cannot both pick the same
/// sequence. Sequence starts at 1 (the empty log has projection.sequence == 0).
#[must_use]
pub fn next_sequence(projection: &ArbitrationProjection) -> u64 {
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

/// List conflicts in the ledger, optionally filtered by resolution state.
///
/// Acquires the lock, rebuilds the projection, and returns the conflicts
/// matching `filter` (or all conflicts if `filter` is `None`). Read-only; no
/// event is appended. This is the query backing
/// `forge-core governance conflicts --status open` (F07.6).
///
/// # Errors
///
/// Returns [`ArbitrationProjectionError`] on lock-acquire or read failure.
pub fn list(root: impl AsRef<Path>, filter: Option<ConflictResolutionState>) -> ProjectionResult {
    let projection = project(root)?;
    let conflicts: Vec<ConflictContract> = if let Some(want) = filter {
        projection
            .conflicts
            .into_values()
            .filter(|c| c.resolution == want)
            .collect()
    } else {
        projection.conflicts.into_values().collect()
    };
    Ok(ArbitrationProjection {
        sequence: projection.sequence,
        conflicts: conflicts
            .into_iter()
            .map(|c| (c.conflict_id.0.clone(), c))
            .collect(),
        diagnostics: projection.diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::{ConflictDetectionReason, IntentScope, IntentScopeKind};

    fn sample_conflict(id: &str, principal_a: &str, principal_b: &str) -> ConflictContract {
        ConflictContract {
            conflict_id: StableId(id.into()),
            intent_a: StableId(format!("intent.{principal_a}")),
            intent_b: StableId(format!("intent.{principal_b}")),
            principal_a: PrincipalId(principal_a.into()),
            principal_b: PrincipalId(principal_b.into()),
            contested_scope: IntentScope {
                kind: IntentScopeKind::PathPrefix,
                target: StableId("stories".into()),
            },
            detection_reason: ConflictDetectionReason::PathOverlap,
            detected_at: 1_700_000_000,
            resolution: ConflictResolutionState::Pending,
        }
    }

    fn detected(seq: u64, id: &str) -> GovernanceEvent {
        GovernanceEvent::Detected {
            sequence: seq,
            at_unix: 1_700_000_000 + seq,
            conflict: sample_conflict(id, "principal.alice", "principal.bob"),
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
    fn replay_detected_inserts_conflict_and_advances_sequence() {
        let events = [detected(1, "conflict.1")];
        let projection = replay(&events);
        assert_eq!(projection.sequence, 1);
        assert_eq!(projection.len(), 1);
        assert!(projection.conflicts.contains_key("conflict.1"));
        assert!(projection.diagnostics.is_empty());
    }

    #[test]
    fn replay_resolved_transitions_resolution() {
        let events = [
            detected(1, "conflict.1"),
            GovernanceEvent::Resolved {
                sequence: 2,
                at_unix: 1_700_000_002,
                conflict_id: StableId("conflict.1".into()),
                arbiter: PrincipalId("principal.daniel".into()),
                decision: ResolutionDecision::AwardedTo(PrincipalId("principal.alice".into())),
            },
        ];
        let projection = replay(&events);
        let conflict = &projection.conflicts["conflict.1"];
        assert!(matches!(
            conflict.resolution,
            ConflictResolutionState::Resolved { .. }
        ));
        assert_eq!(projection.sequence, 2);
    }

    #[test]
    fn replay_escalated_transitions_resolution() {
        let events = [
            detected(1, "conflict.1"),
            GovernanceEvent::Escalated {
                sequence: 2,
                at_unix: 1_700_000_002,
                conflict_id: StableId("conflict.1".into()),
            },
        ];
        let projection = replay(&events);
        let conflict = &projection.conflicts["conflict.1"];
        assert_eq!(conflict.resolution, ConflictResolutionState::Escalated);
        assert_eq!(projection.sequence, 2);
    }

    #[test]
    fn replay_resolved_unknown_conflict_emits_diagnostic_without_error() {
        let events = [GovernanceEvent::Resolved {
            sequence: 1,
            at_unix: 1,
            conflict_id: StableId("conflict.missing".into()),
            arbiter: PrincipalId("principal.daniel".into()),
            decision: ResolutionDecision::BothReleased,
        }];
        let projection = replay(&events);
        assert!(projection.conflicts.is_empty());
        assert_eq!(projection.diagnostics.len(), 1);
        assert_eq!(projection.diagnostics[0].code, "resolution_target_missing");
        assert_eq!(projection.sequence, 1);
    }

    #[test]
    fn replay_ignores_out_of_order_event_without_regressing() {
        let first = detected(5, "conflict.five");
        let mut projection = replay([&first]);
        projection.apply_event(&detected(2, "conflict.two"));
        assert_eq!(projection.sequence, 5, "sequence must not regress");
        assert!(
            !projection.conflicts.contains_key("conflict.two"),
            "stale event must not insert"
        );
        assert!(projection
            .diagnostics
            .iter()
            .any(|d| d.code == "out_of_order_event_ignored"));
    }

    #[test]
    fn event_accessors_return_sequence_and_at_unix() {
        let event = detected(7, "conflict.seven");
        assert_eq!(event.sequence(), 7);
        assert_eq!(event.at_unix(), 1_700_000_007);
    }

    #[test]
    fn next_sequence_starts_at_one_and_saturates() {
        let empty = ArbitrationProjection::default();
        assert_eq!(next_sequence(&empty), 1);
        let high = ArbitrationProjection {
            sequence: u64::MAX,
            ..ArbitrationProjection::default()
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
        use proptest::prelude::*;

        fn arb_conflict_id() -> impl Strategy<Value = String> {
            "conflict\\.[a-z]{1,3}\\.[a-z]{1,3}\\.[a-z]{1,5}"
        }

        fn arb_event(seq: u64, id: String) -> GovernanceEvent {
            match seq % 3 {
                0 => GovernanceEvent::Detected {
                    sequence: seq,
                    at_unix: seq,
                    conflict: ConflictContract {
                        conflict_id: StableId(id),
                        intent_a: StableId("intent.a".into()),
                        intent_b: StableId("intent.b".into()),
                        principal_a: PrincipalId("principal.a".into()),
                        principal_b: PrincipalId("principal.b".into()),
                        contested_scope: IntentScope {
                            kind: IntentScopeKind::PathPrefix,
                            target: StableId("t".into()),
                        },
                        detection_reason: ConflictDetectionReason::PathOverlap,
                        detected_at: seq,
                        resolution: ConflictResolutionState::Pending,
                    },
                },
                1 => GovernanceEvent::Resolved {
                    sequence: seq,
                    at_unix: seq,
                    conflict_id: StableId(id),
                    arbiter: PrincipalId("principal.daniel".into()),
                    decision: ResolutionDecision::BothReleased,
                },
                _ => GovernanceEvent::Escalated {
                    sequence: seq,
                    at_unix: seq,
                    conflict_id: StableId(id),
                },
            }
        }

        proptest! {
            /// Replay determinism: the same event stream ⇒ the same projection.
            #[test]
            fn replay_is_deterministic(events in proptest::collection::vec(arb_conflict_id(), 0..20)) {
                let stream: Vec<GovernanceEvent> = events
                    .iter()
                    .enumerate()
                    .map(|(i, id)| arb_event((i as u64) + 1, id.clone()))
                    .collect();
                let first = replay(&stream);
                let second = replay(&stream);
                prop_assert_eq!(&first, &second, "replay must be deterministic");
                prop_assert_eq!(first.sequence, stream.len() as u64);
            }

            /// Replay advances sequence monotonically for a monotonic input
            /// stream. Diagnostics are NOT asserted empty here: a Resolved/
            /// Escalated of a never-Detected id correctly emits
            /// `resolution_target_missing`.
            #[test]
            fn replay_advances_sequence_monotonically(events in proptest::collection::vec(arb_conflict_id(), 1..20)) {
                let stream: Vec<GovernanceEvent> = events
                    .iter()
                    .enumerate()
                    .map(|(i, id)| arb_event((i as u64) + 1, id.clone()))
                    .collect();
                let projection = replay(&stream);
                prop_assert_eq!(projection.sequence, u64::try_from(stream.len()).unwrap());
            }
        }
    }
}
