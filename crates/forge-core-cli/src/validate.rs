//! Forge workspace contract validator.
//!
//! Walks the repository under a given root, parses every YAML contract
//! (commands, operations, side contracts, runtime contracts, evidence
//! registry, contract family inventory) and accumulates diagnostics into a
//! [`ValidateSummary`]. The summary is the regression anchor used by
//! `forge-core-cli validate --json` and by the parity tests in
//! `forge-contract-validator`. The shape of the JSON output MUST stay
//! stable; refactors here are behavior-preserving.

use crate::cli_error::ExitError;
use crate::cli_util::command_surface_usage;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use tracing::instrument;

use forge_core_command_surface::{CommandSpec, COMMAND_VALIDATE};
use forge_core_contracts::{
    AssuranceCaseDocument, ClaimContractDocument, CommandContractDocument,
    CompletionContractDocument, ContractFamilyInventoryDocument, CoordinationEvalContractDocument,
    DecisionCloseContractDocument, FieldEvidenceRegistry, GateContractDocument,
    HealthRecoveryContractDocument, OperationContractDocument, RequestContractDocument,
    RuntimeCapabilityDocument, RuntimeHandoffContractDocument, RuntimeRegistryEntryDocument,
    ToolEffectContractDocument, WorkflowBehavioralArtifactReference, WorkflowBehavioralCorpusClass,
    WorkflowBehavioralCorpusSetDocument, WorkflowBehavioralCoveragePolicyDocument,
    WorkflowBehavioralDisposition, WorkflowBehavioralReviewSubjectDocument,
    WorkflowBehavioralScenarioCorpusDocument, WorkflowBehavioralScenarioExecution,
    WorkflowBehavioralShadowReportDocument, WorkflowBehavioralVerdict,
    WorkflowGovernanceBundleDocument, WorkflowGovernancePolicyOverlayDocument,
    WorkflowGovernanceReleaseManifestDocument, WorkflowGovernanceReleaseRegistryDocument,
    WorkflowMigrationBatchDocument, WorkflowMigrationPlanDocument,
    WorkflowReleaseAdmissionAuthorizationDocument, WorkflowReleaseAdmissionAuthorizationV2Document,
    WorkflowReleaseDispositionIntent, WorkflowReleaseReviewIndexDocument,
    WorkflowReleaseReviewIndexV2Document, WorkflowReleaseReviewerRegistryDocument,
};
use forge_core_decisions::{
    evaluate_workflow_behavior, evaluate_workflow_migration, evaluate_workflow_release,
    evaluate_workflow_release_registry, load_catalog, load_workflow_documents,
    validate_workflow_governance_bundle, workflow_runtime_bundle_digest,
    WorkflowBehavioralBundleInput, WorkflowBehavioralCorpusInput, WorkflowBehavioralReportIdentity,
    WorkflowGovernanceIssue, WorkflowReleaseEvaluation, WorkflowReleaseEvaluationAuthority,
    WorkflowReleaseEvaluationStatus, WorkflowReleaseEvidenceAssurance,
    WorkflowReleaseRegistryEvaluationAuthority, WorkflowReleaseRegistryEvaluationStatus,
};
use forge_core_store::{collect_known_repo_paths, collect_validation_yaml_documents};
use forge_core_validate::{
    validate_assurance_case, validate_claim, validate_claim_cross_references, validate_command,
    validate_completion, validate_completion_cross_references, validate_coordination_eval,
    validate_coordination_eval_cross_references, validate_decision_close,
    validate_decision_close_cross_references, validate_evidence_registry, validate_gate,
    validate_gate_cross_references, validate_health_recovery,
    validate_health_recovery_cross_references, validate_inventory, validate_inventory_references,
    validate_operation, validate_operation_cross_references, validate_request,
    validate_request_cross_references, validate_runtime_capability, validate_runtime_handoff,
    validate_runtime_handoff_cross_references, validate_runtime_registry_cross_references,
    validate_runtime_registry_entry, validate_tool_effect, validate_tool_effect_cross_references,
    validate_yaml_known_repo_references, validate_yaml_source_id_references, Diagnostic,
    DiagnosticCode, DiagnosticSeverity, ReferenceIndex, ValidationReport,
};

/// Outcome of a single named validation check (passed/failed + counts).
#[derive(Debug, Clone, Serialize)]
pub struct ValidateCheck {
    pub name: String,
    pub status: ValidationStatus,
    pub diagnostics: usize,
    pub errors: usize,
}

/// One diagnostic emitted while validating the workspace.
///
/// Migrated in V2.B: this is now an alias for the canonical
/// `forge_core_validate::Diagnostic`, which keeps `DiagnosticSeverity` and
/// `DiagnosticCode` as the **strong enums** end-to-end instead of degrading
/// them to `String` at this boundary. The serialized JSON shape
/// (`{ severity, code, path, message }`) is unchanged — `severity` is still
/// `"error"`/`"warning"` and `code` is still a stable `snake_case` identifier,
/// but now produced by `DiagnosticCode`'s serde rename rather than the lossy
/// `format!("{:?}", diagnostic.code)` that previously emitted `PascalCase`
/// `Debug` strings (e.g. `YamlReadFailed`) and discarded the enum typing.
pub type ValidateDiagnostic = forge_core_validate::Diagnostic;

/// Aggregated result of validating a Forge workspace. Fields are public so
/// callers (`forge-contract-validator`, integration tests, `main.rs`) can
/// read status/checks/diagnostics directly. JSON output is produced by
/// `Serialize` and must remain stable.
#[derive(Debug, Clone, Serialize)]
pub struct ValidateSummary {
    pub status: ValidationStatus,
    pub root: String,
    pub checks: Vec<ValidateCheck>,
    pub diagnostics: Vec<ValidateDiagnostic>,
}

/// Top-level pass/fail status for a workspace validation run.
///
/// Serializes as lowercase (`"passed"` / `"failed"`) to match the rest of
/// the workspace's JSON contract (`gate_status`, `coordination` verdicts,
/// trace `gate` events). The original `PascalCase` emit was inconsistent
/// with every other status field in the binary.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ValidationStatus {
    Passed,
    Failed,
}

impl ValidateSummary {
    /// Returns `true` when every check passed and no error diagnostics were
    /// collected.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.status == ValidationStatus::Passed
    }

    /// One-line human-readable summary used by the legacy validator bridge.
    #[must_use]
    pub fn human_summary(&self) -> String {
        if self.passed() {
            format!(
                "forge_core_validation_passed checks={} diagnostics=0",
                self.checks.len()
            )
        } else {
            format!(
                "forge_core_validation_failed checks={} diagnostics={}",
                self.checks.len(),
                self.diagnostics.len()
            )
        }
    }

    fn add_report(&mut self, name: &str, report: ValidationReport) {
        let errors = report.error_count();
        let diagnostics = report.diagnostics().len();
        // V2.B: store the typed `Diagnostic` directly. Previously this demoted
        // the strong `DiagnosticCode` enum to a `format!("{:?}")` `Debug` string
        // via `ValidateDiagnostic::from_validation`, losing type information at
        // the boundary. Now the canonical enum flows straight into the summary.
        self.diagnostics
            .extend(report.diagnostics().iter().cloned());
        self.checks.push(ValidateCheck {
            name: name.to_string(),
            status: if errors == 0 {
                ValidationStatus::Passed
            } else {
                ValidationStatus::Failed
            },
            diagnostics,
            errors,
        });
    }

    fn add_validation_diagnostics(&mut self, name: &str, diagnostics: &[Diagnostic]) {
        let errors = diagnostics
            .iter()
            .filter(|item| item.severity == DiagnosticSeverity::Error)
            .count();
        // V2.B: clone the typed diagnostics in directly (no lossy conversion).
        self.diagnostics.extend(diagnostics.iter().cloned());
        self.checks.push(ValidateCheck {
            name: name.to_string(),
            status: if errors == 0 {
                ValidationStatus::Passed
            } else {
                ValidationStatus::Failed
            },
            diagnostics: diagnostics.len(),
            errors,
        });
    }

    fn push_diagnostic(&mut self, diagnostic: ValidateDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    fn finish(&mut self) {
        // V2.B: diagnostics are now typed `Diagnostic`, so compare against the
        // strong `DiagnosticSeverity::Error` enum rather than a `"error"` string.
        self.status = if self
            .diagnostics
            .iter()
            .any(|item| item.severity == DiagnosticSeverity::Error)
        {
            ValidationStatus::Failed
        } else {
            ValidationStatus::Passed
        };
    }
}

/// Detect whether `root` is a consumer project (no local `contracts/` tree)
/// versus the Forge core repo (which owns the canonical contracts).
///
/// A consumer created by `forge-core project init` ships no `contracts/`
/// directory: it carries only the `.forge-method.yaml` pointer and its runtime
/// state lives in the sibling sidecar. The shared contract definitions are
/// served from the binary (embedded), and the core-only fixtures
/// (`contracts/runtimes`, `docs/fixtures/operation-contract-v0`, the evidence
/// registry, the inventory) are not present. This predicate gates those
/// core-only validation passes so a consumer gets a clean validation rather
/// than hard errors for files it is not supposed to ship.
fn is_consumer_repo(root: &Path) -> bool {
    !root.join("contracts").is_dir()
}

/// Validate the Forge workspace rooted at `root`. Walks the contract tree,
/// builds a reference index, parses every YAML document, and accumulates
/// diagnostics into a [`ValidateSummary`] whose JSON shape is the regression
/// anchor for `forge-core-cli validate --json`.
pub fn run_validate(root: impl AsRef<Path>) -> ValidateSummary {
    let root = root.as_ref();
    let mut summary = ValidateSummary {
        status: ValidationStatus::Passed,
        root: root.to_string_lossy().into_owned(),
        checks: Vec::new(),
        diagnostics: Vec::new(),
    };

    // Build the reference index. Seed it with the embedded contract paths so
    // a consumer repo that ships no `contracts/` tree still resolves the
    // shared definitions served from the binary (disk copies still win when
    // present, via insert_existing).
    let embedded_refs = forge_core_decisions::embedded_yaml_paths();
    let index = match forge_core_store::ReferenceIndexBuilder::new()
        .with_known_embedded_refs(embedded_refs)
        .build(root)
    {
        Ok(index) => index,
        Err(err) => {
            summary.push_diagnostic(Diagnostic::error(
                DiagnosticCode::ReferenceIndexBuildFailed,
                "reference_index",
                err.to_string(),
            ));
            summary.finish();
            return summary;
        }
    };
    let yaml_documents = collect_validation_yaml_documents(root);
    let known_repo_paths = collect_known_repo_paths(root);
    summary.add_validation_diagnostics("yaml_parse", &yaml_documents.diagnostics);

    let evidence_path = root.join("contracts/research/field-evidence-20260625.yaml");
    // The evidence registry and inventory are core-only fixtures: a consumer
    // repo never ships them. When the consumer has no local contracts/ tree,
    // read them silently as None (skip the dependent checks) rather than
    // emitting hard errors for files the consumer is not supposed to have.
    let is_consumer = is_consumer_repo(root);
    let evidence = if is_consumer {
        None
    } else {
        read_yaml::<FieldEvidenceRegistry>(&evidence_path, &mut summary)
    };
    if let Some(evidence) = &evidence {
        summary.add_report("evidence_registry", validate_evidence_registry(evidence));
        summary.add_report(
            "yaml_source_id_refs",
            validate_yaml_source_id_references(&yaml_documents.documents, evidence),
        );
    }
    summary.add_report(
        "yaml_known_repo_refs",
        validate_yaml_known_repo_references(&yaml_documents.documents, &known_repo_paths),
    );

    let inventory_path = root.join("contracts/inventory/v0-contract-family-lock.yaml");
    let inventory = if is_consumer {
        None
    } else {
        read_yaml::<ContractFamilyInventoryDocument>(&inventory_path, &mut summary)
    };
    if let Some(inventory) = &inventory {
        if let Some(evidence) = &evidence {
            summary.add_report("inventory", validate_inventory(inventory, evidence));
        }
        summary.add_report(
            "inventory_references",
            validate_inventory_references(inventory, &index),
        );
    }

    validate_named_dir_instances::<CommandContractDocument, _>(
        root,
        "contracts/commands",
        "command-contract-v0.yaml",
        "command_contracts",
        &mut summary,
        validate_command,
    );
    validate_named_dir_instances::<AssuranceCaseDocument, _>(
        root,
        "contracts/assurance",
        "assurance-case-contract-v0.yaml",
        "assurance_case",
        &mut summary,
        validate_assurance_case,
    );
    // Operation fixtures (docs/fixtures/operation-contract-v0), side-contract
    // instances, and runtime contracts (contracts/runtimes/*.yaml) are
    // core-only fixtures. A consumer repo never ships them, so skip these
    // checks entirely when running against a consumer (no local contracts/
    // tree). The consumer's own instances, if any, live under contracts/ and
    // are already covered by the named-dir-instance + known-repo-ref checks
    // above when present.
    if !is_consumer {
        validate_operation_fixtures(root, &index, &mut summary);
        validate_side_contracts(root, &index, &mut summary);
        validate_runtime_contracts(root, &index, &mut summary);
        validate_workflow_governance_contracts(root, &mut summary);
        validate_workflow_release_foundation(root, &mut summary);
        validate_workflow_release_registry(root, &mut summary);
        validate_workflow_behavioral_evidence(root, &mut summary);
        validate_workflow_release_independent_admission(root, &mut summary);
        validate_workflow_release_v2_admission(root, &mut summary);
    }

    summary.finish();
    summary
}

/// Validate the core-owned workflow policy bundles. P5b registers
/// the canonical kernel bundle only; consumer and domain-pack bundles are not
/// admitted by this pass yet and remain work for P5c/P6. The check therefore
/// stays behind the core-only boundary in [`run_validate`].
fn validate_workflow_governance_contracts(root: &Path, summary: &mut ValidateSummary) {
    let dir = root.join("contracts/workflow-governance");
    for path in yaml_files(&dir, summary) {
        if path.file_name().and_then(|value| value.to_str())
            == Some("workflow-governance-contract-v0.yaml")
        {
            continue;
        }
        if let Some(bundle) = read_yaml::<WorkflowGovernanceBundleDocument>(&path, summary) {
            let issues = validate_workflow_governance_bundle(&bundle);
            summary.add_report(
                &format!("workflow_governance_bundle:{}", repo_relative(root, &path)),
                workflow_governance_validation_report(issues),
            );
        }
    }
}

fn workflow_governance_validation_report(issues: Vec<WorkflowGovernanceIssue>) -> ValidationReport {
    let mut report = ValidationReport::new();
    for issue in issues {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            issue.path,
            format!("workflow governance {:?}: {}", issue.code, issue.message),
        ));
    }
    report
}

const WORKFLOW_RELEASE_SPEC_REF: &str = "contracts/spec/workflow-governance-release-v0.yaml";
const WORKFLOW_RELEASE_FOUNDATION_REF: &str =
    "contracts/migration/workflow-governance-release-foundation-v0.yaml";
const WORKFLOW_RELEASE_FOUNDATION_BATCH_REF: &str =
    "contracts/migration/workflow-governance-batch-golden-path-v0.yaml";
const WORKFLOW_MIGRATION_PLAN_REF: &str =
    "contracts/policies/workflow-migration-foundation-v0.yaml";

/// Validate the repository-owned P5d.1 aggregate, rather than merely claiming
/// its evaluator in the inventory. Compatibility/domain blockers are expected
/// foundation dispositions; only malformed, non-canonical, non-candidate-only,
/// or semantically invalid aggregate input fails this structural check.
fn validate_workflow_release_foundation(root: &Path, summary: &mut ValidateSummary) {
    let report = workflow_release_foundation_validation_report(root);
    summary.add_report("workflow_governance_release_foundation", report);
}

#[allow(clippy::too_many_lines)]
fn workflow_release_foundation_validation_report(root: &Path) -> ValidationReport {
    let mut report = ValidationReport::new();

    // The human-readable spec is part of the aggregate boundary too. Comparing
    // parsed YAML against the compiled repository copy detects missing or
    // semantically changed checkout content without depending on line endings.
    if read_canonical_release_yaml::<serde_json::Value>(
        root,
        WORKFLOW_RELEASE_SPEC_REF,
        &mut report,
    )
    .is_none()
    {
        return report;
    }

    let Some(manifest) = read_canonical_release_yaml::<WorkflowGovernanceReleaseManifestDocument>(
        root,
        WORKFLOW_RELEASE_FOUNDATION_REF,
        &mut report,
    ) else {
        return report;
    };

    let batch_refs = &manifest.workflow_governance_release_manifest.batches;
    if batch_refs.len() != 1
        || batch_refs[0].embedded_ref.0 != WORKFLOW_RELEASE_FOUNDATION_BATCH_REF
    {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            WORKFLOW_RELEASE_FOUNDATION_REF,
            format!("P5d.1 foundation must bind exactly {WORKFLOW_RELEASE_FOUNDATION_BATCH_REF}"),
        ));
    }

    let mut batches = Vec::with_capacity(batch_refs.len());
    for reference in batch_refs {
        let repo_ref = reference.embedded_ref.0.as_str();
        if !is_safe_migration_repo_ref(repo_ref) {
            report.push(Diagnostic::error(
                DiagnosticCode::WorkflowGovernanceInvalid,
                format!(
                    "{WORKFLOW_RELEASE_FOUNDATION_REF}.batches.{}.embedded_ref",
                    reference.batch_id.0
                ),
                "batch embedded_ref must be a safe contracts/migration repo-relative YAML path",
            ));
            continue;
        }
        if let Some(batch) = read_canonical_release_yaml::<WorkflowMigrationBatchDocument>(
            root,
            repo_ref,
            &mut report,
        ) {
            batches.push(batch);
        }
    }

    let Some(plan) = read_release_yaml::<WorkflowMigrationPlanDocument>(
        root,
        WORKFLOW_MIGRATION_PLAN_REF,
        &mut report,
    ) else {
        return report;
    };

    let catalog_dir = root.join("contracts/workflows");
    let workflows = load_workflow_documents(&catalog_dir);
    for error in &workflows.errors {
        report.push(Diagnostic::error(
            DiagnosticCode::ParseYamlFailed,
            format!("contracts/workflows/{}", error.path.0),
            error.reason.clone(),
        ));
    }
    let catalog = load_catalog(&catalog_dir);
    for error in &catalog.errors {
        report.push(Diagnostic::error(
            DiagnosticCode::ParseYamlFailed,
            format!("contracts/workflows/{}", error.path.0),
            error.reason.clone(),
        ));
    }
    if !workflows.is_clean() || !catalog.is_clean() {
        return report;
    }

    let migration_audit =
        evaluate_workflow_migration(&plan, &workflows.workflows, &catalog.catalog);
    let evaluation =
        evaluate_workflow_release(&manifest, &batches, &migration_audit, &workflows.workflows);

    for issue in &evaluation.issues {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            issue.path.clone(),
            format!("workflow release {:?}: {}", issue.code, issue.message),
        ));
    }
    if evaluation.authority != WorkflowReleaseEvaluationAuthority::CandidateOnly {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            WORKFLOW_RELEASE_FOUNDATION_REF,
            "P5d.1 repository validation may only derive candidate_only authority",
        ));
    }
    if evaluation.status != WorkflowReleaseEvaluationStatus::StructurallyValid
        && evaluation.issues.is_empty()
    {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            WORKFLOW_RELEASE_FOUNDATION_REF,
            "P5d.1 foundation is not structurally_valid",
        ));
    }
    validate_workflow_release_foundation_baseline(&evaluation, &mut report);

    report
}

fn validate_workflow_release_foundation_baseline(
    evaluation: &WorkflowReleaseEvaluation,
    report: &mut ValidationReport,
) {
    if evaluation.evidence_assurance != WorkflowReleaseEvidenceAssurance::ContentIntegrityOnly {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            WORKFLOW_RELEASE_FOUNDATION_REF,
            "P5d.1 foundation evidence assurance must remain content_integrity_only",
        ));
    }

    let counts = &evaluation.counts;
    let actual_counts = (
        counts.migration_candidate_structurally_valid,
        counts.compatibility_only,
        counts.quarantined,
        counts.domain_pack_candidate,
        counts.retirement_pending_verification,
    );
    let expected_counts = (15, 77, 0, 18, 0);
    if actual_counts != expected_counts {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            WORKFLOW_RELEASE_FOUNDATION_REF,
            format!(
                "P5d.1 derived scorecard drift: expected migration/compatibility/quarantine/domain/retirement {expected_counts:?}, found {actual_counts:?}"
            ),
        ));
    }
    if evaluation.assessments.len() != 110 {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            WORKFLOW_RELEASE_FOUNDATION_REF,
            format!(
                "P5d.1 foundation must derive 110 assessments, found {}",
                evaluation.assessments.len()
            ),
        ));
    }
    if evaluation.non_executable_gaps.len() != 95 {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            WORKFLOW_RELEASE_FOUNDATION_REF,
            format!(
                "P5d.1 foundation must preserve 95 explicit non-executable gaps, found {}",
                evaluation.non_executable_gaps.len()
            ),
        ));
    }
}

const WORKFLOW_RELEASE_ADMISSION_SPEC_REF: &str =
    "contracts/spec/workflow-governance-release-admission-v0.yaml";
const WORKFLOW_RELEASE_REGISTRY_REF: &str =
    "contracts/migration/workflow-governance-release-registry-v0.yaml";
const WORKFLOW_RELEASE_GENESIS_BUNDLE_REF: &str =
    "contracts/workflow-governance/golden-path-v0.yaml";
const WORKFLOW_RELEASE_FOUNDATION_BUNDLE_REF: &str =
    "contracts/workflow-governance/runtime-release-foundation-v0.yaml";

/// Validate the complete repository-owned P5d.2 registry projection. A clean
/// report proves only structural integrity and exact P5c policy equivalence;
/// runtime admission remains an opaque kernel operation.
fn validate_workflow_release_registry(root: &Path, summary: &mut ValidateSummary) {
    let report = workflow_release_registry_validation_report(root);
    summary.add_report("workflow_governance_release_registry", report);
}

fn workflow_release_registry_validation_report(root: &Path) -> ValidationReport {
    let mut report = ValidationReport::new();
    if read_canonical_release_yaml::<serde_json::Value>(
        root,
        WORKFLOW_RELEASE_ADMISSION_SPEC_REF,
        &mut report,
    )
    .is_none()
    {
        return report;
    }
    let Some(registry) = read_canonical_release_yaml::<WorkflowGovernanceReleaseRegistryDocument>(
        root,
        WORKFLOW_RELEASE_REGISTRY_REF,
        &mut report,
    ) else {
        return report;
    };

    let expected_refs = [
        WORKFLOW_RELEASE_GENESIS_BUNDLE_REF,
        WORKFLOW_RELEASE_FOUNDATION_BUNDLE_REF,
    ];
    let actual_refs = registry
        .workflow_governance_release_registry
        .releases
        .iter()
        .map(|entry| entry.runtime_bundle.embedded_ref.0.as_str())
        .collect::<Vec<_>>();
    if actual_refs != expected_refs {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            WORKFLOW_RELEASE_REGISTRY_REF,
            format!(
                "P5d.2 foundation registry must bind the exact ordered runtime bundles {expected_refs:?}, found {actual_refs:?}"
            ),
        ));
        return report;
    }

    let mut bundles = Vec::with_capacity(expected_refs.len());
    for repo_ref in expected_refs {
        if let Some(bundle) = read_canonical_release_yaml::<WorkflowGovernanceBundleDocument>(
            root,
            repo_ref,
            &mut report,
        ) {
            bundles.push(bundle);
        }
    }
    if bundles.len() != expected_refs.len() {
        return report;
    }

    let evaluation = evaluate_workflow_release_registry(&registry, &bundles);
    for issue in &evaluation.issues {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            issue.path.clone(),
            format!(
                "workflow release registry {:?}: {}",
                issue.code, issue.message
            ),
        ));
    }
    if evaluation.status != WorkflowReleaseRegistryEvaluationStatus::StructurallyValid
        && evaluation.issues.is_empty()
    {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            WORKFLOW_RELEASE_REGISTRY_REF,
            "P5d.2 release registry is not structurally_valid",
        ));
    }
    if evaluation.authority != WorkflowReleaseRegistryEvaluationAuthority::NonAuthoritative {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            WORKFLOW_RELEASE_REGISTRY_REF,
            "raw P5d.2 registry evaluation must remain non_authoritative",
        ));
    }
    if evaluation.successor_policy_count != 15 {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            WORKFLOW_RELEASE_REGISTRY_REF,
            format!(
                "P5d.2 may grandfather exactly 15 P5c policy objects, found {}",
                evaluation.successor_policy_count
            ),
        ));
    }
    report
}

const WORKFLOW_BEHAVIOR_SPEC_REF: &str = "contracts/spec/workflow-behavioral-evidence-v0.yaml";
const WORKFLOW_BEHAVIOR_OVERLAY_REF: &str =
    "contracts/policies/workflow-core-assurance-overlay-v0.yaml";
const WORKFLOW_BEHAVIOR_REVIEW_SUBJECT_REF: &str =
    "contracts/migration/workflow-core-assurance-review-subject-v0.yaml";
const WORKFLOW_BEHAVIOR_COVERAGE_REF: &str =
    "contracts/policies/workflow-behavioral-coverage-v0.yaml";
const WORKFLOW_BEHAVIOR_CORPUS_SET_REF: &str =
    "contracts/evidence/workflow-core-assurance-corpus-set-v0.yaml";
const WORKFLOW_BEHAVIOR_REPRESENTATIVE_REF: &str =
    "contracts/evidence/workflow-core-assurance-representative-v0.yaml";
const WORKFLOW_BEHAVIOR_ADVERSARIAL_REF: &str =
    "contracts/evidence/workflow-core-assurance-adversarial-v0.yaml";
const WORKFLOW_BEHAVIOR_REPORT_REF: &str =
    "contracts/evidence/workflow-core-assurance-shadow-report-v0.yaml";
const WORKFLOW_BEHAVIOR_CANDIDATE_BUNDLE_REF: &str =
    "contracts/workflow-governance/runtime-core-assurance-candidate-v0.yaml";
const WORKFLOW_BEHAVIOR_CANDIDATE_BATCH_REF: &str =
    "contracts/migration/workflow-governance-batch-core-assurance-v0.yaml";
const WORKFLOW_BEHAVIOR_CANDIDATE_MANIFEST_REF: &str =
    "contracts/migration/workflow-governance-release-core-assurance-candidate-v0.yaml";
const WORKFLOW_BEHAVIOR_REPORT_ID: &str = "report.workflow-core-assurance.shadow-v0";
const WORKFLOW_BEHAVIOR_REPORT_VERSION: &str = "0.1.0";

const WORKFLOW_BEHAVIOR_ABLATED_BUNDLE_REFS: [&str; 5] = [
    "contracts/workflow-governance/ablated-core-assurance-adversarial-review-v0.yaml",
    "contracts/workflow-governance/ablated-core-assurance-code-review-v0.yaml",
    "contracts/workflow-governance/ablated-core-assurance-nfr-evidence-audit-v0.yaml",
    "contracts/workflow-governance/ablated-core-assurance-risk-register-v0.yaml",
    "contracts/workflow-governance/ablated-core-assurance-traceability-gate-v0.yaml",
];

const ASSURANCE_OPERATIONS_OVERLAY_REF: &str =
    "contracts/policies/workflow-assurance-operations-overlay-v0.yaml";
const ASSURANCE_OPERATIONS_REVIEW_SUBJECT_REF: &str =
    "contracts/migration/workflow-assurance-operations-review-subject-v0.yaml";
const ASSURANCE_OPERATIONS_COVERAGE_REF: &str =
    "contracts/policies/workflow-behavioral-coverage-assurance-operations-v0.yaml";
const ASSURANCE_OPERATIONS_CORPUS_SET_REF: &str =
    "contracts/evidence/workflow-assurance-operations-corpus-set-v0.yaml";
const ASSURANCE_OPERATIONS_REPRESENTATIVE_REF: &str =
    "contracts/evidence/workflow-assurance-operations-representative-v0.yaml";
const ASSURANCE_OPERATIONS_ADVERSARIAL_REF: &str =
    "contracts/evidence/workflow-assurance-operations-adversarial-v0.yaml";
const ASSURANCE_OPERATIONS_REPORT_REF: &str =
    "contracts/evidence/workflow-assurance-operations-shadow-report-v0.yaml";
const ASSURANCE_OPERATIONS_CANDIDATE_BUNDLE_REF: &str =
    "contracts/workflow-governance/runtime-assurance-operations-candidate-v0.yaml";
const ASSURANCE_OPERATIONS_BATCH_REF: &str =
    "contracts/migration/workflow-governance-batch-assurance-operations-v0.yaml";
const ASSURANCE_OPERATIONS_MANIFEST_REF: &str =
    "contracts/migration/workflow-governance-release-assurance-operations-candidate-v0.yaml";
const ASSURANCE_OPERATIONS_REPORT_ID: &str = "report.workflow-assurance-operations.shadow-v0";
const ASSURANCE_OPERATIONS_ABLATED_REFS: [&str; 13] = [
    "contracts/workflow-governance/ablated-assurance-operations-atdd-plan-v0.yaml",
    "contracts/workflow-governance/ablated-assurance-operations-ci-quality-pipeline-v0.yaml",
    "contracts/workflow-governance/ablated-assurance-operations-compliance-checklist-v0.yaml",
    "contracts/workflow-governance/ablated-assurance-operations-devops-deployment-plan-v0.yaml",
    "contracts/workflow-governance/ablated-assurance-operations-eval-design-v0.yaml",
    "contracts/workflow-governance/ablated-assurance-operations-investigation-v0.yaml",
    "contracts/workflow-governance/ablated-assurance-operations-observability-plan-v0.yaml",
    "contracts/workflow-governance/ablated-assurance-operations-platform-ops-plan-v0.yaml",
    "contracts/workflow-governance/ablated-assurance-operations-privacy-data-plan-v0.yaml",
    "contracts/workflow-governance/ablated-assurance-operations-security-plan-v0.yaml",
    "contracts/workflow-governance/ablated-assurance-operations-test-automation-v0.yaml",
    "contracts/workflow-governance/ablated-assurance-operations-test-framework-v0.yaml",
    "contracts/workflow-governance/ablated-assurance-operations-test-review-v0.yaml",
];

const AGENT_NATIVE_CONTINUITY_ABLATED_REFS: [&str; 9] = [
    "contracts/workflow-governance/ablated-agent-native-continuity-checkpoint-preview-v0.yaml",
    "contracts/workflow-governance/ablated-agent-native-continuity-collaboration-handoff-v0.yaml",
    "contracts/workflow-governance/ablated-agent-native-continuity-evolve-project-v0.yaml",
    "contracts/workflow-governance/ablated-agent-native-continuity-product-area-map-v0.yaml",
    "contracts/workflow-governance/ablated-agent-native-continuity-project-context-v0.yaml",
    "contracts/workflow-governance/ablated-agent-native-continuity-research-closeout-v0.yaml",
    "contracts/workflow-governance/ablated-agent-native-continuity-retrospective-v0.yaml",
    "contracts/workflow-governance/ablated-agent-native-continuity-spec-distillation-v0.yaml",
    "contracts/workflow-governance/ablated-agent-native-continuity-sprint-status-v0.yaml",
];

struct BehavioralValidationProfile {
    overlay: &'static str,
    review_subject: &'static str,
    coverage: &'static str,
    corpus_set: &'static str,
    representative: &'static str,
    adversarial: &'static str,
    report: &'static str,
    candidate_bundle: &'static str,
    candidate_batch: &'static str,
    candidate_manifest: &'static str,
    report_id: &'static str,
    ablated_bundles: &'static [&'static str],
    workflow_count: usize,
    bundle_policy_count: usize,
    disposition_counts: [usize; 5],
    predecessor_batches: &'static [&'static str],
    admitted_registry: &'static str,
}

const CORE_ASSURANCE_BEHAVIORAL_PROFILE: BehavioralValidationProfile =
    BehavioralValidationProfile {
        overlay: WORKFLOW_BEHAVIOR_OVERLAY_REF,
        review_subject: WORKFLOW_BEHAVIOR_REVIEW_SUBJECT_REF,
        coverage: WORKFLOW_BEHAVIOR_COVERAGE_REF,
        corpus_set: WORKFLOW_BEHAVIOR_CORPUS_SET_REF,
        representative: WORKFLOW_BEHAVIOR_REPRESENTATIVE_REF,
        adversarial: WORKFLOW_BEHAVIOR_ADVERSARIAL_REF,
        report: WORKFLOW_BEHAVIOR_REPORT_REF,
        candidate_bundle: WORKFLOW_BEHAVIOR_CANDIDATE_BUNDLE_REF,
        candidate_batch: WORKFLOW_BEHAVIOR_CANDIDATE_BATCH_REF,
        candidate_manifest: WORKFLOW_BEHAVIOR_CANDIDATE_MANIFEST_REF,
        report_id: WORKFLOW_BEHAVIOR_REPORT_ID,
        ablated_bundles: &WORKFLOW_BEHAVIOR_ABLATED_BUNDLE_REFS,
        workflow_count: 5,
        bundle_policy_count: 20,
        disposition_counts: [20, 69, 3, 18, 0],
        predecessor_batches: &[WORKFLOW_RELEASE_FOUNDATION_BATCH_REF],
        admitted_registry: WORKFLOW_RELEASE_REGISTRY_REF,
    };

const ASSURANCE_OPERATIONS_BEHAVIORAL_PROFILE: BehavioralValidationProfile =
    BehavioralValidationProfile {
        overlay: ASSURANCE_OPERATIONS_OVERLAY_REF,
        review_subject: ASSURANCE_OPERATIONS_REVIEW_SUBJECT_REF,
        coverage: ASSURANCE_OPERATIONS_COVERAGE_REF,
        corpus_set: ASSURANCE_OPERATIONS_CORPUS_SET_REF,
        representative: ASSURANCE_OPERATIONS_REPRESENTATIVE_REF,
        adversarial: ASSURANCE_OPERATIONS_ADVERSARIAL_REF,
        report: ASSURANCE_OPERATIONS_REPORT_REF,
        candidate_bundle: ASSURANCE_OPERATIONS_CANDIDATE_BUNDLE_REF,
        candidate_batch: ASSURANCE_OPERATIONS_BATCH_REF,
        candidate_manifest: ASSURANCE_OPERATIONS_MANIFEST_REF,
        report_id: ASSURANCE_OPERATIONS_REPORT_ID,
        ablated_bundles: &ASSURANCE_OPERATIONS_ABLATED_REFS,
        workflow_count: 13,
        bundle_policy_count: 33,
        disposition_counts: [33, 56, 3, 18, 0],
        predecessor_batches: &[
            WORKFLOW_RELEASE_FOUNDATION_BATCH_REF,
            "contracts/migration/workflow-governance-batch-core-assurance-v0.yaml",
        ],
        admitted_registry:
            "contracts/migration/workflow-governance-release-registry-core-assurance-v0.yaml",
    };

const AGENT_NATIVE_CONTINUITY_BEHAVIORAL_PROFILE: BehavioralValidationProfile =
    BehavioralValidationProfile {
        overlay: "contracts/policies/workflow-agent-native-continuity-overlay-v0.yaml",
        review_subject: "contracts/migration/workflow-agent-native-continuity-review-subject-v0.yaml",
        coverage: "contracts/policies/workflow-behavioral-coverage-agent-native-continuity-v0.yaml",
        corpus_set: "contracts/evidence/workflow-agent-native-continuity-corpus-set-v0.yaml",
        representative: "contracts/evidence/workflow-agent-native-continuity-representative-v0.yaml",
        adversarial: "contracts/evidence/workflow-agent-native-continuity-adversarial-v0.yaml",
        report: "contracts/evidence/workflow-agent-native-continuity-shadow-report-v0.yaml",
        candidate_bundle: "contracts/workflow-governance/runtime-agent-native-continuity-candidate-v0.yaml",
        candidate_batch: "contracts/migration/workflow-governance-batch-agent-native-continuity-v0.yaml",
        candidate_manifest: "contracts/migration/workflow-governance-release-agent-native-continuity-candidate-v0.yaml",
        report_id: "report.workflow-agent-native-continuity.shadow-v0",
        ablated_bundles: &AGENT_NATIVE_CONTINUITY_ABLATED_REFS,
        workflow_count: 9,
        bundle_policy_count: 42,
        disposition_counts: [42, 47, 3, 18, 0],
        predecessor_batches: &[
            WORKFLOW_RELEASE_FOUNDATION_BATCH_REF,
            "contracts/migration/workflow-governance-batch-core-assurance-v0.yaml",
            ASSURANCE_OPERATIONS_BATCH_REF,
        ],
        admitted_registry: WORKFLOW_RELEASE_V2_REGISTRY_REF,
    };

fn validate_workflow_behavioral_evidence(root: &Path, summary: &mut ValidateSummary) {
    summary.add_report(
        "workflow_behavioral_evidence_candidate",
        workflow_behavioral_evidence_validation_report(root, &CORE_ASSURANCE_BEHAVIORAL_PROFILE),
    );
    summary.add_report(
        "workflow_behavioral_evidence_assurance_operations",
        workflow_behavioral_evidence_validation_report(
            root,
            &ASSURANCE_OPERATIONS_BEHAVIORAL_PROFILE,
        ),
    );
    summary.add_report(
        "workflow_behavioral_evidence_agent_native_continuity",
        workflow_behavioral_evidence_validation_report(
            root,
            &AGENT_NATIVE_CONTINUITY_BEHAVIORAL_PROFILE,
        ),
    );
}

#[allow(clippy::too_many_lines)]
fn workflow_behavioral_evidence_validation_report(
    root: &Path,
    profile: &BehavioralValidationProfile,
) -> ValidationReport {
    let mut report = ValidationReport::new();
    let Some((spec, _)) =
        read_behavioral_yaml::<serde_json::Value>(root, WORKFLOW_BEHAVIOR_SPEC_REF, &mut report)
    else {
        return report;
    };
    let spec_status = spec.get("status").and_then(serde_json::Value::as_str);
    if spec.get("spec").and_then(serde_json::Value::as_str)
        != Some("workflow_behavioral_evidence_v0")
        || !matches!(spec_status, Some("p5d_3_in_progress" | "p5d_3_implemented"))
    {
        behavioral_error(
            &mut report,
            WORKFLOW_BEHAVIOR_SPEC_REF,
            "P5d.3 behavioral spec identity/status must remain the closed workflow_behavioral_evidence_v0 lifecycle",
        );
    }

    let Some((overlay, overlay_bytes)) = read_behavioral_yaml::<
        WorkflowGovernancePolicyOverlayDocument,
    >(root, profile.overlay, &mut report) else {
        return report;
    };
    let Some((review_subject, review_subject_bytes)) = read_behavioral_yaml::<
        WorkflowBehavioralReviewSubjectDocument,
    >(
        root, profile.review_subject, &mut report
    ) else {
        return report;
    };
    push_behavioral_contract_issues(
        &mut report,
        profile.review_subject,
        review_subject.validate(),
    );
    let Some((coverage, coverage_bytes)) = read_behavioral_yaml::<
        WorkflowBehavioralCoveragePolicyDocument,
    >(root, profile.coverage, &mut report) else {
        return report;
    };
    push_behavioral_contract_issues(&mut report, profile.coverage, coverage.validate());
    let Some((corpus_set, corpus_set_bytes)) = read_behavioral_yaml::<
        WorkflowBehavioralCorpusSetDocument,
    >(root, profile.corpus_set, &mut report) else {
        return report;
    };
    push_behavioral_contract_issues(&mut report, profile.corpus_set, corpus_set.validate());

    let mut corpus_documents = Vec::new();
    for repo_ref in [profile.representative, profile.adversarial] {
        let Some((document, bytes)) = read_behavioral_yaml::<
            WorkflowBehavioralScenarioCorpusDocument,
        >(root, repo_ref, &mut report) else {
            return report;
        };
        push_behavioral_contract_issues(&mut report, repo_ref, document.validate());
        corpus_documents.push((repo_ref, document, bytes));
    }
    if corpus_documents[0]
        .1
        .workflow_behavioral_scenario_corpus
        .partition_class
        != WorkflowBehavioralCorpusClass::Representative
        || corpus_documents[1]
            .1
            .workflow_behavioral_scenario_corpus
            .partition_class
            != WorkflowBehavioralCorpusClass::Adversarial
    {
        behavioral_error(
            &mut report,
            profile.corpus_set,
            "corpus set must contain representative then adversarial partitions",
        );
    }

    let Some((checked_report, checked_report_bytes)) = read_behavioral_yaml::<
        WorkflowBehavioralShadowReportDocument,
    >(root, profile.report, &mut report) else {
        return report;
    };
    push_behavioral_contract_issues(&mut report, profile.report, checked_report.validate());

    let mut bundle_sources = Vec::new();
    for repo_ref in
        std::iter::once(profile.candidate_bundle).chain(profile.ablated_bundles.iter().copied())
    {
        let Some((bundle, bytes)) =
            read_behavioral_yaml::<WorkflowGovernanceBundleDocument>(root, repo_ref, &mut report)
        else {
            return report;
        };
        for issue in validate_workflow_governance_bundle(&bundle) {
            behavioral_error(
                &mut report,
                format!("{repo_ref}.{}", issue.path),
                format!("workflow bundle {:?}: {}", issue.code, issue.message),
            );
        }
        bundle_sources.push((repo_ref, bundle, bytes));
    }

    let Some((candidate_batch, candidate_batch_bytes)) =
        read_behavioral_yaml::<WorkflowMigrationBatchDocument>(
            root,
            profile.candidate_batch,
            &mut report,
        )
    else {
        return report;
    };
    let Some((candidate_manifest, candidate_manifest_bytes)) =
        read_behavioral_yaml::<WorkflowGovernanceReleaseManifestDocument>(
            root,
            profile.candidate_manifest,
            &mut report,
        )
    else {
        return report;
    };

    validate_behavioral_candidate_composition(
        profile,
        &overlay,
        &review_subject,
        &bundle_sources[0].1,
        &candidate_batch,
        &candidate_manifest,
        &mut report,
    );

    let mut source_bytes = HashMap::new();
    for (repo_ref, bytes) in [
        (profile.overlay, overlay_bytes),
        (profile.review_subject, review_subject_bytes),
        (profile.coverage, coverage_bytes.clone()),
        (profile.corpus_set, corpus_set_bytes.clone()),
        (profile.report, checked_report_bytes),
        (profile.candidate_batch, candidate_batch_bytes),
        (profile.candidate_manifest, candidate_manifest_bytes),
    ] {
        source_bytes.insert(forge_core_contracts::RepoPath(repo_ref.to_owned()), bytes);
    }
    for (repo_ref, _, bytes) in &corpus_documents {
        source_bytes.insert(
            forge_core_contracts::RepoPath((*repo_ref).to_owned()),
            bytes.clone(),
        );
    }
    for (repo_ref, _, bytes) in &bundle_sources {
        source_bytes.insert(
            forge_core_contracts::RepoPath((*repo_ref).to_owned()),
            bytes.clone(),
        );
    }
    let mut referenced_paths = BTreeMap::<String, BTreeSet<String>>::new();
    let baseline_history = &review_subject
        .workflow_behavioral_review_subject
        .baseline_history;
    referenced_paths
        .entry(baseline_history.embedded_ref.0.clone())
        .or_default()
        .insert(baseline_history.expected_digest.clone());
    for (_, corpus, _) in &corpus_documents {
        for workflow in &corpus.workflow_behavioral_scenario_corpus.workflow_evidence {
            for source in &workflow.bindings.raw_sources {
                referenced_paths
                    .entry(source.embedded_ref.0.clone())
                    .or_default()
                    .insert(source.expected_digest.clone());
            }
            for scenario in &workflow.scenarios {
                if let WorkflowBehavioralScenarioExecution::Resume {
                    checkpoint_source, ..
                } = &scenario.execution
                {
                    referenced_paths
                        .entry(checkpoint_source.embedded_ref.0.clone())
                        .or_default()
                        .insert(checkpoint_source.expected_digest.clone());
                }
            }
        }
    }
    for (path, expected_digests) in referenced_paths {
        let path = forge_core_contracts::RepoPath(path);
        if !source_bytes.contains_key(&path) {
            match fs::read(root.join(&path.0)) {
                Ok(bytes) => {
                    source_bytes.insert(path.clone(), bytes);
                }
                Err(error) => {
                    behavioral_error(
                        &mut report,
                        &path.0,
                        format!("behavioral source bytes are missing: {error}"),
                    );
                    continue;
                }
            }
        }
        let found = behavior_sha256(&source_bytes[&path]);
        if expected_digests.len() != 1 || !expected_digests.contains(&found) {
            behavioral_error(
                &mut report,
                &path.0,
                format!(
                    "behavioral raw-source digest drift: expected {expected_digests:?}, found {found}"
                ),
            );
        }
    }

    let corpus_artifacts = corpus_set.workflow_behavioral_corpus_set.corpora.clone();
    let corpora = corpus_documents
        .into_iter()
        .filter_map(|(repo_ref, document, _)| {
            corpus_artifacts
                .iter()
                .find(|artifact| artifact.embedded_ref.0 == repo_ref)
                .cloned()
                .map(|artifact| WorkflowBehavioralCorpusInput { artifact, document })
        })
        .collect::<Vec<_>>();
    if corpora.len() != 2 {
        behavioral_error(
            &mut report,
            profile.corpus_set,
            "corpus set must bind exactly the two checked-in corpus partitions",
        );
    }

    let mut bundles_by_digest = BTreeMap::new();
    for (repo_ref, document, bytes) in bundle_sources {
        let artifact = WorkflowBehavioralArtifactReference {
            id: document.workflow_governance_bundle.id.clone(),
            embedded_ref: forge_core_contracts::RepoPath(repo_ref.to_owned()),
            expected_digest: behavior_sha256(&bytes),
        };
        let canonical = match workflow_runtime_bundle_digest(&document) {
            Ok(digest) => digest,
            Err(error) => {
                behavioral_error(&mut report, repo_ref, error);
                continue;
            }
        };
        if bundles_by_digest
            .insert(
                canonical,
                WorkflowBehavioralBundleInput { artifact, document },
            )
            .is_some()
        {
            behavioral_error(&mut report, repo_ref, "duplicate canonical bundle digest");
        }
    }

    let coverage_artifact = WorkflowBehavioralArtifactReference {
        id: coverage.workflow_behavioral_coverage_policy.id.clone(),
        embedded_ref: forge_core_contracts::RepoPath(profile.coverage.to_owned()),
        expected_digest: behavior_sha256(&coverage_bytes),
    };
    let corpus_set_artifact = WorkflowBehavioralArtifactReference {
        id: corpus_set.workflow_behavioral_corpus_set.id.clone(),
        embedded_ref: forge_core_contracts::RepoPath(profile.corpus_set.to_owned()),
        expected_digest: behavior_sha256(&corpus_set_bytes),
    };
    let identity = WorkflowBehavioralReportIdentity {
        report_id: forge_core_contracts::StableId(profile.report_id.to_owned()),
        report_version: WORKFLOW_BEHAVIOR_REPORT_VERSION.to_owned(),
        corpus_set: corpus_set_artifact,
        coverage_policy: coverage_artifact,
    };
    let derived = evaluate_workflow_behavior(
        &identity,
        &coverage,
        &corpus_set,
        &review_subject,
        &corpora,
        &bundles_by_digest,
        &source_bytes,
    );
    if derived != checked_report {
        behavioral_error(
            &mut report,
            profile.report,
            "checked-in shadow report does not exactly equal deterministic recomputation",
        );
    }
    validate_behavioral_report_baseline(profile, &derived, &mut report);
    validate_behavioral_candidate_release(
        profile,
        root,
        &candidate_manifest,
        &candidate_batch,
        &mut report,
    );
    validate_candidate_absent_from_admission(profile, root, &review_subject, &mut report);
    report
}

const WORKFLOW_RELEASE_INDEPENDENT_ADMISSION_SPEC_REF: &str =
    "contracts/spec/workflow-governance-independent-admission-v0.yaml";
const WORKFLOW_RELEASE_REVIEW_INDEX_REF: &str =
    "contracts/migration/workflow-core-assurance-review-index-v0.yaml";
const WORKFLOW_RELEASE_REVIEWER_REGISTRY_REF: &str =
    "contracts/policies/workflow-release-reviewer-registry-v0.yaml";
const WORKFLOW_RELEASE_AUTHORIZATION_REF: &str =
    "contracts/migration/workflow-core-assurance-admission-authorization-v0.yaml";
const WORKFLOW_RELEASE_REVIEWED_REGISTRY_REF: &str =
    "contracts/migration/workflow-governance-release-registry-core-assurance-v0.yaml";
const WORKFLOW_RELEASE_REVIEWED_BUNDLE_REF: &str =
    "contracts/workflow-governance/runtime-core-assurance-v0.yaml";
const WORKFLOW_RELEASE_INDEPENDENT_REVIEW_REF: &str =
    "contracts/evidence/workflow-core-assurance-independent-review-v0.yaml";

fn validate_workflow_release_independent_admission(root: &Path, summary: &mut ValidateSummary) {
    let mut report = ValidationReport::new();
    let required_refs = [
        WORKFLOW_RELEASE_INDEPENDENT_ADMISSION_SPEC_REF,
        WORKFLOW_RELEASE_REVIEW_INDEX_REF,
        WORKFLOW_RELEASE_REVIEWER_REGISTRY_REF,
        WORKFLOW_RELEASE_AUTHORIZATION_REF,
        WORKFLOW_RELEASE_REVIEWED_REGISTRY_REF,
        WORKFLOW_RELEASE_REVIEWED_BUNDLE_REF,
        WORKFLOW_RELEASE_INDEPENDENT_REVIEW_REF,
    ];
    for reference in required_refs {
        let disk = match fs::read(root.join(reference)) {
            Ok(bytes) => bytes,
            Err(error) => {
                behavioral_error(
                    &mut report,
                    reference,
                    format!("P5d.4a fixed artifact is missing: {error}"),
                );
                continue;
            }
        };
        if let Some(embedded) = forge_core_decisions::embedded_text(reference) {
            if disk != embedded.as_bytes() {
                behavioral_error(
                    &mut report,
                    reference,
                    "P5d.4a repository artifact differs from the fixed embedded bytes",
                );
            }
        } else {
            behavioral_error(
                &mut report,
                reference,
                "P5d.4a artifact is absent from the fixed embedded contract tree",
            );
        }
    }

    if let Some(index) = read_canonical_release_yaml::<WorkflowReleaseReviewIndexDocument>(
        root,
        WORKFLOW_RELEASE_REVIEW_INDEX_REF,
        &mut report,
    ) {
        for issue in index.validate() {
            behavioral_error(
                &mut report,
                WORKFLOW_RELEASE_REVIEW_INDEX_REF,
                format!("{}: {}", issue.path, issue.message),
            );
        }
    }
    if let Some(registry) = read_canonical_release_yaml::<WorkflowReleaseReviewerRegistryDocument>(
        root,
        WORKFLOW_RELEASE_REVIEWER_REGISTRY_REF,
        &mut report,
    ) {
        for issue in registry.validate() {
            behavioral_error(
                &mut report,
                WORKFLOW_RELEASE_REVIEWER_REGISTRY_REF,
                format!("{}: {}", issue.path, issue.message),
            );
        }
    }
    if let Some(authorization) = read_canonical_release_yaml::<
        WorkflowReleaseAdmissionAuthorizationDocument,
    >(root, WORKFLOW_RELEASE_AUTHORIZATION_REF, &mut report)
    {
        for issue in authorization.validate() {
            behavioral_error(
                &mut report,
                WORKFLOW_RELEASE_AUTHORIZATION_REF,
                format!("{}: {}", issue.path, issue.message),
            );
        }
    }

    if let (Some(registry), Some(bundle)) = (
        read_canonical_release_yaml::<WorkflowGovernanceReleaseRegistryDocument>(
            root,
            WORKFLOW_RELEASE_REVIEWED_REGISTRY_REF,
            &mut report,
        ),
        read_canonical_release_yaml::<WorkflowGovernanceBundleDocument>(
            root,
            WORKFLOW_RELEASE_REVIEWED_BUNDLE_REF,
            &mut report,
        ),
    ) {
        if registry.workflow_governance_release_registry.releases.len() != 3
            || bundle.workflow_governance_bundle.policies.len() != 20
        {
            behavioral_error(
                &mut report,
                WORKFLOW_RELEASE_REVIEWED_REGISTRY_REF,
                "frozen P5d.4a admission must remain exactly 3 releases and 20 policies",
            );
        }
    }
    summary.add_report("workflow_release_independent_admission", report);
}

const WORKFLOW_RELEASE_V2_REVIEW_INDEX_REF: &str =
    "contracts/migration/workflow-assurance-operations-review-index-v0.yaml";
const WORKFLOW_RELEASE_V2_REVIEWER_REGISTRY_REF: &str =
    "contracts/policies/workflow-release-reviewer-registry-assurance-operations-v0.yaml";
const WORKFLOW_RELEASE_V2_AUTHORIZATION_REF: &str =
    "contracts/migration/workflow-assurance-operations-admission-authorization-v0.yaml";
const WORKFLOW_RELEASE_V2_REGISTRY_REF: &str =
    "contracts/migration/workflow-governance-release-registry-assurance-operations-v0.yaml";
const WORKFLOW_RELEASE_V2_BUNDLE_REF: &str =
    "contracts/workflow-governance/runtime-assurance-operations-v0.yaml";
const WORKFLOW_RELEASE_V2_REVIEW_REF: &str =
    "contracts/evidence/workflow-assurance-operations-independent-review-v0.yaml";

struct WorkflowReleaseV2ValidationProfile {
    review_index: &'static str,
    reviewer_registry: &'static str,
    authorization: &'static str,
    registry: &'static str,
    bundle: &'static str,
    independent_review: &'static str,
    check_name: &'static str,
    release_count: usize,
    policy_count: usize,
    latest_release_id: &'static str,
    validate_trusted_tail: bool,
}

const ASSURANCE_OPERATIONS_V2_PROFILE: WorkflowReleaseV2ValidationProfile =
    WorkflowReleaseV2ValidationProfile {
        review_index: WORKFLOW_RELEASE_V2_REVIEW_INDEX_REF,
        reviewer_registry: WORKFLOW_RELEASE_V2_REVIEWER_REGISTRY_REF,
        authorization: WORKFLOW_RELEASE_V2_AUTHORIZATION_REF,
        registry: WORKFLOW_RELEASE_V2_REGISTRY_REF,
        bundle: WORKFLOW_RELEASE_V2_BUNDLE_REF,
        independent_review: WORKFLOW_RELEASE_V2_REVIEW_REF,
        check_name: "workflow_release_v2_admission",
        release_count: 4,
        policy_count: 33,
        latest_release_id: "workflow-governance.release.assurance-operations-v0",
        validate_trusted_tail: false,
    };

const AGENT_NATIVE_CONTINUITY_V2_PROFILE: WorkflowReleaseV2ValidationProfile =
    WorkflowReleaseV2ValidationProfile {
        review_index: "contracts/migration/workflow-agent-native-continuity-review-index-v0.yaml",
        reviewer_registry: "contracts/policies/workflow-release-reviewer-registry-agent-native-continuity-v0.yaml",
        authorization: "contracts/migration/workflow-agent-native-continuity-admission-authorization-v0.yaml",
        registry: "contracts/migration/workflow-governance-release-registry-agent-native-continuity-v0.yaml",
        bundle: "contracts/workflow-governance/runtime-agent-native-continuity-v0.yaml",
        independent_review: "contracts/evidence/workflow-agent-native-continuity-independent-review-v0.yaml",
        check_name: "workflow_release_v2_admission_agent_native_continuity",
        release_count: 5,
        policy_count: 42,
        latest_release_id: "workflow-governance.release.agent-native-continuity-v0",
        validate_trusted_tail: true,
    };

fn validate_workflow_release_v2_admission(root: &Path, summary: &mut ValidateSummary) {
    validate_workflow_release_v2_admission_profile(root, summary, &ASSURANCE_OPERATIONS_V2_PROFILE);
    validate_workflow_release_v2_admission_profile(
        root,
        summary,
        &AGENT_NATIVE_CONTINUITY_V2_PROFILE,
    );
}

fn validate_workflow_release_v2_admission_profile(
    root: &Path,
    summary: &mut ValidateSummary,
    profile: &WorkflowReleaseV2ValidationProfile,
) {
    let mut report = ValidationReport::new();
    for reference in [
        profile.review_index,
        profile.reviewer_registry,
        profile.authorization,
        profile.registry,
        profile.bundle,
        profile.independent_review,
    ] {
        let disk = match fs::read(root.join(reference)) {
            Ok(bytes) => bytes,
            Err(error) => {
                behavioral_error(
                    &mut report,
                    reference,
                    format!("P5d.4b fixed artifact is missing: {error}"),
                );
                continue;
            }
        };
        match forge_core_decisions::embedded_text(reference) {
            Some(embedded) if disk == embedded.as_bytes() => {}
            Some(_) => behavioral_error(
                &mut report,
                reference,
                "P5d.4b repository artifact differs from fixed embedded bytes",
            ),
            None => behavioral_error(
                &mut report,
                reference,
                "P5d.4b artifact is absent from the fixed embedded contract tree",
            ),
        }
    }
    if let Some(index) = read_canonical_release_yaml::<WorkflowReleaseReviewIndexV2Document>(
        root,
        profile.review_index,
        &mut report,
    ) {
        for issue in index.validate() {
            behavioral_error(
                &mut report,
                profile.review_index,
                format!("{}: {}", issue.path, issue.message),
            );
        }
    }
    if let Some(registry) = read_canonical_release_yaml::<WorkflowReleaseReviewerRegistryDocument>(
        root,
        profile.reviewer_registry,
        &mut report,
    ) {
        for issue in registry.validate() {
            behavioral_error(
                &mut report,
                profile.reviewer_registry,
                format!("{}: {}", issue.path, issue.message),
            );
        }
    }
    if let Some(authorization) = read_canonical_release_yaml::<
        WorkflowReleaseAdmissionAuthorizationV2Document,
    >(root, profile.authorization, &mut report)
    {
        for issue in authorization.validate() {
            behavioral_error(
                &mut report,
                profile.authorization,
                format!("{}: {}", issue.path, issue.message),
            );
        }
    }
    if let (Some(registry), Some(bundle)) = (
        read_canonical_release_yaml::<WorkflowGovernanceReleaseRegistryDocument>(
            root,
            profile.registry,
            &mut report,
        ),
        read_canonical_release_yaml::<WorkflowGovernanceBundleDocument>(
            root,
            profile.bundle,
            &mut report,
        ),
    ) {
        let latest_id = registry
            .workflow_governance_release_registry
            .releases
            .last()
            .map(|entry| entry.release.release_id.0.as_str());
        if registry.workflow_governance_release_registry.releases.len() != profile.release_count
            || bundle.workflow_governance_bundle.policies.len() != profile.policy_count
            || latest_id != Some(profile.latest_release_id)
        {
            behavioral_error(
                &mut report,
                profile.registry,
                "fixed reviewed registry or promoted bundle does not match its release profile",
            );
        }
    }
    if profile.validate_trusted_tail {
        match forge_core_kernel::load_admitted_workflow_governance_reviewed_release_registry() {
            Ok(registry) => {
                let latest = registry.latest_release();
                if registry.release_count() != profile.release_count
                    || latest.policy_count() != profile.policy_count
                    || latest.release().release_id.0 != profile.latest_release_id
                    || latest.receipt_carryover()
                        != forge_core_contracts::WorkflowReceiptCarryover::InvalidateAll
                    || ["edge-case-review", "track-decision", "release-readiness"]
                        .iter()
                        .any(|workflow| latest.contains_workflow_policy(workflow))
                {
                    behavioral_error(
                    &mut report,
                    profile.registry,
                    format!(
                        "trusted loader must derive exactly {} releases, {} policies, latest {}, invalidate_all, and zero quarantined policies",
                        profile.release_count, profile.policy_count, profile.latest_release_id
                    ),
                );
                }
            }
            Err(error) => behavioral_error(
                &mut report,
                profile.authorization,
                format!("P5d.4b trusted admission failed: {error:?}"),
            ),
        }
    }
    summary.add_report(profile.check_name, report);
}

fn validate_behavioral_report_baseline(
    profile: &BehavioralValidationProfile,
    document: &WorkflowBehavioralShadowReportDocument,
    report: &mut ValidationReport,
) {
    let shadow = &document.workflow_behavioral_shadow_report;
    if shadow.verdict != WorkflowBehavioralVerdict::BehaviorallyConsistentCandidate
        || shadow.disposition != WorkflowBehavioralDisposition::ReviewCandidate
    {
        behavioral_error(
            report,
            profile.report,
            "derived behavioral evidence must remain behaviorally_consistent_candidate/review_candidate without gaining authority",
        );
    }
    if shadow.workflow_reports.len() != profile.workflow_count {
        behavioral_error(
            report,
            profile.report,
            format!(
                "behavioral report must cover exactly {} workflows, found {}",
                profile.workflow_count,
                shadow.workflow_reports.len(),
            ),
        );
    }
    for workflow in &shadow.workflow_reports {
        let complete_kinds = workflow.scenario_kind_counts.len() == 7
            && workflow
                .scenario_kind_counts
                .iter()
                .all(|count| count.count == 1);
        if workflow.total_scenarios != 7
            || workflow.representative_scenarios != 2
            || workflow.adversarial_scenarios != 5
            || workflow.coverage_basis_points != 10_000
            || workflow.mismatch_count != 0
            || workflow.evaluation_error_count != 0
            || !complete_kinds
        {
            behavioral_error(
                report,
                format!(
                    "{}.{}",
                    profile.report, workflow.bindings.workflow_id.0
                ),
                "workflow must retain seven kinds, representative=2, adversarial=5, full coverage, zero mismatches, and zero errors",
            );
        }
    }
}

fn validate_behavioral_candidate_composition(
    profile: &BehavioralValidationProfile,
    overlay: &WorkflowGovernancePolicyOverlayDocument,
    review: &WorkflowBehavioralReviewSubjectDocument,
    candidate_bundle: &WorkflowGovernanceBundleDocument,
    batch: &WorkflowMigrationBatchDocument,
    manifest: &WorkflowGovernanceReleaseManifestDocument,
    report: &mut ValidationReport,
) {
    let subject = &review.workflow_behavioral_review_subject;
    let candidate_ids = subject
        .candidate_workflows
        .iter()
        .map(|candidate| &candidate.workflow_id)
        .collect::<BTreeSet<_>>();
    let quarantine_ids = subject
        .quarantines
        .iter()
        .map(|quarantine| &quarantine.workflow_id)
        .collect::<BTreeSet<_>>();
    if candidate_ids.len() != profile.workflow_count || quarantine_ids.len() != 3 {
        behavioral_error(
            report,
            profile.review_subject,
            format!(
                "review subject must contain exactly {} candidates and three quarantines",
                profile.workflow_count
            ),
        );
    }
    let overlay_policies = &overlay.workflow_governance_policy_overlay.policies;
    if overlay_policies.len() != profile.workflow_count
        || overlay_policies.iter().any(|policy| {
            !candidate_ids.contains(&policy.compatibility_workflow_id)
                || quarantine_ids.contains(&policy.compatibility_workflow_id)
        })
    {
        behavioral_error(
            report,
            profile.overlay,
            "overlay must contain only the reviewed candidate policies",
        );
    }
    let bundle_policies = &candidate_bundle.workflow_governance_bundle.policies;
    let policy_ids = bundle_policies
        .iter()
        .map(|policy| &policy.id)
        .collect::<BTreeSet<_>>();
    if bundle_policies.len() != profile.bundle_policy_count
        || bundle_policies
            .iter()
            .any(|policy| quarantine_ids.contains(&policy.compatibility_workflow_id))
        || bundle_policies.iter().any(|policy| {
            policy
                .prerequisites
                .iter()
                .any(|prerequisite| !policy_ids.contains(&prerequisite.policy_ref))
        })
    {
        behavioral_error(
            report,
            profile.candidate_bundle,
            format!(
                "candidate bundle must contain {} closed policies with no quarantine policy/prerequisite leak",
                profile.bundle_policy_count
            ),
        );
    }
    let candidate_batch = &batch.workflow_migration_batch;
    if candidate_batch.workflow_bindings.len() != profile.workflow_count
        || candidate_batch.policies.len() != profile.workflow_count
        || candidate_batch
            .workflow_bindings
            .iter()
            .any(|binding| quarantine_ids.contains(&binding.workflow_id))
    {
        behavioral_error(
            report,
            profile.candidate_batch,
            "candidate batch must bind exactly the reviewed policies and no quarantine",
        );
    }
    let manifest_quarantines = manifest
        .workflow_governance_release_manifest
        .workflow_entries
        .iter()
        .filter_map(|entry| {
            matches!(
                entry.disposition_intent,
                WorkflowReleaseDispositionIntent::Quarantined { .. }
            )
            .then_some(&entry.workflow_id)
        })
        .collect::<BTreeSet<_>>();
    if manifest_quarantines != quarantine_ids {
        behavioral_error(
            report,
            profile.candidate_manifest,
            "manifest quarantine set must exactly equal the review subject and cannot satisfy routing/readiness/completion",
        );
    }
}

fn validate_behavioral_candidate_release(
    profile: &BehavioralValidationProfile,
    root: &Path,
    manifest: &WorkflowGovernanceReleaseManifestDocument,
    candidate_batch: &WorkflowMigrationBatchDocument,
    report: &mut ValidationReport,
) {
    let counts = manifest
        .workflow_governance_release_manifest
        .workflow_entries
        .iter()
        .fold([0_usize; 5], |mut counts, entry| {
            let index = match entry.disposition_intent {
                WorkflowReleaseDispositionIntent::MigrationCandidate { .. } => 0,
                WorkflowReleaseDispositionIntent::CompatibilityOnly { .. } => 1,
                WorkflowReleaseDispositionIntent::Quarantined { .. } => 2,
                WorkflowReleaseDispositionIntent::DomainPackCandidate { .. } => 3,
                WorkflowReleaseDispositionIntent::RetirementCandidate { .. } => 4,
            };
            counts[index] += 1;
            counts
        });
    if counts != profile.disposition_counts {
        behavioral_error(
            report,
            profile.candidate_manifest,
            format!(
                "candidate disposition counts must be {:?}=110, found {counts:?}",
                profile.disposition_counts
            ),
        );
    }
    let mut batches = Vec::new();
    for batch_ref in profile.predecessor_batches {
        let Some(batch) =
            read_release_yaml::<WorkflowMigrationBatchDocument>(root, batch_ref, report)
        else {
            return;
        };
        batches.push(batch);
    }
    batches.push(candidate_batch.clone());
    let Some(plan) = read_release_yaml::<WorkflowMigrationPlanDocument>(
        root,
        WORKFLOW_MIGRATION_PLAN_REF,
        report,
    ) else {
        return;
    };
    let workflows = load_workflow_documents(&root.join("contracts/workflows"));
    let catalog = load_catalog(&root.join("contracts/workflows"));
    if !workflows.is_clean() || !catalog.is_clean() {
        behavioral_error(
            report,
            "contracts/workflows",
            "candidate release requires a clean legacy workflow catalog",
        );
        return;
    }
    let migration = evaluate_workflow_migration(&plan, &workflows.workflows, &catalog.catalog);
    let evaluation =
        evaluate_workflow_release(manifest, &batches, &migration, &workflows.workflows);
    for issue in evaluation.issues {
        behavioral_error(
            report,
            issue.path,
            format!("candidate release {:?}: {}", issue.code, issue.message),
        );
    }
    if evaluation.authority != WorkflowReleaseEvaluationAuthority::CandidateOnly
        || evaluation.status != WorkflowReleaseEvaluationStatus::StructurallyValid
    {
        behavioral_error(
            report,
            profile.candidate_manifest,
            "candidate release must be structurally valid while remaining candidate_only",
        );
    }
}

fn validate_candidate_absent_from_admission(
    profile: &BehavioralValidationProfile,
    root: &Path,
    review: &WorkflowBehavioralReviewSubjectDocument,
    report: &mut ValidationReport,
) {
    let Some(registry) = read_release_yaml::<WorkflowGovernanceReleaseRegistryDocument>(
        root,
        profile.admitted_registry,
        report,
    ) else {
        return;
    };
    let subject = &review.workflow_behavioral_review_subject;
    if registry
        .workflow_governance_release_registry
        .releases
        .iter()
        .any(|entry| {
            entry.release.release_id == subject.proposed_release.release_id
                || entry.runtime_bundle.identity.bundle_id == subject.runtime_bundle.bundle_id
        })
    {
        behavioral_error(
            report,
            profile.admitted_registry,
            "behavioral candidate must remain absent from its predecessor admitted registry",
        );
    }
}

fn read_behavioral_yaml<T: serde::de::DeserializeOwned>(
    root: &Path,
    repo_ref: &str,
    report: &mut ValidationReport,
) -> Option<(T, Vec<u8>)> {
    let bytes = match fs::read(root.join(repo_ref)) {
        Ok(bytes) => bytes,
        Err(error) => {
            report.push(Diagnostic::error(
                DiagnosticCode::ReadFileFailed,
                repo_ref,
                error.to_string(),
            ));
            return None;
        }
    };
    match yaml_serde::from_slice(&bytes) {
        Ok(document) => Some((document, bytes)),
        Err(error) => {
            report.push(Diagnostic::error(
                DiagnosticCode::ParseYamlFailed,
                repo_ref,
                error.to_string(),
            ));
            None
        }
    }
}

fn behavior_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

fn push_behavioral_contract_issues(
    report: &mut ValidationReport,
    repo_ref: &str,
    issues: Vec<forge_core_contracts::WorkflowBehavioralContractIssue>,
) {
    for issue in issues {
        behavioral_error(report, format!("{repo_ref}.{}", issue.path), issue.message);
    }
}

fn behavioral_error(
    report: &mut ValidationReport,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    report.push(Diagnostic::error(
        DiagnosticCode::WorkflowGovernanceInvalid,
        path,
        message,
    ));
}

fn read_canonical_release_yaml<T>(
    root: &Path,
    repo_ref: &str,
    report: &mut ValidationReport,
) -> Option<T>
where
    T: serde::de::DeserializeOwned + PartialEq,
{
    let disk = read_release_yaml::<T>(root, repo_ref, report)?;
    let Some(embedded_text) = forge_core_decisions::embedded_text(repo_ref) else {
        report.push(Diagnostic::error(
            DiagnosticCode::MissingReference,
            repo_ref,
            "canonical release artifact is not embedded in this Forge binary",
        ));
        return None;
    };
    let embedded = match yaml_serde::from_str::<T>(embedded_text) {
        Ok(value) => value,
        Err(error) => {
            report.push(Diagnostic::error(
                DiagnosticCode::ParseYamlFailed,
                repo_ref,
                format!("embedded canonical release YAML is invalid: {error}"),
            ));
            return None;
        }
    };
    if disk != embedded {
        report.push(Diagnostic::error(
            DiagnosticCode::WorkflowGovernanceInvalid,
            repo_ref,
            "checkout YAML does not match the canonical artifact embedded in this Forge binary",
        ));
        return None;
    }
    Some(disk)
}

fn read_release_yaml<T: serde::de::DeserializeOwned>(
    root: &Path,
    repo_ref: &str,
    report: &mut ValidationReport,
) -> Option<T> {
    let path = root.join(repo_ref);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            report.push(Diagnostic::error(
                DiagnosticCode::ReadFileFailed,
                repo_ref,
                error.to_string(),
            ));
            return None;
        }
    };
    match yaml_serde::from_str(&text) {
        Ok(value) => Some(value),
        Err(error) => {
            report.push(Diagnostic::error(
                DiagnosticCode::ParseYamlFailed,
                repo_ref,
                error.to_string(),
            ));
            None
        }
    }
}

fn is_safe_migration_repo_ref(repo_ref: &str) -> bool {
    let path = Path::new(repo_ref);
    !path.is_absolute()
        && repo_ref.starts_with("contracts/migration/")
        && path.extension().and_then(|value| value.to_str()) == Some("yaml")
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn validate_operation_fixtures(root: &Path, index: &ReferenceIndex, summary: &mut ValidateSummary) {
    let dir = root.join("docs/fixtures/operation-contract-v0");
    for path in yaml_files(&dir, summary) {
        if let Some(operation) = read_yaml::<OperationContractDocument>(&path, summary) {
            summary.add_report(
                &format!("operation_contract:{}", repo_relative(root, &path)),
                validate_operation(&operation),
            );
            summary.add_report(
                &format!("operation_refs:{}", repo_relative(root, &path)),
                validate_operation_cross_references(&operation, index),
            );
        }
    }
}

fn validate_side_contracts(root: &Path, index: &ReferenceIndex, summary: &mut ValidateSummary) {
    validate_named_dir_instances::<ClaimContractDocument, _>(
        root,
        "contracts/claims",
        "claim-contract-v0.yaml",
        "claim_contract",
        summary,
        validate_claim,
    );
    validate_cross_ref_instances::<ClaimContractDocument, _>(
        root,
        "contracts/claims",
        "claim-contract-v0.yaml",
        "claim_refs",
        summary,
        index,
        validate_claim_cross_references,
    );
    validate_named_dir_instances::<CompletionContractDocument, _>(
        root,
        "contracts/completion",
        "completion-contract-v0.yaml",
        "completion_contract",
        summary,
        validate_completion,
    );
    validate_cross_ref_instances::<CompletionContractDocument, _>(
        root,
        "contracts/completion",
        "completion-contract-v0.yaml",
        "completion_refs",
        summary,
        index,
        validate_completion_cross_references,
    );
    validate_named_dir_instances::<GateContractDocument, _>(
        root,
        "contracts/gates",
        "gate-contract-v0.yaml",
        "gate_contract",
        summary,
        validate_gate,
    );
    validate_cross_ref_instances::<GateContractDocument, _>(
        root,
        "contracts/gates",
        "gate-contract-v0.yaml",
        "gate_refs",
        summary,
        index,
        validate_gate_cross_references,
    );
    validate_named_dir_instances::<RequestContractDocument, _>(
        root,
        "contracts/requests",
        "request-contract-v0.yaml",
        "request_contract",
        summary,
        validate_request,
    );
    validate_cross_ref_instances::<RequestContractDocument, _>(
        root,
        "contracts/requests",
        "request-contract-v0.yaml",
        "request_refs",
        summary,
        index,
        validate_request_cross_references,
    );
    validate_named_dir_instances::<ToolEffectContractDocument, _>(
        root,
        "contracts/effects",
        "tool-effect-contract-v0.yaml",
        "tool_effect_contract",
        summary,
        validate_tool_effect,
    );
    validate_cross_ref_instances::<ToolEffectContractDocument, _>(
        root,
        "contracts/effects",
        "tool-effect-contract-v0.yaml",
        "tool_effect_refs",
        summary,
        index,
        validate_tool_effect_cross_references,
    );
    validate_named_dir_instances::<DecisionCloseContractDocument, _>(
        root,
        "contracts/decisions",
        "decision-close-contract-v0.yaml",
        "decision_close_contract",
        summary,
        validate_decision_close,
    );
    validate_cross_ref_instances::<DecisionCloseContractDocument, _>(
        root,
        "contracts/decisions",
        "decision-close-contract-v0.yaml",
        "decision_close_refs",
        summary,
        index,
        validate_decision_close_cross_references,
    );
    validate_named_dir_instances::<HealthRecoveryContractDocument, _>(
        root,
        "contracts/recovery",
        "health-recovery-contract-v0.yaml",
        "health_recovery_contract",
        summary,
        validate_health_recovery,
    );
    validate_cross_ref_instances::<HealthRecoveryContractDocument, _>(
        root,
        "contracts/recovery",
        "health-recovery-contract-v0.yaml",
        "health_recovery_refs",
        summary,
        index,
        validate_health_recovery_cross_references,
    );
    validate_named_dir_instances::<CoordinationEvalContractDocument, _>(
        root,
        "contracts/evals",
        "coordination-eval-contract-v0.yaml",
        "coordination_eval_contract",
        summary,
        validate_coordination_eval,
    );
    validate_cross_ref_instances::<CoordinationEvalContractDocument, _>(
        root,
        "contracts/evals",
        "coordination-eval-contract-v0.yaml",
        "coordination_eval_refs",
        summary,
        index,
        validate_coordination_eval_cross_references,
    );
}

fn validate_runtime_contracts(root: &Path, index: &ReferenceIndex, summary: &mut ValidateSummary) {
    validate_named::<RuntimeHandoffContractDocument, _>(
        root,
        "contracts/runtimes/cursor-browser-validation-runtime.yaml",
        "runtime_handoff_contract",
        summary,
        validate_runtime_handoff,
    );
    validate_named::<RuntimeHandoffContractDocument, _>(
        root,
        "contracts/runtimes/cursor-browser-validation-missing-capability.yaml",
        "runtime_handoff_contract",
        summary,
        validate_runtime_handoff,
    );
    validate_named_cross::<RuntimeHandoffContractDocument, _>(
        root,
        "contracts/runtimes/cursor-browser-validation-runtime.yaml",
        "runtime_handoff_refs",
        summary,
        index,
        validate_runtime_handoff_cross_references,
    );
    validate_named_cross::<RuntimeHandoffContractDocument, _>(
        root,
        "contracts/runtimes/cursor-browser-validation-missing-capability.yaml",
        "runtime_handoff_refs",
        summary,
        index,
        validate_runtime_handoff_cross_references,
    );
    validate_named::<RuntimeRegistryEntryDocument, _>(
        root,
        "contracts/runtimes/registry-cursor-browser-agent.yaml",
        "runtime_registry_entry",
        summary,
        validate_runtime_registry_entry,
    );
    validate_named_cross::<RuntimeRegistryEntryDocument, _>(
        root,
        "contracts/runtimes/registry-cursor-browser-agent.yaml",
        "runtime_registry_refs",
        summary,
        index,
        validate_runtime_registry_cross_references,
    );
    validate_named::<RuntimeCapabilityDocument, _>(
        root,
        "contracts/runtimes/capability-browser-validation.yaml",
        "runtime_capability",
        summary,
        validate_runtime_capability,
    );
}

fn validate_named_dir_instances<T, F>(
    root: &Path,
    relative_dir: &str,
    definition_file: &str,
    check_prefix: &str,
    summary: &mut ValidateSummary,
    validate: F,
) where
    T: serde::de::DeserializeOwned,
    F: Fn(&T) -> ValidationReport,
{
    let dir = root.join(relative_dir);
    for path in yaml_files(&dir, summary) {
        if path.file_name().and_then(|value| value.to_str()) == Some(definition_file) {
            continue;
        }
        if let Some(contract) = read_yaml::<T>(&path, summary) {
            summary.add_report(
                &format!("{check_prefix}:{}", repo_relative(root, &path)),
                validate(&contract),
            );
        }
    }
}

fn validate_cross_ref_instances<T, F>(
    root: &Path,
    relative_dir: &str,
    definition_file: &str,
    check_prefix: &str,
    summary: &mut ValidateSummary,
    index: &ReferenceIndex,
    validate: F,
) where
    T: serde::de::DeserializeOwned,
    F: Fn(&T, &ReferenceIndex) -> ValidationReport,
{
    let dir = root.join(relative_dir);
    for path in yaml_files(&dir, summary) {
        if path.file_name().and_then(|value| value.to_str()) == Some(definition_file) {
            continue;
        }
        if let Some(contract) = read_yaml::<T>(&path, summary) {
            summary.add_report(
                &format!("{check_prefix}:{}", repo_relative(root, &path)),
                validate(&contract, index),
            );
        }
    }
}

fn validate_named<T, F>(
    root: &Path,
    relative_path: &str,
    check_name: &str,
    summary: &mut ValidateSummary,
    validate: F,
) where
    T: serde::de::DeserializeOwned,
    F: Fn(&T) -> ValidationReport,
{
    let path = root.join(relative_path);
    if let Some(contract) = read_yaml::<T>(&path, summary) {
        summary.add_report(
            &format!("{check_name}:{}", repo_relative(root, &path)),
            validate(&contract),
        );
    }
}

fn validate_named_cross<T, F>(
    root: &Path,
    relative_path: &str,
    check_name: &str,
    summary: &mut ValidateSummary,
    index: &ReferenceIndex,
    validate: F,
) where
    T: serde::de::DeserializeOwned,
    F: Fn(&T, &ReferenceIndex) -> ValidationReport,
{
    let path = root.join(relative_path);
    if let Some(contract) = read_yaml::<T>(&path, summary) {
        summary.add_report(
            &format!("{check_name}:{}", repo_relative(root, &path)),
            validate(&contract, index),
        );
    }
}

fn read_yaml<T: serde::de::DeserializeOwned>(
    path: &Path,
    summary: &mut ValidateSummary,
) -> Option<T> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            summary.push_diagnostic(Diagnostic::error(
                DiagnosticCode::ReadFileFailed,
                path.to_string_lossy(),
                err.to_string(),
            ));
            return None;
        }
    };
    match yaml_serde::from_str(&text) {
        Ok(value) => Some(value),
        Err(err) => {
            summary.push_diagnostic(Diagnostic::error(
                DiagnosticCode::ParseYamlFailed,
                path.to_string_lossy(),
                err.to_string(),
            ));
            None
        }
    }
}

fn yaml_files(dir: &Path, summary: &mut ValidateSummary) -> Vec<PathBuf> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            // A missing directory is not an error: it means there are no
            // instances to validate (common for a consumer repo that ships
            // no contracts/commands, contracts/claims, etc.). Any other IO
            // error (permission denied, …) is a real failure and surfaces.
            if err.kind() == std::io::ErrorKind::NotFound {
                return Vec::new();
            }
            summary.push_diagnostic(Diagnostic::error(
                DiagnosticCode::ReadDirFailed,
                dir.to_string_lossy(),
                err.to_string(),
            ));
            return Vec::new();
        }
    };
    let mut files = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) == Some("yaml") {
                    files.push(path);
                }
            }
            Err(err) => summary.push_diagnostic(Diagnostic::error(
                DiagnosticCode::ReadDirEntryFailed,
                dir.to_string_lossy(),
                err.to_string(),
            )),
        }
    }
    files.sort();
    files
}

fn repo_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
#[must_use]
fn validate_usage(command: &CommandSpec) -> String {
    command_surface_usage(command)
}

#[instrument(skip_all, fields(root = tracing::field::Empty, json = tracing::field::Empty, diagnostic_count = tracing::field::Empty), level = "info")]
/// Dispatch entrypoint for the `forge-core validate` command.
///
/// Loads the project at `--root` (default `.`) and prints the resulting
/// [`ValidateSummary`] as JSON (`--json`) or human-readable text.
///
/// # Errors
///
/// Returns `ExitError::usage` when an unknown flag is present or `--root`
/// is missing a value.
///
/// # Panics
///
/// Panics in JSON mode if the validation summary cannot be serialized. The
/// summary type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_validate_command(args: &[String]) -> Result<(), ExitError> {
    let command = &COMMAND_VALIDATE;
    let mut root = PathBuf::from(".");
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(ExitError::usage(validate_usage(command)));
                };
                root = PathBuf::from(value);
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", validate_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(validate_usage(command)));
            }
        }
        index += 1;
    }

    let span = tracing::Span::current();
    span.record("root", root.to_string_lossy().to_string().as_str());
    span.record("json", json);
    let summary = run_validate(&root);
    span.record("diagnostic_count", summary.diagnostics.len());
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&summary).expect("serialize validation summary")
        );
    } else {
        println!("{}", summary.human_summary());
        for diagnostic in &summary.diagnostics {
            // V2.B: diagnostics are now the typed `Diagnostic`. Print their
            // stable wire-format fields (snake_case `severity`/`code`) by
            // reading them back from the JSON serialization, so the
            // human-readable output matches the `--json` code identifiers
            // rather than the lossy `Debug` form.
            let wire = serde_json::to_value(diagnostic).expect("serialize diagnostic");
            let severity = wire
                .get("severity")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("error");
            let code = wire
                .get("code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            eprintln!(
                "{severity} {code} {}: {}",
                diagnostic.path, diagnostic.message
            );
        }
    }

    if !summary.passed() {
        return Err(ExitError::failed("validation reported errors"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn workflow_governance_issues_keep_typed_kind_in_canonical_diagnostic() {
        let report = workflow_governance_validation_report(vec![WorkflowGovernanceIssue {
            code: forge_core_decisions::WorkflowGovernanceIssueCode::DependencyCycle,
            path: "workflow_governance_bundle.policies".to_owned(),
            message: "cycle detected".to_owned(),
        }]);
        let diagnostic = &report.diagnostics()[0];
        assert_eq!(diagnostic.code, DiagnosticCode::WorkflowGovernanceInvalid);
        assert!(diagnostic.message.contains("DependencyCycle"));
    }

    #[test]
    fn validate_usage_projects_command_surface_line() {
        let usage = validate_usage(&COMMAND_VALIDATE);
        assert!(usage.starts_with("usage:\n"));
        for line in COMMAND_VALIDATE.usage_lines {
            let projected = format!("  {}", line.trim_start());
            assert!(
                usage.contains(&projected),
                "validate usage should include projected Command Surface line {projected:?}: {usage}"
            );
        }
    }

    #[test]
    fn explicit_no_json_is_accepted_by_validate_help_path() {
        run_validate_command(&args(&["validate", "--no-json", "--help"]))
            .expect("validate accepts explicit --no-json");
    }

    #[test]
    fn missing_root_reports_validate_usage() {
        let error = run_validate_command(&args(&["validate", "--root"]))
            .expect_err("missing root value must fail before validation");
        assert!(
            error.message().contains("forge-core validate"),
            "missing root should report validate usage: {error}"
        );
        assert!(
            !error.message().contains("forge-core execute-operation"),
            "validate usage must not include unrelated global commands: {error}"
        );
    }

    #[test]
    fn unknown_arg_reports_validate_usage() {
        let error = run_validate_command(&args(&["validate", "--frobnicate"]))
            .expect_err("unknown validate argument must fail before validation");
        assert!(
            error.message().contains("forge-core validate"),
            "unknown argument should report validate usage: {error}"
        );
        assert!(
            !error.message().contains("forge-core start"),
            "validate usage must not include unrelated global commands: {error}"
        );
    }
}
