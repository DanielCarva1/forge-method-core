//! Operator-protected replay-head anchors outside the Forge state root.
//!
//! A replay WAL can prove integrity of the bytes it contains, but a complete
//! older manifest/WAL pair is still internally valid. This module binds the
//! latest trusted WAL prefix to a separate monotonic document. The anchor
//! store is deliberately outside the state root and is a deployment trust
//! boundary: Forge detects state rollback relative to it, but cannot prevent
//! an actor that can roll back both stores from presenting a consistent past.

use crate::replay_wal::{
    recover_replay_wal, replay_wal_manifest_path, replay_wal_path, ReplayWalError,
};
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const REPLAY_ANCHOR_SCHEMA_VERSION: &str = "0.1";
const MAX_ANCHOR_BYTES: u64 = 16 * 1024;
const MAX_DEPLOYMENT_ID_BYTES: usize = 256;
const ADVANCE_RETRIES: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayAnchorDocument {
    pub schema_version: String,
    pub deployment_id: String,
    pub epoch: String,
    pub generation: u64,
    pub previous_anchor_digest: Option<String>,
    pub head: ReplayWalHead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayWalHead {
    pub manifest_digest: String,
    pub wal_prefix_digest: String,
    pub last_seq: u64,
    pub record_count: usize,
    pub byte_len: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayAnchorStatus {
    Current,
    AdvanceRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayAnchorVerification {
    pub status: ReplayAnchorStatus,
    pub anchor_path: PathBuf,
    pub anchor: ReplayAnchorDocument,
    pub current_head: ReplayWalHead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayAnchorAdvance {
    pub changed: bool,
    pub anchor_path: PathBuf,
    pub anchor: ReplayAnchorDocument,
}

/// Create the first trusted head in an operator-protected external store.
///
/// Existing replay history is allowed so an operator can adopt anchoring for
/// a live deployment. This establishes trust from the observed head forward;
/// it does not retroactively prove that older history was never rolled back.
///
/// # Errors
///
/// Fails if replay authority is invalid, the path is not an absolute regular
/// external file, an anchor already exists, randomness fails, or durable write
/// and synchronization fail.
pub fn provision_replay_anchor(
    state_root: impl AsRef<Path>,
    anchor_path: impl AsRef<Path>,
    deployment_id: &str,
) -> Result<ReplayAnchorAdvance, ReplayAnchorError> {
    if deployment_id.trim().is_empty() {
        return Err(ReplayAnchorError::Invalid(
            "deployment_id must not be blank".to_owned(),
        ));
    }
    let state_root = canonical_state_root(state_root.as_ref())?;
    let anchor_path = validated_external_anchor_path(&state_root, anchor_path.as_ref(), false)?;
    let head = capture_replay_head(&state_root)?;
    let _lock = acquire_anchor_lock(&anchor_path)?;
    if anchor_path
        .try_exists()
        .map_err(|error| io_error(&anchor_path, &error))?
    {
        return Err(ReplayAnchorError::AlreadyExists(anchor_path));
    }
    let mut epoch_bytes = [0_u8; 32];
    getrandom::fill(&mut epoch_bytes)
        .map_err(|error| ReplayAnchorError::Random(error.to_string()))?;
    let document = ReplayAnchorDocument {
        schema_version: REPLAY_ANCHOR_SCHEMA_VERSION.to_owned(),
        deployment_id: deployment_id.to_owned(),
        epoch: hex(&epoch_bytes),
        generation: 1,
        previous_anchor_digest: None,
        head,
    };
    epoch_bytes.fill(0);
    write_anchor_create_new(&anchor_path, &document)?;
    Ok(ReplayAnchorAdvance {
        changed: true,
        anchor_path,
        anchor: document,
    })
}

/// Compare current replay authority with the last externally trusted prefix.
///
/// # Errors
///
/// Returns [`ReplayAnchorError::RollbackDetected`] when the current WAL is
/// shorter/older than the anchor, and `Diverged` when the trusted prefix or
/// manifest differs. A longer prefix-valid WAL is reported as
/// [`ReplayAnchorStatus::AdvanceRequired`].
pub fn verify_replay_anchor(
    state_root: impl AsRef<Path>,
    anchor_path: impl AsRef<Path>,
) -> Result<ReplayAnchorVerification, ReplayAnchorError> {
    let state_root = canonical_state_root(state_root.as_ref())?;
    let anchor_path = validated_external_anchor_path(&state_root, anchor_path.as_ref(), true)?;
    let anchor = read_anchor(&anchor_path)?;
    let current = capture_replay_head_with_bytes(&state_root)?;
    let status = compare_anchor(&anchor, &current.head, &current.wal_bytes)?;
    Ok(ReplayAnchorVerification {
        status,
        anchor_path,
        anchor,
        current_head: current.head,
    })
}

/// Advance an anchor only across a prefix-valid monotonic WAL extension.
///
/// # Errors
///
/// Fails closed on rollback, divergence, invalid anchor state, generation
/// overflow, persistent concurrent drift, or durable replacement failure.
pub fn advance_replay_anchor(
    state_root: impl AsRef<Path>,
    anchor_path: impl AsRef<Path>,
) -> Result<ReplayAnchorAdvance, ReplayAnchorError> {
    advance_replay_anchor_inner(state_root.as_ref(), anchor_path.as_ref(), None)
}

/// Advance an anchor only when it belongs to the expected deployment.
///
/// The deployment check is performed while the external anchor lock is held,
/// before any replacement, so a mismatched anchor is never modified.
///
/// # Errors
///
/// Returns [`ReplayAnchorError::DeploymentMismatch`] without changing the
/// anchor when its operator-provisioned deployment id differs.
pub fn advance_replay_anchor_for_deployment(
    state_root: impl AsRef<Path>,
    anchor_path: impl AsRef<Path>,
    expected_deployment_id: &str,
) -> Result<ReplayAnchorAdvance, ReplayAnchorError> {
    if expected_deployment_id.trim().is_empty() {
        return Err(ReplayAnchorError::Invalid(
            "expected deployment id must not be blank".to_owned(),
        ));
    }
    advance_replay_anchor_inner(
        state_root.as_ref(),
        anchor_path.as_ref(),
        Some(expected_deployment_id),
    )
}

fn advance_replay_anchor_inner(
    state_root: &Path,
    anchor_path: &Path,
    expected_deployment_id: Option<&str>,
) -> Result<ReplayAnchorAdvance, ReplayAnchorError> {
    let state_root = canonical_state_root(state_root)?;
    let anchor_path = validated_external_anchor_path(&state_root, anchor_path, true)?;
    for attempt in 0..ADVANCE_RETRIES {
        let current = capture_replay_head_with_bytes(&state_root)?;
        let lock = acquire_anchor_lock(&anchor_path)?;
        let anchor = read_anchor(&anchor_path)?;
        if let Some(expected) = expected_deployment_id {
            if anchor.deployment_id != expected {
                return Err(ReplayAnchorError::DeploymentMismatch {
                    expected: expected.to_owned(),
                    actual: anchor.deployment_id,
                });
            }
        }
        if anchor.head.last_seq > current.head.last_seq {
            drop(lock);
            if attempt + 1 < ADVANCE_RETRIES {
                continue;
            }
            return Err(ReplayAnchorError::RollbackDetected {
                anchored_seq: anchor.head.last_seq,
                current_seq: current.head.last_seq,
            });
        }
        match compare_anchor(&anchor, &current.head, &current.wal_bytes)? {
            ReplayAnchorStatus::Current => {
                return Ok(ReplayAnchorAdvance {
                    changed: false,
                    anchor_path,
                    anchor,
                });
            }
            ReplayAnchorStatus::AdvanceRequired => {
                let generation = anchor
                    .generation
                    .checked_add(1)
                    .ok_or(ReplayAnchorError::GenerationOverflow)?;
                let previous_anchor_digest = Some(anchor_digest(&anchor)?);
                let updated = ReplayAnchorDocument {
                    schema_version: REPLAY_ANCHOR_SCHEMA_VERSION.to_owned(),
                    deployment_id: anchor.deployment_id,
                    epoch: anchor.epoch,
                    generation,
                    previous_anchor_digest,
                    head: current.head,
                };
                write_anchor_replace(&anchor_path, &updated)?;
                return Ok(ReplayAnchorAdvance {
                    changed: true,
                    anchor_path,
                    anchor: updated,
                });
            }
        }
    }
    Err(ReplayAnchorError::ConcurrentDrift)
}

#[derive(Debug)]
struct CapturedReplayHead {
    head: ReplayWalHead,
    wal_bytes: Vec<u8>,
}

fn capture_replay_head(state_root: &Path) -> Result<ReplayWalHead, ReplayAnchorError> {
    capture_replay_head_with_bytes(state_root).map(|captured| captured.head)
}

fn capture_replay_head_with_bytes(
    state_root: &Path,
) -> Result<CapturedReplayHead, ReplayAnchorError> {
    let recovery = recover_replay_wal(state_root, false).map_err(ReplayAnchorError::Replay)?;
    if !recovery.is_clean() {
        return Err(ReplayAnchorError::Invalid(format!(
            "replay WAL stopped at {:?}",
            recovery.stop_reason
        )));
    }
    let wal_path = replay_wal_path(state_root);
    let wal_bytes = fs::read(&wal_path).map_err(|error| io_error(&wal_path, &error))?;
    if u64::try_from(wal_bytes.len()).unwrap_or(u64::MAX) != recovery.last_good_offset {
        return Err(ReplayAnchorError::ConcurrentDrift);
    }
    let manifest_path = replay_wal_manifest_path(state_root);
    let manifest_bytes =
        fs::read(&manifest_path).map_err(|error| io_error(&manifest_path, &error))?;
    Ok(CapturedReplayHead {
        head: ReplayWalHead {
            manifest_digest: sha256(&manifest_bytes),
            wal_prefix_digest: sha256(&wal_bytes),
            last_seq: recovery.last_observed_seq,
            record_count: recovery.valid_record_count,
            byte_len: recovery.last_good_offset,
        },
        wal_bytes,
    })
}

fn compare_anchor(
    anchor: &ReplayAnchorDocument,
    current: &ReplayWalHead,
    current_wal: &[u8],
) -> Result<ReplayAnchorStatus, ReplayAnchorError> {
    validate_anchor_document(anchor)?;
    if anchor.head.manifest_digest != current.manifest_digest {
        return Err(ReplayAnchorError::Diverged(
            "replay manifest differs from the externally trusted deployment".to_owned(),
        ));
    }
    if current.last_seq < anchor.head.last_seq || current.byte_len < anchor.head.byte_len {
        return Err(ReplayAnchorError::RollbackDetected {
            anchored_seq: anchor.head.last_seq,
            current_seq: current.last_seq,
        });
    }
    let anchored_len = usize::try_from(anchor.head.byte_len).map_err(|_| {
        ReplayAnchorError::Invalid("anchored WAL byte length does not fit this platform".to_owned())
    })?;
    let prefix = current_wal
        .get(..anchored_len)
        .ok_or(ReplayAnchorError::RollbackDetected {
            anchored_seq: anchor.head.last_seq,
            current_seq: current.last_seq,
        })?;
    if sha256(prefix) != anchor.head.wal_prefix_digest {
        return Err(ReplayAnchorError::Diverged(
            "current WAL does not extend the externally trusted byte prefix".to_owned(),
        ));
    }
    if current.byte_len == anchor.head.byte_len {
        if current.last_seq != anchor.head.last_seq
            || current.record_count != anchor.head.record_count
            || current.wal_prefix_digest != anchor.head.wal_prefix_digest
        {
            return Err(ReplayAnchorError::Diverged(
                "equal-length replay authority has different sequence metadata".to_owned(),
            ));
        }
        return Ok(ReplayAnchorStatus::Current);
    }
    if current.last_seq <= anchor.head.last_seq || current.record_count <= anchor.head.record_count
    {
        return Err(ReplayAnchorError::Diverged(
            "longer WAL did not monotonically advance sequence metadata".to_owned(),
        ));
    }
    Ok(ReplayAnchorStatus::AdvanceRequired)
}

fn validate_anchor_document(anchor: &ReplayAnchorDocument) -> Result<(), ReplayAnchorError> {
    if anchor.schema_version != REPLAY_ANCHOR_SCHEMA_VERSION
        || anchor.deployment_id.trim().is_empty()
        || anchor.deployment_id.len() > MAX_DEPLOYMENT_ID_BYTES
        || anchor.epoch.len() != 64
        || !anchor
            .epoch
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || anchor.generation == 0
        || (anchor.generation == 1) != anchor.previous_anchor_digest.is_none()
        || !is_sha256(&anchor.head.manifest_digest)
        || !is_sha256(&anchor.head.wal_prefix_digest)
        || anchor.head.record_count != usize::try_from(anchor.head.last_seq).unwrap_or(usize::MAX)
        || anchor
            .previous_anchor_digest
            .as_ref()
            .is_some_and(|digest| !is_sha256(digest))
    {
        return Err(ReplayAnchorError::Invalid(
            "anchor document violates schema or monotonic head invariants".to_owned(),
        ));
    }
    Ok(())
}

fn read_anchor(path: &Path) -> Result<ReplayAnchorDocument, ReplayAnchorError> {
    ensure_regular_anchor(path)?;
    let metadata = fs::metadata(path).map_err(|error| io_error(path, &error))?;
    if metadata.len() > MAX_ANCHOR_BYTES {
        return Err(ReplayAnchorError::Invalid(format!(
            "anchor is {} bytes; limit is {MAX_ANCHOR_BYTES}",
            metadata.len()
        )));
    }
    let bytes = fs::read(path).map_err(|error| io_error(path, &error))?;
    let anchor: ReplayAnchorDocument = serde_json::from_slice(&bytes)
        .map_err(|error| ReplayAnchorError::Invalid(error.to_string()))?;
    validate_anchor_document(&anchor)?;
    Ok(anchor)
}

fn anchor_digest(anchor: &ReplayAnchorDocument) -> Result<String, ReplayAnchorError> {
    serde_json_canonicalizer::to_vec(anchor)
        .map(|bytes| sha256(&bytes))
        .map_err(|error| ReplayAnchorError::Invalid(error.to_string()))
}

fn validated_external_anchor_path(
    state_root: &Path,
    requested: &Path,
    must_exist: bool,
) -> Result<PathBuf, ReplayAnchorError> {
    if !requested.is_absolute() || requested.file_name().is_none() {
        return Err(ReplayAnchorError::Invalid(
            "anchor path must be an absolute file path".to_owned(),
        ));
    }
    let parent = requested.parent().ok_or_else(|| {
        ReplayAnchorError::Invalid("anchor path has no parent directory".to_owned())
    })?;
    let parent = fs::canonicalize(parent).map_err(|error| io_error(parent, &error))?;
    if parent.starts_with(state_root) {
        return Err(ReplayAnchorError::Invalid(
            "anchor must remain outside the Forge state root".to_owned(),
        ));
    }
    let path = parent.join(requested.file_name().expect("checked file name"));
    if must_exist {
        ensure_regular_anchor(&path)?;
        let canonical = fs::canonicalize(&path).map_err(|error| io_error(&path, &error))?;
        if canonical.starts_with(state_root) {
            return Err(ReplayAnchorError::Invalid(
                "anchor resolves inside the Forge state root".to_owned(),
            ));
        }
    }
    Ok(path)
}

fn ensure_regular_anchor(path: &Path) -> Result<(), ReplayAnchorError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, &error))?;
    if !metadata.file_type().is_file() {
        return Err(ReplayAnchorError::Invalid(
            "anchor must be a regular non-symlink file".to_owned(),
        ));
    }
    Ok(())
}

fn acquire_anchor_lock(anchor_path: &Path) -> Result<File, ReplayAnchorError> {
    let lock_path = anchor_lock_path(anchor_path)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| io_error(&lock_path, &error))?;
    FileExt::lock(&lock).map_err(|error| io_error(&lock_path, &error))?;
    Ok(lock)
}

fn anchor_lock_path(anchor_path: &Path) -> Result<PathBuf, ReplayAnchorError> {
    let file_name = anchor_path
        .file_name()
        .ok_or_else(|| ReplayAnchorError::Invalid("anchor file name missing".to_owned()))?
        .to_string_lossy();
    Ok(anchor_path.with_file_name(format!("{file_name}.lock")))
}

fn write_anchor_create_new(
    path: &Path,
    document: &ReplayAnchorDocument,
) -> Result<(), ReplayAnchorError> {
    validate_anchor_document(document)?;
    let bytes = serialize_anchor(document)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_error(path, &error))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| io_error(path, &error))?;
    sync_parent(path)
}

fn write_anchor_replace(
    path: &Path,
    document: &ReplayAnchorDocument,
) -> Result<(), ReplayAnchorError> {
    validate_anchor_document(document)?;
    let bytes = serialize_anchor(document)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let temp = path.with_file_name(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .expect("validated anchor file name")
            .to_string_lossy(),
        std::process::id(),
        nonce
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| io_error(&temp, &error))?;
    let result = file
        .write_all(&bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| {
            drop(file);
            fs::rename(&temp, path)
        });
    if let Err(error) = result {
        let _ = fs::remove_file(&temp);
        return Err(io_error(path, &error));
    }
    sync_parent(path)
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<(), ReplayAnchorError> {
    let parent = path.parent().expect("validated parent");
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error(parent, &error))
}

fn serialize_anchor(document: &ReplayAnchorDocument) -> Result<Vec<u8>, ReplayAnchorError> {
    let bytes = serde_json::to_vec_pretty(document)
        .map_err(|error| ReplayAnchorError::Invalid(error.to_string()))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_ANCHOR_BYTES {
        return Err(ReplayAnchorError::Invalid(format!(
            "serialized anchor exceeds {MAX_ANCHOR_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)] // same cross-platform durability signature as Unix
fn sync_parent(path: &Path) -> Result<(), ReplayAnchorError> {
    let _ = path;
    Ok(())
}

fn canonical_state_root(path: &Path) -> Result<PathBuf, ReplayAnchorError> {
    let canonical = fs::canonicalize(path).map_err(|error| io_error(path, &error))?;
    if !canonical.is_dir() {
        return Err(ReplayAnchorError::Invalid(
            "state root must be a directory".to_owned(),
        ));
    }
    Ok(canonical)
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{}", hex(Sha256::digest(bytes).as_slice()))
}

fn is_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
        output
    })
}

fn io_error(path: &Path, error: &std::io::Error) -> ReplayAnchorError {
    ReplayAnchorError::Io {
        path: path.to_path_buf(),
        source: error.to_string(),
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ReplayAnchorError {
    Invalid(String),
    Io { path: PathBuf, source: String },
    Random(String),
    Replay(ReplayWalError),
    AlreadyExists(PathBuf),
    DeploymentMismatch { expected: String, actual: String },
    RollbackDetected { anchored_seq: u64, current_seq: u64 },
    Diverged(String),
    GenerationOverflow,
    ConcurrentDrift,
}

impl fmt::Display for ReplayAnchorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(reason) => write!(formatter, "invalid replay anchor: {reason}"),
            Self::Io { path, source } => write!(formatter, "replay anchor I/O {}: {source}", path.display()),
            Self::Random(source) => write!(formatter, "replay anchor randomness failed: {source}"),
            Self::Replay(source) => write!(formatter, "replay authority failed: {source}"),
            Self::AlreadyExists(path) => write!(formatter, "replay anchor {} already exists", path.display()),
            Self::DeploymentMismatch { expected, actual } => write!(
                formatter,
                "replay anchor deployment '{actual}' does not match expected deployment '{expected}'"
            ),
            Self::RollbackDetected { anchored_seq, current_seq } => write!(
                formatter,
                "replay rollback detected: externally anchored sequence {anchored_seq}, current sequence {current_seq}"
            ),
            Self::Diverged(reason) => write!(formatter, "replay authority diverged: {reason}"),
            Self::GenerationOverflow => formatter.write_str("replay anchor generation overflow"),
            Self::ConcurrentDrift => formatter.write_str("replay authority changed during anchor capture"),
        }
    }
}

impl std::error::Error for ReplayAnchorError {}
