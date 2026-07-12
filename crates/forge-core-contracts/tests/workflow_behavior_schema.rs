use forge_core_contracts::{
    RepoPath, StableId, WorkflowBehavioralArtifactReference,
    WorkflowBehavioralContinuationIdentity, WorkflowBehavioralCorpusClass,
    WorkflowBehavioralCorpusSet, WorkflowBehavioralCorpusSetDocument,
    WorkflowBehavioralCoveragePolicy, WorkflowBehavioralCoveragePolicyDocument,
    WorkflowBehavioralDisposition, WorkflowBehavioralEvaluatorIdentity,
    WorkflowBehavioralEvidenceAuthority, WorkflowBehavioralEvidenceBindings,
    WorkflowBehavioralGovernanceInput, WorkflowBehavioralRawSourceKind,
    WorkflowBehavioralRawSourceReference, WorkflowBehavioralScenario,
    WorkflowBehavioralScenarioCorpus, WorkflowBehavioralScenarioCorpusDocument,
    WorkflowBehavioralScenarioExecution, WorkflowBehavioralScenarioKind,
    WorkflowBehavioralShadowReport, WorkflowBehavioralShadowReportDocument,
    WorkflowBehavioralVerdict, WorkflowBehavioralWorkflowCorpus, WorkflowCompletionAssertion,
    WorkflowGovernanceEvaluation, WorkflowGovernanceEvaluationDocument, WorkflowGovernedCompletion,
    WorkflowGovernedEligibility, WorkflowGovernedOutcome, WorkflowGovernedOutcomeDimension,
    WorkflowGovernedProgression, WorkflowGovernedStatus,
    WORKFLOW_BEHAVIORAL_CORPUS_SET_SCHEMA_VERSION,
    WORKFLOW_BEHAVIORAL_COVERAGE_POLICY_SCHEMA_VERSION,
    WORKFLOW_BEHAVIORAL_MINIMUM_SCENARIOS_PER_KIND,
    WORKFLOW_BEHAVIORAL_MINIMUM_SCENARIOS_PER_WORKFLOW,
    WORKFLOW_BEHAVIORAL_REQUIRED_COVERAGE_BASIS_POINTS,
    WORKFLOW_BEHAVIORAL_SCENARIO_CORPUS_SCHEMA_VERSION,
    WORKFLOW_BEHAVIORAL_SHADOW_REPORT_SCHEMA_VERSION,
};
use schemars::schema_for;

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn artifact(value: &str, byte: char) -> WorkflowBehavioralArtifactReference {
    WorkflowBehavioralArtifactReference {
        id: id(value),
        embedded_ref: RepoPath(format!("contracts/{value}.yaml")),
        expected_digest: digest(byte),
    }
}

fn coverage_policy() -> WorkflowBehavioralCoveragePolicyDocument {
    WorkflowBehavioralCoveragePolicyDocument {
        schema_version: WORKFLOW_BEHAVIORAL_COVERAGE_POLICY_SCHEMA_VERSION.to_owned(),
        workflow_behavioral_coverage_policy: WorkflowBehavioralCoveragePolicy {
            id: id("coverage.behavioral.complete-v0"),
            policy_version: "0.1.0".to_owned(),
            authority: WorkflowBehavioralEvidenceAuthority::NonAuthoritativeShadowEvidence,
            required_scenario_kinds: WorkflowBehavioralScenarioKind::all().to_vec(),
            minimum_scenarios_per_kind: WORKFLOW_BEHAVIORAL_MINIMUM_SCENARIOS_PER_KIND,
            minimum_scenarios_per_workflow: WORKFLOW_BEHAVIORAL_MINIMUM_SCENARIOS_PER_WORKFLOW,
            required_coverage_basis_points: WORKFLOW_BEHAVIORAL_REQUIRED_COVERAGE_BASIS_POINTS,
            require_zero_mismatches: true,
            require_zero_evaluation_errors: true,
            required_dimensions: WorkflowGovernedOutcomeDimension::all().to_vec(),
            require_resume_equivalence: true,
            require_ablation_semantic_delta: true,
            require_representative_scenarios: true,
            require_adversarial_scenarios: true,
        },
    }
}

fn outcome() -> WorkflowGovernedOutcome {
    WorkflowGovernedOutcome {
        status: WorkflowGovernedStatus::Blocked,
        eligibility: WorkflowGovernedEligibility::Eligible,
        progression: WorkflowGovernedProgression::Blocked,
        completion: WorkflowGovernedCompletion::Incomplete,
        obligations: Vec::new(),
        claims: Vec::new(),
        decision_refs: Vec::new(),
        capability_refs: Vec::new(),
        issues: Vec::new(),
        next_actions: Vec::new(),
    }
}

fn governance_input(
    bundle: WorkflowBehavioralArtifactReference,
) -> WorkflowBehavioralGovernanceInput {
    WorkflowBehavioralGovernanceInput {
        bundle,
        evaluation: WorkflowGovernanceEvaluationDocument {
            schema_version: "0.1".to_owned(),
            workflow_governance_evaluation: WorkflowGovernanceEvaluation {
                observation_set_id: id("observations.scenario"),
                state_version: 1,
                observed_at_unix: 1_700_000_000,
                bundle_id: id("bundle.candidate"),
                policy_id: id("policy.workflow"),
                current_phase: id("2-spec"),
                target: forge_core_contracts::ReadinessTarget::Execute,
                completed_policy_refs: Vec::new(),
                not_applicable_policy_refs: Vec::new(),
                available_capability_refs: Vec::new(),
                decision_need_refs: Vec::new(),
                resolved_decision_refs: Vec::new(),
                waivers: Vec::new(),
                evidence: Vec::new(),
                completion_assertion: WorkflowCompletionAssertion::NotAsserted,
            },
        },
    }
}

fn bindings() -> WorkflowBehavioralEvidenceBindings {
    let review_subject = artifact("review-subject", '1');
    WorkflowBehavioralEvidenceBindings {
        review_subject: review_subject.clone(),
        review_subject_digest: digest('1'),
        workflow_id: id("workflow"),
        legacy_workflow_digest: digest('2'),
        policy_ref: id("policy.workflow"),
        policy_digest: digest('3'),
        candidate_bundle_id: id("bundle.candidate"),
        candidate_bundle_digest: digest('4'),
        candidate_bundle_source_digest: digest('4'),
        candidate_policy_set_digest: digest('5'),
        migration_batch_id: id("batch.candidate"),
        migration_batch_version: "0.1.0".to_owned(),
        governance_release_id: id("release.candidate"),
        governance_release_version: "0.2.0".to_owned(),
        predecessor_release_digest: digest('6'),
        coverage_policy_id: id("coverage.behavioral.complete-v0"),
        coverage_policy_digest: digest('7'),
        coverage_policy_source_digest: digest('7'),
        evaluator: WorkflowBehavioralEvaluatorIdentity {
            evaluator_id: id("evaluator.behavioral-shadow"),
            evaluator_version: "0.1.0".to_owned(),
            governed_projection_version: "0.1.0".to_owned(),
            evaluator_source_digest: digest('8'),
        },
        raw_sources: vec![WorkflowBehavioralRawSourceReference {
            kind: WorkflowBehavioralRawSourceKind::LegacyWorkflow,
            embedded_ref: RepoPath("contracts/legacy/workflow.yaml".to_owned()),
            expected_digest: digest('2'),
        }],
    }
}

fn continuation() -> WorkflowBehavioralContinuationIdentity {
    WorkflowBehavioralContinuationIdentity {
        ledger_digest: digest('a'),
        ledger_head_digest: digest('b'),
        snapshot_digest: digest('c'),
        active_release_id: id("release.candidate"),
        active_release_digest: digest('d'),
        runtime_bundle_id: id("bundle.candidate"),
        runtime_bundle_digest: digest('4'),
        state_version: 1,
        current_phase: id("2-spec"),
        observed_at_unix: 1_700_000_000,
    }
}

#[test]
fn closed_coverage_policy_accepts_only_the_complete_floor() {
    let valid = coverage_policy();
    assert!(valid.validate().is_empty());

    let mut weakened = valid.clone();
    let policy = &mut weakened.workflow_behavioral_coverage_policy;
    policy.minimum_scenarios_per_kind = 0;
    policy.minimum_scenarios_per_workflow = 6;
    policy.required_coverage_basis_points = 9_999;
    policy.required_scenario_kinds.pop();
    policy.required_dimensions.pop();
    policy.require_zero_mismatches = false;
    policy.require_resume_equivalence = false;
    assert!(weakened.validate().len() >= 7);
}

#[test]
fn corpus_set_is_typed_content_addressed_and_non_authoritative() {
    let set = WorkflowBehavioralCorpusSetDocument {
        schema_version: WORKFLOW_BEHAVIORAL_CORPUS_SET_SCHEMA_VERSION.to_owned(),
        workflow_behavioral_corpus_set: WorkflowBehavioralCorpusSet {
            id: id("corpus-set.core-assurance-v0"),
            corpus_set_version: "0.1.0".to_owned(),
            authority: WorkflowBehavioralEvidenceAuthority::NonAuthoritativeShadowEvidence,
            corpora: vec![
                artifact("corpus.representative", '1'),
                artifact("corpus.adversarial", '2'),
            ],
        },
    };
    assert!(set.validate().is_empty());

    let mut duplicate = set;
    duplicate
        .workflow_behavioral_corpus_set
        .corpora
        .push(artifact("corpus.representative", '3'));
    assert!(!duplicate.validate().is_empty());
}

#[test]
fn authority_and_verdicts_have_no_executable_or_admitted_wire_value() {
    let document = coverage_policy();
    let json = serde_json::to_string(&document).expect("serialize policy");
    assert!(json.contains("non_authoritative_shadow_evidence"));
    for elevated in ["admitted", "executable", "authoritative"] {
        let forged = json.replace("non_authoritative_shadow_evidence", elevated);
        assert!(serde_json::from_str::<WorkflowBehavioralCoveragePolicyDocument>(&forged).is_err());
    }

    let verdict_schema = serde_json::to_string(&schema_for!(WorkflowBehavioralVerdict))
        .expect("serialize verdict schema");
    for allowed in [
        "behaviorally_consistent_candidate",
        "insufficient_evidence",
        "mismatch_detected",
        "invalid_bindings",
    ] {
        assert!(verdict_schema.contains(allowed));
    }
    assert!(!verdict_schema.contains("admitted"));
    assert!(!verdict_schema.contains("executable"));
}

#[test]
fn all_documents_and_nested_execution_shapes_reject_unknown_fields() {
    let mut policy = serde_json::to_value(coverage_policy()).expect("policy JSON");
    policy["workflow_behavioral_coverage_policy"]["invented"] = serde_json::json!(true);
    assert!(serde_json::from_value::<WorkflowBehavioralCoveragePolicyDocument>(policy).is_err());

    let single = WorkflowBehavioralScenarioExecution::Single {
        input: Box::new(governance_input(artifact("bundle.candidate", '4'))),
        expected: Box::new(outcome()),
    };
    let mut execution = serde_json::to_value(single).expect("execution JSON");
    execution["agent_confidence"] = serde_json::json!(1.0);
    assert!(serde_json::from_value::<WorkflowBehavioralScenarioExecution>(execution).is_err());
}

#[test]
fn resume_and_ablation_are_closed_typed_recomputable_inputs() {
    let bundle = artifact("bundle.candidate", '4');
    let resumed = WorkflowBehavioralScenarioExecution::Resume {
        continuation: Box::new(continuation()),
        checkpoint_source: artifact("checkpoint.resume", '9'),
        checkpoint_digest: digest('9'),
        checkpoint_input: Box::new(governance_input(bundle.clone())),
        checkpoint_expected: Box::new(outcome()),
        resumed_input: Box::new(governance_input(bundle.clone())),
        resumed_expected: Box::new(outcome()),
        equivalence_dimensions: WorkflowGovernedOutcomeDimension::all().to_vec(),
    };
    let ablated = WorkflowBehavioralScenarioExecution::Ablation {
        control_input: Box::new(governance_input(bundle)),
        control_expected: Box::new(outcome()),
        ablated_input: Box::new(governance_input(artifact("bundle.ablated", 'a'))),
        ablated_expected: Box::new(outcome()),
        removed_semantic_refs: vec![id("claim.required")],
        required_difference_dimensions: vec![WorkflowGovernedOutcomeDimension::Claims],
    };
    for execution in [resumed, ablated] {
        let json = serde_json::to_string(&execution).expect("serialize execution");
        let decoded: WorkflowBehavioralScenarioExecution =
            serde_json::from_str(&json).expect("typed execution round trip");
        assert_eq!(decoded, execution);
        assert!(json.contains("workflow_governance_evaluation"));
        assert!(json.contains("expected_digest"));
    }
}

#[test]
fn corpus_validation_rejects_no_op_ablation_and_invalid_digest() {
    let bundle = artifact("bundle.candidate", '4');
    let mut bound = bindings();
    let mut scenarios = Vec::new();
    for (index, kind) in WorkflowBehavioralScenarioKind::all()
        .into_iter()
        .enumerate()
    {
        let execution = match kind {
            WorkflowBehavioralScenarioKind::Resume => WorkflowBehavioralScenarioExecution::Resume {
                continuation: Box::new(continuation()),
                checkpoint_source: artifact("checkpoint.resume", 'b'),
                checkpoint_digest: digest('b'),
                checkpoint_input: Box::new(governance_input(bundle.clone())),
                checkpoint_expected: Box::new(outcome()),
                resumed_input: Box::new(governance_input(bundle.clone())),
                resumed_expected: Box::new(outcome()),
                equivalence_dimensions: WorkflowGovernedOutcomeDimension::all().to_vec(),
            },
            WorkflowBehavioralScenarioKind::Ablation => {
                WorkflowBehavioralScenarioExecution::Ablation {
                    control_input: Box::new(governance_input(bundle.clone())),
                    control_expected: Box::new(outcome()),
                    ablated_input: Box::new(governance_input(bundle.clone())),
                    ablated_expected: Box::new(outcome()),
                    removed_semantic_refs: vec![id("claim.required")],
                    required_difference_dimensions: vec![WorkflowGovernedOutcomeDimension::Claims],
                }
            }
            _ => WorkflowBehavioralScenarioExecution::Single {
                input: Box::new(governance_input(bundle.clone())),
                expected: Box::new(outcome()),
            },
        };
        scenarios.push(WorkflowBehavioralScenario {
            scenario_id: id(&format!("scenario-{index}")),
            scenario_kind: kind,
            corpus_class: WorkflowBehavioralCorpusClass::Adversarial,
            execution_input_digest: digest(char::from(
                b'a' + u8::try_from(index).expect("seven scenarios fit in u8"),
            )),
            execution,
        });
    }
    bound.policy_digest = format!("sha256:{}", "A".repeat(64));
    let document = WorkflowBehavioralScenarioCorpusDocument {
        schema_version: WORKFLOW_BEHAVIORAL_SCENARIO_CORPUS_SCHEMA_VERSION.to_owned(),
        workflow_behavioral_scenario_corpus: WorkflowBehavioralScenarioCorpus {
            id: id("corpus.review-v0"),
            corpus_version: "0.1.0".to_owned(),
            authority: WorkflowBehavioralEvidenceAuthority::NonAuthoritativeShadowEvidence,
            partition_class: WorkflowBehavioralCorpusClass::Adversarial,
            coverage_policy: WorkflowBehavioralArtifactReference {
                id: bound.coverage_policy_id.clone(),
                embedded_ref: RepoPath("contracts/coverage.yaml".to_owned()),
                expected_digest: bound.coverage_policy_source_digest.clone(),
            },
            workflow_evidence: vec![WorkflowBehavioralWorkflowCorpus {
                bindings: bound,
                scenarios,
            }],
        },
    };
    let issues = document.validate();
    assert!(issues
        .iter()
        .any(|issue| issue.path.ends_with("policy_digest")));
    assert!(issues
        .iter()
        .any(|issue| issue.message.contains("ablation input")));
}

#[test]
fn a_raw_pass_claim_cannot_validate_as_a_review_candidate() {
    let report = WorkflowBehavioralShadowReportDocument {
        schema_version: WORKFLOW_BEHAVIORAL_SHADOW_REPORT_SCHEMA_VERSION.to_owned(),
        workflow_behavioral_shadow_report: WorkflowBehavioralShadowReport {
            id: id("report.review-v0"),
            report_version: "0.1.0".to_owned(),
            authority: WorkflowBehavioralEvidenceAuthority::NonAuthoritativeShadowEvidence,
            corpus: artifact("corpus.review-v0", 'c'),
            coverage_policy: artifact("coverage.behavioral.complete-v0", '7'),
            workflow_reports: Vec::new(),
            verdict: WorkflowBehavioralVerdict::BehaviorallyConsistentCandidate,
            disposition: WorkflowBehavioralDisposition::ReviewCandidate,
        },
    };
    let issues = report.validate();
    assert!(issues
        .iter()
        .any(|issue| issue.path == "report.workflow_reports"));
    assert!(issues.iter().any(|issue| issue.path == "report.verdict"));
}
