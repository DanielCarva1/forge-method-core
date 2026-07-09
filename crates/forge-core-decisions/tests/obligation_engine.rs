use forge_core_contracts::{
    AssuranceClaimStatus, DecisionAlternative, HumanDecisionReason, ReadinessTarget,
    ReadinessVerdict, StableId,
};
use forge_core_decisions::{
    derive_assurance_case, DecisionNeed, LensApplicability, ObligationEngineInputDocument,
    ObligationEngineIssue, UniversalAssuranceLens,
};
use forge_core_validate::validate_assurance_case;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

fn load_fixture(name: &str) -> ObligationEngineInputDocument {
    let path = repo_root()
        .join("docs/fixtures/obligation-engine-v0")
        .join(name);
    let text = fs::read_to_string(&path).expect("read Obligation Engine fixture");
    yaml_serde::from_str(&text)
        .unwrap_or_else(|error| panic!("deserialize {}: {error}", path.display()))
}

fn assert_generated_case_is_valid(document: &ObligationEngineInputDocument) {
    let output = derive_assurance_case(document).expect("derive Assurance Case");
    let report = validate_assurance_case(&output);
    assert!(
        !report.has_errors(),
        "generated Assurance Case must pass P1 validation: {:#?}",
        report.diagnostics()
    );
}

#[test]
fn every_p2_fixture_derives_a_semantically_valid_assurance_case() {
    for name in [
        "novel-domain-execute.yaml",
        "artifact-only-release.yaml",
        "waived-release.yaml",
        "verified-release.yaml",
        "chat-goal-explore.yaml",
    ] {
        assert_generated_case_is_valid(&load_fixture(name));
    }
}

#[test]
fn novel_domain_blocks_execute_but_allows_exploration() {
    let execute_input = load_fixture("novel-domain-execute.yaml");
    let execute = derive_assurance_case(&execute_input).expect("derive execute case");

    assert_eq!(
        execute.assurance_case.readiness.verdict,
        ReadinessVerdict::Blocked
    );
    assert!(execute
        .assurance_case
        .capability_gaps
        .iter()
        .any(|gap| gap.id.0.contains("domain_pack")));
    assert!(execute
        .assurance_case
        .obligations
        .iter()
        .any(|obligation| obligation.id.0.contains("domain_method_is_credible")));

    let mut explore_input = execute_input;
    explore_input.obligation_engine_input.target = ReadinessTarget::Explore;
    let explore = derive_assurance_case(&explore_input).expect("derive exploration case");

    assert_eq!(
        explore.assurance_case.readiness.verdict,
        ReadinessVerdict::Ready
    );
    assert!(!explore.assurance_case.capability_gaps.is_empty());
    assert_eq!(
        explore.assurance_case.next_actions[0].id.0,
        "action.proceed.explore"
    );
    assert_generated_case_is_valid(&explore_input);
}

#[test]
fn artifact_presence_does_not_verify_integrated_readiness() {
    let input = load_fixture("artifact-only-release.yaml");
    let output = derive_assurance_case(&input).expect("derive artifact-only case");

    assert_eq!(
        output.assurance_case.readiness.verdict,
        ReadinessVerdict::Blocked
    );
    assert!(output.assurance_case.claims.iter().any(|claim| {
        claim.id.0 == "claim.assurance.critical_journeys"
            && claim.status == AssuranceClaimStatus::Supported
    }));
    assert!(output.assurance_case.obligations.iter().any(|obligation| {
        obligation
            .id
            .0
            .contains("assurance_case_survives_independent_challenge")
    }));
}

#[test]
fn explicit_waiver_can_satisfy_one_claim_without_hiding_consequences() {
    let input = load_fixture("waived-release.yaml");
    let output = derive_assurance_case(&input).expect("derive waived case");

    assert_eq!(
        output.assurance_case.readiness.verdict,
        ReadinessVerdict::Ready
    );
    let claim = output
        .assurance_case
        .claims
        .iter()
        .find(|claim| claim.id.0 == "claim.assurance.quality_attributes")
        .expect("quality claim");
    assert_eq!(claim.status, AssuranceClaimStatus::Waived);
    assert!(claim
        .waiver
        .as_ref()
        .is_some_and(|waiver| !waiver.consequences.is_empty()));
}

#[test]
fn verified_release_is_ready_and_proceed_is_ranked_first() {
    let input = load_fixture("verified-release.yaml");
    let output = derive_assurance_case(&input).expect("derive verified case");

    assert_eq!(
        output.assurance_case.readiness.verdict,
        ReadinessVerdict::Ready
    );
    assert!(output.assurance_case.readiness.blocker_refs.is_empty());
    assert_eq!(
        output.assurance_case.next_actions[0].id.0,
        "action.proceed.release"
    );
    assert_eq!(output.assurance_case.next_actions[0].rank, 1);
}

#[test]
fn same_input_produces_byte_equivalent_yaml_output() {
    let input = load_fixture("artifact-only-release.yaml");
    let first = derive_assurance_case(&input).expect("first derivation");
    let second = derive_assurance_case(&input).expect("second derivation");

    assert_eq!(first, second);
    assert_eq!(
        yaml_serde::to_string(&first).expect("serialize first"),
        yaml_serde::to_string(&second).expect("serialize second")
    );
}

#[test]
fn invalid_host_proposal_accumulates_multiple_issues() {
    let mut input = load_fixture("verified-release.yaml");
    input.schema_version = "999".to_owned();
    input
        .obligation_engine_input
        .lens_observations
        .push(input.obligation_engine_input.lens_observations[0].clone());
    input.obligation_engine_input.lens_observations[0].applicability =
        LensApplicability::NotApplicable;
    input.obligation_engine_input.lens_observations[0].rationale = None;

    let rejection = derive_assurance_case(&input).expect_err("invalid input must be rejected");

    assert!(
        rejection.issues.len() >= 3,
        "issues: {:#?}",
        rejection.issues
    );
    assert!(rejection.issues.iter().any(|issue| matches!(
        issue,
        ObligationEngineIssue::UnsupportedSchemaVersion { .. }
    )));
    assert!(rejection.issues.iter().any(|issue| matches!(
        issue,
        ObligationEngineIssue::DuplicateLensObservation { .. }
    )));
    assert!(rejection
        .issues
        .contains(&ObligationEngineIssue::IntendedOutcomeNotApplicable));
}

#[test]
fn irreducible_human_decision_is_mapped_and_blocks_only_at_its_target() {
    let mut input = load_fixture("verified-release.yaml");
    input
        .obligation_engine_input
        .decision_needs
        .push(DecisionNeed {
            id: StableId("decision.release_tradeoff".to_owned()),
            question: "Prefer an earlier internal release or broader compatibility?".to_owned(),
            reason: HumanDecisionReason::ProductDirection,
            alternatives: vec![
                DecisionAlternative {
                    id: StableId("alternative.early".to_owned()),
                    description: "release internally now".to_owned(),
                    consequences: vec!["external compatibility remains deferred".to_owned()],
                },
                DecisionAlternative {
                    id: StableId("alternative.compatibility".to_owned()),
                    description: "wait for broader compatibility".to_owned(),
                    consequences: vec!["release occurs later".to_owned()],
                },
            ],
            recommended_alternative_ref: StableId("alternative.early".to_owned()),
            affected_lenses: vec![UniversalAssuranceLens::IntendedOutcome],
            blocking: true,
            blocks_before: ReadinessTarget::Release,
        });

    let release = derive_assurance_case(&input).expect("derive decision case");
    assert_eq!(
        release.assurance_case.readiness.verdict,
        ReadinessVerdict::Blocked
    );
    assert_eq!(release.assurance_case.decision_requests.len(), 1);
    assert_generated_case_is_valid(&input);

    input.obligation_engine_input.target = ReadinessTarget::Execute;
    let execute = derive_assurance_case(&input).expect("derive pre-decision execute case");
    assert_eq!(
        execute.assurance_case.readiness.verdict,
        ReadinessVerdict::Ready
    );
}
