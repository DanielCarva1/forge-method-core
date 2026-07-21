use forge_core_contracts::inventory::{FamilyStatus, ValidationSurface};
use forge_core_contracts::{
    AssuranceCaseDocument, ClaimContractDocument, CommandContractDocument,
    CompletionContractDocument, ContractFamilyInventoryDocument, CoordinationEvalContractDocument,
    DecisionCloseContractDocument, FieldEvidenceRegistry, GateContractDocument,
    HealthRecoveryContractDocument, OperationContractDocument,
    OperationCrossReferencePolicyDocument, RequestContractDocument, RuntimeCapabilityDocument,
    RuntimeHandoffContractDocument, RuntimeRegistryEntryDocument, ToolEffectContractDocument,
};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

#[test]
fn deserializes_field_evidence_registry() {
    let path = repo_root()
        .join("contracts")
        .join("research")
        .join("field-evidence-20260625.yaml");
    let text = fs::read_to_string(&path).expect("read field evidence registry");
    let registry: FieldEvidenceRegistry =
        yaml_serde::from_str(&text).expect("deserialize registry");

    assert!(registry.sources.len() >= 48);
    assert!(registry
        .sources
        .iter()
        .any(|source| source.id.0 == "schema_first_tool_apis_2603_13404"));
    assert!(registry
        .policy
        .geographic_coverage
        .minimum_behavior
        .iter()
        .any(|rule| rule.contains("Chinese-origin")));
}

#[test]
fn deserializes_contract_family_inventory_lock() {
    let path = repo_root()
        .join("contracts")
        .join("inventory")
        .join("v0-contract-family-lock.yaml");
    let text = fs::read_to_string(&path).expect("read inventory lock");
    let inventory: ContractFamilyInventoryDocument =
        yaml_serde::from_str(&text).expect("deserialize inventory lock");

    assert_eq!(
        inventory.contract_family_inventory.id.0,
        "inventory.v0.contract_family_lock"
    );
    assert!(inventory
        .contract_family_inventory
        .supporting_policy_refs
        .iter()
        .any(|path| path.0 == "contracts/policies/rust-contract-type-order.yaml"));
    assert!(inventory.contract_family_inventory.families.len() >= 12);
    let retirement = inventory
        .contract_family_inventory
        .families
        .iter()
        .filter(|family| {
            family.id.0.starts_with("workflow_retirement_")
                || matches!(
                    family.id.0.as_str(),
                    "workflow_deletion_proof"
                        | "workflow_consumer_compatibility_report"
                        | "workflow_consumer_compatibility_matrix"
                        | "workflow_final_scorecard"
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(retirement.len(), 8);
    assert!(retirement.iter().all(|family| {
        family.schema_ref.0 == "contracts/spec/workflow-governance-retirement-v0.yaml"
    }));
    let authorization_v2 = retirement
        .iter()
        .find(|family| family.id.0 == "workflow_retirement_authorization_v2")
        .expect("retirement V2 inventory family");
    assert_eq!(authorization_v2.status, FamilyStatus::Active);
    assert_eq!(
        authorization_v2.validation_surface,
        ValidationSurface::RustContractValidator
    );
    assert_eq!(
        authorization_v2.validator_function,
        "verify_workflow_retirement_authorization_v2"
    );
}

#[test]
fn deserializes_command_contract_instance() {
    let path = repo_root()
        .join("contracts")
        .join("commands")
        .join("story-validation-fast.yaml");
    let text = fs::read_to_string(&path).expect("read command contract");
    let command: CommandContractDocument =
        yaml_serde::from_str(&text).expect("deserialize command contract");

    assert_eq!(command.command_contract.id.0, "cmd.validate.story_fast");
    assert!(!command.command_contract.args.is_empty());
    assert!(!command.command_contract.safety.shell_string_allowed);
}

#[test]
fn deserializes_operation_reference_policy() {
    let path = repo_root()
        .join("contracts")
        .join("operations")
        .join("operation-reference-policy-v0.yaml");
    let text = fs::read_to_string(&path).expect("read operation reference policy");
    let policy: OperationCrossReferencePolicyDocument =
        yaml_serde::from_str(&text).expect("deserialize operation reference policy");

    assert_eq!(policy.contract.0, "operation_reference_policy");
    assert!(policy
        .allowed_reference_fields
        .iter()
        .any(|field| field.field_path == "effect_contract_refs[]"));
}

#[test]
fn deserializes_all_operation_fixtures() {
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
        let text = fs::read_to_string(&path).expect("read operation fixture");
        let operation: OperationContractDocument =
            yaml_serde::from_str(&text).unwrap_or_else(|err| {
                panic!("deserialize operation fixture {}: {err}", path.display())
            });

        assert_eq!(operation.operation_contract.schema_version, "0.1");
        assert!(!operation.operation_contract.contract_id.0.is_empty());
        assert!(!operation.operation_contract.stop_conditions.is_empty());
        count += 1;
    }

    assert_eq!(count, 24);
}

#[test]
fn deserializes_claim_completion_and_gate_instances() {
    assert_yaml_instances::<ClaimContractDocument>("contracts/claims", "claim-contract-v0.yaml", 4);
    assert_yaml_instances::<CompletionContractDocument>(
        "contracts/completion",
        "completion-contract-v0.yaml",
        2,
    );
    assert_yaml_instances::<GateContractDocument>("contracts/gates", "gate-contract-v0.yaml", 7);
}

#[test]
fn deserializes_request_and_effect_instances() {
    assert_yaml_instances::<RequestContractDocument>(
        "contracts/requests",
        "request-contract-v0.yaml",
        5,
    );
    assert_yaml_instances::<ToolEffectContractDocument>(
        "contracts/effects",
        "tool-effect-contract-v0.yaml",
        10,
    );
}

#[test]
fn deserializes_decision_close_instances() {
    assert_yaml_instances::<DecisionCloseContractDocument>(
        "contracts/decisions",
        "decision-close-contract-v0.yaml",
        2,
    );
}

#[test]
fn deserializes_runtime_boundary_instances() {
    assert_named_yaml::<RuntimeHandoffContractDocument>(
        "contracts/runtimes/cursor-browser-validation-runtime.yaml",
    );
    assert_named_yaml::<RuntimeHandoffContractDocument>(
        "contracts/runtimes/cursor-browser-validation-missing-capability.yaml",
    );
    assert_named_yaml::<RuntimeRegistryEntryDocument>(
        "contracts/runtimes/registry-cursor-browser-agent.yaml",
    );
    assert_named_yaml::<RuntimeCapabilityDocument>(
        "contracts/runtimes/capability-browser-validation.yaml",
    );
}

#[test]
fn deserializes_health_recovery_instances() {
    assert_yaml_instances::<HealthRecoveryContractDocument>(
        "contracts/recovery",
        "health-recovery-contract-v0.yaml",
        2,
    );
}

#[test]
fn deserializes_coordination_eval_instances() {
    assert_yaml_instances::<CoordinationEvalContractDocument>(
        "contracts/evals",
        "coordination-eval-contract-v0.yaml",
        1,
    );
}

#[test]
fn deserializes_assurance_case_instances() {
    assert_yaml_instances::<AssuranceCaseDocument>(
        "contracts/assurance",
        "assurance-case-contract-v0.yaml",
        3,
    );
}

fn assert_named_yaml<T: serde::de::DeserializeOwned>(relative_path: &str) {
    let path = repo_root().join(relative_path);
    let text = fs::read_to_string(&path).expect("read named contract instance");
    let _: T = yaml_serde::from_str(&text)
        .unwrap_or_else(|err| panic!("deserialize {}: {err}", path.display()));
}

fn assert_yaml_instances<T: serde::de::DeserializeOwned>(
    relative_dir: &str,
    definition_file: &str,
    expected_count: usize,
) {
    let dir = repo_root().join(relative_dir);
    let mut count = 0usize;

    for entry in fs::read_dir(&dir).expect("read contract instance dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("yaml")
            || path.file_name().and_then(|value| value.to_str()) == Some(definition_file)
        {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read contract instance");
        let _: T = yaml_serde::from_str(&text)
            .unwrap_or_else(|err| panic!("deserialize {}: {err}", path.display()));
        count += 1;
    }

    assert_eq!(count, expected_count);
}
