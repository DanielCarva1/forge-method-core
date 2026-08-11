//! Honest admission contracts for same-owner cooperative evidence.
//!
//! These DTOs describe a bounded same-owner attestation lane. They cannot
//! establish reviewer independence, trusted-runtime separation, tamper
//! resistance, human presence, or compliance authority.

use crate::command::CommandExecutor;
use crate::{
    CommandContractDocument, PrincipalId, StableId, WorkflowContentAddressedReference,
    WorkflowEvaluatorProvider, WorkflowEvidenceKind, WorkflowEvidenceOutcome,
    WorkflowEvidenceStrength, WorkflowEvidenceSubject, WorkflowEvidenceSubjectKind,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION_V1: &str = "cooperative_evidence_offer_v1";
pub const COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION_V1: &str =
    "cooperative_evidence_attestation_v1";
pub const SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION_V1: &str =
    "solo_cooperative_project_snapshot_v1";
pub const SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION_V1: &str =
    "solo_cooperative_project_snapshot_claim_v1";
pub const COOPERATIVE_EVIDENCE_OFFER_SCHEMA_VERSION: &str = "cooperative_evidence_offer_v2";
pub const COOPERATIVE_EVIDENCE_ATTESTATION_SCHEMA_VERSION: &str =
    "cooperative_evidence_attestation_v2";
pub const COOPERATIVE_EXECUTION_OFFER_SCHEMA_VERSION: &str =
    "cooperative_evidence_execution_offer_v1";
pub const COOPERATIVE_EXECUTION_ATTESTATION_SCHEMA_VERSION: &str =
    "cooperative_evidence_execution_attestation_v1";
pub const COOPERATIVE_APPLICABILITY_OFFER_SCHEMA_VERSION: &str =
    "cooperative_evidence_applicability_offer_v1";
pub const COOPERATIVE_APPLICABILITY_ATTESTATION_SCHEMA_VERSION: &str =
    "cooperative_evidence_applicability_attestation_v1";
pub const SOLO_COOPERATIVE_EVIDENCE_POLICY_VERSION: &str =
    "solo_cooperative_repository_inspection_v1";
pub const SOLO_COOPERATIVE_CLAIM_DESCRIPTOR_VERSION: &str =
    "solo_cooperative_repository_source_claim_v1";
pub const SOLO_COOPERATIVE_EXECUTION_POLICY_VERSION: &str =
    "solo_cooperative_deterministic_execution_v1";
pub const SOLO_COOPERATIVE_EXECUTION_DESCRIPTOR_VERSION: &str =
    "solo_cooperative_deterministic_source_claim_v1";
pub const SOLO_COOPERATIVE_APPLICABILITY_POLICY_VERSION: &str =
    "solo_cooperative_policy_applicability_v1";
pub const SOLO_COOPERATIVE_APPLICABILITY_DESCRIPTOR_VERSION: &str =
    "solo_cooperative_policy_applicability_claim_v1";
pub const MAX_WORKFLOW_COOPERATIVE_EVIDENCE_INPUT_BYTES: usize = 128 * 1024;
pub const MAX_WORKFLOW_COOPERATIVE_EVIDENCE_TEXT_BYTES: usize = 2 * 1024;
pub const MAX_WORKFLOW_COOPERATIVE_EVIDENCE_BASIS_ITEMS: usize = 16;
pub const MAX_WORKFLOW_COOPERATIVE_EVIDENCE_BASIS_FILE_BYTES: usize = 1024 * 1024;
pub const MAX_WORKFLOW_COOPERATIVE_EVIDENCE_BASIS_TOTAL_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_WORKFLOW_COOPERATIVE_EVIDENCE_LIMITATIONS: usize = 16;

/// Exact current coordinates published by `workflow next` and rechecked by the
/// kernel immediately before append.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeEvidenceBinding {
    pub objective_id: StableId,
    pub objective_revision: u64,
    pub objective_digest: String,
    pub assurance_epoch: u64,
    pub accepted_objective_record_digest: String,
    pub accepted_objective_record_sequence: u64,
    pub policy_bundle_digest: String,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub state_version: u64,
}

/// Kernel-derived, versioned policy route published to generic hosts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeEvidenceRoute {
    pub policy_version: String,
    pub claim_descriptor_version: String,
    pub target: WorkflowCooperativeEvidenceTarget,
    /// Currently selected source policy/claim. The cooperative descriptor is
    /// bound to it but explicitly does not satisfy it.
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub evaluator_ref: StableId,
    pub source_provider: WorkflowEvaluatorProvider,
    pub cooperative_claim_ref: StableId,
    pub cooperative_evaluator_ref: StableId,
    pub producer: PrincipalId,
    pub provider: WorkflowEvaluatorProvider,
    pub kind: WorkflowEvidenceKind,
    pub strength: WorkflowEvidenceStrength,
    pub allowed_subject_kinds: Vec<WorkflowEvidenceSubjectKind>,
    pub subject_ref: String,
    pub scenario_digest: String,
    pub max_age_seconds: u64,
    pub assurance_effect: WorkflowCooperativeEvidenceAssuranceEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCooperativeEvidenceTarget {
    SourceClaim,
    PolicyApplicability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCooperativeApplicabilityOutcome {
    Applicable,
    NotApplicable,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCooperativeEvidenceAssuranceEffect {
    CooperativeClaimOnlyDoesNotSatisfySourceClaim,
    SoloSourceClaimSatisfiedByAgentInspection,
    SoloPolicyApplicabilityAssessedByAgentInspection,
    SoloSourceClaimSatisfiedByKernelExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCooperativeMaterialScenarioKind {
    KernelProjectSnapshotReadback,
    AgentRepositoryInspectionWithContentAddressedBasis,
    AgentPolicyApplicabilityInspectionWithContentAddressedBasis,
    KernelDeterministicCommandExecution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeExecutionRequest {
    pub summary: String,
    pub scenario_ref: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
    pub command: CommandContractDocument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCooperativeExecutionStatus {
    Succeeded,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAdmittedCooperativeExecutionAssessment {
    pub summary: String,
    pub scenario_ref: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
    pub command_digest: String,
    pub command_id: StableId,
    pub executor: CommandExecutor,
    pub status: WorkflowCooperativeExecutionStatus,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeSourceAssessmentOffer {
    pub outcome: WorkflowEvidenceOutcome,
    pub summary: String,
    pub basis_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAdmittedCooperativeSourceAssessment {
    pub outcome: WorkflowEvidenceOutcome,
    pub summary: String,
    pub basis: Vec<WorkflowContentAddressedReference>,
    pub basis_digest: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeApplicabilityAssessmentOffer {
    pub outcome: WorkflowCooperativeApplicabilityOutcome,
    pub summary: String,
    pub basis_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAdmittedCooperativeApplicabilityAssessment {
    pub outcome: WorkflowCooperativeApplicabilityOutcome,
    pub summary: String,
    pub basis: Vec<WorkflowContentAddressedReference>,
    pub basis_digest: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

/// Closed same-owner statement carried in the offer. The kernel derives the
/// outcome by executing the descriptor's material scenario and readback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeEvidenceAttestation {
    pub schema_version: String,
    pub policy_version: String,
    pub claim_descriptor_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<WorkflowCooperativeEvidenceTarget>,
    pub binding: WorkflowCooperativeEvidenceBinding,
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub evaluator_ref: StableId,
    pub cooperative_claim_ref: StableId,
    pub cooperative_evaluator_ref: StableId,
    pub producer: PrincipalId,
    pub subject: WorkflowEvidenceSubject,
    pub scenario_kind: WorkflowCooperativeMaterialScenarioKind,
    pub scenario_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_assessment: Option<WorkflowCooperativeSourceAssessmentOffer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applicability_assessment: Option<WorkflowCooperativeApplicabilityAssessmentOffer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_request: Option<WorkflowCooperativeExecutionRequest>,
}

/// Agent-produced offer. `offer_id` is the idempotency key; reusing it for
/// different canonical bytes fails closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCooperativeEvidenceOffer {
    pub schema_version: String,
    pub offer_id: StableId,
    pub attestation: WorkflowCooperativeEvidenceAttestation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCooperativeEvidenceDisposition {
    Admitted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCooperativeEvidenceRejection {
    MalformedOrOversizedOffer,
    UnsupportedSchema,
    PolicyDoesNotPermitCooperation,
    IndependentOrExternalClaim,
    BindingStale,
    WrongProducer,
    WrongSubject,
    SubjectDigestMismatch,
    WrongScenario,
    MissingRepresentativeExecution,
    MissingAuthoritativeReadback,
    FabricatedOrMalformedReceipt,
    EvidenceExpired,
    ConflictingIdempotencyKey,
    InvalidAssessmentBasis,
    UnsafeOrInvalidCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCooperativeEvidenceCurrentStatus {
    Supporting,
    Disproving,
    Inconclusive,
    Stale,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCooperativeEvidenceProof {
    SoloCooperativeClaimSatisfied,
    KernelExecutedProjectSnapshotScenario,
    KernelVerifiedProjectStateReadback,
    SoloSourceClaimSatisfiedByAgentInspection,
    KernelVerifiedContentAddressedBasis,
    SoloPolicyApplicabilityAssessedByAgentInspection,
    SoloSourceClaimSatisfiedByKernelExecution,
    KernelExecutedDeterministicCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCooperativeEvidenceNonProof {
    IndependentSemanticReview,
    TrustedRuntimeSeparation,
    TamperResistance,
    HumanPresence,
    EnterpriseCompliance,
    SelectedSourceClaim,
    SelectedRepresentativeRuntimeClaim,
    IndependentRepositoryInspection,
    HumanApplicabilityJudgment,
    PolicyClaimSatisfaction,
    CapabilitySatisfaction,
}

/// Bounded normalized content retained only for admitted offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAdmittedCooperativeEvidence {
    pub offer_id: StableId,
    pub offer_digest: String,
    pub policy_version: String,
    pub claim_descriptor_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<WorkflowCooperativeEvidenceTarget>,
    pub binding: WorkflowCooperativeEvidenceBinding,
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub evaluator_ref: StableId,
    pub cooperative_claim_ref: StableId,
    pub cooperative_evaluator_ref: StableId,
    pub producer: PrincipalId,
    pub subject: WorkflowEvidenceSubject,
    pub scenario_kind: WorkflowCooperativeMaterialScenarioKind,
    pub scenario_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_assessment: Option<WorkflowAdmittedCooperativeSourceAssessment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applicability_assessment: Option<WorkflowAdmittedCooperativeApplicabilityAssessment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_assessment: Option<WorkflowAdmittedCooperativeExecutionAssessment>,
    pub outcome: WorkflowEvidenceOutcome,
    pub execution_observed_at_unix: u64,
    pub readback_observed_at_unix: u64,
}
