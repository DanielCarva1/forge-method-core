//! WAL orchestration: the mutation entrypoint.
//!
//! [`execute_operation`] is the single public path that mutates state. It walks
//! a staged plan, records command evidence, and applies file-effect transactions
//! through the write-ahead log. [`prepare_effect_transaction`] validates one
//! effect against its payload set before the WAL apply.

#![allow(clippy::unused_self)]

use super::*;
use forge_core_store::producer_quiescence::{
    admit_effect_producer, EffectProducerGuard, ProducerBoundaryError,
};
use forge_core_store::{
    append_effect_target_metadata_records_with_durability_under_boundary,
    append_json_line_with_durability_under_boundary,
    apply_file_effect_transaction_with_wal_lock_with_durability_under_boundary,
};
use std::marker::PhantomData;

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

/// Marker: the context has NOT yet been through the gate chain. An
/// `Unverified` context cannot call [`execute_operation`]; it must be
/// transitioned to [`Audited`] via [`audited`](RuntimeOperationExecutionContext::audited)
/// (or `dangerous_unchecked`
/// under the `dangerous-bypass` feature).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unverified;

/// Marker: the context HAS been through the configured gate chain (or has been
/// explicitly marked dangerous-unchecked). Only an `Audited` context can call
/// [`execute_operation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Audited;

/// Fixed, single-root durable layout for a runtime operation.
///
/// The constructor deliberately has no caller-selectable durable paths: evidence,
/// WAL, locks, metadata, and gate traces always resolve below the same
/// `<effect-store-root>/.forge-method` state root. Command working roots remain
/// in [`CommandExecutionContext`] and are read-only.
#[derive(Debug, Clone)]
pub struct RuntimeOperationStorage<'a> {
    effect_store_root: &'a Path,
    state_root: std::path::PathBuf,
}

impl<'a> RuntimeOperationStorage<'a> {
    fn canonical(effect_store_root: &'a Path) -> Self {
        Self {
            effect_store_root,
            state_root: effect_store_root.join(".forge-method"),
        }
    }

    fn validate(&self) -> Result<(), RuntimeOperationStorageError> {
        let effect_root = self.effect_store_root.canonicalize().map_err(|source| {
            RuntimeOperationStorageError::EffectStoreRootUnavailable {
                path: self.effect_store_root.to_path_buf(),
                source: source.to_string(),
            }
        })?;
        if !effect_root.is_dir() {
            return Err(RuntimeOperationStorageError::EffectStoreRootUnavailable {
                path: effect_root,
                source: "effect store root is not a directory".to_owned(),
            });
        }
        let state_root = self.state_root.canonicalize().map_err(|source| {
            RuntimeOperationStorageError::StateRootUnavailable {
                path: self.state_root.clone(),
                source: source.to_string(),
            }
        })?;
        if !state_root.is_dir()
            || state_root.file_name() != Some(std::ffi::OsStr::new(".forge-method"))
            || state_root.parent() != Some(effect_root.as_path())
        {
            return Err(RuntimeOperationStorageError::InvalidStateRoot { path: state_root });
        }
        Ok(())
    }

    fn effect_store_root(&self) -> &'a Path {
        self.effect_store_root
    }

    fn state_root(&self) -> &Path {
        &self.state_root
    }

    fn evidence_relative_path(&self) -> &'static str {
        ".forge-method/evidence/command-execution.ndjson"
    }

    fn wal_relative_path(&self) -> &'static str {
        ".forge-method/wal/effects.ndjson"
    }

    fn lock_relative_path(&self) -> &'static str {
        ".forge-method/locks/effects.lock"
    }

    fn metadata_relative_path(&self) -> &'static str {
        ".forge-method/index/effect-targets.ndjson"
    }
}

/// Typed preflight failure for a durable operation layout. This is returned
/// before admission or gate evaluation, so malformed/mixed roots cannot create
/// an audit trace or any other durable artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeOperationStorageError {
    EffectStoreRootUnavailable {
        path: std::path::PathBuf,
        source: String,
    },
    StateRootUnavailable {
        path: std::path::PathBuf,
        source: String,
    },
    InvalidStateRoot {
        path: std::path::PathBuf,
    },
}

/// The execution context, parameterized over verification state.
///
/// `Unverified` contexts cannot call [`execute_operation`]; only `Audited` can.
/// Transition with [`audited`](RuntimeOperationExecutionContext::audited) after
/// attaching gates via [`with_gate`](RuntimeOperationExecutionContext::with_gate).
///
/// V2.C note: this struct is no longer `Copy` because it owns a
/// `Vec<Box<dyn OperationGate>>`. Callers pass it by reference to
/// [`execute_operation`]. The existing [`single_root`](Self::single_root)
/// constructor is preserved (returns an `Unverified` context) so historical
/// call sites keep compiling after adding the typestate transition.
pub struct RuntimeOperationExecutionContext<'a, S = Unverified> {
    pub command_context: CommandExecutionContext<'a>,
    storage: RuntimeOperationStorage<'a>,
    pub recorded_at: &'a str,
    pub tx_id_prefix: &'a str,
    /// WAL append durability for this operation (ADR-0009).
    pub durability: WalDurability,
    gates: Vec<Box<dyn OperationGate>>,
    _state: PhantomData<S>,
}

impl<'a, S> RuntimeOperationExecutionContext<'a, S> {
    fn from_parts(project_root: &'a Path, effect_store_root: &'a Path) -> Self {
        Self {
            command_context: CommandExecutionContext::single_root(project_root),
            storage: RuntimeOperationStorage::canonical(effect_store_root),
            recorded_at: "unknown",
            tx_id_prefix: "runtime-operation",
            durability: WalDurability::default(),
            gates: Vec::new(),
            _state: PhantomData,
        }
    }
}

impl<'a> RuntimeOperationExecutionContext<'a, Unverified> {
    #[must_use]
    pub fn single_root(root: &'a Path) -> Self {
        Self::from_parts(root, root)
    }

    /// Construct an operation whose read-only command project root differs from
    /// its trusted durable effect-store root.
    #[must_use]
    pub fn project_and_effect_roots(project_root: &'a Path, effect_store_root: &'a Path) -> Self {
        Self::from_parts(project_root, effect_store_root)
    }

    #[must_use]
    pub fn with_gate(mut self, gate: Box<dyn OperationGate>) -> Self {
        self.gates.push(gate);
        self
    }

    #[must_use]
    pub fn audited(self) -> RuntimeOperationExecutionContext<'a, Audited> {
        RuntimeOperationExecutionContext {
            command_context: self.command_context,
            storage: self.storage,
            recorded_at: self.recorded_at,
            tx_id_prefix: self.tx_id_prefix,
            durability: self.durability,
            gates: self.gates,
            _state: PhantomData,
        }
    }

    #[cfg(feature = "dangerous-bypass")]
    #[must_use]
    pub fn dangerous_unchecked(self) -> RuntimeOperationExecutionContext<'a, Audited> {
        tracing::warn!(tx_id_prefix = %self.tx_id_prefix, "RuntimeOperationExecutionContext marked dangerous_unchecked");
        self.audited()
    }
}

impl RuntimeOperationExecutionContext<'_, Audited> {
    #[must_use]
    pub fn gates(&self) -> &[Box<dyn OperationGate>] {
        &self.gates
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
    /// Durable layout was invalid before admission or gate evaluation.
    InvalidDurableLayout,
    /// The operation could not enter the state root's durable producer boundary.
    ProducerAdmissionFailed,
    /// Legacy execution cannot apply multiple effects as separate commits.
    /// Use the operation-wide commit path so the whole write set shares one WAL.
    OperationWideCommitRequired,
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

/// Run the configured gate chain against the operation plan, then execute the
/// mutation (staging, command evidence, effect application, WAL append). Each
/// attached gate is consulted in attachment order before any state is touched.
///
/// # Errors
///
/// Returns `Err(GateRejection)` when the first gate in the chain rejects the
/// planned mutation. Gates are fail-closed: a rejection blocks the WAL append
/// entirely, so the mutation does not take effect. No gate runs after a
/// rejection (first rejection wins).
#[instrument(skip_all, fields(operation_id = tracing::field::Empty, effect_count = effects.len(), command_count = commands.len()), level = "info")]
pub fn execute_operation(
    document: &OperationContractDocument,
    snapshot: RuntimeReadSnapshot<'_>,
    commands: &[RuntimeOperationCommandInput],
    effects: &[RuntimeOperationEffectInput],
    payloads: &[RuntimeOperationEffectPayload],
    context: &RuntimeOperationExecutionContext<'_, Audited>,
) -> Result<RuntimeOperationExecution, GateRejection> {
    let plan = plan_operation_with_snapshot(document, snapshot);
    let operation_id = plan.contract_id.clone();
    tracing::Span::current().record("operation_id", operation_id.0.as_str());

    // Read/status, review, gate, and human-handoff outcomes are complete typed
    // planner results. They must not require a durable producer boundary or run
    // mutation gates merely because a host called the executor entrypoint.
    if plan.status != RuntimePlanStatus::ReadyToCallOperation {
        return Ok(non_ready_operation_execution(operation_id, plan));
    }

    // Durable layout is fixed before admission: no gate receives a caller-chosen
    // trace root and no later write can resolve outside this operation state root.
    if context.storage.validate().is_err() {
        return Ok(runtime_operation_failure(
            operation_id,
            plan,
            RuntimeOperationExecutionReason::InvalidDurableLayout,
        ));
    }
    let authority = match admit_operation_producer(context) {
        Ok(admission) => OperationProducerAuthority { admission },
        Err(_) => {
            return Ok(runtime_operation_failure(
                operation_id,
                plan,
                RuntimeOperationExecutionReason::ProducerAdmissionFailed,
            ));
        }
    };
    let gate_context =
        GateEvaluationContext::new(context.storage.state_root(), authority.boundary());
    for gate in context.gates() {
        gate.evaluate(&plan, &gate_context)?;
    }

    Ok(execute_operation_inner(
        document,
        snapshot,
        commands,
        effects,
        payloads,
        context,
        &authority,
        plan,
        operation_id,
    ))
}

/// Unchanged body of [`execute_operation`], factored out so the public
/// entrypoint can prepend the V2.C gate preamble without touching the
/// plan/staging/command/WAL logic. Takes the already-computed `plan` and
/// `operation_id` so nothing is recomputed. The `context` keeps the typestate
/// `Audited` marker because this is only reachable from an audited context.
// This helper is the central mutate path and genuinely takes 9 arguments: the
// operation document, read snapshot, command inputs, effect inputs, effect
// payloads, audited execution context, retained operation authority, the
// precomputed plan, and the operation id. Splitting or bundling them would hurt
// readability of the step-by-step mutation walk; the signature mirrors
// `execute_operation`'s public shape plus the retained and precomputed values.
// Follows the same rationale as the crate-level
// `#![allow(clippy::too_many_lines)]`.
#[allow(clippy::too_many_arguments)]
fn execute_operation_inner(
    document: &OperationContractDocument,
    snapshot: RuntimeReadSnapshot<'_>,
    commands: &[RuntimeOperationCommandInput],
    effects: &[RuntimeOperationEffectInput],
    payloads: &[RuntimeOperationEffectPayload],
    context: &RuntimeOperationExecutionContext<'_, Audited>,
    authority: &OperationProducerAuthority,
    plan: RuntimePlan,
    operation_id: StableId,
) -> RuntimeOperationExecution {
    // `document`/`snapshot` were consumed only to build `plan` in the public
    // entrypoint; the body below never reads them again. Keep them as params so
    // this helper's signature mirrors the historical shape and the move stays
    // mechanical.
    let _ = (document, snapshot);
    let mut reasons = Vec::new();

    if plan.status != RuntimePlanStatus::ReadyToCallOperation {
        return non_ready_operation_execution(operation_id, plan);
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

    // The legacy executor historically committed each effect independently.
    // A later failure could therefore leave an operation partially applied.
    // Until this entrypoint is wired to the operation-wide bundle, reject the
    // unsafe shape before command evidence or any file mutation is emitted.
    if staging.effect_contract_refs.len() > 1 {
        reasons.push(RuntimeOperationExecutionReason::OperationWideCommitRequired);
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
        if append_json_line_with_durability_under_boundary(
            authority.boundary(),
            context.storage.effect_store_root(),
            context.storage.evidence_relative_path(),
            &evidence,
            context.durability,
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

        let mut application =
            apply_file_effect_transaction_with_wal_lock_with_durability_under_boundary(
                authority.boundary(),
                context.storage.effect_store_root(),
                &effect.document,
                &store_payloads,
                context.storage.wal_relative_path(),
                context.storage.lock_relative_path(),
                effect_tx_id(
                    context.tx_id_prefix,
                    &effect.document.tool_effect_contract.id,
                ),
                context.durability,
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
        if append_effect_target_metadata_records_with_durability_under_boundary(
            authority.boundary(),
            context.storage.effect_store_root(),
            context.storage.metadata_relative_path(),
            &application.metadata_records,
            context.durability,
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

/// Opaque root-scoped effect-producer authority. It is minted only after the
/// kernel validates the durable state root and remains owned through final write.
struct OperationProducerAuthority {
    admission: EffectProducerGuard,
}

impl OperationProducerAuthority {
    fn boundary(&self) -> &EffectProducerGuard {
        &self.admission
    }
}

fn non_ready_operation_execution(
    operation_id: StableId,
    plan: RuntimePlan,
) -> RuntimeOperationExecution {
    let (status, reason) = if plan.status == RuntimePlanStatus::AwaitingHuman {
        (
            RuntimeOperationExecutionStatus::AwaitingHuman,
            RuntimeOperationExecutionReason::PlanAwaitingHuman,
        )
    } else {
        (
            RuntimeOperationExecutionStatus::Blocked,
            RuntimeOperationExecutionReason::PlanNotReady,
        )
    };
    RuntimeOperationExecution {
        status,
        operation_id,
        plan,
        staging: None,
        command_executions: Vec::new(),
        command_evidence_appended: 0,
        effect_transactions: Vec::new(),
        effect_applications: Vec::new(),
        reasons: vec![reason],
    }
}

fn runtime_operation_failure(
    operation_id: StableId,
    plan: RuntimePlan,
    reason: RuntimeOperationExecutionReason,
) -> RuntimeOperationExecution {
    RuntimeOperationExecution {
        status: RuntimeOperationExecutionStatus::Failed,
        operation_id,
        plan,
        staging: None,
        command_executions: Vec::new(),
        command_evidence_appended: 0,
        effect_transactions: Vec::new(),
        effect_applications: Vec::new(),
        reasons: vec![reason],
    }
}

fn admit_operation_producer(
    context: &RuntimeOperationExecutionContext<'_, Audited>,
) -> Result<EffectProducerGuard, ProducerBoundaryError> {
    admit_effect_producer(context.storage.state_root(), false)
}

#[must_use]
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
mod boundary_layout_tests {
    use super::*;

    #[test]
    fn mixed_durable_state_root_is_rejected_before_admission_or_gate_evaluation() {
        let base =
            std::env::temp_dir().join(format!("forge-kernel-mixed-root-{}", std::process::id()));
        let effect_root = base.join("effect");
        let foreign_root = base.join("foreign");
        std::fs::create_dir_all(effect_root.join(".forge-method")).expect("effect state root");
        std::fs::create_dir_all(foreign_root.join(".forge-method")).expect("foreign state root");
        let mut storage = RuntimeOperationStorage::canonical(&effect_root);
        storage.state_root = foreign_root.join(".forge-method");

        assert!(matches!(
            storage.validate(),
            Err(RuntimeOperationStorageError::InvalidStateRoot { .. })
        ));
        let _ = std::fs::remove_dir_all(base);
    }
}
