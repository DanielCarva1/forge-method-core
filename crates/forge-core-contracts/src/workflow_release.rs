//! Closed P5d.1 contracts for versioned core workflow-governance rollout.
//!
//! These documents describe release and migration intent only. Deserializing a
//! manifest, batch, or retirement authorization never admits executable or
//! retirement authority; those final states must be derived by later trusted
//! validation and signature-verification layers.

use crate::common::{PrincipalId, RepoPath, StableId};
use crate::workflow_governance::WorkflowGovernancePolicy;
use crate::workflow_migration::WorkflowCompatibilityField;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const WORKFLOW_GOVERNANCE_RELEASE_MANIFEST_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_MIGRATION_BATCH_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_RETIREMENT_AUTHORIZATION_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_GOVERNANCE_RELEASE_REGISTRY_SCHEMA_VERSION: &str = "0.1";

/// Repository-owned release discovery. This is raw, non-authoritative input:
/// only the trusted runtime loader may turn one validated entry into a project
/// pin or an upgrade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceReleaseRegistryDocument {
    pub schema_version: String,
    pub workflow_governance_release_registry: WorkflowGovernanceReleaseRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceReleaseRegistry {
    pub registry_id: StableId,
    pub registry_version: String,
    pub lineage_id: StableId,
    pub default_successor_release_id: StableId,
    pub releases: Vec<WorkflowGovernanceReleaseRegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceReleaseRegistryEntry {
    pub release: WorkflowGovernanceReleaseIdentity,
    pub runtime_bundle: WorkflowRuntimeBundleReference,
    #[serde(default)]
    pub predecessor: Option<WorkflowReleasePredecessorReference>,
    pub source: WorkflowReleaseRegistrySource,
    pub receipt_carryover: WorkflowReceiptCarryover,
    pub authority: WorkflowReleaseRegistryAuthority,
}

/// Stable release identity. The digest identifies the release subject, not the
/// registry document that happened to publish it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceReleaseIdentity {
    pub lineage_id: StableId,
    pub release_id: StableId,
    pub release_version: String,
    /// Canonical JCS digest of the release descriptor subject. Raw embedded
    /// byte integrity is bound separately by the source reference.
    pub release_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRuntimeBundleIdentity {
    pub bundle_id: StableId,
    pub bundle_digest: String,
    /// Canonical digest of the ordered policy objects only. This remains
    /// stable when an equivalent release changes the enclosing bundle id.
    pub policy_set_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRuntimeBundleReference {
    pub identity: WorkflowRuntimeBundleIdentity,
    pub embedded_ref: RepoPath,
    pub expected_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleasePredecessorReference {
    pub release_id: StableId,
    pub release_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowReleaseRegistrySource {
    /// Compatibility mapping for ledgers created before explicit releases.
    ImplicitP5cGenesis,
    EmbeddedManifest {
        embedded_ref: RepoPath,
        /// SHA-256 of the exact embedded YAML bytes, not the canonical release
        /// identity digest.
        expected_digest: String,
    },
}

/// A registry document can describe candidates but cannot admit them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseRegistryAuthority {
    CandidateOnly,
}

/// Receipt handling requested by an upgrade. The trusted upgrader must still
/// prove that the selected policy set permits the requested strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReceiptCarryover {
    NotApplicable,
    InvalidateAll,
    PreservePolicyEquivalent,
}

/// Repository-owned intent for one versioned, composed core-governance release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceReleaseManifestDocument {
    pub schema_version: String,
    pub workflow_governance_release_manifest: WorkflowGovernanceReleaseManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceReleaseManifest {
    /// Stable identity across releases. This is distinct from a release digest.
    pub lineage_id: StableId,
    pub release_id: StableId,
    pub release_version: String,
    #[serde(default)]
    pub previous_release_digest: Option<String>,
    pub legacy_catalog_digest: String,
    /// Deterministically ordered, content-addressed candidate batches.
    pub batches: Vec<WorkflowReleaseBatchReference>,
    /// Explicit exhaustive disposition intent; semantic validation proves
    /// catalog completeness and uniqueness rather than applying a fallback.
    pub workflow_entries: Vec<WorkflowReleaseWorkflowEntry>,
    pub compatibility_policy: WorkflowReleaseCompatibilityPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseBatchReference {
    pub batch_id: StableId,
    pub batch_version: String,
    pub embedded_ref: RepoPath,
    pub expected_digest: String,
    pub deterministic_order: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseWorkflowEntry {
    pub workflow_id: StableId,
    pub legacy_workflow_digest: String,
    pub disposition_intent: WorkflowReleaseDispositionIntent,
}

/// Authored rollout intent. Deliberately has no `Executable` or `Retired`
/// variant: trusted evaluators derive those final states from complete gates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowReleaseDispositionIntent {
    MigrationCandidate {
        batch_id: StableId,
        policy_ref: StableId,
    },
    CompatibilityOnly {
        reason: WorkflowCompatibilityReason,
    },
    Quarantined {
        quarantine: WorkflowQuarantine,
    },
    DomainPackCandidate {
        candidate: WorkflowDomainPackCandidate,
    },
    RetirementCandidate {
        replacement_policy_ref: StableId,
        authorization: WorkflowRetirementAuthorizationReference,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCompatibilityReason {
    pub code: WorkflowCompatibilityReasonCode,
    pub explanation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCompatibilityReasonCode {
    AwaitingMigration,
    AdvisoryOnly,
    ConsumerCompatibilityWindow,
    ExplicitlyDeferred,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowQuarantine {
    pub reason_code: WorkflowQuarantineReasonCode,
    pub risk_tier: WorkflowQuarantineRiskTier,
    pub explanation: String,
    pub blocking_refs: Vec<StableId>,
    pub affected_consumer_refs: Vec<StableId>,
    pub review_owner: StableId,
    pub review_due_release_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowQuarantineReasonCode {
    AmbiguousLegacyAuthority,
    MissingEvidenceRule,
    MissingEvaluator,
    DanglingDependency,
    UnsafeAutomaticConversion,
    UnknownSemantics,
    ConsumerCompatibilityRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowQuarantineRiskTier {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDomainPackCandidate {
    pub domain_id: StableId,
    pub proposed_pack_id: StableId,
    pub deferral_reason: WorkflowDomainPackDeferralReason,
    pub explanation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDomainPackDeferralReason {
    DomainSpecificKnowledge,
    DomainSpecificEvaluator,
    DomainSpecificLifecycle,
    DomainSpecificAdapter,
}

/// Compatibility behavior for workflows not yet derived executable/retired.
///
/// Even a `retired` lifecycle is only release intent bound to an authorization
/// reference. It never grants retirement authority without later trusted
/// semantic and signature verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowReleaseCompatibilityPolicy {
    pub policy_version: String,
    pub lifecycle: WorkflowCompatibilityLifecycle,
    pub diagnostic_code: StableId,
    /// Structured replacement command. Semantic validation requires at least
    /// one non-blank argument; this contract only closes its shape.
    pub replacement_argv: Vec<String>,
    pub projection_mode: WorkflowReleaseCompatibilityProjectionMode,
    pub legacy_authority: WorkflowLegacyCompatibilityAuthority,
    pub exact_fields: Vec<WorkflowCompatibilityField>,
    pub consumer_diagnostics: WorkflowConsumerDiagnosticsPolicy,
    pub minimum_consumer_version: String,
    pub retirement_admission: WorkflowRetirementAdmissionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowCompatibilityLifecycle {
    Supported,
    Deprecated {
        announced_at_unix: u64,
        removal_not_before_unix: u64,
    },
    Retired {
        authorization_ref: WorkflowRetirementAuthorizationReference,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReleaseCompatibilityProjectionMode {
    ReadOnlyExactProjection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowLegacyCompatibilityAuthority {
    NonAuthoritative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowConsumerDiagnosticsPolicy {
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRetirementAdmissionPolicy {
    VerifiedAuthorizationRequired,
}

/// Candidate migration batch. The closed authority marker prevents a raw batch
/// from claiming runtime admission merely by being well-formed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationBatchDocument {
    pub schema_version: String,
    pub workflow_migration_batch: WorkflowMigrationBatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationBatch {
    pub id: StableId,
    pub batch_version: String,
    pub authority: WorkflowMigrationBatchAuthority,
    pub source_catalog_digest: String,
    #[serde(default)]
    pub previous_batch_digest: Option<String>,
    pub evidence: WorkflowMigrationBatchEvidence,
    pub workflow_bindings: Vec<WorkflowMigrationBatchBinding>,
    pub policies: Vec<WorkflowGovernancePolicy>,
}

/// Content-addressed evidence proposed with a candidate batch. Presence in a
/// well-formed batch is not evidence validity or runtime admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationBatchEvidence {
    pub representative_fixtures: Vec<WorkflowMigrationEvidenceReference>,
    pub adversarial_fixtures: Vec<WorkflowMigrationEvidenceReference>,
    pub shadow_reports: Vec<WorkflowMigrationEvidenceReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationEvidenceReference {
    pub embedded_ref: RepoPath,
    pub expected_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMigrationBatchAuthority {
    CandidateOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationBatchBinding {
    pub workflow_id: StableId,
    pub legacy_workflow_digest: String,
    pub policy_ref: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementAuthorizationReference {
    pub authorization_id: StableId,
    pub embedded_ref: RepoPath,
    pub expected_digest: String,
}

/// Signed product-level authorization proposed as one retirement gate.
/// Cryptographic and semantic verification deliberately belongs to a later
/// trusted authority layer; this contract only closes and binds the payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementAuthorizationDocument {
    pub schema_version: String,
    pub workflow_retirement_authorization: WorkflowRetirementAuthorization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementAuthorization {
    pub id: StableId,
    pub workflow_id: StableId,
    pub legacy_workflow_digest: String,
    pub replacement_policy_ref: StableId,
    pub replacement_policy_digest: String,
    pub governance_release_digest: String,
    pub evidence: WorkflowRetirementEvidenceBinding,
    pub compatibility_window: WorkflowRetirementCompatibilityWindow,
    pub reviewer: WorkflowRetirementReviewer,
    pub signature: WorkflowRetirementSignatureEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementEvidenceBinding {
    pub executable_coverage_digest: String,
    pub shadow_evidence_digest: String,
    pub deletion_test_digest: String,
    pub consumer_compatibility_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementCompatibilityWindow {
    pub announced_at_unix: u64,
    pub retirement_not_before_unix: u64,
    pub minimum_consumer_version: String,
    pub diagnostics_evidence_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementReviewer {
    pub principal_id: PrincipalId,
    pub credential_id: StableId,
    pub authority_scope: StableId,
    pub registry_digest: String,
    pub public_key_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementSignatureEnvelope {
    pub algorithm: WorkflowRetirementSignatureAlgorithm,
    pub audience: String,
    pub nonce: String,
    pub intent_digest: String,
    pub attestation_digest: String,
    pub signature: String,
    pub signed_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRetirementSignatureAlgorithm {
    Ed25519,
}
