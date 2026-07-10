//! Durable single-use replay reservations for execution admission.
//!
//! Replay state intentionally lives in a WAL separate from claim and effect
//! state. Every authority-bearing transition is framed with a sequence number
//! and CRC32C checksums. Reserve and consume operations hold one exclusive
//! lock across recovery, conflict/CAS checks, append, and `fsync`.
//!
//! The key stored on disk is **pseudonymous**, not confidential: it is an
//! unkeyed SHA-256 digest, so low-entropy inputs remain vulnerable to offline
//! guessing. The state root is a trust boundary and must be an existing,
//! operator-protected directory. An attacker who can rewrite this WAL and its
//! initialization manifest can rewrite replay authority state.
//! Runtime reservation requires an explicitly initialized manifest/WAL pair
//! and never recreates a missing pair. The initializer cannot distinguish a
//! first bootstrap from wholesale deletion or rollback, however, so enforced
//! deployments must restrict initialization and eventually anchor an epoch or
//! head outside this state root.
//!
//! Replay is deliberately capacity-bounded because every mutation currently
//! replays the WAL. Compaction/rotation is deferred; reaching either hard
//! limit fails closed rather than allowing unbounded allocation or replay
//! cost.
//!
//! `commit_digest` must address the caller's immutable canonical commit
//! descriptor (effect/WAL scope and content), not an ad-hoc label. This module
//! enforces canonical SHA-256 token syntax and equality; construction of that
//! descriptor belongs to the execution kernel.

use forge_core_contracts::PrincipalId;
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Component, Path, PathBuf};

const MAGIC: [u8; 4] = *b"FMR1";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 24;
const HEADER_CRC_OFFSET: usize = 20;
const TRAILER_LEN: usize = 4;
const FLAG_PAYLOAD_JSON: u16 = 0b0000_0100;
const MAX_PAYLOAD_LEN: u32 = 1024 * 1024;
const RECORD_TYPE_RESERVE: u8 = 1;
const RECORD_TYPE_CONSUME: u8 = 2;
const KEY_HASH_DOMAIN: &[u8] = b"forge-method:replay-nonce-key:v1\0";
const MANIFEST_MAX_BYTES: u64 = 4 * 1024;

pub const REPLAY_WAL_RELATIVE_PATH: &str = "wal/replay.fmr1";
pub const REPLAY_WAL_LOCK_RELATIVE_PATH: &str = "locks/replay.wal.lock";
pub const REPLAY_WAL_MANIFEST_RELATIVE_PATH: &str = "replay-wal.manifest.json";
pub const REPLAY_WAL_SCHEMA_VERSION: &str = "0.1";
pub const REPLAY_WAL_MAX_BYTES: u64 = 8 * 1024 * 1024;
pub const REPLAY_WAL_MAX_RECORDS: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplayWalManifest {
    schema_version: String,
    format_magic: String,
    wal_relative_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReplayWalOperation {
    Reserve,
    Consume,
}

impl ReplayWalOperation {
    const fn record_type(self) -> u8 {
        match self {
            Self::Reserve => RECORD_TYPE_RESERVE,
            Self::Consume => RECORD_TYPE_CONSUME,
        }
    }

    const fn from_record_type(value: u8) -> Option<Self> {
        match value {
            RECORD_TYPE_RESERVE => Some(Self::Reserve),
            RECORD_TYPE_CONSUME => Some(Self::Consume),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReplayReservationState {
    Reserved,
    Consumed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ReplayWalPayload {
    pub schema_version: String,
    pub operation: ReplayWalOperation,
    pub key_hash: String,
    pub intent_digest: String,
    pub commit_digest: String,
    pub revision: u64,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ReplayWalRecord {
    pub seq: u64,
    pub operation: ReplayWalOperation,
    pub payload: ReplayWalPayload,
    pub offset: u64,
    pub record_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ReplayReservation {
    pub key_hash: String,
    pub intent_digest: String,
    pub commit_digest: String,
    pub revision: u64,
    pub state: ReplayReservationState,
    pub reserved_seq: u64,
    pub consumed_seq: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReplayWalStopReason {
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
    InvalidTransition,
    RecordCapacityExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ReplayWalRecovery {
    pub wal_path: PathBuf,
    pub records: Vec<ReplayWalRecord>,
    pub reservations: BTreeMap<String, ReplayReservation>,
    pub last_observed_seq: u64,
    pub valid_record_count: usize,
    pub last_good_offset: u64,
    pub original_len: u64,
    pub repaired: bool,
    pub stop_reason: ReplayWalStopReason,
}

impl ReplayWalRecovery {
    /// Whether the WAL is an authoritative, fully verified prefix after any
    /// requested safe torn-tail repair.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        matches!(self.stop_reason, ReplayWalStopReason::CleanEof)
            || (self.repaired && is_safe_torn_tail(self.stop_reason))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ReplayReserveResult {
    pub wal_path: PathBuf,
    pub seq: u64,
    pub bytes_appended: u64,
    pub reservation: ReplayReservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ReplayConsumeResult {
    pub wal_path: PathBuf,
    pub seq: u64,
    pub bytes_appended: u64,
    pub appended: bool,
    pub reservation: ReplayReservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ReplayWalInitializationResult {
    pub state_root: PathBuf,
    pub wal_path: PathBuf,
    pub manifest_path: PathBuf,
    pub initialized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplayWalCapacityKind {
    Bytes,
    Records,
}

struct PreparedConsume {
    seq: u64,
    bytes: Vec<u8>,
    reservation: ReplayReservation,
}

struct ReplayGuardState {
    _replay_lock: ReplayWalLock,
    wal_path: PathBuf,
    reserved: ReplayReservation,
    prepared: PreparedConsume,
}

/// Replay authority guard intended to be held across execution admission and
/// effect commit.
///
/// Construction requires an already-held [`crate::EffectStoreLock`], enforcing
/// the global order `effect lock -> replay lock`. The guard retains a borrow of
/// that effect lock and owns the replay lock until [`Self::consume`] or drop.
/// A kernel can therefore evaluate P4a and commit its effect while neither the
/// effect scope nor replay reservation can change underneath it. This store
/// primitive does not prove that the caller chose the canonical effect scope
/// or actually committed before calling [`Self::consume`]; the future kernel
/// must derive that scope and enforce the transaction lifecycle.
pub struct ReplayCommitGuard<'a> {
    effect_lock: &'a crate::EffectStoreLock,
    state: ReplayGuardState,
}

impl fmt::Debug for ReplayCommitGuard<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReplayCommitGuard")
            .field("effect_lock_path", &self.effect_lock.path())
            .field("wal_path", &self.state.wal_path)
            .field("reserved", &self.state.reserved)
            .field("consume_seq", &self.state.prepared.seq)
            .finish_non_exhaustive()
    }
}

impl ReplayCommitGuard<'_> {
    #[must_use]
    pub fn effect_lock(&self) -> &crate::EffectStoreLock {
        self.effect_lock
    }

    #[must_use]
    pub fn reservation(&self) -> &ReplayReservation {
        &self.state.reserved
    }

    /// Durably consume the guarded replay reservation after the effect commit.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayWalError`] if the prepared consume frame cannot be
    /// appended and synced. Both locks remain held until this call returns.
    pub fn consume(self) -> Result<ReplayConsumeResult, ReplayWalError> {
        append_and_sync(&self.state.wal_path, &self.state.prepared.bytes)?;
        Ok(ReplayConsumeResult {
            wal_path: self.state.wal_path.clone(),
            seq: self.state.prepared.seq,
            bytes_appended: u64::try_from(self.state.prepared.bytes.len()).unwrap_or(u64::MAX),
            appended: true,
            reservation: self.state.prepared.reservation.clone(),
        })
    }
}

/// Owned replay authority guard for an opaque prepared kernel transaction.
///
/// Unlike [`ReplayCommitGuard`], this type consumes and owns the effect lock,
/// so it can safely cross function boundaries without a self-referential
/// borrow. Field order is intentional: replay state drops before the effect
/// lock, releasing locks in reverse acquisition order.
pub struct OwnedReplayCommitGuard {
    state: ReplayGuardState,
    effect_lock: crate::EffectStoreLock,
}

/// Replay-consume result that deliberately retains only the effect lock. The
/// replay lock has already been released, allowing the kernel to append the
/// effect-WAL completion marker before releasing the effect boundary.
pub struct ConsumedReplayEffectGuard {
    effect_lock: crate::EffectStoreLock,
    result: ReplayConsumeResult,
}

impl fmt::Debug for ConsumedReplayEffectGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConsumedReplayEffectGuard")
            .field("effect_lock_path", &self.effect_lock.path())
            .field("result", &self.result)
            .finish_non_exhaustive()
    }
}

impl ConsumedReplayEffectGuard {
    #[must_use]
    pub fn effect_lock(&self) -> &crate::EffectStoreLock {
        &self.effect_lock
    }

    #[must_use]
    pub fn result(&self) -> &ReplayConsumeResult {
        &self.result
    }

    #[must_use]
    pub fn into_result(self) -> ReplayConsumeResult {
        self.result
    }
}

impl fmt::Debug for OwnedReplayCommitGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnedReplayCommitGuard")
            .field("effect_lock_path", &self.effect_lock.path())
            .field("wal_path", &self.state.wal_path)
            .field("reserved", &self.state.reserved)
            .field("consume_seq", &self.state.prepared.seq)
            .finish_non_exhaustive()
    }
}

impl OwnedReplayCommitGuard {
    #[must_use]
    pub fn effect_lock(&self) -> &crate::EffectStoreLock {
        &self.effect_lock
    }

    #[must_use]
    pub fn reservation(&self) -> &ReplayReservation {
        &self.state.reserved
    }

    /// Durably consume the guarded replay reservation after an effect commit.
    /// P4b.2c uses the retaining variant below so it can acknowledge completion
    /// in the effect WAL before releasing the effect lock.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayWalError`] when the prepared consume frame cannot be
    /// appended and synced.
    pub fn consume(self) -> Result<ReplayConsumeResult, ReplayWalError> {
        self.consume_retaining_effect_lock()
            .map(ConsumedReplayEffectGuard::into_result)
    }

    /// Durably consume replay, release the replay lock, and retain the effect
    /// lock for the caller's final effect-WAL acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayWalError`] when the prepared consume frame cannot be
    /// appended and synced. On error both owned locks are released.
    pub fn consume_retaining_effect_lock(
        self,
    ) -> Result<ConsumedReplayEffectGuard, ReplayWalError> {
        let Self { state, effect_lock } = self;
        append_and_sync(&state.wal_path, &state.prepared.bytes)?;
        let result = ReplayConsumeResult {
            wal_path: state.wal_path.clone(),
            seq: state.prepared.seq,
            bytes_appended: u64::try_from(state.prepared.bytes.len()).unwrap_or(u64::MAX),
            appended: true,
            reservation: state.prepared.reservation.clone(),
        };
        drop(state);
        Ok(ConsumedReplayEffectGuard {
            effect_lock,
            result,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplayWalError {
    StateRootUnavailable {
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
    InvalidInput {
        field: &'static str,
        reason: &'static str,
    },
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
    ReadWal {
        path: PathBuf,
        source: String,
    },
    RepairWal {
        path: PathBuf,
        source: String,
    },
    RecoveryStopped {
        stop_reason: ReplayWalStopReason,
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
    RevisionOverflow {
        key_hash: String,
        revision: u64,
    },
    OpenWal {
        path: PathBuf,
        source: String,
    },
    WriteWal {
        path: PathBuf,
        source: String,
    },
    SyncWal {
        path: PathBuf,
        source: String,
    },
    CapacityExceeded {
        kind: ReplayWalCapacityKind,
        limit: u64,
        observed: u64,
    },
    DuplicateNonce {
        key_hash: String,
        revision: u64,
        state: ReplayReservationState,
    },
    ReservationMissing {
        key_hash: String,
    },
    IntentDigestMismatch {
        key_hash: String,
    },
    CommitDigestMismatch {
        key_hash: String,
    },
    RevisionMismatch {
        key_hash: String,
        expected: u64,
        actual: u64,
    },
    ReservationNotReserved {
        key_hash: String,
        state: ReplayReservationState,
    },
    EffectLockScopeMismatch {
        expected: PathBuf,
        actual: PathBuf,
    },
    EffectLockOrderViolation {
        path: PathBuf,
    },
}

impl fmt::Display for ReplayWalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateRootUnavailable { path, source } => write!(
                formatter,
                "replay state root {} is unavailable or untrusted: {source}",
                path.display()
            ),
            Self::NotInitialized { state_root } => write!(
                formatter,
                "replay WAL is not initialized under {}",
                state_root.display()
            ),
            Self::InitializationMismatch {
                manifest_path,
                manifest_exists,
                wal_path,
                wal_exists,
            } => write!(
                formatter,
                "replay initialization mismatch: manifest {} exists={manifest_exists}, WAL {} exists={wal_exists}",
                manifest_path.display(),
                wal_path.display()
            ),
            Self::InvalidManifest { path, source } => write!(
                formatter,
                "invalid replay WAL manifest {}: {source}",
                path.display()
            ),
            Self::InvalidInput { field, reason } => {
                write!(formatter, "invalid replay {field}: {reason}")
            }
            Self::CreateDir { path, source } => {
                write!(formatter, "create replay WAL directory {} failed: {source}", path.display())
            }
            Self::OpenLock { path, source } => {
                write!(formatter, "open replay WAL lock {} failed: {source}", path.display())
            }
            Self::Lock { path, source } => {
                write!(formatter, "lock replay WAL {} failed: {source}", path.display())
            }
            Self::ReadWal { path, source } => {
                write!(formatter, "read replay WAL {} failed: {source}", path.display())
            }
            Self::RepairWal { path, source } => {
                write!(formatter, "repair replay WAL {} failed: {source}", path.display())
            }
            Self::RecoveryStopped {
                stop_reason,
                last_good_offset,
                original_len,
            } => write!(
                formatter,
                "replay WAL recovery stopped with {stop_reason:?} at {last_good_offset}/{original_len}"
            ),
            Self::Serialize { source } => {
                write!(formatter, "serialize replay WAL payload failed: {source}")
            }
            Self::PayloadTooLarge {
                byte_len,
                max_byte_len,
            } => write!(
                formatter,
                "replay WAL payload length {byte_len} exceeds max {max_byte_len}"
            ),
            Self::SequenceOverflow { last_seq } => {
                write!(formatter, "replay WAL sequence overflow after {last_seq}")
            }
            Self::RevisionOverflow { key_hash, revision } => write!(
                formatter,
                "replay reservation {key_hash} revision overflow after {revision}"
            ),
            Self::OpenWal { path, source } => {
                write!(formatter, "open replay WAL {} failed: {source}", path.display())
            }
            Self::WriteWal { path, source } => {
                write!(formatter, "write replay WAL {} failed: {source}", path.display())
            }
            Self::SyncWal { path, source } => {
                write!(formatter, "sync replay WAL {} failed: {source}", path.display())
            }
            Self::CapacityExceeded {
                kind,
                limit,
                observed,
            } => write!(
                formatter,
                "replay WAL {kind:?} capacity exceeded: limit {limit}, observed {observed}; compaction is not implemented"
            ),
            Self::DuplicateNonce {
                key_hash,
                revision,
                state,
            } => write!(
                formatter,
                "replay nonce {key_hash} was already observed at revision {revision} ({state:?})"
            ),
            Self::ReservationMissing { key_hash } => {
                write!(formatter, "replay reservation {key_hash} does not exist")
            }
            Self::IntentDigestMismatch { key_hash } => {
                write!(formatter, "replay reservation {key_hash} intent digest mismatch")
            }
            Self::CommitDigestMismatch { key_hash } => {
                write!(formatter, "replay reservation {key_hash} commit digest mismatch")
            }
            Self::RevisionMismatch {
                key_hash,
                expected,
                actual,
            } => write!(
                formatter,
                "replay reservation {key_hash} revision mismatch: expected {expected}, actual {actual}"
            ),
            Self::ReservationNotReserved { key_hash, state } => write!(
                formatter,
                "replay reservation {key_hash} is not reserved ({state:?})"
            ),
            Self::EffectLockScopeMismatch { expected, actual } => write!(
                formatter,
                "effect lock scope mismatch: expected {}, actual {}",
                expected.display(),
                actual.display()
            ),
            Self::EffectLockOrderViolation { path } => write!(
                formatter,
                "effect lock {} aliases the replay lock; required order is effect lock then replay lock",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ReplayWalError {}

#[must_use]
pub fn replay_wal_path(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(REPLAY_WAL_RELATIVE_PATH)
}

#[must_use]
pub fn replay_wal_lock_path(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(REPLAY_WAL_LOCK_RELATIVE_PATH)
}

#[must_use]
pub fn replay_wal_manifest_path(state_root: impl AsRef<Path>) -> PathBuf {
    state_root.as_ref().join(REPLAY_WAL_MANIFEST_RELATIVE_PATH)
}

/// Durably initialize an empty replay WAL and its out-of-WAL manifest.
///
/// The supplied state root must already exist. Initialization syncs both files
/// and their directory chain. Calling this for an already valid pair is an
/// idempotent no-op; either file existing without the other fails closed.
/// Runtime reservation never performs this initialization implicitly, so a
/// missing pair cannot silently reset replay history during admission.
/// This function cannot detect wholesale deletion or rollback of both files;
/// callers must treat reinitialization as an operator-controlled bootstrap or
/// repair action rather than a normal request path.
///
/// # Errors
///
/// Returns [`ReplayWalError`] when the state root is unavailable, the existing
/// initialization is inconsistent, or any create/write/sync step fails.
pub fn initialize_replay_wal(
    state_root: impl AsRef<Path>,
) -> Result<ReplayWalInitializationResult, ReplayWalError> {
    let state_root = trusted_state_root(state_root.as_ref())?;
    let _lock = acquire_replay_lock(&state_root)?;
    let initialized = ensure_replay_initialized_under_lock(&state_root, true)?;
    if !initialized {
        let recovery = recover_replay_wal_under_lock(&replay_wal_path(&state_root), false)?;
        ensure_appendable(&recovery)?;
    }
    Ok(ReplayWalInitializationResult {
        wal_path: replay_wal_path(&state_root),
        manifest_path: replay_wal_manifest_path(&state_root),
        state_root,
        initialized,
    })
}

/// Derive the pseudonymous identity used by the replay WAL.
///
/// Components are domain-separated and length-prefixed before hashing, so
/// ambiguous concatenations cannot alias. The raw nonce, principal, and
/// audience are not written to the WAL. Because this is an unkeyed hash, it
/// does not provide confidentiality for guessable values.
///
/// # Errors
///
/// Returns [`ReplayWalError::InvalidInput`] when any key component is blank.
pub fn replay_nonce_key_hash(
    principal_id: &PrincipalId,
    audience: &str,
    nonce: &str,
) -> Result<String, ReplayWalError> {
    validate_nonblank("principal_id", &principal_id.0)?;
    validate_nonblank("audience", audience)?;
    validate_nonblank("nonce", nonce)?;

    let mut hasher = Sha256::new();
    hasher.update(KEY_HASH_DOMAIN);
    update_length_prefixed(&mut hasher, principal_id.0.as_bytes());
    update_length_prefixed(&mut hasher, audience.as_bytes());
    update_length_prefixed(&mut hasher, nonce.as_bytes());
    Ok(format_sha256(hasher.finalize()))
}

/// Durably reserve a nonce exactly once for canonical intent and commit
/// digests.
///
/// The first reservation always has revision `1`. Any prior observation of
/// the same `(principal_id, audience, nonce)` key is rejected permanently,
/// including after consumption and even when the digest is identical.
///
/// # Errors
///
/// Returns [`ReplayWalError`] if input is invalid, the WAL is corrupt, the
/// nonce was already observed, locking/recovery fails, or the durable append
/// cannot be completed.
pub fn reserve_replay_nonce(
    state_root: impl AsRef<Path>,
    principal_id: &PrincipalId,
    audience: &str,
    nonce: &str,
    intent_digest: &str,
    commit_digest: &str,
) -> Result<ReplayReserveResult, ReplayWalError> {
    validate_sha256_digest(intent_digest)?;
    validate_commit_digest(commit_digest)?;
    let key_hash = replay_nonce_key_hash(principal_id, audience, nonce)?;
    let state_root = trusted_state_root(state_root.as_ref())?;
    let wal_path = replay_wal_path(&state_root);
    let _lock = acquire_replay_lock(&state_root)?;
    ensure_replay_initialized_under_lock(&state_root, false)?;
    let recovery = recover_replay_wal_under_lock(&wal_path, true)?;
    ensure_appendable(&recovery)?;

    if let Some(existing) = recovery.reservations.get(&key_hash) {
        return Err(ReplayWalError::DuplicateNonce {
            key_hash,
            revision: existing.revision,
            state: existing.state,
        });
    }

    let seq = next_sequence(recovery.last_observed_seq)?;
    let payload = ReplayWalPayload {
        schema_version: REPLAY_WAL_SCHEMA_VERSION.to_owned(),
        operation: ReplayWalOperation::Reserve,
        key_hash: key_hash.clone(),
        intent_digest: intent_digest.to_owned(),
        commit_digest: commit_digest.to_owned(),
        revision: 1,
        expected_revision: None,
    };
    let bytes = encode_payload_record(seq, &payload)?;
    ensure_append_capacity(&recovery, bytes.len())?;
    append_and_sync(&wal_path, &bytes)?;

    Ok(ReplayReserveResult {
        wal_path,
        seq,
        bytes_appended: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        reservation: ReplayReservation {
            key_hash,
            intent_digest: intent_digest.to_owned(),
            commit_digest: commit_digest.to_owned(),
            revision: 1,
            state: ReplayReservationState::Reserved,
            reserved_seq: seq,
            consumed_seq: None,
        },
    })
}

/// Consume a reservation with compare-and-swap semantics outside a guarded
/// commit boundary.
///
/// The first successful consume appends revision `expected_revision + 1`.
/// Retrying that exact consume after a lost response is idempotent: the
/// existing consumed reservation is returned without another append, but only
/// when both intent and commit digests match.
///
/// This is a recovery/administrative convenience, **not** a safe kernel commit
/// boundary: it releases the replay lock before any external effect. Runtime
/// mutation must use [`acquire_replay_commit_guard`] and
/// [`ReplayCommitGuard::consume`].
///
/// # Errors
///
/// Returns [`ReplayWalError`] if input is invalid, the reservation is absent,
/// its digest or revision does not match, the WAL is corrupt, or a durable
/// transition cannot be completed.
pub fn consume_replay_nonce_non_boundary(
    state_root: impl AsRef<Path>,
    principal_id: &PrincipalId,
    audience: &str,
    nonce: &str,
    intent_digest: &str,
    commit_digest: &str,
    expected_revision: u64,
) -> Result<ReplayConsumeResult, ReplayWalError> {
    validate_sha256_digest(intent_digest)?;
    validate_commit_digest(commit_digest)?;
    if expected_revision == 0 {
        return Err(ReplayWalError::InvalidInput {
            field: "expected_revision",
            reason: "must be greater than zero",
        });
    }
    let key_hash = replay_nonce_key_hash(principal_id, audience, nonce)?;
    let state_root = trusted_state_root(state_root.as_ref())?;
    let wal_path = replay_wal_path(&state_root);
    let _lock = acquire_replay_lock(&state_root)?;
    ensure_replay_initialized_under_lock(&state_root, false)?;
    let recovery = recover_replay_wal_under_lock(&wal_path, true)?;
    ensure_appendable(&recovery)?;
    consume_replay_recovered(
        wal_path,
        &recovery,
        &key_hash,
        intent_digest,
        commit_digest,
        expected_revision,
    )
}

/// Recovery-only replay consume using the already persisted pseudonymous key
/// hash from a committed effect-WAL receipt. The caller must retain the exact
/// effect lock, preserving the global `effect -> replay` lock order. This API
/// never accepts or persists the raw nonce.
///
/// Retrying an already consumed exact binding is idempotent and appends
/// nothing. It is intended for P4b.2c crash reconciliation, not for initial
/// request admission.
///
/// # Errors
///
/// Returns [`ReplayWalError`] for invalid input, lock-scope mismatch, corrupt
/// replay state, a missing/mismatched reservation, or durable append failure.
#[allow(clippy::too_many_arguments)]
pub fn consume_replay_key_hash_under_effect_lock(
    state_root: impl AsRef<Path>,
    effect_lock: &crate::EffectStoreLock,
    expected_effect_lock_relative_path: &str,
    key_hash: &str,
    intent_digest: &str,
    commit_digest: &str,
    expected_revision: u64,
) -> Result<ReplayConsumeResult, ReplayWalError> {
    validate_sha256_field("key_hash", key_hash)?;
    validate_sha256_digest(intent_digest)?;
    validate_commit_digest(commit_digest)?;
    if expected_revision == 0 {
        return Err(ReplayWalError::InvalidInput {
            field: "expected_revision",
            reason: "must be greater than zero",
        });
    }
    let state_root = trusted_state_root(state_root.as_ref())?;
    validate_effect_lock_scope(&state_root, effect_lock, expected_effect_lock_relative_path)?;
    let wal_path = replay_wal_path(&state_root);
    let _replay_lock = acquire_replay_lock(&state_root)?;
    ensure_replay_initialized_under_lock(&state_root, false)?;
    let recovery = recover_replay_wal_under_lock(&wal_path, true)?;
    ensure_appendable(&recovery)?;
    consume_replay_recovered(
        wal_path,
        &recovery,
        key_hash,
        intent_digest,
        commit_digest,
        expected_revision,
    )
}

fn consume_replay_recovered(
    wal_path: PathBuf,
    recovery: &ReplayWalRecovery,
    key_hash: &str,
    intent_digest: &str,
    commit_digest: &str,
    expected_revision: u64,
) -> Result<ReplayConsumeResult, ReplayWalError> {
    let Some(existing) = recovery.reservations.get(key_hash) else {
        return Err(ReplayWalError::ReservationMissing {
            key_hash: key_hash.to_owned(),
        });
    };
    if existing.intent_digest != intent_digest {
        return Err(ReplayWalError::IntentDigestMismatch {
            key_hash: key_hash.to_owned(),
        });
    }
    if existing.commit_digest != commit_digest {
        return Err(ReplayWalError::CommitDigestMismatch {
            key_hash: key_hash.to_owned(),
        });
    }
    if existing.state == ReplayReservationState::Consumed {
        let reservation_revision = existing.revision.saturating_sub(1);
        if expected_revision == reservation_revision {
            return Ok(ReplayConsumeResult {
                wal_path,
                seq: existing.consumed_seq.unwrap_or(existing.reserved_seq),
                bytes_appended: 0,
                appended: false,
                reservation: existing.clone(),
            });
        }
        return Err(ReplayWalError::RevisionMismatch {
            key_hash: key_hash.to_owned(),
            expected: expected_revision,
            actual: reservation_revision,
        });
    }
    if existing.revision != expected_revision {
        return Err(ReplayWalError::RevisionMismatch {
            key_hash: key_hash.to_owned(),
            expected: expected_revision,
            actual: existing.revision,
        });
    }
    let revision =
        expected_revision
            .checked_add(1)
            .ok_or_else(|| ReplayWalError::RevisionOverflow {
                key_hash: key_hash.to_owned(),
                revision: expected_revision,
            })?;
    let seq = next_sequence(recovery.last_observed_seq)?;
    let payload = ReplayWalPayload {
        schema_version: REPLAY_WAL_SCHEMA_VERSION.to_owned(),
        operation: ReplayWalOperation::Consume,
        key_hash: key_hash.to_owned(),
        intent_digest: intent_digest.to_owned(),
        commit_digest: commit_digest.to_owned(),
        revision,
        expected_revision: Some(expected_revision),
    };
    let bytes = encode_payload_record(seq, &payload)?;
    ensure_append_capacity(recovery, bytes.len())?;
    append_and_sync(&wal_path, &bytes)?;
    Ok(ReplayConsumeResult {
        wal_path,
        seq,
        bytes_appended: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        appended: true,
        reservation: ReplayReservation {
            key_hash: key_hash.to_owned(),
            intent_digest: intent_digest.to_owned(),
            commit_digest: commit_digest.to_owned(),
            revision,
            state: ReplayReservationState::Consumed,
            reserved_seq: existing.reserved_seq,
            consumed_seq: Some(seq),
        },
    })
}

/// Acquire replay authority for a future effect commit while retaining the
/// caller's effect-store lock.
///
/// `expected_effect_lock_relative_path` identifies the exact effect lock scope
/// that the kernel will use. The supplied lock must resolve to that path under
/// the same trusted state root and must not alias the replay lock. Construction
/// validates a still-reserved nonce, both canonical digests, and the CAS
/// revision under the replay lock, then retains both locks until consume/drop.
///
/// # Errors
///
/// Returns [`ReplayWalError`] for an invalid/mismatched effect-lock scope, an
/// unavailable or corrupt replay store, a non-reserved or mismatched binding,
/// or insufficient capacity for the eventual consume record.
#[allow(clippy::too_many_arguments)]
pub fn acquire_replay_commit_guard<'a>(
    state_root: impl AsRef<Path>,
    effect_lock: &'a crate::EffectStoreLock,
    expected_effect_lock_relative_path: &str,
    principal_id: &PrincipalId,
    audience: &str,
    nonce: &str,
    intent_digest: &str,
    commit_digest: &str,
    expected_revision: u64,
) -> Result<ReplayCommitGuard<'a>, ReplayWalError> {
    let state = prepare_replay_guard_state(
        state_root.as_ref(),
        effect_lock,
        expected_effect_lock_relative_path,
        principal_id,
        audience,
        nonce,
        intent_digest,
        commit_digest,
        expected_revision,
    )?;
    Ok(ReplayCommitGuard { effect_lock, state })
}

/// Acquire replay authority while consuming ownership of the already-held
/// effect lock. This is the guard shape used by the kernel's opaque prepared
/// transaction.
///
/// # Errors
///
/// Returns [`ReplayWalError`] under the same fail-closed conditions as
/// [`acquire_replay_commit_guard`].
#[allow(clippy::too_many_arguments)]
pub fn acquire_owned_replay_commit_guard(
    state_root: impl AsRef<Path>,
    effect_lock: crate::EffectStoreLock,
    expected_effect_lock_relative_path: &str,
    principal_id: &PrincipalId,
    audience: &str,
    nonce: &str,
    intent_digest: &str,
    commit_digest: &str,
    expected_revision: u64,
) -> Result<OwnedReplayCommitGuard, ReplayWalError> {
    let state = prepare_replay_guard_state(
        state_root.as_ref(),
        &effect_lock,
        expected_effect_lock_relative_path,
        principal_id,
        audience,
        nonce,
        intent_digest,
        commit_digest,
        expected_revision,
    )?;
    Ok(OwnedReplayCommitGuard { state, effect_lock })
}

#[allow(clippy::too_many_arguments)]
fn prepare_replay_guard_state(
    state_root: &Path,
    effect_lock: &crate::EffectStoreLock,
    expected_effect_lock_relative_path: &str,
    principal_id: &PrincipalId,
    audience: &str,
    nonce: &str,
    intent_digest: &str,
    commit_digest: &str,
    expected_revision: u64,
) -> Result<ReplayGuardState, ReplayWalError> {
    validate_sha256_digest(intent_digest)?;
    validate_commit_digest(commit_digest)?;
    if expected_revision == 0 {
        return Err(ReplayWalError::InvalidInput {
            field: "expected_revision",
            reason: "must be greater than zero",
        });
    }
    let key_hash = replay_nonce_key_hash(principal_id, audience, nonce)?;
    let state_root = trusted_state_root(state_root)?;
    validate_effect_lock_scope(&state_root, effect_lock, expected_effect_lock_relative_path)?;

    // Lock ordering is load-bearing: the EffectStoreLock already exists before
    // replay acquisition and is retained by the borrowed or owned public guard.
    let replay_lock = acquire_replay_lock(&state_root)?;
    ensure_replay_initialized_under_lock(&state_root, false)?;
    let wal_path = replay_wal_path(&state_root);
    let recovery = recover_replay_wal_under_lock(&wal_path, true)?;
    ensure_appendable(&recovery)?;
    let Some(existing) = recovery.reservations.get(&key_hash) else {
        return Err(ReplayWalError::ReservationMissing { key_hash });
    };
    if existing.intent_digest != intent_digest {
        return Err(ReplayWalError::IntentDigestMismatch { key_hash });
    }
    if existing.commit_digest != commit_digest {
        return Err(ReplayWalError::CommitDigestMismatch { key_hash });
    }
    if existing.state != ReplayReservationState::Reserved {
        return Err(ReplayWalError::ReservationNotReserved {
            key_hash,
            state: existing.state,
        });
    }
    if existing.revision != expected_revision {
        return Err(ReplayWalError::RevisionMismatch {
            key_hash,
            expected: expected_revision,
            actual: existing.revision,
        });
    }

    let revision =
        expected_revision
            .checked_add(1)
            .ok_or_else(|| ReplayWalError::RevisionOverflow {
                key_hash: key_hash.clone(),
                revision: expected_revision,
            })?;
    let seq = next_sequence(recovery.last_observed_seq)?;
    let payload = ReplayWalPayload {
        schema_version: REPLAY_WAL_SCHEMA_VERSION.to_owned(),
        operation: ReplayWalOperation::Consume,
        key_hash: key_hash.clone(),
        intent_digest: intent_digest.to_owned(),
        commit_digest: commit_digest.to_owned(),
        revision,
        expected_revision: Some(expected_revision),
    };
    let bytes = encode_payload_record(seq, &payload)?;
    ensure_append_capacity(&recovery, bytes.len())?;
    let consumed = ReplayReservation {
        key_hash,
        intent_digest: intent_digest.to_owned(),
        commit_digest: commit_digest.to_owned(),
        revision,
        state: ReplayReservationState::Consumed,
        reserved_seq: existing.reserved_seq,
        consumed_seq: Some(seq),
    };

    Ok(ReplayGuardState {
        _replay_lock: replay_lock,
        wal_path,
        reserved: existing.clone(),
        prepared: PreparedConsume {
            seq,
            bytes,
            reservation: consumed,
        },
    })
}

/// Recover and verify the replay WAL under its exclusive lock.
///
/// Setting `repair` only permits truncating an incomplete final header or
/// payload. Checksum, sequence, decoding, and transition failures are never
/// rewritten. A returned non-clean recovery is diagnostic only and must not
/// be used as authority state.
///
/// # Errors
///
/// Returns [`ReplayWalError`] when the lock, read, or requested safe repair
/// cannot be completed.
pub fn recover_replay_wal(
    state_root: impl AsRef<Path>,
    repair: bool,
) -> Result<ReplayWalRecovery, ReplayWalError> {
    let state_root = trusted_state_root(state_root.as_ref())?;
    let wal_path = replay_wal_path(&state_root);
    let _lock = acquire_replay_lock(&state_root)?;
    ensure_replay_initialized_under_lock(&state_root, false)?;
    recover_replay_wal_under_lock(&wal_path, repair)
}

fn validate_nonblank(field: &'static str, value: &str) -> Result<(), ReplayWalError> {
    if value.trim().is_empty() {
        return Err(ReplayWalError::InvalidInput {
            field,
            reason: "must not be blank",
        });
    }
    Ok(())
}

fn validate_sha256_digest(value: &str) -> Result<(), ReplayWalError> {
    validate_sha256_field("intent_digest", value)
}

fn validate_commit_digest(value: &str) -> Result<(), ReplayWalError> {
    validate_sha256_field("commit_digest", value)
}

fn validate_sha256_field(field: &'static str, value: &str) -> Result<(), ReplayWalError> {
    if !is_sha256_token(value) {
        return Err(ReplayWalError::InvalidInput {
            field,
            reason: "must be a lowercase sha256:<64-hex> token",
        });
    }
    Ok(())
}

fn is_sha256_token(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn update_length_prefixed(hasher: &mut Sha256, value: &[u8]) {
    let len = u64::try_from(value.len()).unwrap_or(u64::MAX);
    hasher.update(len.to_le_bytes());
    hasher.update(value);
}

fn format_sha256(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(hex, "{byte:02x}");
    }
    format!("sha256:{hex}")
}

fn next_sequence(last_seq: u64) -> Result<u64, ReplayWalError> {
    last_seq
        .checked_add(1)
        .ok_or(ReplayWalError::SequenceOverflow { last_seq })
}

fn trusted_state_root(path: &Path) -> Result<PathBuf, ReplayWalError> {
    let metadata = fs::metadata(path).map_err(|source| ReplayWalError::StateRootUnavailable {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    if !metadata.is_dir() {
        return Err(ReplayWalError::StateRootUnavailable {
            path: path.to_path_buf(),
            source: "path is not a directory".to_owned(),
        });
    }
    fs::canonicalize(path).map_err(|source| ReplayWalError::StateRootUnavailable {
        path: path.to_path_buf(),
        source: source.to_string(),
    })
}

fn ensure_replay_initialized_under_lock(
    state_root: &Path,
    create_if_absent: bool,
) -> Result<bool, ReplayWalError> {
    let wal_path = replay_wal_path(state_root);
    let manifest_path = replay_wal_manifest_path(state_root);
    let wal_exists = path_exists(&wal_path)?;
    let manifest_exists = path_exists(&manifest_path)?;
    match (manifest_exists, wal_exists) {
        (false, false) if create_if_absent => {
            initialize_replay_files(state_root, &wal_path, &manifest_path)?;
            Ok(true)
        }
        (false, false) => Err(ReplayWalError::NotInitialized {
            state_root: state_root.to_path_buf(),
        }),
        (true, true) => {
            validate_replay_manifest(&manifest_path)?;
            ensure_regular_file(state_root, &wal_path)?;
            Ok(false)
        }
        _ => Err(ReplayWalError::InitializationMismatch {
            manifest_path,
            manifest_exists,
            wal_path,
            wal_exists,
        }),
    }
}

fn path_exists(path: &Path) -> Result<bool, ReplayWalError> {
    path.try_exists()
        .map_err(|source| ReplayWalError::StateRootUnavailable {
            path: path.to_path_buf(),
            source: source.to_string(),
        })
}

fn ensure_regular_file(state_root: &Path, path: &Path) -> Result<(), ReplayWalError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ReplayWalError::ReadWal {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    if !metadata.file_type().is_file() {
        return Err(ReplayWalError::ReadWal {
            path: path.to_path_buf(),
            source: "authority file must be a regular non-symlink file".to_owned(),
        });
    }
    let canonical = fs::canonicalize(path).map_err(|source| ReplayWalError::ReadWal {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    if !canonical.starts_with(state_root) {
        return Err(ReplayWalError::ReadWal {
            path: path.to_path_buf(),
            source: "authority file resolves outside the trusted state root".to_owned(),
        });
    }
    Ok(())
}

fn initialize_replay_files(
    state_root: &Path,
    wal_path: &Path,
    manifest_path: &Path,
) -> Result<(), ReplayWalError> {
    create_wal_parent(state_root, wal_path)?;
    let wal_parent = wal_path.parent().ok_or_else(|| ReplayWalError::CreateDir {
        path: wal_path.to_path_buf(),
        source: "WAL path has no parent".to_owned(),
    })?;
    sync_directory(state_root).map_err(|source| ReplayWalError::SyncWal {
        path: state_root.to_path_buf(),
        source: source.to_string(),
    })?;

    let wal = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(wal_path)
        .map_err(|source| ReplayWalError::OpenWal {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        })?;
    wal.sync_all().map_err(|source| ReplayWalError::SyncWal {
        path: wal_path.to_path_buf(),
        source: source.to_string(),
    })?;
    sync_directory(wal_parent).map_err(|source| ReplayWalError::SyncWal {
        path: wal_parent.to_path_buf(),
        source: source.to_string(),
    })?;
    sync_directory(state_root).map_err(|source| ReplayWalError::SyncWal {
        path: state_root.to_path_buf(),
        source: source.to_string(),
    })?;

    let manifest = ReplayWalManifest {
        schema_version: REPLAY_WAL_SCHEMA_VERSION.to_owned(),
        format_magic: String::from_utf8_lossy(&MAGIC).into_owned(),
        wal_relative_path: REPLAY_WAL_RELATIVE_PATH.to_owned(),
    };
    let bytes = serde_json::to_vec(&manifest).map_err(|source| ReplayWalError::Serialize {
        source: source.to_string(),
    })?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(manifest_path)
        .map_err(|source| ReplayWalError::OpenWal {
            path: manifest_path.to_path_buf(),
            source: source.to_string(),
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| ReplayWalError::SyncWal {
            path: manifest_path.to_path_buf(),
            source: source.to_string(),
        })?;
    sync_directory(state_root).map_err(|source| ReplayWalError::SyncWal {
        path: state_root.to_path_buf(),
        source: source.to_string(),
    })
}

fn validate_replay_manifest(path: &Path) -> Result<(), ReplayWalError> {
    ensure_regular_manifest(path)?;
    let bytes = read_bounded_file(path, MANIFEST_MAX_BYTES).map_err(|source| {
        ReplayWalError::InvalidManifest {
            path: path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    let manifest = serde_json::from_slice::<ReplayWalManifest>(&bytes).map_err(|source| {
        ReplayWalError::InvalidManifest {
            path: path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    let expected = ReplayWalManifest {
        schema_version: REPLAY_WAL_SCHEMA_VERSION.to_owned(),
        format_magic: String::from_utf8_lossy(&MAGIC).into_owned(),
        wal_relative_path: REPLAY_WAL_RELATIVE_PATH.to_owned(),
    };
    if manifest != expected {
        return Err(ReplayWalError::InvalidManifest {
            path: path.to_path_buf(),
            source: "manifest does not describe the supported replay WAL".to_owned(),
        });
    }
    Ok(())
}

fn ensure_regular_manifest(path: &Path) -> Result<(), ReplayWalError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| ReplayWalError::InvalidManifest {
            path: path.to_path_buf(),
            source: source.to_string(),
        })?;
    if !metadata.file_type().is_file() {
        return Err(ReplayWalError::InvalidManifest {
            path: path.to_path_buf(),
            source: "manifest must be a regular non-symlink file".to_owned(),
        });
    }
    Ok(())
}

fn validate_effect_lock_scope(
    state_root: &Path,
    effect_lock: &crate::EffectStoreLock,
    expected_relative_path: &str,
) -> Result<(), ReplayWalError> {
    let relative = Path::new(expected_relative_path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ReplayWalError::InvalidInput {
            field: "expected_effect_lock_relative_path",
            reason: "must be a normalized non-empty relative path",
        });
    }
    let expected_lexical = state_root.join(relative);
    if expected_lexical == replay_wal_lock_path(state_root) {
        return Err(ReplayWalError::EffectLockOrderViolation {
            path: expected_lexical,
        });
    }
    let actual_lexical = effect_lock.path().to_path_buf();
    let actual =
        fs::canonicalize(&actual_lexical).map_err(|_| ReplayWalError::EffectLockScopeMismatch {
            expected: expected_lexical.clone(),
            actual: actual_lexical,
        })?;
    let expected = fs::canonicalize(&expected_lexical).map_err(|_| {
        ReplayWalError::EffectLockScopeMismatch {
            expected: expected_lexical,
            actual: actual.clone(),
        }
    })?;
    let replay_lock = replay_wal_lock_path(state_root);
    if fs::canonicalize(&replay_lock).is_ok_and(|path| path == expected) {
        return Err(ReplayWalError::EffectLockOrderViolation { path: expected });
    }
    if actual != expected || !actual.starts_with(state_root) {
        return Err(ReplayWalError::EffectLockScopeMismatch { expected, actual });
    }
    Ok(())
}

fn create_wal_parent(state_root: &Path, wal_path: &Path) -> Result<(), ReplayWalError> {
    let parent = wal_path.parent().ok_or_else(|| ReplayWalError::CreateDir {
        path: wal_path.to_path_buf(),
        source: "WAL path has no parent".to_owned(),
    })?;
    fs::create_dir_all(parent).map_err(|source| ReplayWalError::CreateDir {
        path: parent.to_path_buf(),
        source: source.to_string(),
    })?;
    ensure_directory_within_root(state_root, parent)
}

fn ensure_directory_within_root(state_root: &Path, directory: &Path) -> Result<(), ReplayWalError> {
    let canonical =
        fs::canonicalize(directory).map_err(|source| ReplayWalError::StateRootUnavailable {
            path: directory.to_path_buf(),
            source: source.to_string(),
        })?;
    if !canonical.starts_with(state_root) {
        return Err(ReplayWalError::StateRootUnavailable {
            path: directory.to_path_buf(),
            source: "directory resolves outside the trusted state root".to_owned(),
        });
    }
    Ok(())
}

fn acquire_replay_lock(state_root: &Path) -> Result<ReplayWalLock, ReplayWalError> {
    let path = replay_wal_lock_path(state_root);
    let parent = path.parent().ok_or_else(|| ReplayWalError::CreateDir {
        path: path.clone(),
        source: "lock path has no parent".to_owned(),
    })?;
    fs::create_dir_all(parent).map_err(|source| ReplayWalError::CreateDir {
        path: parent.to_path_buf(),
        source: source.to_string(),
    })?;
    ensure_directory_within_root(state_root, parent)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|source| ReplayWalError::OpenLock {
            path: path.clone(),
            source: source.to_string(),
        })?;
    FileExt::lock(&file).map_err(|source| ReplayWalError::Lock {
        path,
        source: source.to_string(),
    })?;
    Ok(ReplayWalLock { file })
}

struct ReplayWalLock {
    file: File,
}

impl Drop for ReplayWalLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn recover_replay_wal_under_lock(
    wal_path: &Path,
    repair: bool,
) -> Result<ReplayWalRecovery, ReplayWalError> {
    let bytes = read_bounded_file(wal_path, REPLAY_WAL_MAX_BYTES).map_err(|source| {
        if source.kind() == io::ErrorKind::FileTooLarge {
            let observed = fs::metadata(wal_path)
                .map_or(REPLAY_WAL_MAX_BYTES.saturating_add(1), |metadata| {
                    metadata.len()
                });
            ReplayWalError::CapacityExceeded {
                kind: ReplayWalCapacityKind::Bytes,
                limit: REPLAY_WAL_MAX_BYTES,
                observed,
            }
        } else {
            ReplayWalError::ReadWal {
                path: wal_path.to_path_buf(),
                source: source.to_string(),
            }
        }
    })?;
    let original_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let mut recovery = decode_prefix(wal_path, &bytes);
    if recovery.stop_reason == ReplayWalStopReason::RecordCapacityExceeded {
        return Err(ReplayWalError::CapacityExceeded {
            kind: ReplayWalCapacityKind::Records,
            limit: u64::try_from(REPLAY_WAL_MAX_RECORDS).unwrap_or(u64::MAX),
            observed: u64::try_from(REPLAY_WAL_MAX_RECORDS)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        });
    }
    if repair && recovery.last_good_offset < original_len && is_safe_torn_tail(recovery.stop_reason)
    {
        let file = OpenOptions::new()
            .write(true)
            .open(wal_path)
            .map_err(|source| ReplayWalError::RepairWal {
                path: wal_path.to_path_buf(),
                source: source.to_string(),
            })?;
        file.set_len(recovery.last_good_offset)
            .and_then(|()| file.sync_all())
            .map_err(|source| ReplayWalError::RepairWal {
                path: wal_path.to_path_buf(),
                source: source.to_string(),
            })?;
        recovery.repaired = true;
    }
    Ok(recovery)
}

fn read_bounded_file(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let metadata_len = file.metadata()?.len();
    if metadata_len > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            format!("file length {metadata_len} exceeds limit {max_bytes}"),
        ));
    }
    let capacity = usize::try_from(metadata_len).unwrap_or(usize::MAX);
    let mut bytes = Vec::with_capacity(capacity);
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            format!("file grew beyond limit {max_bytes} during read"),
        ));
    }
    Ok(bytes)
}

fn ensure_appendable(recovery: &ReplayWalRecovery) -> Result<(), ReplayWalError> {
    if recovery.is_clean() {
        return Ok(());
    }
    Err(ReplayWalError::RecoveryStopped {
        stop_reason: recovery.stop_reason,
        last_good_offset: recovery.last_good_offset,
        original_len: recovery.original_len,
    })
}

fn ensure_append_capacity(
    recovery: &ReplayWalRecovery,
    record_bytes: usize,
) -> Result<(), ReplayWalError> {
    let next_records = recovery.valid_record_count.saturating_add(1);
    if next_records > REPLAY_WAL_MAX_RECORDS {
        return Err(ReplayWalError::CapacityExceeded {
            kind: ReplayWalCapacityKind::Records,
            limit: u64::try_from(REPLAY_WAL_MAX_RECORDS).unwrap_or(u64::MAX),
            observed: u64::try_from(next_records).unwrap_or(u64::MAX),
        });
    }
    let next_bytes = recovery
        .last_good_offset
        .saturating_add(u64::try_from(record_bytes).unwrap_or(u64::MAX));
    if next_bytes > REPLAY_WAL_MAX_BYTES {
        return Err(ReplayWalError::CapacityExceeded {
            kind: ReplayWalCapacityKind::Bytes,
            limit: REPLAY_WAL_MAX_BYTES,
            observed: next_bytes,
        });
    }
    Ok(())
}

const fn is_safe_torn_tail(reason: ReplayWalStopReason) -> bool {
    matches!(
        reason,
        ReplayWalStopReason::TruncatedHeader | ReplayWalStopReason::TruncatedPayload
    )
}

fn encode_payload_record(seq: u64, payload: &ReplayWalPayload) -> Result<Vec<u8>, ReplayWalError> {
    let payload_bytes =
        serde_json::to_vec(payload).map_err(|source| ReplayWalError::Serialize {
            source: source.to_string(),
        })?;
    if payload_bytes.len() > MAX_PAYLOAD_LEN as usize {
        return Err(ReplayWalError::PayloadTooLarge {
            byte_len: payload_bytes.len(),
            max_byte_len: MAX_PAYLOAD_LEN,
        });
    }
    encode_record(seq, payload.operation.record_type(), &payload_bytes)
}

fn encode_record(seq: u64, record_type: u8, payload: &[u8]) -> Result<Vec<u8>, ReplayWalError> {
    let payload_len =
        u32::try_from(payload.len()).map_err(|_| ReplayWalError::PayloadTooLarge {
            byte_len: payload.len(),
            max_byte_len: MAX_PAYLOAD_LEN,
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

    let payload_crc = payload_crc32c(&header[..HEADER_CRC_OFFSET], payload);
    let mut record = Vec::with_capacity(HEADER_LEN + payload.len() + TRAILER_LEN);
    record.extend_from_slice(&header);
    record.extend_from_slice(payload);
    record.extend_from_slice(&payload_crc.to_le_bytes());
    Ok(record)
}

fn append_and_sync(wal_path: &Path, bytes: &[u8]) -> Result<(), ReplayWalError> {
    let mut file = OpenOptions::new()
        .append(true)
        .open(wal_path)
        .map_err(|source| ReplayWalError::OpenWal {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        })?;
    file.write_all(bytes)
        .map_err(|source| ReplayWalError::WriteWal {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        })?;
    file.sync_all().map_err(|source| ReplayWalError::SyncWal {
        path: wal_path.to_path_buf(),
        source: source.to_string(),
    })
}

#[cfg(not(windows))]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(windows)]
fn sync_directory(path: &Path) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt as _;

    // Windows requires FILE_FLAG_BACKUP_SEMANTICS to open a directory handle.
    // File::sync_all then maps to FlushFileBuffers on that durable handle.
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?
        .sync_all()
}

fn decode_prefix(wal_path: &Path, bytes: &[u8]) -> ReplayWalRecovery {
    decode_prefix_with_record_limit(wal_path, bytes, REPLAY_WAL_MAX_RECORDS)
}

fn decode_prefix_with_record_limit(
    wal_path: &Path,
    bytes: &[u8],
    max_records: usize,
) -> ReplayWalRecovery {
    let original_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let mut records = Vec::new();
    let mut reservations = BTreeMap::new();
    let mut offset = 0usize;
    let mut expected_seq = 1u64;
    let mut stop_reason = ReplayWalStopReason::CleanEof;

    while offset < bytes.len() {
        if records.len() >= max_records {
            stop_reason = ReplayWalStopReason::RecordCapacityExceeded;
            break;
        }
        let record = match decode_record_at(bytes, offset, expected_seq) {
            Ok(record) => record,
            Err(reason) => {
                stop_reason = reason;
                break;
            }
        };
        if !apply_record(&mut reservations, &record) {
            stop_reason = ReplayWalStopReason::InvalidTransition;
            break;
        }
        offset = offset.saturating_add(usize::try_from(record.record_len).unwrap_or(usize::MAX));
        records.push(record);
        if offset < bytes.len() {
            let Some(next_seq) = expected_seq.checked_add(1) else {
                stop_reason = ReplayWalStopReason::SequenceGap;
                break;
            };
            expected_seq = next_seq;
        }
    }

    let last_observed_seq = records.last().map_or(0, |record| record.seq);
    ReplayWalRecovery {
        wal_path: wal_path.to_path_buf(),
        valid_record_count: records.len(),
        records,
        reservations,
        last_observed_seq,
        last_good_offset: u64::try_from(offset).unwrap_or(u64::MAX),
        original_len,
        repaired: false,
        stop_reason,
    }
}

fn apply_record(
    reservations: &mut BTreeMap<String, ReplayReservation>,
    record: &ReplayWalRecord,
) -> bool {
    let payload = &record.payload;
    match payload.operation {
        ReplayWalOperation::Reserve => {
            if payload.revision != 1
                || payload.expected_revision.is_some()
                || reservations.contains_key(&payload.key_hash)
            {
                return false;
            }
            reservations.insert(
                payload.key_hash.clone(),
                ReplayReservation {
                    key_hash: payload.key_hash.clone(),
                    intent_digest: payload.intent_digest.clone(),
                    commit_digest: payload.commit_digest.clone(),
                    revision: 1,
                    state: ReplayReservationState::Reserved,
                    reserved_seq: record.seq,
                    consumed_seq: None,
                },
            );
            true
        }
        ReplayWalOperation::Consume => {
            let Some(current) = reservations.get_mut(&payload.key_hash) else {
                return false;
            };
            let transition_valid = current.state == ReplayReservationState::Reserved
                && current.intent_digest == payload.intent_digest
                && current.commit_digest == payload.commit_digest
                && payload.expected_revision == Some(current.revision)
                && current
                    .revision
                    .checked_add(1)
                    .is_some_and(|next| next == payload.revision);
            if !transition_valid {
                return false;
            }
            current.revision = payload.revision;
            current.state = ReplayReservationState::Consumed;
            current.consumed_seq = Some(record.seq);
            true
        }
    }
}

fn decode_record_at(
    bytes: &[u8],
    offset: usize,
    expected_seq: u64,
) -> Result<ReplayWalRecord, ReplayWalStopReason> {
    let frame = decode_frame(bytes, offset)?;
    if frame.seq != expected_seq {
        return Err(ReplayWalStopReason::SequenceGap);
    }
    let operation = ReplayWalOperation::from_record_type(frame.record_type)
        .ok_or(ReplayWalStopReason::UnsupportedRecordType)?;
    let payload = serde_json::from_slice::<ReplayWalPayload>(frame.payload)
        .map_err(|_| ReplayWalStopReason::PayloadDecodeFailed)?;
    if payload.schema_version != REPLAY_WAL_SCHEMA_VERSION
        || payload.operation != operation
        || !is_sha256_token(&payload.key_hash)
        || !is_sha256_token(&payload.intent_digest)
        || !is_sha256_token(&payload.commit_digest)
    {
        return Err(ReplayWalStopReason::PayloadDecodeFailed);
    }
    Ok(ReplayWalRecord {
        seq: frame.seq,
        operation,
        payload,
        offset: u64::try_from(offset).unwrap_or(u64::MAX),
        record_len: u64::try_from(frame.record_end.saturating_sub(offset)).unwrap_or(u64::MAX),
    })
}

struct DecodedFrame<'a> {
    record_type: u8,
    seq: u64,
    payload: &'a [u8],
    record_end: usize,
}

fn decode_frame(bytes: &[u8], offset: usize) -> Result<DecodedFrame<'_>, ReplayWalStopReason> {
    if bytes.len().saturating_sub(offset) < HEADER_LEN {
        return Err(ReplayWalStopReason::TruncatedHeader);
    }
    let header = &bytes[offset..offset + HEADER_LEN];
    if header[..4] != MAGIC || header[4] != VERSION {
        return Err(ReplayWalStopReason::InvalidHeader);
    }
    let flags = u16::from_le_bytes([header[6], header[7]]);
    if flags != FLAG_PAYLOAD_JSON {
        return Err(ReplayWalStopReason::InvalidHeader);
    }
    let payload_len =
        u32::from_le_bytes(header[16..20].try_into().expect("four-byte payload length"));
    if payload_len > MAX_PAYLOAD_LEN {
        return Err(ReplayWalStopReason::PayloadTooLarge);
    }
    let expected_header_crc = u32::from_le_bytes(
        header[HEADER_CRC_OFFSET..HEADER_LEN]
            .try_into()
            .expect("four-byte header checksum"),
    );
    if crc32c::crc32c(&header[..HEADER_CRC_OFFSET]) != expected_header_crc {
        return Err(ReplayWalStopReason::InvalidHeader);
    }
    let payload_len =
        usize::try_from(payload_len).map_err(|_| ReplayWalStopReason::PayloadTooLarge)?;
    let record_end = offset
        .checked_add(HEADER_LEN)
        .and_then(|value| value.checked_add(payload_len))
        .and_then(|value| value.checked_add(TRAILER_LEN))
        .ok_or(ReplayWalStopReason::PayloadTooLarge)?;
    if record_end > bytes.len() {
        return Err(ReplayWalStopReason::TruncatedPayload);
    }
    let payload_start = offset + HEADER_LEN;
    let payload_end = payload_start + payload_len;
    let payload = &bytes[payload_start..payload_end];
    let expected_payload_crc = u32::from_le_bytes(
        bytes[payload_end..record_end]
            .try_into()
            .expect("four-byte payload checksum"),
    );
    if payload_crc32c(&header[..HEADER_CRC_OFFSET], payload) != expected_payload_crc {
        return Err(ReplayWalStopReason::PayloadChecksumMismatch);
    }
    Ok(DecodedFrame {
        record_type: header[5],
        seq: u64::from_le_bytes(header[8..16].try_into().expect("eight-byte sequence")),
        payload,
        record_end,
    })
}

fn payload_crc32c(header_prefix: &[u8], payload: &[u8]) -> u32 {
    let mut covered = Vec::with_capacity(header_prefix.len() + payload.len());
    covered.extend_from_slice(header_prefix);
    covered.extend_from_slice(payload);
    crc32c::crc32c(&covered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    #[test]
    fn decoder_stops_before_record_capacity_allocation() {
        let first = ReplayWalPayload {
            schema_version: REPLAY_WAL_SCHEMA_VERSION.to_owned(),
            operation: ReplayWalOperation::Reserve,
            key_hash: token('a'),
            intent_digest: token('b'),
            commit_digest: token('c'),
            revision: 1,
            expected_revision: None,
        };
        let second = ReplayWalPayload {
            schema_version: REPLAY_WAL_SCHEMA_VERSION.to_owned(),
            operation: ReplayWalOperation::Reserve,
            key_hash: token('d'),
            intent_digest: token('e'),
            commit_digest: token('f'),
            revision: 1,
            expected_revision: None,
        };
        let mut bytes = encode_payload_record(1, &first).expect("encode first frame");
        bytes.extend(encode_payload_record(2, &second).expect("encode second frame"));

        let recovery = decode_prefix_with_record_limit(Path::new("replay.fmr1"), &bytes, 1);

        assert_eq!(recovery.valid_record_count, 1);
        assert_eq!(
            recovery.stop_reason,
            ReplayWalStopReason::RecordCapacityExceeded
        );
        assert_eq!(recovery.reservations.len(), 1);
        assert!(recovery.last_good_offset < recovery.original_len);
    }

    #[test]
    fn append_preflight_enforces_both_hard_capacities() {
        let mut recovery = ReplayWalRecovery {
            wal_path: PathBuf::from("replay.fmr1"),
            records: Vec::new(),
            reservations: BTreeMap::new(),
            last_observed_seq: 0,
            valid_record_count: 0,
            last_good_offset: REPLAY_WAL_MAX_BYTES,
            original_len: REPLAY_WAL_MAX_BYTES,
            repaired: false,
            stop_reason: ReplayWalStopReason::CleanEof,
        };
        assert!(matches!(
            ensure_append_capacity(&recovery, 1),
            Err(ReplayWalError::CapacityExceeded {
                kind: ReplayWalCapacityKind::Bytes,
                ..
            })
        ));

        recovery.last_good_offset = 0;
        recovery.original_len = 0;
        recovery.valid_record_count = REPLAY_WAL_MAX_RECORDS;
        assert!(matches!(
            ensure_append_capacity(&recovery, 1),
            Err(ReplayWalError::CapacityExceeded {
                kind: ReplayWalCapacityKind::Records,
                ..
            })
        ));
    }
}
