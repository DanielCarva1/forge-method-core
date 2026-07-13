use forge_core_contracts::{
    RepoPath, WorkflowRetirementArtifactBinding, WorkflowRetirementEvidenceIndexDocument,
};
use forge_core_decisions::{
    evaluate_workflow_retirement, WorkflowRetirementCandidateInput,
    WorkflowRetirementEvaluationAuthority, WorkflowRetirementEvaluationStatus,
    WorkflowRetirementIssueCode,
};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

const INDEX: &str = "contracts/migration/workflow-retirement-evidence-index-v0.yaml";

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read<T: DeserializeOwned>(path: &str) -> T {
    yaml_serde::from_slice(&std::fs::read(root().join(path)).expect("artifact bytes"))
        .expect("typed artifact")
}

fn digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical artifact");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn fixture() -> WorkflowRetirementCandidateInput {
    let index: WorkflowRetirementEvidenceIndexDocument = read(INDEX);
    let raw = std::fs::read(root().join(INDEX)).expect("index bytes");
    let binding = WorkflowRetirementArtifactBinding {
        artifact_id: index.workflow_retirement_evidence_index.id.clone(),
        embedded_ref: RepoPath(INDEX.to_owned()),
        raw_digest: format!("sha256:{:x}", Sha256::digest(&raw)),
        canonical_digest: digest(&index),
    };
    WorkflowRetirementCandidateInput {
        evidence_index: index,
        evidence_index_binding: binding,
        deletion_proof: read("contracts/evidence/workflow-retirement-deletion-proof-v0.yaml"),
        consumer_matrix: read("contracts/evidence/workflow-retirement-consumer-matrix-v0.yaml"),
        consumer_report: read("contracts/evidence/workflow-retirement-consumer-window-v0.yaml"),
        tombstone_catalog: read("contracts/migration/workflow-retirement-tombstones-v0.yaml"),
        release_manifest: read(
            "contracts/migration/workflow-governance-release-agent-native-continuity-candidate-v0.yaml",
        ),
        runtime_bundle: read(
            "contracts/workflow-governance/runtime-agent-native-continuity-v0.yaml",
        ),
    }
}

#[test]
fn derives_exact_two_axis_scorecard_without_authority() {
    let evaluation = evaluate_workflow_retirement(&fixture());
    assert_eq!(
        evaluation.status,
        WorkflowRetirementEvaluationStatus::ReadyForIndependentAuthorization
    );
    assert_eq!(
        evaluation.authority,
        WorkflowRetirementEvaluationAuthority::CandidateOnly
    );
    assert!(evaluation.issues.is_empty());
    assert_eq!(evaluation.retired_legacy_count, 42);
    let scorecard = evaluation.scorecard.workflow_final_scorecard;
    assert_eq!(scorecard.assessments.len(), 110);
    assert_eq!(
        (
            scorecard.runtime_disposition_counts.executable,
            scorecard.runtime_disposition_counts.compatibility_only,
            scorecard.runtime_disposition_counts.quarantined,
            scorecard.runtime_disposition_counts.domain_pack_candidate,
        ),
        (42, 47, 3, 18)
    );
    assert_eq!(
        (
            scorecard.legacy_authority_counts.retired,
            scorecard.legacy_authority_counts.retained,
        ),
        (42, 68)
    );
}

#[test]
fn authored_deletion_equality_cannot_hide_an_ablation_mismatch() {
    let mut input = fixture();
    let surface = &mut input.deletion_proof.workflow_deletion_proof.workflows[0].surfaces[0];
    surface.legacy_ablated_digest = format!("sha256:{}", "f".repeat(64));
    surface.equivalent = true;
    let evaluation = evaluate_workflow_retirement(&input);
    assert_eq!(
        evaluation.status,
        WorkflowRetirementEvaluationStatus::Blocked
    );
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowRetirementIssueCode::DeletionProofIncomplete));
}

#[test]
fn equal_authored_hashes_cannot_replace_policy_surface_recomputation() {
    let mut input = fixture();
    let surface = &mut input.deletion_proof.workflow_deletion_proof.workflows[0].surfaces[0];
    let invented = format!("sha256:{}", "e".repeat(64));
    surface.control_digest = invented.clone();
    surface.legacy_ablated_digest = invented;
    surface.equivalent = true;
    let evaluation = evaluate_workflow_retirement(&input);
    assert_eq!(
        evaluation.status,
        WorkflowRetirementEvaluationStatus::Blocked
    );
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowRetirementIssueCode::DeletionProofIncomplete));
}

#[test]
fn deletion_surface_cannot_be_transplanted_to_another_history() {
    let mut input = fixture();
    input
        .deletion_proof
        .workflow_deletion_proof
        .release_history
        .raw_digest = format!("sha256:{}", "d".repeat(64));
    let evaluation = evaluate_workflow_retirement(&input);
    assert_eq!(
        evaluation.status,
        WorkflowRetirementEvaluationStatus::Blocked
    );
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowRetirementIssueCode::DeletionMismatch));
}

#[test]
fn unsupported_consumer_blocks_retirement_candidate() {
    let mut input = fixture();
    input
        .consumer_report
        .workflow_consumer_compatibility_report
        .workflows[0]
        .unsupported_repository_consumer_count = 1;
    let evaluation = evaluate_workflow_retirement(&input);
    assert_eq!(
        evaluation.status,
        WorkflowRetirementEvaluationStatus::Blocked
    );
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowRetirementIssueCode::UnsupportedConsumerPresent));
}

#[test]
fn consumer_report_cannot_diverge_from_bound_repository_matrix() {
    let mut input = fixture();
    input
        .consumer_report
        .workflow_consumer_compatibility_report
        .workflows[0]
        .diagnostic_code =
        forge_core_contracts::StableId("workflow.retired.transplanted".to_owned());
    let evaluation = evaluate_workflow_retirement(&input);
    assert_eq!(
        evaluation.status,
        WorkflowRetirementEvaluationStatus::Blocked
    );
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowRetirementIssueCode::ConsumerDiagnosticMissing));
}

#[test]
fn missing_tombstone_cannot_shrink_exact_retirement_set() {
    let mut input = fixture();
    input
        .tombstone_catalog
        .workflow_retirement_tombstone_catalog
        .tombstones
        .pop();
    let evaluation = evaluate_workflow_retirement(&input);
    assert_eq!(
        evaluation.status,
        WorkflowRetirementEvaluationStatus::Blocked
    );
    assert!(evaluation
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowRetirementIssueCode::RetirementSetMismatch));
}
