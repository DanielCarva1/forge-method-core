//! Durable idempotency state for Workflow Action Packet application.
//!
//! The store is deliberately independent from the effect lock. Every reserve
//! or commit holds its own exclusive lock across full WAL verification,
//! conflict checks, append, flush, and `fsync`. Records form a SHA-256 chain;
//! an incomplete line, corrupt JSON, broken chain, invalid transition, or
//! capacity breach fails closed. There is no in-memory fallback or repair.

use fs4::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

pub const WORKFLOW_ACTION_REPLAY_WAL_RELATIVE_PATH: &str = "wal/workflow-action-replay.jsonl";
pub const WORKFLOW_ACTION_REPLAY_LOCK_RELATIVE_PATH: &str = "locks/workflow-action-replay.lock";
pub const WORKFLOW_ACTION_REPLAY_MANIFEST_RELATIVE_PATH: &str =
    "workflow-action-replay.manifest.json";
pub const WORKFLOW_ACTION_REPLAY_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_ACTION_REPLAY_MAX_BYTES: u64 = 8 * 1024 * 1024;
pub const WORKFLOW_ACTION_REPLAY_MAX_RECORDS: usize = 20_000;

const MANIFEST_MAX_BYTES: u64 = 4 * 1024;
const ORIGIN_HASH_DOMAIN: &[u8] = b"forge-method:workflow-action-origin:v1\0";
const KEY_HASH_DOMAIN: &[u8] = b"forge-method:workflow-action-replay-key:v1\0";
const GENESIS_RECORD_DIGEST: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowActionReplayManifest {
    schema_version: String,
    format: String,
    wal_relative_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorkflowActionReplayOperation {
    Reserve,
    Commit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorkflowActionReplayState {
    Reserved,
    Committed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnsignedRecord {
    schema_version: String,
    sequence: u64,
    operation: WorkflowActionReplayOperation,
    key_hash: String,
    action_packet_digest: String,
    origin_event_id_hash: String,
    planned_ledger_record_digest: String,
    prior_record_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRecord {
    #[serde(flatten)]
    unsigned: UnsignedRecord,
    record_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct WorkflowActionReplayEntry {
    pub key_hash: String,
    pub action_packet_digest: String,
    pub origin_event_id_hash: String,
    pub planned_ledger_record_digest: String,
    pub state: WorkflowActionReplayState,
    pub reserved_sequence: u64,
    pub committed_sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct WorkflowActionReplayRecovery {
    pub wal_path: PathBuf,
    pub entries: BTreeMap<String, WorkflowActionReplayEntry>,
    pub valid_record_count: usize,
    pub last_sequence: u64,
    pub last_record_digest: String,
    pub wal_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct WorkflowActionReplayMutation {
    pub wal_path: PathBuf,
    pub appended: bool,
    pub sequence: u64,
    pub bytes_appended: u64,
    pub entry: WorkflowActionReplayEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct WorkflowActionReplayInitialization {
    pub state_root: PathBuf,
    pub wal_path: PathBuf,
    pub manifest_path: PathBuf,
    pub initialized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WorkflowActionReplayCapacityKind {
    Bytes,
    Records,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkflowActionReplayError {
    InvalidInput {
        field: &'static str,
        reason: &'static str,
    },
    StateRootUnavailable {
        path: PathBuf,
        source: String,
    },
    CreateDirectory {
        path: PathBuf,
        source: String,
    },
    Lock {
        path: PathBuf,
        source: String,
    },
    NotInitialized {
        state_root: PathBuf,
    },
    InitializationMismatch {
        manifest_path: PathBuf,
        manifest_exists: bool,
        wal_path: PathBuf,
        wal_exists: bool,
    },
    InvalidManifest {
        path: PathBuf,
        source: String,
    },
    InvalidAuthorityFile {
        path: PathBuf,
        source: String,
    },
    ReadWal {
        path: PathBuf,
        source: String,
    },
    CorruptWal {
        path: PathBuf,
        line: usize,
        reason: String,
    },
    Serialize {
        source: String,
    },
    WriteWal {
        path: PathBuf,
        source: String,
    },
    CapacityExceeded {
        kind: WorkflowActionReplayCapacityKind,
        limit: u64,
        observed: u64,
    },
    SequenceOverflow {
        last_sequence: u64,
    },
    BindingMismatch {
        key_hash: String,
        field: &'static str,
    },
    PacketReplayConflict {
        action_packet_digest: String,
    },
    OriginReplayConflict {
        origin_event_id_hash: String,
    },
    ReservationMissing {
        key_hash: String,
    },
}

impl fmt::Display for WorkflowActionReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { field, reason } => write!(formatter, "invalid {field}: {reason}"),
            Self::StateRootUnavailable { path, source } => {
                write!(formatter, "state root {} unavailable: {source}", path.display())
            }
            Self::CreateDirectory { path, source } => {
                write!(formatter, "create directory {} failed: {source}", path.display())
            }
            Self::Lock { path, source } => {
                write!(formatter, "lock {} failed: {source}", path.display())
            }
            Self::NotInitialized { state_root } => write!(
                formatter,
                "workflow action replay store is not initialized at {}",
                state_root.display()
            ),
            Self::InitializationMismatch {
                manifest_path,
                manifest_exists,
                wal_path,
                wal_exists,
            } => write!(
                formatter,
                "workflow action replay initialization mismatch: {} exists={manifest_exists}, {} exists={wal_exists}",
                manifest_path.display(),
                wal_path.display()
            ),
            Self::InvalidManifest { path, source } => {
                write!(formatter, "manifest {} invalid: {source}", path.display())
            }
            Self::InvalidAuthorityFile { path, source } => {
                write!(formatter, "authority file {} invalid: {source}", path.display())
            }
            Self::ReadWal { path, source } => {
                write!(formatter, "read WAL {} failed: {source}", path.display())
            }
            Self::CorruptWal { path, line, reason } => write!(
                formatter,
                "WAL {} is corrupt at line {line}: {reason}",
                path.display()
            ),
            Self::Serialize { source } => write!(formatter, "serialize WAL record failed: {source}"),
            Self::WriteWal { path, source } => {
                write!(formatter, "write WAL {} failed: {source}", path.display())
            }
            Self::CapacityExceeded { kind, limit, observed } => write!(
                formatter,
                "workflow action replay {kind:?} capacity exceeded: limit={limit}, observed={observed}"
            ),
            Self::SequenceOverflow { last_sequence } => {
                write!(formatter, "WAL sequence overflow after {last_sequence}")
            }
            Self::BindingMismatch { key_hash, field } => {
                write!(formatter, "replay binding {field} mismatch for {key_hash}")
            }
            Self::PacketReplayConflict { action_packet_digest } => write!(
                formatter,
                "action packet {action_packet_digest} is already bound to a different origin"
            ),
            Self::OriginReplayConflict { origin_event_id_hash } => write!(
                formatter,
                "origin {origin_event_id_hash} is already bound to a different action packet"
            ),
            Self::ReservationMissing { key_hash } => {
                write!(formatter, "replay reservation {key_hash} does not exist")
            }
        }
    }
}

impl std::error::Error for WorkflowActionReplayError {}

/// Initialize the on-disk manifest/WAL pair under the store's exclusive lock.
///
/// # Errors
///
/// Fails if `state_root` is not an existing trusted directory, authority files
/// are inconsistent, or durable creation cannot complete.
pub fn initialize_workflow_action_replay(
    state_root: impl AsRef<Path>,
) -> Result<WorkflowActionReplayInitialization, WorkflowActionReplayError> {
    let state_root = trusted_state_root(state_root.as_ref())?;
    let _lock = acquire_lock(&state_root)?;
    let initialized = ensure_initialized_under_lock(&state_root, true)?;
    Ok(WorkflowActionReplayInitialization {
        wal_path: wal_path(&state_root),
        manifest_path: manifest_path(&state_root),
        state_root,
        initialized,
    })
}

/// Fully verify the durable replay WAL and return its authoritative projection.
///
/// # Errors
///
/// Fails closed for missing initialization, truncation, corruption, invalid
/// transitions, or either capacity limit. No repair is attempted.
pub fn recover_workflow_action_replay(
    state_root: impl AsRef<Path>,
) -> Result<WorkflowActionReplayRecovery, WorkflowActionReplayError> {
    let state_root = trusted_state_root(state_root.as_ref())?;
    let _lock = acquire_lock(&state_root)?;
    ensure_initialized_under_lock(&state_root, false)?;
    recover_under_lock(&wal_path(&state_root))
}

/// Durably reserve one action packet/origin pair for an exact planned ledger
/// record. Exact retries return the existing reserved or committed entry
/// without appending. Rebinding either packet or origin, or changing the
/// planned ledger digest, fails closed.
///
/// # Errors
///
/// Returns [`WorkflowActionReplayError`] for invalid input, corrupt state,
/// binding conflicts, capacity exhaustion, locking, or durable append errors.
pub fn reserve_workflow_action(
    state_root: impl AsRef<Path>,
    action_packet_digest: &str,
    origin_event_id: &str,
    planned_ledger_record_digest: &str,
) -> Result<WorkflowActionReplayMutation, WorkflowActionReplayError> {
    validate_digest("action_packet_digest", action_packet_digest)?;
    validate_origin(origin_event_id)?;
    validate_digest("planned_ledger_record_digest", planned_ledger_record_digest)?;
    let origin_event_id_hash = origin_hash(origin_event_id);
    let key_hash = replay_key_hash(action_packet_digest, &origin_event_id_hash);
    let state_root = trusted_state_root(state_root.as_ref())?;
    let wal_path = wal_path(&state_root);
    let _lock = acquire_lock(&state_root)?;
    ensure_initialized_under_lock(&state_root, false)?;
    let recovery = recover_under_lock(&wal_path)?;

    if let Some(existing) = recovery.entries.get(&key_hash) {
        validate_binding(
            existing,
            action_packet_digest,
            &origin_event_id_hash,
            planned_ledger_record_digest,
        )?;
        return Ok(existing_mutation(wal_path, existing));
    }
    reject_cross_key_replay(&recovery, action_packet_digest, &origin_event_id_hash)?;

    let sequence = next_sequence(recovery.last_sequence)?;
    let unsigned = UnsignedRecord {
        schema_version: WORKFLOW_ACTION_REPLAY_SCHEMA_VERSION.to_owned(),
        sequence,
        operation: WorkflowActionReplayOperation::Reserve,
        key_hash: key_hash.clone(),
        action_packet_digest: action_packet_digest.to_owned(),
        origin_event_id_hash: origin_event_id_hash.clone(),
        planned_ledger_record_digest: planned_ledger_record_digest.to_owned(),
        prior_record_digest: recovery.last_record_digest.clone(),
    };
    let bytes = encode_line(unsigned)?;
    ensure_capacity(&recovery, bytes.len())?;
    append_and_sync(&wal_path, &bytes)?;
    Ok(WorkflowActionReplayMutation {
        wal_path,
        appended: true,
        sequence,
        bytes_appended: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        entry: WorkflowActionReplayEntry {
            key_hash,
            action_packet_digest: action_packet_digest.to_owned(),
            origin_event_id_hash,
            planned_ledger_record_digest: planned_ledger_record_digest.to_owned(),
            state: WorkflowActionReplayState::Reserved,
            reserved_sequence: sequence,
            committed_sequence: None,
        },
    })
}

/// Durably mark an exact reservation committed. An exact committed retry
/// returns the existing entry without appending.
///
/// # Errors
///
/// Returns [`WorkflowActionReplayError`] if the reservation is absent, any
/// binding differs, state is corrupt, or the durable commit cannot complete.
pub fn commit_workflow_action(
    state_root: impl AsRef<Path>,
    action_packet_digest: &str,
    origin_event_id: &str,
    planned_ledger_record_digest: &str,
) -> Result<WorkflowActionReplayMutation, WorkflowActionReplayError> {
    validate_digest("action_packet_digest", action_packet_digest)?;
    validate_origin(origin_event_id)?;
    validate_digest("planned_ledger_record_digest", planned_ledger_record_digest)?;
    let origin_event_id_hash = origin_hash(origin_event_id);
    let key_hash = replay_key_hash(action_packet_digest, &origin_event_id_hash);
    let state_root = trusted_state_root(state_root.as_ref())?;
    let wal_path = wal_path(&state_root);
    let _lock = acquire_lock(&state_root)?;
    ensure_initialized_under_lock(&state_root, false)?;
    let recovery = recover_under_lock(&wal_path)?;
    let existing = recovery.entries.get(&key_hash).ok_or_else(|| {
        WorkflowActionReplayError::ReservationMissing {
            key_hash: key_hash.clone(),
        }
    })?;
    validate_binding(
        existing,
        action_packet_digest,
        &origin_event_id_hash,
        planned_ledger_record_digest,
    )?;
    if existing.state == WorkflowActionReplayState::Committed {
        return Ok(existing_mutation(wal_path, existing));
    }

    let sequence = next_sequence(recovery.last_sequence)?;
    let unsigned = UnsignedRecord {
        schema_version: WORKFLOW_ACTION_REPLAY_SCHEMA_VERSION.to_owned(),
        sequence,
        operation: WorkflowActionReplayOperation::Commit,
        key_hash: key_hash.clone(),
        action_packet_digest: action_packet_digest.to_owned(),
        origin_event_id_hash: origin_event_id_hash.clone(),
        planned_ledger_record_digest: planned_ledger_record_digest.to_owned(),
        prior_record_digest: recovery.last_record_digest.clone(),
    };
    let bytes = encode_line(unsigned)?;
    ensure_capacity(&recovery, bytes.len())?;
    append_and_sync(&wal_path, &bytes)?;
    let mut entry = existing.clone();
    entry.state = WorkflowActionReplayState::Committed;
    entry.committed_sequence = Some(sequence);
    Ok(WorkflowActionReplayMutation {
        wal_path,
        appended: true,
        sequence,
        bytes_appended: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        entry,
    })
}

fn existing_mutation(
    wal_path: PathBuf,
    entry: &WorkflowActionReplayEntry,
) -> WorkflowActionReplayMutation {
    WorkflowActionReplayMutation {
        wal_path,
        appended: false,
        sequence: entry.committed_sequence.unwrap_or(entry.reserved_sequence),
        bytes_appended: 0,
        entry: entry.clone(),
    }
}

fn validate_binding(
    entry: &WorkflowActionReplayEntry,
    packet_digest: &str,
    origin_hash: &str,
    ledger_digest: &str,
) -> Result<(), WorkflowActionReplayError> {
    for (matches, field) in [
        (
            entry.action_packet_digest == packet_digest,
            "action_packet_digest",
        ),
        (entry.origin_event_id_hash == origin_hash, "origin_event_id"),
        (
            entry.planned_ledger_record_digest == ledger_digest,
            "planned_ledger_record_digest",
        ),
    ] {
        if !matches {
            return Err(WorkflowActionReplayError::BindingMismatch {
                key_hash: entry.key_hash.clone(),
                field,
            });
        }
    }
    Ok(())
}

fn reject_cross_key_replay(
    recovery: &WorkflowActionReplayRecovery,
    packet_digest: &str,
    origin_hash: &str,
) -> Result<(), WorkflowActionReplayError> {
    if recovery
        .entries
        .values()
        .any(|entry| entry.action_packet_digest == packet_digest)
    {
        return Err(WorkflowActionReplayError::PacketReplayConflict {
            action_packet_digest: packet_digest.to_owned(),
        });
    }
    if recovery
        .entries
        .values()
        .any(|entry| entry.origin_event_id_hash == origin_hash)
    {
        return Err(WorkflowActionReplayError::OriginReplayConflict {
            origin_event_id_hash: origin_hash.to_owned(),
        });
    }
    Ok(())
}

fn recover_under_lock(
    wal_path: &Path,
) -> Result<WorkflowActionReplayRecovery, WorkflowActionReplayError> {
    ensure_regular_authority_file(wal_path)?;
    let bytes = read_bounded(wal_path, WORKFLOW_ACTION_REPLAY_MAX_BYTES).map_err(|source| {
        if source.kind() == io::ErrorKind::FileTooLarge {
            let observed = fs::metadata(wal_path).map_or(
                WORKFLOW_ACTION_REPLAY_MAX_BYTES.saturating_add(1),
                |metadata| metadata.len(),
            );
            WorkflowActionReplayError::CapacityExceeded {
                kind: WorkflowActionReplayCapacityKind::Bytes,
                limit: WORKFLOW_ACTION_REPLAY_MAX_BYTES,
                observed,
            }
        } else {
            WorkflowActionReplayError::ReadWal {
                path: wal_path.to_path_buf(),
                source: source.to_string(),
            }
        }
    })?;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return Err(corrupt(
            wal_path,
            bytes.split(|byte| *byte == b'\n').count(),
            "truncated final record",
        ));
    }

    let mut entries = BTreeMap::<String, WorkflowActionReplayEntry>::new();
    let mut packet_keys = BTreeMap::<String, String>::new();
    let mut origin_keys = BTreeMap::<String, String>::new();
    let mut last_sequence = 0_u64;
    let mut last_record_digest = GENESIS_RECORD_DIGEST.to_owned();
    let mut valid_record_count = 0_usize;
    let body = bytes.strip_suffix(b"\n").unwrap_or(&bytes);
    for (index, line) in body.split(|byte| *byte == b'\n').enumerate() {
        if bytes.is_empty() {
            break;
        }
        if line.is_empty() {
            return Err(corrupt(wal_path, index + 1, "blank record"));
        }
        if valid_record_count >= WORKFLOW_ACTION_REPLAY_MAX_RECORDS {
            return Err(WorkflowActionReplayError::CapacityExceeded {
                kind: WorkflowActionReplayCapacityKind::Records,
                limit: u64::try_from(WORKFLOW_ACTION_REPLAY_MAX_RECORDS).unwrap_or(u64::MAX),
                observed: u64::try_from(valid_record_count + 1).unwrap_or(u64::MAX),
            });
        }
        let record: StoredRecord = serde_json::from_slice(line).map_err(|error| {
            corrupt(wal_path, index + 1, &format!("JSON decode failed: {error}"))
        })?;
        validate_stored_record(
            wal_path,
            index + 1,
            &record,
            last_sequence,
            &last_record_digest,
        )?;
        apply_record(
            wal_path,
            index + 1,
            &record,
            &mut entries,
            &mut packet_keys,
            &mut origin_keys,
        )?;
        last_sequence = record.unsigned.sequence;
        last_record_digest = record.record_digest;
        valid_record_count += 1;
    }
    Ok(WorkflowActionReplayRecovery {
        wal_path: wal_path.to_path_buf(),
        entries,
        valid_record_count,
        last_sequence,
        last_record_digest,
        wal_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    })
}

fn validate_stored_record(
    path: &Path,
    line: usize,
    record: &StoredRecord,
    last_sequence: u64,
    last_digest: &str,
) -> Result<(), WorkflowActionReplayError> {
    if record.unsigned.schema_version != WORKFLOW_ACTION_REPLAY_SCHEMA_VERSION {
        return Err(corrupt(path, line, "unsupported schema version"));
    }
    let expected_sequence =
        next_sequence(last_sequence).map_err(|error| corrupt(path, line, &error.to_string()))?;
    if record.unsigned.sequence != expected_sequence {
        return Err(corrupt(path, line, "sequence gap"));
    }
    if record.unsigned.prior_record_digest != last_digest {
        return Err(corrupt(path, line, "record chain mismatch"));
    }
    for (field, digest) in [
        ("key_hash", record.unsigned.key_hash.as_str()),
        (
            "action_packet_digest",
            record.unsigned.action_packet_digest.as_str(),
        ),
        (
            "origin_event_id_hash",
            record.unsigned.origin_event_id_hash.as_str(),
        ),
        (
            "planned_ledger_record_digest",
            record.unsigned.planned_ledger_record_digest.as_str(),
        ),
        (
            "prior_record_digest",
            record.unsigned.prior_record_digest.as_str(),
        ),
        ("record_digest", record.record_digest.as_str()),
    ] {
        if !is_digest(digest) {
            return Err(corrupt(path, line, &format!("invalid {field}")));
        }
    }
    let expected_digest = digest_canonical(&record.unsigned)
        .map_err(|error| corrupt(path, line, &error.to_string()))?;
    if record.record_digest != expected_digest {
        return Err(corrupt(path, line, "record digest mismatch"));
    }
    let expected_key = replay_key_hash(
        &record.unsigned.action_packet_digest,
        &record.unsigned.origin_event_id_hash,
    );
    if record.unsigned.key_hash != expected_key {
        return Err(corrupt(path, line, "derived key mismatch"));
    }
    Ok(())
}

fn apply_record(
    path: &Path,
    line: usize,
    record: &StoredRecord,
    entries: &mut BTreeMap<String, WorkflowActionReplayEntry>,
    packet_keys: &mut BTreeMap<String, String>,
    origin_keys: &mut BTreeMap<String, String>,
) -> Result<(), WorkflowActionReplayError> {
    let unsigned = &record.unsigned;
    match unsigned.operation {
        WorkflowActionReplayOperation::Reserve => {
            if entries.contains_key(&unsigned.key_hash) {
                return Err(corrupt(path, line, "duplicate reserve"));
            }
            if packet_keys.contains_key(&unsigned.action_packet_digest) {
                return Err(corrupt(path, line, "packet rebound to another key"));
            }
            if origin_keys.contains_key(&unsigned.origin_event_id_hash) {
                return Err(corrupt(path, line, "origin rebound to another key"));
            }
            packet_keys.insert(
                unsigned.action_packet_digest.clone(),
                unsigned.key_hash.clone(),
            );
            origin_keys.insert(
                unsigned.origin_event_id_hash.clone(),
                unsigned.key_hash.clone(),
            );
            entries.insert(
                unsigned.key_hash.clone(),
                WorkflowActionReplayEntry {
                    key_hash: unsigned.key_hash.clone(),
                    action_packet_digest: unsigned.action_packet_digest.clone(),
                    origin_event_id_hash: unsigned.origin_event_id_hash.clone(),
                    planned_ledger_record_digest: unsigned.planned_ledger_record_digest.clone(),
                    state: WorkflowActionReplayState::Reserved,
                    reserved_sequence: unsigned.sequence,
                    committed_sequence: None,
                },
            );
        }
        WorkflowActionReplayOperation::Commit => {
            let entry = entries
                .get_mut(&unsigned.key_hash)
                .ok_or_else(|| corrupt(path, line, "commit without reserve"))?;
            validate_binding(
                entry,
                &unsigned.action_packet_digest,
                &unsigned.origin_event_id_hash,
                &unsigned.planned_ledger_record_digest,
            )
            .map_err(|error| corrupt(path, line, &error.to_string()))?;
            if entry.state == WorkflowActionReplayState::Committed {
                return Err(corrupt(path, line, "duplicate commit"));
            }
            entry.state = WorkflowActionReplayState::Committed;
            entry.committed_sequence = Some(unsigned.sequence);
        }
    }
    Ok(())
}

fn encode_line(unsigned: UnsignedRecord) -> Result<Vec<u8>, WorkflowActionReplayError> {
    let record_digest = digest_canonical(&unsigned)?;
    let mut bytes = serde_json_canonicalizer::to_vec(&StoredRecord {
        unsigned,
        record_digest,
    })
    .map_err(|source| WorkflowActionReplayError::Serialize {
        source: source.to_string(),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn digest_canonical<T: Serialize>(value: &T) -> Result<String, WorkflowActionReplayError> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|source| {
        WorkflowActionReplayError::Serialize {
            source: source.to_string(),
        }
    })?;
    Ok(format_digest(Sha256::digest(bytes)))
}

fn ensure_capacity(
    recovery: &WorkflowActionReplayRecovery,
    append_len: usize,
) -> Result<(), WorkflowActionReplayError> {
    let records = recovery.valid_record_count.saturating_add(1);
    if records > WORKFLOW_ACTION_REPLAY_MAX_RECORDS {
        return Err(WorkflowActionReplayError::CapacityExceeded {
            kind: WorkflowActionReplayCapacityKind::Records,
            limit: u64::try_from(WORKFLOW_ACTION_REPLAY_MAX_RECORDS).unwrap_or(u64::MAX),
            observed: u64::try_from(records).unwrap_or(u64::MAX),
        });
    }
    let bytes = recovery
        .wal_bytes
        .saturating_add(u64::try_from(append_len).unwrap_or(u64::MAX));
    if bytes > WORKFLOW_ACTION_REPLAY_MAX_BYTES {
        return Err(WorkflowActionReplayError::CapacityExceeded {
            kind: WorkflowActionReplayCapacityKind::Bytes,
            limit: WORKFLOW_ACTION_REPLAY_MAX_BYTES,
            observed: bytes,
        });
    }
    Ok(())
}

fn append_and_sync(path: &Path, bytes: &[u8]) -> Result<(), WorkflowActionReplayError> {
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|source| WorkflowActionReplayError::WriteWal {
            path: path.to_path_buf(),
            source: source.to_string(),
        })?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|source| WorkflowActionReplayError::WriteWal {
            path: path.to_path_buf(),
            source: source.to_string(),
        })
}

fn ensure_initialized_under_lock(
    state_root: &Path,
    create_if_absent: bool,
) -> Result<bool, WorkflowActionReplayError> {
    let wal_path = wal_path(state_root);
    let manifest_path = manifest_path(state_root);
    let wal_exists = try_exists(&wal_path)?;
    let manifest_exists = try_exists(&manifest_path)?;
    match (manifest_exists, wal_exists) {
        (false, false) if create_if_absent => {
            initialize_files(state_root, &wal_path, &manifest_path)?;
            Ok(true)
        }
        (false, false) => Err(WorkflowActionReplayError::NotInitialized {
            state_root: state_root.to_path_buf(),
        }),
        (true, true) => {
            validate_manifest(&manifest_path)?;
            ensure_regular_authority_file(&wal_path)?;
            Ok(false)
        }
        _ => Err(WorkflowActionReplayError::InitializationMismatch {
            manifest_path,
            manifest_exists,
            wal_path,
            wal_exists,
        }),
    }
}

fn initialize_files(
    state_root: &Path,
    wal_path: &Path,
    manifest_path: &Path,
) -> Result<(), WorkflowActionReplayError> {
    let wal_parent =
        wal_path
            .parent()
            .ok_or_else(|| WorkflowActionReplayError::CreateDirectory {
                path: wal_path.to_path_buf(),
                source: "WAL path has no parent".to_owned(),
            })?;
    fs::create_dir_all(wal_parent).map_err(|source| {
        WorkflowActionReplayError::CreateDirectory {
            path: wal_parent.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    ensure_directory_within_root(state_root, wal_parent)?;
    let wal = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(wal_path)
        .map_err(|source| WorkflowActionReplayError::WriteWal {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        })?;
    wal.sync_all()
        .and_then(|()| sync_directory(wal_parent))
        .map_err(|source| WorkflowActionReplayError::WriteWal {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        })?;

    let bytes = serde_json_canonicalizer::to_vec(&expected_manifest()).map_err(|source| {
        WorkflowActionReplayError::Serialize {
            source: source.to_string(),
        }
    })?;
    let mut manifest = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(manifest_path)
        .map_err(|source| WorkflowActionReplayError::WriteWal {
            path: manifest_path.to_path_buf(),
            source: source.to_string(),
        })?;
    manifest
        .write_all(&bytes)
        .and_then(|()| manifest.flush())
        .and_then(|()| manifest.sync_all())
        .and_then(|()| sync_directory(state_root))
        .map_err(|source| WorkflowActionReplayError::WriteWal {
            path: manifest_path.to_path_buf(),
            source: source.to_string(),
        })
}

fn expected_manifest() -> WorkflowActionReplayManifest {
    WorkflowActionReplayManifest {
        schema_version: WORKFLOW_ACTION_REPLAY_SCHEMA_VERSION.to_owned(),
        format: "sha256-chained-jsonl-v1".to_owned(),
        wal_relative_path: WORKFLOW_ACTION_REPLAY_WAL_RELATIVE_PATH.to_owned(),
    }
}

fn validate_manifest(path: &Path) -> Result<(), WorkflowActionReplayError> {
    ensure_regular_authority_file(path)?;
    let bytes = read_bounded(path, MANIFEST_MAX_BYTES).map_err(|source| {
        WorkflowActionReplayError::InvalidManifest {
            path: path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    let actual =
        serde_json::from_slice::<WorkflowActionReplayManifest>(&bytes).map_err(|source| {
            WorkflowActionReplayError::InvalidManifest {
                path: path.to_path_buf(),
                source: source.to_string(),
            }
        })?;
    if actual != expected_manifest() {
        return Err(WorkflowActionReplayError::InvalidManifest {
            path: path.to_path_buf(),
            source: "unsupported manifest contents".to_owned(),
        });
    }
    Ok(())
}

fn trusted_state_root(path: &Path) -> Result<PathBuf, WorkflowActionReplayError> {
    let metadata =
        fs::metadata(path).map_err(|source| WorkflowActionReplayError::StateRootUnavailable {
            path: path.to_path_buf(),
            source: source.to_string(),
        })?;
    if !metadata.is_dir() {
        return Err(WorkflowActionReplayError::StateRootUnavailable {
            path: path.to_path_buf(),
            source: "path is not a directory".to_owned(),
        });
    }
    fs::canonicalize(path).map_err(|source| WorkflowActionReplayError::StateRootUnavailable {
        path: path.to_path_buf(),
        source: source.to_string(),
    })
}

fn ensure_directory_within_root(
    state_root: &Path,
    path: &Path,
) -> Result<(), WorkflowActionReplayError> {
    let canonical = fs::canonicalize(path).map_err(|source| {
        WorkflowActionReplayError::StateRootUnavailable {
            path: path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    if !canonical.starts_with(state_root) {
        return Err(WorkflowActionReplayError::StateRootUnavailable {
            path: path.to_path_buf(),
            source: "path resolves outside trusted state root".to_owned(),
        });
    }
    Ok(())
}

fn ensure_regular_authority_file(path: &Path) -> Result<(), WorkflowActionReplayError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| {
        WorkflowActionReplayError::InvalidAuthorityFile {
            path: path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(WorkflowActionReplayError::InvalidAuthorityFile {
            path: path.to_path_buf(),
            source: "must be a regular non-symlink file".to_owned(),
        });
    }
    Ok(())
}

fn acquire_lock(state_root: &Path) -> Result<WorkflowActionReplayLock, WorkflowActionReplayError> {
    let path = lock_path(state_root);
    let parent = path
        .parent()
        .ok_or_else(|| WorkflowActionReplayError::CreateDirectory {
            path: path.clone(),
            source: "lock path has no parent".to_owned(),
        })?;
    fs::create_dir_all(parent).map_err(|source| WorkflowActionReplayError::CreateDirectory {
        path: parent.to_path_buf(),
        source: source.to_string(),
    })?;
    ensure_directory_within_root(state_root, parent)?;
    if path.exists() {
        ensure_regular_authority_file(&path)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|source| WorkflowActionReplayError::Lock {
            path: path.clone(),
            source: source.to_string(),
        })?;
    FileExt::lock(&file).map_err(|source| WorkflowActionReplayError::Lock {
        path,
        source: source.to_string(),
    })?;
    Ok(WorkflowActionReplayLock { file })
}

struct WorkflowActionReplayLock {
    file: File,
}

impl Drop for WorkflowActionReplayLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn validate_digest(field: &'static str, value: &str) -> Result<(), WorkflowActionReplayError> {
    if !is_digest(value) {
        return Err(WorkflowActionReplayError::InvalidInput {
            field,
            reason: "must be a lowercase sha256:<64-hex> token",
        });
    }
    Ok(())
}

fn validate_origin(value: &str) -> Result<(), WorkflowActionReplayError> {
    if value.trim().is_empty() {
        return Err(WorkflowActionReplayError::InvalidInput {
            field: "origin_event_id",
            reason: "must not be blank",
        });
    }
    if value.len() > 1024 {
        return Err(WorkflowActionReplayError::InvalidInput {
            field: "origin_event_id",
            reason: "must not exceed 1024 bytes",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(WorkflowActionReplayError::InvalidInput {
            field: "origin_event_id",
            reason: "must not contain control characters",
        });
    }
    Ok(())
}

fn is_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn origin_hash(origin_event_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(ORIGIN_HASH_DOMAIN);
    update_length_prefixed(&mut hasher, origin_event_id.as_bytes());
    format_digest(hasher.finalize())
}

fn replay_key_hash(packet_digest: &str, origin_event_id_hash: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(KEY_HASH_DOMAIN);
    update_length_prefixed(&mut hasher, packet_digest.as_bytes());
    update_length_prefixed(&mut hasher, origin_event_id_hash.as_bytes());
    format_digest(hasher.finalize())
}

fn update_length_prefixed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(value);
}

fn format_digest(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(hex, "{byte:02x}");
    }
    format!("sha256:{hex}")
}

fn next_sequence(last_sequence: u64) -> Result<u64, WorkflowActionReplayError> {
    last_sequence
        .checked_add(1)
        .ok_or(WorkflowActionReplayError::SequenceOverflow { last_sequence })
}

fn read_bounded(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let metadata_len = file.metadata()?.len();
    if metadata_len > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "file too large",
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata_len).unwrap_or(usize::MAX));
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "file too large",
        ));
    }
    Ok(bytes)
}

fn try_exists(path: &Path) -> Result<bool, WorkflowActionReplayError> {
    path.try_exists()
        .map_err(|source| WorkflowActionReplayError::StateRootUnavailable {
            path: path.to_path_buf(),
            source: source.to_string(),
        })
}

fn corrupt(path: &Path, line: usize, reason: &str) -> WorkflowActionReplayError {
    WorkflowActionReplayError::CorruptWal {
        path: path.to_path_buf(),
        line,
        reason: reason.to_owned(),
    }
}

fn wal_path(state_root: &Path) -> PathBuf {
    state_root.join(WORKFLOW_ACTION_REPLAY_WAL_RELATIVE_PATH)
}

fn lock_path(state_root: &Path) -> PathBuf {
    state_root.join(WORKFLOW_ACTION_REPLAY_LOCK_RELATIVE_PATH)
}

fn manifest_path(state_root: &Path) -> PathBuf {
    state_root.join(WORKFLOW_ACTION_REPLAY_MANIFEST_RELATIVE_PATH)
}

#[cfg(not(windows))]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(windows)]
fn sync_directory(path: &Path) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt as _;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?
        .sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("forge-action-replay-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create test root");
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn digest(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    #[test]
    fn exact_reserve_and_commit_retries_are_idempotent() {
        let root = TestRoot::new();
        initialize_workflow_action_replay(&root.0).expect("initialize");
        let packet = digest('a');
        let ledger = digest('b');

        let first =
            reserve_workflow_action(&root.0, &packet, "host:event:1", &ledger).expect("reserve");
        assert!(first.appended);
        assert_eq!(first.entry.state, WorkflowActionReplayState::Reserved);
        let retry = reserve_workflow_action(&root.0, &packet, "host:event:1", &ledger)
            .expect("reserve retry");
        assert!(!retry.appended);
        assert_eq!(retry.entry, first.entry);

        let committed =
            commit_workflow_action(&root.0, &packet, "host:event:1", &ledger).expect("commit");
        assert!(committed.appended);
        assert_eq!(committed.entry.state, WorkflowActionReplayState::Committed);
        let retry = commit_workflow_action(&root.0, &packet, "host:event:1", &ledger)
            .expect("commit retry");
        assert!(!retry.appended);
        assert_eq!(retry.entry, committed.entry);

        let reserve_after_commit =
            reserve_workflow_action(&root.0, &packet, "host:event:1", &ledger)
                .expect("reserve retry after commit");
        assert!(!reserve_after_commit.appended);
        assert_eq!(reserve_after_commit.entry, committed.entry);
        let recovery = recover_workflow_action_replay(&root.0).expect("recover");
        assert_eq!(recovery.valid_record_count, 2);
    }

    #[test]
    fn mismatched_binding_and_cross_key_replays_are_rejected() {
        let root = TestRoot::new();
        initialize_workflow_action_replay(&root.0).expect("initialize");
        let packet = digest('a');
        let ledger = digest('b');
        reserve_workflow_action(&root.0, &packet, "host:event:1", &ledger).expect("reserve");

        assert!(matches!(
            reserve_workflow_action(&root.0, &packet, "host:event:1", &digest('c')),
            Err(WorkflowActionReplayError::BindingMismatch {
                field: "planned_ledger_record_digest",
                ..
            })
        ));
        assert!(matches!(
            reserve_workflow_action(&root.0, &packet, "host:event:2", &ledger),
            Err(WorkflowActionReplayError::PacketReplayConflict { .. })
        ));
        assert!(matches!(
            reserve_workflow_action(&root.0, &digest('c'), "host:event:1", &ledger),
            Err(WorkflowActionReplayError::OriginReplayConflict { .. })
        ));
        assert!(matches!(
            commit_workflow_action(&root.0, &digest('d'), "host:event:missing", &ledger),
            Err(WorkflowActionReplayError::ReservationMissing { .. })
        ));
    }

    #[test]
    fn truncation_and_corruption_fail_closed_without_repair() {
        let root = TestRoot::new();
        initialize_workflow_action_replay(&root.0).expect("initialize");
        reserve_workflow_action(&root.0, &digest('a'), "host:event:1", &digest('b'))
            .expect("reserve");
        let path = wal_path(&root.0);
        let mut bytes = fs::read(&path).expect("read WAL");
        bytes.pop();
        fs::write(&path, &bytes).expect("truncate WAL");

        assert!(matches!(
            recover_workflow_action_replay(&root.0),
            Err(WorkflowActionReplayError::CorruptWal { .. })
        ));
        assert!(matches!(
            reserve_workflow_action(&root.0, &digest('c'), "host:event:2", &digest('d')),
            Err(WorkflowActionReplayError::CorruptWal { .. })
        ));

        bytes.push(b'\n');
        bytes[20] ^= 1;
        fs::write(&path, &bytes).expect("corrupt WAL");
        assert!(matches!(
            recover_workflow_action_replay(&root.0),
            Err(WorkflowActionReplayError::CorruptWal { .. })
        ));
    }

    #[test]
    fn missing_or_partial_initialization_fails_closed() {
        let root = TestRoot::new();
        assert!(matches!(
            reserve_workflow_action(&root.0, &digest('a'), "host:event:1", &digest('b')),
            Err(WorkflowActionReplayError::NotInitialized { .. })
        ));
        initialize_workflow_action_replay(&root.0).expect("initialize");
        fs::remove_file(manifest_path(&root.0)).expect("remove manifest");
        assert!(matches!(
            recover_workflow_action_replay(&root.0),
            Err(WorkflowActionReplayError::InitializationMismatch { .. })
        ));
    }
}
