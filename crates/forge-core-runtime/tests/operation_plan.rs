use forge_core_contracts::command::{
    CommandExecutor, CommandKind, CommandSafety, CommandSideEffectPolicy, CwdPolicy,
    EnvInheritPolicy, EnvPolicy, NetworkPolicy, OutputCapture, OutputPolicy, Platform,
};
use forge_core_contracts::operation::{ForgeOperation, OperationGateStatus};
use forge_core_contracts::tool_effect::EffectTargetKind;
use forge_core_contracts::{
    CommandContract, CommandContractDocument, OperationContractDocument, RepoPath, StableId,
    ToolEffectContractDocument,
};
use forge_core_runtime::{
    command_execution_evidence_record, plan_operation, plan_operation_with_snapshot,
    prepare_effect_transaction, preview_operation_with_snapshot, ready_operation_with_snapshot,
    ready_runtime_plan, run_staged_read_only_command, stage_operation_effects,
    CommandExecutionContext, RuntimeCommandExecutionReason, RuntimeCommandExecutionStatus,
    RuntimeEffectPayload, RuntimeEffectPayloadKind, RuntimeEffectStagingReason,
    RuntimeEffectStagingStatus, RuntimeEffectTransactionReason, RuntimeEffectTransactionStatus,
    RuntimeEvidenceKind, RuntimeOperationCommandInput, RuntimeOperationEffectInput,
    RuntimeOperationEffectPayload, RuntimeOperationExecutionContext,
    RuntimeOperationExecutionReason, RuntimeOperationExecutionStatus, RuntimePlanReason,
    RuntimePlanStatus, RuntimePreviewStatus, RuntimeReadSnapshot, RuntimeReadyBlocker,
    RuntimeReadyEvidenceKind, RuntimeReadyStatus, RuntimeRiskLevel,
};
use forge_core_store::build_reference_index;
use forge_core_validate::ReferenceIndex;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture(name: &str) -> OperationContractDocument {
    let path = repo_root()
        .join("docs")
        .join("fixtures")
        .join("operation-contract-v0")
        .join(name);
    let input = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_yaml::from_str(&input)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn effect_fixture(name: &str) -> ToolEffectContractDocument {
    let path = repo_root().join("contracts").join("effects").join(name);
    let input = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_yaml::from_str(&input)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn story_effect_payloads() -> Vec<RuntimeEffectPayload> {
    vec![
        RuntimeEffectPayload {
            target_ref: ".forge-method/artifacts/story-current-result.yaml".to_string(),
            payload_kind: RuntimeEffectPayloadKind::RuntimeGenerated,
            content_hash: Some("sha256:artifact-payload".to_string()),
            byte_len: 128,
        },
        RuntimeEffectPayload {
            target_ref: ".forge-method/evidence/story-validation.json".to_string(),
            payload_kind: RuntimeEffectPayloadKind::CommandEvidence,
            content_hash: Some("sha256:evidence-payload".to_string()),
            byte_len: 256,
        },
    ]
}

fn runtime_payload(target_ref: &str, content: &[u8]) -> RuntimeOperationEffectPayload {
    RuntimeOperationEffectPayload {
        target_ref: target_ref.to_string(),
        payload_kind: RuntimeEffectPayloadKind::RuntimeGenerated,
        content_hash: format!("sha256:{}", hex_sha256(content)),
        content: content.to_vec(),
    }
}

fn hex_sha256(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static TEMP_ROOT_GUARDS: RefCell<Vec<TempRootGuard>> = const { RefCell::new(Vec::new()) };
}

struct TempRootGuard {
    path: PathBuf,
}

impl Drop for TempRootGuard {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!(
                    "failed to remove temp root {}: {error}",
                    self.path.display()
                );
            }
        }
    }
}

fn fresh_temp_root(label: &str) -> PathBuf {
    let counter = TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "forge-core-runtime-{label}-{}-{timestamp_nanos}-{counter}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temp root");
    TEMP_ROOT_GUARDS.with(|guards| {
        guards
            .borrow_mut()
            .push(TempRootGuard { path: path.clone() });
    });
    path
}

fn cargo_version_command(id: &str) -> CommandContractDocument {
    CommandContractDocument {
        schema_version: "0.1".to_string(),
        command_contract: CommandContract {
            id: StableId(id.to_string()),
            contract_ref: RepoPath("contracts/commands/command-contract-v0.yaml".to_string()),
            kind: CommandKind::Test,
            executor: CommandExecutor::Cargo,
            args: vec!["--version".to_string()],
            cwd_policy: CwdPolicy::ProjectRoot,
            side_effect_policy: CommandSideEffectPolicy::ReadOnly,
            platforms: vec![Platform::Windows, Platform::Macos, Platform::Linux],
            timeout_ms: 30_000,
            env_policy: EnvPolicy {
                inherit: EnvInheritPolicy::Minimal,
                required: vec![],
                forbidden: vec!["FORGE_COMMAND_RUNNER_FORBIDDEN_ENV_SHOULD_NOT_EXIST".to_string()],
            },
            network_policy: NetworkPolicy::Disabled,
            output_policy: OutputPolicy {
                capture: OutputCapture::Summary,
                max_bytes: 4096,
            },
            authority_required: vec![StableId("operation_contract".to_string())],
            safety: CommandSafety {
                shell_string_allowed: false,
                writes_files: false,
                publishes: false,
                installs_packages: false,
            },
        },
    }
}

#[test]
fn facilitation_fixture_waits_for_human() {
    let document = fixture("facilitate-first-product-idea.yaml");
    let plan = plan_operation(&document);

    assert_eq!(plan.status, RuntimePlanStatus::AwaitingHuman);
    assert_eq!(plan.reasons, vec![RuntimePlanReason::HumanInputRequired]);
    assert!(plan.prompt.is_some());
    assert_eq!(plan.validation_error_count, 0);
    assert!(!plan.used_read_snapshot);
}

#[test]
fn mechanical_story_execute_fixture_can_call_operation() {
    let document = fixture("mechanical-story-execute.yaml");
    let plan = plan_operation(&document);

    assert_eq!(plan.status, RuntimePlanStatus::ReadyToCallOperation);
    assert_eq!(plan.reasons, vec![RuntimePlanReason::HostCallAllowed]);
    assert_eq!(plan.next_operation, Some(ForgeOperation::RecordArtifact));
    assert!(!plan.command_refs.is_empty());
    assert!(!plan.effect_contract_refs.is_empty());
    assert_eq!(plan.validation_error_count, 0);
}

#[test]
fn release_gate_fixture_requires_gate_before_advance() {
    let document = fixture("release-gate-required.yaml");
    let plan = plan_operation(&document);

    assert_eq!(plan.status, RuntimePlanStatus::GateRequired);
    assert_eq!(plan.reasons, vec![RuntimePlanReason::GateMissingOrPending]);
    assert_eq!(plan.next_operation, Some(ForgeOperation::Gate));
    assert_eq!(plan.validation_error_count, 0);
}

#[test]
fn destructive_missing_inverse_fixture_blocks() {
    let document = fixture("destructive-effect-missing-inverse-blocked.yaml");
    let plan = plan_operation(&document);

    assert_eq!(plan.status, RuntimePlanStatus::Blocked);
    assert_eq!(plan.reasons, vec![RuntimePlanReason::GateBlocked]);
    assert_ne!(plan.status, RuntimePlanStatus::ReadyToCallOperation);
}

#[test]
fn store_snapshot_keeps_valid_mechanical_operation_ready() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let plan = plan_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));

    assert_eq!(plan.status, RuntimePlanStatus::ReadyToCallOperation);
    assert_eq!(plan.reference_error_count, 0);
    assert!(plan.used_read_snapshot);
}

#[test]
fn missing_snapshot_references_block_ready_operation() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = ReferenceIndex::new();
    let plan = plan_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));

    assert_eq!(plan.status, RuntimePlanStatus::Blocked);
    assert_eq!(plan.reasons, vec![RuntimePlanReason::ReferenceErrors]);
    assert!(plan.reference_error_count > 0);
    assert_ne!(plan.status, RuntimePlanStatus::ReadyToCallOperation);
}

#[test]
fn host_drift_fixture_does_not_become_ready_to_execute() {
    let document = fixture("host-drift-invented-next-step.yaml");
    let plan = plan_operation(&document);

    assert_eq!(plan.status, RuntimePlanStatus::AwaitingHuman);
    assert_eq!(plan.reasons, vec![RuntimePlanReason::HumanInputRequired]);
    assert_ne!(plan.status, RuntimePlanStatus::ReadyToCallOperation);
}

#[test]
fn mechanical_story_plan_stages_commands_and_effects_without_commit() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let plan = plan_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));
    let staging = stage_operation_effects(&plan);

    assert_eq!(staging.status, RuntimeEffectStagingStatus::Staged);
    assert_eq!(
        staging.reasons,
        vec![
            RuntimeEffectStagingReason::StagedCommands,
            RuntimeEffectStagingReason::StagedEffects,
            RuntimeEffectStagingReason::CommitRequiresLaterBoundary
        ]
    );
    assert!(!staging.command_refs.is_empty());
    assert!(!staging.effect_contract_refs.is_empty());
    assert!(!staging.commit_allowed);
}

#[test]
fn preview_report_is_deterministic_and_does_not_mutate() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");

    let preview = preview_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));

    assert_eq!(preview.status, RuntimePreviewStatus::Ready);
    assert_eq!(
        preview.operation_id.0,
        "op_fixture_mechanical_story_execute"
    );
    assert!(!preview.preview_mutates_state);
    assert!(preview.operation_mutates_state);
    assert_eq!(preview.risk_level, RuntimeRiskLevel::Medium);
    assert!(!preview.rollback_available);
    assert!(!preview.command_refs.is_empty());
    assert!(!preview.effect_contract_refs.is_empty());
    assert!(preview.required_gate_refs.is_empty());
    assert!(preview.blockers.is_empty());
    assert!(preview.next_human_action.is_none());
    assert_eq!(preview.plan.status, RuntimePlanStatus::ReadyToCallOperation);
    assert_eq!(preview.staging.status, RuntimeEffectStagingStatus::Staged);
    let serialized = serde_yaml::to_string(&preview).expect("serialize preview report");
    assert!(serialized.contains("operation_id: op_fixture_mechanical_story_execute"));
}

#[test]
fn ready_report_passes_only_for_ready_operation() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");

    let ready = ready_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));

    assert_eq!(ready.status, RuntimeReadyStatus::Ready);
    assert!(ready.ready);
    assert!(ready.blocking_reasons.is_empty());
    assert_eq!(ready.plan_status, RuntimePlanStatus::ReadyToCallOperation);
    assert_eq!(ready.staging_status, RuntimeEffectStagingStatus::Staged);
    assert!(ready
        .evidence
        .iter()
        .any(|item| item.kind == RuntimeReadyEvidenceKind::PlanStatus));
}

#[test]
fn ready_report_fails_closed_for_missing_gate() {
    let document = fixture("release-gate-required.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");

    let ready = ready_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));

    assert_eq!(ready.status, RuntimeReadyStatus::NotReady);
    assert!(!ready.ready);
    assert!(ready
        .blocking_reasons
        .contains(&RuntimeReadyBlocker::GateMissingOrPending));
    assert!(ready
        .blocking_reasons
        .contains(&RuntimeReadyBlocker::GatePending));
    assert!(ready
        .evidence
        .iter()
        .any(|item| item.kind == RuntimeReadyEvidenceKind::RequiredGate));
    assert_eq!(ready.plan_status, RuntimePlanStatus::GateRequired);
}

#[test]
fn ready_report_fails_closed_for_pending_gate_even_without_required_gate() {
    let mut document = fixture("mechanical-story-execute.yaml");
    document.operation_contract.gates.current_gate_status = OperationGateStatus::Pending;
    document
        .operation_contract
        .gates
        .required_before_mutation
        .clear();
    document.operation_contract.gates.gate_contract_refs.clear();
    let index = build_reference_index(repo_root()).expect("reference index");

    let ready = ready_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));
    let preview = preview_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));

    assert_eq!(ready.plan_status, RuntimePlanStatus::ReadyToCallOperation);
    assert_eq!(ready.status, RuntimeReadyStatus::NotReady);
    assert!(!ready.ready);
    assert!(ready
        .blocking_reasons
        .contains(&RuntimeReadyBlocker::GatePending));
    assert!(preview.blockers.contains(&RuntimeReadyBlocker::GatePending));
    assert_eq!(preview.risk_level, RuntimeRiskLevel::Blocked);
}

#[test]
fn ready_report_fails_closed_for_unknown_required_gate_status() {
    let mut document = fixture("release-gate-required.yaml");
    document.operation_contract.gates.current_gate_status = OperationGateStatus::NotApplicable;
    let index = build_reference_index(repo_root()).expect("reference index");

    let ready = ready_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));

    assert_eq!(ready.status, RuntimeReadyStatus::NotReady);
    assert!(ready
        .blocking_reasons
        .contains(&RuntimeReadyBlocker::RequiredGateStatusUnknown));
    assert_eq!(ready.required_gate_refs.len(), 1);
}

#[test]
fn ready_report_fails_closed_for_unknown_gate_status_even_without_required_gate() {
    let mut document = fixture("mechanical-story-execute.yaml");
    document.operation_contract.gates.current_gate_status = OperationGateStatus::NotApplicable;
    document
        .operation_contract
        .gates
        .required_before_mutation
        .clear();
    document.operation_contract.gates.gate_contract_refs.clear();
    let index = build_reference_index(repo_root()).expect("reference index");

    let ready = ready_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));
    let preview = preview_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));

    assert_eq!(ready.plan_status, RuntimePlanStatus::ReadyToCallOperation);
    assert_eq!(ready.status, RuntimeReadyStatus::NotReady);
    assert!(!ready.ready);
    assert!(ready
        .blocking_reasons
        .contains(&RuntimeReadyBlocker::RequiredGateStatusUnknown));
    assert!(preview
        .blockers
        .contains(&RuntimeReadyBlocker::RequiredGateStatusUnknown));
    assert_eq!(preview.risk_level, RuntimeRiskLevel::Blocked);
}

#[test]
fn ready_runtime_plan_fails_closed_without_host_call_evidence() {
    let document = fixture("mechanical-story-execute.yaml");
    let mut plan = plan_operation(&document);
    plan.reasons.clear();

    let ready = ready_runtime_plan(&plan);

    assert_eq!(ready.status, RuntimeReadyStatus::NotReady);
    assert!(!ready.ready);
    assert!(ready
        .blocking_reasons
        .contains(&RuntimeReadyBlocker::MissingHostCallEvidence));
    assert!(ready
        .evidence
        .iter()
        .any(|item| item.kind == RuntimeReadyEvidenceKind::PlanReason && item.detail == "none"));
}

#[test]
fn non_ready_facilitation_plan_is_not_stageable() {
    let document = fixture("facilitate-first-product-idea.yaml");
    let plan = plan_operation(&document);
    let staging = stage_operation_effects(&plan);

    assert_eq!(staging.status, RuntimeEffectStagingStatus::NotStageable);
    assert_eq!(
        staging.reasons,
        vec![RuntimeEffectStagingReason::RuntimePlanNotReady]
    );
    assert!(!staging.commit_allowed);
}

#[test]
fn read_write_conflict_notification_stages_effect_refs_only() {
    let document = fixture("read-write-conflict-notify.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let plan = plan_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));
    let staging = stage_operation_effects(&plan);

    assert_eq!(plan.status, RuntimePlanStatus::ReadyToCallOperation);
    assert_eq!(staging.status, RuntimeEffectStagingStatus::Staged);
    assert!(staging.command_refs.is_empty());
    assert_eq!(staging.effect_contract_refs.len(), 2);
    assert_eq!(
        staging.reasons,
        vec![
            RuntimeEffectStagingReason::StagedEffects,
            RuntimeEffectStagingReason::CommitRequiresLaterBoundary
        ]
    );
}

#[test]
fn staged_read_only_command_executes_with_typed_result() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let plan = plan_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));
    let staging = stage_operation_effects(&plan);
    let command = cargo_version_command("cmd.validate.story_fast");
    let root = repo_root();
    let context = CommandExecutionContext::single_root(&root);

    let execution = run_staged_read_only_command(&staging, &command, &context);

    assert_eq!(execution.status, RuntimeCommandExecutionStatus::Succeeded);
    assert_eq!(
        execution.reasons,
        vec![RuntimeCommandExecutionReason::CommandSucceeded]
    );
    assert_eq!(execution.exit_code, Some(0));
    assert!(!execution.timed_out);
    assert!(execution.stdout.to_lowercase().contains("cargo"));
    assert_eq!(execution.validation_error_count, 0);
}

#[test]
fn command_execution_evidence_record_preserves_runtime_output() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let plan = plan_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));
    let staging = stage_operation_effects(&plan);
    let command = cargo_version_command("cmd.validate.story_fast");
    let root = repo_root();
    let context = CommandExecutionContext::single_root(&root);
    let execution = run_staged_read_only_command(&staging, &command, &context);

    let record = command_execution_evidence_record(&staging, &execution, "2026-06-25T00:00:00Z");

    assert_eq!(record.schema_version, "0.1");
    assert_eq!(record.record_kind, RuntimeEvidenceKind::CommandExecution);
    assert_eq!(record.operation_id.0, "op_fixture_mechanical_story_execute");
    assert_eq!(record.command_id.0, "cmd.validate.story_fast");
    assert_eq!(record.status, RuntimeCommandExecutionStatus::Succeeded);
    assert_eq!(
        record.reasons,
        vec![RuntimeCommandExecutionReason::CommandSucceeded]
    );
    assert!(record.stdout.to_lowercase().contains("cargo"));
}

#[test]
fn unstaged_command_is_not_run() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let plan = plan_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));
    let staging = stage_operation_effects(&plan);
    let command = cargo_version_command("cmd.validate.not_staged");
    let root = repo_root();
    let context = CommandExecutionContext::single_root(&root);

    let execution = run_staged_read_only_command(&staging, &command, &context);

    assert_eq!(execution.status, RuntimeCommandExecutionStatus::NotRun);
    assert_eq!(
        execution.reasons,
        vec![RuntimeCommandExecutionReason::CommandNotStaged]
    );
    assert_eq!(execution.exit_code, None);
    assert!(execution.stdout.is_empty());
}

#[test]
fn staged_mutating_command_is_blocked_before_spawn() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let plan = plan_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));
    let staging = stage_operation_effects(&plan);
    let mut command = cargo_version_command("cmd.validate.story_fast");
    command.command_contract.side_effect_policy = CommandSideEffectPolicy::WriteProjectFiles;
    command.command_contract.safety.writes_files = true;
    let root = repo_root();
    let context = CommandExecutionContext::single_root(&root);

    let execution = run_staged_read_only_command(&staging, &command, &context);

    assert_eq!(execution.status, RuntimeCommandExecutionStatus::Blocked);
    assert!(execution
        .reasons
        .contains(&RuntimeCommandExecutionReason::NonReadOnlyCommand));
    assert!(execution
        .reasons
        .contains(&RuntimeCommandExecutionReason::UnsafeCommandSafetyFlags));
}

#[test]
fn shell_executor_is_blocked_even_when_staged() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let plan = plan_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));
    let staging = stage_operation_effects(&plan);
    let mut command = cargo_version_command("cmd.validate.story_fast");
    command.command_contract.executor = CommandExecutor::Sh;
    command.command_contract.args = vec!["--version".to_string()];
    let root = repo_root();
    let context = CommandExecutionContext::single_root(&root);

    let execution = run_staged_read_only_command(&staging, &command, &context);

    assert_eq!(execution.status, RuntimeCommandExecutionStatus::Blocked);
    assert_eq!(
        execution.reasons,
        vec![RuntimeCommandExecutionReason::ShellExecutorBlocked]
    );
}

#[test]
fn zero_timeout_command_is_blocked_before_spawn() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let plan = plan_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));
    let staging = stage_operation_effects(&plan);
    let mut command = cargo_version_command("cmd.validate.story_fast");
    command.command_contract.timeout_ms = 0;
    let root = repo_root();
    let context = CommandExecutionContext::single_root(&root);

    let execution = run_staged_read_only_command(&staging, &command, &context);

    assert_eq!(execution.status, RuntimeCommandExecutionStatus::Blocked);
    assert_eq!(
        execution.reasons,
        vec![RuntimeCommandExecutionReason::TimeoutMustBePositive]
    );
}

#[test]
fn effect_transaction_blocks_write_without_payload() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let plan = plan_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));
    let staging = stage_operation_effects(&plan);
    let effect_ref = RepoPath("contracts/effects/story-artifact-write-effect.yaml".to_string());
    let effect = effect_fixture("story-artifact-write-effect.yaml");

    let transaction = prepare_effect_transaction(&staging, &effect_ref, &effect, &[]);

    assert_eq!(transaction.status, RuntimeEffectTransactionStatus::Blocked);
    assert_eq!(
        transaction.operation_id.0,
        "op_fixture_mechanical_story_execute"
    );
    assert_eq!(
        transaction.effect_id.0,
        "effect.fixture.story_artifact_write"
    );
    assert_eq!(transaction.write_count, 2);
    assert_eq!(transaction.payload_count, 0);
    assert!(transaction
        .reasons
        .contains(&RuntimeEffectTransactionReason::MissingPayloadForWrite));
}

#[test]
fn effect_transaction_is_ready_when_staged_effect_has_payloads() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let plan = plan_operation_with_snapshot(&document, RuntimeReadSnapshot::new(&index));
    let staging = stage_operation_effects(&plan);
    let effect_ref = RepoPath("contracts/effects/story-artifact-write-effect.yaml".to_string());
    let effect = effect_fixture("story-artifact-write-effect.yaml");
    let payloads = story_effect_payloads();

    let transaction = prepare_effect_transaction(&staging, &effect_ref, &effect, &payloads);

    assert_eq!(transaction.status, RuntimeEffectTransactionStatus::Ready);
    assert_eq!(
        transaction.reasons,
        vec![RuntimeEffectTransactionReason::TransactionReady]
    );
    assert_eq!(transaction.write_count, 2);
    assert_eq!(transaction.payload_count, 2);
    assert_eq!(transaction.validation_error_count, 0);
}

#[test]
fn execute_operation_waits_without_side_effects_when_plan_needs_human() {
    let document = fixture("facilitate-first-product-idea.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let temp_root = fresh_temp_root("awaiting-human");
    let context = RuntimeOperationExecutionContext::single_root(&temp_root);

    let execution = forge_core_runtime::execute_operation(
        &document,
        RuntimeReadSnapshot::new(&index),
        &[],
        &[],
        &[],
        &context,
    );

    assert_eq!(
        execution.status,
        RuntimeOperationExecutionStatus::AwaitingHuman
    );
    assert_eq!(
        execution.reasons,
        vec![RuntimeOperationExecutionReason::PlanAwaitingHuman]
    );
    assert!(execution.staging.is_none());
    assert!(execution.command_executions.is_empty());
    assert!(execution.effect_applications.is_empty());
    assert!(!temp_root.join(".forge-method").exists());
}

#[test]
fn execute_operation_blocks_missing_required_command_before_effects() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let temp_root = fresh_temp_root("missing-command");
    let context = RuntimeOperationExecutionContext::single_root(&temp_root);

    let execution = forge_core_runtime::execute_operation(
        &document,
        RuntimeReadSnapshot::new(&index),
        &[],
        &[],
        &[],
        &context,
    );

    assert_eq!(execution.status, RuntimeOperationExecutionStatus::Blocked);
    assert_eq!(
        execution.reasons,
        vec![RuntimeOperationExecutionReason::MissingRequiredCommandContract]
    );
    assert!(execution.command_executions.is_empty());
    assert!(execution.effect_transactions.is_empty());
    assert!(execution.effect_applications.is_empty());
    assert!(!temp_root.join(".forge-method/artifacts").exists());
}

#[test]
fn execute_operation_records_command_evidence_and_applies_effect_with_wal_lock() {
    let document = fixture("mechanical-story-execute.yaml");
    let index = build_reference_index(repo_root()).expect("reference index");
    let command = RuntimeOperationCommandInput {
        document: cargo_version_command("cmd.validate.story_fast"),
    };
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
    let temp_root = fresh_temp_root("execute-success");
    let context = RuntimeOperationExecutionContext {
        command_context: CommandExecutionContext::single_root(&temp_root),
        evidence_log_relative_path: ".forge-method/evidence/command-execution.ndjson",
        wal_relative_path: ".forge-method/wal/effects.ndjson",
        lock_relative_path: ".forge-method/locks/effects.lock",
        effect_metadata_index_relative_path: ".forge-method/index/effect-targets.ndjson",
        recorded_at: "2026-06-25T00:00:00Z",
        tx_id_prefix: "test-execute-operation",
    };

    let execution = forge_core_runtime::execute_operation(
        &document,
        RuntimeReadSnapshot::new(&index),
        &[command],
        &[effect_input],
        &[artifact_payload, evidence_payload],
        &context,
    );

    assert_eq!(
        execution.status,
        RuntimeOperationExecutionStatus::Completed,
        "{execution:#?}"
    );
    assert_eq!(
        execution.reasons,
        vec![RuntimeOperationExecutionReason::OperationCompleted]
    );
    assert_eq!(execution.command_executions.len(), 1);
    assert_eq!(execution.command_evidence_appended, 1);
    assert_eq!(execution.effect_transactions.len(), 1);
    assert_eq!(execution.effect_applications.len(), 1);
    assert!(temp_root
        .join(".forge-method/evidence/command-execution.ndjson")
        .exists());
    assert!(temp_root
        .join(".forge-method/artifacts/story-current-result.yaml")
        .exists());
    assert!(temp_root
        .join(".forge-method/evidence/story-validation.json")
        .exists());
    assert!(temp_root.join(".forge-method/wal/effects.ndjson").exists());
    let effect_index =
        fs::read_to_string(temp_root.join(".forge-method/index/effect-targets.ndjson"))
            .expect("read effect metadata index");
    assert!(effect_index
        .contains("\"logical_ref\":\".forge-method/artifacts/story-current-result.yaml\""));
    assert!(effect_index
        .contains("\"physical_ref\":\".forge-method/artifacts/story-current-result.yaml\""));
    assert!(effect_index.contains("\"recorded_at\":\"2026-06-25T00:00:00Z\""));
    assert!(!effect_index.contains("story: completed"));
}
