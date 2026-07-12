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
use crate::common::{PrincipalId, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const WORKFLOW_GOVERNANCE_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION: &str = "0.1";

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
    pub routing: WorkflowPolicyRouting,
    pub eligible_phases: Vec<StableId>,
    #[serde(default)]
    pub prerequisites: Vec<WorkflowPrerequisite>,
    pub obligations: Vec<WorkflowObligationPolicy>,
    pub claims: Vec<WorkflowClaimPolicy>,
    pub evaluators: Vec<WorkflowEvaluatorBinding>,
    #[serde(default)]
    pub capability_requirements: Vec<WorkflowCapabilityRequirement>,
    #[serde(default)]
    pub decision_rules: Vec<WorkflowDecisionRule>,
    pub advisory_playbook: AdvisoryWorkflowPlaybook,
}

/// Deterministic policy-selection metadata. Priority is an ordering key, not
/// permission to skip prerequisites or evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPolicyRouting {
    pub priority: u16,
    pub readiness_target: ReadinessTarget,
    pub activation: WorkflowPolicyActivation,
    #[serde(default)]
    pub signals: Vec<WorkflowGovernanceSignal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPolicyActivation {
    Required,
    WhenApplicable,
    OnSignal,
}

/// Closed signals that durable trusted state may activate. Free-form agent
/// interpretations never become routing authority.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernanceSignal {
    ContextRecoveryRequired,
    CourseCorrectionRequired,
    ReadinessRequested,
    BuildCompleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPrerequisite {
    pub policy_ref: StableId,
    pub requirement: WorkflowPrerequisiteRequirement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPrerequisiteRequirement {
    Always,
    WhenApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowObligationPolicy {
    pub id: StableId,
    pub description: String,
    pub criticality: ObligationCriticality,
    pub required_before: ReadinessTarget,
    pub claim_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowClaimPolicy {
    pub id: StableId,
    pub statement: String,
    pub evaluator_ref: StableId,
    pub waiver: WorkflowClaimWaiverPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowClaimWaiverPolicy {
    NotWaivable,
    Authorized {
        max_target: ReadinessTarget,
        authority_scope: StableId,
        max_age_seconds: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvaluatorBinding {
    pub id: StableId,
    pub provider: WorkflowEvaluatorProvider,
    pub accepted_evidence_kinds: Vec<WorkflowEvidenceKind>,
    pub minimum_strength: WorkflowEvidenceStrength,
    pub minimum_passing_observations: usize,
    pub minimum_distinct_principals: usize,
    pub max_age_seconds: u64,
    pub freshness: WorkflowFreshnessRequirement,
    pub disproof_policy: WorkflowDisproofPolicy,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvaluatorProvider {
    RepositoryInspector,
    DeterministicTool,
    RepresentativeRuntime,
    IndependentReviewer,
    AuthorizedHuman,
    ExternalAuthority,
    ResearchSource,
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
    pub probe_kind: WorkflowCapabilityProbeKind,
    pub description: String,
    pub affected_claim_refs: Vec<StableId>,
    pub resolution_options: Vec<String>,
    pub blocks_before: ReadinessTarget,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCapabilityProbeKind {
    StaticRegistry,
    LocalCommand,
    RuntimeHandshake,
    CredentialCheck,
    HumanAttestation,
    ExternalVerification,
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

/// Durable ordered governance history. Each record is independently usable as
/// a receipt and is bound to the project, admitted bundle digest, and state
/// version at which it was produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceLedgerDocument {
    pub schema_version: String,
    pub workflow_governance_ledger: WorkflowGovernanceLedger,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceLedger {
    pub records: Vec<WorkflowGovernanceLedgerRecord>,
}

/// Single-record projection for APIs and stores that exchange one governance
/// receipt at a time. It has the identical hash-chain envelope as ledger items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceReceiptDocument {
    pub schema_version: String,
    pub workflow_governance_receipt: WorkflowGovernanceLedgerRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernanceLedgerRecord {
    pub record_id: StableId,
    pub sequence: u64,
    pub project_id: StableId,
    pub bundle_id: StableId,
    pub bundle_digest: String,
    pub state_version: u64,
    #[serde(default)]
    pub previous_record_digest: Option<String>,
    pub record_digest: String,
    pub recorded_at_unix: u64,
    pub event: WorkflowGovernanceEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum WorkflowGovernanceEvent {
    ProjectImported(ProjectImportedEvent),
    PhaseAdvanced(PhaseAdvancedEvent),
    ApplicabilityAssessed(ApplicabilityAssessedEvent),
    SignalChanged(SignalChangedEvent),
    CapabilityProbed(CapabilityProbedEvent),
    DecisionNeedRaised(DecisionNeedRaisedEvent),
    DecisionResolved(DecisionResolvedEvent),
    EvaluatorObserved(EvaluatorObservedEvent),
    WaiverAuthorized(WaiverAuthorizedEvent),
    PolicyCompleted(PolicyCompletedEvent),
    ReceiptRevoked(ReceiptRevokedEvent),
    ContinuityRecorded(ContinuityRecordedEvent),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectImportedEvent {
    pub source_ref: String,
    pub source_digest: String,
    pub snapshot_digest: String,
    pub initial_phase: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PhaseAdvancedEvent {
    #[serde(default)]
    pub from_phase: Option<StableId>,
    pub to_phase: StableId,
    pub snapshot_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplicabilityAssessedEvent {
    pub policy_ref: StableId,
    pub applicable: bool,
    pub assessed_by: PrincipalId,
    pub evaluator_ref: StableId,
    pub credential_id: StableId,
    pub public_key_fingerprint: String,
    pub authorization_registry_digest: String,
    pub basis: Vec<WorkflowContentAddressedReference>,
    pub basis_digest: String,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub observed_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowContentAddressedReference {
    pub subject_ref: String,
    pub subject_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignalChangedEvent {
    pub signal: WorkflowGovernanceSignal,
    pub active: bool,
    pub episode_id: StableId,
    pub generation: u64,
    pub changed_by: PrincipalId,
    pub credential_id: StableId,
    pub public_key_fingerprint: String,
    pub authorization_registry_digest: String,
    pub basis: Vec<WorkflowContentAddressedReference>,
    pub basis_digest: String,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub observed_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityProbedEvent {
    pub policy_ref: StableId,
    pub capability_ref: StableId,
    pub probe_kind: WorkflowCapabilityProbeKind,
    pub credential_id: StableId,
    pub public_key_fingerprint: String,
    pub authorization_registry_digest: String,
    pub available: bool,
    pub probe_ref: String,
    pub probe_digest: String,
    pub subject: WorkflowEvidenceSubject,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub observed_at_unix: u64,
    #[serde(default)]
    pub expires_at_unix: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionNeedRaisedEvent {
    pub policy_ref: StableId,
    pub decision_ref: StableId,
    pub authority_scope: StableId,
    pub question_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionResolvedEvent {
    pub policy_ref: StableId,
    pub decision_ref: StableId,
    pub selected_alternative_ref: StableId,
    pub principal: PrincipalId,
    pub authority_scope: StableId,
    pub credential_id: StableId,
    pub public_key_fingerprint: String,
    pub authorization_registry_digest: String,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub authorization_intent_digest: String,
    pub signature_fingerprint: String,
    pub resolved_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvaluatorObservedEvent {
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub evaluator_ref: StableId,
    pub provider: WorkflowEvaluatorProvider,
    pub credential_id: StableId,
    pub public_key_fingerprint: String,
    pub authorization_registry_digest: String,
    pub kind: WorkflowEvidenceKind,
    pub strength: WorkflowEvidenceStrength,
    pub outcome: WorkflowEvidenceOutcome,
    pub provenance: WorkflowEvidenceProvenance,
    pub subject: WorkflowEvidenceSubject,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub observed_at_unix: u64,
    #[serde(default)]
    pub expires_at_unix: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvidenceProvenance {
    pub source_ref: String,
    pub source_digest: String,
    pub scenario_digest: String,
    pub semantic_identity: StableId,
    pub producer_ref: StableId,
    #[serde(default)]
    pub principal: Option<PrincipalId>,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvidenceSubject {
    pub kind: WorkflowEvidenceSubjectKind,
    pub subject_ref: String,
    pub subject_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvidenceSubjectKind {
    Artifact,
    RepositoryState,
    Runtime,
    ExternalSystem,
    HumanDecision,
    ProjectSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaiverAuthorizedEvent {
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub principal: PrincipalId,
    pub authority_scope: StableId,
    pub credential_id: StableId,
    pub public_key_fingerprint: String,
    pub authorization_registry_digest: String,
    pub max_target: ReadinessTarget,
    pub subject: WorkflowEvidenceSubject,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub authorization_intent_digest: String,
    pub signature_fingerprint: String,
    pub consequences_digest: String,
    pub authorized_at_unix: u64,
    pub expires_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolicyCompletedEvent {
    pub policy_ref: StableId,
    pub target: ReadinessTarget,
    pub phase: StableId,
    pub snapshot_digest: String,
    pub ledger_head_digest: String,
    pub subject: WorkflowEvidenceSubject,
    pub dependency_receipt_digests: Vec<String>,
    pub evidence_receipt_digests: Vec<String>,
    pub unresolved_deferred_obligation_refs: Vec<StableId>,
    pub unresolved_deferred_capability_refs: Vec<StableId>,
    pub completed_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReceiptRevokedEvent {
    pub revoked_record_id: StableId,
    pub revoked_record_digest: String,
    pub principal: PrincipalId,
    pub authority_scope: StableId,
    pub reason: String,
    pub revoked_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContinuityRecordedEvent {
    #[serde(default)]
    pub from_principal: Option<PrincipalId>,
    pub to_principal: PrincipalId,
    pub snapshot_digest: String,
    pub context_digest: String,
    pub next_policy_ref: StableId,
    pub next_action: String,
    pub continuity_at_unix: u64,
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
    /// Trusted Adapter evaluation clock; caller-authored values remain
    /// simulation-only in raw documents.
    pub observed_at_unix: u64,
    pub bundle_id: StableId,
    pub policy_id: StableId,
    pub current_phase: StableId,
    pub target: ReadinessTarget,
    #[serde(default)]
    pub completed_policy_refs: Vec<StableId>,
    /// Policies proven not applicable by trusted applicability receipts. Raw
    /// simulation callers may propose this list, but it remains candidate-only.
    #[serde(default)]
    pub not_applicable_policy_refs: Vec<StableId>,
    #[serde(default)]
    pub available_capability_refs: Vec<StableId>,
    #[serde(default)]
    pub decision_need_refs: Vec<StableId>,
    #[serde(default)]
    pub resolved_decision_refs: Vec<StableId>,
    #[serde(default)]
    pub waivers: Vec<WorkflowClaimWaiverObservation>,
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
    #[serde(default)]
    pub principal: Option<PrincipalId>,
    pub kind: WorkflowEvidenceKind,
    pub strength: WorkflowEvidenceStrength,
    pub freshness: WorkflowEvidenceFreshness,
    pub outcome: WorkflowEvidenceOutcome,
}

/// Raw projection of an authorized waiver receipt. In the simulation lane it
/// remains caller-authored and non-authoritative; the trusted Adapter derives
/// the identical shape only from verified ledger records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowClaimWaiverObservation {
    pub claim_ref: StableId,
    pub principal: PrincipalId,
    pub authority_scope: StableId,
    pub max_target: ReadinessTarget,
    pub authorization_intent_digest: String,
    pub authorized_at_unix: u64,
    pub expires_at_unix: u64,
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
