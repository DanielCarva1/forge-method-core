//! Durable, explicitly non-authoritative capture for local Domain Pack learning.
//!
//! Capturing an observation preserves its exact submitted bytes. It cannot
//! review, promote, install, activate, or otherwise turn the observation into
//! authority. Those operations deliberately have no API in this crate.

use forge_core_contracts::{
    DomainPackLocalLearningAuthority, DomainPackLocalLearningCandidateDocument,
};
use forge_core_store::crash_replace::CrashReplaceError;
use forge_core_store::retained_project_tree::RetainedProjectTree;
use forge_core_store::{
    acquire_effect_store_lock, EffectStoreLock, EffectStoreLockError, RetainedCrashReplaceRead,
    RetainedCrashReplaceSession, RetainedEffectStoreIo, RetainedEffectStoreLeafWitness,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io;
use std::marker::PhantomData;
use std::path::{Component, Path, PathBuf};

pub const LEARNING_ROOT_RELATIVE_PATH: &str = "domain-pack-learning";
pub const LEARNING_LOCK_RELATIVE_PATH: &str = "domain-pack-learning/capture.lock";
pub const LEARNING_INDEX_RELATIVE_PATH: &str = "domain-pack-learning/index.json";
pub const LEARNING_GENERATION_POINTER_RELATIVE_PATH: &str = "domain-pack-learning/generation.json";
pub const LEARNING_GENERATIONS_RELATIVE_PATH: &str = "domain-pack-learning/generations";
const LEARNING_INDEX_IO_RELATIVE_PATH: &str = "index.json";
const LEARNING_OBJECTS_IO_RELATIVE_PATH: &str = "objects";
const LEARNING_GENERATION_POINTER_IO_RELATIVE_PATH: &str = "generation.json";
const LEARNING_GENERATIONS_IO_RELATIVE_PATH: &str = "generations";
pub const MAX_CANDIDATE_BYTES: u64 = 1024 * 1024;
pub const MAX_INDEX_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_CAPTURE_RECORDS: usize = 10_000;
const MAX_GENERATION_BYTES: u64 =
    MAX_INDEX_BYTES + (MAX_CAPTURE_RECORDS as u64) * (MAX_CANDIDATE_BYTES * 6 + 1024);
const MAX_GENERATION_POINTER_BYTES: u64 = MAX_GENERATION_BYTES + 4096;
const MAX_PRISTINE_TREE_ENTRIES: usize = 4;
const MAX_PRISTINE_TREE_BYTES: u64 = MAX_CANDIDATE_BYTES * 7 + 4096;
const INDEX_SCHEMA_VERSION: &str = "0.1";
const GENERATION_SCHEMA_VERSION: &str = "0.1";
const GENERATION_POINTER_SCHEMA_VERSION: &str = "0.1";

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

/// One exact file projected from a retained canonical learning generation.
///
/// The bytes are owned by the immutable generation selected by the Store-owned
/// pointer retained in the enclosing closure. This value cannot outlive that
/// closure's producer lock.
pub struct LearningStoreRawFile<'lock> {
    relative_path: String,
    raw_bytes: Vec<u8>,
    _authority: PhantomData<&'lock EffectStoreLock>,
}

impl fmt::Debug for LearningStoreRawFile<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LearningStoreRawFile")
            .field("relative_path", &self.relative_path)
            .field("byte_length", &self.raw_bytes.len())
            .finish_non_exhaustive()
    }
}

impl LearningStoreRawFile<'_> {
    #[must_use]
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    #[must_use]
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw_bytes
    }
}

/// Canonical index and raw objects from one atomically selected generation.
///
/// The complete decision material comes from the retained pointer record. The
/// matching content-addressed generation leaf remains retained as a second
/// namespace validation, but legacy `index.json` and `objects/*` files are only
/// compatibility projections and never authorize this result.
#[derive(Debug)]
pub struct LearningStoreRawClosure<'lock> {
    index: LearningStoreRawFile<'lock>,
    objects: Vec<LearningStoreRawFile<'lock>>,
    _authority: RetainedLearningGeneration<'lock>,
}

impl<'lock> LearningStoreRawClosure<'lock> {
    #[must_use]
    pub fn index(&self) -> Option<&LearningStoreRawFile<'lock>> {
        Some(&self.index)
    }

    #[must_use]
    pub fn objects(&self) -> &[LearningStoreRawFile<'lock>] {
        &self.objects
    }
}

/// Opaque retained producer lock for raw learning-store capture.
#[derive(Debug)]
pub struct DomainPackLearningStoreLock {
    lock: EffectStoreLock,
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
    CorruptGeneration(String),
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
            Self::CorruptGeneration(reason) => {
                write!(formatter, "corrupt learning generation: {reason}")
            }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LearningGenerationObject {
    object_relative_path: String,
    raw_sha256: String,
    raw_utf8: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LearningGeneration {
    schema_version: String,
    learning_root_relative_path: String,
    lock_relative_path: String,
    store_authority_sha256: String,
    index: LearningIndex,
    objects: Vec<LearningGenerationObject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LearningGenerationPointer {
    schema_version: String,
    learning_root_relative_path: String,
    lock_relative_path: String,
    store_authority_sha256: String,
    operation_nonce: String,
    generation_sha256: String,
    generation_relative_path: String,
    generation: LearningGeneration,
}

struct ReconciledLearningGeneration<'lock> {
    value: LearningGenerationPointer,
    pointer_session: RetainedCrashReplaceSession<'lock>,
    generation_witness: RetainedEffectStoreLeafWitness<'lock>,
}

struct RetainedLearningGeneration<'lock> {
    value: LearningGenerationPointer,
    pointer_anchor: RetainedCrashReplaceRead<'lock>,
    generation_witness: RetainedEffectStoreLeafWitness<'lock>,
}

enum LearningGenerationState<'lock> {
    Present(ReconciledLearningGeneration<'lock>),
    Pristine(PristineLearningStore<'lock>),
}

struct PristineLearningStore<'lock> {
    pointer_session: RetainedCrashReplaceSession<'lock>,
    tree_anchor: RetainedProjectTree,
}

struct PreparedLearningBootstrap<'lock> {
    pointer_session: RetainedCrashReplaceSession<'lock>,
    tree_anchor: RetainedProjectTree,
}

impl fmt::Debug for RetainedLearningGeneration<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedLearningGeneration")
            .field("generation_sha256", &self.value.generation_sha256)
            .field("record_count", &self.value.generation.index.records.len())
            .finish_non_exhaustive()
    }
}

impl Default for LearningIndex {
    fn default() -> Self {
        Self {
            schema_version: INDEX_SCHEMA_VERSION.to_owned(),
            records: Vec::new(),
        }
    }
}

impl LearningGeneration {
    fn empty(store_authority_sha256: String) -> Self {
        Self {
            schema_version: GENERATION_SCHEMA_VERSION.to_owned(),
            learning_root_relative_path: LEARNING_ROOT_RELATIVE_PATH.to_owned(),
            lock_relative_path: LEARNING_LOCK_RELATIVE_PATH.to_owned(),
            store_authority_sha256,
            index: LearningIndex::default(),
            objects: Vec::new(),
        }
    }
}

impl<'lock> PristineLearningStore<'lock> {
    fn revalidate(
        &self,
        io: &RetainedEffectStoreIo<'lock>,
        generations_io: &RetainedEffectStoreIo<'lock>,
    ) -> Result<(), LearningStoreError> {
        revalidate_learning_tree_anchor(
            &self.tree_anchor,
            &pristine_learning_tree_digest()?,
            io,
            generations_io,
        )?;
        if self.pointer_session.raw_bytes().is_some() {
            return Err(LearningStoreError::CorruptGeneration(
                "Store-minted pristine authority unexpectedly contains a generation pointer"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn prepare_after_generation(
        self,
        io: &RetainedEffectStoreIo<'lock>,
        generations_io: &RetainedEffectStoreIo<'lock>,
        generation_sha256: &str,
    ) -> Result<PreparedLearningBootstrap<'lock>, LearningStoreError> {
        let tree_anchor = retain_learning_tree_anchor(
            io,
            generations_io,
            &prepared_learning_tree_digest(generation_sha256)?,
        )?;
        let prepared = PreparedLearningBootstrap {
            pointer_session: self.pointer_session,
            tree_anchor,
        };
        prepared.revalidate(io, generations_io, generation_sha256)?;
        Ok(prepared)
    }
}

impl<'lock> PreparedLearningBootstrap<'lock> {
    fn revalidate(
        &self,
        io: &RetainedEffectStoreIo<'lock>,
        generations_io: &RetainedEffectStoreIo<'lock>,
        generation_sha256: &str,
    ) -> Result<(), LearningStoreError> {
        revalidate_learning_tree_anchor(
            &self.tree_anchor,
            &prepared_learning_tree_digest(generation_sha256)?,
            io,
            generations_io,
        )
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

/// Retain the exact learning-store producer lock for descriptor-relative raw
/// closure capture.
///
/// # Errors
///
/// Fails closed for an invalid root/layout, producer-boundary failure, or lock
/// contention.
pub fn lock_domain_pack_learning_store(
    state_root: impl AsRef<Path>,
) -> Result<DomainPackLearningStoreLock, LearningStoreError> {
    let state_root = prepare_state_root(state_root.as_ref())?;
    ensure_store_layout_safe(&state_root)?;
    let lock = acquire_effect_store_lock(&state_root, LEARNING_LOCK_RELATIVE_PATH)
        .map_err(map_lock_error)?;
    lock.retained_store_io().map_err(map_lock_error)?;
    Ok(DomainPackLearningStoreLock { lock })
}

impl DomainPackLearningStoreLock {
    /// Capture the canonical index and complete raw-object closure from one
    /// immutable generation selected by one retained pointer record.
    ///
    /// # Errors
    ///
    /// Fails closed if the ambient root, retained lock, pointer, content-addressed
    /// generation leaf, or embedded generation validation changed or is unsafe.
    pub fn snapshot_raw_closure(&self) -> Result<LearningStoreRawClosure<'_>, LearningStoreError> {
        let io = self.lock.retained_store_io().map_err(map_lock_error)?;
        let generations_io = retain_generations_io(&io)?;
        let mut authority = load_or_initialize_generation(&io, &generations_io)?;
        let index_raw = canonical_index_bytes(&authority.value.generation.index)?;
        let mut objects = Vec::with_capacity(authority.value.generation.objects.len());
        for object in &authority.value.generation.objects {
            objects.push(LearningStoreRawFile {
                relative_path: object.object_relative_path.clone(),
                raw_bytes: object.raw_utf8.as_bytes().to_vec(),
                _authority: PhantomData,
            });
        }
        let index = LearningStoreRawFile {
            relative_path: LEARNING_INDEX_RELATIVE_PATH.to_owned(),
            raw_bytes: index_raw,
            _authority: PhantomData,
        };
        revalidate_generation_authority(&mut authority, &io, &generations_io)?;
        Ok(LearningStoreRawClosure {
            index,
            objects,
            _authority: authority,
        })
    }
}

/// Capture exact candidate bytes under a retained OS lock.
///
/// The operation is idempotent for an identical candidate digest and fails
/// explicitly when one candidate id is reused for different content. The
/// capture linearization point is publication of one canonical pointer record
/// containing the complete new generation and selecting its matching
/// content-addressed immutable generation leaf.
///
/// # Errors
///
/// Fails closed for malformed or oversized input, invalid self-digests,
/// equivocation, corrupt state, links/special files, lock failure, or any
/// durability error in authoritative generation publication.
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
    let io = lock.retained_store_io().map_err(map_lock_error)?;
    let generations_io = retain_generations_io(&io)?;
    let state = load_generation_authority(&io, &generations_io)?;
    let (state, mut generation) = match state {
        LearningGenerationState::Present(previous) => {
            if let Some(existing) = previous
                .value
                .generation
                .index
                .records
                .iter()
                .find(|record| record.candidate_id == candidate.candidate_id.0)
                .cloned()
            {
                if existing.candidate_digest != candidate.candidate_digest {
                    return Err(LearningStoreError::CandidateIdConflict {
                        candidate_id: candidate.candidate_id.0.clone(),
                        existing_digest: existing.candidate_digest.clone(),
                        submitted_digest: candidate.candidate_digest.clone(),
                    });
                }
                verify_existing_generation_object(
                    &previous.value.generation,
                    &existing,
                    raw_candidate_yaml,
                )?;
                let mut retained = retain_generation_for_read(previous, &io, &generations_io)?;
                best_effort_project_generation(&io, &retained.value.generation);
                revalidate_generation_authority(&mut retained, &io, &generations_io)?;
                let result = receipt(&existing, LearningCaptureDisposition::AlreadyPresent);
                return Ok(result);
            }
            if previous.value.generation.index.records.len() >= MAX_CAPTURE_RECORDS {
                return Err(LearningStoreError::ResourceLimit {
                    resource: "capture records",
                    maximum: MAX_CAPTURE_RECORDS,
                });
            }
            let generation = previous.value.generation.clone();
            (LearningGenerationState::Present(previous), generation)
        }
        LearningGenerationState::Pristine(pristine) => {
            let store_authority_sha256 = mint_store_authority_binding(&io)?;
            (
                LearningGenerationState::Pristine(pristine),
                LearningGeneration::empty(store_authority_sha256),
            )
        }
    };

    let raw_sha256 = sha256_hex(raw_candidate_yaml);
    let object_relative_path = format!("{LEARNING_ROOT_RELATIVE_PATH}/objects/{raw_sha256}");
    let record = LearningIndexRecord {
        authority: LearningCaptureAuthority::NonAuthoritativeObservation,
        candidate_id: candidate.candidate_id.0.clone(),
        candidate_digest: candidate.candidate_digest.clone(),
        raw_sha256: raw_sha256.clone(),
        object_relative_path: object_relative_path.clone(),
    };
    let object = LearningGenerationObject {
        object_relative_path,
        raw_sha256,
        raw_utf8: std::str::from_utf8(raw_candidate_yaml)
            .map_err(|error| LearningStoreError::CandidateEncoding(error.to_string()))?
            .to_owned(),
    };
    let store_authority_sha256 = generation.store_authority_sha256.clone();
    generation.index.records.push(record.clone());
    generation.objects.push(object);
    validate_generation(&generation, &store_authority_sha256)?;

    let committed = commit_generation(&io, &generations_io, state, generation)?;
    // Legacy index/object projection is explicitly non-authoritative. It may run
    // after the atomic pointer selection, but its outcome cannot change success.
    // The exact installed pointer and immutable generation anchors stay live
    // through receipt construction.
    best_effort_project_generation(&io, &committed.value.generation);
    let result = receipt(&record, LearningCaptureDisposition::Captured);
    Ok(result)
}

/// List captured observations from one exact canonical generation.
///
/// This is an integrity projection only. In particular, `Verified` does not
/// mean reviewed, promotable, compatible, trusted, or executable. Legacy
/// mutable index/object projections are intentionally not consulted.
///
/// # Errors
///
/// Fails closed for an invalid root, pointer record, content-addressed
/// generation, embedded index, or raw candidate closure.
pub fn learning_store_status(
    state_root: impl AsRef<Path>,
) -> Result<LearningStoreProjection, LearningStoreError> {
    let state_root = prepare_state_root(state_root.as_ref())?;
    ensure_store_layout_safe(&state_root)?;
    let lock = acquire_effect_store_lock(&state_root, LEARNING_LOCK_RELATIVE_PATH)
        .map_err(map_lock_error)?;
    let io = lock.retained_store_io().map_err(map_lock_error)?;
    let generations_io = retain_generations_io(&io)?;
    let mut authority = load_or_initialize_generation(&io, &generations_io)?;
    let generation = &authority.value.generation;
    let mut records = Vec::with_capacity(generation.index.records.len());
    for (record, object) in generation.index.records.iter().zip(&generation.objects) {
        records.push(LearningCaptureStatus {
            authority: LearningCaptureAuthority::NonAuthoritativeObservation,
            candidate_id: record.candidate_id.clone(),
            candidate_digest: record.candidate_digest.clone(),
            raw_sha256: record.raw_sha256.clone(),
            object_relative_path: record.object_relative_path.clone(),
            integrity: generation_object_integrity(record, object),
        });
    }
    revalidate_generation_authority(&mut authority, &io, &generations_io)?;
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
    ensure_existing_kind(root, LEARNING_GENERATIONS_RELATIVE_PATH, true)?;
    ensure_existing_kind(root, LEARNING_GENERATION_POINTER_RELATIVE_PATH, false)?;
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

fn load_generation_authority<'lock>(
    io: &RetainedEffectStoreIo<'lock>,
    generations_io: &RetainedEffectStoreIo<'lock>,
) -> Result<LearningGenerationState<'lock>, LearningStoreError> {
    let pointer_session = io
        .reconcile_file_crash_safe(
            Path::new(LEARNING_GENERATION_POINTER_IO_RELATIVE_PATH),
            MAX_GENERATION_POINTER_BYTES,
        )
        .map_err(map_persistence_error)?;
    let Some(pointer_raw) = pointer_session.raw_bytes() else {
        let pristine = retain_pristine_learning_store(io, generations_io, pointer_session)?;
        return Ok(LearningGenerationState::Pristine(pristine));
    };
    let value: LearningGenerationPointer = serde_json::from_slice(pointer_raw)
        .map_err(|error| LearningStoreError::CorruptGeneration(error.to_string()))?;
    validate_generation_pointer(&value)?;
    if pointer_raw != canonical_pointer_bytes(&value)?.as_slice() {
        return Err(LearningStoreError::CorruptGeneration(
            "generation pointer record is not canonical JSON".to_owned(),
        ));
    }
    let generation_leaf =
        generation_leaf_relative(&value.generation_relative_path, &value.generation_sha256)?;
    let generation_witness =
        read_generation_witness(generations_io, generation_leaf)?.ok_or_else(|| {
            LearningStoreError::InvalidStorePath {
                path: generations_io.display_path().join(generation_leaf),
                reason: "selected content-addressed learning generation is missing".to_owned(),
            }
        })?;
    let expected_generation = canonical_generation_bytes(&value.generation)?;
    if generation_witness.raw_bytes() != expected_generation.as_slice()
        || sha256_hex(generation_witness.raw_bytes()) != value.generation_sha256
    {
        return Err(LearningStoreError::CorruptGeneration(
            "selected content-addressed generation does not match its canonical pointer record"
                .to_owned(),
        ));
    }
    let mut authority = ReconciledLearningGeneration {
        value,
        pointer_session,
        generation_witness,
    };
    revalidate_reconciled_generation(&mut authority, io, generations_io)?;
    Ok(LearningGenerationState::Present(authority))
}

fn load_or_initialize_generation<'lock>(
    io: &RetainedEffectStoreIo<'lock>,
    generations_io: &RetainedEffectStoreIo<'lock>,
) -> Result<RetainedLearningGeneration<'lock>, LearningStoreError> {
    let authority = match load_generation_authority(io, generations_io)? {
        LearningGenerationState::Present(authority) => {
            retain_generation_for_read(authority, io, generations_io)?
        }
        LearningGenerationState::Pristine(pristine) => {
            let store_authority_sha256 = mint_store_authority_binding(io)?;
            commit_generation(
                io,
                generations_io,
                LearningGenerationState::Pristine(pristine),
                LearningGeneration::empty(store_authority_sha256),
            )?
        }
    };
    best_effort_project_generation(io, &authority.value.generation);
    Ok(authority)
}

fn commit_generation<'lock>(
    io: &RetainedEffectStoreIo<'lock>,
    generations_io: &RetainedEffectStoreIo<'lock>,
    mut previous: LearningGenerationState<'lock>,
    generation: LearningGeneration,
) -> Result<RetainedLearningGeneration<'lock>, LearningStoreError> {
    let store_authority_sha256 = generation.store_authority_sha256.clone();
    validate_generation(&generation, &store_authority_sha256)?;
    match &mut previous {
        LearningGenerationState::Present(authority) => {
            revalidate_reconciled_generation(authority, io, generations_io)?;
        }
        LearningGenerationState::Pristine(pristine) => {
            pristine.revalidate(io, generations_io)?;
        }
    }

    let generation_raw = canonical_generation_bytes(&generation)?;
    let generation_sha256 = sha256_hex(&generation_raw);
    let generation_relative_path =
        format!("{LEARNING_GENERATIONS_RELATIVE_PATH}/{generation_sha256}");
    let generation_leaf = PathBuf::from(&generation_sha256);
    let mut generation_witness = persist_immutable_generation(
        generations_io,
        &generation_leaf,
        &generation_raw,
        &generation_sha256,
    )?;
    let operation_nonce = io
        .mint_operation_nonce()
        .map_err(|source| retained_io_error(io, Path::new(""), source))?;
    let pointer = LearningGenerationPointer {
        schema_version: GENERATION_POINTER_SCHEMA_VERSION.to_owned(),
        learning_root_relative_path: LEARNING_ROOT_RELATIVE_PATH.to_owned(),
        lock_relative_path: LEARNING_LOCK_RELATIVE_PATH.to_owned(),
        store_authority_sha256,
        operation_nonce,
        generation_sha256,
        generation_relative_path,
        generation,
    };
    validate_generation_pointer(&pointer)?;
    generation_witness
        .revalidate()
        .map_err(|source| retained_io_error(generations_io, &generation_leaf, source))?;

    let (pointer_session, _previous_generation_anchor, pristine_tree_anchor) = match previous {
        LearningGenerationState::Present(authority) => {
            let ReconciledLearningGeneration {
                pointer_session,
                generation_witness,
                ..
            } = authority;
            (pointer_session, Some(generation_witness), None)
        }
        LearningGenerationState::Pristine(pristine) => {
            let prepared = pristine.prepare_after_generation(
                io,
                generations_io,
                &pointer.generation_sha256,
            )?;
            let PreparedLearningBootstrap {
                pointer_session,
                tree_anchor,
            } = prepared;
            (pointer_session, None, Some(tree_anchor))
        }
    };
    if let Some(tree_anchor) = pristine_tree_anchor.as_ref() {
        revalidate_learning_tree_anchor(
            tree_anchor,
            &prepared_learning_tree_digest(&pointer.generation_sha256)?,
            io,
            generations_io,
        )?;
    }
    let pointer_anchor = persist_generation_pointer(pointer_session, &pointer)?;
    // Capture linearization point: the exact reconciliation session installs the
    // fixed pointer while the predecessor or pristine tree and the matching new
    // immutable generation handles all remain retained. No pointer pathname is
    // reopened after marker finalization.
    Ok(RetainedLearningGeneration {
        value: pointer,
        pointer_anchor,
        generation_witness,
    })
}

fn validate_generation_pointer(
    pointer: &LearningGenerationPointer,
) -> Result<(), LearningStoreError> {
    if pointer.schema_version != GENERATION_POINTER_SCHEMA_VERSION
        || pointer.learning_root_relative_path != LEARNING_ROOT_RELATIVE_PATH
        || pointer.lock_relative_path != LEARNING_LOCK_RELATIVE_PATH
        || pointer.generation.store_authority_sha256 != pointer.store_authority_sha256
        || !is_sha256_content_hash(&pointer.store_authority_sha256)
        || !is_operation_nonce(&pointer.operation_nonce)
        || !is_sha256_hex(&pointer.generation_sha256)
        || pointer.generation_relative_path
            != format!(
                "{LEARNING_GENERATIONS_RELATIVE_PATH}/{}",
                pointer.generation_sha256
            )
    {
        return Err(LearningStoreError::CorruptGeneration(
            "generation pointer has an invalid schema, root, lock, operation, digest, or path binding"
                .to_owned(),
        ));
    }
    validate_generation(&pointer.generation, &pointer.store_authority_sha256)?;
    let generation_raw = canonical_generation_bytes(&pointer.generation)?;
    if sha256_hex(&generation_raw) != pointer.generation_sha256 {
        return Err(LearningStoreError::CorruptGeneration(
            "generation pointer digest does not bind its embedded canonical generation".to_owned(),
        ));
    }
    let pointer_raw = canonical_pointer_bytes(pointer)?;
    if u64::try_from(pointer_raw.len()).unwrap_or(u64::MAX) > MAX_GENERATION_POINTER_BYTES {
        return Err(LearningStoreError::CorruptGeneration(format!(
            "generation pointer exceeds byte limit {MAX_GENERATION_POINTER_BYTES}"
        )));
    }
    Ok(())
}

fn validate_generation(
    generation: &LearningGeneration,
    expected_store_authority_sha256: &str,
) -> Result<(), LearningStoreError> {
    if generation.schema_version != GENERATION_SCHEMA_VERSION
        || generation.learning_root_relative_path != LEARNING_ROOT_RELATIVE_PATH
        || generation.lock_relative_path != LEARNING_LOCK_RELATIVE_PATH
        || generation.store_authority_sha256 != expected_store_authority_sha256
        || !is_sha256_content_hash(&generation.store_authority_sha256)
    {
        return Err(LearningStoreError::CorruptGeneration(
            "generation has an invalid schema, root, or lock binding".to_owned(),
        ));
    }
    validate_index(&generation.index)?;
    let index_raw = canonical_index_bytes(&generation.index)?;
    if u64::try_from(index_raw.len()).unwrap_or(u64::MAX) > MAX_INDEX_BYTES {
        return Err(LearningStoreError::CorruptIndex(format!(
            "index exceeds byte limit {MAX_INDEX_BYTES}"
        )));
    }
    if generation.objects.len() != generation.index.records.len() {
        return Err(LearningStoreError::CorruptGeneration(
            "generation object closure does not have exactly one object per index record"
                .to_owned(),
        ));
    }
    for (record, object) in generation.index.records.iter().zip(&generation.objects) {
        validate_generation_object(record, object)?;
    }
    let raw = canonical_generation_bytes(generation)?;
    if u64::try_from(raw.len()).unwrap_or(u64::MAX) > MAX_GENERATION_BYTES {
        return Err(LearningStoreError::CorruptGeneration(format!(
            "generation exceeds byte limit {MAX_GENERATION_BYTES}"
        )));
    }
    Ok(())
}

fn validate_generation_object(
    record: &LearningIndexRecord,
    object: &LearningGenerationObject,
) -> Result<(), LearningStoreError> {
    let raw = object.raw_utf8.as_bytes();
    if object.object_relative_path != record.object_relative_path
        || object.raw_sha256 != record.raw_sha256
        || sha256_hex(raw) != record.raw_sha256
    {
        return Err(LearningStoreError::CorruptGeneration(
            "generation object bytes, digest, or path do not match the canonical index".to_owned(),
        ));
    }
    validate_candidate_size(raw).map_err(|error| {
        LearningStoreError::CorruptGeneration(format!("generation object size is invalid: {error}"))
    })?;
    let document = parse_candidate(raw).map_err(|error| {
        LearningStoreError::CorruptGeneration(format!(
            "generation object encoding is invalid: {error}"
        ))
    })?;
    validate_candidate(&document).map_err(|error| {
        LearningStoreError::CorruptGeneration(format!(
            "generation object contract is invalid: {error}"
        ))
    })?;
    let candidate = &document.domain_pack_local_learning_candidate;
    let computed = candidate_self_digest(&document).map_err(|error| {
        LearningStoreError::CorruptGeneration(format!(
            "generation object digest is invalid: {error}"
        ))
    })?;
    if candidate.candidate_id.0 != record.candidate_id
        || candidate.candidate_digest != record.candidate_digest
        || computed != record.candidate_digest
    {
        return Err(LearningStoreError::CorruptGeneration(
            "generation object candidate identity or self-digest does not match the canonical index"
                .to_owned(),
        ));
    }
    Ok(())
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
    let mut raw_digests = std::collections::BTreeSet::new();
    for record in &index.records {
        if record.authority != LearningCaptureAuthority::NonAuthoritativeObservation
            || record.candidate_id.trim().is_empty()
            || !is_sha256_hex(&record.candidate_digest)
            || !is_sha256_hex(&record.raw_sha256)
            || record.object_relative_path
                != format!(
                    "{LEARNING_ROOT_RELATIVE_PATH}/objects/{}",
                    record.raw_sha256
                )
        {
            return Err(LearningStoreError::CorruptIndex(
                "record has an invalid authority, identifier, digest, or object path".to_owned(),
            ));
        }
        if !ids.insert(&record.candidate_id)
            || !digests.insert(&record.candidate_digest)
            || !raw_digests.insert(&record.raw_sha256)
        {
            return Err(LearningStoreError::CorruptIndex(
                "duplicate candidate id, candidate digest, or raw digest".to_owned(),
            ));
        }
    }
    Ok(())
}

fn verify_existing_generation_object(
    generation: &LearningGeneration,
    record: &LearningIndexRecord,
    submitted_raw: &[u8],
) -> Result<(), LearningStoreError> {
    let object = generation
        .index
        .records
        .iter()
        .position(|candidate| candidate == record)
        .and_then(|position| generation.objects.get(position))
        .ok_or_else(|| {
            LearningStoreError::CorruptGeneration(
                "existing record has no object in the canonical generation".to_owned(),
            )
        })?;
    if object.raw_utf8.as_bytes() != submitted_raw
        || sha256_hex(object.raw_utf8.as_bytes()) != record.raw_sha256
    {
        return Err(LearningStoreError::CorruptGeneration(
            "existing canonical generation object differs from submitted exact bytes".to_owned(),
        ));
    }
    validate_generation_object(record, object)
}

fn generation_object_integrity(
    record: &LearningIndexRecord,
    object: &LearningGenerationObject,
) -> LearningObjectIntegrity {
    if object.object_relative_path != record.object_relative_path {
        LearningObjectIntegrity::Missing
    } else if object.raw_sha256 != record.raw_sha256
        || sha256_hex(object.raw_utf8.as_bytes()) != record.raw_sha256
    {
        LearningObjectIntegrity::DigestMismatch
    } else if validate_generation_object(record, object).is_err() {
        LearningObjectIntegrity::CandidateMismatch
    } else {
        LearningObjectIntegrity::Verified
    }
}

fn persist_immutable_generation<'lock>(
    io: &RetainedEffectStoreIo<'lock>,
    relative_path: &Path,
    raw: &[u8],
    expected_digest: &str,
) -> Result<RetainedEffectStoreLeafWitness<'lock>, LearningStoreError> {
    if sha256_hex(raw) != expected_digest || relative_path != Path::new(expected_digest) {
        return Err(LearningStoreError::CorruptGeneration(
            "content-addressed generation path does not match canonical bytes".to_owned(),
        ));
    }
    if let Some(mut existing) = read_generation_witness(io, relative_path)? {
        if existing.raw_bytes() != raw || sha256_hex(existing.raw_bytes()) != expected_digest {
            return Err(LearningStoreError::InvalidStorePath {
                path: io.display_path().join(relative_path),
                reason: "content-addressed generation already contains different bytes".to_owned(),
            });
        }
        existing
            .revalidate()
            .map_err(|source| retained_io_error(io, relative_path, source))?;
        return Ok(existing);
    }
    let mut created = io
        .write_new_file_synced(relative_path, raw, MAX_GENERATION_BYTES)
        .map_err(|source| retained_io_error(io, relative_path, source))?;
    if created.raw_bytes() != raw || sha256_hex(created.raw_bytes()) != expected_digest {
        return Err(LearningStoreError::InvalidStorePath {
            path: io.display_path().join(relative_path),
            reason: "created content-addressed generation witness has unexpected bytes".to_owned(),
        });
    }
    created
        .revalidate()
        .map_err(|source| retained_io_error(io, relative_path, source))?;
    Ok(created)
}

fn persist_generation_pointer<'lock>(
    session: RetainedCrashReplaceSession<'lock>,
    pointer: &LearningGenerationPointer,
) -> Result<RetainedCrashReplaceRead<'lock>, LearningStoreError> {
    let raw = canonical_pointer_bytes(pointer)?;
    let installed = session.replace(&raw).map_err(map_persistence_error)?;
    if installed.raw_bytes() != raw.as_slice() {
        return Err(LearningStoreError::Persistence(
            "committed generation pointer anchor does not contain the requested bytes".to_owned(),
        ));
    }
    Ok(installed)
}

fn best_effort_project_generation(io: &RetainedEffectStoreIo<'_>, generation: &LearningGeneration) {
    if let Ok(objects_io) = retain_objects_io(io) {
        for object in &generation.objects {
            let leaf = Path::new(&object.raw_sha256);
            let _ = persist_legacy_immutable_object(&objects_io, leaf, object.raw_utf8.as_bytes());
        }
    }
    let _ = persist_legacy_index_projection(io, &generation.index);
}

fn persist_legacy_index_projection(
    io: &RetainedEffectStoreIo<'_>,
    index: &LearningIndex,
) -> Result<(), LearningStoreError> {
    recover_target(
        io,
        Path::new(LEARNING_INDEX_IO_RELATIVE_PATH),
        MAX_INDEX_BYTES,
    )?;
    let raw = canonical_index_bytes(index)?;
    let mut expected = io
        .retain_file_crash_safe_expected_leaf(
            Path::new(LEARNING_INDEX_IO_RELATIVE_PATH),
            MAX_INDEX_BYTES,
        )
        .map_err(map_persistence_error)?;
    if expected
        .raw_bytes()
        .is_some_and(|bytes| bytes == raw.as_slice())
    {
        return Ok(());
    }
    let installed = io
        .replace_file_crash_safe(
            Path::new(LEARNING_INDEX_IO_RELATIVE_PATH),
            &mut expected,
            &raw,
            MAX_INDEX_BYTES,
        )
        .map_err(map_persistence_error)?;
    if installed.raw_bytes() != raw.as_slice() {
        return Err(LearningStoreError::Persistence(
            "legacy index projection does not contain the canonical generation index".to_owned(),
        ));
    }
    Ok(())
}

fn persist_legacy_immutable_object(
    io: &RetainedEffectStoreIo<'_>,
    relative_path: &Path,
    raw: &[u8],
) -> Result<(), LearningStoreError> {
    let expected_digest = relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| LearningStoreError::InvalidStorePath {
            path: io.display_path().join(relative_path),
            reason: "legacy object projection has no UTF-8 digest leaf".to_owned(),
        })?;
    if sha256_hex(raw) != expected_digest {
        return Err(LearningStoreError::InvalidStorePath {
            path: io.display_path().join(relative_path),
            reason: "legacy object projection path does not match canonical bytes".to_owned(),
        });
    }
    if let Some(mut existing) = read_object_witness(io, relative_path)? {
        if existing.raw_bytes() != raw || sha256_hex(existing.raw_bytes()) != expected_digest {
            return Err(LearningStoreError::InvalidStorePath {
                path: io.display_path().join(relative_path),
                reason: "legacy content-addressed object projection contains different bytes"
                    .to_owned(),
            });
        }
        existing
            .revalidate()
            .map_err(|source| retained_io_error(io, relative_path, source))?;
        return Ok(());
    }
    let mut created = io
        .write_new_file_synced(relative_path, raw, MAX_CANDIDATE_BYTES)
        .map_err(|source| retained_io_error(io, relative_path, source))?;
    created
        .revalidate()
        .map_err(|source| retained_io_error(io, relative_path, source))
}

fn recover_target(
    io: &RetainedEffectStoreIo<'_>,
    relative_path: &Path,
    maximum: u64,
) -> Result<(), LearningStoreError> {
    io.recover_file_crash_safe(relative_path, maximum)
        .map_err(map_persistence_error)?;
    Ok(())
}

fn retain_objects_io<'lock>(
    io: &RetainedEffectStoreIo<'lock>,
) -> Result<RetainedEffectStoreIo<'lock>, LearningStoreError> {
    io.retain_subdirectory(Path::new(LEARNING_OBJECTS_IO_RELATIVE_PATH))
        .map_err(|source| {
            retained_io_error(io, Path::new(LEARNING_OBJECTS_IO_RELATIVE_PATH), source)
        })
}

fn retain_generations_io<'lock>(
    io: &RetainedEffectStoreIo<'lock>,
) -> Result<RetainedEffectStoreIo<'lock>, LearningStoreError> {
    io.retain_subdirectory(Path::new(LEARNING_GENERATIONS_IO_RELATIVE_PATH))
        .map_err(|source| {
            retained_io_error(io, Path::new(LEARNING_GENERATIONS_IO_RELATIVE_PATH), source)
        })
}

fn retain_pristine_learning_store<'lock>(
    io: &RetainedEffectStoreIo<'lock>,
    generations_io: &RetainedEffectStoreIo<'lock>,
    pointer_session: RetainedCrashReplaceSession<'lock>,
) -> Result<PristineLearningStore<'lock>, LearningStoreError> {
    if pointer_session.raw_bytes().is_some() {
        return Err(LearningStoreError::CorruptGeneration(
            "pristine learning authority cannot contain a generation pointer".to_owned(),
        ));
    }
    reject_non_pristine_learning_root_residue(io)?;
    let tree_anchor =
        retain_learning_tree_anchor(io, generations_io, &pristine_learning_tree_digest()?)?;
    Ok(PristineLearningStore {
        pointer_session,
        tree_anchor,
    })
}

fn reject_non_pristine_learning_root_residue(
    io: &RetainedEffectStoreIo<'_>,
) -> Result<(), LearningStoreError> {
    io.validate()
        .map_err(|source| retained_io_error(io, Path::new(""), source))?;
    let entries = fs::read_dir(io.display_path()).map_err(|source| LearningStoreError::Io {
        path: io.display_path().to_path_buf(),
        source: source.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| LearningStoreError::Io {
            path: io.display_path().to_path_buf(),
            source: source.to_string(),
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(LearningStoreError::CorruptGeneration(
                "generation pointer is absent but non-UTF-8 learning residue remains".to_owned(),
            ));
        };
        let retained_cleanup_debt =
            name.starts_with(".forge-retained-") && name.ends_with(".quarantine");
        let retained_marker_debt =
            name.starts_with(".forge-crash-recovery-marker-") && name.ends_with(".quarantine");
        if matches!(
            name,
            "capture.lock"
                | LEARNING_GENERATIONS_IO_RELATIVE_PATH
                | LEARNING_GENERATION_POINTER_IO_RELATIVE_PATH
        ) || retained_cleanup_debt
            || retained_marker_debt
        {
            continue;
        }
        return Err(LearningStoreError::CorruptGeneration(
            "generation pointer is absent but generations, index, or objects residue remains"
                .to_owned(),
        ));
    }
    io.validate()
        .map_err(|source| retained_io_error(io, Path::new(""), source))
}

fn retain_learning_tree_anchor(
    io: &RetainedEffectStoreIo<'_>,
    generations_io: &RetainedEffectStoreIo<'_>,
    expected_digest: &str,
) -> Result<RetainedProjectTree, LearningStoreError> {
    io.validate()
        .map_err(|source| retained_io_error(io, Path::new(""), source))?;
    generations_io
        .validate()
        .map_err(|source| retained_io_error(generations_io, Path::new(""), source))?;
    let tree_anchor = RetainedProjectTree::capture(
        generations_io.display_path(),
        MAX_PRISTINE_TREE_ENTRIES,
        MAX_PRISTINE_TREE_BYTES,
    )
    .map_err(|error| {
        LearningStoreError::CorruptGeneration(format!(
            "generation pointer is absent but exact pristine learning authority could not be retained: {error}"
        ))
    })?;
    revalidate_learning_tree_anchor(&tree_anchor, expected_digest, io, generations_io)?;
    Ok(tree_anchor)
}

fn revalidate_learning_tree_anchor(
    tree_anchor: &RetainedProjectTree,
    expected_digest: &str,
    io: &RetainedEffectStoreIo<'_>,
    generations_io: &RetainedEffectStoreIo<'_>,
) -> Result<(), LearningStoreError> {
    io.validate()
        .map_err(|source| retained_io_error(io, Path::new(""), source))?;
    generations_io
        .validate()
        .map_err(|source| retained_io_error(generations_io, Path::new(""), source))?;
    if tree_anchor.snapshot_digest() != expected_digest {
        return Err(LearningStoreError::CorruptGeneration(
            "generation pointer is absent but generations, index, or objects residue remains"
                .to_owned(),
        ));
    }
    tree_anchor.revalidate().map_err(|error| {
        LearningStoreError::CorruptGeneration(format!(
            "Store-minted pristine learning authority changed before bootstrap: {error}"
        ))
    })?;
    generations_io
        .validate()
        .map_err(|source| retained_io_error(generations_io, Path::new(""), source))?;
    io.validate()
        .map_err(|source| retained_io_error(io, Path::new(""), source))?;
    tree_anchor.revalidate().map_err(|error| {
        LearningStoreError::CorruptGeneration(format!(
            "Store-minted pristine learning authority changed during validation: {error}"
        ))
    })
}

fn pristine_learning_tree_digest() -> Result<String, LearningStoreError> {
    learning_tree_digest(Vec::new())
}

fn prepared_learning_tree_digest(generation_sha256: &str) -> Result<String, LearningStoreError> {
    if !is_sha256_hex(generation_sha256) {
        return Err(LearningStoreError::CorruptGeneration(
            "prepared pristine generation digest is invalid".to_owned(),
        ));
    }
    learning_tree_digest(vec![(
        generation_sha256.to_owned(),
        format!("sha256:{generation_sha256}"),
    )])
}

fn learning_tree_digest(mut entries: Vec<(String, String)>) -> Result<String, LearningStoreError> {
    entries.sort();
    let raw = serde_json_canonicalizer::to_vec(&entries)
        .map_err(|error| LearningStoreError::CorruptGeneration(error.to_string()))?;
    Ok(format!("sha256:{}", sha256_hex(&raw)))
}

fn read_generation_witness<'lock>(
    io: &RetainedEffectStoreIo<'lock>,
    relative_path: &Path,
) -> Result<Option<RetainedEffectStoreLeafWitness<'lock>>, LearningStoreError> {
    match io.read_optional_bounded(relative_path, MAX_GENERATION_BYTES) {
        Ok(witness) => Ok(witness),
        Err(source) if source.kind() == io::ErrorKind::FileTooLarge => {
            Err(LearningStoreError::CorruptGeneration(format!(
                "generation exceeds byte limit {MAX_GENERATION_BYTES}"
            )))
        }
        Err(source) => Err(retained_io_error(io, relative_path, source)),
    }
}

fn read_object_witness<'lock>(
    io: &RetainedEffectStoreIo<'lock>,
    relative_path: &Path,
) -> Result<Option<RetainedEffectStoreLeafWitness<'lock>>, LearningStoreError> {
    match io.read_optional_bounded(relative_path, MAX_CANDIDATE_BYTES) {
        Ok(witness) => Ok(witness),
        Err(source) if source.kind() == io::ErrorKind::FileTooLarge => {
            Err(LearningStoreError::CandidateSize {
                found: MAX_CANDIDATE_BYTES.saturating_add(1),
                maximum: MAX_CANDIDATE_BYTES,
            })
        }
        Err(source) => Err(retained_io_error(io, relative_path, source)),
    }
}

fn revalidate_reconciled_generation(
    authority: &mut ReconciledLearningGeneration<'_>,
    io: &RetainedEffectStoreIo<'_>,
    generations_io: &RetainedEffectStoreIo<'_>,
) -> Result<(), LearningStoreError> {
    validate_generation_pointer(&authority.value)?;
    let pointer_raw = canonical_pointer_bytes(&authority.value)?;
    if authority.pointer_session.raw_bytes() != Some(pointer_raw.as_slice()) {
        return Err(LearningStoreError::CorruptGeneration(
            "reconciled generation pointer differs from its canonical record".to_owned(),
        ));
    }
    let generation_leaf = generation_leaf_relative(
        &authority.value.generation_relative_path,
        &authority.value.generation_sha256,
    )?;
    let generation_raw = canonical_generation_bytes(&authority.value.generation)?;
    if authority.generation_witness.raw_bytes() != generation_raw.as_slice()
        || sha256_hex(authority.generation_witness.raw_bytes()) != authority.value.generation_sha256
    {
        return Err(LearningStoreError::CorruptGeneration(
            "retained generation witness differs from the reconciled pointer record".to_owned(),
        ));
    }
    authority
        .generation_witness
        .revalidate()
        .map_err(|source| retained_io_error(generations_io, generation_leaf, source))?;
    io.validate()
        .map_err(|source| retained_io_error(io, Path::new(""), source))?;
    generations_io
        .validate()
        .map_err(|source| retained_io_error(generations_io, Path::new(""), source))
}

fn retain_generation_for_read<'lock>(
    mut authority: ReconciledLearningGeneration<'lock>,
    io: &RetainedEffectStoreIo<'lock>,
    generations_io: &RetainedEffectStoreIo<'lock>,
) -> Result<RetainedLearningGeneration<'lock>, LearningStoreError> {
    revalidate_reconciled_generation(&mut authority, io, generations_io)?;
    let ReconciledLearningGeneration {
        value,
        pointer_session,
        generation_witness,
    } = authority;
    let pointer_anchor = pointer_session
        .read_exact()
        .map_err(map_persistence_error)?
        .ok_or_else(|| {
            LearningStoreError::CorruptGeneration(
                "reconciled generation pointer disappeared before exact read retention".to_owned(),
            )
        })?;
    let mut retained = RetainedLearningGeneration {
        value,
        pointer_anchor,
        generation_witness,
    };
    revalidate_generation_authority(&mut retained, io, generations_io)?;
    Ok(retained)
}

fn revalidate_generation_authority(
    authority: &mut RetainedLearningGeneration<'_>,
    io: &RetainedEffectStoreIo<'_>,
    generations_io: &RetainedEffectStoreIo<'_>,
) -> Result<(), LearningStoreError> {
    validate_generation_pointer(&authority.value)?;
    if authority.pointer_anchor.raw_bytes() != canonical_pointer_bytes(&authority.value)?.as_slice()
    {
        return Err(LearningStoreError::CorruptGeneration(
            "retained generation pointer differs from its canonical record".to_owned(),
        ));
    }
    let generation_leaf = generation_leaf_relative(
        &authority.value.generation_relative_path,
        &authority.value.generation_sha256,
    )?;
    let generation_raw = canonical_generation_bytes(&authority.value.generation)?;
    if authority.generation_witness.raw_bytes() != generation_raw.as_slice()
        || sha256_hex(authority.generation_witness.raw_bytes()) != authority.value.generation_sha256
    {
        return Err(LearningStoreError::CorruptGeneration(
            "retained generation witness differs from the canonical pointer record".to_owned(),
        ));
    }
    authority.pointer_anchor.revalidate().map_err(|source| {
        retained_io_error(
            io,
            Path::new(LEARNING_GENERATION_POINTER_IO_RELATIVE_PATH),
            source,
        )
    })?;
    authority
        .generation_witness
        .revalidate()
        .map_err(|source| retained_io_error(generations_io, generation_leaf, source))?;
    // The pointer is the sole decision authority. Rebinding it last makes this
    // closing validation the read-side linearization point while the matching
    // immutable generation handle remains retained.
    authority.pointer_anchor.revalidate().map_err(|source| {
        retained_io_error(
            io,
            Path::new(LEARNING_GENERATION_POINTER_IO_RELATIVE_PATH),
            source,
        )
    })?;
    Ok(())
}

fn generation_leaf_relative<'path>(
    relative_path: &'path str,
    expected_digest: &str,
) -> Result<&'path Path, LearningStoreError> {
    let path = Path::new(relative_path);
    let generation_root = Path::new(LEARNING_GENERATIONS_RELATIVE_PATH);
    let relative =
        path.strip_prefix(generation_root)
            .map_err(|_| LearningStoreError::InvalidStorePath {
                path: path.to_path_buf(),
                reason: "learning generation path is outside the retained generations directory"
                    .to_owned(),
            })?;
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().count() != 1
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || relative.to_str() != Some(expected_digest)
    {
        return Err(LearningStoreError::InvalidStorePath {
            path: path.to_path_buf(),
            reason: "learning generation path is not the exact selected digest leaf".to_owned(),
        });
    }
    Ok(relative)
}

fn canonical_index_bytes(index: &LearningIndex) -> Result<Vec<u8>, LearningStoreError> {
    serde_json_canonicalizer::to_vec(index)
        .map_err(|error| LearningStoreError::CorruptIndex(error.to_string()))
}

fn canonical_generation_bytes(
    generation: &LearningGeneration,
) -> Result<Vec<u8>, LearningStoreError> {
    serde_json_canonicalizer::to_vec(generation)
        .map_err(|error| LearningStoreError::CorruptGeneration(error.to_string()))
}

fn canonical_pointer_bytes(
    pointer: &LearningGenerationPointer,
) -> Result<Vec<u8>, LearningStoreError> {
    serde_json_canonicalizer::to_vec(pointer)
        .map_err(|error| LearningStoreError::CorruptGeneration(error.to_string()))
}

#[allow(clippy::needless_pass_by_value)]
fn retained_io_error(
    io: &RetainedEffectStoreIo<'_>,
    relative_path: &Path,
    source: io::Error,
) -> LearningStoreError {
    LearningStoreError::Io {
        path: io.display_path().join(relative_path),
        source: source.to_string(),
    }
}

fn mint_store_authority_binding(
    io: &RetainedEffectStoreIo<'_>,
) -> Result<String, LearningStoreError> {
    let nonce = io
        .mint_operation_nonce()
        .map_err(|source| retained_io_error(io, Path::new(""), source))?;
    Ok(format!("sha256:{}", sha256_hex(nonce.as_bytes())))
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

fn is_sha256_content_hash(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(is_sha256_hex)
}

fn is_operation_nonce(value: &str) -> bool {
    is_sha256_hex(value)
}

#[allow(clippy::needless_pass_by_value)]
fn map_lock_error(error: EffectStoreLockError) -> LearningStoreError {
    LearningStoreError::Lock(format!("{error:?}"))
}

#[allow(clippy::needless_pass_by_value)]
fn map_persistence_error(error: CrashReplaceError) -> LearningStoreError {
    LearningStoreError::Persistence(error.to_string())
}
