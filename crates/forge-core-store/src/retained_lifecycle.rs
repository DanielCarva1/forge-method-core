//! Sealed descriptor-relative I/O for the Domain Pack lifecycle store.
//!
//! The capability owns the exact effect lock plus retained state and lifecycle
//! directory handles. Callers can address only the Domain Pack namespace, and
//! every operation revalidates the producer boundary, lock leaf, state root,
//! and lifecycle-root identity before success.

use crate::crash_replace::CrashReplaceError;
use crate::retained_crash_replace::{
    reconcile_file_crash_safe_at_owned_retained_target,
    replace_file_crash_safe_at_retained_target_with_witness, ConsumedRetainedCrashReplaceAbsence,
    ConsumedRetainedCrashReplaceLeaf, RetainedCrashReplaceSession, RetainedCrashReplaceTarget,
    RetainedExpectedTarget,
};
use crate::retained_dir::{
    RetainedDirectory, RetainedFileAnchorBinding, RetainedFileIdentity, RetainedFileLifetimeAnchor,
};
use crate::retained_project_tree::{
    RetainedProjectAnchorBinding, RetainedProjectLifetimeAnchors, RetainedProjectRootBinding,
    RetainedProjectTree, RetainedProjectTreeError,
};
use crate::{sha256_content_hash, EffectStoreLock, EffectStoreLockError};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::File;
use std::io::{self, Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Component, Path, PathBuf};

const DOMAIN_PACK_LIFECYCLE_ROOT: &str = "domain-packs";
const DOMAIN_PACK_LIFECYCLE_LOCK: &str = "locks/domain-packs.lifecycle.lock";
const DOMAIN_PACK_ACTIVE_POINTER: &str = "domain-packs/active.lock.yaml";
const DOMAIN_PACK_COMPLETION_SCHEMA_VERSION: &str = "forge-domain-pack-lifecycle-completion-v3";
const DOMAIN_PACK_COMPLETION_SELECTOR_SCHEMA_VERSION: &str =
    "forge-domain-pack-lifecycle-completion-selector-v1";
const DOMAIN_PACK_COMPLETION_MAX_BYTES: u64 = 512 * 1024 * 1024;
const DOMAIN_PACK_COMPLETION_RECORD_LEAF: &str = "completion.record";
const DOMAIN_PACK_COMPLETION_SELECTOR_LEAF: &str = "completion.selector";
const DOMAIN_PACK_LIFECYCLE_ANCHOR_ROOT: &str = ".forge-lifecycle-anchors";

/// Store-owned capability for one exact Domain Pack lifecycle root and lock.
///
/// Construction consumes the effect lock. There is no public field, clone, or
/// generic root accessor, so lifecycle authority cannot be detached from the
/// retained root/lock pair or redirected into another namespace.
#[derive(Debug)]
pub struct RetainedDomainPackLifecycleStore {
    lock: EffectStoreLock,
    state_root_identity: RetainedFileIdentity,
    lifecycle_root: RetainedDirectory,
    lifecycle_root_identity: RetainedFileIdentity,
    project_root_binding: Option<RetainedProjectRootBinding>,
}

/// Opaque exact active-pointer leaf retained beneath one lifecycle Store.
///
/// The witness owns the opened file, its exact bytes and digest, and the
/// lifecycle-root identity. Only the Store that created it can revalidate or
/// consume it as compare-and-swap authority.
pub struct RetainedDomainPackActivePointerWitness {
    state_root_identity: RetainedFileIdentity,
    lifecycle_root_identity: RetainedFileIdentity,
    lifecycle_lock_identity: RetainedFileIdentity,
    file: File,
    identity: RetainedFileIdentity,
    bytes: Vec<u8>,
    digest: String,
    maximum: u64,
}

/// Opaque Store-minted proof that the fixed active pointer was absent beneath
/// one exact retained lifecycle root and producer lock.
pub struct RetainedDomainPackActivePointerAbsenceWitness {
    state_root_identity: RetainedFileIdentity,
    lifecycle_root_identity: RetainedFileIdentity,
    lifecycle_lock_identity: RetainedFileIdentity,
    reconciled_binding: Option<ConsumedRetainedCrashReplaceAbsence>,
}

/// Exact active-pointer state accepted by production lifecycle replacement.
#[derive(Debug)]
pub enum RetainedDomainPackExpectedActivePointer {
    Present(RetainedDomainPackActivePointerWitness),
    Absent(RetainedDomainPackActivePointerAbsenceWitness),
}

impl fmt::Debug for RetainedDomainPackActivePointerWitness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedDomainPackActivePointerWitness")
            .field("byte_length", &self.bytes.len())
            .field("digest", &self.digest)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for RetainedDomainPackActivePointerAbsenceWitness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedDomainPackActivePointerAbsenceWitness")
            .finish_non_exhaustive()
    }
}

impl RetainedDomainPackActivePointerWitness {
    #[must_use]
    pub fn raw_bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

impl RetainedDomainPackExpectedActivePointer {
    #[must_use]
    pub fn raw_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Present(witness) => Some(witness.raw_bytes()),
            Self::Absent(_) => None,
        }
    }

    #[must_use]
    pub fn present(&self) -> Option<&RetainedDomainPackActivePointerWitness> {
        match self {
            Self::Present(witness) => Some(witness),
            Self::Absent(_) => None,
        }
    }
}

/// Untrusted semantic identities used by the Store to derive the exact fixed
/// lifecycle paths that a canonical completion record must bind.
///
/// This is data, not authority. The Store derives every path, rereads every leaf
/// through retained descriptors, and compares the resulting exact identities
/// before it can publish the opaque completion authority.
#[derive(Debug, Clone, Copy)]
pub struct DomainPackLifecycleCompletionInput<'a> {
    pub project_id: &'a str,
    pub project_snapshot_digest: &'a str,
    pub generation: u64,
    pub ledger_record_digest: &'a str,
    pub lock_digest: &'a str,
    pub preflight_digest: &'a str,
    pub compatibility_report_digest: &'a str,
    pub receipt_digest: &'a str,
    pub object_raw_digests: &'a [String],
}

/// Opaque Store-owned proof that one immutable canonical lifecycle completion
/// record was selected by an independently committed immutable selector under
/// the exact retained lifecycle lock.
///
/// There is no public constructor, field, clone, or serde surface. Atomic
/// selector publication is the operation's success linearization point.
pub struct RetainedDomainPackLifecycleCompletion {
    record_digest: String,
    selector: RetainedLifecycleSelectedLeaf,
    record_file: File,
    record_identity: RetainedFileIdentity,
    record_path: PathBuf,
    record_anchor: RetainedFileLifetimeAnchor,
    project_anchors: RetainedProjectLifetimeAnchors,
    parent_anchor: RetainedFileLifetimeAnchor,
    previous_pointer_anchor: Option<RetainedFileLifetimeAnchor>,
    installed_pointer_anchor: RetainedFileLifetimeAnchor,
    materials: Vec<RetainedLifecycleMaterialLeaf>,
    active_pointer: Option<RetainedDomainPackActivePointerWitness>,
}

impl fmt::Debug for RetainedDomainPackLifecycleCompletion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedDomainPackLifecycleCompletion")
            .field("record_digest", &self.record_digest)
            .finish_non_exhaustive()
    }
}

impl RetainedDomainPackLifecycleCompletion {
    /// Borrow the exact active-pointer handle selected by this completion.
    #[must_use]
    pub fn active_pointer_witness(&self) -> Option<&RetainedDomainPackActivePointerWitness> {
        self.active_pointer.as_ref()
    }

    /// Transfer the exact active-pointer handle into the higher-level writer
    /// authority while retaining all selector, record, and material anchors.
    pub fn take_active_pointer_witness(
        &mut self,
    ) -> Option<RetainedDomainPackActivePointerWitness> {
        self.active_pointer.take()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackLifecycleCompletionSelector {
    schema_version: String,
    record_digest: String,
    record_byte_length: u64,
    record_anchor: RetainedFileAnchorBinding,
    parent_anchor: RetainedFileAnchorBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackLifecycleCompletionRecord {
    schema_version: String,
    operation_nonce: String,
    project_id: String,
    project_snapshot_digest: String,
    project_anchors: RetainedProjectAnchorBinding,
    previous_pointer: Option<DomainPackLifecyclePointerBinding>,
    installed_pointer: DomainPackLifecyclePointerBinding,
    generation: DomainPackLifecycleGenerationBinding,
    materials: Vec<DomainPackLifecycleMaterialBinding>,
    committed_receipt: DomainPackLifecycleMaterialBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackLifecyclePointerBinding {
    raw_bytes: Vec<u8>,
    raw_digest: String,
    byte_length: u64,
    anchor: RetainedFileAnchorBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackLifecycleGenerationBinding {
    generation: u64,
    ledger_record_digest: String,
    lock_digest: String,
    preflight_digest: String,
    compatibility_report_digest: String,
    receipt_digest: String,
    object_raw_digests: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackLifecycleMaterialBinding {
    relative_path: String,
    raw_digest: String,
    byte_length: u64,
    anchor: RetainedFileAnchorBinding,
}

struct RetainedLifecycleMaterialLeaf {
    file: File,
    identity: RetainedFileIdentity,
    maximum: u64,
    binding: DomainPackLifecycleMaterialBinding,
    anchor: RetainedFileLifetimeAnchor,
}

struct RetainedLifecycleSelectedLeaf {
    file: File,
    identity: RetainedFileIdentity,
    relative_path: PathBuf,
    raw_digest: String,
    byte_length: u64,
    maximum: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RetainedLifecycleIoError {
    InvalidRelativePath {
        path: String,
    },
    SizeLimit {
        path: PathBuf,
        found: u64,
        maximum: u64,
    },
    Identity {
        path: PathBuf,
        reason: String,
    },
    Io {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for RetainedLifecycleIoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRelativePath { path } => {
                write!(formatter, "invalid retained lifecycle path {path}")
            }
            Self::SizeLimit {
                path,
                found,
                maximum,
            } => write!(
                formatter,
                "retained lifecycle file {} exceeds size limit: {found} > {maximum}",
                path.display()
            ),
            Self::Identity { path, reason } => write!(
                formatter,
                "retained lifecycle identity {} changed: {reason}",
                path.display()
            ),
            Self::Io { path, reason } => write!(
                formatter,
                "retained lifecycle I/O {} failed: {reason}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for RetainedLifecycleIoError {}

impl EffectStoreLock {
    /// Consume this exact lock and seal it into Domain Pack lifecycle I/O for an
    /// embedded state root whose direct parent is the governed project.
    ///
    /// # Errors
    ///
    /// Fails unless this is the designated lifecycle lock beneath the currently
    /// retained producer root and the exact `domain-packs` directory can be
    /// retained without following a link or reparse point.
    pub fn into_domain_pack_lifecycle_store(
        self,
    ) -> Result<RetainedDomainPackLifecycleStore, RetainedLifecycleIoError> {
        self.into_domain_pack_lifecycle_store_inner(None)
    }

    /// Consume this exact lock and bind lifecycle I/O to one explicit retained
    /// project root. This is required when state lives in a detached sidecar.
    ///
    /// # Errors
    ///
    /// Fails if the project binding cannot be retained or this is not the fixed
    /// lifecycle lock beneath the current Store root.
    pub fn into_domain_pack_lifecycle_store_for_project(
        self,
        project_tree: &RetainedProjectTree,
    ) -> Result<RetainedDomainPackLifecycleStore, RetainedLifecycleIoError> {
        let binding = project_tree
            .root_binding()
            .map_err(|error| lifecycle_project_anchor_error(project_tree, error))?;
        self.into_domain_pack_lifecycle_store_inner(Some(binding))
    }

    fn into_domain_pack_lifecycle_store_inner(
        self,
        project_root_binding: Option<RetainedProjectRootBinding>,
    ) -> Result<RetainedDomainPackLifecycleStore, RetainedLifecycleIoError> {
        if self.state_lock_relative_path != Path::new(DOMAIN_PACK_LIFECYCLE_LOCK) {
            return Err(RetainedLifecycleIoError::InvalidRelativePath {
                path: self.state_lock_relative_path.display().to_string(),
            });
        }
        validate_lock_root(&self)?;
        self.state_root
            .create_dir_all(Path::new(DOMAIN_PACK_LIFECYCLE_ROOT))
            .and_then(|()| self.state_root.sync_root())
            .map_err(|error| {
                lifecycle_io_error(&self, Path::new(DOMAIN_PACK_LIFECYCLE_ROOT), error)
            })?;
        let lifecycle_root = self
            .state_root
            .open_directory(Path::new(DOMAIN_PACK_LIFECYCLE_ROOT))
            .map_err(|error| {
                lifecycle_io_error(&self, Path::new(DOMAIN_PACK_LIFECYCLE_ROOT), error)
            })?;
        let state_root_identity = self.state_root.identity().map_err(|error| {
            lifecycle_identity_error(&self, Path::new(""), format!("retain state root: {error}"))
        })?;
        let lifecycle_root_identity = lifecycle_root.identity().map_err(|error| {
            lifecycle_identity_error(
                &self,
                Path::new(DOMAIN_PACK_LIFECYCLE_ROOT),
                format!("retain lifecycle root: {error}"),
            )
        })?;
        let store = RetainedDomainPackLifecycleStore {
            lock: self,
            state_root_identity,
            lifecycle_root,
            lifecycle_root_identity,
            project_root_binding,
        };
        store.validate_current()?;
        Ok(store)
    }
}

impl RetainedDomainPackLifecycleStore {
    /// Revalidate the ambient root name, producer boundary, lock leaf, and exact
    /// retained lifecycle directory without exposing any of those authorities.
    pub fn validate_current(&self) -> Result<(), RetainedLifecycleIoError> {
        validate_lock_root(&self.lock)?;
        let state_identity = self.lock.state_root.identity().map_err(|error| {
            lifecycle_identity_error(
                &self.lock,
                Path::new(""),
                format!("inspect retained state root: {error}"),
            )
        })?;
        if state_identity != self.state_root_identity {
            return Err(lifecycle_identity_error(
                &self.lock,
                Path::new(""),
                "retained state-root handle changed identity".to_owned(),
            ));
        }
        let lifecycle_identity = self.lifecycle_root.identity().map_err(|error| {
            lifecycle_identity_error(
                &self.lock,
                Path::new(DOMAIN_PACK_LIFECYCLE_ROOT),
                format!("inspect retained lifecycle root: {error}"),
            )
        })?;
        if lifecycle_identity != self.lifecycle_root_identity {
            return Err(lifecycle_identity_error(
                &self.lock,
                Path::new(DOMAIN_PACK_LIFECYCLE_ROOT),
                "retained lifecycle-root handle changed identity".to_owned(),
            ));
        }
        let current_lifecycle_identity = self
            .lock
            .state_root
            .open_directory(Path::new(DOMAIN_PACK_LIFECYCLE_ROOT))
            .and_then(|directory| directory.identity())
            .map_err(|error| {
                lifecycle_identity_error(
                    &self.lock,
                    Path::new(DOMAIN_PACK_LIFECYCLE_ROOT),
                    format!("reopen lifecycle root beneath retained state root: {error}"),
                )
            })?;
        if current_lifecycle_identity != self.lifecycle_root_identity {
            return Err(lifecycle_identity_error(
                &self.lock,
                Path::new(DOMAIN_PACK_LIFECYCLE_ROOT),
                "lifecycle namespace no longer names the retained directory".to_owned(),
            ));
        }
        Ok(())
    }

    /// Revalidate one sealed project-tree witness while this exact lifecycle
    /// root and lock remain authoritative.
    ///
    /// The project capability performs only descriptor-relative traversal of its
    /// retained root/full-tree handles. Lifecycle validation on both sides binds
    /// that rehash to this store's retained state-root and lock witness.
    pub fn validate_project_tree(
        &self,
        project_tree: &RetainedProjectTree,
    ) -> Result<(), RetainedLifecycleIoError> {
        self.validate_current()?;
        if let Some(expected) = &self.project_root_binding {
            project_tree
                .validate_root_binding(expected)
                .map_err(|error| lifecycle_project_anchor_error(project_tree, error))?;
        } else {
            let expected_project_root =
                self.lock
                    .state_root
                    .display_path()
                    .parent()
                    .ok_or_else(|| {
                        lifecycle_identity_error(
                            &self.lock,
                            Path::new(""),
                            "retained state root has no project parent".to_owned(),
                        )
                    })?;
            if project_tree.display_root() != expected_project_root {
                return Err(RetainedLifecycleIoError::Identity {
                    path: project_tree.display_root().to_path_buf(),
                    reason:
                        "retained project tree is not the exact parent of the locked state root"
                            .to_owned(),
                });
            }
        }
        project_tree
            .revalidate()
            .map_err(|error| RetainedLifecycleIoError::Identity {
                path: project_tree.display_root().to_path_buf(),
                reason: format!("retained project-tree revalidation failed: {error}"),
            })?;
        self.validate_current()
    }

    /// Retain the exact active-pointer file, bytes, and digest under this lock.
    pub fn retain_active_pointer(
        &self,
        maximum: u64,
    ) -> Result<Option<RetainedDomainPackActivePointerWitness>, RetainedLifecycleIoError> {
        let relative = Path::new("active.lock.yaml");
        self.validate_current()?;
        let mut file = match self
            .lifecycle_root
            .open_leaf_read_delete_rename_authority(relative)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.validate_current()?;
                return Ok(None);
            }
            Err(error) => {
                return Err(lifecycle_io_error(
                    &self.lock,
                    Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                    error,
                ));
            }
        };
        let identity = RetainedDirectory::identity_of(&file).map_err(|error| {
            lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
        })?;
        let bytes = read_retained_lifecycle_leaf(&mut file, maximum).map_err(|error| {
            lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
        })?;
        let witness = RetainedDomainPackActivePointerWitness {
            state_root_identity: self.state_root_identity.clone(),
            lifecycle_root_identity: self.lifecycle_root_identity.clone(),
            lifecycle_lock_identity: self.lock.lock_identity.clone(),
            file,
            identity,
            digest: sha256_content_hash(&bytes),
            bytes,
            maximum,
        };
        self.revalidate_active_pointer(&witness)?;
        Ok(Some(witness))
    }

    /// Retain exact present or Store-minted absence authority for the fixed
    /// active pointer under this lifecycle root and producer lock.
    pub fn retain_expected_active_pointer(
        &self,
        maximum: u64,
    ) -> Result<RetainedDomainPackExpectedActivePointer, RetainedLifecycleIoError> {
        if let Some(witness) = self.retain_active_pointer(maximum)? {
            Ok(RetainedDomainPackExpectedActivePointer::Present(witness))
        } else {
            self.validate_current()?;
            match self.lifecycle_root.open_leaf_read(
                Path::new("active.lock.yaml"),
                crate::retained_dir::RetainedLeafPolicy::Authority,
            ) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Ok(_) => {
                    return Err(lifecycle_identity_error(
                        &self.lock,
                        Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                        "active pointer appeared while absence authority was minted".to_owned(),
                    ));
                }
                Err(error) => {
                    return Err(lifecycle_io_error(
                        &self.lock,
                        Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                        error,
                    ));
                }
            }
            self.validate_current()?;
            Ok(RetainedDomainPackExpectedActivePointer::Absent(
                RetainedDomainPackActivePointerAbsenceWitness {
                    state_root_identity: self.state_root_identity.clone(),
                    lifecycle_root_identity: self.lifecycle_root_identity.clone(),
                    lifecycle_lock_identity: self.lock.lock_identity.clone(),
                    reconciled_binding: None,
                },
            ))
        }
    }

    /// Revalidate an active-pointer witness against this exact lifecycle Store.
    pub fn revalidate_active_pointer(
        &self,
        witness: &RetainedDomainPackActivePointerWitness,
    ) -> Result<(), RetainedLifecycleIoError> {
        let relative = Path::new("active.lock.yaml");
        self.validate_current()?;
        if witness.state_root_identity != self.state_root_identity
            || witness.lifecycle_root_identity != self.lifecycle_root_identity
            || witness.lifecycle_lock_identity != self.lock.lock_identity
            || RetainedDirectory::identity_of(&witness.file).map_err(|error| {
                lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
            })? != witness.identity
        {
            return Err(lifecycle_identity_error(
                &self.lock,
                Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                "active-pointer witness belongs to a different retained leaf or lifecycle root"
                    .to_owned(),
            ));
        }
        self.lifecycle_root
            .verify_retained_authority_binding(relative, &witness.file, &witness.identity)
            .map_err(|error| {
                lifecycle_identity_error(
                    &self.lock,
                    Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                    error.to_string(),
                )
            })?;
        let actual = read_retained_lifecycle_leaf(
            &mut witness.file.try_clone().map_err(|error| {
                lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
            })?,
            witness.maximum,
        )
        .map_err(|error| {
            lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
        })?;
        if actual != witness.bytes || sha256_content_hash(&actual) != witness.digest {
            return Err(lifecycle_identity_error(
                &self.lock,
                Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                "active-pointer witness bytes or digest changed".to_owned(),
            ));
        }
        self.lifecycle_root
            .verify_retained_authority_binding(relative, &witness.file, &witness.identity)
            .map_err(|error| {
                lifecycle_identity_error(
                    &self.lock,
                    Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                    error.to_string(),
                )
            })?;
        self.validate_current()
    }

    fn revalidate_active_pointer_absence(
        &self,
        witness: &RetainedDomainPackActivePointerAbsenceWitness,
    ) -> Result<(), RetainedLifecycleIoError> {
        self.validate_current()?;
        if witness.state_root_identity != self.state_root_identity
            || witness.lifecycle_root_identity != self.lifecycle_root_identity
            || witness.lifecycle_lock_identity != self.lock.lock_identity
        {
            return Err(lifecycle_identity_error(
                &self.lock,
                Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                "active-pointer absence authority belongs to a different root or lock".to_owned(),
            ));
        }
        if let Some(binding) = &witness.reconciled_binding {
            binding
                .revalidate_binding(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER))
                .map_err(|error| {
                    lifecycle_identity_error(
                        &self.lock,
                        Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                        format!("reconciled absence binding changed: {error}"),
                    )
                })?;
            return self.validate_current();
        }
        match self.lifecycle_root.open_leaf_read(
            Path::new("active.lock.yaml"),
            crate::retained_dir::RetainedLeafPolicy::Authority,
        ) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => self.validate_current(),
            Ok(_) => Err(lifecycle_identity_error(
                &self.lock,
                Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                "active pointer appeared after exact absence authority was minted".to_owned(),
            )),
            Err(error) => Err(lifecycle_io_error(
                &self.lock,
                Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                error,
            )),
        }
    }

    /// Revalidate the exact present or absence authority retained by the
    /// higher-level lifecycle guard without reopening or reminting it.
    pub fn revalidate_expected_active_pointer(
        &self,
        expected: &RetainedDomainPackExpectedActivePointer,
    ) -> Result<(), RetainedLifecycleIoError> {
        match expected {
            RetainedDomainPackExpectedActivePointer::Present(witness) => {
                self.revalidate_active_pointer(witness)
            }
            RetainedDomainPackExpectedActivePointer::Absent(witness) => {
                self.revalidate_active_pointer_absence(witness)
            }
        }
    }

    /// Roll back a failed active-pointer completion through exact retained
    /// handles. The authoritative name is first forced to one Store-created
    /// empty placeholder, displacing any installed or substituted occupant as
    /// cleanup debt. An exact previous pointer is then republished by handle; if
    /// that is unavailable or fails, the same exact placeholder remains bound.
    pub fn rollback_active_pointer(
        &self,
        installed: &RetainedDomainPackActivePointerWitness,
        previous: Option<&RetainedDomainPackActivePointerWitness>,
    ) -> Result<Vec<PathBuf>, RetainedLifecycleIoError> {
        let target = Path::new("active.lock.yaml");
        let _installed_validation = self.validate_active_pointer_handle(installed);
        let placeholder_path =
            lifecycle_quarantine_path("rollback-placeholder").map_err(|error| {
                lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
            })?;
        let placeholder_file = self
            .lifecycle_root
            .open_leaf_write_new_authority(&placeholder_path)
            .map_err(|error| {
                lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
            })?;
        placeholder_file.sync_all().map_err(|error| {
            lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
        })?;
        let placeholder_identity =
            RetainedDirectory::identity_of(&placeholder_file).map_err(|error| {
                lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
            })?;
        validate_lifecycle_placeholder(&placeholder_file, &placeholder_identity).map_err(
            |error| lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error),
        )?;
        self.lifecycle_root.sync_root().map_err(|error| {
            lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
        })?;
        let authority = self.lifecycle_root.retain_authority().map_err(|error| {
            lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
        })?;
        let initial_placeholder = match force_lifecycle_placeholder(
            &authority,
            &self.lifecycle_root,
            &placeholder_file,
            &placeholder_identity,
            target,
        ) {
            Ok(debt) => debt,
            Err(error) => {
                let reassertion = force_lifecycle_placeholder(
                    &authority,
                    &self.lifecycle_root,
                    &placeholder_file,
                    &placeholder_identity,
                    target,
                );
                return Err(lifecycle_io_error(
                    &self.lock,
                    Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                    io::Error::other(
                        format!(
                            "initial exact Store placeholder installation failed ({error}); final placeholder reassertion result: {reassertion:?}"
                        ),
                    ),
                ));
            }
        };
        let mut debt = initial_placeholder.into_paths();
        #[cfg(unix)]
        debt.push(placeholder_path.clone());

        let Some(previous) = previous else {
            debt.extend(
                force_lifecycle_placeholder(
                    &authority,
                    &self.lifecycle_root,
                    &placeholder_file,
                    &placeholder_identity,
                    target,
                )
                .map_err(|error| {
                    lifecycle_io_error(
                        &self.lock,
                        Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                        io::Error::other(format!(
                            "final absent-pointer rollback placeholder reassertion failed: {error}"
                        )),
                    )
                })?
                .into_paths(),
            );
            self.validate_current()?;
            self.lifecycle_root
                .verify_retained_authority_binding(target, &placeholder_file, &placeholder_identity)
                .and_then(|()| {
                    validate_lifecycle_placeholder(&placeholder_file, &placeholder_identity)
                })
                .map_err(|error| {
                    lifecycle_identity_error(
                        &self.lock,
                        Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                        error.to_string(),
                    )
                })?;
            return Ok(self.lifecycle_cleanup_paths(debt));
        };
        if let Err(previous_error) = self.validate_active_pointer_handle(previous) {
            let reassertion = force_lifecycle_placeholder(
                &authority,
                &self.lifecycle_root,
                &placeholder_file,
                &placeholder_identity,
                target,
            );
            if let Err(reassertion_error) = reassertion {
                return Err(lifecycle_io_error(
                    &self.lock,
                    Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                    io::Error::other(
                        format!(
                            "exact previous active-pointer witness is unavailable ({previous_error}); exact Store placeholder reassertion and sync failed: {reassertion_error}"
                        ),
                    ),
                ));
            }
            return Err(RetainedLifecycleIoError::Identity {
                path: self.display_path(Path::new(DOMAIN_PACK_ACTIVE_POINTER)),
                reason: format!(
                    "exact previous active-pointer witness is unavailable ({previous_error}); the exact retained Store placeholder was reasserted and synced"
                ),
            });
        }
        let parked_placeholder =
            lifecycle_quarantine_path("rollback-parked-placeholder").map_err(|error| {
                lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
            })?;
        let parked_debt = match authority.rename_file_noreplace_with_validation(
            target,
            &parked_placeholder,
            |directory, source, _| {
                directory.verify_retained_authority_binding(
                    source,
                    &placeholder_file,
                    &placeholder_identity,
                )?;
                validate_lifecycle_placeholder(&placeholder_file, &placeholder_identity)?;
                validate_retained_lifecycle_handle(previous)
            },
        ) {
            Ok(debt) => debt,
            Err(error) => {
                let placeholder_result = force_lifecycle_placeholder(
                    &authority,
                    &self.lifecycle_root,
                    &placeholder_file,
                    &placeholder_identity,
                    target,
                );
                return Err(lifecycle_io_error(
                    &self.lock,
                    Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                    io::Error::other(
                        format!(
                            "parking the exact Store placeholder failed ({error}); placeholder reassertion and sync result: {placeholder_result:?}"
                        ),
                    ),
                ));
            }
        };
        debt.extend(parked_debt.into_paths());
        debt.push(parked_placeholder.clone());
        if let Err(previous_error) =
            authority.publish_retained_handle_noreplace(&previous.file, &previous.identity, target)
        {
            let placeholder_result = force_lifecycle_placeholder(
                &authority,
                &self.lifecycle_root,
                &placeholder_file,
                &placeholder_identity,
                target,
            );
            return Err(lifecycle_io_error(
                &self.lock,
                Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                io::Error::other(
                    format!(
                        "exact previous active-pointer restoration failed ({previous_error}); Store placeholder restoration result: {placeholder_result:?}"
                    ),
                ),
            ));
        }
        self.lifecycle_root
            .verify_retained_authority_binding(target, &previous.file, &previous.identity)
            .and_then(|()| validate_retained_lifecycle_handle(previous))
            .and_then(|()| self.lifecycle_root.sync_root())
            .map_err(|error| {
                let placeholder_result = force_lifecycle_placeholder(
                    &authority,
                    &self.lifecycle_root,
                    &placeholder_file,
                    &placeholder_identity,
                    target,
                );
                lifecycle_io_error(
                    &self.lock,
                    Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                    io::Error::other(
                        format!(
                            "restored previous active pointer failed final validation ({error}); Store placeholder restoration result: {placeholder_result:?}"
                        ),
                    ),
                )
            })?;
        if let Err(error) = self.validate_current() {
            let placeholder_result = force_lifecycle_placeholder(
                &authority,
                &self.lifecycle_root,
                &placeholder_file,
                &placeholder_identity,
                target,
            );
            return Err(RetainedLifecycleIoError::Identity {
                path: self.display_path(Path::new(DOMAIN_PACK_ACTIVE_POINTER)),
                reason: format!(
                    "lifecycle authority changed after previous-pointer restoration ({error}); Store placeholder restoration result: {placeholder_result:?}"
                ),
            });
        }
        if let Err(error) = self
            .lifecycle_root
            .verify_retained_authority_binding(target, &previous.file, &previous.identity)
            .and_then(|()| validate_retained_lifecycle_handle(previous))
        {
            let placeholder_result = force_lifecycle_placeholder(
                &authority,
                &self.lifecycle_root,
                &placeholder_file,
                &placeholder_identity,
                target,
            );
            return Err(RetainedLifecycleIoError::Identity {
                path: self.display_path(Path::new(DOMAIN_PACK_ACTIVE_POINTER)),
                reason: format!(
                    "previous active pointer changed at rollback linearization ({error}); Store placeholder restoration result: {placeholder_result:?}"
                ),
            });
        }
        Ok(self.lifecycle_cleanup_paths(debt))
    }

    /// Diagnostic-only display path. Lifecycle I/O never resolves through it.
    #[must_use]
    pub fn display_path(&self, relative: &Path) -> PathBuf {
        self.lock.state_root.display_path().join(relative)
    }

    /// Read one bounded immutable lifecycle leaf through the retained lifecycle
    /// directory. Missing leaves return `None`.
    pub fn read_optional(
        &self,
        relative: &Path,
        maximum: u64,
    ) -> Result<Option<Vec<u8>>, RetainedLifecycleIoError> {
        let lifecycle_relative = self.lifecycle_relative(relative)?;
        self.validate_current()?;
        let result = match self
            .lifecycle_root
            .read_authority_bounded(lifecycle_relative, maximum)
        {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(lifecycle_io_error(&self.lock, relative, error)),
        };
        self.validate_current()?;
        result
    }

    /// Read one required bounded immutable lifecycle leaf.
    pub fn read_required(
        &self,
        relative: &Path,
        maximum: u64,
    ) -> Result<Vec<u8>, RetainedLifecycleIoError> {
        self.read_optional(relative, maximum)?
            .ok_or_else(|| RetainedLifecycleIoError::Io {
                path: self.display_path(relative),
                reason: "required file is missing".to_owned(),
            })
    }

    /// Create one immutable lifecycle leaf and durably sync its exact parent.
    /// Existing content is accepted only when its bytes are identical, keeping
    /// the idempotence decision inside the same retained Store capability.
    pub fn write_immutable(
        &self,
        relative: &Path,
        content: &[u8],
        maximum: u64,
    ) -> Result<(), RetainedLifecycleIoError> {
        let lifecycle_relative = self.lifecycle_relative(relative)?;
        let found = u64::try_from(content.len()).unwrap_or(u64::MAX);
        if found > maximum {
            return Err(RetainedLifecycleIoError::SizeLimit {
                path: self.display_path(relative),
                found,
                maximum,
            });
        }
        self.validate_current()?;
        self.sync_parent_chain(lifecycle_relative)
            .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
        self.validate_current()?;
        let authority = self
            .lifecycle_root
            .retain_authority()
            .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
        let result = match authority.write_new_file_synced(lifecycle_relative, content) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let existing = self
                    .lifecycle_root
                    .read_authority_bounded(lifecycle_relative, maximum)
                    .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
                if existing == content {
                    Ok(())
                } else {
                    Err(RetainedLifecycleIoError::Io {
                        path: self.display_path(relative),
                        reason: "content-addressed collision with different bytes".to_owned(),
                    })
                }
            }
            Err(error) => Err(lifecycle_io_error(&self.lock, relative, error)),
        };
        self.validate_current()?;
        result
    }

    /// Report whether one fixed lifecycle directory currently exists beneath the
    /// retained lifecycle root. Files, links, and special objects fail closed.
    pub fn directory_exists(&self, relative: &Path) -> Result<bool, RetainedLifecycleIoError> {
        let lifecycle_relative = self.lifecycle_relative(relative)?;
        self.validate_current()?;
        let result = match self.lifecycle_root.open_directory(lifecycle_relative) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(lifecycle_io_error(&self.lock, relative, error)),
        };
        self.validate_current()?;
        result
    }

    /// Reconcile the fixed active pointer and return the exact one-shot present
    /// or absence session produced by marker finalization.
    ///
    /// State loading must inspect this same opaque session and consume it after
    /// completion-selector validation. It must not reopen the pathname later to
    /// remint expected-state authority.
    pub fn reconcile_active_pointer(
        &self,
        maximum: u64,
    ) -> Result<RetainedCrashReplaceSession<'_>, CrashReplaceError> {
        self.validate_current()
            .map_err(|error| self.crash_error(error))?;
        let directory = self
            .lifecycle_root
            .try_clone()
            .map_err(|error| self.crash_io_error(Path::new(DOMAIN_PACK_ACTIVE_POINTER), error))?;
        let target = RetainedCrashReplaceTarget::new(
            &self.lock,
            directory,
            PathBuf::from(DOMAIN_PACK_ACTIVE_POINTER),
        )
        .map_err(|error| self.crash_io_error(Path::new(DOMAIN_PACK_ACTIVE_POINTER), error))?;
        reconcile_file_crash_safe_at_owned_retained_target(target, maximum)
    }

    /// Consume the exact active-pointer reconciliation session into long-lived
    /// expected-state authority for this same lifecycle Store.
    ///
    /// Present authority keeps marker finalization's exact retained handle.
    /// Absent authority is transferred from the session's reconciled absence;
    /// this method performs no second lookup of `active.lock.yaml`.
    ///
    /// # Errors
    ///
    /// Returns [`CrashReplaceError`] if the session belongs to another retained
    /// lock, root, parent, or leaf, or its exact present handle changed.
    pub fn consume_reconciled_active_pointer(
        &self,
        session: RetainedCrashReplaceSession<'_>,
    ) -> Result<RetainedDomainPackExpectedActivePointer, CrashReplaceError> {
        let leaf =
            session.consume_reconciled_leaf(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER))?;
        match leaf {
            ConsumedRetainedCrashReplaceLeaf::Present {
                file,
                identity,
                bytes,
                digest,
                maximum,
            } => Ok(RetainedDomainPackExpectedActivePointer::Present(
                RetainedDomainPackActivePointerWitness {
                    state_root_identity: self.state_root_identity.clone(),
                    lifecycle_root_identity: self.lifecycle_root_identity.clone(),
                    lifecycle_lock_identity: self.lock.lock_identity.clone(),
                    file,
                    identity,
                    bytes,
                    digest,
                    maximum,
                },
            )),
            ConsumedRetainedCrashReplaceLeaf::Absent(reconciled_binding) => {
                Ok(RetainedDomainPackExpectedActivePointer::Absent(
                    RetainedDomainPackActivePointerAbsenceWitness {
                        state_root_identity: self.state_root_identity.clone(),
                        lifecycle_root_identity: self.lifecycle_root_identity.clone(),
                        lifecycle_lock_identity: self.lock.lock_identity.clone(),
                        reconciled_binding: Some(reconciled_binding),
                    },
                ))
            }
        }
    }

    /// Compare-and-swap the fixed active pointer through its exact retained
    /// parent after all immutable generation material has been validated.
    ///
    /// The prior pointer must be supplied as the exact witness returned by this
    /// Store. Success returns the exact installed handle and keeps it available
    /// for the caller's joint project/pointer completion linearization.
    pub fn replace_active_pointer(
        &self,
        expected: &RetainedDomainPackExpectedActivePointer,
        content: &[u8],
        maximum: u64,
    ) -> Result<RetainedDomainPackActivePointerWitness, CrashReplaceError> {
        self.validate_current()
            .map_err(|error| self.crash_error(error))?;
        let (expected_digest, expected_target) = match expected {
            RetainedDomainPackExpectedActivePointer::Present(previous) => {
                self.revalidate_active_pointer(previous)
                    .map_err(|error| self.crash_error(error))?;
                (
                    Some(previous.digest.clone()),
                    RetainedExpectedTarget::Exact {
                        file: &previous.file,
                        identity: &previous.identity,
                    },
                )
            }
            RetainedDomainPackExpectedActivePointer::Absent(absence) => {
                self.revalidate_active_pointer_absence(absence)
                    .map_err(|error| self.crash_error(error))?;
                (
                    None,
                    absence
                        .reconciled_binding
                        .as_ref()
                        .map_or(RetainedExpectedTarget::Absent, |binding| {
                            binding.expected_target()
                        }),
                )
            }
        };
        let directory = self
            .lifecycle_root
            .try_clone()
            .map_err(|error| self.crash_io_error(Path::new(DOMAIN_PACK_ACTIVE_POINTER), error))?;
        let target = RetainedCrashReplaceTarget::new(
            &self.lock,
            directory,
            PathBuf::from(DOMAIN_PACK_ACTIVE_POINTER),
        )
        .map_err(|error| self.crash_io_error(Path::new(DOMAIN_PACK_ACTIVE_POINTER), error))?;
        let retained = replace_file_crash_safe_at_retained_target_with_witness(
            &target,
            expected_digest.as_deref(),
            expected_target,
            content,
            maximum,
        )?;
        let installed = RetainedDomainPackActivePointerWitness {
            state_root_identity: self.state_root_identity.clone(),
            lifecycle_root_identity: self.lifecycle_root_identity.clone(),
            lifecycle_lock_identity: self.lock.lock_identity.clone(),
            file: retained.installed_file,
            identity: retained.installed_identity,
            bytes: content.to_vec(),
            digest: retained.result.installed_digest,
            maximum,
        };
        if installed.digest != sha256_content_hash(content) {
            return Err(CrashReplaceError::Protocol {
                reason: "retained active-pointer replacement returned a mismatched digest"
                    .to_owned(),
            });
        }
        // Marker finalization returned the exact installed handle. Do not reopen
        // the target pathname after that closing sweep; outer lifecycle completion
        // validation will consume this retained witness before selector publication.
        Ok(installed)
    }

    /// Publish one immutable completion record and independently atomically
    /// select it after every record, pointer, project, and material binding has
    /// been retained. The record never declares its own expected identity. The
    /// selector binds the staged record's exact content digest and generation-safe
    /// Store anchors for both the record leaf and its generation parent.
    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    pub fn publish_lifecycle_completion(
        &self,
        project_tree: &RetainedProjectTree,
        previous_pointer: Option<&RetainedDomainPackActivePointerWitness>,
        installed_pointer: &RetainedDomainPackActivePointerWitness,
        input: DomainPackLifecycleCompletionInput<'_>,
    ) -> Result<RetainedDomainPackLifecycleCompletion, RetainedLifecycleIoError> {
        validate_completion_input(&input).map_err(|reason| {
            lifecycle_identity_error(&self.lock, Path::new(DOMAIN_PACK_LIFECYCLE_ROOT), reason)
        })?;
        self.validate_project_tree(project_tree)?;
        if project_tree.snapshot_digest() != input.project_snapshot_digest {
            return Err(RetainedLifecycleIoError::Identity {
                path: project_tree.display_root().to_path_buf(),
                reason: "retained project snapshot differs from lifecycle request".to_owned(),
            });
        }
        self.revalidate_active_pointer(installed_pointer)?;
        if let Some(previous) = previous_pointer {
            self.validate_active_pointer_handle(previous)?;
        }

        let material_leaves = self.retain_completion_materials(&input)?;
        let committed_receipt_path = completion_committed_receipt_path(&input)?;
        let committed_receipt = material_leaves
            .iter()
            .find(|leaf| {
                leaf.binding.relative_path == record_relative_path(&committed_receipt_path)
            })
            .map(|leaf| leaf.binding.clone())
            .ok_or_else(|| {
                lifecycle_identity_error(
                    &self.lock,
                    &committed_receipt_path,
                    "completion material inventory omitted the committed receipt".to_owned(),
                )
            })?;
        let generation_receipt_path = completion_generation_root(&input)?.join("receipt.yaml");
        let generation_receipt = material_leaves
            .iter()
            .find(|leaf| {
                leaf.binding.relative_path == record_relative_path(&generation_receipt_path)
            })
            .map(|leaf| &leaf.binding)
            .ok_or_else(|| {
                lifecycle_identity_error(
                    &self.lock,
                    &generation_receipt_path,
                    "completion material inventory omitted the generation receipt".to_owned(),
                )
            })?;
        if generation_receipt.raw_digest != committed_receipt.raw_digest
            || generation_receipt.byte_length != committed_receipt.byte_length
        {
            return Err(lifecycle_identity_error(
                &self.lock,
                &committed_receipt_path,
                "committed receipt differs from the exact immutable generation receipt".to_owned(),
            ));
        }
        let generation_manifest_path = completion_generation_root(&input)?.join("generation.yaml");
        let generation_manifest = material_leaves
            .iter()
            .find(|leaf| {
                leaf.binding.relative_path == record_relative_path(&generation_manifest_path)
            })
            .ok_or_else(|| {
                lifecycle_identity_error(
                    &self.lock,
                    &generation_manifest_path,
                    "completion material inventory omitted the generation manifest".to_owned(),
                )
            })?;
        let parent_anchor_binding = generation_manifest.binding.anchor.clone();
        let record_state_path = completion_record_path(&input)?;
        let project_anchor_directory = Path::new(DOMAIN_PACK_LIFECYCLE_ROOT)
            .join(lifecycle_anchor_directory(&record_state_path))
            .join("project");
        let project_anchors = self
            .lock
            .retain_project_tree_anchors(project_tree, &project_anchor_directory)
            .map_err(|error| lifecycle_project_anchor_error(project_tree, error))?;

        let (previous_pointer_binding, previous_pointer_anchor) = match previous_pointer {
            Some(pointer) => {
                let (binding, anchor) = self.anchor_pointer(pointer, "previous-pointer")?;
                (Some(binding), Some(anchor))
            }
            None => (None, None),
        };
        let (installed_pointer_binding, installed_pointer_anchor) =
            self.anchor_pointer(installed_pointer, "installed-pointer")?;
        let operation_nonce = lifecycle_operation_nonce().map_err(|error| {
            lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_LIFECYCLE_ROOT), error)
        })?;
        let record = DomainPackLifecycleCompletionRecord {
            schema_version: DOMAIN_PACK_COMPLETION_SCHEMA_VERSION.to_owned(),
            operation_nonce,
            project_id: input.project_id.to_owned(),
            project_snapshot_digest: input.project_snapshot_digest.to_owned(),
            project_anchors: project_anchors.binding().clone(),
            previous_pointer: previous_pointer_binding,
            installed_pointer: installed_pointer_binding,
            generation: completion_generation_binding(&input),
            materials: material_leaves
                .iter()
                .map(|leaf| leaf.binding.clone())
                .collect(),
            committed_receipt,
        };
        let record_bytes = canonical_completion_bytes(&record)
            .map_err(|error| lifecycle_io_error(&self.lock, &record_state_path, error))?;
        let record_length = u64::try_from(record_bytes.len()).unwrap_or(u64::MAX);
        if record_length > DOMAIN_PACK_COMPLETION_MAX_BYTES {
            return Err(RetainedLifecycleIoError::SizeLimit {
                path: self.display_path(&record_state_path),
                found: record_length,
                maximum: DOMAIN_PACK_COMPLETION_MAX_BYTES,
            });
        }
        let record_digest = sha256_content_hash(&record_bytes);
        let record_path = self.lifecycle_relative(&record_state_path)?.to_path_buf();
        let record_staging_path = lifecycle_quarantine_path("completion-record")
            .map_err(|error| lifecycle_io_error(&self.lock, &record_state_path, error))?;
        let mut record_file = self
            .lifecycle_root
            .open_leaf_write_new_authority(&record_staging_path)
            .map_err(|error| lifecycle_io_error(&self.lock, &record_state_path, error))?;
        let record_identity = RetainedDirectory::identity_of(&record_file)
            .map_err(|error| lifecycle_io_error(&self.lock, &record_state_path, error))?;
        record_file
            .write_all(&record_bytes)
            .and_then(|()| record_file.sync_all())
            .map_err(|error| lifecycle_io_error(&self.lock, &record_state_path, error))?;
        validate_retained_bytes(
            &record_file,
            &record_identity,
            &record_bytes,
            DOMAIN_PACK_COMPLETION_MAX_BYTES,
        )
        .and_then(|()| {
            self.lifecycle_root.verify_retained_authority_binding(
                &record_staging_path,
                &record_file,
                &record_identity,
            )
        })
        .and_then(|()| self.lifecycle_root.sync_root())
        .map_err(|error| lifecycle_io_error(&self.lock, &record_state_path, error))?;
        let authority = self
            .lifecycle_root
            .retain_authority()
            .map_err(|error| lifecycle_io_error(&self.lock, &record_state_path, error))?;
        let _record_cleanup_debt = authority
            .rename_file_noreplace_with_validation(
                &record_staging_path,
                &record_path,
                |directory, source, destination| {
                    if destination != record_path {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "lifecycle completion record destination changed",
                        ));
                    }
                    directory.verify_retained_authority_binding(
                        source,
                        &record_file,
                        &record_identity,
                    )?;
                    validate_retained_bytes(
                        &record_file,
                        &record_identity,
                        &record_bytes,
                        DOMAIN_PACK_COMPLETION_MAX_BYTES,
                    )
                },
            )
            .map_err(|error| lifecycle_io_error(&self.lock, &record_state_path, error))?;
        let record_anchor = self
            .lifecycle_root
            .retain_file_lifetime_anchor(
                &lifecycle_anchor_directory(&record_state_path),
                &record_file,
                &record_identity,
                &record_digest,
                record_length,
            )
            .map_err(|error| lifecycle_io_error(&self.lock, &record_state_path, error))?;

        let selector = DomainPackLifecycleCompletionSelector {
            schema_version: DOMAIN_PACK_COMPLETION_SELECTOR_SCHEMA_VERSION.to_owned(),
            record_digest: record_digest.clone(),
            record_byte_length: record_length,
            record_anchor: record_anchor.binding().clone(),
            parent_anchor: parent_anchor_binding,
        };
        let selector_state_path = completion_selector_path(&input)?;
        let selector_bytes = canonical_completion_selector_bytes(&selector)
            .map_err(|error| lifecycle_io_error(&self.lock, &selector_state_path, error))?;
        let selector_length = u64::try_from(selector_bytes.len()).unwrap_or(u64::MAX);
        if selector_length > DOMAIN_PACK_COMPLETION_MAX_BYTES {
            return Err(RetainedLifecycleIoError::SizeLimit {
                path: self.display_path(&selector_state_path),
                found: selector_length,
                maximum: DOMAIN_PACK_COMPLETION_MAX_BYTES,
            });
        }
        let selector_path = self.lifecycle_relative(&selector_state_path)?.to_path_buf();
        let selector_staging_path = lifecycle_quarantine_path("completion-selector")
            .map_err(|error| lifecycle_io_error(&self.lock, &selector_state_path, error))?;
        let mut selector_file = self
            .lifecycle_root
            .open_leaf_write_new_authority(&selector_staging_path)
            .map_err(|error| lifecycle_io_error(&self.lock, &selector_state_path, error))?;
        let selector_identity = RetainedDirectory::identity_of(&selector_file)
            .map_err(|error| lifecycle_io_error(&self.lock, &selector_state_path, error))?;
        selector_file
            .write_all(&selector_bytes)
            .and_then(|()| selector_file.sync_all())
            .map_err(|error| lifecycle_io_error(&self.lock, &selector_state_path, error))?;
        validate_retained_bytes(
            &selector_file,
            &selector_identity,
            &selector_bytes,
            DOMAIN_PACK_COMPLETION_MAX_BYTES,
        )
        .and_then(|()| {
            self.lifecycle_root.verify_retained_authority_binding(
                &selector_staging_path,
                &selector_file,
                &selector_identity,
            )
        })
        .and_then(|()| self.lifecycle_root.sync_root())
        .map_err(|error| lifecycle_io_error(&self.lock, &selector_state_path, error))?;

        let parent_anchor = self
            .lifecycle_root
            .open_file_lifetime_anchor(&selector.parent_anchor)
            .map_err(|error| lifecycle_io_error(&self.lock, &selector_state_path, error))?;
        let _selector_cleanup_debt = authority
            .rename_file_noreplace_with_validation(
                &selector_staging_path,
                &selector_path,
                |directory, source, destination| {
                    if destination != selector_path {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "lifecycle completion selector destination changed",
                        ));
                    }
                    self.validate_current()
                        .map_err(completion_validation_error)?;
                    self.revalidate_active_pointer(installed_pointer)
                        .map_err(completion_validation_error)?;
                    if let Some(previous) = previous_pointer {
                        self.validate_active_pointer_handle(previous)
                            .map_err(completion_validation_error)?;
                    }
                    self.validate_project_tree(project_tree)
                        .map_err(completion_validation_error)?;
                    project_anchors
                        .revalidate()
                        .map_err(completion_project_validation_error)?;
                    if project_tree.snapshot_digest() != input.project_snapshot_digest {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "retained project snapshot changed before completion selection",
                        ));
                    }
                    for leaf in &material_leaves {
                        self.revalidate_completion_leaf(leaf)
                            .map_err(completion_validation_error)?;
                    }
                    record_anchor.validate_retained_file(&record_file, &record_identity)?;
                    self.lifecycle_root.verify_retained_authority_binding(
                        &record_path,
                        &record_file,
                        &record_identity,
                    )?;
                    parent_anchor.revalidate()?;
                    directory.verify_retained_authority_binding(
                        source,
                        &selector_file,
                        &selector_identity,
                    )?;
                    validate_retained_bytes(
                        &selector_file,
                        &selector_identity,
                        &selector_bytes,
                        DOMAIN_PACK_COMPLETION_MAX_BYTES,
                    )
                },
            )
            .map_err(|error| lifecycle_io_error(&self.lock, &selector_state_path, error))?;

        Ok(RetainedDomainPackLifecycleCompletion {
            record_digest,
            selector: RetainedLifecycleSelectedLeaf {
                file: selector_file,
                identity: selector_identity,
                relative_path: selector_state_path,
                raw_digest: sha256_content_hash(&selector_bytes),
                byte_length: selector_length,
                maximum: DOMAIN_PACK_COMPLETION_MAX_BYTES,
            },
            record_file,
            record_identity,
            record_path: record_state_path,
            record_anchor,
            project_anchors,
            parent_anchor,
            previous_pointer_anchor,
            installed_pointer_anchor,
            materials: material_leaves,
            active_pointer: None,
        })
    }

    /// Validate the independently committed completion selector using the exact
    /// active-pointer reconciliation session that supplied state loading. The
    /// selector must still name one exact immutable record whose Store anchors
    /// retain the record leaf, generation parent, active pointer, and every
    /// completion material.
    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    pub fn validate_selected_lifecycle_completion(
        &self,
        project_tree: &RetainedProjectTree,
        active_pointer: &RetainedCrashReplaceSession<'_>,
        input: DomainPackLifecycleCompletionInput<'_>,
    ) -> Result<RetainedDomainPackLifecycleCompletion, RetainedLifecycleIoError> {
        validate_completion_input(&input).map_err(|reason| {
            lifecycle_identity_error(&self.lock, Path::new(DOMAIN_PACK_LIFECYCLE_ROOT), reason)
        })?;
        self.validate_project_tree(project_tree)?;
        if project_tree.snapshot_digest() != input.project_snapshot_digest {
            return Err(RetainedLifecycleIoError::Identity {
                path: project_tree.display_root().to_path_buf(),
                reason: "current project snapshot differs from selected completion".to_owned(),
            });
        }
        let active_bytes = active_pointer.raw_bytes().ok_or_else(|| {
            lifecycle_identity_error(
                &self.lock,
                Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                "selected completion has no reconciled active pointer".to_owned(),
            )
        })?;

        let selector_path = completion_selector_path(&input)?;
        let selector_leaf =
            self.retain_selected_leaf(&selector_path, DOMAIN_PACK_COMPLETION_MAX_BYTES)?;
        let selector_bytes = read_selected_leaf_exact(&selector_leaf)?;
        let selector: DomainPackLifecycleCompletionSelector =
            serde_json::from_slice(&selector_bytes).map_err(|error| {
                lifecycle_identity_error(
                    &self.lock,
                    &selector_path,
                    format!("parse canonical lifecycle completion selector: {error}"),
                )
            })?;
        if selector.schema_version != DOMAIN_PACK_COMPLETION_SELECTOR_SCHEMA_VERSION
            || canonical_completion_selector_bytes(&selector)
                .map_err(|error| lifecycle_io_error(&self.lock, &selector_path, error))?
                != selector_bytes
        {
            return Err(lifecycle_identity_error(
                &self.lock,
                &selector_path,
                "completion selector is non-canonical or has an unsupported schema".to_owned(),
            ));
        }

        let record_path = completion_record_path(&input)?;
        let record_relative = self.lifecycle_relative(&record_path)?;
        let record_anchor = self
            .lifecycle_root
            .open_file_lifetime_anchor(&selector.record_anchor)
            .map_err(|error| lifecycle_io_error(&self.lock, &record_path, error))?;
        let (record_file, record_identity) = record_anchor
            .retain_target(&self.lifecycle_root, record_relative)
            .map_err(|error| lifecycle_io_error(&self.lock, &record_path, error))?;
        let record_bytes = read_retained_lifecycle_leaf(
            &mut record_file
                .try_clone()
                .map_err(|error| lifecycle_io_error(&self.lock, &record_path, error))?,
            DOMAIN_PACK_COMPLETION_MAX_BYTES,
        )
        .map_err(|error| lifecycle_io_error(&self.lock, &record_path, error))?;
        if sha256_content_hash(&record_bytes) != selector.record_digest
            || u64::try_from(record_bytes.len()).unwrap_or(u64::MAX) != selector.record_byte_length
            || selector.record_anchor.content_digest != selector.record_digest
            || selector.record_anchor.byte_length != selector.record_byte_length
        {
            return Err(lifecycle_identity_error(
                &self.lock,
                &record_path,
                "completion selector differs from its exact anchored record".to_owned(),
            ));
        }
        let record: DomainPackLifecycleCompletionRecord = serde_json::from_slice(&record_bytes)
            .map_err(|error| {
                lifecycle_identity_error(
                    &self.lock,
                    &record_path,
                    format!("parse canonical lifecycle completion record: {error}"),
                )
            })?;
        if record.schema_version != DOMAIN_PACK_COMPLETION_SCHEMA_VERSION
            || !valid_operation_nonce(&record.operation_nonce)
            || canonical_completion_bytes(&record)
                .map_err(|error| lifecycle_io_error(&self.lock, &record_path, error))?
                != record_bytes
            || record.project_id != input.project_id
            || record.project_snapshot_digest != input.project_snapshot_digest
            || record.generation != completion_generation_binding(&input)
        {
            return Err(lifecycle_identity_error(
                &self.lock,
                &record_path,
                "completion record canonical content differs from the selected lifecycle request"
                    .to_owned(),
            ));
        }
        let project_anchors = self
            .lock
            .open_project_tree_anchors(project_tree, &record.project_anchors)
            .map_err(|error| lifecycle_project_anchor_error(project_tree, error))?;

        validate_pointer_anchor_binding(&record.installed_pointer).map_err(|error| {
            lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
        })?;
        if record.installed_pointer.raw_bytes != active_bytes
            || record.installed_pointer.raw_digest != sha256_content_hash(active_bytes)
        {
            return Err(lifecycle_identity_error(
                &self.lock,
                Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                "reconciled active pointer differs from the selected completion".to_owned(),
            ));
        }
        let previous_pointer_anchor = if let Some(previous) = &record.previous_pointer {
            validate_pointer_anchor_binding(previous)
                .map_err(|error| lifecycle_io_error(&self.lock, &record_path, error))?;
            let anchor = self
                .lifecycle_root
                .open_file_lifetime_anchor(&previous.anchor)
                .map_err(|error| lifecycle_io_error(&self.lock, &record_path, error))?;
            anchor
                .revalidate()
                .map_err(|error| lifecycle_io_error(&self.lock, &record_path, error))?;
            Some(anchor)
        } else {
            None
        };
        let installed_anchor = self
            .lifecycle_root
            .open_file_lifetime_anchor(&record.installed_pointer.anchor)
            .map_err(|error| {
                lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
            })?;
        let (installed_file, installed_identity) = installed_anchor
            .retain_target(&self.lifecycle_root, Path::new("active.lock.yaml"))
            .map_err(|error| {
                lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
            })?;
        validate_retained_bytes(
            &installed_file,
            &installed_identity,
            &record.installed_pointer.raw_bytes,
            DOMAIN_PACK_COMPLETION_MAX_BYTES,
        )
        .map_err(|error| {
            lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
        })?;
        let installed_pointer = RetainedDomainPackActivePointerWitness {
            state_root_identity: self.state_root_identity.clone(),
            lifecycle_root_identity: self.lifecycle_root_identity.clone(),
            lifecycle_lock_identity: self.lock.lock_identity.clone(),
            file: installed_file,
            identity: installed_identity,
            bytes: record.installed_pointer.raw_bytes.clone(),
            digest: record.installed_pointer.raw_digest.clone(),
            maximum: record.installed_pointer.byte_length,
        };
        self.revalidate_active_pointer(&installed_pointer)?;

        let material_leaves = self.retain_bound_completion_materials(&record.materials)?;
        let mut expected_paths = completion_material_paths(&input)?
            .into_iter()
            .map(|path| record_relative_path(&path))
            .collect::<Vec<_>>();
        expected_paths.sort();
        expected_paths.dedup();
        let current_paths = material_leaves
            .iter()
            .map(|leaf| leaf.binding.relative_path.clone())
            .collect::<Vec<_>>();
        if current_paths != expected_paths {
            return Err(lifecycle_identity_error(
                &self.lock,
                &record_path,
                "selected completion material path set differs from lifecycle request".to_owned(),
            ));
        }
        let committed_receipt_path = completion_committed_receipt_path(&input)?;
        let committed_receipt = record
            .materials
            .iter()
            .find(|binding| binding.relative_path == record_relative_path(&committed_receipt_path))
            .cloned()
            .ok_or_else(|| {
                lifecycle_identity_error(
                    &self.lock,
                    &committed_receipt_path,
                    "selected completion omitted the committed receipt".to_owned(),
                )
            })?;
        let generation_receipt_path = completion_generation_root(&input)?.join("receipt.yaml");
        let generation_receipt = record
            .materials
            .iter()
            .find(|binding| binding.relative_path == record_relative_path(&generation_receipt_path))
            .ok_or_else(|| {
                lifecycle_identity_error(
                    &self.lock,
                    &generation_receipt_path,
                    "selected completion omitted the generation receipt".to_owned(),
                )
            })?;
        if committed_receipt != record.committed_receipt
            || generation_receipt.raw_digest != committed_receipt.raw_digest
            || generation_receipt.byte_length != committed_receipt.byte_length
        {
            return Err(lifecycle_identity_error(
                &self.lock,
                &record_path,
                "selected completion receipt bindings disagree".to_owned(),
            ));
        }
        let generation_manifest_path = completion_generation_root(&input)?.join("generation.yaml");
        let generation_manifest = record
            .materials
            .iter()
            .find(|binding| {
                binding.relative_path == record_relative_path(&generation_manifest_path)
            })
            .ok_or_else(|| {
                lifecycle_identity_error(
                    &self.lock,
                    &generation_manifest_path,
                    "selected completion omitted the generation manifest".to_owned(),
                )
            })?;
        if selector.parent_anchor != generation_manifest.anchor {
            return Err(lifecycle_identity_error(
                &self.lock,
                &selector_path,
                "completion selector parent anchor differs from generation manifest anchor"
                    .to_owned(),
            ));
        }
        let parent_anchor = self
            .lifecycle_root
            .open_file_lifetime_anchor(&selector.parent_anchor)
            .map_err(|error| lifecycle_io_error(&self.lock, &selector_path, error))?;

        for leaf in &material_leaves {
            self.revalidate_completion_leaf(leaf)?;
        }
        self.revalidate_selected_leaf(&selector_leaf)?;
        record_anchor
            .validate_retained_file(&record_file, &record_identity)
            .map_err(|error| lifecycle_io_error(&self.lock, &record_path, error))?;
        parent_anchor
            .revalidate()
            .map_err(|error| lifecycle_io_error(&self.lock, &selector_path, error))?;
        self.revalidate_active_pointer(&installed_pointer)?;
        project_anchors
            .revalidate()
            .map_err(|error| lifecycle_project_anchor_error(project_tree, error))?;
        self.validate_project_tree(project_tree)?;
        if project_tree.snapshot_digest() != record.project_snapshot_digest {
            return Err(RetainedLifecycleIoError::Identity {
                path: project_tree.display_root().to_path_buf(),
                reason: "project snapshot changed during completion validation".to_owned(),
            });
        }
        self.validate_current()?;

        Ok(RetainedDomainPackLifecycleCompletion {
            record_digest: selector.record_digest,
            selector: selector_leaf,
            record_file,
            record_identity,
            record_path,
            record_anchor,
            project_anchors,
            parent_anchor,
            previous_pointer_anchor,
            installed_pointer_anchor: installed_anchor,
            materials: material_leaves,
            active_pointer: Some(installed_pointer),
        })
    }

    /// Revalidate every lifetime-retained selector, record, pointer-anchor, and
    /// material capability carried by a previously selected completion.
    pub fn revalidate_lifecycle_completion(
        &self,
        completion: &RetainedDomainPackLifecycleCompletion,
    ) -> Result<(), RetainedLifecycleIoError> {
        self.revalidate_selected_leaf(&completion.selector)?;
        if completion.record_anchor.binding().content_digest != completion.record_digest {
            return Err(lifecycle_identity_error(
                &self.lock,
                &completion.record_path,
                "retained completion record digest differs from its selector".to_owned(),
            ));
        }
        let record_relative = self.lifecycle_relative(&completion.record_path)?;
        completion
            .record_anchor
            .validate_retained_file(&completion.record_file, &completion.record_identity)
            .map_err(|error| {
                lifecycle_identity_error(&self.lock, &completion.record_path, error.to_string())
            })?;
        self.lifecycle_root
            .verify_retained_authority_binding(
                record_relative,
                &completion.record_file,
                &completion.record_identity,
            )
            .map_err(|error| {
                lifecycle_identity_error(&self.lock, &completion.record_path, error.to_string())
            })?;
        completion.project_anchors.revalidate().map_err(|error| {
            RetainedLifecycleIoError::Identity {
                path: completion.record_path.clone(),
                reason: format!("retained project anchor revalidation failed: {error}"),
            }
        })?;
        completion.parent_anchor.revalidate().map_err(|error| {
            lifecycle_identity_error(
                &self.lock,
                &completion.selector.relative_path,
                error.to_string(),
            )
        })?;
        if let Some(anchor) = &completion.previous_pointer_anchor {
            anchor.revalidate().map_err(|error| {
                lifecycle_identity_error(&self.lock, &completion.record_path, error.to_string())
            })?;
        }
        completion
            .installed_pointer_anchor
            .revalidate()
            .map_err(|error| {
                lifecycle_identity_error(
                    &self.lock,
                    Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                    error.to_string(),
                )
            })?;
        for material in &completion.materials {
            self.revalidate_completion_leaf(material)?;
        }
        if let Some(pointer) = &completion.active_pointer {
            self.revalidate_active_pointer(pointer)?;
        }
        self.validate_current()
    }

    fn retain_completion_materials(
        &self,
        input: &DomainPackLifecycleCompletionInput<'_>,
    ) -> Result<Vec<RetainedLifecycleMaterialLeaf>, RetainedLifecycleIoError> {
        let mut paths = completion_material_paths(input)?;
        paths.sort();
        paths.dedup();
        let object_digests = input
            .object_raw_digests
            .iter()
            .map(|digest| {
                completion_digest_token(digest).map(|token| {
                    (
                        record_relative_path(
                            &Path::new(DOMAIN_PACK_LIFECYCLE_ROOT)
                                .join("objects")
                                .join(token),
                        ),
                        digest.as_str(),
                    )
                })
            })
            .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
        paths
            .into_iter()
            .map(|path| {
                let expected = object_digests.get(&record_relative_path(&path)).copied();
                self.retain_completion_leaf(&path, DOMAIN_PACK_COMPLETION_MAX_BYTES, expected)
            })
            .collect()
    }

    fn retain_completion_leaf(
        &self,
        relative: &Path,
        maximum: u64,
        expected_digest: Option<&str>,
    ) -> Result<RetainedLifecycleMaterialLeaf, RetainedLifecycleIoError> {
        let lifecycle_relative = self.lifecycle_relative(relative)?;
        self.validate_current()?;
        let mut file = self
            .lifecycle_root
            .open_leaf_read_delete_rename_authority(lifecycle_relative)
            .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
        let identity = RetainedDirectory::identity_of(&file)
            .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
        self.lifecycle_root
            .verify_retained_authority_binding(lifecycle_relative, &file, &identity)
            .map_err(|error| lifecycle_identity_error(&self.lock, relative, error.to_string()))?;
        let bytes = read_retained_lifecycle_leaf(&mut file, maximum)
            .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
        let raw_digest = sha256_content_hash(&bytes);
        if expected_digest.is_some_and(|expected| expected != raw_digest) {
            return Err(lifecycle_identity_error(
                &self.lock,
                relative,
                "retained lifecycle material digest differs from its immutable identity".to_owned(),
            ));
        }
        let byte_length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        let anchor = self
            .lifecycle_root
            .retain_file_lifetime_anchor(
                &lifecycle_anchor_directory(relative),
                &file,
                &identity,
                &raw_digest,
                byte_length,
            )
            .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
        let binding = DomainPackLifecycleMaterialBinding {
            relative_path: record_relative_path(relative),
            raw_digest,
            byte_length,
            anchor: anchor.binding().clone(),
        };
        let leaf = RetainedLifecycleMaterialLeaf {
            file,
            identity,
            maximum,
            binding,
            anchor,
        };
        self.revalidate_completion_leaf(&leaf)?;
        Ok(leaf)
    }

    fn retain_bound_completion_materials(
        &self,
        bindings: &[DomainPackLifecycleMaterialBinding],
    ) -> Result<Vec<RetainedLifecycleMaterialLeaf>, RetainedLifecycleIoError> {
        if bindings
            .windows(2)
            .any(|pair| pair[0].relative_path >= pair[1].relative_path)
        {
            return Err(lifecycle_identity_error(
                &self.lock,
                Path::new(DOMAIN_PACK_LIFECYCLE_ROOT),
                "selected completion material bindings are not strictly ordered".to_owned(),
            ));
        }
        bindings
            .iter()
            .map(|binding| {
                if binding.anchor.content_digest != binding.raw_digest
                    || binding.anchor.byte_length != binding.byte_length
                {
                    return Err(lifecycle_identity_error(
                        &self.lock,
                        Path::new(&binding.relative_path),
                        "completion material anchor differs from its content binding".to_owned(),
                    ));
                }
                let relative = Path::new(&binding.relative_path);
                let lifecycle_relative = self.lifecycle_relative(relative)?;
                let anchor = self
                    .lifecycle_root
                    .open_file_lifetime_anchor(&binding.anchor)
                    .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
                let (file, identity) = anchor
                    .retain_target(&self.lifecycle_root, lifecycle_relative)
                    .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
                let leaf = RetainedLifecycleMaterialLeaf {
                    file,
                    identity,
                    maximum: DOMAIN_PACK_COMPLETION_MAX_BYTES,
                    binding: binding.clone(),
                    anchor,
                };
                self.revalidate_completion_leaf(&leaf)?;
                Ok(leaf)
            })
            .collect()
    }

    fn revalidate_completion_leaf(
        &self,
        leaf: &RetainedLifecycleMaterialLeaf,
    ) -> Result<(), RetainedLifecycleIoError> {
        let relative = Path::new(&leaf.binding.relative_path);
        let lifecycle_relative = self.lifecycle_relative(relative)?;
        self.validate_current()?;
        leaf.anchor
            .validate_retained_file(&leaf.file, &leaf.identity)
            .map_err(|error| lifecycle_identity_error(&self.lock, relative, error.to_string()))?;
        self.lifecycle_root
            .verify_retained_authority_binding(lifecycle_relative, &leaf.file, &leaf.identity)
            .map_err(|error| lifecycle_identity_error(&self.lock, relative, error.to_string()))?;
        let bytes = read_retained_lifecycle_leaf(
            &mut leaf
                .file
                .try_clone()
                .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?,
            leaf.maximum,
        )
        .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
        if RetainedDirectory::identity_of(&leaf.file)
            .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?
            != leaf.identity
            || sha256_content_hash(&bytes) != leaf.binding.raw_digest
            || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != leaf.binding.byte_length
        {
            return Err(lifecycle_identity_error(
                &self.lock,
                relative,
                "retained lifecycle material changed identity, bytes, or length".to_owned(),
            ));
        }
        self.lifecycle_root
            .verify_retained_authority_binding(lifecycle_relative, &leaf.file, &leaf.identity)
            .map_err(|error| lifecycle_identity_error(&self.lock, relative, error.to_string()))?;
        leaf.anchor
            .validate_retained_file(&leaf.file, &leaf.identity)
            .map_err(|error| lifecycle_identity_error(&self.lock, relative, error.to_string()))?;
        self.validate_current()
    }

    fn retain_selected_leaf(
        &self,
        relative: &Path,
        maximum: u64,
    ) -> Result<RetainedLifecycleSelectedLeaf, RetainedLifecycleIoError> {
        let lifecycle_relative = self.lifecycle_relative(relative)?;
        self.validate_current()?;
        let mut file = self
            .lifecycle_root
            .open_leaf_read_delete_rename_authority(lifecycle_relative)
            .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
        let identity = RetainedDirectory::identity_of(&file)
            .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
        self.lifecycle_root
            .verify_retained_authority_binding(lifecycle_relative, &file, &identity)
            .map_err(|error| lifecycle_identity_error(&self.lock, relative, error.to_string()))?;
        let bytes = read_retained_lifecycle_leaf(&mut file, maximum)
            .map_err(|error| lifecycle_io_error(&self.lock, relative, error))?;
        let leaf = RetainedLifecycleSelectedLeaf {
            file,
            identity,
            relative_path: relative.to_path_buf(),
            raw_digest: sha256_content_hash(&bytes),
            byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            maximum,
        };
        self.revalidate_selected_leaf(&leaf)?;
        Ok(leaf)
    }

    fn revalidate_selected_leaf(
        &self,
        leaf: &RetainedLifecycleSelectedLeaf,
    ) -> Result<(), RetainedLifecycleIoError> {
        let lifecycle_relative = self.lifecycle_relative(&leaf.relative_path)?;
        self.validate_current()?;
        self.lifecycle_root
            .verify_retained_authority_binding(lifecycle_relative, &leaf.file, &leaf.identity)
            .map_err(|error| {
                lifecycle_identity_error(&self.lock, &leaf.relative_path, error.to_string())
            })?;
        let bytes = read_retained_lifecycle_leaf(
            &mut leaf
                .file
                .try_clone()
                .map_err(|error| lifecycle_io_error(&self.lock, &leaf.relative_path, error))?,
            leaf.maximum,
        )
        .map_err(|error| lifecycle_io_error(&self.lock, &leaf.relative_path, error))?;
        if RetainedDirectory::identity_of(&leaf.file)
            .map_err(|error| lifecycle_io_error(&self.lock, &leaf.relative_path, error))?
            != leaf.identity
            || sha256_content_hash(&bytes) != leaf.raw_digest
            || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != leaf.byte_length
        {
            return Err(lifecycle_identity_error(
                &self.lock,
                &leaf.relative_path,
                "selected lifecycle leaf changed identity, bytes, or length".to_owned(),
            ));
        }
        self.lifecycle_root
            .verify_retained_authority_binding(lifecycle_relative, &leaf.file, &leaf.identity)
            .map_err(|error| {
                lifecycle_identity_error(&self.lock, &leaf.relative_path, error.to_string())
            })?;
        self.validate_current()
    }

    fn anchor_pointer(
        &self,
        pointer: &RetainedDomainPackActivePointerWitness,
        purpose: &str,
    ) -> Result<
        (
            DomainPackLifecyclePointerBinding,
            RetainedFileLifetimeAnchor,
        ),
        RetainedLifecycleIoError,
    > {
        self.validate_active_pointer_handle(pointer)?;
        let byte_length = u64::try_from(pointer.bytes.len()).unwrap_or(u64::MAX);
        let anchor = self
            .lifecycle_root
            .retain_file_lifetime_anchor(
                &Path::new(DOMAIN_PACK_LIFECYCLE_ANCHOR_ROOT).join(purpose),
                &pointer.file,
                &pointer.identity,
                &pointer.digest,
                byte_length,
            )
            .map_err(|error| {
                lifecycle_io_error(&self.lock, Path::new(DOMAIN_PACK_ACTIVE_POINTER), error)
            })?;
        Ok((
            DomainPackLifecyclePointerBinding {
                raw_bytes: pointer.bytes.clone(),
                raw_digest: pointer.digest.clone(),
                byte_length,
                anchor: anchor.binding().clone(),
            },
            anchor,
        ))
    }

    fn validate_active_pointer_handle(
        &self,
        witness: &RetainedDomainPackActivePointerWitness,
    ) -> Result<(), RetainedLifecycleIoError> {
        self.validate_current()?;
        if witness.state_root_identity != self.state_root_identity
            || witness.lifecycle_root_identity != self.lifecycle_root_identity
            || witness.lifecycle_lock_identity != self.lock.lock_identity
        {
            return Err(lifecycle_identity_error(
                &self.lock,
                Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                "active-pointer witness belongs to a different retained root or lock".to_owned(),
            ));
        }
        validate_retained_lifecycle_handle(witness).map_err(|error| {
            lifecycle_identity_error(
                &self.lock,
                Path::new(DOMAIN_PACK_ACTIVE_POINTER),
                error.to_string(),
            )
        })?;
        self.validate_current()
    }

    fn lifecycle_cleanup_paths(&self, paths: Vec<PathBuf>) -> Vec<PathBuf> {
        paths
            .into_iter()
            .map(|path| {
                self.display_path(Path::new(DOMAIN_PACK_LIFECYCLE_ROOT).join(path).as_path())
            })
            .collect()
    }

    fn sync_parent_chain(&self, relative: &Path) -> io::Result<()> {
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        if parent.as_os_str().is_empty() {
            return self.lifecycle_root.sync_root();
        }
        self.lifecycle_root.create_dir_all(parent)?;
        let mut current = PathBuf::new();
        let mut directories = Vec::new();
        for component in parent.components() {
            let Component::Normal(segment) = component else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "retained lifecycle parent is not normalized",
                ));
            };
            current.push(segment);
            directories.push(current.clone());
        }
        for directory in directories.into_iter().rev() {
            self.lifecycle_root.sync_directory(&directory)?;
        }
        self.lifecycle_root.sync_root()
    }

    fn lifecycle_relative<'a>(
        &self,
        relative: &'a Path,
    ) -> Result<&'a Path, RetainedLifecycleIoError> {
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(RetainedLifecycleIoError::InvalidRelativePath {
                path: relative.display().to_string(),
            });
        }
        relative
            .strip_prefix(Path::new(DOMAIN_PACK_LIFECYCLE_ROOT))
            .ok()
            .filter(|path| !path.as_os_str().is_empty())
            .filter(|path| permitted_lifecycle_path(path))
            .ok_or_else(|| RetainedLifecycleIoError::InvalidRelativePath {
                path: relative.display().to_string(),
            })
    }

    fn crash_error(&self, error: RetainedLifecycleIoError) -> CrashReplaceError {
        CrashReplaceError::Io {
            path: self.display_path(Path::new(DOMAIN_PACK_ACTIVE_POINTER)),
            source: error.to_string(),
        }
    }

    fn crash_io_error(&self, relative: &Path, error: io::Error) -> CrashReplaceError {
        CrashReplaceError::Io {
            path: self.display_path(relative),
            source: error.to_string(),
        }
    }
}

fn validate_completion_input(input: &DomainPackLifecycleCompletionInput<'_>) -> Result<(), String> {
    if input.project_id.is_empty() {
        return Err("lifecycle completion project identity is empty".to_owned());
    }
    for (field, digest) in [
        ("project snapshot", input.project_snapshot_digest),
        ("ledger record", input.ledger_record_digest),
        ("lock", input.lock_digest),
        ("preflight", input.preflight_digest),
        ("compatibility report", input.compatibility_report_digest),
        ("receipt", input.receipt_digest),
    ] {
        if completion_digest_token(digest).is_err() {
            return Err(format!("lifecycle completion {field} digest is invalid"));
        }
    }
    let mut objects = input.object_raw_digests.iter().collect::<Vec<_>>();
    objects.sort();
    if objects.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("lifecycle completion object identity is duplicated".to_owned());
    }
    if objects
        .iter()
        .any(|digest| completion_digest_token(digest).is_err())
    {
        return Err("lifecycle completion object digest is invalid".to_owned());
    }
    Ok(())
}

fn completion_generation_binding(
    input: &DomainPackLifecycleCompletionInput<'_>,
) -> DomainPackLifecycleGenerationBinding {
    let mut object_raw_digests = input.object_raw_digests.to_vec();
    object_raw_digests.sort();
    DomainPackLifecycleGenerationBinding {
        generation: input.generation,
        ledger_record_digest: input.ledger_record_digest.to_owned(),
        lock_digest: input.lock_digest.to_owned(),
        preflight_digest: input.preflight_digest.to_owned(),
        compatibility_report_digest: input.compatibility_report_digest.to_owned(),
        receipt_digest: input.receipt_digest.to_owned(),
        object_raw_digests,
    }
}

fn completion_material_paths(
    input: &DomainPackLifecycleCompletionInput<'_>,
) -> Result<Vec<PathBuf>, RetainedLifecycleIoError> {
    let generation_root = completion_generation_root(input)?;
    let mut paths = [
        "generation.yaml",
        "lock.yaml",
        "preflight.yaml",
        "compatibility.yaml",
        "receipt.yaml",
        "catalog.yaml",
        "resolution-request.yaml",
        "composition-request.yaml",
        "trust-input.yaml",
    ]
    .into_iter()
    .map(|name| generation_root.join(name))
    .collect::<Vec<_>>();
    paths.push(
        Path::new(DOMAIN_PACK_LIFECYCLE_ROOT)
            .join("ledger")
            .join(format!(
                "{}.yaml",
                completion_digest_token(input.ledger_record_digest)?
            )),
    );
    paths.push(completion_committed_receipt_path(input)?);
    for digest in input.object_raw_digests {
        paths.push(
            Path::new(DOMAIN_PACK_LIFECYCLE_ROOT)
                .join("objects")
                .join(completion_digest_token(digest)?),
        );
    }
    Ok(paths)
}

fn completion_generation_root(
    input: &DomainPackLifecycleCompletionInput<'_>,
) -> Result<PathBuf, RetainedLifecycleIoError> {
    Ok(Path::new(DOMAIN_PACK_LIFECYCLE_ROOT)
        .join("generations")
        .join(format!(
            "{:020}-{}",
            input.generation,
            completion_digest_token(input.ledger_record_digest)?
        )))
}

fn completion_record_path(
    input: &DomainPackLifecycleCompletionInput<'_>,
) -> Result<PathBuf, RetainedLifecycleIoError> {
    Ok(completion_generation_root(input)?.join(DOMAIN_PACK_COMPLETION_RECORD_LEAF))
}

fn completion_selector_path(
    input: &DomainPackLifecycleCompletionInput<'_>,
) -> Result<PathBuf, RetainedLifecycleIoError> {
    Ok(completion_generation_root(input)?.join(DOMAIN_PACK_COMPLETION_SELECTOR_LEAF))
}

fn completion_committed_receipt_path(
    input: &DomainPackLifecycleCompletionInput<'_>,
) -> Result<PathBuf, RetainedLifecycleIoError> {
    Ok(Path::new(DOMAIN_PACK_LIFECYCLE_ROOT)
        .join("receipts")
        .join(format!(
            "{}.yaml",
            completion_digest_token(input.receipt_digest)?
        )))
}

fn completion_digest_token(digest: &str) -> Result<&str, RetainedLifecycleIoError> {
    let Some(token) = digest.strip_prefix("sha256:") else {
        return Err(RetainedLifecycleIoError::InvalidRelativePath {
            path: digest.to_owned(),
        });
    };
    if token.len() != 64
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RetainedLifecycleIoError::InvalidRelativePath {
            path: digest.to_owned(),
        });
    }
    Ok(token)
}

fn canonical_completion_bytes(record: &DomainPackLifecycleCompletionRecord) -> io::Result<Vec<u8>> {
    serde_json_canonicalizer::to_vec(record).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("canonical lifecycle completion encoding failed: {error}"),
        )
    })
}

fn canonical_completion_selector_bytes(
    selector: &DomainPackLifecycleCompletionSelector,
) -> io::Result<Vec<u8>> {
    serde_json_canonicalizer::to_vec(selector).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("canonical lifecycle completion selector encoding failed: {error}"),
        )
    })
}

fn validate_pointer_anchor_binding(pointer: &DomainPackLifecyclePointerBinding) -> io::Result<()> {
    if sha256_content_hash(&pointer.raw_bytes) != pointer.raw_digest
        || u64::try_from(pointer.raw_bytes.len()).unwrap_or(u64::MAX) != pointer.byte_length
        || pointer.anchor.content_digest != pointer.raw_digest
        || pointer.anchor.byte_length != pointer.byte_length
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "lifecycle pointer anchor differs from its exact content binding",
        ));
    }
    pointer.anchor.canonical_digest().map(|_| ())
}

fn lifecycle_anchor_directory(relative: &Path) -> PathBuf {
    let digest = sha256_content_hash(record_relative_path(relative).as_bytes());
    Path::new(DOMAIN_PACK_LIFECYCLE_ANCHOR_ROOT)
        .join(digest.strip_prefix("sha256:").unwrap_or(digest.as_str()))
}

fn lifecycle_operation_nonce() -> io::Result<String> {
    let mut nonce = [0_u8; 32];
    getrandom::fill(&mut nonce).map_err(|error| {
        io::Error::other(format!(
            "lifecycle completion nonce generation failed: {error}"
        ))
    })?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(nonce.len() * 2);
    for byte in nonce {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(encoded)
}

fn record_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn valid_operation_nonce(nonce: &str) -> bool {
    nonce.len() == 64
        && nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_retained_bytes(
    file: &File,
    identity: &RetainedFileIdentity,
    expected: &[u8],
    maximum: u64,
) -> io::Result<()> {
    if RetainedDirectory::identity_of(file)? != *identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained lifecycle completion leaf changed identity",
        ));
    }
    let actual = read_retained_lifecycle_leaf(&mut file.try_clone()?, maximum)?;
    if actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained lifecycle completion leaf changed exact bytes",
        ));
    }
    Ok(())
}

fn read_selected_leaf_exact(
    leaf: &RetainedLifecycleSelectedLeaf,
) -> Result<Vec<u8>, RetainedLifecycleIoError> {
    let path = leaf.relative_path.clone();
    let bytes = read_retained_lifecycle_leaf(
        &mut leaf
            .file
            .try_clone()
            .map_err(|error| RetainedLifecycleIoError::Io {
                path: path.clone(),
                reason: error.to_string(),
            })?,
        leaf.maximum,
    )
    .map_err(|error| RetainedLifecycleIoError::Io {
        path: path.clone(),
        reason: error.to_string(),
    })?;
    if RetainedDirectory::identity_of(&leaf.file).map_err(|error| RetainedLifecycleIoError::Io {
        path: path.clone(),
        reason: error.to_string(),
    })? != leaf.identity
        || sha256_content_hash(&bytes) != leaf.raw_digest
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != leaf.byte_length
    {
        return Err(RetainedLifecycleIoError::Identity {
            path,
            reason: "selected lifecycle leaf differs from its captured binding".to_owned(),
        });
    }
    Ok(bytes)
}

fn completion_validation_error(error: RetainedLifecycleIoError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn completion_project_validation_error(error: RetainedProjectTreeError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn lifecycle_project_anchor_error(
    project_tree: &RetainedProjectTree,
    error: RetainedProjectTreeError,
) -> RetainedLifecycleIoError {
    RetainedLifecycleIoError::Identity {
        path: project_tree.display_root().to_path_buf(),
        reason: format!("retained project anchor validation failed: {error}"),
    }
}

fn force_lifecycle_placeholder(
    authority: &crate::retained_dir::RetainedAuthorityDirectory<'_>,
    lifecycle_root: &RetainedDirectory,
    file: &File,
    identity: &RetainedFileIdentity,
    target: &Path,
) -> io::Result<crate::retained_dir::RetainedCleanupDebt> {
    validate_lifecycle_placeholder(file, identity)?;
    let debt = authority.force_retained_placeholder_at(file, identity, target)?;
    lifecycle_root.verify_retained_authority_binding(target, file, identity)?;
    validate_lifecycle_placeholder(file, identity)?;
    lifecycle_root.sync_root()?;
    lifecycle_root.verify_retained_authority_binding(target, file, identity)?;
    validate_lifecycle_placeholder(file, identity)?;
    Ok(debt)
}

fn validate_lifecycle_placeholder(file: &File, identity: &RetainedFileIdentity) -> io::Result<()> {
    if RetainedDirectory::identity_of(file)? != *identity || file.metadata()?.len() != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained lifecycle rollback placeholder changed identity or bytes",
        ));
    }
    Ok(())
}

fn validate_retained_lifecycle_handle(
    witness: &RetainedDomainPackActivePointerWitness,
) -> io::Result<()> {
    if RetainedDirectory::identity_of(&witness.file)? != witness.identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained lifecycle active-pointer handle changed identity",
        ));
    }
    let mut file = witness.file.try_clone()?;
    let bytes = read_retained_lifecycle_leaf(&mut file, witness.maximum)?;
    if bytes != witness.bytes || sha256_content_hash(&bytes) != witness.digest {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained lifecycle active-pointer handle changed bytes or digest",
        ));
    }
    Ok(())
}

fn lifecycle_quarantine_path(purpose: &str) -> io::Result<PathBuf> {
    let mut nonce = [0_u8; 16];
    getrandom::fill(&mut nonce).map_err(|error| {
        io::Error::other(format!(
            "lifecycle rollback nonce generation failed: {error}"
        ))
    })?;
    Ok(PathBuf::from(format!(
        ".forge-lifecycle-{purpose}-{}-{:032x}.quarantine",
        std::process::id(),
        u128::from_le_bytes(nonce)
    )))
}

fn read_retained_lifecycle_leaf(file: &mut File, maximum: u64) -> io::Result<Vec<u8>> {
    let before = file.metadata()?;
    if before.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "retained lifecycle leaf exceeds its byte limit",
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    std::io::Read::by_ref(file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum
        || after.len() != before.len()
        || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained lifecycle leaf changed while it was read",
        ));
    }
    Ok(bytes)
}

fn permitted_lifecycle_path(path: &Path) -> bool {
    let components = path
        .components()
        .map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>();
    let Some(components) = components else {
        return false;
    };
    match components.len() {
        1 => matches!(
            components[0],
            "active.lock.yaml" | "ledger" | "generations" | "receipts" | "objects" | "staging"
        ),
        2 if components[0] == "objects" => is_lower_hex(components[1], 64),
        2 if matches!(components[0], "ledger" | "receipts") => components[1]
            .strip_suffix(".yaml")
            .is_some_and(|token| is_lower_hex(token, 64)),
        3 if components[0] == "generations" => {
            let generation = components[1].as_bytes();
            generation.len() == 85
                && generation.get(20) == Some(&b'-')
                && generation[..20].iter().all(u8::is_ascii_digit)
                && generation[21..]
                    .iter()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
                && matches!(
                    components[2],
                    "generation.yaml"
                        | "lock.yaml"
                        | "preflight.yaml"
                        | "compatibility.yaml"
                        | "receipt.yaml"
                        | "catalog.yaml"
                        | "resolution-request.yaml"
                        | "composition-request.yaml"
                        | "trust-input.yaml"
                        | DOMAIN_PACK_COMPLETION_RECORD_LEAF
                        | DOMAIN_PACK_COMPLETION_SELECTOR_LEAF
                )
        }
        _ => false,
    }
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_lock_root(lock: &EffectStoreLock) -> Result<(), RetainedLifecycleIoError> {
    lock.boundary
        .require_effect_authority()
        .map_err(|error| lifecycle_identity_error(lock, Path::new(""), error.to_string()))?;
    lock.boundary
        .validate_root(lock.state_root.display_path())
        .map_err(|error| lifecycle_identity_error(lock, Path::new(""), error.to_string()))?;
    lock.validate_retained_lock_file()
        .map_err(|error| lifecycle_lock_error(lock, error))
}

fn lifecycle_lock_error(
    lock: &EffectStoreLock,
    error: EffectStoreLockError,
) -> RetainedLifecycleIoError {
    lifecycle_identity_error(
        lock,
        Path::new(DOMAIN_PACK_LIFECYCLE_LOCK),
        error.to_string(),
    )
}

fn lifecycle_identity_error(
    lock: &EffectStoreLock,
    relative: &Path,
    reason: String,
) -> RetainedLifecycleIoError {
    RetainedLifecycleIoError::Identity {
        path: lock.state_root.display_path().join(relative),
        reason,
    }
}

fn lifecycle_io_error(
    lock: &EffectStoreLock,
    relative: &Path,
    error: io::Error,
) -> RetainedLifecycleIoError {
    RetainedLifecycleIoError::Io {
        path: lock.state_root.display_path().join(relative),
        reason: error.to_string(),
    }
}
