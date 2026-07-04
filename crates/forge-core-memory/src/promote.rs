//! Promote — the PEP for `MemoryContract::can_promote`.
//!
//! [`promote`] acquires the exclusive memory lock, locates the entry in the
//! projection, calls the pure
//! [`MemoryContract::can_promote`](forge_core_contracts::MemoryContract::can_promote)
//! PDP, and — only if `Allowed` — appends a [`Promoted`](crate::MemoryEvent::Promoted)
//! event carrying `before`/`after` authority for audit. The lock is held across
//! decide-and-write (CWE-367). A denied promote appends nothing and is
//! [`PromoteStatus::DeniedByGate`].
//!
//! Authority-axis ONLY: the review fields on the entry are never touched by a
//! promote (ADR-0023 orthogonality NFR). The `before`/`after` in the event are
//! `AuthorityLevel` values; review state is unaffected.

use std::path::Path;

use forge_core_contracts::{
    AdmissionDecision, AdmissionDenialReason, AdmissionEvidence, AuthorityLevel, MemoryContract,
    MemoryPolicy, StableId,
};
use forge_core_eventlog::{append_event, next_sequence, now_unix, project_locked, EventLogLock};
use forge_core_store::WalDurability;

use crate::{
    MemoryDomain, MemoryEvent, MemoryProjectionDiagnostic, PromoteError, MEMORY_LOCK_RELATIVE_PATH,
    MEMORY_LOG_RELATIVE_PATH,
};

/// The outcome status of a [`promote`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromoteStatus {
    /// The entry's authority was promoted; the `Promoted` event was appended.
    /// `before`/`after` are the authority-axis transitions recorded in the event.
    Promoted {
        sequence: u64,
        before: AuthorityLevel,
        after: AuthorityLevel,
    },
    /// The entry to promote was not found in the store.
    NotFound,
    /// The gate blocked the promote (insufficient raw evidence). Nothing appended.
    DeniedByGate(Vec<AdmissionDenialReason>),
    /// A storage error prevented the promote.
    StoreError(PromoteError),
}

/// The full result of a [`promote`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteResult {
    pub status: PromoteStatus,
    pub entry_id: StableId,
}

impl PromoteResult {
    #[must_use]
    pub fn is_promoted(&self) -> bool {
        matches!(self.status, PromoteStatus::Promoted { .. })
    }
}

/// Promote `entry_id`'s authority using `evidence` under `policy`.
///
/// The promote is authority-axis only and never touches review state. `root`
/// is the state root. Durability defaults to `SyncOnAppend`.
pub fn promote(
    root: impl AsRef<Path>,
    entry_id: StableId,
    policy: &MemoryPolicy,
    evidence: &AdmissionEvidence,
) -> PromoteResult {
    promote_with_durability(root, entry_id, policy, evidence, WalDurability::default())
}

/// As [`promote`] with an explicit durability knob.
#[allow(clippy::needless_pass_by_value)]
pub fn promote_with_durability(
    root: impl AsRef<Path>,
    entry_id: StableId,
    policy: &MemoryPolicy,
    evidence: &AdmissionEvidence,
    durability: WalDurability,
) -> PromoteResult {
    let root = root.as_ref();

    // 1. Acquire the lock for the whole find-then-decide-then-write section.
    let lock = match EventLogLock::acquire::<MemoryProjectionDiagnostic>(
        root,
        MEMORY_LOCK_RELATIVE_PATH,
    ) {
        Ok(lock) => lock,
        Err(source) => {
            return PromoteResult {
                status: PromoteStatus::StoreError(source),
                entry_id,
            };
        }
    };

    // 2. Read the projection (under the lock) to find the entry.
    let projection = match project_locked::<MemoryDomain>(root, MEMORY_LOG_RELATIVE_PATH) {
        Ok(projection) => projection,
        Err(source) => {
            return PromoteResult {
                status: PromoteStatus::StoreError(source),
                entry_id,
            };
        }
    };
    let Some(entry) = projection.entries.get(&entry_id.0) else {
        return PromoteResult {
            status: PromoteStatus::NotFound,
            entry_id,
        };
    };

    // 3. Pure PDP. `can_promote` decides whether the offered evidence clears
    //    the threshold for Authority. The PEP does not re-evaluate thresholds.
    let decision = MemoryContract::can_promote(entry, policy, evidence);
    let AdmissionDecision::Allowed = decision else {
        let AdmissionDecision::Blocked(reasons) = decision else {
            // Unreachable: AdmissionDecision has exactly two variants.
            return PromoteResult {
                status: PromoteStatus::DeniedByGate(vec![]),
                entry_id,
            };
        };
        return PromoteResult {
            status: PromoteStatus::DeniedByGate(reasons),
            entry_id,
        };
    };

    // 4. The target authority is `Authority` (the only promote target the gate
    //    clears for). `before` is the effective authority the entry held.
    //    (A finer Raw→Provisional vs Provisional→Authority distinction is a
    //    future per-policy refinement; today `can_promote`'s Allowed means
    //    "cleared for Authority".)
    let before = entry.authority_level_effective();
    let after = AuthorityLevel::Authority;
    let sequence = next_sequence::<MemoryDomain>(&projection);

    // 5. Append the Promoted event. Only authority-axis fields are recorded;
    //    review state is deliberately absent (orthogonality NFR).
    let event = MemoryEvent::Promoted {
        sequence,
        at_unix: now_unix(),
        entry_id: entry_id.clone(),
        before,
        after,
        evidence_refs: distinct_non_empty(&evidence.evidence_refs),
    };
    match append_event::<MemoryDomain>(root, MEMORY_LOG_RELATIVE_PATH, &event, durability, &lock) {
        Ok(_) => PromoteResult {
            status: PromoteStatus::Promoted {
                sequence,
                before,
                after,
            },
            entry_id,
        },
        Err(err) => PromoteResult {
            status: PromoteStatus::StoreError(err),
            entry_id,
        },
    }
}

/// Distinct, non-empty, trimmed evidence refs — order-independent. Matches the
/// counting rule in `MemoryContract::can_promote` so the event records exactly
/// the refs that satisfied the gate.
fn distinct_non_empty(refs: &[String]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for r in refs {
        let trimmed = r.trim();
        if trimmed.is_empty() {
            continue;
        }
        let owned = trimmed.to_string();
        if !seen.contains(&owned) {
            seen.push(owned);
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{admit, project, AdmissionStatus};
    use forge_core_contracts::{
        AdmissionEvidence, ApprovalState, Freshness, MemoryEntry, MemoryKind, MemoryProvenance,
    };
    use std::fs;
    use std::path::PathBuf;

    fn temp_root(label: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!("forge-memory-promote-{label}-{pid}-{nanos}"));
        fs::create_dir_all(&path).expect("create temp root");
        path
    }

    fn policy_requiring_one_ref() -> MemoryPolicy {
        MemoryPolicy {
            permitted_kinds: vec![MemoryKind::Preference],
            required_evidence_fields: vec![],
            min_evidence_refs_for_authority: 1,
        }
    }

    fn sample_entry(id: &str) -> MemoryEntry {
        MemoryEntry {
            entry_id: StableId(id.into()),
            kind: MemoryKind::Preference,
            content: "prefer typed contracts".into(),
            provenance: MemoryProvenance {
                source_run_id: Some(StableId("run.1".into())),
                source_agent: Some(StableId("agent.1".into())),
                evidence_ref: Some("contracts/evidence.yaml".into()),
                captured_at: "1700000000".into(),
            },
            freshness: Freshness {
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

    fn admit_entry(root: &Path, id: &str) {
        let policy = policy_requiring_one_ref();
        let result = admit(root, sample_entry(id), &policy);
        assert!(
            matches!(result.status, AdmissionStatus::Admitted { .. }),
            "admit failed: {result:?}"
        );
    }

    #[test]
    fn promote_allowed_advances_authority_and_records_before_after() {
        let root = temp_root("promote-allowed");
        admit_entry(&root, "e.one");
        let result = promote(
            &root,
            StableId("e.one".into()),
            &policy_requiring_one_ref(),
            &AdmissionEvidence {
                evidence_refs: vec!["run.alpha".into()],
            },
        );
        let PromoteStatus::Promoted {
            sequence,
            before,
            after,
        } = result.status
        else {
            panic!("expected Promoted, got {:?}", result.status);
        };
        assert_eq!(sequence, 2, "admit was seq 1; promote is seq 2");
        assert_eq!(before, AuthorityLevel::Raw, "admitted entry starts at Raw");
        assert_eq!(after, AuthorityLevel::Authority);
        // Projection reflects the new authority.
        let projection = project(&root).expect("project");
        assert_eq!(
            projection.entries["e.one"].authority_level,
            Some(AuthorityLevel::Authority),
        );
    }

    #[test]
    fn promote_denied_no_evidence_appends_nothing() {
        let root = temp_root("promote-denied");
        admit_entry(&root, "e.one");
        let before_seq = project(&root).expect("project").sequence;
        let result = promote(
            &root,
            StableId("e.one".into()),
            &policy_requiring_one_ref(),
            &AdmissionEvidence {
                evidence_refs: vec![],
            },
        );
        assert!(matches!(result.status, PromoteStatus::DeniedByGate(_)));
        // No event appended: sequence unchanged.
        let after_seq = project(&root).expect("project").sequence;
        assert_eq!(
            before_seq, after_seq,
            "denied promote must not consume a sequence"
        );
        // Authority unchanged.
        let projection = project(&root).expect("project");
        assert_eq!(projection.entries["e.one"].authority_level, None);
    }

    #[test]
    fn promote_unknown_entry_is_not_found() {
        let root = temp_root("promote-missing");
        admit_entry(&root, "e.present");
        let result = promote(
            &root,
            StableId("e.absent".into()),
            &policy_requiring_one_ref(),
            &AdmissionEvidence {
                evidence_refs: vec!["run.alpha".into()],
            },
        );
        assert!(matches!(result.status, PromoteStatus::NotFound));
    }

    #[test]
    fn promote_does_not_touch_review_axis() {
        // After a promote, the review fields are still at the admission floor.
        // This is the Model-B-back-door guard (ADR-0023).
        let root = temp_root("promote-no-review");
        admit_entry(&root, "e.one");
        let _ = promote(
            &root,
            StableId("e.one".into()),
            &policy_requiring_one_ref(),
            &AdmissionEvidence {
                evidence_refs: vec!["run.alpha".into()],
            },
        );
        let projection = project(&root).expect("project");
        let entry = &projection.entries["e.one"];
        assert_eq!(
            entry.review_state, None,
            "promote must not set review_state"
        );
        assert_eq!(entry.reviewed_by, None, "promote must not set reviewed_by");
        assert_eq!(entry.reviewed_at, None, "promote must not set reviewed_at");
    }

    #[test]
    fn distinct_non_empty_trims_dedupes_and_drops_empty() {
        let refs = vec![
            "a".into(),
            " a ".into(),
            String::new(),
            "  ".into(),
            "b".into(),
        ];
        let out = distinct_non_empty(&refs);
        assert_eq!(out, vec!["a".to_string(), "b".to_string()]);
    }
}
