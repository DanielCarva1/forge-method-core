use forge_core_contracts::{
    WorkflowConsumerCompatibilityMatrixDocument, WorkflowConsumerCompatibilityReportDocument,
    WorkflowDeletionProofDocument, WorkflowFinalScorecardDocument,
    WorkflowRetirementAuthorizationV2Document, WorkflowRetirementEvidenceIndexDocument,
    WorkflowRetirementTombstoneCatalogDocument,
    WORKFLOW_RETIREMENT_AUTHORIZATION_V2_SCHEMA_VERSION,
};
use schemars::schema_for;

fn parse<T: serde::de::DeserializeOwned>(relative: &str) -> T {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw = std::fs::read_to_string(root.join(relative)).expect("generated retirement artifact");
    yaml_serde::from_str(&raw).expect("typed retirement artifact")
}

#[test]
fn generated_retirement_checkpoint_has_closed_two_axis_shape() {
    let index: WorkflowRetirementEvidenceIndexDocument =
        parse("contracts/migration/workflow-retirement-evidence-index-v0.yaml");
    let deletion: WorkflowDeletionProofDocument =
        parse("contracts/evidence/workflow-retirement-deletion-proof-v0.yaml");
    let consumer: WorkflowConsumerCompatibilityReportDocument =
        parse("contracts/evidence/workflow-retirement-consumer-window-v0.yaml");
    let matrix: WorkflowConsumerCompatibilityMatrixDocument =
        parse("contracts/evidence/workflow-retirement-consumer-matrix-v0.yaml");
    let tombstones: WorkflowRetirementTombstoneCatalogDocument =
        parse("contracts/migration/workflow-retirement-tombstones-v0.yaml");
    let scorecard: WorkflowFinalScorecardDocument =
        parse("contracts/migration/workflow-governance-final-scorecard-v0.yaml");
    let authorization: WorkflowRetirementAuthorizationV2Document =
        parse("contracts/migration/workflow-retirement-authorization-v0.yaml");

    assert_eq!(
        index.workflow_retirement_evidence_index.retirements.len(),
        42
    );
    assert_eq!(deletion.workflow_deletion_proof.workflows.len(), 42);
    assert_eq!(
        consumer
            .workflow_consumer_compatibility_report
            .workflows
            .len(),
        42
    );
    assert_eq!(
        matrix.workflow_consumer_compatibility_matrix.entries.len(),
        42
    );
    assert_eq!(
        tombstones
            .workflow_retirement_tombstone_catalog
            .tombstones
            .len(),
        42
    );
    let counts = scorecard
        .workflow_final_scorecard
        .runtime_disposition_counts;
    assert_eq!(
        (
            counts.executable,
            counts.compatibility_only,
            counts.quarantined,
            counts.domain_pack_candidate
        ),
        (42, 47, 3, 18)
    );
    let legacy = scorecard.workflow_final_scorecard.legacy_authority_counts;
    assert_eq!((legacy.retired, legacy.retained), (42, 68));
    assert_eq!(
        authorization.schema_version,
        WORKFLOW_RETIREMENT_AUTHORIZATION_V2_SCHEMA_VERSION
    );
    let signed = &authorization.workflow_retirement_authorization_v2;
    assert_eq!(signed.signatures.len(), 2);
    assert!(!signed.payload.release_manifest.raw_digest.is_empty());
    assert!(!signed.payload.runtime_bundle_artifact.raw_digest.is_empty());
    assert!(!signed.payload.snapshot_manifest.raw_digest.is_empty());
    assert!(!signed.payload.runtime_evidence.raw_digest.is_empty());
}

#[test]
fn retirement_documents_reject_unknown_authority_shortcuts() {
    let scorecard: WorkflowFinalScorecardDocument =
        parse("contracts/migration/workflow-governance-final-scorecard-v0.yaml");
    let mut value = serde_json::to_value(scorecard).expect("scorecard json");
    value["workflow_final_scorecard"]["caller_verified"] = serde_json::json!(true);
    assert!(serde_json::from_value::<WorkflowFinalScorecardDocument>(value).is_err());

    let authorization: WorkflowRetirementAuthorizationV2Document =
        parse("contracts/migration/workflow-retirement-authorization-v0.yaml");
    let mut value = serde_json::to_value(authorization).expect("authorization json");
    value["workflow_retirement_authorization_v2"]["payload"]["retired"] = serde_json::json!(true);
    assert!(serde_json::from_value::<WorkflowRetirementAuthorizationV2Document>(value).is_err());
}

#[test]
fn every_new_retirement_schema_is_closed_at_document_root() {
    for schema in [
        serde_json::to_value(schema_for!(WorkflowRetirementEvidenceIndexDocument)).unwrap(),
        serde_json::to_value(schema_for!(WorkflowDeletionProofDocument)).unwrap(),
        serde_json::to_value(schema_for!(WorkflowConsumerCompatibilityReportDocument)).unwrap(),
        serde_json::to_value(schema_for!(WorkflowConsumerCompatibilityMatrixDocument)).unwrap(),
        serde_json::to_value(schema_for!(WorkflowRetirementTombstoneCatalogDocument)).unwrap(),
        serde_json::to_value(schema_for!(WorkflowFinalScorecardDocument)).unwrap(),
        serde_json::to_value(schema_for!(WorkflowRetirementAuthorizationV2Document)).unwrap(),
    ] {
        assert_eq!(schema["additionalProperties"], serde_json::json!(false));
    }
}
