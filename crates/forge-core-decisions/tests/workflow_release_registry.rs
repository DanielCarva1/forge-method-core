use forge_core_contracts::{
    StableId, WorkflowGovernanceBundleDocument, WorkflowGovernanceReleaseManifestDocument,
    WorkflowGovernanceReleaseRegistryDocument, WorkflowMigrationBatchDocument,
    WorkflowMigrationPlanDocument,
};
use forge_core_decisions::{
    evaluate_workflow_migration, evaluate_workflow_release, evaluate_workflow_release_registry,
    load_catalog, load_workflow_documents, WorkflowReleaseRegistryEvaluationAuthority,
    WorkflowReleaseRegistryEvaluationStatus, WorkflowReleaseRegistryIssueCode,
};
use std::path::{Path, PathBuf};

#[derive(Clone)]
struct Fixture {
    registry: WorkflowGovernanceReleaseRegistryDocument,
    bundles: Vec<WorkflowGovernanceBundleDocument>,
}

fn fixture() -> Fixture {
    Fixture {
        registry: yaml_serde::from_str(include_str!(
            "../../../contracts/migration/workflow-governance-release-registry-v0.yaml"
        ))
        .expect("canonical registry"),
        bundles: vec![
            yaml_serde::from_str(include_str!(
                "../../../contracts/workflow-governance/golden-path-v0.yaml"
            ))
            .expect("P5c bundle"),
            yaml_serde::from_str(include_str!(
                "../../../contracts/workflow-governance/runtime-release-foundation-v0.yaml"
            ))
            .expect("foundation runtime bundle"),
        ],
    }
}

fn evaluate(fixture: &Fixture) -> forge_core_decisions::WorkflowReleaseRegistryEvaluation {
    evaluate_workflow_release_registry(&fixture.registry, &fixture.bundles)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn p5d1_evaluated_release_digest() -> String {
    let root = repo_root();
    let catalog_dir = root.join("contracts/evidence/workflow-retirement/legacy-catalog");
    let loaded = load_workflow_documents(&catalog_dir);
    assert!(loaded.is_clean(), "catalog errors: {:?}", loaded.errors);
    let catalog = load_catalog(&catalog_dir);
    assert!(catalog.is_clean(), "catalog errors: {:?}", catalog.errors);
    let plan: WorkflowMigrationPlanDocument = yaml_serde::from_str(include_str!(
        "../../../contracts/policies/workflow-migration-foundation-v0.yaml"
    ))
    .expect("P5a plan");
    let audit = evaluate_workflow_migration(&plan, &loaded.workflows, &catalog.catalog);
    assert!(audit.issues.is_empty(), "P5a audit: {:?}", audit.issues);
    let manifest: WorkflowGovernanceReleaseManifestDocument = yaml_serde::from_str(include_str!(
        "../../../contracts/migration/workflow-governance-release-foundation-v0.yaml"
    ))
    .expect("P5d.1 manifest");
    let batch: WorkflowMigrationBatchDocument = yaml_serde::from_str(include_str!(
        "../../../contracts/migration/workflow-governance-batch-golden-path-v0.yaml"
    ))
    .expect("P5d.1 batch");
    evaluate_workflow_release(&manifest, &[batch], &audit, &loaded.workflows).release_digest
}

#[test]
fn canonical_registry_proves_exact_fifteen_policy_successor_without_authority() {
    let result = evaluate(&fixture());
    assert_eq!(
        result.status,
        WorkflowReleaseRegistryEvaluationStatus::StructurallyValid
    );
    assert_eq!(
        result.authority,
        WorkflowReleaseRegistryEvaluationAuthority::NonAuthoritative
    );
    assert_eq!(result.successor_policy_count, 15);
    assert!(result.issues.is_empty());
    let genesis_bundle = result
        .genesis_runtime_bundle
        .as_ref()
        .expect("genesis bundle");
    let successor_bundle = result
        .default_successor_runtime_bundle
        .as_ref()
        .expect("successor bundle");
    assert_ne!(genesis_bundle.bundle_id, successor_bundle.bundle_id);
    assert_ne!(genesis_bundle.bundle_digest, successor_bundle.bundle_digest);
    assert_eq!(
        genesis_bundle.bundle_digest,
        "sha256:af2a5a012fd3843d5d3686dc4e45bb295e91f60f1615a3040b22b1b0ec5423bb"
    );
    assert_eq!(
        genesis_bundle.policy_set_digest,
        successor_bundle.policy_set_digest
    );
    assert_ne!(
        result.registry_digest,
        result
            .default_successor_release
            .expect("successor")
            .release_digest
    );
}

#[test]
fn input_order_does_not_change_registry_evaluation_bytes() {
    let first = fixture();
    let mut reversed = first.clone();
    reversed.bundles.reverse();
    let left = serde_json::to_vec(&evaluate(&first)).expect("serialize first");
    let right = serde_json::to_vec(&evaluate(&reversed)).expect("serialize reversed");
    assert_eq!(left, right);
}

#[test]
fn successor_release_identity_equals_p5d1_evaluated_release_digest() {
    let case = fixture();
    let successor = &case.registry.workflow_governance_release_registry.releases[1];
    assert_eq!(
        successor.release.release_digest,
        p5d1_evaluated_release_digest()
    );
    let forge_core_contracts::WorkflowReleaseRegistrySource::EmbeddedManifest {
        expected_digest,
        ..
    } = &successor.source
    else {
        panic!("successor must use embedded manifest source");
    };
    assert_ne!(successor.release.release_digest, *expected_digest);
}

#[test]
fn duplicate_release_and_runtime_identities_fail_closed() {
    for target in ["release", "bundle"] {
        let mut case = fixture();
        let first = case.registry.workflow_governance_release_registry.releases[0].clone();
        let second = &mut case.registry.workflow_governance_release_registry.releases[1];
        match target {
            "release" => {
                second.release.release_id = first.release.release_id;
                second.release.release_digest = first.release.release_digest;
            }
            "bundle" => {
                second.runtime_bundle.identity.bundle_id = first.runtime_bundle.identity.bundle_id;
                second.runtime_bundle.identity.bundle_digest =
                    first.runtime_bundle.identity.bundle_digest;
            }
            _ => unreachable!(),
        }
        let result = evaluate(&case);
        assert_eq!(
            result.status,
            WorkflowReleaseRegistryEvaluationStatus::Blocked
        );
        assert!(result.issues.iter().any(|issue| {
            matches!(
                issue.code,
                WorkflowReleaseRegistryIssueCode::DuplicateReleaseIdentity
                    | WorkflowReleaseRegistryIssueCode::DuplicateRuntimeBundleIdentity
            )
        }));
    }
}

#[test]
fn digest_and_embedded_reference_tampering_fail_closed() {
    let mut bad_digest = fixture();
    bad_digest
        .registry
        .workflow_governance_release_registry
        .releases[1]
        .runtime_bundle
        .expected_digest = format!("sha256:{}", "0".repeat(64));
    let result = evaluate(&bad_digest);
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowReleaseRegistryIssueCode::EmbeddedDigestMismatch));

    let mut missing_ref = fixture();
    missing_ref
        .registry
        .workflow_governance_release_registry
        .releases[1]
        .runtime_bundle
        .embedded_ref
        .0 = "contracts/workflow-governance/absent.yaml".to_owned();
    let result = evaluate(&missing_ref);
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowReleaseRegistryIssueCode::EmbeddedReferenceMissing));

    let mut release_identity_drift = fixture();
    release_identity_drift
        .registry
        .workflow_governance_release_registry
        .releases[1]
        .release
        .release_digest = format!("sha256:{}", "7".repeat(64));
    let result = evaluate(&release_identity_drift);
    assert!(result.issues.iter().any(|issue| {
        issue.code == WorkflowReleaseRegistryIssueCode::ReleaseManifestIdentityMismatch
    }));
}

#[test]
fn predecessor_and_default_successor_tampering_fail_closed() {
    let mut bad_predecessor = fixture();
    bad_predecessor
        .registry
        .workflow_governance_release_registry
        .releases[1]
        .predecessor
        .as_mut()
        .expect("predecessor")
        .release_digest = format!("sha256:{}", "4".repeat(64));
    let result = evaluate(&bad_predecessor);
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowReleaseRegistryIssueCode::PredecessorMismatch));

    let mut bad_default = fixture();
    bad_default
        .registry
        .workflow_governance_release_registry
        .default_successor_release_id = StableId("unknown.release".to_owned());
    let result = evaluate(&bad_default);
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowReleaseRegistryIssueCode::DefaultSuccessorMismatch));
}

#[test]
fn policy_set_digest_drift_and_candidate_elevation_fail_before_admission() {
    let mut digest_drift = fixture();
    digest_drift
        .registry
        .workflow_governance_release_registry
        .releases[1]
        .runtime_bundle
        .identity
        .policy_set_digest = format!("sha256:{}", "8".repeat(64));
    let result = evaluate(&digest_drift);
    assert_eq!(
        result.status,
        WorkflowReleaseRegistryEvaluationStatus::Blocked
    );
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowReleaseRegistryIssueCode::PolicySetDrift));

    let mut elevated = fixture();
    let invented = elevated.bundles[1].workflow_governance_bundle.policies[0].clone();
    elevated.bundles[1]
        .workflow_governance_bundle
        .policies
        .push(invented);
    let result = evaluate(&elevated);
    assert_eq!(
        result.status,
        WorkflowReleaseRegistryEvaluationStatus::Blocked
    );
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowReleaseRegistryIssueCode::SuppliedBundleMismatch));
}

#[test]
fn missing_supplied_runtime_bundle_fails_closed() {
    let mut case = fixture();
    case.bundles.pop();
    let result = evaluate(&case);
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowReleaseRegistryIssueCode::SuppliedBundleMissing));
}
