use forge_core_contracts::{
    RepoPath, StableId, WorkflowBehavioralArtifactReference,
    WorkflowBehavioralCandidateWorkflowSubject, WorkflowBehavioralEvaluatorIdentity,
    WorkflowBehavioralProposedBatchSubject, WorkflowBehavioralProposedReleaseSubject,
    WorkflowBehavioralQuarantineSubject, WorkflowBehavioralReviewSubject,
    WorkflowBehavioralReviewSubjectAuthority, WorkflowBehavioralReviewSubjectDocument,
    WorkflowBehavioralRuntimeBundleSubject, WorkflowEvaluatorProvider, WorkflowEvidenceKind,
    WorkflowEvidenceStrength, WorkflowGovernanceBundle, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceEvent, WorkflowGovernancePolicy, WorkflowGovernancePolicyOverlayDocument,
    WorkflowGovernanceReceiptDocument, WorkflowGovernanceReleaseManifestDocument,
    WorkflowMigrationBatch, WorkflowMigrationBatchAuthority, WorkflowMigrationBatchBinding,
    WorkflowMigrationBatchDocument, WorkflowMigrationBatchEvidence,
    WorkflowMigrationEvidenceReference, WorkflowMigrationPlanDocument, WorkflowQuarantine,
    WorkflowQuarantineReasonCode, WorkflowQuarantineRiskTier, WorkflowReleaseBatchReference,
    WorkflowReleaseDispositionIntent, WORKFLOW_BEHAVIORAL_REVIEW_SUBJECT_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_SCHEMA_VERSION, WORKFLOW_MIGRATION_BATCH_SCHEMA_VERSION,
};
use forge_core_decisions::{
    evaluate_workflow_migration, evaluate_workflow_release, load_catalog, load_workflow_documents,
    validate_workflow_governance_bundle, workflow_migration_batch_digest,
    workflow_policy_set_digest, workflow_release_legacy_digest, workflow_release_manifest_digest,
    workflow_release_policy_digest, workflow_runtime_bundle_digest, LoadedWorkflowDocument,
    WorkflowMigrationAuditStatus, WorkflowReleaseEvaluationStatus,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::path::{Path, PathBuf};

const GOLDEN_BUNDLE_PATH: &str = "contracts/workflow-governance/golden-path-v0.yaml";
const GOLDEN_BATCH_PATH: &str = "contracts/migration/workflow-governance-batch-golden-path-v0.yaml";
const FOUNDATION_MANIFEST_PATH: &str =
    "contracts/migration/workflow-governance-release-foundation-v0.yaml";
const OVERLAY_PATH: &str = "contracts/policies/workflow-core-assurance-overlay-v0.yaml";
const REVIEW_SUBJECT_PATH: &str =
    "contracts/migration/workflow-core-assurance-review-subject-v0.yaml";
const BATCH_PATH: &str = "contracts/migration/workflow-governance-batch-core-assurance-v0.yaml";
const MANIFEST_PATH: &str =
    "contracts/migration/workflow-governance-release-core-assurance-candidate-v0.yaml";
const RUNTIME_BUNDLE_PATH: &str =
    "contracts/workflow-governance/runtime-core-assurance-candidate-v0.yaml";
const REPRESENTATIVE_EVIDENCE_PATH: &str =
    "contracts/evidence/workflow-core-assurance-representative-v0.yaml";
const ADVERSARIAL_EVIDENCE_PATH: &str =
    "contracts/evidence/workflow-core-assurance-adversarial-v0.yaml";
const SHADOW_REPORT_PATH: &str = "contracts/evidence/workflow-core-assurance-shadow-report-v0.yaml";
const EVALUATOR_SOURCE_PATH: &str = "crates/forge-core-decisions/src/workflow_behavior.rs";
const P5D2_REGISTRY_PATH: &str = "contracts/migration/workflow-governance-release-registry-v0.yaml";
const P5D2_REGISTRY_SHA256: &str =
    "sha256:f8e556011b0d4f9450c0905661096561f2294e83fc5ec43af1a48574d5599a07";
const BASELINE_HISTORY_PATH: &str =
    "crates/forge-core-kernel/tests/fixtures/p5d2-foundation-history.ndjson";

const GOLDEN_WORKFLOW_IDS: [&str; 15] = [
    "architecture",
    "build-story",
    "context-recovery",
    "correct-course",
    "discover-intent",
    "domain-scan",
    "plan-sprint",
    "product-requirements",
    "readiness-check",
    "ready-release",
    "reality-evidence-gate",
    "story-creation",
    "technical-feasibility-scan",
    "test-strategy",
    "write-spec",
];
const OVERLAY_WORKFLOW_IDS: [&str; 5] = [
    "adversarial-review",
    "code-review",
    "nfr-evidence-audit",
    "risk-register",
    "traceability-gate",
];
const QUARANTINED_WORKFLOW_IDS: [&str; 3] =
    ["edge-case-review", "release-readiness", "track-decision"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
}

struct GeneratedArtifact {
    relative_path: &'static str,
    bytes: Vec<u8>,
}

#[derive(Default)]
struct DispositionCounts {
    migration: usize,
    compatibility: usize,
    quarantined: usize,
    domain: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mode = parse_mode()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let artifacts = generate(&root, mode)?;
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
            "usage: cargo run -p forge-core-decisions --example generate_workflow_core_assurance_candidate -- (--write|--check)",
        )),
    }
}

// Candidate composition keeps all fail-closed count, quarantine, registry,
// and content-addressed checks together for an auditable publication order.
#[allow(clippy::too_many_lines)]
fn generate(root: &Path, mode: Mode) -> Result<Vec<GeneratedArtifact>, Box<dyn Error>> {
    validate_registry_sentinel(root)?;
    let workflows = load_clean_workflows(root)?;
    let migration_audit = load_migration_audit(root, &workflows)?;
    let golden_bundle: WorkflowGovernanceBundleDocument =
        read_yaml(&root.join(GOLDEN_BUNDLE_PATH))?;
    let overlay: WorkflowGovernancePolicyOverlayDocument = read_yaml(&root.join(OVERLAY_PATH))?;
    validate_overlay_header(&overlay, &golden_bundle)?;
    validate_exact_policy_set(
        &golden_bundle.workflow_governance_bundle.policies,
        &GOLDEN_WORKFLOW_IDS,
        "golden",
    )?;
    validate_exact_policy_set(
        &overlay.workflow_governance_policy_overlay.policies,
        &OVERLAY_WORKFLOW_IDS,
        "overlay",
    )?;

    let mut policies = golden_bundle.workflow_governance_bundle.policies.clone();
    policies.extend(overlay.workflow_governance_policy_overlay.policies.clone());
    policies.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    validate_composition(&policies)?;
    validate_frozen_predecessor_authority_paths(
        &golden_bundle.workflow_governance_bundle.policies,
    )?;
    eprintln!(
        "compatibility debt: frozen P5c/P5d.2 evaluators may list accepted evidence tuples that the trusted authority matrix cannot mint; every frozen evaluator retains at least one usable authorized tuple, while every new P5d.3 overlay tuple is exact"
    );
    // P5d.3 cannot rewrite the frozen P5c/P5d.2 predecessor policy set.
    // Every newly compiled evaluator tuple must nevertheless be realizable by
    // the trusted authority matrix before it can enter this candidate.
    validate_evaluator_authority_matrix(&overlay.workflow_governance_policy_overlay.policies)?;

    let runtime_bundle = WorkflowGovernanceBundleDocument {
        schema_version: WORKFLOW_GOVERNANCE_SCHEMA_VERSION.to_owned(),
        workflow_governance_bundle: WorkflowGovernanceBundle {
            id: id("bundle.workflow-governance.core-assurance-candidate-v0"),
            policies: policies.clone(),
        },
    };
    let governance_issues = validate_workflow_governance_bundle(&runtime_bundle);
    if !governance_issues.is_empty() {
        return Err(error(format!(
            "candidate runtime bundle is invalid: {governance_issues:#?}"
        )));
    }

    let foundation_manifest: WorkflowGovernanceReleaseManifestDocument =
        read_yaml(&root.join(FOUNDATION_MANIFEST_PATH))?;
    let foundation_batch: WorkflowMigrationBatchDocument =
        read_yaml(&root.join(GOLDEN_BATCH_PATH))?;
    validate_foundation_inputs(&foundation_manifest, &foundation_batch, &workflows)?;
    let previous_release_digest =
        workflow_release_manifest_digest(&foundation_manifest).map_err(error)?;
    let previous_batch_digest =
        workflow_migration_batch_digest(&foundation_batch).map_err(error)?;

    let quarantines = build_quarantines();
    let review_subject = build_review_subject(
        root,
        &workflows,
        &overlay.workflow_governance_policy_overlay.policies,
        &runtime_bundle,
        &quarantines,
        previous_release_digest.clone(),
    )?;
    let review_subject_issues = review_subject.validate();
    if !review_subject_issues.is_empty() {
        return Err(error(format!(
            "typed review subject is invalid: {review_subject_issues:#?}"
        )));
    }
    let review_subject_bytes = yaml_bytes(&review_subject)?;
    if review_subject_bytes != yaml_bytes(&review_subject)? {
        return Err(error(
            "typed review subject serialization is not deterministic",
        ));
    }
    let runtime_bundle_bytes = yaml_bytes(&runtime_bundle)?;

    // The evidence corpus binds this acyclic pre-evidence subject. On a clean
    // checkout, publish only these deterministic prerequisites first so the
    // behavioral generator can derive its three content-addressed artifacts;
    // this generator still fails closed below until those artifacts exist.
    if mode == Mode::Write {
        write_artifacts(
            root,
            &[
                GeneratedArtifact {
                    relative_path: REVIEW_SUBJECT_PATH,
                    bytes: review_subject_bytes.clone(),
                },
                GeneratedArtifact {
                    relative_path: RUNTIME_BUNDLE_PATH,
                    bytes: runtime_bundle_bytes.clone(),
                },
            ],
        )?;
    }

    let batch = build_overlay_batch(
        root,
        &workflows,
        &overlay.workflow_governance_policy_overlay.policies,
        &foundation_batch,
        previous_batch_digest,
    )?;
    let batch_digest = workflow_migration_batch_digest(&batch).map_err(error)?;
    let manifest = build_candidate_manifest(
        &foundation_manifest,
        &workflows,
        &quarantines,
        batch_digest,
        previous_release_digest,
    )?;

    // The release evaluator intentionally resolves repository-owned embedded
    // bytes, so a clean bootstrap must publish the deterministic candidate
    // documents before the crate is rebuilt with the expanded contracts tree.
    if mode == Mode::Write {
        write_artifacts(
            root,
            &[
                GeneratedArtifact {
                    relative_path: BATCH_PATH,
                    bytes: yaml_bytes(&batch)?,
                },
                GeneratedArtifact {
                    relative_path: MANIFEST_PATH,
                    bytes: yaml_bytes(&manifest)?,
                },
            ],
        )?;
    }

    let release = evaluate_workflow_release(
        &manifest,
        &[foundation_batch, batch.clone()],
        &migration_audit,
        &workflows,
    );
    if release.status != WorkflowReleaseEvaluationStatus::StructurallyValid
        || !release.issues.is_empty()
    {
        return Err(error(format!(
            "candidate release failed structural evaluation: {:#?}",
            release.issues
        )));
    }
    validate_final_invariants(&manifest, &batch, &runtime_bundle)?;

    Ok(vec![
        GeneratedArtifact {
            relative_path: REVIEW_SUBJECT_PATH,
            bytes: review_subject_bytes,
        },
        GeneratedArtifact {
            relative_path: BATCH_PATH,
            bytes: yaml_bytes(&batch)?,
        },
        GeneratedArtifact {
            relative_path: MANIFEST_PATH,
            bytes: yaml_bytes(&manifest)?,
        },
        GeneratedArtifact {
            relative_path: RUNTIME_BUNDLE_PATH,
            bytes: runtime_bundle_bytes,
        },
    ])
}

fn load_clean_workflows(root: &Path) -> Result<Vec<LoadedWorkflowDocument>, Box<dyn Error>> {
    let catalog_dir = root.join("contracts/workflows");
    let loaded = load_workflow_documents(&catalog_dir);
    if !loaded.is_clean() || loaded.workflows.len() != 110 {
        return Err(error(format!(
            "expected clean 110-workflow inventory, found {} workflow(s) and {} error(s)",
            loaded.workflows.len(),
            loaded.errors.len()
        )));
    }
    Ok(loaded.workflows)
}

fn load_migration_audit(
    root: &Path,
    workflows: &[LoadedWorkflowDocument],
) -> Result<forge_core_decisions::WorkflowMigrationAudit, Box<dyn Error>> {
    let catalog_dir = root.join("contracts/workflows");
    let catalog = load_catalog(&catalog_dir);
    if !catalog.is_clean() || catalog.catalog.entries.len() != 110 {
        return Err(error(
            "catalog is not the exact clean 110-workflow inventory",
        ));
    }
    let plan: WorkflowMigrationPlanDocument =
        read_yaml(&root.join("contracts/policies/workflow-migration-foundation-v0.yaml"))?;
    let audit = evaluate_workflow_migration(&plan, workflows, &catalog.catalog);
    if audit.status != WorkflowMigrationAuditStatus::ReadyForShadow || !audit.issues.is_empty() {
        return Err(error(format!(
            "P5a migration audit is not ready: {:#?}",
            audit.issues
        )));
    }
    Ok(audit)
}

fn validate_exact_policy_set(
    policies: &[WorkflowGovernancePolicy],
    expected: &[&str],
    label: &str,
) -> Result<(), Box<dyn Error>> {
    let found = policies
        .iter()
        .map(|policy| policy.compatibility_workflow_id.0.as_str())
        .collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if found != expected || policies.len() != expected.len() {
        return Err(error(format!(
            "{label} policy set must match its exact closed workflow ids; found {found:?}, expected {expected:?}"
        )));
    }
    for policy in policies {
        let expected_policy_id = format!("policy.workflow.{}", policy.compatibility_workflow_id.0);
        if policy.id.0 != expected_policy_id {
            return Err(error(format!(
                "{label} workflow {} must use exact policy id {expected_policy_id}, found {}",
                policy.compatibility_workflow_id.0, policy.id.0
            )));
        }
    }
    Ok(())
}

fn validate_overlay_header(
    overlay: &WorkflowGovernancePolicyOverlayDocument,
    golden_bundle: &WorkflowGovernanceBundleDocument,
) -> Result<(), Box<dyn Error>> {
    if overlay.schema_version != WORKFLOW_GOVERNANCE_SCHEMA_VERSION
        || overlay.workflow_governance_policy_overlay.id.0
            != "overlay.workflow-governance.core-assurance-v0"
        || overlay.workflow_governance_policy_overlay.base_bundle_id
            != golden_bundle.workflow_governance_bundle.id
        || overlay.workflow_governance_policy_overlay.base_bundle_id.0
            != "bundle.workflow-governance.golden-path-v0"
    {
        return Err(error(
            "core-assurance overlay header or exact golden base binding is invalid",
        ));
    }
    Ok(())
}

fn validate_composition(policies: &[WorkflowGovernancePolicy]) -> Result<(), Box<dyn Error>> {
    if policies.len() != 20 {
        return Err(error(format!(
            "candidate runtime must contain exactly 20 policies, found {}",
            policies.len()
        )));
    }
    let quarantine_policy_ids = QUARANTINED_WORKFLOW_IDS
        .iter()
        .map(|workflow_id| format!("policy.workflow.{workflow_id}"))
        .collect::<BTreeSet<_>>();
    let policy_ids = policies
        .iter()
        .map(|policy| policy.id.0.as_str())
        .collect::<BTreeSet<_>>();
    let workflow_ids = policies
        .iter()
        .map(|policy| policy.compatibility_workflow_id.0.as_str())
        .collect::<BTreeSet<_>>();
    if policy_ids.len() != 20 || workflow_ids.len() != 20 {
        return Err(error(
            "candidate runtime contains duplicate policy or workflow ids",
        ));
    }
    for policy in policies {
        if quarantine_policy_ids.contains(&policy.id.0)
            || QUARANTINED_WORKFLOW_IDS.contains(&policy.compatibility_workflow_id.0.as_str())
        {
            return Err(error(format!(
                "quarantined workflow leaked into policy composition: {}",
                policy.compatibility_workflow_id.0
            )));
        }
        for prerequisite in &policy.prerequisites {
            if quarantine_policy_ids.contains(&prerequisite.policy_ref.0) {
                return Err(error(format!(
                    "policy {} depends on quarantined policy {}",
                    policy.id.0, prerequisite.policy_ref.0
                )));
            }
        }
    }
    Ok(())
}

fn validate_evaluator_authority_matrix(
    policies: &[WorkflowGovernancePolicy],
) -> Result<(), Box<dyn Error>> {
    for policy in policies {
        for evaluator in &policy.evaluators {
            for kind in &evaluator.accepted_evidence_kinds {
                let Some(authorizable_strength) = authorizable_strength(evaluator.provider, *kind)
                else {
                    return Err(error(format!(
                        "policy {} evaluator {} provider {:?} cannot authorize evidence kind {:?}",
                        policy.id.0, evaluator.id.0, evaluator.provider, kind
                    )));
                };
                if authorizable_strength < evaluator.minimum_strength {
                    return Err(error(format!(
                        "policy {} evaluator {} requires {:?}, above authorizable {:?} for {:?}/{:?}",
                        policy.id.0,
                        evaluator.id.0,
                        evaluator.minimum_strength,
                        authorizable_strength,
                        evaluator.provider,
                        kind
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_frozen_predecessor_authority_paths(
    policies: &[WorkflowGovernancePolicy],
) -> Result<(), Box<dyn Error>> {
    for policy in policies {
        for evaluator in &policy.evaluators {
            let usable = evaluator.accepted_evidence_kinds.iter().any(|kind| {
                authorizable_strength(evaluator.provider, *kind)
                    .is_some_and(|strength| strength >= evaluator.minimum_strength)
            });
            if !usable {
                return Err(error(format!(
                    "frozen predecessor policy {} evaluator {} has no usable trusted-authority tuple",
                    policy.id.0, evaluator.id.0
                )));
            }
        }
    }
    Ok(())
}

fn authorizable_strength(
    provider: WorkflowEvaluatorProvider,
    kind: WorkflowEvidenceKind,
) -> Option<WorkflowEvidenceStrength> {
    match (provider, kind) {
        (WorkflowEvaluatorProvider::AuthorizedHuman, WorkflowEvidenceKind::HumanAcceptance)
        | (WorkflowEvaluatorProvider::ExternalAuthority, WorkflowEvidenceKind::ExternalAuthority) => {
            Some(WorkflowEvidenceStrength::AuthoritativeAcceptance)
        }
        (
            WorkflowEvaluatorProvider::IndependentReviewer,
            WorkflowEvidenceKind::IndependentReview,
        )
        | (WorkflowEvaluatorProvider::ResearchSource, WorkflowEvidenceKind::Research) => {
            Some(WorkflowEvidenceStrength::IndependentConfirmation)
        }
        (
            WorkflowEvaluatorProvider::RepositoryInspector,
            WorkflowEvidenceKind::ArtifactInspection,
        ) => Some(WorkflowEvidenceStrength::InspectedArtifact),
        (
            WorkflowEvaluatorProvider::DeterministicTool,
            WorkflowEvidenceKind::DeterministicCheck,
        ) => Some(WorkflowEvidenceStrength::DeterministicVerification),
        (
            WorkflowEvaluatorProvider::RepresentativeRuntime,
            WorkflowEvidenceKind::RepresentativeExecution,
        ) => Some(WorkflowEvidenceStrength::RepresentativeExecution),
        _ => None,
    }
}

fn validate_registry_sentinel(root: &Path) -> Result<(), Box<dyn Error>> {
    let registry_bytes = std::fs::read(root.join(P5D2_REGISTRY_PATH)).map_err(|cause| {
        error(format!(
            "embedded P5d.2 registry is unavailable at {P5D2_REGISTRY_PATH}: {cause}"
        ))
    })?;
    let found = sha256(&registry_bytes);
    if found != P5D2_REGISTRY_SHA256 {
        return Err(error(format!(
            "P5d.3 candidate generation cannot change admitted P5d.2 registry bytes: expected {P5D2_REGISTRY_SHA256}, found {found}"
        )));
    }
    Ok(())
}

fn validate_foundation_inputs(
    manifest: &WorkflowGovernanceReleaseManifestDocument,
    batch: &WorkflowMigrationBatchDocument,
    workflows: &[LoadedWorkflowDocument],
) -> Result<(), Box<dyn Error>> {
    let release = &manifest.workflow_governance_release_manifest;
    let golden = &batch.workflow_migration_batch;
    if release.release_id.0 != "workflow-governance.release.foundation-v0"
        || release.release_version != "0.1.0"
        || release.batches.len() != 1
        || release.batches[0].embedded_ref.0 != GOLDEN_BATCH_PATH
        || release.batches[0].expected_digest
            != workflow_migration_batch_digest(batch).map_err(error)?
        || golden.id.0 != "workflow-batch.golden-path-v0"
        || golden.policies.len() != 15
        || release.workflow_entries.len() != 110
    {
        return Err(error(
            "P5d.1 foundation artifacts are not the exact expected predecessor",
        ));
    }
    let workflow_digests = workflow_digest_map(workflows)?;
    for entry in &release.workflow_entries {
        if workflow_digests.get(&entry.workflow_id.0) != Some(&entry.legacy_workflow_digest) {
            return Err(error(format!(
                "foundation manifest legacy digest drift for {}",
                entry.workflow_id.0
            )));
        }
    }
    Ok(())
}

fn build_review_subject(
    root: &Path,
    workflows: &[LoadedWorkflowDocument],
    overlay_policies: &[WorkflowGovernancePolicy],
    runtime_bundle: &WorkflowGovernanceBundleDocument,
    quarantines: &BTreeMap<String, WorkflowQuarantine>,
    previous_release_digest: String,
) -> Result<WorkflowBehavioralReviewSubjectDocument, Box<dyn Error>> {
    let workflow_digests = workflow_digest_map(workflows)?;
    let mut candidate_workflows = overlay_policies
        .iter()
        .map(|policy| {
            let workflow_id = policy.compatibility_workflow_id.clone();
            Ok(WorkflowBehavioralCandidateWorkflowSubject {
                legacy_workflow_digest: workflow_digests
                    .get(&workflow_id.0)
                    .ok_or_else(|| error(format!("missing workflow {workflow_id:?}")))?
                    .clone(),
                workflow_id,
                policy_ref: policy.id.clone(),
                policy_digest: workflow_release_policy_digest(policy).map_err(error)?,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    candidate_workflows.sort_by(|left, right| left.workflow_id.0.cmp(&right.workflow_id.0));
    let quarantine_entries = quarantines
        .iter()
        .map(
            |(workflow_id, quarantine)| WorkflowBehavioralQuarantineSubject {
                workflow_id: id(workflow_id),
                quarantine: quarantine.clone(),
            },
        )
        .collect();
    let overlay_bytes = std::fs::read(root.join(OVERLAY_PATH))?;
    let baseline_bytes = std::fs::read(root.join(BASELINE_HISTORY_PATH))?;
    let (baseline_release, baseline_runtime_bundle) = baseline_upgrade(&baseline_bytes)?;
    let evaluator_source = std::fs::read(root.join(EVALUATOR_SOURCE_PATH)).map_err(|cause| {
        error(format!(
            "behavioral evaluator source is unavailable at {EVALUATOR_SOURCE_PATH}: {cause}"
        ))
    })?;
    Ok(WorkflowBehavioralReviewSubjectDocument {
        schema_version: WORKFLOW_BEHAVIORAL_REVIEW_SUBJECT_SCHEMA_VERSION.to_owned(),
        workflow_behavioral_review_subject: WorkflowBehavioralReviewSubject {
            id: id("workflow-review-subject.core-assurance-v0"),
            authority: WorkflowBehavioralReviewSubjectAuthority::CandidateOnly,
            overlay: WorkflowBehavioralArtifactReference {
                id: id("overlay.workflow-governance.core-assurance-v0"),
                embedded_ref: RepoPath(OVERLAY_PATH.to_owned()),
                expected_digest: sha256(&overlay_bytes),
            },
            baseline_history: WorkflowBehavioralArtifactReference {
                id: id("history.workflow-governance.p5d2-foundation-v0"),
                embedded_ref: RepoPath(BASELINE_HISTORY_PATH.to_owned()),
                expected_digest: sha256(&baseline_bytes),
            },
            baseline_release,
            baseline_runtime_bundle,
            runtime_bundle: WorkflowBehavioralRuntimeBundleSubject {
                bundle_id: runtime_bundle.workflow_governance_bundle.id.clone(),
                bundle_digest: workflow_runtime_bundle_digest(runtime_bundle).map_err(error)?,
                policy_set_digest: workflow_policy_set_digest(
                    &runtime_bundle.workflow_governance_bundle.policies,
                )
                .map_err(error)?,
            },
            proposed_batch: WorkflowBehavioralProposedBatchSubject {
                batch_id: id("workflow-governance.batch.core-assurance-v0"),
                batch_version: "0.2.0".to_owned(),
                previous_batch_digest: workflow_migration_batch_digest(&read_yaml(
                    &root.join(GOLDEN_BATCH_PATH),
                )?)
                .map_err(error)?,
            },
            proposed_release: WorkflowBehavioralProposedReleaseSubject {
                lineage_id: id("workflow-governance.core"),
                release_id: id("workflow-governance.release.core-assurance-v0"),
                release_version: "0.2.0".to_owned(),
                previous_release_digest,
            },
            evaluator: WorkflowBehavioralEvaluatorIdentity {
                evaluator_id: id("evaluator.workflow-behavioral-shadow"),
                evaluator_version: "0.1.0".to_owned(),
                governed_projection_version: "0.1.0".to_owned(),
                evaluator_source_digest: sha256(&evaluator_source),
            },
            candidate_workflows,
            quarantines: quarantine_entries,
        },
    })
}

fn baseline_upgrade(
    bytes: &[u8],
) -> Result<
    (
        forge_core_contracts::WorkflowGovernanceReleaseIdentity,
        forge_core_contracts::WorkflowRuntimeBundleIdentity,
    ),
    Box<dyn Error>,
> {
    let text = std::str::from_utf8(bytes)?;
    let mut records = Vec::new();
    for (index, line) in text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        let record: WorkflowGovernanceReceiptDocument =
            serde_json::from_str(line).map_err(|cause| {
                error(format!(
                    "cannot parse baseline history record {}: {cause}",
                    index + 1
                ))
            })?;
        records.push(record.workflow_governance_receipt);
    }
    if records.len() != 2 || records[0].sequence != 1 || records[1].sequence != 2 {
        return Err(error(
            "baseline history must contain exact import and foundation-upgrade records",
        ));
    }
    let WorkflowGovernanceEvent::ProjectImported(imported) = &records[0].event else {
        return Err(error(
            "baseline history first record is not project_imported",
        ));
    };
    let WorkflowGovernanceEvent::ReleaseUpgraded(upgraded) = &records[1].event else {
        return Err(error(
            "baseline history last record is not release_upgraded",
        ));
    };
    if records[1].previous_record_digest.as_deref() != Some(records[0].record_digest.as_str())
        || upgraded.prior_ledger_head_digest != records[0].record_digest
        || upgraded.admission_proof.snapshot_digest != imported.snapshot_digest
        || upgraded.to_release.release_id.0 != "workflow-governance.release.foundation-v0"
        || upgraded.to_runtime_bundle.bundle_id.0
            != "bundle.workflow-governance.release-foundation-v0"
    {
        return Err(error(
            "baseline history foundation lineage or digest chain is invalid",
        ));
    }
    Ok((
        upgraded.to_release.clone(),
        upgraded.to_runtime_bundle.clone(),
    ))
}

fn build_overlay_batch(
    root: &Path,
    workflows: &[LoadedWorkflowDocument],
    overlay_policies: &[WorkflowGovernancePolicy],
    foundation_batch: &WorkflowMigrationBatchDocument,
    previous_batch_digest: String,
) -> Result<WorkflowMigrationBatchDocument, Box<dyn Error>> {
    let workflow_digests = workflow_digest_map(workflows)?;
    let mut policies = overlay_policies.to_vec();
    policies.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    let mut bindings = policies
        .iter()
        .map(|policy| {
            Ok(WorkflowMigrationBatchBinding {
                workflow_id: policy.compatibility_workflow_id.clone(),
                legacy_workflow_digest: workflow_digests
                    .get(&policy.compatibility_workflow_id.0)
                    .ok_or_else(|| {
                        error(format!(
                            "overlay policy {} references missing workflow {}",
                            policy.id.0, policy.compatibility_workflow_id.0
                        ))
                    })?
                    .clone(),
                policy_ref: policy.id.clone(),
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    bindings.sort_by(|left, right| left.workflow_id.0.cmp(&right.workflow_id.0));
    let evidence = WorkflowMigrationBatchEvidence {
        representative_fixtures: vec![read_evidence_reference(root, REPRESENTATIVE_EVIDENCE_PATH)?],
        adversarial_fixtures: vec![read_evidence_reference(root, ADVERSARIAL_EVIDENCE_PATH)?],
        shadow_reports: vec![read_evidence_reference(root, SHADOW_REPORT_PATH)?],
    };
    if previous_batch_digest != workflow_migration_batch_digest(foundation_batch).map_err(error)? {
        return Err(error(
            "previous batch digest is not the exact P5d.1 batch digest",
        ));
    }
    Ok(WorkflowMigrationBatchDocument {
        schema_version: WORKFLOW_MIGRATION_BATCH_SCHEMA_VERSION.to_owned(),
        workflow_migration_batch: WorkflowMigrationBatch {
            id: id("workflow-governance.batch.core-assurance-v0"),
            batch_version: "0.2.0".to_owned(),
            authority: WorkflowMigrationBatchAuthority::CandidateOnly,
            source_catalog_digest: foundation_batch
                .workflow_migration_batch
                .source_catalog_digest
                .clone(),
            previous_batch_digest: Some(previous_batch_digest),
            evidence,
            workflow_bindings: bindings,
            policies,
        },
    })
}

fn build_candidate_manifest(
    foundation: &WorkflowGovernanceReleaseManifestDocument,
    workflows: &[LoadedWorkflowDocument],
    quarantines: &BTreeMap<String, WorkflowQuarantine>,
    overlay_batch_digest: String,
    previous_release_digest: String,
) -> Result<WorkflowGovernanceReleaseManifestDocument, Box<dyn Error>> {
    let mut manifest = foundation.clone();
    let release = &mut manifest.workflow_governance_release_manifest;
    release.release_id = id("workflow-governance.release.core-assurance-v0");
    "0.2.0".clone_into(&mut release.release_version);
    release.previous_release_digest = Some(previous_release_digest);
    release.batches.push(WorkflowReleaseBatchReference {
        batch_id: id("workflow-governance.batch.core-assurance-v0"),
        batch_version: "0.2.0".to_owned(),
        embedded_ref: RepoPath(BATCH_PATH.to_owned()),
        expected_digest: overlay_batch_digest,
        deterministic_order: 1,
    });
    release.compatibility_policy.replacement_argv = [
        "forge-core",
        "guide",
        "rollout-audit",
        "--manifest-file",
        MANIFEST_PATH,
        "--batch-file",
        GOLDEN_BATCH_PATH,
        "--batch-file",
        BATCH_PATH,
        "--json",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let workflow_digests = workflow_digest_map(workflows)?;
    let overlay_policy_by_workflow = OVERLAY_WORKFLOW_IDS
        .iter()
        .map(|workflow_id| (*workflow_id, id(&format!("policy.workflow.{workflow_id}"))))
        .collect::<BTreeMap<_, _>>();
    let mut counts = DispositionCounts::default();
    for entry in &mut release.workflow_entries {
        if let Some(policy_ref) = overlay_policy_by_workflow.get(entry.workflow_id.0.as_str()) {
            entry.disposition_intent = WorkflowReleaseDispositionIntent::MigrationCandidate {
                batch_id: id("workflow-governance.batch.core-assurance-v0"),
                policy_ref: policy_ref.clone(),
            };
        } else if let Some(quarantine) = quarantines.get(&entry.workflow_id.0) {
            entry.disposition_intent = WorkflowReleaseDispositionIntent::Quarantined {
                quarantine: quarantine.clone(),
            };
        }
        entry.legacy_workflow_digest = workflow_digests
            .get(&entry.workflow_id.0)
            .ok_or_else(|| error(format!("missing workflow {}", entry.workflow_id.0)))?
            .clone();
        match entry.disposition_intent {
            WorkflowReleaseDispositionIntent::MigrationCandidate { .. } => counts.migration += 1,
            WorkflowReleaseDispositionIntent::CompatibilityOnly { .. } => counts.compatibility += 1,
            WorkflowReleaseDispositionIntent::Quarantined { .. } => counts.quarantined += 1,
            WorkflowReleaseDispositionIntent::DomainPackCandidate { .. } => counts.domain += 1,
            WorkflowReleaseDispositionIntent::RetirementCandidate { .. } => {
                return Err(error(
                    "candidate manifest unexpectedly contains retirement intent",
                ));
            }
        }
    }
    if (
        counts.migration,
        counts.compatibility,
        counts.quarantined,
        counts.domain,
    ) != (20, 69, 3, 18)
    {
        return Err(error(format!(
            "unexpected candidate counts: migration={}, compatibility={}, quarantined={}, domain={}",
            counts.migration, counts.compatibility, counts.quarantined, counts.domain
        )));
    }
    Ok(manifest)
}

fn build_quarantines() -> BTreeMap<String, WorkflowQuarantine> {
    [
        (
            "edge-case-review",
            WorkflowQuarantine {
                reason_code: WorkflowQuarantineReasonCode::AmbiguousLegacyAuthority,
                risk_tier: WorkflowQuarantineRiskTier::High,
                explanation: "The legacy workflow overlaps adversarial-review without a closed review-mode taxonomy or precedence rule.".to_owned(),
                blocking_refs: ids(&[
                    "gap.workflow-review-mode.taxonomy",
                    "decision.workflow-review-precedence",
                ]),
                affected_consumer_refs: ids(&["consumer.workflow-review-routing"]),
                review_owner: id("owner.workflow-governance-review"),
                review_due_release_version: "0.3.0".to_owned(),
            },
        ),
        (
            "release-readiness",
            WorkflowQuarantine {
                reason_code: WorkflowQuarantineReasonCode::UnsafeAutomaticConversion,
                risk_tier: WorkflowQuarantineRiskTier::Critical,
                explanation: "The legacy workflow overlaps readiness-check and ready-release while mixing route, artifact, and release-authorization authority.".to_owned(),
                blocking_refs: ids(&[
                    "gap.release-readiness-overlap",
                    "gap.enterprise-artifact-adapter",
                    "gap.release-authorization-separation",
                ]),
                affected_consumer_refs: ids(&["consumer.release-governance"]),
                review_owner: id("owner.workflow-governance-review"),
                review_due_release_version: "0.3.0".to_owned(),
            },
        ),
        (
            "track-decision",
            WorkflowQuarantine {
                reason_code: WorkflowQuarantineReasonCode::AmbiguousLegacyAuthority,
                risk_tier: WorkflowQuarantineRiskTier::Critical,
                explanation: "The legacy workflow mixes route, module-pack, and enterprise artifact authority that core governance cannot safely infer.".to_owned(),
                blocking_refs: ids(&[
                    "gap.route-authority-model",
                    "gap.module-pack-lifecycle",
                    "gap.enterprise-artifact-adapter",
                ]),
                affected_consumer_refs: ids(&["consumer.workflow-routing"]),
                review_owner: id("owner.workflow-governance-review"),
                review_due_release_version: "0.3.0".to_owned(),
            },
        ),
    ]
    .into_iter()
    .map(|(workflow_id, quarantine)| (workflow_id.to_owned(), quarantine))
    .collect()
}

fn validate_final_invariants(
    manifest: &WorkflowGovernanceReleaseManifestDocument,
    batch: &WorkflowMigrationBatchDocument,
    runtime: &WorkflowGovernanceBundleDocument,
) -> Result<(), Box<dyn Error>> {
    let release = &manifest.workflow_governance_release_manifest;
    let quarantined = release
        .workflow_entries
        .iter()
        .filter_map(|entry| match entry.disposition_intent {
            WorkflowReleaseDispositionIntent::Quarantined { .. } => {
                Some(entry.workflow_id.0.as_str())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let expected_quarantine = QUARANTINED_WORKFLOW_IDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if quarantined != expected_quarantine
        || batch.workflow_migration_batch.policies.len() != 5
        || batch.workflow_migration_batch.workflow_bindings.len() != 5
        || runtime.workflow_governance_bundle.policies.len() != 20
        || release.batches.len() != 2
        || release.batches[0].embedded_ref.0 != GOLDEN_BATCH_PATH
        || release.batches[1].embedded_ref.0 != BATCH_PATH
    {
        return Err(error(
            "candidate composition failed exact invariant validation",
        ));
    }
    let quarantine_policy_ids = QUARANTINED_WORKFLOW_IDS
        .iter()
        .map(|workflow_id| format!("policy.workflow.{workflow_id}"))
        .collect::<BTreeSet<_>>();
    for policy in &runtime.workflow_governance_bundle.policies {
        if quarantine_policy_ids.contains(&policy.id.0)
            || policy
                .prerequisites
                .iter()
                .any(|prerequisite| quarantine_policy_ids.contains(&prerequisite.policy_ref.0))
        {
            return Err(error("quarantine leaked into runtime policy composition"));
        }
    }
    Ok(())
}

fn workflow_digest_map(
    workflows: &[LoadedWorkflowDocument],
) -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    workflows
        .iter()
        .map(|workflow| {
            Ok((
                workflow.document.workflow.id.0.clone(),
                workflow_release_legacy_digest(workflow).map_err(error)?,
            ))
        })
        .collect()
}

fn read_evidence_reference(
    root: &Path,
    relative_path: &'static str,
) -> Result<WorkflowMigrationEvidenceReference, Box<dyn Error>> {
    let bytes = std::fs::read(root.join(relative_path)).map_err(|cause| {
        error(format!(
            "required typed behavioral evidence is unavailable at {relative_path}: {cause}"
        ))
    })?;
    Ok(WorkflowMigrationEvidenceReference {
        embedded_ref: RepoPath(relative_path.to_owned()),
        expected_digest: sha256(&bytes),
    })
}

fn read_yaml<T>(path: &Path) -> Result<T, Box<dyn Error>>
where
    T: serde::de::DeserializeOwned,
{
    let text = std::fs::read_to_string(path)?;
    yaml_serde::from_str(&text).map_err(|cause| {
        error(format!(
            "cannot parse typed YAML {}: {cause}",
            path.display()
        ))
    })
}

fn yaml_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut text = yaml_serde::to_string(value)?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text.into_bytes())
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn ids(values: &[&str]) -> Vec<StableId> {
    values.iter().map(|value| id(value)).collect()
}

fn write_artifacts(root: &Path, artifacts: &[GeneratedArtifact]) -> Result<(), Box<dyn Error>> {
    for artifact in artifacts {
        let path = root.join(artifact.relative_path);
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
            |artifact| match std::fs::read(root.join(artifact.relative_path)) {
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
        println!("workflow core-assurance candidate artifacts are byte-exact");
        Ok(())
    } else {
        Err(error(format!(
            "workflow core-assurance candidate drift:\n{}\nrun the generator with --write",
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

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn admitted_registry_bytes_remain_the_frozen_p5d2_sentinel() {
        validate_registry_sentinel(&root()).expect("P5d.2 registry must remain byte-exact");
    }

    #[test]
    fn overlay_evaluators_have_only_authorizable_tuples() {
        let overlay: WorkflowGovernancePolicyOverlayDocument =
            read_yaml(&root().join(OVERLAY_PATH)).expect("typed overlay");
        validate_exact_policy_set(
            &overlay.workflow_governance_policy_overlay.policies,
            &OVERLAY_WORKFLOW_IDS,
            "overlay",
        )
        .expect("exact five overlay ids");
        validate_evaluator_authority_matrix(&overlay.workflow_governance_policy_overlay.policies)
            .expect("all new evaluator tuples must be authorizable");

        let mut impossible = overlay.workflow_governance_policy_overlay.policies;
        impossible[0].evaluators[0].accepted_evidence_kinds =
            vec![WorkflowEvidenceKind::HumanAcceptance];
        assert!(validate_evaluator_authority_matrix(&impossible).is_err());
    }

    #[test]
    fn composed_runtime_is_valid_and_quarantine_cannot_leak() {
        let runtime: WorkflowGovernanceBundleDocument =
            read_yaml(&root().join(RUNTIME_BUNDLE_PATH)).expect("candidate runtime bundle");
        assert_eq!(runtime.workflow_governance_bundle.policies.len(), 20);
        assert!(validate_workflow_governance_bundle(&runtime).is_empty());
        validate_composition(&runtime.workflow_governance_bundle.policies)
            .expect("closed 20-policy composition");

        let golden: WorkflowGovernanceBundleDocument =
            read_yaml(&root().join(GOLDEN_BUNDLE_PATH)).expect("frozen golden bundle");
        validate_frozen_predecessor_authority_paths(&golden.workflow_governance_bundle.policies)
            .expect("each frozen evaluator retains at least one usable authority path");
    }

    #[test]
    fn typed_review_subject_is_valid_deterministic_and_candidate_only() {
        let path = root().join(REVIEW_SUBJECT_PATH);
        let subject: WorkflowBehavioralReviewSubjectDocument =
            read_yaml(&path).expect("typed review subject");
        assert!(subject.validate().is_empty());
        assert_eq!(
            subject.workflow_behavioral_review_subject.authority,
            WorkflowBehavioralReviewSubjectAuthority::CandidateOnly
        );
        let first = yaml_bytes(&subject).expect("first deterministic encoding");
        let second = yaml_bytes(&subject).expect("second deterministic encoding");
        assert_eq!(first, second);
        assert_eq!(std::fs::read(path).expect("checked-in subject"), first);
    }
}
