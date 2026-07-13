use crate::common::{PrincipalId, StableId};
use crate::workflow_governance::{
    WorkflowBrokerOriginProfile, WorkflowEvaluatorProvider, WorkflowEvidenceKind,
    WorkflowEvidenceOutcome, WorkflowEvidenceStrength, WorkflowEvidenceSubjectKind,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const ASSURANCE_CASE_SCHEMA_VERSION: &str = "0.1";

pub const MAX_WORKFLOW_INTENT_DESIRED_OUTCOME_BYTES: usize = 16 * 1024;
pub const MAX_WORKFLOW_INTENT_LIST_ITEMS: usize = 64;
pub const MAX_WORKFLOW_INTENT_ITEM_BYTES: usize = 2 * 1024;
pub const MAX_WORKFLOW_INTENT_TOTAL_BYTES: usize = 64 * 1024;
pub const MAX_WORKFLOW_INTENT_SOURCE_REF_BYTES: usize = 1024;
pub const MAX_REPRESENTATIVE_SLICE_TEXT_BYTES: usize = 4 * 1024;
pub const MAX_REPRESENTATIVE_SLICE_ITEMS: usize = 32;
pub const MAX_REPRESENTATIVE_SLICE_ITEM_BYTES: usize = 2 * 1024;
pub const MAX_REPRESENTATIVE_SLICE_TOTAL_BYTES: usize = 32 * 1024;
pub const MAX_DURABLE_ASSURANCE_NEXT_ACTIONS: usize = 8;
pub const WORKFLOW_REPRESENTATIVE_SLICE_SCHEMA_VERSION: &str = "0.1";

/// A versioned, agent-facing Assurance Case **proposal**.
///
/// The host agent proposes every field, including claim status and readiness.
/// This legacy read-only derivation format therefore has no durable workflow
/// authority. Only admitted governance-ledger events and their deterministic
/// projection can establish durable Assurance state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssuranceCaseDocument {
    pub schema_version: String,
    pub assurance_case: AssuranceCase,
}

/// Evidence-backed guidance state for one interpreted human intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssuranceCase {
    pub id: StableId,
    pub intent: IntentProposal,
    pub project_snapshot: ProjectSnapshot,
    pub obligations: Vec<Obligation>,
    pub claims: Vec<AssuranceClaim>,
    pub decision_requests: Vec<DecisionRequest>,
    pub capability_gaps: Vec<CapabilityGap>,
    pub next_actions: Vec<NextAction>,
    pub readiness: ReadinessAssessment,
}

/// The host agent's typed interpretation of the human's desired outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentProposal {
    pub id: StableId,
    pub desired_outcome: String,
    pub constraints: Vec<String>,
    pub preferences: Vec<String>,
    pub unacceptable_outcomes: Vec<String>,
    pub uncertainties: Vec<String>,
}

/// One human-origin revision admitted into the governance ledger.
///
/// The human supplies semantic content and conversation provenance. The
/// trusted mutation boundary, not the host, assigns `revision` and the
/// enclosing assurance epoch and verifies every bound digest. Claim status,
/// readiness, and evaluator selection are deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowHumanIntentRevision {
    pub intent_id: StableId,
    pub revision: u64,
    pub desired_outcome: String,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub preferences: Vec<String>,
    #[serde(default)]
    pub unacceptable_outcomes: Vec<String>,
    #[serde(default)]
    pub uncertainties: Vec<String>,
    pub source_conversation_ref: String,
    pub source_conversation_digest: String,
}

/// Universal assurance questions that apply across product domains.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum UniversalAssuranceLens {
    IntendedOutcome,
    CriticalJourneys,
    SystemIntegrity,
    QualityAttributes,
    Operability,
    LifecycleCoverage,
    RiskAndFailure,
    EvidenceRepresentativeness,
}

impl UniversalAssuranceLens {
    pub const ALL: [Self; 8] = [
        Self::IntendedOutcome,
        Self::CriticalJourneys,
        Self::SystemIntegrity,
        Self::QualityAttributes,
        Self::Operability,
        Self::LifecycleCoverage,
        Self::RiskAndFailure,
        Self::EvidenceRepresentativeness,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::IntendedOutcome => "intended_outcome",
            Self::CriticalJourneys => "critical_journeys",
            Self::SystemIntegrity => "system_integrity",
            Self::QualityAttributes => "quality_attributes",
            Self::Operability => "operability",
            Self::LifecycleCoverage => "lifecycle_coverage",
            Self::RiskAndFailure => "risk_and_failure",
            Self::EvidenceRepresentativeness => "evidence_representativeness",
        }
    }
}

/// Exact durable epoch identity reconstructed from the accepted ledger event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DurableAssuranceEpochBinding {
    pub project_id: StableId,
    pub assurance_epoch: u64,
    pub intent_id: StableId,
    pub intent_revision: u64,
    pub intent_digest: String,
    pub accepted_record_digest: String,
    pub accepted_sequence: u64,
    pub accepted_state_version: u64,
    pub snapshot_digest: String,
    pub ledger_head_before_acceptance: String,
}

/// Exact epistemic state of a durable universal-lens projection.
///
/// Unlike the proposal-only [`AssuranceClaimStatus`], this closed set has no
/// caller-selectable hypothesis state. It is derived only from admitted policy
/// and current governed receipt facts.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DurableAssuranceEpistemicState {
    Unknown,
    Supported,
    Verified,
    Disproven,
    Waived,
}

/// Audit binding from one policy claim into a universal assurance lens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DurableAssuranceClaimBinding {
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub evaluator_ref: StableId,
    pub evaluator_provider: WorkflowEvaluatorProvider,
    pub required_before: ReadinessTarget,
    pub state: DurableAssuranceEpistemicState,
}

/// Accepted evidence receipt retained in the durable projection for audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DurableAssuranceEvidenceBinding {
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub evaluator_ref: StableId,
    pub evidence_ref: String,
    pub evidence_record_digest: String,
    pub origin_record_digest: String,
    pub provider: WorkflowEvaluatorProvider,
    pub kind: WorkflowEvidenceKind,
    pub strength: WorkflowEvidenceStrength,
    pub outcome: WorkflowEvidenceOutcome,
    pub subject_kind: WorkflowEvidenceSubjectKind,
    pub subject_ref: String,
    pub subject_digest: String,
    pub scenario_digest: String,
    pub origin_principal: PrincipalId,
    pub separation_domain: StableId,
    pub broker_profile: WorkflowBrokerOriginProfile,
}

/// Capability receipt or explicit absence bound to a mapped claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DurableAssuranceCapabilityBinding {
    pub policy_ref: StableId,
    pub capability_ref: StableId,
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_digest: Option<String>,
}

/// Governed human-decision state associated with a mapped claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DurableAssuranceDecisionBinding {
    pub policy_ref: StableId,
    pub decision_ref: StableId,
    pub resolved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_digest: Option<String>,
}

/// Current authorized waiver retained without converting it into evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DurableAssuranceWaiverBinding {
    pub policy_ref: StableId,
    pub claim_ref: StableId,
    pub receipt_digest: String,
    pub expires_at_unix: u64,
}

/// One bounded deterministic next step. A packet digest is present only when
/// the current kernel projection already exposes that executable action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DurableAssuranceNextAction {
    pub lens: UniversalAssuranceLens,
    pub kind: NextActionKind,
    pub policy_ref: StableId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_ref: Option<StableId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_packet_digest: Option<String>,
    pub rank: u32,
}

/// Durable state for one universal lens.
///
/// No accepted intent event contains these authority-bearing fields. The pure
/// projector initializes them, and later slices may change them only from
/// separately admitted typed observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DurableAssuranceLensProjection {
    pub lens: UniversalAssuranceLens,
    pub claim_status: DurableAssuranceEpistemicState,
    pub required_before: ReadinessTarget,
    pub due: bool,
    pub claims: Vec<DurableAssuranceClaimBinding>,
    pub evidence: Vec<DurableAssuranceEvidenceBinding>,
    pub capabilities: Vec<DurableAssuranceCapabilityBinding>,
    pub decisions: Vec<DurableAssuranceDecisionBinding>,
    pub waivers: Vec<DurableAssuranceWaiverBinding>,
}

/// Conservative readiness knowledge for a durable Assurance epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DurableAssuranceReadinessState {
    Unknown,
    Blocked,
    Ready,
}

/// Deterministic, non-caller-authored projection of the latest accepted intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DurableAssuranceProjection {
    pub binding: DurableAssuranceEpochBinding,
    pub intent: WorkflowHumanIntentRevision,
    pub lenses: Vec<DurableAssuranceLensProjection>,
    pub readiness: DurableAssuranceReadinessState,
    pub blocker_lenses: Vec<UniversalAssuranceLens>,
    pub next_actions: Vec<DurableAssuranceNextAction>,
    pub projection_digest: String,
}

/// Versioned agent-authored representative-slice manifest. The manifest remains an
/// ordinary project artifact until an admitted evidence receipt binds its
/// exact bytes; deserializing it grants no workflow authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRepresentativeSliceDefinitionDocument {
    pub schema_version: String,
    pub representative_slice: WorkflowRepresentativeSliceDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRepresentativeSliceDefinition {
    pub intent_digest: String,
    pub critical_journey: String,
    pub falsifier: String,
    pub representative_environment: WorkflowRepresentativeEnvironment,
    pub scenarios: Vec<WorkflowRepresentativeScenarioReference>,
    pub material_failure_modes: Vec<WorkflowRepresentativeFailureMode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRepresentativeEnvironment {
    pub runtime_subject_ref: String,
    pub runtime_subject_digest: String,
    pub expectation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRepresentativeScenarioReference {
    pub scenario_ref: String,
    pub declared_scenario_digest: String,
    pub failure_mode_refs: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRepresentativeFailureMode {
    pub id: StableId,
    pub description: String,
}

/// A derived project-state observation used to evaluate the Assurance Case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectSnapshot {
    pub id: StableId,
    pub state_version: u64,
    pub observed_at: String,
    pub evidence_refs: Vec<String>,
    pub phase_projection: Option<String>,
}

/// A result that must become true before a declared readiness target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Obligation {
    pub id: StableId,
    pub description: String,
    pub criticality: ObligationCriticality,
    pub status: ObligationStatus,
    pub required_before: ReadinessTarget,
    pub claim_refs: Vec<StableId>,
}

/// How strongly an obligation binds readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObligationCriticality {
    Advisory,
    Required,
    Critical,
}

/// Current fulfillment state for an obligation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObligationStatus {
    Pending,
    InProgress,
    Satisfied,
    Blocked,
}

/// A proposition that evidence may support, verify, disprove, or waive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssuranceClaim {
    pub id: StableId,
    pub statement: String,
    pub status: AssuranceClaimStatus,
    pub evidence_refs: Vec<String>,
    pub waiver: Option<AssuranceWaiver>,
}

/// Epistemic state of an Assurance Claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssuranceClaimStatus {
    Unknown,
    Hypothesized,
    Supported,
    Verified,
    Disproven,
    Waived,
}

impl AssuranceClaimStatus {
    /// Whether this status can satisfy an obligation that references the claim.
    #[must_use]
    pub const fn satisfies_obligation(self) -> bool {
        matches!(self, Self::Verified | Self::Waived)
    }
}

/// Explicit human or policy authority for accepting an unresolved claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssuranceWaiver {
    pub authorized_by: PrincipalId,
    pub reason: String,
    pub consequences: Vec<String>,
    pub expires_at: Option<String>,
}

/// A question reserved for an irreducible human decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionRequest {
    pub id: StableId,
    pub question: String,
    pub reason: HumanDecisionReason,
    pub alternatives: Vec<DecisionAlternative>,
    pub recommended_alternative_ref: StableId,
    pub blocking: bool,
    pub blocks_before: ReadinessTarget,
}

/// Why project evidence cannot resolve a decision without human judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HumanDecisionReason {
    Preference,
    ValueJudgment,
    MaterialCost,
    IrreversibleRisk,
    ProductDirection,
    ExternalAuthority,
}

/// One concrete human choice plus its consequences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionAlternative {
    pub id: StableId,
    pub description: String,
    pub consequences: Vec<String>,
}

/// A missing agent, tool, environment, knowledge, evaluator, or authority
/// capability that prevents reliable completion or verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGap {
    pub id: StableId,
    pub kind: CapabilityGapKind,
    pub description: String,
    pub affected_claim_refs: Vec<StableId>,
    pub resolution_options: Vec<String>,
    pub blocking: bool,
    pub blocks_before: ReadinessTarget,
}

/// Kind of capability missing from the current execution context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityGapKind {
    Agent,
    Tool,
    Environment,
    Knowledge,
    Evaluator,
    Authority,
    DomainPack,
}

/// One ranked action that reduces a blocker, risk, or evidence gap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NextAction {
    pub id: StableId,
    pub kind: NextActionKind,
    pub description: String,
    pub addresses_claim_refs: Vec<StableId>,
    pub rationale: String,
    pub rank: u32,
}

/// Strategy category for a next action. This does not prescribe agent wording
/// or a fixed playbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NextActionKind {
    Research,
    Experiment,
    Implement,
    Evaluate,
    Challenge,
    AskHuman,
    AcquireCapability,
    DeclareGap,
    Proceed,
}

/// Readiness verdict for one explicit target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadinessAssessment {
    pub target: ReadinessTarget,
    pub verdict: ReadinessVerdict,
    pub blocker_refs: Vec<StableId>,
    pub rationale: String,
}

/// The action horizon currently being assessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessTarget {
    Explore,
    Execute,
    Release,
}

impl ReadinessTarget {
    /// Monotonic target rank used by semantic validation.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Explore => 0,
            Self::Execute => 1,
            Self::Release => 2,
        }
    }
}

/// Whether the selected readiness target is currently permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessVerdict {
    Blocked,
    Ready,
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERIFIED_FIXTURE: &str =
        include_str!("../../../contracts/assurance/representative-slice-verified-assurance.yaml");

    #[test]
    fn representative_fixture_round_trips() {
        let document: AssuranceCaseDocument =
            yaml_serde::from_str(VERIFIED_FIXTURE).expect("deserialize Assurance Case fixture");
        let serialized = yaml_serde::to_string(&document).expect("serialize Assurance Case");
        let reparsed: AssuranceCaseDocument =
            yaml_serde::from_str(&serialized).expect("deserialize serialized Assurance Case");

        assert_eq!(document, reparsed);
        assert_eq!(document.schema_version, ASSURANCE_CASE_SCHEMA_VERSION);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let yaml = VERIFIED_FIXTURE.replacen(
            "  readiness:\n",
            "  unexpected_authority: true\n  readiness:\n",
            1,
        );

        let result = yaml_serde::from_str::<AssuranceCaseDocument>(&yaml);

        assert!(result.is_err());
    }

    #[test]
    fn unknown_closed_enum_value_is_rejected() {
        let yaml = VERIFIED_FIXTURE.replacen("verdict: \"ready\"", "verdict: \"probably\"", 1);

        let result = yaml_serde::from_str::<AssuranceCaseDocument>(&yaml);

        assert!(result.is_err());
    }

    #[test]
    fn accepted_intent_shape_cannot_smuggle_assurance_authority() {
        let proposed = serde_json::json!({
            "intent_id": "intent.example",
            "revision": 1,
            "desired_outcome": "Create a reliable product",
            "source_conversation_ref": "conversation:turn:42",
            "source_conversation_digest": format!("sha256:{}", "a".repeat(64)),
            "claim_status": "verified",
            "readiness": "ready",
            "evaluator_ref": "evaluator.host"
        });

        assert!(serde_json::from_value::<WorkflowHumanIntentRevision>(proposed).is_err());
    }
}
