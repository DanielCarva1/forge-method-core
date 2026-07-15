use assert_cmd::Command;
use forge_core_contracts::{
    DomainCapabilityDeclarationAuthority, DomainPackCandidateAuthority, DomainPackCapabilityKind,
    DomainPackContent, DomainPackContentDocument, DomainPackDemandProvenance,
    DomainPackDemandSource, DomainPackDiscoveryCandidate, DomainPackDiscoveryRequest,
    DomainPackDiscoveryRequestDocument, DomainPackDomainRequirement, DomainPackProjectRequirements,
    DomainPackPromotionStage, DomainPackProvidedCapability, DomainPackProvidedDomain,
    DomainPackReviewedCompatibility, DomainPackReviewedEligibility,
    DomainPackReviewedRegistryEntry, DomainPackVersionReference, DurableAssuranceEpochBinding,
    StableId, WorkflowGovernancePolicyOverlay, DOMAIN_PACK_DISCOVERY_SCHEMA_VERSION,
    DOMAIN_PACK_SCHEMA_VERSION,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;

fn digest<T: Serialize>(value: &T) -> String {
    let canonical = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    format!("sha256:{:x}", Sha256::digest(canonical))
}

fn raw_digest<T: Serialize>(value: &T) -> String {
    let canonical = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    format!("{:x}", Sha256::digest(canonical))
}

fn hash(character: char) -> String {
    format!("sha256:{}", character.to_string().repeat(64))
}

fn raw_hash(character: char) -> String {
    character.to_string().repeat(64)
}

fn candidate() -> DomainPackDiscoveryCandidate {
    let pack = DomainPackVersionReference {
        publisher: StableId("publisher.neutral".to_owned()),
        name: StableId("neutral-method".to_owned()),
        version: "1.0.0".to_owned(),
    };
    let content = DomainPackContentDocument {
        schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
        domain_pack_content: DomainPackContent {
            pack: pack.clone(),
            namespace: StableId("publisher.neutral.method".to_owned()),
            workflow_overlay: WorkflowGovernancePolicyOverlay {
                id: StableId("overlay.neutral".to_owned()),
                base_bundle_id: StableId("bundle.core".to_owned()),
                policies: Vec::new(),
            },
            provided_domains: vec![DomainPackProvidedDomain {
                id: StableId("domain.neutral".to_owned()),
                description: "Neutral domain supplied by fixture data".to_owned(),
                policy_refs: Vec::new(),
                hazard_refs: Vec::new(),
                lifecycle_model_refs: Vec::new(),
            }],
            provided_capabilities: vec![DomainPackProvidedCapability {
                id: StableId("capability.neutral.review".to_owned()),
                kind: DomainPackCapabilityKind::HumanReview,
                description: "Neutral review capability".to_owned(),
                evidence_rule_refs: Vec::new(),
                authority: DomainCapabilityDeclarationAuthority::DeclarationOnly,
            }],
            hazards: Vec::new(),
            lifecycle_models: Vec::new(),
            evaluators: Vec::new(),
            fixtures: Vec::new(),
            adapters: Vec::new(),
        },
    };
    let content_digest = digest(&content);
    let mut reviewed_entry = DomainPackReviewedRegistryEntry {
        pack,
        package_digest: hash('c'),
        supply_chain_record_digest: hash('d'),
        manifest_digest: hash('e'),
        content_digest,
        license_digest: hash('f'),
        fixture_digests: vec![hash('0')],
        stage: DomainPackPromotionStage::Reviewed,
        eligibility: DomainPackReviewedEligibility::EligibleReviewed,
        promotion_decision_digest: raw_hash('1'),
        authorization_digest: raw_hash('2'),
        independent_review_digests: vec![raw_hash('3'), raw_hash('4')],
        compatibility: DomainPackReviewedCompatibility {
            forge_core_requirement: ">=0.12.0, <1.0.0".to_owned(),
            pack_schema_requirement: "^0.1".to_owned(),
            evaluator_protocol_versions: Vec::new(),
            predecessor_content_digests: Vec::new(),
            breaking_change: false,
            migration_evidence_refs: Vec::new(),
        },
        deprecation: None,
        revocation: None,
        supersession: None,
        entry_digest: String::new(),
    };
    reviewed_entry.entry_digest = raw_digest(&reviewed_entry);
    DomainPackDiscoveryCandidate {
        reviewed_entry,
        content,
    }
}

fn request(domain_id: &str) -> DomainPackDiscoveryRequestDocument {
    DomainPackDiscoveryRequestDocument {
        schema_version: DOMAIN_PACK_DISCOVERY_SCHEMA_VERSION.to_owned(),
        domain_pack_discovery_request: DomainPackDiscoveryRequest {
            request_id: StableId("discovery.cli.neutral".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            assurance_binding: DurableAssuranceEpochBinding {
                project_id: StableId("project.cli.neutral".to_owned()),
                assurance_epoch: 1,
                intent_id: StableId("intent.cli.neutral".to_owned()),
                intent_revision: 1,
                intent_digest: hash('a'),
                accepted_record_digest: hash('5'),
                accepted_sequence: 1,
                accepted_state_version: 1,
                snapshot_digest: hash('6'),
                ledger_head_before_acceptance: hash('7'),
            },
            requirements: DomainPackProjectRequirements {
                project_id: StableId("project.cli.neutral".to_owned()),
                requirement_set_id: StableId("requirements.cli.neutral".to_owned()),
                required_domains: vec![DomainPackDomainRequirement {
                    id: StableId("requirement.cli.neutral".to_owned()),
                    domain_id: StableId(domain_id.to_owned()),
                    pack_version_requirement: "^1.0".to_owned(),
                    required_capability_refs: vec![StableId(
                        "capability.neutral.review".to_owned(),
                    )],
                }],
            },
            provenance: DomainPackDemandProvenance {
                source: DomainPackDemandSource::HostProposal,
                source_ref: "conversation://cli/neutral".to_owned(),
                source_digest: hash('b'),
            },
            uncertainties: vec!["Operator trust is not yet established".to_owned()],
            candidates: vec![candidate()],
        },
    }
}

fn run_file(path: &std::path::Path) -> Value {
    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args([
            "domain-pack",
            "search",
            "--request-file",
            &path.display().to_string(),
            "--json",
        ])
        .output()
        .expect("domain-pack search");
    assert!(
        output.status.success(),
        "search failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON envelope")
}

fn run(request: &DomainPackDiscoveryRequestDocument, label: &str) -> Value {
    let root = std::env::temp_dir().join(format!(
        "forge-domain-pack-discovery-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("fixture root");
    let path = root.join("request.yaml");
    fs::write(
        &path,
        yaml_serde::to_string(request).expect("discovery request YAML"),
    )
    .expect("request file");
    run_file(&path)
}

#[cfg(unix)]
#[test]
fn authority_input_swapped_to_fifo_is_rejected_without_blocking() {
    let root = std::env::temp_dir().join(format!(
        "forge-domain-pack-discovery-fifo-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("fixture root");
    let fifo = root.join("request.yaml");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo available on Unix");
    assert!(status.success());
    let started = std::time::Instant::now();
    let rejected = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args([
            "domain-pack",
            "search",
            "--request-file",
            &fifo.display().to_string(),
            "--json",
        ])
        .output()
        .expect("bounded FIFO rejection");
    assert!(!rejected.status.success());
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "bounded regular-file open must not block on a FIFO"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn yaml_alias_expansion_is_rejected_before_typed_deserialization() {
    let root = std::env::temp_dir().join(format!(
        "forge-domain-pack-discovery-alias-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("alias fixture root");
    let path = root.join("alias.yaml");
    fs::write(
        &path,
        "schema_version: &version '0.1'\ndomain_pack_discovery_request: *version\n",
    )
    .expect("alias fixture");
    let output = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args([
            "domain-pack",
            "search",
            "--request-file",
            &path.display().to_string(),
            "--json",
        ])
        .output()
        .expect("alias rejection");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("alias-free"));
}

#[test]
fn checked_in_corpus_is_domain_neutral_and_data_extensible() {
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/domain-pack-discovery");
    for (file, expected_status) in [
        ("neutral-reviewed-match.yaml", "matched"),
        ("neutral-uncovered-gap.yaml", "gaps_present"),
        ("game-combat-reviewed-match.yaml", "matched"),
    ] {
        let envelope = run_file(&corpus.join(file));
        assert_eq!(
            envelope["data"]["domain_pack_discovery_projection"]["status"], expected_status,
            "unexpected corpus result for {file}"
        );
    }
}

#[test]
fn public_search_is_read_only_candidate_projection_with_explicit_gaps() {
    let matched = run(&request("domain.neutral"), "matched");
    assert_eq!(matched["ok"], true);
    assert_eq!(matched["command"], "domain-pack search");
    assert_eq!(
        matched["data"]["domain_pack_discovery_projection"]["authority"],
        "candidate_only"
    );
    assert_eq!(
        matched["data"]["domain_pack_discovery_projection"]["status"],
        "matched"
    );
    assert_eq!(
        matched["data"]["domain_pack_discovery_projection"]["matches"]
            .as_array()
            .expect("matches")
            .len(),
        1
    );
    assert!(
        matched["data"]["domain_pack_discovery_projection"]["matches"][0]["candidate_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("candidate."))
    );

    let explain_root =
        std::env::temp_dir().join(format!("forge-domain-pack-explain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&explain_root);
    fs::create_dir_all(&explain_root).expect("explain root");
    let projection_path = explain_root.join("projection.yaml");
    fs::write(
        &projection_path,
        yaml_serde::to_string(&matched["data"]).expect("projection YAML"),
    )
    .expect("projection file");
    let explanation = Command::cargo_bin("forge-core")
        .expect("forge-core binary")
        .args([
            "domain-pack",
            "explain",
            "--projection-file",
            &projection_path.display().to_string(),
            "--requirement-ref",
            "requirement.cli.neutral",
            "--json",
        ])
        .output()
        .expect("domain-pack explain");
    assert!(
        explanation.status.success(),
        "explain failed: stdout={} stderr={}",
        String::from_utf8_lossy(&explanation.stdout),
        String::from_utf8_lossy(&explanation.stderr)
    );
    let explanation: Value =
        serde_json::from_slice(&explanation.stdout).expect("explanation envelope");
    assert_eq!(explanation["command"], "domain-pack explain");
    assert_eq!(explanation["data"]["authority"], "candidate_only");
    assert_eq!(
        explanation["data"]["matches"]
            .as_array()
            .expect("explained matches")
            .len(),
        1
    );

    let gap = run(&request("domain.uncovered"), "gap");
    assert_eq!(
        gap["data"]["domain_pack_discovery_projection"]["status"],
        "gaps_present"
    );
    assert_eq!(
        gap["data"]["domain_pack_discovery_projection"]["gaps"][0]["code"],
        "no_eligible_reviewed_pack"
    );
    assert!(
        gap["data"]["domain_pack_discovery_projection"]["gaps"][0]["next_action"]
            .as_str()
            .is_some_and(|action| !action.is_empty())
    );
    assert_eq!(
        gap["data"]["domain_pack_discovery_projection"]["uncertainties"][0],
        "Operator trust is not yet established"
    );
}
