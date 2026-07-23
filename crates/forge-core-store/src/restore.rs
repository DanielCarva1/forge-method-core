//! Verified project-bound restore planning, preflight, atomic publication, and receipts.

use crate::backup::{
    preflight_destination_names, read_file_bounded, resolve_configured_backup_authority,
    verified_archive_members, verify_archive_current_non_state_authorities,
    verify_backup_archive_with_authority, BackupDestinationPlatform, BackupError,
    TrustedBackupAuthority, VerifiedBackupArchive,
};
use crate::producer_quiescence::{quiesce_host_producers, HostQuiescenceGuard};
use crate::replay_anchor::ReplayAnchorDocument;
use forge_core_contracts::{
    BackupEntry, BackupEntryKind, BackupManifestDocument, BackupReplayRollbackAnchor,
    BackupSourceExclusion, ProjectLinkDocument, WorkflowEffectiveBundleIdentity,
    WorkflowGovernanceReleaseIdentity,
};
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

const RESTORE_RECEIPT_SCHEMA_VERSION: &str = "forge_project_state_restore_receipt_v3";
const RESTORE_JOURNAL_SCHEMA_VERSION: &str = "forge_project_state_restore_journal_v2";
const RESTORE_COMPLETION_AUTHORITY_SCHEMA_VERSION: &str =
    "forge_project_state_restore_completion_authority_v2";
const RESTORE_COMPLETION_SELECTOR_SCHEMA_VERSION: &str =
    "forge_project_state_restore_completion_selector_v1";
const RESTORE_RECEIPT_DIGEST_DOMAIN: &[u8] = b"forge-method:project-state-restore-receipt:v1\0";
const RESTORE_COMPLETION_INVENTORY_DIGEST_DOMAIN: &[u8] =
    b"forge-method:project-state-restore-completion-inventory:v2\0";
const RESTORE_PATH_DIGEST_DOMAIN: &[u8] =
    b"forge-method:project-state-restore-configured-path:v1\0";
const MAX_RESTORE_AUTHORITY_BYTES: u64 = 1024 * 1024;
const MAX_RESTORE_RECEIPT_BYTES: u64 = 1024 * 1024;
const MAX_RESTORE_COMPLETION_AUTHORITY_BYTES: u64 = 512 * 1024 * 1024;
const MAX_RESTORE_MEMBERS: usize = 100_000;
const PROJECT_LINK_LEAF: &str = ".forge-method.yaml";
const DESTINATION_STATE_LEAF: &str = ".forge-method";

#[derive(Debug, Clone)]
pub struct RestorePlanRequest {
    pub project_root: PathBuf,
    pub archive_path: PathBuf,
    pub authority_id: String,
    pub destination_platform: BackupDestinationPlatform,
    pub current_principal_registry: Option<PathBuf>,
    pub current_broker_registry: Option<PathBuf>,
}

/// Opaque, non-serializable proof that archive, protected backup receipt,
/// Project Link, current monotonic authority, and destination identity agree.
pub struct RestorePlan {
    request: RestorePlanRequest,
    authority: TrustedBackupAuthority,
    verified: VerifiedBackupArchive,
    project_root: PathBuf,
    project_root_retained: RestoreRetainedDirectory,
    project_link_path: PathBuf,
    project_link_leaf: PathBuf,
    project_link_file: File,
    project_link_identity: crate::retained_dir::RetainedFileIdentity,
    project_link_bytes: Vec<u8>,
    project_link: ProjectLinkDocument,
    destination_sidecar: PathBuf,
    destination_state: PathBuf,
}

impl fmt::Debug for RestorePlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RestorePlan")
            .field("archive", &self.verified.archive_path())
            .field("project_root", &self.project_root)
            .field("destination_sidecar", &self.destination_sidecar)
            .finish_non_exhaustive()
    }
}

/// Opaque preflight result retaining the configured protected authority root
/// and exact-root host quiescence whenever destination state already exists.
/// A caller cannot construct success.
pub struct RestorePreflight {
    plan: RestorePlan,
    members: Vec<RestoreMember>,
    destination_status: DestinationStatus,
    staging_path: PathBuf,
    staging_parent: RestoreRetainedDirectory,
    staging_leaf: PathBuf,
    destination_leaf: PathBuf,
    authority_root: RestoreRetainedDirectory,
    journal_relative: PathBuf,
    journal_path: PathBuf,
    receipt_relative: PathBuf,
    receipt_path: PathBuf,
    completion_directory_relative: PathBuf,
    completion_directory_path: PathBuf,
    completion_anchor_directory_relative: PathBuf,
    completion_anchor_directory_path: PathBuf,
    completion_selector_relative: PathBuf,
    completion_selector_path: PathBuf,
    operation_nonce: String,
    transaction_lock_relative: PathBuf,
    transaction_lock_identity: crate::retained_dir::RetainedFileIdentity,
    replay_anchor: RetainedReplayAnchorAuthority,
    journal_document: Option<RetainedRestoreDocument<RestoreJournalDocument>>,
    receipt_document: Option<RetainedRestoreDocument<RestoreReceiptDocument>>,
    completion_document: Option<RetainedRestoreCompletion>,
    transaction_lock: File,
    quiescence: Option<RetainedDestinationQuiescence>,
}

impl fmt::Debug for RestorePreflight {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RestorePreflight")
            .field("destination", &self.plan.destination_sidecar)
            .field("staging", &self.staging_path)
            .field("member_count", &self.members.len())
            .finish_non_exhaustive()
    }
}

impl RestorePreflight {
    #[must_use]
    pub fn destination_sidecar(&self) -> &Path {
        &self.plan.destination_sidecar
    }

    #[must_use]
    pub fn archive_sha256(&self) -> &str {
        self.plan.verified.archive_sha256()
    }

    #[must_use]
    pub fn manifest_set_digest(&self) -> &str {
        &self
            .plan
            .verified
            .manifest()
            .backup_manifest
            .manifest_set_digest
    }

    #[must_use]
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    #[must_use]
    pub fn destination_already_published(&self) -> bool {
        self.destination_status == DestinationStatus::AlreadyPublished
    }
}

/// Opaque Store-owned capability for the one immutable completion generation.
/// Its retained protected-file handle prevents callers from constructing or
/// substituting a successful restore authority in memory.
pub struct RestoreCompletionAuthority {
    retained: RetainedRestoreCompletion,
}

impl fmt::Debug for RestoreCompletionAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RestoreCompletionAuthority")
            .field("path", &self.retained.completion.retained.path)
            .field("digest", &self.retained.completion.retained.digest)
            .field("selector", &self.retained.selector.retained.path)
            .finish_non_exhaustive()
    }
}

impl RestoreCompletionAuthority {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.retained.completion.retained.path
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.retained.completion.retained.digest
    }

    #[must_use]
    pub fn operation_nonce(&self) -> &str {
        &self.retained.completion.document.operation_nonce
    }
}

#[derive(Debug)]
pub struct RestorePublication {
    pub destination_sidecar: PathBuf,
    pub archive_sha256: String,
    pub manifest_set_digest: String,
    pub receipt_path: PathBuf,
    pub receipt_digest: String,
    pub member_count: usize,
    pub already_restored: bool,
    pub completion_authority: RestoreCompletionAuthority,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum RestoreError {
    Backup(BackupError),
    InvalidPath { path: PathBuf, reason: String },
    Collision { path: PathBuf, reason: String },
    Rollback { reason: String },
    Tampered { reason: String },
    Interrupted { path: PathBuf, reason: String },
    Io { path: PathBuf, source: io::Error },
}

impl fmt::Display for RestoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backup(error) => write!(formatter, "restore backup verification failed: {error}"),
            Self::InvalidPath { path, reason } => {
                write!(
                    formatter,
                    "invalid restore path {}: {reason}",
                    path.display()
                )
            }
            Self::Collision { path, reason } => {
                write!(
                    formatter,
                    "restore collision at {}: {reason}",
                    path.display()
                )
            }
            Self::Rollback { reason } => write!(formatter, "restore rollback rejected: {reason}"),
            Self::Tampered { reason } => write!(formatter, "restore input rejected: {reason}"),
            Self::Interrupted { path, reason } => write!(
                formatter,
                "interrupted restore at {} requires recovery: {reason}",
                path.display()
            ),
            Self::Io { path, source } => {
                write!(formatter, "restore I/O {} failed: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for RestoreError {}

impl From<BackupError> for RestoreError {
    fn from(error: BackupError) -> Self {
        Self::Backup(error)
    }
}

#[derive(Debug)]
struct RestoreMember {
    entry: BackupEntry,
    relative_destination: PathBuf,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DestinationStatus {
    Vacant,
    AlreadyPublished,
}

#[derive(Debug)]
struct RestoreRetainedDirectory {
    handle: File,
    display_path: PathBuf,
    identity: crate::retained_dir::RetainedFileIdentity,
}

#[derive(Debug)]
struct RetainedDestinationQuiescence {
    guard: HostQuiescenceGuard,
    state_root_identity: crate::retained_dir::RetainedFileIdentity,
}

#[derive(Debug)]
struct RetainedReplayAnchorAuthority {
    configured_root: RestoreRetainedDirectory,
    parent: RestoreRetainedDirectory,
    parent_relative: PathBuf,
    anchor_path: PathBuf,
    anchor_leaf: PathBuf,
    anchor_handle: File,
    anchor_identity: crate::retained_dir::RetainedFileIdentity,
    anchor_bytes: Vec<u8>,
    anchor_digest: String,
    lock_path: PathBuf,
    lock_leaf: PathBuf,
    lock_handle: File,
    lock_identity: crate::retained_dir::RetainedFileIdentity,
}

#[derive(Debug)]
enum RestoreCompletionSidecar<'a> {
    Existing {
        tree: &'a RetainedVerifiedSidecarTree,
        quiescence: &'a RetainedDestinationQuiescence,
        destination_state: &'a Path,
    },
    Committed(&'a RetainedCommittedSidecar),
}

#[derive(Debug)]
struct RestoreCompletionInputs<'a> {
    preflight: &'a RestorePreflight,
    sidecar: RestoreCompletionSidecar<'a>,
    journal: &'a RetainedRestoreDocument<RestoreJournalDocument>,
    receipt: &'a RetainedRestoreDocument<RestoreReceiptDocument>,
}

#[derive(Debug)]
struct ValidatedStagingTree {
    root: RestoreRetainedDirectory,
    files: BTreeMap<PathBuf, crate::retained_dir::RetainedFileIdentity>,
    directories: BTreeMap<PathBuf, crate::retained_dir::RetainedFileIdentity>,
}

#[derive(Debug)]
struct RetainedVerifiedSidecarFile {
    parent_relative: PathBuf,
    leaf: PathBuf,
    handle: File,
    identity: crate::retained_dir::RetainedFileIdentity,
    bytes: Vec<u8>,
    digest: String,
}

#[derive(Debug)]
struct RetainedVerifiedSidecarTree {
    parent: RestoreRetainedDirectory,
    root_leaf: PathBuf,
    root_path: PathBuf,
    root: RestoreRetainedDirectory,
    state_root_identity: crate::retained_dir::RetainedFileIdentity,
    directories: BTreeMap<PathBuf, RestoreRetainedDirectory>,
    files: BTreeMap<PathBuf, RetainedVerifiedSidecarFile>,
    namespace: BTreeMap<PathBuf, Vec<PathBuf>>,
}

#[derive(Debug)]
struct RetainedProtectedRestoreFile {
    authority_root: RestoreRetainedDirectory,
    parent: RestoreRetainedDirectory,
    parent_relative: PathBuf,
    relative: PathBuf,
    path: PathBuf,
    leaf: PathBuf,
    handle: File,
    identity: crate::retained_dir::RetainedFileIdentity,
    bytes: Vec<u8>,
    digest: String,
}

#[derive(Debug)]
struct RetainedRestoreDocument<T> {
    retained: RetainedProtectedRestoreFile,
    document: T,
}

#[derive(Debug)]
struct RetainedRestoreCompletion {
    selector: RetainedRestoreDocument<RestoreCompletionSelectorDocument>,
    completion: RetainedRestoreDocument<RestoreCompletionAuthorityDocument>,
    completion_anchor: crate::retained_dir::RetainedFileLifetimeAnchor,
}

#[derive(Debug)]
struct RetainedCommittedSidecar {
    parent: RestoreRetainedDirectory,
    destination_leaf: PathBuf,
    destination_sidecar: PathBuf,
    destination_state: PathBuf,
    root: RestoreRetainedDirectory,
    quiescence: RetainedDestinationQuiescence,
    verified_tree: Option<RetainedVerifiedSidecarTree>,
}

#[derive(Debug)]
struct RestorePublicationIsolation {
    recovery_leaf: PathBuf,
    recovery_root: RestoreRetainedDirectory,
    authoritative_placeholder: RestoreRetainedDirectory,
}

#[derive(Debug, Clone, Copy)]
enum RestoreRelativeOpen {
    Directory,
    DirectoryCreateNew,
    FileRead,
    FileReadWrite,
    FileReadWriteCreate,
    FileWriteNew,
    #[cfg(windows)]
    FileReadSharedDelete,
    #[cfg(windows)]
    FileDelete,
    #[cfg(windows)]
    DirectoryDelete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreJournalDocument {
    schema_version: String,
    operation_nonce: String,
    project_id: String,
    project_link_sha256: String,
    archive_sha256: String,
    manifest_set_digest: String,
    destination_sidecar: String,
    staging_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreReceiptDocument {
    schema_version: String,
    restore_receipt: RestoreReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreReceipt {
    operation_nonce: String,
    archive_sha256: String,
    backup_receipt_digest: String,
    manifest_set_digest: String,
    project_id: String,
    project_link_sha256: String,
    workflow_release: WorkflowGovernanceReleaseIdentity,
    effective_bundle: WorkflowEffectiveBundleIdentity,
    replay_monotonic_head: BackupReplayRollbackAnchor,
    destination_sidecar: String,
    sidecar_root_path_sha256: String,
    state_root_path_sha256: String,
    sidecar_inventory_digest: String,
    restored_at_unix: u64,
    receipt_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreCompletionAuthorityDocument {
    schema_version: String,
    operation_nonce: String,
    project_id: String,
    workflow_release: WorkflowGovernanceReleaseIdentity,
    effective_bundle: WorkflowEffectiveBundleIdentity,
    source: RestoreCompletionSourceIdentity,
    protected_authority_root: RestoreRootPathBinding,
    sidecar: RestoreCompletionSidecarBinding,
    journal: RestoreProtectedDocumentBinding,
    receipt: RestoreProtectedDocumentBinding,
    project_link: RestoreProjectLinkBinding,
    replay_anchor: RestoreReplayAnchorBinding,
    transaction: RestoreTransactionAuthorityBinding,
    quiescence: RestoreQuiescenceBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreCompletionSourceIdentity {
    archive_sha256: String,
    backup_receipt_digest: String,
    manifest_set_digest: String,
    backup_created_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreRootPathBinding {
    configured_path: String,
    configured_path_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreCompletionSidecarBinding {
    destination_sidecar: String,
    retained_parent: RestoreRootPathBinding,
    root_leaf: String,
    root_path_sha256: String,
    state_root_relative: String,
    state_root_path_sha256: String,
    inventory_digest: String,
    inventory: Vec<RestoreCompletionInventoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "entry_kind", rename_all = "snake_case", deny_unknown_fields)]
enum RestoreCompletionInventoryEntry {
    Directory {
        relative_path: String,
    },
    File {
        relative_path: String,
        byte_length: u64,
        sha256: String,
    },
}

impl RestoreCompletionInventoryEntry {
    fn relative_path(&self) -> &str {
        match self {
            Self::Directory { relative_path, .. } | Self::File { relative_path, .. } => {
                relative_path
            }
        }
    }

    fn kind_order(&self) -> u8 {
        match self {
            Self::Directory { .. } => 0,
            Self::File { .. } => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreProtectedDocumentBinding {
    relative_path: String,
    parent_relative: String,
    content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreProjectLinkBinding {
    project_root: RestoreRootPathBinding,
    leaf: String,
    content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreReplayAnchorBinding {
    configured_root: RestoreRootPathBinding,
    parent_relative: String,
    lock_leaf: String,
    anchor_leaf: String,
    anchor_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreTransactionAuthorityBinding {
    lock_relative: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreQuiescenceBinding {
    destination_state: String,
    destination_state_path_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreCompletionSelectorDocument {
    schema_version: String,
    operation_nonce: String,
    project_id: String,
    completion: RestoreCompletionRecordSelection,
    parent_root_anchor: RestoreCompletionParentRootAnchor,
    project: RestoreProjectLinkBinding,
    replay: RestoreReplayAnchorBinding,
    transaction: RestoreCompletionTransactionSelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreCompletionRecordSelection {
    relative_path: String,
    content_sha256: String,
    byte_length: u64,
    leaf_anchor: crate::retained_dir::RetainedFileAnchorBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreCompletionParentRootAnchor {
    protected_authority_root: RestoreRootPathBinding,
    completion_parent_relative: String,
    completion_parent_path_sha256: String,
    selector_relative: String,
    selector_parent_relative: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreCompletionTransactionSelection {
    transaction_lock_relative: String,
    journal: RestoreProtectedDocumentBinding,
    receipt: RestoreProtectedDocumentBinding,
}

/// Verify one immutable backup and bind it to the existing exact Project Link.
/// This step never writes the project, sidecar, staging tree, or receipt store.
pub fn plan_project_restore(request: RestorePlanRequest) -> Result<RestorePlan, RestoreError> {
    let authority = resolve_configured_backup_authority(&request.authority_id)?;
    if authority.authority_id() != request.authority_id {
        return Err(RestoreError::Tampered {
            reason: "configured restore authority differs from the requested authority".to_owned(),
        });
    }
    let verified = verify_backup_archive_with_authority(&request.archive_path, &authority)?;
    let project_root = fs::canonicalize(&request.project_root)
        .map_err(|source| io_error(&request.project_root, source))?;
    let project_root_retained = RestoreRetainedDirectory::open_root(&project_root)
        .map_err(|source| io_error(&project_root, source))?;
    project_root_retained
        .verify_namespace_identity()
        .map_err(|source| io_error(&project_root, source))?;
    let project_link_leaf = PathBuf::from(PROJECT_LINK_LEAF);
    let project_link_path = project_root.join(&project_link_leaf);
    let (project_link_file, project_link_identity) = project_root_retained
        .open_direct_file_retained(&project_link_leaf)
        .map_err(|source| io_error(&project_link_path, source))?;
    let (project_link_bytes, read_identity) = project_root_retained
        .read_direct_file_bounded(&project_link_leaf, MAX_RESTORE_AUTHORITY_BYTES)
        .map_err(|source| io_error(&project_link_path, source))?;
    if read_identity != project_link_identity {
        return Err(RestoreError::Tampered {
            reason: "Project Link changed identity during descriptor-relative planning".to_owned(),
        });
    }
    project_root_retained
        .verify_namespace_identity()
        .map_err(|source| io_error(&project_root, source))?;
    let project_link: ProjectLinkDocument =
        yaml_serde::from_slice(&project_link_bytes).map_err(|error| RestoreError::Tampered {
            reason: format!("selected Project Link parse failed: {error}"),
        })?;
    verified
        .manifest()
        .verify_project_link_bytes(&project_link_bytes, &project_link)
        .map_err(|error| RestoreError::Tampered {
            reason: format!("selected Project Link differs from backup: {error:?}"),
        })?;
    let destination_sidecar = normalized_destination(&project_root, &project_link.sidecar_root.0)?;
    let destination_state = normalized_destination(&project_root, &project_link.state_root.0)?;
    if destination_state.parent() != Some(destination_sidecar.as_path())
        || destination_state
            .file_name()
            .and_then(|value| value.to_str())
            != Some(DESTINATION_STATE_LEAF)
        || destination_sidecar.starts_with(&project_root)
        || project_root.starts_with(&destination_sidecar)
    {
        return Err(RestoreError::InvalidPath {
            path: destination_state,
            reason: "Project Link does not preserve one disjoint sidecar with a direct .forge-method state root"
                .to_owned(),
        });
    }
    for protected in [authority.receipt_store(), authority.replay_anchor_path()] {
        if protected.starts_with(&project_root)
            || protected.starts_with(&destination_sidecar)
            || project_root.starts_with(protected)
            || destination_sidecar.starts_with(protected)
        {
            return Err(RestoreError::InvalidPath {
                path: protected.to_path_buf(),
                reason: "protected restore authority overlaps project or destination state"
                    .to_owned(),
            });
        }
    }
    verify_protected_replay_authority(&verified, &authority)?;
    let plan = RestorePlan {
        request,
        authority,
        verified,
        project_root,
        project_root_retained,
        project_link_path,
        project_link_leaf,
        project_link_file,
        project_link_identity,
        project_link_bytes,
        project_link,
        destination_sidecar,
        destination_state,
    };
    ensure_project_link_unchanged(&plan)?;
    Ok(plan)
}

/// Preflight destination portability, current external authorities, all archive
/// members, and any prior interrupted publication while retaining destination
/// quiescence. No destination bytes are written by this step.
pub fn preflight_project_restore(plan: RestorePlan) -> Result<RestorePreflight, RestoreError> {
    let authority_root_path = plan.authority.receipt_store().to_path_buf();
    let authority_root = RestoreRetainedDirectory::open_root(&authority_root_path)
        .map_err(|source| io_error(&authority_root_path, source))?;
    authority_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&authority_root_path, source))?;
    preflight_destination_names(
        &plan.verified.manifest().backup_manifest.entries,
        plan.request.destination_platform,
    )?;
    verify_archive_current_non_state_authorities(
        &plan.verified,
        &plan.authority,
        plan.request.current_principal_registry.as_deref(),
        plan.request.current_broker_registry.as_deref(),
    )?;
    ensure_project_link_unchanged(&plan)?;
    let members = restore_members(&plan.verified)?;
    let quiescence = if plan.destination_state.is_dir() {
        Some(acquire_restore_destination_quiescence(
            &plan,
            "preflight of existing destination state",
        )?)
    } else {
        None
    };
    ensure_project_link_unchanged(&plan)?;
    let replay_anchor = retain_protected_replay_anchor_authority(&plan.verified, &plan.authority)?;
    let token = digest_token(plan.verified.archive_sha256())?;
    let destination_parent =
        plan.destination_sidecar
            .parent()
            .ok_or_else(|| RestoreError::InvalidPath {
                path: plan.destination_sidecar.clone(),
                reason: "destination sidecar has no parent".to_owned(),
            })?;
    let destination_leaf = plan
        .destination_sidecar
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| RestoreError::InvalidPath {
            path: plan.destination_sidecar.clone(),
            reason: "destination sidecar has no UTF-8 leaf".to_owned(),
        })?;
    let destination_leaf = PathBuf::from(destination_leaf);
    let staging_leaf = PathBuf::from(format!(
        ".{}.forge-restore-{token}",
        destination_leaf.display()
    ));
    let staging_path = destination_parent.join(&staging_leaf);
    let staging_parent = RestoreRetainedDirectory::open_root(destination_parent)
        .map_err(|source| io_error(destination_parent, source))?;
    staging_parent
        .verify_namespace_identity()
        .map_err(|source| io_error(destination_parent, source))?;
    let project_token = safe_component(&plan.project_link.project_id.0)?;
    let (transaction_lock_relative, transaction_lock_identity, transaction_lock) =
        acquire_restore_transaction_lock(&authority_root, &project_token)?;
    let destination_status = inspect_destination(
        &plan,
        &members,
        &staging_parent,
        &destination_leaf,
        quiescence
            .as_ref()
            .map(|retained| &retained.state_root_identity),
    )?;
    let journal_relative = PathBuf::from("restore-journals")
        .join(&project_token)
        .join(format!("{token}.json"));
    let journal_path = authority_root_path.join(&journal_relative);
    let receipt_relative = PathBuf::from("restores")
        .join(&project_token)
        .join(format!("{token}.json"));
    let receipt_path = authority_root_path.join(&receipt_relative);
    let completion_directory_relative = PathBuf::from("restore-completions")
        .join(&project_token)
        .join(token);
    let completion_directory_path = authority_root_path.join(&completion_directory_relative);
    let completion_anchor_directory_relative = PathBuf::from("restore-completion-anchors")
        .join(&project_token)
        .join(token);
    let completion_anchor_directory_path =
        authority_root_path.join(&completion_anchor_directory_relative);
    let completion_selector_relative = PathBuf::from("restore-completion-selectors")
        .join(&project_token)
        .join(format!("{token}.json"));
    let completion_selector_path = authority_root_path.join(&completion_selector_relative);
    let (journal_document, receipt_document, completion_document) = inspect_interrupted_state(
        &plan,
        &members,
        destination_status,
        &staging_path,
        &staging_parent,
        &staging_leaf,
        &authority_root,
        &journal_relative,
        &journal_path,
        &receipt_relative,
        &receipt_path,
        &completion_directory_relative,
        &completion_directory_path,
        &completion_anchor_directory_relative,
        &completion_anchor_directory_path,
        &completion_selector_relative,
        &completion_selector_path,
    )?;
    let operation_nonce = match &journal_document {
        Some(journal) => {
            validate_restore_operation_nonce(&journal.document().operation_nonce)?;
            journal.document().operation_nonce.clone()
        }
        None => new_restore_operation_nonce()?,
    };
    authority_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&authority_root_path, source))?;
    verify_restore_transaction_lock(
        &authority_root,
        &transaction_lock_relative,
        &transaction_lock_identity,
        &transaction_lock,
    )?;
    Ok(RestorePreflight {
        plan,
        members,
        destination_status,
        staging_path,
        staging_parent,
        staging_leaf,
        destination_leaf,
        authority_root,
        journal_relative,
        journal_path,
        receipt_relative,
        receipt_path,
        completion_directory_relative,
        completion_directory_path,
        completion_anchor_directory_relative,
        completion_anchor_directory_path,
        completion_selector_relative,
        completion_selector_path,
        operation_nonce,
        transaction_lock_relative,
        transaction_lock_identity,
        replay_anchor,
        journal_document,
        receipt_document,
        completion_document,
        transaction_lock,
        quiescence,
    })
}

fn retained_restore_journal(
    preflight: &RestorePreflight,
) -> Result<&RetainedRestoreDocument<RestoreJournalDocument>, RestoreError> {
    preflight
        .journal_document
        .as_ref()
        .ok_or_else(|| RestoreError::Interrupted {
            path: preflight.journal_path.clone(),
            reason: "restore journal capability was not retained".to_owned(),
        })
}

/// Apply a preflighted restore using a protected journal, create-new staging,
/// exact staged verification, no-replace directory publication, and one durable
/// content-addressed completion authority. Publishing or selecting that retained
/// authority is the only success linearization point; no decisive I/O follows.
pub fn apply_project_restore(
    preflight: RestorePreflight,
) -> Result<RestorePublication, RestoreError> {
    let mut preflight = preflight;
    let prior_completion = preflight.completion_document.take();
    let already_completed = prior_completion.is_some();
    if preflight.plan.request.destination_platform != host_destination_platform() {
        return Err(RestoreError::InvalidPath {
            path: preflight.plan.destination_sidecar.clone(),
            reason: "restore apply platform differs from the current host".to_owned(),
        });
    }
    preflight
        .staging_parent
        .verify_namespace_identity()
        .map_err(|source| io_error(&preflight.staging_parent.display_path, source))?;
    verify_current_restore_authorities(&preflight)?;
    ensure_project_link_unchanged(&preflight.plan)?;
    if let Some(journal) = &preflight.journal_document {
        journal.revalidate()?;
    }
    if let Some(receipt) = &preflight.receipt_document {
        receipt.revalidate()?;
        validate_restore_receipt_source(&preflight.plan, receipt.document())?;
        if preflight
            .staging_parent
            .open_optional_directory(&preflight.destination_leaf)
            .map_err(|source| io_error(&preflight.plan.destination_sidecar, source))?
            .is_none()
        {
            return Err(RestoreError::Interrupted {
                path: preflight.plan.destination_sidecar.clone(),
                reason: "protected restore receipt exists but destination sidecar is absent"
                    .to_owned(),
            });
        }
        let retained_tree = verify_quiesced_sidecar_exact(&preflight)?;
        verify_current_restore_authorities(&preflight)?;
        cleanup_validated_staging_retained(
            &preflight.staging_parent,
            &preflight.staging_leaf,
            &preflight.staging_path,
            &preflight.members,
            preflight.plan.verified.manifest(),
        )?;
        let quiescence =
            preflight
                .quiescence
                .as_ref()
                .ok_or_else(|| RestoreError::Interrupted {
                    path: preflight.plan.destination_state.clone(),
                    reason: "completion authority lacks existing-destination quiescence".to_owned(),
                })?;
        let expected_completion = {
            let inputs = RestoreCompletionInputs::new(
                &preflight,
                RestoreCompletionSidecar::Existing {
                    tree: &retained_tree,
                    quiescence,
                    destination_state: &preflight.plan.destination_state,
                },
                retained_restore_journal(&preflight)?,
                receipt,
            )?;
            inputs.prepare_authority()?
        };
        let completion_authority = match publish_or_select_completion_authority(
            &preflight,
            prior_completion,
            &expected_completion,
        ) {
            Ok(authority) => authority,
            Err(error) => {
                return Err(isolate_verified_sidecar_after_completion_error(
                    &retained_tree,
                    error,
                ));
            }
        };
        return Ok(publication_from_receipt(
            &preflight.plan,
            &preflight.receipt_path,
            receipt.document(),
            preflight.members.len(),
            already_completed,
            completion_authority,
        ));
    }
    if preflight.destination_status == DestinationStatus::AlreadyPublished {
        return Err(RestoreError::Interrupted {
            path: preflight.plan.destination_sidecar.clone(),
            reason: "an existing exact destination has no retained protected receipt and completion authority; caller-created matching bytes are never accepted"
                .to_owned(),
        });
    }
    if prior_completion.is_some() {
        return Err(RestoreError::Interrupted {
            path: preflight.completion_directory_path.clone(),
            reason: "restore completion authority exists without its protected receipt".to_owned(),
        });
    }

    let expected_journal = restore_journal(
        &preflight.plan,
        &preflight.staging_path,
        &preflight.operation_nonce,
    );
    if preflight.journal_document.is_none() {
        preflight.journal_document = Some(publish_or_validate_journal_retained(
            &preflight.authority_root,
            &preflight.journal_relative,
            &preflight.journal_path,
            &expected_journal,
        )?);
    }
    if retained_restore_journal(&preflight)?.document() != &expected_journal {
        return Err(RestoreError::Interrupted {
            path: preflight.journal_path.clone(),
            reason: "retained restore journal differs from this transaction".to_owned(),
        });
    }
    retained_restore_journal(&preflight)?.revalidate()?;

    let staged = match stage_restore_retained(
        &preflight.staging_parent,
        &preflight.staging_leaf,
        &preflight.staging_path,
        &preflight.members,
        preflight.plan.verified.manifest(),
    ) {
        Ok(staged) => staged,
        Err(error) => {
            rollback_staging_after_error_retained(
                &preflight.staging_parent,
                &preflight.staging_leaf,
                &preflight.staging_path,
                &preflight.members,
            );
            return Err(error);
        }
    };
    verify_current_restore_authorities(&preflight)?;
    ensure_project_link_unchanged(&preflight.plan)?;
    retained_restore_journal(&preflight)?.revalidate()?;
    let staged_state_root_identity = staged
        .directories
        .get(Path::new(DESTINATION_STATE_LEAF))
        .cloned()
        .ok_or_else(|| RestoreError::Tampered {
            reason: "validated restore staging lacks its direct destination state root".to_owned(),
        })?;
    let mut committed = publish_staging_directory_create_new(
        &preflight.staging_parent,
        &preflight.staging_leaf,
        &preflight.destination_leaf,
        &preflight.plan.destination_sidecar,
        &preflight.plan.destination_state,
        &staged.root,
        &staged_state_root_identity,
    )?;
    drop(staged);

    let completion = (|| -> Result<RestorePublication, RestoreError> {
        let state_root_identity = quiescence_bound_state_root_identity(
            &committed.quiescence.guard,
            &preflight.plan.destination_state,
        )?;
        if state_root_identity != committed.quiescence.state_root_identity {
            return Err(RestoreError::Tampered {
                reason: "retained committed restore quiescence changed before exact verification"
                    .to_owned(),
            });
        }
        let retained_tree = verify_sidecar_exact_retained(
            &committed.parent,
            &committed.destination_leaf,
            &committed.destination_sidecar,
            &preflight.members,
            preflight.plan.verified.manifest(),
            &state_root_identity,
        )?;
        committed.install_verified_tree(retained_tree)?;
        preflight
            .staging_parent
            .verify_namespace_identity()
            .map_err(|source| io_error(&preflight.staging_parent.display_path, source))?;
        verify_current_restore_authorities(&preflight)?;
        ensure_project_link_unchanged(&preflight.plan)?;
        retained_restore_journal(&preflight)?.revalidate()?;
        cleanup_validated_staging_retained(
            &preflight.staging_parent,
            &preflight.staging_leaf,
            &preflight.staging_path,
            &preflight.members,
            preflight.plan.verified.manifest(),
        )?;
        verify_current_restore_authorities(&preflight)?;
        committed.revalidate()?;
        retained_restore_journal(&preflight)?.revalidate()?;
        ensure_project_link_unchanged(&preflight.plan)?;

        let committed_tree =
            committed
                .verified_tree
                .as_ref()
                .ok_or_else(|| RestoreError::Interrupted {
                    path: committed.destination_sidecar.clone(),
                    reason: "committed restore lacks its exact tree before receipt publication"
                        .to_owned(),
                })?;
        let receipt =
            build_restore_receipt(&preflight.plan, &preflight.operation_nonce, committed_tree)?;
        preflight.receipt_document = Some(publish_or_validate_restore_receipt_retained(
            &preflight.authority_root,
            &preflight.receipt_relative,
            &preflight.receipt_path,
            &receipt,
        )?);
        let durable_receipt =
            preflight
                .receipt_document
                .as_ref()
                .ok_or_else(|| RestoreError::Interrupted {
                    path: preflight.receipt_path.clone(),
                    reason: "restore receipt capability was not retained".to_owned(),
                })?;
        let expected_completion = {
            let inputs = RestoreCompletionInputs::new(
                &preflight,
                RestoreCompletionSidecar::Committed(&committed),
                retained_restore_journal(&preflight)?,
                durable_receipt,
            )?;
            inputs.prepare_authority()?
        };
        let completion_authority =
            publish_or_select_completion_authority(&preflight, None, &expected_completion)?;
        Ok(publication_from_receipt(
            &preflight.plan,
            &preflight.receipt_path,
            durable_receipt.document(),
            preflight.members.len(),
            false,
            completion_authority,
        ))
    })();
    match completion {
        Ok(publication) => Ok(publication),
        Err(error) => Err(committed.isolate_after_error(error)),
    }
}

fn restore_members(verified: &VerifiedBackupArchive) -> Result<Vec<RestoreMember>, RestoreError> {
    let raw = verified_archive_members(verified)?;
    if raw.len() > MAX_RESTORE_MEMBERS {
        return Err(RestoreError::Tampered {
            reason: "restore member count exceeds the configured bound".to_owned(),
        });
    }
    let mut members = Vec::new();
    for (entry, bytes) in raw {
        if entry.entry_type != forge_core_contracts::BackupArchiveEntryType::RegularFile {
            return Err(RestoreError::Tampered {
                reason: format!(
                    "restore entry is not a retainable regular file: {}",
                    entry.logical_path
                ),
            });
        }
        let decoded = forge_core_contracts::decode_canonical_archive_path(&entry.logical_path)
            .map_err(|error| RestoreError::Tampered {
                reason: format!("archive member name is noncanonical: {error:?}"),
            })?;
        if entry.material == BackupEntryKind::ProjectLink {
            continue;
        }
        let relative = decoded
            .strip_prefix("sidecar/")
            .ok_or_else(|| RestoreError::Tampered {
                reason: format!("archive member is outside the sidecar root: {decoded}"),
            })?;
        let relative_destination = normalized_relative_path(relative)?;
        if sha256(&bytes) != entry.sha256 || bytes.len() as u64 != entry.byte_length {
            return Err(RestoreError::Tampered {
                reason: format!(
                    "archive member bytes differ from manifest: {}",
                    entry.logical_path
                ),
            });
        }
        members.push(RestoreMember {
            entry,
            relative_destination,
            bytes,
        });
    }
    members.sort_by(|left, right| left.relative_destination.cmp(&right.relative_destination));
    let mut paths = BTreeSet::new();
    for member in &members {
        if !paths.insert(member.relative_destination.clone()) {
            return Err(RestoreError::Tampered {
                reason: "duplicate decoded restore destination".to_owned(),
            });
        }
    }
    Ok(members)
}

fn retained_absolute_parent(path: &Path) -> Result<(PathBuf, PathBuf, PathBuf), RestoreError> {
    if !path.is_absolute() {
        return Err(RestoreError::InvalidPath {
            path: path.to_path_buf(),
            reason: "protected replay anchor must be an absolute configured path".to_owned(),
        });
    }
    let parent = path.parent().ok_or_else(|| RestoreError::InvalidPath {
        path: path.to_path_buf(),
        reason: "protected replay anchor has no configured parent".to_owned(),
    })?;
    let configured_root = parent
        .ancestors()
        .find(|ancestor| ancestor.parent().is_none())
        .ok_or_else(|| RestoreError::InvalidPath {
            path: path.to_path_buf(),
            reason: "protected replay anchor has no configured filesystem root".to_owned(),
        })?
        .to_path_buf();
    let parent_relative = parent
        .strip_prefix(&configured_root)
        .map_err(|_| RestoreError::InvalidPath {
            path: path.to_path_buf(),
            reason: "protected replay-anchor parent escaped its configured root".to_owned(),
        })?
        .to_path_buf();
    Ok((configured_root, parent_relative, parent.to_path_buf()))
}

fn retain_protected_replay_anchor_authority(
    verified: &VerifiedBackupArchive,
    authority: &TrustedBackupAuthority,
) -> Result<RetainedReplayAnchorAuthority, RestoreError> {
    let anchor_path = authority.replay_anchor_path().to_path_buf();
    let anchor_leaf = anchor_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| RestoreError::InvalidPath {
            path: anchor_path.clone(),
            reason: "protected replay anchor has no UTF-8 leaf".to_owned(),
        })?;
    let (configured_root_path, parent_relative, parent_path) =
        retained_absolute_parent(&anchor_path)?;
    let configured_root = RestoreRetainedDirectory::open_root(&configured_root_path)
        .map_err(|source| io_error(&configured_root_path, source))?;
    configured_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&configured_root_path, source))?;
    let parent = configured_root
        .open_directory_path(&parent_relative)
        .map_err(|source| io_error(&parent_path, source))?;
    configured_root
        .verify_directory_path_identity(&parent_relative, &parent.identity)
        .map_err(|source| io_error(&parent_path, source))?;

    let anchor_leaf = PathBuf::from(anchor_leaf);
    let lock_leaf = PathBuf::from(format!("{}.lock", anchor_leaf.display()));
    let lock_path = parent_path.join(&lock_leaf);
    let (lock_handle, lock_identity) = parent
        .open_direct_file_read_write_retained(&lock_leaf)
        .map_err(|source| io_error(&lock_path, source))?;
    FileExt::try_lock(&lock_handle).map_err(|source| RestoreError::Collision {
        path: lock_path.clone(),
        reason: format!("protected replay authority is busy: {source}"),
    })?;
    configured_root
        .verify_directory_path_identity(&parent_relative, &parent.identity)
        .map_err(|source| io_error(&parent_path, source))?;
    configured_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&configured_root_path, source))?;

    let (anchor_handle, anchor_identity) = parent
        .open_direct_file_retained(&anchor_leaf)
        .map_err(|source| io_error(&anchor_path, source))?;
    let (anchor_bytes, retained_identity) =
        restore_read_retained_file_bounded(&anchor_handle, MAX_RESTORE_AUTHORITY_BYTES)
            .map_err(|source| io_error(&anchor_path, source))?;
    let (namespace_bytes, namespace_identity) = parent
        .read_direct_file_bounded(&anchor_leaf, MAX_RESTORE_AUTHORITY_BYTES)
        .map_err(|source| io_error(&anchor_path, source))?;
    if retained_identity != anchor_identity
        || namespace_identity != anchor_identity
        || namespace_bytes != anchor_bytes
    {
        return Err(RestoreError::Tampered {
            reason: "protected replay anchor changed while its authority was retained".to_owned(),
        });
    }
    configured_root
        .verify_directory_path_identity(&parent_relative, &parent.identity)
        .map_err(|source| io_error(&parent_path, source))?;
    configured_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&configured_root_path, source))?;

    let retained = RetainedReplayAnchorAuthority {
        configured_root,
        parent,
        parent_relative,
        anchor_path,
        anchor_leaf,
        anchor_handle,
        anchor_identity,
        anchor_digest: sha256(&anchor_bytes),
        anchor_bytes,
        lock_path,
        lock_leaf,
        lock_handle,
        lock_identity,
    };
    retained.revalidate(verified, authority)?;
    Ok(retained)
}

fn verify_current_restore_authorities(preflight: &RestorePreflight) -> Result<(), RestoreError> {
    preflight
        .authority_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&preflight.authority_root.display_path, source))?;
    verify_restore_transaction_lock(
        &preflight.authority_root,
        &preflight.transaction_lock_relative,
        &preflight.transaction_lock_identity,
        &preflight.transaction_lock,
    )?;
    preflight
        .replay_anchor
        .revalidate(&preflight.plan.verified, &preflight.plan.authority)
}

fn quiescence_bound_state_root_identity(
    guard: &HostQuiescenceGuard,
    destination_state: &Path,
) -> Result<crate::retained_dir::RetainedFileIdentity, RestoreError> {
    let lease = crate::producer_quiescence::BoundaryLease::from_boundary(guard, destination_state)
        .map_err(|error| RestoreError::Tampered {
            reason: format!(
                "restore destination quiescence did not retain its exact root: {error}"
            ),
        })?;
    let retained_root = lease
        .retained_root()
        .map_err(|error| RestoreError::Tampered {
            reason: format!("restore destination quiescence root could not be retained: {error}"),
        })?;
    let identity = retained_root
        .identity()
        .map_err(|source| io_error(destination_state, source))?;
    lease
        .validate_root(destination_state)
        .map_err(|error| RestoreError::Tampered {
            reason: format!(
                "restore destination changed while its root identity was retained: {error}"
            ),
        })?;
    if retained_root
        .identity()
        .map_err(|source| io_error(destination_state, source))?
        != identity
    {
        return Err(RestoreError::Tampered {
            reason: "quiescence-bound destination state root changed identity".to_owned(),
        });
    }
    Ok(identity)
}

fn acquire_restore_destination_quiescence(
    plan: &RestorePlan,
    context: &str,
) -> Result<RetainedDestinationQuiescence, RestoreError> {
    let guard = quiesce_host_producers(&plan.destination_state, &AtomicBool::new(false)).map_err(
        |error| RestoreError::Collision {
            path: plan.destination_state.clone(),
            reason: format!("cannot quiesce restore destination during {context}: {error}"),
        },
    )?;
    let state_root_identity =
        quiescence_bound_state_root_identity(&guard, &plan.destination_state)?;
    Ok(RetainedDestinationQuiescence {
        guard,
        state_root_identity,
    })
}

fn require_retained_destination_quiescence(
    preflight: &RestorePreflight,
) -> Result<&crate::retained_dir::RetainedFileIdentity, RestoreError> {
    let retained = preflight
        .quiescence
        .as_ref()
        .ok_or_else(|| RestoreError::Interrupted {
            path: preflight.plan.destination_state.clone(),
            reason: "restore destination is not protected by retained host quiescence".to_owned(),
        })?;
    let current =
        quiescence_bound_state_root_identity(&retained.guard, &preflight.plan.destination_state)?;
    if current != retained.state_root_identity {
        return Err(RestoreError::Tampered {
            reason: "retained restore destination quiescence changed exact root identity"
                .to_owned(),
        });
    }
    Ok(&retained.state_root_identity)
}

fn verify_quiesced_sidecar_exact(
    preflight: &RestorePreflight,
) -> Result<RetainedVerifiedSidecarTree, RestoreError> {
    let state_root_identity = require_retained_destination_quiescence(preflight)?.clone();
    let retained = verify_sidecar_exact_retained(
        &preflight.staging_parent,
        &preflight.destination_leaf,
        &preflight.plan.destination_sidecar,
        &preflight.members,
        preflight.plan.verified.manifest(),
        &state_root_identity,
    )?;
    if require_retained_destination_quiescence(preflight)? != &state_root_identity {
        return Err(RestoreError::Tampered {
            reason: "restore quiescence changed during exact full-tree verification".to_owned(),
        });
    }
    retained.revalidate()?;
    Ok(retained)
}

fn verify_protected_replay_authority(
    verified: &VerifiedBackupArchive,
    authority: &TrustedBackupAuthority,
) -> Result<(), RestoreError> {
    let raw = read_nofollow_bounded(authority.replay_anchor_path(), MAX_RESTORE_AUTHORITY_BYTES)?;
    validate_protected_replay_anchor_bytes(&raw, verified, authority)
}

fn validate_protected_replay_anchor_bytes(
    raw: &[u8],
    verified: &VerifiedBackupArchive,
    authority: &TrustedBackupAuthority,
) -> Result<(), RestoreError> {
    let current: ReplayAnchorDocument =
        serde_json::from_slice(raw).map_err(|error| RestoreError::Tampered {
            reason: format!("current protected replay anchor parse failed: {error}"),
        })?;
    let expected = &verified.receipt().backup_receipt.replay_monotonic_head;
    if expected.protected_anchor_identity != authority.protected_anchor_identity() {
        return Err(RestoreError::Tampered {
            reason: "backup replay anchor identity differs from configured authority".to_owned(),
        });
    }
    if current.generation > expected.generation {
        return Err(RestoreError::Rollback {
            reason: "backup replay generation is older than current protected authority".to_owned(),
        });
    }
    if current.generation < expected.generation {
        return Err(RestoreError::Rollback {
            reason: "current protected replay authority is stale for this backup".to_owned(),
        });
    }
    if current.schema_version != expected.schema_version
        || current.deployment_id != expected.deployment_id
        || current.epoch != expected.epoch
        || current.previous_anchor_digest != expected.previous_anchor_digest
        || sha256(raw) != expected.anchor_document_sha256
        || current.head.manifest_digest != expected.replay_wal_manifest_digest
        || current.head.wal_prefix_digest != expected.replay_wal_prefix_digest
        || current.head.last_seq != expected.replay_wal_last_seq
        || current.head.record_count as u64 != expected.replay_wal_record_count
        || current.head.byte_len != expected.replay_wal_byte_length
    {
        return Err(RestoreError::Tampered {
            reason: "backup replay head is substituted or differs from current protected authority"
                .to_owned(),
        });
    }
    Ok(())
}

fn inspect_destination(
    plan: &RestorePlan,
    members: &[RestoreMember],
    parent: &RestoreRetainedDirectory,
    leaf: &Path,
    state_root_identity: Option<&crate::retained_dir::RetainedFileIdentity>,
) -> Result<DestinationStatus, RestoreError> {
    match parent.open_optional_directory(leaf) {
        Ok(None) => Ok(DestinationStatus::Vacant),
        Ok(Some(_)) => {
            let state_root_identity =
                state_root_identity.ok_or_else(|| RestoreError::Collision {
                    path: plan.destination_state.clone(),
                    reason: "existing destination lacks exact-root retained quiescence".to_owned(),
                })?;
            verify_sidecar_exact_retained(
                parent,
                leaf,
                &plan.destination_sidecar,
                members,
                plan.verified.manifest(),
                state_root_identity,
            )?;
            Ok(DestinationStatus::AlreadyPublished)
        }
        Err(_) => Err(RestoreError::Collision {
            path: plan.destination_sidecar.clone(),
            reason: "destination exists and is linked, special, or not a retained directory"
                .to_owned(),
        }),
    }
}

fn inspect_interrupted_state(
    plan: &RestorePlan,
    members: &[RestoreMember],
    destination_status: DestinationStatus,
    staging_path: &Path,
    staging_parent: &RestoreRetainedDirectory,
    staging_leaf: &Path,
    authority_root: &RestoreRetainedDirectory,
    journal_relative: &Path,
    journal_path: &Path,
    receipt_relative: &Path,
    receipt_path: &Path,
    completion_directory_relative: &Path,
    completion_directory_path: &Path,
    completion_anchor_directory_relative: &Path,
    completion_anchor_directory_path: &Path,
    completion_selector_relative: &Path,
    completion_selector_path: &Path,
) -> Result<
    (
        Option<RetainedRestoreDocument<RestoreJournalDocument>>,
        Option<RetainedRestoreDocument<RestoreReceiptDocument>>,
        Option<RetainedRestoreCompletion>,
    ),
    RestoreError,
> {
    let receipt = load_restore_receipt_retained(authority_root, receipt_relative, receipt_path)?;
    if let Some(receipt) = &receipt {
        validate_restore_receipt_source(plan, receipt.document())?;
        receipt.revalidate()?;
        if destination_status != DestinationStatus::AlreadyPublished {
            return Err(RestoreError::Interrupted {
                path: plan.destination_sidecar.clone(),
                reason: "restore receipt exists without its exact destination".to_owned(),
            });
        }
    } else if destination_status == DestinationStatus::AlreadyPublished {
        return Err(RestoreError::Interrupted {
            path: plan.destination_sidecar.clone(),
            reason: "an exact destination without its retained protected receipt cannot prove Store-owned publication; caller-created matching bytes are never accepted"
                .to_owned(),
        });
    }
    let journal = load_restore_journal_retained(authority_root, journal_relative, journal_path)?;
    if let Some(journal) = &journal {
        validate_restore_operation_nonce(&journal.document().operation_nonce)?;
        if journal.document()
            != &restore_journal(plan, staging_path, &journal.document().operation_nonce)
        {
            return Err(RestoreError::Interrupted {
                path: journal_path.to_path_buf(),
                reason: "protected restore journal bindings differ".to_owned(),
            });
        }
        journal.revalidate()?;
    }
    if let (Some(journal), Some(receipt)) = (&journal, &receipt) {
        if receipt.document().restore_receipt.operation_nonce.as_str()
            != journal.document().operation_nonce.as_str()
        {
            return Err(RestoreError::Interrupted {
                path: receipt_path.to_path_buf(),
                reason: "protected restore receipt belongs to a different journal operation"
                    .to_owned(),
            });
        }
    }
    let completion = load_restore_completion_authority_retained(
        authority_root,
        completion_directory_relative,
        completion_directory_path,
        completion_anchor_directory_relative,
        completion_anchor_directory_path,
        completion_selector_relative,
        completion_selector_path,
    )?;
    if completion.is_some()
        && (journal.is_none()
            || receipt.is_none()
            || destination_status != DestinationStatus::AlreadyPublished)
    {
        return Err(RestoreError::Interrupted {
            path: completion_selector_path.to_path_buf(),
            reason: "restore completion selector exists without its journal, receipt, and exact destination"
                .to_owned(),
        });
    }
    if receipt.is_some() && completion.is_none() {
        return Err(RestoreError::Interrupted {
            path: completion_selector_path.to_path_buf(),
            reason: "protected restore receipt exists without its atomically committed completion selector; exact completion authority cannot be reminted on retry"
                .to_owned(),
        });
    }
    let staged = staging_parent
        .open_optional_directory(staging_leaf)
        .map_err(|source| io_error(staging_path, source))?;
    if journal.is_some() {
        if let Some(root) = staged {
            verify_staging_prefix_retained(root, members)?;
        }
    } else if staged.is_some() {
        return Err(RestoreError::Interrupted {
            path: staging_path.to_path_buf(),
            reason: "unjournaled restore staging directory exists".to_owned(),
        });
    }
    if let Some(receipt) = &receipt {
        receipt.revalidate()?;
    }
    if let Some(journal) = &journal {
        journal.revalidate()?;
    }
    if let Some(completion) = &completion {
        completion.revalidate()?;
    }
    Ok((journal, receipt, completion))
}

impl RestoreRetainedDirectory {
    fn open_root(path: &Path) -> io::Result<Self> {
        Self::from_handle(restore_open_root_directory(path)?, path.to_path_buf())
    }

    fn from_handle(handle: File, display_path: PathBuf) -> io::Result<Self> {
        validate_restore_directory_handle(&handle)?;
        let identity = crate::retained_dir::RetainedDirectory::identity_of(&handle)?;
        Ok(Self {
            handle,
            display_path,
            identity,
        })
    }

    fn try_clone(&self) -> io::Result<Self> {
        Self::from_handle(self.handle.try_clone()?, self.display_path.clone())
    }

    fn verify_identity(&self) -> io::Result<()> {
        validate_restore_directory_handle(&self.handle)?;
        if crate::retained_dir::RetainedDirectory::identity_of(&self.handle)? != self.identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained restore directory changed identity",
            ));
        }
        Ok(())
    }

    fn verify_namespace_identity(&self) -> io::Result<()> {
        self.verify_identity()?;
        let current = Self::open_root(&self.display_path)?;
        if current.identity != self.identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained restore directory namespace was replaced",
            ));
        }
        Ok(())
    }

    fn open_directory(&self, leaf: &Path) -> io::Result<Self> {
        restore_direct_component(leaf)?;
        self.verify_identity()?;
        let child = Self::from_handle(
            restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::Directory)?,
            self.display_path.join(leaf),
        )?;
        self.verify_identity()?;
        Ok(child)
    }

    fn open_optional_directory(&self, leaf: &Path) -> io::Result<Option<Self>> {
        match self.open_directory(leaf) {
            Ok(directory) => Ok(Some(directory)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn create_directory_new(&self, leaf: &Path) -> io::Result<Self> {
        restore_direct_component(leaf)?;
        self.verify_identity()?;
        let child = Self::from_handle(
            restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::DirectoryCreateNew)?,
            self.display_path.join(leaf),
        )?;
        self.verify_direct_directory_identity(leaf, &child.identity)?;
        self.verify_identity()?;
        Ok(child)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<Self> {
        let mut directory = self.try_clone()?;
        for component in restore_relative_components(path)? {
            let leaf = Path::new(&component);
            directory = match directory.open_directory(leaf) {
                Ok(child) => child,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    match directory.create_directory_new(leaf) {
                        Ok(child) => child,
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                            directory.open_directory(leaf)?
                        }
                        Err(error) => return Err(error),
                    }
                }
                Err(error) => return Err(error),
            };
        }
        Ok(directory)
    }

    fn create_dir_all_synced(&self, path: &Path) -> io::Result<Self> {
        let mut directory = self.try_clone()?;
        for component in restore_relative_components(path)? {
            let leaf = Path::new(&component);
            let (child, created) = match directory.open_directory(leaf) {
                Ok(child) => (child, false),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    match directory.create_directory_new(leaf) {
                        Ok(child) => (child, true),
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                            (directory.open_directory(leaf)?, false)
                        }
                        Err(error) => return Err(error),
                    }
                }
                Err(error) => return Err(error),
            };
            if created {
                child.sync_self()?;
                directory.sync_self()?;
            }
            directory.verify_direct_directory_identity(leaf, &child.identity)?;
            directory = child;
        }
        self.verify_identity()?;
        Ok(directory)
    }

    fn open_directory_path(&self, path: &Path) -> io::Result<Self> {
        if path.as_os_str().is_empty() {
            return self.try_clone();
        }
        let mut directory = self.try_clone()?;
        for component in restore_relative_components(path)? {
            directory = directory.open_directory(Path::new(&component))?;
        }
        Ok(directory)
    }

    fn verify_directory_path_identity(
        &self,
        path: &Path,
        expected: &crate::retained_dir::RetainedFileIdentity,
    ) -> io::Result<()> {
        let current = self.open_directory_path(path)?;
        if current.identity != *expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained restore parent was redirected outside staging",
            ));
        }
        Ok(())
    }

    fn open_directory_path_with_identities(
        &self,
        path: &Path,
        identities: &BTreeMap<PathBuf, crate::retained_dir::RetainedFileIdentity>,
    ) -> io::Result<Self> {
        if path.as_os_str().is_empty() {
            return self.try_clone();
        }
        let mut directory = self.try_clone()?;
        let mut relative = PathBuf::new();
        for component in restore_relative_components(path)? {
            relative.push(&component);
            let child = directory.open_directory(Path::new(&component))?;
            let expected = identities.get(&relative).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "cleanup directory lacks a validated identity",
                )
            })?;
            if child.identity != *expected {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "cleanup directory identity changed",
                ));
            }
            directory = child;
        }
        Ok(directory)
    }

    fn verify_direct_directory_identity(
        &self,
        leaf: &Path,
        expected: &crate::retained_dir::RetainedFileIdentity,
    ) -> io::Result<()> {
        let current = self.open_directory(leaf)?;
        if current.identity != *expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained restore directory was substituted",
            ));
        }
        Ok(())
    }

    fn direct_entries(&self) -> io::Result<Vec<PathBuf>> {
        self.verify_identity()?;
        let mut entries = restore_direct_directory_entries(&self.handle)?;
        entries.sort();
        self.verify_identity()?;
        Ok(entries)
    }

    fn open_direct_file_retained(
        &self,
        leaf: &Path,
    ) -> io::Result<(File, crate::retained_dir::RetainedFileIdentity)> {
        restore_direct_component(leaf)?;
        self.verify_identity()?;
        let file = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileRead)?;
        let identity = validate_restore_file_handle(&file)?;
        let reopened = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileRead)?;
        if validate_restore_file_handle(&reopened)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore file namespace identity changed during retained open",
            ));
        }
        self.verify_identity()?;
        Ok((file, identity))
    }

    fn open_direct_authority_file_retained(
        &self,
        leaf: &Path,
    ) -> io::Result<(File, crate::retained_dir::RetainedFileIdentity)> {
        restore_direct_component(leaf)?;
        self.verify_identity()?;
        let file = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileRead)?;
        let identity = validate_restore_authority_file_handle(&file)?;
        let reopened = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileRead)?;
        if validate_restore_authority_file_handle(&reopened)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore authority file namespace changed during retained open",
            ));
        }
        self.verify_identity()?;
        Ok((file, identity))
    }

    fn read_direct_file_bounded(
        &self,
        leaf: &Path,
        maximum: u64,
    ) -> io::Result<(Vec<u8>, crate::retained_dir::RetainedFileIdentity)> {
        restore_direct_component(leaf)?;
        self.verify_identity()?;
        let mut file = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileRead)?;
        let identity = validate_restore_file_handle(&file)?;
        let before = file.metadata()?;
        if before.len() > maximum {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "restore file exceeds its declared byte length",
            ));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
        std::io::Read::by_ref(&mut file)
            .take(maximum.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "restore file exceeds its declared byte length",
            ));
        }
        let after_identity = validate_restore_file_handle(&file)?;
        let after = file.metadata()?;
        if after_identity != identity
            || after.len() != before.len()
            || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore file changed while it was read",
            ));
        }
        let reopened = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileRead)?;
        if validate_restore_file_handle(&reopened)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore file namespace identity changed after read",
            ));
        }
        self.verify_identity()?;
        Ok((bytes, identity))
    }

    fn read_direct_authority_file_bounded(
        &self,
        leaf: &Path,
        maximum: u64,
    ) -> io::Result<(Vec<u8>, crate::retained_dir::RetainedFileIdentity)> {
        restore_direct_component(leaf)?;
        self.verify_identity()?;
        let file = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileRead)?;
        let (bytes, identity) = restore_read_retained_authority_file_bounded(&file, maximum)?;
        let reopened = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileRead)?;
        if validate_restore_authority_file_handle(&reopened)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore authority file namespace identity changed after read",
            ));
        }
        self.verify_identity()?;
        Ok((bytes, identity))
    }

    fn read_optional_direct_file_bounded(
        &self,
        leaf: &Path,
        maximum: u64,
    ) -> io::Result<Option<(Vec<u8>, crate::retained_dir::RetainedFileIdentity)>> {
        match self.read_direct_file_bounded(leaf, maximum) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn direct_file_identity(
        &self,
        leaf: &Path,
    ) -> io::Result<crate::retained_dir::RetainedFileIdentity> {
        restore_direct_component(leaf)?;
        self.verify_identity()?;
        let file = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileRead)?;
        let identity = validate_restore_file_handle(&file)?;
        let reopened = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileRead)?;
        if validate_restore_file_handle(&reopened)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore file namespace identity changed during validation",
            ));
        }
        self.verify_identity()?;
        Ok(identity)
    }

    fn open_direct_file_read_write_retained(
        &self,
        leaf: &Path,
    ) -> io::Result<(File, crate::retained_dir::RetainedFileIdentity)> {
        restore_direct_component(leaf)?;
        self.verify_identity()?;
        let file = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileReadWrite)?;
        let identity = validate_restore_file_handle(&file)?;
        let reopened = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileRead)?;
        if validate_restore_file_handle(&reopened)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore read-write file namespace changed during retained open",
            ));
        }
        self.verify_identity()?;
        Ok((file, identity))
    }

    fn open_or_create_direct_file(
        &self,
        leaf: &Path,
    ) -> io::Result<(File, crate::retained_dir::RetainedFileIdentity)> {
        restore_direct_component(leaf)?;
        self.verify_identity()?;
        let file =
            restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileReadWriteCreate)?;
        let identity = validate_restore_file_handle(&file)?;
        let reopened = restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileRead)?;
        if validate_restore_file_handle(&reopened)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore lock namespace identity changed during acquisition",
            ));
        }
        self.verify_identity()?;
        Ok((file, identity))
    }

    fn read_optional_file_bounded(
        &self,
        path: &Path,
        maximum: u64,
    ) -> io::Result<Option<(Vec<u8>, crate::retained_dir::RetainedFileIdentity)>> {
        let parent_path = path.parent().unwrap_or_else(|| Path::new(""));
        let leaf = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "restore file has no leaf")
        })?;
        let parent = match self.open_directory_path(parent_path) {
            Ok(parent) => parent,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        self.verify_directory_path_identity(parent_path, &parent.identity)?;
        let result = parent.read_optional_direct_file_bounded(Path::new(leaf), maximum)?;
        self.verify_directory_path_identity(parent_path, &parent.identity)?;
        Ok(result)
    }

    fn write_direct_file_new_validated(
        &self,
        leaf: &Path,
        bytes: &[u8],
    ) -> io::Result<crate::retained_dir::RetainedFileIdentity> {
        restore_direct_component(leaf)?;
        self.verify_identity()?;
        let mut file =
            restore_open_relative(&self.handle, leaf, RestoreRelativeOpen::FileWriteNew)?;
        let identity = validate_restore_file_handle(&file)?;
        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = self.remove_direct_file_if_identity(leaf, &identity);
            return Err(error);
        }
        let after_identity = validate_restore_file_handle(&file)?;
        if after_identity != identity
            || file.metadata()?.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore file changed identity while it was written",
            ));
        }
        let (reopened, reopened_identity) =
            self.read_direct_file_bounded(leaf, u64::try_from(bytes.len()).unwrap_or(u64::MAX))?;
        if reopened_identity != identity || reopened != bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore file changed after its retained write",
            ));
        }
        self.verify_identity()?;
        Ok(identity)
    }

    fn write_file_new_validated(
        &self,
        path: &Path,
        bytes: &[u8],
    ) -> io::Result<crate::retained_dir::RetainedFileIdentity> {
        let parent_path = path.parent().unwrap_or_else(|| Path::new(""));
        let leaf = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "restore file has no leaf")
        })?;
        let parent = self.open_directory_path(parent_path)?;
        self.verify_directory_path_identity(parent_path, &parent.identity)?;
        let identity = parent.write_direct_file_new_validated(Path::new(leaf), bytes)?;
        self.verify_directory_path_identity(parent_path, &parent.identity)?;
        Ok(identity)
    }

    fn sync_self(&self) -> io::Result<()> {
        self.verify_identity()?;
        self.handle.sync_all()?;
        self.verify_identity()
    }

    fn sync_tree(&self) -> io::Result<()> {
        for leaf in self.direct_entries()? {
            match self.open_directory(&leaf) {
                Ok(child) => {
                    child.sync_tree()?;
                    self.verify_direct_directory_identity(&leaf, &child.identity)?;
                }
                Err(_) => {
                    let _ = self.read_direct_file_bounded(&leaf, u64::MAX)?;
                }
            }
        }
        self.sync_self()
    }

    fn remove_file_path_if_identity(
        &self,
        path: &Path,
        directories: &BTreeMap<PathBuf, crate::retained_dir::RetainedFileIdentity>,
        expected: &crate::retained_dir::RetainedFileIdentity,
    ) -> io::Result<()> {
        let parent_path = path.parent().unwrap_or_else(|| Path::new(""));
        let leaf = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "cleanup file has no leaf")
        })?;
        let parent = self.open_directory_path_with_identities(parent_path, directories)?;
        parent.remove_direct_file_if_identity(Path::new(leaf), expected)
    }

    fn remove_directory_path_if_identity(
        &self,
        path: &Path,
        directories: &BTreeMap<PathBuf, crate::retained_dir::RetainedFileIdentity>,
        expected: &crate::retained_dir::RetainedFileIdentity,
    ) -> io::Result<()> {
        let parent_path = path.parent().unwrap_or_else(|| Path::new(""));
        let leaf = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "cleanup directory has no leaf")
        })?;
        let parent = self.open_directory_path_with_identities(parent_path, directories)?;
        parent.remove_direct_directory_if_identity(Path::new(leaf), expected)
    }

    fn remove_direct_file_if_identity(
        &self,
        leaf: &Path,
        expected: &crate::retained_dir::RetainedFileIdentity,
    ) -> io::Result<()> {
        restore_direct_component(leaf)?;
        self.verify_identity()?;
        #[cfg(windows)]
        let mode = RestoreRelativeOpen::FileDelete;
        #[cfg(not(windows))]
        let mode = RestoreRelativeOpen::FileRead;
        let file = restore_open_relative(&self.handle, leaf, mode)?;
        if validate_restore_file_handle(&file)? != *expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "refusing to remove a substituted restore file",
            ));
        }
        restore_remove_relative(&self.handle, leaf, file, false)?;
        self.verify_identity()
    }

    fn remove_direct_directory_if_identity(
        &self,
        leaf: &Path,
        expected: &crate::retained_dir::RetainedFileIdentity,
    ) -> io::Result<()> {
        restore_direct_component(leaf)?;
        self.verify_identity()?;
        #[cfg(windows)]
        let mode = RestoreRelativeOpen::DirectoryDelete;
        #[cfg(not(windows))]
        let mode = RestoreRelativeOpen::Directory;
        let directory = restore_open_relative(&self.handle, leaf, mode)?;
        validate_restore_directory_handle(&directory)?;
        if crate::retained_dir::RetainedDirectory::identity_of(&directory)? != *expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "refusing to remove a substituted restore directory",
            ));
        }
        if !restore_direct_directory_entries(&directory)?.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::DirectoryNotEmpty,
                "refusing to remove a nonempty restore directory",
            ));
        }
        restore_remove_relative(&self.handle, leaf, directory, true)?;
        self.verify_identity()
    }
}

impl RetainedReplayAnchorAuthority {
    fn revalidate_filesystem(&self) -> Result<(), RestoreError> {
        if self
            .configured_root
            .display_path
            .join(&self.parent_relative)
            != self.parent.display_path
            || self.parent.display_path.join(&self.anchor_leaf) != self.anchor_path
            || self.parent.display_path.join(&self.lock_leaf) != self.lock_path
            || self.anchor_digest != sha256(&self.anchor_bytes)
        {
            return Err(RestoreError::Tampered {
                reason: "retained replay-anchor authority binding changed".to_owned(),
            });
        }

        self.configured_root
            .verify_namespace_identity()
            .map_err(|source| io_error(&self.configured_root.display_path, source))?;
        self.configured_root
            .verify_directory_path_identity(&self.parent_relative, &self.parent.identity)
            .map_err(|source| io_error(&self.parent.display_path, source))?;
        self.parent
            .verify_identity()
            .map_err(|source| io_error(&self.parent.display_path, source))?;

        let retained_lock_identity = validate_restore_file_handle(&self.lock_handle)
            .map_err(|source| io_error(&self.lock_path, source))?;
        let (reopened_lock, reopened_lock_identity) = self
            .parent
            .open_direct_file_retained(&self.lock_leaf)
            .map_err(|source| io_error(&self.lock_path, source))?;
        if retained_lock_identity != self.lock_identity
            || reopened_lock_identity != self.lock_identity
            || validate_restore_file_handle(&reopened_lock)
                .map_err(|source| io_error(&self.lock_path, source))?
                != self.lock_identity
        {
            return Err(RestoreError::Tampered {
                reason: "protected replay-anchor lock leaf was substituted".to_owned(),
            });
        }

        let maximum = u64::try_from(self.anchor_bytes.len()).unwrap_or(u64::MAX);
        let (handle_bytes, handle_identity) =
            restore_read_retained_file_bounded(&self.anchor_handle, maximum)
                .map_err(|source| io_error(&self.anchor_path, source))?;
        let (reopened_anchor, reopened_anchor_identity) = self
            .parent
            .open_direct_file_retained(&self.anchor_leaf)
            .map_err(|source| io_error(&self.anchor_path, source))?;
        let (namespace_bytes, namespace_identity) =
            restore_read_retained_file_bounded(&reopened_anchor, maximum)
                .map_err(|source| io_error(&self.anchor_path, source))?;
        if handle_identity != self.anchor_identity
            || reopened_anchor_identity != self.anchor_identity
            || namespace_identity != self.anchor_identity
            || handle_bytes != self.anchor_bytes
            || namespace_bytes != self.anchor_bytes
            || sha256(&handle_bytes) != self.anchor_digest
            || sha256(&namespace_bytes) != self.anchor_digest
        {
            return Err(RestoreError::Tampered {
                reason: "protected replay anchor leaf, identity, or bytes changed".to_owned(),
            });
        }

        self.configured_root
            .verify_directory_path_identity(&self.parent_relative, &self.parent.identity)
            .map_err(|source| io_error(&self.parent.display_path, source))?;
        self.configured_root
            .verify_namespace_identity()
            .map_err(|source| io_error(&self.configured_root.display_path, source))
    }

    fn revalidate(
        &self,
        verified: &VerifiedBackupArchive,
        authority: &TrustedBackupAuthority,
    ) -> Result<(), RestoreError> {
        if self.anchor_path.as_path() != authority.replay_anchor_path() {
            return Err(RestoreError::Tampered {
                reason: "retained replay-anchor configured path changed".to_owned(),
            });
        }
        self.revalidate_filesystem()?;
        validate_protected_replay_anchor_bytes(&self.anchor_bytes, verified, authority)
    }
}

impl<'a> RestoreCompletionInputs<'a> {
    fn new(
        preflight: &'a RestorePreflight,
        sidecar: RestoreCompletionSidecar<'a>,
        journal: &'a RetainedRestoreDocument<RestoreJournalDocument>,
        receipt: &'a RetainedRestoreDocument<RestoreReceiptDocument>,
    ) -> Result<Self, RestoreError> {
        validate_restore_operation_nonce(&preflight.operation_nonce)?;
        Ok(Self {
            preflight,
            sidecar,
            journal,
            receipt,
        })
    }

    fn retained_sidecar(
        &self,
    ) -> Result<
        (
            &RetainedVerifiedSidecarTree,
            &RetainedDestinationQuiescence,
            &Path,
        ),
        RestoreError,
    > {
        match &self.sidecar {
            RestoreCompletionSidecar::Existing {
                tree,
                quiescence,
                destination_state,
            } => Ok((tree, quiescence, destination_state)),
            RestoreCompletionSidecar::Committed(committed) => {
                let tree = committed.verified_tree.as_ref().ok_or_else(|| {
                    RestoreError::Interrupted {
                        path: committed.destination_sidecar.clone(),
                        reason: "committed restore lacks its exact retained tree for completion authority"
                            .to_owned(),
                    }
                })?;
                Ok((tree, &committed.quiescence, &committed.destination_state))
            }
        }
    }

    /// Validate every retained component, then reduce the observations to one
    /// immutable canonical generation. The returned document grants no success;
    /// only its later retained protected publication or selection does.
    fn prepare_authority(&self) -> Result<RestoreCompletionAuthorityDocument, RestoreError> {
        verify_current_restore_authorities(self.preflight)?;
        validate_restore_operation_nonce(&self.preflight.operation_nonce)?;
        let expected_journal = restore_journal(
            &self.preflight.plan,
            &self.preflight.staging_path,
            &self.preflight.operation_nonce,
        );
        if self.journal.document() != &expected_journal {
            return Err(RestoreError::Interrupted {
                path: self.preflight.journal_path.clone(),
                reason: "completion inputs observed a different protected journal".to_owned(),
            });
        }
        self.journal.revalidate()?;

        let (tree, quiescence, destination_state) = self.retained_sidecar()?;
        validate_restore_receipt_for_sidecar(
            &self.preflight.plan,
            &self.preflight.operation_nonce,
            tree,
            self.receipt.document(),
        )?;
        self.receipt.revalidate()?;
        let before = quiescence_bound_state_root_identity(&quiescence.guard, destination_state)?;
        if before != quiescence.state_root_identity
            || tree.state_root_identity != quiescence.state_root_identity
        {
            return Err(RestoreError::Tampered {
                reason: "completion inputs lost exact destination quiescence binding".to_owned(),
            });
        }
        tree.revalidate()?;
        let after = quiescence_bound_state_root_identity(&quiescence.guard, destination_state)?;
        if after != quiescence.state_root_identity {
            return Err(RestoreError::Tampered {
                reason: "completion inputs changed destination quiescence generation".to_owned(),
            });
        }
        verify_current_restore_authorities(self.preflight)?;
        self.journal.revalidate()?;
        self.receipt.revalidate()?;
        ensure_project_link_unchanged(&self.preflight.plan)?;

        build_restore_completion_authority(
            self.preflight,
            tree,
            quiescence,
            self.journal,
            self.receipt,
        )
    }
}

impl RetainedVerifiedSidecarTree {
    fn directory_at(&self, relative: &Path) -> io::Result<&RestoreRetainedDirectory> {
        if relative.as_os_str().is_empty() {
            Ok(&self.root)
        } else {
            self.directories.get(relative).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "retained exact sidecar lost a directory capability",
                )
            })
        }
    }

    fn revalidate(&self) -> Result<(), RestoreError> {
        self.parent
            .verify_namespace_identity()
            .map_err(|source| io_error(&self.parent.display_path, source))?;
        self.parent
            .verify_direct_directory_identity(&self.root_leaf, &self.root.identity)
            .map_err(|source| io_error(&self.root_path, source))?;
        self.root
            .verify_identity()
            .map_err(|source| io_error(&self.root_path, source))?;
        let state_root = self
            .directories
            .get(Path::new(DESTINATION_STATE_LEAF))
            .ok_or_else(|| RestoreError::Tampered {
                reason: "retained exact sidecar lost its destination state-root capability"
                    .to_owned(),
            })?;
        if state_root.identity != self.state_root_identity {
            return Err(RestoreError::Tampered {
                reason: "retained exact sidecar state-root capability changed identity".to_owned(),
            });
        }

        for (relative, directory) in &self.directories {
            directory
                .verify_identity()
                .map_err(|source| io_error(&directory.display_path, source))?;
            self.root
                .verify_directory_path_identity(relative, &directory.identity)
                .map_err(|source| io_error(&directory.display_path, source))?;
        }
        for (relative_directory, expected_entries) in &self.namespace {
            let directory = self
                .directory_at(relative_directory)
                .map_err(|source| io_error(&self.root_path.join(relative_directory), source))?;
            let actual = directory
                .direct_entries()
                .map_err(|source| io_error(&directory.display_path, source))?;
            if &actual != expected_entries {
                return Err(RestoreError::Tampered {
                    reason: format!(
                        "retained exact sidecar namespace changed during final sweep: {}",
                        directory.display_path.display()
                    ),
                });
            }
        }
        for (relative, retained) in &self.files {
            if relative.parent().unwrap_or_else(|| Path::new("")) != retained.parent_relative
                || relative.file_name() != Some(retained.leaf.as_os_str())
                || retained.digest != sha256(&retained.bytes)
            {
                return Err(RestoreError::Tampered {
                    reason: "retained exact sidecar file binding changed".to_owned(),
                });
            }
            let maximum = u64::try_from(retained.bytes.len()).unwrap_or(u64::MAX);
            let (handle_bytes, handle_identity) =
                restore_read_retained_file_bounded(&retained.handle, maximum)
                    .map_err(|source| io_error(&self.root_path.join(relative), source))?;
            let parent = self
                .directory_at(&retained.parent_relative)
                .map_err(|source| {
                    io_error(&self.root_path.join(&retained.parent_relative), source)
                })?;
            let (namespace_bytes, namespace_identity) = parent
                .read_direct_file_bounded(&retained.leaf, maximum)
                .map_err(|source| io_error(&self.root_path.join(relative), source))?;
            if handle_identity != retained.identity
                || namespace_identity != retained.identity
                || handle_bytes != retained.bytes
                || namespace_bytes != retained.bytes
                || sha256(&handle_bytes) != retained.digest
                || sha256(&namespace_bytes) != retained.digest
            {
                return Err(RestoreError::Tampered {
                    reason: format!(
                        "retained exact sidecar file changed during final namespace-and-bytes sweep: {}",
                        self.root_path.join(relative).display()
                    ),
                });
            }
        }
        self.root
            .verify_direct_directory_identity(
                Path::new(DESTINATION_STATE_LEAF),
                &self.state_root_identity,
            )
            .map_err(|source| io_error(&state_root.display_path, source))?;
        self.parent
            .verify_direct_directory_identity(&self.root_leaf, &self.root.identity)
            .map_err(|source| io_error(&self.root_path, source))?;
        self.parent
            .verify_namespace_identity()
            .map_err(|source| io_error(&self.parent.display_path, source))
    }
}

impl RetainedProtectedRestoreFile {
    fn revalidate(&self) -> Result<(), RestoreError> {
        verify_restore_authority_relative_path(&self.authority_root, &self.relative, &self.path)?;
        if self.relative.parent().unwrap_or_else(|| Path::new("")) != self.parent_relative
            || self.relative.file_name() != Some(self.leaf.as_os_str())
            || self.digest != sha256(&self.bytes)
        {
            return Err(RestoreError::Tampered {
                reason: "retained protected restore document binding changed".to_owned(),
            });
        }
        self.authority_root
            .verify_namespace_identity()
            .map_err(|source| io_error(&self.authority_root.display_path, source))?;
        self.authority_root
            .verify_directory_path_identity(&self.parent_relative, &self.parent.identity)
            .map_err(|source| io_error(&self.parent.display_path, source))?;
        let maximum = u64::try_from(self.bytes.len()).unwrap_or(u64::MAX);
        let (handle_bytes, handle_identity) =
            restore_read_retained_authority_file_bounded(&self.handle, maximum)
                .map_err(|source| io_error(&self.path, source))?;
        // Reopen the exact leaf only after the retained authority root and
        // parent bindings have succeeded, then compare its identity and bytes.
        let (reopened, reopened_identity) = self
            .parent
            .open_direct_authority_file_retained(&self.leaf)
            .map_err(|source| io_error(&self.path, source))?;
        let (namespace_bytes, namespace_identity) =
            restore_read_retained_authority_file_bounded(&reopened, maximum)
                .map_err(|source| io_error(&self.path, source))?;
        if handle_identity != self.identity
            || reopened_identity != self.identity
            || namespace_identity != self.identity
            || handle_bytes != self.bytes
            || namespace_bytes != self.bytes
            || sha256(&handle_bytes) != self.digest
            || sha256(&namespace_bytes) != self.digest
        {
            return Err(RestoreError::Tampered {
                reason: format!(
                    "protected restore document changed during retained final sweep: {}",
                    self.path.display()
                ),
            });
        }
        self.authority_root
            .verify_directory_path_identity(&self.parent_relative, &self.parent.identity)
            .map_err(|source| io_error(&self.parent.display_path, source))?;
        self.authority_root
            .verify_namespace_identity()
            .map_err(|source| io_error(&self.authority_root.display_path, source))
    }
}

impl<T> RetainedRestoreDocument<T>
where
    T: serde::de::DeserializeOwned + PartialEq,
{
    fn document(&self) -> &T {
        &self.document
    }

    fn revalidate(&self) -> Result<(), RestoreError> {
        self.retained.revalidate()?;
        let reparsed: T = serde_json::from_slice(&self.retained.bytes).map_err(|error| {
            RestoreError::Tampered {
                reason: format!("retained protected restore document no longer parses: {error}"),
            }
        })?;
        if reparsed != self.document {
            return Err(RestoreError::Tampered {
                reason: "retained protected restore document parse changed".to_owned(),
            });
        }
        self.retained.revalidate()
    }
}

impl RetainedRestoreCompletion {
    fn revalidate(&self) -> Result<(), RestoreError> {
        if self.selector.retained.authority_root.identity
            != self.completion.retained.authority_root.identity
            || self.selector.retained.authority_root.display_path
                != self.completion.retained.authority_root.display_path
        {
            return Err(RestoreError::Tampered {
                reason: "restore completion selector and record lost their shared retained authority root"
                    .to_owned(),
            });
        }
        self.selector.revalidate()?;
        self.completion_anchor
            .revalidate()
            .map_err(|source| io_error(&self.completion.retained.path, source))?;
        self.completion_anchor
            .validate_retained_file(
                &self.completion.retained.handle,
                &self.completion.retained.identity,
            )
            .map_err(|source| io_error(&self.completion.retained.path, source))?;
        self.completion.revalidate()?;
        let completion_directory =
            self.completion
                .retained
                .relative
                .parent()
                .ok_or_else(|| RestoreError::Tampered {
                    reason: "retained restore completion content address lost its parent"
                        .to_owned(),
                })?;
        validate_restore_completion_record_namespace(&self.completion)?;
        let selector_relative = &self.selector.retained.relative;
        validate_restore_completion_selector(
            &self.selector.document,
            &self.completion.document,
            &self.selector.retained.authority_root,
            completion_directory,
            selector_relative,
        )?;
        self.selector.revalidate()
    }
}

impl RestorePublicationIsolation {
    fn revalidate(
        &self,
        parent: &RestoreRetainedDirectory,
        destination_leaf: &Path,
        expected_recovery: &crate::retained_dir::RetainedFileIdentity,
    ) -> io::Result<()> {
        parent.verify_namespace_identity()?;
        parent.verify_direct_directory_identity(
            destination_leaf,
            &self.authoritative_placeholder.identity,
        )?;
        parent.verify_direct_directory_identity(&self.recovery_leaf, expected_recovery)?;
        if self.recovery_root.identity != *expected_recovery {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore publication recovery marker retained the wrong directory",
            ));
        }
        self.authoritative_placeholder.verify_identity()?;
        self.recovery_root.verify_identity()?;
        if !self.authoritative_placeholder.direct_entries()?.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore publication blocker placeholder is not empty",
            ));
        }
        parent.verify_direct_directory_identity(
            destination_leaf,
            &self.authoritative_placeholder.identity,
        )?;
        parent.verify_direct_directory_identity(&self.recovery_leaf, expected_recovery)?;
        parent.verify_namespace_identity()
    }

    fn description(&self, parent: &RestoreRetainedDirectory, destination_leaf: &Path) -> String {
        format!(
            "exact Store-created placeholder {} with retained recovery marker {}",
            parent.display_path.join(destination_leaf).display(),
            parent.display_path.join(&self.recovery_leaf).display()
        )
    }
}

impl RetainedCommittedSidecar {
    fn install_verified_tree(
        &mut self,
        tree: RetainedVerifiedSidecarTree,
    ) -> Result<(), RestoreError> {
        if tree.root.identity != self.root.identity
            || tree.root_path != self.destination_sidecar
            || tree.state_root_identity != self.quiescence.state_root_identity
        {
            return Err(RestoreError::Tampered {
                reason: "retained committed sidecar verification bound a different publication"
                    .to_owned(),
            });
        }
        tree.revalidate()?;
        self.verified_tree = Some(tree);
        self.revalidate()
    }

    fn revalidate(&self) -> Result<(), RestoreError> {
        self.parent
            .verify_namespace_identity()
            .map_err(|source| io_error(&self.parent.display_path, source))?;
        self.parent
            .verify_direct_directory_identity(&self.destination_leaf, &self.root.identity)
            .map_err(|source| io_error(&self.destination_sidecar, source))?;
        self.root
            .verify_identity()
            .map_err(|source| io_error(&self.destination_sidecar, source))?;
        let current =
            quiescence_bound_state_root_identity(&self.quiescence.guard, &self.destination_state)?;
        if current != self.quiescence.state_root_identity {
            return Err(RestoreError::Tampered {
                reason: "retained committed sidecar quiescence changed exact state-root identity"
                    .to_owned(),
            });
        }
        let tree = self
            .verified_tree
            .as_ref()
            .ok_or_else(|| RestoreError::Interrupted {
                path: self.destination_sidecar.clone(),
                reason: "retained committed sidecar has not completed exact full-tree verification"
                    .to_owned(),
            })?;
        tree.revalidate()
    }

    fn isolate_after_error(&self, cause: RestoreError) -> RestoreError {
        match restore_vacate_retained_publication(
            &self.parent,
            &self.destination_leaf,
            &self.root,
        ) {
            Ok(isolation) => RestoreError::Interrupted {
                path: self.destination_sidecar.clone(),
                reason: format!(
                    "restore failed after retained directory publication ({cause}); authoritative publication was replaced by {isolation}"
                ),
            },
            Err(isolation) => RestoreError::Interrupted {
                path: self.destination_sidecar.clone(),
                reason: format!(
                    "restore failed after retained directory publication ({cause}); exact committed-sidecar isolation failed: {isolation}"
                ),
            },
        }
    }
}

fn restore_direct_component(path: &Path) -> io::Result<&std::ffi::OsStr> {
    let mut components = path.components();
    let Some(Component::Normal(value)) = components.next() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "restore operation requires one normalized direct child",
        ));
    };
    if components.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "restore operation requires one normalized direct child",
        ));
    }
    Ok(value)
}

fn restore_relative_components(path: &Path) -> io::Result<Vec<std::ffi::OsString>> {
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "restore path is not a normalized relative child",
            ));
        };
        components.push(value.to_os_string());
    }
    Ok(components)
}

fn validate_restore_directory_handle(file: &File) -> io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained restore directory is not a directory",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        if metadata.file_attributes() & 0x400 == 0x400 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained restore directory is a reparse point",
            ));
        }
    }
    Ok(())
}

fn validate_restore_file_handle(
    file: &File,
) -> io::Result<crate::retained_dir::RetainedFileIdentity> {
    let metadata = file.metadata()?;
    let valid = metadata.is_file() && retained_hard_link_count(file, &metadata)? == 1;
    #[cfg(windows)]
    let valid = {
        use std::os::windows::fs::MetadataExt as _;
        valid && metadata.file_attributes() & 0x400 != 0x400
    };
    if !valid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "restore leaf is linked, reparse-backed, or not a regular file",
        ));
    }
    crate::retained_dir::RetainedDirectory::identity_of(file)
}

fn validate_restore_authority_file_handle(
    file: &File,
) -> io::Result<crate::retained_dir::RetainedFileIdentity> {
    let metadata = file.metadata()?;
    let link_count = retained_hard_link_count(file, &metadata)?;
    #[cfg(unix)]
    let valid = metadata.is_file() && link_count >= 1;
    #[cfg(windows)]
    let valid = {
        use std::os::windows::fs::MetadataExt as _;
        metadata.is_file() && link_count == 1 && metadata.file_attributes() & 0x400 != 0x400
    };
    #[cfg(not(any(unix, windows)))]
    let valid = false;
    if !valid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "restore authority leaf is reparse-backed or not a retained regular file with a supported link shape",
        ));
    }
    crate::retained_dir::RetainedDirectory::identity_of(file)
}

fn restore_read_retained_file_bounded(
    retained: &File,
    maximum: u64,
) -> io::Result<(Vec<u8>, crate::retained_dir::RetainedFileIdentity)> {
    let identity = validate_restore_file_handle(retained)?;
    let before = retained.metadata()?;
    if before.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "retained restore file exceeds its declared byte length",
        ));
    }
    let mut reader = retained.try_clone()?;
    reader.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    std::io::Read::by_ref(&mut reader)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "retained restore file exceeds its declared byte length",
        ));
    }
    let after_identity = validate_restore_file_handle(retained)?;
    let after = retained.metadata()?;
    if after_identity != identity
        || after.len() != before.len()
        || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained restore file changed while it was read",
        ));
    }
    Ok((bytes, identity))
}

fn restore_read_retained_authority_file_bounded(
    retained: &File,
    maximum: u64,
) -> io::Result<(Vec<u8>, crate::retained_dir::RetainedFileIdentity)> {
    let identity = validate_restore_authority_file_handle(retained)?;
    let before = retained.metadata()?;
    if before.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "retained restore authority file exceeds its declared byte length",
        ));
    }
    let mut reader = retained.try_clone()?;
    reader.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    std::io::Read::by_ref(&mut reader)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "retained restore authority file exceeds its declared byte length",
        ));
    }
    let after_identity = validate_restore_authority_file_handle(retained)?;
    let after = retained.metadata()?;
    if after_identity != identity
        || after.len() != before.len()
        || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained restore authority file changed while it was read",
        ));
    }
    Ok((bytes, identity))
}

#[cfg(unix)]
fn restore_open_root_directory(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY)
        .open(path)
}

#[cfg(windows)]
fn restore_open_root_directory(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0x0000_0001 | 0x0000_0002)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    validate_restore_directory_handle(&file)?;
    Ok(file)
}

#[cfg(not(any(unix, windows)))]
fn restore_open_root_directory(_path: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "retained restore directories are unsupported on this platform",
    ))
}

#[cfg(unix)]
fn restore_open_relative(
    directory: &File,
    leaf: &Path,
    mode: RestoreRelativeOpen,
) -> io::Result<File> {
    use std::os::fd::{AsRawFd as _, FromRawFd as _};
    use std::os::unix::ffi::OsStrExt as _;
    let leaf = restore_direct_component(leaf)?;
    let name = std::ffi::CString::new(leaf.as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "restore path component contains NUL",
        )
    })?;
    if matches!(mode, RestoreRelativeOpen::DirectoryCreateNew) {
        // SAFETY: the retained directory descriptor and direct child name remain live for mkdirat.
        if unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let flags = match mode {
        RestoreRelativeOpen::Directory | RestoreRelativeOpen::DirectoryCreateNew => {
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC
        }
        RestoreRelativeOpen::FileRead => {
            libc::O_RDONLY | libc::O_NONBLOCK | libc::O_NOFOLLOW | libc::O_CLOEXEC
        }
        RestoreRelativeOpen::FileReadWrite => {
            libc::O_RDWR | libc::O_NONBLOCK | libc::O_NOFOLLOW | libc::O_CLOEXEC
        }
        RestoreRelativeOpen::FileReadWriteCreate => {
            libc::O_RDWR | libc::O_CREAT | libc::O_NONBLOCK | libc::O_NOFOLLOW | libc::O_CLOEXEC
        }
        RestoreRelativeOpen::FileWriteNew => {
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC
        }
    };
    // SAFETY: the retained descriptor and NUL-terminated direct child name are valid;
    // a successful call returns one newly owned descriptor.
    let descriptor = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags, 0o600) };
    if descriptor < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }
}

#[cfg(windows)]
fn restore_open_relative(
    directory: &File,
    leaf: &Path,
    mode: RestoreRelativeOpen,
) -> io::Result<File> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt as _;
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, RawHandle};

    type Handle = *mut c_void;
    const OBJ_CASE_INSENSITIVE: u32 = 0x40;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const DELETE_ACCESS: u32 = 0x0001_0000;
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    const FILE_OPEN: u32 = 1;
    const FILE_CREATE: u32 = 2;
    const FILE_OPEN_IF: u32 = 3;
    const FILE_DIRECTORY_FILE: u32 = 0x1;
    const FILE_NON_DIRECTORY_FILE: u32 = 0x40;
    const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x20;
    const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

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
        security_descriptor: *mut c_void,
        security_quality_of_service: *mut c_void,
    }
    #[repr(C)]
    union IoStatusValue {
        status: i32,
        pointer: *mut c_void,
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
            ea_buffer: *mut c_void,
            ea_length: u32,
        ) -> i32;
        fn RtlNtStatusToDosError(status: i32) -> u32;
    }

    let leaf = restore_direct_component(leaf)?;
    let mut wide = leaf.encode_wide().collect::<Vec<_>>();
    let byte_len = wide
        .len()
        .checked_mul(2)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "restore leaf too long"))?;
    let mut name = UnicodeString {
        length: byte_len,
        maximum_length: byte_len,
        buffer: wide.as_mut_ptr(),
    };
    let mut attributes = ObjectAttributes {
        length: u32::try_from(std::mem::size_of::<ObjectAttributes>())
            .expect("OBJECT_ATTRIBUTES size fits u32"),
        root_directory: directory.as_raw_handle().cast(),
        object_name: &mut name,
        attributes: OBJ_CASE_INSENSITIVE,
        security_descriptor: std::ptr::null_mut(),
        security_quality_of_service: std::ptr::null_mut(),
    };
    let (access, disposition, options) = match mode {
        RestoreRelativeOpen::Directory => (
            FILE_READ_ATTRIBUTES | GENERIC_READ | GENERIC_WRITE,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
        ),
        RestoreRelativeOpen::DirectoryCreateNew => (
            FILE_READ_ATTRIBUTES | GENERIC_READ | GENERIC_WRITE,
            FILE_CREATE,
            FILE_DIRECTORY_FILE,
        ),
        RestoreRelativeOpen::FileRead => (GENERIC_READ, FILE_OPEN, FILE_NON_DIRECTORY_FILE),
        RestoreRelativeOpen::FileReadWrite => (
            GENERIC_READ | GENERIC_WRITE,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
        ),
        RestoreRelativeOpen::FileReadWriteCreate => (
            GENERIC_READ | GENERIC_WRITE,
            FILE_OPEN_IF,
            FILE_NON_DIRECTORY_FILE,
        ),
        RestoreRelativeOpen::FileWriteNew => (
            GENERIC_READ | GENERIC_WRITE,
            FILE_CREATE,
            FILE_NON_DIRECTORY_FILE,
        ),
        RestoreRelativeOpen::FileReadSharedDelete => {
            (GENERIC_READ, FILE_OPEN, FILE_NON_DIRECTORY_FILE)
        }
        RestoreRelativeOpen::FileDelete => (
            DELETE_ACCESS | FILE_READ_ATTRIBUTES | GENERIC_READ,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
        ),
        RestoreRelativeOpen::DirectoryDelete => (
            DELETE_ACCESS | FILE_READ_ATTRIBUTES | GENERIC_READ,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
        ),
    };
    let share_access = match mode {
        RestoreRelativeOpen::Directory
        | RestoreRelativeOpen::DirectoryCreateNew
        | RestoreRelativeOpen::FileReadSharedDelete => {
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
        }
        RestoreRelativeOpen::FileRead
        | RestoreRelativeOpen::FileReadWrite
        | RestoreRelativeOpen::FileReadWriteCreate
        | RestoreRelativeOpen::FileWriteNew
        | RestoreRelativeOpen::FileDelete
        | RestoreRelativeOpen::DirectoryDelete => FILE_SHARE_READ | FILE_SHARE_WRITE,
    };
    let mut status = IoStatusBlock {
        value: IoStatusValue { status: 0 },
        information: 0,
    };
    let mut handle: Handle = std::ptr::null_mut();
    // SAFETY: every pointer references initialized storage for this synchronous call.
    let result = unsafe {
        NtCreateFile(
            &mut handle,
            access | SYNCHRONIZE,
            &mut attributes,
            &mut status,
            std::ptr::null_mut(),
            0,
            share_access,
            disposition,
            options | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null_mut(),
            0,
        )
    };
    if result < 0 {
        // SAFETY: pure NTSTATUS conversion.
        let code = unsafe { RtlNtStatusToDosError(result) };
        return Err(io::Error::from_raw_os_error(
            i32::try_from(code).unwrap_or(i32::MAX),
        ));
    }
    // SAFETY: successful NtCreateFile returns one newly owned handle.
    let file = unsafe { File::from_raw_handle(handle as RawHandle) };
    match mode {
        RestoreRelativeOpen::Directory
        | RestoreRelativeOpen::DirectoryCreateNew
        | RestoreRelativeOpen::DirectoryDelete => validate_restore_directory_handle(&file)?,
        RestoreRelativeOpen::FileRead
        | RestoreRelativeOpen::FileReadWrite
        | RestoreRelativeOpen::FileReadWriteCreate
        | RestoreRelativeOpen::FileWriteNew
        | RestoreRelativeOpen::FileReadSharedDelete
        | RestoreRelativeOpen::FileDelete => {
            validate_restore_file_handle(&file)?;
        }
    }
    Ok(file)
}

#[cfg(not(any(unix, windows)))]
fn restore_open_relative(
    _directory: &File,
    _leaf: &Path,
    _mode: RestoreRelativeOpen,
) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative restore opens are unsupported on this platform",
    ))
}

#[cfg(unix)]
fn restore_direct_directory_entries(directory: &File) -> io::Result<Vec<PathBuf>> {
    use std::os::unix::ffi::OsStringExt as _;
    let mut stream = rustix::fs::Dir::read_from(directory)
        .map_err(|error| io::Error::from_raw_os_error(error.raw_os_error()))?;
    let mut entries = Vec::new();
    for entry in &mut stream {
        let entry = entry.map_err(|error| io::Error::from_raw_os_error(error.raw_os_error()))?;
        let name = entry.file_name().to_bytes();
        if name != b"." && name != b".." {
            entries.push(PathBuf::from(std::ffi::OsString::from_vec(name.to_vec())));
        }
    }
    Ok(entries)
}

#[cfg(windows)]
fn restore_direct_directory_entries(directory: &File) -> io::Result<Vec<PathBuf>> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStringExt as _;
    use std::os::windows::io::AsRawHandle as _;
    #[repr(C)]
    struct IoStatusBlock {
        status: isize,
        information: usize,
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQueryDirectoryFile(
            file_handle: *mut c_void,
            event: *mut c_void,
            apc_routine: *mut c_void,
            apc_context: *mut c_void,
            io_status_block: *mut IoStatusBlock,
            file_information: *mut c_void,
            length: u32,
            file_information_class: u32,
            return_single_entry: u8,
            file_name: *mut c_void,
            restart_scan: u8,
        ) -> i32;
        fn RtlNtStatusToDosError(status: i32) -> u32;
    }
    const STATUS_NO_MORE_FILES: i32 = 0x8000_0006_u32 as i32;
    let mut entries = Vec::new();
    let mut restart = 1_u8;
    loop {
        let mut buffer = vec![0_u8; 64 * 1024];
        let mut io_status = IoStatusBlock {
            status: 0,
            information: 0,
        };
        // SAFETY: the retained handle and output buffer remain live for the call.
        let status = unsafe {
            NtQueryDirectoryFile(
                directory.as_raw_handle().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut io_status,
                buffer.as_mut_ptr().cast(),
                u32::try_from(buffer.len()).expect("directory buffer fits u32"),
                12,
                0,
                std::ptr::null_mut(),
                restart,
            )
        };
        restart = 0;
        if status == STATUS_NO_MORE_FILES {
            return Ok(entries);
        }
        if status < 0 {
            // SAFETY: pure NTSTATUS conversion.
            let code = unsafe { RtlNtStatusToDosError(status) };
            return Err(io::Error::from_raw_os_error(
                i32::try_from(code).unwrap_or(i32::MAX),
            ));
        }
        let mut offset = 0_usize;
        while offset < io_status.information {
            let record = &buffer[offset..io_status.information];
            if record.len() < 12 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "truncated retained restore directory entry",
                ));
            }
            let next = u32::from_ne_bytes(record[0..4].try_into().expect("slice length"));
            let name_bytes = usize::try_from(u32::from_ne_bytes(
                record[8..12].try_into().expect("slice length"),
            ))
            .expect("u32 fits usize");
            let end = 12_usize.checked_add(name_bytes).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "directory entry overflow")
            })?;
            if end > record.len() || name_bytes % 2 != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid retained restore directory entry",
                ));
            }
            let wide = record[12..end]
                .chunks_exact(2)
                .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            let name = std::ffi::OsString::from_wide(&wide);
            if name != "." && name != ".." {
                entries.push(PathBuf::from(name));
            }
            if next == 0 {
                break;
            }
            offset = offset
                .checked_add(usize::try_from(next).expect("u32 fits usize"))
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "directory offset overflow")
                })?;
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn restore_direct_directory_entries(_directory: &File) -> io::Result<Vec<PathBuf>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative restore enumeration is unsupported on this platform",
    ))
}

#[cfg(unix)]
fn restore_remove_relative(
    parent: &File,
    leaf: &Path,
    retained: File,
    directory: bool,
) -> io::Result<()> {
    restore_direct_component(leaf)?;
    let expected = crate::retained_dir::RetainedDirectory::identity_of(&retained)?;
    let quarantine = restore_quarantine_relative_noreplace(parent, leaf, "cleanup-orphan")?;
    let quarantined = restore_open_relative(
        parent,
        &quarantine,
        if directory {
            RestoreRelativeOpen::Directory
        } else {
            RestoreRelativeOpen::FileRead
        },
    )?;
    let quarantined_identity = if directory {
        validate_restore_directory_handle(&quarantined)?;
        if !restore_direct_directory_entries(&quarantined)?.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::DirectoryNotEmpty,
                "refusing to isolate a nonempty quarantined restore directory",
            ));
        }
        crate::retained_dir::RetainedDirectory::identity_of(&quarantined)?
    } else {
        validate_restore_file_handle(&quarantined)?
    };
    if quarantined_identity != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "refusing to accept a substituted quarantined restore entry",
        ));
    }
    if crate::retained_dir::RetainedDirectory::identity_of(&retained)? != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained restore cleanup handle changed after quarantine",
        ));
    }
    let mut authoritative_leaf_vacated = false;
    for _ in 0..32 {
        match restore_quarantine_relative_noreplace(parent, leaf, "cleanup-repopulation") {
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                authoritative_leaf_vacated = true;
                break;
            }
            Err(source) => return Err(source),
        }
    }
    if !authoritative_leaf_vacated {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "bounded restore cleanup isolation was continuously repopulated",
        ));
    }
    parent.sync_all()?;
    let reopened = restore_open_relative(
        parent,
        &quarantine,
        if directory {
            RestoreRelativeOpen::Directory
        } else {
            RestoreRelativeOpen::FileRead
        },
    )?;
    let reopened_identity = if directory {
        validate_restore_directory_handle(&reopened)?;
        if !restore_direct_directory_entries(&reopened)?.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::DirectoryNotEmpty,
                "quarantined restore directory changed before orphan retention",
            ));
        }
        crate::retained_dir::RetainedDirectory::identity_of(&reopened)?
    } else {
        validate_restore_file_handle(&reopened)?
    };
    if reopened_identity != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "quarantined restore entry changed before orphan retention",
        ));
    }
    // The supported Unix targets used here do not expose a directory-entry deletion
    // primitive bound atomically to this exact retained handle. Keep the verified
    // quarantine as a bounded orphan instead of unlinking a mutable name after a check.
    Ok(())
}

#[cfg(windows)]
fn restore_remove_relative(
    _parent: &File,
    _leaf: &Path,
    retained: File,
    _directory: bool,
) -> io::Result<()> {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle as _;
    #[repr(C)]
    struct FileDispositionInfo {
        delete_file: i32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetFileInformationByHandle(
            handle: *mut c_void,
            information_class: u32,
            information: *const c_void,
            buffer_size: u32,
        ) -> i32;
    }
    let disposition = FileDispositionInfo { delete_file: 1 };
    // SAFETY: retained is a live exact-entry handle and disposition has the documented layout.
    let result = unsafe {
        SetFileInformationByHandle(
            retained.as_raw_handle().cast(),
            4,
            (&raw const disposition).cast(),
            u32::try_from(std::mem::size_of::<FileDispositionInfo>())
                .expect("disposition size fits u32"),
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
fn restore_remove_relative(
    _parent: &File,
    _leaf: &Path,
    _retained: File,
    _directory: bool,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative restore cleanup is unsupported on this platform",
    ))
}

fn stage_restore_retained(
    staging_parent: &RestoreRetainedDirectory,
    staging_leaf: &Path,
    staging_path: &Path,
    members: &[RestoreMember],
    manifest: &BackupManifestDocument,
) -> Result<ValidatedStagingTree, RestoreError> {
    staging_parent
        .verify_namespace_identity()
        .map_err(|source| io_error(&staging_parent.display_path, source))?;
    let root = match staging_parent
        .open_optional_directory(staging_leaf)
        .map_err(|source| io_error(staging_path, source))?
    {
        Some(root) => verify_staging_prefix_retained(root, members)?.root,
        None => staging_parent
            .create_directory_new(staging_leaf)
            .map_err(|source| {
                if source.kind() == io::ErrorKind::AlreadyExists {
                    RestoreError::Collision {
                        path: staging_path.to_path_buf(),
                        reason: "restore staging appeared during retained creation".to_owned(),
                    }
                } else {
                    io_error(staging_path, source)
                }
            })?,
    };
    staging_parent
        .verify_direct_directory_identity(staging_leaf, &root.identity)
        .map_err(|source| io_error(staging_path, source))?;
    for member in members {
        let destination = staging_path.join(&member.relative_destination);
        if let Some((raw, _)) = root
            .read_optional_file_bounded(&member.relative_destination, member.entry.byte_length)
            .map_err(|source| RestoreError::Interrupted {
                path: destination.clone(),
                reason: format!(
                    "staged restore member is linked, special, redirected, or changed: {source}"
                ),
            })?
        {
            if raw != member.bytes || sha256(&raw) != member.entry.sha256 {
                return Err(RestoreError::Interrupted {
                    path: destination,
                    reason: "staged restore member was substituted".to_owned(),
                });
            }
        } else {
            if let Some(parent) = member.relative_destination.parent() {
                if !parent.as_os_str().is_empty() {
                    root.create_dir_all(parent)
                        .map_err(|source| io_error(&destination, source))?;
                }
            }
            root.write_file_new_validated(&member.relative_destination, &member.bytes)
                .map_err(|source| io_error(&destination, source))?;
        }
        staging_parent
            .verify_direct_directory_identity(staging_leaf, &root.identity)
            .map_err(|source| io_error(staging_path, source))?;
    }
    root.sync_tree()
        .map_err(|source| io_error(staging_path, source))?;
    staging_parent
        .sync_self()
        .map_err(|source| io_error(&staging_parent.display_path, source))?;
    staging_parent
        .verify_direct_directory_identity(staging_leaf, &root.identity)
        .map_err(|source| io_error(staging_path, source))?;
    staging_parent
        .verify_namespace_identity()
        .map_err(|source| io_error(&staging_parent.display_path, source))?;
    verify_staging_exact_retained(root, members, manifest)
}

#[cfg(test)]
fn stage_restore(
    staging: &Path,
    members: &[RestoreMember],
    manifest: &BackupManifestDocument,
) -> Result<(), RestoreError> {
    let parent_path = staging.parent().ok_or_else(|| RestoreError::InvalidPath {
        path: staging.to_path_buf(),
        reason: "restore staging has no parent".to_owned(),
    })?;
    let leaf = staging
        .file_name()
        .ok_or_else(|| RestoreError::InvalidPath {
            path: staging.to_path_buf(),
            reason: "restore staging has no leaf".to_owned(),
        })?;
    let parent = RestoreRetainedDirectory::open_root(parent_path)
        .map_err(|source| io_error(parent_path, source))?;
    stage_restore_retained(&parent, Path::new(leaf), staging, members, manifest).map(|_| ())
}

fn verify_sidecar_exact_retained(
    parent: &RestoreRetainedDirectory,
    leaf: &Path,
    root_path: &Path,
    members: &[RestoreMember],
    manifest: &BackupManifestDocument,
    state_root_identity: &crate::retained_dir::RetainedFileIdentity,
) -> Result<RetainedVerifiedSidecarTree, RestoreError> {
    parent
        .verify_namespace_identity()
        .map_err(|source| io_error(&parent.display_path, source))?;
    let root = match parent.open_optional_directory(leaf) {
        Ok(Some(root)) => root,
        Ok(None) => {
            return Err(RestoreError::Interrupted {
                path: root_path.to_path_buf(),
                reason: "restored sidecar is absent from its retained parent".to_owned(),
            });
        }
        Err(_) => {
            return Err(RestoreError::Collision {
                path: root_path.to_path_buf(),
                reason: "restored sidecar is linked, special, or not a retained directory"
                    .to_owned(),
            });
        }
    };
    let state_root_leaf = Path::new(DESTINATION_STATE_LEAF);
    let state_root = root
        .open_directory(state_root_leaf)
        .map_err(|source| RestoreError::Tampered {
            reason: format!(
                "quiescence-bound destination state root is absent or unsafe during exact verification: {source}"
            ),
        })?;
    if state_root.identity != *state_root_identity {
        return Err(RestoreError::Tampered {
            reason: "destination state root differs from the exact quiescence-bound identity"
                .to_owned(),
        });
    }
    root.verify_direct_directory_identity(state_root_leaf, state_root_identity)
        .map_err(|source| io_error(&state_root.display_path, source))?;
    let expected = members
        .iter()
        .map(|member| (member.relative_destination.clone(), member))
        .collect::<BTreeMap<_, _>>();
    let mut files = BTreeMap::new();
    let mut directories = BTreeMap::new();
    let mut namespace = BTreeMap::new();
    collect_retained_sidecar(
        &root,
        Path::new(""),
        &expected,
        &mut files,
        &mut directories,
        &mut namespace,
        Some(&state_root),
    )?;
    for relative in expected.keys() {
        if !files.contains_key(relative) {
            return Err(RestoreError::Tampered {
                reason: format!(
                    "restored member is absent from retained sidecar: {}",
                    root_path.join(relative).display()
                ),
            });
        }
    }
    for relative in files.keys() {
        if expected.contains_key(relative) {
            continue;
        }
        let logical = format!("sidecar/{}", slash_path(relative)?);
        let exclusion = BackupManifestDocument::explicit_source_exclusion(
            &logical,
            &manifest.backup_manifest.project.archive_layout,
        )
        .map_err(|error| RestoreError::Tampered {
            reason: format!("destination exclusion classification failed: {error:?}"),
        })?;
        if exclusion != Some(BackupSourceExclusion::ProducerLock) {
            return Err(RestoreError::Collision {
                path: root_path.join(relative),
                reason: "destination contains extra private, partial, crash, or unclassified state"
                    .to_owned(),
            });
        }
    }
    let mut allowed_directories = BTreeSet::from([
        PathBuf::from(DESTINATION_STATE_LEAF),
        PathBuf::from(DESTINATION_STATE_LEAF).join("locks"),
    ]);
    for relative in expected.keys() {
        let mut expected_parent = relative.parent();
        while let Some(value) = expected_parent {
            if value.as_os_str().is_empty() {
                break;
            }
            allowed_directories.insert(value.to_path_buf());
            expected_parent = value.parent();
        }
    }
    if let Some(extra) = directories
        .keys()
        .find(|directory| !allowed_directories.contains(*directory))
    {
        return Err(RestoreError::Collision {
            path: root_path.join(extra),
            reason: "destination contains an extra empty or unclassified directory".to_owned(),
        });
    }
    let retained = RetainedVerifiedSidecarTree {
        parent: parent
            .try_clone()
            .map_err(|source| io_error(&parent.display_path, source))?,
        root_leaf: leaf.to_path_buf(),
        root_path: root_path.to_path_buf(),
        root,
        state_root_identity: state_root_identity.clone(),
        directories,
        files,
        namespace,
    };
    retained.revalidate()?;
    Ok(retained)
}

fn collect_retained_sidecar(
    directory: &RestoreRetainedDirectory,
    relative_directory: &Path,
    expected: &BTreeMap<PathBuf, &RestoreMember>,
    files: &mut BTreeMap<PathBuf, RetainedVerifiedSidecarFile>,
    directories: &mut BTreeMap<PathBuf, RestoreRetainedDirectory>,
    namespace: &mut BTreeMap<PathBuf, Vec<PathBuf>>,
    pinned_state_root: Option<&RestoreRetainedDirectory>,
) -> Result<(), RestoreError> {
    let entries = directory
        .direct_entries()
        .map_err(|source| io_error(&directory.display_path, source))?;
    namespace.insert(relative_directory.to_path_buf(), entries.clone());
    for leaf in entries {
        let relative = relative_directory.join(&leaf);
        let pinned_child = if relative_directory.as_os_str().is_empty()
            && leaf.as_path() == Path::new(DESTINATION_STATE_LEAF)
        {
            pinned_state_root
                .map(RestoreRetainedDirectory::try_clone)
                .transpose()
                .map_err(|source| io_error(&directory.display_path.join(&leaf), source))?
        } else {
            None
        };
        let opened_child = match pinned_child {
            Some(child) => Ok(child),
            None => directory.open_directory(&leaf),
        };
        if let Ok(child) = opened_child {
            collect_retained_sidecar(
                &child,
                &relative,
                expected,
                files,
                directories,
                namespace,
                None,
            )?;
            directory
                .verify_direct_directory_identity(&leaf, &child.identity)
                .map_err(|source| io_error(&child.display_path, source))?;
            if directories.insert(relative.clone(), child).is_some() {
                return Err(RestoreError::Tampered {
                    reason: "retained exact sidecar contained a duplicate directory binding"
                        .to_owned(),
                });
            }
        } else {
            let path = directory.display_path.join(&leaf);
            let maximum = expected
                .get(&relative)
                .map_or(MAX_RESTORE_AUTHORITY_BYTES, |member| {
                    member.entry.byte_length
                });
            let (handle, identity) = directory
                .open_direct_file_retained(&leaf)
                .map_err(|source| io_error(&path, source))?;
            let (raw, retained_identity) = restore_read_retained_file_bounded(&handle, maximum)
                .map_err(|source| io_error(&path, source))?;
            if retained_identity != identity
                || directory
                    .direct_file_identity(&leaf)
                    .map_err(|source| io_error(&path, source))?
                    != identity
            {
                return Err(RestoreError::Tampered {
                    reason: format!(
                        "restored entry changed before its capability was retained: {}",
                        path.display()
                    ),
                });
            }
            if let Some(member) = expected.get(&relative) {
                if raw != member.bytes || sha256(&raw) != member.entry.sha256 {
                    return Err(RestoreError::Tampered {
                        reason: format!("restored member changed: {}", path.display()),
                    });
                }
            }
            let retained = RetainedVerifiedSidecarFile {
                parent_relative: relative_directory.to_path_buf(),
                leaf: leaf.clone(),
                handle,
                identity,
                digest: sha256(&raw),
                bytes: raw,
            };
            if files.insert(relative, retained).is_some() {
                return Err(RestoreError::Tampered {
                    reason: "retained exact sidecar contained a duplicate file binding".to_owned(),
                });
            }
        }
    }
    let final_entries = directory
        .direct_entries()
        .map_err(|source| io_error(&directory.display_path, source))?;
    if namespace.get(relative_directory) != Some(&final_entries) {
        return Err(RestoreError::Tampered {
            reason: format!(
                "restore sidecar namespace changed while capabilities were retained: {}",
                directory.display_path.display()
            ),
        });
    }
    directory
        .verify_identity()
        .map_err(|source| io_error(&directory.display_path, source))
}

#[cfg(test)]
fn verify_sidecar_exact(
    root: &Path,
    members: &[RestoreMember],
    manifest: &BackupManifestDocument,
) -> Result<(), RestoreError> {
    let metadata = fs::symlink_metadata(root).map_err(|source| io_error(root, source))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(RestoreError::Collision {
            path: root.to_path_buf(),
            reason: "sidecar is not a real directory".to_owned(),
        });
    }
    let expected = members
        .iter()
        .map(|member| (member.relative_destination.clone(), member))
        .collect::<BTreeMap<_, _>>();
    for (relative, member) in &expected {
        let path = root.join(relative);
        let raw = read_nofollow_bounded(&path, member.entry.byte_length)?;
        if raw != member.bytes || sha256(&raw) != member.entry.sha256 {
            return Err(RestoreError::Tampered {
                reason: format!("restored member changed: {}", path.display()),
            });
        }
    }
    let mut observed = Vec::new();
    walk_sidecar_files(root, root, &mut observed)?;
    reject_unexpected_directories(root, &expected)?;
    for relative in observed {
        if expected.contains_key(&relative) {
            continue;
        }
        let logical = format!("sidecar/{}", slash_path(&relative)?);
        let exclusion = BackupManifestDocument::explicit_source_exclusion(
            &logical,
            &manifest.backup_manifest.project.archive_layout,
        )
        .map_err(|error| RestoreError::Tampered {
            reason: format!("destination exclusion classification failed: {error:?}"),
        })?;
        if exclusion != Some(BackupSourceExclusion::ProducerLock) {
            return Err(RestoreError::Collision {
                path: root.join(relative),
                reason: "destination contains extra private, partial, crash, or unclassified state"
                    .to_owned(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
fn verify_staging_prefix(root: &Path, members: &[RestoreMember]) -> Result<(), RestoreError> {
    let retained =
        RestoreRetainedDirectory::open_root(root).map_err(|source| io_error(root, source))?;
    verify_staging_prefix_retained(retained, members).map(|_| ())
}

fn verify_staging_prefix_retained(
    root: RestoreRetainedDirectory,
    members: &[RestoreMember],
) -> Result<ValidatedStagingTree, RestoreError> {
    let expected = members
        .iter()
        .map(|member| (member.relative_destination.clone(), member))
        .collect::<BTreeMap<_, _>>();
    let mut files = BTreeMap::new();
    let mut directories = BTreeMap::new();
    collect_retained_staging_prefix(
        &root,
        Path::new(""),
        &expected,
        &mut files,
        &mut directories,
    )?;
    let mut allowed_directories = BTreeSet::new();
    for relative in expected.keys() {
        let mut parent = relative.parent();
        while let Some(value) = parent {
            if value.as_os_str().is_empty() {
                break;
            }
            allowed_directories.insert(value.to_path_buf());
            parent = value.parent();
        }
    }
    if let Some(extra) = directories
        .keys()
        .find(|directory| !allowed_directories.contains(*directory))
    {
        return Err(RestoreError::Interrupted {
            path: root.display_path.join(extra),
            reason: "restore staging contains an unjournaled extra directory".to_owned(),
        });
    }
    Ok(ValidatedStagingTree {
        root,
        files,
        directories,
    })
}

fn collect_retained_staging_prefix(
    directory: &RestoreRetainedDirectory,
    relative_directory: &Path,
    expected: &BTreeMap<PathBuf, &RestoreMember>,
    files: &mut BTreeMap<PathBuf, crate::retained_dir::RetainedFileIdentity>,
    directories: &mut BTreeMap<PathBuf, crate::retained_dir::RetainedFileIdentity>,
) -> Result<(), RestoreError> {
    for leaf in directory
        .direct_entries()
        .map_err(|source| io_error(&directory.display_path, source))?
    {
        let relative = relative_directory.join(&leaf);
        if let Ok(child) = directory.open_directory(&leaf) {
            directories.insert(relative.clone(), child.identity.clone());
            collect_retained_staging_prefix(&child, &relative, expected, files, directories)?;
            directory
                .verify_direct_directory_identity(&leaf, &child.identity)
                .map_err(|source| io_error(&child.display_path, source))?;
        } else {
            let path = directory.display_path.join(&leaf);
            let member = expected
                .get(&relative)
                .ok_or_else(|| RestoreError::Interrupted {
                    path: path.clone(),
                    reason: "restore staging contains an unjournaled extra file or special entry"
                        .to_owned(),
                })?;
            let (raw, identity) = directory
                .read_direct_file_bounded(&leaf, member.entry.byte_length)
                .map_err(|source| RestoreError::Interrupted {
                    path: path.clone(),
                    reason: format!(
                        "staging member is linked, special, missing, or changed: {source}"
                    ),
                })?;
            if raw != member.bytes || sha256(&raw) != member.entry.sha256 {
                return Err(RestoreError::Interrupted {
                    path: directory.display_path.join(&leaf),
                    reason: "restore staging contains a substituted member".to_owned(),
                });
            }
            files.insert(relative, identity);
        }
    }
    directory
        .verify_identity()
        .map_err(|source| io_error(&directory.display_path, source))
}

#[cfg(test)]
fn verify_staging_exact(
    root: &Path,
    members: &[RestoreMember],
    manifest: &BackupManifestDocument,
) -> Result<(), RestoreError> {
    let retained =
        RestoreRetainedDirectory::open_root(root).map_err(|source| io_error(root, source))?;
    verify_staging_exact_retained(retained, members, manifest).map(|_| ())
}

fn verify_staging_exact_retained(
    root: RestoreRetainedDirectory,
    members: &[RestoreMember],
    _manifest: &BackupManifestDocument,
) -> Result<ValidatedStagingTree, RestoreError> {
    let validated = verify_staging_prefix_retained(root, members)?;
    let expected = members
        .iter()
        .map(|member| member.relative_destination.clone())
        .collect::<BTreeSet<_>>();
    if validated.files.keys().cloned().collect::<BTreeSet<_>>() != expected {
        return Err(RestoreError::Interrupted {
            path: validated.root.display_path.clone(),
            reason: "restore staging contains omitted or extra files".to_owned(),
        });
    }
    Ok(validated)
}

#[cfg(test)]
fn reject_unexpected_directories(
    root: &Path,
    expected: &BTreeMap<PathBuf, &RestoreMember>,
) -> Result<(), RestoreError> {
    let mut allowed = BTreeSet::new();
    allowed.insert(PathBuf::from("locks"));
    for path in expected.keys() {
        let mut parent = path.parent();
        while let Some(value) = parent {
            if value.as_os_str().is_empty() {
                break;
            }
            allowed.insert(value.to_path_buf());
            parent = value.parent();
        }
    }
    let mut directories = Vec::new();
    collect_relative_directories(root, root, &mut directories)?;
    for directory in directories {
        if !allowed.contains(&directory) {
            return Err(RestoreError::Collision {
                path: root.join(directory),
                reason: "destination contains an extra empty or unclassified directory".to_owned(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
fn collect_relative_directories(
    root: &Path,
    directory: &Path,
    directories: &mut Vec<PathBuf>,
) -> Result<(), RestoreError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|source| io_error(directory, source))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| io_error(directory, source))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| io_error(&path, source))?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| RestoreError::Tampered {
                    reason: "destination directory escaped the sidecar root".to_owned(),
                })?
                .to_path_buf();
            directories.push(relative);
            collect_relative_directories(root, &path, directories)?;
        }
    }
    Ok(())
}

#[cfg(test)]
fn walk_sidecar_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), RestoreError> {
    let metadata = fs::symlink_metadata(directory).map_err(|source| io_error(directory, source))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(RestoreError::Tampered {
            reason: format!(
                "restore directory is linked or special: {}",
                directory.display()
            ),
        });
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|source| io_error(directory, source))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| io_error(directory, source))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| io_error(&path, source))?;
        if metadata.file_type().is_symlink() {
            return Err(RestoreError::Tampered {
                reason: format!("restore tree contains a link: {}", path.display()),
            });
        }
        if metadata.is_dir() {
            walk_sidecar_files(root, &path, files)?;
        } else if metadata.is_file() && path_hard_link_count(&path, &metadata) == 1 {
            files.push(
                path.strip_prefix(root)
                    .map_err(|_| RestoreError::Tampered {
                        reason: "restore tree escaped its root".to_owned(),
                    })?
                    .to_path_buf(),
            );
        } else {
            return Err(RestoreError::Tampered {
                reason: format!(
                    "restore tree contains a special or linked file: {}",
                    path.display()
                ),
            });
        }
    }
    Ok(())
}

fn restore_vacate_retained_publication(
    parent: &RestoreRetainedDirectory,
    destination_leaf: &Path,
    retained_root: &RestoreRetainedDirectory,
) -> io::Result<String> {
    let mut failures = Vec::new();
    for _ in 0..3 {
        match restore_isolate_unquiesced_publication(parent, destination_leaf, retained_root) {
            Ok(isolation) => {
                isolation.revalidate(parent, destination_leaf, &retained_root.identity)?;
                parent.sync_self()?;
                isolation.revalidate(parent, destination_leaf, &retained_root.identity)?;
                return Ok(isolation.description(parent, destination_leaf));
            }
            Err(error) => failures.push(error.to_string()),
        }
    }
    Err(io::Error::other(
        format!(
            "exact retained publication could not be isolated behind a Store-owned authoritative placeholder with a discoverable recovery marker: {}",
            failures.join("; ")
        ),
    ))
}

fn publish_staging_directory_create_new(
    parent: &RestoreRetainedDirectory,
    staging_leaf: &Path,
    destination_leaf: &Path,
    destination: &Path,
    destination_state: &Path,
    staging: &RestoreRetainedDirectory,
    staged_state_root_identity: &crate::retained_dir::RetainedFileIdentity,
) -> Result<RetainedCommittedSidecar, RestoreError> {
    parent
        .verify_namespace_identity()
        .map_err(|source| io_error(&parent.display_path, source))?;
    parent
        .verify_direct_directory_identity(staging_leaf, &staging.identity)
        .map_err(|source| io_error(&staging.display_path, source))?;
    staging
        .verify_identity()
        .map_err(|source| io_error(&staging.display_path, source))?;
    staging
        .verify_directory_path_identity(
            Path::new(DESTINATION_STATE_LEAF),
            staged_state_root_identity,
        )
        .map_err(|source| io_error(&staging.display_path, source))?;
    let retained_parent = parent
        .try_clone()
        .map_err(|source| io_error(&parent.display_path, source))?;
    let retained_root = staging
        .try_clone()
        .map_err(|source| io_error(&staging.display_path, source))?;
    if let Err(source) = restore_publish_retained_directory_noreplace(
        parent,
        staging_leaf,
        destination_leaf,
        staging,
    ) {
        if source.kind() == io::ErrorKind::AlreadyExists {
            parent
                .verify_direct_directory_identity(staging_leaf, &staging.identity)
                .map_err(|verify| io_error(&staging.display_path, verify))?;
            return Err(RestoreError::Collision {
                path: destination.to_path_buf(),
                reason: "destination appeared during retained atomic publication; matching bytes are not accepted"
                    .to_owned(),
            });
        }
        return Err(io_error(destination, source));
    }

    let quiescence = (|| -> Result<RetainedDestinationQuiescence, RestoreError> {
        let guard = quiesce_host_producers(destination_state, &AtomicBool::new(false)).map_err(
            |error| RestoreError::Collision {
                path: destination_state.to_path_buf(),
                reason: format!(
                    "newly published destination could not be quiesced before exact verification: {error}"
                ),
            },
        )?;
        let state_root_identity = quiescence_bound_state_root_identity(&guard, destination_state)?;
        if &state_root_identity != staged_state_root_identity {
            return Err(RestoreError::Tampered {
                reason: "newly published destination quiesced a substituted state root".to_owned(),
            });
        }
        restore_verify_published_staging_or_isolate(parent, destination_leaf, staging)
            .map_err(|source| io_error(destination, source))?;
        parent
            .verify_namespace_identity()
            .map_err(|source| io_error(&parent.display_path, source))?;
        parent
            .sync_self()
            .map_err(|source| io_error(&parent.display_path, source))?;
        restore_verify_published_staging_or_isolate(parent, destination_leaf, staging)
            .map_err(|source| io_error(destination, source))?;
        Ok(RetainedDestinationQuiescence {
            guard,
            state_root_identity,
        })
    })();
    let quiescence = match quiescence {
        Ok(quiescence) => quiescence,
        Err(cause) => {
            return Err(match restore_vacate_retained_publication(
                parent,
                destination_leaf,
                staging,
            ) {
                Ok(isolation) => RestoreError::Interrupted {
                    path: destination.to_path_buf(),
                    reason: format!(
                        "retained directory publication failed before committed verification ({cause}); authoritative publication was replaced by {isolation}"
                    ),
                },
                Err(isolation) => RestoreError::Interrupted {
                    path: destination.to_path_buf(),
                    reason: format!(
                        "retained directory publication failed before committed verification ({cause}); exact sidecar isolation failed: {isolation}"
                    ),
                },
            });
        }
    };
    Ok(RetainedCommittedSidecar {
        parent: retained_parent,
        destination_leaf: destination_leaf.to_path_buf(),
        destination_sidecar: destination.to_path_buf(),
        destination_state: destination_state.to_path_buf(),
        root: retained_root,
        quiescence,
        verified_tree: None,
    })
}

#[cfg(test)]
fn publish_directory_create_new(staging: &Path, destination: &Path) -> Result<(), RestoreError> {
    if staging.parent() != destination.parent() {
        return Err(RestoreError::InvalidPath {
            path: destination.to_path_buf(),
            reason: "test publication paths do not share one retained parent".to_owned(),
        });
    }
    let parent_path = staging.parent().ok_or_else(|| RestoreError::InvalidPath {
        path: staging.to_path_buf(),
        reason: "test publication staging has no parent".to_owned(),
    })?;
    let staging_leaf = staging
        .file_name()
        .ok_or_else(|| RestoreError::InvalidPath {
            path: staging.to_path_buf(),
            reason: "test publication staging has no leaf".to_owned(),
        })?;
    let destination_leaf = destination
        .file_name()
        .ok_or_else(|| RestoreError::InvalidPath {
            path: destination.to_path_buf(),
            reason: "test publication destination has no leaf".to_owned(),
        })?;
    let parent = RestoreRetainedDirectory::open_root(parent_path)
        .map_err(|source| io_error(parent_path, source))?;
    let staging = parent
        .open_directory(Path::new(staging_leaf))
        .map_err(|source| io_error(staging, source))?;
    restore_publish_retained_directory_noreplace(
        &parent,
        Path::new(staging_leaf),
        Path::new(destination_leaf),
        &staging,
    )
    .map_err(|source| {
        if source.kind() == io::ErrorKind::AlreadyExists {
            RestoreError::Collision {
                path: destination.to_path_buf(),
                reason: "destination exists".to_owned(),
            }
        } else {
            io_error(destination, source)
        }
    })
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
fn restore_publish_retained_directory_noreplace(
    parent: &RestoreRetainedDirectory,
    staging_leaf: &Path,
    destination_leaf: &Path,
    staging: &RestoreRetainedDirectory,
) -> io::Result<()> {
    let (quarantine_leaf, placeholder) = restore_create_publication_placeholder(parent)?;
    if let Err(source) = restore_exchange_relative(&parent.handle, staging_leaf, &quarantine_leaf) {
        parent.remove_direct_directory_if_identity(&quarantine_leaf, &placeholder.identity)?;
        return Err(source);
    }

    let quarantine_validation = (|| -> io::Result<()> {
        let quarantined_staging = parent.open_directory(&quarantine_leaf)?;
        let source_placeholder = parent.open_directory(staging_leaf)?;
        if quarantined_staging.identity != staging.identity
            || source_placeholder.identity != placeholder.identity
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore staging changed during atomic publication quarantine",
            ));
        }
        staging.verify_identity()?;
        parent.sync_self()
    })();
    if let Err(source) = quarantine_validation {
        let rollback = restore_exchange_relative(&parent.handle, &quarantine_leaf, staging_leaf)
            .and_then(|()| parent.verify_direct_directory_identity(staging_leaf, &staging.identity))
            .and_then(|()| {
                parent.verify_direct_directory_identity(&quarantine_leaf, &placeholder.identity)
            })
            .and_then(|()| {
                parent.remove_direct_directory_if_identity(&quarantine_leaf, &placeholder.identity)
            })
            .and_then(|()| parent.sync_self());
        return Err(io::Error::other(
            format!(
                "retained restore publication quarantine validation failed ({source}); rollback result: {rollback:?}"
            ),
        ));
    }

    if let Err(source) = restore_rename_directory_noreplace(
        &parent.handle,
        &quarantine_leaf,
        destination_leaf,
        &staging.handle,
    ) {
        let rollback = restore_exchange_relative(&parent.handle, &quarantine_leaf, staging_leaf)
            .and_then(|()| parent.verify_direct_directory_identity(staging_leaf, &staging.identity))
            .and_then(|()| {
                parent.verify_direct_directory_identity(&quarantine_leaf, &placeholder.identity)
            })
            .and_then(|()| {
                parent.remove_direct_directory_if_identity(&quarantine_leaf, &placeholder.identity)
            })
            .and_then(|()| parent.verify_direct_directory_identity(staging_leaf, &staging.identity))
            .and_then(|()| parent.sync_self());
        if let Err(rollback) = rollback {
            return Err(io::Error::new(
                source.kind(),
                format!(
                    "retained restore publication failed ({source}) and exchange rollback failed ({rollback})"
                ),
            ));
        }
        return Err(source);
    }

    let committed = restore_verify_published_staging_or_isolate(parent, destination_leaf, staging)
        .and_then(|()| {
            parent.remove_direct_directory_if_identity(staging_leaf, &placeholder.identity)
        })
        .and_then(|()| {
            restore_verify_published_staging_or_isolate(parent, destination_leaf, staging)
        })
        .and_then(|()| parent.sync_self())
        .and_then(|()| {
            restore_verify_published_staging_or_isolate(parent, destination_leaf, staging)
        });
    if let Err(source) = committed {
        let isolation = restore_vacate_retained_publication(parent, destination_leaf, staging);
        return Err(io::Error::other(
            format!(
                "retained restore publication finalization failed ({source}); exact destination isolation result: {isolation:?}"
            ),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn restore_publish_retained_directory_noreplace(
    parent: &RestoreRetainedDirectory,
    staging_leaf: &Path,
    destination_leaf: &Path,
    staging: &RestoreRetainedDirectory,
) -> io::Result<()> {
    restore_rename_directory_noreplace(
        &parent.handle,
        staging_leaf,
        destination_leaf,
        &staging.handle,
    )
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos"
    ))
))]
fn restore_publish_retained_directory_noreplace(
    _parent: &RestoreRetainedDirectory,
    _staging_leaf: &Path,
    _destination_leaf: &Path,
    _staging: &RestoreRetainedDirectory,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "retained quarantine/exchange restore publication is unsupported on this Unix target",
    ))
}

#[cfg(not(any(unix, windows)))]
fn restore_publish_retained_directory_noreplace(
    _parent: &RestoreRetainedDirectory,
    _staging_leaf: &Path,
    _destination_leaf: &Path,
    _staging: &RestoreRetainedDirectory,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "retained quarantine/exchange restore publication is unsupported on this platform",
    ))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
fn restore_isolate_unquiesced_publication(
    parent: &RestoreRetainedDirectory,
    destination_leaf: &Path,
    staging: &RestoreRetainedDirectory,
) -> io::Result<RestorePublicationIsolation> {
    parent.verify_namespace_identity()?;
    let (recovery_leaf, placeholder) = restore_create_publication_placeholder(parent)?;
    if let Err(exchange_source) =
        restore_exchange_relative(&parent.handle, destination_leaf, &recovery_leaf)
    {
        // Fall back to a bounded no-replace sequence that still installs this
        // exact Store-created placeholder at the authoritative name. Every
        // displaced entry remains discoverable under a recovery marker.
        for _ in 0..32 {
            let displaced = match restore_quarantine_relative_noreplace(
                &parent.handle,
                destination_leaf,
                "publication-failure-recovery",
            ) {
                Ok(displaced_leaf) => {
                    let displaced_root = parent.open_directory(&displaced_leaf)?;
                    Some((displaced_leaf, displaced_root))
                }
                Err(source) if source.kind() == io::ErrorKind::NotFound => None,
                Err(source) => return Err(source),
            };
            match restore_rename_directory_noreplace(
                &parent.handle,
                &recovery_leaf,
                destination_leaf,
                &placeholder.handle,
            ) {
                Ok(()) => {
                    let authoritative_placeholder = parent.open_directory(destination_leaf)?;
                    parent.sync_self()?;
                    parent.verify_direct_directory_identity(
                        destination_leaf,
                        &placeholder.identity,
                    )?;
                    if let Some((displaced_leaf, displaced_root)) = displaced {
                        if displaced_root.identity == staging.identity {
                            let isolation = RestorePublicationIsolation {
                                recovery_leaf: displaced_leaf,
                                recovery_root: displaced_root,
                                authoritative_placeholder,
                            };
                            isolation.revalidate(parent, destination_leaf, &staging.identity)?;
                            return Ok(isolation);
                        }
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "exact publication exchange failed ({exchange_source}); authoritative Store placeholder installed and substituted blocker retained at {}",
                                parent.display_path.join(displaced_leaf).display()
                            ),
                        ));
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "exact publication exchange failed ({exchange_source}); authoritative Store placeholder installed but retained publication was already absent"
                        ),
                    ));
                }
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
                Err(source) => return Err(source),
            }
        }
        parent.sync_self()?;
        parent.verify_direct_directory_identity(&recovery_leaf, &placeholder.identity)?;
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "exact publication exchange failed ({exchange_source}); authoritative placeholder installation was continuously repopulated and Store marker remains at {}",
                parent.display_path.join(&recovery_leaf).display()
            ),
        ));
    }

    let recovery_root = parent.open_directory(&recovery_leaf)?;
    let authoritative_placeholder = parent.open_directory(destination_leaf)?;
    let isolation = RestorePublicationIsolation {
        recovery_leaf,
        recovery_root,
        authoritative_placeholder,
    };
    staging.verify_identity()?;
    isolation.revalidate(parent, destination_leaf, &staging.identity)?;
    parent.sync_self()?;
    isolation.revalidate(parent, destination_leaf, &staging.identity)?;
    Ok(isolation)
}

#[cfg(windows)]
fn restore_isolate_unquiesced_publication(
    parent: &RestoreRetainedDirectory,
    destination_leaf: &Path,
    staging: &RestoreRetainedDirectory,
) -> io::Result<RestorePublicationIsolation> {
    parent.verify_namespace_identity()?;
    let (placeholder_leaf, placeholder) = restore_create_publication_placeholder(parent)?;
    let nonce = restore_quarantine_nonce()?;
    let mut exact_recovery = None;
    for attempt in 0..32 {
        let current = match parent.open_optional_directory(destination_leaf)? {
            Some(current) => current,
            None => {
                match restore_rename_directory_noreplace(
                    &parent.handle,
                    &placeholder_leaf,
                    destination_leaf,
                    &placeholder.handle,
                ) {
                    Ok(()) => {
                        let authoritative_placeholder = parent.open_directory(destination_leaf)?;
                        parent.sync_self()?;
                        parent.verify_direct_directory_identity(
                            destination_leaf,
                            &placeholder.identity,
                        )?;
                        if let Some((recovery_leaf, recovery_root)) = exact_recovery.take() {
                            let isolation = RestorePublicationIsolation {
                                recovery_leaf,
                                recovery_root,
                                authoritative_placeholder,
                            };
                            isolation.revalidate(parent, destination_leaf, &staging.identity)?;
                            return Ok(isolation);
                        }
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "exact unquiesced publication disappeared; authoritative Store placeholder installed",
                        ));
                    }
                    Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(source) => return Err(source),
                }
            }
        };
        let recovery_leaf = restore_quarantine_leaf("unquiesced-publication", nonce, attempt);
        match restore_rename_directory_noreplace(
            &parent.handle,
            destination_leaf,
            &recovery_leaf,
            &current.handle,
        ) {
            Ok(()) => {
                let recovery_root = parent.open_directory(&recovery_leaf)?;
                if recovery_root.identity != current.identity {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unquiesced publication changed during handle-bound isolation",
                    ));
                }
                if current.identity == staging.identity {
                    exact_recovery = Some((recovery_leaf, recovery_root));
                }
                parent.sync_self()?;
            }
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => return Err(source),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        format!(
            "bounded unquiesced-publication isolation was continuously repopulated; Store placeholder retained at {}",
            parent.display_path.join(placeholder_leaf).display()
        ),
    ))
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos",
    windows
)))]
fn restore_isolate_unquiesced_publication(
    _parent: &RestoreRetainedDirectory,
    _destination_leaf: &Path,
    _staging: &RestoreRetainedDirectory,
) -> io::Result<RestorePublicationIsolation> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "exact unquiesced-publication isolation is unsupported on this platform",
    ))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
fn restore_verify_published_staging_or_isolate(
    parent: &RestoreRetainedDirectory,
    destination_leaf: &Path,
    staging: &RestoreRetainedDirectory,
) -> io::Result<()> {
    let observed = match parent.open_directory(destination_leaf) {
        Ok(published) if published.identity == staging.identity => return Ok(()),
        Ok(published) => Some(published.identity),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained restore publication disappeared before verification",
            ));
        }
        Err(_) => None,
    };
    let isolation = match restore_quarantine_relative_noreplace(
        &parent.handle,
        destination_leaf,
        "publication-mismatch",
    ) {
        Ok(isolation) => isolation,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained restore publication disappeared during isolation",
            ));
        }
        Err(source) => return Err(source),
    };
    if let Some(observed) = observed {
        let isolated = parent.open_directory(&isolation)?;
        if isolated.identity != observed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "restore publication mismatch changed while it was isolated",
            ));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "substituted restore staging was isolated from the authoritative destination",
    ))
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
)))]
fn restore_verify_published_staging_or_isolate(
    parent: &RestoreRetainedDirectory,
    destination_leaf: &Path,
    staging: &RestoreRetainedDirectory,
) -> io::Result<()> {
    parent.verify_direct_directory_identity(destination_leaf, &staging.identity)
}

#[cfg(any(unix, windows))]
fn restore_quarantine_nonce() -> io::Result<u128> {
    let mut nonce = [0_u8; 16];
    getrandom::fill(&mut nonce).map_err(|error| {
        io::Error::other(format!(
            "restore quarantine nonce generation failed: {error}"
        ))
    })?;
    Ok(u128::from_le_bytes(nonce))
}

#[cfg(any(unix, windows))]
fn restore_quarantine_leaf(purpose: &str, nonce: u128, attempt: u32) -> PathBuf {
    PathBuf::from(format!(
        ".forge-restore-{purpose}-{}-{nonce}-{attempt}.quarantine",
        std::process::id()
    ))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos",
    windows
))]
fn restore_create_publication_placeholder(
    parent: &RestoreRetainedDirectory,
) -> io::Result<(PathBuf, RestoreRetainedDirectory)> {
    let nonce = restore_quarantine_nonce()?;
    for attempt in 0..32 {
        let leaf = restore_quarantine_leaf("publication-swap", nonce, attempt);
        match parent.create_directory_new(&leaf) {
            Ok(placeholder) => return Ok((leaf, placeholder)),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => return Err(source),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "restore publication quarantine-name retry exhausted",
    ))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
fn restore_isolate_protected_file_name(
    parent: &RestoreRetainedDirectory,
    protected_leaf: &Path,
    purpose: &str,
) -> io::Result<Vec<PathBuf>> {
    let mut isolated = Vec::new();
    for _ in 0..32 {
        match restore_quarantine_relative_noreplace(&parent.handle, protected_leaf, purpose) {
            Ok(quarantine) => {
                isolated.push(quarantine);
                parent.sync_self()?;
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                parent.sync_self()?;
                return Ok(isolated);
            }
            Err(source) => return Err(source),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "bounded protected restore file isolation was continuously repopulated",
    ))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
fn restore_verify_published_private_file_or_isolate(
    parent: &RestoreRetainedDirectory,
    protected_leaf: &Path,
    retained_temporary: &File,
    temporary_identity: &crate::retained_dir::RetainedFileIdentity,
    bytes: &[u8],
) -> io::Result<()> {
    let maximum = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let retained = restore_read_retained_authority_file_bounded(retained_temporary, maximum);
    let published = parent.read_direct_authority_file_bounded(protected_leaf, maximum);
    let exact = matches!(
        (&retained, &published),
        (Ok((retained_bytes, retained_identity)), Ok((published_bytes, published_identity)))
            if retained_identity == temporary_identity
                && published_identity == temporary_identity
                && retained_bytes.as_slice() == bytes
                && published_bytes.as_slice() == bytes
    );
    if exact {
        parent.verify_identity()?;
        return Ok(());
    }

    let isolation = restore_isolate_protected_file_name(
        parent,
        protected_leaf,
        "protected-file-publication-mismatch",
    );
    match isolation {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "substituted protected restore file was isolated from its authoritative leaf",
        )),
        Err(isolation) => Err(io::Error::other(
            format!(
                "protected restore file failed exact post-link verification and fail-closed isolation failed: {isolation}"
            ),
        )),
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
fn restore_publish_retained_file_noreplace(
    authority_root: &RestoreRetainedDirectory,
    parent_relative: &Path,
    parent: &RestoreRetainedDirectory,
    temporary_leaf: &Path,
    protected_leaf: &Path,
    retained_temporary: &File,
    temporary_identity: &crate::retained_dir::RetainedFileIdentity,
    bytes: &[u8],
) -> io::Result<()> {
    let maximum = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let retained_validation = restore_read_retained_file_bounded(retained_temporary, maximum);
    let named_temporary = restore_open_relative(
        &parent.handle,
        temporary_leaf,
        RestoreRelativeOpen::FileRead,
    );
    if !matches!(
        (&retained_validation, &named_temporary),
        (Ok((retained_bytes, retained_identity)), Ok(named_temporary))
            if retained_identity == temporary_identity
                && retained_bytes.as_slice() == bytes
                && validate_restore_file_handle(named_temporary).ok().as_ref()
                    == Some(temporary_identity)
    ) {
        let isolation = restore_isolate_protected_file_name(
            parent,
            protected_leaf,
            "protected-file-retained-temporary-failure",
        );
        return match isolation {
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained protected restore temporary changed before publication",
            )),
            Err(isolation) => Err(io::Error::other(
                format!(
                    "retained protected restore temporary changed before publication and blocker isolation failed: {isolation}"
                ),
            )),
        };
    }
    parent.verify_identity()?;
    let protected_leaf = restore_direct_component(protected_leaf)?;
    if let Err(source) = crate::retained_dir::RetainedDirectory::link_retained_file_noreplace(
        retained_temporary,
        &parent.handle,
        protected_leaf,
    ) {
        let isolation = restore_isolate_protected_file_name(
            parent,
            Path::new(protected_leaf),
            "protected-file-publication-blocker",
        );
        return match isolation {
            Ok(_) => Err(source),
            Err(isolation) => Err(io::Error::other(
                format!(
                    "exact-handle protected restore publication failed ({source}) and blocker isolation failed: {isolation}"
                ),
            )),
        };
    }

    // The atomic no-replace hard link selects the immutable record. Successful
    // publication linearizes at the closing exact-handle/root/durability sweep
    // below; no caller-side decisive I/O follows. The Store-created temporary
    // name remains discoverable as exact cleanup debt because these Unix targets
    // expose no entry deletion primitive bound to the retained file.
    let committed = (|| -> io::Result<()> {
        restore_verify_published_private_file_or_isolate(
            parent,
            Path::new(protected_leaf),
            retained_temporary,
            temporary_identity,
            bytes,
        )?;
        authority_root.verify_directory_path_identity(parent_relative, &parent.identity)?;
        parent.sync_self()?;
        authority_root.verify_namespace_identity()?;
        restore_verify_published_private_file_or_isolate(
            parent,
            Path::new(protected_leaf),
            retained_temporary,
            temporary_identity,
            bytes,
        )?;
        authority_root.verify_directory_path_identity(parent_relative, &parent.identity)?;
        authority_root.verify_namespace_identity()
    })();
    if let Err(source) = committed {
        let isolation = restore_isolate_protected_file_name(
            parent,
            Path::new(protected_leaf),
            "protected-file-post-link-failure",
        );
        return match isolation {
            Ok(_) => Err(source),
            Err(isolation) => Err(io::Error::other(
                format!(
                    "protected restore exact-handle publication finalization failed ({source}) and fail-closed isolation failed: {isolation}"
                ),
            )),
        };
    }
    Ok(())
}

#[cfg(windows)]
fn restore_publish_retained_file_noreplace(
    authority_root: &RestoreRetainedDirectory,
    parent_relative: &Path,
    parent: &RestoreRetainedDirectory,
    temporary_leaf: &Path,
    protected_leaf: &Path,
    retained_temporary: &File,
    temporary_identity: &crate::retained_dir::RetainedFileIdentity,
    bytes: &[u8],
) -> io::Result<()> {
    restore_rename_file_noreplace(
        &parent.handle,
        temporary_leaf,
        protected_leaf,
        retained_temporary,
    )?;
    let maximum = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let (retained_bytes, retained_identity) =
        restore_read_retained_file_bounded(retained_temporary, maximum)?;
    let (published_bytes, published_identity) =
        parent.read_direct_file_bounded(protected_leaf, maximum)?;
    if retained_identity != *temporary_identity
        || published_identity != *temporary_identity
        || retained_bytes != bytes
        || published_bytes != bytes
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "protected restore file changed during retained Windows publication",
        ));
    }
    authority_root.verify_directory_path_identity(parent_relative, &parent.identity)?;
    parent.sync_self()?;
    authority_root.verify_namespace_identity()?;
    let (final_bytes, final_identity) = parent.read_direct_file_bounded(protected_leaf, maximum)?;
    if final_identity != *temporary_identity || final_bytes != bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "protected restore file changed during final Windows publication sweep",
        ));
    }
    authority_root.verify_directory_path_identity(parent_relative, &parent.identity)?;
    authority_root.verify_namespace_identity()
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos"
    ))
))]
fn restore_publish_retained_file_noreplace(
    _authority_root: &RestoreRetainedDirectory,
    _parent_relative: &Path,
    _parent: &RestoreRetainedDirectory,
    _temporary_leaf: &Path,
    _protected_leaf: &Path,
    _retained_temporary: &File,
    _temporary_identity: &crate::retained_dir::RetainedFileIdentity,
    _bytes: &[u8],
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "retained placeholder/exchange restore file publication is unsupported on this Unix target",
    ))
}

#[cfg(not(any(unix, windows)))]
fn restore_publish_retained_file_noreplace(
    _authority_root: &RestoreRetainedDirectory,
    _parent_relative: &Path,
    _parent: &RestoreRetainedDirectory,
    _temporary_leaf: &Path,
    _protected_leaf: &Path,
    _retained_temporary: &File,
    _temporary_identity: &crate::retained_dir::RetainedFileIdentity,
    _bytes: &[u8],
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "retained placeholder/exchange restore file publication is unsupported on this platform",
    ))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
fn restore_exchange_relative(parent: &File, left: &Path, right: &Path) -> io::Result<()> {
    restore_direct_component(left)?;
    restore_direct_component(right)?;
    rustix::fs::renameat_with(
        parent,
        left,
        parent,
        right,
        rustix::fs::RenameFlags::EXCHANGE,
    )
    .map_err(|error| io::Error::from_raw_os_error(error.raw_os_error()))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos",
    target_os = "redox"
))]
fn restore_rename_relative_noreplace(
    parent: &File,
    source: &Path,
    destination: &Path,
) -> io::Result<()> {
    restore_direct_component(source)?;
    restore_direct_component(destination)?;
    rustix::fs::renameat_with(
        parent,
        source,
        parent,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|error| io::Error::from_raw_os_error(error.raw_os_error()))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos",
    target_os = "redox"
))]
fn restore_quarantine_relative_noreplace(
    parent: &File,
    source: &Path,
    purpose: &str,
) -> io::Result<PathBuf> {
    let nonce = restore_quarantine_nonce()?;
    for attempt in 0..32 {
        let quarantine = restore_quarantine_leaf(purpose, nonce, attempt);
        match restore_rename_relative_noreplace(parent, source, &quarantine) {
            Ok(()) => return Ok(quarantine),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "restore quarantine-name retry exhausted",
    ))
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos",
        target_os = "redox"
    ))
))]
fn restore_quarantine_relative_noreplace(
    _parent: &File,
    _source: &Path,
    _purpose: &str,
) -> io::Result<PathBuf> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace restore quarantine is unsupported on this Unix target",
    ))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos",
    target_os = "redox"
))]
fn restore_rename_directory_noreplace(
    parent: &File,
    source: &Path,
    destination: &Path,
    retained_source: &File,
) -> io::Result<()> {
    let current = restore_open_relative(parent, source, RestoreRelativeOpen::Directory)?;
    validate_restore_directory_handle(&current)?;
    validate_restore_directory_handle(retained_source)?;
    if crate::retained_dir::RetainedDirectory::identity_of(&current)?
        != crate::retained_dir::RetainedDirectory::identity_of(retained_source)?
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained restore directory changed before publication",
        ));
    }
    restore_rename_relative_noreplace(parent, source, destination)
}

#[cfg(windows)]
fn restore_rename_directory_noreplace(
    parent: &File,
    source: &Path,
    destination: &Path,
    retained_source: &File,
) -> io::Result<()> {
    restore_rename_entry_noreplace(parent, source, destination, retained_source, true)
}

#[cfg(windows)]
fn restore_rename_file_noreplace(
    parent: &File,
    source: &Path,
    destination: &Path,
    retained_source: &File,
) -> io::Result<()> {
    restore_rename_entry_noreplace(parent, source, destination, retained_source, false)
}

#[cfg(windows)]
fn restore_rename_entry_noreplace(
    parent: &File,
    source: &Path,
    destination: &Path,
    retained_source: &File,
    directory: bool,
) -> io::Result<()> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt as _;
    use std::os::windows::io::AsRawHandle as _;
    type Handle = *mut c_void;
    #[repr(C)]
    struct FileRenameInfo {
        replace_if_exists: i32,
        root_directory: Handle,
        file_name_length: u32,
        file_name: [u16; 1],
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetFileInformationByHandle(
            handle: Handle,
            information_class: u32,
            information: *const c_void,
            buffer_size: u32,
        ) -> i32;
    }
    restore_direct_component(source)?;
    restore_direct_component(destination)?;
    let namespace_handle = restore_open_relative(
        parent,
        source,
        if directory {
            RestoreRelativeOpen::DirectoryDelete
        } else {
            RestoreRelativeOpen::FileReadSharedDelete
        },
    )?;
    let namespace_identity = if directory {
        validate_restore_directory_handle(&namespace_handle)?;
        crate::retained_dir::RetainedDirectory::identity_of(&namespace_handle)?
    } else {
        validate_restore_file_handle(&namespace_handle)?
    };
    let retained_identity = if directory {
        validate_restore_directory_handle(retained_source)?;
        crate::retained_dir::RetainedDirectory::identity_of(retained_source)?
    } else {
        validate_restore_file_handle(retained_source)?
    };
    if namespace_identity != retained_identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained restore entry changed before publication",
        ));
    }
    let rename_handle = if directory {
        &namespace_handle
    } else {
        retained_source
    };
    let wide = destination.as_os_str().encode_wide().collect::<Vec<_>>();
    let extra = wide.len().saturating_sub(1).checked_mul(2).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "restore destination leaf too long",
        )
    })?;
    let size = std::mem::size_of::<FileRenameInfo>()
        .checked_add(extra)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "rename buffer overflow"))?;
    let mut storage = vec![0_u8; size];
    let info = storage.as_mut_ptr().cast::<FileRenameInfo>();
    // SAFETY: storage contains the fixed header and computed trailing UTF-16 name.
    unsafe {
        (*info).replace_if_exists = 0;
        (*info).root_directory = parent.as_raw_handle().cast();
        (*info).file_name_length = u32::try_from(wide.len() * 2)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "rename leaf too long"))?;
        std::ptr::copy_nonoverlapping(wide.as_ptr(), (*info).file_name.as_mut_ptr(), wide.len());
    }
    // SAFETY: rename_handle names the exact retained entry and storage is initialized.
    let result = unsafe {
        SetFileInformationByHandle(
            rename_handle.as_raw_handle().cast(),
            3,
            info.cast(),
            u32::try_from(size).expect("rename buffer fits u32"),
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos",
        target_os = "redox"
    ))
))]
fn restore_rename_directory_noreplace(
    _parent: &File,
    _source: &Path,
    _destination: &Path,
    _retained_source: &File,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "retained atomic restore publication is unsupported on this Unix target",
    ))
}

#[cfg(not(any(unix, windows)))]
fn restore_rename_directory_noreplace(
    _parent: &File,
    _source: &Path,
    _destination: &Path,
    _retained_source: &File,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "retained atomic restore publication is unsupported on this platform",
    ))
}

fn restore_path_digest(path: &Path) -> Result<String, RestoreError> {
    #[cfg(unix)]
    let encoded = {
        use std::os::unix::ffi::OsStrExt as _;
        path.as_os_str().as_bytes().to_vec()
    };
    #[cfg(windows)]
    let encoded = {
        use std::os::windows::ffi::OsStrExt as _;
        path.as_os_str()
            .encode_wide()
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<_>>()
    };
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        return Err(RestoreError::Tampered {
            reason: "canonical restore path encoding is unsupported on this platform".to_owned(),
        });
    }
    #[cfg(any(unix, windows))]
    {
        let mut hasher = Sha256::new();
        hasher.update(RESTORE_PATH_DIGEST_DOMAIN);
        hasher.update((encoded.len() as u64).to_be_bytes());
        hasher.update(encoded);
        Ok(format!("sha256:{:x}", hasher.finalize()))
    }
}

fn restore_completion_inventory(
    tree: &RetainedVerifiedSidecarTree,
) -> Result<(Vec<RestoreCompletionInventoryEntry>, String), RestoreError> {
    let mut inventory = Vec::with_capacity(tree.directories.len() + tree.files.len());
    for relative in tree.directories.keys() {
        inventory.push(RestoreCompletionInventoryEntry::Directory {
            relative_path: slash_path(relative)?,
        });
    }
    for (relative, file) in &tree.files {
        inventory.push(RestoreCompletionInventoryEntry::File {
            relative_path: slash_path(relative)?,
            byte_length: u64::try_from(file.bytes.len()).unwrap_or(u64::MAX),
            sha256: file.digest.clone(),
        });
    }
    inventory.sort_by(|left, right| {
        left.relative_path()
            .cmp(right.relative_path())
            .then_with(|| left.kind_order().cmp(&right.kind_order()))
    });
    let canonical =
        serde_json_canonicalizer::to_vec(&inventory).map_err(|error| RestoreError::Tampered {
            reason: format!("restore completion inventory canonicalization failed: {error}"),
        })?;
    let mut hasher = Sha256::new();
    hasher.update(RESTORE_COMPLETION_INVENTORY_DIGEST_DOMAIN);
    hasher.update((canonical.len() as u64).to_be_bytes());
    hasher.update(canonical);
    let digest = format!("sha256:{:x}", hasher.finalize());
    Ok((inventory, digest))
}

fn restore_root_path_binding(
    root: &RestoreRetainedDirectory,
) -> Result<RestoreRootPathBinding, RestoreError> {
    Ok(RestoreRootPathBinding {
        configured_path: root.display_path.display().to_string(),
        configured_path_sha256: restore_path_digest(&root.display_path)?,
    })
}

fn restore_protected_document_binding<T>(
    document: &RetainedRestoreDocument<T>,
) -> Result<RestoreProtectedDocumentBinding, RestoreError> {
    Ok(RestoreProtectedDocumentBinding {
        relative_path: slash_path(&document.retained.relative)?,
        parent_relative: slash_path(&document.retained.parent_relative)?,
        content_sha256: document.retained.digest.clone(),
    })
}

fn build_restore_completion_authority(
    preflight: &RestorePreflight,
    tree: &RetainedVerifiedSidecarTree,
    quiescence: &RetainedDestinationQuiescence,
    journal: &RetainedRestoreDocument<RestoreJournalDocument>,
    receipt: &RetainedRestoreDocument<RestoreReceiptDocument>,
) -> Result<RestoreCompletionAuthorityDocument, RestoreError> {
    if journal.retained.authority_root.identity != preflight.authority_root.identity
        || receipt.retained.authority_root.identity != preflight.authority_root.identity
        || journal.retained.authority_root.display_path != preflight.authority_root.display_path
        || receipt.retained.authority_root.display_path != preflight.authority_root.display_path
        || tree.root_path != preflight.plan.destination_sidecar
        || tree.state_root_identity != quiescence.state_root_identity
    {
        return Err(RestoreError::Tampered {
            reason: "restore completion inputs do not share their retained exact roots".to_owned(),
        });
    }
    let (inventory, inventory_digest) = restore_completion_inventory(tree)?;
    let backup = &preflight.plan.verified.receipt().backup_receipt;
    Ok(RestoreCompletionAuthorityDocument {
        schema_version: RESTORE_COMPLETION_AUTHORITY_SCHEMA_VERSION.to_owned(),
        operation_nonce: preflight.operation_nonce.clone(),
        project_id: backup.project_id.0.clone(),
        workflow_release: backup.workflow_release.clone(),
        effective_bundle: backup.effective_epoch.effective_bundle.clone(),
        source: RestoreCompletionSourceIdentity {
            archive_sha256: preflight.plan.verified.archive_sha256().to_owned(),
            backup_receipt_digest: backup.receipt_digest.clone(),
            manifest_set_digest: backup.manifest_set_digest.clone(),
            backup_created_at_unix: backup.created_at_unix,
        },
        protected_authority_root: restore_root_path_binding(&preflight.authority_root)?,
        sidecar: RestoreCompletionSidecarBinding {
            destination_sidecar: tree.root_path.display().to_string(),
            retained_parent: restore_root_path_binding(&tree.parent)?,
            root_leaf: slash_path(&tree.root_leaf)?,
            root_path_sha256: restore_path_digest(&tree.root_path)?,
            state_root_relative: DESTINATION_STATE_LEAF.to_owned(),
            state_root_path_sha256: restore_path_digest(
                &tree.root_path.join(DESTINATION_STATE_LEAF),
            )?,
            inventory_digest,
            inventory,
        },
        journal: restore_protected_document_binding(journal)?,
        receipt: restore_protected_document_binding(receipt)?,
        project_link: RestoreProjectLinkBinding {
            project_root: restore_root_path_binding(&preflight.plan.project_root_retained)?,
            leaf: slash_path(&preflight.plan.project_link_leaf)?,
            content_sha256: sha256(&preflight.plan.project_link_bytes),
        },
        replay_anchor: RestoreReplayAnchorBinding {
            configured_root: restore_root_path_binding(&preflight.replay_anchor.configured_root)?,
            parent_relative: slash_path(&preflight.replay_anchor.parent_relative)?,
            lock_leaf: slash_path(&preflight.replay_anchor.lock_leaf)?,
            anchor_leaf: slash_path(&preflight.replay_anchor.anchor_leaf)?,
            anchor_digest: preflight.replay_anchor.anchor_digest.clone(),
        },
        transaction: RestoreTransactionAuthorityBinding {
            lock_relative: slash_path(&preflight.transaction_lock_relative)?,
        },
        quiescence: RestoreQuiescenceBinding {
            destination_state: preflight.plan.destination_state.display().to_string(),
            destination_state_path_sha256: restore_path_digest(&preflight.plan.destination_state)?,
        },
    })
}

fn validate_same_restore_completion_authority(
    expected: &RestoreCompletionAuthorityDocument,
    actual: &RestoreCompletionAuthorityDocument,
) -> Result<(), RestoreError> {
    validate_restore_operation_nonce(&actual.operation_nonce)?;
    if actual.schema_version != RESTORE_COMPLETION_AUTHORITY_SCHEMA_VERSION || actual != expected {
        return Err(RestoreError::Tampered {
            reason: "protected restore completion authority is stale, substituted, or tampered"
                .to_owned(),
        });
    }
    Ok(())
}

fn canonical_restore_completion_authority_bytes(
    document: &RestoreCompletionAuthorityDocument,
) -> Result<Vec<u8>, RestoreError> {
    serde_json_canonicalizer::to_vec(document).map_err(|error| RestoreError::Tampered {
        reason: format!("restore completion authority canonicalization failed: {error}"),
    })
}

fn completion_authority_relative_path(
    directory_relative: &Path,
    bytes: &[u8],
) -> Result<PathBuf, RestoreError> {
    let digest = sha256(bytes);
    let token = digest_token(&digest)?;
    Ok(directory_relative.join(format!("{token}.json")))
}

fn canonical_restore_completion_selector_bytes(
    document: &RestoreCompletionSelectorDocument,
) -> Result<Vec<u8>, RestoreError> {
    serde_json_canonicalizer::to_vec(document).map_err(|error| RestoreError::Tampered {
        reason: format!("restore completion selector canonicalization failed: {error}"),
    })
}

fn validate_restore_completion_record_namespace(
    completion: &RetainedRestoreDocument<RestoreCompletionAuthorityDocument>,
) -> Result<(), RestoreError> {
    let selected_leaf =
        completion
            .retained
            .relative
            .file_name()
            .ok_or_else(|| RestoreError::Tampered {
                reason: "retained restore completion content address lost its leaf".to_owned(),
            })?;
    let public_leaves = completion
        .retained
        .parent
        .direct_entries()
        .map_err(|source| io_error(&completion.retained.parent.display_path, source))?
        .into_iter()
        .filter(|leaf| !leaf.to_string_lossy().starts_with('.'))
        .collect::<Vec<_>>();
    if public_leaves.len() != 1 || public_leaves[0].as_os_str() != selected_leaf {
        return Err(RestoreError::Interrupted {
            path: completion.retained.parent.display_path.clone(),
            reason:
                "restore completion content-address namespace contains an unselected generation"
                    .to_owned(),
        });
    }
    completion.revalidate()
}

fn restore_directory_has_entries(
    authority_root: &RestoreRetainedDirectory,
    relative: &Path,
    path: &Path,
) -> Result<bool, RestoreError> {
    verify_restore_authority_relative_path(authority_root, relative, path)?;
    let directory = match authority_root.open_directory_path(relative) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(source) => return Err(io_error(path, source)),
    };
    authority_root
        .verify_directory_path_identity(relative, &directory.identity)
        .map_err(|source| io_error(path, source))?;
    let has_entries = !directory
        .direct_entries()
        .map_err(|source| io_error(path, source))?
        .is_empty();
    authority_root
        .verify_directory_path_identity(relative, &directory.identity)
        .map_err(|source| io_error(path, source))?;
    Ok(has_entries)
}

fn retained_store_authority_root(
    authority_root: &RestoreRetainedDirectory,
) -> Result<crate::retained_dir::RetainedDirectory, RestoreError> {
    authority_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&authority_root.display_path, source))?;
    let retained = crate::retained_dir::RetainedDirectory::from_handle(
        authority_root
            .handle
            .try_clone()
            .map_err(|source| io_error(&authority_root.display_path, source))?,
        authority_root.display_path.clone(),
    );
    if retained
        .identity()
        .map_err(|source| io_error(&authority_root.display_path, source))?
        != authority_root.identity
    {
        return Err(RestoreError::Tampered {
            reason: "Store completion anchor retained a different protected authority root"
                .to_owned(),
        });
    }
    authority_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&authority_root.display_path, source))?;
    Ok(retained)
}

fn completion_selector_document(
    preflight: &RestorePreflight,
    completion: &RestoreCompletionAuthorityDocument,
    completion_relative: &Path,
    completion_digest: &str,
    completion_byte_length: u64,
    completion_anchor: &crate::retained_dir::RetainedFileAnchorBinding,
) -> Result<RestoreCompletionSelectorDocument, RestoreError> {
    let completion_parent_relative =
        completion_relative
            .parent()
            .ok_or_else(|| RestoreError::InvalidPath {
                path: completion_relative.to_path_buf(),
                reason: "restore completion content address has no parent".to_owned(),
            })?;
    let selector_parent_relative =
        preflight
            .completion_selector_relative
            .parent()
            .ok_or_else(|| RestoreError::InvalidPath {
                path: preflight.completion_selector_path.clone(),
                reason: "restore completion selector has no parent".to_owned(),
            })?;
    Ok(RestoreCompletionSelectorDocument {
        schema_version: RESTORE_COMPLETION_SELECTOR_SCHEMA_VERSION.to_owned(),
        operation_nonce: completion.operation_nonce.clone(),
        project_id: completion.project_id.clone(),
        completion: RestoreCompletionRecordSelection {
            relative_path: slash_path(completion_relative)?,
            content_sha256: completion_digest.to_owned(),
            byte_length: completion_byte_length,
            leaf_anchor: completion_anchor.clone(),
        },
        parent_root_anchor: RestoreCompletionParentRootAnchor {
            protected_authority_root: completion.protected_authority_root.clone(),
            completion_parent_relative: slash_path(completion_parent_relative)?,
            completion_parent_path_sha256: restore_path_digest(
                &preflight
                    .authority_root
                    .display_path
                    .join(completion_parent_relative),
            )?,
            selector_relative: slash_path(&preflight.completion_selector_relative)?,
            selector_parent_relative: slash_path(selector_parent_relative)?,
        },
        project: completion.project_link.clone(),
        replay: completion.replay_anchor.clone(),
        transaction: RestoreCompletionTransactionSelection {
            transaction_lock_relative: completion.transaction.lock_relative.clone(),
            journal: completion.journal.clone(),
            receipt: completion.receipt.clone(),
        },
    })
}

fn validate_restore_completion_selector(
    selector: &RestoreCompletionSelectorDocument,
    completion: &RestoreCompletionAuthorityDocument,
    authority_root: &RestoreRetainedDirectory,
    completion_directory_relative: &Path,
    completion_selector_relative: &Path,
) -> Result<PathBuf, RestoreError> {
    validate_restore_operation_nonce(&selector.operation_nonce)?;
    let completion_relative = normalized_relative_path(&selector.completion.relative_path)?;
    let expected_parent = completion_relative
        .parent()
        .ok_or_else(|| RestoreError::Tampered {
            reason: "restore completion selector content address has no parent".to_owned(),
        })?;
    let selector_parent =
        completion_selector_relative
            .parent()
            .ok_or_else(|| RestoreError::Tampered {
                reason: "restore completion selector authority has no parent".to_owned(),
            })?;
    let completion_bytes = canonical_restore_completion_authority_bytes(completion)?;
    if selector.schema_version != RESTORE_COMPLETION_SELECTOR_SCHEMA_VERSION
        || selector.operation_nonce != completion.operation_nonce
        || selector.project_id != completion.project_id
        || selector.completion.content_sha256 != sha256(&completion_bytes)
        || selector.completion.byte_length
            != u64::try_from(completion_bytes.len()).unwrap_or(u64::MAX)
        || completion_relative
            != completion_authority_relative_path(completion_directory_relative, &completion_bytes)?
        || selector.parent_root_anchor.protected_authority_root
            != completion.protected_authority_root
        || selector.parent_root_anchor.completion_parent_relative != slash_path(expected_parent)?
        || selector.parent_root_anchor.completion_parent_path_sha256
            != restore_path_digest(&authority_root.display_path.join(expected_parent))?
        || selector.parent_root_anchor.selector_relative
            != slash_path(completion_selector_relative)?
        || selector.parent_root_anchor.selector_parent_relative != slash_path(selector_parent)?
        || selector.project != completion.project_link
        || selector.replay != completion.replay_anchor
        || selector.transaction.transaction_lock_relative != completion.transaction.lock_relative
        || selector.transaction.journal != completion.journal
        || selector.transaction.receipt != completion.receipt
    {
        return Err(RestoreError::Tampered {
            reason: "restore completion selector is stale, rolled back, or bound to a different completion transaction"
                .to_owned(),
        });
    }
    let completion_scope = completion_directory_relative
        .strip_prefix(Path::new("restore-completions"))
        .map_err(|_| RestoreError::Tampered {
            reason: "restore completion content-address directory escaped its protected namespace"
                .to_owned(),
        })?;
    let expected_anchor_directory =
        PathBuf::from("restore-completion-anchors").join(completion_scope);
    let anchor_path = Path::new(&selector.completion.leaf_anchor.anchor_relative_path);
    if anchor_path.parent() != Some(expected_anchor_directory.as_path())
        || selector.completion.leaf_anchor.content_digest != selector.completion.content_sha256
        || selector.completion.leaf_anchor.byte_length != selector.completion.byte_length
    {
        return Err(RestoreError::Tampered {
            reason: "restore completion selector leaf anchor escaped its protected operation namespace or changed its content binding"
                .to_owned(),
        });
    }
    Ok(completion_relative)
}

fn retained_anchored_completion_file(
    authority_root: &RestoreRetainedDirectory,
    relative: &Path,
    handle: File,
    identity: crate::retained_dir::RetainedFileIdentity,
    maximum: u64,
) -> Result<RetainedProtectedRestoreFile, RestoreError> {
    let path = authority_root.display_path.join(relative);
    verify_restore_authority_relative_path(authority_root, relative, &path)?;
    let parent_relative = relative.parent().ok_or_else(|| RestoreError::InvalidPath {
        path: path.clone(),
        reason: "anchored restore completion has no parent".to_owned(),
    })?;
    let leaf = relative
        .file_name()
        .ok_or_else(|| RestoreError::InvalidPath {
            path: path.clone(),
            reason: "anchored restore completion has no leaf".to_owned(),
        })?;
    let parent = authority_root
        .open_directory_path(parent_relative)
        .map_err(|source| {
            io_error(
                path.parent().unwrap_or(&authority_root.display_path),
                source,
            )
        })?;
    authority_root
        .verify_directory_path_identity(parent_relative, &parent.identity)
        .map_err(|source| io_error(&parent.display_path, source))?;
    let (bytes, retained_identity) = restore_read_retained_authority_file_bounded(&handle, maximum)
        .map_err(|source| io_error(&path, source))?;
    if retained_identity != identity {
        return Err(RestoreError::Tampered {
            reason: "anchored restore completion handle changed identity while retained".to_owned(),
        });
    }
    let retained = retained_published_protected_file(
        authority_root
            .try_clone()
            .map_err(|source| io_error(&authority_root.display_path, source))?,
        parent,
        parent_relative,
        relative,
        &path,
        leaf,
        handle,
        identity,
        &bytes,
    );
    retained.revalidate()?;
    Ok(retained)
}

fn load_restore_completion_authority_retained(
    authority_root: &RestoreRetainedDirectory,
    directory_relative: &Path,
    directory_path: &Path,
    anchor_directory_relative: &Path,
    anchor_directory_path: &Path,
    selector_relative: &Path,
    selector_path: &Path,
) -> Result<Option<RetainedRestoreCompletion>, RestoreError> {
    let Some(selector_retained) = load_protected_restore_file_retained(
        authority_root,
        selector_relative,
        selector_path,
        MAX_RESTORE_RECEIPT_BYTES,
    )?
    else {
        if restore_directory_has_entries(authority_root, directory_relative, directory_path)?
            || restore_directory_has_entries(
                authority_root,
                anchor_directory_relative,
                anchor_directory_path,
            )?
        {
            return Err(RestoreError::Interrupted {
                path: selector_path.to_path_buf(),
                reason: "restore completion record or lifetime anchor exists without its authoritative selector; hidden selectors and unselected residue fail closed"
                    .to_owned(),
            });
        }
        return Ok(None);
    };
    let selector: RestoreCompletionSelectorDocument =
        serde_json::from_slice(&selector_retained.bytes).map_err(|error| {
            RestoreError::Tampered {
                reason: format!("protected restore completion selector parse failed: {error}"),
            }
        })?;
    if canonical_restore_completion_selector_bytes(&selector)? != selector_retained.bytes
        || selector.schema_version != RESTORE_COMPLETION_SELECTOR_SCHEMA_VERSION
    {
        return Err(RestoreError::Tampered {
            reason: "restore completion selector is noncanonical or has an unsupported schema"
                .to_owned(),
        });
    }
    let selector = RetainedRestoreDocument {
        retained: selector_retained,
        document: selector,
    };
    selector.revalidate()?;

    let store_root = retained_store_authority_root(authority_root)?;
    let completion_anchor = store_root
        .open_file_lifetime_anchor(&selector.document.completion.leaf_anchor)
        .map_err(|source| io_error(anchor_directory_path, source))?;
    completion_anchor
        .revalidate()
        .map_err(|source| io_error(anchor_directory_path, source))?;
    let selected_relative = normalized_relative_path(&selector.document.completion.relative_path)?;
    let (completion_handle, completion_identity) = completion_anchor
        .retain_target(&store_root, &selected_relative)
        .map_err(|source| {
            io_error(
                &authority_root.display_path.join(&selected_relative),
                source,
            )
        })?;
    let retained = retained_anchored_completion_file(
        authority_root,
        &selected_relative,
        completion_handle,
        completion_identity,
        selector.document.completion.byte_length,
    )?;
    if retained.digest != selector.document.completion.content_sha256
        || u64::try_from(retained.bytes.len()).unwrap_or(u64::MAX)
            != selector.document.completion.byte_length
    {
        return Err(RestoreError::Tampered {
            reason: "restore completion selector selected different bytes or length".to_owned(),
        });
    }
    let document: RestoreCompletionAuthorityDocument = serde_json::from_slice(&retained.bytes)
        .map_err(|error| RestoreError::Tampered {
            reason: format!("protected restore completion authority parse failed: {error}"),
        })?;
    let canonical = canonical_restore_completion_authority_bytes(&document)?;
    if canonical != retained.bytes
        || document.schema_version != RESTORE_COMPLETION_AUTHORITY_SCHEMA_VERSION
        || selected_relative != completion_authority_relative_path(directory_relative, &canonical)?
    {
        return Err(RestoreError::Tampered {
            reason: "restore completion authority is noncanonical or not content-addressed"
                .to_owned(),
        });
    }
    validate_restore_completion_selector(
        &selector.document,
        &document,
        authority_root,
        directory_relative,
        selector_relative,
    )?;
    let completion = RetainedRestoreDocument { retained, document };
    let capability = RetainedRestoreCompletion {
        selector,
        completion,
        completion_anchor,
    };
    capability.revalidate()?;
    Ok(Some(capability))
}

fn publish_or_select_completion_authority(
    preflight: &RestorePreflight,
    existing: Option<RetainedRestoreCompletion>,
    expected: &RestoreCompletionAuthorityDocument,
) -> Result<RestoreCompletionAuthority, RestoreError> {
    validate_restore_operation_nonce(&expected.operation_nonce)?;
    let bytes = canonical_restore_completion_authority_bytes(expected)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_RESTORE_COMPLETION_AUTHORITY_BYTES {
        return Err(RestoreError::Tampered {
            reason: "restore completion authority exceeds its configured bound".to_owned(),
        });
    }
    let content_digest = sha256(&bytes);
    let relative =
        completion_authority_relative_path(&preflight.completion_directory_relative, &bytes)?;
    let path = preflight.authority_root.display_path.join(&relative);
    if let Some(actual) = existing {
        validate_same_restore_completion_authority(expected, actual.completion.document())?;
        if actual.selector.retained.relative != preflight.completion_selector_relative
            || actual.selector.retained.path != preflight.completion_selector_path
            || actual.completion.retained.relative != relative
            || actual.completion.retained.path != path
            || actual.completion.retained.bytes != bytes
            || actual.completion.retained.digest != content_digest
        {
            return Err(RestoreError::Tampered {
                reason: "retained restore completion authority has a different content address"
                    .to_owned(),
            });
        }
        validate_restore_completion_selector(
            actual.selector.document(),
            actual.completion.document(),
            &preflight.authority_root,
            &preflight.completion_directory_relative,
            &preflight.completion_selector_relative,
        )?;
        // Existing-reader selection linearizes at this final retained selector,
        // exact lifetime anchor, and completion-record validation. No decisive I/O
        // may follow before returning the capability.
        actual.revalidate()?;
        return Ok(RestoreCompletionAuthority { retained: actual });
    }

    let retained = match publish_private_file_create_new_retained(
        &preflight.authority_root,
        &relative,
        &path,
        &bytes,
    ) {
        Ok(retained) => retained,
        Err(RestoreError::Collision { .. }) => {
            return Err(RestoreError::Interrupted {
                path,
                reason: "restore completion generation appeared after retained absence; caller-created byte equality is never accepted"
                    .to_owned(),
            });
        }
        Err(error) => return Err(error),
    };
    if retained.relative != relative
        || retained.path != path
        || retained.bytes != bytes
        || retained.digest != content_digest
    {
        return Err(RestoreError::Tampered {
            reason: "published restore completion authority lost its content address".to_owned(),
        });
    }
    let completion = RetainedRestoreDocument {
        retained,
        document: expected.clone(),
    };
    completion.revalidate()?;
    preflight
        .authority_root
        .create_dir_all_synced(&preflight.completion_anchor_directory_relative)
        .map_err(|source| io_error(&preflight.completion_anchor_directory_path, source))?;
    let store_root = retained_store_authority_root(&preflight.authority_root)?;
    let completion_anchor = store_root
        .retain_file_lifetime_anchor(
            &preflight.completion_anchor_directory_relative,
            &completion.retained.handle,
            &completion.retained.identity,
            &completion.retained.digest,
            u64::try_from(completion.retained.bytes.len()).unwrap_or(u64::MAX),
        )
        .map_err(|source| io_error(&preflight.completion_anchor_directory_path, source))?;
    completion_anchor
        .validate_retained_file(&completion.retained.handle, &completion.retained.identity)
        .map_err(|source| io_error(&path, source))?;
    completion.revalidate()?;
    let selector_document = completion_selector_document(
        preflight,
        expected,
        &relative,
        &content_digest,
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        completion_anchor.binding(),
    )?;
    validate_restore_completion_selector(
        &selector_document,
        expected,
        &preflight.authority_root,
        &preflight.completion_directory_relative,
        &preflight.completion_selector_relative,
    )?;
    let selector_bytes = canonical_restore_completion_selector_bytes(&selector_document)?;
    completion_anchor
        .validate_retained_file(&completion.retained.handle, &completion.retained.identity)
        .map_err(|source| io_error(&path, source))?;
    validate_restore_completion_record_namespace(&completion)?;

    let selector_retained = match publish_private_file_create_new_retained(
        &preflight.authority_root,
        &preflight.completion_selector_relative,
        &preflight.completion_selector_path,
        &selector_bytes,
    ) {
        Ok(retained) => retained,
        Err(RestoreError::Collision { .. }) => {
            return Err(RestoreError::Interrupted {
                path: preflight.completion_selector_path.clone(),
                reason: "restore completion selector appeared after retained absence; rollback or byte-only collision recovery is forbidden"
                    .to_owned(),
            });
        }
        Err(error) => return Err(error),
    };
    // The fixed protected selector publication is the restore success
    // linearization point. Its retained parent/root capability, immutable record
    // content address, and generation-safe leaf anchor were all validated before
    // this atomic commit. Constructing the opaque result below is pure and no
    // decisive I/O follows.
    Ok(RestoreCompletionAuthority {
        retained: RetainedRestoreCompletion {
            selector: RetainedRestoreDocument {
                retained: selector_retained,
                document: selector_document,
            },
            completion,
            completion_anchor,
        },
    })
}

fn isolate_verified_sidecar_after_completion_error(
    tree: &RetainedVerifiedSidecarTree,
    cause: RestoreError,
) -> RestoreError {
    match restore_vacate_retained_publication(&tree.parent, &tree.root_leaf, &tree.root) {
        Ok(isolation) => RestoreError::Interrupted {
            path: tree.root_path.clone(),
            reason: format!(
                "restore completion authority publication/selection failed ({cause}); authoritative destination was replaced by {isolation}"
            ),
        },
        Err(isolation) => RestoreError::Interrupted {
            path: tree.root_path.clone(),
            reason: format!(
                "restore completion authority publication/selection failed ({cause}); exact sidecar isolation failed: {isolation}"
            ),
        },
    }
}

fn new_restore_operation_nonce() -> Result<String, RestoreError> {
    let mut nonce = [0_u8; 16];
    getrandom::fill(&mut nonce).map_err(|error| RestoreError::Tampered {
        reason: format!("restore operation nonce generation failed: {error}"),
    })?;
    Ok(format!("{:032x}", u128::from_be_bytes(nonce)))
}

fn validate_restore_operation_nonce(value: &str) -> Result<(), RestoreError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(RestoreError::Tampered {
            reason: "restore operation nonce is not a canonical 128-bit identity".to_owned(),
        });
    }
    Ok(())
}

fn restore_journal(
    plan: &RestorePlan,
    staging_path: &Path,
    operation_nonce: &str,
) -> RestoreJournalDocument {
    RestoreJournalDocument {
        schema_version: RESTORE_JOURNAL_SCHEMA_VERSION.to_owned(),
        operation_nonce: operation_nonce.to_owned(),
        project_id: plan.project_link.project_id.0.clone(),
        project_link_sha256: sha256(&plan.project_link_bytes),
        archive_sha256: plan.verified.archive_sha256().to_owned(),
        manifest_set_digest: plan
            .verified
            .manifest()
            .backup_manifest
            .manifest_set_digest
            .clone(),
        destination_sidecar: plan.destination_sidecar.display().to_string(),
        staging_path: staging_path.display().to_string(),
    }
}

fn verify_restore_authority_relative_path(
    authority_root: &RestoreRetainedDirectory,
    relative: &Path,
    path: &Path,
) -> Result<(), RestoreError> {
    restore_relative_components(relative).map_err(|source| RestoreError::InvalidPath {
        path: path.to_path_buf(),
        reason: format!("protected restore path is not descriptor-relative: {source}"),
    })?;
    if authority_root.display_path.join(relative) != path {
        return Err(RestoreError::InvalidPath {
            path: path.to_path_buf(),
            reason: "protected restore path is not bound to its retained authority root".to_owned(),
        });
    }
    Ok(())
}

fn load_protected_restore_file_retained(
    authority_root: &RestoreRetainedDirectory,
    relative: &Path,
    path: &Path,
    maximum: u64,
) -> Result<Option<RetainedProtectedRestoreFile>, RestoreError> {
    verify_restore_authority_relative_path(authority_root, relative, path)?;
    let parent_relative = relative.parent().ok_or_else(|| RestoreError::InvalidPath {
        path: path.to_path_buf(),
        reason: "protected restore document has no retained parent".to_owned(),
    })?;
    let leaf = relative
        .file_name()
        .ok_or_else(|| RestoreError::InvalidPath {
            path: path.to_path_buf(),
            reason: "protected restore document has no leaf".to_owned(),
        })?;
    authority_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&authority_root.display_path, source))?;
    let parent = match authority_root.open_directory_path(parent_relative) {
        Ok(parent) => parent,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(io_error(
                path.parent().unwrap_or(&authority_root.display_path),
                source,
            ));
        }
    };
    authority_root
        .verify_directory_path_identity(parent_relative, &parent.identity)
        .map_err(|source| io_error(&parent.display_path, source))?;
    let (handle, identity) = match parent.open_direct_authority_file_retained(Path::new(leaf)) {
        Ok(retained) => retained,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(path, source)),
    };
    let (bytes, retained_identity) = restore_read_retained_authority_file_bounded(&handle, maximum)
        .map_err(|source| io_error(path, source))?;
    let (namespace_bytes, namespace_identity) = parent
        .read_direct_authority_file_bounded(Path::new(leaf), maximum)
        .map_err(|source| io_error(path, source))?;
    if retained_identity != identity || namespace_identity != identity || namespace_bytes != bytes {
        return Err(RestoreError::Tampered {
            reason: "protected restore document changed while its capability was retained"
                .to_owned(),
        });
    }
    let retained = RetainedProtectedRestoreFile {
        authority_root: authority_root
            .try_clone()
            .map_err(|source| io_error(&authority_root.display_path, source))?,
        parent,
        parent_relative: parent_relative.to_path_buf(),
        relative: relative.to_path_buf(),
        path: path.to_path_buf(),
        leaf: PathBuf::from(leaf),
        handle,
        identity,
        digest: sha256(&bytes),
        bytes,
    };
    retained.revalidate()?;
    Ok(Some(retained))
}

fn load_restore_journal_retained(
    authority_root: &RestoreRetainedDirectory,
    relative: &Path,
    path: &Path,
) -> Result<Option<RetainedRestoreDocument<RestoreJournalDocument>>, RestoreError> {
    load_protected_restore_file_retained(authority_root, relative, path, MAX_RESTORE_RECEIPT_BYTES)?
        .map(|retained| {
            let document: RestoreJournalDocument = serde_json::from_slice(&retained.bytes)
                .map_err(|error| RestoreError::Interrupted {
                    path: path.to_path_buf(),
                    reason: format!("protected restore journal is malformed: {error}"),
                })?;
            let capability = RetainedRestoreDocument { retained, document };
            capability.revalidate()?;
            Ok(capability)
        })
        .transpose()
}

fn publish_or_validate_journal_retained(
    authority_root: &RestoreRetainedDirectory,
    relative: &Path,
    path: &Path,
    expected: &RestoreJournalDocument,
) -> Result<RetainedRestoreDocument<RestoreJournalDocument>, RestoreError> {
    if let Some(actual) = load_restore_journal_retained(authority_root, relative, path)? {
        if actual.document() != expected {
            return Err(RestoreError::Interrupted {
                path: path.to_path_buf(),
                reason: "protected restore journal differs from this transaction".to_owned(),
            });
        }
        actual.revalidate()?;
        return Ok(actual);
    }
    let bytes = serde_json::to_vec(expected).map_err(|error| RestoreError::Tampered {
        reason: format!("restore journal serialization failed: {error}"),
    })?;
    match publish_private_file_create_new_retained(authority_root, relative, path, &bytes) {
        Ok(retained) => {
            let capability = RetainedRestoreDocument {
                retained,
                document: expected.clone(),
            };
            capability.revalidate()?;
            Ok(capability)
        }
        Err(RestoreError::Collision { .. }) => Err(RestoreError::Interrupted {
            path: path.to_path_buf(),
            reason: "restore journal appeared after retained absence was established; byte-only collision recovery is forbidden"
                .to_owned(),
        }),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
fn publish_or_validate_journal(
    path: &Path,
    expected: &RestoreJournalDocument,
) -> Result<(), RestoreError> {
    let authority_path = path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| RestoreError::InvalidPath {
            path: path.to_path_buf(),
            reason: "test restore journal lacks an authority root".to_owned(),
        })?;
    match fs::create_dir(authority_path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(source) => return Err(io_error(authority_path, source)),
    }
    let authority = RestoreRetainedDirectory::open_root(authority_path)
        .map_err(|source| io_error(authority_path, source))?;
    let relative = path
        .strip_prefix(authority_path)
        .map_err(|_| RestoreError::InvalidPath {
            path: path.to_path_buf(),
            reason: "test restore journal escaped its authority root".to_owned(),
        })?;
    publish_or_validate_journal_retained(&authority, relative, path, expected).map(|_| ())
}

fn build_restore_receipt(
    plan: &RestorePlan,
    operation_nonce: &str,
    tree: &RetainedVerifiedSidecarTree,
) -> Result<RestoreReceiptDocument, RestoreError> {
    build_restore_receipt_with_timestamp(plan, operation_nonce, tree, now_unix()?)
}

fn restore_receipt_digest(document: &RestoreReceiptDocument) -> Result<String, RestoreError> {
    let mut value = serde_json::to_value(document).map_err(|error| RestoreError::Tampered {
        reason: format!("restore receipt encoding failed: {error}"),
    })?;
    value
        .get_mut("restore_receipt")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|receipt| receipt.remove("receipt_digest"))
        .ok_or_else(|| RestoreError::Tampered {
            reason: "restore receipt self-digest field is absent".to_owned(),
        })?;
    let canonical =
        serde_json_canonicalizer::to_vec(&value).map_err(|error| RestoreError::Tampered {
            reason: format!("restore receipt canonicalization failed: {error}"),
        })?;
    let mut hasher = Sha256::new();
    hasher.update(RESTORE_RECEIPT_DIGEST_DOMAIN);
    hasher.update((canonical.len() as u64).to_be_bytes());
    hasher.update(canonical);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn publish_or_validate_restore_receipt_retained(
    authority_root: &RestoreRetainedDirectory,
    relative: &Path,
    path: &Path,
    expected: &RestoreReceiptDocument,
) -> Result<RetainedRestoreDocument<RestoreReceiptDocument>, RestoreError> {
    if let Some(actual) = load_restore_receipt_retained(authority_root, relative, path)? {
        validate_same_restore_receipt(expected, actual.document())?;
        actual.revalidate()?;
        return Ok(actual);
    }
    let bytes = serde_json::to_vec(expected).map_err(|error| RestoreError::Tampered {
        reason: format!("restore receipt serialization failed: {error}"),
    })?;
    match publish_private_file_create_new_retained(authority_root, relative, path, &bytes) {
        Ok(retained) => {
            let capability = RetainedRestoreDocument {
                retained,
                document: expected.clone(),
            };
            capability.revalidate()?;
            Ok(capability)
        }
        Err(RestoreError::Collision { .. }) => Err(RestoreError::Interrupted {
            path: path.to_path_buf(),
            reason: "restore receipt appeared after retained absence was established; byte-only collision recovery is forbidden"
                .to_owned(),
        }),
        Err(error) => Err(error),
    }
}

fn load_restore_receipt_retained(
    authority_root: &RestoreRetainedDirectory,
    relative: &Path,
    path: &Path,
) -> Result<Option<RetainedRestoreDocument<RestoreReceiptDocument>>, RestoreError> {
    load_protected_restore_file_retained(authority_root, relative, path, MAX_RESTORE_RECEIPT_BYTES)?
        .map(|retained| {
            let document: RestoreReceiptDocument = serde_json::from_slice(&retained.bytes)
                .map_err(|error| RestoreError::Tampered {
                    reason: format!("protected restore receipt parse failed: {error}"),
                })?;
            let capability = RetainedRestoreDocument { retained, document };
            capability.revalidate()?;
            Ok(capability)
        })
        .transpose()
}

fn validate_restore_receipt_source(
    plan: &RestorePlan,
    receipt: &RestoreReceiptDocument,
) -> Result<(), RestoreError> {
    let backup = &plan.verified.receipt().backup_receipt;
    let value = &receipt.restore_receipt;
    validate_restore_operation_nonce(&value.operation_nonce)?;
    for digest in [
        &value.archive_sha256,
        &value.backup_receipt_digest,
        &value.manifest_set_digest,
        &value.project_link_sha256,
        &value.sidecar_root_path_sha256,
        &value.state_root_path_sha256,
        &value.sidecar_inventory_digest,
        &value.receipt_digest,
    ] {
        digest_token(digest)?;
    }
    if receipt.schema_version != RESTORE_RECEIPT_SCHEMA_VERSION
        || value.receipt_digest != restore_receipt_digest(receipt)?
        || value.archive_sha256.as_str() != plan.verified.archive_sha256()
        || value.backup_receipt_digest.as_str() != backup.receipt_digest.as_str()
        || value.manifest_set_digest.as_str() != backup.manifest_set_digest.as_str()
        || value.project_id.as_str() != backup.project_id.0.as_str()
        || value.project_link_sha256.as_str() != backup.project_link_sha256.as_str()
        || value.workflow_release != backup.workflow_release
        || value.effective_bundle != backup.effective_epoch.effective_bundle
        || value.replay_monotonic_head != backup.replay_monotonic_head
        || value.destination_sidecar != plan.destination_sidecar.display().to_string()
    {
        return Err(RestoreError::Tampered {
            reason: "protected restore receipt source bindings are stale or tampered".to_owned(),
        });
    }
    Ok(())
}

fn validate_restore_receipt_for_sidecar(
    plan: &RestorePlan,
    operation_nonce: &str,
    tree: &RetainedVerifiedSidecarTree,
    receipt: &RestoreReceiptDocument,
) -> Result<(), RestoreError> {
    validate_restore_receipt_source(plan, receipt)?;
    let expected = build_restore_receipt_with_timestamp(
        plan,
        operation_nonce,
        tree,
        receipt.restore_receipt.restored_at_unix,
    )?;
    validate_same_restore_receipt(&expected, receipt)
}

fn build_restore_receipt_with_timestamp(
    plan: &RestorePlan,
    operation_nonce: &str,
    tree: &RetainedVerifiedSidecarTree,
    restored_at_unix: u64,
) -> Result<RestoreReceiptDocument, RestoreError> {
    validate_restore_operation_nonce(operation_nonce)?;
    if tree.root_path != plan.destination_sidecar {
        return Err(RestoreError::Tampered {
            reason: "restore receipt sidecar binding describes a different destination".to_owned(),
        });
    }
    let (_, sidecar_inventory_digest) = restore_completion_inventory(tree)?;
    let backup = &plan.verified.receipt().backup_receipt;
    let mut document = RestoreReceiptDocument {
        schema_version: RESTORE_RECEIPT_SCHEMA_VERSION.to_owned(),
        restore_receipt: RestoreReceipt {
            operation_nonce: operation_nonce.to_owned(),
            archive_sha256: plan.verified.archive_sha256().to_owned(),
            backup_receipt_digest: backup.receipt_digest.clone(),
            manifest_set_digest: backup.manifest_set_digest.clone(),
            project_id: backup.project_id.0.clone(),
            project_link_sha256: backup.project_link_sha256.clone(),
            workflow_release: backup.workflow_release.clone(),
            effective_bundle: backup.effective_epoch.effective_bundle.clone(),
            replay_monotonic_head: backup.replay_monotonic_head.clone(),
            destination_sidecar: plan.destination_sidecar.display().to_string(),
            sidecar_root_path_sha256: restore_path_digest(&tree.root_path)?,
            state_root_path_sha256: restore_path_digest(
                &tree.root_path.join(DESTINATION_STATE_LEAF),
            )?,
            sidecar_inventory_digest,
            restored_at_unix,
            receipt_digest: String::new(),
        },
    };
    document.restore_receipt.receipt_digest = restore_receipt_digest(&document)?;
    Ok(document)
}

fn validate_same_restore_receipt(
    expected: &RestoreReceiptDocument,
    actual: &RestoreReceiptDocument,
) -> Result<(), RestoreError> {
    if actual.schema_version != RESTORE_RECEIPT_SCHEMA_VERSION
        || actual.restore_receipt.receipt_digest != restore_receipt_digest(actual)?
        || actual != expected
    {
        return Err(RestoreError::Tampered {
            reason: "protected restore receipt is stale, substituted, or tampered".to_owned(),
        });
    }
    Ok(())
}

fn publication_from_receipt(
    plan: &RestorePlan,
    receipt_path: &Path,
    receipt: &RestoreReceiptDocument,
    member_count: usize,
    already_restored: bool,
    completion_authority: RestoreCompletionAuthority,
) -> RestorePublication {
    RestorePublication {
        destination_sidecar: plan.destination_sidecar.clone(),
        archive_sha256: plan.verified.archive_sha256().to_owned(),
        manifest_set_digest: plan
            .verified
            .manifest()
            .backup_manifest
            .manifest_set_digest
            .clone(),
        receipt_path: receipt_path.to_path_buf(),
        receipt_digest: receipt.restore_receipt.receipt_digest.clone(),
        member_count,
        already_restored,
        completion_authority,
    }
}

fn acquire_restore_transaction_lock(
    authority_root: &RestoreRetainedDirectory,
    project_token: &str,
) -> Result<(PathBuf, crate::retained_dir::RetainedFileIdentity, File), RestoreError> {
    authority_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&authority_root.display_path, source))?;
    let directory_relative = PathBuf::from("restore-locks");
    let directory = authority_root
        .create_dir_all_synced(&directory_relative)
        .map_err(|source| {
            io_error(
                &authority_root.display_path.join(&directory_relative),
                source,
            )
        })?;
    authority_root
        .verify_directory_path_identity(&directory_relative, &directory.identity)
        .map_err(|source| io_error(&directory.display_path, source))?;
    let leaf = PathBuf::from(format!("{project_token}.lock"));
    let relative = directory_relative.join(&leaf);
    let path = authority_root.display_path.join(&relative);
    let (file, identity) = directory
        .open_or_create_direct_file(&leaf)
        .map_err(|source| io_error(&path, source))?;
    file.sync_all().map_err(|source| io_error(&path, source))?;
    directory
        .sync_self()
        .map_err(|source| io_error(&directory.display_path, source))?;
    FileExt::try_lock(&file).map_err(|source| RestoreError::Collision {
        path: path.clone(),
        reason: format!("another restore transaction owns this project lock: {source}"),
    })?;
    verify_restore_transaction_lock(authority_root, &relative, &identity, &file)?;
    Ok((relative, identity, file))
}

fn verify_restore_transaction_lock(
    authority_root: &RestoreRetainedDirectory,
    relative: &Path,
    expected: &crate::retained_dir::RetainedFileIdentity,
    retained: &File,
) -> Result<(), RestoreError> {
    let path = authority_root.display_path.join(relative);
    let retained_identity =
        validate_restore_file_handle(retained).map_err(|source| io_error(&path, source))?;
    if &retained_identity != expected {
        return Err(RestoreError::Tampered {
            reason: "retained restore transaction lock changed identity".to_owned(),
        });
    }
    let parent_relative = relative.parent().ok_or_else(|| RestoreError::InvalidPath {
        path: path.clone(),
        reason: "restore transaction lock has no retained parent".to_owned(),
    })?;
    let leaf = relative
        .file_name()
        .ok_or_else(|| RestoreError::InvalidPath {
            path: path.clone(),
            reason: "restore transaction lock has no leaf".to_owned(),
        })?;
    authority_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&authority_root.display_path, source))?;
    let parent = authority_root
        .open_directory_path(parent_relative)
        .map_err(|source| {
            io_error(
                path.parent().unwrap_or(&authority_root.display_path),
                source,
            )
        })?;
    authority_root
        .verify_directory_path_identity(parent_relative, &parent.identity)
        .map_err(|source| io_error(&parent.display_path, source))?;
    let current = parent
        .direct_file_identity(Path::new(leaf))
        .map_err(|source| io_error(&path, source))?;
    if &current != expected {
        return Err(RestoreError::Tampered {
            reason: "restore transaction lock was substituted under its retained authority root"
                .to_owned(),
        });
    }
    authority_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&authority_root.display_path, source))
}

fn retained_published_protected_file(
    authority_root: RestoreRetainedDirectory,
    parent: RestoreRetainedDirectory,
    parent_relative: &Path,
    relative: &Path,
    path: &Path,
    leaf: &std::ffi::OsStr,
    handle: File,
    identity: crate::retained_dir::RetainedFileIdentity,
    bytes: &[u8],
) -> RetainedProtectedRestoreFile {
    RetainedProtectedRestoreFile {
        authority_root,
        parent,
        parent_relative: parent_relative.to_path_buf(),
        relative: relative.to_path_buf(),
        path: path.to_path_buf(),
        leaf: PathBuf::from(leaf),
        handle,
        identity,
        bytes: bytes.to_vec(),
        digest: sha256(bytes),
    }
}

fn publish_private_file_create_new_retained(
    authority_root: &RestoreRetainedDirectory,
    relative: &Path,
    path: &Path,
    bytes: &[u8],
) -> Result<RetainedProtectedRestoreFile, RestoreError> {
    verify_restore_authority_relative_path(authority_root, relative, path)?;
    let parent_relative = relative.parent().ok_or_else(|| RestoreError::InvalidPath {
        path: path.to_path_buf(),
        reason: "protected restore file has no retained parent".to_owned(),
    })?;
    let leaf = relative
        .file_name()
        .ok_or_else(|| RestoreError::InvalidPath {
            path: path.to_path_buf(),
            reason: "protected restore file has no leaf".to_owned(),
        })?;
    authority_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&authority_root.display_path, source))?;
    let parent = authority_root
        .create_dir_all_synced(parent_relative)
        .map_err(|source| {
            io_error(
                path.parent().unwrap_or(&authority_root.display_path),
                source,
            )
        })?;
    authority_root
        .verify_directory_path_identity(parent_relative, &parent.identity)
        .map_err(|source| io_error(&parent.display_path, source))?;
    let retained_authority_root = authority_root
        .try_clone()
        .map_err(|source| io_error(&authority_root.display_path, source))?;
    let leaf_label = leaf.to_string_lossy();
    let mut temporary_nonce = [0_u8; 16];
    getrandom::fill(&mut temporary_nonce).map_err(|error| RestoreError::Tampered {
        reason: format!("protected restore temp nonce generation failed: {error}"),
    })?;
    let temporary_nonce = u128::from_le_bytes(temporary_nonce);
    let mut selected = None;
    for attempt in 0..32 {
        let temporary = PathBuf::from(format!(
            ".{leaf_label}.{}-{temporary_nonce}-{attempt}.restore-tmp",
            std::process::id()
        ));
        match parent.write_direct_file_new_validated(&temporary, bytes) {
            Ok(identity) => {
                selected = Some((temporary, identity));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => return Err(io_error(&parent.display_path.join(&temporary), source)),
        }
    }
    let (temporary, temporary_identity) = selected.ok_or_else(|| RestoreError::Collision {
        path: path.to_path_buf(),
        reason: "protected restore temp-name collision retry exhausted".to_owned(),
    })?;
    #[cfg(windows)]
    let retained_mode = RestoreRelativeOpen::FileDelete;
    #[cfg(not(windows))]
    let retained_mode = RestoreRelativeOpen::FileRead;
    let retained_temporary = match restore_open_relative(&parent.handle, &temporary, retained_mode)
    {
        Ok(retained) => retained,
        Err(source) => {
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "ios",
                target_os = "tvos",
                target_os = "watchos",
                target_os = "visionos"
            ))]
            let isolation = restore_isolate_protected_file_name(
                &parent,
                Path::new(leaf),
                "protected-file-retained-open-failure",
            );
            let _ = parent.remove_direct_file_if_identity(&temporary, &temporary_identity);
            let _ = parent.sync_self();
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "ios",
                target_os = "tvos",
                target_os = "watchos",
                target_os = "visionos"
            ))]
            if let Err(isolation) = isolation {
                return Err(RestoreError::Interrupted {
                    path: path.to_path_buf(),
                    reason: format!(
                        "protected restore temporary could not be retained ({source}) and blocker isolation failed ({isolation})"
                    ),
                });
            }
            return Err(io_error(&parent.display_path.join(&temporary), source));
        }
    };
    let retained_validation = restore_read_retained_file_bounded(
        &retained_temporary,
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    );
    if !matches!(
        &retained_validation,
        Ok((retained_bytes, retained_identity))
            if retained_identity == &temporary_identity && retained_bytes.as_slice() == bytes
    ) {
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        ))]
        let isolation = restore_isolate_protected_file_name(
            &parent,
            Path::new(leaf),
            "protected-file-prepublication-failure",
        );
        drop(retained_temporary);
        let _ = parent.remove_direct_file_if_identity(&temporary, &temporary_identity);
        let _ = parent.sync_self();
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos"
        ))]
        if let Err(isolation) = isolation {
            return Err(RestoreError::Interrupted {
                path: path.to_path_buf(),
                reason: format!(
                    "protected restore temporary was substituted before retained publication and blocker isolation failed: {isolation}"
                ),
            });
        }
        return Err(RestoreError::Tampered {
            reason: "protected restore temporary was substituted before retained publication"
                .to_owned(),
        });
    }

    if let Err(source) = restore_publish_retained_file_noreplace(
        authority_root,
        parent_relative,
        &parent,
        &temporary,
        Path::new(leaf),
        &retained_temporary,
        &temporary_identity,
        bytes,
    ) {
        drop(retained_temporary);
        let _ = parent.remove_direct_file_if_identity(&temporary, &temporary_identity);
        let _ = parent.sync_self();
        return if source.kind() == io::ErrorKind::AlreadyExists {
            Err(RestoreError::Collision {
                path: path.to_path_buf(),
                reason: "protected restore file appeared during retained atomic publication and was not accepted by bytes"
                    .to_owned(),
            })
        } else {
            Err(io_error(path, source))
        };
    }

    // The platform publication primitive performs the closing exact-handle,
    // parent, configured-root, and durability sweep. All fallible preparation was
    // completed before that atomic publication; assembling the retained result is
    // now pure and performs no later decisive I/O.
    Ok(retained_published_protected_file(
        retained_authority_root,
        parent,
        parent_relative,
        relative,
        path,
        leaf,
        retained_temporary,
        temporary_identity,
        bytes,
    ))
}

fn cleanup_completed_journal_retained(
    authority_root: &RestoreRetainedDirectory,
    relative: &Path,
    path: &Path,
    expected: &RestoreJournalDocument,
) -> Result<(), RestoreError> {
    verify_restore_authority_relative_path(authority_root, relative, path)?;
    let parent_relative = relative.parent().ok_or_else(|| RestoreError::InvalidPath {
        path: path.to_path_buf(),
        reason: "protected restore journal has no retained parent".to_owned(),
    })?;
    let leaf = relative
        .file_name()
        .ok_or_else(|| RestoreError::InvalidPath {
            path: path.to_path_buf(),
            reason: "protected restore journal has no leaf".to_owned(),
        })?;
    authority_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&authority_root.display_path, source))?;
    let parent = match authority_root.open_directory_path(parent_relative) {
        Ok(parent) => parent,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            authority_root
                .verify_namespace_identity()
                .map_err(|source| io_error(&authority_root.display_path, source))?;
            return Ok(());
        }
        Err(source) => {
            return Err(io_error(
                path.parent().unwrap_or(&authority_root.display_path),
                source,
            ));
        }
    };
    let Some((raw, identity)) = parent
        .read_optional_direct_file_bounded(Path::new(leaf), MAX_RESTORE_RECEIPT_BYTES)
        .map_err(|source| io_error(path, source))?
    else {
        authority_root
            .verify_namespace_identity()
            .map_err(|source| io_error(&authority_root.display_path, source))?;
        return Ok(());
    };
    let actual: RestoreJournalDocument =
        serde_json::from_slice(&raw).map_err(|error| RestoreError::Interrupted {
            path: path.to_path_buf(),
            reason: format!("refusing to remove malformed restore journal: {error}"),
        })?;
    if actual != *expected {
        return Err(RestoreError::Interrupted {
            path: path.to_path_buf(),
            reason: "refusing to remove a substituted restore journal".to_owned(),
        });
    }
    authority_root
        .verify_directory_path_identity(parent_relative, &parent.identity)
        .map_err(|source| io_error(&parent.display_path, source))?;
    parent
        .remove_direct_file_if_identity(Path::new(leaf), &identity)
        .map_err(|source| io_error(path, source))?;
    authority_root
        .verify_directory_path_identity(parent_relative, &parent.identity)
        .map_err(|source| io_error(&parent.display_path, source))?;
    parent
        .sync_self()
        .map_err(|source| io_error(&parent.display_path, source))?;
    authority_root
        .verify_namespace_identity()
        .map_err(|source| io_error(&authority_root.display_path, source))
}

#[cfg(test)]
fn cleanup_completed_journal(
    path: &Path,
    expected: &RestoreJournalDocument,
) -> Result<(), RestoreError> {
    let authority_path = path.parent().ok_or_else(|| RestoreError::InvalidPath {
        path: path.to_path_buf(),
        reason: "protected restore journal has no parent".to_owned(),
    })?;
    let authority = RestoreRetainedDirectory::open_root(authority_path)
        .map_err(|source| io_error(authority_path, source))?;
    let leaf = path.file_name().ok_or_else(|| RestoreError::InvalidPath {
        path: path.to_path_buf(),
        reason: "protected restore journal has no leaf".to_owned(),
    })?;
    cleanup_completed_journal_retained(&authority, Path::new(leaf), path, expected)
}

fn cleanup_validated_staging_retained(
    staging_parent: &RestoreRetainedDirectory,
    staging_leaf: &Path,
    staging: &Path,
    members: &[RestoreMember],
    manifest: &BackupManifestDocument,
) -> Result<(), RestoreError> {
    let Some(root) = staging_parent
        .open_optional_directory(staging_leaf)
        .map_err(|source| io_error(staging, source))?
    else {
        return Ok(());
    };
    let validated = verify_staging_exact_retained(root, members, manifest)?;
    remove_validated_staging_tree(staging_parent, staging_leaf, staging, validated)
}

#[cfg(test)]
fn cleanup_validated_staging(
    staging: &Path,
    members: &[RestoreMember],
    manifest: &BackupManifestDocument,
) -> Result<(), RestoreError> {
    let parent_path = staging.parent().ok_or_else(|| RestoreError::InvalidPath {
        path: staging.to_path_buf(),
        reason: "restore staging has no parent".to_owned(),
    })?;
    let leaf = staging
        .file_name()
        .ok_or_else(|| RestoreError::InvalidPath {
            path: staging.to_path_buf(),
            reason: "restore staging has no leaf".to_owned(),
        })?;
    let parent = RestoreRetainedDirectory::open_root(parent_path)
        .map_err(|source| io_error(parent_path, source))?;
    cleanup_validated_staging_retained(&parent, Path::new(leaf), staging, members, manifest)
}

fn rollback_staging_after_error_retained(
    staging_parent: &RestoreRetainedDirectory,
    staging_leaf: &Path,
    staging: &Path,
    members: &[RestoreMember],
) {
    let validated = staging_parent
        .open_optional_directory(staging_leaf)
        .map_err(|source| io_error(staging, source))
        .and_then(|root| {
            root.map(|root| verify_staging_prefix_retained(root, members))
                .transpose()
        });
    if let Ok(Some(validated)) = validated {
        let _ = remove_validated_staging_tree(staging_parent, staging_leaf, staging, validated);
    }
}

#[cfg(test)]
fn rollback_staging_after_error(staging: &Path, members: &[RestoreMember]) {
    let Some(parent_path) = staging.parent() else {
        return;
    };
    let Some(leaf) = staging.file_name() else {
        return;
    };
    if let Ok(parent) = RestoreRetainedDirectory::open_root(parent_path) {
        rollback_staging_after_error_retained(&parent, Path::new(leaf), staging, members);
    }
}

fn remove_validated_staging_tree(
    staging_parent: &RestoreRetainedDirectory,
    staging_leaf: &Path,
    staging: &Path,
    validated: ValidatedStagingTree,
) -> Result<(), RestoreError> {
    staging_parent
        .verify_namespace_identity()
        .map_err(|source| io_error(&staging_parent.display_path, source))?;

    #[cfg(unix)]
    {
        validated
            .root
            .verify_identity()
            .map_err(|source| io_error(staging, source))?;
        staging_parent
            .verify_direct_directory_identity(staging_leaf, &validated.root.identity)
            .map_err(|source| io_error(staging, source))?;
        let orphan = restore_quarantine_relative_noreplace(
            &staging_parent.handle,
            staging_leaf,
            "validated-staging-orphan",
        )
        .map_err(|source| io_error(staging, source))?;
        let isolated = staging_parent
            .open_directory(&orphan)
            .map_err(|source| io_error(&staging_parent.display_path.join(&orphan), source))?;
        if isolated.identity != validated.root.identity {
            return Err(RestoreError::Interrupted {
                path: staging.to_path_buf(),
                reason: "validated restore staging changed during retained orphan isolation"
                    .to_owned(),
            });
        }
        validated
            .root
            .verify_identity()
            .map_err(|source| io_error(staging, source))?;
        for _ in 0..32 {
            match restore_quarantine_relative_noreplace(
                &staging_parent.handle,
                staging_leaf,
                "validated-staging-repopulation",
            ) {
                Ok(_) => {}
                Err(source) if source.kind() == io::ErrorKind::NotFound => {
                    staging_parent
                        .sync_self()
                        .map_err(|source| io_error(&staging_parent.display_path, source))?;
                    let reopened = staging_parent.open_directory(&orphan).map_err(|source| {
                        io_error(&staging_parent.display_path.join(&orphan), source)
                    })?;
                    if reopened.identity != validated.root.identity {
                        return Err(RestoreError::Interrupted {
                            path: staging.to_path_buf(),
                            reason:
                                "isolated restore staging was substituted before orphan retention"
                                    .to_owned(),
                        });
                    }
                    validated
                        .root
                        .verify_identity()
                        .map_err(|source| io_error(staging, source))?;
                    staging_parent
                        .verify_namespace_identity()
                        .map_err(|source| io_error(&staging_parent.display_path, source))?;
                    return Ok(());
                }
                Err(source) => return Err(io_error(staging, source)),
            }
        }
        Err(RestoreError::Interrupted {
            path: staging.to_path_buf(),
            reason: "bounded validated-staging isolation was continuously repopulated".to_owned(),
        })
    }

    #[cfg(not(unix))]
    {
        let directories = validated.directories;
        let mut files = validated.files.into_iter().collect::<Vec<_>>();
        files.sort_by_key(|(path, _)| std::cmp::Reverse(path.components().count()));
        for (relative, identity) in files {
            validated
                .root
                .remove_file_path_if_identity(&relative, &directories, &identity)
                .map_err(|source| io_error(&staging.join(&relative), source))?;
        }
        let mut directory_paths = directories.keys().cloned().collect::<Vec<_>>();
        directory_paths.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        for relative in directory_paths {
            let identity = directories
                .get(&relative)
                .expect("validated directory identity exists");
            validated
                .root
                .remove_directory_path_if_identity(&relative, &directories, identity)
                .map_err(|source| io_error(&staging.join(&relative), source))?;
        }
        validated
            .root
            .verify_identity()
            .map_err(|source| io_error(staging, source))?;
        if !validated
            .root
            .direct_entries()
            .map_err(|source| io_error(staging, source))?
            .is_empty()
        {
            return Err(RestoreError::Interrupted {
                path: staging.to_path_buf(),
                reason: "refusing to remove staging after its validated tree changed".to_owned(),
            });
        }
        staging_parent
            .verify_direct_directory_identity(staging_leaf, &validated.root.identity)
            .map_err(|source| io_error(staging, source))?;
        staging_parent
            .verify_namespace_identity()
            .map_err(|source| io_error(&staging_parent.display_path, source))?;
        let root_identity = validated.root.identity.clone();
        drop(validated.root);
        staging_parent
            .remove_direct_directory_if_identity(staging_leaf, &root_identity)
            .map_err(|source| io_error(staging, source))?;
        staging_parent
            .verify_namespace_identity()
            .map_err(|source| io_error(&staging_parent.display_path, source))?;
        staging_parent
            .sync_self()
            .map_err(|source| io_error(&staging_parent.display_path, source))
    }
}

fn ensure_project_link_unchanged(plan: &RestorePlan) -> Result<(), RestoreError> {
    if plan.project_root_retained.display_path != plan.project_root
        || plan.project_root.join(&plan.project_link_leaf) != plan.project_link_path
    {
        return Err(RestoreError::Tampered {
            reason: "retained Project Link bindings no longer describe the planned project root"
                .to_owned(),
        });
    }
    plan.project_root_retained
        .verify_identity()
        .map_err(|source| RestoreError::Tampered {
            reason: format!("retained project-root descriptor changed identity: {source}"),
        })?;
    plan.project_root_retained
        .verify_namespace_identity()
        .map_err(|source| RestoreError::Tampered {
            reason: format!("project-root namespace changed during restore: {source}"),
        })?;
    let retained_link_identity =
        validate_restore_file_handle(&plan.project_link_file).map_err(|source| {
            RestoreError::Tampered {
                reason: format!("retained Project Link descriptor is no longer exact: {source}"),
            }
        })?;
    if retained_link_identity != plan.project_link_identity {
        return Err(RestoreError::Tampered {
            reason: "retained Project Link descriptor changed identity".to_owned(),
        });
    }
    let (raw, namespace_link_identity) = plan
        .project_root_retained
        .read_direct_file_bounded(&plan.project_link_leaf, MAX_RESTORE_AUTHORITY_BYTES)
        .map_err(|source| RestoreError::Tampered {
            reason: format!(
                "Project Link namespace is missing, linked, special, or unreadable: {source}"
            ),
        })?;
    if namespace_link_identity != plan.project_link_identity || raw != plan.project_link_bytes {
        return Err(RestoreError::Tampered {
            reason: "Project Link namespace, leaf identity, or bytes changed during restore; implicit replacement is forbidden"
                .to_owned(),
        });
    }
    if validate_restore_file_handle(&plan.project_link_file).map_err(|source| {
        RestoreError::Tampered {
            reason: format!("retained Project Link descriptor changed after read: {source}"),
        }
    })? != plan.project_link_identity
    {
        return Err(RestoreError::Tampered {
            reason: "retained Project Link descriptor changed identity after read".to_owned(),
        });
    }
    plan.project_root_retained
        .verify_namespace_identity()
        .map_err(|source| RestoreError::Tampered {
            reason: format!("project-root namespace changed after Project Link read: {source}"),
        })
}

fn normalized_destination(project_root: &Path, configured: &str) -> Result<PathBuf, RestoreError> {
    let lexical = lexically_normalize_absolute(&project_root.join(configured))?;
    let mut ancestor = lexical.as_path();
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(RestoreError::InvalidPath {
                        path: ancestor.to_path_buf(),
                        reason: "destination ancestor is a link or reparse point".to_owned(),
                    });
                }
                let mut resolved =
                    fs::canonicalize(ancestor).map_err(|source| io_error(ancestor, source))?;
                for component in suffix.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                // A non-directory destination ancestor is classified later through
                // the retained parent as an operator-owned collision. Treat it like
                // a missing suffix here so planning never probes through that leaf
                // or leaks a less-specific ENOTDIR environment error.
                let leaf = ancestor
                    .file_name()
                    .ok_or_else(|| RestoreError::InvalidPath {
                        path: lexical.clone(),
                        reason: "destination has no existing ancestor".to_owned(),
                    })?;
                suffix.push(leaf.to_os_string());
                ancestor = ancestor.parent().ok_or_else(|| RestoreError::InvalidPath {
                    path: lexical.clone(),
                    reason: "destination has no existing ancestor".to_owned(),
                })?;
            }
            Err(error) => return Err(io_error(ancestor, error)),
        }
    }
}

fn lexically_normalize_absolute(path: &Path) -> Result<PathBuf, RestoreError> {
    if !path.is_absolute() {
        return Err(RestoreError::InvalidPath {
            path: path.to_path_buf(),
            reason: "destination must resolve from an absolute project root".to_owned(),
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
                    return Err(RestoreError::InvalidPath {
                        path: path.to_path_buf(),
                        reason: "destination escapes the filesystem root".to_owned(),
                    });
                }
            }
        }
    }
    Ok(normalized)
}

fn normalized_relative_path(value: &str) -> Result<PathBuf, RestoreError> {
    let path = Path::new(value);
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            _ => {
                return Err(RestoreError::Tampered {
                    reason: format!("archive destination is not normalized: {value}"),
                });
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(RestoreError::Tampered {
            reason: "archive destination is empty".to_owned(),
        });
    }
    Ok(normalized)
}

fn read_nofollow_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, RestoreError> {
    read_file_bounded(path, maximum).map_err(RestoreError::Backup)
}

fn slash_path(path: &Path) -> Result<String, RestoreError> {
    let mut values = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(RestoreError::Tampered {
                reason: "destination path is not normalized".to_owned(),
            });
        };
        values.push(value.to_str().ok_or_else(|| RestoreError::Tampered {
            reason: "destination path is not UTF-8".to_owned(),
        })?);
    }
    Ok(values.join("/"))
}

fn safe_component(value: &str) -> Result<String, RestoreError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(RestoreError::Tampered {
            reason: "project identity is not safe for protected restore storage".to_owned(),
        });
    }
    Ok(value.to_owned())
}

fn digest_token(value: &str) -> Result<&str, RestoreError> {
    value
        .strip_prefix("sha256:")
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| RestoreError::Tampered {
            reason: "archive digest is not a SHA-256 identity".to_owned(),
        })
}

#[cfg(windows)]
const fn host_destination_platform() -> BackupDestinationPlatform {
    BackupDestinationPlatform::Windows
}

#[cfg(not(windows))]
const fn host_destination_platform() -> BackupDestinationPlatform {
    BackupDestinationPlatform::Posix
}

fn now_unix() -> Result<u64, RestoreError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| RestoreError::Tampered {
            reason: format!("system clock is before Unix epoch: {error}"),
        })
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn retained_hard_link_count(file: &File, metadata: &fs::Metadata) -> io::Result<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let _ = file;
        Ok(metadata.nlink())
    }
    #[cfg(windows)]
    {
        let _ = metadata;
        Ok(crate::windows_file_info::file_information(file)?.number_of_links)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (file, metadata);
        Ok(0)
    }
}

#[cfg(all(test, unix))]
fn path_hard_link_count(_path: &Path, metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    metadata.nlink()
}

#[cfg(all(test, windows))]
fn path_hard_link_count(path: &Path, _metadata: &fs::Metadata) -> u64 {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .and_then(|file| crate::windows_file_info::file_information(&file))
        .map_or(0, |information| information.number_of_links)
}

#[cfg(all(test, not(any(unix, windows))))]
fn path_hard_link_count(_path: &Path, _metadata: &fs::Metadata) -> u64 {
    0
}

fn io_error(path: &Path, source: io::Error) -> RestoreError {
    RestoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
#[path = "restore_tests.rs"]
mod tests;
