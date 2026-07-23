//! Closed P6b supply-chain registry, deterministic resolution, and exact-lock wire types.

use crate::{
    DomainPackArtifactBinding, DomainPackCandidateAuthority, DomainPackCandidateInput,
    DomainPackCompositionGap, DomainPackContentBinding, DomainPackCoordinate,
    DomainPackCoreBinding, DomainPackDependency, DomainPackIdentity,
    DomainPackProjectRequirementsDocument, DomainPackVersionReference, RepoPath, StableId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain_pack_learning::DomainPackSemanticAssurance;
use crate::domain_pack_policy::{
    DomainPackLockedCapabilityBinding, DomainPackRuntimeCapabilityGap, DomainPackSourceAssurance,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackSupplyChainRegistryDocument {
    pub schema_version: String,
    pub domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackSupplyChainRegistry {
    pub registry_id: StableId,
    pub registry_version: String,
    pub audience: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub generation: u64,
    pub previous_snapshot_digest: Option<String>,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
    pub publisher_credentials: Vec<DomainPackPublisherCredential>,
    pub namespace_grants: Vec<DomainPackNamespaceGrant>,
    /// Signed transport metadata used only to locate immutable artifact bytes.
    /// It does not create another catalog or grant package authority.
    pub mirrors: Vec<DomainPackRegistryMirror>,
    pub packages: Vec<DomainPackRegistryPackageRecord>,
    pub revocations: Vec<DomainPackPackageRevocation>,
    pub snapshot_digest: String,
    pub signatures: Vec<DomainPackRegistrySignature>,
}

/// A signed mirror endpoint. The transport base is intentionally separate from
/// artifact object paths: a catalog record names only a normalized immutable
/// object path beneath a selected mirror, never an arbitrary URL or local path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRegistryMirror {
    pub mirror_id: StableId,
    pub priority: u16,
    pub transport: DomainPackRegistryMirrorTransport,
}

/// Closed mirror transports. An operator-provisioned local mirror carries an
/// opaque operator location id, not an agent-supplied filesystem path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainPackRegistryMirrorTransport {
    Https { base_url: String },
    OperatorProvisionedLocal { location_id: StableId },
}

/// The role of one immutable package artifact. The descriptor set has exactly
/// one manifest/content/license descriptor and zero or more fixture descriptors.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRemoteArtifactKind {
    Manifest,
    Content,
    License,
    Fixture,
}

/// A closed media-type vocabulary prevents an artifact declaration from turning
/// a consumer-selected parser into an unbounded remote execution surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRemoteArtifactMediaType {
    ApplicationYaml,
    ApplicationJson,
    TextPlain,
    ApplicationOctetStream,
}

/// Immutable descriptor signed through its containing registry record.
///
/// `binding` deliberately reuses the established raw/canonical SHA-256 pins and
/// logical `RepoPath`. `object_path` is a normalized, content-addressed path
/// relative to a selected mirror transport and must be
/// `objects/sha256/<raw-hex>` for `binding.raw_sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRemoteArtifactDescriptor {
    pub kind: DomainPackRemoteArtifactKind,
    pub binding: DomainPackArtifactBinding,
    pub object_path: RepoPath,
    pub byte_length: u64,
    pub media_type: DomainPackRemoteArtifactMediaType,
}

/// Complete immutable artifact descriptor set for one signed package record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRegistryArtifactSet {
    pub manifest: DomainPackRemoteArtifactDescriptor,
    pub content: DomainPackRemoteArtifactDescriptor,
    pub license: DomainPackRemoteArtifactDescriptor,
    pub fixtures: Vec<DomainPackRemoteArtifactDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPublisherCredential {
    pub credential_id: StableId,
    pub publisher: StableId,
    pub public_key_hex: String,
    pub status: crate::DomainPackCredentialStatus,
    pub valid_from_unix: u64,
    pub valid_until_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackNamespaceGrant {
    pub grant_id: StableId,
    pub publisher: StableId,
    pub namespace_prefix: StableId,
    pub valid_from_unix: u64,
    pub valid_until_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRegistryPackageRecord {
    pub identity: DomainPackIdentity,
    pub package_digest: String,
    pub manifest_digest: String,
    pub content_digest: String,
    pub license_digest: String,
    pub fixture_digests: Vec<String>,
    /// Complete signed manifest/content/license/fixture byte descriptors.
    /// `package_digest` above preserves its established package-level semantics.
    pub artifacts: DomainPackRegistryArtifactSet,
    pub namespace_grant_id: StableId,
    pub publisher_credential_id: StableId,
    pub publisher_signature_hex: String,
    pub record_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPackageRevocation {
    pub record_digest: String,
    pub reason: DomainPackRevocationReason,
    pub explanation: String,
    pub revoked_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRevocationReason {
    KeyCompromise,
    ProvenanceFailure,
    PackageTamper,
    OperatorPolicy,
    SupersededUnsafe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRegistrySignature {
    pub signer_key_id: StableId,
    pub role: crate::DomainPackRegistryTrustRole,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPackageBinding {
    pub package_ref: RepoPath,
    pub package_digest: String,
    pub manifest: DomainPackArtifactBinding,
    pub content: DomainPackContentBinding,
    pub license: DomainPackArtifactBinding,
    pub fixtures: Vec<DomainPackArtifactBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackResolutionRequestDocument {
    pub schema_version: String,
    pub domain_pack_resolution_request: DomainPackResolutionRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackResolutionRequest {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub project_id: StableId,
    pub forge_core_version: String,
    pub core: DomainPackCoreBinding,
    pub requirements: DomainPackProjectRequirementsDocument,
    pub roots: Vec<DomainPackResolutionRoot>,
    pub current_lock: Option<DomainPackExactLockDocument>,
    pub policy: DomainPackResolutionPolicy,
    pub registry_snapshot_digest: String,
    pub candidates: Vec<DomainPackResolutionCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackResolutionRoot {
    pub pack: DomainPackCoordinate,
    pub version_requirement: String,
    pub required_content_digest: Option<String>,
    pub reason: DomainPackResolutionRootReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackResolutionRootReason {
    ExistingProjectRoot,
    InstallIntent,
    UpgradeIntent,
    PersistentDomainRequirement,
    RollbackIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackResolutionCandidate {
    pub input: DomainPackCandidateInput,
    pub package: DomainPackPackageBinding,
    pub registry_record_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackResolutionPolicy {
    pub selection: DomainPackVersionSelectionPolicy,
    pub prerelease: DomainPackPrereleasePolicy,
    pub duplicate_version: DomainPackDuplicateVersionPolicy,
    pub dependency_source: DomainPackDependencySourcePolicy,
    pub unrelated_updates: DomainPackUnrelatedUpdatePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackVersionSelectionPolicy {
    MinimalChangeThenHighestCompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackPrereleasePolicy {
    ExplicitOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackDuplicateVersionPolicy {
    RejectDivergentContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackDependencySourcePolicy {
    ExactPublisherOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackUnrelatedUpdatePolicy {
    PreserveLocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackResolutionProjectionDocument {
    pub schema_version: String,
    pub domain_pack_resolution_projection: DomainPackResolutionProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackResolutionProjection {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub status: DomainPackResolutionStatus,
    pub selected: Vec<DomainPackResolvedPackage>,
    pub dependency_edges: Vec<DomainPackResolutionDependencyEdge>,
    pub rejected: Vec<DomainPackRejectedCandidate>,
    pub issues: Vec<DomainPackResolutionIssue>,
    pub resolution_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackResolutionStatus {
    Resolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackResolvedPackage {
    pub identity: DomainPackIdentity,
    pub package: DomainPackPackageBinding,
    pub registry_record_digest: String,
    pub namespace_grant_id: StableId,
    pub source_assurance: DomainPackSourceAssurance,
    /// Independent semantic-review axis. Pure resolution always emits
    /// `Unreviewed`; only the lifecycle TCB may promote an exact reviewed join.
    pub semantic_assurance: DomainPackSemanticAssurance,
    pub reviewed_entry_digest: Option<String>,
    pub promotion_authorization_digest: Option<String>,
    pub dependencies: Vec<DomainPackDependency>,
    pub deterministic_order: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackResolutionDependencyEdge {
    pub from: DomainPackVersionReference,
    pub to: DomainPackVersionReference,
    pub required_content_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackRejectedCandidate {
    pub identity: DomainPackIdentity,
    pub package_digest: String,
    pub reasons: Vec<DomainPackResolutionIssueCode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackResolutionIssue {
    pub code: DomainPackResolutionIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackResolutionIssueCode {
    UnsupportedSchemaVersion,
    InvalidIdentity,
    InvalidDigest,
    InvalidVersionRequirement,
    RegistryDigestMismatch,
    RegistryRecordMissing,
    RegistryRecordMismatch,
    RegistryExpired,
    RegistrySignatureInvalid,
    PublisherSignatureInvalid,
    NamespaceNotGranted,
    RevokedPackage,
    ExplicitlyUntrusted,
    DuplicateVersionEquivocation,
    MissingRoot,
    MissingDependency,
    IncompatibleDependency,
    DependencyCycle,
    DeclaredConflict,
    PrereleaseNotExplicit,
    CurrentLockMismatch,
    ResourceLimitExceeded,
    CompositionBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackExactLockDocument {
    pub schema_version: String,
    pub domain_pack_exact_lock: DomainPackExactLock,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackExactLock {
    pub payload: DomainPackExactLockPayload,
    pub lock_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackExactLockPayload {
    pub project_id: StableId,
    pub core: DomainPackCoreBinding,
    pub requirements_digest: String,
    pub roots: Vec<DomainPackResolutionRoot>,
    pub registry_snapshot_digest: String,
    pub reviewer_registry_digest: String,
    pub reviewed_registry_digest: String,
    pub trust_policy_digest: String,
    pub capability_registry_digest: String,
    pub sandbox_policy_digest: String,
    pub resolution_digest: String,
    pub composition_digest: String,
    pub packages: Vec<DomainPackLockedPackage>,
    pub verified_capability_bindings: Vec<DomainPackLockedCapabilityBinding>,
    pub unresolved_composition_gaps: Vec<DomainPackCompositionGap>,
    pub unresolved_capability_gaps: Vec<DomainPackRuntimeCapabilityGap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackLockedPackage {
    pub identity: DomainPackIdentity,
    pub package_digest: String,
    pub manifest_binding: DomainPackArtifactBinding,
    pub content_binding: DomainPackContentBinding,
    pub license_binding: DomainPackArtifactBinding,
    pub fixture_bindings: Vec<DomainPackArtifactBinding>,
    pub namespace_grant_id: StableId,
    pub registry_record_digest: String,
    pub source_assurance: DomainPackSourceAssurance,
    pub semantic_assurance: DomainPackSemanticAssurance,
    pub reviewed_entry_digest: Option<String>,
    pub promotion_authorization_digest: Option<String>,
    pub dependencies: Vec<DomainPackDependency>,
    pub deterministic_order: u32,
}
