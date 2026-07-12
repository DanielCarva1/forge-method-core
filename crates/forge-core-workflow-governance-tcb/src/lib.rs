//! Strict, append-only storage for authoritative workflow-governance receipts.
//!
//! This store deliberately has no torn-tail repair. A malformed, truncated,
//! oversized, or internally inconsistent ledger fails closed and requires an
//! operator-mediated recovery from a known-good durable copy.

use forge_core_contracts::{
    StableId, WorkflowGovernanceEvent, WorkflowGovernanceLedgerRecord,
    WorkflowGovernanceReceiptDocument, WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION,
};
use forge_core_store::{acquire_effect_store_lock, EffectStoreLock, EffectStoreLockError};
use serde_json_canonicalizer::to_vec as to_canonical_json;
use sha2::{Digest, Sha256};
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
    #[must_use]
    pub fn identity(&self) -> Option<WorkflowGovernanceLedgerIdentity> {
        self.records
            .first()
            .map(|record| WorkflowGovernanceLedgerIdentity {
                project_id: record.project_id.clone(),
                bundle_id: record.bundle_id.clone(),
                bundle_digest: record.bundle_digest.clone(),
            })
    }

    #[must_use]
    pub fn current_state_version(&self) -> Option<u64> {
        self.records.last().map(|record| record.state_version)
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

    /// Return the recovered ledger plus every record prepared so far.
    #[must_use]
    pub fn projection(&self) -> &WorkflowGovernanceLedgerProjection {
        &self.projection
    }

    /// Persist every prepared record in one atomic WAL replacement.
    ///
    /// # Errors
    ///
    /// Rejects an empty batch and forwards safe-path or atomic replacement
    /// failures. The replacement helper restores the original WAL when its
    /// final rename fails.
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
    let mut identity: Option<WorkflowGovernanceLedgerIdentity> = None;
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
        if document.schema_version != WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION {
            return Err(WorkflowGovernanceLedgerError::UnsupportedSchema {
                line: line_number,
                found: document.schema_version,
            });
        }
        let record = document.workflow_governance_receipt;
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
        validate_recovered_semantics(&record, line_number, &mut identity, previous_state_version)?;
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

fn validate_recovered_semantics(
    record: &WorkflowGovernanceLedgerRecord,
    line: usize,
    identity: &mut Option<WorkflowGovernanceLedgerIdentity>,
    previous_state_version: Option<u64>,
) -> Result<(), WorkflowGovernanceLedgerError> {
    if line == 1 {
        if !matches!(record.event, WorkflowGovernanceEvent::ProjectImported(_)) {
            return Err(WorkflowGovernanceLedgerError::FirstEventNotProjectImported);
        }
        *identity = Some(WorkflowGovernanceLedgerIdentity {
            project_id: record.project_id.clone(),
            bundle_id: record.bundle_id.clone(),
            bundle_digest: record.bundle_digest.clone(),
        });
    } else if matches!(record.event, WorkflowGovernanceEvent::ProjectImported(_)) {
        return Err(WorkflowGovernanceLedgerError::ProjectImportedAfterInitialization);
    }
    if let Some(expected) = identity.as_ref() {
        if record.project_id != expected.project_id {
            return Err(WorkflowGovernanceLedgerError::ProjectMismatch {
                line: Some(line),
                expected: expected.project_id.clone(),
                found: record.project_id.clone(),
            });
        }
        if record.bundle_id != expected.bundle_id || record.bundle_digest != expected.bundle_digest
        {
            return Err(WorkflowGovernanceLedgerError::BundleMismatch {
                line: Some(line),
                expected_id: expected.bundle_id.clone(),
                found_id: record.bundle_id.clone(),
                expected_digest: expected.bundle_digest.clone(),
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
    let document = WorkflowGovernanceReceiptDocument {
        schema_version: WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION.to_owned(),
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
        .identity()
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

fn atomic_replace_file(target: &Path, content: &[u8]) -> io::Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    fs::create_dir_all(parent)?;
    if target.exists() && !fs::symlink_metadata(target)?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "workflow-governance WAL target is not a regular file",
        ));
    }

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

    #[cfg(unix)]
    {
        if let Err(error) = fs::rename(&temp, target) {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
        sync_parent_dir(parent)?;
        return Ok(());
    }

    #[cfg(not(unix))]
    let backup = parent.join(format!(".{file_name}.{nonce}.forge-bak"));
    #[cfg(not(unix))]
    let had_target = target.exists();
    #[cfg(not(unix))]
    if had_target {
        if let Err(error) = fs::rename(target, &backup) {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
        if let Err(error) = sync_parent_dir(parent) {
            let _ = fs::rename(&backup, target);
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
    }

    #[cfg(not(unix))]
    if let Err(error) = fs::rename(&temp, target) {
        let _ = fs::remove_file(&temp);
        if had_target {
            let _ = fs::rename(&backup, target);
        }
        let _ = sync_parent_dir(parent);
        return Err(error);
    }
    #[cfg(not(unix))]
    sync_parent_dir(parent)?;

    #[cfg(not(unix))]
    if had_target {
        fs::remove_file(&backup)?;
        sync_parent_dir(parent)?;
    }
    #[cfg(not(unix))]
    Ok(())
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
