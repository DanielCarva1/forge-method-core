use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationContractDocument {
    pub operation_contract: OperationContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationContract {
    pub schema_version: String,
    pub contract_id: StableId,
    pub created_at: String,
    pub project_ref: ProjectRef,
    pub source: OperationSource,
    pub autonomy: AutonomyPolicy,
    pub recommendation: Recommendation,
    pub authority: AuthorityPolicy,
    pub coordination_scope: CoordinationScope,
    pub execution_policy: ExecutionPolicy,
    pub stop_policy: StopPolicy,
    #[serde(default)]
    pub request: Option<SideContractRef>,
    #[serde(default)]
    pub decision_close: Option<SideContractRef>,
    #[serde(default)]
    pub runtime_handoff: Option<SideContractRef>,
    pub allowed_actions: Vec<StableId>,
    pub forbidden_actions: Vec<StableId>,
    pub human: HumanPolicy,
    pub loads: LoadSet,
    pub gates: GatePolicy,
    pub stop_conditions: Vec<StableId>,
    #[serde(default)]
    pub command_refs: Vec<CommandRef>,
    #[serde(default)]
    pub effect_contract_refs: Vec<RepoPath>,
    pub diagnostics: Diagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectRef {
    pub root: RepoPath,
    pub project_id: StableId,
    pub state_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationSource {
    pub host: StableId,
    pub surface: OperationSurface,
    pub operation: ForgeOperation,
    pub human_input_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutonomyPolicy {
    pub mode: AutonomyMode,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Recommendation {
    pub next_actor: NextActor,
    pub next_operation: Option<ForgeOperation>,
    pub host_action: HostAction,
    pub phase: StableId,
    pub workflow: StableId,
    pub action: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorityPolicy {
    pub mutation_policy: MutationPolicy,
    pub side_effect_policy: OperationSideEffectPolicy,
    pub authority_sources: Vec<StableId>,
    pub authority_evidence: Vec<AuthorityEvidence>,
    pub missing_authority: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorityEvidence {
    pub kind: StableId,
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinationScope {
    pub target: CoordinationTarget,
    pub concurrency: ConcurrencyScope,
    pub write_authority: WriteAuthority,
    pub completion: CompletionScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinationTarget {
    pub kind: TargetKind,
    pub id: Option<StableId>,
    pub product_area: Option<StableId>,
    pub paths: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConcurrencyScope {
    pub expected_state_version: u64,
    pub agent_id: Option<StableId>,
    pub caller_role: CallerRole,
    pub fleet_mode: bool,
    pub registry_ref: Option<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteAuthority {
    pub requires_driver_claim: bool,
    pub requires_lane_claim: bool,
    pub claim_contract_ref: Option<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletionScope {
    pub must_check_completion: bool,
    pub completion_contract_ref: Option<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPolicy {
    pub mode: ExecutionMode,
    pub max_steps: u64,
    pub retry_policy: RetryPolicy,
    pub branch_policy: BranchPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetryPolicy {
    pub max_attempts: u64,
    pub on_failure: RetryFailureAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BranchPolicy {
    pub allowed_branches: Vec<StableId>,
    pub default_branch: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StopPolicy {
    pub stop_when: Vec<StableId>,
    pub on_stop: StopHandoff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StopHandoff {
    pub next_actor: NextActor,
    pub next_operation: Option<ForgeOperation>,
    pub host_action: HostAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SideContractRef {
    #[serde(rename = "ref")]
    pub reference: RepoPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HumanPolicy {
    pub input_requirement: HumanInputRequirement,
    pub prompt: HumanPrompt,
    pub tone_contract: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HumanPrompt {
    pub mode: PromptMode,
    pub text: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoadSet {
    pub required: Vec<RepoPath>,
    pub optional: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatePolicy {
    pub required_before_mutation: Vec<RequiredGate>,
    pub current_gate_status: OperationGateStatus,
    pub gate_contract_refs: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RequiredGate {
    pub scope: OperationGateScope,
    pub gate_contract_ref: RepoPath,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommandRef {
    pub id: StableId,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Diagnostics {
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationSurface {
    CliJson,
    Mcp,
    Skill,
    App,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForgeOperation {
    Guide,
    Gate,
    ClaimLane,
    RecordArtifact,
    RecordRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyMode {
    Observe,
    Facilitate,
    Research,
    Plan,
    Execute,
    Repair,
    GateReview,
    Diagnose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NextActor {
    Human,
    HostAgent,
    ForgeCore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostAction {
    ShowStatus,
    CallOperation,
    RequestConfirmation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MutationPolicy {
    Allowed,
    Forbidden,
    RequiresReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationSideEffectPolicy {
    ReadOnly,
    WriteProjectFiles,
    RunCommands,
    Publish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    None,
    Project,
    Story,
    Lane,
    Artifact,
    IntegrationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallerRole {
    Driver,
    Worker,
    Runtime,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    ObserveOnly,
    SingleStep,
    AutonomousSequence,
    BoundedLoop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RetryFailureAction {
    Stop,
    RunGate,
    RecordRequest,
    AskHuman,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HumanInputRequirement {
    None,
    Checkpoint,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromptMode {
    None,
    Status,
    Decision,
    Question,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationGateStatus {
    Pass,
    Pending,
    Blocked,
    Missing,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationGateScope {
    Lane,
    ProductArea,
    Integration,
    Release,
}

#[cfg(test)]
mod tests {
    use super::OperationGateStatus;

    #[test]
    fn operation_gate_status_wire_format_remains_snake_case() {
        let pending = serde_yaml::to_string(&OperationGateStatus::Pending).unwrap();
        assert_eq!(pending.trim(), "pending");
        let blocked = serde_yaml::to_string(&OperationGateStatus::Blocked).unwrap();
        assert_eq!(blocked.trim(), "blocked");
        let round_trip: OperationGateStatus = serde_yaml::from_str("pending").unwrap();
        assert_eq!(round_trip, OperationGateStatus::Pending);
    }
}
