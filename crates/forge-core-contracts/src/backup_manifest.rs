//! Fail-closed, source-derived backup integrity and continuity contract.
//!
//! This S03 module defines a closed classifier and manifest invariants only.
//! Authenticity, restore I/O, retained locks, and opaque trusted receipts are
//! intentionally deferred to C2-S04; no caller-forgeable success capability is exposed.

use crate::{
    ProjectLinkDocument, WorkflowEffectiveBundleIdentity, WorkflowGovernanceReleaseIdentity,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};

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
    pub project: BackupProjectBinding,
    pub workflow_release: WorkflowGovernanceReleaseIdentity,
    pub effective_epoch: BackupEffectiveEpochBinding,
    pub source_state: BackupSourceState,
    pub snapshot_protocol: BackupSnapshotProtocol,
    pub entries: Vec<BackupEntry>,
    /// Observations are integrity-bound but are not trust roots. Restore compares
    /// them with independently supplied trusted expectations.
    pub external_authority_observations: BackupExternalAuthorityObservations,
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
    /// SHA-256 of the exact archived Project Link bytes, not a reserialization.
    pub project_link_sha256: String,
    pub archive_layout: BackupArchiveLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupArchiveLayout {
    pub project_link_archive_path: String,
    pub sidecar_archive_root: String,
    pub state_root_relative_to_sidecar: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupEffectiveEpochBinding {
    pub epoch_id: String,
    pub epoch_generation: u64,
    pub effective_bundle: WorkflowEffectiveBundleIdentity,
    pub governance_ledger_head_digest: String,
}

/// Typed projection of healthy source states. Counts and closures are derived
/// from parsed source stores under the snapshot boundary, never from archive names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupSourceState {
    pub project_state: BackupProjectState,
    pub claim_store: BackupClaimStoreState,
    pub workflow_governance_store: BackupInitializationState,
    pub workflow_action_replay_store: BackupInitializationState,
    pub effect_store: BackupEffectStoreState,
    pub memory_store: BackupInitializationState,
    pub research_store: BackupInitializationState,
    pub governance_conflict_store: BackupInitializationState,
    pub domain_pack_store: BackupDomainPackStoreState,
    pub domain_pack_operator_sources: Option<BackupDomainPackOperatorSourcesProjection>,
    pub domain_pack_learning_store: BackupDomainPackLearningStoreState,
    pub isolation_store: BackupIsolationStoreState,
    pub domain_pack_supply_chain_anchor: BackupProvisioningState,
    pub domain_pack_reviewed_learning_anchor: BackupProvisioningState,
    pub workflow_principal_registry: BackupProvisioningState,
    pub workflow_broker_registry: BackupProvisioningState,
    /// Exact state-root-relative outputs reconstructed from committed effect WAL/index records.
    pub declared_effect_outputs: Vec<BackupDeclaredEffectOutput>,
    pub public_sidecars: BackupPublicSidecarCounts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupProjectState {
    InitializedBeforeStart,
    StartedWithStateYaml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupInitializationState {
    BeforeInitialization,
    Initialized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupProvisioningState {
    NotProvisioned,
    Provisioned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum BackupClaimStoreState {
    EmptyBeforeFirstClaim,
    Active { rotation_generations: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum BackupEffectStoreState {
    EmptyBeforeFirstEffect,
    Active { compaction_manifest_present: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum BackupDomainPackStoreState {
    NoActiveGeneration {
        operator_sources_present: bool,
        rebase_plan_present: bool,
    },
    Active {
        operator_sources_present: bool,
        rebase_plan_present: bool,
        active_generation: u64,
        generations: Vec<BackupDomainPackGeneration>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupDomainPackGeneration {
    pub generation: u64,
    pub record_digest: String,
    pub receipt_digest: String,
    pub object_raw_digests: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupDomainPackOperatorSourcesProjection {
    pub schema_version: String,
    pub file_sha256: String,
    pub operator_root_identity: String,
    pub trust_policy_file: String,
    pub trust_policy_file_sha256: String,
    pub registry_file: String,
    pub registry_file_sha256: String,
    pub reviewer_registry_file: String,
    pub reviewer_registry_file_sha256: String,
    pub reviewed_registry_file: String,
    pub reviewed_registry_file_sha256: String,
    pub capability_registry_file: String,
    pub capability_registry_file_sha256: String,
    pub sandbox_policy_file: String,
    pub sandbox_policy_file_sha256: String,
    pub artifact_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum BackupDomainPackLearningStoreState {
    BeforeFirstCapture,
    Captured {
        records: Vec<BackupDomainPackLearningRecord>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupDomainPackLearningRecord {
    pub candidate_id: String,
    pub candidate_digest: String,
    pub raw_sha256: String,
    pub object_relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum BackupIsolationStoreState {
    Empty,
    Contracts {
        contracts: Vec<BackupIsolationContractProjection>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupIsolationContractProjection {
    pub isolation_id: String,
    pub agent_id: String,
    pub contract_relative_path: String,
    pub contract_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupDeclaredEffectOutput {
    /// Source-derived identity copied only from a parsed metadata-index record.
    pub operation_id: String,
    pub effect_id: String,
    pub target_kind: BackupDeclaredEffectTargetKind,
    /// Exact spelling emitted by the store; normalized archive identity is separate.
    pub physical_ref: String,
    pub logical_ref: String,
    pub state_relative_path: String,
    pub access_mode: BackupEffectOutputAccessMode,
    pub byte_length: u64,
    /// Accepted only when `verify_effect_metadata_index_bytes` finds these exact bytes.
    pub metadata_record_sha256: String,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupDeclaredEffectTargetKind {
    FilePath,
    ArtifactId,
    EvidenceId,
    LedgerStream,
    RequestStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupEffectOutputAccessMode {
    Write,
    Append,
    Create,
}

/// Exact source evidence returned by the metadata-index parser, never manifest input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupParsedEffectOutput {
    pub operation_id: String,
    pub effect_id: String,
    pub target_kind: BackupDeclaredEffectTargetKind,
    pub logical_ref: String,
    pub physical_ref: String,
    pub state_relative_path: String,
    pub access_mode: BackupEffectOutputAccessMode,
    pub content_sha256: String,
    pub byte_length: u64,
    pub source_record_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupPublicSidecarCounts {
    pub claim_cache_files: u64,
    pub official_handoff_artifacts: u64,
    pub artifacts: u64,
    pub evidence: u64,
    pub snapshots: u64,
    pub ledger_streams: u64,
    pub request_streams: u64,
    pub runtime_snapshots: u64,
    pub story_state: u64,
    pub agent_registry_state: u64,
    pub preflight_profiles: u64,
    pub effect_metadata_indexes: u64,
    pub trace_logs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupSnapshotProtocol {
    pub mode: BackupSnapshotMode,
    pub lock_order: Vec<BackupLockScope>,
    pub unlocked_producer_boundary: BackupUnlockedProducerBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupSnapshotMode {
    CooperativeLocksWithProducerQuiescenceAndStableEnumeration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupUnlockedProducerBoundary {
    OpaqueExclusiveQuiescenceRequiredByRestoreEngine,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BackupLockScope {
    ExternalDomainPackSupplyChainAnchor,
    ExternalDomainPackReviewedLearningAnchor,
    DomainPackOperatorSources,
    DomainPackRebasePlan,
    DomainPackLifecycle,
    DomainPackLearningCapture,
    WorkflowCredentialRegistry,
    WorkflowBrokerRegistry,
    WorkflowGovernance,
    ClaimCacheMutation,
    ClaimWal,
    WorkflowActionReplayWal,
    MemoryLog,
    ResearchLog,
    GovernanceConflictLog,
    IsolationContracts,
    CommandEvidenceAppend,
    EffectMetadataIndexAppend,
    TraceAppend,
    EffectWal,
    ReplayWal,
    ExternalReplayAnchor,
}

impl BackupLockScope {
    pub const fn relative_path(self) -> &'static str {
        match self {
            Self::ExternalDomainPackSupplyChainAnchor => {
                "<operator-root>/.forge-domain-pack-registry-anchor.lock"
            }
            Self::ExternalDomainPackReviewedLearningAnchor => {
                "<operator-root>/.forge-domain-pack-learning-anchor.lock"
            }
            Self::DomainPackOperatorSources => "locks/domain-packs.operator-sources.lock",
            Self::DomainPackRebasePlan => "locks/domain-packs.rebase-plan.lock",
            Self::DomainPackLifecycle => "locks/domain-packs.lifecycle.lock",
            Self::DomainPackLearningCapture => "domain-pack-learning/capture.lock",
            Self::WorkflowCredentialRegistry => "<operator-root>/.workflow-credential.lock",
            Self::WorkflowBrokerRegistry => "<operator-root>/.workflow-broker.lock",
            Self::WorkflowGovernance => "locks/workflow-governance.lock",
            Self::ClaimCacheMutation => "claims-active/.forge-claim.lock",
            Self::ClaimWal => "locks/claims.wal.lock",
            Self::WorkflowActionReplayWal => "locks/workflow-action-replay.lock",
            Self::MemoryLog => "locks/memory.log.lock",
            Self::ResearchLog => "locks/research.sources.lock",
            Self::GovernanceConflictLog => "locks/governance.conflicts.lock",
            Self::IsolationContracts => "contracts/isolations/.forge-isolation.lock",
            Self::CommandEvidenceAppend => "locks/append-json-line/<command-evidence-hash>.lock",
            Self::EffectMetadataIndexAppend => "locks/append-json-line/<effect-index-hash>.lock",
            Self::TraceAppend => "locks/append-json-line/<trace-hash>.lock",
            Self::EffectWal => "locks/effects.lock",
            Self::ReplayWal => "locks/replay.wal.lock",
            Self::ExternalReplayAnchor => "<protected-replay-anchor>.lock",
        }
    }
}

/// Compatible with shipped nested acquisitions: supply -> reviewed ->
/// operator-source/rebase -> lifecycle -> workflow, claim-cache -> claim-WAL ->
/// claim-cache atomic mutation, and effect -> replay.
pub const BACKUP_LOCK_ORDER: &[BackupLockScope] = &[
    BackupLockScope::ExternalDomainPackSupplyChainAnchor,
    BackupLockScope::ExternalDomainPackReviewedLearningAnchor,
    BackupLockScope::DomainPackOperatorSources,
    BackupLockScope::DomainPackRebasePlan,
    BackupLockScope::DomainPackLifecycle,
    BackupLockScope::DomainPackLearningCapture,
    BackupLockScope::WorkflowCredentialRegistry,
    BackupLockScope::WorkflowBrokerRegistry,
    BackupLockScope::WorkflowGovernance,
    BackupLockScope::ClaimCacheMutation,
    BackupLockScope::ClaimWal,
    BackupLockScope::WorkflowActionReplayWal,
    BackupLockScope::MemoryLog,
    BackupLockScope::ResearchLog,
    BackupLockScope::GovernanceConflictLog,
    BackupLockScope::IsolationContracts,
    BackupLockScope::CommandEvidenceAppend,
    BackupLockScope::EffectMetadataIndexAppend,
    BackupLockScope::EffectWal,
    BackupLockScope::ReplayWal,
    BackupLockScope::TraceAppend,
    BackupLockScope::ExternalReplayAnchor,
];

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BackupEntryKind {
    ProjectLink,
    ProjectState,
    RootLedger,
    ReplayWalManifest,
    ReplayWal,
    WorkflowGovernanceWal,
    ClaimWal,
    ClaimWalManifest,
    ClaimWalSnapshot,
    ClaimWalArchive,
    WorkflowActionReplayManifest,
    WorkflowActionReplayWal,
    EffectWal,
    EffectWalCompactionManifest,
    MemoryEventLog,
    ResearchEventLog,
    GovernanceConflictEventLog,
    DomainPackOperatorSources,
    DomainPackRebasePlan,
    DomainPackActivePointer,
    DomainPackLedgerRecord,
    DomainPackGenerationManifest,
    DomainPackGenerationLock,
    DomainPackGenerationPreflight,
    DomainPackGenerationCompatibility,
    DomainPackGenerationReceipt,
    DomainPackGenerationResolutionRequest,
    DomainPackGenerationCompositionRequest,
    DomainPackGenerationTrustInput,
    DomainPackPublishedReceipt,
    DomainPackObject,
    DomainPackLearningIndex,
    DomainPackLearningObject,
    IsolationContract,
    PublicPrincipalRegistry,
    PublicBrokerRegistry,
    ClaimCache,
    OfficialHandoffArtifact,
    Artifact,
    Evidence,
    Snapshot,
    LedgerStream,
    RequestStream,
    RuntimeSnapshot,
    StoryState,
    AgentRegistryState,
    DeclaredEffectOutput,
    PreflightProfile,
    EffectMetadataIndex,
    TraceLog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupArchiveEntryType {
    RegularFile,
    Symlink,
    Hardlink,
    Directory,
    Fifo,
    BlockDevice,
    CharacterDevice,
    Socket,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupEntry {
    pub material: BackupEntryKind,
    pub logical_path: String,
    pub entry_type: BackupArchiveEntryType,
    pub byte_length: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupExternalAuthorityObservations {
    pub replay_rollback_anchor: BackupReplayRollbackAnchor,
    pub domain_pack_supply_chain: Option<BackupDomainPackSupplyChainAuthority>,
    pub domain_pack_reviewed_learning: Option<BackupDomainPackLearningAuthority>,
    /// Archived public material; current-machine absence is legitimate and is not represented here.
    pub workflow_principal_registry: Option<BackupPublicRegistryMaterial>,
    pub workflow_broker_registry: Option<BackupPublicRegistryMaterial>,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupDomainPackSupplyChainAuthority {
    pub schema_version: String,
    pub operator_root_identity: String,
    pub registry_id: String,
    pub audience: String,
    pub generation: u64,
    pub anchor_document_sha256: String,
    pub registry_snapshot_digest: String,
    pub trust_policy_digest: String,
    pub registry_file_sha256: String,
    pub trust_policy_file_sha256: String,
    pub capability_registry_file_sha256: String,
    pub sandbox_policy_file_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupDomainPackLearningAuthority {
    pub schema_version: String,
    pub operator_root_identity: String,
    pub reviewer_registry_id: String,
    pub reviewer_audience: String,
    pub reviewer_generation: u64,
    pub reviewer_registry_digest: String,
    pub reviewed_registry_id: String,
    pub reviewed_audience: String,
    pub reviewed_generation: u64,
    pub reviewed_registry_digest: String,
    pub anchor_document_sha256: String,
    pub reviewer_registry_file_sha256: String,
    pub reviewed_registry_file_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupPublicRegistryMaterial {
    pub schema_version: String,
    pub audience: String,
    /// Exact raw SHA-256 is the sole normative identity of archived registry material.
    pub registry_sha256: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BackupForbiddenPrivateMaterial {
    BrokerPrivateKeys,
    WorkflowSecretRoots,
    OperatorSecretRoots,
    McpPrivateKeys,
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
    InvalidProjectLink,
    ProjectLinkEntryMismatch,
    InvalidLogicalPath {
        path: String,
    },
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
    EntriesNotCanonical,
    DuplicateEntryPath {
        path: String,
    },
    ImpossibleArchiveTopology {
        path: String,
    },
    SourceStateInventoryMismatch {
        material: BackupEntryKind,
    },
    InvalidDomainPackProjection,
    InvalidLearningProjection,
    InvalidIsolationProjection,
    InvalidEffectMetadataIndex,
    EffectMetadataIndexBytesMismatch,
    EffectOutputClosureMismatch,
    InvalidDeclaredEffectProjection,
    InvalidSnapshotProtocol,
    InvalidExternalAuthorities,
    PrivateMaterialPolicyMismatch,
    ManifestSetDigestMismatch,
    UnclassifiedSourceFile {
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupArchiveVerificationError {
    MissingEntry { path: String },
    ExtraEntry { path: String },
    DuplicateEntry { path: String },
    SubstitutedEntry { path: String },
    SymlinkOrNonRegularEntry { path: String },
    ProjectLinkBytesMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupSourceExclusion {
    ProducerLock,
    CrashRecoveryArtifact,
    IncompleteDomainPackStaging,
    ForbiddenPrivateMaterial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupSourceFileClassification {
    Archive(BackupEntryKind),
    Exclude(BackupSourceExclusion),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupSourceFileMetadata {
    pub logical_path: String,
    pub entry_type: BackupArchiveEntryType,
    pub hard_link_count: u64,
    pub byte_length: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EffectMetadataRecordKind {
    EffectTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EffectMetadataTargetKind {
    FilePath,
    Glob,
    StateKey,
    ArtifactId,
    EvidenceId,
    LedgerStream,
    RequestStream,
    CompletionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EffectMetadataAccessMode {
    Read,
    Write,
    Append,
    Create,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EffectMetadataActorRole {
    Driver,
    Worker,
    Human,
    Runtime,
    Unknown,
}

/// Wire-compatible mirror of `forge-core-store::EffectTargetMetadataRecord`.
/// The store depends on this crate, so mirroring here avoids a dependency cycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectMetadataRecord {
    schema_version: String,
    record_kind: EffectMetadataRecordKind,
    recorded_at: Option<String>,
    operation_id: String,
    effect_id: String,
    logical_ref: String,
    physical_ref: String,
    target_kind: EffectMetadataTargetKind,
    access_mode: EffectMetadataAccessMode,
    content_hash: Option<String>,
    byte_len: u64,
    actor_agent_id: String,
    actor_role: EffectMetadataActorRole,
    destructive: bool,
    redaction_hint: String,
}

#[derive(Debug, Clone)]
struct IndexedEffectMetadataRecord {
    record: EffectMetadataRecord,
    source_record_sha256: String,
}

impl BackupManifestDocument {
    /// Internal archive integrity only. A successful result does not prove
    /// authenticity, freshness, or resistance to whole-set substitution.
    pub fn validate_integrity(&self) -> Result<(), BackupManifestValidationError> {
        if self.schema_version != BACKUP_MANIFEST_SCHEMA_VERSION {
            return Err(BackupManifestValidationError::UnsupportedSchemaVersion);
        }
        self.backup_manifest.validate_integrity(self)
    }

    /// Backward-compatible name with deliberately integrity-only semantics.
    pub fn validate(&self) -> Result<(), BackupManifestValidationError> {
        self.validate_integrity()
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

    pub fn verify_project_link_bytes(
        &self,
        raw: &[u8],
        parsed_from_raw: &ProjectLinkDocument,
    ) -> Result<(), BackupArchiveVerificationError> {
        let digest = sha256(raw);
        if digest != self.backup_manifest.project.project_link_sha256
            || parsed_from_raw != &self.backup_manifest.project.project_link
        {
            return Err(BackupArchiveVerificationError::ProjectLinkBytesMismatch);
        }
        Ok(())
    }

    /// Validate source filesystem metadata before constructing an archive
    /// member. `hard_link_count` must come from no-follow metadata (for example
    /// Unix `st_nlink`); tar entry tags alone cannot detect a regular hardlink.
    pub fn verify_filesystem_entry_class(
        path: &str,
        entry_type: BackupArchiveEntryType,
        hard_link_count: u64,
    ) -> Result<(), BackupArchiveVerificationError> {
        if entry_type != BackupArchiveEntryType::RegularFile || hard_link_count != 1 {
            return Err(BackupArchiveVerificationError::SymlinkOrNonRegularEntry {
                path: path.to_owned(),
            });
        }
        Ok(())
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
    /// Classify one no-follow regular file from the complete source scan.
    /// Unknown files fail closed; lock/private/crash protocol files are explicit exclusions.
    pub fn classify_source_file(
        &self,
        source_path: &str,
    ) -> Result<BackupSourceFileClassification, BackupManifestValidationError> {
        let archive_path = canonical_archive_path(source_path)?;
        let normalized_source = normalized_utf8_repo_relative(source_path).ok_or_else(|| {
            BackupManifestValidationError::InvalidLogicalPath {
                path: source_path.to_owned(),
            }
        })?;
        let manifest = &self.backup_manifest;
        let state = normalized_utf8_repo_relative(&state_prefix(&manifest.project.archive_layout))
            .ok_or(BackupManifestValidationError::InvalidProjectLink)?;
        // Exclusions are security policy, not a fallback. They must win even when a
        // caller maliciously lists a broad-family descendant in the manifest.
        if is_forbidden_private_path(&normalized_source) {
            return Ok(BackupSourceFileClassification::Exclude(
                BackupSourceExclusion::ForbiddenPrivateMaterial,
            ));
        }
        if is_explicit_lock_path(&normalized_source, &state) {
            return Ok(BackupSourceFileClassification::Exclude(
                BackupSourceExclusion::ProducerLock,
            ));
        }
        if is_crash_protocol_path(&normalized_source) {
            return Ok(BackupSourceFileClassification::Exclude(
                BackupSourceExclusion::CrashRecoveryArtifact,
            ));
        }
        if is_domain_pack_staging_path(&normalized_source, &state) {
            return Ok(BackupSourceFileClassification::Exclude(
                BackupSourceExclusion::IncompleteDomainPackStaging,
            ));
        }
        if let Some(entry) = manifest
            .entries
            .iter()
            .find(|entry| entry.logical_path == archive_path)
        {
            return Ok(BackupSourceFileClassification::Archive(entry.material));
        }
        Err(BackupManifestValidationError::UnclassifiedSourceFile {
            path: source_path.to_owned(),
        })
    }

    /// Compare a complete source enumeration with the manifest. C2-S04 must supply
    /// this metadata directly from no-follow OS I/O; this function mints no capability.
    pub fn verify_source_enumeration(
        &self,
        observed: &[BackupSourceFileMetadata],
    ) -> Result<(), BackupManifestValidationError> {
        self.validate_integrity()?;
        let mut archived = BTreeMap::new();
        let mut all_identities = BTreeSet::new();
        for file in observed {
            let archive_path = canonical_archive_path(&file.logical_path)?;
            if file.entry_type != BackupArchiveEntryType::RegularFile
                || file.hard_link_count != 1
                || !all_identities.insert(archive_path.clone())
            {
                return Err(BackupManifestValidationError::UnclassifiedSourceFile {
                    path: file.logical_path.clone(),
                });
            }
            match self.classify_source_file(&file.logical_path)? {
                BackupSourceFileClassification::Archive(material) => {
                    archived.insert(
                        archive_path,
                        (material, file.byte_length, file.sha256.as_str()),
                    );
                }
                BackupSourceFileClassification::Exclude(_) => {}
            }
        }
        for entry in &self.backup_manifest.entries {
            let Some((material, byte_length, digest_value)) = archived.get(&entry.logical_path)
            else {
                return Err(
                    BackupManifestValidationError::SourceStateInventoryMismatch {
                        material: entry.material,
                    },
                );
            };
            if *material != entry.material
                || *byte_length != entry.byte_length
                || *digest_value != entry.sha256
            {
                return Err(
                    BackupManifestValidationError::SourceStateInventoryMismatch {
                        material: entry.material,
                    },
                );
            }
        }
        Ok(())
    }

    /// Parse the exact archived effect metadata-index bytes and prove that the manifest's
    /// declared file outputs are exactly the latest live `file_path` records.
    pub fn verify_effect_metadata_index_bytes(
        &self,
        archived_index_bytes: &[u8],
    ) -> Result<Vec<BackupParsedEffectOutput>, BackupManifestValidationError> {
        self.validate_integrity()?;
        let index_entry = self
            .backup_manifest
            .entries
            .iter()
            .find(|entry| entry.material == BackupEntryKind::EffectMetadataIndex);
        let Some(index_entry) = index_entry else {
            if archived_index_bytes.is_empty()
                && self
                    .backup_manifest
                    .source_state
                    .declared_effect_outputs
                    .is_empty()
            {
                return Ok(Vec::new());
            }
            return Err(BackupManifestValidationError::EffectMetadataIndexBytesMismatch);
        };
        if index_entry.byte_length != archived_index_bytes.len() as u64
            || index_entry.sha256 != sha256(archived_index_bytes)
        {
            return Err(BackupManifestValidationError::EffectMetadataIndexBytesMismatch);
        }
        let parsed = parse_effect_metadata_index(archived_index_bytes, &self.backup_manifest)?;
        let projected = parsed
            .iter()
            .map(|output| BackupDeclaredEffectOutput {
                operation_id: output.operation_id.clone(),
                effect_id: output.effect_id.clone(),
                target_kind: output.target_kind,
                physical_ref: output.physical_ref.clone(),
                logical_ref: output.logical_ref.clone(),
                state_relative_path: output.state_relative_path.clone(),
                access_mode: output.access_mode,
                byte_length: output.byte_length,
                metadata_record_sha256: output.source_record_sha256.clone(),
                content_sha256: output.content_sha256.clone(),
            })
            .collect::<Vec<_>>();
        if projected != self.backup_manifest.source_state.declared_effect_outputs {
            return Err(BackupManifestValidationError::EffectOutputClosureMismatch);
        }
        Ok(parsed)
    }
}

impl BackupManifest {
    fn validate_integrity(
        &self,
        document: &BackupManifestDocument,
    ) -> Result<(), BackupManifestValidationError> {
        if self.format != BackupManifestFormat::ForgeProjectStateBackupV1 {
            return Err(BackupManifestValidationError::UnsupportedManifestFormat);
        }
        validate_project(self)?;
        validate_release(&self.workflow_release)?;
        validate_effective_epoch(&self.effective_epoch)?;
        if self.snapshot_protocol.mode
            != BackupSnapshotMode::CooperativeLocksWithProducerQuiescenceAndStableEnumeration
            || self.snapshot_protocol.lock_order.as_slice() != BACKUP_LOCK_ORDER
            || self.snapshot_protocol.unlocked_producer_boundary
                != BackupUnlockedProducerBoundary::OpaqueExclusiveQuiescenceRequiredByRestoreEngine
        {
            return Err(BackupManifestValidationError::InvalidSnapshotProtocol);
        }

        let mut previous = None;
        let mut paths = BTreeSet::new();
        for entry in &self.entries {
            validate_canonical_archive_path(&entry.logical_path)?;
            let source_identity = decode_canonical_archive_path(&entry.logical_path)?;
            if is_forbidden_private_path(&source_identity) {
                return Err(BackupManifestValidationError::ForbiddenPrivatePath {
                    path: entry.logical_path.clone(),
                });
            }
            if entry.entry_type != BackupArchiveEntryType::RegularFile {
                return Err(BackupManifestValidationError::NonRegularArchiveEntry {
                    path: entry.logical_path.clone(),
                });
            }
            validate_entry_path(entry, self)?;
            digest("entries[].sha256", &entry.sha256)?;
            let key = (entry.material, entry.logical_path.as_str());
            if previous.is_some_and(|prior| prior >= key) {
                return Err(BackupManifestValidationError::EntriesNotCanonical);
            }
            previous = Some(key);
            if !paths.insert(&entry.logical_path) {
                return Err(BackupManifestValidationError::DuplicateEntryPath {
                    path: entry.logical_path.clone(),
                });
            }
        }
        let mut paths_by_name = paths.into_iter().collect::<Vec<_>>();
        paths_by_name.sort_unstable();
        for pair in paths_by_name.windows(2) {
            if pair[1].starts_with(&format!("{}/", pair[0])) {
                return Err(BackupManifestValidationError::ImpossibleArchiveTopology {
                    path: pair[0].clone(),
                });
            }
        }
        validate_source_inventory(self)?;
        validate_external_authorities(self)?;
        if self.forbidden_private_material
            != [
                BackupForbiddenPrivateMaterial::BrokerPrivateKeys,
                BackupForbiddenPrivateMaterial::WorkflowSecretRoots,
                BackupForbiddenPrivateMaterial::OperatorSecretRoots,
                BackupForbiddenPrivateMaterial::McpPrivateKeys,
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

fn validate_project(manifest: &BackupManifest) -> Result<(), BackupManifestValidationError> {
    let project = &manifest.project;
    required("project.project_id", &project.project_link.project_id.0)?;
    if project.project_link.schema_version != crate::PROJECT_LINK_SCHEMA_VERSION
        || leaf(&project.project_link.state_root.0) != Some(".forge-method")
    {
        return Err(BackupManifestValidationError::InvalidProjectLink);
    }
    digest("project.project_link_sha256", &project.project_link_sha256)?;
    let layout = &project.archive_layout;
    if layout.project_link_archive_path != PROJECT_LINK_ARCHIVE_PATH
        || layout.sidecar_archive_root != "sidecar"
        || layout.state_root_relative_to_sidecar != ".forge-method"
        || normalized_relative(
            &project.project_link.state_root.0,
            &project.project_link.sidecar_root.0,
        )
        .as_deref()
            != Some(".forge-method")
    {
        return Err(BackupManifestValidationError::InvalidProjectLink);
    }
    let link = manifest
        .entries
        .iter()
        .find(|entry| entry.material == BackupEntryKind::ProjectLink);
    if link.is_none_or(|entry| {
        entry.logical_path != PROJECT_LINK_ARCHIVE_PATH
            || entry.sha256 != project.project_link_sha256
    }) {
        return Err(BackupManifestValidationError::ProjectLinkEntryMismatch);
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
    required(
        "core_runtime_bundle.bundle_id",
        &bundle.core_runtime_bundle.bundle_id.0,
    )?;
    required(
        "effective_runtime_bundle.bundle_id",
        &bundle.effective_runtime_bundle.bundle_id.0,
    )?;
    for (field, value) in [
        (
            "core.bundle_digest",
            &bundle.core_runtime_bundle.bundle_digest,
        ),
        (
            "core.policy_set_digest",
            &bundle.core_runtime_bundle.policy_set_digest,
        ),
        (
            "effective.bundle_digest",
            &bundle.effective_runtime_bundle.bundle_digest,
        ),
        (
            "effective.policy_set_digest",
            &bundle.effective_runtime_bundle.policy_set_digest,
        ),
        (
            "effective.receipt_context_digest",
            &bundle.receipt_context_digest,
        ),
    ] {
        digest(field, value)?;
    }
    if let Some(pack) = &bundle.domain_pack_generation {
        if pack.generation == 0 {
            return Err(BackupManifestValidationError::InvalidDomainPackProjection);
        }
        for value in [
            &pack.active_lock_digest,
            &pack.composition_digest,
            &pack.base_core_bundle_digest,
            &pack.supply_chain_registry_digest,
            &pack.reviewer_registry_digest,
            &pack.reviewed_registry_digest,
        ] {
            digest("effective.domain_pack_digest", value)?;
        }
    }
    Ok(())
}

fn validate_source_inventory(
    manifest: &BackupManifest,
) -> Result<(), BackupManifestValidationError> {
    let mut expected = BTreeMap::<BackupEntryKind, u64>::new();
    {
        let mut require = |kind, count| {
            expected.insert(kind, count);
        };
        require(BackupEntryKind::ProjectLink, 1);
        require(BackupEntryKind::RootLedger, 1);
        require(BackupEntryKind::ReplayWalManifest, 1);
        require(BackupEntryKind::ReplayWal, 1);
        require(
            BackupEntryKind::ProjectState,
            u64::from(
                manifest.source_state.project_state == BackupProjectState::StartedWithStateYaml,
            ),
        );
        require(
            BackupEntryKind::WorkflowGovernanceWal,
            u64::from(
                manifest.source_state.workflow_governance_store
                    == BackupInitializationState::Initialized,
            ),
        );
        let (claim_wal, rotations) = match manifest.source_state.claim_store {
            BackupClaimStoreState::EmptyBeforeFirstClaim => (0, 0),
            BackupClaimStoreState::Active {
                rotation_generations,
            } => (1, rotation_generations),
        };
        require(BackupEntryKind::ClaimWal, claim_wal);
        require(BackupEntryKind::ClaimWalManifest, u64::from(rotations > 0));
        require(BackupEntryKind::ClaimWalSnapshot, rotations);
        require(BackupEntryKind::ClaimWalArchive, rotations);
        let action_initialized = manifest.source_state.workflow_action_replay_store
            == BackupInitializationState::Initialized;
        require(
            BackupEntryKind::WorkflowActionReplayManifest,
            u64::from(action_initialized),
        );
        require(
            BackupEntryKind::WorkflowActionReplayWal,
            u64::from(action_initialized),
        );
        match manifest.source_state.effect_store {
            BackupEffectStoreState::EmptyBeforeFirstEffect => {
                require(BackupEntryKind::EffectWal, 0);
                require(BackupEntryKind::EffectWalCompactionManifest, 0);
            }
            BackupEffectStoreState::Active {
                compaction_manifest_present,
            } => {
                require(BackupEntryKind::EffectWal, 1);
                require(
                    BackupEntryKind::EffectWalCompactionManifest,
                    u64::from(compaction_manifest_present),
                );
            }
        }
        require(
            BackupEntryKind::MemoryEventLog,
            u64::from(manifest.source_state.memory_store == BackupInitializationState::Initialized),
        );
        require(
            BackupEntryKind::ResearchEventLog,
            u64::from(
                manifest.source_state.research_store == BackupInitializationState::Initialized,
            ),
        );
        require(
            BackupEntryKind::GovernanceConflictEventLog,
            u64::from(
                manifest.source_state.governance_conflict_store
                    == BackupInitializationState::Initialized,
            ),
        );
        require(
            BackupEntryKind::PublicPrincipalRegistry,
            u64::from(
                manifest.source_state.workflow_principal_registry
                    == BackupProvisioningState::Provisioned,
            ),
        );
        require(
            BackupEntryKind::PublicBrokerRegistry,
            u64::from(
                manifest.source_state.workflow_broker_registry
                    == BackupProvisioningState::Provisioned,
            ),
        );
    }
    validate_domain_pack_inventory(manifest, &mut expected)?;
    validate_learning_inventory(manifest, &mut expected)?;
    validate_isolation_inventory(manifest, &mut expected)?;
    validate_declared_effect_outputs(manifest, &mut expected)?;
    let sidecars = &manifest.source_state.public_sidecars;
    for (kind, count) in [
        (
            BackupEntryKind::PreflightProfile,
            sidecars.preflight_profiles,
        ),
        (
            BackupEntryKind::EffectMetadataIndex,
            sidecars.effect_metadata_indexes,
        ),
        (BackupEntryKind::TraceLog, sidecars.trace_logs),
    ] {
        if count > 1 {
            return Err(
                BackupManifestValidationError::SourceStateInventoryMismatch { material: kind },
            );
        }
    }
    for (kind, count) in [
        (BackupEntryKind::ClaimCache, sidecars.claim_cache_files),
        (
            BackupEntryKind::OfficialHandoffArtifact,
            sidecars.official_handoff_artifacts,
        ),
        (BackupEntryKind::Artifact, sidecars.artifacts),
        (BackupEntryKind::Evidence, sidecars.evidence),
        (BackupEntryKind::Snapshot, sidecars.snapshots),
        (BackupEntryKind::LedgerStream, sidecars.ledger_streams),
        (BackupEntryKind::RequestStream, sidecars.request_streams),
        (BackupEntryKind::RuntimeSnapshot, sidecars.runtime_snapshots),
        (BackupEntryKind::StoryState, sidecars.story_state),
        (
            BackupEntryKind::AgentRegistryState,
            sidecars.agent_registry_state,
        ),
        (
            BackupEntryKind::PreflightProfile,
            sidecars.preflight_profiles,
        ),
        (
            BackupEntryKind::EffectMetadataIndex,
            sidecars.effect_metadata_indexes,
        ),
        (BackupEntryKind::TraceLog, sidecars.trace_logs),
    ] {
        expected.insert(kind, count);
    }
    let mut actual = BTreeMap::<BackupEntryKind, u64>::new();
    for entry in &manifest.entries {
        *actual.entry(entry.material).or_default() += 1;
    }
    for kind in all_entry_kinds() {
        if actual.get(&kind).copied().unwrap_or_default()
            != expected.get(&kind).copied().unwrap_or_default()
        {
            return Err(
                BackupManifestValidationError::SourceStateInventoryMismatch { material: kind },
            );
        }
    }
    Ok(())
}

fn validate_domain_pack_inventory(
    manifest: &BackupManifest,
    expected: &mut BTreeMap<BackupEntryKind, u64>,
) -> Result<(), BackupManifestValidationError> {
    let (operator, rebase, active, generations) = match &manifest.source_state.domain_pack_store {
        BackupDomainPackStoreState::NoActiveGeneration {
            operator_sources_present,
            rebase_plan_present,
        } => {
            if *rebase_plan_present
                || manifest
                    .effective_epoch
                    .effective_bundle
                    .domain_pack_generation
                    .is_some()
            {
                return Err(BackupManifestValidationError::InvalidDomainPackProjection);
            }
            (*operator_sources_present, false, false, &[][..])
        }
        BackupDomainPackStoreState::Active {
            operator_sources_present,
            rebase_plan_present,
            active_generation,
            generations,
        } => {
            let effective_generation = manifest
                .effective_epoch
                .effective_bundle
                .domain_pack_generation
                .as_ref()
                .map(|generation| generation.generation);
            if generations.first().map(|generation| generation.generation) != Some(1)
                || generations.last().map(|generation| generation.generation)
                    != Some(*active_generation)
                || effective_generation != Some(*active_generation)
                || !generations
                    .windows(2)
                    .all(|pair| pair[0].generation.checked_add(1) == Some(pair[1].generation))
            {
                return Err(BackupManifestValidationError::InvalidDomainPackProjection);
            }
            (
                *operator_sources_present,
                *rebase_plan_present,
                true,
                generations.as_slice(),
            )
        }
    };
    if operator != manifest.source_state.domain_pack_operator_sources.is_some() {
        return Err(BackupManifestValidationError::InvalidDomainPackProjection);
    }
    if let Some(sources) = &manifest.source_state.domain_pack_operator_sources {
        if sources.schema_version != "forge-domain-pack-operator-sources-v1" {
            return Err(BackupManifestValidationError::InvalidDomainPackProjection);
        }
        for value in [
            &sources.operator_root_identity,
            &sources.trust_policy_file,
            &sources.registry_file,
            &sources.reviewer_registry_file,
            &sources.reviewed_registry_file,
            &sources.capability_registry_file,
            &sources.sandbox_policy_file,
            &sources.artifact_root,
        ] {
            required("domain_pack.operator_sources", value)?;
        }
        validate_digest_fields([
            &sources.file_sha256,
            &sources.trust_policy_file_sha256,
            &sources.registry_file_sha256,
            &sources.reviewer_registry_file_sha256,
            &sources.reviewed_registry_file_sha256,
            &sources.capability_registry_file_sha256,
            &sources.sandbox_policy_file_sha256,
        ])?;
        let path = format!(
            "{}/domain-packs/operator-sources.yaml",
            state_prefix(&manifest.project.archive_layout)
        );
        if !has_entry(manifest, BackupEntryKind::DomainPackOperatorSources, &path)
            || entry(manifest, BackupEntryKind::DomainPackOperatorSources)?.sha256
                != sources.file_sha256
        {
            return Err(BackupManifestValidationError::InvalidDomainPackProjection);
        }
    }
    expected.insert(
        BackupEntryKind::DomainPackOperatorSources,
        u64::from(operator),
    );
    expected.insert(BackupEntryKind::DomainPackRebasePlan, u64::from(rebase));
    expected.insert(BackupEntryKind::DomainPackActivePointer, u64::from(active));
    let count = generations.len() as u64;
    for kind in [
        BackupEntryKind::DomainPackLedgerRecord,
        BackupEntryKind::DomainPackGenerationManifest,
        BackupEntryKind::DomainPackGenerationLock,
        BackupEntryKind::DomainPackGenerationPreflight,
        BackupEntryKind::DomainPackGenerationCompatibility,
        BackupEntryKind::DomainPackGenerationReceipt,
        BackupEntryKind::DomainPackGenerationResolutionRequest,
        BackupEntryKind::DomainPackGenerationCompositionRequest,
        BackupEntryKind::DomainPackGenerationTrustInput,
        BackupEntryKind::DomainPackPublishedReceipt,
    ] {
        expected.insert(kind, count);
    }
    let mut objects = BTreeSet::new();
    let state = state_prefix(&manifest.project.archive_layout);
    for generation in generations {
        if generation.generation == 0 {
            return Err(BackupManifestValidationError::InvalidDomainPackProjection);
        }
        digest("domain_pack.record_digest", &generation.record_digest)?;
        digest("domain_pack.receipt_digest", &generation.receipt_digest)?;
        let record = digest_token(&generation.record_digest)?;
        let receipt = digest_token(&generation.receipt_digest)?;
        let root = format!(
            "{state}/domain-packs/generations/{:020}-{record}",
            generation.generation
        );
        for (kind, path) in [
            (
                BackupEntryKind::DomainPackLedgerRecord,
                format!("{state}/domain-packs/ledger/{record}.yaml"),
            ),
            (
                BackupEntryKind::DomainPackGenerationManifest,
                format!("{root}/generation.yaml"),
            ),
            (
                BackupEntryKind::DomainPackGenerationLock,
                format!("{root}/lock.yaml"),
            ),
            (
                BackupEntryKind::DomainPackGenerationPreflight,
                format!("{root}/preflight.yaml"),
            ),
            (
                BackupEntryKind::DomainPackGenerationCompatibility,
                format!("{root}/compatibility.yaml"),
            ),
            (
                BackupEntryKind::DomainPackGenerationReceipt,
                format!("{root}/receipt.yaml"),
            ),
            (
                BackupEntryKind::DomainPackGenerationResolutionRequest,
                format!("{root}/resolution-request.yaml"),
            ),
            (
                BackupEntryKind::DomainPackGenerationCompositionRequest,
                format!("{root}/composition-request.yaml"),
            ),
            (
                BackupEntryKind::DomainPackGenerationTrustInput,
                format!("{root}/trust-input.yaml"),
            ),
            (
                BackupEntryKind::DomainPackPublishedReceipt,
                format!("{state}/domain-packs/receipts/{receipt}.yaml"),
            ),
        ] {
            if !has_entry(manifest, kind, &path) {
                return Err(BackupManifestValidationError::InvalidDomainPackProjection);
            }
        }
        if generation
            .object_raw_digests
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(BackupManifestValidationError::InvalidDomainPackProjection);
        }
        for digest_value in &generation.object_raw_digests {
            digest("domain_pack.object_raw_digest", digest_value)?;
            objects.insert(digest_value.clone());
        }
    }
    expected.insert(BackupEntryKind::DomainPackObject, objects.len() as u64);
    for object in objects {
        let path = format!("{state}/domain-packs/objects/{}", digest_token(&object)?);
        let Some(entry) = manifest.entries.iter().find(|entry| {
            entry.material == BackupEntryKind::DomainPackObject && entry.logical_path == path
        }) else {
            return Err(BackupManifestValidationError::InvalidDomainPackProjection);
        };
        if entry.sha256 != object {
            return Err(BackupManifestValidationError::InvalidDomainPackProjection);
        }
    }
    Ok(())
}

fn validate_learning_inventory(
    manifest: &BackupManifest,
    expected: &mut BTreeMap<BackupEntryKind, u64>,
) -> Result<(), BackupManifestValidationError> {
    let state = state_prefix(&manifest.project.archive_layout);
    let records = match &manifest.source_state.domain_pack_learning_store {
        BackupDomainPackLearningStoreState::BeforeFirstCapture => {
            expected.insert(BackupEntryKind::DomainPackLearningIndex, 0);
            expected.insert(BackupEntryKind::DomainPackLearningObject, 0);
            return Ok(());
        }
        BackupDomainPackLearningStoreState::Captured { records } if !records.is_empty() => records,
        BackupDomainPackLearningStoreState::Captured { .. } => {
            return Err(BackupManifestValidationError::InvalidLearningProjection);
        }
    };
    expected.insert(BackupEntryKind::DomainPackLearningIndex, 1);
    expected.insert(
        BackupEntryKind::DomainPackLearningObject,
        records.len() as u64,
    );
    if !has_entry(
        manifest,
        BackupEntryKind::DomainPackLearningIndex,
        &format!("{state}/domain-pack-learning/index.json"),
    ) {
        return Err(BackupManifestValidationError::InvalidLearningProjection);
    }
    let mut candidate_ids = BTreeSet::new();
    let mut raw_digests = BTreeSet::new();
    let mut previous_id: Option<&str> = None;
    for record in records {
        required("domain_pack_learning.candidate_id", &record.candidate_id)?;
        digest(
            "domain_pack_learning.candidate_digest",
            &record.candidate_digest,
        )?;
        digest("domain_pack_learning.raw_sha256", &record.raw_sha256)?;
        if previous_id.is_some_and(|previous| previous >= record.candidate_id.as_str())
            || !candidate_ids.insert(record.candidate_id.as_str())
            || !raw_digests.insert(record.raw_sha256.as_str())
            || record.object_relative_path
                != format!(
                    "domain-pack-learning/objects/{}",
                    digest_token(&record.raw_sha256)?
                )
        {
            return Err(BackupManifestValidationError::InvalidLearningProjection);
        }
        previous_id = Some(&record.candidate_id);
        let path = format!("{state}/{}", record.object_relative_path);
        let Some(object) = manifest.entries.iter().find(|entry| {
            entry.material == BackupEntryKind::DomainPackLearningObject
                && entry.logical_path == path
        }) else {
            return Err(BackupManifestValidationError::InvalidLearningProjection);
        };
        if object.sha256 != record.raw_sha256 {
            return Err(BackupManifestValidationError::InvalidLearningProjection);
        }
    }
    Ok(())
}

fn validate_isolation_inventory(
    manifest: &BackupManifest,
    expected: &mut BTreeMap<BackupEntryKind, u64>,
) -> Result<(), BackupManifestValidationError> {
    let contracts = match &manifest.source_state.isolation_store {
        BackupIsolationStoreState::Empty => &[][..],
        BackupIsolationStoreState::Contracts { contracts } if !contracts.is_empty() => {
            contracts.as_slice()
        }
        BackupIsolationStoreState::Contracts { .. } => {
            return Err(BackupManifestValidationError::InvalidIsolationProjection);
        }
    };
    expected.insert(BackupEntryKind::IsolationContract, contracts.len() as u64);
    let state = state_prefix(&manifest.project.archive_layout);
    let mut ids = BTreeSet::new();
    let mut previous_path: Option<&str> = None;
    for contract in contracts {
        required("isolation.isolation_id", &contract.isolation_id)?;
        required("isolation.agent_id", &contract.agent_id)?;
        digest("isolation.contract_sha256", &contract.contract_sha256)?;
        let relative = contract.contract_relative_path.as_str();
        if !relative.starts_with("contracts/isolations/")
            || !relative.ends_with(".yaml")
            || relative["contracts/isolations/".len()..].contains('/')
            || previous_path.is_some_and(|previous| previous >= relative)
            || !ids.insert(contract.isolation_id.as_str())
        {
            return Err(BackupManifestValidationError::InvalidIsolationProjection);
        }
        previous_path = Some(relative);
        let path = format!("{state}/{relative}");
        let Some(entry) = manifest.entries.iter().find(|entry| {
            entry.material == BackupEntryKind::IsolationContract && entry.logical_path == path
        }) else {
            return Err(BackupManifestValidationError::InvalidIsolationProjection);
        };
        if entry.sha256 != contract.contract_sha256 {
            return Err(BackupManifestValidationError::InvalidIsolationProjection);
        }
    }
    Ok(())
}

fn validate_declared_effect_outputs(
    manifest: &BackupManifest,
    expected: &mut BTreeMap<BackupEntryKind, u64>,
) -> Result<(), BackupManifestValidationError> {
    let outputs = &manifest.source_state.declared_effect_outputs;
    let declared_count = outputs
        .iter()
        .filter(|output| {
            effect_output_material(manifest, &output.state_relative_path)
                == Some(BackupEntryKind::DeclaredEffectOutput)
        })
        .count() as u64;
    expected.insert(BackupEntryKind::DeclaredEffectOutput, declared_count);
    let state = state_prefix(&manifest.project.archive_layout);
    let mut previous: Option<&str> = None;
    let mut metadata_records = BTreeSet::new();
    let mut physical_identities = BTreeSet::new();
    for output in outputs {
        required("declared_effect.operation_id", &output.operation_id)?;
        required("declared_effect.effect_id", &output.effect_id)?;
        digest(
            "declared_effect.metadata_record_sha256",
            &output.metadata_record_sha256,
        )?;
        digest("declared_effect.content_sha256", &output.content_sha256)?;
        validate_canonical_archive_path(&output.state_relative_path)?;
        let metadata_kind = effect_metadata_target_kind(output.target_kind);
        let expected_physical_ref =
            expected_effect_physical_ref(metadata_kind, &output.logical_ref)
                .ok_or(BackupManifestValidationError::InvalidDeclaredEffectProjection)?;
        let expected_state_relative_path = expected_effect_state_relative_path(
            metadata_kind,
            &output.logical_ref,
            &manifest.project.archive_layout,
        )
        .ok_or(BackupManifestValidationError::InvalidDeclaredEffectProjection)?;
        if !is_live_effect_source_path_allowed(
            &expected_physical_ref,
            &manifest.project.archive_layout,
        ) {
            return Err(BackupManifestValidationError::InvalidDeclaredEffectProjection);
        }
        let Some(material) = effect_output_material(manifest, &output.state_relative_path) else {
            return Err(BackupManifestValidationError::InvalidDeclaredEffectProjection);
        };
        if output.physical_ref != expected_physical_ref
            || output.state_relative_path != expected_state_relative_path
            || previous.is_some_and(|path| path >= output.state_relative_path.as_str())
            || !physical_identities.insert(output.state_relative_path.as_str())
            || !metadata_records.insert(output.metadata_record_sha256.as_str())
        {
            return Err(BackupManifestValidationError::InvalidDeclaredEffectProjection);
        }
        previous = Some(&output.state_relative_path);
        let path = format!("{state}/{}", output.state_relative_path);
        let Some(entry) = manifest
            .entries
            .iter()
            .find(|entry| entry.material == material && entry.logical_path == path)
        else {
            return Err(BackupManifestValidationError::InvalidDeclaredEffectProjection);
        };
        if entry.sha256 != output.content_sha256 || entry.byte_length != output.byte_length {
            return Err(BackupManifestValidationError::InvalidDeclaredEffectProjection);
        }
    }
    Ok(())
}

fn effect_output_material(
    manifest: &BackupManifest,
    state_relative_path: &str,
) -> Option<BackupEntryKind> {
    let full_path = format!(
        "{}/{}",
        state_prefix(&manifest.project.archive_layout),
        state_relative_path
    );
    if let Some(entry) = manifest
        .entries
        .iter()
        .find(|entry| entry.logical_path == full_path)
    {
        return Some(entry.material);
    }
    if state_relative_path == "artifacts"
        || state_relative_path == "evidence"
        || state_relative_path == "snapshots"
        || state_relative_path == "ledger"
        || state_relative_path == "requests"
    {
        return Some(BackupEntryKind::DeclaredEffectOutput);
    }
    if state_relative_path.starts_with("artifacts/") {
        Some(BackupEntryKind::Artifact)
    } else if state_relative_path.starts_with("evidence/") {
        Some(BackupEntryKind::Evidence)
    } else if state_relative_path.starts_with("snapshots/") {
        Some(BackupEntryKind::Snapshot)
    } else if state_relative_path.starts_with("ledger/") {
        Some(BackupEntryKind::LedgerStream)
    } else if state_relative_path == "requests.ndjson"
        || state_relative_path.starts_with("requests/")
    {
        Some(BackupEntryKind::RequestStream)
    } else {
        None
    }
}

fn parse_effect_metadata_index(
    raw: &[u8],
    manifest: &BackupManifest,
) -> Result<Vec<BackupParsedEffectOutput>, BackupManifestValidationError> {
    if raw.is_empty() {
        return Err(BackupManifestValidationError::InvalidEffectMetadataIndex);
    }
    let mut latest = BTreeMap::<String, IndexedEffectMetadataRecord>::new();
    for frame in raw.split_inclusive(|byte| *byte == b'\n') {
        let Some(payload) = frame.strip_suffix(b"\n") else {
            return Err(BackupManifestValidationError::InvalidEffectMetadataIndex);
        };
        if payload.is_empty() || payload.ends_with(b"\r") {
            return Err(BackupManifestValidationError::InvalidEffectMetadataIndex);
        }
        let record: EffectMetadataRecord = serde_json::from_slice(payload)
            .map_err(|_| BackupManifestValidationError::InvalidEffectMetadataIndex)?;
        if serde_json::to_vec(&record).ok().as_deref() != Some(payload)
            || record.schema_version != "0.1"
            || record.record_kind != EffectMetadataRecordKind::EffectTarget
            || record.operation_id.trim().is_empty()
            || record.effect_id.trim().is_empty()
            || record.actor_agent_id.trim().is_empty()
            || record.redaction_hint != "raw_content_not_indexed"
        {
            return Err(BackupManifestValidationError::InvalidEffectMetadataIndex);
        }
        if expected_effect_physical_ref(record.target_kind, &record.logical_ref).as_deref()
            != Some(record.physical_ref.as_str())
        {
            return Err(BackupManifestValidationError::InvalidEffectMetadataIndex);
        }
        match record.access_mode {
            EffectMetadataAccessMode::Delete
                if record.content_hash.is_some() || record.byte_len != 0 =>
            {
                return Err(BackupManifestValidationError::InvalidEffectMetadataIndex);
            }
            EffectMetadataAccessMode::Write
            | EffectMetadataAccessMode::Append
            | EffectMetadataAccessMode::Create => {
                digest(
                    "effect_metadata.content_hash",
                    record.content_hash.as_deref().unwrap_or_default(),
                )?;
            }
            EffectMetadataAccessMode::Read => {
                return Err(BackupManifestValidationError::InvalidEffectMetadataIndex);
            }
            EffectMetadataAccessMode::Delete => {}
        }
        let key = format!(
            "{}:{}",
            effect_metadata_target_kind_key(record.target_kind),
            record.logical_ref
        );
        latest.insert(
            key,
            IndexedEffectMetadataRecord {
                record,
                source_record_sha256: sha256(payload),
            },
        );
    }

    let mut outputs = Vec::new();
    let mut paths = BTreeSet::new();
    for indexed in latest.into_values() {
        let record = indexed.record;
        if record.access_mode == EffectMetadataAccessMode::Delete {
            continue;
        }
        let Some(state_relative_path) = expected_effect_state_relative_path(
            record.target_kind,
            &record.logical_ref,
            &manifest.project.archive_layout,
        ) else {
            // Valid repository FilePath targets outside the state root are not backup members.
            continue;
        };
        if !is_live_effect_source_path_allowed(
            &record.physical_ref,
            &manifest.project.archive_layout,
        ) {
            return Err(BackupManifestValidationError::InvalidEffectMetadataIndex);
        }
        let target_kind = backup_effect_target_kind(record.target_kind)
            .expect("validated shipped file-backed metadata target");
        if effect_output_material(manifest, &state_relative_path).is_none() {
            return Err(BackupManifestValidationError::InvalidEffectMetadataIndex);
        }
        if !paths.insert(state_relative_path.clone()) {
            return Err(BackupManifestValidationError::InvalidEffectMetadataIndex);
        }
        let access_mode = match record.access_mode {
            EffectMetadataAccessMode::Write => BackupEffectOutputAccessMode::Write,
            EffectMetadataAccessMode::Append => BackupEffectOutputAccessMode::Append,
            EffectMetadataAccessMode::Create => BackupEffectOutputAccessMode::Create,
            EffectMetadataAccessMode::Read | EffectMetadataAccessMode::Delete => unreachable!(),
        };
        outputs.push(BackupParsedEffectOutput {
            operation_id: record.operation_id,
            target_kind,
            effect_id: record.effect_id,
            logical_ref: record.logical_ref,
            physical_ref: record.physical_ref,
            state_relative_path,
            access_mode,
            content_sha256: record.content_hash.expect("validated live content hash"),
            byte_length: record.byte_len,
            source_record_sha256: indexed.source_record_sha256,
        });
    }
    outputs.sort_by(|left, right| left.state_relative_path.cmp(&right.state_relative_path));
    Ok(outputs)
}

fn effect_metadata_target_kind_key(kind: EffectMetadataTargetKind) -> &'static str {
    match kind {
        EffectMetadataTargetKind::FilePath => "file_path",
        EffectMetadataTargetKind::Glob => "glob",
        EffectMetadataTargetKind::StateKey => "state_key",
        EffectMetadataTargetKind::ArtifactId => "artifact_id",
        EffectMetadataTargetKind::EvidenceId => "evidence_id",
        EffectMetadataTargetKind::LedgerStream => "ledger_stream",
        EffectMetadataTargetKind::RequestStream => "request_stream",
        EffectMetadataTargetKind::CompletionId => "completion_id",
    }
}

fn backup_effect_target_kind(
    kind: EffectMetadataTargetKind,
) -> Option<BackupDeclaredEffectTargetKind> {
    match kind {
        EffectMetadataTargetKind::FilePath => Some(BackupDeclaredEffectTargetKind::FilePath),
        EffectMetadataTargetKind::ArtifactId => Some(BackupDeclaredEffectTargetKind::ArtifactId),
        EffectMetadataTargetKind::EvidenceId => Some(BackupDeclaredEffectTargetKind::EvidenceId),
        EffectMetadataTargetKind::LedgerStream => {
            Some(BackupDeclaredEffectTargetKind::LedgerStream)
        }
        EffectMetadataTargetKind::RequestStream => {
            Some(BackupDeclaredEffectTargetKind::RequestStream)
        }
        EffectMetadataTargetKind::Glob
        | EffectMetadataTargetKind::StateKey
        | EffectMetadataTargetKind::CompletionId => None,
    }
}

fn effect_metadata_target_kind(kind: BackupDeclaredEffectTargetKind) -> EffectMetadataTargetKind {
    match kind {
        BackupDeclaredEffectTargetKind::FilePath => EffectMetadataTargetKind::FilePath,
        BackupDeclaredEffectTargetKind::ArtifactId => EffectMetadataTargetKind::ArtifactId,
        BackupDeclaredEffectTargetKind::EvidenceId => EffectMetadataTargetKind::EvidenceId,
        BackupDeclaredEffectTargetKind::LedgerStream => EffectMetadataTargetKind::LedgerStream,
        BackupDeclaredEffectTargetKind::RequestStream => EffectMetadataTargetKind::RequestStream,
    }
}

fn expected_effect_physical_ref(
    kind: EffectMetadataTargetKind,
    logical_ref: &str,
) -> Option<String> {
    let physical_ref = match kind {
        EffectMetadataTargetKind::FilePath => logical_ref.to_owned(),
        EffectMetadataTargetKind::ArtifactId => project_effect_logical_target(
            logical_ref,
            &[".forge-method/artifacts/"],
            &[],
            ".forge-method/artifacts",
            ".yaml",
        )?,
        EffectMetadataTargetKind::EvidenceId => project_effect_logical_target(
            logical_ref,
            &[".forge-method/evidence/", ".forge-method/snapshots/"],
            &[],
            ".forge-method/evidence",
            ".json",
        )?,
        EffectMetadataTargetKind::LedgerStream => project_effect_logical_target(
            logical_ref,
            &[".forge-method/ledger/"],
            &[".forge-method/ledger.ndjson"],
            ".forge-method/ledger",
            ".ndjson",
        )?,
        EffectMetadataTargetKind::RequestStream => project_effect_logical_target(
            logical_ref,
            &[".forge-method/requests/"],
            &[".forge-method/requests.ndjson"],
            ".forge-method/requests",
            ".ndjson",
        )?,
        EffectMetadataTargetKind::Glob
        | EffectMetadataTargetKind::StateKey
        | EffectMetadataTargetKind::CompletionId => return None,
    };
    normalized_utf8_repo_relative(&physical_ref)?;
    Some(physical_ref)
}

fn project_effect_logical_target(
    logical_ref: &str,
    allowed_prefixes: &[&str],
    allowed_exact: &[&str],
    base_dir: &str,
    extension: &str,
) -> Option<String> {
    if logical_ref.contains('/') || logical_ref.contains('\\') {
        return (allowed_exact.contains(&logical_ref)
            || allowed_prefixes
                .iter()
                .any(|prefix| logical_ref.starts_with(prefix)))
        .then(|| logical_ref.to_owned());
    }
    let sanitized = logical_ref
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let safe_id = sanitized.trim_matches('.');
    if safe_id.is_empty()
        || !safe_id
            .chars()
            .any(|character| character.is_ascii_alphanumeric())
    {
        return None;
    }
    let file_name = if safe_id.ends_with(extension) {
        safe_id.to_owned()
    } else {
        format!("{safe_id}{extension}")
    };
    Some(format!("{base_dir}/{file_name}"))
}

fn normalized_utf8_components(path: &str) -> Option<Vec<String>> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return None;
    }
    let mut normalized = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_str()?;
                if value.contains('\0') {
                    return None;
                }
                normalized.push(value.to_owned());
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => return None,
        }
    }
    (!normalized.is_empty()).then_some(normalized)
}

fn normalized_utf8_repo_relative(path: &str) -> Option<String> {
    normalized_utf8_components(path).map(|components| components.join("/"))
}

fn encode_archive_component(component: &str) -> String {
    let mut encoded = String::new();
    for byte in component.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.') {
            encoded.push(char::from(*byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
}

/// Canonical, injective archive identity for a normalized UTF-8 OS path.
/// Components are normalized with target-platform `Path::components`; every byte outside
/// the portable unreserved set (including `%`, backslash, controls, and non-ASCII UTF-8)
/// is uppercase percent encoded within its component.
///
/// # Errors
///
/// Returns `InvalidLogicalPath` when the source is not a relative, representable UTF-8
/// target-platform path.
pub fn canonical_archive_path(source_path: &str) -> Result<String, BackupManifestValidationError> {
    let components = normalized_utf8_components(source_path).ok_or_else(|| {
        BackupManifestValidationError::InvalidLogicalPath {
            path: source_path.to_owned(),
        }
    })?;
    Ok(components
        .iter()
        .map(|component| encode_archive_component(component))
        .collect::<Vec<_>>()
        .join("/"))
}

fn decode_canonical_archive_path(
    archive_path: &str,
) -> Result<String, BackupManifestValidationError> {
    validate_safe_path(archive_path)?;
    let mut decoded_components = Vec::new();
    for component in archive_path.split('/') {
        let bytes = component.as_bytes();
        let mut decoded = Vec::with_capacity(bytes.len());
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'%' {
                if index + 2 >= bytes.len()
                    || !bytes[index + 1].is_ascii_hexdigit()
                    || !bytes[index + 2].is_ascii_hexdigit()
                {
                    return Err(BackupManifestValidationError::InvalidLogicalPath {
                        path: archive_path.to_owned(),
                    });
                }
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).expect("ASCII hex");
                decoded.push(u8::from_str_radix(hex, 16).map_err(|_| {
                    BackupManifestValidationError::InvalidLogicalPath {
                        path: archive_path.to_owned(),
                    }
                })?);
                index += 3;
            } else {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
        let decoded = String::from_utf8(decoded).map_err(|_| {
            BackupManifestValidationError::InvalidLogicalPath {
                path: archive_path.to_owned(),
            }
        })?;
        decoded_components.push(decoded);
    }
    Ok(decoded_components.join("/"))
}

fn validate_canonical_archive_path(
    archive_path: &str,
) -> Result<(), BackupManifestValidationError> {
    let decoded = decode_canonical_archive_path(archive_path)?;
    if canonical_archive_path(&decoded).as_deref() == Ok(archive_path) {
        Ok(())
    } else {
        Err(BackupManifestValidationError::InvalidLogicalPath {
            path: archive_path.to_owned(),
        })
    }
}

fn expected_effect_state_relative_path(
    kind: EffectMetadataTargetKind,
    logical_ref: &str,
    layout: &BackupArchiveLayout,
) -> Option<String> {
    let physical_ref = expected_effect_physical_ref(kind, logical_ref)?;
    let physical_components = normalized_utf8_components(&physical_ref)?;
    let state_components = normalized_utf8_components(&layout.state_root_relative_to_sidecar)?;
    let relative = physical_components.strip_prefix(state_components.as_slice())?;
    if relative.is_empty() {
        return None;
    }
    Some(
        relative
            .iter()
            .map(|component| encode_archive_component(component))
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn is_live_effect_source_path_allowed(physical_ref: &str, layout: &BackupArchiveLayout) -> bool {
    let Some(path) = normalized_utf8_repo_relative(physical_ref) else {
        return false;
    };
    let Some(state) = normalized_utf8_repo_relative(&layout.state_root_relative_to_sidecar) else {
        return false;
    };
    !is_forbidden_private_path(&path)
        && !is_explicit_lock_path(&path, &state)
        && !is_crash_protocol_path(&path)
        && !is_domain_pack_staging_path(&path, &state)
}

fn validate_external_authorities(
    manifest: &BackupManifest,
) -> Result<(), BackupManifestValidationError> {
    let observations = &manifest.external_authority_observations;
    let anchor = &observations.replay_rollback_anchor;
    if anchor.schema_version != "0.1"
        || anchor.deployment_id.trim().is_empty()
        || anchor.deployment_id.len() > 256
        || anchor.protected_anchor_identity.trim().is_empty()
        || anchor.epoch.len() != 64
        || !anchor
            .epoch
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || anchor.generation == 0
        || (anchor.generation == 1) != anchor.previous_anchor_digest.is_none()
        || anchor.replay_wal_record_count != anchor.replay_wal_last_seq
    {
        return Err(BackupManifestValidationError::InvalidExternalAuthorities);
    }
    for value in [
        &anchor.anchor_document_sha256,
        &anchor.replay_wal_manifest_digest,
        &anchor.replay_wal_prefix_digest,
    ] {
        digest("replay_anchor.digest", value)?;
    }
    if let Some(previous) = &anchor.previous_anchor_digest {
        digest("replay_anchor.previous_anchor_digest", previous)?;
    }
    let replay_manifest = entry(manifest, BackupEntryKind::ReplayWalManifest)?;
    let replay_wal = entry(manifest, BackupEntryKind::ReplayWal)?;
    if replay_manifest.sha256 != anchor.replay_wal_manifest_digest
        || replay_wal.sha256 != anchor.replay_wal_prefix_digest
        || replay_wal.byte_length != anchor.replay_wal_byte_length
    {
        return Err(BackupManifestValidationError::InvalidExternalAuthorities);
    }
    let supply_provisioned = manifest.source_state.domain_pack_supply_chain_anchor
        == BackupProvisioningState::Provisioned;
    let learning_provisioned = manifest.source_state.domain_pack_reviewed_learning_anchor
        == BackupProvisioningState::Provisioned;
    let domain_active = matches!(
        manifest.source_state.domain_pack_store,
        BackupDomainPackStoreState::Active { .. }
    );
    let operator_sources_present = match manifest.source_state.domain_pack_store {
        BackupDomainPackStoreState::NoActiveGeneration {
            operator_sources_present,
            ..
        }
        | BackupDomainPackStoreState::Active {
            operator_sources_present,
            ..
        } => operator_sources_present,
    };
    if observations.domain_pack_supply_chain.is_some() != supply_provisioned
        || observations.domain_pack_reviewed_learning.is_some() != learning_provisioned
        || (domain_active && (!supply_provisioned || !learning_provisioned))
        || (operator_sources_present && (!supply_provisioned || !learning_provisioned))
    {
        return Err(BackupManifestValidationError::InvalidExternalAuthorities);
    }
    if let Some(value) = &observations.domain_pack_supply_chain {
        required(
            "domain_supply.operator_root_identity",
            &value.operator_root_identity,
        )?;
        required("domain_supply.registry_id", &value.registry_id)?;
        required("domain_supply.audience", &value.audience)?;
        if value.schema_version != "forge-domain-pack-registry-anchor-v1" || value.generation == 0 {
            return Err(BackupManifestValidationError::InvalidExternalAuthorities);
        }
        validate_digest_fields([
            &value.anchor_document_sha256,
            &value.registry_snapshot_digest,
            &value.trust_policy_digest,
            &value.registry_file_sha256,
            &value.trust_policy_file_sha256,
            &value.capability_registry_file_sha256,
            &value.sandbox_policy_file_sha256,
        ])?;
        if let Some(sources) = &manifest.source_state.domain_pack_operator_sources {
            if value.operator_root_identity != sources.operator_root_identity
                || value.registry_file_sha256 != sources.registry_file_sha256
                || value.trust_policy_file_sha256 != sources.trust_policy_file_sha256
                || value.capability_registry_file_sha256 != sources.capability_registry_file_sha256
                || value.sandbox_policy_file_sha256 != sources.sandbox_policy_file_sha256
            {
                return Err(BackupManifestValidationError::InvalidExternalAuthorities);
            }
        }
    }
    if let Some(value) = &observations.domain_pack_reviewed_learning {
        for text in [
            &value.operator_root_identity,
            &value.reviewer_registry_id,
            &value.reviewer_audience,
            &value.reviewed_registry_id,
            &value.reviewed_audience,
        ] {
            required("domain_learning.identity", text)?;
        }
        if value.schema_version != "forge-domain-pack-learning-anchor-v1"
            || value.reviewer_generation == 0
            || value.reviewed_generation == 0
        {
            return Err(BackupManifestValidationError::InvalidExternalAuthorities);
        }
        validate_digest_fields([
            &value.reviewer_registry_digest,
            &value.reviewed_registry_digest,
            &value.anchor_document_sha256,
            &value.reviewer_registry_file_sha256,
            &value.reviewed_registry_file_sha256,
        ])?;
        if let Some(sources) = &manifest.source_state.domain_pack_operator_sources {
            if value.operator_root_identity != sources.operator_root_identity
                || value.reviewer_registry_file_sha256 != sources.reviewer_registry_file_sha256
                || value.reviewed_registry_file_sha256 != sources.reviewed_registry_file_sha256
            {
                return Err(BackupManifestValidationError::InvalidExternalAuthorities);
            }
        }
        if let (Some(supply), Some(learning), Some(effective)) = (
            &observations.domain_pack_supply_chain,
            &observations.domain_pack_reviewed_learning,
            &manifest
                .effective_epoch
                .effective_bundle
                .domain_pack_generation,
        ) {
            if supply.registry_snapshot_digest != effective.supply_chain_registry_digest
                || learning.reviewer_registry_digest != effective.reviewer_registry_digest
                || learning.reviewed_registry_digest != effective.reviewed_registry_digest
            {
                return Err(BackupManifestValidationError::InvalidExternalAuthorities);
            }
        }
    }
    validate_registry_observation(
        manifest,
        BackupEntryKind::PublicPrincipalRegistry,
        &observations.workflow_principal_registry,
        manifest.source_state.workflow_principal_registry,
    )?;
    validate_registry_observation(
        manifest,
        BackupEntryKind::PublicBrokerRegistry,
        &observations.workflow_broker_registry,
        manifest.source_state.workflow_broker_registry,
    )
}

fn validate_registry_observation(
    manifest: &BackupManifest,
    kind: BackupEntryKind,
    observation: &Option<BackupPublicRegistryMaterial>,
    state: BackupProvisioningState,
) -> Result<(), BackupManifestValidationError> {
    if (state == BackupProvisioningState::Provisioned) != observation.is_some() {
        return Err(BackupManifestValidationError::InvalidExternalAuthorities);
    }
    if let Some(value) = observation {
        required("public_registry.schema_version", &value.schema_version)?;
        required("public_registry.audience", &value.audience)?;
        digest("public_registry.registry_sha256", &value.registry_sha256)?;
        if entry(manifest, kind)?.sha256 != value.registry_sha256 {
            return Err(BackupManifestValidationError::InvalidExternalAuthorities);
        }
    }
    Ok(())
}

fn validate_entry_path(
    entry: &BackupEntry,
    manifest: &BackupManifest,
) -> Result<(), BackupManifestValidationError> {
    let layout = &manifest.project.archive_layout;
    let state = state_prefix(layout);
    let exact = |suffix: &str| entry.logical_path == format!("{state}/{suffix}");
    let below = |directory: &str| {
        entry
            .logical_path
            .starts_with(&format!("{state}/{directory}/"))
    };
    let sidecar =
        |suffix: &str| entry.logical_path == format!("{}/{suffix}", layout.sidecar_archive_root);
    let valid = match entry.material {
        BackupEntryKind::ProjectLink => entry.logical_path == layout.project_link_archive_path,
        BackupEntryKind::ProjectState => exact("state.yaml"),
        BackupEntryKind::RootLedger => exact("ledger.ndjson"),
        BackupEntryKind::ReplayWalManifest => exact("replay-wal.manifest.json"),
        BackupEntryKind::ReplayWal => exact("wal/replay.fmr1"),
        BackupEntryKind::WorkflowGovernanceWal => exact("wal/workflow-governance.ndjson"),
        BackupEntryKind::ClaimWal => exact("wal/claims.fmw1"),
        BackupEntryKind::ClaimWalManifest => exact("wal/claims.wal.manifest.json"),
        BackupEntryKind::ClaimWalSnapshot => below("wal/snapshots"),
        BackupEntryKind::ClaimWalArchive => below("wal/archive"),
        BackupEntryKind::WorkflowActionReplayManifest => {
            exact("workflow-action-replay.manifest.json")
        }
        BackupEntryKind::WorkflowActionReplayWal => exact("wal/workflow-action-replay.jsonl"),
        BackupEntryKind::EffectWal => exact("wal/effects.ndjson"),
        BackupEntryKind::EffectWalCompactionManifest => {
            exact("wal/.effects.ndjson.compaction-manifest.json")
        }
        BackupEntryKind::MemoryEventLog => exact("memory/events.ndjson"),
        BackupEntryKind::ResearchEventLog => exact("research/sources.ndjson"),
        BackupEntryKind::GovernanceConflictEventLog => exact("governance/conflicts.ndjson"),
        BackupEntryKind::DomainPackOperatorSources => exact("domain-packs/operator-sources.yaml"),
        BackupEntryKind::DomainPackRebasePlan => exact("domain-packs/rebase-plan.yaml"),
        BackupEntryKind::DomainPackActivePointer => exact("domain-packs/active.lock.yaml"),
        BackupEntryKind::DomainPackLedgerRecord => {
            below("domain-packs/ledger") && entry.logical_path.ends_with(".yaml")
        }
        BackupEntryKind::DomainPackGenerationManifest => {
            below("domain-packs/generations") && entry.logical_path.ends_with("/generation.yaml")
        }
        BackupEntryKind::DomainPackGenerationLock => {
            below("domain-packs/generations") && entry.logical_path.ends_with("/lock.yaml")
        }
        BackupEntryKind::DomainPackGenerationPreflight => {
            below("domain-packs/generations") && entry.logical_path.ends_with("/preflight.yaml")
        }
        BackupEntryKind::DomainPackGenerationCompatibility => {
            below("domain-packs/generations") && entry.logical_path.ends_with("/compatibility.yaml")
        }
        BackupEntryKind::DomainPackGenerationReceipt => {
            below("domain-packs/generations") && entry.logical_path.ends_with("/receipt.yaml")
        }
        BackupEntryKind::DomainPackGenerationResolutionRequest => {
            below("domain-packs/generations")
                && entry.logical_path.ends_with("/resolution-request.yaml")
        }
        BackupEntryKind::DomainPackGenerationCompositionRequest => {
            below("domain-packs/generations")
                && entry.logical_path.ends_with("/composition-request.yaml")
        }
        BackupEntryKind::DomainPackGenerationTrustInput => {
            below("domain-packs/generations") && entry.logical_path.ends_with("/trust-input.yaml")
        }
        BackupEntryKind::DomainPackPublishedReceipt => {
            below("domain-packs/receipts") && entry.logical_path.ends_with(".yaml")
        }
        BackupEntryKind::DomainPackObject => below("domain-packs/objects"),
        BackupEntryKind::DomainPackLearningIndex => exact("domain-pack-learning/index.json"),
        BackupEntryKind::DomainPackLearningObject => below("domain-pack-learning/objects"),
        BackupEntryKind::IsolationContract => {
            below("contracts/isolations") && entry.logical_path.ends_with(".yaml")
        }
        BackupEntryKind::PublicPrincipalRegistry => {
            sidecar("operator/workflow-principal-registry.yaml")
        }
        BackupEntryKind::PublicBrokerRegistry => sidecar("operator/workflow-broker-registry.yaml"),
        BackupEntryKind::ClaimCache => {
            below("claims-active") && entry.logical_path.ends_with(".yaml")
        }
        BackupEntryKind::OfficialHandoffArtifact => {
            below("handoffs/expired-claims") && entry.logical_path.ends_with(".yaml")
        }
        BackupEntryKind::Artifact => below("artifacts"),
        BackupEntryKind::Evidence => below("evidence"),
        BackupEntryKind::Snapshot => below("snapshots"),
        BackupEntryKind::LedgerStream => below("ledger"),
        BackupEntryKind::RequestStream => below("requests") || exact("requests.ndjson"),
        BackupEntryKind::RuntimeSnapshot => below("runtime"),
        BackupEntryKind::StoryState => below("stories"),
        BackupEntryKind::AgentRegistryState => below("agents"),
        BackupEntryKind::PreflightProfile => exact("preflight.yaml"),
        BackupEntryKind::EffectMetadataIndex => exact("index/effect-targets.ndjson"),
        BackupEntryKind::TraceLog => exact("traces/events.ndjson"),
        BackupEntryKind::DeclaredEffectOutput => manifest
            .source_state
            .declared_effect_outputs
            .iter()
            .any(|output| entry.logical_path == format!("{state}/{}", output.state_relative_path)),
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

fn all_entry_kinds() -> impl Iterator<Item = BackupEntryKind> {
    [
        BackupEntryKind::ProjectLink,
        BackupEntryKind::ProjectState,
        BackupEntryKind::RootLedger,
        BackupEntryKind::ReplayWalManifest,
        BackupEntryKind::ReplayWal,
        BackupEntryKind::WorkflowGovernanceWal,
        BackupEntryKind::ClaimWal,
        BackupEntryKind::ClaimWalManifest,
        BackupEntryKind::ClaimWalSnapshot,
        BackupEntryKind::ClaimWalArchive,
        BackupEntryKind::WorkflowActionReplayManifest,
        BackupEntryKind::WorkflowActionReplayWal,
        BackupEntryKind::EffectWal,
        BackupEntryKind::EffectWalCompactionManifest,
        BackupEntryKind::MemoryEventLog,
        BackupEntryKind::ResearchEventLog,
        BackupEntryKind::GovernanceConflictEventLog,
        BackupEntryKind::DomainPackOperatorSources,
        BackupEntryKind::DomainPackRebasePlan,
        BackupEntryKind::DomainPackActivePointer,
        BackupEntryKind::DomainPackLedgerRecord,
        BackupEntryKind::DomainPackGenerationManifest,
        BackupEntryKind::DomainPackGenerationLock,
        BackupEntryKind::DomainPackGenerationPreflight,
        BackupEntryKind::DomainPackGenerationCompatibility,
        BackupEntryKind::DomainPackGenerationReceipt,
        BackupEntryKind::DomainPackGenerationResolutionRequest,
        BackupEntryKind::DomainPackGenerationCompositionRequest,
        BackupEntryKind::DomainPackGenerationTrustInput,
        BackupEntryKind::DomainPackPublishedReceipt,
        BackupEntryKind::DomainPackObject,
        BackupEntryKind::DomainPackLearningIndex,
        BackupEntryKind::DomainPackLearningObject,
        BackupEntryKind::IsolationContract,
        BackupEntryKind::PublicPrincipalRegistry,
        BackupEntryKind::PublicBrokerRegistry,
        BackupEntryKind::ClaimCache,
        BackupEntryKind::OfficialHandoffArtifact,
        BackupEntryKind::Artifact,
        BackupEntryKind::Evidence,
        BackupEntryKind::Snapshot,
        BackupEntryKind::LedgerStream,
        BackupEntryKind::RequestStream,
        BackupEntryKind::RuntimeSnapshot,
        BackupEntryKind::StoryState,
        BackupEntryKind::AgentRegistryState,
        BackupEntryKind::DeclaredEffectOutput,
        BackupEntryKind::PreflightProfile,
        BackupEntryKind::EffectMetadataIndex,
        BackupEntryKind::TraceLog,
    ]
    .into_iter()
}

fn entry(
    manifest: &BackupManifest,
    kind: BackupEntryKind,
) -> Result<&BackupEntry, BackupManifestValidationError> {
    manifest
        .entries
        .iter()
        .find(|entry| entry.material == kind)
        .ok_or(BackupManifestValidationError::SourceStateInventoryMismatch { material: kind })
}

fn has_entry(manifest: &BackupManifest, kind: BackupEntryKind, path: &str) -> bool {
    manifest
        .entries
        .iter()
        .any(|entry| entry.material == kind && entry.logical_path == path)
}

fn state_prefix(layout: &BackupArchiveLayout) -> String {
    format!(
        "{}/{}",
        layout.sidecar_archive_root, layout.state_root_relative_to_sidecar
    )
}

fn digest_token(value: &str) -> Result<&str, BackupManifestValidationError> {
    digest("digest_token", value)?;
    Ok(&value[7..])
}

fn validate_digest_fields<'a>(
    values: impl IntoIterator<Item = &'a String>,
) -> Result<(), BackupManifestValidationError> {
    for value in values {
        digest("external_authority.digest", value)?;
    }
    Ok(())
}

fn is_forbidden_private_path(path: &str) -> bool {
    path.starts_with("sidecar/operator/workflow-secrets/")
        || path.starts_with("sidecar/operator/secrets/")
        || path.contains("/private-keys/")
        || path.ends_with(".pem")
        || path.ends_with(".key")
}

fn is_explicit_lock_path(path: &str, state: &str) -> bool {
    path.starts_with(&format!("{state}/locks/"))
        || path == format!("{state}/domain-pack-learning/capture.lock")
        || path == format!("{state}/claims-active/.forge-claim.lock")
        || path == format!("{state}/contracts/isolations/.forge-isolation.lock")
}

fn is_domain_pack_staging_path(path: &str, state: &str) -> bool {
    path.starts_with(&format!("{state}/domain-packs/generations/staging/"))
}

fn is_crash_protocol_path(path: &str) -> bool {
    let Some(name) = path.rsplit('/').next() else {
        return false;
    };
    if name.starts_with('.')
        && [
            ".forge-next",
            ".forge-previous",
            ".forge-transaction",
            ".forge-tmp",
            ".forge-bak",
        ]
        .iter()
        .any(|suffix| name.ends_with(suffix))
    {
        return true;
    }
    if let Some(pid) = name.strip_prefix("state.yaml.tmp-") {
        return pid.parse::<u32>().is_ok();
    }
    if name.starts_with(".forge-method.yaml.tmp-") {
        let mut parts = name.rsplit('-');
        return parts.next().is_some()
            && parts.next().is_some_and(|pid| pid.parse::<u32>().is_ok());
    }
    let claim_temp = name.rsplit_once(".tmp.").is_some_and(|(_, suffix)| {
        let mut values = suffix.split('.');
        values.next().is_some_and(|pid| pid.parse::<u32>().is_ok())
            && values
                .next()
                .is_some_and(|millis| millis.parse::<u64>().is_ok())
            && values.next().is_none()
    });
    if claim_temp {
        return true;
    }
    if !name.starts_with('.') || !name.ends_with(".tmp") {
        return false;
    }
    let mut parts = name.rsplit('.');
    parts.next() == Some("tmp")
        && parts
            .next()
            .is_some_and(|value| value.parse::<u32>().is_ok())
        && parts
            .next()
            .is_some_and(|value| value.parse::<u128>().is_ok())
        && parts
            .next()
            .is_some_and(|value| value.parse::<u32>().is_ok())
        && parts.next().is_some()
}

fn normalized_relative(state_root: &str, sidecar_root: &str) -> Option<String> {
    fn normalize(value: &str) -> Option<Vec<String>> {
        let mut parts = Vec::new();
        for component in value.split('/') {
            match component {
                "" | "." => {}
                ".." if parts.last().is_some_and(|part| part != "..") => {
                    parts.pop();
                }
                ".." => parts.push("..".to_owned()),
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

fn leaf(path: &str) -> Option<&str> {
    path.trim_end_matches('/').rsplit('/').next()
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

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
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

    fn parse_manifest(path: &str) -> BackupManifestDocument {
        yaml_serde::from_str(&fixture(path))
            .unwrap_or_else(|error| panic!("{path} must parse before semantic validation: {error}"))
    }

    fn recompute(document: &mut BackupManifestDocument) {
        document.backup_manifest.manifest_set_digest = document.set_digest().unwrap();
    }

    fn source_metadata(document: &BackupManifestDocument) -> Vec<BackupSourceFileMetadata> {
        document
            .backup_manifest
            .entries
            .iter()
            .map(|entry| BackupSourceFileMetadata {
                logical_path: decode_canonical_archive_path(&entry.logical_path).unwrap(),
                entry_type: entry.entry_type,
                hard_link_count: 1,
                byte_length: entry.byte_length,
                sha256: entry.sha256.clone(),
            })
            .collect()
    }

    fn effect_record(
        operation_id: &str,
        effect_id: &str,
        access_mode: EffectMetadataAccessMode,
        content_hash: Option<&str>,
        byte_len: u64,
    ) -> EffectMetadataRecord {
        EffectMetadataRecord {
            schema_version: "0.1".to_owned(),
            record_kind: EffectMetadataRecordKind::EffectTarget,
            recorded_at: Some("2026-07-16T00:00:00Z".to_owned()),
            operation_id: operation_id.to_owned(),
            effect_id: effect_id.to_owned(),
            logical_ref: ".forge-method/custom/source-derived-output.yaml".to_owned(),
            physical_ref: ".forge-method/custom/source-derived-output.yaml".to_owned(),
            target_kind: EffectMetadataTargetKind::FilePath,
            access_mode,
            content_hash: content_hash.map(str::to_owned),
            byte_len,
            actor_agent_id: "agent.backup-test".to_owned(),
            actor_role: EffectMetadataActorRole::Runtime,
            destructive: access_mode == EffectMetadataAccessMode::Delete,
            redaction_hint: "raw_content_not_indexed".to_owned(),
        }
    }

    fn encoded_effect_records(records: &[EffectMetadataRecord]) -> Vec<u8> {
        let mut raw = Vec::new();
        for record in records {
            raw.extend(serde_json::to_vec(record).unwrap());
            raw.push(b'\n');
        }
        raw
    }

    fn bind_effect_fixture(
        document: &mut BackupManifestDocument,
        raw: &[u8],
        declared_record: &EffectMetadataRecord,
    ) {
        let record_bytes = serde_json::to_vec(declared_record).unwrap();
        let target_kind = backup_effect_target_kind(declared_record.target_kind).unwrap();
        let state_relative_path = expected_effect_state_relative_path(
            declared_record.target_kind,
            &declared_record.logical_ref,
            &document.backup_manifest.project.archive_layout,
        )
        .unwrap();
        if effect_output_material(&document.backup_manifest, &state_relative_path).is_none() {
            let output_path = format!(
                "{}/{}",
                state_prefix(&document.backup_manifest.project.archive_layout),
                state_relative_path
            );
            document
                .backup_manifest
                .entries
                .iter_mut()
                .find(|entry| entry.material == BackupEntryKind::DeclaredEffectOutput)
                .unwrap()
                .logical_path = output_path;
        }
        let material =
            effect_output_material(&document.backup_manifest, &state_relative_path).unwrap();
        let content_sha256 = declared_record.content_hash.clone().unwrap();
        let output = document
            .backup_manifest
            .source_state
            .declared_effect_outputs
            .first_mut()
            .unwrap();
        output
            .operation_id
            .clone_from(&declared_record.operation_id);
        output.effect_id.clone_from(&declared_record.effect_id);
        output.target_kind = target_kind;
        output
            .physical_ref
            .clone_from(&declared_record.physical_ref);
        output.logical_ref.clone_from(&declared_record.logical_ref);
        output.state_relative_path = state_relative_path;
        output.access_mode = match declared_record.access_mode {
            EffectMetadataAccessMode::Write => BackupEffectOutputAccessMode::Write,
            EffectMetadataAccessMode::Append => BackupEffectOutputAccessMode::Append,
            EffectMetadataAccessMode::Create => BackupEffectOutputAccessMode::Create,
            EffectMetadataAccessMode::Read | EffectMetadataAccessMode::Delete => unreachable!(),
        };
        output.byte_length = declared_record.byte_len;
        output.content_sha256.clone_from(&content_sha256);
        output.metadata_record_sha256 = sha256(&record_bytes);
        if material != BackupEntryKind::DeclaredEffectOutput {
            document
                .backup_manifest
                .entries
                .retain(|entry| entry.material != BackupEntryKind::DeclaredEffectOutput);
        }
        let output_entry = document
            .backup_manifest
            .entries
            .iter_mut()
            .find(|entry| entry.material == material)
            .unwrap();
        output_entry.logical_path = format!(
            "{}/{}",
            state_prefix(&document.backup_manifest.project.archive_layout),
            output.state_relative_path
        );
        output_entry.sha256 = content_sha256;
        output_entry.byte_length = declared_record.byte_len;
        let index_entry = document
            .backup_manifest
            .entries
            .iter_mut()
            .find(|entry| entry.material == BackupEntryKind::EffectMetadataIndex)
            .unwrap();
        index_entry.byte_length = raw.len() as u64;
        index_entry.sha256 = sha256(raw);
        document.backup_manifest.entries.sort_by(|left, right| {
            (left.material, &left.logical_path).cmp(&(right.material, &right.logical_path))
        });
        recompute(document);
    }

    fn bind_index_without_outputs(document: &mut BackupManifestDocument, raw: &[u8]) {
        document
            .backup_manifest
            .source_state
            .declared_effect_outputs
            .clear();
        document
            .backup_manifest
            .entries
            .retain(|entry| entry.material != BackupEntryKind::DeclaredEffectOutput);
        let index_entry = document
            .backup_manifest
            .entries
            .iter_mut()
            .find(|entry| entry.material == BackupEntryKind::EffectMetadataIndex)
            .unwrap();
        index_entry.byte_length = raw.len() as u64;
        index_entry.sha256 = sha256(raw);
        recompute(document);
    }

    #[test]
    fn every_valid_fixture_parses_then_validates() {
        for path in [
            "valid/empty-pre-rotation-v1.yaml",
            "valid/multi-generation-v1.yaml",
            "valid/no-active-provisioned-v1.yaml",
            "valid/replacement-machine-public-material-v1.yaml",
        ] {
            parse_manifest(path)
                .validate_integrity()
                .unwrap_or_else(|error| panic!("{path} failed semantic validation: {error:?}"));
        }
    }

    #[test]
    fn every_named_hostile_manifest_is_unique_and_fails_with_its_attack_class() {
        let expected = [
            (
                "anchor-wal-binding.invalid.yaml",
                "InvalidExternalAuthorities",
            ),
            ("duplicate-entry.invalid.yaml", "EntriesNotCanonical"),
            (
                "effect-byte-length-substitution.invalid.yaml",
                "InvalidDeclaredEffectProjection",
            ),
            (
                "external-domain-trust-substitution.invalid.yaml",
                "InvalidExternalAuthorities",
            ),
            ("extra-entry.invalid.yaml", "SourceStateInventoryMismatch"),
            ("fifo-entry.invalid.yaml", "NonRegularArchiveEntry"),
            (
                "generic-fallback-admission.invalid.yaml",
                "InvalidDeclaredEffectProjection",
            ),
            ("hardlink-entry.invalid.yaml", "NonRegularArchiveEntry"),
            (
                "identity-domain-reviewer.invalid.yaml",
                "InvalidExternalAuthorities",
            ),
            ("identity-release-version.invalid.yaml", "InvalidDigest"),
            (
                "isolation-mutation.invalid.yaml",
                "InvalidIsolationProjection",
            ),
            (
                "learning-object-index-race.invalid.yaml",
                "InvalidLearningProjection",
            ),
            (
                "lock-admission.invalid.yaml",
                "InvalidDeclaredEffectProjection",
            ),
            (
                "lock-effect-after-replay.invalid.yaml",
                "InvalidSnapshotProtocol",
            ),
            (
                "malformed-replay-projection.invalid.yaml",
                "InvalidExternalAuthorities",
            ),
            ("mixed-project.invalid.yaml", "InvalidProjectLink"),
            (
                "multiple-generation-omission.invalid.yaml",
                "InvalidDomainPackProjection",
            ),
            (
                "multiple-generation-substitution.invalid.yaml",
                "InvalidDomainPackProjection",
            ),
            (
                "no-active-anchor-substitution.invalid.yaml",
                "InvalidExternalAuthorities",
            ),
            (
                "no-active-reviewed-anchor-omission.invalid.yaml",
                "InvalidExternalAuthorities",
            ),
            (
                "omitted-authority.invalid.yaml",
                "InvalidExternalAuthorities",
            ),
            (
                "omitted-claim-wal.invalid.yaml",
                "SourceStateInventoryMismatch",
            ),
            ("omitted-entry.invalid.yaml", "SourceStateInventoryMismatch"),
            (
                "oversized-deployment-id.invalid.yaml",
                "InvalidExternalAuthorities",
            ),
            ("path-traversal.invalid.yaml", "InvalidLogicalPath"),
            ("private-key-entry.invalid.yaml", "ForbiddenPrivatePath"),
            ("private-root.invalid.yaml", "ForbiddenPrivatePath"),
            (
                "project-link-mismatch.invalid.yaml",
                "ProjectLinkEntryMismatch",
            ),
            (
                "project-link-substitution.invalid.yaml",
                "ProjectLinkEntryMismatch",
            ),
            ("release-mismatch.invalid.yaml", "Blank"),
            (
                "stale-generation.invalid.yaml",
                "InvalidDomainPackProjection",
            ),
            ("substituted-project.invalid.yaml", "InvalidProjectLink"),
            ("symlink-entry.invalid.yaml", "NonRegularArchiveEntry"),
            ("unknown-version.invalid.yaml", "UnsupportedSchemaVersion"),
        ];
        let hostile = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURE_ROOT)
            .join("hostile");
        let mut names = std::fs::read_dir(hostile)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            expected
                .iter()
                .map(|(name, _)| (*name).to_owned())
                .collect::<Vec<_>>()
        );
        let mut frozen_documents = BTreeSet::new();
        for (name, expected_error) in expected {
            let relative = format!("hostile/{name}");
            let raw = fixture(&relative);
            assert!(
                frozen_documents.insert(raw.clone()),
                "{relative} duplicates another nominal hostile case"
            );
            let document: BackupManifestDocument = yaml_serde::from_str(&raw)
                .unwrap_or_else(|error| panic!("{relative} must parse before validation: {error}"));
            let actual = document
                .validate_integrity()
                .expect_err("named hostile fixture must fail semantic validation");
            assert!(
                format!("{actual:?}").starts_with(expected_error),
                "{relative} exercised {actual:?}, expected attack class {expected_error}"
            );
        }
    }

    #[test]
    fn operator_sources_require_both_acquisition_anchors_even_without_active_generation() {
        let mut document = parse_manifest("valid/no-active-provisioned-v1.yaml");
        document
            .backup_manifest
            .source_state
            .domain_pack_reviewed_learning_anchor = BackupProvisioningState::NotProvisioned;
        document
            .backup_manifest
            .external_authority_observations
            .domain_pack_reviewed_learning = None;
        recompute(&mut document);
        assert_eq!(
            document.validate_integrity(),
            Err(BackupManifestValidationError::InvalidExternalAuthorities)
        );
    }

    #[test]
    fn lock_hostile_specifically_places_effect_wal_after_replay_wal() {
        let document = parse_manifest("hostile/lock-effect-after-replay.invalid.yaml");
        let mut exact_attack = BACKUP_LOCK_ORDER.to_vec();
        let effect = exact_attack
            .iter()
            .position(|scope| *scope == BackupLockScope::EffectWal)
            .unwrap();
        let replay = exact_attack
            .iter()
            .position(|scope| *scope == BackupLockScope::ReplayWal)
            .unwrap();
        exact_attack.swap(effect, replay);
        assert_eq!(
            document.backup_manifest.snapshot_protocol.lock_order,
            exact_attack
        );
        assert_eq!(
            document.validate_integrity(),
            Err(BackupManifestValidationError::InvalidSnapshotProtocol)
        );
    }

    #[test]
    fn unknown_field_is_explicit_parse_rejection_not_semantic_coverage() {
        assert!(yaml_serde::from_str::<BackupManifestDocument>(&fixture(
            "parse-rejection/unknown-field.invalid.yaml"
        ))
        .is_err());
    }

    #[test]
    fn whole_set_substitution_remains_integrity_only() {
        let mut substituted = parse_manifest("valid/empty-pre-rotation-v1.yaml");
        substituted
            .backup_manifest
            .project
            .project_link
            .project_id
            .0 = "substituted".to_owned();
        recompute(&mut substituted);
        substituted.validate_integrity().unwrap();
    }

    #[test]
    fn classifier_is_closed_and_explicitly_excludes_only_shipped_transients() {
        let document = parse_manifest("valid/multi-generation-v1.yaml");
        assert!(matches!(
            document
                .classify_source_file("sidecar/.forge-method/domain-pack-learning/capture.lock")
                .unwrap(),
            BackupSourceFileClassification::Exclude(BackupSourceExclusion::ProducerLock)
        ));
        assert!(matches!(
            document
                .classify_source_file("sidecar/.forge-method/claims-active/.forge-claim.lock")
                .unwrap(),
            BackupSourceFileClassification::Exclude(BackupSourceExclusion::ProducerLock)
        ));
        assert!(matches!(
            document
                .classify_source_file(
                    "sidecar/.forge-method/domain-pack-learning/.index.json.forge-next"
                )
                .unwrap(),
            BackupSourceFileClassification::Exclude(BackupSourceExclusion::CrashRecoveryArtifact)
        ));
        assert!(matches!(
            document
                .classify_source_file("sidecar/operator/workflow-secrets/private.yaml")
                .unwrap(),
            BackupSourceFileClassification::Exclude(
                BackupSourceExclusion::ForbiddenPrivateMaterial
            )
        ));
        assert!(matches!(
            document.classify_source_file("sidecar/.forge-method/unclassified.bin"),
            Err(BackupManifestValidationError::UnclassifiedSourceFile { .. })
        ));
        assert!(matches!(
            document.classify_source_file("sidecar/.forge-method/capture.lock"),
            Err(BackupManifestValidationError::UnclassifiedSourceFile { .. })
        ));
    }

    #[test]
    fn exclusions_precede_broad_manifest_family_lookup() {
        let original = parse_manifest("valid/multi-generation-v1.yaml");
        let cases = [
            (
                BackupEntryKind::Artifact,
                "sidecar/.forge-method/artifacts/.result.yaml.123.456.0.tmp",
                BackupSourceExclusion::CrashRecoveryArtifact,
            ),
            (
                BackupEntryKind::Evidence,
                "sidecar/.forge-method/evidence/.command.ndjson.forge-tmp",
                BackupSourceExclusion::CrashRecoveryArtifact,
            ),
            (
                BackupEntryKind::Snapshot,
                "sidecar/.forge-method/snapshots/.browser.json.forge-bak",
                BackupSourceExclusion::CrashRecoveryArtifact,
            ),
            (
                BackupEntryKind::RuntimeSnapshot,
                "sidecar/.forge-method/runtime/.snapshot.yaml.forge-next",
                BackupSourceExclusion::CrashRecoveryArtifact,
            ),
            (
                BackupEntryKind::StoryState,
                "sidecar/.forge-method/locks/story.lock",
                BackupSourceExclusion::ProducerLock,
            ),
            (
                BackupEntryKind::ClaimCache,
                "sidecar/.forge-method/claims-active/.forge-claim.lock",
                BackupSourceExclusion::ProducerLock,
            ),
            (
                BackupEntryKind::AgentRegistryState,
                "sidecar/.forge-method/domain-packs/generations/staging/agent.yaml",
                BackupSourceExclusion::IncompleteDomainPackStaging,
            ),
            (
                BackupEntryKind::Artifact,
                "sidecar/.forge-method/artifacts/private-keys/signing.key",
                BackupSourceExclusion::ForbiddenPrivateMaterial,
            ),
        ];
        for (kind, path, exclusion) in cases {
            let mut document = original.clone();
            document
                .backup_manifest
                .entries
                .iter_mut()
                .find(|entry| entry.material == kind)
                .unwrap()
                .logical_path = path.to_owned();
            assert_eq!(
                document.classify_source_file(path),
                Ok(BackupSourceFileClassification::Exclude(exclusion)),
                "listed broad-family path must remain excluded: {path}"
            );
        }

        for entry_type in [
            BackupArchiveEntryType::Symlink,
            BackupArchiveEntryType::Hardlink,
            BackupArchiveEntryType::Directory,
            BackupArchiveEntryType::Fifo,
            BackupArchiveEntryType::BlockDevice,
            BackupArchiveEntryType::CharacterDevice,
            BackupArchiveEntryType::Socket,
        ] {
            let mut observed = source_metadata(&original);
            observed
                .iter_mut()
                .find(|entry| entry.logical_path == "sidecar/.forge-method/artifacts/result.yaml")
                .unwrap()
                .entry_type = entry_type;
            assert!(original.verify_source_enumeration(&observed).is_err());
        }
        let mut hardlinked_regular = source_metadata(&original);
        hardlinked_regular
            .iter_mut()
            .find(|entry| entry.logical_path == "sidecar/.forge-method/artifacts/result.yaml")
            .unwrap()
            .hard_link_count = 2;
        assert!(original
            .verify_source_enumeration(&hardlinked_regular)
            .is_err());
    }

    #[test]
    fn source_enumerator_rejects_unknown_omitted_linked_and_substituted_metadata() {
        let document = parse_manifest("valid/multi-generation-v1.yaml");
        let complete = source_metadata(&document);
        document.verify_source_enumeration(&complete).unwrap();

        let mut unknown = complete.clone();
        unknown.push(BackupSourceFileMetadata {
            logical_path: "sidecar/.forge-method/unknown.bin".to_owned(),
            entry_type: BackupArchiveEntryType::RegularFile,
            hard_link_count: 1,
            byte_length: 1,
            sha256: format!("sha256:{}", "a".repeat(64)),
        });
        assert!(document.verify_source_enumeration(&unknown).is_err());

        let mut omitted = complete.clone();
        omitted.pop();
        assert!(document.verify_source_enumeration(&omitted).is_err());

        let mut linked = complete.clone();
        linked[0].hard_link_count = 2;
        assert!(document.verify_source_enumeration(&linked).is_err());

        let mut substituted = complete;
        substituted[0].sha256 = format!("sha256:{}", "f".repeat(64));
        assert!(document.verify_source_enumeration(&substituted).is_err());
    }

    #[test]
    fn archive_metadata_api_rejects_omission_substitution_and_special_classes() {
        let document = parse_manifest("valid/multi-generation-v1.yaml");
        let mut missing = document.backup_manifest.entries.clone();
        missing.remove(0);
        assert!(matches!(
            document.verify_archive_entries(&missing),
            Err(BackupArchiveVerificationError::MissingEntry { .. })
        ));
        let mut replaced = document.backup_manifest.entries.clone();
        replaced[0].sha256 = format!("sha256:{}", "f".repeat(64));
        assert!(matches!(
            document.verify_archive_entries(&replaced),
            Err(BackupArchiveVerificationError::SubstitutedEntry { .. })
        ));
        for class in [
            BackupArchiveEntryType::Symlink,
            BackupArchiveEntryType::Hardlink,
            BackupArchiveEntryType::Directory,
            BackupArchiveEntryType::Fifo,
            BackupArchiveEntryType::BlockDevice,
            BackupArchiveEntryType::CharacterDevice,
            BackupArchiveEntryType::Socket,
        ] {
            let mut observed = document.backup_manifest.entries.clone();
            observed[0].entry_type = class;
            assert!(document.verify_archive_entries(&observed).is_err());
        }
        assert!(BackupManifestDocument::verify_filesystem_entry_class(
            "sidecar/.forge-method/ledger.ndjson",
            BackupArchiveEntryType::RegularFile,
            2,
        )
        .is_err());
    }

    #[test]
    fn effect_index_uses_latest_source_record_and_returns_exact_record_evidence() {
        let old_hash = format!("sha256:{}", "1".repeat(64));
        let latest_hash = format!("sha256:{}", "2".repeat(64));
        let old = effect_record(
            "operation.old",
            "effect.old",
            EffectMetadataAccessMode::Create,
            Some(&old_hash),
            10,
        );
        let latest = effect_record(
            "operation.latest",
            "effect.latest",
            EffectMetadataAccessMode::Append,
            Some(&latest_hash),
            20,
        );
        let raw = encoded_effect_records(&[old, latest.clone()]);
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_effect_fixture(&mut document, &raw, &latest);

        let parsed = document.verify_effect_metadata_index_bytes(&raw).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].operation_id, "operation.latest");
        assert_eq!(parsed[0].effect_id, "effect.latest");
        assert_eq!(parsed[0].access_mode, BackupEffectOutputAccessMode::Append);
        assert_eq!(parsed[0].content_sha256, latest_hash);
        assert_eq!(parsed[0].byte_length, 20);
        assert_eq!(
            parsed[0].source_record_sha256,
            sha256(&serde_json::to_vec(&latest).unwrap())
        );
    }

    #[test]
    fn effect_index_closes_source_real_typed_file_backed_target() {
        let content_hash = format!("sha256:{}", "8".repeat(64));
        let mut artifact = effect_record(
            "operation.artifact",
            "effect.artifact",
            EffectMetadataAccessMode::Create,
            Some(&content_hash),
            21,
        );
        artifact.logical_ref = "result".to_owned();
        artifact.physical_ref = ".forge-method/artifacts/result.yaml".to_owned();
        artifact.target_kind = EffectMetadataTargetKind::ArtifactId;
        let raw = encoded_effect_records(std::slice::from_ref(&artifact));
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_effect_fixture(&mut document, &raw, &artifact);

        let parsed = document.verify_effect_metadata_index_bytes(&raw).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            parsed[0].target_kind,
            BackupDeclaredEffectTargetKind::ArtifactId
        );
        assert_eq!(parsed[0].logical_ref, "result");
        assert_eq!(parsed[0].state_relative_path, "artifacts/result.yaml");
        assert_eq!(parsed[0].content_sha256, content_hash);
    }

    #[test]
    fn effect_index_rejects_stale_declared_output() {
        let old_hash = format!("sha256:{}", "3".repeat(64));
        let latest_hash = format!("sha256:{}", "4".repeat(64));
        let old = effect_record(
            "operation.old",
            "effect.old",
            EffectMetadataAccessMode::Create,
            Some(&old_hash),
            10,
        );
        let latest = effect_record(
            "operation.latest",
            "effect.latest",
            EffectMetadataAccessMode::Write,
            Some(&latest_hash),
            11,
        );
        let raw = encoded_effect_records(&[old.clone(), latest]);
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_effect_fixture(&mut document, &raw, &old);
        assert_eq!(
            document.verify_effect_metadata_index_bytes(&raw),
            Err(BackupManifestValidationError::EffectOutputClosureMismatch)
        );
    }

    #[test]
    fn effect_index_rejects_forged_operation_and_effect_identity() {
        let content_hash = format!("sha256:{}", "5".repeat(64));
        let latest = effect_record(
            "operation.source",
            "effect.source",
            EffectMetadataAccessMode::Write,
            Some(&content_hash),
            12,
        );
        let raw = encoded_effect_records(std::slice::from_ref(&latest));
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_effect_fixture(&mut document, &raw, &latest);
        let output = document
            .backup_manifest
            .source_state
            .declared_effect_outputs
            .first_mut()
            .unwrap();
        output.operation_id = "operation.forged".to_owned();
        output.effect_id = "effect.forged".to_owned();
        recompute(&mut document);
        assert_eq!(
            document.verify_effect_metadata_index_bytes(&raw),
            Err(BackupManifestValidationError::EffectOutputClosureMismatch)
        );
    }

    #[test]
    fn effect_index_latest_delete_removes_live_output() {
        let content_hash = format!("sha256:{}", "6".repeat(64));
        let live = effect_record(
            "operation.live",
            "effect.live",
            EffectMetadataAccessMode::Create,
            Some(&content_hash),
            13,
        );
        let deleted = effect_record(
            "operation.delete",
            "effect.delete",
            EffectMetadataAccessMode::Delete,
            None,
            0,
        );
        let raw = encoded_effect_records(&[live.clone(), deleted]);
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_effect_fixture(&mut document, &raw, &live);
        assert_eq!(
            document.verify_effect_metadata_index_bytes(&raw),
            Err(BackupManifestValidationError::EffectOutputClosureMismatch)
        );
    }

    #[test]
    fn effect_physical_ref_mirror_covers_shipped_target_kind_matrix() {
        let cases = [
            (
                EffectMetadataTargetKind::FilePath,
                "src/lib.rs",
                "src/lib.rs",
            ),
            (
                EffectMetadataTargetKind::FilePath,
                "src//lib.rs",
                "src//lib.rs",
            ),
            (EffectMetadataTargetKind::FilePath, " ", " "),
            (
                EffectMetadataTargetKind::ArtifactId,
                "story-current",
                ".forge-method/artifacts/story-current.yaml",
            ),
            (
                EffectMetadataTargetKind::ArtifactId,
                ".forge-method/artifacts/story-current.yaml",
                ".forge-method/artifacts/story-current.yaml",
            ),
            (
                EffectMetadataTargetKind::ArtifactId,
                "artifact\nid",
                ".forge-method/artifacts/artifact_id.yaml",
            ),
            (
                EffectMetadataTargetKind::EvidenceId,
                "evidence\nid",
                ".forge-method/evidence/evidence_id.json",
            ),
            (
                EffectMetadataTargetKind::EvidenceId,
                "browser snapshot",
                ".forge-method/evidence/browser_snapshot.json",
            ),
            (
                EffectMetadataTargetKind::EvidenceId,
                ".forge-method/snapshots/browser.json",
                ".forge-method/snapshots/browser.json",
            ),
            (
                EffectMetadataTargetKind::LedgerStream,
                "agent-main",
                ".forge-method/ledger/agent-main.ndjson",
            ),
            (
                EffectMetadataTargetKind::LedgerStream,
                "ledger\nid",
                ".forge-method/ledger/ledger_id.ndjson",
            ),
            (
                EffectMetadataTargetKind::LedgerStream,
                "custom.yaml",
                ".forge-method/ledger/custom.yaml.ndjson",
            ),
            (
                EffectMetadataTargetKind::LedgerStream,
                ".forge-method/ledger.ndjson",
                ".forge-method/ledger.ndjson",
            ),
            (
                EffectMetadataTargetKind::LedgerStream,
                ".forge-method/ledger/custom.yaml",
                ".forge-method/ledger/custom.yaml",
            ),
            (
                EffectMetadataTargetKind::RequestStream,
                "handoff",
                ".forge-method/requests/handoff.ndjson",
            ),
            (
                EffectMetadataTargetKind::RequestStream,
                "request\nid",
                ".forge-method/requests/request_id.ndjson",
            ),
            (
                EffectMetadataTargetKind::RequestStream,
                ".forge-method/requests.ndjson",
                ".forge-method/requests.ndjson",
            ),
            (
                EffectMetadataTargetKind::RequestStream,
                ".forge-method/requests/custom.yaml",
                ".forge-method/requests/custom.yaml",
            ),
        ];
        for (kind, logical_ref, physical_ref) in cases {
            assert_eq!(
                expected_effect_physical_ref(kind, logical_ref).as_deref(),
                Some(physical_ref),
                "shipped projection mismatch for {kind:?} {logical_ref:?}"
            );
        }

        for (kind, logical_ref) in [
            (EffectMetadataTargetKind::FilePath, ""),
            (EffectMetadataTargetKind::FilePath, "."),
            (EffectMetadataTargetKind::FilePath, "../outside"),
            (EffectMetadataTargetKind::EvidenceId, "\n"),
            (EffectMetadataTargetKind::LedgerStream, "___"),
            (EffectMetadataTargetKind::RequestStream, "..."),
            (EffectMetadataTargetKind::FilePath, "/absolute"),
            (EffectMetadataTargetKind::ArtifactId, ""),
            (EffectMetadataTargetKind::ArtifactId, "..."),
            (EffectMetadataTargetKind::ArtifactId, "___"),
            (EffectMetadataTargetKind::ArtifactId, "\n"),
            (EffectMetadataTargetKind::ArtifactId, "../outside"),
            (
                EffectMetadataTargetKind::ArtifactId,
                ".forge-method/artifacts/../outside.yaml",
            ),
        ] {
            assert_eq!(
                expected_effect_physical_ref(kind, logical_ref),
                None,
                "shipped resolver must reject {kind:?} {logical_ref:?}"
            );
        }
        for unsupported in [
            EffectMetadataTargetKind::Glob,
            EffectMetadataTargetKind::StateKey,
            EffectMetadataTargetKind::CompletionId,
        ] {
            assert_eq!(expected_effect_physical_ref(unsupported, "target"), None);
        }
    }
    #[test]
    fn effect_index_validates_but_omits_normal_repository_file_records() {
        let old_hash = format!("sha256:{}", "9".repeat(64));
        let latest_hash = format!("sha256:{}", "a".repeat(64));
        let mut old = effect_record(
            "operation.source-old",
            "effect.source-old",
            EffectMetadataAccessMode::Create,
            Some(&old_hash),
            11,
        );
        old.logical_ref = "src/lib.rs".to_owned();
        old.physical_ref = "src/lib.rs".to_owned();
        let mut latest = old.clone();
        latest.operation_id = "operation.source-latest".to_owned();
        latest.effect_id = "effect.source-latest".to_owned();
        latest.access_mode = EffectMetadataAccessMode::Write;
        latest.content_hash = Some(latest_hash);
        latest.byte_len = 22;
        let raw = encoded_effect_records(&[old, latest]);
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_index_without_outputs(&mut document, &raw);

        assert!(document
            .verify_effect_metadata_index_bytes(&raw)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn reviewer_canonical_frames_accept_preserved_file_spelling_and_sanitized_id() {
        let hash = format!("sha256:{}", "b".repeat(64));

        let mut file = effect_record(
            "operation.reviewer-file",
            "effect.reviewer-file",
            EffectMetadataAccessMode::Write,
            Some(&hash),
            22,
        );
        file.logical_ref = "src//lib.rs".to_owned();
        file.physical_ref = file.logical_ref.clone();
        let file_raw = encoded_effect_records(std::slice::from_ref(&file));
        let mut file_document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_index_without_outputs(&mut file_document, &file_raw);
        assert!(file_document
            .verify_effect_metadata_index_bytes(&file_raw)
            .unwrap()
            .is_empty());

        let mut normalized_forgery = file.clone();
        normalized_forgery.physical_ref = "src/lib.rs".to_owned();
        let forged_raw = encoded_effect_records(&[normalized_forgery]);
        let mut forged_document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_index_without_outputs(&mut forged_document, &forged_raw);
        assert_eq!(
            forged_document.verify_effect_metadata_index_bytes(&forged_raw),
            Err(BackupManifestValidationError::InvalidEffectMetadataIndex)
        );

        let mut artifact = effect_record(
            "operation.reviewer-artifact",
            "effect.reviewer-artifact",
            EffectMetadataAccessMode::Create,
            Some(&hash),
            23,
        );
        artifact.logical_ref = "artifact\nid".to_owned();
        artifact.physical_ref = ".forge-method/artifacts/artifact_id.yaml".to_owned();
        artifact.target_kind = EffectMetadataTargetKind::ArtifactId;
        let artifact_raw = encoded_effect_records(std::slice::from_ref(&artifact));
        let mut artifact_document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_effect_fixture(&mut artifact_document, &artifact_raw, &artifact);
        let parsed = artifact_document
            .verify_effect_metadata_index_bytes(&artifact_raw)
            .unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].logical_ref, "artifact\nid");
        assert_eq!(
            parsed[0].physical_ref,
            ".forge-method/artifacts/artifact_id.yaml"
        );
    }

    #[test]
    fn historical_sanitized_id_record_remains_valid_before_latest_delete() {
        let hash = format!("sha256:{}", "c".repeat(64));
        let mut historical = effect_record(
            "operation.historical",
            "effect.historical",
            EffectMetadataAccessMode::Create,
            Some(&hash),
            24,
        );
        historical.logical_ref = "artifact\nid".to_owned();
        historical.physical_ref = ".forge-method/artifacts/artifact_id.yaml".to_owned();
        historical.target_kind = EffectMetadataTargetKind::ArtifactId;
        let mut deleted = historical.clone();
        deleted.operation_id = "operation.delete".to_owned();
        deleted.effect_id = "effect.delete".to_owned();
        deleted.access_mode = EffectMetadataAccessMode::Delete;
        deleted.content_hash = None;
        deleted.byte_len = 0;
        deleted.destructive = true;
        let raw = encoded_effect_records(&[historical, deleted]);
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_index_without_outputs(&mut document, &raw);
        assert!(document
            .verify_effect_metadata_index_bytes(&raw)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn live_non_ndjson_streams_have_exact_typed_archive_and_classifier_closure() {
        let hash = format!("sha256:{}", "d".repeat(64));
        for (kind, physical_ref, material) in [
            (
                EffectMetadataTargetKind::LedgerStream,
                ".forge-method/ledger/custom.yaml",
                BackupEntryKind::LedgerStream,
            ),
            (
                EffectMetadataTargetKind::RequestStream,
                ".forge-method/requests/custom.yaml",
                BackupEntryKind::RequestStream,
            ),
        ] {
            let mut stream = effect_record(
                "operation.custom-stream",
                "effect.custom-stream",
                EffectMetadataAccessMode::Append,
                Some(&hash),
                25,
            );
            stream.logical_ref = physical_ref.to_owned();
            stream.physical_ref = physical_ref.to_owned();
            stream.target_kind = kind;
            let raw = encoded_effect_records(std::slice::from_ref(&stream));
            let mut document = parse_manifest("valid/multi-generation-v1.yaml");
            bind_effect_fixture(&mut document, &raw, &stream);

            let parsed = document.verify_effect_metadata_index_bytes(&raw).unwrap();
            assert_eq!(parsed.len(), 1);
            let archive_path = format!("sidecar/{physical_ref}");
            assert_eq!(
                document.classify_source_file(&archive_path),
                Ok(BackupSourceFileClassification::Archive(material))
            );
            document
                .verify_source_enumeration(&source_metadata(&document))
                .unwrap();
            document
                .verify_archive_entries(&document.backup_manifest.entries)
                .unwrap();
        }
    }

    #[test]
    fn normalized_duplicate_live_physical_paths_reject() {
        let hash = format!("sha256:{}", "e".repeat(64));
        let first = effect_record(
            "operation.duplicate-one",
            "effect.duplicate-one",
            EffectMetadataAccessMode::Create,
            Some(&hash),
            26,
        );
        let mut second = first.clone();
        second.operation_id = "operation.duplicate-two".to_owned();
        second.effect_id = "effect.duplicate-two".to_owned();
        second.logical_ref = ".forge-method/custom//source-derived-output.yaml".to_owned();
        second.physical_ref = second.logical_ref.clone();
        let raw = encoded_effect_records(&[first, second]);
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_index_without_outputs(&mut document, &raw);
        assert_eq!(
            document.verify_effect_metadata_index_bytes(&raw),
            Err(BackupManifestValidationError::InvalidEffectMetadataIndex)
        );

        let mut typed = effect_record(
            "operation.typed-artifact",
            "effect.typed-artifact",
            EffectMetadataAccessMode::Create,
            Some(&hash),
            47,
        );
        typed.target_kind = EffectMetadataTargetKind::ArtifactId;
        typed.logical_ref = "result".to_owned();
        typed.physical_ref = ".forge-method/artifacts/result.yaml".to_owned();
        let mut generic = typed.clone();
        generic.operation_id = "operation.generic-artifact".to_owned();
        generic.effect_id = "effect.generic-artifact".to_owned();
        generic.target_kind = EffectMetadataTargetKind::FilePath;
        generic.logical_ref = generic.physical_ref.clone();
        let cross_kind_raw = encoded_effect_records(&[typed, generic]);
        let mut cross_kind = parse_manifest("valid/multi-generation-v1.yaml");
        bind_index_without_outputs(&mut cross_kind, &cross_kind_raw);
        assert_eq!(
            cross_kind.verify_effect_metadata_index_bytes(&cross_kind_raw),
            Err(BackupManifestValidationError::InvalidEffectMetadataIndex)
        );
    }

    #[test]
    fn shipped_non_file_backed_and_read_records_remain_rejected() {
        let hash = format!("sha256:{}", "f".repeat(64));
        for unsupported in [
            EffectMetadataTargetKind::Glob,
            EffectMetadataTargetKind::StateKey,
            EffectMetadataTargetKind::CompletionId,
        ] {
            let mut record = effect_record(
                "operation.unsupported",
                "effect.unsupported",
                EffectMetadataAccessMode::Write,
                Some(&hash),
                27,
            );
            record.target_kind = unsupported;
            record.logical_ref = "target".to_owned();
            record.physical_ref = "target".to_owned();
            let raw = encoded_effect_records(&[record]);
            let mut document = parse_manifest("valid/multi-generation-v1.yaml");
            bind_index_without_outputs(&mut document, &raw);
            assert_eq!(
                document.verify_effect_metadata_index_bytes(&raw),
                Err(BackupManifestValidationError::InvalidEffectMetadataIndex)
            );
        }

        let mut read = effect_record(
            "operation.read",
            "effect.read",
            EffectMetadataAccessMode::Read,
            Some(&hash),
            28,
        );
        read.logical_ref = "src/read.rs".to_owned();
        read.physical_ref = read.logical_ref.clone();
        let raw = encoded_effect_records(&[read]);
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_index_without_outputs(&mut document, &raw);
        assert_eq!(
            document.verify_effect_metadata_index_bytes(&raw),
            Err(BackupManifestValidationError::InvalidEffectMetadataIndex)
        );
    }

    #[test]
    fn effect_index_validates_but_omits_out_of_state_delete() {
        let hash = format!("sha256:{}", "9".repeat(64));
        let mut live = effect_record(
            "operation.source-live",
            "effect.source-live",
            EffectMetadataAccessMode::Create,
            Some(&hash),
            11,
        );
        live.logical_ref = "src/deleted.rs".to_owned();
        live.physical_ref = "src/deleted.rs".to_owned();
        let mut deleted = live.clone();
        deleted.operation_id = "operation.source-delete".to_owned();
        deleted.effect_id = "effect.source-delete".to_owned();
        deleted.access_mode = EffectMetadataAccessMode::Delete;
        deleted.content_hash = None;
        deleted.byte_len = 0;
        deleted.destructive = true;
        let raw = encoded_effect_records(&[live, deleted]);
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_index_without_outputs(&mut document, &raw);

        assert!(document
            .verify_effect_metadata_index_bytes(&raw)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn reserved_family_file_path_is_cross_bound_to_exact_typed_bytes() {
        let forged_hash = format!("sha256:{}", "9".repeat(64));
        let mut reserved = effect_record(
            "operation.reserved",
            "effect.reserved",
            EffectMetadataAccessMode::Create,
            Some(&forged_hash),
            47,
        );
        reserved.logical_ref = ".forge-method/artifacts/result.yaml".to_owned();
        reserved.physical_ref = reserved.logical_ref.clone();
        let forged_raw = encoded_effect_records(std::slice::from_ref(&reserved));
        let mut forged = parse_manifest("valid/multi-generation-v1.yaml");
        bind_index_without_outputs(&mut forged, &forged_raw);
        assert_eq!(
            forged.verify_effect_metadata_index_bytes(&forged_raw),
            Err(BackupManifestValidationError::EffectOutputClosureMismatch)
        );

        reserved.content_hash = Some(format!("sha256:{}", "a".repeat(64)));
        let matching_raw = encoded_effect_records(std::slice::from_ref(&reserved));
        let mut matching = parse_manifest("valid/multi-generation-v1.yaml");
        bind_effect_fixture(&mut matching, &matching_raw, &reserved);
        let parsed = matching
            .verify_effect_metadata_index_bytes(&matching_raw)
            .unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            parsed[0].target_kind,
            BackupDeclaredEffectTargetKind::FilePath
        );
        assert_eq!(parsed[0].physical_ref, reserved.physical_ref);
        matching
            .verify_source_enumeration(&source_metadata(&matching))
            .unwrap();
    }

    #[test]
    fn canonical_archive_encoding_roundtrips_weird_live_linux_paths_without_collisions() {
        let hash = format!("sha256:{}", "8".repeat(64));
        let cases = [
            (".forge-method/custom\\name.yaml", "custom%5Cname.yaml"),
            (".forge-method/custom\nname.yaml", "custom%0Aname.yaml"),
            (".forge-method/custom%5Cname.yaml", "custom%255Cname.yaml"),
        ];
        let mut archive_identities = BTreeSet::new();
        for (physical_ref, archive_relative) in cases {
            let mut record = effect_record(
                "operation.weird",
                "effect.weird",
                EffectMetadataAccessMode::Create,
                Some(&hash),
                55,
            );
            record.logical_ref = physical_ref.to_owned();
            record.physical_ref = physical_ref.to_owned();
            let raw = encoded_effect_records(std::slice::from_ref(&record));
            let mut document = parse_manifest("valid/multi-generation-v1.yaml");
            bind_effect_fixture(&mut document, &raw, &record);

            let parsed = document.verify_effect_metadata_index_bytes(&raw).unwrap();
            assert_eq!(parsed[0].physical_ref, physical_ref);
            assert_eq!(parsed[0].state_relative_path, archive_relative);
            let archive_path = format!("sidecar/.forge-method/{archive_relative}");
            assert!(archive_identities.insert(archive_path.clone()));
            assert_eq!(
                decode_canonical_archive_path(&archive_path).unwrap(),
                format!("sidecar/{physical_ref}")
            );
            assert_eq!(
                document.classify_source_file(&format!("sidecar/{physical_ref}")),
                Ok(BackupSourceFileClassification::Archive(
                    BackupEntryKind::DeclaredEffectOutput
                ))
            );
            document
                .verify_source_enumeration(&source_metadata(&document))
                .unwrap();
            document
                .verify_archive_entries(&document.backup_manifest.entries)
                .unwrap();
        }
    }

    #[test]
    fn exact_effect_spelling_and_normalized_source_identity_are_both_bound() {
        let hash = format!("sha256:{}", "7".repeat(64));
        for physical_ref in [
            ".forge-method/custom/./source-derived-output.yaml",
            ".forge-method/custom//source-derived-output.yaml",
        ] {
            let mut record = effect_record(
                "operation.alias",
                "effect.alias",
                EffectMetadataAccessMode::Write,
                Some(&hash),
                55,
            );
            record.logical_ref = physical_ref.to_owned();
            record.physical_ref = physical_ref.to_owned();
            let raw = encoded_effect_records(std::slice::from_ref(&record));
            let mut document = parse_manifest("valid/multi-generation-v1.yaml");
            bind_effect_fixture(&mut document, &raw, &record);
            document.validate_integrity().unwrap();
            let output = &document
                .backup_manifest
                .source_state
                .declared_effect_outputs[0];
            assert_eq!(output.physical_ref, physical_ref);
            assert_eq!(
                output.state_relative_path,
                "custom/source-derived-output.yaml"
            );
            document.verify_effect_metadata_index_bytes(&raw).unwrap();
        }

        let document = parse_manifest("valid/multi-generation-v1.yaml");
        let mut aliases = source_metadata(&document);
        let duplicate = aliases
            .iter()
            .find(|file| file.logical_path.ends_with("/artifacts/result.yaml"))
            .unwrap()
            .clone();
        let mut spelled_alias = duplicate;
        spelled_alias.logical_path = "sidecar/.forge-method/artifacts/./result.yaml".to_owned();
        aliases.push(spelled_alias);
        assert!(document.verify_source_enumeration(&aliases).is_err());
    }

    #[test]
    fn every_shipped_typed_prefix_root_has_valid_dot_and_repeated_archive_material() {
        let hash = format!("sha256:{}", "6".repeat(64));
        let cases = [
            (
                EffectMetadataTargetKind::ArtifactId,
                "artifacts",
                BackupEntryKind::Artifact,
            ),
            (
                EffectMetadataTargetKind::EvidenceId,
                "evidence",
                BackupEntryKind::Evidence,
            ),
            (
                EffectMetadataTargetKind::EvidenceId,
                "snapshots",
                BackupEntryKind::Snapshot,
            ),
            (
                EffectMetadataTargetKind::LedgerStream,
                "ledger",
                BackupEntryKind::LedgerStream,
            ),
            (
                EffectMetadataTargetKind::RequestStream,
                "requests",
                BackupEntryKind::RequestStream,
            ),
        ];
        for (target_kind, family, owned_material) in cases {
            for suffix in [".", ""] {
                let physical_ref = if suffix.is_empty() {
                    format!(".forge-method/{family}//")
                } else {
                    format!(".forge-method/{family}/{suffix}")
                };
                let mut record = effect_record(
                    "operation.family-root",
                    "effect.family-root",
                    EffectMetadataAccessMode::Create,
                    Some(&hash),
                    55,
                );
                record.target_kind = target_kind;
                record.logical_ref = physical_ref.clone();
                record.physical_ref = physical_ref.clone();
                let raw = encoded_effect_records(std::slice::from_ref(&record));
                let mut document = parse_manifest("valid/multi-generation-v1.yaml");
                document
                    .backup_manifest
                    .entries
                    .retain(|entry| entry.material != owned_material);
                let counts = &mut document.backup_manifest.source_state.public_sidecars;
                match owned_material {
                    BackupEntryKind::Artifact => counts.artifacts = 0,
                    BackupEntryKind::Evidence => counts.evidence = 0,
                    BackupEntryKind::Snapshot => counts.snapshots = 0,
                    BackupEntryKind::LedgerStream => counts.ledger_streams = 0,
                    BackupEntryKind::RequestStream => counts.request_streams = 0,
                    _ => unreachable!(),
                }
                bind_effect_fixture(&mut document, &raw, &record);
                document.validate_integrity().unwrap();
                let parsed = document.verify_effect_metadata_index_bytes(&raw).unwrap();
                assert_eq!(parsed[0].state_relative_path, family);
                assert!(document.backup_manifest.entries.iter().any(|entry| {
                    entry.material == BackupEntryKind::DeclaredEffectOutput
                        && entry.logical_path == format!("sidecar/.forge-method/{family}")
                }));
            }
        }
    }

    #[test]
    fn archive_file_and_descendant_topology_is_impossible() {
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        document
            .backup_manifest
            .source_state
            .declared_effect_outputs[0]
            .state_relative_path = "artifacts".to_owned();
        document
            .backup_manifest
            .entries
            .iter_mut()
            .find(|entry| entry.material == BackupEntryKind::DeclaredEffectOutput)
            .unwrap()
            .logical_path = "sidecar/.forge-method/artifacts".to_owned();
        document.backup_manifest.entries.sort_by(|left, right| {
            (left.material, &left.logical_path).cmp(&(right.material, &right.logical_path))
        });
        recompute(&mut document);
        assert!(matches!(
            document.validate_integrity(),
            Err(BackupManifestValidationError::ImpossibleArchiveTopology { .. })
        ));
    }

    #[test]
    fn effect_index_rejects_forged_999_byte_claim_for_55_byte_archive_entry() {
        let hash = format!("sha256:{}", "a".repeat(64));
        let forged = effect_record(
            "operation.forged-length",
            "effect.forged-length",
            EffectMetadataAccessMode::Create,
            Some(&hash),
            999,
        );
        let raw = encoded_effect_records(std::slice::from_ref(&forged));
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_effect_fixture(&mut document, &raw, &forged);
        let output = document
            .backup_manifest
            .source_state
            .declared_effect_outputs
            .first_mut()
            .unwrap();
        output.byte_length = 55;
        document
            .backup_manifest
            .entries
            .iter_mut()
            .find(|entry| entry.material == BackupEntryKind::DeclaredEffectOutput)
            .unwrap()
            .byte_length = 55;
        recompute(&mut document);

        assert_eq!(
            document.verify_effect_metadata_index_bytes(&raw),
            Err(BackupManifestValidationError::EffectOutputClosureMismatch)
        );
    }

    #[test]
    fn effect_index_cross_binds_declared_access_mode() {
        let hash = format!("sha256:{}", "a".repeat(64));
        let source = effect_record(
            "operation.access",
            "effect.access",
            EffectMetadataAccessMode::Create,
            Some(&hash),
            55,
        );
        let raw = encoded_effect_records(std::slice::from_ref(&source));
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_effect_fixture(&mut document, &raw, &source);
        document
            .backup_manifest
            .source_state
            .declared_effect_outputs
            .first_mut()
            .unwrap()
            .access_mode = BackupEffectOutputAccessMode::Write;
        recompute(&mut document);

        assert_eq!(
            document.verify_effect_metadata_index_bytes(&raw),
            Err(BackupManifestValidationError::EffectOutputClosureMismatch)
        );
    }

    #[test]
    fn effect_index_rejects_forged_physical_ref() {
        let content_hash = format!("sha256:{}", "7".repeat(64));
        let source = effect_record(
            "operation.physical",
            "effect.physical",
            EffectMetadataAccessMode::Create,
            Some(&content_hash),
            14,
        );
        let mut forged = source.clone();
        forged.physical_ref = ".forge-method-other/custom/source-derived-output.yaml".to_owned();
        let raw = encoded_effect_records(std::slice::from_ref(&forged));
        let mut document = parse_manifest("valid/multi-generation-v1.yaml");
        bind_effect_fixture(&mut document, &raw, &source);
        assert_eq!(
            document.verify_effect_metadata_index_bytes(&raw),
            Err(BackupManifestValidationError::InvalidEffectMetadataIndex)
        );
    }

    #[test]
    fn project_link_verification_hashes_exact_raw_bytes() {
        let document = parse_manifest("valid/empty-pre-rotation-v1.yaml");
        let raw = fixture("valid/project-link.yaml");
        let parsed: ProjectLinkDocument = yaml_serde::from_str(&raw).unwrap();
        document
            .verify_project_link_bytes(raw.as_bytes(), &parsed)
            .unwrap();
        let mut changed = raw.into_bytes();
        changed.push(b' ');
        assert!(document
            .verify_project_link_bytes(&changed, &parsed)
            .is_err());
    }

    #[test]
    fn s03_exposes_no_forgeable_restore_or_trusted_success_api() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/backup_manifest.rs"),
        )
        .unwrap();
        let forbidden = [
            ["pub struct BackupRestore", "PreflightV1"].concat(),
            ["pub struct BackupTrusted", "ExpectationV1"].concat(),
            ["pub enum BackupArchive", "MembersStatus"].concat(),
            ["pub enum BackupProducer", "Quiescence"].concat(),
            ["fn validate_for_", "restore"].concat(),
        ];
        for name in forbidden {
            assert!(!source.contains(&name), "forbidden public API: {name}");
        }
    }
}
