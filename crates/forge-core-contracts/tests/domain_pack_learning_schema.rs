use forge_core_contracts::{
    domain_pack_learning_conflict_digest, DomainPackLearningConflictDocument,
    DomainPackLearningContractIssueCode, DomainPackLocalLearningCandidateDocument,
    DomainPackPromotionAuthorization, DomainPackPromotionAuthorizationAuthority,
    DomainPackPromotionAuthorizationDocument, DomainPackPromotionAuthorizationPayload,
    DomainPackPromotionSignature, DomainPackPromotionSignatureAlgorithm, DomainPackPromotionStage,
    DomainPackPromotionTransition, DomainPackReviewedRegistryDocument,
    DomainPackReviewerRegistryDocument, DomainPackReviewerRole, PrincipalId, StableId,
    DOMAIN_PACK_LEARNING_SCHEMA_VERSION,
};
use serde_json::json;

const ROOT: &str = "../../docs/fixtures/domain-pack-learning-v0";
const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn fixture(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(ROOT);
    std::fs::read_to_string(root.join(path)).expect("fixture is readable")
}

#[test]
fn representative_v03_documents_are_closed_and_semantically_valid() {
    let candidate: DomainPackLocalLearningCandidateDocument =
        yaml_serde::from_str(&fixture("valid/local-learning-candidate.yaml"))
            .expect("candidate fixture");
    let reviewers: DomainPackReviewerRegistryDocument =
        yaml_serde::from_str(&fixture("valid/reviewer-registry.yaml"))
            .expect("reviewer registry fixture");
    let registry: DomainPackReviewedRegistryDocument =
        yaml_serde::from_str(&fixture("valid/reviewed-registry.yaml"))
            .expect("reviewed registry fixture");

    assert_eq!(
        candidate.schema_version,
        DOMAIN_PACK_LEARNING_SCHEMA_VERSION
    );
    assert_eq!(
        reviewers.schema_version,
        DOMAIN_PACK_LEARNING_SCHEMA_VERSION
    );
    assert_eq!(registry.schema_version, DOMAIN_PACK_LEARNING_SCHEMA_VERSION);
    assert!(
        candidate.validate().is_empty(),
        "{:?}",
        candidate.validate()
    );
    assert!(
        reviewers.validate().is_empty(),
        "{:?}",
        reviewers.validate()
    );
    assert!(registry.validate().is_empty(), "{:?}", registry.validate());
}

#[test]
fn invented_authority_and_unknown_fields_fail_deserialization() {
    assert!(
        yaml_serde::from_str::<DomainPackLocalLearningCandidateDocument>(&fixture(
            "adversarial/candidate-authority.invalid.yaml"
        ))
        .is_err()
    );
    assert!(
        yaml_serde::from_str::<DomainPackLocalLearningCandidateDocument>(&fixture(
            "adversarial/candidate-unknown.invalid.yaml"
        ))
        .is_err()
    );
}

#[test]
fn p6b_join_digests_are_prefixed_while_p6c_internal_digests_remain_bare() {
    let registry: DomainPackReviewedRegistryDocument =
        yaml_serde::from_str(&fixture("valid/reviewed-registry.yaml"))
            .expect("reviewed registry fixture");
    assert!(registry.validate().is_empty(), "{:?}", registry.validate());

    let mut bare_package = registry.clone();
    bare_package.domain_pack_reviewed_registry.entries[0].package_digest = DIGEST.to_owned();
    assert!(bare_package.validate().iter().any(|issue| {
        issue.code == DomainPackLearningContractIssueCode::InvalidDigest
            && issue.path.ends_with("package_digest")
    }));

    let mut prefixed_internal = registry;
    prefixed_internal
        .domain_pack_reviewed_registry
        .registry_digest = format!("sha256:{DIGEST}");
    assert!(prefixed_internal.validate().iter().any(|issue| {
        issue.code == DomainPackLearningContractIssueCode::InvalidDigest
            && issue.path == "registry.registry_digest"
    }));
}

#[test]
fn revoked_content_cannot_remain_eligible_for_new_resolution() {
    let registry: DomainPackReviewedRegistryDocument =
        yaml_serde::from_str(&fixture("adversarial/revoked-eligible.invalid.yaml"))
            .expect("structurally valid adversarial registry");
    let issues = registry.validate();
    assert!(issues.iter().any(|issue| {
        issue.code == DomainPackLearningContractIssueCode::InvalidRegistryEligibility
    }));
    assert!(issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningContractIssueCode::InvalidRevocation));
}

#[test]
fn promotion_graph_rejects_skips_backtracking_and_terminal_resurrection() {
    let allowed = [
        (
            DomainPackPromotionStage::Candidate,
            DomainPackPromotionStage::Trial,
        ),
        (
            DomainPackPromotionStage::Trial,
            DomainPackPromotionStage::Validated,
        ),
        (
            DomainPackPromotionStage::Validated,
            DomainPackPromotionStage::Reviewed,
        ),
        (
            DomainPackPromotionStage::Reviewed,
            DomainPackPromotionStage::Deprecated,
        ),
        (
            DomainPackPromotionStage::Reviewed,
            DomainPackPromotionStage::Revoked,
        ),
        (
            DomainPackPromotionStage::Reviewed,
            DomainPackPromotionStage::Superseded,
        ),
    ];
    for (from, to) in allowed {
        assert!(DomainPackPromotionTransition { from, to }.is_allowed());
    }

    for transition in [
        DomainPackPromotionTransition {
            from: DomainPackPromotionStage::Candidate,
            to: DomainPackPromotionStage::Reviewed,
        },
        DomainPackPromotionTransition {
            from: DomainPackPromotionStage::Reviewed,
            to: DomainPackPromotionStage::Validated,
        },
        DomainPackPromotionTransition {
            from: DomainPackPromotionStage::Revoked,
            to: DomainPackPromotionStage::Reviewed,
        },
        DomainPackPromotionTransition {
            from: DomainPackPromotionStage::Superseded,
            to: DomainPackPromotionStage::Reviewed,
        },
    ] {
        assert!(!transition.is_allowed());
    }
}

#[test]
fn authorization_requires_distinct_reviewers_and_roles() {
    let signature = DomainPackPromotionSignature {
        reviewer_id: PrincipalId("reviewer.same".to_owned()),
        credential_id: StableId("credential.same".to_owned()),
        role: DomainPackReviewerRole::DomainExpert,
        algorithm: DomainPackPromotionSignatureAlgorithm::Ed25519,
        payload_digest: DIGEST.to_owned(),
        signature: "signature".to_owned(),
        signed_at_unix: 11,
    };
    let document = DomainPackPromotionAuthorizationDocument {
        schema_version: DOMAIN_PACK_LEARNING_SCHEMA_VERSION.to_owned(),
        domain_pack_promotion_authorization: DomainPackPromotionAuthorization {
            authority: DomainPackPromotionAuthorizationAuthority::CandidateAuthorization,
            payload: DomainPackPromotionAuthorizationPayload {
                authorization_id: StableId("authorization.1".to_owned()),
                dossier_digest: DIGEST.to_owned(),
                decision_digest: DIGEST.to_owned(),
                independent_review_digests: vec![DIGEST.to_owned()],
                reviewer_registry_digest: DIGEST.to_owned(),
                current_reviewed_registry_digest: DIGEST.to_owned(),
                proposed_reviewed_registry_digest: DIGEST.to_owned(),
                transition: DomainPackPromotionTransition {
                    from: DomainPackPromotionStage::Validated,
                    to: DomainPackPromotionStage::Reviewed,
                },
                audience: "forge.domain-pack-promotion".to_owned(),
                domain: "forge.domain-pack.learning.v0".to_owned(),
                nonce: "nonce-1".to_owned(),
                issued_at_unix: 10,
                expires_at_unix: 20,
            },
            signatures: vec![signature.clone(), signature],
        },
    };

    let issues = document.validate();
    assert!(issues
        .iter()
        .any(|issue| issue.code == DomainPackLearningContractIssueCode::DuplicateRecord));
    assert!(issues.iter().any(|issue| {
        issue.code == DomainPackLearningContractIssueCode::MissingIndependentReview
    }));
}

#[test]
fn canonical_conflict_digest_rejects_status_mutation_under_stable_identity() {
    let mut conflict: DomainPackLearningConflictDocument = serde_json::from_value(json!({
        "schema_version":"0.3", "domain_pack_learning_conflict": {
            "conflict_id":"conflict.exact", "authority":"conflict_evidence_only",
            "target":{"pack":{"publisher":"publisher.acme","name":"safety"},"base_version":"1.0.0","contribution_ref":null,"proposed_namespace":"guidance.safety"},
            "kind":"contradictory_observation", "subject_digests":[DIGEST],
            "evidence_refs":[], "status":"open", "review_request_digest":DIGEST,
            "resolution":null, "conflict_digest":DIGEST
        }
    }))
    .unwrap();
    conflict.domain_pack_learning_conflict.conflict_digest =
        domain_pack_learning_conflict_digest(&conflict).unwrap();
    assert!(conflict.validate().is_empty(), "{:?}", conflict.validate());

    conflict.domain_pack_learning_conflict.status =
        forge_core_contracts::DomainPackLearningConflictStatus::Resolved;
    assert!(conflict.validate().iter().any(|issue| {
        issue.code == DomainPackLearningContractIssueCode::CrossReferenceMismatch
            && issue.path == "conflict.conflict_digest"
    }));
}
