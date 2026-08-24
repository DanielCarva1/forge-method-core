//! Strict, append-only storage for authoritative workflow-governance receipts.
//!
//! This store deliberately has no torn-tail repair. A malformed, truncated,
//! oversized, or internally inconsistent ledger fails closed and requires an
//! operator-mediated recovery from a known-good durable copy.
//!
//! Replacement recovery protects against interrupted local writes. It is not
//! an external rollback anchor: an actor able to replace the WAL and remove all
//! protocol artifacts can still present an older, internally valid ledger.

use forge_core_contracts::gate::GateStatus;
use forge_core_contracts::request::RequestStatus;
use forge_core_contracts::workflow_governance::{
    WorkflowCooperativeAuthorityBasis, WorkflowCooperativeObjectiveRevisionKind,
    WorkflowReadinessProfile, MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES,
    WORKFLOW_GOVERNANCE_COOPERATIVE_EVIDENCE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_REVISION_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_CURRENT_WORK_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_INTENT_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_LEGACY_SOLO_ADOPTION_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_PRIOR_EVIDENCE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_QUICK_CYCLE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_READINESS_PROFILE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_STRICT_REPLAY_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_WORK_FOCUS_BINDINGS_LEDGER_SCHEMA_VERSION,
};
use forge_core_contracts::{
    CoordinationRequestState, CoordinationStateAppliedEvent, CoordinationStateRecord,
    CoreDomainPackRebasedEvent, DomainPackGenerationTransitionedEvent,
    LegacySoloProfileAdoptedEvent, Phase, PhaseAdvancedEvent, PostBuildVerifyEpisodeAppliedEvent,
    PostBuildVerifyEpisodeOutcome, PostBuildVerifyGateKind, ReleaseUpgradedEvent, StableId,
    WorkflowCooperativeEvidenceObservedEvent, WorkflowCooperativeObjectiveInput,
    WorkflowEffectiveBundleIdentity, WorkflowGovernanceEvent, WorkflowGovernanceLedgerRecord,
    WorkflowGovernanceReceiptDocument, WorkflowGovernanceReleaseIdentity, WorkflowReceiptCarryover,
    WorkflowRuntimeBundleIdentity, WorkflowWorkFocusRecordedEvent, WorkflowWorkFocusState,
    MAX_QUICK_CYCLE_CLOSEOUT_SUMMARY_BYTES, MAX_QUICK_CYCLE_COMPACTNESS_REASON_BYTES,
    MAX_QUICK_CYCLE_EVIDENCE_ITEMS, MAX_QUICK_CYCLE_EXPANSION_ITEMS,
    MAX_QUICK_CYCLE_EXPANSION_REASON_BYTES, MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES,
    MAX_WORKFLOW_INTENT_ITEM_BYTES, MAX_WORKFLOW_INTENT_LIST_ITEMS,
    MAX_WORKFLOW_INTENT_TOTAL_BYTES, MAX_WORK_FOCUS_EVENT_BYTES, MAX_WORK_FOCUS_LIST_ITEMS,
    MAX_WORK_FOCUS_TEXT_BYTES, WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_HOST_ORIGIN_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_POST_BUILD_VERIFY_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_REBASE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_REPLACEMENT_CONTINUITY_LEDGER_SCHEMA_VERSION,
};
use forge_core_store::{
    acquire_effect_store_lock, acquire_existing_effect_store_lock, EffectStoreLock,
    EffectStoreLockError,
};
use serde_json_canonicalizer::to_vec as to_canonical_json;
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH: &str = "wal/workflow-governance.ndjson";
pub const WORKFLOW_GOVERNANCE_LOCK_RELATIVE_PATH: &str = "locks/workflow-governance.lock";
pub const WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES: u64 = 8 * 1024 * 1024;
pub const WORKFLOW_GOVERNANCE_LEDGER_MAX_RECORDS: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowGovernanceLedgerIdentity {
    pub project_id: StableId,
    pub bundle_id: StableId,
    pub bundle_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowGovernanceLedgerProjection {
    pub records: Vec<WorkflowGovernanceLedgerRecord>,
    pub head_digest: Option<String>,
    pub next_sequence: u64,
    pub next_state_version: u64,
}

impl WorkflowGovernanceLedgerProjection {
    /// Identity fixed by the first, `project_imported` record.
    #[must_use]
    pub fn genesis_identity(&self) -> Option<WorkflowGovernanceLedgerIdentity> {
        self.records
            .first()
            .map(WorkflowGovernanceLedgerIdentity::from_record)
    }

    /// Backward-compatible alias for [`Self::genesis_identity`].
    #[must_use]
    pub fn identity(&self) -> Option<WorkflowGovernanceLedgerIdentity> {
        self.genesis_identity()
    }

    /// Runtime identity active after applying every release transition.
    ///
    /// A transition record retains the source identity in its envelope; its
    /// target becomes active only for the following record.
    #[must_use]
    pub fn active_identity(&self) -> Option<WorkflowGovernanceLedgerIdentity> {
        let mut active = self.genesis_identity()?;
        for record in &self.records {
            let target = match &record.event {
                WorkflowGovernanceEvent::ReleaseUpgraded(event) => Some(&event.to_runtime_bundle),
                WorkflowGovernanceEvent::CoreDomainPackRebased(event) => {
                    Some(&event.release_transition.to_runtime_bundle)
                }
                _ => None,
            };
            if let Some(target) = target {
                active.bundle_id = target.bundle_id.clone();
                active.bundle_digest.clone_from(&target.bundle_digest);
            }
        }
        Some(active)
    }

    /// Last fully described runtime identity admitted by a transition.
    ///
    /// Legacy genesis records predate `policy_set_digest`, so this is `None`
    /// until the first release transition supplies that additional binding.
    #[must_use]
    pub fn active_runtime_bundle_identity(&self) -> Option<WorkflowRuntimeBundleIdentity> {
        self.records
            .iter()
            .rev()
            .find_map(|record| match &record.event {
                WorkflowGovernanceEvent::ReleaseUpgraded(event) => {
                    Some(event.to_runtime_bundle.clone())
                }
                WorkflowGovernanceEvent::CoreDomainPackRebased(event) => {
                    Some(event.release_transition.to_runtime_bundle.clone())
                }
                _ => None,
            })
    }

    /// Last effective core-plus-Domain-Pack epoch durably adopted by the
    /// workflow ledger. `None` is the backward-compatible core-only state.
    #[must_use]
    pub fn active_effective_bundle_identity(&self) -> Option<WorkflowEffectiveBundleIdentity> {
        self.records
            .iter()
            .rev()
            .find_map(|record| match &record.event {
                WorkflowGovernanceEvent::DomainPackGenerationTransitioned(event) => {
                    Some(event.to_effective_bundle.clone())
                }
                WorkflowGovernanceEvent::CoreDomainPackRebased(event) => {
                    Some(event.to_effective_bundle.clone())
                }
                _ => None,
            })
    }

    #[must_use]
    pub fn current_state_version(&self) -> Option<u64> {
        self.records.last().map(|record| record.state_version)
    }

    /// Current readiness profile. Historical profile-less ledgers remain
    /// strict until an explicit, append-only legacy solo adoption is present.
    #[must_use]
    pub fn readiness_profile(&self) -> Option<WorkflowReadinessProfile> {
        let genesis = self
            .records
            .first()
            .and_then(|record| match &record.event {
                WorkflowGovernanceEvent::ProjectImported(imported) => {
                    Some(imported.effective_readiness_profile())
                }
                _ => None,
            })?;
        if self.records.iter().any(|record| {
            matches!(
                record.event,
                WorkflowGovernanceEvent::LegacySoloProfileAdopted(_)
            )
        }) {
            Some(WorkflowReadinessProfile::SoloCooperative)
        } else {
            Some(genesis)
        }
    }

    /// Whether this exact projection contains the one-way legacy transition.
    #[must_use]
    pub fn contains_legacy_solo_adoption(&self) -> bool {
        self.records.iter().any(|record| {
            matches!(
                record.event,
                WorkflowGovernanceEvent::LegacySoloProfileAdopted(_)
            )
        })
    }

    fn contains_explicit_readiness_profile(&self) -> bool {
        self.records.first().is_some_and(|record| {
            matches!(
                &record.event,
                WorkflowGovernanceEvent::ProjectImported(imported)
                    if imported.readiness_profile.is_some()
            )
        })
    }

    fn contains_core_domain_pack_rebase(&self) -> bool {
        self.records.iter().any(|record| {
            matches!(
                &record.event,
                WorkflowGovernanceEvent::CoreDomainPackRebased(_)
            )
        })
    }

    fn contains_human_intent_revision(&self) -> bool {
        self.records.iter().any(|record| {
            matches!(
                &record.event,
                WorkflowGovernanceEvent::HumanIntentRevisionAccepted(_)
            )
        })
    }

    fn contains_cooperative_objective(&self) -> bool {
        self.records.iter().any(|record| {
            matches!(
                record.event,
                WorkflowGovernanceEvent::CooperativeObjectiveAccepted(_)
            )
        })
    }

    fn contains_cooperative_objective_revision(&self) -> bool {
        self.records.iter().any(|record| {
            matches!(
                &record.event,
                WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event) if event.revision > 1
            )
        })
    }

    fn contains_cooperative_evidence(&self) -> bool {
        self.records.iter().any(|record| {
            matches!(
                record.event,
                WorkflowGovernanceEvent::CooperativeEvidenceObserved(_)
            )
        })
    }

    fn contains_prior_cooperative_evidence(&self) -> bool {
        self.records
            .iter()
            .any(|record| event_contains_prior_cooperative_evidence(&record.event))
    }

    fn latest_cooperative_objective(
        &self,
    ) -> Option<&forge_core_contracts::CooperativeObjectiveAcceptedEvent> {
        self.records
            .iter()
            .rev()
            .find_map(|record| match &record.event {
                WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event) => Some(event),
                _ => None,
            })
    }

    fn work_focus_wire_level(&self) -> u8 {
        self.records
            .iter()
            .map(|record| work_focus_wire_level(&record.event))
            .max()
            .unwrap_or_default()
    }

    /// Latest Work Focus event after the ledger has passed full recovery
    /// validation. Callers receive no append authority through this view.
    #[must_use]
    pub fn latest_work_focus_record(
        &self,
    ) -> Option<(
        &WorkflowGovernanceLedgerRecord,
        &WorkflowWorkFocusRecordedEvent,
    )> {
        self.records.iter().rev().find_map(|record| {
            let WorkflowGovernanceEvent::WorkFocusRecorded(event) = &record.event else {
                return None;
            };
            Some((record, event))
        })
    }
    fn contains_native_host_provenance(&self) -> bool {
        self.records.iter().any(|record| {
            matches!(
                &record.event,
                WorkflowGovernanceEvent::BrokerOriginApplied(event)
                    if event.native_host_provenance.is_some()
            )
        })
    }

    fn contains_strict_native_interaction_replay_identity(&self) -> bool {
        self.records.iter().any(|record| {
            matches!(
                &record.event,
                WorkflowGovernanceEvent::BrokerOriginApplied(event)
                    if event.native_interaction_replay_digest.is_some()
            )
        })
    }

    fn contains_post_build_verify_episode(&self) -> bool {
        self.records.iter().any(|record| {
            matches!(
                record.event,
                WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(_)
            )
        })
    }

    fn contains_replacement_continuity(&self) -> bool {
        self.records.iter().any(|record| match &record.event {
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event) => {
                event.episode_snapshot.is_some()
            }
            WorkflowGovernanceEvent::CoordinationStateApplied(_) => true,
            _ => false,
        })
    }
}

impl WorkflowGovernanceLedgerIdentity {
    fn from_record(record: &WorkflowGovernanceLedgerRecord) -> Self {
        Self {
            project_id: record.project_id.clone(),
            bundle_id: record.bundle_id.clone(),
            bundle_digest: record.bundle_digest.clone(),
        }
    }
}

/// Exclusive authority scope for capture, late recheck, and durable append.
///
/// Keeping this value alive retains the exact OS lock. Both [`Self::recover`]
/// and append operations execute without releasing it, allowing a kernel to
/// capture inputs, perform work, re-read the head, and append completion under
/// one authority boundary.
#[derive(Debug)]
pub struct LockedWorkflowGovernanceLedger {
    state_root: PathBuf,
    lock: EffectStoreLock,
}

/// Exact workflow-governance WAL bytes and their strictly recovered projection.
///
/// The snapshot borrows the retained ledger lock, so callers cannot keep its
/// candidate backup material after releasing the producer authority that bound
/// the state root and WAL namespace.
#[derive(Debug)]
pub struct WorkflowGovernanceRawWalSnapshot<'lock> {
    raw_wal_bytes: Option<Vec<u8>>,
    projection: WorkflowGovernanceLedgerProjection,
    _authority: &'lock EffectStoreLock,
}

impl WorkflowGovernanceRawWalSnapshot<'_> {
    /// Exact current WAL bytes, or `None` for a pristine empty ledger.
    #[must_use]
    pub fn raw_wal_bytes(&self) -> Option<&[u8]> {
        self.raw_wal_bytes.as_deref()
    }

    /// Projection strictly recovered from [`Self::raw_wal_bytes`].
    #[must_use]
    pub fn projection(&self) -> &WorkflowGovernanceLedgerProjection {
        &self.projection
    }
}

/// Incrementally prepared workflow-governance records guarded by the ledger lock.
///
/// Records are visible through [`Self::projection`] as they are prepared, but
/// no WAL bytes change until [`Self::commit`] atomically replaces the complete
/// file. Dropping this value before commit therefore discards the whole batch.
#[doc(hidden)]
#[derive(Debug)]
#[must_use = "dropping a workflow-governance batch discards all prepared records"]
pub struct WorkflowGovernanceLedgerBatch<'a> {
    ledger: &'a mut LockedWorkflowGovernanceLedger,
    identity: WorkflowGovernanceLedgerIdentity,
    projection: WorkflowGovernanceLedgerProjection,
    original_record_count: usize,
    prepared_wal: Vec<u8>,
}

impl WorkflowGovernanceLedgerBatch<'_> {
    /// Prepare one event without changing the durable WAL.
    ///
    /// # Errors
    ///
    /// Fails closed for a repeated import event, state regression or overflow,
    /// record/byte capacity exhaustion, randomness, clock, or encoding errors.
    pub fn push_event(
        &mut self,
        state_version: u64,
        event: WorkflowGovernanceEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        if matches!(event, WorkflowGovernanceEvent::ReleaseUpgraded(_)) {
            return Err(WorkflowGovernanceLedgerError::ReleaseUpgradeRequiresDedicatedAuthority);
        }
        if matches!(
            event,
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(_)
        ) {
            return Err(
                WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeRequiresDedicatedAuthority,
            );
        }
        if matches!(event, WorkflowGovernanceEvent::CoordinationStateApplied(_)) {
            return Err(WorkflowGovernanceLedgerError::CoordinationStateRequiresDedicatedAuthority);
        }
        if matches!(
            event,
            WorkflowGovernanceEvent::DomainPackGenerationTransitioned(_)
                | WorkflowGovernanceEvent::CoreDomainPackRebased(_)
        ) {
            return Err(
                WorkflowGovernanceLedgerError::DomainPackTransitionRequiresDedicatedAuthority,
            );
        }
        if matches!(
            event,
            WorkflowGovernanceEvent::HumanIntentRevisionAccepted(_)
        ) {
            return Err(WorkflowGovernanceLedgerError::HumanIntentRevisionRequiresBrokerAuthority);
        }
        if matches!(
            event,
            WorkflowGovernanceEvent::CooperativeObjectiveAccepted(_)
        ) {
            return Err(
                WorkflowGovernanceLedgerError::CooperativeObjectiveRequiresDedicatedAuthority,
            );
        }
        if matches!(
            event,
            WorkflowGovernanceEvent::CooperativeEvidenceObserved(_)
        ) {
            return Err(
                WorkflowGovernanceLedgerError::CooperativeEvidenceRequiresDedicatedAuthority,
            );
        }
        if matches!(event, WorkflowGovernanceEvent::WorkFocusRecorded(_)) {
            return Err(WorkflowGovernanceLedgerError::WorkFocusRequiresDedicatedAuthority);
        }
        if matches!(event, WorkflowGovernanceEvent::ProjectImported(_)) {
            return Err(WorkflowGovernanceLedgerError::ProjectImportedAfterInitialization);
        }
        if matches!(event, WorkflowGovernanceEvent::LegacySoloProfileAdopted(_)) {
            return Err(
                WorkflowGovernanceLedgerError::LegacySoloAdoptionRequiresDedicatedAuthority,
            );
        }
        let previous_state_version = self
            .projection
            .current_state_version()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        if state_version < previous_state_version {
            return Err(WorkflowGovernanceLedgerError::StateVersionRegression {
                previous: previous_state_version,
                found: state_version,
            });
        }
        if let WorkflowGovernanceEvent::PhaseAdvanced(event) = &event {
            validate_phase_advance_source(
                event,
                projection_current_phase(&self.projection).as_ref(),
            )?;
        }

        let (record, line) =
            build_record_line(&self.projection, &self.identity, state_version, event)?;
        ensure_prepared_capacity(&self.projection, self.prepared_wal.len(), line.len())?;
        let next_sequence = record.sequence.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::SequenceOverflow {
                current: record.sequence,
            },
        )?;
        let next_state_version = state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: state_version,
            },
        )?;

        self.prepared_wal.extend_from_slice(&line);
        self.projection.head_digest = Some(record.record_digest.clone());
        self.projection.next_sequence = next_sequence;
        self.projection.next_state_version = next_state_version;
        self.projection.records.push(record.clone());
        Ok(record)
    }

    /// Prepare one explicit legacy profile adoption without changing durable bytes.
    #[doc(hidden)]
    pub fn push_legacy_solo_adoption_unchecked_tcb(
        &mut self,
        state_version: u64,
        event: LegacySoloProfileAdoptedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        validate_legacy_solo_adoption(&self.projection, state_version, &event)?;
        let (record, line) = build_record_line(
            &self.projection,
            &self.identity,
            state_version,
            WorkflowGovernanceEvent::LegacySoloProfileAdopted(event),
        )?;
        ensure_prepared_capacity(&self.projection, self.prepared_wal.len(), line.len())?;
        self.prepared_wal.extend_from_slice(&line);
        self.projection.head_digest = Some(record.record_digest.clone());
        self.projection.next_sequence = record.sequence.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::SequenceOverflow {
                current: record.sequence,
            },
        )?;
        self.projection.next_state_version = state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: state_version,
            },
        )?;
        self.projection.records.push(record.clone());
        Ok(record)
    }

    /// Prepare one same-owner cooperative objective revision. Semantic input
    /// and packet reconstruction remain kernel-owned; this boundary enforces
    /// the solo profile, exact head/clock/digest shape, adjacent immutable
    /// history, and additive-only clarification semantics.
    #[doc(hidden)]
    pub fn push_cooperative_objective_unchecked_tcb(
        &mut self,
        state_version: u64,
        event: forge_core_contracts::CooperativeObjectiveAcceptedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        validate_cooperative_objective_event(
            &self.projection,
            &event,
            self.projection.records.len() + 1,
        )?;
        let recorded_at_unix = event.accepted_at_unix;
        let (record, line) = build_record_line_at(
            &self.projection,
            &self.identity,
            state_version,
            WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event),
            recorded_at_unix,
        )?;
        ensure_prepared_capacity(&self.projection, self.prepared_wal.len(), line.len())?;
        let next_sequence = record.sequence.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::SequenceOverflow {
                current: record.sequence,
            },
        )?;
        let next_state_version = state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: state_version,
            },
        )?;
        self.prepared_wal.extend_from_slice(&line);
        self.projection.head_digest = Some(record.record_digest.clone());
        self.projection.next_sequence = next_sequence;
        self.projection.next_state_version = next_state_version;
        self.projection.records.push(record.clone());
        Ok(record)
    }

    /// Prepare one same-owner evidence admission or rejection. The kernel
    /// supplies the policy decision; this boundary preserves its audit record
    /// beneath the exact current ledger head.
    #[doc(hidden)]
    pub fn push_cooperative_evidence_unchecked_tcb(
        &mut self,
        state_version: u64,
        event: WorkflowCooperativeEvidenceObservedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        validate_cooperative_evidence_event(
            &self.projection,
            &event,
            self.projection.records.len() + 1,
        )?;
        let recorded_at_unix = event.observed_at_unix;
        let (record, line) = build_record_line_at(
            &self.projection,
            &self.identity,
            state_version,
            WorkflowGovernanceEvent::CooperativeEvidenceObserved(event),
            recorded_at_unix,
        )?;
        ensure_prepared_capacity(&self.projection, self.prepared_wal.len(), line.len())?;
        self.prepared_wal.extend_from_slice(&line);
        self.projection.head_digest = Some(record.record_digest.clone());
        self.projection.next_sequence = record.sequence.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::SequenceOverflow {
                current: record.sequence,
            },
        )?;
        self.projection.next_state_version = state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: state_version,
            },
        )?;
        self.projection.records.push(record.clone());
        Ok(record)
    }

    /// Prepare one bounded continuity snapshot beneath the exact current head.
    #[doc(hidden)]
    pub fn push_work_focus_unchecked_tcb(
        &mut self,
        state_version: u64,
        event: WorkflowWorkFocusRecordedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        validate_work_focus_event(
            &self.projection,
            state_version,
            &event,
            self.projection.records.len() + 1,
        )?;
        let recorded_at_unix = event.recorded_at_unix;
        let (record, line) = build_record_line_at(
            &self.projection,
            &self.identity,
            state_version,
            WorkflowGovernanceEvent::WorkFocusRecorded(event),
            recorded_at_unix,
        )?;
        ensure_prepared_capacity(&self.projection, self.prepared_wal.len(), line.len())?;
        self.prepared_wal.extend_from_slice(&line);
        self.projection.head_digest = Some(record.record_digest.clone());
        self.projection.next_sequence = record.sequence.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::SequenceOverflow {
                current: record.sequence,
            },
        )?;
        self.projection.next_state_version = state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: state_version,
            },
        )?;
        self.projection.records.push(record.clone());
        Ok(record)
    }

    /// Prepare one broker-origin action with a deterministic envelope while
    /// retaining the existing random-id API unchanged for every other lane.
    /// The event kind is derived from the typed event and cannot be supplied
    /// as serialized input by a host.
    #[doc(hidden)]
    pub fn push_verified_broker_action_unchecked_tcb(
        &mut self,
        state_version: u64,
        event: WorkflowGovernanceEvent,
        action_packet_digest: &str,
        broker_event_digest: &str,
        recorded_at_unix: u64,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        let event_kind = broker_action_event_kind(&event).ok_or(
            WorkflowGovernanceLedgerError::InvalidBrokerActionBinding {
                reason: "event is not a broker-applicable workflow action",
            },
        )?;
        if !is_lower_sha256(action_packet_digest)
            || !is_lower_sha256(broker_event_digest)
            || recorded_at_unix == 0
        {
            return Err(WorkflowGovernanceLedgerError::InvalidBrokerActionBinding {
                reason: "packet/event digests and verified broker clock are required",
            });
        }
        if let WorkflowGovernanceEvent::HumanIntentRevisionAccepted(intent) = &event {
            if intent.acceptance_action_packet_digest != action_packet_digest
                || intent.accepted_at_unix != recorded_at_unix
                || self.projection.head_digest.as_deref()
                    != Some(intent.ledger_head_digest.as_str())
                || !is_lower_sha256(&intent.intent_digest)
                || !is_lower_sha256(&intent.snapshot_digest)
                || intent.assurance_epoch == 0
                || intent.intent.revision == 0
            {
                return Err(WorkflowGovernanceLedgerError::InvalidBrokerActionBinding {
                    reason: "human intent event does not match its packet, head, clock, or epoch",
                });
            }
        }
        let previous_state_version = self
            .projection
            .current_state_version()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        if state_version < previous_state_version {
            return Err(WorkflowGovernanceLedgerError::StateVersionRegression {
                previous: previous_state_version,
                found: state_version,
            });
        }
        let (record, line) = build_deterministic_broker_record_line(
            &self.projection,
            &self.identity,
            state_version,
            event,
            &DeterministicBrokerRecordBinding {
                action_packet_digest,
                broker_event_digest,
                event_kind,
                recorded_at_unix,
            },
        )?;
        if self
            .projection
            .records
            .iter()
            .any(|existing| existing.record_id == record.record_id)
        {
            return Err(WorkflowGovernanceLedgerError::DuplicateRecordId {
                line: self.projection.records.len() + 1,
                record_id: record.record_id,
            });
        }
        ensure_prepared_capacity(&self.projection, self.prepared_wal.len(), line.len())?;
        let next_sequence = record.sequence.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::SequenceOverflow {
                current: record.sequence,
            },
        )?;
        let next_state_version = state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: state_version,
            },
        )?;
        self.prepared_wal.extend_from_slice(&line);
        self.projection.head_digest = Some(record.record_digest.clone());
        self.projection.next_sequence = next_sequence;
        self.projection.next_state_version = next_state_version;
        self.projection.records.push(record.clone());
        Ok(record)
    }

    fn push_post_build_verify_episode_tcb(
        &mut self,
        state_version: u64,
        event: PostBuildVerifyEpisodeAppliedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        if self.projection.records.len() != self.original_record_count {
            return Err(
                WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                    reason: "episode route must be the first and only event in its batch",
                },
            );
        }
        let previous_state_version = self
            .projection
            .current_state_version()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        let expected_state_version = previous_state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: previous_state_version,
            },
        )?;
        if state_version != expected_state_version {
            return Err(
                WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                    reason: "episode route state version is not contiguous",
                },
            );
        }
        validate_post_build_verify_episode_event(
            &event,
            projection_current_phase(&self.projection).as_ref(),
            active_release_identity(&self.projection).as_ref(),
            self.projection.head_digest.as_deref(),
            Some(previous_state_version),
            last_post_build_verify_episode(&self.projection, &event.episode_id)
                .map(|previous| (previous.generation, previous.episode_digest.as_str())),
            true,
        )?;
        let (record, line) = build_record_line(
            &self.projection,
            &self.identity,
            state_version,
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event),
        )?;
        ensure_prepared_capacity(&self.projection, self.prepared_wal.len(), line.len())?;
        self.prepared_wal.extend_from_slice(&line);
        self.projection.head_digest = Some(record.record_digest.clone());
        self.projection.next_sequence = record.sequence.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::SequenceOverflow {
                current: record.sequence,
            },
        )?;
        self.projection.next_state_version = state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: state_version,
            },
        )?;
        self.projection.records.push(record.clone());
        Ok(record)
    }

    fn push_coordination_state_tcb(
        &mut self,
        state_version: u64,
        event: CoordinationStateAppliedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        if self.projection.records.len() != self.original_record_count {
            return Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
                reason: "coordination update must be the first and only event in its batch",
            });
        }
        let previous_state_version = self
            .projection
            .current_state_version()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        let expected_state_version = previous_state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: previous_state_version,
            },
        )?;
        if state_version != expected_state_version {
            return Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
                reason: "coordination update state version is not contiguous",
            });
        }
        let previous_request = match &event.state {
            CoordinationStateRecord::Request(state) => {
                last_coordination_request(&self.projection, &state.request.request_contract.id)
            }
            CoordinationStateRecord::Completion(_) | CoordinationStateRecord::HealthRecovery(_) => {
                None
            }
        };
        validate_coordination_state_event(
            &event,
            self.projection.head_digest.as_deref(),
            Some(previous_state_version),
            previous_request,
        )?;
        let (record, line) = build_record_line(
            &self.projection,
            &self.identity,
            state_version,
            WorkflowGovernanceEvent::CoordinationStateApplied(event),
        )?;
        ensure_prepared_capacity(&self.projection, self.prepared_wal.len(), line.len())?;
        self.prepared_wal.extend_from_slice(&line);
        self.projection.head_digest = Some(record.record_digest.clone());
        self.projection.next_sequence = record.sequence.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::SequenceOverflow {
                current: record.sequence,
            },
        )?;
        self.projection.next_state_version = state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: state_version,
            },
        )?;
        self.projection.records.push(record.clone());
        Ok(record)
    }

    fn push_release_transition_tcb(
        &mut self,
        target_identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: ReleaseUpgradedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        if self.projection.records.len() != self.original_record_count {
            return Err(WorkflowGovernanceLedgerError::DuplicateReleaseTransition);
        }
        let previous_state_version = self
            .projection
            .current_state_version()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        let expected_state_version = previous_state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: previous_state_version,
            },
        )?;
        if state_version != expected_state_version {
            return Err(
                WorkflowGovernanceLedgerError::ReleaseTransitionStateVersionMismatch {
                    expected: expected_state_version,
                    found: state_version,
                },
            );
        }
        if self.projection.active_effective_bundle_identity().is_some() {
            return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
                reason: "active Domain Pack generation requires an explicit core rebase",
            });
        }
        let active_release = active_release_identity(&self.projection);
        let active_runtime = self.projection.active_runtime_bundle_identity();
        validate_release_transition(
            &event,
            &self.identity,
            target_identity,
            active_release.as_ref(),
            active_runtime.as_ref(),
            self.projection.head_digest.as_deref(),
        )?;

        let (record, line) = build_record_line(
            &self.projection,
            &self.identity,
            state_version,
            WorkflowGovernanceEvent::ReleaseUpgraded(event),
        )?;
        ensure_prepared_capacity(&self.projection, self.prepared_wal.len(), line.len())?;
        self.prepared_wal.extend_from_slice(&line);
        self.projection.head_digest = Some(record.record_digest.clone());
        self.projection.next_sequence = record.sequence.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::SequenceOverflow {
                current: record.sequence,
            },
        )?;
        self.projection.next_state_version = state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: state_version,
            },
        )?;
        self.projection.records.push(record.clone());
        Ok(record)
    }

    fn push_domain_pack_transition_tcb(
        &mut self,
        state_version: u64,
        event: DomainPackGenerationTransitionedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        if self.projection.records.len() != self.original_record_count {
            return Err(WorkflowGovernanceLedgerError::DuplicateDomainPackTransition);
        }
        let previous_state_version = self
            .projection
            .current_state_version()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        let expected_state_version = previous_state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: previous_state_version,
            },
        )?;
        if state_version != expected_state_version {
            return Err(
                WorkflowGovernanceLedgerError::DomainPackTransitionStateVersionMismatch {
                    expected: expected_state_version,
                    found: state_version,
                },
            );
        }
        validate_domain_pack_transition(
            &event,
            self.projection.active_identity().as_ref(),
            self.projection.active_runtime_bundle_identity().as_ref(),
            self.projection.active_effective_bundle_identity().as_ref(),
            self.projection.head_digest.as_deref(),
        )?;
        let (record, line) = build_record_line(
            &self.projection,
            &self.identity,
            state_version,
            WorkflowGovernanceEvent::DomainPackGenerationTransitioned(event),
        )?;
        ensure_prepared_capacity(&self.projection, self.prepared_wal.len(), line.len())?;
        self.prepared_wal.extend_from_slice(&line);
        self.projection.head_digest = Some(record.record_digest.clone());
        self.projection.next_sequence = record.sequence.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::SequenceOverflow {
                current: record.sequence,
            },
        )?;
        self.projection.next_state_version = state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: state_version,
            },
        )?;
        self.projection.records.push(record.clone());
        Ok(record)
    }

    fn push_core_domain_pack_rebase_tcb(
        &mut self,
        target_identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: CoreDomainPackRebasedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        if self.projection.records.len() != self.original_record_count {
            return Err(WorkflowGovernanceLedgerError::DuplicateDomainPackTransition);
        }
        let previous_state_version = self
            .projection
            .current_state_version()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        let expected_state_version = previous_state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: previous_state_version,
            },
        )?;
        if state_version != expected_state_version {
            return Err(
                WorkflowGovernanceLedgerError::DomainPackTransitionStateVersionMismatch {
                    expected: expected_state_version,
                    found: state_version,
                },
            );
        }
        validate_core_domain_pack_rebase(
            &event,
            &self.identity,
            target_identity,
            active_release_identity(&self.projection).as_ref(),
            self.projection.active_runtime_bundle_identity().as_ref(),
            self.projection.active_effective_bundle_identity().as_ref(),
            self.projection.head_digest.as_deref(),
        )?;
        let (record, line) = build_record_line(
            &self.projection,
            &self.identity,
            state_version,
            WorkflowGovernanceEvent::CoreDomainPackRebased(Box::new(event)),
        )?;
        ensure_prepared_capacity(&self.projection, self.prepared_wal.len(), line.len())?;
        self.prepared_wal.extend_from_slice(&line);
        self.projection.head_digest = Some(record.record_digest.clone());
        self.projection.next_sequence = record.sequence.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::SequenceOverflow {
                current: record.sequence,
            },
        )?;
        self.projection.next_state_version = state_version.checked_add(1).ok_or(
            WorkflowGovernanceLedgerError::StateVersionOverflow {
                current: state_version,
            },
        )?;
        self.projection.records.push(record.clone());
        Ok(record)
    }

    /// Return the recovered ledger plus every record prepared so far.
    #[must_use]
    pub fn projection(&self) -> &WorkflowGovernanceLedgerProjection {
        &self.projection
    }

    /// Persist every prepared record in one crash-recoverable WAL replacement.
    ///
    /// # Errors
    ///
    /// Rejects an empty batch and forwards safe-path or replacement failures.
    /// On platforms without replace-by-rename semantics, recovery deterministically
    /// resolves the fixed transaction protocol to the old or committed WAL.
    pub fn commit(
        self,
    ) -> Result<Vec<WorkflowGovernanceLedgerRecord>, WorkflowGovernanceLedgerError> {
        if self.projection.records.len() == self.original_record_count {
            return Err(WorkflowGovernanceLedgerError::EmptyBatch);
        }
        replace_wal_atomically(&self.ledger.state_root, &self.prepared_wal)?;
        Ok(self.projection.records[self.original_record_count..].to_vec())
    }
}

impl LockedWorkflowGovernanceLedger {
    /// Capture the exact current WAL and strictly recover its projection beneath
    /// this already-retained producer lock.
    ///
    /// # Errors
    ///
    /// Fails closed if replacement recovery is inconsistent, the ambient state
    /// root or lock binding changed, the WAL is not a confined regular file, its
    /// bytes change while read, capacity is exceeded, or strict recovery fails.
    pub fn snapshot_raw_wal(
        &self,
    ) -> Result<WorkflowGovernanceRawWalSnapshot<'_>, WorkflowGovernanceLedgerError> {
        let authority = self.lock.retained_store_io().map_err(lock_error)?;
        authority
            .validate()
            .map_err(|source| io_error(&self.state_root, source))?;
        let wal_path = workflow_governance_wal_path(&self.state_root)?;
        reconcile_wal_replacement(&wal_path).map_err(|source| io_error(&wal_path, source))?;
        authority
            .validate()
            .map_err(|source| io_error(&self.state_root, source))?;

        let retained_root = self.lock.retained_state_root();
        let mut file =
            match retained_root.open_read(Path::new(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)) {
                Ok(file) => file,
                Err(source) if source.kind() == io::ErrorKind::NotFound => {
                    authority
                        .validate()
                        .map_err(|error| io_error(&self.state_root, error))?;
                    return Ok(WorkflowGovernanceRawWalSnapshot {
                        raw_wal_bytes: None,
                        projection: empty_projection(),
                        _authority: &self.lock,
                    });
                }
                Err(source) => return Err(io_error(&wal_path, source)),
            };
        let before = file
            .metadata()
            .map_err(|source| io_error(&wal_path, source))?;
        if before.len() > WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES {
            return Err(WorkflowGovernanceLedgerError::CapacityBytes {
                found: before.len(),
                maximum: WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES,
            });
        }
        let mut raw_wal_bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
        Read::by_ref(&mut file)
            .take(WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES.saturating_add(1))
            .read_to_end(&mut raw_wal_bytes)
            .map_err(|source| io_error(&wal_path, source))?;
        let found = u64::try_from(raw_wal_bytes.len()).unwrap_or(u64::MAX);
        if found > WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES {
            return Err(WorkflowGovernanceLedgerError::CapacityBytes {
                found,
                maximum: WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES,
            });
        }
        let after = file
            .metadata()
            .map_err(|source| io_error(&wal_path, source))?;
        if before.len() != after.len() || after.len() != found {
            return Err(WorkflowGovernanceLedgerError::Io {
                path: wal_path,
                source: "workflow-governance WAL changed while retained snapshot bytes were read"
                    .to_owned(),
            });
        }
        authority
            .validate()
            .map_err(|source| io_error(&self.state_root, source))?;
        let projection = recover_from_reader(BufReader::new(raw_wal_bytes.as_slice()), &wal_path)?;
        authority
            .validate()
            .map_err(|source| io_error(&self.state_root, source))?;
        Ok(WorkflowGovernanceRawWalSnapshot {
            raw_wal_bytes: Some(raw_wal_bytes),
            projection,
            _authority: &self.lock,
        })
    }

    /// Recover and strictly verify the complete ledger while retaining the lock.
    ///
    /// # Errors
    ///
    /// Returns a typed error for any I/O, capacity, encoding, schema, hash-chain,
    /// identity, or state monotonicity failure.
    pub fn recover(
        &self,
    ) -> Result<WorkflowGovernanceLedgerProjection, WorkflowGovernanceLedgerError> {
        recover_under_lock(&self.state_root)
    }

    /// Initialize an empty ledger with its mandatory `project_imported` event.
    /// Sequence, record id, time, previous digest, and digest are store-owned.
    ///
    /// # Errors
    ///
    /// Fails closed if the ledger is non-empty, the event is not
    /// `project_imported`, or durable append fails.
    #[doc(hidden)]
    pub fn initialize_unchecked_tcb(
        &mut self,
        identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: WorkflowGovernanceEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        validate_identity(identity)?;
        if !matches!(event, WorkflowGovernanceEvent::ProjectImported(_)) {
            return Err(WorkflowGovernanceLedgerError::FirstEventNotProjectImported);
        }
        let projection = self.recover()?;
        if !projection.records.is_empty() {
            return Err(WorkflowGovernanceLedgerError::AlreadyInitialized);
        }
        write_initial_record_atomically(
            &self.state_root,
            &projection,
            identity,
            state_version,
            event,
        )
    }

    /// Append one event after a head-digest CAS while retaining the lock.
    ///
    /// The caller supplies semantic event data and its observed state version,
    /// but cannot choose sequence, record id, timestamp, or chain digests.
    ///
    /// # Errors
    ///
    /// Fails closed on empty/untrusted ledgers, stale expected heads, identity
    /// mismatch, state regression, capacity exhaustion, or durable write error.
    #[doc(hidden)]
    pub fn append_unchecked_tcb_event(
        &mut self,
        expected_head_digest: &str,
        identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: WorkflowGovernanceEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        let mut batch = self.begin_unchecked_tcb_batch(expected_head_digest, identity)?;
        let record = batch.push_event(state_version, event)?;
        batch.commit()?;
        Ok(record)
    }

    /// Append one explicit legacy Solo Cooperative adoption under exact CAS.
    #[doc(hidden)]
    pub fn adopt_legacy_solo_unchecked_tcb(
        &mut self,
        expected_head_digest: &str,
        identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: LegacySoloProfileAdoptedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        let mut batch = self.begin_unchecked_tcb_batch(expected_head_digest, identity)?;
        let record = batch.push_legacy_solo_adoption_unchecked_tcb(state_version, event)?;
        batch.commit()?;
        Ok(record)
    }

    /// Append one Solo Cooperative objective revision under an exact head CAS.
    #[doc(hidden)]
    pub fn accept_cooperative_objective_unchecked_tcb(
        &mut self,
        expected_head_digest: &str,
        identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: forge_core_contracts::CooperativeObjectiveAcceptedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        let mut batch = self.begin_unchecked_tcb_batch(expected_head_digest, identity)?;
        let record = batch.push_cooperative_objective_unchecked_tcb(state_version, event)?;
        batch.commit()?;
        Ok(record)
    }

    /// Append one auditable same-owner evidence decision under an exact head
    /// CAS. Both admitted and rejected offers are durable ledger events.
    #[doc(hidden)]
    pub fn record_cooperative_evidence_unchecked_tcb(
        &mut self,
        expected_head_digest: &str,
        identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: WorkflowCooperativeEvidenceObservedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        let mut batch = self.begin_unchecked_tcb_batch(expected_head_digest, identity)?;
        let record = batch.push_cooperative_evidence_unchecked_tcb(state_version, event)?;
        batch.commit()?;
        Ok(record)
    }

    /// Append one advisory Work Focus snapshot under an exact ledger-head CAS.
    #[doc(hidden)]
    pub fn record_work_focus_unchecked_tcb(
        &mut self,
        expected_head_digest: &str,
        identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: WorkflowWorkFocusRecordedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        let mut batch = self.begin_unchecked_tcb_batch(expected_head_digest, identity)?;
        let record = batch.push_work_focus_unchecked_tcb(state_version, event)?;
        batch.commit()?;
        Ok(record)
    }

    /// Append one structurally validated C5.2 episode route. Candidate
    /// validation and gate admission remain kernel-owned; this boundary enforces
    /// exact ledger, release, phase, generation, and event-shape continuity.
    #[doc(hidden)]
    pub fn apply_post_build_verify_episode_unchecked_tcb(
        &mut self,
        expected_head_digest: &str,
        identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: PostBuildVerifyEpisodeAppliedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        let mut batch = self.begin_unchecked_tcb_batch(expected_head_digest, identity)?;
        let record = batch.push_post_build_verify_episode_tcb(state_version, event)?;
        batch.commit()?;
        Ok(record)
    }

    /// Append one C5.3 coordination projection under the exact retained workflow
    /// ledger head. Semantic request, completion, claim, proof, and recovery
    /// admission remains kernel-owned.
    #[doc(hidden)]
    pub fn apply_coordination_state_unchecked_tcb(
        &mut self,
        expected_head_digest: &str,
        identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: CoordinationStateAppliedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        let mut batch = self.begin_unchecked_tcb_batch(expected_head_digest, identity)?;
        let record = batch.push_coordination_state_tcb(state_version, event)?;
        batch.commit()?;
        Ok(record)
    }

    /// Append exactly one structurally validated release transition.
    ///
    /// Registry admission and predecessor authorization remain kernel-owned;
    /// this TCB boundary validates only ledger/source/target bindings and the
    /// transition DTO's structural integrity under the retained OS lock.
    ///
    /// # Errors
    ///
    /// Fails closed on stale heads, source/target mismatches, malformed
    /// transitions, non-contiguous state versions, or durable commit failure.
    #[doc(hidden)]
    pub fn transition_release_unchecked_tcb(
        &mut self,
        expected_head_digest: &str,
        source_identity: &WorkflowGovernanceLedgerIdentity,
        target_identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: ReleaseUpgradedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        validate_identity(target_identity)?;
        if source_identity.project_id != target_identity.project_id {
            return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
                reason: "source and target project identities differ",
            });
        }
        let mut batch = self.begin_unchecked_tcb_batch(expected_head_digest, source_identity)?;
        let record = batch.push_release_transition_tcb(target_identity, state_version, event)?;
        batch.commit()?;
        Ok(record)
    }

    /// Append exactly one structurally validated Domain Pack effective-bundle
    /// epoch transition under this retained workflow lock.
    #[doc(hidden)]
    pub fn transition_domain_pack_generation_unchecked_tcb(
        &mut self,
        expected_head_digest: &str,
        identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: DomainPackGenerationTransitionedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        let mut batch = self.begin_unchecked_tcb_batch(expected_head_digest, identity)?;
        let record = batch.push_domain_pack_transition_tcb(state_version, event)?;
        batch.commit()?;
        Ok(record)
    }

    /// Append one joined core-release and Domain Pack generation transition.
    /// Both target identities become active in the same workflow WAL record.
    #[doc(hidden)]
    pub fn transition_core_domain_pack_rebase_unchecked_tcb(
        &mut self,
        expected_head_digest: &str,
        source_identity: &WorkflowGovernanceLedgerIdentity,
        target_identity: &WorkflowGovernanceLedgerIdentity,
        state_version: u64,
        event: CoreDomainPackRebasedEvent,
    ) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
        validate_identity(target_identity)?;
        if source_identity.project_id != target_identity.project_id {
            return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
                reason: "source and target project identities differ",
            });
        }
        let mut batch = self.begin_unchecked_tcb_batch(expected_head_digest, source_identity)?;
        let record =
            batch.push_core_domain_pack_rebase_tcb(target_identity, state_version, event)?;
        batch.commit()?;
        Ok(record)
    }

    /// Begin a transactional multi-event append after a head-digest CAS.
    ///
    /// The returned builder borrows this ledger, retaining the same exclusive
    /// OS lock throughout preparation and commit. Identity and the expected
    /// head are validated before any event can be prepared.
    ///
    /// # Errors
    ///
    /// Fails closed on an empty/untrusted ledger, stale expected head,
    /// identity mismatch, recovery failure, or WAL read failure.
    #[doc(hidden)]
    pub fn begin_unchecked_tcb_batch<'a>(
        &'a mut self,
        expected_head_digest: &str,
        identity: &WorkflowGovernanceLedgerIdentity,
    ) -> Result<WorkflowGovernanceLedgerBatch<'a>, WorkflowGovernanceLedgerError> {
        validate_identity(identity)?;
        let projection = self.recover()?;
        let actual_head = projection
            .head_digest
            .as_deref()
            .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
        if expected_head_digest != actual_head {
            return Err(WorkflowGovernanceLedgerError::HeadMismatch {
                expected: expected_head_digest.to_owned(),
                actual: actual_head.to_owned(),
            });
        }
        validate_append_identity(&projection, identity)?;
        let wal_path = workflow_governance_wal_path(&self.state_root)?;
        let prepared_wal = fs::read(&wal_path).map_err(|source| io_error(&wal_path, source))?;
        let original_record_count = projection.records.len();
        Ok(WorkflowGovernanceLedgerBatch {
            ledger: self,
            identity: identity.clone(),
            projection,
            original_record_count,
            prepared_wal,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WorkflowGovernanceLedgerError {
    StateRootUnavailable {
        path: PathBuf,
        source: String,
    },
    Lock {
        source: String,
    },
    Io {
        path: PathBuf,
        source: String,
    },
    CapacityBytes {
        found: u64,
        maximum: u64,
    },
    CapacityRecords {
        found: usize,
        maximum: usize,
    },
    TornTail {
        line: usize,
    },
    BlankLine {
        line: usize,
    },
    MalformedRecord {
        line: usize,
        source: String,
    },
    UnsupportedSchema {
        line: usize,
        found: String,
    },
    EmptyField {
        line: Option<usize>,
        field: &'static str,
    },
    SequenceGap {
        line: usize,
        expected: u64,
        found: u64,
    },
    PreviousDigestMismatch {
        line: usize,
        expected: Option<String>,
        found: Option<String>,
    },
    RecordDigestMismatch {
        line: usize,
        expected: String,
        found: String,
    },
    DuplicateRecordId {
        line: usize,
        record_id: StableId,
    },
    ProjectMismatch {
        line: Option<usize>,
        expected: StableId,
        found: StableId,
    },
    BundleMismatch {
        line: Option<usize>,
        expected_id: StableId,
        found_id: StableId,
        expected_digest: String,
        found_digest: String,
    },
    StateVersionRegression {
        previous: u64,
        found: u64,
    },
    ReleaseUpgradeRequiresDedicatedAuthority,
    PostBuildVerifyEpisodeRequiresDedicatedAuthority,
    PostBuildVerifyEpisodeInvalid {
        reason: &'static str,
    },
    CoordinationStateRequiresDedicatedAuthority,
    CoordinationStateInvalid {
        reason: &'static str,
    },
    DomainPackTransitionRequiresDedicatedAuthority,
    HumanIntentRevisionRequiresBrokerAuthority,
    LegacySoloAdoptionRequiresDedicatedAuthority,
    LegacySoloAdoptionInvalid {
        line: Option<usize>,
        reason: &'static str,
    },
    CooperativeObjectiveRequiresDedicatedAuthority,
    CooperativeObjectiveInvalid {
        line: Option<usize>,
        reason: &'static str,
    },
    CooperativeEvidenceRequiresDedicatedAuthority,
    CooperativeEvidenceInvalid {
        line: Option<usize>,
        reason: &'static str,
    },
    WorkFocusRequiresDedicatedAuthority,
    WorkFocusInvalid {
        line: Option<usize>,
        reason: &'static str,
    },
    InvalidBrokerActionBinding {
        reason: &'static str,
    },
    InvalidBrokerOriginBinding {
        line: Option<usize>,
        reason: &'static str,
    },
    ReleaseTransitionStateVersionMismatch {
        expected: u64,
        found: u64,
    },
    ReleaseTransitionInvalid {
        reason: &'static str,
    },
    DuplicateReleaseTransition,
    DomainPackTransitionStateVersionMismatch {
        expected: u64,
        found: u64,
    },
    DomainPackTransitionInvalid {
        reason: &'static str,
    },
    DuplicateDomainPackTransition,
    FirstEventNotProjectImported,
    ProjectImportedAfterInitialization,
    AlreadyInitialized,
    NotInitialized,
    HeadMismatch {
        expected: String,
        actual: String,
    },
    SequenceOverflow {
        current: u64,
    },
    StateVersionOverflow {
        current: u64,
    },
    EmptyBatch,
    Canonicalization {
        source: String,
    },
    Randomness {
        source: String,
    },
    Clock {
        source: String,
    },
}

impl fmt::Display for WorkflowGovernanceLedgerError {
    // Keeping every wire-visible diagnostic in one exhaustive match makes
    // omissions compiler-visible when the error enum evolves.
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateRootUnavailable { path, source } => {
                write!(formatter, "trusted state root {} unavailable: {source}", path.display())
            }
            Self::Lock { source } => write!(formatter, "workflow-governance lock failed: {source}"),
            Self::Io { path, source } => write!(formatter, "ledger I/O {} failed: {source}", path.display()),
            Self::CapacityBytes { found, maximum } => {
                write!(formatter, "ledger byte capacity exceeded: {found} > {maximum}")
            }
            Self::CapacityRecords { found, maximum } => {
                write!(formatter, "ledger record capacity exceeded: {found} > {maximum}")
            }
            Self::TornTail { line } => write!(formatter, "ledger line {line} has a torn tail"),
            Self::BlankLine { line } => write!(formatter, "ledger line {line} is blank"),
            Self::MalformedRecord { line, source } => {
                write!(formatter, "ledger line {line} is malformed: {source}")
            }
            Self::UnsupportedSchema { line, found } => {
                write!(formatter, "ledger line {line} uses unsupported schema {found}")
            }
            Self::EmptyField { line, field } => match line {
                Some(line) => write!(formatter, "ledger line {line} has blank {field}"),
                None => write!(formatter, "ledger input has blank {field}"),
            },
            Self::SequenceGap { line, expected, found } => write!(
                formatter,
                "ledger line {line} sequence gap: expected {expected}, found {found}"
            ),
            Self::PreviousDigestMismatch { line, expected, found } => write!(
                formatter,
                "ledger line {line} previous digest mismatch: expected {expected:?}, found {found:?}"
            ),
            Self::RecordDigestMismatch { line, expected, found } => write!(
                formatter,
                "ledger line {line} record digest mismatch: expected {expected}, found {found}"
            ),
            Self::DuplicateRecordId { line, record_id } => {
                write!(formatter, "ledger line {line} duplicates record id {}", record_id.0)
            }
            Self::ProjectMismatch { line, expected, found } => write!(
                formatter,
                "ledger{} project mismatch: expected {}, found {}",
                line.map_or_else(String::new, |value| format!(" line {value}")),
                expected.0,
                found.0,
            ),
            Self::BundleMismatch { line, expected_id, found_id, expected_digest, found_digest } => write!(
                formatter,
                "ledger{} bundle mismatch: expected {}/{expected_digest}, found {}/{found_digest}",
                line.map_or_else(String::new, |value| format!(" line {value}")),
                expected_id.0,
                found_id.0,
            ),
            Self::StateVersionRegression { previous, found } => write!(
                formatter,
                "ledger state version regressed from {previous} to {found}"
            ),
            Self::ReleaseUpgradeRequiresDedicatedAuthority => write!(
                formatter,
                "release_upgraded requires the dedicated TCB transition API"
            ),
            Self::PostBuildVerifyEpisodeRequiresDedicatedAuthority => write!(
                formatter,
                "post_build_verify_episode_applied requires the dedicated TCB admission API"
            ),
            Self::PostBuildVerifyEpisodeInvalid { reason } => {
                write!(formatter, "post-BuildVerify episode route is invalid: {reason}")
            }
            Self::CoordinationStateRequiresDedicatedAuthority => write!(
                formatter,
                "coordination_state_applied requires the dedicated kernel/TCB API"
            ),
            Self::CoordinationStateInvalid { reason } => {
                write!(formatter, "coordination state is invalid: {reason}")
            }
            Self::DomainPackTransitionRequiresDedicatedAuthority => write!(
                formatter,
                "domain_pack_generation_transitioned requires the dedicated TCB transition API"
            ),
            Self::HumanIntentRevisionRequiresBrokerAuthority => write!(
                formatter,
                "human_intent_revision_accepted requires verified broker authority"
            ),
            Self::LegacySoloAdoptionRequiresDedicatedAuthority => write!(
                formatter,
                "legacy_solo_profile_adopted requires the dedicated kernel/TCB API"
            ),
            Self::LegacySoloAdoptionInvalid { line, reason } => write!(
                formatter,
                "legacy Solo Cooperative adoption{} is invalid: {reason}",
                line.map_or_else(String::new, |value| format!(" at ledger line {value}")),
            ),
            Self::CooperativeObjectiveRequiresDedicatedAuthority => write!(
                formatter,
                "cooperative_objective_accepted requires the dedicated same-owner TCB API"
            ),
            Self::CooperativeObjectiveInvalid { line, reason } => write!(
                formatter,
                "cooperative objective{} is invalid: {reason}",
                line.map_or_else(String::new, |value| format!(" at ledger line {value}")),
            ),
            Self::CooperativeEvidenceRequiresDedicatedAuthority => write!(
                formatter,
                "cooperative evidence requires the dedicated same-owner TCB API"
            ),
            Self::CooperativeEvidenceInvalid { line, reason } => write!(
                formatter,
                "cooperative evidence{} is invalid: {reason}",
                line.map_or_else(String::new, |value| format!(" at ledger line {value}")),
            ),
            Self::WorkFocusRequiresDedicatedAuthority => write!(
                formatter,
                "work_focus_recorded requires the dedicated continuity TCB API"
            ),
            Self::WorkFocusInvalid { line, reason } => write!(
                formatter,
                "Work Focus{} is invalid: {reason}",
                line.map_or_else(String::new, |value| format!(" at ledger line {value}")),
            ),
            Self::InvalidBrokerActionBinding { reason } => {
                write!(formatter, "verified broker action binding is invalid: {reason}")
            }
            Self::InvalidBrokerOriginBinding { line, reason } => write!(
                formatter,
                "broker origin{} binding is invalid: {reason}",
                line.map_or_else(String::new, |value| format!(" at ledger line {value}")),
            ),
            Self::ReleaseTransitionStateVersionMismatch { expected, found } => write!(
                formatter,
                "release transition state version mismatch: expected {expected}, found {found}"
            ),
            Self::ReleaseTransitionInvalid { reason } => {
                write!(formatter, "release transition is structurally invalid: {reason}")
            }
            Self::DuplicateReleaseTransition => write!(
                formatter,
                "a release transition batch must contain exactly one transition"
            ),
            Self::DomainPackTransitionStateVersionMismatch { expected, found } => write!(
                formatter,
                "Domain Pack transition state version mismatch: expected {expected}, found {found}"
            ),
            Self::DomainPackTransitionInvalid { reason } => write!(
                formatter,
                "Domain Pack transition is structurally invalid: {reason}"
            ),
            Self::DuplicateDomainPackTransition => write!(
                formatter,
                "a Domain Pack transition batch must contain exactly one transition"
            ),
            Self::FirstEventNotProjectImported => write!(formatter, "first ledger event must be project_imported"),
            Self::ProjectImportedAfterInitialization => write!(formatter, "project_imported may only be the first ledger event"),
            Self::AlreadyInitialized => write!(formatter, "workflow-governance ledger is already initialized"),
            Self::NotInitialized => write!(formatter, "workflow-governance ledger is not initialized"),
            Self::HeadMismatch { expected, actual } => write!(
                formatter,
                "workflow-governance head CAS failed: expected {expected}, actual {actual}"
            ),
            Self::SequenceOverflow { current } => write!(formatter, "ledger sequence overflow after {current}"),
            Self::StateVersionOverflow { current } => write!(formatter, "ledger state version overflow after {current}"),
            Self::EmptyBatch => write!(formatter, "workflow-governance batch has no events"),
            Self::Canonicalization { source } => write!(formatter, "canonical ledger encoding failed: {source}"),
            Self::Randomness { source } => write!(formatter, "record id generation failed: {source}"),
            Self::Clock { source } => write!(formatter, "record timestamp failed: {source}"),
        }
    }
}

impl std::error::Error for WorkflowGovernanceLedgerError {}

/// Acquire the fixed workflow-governance lock below a trusted state root.
///
/// # Errors
///
/// Returns an error if `state_root` is not an existing directory or the
/// exclusive lock cannot be acquired.
fn lock_workflow_governance_ledger_internal(
    state_root: impl AsRef<Path>,
) -> Result<LockedWorkflowGovernanceLedger, WorkflowGovernanceLedgerError> {
    let state_root = trusted_state_root(state_root.as_ref())?;
    let lock = acquire_effect_store_lock(&state_root, WORKFLOW_GOVERNANCE_LOCK_RELATIVE_PATH)
        .map_err(lock_error)?;
    Ok(LockedWorkflowGovernanceLedger { state_root, lock })
}

/// Acquire the workflow ledger mutation lock inside the dedicated
/// workflow-governance TCB plus kernel boundary.
///
/// This crate is intentionally a direct dependency only of `forge-core-kernel`.
/// The API does not authenticate semantic event authority; only the kernel
/// Adapter may call it after its own checks.
#[doc(hidden)]
pub fn lock_workflow_governance_ledger_tcb(
    state_root: impl AsRef<Path>,
) -> Result<LockedWorkflowGovernanceLedger, WorkflowGovernanceLedgerError> {
    lock_workflow_governance_ledger_internal(state_root)
}

/// Acquire the pre-existing workflow ledger lock without creating lock state.
///
/// Read-only sidecars use this instead of the mutation constructor so a missing
/// ledger/lock remains observably absent.
#[doc(hidden)]
pub fn observe_existing_workflow_governance_ledger(
    state_root: impl AsRef<Path>,
) -> Result<LockedWorkflowGovernanceLedger, WorkflowGovernanceLedgerError> {
    let state_root = trusted_state_root(state_root.as_ref())?;
    let lock =
        acquire_existing_effect_store_lock(&state_root, WORKFLOW_GOVERNANCE_LOCK_RELATIVE_PATH)
            .map_err(lock_error)?;
    Ok(LockedWorkflowGovernanceLedger { state_root, lock })
}

/// Strictly recover the ledger under its exclusive lock.
///
/// # Errors
///
/// Forwards lock and recovery failures.
pub fn recover_workflow_governance_ledger(
    state_root: impl AsRef<Path>,
) -> Result<WorkflowGovernanceLedgerProjection, WorkflowGovernanceLedgerError> {
    lock_workflow_governance_ledger_internal(state_root)?.recover()
}

/// Initialize a ledger in a single exclusive-lock scope.
///
/// # Errors
///
/// Forwards lock, validation, recovery, and durable append failures.
#[doc(hidden)]
pub fn initialize_workflow_governance_ledger_tcb(
    state_root: impl AsRef<Path>,
    identity: &WorkflowGovernanceLedgerIdentity,
    state_version: u64,
    event: WorkflowGovernanceEvent,
) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
    lock_workflow_governance_ledger_tcb(state_root)?.initialize_unchecked_tcb(
        identity,
        state_version,
        event,
    )
}

/// Append an event with expected-head CAS in one exclusive-lock scope.
///
/// # Errors
///
/// Forwards lock, validation, recovery, CAS, and durable append failures.
#[doc(hidden)]
pub fn append_workflow_governance_event_tcb(
    state_root: impl AsRef<Path>,
    expected_head_digest: &str,
    identity: &WorkflowGovernanceLedgerIdentity,
    state_version: u64,
    event: WorkflowGovernanceEvent,
) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
    lock_workflow_governance_ledger_tcb(state_root)?.append_unchecked_tcb_event(
        expected_head_digest,
        identity,
        state_version,
        event,
    )
}

/// Transition the active release with expected-head CAS in one lock scope.
///
/// The kernel remains responsible for registry admission and predecessor
/// authorization before calling this structural TCB boundary.
///
/// # Errors
///
/// Forwards lock, recovery, CAS, structural validation, and commit failures.
#[doc(hidden)]
pub fn transition_workflow_governance_release_tcb(
    state_root: impl AsRef<Path>,
    expected_head_digest: &str,
    source_identity: &WorkflowGovernanceLedgerIdentity,
    target_identity: &WorkflowGovernanceLedgerIdentity,
    state_version: u64,
    event: ReleaseUpgradedEvent,
) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
    lock_workflow_governance_ledger_tcb(state_root)?.transition_release_unchecked_tcb(
        expected_head_digest,
        source_identity,
        target_identity,
        state_version,
        event,
    )
}

/// Transition the active Domain Pack effective-bundle epoch with expected-head
/// CAS in one workflow-ledger lock scope.
///
/// The kernel remains responsible for consuming the opaque active-generation
/// admission and deriving both effective identities before this structural
/// boundary is called.
#[doc(hidden)]
pub fn transition_workflow_domain_pack_generation_tcb(
    state_root: impl AsRef<Path>,
    expected_head_digest: &str,
    identity: &WorkflowGovernanceLedgerIdentity,
    state_version: u64,
    event: DomainPackGenerationTransitionedEvent,
) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
    lock_workflow_governance_ledger_tcb(state_root)?
        .transition_domain_pack_generation_unchecked_tcb(
            expected_head_digest,
            identity,
            state_version,
            event,
        )
}

/// Compute the canonical JCS digest of a record with `record_digest` blanked.
///
/// # Errors
///
/// Returns an error if canonical JSON encoding fails.
pub fn workflow_governance_record_digest(
    record: &WorkflowGovernanceLedgerRecord,
) -> Result<String, WorkflowGovernanceLedgerError> {
    let mut digest_input = record.clone();
    digest_input.record_digest.clear();
    let canonical = to_canonical_json(&digest_input).map_err(|error| {
        WorkflowGovernanceLedgerError::Canonicalization {
            source: error.to_string(),
        }
    })?;
    Ok(format_sha256(Sha256::digest(canonical)))
}

// Keeping the strict parser as one linear pass makes the sequence, digest,
// identity, and state-version invariants auditable in their exact wire order.
#[allow(clippy::too_many_lines)]
fn recover_under_lock(
    state_root: &Path,
) -> Result<WorkflowGovernanceLedgerProjection, WorkflowGovernanceLedgerError> {
    let wal_path = workflow_governance_wal_path(state_root)?;
    reconcile_wal_replacement(&wal_path).map_err(|source| io_error(&wal_path, source))?;
    if !wal_path.exists() {
        return Ok(empty_projection());
    }
    let metadata = fs::metadata(&wal_path).map_err(|source| io_error(&wal_path, source))?;
    if metadata.len() > WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES {
        return Err(WorkflowGovernanceLedgerError::CapacityBytes {
            found: metadata.len(),
            maximum: WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES,
        });
    }

    let file = File::open(&wal_path).map_err(|source| io_error(&wal_path, source))?;
    recover_from_reader(BufReader::new(file), &wal_path)
}

#[allow(clippy::too_many_lines)]
fn recover_from_reader(
    mut reader: impl BufRead,
    wal_path: &Path,
) -> Result<WorkflowGovernanceLedgerProjection, WorkflowGovernanceLedgerError> {
    let mut records = Vec::new();
    let mut line_bytes = Vec::new();
    let mut ids = HashSet::new();
    let mut expected_previous: Option<String> = None;
    let mut identity_state = RecoveredIdentityState::default();
    let mut previous_state_version: Option<u64> = None;

    loop {
        line_bytes.clear();
        let read = reader
            .read_until(b'\n', &mut line_bytes)
            .map_err(|source| io_error(wal_path, source))?;
        if read == 0 {
            break;
        }
        let line_number = records.len() + 1;
        if !line_bytes.ends_with(b"\n") {
            return Err(WorkflowGovernanceLedgerError::TornTail { line: line_number });
        }
        line_bytes.pop();
        if line_bytes.last() == Some(&b'\r') {
            line_bytes.pop();
        }
        if line_bytes.iter().all(u8::is_ascii_whitespace) {
            return Err(WorkflowGovernanceLedgerError::BlankLine { line: line_number });
        }
        if line_number > WORKFLOW_GOVERNANCE_LEDGER_MAX_RECORDS {
            return Err(WorkflowGovernanceLedgerError::CapacityRecords {
                found: line_number,
                maximum: WORKFLOW_GOVERNANCE_LEDGER_MAX_RECORDS,
            });
        }
        let raw_document: serde_json::Value =
            serde_json::from_slice(&line_bytes).map_err(|error| {
                WorkflowGovernanceLedgerError::MalformedRecord {
                    line: line_number,
                    source: error.to_string(),
                }
            })?;
        if raw_document
            .get("schema_version")
            .and_then(serde_json::Value::as_str)
            == Some(WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_LEDGER_SCHEMA_VERSION)
            && raw_cooperative_objective_revision_metadata_present(&raw_document)
        {
            return Err(WorkflowGovernanceLedgerError::MalformedRecord {
                line: line_number,
                source:
                    "frozen 0.10 cooperative objective records must omit revision metadata entirely"
                        .to_owned(),
            });
        }
        let readiness_profile_field_present =
            raw_project_import_readiness_profile_field_present(&raw_document);
        let document: WorkflowGovernanceReceiptDocument = serde_json::from_value(raw_document)
            .map_err(|error| WorkflowGovernanceLedgerError::MalformedRecord {
                line: line_number,
                source: error.to_string(),
            })?;
        if document.schema_version != WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION
            && document.schema_version != WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION
            && document.schema_version != WORKFLOW_GOVERNANCE_INTENT_LEDGER_SCHEMA_VERSION
            && document.schema_version != WORKFLOW_GOVERNANCE_REBASE_LEDGER_SCHEMA_VERSION
            && document.schema_version != WORKFLOW_GOVERNANCE_HOST_ORIGIN_LEDGER_SCHEMA_VERSION
            && document.schema_version != WORKFLOW_GOVERNANCE_STRICT_REPLAY_LEDGER_SCHEMA_VERSION
            && document.schema_version
                != WORKFLOW_GOVERNANCE_POST_BUILD_VERIFY_LEDGER_SCHEMA_VERSION
            && document.schema_version
                != WORKFLOW_GOVERNANCE_REPLACEMENT_CONTINUITY_LEDGER_SCHEMA_VERSION
            && document.schema_version
                != WORKFLOW_GOVERNANCE_READINESS_PROFILE_LEDGER_SCHEMA_VERSION
            && document.schema_version
                != WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_LEDGER_SCHEMA_VERSION
            && document.schema_version
                != WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_REVISION_LEDGER_SCHEMA_VERSION
            && document.schema_version
                != WORKFLOW_GOVERNANCE_COOPERATIVE_EVIDENCE_LEDGER_SCHEMA_VERSION
            && document.schema_version
                != WORKFLOW_GOVERNANCE_LEGACY_SOLO_ADOPTION_LEDGER_SCHEMA_VERSION
            && document.schema_version != WORKFLOW_GOVERNANCE_PRIOR_EVIDENCE_LEDGER_SCHEMA_VERSION
            && document.schema_version != WORKFLOW_GOVERNANCE_CURRENT_WORK_LEDGER_SCHEMA_VERSION
            && document.schema_version
                != WORKFLOW_GOVERNANCE_WORK_FOCUS_BINDINGS_LEDGER_SCHEMA_VERSION
            && document.schema_version != WORKFLOW_GOVERNANCE_QUICK_CYCLE_LEDGER_SCHEMA_VERSION
        {
            return Err(WorkflowGovernanceLedgerError::UnsupportedSchema {
                line: line_number,
                found: document.schema_version,
            });
        }
        let record = document.workflow_governance_receipt;
        let is_legacy_solo_adoption = matches!(
            &record.event,
            WorkflowGovernanceEvent::LegacySoloProfileAdopted(_)
        );
        let is_readiness_profile_genesis = matches!(
            &record.event,
            WorkflowGovernanceEvent::ProjectImported(imported)
                if imported.readiness_profile.is_some()
        );
        if readiness_profile_field_present && !is_readiness_profile_genesis {
            return Err(WorkflowGovernanceLedgerError::MalformedRecord {
                line: line_number,
                source: "project_imported readiness_profile must be a non-null closed value"
                    .to_owned(),
            });
        }
        let is_domain_transition = matches!(
            &record.event,
            WorkflowGovernanceEvent::DomainPackGenerationTransitioned(_)
                | WorkflowGovernanceEvent::CoreDomainPackRebased(_)
        );
        let is_intent_revision = matches!(
            &record.event,
            WorkflowGovernanceEvent::HumanIntentRevisionAccepted(_)
        );
        let is_cooperative_objective = matches!(
            &record.event,
            WorkflowGovernanceEvent::CooperativeObjectiveAccepted(_)
        );
        let is_cooperative_objective_revision = matches!(
            &record.event,
            WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event) if event.revision > 1
        );
        let is_cooperative_evidence = matches!(
            &record.event,
            WorkflowGovernanceEvent::CooperativeEvidenceObserved(_)
        );
        let is_prior_cooperative_evidence =
            event_contains_prior_cooperative_evidence(&record.event);
        let work_focus_wire_level = work_focus_wire_level(&record.event);
        let is_work_focus = work_focus_wire_level >= 1;
        let is_work_focus_bindings = work_focus_wire_level >= 2;
        let is_quick_cycle = work_focus_wire_level == 3;
        let is_native_host_origin = matches!(
            &record.event,
            WorkflowGovernanceEvent::BrokerOriginApplied(event)
                if event.native_host_provenance.is_some()
        );
        let is_strict_replay_origin = matches!(
            &record.event,
            WorkflowGovernanceEvent::BrokerOriginApplied(event)
                if event.native_interaction_replay_digest.is_some()
        );
        let is_post_build_verify_episode = matches!(
            record.event,
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(_)
        );
        let is_replacement_continuity = match &record.event {
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event) => {
                event.episode_snapshot.is_some()
            }
            WorkflowGovernanceEvent::CoordinationStateApplied(_) => true,
            _ => false,
        };
        let rebase_wire_required = matches!(
            &record.event,
            WorkflowGovernanceEvent::CoreDomainPackRebased(_)
        ) || identity_state.rebase_seen;
        let intent_wire_required = is_intent_revision || identity_state.intent_revision_seen;
        let effective_wire_required =
            is_domain_transition || identity_state.active_effective.is_some();
        let expected_schema = if is_quick_cycle || identity_state.quick_cycle_seen {
            WORKFLOW_GOVERNANCE_QUICK_CYCLE_LEDGER_SCHEMA_VERSION
        } else if is_work_focus_bindings || identity_state.work_focus_bindings_seen {
            WORKFLOW_GOVERNANCE_WORK_FOCUS_BINDINGS_LEDGER_SCHEMA_VERSION
        } else if is_work_focus || identity_state.work_focus_seen {
            WORKFLOW_GOVERNANCE_CURRENT_WORK_LEDGER_SCHEMA_VERSION
        } else if is_prior_cooperative_evidence || identity_state.prior_cooperative_evidence_seen {
            WORKFLOW_GOVERNANCE_PRIOR_EVIDENCE_LEDGER_SCHEMA_VERSION
        } else if is_legacy_solo_adoption || identity_state.legacy_solo_adoption_seen {
            WORKFLOW_GOVERNANCE_LEGACY_SOLO_ADOPTION_LEDGER_SCHEMA_VERSION
        } else if is_cooperative_evidence || identity_state.cooperative_evidence_seen {
            WORKFLOW_GOVERNANCE_COOPERATIVE_EVIDENCE_LEDGER_SCHEMA_VERSION
        } else if is_cooperative_objective_revision
            || identity_state.cooperative_objective_revision_seen
        {
            WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_REVISION_LEDGER_SCHEMA_VERSION
        } else if is_cooperative_objective || identity_state.cooperative_objective_seen {
            WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_LEDGER_SCHEMA_VERSION
        } else if is_readiness_profile_genesis || identity_state.readiness_profile_epoch_seen {
            WORKFLOW_GOVERNANCE_READINESS_PROFILE_LEDGER_SCHEMA_VERSION
        } else if is_replacement_continuity || identity_state.replacement_continuity_seen {
            WORKFLOW_GOVERNANCE_REPLACEMENT_CONTINUITY_LEDGER_SCHEMA_VERSION
        } else if is_post_build_verify_episode || identity_state.post_build_verify_episode_seen {
            WORKFLOW_GOVERNANCE_POST_BUILD_VERIFY_LEDGER_SCHEMA_VERSION
        } else if is_strict_replay_origin || identity_state.strict_replay_origin_seen {
            WORKFLOW_GOVERNANCE_STRICT_REPLAY_LEDGER_SCHEMA_VERSION
        } else if is_native_host_origin || identity_state.native_host_origin_seen {
            WORKFLOW_GOVERNANCE_HOST_ORIGIN_LEDGER_SCHEMA_VERSION
        } else if rebase_wire_required {
            WORKFLOW_GOVERNANCE_REBASE_LEDGER_SCHEMA_VERSION
        } else if intent_wire_required {
            WORKFLOW_GOVERNANCE_INTENT_LEDGER_SCHEMA_VERSION
        } else if effective_wire_required {
            WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION
        } else {
            WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION
        };
        if document.schema_version != expected_schema {
            return Err(WorkflowGovernanceLedgerError::UnsupportedSchema {
                line: line_number,
                found: format!(
                    "{} for event/ledger epoch requiring {expected_schema}",
                    document.schema_version
                ),
            });
        }
        // Strict interaction replay is an authority invariant, not a schema
        // feature flag. Later wire epochs (including cooperative objective
        // 0.10) must never erase it once observed.
        if is_strict_replay_origin || identity_state.strict_replay_origin_seen {
            require_strict_broker_origin_replay_digest(&record.event, Some(line_number))?;
        }
        validate_record_fields(&record, Some(line_number))?;

        let expected_sequence = u64::try_from(line_number).unwrap_or(u64::MAX);
        if record.sequence != expected_sequence {
            return Err(WorkflowGovernanceLedgerError::SequenceGap {
                line: line_number,
                expected: expected_sequence,
                found: record.sequence,
            });
        }
        if record.previous_record_digest != expected_previous {
            return Err(WorkflowGovernanceLedgerError::PreviousDigestMismatch {
                line: line_number,
                expected: expected_previous,
                found: record.previous_record_digest,
            });
        }
        let expected_digest = workflow_governance_record_digest(&record)?;
        if record.record_digest != expected_digest {
            return Err(WorkflowGovernanceLedgerError::RecordDigestMismatch {
                line: line_number,
                expected: expected_digest,
                found: record.record_digest,
            });
        }
        if !ids.insert(record.record_id.clone()) {
            return Err(WorkflowGovernanceLedgerError::DuplicateRecordId {
                line: line_number,
                record_id: record.record_id,
            });
        }
        validate_recovered_semantics(
            &record,
            line_number,
            &mut identity_state,
            previous_state_version,
            &records,
        )?;
        previous_state_version = Some(record.state_version);
        expected_previous = Some(record.record_digest.clone());
        records.push(record);
    }

    let head_digest = expected_previous;
    let next_sequence = u64::try_from(records.len())
        .unwrap_or(u64::MAX)
        .checked_add(1)
        .ok_or(WorkflowGovernanceLedgerError::SequenceOverflow { current: u64::MAX })?;
    let next_state_version = previous_state_version
        .unwrap_or_default()
        .checked_add(u64::from(!records.is_empty()))
        .ok_or(WorkflowGovernanceLedgerError::StateVersionOverflow { current: u64::MAX })?;
    Ok(WorkflowGovernanceLedgerProjection {
        records,
        head_digest,
        next_sequence,
        next_state_version,
    })
}

#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
struct RecoveredIdentityState {
    genesis: Option<WorkflowGovernanceLedgerIdentity>,
    active: Option<WorkflowGovernanceLedgerIdentity>,
    active_release: Option<WorkflowGovernanceReleaseIdentity>,
    active_runtime: Option<WorkflowRuntimeBundleIdentity>,
    active_effective: Option<WorkflowEffectiveBundleIdentity>,
    intent_revision_seen: bool,
    rebase_seen: bool,
    native_host_origin_seen: bool,
    strict_replay_origin_seen: bool,
    post_build_verify_episode_seen: bool,
    replacement_continuity_seen: bool,
    readiness_profile_epoch_seen: bool,
    readiness_profile: Option<WorkflowReadinessProfile>,
    legacy_profileless_genesis: bool,
    legacy_solo_adoption_seen: bool,
    legacy_solo_history_supported: bool,
    genesis_record_digest: Option<String>,
    cooperative_objective_seen: bool,
    cooperative_objective_revision_seen: bool,
    cooperative_evidence_seen: bool,
    prior_cooperative_evidence_seen: bool,
    work_focus_seen: bool,
    work_focus_bindings_seen: bool,
    quick_cycle_seen: bool,
    latest_cooperative_objective: Option<forge_core_contracts::CooperativeObjectiveAcceptedEvent>,
    latest_cooperative_objective_record_digest: Option<String>,
    latest_cooperative_objective_record_sequence: Option<u64>,
    latest_work_focus: Option<WorkflowWorkFocusRecordedEvent>,
    latest_work_focus_record_digest: Option<String>,
    cooperative_offer_by_id: BTreeMap<String, String>,
    current_phase: Option<StableId>,
    last_post_build_verify_episode_by_id: BTreeMap<String, (u64, String)>,
    latest_coordination_request_by_id: BTreeMap<String, CoordinationRequestState>,
}

fn validate_recovered_semantics(
    record: &WorkflowGovernanceLedgerRecord,
    line: usize,
    identity: &mut RecoveredIdentityState,
    previous_state_version: Option<u64>,
    prior_records: &[WorkflowGovernanceLedgerRecord],
) -> Result<(), WorkflowGovernanceLedgerError> {
    if line == 1 {
        let WorkflowGovernanceEvent::ProjectImported(imported) = &record.event else {
            return Err(WorkflowGovernanceLedgerError::FirstEventNotProjectImported);
        };
        let genesis = WorkflowGovernanceLedgerIdentity::from_record(record);
        identity.genesis = Some(genesis.clone());
        identity.active = Some(genesis);
        identity.current_phase = Some(imported.initial_phase.clone());
        identity.readiness_profile_epoch_seen = imported.readiness_profile.is_some();
        identity.readiness_profile = Some(imported.effective_readiness_profile());
        identity.legacy_profileless_genesis = imported.readiness_profile.is_none();
        identity.legacy_solo_history_supported = imported.readiness_profile.is_none();
        identity.genesis_record_digest = Some(record.record_digest.clone());
    } else if matches!(record.event, WorkflowGovernanceEvent::ProjectImported(_)) {
        return Err(WorkflowGovernanceLedgerError::ProjectImportedAfterInitialization);
    }
    if let Some(genesis) = identity.genesis.as_ref() {
        if record.project_id != genesis.project_id {
            return Err(WorkflowGovernanceLedgerError::ProjectMismatch {
                line: Some(line),
                expected: genesis.project_id.clone(),
                found: record.project_id.clone(),
            });
        }
    }
    if let Some(active) = identity.active.as_ref() {
        if record.bundle_id != active.bundle_id || record.bundle_digest != active.bundle_digest {
            return Err(WorkflowGovernanceLedgerError::BundleMismatch {
                line: Some(line),
                expected_id: active.bundle_id.clone(),
                found_id: record.bundle_id.clone(),
                expected_digest: active.bundle_digest.clone(),
                found_digest: record.bundle_digest.clone(),
            });
        }
    }
    if previous_state_version.is_some_and(|previous| record.state_version < previous) {
        return Err(WorkflowGovernanceLedgerError::StateVersionRegression {
            previous: previous_state_version.unwrap_or_default(),
            found: record.state_version,
        });
    }
    validate_recovered_transition_semantics(record, identity, previous_state_version)?;
    if let WorkflowGovernanceEvent::LegacySoloProfileAdopted(event) = &record.event {
        let expected_state_version = previous_state_version
            .and_then(|value| value.checked_add(1))
            .ok_or(WorkflowGovernanceLedgerError::LegacySoloAdoptionInvalid {
                line: Some(line),
                reason: "adoption must follow initialized legacy history",
            })?;
        if !identity.legacy_profileless_genesis
            || identity.legacy_solo_adoption_seen
            || !identity.legacy_solo_history_supported
            || identity.readiness_profile != Some(WorkflowReadinessProfile::StrictExternal)
            || record.state_version != expected_state_version
            || record.previous_record_digest.as_deref()
                != Some(event.prior_ledger_head_digest.as_str())
            || identity.genesis_record_digest.as_deref()
                != Some(event.legacy_project_import_record_digest.as_str())
            || event.authority_basis != WorkflowCooperativeAuthorityBasis::CooperativeSameOwner
        {
            return Err(WorkflowGovernanceLedgerError::LegacySoloAdoptionInvalid {
                line: Some(line),
                reason:
                    "transition is not the single exact legacy profile-less same-owner adoption",
            });
        }
        identity.readiness_profile = Some(WorkflowReadinessProfile::SoloCooperative);
        identity.legacy_solo_adoption_seen = true;
        identity.readiness_profile_epoch_seen = true;
    }
    if let WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event) = &record.event {
        if identity.readiness_profile != Some(WorkflowReadinessProfile::SoloCooperative) {
            return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                line: Some(line),
                reason: "same-owner objective requires the solo_cooperative readiness profile",
            });
        }
        if identity.intent_revision_seen {
            return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                line: Some(line),
                reason: "cooperative objective cannot follow strict human intent authority",
            });
        }
        validate_cooperative_objective_transition(
            identity.latest_cooperative_objective.as_ref(),
            event,
            Some(line),
        )?;
        if record.previous_record_digest.as_deref() != Some(event.ledger_head_digest.as_str()) {
            return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                line: Some(line),
                reason: "objective ledger-head binding does not match its predecessor",
            });
        }
        if record.recorded_at_unix != event.accepted_at_unix {
            return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                line: Some(line),
                reason: "objective commit time does not match the ledger record",
            });
        }
    }
    if let WorkflowGovernanceEvent::CooperativeEvidenceObserved(event) = &record.event {
        if identity.readiness_profile != Some(WorkflowReadinessProfile::SoloCooperative) {
            return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                line: Some(line),
                reason: "same-owner evidence requires the solo_cooperative readiness profile",
            });
        }
        if identity.latest_cooperative_objective.is_none() {
            return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                line: Some(line),
                reason: "same-owner evidence requires an accepted cooperative objective",
            });
        }
        if record.previous_record_digest.as_deref()
            != Some(event.admission_ledger_head_digest.as_str())
            || record.state_version != event.admission_state_version
            || record.recorded_at_unix != event.observed_at_unix
        {
            return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                line: Some(line),
                reason: "evidence admission coordinates do not match its ledger record",
            });
        }
        if let Some(offer_id) = event.offer_id.as_ref() {
            match identity.cooperative_offer_by_id.get(&offer_id.0) {
                Some(original_digest)
                    if event.disposition
                        != forge_core_contracts::WorkflowCooperativeEvidenceDisposition::Rejected
                        || event.rejection
                            != Some(
                                forge_core_contracts::WorkflowCooperativeEvidenceRejection::ConflictingIdempotencyKey,
                            )
                        || original_digest == &event.offer_digest =>
                {
                    return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                        line: Some(line),
                        reason: "reused cooperative evidence offer id is not a distinct rejected conflict",
                    });
                }
                None if event.rejection
                    == Some(
                        forge_core_contracts::WorkflowCooperativeEvidenceRejection::ConflictingIdempotencyKey,
                    ) =>
                {
                    return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                        line: Some(line),
                        reason: "cooperative evidence conflict has no original offer id",
                    });
                }
                Some(_) | None => {}
            }
        }
        if let Some(admitted) = event.admitted_evidence.as_ref() {
            let Some(objective) = identity.latest_cooperative_objective.as_ref() else {
                return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                    line: Some(line),
                    reason: "admitted evidence has no active objective",
                });
            };
            if admitted.binding.objective_id != objective.objective_id
                || admitted.binding.objective_revision != objective.revision
                || admitted.binding.objective_digest != objective.objective_digest
                || admitted.binding.assurance_epoch != objective.assurance_epoch
                || Some(admitted.binding.accepted_objective_record_digest.as_str())
                    != identity
                        .latest_cooperative_objective_record_digest
                        .as_deref()
                || Some(admitted.binding.accepted_objective_record_sequence)
                    != identity.latest_cooperative_objective_record_sequence
                || admitted.binding.ledger_head_digest != event.admission_ledger_head_digest
                || admitted.binding.state_version != event.admission_state_version
                || admitted.binding.snapshot_digest != event.admission_snapshot_digest
                || identity
                    .active_effective
                    .as_ref()
                    .map(|effective| effective.effective_runtime_bundle.bundle_digest.as_str())
                    .or_else(|| {
                        identity
                            .active
                            .as_ref()
                            .map(|active| active.bundle_digest.as_str())
                    })
                    != Some(admitted.binding.policy_bundle_digest.as_str())
            {
                return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                    line: Some(line),
                    reason: "admitted evidence is not fully bound to the latest objective and admission coordinates",
                });
            }
        }
        validate_cooperative_evidence_shape(event, Some(line))?;
    }
    if let WorkflowGovernanceEvent::WorkFocusRecorded(event) = &record.event {
        validate_recovered_work_focus_event(record, line, identity, prior_records, event)?;
    }
    if matches!(
        &record.event,
        WorkflowGovernanceEvent::HumanIntentRevisionAccepted(_)
    ) {
        identity.intent_revision_seen = true;
    }
    if let WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event) = &record.event {
        identity.cooperative_objective_seen = true;
        identity.cooperative_objective_revision_seen |= event.revision > 1;
        identity.latest_cooperative_objective = Some(event.clone());
        identity.latest_cooperative_objective_record_digest = Some(record.record_digest.clone());
        identity.latest_cooperative_objective_record_sequence = Some(record.sequence);
    }
    if matches!(
        &record.event,
        WorkflowGovernanceEvent::CooperativeEvidenceObserved(_)
    ) {
        identity.cooperative_evidence_seen = true;
    }
    if let WorkflowGovernanceEvent::WorkFocusRecorded(event) = &record.event {
        identity.work_focus_seen = true;
        identity.work_focus_bindings_seen |=
            !event.blocker_record_digests.is_empty() || !event.evidence_record_digests.is_empty();
        identity.quick_cycle_seen |= event.quick_cycle.is_some();
        identity.latest_work_focus = Some(event.clone());
        identity.latest_work_focus_record_digest = Some(record.record_digest.clone());
    }
    identity.prior_cooperative_evidence_seen |=
        event_contains_prior_cooperative_evidence(&record.event);
    if let WorkflowGovernanceEvent::CooperativeEvidenceObserved(event) = &record.event {
        if let Some(offer_id) = event.offer_id.as_ref() {
            identity
                .cooperative_offer_by_id
                .entry(offer_id.0.clone())
                .or_insert_with(|| event.offer_digest.clone());
        }
    }
    if matches!(
        &record.event,
        WorkflowGovernanceEvent::CoreDomainPackRebased(_)
    ) {
        identity.rebase_seen = true;
    }
    if matches!(
        &record.event,
        WorkflowGovernanceEvent::BrokerOriginApplied(event)
            if event.native_host_provenance.is_some()
    ) {
        identity.native_host_origin_seen = true;
    }
    if matches!(
        &record.event,
        WorkflowGovernanceEvent::BrokerOriginApplied(event)
            if event.native_interaction_replay_digest.is_some()
    ) {
        identity.strict_replay_origin_seen = true;
    }
    if matches!(
        &record.event,
        WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(_)
    ) {
        identity.post_build_verify_episode_seen = true;
    }
    if matches!(
        &record.event,
        WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event)
            if event.episode_snapshot.is_some()
    ) || matches!(
        &record.event,
        WorkflowGovernanceEvent::CoordinationStateApplied(_)
    ) {
        identity.replacement_continuity_seen = true;
    }
    if !identity.legacy_solo_adoption_seen
        && !matches!(
            record.event,
            WorkflowGovernanceEvent::ProjectImported(_)
                | WorkflowGovernanceEvent::ReleaseUpgraded(_)
        )
    {
        identity.legacy_solo_history_supported = false;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // Release, pack, and joined replay stay in one linear state transition audit.
fn validate_recovered_transition_semantics(
    record: &WorkflowGovernanceLedgerRecord,
    identity: &mut RecoveredIdentityState,
    previous_state_version: Option<u64>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if let WorkflowGovernanceEvent::ReleaseUpgraded(event) = &record.event {
        if identity.active_effective.is_some() {
            return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
                reason: "active Domain Pack generation requires an explicit core rebase",
            });
        }
        let previous = previous_state_version.ok_or(
            WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
                reason: "release transition cannot be the genesis record",
            },
        )?;
        let expected = previous
            .checked_add(1)
            .ok_or(WorkflowGovernanceLedgerError::StateVersionOverflow { current: previous })?;
        if record.state_version != expected {
            return Err(
                WorkflowGovernanceLedgerError::ReleaseTransitionStateVersionMismatch {
                    expected,
                    found: record.state_version,
                },
            );
        }
        let source = identity.active.as_ref().ok_or(
            WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
                reason: "release transition has no active source identity",
            },
        )?;
        let target = WorkflowGovernanceLedgerIdentity {
            project_id: source.project_id.clone(),
            bundle_id: event.to_runtime_bundle.bundle_id.clone(),
            bundle_digest: event.to_runtime_bundle.bundle_digest.clone(),
        };
        validate_release_transition(
            event,
            source,
            &target,
            identity.active_release.as_ref(),
            identity.active_runtime.as_ref(),
            record.previous_record_digest.as_deref(),
        )?;
        identity.active = Some(target);
        identity.active_release = Some(event.to_release.clone());
        identity.active_runtime = Some(event.to_runtime_bundle.clone());
    } else if let WorkflowGovernanceEvent::DomainPackGenerationTransitioned(event) = &record.event {
        let previous = previous_state_version.ok_or(
            WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
                reason: "Domain Pack transition cannot be the genesis record",
            },
        )?;
        let expected = previous
            .checked_add(1)
            .ok_or(WorkflowGovernanceLedgerError::StateVersionOverflow { current: previous })?;
        if record.state_version != expected {
            return Err(
                WorkflowGovernanceLedgerError::DomainPackTransitionStateVersionMismatch {
                    expected,
                    found: record.state_version,
                },
            );
        }
        validate_domain_pack_transition(
            event,
            identity.active.as_ref(),
            identity.active_runtime.as_ref(),
            identity.active_effective.as_ref(),
            record.previous_record_digest.as_deref(),
        )?;
        identity.active_effective = Some(event.to_effective_bundle.clone());
    } else if let WorkflowGovernanceEvent::CoreDomainPackRebased(event) = &record.event {
        let previous = previous_state_version.ok_or(
            WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
                reason: "joined rebase cannot be the genesis record",
            },
        )?;
        let expected = previous
            .checked_add(1)
            .ok_or(WorkflowGovernanceLedgerError::StateVersionOverflow { current: previous })?;
        if record.state_version != expected {
            return Err(
                WorkflowGovernanceLedgerError::DomainPackTransitionStateVersionMismatch {
                    expected,
                    found: record.state_version,
                },
            );
        }
        let source = identity.active.as_ref().ok_or(
            WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
                reason: "joined rebase has no active source identity",
            },
        )?;
        let target = WorkflowGovernanceLedgerIdentity {
            project_id: source.project_id.clone(),
            bundle_id: event.release_transition.to_runtime_bundle.bundle_id.clone(),
            bundle_digest: event
                .release_transition
                .to_runtime_bundle
                .bundle_digest
                .clone(),
        };
        validate_core_domain_pack_rebase(
            event,
            source,
            &target,
            identity.active_release.as_ref(),
            identity.active_runtime.as_ref(),
            identity.active_effective.as_ref(),
            record.previous_record_digest.as_deref(),
        )?;
        identity.active = Some(target);
        identity.active_release = Some(event.release_transition.to_release.clone());
        identity.active_runtime = Some(event.release_transition.to_runtime_bundle.clone());
        identity.active_effective = Some(event.to_effective_bundle.clone());
    } else if let WorkflowGovernanceEvent::PhaseAdvanced(event) = &record.event {
        validate_phase_advance_source(event, identity.current_phase.as_ref())?;
        identity.current_phase = Some(event.to_phase.clone());
    } else if let WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event) = &record.event {
        let previous = previous_state_version.ok_or(
            WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                reason: "post-BuildVerify episode route cannot be the genesis record",
            },
        )?;
        let expected = previous
            .checked_add(1)
            .ok_or(WorkflowGovernanceLedgerError::StateVersionOverflow { current: previous })?;
        if record.state_version != expected {
            return Err(
                WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                    reason: "episode route state version is not contiguous",
                },
            );
        }
        validate_post_build_verify_episode_event(
            event,
            identity.current_phase.as_ref(),
            identity.active_release.as_ref(),
            record.previous_record_digest.as_deref(),
            Some(previous),
            identity
                .last_post_build_verify_episode_by_id
                .get(&event.episode_id.0)
                .map(|(generation, digest)| (*generation, digest.as_str())),
            false,
        )?;
        if let Some(to_phase) = event.to_phase.as_ref() {
            identity.current_phase = Some(to_phase.clone());
        }
        identity.last_post_build_verify_episode_by_id.insert(
            event.episode_id.0.clone(),
            (event.generation, event.episode_digest.clone()),
        );
    } else if let WorkflowGovernanceEvent::CoordinationStateApplied(event) = &record.event {
        let previous = previous_state_version.ok_or(
            WorkflowGovernanceLedgerError::CoordinationStateInvalid {
                reason: "coordination update cannot be the genesis record",
            },
        )?;
        let expected = previous
            .checked_add(1)
            .ok_or(WorkflowGovernanceLedgerError::StateVersionOverflow { current: previous })?;
        if record.state_version != expected {
            return Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
                reason: "coordination update state version is not contiguous",
            });
        }
        let previous_request = match &event.state {
            CoordinationStateRecord::Request(state) => identity
                .latest_coordination_request_by_id
                .get(&state.request.request_contract.id.0),
            CoordinationStateRecord::Completion(_) | CoordinationStateRecord::HealthRecovery(_) => {
                None
            }
        };
        validate_coordination_state_event(
            event,
            record.previous_record_digest.as_deref(),
            Some(previous),
            previous_request,
        )?;
        if let CoordinationStateRecord::Request(state) = &event.state {
            identity
                .latest_coordination_request_by_id
                .insert(state.request.request_contract.id.0.clone(), state.clone());
        }
    }
    Ok(())
}

fn write_initial_record_atomically(
    state_root: &Path,
    projection: &WorkflowGovernanceLedgerProjection,
    identity: &WorkflowGovernanceLedgerIdentity,
    state_version: u64,
    event: WorkflowGovernanceEvent,
) -> Result<WorkflowGovernanceLedgerRecord, WorkflowGovernanceLedgerError> {
    let (record, line) = build_record_line(projection, identity, state_version, event)?;
    ensure_append_capacity(state_root, projection, line.len())?;

    replace_wal_atomically(state_root, &line)?;
    Ok(record)
}

fn replace_wal_atomically(
    state_root: &Path,
    content: &[u8],
) -> Result<(), WorkflowGovernanceLedgerError> {
    let wal_path = workflow_governance_wal_path(state_root)?;
    let parent = wal_path
        .parent()
        .ok_or_else(|| WorkflowGovernanceLedgerError::Io {
            path: wal_path.clone(),
            source: "WAL path has no parent".to_owned(),
        })?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    ensure_resolved_parent_within_root(state_root, &wal_path)
        .map_err(|source| io_error(parent, source))?;
    atomic_replace_file(&wal_path, content).map_err(|source| io_error(&wal_path, source))
}

fn build_record_line(
    projection: &WorkflowGovernanceLedgerProjection,
    identity: &WorkflowGovernanceLedgerIdentity,
    state_version: u64,
    event: WorkflowGovernanceEvent,
) -> Result<(WorkflowGovernanceLedgerRecord, Vec<u8>), WorkflowGovernanceLedgerError> {
    build_record_line_at(projection, identity, state_version, event, unix_time()?)
}

fn build_record_line_at(
    projection: &WorkflowGovernanceLedgerProjection,
    identity: &WorkflowGovernanceLedgerIdentity,
    state_version: u64,
    event: WorkflowGovernanceEvent,
    recorded_at_unix: u64,
) -> Result<(WorkflowGovernanceLedgerRecord, Vec<u8>), WorkflowGovernanceLedgerError> {
    validate_broker_origin_replay_digest(&event, None)?;
    if projection.contains_strict_native_interaction_replay_identity() {
        require_strict_broker_origin_replay_digest(&event, None)?;
    }
    let mut record = WorkflowGovernanceLedgerRecord {
        record_id: unique_record_id(&projection.records)?,
        sequence: projection.next_sequence,
        project_id: identity.project_id.clone(),
        bundle_id: identity.bundle_id.clone(),
        bundle_digest: identity.bundle_digest.clone(),
        state_version,
        previous_record_digest: projection.head_digest.clone(),
        record_digest: String::new(),
        recorded_at_unix,
        event,
    };
    record.record_digest = workflow_governance_record_digest(&record)?;
    let document = WorkflowGovernanceReceiptDocument {
        schema_version: ledger_wire_schema(projection, &record.event).to_owned(),
        workflow_governance_receipt: record.clone(),
    };
    let mut line = serde_json::to_vec(&document).map_err(|error| {
        WorkflowGovernanceLedgerError::Canonicalization {
            source: error.to_string(),
        }
    })?;
    line.push(b'\n');
    Ok((record, line))
}

fn raw_project_import_readiness_profile_field_present(document: &serde_json::Value) -> bool {
    document
        .get("workflow_governance_receipt")
        .and_then(|receipt| receipt.get("event"))
        .and_then(serde_json::Value::as_object)
        .is_some_and(|event| {
            event.get("type").and_then(serde_json::Value::as_str) == Some("project_imported")
                && event
                    .get("payload")
                    .and_then(serde_json::Value::as_object)
                    .is_some_and(|payload| payload.contains_key("readiness_profile"))
        })
}

fn raw_cooperative_objective_revision_metadata_present(document: &serde_json::Value) -> bool {
    document
        .get("workflow_governance_receipt")
        .and_then(|receipt| receipt.get("event"))
        .and_then(serde_json::Value::as_object)
        .is_some_and(|event| {
            event.get("type").and_then(serde_json::Value::as_str)
                == Some("cooperative_objective_accepted")
                && event
                    .get("payload")
                    .and_then(serde_json::Value::as_object)
                    .is_some_and(|payload| {
                        ["revision_kind", "revision_reason", "revision_input_digest"]
                            .iter()
                            .any(|field| payload.contains_key(*field))
                    })
        })
}

struct DeterministicBrokerRecordBinding<'a> {
    action_packet_digest: &'a str,
    broker_event_digest: &'a str,
    event_kind: &'static str,
    recorded_at_unix: u64,
}

fn build_deterministic_broker_record_line(
    projection: &WorkflowGovernanceLedgerProjection,
    identity: &WorkflowGovernanceLedgerIdentity,
    state_version: u64,
    event: WorkflowGovernanceEvent,
    binding: &DeterministicBrokerRecordBinding<'_>,
) -> Result<(WorkflowGovernanceLedgerRecord, Vec<u8>), WorkflowGovernanceLedgerError> {
    let identity_basis = serde_json::json!({
        "domain": "forge-method:workflow-broker-action-record:v1",
        "action_packet_digest": binding.action_packet_digest,
        "broker_event_digest": binding.broker_event_digest,
        "event_kind": binding.event_kind,
        "current_head_digest": projection.head_digest,
        "project_id": identity.project_id,
        "state_version": state_version,
    });
    let canonical = to_canonical_json(&identity_basis).map_err(|error| {
        WorkflowGovernanceLedgerError::Canonicalization {
            source: error.to_string(),
        }
    })?;
    let record_id = StableId(format!("wglr-broker-{:x}", Sha256::digest(canonical)));
    let mut record = WorkflowGovernanceLedgerRecord {
        record_id,
        sequence: projection.next_sequence,
        project_id: identity.project_id.clone(),
        bundle_id: identity.bundle_id.clone(),
        bundle_digest: identity.bundle_digest.clone(),
        state_version,
        previous_record_digest: projection.head_digest.clone(),
        record_digest: String::new(),
        recorded_at_unix: binding.recorded_at_unix,
        event,
    };
    record.record_digest = workflow_governance_record_digest(&record)?;
    let document = WorkflowGovernanceReceiptDocument {
        schema_version: ledger_wire_schema(projection, &record.event).to_owned(),
        workflow_governance_receipt: record.clone(),
    };
    let mut line = serde_json::to_vec(&document).map_err(|error| {
        WorkflowGovernanceLedgerError::Canonicalization {
            source: error.to_string(),
        }
    })?;
    line.push(b'\n');
    Ok((record, line))
}

fn event_contains_prior_cooperative_evidence(event: &WorkflowGovernanceEvent) -> bool {
    matches!(
        event,
        WorkflowGovernanceEvent::CooperativeEvidenceObserved(observed)
            if observed
                .admitted_evidence
                .as_ref()
                .and_then(|evidence| evidence.source_assessment.as_ref())
                .is_some_and(|assessment| !assessment.prior_evidence.is_empty())
    )
}

fn work_focus_wire_level(event: &WorkflowGovernanceEvent) -> u8 {
    match event {
        WorkflowGovernanceEvent::WorkFocusRecorded(focus) if focus.quick_cycle.is_some() => 3,
        WorkflowGovernanceEvent::WorkFocusRecorded(focus)
            if !focus.blocker_record_digests.is_empty()
                || !focus.evidence_record_digests.is_empty() =>
        {
            2
        }
        WorkflowGovernanceEvent::WorkFocusRecorded(_) => 1,
        _ => 0,
    }
}

fn ledger_wire_schema(
    projection: &WorkflowGovernanceLedgerProjection,
    event: &WorkflowGovernanceEvent,
) -> &'static str {
    match work_focus_wire_level(event).max(projection.work_focus_wire_level()) {
        3 => return WORKFLOW_GOVERNANCE_QUICK_CYCLE_LEDGER_SCHEMA_VERSION,
        2 => return WORKFLOW_GOVERNANCE_WORK_FOCUS_BINDINGS_LEDGER_SCHEMA_VERSION,
        1 => return WORKFLOW_GOVERNANCE_CURRENT_WORK_LEDGER_SCHEMA_VERSION,
        _ => {}
    }
    if event_contains_prior_cooperative_evidence(event)
        || projection.contains_prior_cooperative_evidence()
    {
        WORKFLOW_GOVERNANCE_PRIOR_EVIDENCE_LEDGER_SCHEMA_VERSION
    } else if matches!(event, WorkflowGovernanceEvent::LegacySoloProfileAdopted(_))
        || projection.contains_legacy_solo_adoption()
    {
        WORKFLOW_GOVERNANCE_LEGACY_SOLO_ADOPTION_LEDGER_SCHEMA_VERSION
    } else if matches!(
        event,
        WorkflowGovernanceEvent::CooperativeEvidenceObserved(_)
    ) || projection.contains_cooperative_evidence()
    {
        WORKFLOW_GOVERNANCE_COOPERATIVE_EVIDENCE_LEDGER_SCHEMA_VERSION
    } else if matches!(
        event,
        WorkflowGovernanceEvent::CooperativeObjectiveAccepted(objective) if objective.revision > 1
    ) || projection.contains_cooperative_objective_revision()
    {
        WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_REVISION_LEDGER_SCHEMA_VERSION
    } else if matches!(
        event,
        WorkflowGovernanceEvent::CooperativeObjectiveAccepted(_)
    ) || projection.contains_cooperative_objective()
    {
        WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_LEDGER_SCHEMA_VERSION
    } else if matches!(
        event,
        WorkflowGovernanceEvent::ProjectImported(imported)
            if imported.readiness_profile.is_some()
    ) || projection.contains_explicit_readiness_profile()
    {
        WORKFLOW_GOVERNANCE_READINESS_PROFILE_LEDGER_SCHEMA_VERSION
    } else if matches!(
        event,
        WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(applied)
            if applied.episode_snapshot.is_some()
    ) || matches!(event, WorkflowGovernanceEvent::CoordinationStateApplied(_))
        || projection.contains_replacement_continuity()
    {
        WORKFLOW_GOVERNANCE_REPLACEMENT_CONTINUITY_LEDGER_SCHEMA_VERSION
    } else if matches!(
        event,
        WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(_)
    ) || projection.contains_post_build_verify_episode()
    {
        WORKFLOW_GOVERNANCE_POST_BUILD_VERIFY_LEDGER_SCHEMA_VERSION
    } else if matches!(
        event,
        WorkflowGovernanceEvent::BrokerOriginApplied(origin)
            if origin.native_interaction_replay_digest.is_some()
    ) || projection.contains_strict_native_interaction_replay_identity()
    {
        WORKFLOW_GOVERNANCE_STRICT_REPLAY_LEDGER_SCHEMA_VERSION
    } else if matches!(
        event,
        WorkflowGovernanceEvent::BrokerOriginApplied(origin)
            if origin.native_host_provenance.is_some()
    ) || projection.contains_native_host_provenance()
    {
        WORKFLOW_GOVERNANCE_HOST_ORIGIN_LEDGER_SCHEMA_VERSION
    } else if matches!(event, WorkflowGovernanceEvent::CoreDomainPackRebased(_))
        || projection.contains_core_domain_pack_rebase()
    {
        WORKFLOW_GOVERNANCE_REBASE_LEDGER_SCHEMA_VERSION
    } else if matches!(
        event,
        WorkflowGovernanceEvent::HumanIntentRevisionAccepted(_)
    ) || projection.contains_human_intent_revision()
    {
        WORKFLOW_GOVERNANCE_INTENT_LEDGER_SCHEMA_VERSION
    } else if matches!(
        event,
        WorkflowGovernanceEvent::DomainPackGenerationTransitioned(_)
            | WorkflowGovernanceEvent::CoreDomainPackRebased(_)
    ) || projection.active_effective_bundle_identity().is_some()
    {
        WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION
    } else {
        WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION
    }
}

fn validate_legacy_solo_adoption(
    projection: &WorkflowGovernanceLedgerProjection,
    state_version: u64,
    event: &LegacySoloProfileAdoptedEvent,
) -> Result<(), WorkflowGovernanceLedgerError> {
    let Some(genesis) = projection.records.first() else {
        return Err(WorkflowGovernanceLedgerError::NotInitialized);
    };
    let WorkflowGovernanceEvent::ProjectImported(imported) = &genesis.event else {
        return Err(WorkflowGovernanceLedgerError::FirstEventNotProjectImported);
    };
    if imported.readiness_profile.is_some() {
        return Err(WorkflowGovernanceLedgerError::LegacySoloAdoptionInvalid {
            line: None,
            reason: "genesis already selected an explicit readiness profile",
        });
    }
    if projection.contains_legacy_solo_adoption()
        || projection.readiness_profile() != Some(WorkflowReadinessProfile::StrictExternal)
    {
        return Err(WorkflowGovernanceLedgerError::LegacySoloAdoptionInvalid {
            line: None,
            reason: "legacy readiness profile has already transitioned",
        });
    }
    if projection.records.iter().any(|record| {
        !matches!(
            record.event,
            WorkflowGovernanceEvent::ProjectImported(_)
                | WorkflowGovernanceEvent::ReleaseUpgraded(_)
        )
    }) {
        return Err(WorkflowGovernanceLedgerError::LegacySoloAdoptionInvalid {
            line: None,
            reason: "legacy history contains authority-bearing or unsupported workflow events",
        });
    }
    if event.legacy_project_import_record_digest != genesis.record_digest
        || projection.head_digest.as_deref() != Some(event.prior_ledger_head_digest.as_str())
        || event.authority_basis != WorkflowCooperativeAuthorityBasis::CooperativeSameOwner
        || state_version != projection.next_state_version
    {
        return Err(WorkflowGovernanceLedgerError::LegacySoloAdoptionInvalid {
            line: None,
            reason: "adoption bindings do not match current legacy history",
        });
    }
    if !is_sha256_digest(&event.snapshot_digest) {
        return Err(WorkflowGovernanceLedgerError::LegacySoloAdoptionInvalid {
            line: None,
            reason: "snapshot digest is not canonical lowercase sha256",
        });
    }
    Ok(())
}

const fn broker_action_event_kind(event: &WorkflowGovernanceEvent) -> Option<&'static str> {
    match event {
        WorkflowGovernanceEvent::HumanIntentRevisionAccepted(_) => Some("intent_revision"),
        WorkflowGovernanceEvent::ApplicabilityAssessed(_) => Some("applicability"),
        WorkflowGovernanceEvent::CapabilityProbed(_) => Some("capability"),
        WorkflowGovernanceEvent::DecisionResolved(_) => Some("decision"),
        WorkflowGovernanceEvent::EvaluatorObserved(_) => Some("evidence"),
        WorkflowGovernanceEvent::SignalChanged(_) => Some("signal"),
        WorkflowGovernanceEvent::WaiverAuthorized(_) => Some("waiver"),
        _ => None,
    }
}

fn is_lower_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn ensure_append_capacity(
    state_root: &Path,
    projection: &WorkflowGovernanceLedgerProjection,
    new_line_bytes: usize,
) -> Result<(), WorkflowGovernanceLedgerError> {
    let next_count = projection.records.len().saturating_add(1);
    if next_count > WORKFLOW_GOVERNANCE_LEDGER_MAX_RECORDS {
        return Err(WorkflowGovernanceLedgerError::CapacityRecords {
            found: next_count,
            maximum: WORKFLOW_GOVERNANCE_LEDGER_MAX_RECORDS,
        });
    }
    let wal_path = workflow_governance_wal_path(state_root)?;
    let existing_bytes = if wal_path.exists() {
        fs::metadata(&wal_path)
            .map_err(|source| io_error(&wal_path, source))?
            .len()
    } else {
        0
    };
    let found = existing_bytes.saturating_add(u64::try_from(new_line_bytes).unwrap_or(u64::MAX));
    if found > WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES {
        return Err(WorkflowGovernanceLedgerError::CapacityBytes {
            found,
            maximum: WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES,
        });
    }
    Ok(())
}

fn ensure_prepared_capacity(
    projection: &WorkflowGovernanceLedgerProjection,
    prepared_wal_bytes: usize,
    new_line_bytes: usize,
) -> Result<(), WorkflowGovernanceLedgerError> {
    let next_count = projection.records.len().saturating_add(1);
    if next_count > WORKFLOW_GOVERNANCE_LEDGER_MAX_RECORDS {
        return Err(WorkflowGovernanceLedgerError::CapacityRecords {
            found: next_count,
            maximum: WORKFLOW_GOVERNANCE_LEDGER_MAX_RECORDS,
        });
    }
    let found = u64::try_from(prepared_wal_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(u64::try_from(new_line_bytes).unwrap_or(u64::MAX));
    if found > WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES {
        return Err(WorkflowGovernanceLedgerError::CapacityBytes {
            found,
            maximum: WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES,
        });
    }
    Ok(())
}

fn validate_append_identity(
    projection: &WorkflowGovernanceLedgerProjection,
    identity: &WorkflowGovernanceLedgerIdentity,
) -> Result<(), WorkflowGovernanceLedgerError> {
    let expected = projection
        .active_identity()
        .ok_or(WorkflowGovernanceLedgerError::NotInitialized)?;
    if expected.project_id != identity.project_id {
        return Err(WorkflowGovernanceLedgerError::ProjectMismatch {
            line: None,
            expected: expected.project_id,
            found: identity.project_id.clone(),
        });
    }
    if expected.bundle_id != identity.bundle_id || expected.bundle_digest != identity.bundle_digest
    {
        return Err(WorkflowGovernanceLedgerError::BundleMismatch {
            line: None,
            expected_id: expected.bundle_id,
            found_id: identity.bundle_id.clone(),
            expected_digest: expected.bundle_digest,
            found_digest: identity.bundle_digest.clone(),
        });
    }
    Ok(())
}

fn validate_phase_advance_source(
    event: &PhaseAdvancedEvent,
    current_phase: Option<&StableId>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if event
        .from_phase
        .as_ref()
        .is_some_and(|from| current_phase.is_none_or(|current| from != current))
    {
        return Err(
            WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                reason: "phase advancement source does not match the recovered current phase",
            },
        );
    }
    Ok(())
}

fn projection_current_phase(projection: &WorkflowGovernanceLedgerProjection) -> Option<StableId> {
    let mut phase = None;
    for record in &projection.records {
        match &record.event {
            WorkflowGovernanceEvent::ProjectImported(event) => {
                phase = Some(event.initial_phase.clone());
            }
            WorkflowGovernanceEvent::PhaseAdvanced(event) => {
                phase = Some(event.to_phase.clone());
            }
            WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event) => {
                if let Some(to_phase) = event.to_phase.as_ref() {
                    phase = Some(to_phase.clone());
                }
            }
            _ => {}
        }
    }
    phase
}

fn last_post_build_verify_episode<'a>(
    projection: &'a WorkflowGovernanceLedgerProjection,
    episode_id: &StableId,
) -> Option<&'a PostBuildVerifyEpisodeAppliedEvent> {
    projection.records.iter().rev().find_map(|record| {
        if let WorkflowGovernanceEvent::PostBuildVerifyEpisodeApplied(event) = &record.event {
            (event.episode_id == *episode_id).then_some(event)
        } else {
            None
        }
    })
}

fn last_coordination_request<'a>(
    projection: &'a WorkflowGovernanceLedgerProjection,
    request_id: &StableId,
) -> Option<&'a CoordinationRequestState> {
    projection.records.iter().rev().find_map(|record| {
        if let WorkflowGovernanceEvent::CoordinationStateApplied(event) = &record.event {
            if let CoordinationStateRecord::Request(state) = &event.state {
                return (state.request.request_contract.id == *request_id).then_some(state);
            }
        }
        None
    })
}

const fn request_status_transition_allowed(from: RequestStatus, to: RequestStatus) -> bool {
    matches!(
        (from, to),
        (
            RequestStatus::Pending,
            RequestStatus::Accepted
                | RequestStatus::Rejected
                | RequestStatus::Superseded
                | RequestStatus::Expired
        ) | (
            RequestStatus::Accepted,
            RequestStatus::Applied
                | RequestStatus::Rejected
                | RequestStatus::Superseded
                | RequestStatus::Expired
        )
    )
}

#[allow(clippy::too_many_lines)]
fn validate_coordination_state_event(
    event: &CoordinationStateAppliedEvent,
    current_head_digest: Option<&str>,
    current_state_version: Option<u64>,
    previous_request: Option<&CoordinationRequestState>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if !is_lower_sha256(&event.prior_ledger_head_digest)
        || current_head_digest != Some(event.prior_ledger_head_digest.as_str())
        || current_state_version != Some(event.prior_state_version)
    {
        return Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
            reason: "coordination head or state-version binding is invalid",
        });
    }

    match &event.state {
        CoordinationStateRecord::Request(state) => {
            let request = &state.request.request_contract;
            if state.request.schema_version.trim().is_empty()
                || request.id.0.trim().is_empty()
                || request.contract_ref.0.trim().is_empty()
                || request.sender_agent_id.0.trim().is_empty()
                || request.target_driver.0.trim().is_empty()
                || request.requested_operation.0.trim().is_empty()
                || state.actor_agent_id.0.trim().is_empty()
                || state
                    .response_evidence_refs
                    .iter()
                    .any(|reference| reference.trim().is_empty())
            {
                return Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
                    reason: "request coordination identity or evidence is incomplete",
                });
            }
            match previous_request {
                None if state.previous_status.is_none()
                    && request.status == RequestStatus::Pending
                    && state.actor_agent_id == request.sender_agent_id => {}
                Some(previous)
                    if state.previous_status == Some(previous.request.request_contract.status)
                        && request_status_transition_allowed(
                            previous.request.request_contract.status,
                            request.status,
                        )
                        && state.actor_agent_id == request.target_driver =>
                {
                    let mut expected = previous.request.clone();
                    expected.request_contract.status = request.status;
                    if expected != state.request {
                        return Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
                            reason: "request transition changed immutable request fields",
                        });
                    }
                }
                _ => {
                    return Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
                        reason: "request status does not extend the exact durable predecessor",
                    });
                }
            }
            if let Some(handoff) = state.mutation_handoff.as_ref() {
                if request.status != RequestStatus::Applied
                    || !request.safety.driver_must_apply
                    || handoff.driver_agent_id != request.target_driver
                    || handoff.requested_operation != request.requested_operation
                    || handoff.claim_contract_ref.0.trim().is_empty()
                    || handoff.authority_refs.is_empty()
                    || handoff.effect_contract_refs.is_empty()
                    || handoff
                        .authority_refs
                        .iter()
                        .chain(&handoff.effect_contract_refs)
                        .any(|reference| reference.trim().is_empty())
                {
                    return Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
                        reason: "mutation handoff is not an exact evidence-only driver binding",
                    });
                }
            } else if request.status == RequestStatus::Applied && request.safety.driver_must_apply {
                return Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
                    reason:
                        "driver-applied request is missing its authority/effect handoff evidence",
                });
            }
        }
        CoordinationStateRecord::Completion(state) => {
            let completion = &state.completion.completion_contract;
            if state.completion.schema_version.trim().is_empty()
                || completion.id.0.trim().is_empty()
                || completion.contract_ref.0.trim().is_empty()
                || completion.task.task_id.0.trim().is_empty()
                || completion.status.changed_by.0.trim().is_empty()
                || completion.status.checked_at_state_version != event.prior_state_version
                || state.applied_claim_id.0.trim().is_empty()
                || completion
                    .claim
                    .claim_contract_ref
                    .as_ref()
                    .is_none_or(|reference| reference.0.trim().is_empty())
                || completion
                    .proof_refs
                    .iter()
                    .any(|reference| reference.trim().is_empty())
            {
                return Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
                    reason:
                        "completion projection lacks its exact task, claim, state, or proof binding",
                });
            }
            if matches!(
                completion.status.value,
                forge_core_contracts::completion::CompletionStatus::Done
            ) && completion.proof_policy.required_for_done
                && completion.proof_refs.is_empty()
            {
                return Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
                    reason: "done completion is missing required proof",
                });
            }
        }
        CoordinationStateRecord::HealthRecovery(state) => {
            let recovery = &state.recovery.health_recovery_contract;
            if state.recovery.schema_version.trim().is_empty()
                || recovery.id.0.trim().is_empty()
                || recovery.contract_ref.0.trim().is_empty()
                || recovery.runtime.agent_id.0.trim().is_empty()
                || recovery.runtime.host.0.trim().is_empty()
                || state.actor_agent_id.0.trim().is_empty()
                || (recovery.recovery.requires_review && recovery.recovery.automatic_allowed)
            {
                return Err(WorkflowGovernanceLedgerError::CoordinationStateInvalid {
                    reason: "health-recovery projection has an invalid identity or review boundary",
                });
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn validate_post_build_verify_episode_event(
    event: &PostBuildVerifyEpisodeAppliedEvent,
    current_phase: Option<&StableId>,
    active_release: Option<&WorkflowGovernanceReleaseIdentity>,
    current_head_digest: Option<&str>,
    current_state_version: Option<u64>,
    previous_episode: Option<(u64, &str)>,
    complete_snapshot_required: bool,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if event.episode_id.0.trim().is_empty()
        || event.generation == 0
        || !is_lower_sha256(&event.episode_digest)
        || !is_lower_sha256(&event.decision_digest)
        || !is_lower_sha256(&event.snapshot_digest)
        || !is_lower_sha256(&event.prior_ledger_head_digest)
        || event.release_subject.lineage_id.0.trim().is_empty()
        || event.release_subject.release_id.0.trim().is_empty()
        || event.release_subject.release_version.trim().is_empty()
        || !is_lower_sha256(&event.release_subject.release_digest)
        || active_release.is_some_and(|active| active != &event.release_subject)
        || current_head_digest != Some(event.prior_ledger_head_digest.as_str())
        || current_state_version != Some(event.prior_state_version)
        || current_phase != Some(&event.from_phase)
    {
        return Err(
            WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                reason:
                    "episode identity, release, snapshot, head, state, or phase binding is invalid",
            },
        );
    }
    match event.episode_snapshot.as_ref() {
        Some(document) => {
            let episode = &document.post_build_verify_episode;
            if !document.validate().is_empty()
                || episode.episode_id != event.episode_id
                || episode.generation != event.generation
                || episode.previous_episode_digest != event.previous_episode_digest
                || episode.episode_digest != event.episode_digest
                || episode.release_subject != event.release_subject
                || episode.build_verify_snapshot.subject_digest != event.snapshot_digest
            {
                return Err(
                    WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                        reason:
                            "complete episode snapshot does not match the durable event summary",
                    },
                );
            }
        }
        None if complete_snapshot_required => {
            return Err(
                WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                    reason: "replacement-continuity episode is missing its complete snapshot",
                },
            );
        }
        None => {}
    }
    match previous_episode {
        None if event.generation == 1 && event.previous_episode_digest.is_none() => {}
        Some((generation, digest))
            if event.generation == generation.saturating_add(1)
                && event.previous_episode_digest.as_deref() == Some(digest) => {}
        _ => {
            return Err(
                WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                    reason: "episode generation does not extend the exact predecessor",
                },
            );
        }
    }
    let phase = Phase::parse(&event.from_phase.0).ok_or(
        WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
            reason: "episode source phase is invalid",
        },
    )?;
    let gate = event.admitted_gate.as_ref();
    let valid = match event.outcome {
        PostBuildVerifyEpisodeOutcome::AdvancedToReadyOperate => {
            phase == Phase::BuildVerify
                && event
                    .to_phase
                    .as_ref()
                    .and_then(|value| Phase::parse(&value.0))
                    == Some(Phase::ReadyOperate)
                && gate.is_some_and(|result| {
                    result.kind == PostBuildVerifyGateKind::Readiness
                        && matches!(result.status, GateStatus::Pass)
                        && is_lower_sha256(&result.effective_bundle_digest)
                })
        }
        PostBuildVerifyEpisodeOutcome::AdvancedToEvolve => {
            phase == Phase::ReadyOperate
                && event
                    .to_phase
                    .as_ref()
                    .and_then(|value| Phase::parse(&value.0))
                    == Some(Phase::Evolve)
                && gate.is_some_and(|result| {
                    result.kind == PostBuildVerifyGateKind::Release
                        && matches!(result.status, GateStatus::Pass)
                        && is_lower_sha256(&result.effective_bundle_digest)
                })
        }
        PostBuildVerifyEpisodeOutcome::RollbackAssessmentOpened
        | PostBuildVerifyEpisodeOutcome::EvolutionTriageOpened => {
            matches!(phase, Phase::ReadyOperate | Phase::Evolve)
                && event.to_phase.is_none()
                && gate.is_none()
        }
    };
    if !valid {
        return Err(
            WorkflowGovernanceLedgerError::PostBuildVerifyEpisodeInvalid {
                reason: "episode outcome, phase transition, or admitted gate is invalid",
            },
        );
    }
    Ok(())
}

fn active_release_identity(
    projection: &WorkflowGovernanceLedgerProjection,
) -> Option<WorkflowGovernanceReleaseIdentity> {
    projection
        .records
        .iter()
        .rev()
        .find_map(|record| match &record.event {
            WorkflowGovernanceEvent::ReleaseUpgraded(event) => Some(event.to_release.clone()),
            WorkflowGovernanceEvent::CoreDomainPackRebased(event) => {
                Some(event.release_transition.to_release.clone())
            }
            _ => None,
        })
}

/// Deterministic receipt migration for one Domain Pack epoch transition.
/// Preservation is allowed only when the complete core runtime, effective
/// runtime, and kernel-derived receipt context remain byte-identical.
#[must_use]
pub fn domain_pack_receipt_carryover(
    from: &WorkflowEffectiveBundleIdentity,
    to: &WorkflowEffectiveBundleIdentity,
) -> WorkflowReceiptCarryover {
    if from.core_runtime_bundle == to.core_runtime_bundle
        && from.effective_runtime_bundle == to.effective_runtime_bundle
        && from.receipt_context_digest == to.receipt_context_digest
    {
        WorkflowReceiptCarryover::PreservePolicyEquivalent
    } else {
        WorkflowReceiptCarryover::InvalidateAll
    }
}

fn validate_domain_pack_transition(
    event: &DomainPackGenerationTransitionedEvent,
    active_core_envelope: Option<&WorkflowGovernanceLedgerIdentity>,
    active_core_runtime: Option<&WorkflowRuntimeBundleIdentity>,
    active_effective: Option<&WorkflowEffectiveBundleIdentity>,
    previous_head_digest: Option<&str>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    let active_core_envelope =
        active_core_envelope.ok_or(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "Domain Pack transition has no active core identity",
        })?;
    let previous_head_digest =
        previous_head_digest.ok_or(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "Domain Pack transition has no previous ledger head",
        })?;
    if !is_sha256_digest(&event.prior_ledger_head_digest) {
        return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "prior ledger head digest is invalid",
        });
    }
    if event.prior_ledger_head_digest != previous_head_digest {
        return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "prior ledger head does not match the transition envelope",
        });
    }
    validate_effective_identity(&event.from_effective_bundle)?;
    validate_effective_identity(&event.to_effective_bundle)?;
    for effective in [&event.from_effective_bundle, &event.to_effective_bundle] {
        if effective.core_runtime_bundle.bundle_id != active_core_envelope.bundle_id
            || effective.core_runtime_bundle.bundle_digest != active_core_envelope.bundle_digest
        {
            return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
                reason: "effective identity does not bind the active core ledger envelope",
            });
        }
        if active_core_runtime.is_some_and(|active| active != &effective.core_runtime_bundle) {
            return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
                reason: "effective identity does not bind the active core runtime",
            });
        }
    }
    match active_effective {
        Some(active) if active != &event.from_effective_bundle => {
            return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
                reason: "from effective identity is not the active ledger epoch",
            });
        }
        None => {
            if event.from_effective_bundle.domain_pack_generation.is_some()
                || event.from_effective_bundle.core_runtime_bundle
                    != event.from_effective_bundle.effective_runtime_bundle
            {
                return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
                    reason: "first Domain Pack transition must start from core-only identity",
                });
            }
        }
        Some(_) => {}
    }
    let to_generation = event
        .to_effective_bundle
        .domain_pack_generation
        .as_ref()
        .ok_or(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "target effective identity has no Domain Pack generation",
        })?;
    if let Some(from_generation) = event.from_effective_bundle.domain_pack_generation.as_ref() {
        if to_generation.generation <= from_generation.generation {
            return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
                reason: "Domain Pack generation must advance monotonically",
            });
        }
    }
    if event.receipt_carryover
        != domain_pack_receipt_carryover(&event.from_effective_bundle, &event.to_effective_bundle)
    {
        return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "receipt carryover is not the deterministic exact-equivalence result",
        });
    }
    Ok(())
}

fn validate_core_domain_pack_rebase(
    event: &CoreDomainPackRebasedEvent,
    source: &WorkflowGovernanceLedgerIdentity,
    target: &WorkflowGovernanceLedgerIdentity,
    active_release: Option<&WorkflowGovernanceReleaseIdentity>,
    active_runtime: Option<&WorkflowRuntimeBundleIdentity>,
    active_effective: Option<&WorkflowEffectiveBundleIdentity>,
    previous_head_digest: Option<&str>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    validate_release_transition(
        &event.release_transition,
        source,
        target,
        active_release,
        active_runtime,
        previous_head_digest,
    )?;
    let previous_head_digest =
        previous_head_digest.ok_or(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "joined rebase has no previous ledger head",
        })?;
    if event.prior_ledger_head_digest != previous_head_digest
        || event.release_transition.prior_ledger_head_digest != previous_head_digest
    {
        return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "joined rebase does not bind one exact prior ledger head",
        });
    }
    validate_effective_identity(&event.from_effective_bundle)?;
    validate_effective_identity(&event.to_effective_bundle)?;
    if active_effective != Some(&event.from_effective_bundle) {
        return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "joined rebase source is not the active effective epoch",
        });
    }
    if event.from_effective_bundle.core_runtime_bundle
        != event.release_transition.from_runtime_bundle
        || event.to_effective_bundle.core_runtime_bundle
            != event.release_transition.to_runtime_bundle
    {
        return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "joined rebase effective epochs do not bind release runtime endpoints",
        });
    }
    let from_generation = event
        .from_effective_bundle
        .domain_pack_generation
        .as_ref()
        .ok_or(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "joined rebase source has no Domain Pack generation",
        })?;
    let to_generation = event
        .to_effective_bundle
        .domain_pack_generation
        .as_ref()
        .ok_or(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "joined rebase target has no Domain Pack generation",
        })?;
    if to_generation.generation <= from_generation.generation
        || to_generation.base_core_bundle_digest == from_generation.base_core_bundle_digest
    {
        return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "joined rebase must advance generation and change its sealed Core binding",
        });
    }
    let expected =
        domain_pack_receipt_carryover(&event.from_effective_bundle, &event.to_effective_bundle);
    if event.receipt_carryover != expected
        || event.release_transition.receipt_carryover != expected
        || expected != WorkflowReceiptCarryover::InvalidateAll
    {
        return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid {
            reason: "joined rebase receipt carryover must be deterministic invalidation",
        });
    }
    Ok(())
}

fn validate_effective_identity(
    identity: &WorkflowEffectiveBundleIdentity,
) -> Result<(), WorkflowGovernanceLedgerError> {
    for (value, reason) in [
        (
            identity.core_runtime_bundle.bundle_id.0.as_str(),
            "core runtime bundle id is blank",
        ),
        (
            identity.effective_runtime_bundle.bundle_id.0.as_str(),
            "effective runtime bundle id is blank",
        ),
    ] {
        if value.trim().is_empty() {
            return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid { reason });
        }
    }
    for (value, reason) in [
        (
            identity.core_runtime_bundle.bundle_digest.as_str(),
            "core runtime bundle digest is invalid",
        ),
        (
            identity.core_runtime_bundle.policy_set_digest.as_str(),
            "core policy-set digest is invalid",
        ),
        (
            identity.effective_runtime_bundle.bundle_digest.as_str(),
            "effective runtime bundle digest is invalid",
        ),
        (
            identity.effective_runtime_bundle.policy_set_digest.as_str(),
            "effective policy-set digest is invalid",
        ),
        (
            identity.receipt_context_digest.as_str(),
            "receipt context digest is invalid",
        ),
    ] {
        if !is_sha256_digest(value) {
            return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid { reason });
        }
    }
    if let Some(generation) = &identity.domain_pack_generation {
        for (value, reason) in [
            (
                generation.active_lock_digest.as_str(),
                "active lock digest is invalid",
            ),
            (
                generation.composition_digest.as_str(),
                "composition digest is invalid",
            ),
            (
                generation.base_core_bundle_digest.as_str(),
                "base core bundle digest is invalid",
            ),
            (
                generation.supply_chain_registry_digest.as_str(),
                "supply-chain registry digest is invalid",
            ),
        ] {
            if !is_sha256_digest(value) {
                return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid { reason });
            }
        }
        for (value, reason) in [
            (
                generation.reviewer_registry_digest.as_str(),
                "reviewer registry digest is not bare lowercase sha256 hex",
            ),
            (
                generation.reviewed_registry_digest.as_str(),
                "reviewed registry digest is not bare lowercase sha256 hex",
            ),
        ] {
            if !is_bare_sha256_hex(value) {
                return Err(WorkflowGovernanceLedgerError::DomainPackTransitionInvalid { reason });
            }
        }
    }
    Ok(())
}

fn validate_release_transition(
    event: &ReleaseUpgradedEvent,
    source: &WorkflowGovernanceLedgerIdentity,
    target: &WorkflowGovernanceLedgerIdentity,
    active_release: Option<&WorkflowGovernanceReleaseIdentity>,
    active_runtime: Option<&WorkflowRuntimeBundleIdentity>,
    previous_head_digest: Option<&str>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    validate_release_transition_identities(event, source, target, active_release, active_runtime)?;
    validate_release_transition_fields(event)?;
    let previous_head_digest =
        previous_head_digest.ok_or(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
            reason: "release transition has no previous ledger head",
        })?;
    if event.prior_ledger_head_digest != previous_head_digest {
        return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
            reason: "prior ledger head does not match the transition record envelope",
        });
    }
    if event.admission_proof.from_policy_set_digest != event.from_runtime_bundle.policy_set_digest
        || event.admission_proof.to_policy_set_digest != event.to_runtime_bundle.policy_set_digest
    {
        return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
            reason: "admission proof policy-set bindings do not match the runtime bundles",
        });
    }
    Ok(())
}

fn validate_release_transition_identities(
    event: &ReleaseUpgradedEvent,
    source: &WorkflowGovernanceLedgerIdentity,
    target: &WorkflowGovernanceLedgerIdentity,
    active_release: Option<&WorkflowGovernanceReleaseIdentity>,
    active_runtime: Option<&WorkflowRuntimeBundleIdentity>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if source.project_id != target.project_id {
        return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
            reason: "source and target project identities differ",
        });
    }
    if event.from_runtime_bundle.bundle_id != source.bundle_id
        || event.from_runtime_bundle.bundle_digest != source.bundle_digest
    {
        return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
            reason: "from_runtime_bundle does not match the active source identity",
        });
    }
    if active_runtime.is_some_and(|active| active != &event.from_runtime_bundle) {
        return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
            reason: "from_runtime_bundle does not match the current policy-set identity",
        });
    }
    if event.to_runtime_bundle.bundle_id != target.bundle_id
        || event.to_runtime_bundle.bundle_digest != target.bundle_digest
    {
        return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
            reason: "to_runtime_bundle does not match the exact target identity",
        });
    }
    if source.bundle_id == target.bundle_id && source.bundle_digest == target.bundle_digest {
        return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
            reason: "release transition target is identical to its source",
        });
    }
    if event.from_release == event.to_release {
        return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
            reason: "release transition cannot upgrade a release to itself",
        });
    }
    if event.from_release.lineage_id != event.to_release.lineage_id {
        return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
            reason: "release transition changes release lineage",
        });
    }
    if active_release.is_some_and(|active| active != &event.from_release) {
        return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid {
            reason: "from_release does not match the current release identity",
        });
    }
    Ok(())
}

fn validate_release_transition_fields(
    event: &ReleaseUpgradedEvent,
) -> Result<(), WorkflowGovernanceLedgerError> {
    for (value, reason) in [
        (
            &event.from_release.lineage_id.0,
            "from release lineage id is blank",
        ),
        (&event.from_release.release_id.0, "from release id is blank"),
        (
            &event.from_release.release_version,
            "from release version is blank",
        ),
        (
            &event.to_release.lineage_id.0,
            "to release lineage id is blank",
        ),
        (&event.to_release.release_id.0, "to release id is blank"),
        (
            &event.to_release.release_version,
            "to release version is blank",
        ),
        (
            &event.from_runtime_bundle.bundle_id.0,
            "from runtime bundle id is blank",
        ),
        (
            &event.to_runtime_bundle.bundle_id.0,
            "to runtime bundle id is blank",
        ),
        (
            &event.registry_provenance.registry_id.0,
            "registry provenance id is blank",
        ),
        (
            &event.registry_provenance.registry_version,
            "registry provenance version is blank",
        ),
        (
            &event.admission_proof.proof_id.0,
            "admission proof id is blank",
        ),
    ] {
        if value.trim().is_empty() {
            return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid { reason });
        }
    }
    for (value, reason) in [
        (
            &event.from_release.release_digest,
            "from release digest is invalid",
        ),
        (
            &event.to_release.release_digest,
            "to release digest is invalid",
        ),
        (
            &event.from_runtime_bundle.bundle_digest,
            "from runtime bundle digest is invalid",
        ),
        (
            &event.from_runtime_bundle.policy_set_digest,
            "from policy-set digest is invalid",
        ),
        (
            &event.to_runtime_bundle.bundle_digest,
            "to runtime bundle digest is invalid",
        ),
        (
            &event.to_runtime_bundle.policy_set_digest,
            "to policy-set digest is invalid",
        ),
        (
            &event.registry_provenance.registry_digest,
            "registry provenance digest is invalid",
        ),
        (
            &event.admission_proof.proof_digest,
            "admission proof digest is invalid",
        ),
        (
            &event.admission_proof.snapshot_digest,
            "admission snapshot digest is invalid",
        ),
        (
            &event.admission_proof.from_policy_set_digest,
            "admission source policy-set digest is invalid",
        ),
        (
            &event.admission_proof.to_policy_set_digest,
            "admission target policy-set digest is invalid",
        ),
        (
            &event.prior_ledger_head_digest,
            "prior ledger head digest is invalid",
        ),
    ] {
        if !is_sha256_digest(value) {
            return Err(WorkflowGovernanceLedgerError::ReleaseTransitionInvalid { reason });
        }
    }
    Ok(())
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn is_bare_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_identity(
    identity: &WorkflowGovernanceLedgerIdentity,
) -> Result<(), WorkflowGovernanceLedgerError> {
    validate_nonblank(&identity.project_id.0, None, "project_id")?;
    validate_nonblank(&identity.bundle_id.0, None, "bundle_id")?;
    validate_nonblank(&identity.bundle_digest, None, "bundle_digest")
}

fn validate_record_fields(
    record: &WorkflowGovernanceLedgerRecord,
    line: Option<usize>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    validate_nonblank(&record.record_id.0, line, "record_id")?;
    validate_nonblank(&record.project_id.0, line, "project_id")?;
    validate_nonblank(&record.bundle_id.0, line, "bundle_id")?;
    validate_nonblank(&record.bundle_digest, line, "bundle_digest")?;
    validate_nonblank(&record.record_digest, line, "record_digest")?;
    validate_broker_origin_replay_digest(&record.event, line)?;
    if let WorkflowGovernanceEvent::LegacySoloProfileAdopted(event) = &record.event {
        for value in [
            &event.legacy_project_import_record_digest,
            &event.prior_ledger_head_digest,
            &event.snapshot_digest,
        ] {
            if !is_sha256_digest(value) {
                return Err(WorkflowGovernanceLedgerError::LegacySoloAdoptionInvalid {
                    line,
                    reason: "transition contains a non-canonical digest",
                });
            }
        }
    }
    if let WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event) = &record.event {
        validate_cooperative_objective_shape(event, line)?;
    }
    if let WorkflowGovernanceEvent::CooperativeEvidenceObserved(event) = &record.event {
        validate_cooperative_evidence_shape(event, line)?;
    }
    if let WorkflowGovernanceEvent::WorkFocusRecorded(event) = &record.event {
        validate_work_focus_shape(event, line)?;
    }
    Ok(())
}

fn validate_work_focus_event(
    projection: &WorkflowGovernanceLedgerProjection,
    state_version: u64,
    event: &WorkflowWorkFocusRecordedEvent,
    line: usize,
) -> Result<(), WorkflowGovernanceLedgerError> {
    validate_work_focus_shape(event, Some(line))?;
    validate_work_focus_reference_bindings(
        &event.blocker_record_digests,
        &event.evidence_record_digests,
        &projection.records,
        Some(line),
    )?;
    if projection.readiness_profile() != Some(WorkflowReadinessProfile::SoloCooperative) {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line: Some(line),
            reason: "Work Focus requires the solo_cooperative readiness profile",
        });
    }
    let Some((objective_record, objective)) = projection.records.iter().rev().find_map(|record| {
        let WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event) = &record.event else {
            return None;
        };
        Some((record, event))
    }) else {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line: Some(line),
            reason: "Work Focus requires an accepted cooperative objective",
        });
    };
    if projection.head_digest.as_deref() != Some(event.admission_ledger_head_digest.as_str())
        || projection.current_state_version() != Some(event.admission_state_version)
        || state_version != event.admission_state_version
    {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line: Some(line),
            reason: "Work Focus admission coordinates are stale",
        });
    }
    validate_work_focus_current_bindings(
        event,
        objective,
        &objective_record.record_digest,
        objective_record.sequence,
        projection_current_phase(projection).as_ref(),
        projection
            .latest_work_focus_record()
            .map(|(record, event)| (record.record_digest.as_str(), event)),
        Some(line),
    )
}

fn validate_work_focus_reference_bindings(
    blocker_record_digests: &[String],
    evidence_record_digests: &[String],
    records: &[WorkflowGovernanceLedgerRecord],
    line: Option<usize>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    let blockers_are_canonical = blocker_record_digests.iter().all(|digest| {
        matches!(
            records
                .iter()
                .find(|record| record.record_digest.as_str() == digest.as_str())
                .map(|record| &record.event),
            Some(WorkflowGovernanceEvent::DecisionNeedRaised(_))
        )
    });
    let evidence_is_canonical = evidence_record_digests.iter().all(|digest| {
        matches!(
            records
                .iter()
                .find(|record| record.record_digest.as_str() == digest.as_str())
                .map(|record| &record.event),
            Some(WorkflowGovernanceEvent::CooperativeEvidenceObserved(event))
                if event.disposition
                    == forge_core_contracts::WorkflowCooperativeEvidenceDisposition::Admitted
                    && event.admitted_evidence.is_some()
        )
    });
    if !blockers_are_canonical || !evidence_is_canonical {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line,
            reason: "Work Focus references must identify earlier canonical blocker and admitted evidence records",
        });
    }
    Ok(())
}

fn validate_recovered_work_focus_event(
    record: &WorkflowGovernanceLedgerRecord,
    line: usize,
    identity: &RecoveredIdentityState,
    prior_records: &[WorkflowGovernanceLedgerRecord],
    event: &WorkflowWorkFocusRecordedEvent,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if identity.readiness_profile != Some(WorkflowReadinessProfile::SoloCooperative) {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line: Some(line),
            reason: "Work Focus requires the solo_cooperative readiness profile",
        });
    }
    let Some(objective) = identity.latest_cooperative_objective.as_ref() else {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line: Some(line),
            reason: "Work Focus requires an accepted cooperative objective",
        });
    };
    if record.previous_record_digest.as_deref() != Some(event.admission_ledger_head_digest.as_str())
        || record.state_version != event.admission_state_version
        || record.recorded_at_unix != event.recorded_at_unix
    {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line: Some(line),
            reason: "Work Focus admission coordinates do not match its ledger record",
        });
    }
    validate_work_focus_reference_bindings(
        &event.blocker_record_digests,
        &event.evidence_record_digests,
        prior_records,
        Some(line),
    )?;
    validate_work_focus_current_bindings(
        event,
        objective,
        identity
            .latest_cooperative_objective_record_digest
            .as_deref()
            .unwrap_or_default(),
        identity
            .latest_cooperative_objective_record_sequence
            .unwrap_or_default(),
        identity.current_phase.as_ref(),
        identity.latest_work_focus.as_ref().map(|previous| {
            (
                identity
                    .latest_work_focus_record_digest
                    .as_deref()
                    .unwrap_or_default(),
                previous,
            )
        }),
        Some(line),
    )
}

fn validate_work_focus_current_bindings(
    event: &WorkflowWorkFocusRecordedEvent,
    objective: &forge_core_contracts::CooperativeObjectiveAcceptedEvent,
    objective_record_digest: &str,
    objective_record_sequence: u64,
    current_phase: Option<&StableId>,
    previous: Option<(&str, &WorkflowWorkFocusRecordedEvent)>,
    line: Option<usize>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if event.objective.objective_id != objective.objective_id
        || event.objective.objective_revision != objective.revision
        || event.objective.objective_digest != objective.objective_digest
        || event.objective.assurance_epoch != objective.assurance_epoch
        || event.objective.accepted_objective_record_digest != objective_record_digest
        || event.objective.accepted_objective_record_sequence != objective_record_sequence
    {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line,
            reason: "Work Focus is not bound to the latest accepted objective",
        });
    }
    let expected_phase = event.phase.to_string();
    if current_phase.map(|phase| phase.0.as_str()) != Some(expected_phase.as_str()) {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line,
            reason: "Work Focus phase does not match the recovered current phase",
        });
    }
    match previous {
        None => {
            if event.previous_work_focus_record_digest.is_some()
                || event.state != WorkflowWorkFocusState::Active
            {
                return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
                    line,
                    reason: "the first Work Focus must be active and have no predecessor",
                });
            }
        }
        Some((previous_record_digest, previous_event)) => {
            if event.previous_work_focus_record_digest.as_deref() != Some(previous_record_digest) {
                return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
                    line,
                    reason: "Work Focus predecessor does not match the latest focus record",
                });
            }
            let previous_is_terminal = matches!(
                previous_event.state,
                WorkflowWorkFocusState::Completed | WorkflowWorkFocusState::Abandoned
            );
            if (previous_is_terminal || event.focus_id != previous_event.focus_id)
                && (event.focus_id == previous_event.focus_id
                    || event.state != WorkflowWorkFocusState::Active)
            {
                return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
                    line,
                    reason:
                        "a terminal or superseded Work Focus must continue as a new active focus",
                });
            }
        }
    }
    Ok(())
}

fn validate_work_focus_shape(
    event: &WorkflowWorkFocusRecordedEvent,
    line: Option<usize>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    let bounded_nonblank = |value: &str| {
        !value.trim().is_empty() && value.as_bytes().len() <= MAX_WORK_FOCUS_TEXT_BYTES
    };
    if !bounded_nonblank(&event.focus_id.0)
        || !bounded_nonblank(&event.title)
        || !bounded_nonblank(&event.intended_outcome)
        || !bounded_nonblank(&event.acceptance_summary)
        || !bounded_nonblank(&event.current_activity)
        || !bounded_nonblank(&event.next_step)
        || !bounded_nonblank(&event.recorded_by.0)
        || event.recorded_at_unix == 0
        || event.host_provenance.observed_at_unix == 0
        || event.host_provenance.observed_at_unix > event.recorded_at_unix
    {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line,
            reason: "Work Focus required text and ordered clocks must satisfy their bounds",
        });
    }
    if event.non_goals.len() > MAX_WORK_FOCUS_LIST_ITEMS
        || event.canonical_refs.len() > MAX_WORK_FOCUS_LIST_ITEMS
        || event.affected_area_refs.len() > MAX_WORK_FOCUS_LIST_ITEMS
        || event.blocker_record_digests.len() > MAX_WORK_FOCUS_LIST_ITEMS
        || event.evidence_record_digests.len() > MAX_WORK_FOCUS_LIST_ITEMS
        || event.non_goals.iter().any(|value| !bounded_nonblank(value))
        || event.canonical_refs.iter().any(|value| {
            !bounded_nonblank(&value.0) || !cooperative_source_basis_path_is_normalized(&value.0)
        })
        || event.affected_area_refs.iter().any(|value| {
            !bounded_nonblank(&value.0) || !cooperative_source_basis_path_is_normalized(&value.0)
        })
        || event
            .external_work_item_ref
            .as_deref()
            .is_some_and(|value| !bounded_nonblank(value))
        || event
            .selected_practice_ref
            .as_ref()
            .is_some_and(|value| !bounded_nonblank(&value.0))
        || event
            .selected_practice_reason
            .as_deref()
            .is_some_and(|value| !bounded_nonblank(value))
        || event.selected_practice_ref.is_some() != event.selected_practice_reason.is_some()
    {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line,
            reason: "Work Focus lists or optional references exceed their bounds",
        });
    }
    let blocker_refs = event.blocker_record_digests.iter().collect::<BTreeSet<_>>();
    let evidence_refs = event
        .evidence_record_digests
        .iter()
        .collect::<BTreeSet<_>>();
    if blocker_refs.len() != event.blocker_record_digests.len()
        || evidence_refs.len() != event.evidence_record_digests.len()
        || event
            .blocker_record_digests
            .iter()
            .chain(event.evidence_record_digests.iter())
            .any(|digest| !is_lower_sha256(digest))
    {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line,
            reason: "Work Focus record bindings must be unique canonical digests",
        });
    }
    validate_quick_cycle_shape(event, line)?;
    for digest in [
        event.objective.objective_digest.as_str(),
        event.objective.accepted_objective_record_digest.as_str(),
        event.admission_ledger_head_digest.as_str(),
        event.host_provenance.conversation_digest.as_str(),
    ] {
        if !is_lower_sha256(digest) {
            return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
                line,
                reason: "Work Focus contains a non-canonical digest",
            });
        }
    }
    if event
        .previous_work_focus_record_digest
        .as_deref()
        .is_some_and(|digest| !is_lower_sha256(digest))
        || event.objective.objective_revision == 0
        || event.objective.accepted_objective_record_sequence == 0
        || event.objective.assurance_epoch == 0
    {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line,
            reason: "Work Focus revision, sequence, epoch, state, or predecessor is invalid",
        });
    }
    for value in [
        event.objective.objective_id.0.as_str(),
        event.host_provenance.host_id.0.as_str(),
        event.host_provenance.host_version.as_str(),
        event.host_provenance.session_ref.as_str(),
        event.host_provenance.interaction_ref.as_str(),
    ] {
        if !bounded_nonblank(value) {
            return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
                line,
                reason: "Work Focus identity or host provenance exceeds its bound",
            });
        }
    }
    let encoded = to_canonical_json(event).map_err(|error| {
        WorkflowGovernanceLedgerError::Canonicalization {
            source: error.to_string(),
        }
    })?;
    if encoded.len() > MAX_WORK_FOCUS_EVENT_BYTES {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line,
            reason: "Work Focus event exceeds its total byte bound",
        });
    }
    Ok(())
}

fn validate_quick_cycle_shape(
    event: &WorkflowWorkFocusRecordedEvent,
    line: Option<usize>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    let Some(quick_cycle) = event.quick_cycle.as_ref() else {
        return Ok(());
    };
    if quick_cycle.compactness_reason.trim().is_empty()
        || quick_cycle.compactness_reason.as_bytes().len()
            > MAX_QUICK_CYCLE_COMPACTNESS_REASON_BYTES
        || quick_cycle.expansion_history.len() > MAX_QUICK_CYCLE_EXPANSION_ITEMS
    {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line,
            reason: "Quick Cycle compactness or expansion count exceeds its bound",
        });
    }

    let references_are_valid = |references: &[String]| {
        references.len() <= MAX_QUICK_CYCLE_EVIDENCE_ITEMS
            && references.iter().collect::<BTreeSet<_>>().len() == references.len()
            && references.iter().all(|digest| {
                is_lower_sha256(digest)
                    && event
                        .evidence_record_digests
                        .iter()
                        .any(|owned| owned == digest)
            })
    };
    let closeouts = [
        quick_cycle.stage_closeouts.analysis_discovery.as_ref(),
        quick_cycle.stage_closeouts.product_planning.as_ref(),
        quick_cycle.stage_closeouts.solution_definition.as_ref(),
        quick_cycle.stage_closeouts.implementation.as_ref(),
        quick_cycle.stage_closeouts.validation_delivery.as_ref(),
    ];
    if closeouts.iter().flatten().any(|closeout| {
        closeout.summary.trim().is_empty()
            || closeout.summary.as_bytes().len() > MAX_QUICK_CYCLE_CLOSEOUT_SUMMARY_BYTES
            || !references_are_valid(&closeout.evidence_record_digests)
    }) {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line,
            reason: "Quick Cycle closeout exceeds its text, evidence, or ownership bound",
        });
    }
    if quick_cycle.expansion_history.iter().any(|expansion| {
        !expansion.phase.is_product_lifecycle_stage()
            || expansion.reason.trim().is_empty()
            || expansion.reason.as_bytes().len() > MAX_QUICK_CYCLE_EXPANSION_REASON_BYTES
            || !references_are_valid(&expansion.evidence_record_digests)
    }) {
        return Err(WorkflowGovernanceLedgerError::WorkFocusInvalid {
            line,
            reason: "Quick Cycle expansion exceeds its text, evidence, or ownership bound",
        });
    }
    Ok(())
}

fn validate_cooperative_evidence_event(
    projection: &WorkflowGovernanceLedgerProjection,
    event: &WorkflowCooperativeEvidenceObservedEvent,
    line: usize,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if projection.readiness_profile() != Some(WorkflowReadinessProfile::SoloCooperative) {
        return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
            line: Some(line),
            reason: "same-owner evidence requires the solo_cooperative readiness profile",
        });
    }
    if projection.latest_cooperative_objective().is_none() {
        return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
            line: Some(line),
            reason: "same-owner evidence requires an accepted cooperative objective",
        });
    }
    if projection.head_digest.as_deref() != Some(event.admission_ledger_head_digest.as_str())
        || projection.current_state_version() != Some(event.admission_state_version)
    {
        return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
            line: Some(line),
            reason: "evidence admission coordinates are stale",
        });
    }
    if let Some(offer_id) = event.offer_id.as_ref() {
        if let Some(previous) = projection.records.iter().find_map(|record| {
            let WorkflowGovernanceEvent::CooperativeEvidenceObserved(previous) = &record.event
            else {
                return None;
            };
            (previous.offer_id.as_ref() == Some(offer_id)).then_some(previous)
        }) {
            if event.disposition
                != forge_core_contracts::WorkflowCooperativeEvidenceDisposition::Rejected
                || event.rejection
                    != Some(
                        forge_core_contracts::WorkflowCooperativeEvidenceRejection::ConflictingIdempotencyKey,
                    )
                || previous.offer_digest == event.offer_digest
            {
                return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                    line: Some(line),
                    reason: "reused cooperative evidence offer id is not a distinct rejected conflict",
                });
            }
        } else if event.rejection
            == Some(
                forge_core_contracts::WorkflowCooperativeEvidenceRejection::ConflictingIdempotencyKey,
            )
        {
            return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                line: Some(line),
                reason: "cooperative evidence conflict has no original offer id",
            });
        }
    }
    if let Some(admitted) = event.admitted_evidence.as_ref() {
        let Some(objective_record) = projection.records.iter().rev().find(|record| {
            matches!(
                record.event,
                WorkflowGovernanceEvent::CooperativeObjectiveAccepted(_)
            )
        }) else {
            return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                line: Some(line),
                reason: "admitted evidence has no active objective record",
            });
        };
        let WorkflowGovernanceEvent::CooperativeObjectiveAccepted(objective) =
            &objective_record.event
        else {
            unreachable!("predicate guarantees cooperative objective");
        };
        let active_bundle_digest = projection
            .active_effective_bundle_identity()
            .map(|identity| identity.effective_runtime_bundle.bundle_digest)
            .or_else(|| {
                projection
                    .active_identity()
                    .map(|identity| identity.bundle_digest)
            });
        if admitted.binding.objective_id != objective.objective_id
            || admitted.binding.objective_revision != objective.revision
            || admitted.binding.objective_digest != objective.objective_digest
            || admitted.binding.assurance_epoch != objective.assurance_epoch
            || admitted.binding.accepted_objective_record_digest != objective_record.record_digest
            || admitted.binding.accepted_objective_record_sequence != objective_record.sequence
            || admitted.binding.snapshot_digest != event.admission_snapshot_digest
            || admitted.binding.ledger_head_digest != event.admission_ledger_head_digest
            || admitted.binding.state_version != event.admission_state_version
            || active_bundle_digest.as_deref()
                != Some(admitted.binding.policy_bundle_digest.as_str())
        {
            return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                line: Some(line),
                reason: "admitted evidence does not match the current objective, bundle, or admission coordinates",
            });
        }
    }
    validate_cooperative_evidence_shape(event, Some(line))
}

fn cooperative_source_basis_path_is_normalized(path: &str) -> bool {
    let mut components = path.split('/');
    let Some(first) = components.next() else {
        return false;
    };
    !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains('\\')
        && !first.is_empty()
        && !matches!(
            first,
            "." | ".." | ".git" | ".forge-method" | "target" | "node_modules"
        )
        && components.all(|component| !component.is_empty() && !matches!(component, "." | ".."))
}

fn validate_cooperative_evidence_shape(
    event: &WorkflowCooperativeEvidenceObservedEvent,
    line: Option<usize>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    use forge_core_contracts::WorkflowCooperativeEvidenceDisposition::{Admitted, Rejected};

    if event.observed_at_unix == 0 {
        return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
            line,
            reason: "evidence has no kernel observation time",
        });
    }
    for digest in [
        event.offer_digest.as_str(),
        event.admission_snapshot_digest.as_str(),
        event.admission_ledger_head_digest.as_str(),
    ] {
        if !is_lower_sha256(digest) {
            return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                line,
                reason: "evidence contains a non-canonical digest",
            });
        }
    }
    if event.offer_id.as_ref().is_some_and(|offer_id| {
        offer_id.0.trim().is_empty()
            || offer_id.0.len() > forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
    }) {
        return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
            line,
            reason: "evidence offer id is not bounded and nonblank",
        });
    }
    match (
        event.disposition,
        event.rejection,
        event.admitted_evidence.as_ref(),
    ) {
        (
            Rejected,
            Some(forge_core_contracts::WorkflowCooperativeEvidenceRejection::ConflictingIdempotencyKey),
            None,
        ) if event.offer_id.is_some() => Ok(()),
        (Rejected, Some(reason), None)
            if reason
                != forge_core_contracts::WorkflowCooperativeEvidenceRejection::ConflictingIdempotencyKey =>
        {
            Ok(())
        }
        (Admitted, None, Some(admitted)) => {
            let binding = &admitted.binding;
            let text_fields = [
                admitted.offer_id.0.as_str(),
                admitted.policy_version.as_str(),
                admitted.claim_descriptor_version.as_str(),
                admitted.policy_ref.0.as_str(),
                admitted.claim_ref.0.as_str(),
                admitted.evaluator_ref.0.as_str(),
                admitted.cooperative_claim_ref.0.as_str(),
                admitted.cooperative_evaluator_ref.0.as_str(),
                admitted.producer.0.as_str(),
                admitted.subject.subject_ref.as_str(),
            ];
            let legacy_shape = admitted.policy_version
                == forge_core_contracts::SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION_V1
                && admitted.claim_descriptor_version
                    == forge_core_contracts::SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION_V1
                && admitted
                    .target
                    .unwrap_or(forge_core_contracts::WorkflowCooperativeEvidenceTarget::SourceClaim)
                    == forge_core_contracts::WorkflowCooperativeEvidenceTarget::SourceClaim
                && admitted.scenario_kind
                    == forge_core_contracts::WorkflowCooperativeMaterialScenarioKind::KernelProjectSnapshotReadback
                && admitted.source_assessment.is_none()
                && admitted.applicability_assessment.is_none()
                && admitted.execution_assessment.is_none()
                && admitted.outcome == forge_core_contracts::WorkflowEvidenceOutcome::Pass;
            let source_shape = admitted.policy_version
                == forge_core_contracts::SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION
                && admitted.claim_descriptor_version
                    == forge_core_contracts::SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION
                && admitted
                    .target
                    .unwrap_or(forge_core_contracts::WorkflowCooperativeEvidenceTarget::SourceClaim)
                    == forge_core_contracts::WorkflowCooperativeEvidenceTarget::SourceClaim
                && admitted.applicability_assessment.is_none()
                && admitted.execution_assessment.is_none()
                && admitted.scenario_kind
                    == forge_core_contracts::WorkflowCooperativeMaterialScenarioKind::AgentRepositoryInspectionWithContentAddressedBasis
                && admitted.source_assessment.as_ref().is_some_and(|assessment| {
                    let text_is_bounded = !assessment.summary.trim().is_empty()
                        && assessment.summary.len()
                            <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
                        && assessment.limitations.len()
                            <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_LIMITATIONS
                        && assessment.limitations.iter().all(|limitation| {
                            !limitation.trim().is_empty()
                                && limitation.len()
                                    <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
                        });
                    let basis_is_bounded = (!assessment.basis.is_empty()
                        || !assessment.prior_evidence.is_empty())
                        && assessment.basis.len() + assessment.prior_evidence.len()
                            <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_BASIS_ITEMS
                        && assessment.basis.windows(2).all(|pair| {
                            pair[0].subject_ref < pair[1].subject_ref
                                || (pair[0].subject_ref == pair[1].subject_ref
                                    && pair[0].subject_digest < pair[1].subject_digest)
                        })
                        && assessment.basis.iter().all(|reference| {
                            cooperative_source_basis_path_is_normalized(&reference.subject_ref)
                                && reference.subject_ref.len()
                                    <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
                                && is_lower_sha256(&reference.subject_digest)
                        })
                        && is_lower_sha256(&assessment.basis_digest)
                        && to_canonical_json(&assessment.basis).is_ok_and(|canonical| {
                            format_sha256(Sha256::digest(canonical)) == assessment.basis_digest
                        })
                        && assessment.prior_evidence.windows(2).all(|pair| {
                            pair[0].record_digest < pair[1].record_digest
                        })
                        && assessment.prior_evidence.iter().all(|reference| {
                            is_lower_sha256(&reference.record_digest)
                                && reference.valid_through_unix.is_none_or(|valid_through| {
                                    valid_through >= reference.observed_at_unix
                                })
                        });
                    text_is_bounded
                        && basis_is_bounded
                        && admitted.outcome == assessment.outcome
                });
            let applicability_shape = admitted.policy_version
                == forge_core_contracts::SOLO_COOPERATIVE_APPLICABILITY_POLICY_VERSION
                && admitted.claim_descriptor_version
                    == forge_core_contracts::SOLO_COOPERATIVE_APPLICABILITY_DESCRIPTOR_VERSION
                && admitted.target
                    == Some(forge_core_contracts::WorkflowCooperativeEvidenceTarget::PolicyApplicability)
                && admitted.source_assessment.is_none()
                && admitted.execution_assessment.is_none()
                && admitted.scenario_kind
                    == forge_core_contracts::WorkflowCooperativeMaterialScenarioKind::AgentPolicyApplicabilityInspectionWithContentAddressedBasis
                && admitted
                    .applicability_assessment
                    .as_ref()
                    .is_some_and(|assessment| {
                        let text_is_bounded = !assessment.summary.trim().is_empty()
                            && assessment.summary.len()
                                <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
                            && assessment.limitations.len()
                                <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_LIMITATIONS
                            && assessment.limitations.iter().all(|limitation| {
                                !limitation.trim().is_empty()
                                    && limitation.len()
                                        <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
                            });
                        let basis_is_bounded = !assessment.basis.is_empty()
                            && assessment.basis.len()
                                <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_BASIS_ITEMS
                            && assessment.basis.windows(2).all(|pair| {
                                pair[0].subject_ref < pair[1].subject_ref
                                    || (pair[0].subject_ref == pair[1].subject_ref
                                        && pair[0].subject_digest < pair[1].subject_digest)
                            })
                            && assessment.basis.iter().all(|reference| {
                                cooperative_source_basis_path_is_normalized(&reference.subject_ref)
                                    && reference.subject_ref.len()
                                        <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
                                    && is_lower_sha256(&reference.subject_digest)
                            })
                            && is_lower_sha256(&assessment.basis_digest)
                            && to_canonical_json(&assessment.basis).is_ok_and(|canonical| {
                                format_sha256(Sha256::digest(canonical)) == assessment.basis_digest
                            });
                        let expected_outcome = match assessment.outcome {
                            forge_core_contracts::WorkflowCooperativeApplicabilityOutcome::Applicable => {
                                forge_core_contracts::WorkflowEvidenceOutcome::Pass
                            }
                            forge_core_contracts::WorkflowCooperativeApplicabilityOutcome::NotApplicable => {
                                forge_core_contracts::WorkflowEvidenceOutcome::Fail
                            }
                            forge_core_contracts::WorkflowCooperativeApplicabilityOutcome::Inconclusive => {
                                forge_core_contracts::WorkflowEvidenceOutcome::Inconclusive
                            }
                        };
                        text_is_bounded && basis_is_bounded && admitted.outcome == expected_outcome
                    });
            let execution_shape = admitted.policy_version
                == forge_core_contracts::SOLO_COOPERATIVE_EXECUTION_POLICY_VERSION
                && admitted.claim_descriptor_version
                    == forge_core_contracts::SOLO_COOPERATIVE_EXECUTION_DESCRIPTOR_VERSION
                && admitted
                    .target
                    .unwrap_or(forge_core_contracts::WorkflowCooperativeEvidenceTarget::SourceClaim)
                    == forge_core_contracts::WorkflowCooperativeEvidenceTarget::SourceClaim
                && admitted.source_assessment.is_none()
                && admitted.applicability_assessment.is_none()
                && admitted.scenario_kind
                    == forge_core_contracts::WorkflowCooperativeMaterialScenarioKind::KernelDeterministicCommandExecution
                && admitted.execution_assessment.as_ref().is_some_and(|assessment| {
                    let text_is_bounded = !assessment.summary.trim().is_empty()
                        && assessment.summary.len()
                            <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
                        && !assessment.scenario_ref.trim().is_empty()
                        && assessment.scenario_ref.len()
                            <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
                        && assessment.limitations.len()
                            <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_LIMITATIONS
                        && assessment.limitations.iter().all(|limitation| {
                            !limitation.trim().is_empty()
                                && limitation.len()
                                    <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
                        })
                        && assessment.stdout.len()
                            <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_INPUT_BYTES
                        && assessment.stderr.len()
                            <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_INPUT_BYTES
                        && assessment.reasons.len() <= 16
                        && assessment.reasons.iter().all(|reason| {
                            !reason.trim().is_empty()
                                && reason.len()
                                    <= forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
                        });
                    let expected_outcome = match assessment.status {
                        forge_core_contracts::WorkflowCooperativeExecutionStatus::Succeeded => {
                            forge_core_contracts::WorkflowEvidenceOutcome::Pass
                        }
                        forge_core_contracts::WorkflowCooperativeExecutionStatus::Failed
                            if assessment.exit_code.is_some() =>
                        {
                            forge_core_contracts::WorkflowEvidenceOutcome::Fail
                        }
                        forge_core_contracts::WorkflowCooperativeExecutionStatus::Failed
                        | forge_core_contracts::WorkflowCooperativeExecutionStatus::TimedOut => {
                            forge_core_contracts::WorkflowEvidenceOutcome::Inconclusive
                        }
                    };
                    text_is_bounded
                        && is_lower_sha256(&assessment.command_digest)
                        && assessment.timed_out
                            == (assessment.status
                                == forge_core_contracts::WorkflowCooperativeExecutionStatus::TimedOut)
                        && admitted.outcome == expected_outcome
                });
            if text_fields.iter().any(|value| {
                value.trim().is_empty()
                    || value.len()
                        > forge_core_contracts::MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES
            }) || admitted.offer_digest != event.offer_digest
                || event.offer_id.as_ref() != Some(&admitted.offer_id)
                || (!legacy_shape && !source_shape && !applicability_shape && !execution_shape)
                || binding.objective_revision == 0
                || binding.assurance_epoch == 0
                || binding.accepted_objective_record_sequence == 0
                || admitted.execution_observed_at_unix != event.observed_at_unix
                || admitted.readback_observed_at_unix != event.observed_at_unix
                || admitted.subject.kind
                    != forge_core_contracts::WorkflowEvidenceSubjectKind::ProjectSnapshot
            {
                return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                    line,
                    reason: "admitted evidence has invalid bounded fields, subject, or observation times",
                });
            }
            for digest in [
                binding.objective_digest.as_str(),
                binding.accepted_objective_record_digest.as_str(),
                binding.policy_bundle_digest.as_str(),
                binding.snapshot_digest.as_str(),
                binding.ledger_head_digest.as_str(),
                admitted.subject.subject_digest.as_str(),
                admitted.scenario_digest.as_str(),
            ] {
                if !is_lower_sha256(digest) {
                    return Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
                        line,
                        reason: "admitted evidence contains a non-canonical digest",
                    });
                }
            }
            Ok(())
        }
        _ => Err(WorkflowGovernanceLedgerError::CooperativeEvidenceInvalid {
            line,
            reason: "evidence disposition, rejection, and normalized admission are inconsistent",
        }),
    }
}

fn validate_cooperative_objective_event(
    projection: &WorkflowGovernanceLedgerProjection,
    event: &forge_core_contracts::CooperativeObjectiveAcceptedEvent,
    line: usize,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if projection.readiness_profile() != Some(WorkflowReadinessProfile::SoloCooperative) {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line: Some(line),
            reason: "same-owner objective requires the solo_cooperative readiness profile",
        });
    }
    if projection.contains_human_intent_revision() {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line: Some(line),
            reason: "cooperative objective cannot follow strict human intent authority",
        });
    }
    if projection.head_digest.as_deref() != Some(event.ledger_head_digest.as_str()) {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line: Some(line),
            reason: "objective ledger-head binding is stale",
        });
    }
    validate_cooperative_objective_transition(
        projection.latest_cooperative_objective(),
        event,
        Some(line),
    )
}

fn validate_cooperative_objective_transition(
    previous: Option<&forge_core_contracts::CooperativeObjectiveAcceptedEvent>,
    event: &forge_core_contracts::CooperativeObjectiveAcceptedEvent,
    line: Option<usize>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    validate_cooperative_objective_shape(event, line)?;
    let Some(previous) = previous else {
        if event.revision != 1
            || event.assurance_epoch != 1
            || event.previous_objective_digest.is_some()
            || event.revision_kind != WorkflowCooperativeObjectiveRevisionKind::Initial
            || event.revision_reason.is_some()
            || event.revision_input_digest.is_some()
        {
            return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                line,
                reason: "initial cooperative objective must use revision and epoch one with no predecessor or revision metadata",
            });
        }
        return Ok(());
    };

    let expected_revision = previous.revision.checked_add(1).ok_or(
        WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line,
            reason: "cooperative objective revision overflow",
        },
    )?;
    let expected_epoch = previous.assurance_epoch.checked_add(1).ok_or(
        WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line,
            reason: "cooperative objective assurance epoch overflow",
        },
    )?;
    if event.objective_id != previous.objective_id
        || event.revision != expected_revision
        || event.assurance_epoch != expected_epoch
        || event.previous_objective_digest.as_deref() != Some(previous.objective_digest.as_str())
    {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line,
            reason: "cooperative objective revision must be adjacent and bind the active objective digest",
        });
    }
    let reason = event.revision_reason.as_deref().unwrap_or_default();
    if reason.trim().is_empty() || reason.len() > MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line,
            reason: "cooperative objective revision reason is missing or exceeds its bound",
        });
    }
    let revision_input = match event.revision_kind {
        WorkflowCooperativeObjectiveRevisionKind::Initial => {
            return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                line,
                reason: "a successor objective cannot claim initial revision kind",
            });
        }
        WorkflowCooperativeObjectiveRevisionKind::MaterialSupersession => {
            if event.proposal == previous.proposal {
                return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                    line,
                    reason: "material supersession must change the objective proposal",
                });
            }
            WorkflowCooperativeObjectiveInput::MaterialSupersession {
                proposal: event.proposal.clone(),
                supersession_reason: reason.to_owned(),
                carrying_principal: event.carrying_principal.clone(),
                host_provenance: event.host_provenance.clone(),
            }
        }
        WorkflowCooperativeObjectiveRevisionKind::NonMaterialClarification => {
            if event.proposal.outcome != previous.proposal.outcome
                || event.proposal == previous.proposal
            {
                return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                    line,
                    reason: "non-material clarification must be additive-only and preserve outcome",
                });
            }
            let added_constraints = cooperative_unique_additive_suffix(
                &previous.proposal.constraints,
                &event.proposal.constraints,
            )
            .ok_or(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                line,
                reason:
                    "non-material clarification cannot repeat prior or duplicate appended details",
            })?;
            let added_unacceptable_outcomes = cooperative_unique_additive_suffix(
                &previous.proposal.unacceptable_outcomes,
                &event.proposal.unacceptable_outcomes,
            )
            .ok_or(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                line,
                reason:
                    "non-material clarification cannot repeat prior or duplicate appended details",
            })?;
            let added_open_uncertainties = cooperative_unique_additive_suffix(
                &previous.proposal.open_uncertainties,
                &event.proposal.open_uncertainties,
            )
            .ok_or(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                line,
                reason:
                    "non-material clarification cannot repeat prior or duplicate appended details",
            })?;
            WorkflowCooperativeObjectiveInput::NonMaterialClarification {
                added_constraints,
                added_unacceptable_outcomes,
                added_open_uncertainties,
                clarification_reason: reason.to_owned(),
                carrying_principal: event.carrying_principal.clone(),
                host_provenance: event.host_provenance.clone(),
            }
        }
    };
    let expected_input_digest = cooperative_revision_input_digest(&revision_input)?;
    if event.revision_input_digest.as_deref() != Some(expected_input_digest.as_str()) {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line,
            reason: "cooperative objective revision input digest does not match the exact accepted input",
        });
    }
    Ok(())
}

fn cooperative_unique_additive_suffix(
    previous: &[String],
    current: &[String],
) -> Option<Vec<String>> {
    let suffix = current.strip_prefix(previous)?;
    let mut seen = HashSet::new();
    if suffix
        .iter()
        .any(|value| previous.contains(value) || !seen.insert(value))
    {
        return None;
    }
    Some(suffix.to_vec())
}

fn cooperative_revision_input_digest(
    input: &WorkflowCooperativeObjectiveInput,
) -> Result<String, WorkflowGovernanceLedgerError> {
    let canonical = to_canonical_json(input).map_err(|error| {
        WorkflowGovernanceLedgerError::Canonicalization {
            source: error.to_string(),
        }
    })?;
    Ok(format_sha256(Sha256::digest(canonical)))
}

fn validate_cooperative_objective_shape(
    event: &forge_core_contracts::CooperativeObjectiveAcceptedEvent,
    line: Option<usize>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if event.revision == 0 || event.assurance_epoch == 0 {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line,
            reason: "cooperative objective revision and assurance epoch must be nonzero",
        });
    }
    if event.authority_basis != WorkflowCooperativeAuthorityBasis::CooperativeSameOwner {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line,
            reason: "objective authority basis is not cooperative_same_owner",
        });
    }
    for digest in [
        event.objective_digest.as_str(),
        event.snapshot_digest.as_str(),
        event.ledger_head_digest.as_str(),
        event.acceptance_action_packet_digest.as_str(),
        event.host_provenance.conversation_digest.as_str(),
    ] {
        if !is_lower_sha256(digest) {
            return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                line,
                reason: "objective contains a non-canonical digest",
            });
        }
    }
    if event
        .revision_input_digest
        .as_deref()
        .is_some_and(|digest| !is_lower_sha256(digest))
    {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line,
            reason: "objective revision input digest is non-canonical",
        });
    }
    let semantic_digest = cooperative_objective_digest(event)?;
    if event.objective_digest != semantic_digest {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line,
            reason: "objective digest does not match the canonical objective semantics",
        });
    }
    if event.objective_id.0.trim().is_empty()
        || event.objective_id.0.len() > MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES
        || event.carrying_principal.0.trim().is_empty()
        || event.carrying_principal.0.len() > MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES
        || event.proposal.outcome.trim().is_empty()
        || event.host_provenance.host_id.0.trim().is_empty()
        || event.host_provenance.host_version.trim().is_empty()
        || event.host_provenance.session_ref.trim().is_empty()
        || event.host_provenance.interaction_ref.trim().is_empty()
        || event.accepted_at_unix == 0
        || event.host_provenance.observed_at_unix == 0
        || event.host_provenance.observed_at_unix > event.accepted_at_unix
    {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line,
            reason: "objective identity, principal, proposal, host coordinates, and ordered clocks must satisfy the bounded wire shape",
        });
    }
    for value in [
        event.host_provenance.host_id.0.as_str(),
        event.host_provenance.host_version.as_str(),
        event.host_provenance.session_ref.as_str(),
        event.host_provenance.interaction_ref.as_str(),
    ] {
        if value.len() > MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES {
            return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                line,
                reason: "objective host provenance exceeds its bounded wire shape",
            });
        }
    }
    if event.proposal.outcome.len() > MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line,
            reason: "objective outcome exceeds its bounded wire shape",
        });
    }
    let mut proposal_bytes = event.proposal.outcome.len();
    for values in [
        &event.proposal.constraints,
        &event.proposal.unacceptable_outcomes,
        &event.proposal.open_uncertainties,
    ] {
        if values.len() > MAX_WORKFLOW_INTENT_LIST_ITEMS
            || values.iter().any(|value| {
                value.trim().is_empty() || value.len() > MAX_WORKFLOW_INTENT_ITEM_BYTES
            })
        {
            return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                line,
                reason: "objective proposal lists exceed their bounded wire shape",
            });
        }
        proposal_bytes =
            proposal_bytes.saturating_add(values.iter().map(String::len).sum::<usize>());
    }
    if proposal_bytes > MAX_WORKFLOW_INTENT_TOTAL_BYTES {
        return Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
            line,
            reason: "objective proposal exceeds its aggregate wire bound",
        });
    }
    Ok(())
}

fn cooperative_objective_digest(
    event: &forge_core_contracts::CooperativeObjectiveAcceptedEvent,
) -> Result<String, WorkflowGovernanceLedgerError> {
    let subject = serde_json::json!({
        "objective_id": event.objective_id,
        "revision": event.revision,
        "assurance_epoch": event.assurance_epoch,
        "proposal": event.proposal,
    });
    let canonical = to_canonical_json(&subject).map_err(|error| {
        WorkflowGovernanceLedgerError::Canonicalization {
            source: error.to_string(),
        }
    })?;
    Ok(format_sha256(Sha256::digest(canonical)))
}

fn validate_broker_origin_replay_digest(
    event: &WorkflowGovernanceEvent,
    line: Option<usize>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    let WorkflowGovernanceEvent::BrokerOriginApplied(origin) = event else {
        return Ok(());
    };
    if origin
        .native_interaction_replay_digest
        .as_deref()
        .is_some_and(|digest| !is_sha256_digest(digest))
    {
        return Err(WorkflowGovernanceLedgerError::InvalidBrokerOriginBinding {
            line,
            reason: "native interaction replay digest is invalid",
        });
    }
    if origin.native_interaction_replay_digest.is_some() && origin.native_host_provenance.is_none()
    {
        return Err(WorkflowGovernanceLedgerError::InvalidBrokerOriginBinding {
            line,
            reason: "strict replay identity requires native host provenance",
        });
    }
    Ok(())
}

fn require_strict_broker_origin_replay_digest(
    event: &WorkflowGovernanceEvent,
    line: Option<usize>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if matches!(
        event,
        WorkflowGovernanceEvent::BrokerOriginApplied(origin)
            if origin.native_interaction_replay_digest.is_none()
    ) {
        return Err(WorkflowGovernanceLedgerError::InvalidBrokerOriginBinding {
            line,
            reason: "strict ledger epoch requires native interaction replay identity",
        });
    }
    Ok(())
}

fn validate_nonblank(
    value: &str,
    line: Option<usize>,
    field: &'static str,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if value.trim().is_empty() {
        return Err(WorkflowGovernanceLedgerError::EmptyField { line, field });
    }
    Ok(())
}

fn empty_projection() -> WorkflowGovernanceLedgerProjection {
    WorkflowGovernanceLedgerProjection {
        records: Vec::new(),
        head_digest: None,
        next_sequence: 1,
        next_state_version: 0,
    }
}

fn workflow_governance_wal_path(
    state_root: &Path,
) -> Result<PathBuf, WorkflowGovernanceLedgerError> {
    resolve_safe_repo_relative(state_root, WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH).map_err(|error| {
        WorkflowGovernanceLedgerError::Io {
            path: state_root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH),
            source: error.to_string(),
        }
    })
}

fn trusted_state_root(path: &Path) -> Result<PathBuf, WorkflowGovernanceLedgerError> {
    let metadata = fs::metadata(path).map_err(|source| {
        WorkflowGovernanceLedgerError::StateRootUnavailable {
            path: path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    if !metadata.is_dir() {
        return Err(WorkflowGovernanceLedgerError::StateRootUnavailable {
            path: path.to_path_buf(),
            source: "path is not a directory".to_owned(),
        });
    }
    fs::canonicalize(path).map_err(
        |source| WorkflowGovernanceLedgerError::StateRootUnavailable {
            path: path.to_path_buf(),
            source: source.to_string(),
        },
    )
}

fn unique_record_id(
    existing: &[WorkflowGovernanceLedgerRecord],
) -> Result<StableId, WorkflowGovernanceLedgerError> {
    for _ in 0..8 {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).map_err(|error| WorkflowGovernanceLedgerError::Randomness {
            source: error.to_string(),
        })?;
        let candidate = StableId(format!("wglr-{}", hex(&bytes)));
        if existing.iter().all(|record| record.record_id != candidate) {
            return Ok(candidate);
        }
    }
    Err(WorkflowGovernanceLedgerError::Randomness {
        source: "record id collided eight times".to_owned(),
    })
}

fn unix_time() -> Result<u64, WorkflowGovernanceLedgerError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| WorkflowGovernanceLedgerError::Clock {
            source: error.to_string(),
        })
}

fn hex(bytes: &[u8]) -> String {
    use fmt::Write as _;
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(value, "{byte:02x}");
    }
    value
}

fn format_sha256(bytes: impl AsRef<[u8]>) -> String {
    format!("sha256:{}", hex(bytes.as_ref()))
}

fn resolve_safe_repo_relative(root: &Path, relative_path: &str) -> io::Result<PathBuf> {
    let path = Path::new(relative_path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(forbidden_relative_component)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "workflow-governance path must be a safe relative path",
        ));
    }

    let canonical_root = fs::canonicalize(root)?;
    let components = path_components(path);
    if components.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "workflow-governance path has no normal components",
        ));
    }

    let mut resolved = canonical_root.clone();
    for (index, component) in components.iter().enumerate() {
        let candidate = resolved.join(component);
        if candidate.exists() {
            let canonical_candidate = fs::canonicalize(&candidate)?;
            if !canonical_candidate.starts_with(&canonical_root) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "workflow-governance path escapes the trusted state root",
                ));
            }
            resolved = canonical_candidate;
        } else {
            resolved = candidate;
            for remaining in components.iter().skip(index + 1) {
                resolved.push(remaining);
            }
            break;
        }
    }

    if resolved_parent_stays_within_root(&canonical_root, &resolved) {
        Ok(resolved)
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "workflow-governance target parent escapes the trusted state root",
        ))
    }
}

fn forbidden_relative_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Prefix(_) | Component::RootDir | Component::ParentDir
    )
}

fn path_components(path: &Path) -> Vec<OsString> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_os_string()),
            _ => None,
        })
        .collect()
}

fn resolved_parent_stays_within_root(canonical_root: &Path, resolved: &Path) -> bool {
    let Some(parent) = resolved.parent() else {
        return false;
    };
    if parent.exists() {
        return fs::canonicalize(parent)
            .is_ok_and(|canonical_parent| canonical_parent.starts_with(canonical_root));
    }

    let mut ancestor = parent;
    while !ancestor.exists() {
        let Some(next) = ancestor.parent() else {
            return false;
        };
        ancestor = next;
    }
    fs::canonicalize(ancestor).is_ok_and(|canonical_ancestor| {
        canonical_ancestor.starts_with(canonical_root) || resolved.starts_with(canonical_root)
    })
}

fn ensure_resolved_parent_within_root(root: &Path, target: &Path) -> io::Result<()> {
    let canonical_root = fs::canonicalize(root)?;
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    let canonical_parent = fs::canonicalize(parent)?;
    if canonical_parent.starts_with(&canonical_root) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "workflow-governance target parent escapes the trusted state root",
        ))
    }
}

const REPLACEMENT_PROTOCOL_VERSION: &str = "forge-wal-replacement-v1";
const REPLACEMENT_NEXT_SUFFIX: &str = "forge-next";
const REPLACEMENT_PREVIOUS_SUFFIX: &str = "forge-previous";
const REPLACEMENT_TRANSACTION_SUFFIX: &str = "forge-transaction";
const REPLACEMENT_MARKER_MAX_BYTES: u64 = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplacementMarker {
    previous_digest: Option<String>,
    next_digest: String,
}

#[derive(Debug, Clone)]
struct ReplacementPaths {
    next: PathBuf,
    previous: PathBuf,
    transaction: PathBuf,
}

#[cfg(any(not(unix), test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplacementCrashPoint {
    NextSynced,
    TransactionSynced,
    PreviousInstalled,
    TargetInstalled,
}

#[cfg(test)]
thread_local! {
    static REPLACEMENT_CRASH_POINT: Cell<Option<ReplacementCrashPoint>> = const { Cell::new(None) };
}

#[cfg(any(not(unix), test))]
fn maybe_inject_replacement_crash(point: ReplacementCrashPoint) {
    #[cfg(test)]
    REPLACEMENT_CRASH_POINT.with(|configured| {
        assert!(
            configured.get() != Some(point),
            "injected WAL replacement crash at {point:?}"
        );
    });
    #[cfg(not(test))]
    let _ = point;
}

fn atomic_replace_file(target: &Path, content: &[u8]) -> io::Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    fs::create_dir_all(parent)?;
    reconcile_wal_replacement(target)?;

    #[cfg(unix)]
    return atomic_replace_file_unix(target, content);
    #[cfg(not(unix))]
    replace_file_with_recovery_protocol(target, content)
}

#[cfg(unix)]
fn atomic_replace_file_unix(target: &Path, content: &[u8]) -> io::Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    let nonce = transaction_nonce();
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no file name"))?;
    let temp = parent.join(format!(".{file_name}.{nonce}.forge-tmp"));
    let write_result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(content)?;
        file.sync_all()
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }

    if let Err(error) = fs::rename(&temp, target) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    sync_parent_dir(parent)
}

#[cfg(any(not(unix), test))]
fn replace_file_with_recovery_protocol(target: &Path, content: &[u8]) -> io::Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    let paths = replacement_paths(target)?;
    let previous_digest = file_digest_if_regular(target)?;
    let marker = ReplacementMarker {
        previous_digest,
        next_digest: sha256_digest(content),
    };

    // Each fixed artifact and its retained parent namespace are synced before
    // the subsequent namespace change. Durability failures are always fatal.
    write_new_synced_file(&paths.next, content)?;
    sync_parent_dir(parent)?;
    maybe_inject_replacement_crash(ReplacementCrashPoint::NextSynced);

    if let Err(error) = write_new_synced_file(&paths.transaction, &encode_marker(&marker)) {
        let _ = fs::remove_file(&paths.transaction);
        let _ = fs::remove_file(&paths.next);
        return Err(error);
    }
    sync_parent_dir(parent)?;
    maybe_inject_replacement_crash(ReplacementCrashPoint::TransactionSynced);

    if marker.previous_digest.is_some() {
        if let Err(error) = fs::rename(target, &paths.previous) {
            let _ = fs::remove_file(&paths.transaction);
            let _ = fs::remove_file(&paths.next);
            return Err(error);
        }
        sync_parent_dir(parent)?;
        maybe_inject_replacement_crash(ReplacementCrashPoint::PreviousInstalled);
    }

    fs::rename(&paths.next, target)?;
    sync_parent_dir(parent)?;
    maybe_inject_replacement_crash(ReplacementCrashPoint::TargetInstalled);

    if marker.previous_digest.is_some() {
        fs::remove_file(&paths.previous)?;
        sync_parent_dir(parent)?;
    }
    fs::remove_file(&paths.transaction)?;
    sync_parent_dir(parent)
}

fn reconcile_wal_replacement(target: &Path) -> io::Result<()> {
    let paths = replacement_paths(target)?;
    let marker_bytes = read_regular_file_bounded(
        &paths.transaction,
        REPLACEMENT_MARKER_MAX_BYTES,
        "replacement transaction marker",
    )?;

    let Some(marker_bytes) = marker_bytes else {
        return reconcile_without_marker(
            target,
            &paths,
            regular_file_exists(target, "workflow-governance WAL target")?,
            regular_file_exists(&paths.next, "replacement next WAL")?,
            regular_file_exists(&paths.previous, "replacement previous WAL")?,
        );
    };
    let marker = parse_marker(&marker_bytes)?;
    let target_digest = file_digest_if_regular(target)?;
    let next_digest = file_digest_if_regular(&paths.next)?;
    let previous_digest = file_digest_if_regular(&paths.previous)?;
    reconcile_with_marker(
        target,
        &paths,
        &marker,
        target_digest.as_deref(),
        next_digest.as_deref(),
        previous_digest.as_deref(),
    )
}

fn reconcile_without_marker(
    target: &Path,
    paths: &ReplacementPaths,
    target_exists: bool,
    next_exists: bool,
    previous_exists: bool,
) -> io::Result<()> {
    if previous_exists {
        return protocol_error("previous WAL exists without a transaction marker");
    }
    if next_exists {
        if !target_exists {
            return protocol_error("next WAL exists without a marker or durable target");
        }
        fs::remove_file(&paths.next)?;
        sync_target_parent(target)?;
    }
    Ok(())
}

fn reconcile_with_marker(
    target: &Path,
    paths: &ReplacementPaths,
    marker: &ReplacementMarker,
    target_digest: Option<&str>,
    next_digest: Option<&str>,
    previous_digest: Option<&str>,
) -> io::Result<()> {
    ensure_optional_digest_matches("next WAL", next_digest, &marker.next_digest)?;
    if let Some(expected_previous) = marker.previous_digest.as_deref() {
        ensure_optional_digest_matches("previous WAL", previous_digest, expected_previous)?;
    } else if previous_digest.is_some() {
        return protocol_error("unexpected previous WAL for an initially empty transaction");
    }

    match target_digest {
        Some(found) if found == marker.next_digest.as_str() => {
            if next_digest.is_some() {
                return protocol_error("committed target coexists with a next WAL");
            }
            finish_committed_cleanup(target, paths, marker, previous_digest.is_some())
        }
        Some(found) if marker.previous_digest.as_deref() == Some(found) => {
            if previous_digest.is_some() {
                return protocol_error("old target coexists with a previous WAL");
            }
            finish_aborted_cleanup(target, paths, next_digest.is_some())
        }
        Some(_) => protocol_error("target digest is not bound by the transaction marker"),
        None => recover_missing_target(
            target,
            paths,
            marker,
            next_digest.is_some(),
            previous_digest.is_some(),
        ),
    }
}

fn recover_missing_target(
    target: &Path,
    paths: &ReplacementPaths,
    marker: &ReplacementMarker,
    next_exists: bool,
    previous_exists: bool,
) -> io::Result<()> {
    if marker.previous_digest.is_some() {
        if !previous_exists {
            return protocol_error("target and marker-bound previous WAL are both missing");
        }
        fs::rename(&paths.previous, target)?;
        sync_target_parent(target)?;
        return finish_aborted_cleanup(target, paths, next_exists);
    }
    if previous_exists || !next_exists {
        return protocol_error("initial replacement transaction is incomplete or inconsistent");
    }
    fs::rename(&paths.next, target)?;
    sync_target_parent(target)?;
    finish_committed_cleanup(target, paths, marker, false)
}

fn finish_committed_cleanup(
    target: &Path,
    paths: &ReplacementPaths,
    marker: &ReplacementMarker,
    has_previous: bool,
) -> io::Result<()> {
    if has_previous {
        fs::remove_file(&paths.previous)?;
        sync_target_parent(target)?;
    } else if marker.previous_digest.is_none() {
        // No previous WAL is expected for initialization.
    }
    fs::remove_file(&paths.transaction)?;
    sync_target_parent(target)
}

fn finish_aborted_cleanup(
    target: &Path,
    paths: &ReplacementPaths,
    has_next: bool,
) -> io::Result<()> {
    if has_next {
        fs::remove_file(&paths.next)?;
        sync_target_parent(target)?;
    }
    fs::remove_file(&paths.transaction)?;
    sync_target_parent(target)
}

fn ensure_optional_digest_matches(
    label: &str,
    found: Option<&str>,
    expected: &str,
) -> io::Result<()> {
    if found.is_some_and(|digest| digest != expected) {
        protocol_error(&format!(
            "{label} digest does not match the transaction marker"
        ))
    } else {
        Ok(())
    }
}

fn replacement_paths(target: &Path) -> io::Result<ReplacementPaths> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no file name"))?;
    Ok(ReplacementPaths {
        next: parent.join(format!(".{file_name}.{REPLACEMENT_NEXT_SUFFIX}")),
        previous: parent.join(format!(".{file_name}.{REPLACEMENT_PREVIOUS_SUFFIX}")),
        transaction: parent.join(format!(".{file_name}.{REPLACEMENT_TRANSACTION_SUFFIX}")),
    })
}

#[cfg(any(not(unix), test))]
fn encode_marker(marker: &ReplacementMarker) -> Vec<u8> {
    let previous = marker.previous_digest.as_deref().unwrap_or("absent");
    format!(
        "{REPLACEMENT_PROTOCOL_VERSION}\nprevious={previous}\nnext={}\n",
        marker.next_digest
    )
    .into_bytes()
}

fn parse_marker(bytes: &[u8]) -> io::Result<ReplacementMarker> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| protocol_io_error("replacement marker is not UTF-8"))?;
    if !text.ends_with('\n') {
        return protocol_error("replacement marker has a torn tail");
    }
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() != 3 || lines[0] != REPLACEMENT_PROTOCOL_VERSION {
        return protocol_error("replacement marker has an unsupported shape or version");
    }
    let previous = lines[1]
        .strip_prefix("previous=")
        .ok_or_else(|| protocol_io_error("replacement marker has no previous digest"))?;
    let next = lines[2]
        .strip_prefix("next=")
        .ok_or_else(|| protocol_io_error("replacement marker has no next digest"))?;
    let previous_digest = if previous == "absent" {
        None
    } else {
        validate_sha256_digest(previous)?;
        Some(previous.to_owned())
    };
    validate_sha256_digest(next)?;
    Ok(ReplacementMarker {
        previous_digest,
        next_digest: next.to_owned(),
    })
}

fn validate_sha256_digest(value: &str) -> io::Result<()> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return protocol_error("replacement marker digest has no sha256 prefix");
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return protocol_error("replacement marker digest is not lowercase sha256 hex");
    }
    Ok(())
}

fn file_digest_if_regular(path: &Path) -> io::Result<Option<String>> {
    read_regular_file_bounded(
        path,
        WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES,
        "replacement protocol file",
    )
    .map(|content| content.map(|bytes| sha256_digest(&bytes)))
}

fn regular_file_exists(path: &Path, label: &str) -> io::Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_file() {
        return protocol_error(&format!("{label} is not a confined regular file"));
    }
    Ok(true)
}

fn read_regular_file_bounded(
    path: &Path,
    maximum: u64,
    label: &str,
) -> io::Result<Option<Vec<u8>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_file() {
        return protocol_error(&format!("{label} is not a confined regular file"));
    }
    if metadata.len() > maximum {
        return protocol_error(&format!("{label} exceeds its maximum size"));
    }
    fs::read(path).map(Some)
}

#[cfg(any(not(unix), test))]
fn write_new_synced_file(path: &Path, content: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = file.write_all(content).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}

fn sha256_digest(content: &[u8]) -> String {
    format_sha256(Sha256::digest(content))
}

fn sync_target_parent(target: &Path) -> io::Result<()> {
    target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))
        .and_then(sync_parent_dir)
}

fn protocol_error<T>(message: &str) -> io::Result<T> {
    Err(protocol_io_error(message))
}

fn protocol_io_error(message: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("workflow-governance replacement protocol: {message}"),
    )
}

#[cfg(unix)]
fn sync_parent_dir(parent: &Path) -> io::Result<()> {
    File::open(parent)?.sync_all()
}

#[cfg(windows)]
fn sync_parent_dir(parent: &Path) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt as _;
    use std::os::windows::fs::OpenOptionsExt as _;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    if parent.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "parent directory is empty",
        ));
    }
    let directory = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(parent)?;
    let metadata = directory.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 == 0x400 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "workflow-governance WAL parent is not a real, non-reparse directory",
        ));
    }
    directory.sync_all()
}

#[cfg(all(not(unix), not(windows)))]
fn sync_parent_dir(parent: &Path) -> io::Result<()> {
    if parent.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "parent directory is empty",
        ));
    }
    File::open(parent)?.sync_all()
}

#[cfg(unix)]
fn transaction_nonce() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{}-{nanos}", std::process::id())
}

// `Result::map_err` transfers ownership; retaining the owned adapter avoids a
// closure at every lock call while the stored error remains string-only.
#[allow(clippy::needless_pass_by_value)]
fn lock_error(error: EffectStoreLockError) -> WorkflowGovernanceLedgerError {
    WorkflowGovernanceLedgerError::Lock {
        source: error.to_string(),
    }
}

// IO errors arrive by value from `map_err` and are immediately normalized into
// the stable public error shape.
#[allow(clippy::needless_pass_by_value)]
fn io_error(path: &Path, source: std::io::Error) -> WorkflowGovernanceLedgerError {
    WorkflowGovernanceLedgerError::Io {
        path: path.to_path_buf(),
        source: source.to_string(),
    }
}

#[cfg(test)]
mod replacement_protocol_tests {
    use super::*;
    use forge_core_contracts::{
        CooperativeObjectiveAcceptedEvent, HumanIntentRevisionAcceptedEvent, PhaseAdvancedEvent,
        PrincipalId, ProjectImportedEvent, RepoPath, SignalChangedEvent,
        WorkflowCooperativeHostProvenance, WorkflowCooperativeObjectiveProposal,
        WorkflowCurrentWorkAuthority, WorkflowGovernanceSignal, WorkflowHumanIntentRevision,
        WorkflowReceiptCarryover, WorkflowReleaseAdmissionProof, WorkflowReleaseRegistryProvenance,
        WorkflowWorkFocusObjectiveBinding,
    };
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn historical_source_basis_allows_normalized_top_level_local_path() {
        assert!(cooperative_source_basis_path_is_normalized(
            ".local/journal.md"
        ));
    }

    fn test_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "forge-wal-replacement-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create root");
        root
    }

    fn test_identity() -> WorkflowGovernanceLedgerIdentity {
        WorkflowGovernanceLedgerIdentity {
            project_id: StableId("project-protocol-test".to_owned()),
            bundle_id: StableId("bundle-protocol-test".to_owned()),
            bundle_digest: sha256_digest(b"bundle-protocol-test"),
        }
    }

    fn test_release_identity(version: &str) -> WorkflowGovernanceReleaseIdentity {
        WorkflowGovernanceReleaseIdentity {
            lineage_id: StableId("release-lineage".to_owned()),
            release_id: StableId(format!("release-{version}")),
            release_version: version.to_owned(),
            release_digest: sha256_digest(format!("release-{version}").as_bytes()),
        }
    }

    fn test_release_event(
        head: &str,
        source: &WorkflowGovernanceLedgerIdentity,
        target: &WorkflowGovernanceLedgerIdentity,
    ) -> ReleaseUpgradedEvent {
        let from_policy = sha256_digest(b"policy-v1");
        let to_policy = sha256_digest(b"policy-v2");
        ReleaseUpgradedEvent {
            from_release: test_release_identity("1.0.0"),
            to_release: test_release_identity("2.0.0"),
            from_runtime_bundle: WorkflowRuntimeBundleIdentity {
                bundle_id: source.bundle_id.clone(),
                bundle_digest: source.bundle_digest.clone(),
                policy_set_digest: from_policy.clone(),
            },
            to_runtime_bundle: WorkflowRuntimeBundleIdentity {
                bundle_id: target.bundle_id.clone(),
                bundle_digest: target.bundle_digest.clone(),
                policy_set_digest: to_policy.clone(),
            },
            registry_provenance: WorkflowReleaseRegistryProvenance {
                registry_id: StableId("registry".to_owned()),
                registry_version: "1.0.0".to_owned(),
                registry_digest: sha256_digest(b"registry"),
            },
            admission_proof: WorkflowReleaseAdmissionProof {
                proof_id: StableId("proof".to_owned()),
                proof_digest: sha256_digest(b"proof"),
                snapshot_digest: sha256_digest(b"snapshot"),
                from_policy_set_digest: from_policy,
                to_policy_set_digest: to_policy,
            },
            receipt_carryover: WorkflowReceiptCarryover::InvalidateAll,
            prior_ledger_head_digest: head.to_owned(),
        }
    }

    fn valid_wal_versions(root: &Path) -> (PathBuf, Vec<u8>, Vec<u8>) {
        let target = root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        fs::create_dir_all(target.parent().expect("WAL parent")).expect("create WAL parent");
        let (first, first_line) = build_record_line(
            &empty_projection(),
            &test_identity(),
            0,
            WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
                source_ref: "project/state.yaml".to_owned(),
                source_digest: "sha256:source".to_owned(),
                snapshot_digest: "sha256:snapshot-0".to_owned(),
                initial_phase: StableId("discover".to_owned()),
                readiness_profile: None,
            }),
        )
        .expect("build initial record");
        fs::write(&target, &first_line).expect("write old WAL");
        let projection = recover_under_lock(root).expect("recover old WAL");
        assert_eq!(projection.head_digest, Some(first.record_digest));
        let (_, second_line) = build_record_line(
            &projection,
            &test_identity(),
            1,
            WorkflowGovernanceEvent::PhaseAdvanced(PhaseAdvancedEvent {
                from_phase: Some(StableId("discover".to_owned())),
                to_phase: StableId("define".to_owned()),
                snapshot_digest: "sha256:snapshot-1".to_owned(),
            }),
        )
        .expect("build second record");
        let old = first_line;
        let mut new = old.clone();
        new.extend_from_slice(&second_line);
        (target, old, new)
    }

    fn set_crash_point(point: Option<ReplacementCrashPoint>) {
        REPLACEMENT_CRASH_POINT.with(|configured| configured.set(point));
    }

    fn initialize_legacy_profileless(
        root: &Path,
        readiness_profile: Option<WorkflowReadinessProfile>,
    ) -> (
        WorkflowGovernanceLedgerIdentity,
        WorkflowGovernanceLedgerRecord,
    ) {
        let identity = test_identity();
        let mut ledger = lock_workflow_governance_ledger_tcb(root).expect("lock legacy ledger");
        let record = ledger
            .initialize_unchecked_tcb(
                &identity,
                0,
                WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
                    source_ref: "/tmp/legacy project".to_owned(),
                    source_digest: sha256_digest(b"source"),
                    snapshot_digest: sha256_digest(b"snapshot"),
                    initial_phase: StableId("discover".to_owned()),
                    readiness_profile,
                }),
            )
            .expect("initialize legacy ledger");
        (identity, record)
    }

    fn legacy_adoption_event(
        genesis: &WorkflowGovernanceLedgerRecord,
        head: &str,
    ) -> LegacySoloProfileAdoptedEvent {
        LegacySoloProfileAdoptedEvent {
            legacy_project_import_record_digest: genesis.record_digest.clone(),
            prior_ledger_head_digest: head.to_owned(),
            snapshot_digest: sha256_digest(b"snapshot"),
            authority_basis: WorkflowCooperativeAuthorityBasis::CooperativeSameOwner,
        }
    }

    #[test]
    fn profileless_release_history_adopts_solo_append_only_and_one_way() {
        let root = test_root("legacy-profile-release-history");
        let (source, genesis) = initialize_legacy_profileless(&root, None);
        let before_release =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("genesis WAL");
        let target = WorkflowGovernanceLedgerIdentity {
            project_id: source.project_id.clone(),
            bundle_id: StableId("bundle-protocol-next".to_owned()),
            bundle_digest: sha256_digest(b"bundle-protocol-next"),
        };
        let release = test_release_event(&genesis.record_digest, &source, &target);
        let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock release");
        let released = ledger
            .transition_release_unchecked_tcb(&genesis.record_digest, &source, &target, 1, release)
            .expect("release upgrade");
        drop(ledger);
        let before_adoption =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("release WAL");
        assert!(before_adoption.starts_with(&before_release));
        let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock adoption");
        assert_eq!(
            ledger.recover().expect("pre-adoption").readiness_profile(),
            Some(WorkflowReadinessProfile::StrictExternal)
        );
        let adopted = ledger
            .adopt_legacy_solo_unchecked_tcb(
                &released.record_digest,
                &target,
                2,
                legacy_adoption_event(&genesis, &released.record_digest),
            )
            .expect("adopt Solo Cooperative");
        let projected = ledger.recover().expect("recover adopted");
        assert_eq!(
            projected.readiness_profile(),
            Some(WorkflowReadinessProfile::SoloCooperative)
        );
        assert_eq!(projected.records.len(), 3);
        let after =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("adopted WAL");
        assert!(after.starts_with(&before_adoption));
        assert_eq!(
            schema_from_line(
                after
                    .split(|byte| *byte == b'\n')
                    .nth(2)
                    .expect("adoption line")
            ),
            WORKFLOW_GOVERNANCE_LEGACY_SOLO_ADOPTION_LEDGER_SCHEMA_VERSION
        );
        assert!(matches!(
            ledger.adopt_legacy_solo_unchecked_tcb(
                &adopted.record_digest,
                &target,
                3,
                legacy_adoption_event(&genesis, &adopted.record_digest),
            ),
            Err(WorkflowGovernanceLedgerError::LegacySoloAdoptionInvalid { .. })
        ));
        assert_eq!(
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("unchanged WAL"),
            after
        );
        drop(ledger);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn legacy_adoption_rejects_explicit_and_authority_bearing_history_without_writes() {
        let explicit = test_root("legacy-explicit-strict");
        let (identity, genesis) = initialize_legacy_profileless(
            &explicit,
            Some(WorkflowReadinessProfile::StrictExternal),
        );
        let before =
            fs::read(explicit.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("explicit WAL");
        let mut ledger = lock_workflow_governance_ledger_tcb(&explicit).expect("lock explicit");
        assert!(matches!(
            ledger.adopt_legacy_solo_unchecked_tcb(
                &genesis.record_digest,
                &identity,
                1,
                legacy_adoption_event(&genesis, &genesis.record_digest),
            ),
            Err(WorkflowGovernanceLedgerError::LegacySoloAdoptionInvalid { .. })
        ));
        drop(ledger);
        assert_eq!(
            fs::read(explicit.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("explicit after"),
            before
        );
        fs::remove_dir_all(explicit).expect("cleanup explicit");

        let authority = test_root("legacy-authority-history");
        let (identity, genesis) = initialize_legacy_profileless(&authority, None);
        let mut ledger = lock_workflow_governance_ledger_tcb(&authority).expect("lock authority");
        let phase = ledger
            .append_unchecked_tcb_event(
                &genesis.record_digest,
                &identity,
                1,
                WorkflowGovernanceEvent::PhaseAdvanced(PhaseAdvancedEvent {
                    from_phase: Some(StableId("discover".to_owned())),
                    to_phase: StableId("define".to_owned()),
                    snapshot_digest: sha256_digest(b"snapshot"),
                }),
            )
            .expect("authority-bearing history");
        let before =
            fs::read(authority.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("authority WAL");
        assert!(matches!(
            ledger.adopt_legacy_solo_unchecked_tcb(
                &phase.record_digest,
                &identity,
                2,
                legacy_adoption_event(&genesis, &phase.record_digest),
            ),
            Err(WorkflowGovernanceLedgerError::LegacySoloAdoptionInvalid { .. })
        ));
        drop(ledger);
        assert_eq!(
            fs::read(authority.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH))
                .expect("authority after"),
            before
        );
        fs::remove_dir_all(authority).expect("cleanup authority");
    }

    fn rewrite_last_adoption_and_recompute(root: &Path, case: &str) -> Vec<u8> {
        let wal_path = root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let text = fs::read_to_string(&wal_path).expect("read adoption WAL");
        let mut documents = text
            .lines()
            .map(|line| {
                serde_json::from_str::<WorkflowGovernanceReceiptDocument>(line)
                    .expect("typed adoption document")
            })
            .collect::<Vec<_>>();
        let document = documents.last_mut().expect("adoption document");
        let WorkflowGovernanceEvent::LegacySoloProfileAdopted(event) =
            &mut document.workflow_governance_receipt.event
        else {
            panic!("last record must be legacy adoption")
        };
        match case {
            "genesis" => {
                event.legacy_project_import_record_digest = sha256_digest(b"other-genesis")
            }
            "head" => event.prior_ledger_head_digest = sha256_digest(b"other-head"),
            "snapshot" => event.snapshot_digest = "sha256:NOT-CANONICAL".to_owned(),
            _ => panic!("unknown tamper case"),
        }
        document.workflow_governance_receipt.record_digest =
            workflow_governance_record_digest(&document.workflow_governance_receipt)
                .expect("recomputed adversarial digest");
        let mut bytes = Vec::new();
        for document in documents {
            bytes.extend_from_slice(
                &serde_json::to_vec(&document).expect("serialize adversarial WAL"),
            );
            bytes.push(b'\n');
        }
        fs::write(&wal_path, &bytes).expect("install adversarial WAL");
        bytes
    }

    #[test]
    fn recomputed_legacy_adoption_binding_tampering_fails_closed_without_repair() {
        for case in ["genesis", "head", "snapshot"] {
            let root = test_root(&format!("legacy-adoption-tampered-{case}"));
            let (identity, genesis) = initialize_legacy_profileless(&root, None);
            let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock adoption");
            ledger
                .adopt_legacy_solo_unchecked_tcb(
                    &genesis.record_digest,
                    &identity,
                    1,
                    legacy_adoption_event(&genesis, &genesis.record_digest),
                )
                .expect("valid adoption before tampering");
            drop(ledger);

            let tampered = rewrite_last_adoption_and_recompute(&root, case);
            assert!(
                recover_under_lock(&root).is_err(),
                "recomputed {case} tampering must not recover",
            );
            assert_eq!(
                fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH))
                    .expect("tampered WAL after recovery rejection"),
                tampered,
                "recovery must not rewrite rejected {case} tampering",
            );
            fs::remove_dir_all(root).expect("cleanup tamper case");
        }
    }

    #[test]
    fn legacy_adoption_epoch_rejects_invalid_history_and_schema_rollback() {
        let invalid = test_root("legacy-adoption-after-authority");
        let (identity, genesis) = initialize_legacy_profileless(&invalid, None);
        let mut ledger = lock_workflow_governance_ledger_tcb(&invalid).expect("lock authority");
        let phase = ledger
            .append_unchecked_tcb_event(
                &genesis.record_digest,
                &identity,
                1,
                WorkflowGovernanceEvent::PhaseAdvanced(PhaseAdvancedEvent {
                    from_phase: Some(StableId("discover".to_owned())),
                    to_phase: StableId("define".to_owned()),
                    snapshot_digest: sha256_digest(b"snapshot"),
                }),
            )
            .expect("authority-bearing predecessor");
        let projection = ledger.recover().expect("recover authority history");
        drop(ledger);
        let (_, forged_adoption) = build_record_line_at(
            &projection,
            &identity,
            2,
            WorkflowGovernanceEvent::LegacySoloProfileAdopted(legacy_adoption_event(
                &genesis,
                &phase.record_digest,
            )),
            100,
        )
        .expect("forge schema 0.13 record below the guarded write API");
        assert_eq!(
            schema_from_line(&forged_adoption),
            WORKFLOW_GOVERNANCE_LEGACY_SOLO_ADOPTION_LEDGER_SCHEMA_VERSION,
        );
        let wal_path = invalid.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let mut forged = fs::read(&wal_path).expect("authority WAL");
        forged.extend_from_slice(&forged_adoption);
        fs::write(&wal_path, &forged).expect("install forged adoption");
        assert!(matches!(
            recover_under_lock(&invalid),
            Err(WorkflowGovernanceLedgerError::LegacySoloAdoptionInvalid { .. })
        ));
        assert_eq!(fs::read(&wal_path).expect("invalid history WAL"), forged);
        fs::remove_dir_all(invalid).expect("cleanup invalid history");

        let rollback = test_root("legacy-adoption-schema-rollback");
        let (identity, genesis) = initialize_legacy_profileless(&rollback, None);
        let mut ledger = lock_workflow_governance_ledger_tcb(&rollback).expect("lock adoption");
        ledger
            .adopt_legacy_solo_unchecked_tcb(
                &genesis.record_digest,
                &identity,
                1,
                legacy_adoption_event(&genesis, &genesis.record_digest),
            )
            .expect("adopt before rollback attempt");
        let projection = ledger.recover().expect("recover adopted epoch");
        let head = projection.head_digest.clone().expect("adoption head");
        let (_, later_line) = build_record_line_at(
            &projection,
            &identity,
            2,
            WorkflowGovernanceEvent::PhaseAdvanced(PhaseAdvancedEvent {
                from_phase: Some(StableId("discover".to_owned())),
                to_phase: StableId("define".to_owned()),
                snapshot_digest: sha256_digest(b"later-snapshot"),
            }),
            101,
        )
        .expect("valid later 0.13 record");
        drop(ledger);
        let mut later_document: WorkflowGovernanceReceiptDocument =
            serde_json::from_slice(later_line.strip_suffix(b"\n").expect("line terminator"))
                .expect("typed later record");
        assert_eq!(
            later_document
                .workflow_governance_receipt
                .previous_record_digest
                .as_deref(),
            Some(head.as_str()),
        );
        later_document.schema_version =
            WORKFLOW_GOVERNANCE_COOPERATIVE_EVIDENCE_LEDGER_SCHEMA_VERSION.to_owned();
        let wal_path = rollback.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
        let mut rollback_bytes = fs::read(&wal_path).expect("adopted WAL");
        rollback_bytes.extend_from_slice(
            &serde_json::to_vec(&later_document).expect("serialize rollback record"),
        );
        rollback_bytes.push(b'\n');
        fs::write(&wal_path, &rollback_bytes).expect("install schema rollback");
        assert!(matches!(
            recover_under_lock(&rollback),
            Err(WorkflowGovernanceLedgerError::UnsupportedSchema { .. })
        ));
        assert_eq!(
            fs::read(&wal_path).expect("rollback WAL after rejection"),
            rollback_bytes,
        );
        fs::remove_dir_all(rollback).expect("cleanup rollback");
    }

    #[test]
    fn interrupted_legacy_adoption_recovers_to_exactly_one_transition() {
        for (point, committed) in [
            (ReplacementCrashPoint::NextSynced, false),
            (ReplacementCrashPoint::TransactionSynced, false),
            (ReplacementCrashPoint::PreviousInstalled, false),
            (ReplacementCrashPoint::TargetInstalled, true),
        ] {
            let root = test_root(&format!("legacy-adoption-{point:?}"));
            let (identity, genesis) = initialize_legacy_profileless(&root, None);
            let event = legacy_adoption_event(&genesis, &genesis.record_digest);
            let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock adoption");
            let mut batch = ledger
                .begin_unchecked_tcb_batch(&genesis.record_digest, &identity)
                .expect("prepare adoption batch");
            batch
                .push_legacy_solo_adoption_unchecked_tcb(1, event.clone())
                .expect("prepare typed adoption");
            let adopted_wal = batch.prepared_wal.clone();
            drop(batch);
            drop(ledger);

            // Unix commits use the rename fast path, while these injected
            // phases belong to the portable replacement protocol used on
            // platforms without replace-by-rename semantics. Drive that real
            // protocol with the exact typed batch bytes prepared above.
            let target = root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH);
            set_crash_point(Some(point));
            let result = catch_unwind(AssertUnwindSafe(|| {
                replace_file_with_recovery_protocol(&target, &adopted_wal)
            }));
            set_crash_point(None);
            assert!(result.is_err(), "fault injection must interrupt {point:?}");

            let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("recover lock");
            let recovered = ledger.recover().expect("recover exact WAL");
            assert_eq!(
                recovered.contains_legacy_solo_adoption(),
                committed,
                "reconciliation must select the documented side at {point:?}"
            );
            if !recovered.contains_legacy_solo_adoption() {
                ledger
                    .adopt_legacy_solo_unchecked_tcb(&genesis.record_digest, &identity, 1, event)
                    .expect("complete old-side recovery");
            }
            let final_projection = ledger.recover().expect("final projection");
            assert_eq!(
                final_projection
                    .records
                    .iter()
                    .filter(|record| matches!(
                        record.event,
                        WorkflowGovernanceEvent::LegacySoloProfileAdopted(_)
                    ))
                    .count(),
                1
            );
            drop(ledger);
            fs::remove_dir_all(root).expect("cleanup");
        }
    }

    #[test]
    fn every_replacement_phase_recovers_old_or_committed_new_valid_wal() {
        for (point, committed) in [
            (ReplacementCrashPoint::NextSynced, false),
            (ReplacementCrashPoint::TransactionSynced, false),
            (ReplacementCrashPoint::PreviousInstalled, false),
            (ReplacementCrashPoint::TargetInstalled, true),
        ] {
            let root = test_root(&format!("phase-{point:?}"));
            let (target, old, new) = valid_wal_versions(&root);
            set_crash_point(Some(point));
            let result = catch_unwind(AssertUnwindSafe(|| {
                replace_file_with_recovery_protocol(&target, &new)
            }));
            set_crash_point(None);
            assert!(result.is_err(), "fault injection must interrupt {point:?}");

            reconcile_wal_replacement(&target).expect("deterministic reconciliation");
            assert_eq!(
                fs::read(&target).expect("recovered target"),
                if committed {
                    new.as_slice()
                } else {
                    old.as_slice()
                },
                "phase {point:?} must recover exactly old or committed new bytes"
            );
            let projection = recover_under_lock(&root).expect("recovered WAL remains valid");
            assert_eq!(projection.records.len(), if committed { 2 } else { 1 });
            let paths = replacement_paths(&target).expect("protocol paths");
            for artifact in [paths.next, paths.previous, paths.transaction] {
                assert!(
                    fs::symlink_metadata(artifact).is_err(),
                    "successful recovery must remove protocol artifacts"
                );
            }
            fs::remove_dir_all(root).expect("cleanup");
        }
    }

    #[test]
    fn interrupted_initial_write_cannot_be_recovered_as_silently_empty() {
        let root = test_root("initial-next-only");
        let (source_target, _, valid_content) = valid_wal_versions(&root);
        fs::remove_file(&source_target).expect("return fixture to empty state");
        set_crash_point(Some(ReplacementCrashPoint::NextSynced));
        let result = catch_unwind(AssertUnwindSafe(|| {
            replace_file_with_recovery_protocol(&source_target, &valid_content)
        }));
        set_crash_point(None);
        assert!(
            result.is_err(),
            "fault injection must interrupt initial write"
        );
        assert!(
            recover_under_lock(&root).is_err(),
            "ambiguous next-only initialization must fail closed, not return an empty ledger"
        );
        assert!(!source_target.exists());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn batch_cannot_prepare_a_second_release_transition() {
        let root = test_root("duplicate-release-transition");
        let (target_path, old, _) = valid_wal_versions(&root);
        let source = test_identity();
        let projection = recover_under_lock(&root).expect("recover source");
        let head = projection.head_digest.expect("source head");
        let target = WorkflowGovernanceLedgerIdentity {
            project_id: source.project_id.clone(),
            bundle_id: StableId("bundle-protocol-next".to_owned()),
            bundle_digest: sha256_digest(b"bundle-protocol-next"),
        };
        let event = test_release_event(&head, &source, &target);
        let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock ledger");
        let mut batch = ledger
            .begin_unchecked_tcb_batch(&head, &source)
            .expect("begin batch");
        batch
            .push_release_transition_tcb(&target, 1, event.clone())
            .expect("prepare first transition");
        assert!(matches!(
            batch.push_release_transition_tcb(&target, 1, event),
            Err(WorkflowGovernanceLedgerError::DuplicateReleaseTransition)
        ));
        drop(batch);
        drop(ledger);
        assert_eq!(fs::read(target_path).expect("source WAL"), old);
        fs::remove_dir_all(root).expect("cleanup");
    }

    fn effective_identity(
        core: WorkflowRuntimeBundleIdentity,
        generation: Option<u64>,
        label: &str,
    ) -> WorkflowEffectiveBundleIdentity {
        let effective = WorkflowRuntimeBundleIdentity {
            bundle_id: StableId(format!("bundle-effective-{label}")),
            bundle_digest: sha256_digest(format!("effective-{label}").as_bytes()),
            policy_set_digest: sha256_digest(format!("effective-policy-{label}").as_bytes()),
        };
        WorkflowEffectiveBundleIdentity {
            core_runtime_bundle: core.clone(),
            effective_runtime_bundle: generation.map_or_else(|| core, |_| effective),
            domain_pack_generation: generation.map(|generation| {
                forge_core_contracts::WorkflowDomainPackGenerationIdentity {
                    generation,
                    active_lock_digest: sha256_digest(format!("lock-{label}").as_bytes()),
                    composition_digest: sha256_digest(format!("composition-{label}").as_bytes()),
                    base_core_bundle_digest: sha256_digest(
                        format!("inner-core-{label}").as_bytes(),
                    ),
                    supply_chain_registry_digest: sha256_digest(
                        format!("supply-{label}").as_bytes(),
                    ),
                    reviewer_registry_digest: "a".repeat(64),
                    reviewed_registry_digest: "b".repeat(64),
                }
            }),
            receipt_context_digest: sha256_digest(format!("receipt-{label}").as_bytes()),
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn joined_rebase_advances_core_and_effective_epoch_in_one_record() {
        let root = test_root("joined-core-domain-pack-rebase");
        let source = test_identity();
        let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock ledger");
        let imported = ledger
            .initialize_unchecked_tcb(
                &source,
                0,
                WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
                    source_ref: "project/state.yaml".to_owned(),
                    source_digest: sha256_digest(b"source"),
                    snapshot_digest: sha256_digest(b"snapshot"),
                    initial_phase: StableId("discover".to_owned()),
                    readiness_profile: None,
                }),
            )
            .expect("initialize");
        let source_runtime = WorkflowRuntimeBundleIdentity {
            bundle_id: source.bundle_id.clone(),
            bundle_digest: source.bundle_digest.clone(),
            policy_set_digest: sha256_digest(b"policy-v1"),
        };
        let core_only = effective_identity(source_runtime.clone(), None, "core-only");
        let active_source = effective_identity(source_runtime, Some(1), "source");
        let domain_event = DomainPackGenerationTransitionedEvent {
            from_effective_bundle: core_only,
            to_effective_bundle: active_source.clone(),
            receipt_carryover: WorkflowReceiptCarryover::InvalidateAll,
            prior_ledger_head_digest: imported.record_digest.clone(),
        };
        let domain_record = ledger
            .transition_domain_pack_generation_unchecked_tcb(
                &imported.record_digest,
                &source,
                1,
                domain_event,
            )
            .expect("activate source generation");
        let effective_wal =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("0.2 WAL bytes");
        let effective_line = effective_wal
            .split(|byte| *byte == b'\n')
            .rfind(|line| !line.is_empty())
            .expect("0.2 WAL line");
        assert_eq!(
            schema_from_line(effective_line),
            WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION
        );
        ledger.recover().expect("recover exact 0.2 epoch");
        assert_eq!(
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH))
                .expect("0.2 WAL after recovery"),
            effective_wal,
            "0.2 recovery must preserve every historical byte"
        );
        let target = WorkflowGovernanceLedgerIdentity {
            project_id: source.project_id.clone(),
            bundle_id: StableId("bundle-protocol-next".to_owned()),
            bundle_digest: sha256_digest(b"bundle-protocol-next"),
        };
        let release = test_release_event(&domain_record.record_digest, &source, &target);
        let target_effective =
            effective_identity(release.to_runtime_bundle.clone(), Some(2), "target");
        let event = CoreDomainPackRebasedEvent {
            release_transition: release,
            from_effective_bundle: active_source,
            to_effective_bundle: target_effective.clone(),
            receipt_carryover: WorkflowReceiptCarryover::InvalidateAll,
            prior_ledger_head_digest: domain_record.record_digest.clone(),
        };
        ledger
            .transition_core_domain_pack_rebase_unchecked_tcb(
                &domain_record.record_digest,
                &source,
                &target,
                2,
                event,
            )
            .expect("joined rebase");
        let rebase_wal_before_recovery =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("0.4 WAL bytes");
        let projection = ledger.recover().expect("recover joined epoch");
        assert_eq!(
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH))
                .expect("0.4 WAL after recovery"),
            rebase_wal_before_recovery,
            "0.4 recovery must preserve the 0.1/0.2 prefix and 0.4 record bytes"
        );
        assert_eq!(projection.active_identity(), Some(target));
        assert_eq!(
            projection.active_effective_bundle_identity(),
            Some(target_effective)
        );
        let wal =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("joined WAL bytes");
        let last = wal
            .split(|byte| *byte == b'\n')
            .rfind(|line| !line.is_empty())
            .expect("joined WAL line");
        let document: WorkflowGovernanceReceiptDocument =
            serde_json::from_slice(last).expect("joined receipt wire");
        assert_eq!(
            document.schema_version,
            WORKFLOW_GOVERNANCE_REBASE_LEDGER_SCHEMA_VERSION
        );
        drop(ledger);
        fs::remove_dir_all(root).expect("cleanup");
    }

    fn broker_signal_event(head: &str) -> WorkflowGovernanceEvent {
        WorkflowGovernanceEvent::SignalChanged(SignalChangedEvent {
            signal: WorkflowGovernanceSignal::ReadinessRequested,
            active: true,
            episode_id: StableId("episode.test".to_owned()),
            generation: 1,
            changed_by: PrincipalId("origin.test".to_owned()),
            credential_id: StableId("issuer.test".to_owned()),
            public_key_fingerprint: sha256_digest(b"key"),
            authorization_registry_digest: sha256_digest(b"registry"),
            basis: Vec::new(),
            basis_digest: sha256_digest(b"basis"),
            snapshot_digest: sha256_digest(b"snapshot"),
            ledger_head_digest: head.to_owned(),
            observed_at_unix: 100,
            expires_at_unix: 200,
        })
    }

    fn broker_intent_event(head: &str, packet: &str) -> WorkflowGovernanceEvent {
        WorkflowGovernanceEvent::HumanIntentRevisionAccepted(HumanIntentRevisionAcceptedEvent {
            assurance_epoch: 1,
            intent: WorkflowHumanIntentRevision {
                intent_id: StableId("intent.workflow.project-protocol-test".to_owned()),
                revision: 1,
                desired_outcome: "Build a dependable governed product".to_owned(),
                constraints: Vec::new(),
                preferences: Vec::new(),
                unacceptable_outcomes: Vec::new(),
                uncertainties: Vec::new(),
                source_conversation_ref: "conversation://test/intent".to_owned(),
                source_conversation_digest: sha256_digest(b"conversation"),
            },
            intent_digest: sha256_digest(b"intent"),
            previous_intent_digest: None,
            snapshot_digest: sha256_digest(b"snapshot"),
            ledger_head_digest: head.to_owned(),
            acceptance_action_packet_digest: packet.to_owned(),
            accepted_by: PrincipalId("principal.human".to_owned()),
            accepted_at_unix: 100,
        })
    }

    fn native_host_origin_event(action_record_digest: &str) -> WorkflowGovernanceEvent {
        WorkflowGovernanceEvent::BrokerOriginApplied(
            forge_core_contracts::BrokerOriginAppliedEvent {
                action_packet_digest: sha256_digest(b"native-host-packet"),
                broker_event_digest: sha256_digest(b"native-host-event"),
                action_record_digest: action_record_digest.to_owned(),
                origin_principal_id: forge_core_contracts::PrincipalId(
                    "principal.human.native-host".to_owned(),
                ),
                separation_domain: StableId("native-host.session".to_owned()),
                nonce_fingerprint: sha256_digest(b"native-host-nonce"),
                issuer_id: StableId("broker.native-host".to_owned()),
                issuer_profile: forge_core_contracts::WorkflowBrokerOriginProfile::Human,
                public_key_fingerprint: sha256_digest(b"native-host-key"),
                signature_fingerprint: sha256_digest(b"native-host-signature"),
                enrollment_ceremony_digest: sha256_digest(b"native-host-enrollment"),
                broker_registry_digest: sha256_digest(b"native-host-registry"),
                native_interaction_replay_digest: None,
                issued_at_unix: 100,
                expires_at_unix: 200,
                native_host_provenance: Some(
                    forge_core_contracts::WorkflowBrokerNativeHostProvenance {
                        host_kind: forge_core_contracts::RuntimeKind::ForgeStandalone,
                        host_version: "0.12.0".to_owned(),
                        adapter_id: StableId("adapter.forge-standalone.tcb-test".to_owned()),
                        adapter_version: "0.1.0".to_owned(),
                        interaction_kind:
                            forge_core_contracts::WorkflowBrokerHostInteractionKind::NativeHumanConfirmation,
                        host_event_ref: "host-event-tcb-test-0001".to_owned(),
                        host_session_ref: "host-session-tcb-test-0001".to_owned(),
                        host_interaction_ref: "host-interaction-tcb-test-0001".to_owned(),
                        host_event_descriptor_digest: sha256_digest(b"native-host-descriptor"),
                        host_observed_at_unix: 100,
                    },
                ),
            },
        )
    }
    fn strict_native_host_origin_event(action_record_digest: &str) -> WorkflowGovernanceEvent {
        let mut event = native_host_origin_event(action_record_digest);
        let WorkflowGovernanceEvent::BrokerOriginApplied(origin) = &mut event else {
            unreachable!("native host helper always returns broker provenance");
        };
        origin.native_interaction_replay_digest =
            Some(sha256_digest(b"strict-native-interaction-replay"));
        event
    }

    fn schema_from_line(line: &[u8]) -> String {
        let document: WorkflowGovernanceReceiptDocument =
            serde_json::from_slice(line.strip_suffix(b"\n").unwrap_or(line))
                .expect("typed receipt line");
        document.schema_version
    }

    fn initialize_solo_profile(
        root: &Path,
    ) -> (
        WorkflowGovernanceLedgerProjection,
        WorkflowGovernanceLedgerRecord,
        Vec<u8>,
    ) {
        initialize_solo_profile_at(root, StableId("discover".to_owned()))
    }

    fn initialize_solo_profile_at(
        root: &Path,
        initial_phase: StableId,
    ) -> (
        WorkflowGovernanceLedgerProjection,
        WorkflowGovernanceLedgerRecord,
        Vec<u8>,
    ) {
        let mut ledger = lock_workflow_governance_ledger_tcb(root).expect("solo ledger");
        let genesis = ledger
            .initialize_unchecked_tcb(
                &test_identity(),
                0,
                WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
                    source_ref: "project/state.yaml".to_owned(),
                    source_digest: sha256_digest(b"solo-source"),
                    snapshot_digest: sha256_digest(b"solo-snapshot"),
                    initial_phase,
                    readiness_profile: Some(WorkflowReadinessProfile::SoloCooperative),
                }),
            )
            .expect("initialize solo profile");
        let projection = ledger.recover().expect("recover solo profile");
        let bytes =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("solo WAL bytes");
        assert_eq!(
            schema_from_line(&bytes),
            WORKFLOW_GOVERNANCE_READINESS_PROFILE_LEDGER_SCHEMA_VERSION
        );
        (projection, genesis, bytes)
    }

    fn cooperative_objective_event(
        head: &str,
        objective_id: String,
        carrying_principal: String,
        observed_at_unix: u64,
        accepted_at_unix: u64,
    ) -> CooperativeObjectiveAcceptedEvent {
        let mut event = CooperativeObjectiveAcceptedEvent {
            objective_id: StableId(objective_id),
            revision: 1,
            assurance_epoch: 1,
            proposal: WorkflowCooperativeObjectiveProposal {
                outcome: "Dogfood Forge with a solo developer and agents".to_owned(),
                constraints: vec!["Remain host neutral".to_owned()],
                unacceptable_outcomes: vec!["Claim verified human origin".to_owned()],
                open_uncertainties: vec!["Future team authority".to_owned()],
            },
            objective_digest: String::new(),
            previous_objective_digest: None,
            revision_kind: WorkflowCooperativeObjectiveRevisionKind::Initial,
            revision_reason: None,
            revision_input_digest: None,
            snapshot_digest: sha256_digest(b"cooperative-snapshot"),
            ledger_head_digest: head.to_owned(),
            acceptance_action_packet_digest: sha256_digest(b"cooperative-packet"),
            carrying_principal: PrincipalId(carrying_principal),
            host_provenance: WorkflowCooperativeHostProvenance {
                host_id: StableId("host.tcb-test".to_owned()),
                host_version: "0.12.0-test".to_owned(),
                session_ref: "session.tcb-test".to_owned(),
                interaction_ref: "interaction.tcb-test".to_owned(),
                conversation_digest: sha256_digest(b"conversation"),
                observed_at_unix,
            },
            authority_basis: WorkflowCooperativeAuthorityBasis::CooperativeSameOwner,
            accepted_at_unix,
        };
        event.objective_digest =
            cooperative_objective_digest(&event).expect("canonical cooperative objective");
        event
    }

    fn initialize_quick_cycle_focus(
        root: &Path,
        objective_id: &str,
    ) -> (
        WorkflowGovernanceLedgerProjection,
        WorkflowGovernanceLedgerRecord,
    ) {
        let (projection, genesis, _) =
            initialize_solo_profile_at(root, StableId("1-discovery".to_owned()));
        let objective = cooperative_objective_event(
            &genesis.record_digest,
            objective_id.to_owned(),
            "principal.same-owner".to_owned(),
            90,
            100,
        );
        let objective_record = lock_workflow_governance_ledger_tcb(root)
            .expect("objective ledger")
            .accept_cooperative_objective_unchecked_tcb(
                &genesis.record_digest,
                &test_identity(),
                projection.current_state_version().unwrap_or_default(),
                objective,
            )
            .expect("accept objective");
        (
            recover_under_lock(root).expect("recover objective"),
            objective_record,
        )
    }

    fn quick_cycle_snapshot() -> forge_core_contracts::WorkflowQuickCycleSnapshot {
        forge_core_contracts::WorkflowQuickCycleSnapshot {
            compactness_reason: "The change is bounded and reversible".to_owned(),
            stage_closeouts: forge_core_contracts::WorkflowQuickCycleStageCloseouts {
                analysis_discovery: None,
                product_planning: None,
                solution_definition: None,
                implementation: None,
                validation_delivery: None,
            },
            expansion_history: Vec::new(),
        }
    }

    fn work_focus_event(
        projection: &WorkflowGovernanceLedgerProjection,
        objective_record: &WorkflowGovernanceLedgerRecord,
    ) -> WorkflowWorkFocusRecordedEvent {
        let WorkflowGovernanceEvent::CooperativeObjectiveAccepted(objective) =
            &objective_record.event
        else {
            panic!("work focus fixture requires an objective record");
        };
        WorkflowWorkFocusRecordedEvent {
            focus_id: StableId("focus.current-work-continuity".to_owned()),
            objective: WorkflowWorkFocusObjectiveBinding {
                objective_id: objective.objective_id.clone(),
                objective_revision: objective.revision,
                objective_digest: objective.objective_digest.clone(),
                accepted_objective_record_digest: objective_record.record_digest.clone(),
                accepted_objective_record_sequence: objective_record.sequence,
                assurance_epoch: objective.assurance_epoch,
            },
            phase: Phase::Discovery,
            state: WorkflowWorkFocusState::Active,
            title: "Restore current work continuity".to_owned(),
            intended_outcome: "A replacement agent can continue without chat history".to_owned(),
            acceptance_summary: "The bounded focus is recoverable from the ledger".to_owned(),
            non_goals: vec!["Do not classify every message".to_owned()],
            canonical_refs: vec![RepoPath(
                "contracts/spec/product-journey-guidance-v0.yaml".to_owned(),
            )],
            affected_area_refs: vec![RepoPath("crates/forge-core-contracts".to_owned())],
            external_work_item_ref: Some("github:DanielCarva1/forge-method-core#32".to_owned()),
            selected_practice_ref: Some(StableId("investigation".to_owned())),
            selected_practice_reason: Some(
                "Map the current state before changing the contract".to_owned(),
            ),
            current_activity: "Define the ledger contract".to_owned(),
            next_step: "Project the focus through resume".to_owned(),
            blocker_record_digests: Vec::new(),
            evidence_record_digests: Vec::new(),
            quick_cycle: None,
            previous_work_focus_record_digest: None,
            admission_ledger_head_digest: projection.head_digest.clone().expect("objective head"),
            admission_state_version: projection.current_state_version().expect("objective state"),
            recorded_by: PrincipalId("principal.same-owner".to_owned()),
            host_provenance: WorkflowCooperativeHostProvenance {
                host_id: StableId("host.tcb-test".to_owned()),
                host_version: "0.12.0-test".to_owned(),
                session_ref: "session.work-focus".to_owned(),
                interaction_ref: "interaction.work-focus".to_owned(),
                conversation_digest: sha256_digest(b"work-focus-conversation"),
                observed_at_unix: 110,
            },
            authority: WorkflowCurrentWorkAuthority::AdvisoryReadOnly,
            recorded_at_unix: 110,
        }
    }

    #[test]
    fn work_focus_advances_to_0_15_and_recovers_without_rewriting_history() {
        let root = test_root("work-focus-wire");
        let (projection, genesis, _) =
            initialize_solo_profile_at(&root, StableId("1-discovery".to_owned()));
        let objective = cooperative_objective_event(
            &genesis.record_digest,
            "objective.workflow.project-protocol-test".to_owned(),
            "principal.same-owner".to_owned(),
            90,
            100,
        );
        let objective_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("objective ledger")
            .accept_cooperative_objective_unchecked_tcb(
                &genesis.record_digest,
                &test_identity(),
                projection.current_state_version().unwrap_or_default(),
                objective,
            )
            .expect("accept objective");
        let historical =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("historical WAL");
        let projection = recover_under_lock(&root).expect("recover objective");
        let event = work_focus_event(&projection, &objective_record);
        assert!(matches!(
            lock_workflow_governance_ledger_tcb(&root)
                .expect("generic event ledger")
                .append_unchecked_tcb_event(
                    projection.head_digest.as_deref().expect("objective head"),
                    &test_identity(),
                    projection.current_state_version().expect("objective state"),
                    WorkflowGovernanceEvent::WorkFocusRecorded(event.clone()),
                ),
            Err(WorkflowGovernanceLedgerError::WorkFocusRequiresDedicatedAuthority)
        ));
        let focus_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("focus ledger")
            .record_work_focus_unchecked_tcb(
                projection.head_digest.as_deref().expect("objective head"),
                &test_identity(),
                projection.current_state_version().expect("objective state"),
                event,
            )
            .expect("record focus");

        let complete =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("0.15 WAL");
        assert_eq!(&complete[..historical.len()], historical.as_slice());
        let last = complete
            .split_inclusive(|byte| *byte == b'\n')
            .last()
            .expect("0.15 record");
        assert_eq!(
            schema_from_line(last),
            WORKFLOW_GOVERNANCE_CURRENT_WORK_LEDGER_SCHEMA_VERSION
        );
        let recovered = recover_under_lock(&root).expect("recover 0.15");
        assert_eq!(recovered.head_digest, Some(focus_record.record_digest));

        let downgraded = String::from_utf8(last.to_vec())
            .expect("0.15 UTF-8")
            .replacen(
                &format!(
                "\"schema_version\":\"{WORKFLOW_GOVERNANCE_CURRENT_WORK_LEDGER_SCHEMA_VERSION}\""
            ),
                &format!(
                "\"schema_version\":\"{WORKFLOW_GOVERNANCE_PRIOR_EVIDENCE_LEDGER_SCHEMA_VERSION}\""
            ),
                1,
            );
        let mut downgraded_wal = historical;
        downgraded_wal.extend_from_slice(downgraded.as_bytes());
        fs::write(
            root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH),
            downgraded_wal,
        )
        .expect("install downgraded focus");
        assert!(matches!(
            recover_under_lock(&root),
            Err(WorkflowGovernanceLedgerError::UnsupportedSchema { line: 3, .. })
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn work_focus_bindings_advance_to_0_16_and_reject_wrong_record_kinds() {
        let root = test_root("work-focus-bindings-wire");
        let (projection, genesis, _) =
            initialize_solo_profile_at(&root, StableId("1-discovery".to_owned()));
        let objective = cooperative_objective_event(
            &genesis.record_digest,
            "objective.workflow.bindings-test".to_owned(),
            "principal.same-owner".to_owned(),
            90,
            100,
        );
        let objective_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("objective ledger")
            .accept_cooperative_objective_unchecked_tcb(
                &genesis.record_digest,
                &test_identity(),
                projection.current_state_version().unwrap_or_default(),
                objective,
            )
            .expect("accept objective");
        let projection = recover_under_lock(&root).expect("recover objective");
        let focus_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("focus ledger")
            .record_work_focus_unchecked_tcb(
                projection.head_digest.as_deref().expect("objective head"),
                &test_identity(),
                projection.current_state_version().expect("objective state"),
                work_focus_event(&projection, &objective_record),
            )
            .expect("record focus");
        let projection = recover_under_lock(&root).expect("recover focus");
        let blocker = lock_workflow_governance_ledger_tcb(&root)
            .expect("blocker ledger")
            .append_unchecked_tcb_event(
                projection.head_digest.as_deref().expect("focus head"),
                &test_identity(),
                projection.current_state_version().expect("focus state"),
                WorkflowGovernanceEvent::DecisionNeedRaised(
                    forge_core_contracts::DecisionNeedRaisedEvent {
                        policy_ref: StableId("policy.workflow.bindings-test".to_owned()),
                        decision_ref: StableId("decision.workflow.bindings-test".to_owned()),
                        authority_scope: StableId("workflow.decision.resolve".to_owned()),
                        question_digest: sha256_digest(b"bindings question"),
                    },
                ),
            )
            .expect("record blocker");
        let projection = recover_under_lock(&root).expect("recover blocker");
        let mut bound = work_focus_event(&projection, &objective_record);
        bound.previous_work_focus_record_digest = Some(focus_record.record_digest);
        bound.blocker_record_digests = vec![blocker.record_digest.clone()];
        let bound_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("binding ledger")
            .record_work_focus_unchecked_tcb(
                projection.head_digest.as_deref().expect("blocker head"),
                &test_identity(),
                projection.current_state_version().expect("blocker state"),
                bound,
            )
            .expect("record binding");
        let wal = fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("binding WAL");
        let last = wal
            .split_inclusive(|byte| *byte == b'\n')
            .last()
            .expect("binding record");
        assert_eq!(
            schema_from_line(last),
            WORKFLOW_GOVERNANCE_WORK_FOCUS_BINDINGS_LEDGER_SCHEMA_VERSION
        );
        let projection = recover_under_lock(&root).expect("recover binding");
        assert_eq!(
            projection.head_digest,
            Some(bound_record.record_digest.clone())
        );

        let before_wrong_kind = fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH))
            .expect("WAL before wrong kind");
        let mut wrong_kind = work_focus_event(&projection, &objective_record);
        wrong_kind.previous_work_focus_record_digest = Some(bound_record.record_digest);
        wrong_kind.evidence_record_digests = vec![blocker.record_digest];
        assert!(matches!(
            lock_workflow_governance_ledger_tcb(&root)
                .expect("wrong-kind ledger")
                .record_work_focus_unchecked_tcb(
                    projection.head_digest.as_deref().expect("binding head"),
                    &test_identity(),
                    projection.current_state_version().expect("binding state"),
                    wrong_kind,
                ),
            Err(WorkflowGovernanceLedgerError::WorkFocusInvalid { .. })
        ));
        assert_eq!(
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH))
                .expect("WAL after wrong kind"),
            before_wrong_kind
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn quick_cycle_advances_to_0_17_without_rewriting_history() {
        let root = test_root("quick-cycle-wire");
        let (projection, objective_record) =
            initialize_quick_cycle_focus(&root, "objective.workflow.quick-cycle-wire-test");
        let historical =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("historical WAL");
        let mut event = work_focus_event(&projection, &objective_record);
        event.quick_cycle = Some(quick_cycle_snapshot());
        let quick_cycle_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("Quick Cycle ledger")
            .record_work_focus_unchecked_tcb(
                projection.head_digest.as_deref().expect("objective head"),
                &test_identity(),
                projection.current_state_version().expect("objective state"),
                event,
            )
            .expect("record Quick Cycle");

        let quick_cycle_wal =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("0.17 WAL");
        assert_eq!(&quick_cycle_wal[..historical.len()], historical.as_slice());
        let quick_cycle_line = quick_cycle_wal
            .split_inclusive(|byte| *byte == b'\n')
            .last()
            .expect("0.17 record");
        assert_eq!(
            schema_from_line(quick_cycle_line),
            WORKFLOW_GOVERNANCE_QUICK_CYCLE_LEDGER_SCHEMA_VERSION
        );
        let projection = recover_under_lock(&root).expect("recover 0.17");
        assert_eq!(
            projection.head_digest,
            Some(quick_cycle_record.record_digest.clone())
        );

        lock_workflow_governance_ledger_tcb(&root)
            .expect("successor ledger")
            .append_unchecked_tcb_event(
                &quick_cycle_record.record_digest,
                &test_identity(),
                projection
                    .current_state_version()
                    .expect("Quick Cycle state"),
                WorkflowGovernanceEvent::DecisionNeedRaised(
                    forge_core_contracts::DecisionNeedRaisedEvent {
                        policy_ref: StableId("policy.workflow.quick-cycle-wire-test".to_owned()),
                        decision_ref: StableId(
                            "decision.workflow.quick-cycle-wire-test".to_owned(),
                        ),
                        authority_scope: StableId("workflow.decision.resolve".to_owned()),
                        question_digest: sha256_digest(b"Quick Cycle wire question"),
                    },
                ),
            )
            .expect("record successor");
        let successor_wal =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("successor WAL");
        let successor_line = successor_wal
            .split_inclusive(|byte| *byte == b'\n')
            .last()
            .expect("successor record");
        assert_eq!(
            schema_from_line(successor_line),
            WORKFLOW_GOVERNANCE_QUICK_CYCLE_LEDGER_SCHEMA_VERSION
        );

        let downgraded = String::from_utf8(quick_cycle_line.to_vec())
            .expect("0.17 UTF-8")
            .replacen(
                &format!(
                    "\"schema_version\":\"{WORKFLOW_GOVERNANCE_QUICK_CYCLE_LEDGER_SCHEMA_VERSION}\""
                ),
                &format!(
                    "\"schema_version\":\"{WORKFLOW_GOVERNANCE_WORK_FOCUS_BINDINGS_LEDGER_SCHEMA_VERSION}\""
                ),
                1,
            );
        let mut downgraded_wal = historical;
        downgraded_wal.extend_from_slice(downgraded.as_bytes());
        fs::write(
            root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH),
            downgraded_wal,
        )
        .expect("install downgraded Quick Cycle");
        assert!(matches!(
            recover_under_lock(&root),
            Err(WorkflowGovernanceLedgerError::UnsupportedSchema { line: 3, .. })
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn quick_cycle_rejects_unbounded_or_unowned_data_before_append() {
        let root = test_root("quick-cycle-bounds");
        let (projection, objective_record) =
            initialize_quick_cycle_focus(&root, "objective.workflow.quick-cycle-bounds-test");
        let mut base = work_focus_event(&projection, &objective_record);
        base.quick_cycle = Some(quick_cycle_snapshot());

        let mut oversized_compactness = base.clone();
        oversized_compactness
            .quick_cycle
            .as_mut()
            .expect("Quick Cycle")
            .compactness_reason =
            "x".repeat(forge_core_contracts::MAX_QUICK_CYCLE_COMPACTNESS_REASON_BYTES + 1);

        let mut oversized_closeout = base.clone();
        oversized_closeout
            .quick_cycle
            .as_mut()
            .expect("Quick Cycle")
            .stage_closeouts
            .analysis_discovery = Some(forge_core_contracts::WorkflowQuickCycleCloseout {
            summary: "x".repeat(forge_core_contracts::MAX_QUICK_CYCLE_CLOSEOUT_SUMMARY_BYTES + 1),
            evidence_record_digests: Vec::new(),
        });

        let mut too_many_expansions = base.clone();
        too_many_expansions
            .quick_cycle
            .as_mut()
            .expect("Quick Cycle")
            .expansion_history = (0..=forge_core_contracts::MAX_QUICK_CYCLE_EXPANSION_ITEMS)
            .map(|index| forge_core_contracts::WorkflowQuickCycleExpansion {
                phase: Phase::Discovery,
                reason: format!("accepted expansion {index}"),
                evidence_record_digests: Vec::new(),
            })
            .collect();

        let invalid_expansion = |phase| {
            let mut event = base.clone();
            event
                .quick_cycle
                .as_mut()
                .expect("Quick Cycle")
                .expansion_history = vec![forge_core_contracts::WorkflowQuickCycleExpansion {
                phase,
                reason: "This is not a lifecycle stage".to_owned(),
                evidence_record_digests: Vec::new(),
            }];
            event
        };
        let route_expansion = invalid_expansion(Phase::Route);
        let evolve_expansion = invalid_expansion(Phase::Evolve);

        let mut oversized_expansion_reason = base.clone();
        oversized_expansion_reason
            .quick_cycle
            .as_mut()
            .expect("Quick Cycle")
            .expansion_history = vec![forge_core_contracts::WorkflowQuickCycleExpansion {
            phase: Phase::Discovery,
            reason: "x".repeat(forge_core_contracts::MAX_QUICK_CYCLE_EXPANSION_REASON_BYTES + 1),
            evidence_record_digests: Vec::new(),
        }];

        let mut unowned_evidence = base;
        unowned_evidence
            .quick_cycle
            .as_mut()
            .expect("Quick Cycle")
            .stage_closeouts
            .analysis_discovery = Some(forge_core_contracts::WorkflowQuickCycleCloseout {
            summary: "The investigation is closed".to_owned(),
            evidence_record_digests: vec![sha256_digest(b"not in the Work Focus evidence set")],
        });

        let before =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("WAL before rejects");
        for invalid in [
            oversized_compactness,
            oversized_closeout,
            too_many_expansions,
            route_expansion,
            evolve_expansion,
            oversized_expansion_reason,
            unowned_evidence,
        ] {
            assert!(matches!(
                lock_workflow_governance_ledger_tcb(&root)
                    .expect("invalid Quick Cycle ledger")
                    .record_work_focus_unchecked_tcb(
                        projection.head_digest.as_deref().expect("objective head"),
                        &test_identity(),
                        projection.current_state_version().expect("objective state"),
                        invalid,
                    ),
                Err(WorkflowGovernanceLedgerError::WorkFocusInvalid { .. })
            ));
            assert_eq!(
                fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH))
                    .expect("WAL after reject"),
                before
            );
        }
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn work_focus_rejects_stale_conflicting_and_oversized_updates() {
        let root = test_root("work-focus-conflicts");
        let (projection, genesis, _) =
            initialize_solo_profile_at(&root, StableId("1-discovery".to_owned()));
        let objective = cooperative_objective_event(
            &genesis.record_digest,
            "objective.workflow.project-protocol-test".to_owned(),
            "principal.same-owner".to_owned(),
            90,
            100,
        );
        let initial_objective_digest = objective.objective_digest.clone();
        let objective_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("objective ledger")
            .accept_cooperative_objective_unchecked_tcb(
                &genesis.record_digest,
                &test_identity(),
                projection.current_state_version().unwrap_or_default(),
                objective,
            )
            .expect("accept objective");
        let projection = recover_under_lock(&root).expect("recover objective");
        let mut wrong_phase = work_focus_event(&projection, &objective_record);
        wrong_phase.phase = Phase::Specification;
        assert!(matches!(
            lock_workflow_governance_ledger_tcb(&root)
                .expect("wrong phase ledger")
                .record_work_focus_unchecked_tcb(
                    projection.head_digest.as_deref().expect("objective head"),
                    &test_identity(),
                    projection.current_state_version().expect("objective state"),
                    wrong_phase,
                ),
            Err(WorkflowGovernanceLedgerError::WorkFocusInvalid { .. })
        ));
        let mut oversized = work_focus_event(&projection, &objective_record);
        oversized.title = "x".repeat(MAX_WORK_FOCUS_TEXT_BYTES + 1);
        assert!(matches!(
            lock_workflow_governance_ledger_tcb(&root)
                .expect("oversized ledger")
                .record_work_focus_unchecked_tcb(
                    projection.head_digest.as_deref().expect("objective head"),
                    &test_identity(),
                    projection.current_state_version().expect("objective state"),
                    oversized,
                ),
            Err(WorkflowGovernanceLedgerError::WorkFocusInvalid { .. })
        ));

        let mut oversized_total = work_focus_event(&projection, &objective_record);
        oversized_total.non_goals = (0..MAX_WORK_FOCUS_LIST_ITEMS)
            .map(|index| format!("{index:02}{}", "x".repeat(MAX_WORK_FOCUS_TEXT_BYTES - 2)))
            .collect();
        assert!(matches!(
            lock_workflow_governance_ledger_tcb(&root)
                .expect("total-bound ledger")
                .record_work_focus_unchecked_tcb(
                    projection.head_digest.as_deref().expect("objective head"),
                    &test_identity(),
                    projection.current_state_version().expect("objective state"),
                    oversized_total,
                ),
            Err(WorkflowGovernanceLedgerError::WorkFocusInvalid { .. })
        ));

        let first = work_focus_event(&projection, &objective_record);
        let first_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("first focus ledger")
            .record_work_focus_unchecked_tcb(
                projection.head_digest.as_deref().expect("objective head"),
                &test_identity(),
                projection.current_state_version().expect("objective state"),
                first,
            )
            .expect("first focus");
        let projection = recover_under_lock(&root).expect("recover first focus");
        let mut update = work_focus_event(&projection, &objective_record);
        update.previous_work_focus_record_digest = Some(sha256_digest(b"wrong predecessor"));
        assert!(matches!(
            lock_workflow_governance_ledger_tcb(&root)
                .expect("conflict ledger")
                .record_work_focus_unchecked_tcb(
                    projection.head_digest.as_deref().expect("focus head"),
                    &test_identity(),
                    projection.current_state_version().expect("focus state"),
                    update,
                ),
            Err(WorkflowGovernanceLedgerError::WorkFocusInvalid { .. })
        ));

        let mut stale_cas = work_focus_event(&projection, &objective_record);
        stale_cas.previous_work_focus_record_digest = Some(first_record.record_digest.clone());
        assert!(matches!(
            lock_workflow_governance_ledger_tcb(&root)
                .expect("stale CAS ledger")
                .record_work_focus_unchecked_tcb(
                    &genesis.record_digest,
                    &test_identity(),
                    projection.current_state_version().expect("focus state"),
                    stale_cas,
                ),
            Err(WorkflowGovernanceLedgerError::HeadMismatch { .. })
        ));

        let mut revision = cooperative_objective_event(
            projection.head_digest.as_deref().expect("focus head"),
            "objective.workflow.project-protocol-test".to_owned(),
            "principal.same-owner".to_owned(),
            111,
            112,
        );
        revision.revision = 2;
        revision.assurance_epoch = 2;
        revision.proposal.outcome = "Dogfood Forge with durable current work continuity".to_owned();
        revision.previous_objective_digest = Some(initial_objective_digest);
        revision.revision_kind = WorkflowCooperativeObjectiveRevisionKind::MaterialSupersession;
        revision.revision_reason = Some("The accepted product objective changed".to_owned());
        revision.objective_digest =
            cooperative_objective_digest(&revision).expect("revised objective digest");
        revision.revision_input_digest = Some(
            cooperative_revision_input_digest(
                &WorkflowCooperativeObjectiveInput::MaterialSupersession {
                    proposal: revision.proposal.clone(),
                    supersession_reason: revision.revision_reason.clone().expect("revision reason"),
                    carrying_principal: revision.carrying_principal.clone(),
                    host_provenance: revision.host_provenance.clone(),
                },
            )
            .expect("revision input digest"),
        );
        let revision_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("revision ledger")
            .accept_cooperative_objective_unchecked_tcb(
                projection.head_digest.as_deref().expect("focus head"),
                &test_identity(),
                projection.current_state_version().expect("focus state"),
                revision,
            )
            .expect("accept objective revision");
        let revised = recover_under_lock(&root).expect("recover revised objective");
        let stale_objective_focus = work_focus_event(&revised, &objective_record);
        assert!(matches!(
            lock_workflow_governance_ledger_tcb(&root)
                .expect("stale objective ledger")
                .record_work_focus_unchecked_tcb(
                    revised.head_digest.as_deref().expect("revised head"),
                    &test_identity(),
                    revised.current_state_version().expect("revised state"),
                    stale_objective_focus,
                ),
            Err(WorkflowGovernanceLedgerError::WorkFocusInvalid { .. })
        ));

        let mut superseding = work_focus_event(&revised, &revision_record);
        superseding.focus_id = StableId("focus.current-work-projection".to_owned());
        superseding.previous_work_focus_record_digest = Some(first_record.record_digest);
        let superseding_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("superseding focus ledger")
            .record_work_focus_unchecked_tcb(
                revised.head_digest.as_deref().expect("revised head"),
                &test_identity(),
                revised.current_state_version().expect("revised state"),
                superseding,
            )
            .expect("explicitly supersede stale focus");

        let projection = recover_under_lock(&root).expect("recover superseding focus");
        let mut completed = work_focus_event(&projection, &revision_record);
        completed.focus_id = StableId("focus.current-work-projection".to_owned());
        completed.state = WorkflowWorkFocusState::Completed;
        completed.previous_work_focus_record_digest = Some(superseding_record.record_digest);
        let completed_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("completed focus ledger")
            .record_work_focus_unchecked_tcb(
                projection.head_digest.as_deref().expect("superseding head"),
                &test_identity(),
                projection
                    .current_state_version()
                    .expect("superseding state"),
                completed,
            )
            .expect("complete focus");

        let projection = recover_under_lock(&root).expect("recover completed focus");
        let mut restarted = work_focus_event(&projection, &revision_record);
        restarted.focus_id = StableId("focus.next-deliverable".to_owned());
        restarted.previous_work_focus_record_digest = Some(completed_record.record_digest);
        lock_workflow_governance_ledger_tcb(&root)
            .expect("restart focus ledger")
            .record_work_focus_unchecked_tcb(
                projection.head_digest.as_deref().expect("completed head"),
                &test_identity(),
                projection.current_state_version().expect("completed state"),
                restarted,
            )
            .expect("start a new focus after terminal state");
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn native_host_provenance_advances_wire_to_0_5_without_rewriting_history() {
        let root = test_root("native-host-origin-wire-successor");
        let (target, _, historical_wal) = valid_wal_versions(&root);
        fs::write(&target, &historical_wal).expect("install historical WAL");
        let historical_before = fs::read(&target).expect("historical bytes");
        let projection = recover_under_lock(&root).expect("historical projection");
        let action_record_digest = projection.head_digest.clone().expect("historical head");
        let (_, origin_line) = build_record_line(
            &projection,
            &test_identity(),
            1,
            native_host_origin_event(&action_record_digest),
        )
        .expect("native host provenance record");
        assert_eq!(
            schema_from_line(&origin_line),
            WORKFLOW_GOVERNANCE_HOST_ORIGIN_LEDGER_SCHEMA_VERSION
        );

        let mut with_origin = historical_wal.clone();
        with_origin.extend_from_slice(&origin_line);
        assert_eq!(
            &with_origin[..historical_before.len()],
            historical_before.as_slice(),
            "the 0.5 successor must not rewrite frozen historical bytes"
        );
        fs::write(&target, &with_origin).expect("install 0.5 WAL");
        let projection = recover_under_lock(&root).expect("recover 0.5 wire");
        assert_eq!(
            fs::read(&target).expect("0.5 WAL after recovery"),
            with_origin,
            "0.5 recovery must preserve the complete historical prefix and provenance bytes"
        );
        let head = projection.head_digest.clone().expect("0.5 head");
        let (_, later_line) =
            build_record_line(&projection, &test_identity(), 1, broker_signal_event(&head))
                .expect("post-provenance record");
        assert_eq!(
            schema_from_line(&later_line),
            WORKFLOW_GOVERNANCE_HOST_ORIGIN_LEDGER_SCHEMA_VERSION,
            "the first native-host companion permanently advances the ledger wire"
        );
        with_origin.extend_from_slice(&later_line);
        fs::write(&target, &with_origin).expect("install post-provenance WAL");
        assert_eq!(
            recover_under_lock(&root)
                .expect("recover permanent 0.5 epoch")
                .records
                .len(),
            4
        );

        let later_text = String::from_utf8(later_line).expect("later record UTF-8");
        for earlier_schema in [
            WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION,
            WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION,
            WORKFLOW_GOVERNANCE_INTENT_LEDGER_SCHEMA_VERSION,
            WORKFLOW_GOVERNANCE_REBASE_LEDGER_SCHEMA_VERSION,
        ] {
            let downgraded_later = later_text.replacen(
                &format!(
                    "\"schema_version\":\"{WORKFLOW_GOVERNANCE_HOST_ORIGIN_LEDGER_SCHEMA_VERSION}\""
                ),
                &format!("\"schema_version\":\"{earlier_schema}\""),
                1,
            );
            let mut downgraded_successor_wal = historical_wal.clone();
            downgraded_successor_wal.extend_from_slice(&origin_line);
            downgraded_successor_wal.extend_from_slice(downgraded_later.as_bytes());
            fs::write(&target, downgraded_successor_wal)
                .expect("install downgraded post-provenance WAL");
            assert!(matches!(
                recover_under_lock(&root),
                Err(WorkflowGovernanceLedgerError::UnsupportedSchema { line: 4, .. })
            ));
        }
        let origin_text = String::from_utf8(origin_line).expect("origin UTF-8");
        let downgraded = origin_text.replacen(
            &format!(
                "\"schema_version\":\"{WORKFLOW_GOVERNANCE_HOST_ORIGIN_LEDGER_SCHEMA_VERSION}\""
            ),
            &format!("\"schema_version\":\"{WORKFLOW_GOVERNANCE_REBASE_LEDGER_SCHEMA_VERSION}\""),
            1,
        );
        let mut downgraded_wal = historical_wal;
        downgraded_wal.extend_from_slice(downgraded.as_bytes());
        fs::write(&target, downgraded_wal).expect("install downgraded provenance WAL");
        assert!(matches!(
            recover_under_lock(&root),
            Err(WorkflowGovernanceLedgerError::UnsupportedSchema { line: 3, .. })
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn strict_replay_identity_advances_wire_to_0_6_and_missing_identity_fails_closed() {
        let root = test_root("strict-replay-origin-wire-successor");
        let (target, _, historical_wal) = valid_wal_versions(&root);
        fs::write(&target, &historical_wal).expect("install historical WAL");
        let historical_before = fs::read(&target).expect("historical bytes");
        let projection = recover_under_lock(&root).expect("historical projection");
        let action_record_digest = projection.head_digest.clone().expect("historical head");
        let (_, strict_origin_line) = build_record_line(
            &projection,
            &test_identity(),
            1,
            strict_native_host_origin_event(&action_record_digest),
        )
        .expect("strict replay provenance record");
        assert_eq!(
            schema_from_line(&strict_origin_line),
            WORKFLOW_GOVERNANCE_STRICT_REPLAY_LEDGER_SCHEMA_VERSION
        );

        let mut strict_wal = historical_wal.clone();
        strict_wal.extend_from_slice(&strict_origin_line);
        assert_eq!(
            &strict_wal[..historical_before.len()],
            historical_before.as_slice(),
            "the 0.6 successor must preserve every frozen historical byte"
        );
        fs::write(&target, &strict_wal).expect("install 0.6 WAL");
        let strict_projection = recover_under_lock(&root).expect("recover 0.6 wire");
        let WorkflowGovernanceEvent::BrokerOriginApplied(strict_origin) = &strict_projection
            .records
            .last()
            .expect("strict origin")
            .event
        else {
            panic!("strict projection must retain broker origin provenance");
        };
        assert!(strict_origin.native_interaction_replay_digest.is_some());
        assert!(matches!(
            build_record_line(
                &strict_projection,
                &test_identity(),
                1,
                native_host_origin_event(&strict_origin.action_record_digest),
            ),
            Err(WorkflowGovernanceLedgerError::InvalidBrokerOriginBinding { .. })
        ));
        let strict_head = strict_projection.head_digest.clone().expect("strict head");
        let (_, later_line) = build_record_line(
            &strict_projection,
            &test_identity(),
            1,
            broker_signal_event(&strict_head),
        )
        .expect("post-strict record");
        assert_eq!(
            schema_from_line(&later_line),
            WORKFLOW_GOVERNANCE_STRICT_REPLAY_LEDGER_SCHEMA_VERSION,
            "strict replay provenance must permanently advance the ledger wire"
        );

        let strict_text = String::from_utf8(strict_origin_line).expect("strict origin UTF-8");
        let downgraded = strict_text.replacen(
            &format!(
                "\"schema_version\":\"{WORKFLOW_GOVERNANCE_STRICT_REPLAY_LEDGER_SCHEMA_VERSION}\""
            ),
            &format!(
                "\"schema_version\":\"{WORKFLOW_GOVERNANCE_HOST_ORIGIN_LEDGER_SCHEMA_VERSION}\""
            ),
            1,
        );
        let mut downgraded_wal = historical_wal.clone();
        downgraded_wal.extend_from_slice(downgraded.as_bytes());
        fs::write(&target, downgraded_wal).expect("install downgraded strict provenance");
        assert!(matches!(
            recover_under_lock(&root),
            Err(WorkflowGovernanceLedgerError::UnsupportedSchema { line: 3, .. })
        ));

        let replay_digest = sha256_digest(b"strict-native-interaction-replay");
        let missing_identity = strict_text.replacen(
            &format!("\"native_interaction_replay_digest\":\"{replay_digest}\","),
            "",
            1,
        );
        assert_ne!(missing_identity, strict_text);
        let mut missing_identity_wal = historical_wal;
        missing_identity_wal.extend_from_slice(missing_identity.as_bytes());
        fs::write(&target, missing_identity_wal).expect("install missing strict identity");
        assert!(matches!(
            recover_under_lock(&root),
            Err(WorkflowGovernanceLedgerError::UnsupportedSchema { line: 3, .. })
        ));

        let mut invalid = strict_native_host_origin_event(&action_record_digest);
        let WorkflowGovernanceEvent::BrokerOriginApplied(origin) = &mut invalid else {
            unreachable!();
        };
        origin.native_interaction_replay_digest = Some("not-a-digest".to_owned());
        assert!(matches!(
            build_record_line(&projection, &test_identity(), 1, invalid),
            Err(WorkflowGovernanceLedgerError::InvalidBrokerOriginBinding { .. })
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn historical_wire_remains_byte_exact_until_an_intent_revision_exists() {
        let root = test_root("historical-wire-preserved");
        let (target, _, historical_wal) = valid_wal_versions(&root);
        fs::write(&target, &historical_wal).expect("install historical WAL");
        let before = fs::read(&target).expect("historical bytes before recovery");
        let recovered = recover_under_lock(&root).expect("recover historical wire");
        assert_eq!(recovered.records.len(), 2);
        assert_eq!(
            fs::read(&target).expect("historical bytes after recovery"),
            before,
            "recovery must not rewrite frozen 0.1 bytes"
        );
        for line in before.split_inclusive(|byte| *byte == b'\n') {
            if !line.is_empty() {
                assert_eq!(
                    schema_from_line(line),
                    WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION
                );
            }
        }
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn intent_revision_advances_the_wire_to_0_3_and_keeps_it_there() {
        let root = test_root("intent-wire-successor");
        let (target, _, historical_wal) = valid_wal_versions(&root);
        fs::write(&target, &historical_wal).expect("install historical WAL");
        let projection = recover_under_lock(&root).expect("historical projection");
        let head = projection.head_digest.clone().expect("historical head");
        let packet = sha256_digest(b"intent-packet");
        let origin = sha256_digest(b"intent-origin");
        let (_, intent_line) = build_deterministic_broker_record_line(
            &projection,
            &test_identity(),
            1,
            broker_intent_event(&head, &packet),
            &DeterministicBrokerRecordBinding {
                action_packet_digest: &packet,
                broker_event_digest: &origin,
                event_kind: "intent_revision",
                recorded_at_unix: 100,
            },
        )
        .expect("intent successor record");
        assert_eq!(
            schema_from_line(&intent_line),
            WORKFLOW_GOVERNANCE_INTENT_LEDGER_SCHEMA_VERSION
        );

        let mut with_intent = historical_wal;
        with_intent.extend_from_slice(&intent_line);
        fs::write(&target, &with_intent).expect("install intent WAL");
        let projection = recover_under_lock(&root).expect("recover intent successor wire");
        assert_eq!(
            fs::read(&target).expect("0.3 WAL after recovery"),
            with_intent,
            "0.3 recovery must preserve the 0.1 prefix and accepted-intent bytes"
        );
        let intent_head = projection.head_digest.clone().expect("intent head");
        let (_, later_line) = build_deterministic_broker_record_line(
            &projection,
            &test_identity(),
            1,
            broker_signal_event(&intent_head),
            &DeterministicBrokerRecordBinding {
                action_packet_digest: &sha256_digest(b"later-packet"),
                broker_event_digest: &sha256_digest(b"later-origin"),
                event_kind: "signal",
                recorded_at_unix: 100,
            },
        )
        .expect("post-intent record");
        assert_eq!(
            schema_from_line(&later_line),
            WORKFLOW_GOVERNANCE_INTENT_LEDGER_SCHEMA_VERSION,
            "the first accepted intent permanently advances the ledger wire"
        );
        with_intent.extend_from_slice(&later_line);
        fs::write(&target, &with_intent).expect("install post-intent WAL");
        assert_eq!(
            recover_under_lock(&root)
                .expect("recover post-intent successor wire")
                .records
                .len(),
            4
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn intent_event_cannot_be_downgraded_and_0_3_cannot_be_used_early() {
        for old_schema in [
            WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION,
            WORKFLOW_GOVERNANCE_REBASE_LEDGER_SCHEMA_VERSION,
            WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION,
        ] {
            let root = test_root(&format!("intent-wire-downgrade-{old_schema}"));
            let (target, _, historical_wal) = valid_wal_versions(&root);
            fs::write(&target, &historical_wal).expect("install historical WAL");
            let projection = recover_under_lock(&root).expect("historical projection");
            let head = projection.head_digest.clone().expect("historical head");
            let packet = sha256_digest(b"intent-packet");
            let origin = sha256_digest(b"intent-origin");
            let (_, intent_line) = build_deterministic_broker_record_line(
                &projection,
                &test_identity(),
                1,
                broker_intent_event(&head, &packet),
                &DeterministicBrokerRecordBinding {
                    action_packet_digest: &packet,
                    broker_event_digest: &origin,
                    event_kind: "intent_revision",
                    recorded_at_unix: 100,
                },
            )
            .expect("intent successor record");
            let intent_text = String::from_utf8(intent_line).expect("intent UTF-8");
            let downgraded = intent_text.replacen(
                &format!(
                    "\"schema_version\":\"{WORKFLOW_GOVERNANCE_INTENT_LEDGER_SCHEMA_VERSION}\""
                ),
                &format!("\"schema_version\":\"{old_schema}\""),
                1,
            );
            let mut mutant = historical_wal;
            mutant.extend_from_slice(downgraded.as_bytes());
            fs::write(&target, mutant).expect("install downgraded intent WAL");
            assert!(matches!(
                recover_under_lock(&root),
                Err(WorkflowGovernanceLedgerError::UnsupportedSchema { line: 3, .. })
            ));
            fs::remove_dir_all(root).expect("cleanup");
        }

        let root = test_root("intent-wire-too-early");
        let (target, _, historical_wal) = valid_wal_versions(&root);
        let historical_text = String::from_utf8(historical_wal).expect("historical UTF-8");
        let premature = historical_text.replacen(
            &format!("\"schema_version\":\"{WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION}\""),
            &format!("\"schema_version\":\"{WORKFLOW_GOVERNANCE_INTENT_LEDGER_SCHEMA_VERSION}\""),
            1,
        );
        fs::write(&target, premature).expect("install premature 0.3 WAL");
        assert!(matches!(
            recover_under_lock(&root),
            Err(WorkflowGovernanceLedgerError::UnsupportedSchema { line: 1, .. })
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    fn assert_intent_broker_path_is_deterministic_and_exclusive(
        root: &Path,
        head: &str,
        identity: &WorkflowGovernanceLedgerIdentity,
        packet: &str,
        origin: &str,
    ) {
        let intent_first = {
            let mut ledger = lock_workflow_governance_ledger_tcb(root).expect("intent ledger");
            let mut batch = ledger
                .begin_unchecked_tcb_batch(head, identity)
                .expect("intent batch");
            batch
                .push_verified_broker_action_unchecked_tcb(
                    0,
                    broker_intent_event(head, packet),
                    packet,
                    origin,
                    100,
                )
                .expect("first deterministic intent record")
        };
        let intent_retry = {
            let mut ledger =
                lock_workflow_governance_ledger_tcb(root).expect("intent retry ledger");
            let mut batch = ledger
                .begin_unchecked_tcb_batch(head, identity)
                .expect("intent retry batch");
            batch
                .push_verified_broker_action_unchecked_tcb(
                    0,
                    broker_intent_event(head, packet),
                    packet,
                    origin,
                    100,
                )
                .expect("exact deterministic intent retry")
        };
        assert_eq!(intent_first, intent_retry);
        let mut ledger = lock_workflow_governance_ledger_tcb(root).expect("generic intent ledger");
        let mut batch = ledger
            .begin_unchecked_tcb_batch(head, identity)
            .expect("generic intent batch");
        assert!(matches!(
            batch.push_event(0, broker_intent_event(head, packet)),
            Err(WorkflowGovernanceLedgerError::HumanIntentRevisionRequiresBrokerAuthority)
        ));
    }

    #[test]
    fn broker_action_record_is_exactly_retryable_while_legacy_api_remains_random() {
        let root = test_root("deterministic-broker-record");
        let (_, _, _) = valid_wal_versions(&root);
        let identity = test_identity();
        let projection = recover_under_lock(&root).expect("projection");
        let head = projection.head_digest.clone().expect("head");
        let packet = sha256_digest(b"packet");
        let origin = sha256_digest(b"origin-event");

        let first = {
            let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("ledger");
            let mut batch = ledger
                .begin_unchecked_tcb_batch(&head, &identity)
                .expect("batch");
            batch
                .push_verified_broker_action_unchecked_tcb(
                    0,
                    broker_signal_event(&head),
                    &packet,
                    &origin,
                    100,
                )
                .expect("first deterministic record")
        };
        let retry = {
            let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("ledger retry");
            let mut batch = ledger
                .begin_unchecked_tcb_batch(&head, &identity)
                .expect("retry batch");
            batch
                .push_verified_broker_action_unchecked_tcb(
                    0,
                    broker_signal_event(&head),
                    &packet,
                    &origin,
                    100,
                )
                .expect("exact deterministic retry")
        };
        assert_eq!(first, retry);
        assert_intent_broker_path_is_deterministic_and_exclusive(
            &root, &head, &identity, &packet, &origin,
        );

        let (legacy_one, legacy_two) = {
            let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("legacy ledger");
            let mut batch = ledger
                .begin_unchecked_tcb_batch(&head, &identity)
                .expect("legacy batch");
            let one = batch
                .push_event(0, broker_signal_event(&head))
                .expect("legacy random record");
            drop(batch);
            let mut batch = ledger
                .begin_unchecked_tcb_batch(&head, &identity)
                .expect("legacy retry batch");
            let two = batch
                .push_event(0, broker_signal_event(&head))
                .expect("legacy second random record");
            (one, two)
        };
        assert_ne!(legacy_one.record_id, legacy_two.record_id);
        assert!(matches!(
            lock_workflow_governance_ledger_tcb(&root)
                .expect("wrong-head ledger")
                .begin_unchecked_tcb_batch(&sha256_digest(b"wrong-head"), &identity),
            Err(WorkflowGovernanceLedgerError::HeadMismatch { .. })
        ));

        let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("state ledger");
        let mut batch = ledger
            .begin_unchecked_tcb_batch(&head, &identity)
            .expect("state batch");
        batch
            .push_event(
                1,
                WorkflowGovernanceEvent::PhaseAdvanced(PhaseAdvancedEvent {
                    from_phase: Some(StableId("discover".to_owned())),
                    to_phase: StableId("define".to_owned()),
                    snapshot_digest: sha256_digest(b"snapshot-next"),
                }),
            )
            .expect("advance prepared state");
        assert!(matches!(
            batch.push_verified_broker_action_unchecked_tcb(
                0,
                broker_signal_event(&head),
                &packet,
                &origin,
                100,
            ),
            Err(WorkflowGovernanceLedgerError::StateVersionRegression { .. })
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn cooperative_objective_wire_is_0_10_non_downgradable_and_preserves_0_9_bytes() {
        let root = test_root("cooperative-objective-wire");
        let (projection, genesis, frozen_0_9) = initialize_solo_profile(&root);
        let event = cooperative_objective_event(
            &genesis.record_digest,
            "objective.workflow.project-protocol-test".to_owned(),
            "principal.same-owner".to_owned(),
            90,
            100,
        );
        let accepted = lock_workflow_governance_ledger_tcb(&root)
            .expect("objective ledger")
            .accept_cooperative_objective_unchecked_tcb(
                &genesis.record_digest,
                &test_identity(),
                projection.current_state_version().unwrap_or_default(),
                event,
            )
            .expect("accept cooperative objective");
        let complete =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("0.10 WAL");
        assert_eq!(
            &complete[..frozen_0_9.len()],
            frozen_0_9.as_slice(),
            "0.10 append must preserve the exact 0.9 prefix bytes and hashes"
        );
        let last = complete
            .split_inclusive(|byte| *byte == b'\n')
            .last()
            .expect("0.10 record");
        assert_eq!(
            schema_from_line(last),
            WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_LEDGER_SCHEMA_VERSION
        );
        let recovered = recover_under_lock(&root).expect("recover 0.10");
        assert_eq!(recovered.head_digest, Some(accepted.record_digest));
        assert_eq!(
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("WAL after recovery"),
            complete,
            "0.10 recovery must not rewrite the historical 0.9 prefix or new record"
        );

        let last_text = String::from_utf8(last.to_vec()).expect("0.10 UTF-8");
        let downgraded = last_text.replacen(
            &format!(
                "\"schema_version\":\"{WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_LEDGER_SCHEMA_VERSION}\""
            ),
            &format!(
                "\"schema_version\":\"{WORKFLOW_GOVERNANCE_READINESS_PROFILE_LEDGER_SCHEMA_VERSION}\""
            ),
            1,
        );
        let mut downgraded_wal = frozen_0_9.clone();
        downgraded_wal.extend_from_slice(downgraded.as_bytes());
        fs::write(
            root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH),
            downgraded_wal,
        )
        .expect("install downgraded 0.10 event");
        assert!(matches!(
            recover_under_lock(&root),
            Err(WorkflowGovernanceLedgerError::UnsupportedSchema { line: 2, .. })
        ));

        let premature = String::from_utf8(frozen_0_9).expect("0.9 UTF-8").replacen(
            &format!(
                "\"schema_version\":\"{WORKFLOW_GOVERNANCE_READINESS_PROFILE_LEDGER_SCHEMA_VERSION}\""
            ),
            &format!(
                "\"schema_version\":\"{WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_LEDGER_SCHEMA_VERSION}\""
            ),
            1,
        );
        fs::write(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH), premature)
            .expect("install premature 0.10 genesis");
        assert!(matches!(
            recover_under_lock(&root),
            Err(WorkflowGovernanceLedgerError::UnsupportedSchema { line: 1, .. })
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn cooperative_objective_revision_advances_to_0_11_and_recovers_the_full_chain() {
        let root = test_root("cooperative-objective-revision-wire");
        let (projection, genesis, _) = initialize_solo_profile(&root);
        let initial = cooperative_objective_event(
            &genesis.record_digest,
            "objective.workflow.project-protocol-test".to_owned(),
            "principal.same-owner".to_owned(),
            90,
            100,
        );
        let initial_digest = initial.objective_digest.clone();
        let initial_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("initial ledger")
            .accept_cooperative_objective_unchecked_tcb(
                &genesis.record_digest,
                &test_identity(),
                projection.current_state_version().unwrap_or_default(),
                initial,
            )
            .expect("accept initial objective");
        let after_initial = recover_under_lock(&root).expect("recover initial objective");
        let mut revision = cooperative_objective_event(
            &initial_record.record_digest,
            "objective.workflow.project-protocol-test".to_owned(),
            "principal.same-owner".to_owned(),
            101,
            102,
        );
        revision.revision = 2;
        revision.assurance_epoch = 2;
        revision.proposal.outcome = "Dogfood Forge excellently before expanding teams".to_owned();
        revision.previous_objective_digest = Some(initial_digest);
        revision.revision_kind = WorkflowCooperativeObjectiveRevisionKind::MaterialSupersession;
        revision.revision_reason =
            Some("The owner narrowed the immediate product direction".to_owned());
        revision.objective_digest =
            cooperative_objective_digest(&revision).expect("canonical revised objective");
        revision.revision_input_digest = Some(
            cooperative_revision_input_digest(
                &WorkflowCooperativeObjectiveInput::MaterialSupersession {
                    proposal: revision.proposal.clone(),
                    supersession_reason: revision.revision_reason.clone().expect("revision reason"),
                    carrying_principal: revision.carrying_principal.clone(),
                    host_provenance: revision.host_provenance.clone(),
                },
            )
            .expect("canonical revision input"),
        );
        let revised = lock_workflow_governance_ledger_tcb(&root)
            .expect("revision ledger")
            .accept_cooperative_objective_unchecked_tcb(
                &initial_record.record_digest,
                &test_identity(),
                after_initial.current_state_version().unwrap_or_default(),
                revision,
            )
            .expect("accept objective revision");

        let wal = fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("revision WAL");
        let lines = wal
            .split_inclusive(|byte| *byte == b'\n')
            .collect::<Vec<_>>();
        assert_eq!(
            schema_from_line(lines[1]),
            WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_LEDGER_SCHEMA_VERSION
        );
        assert!(
            !String::from_utf8_lossy(lines[1]).contains("revision_kind"),
            "frozen 0.10 initial record must not gain revision metadata"
        );
        assert_eq!(
            schema_from_line(lines[2]),
            WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_REVISION_LEDGER_SCHEMA_VERSION
        );
        let recovered = recover_under_lock(&root).expect("recover 0.11 history");
        assert_eq!(recovered.records.len(), 3);
        assert_eq!(recovered.head_digest, Some(revised.record_digest));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn cooperative_objective_adversarial_metadata_digests_and_clarifications_fail_closed() {
        for (field, value) in [
            ("revision_kind", serde_json::json!("initial")),
            ("revision_reason", serde_json::Value::Null),
            ("revision_input_digest", serde_json::Value::Null),
        ] {
            let root = test_root(&format!("cooperative-frozen-metadata-{field}"));
            let (projection, genesis, mut wal) = initialize_solo_profile(&root);
            let event = cooperative_objective_event(
                &genesis.record_digest,
                "objective.workflow.project-protocol-test".to_owned(),
                "principal.same-owner".to_owned(),
                90,
                100,
            );
            let (_, line) = build_record_line_at(
                &projection,
                &test_identity(),
                projection.current_state_version().unwrap_or_default(),
                WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event),
                100,
            )
            .expect("valid frozen objective line");
            let mut document: serde_json::Value =
                serde_json::from_slice(&line).expect("objective JSON");
            document["workflow_governance_receipt"]["event"]["payload"]
                .as_object_mut()
                .expect("objective payload")
                .insert(field.to_owned(), value);
            let mut tampered = serde_json::to_vec(&document).expect("tampered objective JSON");
            tampered.push(b'\n');
            wal.extend_from_slice(&tampered);
            fs::write(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH), wal)
                .expect("install explicit frozen metadata");
            assert!(matches!(
                recover_under_lock(&root),
                Err(WorkflowGovernanceLedgerError::MalformedRecord { line: 2, .. })
            ));
            fs::remove_dir_all(root).expect("cleanup frozen metadata");
        }

        let root = test_root("cooperative-false-objective-digest");
        let (projection, genesis, prefix) = initialize_solo_profile(&root);
        let mut false_digest = cooperative_objective_event(
            &genesis.record_digest,
            "objective.workflow.project-protocol-test".to_owned(),
            "principal.same-owner".to_owned(),
            90,
            100,
        );
        false_digest.objective_digest = sha256_digest(b"semantically-false-objective");
        assert!(matches!(
            lock_workflow_governance_ledger_tcb(&root)
                .expect("false-digest append ledger")
                .accept_cooperative_objective_unchecked_tcb(
                    &genesis.record_digest,
                    &test_identity(),
                    projection.current_state_version().unwrap_or_default(),
                    false_digest.clone(),
                ),
            Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid { .. })
        ));
        let (_, false_line) = build_record_line_at(
            &projection,
            &test_identity(),
            projection.current_state_version().unwrap_or_default(),
            WorkflowGovernanceEvent::CooperativeObjectiveAccepted(false_digest),
            100,
        )
        .expect("build false-digest recovery record");
        let mut wal = prefix;
        wal.extend_from_slice(&false_line);
        fs::write(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH), wal)
            .expect("install false objective digest");
        assert!(matches!(
            recover_under_lock(&root),
            Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid { line: Some(2), .. })
        ));
        fs::remove_dir_all(root).expect("cleanup false objective digest");

        let root = test_root("cooperative-duplicate-clarification");
        let (projection, genesis, _) = initialize_solo_profile(&root);
        let initial = cooperative_objective_event(
            &genesis.record_digest,
            "objective.workflow.project-protocol-test".to_owned(),
            "principal.same-owner".to_owned(),
            90,
            100,
        );
        let initial_digest = initial.objective_digest.clone();
        let initial_record = lock_workflow_governance_ledger_tcb(&root)
            .expect("initial duplicate-clarification ledger")
            .accept_cooperative_objective_unchecked_tcb(
                &genesis.record_digest,
                &test_identity(),
                projection.current_state_version().unwrap_or_default(),
                initial,
            )
            .expect("accept initial objective");
        let after_initial = recover_under_lock(&root).expect("recover initial objective");
        let mut clarification = cooperative_objective_event(
            &initial_record.record_digest,
            "objective.workflow.project-protocol-test".to_owned(),
            "principal.same-owner".to_owned(),
            101,
            102,
        );
        clarification.revision = 2;
        clarification.assurance_epoch = 2;
        clarification.previous_objective_digest = Some(initial_digest);
        clarification.revision_kind =
            WorkflowCooperativeObjectiveRevisionKind::NonMaterialClarification;
        clarification.revision_reason = Some("Repeat an existing detail".to_owned());
        clarification
            .proposal
            .constraints
            .push("Remain host neutral".to_owned());
        clarification.objective_digest =
            cooperative_objective_digest(&clarification).expect("clarification objective digest");
        clarification.revision_input_digest = Some(
            cooperative_revision_input_digest(
                &WorkflowCooperativeObjectiveInput::NonMaterialClarification {
                    added_constraints: vec!["Remain host neutral".to_owned()],
                    added_unacceptable_outcomes: Vec::new(),
                    added_open_uncertainties: Vec::new(),
                    clarification_reason: clarification
                        .revision_reason
                        .clone()
                        .expect("clarification reason"),
                    carrying_principal: clarification.carrying_principal.clone(),
                    host_provenance: clarification.host_provenance.clone(),
                },
            )
            .expect("clarification input digest"),
        );
        assert!(matches!(
            lock_workflow_governance_ledger_tcb(&root)
                .expect("duplicate clarification append ledger")
                .accept_cooperative_objective_unchecked_tcb(
                    &initial_record.record_digest,
                    &test_identity(),
                    after_initial.current_state_version().unwrap_or_default(),
                    clarification.clone(),
                ),
            Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid { .. })
        ));
        let (_, invalid_line) = build_record_line_at(
            &after_initial,
            &test_identity(),
            after_initial.current_state_version().unwrap_or_default(),
            WorkflowGovernanceEvent::CooperativeObjectiveAccepted(clarification),
            102,
        )
        .expect("build duplicate clarification recovery record");
        let mut wal =
            fs::read(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)).expect("objective WAL");
        wal.extend_from_slice(&invalid_line);
        fs::write(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH), wal)
            .expect("install duplicate clarification");
        assert!(matches!(
            recover_under_lock(&root),
            Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid { line: Some(3), .. })
        ));
        fs::remove_dir_all(root).expect("cleanup duplicate clarification");
    }

    #[test]
    fn cooperative_epoch_preserves_permanent_strict_replay_invariant() {
        let root = test_root("cooperative-strict-replay");
        let (projection, genesis, mut wal) = initialize_solo_profile(&root);
        let event = cooperative_objective_event(
            &genesis.record_digest,
            "objective.workflow.project-protocol-test".to_owned(),
            "principal.same-owner".to_owned(),
            90,
            100,
        );
        let (_, objective_line) = build_record_line_at(
            &projection,
            &test_identity(),
            0,
            WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event),
            100,
        )
        .expect("0.10 objective line");
        wal.extend_from_slice(&objective_line);
        fs::write(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH), &wal)
            .expect("install objective chain");
        let cooperative = recover_under_lock(&root).expect("recover cooperative epoch");

        let cooperative_head = cooperative.head_digest.clone().expect("cooperative head");
        let (_, strict_line) = build_record_line_at(
            &cooperative,
            &test_identity(),
            0,
            strict_native_host_origin_event(&cooperative_head),
            101,
        )
        .expect("strict companion under 0.10");
        assert_eq!(
            schema_from_line(&strict_line),
            WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_LEDGER_SCHEMA_VERSION
        );
        wal.extend_from_slice(&strict_line);
        fs::write(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH), &wal)
            .expect("install strict 0.10 chain");
        let strict = recover_under_lock(&root).expect("recover strict identity under 0.10");

        let strict_head = strict.head_digest.clone().expect("strict head");
        let event = native_host_origin_event(&strict_head);
        assert!(matches!(
            build_record_line_at(&strict, &test_identity(), 0, event.clone(), 102),
            Err(WorkflowGovernanceLedgerError::InvalidBrokerOriginBinding { .. })
        ));

        // Construct the adversarial record below the normal write boundary so
        // recovery itself must enforce the permanent strict invariant.
        let mut missing = WorkflowGovernanceLedgerRecord {
            record_id: StableId("wglr-adversarial-missing-strict-replay".to_owned()),
            sequence: strict.next_sequence,
            project_id: test_identity().project_id,
            bundle_id: test_identity().bundle_id,
            bundle_digest: test_identity().bundle_digest,
            state_version: strict.current_state_version().unwrap_or_default(),
            previous_record_digest: Some(strict_head),
            record_digest: String::new(),
            recorded_at_unix: 102,
            event,
        };
        missing.record_digest =
            workflow_governance_record_digest(&missing).expect("adversarial record digest");
        let mut missing_line = serde_json::to_vec(&WorkflowGovernanceReceiptDocument {
            schema_version: WORKFLOW_GOVERNANCE_COOPERATIVE_OBJECTIVE_LEDGER_SCHEMA_VERSION
                .to_owned(),
            workflow_governance_receipt: missing,
        })
        .expect("adversarial record wire");
        missing_line.push(b'\n');
        wal.extend_from_slice(&missing_line);
        fs::write(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH), wal)
            .expect("install missing strict replay companion");
        assert!(matches!(
            recover_under_lock(&root),
            Err(WorkflowGovernanceLedgerError::InvalidBrokerOriginBinding { line: Some(4), .. })
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn cooperative_objective_bounds_and_clock_hold_for_append_and_recovery() {
        let exact_root = test_root("cooperative-bounds-exact");
        let (projection, genesis, _) = initialize_solo_profile(&exact_root);
        let exact = cooperative_objective_event(
            &genesis.record_digest,
            "o".repeat(MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES),
            "p".repeat(MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES),
            100,
            100,
        );
        lock_workflow_governance_ledger_tcb(&exact_root)
            .expect("exact-bound ledger")
            .accept_cooperative_objective_unchecked_tcb(
                &genesis.record_digest,
                &test_identity(),
                projection.current_state_version().unwrap_or_default(),
                exact,
            )
            .expect("exact 1024-byte ids must pass");
        recover_under_lock(&exact_root).expect("recover exact-bound objective");
        fs::remove_dir_all(exact_root).expect("cleanup exact bounds");

        for (label, event) in [
            (
                "objective-id-oversize",
                cooperative_objective_event(
                    "unused",
                    "o".repeat(MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES + 1),
                    "principal".to_owned(),
                    90,
                    100,
                ),
            ),
            (
                "principal-oversize",
                cooperative_objective_event(
                    "unused",
                    "objective".to_owned(),
                    "p".repeat(MAX_WORKFLOW_COOPERATIVE_HOST_TEXT_BYTES + 1),
                    90,
                    100,
                ),
            ),
            (
                "clock-inversion",
                cooperative_objective_event(
                    "unused",
                    "objective".to_owned(),
                    "principal".to_owned(),
                    101,
                    100,
                ),
            ),
        ] {
            let root = test_root(label);
            let (projection, genesis, prefix) = initialize_solo_profile(&root);
            let mut event = event;
            event.ledger_head_digest = genesis.record_digest.clone();
            assert!(matches!(
                lock_workflow_governance_ledger_tcb(&root)
                    .expect("invalid append ledger")
                    .accept_cooperative_objective_unchecked_tcb(
                        &genesis.record_digest,
                        &test_identity(),
                        projection.current_state_version().unwrap_or_default(),
                        event.clone(),
                    ),
                Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid { .. })
            ));
            let (_, invalid_line) = build_record_line_at(
                &projection,
                &test_identity(),
                projection.current_state_version().unwrap_or_default(),
                WorkflowGovernanceEvent::CooperativeObjectiveAccepted(event),
                100,
            )
            .expect("build adversarial invalid cooperative record");
            let mut wal = prefix;
            wal.extend_from_slice(&invalid_line);
            fs::write(root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH), wal)
                .expect("install adversarial invalid cooperative record");
            assert!(matches!(
                recover_under_lock(&root),
                Err(WorkflowGovernanceLedgerError::CooperativeObjectiveInvalid {
                    line: Some(2),
                    ..
                })
            ));
            fs::remove_dir_all(root).expect("cleanup invalid bounds");
        }
    }
}
