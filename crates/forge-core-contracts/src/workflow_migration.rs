use crate::common::StableId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const WORKFLOW_MIGRATION_PLAN_SCHEMA_VERSION: &str = "0.1";

/// Operator/repository-owned policy for the read-only P5a migration audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationPlanDocument {
    pub schema_version: String,
    pub workflow_migration_plan: WorkflowMigrationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationPlan {
    pub id: StableId,
    pub expected_catalog_count: usize,
    /// Canonical digest of every complete workflow document in stable id order.
    pub expected_catalog_digest: String,
    pub expected_workflow_schema_version: String,
    pub field_mappings: Vec<LegacyWorkflowFieldMapping>,
    pub golden_path_selections: Vec<WorkflowGoldenPathSelection>,
    pub domain_pack_candidate_ids: Vec<StableId>,
    #[serde(default)]
    pub quarantine: Vec<WorkflowMigrationQuarantine>,
    pub compatibility_projection: WorkflowCompatibilityProjectionPolicy,
    pub target_namespaces: WorkflowMigrationTargetNamespaces,
    pub retirement_policy: WorkflowRetirementPolicy,
}

/// Durable reason that a workflow belongs to the representative P5 golden path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGoldenPathSelection {
    pub workflow_id: StableId,
    pub leverage: WorkflowSelectionTier,
    pub risk: WorkflowSelectionTier,
    pub coverage: Vec<WorkflowGoldenPathCoverage>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowSelectionTier {
    Medium,
    High,
    Critical,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGoldenPathCoverage {
    Intent,
    DomainUnknowns,
    Feasibility,
    Requirements,
    Specification,
    Architecture,
    Planning,
    WorkSlicing,
    Implementation,
    Verification,
    RealityFeedback,
    Correction,
    Readiness,
    Release,
    Continuity,
}

/// Every legacy workflow slot must have one explicit future role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LegacyWorkflowFieldMapping {
    pub field: LegacyWorkflowField,
    pub role: LegacyWorkflowFieldRole,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum LegacyWorkflowField {
    Phases,
    Trigger,
    Inputs,
    Steps,
    Outputs,
    DoneWhen,
    BlockedWhen,
    Handoff,
    Module,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LegacyWorkflowFieldRole {
    PhaseProjection,
    RoutingSignal,
    InputObservationCandidate,
    AdvisoryPlaybook,
    ArtifactProjection,
    CompletionClaimCandidate,
    BlockingGapCandidate,
    ContinuityProjection,
    GroupingProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationQuarantine {
    pub workflow_id: StableId,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCompatibilityProjectionPolicy {
    pub mode: WorkflowShadowMode,
    pub exact_fields: Vec<WorkflowCompatibilityField>,
    pub mutation_allowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowShadowMode {
    ReadOnlyExactProjection,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCompatibilityField {
    Id,
    Phases,
    WorkflowRef,
    Triggers,
    Prerequisites,
    Outputs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationTargetNamespaces {
    pub policy: String,
    pub obligation: String,
    pub claim: String,
    pub playbook: String,
    pub evaluator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRetirementPolicy {
    pub required_gates: Vec<WorkflowRetirementGate>,
    pub retirement_allowed_during_foundation: bool,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRetirementGate {
    ExecutableCoverage,
    ShadowParity,
    DeletionTest,
    HumanReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMigrationDisposition {
    GoldenPath,
    CompatibilityPlaybook,
    DomainPackCandidate,
    Quarantined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMigrationAuthority {
    LegacyCompatibilityOnly,
    ShadowEvaluated,
    ExecutableGovernance,
}
