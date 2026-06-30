use forge_core_contracts::command::{
    CommandExecutor, CommandSideEffectPolicy, CwdPolicy, EnvInheritPolicy, EnvPolicy,
    NetworkPolicy, Platform,
};
use forge_core_contracts::operation::{
    AutonomyMode, CommandRef, ExecutionMode, ForgeOperation, HostAction, HumanInputRequirement,
    HumanPrompt, MutationPolicy, NextActor, OperationGateStatus, OperationSideEffectPolicy,
    RequiredGate,
};
use forge_core_contracts::tool_effect::{AccessMode, ToolEffectContractDocument};
use forge_core_contracts::{
    CommandContractDocument, OperationContractDocument, RepoPath, StableId,
};
use forge_core_store::{
    append_effect_target_metadata_records, append_json_line,
    apply_file_effect_transaction_with_wal_lock, EffectApplicationPayload, EffectApplicationResult,
    EffectApplicationStatus,
};
use forge_core_validate::{
    validate_command, validate_operation, validate_operation_cross_references,
    validate_tool_effect, DiagnosticSeverity, ReferenceIndex,
};
use serde::Serialize;
use std::env;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tracing::instrument;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeReadSnapshot<'a> {
    pub reference_index: &'a ReferenceIndex,
}

impl<'a> RuntimeReadSnapshot<'a> {
    pub fn new(reference_index: &'a ReferenceIndex) -> Self {
        Self { reference_index }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePlan {
    pub status: RuntimePlanStatus,
    pub contract_id: StableId,
    pub autonomy_mode: AutonomyMode,
    pub next_actor: NextActor,
    pub host_action: HostAction,
    pub next_operation: Option<ForgeOperation>,
    pub phase: StableId,
    pub workflow: StableId,
    pub action: StableId,
    pub mutation_policy: MutationPolicy,
    pub side_effect_policy: OperationSideEffectPolicy,
    pub execution_mode: ExecutionMode,
    pub gate_status: OperationGateStatus,
    pub human_input_requirement: HumanInputRequirement,
    pub prompt: Option<HumanPrompt>,
    pub command_refs: Vec<CommandRef>,
    pub effect_contract_refs: Vec<RepoPath>,
    pub reasons: Vec<RuntimePlanReason>,
    pub validation_error_count: usize,
    pub validation_warning_count: usize,
    pub reference_error_count: usize,
    pub reference_warning_count: usize,
    pub used_read_snapshot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePreviewReport {
    pub status: RuntimePreviewStatus,
    pub operation_id: StableId,
    pub preview_mutates_state: bool,
    pub operation_mutates_state: bool,
    pub touched_refs: Vec<RepoPath>,
    pub command_refs: Vec<CommandRef>,
    pub effect_contract_refs: Vec<RepoPath>,
    pub required_gate_refs: Vec<RepoPath>,
    pub gate_contract_refs: Vec<RepoPath>,
    pub authority_sources: Vec<StableId>,
    pub missing_authority: Vec<StableId>,
    pub blockers: Vec<RuntimeReadyBlocker>,
    pub destructive: bool,
    pub risk_level: RuntimeRiskLevel,
    pub rollback_available: bool,
    pub next_human_action: Option<String>,
    pub plan: RuntimePlan,
    pub staging: RuntimeEffectStagingPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePreviewStatus {
    Blocked,
    AwaitingHuman,
    GateRequired,
    ReviewRequired,
    ReadOnly,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeReadyReport {
    pub status: RuntimeReadyStatus,
    pub ready: bool,
    pub operation_id: StableId,
    pub plan_status: RuntimePlanStatus,
    pub staging_status: RuntimeEffectStagingStatus,
    pub gate_status: OperationGateStatus,
    pub reasons: Vec<RuntimePlanReason>,
    pub staging_reasons: Vec<RuntimeEffectStagingReason>,
    pub blocking_reasons: Vec<RuntimeReadyBlocker>,
    pub required_gate_refs: Vec<RepoPath>,
    pub evidence: Vec<RuntimeReadyEvidence>,
    pub validation_error_count: usize,
    pub validation_warning_count: usize,
    pub reference_error_count: usize,
    pub reference_warning_count: usize,
    pub used_read_snapshot: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeReadyStatus {
    Ready,
    NotReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeReadyBlocker {
    ValidationErrors,
    ReferenceErrors,
    OperationDiagnosticsErrors,
    GateBlocked,
    GateMissing,
    GatePending,
    HumanInputRequired,
    GateMissingOrPending,
    RequiredGateStatusUnknown,
    MutationRequiresReview,
    HumanCheckpointRequired,
    HostRequestedConfirmation,
    ShowStatusOnly,
    MutationForbidden,
    ObserveOnly,
    RuntimePlanBlocked,
    RuntimePlanNotReady,
    MissingEffectContractsForMutatingPlan,
    MissingHostCallEvidence,
    NonReadyPlanReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeReadyEvidence {
    pub kind: RuntimeReadyEvidenceKind,
    pub subject: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeReadyEvidenceKind {
    PlanStatus,
    PlanReason,
    GateStatus,
    RequiredGate,
    GateContract,
    ValidationDiagnostics,
    ReferenceDiagnostics,
    StagingStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRiskLevel {
    Low,
    Medium,
    High,
    Blocked,
}

pub type PreviewReport = RuntimePreviewReport;
pub type ReadyReport = RuntimeReadyReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePlanStatus {
    Blocked,
    AwaitingHuman,
    GateRequired,
    ReviewRequired,
    ReadOnlyStatus,
    ReadyToCallOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePlanReason {
    ValidationErrors,
    ReferenceErrors,
    OperationDiagnosticsErrors,
    GateBlocked,
    HumanInputRequired,
    GateMissingOrPending,
    MutationRequiresReview,
    HumanCheckpointRequired,
    HostRequestedConfirmation,
    ShowStatusOnly,
    MutationForbidden,
    ObserveOnly,
    HostCallAllowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEffectStagingPlan {
    pub status: RuntimeEffectStagingStatus,
    pub contract_id: StableId,
    pub side_effect_policy: OperationSideEffectPolicy,
    pub command_refs: Vec<CommandRef>,
    pub effect_contract_refs: Vec<RepoPath>,
    pub commit_allowed: bool,
    pub reasons: Vec<RuntimeEffectStagingReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEffectStagingStatus {
    Blocked,
    NotStageable,
    NoEffects,
    Staged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEffectStagingReason {
    RuntimePlanBlocked,
    RuntimePlanNotReady,
    MissingEffectContractsForMutatingPlan,
    NoCommandsOrEffects,
    StagedCommands,
    StagedEffects,
    CommitRequiresLaterBoundary,
}

#[derive(Debug, Clone, Copy)]
pub struct CommandExecutionContext<'a> {
    pub repo_root: &'a Path,
    pub project_root: &'a Path,
    pub package_root: &'a Path,
}

impl<'a> CommandExecutionContext<'a> {
    pub fn single_root(root: &'a Path) -> Self {
        Self {
            repo_root: root,
            project_root: root,
            package_root: root,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCommandExecution {
    pub status: RuntimeCommandExecutionStatus,
    pub command_id: StableId,
    pub executor: CommandExecutor,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub reasons: Vec<RuntimeCommandExecutionReason>,
    pub validation_error_count: usize,
    pub validation_warning_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCommandEvidenceRecord {
    pub schema_version: String,
    pub record_kind: RuntimeEvidenceKind,
    pub recorded_at: String,
    pub operation_id: StableId,
    pub command_id: StableId,
    pub executor: CommandExecutor,
    pub status: RuntimeCommandExecutionStatus,
    pub reasons: Vec<RuntimeCommandExecutionReason>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub validation_error_count: usize,
    pub validation_warning_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEvidenceKind {
    CommandExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCommandExecutionStatus {
    Succeeded,
    Failed,
    TimedOut,
    Blocked,
    NotRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCommandExecutionReason {
    StagingPlanNotStaged,
    CommandNotStaged,
    CommandValidationErrors,
    NonReadOnlyCommand,
    UnsafeCommandSafetyFlags,
    NetworkNotDisabled,
    ShellExecutorBlocked,
    UnsupportedPlatform,
    TimeoutMustBePositive,
    RequiredEnvMissing,
    ForbiddenEnvPresent,
    SpawnFailed,
    CommandSucceeded,
    CommandFailed,
    CommandTimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEffectPayload {
    pub target_ref: String,
    pub payload_kind: RuntimeEffectPayloadKind,
    pub content_hash: Option<String>,
    pub byte_len: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEffectPayloadKind {
    RuntimeGenerated,
    HumanApproved,
    CommandEvidence,
    ExternalArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEffectTransactionPlan {
    pub status: RuntimeEffectTransactionStatus,
    pub operation_id: StableId,
    pub effect_id: StableId,
    pub effect_ref: RepoPath,
    pub write_count: usize,
    pub payload_count: usize,
    pub reasons: Vec<RuntimeEffectTransactionReason>,
    pub validation_error_count: usize,
    pub validation_warning_count: usize,
}

#[derive(Debug, Clone)]
pub struct RuntimeOperationCommandInput {
    pub document: CommandContractDocument,
}

#[derive(Debug, Clone)]
pub struct RuntimeOperationEffectInput {
    pub effect_ref: RepoPath,
    pub document: ToolEffectContractDocument,
}

#[derive(Debug, Clone)]
pub struct RuntimeOperationEffectPayload {
    pub target_ref: String,
    pub payload_kind: RuntimeEffectPayloadKind,
    pub content_hash: String,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeOperationExecutionContext<'a> {
    pub command_context: CommandExecutionContext<'a>,
    pub effect_store_root: &'a Path,
    pub evidence_log_relative_path: &'a str,
    pub wal_relative_path: &'a str,
    pub lock_relative_path: &'a str,
    pub effect_metadata_index_relative_path: &'a str,
    pub recorded_at: &'a str,
    pub tx_id_prefix: &'a str,
}

impl<'a> RuntimeOperationExecutionContext<'a> {
    pub fn single_root(root: &'a Path) -> Self {
        Self {
            command_context: CommandExecutionContext::single_root(root),
            effect_store_root: root,
            evidence_log_relative_path: ".forge-method/evidence/command-execution.ndjson",
            wal_relative_path: ".forge-method/wal/effects.ndjson",
            lock_relative_path: ".forge-method/locks/effects.lock",
            effect_metadata_index_relative_path: ".forge-method/index/effect-targets.ndjson",
            recorded_at: "unknown",
            tx_id_prefix: "runtime-operation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeOperationExecution {
    pub status: RuntimeOperationExecutionStatus,
    pub operation_id: StableId,
    pub plan: RuntimePlan,
    pub staging: Option<RuntimeEffectStagingPlan>,
    pub command_executions: Vec<RuntimeCommandExecution>,
    pub command_evidence_appended: usize,
    pub effect_transactions: Vec<RuntimeEffectTransactionPlan>,
    pub effect_applications: Vec<EffectApplicationResult>,
    pub reasons: Vec<RuntimeOperationExecutionReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOperationExecutionStatus {
    AwaitingHuman,
    Blocked,
    Failed,
    /// File effects were committed, but appending the effect metadata index failed.
    /// Repair by rebuilding the effect index from committed WAL records.
    AppliedButMetadataMissing,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOperationExecutionReason {
    PlanAwaitingHuman,
    PlanNotReady,
    StagingBlocked,
    NoEffectsOrCommands,
    MissingRequiredCommandContract,
    MissingOptionalCommandContract,
    RequiredCommandUnsuccessful,
    CommandEvidenceAppendFailed,
    MissingEffectContract,
    EffectTransactionBlocked,
    EffectApplicationFailed,
    EffectMetadataAppendFailed,
    /// Suggested repair: run `forge-core rebuild-effect-index` with this execution's WAL, index, and lock paths.
    RebuildEffectIndexSuggested,
    OperationCompleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEffectTransactionStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEffectTransactionReason {
    StagingPlanNotStaged,
    EffectNotStaged,
    EffectValidationErrors,
    NoWrites,
    MissingPayloadForWrite,
    TransactionReady,
}

#[instrument(skip_all, fields(operation_id = tracing::field::Empty, effect_count = effects.len(), command_count = commands.len()), level = "info")]
pub fn execute_operation(
    document: &OperationContractDocument,
    snapshot: RuntimeReadSnapshot<'_>,
    commands: &[RuntimeOperationCommandInput],
    effects: &[RuntimeOperationEffectInput],
    payloads: &[RuntimeOperationEffectPayload],
    context: &RuntimeOperationExecutionContext<'_>,
) -> RuntimeOperationExecution {
    let plan = plan_operation_with_snapshot(document, snapshot);
    let operation_id = plan.contract_id.clone();
    tracing::Span::current().record("operation_id", operation_id.0.as_str());
    let mut reasons = Vec::new();

    if plan.status != RuntimePlanStatus::ReadyToCallOperation {
        let status = if plan.status == RuntimePlanStatus::AwaitingHuman {
            reasons.push(RuntimeOperationExecutionReason::PlanAwaitingHuman);
            RuntimeOperationExecutionStatus::AwaitingHuman
        } else {
            reasons.push(RuntimeOperationExecutionReason::PlanNotReady);
            RuntimeOperationExecutionStatus::Blocked
        };
        return RuntimeOperationExecution {
            status,
            operation_id,
            plan,
            staging: None,
            command_executions: Vec::new(),
            command_evidence_appended: 0,
            effect_transactions: Vec::new(),
            effect_applications: Vec::new(),
            reasons,
        };
    }

    let staging = stage_operation_effects(&plan);
    if staging.status == RuntimeEffectStagingStatus::Blocked
        || staging.status == RuntimeEffectStagingStatus::NotStageable
    {
        reasons.push(RuntimeOperationExecutionReason::StagingBlocked);
        return RuntimeOperationExecution {
            status: RuntimeOperationExecutionStatus::Blocked,
            operation_id,
            plan,
            staging: Some(staging),
            command_executions: Vec::new(),
            command_evidence_appended: 0,
            effect_transactions: Vec::new(),
            effect_applications: Vec::new(),
            reasons,
        };
    }

    if staging.status == RuntimeEffectStagingStatus::NoEffects {
        reasons.push(RuntimeOperationExecutionReason::NoEffectsOrCommands);
        reasons.push(RuntimeOperationExecutionReason::OperationCompleted);
        return RuntimeOperationExecution {
            status: RuntimeOperationExecutionStatus::Completed,
            operation_id,
            plan,
            staging: Some(staging),
            command_executions: Vec::new(),
            command_evidence_appended: 0,
            effect_transactions: Vec::new(),
            effect_applications: Vec::new(),
            reasons,
        };
    }

    let mut command_executions = Vec::new();
    let mut command_evidence_appended = 0usize;
    for command_ref in &staging.command_refs {
        let Some(command) = commands
            .iter()
            .find(|input| input.document.command_contract.id == command_ref.id)
        else {
            if command_ref.required {
                reasons.push(RuntimeOperationExecutionReason::MissingRequiredCommandContract);
                return RuntimeOperationExecution {
                    status: RuntimeOperationExecutionStatus::Blocked,
                    operation_id,
                    plan,
                    staging: Some(staging),
                    command_executions,
                    command_evidence_appended,
                    effect_transactions: Vec::new(),
                    effect_applications: Vec::new(),
                    reasons,
                };
            }
            reasons.push(RuntimeOperationExecutionReason::MissingOptionalCommandContract);
            continue;
        };

        let execution =
            run_staged_read_only_command(&staging, &command.document, &context.command_context);
        let evidence = command_execution_evidence_record(&staging, &execution, context.recorded_at);
        if append_json_line(
            context.effect_store_root,
            context.evidence_log_relative_path,
            &evidence,
        )
        .is_err()
        {
            reasons.push(RuntimeOperationExecutionReason::CommandEvidenceAppendFailed);
            command_executions.push(execution);
            return RuntimeOperationExecution {
                status: RuntimeOperationExecutionStatus::Failed,
                operation_id,
                plan,
                staging: Some(staging),
                command_executions,
                command_evidence_appended,
                effect_transactions: Vec::new(),
                effect_applications: Vec::new(),
                reasons,
            };
        }
        command_evidence_appended += 1;
        let command_succeeded = execution.status == RuntimeCommandExecutionStatus::Succeeded;
        command_executions.push(execution);
        if command_ref.required && !command_succeeded {
            reasons.push(RuntimeOperationExecutionReason::RequiredCommandUnsuccessful);
            return RuntimeOperationExecution {
                status: RuntimeOperationExecutionStatus::Failed,
                operation_id,
                plan,
                staging: Some(staging),
                command_executions,
                command_evidence_appended,
                effect_transactions: Vec::new(),
                effect_applications: Vec::new(),
                reasons,
            };
        }
    }

    let runtime_payloads = runtime_effect_payloads(payloads);
    let store_payloads = store_effect_payloads(payloads);
    let mut effect_transactions = Vec::new();
    let mut effect_applications = Vec::new();
    for effect_ref in &staging.effect_contract_refs {
        let Some(effect) = effects.iter().find(|input| &input.effect_ref == effect_ref) else {
            reasons.push(RuntimeOperationExecutionReason::MissingEffectContract);
            return RuntimeOperationExecution {
                status: RuntimeOperationExecutionStatus::Blocked,
                operation_id,
                plan,
                staging: Some(staging),
                command_executions,
                command_evidence_appended,
                effect_transactions,
                effect_applications,
                reasons,
            };
        };

        let transaction =
            prepare_effect_transaction(&staging, effect_ref, &effect.document, &runtime_payloads);
        let transaction_ready = transaction.status == RuntimeEffectTransactionStatus::Ready;
        effect_transactions.push(transaction);
        if !transaction_ready {
            reasons.push(RuntimeOperationExecutionReason::EffectTransactionBlocked);
            return RuntimeOperationExecution {
                status: RuntimeOperationExecutionStatus::Blocked,
                operation_id,
                plan,
                staging: Some(staging),
                command_executions,
                command_evidence_appended,
                effect_transactions,
                effect_applications,
                reasons,
            };
        }

        let mut application = apply_file_effect_transaction_with_wal_lock(
            context.effect_store_root,
            &effect.document,
            &store_payloads,
            context.wal_relative_path,
            context.lock_relative_path,
            effect_tx_id(
                context.tx_id_prefix,
                &effect.document.tool_effect_contract.id,
            ),
        );
        let applied = application.status == EffectApplicationStatus::Applied;
        if !applied {
            effect_applications.push(application);
            reasons.push(RuntimeOperationExecutionReason::EffectApplicationFailed);
            return RuntimeOperationExecution {
                status: RuntimeOperationExecutionStatus::Failed,
                operation_id,
                plan,
                staging: Some(staging),
                command_executions,
                command_evidence_appended,
                effect_transactions,
                effect_applications,
                reasons,
            };
        }
        for record in &mut application.metadata_records {
            record.recorded_at = Some(context.recorded_at.to_string());
        }
        if append_effect_target_metadata_records(
            context.effect_store_root,
            context.effect_metadata_index_relative_path,
            &application.metadata_records,
        )
        .is_err()
        {
            effect_applications.push(application);
            reasons.push(RuntimeOperationExecutionReason::EffectMetadataAppendFailed);
            reasons.push(RuntimeOperationExecutionReason::RebuildEffectIndexSuggested);
            return RuntimeOperationExecution {
                status: RuntimeOperationExecutionStatus::AppliedButMetadataMissing,
                operation_id,
                plan,
                staging: Some(staging),
                command_executions,
                command_evidence_appended,
                effect_transactions,
                effect_applications,
                reasons,
            };
        }
        effect_applications.push(application);
    }

    reasons.push(RuntimeOperationExecutionReason::OperationCompleted);
    RuntimeOperationExecution {
        status: RuntimeOperationExecutionStatus::Completed,
        operation_id,
        plan,
        staging: Some(staging),
        command_executions,
        command_evidence_appended,
        effect_transactions,
        effect_applications,
        reasons,
    }
}

pub fn plan_operation(document: &OperationContractDocument) -> RuntimePlan {
    plan_operation_inner(document, None)
}

pub fn plan_operation_with_snapshot(
    document: &OperationContractDocument,
    snapshot: RuntimeReadSnapshot<'_>,
) -> RuntimePlan {
    plan_operation_inner(document, Some(snapshot))
}

pub fn preview_operation(document: &OperationContractDocument) -> RuntimePreviewReport {
    preview_operation_inner(document, None)
}

pub fn preview_operation_with_snapshot(
    document: &OperationContractDocument,
    snapshot: RuntimeReadSnapshot<'_>,
) -> RuntimePreviewReport {
    preview_operation_inner(document, Some(snapshot))
}

#[must_use]
pub fn preview_operation_from_plan(
    document: &OperationContractDocument,
    plan: &RuntimePlan,
) -> RuntimePreviewReport {
    let staging = stage_operation_effects(plan);
    let operation = &document.operation_contract;
    let operation_mutates_state = mutating_side_effect(plan.side_effect_policy);
    let required_gate_refs = required_gate_refs(&operation.gates.required_before_mutation);
    let blockers =
        runtime_ready_blockers(plan, &staging, &operation.gates.required_before_mutation);

    RuntimePreviewReport {
        status: preview_status(plan.status),
        operation_id: plan.contract_id.clone(),
        preview_mutates_state: false,
        operation_mutates_state,
        touched_refs: operation.coordination_scope.target.paths.clone(),
        command_refs: plan.command_refs.clone(),
        effect_contract_refs: plan.effect_contract_refs.clone(),
        required_gate_refs,
        gate_contract_refs: operation.gates.gate_contract_refs.clone(),
        authority_sources: operation.authority.authority_sources.clone(),
        missing_authority: operation.authority.missing_authority.clone(),
        blockers: blockers.clone(),
        destructive: operation_mutates_state,
        risk_level: runtime_risk_level(plan.status, plan.side_effect_policy, &blockers),
        rollback_available: false,
        next_human_action: next_human_action(plan),
        plan: plan.clone(),
        staging,
    }
}

#[must_use]
pub fn preview_runtime_plan(plan: &RuntimePlan) -> RuntimePreviewReport {
    let staging = stage_operation_effects(plan);
    let operation_mutates_state = mutating_side_effect(plan.side_effect_policy);
    let blockers = runtime_ready_blockers(plan, &staging, &[]);

    RuntimePreviewReport {
        status: preview_status(plan.status),
        operation_id: plan.contract_id.clone(),
        preview_mutates_state: false,
        operation_mutates_state,
        touched_refs: Vec::new(),
        command_refs: plan.command_refs.clone(),
        effect_contract_refs: plan.effect_contract_refs.clone(),
        required_gate_refs: Vec::new(),
        gate_contract_refs: Vec::new(),
        authority_sources: Vec::new(),
        missing_authority: Vec::new(),
        blockers: blockers.clone(),
        destructive: operation_mutates_state,
        risk_level: runtime_risk_level(plan.status, plan.side_effect_policy, &blockers),
        rollback_available: false,
        next_human_action: next_human_action(plan),
        plan: plan.clone(),
        staging,
    }
}

#[instrument(skip_all, fields(operation_id = %document.operation_contract.contract_id.0), level = "info")]
fn preview_operation_inner(
    document: &OperationContractDocument,
    snapshot: Option<RuntimeReadSnapshot<'_>>,
) -> RuntimePreviewReport {
    let plan = plan_operation_inner(document, snapshot);
    preview_operation_from_plan(document, &plan)
}

pub fn ready_operation(document: &OperationContractDocument) -> RuntimeReadyReport {
    ready_operation_inner(document, None)
}

pub fn ready_operation_with_snapshot(
    document: &OperationContractDocument,
    snapshot: RuntimeReadSnapshot<'_>,
) -> RuntimeReadyReport {
    ready_operation_inner(document, Some(snapshot))
}

#[must_use]
pub fn ready_operation_from_plan(
    document: &OperationContractDocument,
    plan: &RuntimePlan,
) -> RuntimeReadyReport {
    let staging = stage_operation_effects(plan);
    let operation = &document.operation_contract;
    runtime_ready_report(
        plan,
        &staging,
        &operation.gates.required_before_mutation,
        &operation.gates.gate_contract_refs,
    )
}

#[must_use]
pub fn ready_runtime_plan(plan: &RuntimePlan) -> RuntimeReadyReport {
    let staging = stage_operation_effects(plan);
    runtime_ready_report(plan, &staging, &[], &[])
}

#[instrument(skip_all, fields(operation_id = %document.operation_contract.contract_id.0), level = "info")]
fn ready_operation_inner(
    document: &OperationContractDocument,
    snapshot: Option<RuntimeReadSnapshot<'_>>,
) -> RuntimeReadyReport {
    let plan = plan_operation_inner(document, snapshot);
    ready_operation_from_plan(document, &plan)
}

fn runtime_ready_report(
    plan: &RuntimePlan,
    staging: &RuntimeEffectStagingPlan,
    required_gates: &[RequiredGate],
    gate_contract_refs: &[RepoPath],
) -> RuntimeReadyReport {
    let required_gate_refs = required_gate_refs(required_gates);
    let blocking_reasons = runtime_ready_blockers(plan, staging, required_gates);
    let evidence = runtime_ready_evidence(plan, staging, required_gates, gate_contract_refs);
    let ready = blocking_reasons.is_empty()
        && plan.status == RuntimePlanStatus::ReadyToCallOperation
        && matches!(
            staging.status,
            RuntimeEffectStagingStatus::Staged | RuntimeEffectStagingStatus::NoEffects
        );

    RuntimeReadyReport {
        status: if ready {
            RuntimeReadyStatus::Ready
        } else {
            RuntimeReadyStatus::NotReady
        },
        ready,
        operation_id: plan.contract_id.clone(),
        plan_status: plan.status,
        staging_status: staging.status,
        gate_status: plan.gate_status,
        reasons: plan.reasons.clone(),
        staging_reasons: staging.reasons.clone(),
        blocking_reasons,
        required_gate_refs,
        evidence,
        validation_error_count: plan.validation_error_count,
        validation_warning_count: plan.validation_warning_count,
        reference_error_count: plan.reference_error_count,
        reference_warning_count: plan.reference_warning_count,
        used_read_snapshot: plan.used_read_snapshot,
    }
}

#[instrument(skip_all, fields(operation_id = %document.operation_contract.contract_id.0), level = "info")]
fn plan_operation_inner(
    document: &OperationContractDocument,
    snapshot: Option<RuntimeReadSnapshot<'_>>,
) -> RuntimePlan {
    let validation = validate_operation(document);
    let validation_error_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .count();
    let validation_warning_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
        .count();

    let reference_validation = snapshot
        .map(|snapshot| validate_operation_cross_references(document, snapshot.reference_index));
    let reference_error_count = reference_validation
        .as_ref()
        .map(|report| {
            report
                .diagnostics()
                .iter()
                .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
                .count()
        })
        .unwrap_or(0);
    let reference_warning_count = reference_validation
        .as_ref()
        .map(|report| {
            report
                .diagnostics()
                .iter()
                .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
                .count()
        })
        .unwrap_or(0);

    let operation = &document.operation_contract;
    let mut reasons = Vec::new();

    let status = if validation_error_count > 0 {
        reasons.push(RuntimePlanReason::ValidationErrors);
        RuntimePlanStatus::Blocked
    } else if reference_error_count > 0 {
        reasons.push(RuntimePlanReason::ReferenceErrors);
        RuntimePlanStatus::Blocked
    } else if !operation.diagnostics.errors.is_empty() {
        reasons.push(RuntimePlanReason::OperationDiagnosticsErrors);
        RuntimePlanStatus::Blocked
    } else if operation.gates.current_gate_status == OperationGateStatus::Blocked {
        reasons.push(RuntimePlanReason::GateBlocked);
        RuntimePlanStatus::Blocked
    } else if operation.human.input_requirement == HumanInputRequirement::Required {
        reasons.push(RuntimePlanReason::HumanInputRequired);
        RuntimePlanStatus::AwaitingHuman
    } else if gate_is_missing_or_pending(operation.gates.current_gate_status)
        && !operation.gates.required_before_mutation.is_empty()
    {
        reasons.push(RuntimePlanReason::GateMissingOrPending);
        RuntimePlanStatus::GateRequired
    } else if operation.authority.mutation_policy == MutationPolicy::RequiresReview {
        reasons.push(RuntimePlanReason::MutationRequiresReview);
        RuntimePlanStatus::ReviewRequired
    } else if operation.human.input_requirement == HumanInputRequirement::Checkpoint {
        reasons.push(RuntimePlanReason::HumanCheckpointRequired);
        RuntimePlanStatus::ReviewRequired
    } else if operation.recommendation.host_action == HostAction::RequestConfirmation {
        reasons.push(RuntimePlanReason::HostRequestedConfirmation);
        RuntimePlanStatus::AwaitingHuman
    } else if operation.recommendation.host_action == HostAction::ShowStatus {
        reasons.push(RuntimePlanReason::ShowStatusOnly);
        RuntimePlanStatus::ReadOnlyStatus
    } else if operation.authority.mutation_policy == MutationPolicy::Forbidden {
        reasons.push(RuntimePlanReason::MutationForbidden);
        RuntimePlanStatus::ReadOnlyStatus
    } else if operation.execution_policy.mode == ExecutionMode::ObserveOnly {
        reasons.push(RuntimePlanReason::ObserveOnly);
        RuntimePlanStatus::ReadOnlyStatus
    } else {
        reasons.push(RuntimePlanReason::HostCallAllowed);
        RuntimePlanStatus::ReadyToCallOperation
    };

    RuntimePlan {
        status,
        contract_id: operation.contract_id.clone(),
        autonomy_mode: operation.autonomy.mode,
        next_actor: operation.recommendation.next_actor,
        host_action: operation.recommendation.host_action,
        next_operation: operation.recommendation.next_operation,
        phase: operation.recommendation.phase.clone(),
        workflow: operation.recommendation.workflow.clone(),
        action: operation.recommendation.action.clone(),
        mutation_policy: operation.authority.mutation_policy,
        side_effect_policy: operation.authority.side_effect_policy,
        execution_mode: operation.execution_policy.mode,
        gate_status: operation.gates.current_gate_status,
        human_input_requirement: operation.human.input_requirement,
        prompt: meaningful_prompt(&operation.human.prompt),
        command_refs: operation.command_refs.clone(),
        effect_contract_refs: operation.effect_contract_refs.clone(),
        reasons,
        validation_error_count,
        validation_warning_count,
        reference_error_count,
        reference_warning_count,
        used_read_snapshot: snapshot.is_some(),
    }
}

fn gate_is_missing_or_pending(status: OperationGateStatus) -> bool {
    matches!(
        status,
        OperationGateStatus::Missing | OperationGateStatus::Pending
    )
}

fn meaningful_prompt(prompt: &HumanPrompt) -> Option<HumanPrompt> {
    if prompt.text.trim().is_empty() && prompt.options.is_empty() {
        None
    } else {
        Some(prompt.clone())
    }
}

pub fn stage_operation_effects(plan: &RuntimePlan) -> RuntimeEffectStagingPlan {
    let mut reasons = Vec::new();

    let status = if plan.status == RuntimePlanStatus::Blocked {
        reasons.push(RuntimeEffectStagingReason::RuntimePlanBlocked);
        RuntimeEffectStagingStatus::Blocked
    } else if plan.status != RuntimePlanStatus::ReadyToCallOperation {
        reasons.push(RuntimeEffectStagingReason::RuntimePlanNotReady);
        RuntimeEffectStagingStatus::NotStageable
    } else if mutating_side_effect(plan.side_effect_policy) && plan.effect_contract_refs.is_empty()
    {
        reasons.push(RuntimeEffectStagingReason::MissingEffectContractsForMutatingPlan);
        RuntimeEffectStagingStatus::Blocked
    } else if plan.command_refs.is_empty() && plan.effect_contract_refs.is_empty() {
        reasons.push(RuntimeEffectStagingReason::NoCommandsOrEffects);
        RuntimeEffectStagingStatus::NoEffects
    } else {
        if !plan.command_refs.is_empty() {
            reasons.push(RuntimeEffectStagingReason::StagedCommands);
        }
        if !plan.effect_contract_refs.is_empty() {
            reasons.push(RuntimeEffectStagingReason::StagedEffects);
        }
        reasons.push(RuntimeEffectStagingReason::CommitRequiresLaterBoundary);
        RuntimeEffectStagingStatus::Staged
    };

    RuntimeEffectStagingPlan {
        status,
        contract_id: plan.contract_id.clone(),
        side_effect_policy: plan.side_effect_policy,
        command_refs: plan.command_refs.clone(),
        effect_contract_refs: plan.effect_contract_refs.clone(),
        commit_allowed: false,
        reasons,
    }
}

fn mutating_side_effect(policy: OperationSideEffectPolicy) -> bool {
    matches!(
        policy,
        OperationSideEffectPolicy::WriteProjectFiles
            | OperationSideEffectPolicy::RunCommands
            | OperationSideEffectPolicy::Publish
    )
}

fn preview_status(status: RuntimePlanStatus) -> RuntimePreviewStatus {
    match status {
        RuntimePlanStatus::Blocked => RuntimePreviewStatus::Blocked,
        RuntimePlanStatus::AwaitingHuman => RuntimePreviewStatus::AwaitingHuman,
        RuntimePlanStatus::GateRequired => RuntimePreviewStatus::GateRequired,
        RuntimePlanStatus::ReviewRequired => RuntimePreviewStatus::ReviewRequired,
        RuntimePlanStatus::ReadOnlyStatus => RuntimePreviewStatus::ReadOnly,
        RuntimePlanStatus::ReadyToCallOperation => RuntimePreviewStatus::Ready,
    }
}

fn runtime_risk_level(
    status: RuntimePlanStatus,
    side_effect_policy: OperationSideEffectPolicy,
    blockers: &[RuntimeReadyBlocker],
) -> RuntimeRiskLevel {
    if status == RuntimePlanStatus::Blocked || !blockers.is_empty() {
        return RuntimeRiskLevel::Blocked;
    }
    match side_effect_policy {
        OperationSideEffectPolicy::ReadOnly => RuntimeRiskLevel::Low,
        OperationSideEffectPolicy::WriteProjectFiles | OperationSideEffectPolicy::RunCommands => {
            RuntimeRiskLevel::Medium
        }
        OperationSideEffectPolicy::Publish => RuntimeRiskLevel::High,
    }
}

fn next_human_action(plan: &RuntimePlan) -> Option<String> {
    let action = match plan.status {
        RuntimePlanStatus::Blocked => "inspect blockers before retrying",
        RuntimePlanStatus::AwaitingHuman => {
            if let Some(prompt) = &plan.prompt {
                if !prompt.text.trim().is_empty() {
                    return Some(prompt.text.clone());
                }
            }
            "provide required human input"
        }
        RuntimePlanStatus::GateRequired => "provide required gate evidence",
        RuntimePlanStatus::ReviewRequired => "review and approve the operation boundary",
        RuntimePlanStatus::ReadOnlyStatus => "show read-only status; no mutation is authorized",
        RuntimePlanStatus::ReadyToCallOperation => return None,
    };
    Some(action.to_string())
}

fn runtime_ready_blockers(
    plan: &RuntimePlan,
    staging: &RuntimeEffectStagingPlan,
    required_gates: &[RequiredGate],
) -> Vec<RuntimeReadyBlocker> {
    let mut blockers = ready_plan_blockers(plan);
    for blocker in ready_gate_blockers(plan.gate_status, required_gates) {
        push_ready_blocker(&mut blockers, blocker);
    }
    for blocker in ready_staging_blockers(staging) {
        push_ready_blocker(&mut blockers, blocker);
    }
    blockers
}

fn required_gate_refs(required_gates: &[RequiredGate]) -> Vec<RepoPath> {
    required_gates
        .iter()
        .map(|gate| gate.gate_contract_ref.clone())
        .collect()
}

fn ready_plan_blockers(plan: &RuntimePlan) -> Vec<RuntimeReadyBlocker> {
    let mut blockers = Vec::new();
    for reason in &plan.reasons {
        match reason {
            RuntimePlanReason::ValidationErrors => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::ValidationErrors);
            }
            RuntimePlanReason::ReferenceErrors => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::ReferenceErrors);
            }
            RuntimePlanReason::OperationDiagnosticsErrors => {
                push_ready_blocker(
                    &mut blockers,
                    RuntimeReadyBlocker::OperationDiagnosticsErrors,
                );
            }
            RuntimePlanReason::GateBlocked => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::GateBlocked);
            }
            RuntimePlanReason::HumanInputRequired => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::HumanInputRequired);
            }
            RuntimePlanReason::GateMissingOrPending => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::GateMissingOrPending);
            }
            RuntimePlanReason::MutationRequiresReview => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::MutationRequiresReview);
            }
            RuntimePlanReason::HumanCheckpointRequired => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::HumanCheckpointRequired);
            }
            RuntimePlanReason::HostRequestedConfirmation => {
                push_ready_blocker(
                    &mut blockers,
                    RuntimeReadyBlocker::HostRequestedConfirmation,
                );
            }
            RuntimePlanReason::ShowStatusOnly => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::ShowStatusOnly);
            }
            RuntimePlanReason::MutationForbidden => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::MutationForbidden);
            }
            RuntimePlanReason::ObserveOnly => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::ObserveOnly);
            }
            RuntimePlanReason::HostCallAllowed => {}
        }
    }
    if plan.status == RuntimePlanStatus::ReadyToCallOperation {
        if !plan
            .reasons
            .iter()
            .any(|reason| reason == &RuntimePlanReason::HostCallAllowed)
        {
            push_ready_blocker(&mut blockers, RuntimeReadyBlocker::MissingHostCallEvidence);
        }
        if plan
            .reasons
            .iter()
            .any(|reason| reason != &RuntimePlanReason::HostCallAllowed)
        {
            push_ready_blocker(&mut blockers, RuntimeReadyBlocker::NonReadyPlanReason);
        }
    }
    blockers
}

fn ready_gate_blockers(
    gate_status: OperationGateStatus,
    _required_gates: &[RequiredGate],
) -> Vec<RuntimeReadyBlocker> {
    let mut blockers = Vec::new();
    match gate_status {
        OperationGateStatus::Pass => {}
        OperationGateStatus::Missing => {
            push_ready_blocker(&mut blockers, RuntimeReadyBlocker::GateMissing);
            push_ready_blocker(&mut blockers, RuntimeReadyBlocker::GateMissingOrPending);
        }
        OperationGateStatus::Pending => {
            push_ready_blocker(&mut blockers, RuntimeReadyBlocker::GatePending);
            push_ready_blocker(&mut blockers, RuntimeReadyBlocker::GateMissingOrPending);
        }
        OperationGateStatus::Blocked => {
            push_ready_blocker(&mut blockers, RuntimeReadyBlocker::GateBlocked);
        }
        OperationGateStatus::NotApplicable => {
            push_ready_blocker(
                &mut blockers,
                RuntimeReadyBlocker::RequiredGateStatusUnknown,
            );
        }
    }
    blockers
}

fn ready_staging_blockers(staging: &RuntimeEffectStagingPlan) -> Vec<RuntimeReadyBlocker> {
    let mut blockers = Vec::new();
    for reason in &staging.reasons {
        match reason {
            RuntimeEffectStagingReason::RuntimePlanBlocked => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::RuntimePlanBlocked);
            }
            RuntimeEffectStagingReason::RuntimePlanNotReady => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::RuntimePlanNotReady);
            }
            RuntimeEffectStagingReason::MissingEffectContractsForMutatingPlan => {
                push_ready_blocker(
                    &mut blockers,
                    RuntimeReadyBlocker::MissingEffectContractsForMutatingPlan,
                );
            }
            RuntimeEffectStagingReason::NoCommandsOrEffects
            | RuntimeEffectStagingReason::StagedCommands
            | RuntimeEffectStagingReason::StagedEffects
            | RuntimeEffectStagingReason::CommitRequiresLaterBoundary => {}
        }
    }
    blockers
}

fn push_ready_blocker(blockers: &mut Vec<RuntimeReadyBlocker>, blocker: RuntimeReadyBlocker) {
    if !blockers.contains(&blocker) {
        blockers.push(blocker);
    }
}

fn runtime_ready_evidence(
    plan: &RuntimePlan,
    staging: &RuntimeEffectStagingPlan,
    required_gates: &[RequiredGate],
    gate_contract_refs: &[RepoPath],
) -> Vec<RuntimeReadyEvidence> {
    let mut evidence = vec![
        RuntimeReadyEvidence {
            kind: RuntimeReadyEvidenceKind::PlanStatus,
            subject: plan.contract_id.0.clone(),
            detail: format!("{:?}", plan.status),
        },
        RuntimeReadyEvidence {
            kind: RuntimeReadyEvidenceKind::GateStatus,
            subject: plan.contract_id.0.clone(),
            detail: format!("{:?}", plan.gate_status),
        },
        RuntimeReadyEvidence {
            kind: RuntimeReadyEvidenceKind::StagingStatus,
            subject: plan.contract_id.0.clone(),
            detail: format!("{:?}", staging.status),
        },
    ];

    if plan.reasons.is_empty() {
        evidence.push(RuntimeReadyEvidence {
            kind: RuntimeReadyEvidenceKind::PlanReason,
            subject: plan.contract_id.0.clone(),
            detail: "none".to_string(),
        });
    } else {
        for reason in &plan.reasons {
            evidence.push(RuntimeReadyEvidence {
                kind: RuntimeReadyEvidenceKind::PlanReason,
                subject: plan.contract_id.0.clone(),
                detail: format!("{reason:?}"),
            });
        }
    }

    if plan.validation_error_count > 0 || plan.validation_warning_count > 0 {
        evidence.push(RuntimeReadyEvidence {
            kind: RuntimeReadyEvidenceKind::ValidationDiagnostics,
            subject: plan.contract_id.0.clone(),
            detail: format!(
                "errors={}, warnings={}",
                plan.validation_error_count, plan.validation_warning_count
            ),
        });
    }
    if plan.reference_error_count > 0 || plan.reference_warning_count > 0 {
        evidence.push(RuntimeReadyEvidence {
            kind: RuntimeReadyEvidenceKind::ReferenceDiagnostics,
            subject: plan.contract_id.0.clone(),
            detail: format!(
                "errors={}, warnings={}",
                plan.reference_error_count, plan.reference_warning_count
            ),
        });
    }
    for gate in required_gates {
        evidence.push(RuntimeReadyEvidence {
            kind: RuntimeReadyEvidenceKind::RequiredGate,
            subject: gate.gate_contract_ref.0.clone(),
            detail: gate
                .reason
                .clone()
                .unwrap_or_else(|| "required before mutation".to_string()),
        });
    }
    for gate_ref in gate_contract_refs {
        evidence.push(RuntimeReadyEvidence {
            kind: RuntimeReadyEvidenceKind::GateContract,
            subject: gate_ref.0.clone(),
            detail: "declared gate contract".to_string(),
        });
    }
    evidence
}

pub fn run_staged_read_only_command(
    staging: &RuntimeEffectStagingPlan,
    command: &CommandContractDocument,
    context: &CommandExecutionContext<'_>,
) -> RuntimeCommandExecution {
    let command_contract = &command.command_contract;
    let mut reasons = Vec::new();
    let validation = validate_command(command);
    let validation_error_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .count();
    let validation_warning_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
        .count();

    if staging.status != RuntimeEffectStagingStatus::Staged {
        reasons.push(RuntimeCommandExecutionReason::StagingPlanNotStaged);
        return command_result(
            RuntimeCommandExecutionStatus::NotRun,
            command_contract,
            reasons,
            validation_error_count,
            validation_warning_count,
        );
    }

    if !staging_command_matches(staging, &command_contract.id) {
        reasons.push(RuntimeCommandExecutionReason::CommandNotStaged);
        return command_result(
            RuntimeCommandExecutionStatus::NotRun,
            command_contract,
            reasons,
            validation_error_count,
            validation_warning_count,
        );
    }

    if validation_error_count > 0 {
        reasons.push(RuntimeCommandExecutionReason::CommandValidationErrors);
    }
    if command_contract.side_effect_policy != CommandSideEffectPolicy::ReadOnly {
        reasons.push(RuntimeCommandExecutionReason::NonReadOnlyCommand);
    }
    if command_contract.network_policy != NetworkPolicy::Disabled {
        reasons.push(RuntimeCommandExecutionReason::NetworkNotDisabled);
    }
    if command_contract.safety.shell_string_allowed
        || command_contract.safety.writes_files
        || command_contract.safety.publishes
        || command_contract.safety.installs_packages
    {
        reasons.push(RuntimeCommandExecutionReason::UnsafeCommandSafetyFlags);
    }
    if shell_executor(command_contract.executor) {
        reasons.push(RuntimeCommandExecutionReason::ShellExecutorBlocked);
    }
    if !command_contract.platforms.contains(&current_platform()) {
        reasons.push(RuntimeCommandExecutionReason::UnsupportedPlatform);
    }
    if command_contract.timeout_ms == 0 {
        reasons.push(RuntimeCommandExecutionReason::TimeoutMustBePositive);
    }
    if missing_required_env(&command_contract.env_policy) {
        reasons.push(RuntimeCommandExecutionReason::RequiredEnvMissing);
    }
    if forbidden_env_present(&command_contract.env_policy) {
        reasons.push(RuntimeCommandExecutionReason::ForbiddenEnvPresent);
    }

    if !reasons.is_empty() {
        return command_result(
            RuntimeCommandExecutionStatus::Blocked,
            command_contract,
            reasons,
            validation_error_count,
            validation_warning_count,
        );
    }

    let started = Instant::now();
    let mut process = Command::new(executor_program(command_contract.executor));
    process
        .args(&command_contract.args)
        .current_dir(resolve_cwd(command_contract.cwd_policy, context))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_env_policy(&mut process, &command_contract.env_policy);

    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => {
            let mut result = command_result(
                RuntimeCommandExecutionStatus::Failed,
                command_contract,
                vec![RuntimeCommandExecutionReason::SpawnFailed],
                validation_error_count,
                validation_warning_count,
            );
            result.stderr = error.to_string();
            result.duration_ms = duration_millis(started.elapsed());
            return result;
        }
    };

    let output_limit =
        usize::try_from(command_contract.output_policy.max_bytes).unwrap_or(usize::MAX);
    let stdout_handle = child
        .stdout
        .take()
        .map(|stdout| spawn_limited_capture(stdout, output_limit));
    let stderr_handle = child
        .stderr
        .take()
        .map(|stderr| spawn_limited_capture(stderr, output_limit));
    let timeout = Duration::from_millis(command_contract.timeout_ms);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = join_capture(stdout_handle);
                let stderr = join_capture(stderr_handle);
                let reason = if status.success() {
                    RuntimeCommandExecutionReason::CommandSucceeded
                } else {
                    RuntimeCommandExecutionReason::CommandFailed
                };
                return RuntimeCommandExecution {
                    status: if status.success() {
                        RuntimeCommandExecutionStatus::Succeeded
                    } else {
                        RuntimeCommandExecutionStatus::Failed
                    },
                    command_id: command_contract.id.clone(),
                    executor: command_contract.executor,
                    exit_code: status.code(),
                    timed_out: false,
                    duration_ms: duration_millis(started.elapsed()),
                    stdout: stdout.text,
                    stderr: stderr.text,
                    stdout_truncated: stdout.truncated,
                    stderr_truncated: stderr.truncated,
                    reasons: vec![reason],
                    validation_error_count,
                    validation_warning_count,
                };
            }
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let stdout = join_capture(stdout_handle);
                let stderr = join_capture(stderr_handle);
                return RuntimeCommandExecution {
                    status: RuntimeCommandExecutionStatus::TimedOut,
                    command_id: command_contract.id.clone(),
                    executor: command_contract.executor,
                    exit_code: None,
                    timed_out: true,
                    duration_ms: duration_millis(started.elapsed()),
                    stdout: stdout.text,
                    stderr: stderr.text,
                    stdout_truncated: stdout.truncated,
                    stderr_truncated: stderr.truncated,
                    reasons: vec![RuntimeCommandExecutionReason::CommandTimedOut],
                    validation_error_count,
                    validation_warning_count,
                };
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                let mut result = command_result(
                    RuntimeCommandExecutionStatus::Failed,
                    command_contract,
                    vec![RuntimeCommandExecutionReason::CommandFailed],
                    validation_error_count,
                    validation_warning_count,
                );
                result.stderr = error.to_string();
                result.duration_ms = duration_millis(started.elapsed());
                return result;
            }
        }
    }
}

pub fn command_execution_evidence_record(
    staging: &RuntimeEffectStagingPlan,
    execution: &RuntimeCommandExecution,
    recorded_at: impl Into<String>,
) -> RuntimeCommandEvidenceRecord {
    RuntimeCommandEvidenceRecord {
        schema_version: "0.1".to_string(),
        record_kind: RuntimeEvidenceKind::CommandExecution,
        recorded_at: recorded_at.into(),
        operation_id: staging.contract_id.clone(),
        command_id: execution.command_id.clone(),
        executor: execution.executor,
        status: execution.status,
        reasons: execution.reasons.clone(),
        exit_code: execution.exit_code,
        timed_out: execution.timed_out,
        duration_ms: execution.duration_ms,
        stdout: execution.stdout.clone(),
        stderr: execution.stderr.clone(),
        stdout_truncated: execution.stdout_truncated,
        stderr_truncated: execution.stderr_truncated,
        validation_error_count: execution.validation_error_count,
        validation_warning_count: execution.validation_warning_count,
    }
}

pub fn prepare_effect_transaction(
    staging: &RuntimeEffectStagingPlan,
    effect_ref: &RepoPath,
    effect: &ToolEffectContractDocument,
    payloads: &[RuntimeEffectPayload],
) -> RuntimeEffectTransactionPlan {
    let effect_contract = &effect.tool_effect_contract;
    let validation = validate_tool_effect(effect);
    let validation_error_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .count();
    let validation_warning_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
        .count();
    let mut reasons = Vec::new();

    if staging.status != RuntimeEffectStagingStatus::Staged {
        reasons.push(RuntimeEffectTransactionReason::StagingPlanNotStaged);
    }
    if !staging.effect_contract_refs.contains(effect_ref) {
        reasons.push(RuntimeEffectTransactionReason::EffectNotStaged);
    }
    if validation_error_count > 0 {
        reasons.push(RuntimeEffectTransactionReason::EffectValidationErrors);
    }
    if effect_contract.write_set.is_empty() {
        reasons.push(RuntimeEffectTransactionReason::NoWrites);
    }
    if effect_contract.write_set.iter().any(|write| {
        write.access_mode != AccessMode::Delete
            && !payloads
                .iter()
                .any(|payload| payload.target_ref == write.reference)
    }) {
        reasons.push(RuntimeEffectTransactionReason::MissingPayloadForWrite);
    }

    let status = if reasons.is_empty() {
        reasons.push(RuntimeEffectTransactionReason::TransactionReady);
        RuntimeEffectTransactionStatus::Ready
    } else {
        RuntimeEffectTransactionStatus::Blocked
    };

    RuntimeEffectTransactionPlan {
        status,
        operation_id: staging.contract_id.clone(),
        effect_id: effect_contract.id.clone(),
        effect_ref: effect_ref.clone(),
        write_count: effect_contract.write_set.len(),
        payload_count: payloads.len(),
        reasons,
        validation_error_count,
        validation_warning_count,
    }
}

fn command_result(
    status: RuntimeCommandExecutionStatus,
    command: &forge_core_contracts::CommandContract,
    reasons: Vec<RuntimeCommandExecutionReason>,
    validation_error_count: usize,
    validation_warning_count: usize,
) -> RuntimeCommandExecution {
    RuntimeCommandExecution {
        status,
        command_id: command.id.clone(),
        executor: command.executor,
        exit_code: None,
        timed_out: false,
        duration_ms: 0,
        stdout: String::new(),
        stderr: String::new(),
        stdout_truncated: false,
        stderr_truncated: false,
        reasons,
        validation_error_count,
        validation_warning_count,
    }
}

fn staging_command_matches(staging: &RuntimeEffectStagingPlan, command_id: &StableId) -> bool {
    staging
        .command_refs
        .iter()
        .any(|command_ref| &command_ref.id == command_id)
}

fn shell_executor(executor: CommandExecutor) -> bool {
    matches!(executor, CommandExecutor::Powershell | CommandExecutor::Sh)
}

fn executor_program(executor: CommandExecutor) -> &'static str {
    match executor {
        CommandExecutor::Cargo => "cargo",
        CommandExecutor::Node => "node",
        CommandExecutor::Bun => "bun",
        CommandExecutor::Powershell => "powershell",
        CommandExecutor::Sh => "sh",
        CommandExecutor::Git => "git",
    }
}

fn current_platform() -> Platform {
    if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::Macos
    } else {
        Platform::Linux
    }
}

fn resolve_cwd<'a>(policy: CwdPolicy, context: &'a CommandExecutionContext<'_>) -> &'a Path {
    match policy {
        CwdPolicy::ProjectRoot => context.project_root,
        CwdPolicy::RepoRoot => context.repo_root,
        CwdPolicy::PackageRoot => context.package_root,
    }
}

fn apply_env_policy(process: &mut Command, policy: &EnvPolicy) {
    match policy.inherit {
        EnvInheritPolicy::None => {
            process.env_clear();
        }
        EnvInheritPolicy::Minimal => {
            process.env_clear();
            for key in minimal_env_allowlist() {
                if let Some(value) = env::var_os(key) {
                    process.env(key, value);
                }
            }
        }
        EnvInheritPolicy::Project => {}
    }
}

fn minimal_env_allowlist() -> &'static [&'static str] {
    &[
        "PATH",
        "Path",
        "PATHEXT",
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "HOME",
        "USERPROFILE",
    ]
}

fn missing_required_env(policy: &EnvPolicy) -> bool {
    policy.required.iter().any(|key| !env_key_exists(key))
}

fn forbidden_env_present(policy: &EnvPolicy) -> bool {
    policy.forbidden.iter().any(|key| env_key_exists(key))
}

fn env_key_exists(expected: &str) -> bool {
    env::vars_os().any(|(key, _)| {
        let actual = key.to_string_lossy();
        if cfg!(windows) {
            actual.eq_ignore_ascii_case(expected)
        } else {
            actual == expected
        }
    })
}

#[derive(Debug)]
struct CapturedOutput {
    text: String,
    truncated: bool,
}

fn spawn_limited_capture<R>(mut reader: R, max_bytes: usize) -> thread::JoinHandle<CapturedOutput>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut captured = Vec::new();
        let mut truncated = false;
        let mut buffer = [0_u8; 8192];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes_read) => {
                    if captured.len() < max_bytes {
                        let remaining = max_bytes - captured.len();
                        let keep = remaining.min(bytes_read);
                        captured.extend_from_slice(&buffer[..keep]);
                        if keep < bytes_read {
                            truncated = true;
                        }
                    } else if bytes_read > 0 {
                        truncated = true;
                    }
                }
                Err(_) => break,
            }
        }

        CapturedOutput {
            text: String::from_utf8_lossy(&captured).to_string(),
            truncated,
        }
    })
}

fn join_capture(handle: Option<thread::JoinHandle<CapturedOutput>>) -> CapturedOutput {
    handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_else(|| CapturedOutput {
            text: String::new(),
            truncated: false,
        })
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn runtime_effect_payloads(
    payloads: &[RuntimeOperationEffectPayload],
) -> Vec<RuntimeEffectPayload> {
    payloads
        .iter()
        .map(|payload| RuntimeEffectPayload {
            target_ref: payload.target_ref.clone(),
            payload_kind: payload.payload_kind,
            content_hash: Some(payload.content_hash.clone()),
            byte_len: u64::try_from(payload.content.len()).unwrap_or(u64::MAX),
        })
        .collect()
}

fn store_effect_payloads(
    payloads: &[RuntimeOperationEffectPayload],
) -> Vec<EffectApplicationPayload> {
    payloads
        .iter()
        .map(|payload| EffectApplicationPayload {
            target_ref: payload.target_ref.clone(),
            content: payload.content.clone(),
            content_hash: payload.content_hash.clone(),
        })
        .collect()
}

fn effect_tx_id(prefix: &str, effect_id: &StableId) -> String {
    let sanitized: String = effect_id
        .0
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    format!("{prefix}-{sanitized}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::tool_effect::EffectTargetKind;
    use forge_core_store::{build_reference_index, sha256_content_hash};
    use std::fs;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn operation_fixture(name: &str) -> OperationContractDocument {
        let path = repo_root()
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0")
            .join(name);
        let input = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        yaml_serde::from_str(&input)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
    }

    fn effect_fixture(name: &str) -> ToolEffectContractDocument {
        let path = repo_root().join("contracts").join("effects").join(name);
        let input = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        yaml_serde::from_str(&input)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
    }

    fn fresh_temp_root(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "forge-core-runtime-lib-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp root");
        path
    }

    fn runtime_payload(target_ref: &str, content: &[u8]) -> RuntimeOperationEffectPayload {
        RuntimeOperationEffectPayload {
            target_ref: target_ref.to_string(),
            payload_kind: RuntimeEffectPayloadKind::RuntimeGenerated,
            content_hash: sha256_content_hash(content),
            content: content.to_vec(),
        }
    }

    #[test]
    fn execute_operation_reports_applied_but_metadata_missing_when_index_append_fails() {
        let mut document = operation_fixture("mechanical-story-execute.yaml");
        document.operation_contract.command_refs.clear();
        let index = build_reference_index(repo_root()).expect("reference index");
        let mut effect = effect_fixture("story-artifact-write-effect.yaml");
        effect.tool_effect_contract.read_set.truncate(1);
        effect.tool_effect_contract.read_set[0].target_kind = EffectTargetKind::FilePath;
        effect.tool_effect_contract.read_set[0].reference =
            ".forge-method/stories/current.yaml".to_string();
        effect.tool_effect_contract.read_set[0].expected_hash = None;
        effect.tool_effect_contract.read_set[0].expected_version = None;
        let effect_input = RuntimeOperationEffectInput {
            effect_ref: RepoPath("contracts/effects/story-artifact-write-effect.yaml".to_string()),
            document: effect,
        };
        let artifact_payload = runtime_payload(
            ".forge-method/artifacts/story-current-result.yaml",
            b"story: completed\n",
        );
        let evidence_payload = runtime_payload(
            ".forge-method/evidence/story-validation.json",
            br#"{"status":"passed"}"#,
        );
        let temp_root = fresh_temp_root("metadata-append-failure");
        let index_path = temp_root.join(".forge-method/index/effect-targets.ndjson");
        fs::create_dir_all(&index_path).expect("create directory where metadata file should be");
        let context = RuntimeOperationExecutionContext {
            command_context: CommandExecutionContext::single_root(&temp_root),
            effect_store_root: &temp_root,
            evidence_log_relative_path: ".forge-method/evidence/command-execution.ndjson",
            wal_relative_path: ".forge-method/wal/effects.ndjson",
            lock_relative_path: ".forge-method/locks/effects.lock",
            effect_metadata_index_relative_path: ".forge-method/index/effect-targets.ndjson",
            recorded_at: "2026-06-25T00:00:00Z",
            tx_id_prefix: "test-execute-operation",
        };

        let execution = execute_operation(
            &document,
            RuntimeReadSnapshot::new(&index),
            &[],
            &[effect_input],
            &[artifact_payload, evidence_payload],
            &context,
        );

        assert_eq!(
            execution.status,
            RuntimeOperationExecutionStatus::AppliedButMetadataMissing,
            "{execution:#?}"
        );
        assert_eq!(
            execution.reasons,
            vec![
                RuntimeOperationExecutionReason::EffectMetadataAppendFailed,
                RuntimeOperationExecutionReason::RebuildEffectIndexSuggested,
            ]
        );
        assert_eq!(execution.effect_applications.len(), 1);
        assert_eq!(
            execution.effect_applications[0].status,
            EffectApplicationStatus::Applied
        );
        assert!(temp_root
            .join(".forge-method/artifacts/story-current-result.yaml")
            .exists());
        assert!(temp_root.join(".forge-method/wal/effects.ndjson").exists());
        assert!(index_path.is_dir());
    }
}
