use forge_core_contracts::{
    DomainPackAuthorArtifactRefs, DomainPackAuthorIssueCode, DomainPackAuthorProvenanceTemplate,
    DomainPackAuthorRawSidecars, DomainPackAuthorSkeletonRequest,
    DomainPackAuthorSkeletonRequestDocument, DomainPackAuthorTestRequest,
    DomainPackAuthorTestRequestDocument, DomainPackCandidateAuthority, DomainPackCoreBinding,
    DomainPackDomainRequirement, DomainPackProjectRequirements,
    DomainPackProjectRequirementsDocument, DomainPackSourceKind, RepoPath, StableId,
    WorkflowGovernanceBundle, DOMAIN_PACK_AUTHORING_SCHEMA_VERSION, DOMAIN_PACK_SCHEMA_VERSION,
};
use forge_core_decisions::{
    evaluate_domain_pack_author_test, generate_domain_pack_author_skeleton,
};
use sha2::{Digest, Sha256};

fn digest<T: serde::Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn source_digest() -> String {
    format!("sha256:{}", "0".repeat(64))
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
                bundle_digest: digest(&bundle),
                policy_set_digest: digest(&bundle.policies),
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
                source_digest: source_digest(),
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

fn author_test_request() -> DomainPackAuthorTestRequestDocument {
    let skeleton = generate_domain_pack_author_skeleton(&skeleton_request());
    let template = skeleton
        .domain_pack_author_skeleton
        .template
        .expect("valid generic skeleton");
    DomainPackAuthorTestRequestDocument {
        schema_version: DOMAIN_PACK_AUTHORING_SCHEMA_VERSION.to_owned(),
        domain_pack_author_test_request: DomainPackAuthorTestRequest {
            request_id: StableId("authoring.test".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            candidate: template.candidate.clone(),
            composition_request: template.composition_request,
            raw_sidecars: vec![DomainPackAuthorRawSidecars {
                pack: forge_core_contracts::DomainPackVersionReference {
                    publisher: template
                        .candidate
                        .manifest
                        .domain_pack_manifest
                        .identity
                        .publisher,
                    name: template
                        .candidate
                        .manifest
                        .domain_pack_manifest
                        .identity
                        .name,
                    version: template
                        .candidate
                        .manifest
                        .domain_pack_manifest
                        .identity
                        .version,
                },
                manifest_raw: template.manifest.raw_bytes,
                content_raw: template.content.raw_bytes,
                license_raw: template.license.raw_bytes,
            }],
            compatibility: None,
            learning: None,
            reviewed_registry: None,
        },
    }
}

#[test]
fn generic_skeleton_is_deterministic_and_candidate_only() {
    let first = generate_domain_pack_author_skeleton(&skeleton_request());
    let second = generate_domain_pack_author_skeleton(&skeleton_request());
    assert_eq!(first, second);
    let skeleton = &first.domain_pack_author_skeleton;
    assert_eq!(
        skeleton.authority,
        DomainPackCandidateAuthority::CandidateOnly
    );
    assert!(skeleton.template.is_some());
    assert!(skeleton.issues.is_empty(), "{:?}", skeleton.issues);
}

#[test]
fn skeleton_generation_blocks_malformed_sealed_core_bindings() {
    let mut malformed_digest = skeleton_request();
    malformed_digest
        .domain_pack_author_skeleton_request
        .core
        .bundle_digest = "sha256:not-a-digest".to_owned();
    malformed_digest
        .domain_pack_author_skeleton_request
        .core
        .policy_set_digest = "not-a-digest".to_owned();
    let malformed_result = generate_domain_pack_author_skeleton(&malformed_digest);
    assert!(malformed_result
        .domain_pack_author_skeleton
        .template
        .is_none());
    assert_eq!(
        malformed_result.domain_pack_author_skeleton.status,
        forge_core_contracts::DomainPackAuthorSkeletonStatus::Blocked
    );
    assert!(malformed_result
        .domain_pack_author_skeleton
        .issues
        .iter()
        .any(|issue| issue.path == "core.bundle_digest"));
    assert!(malformed_result
        .domain_pack_author_skeleton
        .issues
        .iter()
        .any(|issue| issue.path == "core.policy_set_digest"));

    let mut mismatched_identity = skeleton_request();
    mismatched_identity
        .domain_pack_author_skeleton_request
        .core
        .bundle_id = StableId("other.core".to_owned());
    let mismatch_result = generate_domain_pack_author_skeleton(&mismatched_identity);
    assert!(mismatch_result
        .domain_pack_author_skeleton
        .template
        .is_none());
    assert!(mismatch_result
        .domain_pack_author_skeleton
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackAuthorIssueCode::CoreShadowing));
}

#[test]
fn optional_compatibility_and_learning_sections_are_absent_without_evidence() {
    let report = evaluate_domain_pack_author_test(&author_test_request());
    assert!(report
        .domain_pack_author_test_report
        .compatibility
        .is_none());
    assert!(report.domain_pack_author_test_report.learning.is_none());
    assert!(report
        .domain_pack_author_test_report
        .reviewed_registry
        .is_none());
}

#[test]
fn author_test_rejects_raw_tamper_and_is_order_deterministic() {
    let request = author_test_request();
    let mut tampered = request.clone();
    tampered.domain_pack_author_test_request.raw_sidecars[0]
        .content_raw
        .extend_from_slice(b"\n# raw tamper\n");
    let report = evaluate_domain_pack_author_test(&tampered);
    assert!(report
        .domain_pack_author_test_report
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackAuthorIssueCode::RawCanonicalMismatch));

    let mut reordered = tampered.clone();
    let duplicate = reordered.domain_pack_author_test_request.raw_sidecars[0].clone();
    reordered
        .domain_pack_author_test_request
        .raw_sidecars
        .push(duplicate);
    let mut reversed = reordered.clone();
    reversed
        .domain_pack_author_test_request
        .raw_sidecars
        .reverse();
    assert_eq!(
        evaluate_domain_pack_author_test(&reordered),
        evaluate_domain_pack_author_test(&reversed)
    );
}

#[test]
fn author_test_keeps_core_shadow_and_gap_diagnostics_explicit() {
    let mut request = author_test_request();
    request
        .domain_pack_author_test_request
        .composition_request
        .domain_pack_composition_request
        .core
        .bundle_id = StableId("other.core".to_owned());
    request
        .domain_pack_author_test_request
        .composition_request
        .domain_pack_composition_request
        .forge_core_version = "2.0.0".to_owned();
    request
        .domain_pack_author_test_request
        .composition_request
        .domain_pack_composition_request
        .requirements
        .required_domains
        .push(DomainPackDomainRequirement {
            id: StableId("required.domain".to_owned()),
            domain_id: StableId("required.domain".to_owned()),
            pack_version_requirement: "*".to_owned(),
            required_capability_refs: Vec::new(),
        });
    let report = evaluate_domain_pack_author_test(&request);
    assert!(report
        .domain_pack_author_test_report
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackAuthorIssueCode::CoreShadowing));
    assert!(report
        .domain_pack_author_test_report
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackAuthorIssueCode::IncompatibleCore));
    assert!(report
        .domain_pack_author_test_report
        .gaps
        .iter()
        .any(|issue| issue.code == DomainPackAuthorIssueCode::MissingDomain));

    let serialized = serde_json::to_string(&report).expect("author report JSON");
    assert!(serialized.contains("candidate_only"));
    assert!(!serialized.contains("active_pointer"));
    assert!(!serialized.contains("lifecycle_receipt"));
    assert!(!serialized.contains("activation_authority"));
    assert!(!serialized.contains("\"operation\""));
    assert!(!serialized.contains("\"lifecycle\""));
}
