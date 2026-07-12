use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    verify_workflow_release_admission_authorization, workflow_release_admission_payload_digest,
    workflow_release_admission_signing_bytes, workflow_release_reviewer_key_fingerprint,
    WorkflowReleaseAdmissionAuditAuthority, WorkflowReleaseAdmissionAuthorityError,
};
use forge_core_contracts::{
    WorkflowReleaseAdmissionAuthorizationDocument, WorkflowReleaseReviewerRegistryDocument,
};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

const AUDIENCE: &str = "forge-runtime:workflow-governance-release";

struct Fixture {
    registry: WorkflowReleaseReviewerRegistryDocument,
    registry_raw: Vec<u8>,
    authorization: WorkflowReleaseAdmissionAuthorizationDocument,
    semantic_key: SigningKey,
    authorizer_key: SigningKey,
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    let value = serde_json::to_value(value).expect("JSON value");
    sha256(&serde_json_canonicalizer::to_vec(&value).expect("JCS"))
}

fn hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn decisions(prefix: &str, count: usize) -> Vec<serde_json::Value> {
    (0..count)
        .map(|index| {
            json!({
                "workflow_id": format!("{prefix}.{index}"),
                "decision": "approved",
                "rationale": "independently reviewed",
                "finding_refs": []
            })
        })
        .collect()
}

fn test_key(seed: u64) -> SigningKey {
    let mut bytes = [0_u8; 32];
    StdRng::seed_from_u64(seed).fill_bytes(&mut bytes);
    SigningKey::from_bytes(&bytes)
}

#[allow(
    clippy::too_many_lines,
    reason = "one complete closed wire fixture is easier to audit"
)]
fn fixture() -> Fixture {
    let semantic_key = test_key(71);
    let authorizer_key = test_key(72);
    let registry: WorkflowReleaseReviewerRegistryDocument = serde_json::from_value(json!({
        "schema_version": "0.1",
        "workflow_release_reviewer_registry": {
            "registry_id": "reviewers.p5d4",
            "registry_version": "1.0.0",
            "authority": "candidate_only",
            "credentials": [
                {
                    "credential_id": "credential.semantic",
                    "principal_id": "principal.semantic",
                    "public_key_hex": hex(semantic_key.verifying_key().as_bytes()),
                    "public_key_fingerprint": workflow_release_reviewer_key_fingerprint(semantic_key.verifying_key().as_bytes()),
                    "algorithm": "ed25519",
                    "roles": ["semantic_reviewer"],
                    "status": "active",
                    "valid_from_unix": 1_000,
                    "valid_until_unix": 2_000,
                    "independence_domain": "semantic-review"
                },
                {
                    "credential_id": "credential.authorizer",
                    "principal_id": "principal.authorizer",
                    "public_key_hex": hex(authorizer_key.verifying_key().as_bytes()),
                    "public_key_fingerprint": workflow_release_reviewer_key_fingerprint(authorizer_key.verifying_key().as_bytes()),
                    "algorithm": "ed25519",
                    "roles": ["release_authorizer"],
                    "status": "active",
                    "valid_from_unix": 1_000,
                    "valid_until_unix": 2_000,
                    "independence_domain": "release-operations"
                }
            ]
        }
    }))
    .expect("registry");
    let registry_raw = serde_json::to_vec_pretty(&registry).expect("registry bytes");
    let dimensions = [
        "status",
        "eligibility",
        "progression",
        "completion",
        "obligations",
        "claims",
        "decisions",
        "capabilities",
        "issues",
        "next_actions",
    ]
    .into_iter()
    .map(|dimension| {
        json!({
            "dimension": dimension,
            "decision": "approved",
            "rationale": "dimension reviewed",
            "finding_refs": []
        })
    })
    .collect::<Vec<_>>();
    let authorization: WorkflowReleaseAdmissionAuthorizationDocument =
        serde_json::from_value(json!({
            "schema_version": "0.1",
            "workflow_release_admission_authorization": {
                "authority": "candidate_authorization",
                "payload": {
                    "authorization_id": "authorization.core-assurance.v1",
                    "review_index_id": "review-index.core-assurance",
                    "review_index_version": "1.0.0",
                    "review_index_raw_digest": sha256(b"review-index-raw"),
                    "review_index_canonical_digest": sha256(b"review-index-jcs"),
                    "evaluation_digest": sha256(b"independent-evaluation"),
                    "reviewer_registry_id": "reviewers.p5d4",
                    "reviewer_registry_version": "1.0.0",
                    "reviewer_registry_raw_digest": sha256(&registry_raw),
                    "reviewer_registry_canonical_digest": canonical_digest(&registry),
                    "promotion": {
                        "predecessor": {
                            "release_id": "release.foundation",
                            "release_digest": sha256(b"foundation")
                        },
                        "candidate_release": {
                            "lineage_id": "lineage.core",
                            "release_id": "release.core-assurance",
                            "release_version": "0.3.0",
                            "release_digest": sha256(b"core-assurance")
                        },
                        "candidate_runtime_bundle": {
                            "bundle_id": "bundle.core-assurance",
                            "bundle_digest": sha256(b"bundle"),
                            "policy_set_digest": sha256(b"policies")
                        },
                        "promoted_runtime_bundle": {
                            "bundle_id": "bundle.core-assurance.admitted",
                            "bundle_digest": sha256(b"promoted-bundle"),
                            "policy_set_digest": sha256(b"policies")
                        }
                    },
                    "invalidate_all_receipts": true,
                    "workflow_decisions": decisions("workflow", 5),
                    "quarantine_decisions": decisions("quarantine", 3),
                    "dimension_decisions": dimensions,
                    "audience": AUDIENCE,
                    "domain": "forge-method:workflow-release-admission:v1",
                    "nonce": "review-session-core-assurance-v1",
                    "issued_at_unix": 1_100,
                    "expires_at_unix": 1_900
                },
                "signatures": [
                    {
                        "principal_id": "principal.semantic",
                        "credential_id": "credential.semantic",
                        "role": "semantic_reviewer",
                        "algorithm": "ed25519",
                        "payload_digest": sha256(b"pending"),
                        "signature": "00".repeat(64),
                        "signed_at_unix": 1_500
                    },
                    {
                        "principal_id": "principal.authorizer",
                        "credential_id": "credential.authorizer",
                        "role": "release_authorizer",
                        "algorithm": "ed25519",
                        "payload_digest": sha256(b"pending"),
                        "signature": "00".repeat(64),
                        "signed_at_unix": 1_501
                    }
                ]
            }
        }))
        .expect("authorization");
    let mut fixture = Fixture {
        registry,
        registry_raw,
        authorization,
        semantic_key,
        authorizer_key,
    };
    resign(&mut fixture);
    fixture
}

fn resign(fixture: &mut Fixture) {
    fixture.registry_raw = serde_json::to_vec_pretty(&fixture.registry).expect("registry bytes");
    let registry = &fixture.registry.workflow_release_reviewer_registry;
    let authorization = &mut fixture
        .authorization
        .workflow_release_admission_authorization;
    authorization.payload.reviewer_registry_id = registry.registry_id.clone();
    authorization
        .payload
        .reviewer_registry_version
        .clone_from(&registry.registry_version);
    authorization.payload.reviewer_registry_raw_digest = sha256(&fixture.registry_raw);
    authorization.payload.reviewer_registry_canonical_digest = canonical_digest(&fixture.registry);
    let payload_digest =
        workflow_release_admission_payload_digest(&authorization.payload).expect("payload digest");
    for (index, signature) in authorization.signatures.iter_mut().enumerate() {
        signature.payload_digest.clone_from(&payload_digest);
        let bytes = workflow_release_admission_signing_bytes(&authorization.payload, signature)
            .expect("signing bytes");
        let key = if index == 0 {
            &fixture.semantic_key
        } else {
            &fixture.authorizer_key
        };
        signature.signature = hex(&key.sign(&bytes).to_bytes());
    }
}

fn verify(
    fixture: &Fixture,
) -> Result<
    forge_core_authority::VerifiedWorkflowReleaseAdmissionAuthorization,
    WorkflowReleaseAdmissionAuthorityError,
> {
    verify_workflow_release_admission_authorization(
        &fixture.registry,
        &fixture.registry_raw,
        &fixture.authorization,
        AUDIENCE,
    )
}

#[test]
fn distinct_registry_bound_reviewers_create_only_an_opaque_capability() {
    let valid = fixture();
    let capability = verify(&valid).expect("independent authorization");
    assert_eq!(
        capability.authorization_id(),
        "authorization.core-assurance.v1"
    );
    assert_eq!(
        capability.candidate_release().release_id.0,
        "release.core-assurance"
    );
    assert_eq!(
        capability.audit().authority,
        WorkflowReleaseAdmissionAuditAuthority::NonAuthoritative
    );
}

#[test]
fn rejects_wrong_audience_revocation_and_out_of_window_signature() {
    let valid = fixture();
    assert!(matches!(
        verify_workflow_release_admission_authorization(
            &valid.registry,
            &valid.registry_raw,
            &valid.authorization,
            "another-runtime"
        ),
        Err(WorkflowReleaseAdmissionAuthorityError::WrongAudience { .. })
    ));

    let mut revoked = fixture();
    revoked
        .registry
        .workflow_release_reviewer_registry
        .credentials[0]
        .status = forge_core_contracts::WorkflowReleaseReviewerCredentialStatus::Revoked;
    resign(&mut revoked);
    assert!(matches!(
        verify(&revoked),
        Err(WorkflowReleaseAdmissionAuthorityError::CredentialNotActive { .. })
    ));

    let mut expired = fixture();
    expired
        .registry
        .workflow_release_reviewer_registry
        .credentials[0]
        .valid_until_unix = 1_400;
    resign(&mut expired);
    assert!(matches!(
        verify(&expired),
        Err(WorkflowReleaseAdmissionAuthorityError::CredentialOutsideValidity { .. })
    ));
}

#[test]
fn rejects_tamper_and_signature_transplant_to_another_authorization() {
    let mut tampered = fixture();
    tampered
        .authorization
        .workflow_release_admission_authorization
        .payload
        .promotion
        .candidate_release
        .release_digest = sha256(b"attacker-release");
    assert!(matches!(
        verify(&tampered),
        Err(WorkflowReleaseAdmissionAuthorityError::PayloadDigestMismatch { .. })
    ));

    let mut replay = fixture();
    replay
        .authorization
        .workflow_release_admission_authorization
        .payload
        .authorization_id
        .0 = "authorization.transplanted".to_owned();
    assert!(matches!(
        verify(&replay),
        Err(WorkflowReleaseAdmissionAuthorityError::PayloadDigestMismatch { .. })
    ));
}

#[test]
fn rejects_wrong_domain_and_signature_from_key_outside_registry() {
    let mut wrong_domain = fixture();
    wrong_domain
        .authorization
        .workflow_release_admission_authorization
        .payload
        .domain = "forge-method:another-domain:v1".to_owned();
    resign(&mut wrong_domain);
    assert!(matches!(
        verify(&wrong_domain),
        Err(WorkflowReleaseAdmissionAuthorityError::WrongDomain { .. })
    ));

    let mut attacker = fixture();
    attacker.authorizer_key = test_key(999);
    resign(&mut attacker);
    assert!(matches!(
        verify(&attacker),
        Err(WorkflowReleaseAdmissionAuthorityError::SignatureInvalid { .. })
    ));
}

#[test]
fn rejects_duplicate_principal_key_and_signature() {
    let mut same_principal = fixture();
    same_principal
        .registry
        .workflow_release_reviewer_registry
        .credentials[1]
        .principal_id = same_principal
        .registry
        .workflow_release_reviewer_registry
        .credentials[0]
        .principal_id
        .clone();
    same_principal
        .authorization
        .workflow_release_admission_authorization
        .signatures[1]
        .principal_id = same_principal
        .registry
        .workflow_release_reviewer_registry
        .credentials[0]
        .principal_id
        .clone();
    resign(&mut same_principal);
    assert!(verify(&same_principal).is_err());

    let mut same_key = fixture();
    same_key.authorizer_key = test_key(71);
    let key_hex = hex(same_key.authorizer_key.verifying_key().as_bytes());
    let fingerprint = workflow_release_reviewer_key_fingerprint(
        same_key.authorizer_key.verifying_key().as_bytes(),
    );
    let credential = &mut same_key
        .registry
        .workflow_release_reviewer_registry
        .credentials[1];
    credential.public_key_hex = key_hex;
    credential.public_key_fingerprint = fingerprint;
    resign(&mut same_key);
    assert!(verify(&same_key).is_err());

    let mut duplicate_signature = fixture();
    let first = duplicate_signature
        .authorization
        .workflow_release_admission_authorization
        .signatures[0]
        .signature
        .clone();
    duplicate_signature
        .authorization
        .workflow_release_admission_authorization
        .signatures[1]
        .signature = first;
    assert!(matches!(
        verify(&duplicate_signature),
        Err(WorkflowReleaseAdmissionAuthorityError::DuplicateSignature)
    ));
}

#[test]
fn explicit_blocking_decision_never_authorizes() {
    let mut fixture = fixture();
    fixture
        .authorization
        .workflow_release_admission_authorization
        .payload
        .workflow_decisions[0]
        .decision = forge_core_contracts::WorkflowReleaseReviewDecision::ChangesRequired;
    resign(&mut fixture);
    assert!(matches!(
        verify(&fixture),
        Err(WorkflowReleaseAdmissionAuthorityError::BlockingReviewDecision)
    ));
}
