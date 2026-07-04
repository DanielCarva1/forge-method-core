//! Admission — the PEP for `ResearchContract::can_admit_source`.
//!
//! [`admit_source`] acquires the exclusive research lock, reads the current
//! projection (to compute the next sequence number), calls the pure
//! [`ResearchContract::can_admit_source`](forge_core_contracts::ResearchContract::can_admit_source)
//! PDP, and — only if `Allowed` — appends a [`SourceAdded`](crate::ResearchEvent::SourceAdded)
//! event under the same lock. The lock is held across decide-and-write, closing
//! the TOCTOU window (CWE-367). A denied decision appends **nothing** and is
//! reported as [`AdmissionStatus::DeniedByGate`], not as an error (the PEP
//! enforces; it does not re-evaluate policy — Cedar/OPA/XACML).
//!
//! This module is the F14 twin of `forge-core-memory::admission`; the storage
//! mechanics (lock + append + sequence) are identical, only the event shape and
//! the PDP differ (ADR-0010).

use std::path::Path;

use forge_core_contracts::{
    ResearchAdmissionDecision, ResearchAdmissionDenialReason, ResearchContract, ResearchPolicy,
    ResearchSource, SourceId,
};
use forge_core_eventlog::{append_event, next_sequence, now_unix, project_locked, EventLogLock};
use forge_core_store::WalDurability;

use crate::{
    ResearchAdmitError, ResearchDomain, ResearchEvent, ResearchProjectionDiagnostic,
    RESEARCH_LOCK_RELATIVE_PATH, RESEARCH_LOG_RELATIVE_PATH,
};

/// The outcome status of an [`admit_source`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionStatus {
    /// The source was admitted; the `SourceAdded` event was appended with this
    /// sequence number.
    Admitted { sequence: u64 },
    /// The gate blocked the source. No event was appended. The reasons come
    /// straight from the pure PDP.
    DeniedByGate(Vec<ResearchAdmissionDenialReason>),
    /// A storage error prevented admission (lock, append, serialize, read).
    StoreError(ResearchAdmitError),
}

/// The full result of an [`admit_source`] call: the status plus the source id
/// under test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionResult {
    pub status: AdmissionStatus,
    pub source_id: SourceId,
}

impl AdmissionResult {
    /// Convenience: was the source admitted?
    #[must_use]
    pub fn is_admitted(&self) -> bool {
        matches!(self.status, AdmissionStatus::Admitted { .. })
    }
}

/// Admit `source` under `policy`, writing to
/// `<state_root>/research/sources.ndjson`.
///
/// `root` is the state root. Durability defaults to
/// [`WalDurability::SyncOnAppend`] (production); tests pass
/// [`WalDurability::NoSync`].
pub fn admit_source(
    root: impl AsRef<Path>,
    source: ResearchSource,
    policy: &ResearchPolicy,
) -> AdmissionResult {
    admit_source_with_durability(root, source, policy, WalDurability::default())
}

/// As [`admit_source`] with an explicit durability knob (the repo's
/// `_with_durability` convention).
#[allow(clippy::needless_pass_by_value)]
pub fn admit_source_with_durability(
    root: impl AsRef<Path>,
    source: ResearchSource,
    policy: &ResearchPolicy,
    durability: WalDurability,
) -> AdmissionResult {
    let root = root.as_ref();
    let source_id = source.id.clone();

    // 1. Pure PDP. Decide BEFORE taking the lock — the decision is a pure
    //    function of (source, policy) and does not depend on store state.
    //    (Cedar/OPA: the decision is deterministic, replayable, side-effect-free.)
    let ResearchAdmissionDecision::Allowed = ResearchContract::can_admit_source(&source, policy)
    else {
        let ResearchAdmissionDecision::Blocked(reasons) =
            ResearchContract::can_admit_source(&source, policy)
        else {
            // Unreachable: AdmissionDecision has exactly two variants.
            return AdmissionResult {
                status: AdmissionStatus::DeniedByGate(vec![]),
                source_id,
            };
        };
        return AdmissionResult {
            status: AdmissionStatus::DeniedByGate(reasons),
            source_id,
        };
    };

    // 2. Acquire the exclusive lock for the whole read-sequence-then-write
    //    critical section. Held until this function returns (RAII lock).
    let lock = match EventLogLock::acquire::<ResearchProjectionDiagnostic>(
        root,
        RESEARCH_LOCK_RELATIVE_PATH,
    ) {
        Ok(lock) => lock,
        Err(source) => {
            return AdmissionResult {
                status: AdmissionStatus::StoreError(source),
                source_id,
            };
        }
    };

    // 3. Read the current projection (under the lock) to compute the next
    //    sequence number. Two concurrent admitters cannot both see seq=N
    //    because the lock serializes them.
    let projection = match project_locked::<ResearchDomain>(root, RESEARCH_LOG_RELATIVE_PATH) {
        Ok(projection) => projection,
        Err(source) => {
            return AdmissionResult {
                status: AdmissionStatus::StoreError(source),
                source_id,
            };
        }
    };
    let sequence = next_sequence::<ResearchDomain>(&projection);

    // 4. Append the event. The serialize→Value→append shim lives in
    //    forge_core_eventlog::append_event; the store's internal per-path lock
    //    handles torn-write safety, and our RESEARCH_LOCK_RELATIVE_PATH
    //    serializes the read-sequence-then-write window so the two compose.
    let event = ResearchEvent::SourceAdded {
        sequence,
        at_unix: now_unix(),
        source,
    };
    match append_event::<ResearchDomain>(
        root,
        RESEARCH_LOG_RELATIVE_PATH,
        &event,
        durability,
        &lock,
    ) {
        Ok(_) => AdmissionResult {
            status: AdmissionStatus::Admitted { sequence },
            source_id,
        },
        Err(err) => AdmissionResult {
            status: AdmissionStatus::StoreError(err),
            source_id,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project;
    use forge_core_contracts::ResearchSourceKind;
    use std::fs;
    use std::path::PathBuf;

    /// Hand-rolled temp dir (repo convention: no `tempfile` workspace dep).
    fn temp_root(label: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("forge-research-{label}-{pid}-{nanos}"));
        fs::create_dir_all(&path).expect("create temp root");
        path
    }

    fn permissive_policy() -> ResearchPolicy {
        ResearchPolicy {
            permitted_source_kinds: vec![
                ResearchSourceKind::Paper,
                ResearchSourceKind::WebUrl,
                ResearchSourceKind::LocalDoc,
                ResearchSourceKind::RepoRef,
            ],
            require_content_hash: false,
            require_trace_ref: false,
        }
    }

    fn deny_all_policy() -> ResearchPolicy {
        ResearchPolicy {
            permitted_source_kinds: vec![],
            require_content_hash: false,
            require_trace_ref: false,
        }
    }

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

    #[test]
    fn admit_allowed_appends_event_and_advances_sequence() {
        let root = temp_root("admit-allowed");
        let result = admit_source(&root, sample_source("s.one"), &permissive_policy());
        assert!(result.is_admitted(), "{result:?}");
        let AdmissionStatus::Admitted { sequence } = result.status else {
            panic!("expected Admitted");
        };
        assert_eq!(sequence, 1, "first admit is sequence 1");
        let projection = project(&root).expect("project after admit");
        assert!(projection.sources.contains_key("s.one"));
        assert_eq!(projection.sequence, 1);
    }

    #[test]
    fn admit_denied_by_gate_appends_nothing() {
        let root = temp_root("admit-denied");
        let result = admit_source(&root, sample_source("s.one"), &deny_all_policy());
        assert!(!result.is_admitted());
        assert!(matches!(result.status, AdmissionStatus::DeniedByGate(_)));
        // No event was appended — the log does not exist.
        assert!(!root.join(RESEARCH_LOG_RELATIVE_PATH).exists());
        let projection = project(&root).expect("project is empty");
        assert!(projection.is_empty());
    }

    #[test]
    fn admit_denied_does_not_advance_sequence_for_later_admit() {
        let root = temp_root("admit-denied-then-allowed");
        let denied = admit_source(&root, sample_source("s.denied"), &deny_all_policy());
        assert!(!denied.is_admitted());
        let allowed = admit_source(&root, sample_source("s.ok"), &permissive_policy());
        let AdmissionStatus::Admitted { sequence } = allowed.status else {
            panic!("expected Admitted, got {:?}", allowed.status);
        };
        assert_eq!(sequence, 1, "denial must not consume a sequence number");
    }

    #[test]
    fn admit_two_sources_yields_monotonic_sequence() {
        let root = temp_root("admit-two");
        let r1 = admit_source(&root, sample_source("s.one"), &permissive_policy());
        let r2 = admit_source(&root, sample_source("s.two"), &permissive_policy());
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

    /// The explicit-durability knob is wired through to `append_event`. This
    /// pins the `NoSync` path (the documented test-mode durability) which the
    /// default `admit_source` wrapper never exercises directly.
    #[test]
    fn admit_with_explicit_no_sync_durability_persists() {
        let root = temp_root("admit-nosync");
        let result = admit_source_with_durability(
            &root,
            sample_source("s.nosync"),
            &permissive_policy(),
            WalDurability::NoSync,
        );
        let AdmissionStatus::Admitted { sequence } = result.status else {
            panic!("expected Admitted, got {:?}", result.status);
        };
        assert_eq!(sequence, 1);
        // The event must have hit disk regardless of durability: a SyncOnAppend
        // vs NoSync difference is invisible to a subsequent cold read in the
        // same process. What we assert is that the *_with_durability entry
        // point round-trips the event through the store, not that NoSync skips
        // fsync (that is a store-level concern, not the PEP's).
        let projection = project(&root).expect("project after NoSync admit");
        assert!(projection.sources.contains_key("s.nosync"));
    }

    /// The `StoreError` arm of `AdmissionStatus` is exercised when the cold
    /// read inside the critical section hits a hard `Parse` failure (schema
    /// drift: valid JSON, wrong shape for `ResearchEvent`). This is the only
    /// store-error path that is deterministic without a second process or
    /// platform-specific permission tricks.
    #[test]
    fn admit_returns_store_error_when_log_has_schema_drift() {
        let root = temp_root("admit-schema-drift");
        // Seed the log with a line that is valid JSON but cannot deserialize
        // as a ResearchEvent (no `sequence`/`at_unix`/variant tag). This is
        // schema drift, which project_locked treats as a hard Parse error --
        // distinct from a torn line (invalid JSON) which is a soft diagnostic.
        fs::create_dir_all(root.join("research")).expect("mkdir research");
        fs::write(
            root.join(RESEARCH_LOG_RELATIVE_PATH),
            serde_json::to_string(&serde_json::json!({"unrelated": "shape"}))
                .expect("serialize drift"),
        )
        .expect("seed drift");

        let result = admit_source(&root, sample_source("s.x"), &permissive_policy());
        assert!(
            matches!(result.status, AdmissionStatus::StoreError(_)),
            "schema drift must surface as StoreError, got {:?}",
            result.status
        );
        assert_eq!(result.source_id, SourceId("s.x".into()));
    }
}
