//! `forge-core-memory` — the Policy Enforcement Point (PEP) for the memory
//! trust model (ADR 0002 + ADR 0003).
//!
//! Candidato 1 (in `forge-core-contracts`) built the pure decision functions
//! (`MemoryContract::can_admit`, `can_promote`, `mark_stale`). This crate is
//! their enforcement counterpart: it calls those PDPs and performs the
//! mutation **atomically** under an exclusive file lock, closing the TOCTOU
//! window between decide and write (CWE-367 — atomicity at the write site, not
//! check-fusion; ADR-0002 Decision 1).
//!
//! # Architecture (ADR 0003)
//!
//! - **Event log** (`memory/events.ndjson`): append-only JSONL. The source of
//!   truth. Never mutated in place ("the dataset only grows" — rerun.io).
//! - **Projection** ([`MemoryProjection`]): the rebuildable read model
//!   (`entry_id → current entry`, `superseded` set), rebuilt by replaying the
//!   log (Fowler event-sourcing: discard and rebuild the projection).
//! - **Lock**: `fs4` exclusive OS file lock via
//!   `forge_core_store::acquire_effect_store_lock`, held across decide-and-write.
//!   Reused verbatim — the store crate already implements TOCTOU-safe locking.
//! - **Lazy TTL**: no background thread; [`retention::list_now`] calls
//!   `MemoryContract::mark_stale` under the read lock and persists flipped
//!   flags (Redis passive-expiry model).
//!
//! The PEP **never re-evaluates policy** (Cedar/OPA/XACML): a denied decision
//! is a `*Status::DeniedByGate` outcome, not an error, and appends nothing.

pub mod admission;
pub mod error;
pub mod promote;
pub mod retention;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use forge_core_contracts::{AdmissionDenialReason, AuthorityLevel, MemoryEntry, StableId};
use forge_core_eventlog::{event_envelope, EventSourced};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// Re-export the generic event-log mechanics under the memory crate's public
// paths so the public API stays stable (`forge-core-cli` uses
// `forge_core_memory::now_unix`). These are now the generic versions
// parameterised over [`MemoryDomain`]'s [`EventSourced`] impl: callers pass
// owned events to `replay`, and `project_locked` takes `(root, log_rel_path)`.
pub use forge_core_eventlog::{next_sequence, now_unix, project_locked, replay};

pub use admission::{admit, admit_with_durability, AdmissionResult, AdmissionStatus};
pub use error::{AdmitError, ForgetError, MemoryProjectionError, PromoteError};
pub use promote::{promote, promote_with_durability, PromoteResult, PromoteStatus};
pub use retention::{
    forget, forget_with_durability, list_now, list_now_with_durability, ForgetResult, ForgetStatus,
    ListResult, ListStatus,
};

/// State-root-relative path of the append-only memory event log.
pub const MEMORY_LOG_RELATIVE_PATH: &str = "memory/events.ndjson";

/// State-root-relative path of the exclusive lock guarding the memory log.
/// Held across every decide-and-write critical section (CWE-367).
pub const MEMORY_LOCK_RELATIVE_PATH: &str = "locks/memory.log.lock";

/// The append-only event stream that is the source of truth for the memory
/// store. One JSON object per line in `memory/events.ndjson`. Never mutated in
/// place; the projection ([`MemoryProjection`]) is the disposable read model.
///
/// Variants mirror the three PEP operations:
/// - [`Admitted`](MemoryEvent::Admitted) — an entry entered the store at the
///   trust floor (`Raw`, `Unreviewed`), gated by `can_admit`.
/// - [`Promoted`](MemoryEvent::Promoted) — authority-axis transition, gated by
///   `can_promote`. Carries `before`/`after` authority for audit. Never touches
///   the review axis.
/// - [`Forgotten`](MemoryEvent::Forgotten) — explicit removal. Carries the FULL
///   before-image (Debezium `before` / Postgres `REPLICA IDENTITY FULL`
///   pattern; ADR-0002 requires the prior `(authority_level, review_state)`,
///   the full entry makes it reversible-by-replay).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum MemoryEvent {
    Admitted {
        sequence: u64,
        at_unix: u64,
        entry: MemoryEntry,
    },
    Promoted {
        sequence: u64,
        at_unix: u64,
        entry_id: StableId,
        before: AuthorityLevel,
        after: AuthorityLevel,
        /// The distinct non-empty raw evidence refs that satisfied the promote gate.
        evidence_refs: Vec<String>,
    },
    Forgotten {
        sequence: u64,
        at_unix: u64,
        /// Full prior entry (content, provenance, both trust axes) — the audit
        /// before-image. Replaying the log without this event restores the entry.
        before: MemoryEntry,
        /// `"sha256:{hex}"` of the JSON-serialized `before` entry — a tamper-evident
        /// fingerprint, matching the repo's `sha256_content_hash` convention.
        content_hash: String,
    },
}

// The `sequence()`/`at_unix()` envelope accessors + `EventEnvelope` impl are
// generated by the `event_envelope!` macro from forge-core-eventlog (the most
// mechanical triplication — every variant carries `sequence`/`at_unix`). Only
// the domain-specific `content_hash_of` stays hand-written here.
event_envelope!(MemoryEvent, [Admitted, Promoted, Forgotten]);

impl MemoryEvent {
    /// Compute the `"sha256:{hex}"` content hash of a serialized entry. Used by
    /// `forget` to stamp [`Forgotten.content_hash`](MemoryEvent::Forgotten).
    /// Matches `forge_core_store::sha256_content_hash` so hashes are comparable
    /// across crates.
    #[must_use]
    pub fn content_hash_of(entry: &MemoryEntry) -> String {
        // Best-effort: serialization of a well-formed MemoryEntry cannot fail;
        // on the impossible failure we hash the raw bytes of the debug form so
        // the field is always populated (never empty — an empty hash would
        // defeat the tamper-evidence purpose).
        let payload =
            serde_json::to_vec(entry).unwrap_or_else(|_| format!("{entry:?}").into_bytes());
        let digest = Sha256::digest(&payload);
        format!("sha256:{digest:x}")
    }
}

/// Severity for a projection diagnostic. Mirrors the validate crate's
/// `DiagnosticSeverity` granularity (error/warning) without importing it —
/// this crate stays decoupled from the validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryProjectionSeverity {
    Warning,
    Error,
}

/// A non-fatal observation produced while replaying the log (e.g. a torn final
/// line was skipped, or an out-of-order sequence was seen). The projection
/// stops at the last valid record rather than erroring, mirroring
/// `ClaimWalProjectionError::RecoveryStopped` as a diagnostic, not a hard fail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemoryProjectionDiagnostic {
    pub severity: MemoryProjectionSeverity,
    /// Stable diagnostic code (e.g. `"torn_final_line_skipped"`).
    pub code: String,
    pub message: String,
}

/// The rebuildable read model: `entry_id → current entry` plus the `superseded`
/// set (forgotten / replaced ids). Rebuilt from scratch by [`replay`]; never
/// the source of truth (the event log is). Last-event-wins per `entry_id`,
/// matching `claim_wal.rs`'s [`apply_record`] discipline.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemoryProjection {
    /// The highest sequence number applied so far. The next event written is
    /// `sequence + 1`.
    pub sequence: u64,
    /// `entry_id.0 → current MemoryEntry`. Inserted/updated by `Admitted` and
    /// `Promoted`; removed by `Forgotten`.
    pub entries: BTreeMap<String, MemoryEntry>,
    /// `entry_id.0` values that have been forgotten. Membership prevents a
    /// stale writer from re-resurrecting a forgotten id (defence-in-depth).
    pub superseded: BTreeSet<String>,
    /// Non-fatal observations from the last replay.
    pub diagnostics: Vec<MemoryProjectionDiagnostic>,
}

impl MemoryProjection {
    /// Apply a single event, advancing the projection. Thin wrapper that
    /// delegates to the generic [`forge_core_eventlog::apply_event`] (which owns
    /// the out-of-order guard); the domain fold itself lives in
    /// [`MemoryDomain`]'s [`EventSourced::apply`] impl. Kept as a method so
    /// existing callers/tests that hold a `&mut MemoryProjection` keep working.
    pub fn apply_event(&mut self, event: &MemoryEvent) {
        forge_core_eventlog::apply_event::<MemoryDomain>(self, event);
    }

    /// Number of live (non-forgotten) entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no live entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The marker type carrying the memory domain's [`EventSourced`] impl. The
/// generic event-log mechanics (`replay`, `project_locked`, `append_event`,
/// `next_sequence`) are parameterised over this type so they resolve to the
/// memory event/projection/diagnostic types.
pub struct MemoryDomain;

impl EventSourced for MemoryDomain {
    type Event = MemoryEvent;
    type Projection = MemoryProjection;
    type Diagnostic = MemoryProjectionDiagnostic;

    fn apply(projection: &mut Self::Projection, event: &Self::Event) {
        match event {
            MemoryEvent::Admitted { entry, .. } => {
                let key = entry.entry_id.0.clone();
                projection.entries.insert(key, entry.clone());
            }
            MemoryEvent::Promoted {
                entry_id,
                after,
                evidence_refs,
                ..
            } => {
                if let Some(existing) = projection.entries.get_mut(&entry_id.0) {
                    existing.authority_level = Some(*after);
                    // Record the promoting evidence on the entry for downstream
                    // inspection (matches how a CLI `promote` would surface it).
                    // Stored lossily as a comma-joined provenance note so we do
                    // not widen MemoryEntry's schema here.
                    let _ = evidence_refs; // (audit-only; see the event log for the refs)
                } else {
                    projection.diagnostics.push(MemoryProjectionDiagnostic {
                        severity: MemoryProjectionSeverity::Warning,
                        code: "promote_target_missing".into(),
                        message: format!("promote targeted unknown entry {}", entry_id.0),
                    });
                }
            }
            MemoryEvent::Forgotten { before, .. } => {
                let key = before.entry_id.0.clone();
                projection.entries.remove(&key);
                projection.superseded.insert(key);
            }
        }
    }

    fn record_diagnostic(projection: &mut Self::Projection, diagnostic: Self::Diagnostic) {
        projection.diagnostics.push(diagnostic);
    }

    fn sequence_of(projection: &Self::Projection) -> u64 {
        projection.sequence
    }

    fn advance_sequence(projection: &mut Self::Projection, new_sequence: u64) {
        projection.sequence = new_sequence;
    }

    fn diagnostic_out_of_order_event_ignored(
        event_seq: u64,
        projection_seq: u64,
    ) -> Self::Diagnostic {
        MemoryProjectionDiagnostic {
            severity: MemoryProjectionSeverity::Warning,
            code: forge_core_eventlog::CODE_OUT_OF_ORDER_EVENT_IGNORED.into(),
            message: format!(
                "event sequence {event_seq} <= projection sequence {projection_seq}; ignored"
            ),
        }
    }

    fn diagnostic_torn_final_line_skipped(
        line_number: usize,
        source: &serde_json::Error,
    ) -> Self::Diagnostic {
        MemoryProjectionDiagnostic {
            severity: MemoryProjectionSeverity::Warning,
            code: forge_core_eventlog::CODE_TORN_FINAL_LINE_SKIPPED.into(),
            message: format!("skipped incomplete final line {line_number}: {source}"),
        }
    }
}

/// The status of an [`project`] read. The `Ok` arm always carries the
/// projection (which may itself carry diagnostics); only structural I/O or
/// deserialization failures are `Err`. Now an alias for
/// [`EventLogError<MemoryProjectionDiagnostic>`](forge_core_eventlog::EventLogError).
pub type ProjectionResult = Result<MemoryProjection, MemoryProjectionError>;

/// Read the memory log under the lock and rebuild the projection by replay.
///
/// **Acquires the lock.** Callers already holding the memory lock (the PEP
/// entry points: `admit`, `promote`, `forget`, `list_now`) must call
/// [`forge_core_eventlog::project_locked`] instead — fs4 locks are NOT
/// re-entrant, so re-acquiring would self-deadlock (return `WouldBlock`).
///
/// The cold-read body, torn-final-line tolerance, and out-of-order guard live
/// in `forge_core_eventlog` (parameterised over [`MemoryDomain`]); this fn is
/// just the lock acquire + delegate.
///
/// # Errors
///
/// Returns [`MemoryProjectionError::Lock`] if the lock cannot be acquired;
/// [`MemoryProjectionError::Read`] if the log file cannot be read (other than
/// `NotFound`, which yields an empty projection); [`MemoryProjectionError::Parse`]
/// if a well-formed JSON line fails to deserialize as a [`MemoryEvent`].
pub fn project(root: impl AsRef<Path>) -> ProjectionResult {
    let root = root.as_ref();
    let _lock = forge_core_eventlog::EventLogLock::acquire::<MemoryProjectionDiagnostic>(
        root,
        MEMORY_LOCK_RELATIVE_PATH,
    )?;
    project_locked::<MemoryDomain>(root, MEMORY_LOG_RELATIVE_PATH)
}

/// The denial reasons carried by a `DeniedByGate` outcome. Re-exported from
/// Candidato 1 so callers of this crate do not need a second import.
pub type DenialReasons = Vec<AdmissionDenialReason>;

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::{ApprovalState, MemoryKind};

    fn sample_entry(id: &str) -> MemoryEntry {
        MemoryEntry {
            entry_id: StableId(id.into()),
            kind: MemoryKind::Preference,
            content: "prefer typed contracts".into(),
            provenance: forge_core_contracts::MemoryProvenance {
                source_run_id: Some(StableId("run.1".into())),
                source_agent: Some(StableId("agent.1".into())),
                evidence_ref: Some("contracts/evidence.yaml".into()),
                captured_at: "1700000000".into(),
            },
            freshness: forge_core_contracts::Freshness {
                ttl_seconds: None,
                last_confirmed_at: "1700000000".into(),
                stale: false,
            },
            confidence: 80,
            approval: ApprovalState::Proposed,
            supersedes: None,
            invalidation_reason: None,
            authority_level: None,
            review_state: None,
            reviewed_by: None,
            reviewed_at: None,
        }
    }

    fn admitted(seq: u64, id: &str) -> MemoryEvent {
        MemoryEvent::Admitted {
            sequence: seq,
            at_unix: 1_700_000_000 + seq,
            entry: sample_entry(id),
        }
    }

    fn promoted(seq: u64, id: &str, before: AuthorityLevel, after: AuthorityLevel) -> MemoryEvent {
        MemoryEvent::Promoted {
            sequence: seq,
            at_unix: 1_700_000_000 + seq,
            entry_id: StableId(id.into()),
            before,
            after,
            evidence_refs: vec!["run.alpha".into()],
        }
    }

    fn forgotten(seq: u64, id: &str) -> MemoryEvent {
        let before = sample_entry(id);
        let content_hash = MemoryEvent::content_hash_of(&before);
        MemoryEvent::Forgotten {
            sequence: seq,
            at_unix: 1_700_000_000 + seq,
            before,
            content_hash,
        }
    }

    #[test]
    fn replay_empty_yields_empty_projection() {
        let projection: MemoryProjection = replay::<MemoryDomain>(std::iter::empty());
        assert_eq!(projection.sequence, 0);
        assert!(projection.is_empty());
        assert!(projection.diagnostics.is_empty());
    }

    #[test]
    fn replay_admitted_inserts_entry_and_advances_sequence() {
        let events = [admitted(1, "e.one")];
        let projection = replay::<MemoryDomain>(events);
        assert_eq!(projection.sequence, 1);
        assert_eq!(projection.len(), 1);
        assert!(projection.entries.contains_key("e.one"));
        assert!(projection.superseded.is_empty());
    }

    #[test]
    fn replay_promoted_updates_authority_only() {
        // Admit at floor, then promote to Provisional. The review axis must be
        // untouched (the orthogonality NFR).
        let events = [
            admitted(1, "e.one"),
            promoted(2, "e.one", AuthorityLevel::Raw, AuthorityLevel::Provisional),
        ];
        let projection = replay::<MemoryDomain>(events);
        let entry = &projection.entries["e.one"];
        assert_eq!(entry.authority_level, Some(AuthorityLevel::Provisional));
        // Review fields remain at the admission floor (None).
        assert_eq!(entry.review_state, None);
        assert_eq!(entry.reviewed_by, None);
        assert_eq!(projection.sequence, 2);
    }

    #[test]
    fn replay_promote_unknown_entry_emits_diagnostic_without_error() {
        let events = [promoted(
            1,
            "e.missing",
            AuthorityLevel::Raw,
            AuthorityLevel::Authority,
        )];
        let projection = replay::<MemoryDomain>(events);
        assert!(projection.entries.is_empty());
        assert_eq!(projection.diagnostics.len(), 1);
        assert_eq!(projection.diagnostics[0].code, "promote_target_missing");
        // Sequence still advances (the event was observed).
        assert_eq!(projection.sequence, 1);
    }

    #[test]
    fn replay_forgotten_removes_entry_and_marks_superseded() {
        let events = [admitted(1, "e.one"), forgotten(2, "e.one")];
        let projection = replay::<MemoryDomain>(events);
        assert!(projection.entries.is_empty());
        assert!(projection.superseded.contains("e.one"));
        assert_eq!(projection.sequence, 2);
    }

    #[test]
    fn replay_ignores_out_of_order_event_without_regressing() {
        // sequence 5 applied, then a stray sequence 2 must NOT regress state.
        let first = admitted(5, "e.five");
        let mut projection = replay::<MemoryDomain>([first]);
        projection.apply_event(&admitted(2, "e.two"));
        assert_eq!(projection.sequence, 5, "sequence must not regress");
        assert!(
            !projection.entries.contains_key("e.two"),
            "stale event must not insert"
        );
        assert!(projection
            .diagnostics
            .iter()
            .any(|d| d.code == "out_of_order_event_ignored"));
    }

    #[test]
    fn replay_last_event_wins_on_redundant_admit() {
        // Two admits for the same id with different sequences — the later one
        // (higher sequence) wins, matching the claim_wal last-record-wins rule.
        let mut first = sample_entry("e.dup");
        first.content = "first".into();
        let mut second = sample_entry("e.dup");
        second.content = "second".into();
        let events = [
            MemoryEvent::Admitted {
                sequence: 1,
                at_unix: 1,
                entry: first,
            },
            MemoryEvent::Admitted {
                sequence: 2,
                at_unix: 2,
                entry: second,
            },
        ];
        let projection = replay::<MemoryDomain>(events);
        assert_eq!(projection.entries["e.dup"].content, "second");
    }

    #[test]
    fn content_hash_is_sha256_prefixed_and_stable() {
        let entry = sample_entry("e.hash");
        let hash = MemoryEvent::content_hash_of(&entry);
        assert!(
            hash.starts_with("sha256:"),
            "hash must be sha256-prefixed, got: {hash}"
        );
        assert_eq!(
            hash.len(),
            "sha256:".len() + 64,
            "sha256 hex digest is 64 chars"
        );
        // Deterministic: same entry ⇒ same hash.
        assert_eq!(hash, MemoryEvent::content_hash_of(&entry));
        // Different entry ⇒ different hash.
        let other = sample_entry("e.other");
        assert_ne!(hash, MemoryEvent::content_hash_of(&other));
    }

    #[test]
    fn event_accessors_return_sequence_and_at_unix() {
        let event = admitted(7, "e.seven");
        assert_eq!(event.sequence(), 7);
        assert_eq!(event.at_unix(), 1_700_000_007);
    }

    #[test]
    fn next_sequence_starts_at_one_and_saturates() {
        let empty = MemoryProjection::default();
        assert_eq!(next_sequence::<MemoryDomain>(&empty), 1);
        let high = MemoryProjection {
            sequence: u64::MAX,
            ..MemoryProjection::default()
        };
        assert_eq!(
            next_sequence::<MemoryDomain>(&high),
            u64::MAX,
            "saturating add must not overflow"
        );
    }

    // --- proptest: the Fowler replay-determinism guarantee ---
    //
    // The defining property of an event-sourced projection (Fowler, "discard
    // and rebuild"): replaying the same event stream twice yields the SAME
    // projection, regardless of stream length or content. This is what makes
    // the projection disposable/rebuildable and the log the source of truth.
    #[cfg(test)]
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        fn arb_entry_id() -> impl Strategy<Value = String> {
            "[a-z]{1,3}\\.entry\\.[0-9]{1,3}"
        }

        fn arb_entry(id: String) -> MemoryEntry {
            MemoryEntry {
                entry_id: StableId(id),
                kind: MemoryKind::Preference,
                content: "c".into(),
                provenance: forge_core_contracts::MemoryProvenance {
                    source_run_id: Some(StableId("run.1".into())),
                    source_agent: Some(StableId("agent.1".into())),
                    evidence_ref: Some("e".into()),
                    captured_at: "1".into(),
                },
                freshness: forge_core_contracts::Freshness {
                    ttl_seconds: None,
                    last_confirmed_at: "1".into(),
                    stale: false,
                },
                confidence: 50,
                approval: ApprovalState::Proposed,
                supersedes: None,
                invalidation_reason: None,
                authority_level: None,
                review_state: None,
                reviewed_by: None,
                reviewed_at: None,
            }
        }

        fn arb_event(seq: u64, id: String) -> MemoryEvent {
            // Three event shapes, picked by seq % 3 to keep it deterministic.
            match seq % 3 {
                0 => MemoryEvent::Admitted {
                    sequence: seq,
                    at_unix: seq,
                    entry: arb_entry(id),
                },
                1 => MemoryEvent::Promoted {
                    sequence: seq,
                    at_unix: seq,
                    entry_id: StableId(id),
                    before: AuthorityLevel::Raw,
                    after: AuthorityLevel::Provisional,
                    evidence_refs: vec!["r".into()],
                },
                _ => MemoryEvent::Forgotten {
                    sequence: seq,
                    at_unix: seq,
                    before: arb_entry(id),
                    content_hash: "sha256:0".into(),
                },
            }
        }

        proptest! {
            /// Replay determinism: the same event stream ⇒ the same projection.
            #[test]
            fn replay_is_deterministic(events in proptest::collection::vec(arb_entry_id(), 0..20)) {
                let stream: Vec<MemoryEvent> = events
                    .iter()
                    .enumerate()
                    .map(|(i, id)| arb_event((i as u64) + 1, id.clone()))
                    .collect();
                let first = replay::<MemoryDomain>(stream.iter().cloned());
                let second = replay::<MemoryDomain>(stream.iter().cloned());
                prop_assert_eq!(&first, &second, "replay must be deterministic");
                prop_assert_eq!(first.sequence, stream.len() as u64);
            }

            /// Replay advances sequence monotonically for a monotonic input
            /// stream. Diagnostics are NOT asserted empty here: a promote of a
            /// never-admitted id correctly emits `promote_target_missing` (the
            /// generator picks event shapes by `seq % 3` independent of prior
            /// admits), so diagnostics are expected and correct behaviour.
            #[test]
            fn replay_advances_sequence_monotonically(events in proptest::collection::vec(arb_entry_id(), 1..20)) {
                let stream: Vec<MemoryEvent> = events
                    .iter()
                    .enumerate()
                    .map(|(i, id)| arb_event((i as u64) + 1, id.clone()))
                    .collect();
                let stream_len = stream.len();
                let projection = replay::<MemoryDomain>(stream.into_iter());
                // Sequence always advances to the last event's sequence (the
                // out-of-order guard never fires for a strictly monotonic stream).
                prop_assert_eq!(projection.sequence, u64::try_from(stream_len).unwrap());
            }
        }
    }
}
