//! Record — the PEP that persists a freshly-detected [`ConflictContract`] to the
//! arbitration ledger.
//!
//! [`record`] acquires the exclusive governance lock, reads the current
//! projection (to compute the next sequence number AND to enforce idempotency),
//! and appends a [`Detected`](crate::GovernanceEvent::Detected) event under the
//! same lock. The lock is held across read-and-write, closing the TOCTOU window
//! (CWE-367).
//!
//! # Idempotency
//!
//! The `conflict_id` produced by `claim_engine::build_conflict` is deterministic
//! and ordering-independent (the two principals are sorted, so alice-vs-bob and
//! bob-vs-alice yield the same id). A second `record` of a conflict whose id is
//! already in the ledger is a no-op [`AlreadyRecorded`](RecordStatus::AlreadyRecorded):
//! it appends **nothing** and consumes **no sequence number**. This is the
//! F07 NFR — conflict is a structured, deduplicated object, never a duplicated
//! emission — and mirrors the memory PEP's `AlreadyForgotten` idempotency rule.

use std::path::Path;

use forge_core_contracts::ConflictContract;
use forge_core_store::{append_json_line_with_durability, WalDurability};

use crate::{
    next_sequence, now_unix, project_locked, GovernanceEvent, RecordError,
    GOVERNANCE_LOCK_RELATIVE_PATH, GOVERNANCE_LOG_RELATIVE_PATH,
};

/// The outcome status of a [`record`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordStatus {
    /// The conflict was recorded; the `Detected` event was appended with this
    /// sequence number.
    Recorded { sequence: u64 },
    /// The conflict id was already in the ledger (a prior `record` already
    /// persisted it). A no-op: nothing appended, no sequence consumed. This is
    /// the deterministic-`conflict_id` idempotency path — two acquires of the
    /// same overlap produce the same id, and the second `record` deduplicates.
    AlreadyRecorded,
    /// A storage error prevented recording (lock, append, serialize, read).
    StoreError(RecordError),
}

/// The full result of a [`record`] call: the status plus the conflict id under
/// test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordResult {
    pub status: RecordStatus,
    pub conflict_id: forge_core_contracts::StableId,
}

impl RecordResult {
    /// Convenience: was the conflict newly recorded (vs already-known)?
    #[must_use]
    pub fn is_recorded(&self) -> bool {
        matches!(self.status, RecordStatus::Recorded { .. })
    }
}

/// Record `conflict` to `<state_root>/governance/conflicts.ndjson`. Idempotent
/// on `conflict_id`. Durability defaults to [`WalDurability::SyncOnAppend`].
pub fn record(root: impl AsRef<Path>, conflict: ConflictContract) -> RecordResult {
    record_with_durability(root, conflict, WalDurability::default())
}

/// As [`record`] with an explicit durability knob (the repo's `_with_durability`
/// convention).
#[allow(clippy::needless_pass_by_value)]
pub fn record_with_durability(
    root: impl AsRef<Path>,
    conflict: ConflictContract,
    durability: WalDurability,
) -> RecordResult {
    let root = root.as_ref();
    let conflict_id = conflict.conflict_id.clone();

    // 1. Acquire the exclusive lock for the whole read-then-write critical
    //    section. Held until this function returns (RAII _lock).
    let _lock =
        match forge_core_store::acquire_effect_store_lock(root, GOVERNANCE_LOCK_RELATIVE_PATH) {
            Ok(lock) => lock,
            Err(source) => {
                return RecordResult {
                    status: RecordStatus::StoreError(RecordError::Lock {
                        path: root.join(GOVERNANCE_LOCK_RELATIVE_PATH),
                        source: source.to_string(),
                    }),
                    conflict_id,
                };
            }
        };

    // 2. Read the current projection (under the lock) to compute the next
    //    sequence number AND to enforce idempotency on conflict_id. Two
    //    concurrent recorders cannot both miss each other because the lock
    //    serializes them.
    let projection = match project_locked(root) {
        Ok(projection) => projection,
        Err(source) => {
            return RecordResult {
                status: RecordStatus::StoreError(RecordError::Read {
                    path: root.join(GOVERNANCE_LOG_RELATIVE_PATH),
                    source: source.to_string(),
                }),
                conflict_id,
            };
        }
    };

    // Idempotency: a conflict with this id is already in the ledger. No-op.
    if projection.conflicts.contains_key(&conflict_id.0) {
        return RecordResult {
            status: RecordStatus::AlreadyRecorded,
            conflict_id,
        };
    }

    let sequence = next_sequence(&projection);

    // 3. Append the Detected event. append_json_line_with_durability takes its
    //    own per-path lock internally; our GOVERNANCE_LOCK_RELATIVE_PATH
    //    serializes the read-sequence-then-write window so the two locks
    //    compose correctly.
    let event = GovernanceEvent::Detected {
        sequence,
        at_unix: now_unix(),
        conflict,
    };
    let serialized = match serde_json::to_vec(&event) {
        Ok(serialized) => serialized,
        Err(source) => {
            return RecordResult {
                status: RecordStatus::StoreError(RecordError::Serialize {
                    source: source.to_string(),
                }),
                conflict_id,
            };
        }
    };
    match append_bytes(root, &serialized, durability) {
        Ok(()) => RecordResult {
            status: RecordStatus::Recorded { sequence },
            conflict_id,
        },
        Err(err) => RecordResult {
            status: RecordStatus::StoreError(err),
            conflict_id,
        },
    }
}

/// Append an already-serialized JSON line to the governance log under durability.
/// Re-wraps the bytes as a `serde_json::Value` and routes through the store
/// helper so the store owns all framing/path/lock conventions. See the memory
/// PEP's `append_bytes` for the same trade (correctness over micro-optimization).
fn append_bytes(
    root: &Path,
    serialized: &[u8],
    durability: WalDurability,
) -> Result<(), RecordError> {
    let value: serde_json::Value =
        serde_json::from_slice(serialized).map_err(|source| RecordError::Serialize {
            source: source.to_string(),
        })?;
    append_json_line_with_durability(root, GOVERNANCE_LOG_RELATIVE_PATH, &value, durability)
        .map_err(|source| RecordError::Append {
            path: root.join(GOVERNANCE_LOG_RELATIVE_PATH),
            source: source.to_string(),
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{project, GovernanceEvent};
    use forge_core_contracts::{
        ConflictContract, ConflictDetectionReason, ConflictResolutionState, IntentScope,
        IntentScopeKind, PrincipalId, StableId,
    };
    use std::fs;
    use std::path::PathBuf;

    fn temp_root(label: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("forge-governance-{label}-{pid}-{nanos}"));
        fs::create_dir_all(&path).expect("create temp root");
        path
    }

    fn sample_conflict(id: &str) -> ConflictContract {
        ConflictContract {
            conflict_id: StableId(id.into()),
            intent_a: StableId("intent.alice".into()),
            intent_b: StableId("intent.bob".into()),
            principal_a: PrincipalId("principal.alice".into()),
            principal_b: PrincipalId("principal.bob".into()),
            contested_scope: IntentScope {
                kind: IntentScopeKind::PathPrefix,
                target: StableId("stories".into()),
            },
            detection_reason: ConflictDetectionReason::PathOverlap,
            detected_at: 1_700_000_000,
            resolution: ConflictResolutionState::Pending,
        }
    }

    #[test]
    fn record_appends_event_and_advances_sequence() {
        let root = temp_root("record-once");
        let result = record(&root, sample_conflict("conflict.1"));
        assert!(result.is_recorded(), "{result:?}");
        let RecordStatus::Recorded { sequence } = result.status else {
            panic!("expected Recorded");
        };
        assert_eq!(sequence, 1, "first record is sequence 1");
        let projection = project(&root).expect("project after record");
        assert!(projection.conflicts.contains_key("conflict.1"));
        assert_eq!(projection.sequence, 1);
        // Detected event carries the full conflict at Pending.
        let conflict = &projection.conflicts["conflict.1"];
        assert_eq!(conflict.resolution, ConflictResolutionState::Pending);
    }

    #[test]
    fn record_same_conflict_id_is_already_recorded_noop() {
        // Idempotency: recording the same conflict_id a second time appends
        // nothing and consumes no sequence.
        let root = temp_root("record-idempotent");
        let r1 = record(&root, sample_conflict("conflict.1"));
        assert!(r1.is_recorded());
        let seq_after_first = project(&root).expect("project").sequence;
        let r2 = record(&root, sample_conflict("conflict.1"));
        assert!(matches!(r2.status, RecordStatus::AlreadyRecorded));
        let seq_after_second = project(&root).expect("project").sequence;
        assert_eq!(
            seq_after_first, seq_after_second,
            "idempotent record must not append another event"
        );
    }

    #[test]
    fn record_two_distinct_conflicts_yields_monotonic_sequence() {
        let root = temp_root("record-two");
        let r1 = record(&root, sample_conflict("conflict.1"));
        let r2 = record(&root, sample_conflict("conflict.2"));
        let RecordStatus::Recorded { sequence: s1 } = r1.status else {
            panic!();
        };
        let RecordStatus::Recorded { sequence: s2 } = r2.status else {
            panic!();
        };
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
        let projection = project(&root).expect("project");
        assert_eq!(projection.len(), 2);
        assert_eq!(projection.sequence, 2);
    }

    #[test]
    fn record_detected_event_round_trips_through_log() {
        // The Detected event written to the log must deserialize back to the
        // same event (serde tag = "kind" round-trip).
        let root = temp_root("record-roundtrip");
        record(&root, sample_conflict("conflict.1"));
        let log_bytes = fs::read(root.join(GOVERNANCE_LOG_RELATIVE_PATH)).expect("read log");
        let line = String::from_utf8(log_bytes)
            .expect("utf8")
            .trim()
            .to_string();
        let event: GovernanceEvent = serde_json::from_str(&line).expect("deserialize");
        let GovernanceEvent::Detected {
            conflict, sequence, ..
        } = event
        else {
            panic!("expected Detected, got {event:?}");
        };
        assert_eq!(sequence, 1);
        assert_eq!(conflict.conflict_id.0, "conflict.1");
    }
}
