//! Durable, explicitly non-authoritative capture for local Domain Pack learning.
//!
//! Capturing an observation preserves its exact submitted bytes. It cannot
//! review, promote, install, activate, or otherwise turn the observation into
//! authority. Those operations deliberately have no API in this crate.

use forge_core_contracts::{
    DomainPackLocalLearningAuthority, DomainPackLocalLearningCandidateDocument,
};
use forge_core_store::crash_replace::{
    recover_file_crash_safe_under_lock, replace_file_crash_safe_under_lock, CrashReplaceError,
};
use forge_core_store::{acquire_effect_store_lock, EffectStoreLockError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub const LEARNING_ROOT_RELATIVE_PATH: &str = "domain-pack-learning";
pub const LEARNING_LOCK_RELATIVE_PATH: &str = "domain-pack-learning/capture.lock";
pub const LEARNING_INDEX_RELATIVE_PATH: &str = "domain-pack-learning/index.json";
pub const MAX_CANDIDATE_BYTES: u64 = 1024 * 1024;
pub const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_CAPTURE_RECORDS: usize = 10_000;
const INDEX_SCHEMA_VERSION: &str = "0.1";

/// Result authority is intentionally closed to one non-authoritative value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningCaptureAuthority {
    NonAuthoritativeObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningCaptureDisposition {
    Captured,
    AlreadyPresent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningCaptureReceipt {
    pub authority: LearningCaptureAuthority,
    pub disposition: LearningCaptureDisposition,
    pub candidate_id: String,
    pub candidate_digest: String,
    pub raw_sha256: String,
    pub object_relative_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningObjectIntegrity {
    Verified,
    Missing,
    DigestMismatch,
    CandidateMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningCaptureStatus {
    pub authority: LearningCaptureAuthority,
    pub candidate_id: String,
    pub candidate_digest: String,
    pub raw_sha256: String,
    pub object_relative_path: String,
    pub integrity: LearningObjectIntegrity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningStoreProjection {
    pub authority: LearningCaptureAuthority,
    pub records: Vec<LearningCaptureStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LearningStoreError {
    CandidateSize {
        found: u64,
        maximum: u64,
    },
    CandidateEncoding(String),
    CandidateContract(Vec<String>),
    CandidateDigestMismatch {
        authored: String,
        computed: String,
    },
    CandidateIdConflict {
        candidate_id: String,
        existing_digest: String,
        submitted_digest: String,
    },
    ResourceLimit {
        resource: &'static str,
        maximum: usize,
    },
    InvalidStateRoot {
        path: PathBuf,
        reason: String,
    },
    InvalidStorePath {
        path: PathBuf,
        reason: String,
    },
    CorruptIndex(String),
    Lock(String),
    Persistence(String),
    Io {
        path: PathBuf,
        source: String,
    },
}

impl fmt::Display for LearningStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CandidateSize { found, maximum } => {
                write!(formatter, "candidate exceeds byte limit: {found} > {maximum}")
            }
            Self::CandidateEncoding(reason) => write!(formatter, "invalid candidate YAML: {reason}"),
            Self::CandidateContract(issues) => {
                write!(formatter, "candidate violates contract: {}", issues.join("; "))
            }
            Self::CandidateDigestMismatch { authored, computed } => write!(
                formatter,
                "candidate self-digest mismatch: authored {authored}, computed {computed}"
            ),
            Self::CandidateIdConflict {
                candidate_id,
                existing_digest,
                submitted_digest,
            } => write!(
                formatter,
                "candidate id {candidate_id} already names digest {existing_digest}, not {submitted_digest}"
            ),
            Self::ResourceLimit { resource, maximum } => {
                write!(formatter, "{resource} exceeds limit {maximum}")
            }
            Self::InvalidStateRoot { path, reason } => {
                write!(formatter, "invalid state root {}: {reason}", path.display())
            }
            Self::InvalidStorePath { path, reason } => {
                write!(formatter, "invalid learning-store path {}: {reason}", path.display())
            }
            Self::CorruptIndex(reason) => write!(formatter, "corrupt learning index: {reason}"),
            Self::Lock(reason) => write!(formatter, "learning-store lock failed: {reason}"),
            Self::Persistence(reason) => write!(formatter, "learning-store persistence failed: {reason}"),
            Self::Io { path, source } => write!(formatter, "I/O {} failed: {source}", path.display()),
        }
    }
}

impl std::error::Error for LearningStoreError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LearningIndex {
    schema_version: String,
    records: Vec<LearningIndexRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LearningIndexRecord {
    authority: LearningCaptureAuthority,
    candidate_id: String,
    candidate_digest: String,
    raw_sha256: String,
    object_relative_path: String,
}

impl Default for LearningIndex {
    fn default() -> Self {
        Self {
            schema_version: INDEX_SCHEMA_VERSION.to_owned(),
            records: Vec::new(),
        }
    }
}

/// Compute the authored candidate self-digest.
///
/// Algorithm: serialize the typed document to JSON, remove exactly
/// `domain_pack_local_learning_candidate.candidate_digest`, canonicalize the
/// remaining JSON with RFC 8785/JCS, then return lowercase SHA-256 hex without
/// a prefix. Exact YAML presentation is intentionally excluded; exact raw
/// bytes are bound independently by `raw_sha256`.
///
/// # Errors
///
/// Returns an encoding error if the typed document cannot be represented as
/// canonical JSON.
pub fn candidate_self_digest(
    document: &DomainPackLocalLearningCandidateDocument,
) -> Result<String, LearningStoreError> {
    let mut value = serde_json::to_value(document)
        .map_err(|error| LearningStoreError::CandidateEncoding(error.to_string()))?;
    let candidate = value
        .get_mut("domain_pack_local_learning_candidate")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| {
            LearningStoreError::CandidateEncoding("candidate object is absent".to_owned())
        })?;
    candidate.remove("candidate_digest").ok_or_else(|| {
        LearningStoreError::CandidateEncoding("candidate_digest field is absent".to_owned())
    })?;
    let canonical = serde_json_canonicalizer::to_vec(&value)
        .map_err(|error| LearningStoreError::CandidateEncoding(error.to_string()))?;
    Ok(sha256_hex(&canonical))
}

/// Capture exact candidate bytes under a retained OS lock.
///
/// The operation is idempotent for an identical candidate digest and fails
/// explicitly when one candidate id is reused for different content.
///
/// # Errors
///
/// Fails closed for malformed or oversized input, invalid self-digests,
/// equivocation, corrupt state, links/special files, lock failure, or any
/// durability error.
pub fn capture_local_learning(
    state_root: impl AsRef<Path>,
    raw_candidate_yaml: &[u8],
) -> Result<LearningCaptureReceipt, LearningStoreError> {
    let state_root = prepare_state_root(state_root.as_ref())?;
    validate_candidate_size(raw_candidate_yaml)?;
    let document = parse_candidate(raw_candidate_yaml)?;
    validate_candidate(&document)?;
    let computed_digest = candidate_self_digest(&document)?;
    let candidate = &document.domain_pack_local_learning_candidate;
    if candidate.candidate_digest != computed_digest {
        return Err(LearningStoreError::CandidateDigestMismatch {
            authored: candidate.candidate_digest.clone(),
            computed: computed_digest,
        });
    }

    ensure_store_layout_safe(&state_root)?;
    let lock = acquire_effect_store_lock(&state_root, LEARNING_LOCK_RELATIVE_PATH)
        .map_err(map_lock_error)?;
    ensure_store_layout_safe(&state_root)?;
    recover_target(
        &state_root,
        &lock,
        LEARNING_INDEX_RELATIVE_PATH,
        MAX_INDEX_BYTES,
    )?;

    let mut index = load_index(&state_root)?;
    validate_index(&index)?;
    if let Some(existing) = index
        .records
        .iter()
        .find(|record| record.candidate_id == candidate.candidate_id.0)
    {
        if existing.candidate_digest != candidate.candidate_digest {
            return Err(LearningStoreError::CandidateIdConflict {
                candidate_id: candidate.candidate_id.0.clone(),
                existing_digest: existing.candidate_digest.clone(),
                submitted_digest: candidate.candidate_digest.clone(),
            });
        }
        verify_existing_object(&state_root, existing, raw_candidate_yaml)?;
        return Ok(receipt(
            existing,
            LearningCaptureDisposition::AlreadyPresent,
        ));
    }
    if index.records.len() >= MAX_CAPTURE_RECORDS {
        return Err(LearningStoreError::ResourceLimit {
            resource: "capture records",
            maximum: MAX_CAPTURE_RECORDS,
        });
    }

    let raw_sha256 = sha256_hex(raw_candidate_yaml);
    let object_relative_path = format!("{LEARNING_ROOT_RELATIVE_PATH}/objects/{raw_sha256}");
    recover_target(
        &state_root,
        &lock,
        &object_relative_path,
        MAX_CANDIDATE_BYTES,
    )?;
    ensure_relative_regular_or_absent(&state_root, &object_relative_path)?;
    persist_immutable_object(
        &state_root,
        &lock,
        &object_relative_path,
        raw_candidate_yaml,
    )?;

    let record = LearningIndexRecord {
        authority: LearningCaptureAuthority::NonAuthoritativeObservation,
        candidate_id: candidate.candidate_id.0.clone(),
        candidate_digest: candidate.candidate_digest.clone(),
        raw_sha256,
        object_relative_path,
    };
    index.records.push(record.clone());
    persist_index(&state_root, &lock, &index)?;
    Ok(receipt(&record, LearningCaptureDisposition::Captured))
}

/// List captured observations and rehash every referenced raw object.
///
/// This is an integrity projection only. In particular, `Verified` does not
/// mean reviewed, promotable, compatible, trusted, or executable.
///
/// # Errors
///
/// Fails for an invalid root/index or a linked/special object. Missing,
/// digest-mismatched, and semantically mismatched regular objects are reported
/// as typed integrity statuses.
pub fn learning_store_status(
    state_root: impl AsRef<Path>,
) -> Result<LearningStoreProjection, LearningStoreError> {
    let state_root = prepare_state_root(state_root.as_ref())?;
    ensure_store_layout_safe(&state_root)?;
    let lock = acquire_effect_store_lock(&state_root, LEARNING_LOCK_RELATIVE_PATH)
        .map_err(map_lock_error)?;
    ensure_store_layout_safe(&state_root)?;
    recover_target(
        &state_root,
        &lock,
        LEARNING_INDEX_RELATIVE_PATH,
        MAX_INDEX_BYTES,
    )?;
    let index = load_index(&state_root)?;
    validate_index(&index)?;
    let mut records = Vec::with_capacity(index.records.len());
    for record in &index.records {
        records.push(LearningCaptureStatus {
            authority: LearningCaptureAuthority::NonAuthoritativeObservation,
            candidate_id: record.candidate_id.clone(),
            candidate_digest: record.candidate_digest.clone(),
            raw_sha256: record.raw_sha256.clone(),
            object_relative_path: record.object_relative_path.clone(),
            integrity: object_integrity(&state_root, record)?,
        });
    }
    drop(lock);
    Ok(LearningStoreProjection {
        authority: LearningCaptureAuthority::NonAuthoritativeObservation,
        records,
    })
}

fn validate_candidate_size(raw: &[u8]) -> Result<(), LearningStoreError> {
    let found = u64::try_from(raw.len()).unwrap_or(u64::MAX);
    if raw.is_empty() || found > MAX_CANDIDATE_BYTES {
        return Err(LearningStoreError::CandidateSize {
            found,
            maximum: MAX_CANDIDATE_BYTES,
        });
    }
    Ok(())
}

fn parse_candidate(
    raw: &[u8],
) -> Result<DomainPackLocalLearningCandidateDocument, LearningStoreError> {
    let text = std::str::from_utf8(raw)
        .map_err(|error| LearningStoreError::CandidateEncoding(error.to_string()))?;
    yaml_serde::from_str(text)
        .map_err(|error| LearningStoreError::CandidateEncoding(error.to_string()))
}

fn validate_candidate(
    document: &DomainPackLocalLearningCandidateDocument,
) -> Result<(), LearningStoreError> {
    if document.domain_pack_local_learning_candidate.authority
        != DomainPackLocalLearningAuthority::NonAuthoritativeObservation
    {
        return Err(LearningStoreError::CandidateContract(vec![
            "candidate authority must remain non_authoritative_observation".to_owned(),
        ]));
    }
    let issues = document.validate();
    if issues.is_empty() {
        Ok(())
    } else {
        Err(LearningStoreError::CandidateContract(
            issues
                .into_iter()
                .map(|issue| format!("{}: {}", issue.path, issue.message))
                .collect(),
        ))
    }
}

fn prepare_state_root(path: &Path) -> Result<PathBuf, LearningStoreError> {
    if path.as_os_str().is_empty() {
        return Err(LearningStoreError::InvalidStateRoot {
            path: path.to_path_buf(),
            reason: "path is empty".to_owned(),
        });
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(LearningStoreError::InvalidStateRoot {
                path: path.to_path_buf(),
                reason: "root must be a real directory, not a link or special file".to_owned(),
            });
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|source| LearningStoreError::Io {
                path: path.to_path_buf(),
                source: source.to_string(),
            })?;
        }
        Err(source) => {
            return Err(LearningStoreError::Io {
                path: path.to_path_buf(),
                source: source.to_string(),
            });
        }
    }
    fs::canonicalize(path).map_err(|source| LearningStoreError::InvalidStateRoot {
        path: path.to_path_buf(),
        reason: source.to_string(),
    })
}

fn ensure_store_layout_safe(root: &Path) -> Result<(), LearningStoreError> {
    ensure_existing_kind(root, LEARNING_ROOT_RELATIVE_PATH, true)?;
    ensure_existing_kind(root, "domain-pack-learning/objects", true)?;
    ensure_existing_kind(root, LEARNING_INDEX_RELATIVE_PATH, false)?;
    ensure_existing_kind(root, LEARNING_LOCK_RELATIVE_PATH, false)?;
    Ok(())
}

fn ensure_existing_kind(
    root: &Path,
    relative: &str,
    expect_directory: bool,
) -> Result<(), LearningStoreError> {
    ensure_confined_existing_path(root, relative)?;
    let path = root.join(relative);
    match fs::symlink_metadata(&path) {
        Ok(metadata)
            if (expect_directory && !metadata.file_type().is_dir())
                || (!expect_directory && !metadata.file_type().is_file()) =>
        {
            Err(LearningStoreError::InvalidStorePath {
                path,
                reason: if expect_directory {
                    "path must be a real directory".to_owned()
                } else {
                    "path must be a regular file".to_owned()
                },
            })
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(LearningStoreError::Io {
            path,
            source: source.to_string(),
        }),
    }
}

fn ensure_confined_existing_path(root: &Path, relative: &str) -> Result<(), LearningStoreError> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(LearningStoreError::InvalidStorePath {
            path: relative_path.to_path_buf(),
            reason: "path is not a confined normal relative path".to_owned(),
        });
    }
    let mut current = root.to_path_buf();
    for component in relative_path.components() {
        let Component::Normal(segment) = component else {
            unreachable!()
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(LearningStoreError::InvalidStorePath {
                        path: current,
                        reason: "symbolic link or junction is forbidden".to_owned(),
                    });
                }
                let canonical =
                    fs::canonicalize(&current).map_err(|source| LearningStoreError::Io {
                        path: current.clone(),
                        source: source.to_string(),
                    })?;
                if !canonical.starts_with(root) {
                    return Err(LearningStoreError::InvalidStorePath {
                        path: canonical,
                        reason: "existing component escapes canonical state root".to_owned(),
                    });
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(source) => {
                return Err(LearningStoreError::Io {
                    path: current,
                    source: source.to_string(),
                });
            }
        }
    }
    Ok(())
}

fn ensure_relative_regular_or_absent(
    root: &Path,
    relative: &str,
) -> Result<(), LearningStoreError> {
    ensure_confined_existing_path(root, relative)?;
    let path = root.join(relative);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            Err(LearningStoreError::InvalidStorePath {
                path,
                reason: "path must be a regular file".to_owned(),
            })
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(LearningStoreError::Io {
            path,
            source: source.to_string(),
        }),
    }
}

fn load_index(root: &Path) -> Result<LearningIndex, LearningStoreError> {
    ensure_relative_regular_or_absent(root, LEARNING_INDEX_RELATIVE_PATH)?;
    let path = root.join(LEARNING_INDEX_RELATIVE_PATH);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(LearningIndex::default())
        }
        Err(source) => {
            return Err(LearningStoreError::Io {
                path,
                source: source.to_string(),
            })
        }
    };
    if metadata.len() > MAX_INDEX_BYTES {
        return Err(LearningStoreError::CorruptIndex(format!(
            "index exceeds byte limit {MAX_INDEX_BYTES}"
        )));
    }
    let raw = fs::read(&path).map_err(|source| LearningStoreError::Io {
        path,
        source: source.to_string(),
    })?;
    serde_json::from_slice(&raw)
        .map_err(|error| LearningStoreError::CorruptIndex(error.to_string()))
}

fn validate_index(index: &LearningIndex) -> Result<(), LearningStoreError> {
    if index.schema_version != INDEX_SCHEMA_VERSION {
        return Err(LearningStoreError::CorruptIndex(format!(
            "unsupported schema version {}",
            index.schema_version
        )));
    }
    if index.records.len() > MAX_CAPTURE_RECORDS {
        return Err(LearningStoreError::ResourceLimit {
            resource: "capture records",
            maximum: MAX_CAPTURE_RECORDS,
        });
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut digests = std::collections::BTreeSet::new();
    for record in &index.records {
        if record.candidate_id.trim().is_empty()
            || !is_sha256_hex(&record.candidate_digest)
            || !is_sha256_hex(&record.raw_sha256)
            || record.object_relative_path
                != format!(
                    "{LEARNING_ROOT_RELATIVE_PATH}/objects/{}",
                    record.raw_sha256
                )
        {
            return Err(LearningStoreError::CorruptIndex(
                "record has an invalid identifier, digest, or object path".to_owned(),
            ));
        }
        if !ids.insert(&record.candidate_id) || !digests.insert(&record.candidate_digest) {
            return Err(LearningStoreError::CorruptIndex(
                "duplicate candidate id or candidate digest".to_owned(),
            ));
        }
    }
    Ok(())
}

fn persist_index(
    root: &Path,
    lock: &forge_core_store::EffectStoreLock,
    index: &LearningIndex,
) -> Result<(), LearningStoreError> {
    let raw = serde_json_canonicalizer::to_vec(index)
        .map_err(|error| LearningStoreError::CorruptIndex(error.to_string()))?;
    let path = root.join(LEARNING_INDEX_RELATIVE_PATH);
    let expected = match fs::read(&path) {
        Ok(previous) => Some(forge_core_store::sha256_content_hash(&previous)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(LearningStoreError::Io {
                path,
                source: source.to_string(),
            })
        }
    };
    replace_file_crash_safe_under_lock(
        root,
        lock,
        LEARNING_LOCK_RELATIVE_PATH,
        LEARNING_INDEX_RELATIVE_PATH,
        expected.as_deref(),
        &raw,
        MAX_INDEX_BYTES,
    )
    .map_err(map_persistence_error)?;
    Ok(())
}

fn persist_immutable_object(
    root: &Path,
    lock: &forge_core_store::EffectStoreLock,
    relative_path: &str,
    raw: &[u8],
) -> Result<(), LearningStoreError> {
    let path = root.join(relative_path);
    if let Some(existing) = read_regular_bounded(&path, MAX_CANDIDATE_BYTES)? {
        if existing == raw && sha256_hex(&existing) == sha256_hex(raw) {
            return Ok(());
        }
        return Err(LearningStoreError::InvalidStorePath {
            path,
            reason: "content-addressed object already contains different bytes".to_owned(),
        });
    }
    replace_file_crash_safe_under_lock(
        root,
        lock,
        LEARNING_LOCK_RELATIVE_PATH,
        relative_path,
        None,
        raw,
        MAX_CANDIDATE_BYTES,
    )
    .map_err(map_persistence_error)?;
    Ok(())
}

fn recover_target(
    root: &Path,
    lock: &forge_core_store::EffectStoreLock,
    relative_path: &str,
    maximum: u64,
) -> Result<(), LearningStoreError> {
    recover_file_crash_safe_under_lock(
        root,
        lock,
        LEARNING_LOCK_RELATIVE_PATH,
        relative_path,
        maximum,
    )
    .map_err(map_persistence_error)?;
    Ok(())
}

fn verify_existing_object(
    root: &Path,
    record: &LearningIndexRecord,
    submitted_raw: &[u8],
) -> Result<(), LearningStoreError> {
    ensure_relative_regular_or_absent(root, &record.object_relative_path)?;
    let path = root.join(&record.object_relative_path);
    let raw = read_regular_bounded(&path, MAX_CANDIDATE_BYTES)?.ok_or_else(|| {
        LearningStoreError::InvalidStorePath {
            path: path.clone(),
            reason: "indexed immutable object is missing".to_owned(),
        }
    })?;
    if raw != submitted_raw || sha256_hex(&raw) != record.raw_sha256 {
        return Err(LearningStoreError::InvalidStorePath {
            path,
            reason: "indexed immutable object differs from submitted exact bytes".to_owned(),
        });
    }
    Ok(())
}

fn object_integrity(
    root: &Path,
    record: &LearningIndexRecord,
) -> Result<LearningObjectIntegrity, LearningStoreError> {
    ensure_relative_regular_or_absent(root, &record.object_relative_path)?;
    let path = root.join(&record.object_relative_path);
    let Some(raw) = read_regular_bounded(&path, MAX_CANDIDATE_BYTES)? else {
        return Ok(LearningObjectIntegrity::Missing);
    };
    if sha256_hex(&raw) != record.raw_sha256 {
        return Ok(LearningObjectIntegrity::DigestMismatch);
    }
    let Ok(document) = parse_candidate(&raw) else {
        return Ok(LearningObjectIntegrity::CandidateMismatch);
    };
    let candidate = &document.domain_pack_local_learning_candidate;
    if candidate.candidate_id.0 != record.candidate_id
        || candidate.candidate_digest != record.candidate_digest
        || candidate_self_digest(&document).ok().as_deref() != Some(&record.candidate_digest)
        || !document.validate().is_empty()
    {
        return Ok(LearningObjectIntegrity::CandidateMismatch);
    }
    Ok(LearningObjectIntegrity::Verified)
}

fn read_regular_bounded(path: &Path, maximum: u64) -> Result<Option<Vec<u8>>, LearningStoreError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(LearningStoreError::Io {
                path: path.to_path_buf(),
                source: source.to_string(),
            })
        }
    };
    if !metadata.file_type().is_file() {
        return Err(LearningStoreError::InvalidStorePath {
            path: path.to_path_buf(),
            reason: "indexed object is linked or is not a regular file".to_owned(),
        });
    }
    if metadata.len() > maximum {
        return Err(LearningStoreError::CandidateSize {
            found: metadata.len(),
            maximum,
        });
    }
    fs::read(path)
        .map(Some)
        .map_err(|source| LearningStoreError::Io {
            path: path.to_path_buf(),
            source: source.to_string(),
        })
}

fn receipt(
    record: &LearningIndexRecord,
    disposition: LearningCaptureDisposition,
) -> LearningCaptureReceipt {
    LearningCaptureReceipt {
        authority: LearningCaptureAuthority::NonAuthoritativeObservation,
        disposition,
        candidate_id: record.candidate_id.clone(),
        candidate_digest: record.candidate_digest.clone(),
        raw_sha256: record.raw_sha256.clone(),
        object_relative_path: record.object_relative_path.clone(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[allow(clippy::needless_pass_by_value)]
fn map_lock_error(error: EffectStoreLockError) -> LearningStoreError {
    LearningStoreError::Lock(format!("{error:?}"))
}

#[allow(clippy::needless_pass_by_value)]
fn map_persistence_error(error: CrashReplaceError) -> LearningStoreError {
    LearningStoreError::Persistence(error.to_string())
}
