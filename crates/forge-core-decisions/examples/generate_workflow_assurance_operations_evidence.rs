use forge_core_contracts::{
    PrincipalId, ReadinessTarget, RepoPath, StableId, WorkflowBehavioralArtifactReference,
    WorkflowBehavioralContinuationIdentity, WorkflowBehavioralCorpusClass,
    WorkflowBehavioralCorpusSet, WorkflowBehavioralCorpusSetDocument,
    WorkflowBehavioralCoveragePolicy, WorkflowBehavioralCoveragePolicyDocument,
    WorkflowBehavioralDisposition, WorkflowBehavioralEvidenceAuthority,
    WorkflowBehavioralEvidenceBindings, WorkflowBehavioralGovernanceInput,
    WorkflowBehavioralRawSourceKind, WorkflowBehavioralRawSourceReference,
    WorkflowBehavioralReviewSubjectDocument, WorkflowBehavioralScenario,
    WorkflowBehavioralScenarioCorpus, WorkflowBehavioralScenarioCorpusDocument,
    WorkflowBehavioralScenarioExecution, WorkflowBehavioralScenarioKind, WorkflowBehavioralVerdict,
    WorkflowBehavioralWorkflowCorpus, WorkflowCompletionAssertion, WorkflowEvidenceFreshness,
    WorkflowEvidenceObservation, WorkflowEvidenceOutcome, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceEvaluation, WorkflowGovernanceEvaluationDocument, WorkflowGovernanceEvent,
    WorkflowGovernancePolicy, WorkflowGovernanceReceiptDocument,
    WorkflowGovernanceReleaseRegistryDocument, WorkflowGovernedOutcome,
    WorkflowGovernedOutcomeDimension, WORKFLOW_BEHAVIORAL_CORPUS_SET_SCHEMA_VERSION,
    WORKFLOW_BEHAVIORAL_COVERAGE_POLICY_SCHEMA_VERSION,
    WORKFLOW_BEHAVIORAL_MINIMUM_SCENARIOS_PER_KIND,
    WORKFLOW_BEHAVIORAL_MINIMUM_SCENARIOS_PER_WORKFLOW,
    WORKFLOW_BEHAVIORAL_REQUIRED_COVERAGE_BASIS_POINTS,
    WORKFLOW_BEHAVIORAL_SCENARIO_CORPUS_SCHEMA_VERSION,
};
use forge_core_decisions::{
    derive_workflow_governed_outcome, evaluate_workflow_behavior,
    validate_workflow_governance_bundle, workflow_behavior_execution_input_digest,
    workflow_policy_set_digest, workflow_release_policy_digest, workflow_runtime_bundle_digest,
    WorkflowBehavioralBundleInput, WorkflowBehavioralCorpusInput, WorkflowBehavioralReportIdentity,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::path::{Path, PathBuf};

const COVERAGE_PATH: &str =
    "contracts/policies/workflow-behavioral-coverage-assurance-operations-v0.yaml";
const REVIEW_SUBJECT_PATH: &str =
    "contracts/migration/workflow-assurance-operations-review-subject-v0.yaml";
const RUNTIME_BUNDLE_PATH: &str =
    "contracts/workflow-governance/runtime-assurance-operations-candidate-v0.yaml";
const OVERLAY_PATH: &str = "contracts/policies/workflow-assurance-operations-overlay-v0.yaml";
const EVALUATOR_PATH: &str = "crates/forge-core-decisions/src/workflow_behavior.rs";
const BASELINE_HISTORY_PATH: &str =
    "contracts/evidence/workflow-core-assurance-frozen-history-v0.ndjson";
const PREDECESSOR_REGISTRY_PATH: &str =
    "contracts/migration/workflow-governance-release-registry-core-assurance-v0.yaml";
const REPRESENTATIVE_PATH: &str =
    "contracts/evidence/workflow-assurance-operations-representative-v0.yaml";
const ADVERSARIAL_PATH: &str =
    "contracts/evidence/workflow-assurance-operations-adversarial-v0.yaml";
const CORPUS_SET_PATH: &str = "contracts/evidence/workflow-assurance-operations-corpus-set-v0.yaml";
const SHADOW_REPORT_PATH: &str =
    "contracts/evidence/workflow-assurance-operations-shadow-report-v0.yaml";
const WORKFLOW_IDS: [&str; 13] = [
    "investigation",
    "platform-ops-plan",
    "security-plan",
    "privacy-data-plan",
    "test-framework",
    "atdd-plan",
    "eval-design",
    "test-automation",
    "test-review",
    "ci-quality-pipeline",
    "observability-plan",
    "devops-deployment-plan",
    "compliance-checklist",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
}

struct GeneratedArtifact {
    relative_path: String,
    bytes: Vec<u8>,
}

struct SourceMaterial {
    path: &'static str,
    bytes: Vec<u8>,
}

struct EvidenceContext {
    subject: WorkflowBehavioralReviewSubjectDocument,
    subject_artifact: WorkflowBehavioralArtifactReference,
    subject_digest: String,
    runtime: WorkflowGovernanceBundleDocument,
    runtime_artifact: WorkflowBehavioralArtifactReference,
    runtime_digest: String,
    policy_set_digest: String,
    coverage: WorkflowBehavioralCoveragePolicyDocument,
    coverage_artifact: WorkflowBehavioralArtifactReference,
    coverage_digest: String,
    sources: HashMap<RepoPath, Vec<u8>>,
    overlay: SourceMaterial,
    evaluator: SourceMaterial,
    baseline: FrozenBaseline,
}

struct FrozenBaseline {
    history_digest: String,
    ledger_head_digest: String,
    snapshot_digest: String,
    active_release_id: StableId,
    active_release_digest: String,
    runtime_bundle_id: StableId,
    runtime_bundle_digest: String,
    state_version: u64,
    current_phase: StableId,
    observed_at_unix: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mode = parse_mode()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let artifacts = generate(&root)?;
    match mode {
        Mode::Write => write_artifacts(&root, &artifacts),
        Mode::Check => check_artifacts(&root, &artifacts),
    }
}

fn parse_mode() -> Result<Mode, Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [flag] if flag == "--write" => Ok(Mode::Write),
        [flag] if flag == "--check" => Ok(Mode::Check),
        _ => Err(error(
            "usage: cargo run -p forge-core-decisions --example generate_workflow_assurance_operations_evidence -- (--write|--check)",
        )),
    }
}

// Generation order is intentionally explicit because every later artifact is
// content-addressed to earlier bytes; splitting it would hide digest ordering.
#[allow(clippy::too_many_lines)]
fn generate(root: &Path) -> Result<Vec<GeneratedArtifact>, Box<dyn Error>> {
    let mut context = load_context(root)?;
    let mut bundle_inputs = BTreeMap::new();
    bundle_inputs.insert(
        context.runtime_digest.clone(),
        WorkflowBehavioralBundleInput {
            artifact: context.runtime_artifact.clone(),
            document: context.runtime.clone(),
        },
    );

    let mut ablated_artifacts = Vec::new();
    let mut checkpoint_artifacts = Vec::new();
    let mut representative = Vec::new();
    let mut adversarial = Vec::new();
    for workflow_id in WORKFLOW_IDS {
        let policy = context
            .runtime
            .workflow_governance_bundle
            .policies
            .iter()
            .find(|policy| policy.compatibility_workflow_id.0 == workflow_id)
            .ok_or_else(|| error(format!("missing candidate policy for {workflow_id}")))?
            .clone();
        let (ablated, removed, selected_claim) = ablate_policy(&context.runtime, &policy)?;
        let ablated_path = ablated_path(workflow_id);
        let ablated_bytes = yaml_bytes(&ablated)?;
        let ablated_artifact = WorkflowBehavioralArtifactReference {
            id: ablated.workflow_governance_bundle.id.clone(),
            embedded_ref: RepoPath(ablated_path.clone()),
            expected_digest: sha256(&ablated_bytes),
        };
        let ablated_digest = workflow_runtime_bundle_digest(&ablated).map_err(error)?;
        context
            .sources
            .insert(ablated_artifact.embedded_ref.clone(), ablated_bytes.clone());
        bundle_inputs.insert(
            ablated_digest,
            WorkflowBehavioralBundleInput {
                artifact: ablated_artifact.clone(),
                document: ablated.clone(),
            },
        );
        ablated_artifacts.push(GeneratedArtifact {
            relative_path: ablated_path,
            bytes: ablated_bytes,
        });

        let bindings = bindings(root, &context, &policy)?;
        let (resume, checkpoint_artifact) = resume_scenario(&mut context, &policy)?;
        checkpoint_artifacts.push(checkpoint_artifact);
        representative.push(WorkflowBehavioralWorkflowCorpus {
            bindings: bindings.clone(),
            scenarios: vec![
                single_scenario(
                    &context,
                    &policy,
                    WorkflowBehavioralScenarioKind::Positive,
                    ScenarioEvidence::Passing,
                )?,
                resume,
            ],
        });
        adversarial.push(WorkflowBehavioralWorkflowCorpus {
            bindings,
            scenarios: vec![
                single_scenario(
                    &context,
                    &policy,
                    WorkflowBehavioralScenarioKind::Negative,
                    ScenarioEvidence::Failing,
                )?,
                single_scenario(
                    &context,
                    &policy,
                    WorkflowBehavioralScenarioKind::Ambiguity,
                    ScenarioEvidence::Inconclusive,
                )?,
                single_scenario(
                    &context,
                    &policy,
                    WorkflowBehavioralScenarioKind::FalseCompletion,
                    ScenarioEvidence::FalseCompletion,
                )?,
                single_scenario(
                    &context,
                    &policy,
                    WorkflowBehavioralScenarioKind::StaleEvidence,
                    ScenarioEvidence::Stale,
                )?,
                ablation_scenario(
                    &context,
                    &policy,
                    &selected_claim,
                    &ablated,
                    ablated_artifact,
                    removed,
                )?,
            ],
        });
    }

    representative.sort_by(|left, right| {
        left.bindings
            .workflow_id
            .0
            .cmp(&right.bindings.workflow_id.0)
    });
    adversarial.sort_by(|left, right| {
        left.bindings
            .workflow_id
            .0
            .cmp(&right.bindings.workflow_id.0)
    });
    let representative_document = corpus_document(
        "corpus.workflow-assurance-operations.representative-v0",
        WorkflowBehavioralCorpusClass::Representative,
        context.coverage_artifact.clone(),
        representative,
    );
    let adversarial_document = corpus_document(
        "corpus.workflow-assurance-operations.adversarial-v0",
        WorkflowBehavioralCorpusClass::Adversarial,
        context.coverage_artifact.clone(),
        adversarial,
    );
    assert_clean_corpus(&representative_document)?;
    assert_clean_corpus(&adversarial_document)?;
    let representative_bytes = yaml_bytes(&representative_document)?;
    let adversarial_bytes = yaml_bytes(&adversarial_document)?;
    let representative_artifact = artifact(
        "corpus.workflow-assurance-operations.representative-v0",
        REPRESENTATIVE_PATH,
        &representative_bytes,
    );
    let adversarial_artifact = artifact(
        "corpus.workflow-assurance-operations.adversarial-v0",
        ADVERSARIAL_PATH,
        &adversarial_bytes,
    );
    context.sources.insert(
        representative_artifact.embedded_ref.clone(),
        representative_bytes.clone(),
    );
    context.sources.insert(
        adversarial_artifact.embedded_ref.clone(),
        adversarial_bytes.clone(),
    );

    let corpus_set = WorkflowBehavioralCorpusSetDocument {
        schema_version: WORKFLOW_BEHAVIORAL_CORPUS_SET_SCHEMA_VERSION.to_owned(),
        workflow_behavioral_corpus_set: WorkflowBehavioralCorpusSet {
            id: id("corpus-set.workflow-assurance-operations-v0"),
            corpus_set_version: "0.1.0".to_owned(),
            authority: WorkflowBehavioralEvidenceAuthority::NonAuthoritativeShadowEvidence,
            corpora: vec![
                representative_artifact.clone(),
                adversarial_artifact.clone(),
            ],
        },
    };
    let corpus_set_issues = corpus_set.validate();
    if !corpus_set_issues.is_empty() {
        return Err(error(format!("invalid corpus set: {corpus_set_issues:#?}")));
    }
    let corpus_set_bytes = yaml_bytes(&corpus_set)?;
    let corpus_set_artifact = artifact(
        "corpus-set.workflow-assurance-operations-v0",
        CORPUS_SET_PATH,
        &corpus_set_bytes,
    );
    context.sources.insert(
        corpus_set_artifact.embedded_ref.clone(),
        corpus_set_bytes.clone(),
    );

    let corpora = vec![
        WorkflowBehavioralCorpusInput {
            artifact: representative_artifact,
            document: representative_document,
        },
        WorkflowBehavioralCorpusInput {
            artifact: adversarial_artifact,
            document: adversarial_document,
        },
    ];
    let report = evaluate_workflow_behavior(
        &WorkflowBehavioralReportIdentity {
            report_id: id("report.workflow-assurance-operations.shadow-v0"),
            report_version: "0.1.0".to_owned(),
            corpus_set: corpus_set_artifact,
            coverage_policy: context.coverage_artifact.clone(),
        },
        &context.coverage,
        &corpus_set,
        &context.subject,
        &corpora,
        &bundle_inputs,
        &context.sources,
    );
    if report.workflow_behavioral_shadow_report.verdict
        != WorkflowBehavioralVerdict::BehaviorallyConsistentCandidate
        || report.workflow_behavioral_shadow_report.disposition
            != WorkflowBehavioralDisposition::ReviewCandidate
        || report
            .workflow_behavioral_shadow_report
            .workflow_reports
            .len()
            != 13
        || report
            .workflow_behavioral_shadow_report
            .workflow_reports
            .iter()
            .any(|workflow| {
                workflow.total_scenarios != 7
                    || workflow.representative_scenarios != 2
                    || workflow.adversarial_scenarios != 5
                    || workflow.mismatch_count != 0
                    || workflow.evaluation_error_count != 0
            })
    {
        return Err(error(format!(
            "derived shadow evidence is not a closed 13x7 review candidate: {:#?}",
            report.workflow_behavioral_shadow_report
        )));
    }
    let report_issues = report.validate();
    if !report_issues.is_empty() {
        return Err(error(format!(
            "derived report is invalid: {report_issues:#?}"
        )));
    }

    let mut artifacts = vec![
        GeneratedArtifact {
            relative_path: COVERAGE_PATH.to_owned(),
            bytes: yaml_bytes(&context.coverage)?,
        },
        GeneratedArtifact {
            relative_path: REPRESENTATIVE_PATH.to_owned(),
            bytes: representative_bytes,
        },
        GeneratedArtifact {
            relative_path: ADVERSARIAL_PATH.to_owned(),
            bytes: adversarial_bytes,
        },
        GeneratedArtifact {
            relative_path: CORPUS_SET_PATH.to_owned(),
            bytes: corpus_set_bytes,
        },
        GeneratedArtifact {
            relative_path: SHADOW_REPORT_PATH.to_owned(),
            bytes: yaml_bytes(&report)?,
        },
    ];
    artifacts.extend(ablated_artifacts);
    artifacts.extend(checkpoint_artifacts);
    Ok(artifacts)
}

fn load_context(root: &Path) -> Result<EvidenceContext, Box<dyn Error>> {
    let subject_bytes = read_bytes(root, REVIEW_SUBJECT_PATH)?;
    let subject: WorkflowBehavioralReviewSubjectDocument =
        parse_yaml(&subject_bytes, REVIEW_SUBJECT_PATH)?;
    if !subject.validate().is_empty() {
        return Err(error("review subject is not valid"));
    }
    let runtime_bytes = read_bytes(root, RUNTIME_BUNDLE_PATH)?;
    let runtime: WorkflowGovernanceBundleDocument =
        parse_yaml(&runtime_bytes, RUNTIME_BUNDLE_PATH)?;
    let coverage = coverage_policy();
    let coverage_bytes = yaml_bytes(&coverage)?;
    let overlay = SourceMaterial {
        path: OVERLAY_PATH,
        bytes: read_bytes(root, OVERLAY_PATH)?,
    };
    let evaluator = SourceMaterial {
        path: EVALUATOR_PATH,
        bytes: read_bytes(root, EVALUATOR_PATH)?,
    };
    let baseline_bytes = read_bytes(root, BASELINE_HISTORY_PATH)?;
    let baseline = parse_frozen_baseline(&baseline_bytes)?;
    let predecessor_registry_bytes = read_bytes(root, PREDECESSOR_REGISTRY_PATH)?;
    let predecessor_registry: WorkflowGovernanceReleaseRegistryDocument =
        parse_yaml(&predecessor_registry_bytes, PREDECESSOR_REGISTRY_PATH)?;
    let predecessor = predecessor_registry
        .workflow_governance_release_registry
        .releases
        .last()
        .ok_or_else(|| error("predecessor registry must contain a release"))?;
    let runtime_digest = workflow_runtime_bundle_digest(&runtime).map_err(error)?;
    let policy_set_digest =
        workflow_policy_set_digest(&runtime.workflow_governance_bundle.policies).map_err(error)?;
    let subject_digest = canonical_digest(&subject)?;
    let mut sources = HashMap::new();
    for (path, bytes) in [
        (REVIEW_SUBJECT_PATH, subject_bytes.as_slice()),
        (RUNTIME_BUNDLE_PATH, runtime_bytes.as_slice()),
        (OVERLAY_PATH, overlay.bytes.as_slice()),
        (EVALUATOR_PATH, evaluator.bytes.as_slice()),
        (COVERAGE_PATH, coverage_bytes.as_slice()),
        (BASELINE_HISTORY_PATH, baseline_bytes.as_slice()),
        (
            PREDECESSOR_REGISTRY_PATH,
            predecessor_registry_bytes.as_slice(),
        ),
    ] {
        sources.insert(RepoPath(path.to_owned()), bytes.to_vec());
    }
    for workflow_id in WORKFLOW_IDS {
        let path = format!("contracts/workflows/{workflow_id}.yaml");
        let bytes = std::fs::read(
            root.join("contracts/evidence/workflow-retirement/legacy-catalog")
                .join(format!("{workflow_id}.yaml")),
        )?;
        sources.insert(RepoPath(path), bytes);
    }
    let subject_state = &subject.workflow_behavioral_review_subject;
    if subject_state.runtime_bundle.bundle_digest != runtime_digest
        || subject_state.runtime_bundle.policy_set_digest != policy_set_digest
        || subject_state.evaluator.evaluator_source_digest != sha256(&evaluator.bytes)
        || subject_state.overlay.expected_digest != sha256(&overlay.bytes)
        || subject_state.baseline_history.expected_digest != sha256(&baseline_bytes)
        || subject_state.baseline_release != predecessor.release
        || subject_state.baseline_runtime_bundle != predecessor.runtime_bundle.identity
        || subject_state.baseline_release.release_id.0
            != "workflow-governance.release.core-assurance-v0"
        || subject_state.baseline_runtime_bundle.bundle_id.0
            != "bundle.workflow-governance.core-assurance-v0"
    {
        return Err(error(
            "review subject drifted from candidate bundle, evaluator, or overlay bytes; regenerate candidate prerequisites first",
        ));
    }
    Ok(EvidenceContext {
        subject_artifact: artifact(&subject_state.id.0, REVIEW_SUBJECT_PATH, &subject_bytes),
        subject_digest,
        runtime_artifact: artifact(
            &runtime.workflow_governance_bundle.id.0,
            RUNTIME_BUNDLE_PATH,
            &runtime_bytes,
        ),
        runtime_digest,
        policy_set_digest,
        coverage_artifact: artifact(
            "coverage.workflow-assurance-operations-v0",
            COVERAGE_PATH,
            &coverage_bytes,
        ),
        coverage_digest: canonical_digest(&coverage)?,
        subject,
        runtime,
        coverage,
        sources,
        overlay,
        evaluator,
        baseline,
    })
}

fn parse_frozen_baseline(bytes: &[u8]) -> Result<FrozenBaseline, Box<dyn Error>> {
    let text = std::str::from_utf8(bytes)?;
    let records = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<WorkflowGovernanceReceiptDocument>)
        .collect::<Result<Vec<_>, _>>()?;
    if records.len() != 3 {
        return Err(error(
            "frozen predecessor history must contain exactly three records",
        ));
    }
    let imported_record = &records[0].workflow_governance_receipt;
    let foundation_record = &records[1].workflow_governance_receipt;
    let upgraded_record = &records[2].workflow_governance_receipt;
    let WorkflowGovernanceEvent::ProjectImported(imported) = &imported_record.event else {
        return Err(error("frozen history does not begin with project import"));
    };
    let WorkflowGovernanceEvent::ReleaseUpgraded(upgraded) = &upgraded_record.event else {
        return Err(error("frozen history does not end with release upgrade"));
    };
    if foundation_record.previous_record_digest.as_deref()
        != Some(imported_record.record_digest.as_str())
        || upgraded_record.previous_record_digest.as_deref()
            != Some(foundation_record.record_digest.as_str())
        || upgraded.prior_ledger_head_digest != foundation_record.record_digest
        || upgraded.admission_proof.snapshot_digest != imported.snapshot_digest
    {
        return Err(error("frozen predecessor history digest chain is invalid"));
    }
    Ok(FrozenBaseline {
        history_digest: sha256(bytes),
        ledger_head_digest: upgraded_record.record_digest.clone(),
        snapshot_digest: imported.snapshot_digest.clone(),
        active_release_id: upgraded.to_release.release_id.clone(),
        active_release_digest: upgraded.to_release.release_digest.clone(),
        runtime_bundle_id: upgraded.to_runtime_bundle.bundle_id.clone(),
        runtime_bundle_digest: upgraded.to_runtime_bundle.bundle_digest.clone(),
        state_version: upgraded_record.state_version,
        current_phase: imported.initial_phase.clone(),
        observed_at_unix: upgraded_record.recorded_at_unix,
    })
}

fn coverage_policy() -> WorkflowBehavioralCoveragePolicyDocument {
    WorkflowBehavioralCoveragePolicyDocument {
        schema_version: WORKFLOW_BEHAVIORAL_COVERAGE_POLICY_SCHEMA_VERSION.to_owned(),
        workflow_behavioral_coverage_policy: WorkflowBehavioralCoveragePolicy {
            id: id("coverage.workflow-assurance-operations-v0"),
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

#[derive(Debug, Clone, Copy)]
enum ScenarioEvidence {
    Passing,
    Failing,
    Inconclusive,
    FalseCompletion,
    Stale,
}

fn single_scenario(
    context: &EvidenceContext,
    policy: &WorkflowGovernancePolicy,
    kind: WorkflowBehavioralScenarioKind,
    evidence: ScenarioEvidence,
) -> Result<WorkflowBehavioralScenario, Box<dyn Error>> {
    let evaluation = evaluation(context, policy, kind, evidence);
    let expected =
        derive_workflow_governed_outcome(&context.runtime, &evaluation).map_err(|rejection| {
            error(format!(
                "scenario {kind:?} rejected: {:#?}",
                rejection.issues
            ))
        })?;
    let execution = WorkflowBehavioralScenarioExecution::Single {
        input: Box::new(governance_input(
            context.runtime_artifact.clone(),
            evaluation,
        )),
        expected: Box::new(expected),
    };
    scenario(policy, kind, class(kind), execution)
}

fn resume_scenario(
    context: &mut EvidenceContext,
    policy: &WorkflowGovernancePolicy,
) -> Result<(WorkflowBehavioralScenario, GeneratedArtifact), Box<dyn Error>> {
    let evaluation = evaluation(
        context,
        policy,
        WorkflowBehavioralScenarioKind::Resume,
        ScenarioEvidence::Passing,
    );
    let authored_checkpoint =
        governance_input(context.runtime_artifact.clone(), evaluation.clone());
    let checkpoint_bytes = yaml_bytes(&authored_checkpoint)?;
    let checkpoint_path = checkpoint_path(&policy.compatibility_workflow_id.0);
    let checkpoint_source = artifact(
        &format!(
            "checkpoint.workflow-assurance-operations.{}.v0",
            policy.compatibility_workflow_id.0
        ),
        &checkpoint_path,
        &checkpoint_bytes,
    );
    context.sources.insert(
        checkpoint_source.embedded_ref.clone(),
        checkpoint_bytes.clone(),
    );
    // The checkpoint input is always reparsed from its exact published bytes.
    let checkpoint_input: WorkflowBehavioralGovernanceInput =
        parse_yaml(&checkpoint_bytes, &checkpoint_path)?;
    // A replacement agent independently reconstructs the same typed input;
    // it is deliberately not cloned from the parsed checkpoint object.
    let resumed_input = governance_input(context.runtime_artifact.clone(), evaluation.clone());
    let expected = derive_workflow_governed_outcome(&context.runtime, &evaluation)
        .map_err(|rejection| error(format!("resume rejected: {:#?}", rejection.issues)))?;
    let checkpoint_digest = canonical_digest(&checkpoint_input)?;
    let baseline = &context.baseline;
    let execution = WorkflowBehavioralScenarioExecution::Resume {
        continuation: Box::new(WorkflowBehavioralContinuationIdentity {
            ledger_digest: baseline.history_digest.clone(),
            ledger_head_digest: baseline.ledger_head_digest.clone(),
            snapshot_digest: baseline.snapshot_digest.clone(),
            active_release_id: baseline.active_release_id.clone(),
            active_release_digest: baseline.active_release_digest.clone(),
            runtime_bundle_id: baseline.runtime_bundle_id.clone(),
            runtime_bundle_digest: baseline.runtime_bundle_digest.clone(),
            state_version: baseline.state_version,
            current_phase: baseline.current_phase.clone(),
            observed_at_unix: baseline.observed_at_unix,
        }),
        checkpoint_source,
        checkpoint_digest,
        checkpoint_input: Box::new(checkpoint_input),
        checkpoint_expected: Box::new(expected.clone()),
        resumed_input: Box::new(resumed_input),
        resumed_expected: Box::new(expected),
        equivalence_dimensions: WorkflowGovernedOutcomeDimension::all().to_vec(),
    };
    Ok((
        scenario(
            policy,
            WorkflowBehavioralScenarioKind::Resume,
            WorkflowBehavioralCorpusClass::Representative,
            execution,
        )?,
        GeneratedArtifact {
            relative_path: checkpoint_path,
            bytes: checkpoint_bytes,
        },
    ))
}

fn ablation_scenario(
    context: &EvidenceContext,
    policy: &WorkflowGovernancePolicy,
    selected_claim: &StableId,
    ablated: &WorkflowGovernanceBundleDocument,
    ablated_artifact: WorkflowBehavioralArtifactReference,
    removed: Vec<StableId>,
) -> Result<WorkflowBehavioralScenario, Box<dyn Error>> {
    let mut evaluation = evaluation(
        context,
        policy,
        WorkflowBehavioralScenarioKind::Ablation,
        ScenarioEvidence::Passing,
    );
    evaluation
        .workflow_governance_evaluation
        .evidence
        .retain(|evidence| evidence.claim_ref != *selected_claim);
    evaluation
        .workflow_governance_evaluation
        .available_capability_refs
        .retain(|capability| !removed.contains(capability));
    evaluation
        .workflow_governance_evaluation
        .decision_need_refs
        .retain(|decision| !removed.contains(decision));
    evaluation
        .workflow_governance_evaluation
        .resolved_decision_refs
        .retain(|decision| !removed.contains(decision));
    let control_expected = derive_workflow_governed_outcome(&context.runtime, &evaluation)
        .map_err(|rejection| {
            error(format!(
                "ablation control rejected: {:#?}",
                rejection.issues
            ))
        })?;
    let ablated_expected = derive_workflow_governed_outcome(ablated, &evaluation)
        .map_err(|rejection| error(format!("ablated bundle rejected: {:#?}", rejection.issues)))?;
    let required_difference_dimensions = outcome_differences(&control_expected, &ablated_expected);
    for required in [
        WorkflowGovernedOutcomeDimension::Status,
        WorkflowGovernedOutcomeDimension::Completion,
        WorkflowGovernedOutcomeDimension::Obligations,
        WorkflowGovernedOutcomeDimension::Claims,
        WorkflowGovernedOutcomeDimension::NextActions,
    ] {
        if !required_difference_dimensions.contains(&required) {
            return Err(error(format!(
                "load-bearing ablation for {} does not change required governed dimension {required:?}",
                policy.compatibility_workflow_id.0
            )));
        }
    }
    if control_expected.completion == forge_core_contracts::WorkflowGovernedCompletion::Complete
        || ablated_expected.completion != forge_core_contracts::WorkflowGovernedCompletion::Complete
    {
        return Err(error(format!(
            "load-bearing ablation for {} must move incomplete control to complete ablated outcome",
            policy.compatibility_workflow_id.0
        )));
    }
    let execution = WorkflowBehavioralScenarioExecution::Ablation {
        control_input: Box::new(governance_input(
            context.runtime_artifact.clone(),
            evaluation.clone(),
        )),
        control_expected: Box::new(control_expected),
        ablated_input: Box::new(governance_input(ablated_artifact, evaluation)),
        ablated_expected: Box::new(ablated_expected),
        removed_semantic_refs: removed,
        required_difference_dimensions,
    };
    scenario(
        policy,
        WorkflowBehavioralScenarioKind::Ablation,
        WorkflowBehavioralCorpusClass::Adversarial,
        execution,
    )
}

fn outcome_differences(
    left: &WorkflowGovernedOutcome,
    right: &WorkflowGovernedOutcome,
) -> Vec<WorkflowGovernedOutcomeDimension> {
    let mut result = Vec::new();
    for (dimension, differs) in [
        (
            WorkflowGovernedOutcomeDimension::Status,
            left.status != right.status,
        ),
        (
            WorkflowGovernedOutcomeDimension::Eligibility,
            left.eligibility != right.eligibility,
        ),
        (
            WorkflowGovernedOutcomeDimension::Progression,
            left.progression != right.progression,
        ),
        (
            WorkflowGovernedOutcomeDimension::Completion,
            left.completion != right.completion,
        ),
        (
            WorkflowGovernedOutcomeDimension::Obligations,
            left.obligations != right.obligations,
        ),
        (
            WorkflowGovernedOutcomeDimension::Claims,
            left.claims != right.claims,
        ),
        (
            WorkflowGovernedOutcomeDimension::Decisions,
            left.decision_refs != right.decision_refs,
        ),
        (
            WorkflowGovernedOutcomeDimension::Capabilities,
            left.capability_refs != right.capability_refs,
        ),
        (
            WorkflowGovernedOutcomeDimension::Issues,
            left.issues != right.issues,
        ),
        (
            WorkflowGovernedOutcomeDimension::NextActions,
            left.next_actions != right.next_actions,
        ),
    ] {
        if differs {
            result.push(dimension);
        }
    }
    result
}

fn scenario(
    policy: &WorkflowGovernancePolicy,
    kind: WorkflowBehavioralScenarioKind,
    corpus_class: WorkflowBehavioralCorpusClass,
    execution: WorkflowBehavioralScenarioExecution,
) -> Result<WorkflowBehavioralScenario, Box<dyn Error>> {
    let execution_input_digest = workflow_behavior_execution_input_digest(&execution)
        .ok_or_else(|| error("cannot digest behavioral execution input"))?;
    Ok(WorkflowBehavioralScenario {
        scenario_id: id(&format!(
            "scenario.workflow.{}.{}.v0",
            policy.compatibility_workflow_id.0,
            kind_name(kind)
        )),
        scenario_kind: kind,
        corpus_class,
        execution_input_digest,
        execution,
    })
}

fn evaluation(
    context: &EvidenceContext,
    policy: &WorkflowGovernancePolicy,
    kind: WorkflowBehavioralScenarioKind,
    evidence_mode: ScenarioEvidence,
) -> WorkflowGovernanceEvaluationDocument {
    let evidence = match evidence_mode {
        ScenarioEvidence::FalseCompletion => Vec::new(),
        ScenarioEvidence::Passing => observations(
            policy,
            kind,
            WorkflowEvidenceOutcome::Pass,
            WorkflowEvidenceFreshness::Current,
        ),
        ScenarioEvidence::Failing => observations(
            policy,
            kind,
            WorkflowEvidenceOutcome::Fail,
            WorkflowEvidenceFreshness::Current,
        ),
        ScenarioEvidence::Inconclusive => observations(
            policy,
            kind,
            WorkflowEvidenceOutcome::Inconclusive,
            WorkflowEvidenceFreshness::Current,
        ),
        ScenarioEvidence::Stale => observations(
            policy,
            kind,
            WorkflowEvidenceOutcome::Pass,
            WorkflowEvidenceFreshness::Stale,
        ),
    };
    let completion_assertion = if matches!(
        evidence_mode,
        ScenarioEvidence::Passing | ScenarioEvidence::FalseCompletion
    ) {
        WorkflowCompletionAssertion::Asserted
    } else {
        WorkflowCompletionAssertion::NotAsserted
    };
    WorkflowGovernanceEvaluationDocument {
        schema_version: "0.1".to_owned(),
        workflow_governance_evaluation: WorkflowGovernanceEvaluation {
            observation_set_id: id(&format!(
                "observations.{}.{}",
                policy.compatibility_workflow_id.0,
                kind_name(kind)
            )),
            state_version: 7,
            observed_at_unix: 1_720_000_000,
            bundle_id: context.runtime.workflow_governance_bundle.id.clone(),
            policy_id: policy.id.clone(),
            current_phase: policy
                .eligible_phases
                .first()
                .filter(|phase| phase.0 != "anytime")
                .cloned()
                .unwrap_or_else(|| id("2-spec")),
            target: if kind == WorkflowBehavioralScenarioKind::Ambiguity {
                ReadinessTarget::Release
            } else {
                policy.routing.readiness_target
            },
            completed_policy_refs: policy
                .prerequisites
                .iter()
                .map(|prerequisite| prerequisite.policy_ref.clone())
                .collect(),
            not_applicable_policy_refs: Vec::new(),
            available_capability_refs: policy
                .capability_requirements
                .iter()
                .map(|capability| capability.id.clone())
                .collect(),
            decision_need_refs: policy
                .decision_rules
                .iter()
                .map(|decision| decision.id.clone())
                .collect(),
            resolved_decision_refs: if kind == WorkflowBehavioralScenarioKind::Ambiguity {
                Vec::new()
            } else {
                policy
                    .decision_rules
                    .iter()
                    .map(|decision| decision.id.clone())
                    .collect()
            },
            waivers: Vec::new(),
            evidence,
            completion_assertion,
        },
    }
}

fn observations(
    policy: &WorkflowGovernancePolicy,
    kind: WorkflowBehavioralScenarioKind,
    outcome: WorkflowEvidenceOutcome,
    freshness: WorkflowEvidenceFreshness,
) -> Vec<WorkflowEvidenceObservation> {
    let evaluators = policy
        .evaluators
        .iter()
        .map(|evaluator| (evaluator.id.0.as_str(), evaluator))
        .collect::<BTreeMap<_, _>>();
    let mut observations = Vec::new();
    for claim in &policy.claims {
        let evaluator = evaluators
            .get(claim.evaluator_ref.0.as_str())
            .expect("validated policy evaluator");
        let count = evaluator.minimum_passing_observations.max(1);
        for index in 0..count {
            observations.push(WorkflowEvidenceObservation {
                evidence_ref: format!(
                    "evidence.{}.{}.{}.{}",
                    policy.compatibility_workflow_id.0,
                    kind_name(kind),
                    claim.id.0,
                    index
                ),
                claim_ref: claim.id.clone(),
                evaluator_ref: evaluator.id.clone(),
                principal: (index < evaluator.minimum_distinct_principals).then(|| {
                    PrincipalId(format!(
                        "principal.{}.{}",
                        policy.compatibility_workflow_id.0, index
                    ))
                }),
                kind: evaluator.accepted_evidence_kinds[0],
                strength: evaluator.minimum_strength,
                freshness,
                outcome,
            });
        }
    }
    observations
}

fn governance_input(
    bundle: WorkflowBehavioralArtifactReference,
    evaluation: WorkflowGovernanceEvaluationDocument,
) -> WorkflowBehavioralGovernanceInput {
    WorkflowBehavioralGovernanceInput { bundle, evaluation }
}

fn bindings(
    root: &Path,
    context: &EvidenceContext,
    policy: &WorkflowGovernancePolicy,
) -> Result<WorkflowBehavioralEvidenceBindings, Box<dyn Error>> {
    let subject = &context.subject.workflow_behavioral_review_subject;
    let candidate = subject
        .candidate_workflows
        .iter()
        .find(|candidate| candidate.policy_ref == policy.id)
        .ok_or_else(|| error(format!("review subject missing {}", policy.id.0)))?;
    let legacy_path = format!(
        "contracts/workflows/{}.yaml",
        policy.compatibility_workflow_id.0
    );
    let legacy_bytes = std::fs::read(
        root.join("contracts/evidence/workflow-retirement/legacy-catalog")
            .join(format!("{}.yaml", policy.compatibility_workflow_id.0)),
    )?;
    Ok(WorkflowBehavioralEvidenceBindings {
        review_subject: context.subject_artifact.clone(),
        review_subject_digest: context.subject_digest.clone(),
        workflow_id: policy.compatibility_workflow_id.clone(),
        legacy_workflow_digest: candidate.legacy_workflow_digest.clone(),
        policy_ref: policy.id.clone(),
        policy_digest: workflow_release_policy_digest(policy).map_err(error)?,
        candidate_bundle_id: context.runtime.workflow_governance_bundle.id.clone(),
        candidate_bundle_digest: context.runtime_digest.clone(),
        candidate_bundle_source_digest: context.runtime_artifact.expected_digest.clone(),
        candidate_policy_set_digest: context.policy_set_digest.clone(),
        migration_batch_id: subject.proposed_batch.batch_id.clone(),
        migration_batch_version: subject.proposed_batch.batch_version.clone(),
        governance_release_id: subject.proposed_release.release_id.clone(),
        governance_release_version: subject.proposed_release.release_version.clone(),
        predecessor_release_digest: subject.proposed_release.previous_release_digest.clone(),
        coverage_policy_id: context
            .coverage
            .workflow_behavioral_coverage_policy
            .id
            .clone(),
        coverage_policy_digest: context.coverage_digest.clone(),
        coverage_policy_source_digest: context.coverage_artifact.expected_digest.clone(),
        evaluator: subject.evaluator.clone(),
        raw_sources: vec![
            raw_source(
                WorkflowBehavioralRawSourceKind::LegacyWorkflow,
                &legacy_path,
                &legacy_bytes,
            ),
            raw_source(
                WorkflowBehavioralRawSourceKind::GovernancePolicy,
                context.overlay.path,
                &context.overlay.bytes,
            ),
            raw_source(
                WorkflowBehavioralRawSourceKind::CandidateBundle,
                RUNTIME_BUNDLE_PATH,
                &read_bytes(root, RUNTIME_BUNDLE_PATH)?,
            ),
            raw_source(
                WorkflowBehavioralRawSourceKind::CoveragePolicy,
                COVERAGE_PATH,
                &yaml_bytes(&context.coverage)?,
            ),
            raw_source(
                WorkflowBehavioralRawSourceKind::Evaluator,
                context.evaluator.path,
                &context.evaluator.bytes,
            ),
        ],
    })
}

fn ablate_policy(
    runtime: &WorkflowGovernanceBundleDocument,
    policy: &WorkflowGovernancePolicy,
) -> Result<(WorkflowGovernanceBundleDocument, Vec<StableId>, StableId), Box<dyn Error>> {
    let selected_claim = policy
        .claims
        .iter()
        .find(|claim| {
            policy
                .claims
                .iter()
                .filter(|candidate| candidate.evaluator_ref == claim.evaluator_ref)
                .count()
                == 1
                && policy
                    .obligations
                    .iter()
                    .any(|obligation| obligation.claim_refs.contains(&claim.id))
        })
        .ok_or_else(|| {
            error(format!(
                "policy {} has no closed load-bearing claim",
                policy.id.0
            ))
        })?
        .clone();
    let mut removed = vec![
        selected_claim.id.clone(),
        selected_claim.evaluator_ref.clone(),
    ];
    removed.extend(
        policy
            .obligations
            .iter()
            .filter(|obligation| obligation.claim_refs.contains(&selected_claim.id))
            .map(|obligation| obligation.id.clone()),
    );
    removed.extend(
        policy
            .decision_rules
            .iter()
            .filter(|decision| decision.claim_ref.as_ref() == Some(&selected_claim.id))
            .map(|decision| decision.id.clone()),
    );
    removed.extend(
        policy
            .capability_requirements
            .iter()
            .filter(|capability| capability.affected_claim_refs.contains(&selected_claim.id))
            .map(|capability| capability.id.clone()),
    );
    removed.sort_by(|left, right| left.0.cmp(&right.0));
    removed.dedup();
    let mut ablated = runtime.clone();
    let target = ablated
        .workflow_governance_bundle
        .policies
        .iter_mut()
        .find(|candidate| candidate.id == policy.id)
        .ok_or_else(|| error("candidate policy disappeared during ablation"))?;
    target
        .obligations
        .retain(|obligation| !removed.contains(&obligation.id));
    target.claims.retain(|claim| !removed.contains(&claim.id));
    target
        .evaluators
        .retain(|evaluator| !removed.contains(&evaluator.id));
    target
        .decision_rules
        .retain(|decision| !removed.contains(&decision.id));
    target
        .capability_requirements
        .retain(|capability| !removed.contains(&capability.id));
    let issues = validate_workflow_governance_bundle(&ablated);
    if !issues.is_empty() {
        return Err(error(format!(
            "load-bearing ablated bundle for {} is structurally invalid: {issues:#?}",
            policy.compatibility_workflow_id.0
        )));
    }
    Ok((ablated, removed, selected_claim.id))
}

fn corpus_document(
    corpus_id: &str,
    partition_class: WorkflowBehavioralCorpusClass,
    coverage_policy: WorkflowBehavioralArtifactReference,
    workflow_evidence: Vec<WorkflowBehavioralWorkflowCorpus>,
) -> WorkflowBehavioralScenarioCorpusDocument {
    WorkflowBehavioralScenarioCorpusDocument {
        schema_version: WORKFLOW_BEHAVIORAL_SCENARIO_CORPUS_SCHEMA_VERSION.to_owned(),
        workflow_behavioral_scenario_corpus: WorkflowBehavioralScenarioCorpus {
            id: id(corpus_id),
            corpus_version: "0.1.0".to_owned(),
            authority: WorkflowBehavioralEvidenceAuthority::NonAuthoritativeShadowEvidence,
            partition_class,
            coverage_policy,
            workflow_evidence,
        },
    }
}

fn assert_clean_corpus(
    corpus: &WorkflowBehavioralScenarioCorpusDocument,
) -> Result<(), Box<dyn Error>> {
    let issues = corpus.validate();
    if issues.is_empty() {
        Ok(())
    } else {
        Err(error(format!("invalid behavioral corpus: {issues:#?}")))
    }
}

fn class(kind: WorkflowBehavioralScenarioKind) -> WorkflowBehavioralCorpusClass {
    match kind {
        WorkflowBehavioralScenarioKind::Positive | WorkflowBehavioralScenarioKind::Resume => {
            WorkflowBehavioralCorpusClass::Representative
        }
        _ => WorkflowBehavioralCorpusClass::Adversarial,
    }
}

fn kind_name(kind: WorkflowBehavioralScenarioKind) -> &'static str {
    match kind {
        WorkflowBehavioralScenarioKind::Positive => "positive",
        WorkflowBehavioralScenarioKind::Negative => "negative",
        WorkflowBehavioralScenarioKind::Ambiguity => "ambiguity",
        WorkflowBehavioralScenarioKind::FalseCompletion => "false-completion",
        WorkflowBehavioralScenarioKind::StaleEvidence => "stale-evidence",
        WorkflowBehavioralScenarioKind::Resume => "resume",
        WorkflowBehavioralScenarioKind::Ablation => "ablation",
    }
}

fn ablated_path(workflow_id: &str) -> String {
    format!("contracts/workflow-governance/ablated-assurance-operations-{workflow_id}-v0.yaml")
}

fn checkpoint_path(workflow_id: &str) -> String {
    format!(
        "contracts/evidence/workflow-assurance-operations-{workflow_id}-resume-checkpoint-v0.yaml"
    )
}

fn raw_source(
    kind: WorkflowBehavioralRawSourceKind,
    path: &str,
    bytes: &[u8],
) -> WorkflowBehavioralRawSourceReference {
    WorkflowBehavioralRawSourceReference {
        kind,
        embedded_ref: RepoPath(path.to_owned()),
        expected_digest: sha256(bytes),
    }
}

fn artifact(artifact_id: &str, path: &str, bytes: &[u8]) -> WorkflowBehavioralArtifactReference {
    WorkflowBehavioralArtifactReference {
        id: id(artifact_id),
        embedded_ref: RepoPath(path.to_owned()),
        expected_digest: sha256(bytes),
    }
}

fn read_bytes(root: &Path, path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    std::fs::read(root.join(path))
        .map_err(|cause| error(format!("required source is unavailable at {path}: {cause}")))
}

fn parse_yaml<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    path: &str,
) -> Result<T, Box<dyn Error>> {
    let text = std::str::from_utf8(bytes)?;
    yaml_serde::from_str(text)
        .map_err(|cause| error(format!("cannot parse typed YAML {path}: {cause}")))
}

fn yaml_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut text = yaml_serde::to_string(value)?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text.into_bytes())
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, Box<dyn Error>> {
    let canonical = serde_json_canonicalizer::to_vec(value)?;
    Ok(sha256(&canonical))
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn write_artifacts(root: &Path, artifacts: &[GeneratedArtifact]) -> Result<(), Box<dyn Error>> {
    for artifact in artifacts {
        let path = root.join(&artifact.relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &artifact.bytes)?;
        println!("wrote {}", artifact.relative_path);
    }
    Ok(())
}

fn check_artifacts(root: &Path, artifacts: &[GeneratedArtifact]) -> Result<(), Box<dyn Error>> {
    let drift = artifacts
        .iter()
        .filter_map(
            |artifact| match std::fs::read(root.join(&artifact.relative_path)) {
                Ok(found) if found == artifact.bytes => None,
                Ok(_) => Some(format!("{} has byte drift", artifact.relative_path)),
                Err(cause) => Some(format!(
                    "{} is unavailable: {cause}",
                    artifact.relative_path
                )),
            },
        )
        .collect::<Vec<_>>();
    if drift.is_empty() {
        println!("workflow assurance-operations behavioral evidence is byte-exact");
        Ok(())
    } else {
        Err(error(format!(
            "workflow assurance-operations evidence drift:\n{}\nrun the generator with --write",
            drift.join("\n")
        )))
    }
}

fn error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(std::io::Error::other(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_evidence_recomputes_as_exact_closed_thirteen_by_seven_corpus() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let artifacts = generate(&root).expect("closed behavioral evidence recomputation");
        assert_eq!(artifacts.len(), 31);
        assert!(artifacts
            .iter()
            .any(|artifact| artifact.relative_path == SHADOW_REPORT_PATH));
        assert!(
            artifacts
                .iter()
                .filter(|artifact| artifact
                    .relative_path
                    .contains("ablated-assurance-operations"))
                .count()
                == 13
        );
        assert!(
            artifacts
                .iter()
                .filter(|artifact| artifact.relative_path.contains("resume-checkpoint"))
                .count()
                == 13
        );
        check_artifacts(&root, &artifacts).expect("all generated bytes must be deterministic");
    }
}
