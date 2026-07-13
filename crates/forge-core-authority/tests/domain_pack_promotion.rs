use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    domain_pack_independent_review_digest, domain_pack_promotion_decision_digest,
    domain_pack_promotion_dossier_digest, domain_pack_promotion_payload_digest,
    domain_pack_promotion_reviewer_key_fingerprint, domain_pack_promotion_signing_bytes,
    domain_pack_reviewed_registry_digest, domain_pack_reviewed_registry_entry_digest,
    domain_pack_reviewed_registry_proposal_digest, domain_pack_reviewed_registry_signing_bytes,
    domain_pack_reviewer_registry_digest, domain_pack_reviewer_registry_rotation_signing_bytes,
    verify_domain_pack_promotion_authorization, DomainPackPromotionAuditAuthority,
    DomainPackPromotionAuthorityError, DomainPackPromotionExpectedContext,
    DomainPackReviewerRegistryAnchor, ReviewedDomainPackRegistryAnchor,
    DOMAIN_PACK_PROMOTION_PAYLOAD_DOMAIN,
};
use forge_core_contracts::domain_pack_learning::{
    DomainPackIndependentReviewDocument, DomainPackLearningConflictDocument,
    DomainPackLocalLearningCandidateDocument, DomainPackPromotionAuthorizationDocument,
    DomainPackPromotionDecisionDocument, DomainPackPromotionDossierDocument,
    DomainPackReviewedRegistryDocument, DomainPackReviewerRegistryDocument,
};
use forge_core_contracts::domain_pack_learning_conflict_digest;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const AUDIENCE: &str = "forge-domain-pack-runtime";
const NOW: u64 = 1_500;

struct Fixture {
    semantic_key: SigningKey,
    authorizer_key: SigningKey,
    reviewer_registry: DomainPackReviewerRegistryDocument,
    reviewer_anchor: DomainPackReviewerRegistryAnchor,
    current: DomainPackReviewedRegistryDocument,
    proposed: DomainPackReviewedRegistryDocument,
    dossier: DomainPackPromotionDossierDocument,
    candidates: Vec<DomainPackLocalLearningCandidateDocument>,
    reviews: Vec<DomainPackIndependentReviewDocument>,
    conflicts: Vec<DomainPackLearningConflictDocument>,
    decision: DomainPackPromotionDecisionDocument,
    authorization: DomainPackPromotionAuthorizationDocument,
}

fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}
fn digest(label: &str) -> String {
    format!("{:x}", Sha256::digest(label.as_bytes()))
}
fn supply_digest(label: &str) -> String {
    format!("sha256:{}", digest(label))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        use std::fmt::Write as _;
        write!(out, "{byte:02x}").unwrap();
        out
    })
}
fn full_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).unwrap();
    format!("{:x}", Sha256::digest(bytes))
}

fn local_candidate_digest(document: &DomainPackLocalLearningCandidateDocument) -> String {
    let mut value = serde_json::to_value(document).unwrap();
    value["domain_pack_local_learning_candidate"]
        .as_object_mut()
        .unwrap()
        .remove("candidate_digest")
        .unwrap();
    full_digest(&value)
}

fn reviewer(id: &str, credential: &str, role: &str, domain: &str, key: &SigningKey) -> Value {
    json!({
        "reviewer_id": id, "credential_id": credential,
        "public_key_hex": hex(key.verifying_key().as_bytes()),
        "public_key_fingerprint": domain_pack_promotion_reviewer_key_fingerprint(key.verifying_key().as_bytes()),
        "algorithm": "ed25519", "roles": [role], "independence_domains": [domain],
        "status": "active", "valid_from_unix": 1_000, "valid_until_unix": 2_000
    })
}

fn sign_snapshot(document: &mut DomainPackReviewedRegistryDocument, keys: [&SigningKey; 2]) {
    let registry_digest = domain_pack_reviewed_registry_digest(document).unwrap();
    document
        .domain_pack_reviewed_registry
        .registry_digest
        .clone_from(&registry_digest);
    for (index, key) in keys.into_iter().enumerate() {
        document.domain_pack_reviewed_registry.snapshot_signatures[index]
            .payload_digest
            .clone_from(&registry_digest);
        let signed = document.domain_pack_reviewed_registry.snapshot_signatures[index].clone();
        let bytes = domain_pack_reviewed_registry_signing_bytes(document, &signed).unwrap();
        document.domain_pack_reviewed_registry.snapshot_signatures[index].signature =
            hex(&key.sign(&bytes).to_bytes());
    }
}

fn reviewer_successor(
    predecessor: &DomainPackReviewerRegistryDocument,
    keys: [&SigningKey; 2],
) -> DomainPackReviewerRegistryDocument {
    let mut candidate = predecessor.clone();
    let predecessor_digest = domain_pack_reviewer_registry_digest(predecessor).unwrap();
    candidate.domain_pack_reviewer_registry.generation += 1;
    candidate
        .domain_pack_reviewer_registry
        .previous_registry_digest = Some(predecessor_digest.clone());
    for signed in &mut candidate.domain_pack_reviewer_registry.rotation_signatures {
        signed.predecessor_registry_digest = Some(predecessor_digest.clone());
        signed.payload_digest = digest("pending");
        signed.signature = "00".repeat(64);
        signed.signed_at_unix = NOW;
    }
    let candidate_digest = domain_pack_reviewer_registry_digest(&candidate).unwrap();
    candidate
        .domain_pack_reviewer_registry
        .registry_digest
        .clone_from(&candidate_digest);
    for (index, key) in keys.into_iter().enumerate() {
        candidate.domain_pack_reviewer_registry.rotation_signatures[index]
            .payload_digest
            .clone_from(&candidate_digest);
        let signed = candidate.domain_pack_reviewer_registry.rotation_signatures[index].clone();
        let bytes =
            domain_pack_reviewer_registry_rotation_signing_bytes(&candidate, &signed).unwrap();
        candidate.domain_pack_reviewer_registry.rotation_signatures[index].signature =
            hex(&key.sign(&bytes).to_bytes());
    }
    candidate
}

#[allow(clippy::too_many_lines)]
fn fixture() -> Fixture {
    let semantic_key = key(31);
    let authorizer_key = key(47);
    let mut reviewer_registry: DomainPackReviewerRegistryDocument = serde_json::from_value(json!({
        "schema_version": "0.3", "domain_pack_reviewer_registry": {
            "registry_id": "reviewers.learning", "audience": AUDIENCE, "generation": 0,
            "previous_registry_digest": null, "trust_policy_digest": digest("operator-trust"),
            "signature_threshold": 2,
            "reviewers": [
                reviewer("principal.semantic", "credential.semantic", "domain_expert", "domain.semantic", &semantic_key),
                reviewer("principal.authorizer", "credential.authorizer", "registry_authorizer", "domain.registry", &authorizer_key)
            ],
            "rotation_signatures": [
                {"signer_id":"principal.semantic","credential_id":"credential.semantic","predecessor_registry_digest":null,"payload_digest":digest("genesis"),"algorithm":"ed25519","signature":"00","signed_at_unix":NOW},
                {"signer_id":"principal.authorizer","credential_id":"credential.authorizer","predecessor_registry_digest":null,"payload_digest":digest("genesis"),"algorithm":"ed25519","signature":"11","signed_at_unix":NOW}
            ],
            "registry_digest": digest("pending")
        }
    })).unwrap();
    reviewer_registry
        .domain_pack_reviewer_registry
        .registry_digest = domain_pack_reviewer_registry_digest(&reviewer_registry).unwrap();
    let reviewer_full = full_digest(&reviewer_registry);
    let reviewer_anchor = DomainPackReviewerRegistryAnchor::from_operator_protected_genesis(
        reviewer_registry.clone(),
        &digest("operator-trust"),
        &reviewer_full,
    )
    .unwrap();

    let signature_stubs = || {
        json!([
            {"reviewer_id":"principal.semantic","credential_id":"credential.semantic","role":"domain_expert","algorithm":"ed25519","payload_digest":digest("pending"),"signature":"00".repeat(64),"signed_at_unix":NOW},
            {"reviewer_id":"principal.authorizer","credential_id":"credential.authorizer","role":"registry_authorizer","algorithm":"ed25519","payload_digest":digest("pending"),"signature":"00".repeat(64),"signed_at_unix":NOW}
        ])
    };
    let mut current: DomainPackReviewedRegistryDocument = serde_json::from_value(json!({
        "schema_version":"0.3", "domain_pack_reviewed_registry": {
            "registry_id":"reviewed.learning", "audience":AUDIENCE, "generation":0,
            "previous_registry_digest":null, "entries":[], "snapshot_signatures":signature_stubs(),
            "registry_digest":digest("pending")
        }
    }))
    .unwrap();
    sign_snapshot(&mut current, [&semantic_key, &authorizer_key]);
    let current_digest = domain_pack_reviewed_registry_digest(&current).unwrap();

    let proposed_value = json!({
        "schema_version":"0.3", "domain_pack_reviewed_registry": {
            "registry_id":"reviewed.learning", "audience":AUDIENCE, "generation":1,
            "previous_registry_digest":current_digest, "entries":[{
                "pack":{"publisher":"publisher.acme","name":"safety","version":"1.0.0"},
                "package_digest":supply_digest("package"), "supply_chain_record_digest":supply_digest("record"),
                "manifest_digest":supply_digest("manifest"), "content_digest":supply_digest("content"),
                "license_digest":supply_digest("license"), "fixture_digests":[supply_digest("fixture-canonical")],
                "stage":"reviewed", "eligibility":"eligible_reviewed",
                "promotion_decision_digest":"", "authorization_digest":"",
                "independent_review_digests":[],
                "compatibility":{"forge_core_requirement":">=0.7.0","pack_schema_requirement":"^0.2","evaluator_protocol_versions":["1"],"predecessor_content_digests":[],"breaking_change":false,"migration_evidence_refs":[]},
                "deprecation":null,"revocation":null,"supersession":null,"entry_digest":""
            }], "snapshot_signatures":signature_stubs(), "registry_digest":""
        }
    });
    let mut proposed: DomainPackReviewedRegistryDocument =
        serde_json::from_value(proposed_value).unwrap();

    let mut candidate: DomainPackLocalLearningCandidateDocument =
        serde_json::from_value(json!({
            "schema_version":"0.3", "domain_pack_local_learning_candidate": {
                "candidate_id":"candidate.safety", "authority":"non_authoritative_observation",
                "target":{"pack":{"publisher":"publisher.acme","name":"safety"},"base_version":"1.0.0","contribution_ref":null,"proposed_namespace":"guidance.safety"},
                "assertion":"the safety guidance improves the exact evaluator outcome",
                "provenance":{"source_kind":"evaluator_observation","source_ref":"runs/safety.yaml","source_digest":digest("candidate-source"),"captured_by":"principal.capture","capture_run_id":"capture.safety","chat_transcript_ref":null},
                "evidence":[{"evidence_id":"evidence.candidate","kind":"evaluation_run","artifact":{"artifact_ref":"evidence/candidate.yaml","raw_sha256":supply_digest("candidate-raw"),"canonical_sha256":supply_digest("candidate-canonical")},"producer":"principal.evidence","produced_at_unix":1_400,"provenance_digest":digest("candidate-provenance")}],
                "observed_at_unix":1_400,"candidate_digest":digest("pending")
            }
        }))
        .unwrap();
    candidate
        .domain_pack_local_learning_candidate
        .candidate_digest = local_candidate_digest(&candidate);
    let candidate_digest = candidate
        .domain_pack_local_learning_candidate
        .candidate_digest
        .clone();

    let mut dossier: DomainPackPromotionDossierDocument = serde_json::from_value(json!({
        "schema_version":"0.3", "domain_pack_promotion_dossier": {
            "dossier_id":"dossier.safety", "authority":"candidate_only",
            "pack":{"publisher":"publisher.acme","name":"safety","version":"1.0.0"},
            "package_digest":supply_digest("package"),"manifest_digest":supply_digest("manifest"),"content_digest":supply_digest("content"),"license_digest":supply_digest("license"),
            "transition":{"from":"validated","to":"reviewed"}, "candidate_digests":[candidate_digest],
            "prior_promotion_record_digest":null,
            "evidence":[{"evidence_id":"evidence.ablation","kind":"ablation","artifact":{"artifact_ref":"evidence/ablation.yaml","raw_sha256":supply_digest("raw-evidence"),"canonical_sha256":supply_digest("canonical-evidence")},"producer":"principal.evidence","produced_at_unix":1_400,"provenance_digest":digest("provenance")}],
            "evaluator_runs":[{"run_id":"run.ablation","evaluator_ref":"evaluator.ablation","evaluator_principal":"principal.evaluator","evaluator_digest":digest("evaluator"),"fixture_set_digest":digest("fixtures"),"protocol_version":"1","comparison":{"method":"ablation","baseline_outcome_digest":digest("baseline"),"candidate_outcome_digest":digest("candidate-outcome"),"verdict":"improved","regression_finding_refs":[],"unknown_gap_refs":[],"rationale":"improved with no regression"},"strong_judge_proof":null,"evidence_ref":"evidence.ablation","run_digest":digest("run")}],
            "fixture_bindings":[{"fixture_id":"fixture.one","fixture_ref":"fixtures/one.yaml","producer":"principal.fixture","raw_sha256":supply_digest("fixture-raw"),"canonical_sha256":supply_digest("fixture-canonical"),"expected_outcome_digest":digest("expected"),"provenance_digest":digest("fixture-provenance")}],
            "provenance":{"authored_by":["principal.author"],"source_repository":"https://example.invalid/repo","source_revision":"abc123","source_tree_digest":digest("tree"),"build_recipe_digest":digest("build"),"generated_artifact_refs":[]},
            "conflict_record_digests":[],"open_gap_refs":[],"dossier_digest":digest("pending")
        }
    })).unwrap();
    dossier.domain_pack_promotion_dossier.dossier_digest =
        domain_pack_promotion_dossier_digest(&dossier).unwrap();
    let dossier_digest = dossier.domain_pack_promotion_dossier.dossier_digest.clone();
    let reviewer_digest = reviewer_registry
        .domain_pack_reviewer_registry
        .registry_digest
        .clone();
    let reviews = [
        ("review.semantic", "principal.semantic", "credential.semantic", "domain_expert"),
        ("review.authorizer", "principal.authorizer", "credential.authorizer", "registry_authorizer"),
    ].into_iter().map(|(id, principal, credential, role)| {
        let mut review: DomainPackIndependentReviewDocument = serde_json::from_value(json!({
            "schema_version":"0.3", "domain_pack_independent_review": {
                "review_id":id,"authority":"review_evidence_only","dossier_digest":dossier_digest,
                "reviewer_id":principal,"reviewer_role":role,"reviewer_registry_digest":reviewer_digest,
                "credential_id":credential,"independence":{"kind":"independent","attestation":"independent"},
                "decision":"approve","findings":[],"signed_subject_digest":dossier_digest,
                "issued_at_unix":1_400,"expires_at_unix":1_600,"review_digest":digest("pending")
            }
        })).unwrap();
        review.domain_pack_independent_review.review_digest = domain_pack_independent_review_digest(&review).unwrap();
        review
    }).collect::<Vec<_>>();
    let review_digests = reviews
        .iter()
        .map(|r| r.domain_pack_independent_review.review_digest.clone())
        .collect::<Vec<_>>();
    proposed.domain_pack_reviewed_registry.entries[0]
        .independent_review_digests
        .clone_from(&review_digests);
    let proposed_binding_digest = domain_pack_reviewed_registry_proposal_digest(&proposed).unwrap();
    let mut decision: DomainPackPromotionDecisionDocument = serde_json::from_value(json!({
        "schema_version":"0.3", "domain_pack_promotion_decision": {
            "decision_id":"decision.safety","authority":"candidate_decision_only","dossier_digest":dossier_digest,
            "transition":{"from":"validated","to":"reviewed"},"decision":"approve",
            "independent_review_digests":review_digests,"resolved_conflict_digests":[],
            "registry_predecessor_digest":current_digest,"proposed_registry_digest":proposed_binding_digest,
            "rationale":"approved exact promotion","decided_at_unix":NOW,"decision_digest":digest("pending")
        }
    })).unwrap();
    decision.domain_pack_promotion_decision.decision_digest =
        domain_pack_promotion_decision_digest(&decision).unwrap();
    let mut authorization: DomainPackPromotionAuthorizationDocument = serde_json::from_value(json!({
        "schema_version":"0.3", "domain_pack_promotion_authorization": {
            "authority":"candidate_authorization", "payload": {
                "authorization_id":"authorization.safety","dossier_digest":dossier_digest,
                "decision_digest":decision.domain_pack_promotion_decision.decision_digest,
                "independent_review_digests":review_digests,"reviewer_registry_digest":reviewer_digest,
                "current_reviewed_registry_digest":current_digest,"proposed_reviewed_registry_digest":proposed_binding_digest,
                "transition":{"from":"validated","to":"reviewed"},"audience":AUDIENCE,
                "domain":DOMAIN_PACK_PROMOTION_PAYLOAD_DOMAIN,"nonce":"nonce-1","issued_at_unix":1_400,"expires_at_unix":1_600
            }, "signatures":[
                {"reviewer_id":"principal.semantic","credential_id":"credential.semantic","role":"domain_expert","algorithm":"ed25519","payload_digest":digest("pending"),"signature":"00".repeat(64),"signed_at_unix":NOW},
                {"reviewer_id":"principal.authorizer","credential_id":"credential.authorizer","role":"registry_authorizer","algorithm":"ed25519","payload_digest":digest("pending"),"signature":"00".repeat(64),"signed_at_unix":NOW}
            ]
        }
    })).unwrap();
    let payload_digest = domain_pack_promotion_payload_digest(
        &authorization.domain_pack_promotion_authorization.payload,
    )
    .unwrap();
    for (index, signed) in authorization
        .domain_pack_promotion_authorization
        .signatures
        .iter_mut()
        .enumerate()
    {
        signed.payload_digest.clone_from(&payload_digest);
        let bytes = domain_pack_promotion_signing_bytes(
            &authorization.domain_pack_promotion_authorization.payload,
            signed,
        )
        .unwrap();
        signed.signature = hex(&[&semantic_key, &authorizer_key][index]
            .sign(&bytes)
            .to_bytes());
    }
    let promoted_entry = &mut proposed.domain_pack_reviewed_registry.entries[0];
    promoted_entry
        .promotion_decision_digest
        .clone_from(&decision.domain_pack_promotion_decision.decision_digest);
    promoted_entry
        .authorization_digest
        .clone_from(&payload_digest);
    promoted_entry.entry_digest =
        domain_pack_reviewed_registry_entry_digest(promoted_entry).unwrap();
    sign_snapshot(&mut proposed, [&semantic_key, &authorizer_key]);
    Fixture {
        semantic_key,
        authorizer_key,
        reviewer_registry,
        reviewer_anchor,
        current,
        proposed,
        dossier,
        candidates: vec![candidate],
        reviews,
        conflicts: Vec::new(),
        decision,
        authorization,
    }
}

fn resign_exact_graph(fixture: &mut Fixture) {
    fixture.dossier.domain_pack_promotion_dossier.dossier_digest =
        domain_pack_promotion_dossier_digest(&fixture.dossier).unwrap();
    let dossier_digest = fixture
        .dossier
        .domain_pack_promotion_dossier
        .dossier_digest
        .clone();
    let reviewer_registry_digest = fixture
        .reviewer_registry
        .domain_pack_reviewer_registry
        .registry_digest
        .clone();
    for review in &mut fixture.reviews {
        review
            .domain_pack_independent_review
            .dossier_digest
            .clone_from(&dossier_digest);
        review
            .domain_pack_independent_review
            .signed_subject_digest
            .clone_from(&dossier_digest);
        review
            .domain_pack_independent_review
            .reviewer_registry_digest
            .clone_from(&reviewer_registry_digest);
        review.domain_pack_independent_review.review_digest =
            domain_pack_independent_review_digest(review).unwrap();
    }
    let review_digests = fixture
        .reviews
        .iter()
        .map(|review| review.domain_pack_independent_review.review_digest.clone())
        .collect::<Vec<_>>();
    for entry in &mut fixture.proposed.domain_pack_reviewed_registry.entries {
        entry.independent_review_digests.clone_from(&review_digests);
    }
    let current_digest = domain_pack_reviewed_registry_digest(&fixture.current).unwrap();
    let proposed_binding_digest =
        domain_pack_reviewed_registry_proposal_digest(&fixture.proposed).unwrap();

    {
        let decision = &mut fixture.decision.domain_pack_promotion_decision;
        decision.dossier_digest.clone_from(&dossier_digest);
        decision.transition = fixture.dossier.domain_pack_promotion_dossier.transition;
        decision
            .independent_review_digests
            .clone_from(&review_digests);
        decision.resolved_conflict_digests.clone_from(
            &fixture
                .dossier
                .domain_pack_promotion_dossier
                .conflict_record_digests,
        );
        decision
            .registry_predecessor_digest
            .clone_from(&current_digest);
        decision
            .proposed_registry_digest
            .clone_from(&proposed_binding_digest);
    }
    fixture
        .decision
        .domain_pack_promotion_decision
        .decision_digest = domain_pack_promotion_decision_digest(&fixture.decision).unwrap();
    let decision = &fixture.decision.domain_pack_promotion_decision;

    let authorization = &mut fixture.authorization.domain_pack_promotion_authorization;
    authorization.payload.dossier_digest = dossier_digest;
    authorization.payload.decision_digest = decision.decision_digest.clone();
    authorization.payload.independent_review_digests = review_digests;
    authorization.payload.reviewer_registry_digest = reviewer_registry_digest;
    authorization.payload.current_reviewed_registry_digest = current_digest;
    authorization.payload.proposed_reviewed_registry_digest = proposed_binding_digest;
    authorization.payload.transition = decision.transition;
    let payload_digest = domain_pack_promotion_payload_digest(&authorization.payload).unwrap();
    for (index, signed) in authorization.signatures.iter_mut().enumerate() {
        signed.payload_digest.clone_from(&payload_digest);
        let bytes = domain_pack_promotion_signing_bytes(&authorization.payload, signed).unwrap();
        signed.signature = hex(&[&fixture.semantic_key, &fixture.authorizer_key][index]
            .sign(&bytes)
            .to_bytes());
    }
    for entry in &mut fixture.proposed.domain_pack_reviewed_registry.entries {
        entry
            .promotion_decision_digest
            .clone_from(&decision.decision_digest);
        entry.authorization_digest.clone_from(&payload_digest);
        entry.entry_digest = domain_pack_reviewed_registry_entry_digest(entry).unwrap();
    }
    sign_snapshot(
        &mut fixture.proposed,
        [&fixture.semantic_key, &fixture.authorizer_key],
    );
}

fn verify(
    fixture: &Fixture,
) -> Result<
    forge_core_authority::VerifiedDomainPackPromotionAuthorization,
    DomainPackPromotionAuthorityError,
> {
    verify_domain_pack_promotion_authorization(
        &fixture.reviewer_anchor,
        &fixture.authorization,
        DomainPackPromotionExpectedContext {
            dossier: &fixture.dossier,
            candidates: &fixture.candidates,
            decision: &fixture.decision,
            independent_reviews: &fixture.reviews,
            conflicts: &fixture.conflicts,
            current_reviewed_registry: &fixture.current,
            proposed_reviewed_registry: &fixture.proposed,
            verified_at_unix: NOW,
        },
        AUDIENCE,
    )
}

fn attach_open_conflict(fixture: &mut Fixture) {
    let candidate_digest = fixture.candidates[0]
        .domain_pack_local_learning_candidate
        .candidate_digest
        .clone();
    let mut conflict: DomainPackLearningConflictDocument =
        serde_json::from_value(json!({
            "schema_version":"0.3", "domain_pack_learning_conflict": {
                "conflict_id":"conflict.open", "authority":"conflict_evidence_only",
                "target":{"pack":{"publisher":"publisher.acme","name":"safety"},"base_version":"1.0.0","contribution_ref":null,"proposed_namespace":"guidance.safety"},
                "kind":"evaluator_disagreement", "subject_digests":[candidate_digest],
                "evidence_refs":[], "status":"open", "review_request_digest":digest("review-request"),
                "resolution":null, "conflict_digest":digest("pending")
            }
        }))
        .unwrap();
    conflict.domain_pack_learning_conflict.conflict_digest =
        domain_pack_learning_conflict_digest(&conflict).unwrap();
    fixture
        .dossier
        .domain_pack_promotion_dossier
        .conflict_record_digests = vec![conflict
        .domain_pack_learning_conflict
        .conflict_digest
        .clone()];
    fixture.conflicts.push(conflict);
}

#[test]
fn exact_dual_review_mints_only_opaque_authority_and_advances_registry() {
    let fixture = fixture();
    let capability = verify(&fixture).expect("exact promotion authorization");
    assert_eq!(
        capability.audit().authority,
        DomainPackPromotionAuditAuthority::NonAuthoritative
    );
    let current_digest = domain_pack_reviewed_registry_digest(&fixture.current).unwrap();
    let mut anchor = ReviewedDomainPackRegistryAnchor::from_operator_protected_head(
        &fixture.reviewer_anchor,
        fixture.current.clone(),
        &current_digest,
        NOW,
    )
    .unwrap();
    let version = anchor.version();
    let anchored = anchor
        .compare_and_advance(&version, &fixture.reviewer_anchor, capability, NOW)
        .unwrap();
    assert_eq!(
        anchored.registry().domain_pack_reviewed_registry.generation,
        1
    );
    assert!(anchored.authorization_audit().is_some());
}

#[test]
fn fully_resigned_dossier_a_registry_entry_b_transplant_cannot_authorize() {
    let mut fixture = fixture();
    let entry = &mut fixture.proposed.domain_pack_reviewed_registry.entries[0];
    entry.pack.name.0 = "unrelated-pack".to_owned();
    entry.package_digest = supply_digest("package-b");
    entry.manifest_digest = supply_digest("manifest-b");
    entry.content_digest = supply_digest("content-b");
    entry.license_digest = supply_digest("license-b");
    entry.fixture_digests = vec![supply_digest("fixture-b")];
    resign_exact_graph(&mut fixture);
    assert!(matches!(
        verify(&fixture),
        Err(DomainPackPromotionAuthorityError::ReviewedRegistryEvolution { .. })
    ));
}

#[test]
fn resigned_registry_provenance_backlink_transplant_cannot_authorize() {
    let mut fixture = fixture();
    let entry = &mut fixture.proposed.domain_pack_reviewed_registry.entries[0];
    entry.promotion_decision_digest = digest("decision-from-other-dossier");
    entry.authorization_digest = digest("authorization-from-other-dossier");
    entry.entry_digest = domain_pack_reviewed_registry_entry_digest(entry).unwrap();
    sign_snapshot(
        &mut fixture.proposed,
        [&fixture.semantic_key, &fixture.authorizer_key],
    );
    assert!(matches!(
        verify(&fixture),
        Err(DomainPackPromotionAuthorityError::ReviewedRegistryEvolution { .. })
    ));
}

#[test]
fn fully_resigned_regression_graph_cannot_authorize() {
    let mut fixture = fixture();
    fixture.dossier.domain_pack_promotion_dossier.evaluator_runs[0]
        .comparison
        .verdict =
        forge_core_contracts::domain_pack_learning::DomainPackLearningComparisonVerdict::Regressed;
    resign_exact_graph(&mut fixture);
    assert!(matches!(
        verify(&fixture),
        Err(DomainPackPromotionAuthorityError::BlockingDecision)
    ));
}

#[test]
fn fully_resigned_unresolved_conflict_graph_cannot_authorize() {
    let mut fixture = fixture();
    attach_open_conflict(&mut fixture);
    resign_exact_graph(&mut fixture);
    assert!(matches!(
        verify(&fixture),
        Err(DomainPackPromotionAuthorityError::BlockingDecision)
    ));
}

#[test]
fn fully_resigned_open_to_resolved_mutation_cannot_reuse_conflict_digest() {
    let mut fixture = fixture();
    attach_open_conflict(&mut fixture);
    let review_digests = fixture
        .reviews
        .iter()
        .map(|review| review.domain_pack_independent_review.review_digest.clone())
        .collect::<Vec<_>>();
    let conflict = &mut fixture.conflicts[0].domain_pack_learning_conflict;
    conflict.status =
        forge_core_contracts::domain_pack_learning::DomainPackLearningConflictStatus::Resolved;
    conflict.evidence_refs = vec![forge_core_contracts::StableId(
        "evidence.resolution".to_owned(),
    )];
    conflict.resolution = Some(
        forge_core_contracts::domain_pack_learning::DomainPackLearningConflictResolution {
            decision: forge_core_contracts::domain_pack_learning::DomainPackLearningConflictResolutionDecision::PreferCandidate,
            rationale: "resolved with exact independent evidence".to_owned(),
            evidence_refs: conflict.evidence_refs.clone(),
            resolved_by_review_digests: review_digests,
        },
    );
    resign_exact_graph(&mut fixture);
    assert!(matches!(
        verify(&fixture),
        Err(DomainPackPromotionAuthorityError::InvalidContract { .. }
            | DomainPackPromotionAuthorityError::BindingMismatch { .. })
    ));
}

#[test]
fn correlated_reviewers_and_participant_overlap_fail_closed() {
    let mut fixture = fixture();
    fixture
        .dossier
        .domain_pack_promotion_dossier
        .provenance
        .authored_by = vec![forge_core_contracts::PrincipalId(
        "principal.semantic".into(),
    )];
    fixture.dossier.domain_pack_promotion_dossier.dossier_digest =
        domain_pack_promotion_dossier_digest(&fixture.dossier).unwrap();
    assert!(verify(&fixture).is_err());
}

#[test]
fn exact_head_snapshot_replay_is_freshly_reverified() {
    let mut fixture = fixture();
    let digest = domain_pack_reviewed_registry_digest(&fixture.current).unwrap();
    let mut anchor = ReviewedDomainPackRegistryAnchor::from_operator_protected_head(
        &fixture.reviewer_anchor,
        fixture.current.clone(),
        &digest,
        NOW,
    )
    .unwrap();
    let replay = anchor
        .verify_exact_replay(&fixture.reviewer_anchor, fixture.current.clone(), NOW)
        .unwrap();
    assert!(replay.authorization_audit().is_none());
    fixture
        .current
        .domain_pack_reviewed_registry
        .snapshot_signatures[0]
        .signature = "11".repeat(64);
    assert!(anchor
        .verify_exact_replay(&fixture.reviewer_anchor, fixture.current, NOW)
        .is_err());
}

#[test]
fn replacement_agent_restores_rotated_reviewer_head_and_continues_chain() {
    let mut fixture = fixture();
    let first = reviewer_successor(
        &fixture.reviewer_registry,
        [&fixture.semantic_key, &fixture.authorizer_key],
    );
    let version = fixture.reviewer_anchor.version();
    fixture
        .reviewer_anchor
        .compare_and_advance(&version, first.clone(), NOW)
        .unwrap();

    let first_full = full_digest(&first);
    let mut restored = DomainPackReviewerRegistryAnchor::from_operator_protected_head(
        first.clone(),
        &digest("operator-trust"),
        &first_full,
    )
    .unwrap();
    let second = reviewer_successor(&first, [&fixture.semantic_key, &fixture.authorizer_key]);
    let restored_version = restored.version();
    let audit = restored
        .compare_and_advance(&restored_version, second, NOW)
        .unwrap();
    assert_eq!(audit.generation, 2);
}

#[test]
fn reviewer_registry_cannot_downgrade_threshold_for_one_signer_takeover() {
    let mut fixture = fixture();
    let mut takeover = reviewer_successor(
        &fixture.reviewer_registry,
        [&fixture.semantic_key, &fixture.authorizer_key],
    );
    takeover.domain_pack_reviewer_registry.signature_threshold = 1;
    takeover
        .domain_pack_reviewer_registry
        .rotation_signatures
        .truncate(1);
    let takeover_digest = domain_pack_reviewer_registry_digest(&takeover).unwrap();
    takeover
        .domain_pack_reviewer_registry
        .registry_digest
        .clone_from(&takeover_digest);
    takeover.domain_pack_reviewer_registry.rotation_signatures[0]
        .payload_digest
        .clone_from(&takeover_digest);
    let signed = takeover.domain_pack_reviewer_registry.rotation_signatures[0].clone();
    let bytes = domain_pack_reviewer_registry_rotation_signing_bytes(&takeover, &signed).unwrap();
    takeover.domain_pack_reviewer_registry.rotation_signatures[0].signature =
        hex(&fixture.semantic_key.sign(&bytes).to_bytes());
    let version = fixture.reviewer_anchor.version();
    assert!(matches!(
        fixture
            .reviewer_anchor
            .compare_and_advance(&version, takeover, NOW),
        Err(DomainPackPromotionAuthorityError::ReviewerRegistryThresholdNotMet)
    ));
}

#[test]
fn one_multi_role_principal_cannot_satisfy_both_rotation_roles() {
    let fixture = fixture();
    let mut predecessor = fixture.reviewer_registry.clone();
    predecessor
        .domain_pack_reviewer_registry
        .signature_threshold = 1;
    predecessor
        .domain_pack_reviewer_registry
        .reviewers
        .truncate(1);
    predecessor.domain_pack_reviewer_registry.reviewers[0]
        .roles
        .push(
            forge_core_contracts::domain_pack_learning::DomainPackReviewerRole::RegistryAuthorizer,
        );
    predecessor
        .domain_pack_reviewer_registry
        .rotation_signatures
        .truncate(1);
    predecessor.domain_pack_reviewer_registry.registry_digest =
        domain_pack_reviewer_registry_digest(&predecessor).unwrap();
    let predecessor_full = full_digest(&predecessor);
    let mut anchor = DomainPackReviewerRegistryAnchor::from_operator_protected_genesis(
        predecessor.clone(),
        &digest("operator-trust"),
        &predecessor_full,
    )
    .unwrap();

    let predecessor_digest = domain_pack_reviewer_registry_digest(&predecessor).unwrap();
    let mut successor = predecessor;
    successor.domain_pack_reviewer_registry.generation = 1;
    successor
        .domain_pack_reviewer_registry
        .previous_registry_digest = Some(predecessor_digest.clone());
    let signed = &mut successor.domain_pack_reviewer_registry.rotation_signatures[0];
    signed.predecessor_registry_digest = Some(predecessor_digest);
    signed.signed_at_unix = NOW;
    let successor_digest = domain_pack_reviewer_registry_digest(&successor).unwrap();
    successor
        .domain_pack_reviewer_registry
        .registry_digest
        .clone_from(&successor_digest);
    successor.domain_pack_reviewer_registry.rotation_signatures[0]
        .payload_digest
        .clone_from(&successor_digest);
    let signed = successor.domain_pack_reviewer_registry.rotation_signatures[0].clone();
    let bytes = domain_pack_reviewer_registry_rotation_signing_bytes(&successor, &signed).unwrap();
    successor.domain_pack_reviewer_registry.rotation_signatures[0].signature =
        hex(&fixture.semantic_key.sign(&bytes).to_bytes());
    let version = anchor.version();
    assert!(matches!(
        anchor.compare_and_advance(&version, successor, NOW),
        Err(DomainPackPromotionAuthorityError::ReviewerRegistryThresholdNotMet)
    ));
}

#[test]
fn capability_cannot_cross_reviewer_head_or_outlive_authorization() {
    let mut rotated_fixture = fixture();
    let capability = verify(&rotated_fixture).unwrap();
    let successor = reviewer_successor(
        &rotated_fixture.reviewer_registry,
        [
            &rotated_fixture.semantic_key,
            &rotated_fixture.authorizer_key,
        ],
    );
    let reviewer_version = rotated_fixture.reviewer_anchor.version();
    rotated_fixture
        .reviewer_anchor
        .compare_and_advance(&reviewer_version, successor, NOW)
        .unwrap();
    let current_digest = domain_pack_reviewed_registry_digest(&rotated_fixture.current).unwrap();
    let mut reviewed_anchor = ReviewedDomainPackRegistryAnchor::from_operator_protected_head(
        &rotated_fixture.reviewer_anchor,
        rotated_fixture.current.clone(),
        &current_digest,
        NOW,
    )
    .unwrap();
    let registry_anchor_version = reviewed_anchor.version();
    assert!(matches!(
        reviewed_anchor.compare_and_advance(
            &registry_anchor_version,
            &rotated_fixture.reviewer_anchor,
            capability,
            NOW,
        ),
        Err(DomainPackPromotionAuthorityError::BindingMismatch {
            field: "capability.reviewer_registry_digest"
        })
    ));

    let expired_fixture = fixture();
    let expired_capability = verify(&expired_fixture).unwrap();
    let mut reviewed_anchor = ReviewedDomainPackRegistryAnchor::from_operator_protected_head(
        &expired_fixture.reviewer_anchor,
        expired_fixture.current.clone(),
        &current_digest,
        NOW,
    )
    .unwrap();
    let version = reviewed_anchor.version();
    assert!(matches!(
        reviewed_anchor.compare_and_advance(
            &version,
            &expired_fixture.reviewer_anchor,
            expired_capability,
            1_601,
        ),
        Err(DomainPackPromotionAuthorityError::AuthorizationExpired)
    ));
}
