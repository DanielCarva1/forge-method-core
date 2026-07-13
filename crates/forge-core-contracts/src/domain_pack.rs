//! Closed, candidate-only contracts for P6 Domain Pack composition.
//!
//! These types describe knowledge and deterministic composition input. They
//! deliberately cannot represent installation, activation, trust, execution,
//! or mutation authority. Those lifecycle transitions belong to a later
//! trusted boundary.

use crate::common::{RepoPath, StableId};
use crate::workflow_governance::{
    WorkflowEvaluatorProvider, WorkflowEvidenceKind, WorkflowEvidenceStrength,
    WorkflowGovernanceBundle, WorkflowGovernancePolicyOverlay,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DOMAIN_PACK_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackManifestDocument {
    pub schema_version: String,
    pub domain_pack_manifest: DomainPackManifest,
}

/// Authored package metadata. A valid document remains candidate-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackManifest {
    pub identity: DomainPackIdentity,
    pub authority: DomainPackCandidateAuthority,
    pub compatibility: DomainPackCompatibility,
    pub provenance: DomainPackProvenance,
    pub content: DomainPackContentBinding,
    pub dependencies: Vec<DomainPackDependency>,
    pub conflicts: Vec<DomainPackConflict>,
    pub replacement_slots: Vec<DomainPackReplacementSlot>,
    pub replacement_declarations: Vec<DomainPackReplacementDeclaration>,
}

/// Stable package coordinate plus the namespace it exclusively proposes to
/// own. Semantic validation, rather than deserialization, checks namespace
/// syntax and ownership.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackIdentity {
    pub publisher: StableId,
    pub name: StableId,
    pub namespace: StableId,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCoordinate {
    pub publisher: StableId,
    pub name: StableId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackCandidateAuthority {
    CandidateOnly,
}

/// Version requirements remain strings on the contract wire. The pure P6a
/// composer owns their strict semantic-version parsing and comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCompatibility {
    pub pack_schema_requirement: String,
    pub forge_core_requirement: String,
}

/// Declarative source metadata only. It is not a signature, review result, or
/// trusted registry assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackProvenance {
    pub source_kind: DomainPackSourceKind,
    pub source_uri: String,
    pub source_revision: String,
    pub source_digest: String,
    pub authors: Vec<StableId>,
    pub license_spdx_expression: String,
    pub license_text: DomainPackArtifactBinding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackSourceKind {
    Repository,
    Registry,
    LocalCandidate,
}

/// Raw bytes and canonical semantics are bound separately so neither YAML
/// rewriting nor semantic drift is hidden by one digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackArtifactBinding {
    pub artifact_ref: RepoPath,
    pub raw_sha256: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackContentBinding {
    pub content_ref: RepoPath,
    pub raw_sha256: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackDependency {
    pub pack: DomainPackCoordinate,
    pub version_requirement: String,
    /// When present, resolution must select these exact canonical semantics.
    pub required_content_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackConflict {
    pub pack: DomainPackCoordinate,
    pub version_requirement: String,
    pub reason: DomainPackConflictReason,
    pub explanation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackConflictReason {
    SemanticIncompatibility,
    NamespaceOwnership,
    EvaluatorIncompatibility,
    AdapterIncompatibility,
    LifecycleIncompatibility,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackContributionKind {
    Policy,
    Obligation,
    Claim,
    Playbook,
    Hazard,
    LifecycleModel,
    Evaluator,
    Fixture,
    Capability,
    Adapter,
    Domain,
}

/// Target-side opt-in to replacement. A replacement is valid only when a
/// matching source-side declaration names this slot and exact target digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReplacementSlot {
    pub id: StableId,
    pub contribution_kind: DomainPackContributionKind,
    pub target_ref: StableId,
    pub target_digest: String,
    pub allowed_replacers: Vec<DomainPackCoordinate>,
    pub replacement_version_requirement: String,
}

/// Source-side half of a bilateral replacement agreement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackReplacementDeclaration {
    pub target_pack: DomainPackCoordinate,
    pub target_slot_ref: StableId,
    pub contribution_kind: DomainPackContributionKind,
    pub target_ref: StableId,
    pub target_digest: String,
    pub replacement_ref: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackContentDocument {
    pub schema_version: String,
    pub domain_pack_content: DomainPackContent,
}

/// Closed knowledge payload. The workflow overlay reuses the already-closed
/// policy, obligation, claim, evaluator, capability, and advisory-playbook
/// vocabulary; all additional domain surfaces are declaration-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackContent {
    pub pack: DomainPackVersionReference,
    pub namespace: StableId,
    pub workflow_overlay: WorkflowGovernancePolicyOverlay,
    pub provided_domains: Vec<DomainPackProvidedDomain>,
    pub provided_capabilities: Vec<DomainPackProvidedCapability>,
    pub hazards: Vec<DomainHazard>,
    pub lifecycle_models: Vec<DomainLifecycleModel>,
    pub evaluators: Vec<DomainEvaluatorDeclaration>,
    pub fixtures: Vec<DomainFixtureReference>,
    pub adapters: Vec<DomainAdapterDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackVersionReference {
    pub publisher: StableId,
    pub name: StableId,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackProvidedDomain {
    pub id: StableId,
    pub description: String,
    pub policy_refs: Vec<StableId>,
    pub hazard_refs: Vec<StableId>,
    pub lifecycle_model_refs: Vec<StableId>,
}

/// Declaring a capability does not prove that any runtime currently has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackProvidedCapability {
    pub id: StableId,
    pub kind: DomainPackCapabilityKind,
    pub description: String,
    pub evidence_rule_refs: Vec<StableId>,
    pub authority: DomainCapabilityDeclarationAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackCapabilityKind {
    Knowledge,
    Evaluator,
    Adapter,
    Tool,
    Runtime,
    Credential,
    HumanReview,
    ExternalAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainCapabilityDeclarationAuthority {
    DeclarationOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainHazard {
    pub id: StableId,
    pub category: DomainHazardCategory,
    pub severity: DomainHazardSeverity,
    pub description: String,
    pub trigger_refs: Vec<StableId>,
    pub mitigation_obligation_refs: Vec<StableId>,
    pub evidence_claim_refs: Vec<StableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainHazardCategory {
    Safety,
    Security,
    Privacy,
    Legal,
    Reliability,
    Quality,
    Accessibility,
    DomainSpecific,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DomainHazardSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainLifecycleModel {
    pub id: StableId,
    pub description: String,
    pub initial_state_ref: StableId,
    pub terminal_state_refs: Vec<StableId>,
    pub states: Vec<DomainLifecycleState>,
    pub transitions: Vec<DomainLifecycleTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainLifecycleState {
    pub id: StableId,
    pub description: String,
    pub entry_obligation_refs: Vec<StableId>,
    pub exit_claim_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainLifecycleTransition {
    pub id: StableId,
    pub from_state_ref: StableId,
    pub to_state_ref: StableId,
    pub guard_claim_refs: Vec<StableId>,
    pub required_capability_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainEvaluatorDeclaration {
    pub id: StableId,
    pub implementation: DomainEvaluatorImplementation,
    pub accepted_evidence_kinds: Vec<WorkflowEvidenceKind>,
    pub minimum_strength: WorkflowEvidenceStrength,
    pub authority: DomainEvaluatorDeclarationAuthority,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainEvaluatorImplementation {
    BuiltIn {
        provider: WorkflowEvaluatorProvider,
    },
    Adapter {
        adapter_ref: StableId,
        protocol: DomainAdapterProtocol,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainEvaluatorDeclarationAuthority {
    DeclarationOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainFixtureReference {
    pub id: StableId,
    pub kind: DomainFixtureKind,
    pub artifact: DomainPackArtifactBinding,
    pub subject_refs: Vec<StableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainFixtureKind {
    Representative,
    Adversarial,
    Regression,
    Ablation,
    Compatibility,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainAdapterDeclaration {
    pub id: StableId,
    pub protocol: DomainAdapterProtocol,
    pub surface: StableId,
    pub required_capability_refs: Vec<StableId>,
    pub authority: DomainAdapterDeclarationAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainAdapterProtocol {
    BuiltIn,
    LocalProcess,
    Mcp,
    RuntimeHandshake,
    ExternalConnector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainAdapterDeclarationAuthority {
    DeclarationOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackProjectRequirementsDocument {
    pub schema_version: String,
    pub domain_pack_project_requirements: DomainPackProjectRequirements,
}

/// Durable desired domain surface. Keeping this requirement after a pack is
/// removed lets composition emit an explicit missing-domain gap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackProjectRequirements {
    pub project_id: StableId,
    pub requirement_set_id: StableId,
    pub required_domains: Vec<DomainPackDomainRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackDomainRequirement {
    pub id: StableId,
    pub domain_id: StableId,
    pub pack_version_requirement: String,
    pub required_capability_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCompositionRequestDocument {
    pub schema_version: String,
    pub domain_pack_composition_request: DomainPackCompositionRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCompositionRequest {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub forge_core_version: String,
    pub core: DomainPackCoreBinding,
    pub requirements: DomainPackProjectRequirements,
    pub candidates: Vec<DomainPackCandidateInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCoreBinding {
    pub bundle_id: StableId,
    pub bundle_digest: String,
    pub policy_set_digest: String,
    pub bundle: WorkflowGovernanceBundle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCandidateInput {
    /// Exact authored manifest sidecar. The embedded typed manifest is the
    /// closed semantic view; this binding proves which raw bytes produced it.
    pub manifest_binding: DomainPackArtifactBinding,
    pub manifest: DomainPackManifestDocument,
    pub content: DomainPackContentDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCompositionProjectionDocument {
    pub schema_version: String,
    pub domain_pack_composition_projection: DomainPackCompositionProjection,
}

/// Derived, auditable candidate projection. Even `composable` is structural
/// readiness only; it cannot activate packs or authorize governed execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCompositionProjection {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub status: DomainPackCompositionStatus,
    pub core_bundle_digest: String,
    pub ordered_packs: Vec<DomainPackComposedIdentity>,
    pub contribution_index: Vec<DomainPackContributionIndexEntry>,
    pub provided_domain_refs: Vec<StableId>,
    pub declared_capability_refs: Vec<StableId>,
    pub composed_bundle: Option<WorkflowGovernanceBundle>,
    pub gaps: Vec<DomainPackCompositionGap>,
    pub issues: Vec<DomainPackCompositionIssue>,
    pub composition_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackCompositionStatus {
    Composable,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackComposedIdentity {
    pub identity: DomainPackIdentity,
    pub content_digest: String,
    pub manifest_digest: String,
    pub deterministic_order: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackContributionIndexEntry {
    pub pack: DomainPackVersionReference,
    pub kind: DomainPackContributionKind,
    pub contribution_ref: StableId,
    pub contribution_digest: String,
    pub replaces_ref: Option<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCompositionGap {
    pub code: DomainPackCompositionGapCode,
    pub requirement_ref: StableId,
    pub subject_ref: StableId,
    pub message: String,
    pub authority: DomainPackCandidateAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackCompositionGapCode {
    MissingDomain,
    MissingDependency,
    MissingCapability,
    MissingEvaluator,
    MissingAdapter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCompositionIssue {
    pub code: DomainPackCompositionIssueCode,
    pub path: String,
    pub message: String,
    pub authority: DomainPackCandidateAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackCompositionIssueCode {
    ResourceLimitExceeded,
    UnsupportedSchemaVersion,
    InvalidIdentity,
    InvalidProvenance,
    InvalidVersionRequirement,
    ContentBindingMismatch,
    IncompatiblePackSchema,
    IncompatibleForgeCore,
    DuplicatePack,
    DuplicateNamespace,
    DuplicateContribution,
    MissingDependency,
    IncompatibleDependency,
    DependencyCycle,
    DeclaredConflict,
    CoreShadow,
    PackShadow,
    ReplacementNotBilateral,
    ReplacementTargetMismatch,
    DanglingReference,
    InvalidLifecycleModel,
    InvalidCapabilityDeclaration,
    InvalidEvaluatorDeclaration,
    InvalidAdapterDeclaration,
    InvalidComposedBundle,
}
