//! Escalate — the PEP that moves a conflict from `Pending` to `Escalated`.
//!
//! [`escalate`] acquires the exclusive governance lock, reads the current
//! projection (to locate the conflict and verify it is `Pending`), calls the
//! pure [`GovernancePolicy::can_arbitrate`](forge_core_contracts::GovernancePolicy::can_arbitrate)
//! PDP to authorize the escallating principal, and — only if authorized AND the
//! conflict is `Pending` — appends an [`Escalated`](crate::GovernanceEvent::Escalated)
//! event under the same lock.
//!
//! Escalation is the path for a conflict that has no in-system resolution (e.g.
//! no `AwardedTo`/`BothReleased`/`SplitScope` decision is acceptable to an
//! authorized arbiter) and must be routed to a human/external arbiter. It uses
//! the same gate as `arbitrate` — escalating is itself an arbitration act.

use std::path::Path;

use forge_core_contracts::{ConflictResolutionState, GovernancePolicy, PrincipalId, StableId};
use forge_core_store::{append_json_line_with_durability, WalDurability};

use crate::{
    next_sequence, now_unix, project_locked, EscalateError, GovernanceEvent,
    GOVERNANCE_LOCK_RELATIVE_PATH, GOVERNANCE_LOG_RELATIVE_PATH,
};

/// The outcome status of an [`escalate`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscalateStatus {
    /// The conflict was escalated; the `Escalated` event was appended with this
    /// sequence number.
    Escalated { sequence: u64 },
    /// The escallating principal is not authorized by the governance policy.
    /// Nothing appended.
    DeniedByGate,
    /// The conflict id is not in the ledger.
    ConflictNotFound,
    /// The conflict is not in the `Pending` state.
    NotPending,
    /// A storage error prevented the escalation.
    StoreError(EscalateError),
}

/// The full result of an [`escalate`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalateResult {
    pub status: EscalateStatus,
    pub conflict_id: StableId,
}

impl EscalateResult {
    /// Convenience: was the conflict newly escalated?
    #[must_use]
    pub fn is_escalated(&self) -> bool {
        matches!(self.status, EscalateStatus::Escalated { .. })
    }
}

/// Escalate `conflict_id` by `principal`, gated by `policy`. Durability defaults
/// to [`WalDurability::SyncOnAppend`].
pub fn escalate(
    root: impl AsRef<Path>,
    conflict_id: StableId,
    principal: &PrincipalId,
    policy: &GovernancePolicy,
) -> EscalateResult {
    escalate_with_durability(
        root,
        conflict_id,
        principal,
        policy,
        WalDurability::default(),
    )
}

/// As [`escalate`] with an explicit durability knob.
#[allow(clippy::needless_pass_by_value)]
pub fn escalate_with_durability(
    root: impl AsRef<Path>,
    conflict_id: StableId,
    principal: &PrincipalId,
    policy: &GovernancePolicy,
    durability: WalDurability,
) -> EscalateResult {
    let root = root.as_ref();

    // 1. Pure PDP — authorize the principal BEFORE taking the lock.
    if !policy.can_arbitrate(principal) {
        return EscalateResult {
            status: EscalateStatus::DeniedByGate,
            conflict_id,
        };
    }

    // 2. Acquire the exclusive lock.
    let _lock =
        match forge_core_store::acquire_effect_store_lock(root, GOVERNANCE_LOCK_RELATIVE_PATH) {
            Ok(lock) => lock,
            Err(source) => {
                return EscalateResult {
                    status: EscalateStatus::StoreError(EscalateError::Lock {
                        path: root.join(GOVERNANCE_LOCK_RELATIVE_PATH),
                        source: source.to_string(),
                    }),
                    conflict_id,
                };
            }
        };

    // 3. Read the projection (under the lock).
    let projection = match project_locked(root) {
        Ok(projection) => projection,
        Err(source) => {
            return EscalateResult {
                status: EscalateStatus::StoreError(EscalateError::Read {
                    path: root.join(GOVERNANCE_LOG_RELATIVE_PATH),
                    source: source.to_string(),
                }),
                conflict_id,
            };
        }
    };
    let Some(conflict) = projection.conflicts.get(&conflict_id.0) else {
        return EscalateResult {
            status: EscalateStatus::ConflictNotFound,
            conflict_id,
        };
    };
    if !matches!(conflict.resolution, ConflictResolutionState::Pending) {
        return EscalateResult {
            status: EscalateStatus::NotPending,
            conflict_id,
        };
    }

    let sequence = next_sequence(&projection);

    // 4. Append the Escalated event.
    let event = GovernanceEvent::Escalated {
        sequence,
        at_unix: now_unix(),
        conflict_id: conflict_id.clone(),
    };
    let serialized = match serde_json::to_vec(&event) {
        Ok(serialized) => serialized,
        Err(source) => {
            return EscalateResult {
                status: EscalateStatus::StoreError(EscalateError::Serialize {
                    source: source.to_string(),
                }),
                conflict_id,
            };
        }
    };
    match append_bytes(root, &serialized, durability) {
        Ok(()) => EscalateResult {
            status: EscalateStatus::Escalated { sequence },
            conflict_id,
        },
        Err(err) => EscalateResult {
            status: EscalateStatus::StoreError(err),
            conflict_id,
        },
    }
}

fn append_bytes(
    root: &Path,
    serialized: &[u8],
    durability: WalDurability,
) -> Result<(), EscalateError> {
    let value: serde_json::Value =
        serde_json::from_slice(serialized).map_err(|source| EscalateError::Serialize {
            source: source.to_string(),
        })?;
    append_json_line_with_durability(root, GOVERNANCE_LOG_RELATIVE_PATH, &value, durability)
        .map_err(|source| EscalateError::Append {
            path: root.join(GOVERNANCE_LOG_RELATIVE_PATH),
            source: source.to_string(),
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{project, record};
    use forge_core_contracts::{
        ConflictContract, ConflictDetectionReason, ConflictPolicy, IntentScope, IntentScopeKind,
    };
    use std::fs;
    use std::path::PathBuf;

    fn temp_root(label: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("forge-governance-esc-{label}-{pid}-{nanos}"));
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
    fn escalate_authorized_transitions_pending_to_escalated() {
        let root = temp_root("esc-allowed");
        record(&root, sample_conflict("conflict.1"));
        let result = escalate(
            &root,
            StableId("conflict.1".into()),
            &PrincipalId("principal.daniel".into()),
            &policy_with_arbiter("principal.daniel"),
        );
        let EscalateStatus::Escalated { sequence } = result.status else {
            panic!("expected Escalated, got {:?}", result.status);
        };
        assert_eq!(sequence, 2);
        let projection = project(&root).expect("project");
        assert_eq!(
            projection.conflicts["conflict.1"].resolution,
            ConflictResolutionState::Escalated
        );
    }

    #[test]
    fn escalate_unauthorized_is_denied_by_gate() {
        let root = temp_root("esc-denied");
        record(&root, sample_conflict("conflict.1"));
        let before_seq = project(&root).expect("project").sequence;
        let result = escalate(
            &root,
            StableId("conflict.1".into()),
            &PrincipalId("principal.eve".into()),
            &policy_with_arbiter("principal.daniel"),
        );
        assert!(matches!(result.status, EscalateStatus::DeniedByGate));
        let after_seq = project(&root).expect("project").sequence;
        assert_eq!(before_seq, after_seq);
    }

    #[test]
    fn escalate_already_escalated_is_not_pending() {
        let root = temp_root("esc-double");
        record(&root, sample_conflict("conflict.1"));
        let r1 = escalate(
            &root,
            StableId("conflict.1".into()),
            &PrincipalId("principal.daniel".into()),
            &policy_with_arbiter("principal.daniel"),
        );
        assert!(r1.is_escalated());
        let r2 = escalate(
            &root,
            StableId("conflict.1".into()),
            &PrincipalId("principal.daniel".into()),
            &policy_with_arbiter("principal.daniel"),
        );
        assert!(matches!(r2.status, EscalateStatus::NotPending));
    }
}
