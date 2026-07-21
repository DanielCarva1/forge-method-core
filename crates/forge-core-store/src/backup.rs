#![allow(clippy::case_sensitive_file_extension_comparisons)]

//! Durable project-state backup archive and protected receipt I/O.
//!
//! This module owns the binary container, immutable publication protocol, strict
//! reader, protected receipt store, and opaque verified capability. Manifest
//! integrity is necessary but never sufficient: verification succeeds only
//! after an independently loaded protected receipt is bound to the archive.

use crate::producer_quiescence::HostQuiescenceGuard;
use crate::replay_anchor::{
    snapshot_replay_anchor_under_retained_lock, verify_replay_anchor_under_retained_lock,
    ReplayAnchorRetainedLock, ReplayAnchorStatus, ReplayAnchorVerification, ReplayWalHead,
};
use forge_core_authority::{AuthorizedWorkflowBrokerRegistry, WorkflowBrokerRegistryDocument};
use forge_core_contracts::{
    canonical_archive_path, decode_canonical_archive_path, BackupArchiveEntryType,
    BackupArchiveLayout, BackupArchiveVerificationError, BackupClaimStoreState,
    BackupDeclaredEffectOutput, BackupDomainPackGeneration, BackupDomainPackLearningAuthority,
    BackupDomainPackLearningRecord, BackupDomainPackLearningStoreState,
    BackupDomainPackOperatorSourcesProjection, BackupDomainPackStoreState,
    BackupDomainPackSupplyChainAuthority, BackupEffectStoreState, BackupEffectiveEpochBinding,
    BackupEntry, BackupEntryKind, BackupExternalAuthorityObservations,
    BackupForbiddenPrivateMaterial, BackupInitializationState, BackupIsolationContractProjection,
    BackupIsolationStoreState, BackupManifest, BackupManifestDocument, BackupManifestFormat,
    BackupProjectBinding, BackupProjectState, BackupProvisioningState,
    BackupPublicRegistryMaterial, BackupPublicSidecarCounts, BackupReceipt, BackupReceiptDocument,
    BackupReceiptValidationError, BackupReplayRollbackAnchor, BackupSnapshotMode,
    BackupSnapshotProtocol, BackupSourceFileMetadata, BackupSourceState,
    BackupUnlockedProducerBoundary, ProjectLinkDocument, WorkflowEffectiveBundleIdentity,
    WorkflowGovernanceLedgerRecord, WorkflowGovernanceReleaseIdentity, BACKUP_LOCK_ORDER,
    BACKUP_MANIFEST_SCHEMA_VERSION, BACKUP_RECEIPT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

const ARCHIVE_MAGIC: &[u8; 16] = b"FORGE-BACKUP-V1\0";
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MEMBER_COUNT: u64 = 100_000;
const MAX_MEMBER_NAME_BYTES: u64 = 16 * 1024;
const MAX_MEMBER_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const BACKUP_AUTHORITY_CATALOG_SCHEMA_VERSION: &str = "forge_backup_authority_catalog_v1";
const MAX_AUTHORITY_CATALOG_BYTES: u64 = 1024 * 1024;
const MAX_RECEIPT_BYTES: u64 = 1024 * 1024;

/// One source file captured while producer locks and host quiescence are held.
/// Construction remains crate-private so serialized caller claims cannot become
/// archive members.
#[derive(Debug)]
pub(crate) struct CapturedBackupMember {
    pub(crate) entry: BackupEntry,
    pub(crate) bytes: Vec<u8>,
}

/// One no-follow source observation. Excluded private/lock/debris files retain
/// metadata only; their bytes are never read or copied.
#[derive(Debug)]
pub struct CapturedSourceFile {
    metadata: BackupSourceFileMetadata,
    bytes: Option<Vec<u8>>,
}

impl CapturedSourceFile {
    #[must_use]
    pub const fn metadata(&self) -> &BackupSourceFileMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
}

/// Stable complete source enumeration captured between two identical no-follow
/// metadata walks while the caller retains every producer guard.
#[derive(Debug)]
pub struct BackupSourceCapture {
    files: Vec<CapturedSourceFile>,
}

impl BackupSourceCapture {
    #[must_use]
    pub fn files(&self) -> &[CapturedSourceFile] {
        &self.files
    }
}

/// Complete source-derived snapshot retained only inside the store engine.
#[derive(Debug)]
pub(crate) struct CapturedBackupSnapshot {
    pub(crate) manifest: BackupManifestDocument,
    pub(crate) members: Vec<CapturedBackupMember>,
}

/// Opaque result of exact archive, raw Project Link, receipt, and protected
/// authority verification. It is deliberately non-Clone and non-serde.
pub struct VerifiedBackupArchive {
    manifest: BackupManifestDocument,
    receipt: BackupReceiptDocument,
    archive_sha256: String,
    archive_path: PathBuf,
    member_count: usize,
}

impl fmt::Debug for VerifiedBackupArchive {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedBackupArchive")
            .field("archive_path", &self.archive_path)
            .field("archive_sha256", &self.archive_sha256)
            .field("member_count", &self.member_count)
            .finish_non_exhaustive()
    }
}

impl VerifiedBackupArchive {
    #[must_use]
    pub const fn manifest(&self) -> &BackupManifestDocument {
        &self.manifest
    }

    #[must_use]
    pub const fn receipt(&self) -> &BackupReceiptDocument {
        &self.receipt
    }

    #[must_use]
    pub fn archive_sha256(&self) -> &str {
        &self.archive_sha256
    }

    #[must_use]
    pub fn archive_path(&self) -> &Path {
        &self.archive_path
    }

    #[must_use]
    pub const fn member_count(&self) -> usize {
        self.member_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupPublication {
    pub archive_path: PathBuf,
    pub archive_sha256: String,
    pub receipt_path: PathBuf,
    pub receipt_digest: String,
    pub manifest_set_digest: String,
    pub member_count: usize,
    pub already_published: bool,
}

/// Kernel-derived workflow identity observed immediately before host quiescence.
/// These values are not success proof: the assembler binds the claimed ledger
/// head to the exact captured governance WAL before it can publish a receipt.
#[derive(Debug, Clone)]
pub struct BackupGovernanceProjection {
    pub workflow_release: WorkflowGovernanceReleaseIdentity,
    pub effective_bundle: WorkflowEffectiveBundleIdentity,
    pub state_version: u64,
    pub governance_ledger_head_digest: String,
}

/// Redundant exact-member observation produced by a closed producer adapter.
/// Public fields do not grant authority; the Store compares every observation
/// with its independently captured source bytes under host quiescence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupExpectedMember {
    pub logical_path: String,
    pub sha256: String,
}

/// Inputs for one protected backup creation transaction. Trust locations are
/// selected by `authority_id`; callers cannot provide receipt or replay-anchor
/// paths. The exact Project Link and sidecar roots are resolved from the project.
#[derive(Debug, Clone)]
pub struct BackupCreateRequest {
    pub project_root: PathBuf,
    pub archive_path: PathBuf,
    pub authority_id: String,
    pub governance: BackupGovernanceProjection,
    pub current_principal_registry: Option<PathBuf>,
    pub current_broker_registry: Option<PathBuf>,
    pub expected_members: Vec<BackupExpectedMember>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupDestinationPlatform {
    Posix,
    Windows,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum BackupError {
    InvalidPath {
        path: PathBuf,
        reason: String,
    },
    UnsafeFileType {
        path: PathBuf,
        reason: String,
    },
    ResourceLimit {
        resource: &'static str,
        maximum: u64,
    },
    Manifest {
        reason: String,
    },
    Archive {
        reason: String,
    },
    Receipt {
        reason: String,
    },
    ExistingDifferent {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
}

/// Public verification inputs contain only an operator-configured authority
/// selector. Receipt and protected-anchor paths are deliberately absent, so a
/// caller cannot turn an arbitrary self-digested receipt into trusted proof.
#[derive(Debug, Clone)]
pub struct BackupVerifyRequest {
    pub project_root: PathBuf,
    pub archive_path: PathBuf,
    pub authority_id: String,
    pub current_principal_registry: Option<PathBuf>,
    pub current_broker_registry: Option<PathBuf>,
}

/// Opaque capability resolved from the machine-owned backup authority catalog.
/// It contains public filesystem locations and identities, never secret material.
pub struct TrustedBackupAuthority {
    authority_id: String,
    receipt_store: PathBuf,
    replay_anchor_path: PathBuf,
    protected_anchor_identity: String,
    domain_pack_operator: Option<ConfiguredDomainPackOperator>,
}

impl fmt::Debug for TrustedBackupAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TrustedBackupAuthority")
            .field("authority_id", &self.authority_id)
            .finish_non_exhaustive()
    }
}

impl TrustedBackupAuthority {
    pub(crate) fn authority_id(&self) -> &str {
        &self.authority_id
    }

    pub(crate) fn receipt_store(&self) -> &Path {
        &self.receipt_store
    }

    pub(crate) fn replay_anchor_path(&self) -> &Path {
        &self.replay_anchor_path
    }

    pub(crate) fn protected_anchor_identity(&self) -> &str {
        &self.protected_anchor_identity
    }
}

#[derive(Debug)]
struct ConfiguredDomainPackOperator {
    root: PathBuf,
    root_identity: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupAuthorityCatalog {
    schema_version: String,
    authorities: Vec<BackupAuthorityConfiguration>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupAuthorityConfiguration {
    authority_id: String,
    receipt_store: PathBuf,
    replay_anchor_path: PathBuf,
    protected_anchor_identity: String,
    domain_pack_operator_root: Option<PathBuf>,
    domain_pack_operator_root_identity: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackRegistryAnchorHead {
    schema_version: String,
    registry_id: forge_core_contracts::StableId,
    audience: forge_core_contracts::StableId,
    generation: u64,
    snapshot_digest: String,
    trust_policy_digest: String,
    #[serde(default, rename = "cumulative_revocations")]
    _cumulative_revocations: Vec<forge_core_contracts::DomainPackPackageRevocation>,
    #[serde(default, rename = "cumulative_revocation_digest")]
    _cumulative_revocation_digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewerHead {
    registry_id: forge_core_contracts::StableId,
    audience: String,
    generation: u64,
    registry_digest: String,
    full_digest: String,
    trust_policy_digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedHead {
    registry_id: forge_core_contracts::StableId,
    audience: String,
    generation: u64,
    registry_digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LearningAnchorHead {
    schema_version: String,
    reviewer: ReviewerHead,
    reviewed: ReviewedHead,
}

impl fmt::Display for BackupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath { path, reason } => {
                write!(
                    formatter,
                    "invalid backup path {}: {reason}",
                    path.display()
                )
            }
            Self::UnsafeFileType { path, reason } => {
                write!(formatter, "unsafe backup file {}: {reason}", path.display())
            }
            Self::ResourceLimit { resource, maximum } => {
                write!(formatter, "backup {resource} exceeds limit {maximum}")
            }
            Self::Manifest { reason } => write!(formatter, "backup manifest invalid: {reason}"),
            Self::Archive { reason } => write!(formatter, "backup archive invalid: {reason}"),
            Self::Receipt { reason } => write!(formatter, "backup receipt invalid: {reason}"),
            Self::ExistingDifferent { path } => write!(
                formatter,
                "refusing to overwrite different existing backup material {}",
                path.display()
            ),
            Self::Io { path, source } => {
                write!(formatter, "backup I/O {} failed: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for BackupError {}

/// Capture the exact Project Link plus complete sidecar tree twice under one
/// exact-root typed host-quiescence capability. The effect lock is not a caller
/// assertion: acquiring it through the sealed boundary validates that the guard
/// protects the resolved `.forge-method` root and retains that authority until
/// both enumerations and their comparison complete.
pub fn capture_stable_source(
    project_root: impl AsRef<Path>,
    sidecar_root: impl AsRef<Path>,
    quiescence: &HostQuiescenceGuard,
    layout: &BackupArchiveLayout,
) -> Result<BackupSourceCapture, BackupError> {
    let project_root = fs::canonicalize(project_root.as_ref())
        .map_err(|source| io_error(project_root.as_ref(), source))?;
    let sidecar_root = fs::canonicalize(sidecar_root.as_ref())
        .map_err(|source| io_error(sidecar_root.as_ref(), source))?;
    if project_root == sidecar_root {
        return Err(BackupError::InvalidPath {
            path: project_root,
            reason: "project and sidecar must be distinct existing directories".to_owned(),
        });
    }
    let state_root = fs::canonicalize(sidecar_root.join(&layout.state_root_relative_to_sidecar))
        .map_err(|source| io_error(&sidecar_root, source))?;
    if state_root.parent() != Some(sidecar_root.as_path())
        || state_root.file_name().and_then(|value| value.to_str()) != Some(".forge-method")
    {
        return Err(BackupError::InvalidPath {
            path: state_root,
            reason: "resolved state root is not the direct .forge-method child of the sidecar"
                .to_owned(),
        });
    }
    let mut source_roots = RetainedBackupSourceRoots::open(
        &project_root,
        &sidecar_root,
        &state_root,
        Path::new(&layout.state_root_relative_to_sidecar),
    )?;
    source_roots.validate_object_bindings()?;
    let claim_lock =
        crate::claim_wal::acquire_claim_wal_retained_lock_under_boundary(quiescence, &state_root)
            .map_err(|source| BackupError::Archive {
            reason: format!("cannot retain claim WAL for backup: {source}"),
        })?;
    let claim_recovery =
        crate::claim_wal::recover_claim_wal_under_retained_lock(&state_root, &claim_lock, false)
            .map_err(|source| BackupError::Archive {
                reason: format!("claim WAL is partial or invalid: {source}"),
            })?;
    if claim_recovery.stop_reason != crate::claim_wal::ClaimWalStopReason::CleanEof {
        return Err(BackupError::Archive {
            reason: format!(
                "claim WAL stopped before clean EOF: {:?}",
                claim_recovery.stop_reason
            ),
        });
    }
    let action_lock =
        crate::workflow_action_replay::acquire_workflow_action_replay_retained_lock_under_boundary(
            quiescence,
            &state_root,
        )
        .map_err(|source| BackupError::Archive {
            reason: format!("cannot retain workflow-action replay WAL for backup: {source}"),
        })?;
    let _action_recovery =
        crate::workflow_action_replay::recover_workflow_action_replay_under_retained_lock(
            &state_root,
            &action_lock,
        )
        .map_err(|source| BackupError::Archive {
            reason: format!("workflow-action replay WAL is partial or invalid: {source}"),
        })?;
    let boundary_lock = crate::acquire_effect_store_lock_under_boundary(
        quiescence,
        &sidecar_root,
        ".forge-method/locks/effects.lock",
    )
    .map_err(|source| BackupError::Archive {
        reason: format!("host quiescence does not protect the resolved state root: {source:?}"),
    })?;
    let effect_recovery = crate::recover_effect_wal_under_lock(
        &sidecar_root,
        &boundary_lock,
        ".forge-method/locks/effects.lock",
        ".forge-method/wal/effects.ndjson",
    );
    if effect_recovery.status == crate::EffectWalRecoveryStatus::RecoveryFailed {
        return Err(BackupError::Archive {
            reason: format!(
                "effect WAL is partial or invalid: {}",
                effect_recovery.diagnostics.join("; ")
            ),
        });
    }
    let replay_lock =
        crate::replay_wal::acquire_replay_wal_retained_lock_under_boundary(quiescence, &state_root)
            .map_err(|source| BackupError::Archive {
                reason: format!("cannot retain replay WAL for backup: {source}"),
            })?;
    let replay_recovery =
        crate::replay_wal::recover_replay_wal_under_retained_lock(&state_root, &replay_lock, false)
            .map_err(|source| BackupError::Archive {
                reason: format!("replay WAL is partial or invalid: {source}"),
            })?;
    if !replay_recovery.is_clean() {
        return Err(BackupError::Archive {
            reason: format!(
                "replay WAL stopped before clean EOF: {:?}",
                replay_recovery.stop_reason
            ),
        });
    }
    // Recovery may durably repair files beneath the state directory retained as
    // a sidecar member. Accept only that member's resulting metadata, and only
    // after proving every retained root still names the same no-follow object.
    source_roots.rebaseline_sidecar_member_after_recovery()?;
    let first = capture_source_walk_retained(&source_roots, layout)?;
    source_roots.validate_stable_bindings()?;
    boundary_lock
        .validate_retained_lock_file()
        .map_err(|source| BackupError::Archive {
            reason: format!("effect lock changed during backup capture: {source}"),
        })?;
    let second = capture_source_walk_retained(&source_roots, layout)?;
    source_roots.validate_stable_bindings()?;
    boundary_lock
        .validate_retained_lock_file()
        .map_err(|source| BackupError::Archive {
            reason: format!("effect lock changed during backup reconciliation: {source}"),
        })?;
    if first.namespace != second.namespace
        || first.files.len() != second.files.len()
        || first.files.iter().zip(&second.files).any(|(left, right)| {
            left.metadata != right.metadata || left.bytes.as_deref() != right.bytes.as_deref()
        })
    {
        return Err(BackupError::Archive {
            reason: "source changed between copy and stable re-enumeration".to_owned(),
        });
    }
    Ok(BackupSourceCapture { files: first.files })
}

#[derive(Debug)]
struct RetainedBackupSourceRoots {
    project: RetainedSourceDirectory,
    sidecar: RetainedSourceDirectory,
    state: RetainedSourceDirectory,
    state_path: PathBuf,
    state_relative_to_sidecar: PathBuf,
}

impl RetainedBackupSourceRoots {
    fn open(
        project_path: &Path,
        sidecar_path: &Path,
        state_path: &Path,
        state_relative_to_sidecar: &Path,
    ) -> Result<Self, BackupError> {
        let project = RetainedSourceDirectory::open_root(project_path)?;
        let sidecar = RetainedSourceDirectory::open_root(sidecar_path)?;
        let state = sidecar.open_relative_directory(state_relative_to_sidecar)?;
        let state_from_path = RetainedSourceDirectory::open_root(state_path)?;
        if !same_file_object(&state.opened_metadata, &state_from_path.opened_metadata) {
            return Err(BackupError::Archive {
                reason: "retained sidecar state directory differs from the resolved state root"
                    .to_owned(),
            });
        }
        Ok(Self {
            project,
            sidecar,
            state,
            state_path: state_path.to_path_buf(),
            state_relative_to_sidecar: state_relative_to_sidecar.to_path_buf(),
        })
    }

    fn validate_object_bindings(&self) -> Result<(), BackupError> {
        self.project.validate_namespace_object()?;
        self.sidecar.validate_namespace_object()?;
        self.state.validate_retained_object()?;
        let state_from_sidecar = self
            .sidecar
            .open_relative_directory(&self.state_relative_to_sidecar)?;
        let state_from_path = RetainedSourceDirectory::open_root(&self.state_path)?;
        if !same_file_object(
            &self.state.opened_metadata,
            &state_from_sidecar.opened_metadata,
        ) || !same_file_object(
            &self.state.opened_metadata,
            &state_from_path.opened_metadata,
        ) {
            return Err(BackupError::Archive {
                reason: "retained backup state-root binding was substituted".to_owned(),
            });
        }
        Ok(())
    }

    fn rebaseline_sidecar_member_after_recovery(&mut self) -> Result<(), BackupError> {
        self.validate_object_bindings()?;
        self.state.rebaseline()?;
        self.validate_object_bindings()
    }

    fn validate_stable_bindings(&self) -> Result<(), BackupError> {
        self.project.validate_namespace_stable()?;
        self.sidecar.validate_namespace_stable()?;
        self.state.validate_retained_stable()?;
        let state_from_sidecar = self
            .sidecar
            .open_relative_directory(&self.state_relative_to_sidecar)?;
        let state_from_path = RetainedSourceDirectory::open_root(&self.state_path)?;
        if !same_file_identity(
            &self.state.opened_metadata,
            &state_from_sidecar.opened_metadata,
        ) || !same_file_identity(
            &self.state.opened_metadata,
            &state_from_path.opened_metadata,
        ) {
            return Err(BackupError::Archive {
                reason: "retained backup state-root metadata or namespace changed".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
struct RetainedSourceDirectory {
    handle: File,
    display_path: PathBuf,
    opened_metadata: fs::Metadata,
}

#[derive(Debug)]
struct RetainedSourceFile {
    handle: File,
    display_path: PathBuf,
    opened_metadata: fs::Metadata,
}

#[derive(Debug)]
enum RetainedSourceChild {
    Directory(RetainedSourceDirectory),
    File(RetainedSourceFile),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceDirectoryEntry {
    name: OsString,
    object_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceNamespaceEntryType {
    Directory,
    RegularFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedSourceNamespaceEntry {
    logical_path: String,
    object_id: u64,
    entry_type: SourceNamespaceEntryType,
}

#[derive(Debug)]
struct CapturedSourceWalk {
    files: Vec<CapturedSourceFile>,
    namespace: Vec<CapturedSourceNamespaceEntry>,
}

impl RetainedSourceChild {
    fn opened_metadata(&self) -> &fs::Metadata {
        match self {
            Self::Directory(directory) => &directory.opened_metadata,
            Self::File(file) => &file.opened_metadata,
        }
    }
}

impl RetainedSourceDirectory {
    fn open_root(path: &Path) -> Result<Self, BackupError> {
        if !path.is_absolute() {
            return Err(BackupError::InvalidPath {
                path: path.to_path_buf(),
                reason: "retained source root must be absolute".to_owned(),
            });
        }
        let mut anchor = PathBuf::new();
        let mut components = Vec::new();
        let mut rooted = false;
        for component in path.components() {
            match component {
                Component::Prefix(prefix) => {
                    if !anchor.as_os_str().is_empty() {
                        return Err(BackupError::InvalidPath {
                            path: path.to_path_buf(),
                            reason: "source path has multiple platform prefixes".to_owned(),
                        });
                    }
                    anchor.push(prefix.as_os_str());
                }
                Component::RootDir => {
                    anchor.push(Path::new(std::path::MAIN_SEPARATOR_STR));
                    rooted = true;
                }
                Component::Normal(value) => components.push(value.to_os_string()),
                Component::CurDir | Component::ParentDir => {
                    return Err(BackupError::InvalidPath {
                        path: path.to_path_buf(),
                        reason: "source path is not lexically normalized".to_owned(),
                    });
                }
            }
        }
        if !rooted {
            return Err(BackupError::InvalidPath {
                path: path.to_path_buf(),
                reason: "source path has no filesystem root".to_owned(),
            });
        }
        let handle = source_platform::open_root_directory(&anchor)
            .map_err(|source| io_error(&anchor, source))?;
        let mut directory = Self::from_handle(handle, anchor)?;
        for component in components {
            directory = directory.open_directory_component(&component)?;
        }
        directory.display_path = path.to_path_buf();
        Ok(directory)
    }

    fn from_handle(handle: File, display_path: PathBuf) -> Result<Self, BackupError> {
        let metadata = handle
            .metadata()
            .map_err(|source| io_error(&display_path, source))?;
        validate_source_directory_metadata(&metadata, &display_path)?;
        Ok(Self {
            handle,
            display_path,
            opened_metadata: metadata,
        })
    }

    fn open_child_component(&self, name: &OsStr) -> Result<RetainedSourceChild, BackupError> {
        validate_source_component(name, &self.display_path)?;
        let display_path = self.display_path.join(name);
        let handle = source_platform::open_child(&self.handle, name)
            .map_err(|source| io_error(&display_path, source))?;
        let metadata = handle
            .metadata()
            .map_err(|source| io_error(&display_path, source))?;
        if metadata_is_reparse(&metadata) || metadata.file_type().is_symlink() {
            return Err(BackupError::UnsafeFileType {
                path: display_path,
                reason: "source links and reparse points are forbidden".to_owned(),
            });
        }
        if metadata.is_dir() {
            return Ok(RetainedSourceChild::Directory(Self {
                handle,
                display_path,
                opened_metadata: metadata,
            }));
        }
        if metadata.is_file() && hard_link_count(&metadata) == 1 {
            return Ok(RetainedSourceChild::File(RetainedSourceFile {
                handle,
                display_path,
                opened_metadata: metadata,
            }));
        }
        Err(BackupError::UnsafeFileType {
            path: display_path,
            reason: "source must be a directory or one single-link regular file".to_owned(),
        })
    }

    fn open_directory_component(&self, name: &OsStr) -> Result<Self, BackupError> {
        match self.open_child_component(name)? {
            RetainedSourceChild::Directory(directory) => Ok(directory),
            RetainedSourceChild::File(file) => Err(BackupError::UnsafeFileType {
                path: file.display_path,
                reason: "source ancestor must be a no-follow directory".to_owned(),
            }),
        }
    }

    fn open_file_component(&self, name: &OsStr) -> Result<RetainedSourceFile, BackupError> {
        match self.open_child_component(name)? {
            RetainedSourceChild::File(file) => Ok(file),
            RetainedSourceChild::Directory(directory) => Err(BackupError::UnsafeFileType {
                path: directory.display_path,
                reason: "source leaf must be one no-follow single-link regular file".to_owned(),
            }),
        }
    }

    fn open_relative_directory(&self, path: &Path) -> Result<Self, BackupError> {
        if path.is_absolute() {
            return Err(BackupError::InvalidPath {
                path: path.to_path_buf(),
                reason: "retained source child must be relative".to_owned(),
            });
        }
        let mut directory = self.try_clone()?;
        let mut opened = false;
        for component in path.components() {
            let Component::Normal(value) = component else {
                return Err(BackupError::InvalidPath {
                    path: path.to_path_buf(),
                    reason: "retained source child is not normalized".to_owned(),
                });
            };
            directory = directory.open_directory_component(value)?;
            opened = true;
        }
        if !opened {
            return Err(BackupError::InvalidPath {
                path: path.to_path_buf(),
                reason: "retained source child is empty".to_owned(),
            });
        }
        Ok(directory)
    }

    fn try_clone(&self) -> Result<Self, BackupError> {
        let handle = self
            .handle
            .try_clone()
            .map_err(|source| io_error(&self.display_path, source))?;
        Ok(Self {
            handle,
            display_path: self.display_path.clone(),
            opened_metadata: self.opened_metadata.clone(),
        })
    }

    fn read_entries(&self) -> Result<Vec<SourceDirectoryEntry>, BackupError> {
        let mut entries = source_platform::read_entries(&self.handle)
            .map_err(|source| io_error(&self.display_path, source))?;
        for entry in &entries {
            validate_source_component(&entry.name, &self.display_path)?;
            if entry.object_id == 0 {
                return Err(BackupError::Archive {
                    reason: format!(
                        "source directory returned an entry without stable identity: {}",
                        self.display_path.join(&entry.name).display()
                    ),
                });
            }
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        if entries.windows(2).any(|pair| pair[0].name == pair[1].name) {
            return Err(BackupError::Archive {
                reason: format!(
                    "source directory returned duplicate entries: {}",
                    self.display_path.display()
                ),
            });
        }
        Ok(entries)
    }

    fn required_entry(&self, name: &OsStr) -> Result<SourceDirectoryEntry, BackupError> {
        self.read_entries()?
            .into_iter()
            .find(|entry| entry.name == name)
            .ok_or_else(|| BackupError::Io {
                path: self.display_path.join(name),
                source: io::Error::new(io::ErrorKind::NotFound, "source entry is absent"),
            })
    }

    fn validate_retained_object(&self) -> Result<(), BackupError> {
        let current = self
            .handle
            .metadata()
            .map_err(|source| io_error(&self.display_path, source))?;
        validate_source_directory_metadata(&current, &self.display_path)?;
        if !same_file_object(&self.opened_metadata, &current) {
            return Err(BackupError::Archive {
                reason: format!(
                    "retained source directory identity changed: {}",
                    self.display_path.display()
                ),
            });
        }
        Ok(())
    }

    fn validate_retained_stable(&self) -> Result<(), BackupError> {
        let current = self
            .handle
            .metadata()
            .map_err(|source| io_error(&self.display_path, source))?;
        validate_source_directory_metadata(&current, &self.display_path)?;
        if !same_file_identity(&self.opened_metadata, &current) {
            return Err(BackupError::Archive {
                reason: format!(
                    "retained source directory changed during traversal: {}",
                    self.display_path.display()
                ),
            });
        }
        Ok(())
    }

    fn validate_namespace_object(&self) -> Result<(), BackupError> {
        self.validate_retained_object()?;
        let reopened = Self::open_root(&self.display_path)?;
        if !same_file_object(&self.opened_metadata, &reopened.opened_metadata) {
            return Err(BackupError::Archive {
                reason: format!(
                    "source directory namespace was replaced: {}",
                    self.display_path.display()
                ),
            });
        }
        Ok(())
    }

    fn validate_namespace_stable(&self) -> Result<(), BackupError> {
        self.validate_retained_stable()?;
        let reopened = Self::open_root(&self.display_path)?;
        if !same_file_identity(&self.opened_metadata, &reopened.opened_metadata) {
            return Err(BackupError::Archive {
                reason: format!(
                    "source directory namespace or metadata changed: {}",
                    self.display_path.display()
                ),
            });
        }
        Ok(())
    }

    fn rebaseline(&mut self) -> Result<(), BackupError> {
        let current = self
            .handle
            .metadata()
            .map_err(|source| io_error(&self.display_path, source))?;
        validate_source_directory_metadata(&current, &self.display_path)?;
        if !same_file_object(&self.opened_metadata, &current) {
            return Err(BackupError::Archive {
                reason: format!(
                    "retained source directory was replaced during recovery: {}",
                    self.display_path.display()
                ),
            });
        }
        self.opened_metadata = current;
        Ok(())
    }
}

impl RetainedSourceFile {
    fn validate_retained_stable(&self) -> Result<(), BackupError> {
        let current = self
            .handle
            .metadata()
            .map_err(|source| io_error(&self.display_path, source))?;
        validate_source_file_metadata(&current, &self.display_path)?;
        if !same_file_identity(&self.opened_metadata, &current) {
            return Err(BackupError::Archive {
                reason: format!(
                    "retained source file changed while reading: {}",
                    self.display_path.display()
                ),
            });
        }
        Ok(())
    }

    fn validate_namespace(
        &self,
        parent: &RetainedSourceDirectory,
        name: &OsStr,
    ) -> Result<(), BackupError> {
        let reopened = parent.open_file_component(name)?;
        if !same_file_identity(&self.opened_metadata, &reopened.opened_metadata) {
            return Err(BackupError::Archive {
                reason: format!(
                    "source file namespace was replaced: {}",
                    self.display_path.display()
                ),
            });
        }
        Ok(())
    }
}

fn validate_source_component(name: &OsStr, parent: &Path) -> Result<(), BackupError> {
    let mut components = Path::new(name).components();
    let valid = matches!(components.next(), Some(Component::Normal(value)) if value == name)
        && components.next().is_none();
    if !valid {
        return Err(BackupError::InvalidPath {
            path: parent.join(name),
            reason: "source enumeration returned a non-child component".to_owned(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn directory_entry_matches(entry: &SourceDirectoryEntry, metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    entry.object_id == metadata.ino()
}

#[cfg(windows)]
fn directory_entry_matches(entry: &SourceDirectoryEntry, metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    metadata
        .file_index()
        .is_some_and(|index| entry.object_id == index)
}

#[cfg(not(any(unix, windows)))]
fn directory_entry_matches(_entry: &SourceDirectoryEntry, _metadata: &fs::Metadata) -> bool {
    false
}

fn validate_source_directory_metadata(
    metadata: &fs::Metadata,
    path: &Path,
) -> Result<(), BackupError> {
    if !metadata.is_dir() || metadata.file_type().is_symlink() || metadata_is_reparse(metadata) {
        return Err(BackupError::UnsafeFileType {
            path: path.to_path_buf(),
            reason: "source ancestor must be a no-follow non-reparse directory".to_owned(),
        });
    }
    Ok(())
}

fn validate_source_file_metadata(metadata: &fs::Metadata, path: &Path) -> Result<(), BackupError> {
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata_is_reparse(metadata)
        || hard_link_count(metadata) != 1
    {
        return Err(BackupError::UnsafeFileType {
            path: path.to_path_buf(),
            reason: "source leaf must be one no-follow single-link regular file".to_owned(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn metadata_is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

fn capture_source_walk(
    project_root: &Path,
    sidecar_root: &Path,
    layout: &BackupArchiveLayout,
) -> Result<Vec<CapturedSourceFile>, BackupError> {
    let project_root = absolute_source_path(project_root)?;
    let sidecar_root = absolute_source_path(sidecar_root)?;
    let state_root = sidecar_root.join(&layout.state_root_relative_to_sidecar);
    let roots = RetainedBackupSourceRoots::open(
        &project_root,
        &sidecar_root,
        &state_root,
        Path::new(&layout.state_root_relative_to_sidecar),
    )?;
    roots.validate_stable_bindings()?;
    let walk = capture_source_walk_retained(&roots, layout)?;
    roots.validate_stable_bindings()?;
    Ok(walk.files)
}

fn absolute_source_path(path: &Path) -> Result<PathBuf, BackupError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|source| io_error(path, source))?
            .join(path)
    };
    lexically_normalize_absolute(&absolute)
}

fn capture_source_walk_retained(
    roots: &RetainedBackupSourceRoots,
    layout: &BackupArchiveLayout,
) -> Result<CapturedSourceWalk, BackupError> {
    let mut files = Vec::new();
    let mut namespace = Vec::new();
    let project_link_name = OsStr::new(".forge-method.yaml");
    let project_link_entry = roots.project.required_entry(project_link_name)?;
    let project_link = roots.project.open_file_component(project_link_name)?;
    if !directory_entry_matches(&project_link_entry, &project_link.opened_metadata) {
        return Err(BackupError::Archive {
            reason: "Project Link identity changed before its handle was retained".to_owned(),
        });
    }
    namespace.push(CapturedSourceNamespaceEntry {
        logical_path: "project/.forge-method.yaml".to_owned(),
        object_id: project_link_entry.object_id,
        entry_type: SourceNamespaceEntryType::RegularFile,
    });
    capture_retained_source(
        &roots.project,
        project_link_name,
        project_link,
        "project/.forge-method.yaml".to_owned(),
        layout,
        &mut files,
    )?;
    if roots.project.required_entry(project_link_name)? != project_link_entry {
        return Err(BackupError::Archive {
            reason: "Project Link directory entry changed during capture".to_owned(),
        });
    }
    capture_directory(
        &roots.sidecar,
        "sidecar",
        layout,
        &mut files,
        &mut namespace,
    )?;
    files.sort_by(|left, right| left.metadata.logical_path.cmp(&right.metadata.logical_path));
    let mut identities = BTreeSet::new();
    for file in &files {
        let canonical = canonical_archive_path(&file.metadata.logical_path).map_err(|error| {
            BackupError::Manifest {
                reason: format!("source identity rejected: {error:?}"),
            }
        })?;
        if !identities.insert(canonical) {
            return Err(BackupError::Archive {
                reason: "duplicate normalized source identity".to_owned(),
            });
        }
    }
    Ok(CapturedSourceWalk { files, namespace })
}

fn capture_directory(
    directory: &RetainedSourceDirectory,
    logical_directory: &str,
    layout: &BackupArchiveLayout,
    files: &mut Vec<CapturedSourceFile>,
    namespace: &mut Vec<CapturedSourceNamespaceEntry>,
) -> Result<(), BackupError> {
    directory.validate_retained_stable()?;
    let children = directory.read_entries()?;
    for entry in &children {
        let path = directory.display_path.join(&entry.name);
        let name = entry
            .name
            .to_str()
            .ok_or_else(|| BackupError::InvalidPath {
                path: path.clone(),
                reason: "non-UTF-8 source name is not representable".to_owned(),
            })?;
        let logical = format!("{logical_directory}/{name}");
        let child = directory.open_child_component(&entry.name)?;
        if !directory_entry_matches(entry, child.opened_metadata()) {
            return Err(BackupError::Archive {
                reason: format!(
                    "source entry identity changed before it could be retained: {}",
                    path.display()
                ),
            });
        }
        namespace.push(CapturedSourceNamespaceEntry {
            logical_path: logical.clone(),
            object_id: entry.object_id,
            entry_type: match &child {
                RetainedSourceChild::Directory(_) => SourceNamespaceEntryType::Directory,
                RetainedSourceChild::File(_) => SourceNamespaceEntryType::RegularFile,
            },
        });
        match child {
            RetainedSourceChild::Directory(child) => {
                capture_directory(&child, &logical, layout, files, namespace)?;
                child.validate_retained_stable()?;
                let reopened = directory.open_directory_component(&entry.name)?;
                if !same_file_identity(&child.opened_metadata, &reopened.opened_metadata) {
                    return Err(BackupError::Archive {
                        reason: format!(
                            "source directory namespace was replaced: {}",
                            child.display_path.display()
                        ),
                    });
                }
            }
            RetainedSourceChild::File(child) => {
                capture_retained_source(directory, &entry.name, child, logical, layout, files)?;
            }
        }
    }
    let after = directory.read_entries()?;
    if children != after {
        return Err(BackupError::Archive {
            reason: format!(
                "source directory entries changed during traversal: {}",
                directory.display_path.display()
            ),
        });
    }
    directory.validate_retained_stable()
}

fn capture_retained_source(
    parent: &RetainedSourceDirectory,
    name: &OsStr,
    mut source: RetainedSourceFile,
    logical_path: String,
    layout: &BackupArchiveLayout,
    files: &mut Vec<CapturedSourceFile>,
) -> Result<(), BackupError> {
    validate_source_file_metadata(&source.opened_metadata, &source.display_path)?;
    let exclusion = BackupManifestDocument::explicit_source_exclusion(&logical_path, layout)
        .map_err(|error| BackupError::Manifest {
            reason: format!("source exclusion failed: {error:?}"),
        })?;
    let byte_length = source.opened_metadata.len();
    let bytes = if exclusion.is_some() {
        None
    } else {
        if byte_length > MAX_MEMBER_BYTES {
            return Err(BackupError::ResourceLimit {
                resource: "source bytes",
                maximum: MAX_MEMBER_BYTES,
            });
        }
        let mut bytes = Vec::with_capacity(usize_from_u64(byte_length, "source bytes")?);
        (&mut source.handle)
            .take(MAX_MEMBER_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source_error| io_error(&source.display_path, source_error))?;
        if bytes.len() as u64 > MAX_MEMBER_BYTES {
            return Err(BackupError::ResourceLimit {
                resource: "source bytes",
                maximum: MAX_MEMBER_BYTES,
            });
        }
        if bytes.len() as u64 != byte_length {
            return Err(BackupError::Archive {
                reason: format!(
                    "source length changed while reading {}",
                    source.display_path.display()
                ),
            });
        }
        Some(bytes)
    };
    source.validate_retained_stable()?;
    source.validate_namespace(parent, name)?;
    files.push(CapturedSourceFile {
        metadata: BackupSourceFileMetadata {
            logical_path,
            entry_type: BackupArchiveEntryType::RegularFile,
            hard_link_count: 1,
            byte_length,
            sha256: bytes.as_deref().map_or_else(|| sha256(&[]), sha256),
        },
        bytes,
    });
    Ok(())
}

fn capture_one_source(
    path: &Path,
    logical_path: String,
    layout: &BackupArchiveLayout,
    files: &mut Vec<CapturedSourceFile>,
) -> Result<(), BackupError> {
    let path = absolute_source_path(path)?;
    let parent_path = path.parent().ok_or_else(|| BackupError::InvalidPath {
        path: path.clone(),
        reason: "source leaf has no parent directory".to_owned(),
    })?;
    let name = path.file_name().ok_or_else(|| BackupError::InvalidPath {
        path: path.clone(),
        reason: "source leaf has no file name".to_owned(),
    })?;
    let parent = RetainedSourceDirectory::open_root(parent_path)?;
    parent.validate_namespace_stable()?;
    let entry = parent.required_entry(name)?;
    let source = parent.open_file_component(name)?;
    if !directory_entry_matches(&entry, &source.opened_metadata) {
        return Err(BackupError::Archive {
            reason: format!(
                "external source identity changed before its handle was retained: {}",
                path.display()
            ),
        });
    }
    capture_retained_source(&parent, name, source, logical_path, layout, files)?;
    if parent.required_entry(name)? != entry {
        return Err(BackupError::Archive {
            reason: format!(
                "external source entry changed during capture: {}",
                path.display()
            ),
        });
    }
    parent.validate_namespace_stable()
}

#[cfg(unix)]
mod source_platform {
    use super::{io, Digest, File, OsStr, OsString, Path, SourceDirectoryEntry};
    use std::os::unix::ffi::OsStringExt as _;

    pub(super) fn open_root_directory(path: &Path) -> io::Result<File> {
        use rustix::fs::{open, Mode, OFlags};
        let fd = open(
            path,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
            Mode::empty(),
        )
        .map_err(io::Error::from)?;
        Ok(File::from(fd))
    }

    pub(super) fn open_child(parent: &File, name: &OsStr) -> io::Result<File> {
        use rustix::fs::{openat, Mode, OFlags};
        let fd = openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(io::Error::from)?;
        Ok(File::from(fd))
    }

    pub(super) fn read_entries(directory: &File) -> io::Result<Vec<SourceDirectoryEntry>> {
        let mut directory = rustix::fs::Dir::read_from(directory).map_err(io::Error::from)?;
        let mut entries = Vec::new();
        for entry in &mut directory {
            let entry = entry.map_err(io::Error::from)?;
            let name = entry.file_name().to_bytes();
            if name != b"." && name != b".." {
                entries.push(SourceDirectoryEntry {
                    name: OsString::from_vec(name.to_vec()),
                    object_id: entry.ino(),
                });
            }
        }
        Ok(entries)
    }
}

#[cfg(windows)]
mod source_platform {
    use super::*;
    use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};
    use std::os::windows::fs::OpenOptionsExt as _;
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, RawHandle};

    type Handle = *mut std::ffi::c_void;
    type NtStatus = i32;
    const OBJ_CASE_INSENSITIVE: u32 = 0x40;
    const GENERIC_READ: u32 = 0x8000_0000;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_OPEN: u32 = 1;
    const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x20;
    const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }

    #[repr(C)]
    struct ObjectAttributes {
        length: u32,
        root_directory: Handle,
        object_name: *mut UnicodeString,
        attributes: u32,
        security_descriptor: *mut std::ffi::c_void,
        security_quality_of_service: *mut std::ffi::c_void,
    }

    #[repr(C)]
    union IoStatusValue {
        status: NtStatus,
        pointer: *mut std::ffi::c_void,
    }

    #[repr(C)]
    struct IoStatusBlock {
        value: IoStatusValue,
        information: usize,
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtCreateFile(
            file_handle: *mut Handle,
            desired_access: u32,
            object_attributes: *mut ObjectAttributes,
            io_status_block: *mut IoStatusBlock,
            allocation_size: *mut i64,
            file_attributes: u32,
            share_access: u32,
            create_disposition: u32,
            create_options: u32,
            ea_buffer: *mut std::ffi::c_void,
            ea_length: u32,
        ) -> NtStatus;
        fn RtlNtStatusToDosError(status: NtStatus) -> u32;
    }

    pub(super) fn open_root_directory(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
    }

    pub(super) fn open_child(parent: &File, name: &OsStr) -> io::Result<File> {
        let mut wide = name.encode_wide().collect::<Vec<_>>();
        let byte_len = wide
            .len()
            .checked_mul(2)
            .and_then(|length| u16::try_from(length).ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "source name too long"))?;
        let mut name = UnicodeString {
            length: byte_len,
            maximum_length: byte_len,
            buffer: wide.as_mut_ptr(),
        };
        let mut attributes = ObjectAttributes {
            length: u32::try_from(std::mem::size_of::<ObjectAttributes>())
                .expect("OBJECT_ATTRIBUTES size"),
            root_directory: parent.as_raw_handle().cast(),
            object_name: &mut name,
            attributes: OBJ_CASE_INSENSITIVE,
            security_descriptor: std::ptr::null_mut(),
            security_quality_of_service: std::ptr::null_mut(),
        };
        let mut io_status = IoStatusBlock {
            value: IoStatusValue { status: 0 },
            information: 0,
        };
        let mut handle: Handle = std::ptr::null_mut();
        // SAFETY: all pointers reference initialized storage for the duration of the call.
        let status = unsafe {
            NtCreateFile(
                &mut handle,
                GENERIC_READ | SYNCHRONIZE,
                &mut attributes,
                &mut io_status,
                std::ptr::null_mut(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                FILE_OPEN,
                FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                std::ptr::null_mut(),
                0,
            )
        };
        if status < 0 {
            // SAFETY: pure NTSTATUS conversion.
            return Err(io::Error::from_raw_os_error(
                unsafe { RtlNtStatusToDosError(status) } as i32,
            ));
        }
        // SAFETY: successful NtCreateFile returned one newly-owned handle.
        Ok(unsafe { File::from_raw_handle(handle as RawHandle) })
    }

    pub(super) fn read_entries(directory: &File) -> io::Result<Vec<SourceDirectoryEntry>> {
        use windows_sys::Win32::Storage::FileSystem::{
            FileIdBothDirectoryInfo, FileIdBothDirectoryRestartInfo, GetFileInformationByHandleEx,
            FILE_ID_BOTH_DIR_INFO,
        };

        const ERROR_NO_MORE_FILES: i32 = 18;
        const BUFFER_BYTES: usize = 64 * 1024;
        let mut restart = true;
        let mut entries = Vec::new();
        loop {
            let mut buffer = vec![0_u8; BUFFER_BYTES];
            let class = if restart {
                FileIdBothDirectoryRestartInfo
            } else {
                FileIdBothDirectoryInfo
            };
            // SAFETY: the directory handle is live and buffer is writable for its full length.
            let result = unsafe {
                GetFileInformationByHandleEx(
                    directory.as_raw_handle().cast(),
                    class,
                    buffer.as_mut_ptr().cast(),
                    u32::try_from(buffer.len()).expect("directory query buffer length"),
                )
            };
            if result == 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(ERROR_NO_MORE_FILES) {
                    break;
                }
                return Err(error);
            }
            restart = false;
            let mut offset = 0_usize;
            loop {
                let header = std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
                if offset
                    .checked_add(header)
                    .is_none_or(|end| end > buffer.len())
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "directory query returned a truncated entry",
                    ));
                }
                let entry_ptr =
                    unsafe { buffer.as_ptr().add(offset) }.cast::<FILE_ID_BOTH_DIR_INFO>();
                // SAFETY: the fixed header was bounds-checked; the buffer may be unaligned.
                let entry = unsafe { std::ptr::read_unaligned(entry_ptr) };
                let name_bytes = usize::try_from(entry.FileNameLength).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "directory name length overflow")
                })?;
                if name_bytes % 2 != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "directory query returned an odd UTF-16 byte length",
                    ));
                }
                let name_start = offset.checked_add(header).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "directory name offset overflow")
                })?;
                let name_end = name_start.checked_add(name_bytes).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "directory name length overflow")
                })?;
                if name_end > buffer.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "directory query returned a truncated name",
                    ));
                }
                let mut wide = Vec::with_capacity(name_bytes / 2);
                for index in 0..(name_bytes / 2) {
                    let pointer =
                        unsafe { buffer.as_ptr().add(name_start + index * 2) }.cast::<u16>();
                    // SAFETY: each two-byte unit is within the checked name range.
                    wide.push(unsafe { std::ptr::read_unaligned(pointer) });
                }
                let name = OsString::from_wide(&wide);
                if name != OsStr::new(".") && name != OsStr::new("..") {
                    entries.push(SourceDirectoryEntry {
                        name,
                        object_id: entry.FileId as u64,
                    });
                }
                if entry.NextEntryOffset == 0 {
                    break;
                }
                let next = usize::try_from(entry.NextEntryOffset).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "directory offset overflow")
                })?;
                if next < header
                    || offset
                        .checked_add(next)
                        .is_none_or(|end| end >= buffer.len())
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "directory query returned an invalid next-entry offset",
                    ));
                }
                offset += next;
            }
        }
        Ok(entries)
    }
}

#[cfg(not(any(unix, windows)))]
mod source_platform {
    use super::*;

    fn unsupported<T>() -> io::Result<T> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "descriptor-relative source traversal is unsupported on this platform",
        ))
    }

    pub(super) fn open_root_directory(_path: &Path) -> io::Result<File> {
        unsupported()
    }

    pub(super) fn open_child(_parent: &File, _name: &OsStr) -> io::Result<File> {
        unsupported()
    }

    pub(super) fn read_entries(_directory: &File) -> io::Result<Vec<SourceDirectoryEntry>> {
        unsupported()
    }
}

#[cfg(unix)]
fn same_file_object(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino() && left.file_type() == right.file_type()
}

#[cfg(windows)]
fn same_file_object(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    left.volume_serial_number().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index().is_some()
        && left.file_index() == right.file_index()
        && left.file_type() == right.file_type()
        && !metadata_is_reparse(left)
        && !metadata_is_reparse(right)
}

#[cfg(not(any(unix, windows)))]
fn same_file_object(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    same_file_object(left, right)
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(windows)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    same_file_object(left, right)
        && left.file_attributes() == right.file_attributes()
        && left.creation_time() == right.creation_time()
        && left.number_of_links() == right.number_of_links()
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
}

#[cfg(not(any(unix, windows)))]
fn same_file_identity(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

/// Capture, assemble, publish, and independently reverify one protected backup.
/// The host guard is borrowed through the complete source transaction and the
/// independently configured authority selects both replay trust and receipt I/O.
pub fn create_project_backup(
    request: &BackupCreateRequest,
    quiescence: &HostQuiescenceGuard,
) -> Result<BackupPublication, BackupError> {
    let authority = resolve_configured_backup_authority(&request.authority_id)?;
    if request.authority_id != authority.authority_id {
        return Err(BackupError::Receipt {
            reason: "resolved backup authority does not match the create request".to_owned(),
        });
    }
    let roots = resolve_backup_roots(&request.project_root)?;
    let archive_path = normalize_future_output_path(&request.archive_path)?;
    let archive_parent = archive_path
        .parent()
        .ok_or_else(|| BackupError::InvalidPath {
            path: archive_path.clone(),
            reason: "backup archive has no parent".to_owned(),
        })?;
    if archive_parent.starts_with(&roots.project_root)
        || archive_parent.starts_with(&roots.sidecar_root)
    {
        return Err(BackupError::InvalidPath {
            path: archive_path.clone(),
            reason: "backup archive and staging must remain outside project and sidecar state"
                .to_owned(),
        });
    }
    if archive_parent.starts_with(&authority.receipt_store)
        || authority.receipt_store.starts_with(archive_parent)
    {
        return Err(BackupError::InvalidPath {
            path: archive_path.clone(),
            reason:
                "backup archive and staging must remain disjoint from the protected receipt store"
                    .to_owned(),
        });
    }
    for protected in [&authority.receipt_store, &authority.replay_anchor_path] {
        if protected.starts_with(&roots.project_root)
            || protected.starts_with(&roots.sidecar_root)
            || roots.project_root.starts_with(protected)
            || roots.sidecar_root.starts_with(protected)
        {
            return Err(BackupError::InvalidPath {
                path: protected.clone(),
                reason: "protected backup authority overlaps project or sidecar state".to_owned(),
            });
        }
    }
    let layout = BackupArchiveLayout {
        project_link_archive_path: "project/.forge-method.yaml".to_owned(),
        sidecar_archive_root: "sidecar".to_owned(),
        state_root_relative_to_sidecar: ".forge-method".to_owned(),
    };
    let mut capture = capture_stable_source(
        &roots.project_root,
        &roots.sidecar_root,
        quiescence,
        &layout,
    )?;
    let replay_anchor_lock =
        crate::replay_anchor::acquire_replay_anchor_retained_lock_under_boundary(
            quiescence,
            &roots.state_root,
            &authority.replay_anchor_path,
        )
        .map_err(|source| BackupError::Receipt {
            reason: format!("cannot retain protected replay anchor for backup: {source}"),
        })?;
    let replay_anchor_snapshot = crate::replay_anchor::snapshot_replay_anchor_under_retained_lock(
        &roots.state_root,
        &authority.replay_anchor_path,
        &replay_anchor_lock,
    )
    .map_err(|source| BackupError::Receipt {
        reason: format!("cannot snapshot protected replay anchor for backup: {source}"),
    })?
    .into_parts();
    capture_optional_external_public_file(
        request.current_principal_registry.as_deref(),
        "sidecar/operator/workflow-principal-registry.yaml",
        &layout,
        &mut capture.files,
        None,
    )?;
    let expected_broker_audience = workflow_broker_audience(&roots.project_link.project_id.0);
    capture_optional_external_public_file(
        request.current_broker_registry.as_deref(),
        "sidecar/operator/workflow-broker-registry.yaml",
        &layout,
        &mut capture.files,
        Some(&expected_broker_audience),
    )?;
    capture
        .files
        .sort_by(|left, right| left.metadata.logical_path.cmp(&right.metadata.logical_path));
    reject_duplicate_source_identities(&capture.files)?;
    verify_expected_members(&capture, &request.expected_members)?;
    let replay_anchor_recheck = crate::replay_anchor::snapshot_replay_anchor_under_retained_lock(
        &roots.state_root,
        &authority.replay_anchor_path,
        &replay_anchor_lock,
    )
    .map_err(|source| BackupError::Receipt {
        reason: format!("cannot recheck protected replay anchor for backup: {source}"),
    })?
    .into_parts();
    if replay_anchor_snapshot != replay_anchor_recheck {
        return Err(BackupError::Receipt {
            reason: "protected replay anchor changed during backup capture".to_owned(),
        });
    }
    let replay = verify_replay_anchor_under_retained_lock(
        quiescence,
        &roots.state_root,
        &authority.replay_anchor_path,
        &replay_anchor_lock,
    )
    .map_err(|error| BackupError::Receipt {
        reason: format!("current replay authority rejected during capture: {error}"),
    })?;
    if replay.anchor != replay_anchor_recheck.0 {
        return Err(BackupError::Receipt {
            reason: "protected replay anchor changed during backup capture".to_owned(),
        });
    }
    let snapshot = assemble_captured_snapshot(
        capture,
        &roots,
        &layout,
        &request.governance,
        &authority,
        (replay, replay_anchor_recheck.1),
    )?;
    let publication =
        publish_captured_snapshot(&snapshot, &archive_path, &authority.receipt_store)?;
    let verified = verify_project_backup_under_retained_authority(
        &BackupVerifyRequest {
            project_root: roots.project_root,
            archive_path: publication.archive_path.clone(),
            authority_id: request.authority_id.clone(),
            current_principal_registry: request.current_principal_registry.clone(),
            current_broker_registry: request.current_broker_registry.clone(),
        },
        &authority,
        quiescence,
        &replay_anchor_lock,
    )?;
    if verified.archive_sha256() != publication.archive_sha256
        || verified.manifest().backup_manifest.manifest_set_digest
            != publication.manifest_set_digest
    {
        return Err(BackupError::Archive {
            reason: "published backup changed before final protected verification".to_owned(),
        });
    }
    Ok(publication)
}

struct ResolvedBackupRoots {
    project_root: PathBuf,
    sidecar_root: PathBuf,
    state_root: PathBuf,
    project_link: ProjectLinkDocument,
    project_link_bytes: Vec<u8>,
}

fn resolve_backup_roots(project_root: &Path) -> Result<ResolvedBackupRoots, BackupError> {
    let project_root =
        fs::canonicalize(project_root).map_err(|source| io_error(project_root, source))?;
    let link_path = project_root.join(".forge-method.yaml");
    ensure_nofollow_regular_single_link(&link_path)?;
    let project_link_bytes = read_file_bounded(&link_path, MAX_RECEIPT_BYTES)?;
    let project_link: ProjectLinkDocument =
        yaml_serde::from_slice(&project_link_bytes).map_err(|error| BackupError::Manifest {
            reason: format!("Project Link parse failed: {error}"),
        })?;
    let sidecar_candidate = project_root.join(&project_link.sidecar_root.0);
    let state_candidate = project_root.join(&project_link.state_root.0);
    let sidecar_root = fs::canonicalize(&sidecar_candidate)
        .map_err(|source| io_error(&sidecar_candidate, source))?;
    let state_root =
        fs::canonicalize(&state_candidate).map_err(|source| io_error(&state_candidate, source))?;
    if state_root.parent() != Some(sidecar_root.as_path())
        || state_root.file_name().and_then(|value| value.to_str()) != Some(".forge-method")
        || sidecar_root.starts_with(&project_root)
        || project_root.starts_with(&sidecar_root)
    {
        return Err(BackupError::InvalidPath {
            path: state_root,
            reason: "Project Link does not bind one disjoint sidecar with a direct .forge-method state root"
                .to_owned(),
        });
    }
    Ok(ResolvedBackupRoots {
        project_root,
        sidecar_root,
        state_root,
        project_link,
        project_link_bytes,
    })
}

fn capture_optional_external_public_file(
    path: Option<&Path>,
    logical_path: &str,
    layout: &BackupArchiveLayout,
    files: &mut Vec<CapturedSourceFile>,
    expected_broker_audience: Option<&str>,
) -> Result<(), BackupError> {
    let Some(path) = path else {
        return Ok(());
    };
    let mut first = Vec::new();
    capture_one_source(path, logical_path.to_owned(), layout, &mut first)?;
    if let Some(expected_audience) = expected_broker_audience {
        let raw = first
            .first()
            .and_then(|source| source.bytes.as_deref())
            .ok_or_else(|| BackupError::Archive {
                reason: "public workflow broker registry capture is incomplete".to_owned(),
            })?;
        validate_public_workflow_broker_registry(raw, expected_audience)?;
    }
    if let Some(existing) = files
        .iter()
        .find(|file| file.metadata.logical_path == logical_path)
    {
        if first.len() != 1
            || first[0].metadata != existing.metadata
            || first[0].bytes.as_deref() != existing.bytes.as_deref()
        {
            return Err(BackupError::Archive {
                reason: format!(
                    "project-bound public material {logical_path} differs from the full sidecar capture"
                ),
            });
        }
        return Ok(());
    }
    let mut second = Vec::new();
    capture_one_source(path, logical_path.to_owned(), layout, &mut second)?;
    if first.len() != 1
        || second.len() != 1
        || first[0].metadata != second[0].metadata
        || first[0].bytes.as_deref() != second[0].bytes.as_deref()
    {
        return Err(BackupError::Archive {
            reason: format!("external public material {logical_path} changed during capture"),
        });
    }
    files.extend(first);
    Ok(())
}

fn reject_duplicate_source_identities(files: &[CapturedSourceFile]) -> Result<(), BackupError> {
    let mut identities = BTreeSet::new();
    for file in files {
        let canonical = canonical_archive_path(&file.metadata.logical_path).map_err(|error| {
            BackupError::Manifest {
                reason: format!("source identity rejected: {error:?}"),
            }
        })?;
        if !identities.insert(canonical) {
            return Err(BackupError::Archive {
                reason: "duplicate normalized source identity across captured roots".to_owned(),
            });
        }
    }
    Ok(())
}

fn verify_expected_members(
    capture: &BackupSourceCapture,
    expected: &[BackupExpectedMember],
) -> Result<(), BackupError> {
    let mut observations = BTreeMap::new();
    for member in expected {
        if observations
            .insert(member.logical_path.as_str(), member.sha256.as_str())
            .is_some()
        {
            return Err(BackupError::Archive {
                reason: "duplicate typed producer snapshot expectation".to_owned(),
            });
        }
    }
    for (path, expected_sha256) in observations {
        let source = capture
            .files
            .iter()
            .find(|file| file.metadata.logical_path == path)
            .ok_or_else(|| BackupError::Archive {
                reason: format!("typed producer snapshot member {path} was omitted"),
            })?;
        if source.bytes.is_none() || source.metadata.sha256 != expected_sha256 {
            return Err(BackupError::Archive {
                reason: format!("typed producer snapshot member {path} was substituted"),
            });
        }
    }
    Ok(())
}

fn assemble_captured_snapshot(
    capture: BackupSourceCapture,
    roots: &ResolvedBackupRoots,
    layout: &BackupArchiveLayout,
    governance: &BackupGovernanceProjection,
    authority: &TrustedBackupAuthority,
    replay_anchor_snapshot: (ReplayAnchorVerification, Vec<u8>),
) -> Result<CapturedBackupSnapshot, BackupError> {
    validate_governance_projection(governance, &roots.project_link.project_id.0, &capture)?;
    let expected_broker_audience = workflow_broker_audience(&roots.project_link.project_id.0);
    let mut members = Vec::new();
    for source in &capture.files {
        if BackupManifestDocument::explicit_source_exclusion(&source.metadata.logical_path, layout)
            .map_err(manifest_error)?
            .is_some()
        {
            if source.bytes.is_some() {
                return Err(BackupError::Archive {
                    reason: format!(
                        "excluded source {} unexpectedly exposed content",
                        source.metadata.logical_path
                    ),
                });
            }
            continue;
        }
        let material = classify_authoritative_member(&source.metadata.logical_path, layout)
            .ok_or_else(|| BackupError::Manifest {
                reason: format!(
                    "unclassified authoritative source {}",
                    source.metadata.logical_path
                ),
            })?;
        let bytes = source.bytes.clone().ok_or_else(|| BackupError::Archive {
            reason: format!(
                "archive member {} has no captured bytes",
                source.metadata.logical_path
            ),
        })?;
        if material == BackupEntryKind::PublicBrokerRegistry {
            validate_public_workflow_broker_registry(&bytes, &expected_broker_audience)?;
        }
        members.push(CapturedBackupMember {
            entry: BackupEntry {
                material,
                logical_path: canonical_archive_path(&source.metadata.logical_path)
                    .map_err(manifest_error)?,
                entry_type: BackupArchiveEntryType::RegularFile,
                byte_length: u64::try_from(bytes.len()).map_err(|_| {
                    BackupError::ResourceLimit {
                        resource: "member bytes",
                        maximum: MAX_MEMBER_BYTES,
                    }
                })?,
                sha256: sha256(&bytes),
            },
            bytes,
        });
    }
    members.sort_by(|left, right| {
        (left.entry.material, left.entry.logical_path.as_str())
            .cmp(&(right.entry.material, right.entry.logical_path.as_str()))
    });
    let entries = members
        .iter()
        .map(|member| member.entry.clone())
        .collect::<Vec<_>>();
    let external_authority_observations =
        capture_external_authorities(roots, &members, authority, replay_anchor_snapshot)?;
    let mut source_state =
        derive_source_state(&members, &external_authority_observations, authority)?;
    let mut manifest = BackupManifestDocument {
        schema_version: BACKUP_MANIFEST_SCHEMA_VERSION.to_owned(),
        backup_manifest: BackupManifest {
            format: BackupManifestFormat::ForgeProjectStateBackupV1,
            project: BackupProjectBinding {
                project_link: roots.project_link.clone(),
                project_link_sha256: sha256(&roots.project_link_bytes),
                archive_layout: layout.clone(),
            },
            workflow_release: governance.workflow_release.clone(),
            effective_epoch: BackupEffectiveEpochBinding {
                epoch_id: format!("workflow-effective:{}", roots.project_link.project_id.0),
                epoch_generation: governance.state_version.max(1),
                effective_bundle: governance.effective_bundle.clone(),
                governance_ledger_head_digest: governance.governance_ledger_head_digest.clone(),
            },
            source_state: source_state.clone(),
            snapshot_protocol: BackupSnapshotProtocol {
                mode:
                    BackupSnapshotMode::CooperativeLocksWithProducerQuiescenceAndStableEnumeration,
                lock_order: BACKUP_LOCK_ORDER.to_vec(),
                unlocked_producer_boundary:
                    BackupUnlockedProducerBoundary::OpaqueExclusiveQuiescenceRequiredByRestoreEngine,
            },
            entries,
            external_authority_observations,
            forbidden_private_material: vec![
                BackupForbiddenPrivateMaterial::BrokerPrivateKeys,
                BackupForbiddenPrivateMaterial::WorkflowSecretRoots,
                BackupForbiddenPrivateMaterial::OperatorSecretRoots,
                BackupForbiddenPrivateMaterial::McpPrivateKeys,
            ],
            manifest_set_digest: String::new(),
        },
    };
    let effect_index =
        member_bytes(&members, BackupEntryKind::EffectMetadataIndex).unwrap_or_default();
    let parsed_effects = manifest
        .derive_effect_metadata_index_bytes(effect_index)
        .map_err(manifest_error)?;
    source_state.declared_effect_outputs = parsed_effects
        .into_iter()
        .map(|output| BackupDeclaredEffectOutput {
            operation_id: output.operation_id,
            effect_id: output.effect_id,
            target_kind: output.target_kind,
            physical_ref: output.physical_ref,
            logical_ref: output.logical_ref,
            state_relative_path: output.state_relative_path,
            access_mode: output.access_mode,
            byte_length: output.byte_length,
            metadata_record_sha256: output.source_record_sha256,
            content_sha256: output.content_sha256,
        })
        .collect();
    manifest.backup_manifest.source_state = source_state;
    manifest.backup_manifest.manifest_set_digest = manifest.set_digest().map_err(manifest_error)?;
    manifest.validate_integrity().map_err(manifest_error)?;
    manifest
        .verify_source_enumeration(
            &capture
                .files
                .iter()
                .map(|file| file.metadata.clone())
                .collect::<Vec<_>>(),
        )
        .map_err(manifest_error)?;
    manifest
        .verify_effect_metadata_index_bytes(effect_index)
        .map_err(manifest_error)?;
    Ok(CapturedBackupSnapshot { manifest, members })
}

fn validate_governance_projection(
    governance: &BackupGovernanceProjection,
    expected_project_id: &str,
    capture: &BackupSourceCapture,
) -> Result<(), BackupError> {
    if governance.state_version == 0 || governance.governance_ledger_head_digest.trim().is_empty() {
        return Err(BackupError::Manifest {
            reason: "kernel governance projection is incomplete".to_owned(),
        });
    }
    let wal = capture
        .files
        .iter()
        .find(|file| {
            file.metadata.logical_path == "sidecar/.forge-method/wal/workflow-governance.ndjson"
        })
        .and_then(|file| file.bytes.as_deref())
        .ok_or_else(|| BackupError::Manifest {
            reason: "workflow governance WAL is not initialized".to_owned(),
        })?;
    if wal.is_empty() || !wal.ends_with(b"\n") {
        return Err(BackupError::Manifest {
            reason: "workflow governance WAL is empty or partial".to_owned(),
        });
    }
    let mut records = Vec::new();
    let mut values = Vec::new();
    for line in wal
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let record: WorkflowGovernanceLedgerRecord =
            serde_json::from_slice(line).map_err(|error| BackupError::Manifest {
                reason: format!("workflow governance WAL parse failed: {error}"),
            })?;
        let value = serde_json::to_value(&record).map_err(|error| BackupError::Manifest {
            reason: format!("workflow governance WAL projection failed: {error}"),
        })?;
        records.push(record);
        values.push(value);
    }
    for (index, record) in records.iter().enumerate() {
        if record.project_id.0 != expected_project_id
            || record.sequence != (index as u64).saturating_add(1)
            || (index == 0 && record.previous_record_digest.is_some())
            || (index > 0
                && record.previous_record_digest.as_deref()
                    != Some(records[index - 1].record_digest.as_str()))
        {
            return Err(BackupError::Manifest {
                reason: "workflow governance WAL identity or hash chain is invalid".to_owned(),
            });
        }
    }
    let last = records.last().ok_or_else(|| BackupError::Manifest {
        reason: "workflow governance WAL has no complete record".to_owned(),
    })?;
    let release_value = serde_json::to_value(&governance.workflow_release).map_err(|error| {
        BackupError::Manifest {
            reason: format!("workflow release projection failed: {error}"),
        }
    })?;
    let effective_value = serde_json::to_value(&governance.effective_bundle).map_err(|error| {
        BackupError::Manifest {
            reason: format!("effective bundle projection failed: {error}"),
        }
    })?;
    let release_bound = values
        .iter()
        .any(|record| json_contains_value(record, &release_value))
        || last.bundle_digest == governance.workflow_release.release_digest
        || last.bundle_digest
            == governance
                .effective_bundle
                .core_runtime_bundle
                .bundle_digest;
    let effective_bound = values
        .iter()
        .any(|record| json_contains_value(record, &effective_value))
        || (governance.effective_bundle.domain_pack_generation.is_none()
            && governance.effective_bundle.core_runtime_bundle
                == governance.effective_bundle.effective_runtime_bundle
            && last.bundle_id
                == governance
                    .effective_bundle
                    .effective_runtime_bundle
                    .bundle_id
            && last.bundle_digest
                == governance
                    .effective_bundle
                    .effective_runtime_bundle
                    .bundle_digest);
    if last.record_digest != governance.governance_ledger_head_digest
        || last.state_version != governance.state_version
        || !release_bound
        || !effective_bound
    {
        return Err(BackupError::Manifest {
            reason: "kernel governance projection is stale or substituted".to_owned(),
        });
    }
    Ok(())
}

fn json_contains_value(value: &serde_json::Value, expected: &serde_json::Value) -> bool {
    if value == expected {
        return true;
    }
    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| json_contains_value(value, expected)),
        serde_json::Value::Object(values) => values
            .values()
            .any(|value| json_contains_value(value, expected)),
        _ => false,
    }
}

fn classify_authoritative_member(
    path: &str,
    layout: &BackupArchiveLayout,
) -> Option<BackupEntryKind> {
    let state = format!(
        "{}/{}",
        layout.sidecar_archive_root, layout.state_root_relative_to_sidecar
    );
    let exact = |suffix: &str| path == format!("{state}/{suffix}");
    let below = |directory: &str| path.starts_with(&format!("{state}/{directory}/"));
    if path == layout.project_link_archive_path {
        Some(BackupEntryKind::ProjectLink)
    } else if exact("state.yaml") {
        Some(BackupEntryKind::ProjectState)
    } else if exact("ledger.ndjson") {
        Some(BackupEntryKind::RootLedger)
    } else if exact("replay-wal.manifest.json") {
        Some(BackupEntryKind::ReplayWalManifest)
    } else if exact("wal/replay.fmr1") {
        Some(BackupEntryKind::ReplayWal)
    } else if exact("wal/workflow-governance.ndjson") {
        Some(BackupEntryKind::WorkflowGovernanceWal)
    } else if exact("wal/claims.fmw1") {
        Some(BackupEntryKind::ClaimWal)
    } else if exact("wal/claims.wal.manifest.json") {
        Some(BackupEntryKind::ClaimWalManifest)
    } else if below("wal/snapshots") {
        Some(BackupEntryKind::ClaimWalSnapshot)
    } else if below("wal/archive") {
        Some(BackupEntryKind::ClaimWalArchive)
    } else if exact("workflow-action-replay.manifest.json") {
        Some(BackupEntryKind::WorkflowActionReplayManifest)
    } else if exact("wal/workflow-action-replay.jsonl") {
        Some(BackupEntryKind::WorkflowActionReplayWal)
    } else if exact("wal/effects.ndjson") {
        Some(BackupEntryKind::EffectWal)
    } else if exact("wal/.effects.ndjson.compaction-manifest.json") {
        Some(BackupEntryKind::EffectWalCompactionManifest)
    } else if exact("memory/events.ndjson") {
        Some(BackupEntryKind::MemoryEventLog)
    } else if exact("research/sources.ndjson") {
        Some(BackupEntryKind::ResearchEventLog)
    } else if exact("governance/conflicts.ndjson") {
        Some(BackupEntryKind::GovernanceConflictEventLog)
    } else if exact("domain-packs/operator-sources.yaml") {
        Some(BackupEntryKind::DomainPackOperatorSources)
    } else if exact("domain-packs/rebase-plan.yaml") {
        Some(BackupEntryKind::DomainPackRebasePlan)
    } else if exact("domain-packs/active.lock.yaml") {
        Some(BackupEntryKind::DomainPackActivePointer)
    } else if below("domain-packs/ledger") && path.ends_with(".yaml") {
        Some(BackupEntryKind::DomainPackLedgerRecord)
    } else if below("domain-packs/generations") && path.ends_with("/generation.yaml") {
        Some(BackupEntryKind::DomainPackGenerationManifest)
    } else if below("domain-packs/generations") && path.ends_with("/catalog.yaml") {
        Some(BackupEntryKind::DomainPackGenerationCatalog)
    } else if below("domain-packs/generations") && path.ends_with("/lock.yaml") {
        Some(BackupEntryKind::DomainPackGenerationLock)
    } else if below("domain-packs/generations") && path.ends_with("/preflight.yaml") {
        Some(BackupEntryKind::DomainPackGenerationPreflight)
    } else if below("domain-packs/generations") && path.ends_with("/compatibility.yaml") {
        Some(BackupEntryKind::DomainPackGenerationCompatibility)
    } else if below("domain-packs/generations") && path.ends_with("/receipt.yaml") {
        Some(BackupEntryKind::DomainPackGenerationReceipt)
    } else if below("domain-packs/generations") && path.ends_with("/resolution-request.yaml") {
        Some(BackupEntryKind::DomainPackGenerationResolutionRequest)
    } else if below("domain-packs/generations") && path.ends_with("/composition-request.yaml") {
        Some(BackupEntryKind::DomainPackGenerationCompositionRequest)
    } else if below("domain-packs/generations") && path.ends_with("/trust-input.yaml") {
        Some(BackupEntryKind::DomainPackGenerationTrustInput)
    } else if below("domain-packs/receipts") && path.ends_with(".yaml") {
        Some(BackupEntryKind::DomainPackPublishedReceipt)
    } else if below("domain-packs/objects") {
        Some(BackupEntryKind::DomainPackObject)
    } else if exact("domain-pack-learning/index.json") {
        Some(BackupEntryKind::DomainPackLearningIndex)
    } else if below("domain-pack-learning/objects") {
        Some(BackupEntryKind::DomainPackLearningObject)
    } else if below("contracts/isolations") && path.ends_with(".yaml") {
        Some(BackupEntryKind::IsolationContract)
    } else if path == "sidecar/operator/workflow-principal-registry.yaml" {
        Some(BackupEntryKind::PublicPrincipalRegistry)
    } else if path == "sidecar/operator/workflow-broker-registry.yaml" {
        Some(BackupEntryKind::PublicBrokerRegistry)
    } else if below("claims-active") && path.ends_with(".yaml") {
        Some(BackupEntryKind::ClaimCache)
    } else if below("handoffs/expired-claims") && path.ends_with(".yaml") {
        Some(BackupEntryKind::OfficialHandoffArtifact)
    } else if below("artifacts") {
        Some(BackupEntryKind::Artifact)
    } else if below("evidence") {
        Some(BackupEntryKind::Evidence)
    } else if below("snapshots") {
        Some(BackupEntryKind::Snapshot)
    } else if below("ledger") {
        Some(BackupEntryKind::LedgerStream)
    } else if below("requests") || exact("requests.ndjson") {
        Some(BackupEntryKind::RequestStream)
    } else if below("runtime") {
        Some(BackupEntryKind::RuntimeSnapshot)
    } else if below("stories") {
        Some(BackupEntryKind::StoryState)
    } else if below("agents") {
        Some(BackupEntryKind::AgentRegistryState)
    } else if exact("preflight.yaml") {
        Some(BackupEntryKind::PreflightProfile)
    } else if exact("index/effect-targets.ndjson") {
        Some(BackupEntryKind::EffectMetadataIndex)
    } else if exact("traces/events.ndjson") {
        Some(BackupEntryKind::TraceLog)
    } else if ["artifacts", "evidence", "snapshots", "ledger", "requests"]
        .iter()
        .any(|root| exact(root))
        || below("custom")
    {
        Some(BackupEntryKind::DeclaredEffectOutput)
    } else {
        None
    }
}

fn member_bytes(members: &[CapturedBackupMember], kind: BackupEntryKind) -> Option<&[u8]> {
    members
        .iter()
        .find(|member| member.entry.material == kind)
        .map(|member| member.bytes.as_slice())
}

fn members_of_kind(
    members: &[CapturedBackupMember],
    kind: BackupEntryKind,
) -> impl Iterator<Item = &CapturedBackupMember> {
    members
        .iter()
        .filter(move |member| member.entry.material == kind)
}

fn capture_external_authorities(
    roots: &ResolvedBackupRoots,
    members: &[CapturedBackupMember],
    authority: &TrustedBackupAuthority,
    replay_anchor_snapshot: (ReplayAnchorVerification, Vec<u8>),
) -> Result<BackupExternalAuthorityObservations, BackupError> {
    let (replay, anchor_bytes) = replay_anchor_snapshot;
    let anchor = &replay.anchor;
    if replay.status != ReplayAnchorStatus::Current || anchor.head != replay.current_head {
        return Err(BackupError::Receipt {
            reason: "current replay authority is stale, partial, rolled back, substituted, or not exactly anchored"
                .to_owned(),
        });
    }
    let replay_rollback_anchor = BackupReplayRollbackAnchor {
        schema_version: anchor.schema_version.clone(),
        protected_anchor_identity: authority.protected_anchor_identity.clone(),
        deployment_id: anchor.deployment_id.clone(),
        epoch: anchor.epoch.clone(),
        generation: anchor.generation,
        previous_anchor_digest: anchor.previous_anchor_digest.clone(),
        anchor_document_sha256: sha256(&anchor_bytes),
        replay_wal_manifest_digest: replay.current_head.manifest_digest.clone(),
        replay_wal_prefix_digest: replay.current_head.wal_prefix_digest.clone(),
        replay_wal_last_seq: replay.current_head.last_seq,
        replay_wal_record_count: replay.current_head.record_count as u64,
        replay_wal_byte_length: replay.current_head.byte_len,
    };
    let principal = public_principal_registry_observation(member_bytes(
        members,
        BackupEntryKind::PublicPrincipalRegistry,
    ))?;
    let broker = public_workflow_broker_registry_observation(
        member_bytes(members, BackupEntryKind::PublicBrokerRegistry),
        &workflow_broker_audience(&roots.project_link.project_id.0),
    )?;
    let (supply_chain, reviewed_learning) =
        capture_domain_pack_authorities(members, authority.domain_pack_operator.as_ref())?;
    Ok(BackupExternalAuthorityObservations {
        replay_rollback_anchor,
        domain_pack_supply_chain: supply_chain,
        domain_pack_reviewed_learning: reviewed_learning,
        workflow_principal_registry: principal,
        workflow_broker_registry: broker,
    })
}

fn public_principal_registry_observation(
    raw: Option<&[u8]>,
) -> Result<Option<BackupPublicRegistryMaterial>, BackupError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let value = parse_yaml_value(raw, "workflow principal registry")?;
    Ok(Some(BackupPublicRegistryMaterial {
        schema_version: value_string(&value, &["schema_version"])?,
        audience: value_string(&value, &["principal_registry", "audience"])?,
        registry_sha256: sha256(raw),
    }))
}

fn public_workflow_broker_registry_observation(
    raw: Option<&[u8]>,
    expected_audience: &str,
) -> Result<Option<BackupPublicRegistryMaterial>, BackupError> {
    raw.map(|raw| validate_public_workflow_broker_registry(raw, expected_audience))
        .transpose()
}

fn workflow_broker_audience(project_id: &str) -> String {
    format!("forge-core:workflow:{project_id}")
}

fn validate_public_workflow_broker_registry(
    raw: &[u8],
    expected_audience: &str,
) -> Result<BackupPublicRegistryMaterial, BackupError> {
    let document: WorkflowBrokerRegistryDocument =
        yaml_serde::from_slice(raw).map_err(|error| BackupError::Archive {
            reason: format!("strict public workflow broker registry parse failed: {error}"),
        })?;
    let schema_version = document.schema_version.clone();
    let audience = document.audience.clone();
    AuthorizedWorkflowBrokerRegistry::from_document_for_audience(document, expected_audience)
        .map_err(|error| BackupError::Archive {
            reason: format!("canonical public workflow broker registry admission failed: {error}"),
        })?;
    Ok(BackupPublicRegistryMaterial {
        schema_version,
        audience,
        registry_sha256: sha256(raw),
    })
}

fn capture_domain_pack_authorities(
    members: &[CapturedBackupMember],
    operator: Option<&ConfiguredDomainPackOperator>,
) -> Result<
    (
        Option<BackupDomainPackSupplyChainAuthority>,
        Option<BackupDomainPackLearningAuthority>,
    ),
    BackupError,
> {
    let Some(raw_sources) = member_bytes(members, BackupEntryKind::DomainPackOperatorSources)
    else {
        return Ok((None, None));
    };
    let operator = operator.ok_or_else(|| BackupError::Receipt {
        reason: "project has Domain Pack operator sources but no configured operator authority"
            .to_owned(),
    })?;
    let sources = parse_yaml_value(raw_sources, "Domain Pack operator sources")?;
    let configured_root = fs::canonicalize(value_string(&sources, &["operator_root"])?)
        .map_err(|source| io_error(&operator.root, source))?;
    if configured_root != operator.root {
        return Err(BackupError::Receipt {
            reason: "Domain Pack operator-source root differs from configured authority".to_owned(),
        });
    }
    let trust_policy =
        read_operator_source_file(operator, &value_string(&sources, &["trust_policy_file"])?)?;
    let registry =
        read_operator_source_file(operator, &value_string(&sources, &["registry_file"])?)?;
    let reviewer_registry_bytes = read_operator_source_file(
        operator,
        &value_string(&sources, &["reviewer_registry_file"])?,
    )?;
    let accepted_registry_bytes = read_operator_source_file(
        operator,
        &value_string(&sources, &["reviewed_registry_file"])?,
    )?;
    let capability = read_operator_source_file(
        operator,
        &value_string(&sources, &["capability_registry_file"])?,
    )?;
    let sandbox =
        read_operator_source_file(operator, &value_string(&sources, &["sandbox_policy_file"])?)?;
    let supply_anchor_path = operator
        .root
        .join(".forge-domain-pack-registry-anchor.yaml");
    ensure_nofollow_regular_single_link(&supply_anchor_path)?;
    let supply_anchor_raw = read_file_bounded(&supply_anchor_path, MAX_RECEIPT_BYTES)?;
    let supply: DomainPackRegistryAnchorHead =
        yaml_serde::from_slice(&supply_anchor_raw).map_err(|error| BackupError::Receipt {
            reason: format!("Domain Pack supply-chain anchor parse failed: {error}"),
        })?;
    let learning_anchor_path = operator
        .root
        .join(".forge-domain-pack-learning-anchor.yaml");
    ensure_nofollow_regular_single_link(&learning_anchor_path)?;
    let learning_anchor_raw = read_file_bounded(&learning_anchor_path, MAX_RECEIPT_BYTES)?;
    let learning: LearningAnchorHead =
        yaml_serde::from_slice(&learning_anchor_raw).map_err(|error| BackupError::Receipt {
            reason: format!("Domain Pack reviewed-learning anchor parse failed: {error}"),
        })?;
    let supply_chain = BackupDomainPackSupplyChainAuthority {
        schema_version: supply.schema_version,
        operator_root_identity: operator.root_identity.clone(),
        registry_id: supply.registry_id.0,
        audience: supply.audience.0,
        generation: supply.generation,
        anchor_document_sha256: sha256(&supply_anchor_raw),
        registry_snapshot_digest: supply.snapshot_digest,
        trust_policy_digest: supply.trust_policy_digest,
        registry_file_sha256: sha256(&registry),
        trust_policy_file_sha256: sha256(&trust_policy),
        capability_registry_file_sha256: sha256(&capability),
        sandbox_policy_file_sha256: sha256(&sandbox),
    };
    let reviewed_learning = BackupDomainPackLearningAuthority {
        schema_version: learning.schema_version,
        operator_root_identity: operator.root_identity.clone(),
        reviewer_registry_id: learning.reviewer.registry_id.0,
        reviewer_audience: learning.reviewer.audience,
        reviewer_generation: learning.reviewer.generation,
        reviewer_registry_digest: learning.reviewer.registry_digest,
        reviewed_registry_id: learning.reviewed.registry_id.0,
        reviewed_audience: learning.reviewed.audience,
        reviewed_generation: learning.reviewed.generation,
        reviewed_registry_digest: learning.reviewed.registry_digest,
        anchor_document_sha256: sha256(&learning_anchor_raw),
        reviewer_registry_file_sha256: sha256(&reviewer_registry_bytes),
        reviewed_registry_file_sha256: sha256(&accepted_registry_bytes),
    };
    Ok((Some(supply_chain), Some(reviewed_learning)))
}

fn read_operator_source_file(
    operator: &ConfiguredDomainPackOperator,
    configured: &str,
) -> Result<Vec<u8>, BackupError> {
    let candidate = PathBuf::from(configured);
    let path = if candidate.is_absolute() {
        candidate
    } else {
        operator.root.join(candidate)
    };
    ensure_nofollow_regular_single_link(&path)?;
    let canonical = fs::canonicalize(&path).map_err(|source| io_error(&path, source))?;
    if canonical.parent() != Some(operator.root.as_path()) {
        return Err(BackupError::InvalidPath {
            path: canonical,
            reason: "Domain Pack public authority file is not a direct operator-root child"
                .to_owned(),
        });
    }
    read_file_bounded(&path, MAX_RECEIPT_BYTES)
}

fn derive_source_state(
    members: &[CapturedBackupMember],
    observations: &BackupExternalAuthorityObservations,
    authority: &TrustedBackupAuthority,
) -> Result<BackupSourceState, BackupError> {
    let count = |kind| members_of_kind(members, kind).count() as u64;
    let claim_rotations = count(BackupEntryKind::ClaimWalSnapshot);
    if claim_rotations != count(BackupEntryKind::ClaimWalArchive) {
        return Err(BackupError::Manifest {
            reason: "claim WAL snapshot/archive rotation closure is incomplete".to_owned(),
        });
    }
    let claim_store = if count(BackupEntryKind::ClaimWal) == 0 {
        BackupClaimStoreState::EmptyBeforeFirstClaim
    } else {
        BackupClaimStoreState::Active {
            rotation_generations: claim_rotations,
        }
    };
    let effect_store = if count(BackupEntryKind::EffectWal) == 0 {
        BackupEffectStoreState::EmptyBeforeFirstEffect
    } else {
        BackupEffectStoreState::Active {
            compaction_manifest_present: count(BackupEntryKind::EffectWalCompactionManifest) == 1,
        }
    };
    let domain_pack_operator_sources = derive_operator_sources_projection(members, authority)?;
    let domain_pack_store = derive_domain_pack_store(members)?;
    let domain_pack_learning_store = derive_learning_store(members)?;
    let isolation_store = derive_isolation_store(members)?;
    Ok(BackupSourceState {
        project_state: if count(BackupEntryKind::ProjectState) == 0 {
            BackupProjectState::InitializedBeforeStart
        } else {
            BackupProjectState::StartedWithStateYaml
        },
        claim_store,
        workflow_governance_store: initialized(count(BackupEntryKind::WorkflowGovernanceWal)),
        workflow_action_replay_store: initialized(count(BackupEntryKind::WorkflowActionReplayWal)),
        effect_store,
        memory_store: initialized(count(BackupEntryKind::MemoryEventLog)),
        research_store: initialized(count(BackupEntryKind::ResearchEventLog)),
        governance_conflict_store: initialized(count(BackupEntryKind::GovernanceConflictEventLog)),
        domain_pack_store,
        domain_pack_operator_sources,
        domain_pack_learning_store,
        isolation_store,
        domain_pack_supply_chain_anchor: provisioned(
            observations.domain_pack_supply_chain.is_some(),
        ),
        domain_pack_reviewed_learning_anchor: provisioned(
            observations.domain_pack_reviewed_learning.is_some(),
        ),
        workflow_principal_registry: provisioned(
            observations.workflow_principal_registry.is_some(),
        ),
        workflow_broker_registry: provisioned(observations.workflow_broker_registry.is_some()),
        declared_effect_outputs: Vec::new(),
        public_sidecars: BackupPublicSidecarCounts {
            claim_cache_files: count(BackupEntryKind::ClaimCache),
            official_handoff_artifacts: count(BackupEntryKind::OfficialHandoffArtifact),
            artifacts: count(BackupEntryKind::Artifact),
            evidence: count(BackupEntryKind::Evidence),
            snapshots: count(BackupEntryKind::Snapshot),
            ledger_streams: count(BackupEntryKind::LedgerStream),
            request_streams: count(BackupEntryKind::RequestStream),
            runtime_snapshots: count(BackupEntryKind::RuntimeSnapshot),
            story_state: count(BackupEntryKind::StoryState),
            agent_registry_state: count(BackupEntryKind::AgentRegistryState),
            preflight_profiles: count(BackupEntryKind::PreflightProfile),
            effect_metadata_indexes: count(BackupEntryKind::EffectMetadataIndex),
            trace_logs: count(BackupEntryKind::TraceLog),
        },
    })
}

fn initialized(count: u64) -> BackupInitializationState {
    if count == 0 {
        BackupInitializationState::BeforeInitialization
    } else {
        BackupInitializationState::Initialized
    }
}

fn provisioned(present: bool) -> BackupProvisioningState {
    if present {
        BackupProvisioningState::Provisioned
    } else {
        BackupProvisioningState::NotProvisioned
    }
}

fn derive_domain_pack_store(
    members: &[CapturedBackupMember],
) -> Result<BackupDomainPackStoreState, BackupError> {
    let operator_sources_present =
        member_bytes(members, BackupEntryKind::DomainPackOperatorSources).is_some();
    let rebase_plan_present =
        member_bytes(members, BackupEntryKind::DomainPackRebasePlan).is_some();
    let Some(active_raw) = member_bytes(members, BackupEntryKind::DomainPackActivePointer) else {
        return Ok(BackupDomainPackStoreState::NoActiveGeneration {
            operator_sources_present,
            rebase_plan_present,
        });
    };
    let active = parse_yaml_value(active_raw, "Domain Pack active pointer")?;
    let active_generation = value_u64(&active, &["domain_pack_active_pointer", "generation"])?;
    let mut generations = Vec::new();
    for member in members_of_kind(members, BackupEntryKind::DomainPackGenerationManifest) {
        let value = parse_yaml_value(&member.bytes, "Domain Pack generation manifest")?;
        generations.push(BackupDomainPackGeneration {
            generation: value_u64(&value, &["generation"])?,
            record_digest: value_string(&value, &["record_digest"])?,
            receipt_digest: value_string(&value, &["receipt_digest"])?,
            object_raw_digests: value_string_array(&value, &["object_raw_digests"])?,
        });
    }
    generations.sort_by_key(|generation| generation.generation);
    if generations.last().map(|generation| generation.generation) != Some(active_generation) {
        return Err(BackupError::Manifest {
            reason: "Domain Pack active pointer and generation closure differ".to_owned(),
        });
    }
    Ok(BackupDomainPackStoreState::Active {
        operator_sources_present,
        rebase_plan_present,
        active_generation,
        generations,
    })
}

fn derive_operator_sources_projection(
    members: &[CapturedBackupMember],
    authority: &TrustedBackupAuthority,
) -> Result<Option<BackupDomainPackOperatorSourcesProjection>, BackupError> {
    let Some(raw) = member_bytes(members, BackupEntryKind::DomainPackOperatorSources) else {
        return Ok(None);
    };
    let operator = authority
        .domain_pack_operator
        .as_ref()
        .ok_or_else(|| BackupError::Receipt {
            reason: "Domain Pack operator sources have no configured authority".to_owned(),
        })?;
    let value = parse_yaml_value(raw, "Domain Pack operator sources")?;
    let trust_policy_file = value_string(&value, &["trust_policy_file"])?;
    let registry_file = value_string(&value, &["registry_file"])?;
    let reviewer_source = value_string(&value, &["reviewer_registry_file"])?;
    let accepted_source = value_string(&value, &["reviewed_registry_file"])?;
    let capability_registry_file = value_string(&value, &["capability_registry_file"])?;
    let sandbox_policy_file = value_string(&value, &["sandbox_policy_file"])?;
    Ok(Some(BackupDomainPackOperatorSourcesProjection {
        schema_version: value_string(&value, &["schema_version"])?,
        file_sha256: sha256(raw),
        operator_root_identity: operator.root_identity.clone(),
        trust_policy_file: trust_policy_file.clone(),
        trust_policy_file_sha256: sha256(&read_operator_source_file(operator, &trust_policy_file)?),
        registry_file: registry_file.clone(),
        registry_file_sha256: sha256(&read_operator_source_file(operator, &registry_file)?),
        reviewer_registry_file: reviewer_source.clone(),
        reviewer_registry_file_sha256: sha256(&read_operator_source_file(
            operator,
            &reviewer_source,
        )?),
        reviewed_registry_file: accepted_source.clone(),
        reviewed_registry_file_sha256: sha256(&read_operator_source_file(
            operator,
            &accepted_source,
        )?),
        capability_registry_file: capability_registry_file.clone(),
        capability_registry_file_sha256: sha256(&read_operator_source_file(
            operator,
            &capability_registry_file,
        )?),
        sandbox_policy_file: sandbox_policy_file.clone(),
        sandbox_policy_file_sha256: sha256(&read_operator_source_file(
            operator,
            &sandbox_policy_file,
        )?),
        artifact_root: value_string(&value, &["artifact_root"])?,
    }))
}

fn derive_learning_store(
    members: &[CapturedBackupMember],
) -> Result<BackupDomainPackLearningStoreState, BackupError> {
    let Some(index) = member_bytes(members, BackupEntryKind::DomainPackLearningIndex) else {
        return Ok(BackupDomainPackLearningStoreState::BeforeFirstCapture);
    };
    let value: serde_json::Value =
        serde_json::from_slice(index).map_err(|error| BackupError::Manifest {
            reason: format!("Domain Pack learning index parse failed: {error}"),
        })?;
    let records = value
        .get("records")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| BackupError::Manifest {
            reason: "Domain Pack learning index has no records array".to_owned(),
        })?;
    let mut projected = Vec::with_capacity(records.len());
    for record in records {
        projected.push(BackupDomainPackLearningRecord {
            candidate_id: value_string(record, &["candidate_id"])?,
            candidate_digest: normalize_digest(value_string(record, &["candidate_digest"])?)?,
            raw_sha256: normalize_digest(value_string(record, &["raw_sha256"])?)?,
            object_relative_path: value_string(record, &["object_relative_path"])?,
        });
    }
    projected.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    Ok(BackupDomainPackLearningStoreState::Captured { records: projected })
}

fn derive_isolation_store(
    members: &[CapturedBackupMember],
) -> Result<BackupIsolationStoreState, BackupError> {
    let mut contracts = Vec::new();
    for member in members_of_kind(members, BackupEntryKind::IsolationContract) {
        let value = parse_yaml_value(&member.bytes, "isolation contract")?;
        let relative_path = member
            .entry
            .logical_path
            .strip_prefix("sidecar/.forge-method/")
            .ok_or_else(|| BackupError::Manifest {
                reason: "isolation contract escaped state root".to_owned(),
            })?
            .to_owned();
        contracts.push(BackupIsolationContractProjection {
            isolation_id: value_string(&value, &["isolation_contract", "id"])?,
            agent_id: value_string(&value, &["isolation_contract", "agent_id"])?,
            contract_relative_path: relative_path,
            contract_sha256: member.entry.sha256.clone(),
        });
    }
    contracts.sort_by(|left, right| {
        left.contract_relative_path
            .cmp(&right.contract_relative_path)
    });
    if contracts.is_empty() {
        Ok(BackupIsolationStoreState::Empty)
    } else {
        Ok(BackupIsolationStoreState::Contracts { contracts })
    }
}

fn parse_yaml_value(raw: &[u8], label: &str) -> Result<serde_json::Value, BackupError> {
    yaml_serde::from_slice(raw).map_err(|error| BackupError::Manifest {
        reason: format!("{label} parse failed: {error}"),
    })
}

fn value_at<'a>(
    value: &'a serde_json::Value,
    path: &[&str],
) -> Result<&'a serde_json::Value, BackupError> {
    let mut current = value;
    for component in path {
        current = current
            .get(*component)
            .ok_or_else(|| BackupError::Manifest {
                reason: format!("required source field {} is absent", path.join(".")),
            })?;
    }
    Ok(current)
}

fn value_string(value: &serde_json::Value, path: &[&str]) -> Result<String, BackupError> {
    let value = value_at(value, path)?
        .as_str()
        .ok_or_else(|| BackupError::Manifest {
            reason: format!("required source field {} is not a string", path.join(".")),
        })?;
    if value.trim().is_empty() {
        return Err(BackupError::Manifest {
            reason: format!("required source field {} is blank", path.join(".")),
        });
    }
    Ok(value.to_owned())
}

fn value_u64(value: &serde_json::Value, path: &[&str]) -> Result<u64, BackupError> {
    value_at(value, path)?
        .as_u64()
        .ok_or_else(|| BackupError::Manifest {
            reason: format!(
                "required source field {} is not an unsigned integer",
                path.join(".")
            ),
        })
}

fn value_string_array(
    value: &serde_json::Value,
    path: &[&str],
) -> Result<Vec<String>, BackupError> {
    value_at(value, path)?
        .as_array()
        .ok_or_else(|| BackupError::Manifest {
            reason: format!("required source field {} is not an array", path.join(".")),
        })?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| BackupError::Manifest {
                    reason: format!(
                        "required source field {} contains a non-string or blank value",
                        path.join(".")
                    ),
                })
        })
        .collect()
}

fn normalize_digest(value: String) -> Result<String, BackupError> {
    if value.starts_with("sha256:") {
        return Ok(value);
    }
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(format!("sha256:{}", value.to_ascii_lowercase()));
    }
    Err(BackupError::Manifest {
        reason: "source digest is not a SHA-256 identity".to_owned(),
    })
}

fn manifest_error(error: impl fmt::Debug) -> BackupError {
    BackupError::Manifest {
        reason: format!("{error:?}"),
    }
}

/// Validate canonical archive names for a future destination before extraction.
/// No name is trimmed, folded, substituted, or renamed.
pub fn preflight_destination_names(
    entries: &[BackupEntry],
    platform: BackupDestinationPlatform,
) -> Result<(), BackupError> {
    let mut identities = BTreeSet::new();
    for entry in entries {
        let decoded = decode_canonical_archive_path(&entry.logical_path).map_err(|error| {
            BackupError::Manifest {
                reason: format!("cannot decode {}: {error:?}", entry.logical_path),
            }
        })?;
        let key = match platform {
            BackupDestinationPlatform::Posix => decoded.clone(),
            BackupDestinationPlatform::Windows => windows_destination_identity(&decoded)?,
        };
        if !identities.insert(key) {
            return Err(BackupError::InvalidPath {
                path: PathBuf::from(decoded),
                reason: "destination filesystem identity collision".to_owned(),
            });
        }
    }
    Ok(())
}

fn windows_destination_identity(path: &str) -> Result<String, BackupError> {
    let mut normalized = Vec::new();
    for component in path.split('/') {
        if component.is_empty()
            || component.ends_with([' ', '.'])
            || component.contains(['<', '>', ':', '"', '\\', '|', '?', '*'])
            || component.bytes().any(|byte| byte < 32)
        {
            return Err(BackupError::InvalidPath {
                path: PathBuf::from(path),
                reason: "name is not representable on Windows".to_owned(),
            });
        }
        let stem = component.split('.').next().unwrap_or_default();
        let upper = stem.to_ascii_uppercase();
        let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || upper.strip_prefix("COM").is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
            || upper.strip_prefix("LPT").is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            });
        if reserved {
            return Err(BackupError::InvalidPath {
                path: PathBuf::from(path),
                reason: "reserved Windows device name".to_owned(),
            });
        }
        normalized.push(component.to_lowercase());
    }
    Ok(normalized.join("/"))
}

/// Verify archive integrity against an independently protected receipt while
/// remaining inside the store trust boundary. Public callers must use
/// [`verify_project_backup`], which additionally compares current authorities.
fn verify_backup_archive(
    archive_path: impl AsRef<Path>,
    receipt_store: impl AsRef<Path>,
) -> Result<VerifiedBackupArchive, BackupError> {
    let archive_path = archive_path.as_ref();
    let receipt_store = validate_receipt_store(receipt_store.as_ref(), archive_path)?;
    let parsed = read_archive(archive_path)?;
    verify_parsed_archive(&parsed)?;
    let receipt_path = receipt_path_for_manifest(&parsed.manifest, &receipt_store)?;
    verify_archive_with_receipt(archive_path, &receipt_path)
}

pub(crate) fn verify_backup_archive_with_authority(
    archive_path: &Path,
    authority: &TrustedBackupAuthority,
) -> Result<VerifiedBackupArchive, BackupError> {
    verify_backup_archive(archive_path, &authority.receipt_store)
}

/// Resolve one authority from the machine-owned backup authority catalog.
///
/// The catalog location is fixed by the installation, not supplied by the
/// verification caller. The returned capability is opaque and contains no
/// secret material.
pub fn resolve_configured_backup_authority(
    authority_id: &str,
) -> Result<TrustedBackupAuthority, BackupError> {
    resolve_backup_authority_from_catalog(authority_id, &backup_authority_catalog_path())
}

#[cfg(unix)]
fn backup_authority_catalog_path() -> PathBuf {
    PathBuf::from("/etc/forge-core/backup-authorities.json")
}

#[cfg(windows)]
fn backup_authority_catalog_path() -> PathBuf {
    PathBuf::from(r"C:\ProgramData\Forge Method\backup-authorities.json")
}

#[cfg(not(any(unix, windows)))]
fn backup_authority_catalog_path() -> PathBuf {
    PathBuf::from("/etc/forge-core/backup-authorities.json")
}

fn resolve_backup_authority_from_catalog(
    authority_id: &str,
    catalog_path: &Path,
) -> Result<TrustedBackupAuthority, BackupError> {
    if authority_id.trim() != authority_id
        || authority_id.is_empty()
        || authority_id.len() > 256
        || !authority_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(BackupError::InvalidPath {
            path: catalog_path.to_path_buf(),
            reason: "backup authority id is invalid".to_owned(),
        });
    }
    if !catalog_path.is_absolute() {
        return Err(BackupError::InvalidPath {
            path: catalog_path.to_path_buf(),
            reason: "backup authority catalog path must be absolute".to_owned(),
        });
    }
    ensure_nofollow_regular_single_link(catalog_path)?;
    let bytes = read_file_bounded(catalog_path, MAX_AUTHORITY_CATALOG_BYTES)?;
    let catalog: BackupAuthorityCatalog =
        serde_json::from_slice(&bytes).map_err(|error| BackupError::Receipt {
            reason: format!("backup authority catalog parse failed: {error}"),
        })?;
    if catalog.schema_version != BACKUP_AUTHORITY_CATALOG_SCHEMA_VERSION {
        return Err(BackupError::Receipt {
            reason: "backup authority catalog schema is unsupported".to_owned(),
        });
    }
    let mut ids = BTreeSet::new();
    let mut selected = None;
    for authority in catalog.authorities {
        if !ids.insert(authority.authority_id.clone()) {
            return Err(BackupError::Receipt {
                reason: "backup authority catalog contains duplicate ids".to_owned(),
            });
        }
        if authority.authority_id == authority_id {
            selected = Some(authority);
        }
    }
    let selected = selected.ok_or_else(|| BackupError::Receipt {
        reason: format!("backup authority {authority_id:?} is not configured"),
    })?;
    if selected.protected_anchor_identity.trim().is_empty() {
        return Err(BackupError::Receipt {
            reason: "configured replay protected-anchor identity is blank".to_owned(),
        });
    }
    let receipt_store = canonical_configured_directory(&selected.receipt_store, "receipt store")?;
    if !selected.replay_anchor_path.is_absolute() {
        return Err(BackupError::InvalidPath {
            path: selected.replay_anchor_path,
            reason: "configured replay anchor path must be absolute".to_owned(),
        });
    }
    ensure_nofollow_regular_single_link(&selected.replay_anchor_path)?;
    let replay_anchor_path = fs::canonicalize(&selected.replay_anchor_path)
        .map_err(|source| io_error(&selected.replay_anchor_path, source))?;
    let domain_pack_operator = match (
        selected.domain_pack_operator_root,
        selected.domain_pack_operator_root_identity,
    ) {
        (None, None) => None,
        (Some(root), Some(root_identity)) if !root_identity.trim().is_empty() => {
            Some(ConfiguredDomainPackOperator {
                root: canonical_configured_directory(&root, "Domain Pack operator root")?,
                root_identity,
            })
        }
        _ => {
            return Err(BackupError::Receipt {
                reason: "Domain Pack operator root and identity must be configured together"
                    .to_owned(),
            });
        }
    };
    Ok(TrustedBackupAuthority {
        authority_id: selected.authority_id,
        receipt_store,
        replay_anchor_path,
        protected_anchor_identity: selected.protected_anchor_identity,
        domain_pack_operator,
    })
}

fn canonical_configured_directory(path: &Path, label: &str) -> Result<PathBuf, BackupError> {
    if !path.is_absolute() {
        return Err(BackupError::InvalidPath {
            path: path.to_path_buf(),
            reason: format!("configured {label} path must be absolute"),
        });
    }
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(BackupError::InvalidPath {
            path: path.to_path_buf(),
            reason: format!("configured {label} must be a real existing directory"),
        });
    }
    fs::canonicalize(path).map_err(|source| io_error(path, source))
}

/// Verify archive/receipt bindings against the selected project and independently
/// configured current monotonic authorities. This is the only public constructor
/// of [`VerifiedBackupArchive`].
pub fn verify_project_backup(
    request: &BackupVerifyRequest,
) -> Result<VerifiedBackupArchive, BackupError> {
    let authority = resolve_configured_backup_authority(&request.authority_id)?;
    verify_project_backup_with_authority(request, &authority)
}

pub(crate) fn verify_project_backup_with_authority(
    request: &BackupVerifyRequest,
    authority: &TrustedBackupAuthority,
) -> Result<VerifiedBackupArchive, BackupError> {
    let roots = resolve_backup_roots(&request.project_root)?;
    let quiescence = crate::producer_quiescence::quiesce_host_producers(
        &roots.state_root,
        &AtomicBool::new(false),
    )
    .map_err(|source| BackupError::Receipt {
        reason: format!("cannot quiesce current project for backup verification: {source}"),
    })?;
    let replay_anchor_lock =
        crate::replay_anchor::acquire_replay_anchor_retained_lock_under_boundary(
            &quiescence,
            &roots.state_root,
            &authority.replay_anchor_path,
        )
        .map_err(|source| BackupError::Receipt {
            reason: format!("cannot retain protected replay anchor for verification: {source}"),
        })?;
    verify_project_backup_under_retained_authority(
        request,
        authority,
        &quiescence,
        &replay_anchor_lock,
    )
}

fn verify_project_backup_under_retained_authority(
    request: &BackupVerifyRequest,
    authority: &TrustedBackupAuthority,
    quiescence: &HostQuiescenceGuard,
    replay_anchor_lock: &ReplayAnchorRetainedLock,
) -> Result<VerifiedBackupArchive, BackupError> {
    if request.authority_id != authority.authority_id {
        return Err(BackupError::Receipt {
            reason: "resolved backup authority does not match the requested authority".to_owned(),
        });
    }
    let project_root = fs::canonicalize(&request.project_root)
        .map_err(|source| io_error(&request.project_root, source))?;
    let archive = verify_backup_archive(&request.archive_path, &authority.receipt_store)?;
    let link_path = project_root.join(".forge-method.yaml");
    ensure_nofollow_regular_single_link(&link_path)?;
    let link_bytes = read_file_bounded(&link_path, MAX_RECEIPT_BYTES)?;
    let link: ProjectLinkDocument =
        yaml_serde::from_slice(&link_bytes).map_err(|error| BackupError::Manifest {
            reason: format!("selected Project Link parse failed: {error}"),
        })?;
    archive
        .manifest
        .verify_project_link_bytes(&link_bytes, &link)
        .map_err(archive_verification_error)?;
    let state_root = fs::canonicalize(project_root.join(&link.state_root.0))
        .map_err(|source| io_error(Path::new(&link.state_root.0), source))?;
    let replay = verify_replay_anchor_under_retained_lock(
        quiescence,
        &state_root,
        &authority.replay_anchor_path,
        replay_anchor_lock,
    )
    .map_err(|error| BackupError::Receipt {
        reason: format!("current replay authority rejected: {error}"),
    })?;
    if replay.status != ReplayAnchorStatus::Current || replay.anchor.head != replay.current_head {
        return Err(BackupError::Receipt {
            reason: "current replay WAL is not exactly anchored; protected anchor advance required"
                .to_owned(),
        });
    }
    let (anchor_recheck, anchor_bytes) = snapshot_replay_anchor_under_retained_lock(
        &state_root,
        &authority.replay_anchor_path,
        replay_anchor_lock,
    )
    .map_err(|error| BackupError::Receipt {
        reason: format!("cannot recheck protected replay anchor for verification: {error}"),
    })?
    .into_parts();
    if anchor_recheck != replay.anchor {
        return Err(BackupError::Receipt {
            reason: "protected replay anchor changed during backup verification".to_owned(),
        });
    }
    let expected_replay = &archive.receipt.backup_receipt.replay_monotonic_head;
    let current = &replay.anchor;
    if expected_replay.protected_anchor_identity != authority.protected_anchor_identity
        || expected_replay.schema_version != current.schema_version
        || expected_replay.deployment_id != current.deployment_id
        || expected_replay.epoch != current.epoch
        || expected_replay.generation != current.generation
        || expected_replay.previous_anchor_digest != current.previous_anchor_digest
        || expected_replay.anchor_document_sha256 != sha256(&anchor_bytes)
        || !receipt_replay_head_matches(expected_replay, &replay.current_head)
    {
        return Err(BackupError::Receipt {
            reason: "backup replay identity or head is stale or differs from current protected authority"
                .to_owned(),
        });
    }
    verify_domain_pack_authorities(&archive, authority.domain_pack_operator.as_ref())?;
    verify_optional_registry(
        request.current_principal_registry.as_deref(),
        archive
            .receipt
            .backup_receipt
            .archived_principal_registry_raw_sha256
            .as_deref(),
        "workflow principal registry",
    )?;
    verify_optional_broker_registry(
        request.current_broker_registry.as_deref(),
        archive
            .receipt
            .backup_receipt
            .archived_broker_registry_raw_sha256
            .as_deref(),
        &workflow_broker_audience(&link.project_id.0),
    )?;
    Ok(archive)
}

fn receipt_replay_head_matches(
    expected: &forge_core_contracts::BackupReplayRollbackAnchor,
    current: &ReplayWalHead,
) -> bool {
    expected.replay_wal_manifest_digest == current.manifest_digest
        && expected.replay_wal_prefix_digest == current.wal_prefix_digest
        && expected.replay_wal_last_seq == current.last_seq
        && expected.replay_wal_record_count == current.record_count as u64
        && expected.replay_wal_byte_length == current.byte_len
}

fn verify_domain_pack_authorities(
    archive: &VerifiedBackupArchive,
    operator: Option<&ConfiguredDomainPackOperator>,
) -> Result<(), BackupError> {
    let receipt = &archive.receipt.backup_receipt;
    if receipt.domain_pack_supply_chain.is_none() && receipt.domain_pack_reviewed_learning.is_none()
    {
        return Ok(());
    }
    let operator = operator.ok_or_else(|| BackupError::Receipt {
        reason: "provisioned Domain Pack backup has no configured operator authority".to_owned(),
    })?;
    if receipt
        .domain_pack_supply_chain
        .as_ref()
        .is_some_and(|expected| expected.operator_root_identity != operator.root_identity)
        || receipt
            .domain_pack_reviewed_learning
            .as_ref()
            .is_some_and(|expected| expected.operator_root_identity != operator.root_identity)
    {
        return Err(BackupError::Receipt {
            reason: "backup Domain Pack operator-root identity differs from configured authority"
                .to_owned(),
        });
    }
    if let Some(sources) = &archive
        .manifest
        .backup_manifest
        .source_state
        .domain_pack_operator_sources
    {
        if sources.operator_root_identity != operator.root_identity {
            return Err(BackupError::Receipt {
                reason:
                    "backup Domain Pack operator-source identity differs from configured authority"
                        .to_owned(),
            });
        }
        for (configured, expected, label) in [
            (
                sources.trust_policy_file.as_str(),
                sources.trust_policy_file_sha256.as_str(),
                "trust policy",
            ),
            (
                sources.registry_file.as_str(),
                sources.registry_file_sha256.as_str(),
                "supply-chain registry",
            ),
            (
                sources.reviewer_registry_file.as_str(),
                sources.reviewer_registry_file_sha256.as_str(),
                "reviewer registry",
            ),
            (
                sources.reviewed_registry_file.as_str(),
                sources.reviewed_registry_file_sha256.as_str(),
                "reviewed registry",
            ),
            (
                sources.capability_registry_file.as_str(),
                sources.capability_registry_file_sha256.as_str(),
                "capability registry",
            ),
            (
                sources.sandbox_policy_file.as_str(),
                sources.sandbox_policy_file_sha256.as_str(),
                "sandbox policy",
            ),
        ] {
            let raw = read_operator_source_file(operator, configured)?;
            if sha256(&raw) != expected {
                return Err(BackupError::Receipt {
                    reason: format!(
                        "backup Domain Pack {label} differs from current configured public authority"
                    ),
                });
            }
        }
    }
    let operator_root = &operator.root;
    if let Some(expected) = &receipt.domain_pack_supply_chain {
        let path = operator_root.join(".forge-domain-pack-registry-anchor.yaml");
        ensure_nofollow_regular_single_link(&path)?;
        let raw = read_file_bounded(&path, MAX_RECEIPT_BYTES)?;
        let current: DomainPackRegistryAnchorHead =
            yaml_serde::from_slice(&raw).map_err(|error| BackupError::Receipt {
                reason: format!("current Domain Pack registry anchor parse failed: {error}"),
            })?;
        if current.schema_version != expected.schema_version
            || current.registry_id.0 != expected.registry_id
            || current.audience.0 != expected.audience
            || current.generation != expected.generation
            || current.snapshot_digest != expected.registry_snapshot_digest
            || current.trust_policy_digest != expected.trust_policy_digest
            || sha256(&raw) != expected.anchor_document_sha256
        {
            return Err(BackupError::Receipt {
                reason: "backup Domain Pack supply-chain head is stale or substituted".to_owned(),
            });
        }
    }
    if let Some(expected) = &receipt.domain_pack_reviewed_learning {
        let path = operator_root.join(".forge-domain-pack-learning-anchor.yaml");
        ensure_nofollow_regular_single_link(&path)?;
        let raw = read_file_bounded(&path, MAX_RECEIPT_BYTES)?;
        let current: LearningAnchorHead =
            yaml_serde::from_slice(&raw).map_err(|error| BackupError::Receipt {
                reason: format!("current reviewed-learning anchor parse failed: {error}"),
            })?;
        if current.schema_version != expected.schema_version
            || current.reviewer.registry_id.0 != expected.reviewer_registry_id
            || current.reviewer.audience != expected.reviewer_audience
            || current.reviewer.generation != expected.reviewer_generation
            || current.reviewer.registry_digest != expected.reviewer_registry_digest
            || current.reviewed.registry_id.0 != expected.reviewed_registry_id
            || current.reviewed.audience != expected.reviewed_audience
            || current.reviewed.generation != expected.reviewed_generation
            || current.reviewed.registry_digest != expected.reviewed_registry_digest
            || sha256(&raw) != expected.anchor_document_sha256
        {
            return Err(BackupError::Receipt {
                reason: "backup Domain Pack reviewed-learning head is stale or substituted"
                    .to_owned(),
            });
        }
        if current.reviewer.full_digest.trim().is_empty()
            || current.reviewer.trust_policy_digest.trim().is_empty()
        {
            return Err(BackupError::Receipt {
                reason: "current reviewed-learning anchor has blank integrity fields".to_owned(),
            });
        }
    }
    Ok(())
}

pub(crate) fn verify_archive_current_non_state_authorities(
    archive: &VerifiedBackupArchive,
    authority: &TrustedBackupAuthority,
    current_principal_registry: Option<&Path>,
    current_broker_registry: Option<&Path>,
) -> Result<(), BackupError> {
    verify_domain_pack_authorities(archive, authority.domain_pack_operator.as_ref())?;
    verify_optional_registry(
        current_principal_registry,
        archive
            .receipt
            .backup_receipt
            .archived_principal_registry_raw_sha256
            .as_deref(),
        "workflow principal registry",
    )?;
    verify_optional_broker_registry(
        current_broker_registry,
        archive
            .receipt
            .backup_receipt
            .archived_broker_registry_raw_sha256
            .as_deref(),
        &workflow_broker_audience(
            &archive
                .manifest
                .backup_manifest
                .project
                .project_link
                .project_id
                .0,
        ),
    )
}

fn verify_optional_broker_registry(
    path: Option<&Path>,
    archived_sha256: Option<&str>,
    expected_audience: &str,
) -> Result<(), BackupError> {
    let Some(path) = path else {
        // Replacement-machine absence is legitimate and never synthesized from
        // archived public bytes.
        return Ok(());
    };
    let expected = archived_sha256.ok_or_else(|| BackupError::Receipt {
        reason: "current workflow broker registry supplied but backup did not archive one"
            .to_owned(),
    })?;
    ensure_nofollow_regular_single_link(path)?;
    let raw = read_file_bounded(path, MAX_RECEIPT_BYTES)?;
    validate_public_workflow_broker_registry(&raw, expected_audience)?;
    if sha256(&raw) != expected {
        return Err(BackupError::Receipt {
            reason: "current workflow broker registry raw bytes differ from protected receipt"
                .to_owned(),
        });
    }
    Ok(())
}

fn verify_optional_registry(
    path: Option<&Path>,
    archived_sha256: Option<&str>,
    label: &str,
) -> Result<(), BackupError> {
    let Some(path) = path else {
        // Replacement-machine absence is legitimate and never synthesized from
        // archived public bytes.
        return Ok(());
    };
    let expected = archived_sha256.ok_or_else(|| BackupError::Receipt {
        reason: format!("current {label} supplied but backup did not archive one"),
    })?;
    ensure_nofollow_regular_single_link(path)?;
    let raw = read_file_bounded(path, MAX_RECEIPT_BYTES)?;
    if sha256(&raw) != expected {
        return Err(BackupError::Receipt {
            reason: format!("current {label} raw bytes differ from protected receipt"),
        });
    }
    Ok(())
}
/// Strictly verify an immutable archive against a protected receipt loaded from
/// storage outside the archive. Current monotonic authority comparison is done
/// by the aggregate producer layer before this capability is returned publicly.
pub(crate) fn verify_archive_with_receipt(
    archive_path: &Path,
    receipt_path: &Path,
) -> Result<VerifiedBackupArchive, BackupError> {
    ensure_nofollow_regular_single_link(archive_path)?;
    ensure_nofollow_regular_single_link(receipt_path)?;
    let parsed = read_archive(archive_path)?;
    let archive_sha256 = parsed.archive_sha256.clone();
    let receipt_bytes = read_file_bounded(receipt_path, MAX_RECEIPT_BYTES)?;
    let receipt: BackupReceiptDocument =
        serde_json::from_slice(&receipt_bytes).map_err(|error| BackupError::Receipt {
            reason: format!("strict receipt parse failed: {error}"),
        })?;
    receipt
        .validate_against(&parsed.manifest)
        .map_err(receipt_validation_error)?;
    if receipt.backup_receipt.archive_sha256 != archive_sha256 {
        return Err(BackupError::Receipt {
            reason: "archive digest does not match protected receipt".to_owned(),
        });
    }
    verify_parsed_archive(&parsed)?;
    Ok(VerifiedBackupArchive {
        manifest: parsed.manifest,
        receipt,
        archive_sha256,
        archive_path: fs::canonicalize(archive_path)
            .map_err(|source| io_error(archive_path, source))?,
        member_count: parsed.members.len(),
    })
}

pub(crate) fn verified_archive_members(
    verified: &VerifiedBackupArchive,
) -> Result<Vec<(BackupEntry, Vec<u8>)>, BackupError> {
    let parsed = read_archive(&verified.archive_path)?;
    verify_parsed_archive(&parsed)?;
    if parsed.archive_sha256 != verified.archive_sha256
        || parsed.manifest != verified.manifest
        || parsed.members.len() != verified.member_count
    {
        return Err(BackupError::Archive {
            reason: "verified archive changed before restore extraction".to_owned(),
        });
    }
    Ok(parsed
        .members
        .into_iter()
        .map(|member| (member.entry, member.bytes))
        .collect())
}

/// Publish a fully captured source snapshot using immutable create-new staging,
/// file fsync, staged self-verification, atomic rename, and parent fsync. The
/// protected receipt is published only after the final archive is durable.
pub(crate) fn publish_captured_snapshot(
    snapshot: &CapturedBackupSnapshot,
    archive_path: &Path,
    receipt_store: &Path,
) -> Result<BackupPublication, BackupError> {
    validate_captured_snapshot(snapshot)?;
    let archive_parent = archive_path
        .parent()
        .ok_or_else(|| BackupError::InvalidPath {
            path: archive_path.to_path_buf(),
            reason: "archive has no parent".to_owned(),
        })?;
    fs::create_dir_all(archive_parent).map_err(|source| io_error(archive_parent, source))?;
    let receipt_store = validate_receipt_store(receipt_store, archive_path)?;
    let receipt_path = receipt_path_for(snapshot, &receipt_store)?;

    if archive_path
        .try_exists()
        .map_err(|source| io_error(archive_path, source))?
    {
        if !receipt_path
            .try_exists()
            .map_err(|source| io_error(&receipt_path, source))?
        {
            return complete_orphaned_archive(snapshot, archive_path, &receipt_path);
        }
        let verified = verify_archive_with_receipt(archive_path, &receipt_path)?;
        if verified.manifest != snapshot.manifest {
            return Err(BackupError::ExistingDifferent {
                path: archive_path.to_path_buf(),
            });
        }
        return Ok(publication_from_verified(verified, receipt_path, true));
    }
    if receipt_path
        .try_exists()
        .map_err(|source| io_error(&receipt_path, source))?
    {
        return Err(BackupError::ExistingDifferent { path: receipt_path });
    }

    let staging = unique_staging_path(archive_path)?;
    let result = (|| {
        write_archive_create_new(snapshot, &staging)?;
        ensure_nofollow_regular_single_link(&staging)?;
        let parsed = read_archive(&staging)?;
        verify_parsed_archive(&parsed)?;
        if parsed.manifest != snapshot.manifest {
            return Err(BackupError::Archive {
                reason: "staged manifest changed".to_owned(),
            });
        }
        rename_create_new(&staging, archive_path)?;
        sync_parent(archive_path)?;
        let archive_sha256 = hash_file_bounded(archive_path, MAX_ARCHIVE_BYTES)?;
        let receipt = build_receipt(&snapshot.manifest, &archive_sha256)?;
        publish_receipt(&receipt, &receipt_path)?;
        let verified = verify_archive_with_receipt(archive_path, &receipt_path)?;
        Ok(publication_from_verified(
            verified,
            receipt_path.clone(),
            false,
        ))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}

fn complete_orphaned_archive(
    snapshot: &CapturedBackupSnapshot,
    archive_path: &Path,
    receipt_path: &Path,
) -> Result<BackupPublication, BackupError> {
    let parsed = read_archive(archive_path)?;
    verify_parsed_archive(&parsed)?;
    if parsed.manifest != snapshot.manifest {
        return Err(BackupError::ExistingDifferent {
            path: archive_path.to_path_buf(),
        });
    }
    let archive_sha256 = hash_file_bounded(archive_path, MAX_ARCHIVE_BYTES)?;
    let receipt = build_receipt(&snapshot.manifest, &archive_sha256)?;
    publish_receipt(&receipt, receipt_path)?;
    let verified = verify_archive_with_receipt(archive_path, receipt_path)?;
    Ok(publication_from_verified(
        verified,
        receipt_path.to_path_buf(),
        true,
    ))
}

fn publication_from_verified(
    verified: VerifiedBackupArchive,
    receipt_path: PathBuf,
    already_published: bool,
) -> BackupPublication {
    BackupPublication {
        archive_path: verified.archive_path,
        archive_sha256: verified.archive_sha256,
        receipt_path,
        receipt_digest: verified.receipt.backup_receipt.receipt_digest,
        manifest_set_digest: verified.manifest.backup_manifest.manifest_set_digest,
        member_count: verified.member_count,
        already_published,
    }
}

fn validate_captured_snapshot(snapshot: &CapturedBackupSnapshot) -> Result<(), BackupError> {
    snapshot
        .manifest
        .validate_integrity()
        .map_err(|error| BackupError::Manifest {
            reason: format!("integrity validation failed: {error:?}"),
        })?;
    let observed = snapshot
        .members
        .iter()
        .map(|member| member.entry.clone())
        .collect::<Vec<_>>();
    snapshot
        .manifest
        .verify_archive_entries(&observed)
        .map_err(archive_verification_error)?;
    let expected_broker_audience = workflow_broker_audience(
        &snapshot
            .manifest
            .backup_manifest
            .project
            .project_link
            .project_id
            .0,
    );
    for member in &snapshot.members {
        if member.entry.byte_length != member.bytes.len() as u64
            || member.entry.sha256 != sha256(&member.bytes)
        {
            return Err(BackupError::Archive {
                reason: format!("captured bytes changed for {}", member.entry.logical_path),
            });
        }
        if member.entry.material == BackupEntryKind::PublicBrokerRegistry {
            validate_public_workflow_broker_registry(&member.bytes, &expected_broker_audience)?;
        }
    }
    preflight_destination_names(
        &snapshot.manifest.backup_manifest.entries,
        BackupDestinationPlatform::Posix,
    )
}

struct ParsedArchive {
    manifest: BackupManifestDocument,
    members: Vec<CapturedBackupMember>,
    archive_sha256: String,
}

fn verify_parsed_archive(parsed: &ParsedArchive) -> Result<(), BackupError> {
    parsed
        .manifest
        .validate_integrity()
        .map_err(|error| BackupError::Manifest {
            reason: format!("integrity validation failed: {error:?}"),
        })?;
    let entries = parsed
        .members
        .iter()
        .map(|member| member.entry.clone())
        .collect::<Vec<_>>();
    parsed
        .manifest
        .verify_archive_entries(&entries)
        .map_err(archive_verification_error)?;
    let project_entry = parsed
        .members
        .iter()
        .find(|member| member.entry.logical_path == "project/.forge-method.yaml")
        .ok_or_else(|| BackupError::Archive {
            reason: "Project Link member is missing".to_owned(),
        })?;
    let project_link: ProjectLinkDocument =
        yaml_serde::from_slice(&project_entry.bytes).map_err(|error| BackupError::Archive {
            reason: format!("Project Link parse failed: {error}"),
        })?;
    parsed
        .manifest
        .verify_project_link_bytes(&project_entry.bytes, &project_link)
        .map_err(archive_verification_error)?;
    if let Some(broker_registry) = parsed
        .members
        .iter()
        .find(|member| member.entry.material == BackupEntryKind::PublicBrokerRegistry)
    {
        validate_public_workflow_broker_registry(
            &broker_registry.bytes,
            &workflow_broker_audience(&project_link.project_id.0),
        )?;
    }
    Ok(())
}

fn write_archive_create_new(
    snapshot: &CapturedBackupSnapshot,
    staging: &Path,
) -> Result<(), BackupError> {
    let manifest_bytes =
        serde_json::to_vec(&snapshot.manifest).map_err(|error| BackupError::Manifest {
            reason: format!("serialize failed: {error}"),
        })?;
    if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(BackupError::ResourceLimit {
            resource: "manifest bytes",
            maximum: MAX_MANIFEST_BYTES,
        });
    }
    let file = open_private_create_new(staging)?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(ARCHIVE_MAGIC)
        .map_err(|source| io_error(staging, source))?;
    write_u64(&mut writer, manifest_bytes.len() as u64, staging)?;
    writer
        .write_all(&manifest_bytes)
        .map_err(|source| io_error(staging, source))?;
    write_u64(&mut writer, snapshot.members.len() as u64, staging)?;
    for member in &snapshot.members {
        let name = member.entry.logical_path.as_bytes();
        write_u64(&mut writer, name.len() as u64, staging)?;
        write_u64(&mut writer, member.bytes.len() as u64, staging)?;
        writer
            .write_all(name)
            .map_err(|source| io_error(staging, source))?;
        writer
            .write_all(&member.bytes)
            .map_err(|source| io_error(staging, source))?;
    }
    writer.flush().map_err(|source| io_error(staging, source))?;
    let file = writer
        .into_inner()
        .map_err(|error| io_error(staging, error.into_error()))?;
    file.sync_all().map_err(|source| io_error(staging, source))
}

fn read_archive(path: &Path) -> Result<ParsedArchive, BackupError> {
    let archive_bytes = read_file_bounded(path, MAX_ARCHIVE_BYTES)?;
    let archive_sha256 = sha256(&archive_bytes);
    let mut reader = Cursor::new(archive_bytes);
    let mut magic = [0_u8; 16];
    reader
        .read_exact(&mut magic)
        .map_err(|source| io_error(path, source))?;
    if &magic != ARCHIVE_MAGIC {
        return Err(BackupError::Archive {
            reason: "unsupported archive magic/version".to_owned(),
        });
    }
    let manifest_len = read_bounded_u64(&mut reader, path, "manifest bytes", MAX_MANIFEST_BYTES)?;
    let mut manifest_bytes = vec![0_u8; usize_from_u64(manifest_len, "manifest bytes")?];
    reader
        .read_exact(&mut manifest_bytes)
        .map_err(|source| io_error(path, source))?;
    let manifest: BackupManifestDocument =
        serde_json::from_slice(&manifest_bytes).map_err(|error| BackupError::Manifest {
            reason: format!("strict parse failed: {error}"),
        })?;
    let count = read_bounded_u64(&mut reader, path, "member count", MAX_MEMBER_COUNT)?;
    let expected = manifest
        .backup_manifest
        .entries
        .iter()
        .map(|entry| (entry.logical_path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut names = BTreeSet::new();
    let mut members = Vec::with_capacity(usize_from_u64(count, "member count")?);
    for _ in 0..count {
        let name_len = read_bounded_u64(
            &mut reader,
            path,
            "member name bytes",
            MAX_MEMBER_NAME_BYTES,
        )?;
        let byte_len = read_bounded_u64(&mut reader, path, "member bytes", MAX_MEMBER_BYTES)?;
        let mut name = vec![0_u8; usize_from_u64(name_len, "member name bytes")?];
        reader
            .read_exact(&mut name)
            .map_err(|source| io_error(path, source))?;
        let name = String::from_utf8(name).map_err(|_| BackupError::Archive {
            reason: "member name is not UTF-8".to_owned(),
        })?;
        let decoded =
            decode_canonical_archive_path(&name).map_err(|error| BackupError::Archive {
                reason: format!("noncanonical member {name}: {error:?}"),
            })?;
        if decoded.is_empty() || !names.insert(name.clone()) {
            return Err(BackupError::Archive {
                reason: format!("duplicate or empty member {name}"),
            });
        }
        let mut bytes = vec![0_u8; usize_from_u64(byte_len, "member bytes")?];
        reader
            .read_exact(&mut bytes)
            .map_err(|source| io_error(path, source))?;
        let expected_entry = expected
            .get(name.as_str())
            .ok_or_else(|| BackupError::Archive {
                reason: format!("extra member {name}"),
            })?;
        members.push(CapturedBackupMember {
            entry: BackupEntry {
                material: expected_entry.material,
                logical_path: name,
                entry_type: BackupArchiveEntryType::RegularFile,
                byte_length: byte_len,
                sha256: sha256(&bytes),
            },
            bytes,
        });
    }
    let mut trailing = [0_u8; 1];
    if reader
        .read(&mut trailing)
        .map_err(|source| io_error(path, source))?
        != 0
    {
        return Err(BackupError::Archive {
            reason: "trailing bytes after final member".to_owned(),
        });
    }
    Ok(ParsedArchive {
        manifest,
        members,
        archive_sha256,
    })
}

fn build_receipt(
    manifest: &BackupManifestDocument,
    archive_sha256: &str,
) -> Result<BackupReceiptDocument, BackupError> {
    let backup = &manifest.backup_manifest;
    let observations = &backup.external_authority_observations;
    let created_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| BackupError::Receipt {
            reason: format!("system clock before epoch: {error}"),
        })?
        .as_secs();
    let mut document = BackupReceiptDocument {
        schema_version: BACKUP_RECEIPT_SCHEMA_VERSION.to_owned(),
        backup_receipt: BackupReceipt {
            archive_sha256: archive_sha256.to_owned(),
            manifest_set_digest: backup.manifest_set_digest.clone(),
            project_id: backup.project.project_link.project_id.clone(),
            project_link_sha256: backup.project.project_link_sha256.clone(),
            workflow_release: backup.workflow_release.clone(),
            effective_epoch: backup.effective_epoch.clone(),
            replay_monotonic_head: observations.replay_rollback_anchor.clone(),
            domain_pack_supply_chain: observations.domain_pack_supply_chain.clone(),
            domain_pack_reviewed_learning: observations.domain_pack_reviewed_learning.clone(),
            archived_principal_registry_raw_sha256: observations
                .workflow_principal_registry
                .as_ref()
                .map(|registry| registry.registry_sha256.clone()),
            archived_broker_registry_raw_sha256: observations
                .workflow_broker_registry
                .as_ref()
                .map(|registry| registry.registry_sha256.clone()),
            created_at_unix,
            receipt_digest:
                "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        },
    };
    document.backup_receipt.receipt_digest = document.digest().map_err(receipt_validation_error)?;
    document
        .validate_against(manifest)
        .map_err(receipt_validation_error)?;
    Ok(document)
}

fn publish_receipt(document: &BackupReceiptDocument, path: &Path) -> Result<(), BackupError> {
    let bytes = serde_json::to_vec(document).map_err(|error| BackupError::Receipt {
        reason: format!("serialize failed: {error}"),
    })?;
    let temporary = unique_staging_path(path)?;
    let result = (|| {
        let mut file = open_private_create_new(&temporary)?;
        file.write_all(&bytes)
            .map_err(|source| io_error(&temporary, source))?;
        file.sync_all()
            .map_err(|source| io_error(&temporary, source))?;
        rename_create_new(&temporary, path)?;
        sync_parent(path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn validate_receipt_store(store: &Path, archive_path: &Path) -> Result<PathBuf, BackupError> {
    let store = fs::canonicalize(store).map_err(|source| io_error(store, source))?;
    let metadata = fs::symlink_metadata(&store).map_err(|source| io_error(&store, source))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(BackupError::InvalidPath {
            path: store,
            reason: "receipt store must be a real existing directory".to_owned(),
        });
    }
    let archive_parent = archive_path
        .parent()
        .ok_or_else(|| BackupError::InvalidPath {
            path: archive_path.to_path_buf(),
            reason: "archive has no parent".to_owned(),
        })?;
    let archive_parent = canonicalize_existing_ancestor(archive_parent)?;
    if store.starts_with(&archive_parent) || archive_parent.starts_with(&store) {
        return Err(BackupError::InvalidPath {
            path: store,
            reason: "protected receipt store must be disjoint from archive and staging".to_owned(),
        });
    }
    Ok(store)
}

fn receipt_path_for(
    snapshot: &CapturedBackupSnapshot,
    store: &Path,
) -> Result<PathBuf, BackupError> {
    receipt_path_for_manifest(&snapshot.manifest, store)
}

fn receipt_path_for_manifest(
    manifest: &BackupManifestDocument,
    store: &Path,
) -> Result<PathBuf, BackupError> {
    let digest = manifest
        .backup_manifest
        .manifest_set_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| BackupError::Manifest {
            reason: "set digest lacks sha256 prefix".to_owned(),
        })?;
    Ok(store.join(format!("{digest}.receipt.json")))
}

fn unique_staging_path(path: &Path) -> Result<PathBuf, BackupError> {
    let parent = path.parent().ok_or_else(|| BackupError::InvalidPath {
        path: path.to_path_buf(),
        reason: "path has no parent".to_owned(),
    })?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| BackupError::InvalidPath {
            path: path.to_path_buf(),
            reason: "path name must be UTF-8".to_owned(),
        })?;
    let mut nonce = [0_u8; 16];
    getrandom::fill(&mut nonce).map_err(|error| BackupError::Archive {
        reason: format!("staging nonce generation failed: {error}"),
    })?;
    Ok(parent.join(format!(".{name}.{}.forge-backup-staging", hex(&nonce))))
}

fn rename_create_new(source: &Path, destination: &Path) -> Result<(), BackupError> {
    // `rename` may overwrite on Unix. Linking the same-filesystem staging inode
    // is the portable create-new publication primitive: an existing destination
    // makes `hard_link` fail atomically. The staging name is then removed before
    // the final single-link invariant is accepted.
    fs::hard_link(source, destination).map_err(|source_error| {
        if destination.exists() {
            BackupError::ExistingDifferent {
                path: destination.to_path_buf(),
            }
        } else {
            io_error(destination, source_error)
        }
    })?;
    if let Err(source_error) = fs::remove_file(source) {
        let _ = fs::remove_file(destination);
        return Err(io_error(source, source_error));
    }
    ensure_nofollow_regular_single_link(destination)
}

fn ensure_nofollow_regular_single_link(path: &Path) -> Result<(), BackupError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(BackupError::UnsafeFileType {
            path: path.to_path_buf(),
            reason: "must be a no-follow regular file".to_owned(),
        });
    }
    if hard_link_count(&metadata) != 1 {
        return Err(BackupError::UnsafeFileType {
            path: path.to_path_buf(),
            reason: "hard-link count must be exactly one".to_owned(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn hard_link_count(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    metadata.nlink()
}

#[cfg(windows)]
fn hard_link_count(metadata: &fs::Metadata) -> u64 {
    use std::os::windows::fs::MetadataExt as _;
    metadata.number_of_links().unwrap_or(0)
}

#[cfg(not(any(unix, windows)))]
fn hard_link_count(_metadata: &fs::Metadata) -> u64 {
    0
}

#[cfg(unix)]
fn open_nofollow_read(path: &Path) -> Result<File, BackupError> {
    use rustix::fs::{open, Mode, OFlags};

    let fd = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|source| io_error(path, io::Error::from(source)))?;
    let file = File::from(fd);
    validate_opened_regular_single_link(&file, path)?;
    Ok(file)
}

#[cfg(windows)]
fn open_nofollow_read(path: &Path) -> Result<File, BackupError> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|source| io_error(path, source))?;
    validate_opened_regular_single_link(&file, path)?;
    Ok(file)
}

#[cfg(not(any(unix, windows)))]
fn open_nofollow_read(path: &Path) -> Result<File, BackupError> {
    ensure_nofollow_regular_single_link(path)?;
    let file = File::open(path).map_err(|source| io_error(path, source))?;
    validate_opened_regular_single_link(&file, path)?;
    Ok(file)
}

fn validate_opened_regular_single_link(file: &File, path: &Path) -> Result<(), BackupError> {
    let metadata = file.metadata().map_err(|source| io_error(path, source))?;
    if !metadata.file_type().is_file() || hard_link_count(&metadata) != 1 {
        return Err(BackupError::UnsafeFileType {
            path: path.to_path_buf(),
            reason: "opened source must be one no-follow single-link regular file".to_owned(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn open_private_create_new(path: &Path) -> Result<File, BackupError> {
    use std::os::unix::fs::OpenOptionsExt as _;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|source| io_error(path, source))
}

#[cfg(not(unix))]
fn open_private_create_new(path: &Path) -> Result<File, BackupError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| io_error(path, source))
}

pub(crate) fn read_file_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, BackupError> {
    let mut file = open_nofollow_read(path)?;
    let metadata = file.metadata().map_err(|source| io_error(path, source))?;
    if metadata.len() > maximum {
        return Err(BackupError::ResourceLimit {
            resource: "file bytes",
            maximum,
        });
    }
    let capacity = usize_from_u64(metadata.len(), "file bytes")?;
    let mut bytes = Vec::with_capacity(capacity);
    (&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| io_error(path, source))?;
    if bytes.len() as u64 > maximum {
        return Err(BackupError::ResourceLimit {
            resource: "file bytes",
            maximum,
        });
    }
    let after = file.metadata().map_err(|source| io_error(path, source))?;
    if !same_file_identity(&metadata, &after) || after.len() != bytes.len() as u64 {
        return Err(BackupError::Archive {
            reason: format!("file changed while reading {}", path.display()),
        });
    }
    Ok(bytes)
}

fn hash_file_bounded(path: &Path, maximum: u64) -> Result<String, BackupError> {
    let bytes = read_file_bounded(path, maximum)?;
    Ok(sha256(&bytes))
}

fn read_bounded_u64<R: Read>(
    reader: &mut R,
    path: &Path,
    resource: &'static str,
    maximum: u64,
) -> Result<u64, BackupError> {
    let mut bytes = [0_u8; 8];
    reader
        .read_exact(&mut bytes)
        .map_err(|source| io_error(path, source))?;
    let value = u64::from_be_bytes(bytes);
    if value > maximum {
        Err(BackupError::ResourceLimit { resource, maximum })
    } else {
        Ok(value)
    }
}

fn write_u64<W: Write>(writer: &mut W, value: u64, path: &Path) -> Result<(), BackupError> {
    writer
        .write_all(&value.to_be_bytes())
        .map_err(|source| io_error(path, source))
}

fn usize_from_u64(value: u64, resource: &'static str) -> Result<usize, BackupError> {
    usize::try_from(value).map_err(|_| BackupError::ResourceLimit {
        resource,
        maximum: usize::MAX as u64,
    })
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn receipt_validation_error(error: BackupReceiptValidationError) -> BackupError {
    BackupError::Receipt {
        reason: format!("binding validation failed: {error:?}"),
    }
}

fn archive_verification_error(error: BackupArchiveVerificationError) -> BackupError {
    BackupError::Archive {
        reason: format!("member verification failed: {error:?}"),
    }
}

fn normalize_future_output_path(path: &Path) -> Result<PathBuf, BackupError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|source| io_error(path, source))?
            .join(path)
    };
    let lexical = lexically_normalize_absolute(&absolute)?;
    let mut ancestor = lexical.as_path();
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(BackupError::InvalidPath {
                        path: ancestor.to_path_buf(),
                        reason: "backup output ancestor is a link or reparse point".to_owned(),
                    });
                }
                if !suffix.is_empty() && !metadata.is_dir() {
                    return Err(BackupError::InvalidPath {
                        path: ancestor.to_path_buf(),
                        reason: "backup output has a non-directory ancestor".to_owned(),
                    });
                }
                let mut resolved =
                    fs::canonicalize(ancestor).map_err(|source| io_error(ancestor, source))?;
                for component in suffix.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let leaf = ancestor
                    .file_name()
                    .ok_or_else(|| BackupError::InvalidPath {
                        path: lexical.clone(),
                        reason: "backup output has no existing ancestor".to_owned(),
                    })?;
                suffix.push(leaf.to_os_string());
                ancestor = ancestor.parent().ok_or_else(|| BackupError::InvalidPath {
                    path: lexical.clone(),
                    reason: "backup output has no existing ancestor".to_owned(),
                })?;
            }
            Err(error) => return Err(io_error(ancestor, error)),
        }
    }
}

fn lexically_normalize_absolute(path: &Path) -> Result<PathBuf, BackupError> {
    if !path.is_absolute() {
        return Err(BackupError::InvalidPath {
            path: path.to_path_buf(),
            reason: "backup output must resolve from an absolute path".to_owned(),
        });
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(value) => normalized.push(value.as_os_str()),
            Component::RootDir => normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(BackupError::InvalidPath {
                        path: path.to_path_buf(),
                        reason: "backup output escapes the filesystem root".to_owned(),
                    });
                }
            }
        }
    }
    Ok(normalized)
}

fn canonicalize_existing_ancestor(path: &Path) -> Result<PathBuf, BackupError> {
    let normalized = normalize_future_output_path(path)?;
    if normalized.exists() {
        return fs::canonicalize(&normalized).map_err(|source| io_error(&normalized, source));
    }
    let mut candidate = normalized.as_path();
    loop {
        match fs::canonicalize(candidate) {
            Ok(value) => return Ok(value),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                candidate = candidate.parent().ok_or_else(|| BackupError::InvalidPath {
                    path: normalized.clone(),
                    reason: "no existing ancestor".to_owned(),
                })?;
            }
            Err(error) => return Err(io_error(candidate, error)),
        }
    }
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

fn sync_parent(path: &Path) -> Result<(), BackupError> {
    let parent = path.parent().ok_or_else(|| BackupError::InvalidPath {
        path: path.to_path_buf(),
        reason: "path has no parent".to_owned(),
    })?;
    sync_directory(parent).map_err(|source| io_error(parent, source))
}

fn io_error(path: &Path, source: io::Error) -> BackupError {
    BackupError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
const NON_DECOMPRESSIBLE_ED25519_PUBLIC_KEY_HEX: &str =
    "0200000000000000000000000000000000000000000000000000000000000000";
#[cfg(test)]
const ALL_FF_ED25519_PUBLIC_KEY_HEX: &str =
    "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

#[cfg(test)]
fn adversarial_broker_registry(public_key_hex: &str) -> String {
    format!(
        r#"schema_version: "0.1"
audience: "forge-core:workflow:project.test"
issuers:
  - issuer_id: "broker.test"
    profile: "human"
    public_key_hex: "{public_key_hex}"
    status: "active"
    enrollment:
      ceremony_ref: "operator://ceremony/test"
      ceremony_digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
      declared_at_unix: 1
"#,
    )
}

#[cfg(test)]
#[test]
fn public_broker_registry_rejects_non_decompressible_ed25519_key_encoding() {
    let raw = adversarial_broker_registry(NON_DECOMPRESSIBLE_ED25519_PUBLIC_KEY_HEX);
    assert!(validate_public_workflow_broker_registry(
        raw.as_bytes(),
        "forge-core:workflow:project.test",
    )
    .is_err());
}

#[cfg(test)]
#[test]
fn public_broker_registry_preserves_dalek_all_ff_key_semantics() {
    let raw = adversarial_broker_registry(ALL_FF_ED25519_PUBLIC_KEY_HEX);
    assert!(validate_public_workflow_broker_registry(
        raw.as_bytes(),
        "forge-core:workflow:project.test",
    )
    .is_ok());
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;
