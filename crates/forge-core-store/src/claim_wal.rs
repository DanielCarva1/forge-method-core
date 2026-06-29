use forge_core_contracts::claim::ClaimContract;
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAGIC: [u8; 4] = *b"FMW1";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 24;
const HEADER_CRC_OFFSET: usize = 20;
const TRAILER_LEN: usize = 4;
const FLAG_SKIPPABLE_UNKNOWN: u16 = 0b0000_0001;
const FLAG_PAYLOAD_JSON: u16 = 0b0000_0100;
const ALLOWED_FLAGS: u16 = FLAG_SKIPPABLE_UNKNOWN | FLAG_PAYLOAD_JSON;
const DEFAULT_MAX_PAYLOAD_LEN: u32 = 16 * 1024 * 1024;

pub const CLAIM_WAL_RELATIVE_PATH: &str = "wal/claims.fmw1";
pub const CLAIM_WAL_LOCK_RELATIVE_PATH: &str = "locks/claims.wal.lock";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimWalOperation {
    Acquire,
    Release,
    Heartbeat,
    HandoffRecorded,
}

impl ClaimWalOperation {
    #[must_use]
    pub fn record_type(self) -> u8 {
        match self {
            Self::Acquire => 1,
            Self::Release => 2,
            Self::Heartbeat => 3,
            Self::HandoffRecorded => 5,
        }
    }

    fn from_record_type(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Acquire),
            2 => Some(Self::Release),
            3 => Some(Self::Heartbeat),
            5 => Some(Self::HandoffRecorded),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalAppendResult {
    pub wal_path: PathBuf,
    pub seq: u64,
    pub bytes_appended: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimWalRecovery {
    pub wal_path: PathBuf,
    pub records: Vec<ClaimWalRecord>,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimWalAppendError {
    CreateDir { path: PathBuf, source: String },
    OpenLock { path: PathBuf, source: String },
    Lock { path: PathBuf, source: String },
    OpenWal { path: PathBuf, source: String },
    ReadWal { path: PathBuf, source: String },
    RepairWal { path: PathBuf, source: String },
    Serialize { source: String },
    PayloadTooLarge { byte_len: usize, max_byte_len: u32 },
    SequenceOverflow { last_seq: u64 },
    WriteWal { path: PathBuf, source: String },
    SyncWal { path: PathBuf, source: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimWalReadError {
    OpenLock { path: PathBuf, source: String },
    Lock { path: PathBuf, source: String },
    ReadWal { path: PathBuf, source: String },
    RepairWal { path: PathBuf, source: String },
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

#[must_use]
pub fn claim_wal_path(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(CLAIM_WAL_RELATIVE_PATH)
}

#[must_use]
pub fn claim_wal_lock_path(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(CLAIM_WAL_LOCK_RELATIVE_PATH)
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
    let state_root = state_root.as_ref();
    let wal_path = claim_wal_path(state_root);
    let lock_path = claim_wal_lock_path(state_root);

    create_parent_dir(&lock_path).map_err(|source| ClaimWalAppendError::CreateDir {
        path: lock_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf(),
        source: source.to_string(),
    })?;
    let lock = lock_exclusive(&lock_path).map_err(|source| match source.kind() {
        io::ErrorKind::Other => ClaimWalAppendError::Lock {
            path: lock_path.clone(),
            source: source.to_string(),
        },
        _ => ClaimWalAppendError::OpenLock {
            path: lock_path.clone(),
            source: source.to_string(),
        },
    })?;

    create_parent_dir(&wal_path).map_err(|source| ClaimWalAppendError::CreateDir {
        path: wal_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf(),
        source: source.to_string(),
    })?;
    let recovery = recover_claim_wal_under_lock(&wal_path, true).map_err(|source| {
        ClaimWalAppendError::ReadWal {
            path: wal_path.clone(),
            source: source.to_string(),
        }
    })?;
    let last_seq = recovery.records.last().map_or(0, |record| record.seq);
    let seq = last_seq
        .checked_add(1)
        .ok_or(ClaimWalAppendError::SequenceOverflow { last_seq })?;
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
    let record_bytes = encode_record(seq, operation.record_type(), &payload_bytes)?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&wal_path)
        .map_err(|source| ClaimWalAppendError::OpenWal {
            path: wal_path.clone(),
            source: source.to_string(),
        })?;
    file.write_all(&record_bytes)
        .map_err(|source| ClaimWalAppendError::WriteWal {
            path: wal_path.clone(),
            source: source.to_string(),
        })?;
    file.sync_data()
        .map_err(|source| ClaimWalAppendError::SyncWal {
            path: wal_path.clone(),
            source: source.to_string(),
        })?;
    drop(lock);

    Ok(ClaimWalAppendResult {
        wal_path,
        seq,
        bytes_appended: u64::try_from(record_bytes.len()).unwrap_or(u64::MAX),
    })
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

fn recover_claim_wal_under_lock(wal_path: &Path, repair: bool) -> io::Result<ClaimWalRecovery> {
    let bytes = match fs::read(wal_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    let original_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let mut recovery = decode_prefix(wal_path, &bytes);
    if repair && recovery.last_good_offset < original_len {
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
    SkippedUnknown {
        next_offset: usize,
    },
    Stop(ClaimWalStopReason),
}

fn decode_prefix(wal_path: &Path, bytes: &[u8]) -> ClaimWalRecovery {
    let original_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let mut records = Vec::new();
    let mut offset = 0usize;
    let mut expected_seq = 1u64;
    let mut stop_reason = ClaimWalStopReason::CleanEof;

    loop {
        let remaining = bytes.len().saturating_sub(offset);
        if remaining == 0 {
            break;
        }
        match decode_record_at(bytes, offset, expected_seq) {
            DecodeRecordOutcome::Known {
                record,
                next_offset,
            } => {
                records.push(*record);
                offset = next_offset;
                expected_seq = expected_seq.saturating_add(1);
            }
            DecodeRecordOutcome::SkippedUnknown { next_offset } => {
                offset = next_offset;
                expected_seq = expected_seq.saturating_add(1);
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
        last_good_offset: u64::try_from(offset).unwrap_or(u64::MAX),
        original_len,
        repaired: false,
        stop_reason,
    }
}

fn decode_record_at(bytes: &[u8], offset: usize, expected_seq: u64) -> DecodeRecordOutcome {
    if bytes.len().saturating_sub(offset) < HEADER_LEN {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::TruncatedHeader);
    }

    let header = &bytes[offset..offset + HEADER_LEN];
    if header[0..4] != MAGIC || header[4] != VERSION {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::InvalidHeader);
    }
    let flags = u16::from_le_bytes([header[6], header[7]]);
    if flags & !ALLOWED_FLAGS != 0 || flags & FLAG_PAYLOAD_JSON == 0 {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::InvalidHeader);
    }
    let record_type = header[5];
    let operation = ClaimWalOperation::from_record_type(record_type);
    let seq = u64::from_le_bytes(header[8..16].try_into().expect("8 byte seq"));
    if seq != expected_seq {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::SequenceGap);
    }
    let payload_len = u32::from_le_bytes(header[16..20].try_into().expect("4 byte payload length"));
    if payload_len > DEFAULT_MAX_PAYLOAD_LEN {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::PayloadTooLarge);
    }
    let header_crc = u32::from_le_bytes(
        header[HEADER_CRC_OFFSET..HEADER_LEN]
            .try_into()
            .expect("4 byte header crc"),
    );
    if crc32c::crc32c(&header[0..HEADER_CRC_OFFSET]) != header_crc {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::InvalidHeader);
    }

    let Ok(payload_len_usize) = usize::try_from(payload_len) else {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::PayloadTooLarge);
    };
    let Some(record_end) = offset
        .checked_add(HEADER_LEN)
        .and_then(|value| value.checked_add(payload_len_usize))
        .and_then(|value| value.checked_add(TRAILER_LEN))
    else {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::PayloadTooLarge);
    };
    if record_end > bytes.len() {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::TruncatedPayload);
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
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::PayloadChecksumMismatch);
    }
    let Some(operation) = operation else {
        if flags & FLAG_SKIPPABLE_UNKNOWN != 0 {
            return DecodeRecordOutcome::SkippedUnknown {
                next_offset: record_end,
            };
        }
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::UnsupportedRecordType);
    };
    let Ok(decoded_payload) = serde_json::from_slice::<ClaimWalPayload>(payload) else {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::PayloadDecodeFailed);
    };
    if decoded_payload.operation != operation {
        return DecodeRecordOutcome::Stop(ClaimWalStopReason::PayloadDecodeFailed);
    }
    DecodeRecordOutcome::Known {
        record: Box::new(ClaimWalRecord {
            seq,
            operation,
            payload: decoded_payload,
            offset: u64::try_from(offset).unwrap_or(u64::MAX),
            record_len: u64::try_from(record_end - offset).unwrap_or(u64::MAX),
        }),
        next_offset: record_end,
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
