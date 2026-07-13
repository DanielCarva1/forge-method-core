//! Strict, append-only storage for authoritative workflow-governance receipts.
//!
//! This store deliberately has no torn-tail repair. A malformed, truncated,
//! oversized, or internally inconsistent ledger fails closed and requires an
//! operator-mediated recovery from a known-good durable copy.
//!
//! Replacement recovery protects against interrupted local writes. It is not
//! an external rollback anchor: an actor able to replace the WAL and remove all
//! protocol artifacts can still present an older, internally valid ledger.

use forge_core_contracts::{
    DomainPackGenerationTransitionedEvent, ReleaseUpgradedEvent, StableId,
    WorkflowEffectiveBundleIdentity, WorkflowGovernanceEvent, WorkflowGovernanceLedgerRecord,
    WorkflowGovernanceReceiptDocument, WorkflowGovernanceReleaseIdentity, WorkflowReceiptCarryover,
    WorkflowRuntimeBundleIdentity, WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION,
};
use forge_core_store::{acquire_effect_store_lock, EffectStoreLock, EffectStoreLockError};
use serde_json_canonicalizer::to_vec as to_canonical_json;
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::cell::Cell;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
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
            if let WorkflowGovernanceEvent::ReleaseUpgraded(event) = &record.event {
                active.bundle_id = event.to_runtime_bundle.bundle_id.clone();
                active
                    .bundle_digest
                    .clone_from(&event.to_runtime_bundle.bundle_digest);
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
        self.records.iter().rev().find_map(|record| {
            if let WorkflowGovernanceEvent::ReleaseUpgraded(event) = &record.event {
                Some(event.to_runtime_bundle.clone())
            } else {
                None
            }
        })
    }

    /// Last effective core-plus-Domain-Pack epoch durably adopted by the
    /// workflow ledger. `None` is the backward-compatible core-only state.
    #[must_use]
    pub fn active_effective_bundle_identity(&self) -> Option<WorkflowEffectiveBundleIdentity> {
        self.records.iter().rev().find_map(|record| {
            if let WorkflowGovernanceEvent::DomainPackGenerationTransitioned(event) = &record.event
            {
                Some(event.to_effective_bundle.clone())
            } else {
                None
            }
        })
    }

    #[must_use]
    pub fn current_state_version(&self) -> Option<u64> {
        self.records.last().map(|record| record.state_version)
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
    _lock: EffectStoreLock,
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
            WorkflowGovernanceEvent::DomainPackGenerationTransitioned(_)
        ) {
            return Err(
                WorkflowGovernanceLedgerError::DomainPackTransitionRequiresDedicatedAuthority,
            );
        }
        if matches!(event, WorkflowGovernanceEvent::ProjectImported(_)) {
            return Err(WorkflowGovernanceLedgerError::ProjectImportedAfterInitialization);
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
    DomainPackTransitionRequiresDedicatedAuthority,
    InvalidBrokerActionBinding {
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
            Self::DomainPackTransitionRequiresDedicatedAuthority => write!(
                formatter,
                "domain_pack_generation_transitioned requires the dedicated TCB transition API"
            ),
            Self::InvalidBrokerActionBinding { reason } => {
                write!(formatter, "verified broker action binding is invalid: {reason}")
            }
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
    Ok(LockedWorkflowGovernanceLedger {
        state_root,
        _lock: lock,
    })
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
    let mut reader = BufReader::new(file);
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
            .map_err(|source| io_error(&wal_path, source))?;
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
        let document: WorkflowGovernanceReceiptDocument = serde_json::from_slice(&line_bytes)
            .map_err(|error| WorkflowGovernanceLedgerError::MalformedRecord {
                line: line_number,
                source: error.to_string(),
            })?;
        if document.schema_version != WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION
            && document.schema_version != WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION
        {
            return Err(WorkflowGovernanceLedgerError::UnsupportedSchema {
                line: line_number,
                found: document.schema_version,
            });
        }
        let effective_wire =
            document.schema_version == WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION;
        let record = document.workflow_governance_receipt;
        let is_domain_transition = matches!(
            &record.event,
            WorkflowGovernanceEvent::DomainPackGenerationTransitioned(_)
        );
        if effective_wire != (is_domain_transition || identity_state.active_effective.is_some()) {
            return Err(WorkflowGovernanceLedgerError::UnsupportedSchema {
                line: line_number,
                found: if effective_wire {
                    "0.2 before a Domain Pack effective epoch".to_owned()
                } else {
                    "0.1 after a Domain Pack effective epoch".to_owned()
                },
            });
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
struct RecoveredIdentityState {
    genesis: Option<WorkflowGovernanceLedgerIdentity>,
    active: Option<WorkflowGovernanceLedgerIdentity>,
    active_release: Option<WorkflowGovernanceReleaseIdentity>,
    active_runtime: Option<WorkflowRuntimeBundleIdentity>,
    active_effective: Option<WorkflowEffectiveBundleIdentity>,
}

fn validate_recovered_semantics(
    record: &WorkflowGovernanceLedgerRecord,
    line: usize,
    identity: &mut RecoveredIdentityState,
    previous_state_version: Option<u64>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if line == 1 {
        if !matches!(record.event, WorkflowGovernanceEvent::ProjectImported(_)) {
            return Err(WorkflowGovernanceLedgerError::FirstEventNotProjectImported);
        }
        let genesis = WorkflowGovernanceLedgerIdentity::from_record(record);
        identity.genesis = Some(genesis.clone());
        identity.active = Some(genesis);
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
    Ok(())
}

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
    let mut record = WorkflowGovernanceLedgerRecord {
        record_id: unique_record_id(&projection.records)?,
        sequence: projection.next_sequence,
        project_id: identity.project_id.clone(),
        bundle_id: identity.bundle_id.clone(),
        bundle_digest: identity.bundle_digest.clone(),
        state_version,
        previous_record_digest: projection.head_digest.clone(),
        record_digest: String::new(),
        recorded_at_unix: unix_time()?,
        event,
    };
    record.record_digest = workflow_governance_record_digest(&record)?;
    let effective_wire = matches!(
        &record.event,
        WorkflowGovernanceEvent::DomainPackGenerationTransitioned(_)
    ) || projection.active_effective_bundle_identity().is_some();
    let document = WorkflowGovernanceReceiptDocument {
        schema_version: if effective_wire {
            WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION
        } else {
            WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION
        }
        .to_owned(),
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
    let effective_wire = projection.active_effective_bundle_identity().is_some();
    let document = WorkflowGovernanceReceiptDocument {
        schema_version: if effective_wire {
            WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION
        } else {
            WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION
        }
        .to_owned(),
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

const fn broker_action_event_kind(event: &WorkflowGovernanceEvent) -> Option<&'static str> {
    match event {
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

fn active_release_identity(
    projection: &WorkflowGovernanceLedgerProjection,
) -> Option<WorkflowGovernanceReleaseIdentity> {
    projection.records.iter().rev().find_map(|record| {
        if let WorkflowGovernanceEvent::ReleaseUpgraded(event) = &record.event {
            Some(event.to_release.clone())
        } else {
            None
        }
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
    validate_nonblank(&record.record_digest, line, "record_digest")
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

    // Each fixed artifact is synced before the subsequent namespace change.
    // `sync_parent_dir` is best-effort on Windows, where std cannot open a
    // directory with the flags required by `FlushFileBuffers`.
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

#[cfg(not(unix))]
fn sync_parent_dir(parent: &Path) -> io::Result<()> {
    if parent.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "parent directory is empty",
        ));
    }
    let _ = File::open(parent).and_then(|file| file.sync_all());
    Ok(())
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
        PhaseAdvancedEvent, PrincipalId, ProjectImportedEvent, SignalChangedEvent,
        WorkflowGovernanceSignal, WorkflowReceiptCarryover, WorkflowReleaseAdmissionProof,
        WorkflowReleaseRegistryProvenance,
    };
    use std::panic::{catch_unwind, AssertUnwindSafe};

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
}
