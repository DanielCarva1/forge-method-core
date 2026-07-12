use forge_core_contracts::{
    WorkflowGovernancePolicyOverlayDocument, WorkflowGovernanceSignal,
    WORKFLOW_GOVERNANCE_SCHEMA_VERSION,
};
use schemars::schema_for;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn core_assurance_overlay_is_typed_but_not_a_runtime_bundle() {
    let path = repo_root().join("contracts/policies/workflow-core-assurance-overlay-v0.yaml");
    let bytes = std::fs::read(&path).expect("published typed overlay");
    let document: WorkflowGovernancePolicyOverlayDocument =
        yaml_serde::from_slice(&bytes).expect("closed overlay document");

    assert_eq!(document.schema_version, WORKFLOW_GOVERNANCE_SCHEMA_VERSION);
    let overlay = document.workflow_governance_policy_overlay;
    assert_eq!(
        overlay.id.0,
        "overlay.workflow-governance.core-assurance-v0"
    );
    assert_eq!(
        overlay.base_bundle_id.0,
        "bundle.workflow-governance.golden-path-v0"
    );
    assert_eq!(overlay.policies.len(), 5);
    assert_eq!(
        overlay
            .policies
            .iter()
            .map(|policy| policy.compatibility_workflow_id.0.as_str())
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "adversarial-review",
            "code-review",
            "nfr-evidence-audit",
            "risk-register",
            "traceability-gate",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        overlay.policies[0].routing.signals,
        vec![WorkflowGovernanceSignal::AdversarialReviewRequested]
    );

    let wire = String::from_utf8(bytes).expect("utf-8 overlay");
    for forbidden in ["executable:", "admitted:", "retired:", "authority:"] {
        assert!(!wire.contains(forbidden), "overlay leaked {forbidden}");
    }
}

#[test]
fn overlay_schema_is_distinct_from_runtime_bundle_schema() {
    let schema = serde_json::to_string(&schema_for!(WorkflowGovernancePolicyOverlayDocument))
        .expect("overlay JSON schema");
    assert!(schema.contains("workflow_governance_policy_overlay"));
    assert!(schema.contains("base_bundle_id"));
    assert!(!schema.contains("workflow_governance_bundle"));
}
