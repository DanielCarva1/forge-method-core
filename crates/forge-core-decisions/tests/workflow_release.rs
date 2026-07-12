use forge_core_contracts::{
    RepoPath, StableId, WorkflowCompatibilityLifecycle, WorkflowGovernanceReleaseManifestDocument,
    WorkflowMigrationBatchDocument, WorkflowMigrationPlanDocument, WorkflowQuarantine,
    WorkflowQuarantineReasonCode, WorkflowQuarantineRiskTier, WorkflowReleaseDispositionIntent,
    WorkflowReleaseWorkflowEntry, WorkflowRetirementAuthorizationReference,
};
use forge_core_decisions::{
    evaluate_workflow_migration, evaluate_workflow_release, load_catalog, load_workflow_documents,
    LoadedWorkflowDocument, WorkflowMigrationAudit, WorkflowReleaseDerivedState,
    WorkflowReleaseEvaluationAuthority, WorkflowReleaseEvaluationStatus,
    WorkflowReleaseEvidenceAssurance, WorkflowReleaseIssueCode,
};
use std::path::{Path, PathBuf};

const NON_BATCH_EMBEDDED_REF: &str = "contracts/spec/workflow-migration-foundation-v0.yaml";

#[derive(Clone)]
struct Fixture {
    manifest: WorkflowGovernanceReleaseManifestDocument,
    batches: Vec<WorkflowMigrationBatchDocument>,
    audit: WorkflowMigrationAudit,
    workflows: Vec<LoadedWorkflowDocument>,
}

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture() -> Fixture {
    let root = repo_root();
    let catalog_dir = root.join("contracts/workflows");
    let loaded = load_workflow_documents(&catalog_dir);
    assert!(loaded.is_clean(), "catalog errors: {:?}", loaded.errors);
    let workflows = loaded.workflows;
    let catalog = load_catalog(&catalog_dir);
    assert!(catalog.is_clean(), "catalog errors: {:?}", catalog.errors);
    let plan: WorkflowMigrationPlanDocument = yaml_serde::from_str(include_str!(
        "../../../contracts/policies/workflow-migration-foundation-v0.yaml"
    ))
    .expect("P5a plan");
    let audit = evaluate_workflow_migration(&plan, &workflows, &catalog.catalog);
    assert!(audit.issues.is_empty(), "P5a audit: {:?}", audit.issues);

    let canonical_manifest: WorkflowGovernanceReleaseManifestDocument = yaml_serde::from_str(
        include_str!("../../../contracts/migration/workflow-governance-release-foundation-v0.yaml"),
    )
    .expect("canonical release manifest");
    let canonical_batch: WorkflowMigrationBatchDocument = yaml_serde::from_str(include_str!(
        "../../../contracts/migration/workflow-governance-batch-golden-path-v0.yaml"
    ))
    .expect("canonical migration batch");
    Fixture {
        manifest: canonical_manifest,
        batches: vec![canonical_batch],
        audit,
        workflows,
    }
}

fn entry_mut<'a>(
    fixture: &'a mut Fixture,
    workflow_id: &str,
) -> &'a mut WorkflowReleaseWorkflowEntry {
    fixture
        .manifest
        .workflow_governance_release_manifest
        .workflow_entries
        .iter_mut()
        .find(|entry| entry.workflow_id.0 == workflow_id)
        .expect("known manifest entry")
}

fn batch_mut<'a>(
    fixture: &'a mut Fixture,
    batch_id: &str,
) -> &'a mut WorkflowMigrationBatchDocument {
    fixture
        .batches
        .iter_mut()
        .find(|batch| batch.workflow_migration_batch.id.0 == batch_id)
        .expect("known batch")
}

fn evaluate(fixture: &Fixture) -> forge_core_decisions::WorkflowReleaseEvaluation {
    evaluate_workflow_release(
        &fixture.manifest,
        &fixture.batches,
        &fixture.audit,
        &fixture.workflows,
    )
}

#[test]
fn input_order_does_not_change_evaluation_bytes() {
    let first = fixture();
    let mut reordered = first.clone();
    reordered.batches.reverse();
    reordered.workflows.reverse();

    let left = serde_json::to_vec(&evaluate(&first)).expect("serialize first");
    let right = serde_json::to_vec(&evaluate(&reordered)).expect("serialize reordered");
    assert_eq!(left, right);
    assert_eq!(
        evaluate(&first).status,
        WorkflowReleaseEvaluationStatus::StructurallyValid
    );
}

#[test]
fn missing_and_duplicate_manifest_entries_fail_closed() {
    let baseline = fixture();
    for duplicate in [false, true] {
        let mut case = baseline.clone();
        if duplicate {
            let entry = case
                .manifest
                .workflow_governance_release_manifest
                .workflow_entries[0]
                .clone();
            case.manifest
                .workflow_governance_release_manifest
                .workflow_entries
                .push(entry);
        } else {
            case.manifest
                .workflow_governance_release_manifest
                .workflow_entries
                .pop();
        }
        let result = evaluate(&case);
        assert_eq!(result.status, WorkflowReleaseEvaluationStatus::Blocked);
        assert!(result.issues.iter().any(|issue| {
            matches!(
                issue.code,
                WorkflowReleaseIssueCode::MissingManifestEntry
                    | WorkflowReleaseIssueCode::DuplicateManifestEntry
            )
        }));
    }
}

#[test]
fn bad_batch_digest_and_tampered_embedded_evidence_fail_closed() {
    let mut bad_batch = fixture();
    bad_batch
        .manifest
        .workflow_governance_release_manifest
        .batches[0]
        .expected_digest = format!("sha256:{}", "0".repeat(64));
    let result = evaluate(&bad_batch);
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowReleaseIssueCode::BatchDigestMismatch));

    let mut bad_evidence = fixture();
    batch_mut(&mut bad_evidence, "workflow-batch.golden-path-v0")
        .workflow_migration_batch
        .evidence
        .shadow_reports[0]
        .expected_digest = format!("sha256:{}", "1".repeat(64));
    let result = evaluate(&bad_evidence);
    assert_eq!(result.status, WorkflowReleaseEvaluationStatus::Blocked);
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowReleaseIssueCode::EvidenceDigestMismatch));
}

#[test]
fn missing_invalid_and_mismatched_embedded_batches_fail_closed() {
    let mut missing = fixture();
    missing
        .manifest
        .workflow_governance_release_manifest
        .batches[0]
        .embedded_ref = RepoPath("contracts/migration/does-not-exist.yaml".to_owned());
    let result = evaluate(&missing);
    assert!(result
        .issues
        .iter()
        .any(|issue| { issue.code == WorkflowReleaseIssueCode::EmbeddedBatchReferenceMissing }));

    let mut invalid = fixture();
    invalid
        .manifest
        .workflow_governance_release_manifest
        .batches[0]
        .embedded_ref = RepoPath(NON_BATCH_EMBEDDED_REF.to_owned());
    let result = evaluate(&invalid);
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowReleaseIssueCode::EmbeddedBatchParseFailed));

    let mut mismatched = fixture();
    batch_mut(&mut mismatched, "workflow-batch.golden-path-v0")
        .workflow_migration_batch
        .policies[0]
        .advisory_playbook
        .steps
        .push("caller-authored tamper".to_owned());
    let result = evaluate(&mismatched);
    assert!(result
        .issues
        .iter()
        .any(|issue| { issue.code == WorkflowReleaseIssueCode::EmbeddedBatchDocumentMismatch }));
}

#[test]
fn global_duplicate_policy_priority_and_workflow_are_rejected() {
    for mutation in ["policy", "priority", "workflow"] {
        let mut case = fixture();
        let batch = batch_mut(&mut case, "workflow-batch.golden-path-v0");
        let first_policy = batch.workflow_migration_batch.policies[0].clone();
        let second = &mut batch.workflow_migration_batch.policies[1];
        match mutation {
            "policy" => {
                second.id = first_policy.id.clone();
            }
            "priority" => {
                second.routing.priority = first_policy.routing.priority;
            }
            "workflow" => {
                second.compatibility_workflow_id = first_policy.compatibility_workflow_id;
            }
            _ => unreachable!(),
        }
        let result = evaluate(&case);
        assert_eq!(result.status, WorkflowReleaseEvaluationStatus::Blocked);
        assert!(result.issues.iter().any(|issue| {
            issue.code == WorkflowReleaseIssueCode::GlobalPolicyCompositionInvalid
        }));
    }
}

#[test]
fn p5a_domain_candidate_cannot_leak_into_core_batch_intent() {
    let mut case = fixture();
    entry_mut(&mut case, "game-brief").disposition_intent =
        WorkflowReleaseDispositionIntent::MigrationCandidate {
            batch_id: id("workflow-batch.golden-path-v0"),
            policy_ref: id("policy.workflow.context-recovery"),
        };
    let result = evaluate(&case);
    assert_eq!(result.status, WorkflowReleaseEvaluationStatus::Blocked);
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowReleaseIssueCode::DomainCoreLeak));
}

#[test]
fn label_and_count_theater_cannot_upgrade_an_invalid_candidate() {
    let baseline = evaluate(&fixture());
    assert_eq!(baseline.counts.migration_candidate_structurally_valid, 15);
    let mut theater = fixture();
    entry_mut(&mut theater, "adversarial-review").disposition_intent =
        WorkflowReleaseDispositionIntent::MigrationCandidate {
            batch_id: id("batch.nonexistent"),
            policy_ref: id("policy.workflow.adversarial-review"),
        };
    let result = evaluate(&theater);
    assert_eq!(result.status, WorkflowReleaseEvaluationStatus::Blocked);
    assert_eq!(result.counts.migration_candidate_structurally_valid, 0);
    assert!(result.counts.compatibility_only > baseline.counts.compatibility_only);

    let mut audit_theater = fixture();
    audit_theater.audit.golden_path_count += 1;
    let result = evaluate(&audit_theater);
    assert_eq!(result.status, WorkflowReleaseEvaluationStatus::Blocked);
    assert_eq!(result.counts.migration_candidate_structurally_valid, 0);
    assert!(result.issues.iter().any(|issue| {
        issue.path == "workflow_migration_audit.counts"
            && issue.code == WorkflowReleaseIssueCode::CatalogMismatch
    }));
}

#[test]
fn empty_evidence_prevents_structural_validation() {
    let mut case = fixture();
    batch_mut(&mut case, "workflow-batch.golden-path-v0")
        .workflow_migration_batch
        .evidence
        .adversarial_fixtures
        .clear();
    let result = evaluate(&case);
    assert_eq!(result.status, WorkflowReleaseEvaluationStatus::Blocked);
    assert_eq!(result.counts.migration_candidate_structurally_valid, 0);
}

#[test]
fn quarantine_requires_explicit_blockers_consumers_owner_and_due_release() {
    let mut case = fixture();
    entry_mut(&mut case, "adversarial-review").disposition_intent =
        WorkflowReleaseDispositionIntent::Quarantined {
            quarantine: WorkflowQuarantine {
                reason_code: WorkflowQuarantineReasonCode::AmbiguousLegacyAuthority,
                risk_tier: WorkflowQuarantineRiskTier::High,
                explanation: "routing overlaps another review".to_owned(),
                blocking_refs: Vec::new(),
                affected_consumer_refs: Vec::new(),
                review_owner: id(" "),
                review_due_release_version: "next".to_owned(),
            },
        };
    let result = evaluate(&case);
    assert_eq!(result.status, WorkflowReleaseEvaluationStatus::Blocked);
    assert!(result.issues.iter().any(|issue| {
        issue.path.contains("blocking_refs")
            && issue.code == WorkflowReleaseIssueCode::InvalidIdentifier
    }));
    assert!(result.issues.iter().any(|issue| {
        issue.path.contains("affected_consumer_refs")
            && issue.code == WorkflowReleaseIssueCode::InvalidIdentifier
    }));
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowReleaseIssueCode::InvalidSemver));
}

#[test]
fn compatibility_surface_and_batch_order_cannot_shrink_or_skip() {
    let mut case = fixture();
    case.manifest
        .workflow_governance_release_manifest
        .compatibility_policy
        .exact_fields
        .pop();
    case.manifest
        .workflow_governance_release_manifest
        .compatibility_policy
        .lifecycle = WorkflowCompatibilityLifecycle::Deprecated {
        announced_at_unix: 20,
        removal_not_before_unix: 10,
    };
    case.manifest.workflow_governance_release_manifest.batches[0].deterministic_order = 1;
    let result = evaluate(&case);
    assert_eq!(result.status, WorkflowReleaseEvaluationStatus::Blocked);
    assert!(result.issues.iter().any(|issue| {
        issue.path.contains("exact_fields")
            && issue.code == WorkflowReleaseIssueCode::CatalogMismatch
    }));
    assert!(result.issues.iter().any(|issue| {
        issue.path.contains("deterministic_order")
            && issue.code == WorkflowReleaseIssueCode::DuplicateBatchReference
    }));
    assert!(result.issues.iter().any(|issue| {
        issue.path.contains("lifecycle") && issue.code == WorkflowReleaseIssueCode::CatalogMismatch
    }));
}

#[test]
fn raw_batches_and_retirement_intent_never_derive_final_authority() {
    let baseline = evaluate(&fixture());
    assert_eq!(
        baseline.authority,
        WorkflowReleaseEvaluationAuthority::CandidateOnly
    );
    assert_eq!(
        baseline.evidence_assurance,
        WorkflowReleaseEvidenceAssurance::ContentIntegrityOnly
    );
    assert_eq!(baseline.counts.migration_candidate_structurally_valid, 15);

    let mut retirement = fixture();
    entry_mut(&mut retirement, "adversarial-review").disposition_intent =
        WorkflowReleaseDispositionIntent::RetirementCandidate {
            replacement_policy_ref: id("policy.workflow.adversarial-review"),
            authorization: WorkflowRetirementAuthorizationReference {
                authorization_id: id("retirement.adversarial-review"),
                embedded_ref: RepoPath(
                    "contracts/workflow-governance/retirement/adversarial-review.yaml".to_owned(),
                ),
                expected_digest: format!("sha256:{}", "a".repeat(64)),
            },
        };
    let result = evaluate(&retirement);
    assert_eq!(result.status, WorkflowReleaseEvaluationStatus::Blocked);
    assert!(result.assessments.iter().any(|assessment| {
        assessment.workflow_id.0 == "adversarial-review"
            && assessment.state == WorkflowReleaseDerivedState::RetirementPendingVerification
    }));
    let serialized = serde_json::to_string(&result).expect("serialize retirement result");
    assert!(!serialized.contains("\"state\":\"retired\""));
    assert!(!serialized.contains("\"state\":\"executable\""));

    let mut lifecycle = fixture();
    lifecycle
        .manifest
        .workflow_governance_release_manifest
        .compatibility_policy
        .lifecycle = WorkflowCompatibilityLifecycle::Retired {
        authorization_ref: WorkflowRetirementAuthorizationReference {
            authorization_id: id("retirement.compatibility-surface"),
            embedded_ref: RepoPath(
                "contracts/workflow-governance/retirement/compatibility.yaml".to_owned(),
            ),
            expected_digest: format!("sha256:{}", "b".repeat(64)),
        },
    };
    let result = evaluate(&lifecycle);
    assert_eq!(result.status, WorkflowReleaseEvaluationStatus::Blocked);
    assert!(result.issues.iter().any(|issue| {
        issue.code == WorkflowReleaseIssueCode::RetirementVerificationUnavailable
    }));
}
