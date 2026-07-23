//! Read-only operation analysis: planning, preview, and readiness.
//!
//! These functions decide what an operation WILL do before any mutation. They
//! produce [`RuntimePlan`], [`RuntimePreviewReport`], and [`RuntimeReadyReport`]
//! and never touch the WAL or the effect store.

use super::*;
use forge_core_contracts::funnel_autonomy::FunnelPhaseProfile;
use forge_core_contracts::operation::OperationRiskBoundary;
use forge_core_decisions::{
    evaluate_funnel_operation, load_accepted_funnel_autonomy_policy, FunnelOperationDisposition,
    FunnelOperationReason,
};

#[derive(Debug, Clone, Copy)]
pub struct RuntimeReadSnapshot<'a> {
    pub reference_index: &'a ReferenceIndex,
}

impl<'a> RuntimeReadSnapshot<'a> {
    #[must_use]
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
    pub funnel_disposition: FunnelOperationDisposition,
    pub funnel_phase_profile: Option<FunnelPhaseProfile>,
    pub funnel_reasons: Vec<FunnelOperationReason>,
    pub protected_boundaries: Vec<OperationRiskBoundary>,
    pub command_refs: Vec<CommandRef>,
    pub effect_contract_refs: Vec<RepoPath>,
    pub reasons: Vec<RuntimePlanReason>,
    pub validation_error_count: usize,
    pub validation_warning_count: usize,
    pub reference_error_count: usize,
    pub reference_warning_count: usize,
    pub used_read_snapshot: bool,
}

// The preview report carries independent risk/gate signals that the host
// inspects one by one; collapsing them into bitflags would hurt schema
// clarity for downstream agents.
#[allow(clippy::struct_excessive_bools)]
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
    FunnelPolicyUnavailable,
    FunnelOperationBlocked,
    FunnelGateRequired,
    FunnelReviewRequired,
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
    FunnelPolicyUnavailable,
    FunnelOperationBlocked,
    FunnelGateRequired,
    FunnelReviewRequired,
    GateMissingOrPending,
    MutationRequiresReview,
    HumanCheckpointRequired,
    HostRequestedConfirmation,
    ShowStatusOnly,
    MutationForbidden,
    ObserveOnly,
    HostCallAllowed,
}

#[must_use]
pub fn plan_operation(document: &OperationContractDocument) -> RuntimePlan {
    plan_operation_inner(document, None)
}

#[must_use]
pub fn plan_operation_with_snapshot(
    document: &OperationContractDocument,
    snapshot: RuntimeReadSnapshot<'_>,
) -> RuntimePlan {
    plan_operation_inner(document, Some(snapshot))
}

#[must_use]
pub fn preview_operation(document: &OperationContractDocument) -> RuntimePreviewReport {
    preview_operation_inner(document, None)
}

#[must_use]
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
        status: preview_status(plan.status, &blockers),
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
        next_human_action: next_human_action(plan, &blockers),
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
        status: preview_status(plan.status, &blockers),
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
        next_human_action: next_human_action(plan, &blockers),
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

/// Compute whether every staged effect has a real rollback path, based on the
/// `EffectRepair.inverse.kind` declared in each `ToolEffect` contract.
///
/// Returns `true` only when every provided effect has `inverse.kind` other than
/// [`InverseKind::None`]. An empty slice returns `true` (vacuously: read-only
/// operations have nothing to roll back). When `stop_if_inverse_missing` is
/// `true` for any effect and that effect's inverse kind is `None`, returns
/// `false` and the host should treat the operation as non-rollbackable.
///
/// This is the building block that powers `rollback_available` in
/// [`preview_operation_with_effect_documents`]. Callers without effect
/// documents (e.g. plan-only previews) keep the legacy `false` placeholder.
#[must_use]
pub fn compute_rollback_available(effects: &[ToolEffectContractDocument]) -> bool {
    effects
        .iter()
        .all(|document| document.tool_effect_contract.repair.inverse.kind != InverseKind::None)
}

/// Like [`preview_operation_with_snapshot`], but also computes the real
/// `rollback_available` value and unions `touched_refs` with the write-sets
/// declared in the provided `ToolEffect` contract documents. The documents
/// should correspond to `plan.effect_contract_refs`; callers that cannot
/// supply them should fall back to [`preview_operation_with_snapshot`] and
/// accept the conservative `rollback_available = false` placeholder and the
/// shallow `touched_refs` derived only from `CoordinationScope.target.paths`.
#[must_use]
pub fn preview_operation_with_effect_documents(
    document: &OperationContractDocument,
    snapshot: RuntimeReadSnapshot<'_>,
    effects: &[ToolEffectContractDocument],
) -> RuntimePreviewReport {
    let mut report = preview_operation_with_snapshot(document, snapshot);
    report.rollback_available = compute_rollback_available(effects);
    let mut touched = report.touched_refs.clone();
    for added in collect_effect_touched_refs(effects) {
        if !touched.iter().any(|existing| existing == &added) {
            touched.push(added);
        }
    }
    report.touched_refs = touched;
    report
}

/// Collect repo paths that the provided `ToolEffect` contracts would write.
/// Includes only file-backed target kinds (matching the effect-store
/// definition); logical targets like `StateKey` and `Glob` are excluded because
/// they do not resolve to a single repo path.
///
/// Order is stable: writes are emitted in document order, then in `write_set`
/// order within each document. Duplicates within a single call are preserved;
/// deduplication is the caller's responsibility ([`preview_operation_with_effect_documents`]
/// dedupes against the existing `touched_refs`).
#[must_use]
pub fn collect_effect_touched_refs(effects: &[ToolEffectContractDocument]) -> Vec<RepoPath> {
    let mut refs = Vec::new();
    for document in effects {
        for write in &document.tool_effect_contract.write_set {
            if !file_backed_effect_target(write.target_kind) {
                continue;
            }
            refs.push(RepoPath(write.reference.clone()));
        }
    }
    refs
}

/// Mirror of [`forge_core_store::file_backed_target`] that the runtime can call
/// without taking a dependency on the store crate for a single predicate. The
/// set must stay in sync with the store's definition.
fn file_backed_effect_target(
    target_kind: forge_core_contracts::tool_effect::EffectTargetKind,
) -> bool {
    use forge_core_contracts::tool_effect::EffectTargetKind;
    matches!(
        target_kind,
        EffectTargetKind::FilePath
            | EffectTargetKind::ArtifactId
            | EffectTargetKind::EvidenceId
            | EffectTargetKind::LedgerStream
            | EffectTargetKind::RequestStream
    )
}

#[must_use]
pub fn ready_operation(document: &OperationContractDocument) -> RuntimeReadyReport {
    ready_operation_inner(document, None)
}

#[must_use]
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
    let reference_error_count = reference_validation.as_ref().map_or(0, |report| {
        report
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
            .count()
    });
    let reference_warning_count = reference_validation.as_ref().map_or(0, |report| {
        report
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
            .count()
    });

    let operation = &document.operation_contract;
    let mut reasons = Vec::new();
    let (
        funnel_disposition,
        funnel_phase_profile,
        funnel_reasons,
        protected_boundaries,
        funnel_policy_unavailable,
    ) = match load_accepted_funnel_autonomy_policy()
        .and_then(|policy| evaluate_funnel_operation(policy, operation, &[]))
    {
        Ok(decision) => (
            decision.disposition,
            decision.phase_profile,
            decision.reasons,
            decision.protected_boundaries,
            false,
        ),
        Err(_) => (
            FunnelOperationDisposition::Blocked,
            None,
            Vec::new(),
            operation.risk_boundaries.clone(),
            true,
        ),
    };

    let status = if validation_error_count > 0 {
        reasons.push(RuntimePlanReason::ValidationErrors);
        RuntimePlanStatus::Blocked
    } else if reference_error_count > 0 {
        reasons.push(RuntimePlanReason::ReferenceErrors);
        RuntimePlanStatus::Blocked
    } else if !operation.diagnostics.errors.is_empty() {
        reasons.push(RuntimePlanReason::OperationDiagnosticsErrors);
        RuntimePlanStatus::Blocked
    } else if funnel_policy_unavailable {
        reasons.push(RuntimePlanReason::FunnelPolicyUnavailable);
        RuntimePlanStatus::Blocked
    } else if funnel_disposition == FunnelOperationDisposition::Blocked {
        reasons.push(RuntimePlanReason::FunnelOperationBlocked);
        RuntimePlanStatus::Blocked
    } else if operation.gates.current_gate_status == OperationGateStatus::Blocked {
        reasons.push(RuntimePlanReason::GateBlocked);
        RuntimePlanStatus::Blocked
    } else if operation.human.input_requirement == HumanInputRequirement::Required {
        reasons.push(RuntimePlanReason::HumanInputRequired);
        RuntimePlanStatus::AwaitingHuman
    } else if funnel_disposition == FunnelOperationDisposition::GateRequired {
        reasons.push(RuntimePlanReason::FunnelGateRequired);
        RuntimePlanStatus::GateRequired
    } else if gate_is_missing_or_pending(operation.gates.current_gate_status)
        && !operation.gates.required_before_mutation.is_empty()
    {
        reasons.push(RuntimePlanReason::GateMissingOrPending);
        RuntimePlanStatus::GateRequired
    } else if funnel_disposition == FunnelOperationDisposition::ReviewRequired {
        reasons.push(RuntimePlanReason::FunnelReviewRequired);
        RuntimePlanStatus::ReviewRequired
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
        funnel_disposition,
        funnel_phase_profile,
        funnel_reasons,
        protected_boundaries,
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

fn preview_status(
    status: RuntimePlanStatus,
    blockers: &[RuntimeReadyBlocker],
) -> RuntimePreviewStatus {
    match status {
        RuntimePlanStatus::Blocked => RuntimePreviewStatus::Blocked,
        RuntimePlanStatus::AwaitingHuman => RuntimePreviewStatus::AwaitingHuman,
        RuntimePlanStatus::GateRequired => RuntimePreviewStatus::GateRequired,
        RuntimePlanStatus::ReviewRequired => RuntimePreviewStatus::ReviewRequired,
        RuntimePlanStatus::ReadOnlyStatus => RuntimePreviewStatus::ReadOnly,
        // A plan that is nominally Ready but still carries ready-blockers (e.g.
        // GatePending, RequiredGateStatusUnknown, MissingHostCallEvidence) is
        // not actually executable. Downgrade to Blocked so `preview.status`
        // and `preview.risk_level` agree and `next_human_action` is populated.
        RuntimePlanStatus::ReadyToCallOperation if !blockers.is_empty() => {
            RuntimePreviewStatus::Blocked
        }
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

fn next_human_action(plan: &RuntimePlan, blockers: &[RuntimeReadyBlocker]) -> Option<String> {
    // A nominally-Ready plan with blockers is surfaced as preview.status = Blocked.
    // Provide the same guidance as the explicit Blocked path so consumers do not
    // receive an empty `next_human_action` for an effectively blocked operation.
    let action = match plan.status {
        RuntimePlanStatus::Blocked => "inspect blockers before retrying",
        RuntimePlanStatus::ReadyToCallOperation if !blockers.is_empty() => {
            "resolve blockers before executing"
        }
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
            RuntimePlanReason::FunnelPolicyUnavailable => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::FunnelPolicyUnavailable);
            }
            RuntimePlanReason::FunnelOperationBlocked => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::FunnelOperationBlocked);
            }
            RuntimePlanReason::FunnelGateRequired => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::FunnelGateRequired);
            }
            RuntimePlanReason::FunnelReviewRequired => {
                push_ready_blocker(&mut blockers, RuntimeReadyBlocker::FunnelReviewRequired);
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
    required_gates: &[RequiredGate],
) -> Vec<RuntimeReadyBlocker> {
    let mut blockers = Vec::new();
    match gate_status {
        OperationGateStatus::Pass => {
            // The current gate passed, but if the contract also declares
            // `required_before_mutation` gates we have no signal for those.
            // Surface a blocker so the host cannot silently mutate based on a
            // partial gate verdict. The plan stays Ready (the runtime does not
            // invent a GateRequired status from absence of evidence), but the
            // preview surfaces status=Blocked via F01.1.
            if !required_gates.is_empty() {
                push_ready_blocker(
                    &mut blockers,
                    RuntimeReadyBlocker::RequiredGateStatusUnknown,
                );
            }
        }
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
