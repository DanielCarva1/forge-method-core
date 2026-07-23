use forge_core_contracts::{
    DomainPackAuthorArtifactRefs, DomainPackAuthorIssue, DomainPackAuthorIssueCode,
    DomainPackAuthorProvenanceTemplate, DomainPackAuthorRawSidecars,
    DomainPackAuthorSkeletonRequest, DomainPackAuthorSkeletonRequestDocument,
    DomainPackCandidateAuthority, DomainPackCoreBinding, DomainPackProjectRequirements,
    DomainPackProjectRequirementsDocument, DomainPackSourceKind, DomainPackVersionReference,
    RepoPath, StableId, WorkflowGovernanceBundle, DOMAIN_PACK_AUTHORING_SCHEMA_VERSION,
    DOMAIN_PACK_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};

fn canonical_digest<T: serde::Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn sidecars() -> DomainPackAuthorRawSidecars {
    DomainPackAuthorRawSidecars {
        pack: DomainPackVersionReference {
            publisher: StableId("example".to_owned()),
            name: StableId("authoring".to_owned()),
            version: "0.1.0".to_owned(),
        },
        manifest_raw: b"manifest".to_vec(),
        content_raw: b"content".to_vec(),
        license_raw: b"license".to_vec(),
    }
}

fn skeleton_request() -> DomainPackAuthorSkeletonRequestDocument {
    let bundle = WorkflowGovernanceBundle {
        id: StableId("core.bundle".to_owned()),
        policies: Vec::new(),
    };
    DomainPackAuthorSkeletonRequestDocument {
        schema_version: DOMAIN_PACK_AUTHORING_SCHEMA_VERSION.to_owned(),
        domain_pack_author_skeleton_request: DomainPackAuthorSkeletonRequest {
            request_id: StableId("authoring.request".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            publisher: StableId("example".to_owned()),
            name: StableId("authoring".to_owned()),
            namespace: StableId("example.authoring".to_owned()),
            version: "0.1.0".to_owned(),
            forge_core_version: "1.85.0".to_owned(),
            core: DomainPackCoreBinding {
                bundle_id: bundle.id.clone(),
                bundle_digest: canonical_digest(&bundle),
                policy_set_digest: canonical_digest(&bundle.policies),
                bundle,
            },
            requirements: DomainPackProjectRequirementsDocument {
                schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
                domain_pack_project_requirements: DomainPackProjectRequirements {
                    project_id: StableId("example.project".to_owned()),
                    requirement_set_id: StableId("example.requirements".to_owned()),
                    required_domains: Vec::new(),
                },
            },
            provenance: DomainPackAuthorProvenanceTemplate {
                source_kind: DomainPackSourceKind::LocalCandidate,
                source_uri: "https://example.invalid/domain-pack".to_owned(),
                source_revision: "draft-0".to_owned(),
                source_digest: format!("sha256:{}", "0".repeat(64)),
                authors: vec![StableId("example.author".to_owned())],
                license_spdx_expression: "MIT".to_owned(),
            },
            artifact_refs: DomainPackAuthorArtifactRefs {
                manifest_ref: RepoPath("domain-packs/example-authoring/manifest.yaml".to_owned()),
                content_ref: RepoPath("domain-packs/example-authoring/content.yaml".to_owned()),
                license_ref: RepoPath("domain-packs/example-authoring/LICENSE.yaml".to_owned()),
            },
        },
    }
}

#[test]
fn authoring_sidecars_reject_unknown_fields() {
    let mut value = serde_json::to_value(sidecars()).expect("sidecar JSON");
    value["invented_authority"] = serde_json::json!("trusted");
    assert!(serde_json::from_value::<DomainPackAuthorRawSidecars>(value).is_err());
}

#[test]
fn authoring_issue_has_only_candidate_authority() {
    let issue = DomainPackAuthorIssue {
        code: DomainPackAuthorIssueCode::MissingMaterial,
        path: "raw_sidecars".to_owned(),
        message: "raw sidecars are required".to_owned(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
    };
    let mut value = serde_json::to_value(&issue).expect("issue JSON");
    assert_eq!(value["authority"], "candidate_only");
    value["authority"] = serde_json::json!("active");
    assert!(serde_json::from_value::<DomainPackAuthorIssue>(value).is_err());
}

#[test]
fn sealed_core_binding_rejects_malformed_digests_and_identity_mismatch() {
    let mut malformed_digest = skeleton_request();
    malformed_digest
        .domain_pack_author_skeleton_request
        .core
        .bundle_digest = "sha256:not-a-digest".to_owned();
    malformed_digest
        .domain_pack_author_skeleton_request
        .core
        .policy_set_digest = "not-a-digest".to_owned();
    let digest_issues = malformed_digest.validate_sealed_core_binding();
    assert!(digest_issues
        .iter()
        .any(|issue| issue.path == "core.bundle_digest"));
    assert!(digest_issues
        .iter()
        .any(|issue| issue.path == "core.policy_set_digest"));

    let mut invalid_identity = skeleton_request();
    invalid_identity
        .domain_pack_author_skeleton_request
        .core
        .bundle_id = StableId("other.core".to_owned());
    assert!(invalid_identity
        .validate_sealed_core_binding()
        .iter()
        .any(|issue| issue.code == DomainPackAuthorIssueCode::CoreShadowing));
}
