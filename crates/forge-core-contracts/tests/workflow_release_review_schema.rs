use forge_core_contracts::{
    PrincipalId, RepoPath, StableId, WorkflowGovernanceReleaseIdentity,
    WorkflowGovernedOutcomeDimension, WorkflowReleaseAdmissionAuthorization,
    WorkflowReleaseAdmissionAuthorizationAuthority, WorkflowReleaseAdmissionAuthorizationDocument,
    WorkflowReleaseAdmissionAuthorizationPayload, WorkflowReleaseAdmissionSignature,
    WorkflowReleaseAdmissionSignatureAlgorithm, WorkflowReleasePredecessorReference,
    WorkflowReleasePromotionBinding, WorkflowReleaseReviewArtifactBinding,
    WorkflowReleaseReviewDecision, WorkflowReleaseReviewDimensionDecision,
    WorkflowReleaseReviewIndex, WorkflowReleaseReviewIndexAuthority,
    WorkflowReleaseReviewIndexDocument, WorkflowReleaseReviewQuarantineDecision,
    WorkflowReleaseReviewWorkflowDecision, WorkflowReleaseReviewerCredential,
    WorkflowReleaseReviewerCredentialStatus, WorkflowReleaseReviewerRegistry,
    WorkflowReleaseReviewerRegistryAuthority, WorkflowReleaseReviewerRegistryDocument,
    WorkflowReleaseReviewerRole, WorkflowRuntimeBundleIdentity,
    WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_SCHEMA_VERSION,
    WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION,
    WORKFLOW_RELEASE_REVIEW_INDEX_SCHEMA_VERSION,
};
use schemars::schema_for;

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn artifact(value: &str, byte: char) -> WorkflowReleaseReviewArtifactBinding {
    WorkflowReleaseReviewArtifactBinding {
        artifact_id: id(value),
        embedded_ref: RepoPath(format!("contracts/{value}.yaml")),
        raw_digest: digest(byte),
        canonical_digest: digest(byte),
    }
}

fn promotion() -> WorkflowReleasePromotionBinding {
    WorkflowReleasePromotionBinding {
        predecessor: WorkflowReleasePredecessorReference {
            release_id: id("release.foundation"),
            release_digest: digest('1'),
        },
        candidate_release: WorkflowGovernanceReleaseIdentity {
            lineage_id: id("lineage.core"),
            release_id: id("release.core-assurance"),
            release_version: "0.3.0".to_owned(),
            release_digest: digest('2'),
        },
        candidate_runtime_bundle: WorkflowRuntimeBundleIdentity {
            bundle_id: id("bundle.core-assurance"),
            bundle_digest: digest('3'),
            policy_set_digest: digest('4'),
        },
        promoted_runtime_bundle: WorkflowRuntimeBundleIdentity {
            bundle_id: id("bundle.core-assurance.promoted"),
            bundle_digest: digest('5'),
            policy_set_digest: digest('4'),
        },
    }
}

fn workflow_decisions() -> Vec<WorkflowReleaseReviewWorkflowDecision> {
    (0..5)
        .map(|index| WorkflowReleaseReviewWorkflowDecision {
            workflow_id: id(&format!("workflow-{index}")),
            decision: WorkflowReleaseReviewDecision::Approved,
            rationale: "independently reviewed".to_owned(),
            finding_refs: Vec::new(),
        })
        .collect()
}

fn quarantine_decisions() -> Vec<WorkflowReleaseReviewQuarantineDecision> {
    (0..3)
        .map(|index| WorkflowReleaseReviewQuarantineDecision {
            workflow_id: id(&format!("quarantine-{index}")),
            decision: WorkflowReleaseReviewDecision::Approved,
            rationale: "quarantine remains closed".to_owned(),
            finding_refs: Vec::new(),
        })
        .collect()
}

fn dimension_decisions() -> Vec<WorkflowReleaseReviewDimensionDecision> {
    WorkflowGovernedOutcomeDimension::all()
        .into_iter()
        .map(|dimension| WorkflowReleaseReviewDimensionDecision {
            dimension,
            decision: WorkflowReleaseReviewDecision::Approved,
            rationale: "dimension reviewed".to_owned(),
            finding_refs: Vec::new(),
        })
        .collect()
}

fn review_index() -> WorkflowReleaseReviewIndexDocument {
    WorkflowReleaseReviewIndexDocument {
        schema_version: WORKFLOW_RELEASE_REVIEW_INDEX_SCHEMA_VERSION.to_owned(),
        workflow_release_review_index: WorkflowReleaseReviewIndex {
            id: id("review-index.core-assurance"),
            index_version: "0.1.0".to_owned(),
            authority: WorkflowReleaseReviewIndexAuthority::CandidateOnly,
            promotion: promotion(),
            release_manifest: artifact("manifest", '5'),
            migration_batches: vec![artifact("batch-golden", '6'), artifact("batch-core", '7')],
            review_subjects: vec![artifact("review-subject", '8')],
            coverage_policy: artifact("coverage", '9'),
            corpus_set: artifact("corpus-set", 'a'),
            representative_corpus: artifact("corpus-representative", 'b'),
            adversarial_corpus: artifact("corpus-adversarial", 'c'),
            shadow_report: artifact("shadow-report", 'd'),
            candidate_runtime_bundle: artifact("runtime-candidate", 'e'),
            promoted_runtime_bundle: artifact("runtime-promoted", 'f'),
            predecessor_registry: artifact("registry-predecessor", '1'),
            proposed_registry: artifact("registry-proposed", '2'),
            evaluator_source: artifact("evaluator-source", '3'),
            frozen_history: artifact("frozen-history", '4'),
            workflow_decisions: workflow_decisions(),
            quarantine_decisions: quarantine_decisions(),
            dimension_decisions: dimension_decisions(),
        },
    }
}

fn registry() -> WorkflowReleaseReviewerRegistryDocument {
    WorkflowReleaseReviewerRegistryDocument {
        schema_version: WORKFLOW_RELEASE_REVIEWER_REGISTRY_SCHEMA_VERSION.to_owned(),
        workflow_release_reviewer_registry: WorkflowReleaseReviewerRegistry {
            registry_id: id("reviewers.core"),
            registry_version: "0.1.0".to_owned(),
            authority: WorkflowReleaseReviewerRegistryAuthority::CandidateOnly,
            credentials: vec![WorkflowReleaseReviewerCredential {
                credential_id: id("credential.reviewer"),
                principal_id: PrincipalId("principal.reviewer".to_owned()),
                public_key_fingerprint: digest('5'),
                public_key_hex: "ab".repeat(32),
                algorithm: WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519,
                roles: vec![
                    WorkflowReleaseReviewerRole::SemanticReviewer,
                    WorkflowReleaseReviewerRole::ReleaseAuthorizer,
                ],
                status: WorkflowReleaseReviewerCredentialStatus::Active,
                valid_from_unix: 1_700_000_000,
                valid_until_unix: 1_800_000_000,
                independence_domain: "forge-release-review".to_owned(),
            }],
        },
    }
}

fn authorization() -> WorkflowReleaseAdmissionAuthorizationDocument {
    WorkflowReleaseAdmissionAuthorizationDocument {
        schema_version: WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_SCHEMA_VERSION.to_owned(),
        workflow_release_admission_authorization: WorkflowReleaseAdmissionAuthorization {
            authority: WorkflowReleaseAdmissionAuthorizationAuthority::CandidateAuthorization,
            payload: WorkflowReleaseAdmissionAuthorizationPayload {
                authorization_id: id("authorization.core-assurance"),
                review_index_id: id("review-index.core-assurance"),
                review_index_version: "0.1.0".to_owned(),
                review_index_raw_digest: digest('6'),
                review_index_canonical_digest: digest('7'),
                evaluation_digest: digest('8'),
                reviewer_registry_id: id("reviewers.core"),
                reviewer_registry_version: "0.1.0".to_owned(),
                reviewer_registry_raw_digest: digest('9'),
                reviewer_registry_canonical_digest: digest('a'),
                promotion: promotion(),
                invalidate_all_receipts: true,
                workflow_decisions: workflow_decisions(),
                quarantine_decisions: quarantine_decisions(),
                dimension_decisions: dimension_decisions(),
                audience: "forge-core-release-admission".to_owned(),
                domain: "forge-workflow-governance-v0".to_owned(),
                nonce: "nonce-unique".to_owned(),
                issued_at_unix: 1_700_000_000,
                expires_at_unix: 1_700_003_600,
            },
            signatures: vec![
                signature("semantic", WorkflowReleaseReviewerRole::SemanticReviewer),
                signature("authorizer", WorkflowReleaseReviewerRole::ReleaseAuthorizer),
            ],
        },
    }
}

fn signature(name: &str, role: WorkflowReleaseReviewerRole) -> WorkflowReleaseAdmissionSignature {
    WorkflowReleaseAdmissionSignature {
        principal_id: PrincipalId(format!("principal.{name}")),
        credential_id: id(&format!("credential.{name}")),
        role,
        algorithm: WorkflowReleaseAdmissionSignatureAlgorithm::Ed25519,
        payload_digest: digest('b'),
        signature: "ab".repeat(64),
        signed_at_unix: 1_700_000_100,
    }
}

#[test]
fn review_index_binds_the_complete_candidate_and_all_decisions() {
    let valid = review_index();
    assert!(valid.validate().is_empty());

    let mut incomplete = valid;
    incomplete
        .workflow_release_review_index
        .workflow_decisions
        .pop();
    incomplete
        .workflow_release_review_index
        .dimension_decisions
        .pop();
    incomplete
        .workflow_release_review_index
        .frozen_history
        .raw_digest = "bad".to_owned();
    assert!(incomplete.validate().len() >= 3);
}

#[test]
fn reviewer_registry_closes_key_role_status_and_validity_shape() {
    let valid = registry();
    assert!(valid.validate().is_empty());

    let mut invalid = valid;
    let credential = &mut invalid.workflow_release_reviewer_registry.credentials[0];
    credential.public_key_hex = "abcd".to_owned();
    credential
        .roles
        .push(WorkflowReleaseReviewerRole::SemanticReviewer);
    credential.valid_until_unix = credential.valid_from_unix;
    assert!(invalid.validate().len() >= 3);
}

#[test]
fn authorization_requires_closed_payload_and_separate_role_signatures() {
    let valid = authorization();
    assert!(valid.validate().is_empty());

    let mut invalid = valid;
    invalid
        .workflow_release_admission_authorization
        .payload
        .invalidate_all_receipts = false;
    invalid.workflow_release_admission_authorization.signatures[1].role =
        WorkflowReleaseReviewerRole::SemanticReviewer;
    assert!(invalid.validate().len() >= 2);
}

#[test]
fn documents_deny_unknown_fields_and_expose_only_candidate_authority() {
    let mut json = serde_json::to_value(review_index()).expect("serialize review index");
    json["workflow_release_review_index"]["invented_authority"] = serde_json::json!(true);
    assert!(serde_json::from_value::<WorkflowReleaseReviewIndexDocument>(json).is_err());

    let schema =
        serde_json::to_string(&schema_for!(WorkflowReleaseAdmissionAuthorizationAuthority))
            .expect("serialize authority schema");
    assert!(schema.contains("candidate_authorization"));
    assert!(!schema.contains("admitted"));
    assert!(!schema.contains("executable"));
}
