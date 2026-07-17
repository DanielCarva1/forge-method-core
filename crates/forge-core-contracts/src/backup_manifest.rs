//! Closed, source-derived contract for complete Forge state backups.
//!
//! This is deliberately an authority inventory, not a recursive file backup
//! format. Every v1 member has a typed name and a source-defined path. Archive
//! walkers must no-follow enumerate the declared roots, reject private roots
//! before constructing entries, and compare their result exactly to this list.

use crate::{
    ProjectLinkDocument, WorkflowEffectiveBundleIdentity, WorkflowGovernanceReleaseIdentity,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const BACKUP_MANIFEST_SCHEMA_VERSION: &str = "forge_project_state_backup_manifest_v1";
const SET_DIGEST_DOMAIN: &[u8] = b"forge-method:project-state-backup-set:v1\0";
const PROJECT_LINK_ARCHIVE_PATH: &str = "project/.forge-method.yaml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupManifestDocument {
    pub schema_version: String,
    pub backup_manifest: BackupManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupManifest {
    pub format: BackupManifestFormat,
    /// The full Project Link is retained so archive mapping cannot silently
    /// assume the default sidecar layout.
    pub project: BackupProjectBinding,
    /// The existing release identity is retained verbatim, including
    /// `release_version`.
    pub workflow_release: WorkflowGovernanceReleaseIdentity,
    /// The existing effective bundle identity is retained verbatim, including
    /// both runtime identities, receipt context, and domain-pack bindings.
    pub effective_epoch: BackupEffectiveEpochBinding,
    pub snapshot: BackupSnapshotBinding,
    /// Exact ordered, typed authority set. No catch-all state-file variant is
    /// available in v1.
    pub entries: Vec<BackupEntry>,
    pub external_authorities: BackupExternalAuthorities,
    pub forbidden_private_material: Vec<BackupForbiddenPrivateMaterial>,
    pub manifest_set_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupManifestFormat {
    ForgeProjectStateBackupV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupProjectBinding {
    pub project_link: ProjectLinkDocument,
    pub project_link_sha256: String,
    pub state_generation: u64,
    /// Physical-to-logical mapping derived from the exact Project Link.
    pub archive_layout: BackupArchiveLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupArchiveLayout {
    pub project_link_archive_path: String,
    pub sidecar_archive_root: String,
    /// The normalized relative relationship `state_root - sidecar_root` from
    /// the Project Link. It is never assumed to be `.forge-method`.
    pub state_root_relative_to_sidecar: String,
    /// Directories are created during restore but are not archive members.
    pub restore_created_directories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupEffectiveEpochBinding {
    pub epoch_id: String,
    pub epoch_generation: u64,
    pub effective_bundle: WorkflowEffectiveBundleIdentity,
    pub governance_ledger_head_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupSnapshotBinding {
    /// Actual cooperative lock files, held in this order through recovery,
    /// no-follow enumeration, staged copy/hash/fsync, and manifest publish.
    pub lock_order: Vec<BackupLockScope>,
    pub stores_recovered_before_copy: bool,
    pub manifest_published_last: bool,
    /// Restore must read and compare the currently protected external anchor
    /// before any replay or write; an internally valid older backup fails.
    pub restore_reads_current_anchor_before_replay: bool,
    pub restore_compatibility: BackupRestoreCompatibility,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BackupLockScope {
    DomainPackLifecycle,
    WorkflowGovernance,
    ClaimWal,
    WorkflowActionReplayWal,
    EffectWal,
    ReplayWal,
    ExternalReplayAnchor,
}

impl BackupLockScope {
    pub const fn relative_path(self) -> &'static str {
        match self {
            Self::DomainPackLifecycle => "locks/domain-packs.lifecycle.lock",
            Self::WorkflowGovernance => "locks/workflow-governance.lock",
            Self::ClaimWal => "locks/claims.wal.lock",
            Self::WorkflowActionReplayWal => "locks/workflow-action-replay.lock",
            Self::EffectWal => "locks/effects.lock",
            Self::ReplayWal => "locks/replay.wal.lock",
            // The actual anchor lock is the protected anchor filename plus
            // `.lock`; its parent is outside the sidecar by design.
            Self::ExternalReplayAnchor => "<protected-anchor>.lock",
        }
    }
}

/// Effect precedes replay, matching `replay_wal`'s required global ordering.
pub const BACKUP_LOCK_ORDER: &[BackupLockScope] = &[
    BackupLockScope::DomainPackLifecycle,
    BackupLockScope::WorkflowGovernance,
    BackupLockScope::ClaimWal,
    BackupLockScope::WorkflowActionReplayWal,
    BackupLockScope::EffectWal,
    BackupLockScope::ReplayWal,
    BackupLockScope::ExternalReplayAnchor,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupRestoreCompatibility {
    ExactV1Only,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupMaterialClass {
    Required,
}

/// Source-derived durable authority inventory. The names and path forms match
/// the store APIs; adding a new durable authority requires a new manifest
/// version rather than being admitted through a generic file variant.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BackupEntryKind {
    ProjectLink,
    WorkflowGovernanceWal,
    ClaimWal,
    ClaimWalManifest,
    ClaimWalSnapshot,
    ClaimWalArchive,
    ReplayWalManifest,
    ReplayWal,
    WorkflowActionReplayManifest,
    WorkflowActionReplayWal,
    EffectWal,
    EffectWalCompactionManifest,
    DomainPackActiveLock,
    DomainPackLedgerRecord,
    DomainPackGenerationLock,
    DomainPackGenerationPreflight,
    DomainPackGenerationReceipt,
    DomainPackReceipt,
    DomainPackObject,
    PublicPrincipalRegistry,
    PublicBrokerRegistry,
}

impl BackupEntryKind {
    const fn may_repeat(self) -> bool {
        matches!(
            self,
            Self::ClaimWalSnapshot
                | Self::ClaimWalArchive
                | Self::DomainPackLedgerRecord
                | Self::DomainPackReceipt
                | Self::DomainPackObject
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupArchiveEntryType {
    RegularFile,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupEntry {
    pub material: BackupEntryKind,
    pub classification: BackupMaterialClass,
    pub logical_path: String,
    pub entry_type: BackupArchiveEntryType,
    pub byte_length: u64,
    pub sha256: String,
    pub project_id: String,
    pub state_generation: u64,
    pub workflow_release_digest: String,
    pub effective_receipt_context_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupExternalAuthorities {
    pub broker_public_trust: BackupPublicBrokerTrust,
    pub replay_rollback_anchor: BackupReplayRollbackAnchor,
}

/// The broker registry is the real public authority root. Private key roots
/// are neither a registry alternative nor archive material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupPublicBrokerTrust {
    pub registry_format: String,
    pub registry_logical_path: String,
    pub identity: String,
    pub public_key_fingerprint: String,
    pub observed_generation: u64,
    pub registry_sha256: String,
}

/// Complete replay-anchor document binding. Restore preflight compares this
/// whole value with the current protected anchor before replaying a WAL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupReplayRollbackAnchor {
    pub schema_version: String,
    pub protected_anchor_identity: String,
    pub deployment_id: String,
    pub epoch: String,
    pub generation: u64,
    pub previous_anchor_digest: Option<String>,
    pub anchor_document_sha256: String,
    pub replay_wal_manifest_digest: String,
    pub replay_wal_prefix_digest: String,
    pub replay_wal_last_seq: u64,
    pub replay_wal_record_count: u64,
    pub replay_wal_byte_length: u64,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BackupForbiddenPrivateMaterial {
    BrokerPrivateKeys,
    WorkflowSecretRoots,
    OperatorSecretRoots,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupManifestValidationError {
    UnsupportedSchemaVersion,
    UnsupportedManifestFormat,
    InvalidDigest {
        field: &'static str,
    },
    Blank {
        field: &'static str,
    },
    InvalidLogicalPath {
        path: String,
    },
    InvalidArchiveLayout,
    InvalidEntryPath {
        material: BackupEntryKind,
        path: String,
    },
    ForbiddenPrivatePath {
        path: String,
    },
    NonRegularArchiveEntry {
        path: String,
    },
    WrongMaterialClassification {
        material: BackupEntryKind,
    },
    EntriesNotCanonical,
    DuplicateEntry {
        material: BackupEntryKind,
    },
    MissingRequiredEntry {
        material: BackupEntryKind,
    },
    BindingMismatch {
        field: &'static str,
    },
    InvalidSnapshotProtocol,
    InvalidExternalAuthorities,
    PrivateMaterialPolicyMismatch,
    ManifestSetDigestMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupArchiveVerificationError {
    MissingEntry { path: String },
    ExtraEntry { path: String },
    DuplicateEntry { path: String },
    SubstitutedEntry { path: String },
    SymlinkOrNonRegularEntry { path: String },
}

impl BackupManifestDocument {
    pub fn validate(&self) -> Result<(), BackupManifestValidationError> {
        if self.schema_version != BACKUP_MANIFEST_SCHEMA_VERSION {
            return Err(BackupManifestValidationError::UnsupportedSchemaVersion);
        }
        self.backup_manifest.validate(self)
    }

    pub fn canonical_set_bytes(&self) -> Result<Vec<u8>, BackupManifestValidationError> {
        let mut value = serde_json::to_value(self)
            .map_err(|_| BackupManifestValidationError::ManifestSetDigestMismatch)?;
        value
            .get_mut("backup_manifest")
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|manifest| manifest.remove("manifest_set_digest"))
            .ok_or(BackupManifestValidationError::ManifestSetDigestMismatch)?;
        serde_json_canonicalizer::to_vec(&value)
            .map_err(|_| BackupManifestValidationError::ManifestSetDigestMismatch)
    }

    pub fn set_digest(&self) -> Result<String, BackupManifestValidationError> {
        let canonical = self.canonical_set_bytes()?;
        let mut hasher = Sha256::new();
        hasher.update(SET_DIGEST_DOMAIN);
        hasher.update((canonical.len() as u64).to_be_bytes());
        hasher.update(canonical);
        Ok(format!("sha256:{:x}", hasher.finalize()))
    }

    pub fn verify_archive_entries(
        &self,
        observed: &[BackupEntry],
    ) -> Result<(), BackupArchiveVerificationError> {
        let expected = self
            .backup_manifest
            .entries
            .iter()
            .map(|entry| (entry.logical_path.as_str(), entry))
            .collect::<BTreeMap<_, _>>();
        let mut actual = BTreeMap::new();
        for entry in observed {
            if actual.insert(entry.logical_path.as_str(), entry).is_some() {
                return Err(BackupArchiveVerificationError::DuplicateEntry {
                    path: entry.logical_path.clone(),
                });
            }
        }
        for path in expected.keys() {
            if !actual.contains_key(path) {
                return Err(BackupArchiveVerificationError::MissingEntry {
                    path: (*path).to_owned(),
                });
            }
        }
        for path in actual.keys() {
            if !expected.contains_key(path) {
                return Err(BackupArchiveVerificationError::ExtraEntry {
                    path: (*path).to_owned(),
                });
            }
        }
        for (path, expected_entry) in expected {
            let actual_entry = actual[&path];
            if actual_entry.entry_type != BackupArchiveEntryType::RegularFile {
                return Err(BackupArchiveVerificationError::SymlinkOrNonRegularEntry {
                    path: path.to_owned(),
                });
            }
            if actual_entry != expected_entry {
                return Err(BackupArchiveVerificationError::SubstitutedEntry {
                    path: path.to_owned(),
                });
            }
        }
        Ok(())
    }
}

impl BackupManifest {
    fn validate(
        &self,
        document: &BackupManifestDocument,
    ) -> Result<(), BackupManifestValidationError> {
        if self.format != BackupManifestFormat::ForgeProjectStateBackupV1 {
            return Err(BackupManifestValidationError::UnsupportedManifestFormat);
        }
        validate_project(&self.project)?;
        validate_release(&self.workflow_release)?;
        validate_effective_epoch(&self.effective_epoch)?;
        if self.snapshot.lock_order.as_slice() != BACKUP_LOCK_ORDER
            || !self.snapshot.stores_recovered_before_copy
            || !self.snapshot.manifest_published_last
            || !self.snapshot.restore_reads_current_anchor_before_replay
            || self.snapshot.restore_compatibility != BackupRestoreCompatibility::ExactV1Only
        {
            return Err(BackupManifestValidationError::InvalidSnapshotProtocol);
        }

        let mut previous = None;
        let mut paths = BTreeSet::new();
        let mut seen = BTreeSet::new();
        for entry in &self.entries {
            validate_safe_path(&entry.logical_path)?;
            if is_forbidden_private_path(&entry.logical_path) {
                return Err(BackupManifestValidationError::ForbiddenPrivatePath {
                    path: entry.logical_path.clone(),
                });
            }
            if entry.entry_type != BackupArchiveEntryType::RegularFile {
                return Err(BackupManifestValidationError::NonRegularArchiveEntry {
                    path: entry.logical_path.clone(),
                });
            }
            if entry.classification != BackupMaterialClass::Required {
                return Err(BackupManifestValidationError::WrongMaterialClassification {
                    material: entry.material,
                });
            }
            validate_entry_path(entry, &self.project.archive_layout)?;
            let key = (entry.material, entry.logical_path.as_str());
            if previous.is_some_and(|prior| prior >= key) || !paths.insert(&entry.logical_path) {
                return Err(BackupManifestValidationError::EntriesNotCanonical);
            }
            previous = Some(key);
            if !entry.material.may_repeat() && seen.contains(&entry.material) {
                return Err(BackupManifestValidationError::DuplicateEntry {
                    material: entry.material,
                });
            }
            seen.insert(entry.material);
            digest("entries[].sha256", &entry.sha256)?;
            if entry.project_id != self.project.project_link.project_id.0 {
                return Err(BackupManifestValidationError::BindingMismatch {
                    field: "entries[].project_id",
                });
            }
            if entry.state_generation != self.project.state_generation {
                return Err(BackupManifestValidationError::BindingMismatch {
                    field: "entries[].state_generation",
                });
            }
            if entry.workflow_release_digest != self.workflow_release.release_digest {
                return Err(BackupManifestValidationError::BindingMismatch {
                    field: "entries[].workflow_release_digest",
                });
            }
            if entry.effective_receipt_context_digest
                != self.effective_epoch.effective_bundle.receipt_context_digest
            {
                return Err(BackupManifestValidationError::BindingMismatch {
                    field: "entries[].effective_receipt_context_digest",
                });
            }
        }
        for material in required_materials() {
            if !seen.contains(&material) {
                return Err(BackupManifestValidationError::MissingRequiredEntry { material });
            }
        }
        validate_external_authorities(self)?;
        if self.forbidden_private_material
            != [
                BackupForbiddenPrivateMaterial::BrokerPrivateKeys,
                BackupForbiddenPrivateMaterial::WorkflowSecretRoots,
                BackupForbiddenPrivateMaterial::OperatorSecretRoots,
            ]
        {
            return Err(BackupManifestValidationError::PrivateMaterialPolicyMismatch);
        }
        digest("manifest_set_digest", &self.manifest_set_digest)?;
        if self.manifest_set_digest != document.set_digest()? {
            return Err(BackupManifestValidationError::ManifestSetDigestMismatch);
        }
        Ok(())
    }
}

fn required_materials() -> impl Iterator<Item = BackupEntryKind> {
    [
        BackupEntryKind::ProjectLink,
        BackupEntryKind::WorkflowGovernanceWal,
        BackupEntryKind::ClaimWal,
        BackupEntryKind::ClaimWalManifest,
        BackupEntryKind::ClaimWalSnapshot,
        BackupEntryKind::ClaimWalArchive,
        BackupEntryKind::ReplayWalManifest,
        BackupEntryKind::ReplayWal,
        BackupEntryKind::WorkflowActionReplayManifest,
        BackupEntryKind::WorkflowActionReplayWal,
        BackupEntryKind::EffectWal,
        BackupEntryKind::EffectWalCompactionManifest,
        BackupEntryKind::DomainPackActiveLock,
        BackupEntryKind::DomainPackLedgerRecord,
        BackupEntryKind::DomainPackGenerationLock,
        BackupEntryKind::DomainPackGenerationPreflight,
        BackupEntryKind::DomainPackGenerationReceipt,
        BackupEntryKind::DomainPackReceipt,
        BackupEntryKind::DomainPackObject,
        BackupEntryKind::PublicPrincipalRegistry,
        BackupEntryKind::PublicBrokerRegistry,
    ]
    .into_iter()
}

fn validate_project(project: &BackupProjectBinding) -> Result<(), BackupManifestValidationError> {
    required(
        "project.project_link.project_id",
        &project.project_link.project_id.0,
    )?;
    if project.project_link.schema_version != crate::PROJECT_LINK_SCHEMA_VERSION {
        return Err(BackupManifestValidationError::BindingMismatch {
            field: "project.project_link.schema_version",
        });
    }
    digest("project.project_link_sha256", &project.project_link_sha256)?;
    let layout = &project.archive_layout;
    if layout.project_link_archive_path != PROJECT_LINK_ARCHIVE_PATH
        || layout.sidecar_archive_root != "sidecar"
        || !safe_relative(&layout.state_root_relative_to_sidecar)
        || layout.restore_created_directories
            != vec![
                "locks".to_owned(),
                "wal/snapshots".to_owned(),
                "wal/archive".to_owned(),
                "domain-packs/objects".to_owned(),
            ]
        || normalized_relative(
            &project.project_link.state_root.0,
            &project.project_link.sidecar_root.0,
        )
        .as_deref()
            != Some(layout.state_root_relative_to_sidecar.as_str())
    {
        return Err(BackupManifestValidationError::InvalidArchiveLayout);
    }
    Ok(())
}

fn validate_release(
    value: &WorkflowGovernanceReleaseIdentity,
) -> Result<(), BackupManifestValidationError> {
    required("workflow_release.lineage_id", &value.lineage_id.0)?;
    required("workflow_release.release_id", &value.release_id.0)?;
    required("workflow_release.release_version", &value.release_version)?;
    digest("workflow_release.release_digest", &value.release_digest)
}

fn validate_effective_epoch(
    value: &BackupEffectiveEpochBinding,
) -> Result<(), BackupManifestValidationError> {
    required("effective_epoch.epoch_id", &value.epoch_id)?;
    digest(
        "effective_epoch.governance_ledger_head_digest",
        &value.governance_ledger_head_digest,
    )?;
    let bundle = &value.effective_bundle;
    for (field, identity) in [
        (
            "effective_epoch.core_runtime_bundle.bundle_id",
            &bundle.core_runtime_bundle.bundle_id.0,
        ),
        (
            "effective_epoch.effective_runtime_bundle.bundle_id",
            &bundle.effective_runtime_bundle.bundle_id.0,
        ),
    ] {
        required(field, identity)?;
    }
    for (field, value) in [
        (
            "effective_epoch.core_runtime_bundle.bundle_digest",
            &bundle.core_runtime_bundle.bundle_digest,
        ),
        (
            "effective_epoch.core_runtime_bundle.policy_set_digest",
            &bundle.core_runtime_bundle.policy_set_digest,
        ),
        (
            "effective_epoch.effective_runtime_bundle.bundle_digest",
            &bundle.effective_runtime_bundle.bundle_digest,
        ),
        (
            "effective_epoch.effective_runtime_bundle.policy_set_digest",
            &bundle.effective_runtime_bundle.policy_set_digest,
        ),
        (
            "effective_epoch.receipt_context_digest",
            &bundle.receipt_context_digest,
        ),
    ] {
        digest(field, value)?;
    }
    if let Some(pack) = &bundle.domain_pack_generation {
        for (field, value) in [
            (
                "effective_epoch.domain_pack.active_lock_digest",
                &pack.active_lock_digest,
            ),
            (
                "effective_epoch.domain_pack.composition_digest",
                &pack.composition_digest,
            ),
            (
                "effective_epoch.domain_pack.base_core_bundle_digest",
                &pack.base_core_bundle_digest,
            ),
            (
                "effective_epoch.domain_pack.supply_chain_registry_digest",
                &pack.supply_chain_registry_digest,
            ),
            (
                "effective_epoch.domain_pack.reviewer_registry_digest",
                &pack.reviewer_registry_digest,
            ),
            (
                "effective_epoch.domain_pack.reviewed_registry_digest",
                &pack.reviewed_registry_digest,
            ),
        ] {
            digest(field, value)?;
        }
    }
    Ok(())
}

fn validate_external_authorities(
    manifest: &BackupManifest,
) -> Result<(), BackupManifestValidationError> {
    let external = &manifest.external_authorities;
    let broker = &external.broker_public_trust;
    if broker.registry_format != "forge_workflow_broker_registry_v1"
        || broker.registry_logical_path
            != sidecar_path(
                &manifest.project.archive_layout,
                "operator/workflow-broker-registry.yaml",
            )
    {
        return Err(BackupManifestValidationError::InvalidExternalAuthorities);
    }
    required("broker_public_trust.identity", &broker.identity)?;
    digest(
        "broker_public_trust.public_key_fingerprint",
        &broker.public_key_fingerprint,
    )?;
    digest(
        "broker_public_trust.registry_sha256",
        &broker.registry_sha256,
    )?;
    let broker_entry = manifest
        .entries
        .iter()
        .find(|entry| entry.material == BackupEntryKind::PublicBrokerRegistry);
    if broker_entry.is_none_or(|entry| entry.sha256 != broker.registry_sha256) {
        return Err(BackupManifestValidationError::BindingMismatch {
            field: "broker_public_trust.registry_sha256",
        });
    }

    let anchor = &external.replay_rollback_anchor;
    required(
        "replay_rollback_anchor.protected_anchor_identity",
        &anchor.protected_anchor_identity,
    )?;
    required(
        "replay_rollback_anchor.deployment_id",
        &anchor.deployment_id,
    )?;
    required("replay_rollback_anchor.epoch", &anchor.epoch)?;
    digest(
        "replay_rollback_anchor.anchor_document_sha256",
        &anchor.anchor_document_sha256,
    )?;
    digest(
        "replay_rollback_anchor.replay_wal_manifest_digest",
        &anchor.replay_wal_manifest_digest,
    )?;
    digest(
        "replay_rollback_anchor.replay_wal_prefix_digest",
        &anchor.replay_wal_prefix_digest,
    )?;
    if let Some(previous) = &anchor.previous_anchor_digest {
        digest("replay_rollback_anchor.previous_anchor_digest", previous)?;
    }
    let manifest_entry = manifest
        .entries
        .iter()
        .find(|entry| entry.material == BackupEntryKind::ReplayWalManifest);
    if manifest_entry.is_none_or(|entry| entry.sha256 != anchor.replay_wal_manifest_digest)
        || anchor.generation < manifest.project.state_generation
    {
        return Err(BackupManifestValidationError::BindingMismatch {
            field: "replay_rollback_anchor",
        });
    }
    Ok(())
}

fn validate_entry_path(
    entry: &BackupEntry,
    layout: &BackupArchiveLayout,
) -> Result<(), BackupManifestValidationError> {
    let state = |suffix: &str| {
        sidecar_path(
            layout,
            &format!("{}/{}", layout.state_root_relative_to_sidecar, suffix),
        )
    };
    let sidecar = |suffix: &str| sidecar_path(layout, suffix);
    let valid = match entry.material {
        BackupEntryKind::ProjectLink => entry.logical_path == layout.project_link_archive_path,
        BackupEntryKind::WorkflowGovernanceWal => {
            entry.logical_path == state("wal/workflow-governance.ndjson")
        }
        BackupEntryKind::ClaimWal => entry.logical_path == state("wal/claims.fmw1"),
        BackupEntryKind::ClaimWalManifest => {
            entry.logical_path == state("wal/claims.wal.manifest.json")
        }
        BackupEntryKind::ClaimWalSnapshot => {
            entry.logical_path.starts_with(&state("wal/snapshots/"))
        }
        BackupEntryKind::ClaimWalArchive => entry.logical_path.starts_with(&state("wal/archive/")),
        BackupEntryKind::ReplayWalManifest => {
            entry.logical_path == state("replay-wal.manifest.json")
        }
        BackupEntryKind::ReplayWal => entry.logical_path == state("wal/replay.fmr1"),
        BackupEntryKind::WorkflowActionReplayManifest => {
            entry.logical_path == state("workflow-action-replay.manifest.json")
        }
        BackupEntryKind::WorkflowActionReplayWal => {
            entry.logical_path == state("wal/workflow-action-replay.jsonl")
        }
        BackupEntryKind::EffectWal => entry.logical_path == state("wal/effects.ndjson"),
        BackupEntryKind::EffectWalCompactionManifest => {
            entry.logical_path == state("wal/.effects.ndjson.compaction-manifest.json")
        }
        BackupEntryKind::DomainPackActiveLock => {
            entry.logical_path == state("domain-packs/active.lock.yaml")
        }
        BackupEntryKind::DomainPackLedgerRecord => entry
            .logical_path
            .starts_with(&state("domain-packs/ledger/")),
        BackupEntryKind::DomainPackGenerationLock => {
            entry
                .logical_path
                .starts_with(&state("domain-packs/generations/"))
                && entry.logical_path.ends_with("/lock.yaml")
        }
        BackupEntryKind::DomainPackGenerationPreflight => {
            entry
                .logical_path
                .starts_with(&state("domain-packs/generations/"))
                && entry.logical_path.ends_with("/preflight.yaml")
        }
        BackupEntryKind::DomainPackGenerationReceipt => {
            entry
                .logical_path
                .starts_with(&state("domain-packs/generations/"))
                && entry.logical_path.ends_with("/receipt.yaml")
        }
        BackupEntryKind::DomainPackReceipt => entry
            .logical_path
            .starts_with(&state("domain-packs/receipts/")),
        BackupEntryKind::DomainPackObject => entry
            .logical_path
            .starts_with(&state("domain-packs/objects/")),
        BackupEntryKind::PublicPrincipalRegistry => {
            entry.logical_path == sidecar("operator/workflow-principal-registry.yaml")
        }
        BackupEntryKind::PublicBrokerRegistry => {
            entry.logical_path == sidecar("operator/workflow-broker-registry.yaml")
        }
    };
    if valid {
        Ok(())
    } else {
        Err(BackupManifestValidationError::InvalidEntryPath {
            material: entry.material,
            path: entry.logical_path.clone(),
        })
    }
}

fn sidecar_path(layout: &BackupArchiveLayout, suffix: &str) -> String {
    format!("{}/{}", layout.sidecar_archive_root, suffix)
}

fn is_forbidden_private_path(path: &str) -> bool {
    path.starts_with("sidecar/operator/workflow-secrets/")
        || path.starts_with("sidecar/operator/secrets/")
        || path.contains("/private-keys/")
        || path.ends_with(".pem")
        || path.ends_with(".key")
}

fn safe_relative(path: &str) -> bool {
    !path.is_empty() && validate_safe_path(path).is_ok()
}

fn normalized_relative(state_root: &str, sidecar_root: &str) -> Option<String> {
    fn normalize(value: &str) -> Option<Vec<String>> {
        let mut parts = Vec::new();
        for component in value.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    if parts.last().is_some_and(|part| part != "..") {
                        parts.pop();
                    } else {
                        parts.push("..".to_owned());
                    }
                }
                value
                    if !value.contains('\\')
                        && !value.bytes().any(|byte| byte.is_ascii_control()) =>
                {
                    parts.push(value.to_owned())
                }
                _ => return None,
            }
        }
        Some(parts)
    }
    let sidecar = normalize(sidecar_root)?;
    let state = normalize(state_root)?;
    Some(state.strip_prefix(sidecar.as_slice())?.join("/"))
}

fn validate_safe_path(path: &str) -> Result<(), BackupManifestValidationError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(BackupManifestValidationError::InvalidLogicalPath {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn required(field: &'static str, value: &str) -> Result<(), BackupManifestValidationError> {
    if value.trim().is_empty() {
        Err(BackupManifestValidationError::Blank { field })
    } else {
        Ok(())
    }
}

fn digest(field: &'static str, value: &str) -> Result<(), BackupManifestValidationError> {
    if value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(BackupManifestValidationError::InvalidDigest { field })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_ROOT: &str = "../../contracts/fixtures/backup-manifest";

    fn fixture(path: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(FIXTURE_ROOT)
                .join(path),
        )
        .unwrap()
    }

    fn valid_fixture() -> BackupManifestDocument {
        yaml_serde::from_str(&fixture("valid/complete-state-v1.yaml")).unwrap()
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct HostileFixture {
        case: String,
        replacements: Vec<HostileReplacement>,
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct HostileReplacement {
        from: String,
        to: String,
    }

    fn hostile_fixture(path: &str) -> String {
        let hostile: HostileFixture = yaml_serde::from_str(&fixture(path)).unwrap();
        assert_eq!(
            hostile.case,
            path.rsplit('/')
                .next()
                .unwrap()
                .trim_end_matches(".invalid.yaml")
        );
        let mut document = fixture("valid/complete-state-v1.yaml");
        for replacement in hostile.replacements {
            assert!(
                document.contains(&replacement.from),
                "{} must apply",
                hostile.case
            );
            document = document.replacen(&replacement.from, &replacement.to, 1);
        }
        document
    }

    #[test]
    fn backup_manifest_valid_fixture_is_complete_and_canonical() {
        let document = valid_fixture();
        document.validate().unwrap();
        assert_eq!(
            document.set_digest().unwrap(),
            document.backup_manifest.manifest_set_digest
        );
    }

    #[test]
    fn frozen_hostile_fixtures_fail_closed() {
        for name in [
            "anchor-wal-binding.invalid.yaml",
            "duplicate-entry.invalid.yaml",
            "extra-entry.invalid.yaml",
            "identity-domain-reviewer.invalid.yaml",
            "identity-release-version.invalid.yaml",
            "lock-effect-after-replay.invalid.yaml",
            "mixed-project.invalid.yaml",
            "omitted-claim-wal.invalid.yaml",
            "omitted-entry.invalid.yaml",
            "path-traversal.invalid.yaml",
            "private-key-entry.invalid.yaml",
            "private-root.invalid.yaml",
            "release-mismatch.invalid.yaml",
            "stale-generation.invalid.yaml",
            "substituted-project.invalid.yaml",
            "symlink-entry.invalid.yaml",
            "unknown-field.invalid.yaml",
            "unknown-version.invalid.yaml",
        ] {
            let parsed = yaml_serde::from_str::<BackupManifestDocument>(&hostile_fixture(
                &format!("hostile/{name}"),
            ));
            assert!(
                parsed.is_err() || parsed.unwrap().validate().is_err(),
                "{name} must fail closed"
            );
        }
    }

    #[test]
    fn backup_manifest_rejects_each_typed_authority_omission_and_substitution() {
        let document = valid_fixture();
        for index in 0..document.backup_manifest.entries.len() {
            let mut missing = document.backup_manifest.entries.clone();
            missing.remove(index);
            assert!(document.verify_archive_entries(&missing).is_err());
            let mut replaced = document.backup_manifest.entries.clone();
            replaced[index].sha256 = format!("sha256:{}", "f".repeat(64));
            assert!(matches!(
                document.verify_archive_entries(&replaced),
                Err(BackupArchiveVerificationError::SubstitutedEntry { .. })
            ));
        }
        let mut extra = document.backup_manifest.entries.clone();
        let mut injected = extra[0].clone();
        injected.logical_path = "sidecar/private-keys/injected.key".to_owned();
        extra.push(injected);
        assert!(matches!(
            document.verify_archive_entries(&extra),
            Err(BackupArchiveVerificationError::ExtraEntry { .. })
        ));
    }

    fn recompute(document: &mut BackupManifestDocument) {
        document.backup_manifest.manifest_set_digest = document.set_digest().unwrap();
    }

    #[test]
    fn hostile_identity_component_anchor_and_lock_order_mutations_fail_closed() {
        let document = valid_fixture();
        let mut release = document.clone();
        release
            .backup_manifest
            .workflow_release
            .release_version
            .clear();
        recompute(&mut release);
        assert!(release.validate().is_err());

        for component in 0..7 {
            let mut effective = document.clone();
            let bundle = &mut effective.backup_manifest.effective_epoch.effective_bundle;
            match component {
                0 => bundle.core_runtime_bundle.bundle_id.0.clear(),
                1 => bundle.core_runtime_bundle.bundle_digest.clear(),
                2 => bundle.core_runtime_bundle.policy_set_digest.clear(),
                3 => bundle.effective_runtime_bundle.bundle_id.0.clear(),
                4 => bundle.effective_runtime_bundle.bundle_digest.clear(),
                5 => bundle.effective_runtime_bundle.policy_set_digest.clear(),
                _ => bundle.receipt_context_digest.clear(),
            }
            recompute(&mut effective);
            assert!(effective.validate().is_err());
        }

        for field in 0..6 {
            let mut pack_document = document.clone();
            let generation = pack_document
                .backup_manifest
                .effective_epoch
                .effective_bundle
                .domain_pack_generation
                .as_mut()
                .unwrap();
            match field {
                0 => generation.active_lock_digest.clear(),
                1 => generation.composition_digest.clear(),
                2 => generation.base_core_bundle_digest.clear(),
                3 => generation.supply_chain_registry_digest.clear(),
                4 => generation.reviewer_registry_digest.clear(),
                _ => generation.reviewed_registry_digest.clear(),
            }
            recompute(&mut pack_document);
            assert!(pack_document.validate().is_err());
        }

        let mut anchor = document.clone();
        anchor
            .backup_manifest
            .external_authorities
            .replay_rollback_anchor
            .replay_wal_manifest_digest = format!("sha256:{}", "f".repeat(64));
        recompute(&mut anchor);
        assert!(anchor.validate().is_err());
        let mut lock_order = document;
        lock_order.backup_manifest.snapshot.lock_order.swap(4, 5);
        recompute(&mut lock_order);
        assert!(matches!(
            lock_order.validate(),
            Err(BackupManifestValidationError::InvalidSnapshotProtocol)
        ));
    }
}
