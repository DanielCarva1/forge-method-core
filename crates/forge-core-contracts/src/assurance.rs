use crate::common::{PrincipalId, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const ASSURANCE_CASE_SCHEMA_VERSION: &str = "0.1";

/// A versioned, agent-facing Assurance Case document.
///
/// The host agent proposes the content. Forge validation decides whether the
/// resulting readiness claim is internally coherent; proposal is not authority.
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
}
