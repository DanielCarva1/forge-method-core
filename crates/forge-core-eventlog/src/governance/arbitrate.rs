//! Arbitrate — the PEP that moves a conflict from `Pending` to `Resolved`.
//!
//! [`arbitrate`] acquires the exclusive governance lock, reads the current
//! projection (to locate the conflict and verify it is `Pending`), calls the
//! pure [`GovernancePolicy::can_arbitrate`](forge_core_contracts::GovernancePolicy::can_arbitrate)
//! PDP to authorize the arbiter, and — only if authorized AND the conflict is
//! `Pending` — appends a [`Resolved`](crate::GovernanceEvent::Resolved) event
//! under the same lock. The lock is held across decide-and-write, closing the
//! TOCTOU window (CWE-367). A denied arbitration appends **nothing** and is
//! reported as [`ArbitrateStatus::DeniedByGate`], not as an error.
//!
//! This is the F07 resolution lifecycle: an authorized arbiter (per
//! `GovernancePolicy::authorized_reviewers`) decides a pending conflict. The
//! decision itself is a [`ResolutionDecision`] — `AwardedTo(principal)`,
//! `BothReleased`, or `SplitScope` — recorded in the event for auditability.

use std::path::Path;

use crate::{
    now_unix,
    tcb::{next_sequence, StreamId, StreamTxn},
};
use forge_core_contracts::{
    ConflictResolutionState, GovernancePolicy, PrincipalId, ResolutionDecision, StableId,
};
use forge_core_store::WalDurability;

use super::{ArbitrateError, ArbitrationProjectionDiagnostic, GovernanceDomain, GovernanceEvent};

/// The outcome status of an [`arbitrate`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArbitrateStatus {
    /// The conflict was resolved; the `Resolved` event was appended with this
    /// sequence number.
    Resolved { sequence: u64 },
    /// The arbiter is not authorized by the governance policy
    /// (`can_arbitrate` returned `false`). Nothing appended.
    DeniedByGate,
    /// The conflict id is not in the ledger (never recorded, or recorded under
    /// a different id). Nothing appended.
    ConflictNotFound,
    /// The conflict is not in the `Pending` state (it was already resolved or
    /// escalated). A double-resolve is barred. Nothing appended.
    NotPending,
    /// A storage error prevented the arbitration.
    StoreError(ArbitrateError),
}

/// The full result of an [`arbitrate`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArbitrateResult {
    pub status: ArbitrateStatus,
    pub conflict_id: StableId,
}

impl ArbitrateResult {
    /// Convenience: was the conflict newly resolved?
    #[must_use]
    pub fn is_resolved(&self) -> bool {
        matches!(self.status, ArbitrateStatus::Resolved { .. })
    }
}

/// Resolve `conflict_id` by `arbiter` with `decision`, gated by `policy`.
/// Durability defaults to [`WalDurability::SyncOnAppend`].
pub fn arbitrate(
    root: impl AsRef<Path>,
    conflict_id: StableId,
    arbiter: &PrincipalId,
    decision: ResolutionDecision,
    policy: &GovernancePolicy,
) -> ArbitrateResult {
    arbitrate_with_durability(
        root,
        conflict_id,
        arbiter,
        decision,
        policy,
        WalDurability::default(),
    )
}

/// As [`arbitrate`] with an explicit durability knob.
#[allow(clippy::needless_pass_by_value)]
pub fn arbitrate_with_durability(
    root: impl AsRef<Path>,
    conflict_id: StableId,
    arbiter: &PrincipalId,
    decision: ResolutionDecision,
    policy: &GovernancePolicy,
    durability: WalDurability,
) -> ArbitrateResult {
    let root = root.as_ref();

    // 1. Pure PDP — authorize the arbiter BEFORE taking the lock. The decision
    //    is a pure function of (policy, arbiter), deterministic and replayable.
    if !policy.can_arbitrate(arbiter) {
        return ArbitrateResult {
            status: ArbitrateStatus::DeniedByGate,
            conflict_id,
        };
    }

    // 2. Acquire the exclusive lock for the whole read-then-write section.
    let mut transaction =
        match StreamTxn::begin::<ArbitrationProjectionDiagnostic>(root, StreamId::Governance) {
            Ok(transaction) => transaction,
            Err(source) => {
                return ArbitrateResult {
                    status: ArbitrateStatus::StoreError(source),
                    conflict_id,
                };
            }
        };

    // 3. Read the projection (under the lock) to locate the conflict.
    let projection = match transaction.project::<GovernanceDomain>() {
        Ok(projection) => projection,
        Err(source) => {
            return ArbitrateResult {
                status: ArbitrateStatus::StoreError(source),
                conflict_id,
            };
        }
    };
    let Some(conflict) = projection.conflicts.get(&conflict_id.0) else {
        return ArbitrateResult {
            status: ArbitrateStatus::ConflictNotFound,
            conflict_id,
        };
    };
    if !matches!(conflict.resolution, ConflictResolutionState::Pending) {
        return ArbitrateResult {
            status: ArbitrateStatus::NotPending,
            conflict_id,
        };
    }

    let sequence = match next_sequence::<GovernanceDomain>(&projection) {
        Ok(sequence) => sequence,
        Err(source) => {
            return ArbitrateResult {
                status: ArbitrateStatus::StoreError(source),
                conflict_id,
            };
        }
    };

    // 4. Append the Resolved event. The serialize→Value→append shim lives in
    //    forge_core_eventlog::append_event; the store's internal per-path lock
    //    handles torn-write safety.
    let event = GovernanceEvent::Resolved {
        sequence,
        at_unix: now_unix(),
        conflict_id: conflict_id.clone(),
        arbiter: arbiter.clone(),
        decision,
    };
    match transaction.append::<GovernanceDomain>(&event, durability) {
        Ok(()) => ArbitrateResult {
            status: ArbitrateStatus::Resolved { sequence },
            conflict_id,
        },
        Err(err) => ArbitrateResult {
            status: ArbitrateStatus::StoreError(err),
            conflict_id,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::super::{project, record};
    use super::*;
    use forge_core_contracts::{
        ConflictContract, ConflictDetectionReason, ConflictPolicy, IntentScope, IntentScopeKind,
    };
    use std::fs;
    use std::path::PathBuf;

    fn temp_root(label: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!("forge-governance-arb-{label}-{pid}-{nanos}"));
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

    fn policy_with_arbiter(arbiter: &str) -> GovernancePolicy {
        GovernancePolicy {
            policy_id: StableId("governance.policy.test".into()),
            permitted_principals: vec![PrincipalId("principal.alice".into())],
            authorized_reviewers: vec![PrincipalId(arbiter.into())],
            conflict_policy: ConflictPolicy::EmitContract,
        }
    }

    #[test]
    fn arbitrate_authorized_transitions_pending_to_resolved() {
        let root = temp_root("arb-allowed");
        record(&root, sample_conflict("conflict.1"));
        let result = arbitrate(
            &root,
            StableId("conflict.1".into()),
            &PrincipalId("principal.daniel".into()),
            ResolutionDecision::AwardedTo(PrincipalId("principal.alice".into())),
            &policy_with_arbiter("principal.daniel"),
        );
        let ArbitrateStatus::Resolved { sequence } = result.status else {
            panic!("expected Resolved, got {:?}", result.status);
        };
        assert_eq!(sequence, 2, "record=1, arbitrate=2");
        let projection = project(&root).expect("project");
        let conflict = &projection.conflicts["conflict.1"];
        let ConflictResolutionState::Resolved {
            arbiter, decision, ..
        } = &conflict.resolution
        else {
            panic!("expected Resolved, got {:?}", conflict.resolution);
        };
        assert_eq!(arbiter.0, "principal.daniel");
        assert_eq!(
            *decision,
            ResolutionDecision::AwardedTo(PrincipalId("principal.alice".into()))
        );
    }

    #[test]
    fn arbitrate_unauthorized_arbiter_is_denied_by_gate() {
        let root = temp_root("arb-denied");
        record(&root, sample_conflict("conflict.1"));
        let before_seq = project(&root).expect("project").sequence;
        let result = arbitrate(
            &root,
            StableId("conflict.1".into()),
            &PrincipalId("principal.eve".into()), // not an authorized reviewer
            ResolutionDecision::BothReleased,
            &policy_with_arbiter("principal.daniel"),
        );
        assert!(matches!(result.status, ArbitrateStatus::DeniedByGate));
        let after_seq = project(&root).expect("project").sequence;
        assert_eq!(
            before_seq, after_seq,
            "denied arbitrate must not consume a sequence"
        );
    }

    #[test]
    fn arbitrate_unknown_conflict_is_not_found() {
        let root = temp_root("arb-missing");
        record(&root, sample_conflict("conflict.1"));
        let result = arbitrate(
            &root,
            StableId("conflict.absent".into()),
            &PrincipalId("principal.daniel".into()),
            ResolutionDecision::BothReleased,
            &policy_with_arbiter("principal.daniel"),
        );
        assert!(matches!(result.status, ArbitrateStatus::ConflictNotFound));
    }

    #[test]
    fn arbitrate_already_resolved_is_not_pending() {
        // Double-resolve is barred: a second arbitrate of an already-Resolved
        // conflict is NotPending.
        let root = temp_root("arb-double");
        record(&root, sample_conflict("conflict.1"));
        let r1 = arbitrate(
            &root,
            StableId("conflict.1".into()),
            &PrincipalId("principal.daniel".into()),
            ResolutionDecision::BothReleased,
            &policy_with_arbiter("principal.daniel"),
        );
        assert!(r1.is_resolved());
        let r2 = arbitrate(
            &root,
            StableId("conflict.1".into()),
            &PrincipalId("principal.daniel".into()),
            ResolutionDecision::AwardedTo(PrincipalId("principal.alice".into())),
            &policy_with_arbiter("principal.daniel"),
        );
        assert!(matches!(r2.status, ArbitrateStatus::NotPending));
    }

    #[test]
    fn arbitrate_authorized_reviewers_empty_denies_all() {
        // Fail-closed: an empty authorized_reviewers denies everyone.
        let root = temp_root("arb-empty");
        record(&root, sample_conflict("conflict.1"));
        let policy = GovernancePolicy {
            policy_id: StableId("governance.policy.empty".into()),
            permitted_principals: vec![],
            authorized_reviewers: vec![],
            conflict_policy: ConflictPolicy::EmitContract,
        };
        let result = arbitrate(
            &root,
            StableId("conflict.1".into()),
            &PrincipalId("principal.anyone".into()),
            ResolutionDecision::BothReleased,
            &policy,
        );
        assert!(matches!(result.status, ArbitrateStatus::DeniedByGate));
    }

    #[test]
    fn arbitrate_with_durability_nosync_resolves() {
        // The explicit _with_durability entry point is never exercised by the
        // arbitrate() tests above (they use the SyncOnAppend default). Drive it
        // directly with NoSync — durability only governs fsync, so the
        // observable contract is identical: Resolved, sequence advances, and
        // the projection reflects the decision.
        let root = temp_root("arb-nosync");
        record(&root, sample_conflict("conflict.1"));
        let result = arbitrate_with_durability(
            &root,
            StableId("conflict.1".into()),
            &PrincipalId("principal.daniel".into()),
            ResolutionDecision::BothReleased,
            &policy_with_arbiter("principal.daniel"),
            WalDurability::NoSync,
        );
        let ArbitrateStatus::Resolved { sequence } = result.status else {
            panic!("expected Resolved, got {:?}", result.status);
        };
        assert_eq!(sequence, 2, "record=1, arbitrate=2");
        let conflict = &project(&root).expect("project").conflicts["conflict.1"];
        assert!(matches!(
            conflict.resolution,
            ConflictResolutionState::Resolved { .. }
        ));
    }

    #[test]
    fn arbitrate_on_regular_file_root_is_store_error() {
        // A root that is a regular file makes the lock acquire fail (ENOTDIR on
        // create_dir_all), which arbitrate_with_durability maps to
        // ArbitrateStatus::StoreError. Cheap path — no fs-fault injection.
        use std::fs;
        let parent = temp_root("arb-not-a-dir");
        let not_a_dir = parent.join("i-am-a-file");
        fs::write(&not_a_dir, b"x").expect("write file");
        let result = arbitrate(
            &not_a_dir,
            StableId("conflict.1".into()),
            &PrincipalId("principal.daniel".into()),
            ResolutionDecision::BothReleased,
            &policy_with_arbiter("principal.daniel"),
        );
        assert!(
            matches!(result.status, ArbitrateStatus::StoreError(_)),
            "expected StoreError, got {:?}",
            result.status
        );
    }
}
