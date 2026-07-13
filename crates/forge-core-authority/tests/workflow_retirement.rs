use std::fmt::Write as _;

use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    verify_workflow_retirement_authorization_v2, workflow_retirement_payload_digest_v2,
    workflow_retirement_reviewer_key_fingerprint_v2, workflow_retirement_signing_bytes_v2,
    WorkflowRetirementAuthorityErrorV2, WorkflowRetirementExpectedContextV2,
    WORKFLOW_RETIREMENT_PAYLOAD_DOMAIN_V2,
};
use forge_core_contracts::{
    WorkflowReleaseReviewerRegistryDocument, WorkflowRetirementAuthorizationV2Document,
    WorkflowRetirementAuthorizationV2Payload,
};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

const AUDIENCE: &str = "forge-runtime:workflow-retirement";

struct Fixture {
    registry: WorkflowReleaseReviewerRegistryDocument,
    registry_raw: Vec<u8>,
    authorization: WorkflowRetirementAuthorizationV2Document,
    expected: WorkflowRetirementAuthorizationV2Payload,
    semantic_key: SigningKey,
    authorizer_key: SigningKey,
    trusted_registry_raw_digest: String,
    trusted_semantic_fingerprint: String,
    trusted_authorizer_fingerprint: String,
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    let value = serde_json::to_value(value).expect("JSON");
    digest(&serde_json_canonicalizer::to_vec(&value).expect("JCS"))
}

fn hex(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("String write");
    }
    value
}

fn key(seed: u64) -> SigningKey {
    let mut bytes = [0; 32];
    StdRng::seed_from_u64(seed).fill_bytes(&mut bytes);
    SigningKey::from_bytes(&bytes)
}

fn artifact(id: &str) -> serde_json::Value {
    json!({
        "artifact_id": id,
        "embedded_ref": format!("contracts/evidence/{id}.yaml"),
        "raw_digest": digest(format!("{id}:raw").as_bytes()),
        "canonical_digest": digest(format!("{id}:canonical").as_bytes())
    })
}

#[allow(clippy::too_many_lines)]
fn fixture() -> Fixture {
    let semantic_key = key(81);
    let authorizer_key = key(82);
    let registry: WorkflowReleaseReviewerRegistryDocument = serde_json::from_value(json!({
        "schema_version": "0.1",
        "workflow_release_reviewer_registry": {
            "registry_id": "reviewers.retirement",
            "registry_version": "1.0.0",
            "authority": "candidate_only",
            "credentials": [
                { "credential_id": "credential.retirement.semantic", "principal_id": "principal.retirement.semantic",
                  "public_key_hex": hex(semantic_key.verifying_key().as_bytes()),
                  "public_key_fingerprint": workflow_retirement_reviewer_key_fingerprint_v2(semantic_key.verifying_key().as_bytes()),
                  "algorithm": "ed25519", "roles": ["semantic_reviewer"], "status": "active",
                  "valid_from_unix": 1000, "valid_until_unix": 2000, "independence_domain": "retirement-evidence" },
                { "credential_id": "credential.retirement.authorizer", "principal_id": "principal.retirement.authorizer",
                  "public_key_hex": hex(authorizer_key.verifying_key().as_bytes()),
                  "public_key_fingerprint": workflow_retirement_reviewer_key_fingerprint_v2(authorizer_key.verifying_key().as_bytes()),
                  "algorithm": "ed25519", "roles": ["release_authorizer"], "status": "active",
                  "valid_from_unix": 1000, "valid_until_unix": 2000, "independence_domain": "retirement-operations" }
            ]
        }
    })).expect("registry");
    let registry_raw = serde_json::to_vec_pretty(&registry).expect("registry bytes");
    let retirements = (0..42)
        .map(|index| {
            json!({
                "workflow_id": format!("workflow-{index:02}"),
                "legacy_workflow_digest": digest(format!("legacy-{index}").as_bytes()),
                "replacement_policy_ref": format!("policy.workflow-{index:02}"),
                "replacement_policy_digest": digest(format!("policy-{index}").as_bytes())
            })
        })
        .collect::<Vec<_>>();
    let mut reviewer_binding = artifact("reviewer-registry");
    reviewer_binding["raw_digest"] = json!(digest(&registry_raw));
    reviewer_binding["canonical_digest"] = json!(canonical_digest(&registry));
    let authorization: WorkflowRetirementAuthorizationV2Document = serde_json::from_value(json!({
        "schema_version": "0.2",
        "workflow_retirement_authorization_v2": {
            "authority": "candidate_authorization",
            "payload": {
                "authorization_id": "authorization.retirement.core-42",
                "release": { "lineage_id": "lineage.core", "release_id": "release.agent-native-continuity", "release_version": "0.4.0", "release_digest": digest(b"release") },
                "runtime_bundle": { "bundle_id": "bundle.agent-native-continuity", "bundle_digest": digest(b"bundle"), "policy_set_digest": digest(b"policies") },
                "legacy_catalog_digest": digest(b"legacy-catalog-110"), "retirements": retirements,
                "release_manifest": artifact("release-manifest"),
                "runtime_bundle_artifact": artifact("runtime-bundle"),
                "snapshot_manifest": artifact("snapshot-manifest"),
                "runtime_evidence": artifact("runtime-evidence"),
                "release_history": artifact("release-history"),
                "evidence_index": artifact("retirement-index"), "deletion_proof": artifact("deletion-proof"),
                "consumer_report": artifact("consumer-report"), "tombstone_catalog": artifact("tombstones"),
                "final_scorecard": artifact("final-scorecard"), "reviewer_registry": reviewer_binding,
                "audience": AUDIENCE, "domain": WORKFLOW_RETIREMENT_PAYLOAD_DOMAIN_V2,
                "nonce": "retirement-core-42-2026-07-12-unique", "issued_at_unix": 1100, "expires_at_unix": 1900
            },
            "signatures": [
                { "principal_id": "principal.retirement.semantic", "credential_id": "credential.retirement.semantic", "role": "semantic_reviewer", "algorithm": "ed25519", "payload_digest": digest(b"pending"), "signature": "00".repeat(64), "signed_at_unix": 1500 },
                { "principal_id": "principal.retirement.authorizer", "credential_id": "credential.retirement.authorizer", "role": "release_authorizer", "algorithm": "ed25519", "payload_digest": digest(b"pending"), "signature": "00".repeat(64), "signed_at_unix": 1501 }
            ]
        }
    })).expect("authorization");
    let expected = authorization
        .workflow_retirement_authorization_v2
        .payload
        .clone();
    let trusted_registry_raw_digest = digest(&registry_raw);
    let trusted_semantic_fingerprint = registry.workflow_release_reviewer_registry.credentials[0]
        .public_key_fingerprint
        .clone();
    let trusted_authorizer_fingerprint = registry.workflow_release_reviewer_registry.credentials[1]
        .public_key_fingerprint
        .clone();
    let mut fixture = Fixture {
        registry,
        registry_raw,
        authorization,
        expected,
        semantic_key,
        authorizer_key,
        trusted_registry_raw_digest,
        trusted_semantic_fingerprint,
        trusted_authorizer_fingerprint,
    };
    resign(&mut fixture, false);
    fixture
}

fn resign(fixture: &mut Fixture, sync_expected_registry: bool) {
    fixture.registry_raw = serde_json::to_vec_pretty(&fixture.registry).expect("registry bytes");
    let binding = &mut fixture
        .authorization
        .workflow_retirement_authorization_v2
        .payload
        .reviewer_registry;
    binding.raw_digest = digest(&fixture.registry_raw);
    binding.canonical_digest = canonical_digest(&fixture.registry);
    if sync_expected_registry {
        fixture.expected.reviewer_registry = binding.clone();
    }
    let authorization = &mut fixture.authorization.workflow_retirement_authorization_v2;
    let payload_digest =
        workflow_retirement_payload_digest_v2(&authorization.payload).expect("digest");
    for (index, signature) in authorization.signatures.iter_mut().enumerate() {
        signature.payload_digest.clone_from(&payload_digest);
        let bytes =
            workflow_retirement_signing_bytes_v2(&authorization.payload, signature).expect("bytes");
        let signing_key = if index == 0 {
            &fixture.semantic_key
        } else {
            &fixture.authorizer_key
        };
        signature.signature = hex(&signing_key.sign(&bytes).to_bytes());
    }
}

fn verify(
    fixture: &Fixture,
    admission_epoch_unix: u64,
) -> Result<
    forge_core_authority::VerifiedWorkflowRetirementAuthorizationV2,
    WorkflowRetirementAuthorityErrorV2,
> {
    let p = &fixture.expected;
    verify_workflow_retirement_authorization_v2(
        &fixture.registry,
        &fixture.registry_raw,
        &fixture.authorization,
        WorkflowRetirementExpectedContextV2 {
            release: &p.release,
            runtime_bundle: &p.runtime_bundle,
            legacy_catalog_digest: &p.legacy_catalog_digest,
            retirements: &p.retirements,
            release_manifest: &p.release_manifest,
            runtime_bundle_artifact: &p.runtime_bundle_artifact,
            snapshot_manifest: &p.snapshot_manifest,
            runtime_evidence: &p.runtime_evidence,
            release_history: &p.release_history,
            evidence_index: &p.evidence_index,
            deletion_proof: &p.deletion_proof,
            consumer_report: &p.consumer_report,
            tombstone_catalog: &p.tombstone_catalog,
            final_scorecard: &p.final_scorecard,
            reviewer_registry: &p.reviewer_registry,
            admission_epoch_unix,
            consumer_observed_until_unix: 1400,
            reviewer_registry_raw_digest: &fixture.trusted_registry_raw_digest,
            evidence_reviewer_key_fingerprint: &fixture.trusted_semantic_fingerprint,
            retirement_authorizer_key_fingerprint: &fixture.trusted_authorizer_fingerprint,
        },
        AUDIENCE,
    )
}

#[test]
fn exact_aggregate_and_independent_signatures_create_opaque_capability() {
    let fixture = fixture();
    let verified = verify(&fixture, 1600).expect("valid retirement authorization");
    assert_eq!(
        verified.authorization_id(),
        "authorization.retirement.core-42"
    );
    assert_eq!(verified.release().release_version, "0.4.0");
    assert_ne!(
        verified.audit().evidence_reviewer.principal_id,
        verified.audit().retirement_authorizer.principal_id
    );
}

#[test]
fn fixed_admission_epoch_is_strictly_enforced() {
    let fixture = fixture();
    assert!(matches!(
        verify(&fixture, 1099),
        Err(WorkflowRetirementAuthorityErrorV2::AuthorizationNotYetValid)
    ));
    assert!(matches!(
        verify(&fixture, 1900),
        Err(WorkflowRetirementAuthorityErrorV2::AuthorizationExpired)
    ));
}

#[test]
fn evidence_scorecard_and_workflow_transplants_fail_before_signature_trust() {
    let mut scorecard = fixture();
    scorecard
        .authorization
        .workflow_retirement_authorization_v2
        .payload
        .final_scorecard
        .canonical_digest = digest(b"other-scorecard");
    assert!(matches!(
        verify(&scorecard, 1600),
        Err(WorkflowRetirementAuthorityErrorV2::BindingMismatch {
            field: "final_scorecard"
        })
    ));

    let mut workflow = fixture();
    workflow
        .authorization
        .workflow_retirement_authorization_v2
        .payload
        .retirements[0]
        .workflow_id
        .0 = "attacker-workflow".to_owned();
    assert!(matches!(
        verify(&workflow, 1600),
        Err(WorkflowRetirementAuthorityErrorV2::BindingMismatch {
            field: "retirements"
        })
    ));

    let mut history = fixture();
    history
        .authorization
        .workflow_retirement_authorization_v2
        .payload
        .release_history
        .canonical_digest = digest(b"rollback-history");
    assert!(matches!(
        verify(&history, 1600),
        Err(WorkflowRetirementAuthorityErrorV2::BindingMismatch {
            field: "release_history"
        })
    ));
}

#[test]
fn reviewer_independence_and_current_credential_validity_fail_closed() {
    let mut same_domain = fixture();
    same_domain
        .registry
        .workflow_release_reviewer_registry
        .credentials[1]
        .independence_domain = "retirement-evidence".to_owned();
    resign(&mut same_domain, true);
    same_domain.trusted_registry_raw_digest = digest(&same_domain.registry_raw);
    assert!(matches!(
        verify(&same_domain, 1600),
        Err(
            WorkflowRetirementAuthorityErrorV2::ReviewerSeparationViolation {
                dimension: "independence domain"
            }
        )
    ));

    let mut expired_credential = fixture();
    expired_credential
        .registry
        .workflow_release_reviewer_registry
        .credentials[0]
        .valid_until_unix = 1550;
    resign(&mut expired_credential, true);
    expired_credential.trusted_registry_raw_digest = digest(&expired_credential.registry_raw);
    assert!(matches!(
        verify(&expired_credential, 1600),
        Err(WorkflowRetirementAuthorityErrorV2::CredentialOutsideValidity { .. })
    ));
}

#[test]
fn tampered_signature_and_non_42_aggregate_reject() {
    let mut signature = fixture();
    signature
        .authorization
        .workflow_retirement_authorization_v2
        .signatures[0]
        .signature = "11".repeat(64);
    assert!(matches!(
        verify(&signature, 1600),
        Err(WorkflowRetirementAuthorityErrorV2::SignatureInvalid { .. })
    ));

    let mut short = fixture();
    short
        .authorization
        .workflow_retirement_authorization_v2
        .payload
        .retirements
        .pop();
    short.expected.retirements.pop();
    resign(&mut short, false);
    assert!(matches!(
        verify(&short, 1600),
        Err(WorkflowRetirementAuthorityErrorV2::InvalidAggregate { .. })
    ));
}

#[test]
fn coordinated_registry_key_rotation_cannot_replace_binary_trust_roots() {
    let mut rotated = fixture();
    rotated.semantic_key = key(181);
    rotated.authorizer_key = key(182);
    let credentials = &mut rotated
        .registry
        .workflow_release_reviewer_registry
        .credentials;
    for (credential, key) in credentials
        .iter_mut()
        .zip([&rotated.semantic_key, &rotated.authorizer_key])
    {
        credential.public_key_hex = hex(key.verifying_key().as_bytes());
        credential.public_key_fingerprint =
            workflow_retirement_reviewer_key_fingerprint_v2(key.verifying_key().as_bytes());
    }
    resign(&mut rotated, true);
    assert!(matches!(
        verify(&rotated, 1600),
        Err(WorkflowRetirementAuthorityErrorV2::BindingMismatch {
            field: "trusted_reviewer_registry.raw_digest"
        })
    ));
}

#[test]
fn release_runtime_snapshot_and_consumer_chronology_are_signed_and_fail_closed() {
    for field in [
        "release_manifest",
        "runtime_bundle_artifact",
        "snapshot_manifest",
    ] {
        let mut changed = fixture();
        let binding = match field {
            "release_manifest" => {
                &mut changed
                    .authorization
                    .workflow_retirement_authorization_v2
                    .payload
                    .release_manifest
            }
            "runtime_bundle_artifact" => {
                &mut changed
                    .authorization
                    .workflow_retirement_authorization_v2
                    .payload
                    .runtime_bundle_artifact
            }
            "snapshot_manifest" => {
                &mut changed
                    .authorization
                    .workflow_retirement_authorization_v2
                    .payload
                    .snapshot_manifest
            }
            _ => unreachable!(),
        };
        binding.raw_digest = digest(b"whitespace drift");
        assert!(matches!(
            verify(&changed, 1600),
            Err(WorkflowRetirementAuthorityErrorV2::BindingMismatch { .. })
        ));
    }
    let fixture = fixture();
    // The evidence window closes at 1400; admitting a signature from before it is forbidden.
    let mut early = fixture;
    early
        .authorization
        .workflow_retirement_authorization_v2
        .signatures[0]
        .signed_at_unix = 1399;
    resign(&mut early, false);
    assert!(matches!(
        verify(&early, 1600),
        Err(WorkflowRetirementAuthorityErrorV2::SignatureOutsideValidity { .. })
    ));
}
