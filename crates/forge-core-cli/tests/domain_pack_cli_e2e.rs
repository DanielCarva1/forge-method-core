use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    domain_pack_independent_review_digest, domain_pack_package_record_digest,
    domain_pack_promotion_decision_digest, domain_pack_promotion_dossier_digest,
    domain_pack_promotion_payload_digest, domain_pack_promotion_reviewer_key_fingerprint,
    domain_pack_promotion_signing_bytes, domain_pack_publisher_signing_bytes,
    domain_pack_registry_signing_bytes, domain_pack_registry_snapshot_digest,
    domain_pack_reviewed_registry_digest, domain_pack_reviewed_registry_entry_digest,
    domain_pack_reviewed_registry_proposal_digest, domain_pack_reviewed_registry_signing_bytes,
    domain_pack_reviewer_registry_digest, domain_pack_reviewer_registry_rotation_signing_bytes,
    DOMAIN_PACK_PROMOTION_PAYLOAD_DOMAIN,
};
use forge_core_contracts::*;
use forge_core_decisions::MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES;
use forge_core_domain_pack_learning_store::candidate_self_digest;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

#[path = "domain_pack_cli_e2e/p6d_workflow_journey.rs"]
mod p6d_workflow_journey;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("create fixture destination");
    for entry in fs::read_dir(source).expect("read fixture source") {
        let entry = entry.expect("fixture entry");
        let destination = target.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).expect("copy fixture");
        }
    }
}

fn snapshot(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, current: &Path, output: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(current).expect("read snapshot tree") {
            let entry = entry.expect("snapshot entry");
            if entry.path().is_dir() {
                walk(root, &entry.path(), output);
            } else {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .expect("relative fixture")
                    .to_string_lossy()
                    .replace('\\', "/");
                let bytes = fs::read(entry.path()).expect("snapshot bytes");
                output.insert(relative, format!("{:x}", Sha256::digest(bytes)));
            }
        }
    }
    let mut output = BTreeMap::new();
    walk(root, root, &mut output);
    output
}

fn fresh_temp(label: &str) -> PathBuf {
    let temp = std::env::temp_dir().join(format!(
        "forge-domain-pack-cli-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("clean stale temp");
    }
    fs::create_dir_all(&temp).expect("create temp root");
    temp
}

fn write_oversized(path: &Path) {
    let file = fs::File::create(path).expect("create oversized input");
    file.set_len((MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES as u64) + 1)
        .expect("size oversized input");
}

#[allow(clippy::too_many_lines)] // Cohesive cryptographic graph sealed from one clock and key set.
fn write_signed_learning_roots(operator_root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let semantic_key = SigningKey::from_bytes(&[71_u8; 32]);
    let authorizer_key = SigningKey::from_bytes(&[72_u8; 32]);
    let reviewer_json = |id: &str, credential: &str, role: &str, domain: &str, key: &SigningKey| {
        serde_json::json!({
            "reviewer_id": id,
            "credential_id": credential,
            "public_key_hex": hex(key.verifying_key().as_bytes()),
            "public_key_fingerprint": domain_pack_promotion_reviewer_key_fingerprint(key.verifying_key().as_bytes()),
            "algorithm": "ed25519",
            "roles": [role],
            "independence_domains": [domain],
            "status": "active",
            "valid_from_unix": now.saturating_sub(60),
            "valid_until_unix": now + 3_600
        })
    };
    let mut reviewers: DomainPackReviewerRegistryDocument = serde_json::from_value(
        serde_json::json!({
            "schema_version": "0.3",
            "domain_pack_reviewer_registry": {
                "registry_id": "reviewers.cli.e2e",
                "audience": "forge-domain-pack-runtime",
                "generation": 0,
                "previous_registry_digest": null,
                "trust_policy_digest": format!("{:x}", Sha256::digest(b"cli-learning-trust")),
                "signature_threshold": 2,
                "reviewers": [
                    reviewer_json("principal.semantic", "credential.semantic", "domain_expert", "domain.semantic", &semantic_key),
                    reviewer_json("principal.authorizer", "credential.authorizer", "registry_authorizer", "domain.registry", &authorizer_key)
                ],
                "rotation_signatures": [
                    {"signer_id":"principal.semantic","credential_id":"credential.semantic","predecessor_registry_digest":null,"payload_digest":"0".repeat(64),"algorithm":"ed25519","signature":"00","signed_at_unix":now},
                    {"signer_id":"principal.authorizer","credential_id":"credential.authorizer","predecessor_registry_digest":null,"payload_digest":"1".repeat(64),"algorithm":"ed25519","signature":"11","signed_at_unix":now}
                ],
                "registry_digest": "2".repeat(64)
            }
        }),
    )
    .expect("reviewer registry");
    reviewers.domain_pack_reviewer_registry.registry_digest =
        domain_pack_reviewer_registry_digest(&reviewers).expect("reviewer digest");
    let predecessor_digest = reviewers
        .domain_pack_reviewer_registry
        .registry_digest
        .clone();
    let mut successor = reviewers.clone();
    successor.domain_pack_reviewer_registry.generation = 1;
    successor
        .domain_pack_reviewer_registry
        .previous_registry_digest = Some(predecessor_digest.clone());
    for signature in &mut successor.domain_pack_reviewer_registry.rotation_signatures {
        signature.predecessor_registry_digest = Some(predecessor_digest.clone());
        signature.payload_digest = "6".repeat(64);
        signature.signature = "00".repeat(64);
        signature.signed_at_unix = now;
    }
    let successor_digest =
        domain_pack_reviewer_registry_digest(&successor).expect("successor digest");
    successor
        .domain_pack_reviewer_registry
        .registry_digest
        .clone_from(&successor_digest);
    for (index, key) in [&semantic_key, &authorizer_key].into_iter().enumerate() {
        successor.domain_pack_reviewer_registry.rotation_signatures[index]
            .payload_digest
            .clone_from(&successor_digest);
        let signed = successor.domain_pack_reviewer_registry.rotation_signatures[index].clone();
        let bytes = domain_pack_reviewer_registry_rotation_signing_bytes(&successor, &signed)
            .expect("reviewer rotation signing bytes");
        successor.domain_pack_reviewer_registry.rotation_signatures[index].signature =
            hex(&key.sign(&bytes).to_bytes());
    }

    let signature_stubs = serde_json::json!([
        {"reviewer_id":"principal.semantic","credential_id":"credential.semantic","role":"domain_expert","algorithm":"ed25519","payload_digest":"3".repeat(64),"signature":"00".repeat(64),"signed_at_unix":now},
        {"reviewer_id":"principal.authorizer","credential_id":"credential.authorizer","role":"registry_authorizer","algorithm":"ed25519","payload_digest":"4".repeat(64),"signature":"00".repeat(64),"signed_at_unix":now}
    ]);
    let mut reviewed: DomainPackReviewedRegistryDocument =
        serde_json::from_value(serde_json::json!({
            "schema_version": "0.3",
            "domain_pack_reviewed_registry": {
                "registry_id": "reviewed.cli.e2e",
                "audience": "forge-domain-pack-runtime",
                "generation": 0,
                "previous_registry_digest": null,
                "entries": [],
                "snapshot_signatures": signature_stubs,
                "registry_digest": "5".repeat(64)
            }
        }))
        .expect("reviewed registry");
    let snapshot_digest = domain_pack_reviewed_registry_digest(&reviewed).expect("reviewed digest");
    reviewed
        .domain_pack_reviewed_registry
        .registry_digest
        .clone_from(&snapshot_digest);
    for (index, key) in [&semantic_key, &authorizer_key].into_iter().enumerate() {
        reviewed.domain_pack_reviewed_registry.snapshot_signatures[index]
            .payload_digest
            .clone_from(&snapshot_digest);
        let signed = reviewed.domain_pack_reviewed_registry.snapshot_signatures[index].clone();
        let bytes = domain_pack_reviewed_registry_signing_bytes(&reviewed, &signed)
            .expect("reviewed signing bytes");
        reviewed.domain_pack_reviewed_registry.snapshot_signatures[index].signature =
            hex(&key.sign(&bytes).to_bytes());
    }

    let reviewer_path = operator_root.join("reviewers.yaml");
    let reviewed_path = operator_root.join("reviewed.yaml");
    let successor_path = operator_root.join("reviewers-next.yaml");
    fs::write(
        &reviewer_path,
        yaml_serde::to_string(&reviewers).expect("reviewers yaml"),
    )
    .expect("write reviewers");
    fs::write(
        &successor_path,
        yaml_serde::to_string(&successor).expect("successor yaml"),
    )
    .expect("write successor");
    fs::write(
        &reviewed_path,
        yaml_serde::to_string(&reviewed).expect("reviewed yaml"),
    )
    .expect("write reviewed");
    (reviewer_path, reviewed_path, successor_path)
}

struct PromotionGraphPaths {
    proposed: PathBuf,
    candidate: PathBuf,
    dossier: PathBuf,
    reviews: [PathBuf; 2],
    decision: PathBuf,
    authorization: PathBuf,
}

fn learning_hash(label: &str) -> String {
    format!("{:x}", Sha256::digest(label.as_bytes()))
}

fn supply_hash(label: &str) -> String {
    format!("sha256:{}", learning_hash(label))
}

fn write_typed_yaml<T: Serialize>(root: &Path, name: &str, value: &T) -> PathBuf {
    let path = root.join(name);
    fs::write(
        &path,
        yaml_serde::to_string(value).expect("typed fixture YAML"),
    )
    .expect("write typed fixture");
    path
}

#[allow(clippy::too_many_lines)] // One cryptographically sealed promotion graph avoids fake CLI authority.
fn write_promotable_learning_graph(
    operator_root: &Path,
    reviewer_path: &Path,
    current_path: &Path,
) -> PromotionGraphPaths {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let issued = now.saturating_sub(60);
    let expires = now + 3_600;
    let semantic_key = SigningKey::from_bytes(&[71_u8; 32]);
    let authorizer_key = SigningKey::from_bytes(&[72_u8; 32]);
    let reviewer_registry: DomainPackReviewerRegistryDocument = yaml_serde::from_str(
        &fs::read_to_string(reviewer_path).expect("reviewer registry fixture"),
    )
    .expect("typed reviewer registry");
    let current: DomainPackReviewedRegistryDocument =
        yaml_serde::from_str(&fs::read_to_string(current_path).expect("reviewed registry fixture"))
            .expect("typed reviewed registry");
    let current_digest = current
        .domain_pack_reviewed_registry
        .registry_digest
        .clone();
    let signature_stubs = || {
        serde_json::json!([
            {"reviewer_id":"principal.semantic","credential_id":"credential.semantic","role":"domain_expert","algorithm":"ed25519","payload_digest":learning_hash("pending"),"signature":"00".repeat(64),"signed_at_unix":now},
            {"reviewer_id":"principal.authorizer","credential_id":"credential.authorizer","role":"registry_authorizer","algorithm":"ed25519","payload_digest":learning_hash("pending"),"signature":"00".repeat(64),"signed_at_unix":now}
        ])
    };
    let proposed_value = serde_json::json!({
        "schema_version":"0.3", "domain_pack_reviewed_registry": {
            "registry_id":"reviewed.cli.e2e", "audience":"forge-domain-pack-runtime", "generation":1,
            "previous_registry_digest":current_digest, "entries":[{
                "pack":{"publisher":"publisher.acme","name":"safety","version":"1.0.0"},
                "package_digest":supply_hash("package"), "supply_chain_record_digest":supply_hash("record"),
                "manifest_digest":supply_hash("manifest"), "content_digest":supply_hash("content"),
                "license_digest":supply_hash("license"), "fixture_digests":[supply_hash("fixture-canonical")],
                "stage":"reviewed", "eligibility":"eligible_reviewed",
                "promotion_decision_digest":"", "authorization_digest":"",
                "independent_review_digests":[],
                "compatibility":{"forge_core_requirement":">=0.7.0","pack_schema_requirement":"^0.2","evaluator_protocol_versions":["1"],"predecessor_content_digests":[],"breaking_change":false,"migration_evidence_refs":[]},
                "deprecation":null,"revocation":null,"supersession":null,"entry_digest":""
            }], "snapshot_signatures":signature_stubs(), "registry_digest":""
        }
    });
    let mut proposed: DomainPackReviewedRegistryDocument =
        serde_json::from_value(proposed_value).expect("reviewed successor");

    let mut candidate: DomainPackLocalLearningCandidateDocument = serde_json::from_value(
        serde_json::json!({
            "schema_version":"0.3", "domain_pack_local_learning_candidate": {
                "candidate_id":"candidate.safety", "authority":"non_authoritative_observation",
                "target":{"pack":{"publisher":"publisher.acme","name":"safety"},"base_version":"1.0.0","contribution_ref":null,"proposed_namespace":"guidance.safety"},
                "assertion":"the safety guidance improves the exact evaluator outcome",
                "provenance":{"source_kind":"evaluator_observation","source_ref":"runs/safety.yaml","source_digest":learning_hash("candidate-source"),"captured_by":"principal.capture","capture_run_id":"capture.safety","chat_transcript_ref":null},
                "evidence":[{"evidence_id":"evidence.candidate","kind":"evaluation_run","artifact":{"artifact_ref":"evidence/candidate.yaml","raw_sha256":supply_hash("candidate-raw"),"canonical_sha256":supply_hash("candidate-canonical")},"producer":"principal.evidence","produced_at_unix":issued,"provenance_digest":learning_hash("candidate-provenance")}],
                "observed_at_unix":issued,"candidate_digest":learning_hash("pending")
            }
        }),
    )
    .expect("local candidate");
    candidate
        .domain_pack_local_learning_candidate
        .candidate_digest = candidate_self_digest(&candidate).expect("candidate self digest");
    let candidate_digest = candidate
        .domain_pack_local_learning_candidate
        .candidate_digest
        .clone();

    let mut dossier: DomainPackPromotionDossierDocument = serde_json::from_value(
        serde_json::json!({
            "schema_version":"0.3", "domain_pack_promotion_dossier": {
                "dossier_id":"dossier.safety", "authority":"candidate_only",
                "pack":{"publisher":"publisher.acme","name":"safety","version":"1.0.0"},
                "package_digest":supply_hash("package"),"manifest_digest":supply_hash("manifest"),"content_digest":supply_hash("content"),"license_digest":supply_hash("license"),
                "transition":{"from":"validated","to":"reviewed"}, "candidate_digests":[candidate_digest],
                "prior_promotion_record_digest":null,
                "evidence":[{"evidence_id":"evidence.ablation","kind":"ablation","artifact":{"artifact_ref":"evidence/ablation.yaml","raw_sha256":supply_hash("raw-evidence"),"canonical_sha256":supply_hash("canonical-evidence")},"producer":"principal.evidence","produced_at_unix":issued,"provenance_digest":learning_hash("provenance")}],
                "evaluator_runs":[{"run_id":"run.ablation","evaluator_ref":"evaluator.ablation","evaluator_principal":"principal.evaluator","evaluator_digest":learning_hash("evaluator"),"fixture_set_digest":learning_hash("fixtures"),"protocol_version":"1","comparison":{"method":"ablation","baseline_outcome_digest":learning_hash("baseline"),"candidate_outcome_digest":learning_hash("candidate-outcome"),"verdict":"improved","regression_finding_refs":[],"unknown_gap_refs":[],"rationale":"improved with no regression"},"strong_judge_proof":null,"evidence_ref":"evidence.ablation","run_digest":learning_hash("run")}],
                "fixture_bindings":[{"fixture_id":"fixture.one","fixture_ref":"fixtures/one.yaml","producer":"principal.fixture","raw_sha256":supply_hash("fixture-raw"),"canonical_sha256":supply_hash("fixture-canonical"),"expected_outcome_digest":learning_hash("expected"),"provenance_digest":learning_hash("fixture-provenance")}],
                "provenance":{"authored_by":["principal.author"],"source_repository":"https://example.invalid/repo","source_revision":"abc123","source_tree_digest":learning_hash("tree"),"build_recipe_digest":learning_hash("build"),"generated_artifact_refs":[]},
                "conflict_record_digests":[],"open_gap_refs":[],"dossier_digest":learning_hash("pending")
            }
        }),
    )
    .expect("promotion dossier");
    dossier.domain_pack_promotion_dossier.dossier_digest =
        domain_pack_promotion_dossier_digest(&dossier).expect("dossier digest");
    let dossier_digest = dossier.domain_pack_promotion_dossier.dossier_digest.clone();
    let reviewer_digest = reviewer_registry
        .domain_pack_reviewer_registry
        .registry_digest
        .clone();
    let reviews = [
        (
            "review.semantic",
            "principal.semantic",
            "credential.semantic",
            "domain_expert",
        ),
        (
            "review.authorizer",
            "principal.authorizer",
            "credential.authorizer",
            "registry_authorizer",
        ),
    ]
    .into_iter()
    .map(|(id, principal, credential, role)| {
        let mut review: DomainPackIndependentReviewDocument = serde_json::from_value(
            serde_json::json!({
                "schema_version":"0.3", "domain_pack_independent_review": {
                    "review_id":id,"authority":"review_evidence_only","dossier_digest":dossier_digest,
                    "reviewer_id":principal,"reviewer_role":role,"reviewer_registry_digest":reviewer_digest,
                    "credential_id":credential,"independence":{"kind":"independent","attestation":"independent"},
                    "decision":"approve","findings":[],"signed_subject_digest":dossier_digest,
                    "issued_at_unix":issued,"expires_at_unix":expires,"review_digest":learning_hash("pending")
                }
            }),
        )
        .expect("independent review");
        review.domain_pack_independent_review.review_digest =
            domain_pack_independent_review_digest(&review).expect("review digest");
        review
    })
    .collect::<Vec<_>>();
    let review_digests = reviews
        .iter()
        .map(|review| review.domain_pack_independent_review.review_digest.clone())
        .collect::<Vec<_>>();
    proposed.domain_pack_reviewed_registry.entries[0]
        .independent_review_digests
        .clone_from(&review_digests);
    let proposed_binding_digest =
        domain_pack_reviewed_registry_proposal_digest(&proposed).expect("reviewed proposal digest");
    let mut decision: DomainPackPromotionDecisionDocument = serde_json::from_value(
        serde_json::json!({
            "schema_version":"0.3", "domain_pack_promotion_decision": {
                "decision_id":"decision.safety","authority":"candidate_decision_only","dossier_digest":dossier_digest,
                "transition":{"from":"validated","to":"reviewed"},"decision":"approve",
                "independent_review_digests":review_digests,"resolved_conflict_digests":[],
                "registry_predecessor_digest":current_digest,"proposed_registry_digest":proposed_binding_digest,
                "rationale":"approved exact promotion","decided_at_unix":now,"decision_digest":learning_hash("pending")
            }
        }),
    )
    .expect("promotion decision");
    decision.domain_pack_promotion_decision.decision_digest =
        domain_pack_promotion_decision_digest(&decision).expect("decision digest");
    let mut authorization: DomainPackPromotionAuthorizationDocument = serde_json::from_value(
        serde_json::json!({
            "schema_version":"0.3", "domain_pack_promotion_authorization": {
                "authority":"candidate_authorization", "payload": {
                    "authorization_id":"authorization.safety","dossier_digest":dossier_digest,
                    "decision_digest":decision.domain_pack_promotion_decision.decision_digest,
                    "independent_review_digests":review_digests,"reviewer_registry_digest":reviewer_digest,
                    "current_reviewed_registry_digest":current_digest,"proposed_reviewed_registry_digest":proposed_binding_digest,
                    "transition":{"from":"validated","to":"reviewed"},"audience":"forge-domain-pack-runtime",
                    "domain":DOMAIN_PACK_PROMOTION_PAYLOAD_DOMAIN,"nonce":"nonce-cli-e2e","issued_at_unix":issued,"expires_at_unix":expires
                }, "signatures":[
                    {"reviewer_id":"principal.semantic","credential_id":"credential.semantic","role":"domain_expert","algorithm":"ed25519","payload_digest":learning_hash("pending"),"signature":"00".repeat(64),"signed_at_unix":now},
                    {"reviewer_id":"principal.authorizer","credential_id":"credential.authorizer","role":"registry_authorizer","algorithm":"ed25519","payload_digest":learning_hash("pending"),"signature":"00".repeat(64),"signed_at_unix":now}
                ]
            }
        }),
    )
    .expect("promotion authorization");
    let payload_digest = domain_pack_promotion_payload_digest(
        &authorization.domain_pack_promotion_authorization.payload,
    )
    .expect("authorization payload digest");
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
        .expect("promotion signing bytes");
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
        domain_pack_reviewed_registry_entry_digest(promoted_entry).expect("reviewed entry digest");
    let final_registry_digest =
        domain_pack_reviewed_registry_digest(&proposed).expect("final reviewed registry digest");
    proposed
        .domain_pack_reviewed_registry
        .registry_digest
        .clone_from(&final_registry_digest);
    for (index, key) in [&semantic_key, &authorizer_key].into_iter().enumerate() {
        proposed.domain_pack_reviewed_registry.snapshot_signatures[index]
            .payload_digest
            .clone_from(&final_registry_digest);
        let signed = proposed.domain_pack_reviewed_registry.snapshot_signatures[index].clone();
        let bytes = domain_pack_reviewed_registry_signing_bytes(&proposed, &signed)
            .expect("final snapshot signing bytes");
        proposed.domain_pack_reviewed_registry.snapshot_signatures[index].signature =
            hex(&key.sign(&bytes).to_bytes());
    }

    PromotionGraphPaths {
        proposed: write_typed_yaml(operator_root, "reviewed-next.yaml", &proposed),
        candidate: write_typed_yaml(operator_root, "candidate.yaml", &candidate),
        dossier: write_typed_yaml(operator_root, "dossier.yaml", &dossier),
        reviews: [
            write_typed_yaml(operator_root, "review-semantic.yaml", &reviews[0]),
            write_typed_yaml(operator_root, "review-authorizer.yaml", &reviews[1]),
        ],
        decision: write_typed_yaml(operator_root, "decision.yaml", &decision),
        authorization: write_typed_yaml(operator_root, "authorization.yaml", &authorization),
    }
}

fn assert_too_large(temp: &Path, args: &[&str], label: &str) {
    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(temp)
        .args(args)
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&output);
    assert!(
        stderr.contains(label) && stderr.contains("exceeds maximum"),
        "unexpected bounded-read error: {stderr}"
    );
}

fn create_dir_link(link: &Path, target: &Path) {
    #[cfg(windows)]
    {
        // PowerShell binds paths as parameters instead of reparsing one
        // `cmd /C` command string, which keeps nested temporary paths intact.
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "New-Item -ItemType Junction -Path $env:FORGE_TEST_LINK -Target $env:FORGE_TEST_TARGET | Out-Null",
            ])
            .env("FORGE_TEST_LINK", link)
            .env("FORGE_TEST_TARGET", target)
            .output()
            .expect("create Windows directory junction");
        assert!(
            output.status.success(),
            "junction creation failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).expect("create Unix directory symlink");

    #[cfg(not(any(windows, unix)))]
    panic!("directory-link escape tests require Windows junctions or Unix symlinks");
}

fn remove_dir_link(link: &Path) {
    #[cfg(windows)]
    fs::remove_dir(link).expect("remove Windows directory link");

    #[cfg(unix)]
    fs::remove_file(link).expect("remove Unix directory symlink");

    #[cfg(not(any(windows, unix)))]
    panic!("directory-link escape tests require Windows junctions or Unix symlinks");
}

#[test]
fn domain_pack_learning_help_exposes_governed_journey_without_caller_time() {
    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(["domain-pack", "learning", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    for command in [
        "learning capture",
        "learning status",
        "learning evaluate",
        "learning conflict-check",
        "learning trust-provision",
        "learning reviewer-rotate",
        "learning registry-check",
        "learning promote",
    ] {
        assert!(stdout.contains(command), "missing {command}: {stdout}");
    }
    assert!(
        !stdout.contains("--now-unix"),
        "caller time leaked: {stdout}"
    );
    assert!(
        stdout.contains("--candidate-file <yaml> [--candidate-file <yaml>]...")
            && stdout.contains("[--conflict-file <yaml>]..."),
        "promotion exact-graph inputs missing from help: {stdout}"
    );
}

#[test]
fn domain_pack_learning_capture_then_status_remains_non_authoritative() {
    let temp = fresh_temp("learning-capture-status");
    let state = temp.join("state");
    let fixture = repo_root()
        .join("docs/fixtures/domain-pack-learning-v0/valid/local-learning-candidate.yaml");
    let mut candidate: DomainPackLocalLearningCandidateDocument =
        yaml_serde::from_str(&fs::read_to_string(fixture).expect("candidate fixture"))
            .expect("typed candidate");
    candidate
        .domain_pack_local_learning_candidate
        .candidate_digest = candidate_self_digest(&candidate).expect("candidate self digest");
    let candidate_path = temp.join("candidate.yaml");
    fs::write(
        &candidate_path,
        yaml_serde::to_string(&candidate).expect("candidate yaml"),
    )
    .expect("write candidate");

    let capture = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args([
            "domain-pack",
            "learning",
            "capture",
            "--candidate-file",
            candidate_path.to_str().expect("candidate path"),
            "--state-root",
            state.to_str().expect("state path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&capture).contains("non_authoritative"),
        "capture must disclose authority boundary"
    );

    let status = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args([
            "domain-pack",
            "learning",
            "status",
            "--state-root",
            state.to_str().expect("state path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&status);
    assert!(stdout.contains("learning.game-loop.1"), "{stdout}");
    assert!(stdout.contains("not semantic review"), "{stdout}");
    fs::remove_dir_all(temp).expect("remove temp");
}

#[test]
fn domain_pack_learning_trust_provision_requires_exact_operator_acknowledgement() {
    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(["domain-pack", "learning", "trust-provision"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(
        String::from_utf8_lossy(&stderr).contains("I_UNDERSTAND_REVIEW_TRUST_ON_FIRST_USE"),
        "missing explicit trust acknowledgement gate"
    );
}

#[test]
fn domain_pack_learning_reviewer_rotate_rejects_caller_authored_time() {
    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args([
            "domain-pack",
            "learning",
            "reviewer-rotate",
            "--now-unix",
            "1783900000",
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(stderr.contains("usage:"), "unexpected error: {stderr}");
    assert!(
        stderr.contains("caller-authored time is forbidden"),
        "trusted-time boundary missing: {stderr}"
    );
}

#[test]
fn domain_pack_learning_promote_blocks_an_incomplete_candidate_graph() {
    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(["domain-pack", "learning", "promote"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(stderr.contains("requires --candidate-file"), "{stderr}");
    assert!(stderr.contains("exact promotion graph"), "{stderr}");
}

#[test]
fn domain_pack_learning_provisions_one_atomic_head_then_freshly_checks_registry() {
    let temp = fresh_temp("learning-trust-registry-check");
    let operator = temp.join("operator");
    let project = temp.join("project");
    let state = temp.join("state");
    fs::create_dir_all(&operator).expect("operator root");
    fs::create_dir_all(&project).expect("project root");
    fs::create_dir_all(&state).expect("state root");
    let (reviewers, reviewed, successor) = write_signed_learning_roots(&operator);
    let common = [
        "--operator-root",
        operator.to_str().expect("operator path"),
        "--reviewer-registry-file",
        reviewers.to_str().expect("reviewers path"),
        "--reviewed-registry-file",
        reviewed.to_str().expect("reviewed path"),
        "--project-root",
        project.to_str().expect("project path"),
        "--state-root",
        state.to_str().expect("state path"),
    ];
    let mut provision_args = vec!["domain-pack", "learning", "trust-provision"];
    provision_args.extend(common);
    provision_args.extend([
        "--operator-acknowledge-trust-on-first-use",
        "I_UNDERSTAND_REVIEW_TRUST_ON_FIRST_USE",
    ]);
    Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(provision_args)
        .assert()
        .success();
    assert!(operator
        .join(".forge-domain-pack-learning-anchor.yaml")
        .is_file());
    assert!(!operator
        .join(".forge-domain-pack-reviewer-anchor.yaml")
        .exists());
    assert!(!operator
        .join(".forge-domain-pack-reviewed-anchor.yaml")
        .exists());

    let mut check_args = vec!["domain-pack", "learning", "registry-check"];
    check_args.extend(common);
    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args(check_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    assert!(
        stdout.contains("fresh exact cryptographic replay"),
        "{stdout}"
    );

    let rotation = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args([
            "domain-pack",
            "learning",
            "reviewer-rotate",
            "--operator-root",
            operator.to_str().expect("operator path"),
            "--reviewer-registry-file",
            reviewers.to_str().expect("reviewers path"),
            "--proposed-reviewer-registry-file",
            successor.to_str().expect("successor path"),
            "--project-root",
            project.to_str().expect("project path"),
            "--state-root",
            state.to_str().expect("state path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rotation = String::from_utf8_lossy(&rotation);
    let rotation_json: serde_json::Value =
        serde_json::from_str(&rotation).expect("rotation JSON envelope");
    assert_eq!(rotation_json["data"]["generation"], 1);
    assert!(rotation.contains("predecessor-signed"), "{rotation}");
    fs::remove_dir_all(temp).expect("remove temp");
}

#[test]
fn domain_pack_lifecycle_supply_only_inputs_are_blocked_without_reviewed_authority() {
    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args([
            "domain-pack",
            "preflight",
            "--preflight-file",
            "preflight.yaml",
            "--trust-policy-file",
            "trust.yaml",
            "--registry-file",
            "supply.yaml",
            "--resolution-request-file",
            "resolution.yaml",
            "--composition-request-file",
            "composition.yaml",
            "--trust-input-file",
            "trust-input.yaml",
            "--project-root",
            ".",
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(stderr.contains("--reviewer-registry-file"), "{stderr}");
    assert!(stderr.contains("--reviewed-registry-file"), "{stderr}");
}

#[test]
fn domain_pack_learning_promote_consumes_the_complete_exact_graph() {
    let temp = fresh_temp("learning-promote-exact-graph");
    let operator = temp.join("operator");
    let project = temp.join("project");
    let state = temp.join("state");
    fs::create_dir_all(&operator).expect("operator root");
    fs::create_dir_all(&project).expect("project root");
    fs::create_dir_all(&state).expect("state root");
    let (reviewers, reviewed, _) = write_signed_learning_roots(&operator);
    let graph = write_promotable_learning_graph(&operator, &reviewers, &reviewed);

    Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args([
            "domain-pack",
            "learning",
            "trust-provision",
            "--operator-root",
            operator.to_str().expect("operator path"),
            "--reviewer-registry-file",
            reviewers.to_str().expect("reviewers path"),
            "--reviewed-registry-file",
            reviewed.to_str().expect("reviewed path"),
            "--project-root",
            project.to_str().expect("project path"),
            "--state-root",
            state.to_str().expect("state path"),
            "--operator-acknowledge-trust-on-first-use",
            "I_UNDERSTAND_REVIEW_TRUST_ON_FIRST_USE",
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args([
            "domain-pack",
            "learning",
            "promote",
            "--operator-root",
            operator.to_str().expect("operator path"),
            "--reviewer-registry-file",
            reviewers.to_str().expect("reviewers path"),
            "--reviewed-registry-file",
            reviewed.to_str().expect("reviewed path"),
            "--proposed-registry-file",
            graph.proposed.to_str().expect("proposed path"),
            "--dossier-file",
            graph.dossier.to_str().expect("dossier path"),
            "--candidate-file",
            graph.candidate.to_str().expect("candidate path"),
            "--decision-file",
            graph.decision.to_str().expect("decision path"),
            "--authorization-file",
            graph.authorization.to_str().expect("authorization path"),
            "--review-file",
            graph.reviews[0].to_str().expect("semantic review path"),
            "--review-file",
            graph.reviews[1].to_str().expect("authorizer review path"),
            "--project-root",
            project.to_str().expect("project path"),
            "--state-root",
            state.to_str().expect("state path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: serde_json::Value =
        serde_json::from_slice(&output).expect("promotion JSON envelope");
    assert_eq!(envelope["data"]["generation"], 1);
    assert_eq!(
        envelope["data"]["boundary"],
        "opaque dual-reviewed authority consumed under retained operator lock and monotonic CAS"
    );
    fs::remove_dir_all(temp).expect("remove temp");
}

fn digest(seed: u64) -> String {
    format!("sha256:{seed:064x}")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a String cannot fail");
            output
        },
    )
}

#[allow(clippy::too_many_lines)] // Cohesive cryptographic fixture: all cross-linked fields are sealed together.
fn write_signed_supply_chain(operator_root: &Path) -> (PathBuf, PathBuf) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let registry_key = SigningKey::from_bytes(&[41_u8; 32]);
    let publisher_key = SigningKey::from_bytes(&[42_u8; 32]);
    let registry_key_id = StableId("registry.key.cli".to_owned());
    let registry_id = StableId("registry.domain-pack.cli".to_owned());
    let audience = StableId("forge.domain-pack.cli".to_owned());
    let policy = DomainPackTrustPolicyDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_trust_policy: DomainPackTrustPolicy {
            policy_id: StableId("policy.domain-pack.cli".to_owned()),
            policy_version: "1.0.0".to_owned(),
            audience: audience.clone(),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            registry_keys: vec![DomainPackRegistryTrustKey {
                key_id: registry_key_id.clone(),
                role: DomainPackRegistryTrustRole::RegistrySigner,
                public_key_hex: hex(&registry_key.verifying_key().to_bytes()),
                status: DomainPackCredentialStatus::Active,
                valid_from_unix: now.saturating_sub(60),
                valid_until_unix: now + 3_600,
            }],
            required_registry_signature_threshold: 1,
            minimum_activation_assurance: DomainPackSourceAssurance::SupplyChainVerified,
            rules: vec![DomainPackTrustRule {
                rule_id: StableId("rule.cli".to_owned()),
                pack: DomainPackCoordinate {
                    publisher: StableId("publisher.cli".to_owned()),
                    name: StableId("foundation".to_owned()),
                },
                package_digest: None,
                content_digest: None,
                disposition: DomainPackTrustDisposition::InspectOnly,
            }],
            default_disposition: DomainPackTrustDisposition::Reject,
        },
    };
    let mut record = DomainPackRegistryPackageRecord {
        identity: DomainPackIdentity {
            publisher: StableId("publisher.cli".to_owned()),
            name: StableId("foundation".to_owned()),
            namespace: StableId("sample.foundation".to_owned()),
            version: "1.0.0".to_owned(),
        },
        package_digest: digest(80_001),
        manifest_digest: digest(80_002),
        content_digest: digest(80_003),
        license_digest: digest(80_004),
        fixture_digests: vec![digest(80_005)],
        namespace_grant_id: StableId("grant.cli".to_owned()),
        publisher_credential_id: StableId("credential.cli".to_owned()),
        publisher_signature_hex: "00".repeat(64),
        record_digest: digest(80_006),
    };
    record.record_digest = domain_pack_package_record_digest(&record).expect("record digest");
    let publisher_bytes = domain_pack_publisher_signing_bytes(&registry_id, &audience, &record)
        .expect("publisher signing bytes");
    record.publisher_signature_hex = hex(&publisher_key.sign(&publisher_bytes).to_bytes());
    let mut registry = DomainPackSupplyChainRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
            registry_id,
            registry_version: "1.0.0".to_owned(),
            audience,
            authority: DomainPackCandidateAuthority::CandidateOnly,
            generation: 1,
            previous_snapshot_digest: None,
            issued_at_unix: now.saturating_sub(30),
            expires_at_unix: now + 1_800,
            publisher_credentials: vec![DomainPackPublisherCredential {
                credential_id: StableId("credential.cli".to_owned()),
                publisher: StableId("publisher.cli".to_owned()),
                public_key_hex: hex(&publisher_key.verifying_key().to_bytes()),
                status: DomainPackCredentialStatus::Active,
                valid_from_unix: now.saturating_sub(60),
                valid_until_unix: now + 3_600,
            }],
            namespace_grants: vec![DomainPackNamespaceGrant {
                grant_id: StableId("grant.cli".to_owned()),
                publisher: StableId("publisher.cli".to_owned()),
                namespace_prefix: StableId("sample".to_owned()),
                valid_from_unix: now.saturating_sub(60),
                valid_until_unix: now + 3_600,
            }],
            packages: vec![record],
            revocations: Vec::new(),
            snapshot_digest: digest(80_007),
            signatures: Vec::new(),
        },
    };
    registry.domain_pack_supply_chain_registry.snapshot_digest =
        domain_pack_registry_snapshot_digest(&registry).expect("snapshot digest");
    let registry_bytes = domain_pack_registry_signing_bytes(
        &registry,
        &registry_key_id,
        DomainPackRegistryTrustRole::RegistrySigner,
    )
    .expect("registry signing bytes");
    registry
        .domain_pack_supply_chain_registry
        .signatures
        .push(DomainPackRegistrySignature {
            signer_key_id: registry_key_id,
            role: DomainPackRegistryTrustRole::RegistrySigner,
            signature_hex: hex(&registry_key.sign(&registry_bytes).to_bytes()),
        });

    let policy_path = operator_root.join("trust-policy.yaml");
    let registry_path = operator_root.join("registry.yaml");
    fs::write(
        &policy_path,
        yaml_serde::to_string(&policy).expect("serialize policy"),
    )
    .expect("write policy");
    fs::write(
        &registry_path,
        yaml_serde::to_string(&registry).expect("serialize registry"),
    )
    .expect("write registry");
    (policy_path, registry_path)
}

fn resolution_candidate(version: &str, seed: u64) -> DomainPackResolutionCandidate {
    let base: DomainPackCompositionRequestDocument = yaml_serde::from_str(
        &fs::read_to_string(
            repo_root().join("docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml"),
        )
        .expect("read composition fixture"),
    )
    .expect("parse composition fixture");
    let mut input = base
        .domain_pack_composition_request
        .candidates
        .into_iter()
        .next()
        .expect("foundation candidate");
    version.clone_into(&mut input.manifest.domain_pack_manifest.identity.version);
    version.clone_into(&mut input.content.domain_pack_content.pack.version);
    let package = DomainPackPackageBinding {
        package_ref: RepoPath(format!("packages/foundation-{version}.yaml")),
        package_digest: digest(seed),
        manifest: input.manifest_binding.clone(),
        content: input.manifest.domain_pack_manifest.content.clone(),
        license: input
            .manifest
            .domain_pack_manifest
            .provenance
            .license_text
            .clone(),
        fixtures: input
            .content
            .domain_pack_content
            .fixtures
            .iter()
            .map(|fixture| fixture.artifact.clone())
            .collect(),
    };
    DomainPackResolutionCandidate {
        input,
        package,
        registry_record_digest: Some(digest(seed + 10_000)),
    }
}

fn resolution_registry(
    candidates: &[DomainPackResolutionCandidate],
) -> DomainPackSupplyChainRegistryDocument {
    DomainPackSupplyChainRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
            registry_id: StableId("registry.fixture".to_owned()),
            registry_version: "1.0.0".to_owned(),
            audience: StableId("forge.fixture".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            generation: 1,
            previous_snapshot_digest: None,
            issued_at_unix: 100,
            expires_at_unix: 200,
            publisher_credentials: vec![DomainPackPublisherCredential {
                credential_id: StableId("credential.fixture".to_owned()),
                publisher: StableId("forge.fixture".to_owned()),
                public_key_hex: "00".repeat(32),
                status: DomainPackCredentialStatus::Active,
                valid_from_unix: 0,
                valid_until_unix: 300,
            }],
            namespace_grants: vec![DomainPackNamespaceGrant {
                grant_id: StableId("grant.fixture".to_owned()),
                publisher: StableId("forge.fixture".to_owned()),
                namespace_prefix: StableId("sample".to_owned()),
                valid_from_unix: 0,
                valid_until_unix: 300,
            }],
            packages: candidates
                .iter()
                .map(|candidate| DomainPackRegistryPackageRecord {
                    identity: candidate
                        .input
                        .manifest
                        .domain_pack_manifest
                        .identity
                        .clone(),
                    package_digest: candidate.package.package_digest.clone(),
                    manifest_digest: candidate.package.manifest.canonical_sha256.clone(),
                    content_digest: candidate.package.content.canonical_sha256.clone(),
                    license_digest: candidate.package.license.canonical_sha256.clone(),
                    fixture_digests: candidate
                        .package
                        .fixtures
                        .iter()
                        .map(|fixture| fixture.canonical_sha256.clone())
                        .collect(),
                    namespace_grant_id: StableId("grant.fixture".to_owned()),
                    publisher_credential_id: StableId("credential.fixture".to_owned()),
                    publisher_signature_hex: "00".repeat(64),
                    record_digest: candidate
                        .registry_record_digest
                        .clone()
                        .expect("record digest"),
                })
                .collect(),
            revocations: Vec::new(),
            snapshot_digest: digest(9_000),
            signatures: Vec::new(),
        },
    }
}

fn resolution_request(
    candidates: Vec<DomainPackResolutionCandidate>,
) -> DomainPackResolutionRequestDocument {
    let base: DomainPackCompositionRequestDocument = yaml_serde::from_str(
        &fs::read_to_string(
            repo_root().join("docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml"),
        )
        .expect("read composition fixture"),
    )
    .expect("parse composition fixture");
    let base = base.domain_pack_composition_request;
    DomainPackResolutionRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_resolution_request: DomainPackResolutionRequest {
            request_id: StableId("resolution.fixture".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: base.requirements.project_id.clone(),
            forge_core_version: base.forge_core_version,
            core: base.core,
            requirements: DomainPackProjectRequirementsDocument {
                schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
                domain_pack_project_requirements: base.requirements,
            },
            roots: vec![DomainPackResolutionRoot {
                pack: DomainPackCoordinate {
                    publisher: StableId("forge.fixture".to_owned()),
                    name: StableId("foundation".to_owned()),
                },
                version_requirement: ">=1,<3".to_owned(),
                required_content_digest: None,
                reason: DomainPackResolutionRootReason::InstallIntent,
            }],
            current_lock: None,
            policy: DomainPackResolutionPolicy {
                selection: DomainPackVersionSelectionPolicy::MinimalChangeThenHighestCompatible,
                prerelease: DomainPackPrereleasePolicy::ExplicitOnly,
                duplicate_version: DomainPackDuplicateVersionPolicy::RejectDivergentContent,
                dependency_source: DomainPackDependencySourcePolicy::ExactPublisherOnly,
                unrelated_updates: DomainPackUnrelatedUpdatePolicy::PreserveLocked,
            },
            registry_snapshot_digest: digest(9_000),
            candidates,
        },
    }
}

fn fixture_document<T: serde::de::DeserializeOwned>(relative: &str) -> T {
    let path = repo_root().join(relative);
    yaml_serde::from_str(&fs::read_to_string(&path).expect("read typed fixture"))
        .unwrap_or_else(|error| panic!("parse fixture '{}': {error}", path.display()))
}

fn artifact_guard_preflight(
    binding: DomainPackArtifactBinding,
) -> DomainPackLifecyclePreflightDocument {
    let project_snapshot_digest = digest(90_001);
    let operation = DomainPackLifecycleOperation::Install {
        root: DomainPackCoordinate {
            publisher: StableId("forge.fixture".to_owned()),
            name: StableId("foundation".to_owned()),
        },
    };
    let expected_state = DomainPackExpectedLifecycleState::Uninitialized {
        project_snapshot_digest: project_snapshot_digest.clone(),
    };
    let request = DomainPackLifecycleRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_request: DomainPackLifecycleRequest {
            request_id: StableId("lifecycle.artifact-guard".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: StableId("project.domain-pack.fixture".to_owned()),
            principal_id: StableId("principal.fixture".to_owned()),
            operation,
            expected_state: expected_state.clone(),
            resolution_request_digest: digest(90_002),
            project_snapshot_digest,
        },
    };
    DomainPackLifecyclePreflightDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_lifecycle_preflight: DomainPackLifecyclePreflight {
            preflight_id: StableId("preflight.artifact-guard".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            request,
            request_digest: digest(90_003),
            observed_state: expected_state,
            resolution: fixture_document(
                "docs/fixtures/domain-pack-lifecycle-v0/valid/resolution-projection.yaml",
            ),
            proposed_lock: fixture_document(
                "docs/fixtures/domain-pack-lifecycle-v0/valid/exact-lock.yaml",
            ),
            composition: fixture_document(
                "docs/fixtures/domain-pack-v0/projections/neutral-two-pack.expected.yaml",
            ),
            supply_chain_assessments: Vec::new(),
            trust_decisions: Vec::new(),
            capability_gaps: Vec::new(),
            compatibility_report: fixture_document(
                "docs/fixtures/domain-pack-lifecycle-v0/valid/compatibility-report.yaml",
            ),
            staged_artifacts: vec![binding],
            status: DomainPackLifecyclePreflightStatus::Ready,
            issues: Vec::new(),
            preflight_digest: digest(90_004),
        },
    }
}

fn lifecycle_preflight_command(
    current_dir: &Path,
    project_root: &Path,
    artifact_root: &Path,
    state_root: &Path,
    preflight_file: &Path,
    trust_policy_file: &Path,
    registry_file: &Path,
) -> Command {
    let operator_root = registry_file
        .parent()
        .expect("registry fixture has an operator parent");
    let mut command = Command::cargo_bin("forge-core").expect("forge-core binary");
    command
        .current_dir(current_dir)
        .arg("domain-pack")
        .arg("preflight")
        .arg("--preflight-file")
        .arg(preflight_file)
        .arg("--trust-policy-file")
        .arg(trust_policy_file)
        .arg("--registry-file")
        .arg(registry_file)
        .arg("--reviewer-registry-file")
        .arg(operator_root.join("reviewer-registry.yaml"))
        .arg("--reviewed-registry-file")
        .arg(operator_root.join("reviewed-registry.yaml"))
        .arg("--resolution-request-file")
        .arg(current_dir.join("resolution.yaml"))
        .arg("--composition-request-file")
        .arg(current_dir.join("composition.yaml"))
        .arg("--trust-input-file")
        .arg(current_dir.join("trust-input.yaml"))
        .arg("--project-root")
        .arg(project_root)
        .arg("--artifact-root")
        .arg(artifact_root)
        .arg("--state-root")
        .arg(state_root)
        .arg("--json");
    command
}

#[test]
fn agent_cli_validates_and_composes_without_writes() {
    let temp = std::env::temp_dir().join(format!(
        "forge-domain-pack-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let fixture_root = temp.join("docs/fixtures/domain-pack-v0");
    copy_tree(
        &repo_root().join("docs/fixtures/domain-pack-v0"),
        &fixture_root,
    );
    let before = snapshot(&temp);

    let validate = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "validate",
            "--manifest-file",
            "docs/fixtures/domain-pack-v0/manifests/foundation.yaml",
            "--content-file",
            "docs/fixtures/domain-pack-v0/content/foundation.yaml",
            "--artifact-root",
            ".",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let validate_json: serde_json::Value =
        serde_json::from_slice(&validate).expect("validate JSON");
    assert_eq!(validate_json["ok"], true);
    assert_eq!(validate_json["data"]["structurally_valid"], true);
    assert_eq!(validate_json["data"]["authority"], "candidate_only");

    let compose = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "compose",
            "--request-file",
            "docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml",
            "--artifact-root",
            ".",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let compose_json: serde_json::Value = serde_json::from_slice(&compose).expect("compose JSON");
    assert_eq!(compose_json["ok"], true);
    assert_eq!(
        compose_json["data"]["domain_pack_composition_projection"]["status"],
        "composable"
    );
    assert_eq!(
        compose_json["data"]["domain_pack_composition_projection"]["authority"],
        "candidate_only"
    );
    assert_eq!(
        snapshot(&temp),
        before,
        "read-only CLI changed fixture tree"
    );

    fs::remove_dir_all(temp).expect("cleanup CLI fixture");
}

#[test]
fn compose_rejects_artifact_path_escape() {
    let temp = std::env::temp_dir().join(format!(
        "forge-domain-pack-cli-escape-{}",
        std::process::id()
    ));
    let fixture_root = temp.join("docs/fixtures/domain-pack-v0");
    if temp.exists() {
        fs::remove_dir_all(&temp).expect("clean stale temp");
    }
    copy_tree(
        &repo_root().join("docs/fixtures/domain-pack-v0"),
        &fixture_root,
    );
    let request_path = fixture_root.join("requests/escape.yaml");
    let request = fs::read_to_string(fixture_root.join("requests/neutral-extension-removed.yaml"))
        .expect("request");
    fs::write(
        &request_path,
        request.replace(
            "docs/fixtures/domain-pack-v0/content/foundation.yaml",
            "../outside.yaml",
        ),
    )
    .expect("escape request");

    Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "compose",
            "--request-file",
            "docs/fixtures/domain-pack-v0/requests/escape.yaml",
            "--artifact-root",
            ".",
            "--json",
        ])
        .assert()
        .failure();
    fs::remove_dir_all(temp).expect("cleanup escape fixture");
}

#[test]
fn domain_pack_inputs_are_bounded_before_parse() {
    let temp = fresh_temp("bounded-inputs");
    let fixture_root = temp.join("docs/fixtures/domain-pack-v0");
    copy_tree(
        &repo_root().join("docs/fixtures/domain-pack-v0"),
        &fixture_root,
    );

    let oversized_request = temp.join("oversized-request.yaml");
    write_oversized(&oversized_request);
    assert_too_large(
        &temp,
        &[
            "domain-pack",
            "compose",
            "--request-file",
            "oversized-request.yaml",
            "--artifact-root",
            ".",
            "--json",
        ],
        "composition request",
    );

    let oversized_manifest = temp.join("oversized-manifest.yaml");
    write_oversized(&oversized_manifest);
    assert_too_large(
        &temp,
        &[
            "domain-pack",
            "validate",
            "--manifest-file",
            "oversized-manifest.yaml",
            "--content-file",
            "docs/fixtures/domain-pack-v0/content/foundation.yaml",
            "--artifact-root",
            ".",
            "--json",
        ],
        "manifest",
    );

    let oversized_content = temp.join("oversized-content.yaml");
    write_oversized(&oversized_content);
    assert_too_large(
        &temp,
        &[
            "domain-pack",
            "validate",
            "--manifest-file",
            "docs/fixtures/domain-pack-v0/manifests/foundation.yaml",
            "--content-file",
            "oversized-content.yaml",
            "--artifact-root",
            ".",
            "--json",
        ],
        "content",
    );

    write_oversized(&fixture_root.join("artifacts/license-notice.yaml"));
    assert_too_large(
        &temp,
        &[
            "domain-pack",
            "validate",
            "--manifest-file",
            "docs/fixtures/domain-pack-v0/manifests/foundation.yaml",
            "--content-file",
            "docs/fixtures/domain-pack-v0/content/foundation.yaml",
            "--artifact-root",
            ".",
            "--json",
        ],
        "license artifact",
    );

    fs::remove_dir_all(temp).expect("cleanup bounded-input fixture");
}

#[test]
fn compose_rejects_canonical_directory_link_escape() {
    let temp = fresh_temp("canonical-escape");
    let fixture_root = temp.join("docs/fixtures/domain-pack-v0");
    copy_tree(
        &repo_root().join("docs/fixtures/domain-pack-v0"),
        &fixture_root,
    );
    let outside = temp.join("outside");
    fs::create_dir_all(&outside).expect("create external artifact directory");
    fs::copy(
        fixture_root.join("content/foundation.yaml"),
        outside.join("foundation.yaml"),
    )
    .expect("copy external artifact");
    let link = fixture_root.join("escape-link");
    create_dir_link(&link, &outside);

    let request_path = fixture_root.join("requests/canonical-escape.yaml");
    let request = fs::read_to_string(fixture_root.join("requests/neutral-extension-removed.yaml"))
        .expect("request");
    fs::write(
        &request_path,
        request.replace(
            "docs/fixtures/domain-pack-v0/manifests/foundation.yaml",
            "escape-link/foundation.yaml",
        ),
    )
    .expect("canonical escape request");

    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "compose",
            "--request-file",
            "docs/fixtures/domain-pack-v0/requests/canonical-escape.yaml",
            "--artifact-root",
            "docs/fixtures/domain-pack-v0",
            "--json",
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(
        String::from_utf8_lossy(&stderr).contains("escapes canonical --artifact-root"),
        "unexpected canonical confinement error: {}",
        String::from_utf8_lossy(&stderr)
    );

    remove_dir_link(&link);
    fs::remove_dir_all(temp).expect("cleanup canonical escape fixture");
}

#[test]
fn lifecycle_status_and_recover_emit_integrity_checked_empty_state() {
    let temp = fresh_temp("lifecycle-empty-state");
    let state_root = temp.join(".forge-method");
    fs::create_dir(&state_root).expect("create canonical state root");

    for subcommand in ["status", "recover"] {
        let stdout = Command::cargo_bin("forge-core")
            .expect("forge-core binary")
            .current_dir(&temp)
            .args([
                "domain-pack",
                subcommand,
                "--state-root",
                ".forge-method",
                "--json",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let envelope: serde_json::Value =
            serde_json::from_slice(&stdout).expect("lifecycle JSON envelope");
        assert_eq!(envelope["ok"], true);
        assert_eq!(envelope["command"], format!("domain-pack {subcommand}"));
        assert_eq!(envelope["data"]["active"], false);
        assert_eq!(envelope["data"]["active_pointer"], serde_json::Value::Null);
        assert_eq!(envelope["data"]["active_lock"], serde_json::Value::Null);
        assert_eq!(envelope["data"]["ledger_records"], serde_json::json!([]));
        assert_eq!(
            envelope["data"]["recovery_report"]["domain_pack_recovery_report"]["authority"],
            "candidate_only"
        );
        assert_eq!(
            envelope["data"]["recovery_report"]["domain_pack_recovery_report"]["status"],
            "clean"
        );
        assert_eq!(
            envelope["data"]["recovery_report"]["domain_pack_recovery_report"]["active_state"],
            serde_json::Value::Null
        );
        assert_eq!(envelope["data"]["recovery_checked"], true);
        assert!(envelope["data"]["state_root"]
            .as_str()
            .expect("state root")
            .ends_with("/.forge-method"));
    }

    fs::remove_dir_all(temp).expect("cleanup lifecycle state fixture");
}

#[test]
fn resolve_requires_both_closed_typed_inputs() {
    let temp = fresh_temp("resolve-inputs");
    fs::write(temp.join("request.yaml"), "schema_version: '0.2'\n")
        .expect("write malformed request");
    fs::write(temp.join("registry.yaml"), "schema_version: '0.2'\n")
        .expect("write malformed registry");

    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "resolve",
            "--request-file",
            "request.yaml",
            "--registry-file",
            "registry.yaml",
            "--json",
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(
        String::from_utf8_lossy(&stderr).contains("not a closed typed document"),
        "unexpected resolution input error: {}",
        String::from_utf8_lossy(&stderr)
    );

    fs::remove_dir_all(temp).expect("cleanup resolution input fixture");
}

#[test]
fn resolve_emits_deterministic_highest_compatible_projection() {
    let temp = fresh_temp("resolve-success");
    let candidates = vec![
        resolution_candidate("1.0.0", 1),
        resolution_candidate("2.0.0", 2),
    ];
    let request = resolution_request(candidates.clone());
    let registry = resolution_registry(&candidates);
    fs::write(
        temp.join("request.yaml"),
        yaml_serde::to_string(&request).expect("serialize resolution request"),
    )
    .expect("write resolution request");
    fs::write(
        temp.join("registry.yaml"),
        yaml_serde::to_string(&registry).expect("serialize registry"),
    )
    .expect("write registry");

    let stdout = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "resolve",
            "--request-file",
            "request.yaml",
            "--registry-file",
            "registry.yaml",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: serde_json::Value =
        serde_json::from_slice(&stdout).expect("resolution JSON envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(
        envelope["data"]["domain_pack_resolution_projection"]["status"],
        "resolved"
    );
    assert_eq!(
        envelope["data"]["domain_pack_resolution_projection"]["authority"],
        "candidate_only"
    );
    assert_eq!(
        envelope["data"]["domain_pack_resolution_projection"]["selected"][0]["identity"]["version"],
        "2.0.0"
    );

    fs::remove_dir_all(temp).expect("cleanup successful resolution fixture");
}

#[test]
fn lifecycle_preflight_requires_an_explicit_project_snapshot_root() {
    let temp = fresh_temp("preflight-project-root");
    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "preflight",
            "--preflight-file",
            "preflight.yaml",
            "--trust-policy-file",
            "trust-policy.yaml",
            "--registry-file",
            "registry.yaml",
            "--resolution-request-file",
            "resolution.yaml",
            "--composition-request-file",
            "composition.yaml",
            "--trust-input-file",
            "trust-input.yaml",
            "--json",
        ])
        .assert()
        .code(2)
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        stderr.contains("--project-root <path>"),
        "usage did not expose the mandatory project snapshot root: {stderr}"
    );

    fs::remove_dir_all(temp).expect("cleanup project-root requirement fixture");
}

#[test]
fn lifecycle_commands_reject_caller_authored_time_and_hide_it_from_help() {
    let temp = fresh_temp("preflight-real-clock");
    let help = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args(["domain-pack", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8_lossy(&help);
    assert!(
        !help.contains("--now-unix"),
        "domain-pack help exposed caller-authored time: {help}"
    );

    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args([
            "domain-pack",
            "preflight",
            "--preflight-file",
            "preflight.yaml",
            "--trust-policy-file",
            "trust-policy.yaml",
            "--registry-file",
            "registry.yaml",
            "--resolution-request-file",
            "resolution.yaml",
            "--composition-request-file",
            "composition.yaml",
            "--trust-input-file",
            "trust-input.yaml",
            "--project-root",
            ".",
            "--now-unix",
            "100",
            "--json",
        ])
        .assert()
        .code(2)
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        !stderr.contains("--now-unix"),
        "usage accidentally re-advertised rejected caller time: {stderr}"
    );
    assert!(
        stderr.contains("domain-pack preflight"),
        "unexpected rejection diagnostic: {stderr}"
    );

    fs::remove_dir_all(temp).expect("cleanup real-clock CLI fixture");
}

#[test]
#[allow(clippy::too_many_lines)] // One end-to-end ceremony proves denial, initialization, persistence, and replay.
fn trust_provision_is_explicit_signed_and_required_before_lifecycle_authority() {
    let temp = fresh_temp("explicit-trust-provision");
    let project_root = temp.join("project");
    let artifact_root = project_root.join("artifacts");
    let state_root = project_root.join(".forge-method");
    let operator_root = temp.join("operator");
    fs::create_dir_all(&artifact_root).expect("create artifact root");
    fs::create_dir_all(&operator_root).expect("create operator root");
    let (trust_policy, registry) = write_signed_supply_chain(&operator_root);
    for name in ["reviewer-registry.yaml", "reviewed-registry.yaml"] {
        fs::write(operator_root.join(name), "not: authoritative\n")
            .expect("write inert reviewed-authority root");
    }

    let mut preflight = artifact_guard_preflight(DomainPackArtifactBinding {
        artifact_ref: RepoPath("unused.yaml".to_owned()),
        raw_sha256: digest(81_001),
        canonical_sha256: digest(81_002),
    });
    preflight
        .domain_pack_lifecycle_preflight
        .staged_artifacts
        .clear();
    let preflight_file = temp.join("preflight.yaml");
    fs::write(
        &preflight_file,
        yaml_serde::to_string(&preflight).expect("serialize preflight"),
    )
    .expect("write preflight");
    for name in ["resolution.yaml", "composition.yaml", "trust-input.yaml"] {
        fs::write(temp.join(name), "not: authoritative\n").expect("write inert input");
    }

    let stderr = lifecycle_preflight_command(
        &temp,
        &project_root,
        &artifact_root,
        &state_root,
        &preflight_file,
        &trust_policy,
        &registry,
    )
    .assert()
    .code(2)
    .get_output()
    .stderr
    .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        stderr.contains("operator registry anchor is not provisioned")
            && stderr.contains("trust-provision"),
        "lifecycle command did not fail closed on absent anchor: {stderr}"
    );

    let base_args = [
        "domain-pack",
        "trust-provision",
        "--operator-root",
        operator_root.to_str().expect("operator root utf8"),
        "--trust-policy-file",
        trust_policy.to_str().expect("policy path utf8"),
        "--registry-file",
        registry.to_str().expect("registry path utf8"),
        "--project-root",
        project_root.to_str().expect("project root utf8"),
        "--artifact-root",
        artifact_root.to_str().expect("artifact root utf8"),
        "--state-root",
        state_root.to_str().expect("state root utf8"),
    ];
    let stderr = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args(base_args)
        .assert()
        .code(3)
        .get_output()
        .stderr
        .clone();
    assert!(
        String::from_utf8_lossy(&stderr)
            .contains("--operator-acknowledge-trust-on-first-use I_UNDERSTAND_TRUST_ON_FIRST_USE"),
        "missing acknowledgement did not fail closed: {}",
        String::from_utf8_lossy(&stderr)
    );
    assert!(
        !operator_root
            .join(".forge-domain-pack-registry-anchor.yaml")
            .exists(),
        "anchor was created without acknowledgement"
    );

    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args(base_args)
        .args([
            "--operator-acknowledge-trust-on-first-use",
            "I_UNDERSTAND_TRUST_ON_FIRST_USE",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: serde_json::Value =
        serde_json::from_slice(&output).expect("trust provision JSON envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["generation"], 1);
    assert_eq!(envelope["data"]["anchor_previously_present"], false);
    assert!(
        operator_root
            .join(".forge-domain-pack-registry-anchor.yaml")
            .is_file(),
        "explicit signed ceremony did not persist the anchor"
    );

    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .current_dir(&temp)
        .args(base_args)
        .args([
            "--operator-acknowledge-trust-on-first-use",
            "I_UNDERSTAND_TRUST_ON_FIRST_USE",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let envelope: serde_json::Value =
        serde_json::from_slice(&output).expect("trust replay JSON envelope");
    assert_eq!(envelope["data"]["anchor_previously_present"], true);

    fs::remove_dir_all(temp).expect("cleanup explicit trust fixture");
}

#[test]
fn lifecycle_mutation_rejects_project_controlled_trust_roots() {
    let temp = fresh_temp("trust-root-boundary");
    let project_root = temp.join("project");
    let artifact_root = project_root.join("artifacts");
    let state_root = project_root.join(".forge-method");
    let operator_root = temp.join("operator");
    fs::create_dir_all(&artifact_root).expect("create artifact root");
    fs::create_dir_all(&operator_root).expect("create operator root");
    for name in [
        "preflight.yaml",
        "resolution.yaml",
        "composition.yaml",
        "trust-input.yaml",
    ] {
        fs::write(temp.join(name), "not: authoritative\n").expect("write inert input");
    }
    let external_policy = operator_root.join("trust-policy.yaml");
    let external_registry = operator_root.join("registry.yaml");
    let project_policy = project_root.join("trust-policy.yaml");
    let project_registry = project_root.join("registry.yaml");
    for path in [
        &external_policy,
        &external_registry,
        &project_policy,
        &project_registry,
    ] {
        fs::write(path, "not: authoritative\n").expect("write trust-root probe");
    }
    for name in ["reviewer-registry.yaml", "reviewed-registry.yaml"] {
        fs::write(operator_root.join(name), "not: authoritative\n")
            .expect("write inert reviewed-authority root");
    }

    for (policy, registry, label) in [
        (&project_policy, &external_registry, "operator trust policy"),
        (
            &external_policy,
            &project_registry,
            "signed supply-chain registry",
        ),
    ] {
        let stderr = lifecycle_preflight_command(
            &temp,
            &project_root,
            &artifact_root,
            &state_root,
            &temp.join("preflight.yaml"),
            policy,
            registry,
        )
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
        let stderr = String::from_utf8_lossy(&stderr);
        assert!(
            stderr.contains(label)
                && stderr.contains("operator-controlled")
                && stderr.contains("external to --project-root"),
            "unexpected trust-root rejection: {stderr}"
        );
    }

    let linked_operator = project_root.join("linked-operator-root");
    create_dir_link(&linked_operator, &operator_root);
    let stderr = lifecycle_preflight_command(
        &temp,
        &project_root,
        &artifact_root,
        &state_root,
        &temp.join("preflight.yaml"),
        &linked_operator.join("trust-policy.yaml"),
        &external_registry,
    )
    .assert()
    .failure()
    .get_output()
    .stderr
    .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        stderr.contains("operator trust policy") && stderr.contains("external to --project-root"),
        "unexpected linked trust-root rejection: {stderr}"
    );
    remove_dir_link(&linked_operator);

    fs::remove_dir_all(temp).expect("cleanup trust-root fixture");
}

#[test]
fn lifecycle_preflight_rejects_missing_and_tampered_staged_fixture_bytes() {
    let temp = fresh_temp("immutable-artifact-bytes");
    let project_root = temp.join("project");
    let artifact_root = project_root.join("artifacts");
    let state_root = project_root.join(".forge-method");
    let operator_root = temp.join("operator");
    fs::create_dir_all(&artifact_root).expect("create artifact root");
    fs::create_dir_all(&operator_root).expect("create operator root");
    for name in ["resolution.yaml", "composition.yaml", "trust-input.yaml"] {
        fs::write(temp.join(name), "not: authoritative\n").expect("write inert input");
    }
    let trust_policy = operator_root.join("trust-policy.yaml");
    let registry = operator_root.join("registry.yaml");
    fs::write(&trust_policy, "not: authoritative\n").expect("write external policy");
    fs::write(&registry, "not: authoritative\n").expect("write external registry");
    for name in ["reviewer-registry.yaml", "reviewed-registry.yaml"] {
        fs::write(operator_root.join(name), "not: authoritative\n")
            .expect("write inert reviewed-authority root");
    }

    let missing_binding = DomainPackArtifactBinding {
        artifact_ref: RepoPath("fixtures/missing.yaml".to_owned()),
        raw_sha256: digest(91_001),
        canonical_sha256: digest(91_002),
    };
    let preflight_file = temp.join("preflight.yaml");
    fs::write(
        &preflight_file,
        yaml_serde::to_string(&artifact_guard_preflight(missing_binding))
            .expect("serialize missing-artifact preflight"),
    )
    .expect("write missing-artifact preflight");
    let stderr = lifecycle_preflight_command(
        &temp,
        &project_root,
        &artifact_root,
        &state_root,
        &preflight_file,
        &trust_policy,
        &registry,
    )
    .assert()
    .failure()
    .get_output()
    .stderr
    .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        stderr.contains("fixtures/missing.yaml") && stderr.contains("cannot inspect artifact"),
        "unexpected missing-artifact rejection: {stderr}"
    );

    let artifact_ref = "fixtures/tampered.yaml";
    let artifact_path = artifact_root.join(artifact_ref);
    fs::create_dir_all(artifact_path.parent().expect("fixture parent"))
        .expect("create fixture parent");
    fs::write(&artifact_path, b"tampered bytes").expect("write tampered fixture");
    let expected_bytes = b"expected immutable bytes";
    let tampered_binding = DomainPackArtifactBinding {
        artifact_ref: RepoPath(artifact_ref.to_owned()),
        raw_sha256: format!("sha256:{:x}", Sha256::digest(expected_bytes)),
        canonical_sha256: digest(91_003),
    };
    fs::write(
        &preflight_file,
        yaml_serde::to_string(&artifact_guard_preflight(tampered_binding))
            .expect("serialize tampered-artifact preflight"),
    )
    .expect("write tampered-artifact preflight");
    let stderr = lifecycle_preflight_command(
        &temp,
        &project_root,
        &artifact_root,
        &state_root,
        &preflight_file,
        &trust_policy,
        &registry,
    )
    .assert()
    .failure()
    .get_output()
    .stderr
    .clone();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        stderr.contains(artifact_ref) && stderr.contains("differs from raw_sha256 binding"),
        "unexpected tampered-artifact rejection: {stderr}"
    );

    fs::remove_dir_all(temp).expect("cleanup immutable-artifact fixture");
}
