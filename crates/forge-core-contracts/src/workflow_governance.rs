//! Closed P5b policy and untrusted-observation contracts for workflow governance.
//!
//! Raw documents are simulation input only. Policies define outcomes, evidence
//! rules, capability requirements, and irreducible decisions; verified
//! authority exists only behind the mutation kernel's opaque trusted-snapshot
//! boundary. Advisory playbooks are carried for agent leverage but deliberately
//! have no field capable of authorizing progression or done.

use crate::assurance::{
    CapabilityGapKind, DecisionAlternative, HumanDecisionReason, ObligationCriticality,
    ReadinessTarget,
};
use crate::common::StableId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const WORKFLOW_GOVERNANCE_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceBundleDocument {
    pub schema_version: String,
    pub workflow_governance_bundle: WorkflowGovernanceBundle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceBundle {
    pub id: StableId,
    pub policies: Vec<WorkflowGovernancePolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernancePolicy {
    pub id: StableId,
    pub compatibility_workflow_id: StableId,
    pub eligible_phases: Vec<StableId>,
    #[serde(default)]
    pub prerequisite_policy_refs: Vec<StableId>,
    pub obligations: Vec<WorkflowObligationPolicy>,
    pub claims: Vec<WorkflowClaimPolicy>,
    pub evaluators: Vec<WorkflowEvaluatorBinding>,
    #[serde(default)]
    pub capability_requirements: Vec<WorkflowCapabilityRequirement>,
    #[serde(default)]
    pub decision_rules: Vec<WorkflowDecisionRule>,
    pub advisory_playbook: AdvisoryWorkflowPlaybook,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowObligationPolicy {
    pub id: StableId,
    pub description: String,
    pub criticality: ObligationCriticality,
    pub claim_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowClaimPolicy {
    pub id: StableId,
    pub statement: String,
    pub evaluator_ref: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvaluatorBinding {
    pub id: StableId,
    pub accepted_evidence_kinds: Vec<WorkflowEvidenceKind>,
    pub minimum_strength: WorkflowEvidenceStrength,
    pub minimum_passing_observations: usize,
    pub freshness: WorkflowFreshnessRequirement,
    pub disproof_policy: WorkflowDisproofPolicy,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvidenceKind {
    ArtifactInspection,
    DeterministicCheck,
    RepresentativeExecution,
    IndependentReview,
    HumanAcceptance,
    ExternalAuthority,
    Research,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
/// Ordered weakest-to-strongest. Declaration order is part of the evaluator
/// contract and must not change without a schema version change.
pub enum WorkflowEvidenceStrength {
    ArtifactPresence,
    InspectedArtifact,
    DeterministicVerification,
    RepresentativeExecution,
    IndependentConfirmation,
    AuthoritativeAcceptance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowFreshnessRequirement {
    CurrentOnly,
    StaleAllowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDisproofPolicy {
    RejectAnyDisproof,
    RequireUncontestedSupport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCapabilityRequirement {
    pub id: StableId,
    pub kind: CapabilityGapKind,
    pub description: String,
    pub affected_claim_refs: Vec<StableId>,
    pub resolution_options: Vec<String>,
    pub blocks_before: ReadinessTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDecisionRule {
    pub id: StableId,
    pub activation: WorkflowDecisionActivation,
    #[serde(default)]
    pub claim_ref: Option<StableId>,
    pub question: String,
    pub reason: HumanDecisionReason,
    pub alternatives: Vec<DecisionAlternative>,
    pub recommended_alternative_ref: StableId,
    pub blocking: bool,
    pub blocks_before: ReadinessTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDecisionActivation {
    ObservedNeed,
    ClaimUnresolved,
    ClaimDisproven,
}

/// Non-authoritative strategy projection. No eligibility, completion, or
/// mutation field exists here by design.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdvisoryWorkflowPlaybook {
    pub id: StableId,
    #[serde(default)]
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceEvaluationDocument {
    pub schema_version: String,
    pub workflow_governance_evaluation: WorkflowGovernanceEvaluation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceEvaluation {
    pub observation_set_id: StableId,
    pub state_version: u64,
    pub bundle_id: StableId,
    pub policy_id: StableId,
    pub current_phase: StableId,
    pub target: ReadinessTarget,
    #[serde(default)]
    pub completed_policy_refs: Vec<StableId>,
    #[serde(default)]
    pub available_capability_refs: Vec<StableId>,
    #[serde(default)]
    pub decision_need_refs: Vec<StableId>,
    #[serde(default)]
    pub resolved_decision_refs: Vec<StableId>,
    #[serde(default)]
    pub evidence: Vec<WorkflowEvidenceObservation>,
    pub completion_assertion: WorkflowCompletionAssertion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvidenceObservation {
    pub evidence_ref: String,
    pub claim_ref: StableId,
    pub evaluator_ref: StableId,
    pub kind: WorkflowEvidenceKind,
    pub strength: WorkflowEvidenceStrength,
    pub freshness: WorkflowEvidenceFreshness,
    pub outcome: WorkflowEvidenceOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvidenceFreshness {
    Current,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvidenceOutcome {
    Pass,
    Fail,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCompletionAssertion {
    NotAsserted,
    Asserted,
}
