use forge_core_contracts::claim::{ClaimContract, ClaimStatus};
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const MAGIC: [u8; 4] = *b"FMW1";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 24;
const HEADER_CRC_OFFSET: usize = 20;
const TRAILER_LEN: usize = 4;
const FLAG_SKIPPABLE_UNKNOWN: u16 = 0b0000_0001;
const FLAG_PAYLOAD_JSON: u16 = 0b0000_0100;
const ALLOWED_FLAGS: u16 = FLAG_SKIPPABLE_UNKNOWN | FLAG_PAYLOAD_JSON;
const DEFAULT_MAX_PAYLOAD_LEN: u32 = 16 * 1024 * 1024;
const RECORD_TYPE_CHECKPOINT_REF: u8 = 4;
const DEFAULT_ROTATE_MAX_WAL_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_ROTATE_MAX_RECORDS: usize = 100_000;
const DEFAULT_ROTATE_MAX_REPLAY_MILLIS: u64 = 250;

pub const CLAIM_WAL_RELATIVE_PATH: &str = "wal/claims.fmw1";
pub const CLAIM_WAL_LOCK_RELATIVE_PATH: &str = "locks/claims.wal.lock";
pub const CLAIM_WAL_MANIFEST_RELATIVE_PATH: &str = "wal/claims.wal.manifest.json";
pub const CLAIM_WAL_SNAPSHOT_RELATIVE_DIR: &str = "wal/snapshots";
pub const CLAIM_WAL_ARCHIVE_RELATIVE_DIR: &str = "wal/archive";

type ClaimIdIndex = BTreeMap<String, Vec<String>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimWalOperation {
    Acquire,
    Release,
    Heartbeat,
    HandoffRecorded,
    /// Materialized by the periodic reconciler. Uses record type 7 so it does
    /// not collide with the original research reservation for 4/5/6
    /// (checkpoint/tombstone/rotate); older binaries will fail closed on it.
    ReconcileStatus,
}

impl ClaimWalOperation {
    #[must_use]
    pub fn record_type(self) -> u8 {
        match self {
            Self::Acquire => 1,
            Self::Release => 2,
            Self::Heartbeat => 3,
            Self::HandoffRecorded => 5,
            Self::ReconcileStatus => 7,
        }
    }

    fn from_record_type(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Acquire),
            2 => Some(Self::Release),
            3 => Some(Self::Heartbeat),
            5 => Some(Self::HandoffRecorded),
            7 => Some(Self::ReconcileStatus),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalPayload {
    pub schema_version: String,
    pub operation: ClaimWalOperation,
    pub recorded_at: String,
    pub claim_contract: ClaimContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalCheckpointPayload {
    pub schema_version: String,
    pub snapshot_path: String,
    pub snapshot_crc32c: u32,
    pub last_seq_in_snapshot: u64,
    pub created_at: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalSnapshotPayload {
    pub schema_version: String,
    pub created_at: String,
    pub created_at_ms: u64,
    pub last_seq: u64,
    pub latest_claims: Vec<ClaimWalSnapshotClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalSnapshotClaim {
    pub claim_contract: ClaimContract,
    pub last_seq: u64,
    pub last_operation: ClaimWalOperation,
    pub recorded_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalManifestPayload {
    pub schema_version: String,
    pub active_wal_path: String,
    pub snapshot_path: String,
    pub snapshot_crc32c: u32,
    pub archived_wal_path: String,
    pub checkpoint_seq: u64,
    pub last_seq_in_snapshot: u64,
    pub updated_at: String,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalAppendResult {
    pub wal_path: PathBuf,
    pub seq: u64,
    pub bytes_appended: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalRotationResult {
    pub rotated: bool,
    pub reason: Option<ClaimWalRotationReason>,
    pub wal_path: PathBuf,
    pub snapshot_path: Option<PathBuf>,
    pub archived_wal_path: Option<PathBuf>,
    pub manifest_path: Option<PathBuf>,
    pub checkpoint_seq: Option<u64>,
    pub last_seq_in_snapshot: u64,
    pub compacted_records: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimWalRotationReason {
    WalSizeBytes,
    RecordCount,
    ReplayDurationMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimWalRotationOptions {
    pub max_wal_bytes: u64,
    pub max_records: usize,
    pub max_replay_millis: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalRecovery {
    pub wal_path: PathBuf,
    pub records: Vec<ClaimWalRecord>,
    pub checkpoint: Option<ClaimWalCheckpointRecord>,
    pub last_observed_seq: u64,
    pub valid_record_count: usize,
    pub last_good_offset: u64,
    pub original_len: u64,
    pub repaired: bool,
    pub stop_reason: ClaimWalStopReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalRecord {
    pub seq: u64,
    pub operation: ClaimWalOperation,
    pub payload: ClaimWalPayload,
    pub offset: u64,
    pub record_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalCheckpointRecord {
    pub seq: u64,
    pub payload: ClaimWalCheckpointPayload,
    pub snapshot: ClaimWalSnapshotPayload,
    pub offset: u64,
    pub record_len: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimWalStopReason {
    #[default]
    CleanEof,
    TruncatedHeader,
    InvalidHeader,
    PayloadTooLarge,
    TruncatedPayload,
    PayloadChecksumMismatch,
    UnsupportedRecordType,
    SequenceGap,
    PayloadDecodeFailed,
    CheckpointNotAtStart,
    CheckpointSnapshotInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalProjection {
    pub recovery: ClaimWalRecovery,
    pub last_applied_seq: u64,
    pub applied_records: usize,
    pub claims: Vec<ClaimContract>,
    pub latest_by_claim_id: BTreeMap<String, ProjectedClaim>,
    pub active_by_claim_id: BTreeMap<String, ProjectedClaim>,
    pub released_by_claim_id: BTreeMap<String, ProjectedClaim>,
    pub handoff_recorded_by_claim_id: BTreeMap<String, ProjectedClaim>,
    pub active_claim_ids_by_agent: BTreeMap<String, Vec<String>>,
    pub active_claim_ids_by_scope: BTreeMap<String, Vec<String>>,
    pub active_claim_ids_by_path: BTreeMap<String, Vec<String>>,
    pub diagnostics: Vec<ClaimWalProjectionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedClaim {
    pub claim_contract: ClaimContract,
    pub last_seq: u64,
    pub last_operation: ClaimWalOperation,
    pub recorded_at: String,
    pub wal_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalProjectionDiagnostic {
    pub severity: ClaimWalProjectionDiagnosticSeverity,
    pub code: ClaimWalProjectionDiagnosticCode,
    pub seq: u64,
    pub claim_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimWalProjectionDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimWalProjectionDiagnosticCode {
    HeartbeatWithoutActiveClaim,
    HeartbeatAgentMismatch,
    ReleaseWithoutActiveClaim,
    ReleaseAgentMismatch,
    HandoffWithoutActiveClaim,
    HandoffAgentMismatch,
    ReconcileWithoutOpenClaim,
    ReconcileAgentMismatch,
    ReacquireByDifferentAgent,
    OperationStatusMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimWalProjectionStopPolicy {
    ProjectValidPrefix,
    RequireCleanEof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimWalProjectionOptions {
    pub repair: bool,
    pub stop_policy: ClaimWalProjectionStopPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimWalProjectionError {
    RecoverWal {
        source: ClaimWalReadError,
    },
    RecoveryStopped {
        stop_reason: ClaimWalStopReason,
        last_good_offset: u64,
        original_len: u64,
    },
}

impl Default for ClaimWalProjectionOptions {
    fn default() -> Self {
        Self {
            repair: false,
            stop_policy: ClaimWalProjectionStopPolicy::ProjectValidPrefix,
        }
    }
}

impl Default for ClaimWalRotationOptions {
    fn default() -> Self {
        Self {
            max_wal_bytes: DEFAULT_ROTATE_MAX_WAL_BYTES,
            max_records: DEFAULT_ROTATE_MAX_RECORDS,
            max_replay_millis: DEFAULT_ROTATE_MAX_REPLAY_MILLIS,
        }
    }
}

impl fmt::Display for ClaimWalProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecoverWal { source } => write!(formatter, "recover claim WAL failed: {source}"),
            Self::RecoveryStopped {
                stop_reason,
                last_good_offset,
                original_len,
            } => write!(
                formatter,
                "claim WAL recovery stopped with {stop_reason:?} at {last_good_offset}/{original_len}"
            ),
        }
    }
}

impl std::error::Error for ClaimWalProjectionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimWalAppendError {
    CreateDir {
        path: PathBuf,
        source: String,
    },
    OpenLock {
        path: PathBuf,
        source: String,
    },
    Lock {
        path: PathBuf,
        source: String,
    },
    OpenWal {
        path: PathBuf,
        source: String,
    },
    ReadWal {
        path: PathBuf,
        source: String,
    },
    RepairWal {
        path: PathBuf,
        source: String,
    },
    RecoveryStopped {
        stop_reason: ClaimWalStopReason,
        last_good_offset: u64,
        original_len: u64,
    },
    Serialize {
        source: String,
    },
    PayloadTooLarge {
        byte_len: usize,
        max_byte_len: u32,
    },
    SequenceOverflow {
        last_seq: u64,
    },
    WriteWal {
        path: PathBuf,
        source: String,
    },
    SyncWal {
        path: PathBuf,
        source: String,
    },
    RotateWal {
        path: PathBuf,
        source: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimWalReadError {
    OpenLock { path: PathBuf, source: String },
    Lock { path: PathBuf, source: String },
    ReadWal { path: PathBuf, source: String },
    RepairWal { path: PathBuf, source: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimWalRotationError {
    CreateDir {
        path: PathBuf,
        source: String,
    },
    OpenLock {
        path: PathBuf,
        source: String,
    },
    Lock {
        path: PathBuf,
        source: String,
    },
    RecoverWal {
        path: PathBuf,
        source: String,
    },
    RecoveryStopped {
        stop_reason: ClaimWalStopReason,
        last_good_offset: u64,
        original_len: u64,
    },
    SerializeSnapshot {
        source: String,
    },
    SerializeCheckpoint {
        source: String,
    },
    SerializeManifest {
        source: String,
    },
    SequenceOverflow {
        last_seq: u64,
    },
    WriteFile {
        path: PathBuf,
        source: String,
    },
    SyncFile {
        path: PathBuf,
        source: String,
    },
    RenameFile {
        from: PathBuf,
        to: PathBuf,
        source: String,
    },
    CopyFile {
        from: PathBuf,
        to: PathBuf,
        source: String,
    },
    VerifyWal {
        path: PathBuf,
        source: String,
    },
}

impl fmt::Display for ClaimWalAppendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDir { path, source } => {
                write!(
                    formatter,
                    "create WAL directory {} failed: {source}",
                    path.display()
                )
            }
            Self::OpenLock { path, source } => {
                write!(
                    formatter,
                    "open WAL lock {} failed: {source}",
                    path.display()
                )
            }
            Self::Lock { path, source } => {
                write!(formatter, "lock WAL {} failed: {source}", path.display())
            }
            Self::OpenWal { path, source } => {
                write!(formatter, "open WAL {} failed: {source}", path.display())
            }
            Self::ReadWal { path, source } => {
                write!(formatter, "read WAL {} failed: {source}", path.display())
            }
            Self::RepairWal { path, source } => {
                write!(formatter, "repair WAL {} failed: {source}", path.display())
            }
            Self::RecoveryStopped {
                stop_reason,
                last_good_offset,
                original_len,
            } => write!(
                formatter,
                "claim WAL recovery stopped with {stop_reason:?} at {last_good_offset}/{original_len}"
            ),
            Self::Serialize { source } => write!(formatter, "serialize claim WAL failed: {source}"),
            Self::PayloadTooLarge {
                byte_len,
                max_byte_len,
            } => write!(
                formatter,
                "claim WAL payload length {byte_len} exceeds max {max_byte_len}"
            ),
            Self::SequenceOverflow { last_seq } => {
                write!(formatter, "claim WAL sequence overflow after {last_seq}")
            }
            Self::WriteWal { path, source } => {
                write!(formatter, "write WAL {} failed: {source}", path.display())
            }
            Self::SyncWal { path, source } => {
                write!(formatter, "sync WAL {} failed: {source}", path.display())
            }
            Self::RotateWal { path, source } => {
                write!(formatter, "rotate WAL {} failed: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ClaimWalAppendError {}

impl fmt::Display for ClaimWalReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenLock { path, source } => {
                write!(
                    formatter,
                    "open WAL lock {} failed: {source}",
                    path.display()
                )
            }
            Self::Lock { path, source } => {
                write!(formatter, "lock WAL {} failed: {source}", path.display())
            }
            Self::ReadWal { path, source } => {
                write!(formatter, "read WAL {} failed: {source}", path.display())
            }
            Self::RepairWal { path, source } => {
                write!(formatter, "repair WAL {} failed: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ClaimWalReadError {}

impl fmt::Display for ClaimWalRotationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDir { path, source } => {
                write!(formatter, "create WAL directory {} failed: {source}", path.display())
            }
            Self::OpenLock { path, source } => {
                write!(formatter, "open WAL lock {} failed: {source}", path.display())
            }
            Self::Lock { path, source } => {
                write!(formatter, "lock WAL {} failed: {source}", path.display())
            }
            Self::RecoverWal { path, source } => {
                write!(formatter, "recover WAL {} failed: {source}", path.display())
            }
            Self::RecoveryStopped {
                stop_reason,
                last_good_offset,
                original_len,
            } => write!(
                formatter,
                "claim WAL recovery stopped with {stop_reason:?} at {last_good_offset}/{original_len}"
            ),
            Self::SerializeSnapshot { source } => {
                write!(formatter, "serialize claim WAL snapshot failed: {source}")
            }
            Self::SerializeCheckpoint { source } => {
                write!(formatter, "serialize claim WAL checkpoint failed: {source}")
            }
            Self::SerializeManifest { source } => {
                write!(formatter, "serialize claim WAL manifest failed: {source}")
            }
            Self::SequenceOverflow { last_seq } => {
                write!(formatter, "claim WAL sequence overflow after {last_seq}")
            }
            Self::WriteFile { path, source } => {
                write!(formatter, "write {} failed: {source}", path.display())
            }
            Self::SyncFile { path, source } => {
                write!(formatter, "sync {} failed: {source}", path.display())
            }
            Self::RenameFile { from, to, source } => write!(
                formatter,
                "rename {} to {} failed: {source}",
                from.display(),
                to.display()
            ),
            Self::CopyFile { from, to, source } => write!(
                formatter,
                "copy {} to {} failed: {source}",
                from.display(),
                to.display()
            ),
            Self::VerifyWal { path, source } => {
                write!(formatter, "verify rotated WAL {} failed: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ClaimWalRotationError {}

#[must_use]
pub fn claim_wal_path(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(CLAIM_WAL_RELATIVE_PATH)
}

#[must_use]
pub fn claim_wal_lock_path(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(CLAIM_WAL_LOCK_RELATIVE_PATH)
}

#[must_use]
pub fn claim_wal_manifest_path(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(CLAIM_WAL_MANIFEST_RELATIVE_PATH)
}

#[must_use]
pub fn claim_wal_snapshot_dir(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(CLAIM_WAL_SNAPSHOT_RELATIVE_DIR)
}

#[must_use]
pub fn claim_wal_archive_dir(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(CLAIM_WAL_ARCHIVE_RELATIVE_DIR)
}

/// Append one claim lifecycle event to the binary Forge Method WAL.
///
/// The append path holds the WAL lock, repairs any torn tail to the last
/// verified prefix, computes the next monotonic sequence number, writes the
/// complete record, and syncs the WAL file before returning.
///
/// # Errors
///
/// Returns [`ClaimWalAppendError`] when the lock cannot be acquired, the
/// existing WAL cannot be recovered, the payload cannot be serialized, the
/// payload is too large, the sequence number overflows, or the record cannot be
/// written and synced.
pub fn append_claim_wal_record(
    state_root: impl AsRef<Path>,
    operation: ClaimWalOperation,
    claim_contract: &ClaimContract,
    recorded_at: &str,
) -> Result<ClaimWalAppendResult, ClaimWalAppendError> {
    append_claim_wal_record_with_durability(
        state_root,
        operation,
        claim_contract,
        recorded_at,
        crate::WalDurability::SyncOnAppend,
    )
}

/// Like [`append_claim_wal_record`] but lets the caller pick the
/// [`crate::WalDurability`] tier. See ADR-0009 for when `NoSync` is appropriate
/// (benchmarks, tests, dev) and when it is not (production).
///
/// # Errors
///
/// Forwards [`ClaimWalAppendError`] from the inner append path.
pub fn append_claim_wal_record_with_durability(
    state_root: impl AsRef<Path>,
    operation: ClaimWalOperation,
    claim_contract: &ClaimContract,
    recorded_at: &str,
    durability: crate::WalDurability,
) -> Result<ClaimWalAppendResult, ClaimWalAppendError> {
    let state_root = state_root.as_ref();
    let wal_path = claim_wal_path(state_root);
    let lock_path = claim_wal_lock_path(state_root);

    let lock = acquire_claim_wal_append_lock(&lock_path)?;

    create_parent_dir(&wal_path).map_err(|source| ClaimWalAppendError::CreateDir {
        path: wal_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf(),
        source: source.to_string(),
    })?;
    let (recovery, recovery_elapsed_ms) = recover_appendable_wal(&wal_path)?;
    let last_seq = recovery.last_observed_seq;
    let seq = last_seq
        .checked_add(1)
        .ok_or(ClaimWalAppendError::SequenceOverflow { last_seq })?;
    let payload_bytes = lifecycle_payload_bytes(operation, claim_contract, recorded_at)?;
    let record_bytes = encode_record(seq, operation.record_type(), &payload_bytes)?;

    write_claim_wal_record_bytes_durability(&wal_path, &record_bytes, durability)?;
    maybe_rotate_after_append(
        state_root,
        &wal_path,
        &recovery,
        record_bytes.len(),
        recovery_elapsed_ms,
        recorded_at,
    )?;
    drop(lock);

    Ok(ClaimWalAppendResult {
        wal_path,
        seq,
        bytes_appended: u64::try_from(record_bytes.len()).unwrap_or(u64::MAX),
    })
}

fn acquire_claim_wal_append_lock(lock_path: &Path) -> Result<ClaimWalLock, ClaimWalAppendError> {
    create_parent_dir(lock_path).map_err(|source| ClaimWalAppendError::CreateDir {
        path: lock_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf(),
        source: source.to_string(),
    })?;
    lock_exclusive(lock_path).map_err(|source| match source.kind() {
        io::ErrorKind::Other => ClaimWalAppendError::Lock {
            path: lock_path.to_path_buf(),
            source: source.to_string(),
        },
        _ => ClaimWalAppendError::OpenLock {
            path: lock_path.to_path_buf(),
            source: source.to_string(),
        },
    })
}

fn recover_appendable_wal(wal_path: &Path) -> Result<(ClaimWalRecovery, u64), ClaimWalAppendError> {
    let recovery_started_at = Instant::now();
    let recovery = recover_claim_wal_under_lock(wal_path, true).map_err(|source| {
        ClaimWalAppendError::ReadWal {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    let recovery_elapsed_ms = elapsed_millis_u64(recovery_started_at);
    ensure_recovered_appendable(&recovery)?;
    Ok((recovery, recovery_elapsed_ms))
}

fn lifecycle_payload_bytes(
    operation: ClaimWalOperation,
    claim_contract: &ClaimContract,
    recorded_at: &str,
) -> Result<Vec<u8>, ClaimWalAppendError> {
    let payload = ClaimWalPayload {
        schema_version: "0.1".to_string(),
        operation,
        recorded_at: recorded_at.to_string(),
        claim_contract: claim_contract.clone(),
    };
    let payload_bytes =
        serde_json::to_vec(&payload).map_err(|source| ClaimWalAppendError::Serialize {
            source: source.to_string(),
        })?;
    if payload_bytes.len() > DEFAULT_MAX_PAYLOAD_LEN as usize {
        return Err(ClaimWalAppendError::PayloadTooLarge {
            byte_len: payload_bytes.len(),
            max_byte_len: DEFAULT_MAX_PAYLOAD_LEN,
        });
    }
    Ok(payload_bytes)
}

fn write_claim_wal_record_bytes_durability(
    wal_path: &Path,
    record_bytes: &[u8],
    durability: crate::WalDurability,
) -> Result<(), ClaimWalAppendError> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(wal_path)
        .map_err(|source| ClaimWalAppendError::OpenWal {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        })?;
    file.write_all(record_bytes)
        .map_err(|source| ClaimWalAppendError::WriteWal {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        })?;
    if let crate::WalDurability::SyncOnAppend = durability {
        file.sync_data()
            .map_err(|source| ClaimWalAppendError::SyncWal {
                path: wal_path.to_path_buf(),
                source: source.to_string(),
            })?;
    }
    Ok(())
}

fn maybe_rotate_after_append(
    state_root: &Path,
    wal_path: &Path,
    recovery: &ClaimWalRecovery,
    record_byte_len: usize,
    recovery_elapsed_ms: u64,
    recorded_at: &str,
) -> Result<(), ClaimWalAppendError> {
    let post_append_len = recovery
        .last_good_offset
        .saturating_add(u64::try_from(record_byte_len).unwrap_or(u64::MAX));
    let post_append_records = recovery.valid_record_count.saturating_add(1);
    let Some(reason) = rotation_reason(
        post_append_len,
        post_append_records,
        recovery_elapsed_ms,
        &ClaimWalRotationOptions::default(),
    ) else {
        return Ok(());
    };
    let rotation_recovery = recover_claim_wal_under_lock(wal_path, false).map_err(|source| {
        ClaimWalAppendError::RotateWal {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    rotate_claim_wal_under_lock(
        state_root,
        wal_path,
        &rotation_recovery,
        recorded_at,
        reason,
    )
    .map_err(|source| ClaimWalAppendError::RotateWal {
        path: wal_path.to_path_buf(),
        source: source.to_string(),
    })?;
    Ok(())
}

/// Recover the readable prefix of the claim WAL.
///
/// If `repair` is true, the WAL is truncated to the last verified byte offset
/// when recovery stops before clean EOF.
///
/// # Errors
///
/// Returns [`ClaimWalReadError`] when the WAL lock cannot be acquired, the WAL
/// file cannot be read, or a requested repair truncation cannot be persisted.
pub fn recover_claim_wal(
    state_root: impl AsRef<Path>,
    repair: bool,
) -> Result<ClaimWalRecovery, ClaimWalReadError> {
    let state_root = state_root.as_ref();
    let wal_path = claim_wal_path(state_root);
    let lock_path = claim_wal_lock_path(state_root);
    create_parent_dir(&lock_path).map_err(|source| ClaimWalReadError::OpenLock {
        path: lock_path.clone(),
        source: source.to_string(),
    })?;
    let lock = lock_exclusive(&lock_path).map_err(|source| ClaimWalReadError::Lock {
        path: lock_path.clone(),
        source: source.to_string(),
    })?;
    let result = recover_claim_wal_under_lock(&wal_path, repair);
    drop(lock);
    result.map_err(|source| ClaimWalReadError::ReadWal {
        path: wal_path,
        source: source.to_string(),
    })
}

/// Replay the claim WAL into the materialized claim state.
///
/// Projection is deterministic and last-record-wins per claim id. Released and
/// handoff-recorded claims remain present in the projected state so the engine
/// can distinguish historical/non-live claims from absent claims.
///
/// If `repair` is true, the underlying recovery pass may truncate a non-empty
/// invalid tail to the last verified byte offset before projection.
///
/// # Errors
///
/// Returns [`ClaimWalReadError`] when the WAL lock cannot be acquired, the WAL
/// file cannot be read, or a requested repair truncation cannot be persisted.
pub fn replay_claim_wal(
    state_root: impl AsRef<Path>,
    repair: bool,
) -> Result<ClaimWalProjection, ClaimWalReadError> {
    let recovery = recover_claim_wal(state_root, repair)?;
    Ok(project_claim_wal_recovery(recovery))
}

/// Project claim WAL records using explicit projection options.
///
/// # Errors
///
/// Returns [`ClaimWalProjectionError`] when WAL recovery fails or when
/// `RequireCleanEof` is selected and the recovered prefix stopped before clean
/// EOF.
pub fn project_claim_wal(
    state_root: impl AsRef<Path>,
    options: &ClaimWalProjectionOptions,
) -> Result<ClaimWalProjection, ClaimWalProjectionError> {
    let recovery = recover_claim_wal(state_root, options.repair)
        .map_err(|source| ClaimWalProjectionError::RecoverWal { source })?;
    if options.stop_policy == ClaimWalProjectionStopPolicy::RequireCleanEof
        && recovery.stop_reason != ClaimWalStopReason::CleanEof
    {
        return Err(ClaimWalProjectionError::RecoveryStopped {
            stop_reason: recovery.stop_reason,
            last_good_offset: recovery.last_good_offset,
            original_len: recovery.original_len,
        });
    }
    Ok(project_claim_wal_recovery(recovery))
}

/// Rotate the active claim WAL when one of the configured thresholds is crossed.
///
/// Rotation writes an external snapshot, installs a new active `claims.fmw1`
/// whose first record is a checkpoint reference, and archives the previous WAL.
/// All steps run under the same WAL lock used by lifecycle appends.
///
/// # Errors
///
/// Returns [`ClaimWalRotationError`] when recovery, snapshot persistence,
/// active WAL replacement, or post-rotation verification fails.
pub fn rotate_claim_wal_if_needed(
    state_root: impl AsRef<Path>,
    created_at: &str,
    options: &ClaimWalRotationOptions,
) -> Result<ClaimWalRotationResult, ClaimWalRotationError> {
    let state_root = state_root.as_ref();
    let wal_path = claim_wal_path(state_root);
    let lock_path = claim_wal_lock_path(state_root);
    create_parent_dir(&lock_path).map_err(|source| ClaimWalRotationError::OpenLock {
        path: lock_path.clone(),
        source: source.to_string(),
    })?;
    let lock = lock_exclusive(&lock_path).map_err(|source| match source.kind() {
        io::ErrorKind::Other => ClaimWalRotationError::Lock {
            path: lock_path.clone(),
            source: source.to_string(),
        },
        _ => ClaimWalRotationError::OpenLock {
            path: lock_path.clone(),
            source: source.to_string(),
        },
    })?;
    let recovery_started_at = Instant::now();
    let recovery = recover_claim_wal_under_lock(&wal_path, true).map_err(|source| {
        ClaimWalRotationError::RecoverWal {
            path: wal_path.clone(),
            source: source.to_string(),
        }
    })?;
    let recovery_elapsed_ms = elapsed_millis_u64(recovery_started_at);
    ensure_recovered_rotatable(&recovery)?;
    let Some(reason) = rotation_reason(
        recovery.original_len,
        recovery.valid_record_count,
        recovery_elapsed_ms,
        options,
    ) else {
        drop(lock);
        return Ok(ClaimWalRotationResult {
            rotated: false,
            reason: None,
            wal_path,
            snapshot_path: None,
            archived_wal_path: None,
            manifest_path: None,
            checkpoint_seq: None,
            last_seq_in_snapshot: recovery.last_observed_seq,
            compacted_records: recovery.valid_record_count,
        });
    };
    let result = rotate_claim_wal_under_lock(state_root, &wal_path, &recovery, created_at, reason)?;
    drop(lock);
    Ok(result)
}

#[must_use]
pub fn project_claim_wal_recovery(recovery: ClaimWalRecovery) -> ClaimWalProjection {
    let mut accumulator = ProjectionAccumulator::default();
    if let Some(checkpoint) = &recovery.checkpoint {
        accumulator.seed_snapshot(&checkpoint.snapshot);
    }
    for record in &recovery.records {
        accumulator.apply_record(record);
    }
    let claims = accumulator
        .latest_by_claim_id
        .values()
        .map(|projected| projected.claim_contract.clone())
        .collect();
    let (active_claim_ids_by_agent, active_claim_ids_by_scope, active_claim_ids_by_path) =
        build_active_indexes(&accumulator.active_by_claim_id);
    ClaimWalProjection {
        recovery,
        last_applied_seq: accumulator.last_applied_seq,
        applied_records: accumulator.applied_records,
        claims,
        latest_by_claim_id: accumulator.latest_by_claim_id,
        active_by_claim_id: accumulator.active_by_claim_id,
        released_by_claim_id: accumulator.released_by_claim_id,
        handoff_recorded_by_claim_id: accumulator.handoff_recorded_by_claim_id,
        active_claim_ids_by_agent,
        active_claim_ids_by_scope,
        active_claim_ids_by_path,
        diagnostics: accumulator.diagnostics,
    }
}

#[derive(Debug, Default)]
struct ProjectionAccumulator {
    latest_by_claim_id: BTreeMap<String, ProjectedClaim>,
    active_by_claim_id: BTreeMap<String, ProjectedClaim>,
    released_by_claim_id: BTreeMap<String, ProjectedClaim>,
    handoff_recorded_by_claim_id: BTreeMap<String, ProjectedClaim>,
    diagnostics: Vec<ClaimWalProjectionDiagnostic>,
    last_applied_seq: u64,
    applied_records: usize,
}

impl ProjectionAccumulator {
    fn seed_snapshot(&mut self, snapshot: &ClaimWalSnapshotPayload) {
        self.last_applied_seq = snapshot.last_seq;
        for snapshot_claim in &snapshot.latest_claims {
            let claim_id = snapshot_claim.claim_contract.id.0.clone();
            let projected = ProjectedClaim {
                claim_contract: snapshot_claim.claim_contract.clone(),
                last_seq: snapshot_claim.last_seq,
                last_operation: snapshot_claim.last_operation,
                recorded_at: snapshot_claim.recorded_at.clone(),
                wal_offset: 0,
            };
            self.latest_by_claim_id
                .insert(claim_id.clone(), projected.clone());
            match projected.claim_contract.status.value {
                ClaimStatus::Active | ClaimStatus::Stale => {
                    self.active_by_claim_id.insert(claim_id.clone(), projected);
                    self.released_by_claim_id.remove(&claim_id);
                    self.handoff_recorded_by_claim_id.remove(&claim_id);
                }
                ClaimStatus::Released => {
                    self.active_by_claim_id.remove(&claim_id);
                    self.released_by_claim_id
                        .insert(claim_id.clone(), projected);
                    self.handoff_recorded_by_claim_id.remove(&claim_id);
                }
                ClaimStatus::HandoffRecorded => {
                    self.active_by_claim_id.remove(&claim_id);
                    self.released_by_claim_id.remove(&claim_id);
                    self.handoff_recorded_by_claim_id
                        .insert(claim_id.clone(), projected);
                }
                ClaimStatus::Expired | ClaimStatus::HandoffRequired => {
                    self.active_by_claim_id.remove(&claim_id);
                    self.released_by_claim_id.remove(&claim_id);
                    self.handoff_recorded_by_claim_id.remove(&claim_id);
                }
            }
        }
    }

    fn apply_record(&mut self, record: &ClaimWalRecord) {
        let claim_id = record.payload.claim_contract.id.0.clone();
        let projected = projected_claim(record);
        push_status_mismatch_diagnostic(record, &mut self.diagnostics);
        match record.operation {
            ClaimWalOperation::Acquire => {
                if let Some(active) = self.active_by_claim_id.get(&claim_id) {
                    if active.claim_contract.claim.claimant_agent_id
                        != projected.claim_contract.claim.claimant_agent_id
                    {
                        self.diagnostics.push(projection_warning(
                            ClaimWalProjectionDiagnosticCode::ReacquireByDifferentAgent,
                            record,
                            "acquire replaced an active claim held by another agent",
                        ));
                    }
                }
                self.latest_by_claim_id
                    .insert(claim_id.clone(), projected.clone());
                self.active_by_claim_id.insert(claim_id.clone(), projected);
                self.released_by_claim_id.remove(&claim_id);
                self.handoff_recorded_by_claim_id.remove(&claim_id);
                self.record_applied(record);
            }
            ClaimWalOperation::Heartbeat => {
                if matching_active_claim(
                    record,
                    &self.active_by_claim_id,
                    &mut self.diagnostics,
                    MissingOp::Heartbeat,
                ) {
                    self.latest_by_claim_id
                        .insert(claim_id.clone(), projected.clone());
                    self.active_by_claim_id.insert(claim_id, projected);
                    self.record_applied(record);
                }
            }
            ClaimWalOperation::Release => {
                if matching_active_claim(
                    record,
                    &self.active_by_claim_id,
                    &mut self.diagnostics,
                    MissingOp::Release,
                ) {
                    self.active_by_claim_id.remove(&claim_id);
                    self.latest_by_claim_id
                        .insert(claim_id.clone(), projected.clone());
                    self.released_by_claim_id
                        .insert(claim_id.clone(), projected);
                    self.handoff_recorded_by_claim_id.remove(&claim_id);
                    self.record_applied(record);
                }
            }
            ClaimWalOperation::HandoffRecorded => {
                if matching_open_claim_for_handoff(
                    record,
                    &self.latest_by_claim_id,
                    &mut self.diagnostics,
                ) {
                    self.active_by_claim_id.remove(&claim_id);
                    self.latest_by_claim_id
                        .insert(claim_id.clone(), projected.clone());
                    self.handoff_recorded_by_claim_id
                        .insert(claim_id.clone(), projected);
                    self.released_by_claim_id.remove(&claim_id);
                    self.record_applied(record);
                }
            }
            ClaimWalOperation::ReconcileStatus => {
                if matching_active_claim(
                    record,
                    &self.active_by_claim_id,
                    &mut self.diagnostics,
                    MissingOp::Reconcile,
                ) {
                    match projected.claim_contract.status.value {
                        ClaimStatus::Active | ClaimStatus::Stale => {
                            self.active_by_claim_id
                                .insert(claim_id.clone(), projected.clone());
                        }
                        ClaimStatus::Expired
                        | ClaimStatus::HandoffRequired
                        | ClaimStatus::HandoffRecorded
                        | ClaimStatus::Released => {
                            self.active_by_claim_id.remove(&claim_id);
                        }
                    }
                    self.latest_by_claim_id
                        .insert(claim_id.clone(), projected.clone());
                    self.released_by_claim_id.remove(&claim_id);
                    self.handoff_recorded_by_claim_id.remove(&claim_id);
                    self.record_applied(record);
                }
            }
        }
    }

    fn record_applied(&mut self, record: &ClaimWalRecord) {
        self.last_applied_seq = record.seq;
        self.applied_records = self.applied_records.saturating_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissingOp {
    Heartbeat,
    Release,
    Reconcile,
}

fn matching_active_claim(
    record: &ClaimWalRecord,
    active_by_claim_id: &BTreeMap<String, ProjectedClaim>,
    diagnostics: &mut Vec<ClaimWalProjectionDiagnostic>,
    op: MissingOp,
) -> bool {
    let claim_id = &record.payload.claim_contract.id.0;
    let Some(active) = active_by_claim_id.get(claim_id) else {
        diagnostics.push(projection_warning(
            missing_diag_code(op),
            record,
            missing_message(op),
        ));
        return false;
    };
    if active.claim_contract.claim.claimant_agent_id
        != record.payload.claim_contract.claim.claimant_agent_id
    {
        diagnostics.push(projection_warning(
            mismatch_diag_code(op),
            record,
            "claim lifecycle record agent does not match active claim agent",
        ));
        return false;
    }
    true
}

fn missing_diag_code(op: MissingOp) -> ClaimWalProjectionDiagnosticCode {
    match op {
        MissingOp::Heartbeat => ClaimWalProjectionDiagnosticCode::HeartbeatWithoutActiveClaim,
        MissingOp::Release => ClaimWalProjectionDiagnosticCode::ReleaseWithoutActiveClaim,
        MissingOp::Reconcile => ClaimWalProjectionDiagnosticCode::ReconcileWithoutOpenClaim,
    }
}

fn mismatch_diag_code(op: MissingOp) -> ClaimWalProjectionDiagnosticCode {
    match op {
        MissingOp::Heartbeat => ClaimWalProjectionDiagnosticCode::HeartbeatAgentMismatch,
        MissingOp::Release => ClaimWalProjectionDiagnosticCode::ReleaseAgentMismatch,
        MissingOp::Reconcile => ClaimWalProjectionDiagnosticCode::ReconcileAgentMismatch,
    }
}

fn missing_message(op: MissingOp) -> &'static str {
    match op {
        MissingOp::Heartbeat => "heartbeat has no matching active claim",
        MissingOp::Release => "release has no matching active claim",
        MissingOp::Reconcile => "reconcile status record has no matching active claim",
    }
}

fn matching_open_claim_for_handoff(
    record: &ClaimWalRecord,
    latest_by_claim_id: &BTreeMap<String, ProjectedClaim>,
    diagnostics: &mut Vec<ClaimWalProjectionDiagnostic>,
) -> bool {
    let claim_id = &record.payload.claim_contract.id.0;
    let Some(latest) = latest_by_claim_id.get(claim_id) else {
        diagnostics.push(projection_warning(
            ClaimWalProjectionDiagnosticCode::HandoffWithoutActiveClaim,
            record,
            "handoff has no matching open claim",
        ));
        return false;
    };
    if !matches!(
        latest.claim_contract.status.value,
        ClaimStatus::Active | ClaimStatus::Stale | ClaimStatus::HandoffRequired
    ) {
        diagnostics.push(projection_warning(
            ClaimWalProjectionDiagnosticCode::HandoffWithoutActiveClaim,
            record,
            "handoff target is not open or handoff-required",
        ));
        return false;
    }
    if latest.claim_contract.claim.claimant_agent_id
        != record.payload.claim_contract.claim.claimant_agent_id
    {
        diagnostics.push(projection_warning(
            ClaimWalProjectionDiagnosticCode::HandoffAgentMismatch,
            record,
            "handoff lifecycle record agent does not match open claim agent",
        ));
        return false;
    }
    true
}

fn push_status_mismatch_diagnostic(
    record: &ClaimWalRecord,
    diagnostics: &mut Vec<ClaimWalProjectionDiagnostic>,
) {
    let status = record.payload.claim_contract.status.value;
    let ok = match record.operation {
        ClaimWalOperation::Acquire | ClaimWalOperation::Heartbeat => status == ClaimStatus::Active,
        ClaimWalOperation::Release => status == ClaimStatus::Released,
        ClaimWalOperation::HandoffRecorded => status == ClaimStatus::HandoffRecorded,
        ClaimWalOperation::ReconcileStatus => {
            matches!(
                status,
                ClaimStatus::Stale | ClaimStatus::Expired | ClaimStatus::HandoffRequired
            )
        }
    };
    if !ok {
        diagnostics.push(projection_warning(
            ClaimWalProjectionDiagnosticCode::OperationStatusMismatch,
            record,
            "claim WAL operation does not match embedded claim status",
        ));
    }
}

fn projection_warning(
    code: ClaimWalProjectionDiagnosticCode,
    record: &ClaimWalRecord,
    message: &str,
) -> ClaimWalProjectionDiagnostic {
    ClaimWalProjectionDiagnostic {
        severity: ClaimWalProjectionDiagnosticSeverity::Warning,
        code,
        seq: record.seq,
        claim_id: record.payload.claim_contract.id.0.clone(),
        message: message.to_string(),
    }
}

fn projected_claim(record: &ClaimWalRecord) -> ProjectedClaim {
    ProjectedClaim {
        claim_contract: record.payload.claim_contract.clone(),
        last_seq: record.seq,
        last_operation: record.operation,
        recorded_at: record.payload.recorded_at.clone(),
        wal_offset: record.offset,
    }
}

fn build_active_indexes(
    active_by_claim_id: &BTreeMap<String, ProjectedClaim>,
) -> (ClaimIdIndex, ClaimIdIndex, ClaimIdIndex) {
    let mut by_agent = ClaimIdIndex::new();
    let mut by_scope = ClaimIdIndex::new();
    let mut by_path = ClaimIdIndex::new();
    for (claim_id, projected) in active_by_claim_id {
        by_agent
            .entry(projected.claim_contract.claim.claimant_agent_id.0.clone())
            .or_default()
            .push(claim_id.clone());
        by_scope
            .entry(projected.claim_contract.scope.id.0.clone())
            .or_default()
            .push(claim_id.clone());
        for path in &projected.claim_contract.scope.paths {
            by_path
                .entry(path.0.clone())
                .or_default()
                .push(claim_id.clone());
        }
    }
    sort_index_values(&mut by_agent);
    sort_index_values(&mut by_scope);
    sort_index_values(&mut by_path);
    (by_agent, by_scope, by_path)
}

fn sort_index_values(index: &mut BTreeMap<String, Vec<String>>) {
    for values in index.values_mut() {
        values.sort();
        values.dedup();
    }
}

fn rotate_claim_wal_under_lock(
    state_root: &Path,
    wal_path: &Path,
    recovery: &ClaimWalRecovery,
    created_at: &str,
    reason: ClaimWalRotationReason,
) -> Result<ClaimWalRotationResult, ClaimWalRotationError> {
    ensure_recovered_rotatable(recovery)?;
    let projection = project_claim_wal_recovery((*recovery).clone());
    let last_seq_in_snapshot = recovery.last_observed_seq;
    let checkpoint_seq = next_checkpoint_seq(last_seq_in_snapshot)?;
    let created_at_ms = now_unix_millis();
    let (snapshot_path, snapshot_rel_path, snapshot_crc32c) = write_snapshot_file(
        state_root,
        &projection,
        last_seq_in_snapshot,
        created_at,
        created_at_ms,
    )?;
    let new_wal_bytes = checkpoint_record_bytes(
        &snapshot_rel_path,
        snapshot_crc32c,
        last_seq_in_snapshot,
        checkpoint_seq,
        created_at,
        created_at_ms,
    )?;
    let (archive_abs_path, archive_path) =
        replace_active_wal_with_checkpoint(state_root, wal_path, checkpoint_seq, &new_wal_bytes)?;
    verify_rotated_wal(wal_path)?;
    let manifest_path = write_rotation_manifest(
        state_root,
        &RotationManifestInput {
            snapshot_rel_path: &snapshot_rel_path,
            snapshot_crc32c,
            archive_path: &archive_path,
            checkpoint_seq,
            last_seq_in_snapshot,
            updated_at: created_at,
            updated_at_ms: created_at_ms,
        },
    )?;
    Ok(ClaimWalRotationResult {
        rotated: true,
        reason: Some(reason),
        wal_path: wal_path.to_path_buf(),
        snapshot_path: Some(snapshot_path),
        archived_wal_path: Some(archive_abs_path),
        manifest_path: Some(manifest_path),
        checkpoint_seq: Some(checkpoint_seq),
        last_seq_in_snapshot,
        compacted_records: recovery.valid_record_count,
    })
}

fn next_checkpoint_seq(last_seq: u64) -> Result<u64, ClaimWalRotationError> {
    last_seq
        .checked_add(1)
        .ok_or(ClaimWalRotationError::SequenceOverflow { last_seq })
}

fn write_snapshot_file(
    state_root: &Path,
    projection: &ClaimWalProjection,
    last_seq: u64,
    created_at: &str,
    created_at_ms: u64,
) -> Result<(PathBuf, PathBuf, u32), ClaimWalRotationError> {
    let snapshot_rel_path = snapshot_relative_path(last_seq);
    let snapshot = ClaimWalSnapshotPayload {
        schema_version: "0.1".to_string(),
        created_at: created_at.to_string(),
        created_at_ms,
        last_seq,
        latest_claims: snapshot_claims_from_projection(projection),
    };
    let snapshot_bytes = serde_json::to_vec(&snapshot).map_err(|source| {
        ClaimWalRotationError::SerializeSnapshot {
            source: source.to_string(),
        }
    })?;
    let snapshot_crc32c = crc32c::crc32c(&snapshot_bytes);
    let snapshot_path = state_root.join(&snapshot_rel_path);
    write_durable_replaced_file(&snapshot_path, &snapshot_bytes)
        .map_err(|source| map_rotation_io_error(&snapshot_path, &source))?;
    Ok((snapshot_path, snapshot_rel_path, snapshot_crc32c))
}

fn checkpoint_record_bytes(
    snapshot_rel_path: &Path,
    snapshot_crc32c: u32,
    last_seq_in_snapshot: u64,
    checkpoint_seq: u64,
    created_at: &str,
    created_at_ms: u64,
) -> Result<Vec<u8>, ClaimWalRotationError> {
    let checkpoint_payload = ClaimWalCheckpointPayload {
        schema_version: "0.1".to_string(),
        snapshot_path: path_to_wal_string(snapshot_rel_path),
        snapshot_crc32c,
        last_seq_in_snapshot,
        created_at: created_at.to_string(),
        created_at_ms,
    };
    let checkpoint_bytes = serde_json::to_vec(&checkpoint_payload).map_err(|source| {
        ClaimWalRotationError::SerializeCheckpoint {
            source: source.to_string(),
        }
    })?;
    encode_record(
        checkpoint_seq,
        RECORD_TYPE_CHECKPOINT_REF,
        &checkpoint_bytes,
    )
    .map_err(|source| ClaimWalRotationError::SerializeCheckpoint {
        source: source.to_string(),
    })
}

fn replace_active_wal_with_checkpoint(
    state_root: &Path,
    wal_path: &Path,
    checkpoint_seq: u64,
    new_wal_bytes: &[u8],
) -> Result<(PathBuf, PathBuf), ClaimWalRotationError> {
    let wal_dir = wal_path
        .parent()
        .ok_or_else(|| ClaimWalRotationError::CreateDir {
            path: wal_path.to_path_buf(),
            source: "active WAL path has no parent".to_string(),
        })?;
    fs::create_dir_all(wal_dir).map_err(|source| ClaimWalRotationError::CreateDir {
        path: wal_dir.to_path_buf(),
        source: source.to_string(),
    })?;
    let archive_path = archive_relative_path(checkpoint_seq);
    let archive_abs_path = state_root.join(&archive_path);
    copy_active_wal_to_archive(wal_path, &archive_abs_path)?;
    write_durable_replaced_file(wal_path, new_wal_bytes)
        .map_err(|source| map_rotation_io_error(wal_path, &source))?;
    Ok((archive_abs_path, archive_path))
}

fn verify_rotated_wal(wal_path: &Path) -> Result<(), ClaimWalRotationError> {
    let verification = recover_claim_wal_under_lock(wal_path, false).map_err(|source| {
        ClaimWalRotationError::VerifyWal {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    if verification.stop_reason == ClaimWalStopReason::CleanEof {
        return Ok(());
    }
    Err(ClaimWalRotationError::VerifyWal {
        path: wal_path.to_path_buf(),
        source: format!(
            "rotated WAL recovery stopped with {:?}",
            verification.stop_reason
        ),
    })
}

struct RotationManifestInput<'a> {
    snapshot_rel_path: &'a Path,
    snapshot_crc32c: u32,
    archive_path: &'a Path,
    checkpoint_seq: u64,
    last_seq_in_snapshot: u64,
    updated_at: &'a str,
    updated_at_ms: u64,
}

fn write_rotation_manifest(
    state_root: &Path,
    input: &RotationManifestInput<'_>,
) -> Result<PathBuf, ClaimWalRotationError> {
    let manifest_path = claim_wal_manifest_path(state_root);
    let manifest = ClaimWalManifestPayload {
        schema_version: "0.1".to_string(),
        active_wal_path: CLAIM_WAL_RELATIVE_PATH.to_string(),
        snapshot_path: path_to_wal_string(input.snapshot_rel_path),
        snapshot_crc32c: input.snapshot_crc32c,
        archived_wal_path: path_to_wal_string(input.archive_path),
        checkpoint_seq: input.checkpoint_seq,
        last_seq_in_snapshot: input.last_seq_in_snapshot,
        updated_at: input.updated_at.to_string(),
        updated_at_ms: input.updated_at_ms,
    };
    let manifest_bytes = serde_json::to_vec(&manifest).map_err(|source| {
        ClaimWalRotationError::SerializeManifest {
            source: source.to_string(),
        }
    })?;
    write_durable_replaced_file(&manifest_path, &manifest_bytes)
        .map_err(|source| map_rotation_io_error(&manifest_path, &source))?;
    Ok(manifest_path)
}

fn snapshot_claims_from_projection(projection: &ClaimWalProjection) -> Vec<ClaimWalSnapshotClaim> {
    projection
        .latest_by_claim_id
        .values()
        .map(|projected| ClaimWalSnapshotClaim {
            claim_contract: projected.claim_contract.clone(),
            last_seq: projected.last_seq,
            last_operation: projected.last_operation,
            recorded_at: projected.recorded_at.clone(),
        })
        .collect()
}

fn rotation_reason(
    wal_bytes: u64,
    record_count: usize,
    replay_millis: u64,
    options: &ClaimWalRotationOptions,
) -> Option<ClaimWalRotationReason> {
    if wal_bytes > options.max_wal_bytes {
        return Some(ClaimWalRotationReason::WalSizeBytes);
    }
    if record_count > options.max_records {
        return Some(ClaimWalRotationReason::RecordCount);
    }
    (replay_millis > options.max_replay_millis)
        .then_some(ClaimWalRotationReason::ReplayDurationMillis)
}

fn ensure_recovered_appendable(recovery: &ClaimWalRecovery) -> Result<(), ClaimWalAppendError> {
    if recovery.stop_reason == ClaimWalStopReason::CleanEof || recovery.repaired {
        return Ok(());
    }
    Err(ClaimWalAppendError::RecoveryStopped {
        stop_reason: recovery.stop_reason,
        last_good_offset: recovery.last_good_offset,
        original_len: recovery.original_len,
    })
}

fn ensure_recovered_rotatable(recovery: &ClaimWalRecovery) -> Result<(), ClaimWalRotationError> {
    if recovery.stop_reason == ClaimWalStopReason::CleanEof || recovery.repaired {
        return Ok(());
    }
    Err(ClaimWalRotationError::RecoveryStopped {
        stop_reason: recovery.stop_reason,
        last_good_offset: recovery.last_good_offset,
        original_len: recovery.original_len,
    })
}

fn copy_active_wal_to_archive(
    wal_path: &Path,
    archive_path: &Path,
) -> Result<(), ClaimWalRotationError> {
    let archive_dir = archive_path
        .parent()
        .ok_or_else(|| ClaimWalRotationError::CreateDir {
            path: archive_path.to_path_buf(),
            source: "archive path has no parent".to_string(),
        })?;
    fs::create_dir_all(archive_dir).map_err(|source| ClaimWalRotationError::CreateDir {
        path: archive_dir.to_path_buf(),
        source: source.to_string(),
    })?;
    fs::copy(wal_path, archive_path).map_err(|source| ClaimWalRotationError::CopyFile {
        from: wal_path.to_path_buf(),
        to: archive_path.to_path_buf(),
        source: source.to_string(),
    })?;
    let archive = OpenOptions::new()
        .read(true)
        .write(true)
        .open(archive_path)
        .map_err(|source| ClaimWalRotationError::SyncFile {
            path: archive_path.to_path_buf(),
            source: source.to_string(),
        })?;
    archive
        .sync_all()
        .map_err(|source| ClaimWalRotationError::SyncFile {
            path: archive_path.to_path_buf(),
            source: source.to_string(),
        })?;
    sync_parent_dir_best_effort(archive_path).map_err(|source| ClaimWalRotationError::SyncFile {
        path: archive_dir.to_path_buf(),
        source: source.to_string(),
    })
}

fn write_durable_replaced_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    create_parent_dir(path)?;
    let temp_path = temp_path_for(path);
    {
        let mut temp_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)?;
        temp_file.write_all(bytes)?;
        temp_file.sync_all()?;
    }
    fs::rename(&temp_path, path)?;
    sync_parent_dir_best_effort(path)
}

fn map_rotation_io_error(path: &Path, source: &io::Error) -> ClaimWalRotationError {
    match source.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => {
            ClaimWalRotationError::WriteFile {
                path: path.to_path_buf(),
                source: source.to_string(),
            }
        }
        _ => ClaimWalRotationError::SyncFile {
            path: path.to_path_buf(),
            source: source.to_string(),
        },
    }
}

fn load_checkpoint_snapshot(
    wal_path: &Path,
    payload: &ClaimWalCheckpointPayload,
) -> Option<ClaimWalSnapshotPayload> {
    let state_root = state_root_from_wal_path(wal_path)?;
    let snapshot_rel = safe_relative_path(&payload.snapshot_path)?;
    let snapshot_path = state_root.join(snapshot_rel);
    let snapshot_bytes = fs::read(snapshot_path).ok()?;
    if crc32c::crc32c(&snapshot_bytes) != payload.snapshot_crc32c {
        return None;
    }
    let snapshot = serde_json::from_slice::<ClaimWalSnapshotPayload>(&snapshot_bytes).ok()?;
    (snapshot.schema_version == "0.1").then_some(snapshot)
}

fn state_root_from_wal_path(wal_path: &Path) -> Option<PathBuf> {
    wal_path.parent()?.parent().map(Path::to_path_buf)
}

fn safe_relative_path(value: &str) -> Option<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute() {
        return None;
    }
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!safe.as_os_str().is_empty()).then_some(safe)
}

fn snapshot_relative_path(last_seq: u64) -> PathBuf {
    PathBuf::from(CLAIM_WAL_SNAPSHOT_RELATIVE_DIR)
        .join(format!("claims.snapshot.{last_seq:020}.json"))
}

fn archive_relative_path(checkpoint_seq: u64) -> PathBuf {
    PathBuf::from(CLAIM_WAL_ARCHIVE_RELATIVE_DIR)
        .join(format!("claims.fmw1.before-{checkpoint_seq:020}"))
}

fn temp_path_for(path: &Path) -> PathBuf {
    let extension = format!("tmp.{}.{}", std::process::id(), now_unix_millis());
    path.with_extension(extension)
}

fn path_to_wal_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn elapsed_millis_u64(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn now_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(u64::MAX)
}

fn is_repairable_stop_reason(reason: ClaimWalStopReason) -> bool {
    !matches!(
        reason,
        ClaimWalStopReason::CleanEof
            | ClaimWalStopReason::CheckpointSnapshotInvalid
            | ClaimWalStopReason::CheckpointNotAtStart
    )
}

fn sync_parent_dir_best_effort(path: &Path) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    match File::open(parent) {
        Ok(directory) => directory.sync_all(),
        Err(error) if is_unsupported_directory_sync(&error) => Ok(()),
        Err(error) => Err(error),
    }
}

fn is_unsupported_directory_sync(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::PermissionDenied
            | io::ErrorKind::Unsupported
            | io::ErrorKind::NotFound
            | io::ErrorKind::Other
    )
}

#[cfg(test)]
mod tests {
    use super::{
        rotation_reason, ClaimWalRotationOptions, ClaimWalRotationReason,
        DEFAULT_ROTATE_MAX_RECORDS, DEFAULT_ROTATE_MAX_REPLAY_MILLIS, DEFAULT_ROTATE_MAX_WAL_BYTES,
    };

    #[test]
    fn rotation_defaults_match_backlog_thresholds() {
        let options = ClaimWalRotationOptions::default();

        assert_eq!(options.max_wal_bytes, 64 * 1024 * 1024);
        assert_eq!(options.max_wal_bytes, DEFAULT_ROTATE_MAX_WAL_BYTES);
        assert_eq!(options.max_records, 100_000);
        assert_eq!(options.max_records, DEFAULT_ROTATE_MAX_RECORDS);
        assert_eq!(options.max_replay_millis, 250);
        assert_eq!(options.max_replay_millis, DEFAULT_ROTATE_MAX_REPLAY_MILLIS);
    }

    #[test]
    fn rotation_reason_prefers_size_then_records_then_replay_time() {
        let options = ClaimWalRotationOptions {
            max_wal_bytes: 10,
            max_records: 5,
            max_replay_millis: 2,
        };

        assert_eq!(
            rotation_reason(11, 6, 3, &options),
            Some(ClaimWalRotationReason::WalSizeBytes)
        );
        assert_eq!(
            rotation_reason(10, 6, 3, &options),
            Some(ClaimWalRotationReason::RecordCount)
        );
        assert_eq!(
            rotation_reason(10, 5, 3, &options),
            Some(ClaimWalRotationReason::ReplayDurationMillis)
        );
        assert_eq!(rotation_reason(10, 5, 2, &options), None);
    }
}

fn recover_claim_wal_under_lock(wal_path: &Path, repair: bool) -> io::Result<ClaimWalRecovery> {
    let bytes = match fs::read(wal_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    let original_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let mut recovery = decode_prefix(wal_path, &bytes);
    if repair
        && recovery.last_good_offset < original_len
        && is_repairable_stop_reason(recovery.stop_reason)
    {
        let file = OpenOptions::new().write(true).open(wal_path)?;
        file.set_len(recovery.last_good_offset)?;
        file.sync_all()?;
        recovery.repaired = true;
    }
    Ok(recovery)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DecodeRecordOutcome {
    Known {
        record: Box<ClaimWalRecord>,
        next_offset: usize,
    },
    Checkpoint {
        checkpoint: Box<ClaimWalCheckpointRecord>,
        next_offset: usize,
    },
    SkippedUnknown {
        seq: u64,
        next_offset: usize,
    },
    Stop(ClaimWalStopReason),
}

fn decode_prefix(wal_path: &Path, bytes: &[u8]) -> ClaimWalRecovery {
    let original_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let mut records = Vec::new();
    let mut checkpoint = None;
    let mut offset = 0usize;
    let mut expected_seq = 1u64;
    let mut last_observed_seq = 0u64;
    let mut valid_record_count = 0usize;
    let mut stop_reason = ClaimWalStopReason::CleanEof;

    loop {
        let remaining = bytes.len().saturating_sub(offset);
        if remaining == 0 {
            break;
        }
        match decode_record_at(wal_path, bytes, offset, expected_seq) {
            DecodeRecordOutcome::Known {
                record,
                next_offset,
            } => {
                last_observed_seq = record.seq;
                records.push(*record);
                offset = next_offset;
                expected_seq = expected_seq.saturating_add(1);
                valid_record_count = valid_record_count.saturating_add(1);
            }
            DecodeRecordOutcome::Checkpoint {
                checkpoint: decoded_checkpoint,
                next_offset,
            } => {
                last_observed_seq = decoded_checkpoint.seq;
                checkpoint = Some(*decoded_checkpoint);
                offset = next_offset;
                expected_seq = last_observed_seq.saturating_add(1);
                valid_record_count = valid_record_count.saturating_add(1);
            }
            DecodeRecordOutcome::SkippedUnknown { seq, next_offset } => {
                last_observed_seq = seq;
                offset = next_offset;
                expected_seq = expected_seq.saturating_add(1);
                valid_record_count = valid_record_count.saturating_add(1);
            }
            DecodeRecordOutcome::Stop(reason) => {
                stop_reason = reason;
                break;
            }
        }
    }

    ClaimWalRecovery {
        wal_path: wal_path.to_path_buf(),
        records,
        checkpoint,
        last_observed_seq,
        valid_record_count,
        last_good_offset: u64::try_from(offset).unwrap_or(u64::MAX),
        original_len,
        repaired: false,
        stop_reason,
    }
}

struct DecodedRecordFrame<'a> {
    flags: u16,
    record_type: u8,
    seq: u64,
    payload: &'a [u8],
    record_end: usize,
}

fn decode_record_at(
    wal_path: &Path,
    bytes: &[u8],
    offset: usize,
    expected_seq: u64,
) -> DecodeRecordOutcome {
    let frame = match decode_record_frame(bytes, offset) {
        Ok(frame) => frame,
        Err(reason) => return DecodeRecordOutcome::Stop(reason),
    };
    if frame.record_type == RECORD_TYPE_CHECKPOINT_REF {
        return decode_checkpoint_record(wal_path, offset, &frame);
    }
    if frame.seq != expected_seq {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::SequenceGap);
    }
    let Some(operation) = ClaimWalOperation::from_record_type(frame.record_type) else {
        if frame.flags & FLAG_SKIPPABLE_UNKNOWN != 0 {
            return DecodeRecordOutcome::SkippedUnknown {
                seq: frame.seq,
                next_offset: frame.record_end,
            };
        }
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::UnsupportedRecordType);
    };
    decode_lifecycle_record(offset, operation, &frame)
}

fn decode_record_frame(
    bytes: &[u8],
    offset: usize,
) -> Result<DecodedRecordFrame<'_>, ClaimWalStopReason> {
    if bytes.len().saturating_sub(offset) < HEADER_LEN {
        return Err(ClaimWalStopReason::TruncatedHeader);
    }
    let header = &bytes[offset..offset + HEADER_LEN];
    if header[0..4] != MAGIC || header[4] != VERSION {
        return Err(ClaimWalStopReason::InvalidHeader);
    }
    let flags = u16::from_le_bytes([header[6], header[7]]);
    if flags & !ALLOWED_FLAGS != 0 || flags & FLAG_PAYLOAD_JSON == 0 {
        return Err(ClaimWalStopReason::InvalidHeader);
    }
    let payload_len = u32::from_le_bytes(header[16..20].try_into().expect("4 byte payload length"));
    if payload_len > DEFAULT_MAX_PAYLOAD_LEN {
        return Err(ClaimWalStopReason::PayloadTooLarge);
    }
    let header_crc = u32::from_le_bytes(
        header[HEADER_CRC_OFFSET..HEADER_LEN]
            .try_into()
            .expect("4 byte header crc"),
    );
    if crc32c::crc32c(&header[0..HEADER_CRC_OFFSET]) != header_crc {
        return Err(ClaimWalStopReason::InvalidHeader);
    }
    let payload_len_usize =
        usize::try_from(payload_len).map_err(|_| ClaimWalStopReason::PayloadTooLarge)?;
    let record_end = checked_record_end(offset, payload_len_usize)?;
    if record_end > bytes.len() {
        return Err(ClaimWalStopReason::TruncatedPayload);
    }
    let payload_start = offset + HEADER_LEN;
    let payload_end = payload_start + payload_len_usize;
    let payload = &bytes[payload_start..payload_end];
    let payload_crc = u32::from_le_bytes(
        bytes[payload_end..record_end]
            .try_into()
            .expect("4 byte payload crc"),
    );
    if payload_crc32c(&header[0..HEADER_CRC_OFFSET], payload) != payload_crc {
        return Err(ClaimWalStopReason::PayloadChecksumMismatch);
    }
    Ok(DecodedRecordFrame {
        flags,
        record_type: header[5],
        seq: u64::from_le_bytes(header[8..16].try_into().expect("8 byte seq")),
        payload,
        record_end,
    })
}

fn checked_record_end(offset: usize, payload_len: usize) -> Result<usize, ClaimWalStopReason> {
    offset
        .checked_add(HEADER_LEN)
        .and_then(|value| value.checked_add(payload_len))
        .and_then(|value| value.checked_add(TRAILER_LEN))
        .ok_or(ClaimWalStopReason::PayloadTooLarge)
}

fn decode_checkpoint_record(
    wal_path: &Path,
    offset: usize,
    frame: &DecodedRecordFrame<'_>,
) -> DecodeRecordOutcome {
    if offset != 0 {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::CheckpointNotAtStart);
    }
    let Ok(decoded_payload) = serde_json::from_slice::<ClaimWalCheckpointPayload>(frame.payload)
    else {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::PayloadDecodeFailed);
    };
    if decoded_payload.schema_version != "0.1" {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::PayloadDecodeFailed);
    }
    let Some(snapshot) = load_checkpoint_snapshot(wal_path, &decoded_payload) else {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::CheckpointSnapshotInvalid);
    };
    if decoded_payload.last_seq_in_snapshot != snapshot.last_seq {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::CheckpointSnapshotInvalid);
    }
    let Some(expected_checkpoint_seq) = snapshot.last_seq.checked_add(1) else {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::SequenceGap);
    };
    if frame.seq != expected_checkpoint_seq {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::SequenceGap);
    }
    DecodeRecordOutcome::Checkpoint {
        checkpoint: Box::new(ClaimWalCheckpointRecord {
            seq: frame.seq,
            payload: decoded_payload,
            snapshot,
            offset: u64::try_from(offset).unwrap_or(u64::MAX),
            record_len: u64::try_from(frame.record_end - offset).unwrap_or(u64::MAX),
        }),
        next_offset: frame.record_end,
    }
}

fn decode_lifecycle_record(
    offset: usize,
    operation: ClaimWalOperation,
    frame: &DecodedRecordFrame<'_>,
) -> DecodeRecordOutcome {
    let Ok(decoded_payload) = serde_json::from_slice::<ClaimWalPayload>(frame.payload) else {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::PayloadDecodeFailed);
    };
    if decoded_payload.operation != operation {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::PayloadDecodeFailed);
    }
    DecodeRecordOutcome::Known {
        record: Box::new(ClaimWalRecord {
            seq: frame.seq,
            operation,
            payload: decoded_payload,
            offset: u64::try_from(offset).unwrap_or(u64::MAX),
            record_len: u64::try_from(frame.record_end - offset).unwrap_or(u64::MAX),
        }),
        next_offset: frame.record_end,
    }
}

fn encode_record(
    seq: u64,
    record_type: u8,
    payload: &[u8],
) -> Result<Vec<u8>, ClaimWalAppendError> {
    let payload_len =
        u32::try_from(payload.len()).map_err(|_| ClaimWalAppendError::PayloadTooLarge {
            byte_len: payload.len(),
            max_byte_len: DEFAULT_MAX_PAYLOAD_LEN,
        })?;
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(&MAGIC);
    header.push(VERSION);
    header.push(record_type);
    header.extend_from_slice(&FLAG_PAYLOAD_JSON.to_le_bytes());
    header.extend_from_slice(&seq.to_le_bytes());
    header.extend_from_slice(&payload_len.to_le_bytes());
    let header_crc = crc32c::crc32c(&header);
    header.extend_from_slice(&header_crc.to_le_bytes());

    let payload_crc = payload_crc32c(&header[0..HEADER_CRC_OFFSET], payload);
    let mut record = Vec::with_capacity(HEADER_LEN + payload.len() + TRAILER_LEN);
    record.extend_from_slice(&header);
    record.extend_from_slice(payload);
    record.extend_from_slice(&payload_crc.to_le_bytes());
    Ok(record)
}

fn payload_crc32c(header_prefix: &[u8], payload: &[u8]) -> u32 {
    let mut covered = Vec::with_capacity(header_prefix.len() + payload.len());
    covered.extend_from_slice(header_prefix);
    covered.extend_from_slice(payload);
    crc32c::crc32c(&covered)
}

fn create_parent_dir(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
    fs::create_dir_all(parent)
}

fn lock_exclusive(path: &Path) -> io::Result<ClaimWalLock> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    FileExt::lock(&file)?;
    Ok(ClaimWalLock { file })
}

struct ClaimWalLock {
    file: File,
}

impl Drop for ClaimWalLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

/// Fuzz-only entrypoint that reuses [`decode_prefix`] against a synthetic
/// in-memory buffer without touching the filesystem. The `wal_path` argument
/// exists only to populate diagnostic messages; the fuzzer supplies a
/// placeholder. Not part of the stable API surface.
#[cfg(feature = "fuzz")]
#[must_use]
pub fn recover_claim_wal_from_bytes(bytes: &[u8]) -> ClaimWalRecovery {
    decode_prefix(std::path::Path::new("<fuzz>"), bytes)
}
