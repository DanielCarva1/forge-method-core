use forge_core_contracts::{
    AssuranceCaseDocument, AssuranceClaimStatus, AssuranceWaiver, DecisionAlternative,
    DecisionRequest, HumanDecisionReason, ObligationStatus, PrincipalId, ReadinessTarget,
    ReadinessVerdict, StableId,
};
use forge_core_validate::{validate_assurance_case, DiagnosticCode};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

fn load_fixture(name: &str) -> AssuranceCaseDocument {
    let path = repo_root().join("contracts/assurance").join(name);
    let text = fs::read_to_string(&path).expect("read Assurance Case fixture");
    yaml_serde::from_str(&text)
        .unwrap_or_else(|error| panic!("deserialize {}: {error}", path.display()))
}

#[test]
fn acceptance_fixtures_are_semantically_valid() {
    for name in [
        "underspecified-novel-domain-assurance.yaml",
        "artifact-only-progress-assurance.yaml",
        "representative-slice-verified-assurance.yaml",
    ] {
        let document = load_fixture(name);
        let report = validate_assurance_case(&document);

        assert!(
            !report.has_errors(),
            "{name} must be valid: {:#?}",
            report.diagnostics()
        );
    }
}

#[test]
fn fixtures_distinguish_blocked_progress_from_verified_readiness() {
    let novel = load_fixture("underspecified-novel-domain-assurance.yaml");
    let artifact_only = load_fixture("artifact-only-progress-assurance.yaml");
    let verified = load_fixture("representative-slice-verified-assurance.yaml");

    assert_eq!(
        novel.assurance_case.readiness.verdict,
        ReadinessVerdict::Blocked
    );
    assert_eq!(
        novel.assurance_case.readiness.target,
        ReadinessTarget::Execute
    );
    assert_eq!(
        artifact_only.assurance_case.readiness.verdict,
        ReadinessVerdict::Blocked
    );
    assert_eq!(
        verified.assurance_case.readiness.verdict,
        ReadinessVerdict::Ready
    );
    assert_eq!(
        verified.assurance_case.readiness.target,
        ReadinessTarget::Release
    );
}

#[test]
fn unsupported_schema_version_is_rejected_semantically() {
    let mut document = load_fixture("representative-slice-verified-assurance.yaml");
    document.schema_version = "999".to_owned();

    let report = validate_assurance_case(&document);

    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::AssuranceUnsupportedSchemaVersion
    }));
}

#[test]
fn evidence_bearing_claim_status_requires_evidence() {
    let mut document = load_fixture("representative-slice-verified-assurance.yaml");
    document.assurance_case.claims[0].evidence_refs.clear();

    let report = validate_assurance_case(&document);

    assert!(report
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::AssuranceClaimEvidenceMissing));
}

#[test]
fn waiver_is_explicit_and_only_valid_for_waived_claims() {
    let mut document = load_fixture("artifact-only-progress-assurance.yaml");
    document.assurance_case.claims[0].waiver = Some(AssuranceWaiver {
        authorized_by: PrincipalId("human.owner".to_owned()),
        reason: "accept uncertainty for a reversible prototype".to_owned(),
        consequences: vec!["release remains prohibited".to_owned()],
        expires_at: None,
    });

    let report = validate_assurance_case(&document);
    assert!(report
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::AssuranceWaiverInconsistent));

    document.assurance_case.claims[0].status = AssuranceClaimStatus::Waived;
    let report = validate_assurance_case(&document);
    assert!(!report
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::AssuranceWaiverInconsistent));
}

#[test]
fn satisfied_obligation_requires_verified_or_waived_claims() {
    let mut document = load_fixture("artifact-only-progress-assurance.yaml");
    document.assurance_case.obligations[0].status = ObligationStatus::Satisfied;

    let report = validate_assurance_case(&document);

    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::AssuranceSatisfiedObligationUnsupported
    }));
}

#[test]
fn ready_verdict_rejects_due_unsatisfied_obligations_and_blocking_gaps() {
    let mut document = load_fixture("underspecified-novel-domain-assurance.yaml");
    document.assurance_case.readiness.verdict = ReadinessVerdict::Ready;
    document.assurance_case.readiness.blocker_refs.clear();

    let report = validate_assurance_case(&document);

    assert!(report
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::AssuranceReadinessInconsistent));
}

#[test]
fn decision_request_requires_two_options_and_a_resolving_recommendation() {
    let mut document = load_fixture("artifact-only-progress-assurance.yaml");
    document
        .assurance_case
        .decision_requests
        .push(DecisionRequest {
            id: StableId("decision.release_priority".to_owned()),
            question: "Which outcome matters more?".to_owned(),
            reason: HumanDecisionReason::Preference,
            alternatives: vec![DecisionAlternative {
                id: StableId("alternative.speed".to_owned()),
                description: "ship sooner".to_owned(),
                consequences: vec!["lower scope".to_owned()],
            }],
            recommended_alternative_ref: StableId("alternative.quality".to_owned()),
            blocking: false,
            blocks_before: ReadinessTarget::Release,
        });

    let report = validate_assurance_case(&document);

    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::AssuranceDecisionRecommendationInvalid
    }));
}

#[test]
fn dangling_claim_refs_and_noncontiguous_ranks_are_rejected() {
    let mut document = load_fixture("artifact-only-progress-assurance.yaml");
    document.assurance_case.next_actions[0]
        .addresses_claim_refs
        .push(StableId("claim.missing".to_owned()));
    document.assurance_case.next_actions[1].rank = 3;

    let report = validate_assurance_case(&document);

    assert!(report
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::AssuranceDanglingClaimRef));
    assert!(report
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::AssuranceNextActionRankInvalid));
}
