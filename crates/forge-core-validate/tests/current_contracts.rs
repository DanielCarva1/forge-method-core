use forge_core_contracts::{
    ClaimContractDocument, CommandContractDocument, CompletionContractDocument,
    ContractFamilyInventoryDocument, CoordinationEvalContractDocument,
    DecisionCloseContractDocument, FieldEvidenceRegistry, GateContractDocument,
    HealthRecoveryContractDocument, OperationContractDocument, RequestContractDocument,
    RuntimeCapabilityDocument, RuntimeHandoffContractDocument, RuntimeRegistryEntryDocument,
    ToolEffectContractDocument,
};
use forge_core_validate::{
    validate_claim, validate_claim_cross_references, validate_command, validate_completion,
    validate_completion_cross_references, validate_coordination_eval,
    validate_coordination_eval_cross_references, validate_decision_close,
    validate_decision_close_cross_references, validate_evidence_registry, validate_gate,
    validate_gate_cross_references, validate_health_recovery,
    validate_health_recovery_cross_references, validate_inventory, validate_inventory_references,
    validate_operation, validate_operation_cross_references, validate_request,
    validate_request_cross_references, validate_runtime_capability, validate_runtime_handoff,
    validate_runtime_handoff_cross_references, validate_runtime_registry_cross_references,
    validate_runtime_registry_entry, validate_tool_effect, validate_tool_effect_cross_references,
    validate_yaml_known_repo_references, validate_yaml_source_id_references, DiagnosticCode,
    ParsedYamlDocument, ReferenceIndex, ReferenceKind,
};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

fn read_yaml<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let text =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_yaml::from_str(&text)
        .unwrap_or_else(|err| panic!("deserialize {}: {err}", path.display()))
}

#[test]
fn validates_current_evidence_and_inventory() {
    let root = repo_root();
    let evidence: FieldEvidenceRegistry = read_yaml(
        &root
            .join("contracts")
            .join("research")
            .join("field-evidence-20260625.yaml"),
    );
    let inventory: ContractFamilyInventoryDocument = read_yaml(
        &root
            .join("contracts")
            .join("inventory")
            .join("v0-contract-family-lock.yaml"),
    );

    let evidence_report = validate_evidence_registry(&evidence);
    assert!(
        !evidence_report.has_errors(),
        "evidence diagnostics: {:?}",
        evidence_report.diagnostics()
    );

    let inventory_report = validate_inventory(&inventory, &evidence);
    assert!(
        !inventory_report.has_errors(),
        "inventory diagnostics: {:?}",
        inventory_report.diagnostics()
    );
}

#[test]
fn validates_current_command_contract() {
    let command: CommandContractDocument = read_yaml(
        &repo_root()
            .join("contracts")
            .join("commands")
            .join("story-validation-fast.yaml"),
    );

    let report = validate_command(&command);
    assert!(
        !report.has_errors(),
        "command diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn validates_current_operation_fixtures() {
    let dir = repo_root()
        .join("docs")
        .join("fixtures")
        .join("operation-contract-v0");
    let mut count = 0usize;

    for entry in fs::read_dir(&dir).expect("read operation fixture dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("yaml") {
            continue;
        }
        let operation: OperationContractDocument = read_yaml(&path);
        let report = validate_operation(&operation);
        assert!(
            !report.has_errors(),
            "operation diagnostics for {}: {:?}",
            path.display(),
            report.diagnostics()
        );
        count += 1;
    }

    assert_eq!(count, 23);
}

#[test]
fn validates_current_claim_completion_and_gate_instances() {
    assert_valid_instances::<ClaimContractDocument, _>(
        "contracts/claims",
        "claim-contract-v0.yaml",
        validate_claim,
        4,
    );
    assert_valid_instances::<CompletionContractDocument, _>(
        "contracts/completion",
        "completion-contract-v0.yaml",
        validate_completion,
        2,
    );
    assert_valid_instances::<GateContractDocument, _>(
        "contracts/gates",
        "gate-contract-v0.yaml",
        validate_gate,
        5,
    );
}

#[test]
fn validates_current_request_and_effect_instances() {
    assert_valid_instances::<RequestContractDocument, _>(
        "contracts/requests",
        "request-contract-v0.yaml",
        validate_request,
        5,
    );
    assert_valid_instances::<ToolEffectContractDocument, _>(
        "contracts/effects",
        "tool-effect-contract-v0.yaml",
        validate_tool_effect,
        10,
    );
}

#[test]
fn validates_current_decision_close_instances() {
    assert_valid_instances::<DecisionCloseContractDocument, _>(
        "contracts/decisions",
        "decision-close-contract-v0.yaml",
        validate_decision_close,
        2,
    );
}

#[test]
fn validates_current_runtime_boundary_instances() {
    let root = repo_root();
    let suggestible: RuntimeHandoffContractDocument =
        read_yaml(&root.join("contracts/runtimes/cursor-browser-validation-runtime.yaml"));
    let blocked: RuntimeHandoffContractDocument = read_yaml(
        &root.join("contracts/runtimes/cursor-browser-validation-missing-capability.yaml"),
    );
    let registry: RuntimeRegistryEntryDocument =
        read_yaml(&root.join("contracts/runtimes/registry-cursor-browser-agent.yaml"));
    let capability: RuntimeCapabilityDocument =
        read_yaml(&root.join("contracts/runtimes/capability-browser-validation.yaml"));

    for (label, report) in [
        ("suggestible", validate_runtime_handoff(&suggestible)),
        ("blocked", validate_runtime_handoff(&blocked)),
        ("registry", validate_runtime_registry_entry(&registry)),
        ("capability", validate_runtime_capability(&capability)),
    ] {
        assert!(
            !report.has_errors(),
            "runtime diagnostics for {label}: {:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn validates_current_health_recovery_instances() {
    assert_valid_instances::<HealthRecoveryContractDocument, _>(
        "contracts/recovery",
        "health-recovery-contract-v0.yaml",
        validate_health_recovery,
        2,
    );
}

#[test]
fn validates_current_coordination_eval_instances() {
    assert_valid_instances::<CoordinationEvalContractDocument, _>(
        "contracts/evals",
        "coordination-eval-contract-v0.yaml",
        validate_coordination_eval,
        1,
    );
}

#[test]
fn validates_current_cross_file_references_with_pure_index() {
    let root = repo_root();
    let index = current_reference_index(&root);

    let inventory: ContractFamilyInventoryDocument =
        read_yaml(&root.join("contracts/inventory/v0-contract-family-lock.yaml"));
    assert_report_ok(
        "inventory refs",
        validate_inventory_references(&inventory, &index),
    );

    let operation_dir = root.join("docs/fixtures/operation-contract-v0");
    for entry in fs::read_dir(&operation_dir).expect("read operation fixture dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|value| value.to_str()) == Some("yaml") {
            let operation: OperationContractDocument = read_yaml(&path);
            assert_report_ok(
                &format!("operation refs {}", path.display()),
                validate_operation_cross_references(&operation, &index),
            );
        }
    }

    assert_cross_ref_instances::<ClaimContractDocument, _>(
        "contracts/claims",
        "claim-contract-v0.yaml",
        &index,
        validate_claim_cross_references,
        4,
    );
    assert_cross_ref_instances::<CompletionContractDocument, _>(
        "contracts/completion",
        "completion-contract-v0.yaml",
        &index,
        validate_completion_cross_references,
        2,
    );
    assert_cross_ref_instances::<GateContractDocument, _>(
        "contracts/gates",
        "gate-contract-v0.yaml",
        &index,
        validate_gate_cross_references,
        5,
    );
    assert_cross_ref_instances::<RequestContractDocument, _>(
        "contracts/requests",
        "request-contract-v0.yaml",
        &index,
        validate_request_cross_references,
        5,
    );
    assert_cross_ref_instances::<ToolEffectContractDocument, _>(
        "contracts/effects",
        "tool-effect-contract-v0.yaml",
        &index,
        validate_tool_effect_cross_references,
        10,
    );
    assert_cross_ref_instances::<DecisionCloseContractDocument, _>(
        "contracts/decisions",
        "decision-close-contract-v0.yaml",
        &index,
        validate_decision_close_cross_references,
        2,
    );
    assert_named_cross_ref::<RuntimeHandoffContractDocument, _>(
        "contracts/runtimes/cursor-browser-validation-runtime.yaml",
        &index,
        validate_runtime_handoff_cross_references,
    );
    assert_named_cross_ref::<RuntimeHandoffContractDocument, _>(
        "contracts/runtimes/cursor-browser-validation-missing-capability.yaml",
        &index,
        validate_runtime_handoff_cross_references,
    );
    assert_named_cross_ref::<RuntimeRegistryEntryDocument, _>(
        "contracts/runtimes/registry-cursor-browser-agent.yaml",
        &index,
        validate_runtime_registry_cross_references,
    );
    assert_cross_ref_instances::<HealthRecoveryContractDocument, _>(
        "contracts/recovery",
        "health-recovery-contract-v0.yaml",
        &index,
        validate_health_recovery_cross_references,
        2,
    );
    assert_cross_ref_instances::<CoordinationEvalContractDocument, _>(
        "contracts/evals",
        "coordination-eval-contract-v0.yaml",
        &index,
        validate_coordination_eval_cross_references,
        1,
    );
}

#[test]
fn typed_reference_validation_reports_missing_and_wrong_kind() {
    let root = repo_root();
    let inventory: ContractFamilyInventoryDocument =
        read_yaml(&root.join("contracts/inventory/v0-contract-family-lock.yaml"));

    let missing_report = validate_inventory_references(&inventory, &ReferenceIndex::new());
    assert!(
        missing_report
            .diagnostics()
            .iter()
            .any(|item| item.code == DiagnosticCode::MissingReference),
        "missing report diagnostics: {:?}",
        missing_report.diagnostics()
    );

    let mut wrong_kind_index = ReferenceIndex::new();
    wrong_kind_index.insert(
        "contracts/inventory/contract-family-inventory-v0.yaml",
        ReferenceKind::Policy,
    );
    let wrong_kind_report = validate_inventory_references(&inventory, &wrong_kind_index);
    assert!(
        wrong_kind_report
            .diagnostics()
            .iter()
            .any(|item| item.code == DiagnosticCode::ReferenceKindMismatch),
        "wrong-kind report diagnostics: {:?}",
        wrong_kind_report.diagnostics()
    );
}

#[test]
fn yaml_source_id_validation_reports_unknown_sources() {
    let root = repo_root();
    let evidence: FieldEvidenceRegistry = read_yaml(
        &root
            .join("contracts")
            .join("research")
            .join("field-evidence-20260625.yaml"),
    );
    let document = ParsedYamlDocument {
        path: "contracts/policies/synthetic.yaml".to_string(),
        value: serde_yaml::from_str(
            r#"
schema_version: "0.1"
evidence_basis:
  direct_patterns:
    - source_id: "missing_source_for_test"
"#,
        )
        .expect("synthetic yaml"),
    };

    let report = validate_yaml_source_id_references(&[document], &evidence);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|item| item.code == DiagnosticCode::UnknownEvidenceSourceRef),
        "source id diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn yaml_known_repo_ref_validation_reports_missing_refs() {
    let document = ParsedYamlDocument {
        path: "contracts/inventory/synthetic.yaml".to_string(),
        value: serde_yaml::from_str(
            r#"
schema_version: "0.1"
contract_family_inventory:
  supporting_policy_refs:
    - "contracts/policies/missing-policy.yaml"
"#,
        )
        .expect("synthetic yaml"),
    };

    let report = validate_yaml_known_repo_references(&[document], &HashSet::new());
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|item| item.code == DiagnosticCode::MissingKnownRepoRef),
        "known-ref diagnostics: {:?}",
        report.diagnostics()
    );
}

fn assert_valid_instances<T, F>(
    relative_dir: &str,
    definition_file: &str,
    validate: F,
    expected_count: usize,
) where
    T: serde::de::DeserializeOwned,
    F: Fn(&T) -> forge_core_validate::ValidationReport,
{
    let dir = repo_root().join(relative_dir);
    let mut count = 0usize;

    for entry in fs::read_dir(&dir).expect("read contract instance dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("yaml")
            || path.file_name().and_then(|value| value.to_str()) == Some(definition_file)
        {
            continue;
        }
        let contract: T = read_yaml(&path);
        let report = validate(&contract);
        assert!(
            !report.has_errors(),
            "contract diagnostics for {}: {:?}",
            path.display(),
            report.diagnostics()
        );
        count += 1;
    }

    assert_eq!(count, expected_count);
}

fn assert_cross_ref_instances<T, F>(
    relative_dir: &str,
    definition_file: &str,
    index: &ReferenceIndex,
    validate: F,
    expected_count: usize,
) where
    T: serde::de::DeserializeOwned,
    F: Fn(&T, &ReferenceIndex) -> forge_core_validate::ValidationReport,
{
    let dir = repo_root().join(relative_dir);
    let mut count = 0usize;

    for entry in fs::read_dir(&dir).expect("read contract instance dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("yaml")
            || path.file_name().and_then(|value| value.to_str()) == Some(definition_file)
        {
            continue;
        }
        let contract: T = read_yaml(&path);
        assert_report_ok(
            &format!("cross refs {}", path.display()),
            validate(&contract, index),
        );
        count += 1;
    }

    assert_eq!(count, expected_count);
}

fn assert_named_cross_ref<T, F>(relative_path: &str, index: &ReferenceIndex, validate: F)
where
    T: serde::de::DeserializeOwned,
    F: Fn(&T, &ReferenceIndex) -> forge_core_validate::ValidationReport,
{
    let path = repo_root().join(relative_path);
    let contract: T = read_yaml(&path);
    assert_report_ok(
        &format!("cross refs {}", path.display()),
        validate(&contract, index),
    );
}

fn assert_report_ok(label: &str, report: forge_core_validate::ValidationReport) {
    assert!(!report.has_errors(), "{label}: {:?}", report.diagnostics());
}

fn current_reference_index(root: &Path) -> ReferenceIndex {
    let mut index = ReferenceIndex::new();

    add_contract_definitions(&mut index);
    add_policy_files(&mut index, root);
    add_operation_fixtures(&mut index, root);
    add_contract_instances(&mut index, root);
    add_virtual_runtime_projection_refs(&mut index);

    index
}

fn add_contract_definitions(index: &mut ReferenceIndex) {
    for reference in [
        "contracts/commands/command-contract-v0.yaml",
        "contracts/claims/claim-contract-v0.yaml",
        "contracts/completion/completion-contract-v0.yaml",
        "contracts/decisions/decision-close-contract-v0.yaml",
        "contracts/effects/tool-effect-contract-v0.yaml",
        "contracts/evals/coordination-eval-contract-v0.yaml",
        "contracts/gates/gate-contract-v0.yaml",
        "contracts/inventory/contract-family-inventory-v0.yaml",
        "contracts/operations/operation-contract-v0.yaml",
        "contracts/requests/request-contract-v0.yaml",
        "contracts/recovery/health-recovery-contract-v0.yaml",
        "contracts/runtimes/runtime-handoff-contract-v0.yaml",
    ] {
        index.insert(reference, ReferenceKind::ContractDefinition);
    }
}

fn add_policy_files(index: &mut ReferenceIndex, root: &Path) {
    let policy_dir = root.join("contracts/policies");
    for entry in fs::read_dir(&policy_dir).expect("read policy dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|value| value.to_str()) == Some("yaml") {
            index.insert(repo_relative(root, &path), ReferenceKind::Policy);
        }
    }
    index.insert(
        "contracts/operations/operation-reference-policy-v0.yaml",
        ReferenceKind::Policy,
    );
}

fn add_operation_fixtures(index: &mut ReferenceIndex, root: &Path) {
    let dir = root.join("docs/fixtures/operation-contract-v0");
    for entry in fs::read_dir(&dir).expect("read operation fixture dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|value| value.to_str()) == Some("yaml") {
            index.insert(repo_relative(root, &path), ReferenceKind::OperationFixture);
        }
    }
}

fn add_contract_instances(index: &mut ReferenceIndex, root: &Path) {
    add_instance_dir(
        index,
        root,
        "contracts/claims",
        "claim-contract-v0.yaml",
        ReferenceKind::ClaimContract,
    );
    add_instance_dir(
        index,
        root,
        "contracts/completion",
        "completion-contract-v0.yaml",
        ReferenceKind::CompletionContract,
    );
    add_instance_dir(
        index,
        root,
        "contracts/gates",
        "gate-contract-v0.yaml",
        ReferenceKind::GateContract,
    );
    add_instance_dir(
        index,
        root,
        "contracts/requests",
        "request-contract-v0.yaml",
        ReferenceKind::RequestContract,
    );
    add_instance_dir(
        index,
        root,
        "contracts/effects",
        "tool-effect-contract-v0.yaml",
        ReferenceKind::ToolEffectContract,
    );
    add_instance_dir(
        index,
        root,
        "contracts/decisions",
        "decision-close-contract-v0.yaml",
        ReferenceKind::DecisionCloseContract,
    );
    add_instance_dir(
        index,
        root,
        "contracts/recovery",
        "health-recovery-contract-v0.yaml",
        ReferenceKind::HealthRecoveryContract,
    );
    add_instance_dir(
        index,
        root,
        "contracts/evals",
        "coordination-eval-contract-v0.yaml",
        ReferenceKind::CoordinationEvalContract,
    );

    index.insert(
        "contracts/runtimes/cursor-browser-validation-runtime.yaml",
        ReferenceKind::RuntimeHandoffContract,
    );
    index.insert(
        "contracts/runtimes/cursor-browser-validation-missing-capability.yaml",
        ReferenceKind::RuntimeHandoffContract,
    );
    index.insert(
        "contracts/runtimes/registry-cursor-browser-agent.yaml",
        ReferenceKind::RuntimeRegistryEntry,
    );
    index.insert(
        "contracts/runtimes/capability-browser-validation.yaml",
        ReferenceKind::RuntimeCapability,
    );

    let command: CommandContractDocument =
        read_yaml(&root.join("contracts/commands/story-validation-fast.yaml"));
    index.insert(
        command.command_contract.id.0,
        ReferenceKind::CommandContract,
    );
}

fn add_instance_dir(
    index: &mut ReferenceIndex,
    root: &Path,
    relative_dir: &str,
    definition_file: &str,
    kind: ReferenceKind,
) {
    let dir = root.join(relative_dir);
    for entry in fs::read_dir(&dir).expect("read instance dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|value| value.to_str()) == Some("yaml")
            && path.file_name().and_then(|value| value.to_str()) != Some(definition_file)
        {
            index.insert(repo_relative(root, &path), kind);
        }
    }
}

fn add_virtual_runtime_projection_refs(index: &mut ReferenceIndex) {
    index.insert(
        ".forge-method/agents/registry.yaml",
        ReferenceKind::RuntimeRegistryProjection,
    );
    index.insert(".forge-method/ledger.ndjson", ReferenceKind::Ledger);
}

fn repo_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("path under root")
        .to_string_lossy()
        .replace('\\', "/")
}
