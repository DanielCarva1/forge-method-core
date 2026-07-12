use forge_core_contracts::{
    WorkflowGovernanceReleaseRegistryDocument, WorkflowReleaseAdmissionAuthorizationDocument,
    WorkflowReleaseReviewerRegistryDocument,
};
use forge_core_decisions::embedded_text;
use forge_core_kernel::{
    load_admitted_workflow_governance_release_registry,
    load_admitted_workflow_governance_reviewed_release_registry,
    REVIEWED_WORKFLOW_RELEASE_REGISTRY_REF, WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_REF,
    WORKFLOW_RELEASE_REVIEWER_REGISTRY_REF,
};

#[test]
fn reviewed_loader_admits_exact_append_only_three_release_registry() {
    let historical =
        load_admitted_workflow_governance_release_registry().expect("historical registry");
    let reviewed = load_admitted_workflow_governance_reviewed_release_registry()
        .expect("independently reviewed registry");

    assert_eq!(historical.release_count(), 2);
    assert_eq!(historical.latest_release().policy_count(), 15);
    assert_eq!(reviewed.release_count(), 3);
    assert_eq!(reviewed.latest_release().policy_count(), 20);
    assert_ne!(historical.registry_digest(), reviewed.registry_digest());
    assert_eq!(
        reviewed.latest_release().receipt_carryover(),
        forge_core_contracts::WorkflowReceiptCarryover::InvalidateAll
    );
}

#[test]
fn quarantined_workflows_are_not_runtime_policies() {
    let reviewed = load_admitted_workflow_governance_reviewed_release_registry()
        .expect("independently reviewed registry");
    for workflow_id in ["edge-case-review", "track-decision", "release-readiness"] {
        assert!(!reviewed
            .latest_release()
            .contains_workflow_policy(workflow_id));
    }
}

#[test]
fn raw_review_documents_remain_non_authoritative_inputs() {
    let registry: WorkflowGovernanceReleaseRegistryDocument = yaml_serde::from_str(
        embedded_text(REVIEWED_WORKFLOW_RELEASE_REGISTRY_REF).expect("expanded registry bytes"),
    )
    .expect("expanded registry document");
    let reviewers: WorkflowReleaseReviewerRegistryDocument = yaml_serde::from_str(
        embedded_text(WORKFLOW_RELEASE_REVIEWER_REGISTRY_REF).expect("reviewer registry bytes"),
    )
    .expect("reviewer registry document");
    let authorization: WorkflowReleaseAdmissionAuthorizationDocument = yaml_serde::from_str(
        embedded_text(WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_REF)
            .expect("authorization document bytes"),
    )
    .expect("authorization document");

    assert_eq!(
        registry.workflow_governance_release_registry.releases.len(),
        3
    );
    assert_eq!(
        reviewers.workflow_release_reviewer_registry.authority,
        forge_core_contracts::WorkflowReleaseReviewerRegistryAuthority::CandidateOnly
    );
    assert_eq!(
        authorization
            .workflow_release_admission_authorization
            .authority,
        forge_core_contracts::WorkflowReleaseAdmissionAuthorizationAuthority::CandidateAuthorization
    );
    // Parsing all three documents confers no capability: the only public
    // admission path reloads fixed embedded bytes, recomputes review, verifies
    // signatures, and consumes its opaque token internally.
    assert_eq!(
        load_admitted_workflow_governance_release_registry()
            .expect("historical registry")
            .release_count(),
        2
    );
}
