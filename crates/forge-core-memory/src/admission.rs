//! Admission — the PEP for `MemoryContract::can_admit`.
//!
//! [`admit`] acquires the exclusive memory lock, reads the current projection
//! (to compute the next sequence number), calls the pure
//! [`MemoryContract::can_admit`](forge_core_contracts::MemoryContract::can_admit)
//! PDP, and — only if `Allowed` — appends an [`Admitted`](crate::MemoryEvent::Admitted)
//! event under the same lock. The lock is held across decide-and-write, closing
//! the TOCTOU window (CWE-367). A denied decision appends **nothing** and is
//! reported as [`AdmissionStatus::DeniedByGate`], not as an error (the PEP
//! enforces; it does not re-evaluate policy — Cedar/OPA/XACML).

use std::path::Path;

use forge_core_contracts::{MemoryContract, MemoryEntry, MemoryPolicy};
use forge_core_eventlog::{append_event, next_sequence, now_unix, project_locked, EventLogLock};
use forge_core_store::WalDurability;

use crate::{
    AdmissionDenialReason, AdmitError, MemoryDomain, MemoryEvent, MemoryProjectionDiagnostic,
    MEMORY_LOCK_RELATIVE_PATH, MEMORY_LOG_RELATIVE_PATH,
};

/// The outcome status of an [`admit`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionStatus {
    /// The entry was admitted at the trust floor (`Raw`, `Unreviewed`); the
    /// `Admitted` event was appended with this sequence number.
    Admitted { sequence: u64 },
    /// The gate blocked the entry. No event was appended. The reasons come
    /// straight from the pure PDP.
    DeniedByGate(Vec<AdmissionDenialReason>),
    /// A storage error prevented admission (lock, append, serialize, read).
    StoreError(AdmitError),
}

/// The full result of an [`admit`] call: the status plus the entry id under test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionResult {
    pub status: AdmissionStatus,
    pub entry_id: forge_core_contracts::StableId,
}

impl AdmissionResult {
    /// Convenience: was the entry admitted?
    #[must_use]
    pub fn is_admitted(&self) -> bool {
        matches!(self.status, AdmissionStatus::Admitted { .. })
    }
}

/// Admit `entry` under `policy`, writing to `<state_root>/memory/events.ndjson`.
///
/// `root` is the state root. Durability defaults to
/// [`WalDurability::SyncOnAppend`] (production); tests pass
/// [`WalDurability::NoSync`].
pub fn admit(root: impl AsRef<Path>, entry: MemoryEntry, policy: &MemoryPolicy) -> AdmissionResult {
    admit_with_durability(root, entry, policy, WalDurability::default())
}

/// As [`admit`] with an explicit durability knob (the repo's `_with_durability`
/// convention).
#[allow(clippy::needless_pass_by_value)]
pub fn admit_with_durability(
    root: impl AsRef<Path>,
    entry: MemoryEntry,
    policy: &MemoryPolicy,
    durability: WalDurability,
) -> AdmissionResult {
    let root = root.as_ref();
    let entry_id = entry.entry_id.clone();

    // 1. Pure PDP. Decide BEFORE taking the lock — the decision is a pure
    //    function of (entry, policy) and does not depend on store state.
    //    (Cedar/OPA: the decision is deterministic, replayable, side-effect-free.)
    match MemoryContract::can_admit(&entry, policy) {
        forge_core_contracts::AdmissionDecision::Allowed => {}
        forge_core_contracts::AdmissionDecision::Blocked(reasons) => {
            return AdmissionResult {
                status: AdmissionStatus::DeniedByGate(reasons),
                entry_id,
            };
        }
    }

    // 2. Acquire the exclusive lock for the whole read-sequence-then-write
    //    critical section. Held until this function returns (RAII lock).
    let lock = match EventLogLock::acquire::<MemoryProjectionDiagnostic>(
        root,
        MEMORY_LOCK_RELATIVE_PATH,
    ) {
        Ok(lock) => lock,
        Err(source) => {
            return AdmissionResult {
                status: AdmissionStatus::StoreError(source),
                entry_id,
            };
        }
    };

    // 3. Read the current projection (under the lock) to compute the next
    //    sequence number. Two concurrent admitters cannot both see seq=N
    //    because the lock serializes them.
    let projection = match project_locked::<MemoryDomain>(root, MEMORY_LOG_RELATIVE_PATH) {
        Ok(projection) => projection,
        Err(source) => {
            return AdmissionResult {
                status: AdmissionStatus::StoreError(source),
                entry_id,
            };
        }
    };
    let sequence = next_sequence::<MemoryDomain>(&projection);

    // 4. Append the event. The serialize→Value→append shim lives in
    //    forge_core_eventlog::append_event; the store's internal per-path lock
    //    handles torn-write safety, and our MEMORY_LOCK_RELATIVE_PATH serializes
    //    the read-sequence-then-write window so the two compose.
    let event = MemoryEvent::Admitted {
        sequence,
        at_unix: now_unix(),
        entry,
    };
    match append_event::<MemoryDomain>(root, MEMORY_LOG_RELATIVE_PATH, &event, durability, &lock) {
        Ok(_) => AdmissionResult {
            status: AdmissionStatus::Admitted { sequence },
            entry_id,
        },
        Err(err) => AdmissionResult {
            status: AdmissionStatus::StoreError(err),
            entry_id,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project;
    use forge_core_contracts::{ApprovalState, MemoryKind};
    use std::fs;
    use std::path::PathBuf;

    /// Hand-rolled temp dir (repo convention: no `tempfile` workspace dep).
    fn temp_root(label: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("forge-memory-{label}-{pid}-{nanos}"));
        fs::create_dir_all(&path).expect("create temp root");
        path
    }

    fn permissive_policy() -> MemoryPolicy {
        MemoryPolicy {
            permitted_kinds: vec![MemoryKind::Preference, MemoryKind::Decision],
            required_evidence_fields: vec![],
            min_evidence_refs_for_authority: 1,
        }
    }

    fn deny_all_policy() -> MemoryPolicy {
        MemoryPolicy {
            permitted_kinds: vec![],
            required_evidence_fields: vec![],
            min_evidence_refs_for_authority: 1,
        }
    }

    fn sample_entry(id: &str) -> MemoryEntry {
        MemoryEntry {
            entry_id: forge_core_contracts::StableId(id.into()),
            kind: MemoryKind::Preference,
            content: "prefer typed contracts".into(),
            provenance: forge_core_contracts::MemoryProvenance {
                source_run_id: Some(forge_core_contracts::StableId("run.1".into())),
                source_agent: Some(forge_core_contracts::StableId("agent.1".into())),
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

    #[test]
    fn admit_allowed_appends_event_and_advances_sequence() {
        let root = temp_root("admit-allowed");
        let result = admit(&root, sample_entry("e.one"), &permissive_policy());
        assert!(result.is_admitted(), "{result:?}");
        let AdmissionStatus::Admitted { sequence } = result.status else {
            panic!("expected Admitted");
        };
        assert_eq!(sequence, 1, "first admit is sequence 1");
        // Projection now contains the entry.
        let projection = project(&root).expect("project after admit");
        assert!(projection.entries.contains_key("e.one"));
        assert_eq!(projection.sequence, 1);
    }

    #[test]
    fn admit_denied_by_gate_appends_nothing() {
        let root = temp_root("admit-denied");
        let result = admit(&root, sample_entry("e.one"), &deny_all_policy());
        assert!(!result.is_admitted());
        assert!(matches!(result.status, AdmissionStatus::DeniedByGate(_)));
        // No event was appended — the log does not exist.
        assert!(!root.join(MEMORY_LOG_RELATIVE_PATH).exists());
        let projection = project(&root).expect("project is empty");
        assert!(projection.is_empty());
    }

    #[test]
    fn admit_denied_does_not_advance_sequence_for_later_admit() {
        // A denied admit appends nothing; the next allowed admit is still seq 1.
        let root = temp_root("admit-denied-then-allowed");
        let denied = admit(&root, sample_entry("e.denied"), &deny_all_policy());
        assert!(!denied.is_admitted());
        let allowed = admit(&root, sample_entry("e.ok"), &permissive_policy());
        let AdmissionStatus::Admitted { sequence } = allowed.status else {
            panic!("expected Admitted, got {:?}", allowed.status);
        };
        assert_eq!(sequence, 1, "denial must not consume a sequence number");
    }

    #[test]
    fn admit_two_entries_yields_monotonic_sequence() {
        let root = temp_root("admit-two");
        let r1 = admit(&root, sample_entry("e.one"), &permissive_policy());
        let r2 = admit(&root, sample_entry("e.two"), &permissive_policy());
        let AdmissionStatus::Admitted { sequence: s1 } = r1.status else {
            panic!();
        };
        let AdmissionStatus::Admitted { sequence: s2 } = r2.status else {
            panic!();
        };
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
        let projection = project(&root).expect("project");
        assert_eq!(projection.len(), 2);
        assert_eq!(projection.sequence, 2);
    }

    #[test]
    fn admit_admitted_entry_lands_at_trust_floor() {
        // The NFR: admitting never raises authority above Raw or review above
        // Unreviewed. The entry was authored with authority_level=None (floor),
        // and the PEP does not promote it.
        let root = temp_root("admit-floor");
        admit(&root, sample_entry("e.floor"), &permissive_policy());
        let projection = project(&root).expect("project");
        let entry = &projection.entries["e.floor"];
        assert_eq!(
            entry.authority_level_effective(),
            forge_core_contracts::AuthorityLevel::Raw
        );
        assert_eq!(
            entry.review_state_effective(),
            forge_core_contracts::ReviewState::Unreviewed
        );
    }
}
