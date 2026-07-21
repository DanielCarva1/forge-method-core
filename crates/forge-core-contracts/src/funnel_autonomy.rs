use crate::autonomy_policy::AutonomyPolicyContract;
use crate::operation::{AutonomyMode, OperationGateScope, OperationRiskBoundary};
use crate::{Phase, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const FUNNEL_AUTONOMY_SCHEMA_VERSION: &str = "0.1";
pub const FUNNEL_AUTONOMY_POLICY_REF: &str = "contracts/policies/funnel-autonomy.yaml";

/// The one accepted, host-neutral funnel-autonomy policy consumed by Guide,
/// operation planning, and runtime gates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FunnelAutonomyPolicyDocument {
    pub schema_version: String,
    pub artifact_kind: FunnelAutonomyArtifactKind,
    pub status: FunnelAutonomyPolicyStatus,
    pub funnel_autonomy_policy: FunnelAutonomyPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FunnelAutonomyPolicy {
    pub id: StableId,
    /// Reuses the generic autonomy contract for tool-class risk and escalation;
    /// the funnel fields below add phase and protected-boundary semantics.
    pub routing_policy: AutonomyPolicyContract,
    pub phase_profiles: Vec<FunnelPhaseProfile>,
    pub mechanical_loop: FunnelMechanicalLoopPolicy,
    pub protected_boundaries: Vec<FunnelProtectedBoundaryPolicy>,
    pub authority_limits: FunnelAuthorityLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FunnelPhaseProfile {
    pub phase: Phase,
    pub contact_density: FunnelContactDensity,
    pub lane: FunnelLane,
    pub ambiguity_pressure: FunnelAmbiguityPressure,
    pub procedural_confirmation: FunnelProceduralConfirmation,
    pub claim_required_for_mutation: bool,
    pub automatic_gates: Vec<FunnelAutomaticGate>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FunnelMechanicalLoopPolicy {
    pub eligible_phases: Vec<Phase>,
    pub autonomy_modes: Vec<AutonomyMode>,
    pub require_lane_claim: bool,
    pub require_gate_pass: bool,
    pub require_authority_evidence: bool,
    pub require_effect_contracts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FunnelProtectedBoundaryPolicy {
    pub boundary: OperationRiskBoundary,
    pub required_gate_scope: OperationGateScope,
    pub lane: FunnelLane,
    pub contact_density: FunnelContactDensity,
    pub human_checkpoint_required: bool,
}

/// Explicitly records what the accepted policy cannot authorize.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FunnelAuthorityLimits {
    pub grants_mutation_authority: bool,
    pub grants_phase_authority: bool,
    pub grants_release_authority: bool,
    pub grants_signing_or_private_key_authority: bool,
    pub selected_host: Option<StableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FunnelAutonomyArtifactKind {
    FunnelAutonomyPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FunnelAutonomyPolicyStatus {
    Accepted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FunnelContactDensity {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FunnelLane {
    Fast,
    Rigorous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FunnelAmbiguityPressure {
    HumanGuidanceAndResearch,
    HumanCheckpoint,
    GateReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FunnelProceduralConfirmation {
    Expected,
    Conditional,
    Forbidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FunnelAutomaticGate {
    ClaimCoverage,
    Phase,
    ProtectedBoundary,
}
