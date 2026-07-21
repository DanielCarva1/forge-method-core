use forge_core_contracts::claim::{ClaimContract, ClaimStatus};
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read as _, Seek as _, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
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
pub const CLAIM_WAL_CHECKPOINT_RELATIVE_DIR: &str = "wal/checkpoints";
pub const CLAIM_WAL_SNAPSHOT_RELATIVE_DIR: &str = "wal/snapshots";
pub const CLAIM_WAL_ARCHIVE_RELATIVE_DIR: &str = "wal/archive";

const CLAIM_CHECKPOINT_GENERATION_SCHEMA_VERSION: &str = "0.2";
const CLAIM_CHECKPOINT_PAYLOAD_SCHEMA_VERSION: &str = "0.4";
const CLAIM_CHECKPOINT_AUTHORITY_KIND: &str = "forge-claim-checkpoint-generation";
const CLAIM_CHECKPOINT_ANCHOR_RELATIVE_DIR: &str = "wal/checkpoint-authority";
const CLAIM_GENERATION_ANCHOR_RELATIVE_DIR: &str = "wal/checkpoint-authority/generations";
const CLAIM_SNAPSHOT_ANCHOR_RELATIVE_DIR: &str = "wal/checkpoint-authority/snapshots";
const CLAIM_ARCHIVE_ANCHOR_RELATIVE_DIR: &str = "wal/checkpoint-authority/archives";

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

/// Persisted Store-owned lifetime-anchor binding for one immutable claim leaf.
///
/// The binding is data only. Recovery accepts it only by reopening the private
/// anchor through the retained-directory foundation and retaining the exact
/// anchored target handle; callers cannot mint the opaque capability that
/// authorizes a successful read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalFileAnchorBinding {
    pub schema_version: String,
    pub anchor_relative_path: String,
    pub nonce: String,
    pub content_digest: String,
    pub byte_length: u64,
}

impl ClaimWalFileAnchorBinding {
    fn from_retained(binding: &crate::retained_dir::RetainedFileAnchorBinding) -> Self {
        Self {
            schema_version: binding.schema_version.clone(),
            anchor_relative_path: binding.anchor_relative_path.clone(),
            nonce: binding.nonce.clone(),
            content_digest: binding.content_digest.clone(),
            byte_length: binding.byte_length,
        }
    }

    fn to_retained(&self) -> crate::retained_dir::RetainedFileAnchorBinding {
        crate::retained_dir::RetainedFileAnchorBinding {
            schema_version: self.schema_version.clone(),
            anchor_relative_path: self.anchor_relative_path.clone(),
            nonce: self.nonce.clone(),
            content_digest: self.content_digest.clone(),
            byte_length: self.byte_length,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalCheckpointPayload {
    pub schema_version: String,
    pub snapshot_path: String,
    pub snapshot_crc32c: u32,
    pub last_seq_in_snapshot: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_wal_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_wal_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_anchor: Option<ClaimWalFileAnchorBinding>,
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

/// Store-owned immutable checkpoint authority. The active WAL selects exactly
/// one content-addressed instance of this sealed record; legacy snapshot and
/// manifest files are projections and never authorize recovery independently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimCheckpointGeneration {
    schema_version: String,
    authority_kind: String,
    operation_nonce: String,
    active_wal_path: String,
    checkpoint_seq: u64,
    lock_path: String,
    source_wal_path: String,
    source_wal_sha256: String,
    source_wal_byte_len: u64,
    source_wal_last_seq: u64,
    source_wal_valid_record_count: usize,
    source_wal_last_good_offset: u64,
    source_wal_original_len: u64,
    source_wal_repaired: bool,
    source_wal_stop_reason: ClaimWalStopReason,
    snapshot_path: String,
    snapshot_sha256: String,
    snapshot_crc32c: u32,
    snapshot_anchor: ClaimWalFileAnchorBinding,
    snapshot_payload: ClaimWalSnapshotPayload,
    archived_wal_path: String,
    archived_wal_sha256: String,
    archived_wal_byte_len: u64,
    archived_wal_anchor: ClaimWalFileAnchorBinding,
    created_at: String,
    created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalManifestPayload {
    pub schema_version: String,
    pub active_wal_path: String,
    pub snapshot_path: String,
    pub snapshot_crc32c: u32,
    pub archived_wal_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_wal_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_anchor: Option<ClaimWalFileAnchorBinding>,
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
    pub generation_path: Option<PathBuf>,
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

/// Opaque retained authority carried by successful claim recovery.
///
/// Callers may keep or drop this capability, but cannot construct one. Cloning
/// a recovery shares the same exact retained handles instead of reminting them
/// from paths or persisted digests.
#[derive(Clone)]
pub struct ClaimWalRecoveryAuthority {
    inner: Arc<ClaimWalRecoveryAuthorityInner>,
}

impl fmt::Debug for ClaimWalRecoveryAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClaimWalRecoveryAuthority")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Serialize)]
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
    #[doc(hidden)]
    #[serde(skip)]
    pub retained_authority: Option<ClaimWalRecoveryAuthority>,
}

impl PartialEq for ClaimWalRecovery {
    fn eq(&self, other: &Self) -> bool {
        self.wal_path == other.wal_path
            && self.records == other.records
            && self.checkpoint == other.checkpoint
            && self.last_observed_seq == other.last_observed_seq
            && self.valid_record_count == other.valid_record_count
            && self.last_good_offset == other.last_good_offset
            && self.original_len == other.original_len
            && self.repaired == other.repaired
            && self.stop_reason == other.stop_reason
    }
}

impl Eq for ClaimWalRecovery {}

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    CheckpointArchiveInvalid,
    CheckpointGenerationInvalid,
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
pub fn claim_wal_checkpoint_dir(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(CLAIM_WAL_CHECKPOINT_RELATIVE_DIR)
}

#[must_use]
pub fn claim_wal_snapshot_dir(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(CLAIM_WAL_SNAPSHOT_RELATIVE_DIR)
}

#[must_use]
pub fn claim_wal_archive_dir(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(CLAIM_WAL_ARCHIVE_RELATIVE_DIR)
}

/// Crate-private retained claim-WAL authority.
///
/// This guard proves only the inner claim-WAL lock. Backup orchestration must
/// already retain the exact claim-cache `DirLock`; this type deliberately does
/// not encode or imply that outer authority.
pub(crate) struct ClaimWalRetainedLock {
    lock: ClaimWalLock,
    boundary: crate::producer_quiescence::BoundaryLease,
    root: crate::retained_dir::RetainedDirectory,
    lock_identity: crate::retained_dir::RetainedFileIdentity,
    state_root: PathBuf,
    wal_path: PathBuf,
}

impl ClaimWalRetainedLock {
    fn validate(&self, state_root: &Path) -> Result<(), ClaimWalReadError> {
        self.boundary
            .validate_root(state_root)
            .map_err(|source| ClaimWalReadError::ReadWal {
                path: claim_wal_path(state_root),
                source: source.to_string(),
            })?;
        let current = self
            .root
            .open_leaf_read(
                Path::new(CLAIM_WAL_LOCK_RELATIVE_PATH),
                crate::retained_dir::RetainedLeafPolicy::Authority,
            )
            .and_then(|file| crate::retained_dir::RetainedDirectory::identity_of(&file));
        if !current.is_ok_and(|identity| identity == self.lock_identity) {
            return Err(ClaimWalReadError::ReadWal {
                path: claim_wal_lock_path(state_root),
                source: "claim WAL lock identity changed".to_owned(),
            });
        }
        Ok(())
    }

    fn append_wal(
        &self,
        bytes: &[u8],
        durability: crate::WalDurability,
    ) -> Result<(), ClaimWalAppendError> {
        self.validate(&self.state_root)
            .map_err(|source| ClaimWalAppendError::OpenWal {
                path: self.wal_path.clone(),
                source: source.to_string(),
            })?;
        let relative = Path::new(CLAIM_WAL_RELATIVE_PATH);
        let mut file = self
            .root
            .open_read_write(relative)
            .or_else(|source| {
                if source.kind() == io::ErrorKind::NotFound {
                    self.root.open_read_write_create(relative)
                } else {
                    Err(source)
                }
            })
            .map_err(|source| ClaimWalAppendError::OpenWal {
                path: self.wal_path.clone(),
                source: source.to_string(),
            })?;
        let identity =
            crate::retained_dir::RetainedDirectory::identity_of(&file).map_err(|source| {
                ClaimWalAppendError::OpenWal {
                    path: self.wal_path.clone(),
                    source: source.to_string(),
                }
            })?;
        self.root
            .verify_retained_authority_binding(relative, &file, &identity)
            .map_err(|source| ClaimWalAppendError::OpenWal {
                path: self.wal_path.clone(),
                source: source.to_string(),
            })?;
        file.seek(SeekFrom::End(0))
            .and_then(|_| file.write_all(bytes))
            .map_err(|source| ClaimWalAppendError::WriteWal {
                path: self.wal_path.clone(),
                source: source.to_string(),
            })?;
        if let crate::WalDurability::SyncOnAppend = durability {
            file.sync_data()
                .map_err(|source| ClaimWalAppendError::SyncWal {
                    path: self.wal_path.clone(),
                    source: source.to_string(),
                })?;
        }
        self.root
            .verify_retained_authority_binding(relative, &file, &identity)
            .map_err(|source| ClaimWalAppendError::WriteWal {
                path: self.wal_path.clone(),
                source: source.to_string(),
            })?;
        self.validate(&self.state_root)
            .map_err(|source| ClaimWalAppendError::OpenWal {
                path: self.wal_path.clone(),
                source: source.to_string(),
            })
    }
}

/// Acquire and retain only the claim-WAL lock for the exact canonical root.
///
/// The caller remains responsible for acquiring claim-cache authority first.
pub(crate) fn acquire_claim_wal_retained_lock(
    state_root: &Path,
) -> Result<ClaimWalRetainedLock, ClaimWalReadError> {
    let boundary = crate::producer_quiescence::admit_producer(state_root).map_err(|source| {
        ClaimWalReadError::Lock {
            path: claim_wal_lock_path(state_root),
            source: source.to_string(),
        }
    })?;
    acquire_claim_wal_retained_lock_under_boundary(&boundary, state_root)
}

pub(crate) fn acquire_claim_wal_retained_lock_under_boundary(
    boundary: &impl crate::producer_quiescence::ProducerBoundary,
    state_root: &Path,
) -> Result<ClaimWalRetainedLock, ClaimWalReadError> {
    let requested_lock_path = claim_wal_lock_path(state_root);
    acquire_claim_wal_retained_lock_raw_under_boundary(boundary, state_root).map_err(|source| {
        match source.kind() {
            io::ErrorKind::Other => ClaimWalReadError::Lock {
                path: requested_lock_path.clone(),
                source: source.to_string(),
            },
            _ => ClaimWalReadError::OpenLock {
                path: requested_lock_path.clone(),
                source: source.to_string(),
            },
        }
    })
}

/// Recover the exact claim WAL protected by `guard` without reacquiring it.
pub(crate) fn recover_claim_wal_under_retained_lock(
    state_root: &Path,
    guard: &ClaimWalRetainedLock,
    repair: bool,
) -> Result<ClaimWalRecovery, ClaimWalReadError> {
    guard.validate(state_root)?;
    recover_claim_wal_for_guard(guard, repair)
}

fn recover_claim_wal_for_guard(
    guard: &ClaimWalRetainedLock,
    repair: bool,
) -> Result<ClaimWalRecovery, ClaimWalReadError> {
    recover_claim_wal_file_under_lock(guard, repair)
        .and_then(|recovery| recovery.into_recovery(guard))
        .map_err(|source| ClaimWalReadError::ReadWal {
            path: guard.wal_path.clone(),
            source: source.to_string(),
        })
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
    let requested_root = state_root.as_ref();
    let requested_lock_path = claim_wal_lock_path(requested_root);
    let guard =
        acquire_claim_wal_retained_lock_raw(requested_root).map_err(|source| {
            match source.kind() {
                io::ErrorKind::Other => ClaimWalAppendError::Lock {
                    path: requested_lock_path.clone(),
                    source: source.to_string(),
                },
                _ => ClaimWalAppendError::OpenLock {
                    path: requested_lock_path.clone(),
                    source: source.to_string(),
                },
            }
        })?;
    append_claim_wal_record_under_retained_lock(
        &guard,
        operation,
        claim_contract,
        recorded_at,
        durability,
    )
}

fn append_claim_wal_record_under_retained_lock(
    guard: &ClaimWalRetainedLock,
    operation: ClaimWalOperation,
    claim_contract: &ClaimContract,
    recorded_at: &str,
    durability: crate::WalDurability,
) -> Result<ClaimWalAppendResult, ClaimWalAppendError> {
    guard
        .validate(&guard.state_root)
        .map_err(|source| ClaimWalAppendError::CreateDir {
            path: guard.state_root.join("wal"),
            source: source.to_string(),
        })?;
    guard
        .root
        .create_dir_all(Path::new("wal"))
        .map_err(|source| ClaimWalAppendError::CreateDir {
            path: guard.state_root.join("wal"),
            source: source.to_string(),
        })?;
    let (mut recovery, recovery_elapsed_ms) = recover_appendable_wal(guard)?;
    let last_seq = recovery.recovery.last_observed_seq;
    let seq = last_seq
        .checked_add(1)
        .ok_or(ClaimWalAppendError::SequenceOverflow { last_seq })?;
    let payload_bytes = lifecycle_payload_bytes(operation, claim_contract, recorded_at)?;
    let record_bytes = encode_record(seq, operation.record_type(), &payload_bytes)?;

    recovery
        .validate(guard)
        .map_err(|source| ClaimWalAppendError::ReadWal {
            path: guard.wal_path.clone(),
            source: source.to_string(),
        })?;
    guard.append_wal(&record_bytes, durability)?;
    recovery
        .observe_append(guard, &record_bytes)
        .map_err(|source| ClaimWalAppendError::WriteWal {
            path: guard.wal_path.clone(),
            source: source.to_string(),
        })?;
    let rotated = maybe_rotate_after_append(
        guard,
        &recovery,
        record_bytes.len(),
        recovery_elapsed_ms,
        recorded_at,
    )?;
    if !rotated {
        recovery
            .validate(guard)
            .map_err(|source| ClaimWalAppendError::WriteWal {
                path: guard.wal_path.clone(),
                source: source.to_string(),
            })?;
    }

    Ok(ClaimWalAppendResult {
        wal_path: guard.wal_path.clone(),
        seq,
        bytes_appended: u64::try_from(record_bytes.len()).unwrap_or(u64::MAX),
    })
}

fn recover_appendable_wal(
    guard: &ClaimWalRetainedLock,
) -> Result<(RetainedClaimWalRecovery, u64), ClaimWalAppendError> {
    let recovery_started_at = Instant::now();
    let recovery = recover_claim_wal_file_under_lock(guard, true).map_err(|source| {
        ClaimWalAppendError::ReadWal {
            path: guard.wal_path.clone(),
            source: source.to_string(),
        }
    })?;
    let recovery_elapsed_ms = elapsed_millis_u64(recovery_started_at);
    ensure_recovered_appendable(&recovery.recovery)?;
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

fn maybe_rotate_after_append(
    guard: &ClaimWalRetainedLock,
    recovery: &RetainedClaimWalRecovery,
    record_byte_len: usize,
    recovery_elapsed_ms: u64,
    recorded_at: &str,
) -> Result<bool, ClaimWalAppendError> {
    let post_append_len = recovery
        .recovery
        .last_good_offset
        .saturating_add(u64::try_from(record_byte_len).unwrap_or(u64::MAX));
    let post_append_records = recovery.recovery.valid_record_count.saturating_add(1);
    let Some(reason) = rotation_reason(
        post_append_len,
        post_append_records,
        recovery_elapsed_ms,
        &ClaimWalRotationOptions::default(),
    ) else {
        return Ok(false);
    };
    let rotation_recovery = recover_claim_wal_file_under_lock(guard, false).map_err(|source| {
        ClaimWalAppendError::RotateWal {
            path: guard.wal_path.clone(),
            source: source.to_string(),
        }
    })?;
    rotate_claim_wal_under_lock(guard, &rotation_recovery, recorded_at, reason).map_err(
        |source| ClaimWalAppendError::RotateWal {
            path: guard.wal_path.clone(),
            source: source.to_string(),
        },
    )?;
    Ok(true)
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
    let guard = acquire_claim_wal_retained_lock(state_root)?;
    recover_claim_wal_under_retained_lock(state_root, &guard, repair)
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
    let requested_root = state_root.as_ref();
    let requested_lock_path = claim_wal_lock_path(requested_root);
    let guard =
        acquire_claim_wal_retained_lock_raw(requested_root).map_err(|source| {
            match source.kind() {
                io::ErrorKind::Other => ClaimWalRotationError::Lock {
                    path: requested_lock_path.clone(),
                    source: source.to_string(),
                },
                _ => ClaimWalRotationError::OpenLock {
                    path: requested_lock_path.clone(),
                    source: source.to_string(),
                },
            }
        })?;
    rotate_claim_wal_if_needed_under_retained_lock(&guard, created_at, options)
}

fn rotate_claim_wal_if_needed_under_retained_lock(
    guard: &ClaimWalRetainedLock,
    created_at: &str,
    options: &ClaimWalRotationOptions,
) -> Result<ClaimWalRotationResult, ClaimWalRotationError> {
    let recovery_started_at = Instant::now();
    let recovery = recover_claim_wal_file_under_lock(guard, true).map_err(|source| {
        ClaimWalRotationError::RecoverWal {
            path: guard.wal_path.clone(),
            source: source.to_string(),
        }
    })?;
    let recovery_elapsed_ms = elapsed_millis_u64(recovery_started_at);
    ensure_recovered_rotatable(&recovery.recovery)?;
    let Some(reason) = rotation_reason(
        recovery.recovery.original_len,
        recovery.recovery.valid_record_count,
        recovery_elapsed_ms,
        options,
    ) else {
        let result = ClaimWalRotationResult {
            rotated: false,
            reason: None,
            wal_path: guard.wal_path.clone(),
            snapshot_path: None,
            archived_wal_path: None,
            generation_path: None,
            manifest_path: None,
            checkpoint_seq: None,
            last_seq_in_snapshot: recovery.recovery.last_observed_seq,
            compacted_records: recovery.recovery.valid_record_count,
        };
        recovery
            .validate(guard)
            .map_err(|source| ClaimWalRotationError::RecoverWal {
                path: guard.wal_path.clone(),
                source: source.to_string(),
            })?;
        return Ok(result);
    };
    rotate_claim_wal_under_lock(guard, &recovery, created_at, reason)
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
    guard: &ClaimWalRetainedLock,
    recovery: &RetainedClaimWalRecovery,
    created_at: &str,
    reason: ClaimWalRotationReason,
) -> Result<ClaimWalRotationResult, ClaimWalRotationError> {
    ensure_recovered_rotatable(&recovery.recovery)?;
    recovery
        .validate(guard)
        .map_err(|source| ClaimWalRotationError::RecoverWal {
            path: guard.wal_path.clone(),
            source: source.to_string(),
        })?;
    let projection = project_claim_wal_recovery(recovery.recovery.clone());
    let last_seq_in_snapshot = recovery.recovery.last_observed_seq;
    let checkpoint_seq = next_checkpoint_seq(last_seq_in_snapshot)?;
    let created_at_ms = now_unix_millis();
    let snapshot = write_snapshot_file(
        guard,
        recovery,
        &projection,
        last_seq_in_snapshot,
        created_at,
        created_at_ms,
    )?;
    let archive = copy_active_wal_to_archive_retained(guard, recovery, checkpoint_seq)?;
    let generation = write_checkpoint_generation(
        guard,
        recovery,
        &snapshot,
        &archive,
        checkpoint_seq,
        created_at,
        created_at_ms,
    )?;
    let manifest_bytes = rotation_manifest_bytes(&generation)?;
    ensure_rotation_manifest(
        guard,
        recovery,
        &snapshot,
        &archive,
        &generation,
        &manifest_bytes,
    )?;
    let new_wal_bytes = checkpoint_record_bytes(&generation)?;
    replace_active_wal_with_checkpoint(
        guard,
        recovery,
        &snapshot,
        &archive,
        &generation,
        &new_wal_bytes,
    )?;
    // Linearization point: one atomic replacement installs the active WAL's
    // immutable generation-binding checkpoint record. Snapshot, archive, and
    // legacy manifest are projections of that one content-addressed generation;
    // none can authorize recovery independently.
    Ok(ClaimWalRotationResult {
        rotated: true,
        reason: Some(reason),
        wal_path: guard.wal_path.clone(),
        snapshot_path: Some(snapshot.absolute_path.clone()),
        archived_wal_path: Some(archive.absolute_path.clone()),
        generation_path: Some(generation.absolute_path.clone()),
        manifest_path: Some(claim_wal_manifest_path(&guard.state_root)),
        checkpoint_seq: Some(checkpoint_seq),
        last_seq_in_snapshot,
        compacted_records: recovery.recovery.valid_record_count,
    })
}

fn next_checkpoint_seq(last_seq: u64) -> Result<u64, ClaimWalRotationError> {
    last_seq
        .checked_add(1)
        .ok_or(ClaimWalRotationError::SequenceOverflow { last_seq })
}

fn write_snapshot_file(
    guard: &ClaimWalRetainedLock,
    recovery: &RetainedClaimWalRecovery,
    projection: &ClaimWalProjection,
    last_seq: u64,
    created_at: &str,
    created_at_ms: u64,
) -> Result<RetainedClaimWalSnapshot, ClaimWalRotationError> {
    let document = ClaimWalSnapshotPayload {
        schema_version: "0.1".to_string(),
        created_at: created_at.to_string(),
        created_at_ms,
        last_seq,
        latest_claims: snapshot_claims_from_projection(projection),
    };
    let bytes = serde_json_canonicalizer::to_vec(&document).map_err(|source| {
        ClaimWalRotationError::SerializeSnapshot {
            source: source.to_string(),
        }
    })?;
    let digest = crate::sha256_content_hash(&bytes);
    let relative_path = snapshot_relative_path(last_seq, &digest).ok_or_else(|| {
        ClaimWalRotationError::SerializeSnapshot {
            source: "snapshot digest is not canonical SHA-256".to_owned(),
        }
    })?;
    let absolute_path = guard.state_root.join(&relative_path);
    let leaf = publish_immutable_claim_leaf(guard, &relative_path, &bytes)
        .map_err(|source| map_rotation_io_error(&absolute_path, &source))?;
    let anchor = retain_claim_leaf_anchor(
        guard,
        Path::new(CLAIM_SNAPSHOT_ANCHOR_RELATIVE_DIR),
        &leaf,
        &digest,
    )
    .map_err(|source| map_rotation_io_error(&absolute_path, &source))?;
    recovery
        .validate_for_rotation(guard)
        .and_then(|()| leaf.validate(&guard.root))
        .and_then(|()| anchor.validate_retained_file(&leaf.file, &leaf.identity))
        .map_err(|source| ClaimWalRotationError::VerifyWal {
            path: absolute_path.clone(),
            source: source.to_string(),
        })?;
    Ok(RetainedClaimWalSnapshot {
        relative_path,
        absolute_path,
        leaf,
        anchor,
        document,
        digest,
        crc32c: crc32c::crc32c(&bytes),
    })
}

fn write_checkpoint_generation(
    guard: &ClaimWalRetainedLock,
    recovery: &RetainedClaimWalRecovery,
    snapshot: &RetainedClaimWalSnapshot,
    archive: &RetainedClaimWalArchive,
    checkpoint_seq: u64,
    created_at: &str,
    created_at_ms: u64,
) -> Result<RetainedClaimCheckpointGeneration, ClaimWalRotationError> {
    let source_wal =
        recovery
            .active_wal
            .as_ref()
            .ok_or_else(|| ClaimWalRotationError::RecoverWal {
                path: guard.wal_path.clone(),
                source: "rotation requires one exact retained active WAL".to_owned(),
            })?;
    let document = ClaimCheckpointGeneration {
        schema_version: CLAIM_CHECKPOINT_GENERATION_SCHEMA_VERSION.to_owned(),
        authority_kind: CLAIM_CHECKPOINT_AUTHORITY_KIND.to_owned(),
        operation_nonce: claim_checkpoint_operation_nonce().map_err(|source| {
            ClaimWalRotationError::SerializeCheckpoint {
                source: source.to_string(),
            }
        })?,
        active_wal_path: CLAIM_WAL_RELATIVE_PATH.to_owned(),
        checkpoint_seq,
        lock_path: CLAIM_WAL_LOCK_RELATIVE_PATH.to_owned(),
        source_wal_path: CLAIM_WAL_RELATIVE_PATH.to_owned(),
        source_wal_sha256: crate::sha256_content_hash(&source_wal.bytes),
        source_wal_byte_len: u64::try_from(source_wal.bytes.len()).unwrap_or(u64::MAX),
        source_wal_last_seq: recovery.recovery.last_observed_seq,
        source_wal_valid_record_count: recovery.recovery.valid_record_count,
        source_wal_last_good_offset: recovery.recovery.last_good_offset,
        source_wal_original_len: recovery.recovery.original_len,
        source_wal_repaired: recovery.recovery.repaired,
        source_wal_stop_reason: recovery.recovery.stop_reason,
        snapshot_path: path_to_wal_string(&snapshot.relative_path),
        snapshot_sha256: snapshot.digest.clone(),
        snapshot_crc32c: snapshot.crc32c,
        snapshot_anchor: ClaimWalFileAnchorBinding::from_retained(snapshot.anchor.binding()),
        snapshot_payload: snapshot.document.clone(),
        archived_wal_path: path_to_wal_string(&archive.relative_path),
        archived_wal_sha256: archive.digest.clone(),
        archived_wal_byte_len: u64::try_from(archive.bytes.len()).unwrap_or(u64::MAX),
        archived_wal_anchor: ClaimWalFileAnchorBinding::from_retained(archive.anchor.binding()),
        created_at: created_at.to_owned(),
        created_at_ms,
    };
    let bytes = serde_json_canonicalizer::to_vec(&document).map_err(|source| {
        ClaimWalRotationError::SerializeCheckpoint {
            source: source.to_string(),
        }
    })?;
    let digest = crate::sha256_content_hash(&bytes);
    let relative_path = generation_relative_path(checkpoint_seq, &digest).ok_or_else(|| {
        ClaimWalRotationError::SerializeCheckpoint {
            source: "checkpoint generation digest is not canonical SHA-256".to_owned(),
        }
    })?;
    let absolute_path = guard.state_root.join(&relative_path);
    let leaf = publish_immutable_claim_leaf(guard, &relative_path, &bytes)
        .map_err(|source| map_rotation_io_error(&absolute_path, &source))?;
    let anchor = retain_claim_leaf_anchor(
        guard,
        Path::new(CLAIM_GENERATION_ANCHOR_RELATIVE_DIR),
        &leaf,
        &digest,
    )
    .map_err(|source| map_rotation_io_error(&absolute_path, &source))?;
    let generation = RetainedClaimCheckpointGeneration {
        relative_path,
        absolute_path,
        leaf,
        anchor,
        document,
        digest,
    };
    recovery
        .validate_for_rotation(guard)
        .and_then(|()| snapshot.validate(guard))
        .and_then(|()| archive.validate(guard))
        .and_then(|()| generation.validate(guard))
        .map_err(|source| ClaimWalRotationError::VerifyWal {
            path: generation.absolute_path.clone(),
            source: source.to_string(),
        })?;
    Ok(generation)
}

fn checkpoint_record_bytes(
    generation: &RetainedClaimCheckpointGeneration,
) -> Result<Vec<u8>, ClaimWalRotationError> {
    let checkpoint_payload = ClaimWalCheckpointPayload {
        schema_version: CLAIM_CHECKPOINT_PAYLOAD_SCHEMA_VERSION.to_owned(),
        snapshot_path: generation.document.snapshot_path.clone(),
        snapshot_crc32c: generation.document.snapshot_crc32c,
        last_seq_in_snapshot: generation.document.source_wal_last_seq,
        archived_wal_path: Some(generation.document.archived_wal_path.clone()),
        archived_wal_sha256: Some(generation.document.archived_wal_sha256.clone()),
        generation_path: Some(path_to_wal_string(&generation.relative_path)),
        generation_sha256: Some(generation.digest.clone()),
        generation_anchor: Some(ClaimWalFileAnchorBinding::from_retained(
            generation.anchor.binding(),
        )),
        created_at: generation.document.created_at.clone(),
        created_at_ms: generation.document.created_at_ms,
    };
    let checkpoint_bytes =
        serde_json_canonicalizer::to_vec(&checkpoint_payload).map_err(|source| {
            ClaimWalRotationError::SerializeCheckpoint {
                source: source.to_string(),
            }
        })?;
    encode_record(
        generation.document.checkpoint_seq,
        RECORD_TYPE_CHECKPOINT_REF,
        &checkpoint_bytes,
    )
    .map_err(|source| ClaimWalRotationError::SerializeCheckpoint {
        source: source.to_string(),
    })
}

fn replace_active_wal_with_checkpoint(
    guard: &ClaimWalRetainedLock,
    recovery: &RetainedClaimWalRecovery,
    snapshot: &RetainedClaimWalSnapshot,
    archive: &RetainedClaimWalArchive,
    generation: &RetainedClaimCheckpointGeneration,
    new_wal_bytes: &[u8],
) -> Result<(), ClaimWalRotationError> {
    let source_wal =
        recovery
            .active_wal
            .as_ref()
            .ok_or_else(|| ClaimWalRotationError::RecoverWal {
                path: guard.wal_path.clone(),
                source: "rotation lost its exact retained active WAL".to_owned(),
            })?;
    write_durable_replaced_file_retained_from_expected_with_commit(
        guard,
        Path::new(CLAIM_WAL_RELATIVE_PATH),
        source_wal,
        new_wal_bytes,
        || {
            recovery.validate_after_active_replacement(guard)?;
            snapshot.validate(guard)?;
            archive.validate(guard)?;
            generation.validate(guard)?;
            verify_rotated_wal(guard).map_err(|source| io::Error::other(source.to_string()))
        },
    )
    .map_err(|source| map_rotation_io_error(&guard.wal_path, &source))
}

fn verify_rotated_wal(guard: &ClaimWalRetainedLock) -> Result<(), ClaimWalRotationError> {
    let verification = recover_claim_wal_file_under_lock(guard, false).map_err(|source| {
        ClaimWalRotationError::VerifyWal {
            path: guard.wal_path.clone(),
            source: source.to_string(),
        }
    })?;
    if verification.recovery.stop_reason != ClaimWalStopReason::CleanEof {
        return Err(ClaimWalRotationError::VerifyWal {
            path: guard.wal_path.clone(),
            source: format!(
                "rotated WAL recovery stopped with {:?}",
                verification.recovery.stop_reason
            ),
        });
    }
    verification
        .into_recovery(guard)
        .map(|_| ())
        .map_err(|source| ClaimWalRotationError::VerifyWal {
            path: guard.wal_path.clone(),
            source: source.to_string(),
        })
}

fn rotation_manifest_bytes(
    generation: &RetainedClaimCheckpointGeneration,
) -> Result<Vec<u8>, ClaimWalRotationError> {
    let manifest = ClaimWalManifestPayload {
        schema_version: CLAIM_CHECKPOINT_PAYLOAD_SCHEMA_VERSION.to_owned(),
        active_wal_path: CLAIM_WAL_RELATIVE_PATH.to_owned(),
        snapshot_path: generation.document.snapshot_path.clone(),
        snapshot_crc32c: generation.document.snapshot_crc32c,
        archived_wal_path: generation.document.archived_wal_path.clone(),
        archived_wal_sha256: Some(generation.document.archived_wal_sha256.clone()),
        generation_path: Some(path_to_wal_string(&generation.relative_path)),
        generation_sha256: Some(generation.digest.clone()),
        generation_anchor: Some(ClaimWalFileAnchorBinding::from_retained(
            generation.anchor.binding(),
        )),
        checkpoint_seq: generation.document.checkpoint_seq,
        last_seq_in_snapshot: generation.document.source_wal_last_seq,
        updated_at: generation.document.created_at.clone(),
        updated_at_ms: generation.document.created_at_ms,
    };
    serde_json_canonicalizer::to_vec(&manifest).map_err(|source| {
        ClaimWalRotationError::SerializeManifest {
            source: source.to_string(),
        }
    })
}

fn ensure_rotation_manifest(
    guard: &ClaimWalRetainedLock,
    recovery: &RetainedClaimWalRecovery,
    snapshot: &RetainedClaimWalSnapshot,
    archive: &RetainedClaimWalArchive,
    generation: &RetainedClaimCheckpointGeneration,
    manifest_bytes: &[u8],
) -> Result<(), ClaimWalRotationError> {
    let manifest_relative = Path::new(CLAIM_WAL_MANIFEST_RELATIVE_PATH);
    write_durable_replaced_file_retained_with_commit(
        guard,
        manifest_relative,
        manifest_bytes,
        || {
            recovery.validate_for_rotation(guard)?;
            snapshot.validate(guard)?;
            archive.validate(guard)?;
            generation.validate(guard)
        },
    )
    .map_err(|source| map_rotation_io_error(&claim_wal_manifest_path(&guard.state_root), &source))
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

struct RetainedClaimWalLeaf {
    relative_path: PathBuf,
    file: File,
    identity: crate::retained_dir::RetainedFileIdentity,
    bytes: Vec<u8>,
}

impl RetainedClaimWalLeaf {
    fn validate(&self, root: &crate::retained_dir::RetainedDirectory) -> io::Result<()> {
        validate_retained_claim_leaf(
            root,
            &self.relative_path,
            &self.file,
            &self.identity,
            &self.bytes,
        )
    }

    fn validate_handle(&self) -> io::Result<()> {
        validate_retained_claim_handle(&self.file, &self.identity, &self.bytes)
    }
}

struct RetainedClaimWalSnapshot {
    relative_path: PathBuf,
    absolute_path: PathBuf,
    leaf: RetainedClaimWalLeaf,
    anchor: crate::retained_dir::RetainedFileLifetimeAnchor,
    document: ClaimWalSnapshotPayload,
    digest: String,
    crc32c: u32,
}

impl RetainedClaimWalSnapshot {
    fn validate(&self, guard: &ClaimWalRetainedLock) -> io::Result<()> {
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))?;
        if self.leaf.relative_path != self.relative_path
            || crate::sha256_content_hash(&self.leaf.bytes) != self.digest
            || crc32c::crc32c(&self.leaf.bytes) != self.crc32c
            || serde_json_canonicalizer::to_vec(&self.document)
                .map_err(|source| io::Error::new(io::ErrorKind::InvalidData, source))?
                != self.leaf.bytes
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained claim snapshot no longer matches its canonical payload",
            ));
        }
        self.leaf.validate(&guard.root)?;
        self.anchor
            .validate_retained_file(&self.leaf.file, &self.leaf.identity)?;
        self.leaf.validate(&guard.root)
    }
}

#[derive(Debug)]
struct RetainedClaimWalArchive {
    relative_path: PathBuf,
    absolute_path: PathBuf,
    file: File,
    identity: crate::retained_dir::RetainedFileIdentity,
    bytes: Vec<u8>,
    digest: String,
    anchor: crate::retained_dir::RetainedFileLifetimeAnchor,
}

impl RetainedClaimWalArchive {
    fn validate(&self, guard: &ClaimWalRetainedLock) -> io::Result<()> {
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))?;
        validate_retained_claim_leaf(
            &guard.root,
            &self.relative_path,
            &self.file,
            &self.identity,
            &self.bytes,
        )?;
        if crate::sha256_content_hash(&self.bytes) != self.digest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained claim WAL archive digest changed",
            ));
        }
        self.anchor
            .validate_retained_file(&self.file, &self.identity)?;
        validate_retained_claim_leaf(
            &guard.root,
            &self.relative_path,
            &self.file,
            &self.identity,
            &self.bytes,
        )?;
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))
    }
}

struct RetainedClaimCheckpointGeneration {
    relative_path: PathBuf,
    absolute_path: PathBuf,
    leaf: RetainedClaimWalLeaf,
    anchor: crate::retained_dir::RetainedFileLifetimeAnchor,
    document: ClaimCheckpointGeneration,
    digest: String,
}

impl RetainedClaimCheckpointGeneration {
    fn validate(&self, guard: &ClaimWalRetainedLock) -> io::Result<()> {
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))?;
        if self.leaf.relative_path != self.relative_path
            || crate::sha256_content_hash(&self.leaf.bytes) != self.digest
            || serde_json_canonicalizer::to_vec(&self.document)
                .map_err(|source| io::Error::new(io::ErrorKind::InvalidData, source))?
                != self.leaf.bytes
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained claim checkpoint generation changed",
            ));
        }
        self.leaf.validate(&guard.root)?;
        self.anchor
            .validate_retained_file(&self.leaf.file, &self.leaf.identity)?;
        self.leaf.validate(&guard.root)
    }
}

struct RetainedCheckpointWitnessBundle {
    root: crate::retained_dir::RetainedDirectory,
    root_identity: crate::retained_dir::RetainedFileIdentity,
    lock: RetainedClaimWalLeaf,
    active_wal: RetainedClaimWalLeaf,
    generation: RetainedClaimWalLeaf,
    generation_anchor: crate::retained_dir::RetainedFileLifetimeAnchor,
    snapshot: RetainedClaimWalLeaf,
    snapshot_anchor: crate::retained_dir::RetainedFileLifetimeAnchor,
    archive: RetainedClaimWalLeaf,
    archive_anchor: crate::retained_dir::RetainedFileLifetimeAnchor,
}

impl RetainedCheckpointWitnessBundle {
    fn validate(&self, guard: &ClaimWalRetainedLock) -> io::Result<()> {
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))?;
        if guard.root.identity()? != self.root_identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained claim checkpoint guard root identity changed",
            ));
        }
        self.validate_retained()?;
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))
    }

    fn validate_retained(&self) -> io::Result<()> {
        self.validate_retained_root_and_lock()?;
        for leaf in [
            &self.active_wal,
            &self.generation,
            &self.snapshot,
            &self.archive,
        ] {
            leaf.validate(&self.root)?;
        }
        self.generation_anchor
            .validate_retained_file(&self.generation.file, &self.generation.identity)?;
        self.snapshot_anchor
            .validate_retained_file(&self.snapshot.file, &self.snapshot.identity)?;
        self.archive_anchor
            .validate_retained_file(&self.archive.file, &self.archive.identity)?;
        self.validate_retained_root_and_lock()
    }

    fn validate_after_active_replacement(&self, guard: &ClaimWalRetainedLock) -> io::Result<()> {
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))?;
        self.validate_retained_root_and_lock()?;
        self.active_wal.validate_handle()?;
        for leaf in [&self.generation, &self.snapshot, &self.archive] {
            leaf.validate(&self.root)?;
        }
        self.generation_anchor
            .validate_retained_file(&self.generation.file, &self.generation.identity)?;
        self.snapshot_anchor
            .validate_retained_file(&self.snapshot.file, &self.snapshot.identity)?;
        self.archive_anchor
            .validate_retained_file(&self.archive.file, &self.archive.identity)?;
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))
    }

    fn validate_retained_root_and_lock(&self) -> io::Result<()> {
        if self.root.identity()? != self.root_identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained claim checkpoint root identity changed",
            ));
        }
        self.lock.validate(&self.root)
    }

    fn update_active_wal_bytes(&mut self, bytes: &[u8]) {
        self.active_wal.bytes.clear();
        self.active_wal.bytes.extend_from_slice(bytes);
    }
}

struct RetainedPristineClaimAuthority {
    root: crate::retained_dir::RetainedDirectory,
    root_identity: crate::retained_dir::RetainedFileIdentity,
    lock: RetainedClaimWalLeaf,
}

impl RetainedPristineClaimAuthority {
    fn validate(
        &self,
        guard: &ClaimWalRetainedLock,
        active_wal: Option<&RetainedClaimWalLeaf>,
    ) -> io::Result<()> {
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))?;
        if guard.root.identity()? != self.root_identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pristine claim authority guard root identity changed",
            ));
        }
        self.validate_retained(active_wal)?;
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))
    }

    fn validate_for_rotation(
        &self,
        guard: &ClaimWalRetainedLock,
        active_wal: Option<&RetainedClaimWalLeaf>,
    ) -> io::Result<()> {
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))?;
        if self.root.identity()? != self.root_identity
            || guard.root.identity()? != self.root_identity
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pristine claim rotation root identity changed",
            ));
        }
        self.lock.validate(&self.root)?;
        let active_wal = active_wal.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "claim WAL rotation requires an exact retained active selector",
            )
        })?;
        active_wal.validate(&self.root)?;
        self.lock.validate(&self.root)
    }

    fn validate_after_active_replacement(
        &self,
        guard: &ClaimWalRetainedLock,
        active_wal: Option<&RetainedClaimWalLeaf>,
    ) -> io::Result<()> {
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))?;
        if self.root.identity()? != self.root_identity
            || guard.root.identity()? != self.root_identity
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pristine claim replacement root identity changed",
            ));
        }
        self.lock.validate(&self.root)?;
        active_wal
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "claim WAL replacement lost its exact predecessor",
                )
            })?
            .validate_handle()?;
        self.lock.validate(&self.root)
    }

    fn validate_retained(&self, active_wal: Option<&RetainedClaimWalLeaf>) -> io::Result<()> {
        if self.root.identity()? != self.root_identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pristine claim authority root identity changed",
            ));
        }
        self.lock.validate(&self.root)?;
        if let Some(active_wal) = active_wal {
            active_wal.validate(&self.root)?;
        } else {
            require_missing_claim_leaf(&self.root, Path::new(CLAIM_WAL_RELATIVE_PATH))?;
        }
        require_pristine_claim_checkpoint_namespace(&self.root)?;
        self.lock.validate(&self.root)
    }
}

enum ClaimWalRecoveryAuthorityInner {
    Checkpoint(RetainedCheckpointWitnessBundle),
    Pristine {
        authority: RetainedPristineClaimAuthority,
        active_wal: Option<RetainedClaimWalLeaf>,
    },
}

impl ClaimWalRecoveryAuthority {
    pub(crate) fn revalidate(&self) -> io::Result<()> {
        match self.inner.as_ref() {
            ClaimWalRecoveryAuthorityInner::Checkpoint(bundle) => bundle.validate_retained(),
            ClaimWalRecoveryAuthorityInner::Pristine {
                authority,
                active_wal,
            } => authority.validate_retained(active_wal.as_ref()),
        }
    }
}

fn copy_active_wal_to_archive_retained(
    guard: &ClaimWalRetainedLock,
    recovery: &RetainedClaimWalRecovery,
    checkpoint_seq: u64,
) -> Result<RetainedClaimWalArchive, ClaimWalRotationError> {
    recovery
        .validate_for_rotation(guard)
        .map_err(|source| ClaimWalRotationError::CopyFile {
            from: guard.wal_path.clone(),
            to: claim_wal_archive_dir(&guard.state_root),
            source: source.to_string(),
        })?;
    let source = recovery
        .active_wal
        .as_ref()
        .ok_or_else(|| ClaimWalRotationError::CopyFile {
            from: guard.wal_path.clone(),
            to: claim_wal_archive_dir(&guard.state_root),
            source: "rotation requires the exact retained active WAL handle and bytes".to_owned(),
        })?;
    let digest = crate::sha256_content_hash(&source.bytes);
    let archive_path = archive_relative_path(checkpoint_seq, &digest).ok_or_else(|| {
        ClaimWalRotationError::CopyFile {
            from: guard.wal_path.clone(),
            to: claim_wal_archive_dir(&guard.state_root),
            source: "active WAL digest is not canonical SHA-256".to_owned(),
        }
    })?;
    let archive_abs_path = guard.state_root.join(&archive_path);
    let leaf =
        publish_immutable_claim_leaf(guard, &archive_path, &source.bytes).map_err(|source| {
            ClaimWalRotationError::CopyFile {
                from: guard.wal_path.clone(),
                to: archive_abs_path.clone(),
                source: source.to_string(),
            }
        })?;
    let anchor = retain_claim_leaf_anchor(
        guard,
        Path::new(CLAIM_ARCHIVE_ANCHOR_RELATIVE_DIR),
        &leaf,
        &digest,
    )
    .map_err(|source| ClaimWalRotationError::CopyFile {
        from: guard.wal_path.clone(),
        to: archive_abs_path.clone(),
        source: source.to_string(),
    })?;
    let archive = RetainedClaimWalArchive {
        relative_path: archive_path,
        absolute_path: archive_abs_path,
        identity: leaf.identity,
        file: leaf.file,
        digest,
        bytes: leaf.bytes,
        anchor,
    };
    recovery
        .validate_for_rotation(guard)
        .and_then(|()| archive.validate(guard))
        .map_err(|source| ClaimWalRotationError::VerifyWal {
            path: archive.absolute_path.clone(),
            source: source.to_string(),
        })?;
    Ok(archive)
}

fn publish_immutable_claim_leaf(
    guard: &ClaimWalRetainedLock,
    relative_path: &Path,
    bytes: &[u8],
) -> io::Result<RetainedClaimWalLeaf> {
    let maximum = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    match retain_claim_wal_leaf(&guard.root, relative_path, maximum) {
        Ok(existing) if existing.bytes == bytes => return Ok(existing),
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "content-addressed claim checkpoint path contains different bytes",
            ));
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => return Err(source),
    }

    let parent = relative_path.parent().unwrap_or_else(|| Path::new(""));
    let staging_path = immutable_claim_staging_path(parent)?;
    let mut file = guard.root.open_write_new(&staging_path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    if !parent.as_os_str().is_empty() {
        guard.root.sync_directory(parent)?;
    }
    let identity = crate::retained_dir::RetainedDirectory::identity_of(&file)?;
    validate_retained_claim_leaf(&guard.root, &staging_path, &file, &identity, bytes)?;
    let authority = guard.root.retain_authority()?;
    authority.publish_retained_handle_noreplace(&file, &identity, relative_path)?;
    let retained = RetainedClaimWalLeaf {
        relative_path: relative_path.to_path_buf(),
        file,
        identity,
        bytes: bytes.to_vec(),
    };
    retained.validate(&guard.root)?;
    Ok(retained)
}

fn retain_claim_leaf_anchor(
    guard: &ClaimWalRetainedLock,
    anchor_directory: &Path,
    leaf: &RetainedClaimWalLeaf,
    digest: &str,
) -> io::Result<crate::retained_dir::RetainedFileLifetimeAnchor> {
    leaf.validate(&guard.root)?;
    let anchor = guard.root.retain_file_lifetime_anchor(
        anchor_directory,
        &leaf.file,
        &leaf.identity,
        digest,
        u64::try_from(leaf.bytes.len()).unwrap_or(u64::MAX),
    )?;
    anchor.validate_retained_file(&leaf.file, &leaf.identity)?;
    leaf.validate(&guard.root)?;
    Ok(anchor)
}

fn immutable_claim_staging_path(parent: &Path) -> io::Result<PathBuf> {
    let mut nonce = [0_u8; 16];
    getrandom::fill(&mut nonce).map_err(|error| {
        io::Error::other(format!(
            "claim checkpoint staging nonce generation failed: {error}"
        ))
    })?;
    Ok(parent.join(format!(
        ".claim-checkpoint-{}-{:032x}.quarantine",
        std::process::id(),
        u128::from_le_bytes(nonce)
    )))
}

fn retain_claim_wal_leaf(
    root: &crate::retained_dir::RetainedDirectory,
    relative_path: &Path,
    maximum: u64,
) -> io::Result<RetainedClaimWalLeaf> {
    let mut file = root.open_leaf_read(
        relative_path,
        crate::retained_dir::RetainedLeafPolicy::Authority,
    )?;
    let identity = crate::retained_dir::RetainedDirectory::identity_of(&file)?;
    let before = file.metadata()?;
    if before.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "retained claim WAL leaf exceeds its byte limit",
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum
        || after.len() != before.len()
        || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained claim WAL leaf changed while it was read",
        ));
    }
    let retained = RetainedClaimWalLeaf {
        relative_path: relative_path.to_path_buf(),
        file,
        identity,
        bytes,
    };
    validate_retained_claim_leaf(
        root,
        &retained.relative_path,
        &retained.file,
        &retained.identity,
        &retained.bytes,
    )?;
    Ok(retained)
}

fn claim_anchor_binding_matches(
    binding: &ClaimWalFileAnchorBinding,
    expected_directory: &Path,
    expected_digest: &str,
    expected_byte_length: u64,
) -> bool {
    let Some(path) = safe_relative_path(&binding.anchor_relative_path) else {
        return false;
    };
    path.parent() == Some(expected_directory)
        && binding.content_digest == expected_digest
        && binding.byte_length == expected_byte_length
}

fn retain_anchored_claim_wal_leaf(
    root: &crate::retained_dir::RetainedDirectory,
    target_relative_path: &Path,
    expected_anchor_directory: &Path,
    binding: &ClaimWalFileAnchorBinding,
    expected_digest: &str,
    maximum: u64,
) -> io::Result<(
    RetainedClaimWalLeaf,
    crate::retained_dir::RetainedFileLifetimeAnchor,
)> {
    if binding.byte_length > maximum
        || !claim_anchor_binding_matches(
            binding,
            expected_anchor_directory,
            expected_digest,
            binding.byte_length,
        )
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "claim checkpoint anchor binding does not match its selected leaf",
        ));
    }
    let anchor = root.open_file_lifetime_anchor(&binding.to_retained())?;
    let (file, _identity) = anchor.retain_target(root, target_relative_path)?;
    let leaf = retain_open_claim_wal_leaf(root, target_relative_path, file, maximum)?;
    if u64::try_from(leaf.bytes.len()).unwrap_or(u64::MAX) != binding.byte_length
        || crate::sha256_content_hash(&leaf.bytes) != expected_digest
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "anchored claim checkpoint leaf changed content",
        ));
    }
    anchor.validate_retained_file(&leaf.file, &leaf.identity)?;
    leaf.validate(root)?;
    Ok((leaf, anchor))
}

fn clone_retained_claim_leaf(
    root: &crate::retained_dir::RetainedDirectory,
    retained: &RetainedClaimWalLeaf,
) -> io::Result<RetainedClaimWalLeaf> {
    let cloned = RetainedClaimWalLeaf {
        relative_path: retained.relative_path.clone(),
        file: retained.file.try_clone()?,
        identity: retained.identity.clone(),
        bytes: retained.bytes.clone(),
    };
    validate_retained_claim_leaf(
        root,
        &cloned.relative_path,
        &cloned.file,
        &cloned.identity,
        &cloned.bytes,
    )?;
    Ok(cloned)
}

fn retain_open_claim_wal_leaf(
    root: &crate::retained_dir::RetainedDirectory,
    relative_path: &Path,
    mut file: File,
    maximum: u64,
) -> io::Result<RetainedClaimWalLeaf> {
    let identity = crate::retained_dir::RetainedDirectory::identity_of(&file)?;
    let before = file.metadata()?;
    if before.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "retained claim WAL leaf exceeds its byte limit",
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum
        || after.len() != before.len()
        || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained claim WAL leaf changed while it was read",
        ));
    }
    let retained = RetainedClaimWalLeaf {
        relative_path: relative_path.to_path_buf(),
        file,
        identity,
        bytes,
    };
    validate_retained_claim_leaf(
        root,
        &retained.relative_path,
        &retained.file,
        &retained.identity,
        &retained.bytes,
    )?;
    Ok(retained)
}

fn validate_retained_claim_handle(
    retained_file: &File,
    retained_identity: &crate::retained_dir::RetainedFileIdentity,
    expected_bytes: &[u8],
) -> io::Result<()> {
    if crate::retained_dir::RetainedDirectory::identity_of(retained_file)? != *retained_identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained claim WAL leaf handle changed identity",
        ));
    }
    let mut retained = retained_file.try_clone()?;
    retained.seek(SeekFrom::Start(0))?;
    let mut actual = Vec::new();
    retained.read_to_end(&mut actual)?;
    if actual != expected_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained claim WAL leaf handle bytes changed",
        ));
    }
    Ok(())
}

fn validate_retained_claim_leaf(
    root: &crate::retained_dir::RetainedDirectory,
    relative_path: &Path,
    retained_file: &File,
    retained_identity: &crate::retained_dir::RetainedFileIdentity,
    expected_bytes: &[u8],
) -> io::Result<()> {
    validate_retained_claim_handle(retained_file, retained_identity, expected_bytes)?;
    let mut current = root.open_leaf_read(
        relative_path,
        crate::retained_dir::RetainedLeafPolicy::Authority,
    )?;
    if crate::retained_dir::RetainedDirectory::identity_of(&current)? != *retained_identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained claim WAL leaf namespace changed identity",
        ));
    }
    current.seek(SeekFrom::Start(0))?;
    let mut actual = Vec::new();
    current.read_to_end(&mut actual)?;
    if actual != expected_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained claim WAL leaf bytes changed",
        ));
    }
    Ok(())
}

fn write_durable_replaced_file_retained_with_commit<F>(
    guard: &ClaimWalRetainedLock,
    path: &Path,
    bytes: &[u8],
    mut commit_validation: F,
) -> io::Result<()>
where
    F: FnMut() -> io::Result<()>,
{
    guard
        .validate(&guard.state_root)
        .map_err(|source| io::Error::other(source.to_string()))?;
    let _cleanup_debt = crate::replace_retained_file_two_phase(&guard.root, path, bytes, || {
        commit_validation()?;
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))
    })?;
    Ok(())
}

fn write_durable_replaced_file_retained_from_expected_with_commit<F>(
    guard: &ClaimWalRetainedLock,
    path: &Path,
    expected: &RetainedClaimWalLeaf,
    bytes: &[u8],
    mut commit_validation: F,
) -> io::Result<()>
where
    F: FnMut() -> io::Result<()>,
{
    guard
        .validate(&guard.state_root)
        .map_err(|source| io::Error::other(source.to_string()))?;
    expected.validate(&guard.root)?;
    let _cleanup_debt = crate::replace_retained_file_two_phase_from_expected(
        &guard.root,
        path,
        &expected.file,
        &expected.identity,
        &expected.bytes,
        bytes,
        || {
            commit_validation()?;
            guard
                .validate(&guard.state_root)
                .map_err(|source| io::Error::other(source.to_string()))
        },
    )?;
    Ok(())
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

struct ClaimWalDecodeAuthority<'a> {
    guard: &'a ClaimWalRetainedLock,
    active_wal: &'a RetainedClaimWalLeaf,
}

fn retain_checkpoint_witness_bundle(
    authority: &ClaimWalDecodeAuthority<'_>,
    checkpoint: &ClaimWalCheckpointPayload,
    checkpoint_seq: u64,
) -> Result<(ClaimWalSnapshotPayload, RetainedCheckpointWitnessBundle), ClaimWalStopReason> {
    if checkpoint.schema_version != CLAIM_CHECKPOINT_PAYLOAD_SCHEMA_VERSION {
        // Every current checkpoint must select one immutable generation. Legacy
        // snapshot/manifest/archive tuples cannot be promoted by hiding or
        // downgrading any auxiliary file.
        return Err(ClaimWalStopReason::CheckpointGenerationInvalid);
    }
    let generation_digest = checkpoint
        .generation_sha256
        .as_deref()
        .filter(|digest| valid_sha256_content_hash(digest))
        .ok_or(ClaimWalStopReason::CheckpointGenerationInvalid)?;
    let generation_path = checkpoint_generation_relative_path(checkpoint, checkpoint_seq)
        .ok_or(ClaimWalStopReason::CheckpointGenerationInvalid)?;
    let root = authority
        .guard
        .root
        .try_clone()
        .map_err(|_| ClaimWalStopReason::CheckpointGenerationInvalid)?;
    let root_identity = root
        .identity()
        .map_err(|_| ClaimWalStopReason::CheckpointGenerationInvalid)?;
    match authority.guard.root.identity() {
        Ok(identity) if identity == root_identity => {}
        _ => return Err(ClaimWalStopReason::CheckpointGenerationInvalid),
    }
    let lock_file = authority
        .guard
        .lock
        .file
        .try_clone()
        .map_err(|_| ClaimWalStopReason::CheckpointGenerationInvalid)?;
    let lock = retain_open_claim_wal_leaf(
        &root,
        Path::new(CLAIM_WAL_LOCK_RELATIVE_PATH),
        lock_file,
        u64::from(DEFAULT_MAX_PAYLOAD_LEN),
    )
    .map_err(|_| ClaimWalStopReason::CheckpointGenerationInvalid)?;
    if lock.identity != authority.guard.lock_identity {
        return Err(ClaimWalStopReason::CheckpointGenerationInvalid);
    }
    let active_wal = clone_retained_claim_leaf(&root, authority.active_wal)
        .map_err(|_| ClaimWalStopReason::CheckpointGenerationInvalid)?;
    let generation_binding = checkpoint
        .generation_anchor
        .as_ref()
        .ok_or(ClaimWalStopReason::CheckpointGenerationInvalid)?;
    let (generation, generation_anchor) = retain_anchored_claim_wal_leaf(
        &root,
        &generation_path,
        Path::new(CLAIM_GENERATION_ANCHOR_RELATIVE_DIR),
        generation_binding,
        generation_digest,
        u64::from(DEFAULT_MAX_PAYLOAD_LEN).saturating_mul(2),
    )
    .map_err(|_| ClaimWalStopReason::CheckpointGenerationInvalid)?;
    let generation_document =
        serde_json::from_slice::<ClaimCheckpointGeneration>(&generation.bytes)
            .map_err(|_| ClaimWalStopReason::CheckpointGenerationInvalid)?;
    match serde_json_canonicalizer::to_vec(&generation_document) {
        Ok(canonical) if canonical == generation.bytes => {}
        _ => return Err(ClaimWalStopReason::CheckpointGenerationInvalid),
    }
    if !generation_matches_checkpoint(
        checkpoint,
        checkpoint_seq,
        &generation_path,
        generation_digest,
        u64::try_from(generation.bytes.len()).unwrap_or(u64::MAX),
        &generation_document,
    ) {
        return Err(ClaimWalStopReason::CheckpointGenerationInvalid);
    }

    let snapshot_path = generation_snapshot_relative_path(&generation_document)
        .ok_or(ClaimWalStopReason::CheckpointSnapshotInvalid)?;
    let (snapshot, snapshot_anchor) = retain_anchored_claim_wal_leaf(
        &root,
        &snapshot_path,
        Path::new(CLAIM_SNAPSHOT_ANCHOR_RELATIVE_DIR),
        &generation_document.snapshot_anchor,
        &generation_document.snapshot_sha256,
        u64::from(DEFAULT_MAX_PAYLOAD_LEN),
    )
    .map_err(|_| ClaimWalStopReason::CheckpointSnapshotInvalid)?;
    let canonical_snapshot =
        serde_json_canonicalizer::to_vec(&generation_document.snapshot_payload)
            .map_err(|_| ClaimWalStopReason::CheckpointSnapshotInvalid)?;
    if snapshot.bytes != canonical_snapshot
        || crate::sha256_content_hash(&snapshot.bytes) != generation_document.snapshot_sha256
        || crc32c::crc32c(&snapshot.bytes) != generation_document.snapshot_crc32c
    {
        return Err(ClaimWalStopReason::CheckpointSnapshotInvalid);
    }

    let archive_path = generation_archive_relative_path(&generation_document)
        .ok_or(ClaimWalStopReason::CheckpointArchiveInvalid)?;
    let (archive, archive_anchor) = retain_anchored_claim_wal_leaf(
        &root,
        &archive_path,
        Path::new(CLAIM_ARCHIVE_ANCHOR_RELATIVE_DIR),
        &generation_document.archived_wal_anchor,
        &generation_document.archived_wal_sha256,
        u64::MAX,
    )
    .map_err(|_| ClaimWalStopReason::CheckpointArchiveInvalid)?;
    if crate::sha256_content_hash(&archive.bytes) != generation_document.archived_wal_sha256
        || u64::try_from(archive.bytes.len()).unwrap_or(u64::MAX)
            != generation_document.archived_wal_byte_len
        || generation_document.archived_wal_sha256 != generation_document.source_wal_sha256
        || generation_document.archived_wal_byte_len != generation_document.source_wal_byte_len
    {
        return Err(ClaimWalStopReason::CheckpointArchiveInvalid);
    }

    let snapshot_document = generation_document.snapshot_payload.clone();
    let bundle = RetainedCheckpointWitnessBundle {
        root,
        root_identity,
        lock,
        active_wal,
        generation,
        generation_anchor,
        snapshot,
        snapshot_anchor,
        archive,
        archive_anchor,
    };
    bundle
        .validate(authority.guard)
        .map_err(|_| ClaimWalStopReason::CheckpointGenerationInvalid)?;
    Ok((snapshot_document, bundle))
}

fn generation_matches_checkpoint(
    checkpoint: &ClaimWalCheckpointPayload,
    checkpoint_seq: u64,
    generation_path: &Path,
    generation_digest: &str,
    generation_byte_length: u64,
    generation: &ClaimCheckpointGeneration,
) -> bool {
    let clean_source_shape = if generation.source_wal_repaired {
        generation.source_wal_original_len >= generation.source_wal_byte_len
            && generation.source_wal_last_good_offset == generation.source_wal_byte_len
    } else {
        generation.source_wal_stop_reason == ClaimWalStopReason::CleanEof
            && generation.source_wal_original_len == generation.source_wal_byte_len
            && generation.source_wal_last_good_offset == generation.source_wal_byte_len
    };
    let Ok(snapshot_bytes) = serde_json_canonicalizer::to_vec(&generation.snapshot_payload) else {
        return false;
    };
    let snapshot_byte_length = u64::try_from(snapshot_bytes.len()).unwrap_or(u64::MAX);
    let generation_path_string = path_to_wal_string(generation_path);
    generation.schema_version == CLAIM_CHECKPOINT_GENERATION_SCHEMA_VERSION
        && generation.authority_kind == CLAIM_CHECKPOINT_AUTHORITY_KIND
        && valid_operation_nonce(&generation.operation_nonce)
        && generation.active_wal_path == CLAIM_WAL_RELATIVE_PATH
        && generation.checkpoint_seq == checkpoint_seq
        && generation.lock_path == CLAIM_WAL_LOCK_RELATIVE_PATH
        && generation.source_wal_path == CLAIM_WAL_RELATIVE_PATH
        && valid_sha256_content_hash(&generation.source_wal_sha256)
        && clean_source_shape
        && generation.source_wal_last_seq.checked_add(1) == Some(checkpoint_seq)
        && generation.source_wal_last_seq == checkpoint.last_seq_in_snapshot
        && generation.snapshot_payload.schema_version == "0.1"
        && generation.snapshot_payload.last_seq == generation.source_wal_last_seq
        && generation.snapshot_payload.created_at == generation.created_at
        && generation.snapshot_payload.created_at_ms == generation.created_at_ms
        && valid_sha256_content_hash(&generation.snapshot_sha256)
        && valid_sha256_content_hash(&generation.archived_wal_sha256)
        && claim_anchor_binding_matches(
            &generation.snapshot_anchor,
            Path::new(CLAIM_SNAPSHOT_ANCHOR_RELATIVE_DIR),
            &generation.snapshot_sha256,
            snapshot_byte_length,
        )
        && claim_anchor_binding_matches(
            &generation.archived_wal_anchor,
            Path::new(CLAIM_ARCHIVE_ANCHOR_RELATIVE_DIR),
            &generation.archived_wal_sha256,
            generation.archived_wal_byte_len,
        )
        && checkpoint.snapshot_path == generation.snapshot_path
        && checkpoint.snapshot_crc32c == generation.snapshot_crc32c
        && checkpoint.archived_wal_path.as_deref() == Some(generation.archived_wal_path.as_str())
        && checkpoint.archived_wal_sha256.as_deref()
            == Some(generation.archived_wal_sha256.as_str())
        && checkpoint.generation_path.as_deref() == Some(generation_path_string.as_str())
        && checkpoint.generation_sha256.as_deref() == Some(generation_digest)
        && checkpoint
            .generation_anchor
            .as_ref()
            .is_some_and(|binding| {
                claim_anchor_binding_matches(
                    binding,
                    Path::new(CLAIM_GENERATION_ANCHOR_RELATIVE_DIR),
                    generation_digest,
                    generation_byte_length,
                )
            })
        && checkpoint.created_at == generation.created_at
        && checkpoint.created_at_ms == generation.created_at_ms
}

fn active_wal_declares_required_generation(bytes: &[u8]) -> bool {
    let Ok(frame) = decode_record_frame(bytes, 0) else {
        return false;
    };
    if frame.record_type != RECORD_TYPE_CHECKPOINT_REF {
        return false;
    }
    let Ok(checkpoint) = serde_json::from_slice::<ClaimWalCheckpointPayload>(frame.payload) else {
        return false;
    };
    if checkpoint.schema_version != CLAIM_CHECKPOINT_PAYLOAD_SCHEMA_VERSION
        || !matches!(
            serde_json_canonicalizer::to_vec(&checkpoint),
            Ok(canonical) if canonical.as_slice() == frame.payload
        )
    {
        return false;
    }
    let Some(generation_digest) = checkpoint
        .generation_sha256
        .as_deref()
        .filter(|digest| valid_sha256_content_hash(digest))
    else {
        return false;
    };
    if checkpoint_generation_relative_path(&checkpoint, frame.seq).is_none() {
        return false;
    }
    checkpoint
        .generation_anchor
        .as_ref()
        .is_some_and(|binding| {
            claim_anchor_binding_matches(
                binding,
                Path::new(CLAIM_GENERATION_ANCHOR_RELATIVE_DIR),
                generation_digest,
                binding.byte_length,
            )
        })
}

fn checkpoint_generation_relative_path(
    checkpoint: &ClaimWalCheckpointPayload,
    checkpoint_seq: u64,
) -> Option<PathBuf> {
    let digest = checkpoint.generation_sha256.as_deref()?;
    let declared = checkpoint.generation_path.as_deref()?;
    let path = safe_relative_path(declared)?;
    (path == generation_relative_path(checkpoint_seq, digest)?
        && path_to_wal_string(&path) == declared)
        .then_some(path)
}

fn generation_snapshot_relative_path(generation: &ClaimCheckpointGeneration) -> Option<PathBuf> {
    let path = safe_relative_path(&generation.snapshot_path)?;
    (path == snapshot_relative_path(generation.source_wal_last_seq, &generation.snapshot_sha256)?
        && path_to_wal_string(&path) == generation.snapshot_path)
        .then_some(path)
}

fn generation_archive_relative_path(generation: &ClaimCheckpointGeneration) -> Option<PathBuf> {
    let path = safe_relative_path(&generation.archived_wal_path)?;
    (path == archive_relative_path(generation.checkpoint_seq, &generation.archived_wal_sha256)?
        && path_to_wal_string(&path) == generation.archived_wal_path)
        .then_some(path)
}

fn claim_checkpoint_operation_nonce() -> io::Result<String> {
    let mut nonce = [0_u8; 32];
    getrandom::fill(&mut nonce)
        .map_err(|error| io::Error::other(format!("claim checkpoint nonce failed: {error}")))?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(nonce.len() * 2);
    for byte in nonce {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(encoded)
}

fn valid_operation_nonce(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_sha256_content_hash(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn content_hash_hex(value: &str) -> Option<&str> {
    valid_sha256_content_hash(value).then(|| value.trim_start_matches("sha256:"))
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

fn snapshot_relative_path(last_seq: u64, digest: &str) -> Option<PathBuf> {
    Some(PathBuf::from(CLAIM_WAL_SNAPSHOT_RELATIVE_DIR).join(format!(
        "claims.snapshot.{last_seq:020}.{}.json",
        content_hash_hex(digest)?
    )))
}

fn archive_relative_path(checkpoint_seq: u64, digest: &str) -> Option<PathBuf> {
    Some(PathBuf::from(CLAIM_WAL_ARCHIVE_RELATIVE_DIR).join(format!(
        "claims.fmw1.before-{checkpoint_seq:020}.{}",
        content_hash_hex(digest)?
    )))
}

fn generation_relative_path(checkpoint_seq: u64, digest: &str) -> Option<PathBuf> {
    Some(
        PathBuf::from(CLAIM_WAL_CHECKPOINT_RELATIVE_DIR).join(format!(
            "claims.checkpoint.{checkpoint_seq:020}.{}.json",
            content_hash_hex(digest)?
        )),
    )
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

fn require_missing_claim_leaf(
    root: &crate::retained_dir::RetainedDirectory,
    relative_path: &Path,
) -> io::Result<()> {
    match root.open_leaf_read(
        relative_path,
        crate::retained_dir::RetainedLeafPolicy::Authority,
    ) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "claim authority leaf {} is present",
                relative_path.display()
            ),
        )),
        Err(error) => Err(error),
    }
}

fn require_missing_claim_directory(
    root: &crate::retained_dir::RetainedDirectory,
    relative_path: &Path,
) -> io::Result<()> {
    match root.open_directory(relative_path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "claim authority directory {} contains retained residue",
                relative_path.display()
            ),
        )),
        Err(error) => Err(error),
    }
}

fn require_pristine_claim_checkpoint_namespace(
    root: &crate::retained_dir::RetainedDirectory,
) -> io::Result<()> {
    // These canonical directories are created only when checkpoint publication
    // begins and are never removed by a successful or failed claim operation.
    // Treating the descriptor-relatively retained directory itself as residue is
    // deliberately stricter than ambient pathname enumeration: even an emptied
    // authority namespace cannot be reclassified as a pristine Store.
    for relative_path in [
        CLAIM_WAL_CHECKPOINT_RELATIVE_DIR,
        CLAIM_WAL_SNAPSHOT_RELATIVE_DIR,
        CLAIM_WAL_ARCHIVE_RELATIVE_DIR,
        CLAIM_CHECKPOINT_ANCHOR_RELATIVE_DIR,
    ] {
        require_missing_claim_directory(root, Path::new(relative_path))?;
    }
    require_missing_claim_leaf(root, Path::new(CLAIM_WAL_MANIFEST_RELATIVE_PATH))
}

fn retain_pristine_claim_authority(
    guard: &ClaimWalRetainedLock,
    active_wal: Option<&RetainedClaimWalLeaf>,
) -> io::Result<RetainedPristineClaimAuthority> {
    let root = guard.root.try_clone()?;
    let root_identity = root.identity()?;
    if guard.root.identity()? != root_identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "claim pristine authority root changed during retention",
        ));
    }
    let lock_file = guard.lock.file.try_clone()?;
    let lock = retain_open_claim_wal_leaf(
        &root,
        Path::new(CLAIM_WAL_LOCK_RELATIVE_PATH),
        lock_file,
        u64::from(DEFAULT_MAX_PAYLOAD_LEN),
    )?;
    if lock.identity != guard.lock_identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "claim pristine authority lock identity changed",
        ));
    }
    let authority = RetainedPristineClaimAuthority {
        root,
        root_identity,
        lock,
    };
    authority.validate(guard, active_wal)?;
    Ok(authority)
}

fn reject_unselected_checkpoint_residue(recovery: &mut ClaimWalRecovery) {
    recovery.records.clear();
    recovery.checkpoint = None;
    recovery.last_observed_seq = 0;
    recovery.valid_record_count = 0;
    recovery.last_good_offset = 0;
    recovery.repaired = false;
    recovery.stop_reason = ClaimWalStopReason::CheckpointGenerationInvalid;
}

fn is_repairable_stop_reason(reason: ClaimWalStopReason) -> bool {
    !matches!(
        reason,
        ClaimWalStopReason::CleanEof
            | ClaimWalStopReason::CheckpointSnapshotInvalid
            | ClaimWalStopReason::CheckpointArchiveInvalid
            | ClaimWalStopReason::CheckpointGenerationInvalid
            | ClaimWalStopReason::CheckpointNotAtStart
    )
}

struct RetainedClaimWalRecovery {
    recovery: ClaimWalRecovery,
    checkpoint_witness: Option<RetainedCheckpointWitnessBundle>,
    pristine_authority: Option<RetainedPristineClaimAuthority>,
    active_wal: Option<RetainedClaimWalLeaf>,
}

impl RetainedClaimWalRecovery {
    fn validate(&self, guard: &ClaimWalRetainedLock) -> io::Result<()> {
        if let Some(witness) = self.checkpoint_witness.as_ref() {
            return witness.validate(guard);
        }
        if let Some(pristine) = self.pristine_authority.as_ref() {
            return pristine.validate(guard, self.active_wal.as_ref());
        }
        self.validate_active_wal_or_absence(guard)
    }

    fn validate_for_rotation(&self, guard: &ClaimWalRetainedLock) -> io::Result<()> {
        if let Some(witness) = self.checkpoint_witness.as_ref() {
            return witness.validate(guard);
        }
        if let Some(pristine) = self.pristine_authority.as_ref() {
            return pristine.validate_for_rotation(guard, self.active_wal.as_ref());
        }
        self.validate_active_wal_or_absence(guard)
    }

    fn validate_active_wal_or_absence(&self, guard: &ClaimWalRetainedLock) -> io::Result<()> {
        if let Some(active_wal) = self.active_wal.as_ref() {
            validate_retained_claim_leaf(
                &guard.root,
                &active_wal.relative_path,
                &active_wal.file,
                &active_wal.identity,
                &active_wal.bytes,
            )?;
        } else {
            require_missing_claim_leaf(&guard.root, Path::new(CLAIM_WAL_RELATIVE_PATH))?;
        }
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))
    }

    fn validate_after_active_replacement(&self, guard: &ClaimWalRetainedLock) -> io::Result<()> {
        if let Some(witness) = self.checkpoint_witness.as_ref() {
            witness.validate_after_active_replacement(guard)?;
        } else if let Some(pristine) = self.pristine_authority.as_ref() {
            pristine.validate_after_active_replacement(guard, self.active_wal.as_ref())?;
        }
        if let Some(active_wal) = self.active_wal.as_ref() {
            active_wal.validate_handle()?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "claim WAL rotation lost its retained source witness",
            ));
        }
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))
    }

    fn observe_append(&mut self, guard: &ClaimWalRetainedLock, bytes: &[u8]) -> io::Result<()> {
        if let Some(active_wal) = self.active_wal.as_mut() {
            active_wal.bytes.extend_from_slice(bytes);
        } else {
            let relative = Path::new(CLAIM_WAL_RELATIVE_PATH);
            let file = guard
                .root
                .open_leaf_read(relative, crate::retained_dir::RetainedLeafPolicy::Authority)?;
            self.active_wal = Some(retain_open_claim_wal_leaf(
                &guard.root,
                relative,
                file,
                u64::MAX,
            )?);
        }
        if let Some(witness) = self.checkpoint_witness.as_mut() {
            let expected = self
                .active_wal
                .as_ref()
                .map(|active_wal| active_wal.bytes.as_slice())
                .unwrap_or_default();
            witness.update_active_wal_bytes(expected);
        }
        self.validate(guard)
    }

    fn into_recovery(mut self, guard: &ClaimWalRetainedLock) -> io::Result<ClaimWalRecovery> {
        // Public recovery linearizes at this final joint retained validation and
        // carries the same exact bundle through the returned value.
        self.validate(guard)?;
        if self.recovery.stop_reason == ClaimWalStopReason::CleanEof || self.recovery.repaired {
            let inner = if let Some(witness) = self.checkpoint_witness.take() {
                Some(ClaimWalRecoveryAuthorityInner::Checkpoint(witness))
            } else {
                self.pristine_authority.take().map(|authority| {
                    ClaimWalRecoveryAuthorityInner::Pristine {
                        authority,
                        active_wal: self.active_wal.take(),
                    }
                })
            };
            self.recovery.retained_authority = inner.map(|inner| ClaimWalRecoveryAuthority {
                inner: Arc::new(inner),
            });
        }
        Ok(self.recovery)
    }
}

fn recover_claim_wal_file_under_lock(
    guard: &ClaimWalRetainedLock,
    repair: bool,
) -> io::Result<RetainedClaimWalRecovery> {
    let relative = Path::new(CLAIM_WAL_RELATIVE_PATH);
    let opened = if repair {
        guard.root.open_leaf_read_write_existing(relative)
    } else {
        guard
            .root
            .open_leaf_read(relative, crate::retained_dir::RetainedLeafPolicy::Authority)
    };
    let mut retained = match opened {
        Ok(file) => Some(retain_open_claim_wal_leaf(
            &guard.root,
            relative,
            file,
            u64::MAX,
        )?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let mut decoded = {
        let empty = Vec::new();
        let bytes = retained
            .as_ref()
            .map_or(empty.as_slice(), |retained| retained.bytes.as_slice());
        let authority = retained
            .as_ref()
            .map(|active_wal| ClaimWalDecodeAuthority { guard, active_wal });
        decode_prefix(&guard.wal_path, bytes, authority.as_ref())
    };
    let original_len = decoded.recovery.original_len;
    let active_requires_pristine = retained
        .as_ref()
        .is_none_or(|active_wal| !active_wal_declares_required_generation(&active_wal.bytes));
    let pristine_authority = if decoded.checkpoint_witness.is_none() && active_requires_pristine {
        match retain_pristine_claim_authority(guard, retained.as_ref()) {
            Ok(authority) => Some(authority),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                reject_unselected_checkpoint_residue(&mut decoded.recovery);
                None
            }
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    if repair
        && decoded.recovery.last_good_offset < original_len
        && is_repairable_stop_reason(decoded.recovery.stop_reason)
    {
        guard
            .validate(&guard.state_root)
            .map_err(|source| io::Error::other(source.to_string()))?;
        let retained = retained.as_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "claim WAL disappeared before retained repair",
            )
        })?;
        guard.root.verify_retained_authority_binding(
            relative,
            &retained.file,
            &retained.identity,
        )?;
        let repaired_len = usize::try_from(decoded.recovery.last_good_offset)
            .map_err(|_| io::Error::other("claim WAL repair offset does not fit usize"))?;
        retained.file.set_len(decoded.recovery.last_good_offset)?;
        retained.file.sync_all()?;
        retained.bytes.truncate(repaired_len);
        if let Some(witness) = decoded.checkpoint_witness.as_mut() {
            witness.update_active_wal_bytes(&retained.bytes);
        }
        decoded.recovery.repaired = true;
    }
    let retained_recovery = RetainedClaimWalRecovery {
        recovery: decoded.recovery,
        checkpoint_witness: decoded.checkpoint_witness,
        pristine_authority,
        active_wal: retained,
    };
    retained_recovery.validate(guard)?;
    Ok(retained_recovery)
}

enum DecodeRecordOutcome {
    Known {
        record: Box<ClaimWalRecord>,
        next_offset: usize,
    },
    Checkpoint {
        checkpoint: Box<ClaimWalCheckpointRecord>,
        witness: Box<RetainedCheckpointWitnessBundle>,
        next_offset: usize,
    },
    SkippedUnknown {
        seq: u64,
        next_offset: usize,
    },
    Stop(ClaimWalStopReason),
}

struct DecodedClaimWalPrefix {
    recovery: ClaimWalRecovery,
    checkpoint_witness: Option<RetainedCheckpointWitnessBundle>,
}

fn decode_prefix(
    wal_path: &Path,
    bytes: &[u8],
    authority: Option<&ClaimWalDecodeAuthority<'_>>,
) -> DecodedClaimWalPrefix {
    let original_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let mut records = Vec::new();
    let mut checkpoint = None;
    let mut checkpoint_witness = None;
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
        match decode_record_at(bytes, offset, expected_seq, authority) {
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
                witness,
                next_offset,
            } => {
                last_observed_seq = decoded_checkpoint.seq;
                checkpoint = Some(*decoded_checkpoint);
                checkpoint_witness = Some(*witness);
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

    DecodedClaimWalPrefix {
        recovery: ClaimWalRecovery {
            wal_path: wal_path.to_path_buf(),
            records,
            checkpoint,
            last_observed_seq,
            valid_record_count,
            last_good_offset: u64::try_from(offset).unwrap_or(u64::MAX),
            original_len,
            repaired: false,
            stop_reason,
            retained_authority: None,
        },
        checkpoint_witness,
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
    bytes: &[u8],
    offset: usize,
    expected_seq: u64,
    authority: Option<&ClaimWalDecodeAuthority<'_>>,
) -> DecodeRecordOutcome {
    let frame = match decode_record_frame(bytes, offset) {
        Ok(frame) => frame,
        Err(reason) => return DecodeRecordOutcome::Stop(reason),
    };
    if frame.record_type == RECORD_TYPE_CHECKPOINT_REF {
        return decode_checkpoint_record(offset, &frame, authority);
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
    offset: usize,
    frame: &DecodedRecordFrame<'_>,
    authority: Option<&ClaimWalDecodeAuthority<'_>>,
) -> DecodeRecordOutcome {
    if offset != 0 {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::CheckpointNotAtStart);
    }
    let Ok(decoded_payload) = serde_json::from_slice::<ClaimWalCheckpointPayload>(frame.payload)
    else {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::PayloadDecodeFailed);
    };
    if !matches!(
        serde_json_canonicalizer::to_vec(&decoded_payload),
        Ok(canonical) if canonical.as_slice() == frame.payload
    ) {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::CheckpointGenerationInvalid);
    }
    let Some(expected_checkpoint_seq) = decoded_payload.last_seq_in_snapshot.checked_add(1) else {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::SequenceGap);
    };
    if frame.seq != expected_checkpoint_seq {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::SequenceGap);
    }
    let Some(authority) = authority else {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::CheckpointGenerationInvalid);
    };
    let (snapshot, witness) =
        match retain_checkpoint_witness_bundle(authority, &decoded_payload, frame.seq) {
            Ok(decoded) => decoded,
            Err(reason) => return DecodeRecordOutcome::Stop(reason),
        };
    DecodeRecordOutcome::Checkpoint {
        checkpoint: Box::new(ClaimWalCheckpointRecord {
            seq: frame.seq,
            payload: decoded_payload,
            snapshot,
            offset: u64::try_from(offset).unwrap_or(u64::MAX),
            record_len: u64::try_from(frame.record_end - offset).unwrap_or(u64::MAX),
        }),
        witness: Box::new(witness),
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

fn acquire_claim_wal_retained_lock_raw(state_root: &Path) -> io::Result<ClaimWalRetainedLock> {
    let boundary = crate::producer_quiescence::admit_producer(state_root)
        .map_err(|source| io::Error::other(source.to_string()))?;
    acquire_claim_wal_retained_lock_raw_under_boundary(&boundary, state_root)
}

fn acquire_claim_wal_retained_lock_raw_under_boundary(
    boundary: &impl crate::producer_quiescence::ProducerBoundary,
    state_root: &Path,
) -> io::Result<ClaimWalRetainedLock> {
    let boundary = crate::producer_quiescence::BoundaryLease::from_boundary(boundary, state_root)
        .map_err(|source| io::Error::other(source.to_string()))?;
    let state_root = fs::canonicalize(state_root)?;
    let root = boundary
        .retained_root()
        .map_err(|source| io::Error::other(source.to_string()))?;
    let relative = Path::new(CLAIM_WAL_LOCK_RELATIVE_PATH);
    root.create_dir_all(relative.parent().expect("lock has parent"))?;
    let file = root.open_read_write_create(relative)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("claim WAL lock is not a regular file"));
    }
    FileExt::lock(&file)?;
    let lock_identity = crate::retained_dir::RetainedDirectory::identity_of(&file)?;
    boundary
        .validate_root(&state_root)
        .map_err(|source| io::Error::other(source.to_string()))?;
    Ok(ClaimWalRetainedLock {
        boundary,
        root,
        lock_identity,
        lock: ClaimWalLock { file },
        wal_path: claim_wal_path(&state_root),
        state_root,
    })
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
    decode_prefix(std::path::Path::new("<fuzz>"), bytes, None).recovery
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use forge_core_contracts::claim::{
        ActorRole, ClaimIdentity, ClaimKind, ClaimLease, ClaimScope, ClaimScopeKind,
        ClaimStatusRecord, ExpiryAction, ExpiryPolicy, ReclaimPolicy,
    };
    #[cfg(unix)]
    use forge_core_contracts::{ClaimId, RepoPath, ScopeId, StableId};
    use std::fs::OpenOptions;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("forge-claim-wal-lock-{}-{id}", std::process::id()));
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
    #[test]
    fn retained_lock_binds_root_recovers_without_relocking_and_releases_on_drop() {
        let root = TestRoot::new();
        let other = TestRoot::new();
        let guard = acquire_claim_wal_retained_lock(&root.0).expect("acquire retained lock");

        let second = OpenOptions::new()
            .read(true)
            .write(true)
            .open(claim_wal_lock_path(&root.0))
            .expect("open second lock handle");
        let error = FileExt::try_lock(&second).expect_err("retained guard must block");
        assert!(matches!(error, fs4::TryLockError::WouldBlock));

        let recovery = recover_claim_wal_under_retained_lock(&root.0, &guard, false)
            .expect("recover under retained lock");
        assert_eq!(recovery.stop_reason, ClaimWalStopReason::CleanEof);
        let mismatch = recover_claim_wal_under_retained_lock(&other.0, &guard, false)
            .expect_err("mismatched root must fail");
        assert!(matches!(mismatch, ClaimWalReadError::ReadWal { .. }));

        drop(guard);
        FileExt::try_lock(&second).expect("drop must release lock");
        FileExt::unlock(&second).expect("unlock second handle");
    }

    #[cfg(unix)]
    #[test]
    fn two_phase_replacement_restores_exact_previous_leaf_when_lock_identity_changes() {
        let root = TestRoot::new();
        fs::create_dir_all(claim_wal_path(&root.0).parent().expect("WAL parent"))
            .expect("create WAL parent");
        let previous_bytes = b"exact previous claim WAL";
        fs::write(claim_wal_path(&root.0), previous_bytes).expect("seed previous WAL");
        let previous_file = File::open(claim_wal_path(&root.0)).expect("retain previous WAL");
        let previous_identity = crate::retained_dir::RetainedDirectory::identity_of(&previous_file)
            .expect("previous identity");
        let guard = acquire_claim_wal_retained_lock(&root.0).expect("acquire retained lock");
        let lock_path = claim_wal_lock_path(&root.0);
        let displaced_lock = root.0.join("locks/claims.wal.lock.displaced");

        let error = write_durable_replaced_file_retained_with_commit(
            &guard,
            Path::new(CLAIM_WAL_RELATIVE_PATH),
            b"replacement must not remain committed",
            || {
                fs::rename(&lock_path, &displaced_lock)?;
                fs::write(&lock_path, b"substitute lock")?;
                guard
                    .validate(&guard.state_root)
                    .map_err(|source| io::Error::other(source.to_string()))
            },
        )
        .expect_err("changed lock identity must reject final commit");
        assert!(error.to_string().contains("restored"));
        assert_eq!(
            fs::read(claim_wal_path(&root.0)).expect("read rolled-back WAL"),
            previous_bytes
        );
        let restored = File::open(claim_wal_path(&root.0)).expect("open restored WAL");
        assert_eq!(
            crate::retained_dir::RetainedDirectory::identity_of(&restored)
                .expect("restored identity"),
            previous_identity,
            "rollback must restore the exact previous object, not only its bytes"
        );
    }

    #[cfg(unix)]
    #[test]
    fn two_phase_rollback_isolates_substitute_before_restoring_exact_previous_leaf() {
        let root = TestRoot::new();
        fs::create_dir_all(claim_wal_path(&root.0).parent().expect("WAL parent"))
            .expect("create WAL parent");
        let previous_bytes = b"exact previous claim WAL";
        fs::write(claim_wal_path(&root.0), previous_bytes).expect("seed previous WAL");
        let previous_file = File::open(claim_wal_path(&root.0)).expect("retain previous WAL");
        let previous_identity = crate::retained_dir::RetainedDirectory::identity_of(&previous_file)
            .expect("previous identity");
        let guard = acquire_claim_wal_retained_lock(&root.0).expect("acquire retained lock");

        write_durable_replaced_file_retained_with_commit(
            &guard,
            Path::new(CLAIM_WAL_RELATIVE_PATH),
            b"replacement must be rolled back",
            || {
                fs::remove_file(claim_wal_path(&root.0))?;
                fs::write(claim_wal_path(&root.0), b"attacker-controlled substitute")
            },
        )
        .expect_err("substituted target must fail final commit validation");

        assert_eq!(
            fs::read(claim_wal_path(&root.0)).expect("read exact restored WAL"),
            previous_bytes
        );
        let restored = File::open(claim_wal_path(&root.0)).expect("open restored WAL");
        assert_eq!(
            crate::retained_dir::RetainedDirectory::identity_of(&restored)
                .expect("restored identity"),
            previous_identity,
            "rollback must restore the exact previous inode after isolating the substitute"
        );
        assert!(
            fs::read_dir(claim_wal_path(&root.0).parent().expect("WAL parent"))
                .expect("list cleanup debt")
                .filter_map(Result::ok)
                .any(|entry| {
                    fs::read(entry.path()).ok().as_deref()
                        == Some(&b"attacker-controlled substitute"[..])
                })
        );
    }

    #[test]
    fn two_phase_replacement_restores_store_placeholder_when_destination_was_absent() {
        let root = TestRoot::new();
        let guard = acquire_claim_wal_retained_lock(&root.0).expect("acquire retained lock");
        let target = Path::new("wal/new-claim-wal-metadata.json");

        write_durable_replaced_file_retained_with_commit(
            &guard,
            target,
            b"must remain isolated",
            || Err(io::Error::other("reject final commit")),
        )
        .expect_err("failed final validation must isolate new destination");

        assert_eq!(
            fs::read(guard.state_root.join(target)).expect("read fail-closed placeholder"),
            b"",
            "failed create-new replacement must leave the exact Store placeholder authoritative"
        );
    }

    #[cfg(unix)]
    #[test]
    fn expected_source_replacement_rejects_coordinated_active_wal_aba() {
        let root = TestRoot::new();
        fs::create_dir_all(claim_wal_path(&root.0).parent().expect("WAL parent"))
            .expect("create WAL parent");
        let source_bytes = b"exact recovered claim WAL";
        fs::write(claim_wal_path(&root.0), source_bytes).expect("seed source WAL");
        let guard = acquire_claim_wal_retained_lock(&root.0).expect("acquire retained lock");
        let source =
            retain_claim_wal_leaf(&guard.root, Path::new(CLAIM_WAL_RELATIVE_PATH), u64::MAX)
                .expect("retain source WAL");
        let displaced = root.0.join("wal/claims.fmw1.displaced-a");
        fs::rename(claim_wal_path(&root.0), &displaced).expect("displace source A");
        fs::write(claim_wal_path(&root.0), source_bytes).expect("install byte-identical B");

        crate::replace_retained_file_two_phase_from_expected(
            &guard.root,
            Path::new(CLAIM_WAL_RELATIVE_PATH),
            &source.file,
            &source.identity,
            &source.bytes,
            b"checkpoint replacement",
            || Ok(()),
        )
        .expect_err("byte-identical substitute B must not satisfy retained A authority");

        assert_eq!(
            fs::read(claim_wal_path(&root.0)).expect("read substitute B"),
            source_bytes
        );
        assert_eq!(
            crate::retained_dir::RetainedDirectory::identity_of(
                &File::open(&displaced).expect("open displaced A")
            )
            .expect("displaced A identity"),
            source.identity
        );
    }

    #[cfg(unix)]
    #[test]
    fn retained_archive_rejects_active_wal_aba_before_copying_bytes() {
        let root = TestRoot::new();
        let guard = acquire_claim_wal_retained_lock(&root.0).expect("acquire retained lock");
        append_claim_wal_record_under_retained_lock(
            &guard,
            ClaimWalOperation::Acquire,
            &test_claim(),
            "2027-01-15T08:00:00Z",
            crate::WalDurability::SyncOnAppend,
        )
        .expect("seed WAL");
        let recovery = recover_claim_wal_file_under_lock(&guard, false)
            .expect("retain exact active WAL recovery");
        let source_bytes = recovery
            .active_wal
            .as_ref()
            .expect("retained active WAL")
            .bytes
            .clone();
        fs::rename(
            claim_wal_path(&root.0),
            root.0.join("wal/claims.fmw1.displaced-before-archive"),
        )
        .expect("displace retained active WAL");
        fs::write(claim_wal_path(&root.0), &source_bytes)
            .expect("install byte-identical substitute");

        copy_active_wal_to_archive_retained(&guard, &recovery, 2)
            .expect_err("archive must reject substituted active path");
    }

    #[cfg(unix)]
    #[test]
    fn retained_archive_rejects_same_bytes_under_a_substitute_name_binding() {
        let root = TestRoot::new();
        let guard = acquire_claim_wal_retained_lock(&root.0).expect("acquire retained lock");
        append_claim_wal_record_under_retained_lock(
            &guard,
            ClaimWalOperation::Acquire,
            &test_claim(),
            "2027-01-15T08:00:00Z",
            crate::WalDurability::SyncOnAppend,
        )
        .expect("seed WAL");
        let recovery = recover_claim_wal_file_under_lock(&guard, false)
            .expect("retain exact active WAL recovery");
        let archive = copy_active_wal_to_archive_retained(&guard, &recovery, 2)
            .expect("publish archive from retained recovery");
        let displaced = archive.absolute_path.with_extension("retained-original");
        fs::rename(&archive.absolute_path, &displaced).expect("displace retained archive");
        fs::write(&archive.absolute_path, &archive.bytes)
            .expect("install byte-identical substitute");

        let error = archive
            .validate(&guard)
            .expect_err("substitute archive identity must be rejected");
        assert!(error.to_string().contains("identity"));
        assert!(!claim_wal_manifest_path(&root.0).exists());
    }

    #[cfg(unix)]
    #[test]
    fn rotation_ignores_legacy_authority_temp_orphan_and_remains_repeatable() {
        let root = TestRoot::new();
        let guard = acquire_claim_wal_retained_lock(&root.0).expect("acquire retained lock");
        append_claim_wal_record_under_retained_lock(
            &guard,
            ClaimWalOperation::Acquire,
            &test_claim(),
            "2027-01-15T08:00:00Z",
            crate::WalDurability::SyncOnAppend,
        )
        .expect("seed active claim WAL");
        let stale_temp = root.0.join("wal/claims.authority.tmp");
        let stale_sentinel = b"crash orphan must not become the active WAL";
        fs::write(&stale_temp, stale_sentinel).expect("seed legacy crash orphan");
        let options = ClaimWalRotationOptions {
            max_wal_bytes: u64::MAX,
            max_records: 0,
            max_replay_millis: u64::MAX,
        };

        let first = rotate_claim_wal_if_needed_under_retained_lock(
            &guard,
            "2027-01-15T08:01:00Z",
            &options,
        )
        .expect("legacy orphan must not block rotation");
        assert!(first.rotated);
        assert_eq!(first.wal_path, claim_wal_path(&root.0));
        let first_wal = fs::read(&first.wal_path).expect("read durable replacement");
        assert_ne!(first_wal, stale_sentinel);
        assert_eq!(
            recover_claim_wal_for_guard(&guard, false)
                .expect("recover first replacement")
                .stop_reason,
            ClaimWalStopReason::CleanEof
        );
        assert_eq!(fs::read(&stale_temp).expect("read orphan"), stale_sentinel);

        let second = rotate_claim_wal_if_needed_under_retained_lock(
            &guard,
            "2027-01-15T08:02:00Z",
            &options,
        )
        .expect("subsequent rotation must also succeed");
        assert!(second.rotated);
        let second_wal = fs::read(&second.wal_path).expect("read second replacement");
        assert_ne!(second_wal, first_wal);
        assert_eq!(
            recover_claim_wal_for_guard(&guard, false)
                .expect("recover second replacement")
                .stop_reason,
            ClaimWalStopReason::CleanEof
        );
    }
    #[cfg(unix)]
    #[test]
    fn retained_claim_wal_append_on_a_rejects_replacement_b_without_writing_either() {
        let root = TestRoot::new();
        fs::create_dir_all(claim_wal_path(&root.0).parent().expect("WAL parent"))
            .expect("create A WAL parent");
        fs::write(claim_wal_path(&root.0), b"").expect("create A WAL");
        let guard = acquire_claim_wal_retained_lock(&root.0).expect("lock A");
        let displaced = root.0.with_extension("inode-a");
        fs::rename(&root.0, &displaced).expect("displace A");
        fs::create_dir_all(claim_wal_path(&root.0).parent().expect("B WAL parent"))
            .expect("create B WAL parent");
        fs::write(claim_wal_path(&root.0), b"B-sentinel").expect("shape B WAL");
        let a_before = fs::read(claim_wal_path(&displaced)).expect("read A");
        let b_before = fs::read(claim_wal_path(&root.0)).expect("read B");

        assert!(append_claim_wal_record_under_retained_lock(
            &guard,
            ClaimWalOperation::Acquire,
            &test_claim(),
            "2027-01-15T08:00:00Z",
            crate::WalDurability::NoSync,
        )
        .is_err());
        assert_eq!(
            fs::read(claim_wal_path(&displaced)).expect("read A after"),
            a_before
        );
        assert_eq!(
            fs::read(claim_wal_path(&root.0)).expect("read B after"),
            b_before
        );

        drop(guard);
        fs::remove_dir_all(&root.0).expect("remove B");
        fs::rename(displaced, &root.0).expect("restore A");
    }
    #[cfg(unix)]
    #[test]
    fn canonical_guard_rejects_caller_symlink_alias_and_remains_bound_to_a() {
        let sandbox = TestRoot::new();
        let authority_a = sandbox.0.join("authority-a");
        let authority_b = sandbox.0.join("authority-b");
        let selected = sandbox.0.join("selected");
        fs::create_dir_all(claim_wal_path(&authority_a).parent().expect("A WAL parent"))
            .expect("create A WAL parent");
        fs::create_dir_all(claim_wal_path(&authority_b).parent().expect("B WAL parent"))
            .expect("create B WAL parent");
        fs::write(claim_wal_path(&authority_a), b"torn").expect("write torn A WAL");
        let b_sentinel = b"B must remain untouched";
        fs::write(claim_wal_path(&authority_b), b_sentinel).expect("write B sentinel");
        symlink(&authority_a, &selected).expect("point selected root at A");

        let guard = acquire_claim_wal_retained_lock(&authority_a).expect("lock canonical A");
        let alias_rejected = guard
            .validate(&selected)
            .expect_err("caller symlink aliases must be rejected even when they resolve to A");
        assert!(matches!(alias_rejected, ClaimWalReadError::ReadWal { .. }));
        fs::remove_file(&selected).expect("unlink selected root");
        symlink(&authority_b, &selected).expect("repoint selected root at B");

        let rejected = recover_claim_wal_under_retained_lock(&selected, &guard, true)
            .expect_err("retargeted caller alias must be rejected");
        assert!(matches!(rejected, ClaimWalReadError::ReadWal { .. }));
        assert_eq!(
            fs::read(claim_wal_path(&authority_b)).expect("read B after rejection"),
            b_sentinel
        );

        let appended = append_claim_wal_record_under_retained_lock(
            &guard,
            ClaimWalOperation::Acquire,
            &test_claim(),
            "2027-01-15T08:00:00Z",
            crate::WalDurability::NoSync,
        )
        .expect("repair and append through canonical guard");
        assert_eq!(appended.wal_path, claim_wal_path(&authority_a));
        let recovery = recover_claim_wal_for_guard(&guard, false).expect("recover canonical A");
        assert!(recovery.last_observed_seq >= appended.seq);
        assert_eq!(
            fs::read(claim_wal_path(&authority_b)).expect("read B after append"),
            b_sentinel
        );

        let rotation = rotate_claim_wal_if_needed_under_retained_lock(
            &guard,
            "2027-01-15T08:01:00Z",
            &ClaimWalRotationOptions {
                max_wal_bytes: u64::MAX,
                max_records: 0,
                max_replay_millis: u64::MAX,
            },
        )
        .expect("rotate canonical A");
        assert!(rotation.rotated);
        assert_eq!(rotation.wal_path, claim_wal_path(&authority_a));
        for path in [
            rotation.snapshot_path,
            rotation.archived_wal_path,
            rotation.generation_path,
            rotation.manifest_path,
        ] {
            let path = path.expect("rotation output path");
            assert!(
                path.starts_with(&authority_a),
                "{} escaped A",
                path.display()
            );
            assert!(path.exists(), "{} was not persisted", path.display());
        }
        assert_eq!(
            fs::read(claim_wal_path(&authority_b)).expect("read B after rotation"),
            b_sentinel
        );
        assert!(!claim_wal_snapshot_dir(&authority_b).exists());
        assert!(!claim_wal_archive_dir(&authority_b).exists());
        assert!(!claim_wal_manifest_path(&authority_b).exists());
    }

    #[cfg(unix)]
    fn retained_decoded_checkpoint(root: &Path) -> (ClaimWalRetainedLock, DecodedClaimWalPrefix) {
        let guard = acquire_claim_wal_retained_lock(root).expect("retain checkpoint lock");
        let relative = Path::new(CLAIM_WAL_RELATIVE_PATH);
        let file = guard
            .root
            .open_leaf_read(relative, crate::retained_dir::RetainedLeafPolicy::Authority)
            .expect("open active checkpoint WAL");
        let active_wal = retain_open_claim_wal_leaf(&guard.root, relative, file, u64::MAX)
            .expect("retain active checkpoint WAL");
        let authority = ClaimWalDecodeAuthority {
            guard: &guard,
            active_wal: &active_wal,
        };
        let decoded = decode_prefix(&guard.wal_path, &active_wal.bytes, Some(&authority));
        assert_eq!(decoded.recovery.stop_reason, ClaimWalStopReason::CleanEof);
        assert!(decoded.checkpoint_witness.is_some());
        (guard, decoded)
    }

    #[cfg(unix)]
    fn rotated_checkpoint_test_root() -> TestRoot {
        let root = TestRoot::new();
        append_claim_wal_record(
            &root.0,
            ClaimWalOperation::Acquire,
            &test_claim(),
            "2027-01-15T08:00:00Z",
        )
        .expect("seed checkpoint WAL");
        rotate_claim_wal_if_needed(
            &root.0,
            "2027-01-15T08:01:00Z",
            &ClaimWalRotationOptions {
                max_wal_bytes: u64::MAX,
                max_records: 0,
                max_replay_millis: u64::MAX,
            },
        )
        .expect("rotate checkpoint WAL");
        root
    }

    #[cfg(unix)]
    #[test]
    fn retained_checkpoint_witness_detects_generation_hiding_after_decode() {
        let root = rotated_checkpoint_test_root();
        let (guard, decoded) = retained_decoded_checkpoint(&root.0);
        let witness = decoded
            .checkpoint_witness
            .as_ref()
            .expect("retained checkpoint witness");
        let generation = root.0.join(&witness.generation.relative_path);
        fs::rename(&generation, root.0.join("generation-hidden-after-decode"))
            .expect("hide retained generation");

        assert!(witness.validate(&guard).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn retained_checkpoint_witness_detects_generation_replacement_after_decode() {
        let root = rotated_checkpoint_test_root();
        let (guard, decoded) = retained_decoded_checkpoint(&root.0);
        let witness = decoded
            .checkpoint_witness
            .as_ref()
            .expect("retained checkpoint witness");
        let generation = root.0.join(&witness.generation.relative_path);
        fs::rename(&generation, root.0.join("generation-original-after-decode"))
            .expect("displace retained generation");
        fs::write(&generation, &witness.generation.bytes).expect("replace generation by name");

        assert!(witness.validate(&guard).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn retained_checkpoint_witness_ignores_legacy_manifest_hiding_after_decode() {
        let root = rotated_checkpoint_test_root();
        let (guard, decoded) = retained_decoded_checkpoint(&root.0);
        let witness = decoded
            .checkpoint_witness
            .as_ref()
            .expect("retained checkpoint witness");
        fs::rename(
            claim_wal_manifest_path(&root.0),
            root.0.join("manifest-hidden-after-decode"),
        )
        .expect("hide projection-only manifest");

        witness
            .validate(&guard)
            .expect("manifest is not checkpoint authority");
    }

    #[cfg(unix)]
    #[test]
    fn retained_checkpoint_witness_detects_archive_hiding_after_decode() {
        let root = rotated_checkpoint_test_root();
        let (guard, decoded) = retained_decoded_checkpoint(&root.0);
        let witness = decoded
            .checkpoint_witness
            .as_ref()
            .expect("retained checkpoint witness");
        let archive = root.0.join(&witness.archive.relative_path);
        fs::rename(&archive, root.0.join("archive-hidden-after-decode"))
            .expect("hide retained archive");

        assert!(witness.validate(&guard).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn retained_checkpoint_witness_detects_archive_replacement_after_decode() {
        let root = rotated_checkpoint_test_root();
        let (guard, decoded) = retained_decoded_checkpoint(&root.0);
        let witness = decoded
            .checkpoint_witness
            .as_ref()
            .expect("retained checkpoint witness");
        let archive = root.0.join(&witness.archive.relative_path);
        fs::rename(&archive, root.0.join("archive-original-after-decode"))
            .expect("displace retained archive");
        fs::write(&archive, &witness.archive.bytes).expect("replace archive by name");

        assert!(witness.validate(&guard).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn returned_pristine_authority_rejects_later_checkpoint_residue() {
        let root = TestRoot::new();
        let recovery = recover_claim_wal(&root.0, false).expect("recover pristine claim store");
        let authority = recovery
            .retained_authority
            .as_ref()
            .expect("returned pristine authority");
        authority
            .revalidate()
            .expect("pristine authority initially validates");
        fs::create_dir_all(claim_wal_snapshot_dir(&root.0))
            .expect("introduce canonical snapshot residue");

        authority
            .revalidate()
            .expect_err("pristine authority must reject later checkpoint residue");
    }

    #[cfg(unix)]
    #[test]
    fn returned_recovery_authority_keeps_exact_checkpoint_bundle_live() {
        let root = rotated_checkpoint_test_root();
        let recovery =
            recover_claim_wal(&root.0, false).expect("recover retained checkpoint bundle");
        let authority = recovery
            .retained_authority
            .as_ref()
            .expect("returned retained recovery authority");
        authority
            .revalidate()
            .expect("returned authority initially validates");
        let generation = recovery
            .checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.payload.generation_path.as_deref())
            .expect("selected generation path");
        fs::rename(
            root.0.join(generation),
            root.0.join("generation-hidden-after-return"),
        )
        .expect("hide generation after recovery return");

        authority
            .revalidate()
            .expect_err("returned authority must retain and revalidate exact bundle");
    }

    #[cfg(unix)]
    fn test_claim() -> ClaimContract {
        ClaimContract {
            id: ClaimId("claim.story.C2.C2".to_string()),
            contract_ref: RepoPath("claims-active/claim-story-C2-C2.yaml".to_string()),
            claim: ClaimIdentity {
                claimant_principal_id: None,
                kind: ClaimKind::Story,
                claimant_agent_id: StableId("luna".to_string()),
                claimant_role: ActorRole::Worker,
                registry_ref: None,
            },
            scope: ClaimScope {
                kind: ClaimScopeKind::Story,
                id: ScopeId("C2".to_string()),
                product_area: None,
                paths: vec![RepoPath("src/lib.rs".to_string())],
            },
            lease: ClaimLease {
                acquired_at: "2027-01-15T08:00:00Z".to_string(),
                last_heartbeat_at: "2027-01-15T08:00:00Z".to_string(),
                expires_at: "2027-01-15T08:10:00Z".to_string(),
                ttl_seconds: 600,
                heartbeat_interval_seconds: 120,
                expected_state_version: 0,
            },
            status: ClaimStatusRecord {
                value: ClaimStatus::Active,
                evaluated_at: "2027-01-15T08:00:00Z".to_string(),
                reason_code: None,
            },
            expiry_policy: ExpiryPolicy {
                on_expiry: ExpiryAction::RecordHandoffRequest,
                handoff_required: true,
                release_without_handoff_allowed: false,
                reclaim_policy: ReclaimPolicy::DriverReview,
                handoff_request_ref: Some(RepoPath(
                    "contracts/requests/claim-expiry-handoff-request.yaml".to_string(),
                )),
            },
            evidence_refs: Vec::new(),
        }
    }
}
