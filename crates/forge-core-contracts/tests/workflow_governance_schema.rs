use forge_core_contracts::{
    WorkflowCompletionAssertion, WorkflowDecisionActivation, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceEvaluationDocument, WORKFLOW_GOVERNANCE_SCHEMA_VERSION,
};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(repo_root().join(relative)).expect("published P5b fixture")
}

#[test]
fn published_bundle_round_trips_as_a_closed_typed_contract() {
    let yaml = read("contracts/workflow-governance/kernel-v0.yaml");
    let document: WorkflowGovernanceBundleDocument =
        yaml_serde::from_str(&yaml).expect("typed workflow governance bundle");
    assert_eq!(document.schema_version, WORKFLOW_GOVERNANCE_SCHEMA_VERSION);
    assert_eq!(document.workflow_governance_bundle.policies.len(), 2);
    let build = document
        .workflow_governance_bundle
        .policies
        .iter()
        .find(|policy| policy.id.0 == "policy.workflow.build-story")
        .expect("build-story policy");
    assert_eq!(build.obligations.len(), 2);
    assert_eq!(build.claims.len(), 2);
    assert_eq!(build.evaluators.len(), 2);
    assert_eq!(build.capability_requirements.len(), 1);
    assert_eq!(build.decision_rules.len(), 1);
    assert_eq!(
        build.decision_rules[0].activation,
        WorkflowDecisionActivation::ObservedNeed
    );

    let serialized = yaml_serde::to_string(&document).expect("serialize governance bundle");
    let round_trip: WorkflowGovernanceBundleDocument =
        yaml_serde::from_str(&serialized).expect("round-trip governance bundle");
    assert_eq!(round_trip, document);
}

#[test]
fn published_evaluations_round_trip_as_closed_typed_contracts() {
    for name in [
        "complete",
        "active",
        "missing-evidence",
        "missing-capability",
        "human-decision",
        "contradictory-evidence",
        "invented-completion",
    ] {
        let yaml = read(&format!(
            "docs/fixtures/workflow-governance-kernel-v0/{name}.yaml"
        ));
        let document: WorkflowGovernanceEvaluationDocument =
            yaml_serde::from_str(&yaml).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(document.schema_version, WORKFLOW_GOVERNANCE_SCHEMA_VERSION);
        let serialized = yaml_serde::to_string(&document).expect("serialize evaluation");
        let round_trip: WorkflowGovernanceEvaluationDocument =
            yaml_serde::from_str(&serialized).expect("round-trip evaluation");
        assert_eq!(round_trip, document, "fixture {name}");
    }

    let invented: WorkflowGovernanceEvaluationDocument = yaml_serde::from_str(&read(
        "docs/fixtures/workflow-governance-kernel-v0/invented-completion.yaml",
    ))
    .expect("invented completion fixture");
    assert_eq!(
        invented.workflow_governance_evaluation.completion_assertion,
        WorkflowCompletionAssertion::Asserted
    );
}

#[test]
fn bundle_and_evaluation_reject_unknown_fields_and_enum_values() {
    let bundle = read("contracts/workflow-governance/kernel-v0.yaml");
    let unknown_bundle_field = bundle.replace(
        "  id: bundle.workflow-governance.kernel-v0",
        "  id: bundle.workflow-governance.kernel-v0\n  caller_can_override: true",
    );
    assert!(
        yaml_serde::from_str::<WorkflowGovernanceBundleDocument>(&unknown_bundle_field).is_err()
    );
    let unsafe_activation =
        bundle.replace("activation: observed_need", "activation: agent_decides");
    assert!(yaml_serde::from_str::<WorkflowGovernanceBundleDocument>(&unsafe_activation).is_err());

    let evaluation = read("docs/fixtures/workflow-governance-kernel-v0/complete.yaml");
    let unknown_evaluation_field = evaluation.replace(
        "  state_version: 7",
        "  state_version: 7\n  skip_governance: true",
    );
    assert!(
        yaml_serde::from_str::<WorkflowGovernanceEvaluationDocument>(&unknown_evaluation_field)
            .is_err()
    );
    let invented_outcome = evaluation.replace("outcome: pass", "outcome: assumed");
    assert!(
        yaml_serde::from_str::<WorkflowGovernanceEvaluationDocument>(&invented_outcome).is_err()
    );
}
