use forge_core_contracts::{
    DomainPackCandidateInput, DomainPackCompositionGapCode, DomainPackCompositionIssueCode,
    DomainPackCompositionRequestDocument, DomainPackCompositionStatus, HumanDecisionReason,
    NextActionKind, PrincipalId, ReadinessTarget, StableId, WorkflowCompletionAssertion,
    WorkflowEvidenceFreshness, WorkflowEvidenceKind, WorkflowEvidenceObservation,
    WorkflowEvidenceOutcome, WorkflowEvidenceStrength, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceEvaluation, WorkflowGovernanceEvaluationDocument,
    WORKFLOW_GOVERNANCE_SCHEMA_VERSION,
};
use forge_core_decisions::{
    compose_domain_packs, simulate_workflow_governance, validate_domain_pack_candidate,
    DomainPackCandidateMaterial, WorkflowClaimResultStatus, WorkflowCompletionVerdict,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const FIXTURE_ROOT: &str = "docs/fixtures/domain-pack-reference-v0";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> Vec<u8> {
    std::fs::read(repo_root().join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

fn load_yaml<T: serde::de::DeserializeOwned>(relative: &str) -> T {
    yaml_serde::from_str(
        std::str::from_utf8(&read(relative))
            .unwrap_or_else(|error| panic!("non-UTF-8 fixture {relative}: {error}")),
    )
    .unwrap_or_else(|error| panic!("invalid fixture {relative}: {error}"))
}

fn sha256(raw: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(raw))
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    sha256(&bytes)
}

#[derive(Debug)]
struct Material {
    publisher: String,
    name: String,
    version: String,
    manifest: Vec<u8>,
    content: Vec<u8>,
    license: Vec<u8>,
}

impl Material {
    fn borrowed(&self) -> DomainPackCandidateMaterial<'_> {
        DomainPackCandidateMaterial {
            publisher: &self.publisher,
            name: &self.name,
            version: &self.version,
            manifest_raw: &self.manifest,
            content_raw: &self.content,
            license_raw: &self.license,
        }
    }
}

fn request() -> DomainPackCompositionRequestDocument {
    load_yaml(&format!("{FIXTURE_ROOT}/requests/agent-built-game.yaml"))
}

fn reference_namespace() -> String {
    ["reference", "game-development"].join(".")
}

fn reference_id(suffix: &str) -> StableId {
    StableId(format!("{}.{}", reference_namespace(), suffix))
}

fn material(candidate: &DomainPackCandidateInput) -> Material {
    let manifest = &candidate.manifest.domain_pack_manifest;
    Material {
        publisher: manifest.identity.publisher.0.clone(),
        name: manifest.identity.name.0.clone(),
        version: manifest.identity.version.clone(),
        manifest: read(&candidate.manifest_binding.artifact_ref.0),
        content: read(&manifest.content.content_ref.0),
        license: read(&manifest.provenance.license_text.artifact_ref.0),
    }
}

fn compose_reference(
    request: &DomainPackCompositionRequestDocument,
) -> forge_core_contracts::DomainPackCompositionProjectionDocument {
    let materials = request
        .domain_pack_composition_request
        .candidates
        .iter()
        .map(material)
        .collect::<Vec<_>>();
    let borrowed = materials.iter().map(Material::borrowed).collect::<Vec<_>>();
    compose_domain_packs(request, &borrowed)
}

#[test]
fn game_reference_pack_composes_and_removal_preserves_unknown_unknown_gaps() {
    let request = request();
    assert_eq!(
        request.domain_pack_composition_request.forge_core_version,
        "0.9.0"
    );
    let candidate = &request.domain_pack_composition_request.candidates[0];
    assert_eq!(
        candidate
            .manifest
            .domain_pack_manifest
            .compatibility
            .forge_core_requirement,
        ">=0.9,<1.0"
    );
    let owned = material(candidate);
    assert!(
        validate_domain_pack_candidate(candidate, &owned.borrowed(), "0.8.0")
            .iter()
            .any(|issue| issue.code == DomainPackCompositionIssueCode::IncompatibleForgeCore)
    );
    let lower_bound_issues = validate_domain_pack_candidate(candidate, &owned.borrowed(), "0.9.0");
    assert!(
        lower_bound_issues.is_empty(),
        "the reference candidate must be clean at its declared P6d lower bound: {lower_bound_issues:?}"
    );
    let projection = compose_reference(&request);
    let expected: forge_core_contracts::DomainPackCompositionProjectionDocument = load_yaml(
        &format!("{FIXTURE_ROOT}/projections/agent-built-game.expected.yaml"),
    );
    assert_eq!(
        projection, expected,
        "the complete effective composition projection is an exact reviewed reference surface"
    );
    let result = &projection.domain_pack_composition_projection;
    assert_eq!(
        result.status,
        DomainPackCompositionStatus::Composable,
        "issues={:?}; gaps={:?}",
        result.issues,
        result.gaps
    );
    assert!(result.issues.is_empty(), "{:?}", result.issues);
    assert!(result.gaps.is_empty(), "{:?}", result.gaps);
    assert_eq!(result.ordered_packs.len(), 1);
    assert_eq!(
        result.ordered_packs[0].identity.publisher.0,
        "forge.reference"
    );
    assert_eq!(result.ordered_packs[0].identity.name.0, "game-development");
    let bundle = result.composed_bundle.as_ref().expect("candidate bundle");
    assert_eq!(
        bundle
            .policies
            .iter()
            .filter(|policy| policy.id.0.starts_with(&reference_namespace()))
            .count(),
        4
    );

    let mut absent = request;
    absent.domain_pack_composition_request.candidates.clear();
    let projection = compose_domain_packs(&absent, &[]);
    let result = &projection.domain_pack_composition_projection;
    assert_eq!(result.status, DomainPackCompositionStatus::Blocked);
    assert!(
        result.issues.is_empty(),
        "absence is a gap, not malformed input: {:?}",
        result.issues
    );
    assert!(result
        .gaps
        .iter()
        .any(|gap| gap.code == DomainPackCompositionGapCode::MissingDomain));
    assert!(result
        .gaps
        .iter()
        .any(|gap| gap.code == DomainPackCompositionGapCode::MissingCapability));
    assert!(result
        .gaps
        .iter()
        .any(|gap| gap.subject_ref.0.ends_with("domain.first-use-validation")));
    assert!(result.gaps.iter().any(|gap| gap
        .subject_ref
        .0
        .ends_with("capability.packaging-toolchain")));
}

#[test]
fn hostile_core_namespace_candidate_is_rejected_by_generic_validation() {
    let candidate: DomainPackCandidateInput = load_yaml(&format!(
        "{FIXTURE_ROOT}/hostile/candidate-core-shadow.invalid.yaml"
    ));
    let owned = material(&candidate);
    let issues = validate_domain_pack_candidate(&candidate, &owned.borrowed(), "0.9.0");
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == DomainPackCompositionIssueCode::CoreShadow),
        "generic validator must reject a sealed-core namespace claim: {issues:?}"
    );
}

fn evidence(
    evidence_ref: &str,
    claim_suffix: &str,
    evaluator_suffix: &str,
    principal: &str,
    kind: WorkflowEvidenceKind,
    strength: WorkflowEvidenceStrength,
) -> WorkflowEvidenceObservation {
    WorkflowEvidenceObservation {
        evidence_ref: evidence_ref.to_owned(),
        claim_ref: reference_id(claim_suffix),
        evaluator_ref: reference_id(evaluator_suffix),
        principal: Some(PrincipalId(principal.to_owned())),
        kind,
        strength,
        freshness: WorkflowEvidenceFreshness::Current,
        outcome: WorkflowEvidenceOutcome::Pass,
    }
}

fn composed_bundle() -> WorkflowGovernanceBundleDocument {
    let projection = compose_reference(&request());
    WorkflowGovernanceBundleDocument {
        schema_version: WORKFLOW_GOVERNANCE_SCHEMA_VERSION.to_owned(),
        workflow_governance_bundle: projection
            .domain_pack_composition_projection
            .composed_bundle
            .expect("valid composed bundle"),
    }
}

fn evaluation(
    bundle: &WorkflowGovernanceBundleDocument,
    policy_suffix: &str,
    target: ReadinessTarget,
    completed_policy_refs: Vec<StableId>,
    evidence: Vec<WorkflowEvidenceObservation>,
    completion_assertion: WorkflowCompletionAssertion,
) -> WorkflowGovernanceEvaluationDocument {
    WorkflowGovernanceEvaluationDocument {
        schema_version: WORKFLOW_GOVERNANCE_SCHEMA_VERSION.to_owned(),
        workflow_governance_evaluation: WorkflowGovernanceEvaluation {
            observation_set_id: StableId(format!("observations.{policy_suffix}")),
            state_version: 1,
            observed_at_unix: 1_720_000_000,
            bundle_id: bundle.workflow_governance_bundle.id.clone(),
            policy_id: reference_id(policy_suffix),
            current_phase: StableId("4-build-verify".to_owned()),
            target,
            completed_policy_refs,
            not_applicable_policy_refs: vec![],
            available_capability_refs: vec![reference_id(&format!(
                "capability-requirement.{}",
                policy_suffix.trim_start_matches("policy.")
            ))],
            decision_need_refs: vec![],
            resolved_decision_refs: vec![],
            waivers: vec![],
            evidence,
            completion_assertion,
        },
    }
}

fn discovery_foundation_evidence() -> Vec<WorkflowEvidenceObservation> {
    vec![
        evidence(
            "discovery.player.1",
            "claim.discovery.player-context",
            "evaluator.discovery.player-context",
            "researcher.one",
            WorkflowEvidenceKind::Research,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ),
        evidence(
            "discovery.player.2",
            "claim.discovery.player-context",
            "evaluator.discovery.player-context",
            "researcher.two",
            WorkflowEvidenceKind::Research,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ),
        evidence(
            "discovery.production.1",
            "claim.discovery.production-feasibility",
            "evaluator.discovery.production-feasibility",
            "authority.one",
            WorkflowEvidenceKind::ExternalAuthority,
            WorkflowEvidenceStrength::AuthoritativeAcceptance,
        ),
        evidence(
            "discovery.production.2",
            "claim.discovery.production-feasibility",
            "evaluator.discovery.production-feasibility",
            "authority.two",
            WorkflowEvidenceKind::ExternalAuthority,
            WorkflowEvidenceStrength::AuthoritativeAcceptance,
        ),
    ]
}

fn discovery_risk_evidence() -> [WorkflowEvidenceObservation; 2] {
    [
        evidence(
            "discovery.risk.1",
            "claim.discovery.material-risks",
            "evaluator.discovery.material-risks",
            "reviewer.one",
            WorkflowEvidenceKind::IndependentReview,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ),
        evidence(
            "discovery.risk.2",
            "claim.discovery.material-risks",
            "evaluator.discovery.material-risks",
            "reviewer.two",
            WorkflowEvidenceKind::IndependentReview,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ),
    ]
}

#[test]
fn discovery_requires_all_independent_claims_before_asking_the_human() {
    let bundle = composed_bundle();
    let mut discovery = evaluation(
        &bundle,
        "policy.discovery",
        ReadinessTarget::Explore,
        vec![],
        vec![],
        WorkflowCompletionAssertion::NotAsserted,
    );
    discovery.workflow_governance_evaluation.current_phase = StableId("1-discovery".to_owned());

    let research_needed =
        simulate_workflow_governance(&bundle, &discovery).expect("valid discovery simulation");
    assert!(research_needed.candidate_decision_requests.is_empty());
    assert!(research_needed
        .candidate_next_actions
        .iter()
        .all(|action| action.kind != NextActionKind::AskHuman));

    discovery.workflow_governance_evaluation.evidence = discovery_foundation_evidence();
    let partial =
        simulate_workflow_governance(&bundle, &discovery).expect("valid partial discovery");
    assert!(partial.candidate_decision_requests.is_empty());
    assert!(partial.candidate_claim_results.iter().any(|claim| {
        claim.claim_id.ends_with("claim.discovery.material-risks")
            && claim.status != WorkflowClaimResultStatus::Verified
    }));
    assert!(partial
        .candidate_next_actions
        .iter()
        .all(|action| action.kind != NextActionKind::AskHuman));

    discovery
        .workflow_governance_evaluation
        .evidence
        .extend(discovery_risk_evidence());
    let direction_needed =
        simulate_workflow_governance(&bundle, &discovery).expect("all discovery evidence");
    assert!(direction_needed
        .candidate_claim_results
        .iter()
        .all(|claim| claim.status == WorkflowClaimResultStatus::Verified));
    assert_eq!(direction_needed.candidate_decision_requests.len(), 1);
    let decision_request = &direction_needed.candidate_decision_requests[0];
    assert_eq!(
        decision_request.reason,
        HumanDecisionReason::ProductDirection
    );
    assert!(decision_request.blocking);
    assert_eq!(decision_request.blocks_before, ReadinessTarget::Explore);
    assert_eq!(decision_request.alternatives.len(), 3);
    assert_eq!(
        decision_request.recommended_alternative_ref,
        reference_id("alternative.discovery.single-platform-vertical-slice")
    );
    assert!(direction_needed
        .candidate_next_actions
        .iter()
        .any(|action| action.kind == NextActionKind::AskHuman));
}

#[test]
fn first_use_requires_representative_session_and_independent_review() {
    let bundle = composed_bundle();
    let representative_fixture = repo_root().join(format!(
        "{FIXTURE_ROOT}/artifacts/representative-first-use-playtest.yaml"
    ));
    assert!(representative_fixture.is_file());

    let mut first_use = evaluation(
        &bundle,
        "policy.first-use-playtest",
        ReadinessTarget::Execute,
        vec![
            reference_id("policy.discovery"),
            reference_id("policy.playable-loop"),
        ],
        vec![
            evidence(
                "first-use.session.self-report",
                "claim.first-use-playtest.representative-session",
                "evaluator.first-use-playtest.representative-session",
                "producer.agent",
                WorkflowEvidenceKind::RepresentativeExecution,
                WorkflowEvidenceStrength::RepresentativeExecution,
            ),
            evidence(
                "first-use.review.self-report",
                "claim.first-use-playtest.independent-review",
                "evaluator.first-use-playtest.independent-review",
                "producer.agent",
                WorkflowEvidenceKind::IndependentReview,
                WorkflowEvidenceStrength::IndependentConfirmation,
            ),
        ],
        WorkflowCompletionAssertion::Asserted,
    );
    let partial = simulate_workflow_governance(&bundle, &first_use)
        .expect("single self-report remains structurally valid but insufficient");
    assert!(partial.candidate_claim_results.iter().any(|claim| {
        claim
            .claim_id
            .ends_with("claim.first-use-playtest.independent-review")
            && claim.status != WorkflowClaimResultStatus::Verified
    }));
    assert_eq!(
        partial.candidate_completion,
        WorkflowCompletionVerdict::Incomplete
    );
    first_use.workflow_governance_evaluation.evidence = vec![
        evidence(
            "first-use.session.executed",
            "claim.first-use-playtest.representative-session",
            "evaluator.first-use-playtest.representative-session",
            "representative.player",
            WorkflowEvidenceKind::RepresentativeExecution,
            WorkflowEvidenceStrength::RepresentativeExecution,
        ),
        evidence(
            "first-use.review.independent.1",
            "claim.first-use-playtest.independent-review",
            "evaluator.first-use-playtest.independent-review",
            "reviewer.one",
            WorkflowEvidenceKind::IndependentReview,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ),
        evidence(
            "first-use.review.independent.2",
            "claim.first-use-playtest.independent-review",
            "evaluator.first-use-playtest.independent-review",
            "reviewer.two",
            WorkflowEvidenceKind::IndependentReview,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ),
    ];
    let complete = simulate_workflow_governance(&bundle, &first_use)
        .expect("representative session plus independent review");
    assert!(complete
        .candidate_claim_results
        .iter()
        .all(|claim| claim.status == WorkflowClaimResultStatus::Verified));
    assert_eq!(
        complete.candidate_completion,
        WorkflowCompletionVerdict::Complete
    );
}

#[test]
fn packaging_requires_build_identity_installed_behavior_and_release_audit() {
    let bundle = composed_bundle();
    let mut packaging = evaluation(
        &bundle,
        "policy.packaging-readiness",
        ReadinessTarget::Release,
        vec![
            reference_id("policy.discovery"),
            reference_id("policy.playable-loop"),
            reference_id("policy.first-use-playtest"),
        ],
        vec![
            evidence(
                "package.build.1",
                "claim.packaging.clean-package-identity",
                "evaluator.packaging.clean-package-identity",
                "build.tool",
                WorkflowEvidenceKind::DeterministicCheck,
                WorkflowEvidenceStrength::DeterministicVerification,
            ),
            evidence(
                "package.build.2",
                "claim.packaging.clean-package-identity",
                "evaluator.packaging.clean-package-identity",
                "build.tool",
                WorkflowEvidenceKind::DeterministicCheck,
                WorkflowEvidenceStrength::DeterministicVerification,
            ),
        ],
        WorkflowCompletionAssertion::Asserted,
    );
    let artifact_only =
        simulate_workflow_governance(&bundle, &packaging).expect("artifact-only package evidence");
    assert_eq!(
        artifact_only.candidate_completion,
        WorkflowCompletionVerdict::Incomplete
    );
    assert_eq!(
        artifact_only
            .candidate_claim_results
            .iter()
            .filter(|claim| claim.status == WorkflowClaimResultStatus::Verified)
            .count(),
        1
    );

    packaging.workflow_governance_evaluation.evidence.extend([
        evidence(
            "package.runtime.1",
            "claim.packaging.installed-runtime-behavior",
            "evaluator.packaging.installed-runtime-behavior",
            "installer.one",
            WorkflowEvidenceKind::RepresentativeExecution,
            WorkflowEvidenceStrength::RepresentativeExecution,
        ),
        evidence(
            "package.runtime.2",
            "claim.packaging.installed-runtime-behavior",
            "evaluator.packaging.installed-runtime-behavior",
            "installer.two",
            WorkflowEvidenceKind::RepresentativeExecution,
            WorkflowEvidenceStrength::RepresentativeExecution,
        ),
        evidence(
            "package.audit.1",
            "claim.packaging.release-audit",
            "evaluator.packaging.release-audit",
            "release.reviewer.one",
            WorkflowEvidenceKind::IndependentReview,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ),
        evidence(
            "package.audit.2",
            "claim.packaging.release-audit",
            "evaluator.packaging.release-audit",
            "release.reviewer.two",
            WorkflowEvidenceKind::IndependentReview,
            WorkflowEvidenceStrength::IndependentConfirmation,
        ),
    ]);
    let complete =
        simulate_workflow_governance(&bundle, &packaging).expect("all independent packaging lanes");
    assert!(complete
        .candidate_claim_results
        .iter()
        .all(|claim| claim.status == WorkflowClaimResultStatus::Verified));
    assert_eq!(
        complete.candidate_completion,
        WorkflowCompletionVerdict::Complete
    );
}

#[test]
fn all_declared_artifact_bindings_match_exact_raw_and_canonical_bytes() {
    let request = request();
    let golden_path = "contracts/workflow-governance/golden-path-v0.yaml";
    let golden_raw = read(golden_path);
    let golden_document: WorkflowGovernanceBundleDocument = yaml_serde::from_str(
        std::str::from_utf8(&golden_raw).expect("golden-path genesis is UTF-8 YAML"),
    )
    .expect("typed golden-path genesis");
    let core = &request.domain_pack_composition_request.core;
    assert_eq!(
        sha256(&golden_raw),
        "sha256:5e0420ec70f7a25fa762d4f0f3d2dd025664876cae50bf91dc9fc9bd69900d45",
        "exact admitted genesis bytes drifted"
    );
    assert_eq!(
        canonical_digest(&golden_document),
        "sha256:af2a5a012fd3843d5d3686dc4e45bb295e91f60f1615a3040b22b1b0ec5423bb",
        "admitted genesis document digest drifted"
    );
    assert_eq!(
        canonical_digest(&golden_document.workflow_governance_bundle),
        "sha256:0a256f508294658539374e4e30f6ad128065a5d3c0dea8c6f69b8569cee454c3",
        "inner bundle digest drifted"
    );
    assert_eq!(
        canonical_digest(&golden_document.workflow_governance_bundle.policies),
        "sha256:399a204c1f47fd69e6e5596e320b6d13feb7872714a66c34adfc703569defb7d",
        "policy-set digest drifted"
    );
    assert_eq!(core.bundle, golden_document.workflow_governance_bundle);
    assert_eq!(
        core.bundle_id.0,
        "bundle.workflow-governance.golden-path-v0"
    );
    assert_eq!(
        core.bundle_digest,
        canonical_digest(&core.bundle),
        "Domain Pack composition binds the inner bundle semantics"
    );
    assert_eq!(
        core.policy_set_digest,
        canonical_digest(&core.bundle.policies)
    );

    let candidate = &request.domain_pack_composition_request.candidates[0];
    let manifest = &candidate.manifest.domain_pack_manifest;

    for binding in [
        &candidate.manifest_binding,
        &manifest.provenance.license_text,
    ] {
        let raw = read(&binding.artifact_ref.0);
        let value: serde_json::Value = yaml_serde::from_str(
            std::str::from_utf8(&raw).expect("artifact fixture is UTF-8 YAML"),
        )
        .expect("artifact fixture parses");
        assert_eq!(
            sha256(&raw),
            binding.raw_sha256,
            "{}",
            binding.artifact_ref.0
        );
        assert_eq!(
            canonical_digest(&value),
            binding.canonical_sha256,
            "{}",
            binding.artifact_ref.0
        );
    }

    let content_raw = read(&manifest.content.content_ref.0);
    assert_eq!(sha256(&content_raw), manifest.content.raw_sha256);
    assert_eq!(
        canonical_digest(&candidate.content),
        manifest.content.canonical_sha256
    );

    for fixture in &candidate.content.domain_pack_content.fixtures {
        let binding = &fixture.artifact;
        let raw = read(&binding.artifact_ref.0);
        let value: serde_json::Value =
            yaml_serde::from_str(std::str::from_utf8(&raw).expect("fixture is UTF-8 YAML"))
                .expect("fixture parses");
        assert_eq!(sha256(&raw), binding.raw_sha256, "{}", fixture.id.0);
        assert_eq!(
            canonical_digest(&value),
            binding.canonical_sha256,
            "{}",
            fixture.id.0
        );
    }
}

fn rust_sources_under(path: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(path).expect("read source tree") {
        let entry = entry.expect("source entry");
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "tests") {
                continue;
            }
            rust_sources_under(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn reference_domain_has_no_special_case_in_core_rust() {
    // Assemble the needles so this proof can safely scan its own source file
    // without excluding a literal that could hide a special case beside it.
    let namespace = reference_namespace();
    let domain = format!("{namespace}.domain.first-use-validation");
    let coordinate = ["forge", "reference", "game-development"].join("::");
    let mut sources = Vec::new();
    rust_sources_under(&repo_root().join("crates"), &mut sources);
    let offenders = sources
        .into_iter()
        .filter(|path| {
            let source = std::fs::read_to_string(path).expect("Rust source is UTF-8");
            source.contains(&namespace) || source.contains(&domain) || source.contains(&coordinate)
        })
        .collect::<Vec<_>>();
    assert!(
        offenders.is_empty(),
        "reference-pack semantics must remain generic data, found special cases in {offenders:?}"
    );
}
