//! `guide` command family — the agent-first method surface (slice 3).
//!
//! These commands are the PRIMARY consumer of host LLMs. Every command emits a
//! single [`CliEnvelope`] as JSON to stdout; diagnostics go to stderr.
//! Implements R1/R3/R4 from the slice-3 spec.

use crate::project_cmd::resolve_project;
use forge_core_command_surface::COMMAND_GUIDE;
use forge_core_contracts::{
    Catalog, CatalogEntry, CliEnvelope, ExitReason, Phase, RepoPath, WorkflowDocument,
    WorkflowGovernanceBundleDocument, WorkflowGovernanceEvaluationDocument,
    WorkflowGovernanceReleaseManifestDocument, WorkflowMigrationBatchDocument,
    WorkflowMigrationPlanDocument, WorkflowRetirementTombstone,
    WorkflowRetirementTombstoneCatalogDocument,
};
use forge_core_contracts::{
    FunnelAmbiguityPressure, FunnelAutomaticGate, FunnelContactDensity, FunnelLane,
    FunnelProceduralConfirmation, GuideProtocolDocument, OperationContractDocument,
};
use forge_core_decisions::{
    evaluate_funnel_phase, evaluate_workflow_migration, evaluate_workflow_release,
    load_accepted_funnel_autonomy_policy, load_catalog, load_embedded_catalog,
    load_workflow_documents, project_legacy_workflow_compatibility, simulate_workflow_governance,
    CatalogLoadReport, LegacyWorkflowGovernanceProjection, WorkflowDocumentLoadReport,
    WorkflowGovernanceSimulation, WorkflowMigrationAudit, WorkflowMigrationAuditStatus,
    WorkflowReleaseEvaluation, WorkflowReleaseEvaluationStatus,
};
use forge_core_decisions::{GateKind, ProvidedGateResult};
use forge_core_kernel::{
    load_admitted_workflow_retirement_checkpoint, validate_guide_protocol, GuideRoute,
};
use std::collections::BTreeSet;
use std::path::Path;

use crate::cli_error::ExitError;

const DEFAULT_WORKFLOW_MIGRATION_PLAN_REF: &str =
    "contracts/policies/workflow-migration-foundation-v0.yaml";

/// Version of the compact routing payload carried by `guide describe` and
/// `guide status`. This is intentionally independent of the CLI envelope
/// version: removing 42 routes changes the meaning of workflow counts and a
/// 0.1 host must fail closed instead of interpreting 68 as the old 110.
pub const GUIDE_ROUTING_PAYLOAD_SCHEMA_VERSION: &str = "0.2";

// ============================================================================
// guide describe — the compact routing surface (R3 token cliff, DD13).
// ============================================================================

/// One compact workflow row in `describe`. Deliberately small: id, phase tags,
/// one-line description derived from the first trigger. The host reads this
/// ONCE per session and never re-reads unless `schema_version` changes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DescribeWorkflow {
    pub id: String,
    pub phases: Vec<String>,
    pub summary: String,
}

/// One gate row in `describe` — the phase transitions that require it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DescribeGate {
    pub gate: String,
    pub required_for: Vec<String>,
}

/// The full `describe` payload.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DescribePayload {
    pub schema_version: String,
    pub phases: Vec<String>,
    pub workflows: Vec<DescribeWorkflow>,
    /// Non-routable compatibility identifiers retained only so an agent can
    /// recover from a stale workflow recommendation without guessing.
    pub retired_workflows: Vec<RetiredWorkflowDiagnostic>,
    pub gates: Vec<DescribeGate>,
    pub exit_reasons: Vec<String>,
}

impl DescribePayload {
    /// Build the describe payload from a loaded catalog + the static gate map.
    ///
    /// # Errors
    /// Fails closed when the embedded tombstone catalog cannot be parsed; a
    /// host must never mistake a retired id for an unknown routable id.
    pub fn from_catalog(catalog: &Catalog) -> Result<Self, String> {
        let tombstones = embedded_retirement_tombstones()?;
        let retired_ids = retirement_ids(tombstones);
        let workflows = catalog
            .entries
            .iter()
            .filter(|entry| !retired_ids.contains(entry.id.0.as_str()))
            .map(compact_workflow)
            .collect::<Vec<_>>();
        Ok(Self {
            schema_version: GUIDE_ROUTING_PAYLOAD_SCHEMA_VERSION.to_owned(),
            phases: Phase::ALL.iter().map(Phase::to_string).collect(),
            workflows,
            retired_workflows: retirement_rows(tombstones),
            gates: gate_table(),
            exit_reasons: vec![
                "ok".into(),
                "rejected_by_gate".into(),
                "invalid_decision_shape".into(),
                "conflict".into(),
                "env_config".into(),
            ],
        })
    }
}

/// Typed, non-authoritative compatibility diagnostic for one retired legacy
/// workflow. This is deliberately distinct from [`DescribeWorkflow`], so a
/// host cannot accidentally treat a tombstone as routable catalog content.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RetiredWorkflowDiagnostic {
    pub workflow_id: String,
    pub diagnostic_code: String,
    pub replacement_policy_ref: String,
    pub replacement_release_id: String,
    pub replacement_argv: Vec<String>,
}

fn embedded_retirement_tombstones(
) -> Result<&'static WorkflowRetirementTombstoneCatalogDocument, String> {
    load_admitted_workflow_retirement_checkpoint()
        .map(forge_core_kernel::AdmittedWorkflowRetirementCheckpoint::tombstones)
        .map_err(|error| format!("verified retirement checkpoint is unavailable: {error}"))
}

fn retirement_rows(
    catalog: &WorkflowRetirementTombstoneCatalogDocument,
) -> Vec<RetiredWorkflowDiagnostic> {
    catalog
        .workflow_retirement_tombstone_catalog
        .tombstones
        .iter()
        .map(retirement_row)
        .collect()
}

fn retirement_ids(catalog: &WorkflowRetirementTombstoneCatalogDocument) -> BTreeSet<&str> {
    catalog
        .workflow_retirement_tombstone_catalog
        .tombstones
        .iter()
        .map(|tombstone| tombstone.workflow_id.0.as_str())
        .collect()
}

fn retirement_row(tombstone: &WorkflowRetirementTombstone) -> RetiredWorkflowDiagnostic {
    RetiredWorkflowDiagnostic {
        workflow_id: tombstone.workflow_id.0.clone(),
        diagnostic_code: tombstone.diagnostic_code.0.clone(),
        replacement_policy_ref: tombstone.replacement_policy_ref.0.clone(),
        replacement_release_id: tombstone.replacement_release_id.0.clone(),
        replacement_argv: tombstone.replacement_argv.clone(),
    }
}

fn retired_workflow(workflow_id: &str) -> Result<Option<RetiredWorkflowDiagnostic>, String> {
    Ok(embedded_retirement_tombstones()?
        .workflow_retirement_tombstone_catalog
        .tombstones
        .iter()
        .find(|tombstone| tombstone.workflow_id.0 == workflow_id)
        .map(retirement_row))
}

/// Compress a catalog entry to the compact describe row.
fn compact_workflow(e: &CatalogEntry) -> DescribeWorkflow {
    // summary = first trigger (the matching predicate) is the most concise
    // intent signal available without loading the full workflow text.
    let summary = e
        .triggers
        .first()
        .cloned()
        .unwrap_or_else(|| format!("workflow {}", e.id.0));
    DescribeWorkflow {
        id: e.id.0.clone(),
        phases: e.phases.iter().map(|p| p.0.clone()).collect(),
        summary,
    }
}

/// The static map of which gate is required for which forward transition.
/// Kept in lockstep with forge-core-decisions::phase_transition::required_gate_for.
fn gate_table() -> Vec<DescribeGate> {
    vec![
        DescribeGate {
            gate: "grill-gate".into(),
            required_for: vec!["1-discovery -> 2-specification".into()],
        },
        DescribeGate {
            gate: "system-design".into(),
            required_for: vec!["2-specification -> 3-plan".into()],
        },
    ]
}

/// Run `guide describe`. Loads the catalog from `catalog_dir` and emits the
/// compact routing surface.
///
/// # Errors
/// Returns an error envelope (exit 5) if the catalog directory cannot be read
/// or any workflow file is malformed.
#[must_use]
/// Resolve the catalog source:
/// - `Some(dir)` → load that directory from disk (explicit `--catalog-dir`).
/// - `None` → fall through: if a local `contracts/workflows/` exists in the
///   current working directory, use it (brownfield/forge workspace); otherwise
///   load the catalog embedded in the binary (greenfield, zero-config).
///
/// A freshly installed binary carries only the current operational catalog,
/// so routing never falls back to the frozen legacy retirement subject.
fn resolve_catalog(catalog_dir: Option<&Path>) -> CatalogLoadReport {
    if let Some(dir) = catalog_dir {
        load_catalog(dir)
    } else {
        let local = Path::new("contracts/workflows");
        if local.is_dir() {
            load_catalog(local)
        } else {
            load_embedded_catalog()
        }
    }
}

const FROZEN_LEGACY_CATALOG_DIR: &str = "contracts/evidence/workflow-retirement/legacy-catalog";

/// Resolve the historical P5a audit subject without ever leaking it into the
/// live routing surface. An explicit directory remains caller-owned input;
/// default source is the frozen 110-document retirement evidence snapshot.
fn resolve_migration_catalog(catalog_dir: Option<&Path>) -> CatalogLoadReport {
    if let Some(dir) = catalog_dir {
        load_catalog(dir)
    } else {
        let local = Path::new(FROZEN_LEGACY_CATALOG_DIR);
        if local.is_dir() {
            load_catalog(local)
        } else {
            forge_core_decisions::catalog::load_embedded_frozen_legacy_catalog()
        }
    }
}

fn resolve_migration_workflow_documents(catalog_dir: Option<&Path>) -> WorkflowDocumentLoadReport {
    if let Some(dir) = catalog_dir {
        load_workflow_documents(dir)
    } else {
        let local = Path::new(FROZEN_LEGACY_CATALOG_DIR);
        if local.is_dir() {
            load_workflow_documents(local)
        } else {
            forge_core_decisions::catalog::load_embedded_frozen_legacy_workflow_documents()
        }
    }
}

#[must_use]
pub fn run_describe(catalog_dir: Option<&Path>) -> CliEnvelope<DescribePayload> {
    let report = resolve_catalog(catalog_dir);
    if !report.is_clean() {
        return CliEnvelope::err(
            "guide.describe",
            ExitReason::EnvConfig,
            format!("catalog load failed: {} error(s)", report.errors.len()),
        );
    }
    match DescribePayload::from_catalog(&report.catalog) {
        Ok(payload) => CliEnvelope::ok("guide.describe", payload),
        Err(error) => CliEnvelope::err("guide.describe", ExitReason::EnvConfig, error),
    }
}

// Re-export the load report type for callers that want the raw errors.
pub type DescribeReport = CatalogLoadReport;

/// Run the complete read-only P5a inventory, classification, target-link,
/// shadow-parity, and deletion-baseline audit.
#[must_use]
pub fn run_migration_audit(
    catalog_dir: Option<&Path>,
    plan_file: Option<&Path>,
) -> CliEnvelope<WorkflowMigrationAudit> {
    let workflows = resolve_migration_workflow_documents(catalog_dir);
    let catalog = resolve_migration_catalog(catalog_dir);
    if !workflows.is_clean() || !catalog.is_clean() {
        return CliEnvelope::err(
            "guide.migration-audit",
            ExitReason::EnvConfig,
            format!(
                "complete workflow inventory failed: {} workflow error(s), {} catalog error(s)",
                workflows.errors.len(),
                catalog.errors.len()
            ),
        );
    }
    let plan_text = match plan_file {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(error) => {
                return CliEnvelope::err(
                    "guide.migration-audit",
                    ExitReason::EnvConfig,
                    format!(
                        "cannot read workflow migration plan {}: {error}",
                        path.display()
                    ),
                );
            }
        },
        None => forge_core_decisions::read_contract_text(
            Path::new("."),
            DEFAULT_WORKFLOW_MIGRATION_PLAN_REF,
        ),
    };
    let Some(plan_text) = plan_text else {
        return CliEnvelope::err(
            "guide.migration-audit",
            ExitReason::EnvConfig,
            "workflow migration plan is unavailable",
        );
    };
    let plan: WorkflowMigrationPlanDocument = match yaml_serde::from_str(&plan_text) {
        Ok(plan) => plan,
        Err(error) => {
            return CliEnvelope::err(
                "guide.migration-audit",
                ExitReason::InvalidDecisionShape,
                format!("workflow migration plan is invalid: {error}"),
            );
        }
    };
    let audit = evaluate_workflow_migration(&plan, &workflows.workflows, &catalog.catalog);
    if audit.status == WorkflowMigrationAuditStatus::ReadyForShadow {
        CliEnvelope::ok("guide.migration-audit", audit)
    } else {
        CliEnvelope::reject(
            "guide.migration-audit",
            ExitReason::RejectedByGate,
            "workflow migration foundation is blocked; inspect typed issues and drift",
            audit,
        )
    }
}

/// Evaluate one complete P5d.1 rollout candidate without granting execution or
/// retirement authority. The typed evaluator always reports `candidate_only`.
#[must_use]
pub fn run_rollout_audit(
    manifest_file: &Path,
    batch_files: &[std::path::PathBuf],
    catalog_dir: Option<&Path>,
    plan_file: Option<&Path>,
) -> CliEnvelope<WorkflowReleaseEvaluation> {
    let manifest: WorkflowGovernanceReleaseManifestDocument =
        match read_closed_rollout_yaml(manifest_file, "workflow governance release manifest") {
            Ok(manifest) => manifest,
            Err((exit_reason, message)) => {
                return CliEnvelope::err("guide.rollout-audit", exit_reason, message);
            }
        };

    let mut batches = Vec::with_capacity(batch_files.len());
    for batch_file in batch_files {
        match read_closed_rollout_yaml::<WorkflowMigrationBatchDocument>(
            batch_file,
            "workflow migration batch",
        ) {
            Ok(batch) => batches.push(batch),
            Err((exit_reason, message)) => {
                return CliEnvelope::err("guide.rollout-audit", exit_reason, message);
            }
        }
    }

    // Reuse the P5a command path so catalog loading, plan fallback, typed plan
    // parsing, and audit derivation cannot drift between migration and rollout.
    let migration_envelope = run_migration_audit(catalog_dir, plan_file);
    let Some(migration_audit) = migration_envelope.data else {
        let exit = match migration_envelope.exit_reason.0.as_str() {
            "invalid_decision_shape" => ExitReason::InvalidDecisionShape,
            "rejected_by_gate" => ExitReason::RejectedByGate,
            _ => ExitReason::EnvConfig,
        };
        let message = migration_envelope.error.map_or_else(
            || "P5a workflow migration audit is unavailable".to_owned(),
            |error| error.message,
        );
        return CliEnvelope::err("guide.rollout-audit", exit, message);
    };

    let workflows = resolve_migration_workflow_documents(catalog_dir);
    if !workflows.is_clean() {
        return CliEnvelope::err(
            "guide.rollout-audit",
            ExitReason::EnvConfig,
            format!(
                "complete workflow inventory failed: {} workflow error(s)",
                workflows.errors.len()
            ),
        );
    }

    let evaluation =
        evaluate_workflow_release(&manifest, &batches, &migration_audit, &workflows.workflows);
    if evaluation.status == WorkflowReleaseEvaluationStatus::StructurallyValid {
        CliEnvelope::ok("guide.rollout-audit", evaluation)
    } else {
        CliEnvelope::reject(
            "guide.rollout-audit",
            ExitReason::RejectedByGate,
            "workflow rollout structure is blocked; inspect typed issues and non-executable gaps",
            evaluation,
        )
    }
}

fn read_closed_rollout_yaml<T>(path: &Path, label: &str) -> Result<T, (ExitReason, String)>
where
    T: serde::de::DeserializeOwned,
{
    let text = std::fs::read_to_string(path).map_err(|error| {
        (
            ExitReason::EnvConfig,
            format!("cannot read {label} {}: {error}", path.display()),
        )
    })?;
    yaml_serde::from_str(&text).map_err(|error| {
        (
            ExitReason::InvalidDecisionShape,
            format!(
                "{label} {} is not a closed valid contract: {error}",
                path.display()
            ),
        )
    })
}

// ============================================================================
// guide govern-simulate -- inspect a non-authoritative P5b candidate result.
// ============================================================================

/// Agent-facing projection of one non-authoritative workflow simulation. The
/// legacy projection is present only when the caller explicitly supplies a
/// legacy workflow document; it remains simulation-only compatibility output.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct GovernSimulationPayload {
    pub simulation: WorkflowGovernanceSimulation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_projection: Option<LegacyWorkflowGovernanceProjection>,
}

/// Simulate one closed workflow-governance bundle and caller-authored input.
///
/// The output is explicitly `simulation_only`: candidate verdicts can guide
/// exploration but cannot unlock progression, completion, or execution. Only
/// malformed or structurally invalid contracts fail with
/// `invalid_decision_shape`.
#[must_use]
pub fn run_govern_simulate(
    bundle_file: &Path,
    input_file: &Path,
    legacy_workflow_file: Option<&Path>,
) -> CliEnvelope<GovernSimulationPayload> {
    let bundle_text = match std::fs::read_to_string(bundle_file) {
        Ok(text) => text,
        Err(error) => {
            return CliEnvelope::err(
                "guide.govern-simulate",
                ExitReason::InvalidDecisionShape,
                format!(
                    "cannot read workflow governance bundle {}: {error}",
                    bundle_file.display()
                ),
            );
        }
    };
    let bundle: WorkflowGovernanceBundleDocument = match yaml_serde::from_str(&bundle_text) {
        Ok(bundle) => bundle,
        Err(error) => {
            return CliEnvelope::err(
                "guide.govern-simulate",
                ExitReason::InvalidDecisionShape,
                format!(
                    "workflow governance bundle {} is not a closed valid contract: {error}",
                    bundle_file.display()
                ),
            );
        }
    };

    let input_text = match std::fs::read_to_string(input_file) {
        Ok(text) => text,
        Err(error) => {
            return CliEnvelope::err(
                "guide.govern-simulate",
                ExitReason::InvalidDecisionShape,
                format!(
                    "cannot read workflow governance input {}: {error}",
                    input_file.display()
                ),
            );
        }
    };
    let input: WorkflowGovernanceEvaluationDocument = match yaml_serde::from_str(&input_text) {
        Ok(input) => input,
        Err(error) => {
            return CliEnvelope::err(
                "guide.govern-simulate",
                ExitReason::InvalidDecisionShape,
                format!(
                    "workflow governance input {} is not a closed valid contract: {error}",
                    input_file.display()
                ),
            );
        }
    };

    let simulation = match simulate_workflow_governance(&bundle, &input) {
        Ok(simulation) => simulation,
        Err(rejection) => {
            let details = rejection
                .issues
                .iter()
                .map(|issue| format!("{:?} at {}: {}", issue.code, issue.path, issue.message))
                .collect::<Vec<_>>()
                .join("; ");
            return CliEnvelope::err(
                "guide.govern-simulate",
                ExitReason::InvalidDecisionShape,
                format!(
                    "workflow governance contracts were structurally rejected ({} issue(s)): {details}",
                    rejection.issues.len()
                ),
            );
        }
    };

    let legacy_projection = match legacy_workflow_file {
        Some(path) => {
            let text = match std::fs::read_to_string(path) {
                Ok(text) => text,
                Err(error) => {
                    return CliEnvelope::err(
                        "guide.govern-simulate",
                        ExitReason::InvalidDecisionShape,
                        format!("cannot read legacy workflow {}: {error}", path.display()),
                    );
                }
            };
            let document: WorkflowDocument = match yaml_serde::from_str(&text) {
                Ok(document) => document,
                Err(error) => {
                    return CliEnvelope::err(
                        "guide.govern-simulate",
                        ExitReason::InvalidDecisionShape,
                        format!(
                            "legacy workflow {} is not a closed valid WorkflowDocument: {error}",
                            path.display()
                        ),
                    );
                }
            };
            let workflow = document.workflow;
            let entry = CatalogEntry {
                id: workflow.id,
                phases: workflow.phases,
                workflow_ref: legacy_workflow_ref(path),
                triggers: workflow.trigger,
                prerequisites: workflow.inputs,
                outputs: workflow.outputs,
            };
            match project_legacy_workflow_compatibility(&simulation, &entry) {
                Ok(projection) => Some(projection),
                Err(error) => {
                    return CliEnvelope::err(
                        "guide.govern-simulate",
                        ExitReason::InvalidDecisionShape,
                        format!(
                            "legacy compatibility projection was rejected at {}: {}",
                            error.issue.path, error.issue.message
                        ),
                    );
                }
            }
        }
        None => None,
    };

    CliEnvelope::ok(
        "guide.govern-simulate",
        GovernSimulationPayload {
            simulation,
            legacy_projection,
        },
    )
}

fn legacy_workflow_ref(path: &Path) -> RepoPath {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let marker = "contracts/workflows/";
    let reference = normalized
        .rfind(marker)
        .map_or(normalized.as_str(), |index| &normalized[index..]);
    RepoPath(reference.to_owned())
}

// ============================================================================
// guide decide — validate a host-proposed GuideDecision (R2).
// ============================================================================

/// The success payload for `guide decide` when the decision is Accepted.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DecideAccepted {
    pub recommended_workflow: String,
    pub current_phase: String,
    pub proposed_next_phase: Option<String>,
    pub reason: String,
    /// Closed route derived from the exact validated `OperationContract`.
    pub route: GuideRoute,
    /// The exact authority response the host must render or execute next. The
    /// host may not replace its phase, workflow, action, or control flow.
    pub next_operation: OperationContractDocument,
    /// V5 — binding enforcement policy the agent must satisfy to advance this
    /// workflow. Populated when `--root` resolves a project; `None` when no
    /// project context is available (the legacy behavior).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforcement_policy: Option<EnforcementPolicy>,
}

/// The typed phase profile a `guide decide` acceptance projects from the one
/// accepted funnel-autonomy policy. Guide does not author or override these
/// values, and the projection grants no execution or phase authority.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnforcementPolicy {
    /// Whether mutation in this phase requires a covering lane claim.
    pub claim_required: bool,
    /// Fast or rigorous lane from the accepted phase profile.
    pub lane: FunnelLane,
    /// Gates attached or enforced automatically by the runtime path.
    pub automatic_gates: Vec<FunnelAutomaticGate>,
    /// Expected human-contact density for this phase.
    pub contact_density: FunnelContactDensity,
    /// How semantic uncertainty restores human guidance, research, or review.
    pub ambiguity_pressure: FunnelAmbiguityPressure,
    /// Whether procedural confirmation is expected, conditional, or forbidden.
    pub procedural_confirmation: FunnelProceduralConfirmation,
}

/// The failure payload for `guide decide` when the decision is Rejected.
/// Carries a machine-readable reject code so the host can self-correct (R2).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DecideRejected {
    /// One of: `unrecognized_current_phase` | `unknown_workflow` |
    /// `not_eligible_in_phase` | `illegal_transition`.
    pub reject_code: String,
    pub detail: String,
}

/// Run `guide decide`. Loads decision + catalog + gates, validates, emits
/// Accepted|Rejected envelope with the DD10 exit code.
///
/// # Errors
/// Returns an `InvalidDecisionShape` (exit 3) envelope if the decision file
/// cannot be deserialized; `EnvConfig` (exit 5) if the catalog won't load;
/// `RejectedByGate` (exit 2) if the engine refuses the decision.
#[must_use]
pub fn run_decide(
    decision_file: &Path,
    catalog_dir: Option<&Path>,
    gates: &[ProvidedGateResult],
    project: Option<&crate::project_cmd::ProjectResolvePayload>,
) -> CliEnvelope<DecideAccepted> {
    // 1. Load the closed guide protocol. A decision without the exact next
    // OperationContract is incomplete and cannot authorize host control flow.
    let protocol_text = match std::fs::read_to_string(decision_file) {
        Ok(text) => text,
        Err(error) => {
            return CliEnvelope::err(
                "guide.decide",
                ExitReason::InvalidDecisionShape,
                format!("cannot read guide protocol file: {error}"),
            );
        }
    };
    let protocol = match yaml_serde::from_str::<GuideProtocolDocument>(&protocol_text) {
        Ok(document) => document,
        Err(error) => {
            return CliEnvelope::err(
                "guide.decide",
                ExitReason::InvalidDecisionShape,
                format!("file is not a valid GuideProtocolDocument: {error}"),
            );
        }
    };
    let decision = &protocol.guide_protocol.decision;

    // Tombstones are checked before the operational catalog. A retired id is
    // neither Unknown nor Accepted, even if a stale external catalog still
    // contains the legacy workflow.
    let retired = match retired_workflow(&decision.recommended_workflow.0) {
        Ok(retired) => retired,
        Err(error) => {
            return CliEnvelope::err("guide.decide", ExitReason::EnvConfig, error);
        }
    };
    if let Some(retired) = retired {
        let detail = match serde_json::to_string(&retired) {
            Ok(detail) => detail,
            Err(error) => {
                return CliEnvelope::err(
                    "guide.decide",
                    ExitReason::EnvConfig,
                    format!("cannot serialize retirement diagnostic: {error}"),
                );
            }
        };
        let mut envelope: CliEnvelope<DecideAccepted> =
            CliEnvelope::err("guide.decide", ExitReason::RejectedByGate, detail);
        if let Some(error) = envelope.error.as_mut() {
            "workflow_retired".clone_into(&mut error.code.0);
        }
        return envelope;
    }

    // 2. Load the catalog.
    let report = resolve_catalog(catalog_dir);
    if !report.is_clean() {
        return CliEnvelope::err(
            "guide.decide",
            ExitReason::EnvConfig,
            format!("catalog load failed: {} error(s)", report.errors.len()),
        );
    }

    // 3. Validate the recommendation and its exact next OperationContract as
    // one protocol. No oracle, gate, doctor, adapter, or host action receives a
    // route unless this composition passes.
    match validate_guide_protocol(&protocol, &report.catalog, gates) {
        Ok(route) => {
            let enforcement_policy = match project.map(resolve_enforcement_policy).transpose() {
                Ok(policy) => policy,
                Err(error) => {
                    return CliEnvelope::err("guide.decide", ExitReason::EnvConfig, error);
                }
            };
            CliEnvelope::ok(
                "guide.decide",
                DecideAccepted {
                    recommended_workflow: decision.recommended_workflow.0.clone(),
                    current_phase: decision.current_phase.0.clone(),
                    proposed_next_phase: decision.proposed_next_phase.as_ref().map(|p| p.0.clone()),
                    reason: decision.reason.clone(),
                    route,
                    next_operation: protocol.guide_protocol.next_operation.clone(),
                    enforcement_policy,
                },
            )
        }
        Err(reason) => {
            let rejected = DecideRejected {
                reject_code: reason.code.as_str().to_owned(),
                detail: reason.detail,
            };
            let mut env: CliEnvelope<DecideAccepted> =
                CliEnvelope::err("guide.decide", ExitReason::RejectedByGate, &rejected.detail);
            if let Some(error) = env.error.as_mut() {
                error.code.0 = format!("{}:{}", rejected.reject_code, rejected.detail);
            }
            env
        }
    }
}

/// Project the exact phase profile from the accepted funnel-autonomy policy.
/// A missing project phase safely starts at Discovery; a malformed recorded
/// phase or unavailable policy fails closed rather than inventing defaults.
fn resolve_enforcement_policy(
    project: &crate::project_cmd::ProjectResolvePayload,
) -> Result<EnforcementPolicy, String> {
    let phase = match project.current_phase.as_deref() {
        Some(raw) => Phase::parse(raw)
            .ok_or_else(|| format!("project records an unsupported current phase: {raw}"))?,
        None => Phase::Discovery,
    };
    let policy = load_accepted_funnel_autonomy_policy().map_err(|rejection| {
        format!(
            "accepted funnel-autonomy policy is unavailable: {}",
            rejection
                .issues
                .iter()
                .map(|issue| format!("{}: {}", issue.path, issue.message))
                .collect::<Vec<_>>()
                .join("; ")
        )
    })?;
    let profile = evaluate_funnel_phase(policy, phase)
        .map_err(|rejection| {
            format!(
                "accepted funnel-autonomy phase profile is invalid: {}",
                rejection
                    .issues
                    .iter()
                    .map(|issue| format!("{}: {}", issue.path, issue.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        })?
        .profile;
    Ok(EnforcementPolicy {
        claim_required: profile.claim_required_for_mutation,
        lane: profile.lane,
        automatic_gates: profile.automatic_gates,
        contact_density: profile.contact_density,
        ambiguity_pressure: profile.ambiguity_pressure,
        procedural_confirmation: profile.procedural_confirmation,
    })
}

// ============================================================================
// guide status — orient the host: phase + eligible workflows + pending gates.
// ============================================================================

/// The `guide status` payload. Tells the host WHERE it is and WHAT it may do next.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StatusPayload {
    pub schema_version: String,
    /// The phase this status is oriented to.
    pub current_phase: String,
    /// Workflows eligible in `current_phase` (id + phases).
    pub eligible_workflows: Vec<StatusWorkflow>,
    /// Compatibility-only tombstones. These identifiers are never eligible
    /// and never enter routing candidates.
    pub retired_workflows: Vec<RetiredWorkflowDiagnostic>,
    /// Gates required to move FORWARD out of this phase, if any.
    pub pending_gates: Vec<StatusGate>,
    /// The phase each pending gate unlocks.
    pub next_phases: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StatusWorkflow {
    pub id: String,
    pub phases: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StatusGate {
    pub gate: String,
    pub unlocks: String,
}

/// Run `guide status` for a given phase. The host passes its current phase
/// (it always knows it from the method protocol); the engine reports what is
/// eligible now and which gates gate forward progress.
///
/// # Errors
/// Returns `EnvConfig` (exit 5) if the catalog won't load;
/// `InvalidDecisionShape` (exit 3) if `phase` does not categorize.
#[must_use]
pub fn run_status(catalog_dir: Option<&Path>, phase: &str) -> CliEnvelope<StatusPayload> {
    // categorize phase
    let Some(current) = Phase::parse(phase) else {
        return CliEnvelope::err(
            "guide.status",
            ExitReason::InvalidDecisionShape,
            format!("unrecognized phase '{phase}'"),
        );
    };

    let report = resolve_catalog(catalog_dir);
    if !report.is_clean() {
        return CliEnvelope::err(
            "guide.status",
            ExitReason::EnvConfig,
            format!("catalog load failed: {} error(s)", report.errors.len()),
        );
    }

    let tombstones = match embedded_retirement_tombstones() {
        Ok(catalog) => catalog,
        Err(error) => return CliEnvelope::err("guide.status", ExitReason::EnvConfig, error),
    };
    let retired_ids = retirement_ids(tombstones);
    let eligible_workflows = report
        .catalog
        .entries
        .iter()
        .filter(|entry| !retired_ids.contains(entry.id.0.as_str()))
        .filter(|e| {
            e.phases
                .iter()
                .any(|tag| Phase::tag_eligible(&tag.0, current))
        })
        .map(|e| StatusWorkflow {
            id: e.id.0.clone(),
            phases: e.phases.iter().map(|p| p.0.clone()).collect(),
        })
        .collect::<Vec<_>>();

    let retired_workflows = retirement_rows(tombstones);
    let (pending_gates, next_phases) = forward_gates_for(current);

    CliEnvelope::ok(
        "guide.status",
        StatusPayload {
            schema_version: GUIDE_ROUTING_PAYLOAD_SCHEMA_VERSION.to_owned(),
            current_phase: current.to_string(),
            eligible_workflows,
            retired_workflows,
            pending_gates,
            next_phases,
        },
    )
}

/// The forward gate + destination for a phase, in lockstep with `phase_transition`.
fn forward_gates_for(phase: Phase) -> (Vec<StatusGate>, Vec<String>) {
    use forge_core_decisions::GateKind;
    let (gate, unlocks) = match phase {
        Phase::Discovery => (Some(GateKind::Grill), Some(Phase::Specification)),
        Phase::Specification => (Some(GateKind::SystemDesign), Some(Phase::Plan)),
        Phase::Plan => (Some(GateKind::StoryReady), Some(Phase::BuildVerify)),
        Phase::BuildVerify => (Some(GateKind::Readiness), Some(Phase::ReadyOperate)),
        Phase::ReadyOperate => (Some(GateKind::Release), Some(Phase::Evolve)),
        _ => (None, None),
    };
    let pending_gates = gate
        .map(|g| StatusGate {
            gate: gate_str(g),
            unlocks: unlocks.unwrap().to_string(),
        })
        .into_iter()
        .collect();
    let next_phases = unlocks.map(|p| vec![p.to_string()]).unwrap_or_default();
    (pending_gates, next_phases)
}

fn gate_str(g: GateKind) -> String {
    match g {
        GateKind::Grill => "grill".into(),
        GateKind::SystemDesign => "system-design".into(),
        GateKind::StoryReady => "story-ready".into(),
        GateKind::Readiness => "readiness".into(),
        GateKind::Release => "release".into(),
    }
}
/// Dispatch entrypoint for the `forge-core guide` subcommand tree.
///
/// Routes to the concrete guide subcommand based on `args[1]`, and
/// prints usage on `--help` / unknown subcommand.
///
/// # Errors
///
/// Returns `ExitError::usage` when the subcommand is unknown. Sub-command
/// dispatchers may surface their own `ExitError::usage`,
/// `ExitError::invalid_value`, or `ExitError::with_code` variants.
pub fn run_guide_command(args: &[String]) -> Result<(), ExitError> {
    // Subcommand: `forge-core guide <subcommand> [...]`.
    let sub = args.get(1).map_or("--help", String::as_str);

    match sub {
        "describe" => run_guide_describe(&args[2..]),
        "decide" => run_guide_decide(&args[2..]),
        "status" => run_guide_status(&args[2..]),
        "migration-audit" => run_guide_migration_audit(&args[2..]),
        "rollout-audit" => run_guide_rollout_audit(&args[2..]),
        "govern-simulate" => run_guide_govern_simulate(&args[2..]),
        "--help" | "-h" | "help" => {
            print_guide_usage();
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core guide: unknown subcommand '{other}'. Try: {hint}",
            hint = guide_subcommand_hint()
        ))),
    }
}

fn print_guide_usage() {
    println!("forge-core guide <subcommand> [options]");
    for line in COMMAND_GUIDE.local_usage_lines() {
        println!("  {line}");
    }
}

fn guide_subcommand_hint() -> String {
    COMMAND_GUIDE.concrete_subcommand_hint()
}

fn guide_command_surface_usage_line_for(subcommand: &str) -> &'static str {
    COMMAND_GUIDE
        .usage_line_for_subcommand(subcommand)
        .unwrap_or("forge-core guide <subcommand> [options]")
}

fn guide_invalid_value_with_usage(subcommand: &str, message: &str) -> ExitError {
    ExitError::invalid_value(format!(
        "{message}\n\nusage:\n  {}",
        guide_command_surface_usage_line_for(subcommand)
    ))
}

pub fn guide_value(args: &[String], idx: usize) -> Option<&str> {
    args.get(idx)
        .filter(|value| !value.is_empty() && !value.starts_with("--"))
        .map(String::as_str)
}

/// Reads the value at `args[idx]`, returning `None` when missing, empty,
/// or starting with `--` (i.e. looks like the next flag).
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when [`guide_value`] returns `None`,
/// i.e. the slot at `idx` is missing, empty, or starts with `--`.
pub fn require_guide_value(
    args: &[String],
    idx: usize,
    subcommand: &str,
    flag: &str,
) -> Result<String, ExitError> {
    if let Some(value) = guide_value(args, idx) {
        Ok(value.to_owned())
    } else {
        let message = format!("guide {subcommand}: --{flag} requires a value");
        Err(guide_invalid_value_with_usage(subcommand, &message))
    }
}

#[must_use]
pub fn reject_unknown_guide_arg(subcommand: &str, arg: &str) -> ExitError {
    let message = format!("guide {subcommand}: unrecognized argument '{arg}'");
    eprintln!("{message}");
    guide_invalid_value_with_usage(subcommand, &message)
}

/// Runs the `forge-core guide describe` subcommand.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when an argument is missing or
/// unrecognized, and `ExitError::with_code` (via [`emit_guide`]) when the
/// describe envelope carries a non-zero exit code.
pub fn run_guide_describe(args: &[String]) -> Result<(), ExitError> {
    use forge_core_contracts::CliEnvelope;

    let mut catalog_dir: Option<std::path::PathBuf> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--catalog-dir" => {
                idx += 1;
                catalog_dir = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "describe",
                    "catalog-dir",
                )?));
            }
            "--no-json" | "--text" => want_json = false,
            "--json" => want_json = true,
            "--help" | "-h" => {
                println!("{}", guide_command_surface_usage_line_for("describe"));
                return Ok(());
            }
            other => return Err(reject_unknown_guide_arg("describe", other)),
        }
        idx += 1;
    }

    let env: CliEnvelope<DescribePayload> = run_describe(catalog_dir.as_deref());
    emit_guide(env, want_json)
}

/// Runs the `forge-core guide decide` subcommand.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when `--protocol-file` is missing
/// or an argument is unrecognized, and `ExitError::with_code` (via
/// [`emit_guide`]) when the decide envelope carries a non-zero exit code.
pub fn run_guide_decide(args: &[String]) -> Result<(), ExitError> {
    use forge_core_contracts::CliEnvelope;

    let mut decision_file: Option<std::path::PathBuf> = None;
    let mut root: Option<std::path::PathBuf> = None;
    let mut catalog_dir: Option<std::path::PathBuf> = None;
    let mut gates_file: Option<std::path::PathBuf> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--protocol-file" | "--decision-file" => {
                let flag = args[idx].trim_start_matches("--").to_owned();
                idx += 1;
                decision_file = Some(std::path::PathBuf::from(require_guide_value(
                    args, idx, "decide", &flag,
                )?));
            }
            "--root" => {
                idx += 1;
                root = Some(std::path::PathBuf::from(require_guide_value(
                    args, idx, "decide", "root",
                )?));
            }
            "--catalog-dir" => {
                idx += 1;
                catalog_dir = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "decide",
                    "catalog-dir",
                )?));
            }
            "--gates-file" => {
                idx += 1;
                gates_file = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "decide",
                    "gates-file",
                )?));
            }
            "--no-json" | "--text" => want_json = false,
            "--json" => want_json = true,
            "--help" | "-h" => {
                println!("{}", guide_command_surface_usage_line_for("decide"));
                return Ok(());
            }
            other => return Err(reject_unknown_guide_arg("decide", other)),
        }
        idx += 1;
    }

    let decision_file = decision_file.ok_or_else(|| {
        let message = "guide decide: --protocol-file is required";
        eprintln!("{message}");
        guide_invalid_value_with_usage("decide", message)
    })?;

    // Gates are optional (only needed for phase transitions). Loaded from a simple
    // YAML file: [{gate_kind: system-design, status: pass}, ...].
    let gates = load_gates(gates_file.as_deref());

    // Resolve project state (for the enforcement policy). When --root is
    // absent or the project isn't bootstrapped, the policy stays None and the
    // legacy (advisory) decide behavior is preserved.
    let project = root.as_deref().and_then(|r| resolve_project(r).ok());

    let env: CliEnvelope<DecideAccepted> = run_decide(
        &decision_file,
        catalog_dir.as_deref(),
        &gates,
        project.as_ref(),
    );
    emit_guide(env, want_json)
}

/// Runs the `forge-core guide status` subcommand.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when `--phase` is missing or an
/// argument is unrecognized, and `ExitError::with_code` (via [`emit_guide`])
/// when the status envelope carries a non-zero exit code.
pub fn run_guide_status(args: &[String]) -> Result<(), ExitError> {
    use forge_core_contracts::CliEnvelope;

    let mut phase: Option<String> = None;
    let mut root: Option<std::path::PathBuf> = None;
    let mut catalog_dir: Option<std::path::PathBuf> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--phase" => {
                idx += 1;
                phase = Some(require_guide_value(args, idx, "status", "phase")?);
            }
            "--root" => {
                idx += 1;
                root = Some(std::path::PathBuf::from(require_guide_value(
                    args, idx, "status", "root",
                )?));
            }
            "--catalog-dir" => {
                idx += 1;
                catalog_dir = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "status",
                    "catalog-dir",
                )?));
            }
            "--no-json" | "--text" => want_json = false,
            "--json" => want_json = true,
            "--help" | "-h" => {
                println!("{}", guide_command_surface_usage_line_for("status"));
                return Ok(());
            }
            other => return Err(reject_unknown_guide_arg("status", other)),
        }
        idx += 1;
    }

    // Phase authority: an explicit `--phase` wins (host override). Otherwise
    // read the authoritative phase from the project's `state.yaml`. If no root
    // is given or the state file is missing, fall back to `1-discovery` so a
    // freshly-bootstrapped project still has a usable funnel entry point.
    let phase = phase.unwrap_or_else(|| resolve_current_phase(root.as_deref()));

    let env: CliEnvelope<StatusPayload> = run_status(catalog_dir.as_deref(), &phase);
    emit_guide(env, want_json)
}

/// Runs the P5a read-only migration audit.
///
/// # Errors
///
/// Returns a usage/config error for malformed flags and propagates a typed
/// rejection when the audit finds unresolved classification or parity drift.
pub fn run_guide_migration_audit(args: &[String]) -> Result<(), ExitError> {
    let mut catalog_dir = None;
    let mut plan_file = None;
    let want_json = !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"));
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--catalog-dir" => {
                index += 1;
                catalog_dir = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    index,
                    "migration-audit",
                    "catalog-dir",
                )?));
            }
            "--plan-file" => {
                index += 1;
                plan_file = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    index,
                    "migration-audit",
                    "plan-file",
                )?));
            }
            "--json" | "--no-json" | "--text" => {}
            "--help" | "-h" => {
                println!(
                    "{}",
                    guide_command_surface_usage_line_for("migration-audit")
                );
                return Ok(());
            }
            other => return Err(reject_unknown_guide_arg("migration-audit", other)),
        }
        index += 1;
    }
    emit_guide(
        run_migration_audit(catalog_dir.as_deref(), plan_file.as_deref()),
        want_json,
    )
}

/// Runs the P5d.1 read-only release rollout audit.
///
/// # Errors
///
/// Returns a usage error for missing/unrecognized arguments, an
/// `invalid_decision_shape` envelope for malformed closed YAML contracts, and
/// a `rejected_by_gate` envelope with the typed evaluation when rollout gates
/// remain blocked.
pub fn run_guide_rollout_audit(args: &[String]) -> Result<(), ExitError> {
    let mut manifest_file = None;
    let mut batch_files = Vec::new();
    let mut catalog_dir = None;
    let mut plan_file = None;
    let mut want_json = true;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest-file" => {
                index += 1;
                manifest_file = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    index,
                    "rollout-audit",
                    "manifest-file",
                )?));
            }
            "--batch-file" => {
                index += 1;
                batch_files.push(std::path::PathBuf::from(require_guide_value(
                    args,
                    index,
                    "rollout-audit",
                    "batch-file",
                )?));
            }
            "--catalog-dir" => {
                index += 1;
                catalog_dir = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    index,
                    "rollout-audit",
                    "catalog-dir",
                )?));
            }
            "--plan-file" => {
                index += 1;
                plan_file = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    index,
                    "rollout-audit",
                    "plan-file",
                )?));
            }
            "--json" => want_json = true,
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("{}", guide_command_surface_usage_line_for("rollout-audit"));
                return Ok(());
            }
            other => return Err(reject_unknown_guide_arg("rollout-audit", other)),
        }
        index += 1;
    }

    let manifest_file = manifest_file.ok_or_else(|| {
        let message = "guide rollout-audit: --manifest-file is required";
        eprintln!("{message}");
        guide_invalid_value_with_usage("rollout-audit", message)
    })?;

    emit_guide(
        run_rollout_audit(
            &manifest_file,
            &batch_files,
            catalog_dir.as_deref(),
            plan_file.as_deref(),
        ),
        want_json,
    )
}

/// Runs the P5b deterministic, non-authoritative workflow simulation adapter.
///
/// # Errors
///
/// Returns a usage error for missing/unrecognized arguments and propagates an
/// `invalid_decision_shape` envelope when either closed contract, its semantic
/// structure, or an explicitly requested legacy projection is invalid.
pub fn run_guide_govern_simulate(args: &[String]) -> Result<(), ExitError> {
    let mut bundle_file = None;
    let mut input_file = None;
    let mut legacy_workflow_file = None;
    let mut want_json = true;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--bundle-file" => {
                index += 1;
                bundle_file = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    index,
                    "govern-simulate",
                    "bundle-file",
                )?));
            }
            "--input-file" => {
                index += 1;
                input_file = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    index,
                    "govern-simulate",
                    "input-file",
                )?));
            }
            "--legacy-workflow-file" => {
                index += 1;
                legacy_workflow_file = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    index,
                    "govern-simulate",
                    "legacy-workflow-file",
                )?));
            }
            "--json" => want_json = true,
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!(
                    "{}",
                    guide_command_surface_usage_line_for("govern-simulate")
                );
                return Ok(());
            }
            other => return Err(reject_unknown_guide_arg("govern-simulate", other)),
        }
        index += 1;
    }

    let bundle_file = bundle_file.ok_or_else(|| {
        let message = "guide govern-simulate: --bundle-file is required";
        eprintln!("{message}");
        guide_invalid_value_with_usage("govern-simulate", message)
    })?;
    let input_file = input_file.ok_or_else(|| {
        let message = "guide govern-simulate: --input-file is required";
        eprintln!("{message}");
        guide_invalid_value_with_usage("govern-simulate", message)
    })?;

    emit_guide(
        run_govern_simulate(&bundle_file, &input_file, legacy_workflow_file.as_deref()),
        want_json,
    )
}

/// Resolve the authoritative current phase for a project root. Reads
/// `<state_root>/state.yaml` via `resolve_project`; on any failure or missing
/// file, returns `"1-discovery"` as the funnel entry point. This makes the
/// runtime the phase authority rather than trusting whatever the agent passes
/// on `--phase`.
fn resolve_current_phase(root: Option<&std::path::Path>) -> String {
    const DEFAULT_PHASE: &str = "1-discovery";
    let Some(root) = root else {
        return DEFAULT_PHASE.to_string();
    };
    match resolve_project(root) {
        Ok(payload) => payload
            .current_phase
            .unwrap_or_else(|| DEFAULT_PHASE.to_string()),
        Err(_) => DEFAULT_PHASE.to_string(),
    }
}

/// Parse the gates-file into `ProvidedGateResult` rows. Empty/absent = no gates provided.
#[must_use]
pub fn load_gates(path: Option<&std::path::Path>) -> Vec<forge_core_decisions::ProvidedGateResult> {
    use forge_core_contracts::gate::GateStatus;
    use forge_core_decisions::GateKind;
    let Some(path) = path else {
        return Vec::new();
    };
    #[allow(clippy::items_after_statements)]
    #[derive(serde::Deserialize)]
    struct GateRow {
        gate_kind: String,
        status: String,
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let rows: Vec<GateRow> = yaml_serde::from_str(&text).unwrap_or_default();
    rows.into_iter()
        .filter_map(|r| {
            let gk = match r.gate_kind.as_str() {
                "system-design" => Some(GateKind::SystemDesign),
                "grill" | "grill-gate" => Some(GateKind::Grill),
                _ => None,
            }?;
            let status = match r.status.as_str() {
                "pass" => GateStatus::Pass,
                "fail" => GateStatus::Fail,
                "concerns" => GateStatus::Concerns,
                "missing" => GateStatus::Missing,
                _ => GateStatus::NotApplicable,
            };
            Some(forge_core_decisions::ProvidedGateResult {
                gate_kind: gk,
                status,
            })
        })
        .collect()
}

/// Emit a guide envelope to stdout (JSON) or stderr (text) and propagate
/// the envelope's exit code as an `ExitError` when non-zero.
///
/// # Errors
///
/// Returns `ExitError::with_code` carrying the envelope's non-zero exit
/// code so the entrypoint can translate it into `process::exit(code)`.
///
/// # Panics
///
/// Does NOT panic on serialization failure (V4.A). In JSON mode, if `env`
/// cannot be serialized by `serde_json`, an error is written to stderr and an
/// `ExitError::env_config` (exit code 5) is returned. `T: Serialize` is bound,
/// so this is effectively infallible in practice, but a panic is the wrong
/// tool in a shared stdout emit path.
pub fn emit_guide<T: serde::Serialize>(
    env: forge_core_contracts::CliEnvelope<T>,
    want_json: bool,
) -> Result<(), ExitError> {
    let code = env.exit_code();
    if want_json {
        // Serialize before printing so a failure is a typed error, not a panic.
        let json = serde_json::to_string_pretty(&env).map_err(|e| {
            eprintln!("internal error: failed to serialize guide envelope: {e}");
            ExitError::env_config(format!("failed to serialize guide envelope: {e}"))
        })?;
        println!("{json}");
    } else if !env.ok {
        eprintln!(
            "guide failed: {}",
            env.error.as_ref().map_or("unknown", |e| e.message.as_str())
        );
    }
    if code == 0 {
        Ok(())
    } else {
        Err(ExitError::with_code(code, String::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    fn tempfile_dir() -> std::path::PathBuf {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("forge-guide-test-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    use forge_core_contracts::{CatalogEntry, StableId};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn assert_guide_error_projects_only_subcommand_usage(
        error: &ExitError,
        subcommand: &str,
        expected_diagnostic: &str,
    ) {
        assert_eq!(error.exit_code(), 3);
        let message = error.message();
        assert!(
            message.contains(expected_diagnostic),
            "error should preserve diagnostic {expected_diagnostic:?}: {message}"
        );
        let projected = COMMAND_GUIDE
            .usage_line_for_subcommand(subcommand)
            .expect("guide subcommand usage");
        assert!(
            message.contains(projected),
            "error should project {subcommand} Command Surface usage {projected:?}: {message}"
        );
        for sibling in [
            "describe",
            "decide",
            "status",
            "migration-audit",
            "rollout-audit",
            "govern-simulate",
        ] {
            if sibling != subcommand {
                let sibling_usage = COMMAND_GUIDE
                    .usage_line_for_subcommand(sibling)
                    .expect("sibling usage");
                assert!(
                    !message.contains(sibling_usage),
                    "error for {subcommand} should not leak {sibling} usage: {message}"
                );
            }
        }
    }

    fn real_catalog_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/workflows")
            .canonicalize()
            .expect("catalog dir")
    }

    #[test]
    fn describe_emits_all_68_operational_workflows_compactly() {
        let env = run_describe(Some(&real_catalog_dir()));
        assert!(env.ok, "describe should succeed");
        assert_eq!(env.exit_code(), 0);
        let payload = env.data.as_ref().expect("payload");
        assert_eq!(payload.workflows.len(), 68);
        // every row is compact: id + phases + one summary line
        for w in &payload.workflows {
            assert!(!w.id.is_empty());
            assert!(!w.phases.is_empty());
            assert!(!w.summary.is_empty());
        }
    }

    #[test]
    fn describe_includes_phases_gates_exit_reasons_and_schema_version() {
        let env = run_describe(Some(&real_catalog_dir()));
        let p = env.data.as_ref().expect("payload");
        assert_eq!(p.schema_version, GUIDE_ROUTING_PAYLOAD_SCHEMA_VERSION);
        assert!(p.phases.contains(&"1-discovery".to_string()));
        assert!(p.phases.contains(&"6-evolve".to_string()));
        assert!(p.gates.iter().any(|g| g.gate == "system-design"));
        assert!(p.exit_reasons.contains(&"rejected_by_gate".to_string()));
    }

    #[test]
    fn describe_returns_env_config_envelope_when_catalog_dir_missing() {
        let env = run_describe(Some(std::path::Path::new("/nonexistent/does/not/exist")));
        assert!(!env.ok);
        assert_eq!(env.exit_reason.0, StableId("env_config".into()).0);
        assert_eq!(env.exit_code(), 5);
    }

    #[test]
    fn compact_workflow_uses_first_trigger_as_summary() {
        let entry = CatalogEntry {
            id: StableId("x".into()),
            phases: vec![StableId("1-discovery".into())],
            workflow_ref: forge_core_contracts::RepoPath("p".into()),
            triggers: vec!["does X, use when Y".into(), "second".into()],
            prerequisites: vec![],
            outputs: vec![],
        };
        let cw = compact_workflow(&entry);
        assert_eq!(cw.summary, "does X, use when Y");
    }

    #[test]
    fn payload_serializes_to_json_cleanly() {
        let env = run_describe(Some(&real_catalog_dir()));
        let json = serde_json::to_string(&env).expect("serialize");
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"workflows\""));
    }

    // --- guide decide tests (S3.3) ---

    fn base_protocol() -> GuideProtocolDocument {
        yaml_serde::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/fixtures/guide-protocol-v0/facilitation.yaml"
        )))
        .expect("guide protocol fixture")
    }

    fn protocol_for(
        workflow: &str,
        phase: &str,
        proposed_next_phase: Option<&str>,
    ) -> GuideProtocolDocument {
        let mut document = base_protocol();
        let protocol = &mut document.guide_protocol;
        protocol.decision.recommended_workflow = StableId(workflow.to_owned());
        protocol.decision.current_phase = StableId(phase.to_owned());
        protocol.decision.proposed_next_phase =
            proposed_next_phase.map(|value| StableId(value.to_owned()));
        protocol
            .next_operation
            .operation_contract
            .recommendation
            .workflow = StableId(workflow.to_owned());
        protocol
            .next_operation
            .operation_contract
            .recommendation
            .phase = StableId(proposed_next_phase.unwrap_or(phase).to_owned());
        protocol.next_operation.operation_contract.contract_id =
            StableId(format!("op_guide_{workflow}"));
        document
    }

    fn write_protocol(
        tmp: &std::path::Path,
        name: &str,
        document: &GuideProtocolDocument,
    ) -> std::path::PathBuf {
        let path = tmp.join(name);
        let body = yaml_serde::to_string(document).expect("serialize guide protocol");
        std::fs::write(&path, body).expect("write guide protocol");
        path
    }

    fn write_raw(tmp: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
        let path = tmp.join(name);
        std::fs::write(&path, body).expect("write raw fixture");
        path
    }

    const BAD_YAML: &str = "schema_version: \"0.1\"\nguide_protocol: { not valid";

    #[test]
    fn decide_accepts_valid_in_phase_decision() {
        let tmp = tempfile_dir();
        let df = write_protocol(
            &tmp,
            "d.yaml",
            &protocol_for("brainstorming", "1-discovery", None),
        );
        let env = run_decide(&df, Some(&real_catalog_dir()), &[], None);
        assert!(env.ok, "should accept: {:?}", env.error);
        assert_eq!(env.exit_code(), 0);
        let p = env.data.as_ref().expect("payload");
        assert_eq!(p.recommended_workflow, "brainstorming");
        assert_eq!(p.route, GuideRoute::Facilitation);
        assert_eq!(
            p.next_operation
                .operation_contract
                .recommendation
                .workflow
                .0,
            "brainstorming"
        );
    }

    #[test]
    fn decide_rejects_ineligible_workflow_with_typed_code() {
        let tmp = tempfile_dir();
        let df = write_protocol(
            &tmp,
            "d.yaml",
            &protocol_for("create-epics", "1-discovery", None),
        );
        let env = run_decide(&df, Some(&real_catalog_dir()), &[], None);
        assert!(!env.ok);
        assert_eq!(env.exit_reason.0, "rejected_by_gate");
        assert_eq!(env.exit_code(), 2);
        let code = env.error.as_ref().expect("error").code.0.clone();
        assert!(code.starts_with("not_eligible_in_phase"), "got: {code}");
    }

    #[test]
    fn decide_rejects_unknown_workflow() {
        let tmp = tempfile_dir();
        let df = write_protocol(&tmp, "d.yaml", &protocol_for("nope", "1-discovery", None));
        let env = run_decide(&df, Some(&real_catalog_dir()), &[], None);
        assert!(!env.ok);
        let code = env.error.as_ref().expect("error").code.0.clone();
        assert!(code.starts_with("unknown_workflow"), "got: {code}");
    }

    #[test]
    fn retired_workflow_is_typed_and_never_unknown_or_accepted() {
        let tmp = tempfile_dir();
        let df = write_protocol(
            &tmp,
            "retired.yaml",
            &protocol_for("architecture", "1-discovery", None),
        );
        let env = run_decide(&df, Some(&real_catalog_dir()), &[], None);

        assert_eq!(env.exit_reason.0, "rejected_by_gate");
        assert!(env.data.is_none());
        let error = env.error.expect("typed retirement diagnostic");
        assert_eq!(error.code.0, "workflow_retired");
        assert!(error.message.contains("policy.workflow.architecture"));
        assert!(error.message.contains("replacement_argv"));
    }

    #[test]
    fn every_retired_workflow_is_typed_and_never_routed() {
        let tombstones = embedded_retirement_tombstones().expect("embedded tombstones");
        assert_eq!(
            tombstones
                .workflow_retirement_tombstone_catalog
                .tombstones
                .len(),
            42
        );
        let tmp = tempfile_dir();
        let decision_path = tmp.join("retired.yaml");

        for tombstone in &tombstones.workflow_retirement_tombstone_catalog.tombstones {
            let document = protocol_for(&tombstone.workflow_id.0, "1-discovery", None);
            std::fs::write(
                &decision_path,
                yaml_serde::to_string(&document).expect("serialize retired protocol"),
            )
            .expect("write retired protocol");
            let envelope = run_decide(&decision_path, Some(&real_catalog_dir()), &[], None);

            assert!(!envelope.ok, "{} must not route", tombstone.workflow_id.0);
            assert!(envelope.data.is_none());
            let error = envelope.error.expect("typed retirement diagnostic");
            assert_eq!(
                error.code.0, "workflow_retired",
                "{} must not degrade to unknown_workflow",
                tombstone.workflow_id.0
            );
        }
    }

    #[test]
    fn describe_separates_operational_routes_from_retired_tombstones() {
        let env = run_describe(Some(&real_catalog_dir()));
        let payload = env.data.expect("describe payload");
        assert_eq!(payload.workflows.len(), 68);
        assert_eq!(payload.retired_workflows.len(), 42);
        assert!(payload
            .retired_workflows
            .iter()
            .any(|row| row.workflow_id == "architecture"));
        assert!(!payload.workflows.iter().any(|row| row.id == "architecture"));
    }

    #[test]
    fn tombstones_come_only_from_the_verified_checkpoint() {
        let tombstones = embedded_retirement_tombstones().expect("verified checkpoint");
        assert_eq!(
            tombstones
                .workflow_retirement_tombstone_catalog
                .tombstones
                .len(),
            42
        );
    }

    #[test]
    fn decide_returns_invalid_decision_shape_on_bad_yaml() {
        let tmp = tempfile_dir();
        let df = write_raw(&tmp, "d.yaml", BAD_YAML);
        let env = run_decide(&df, Some(&real_catalog_dir()), &[], None);
        assert!(!env.ok);
        assert_eq!(env.exit_reason.0, "invalid_decision_shape");
        assert_eq!(env.exit_code(), 3);
    }

    #[test]
    fn decide_returns_env_config_when_decision_file_unreadable() {
        let env = run_decide(
            std::path::Path::new("/no/such/file.yaml"),
            Some(&real_catalog_dir()),
            &[],
            None,
        );
        assert!(!env.ok);
        // missing decision file = invalid input (no decision to validate) -> exit 3
        assert_eq!(env.exit_code(), 3);
    }

    // --- guide status tests (S3.4) ---

    #[test]
    fn status_reports_eligible_workflows_and_pending_gate() {
        let env = run_status(Some(&real_catalog_dir()), "2-specification");
        assert!(env.ok, "{:?}", env.error);
        let p = env.data.as_ref().expect("payload");
        assert_eq!(p.current_phase, "2-specification");
        assert!(!p.eligible_workflows.is_empty());
        assert_eq!(p.retired_workflows.len(), 42);
        assert!(!p
            .eligible_workflows
            .iter()
            .any(|workflow| workflow.id == "architecture"));
        // Tombstones remain diagnostics only, never eligible routes.
        assert!(p
            .retired_workflows
            .iter()
            .any(|workflow| workflow.workflow_id == "adversarial-review"));
        // the system-design gate unlocks 3-plan
        assert_eq!(p.pending_gates.len(), 1);
        assert_eq!(p.pending_gates[0].gate, "system-design");
        assert_eq!(p.pending_gates[0].unlocks, "3-plan");
        assert_eq!(p.next_phases, vec!["3-plan".to_string()]);
    }

    #[test]
    fn status_rejects_unknown_phase() {
        let env = run_status(Some(&real_catalog_dir()), "nonsense");
        assert!(!env.ok);
        assert_eq!(env.exit_reason.0, "invalid_decision_shape");
        assert_eq!(env.exit_code(), 3);
    }

    #[test]
    fn status_accepts_phase_aliases() {
        // Phase::parse is permissive: "3", "plan", "3-plan" all categorize.
        for alias in ["3", "plan", "3-plan"] {
            let env = run_status(Some(&real_catalog_dir()), alias);
            assert!(env.ok, "alias '{alias}' should parse: {:?}", env.error);
            assert_eq!(env.data.as_ref().unwrap().current_phase, "3-plan");
        }
    }

    #[test]
    fn status_terminal_phase_has_no_pending_gate() {
        // evolve is the last phase; no forward gate.
        let env = run_status(Some(&real_catalog_dir()), "6-evolve");
        assert!(env.ok);
        assert!(env.data.as_ref().unwrap().pending_gates.is_empty());
    }

    #[test]
    fn guide_value_requires_present_non_flag_value() {
        let parsed: Vec<String> = ["--catalog-dir", "contracts/workflows"]
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(guide_value(&parsed, 1), Some("contracts/workflows"));

        let missing: Vec<String> = ["--catalog-dir"].iter().map(ToString::to_string).collect();
        assert_eq!(guide_value(&missing, 1), None);

        let next_flag: Vec<String> = ["--catalog-dir", "--no-json"]
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(guide_value(&next_flag, 1), None);
    }

    #[test]
    fn guide_usage_projects_command_surface_lines() {
        let mut usage = String::from("forge-core guide <subcommand> [options]");
        for line in COMMAND_GUIDE.local_usage_lines() {
            usage.push('\n');
            usage.push_str("  ");
            usage.push_str(line);
        }

        assert!(
            usage.starts_with("forge-core guide <subcommand> [options]"),
            "guide usage should keep the local command-tree header: {usage}"
        );
        for line in COMMAND_GUIDE.usage_lines {
            let subcommand_usage = COMMAND_GUIDE.local_usage_line(line);
            assert!(
                usage.contains(subcommand_usage),
                "guide usage should include projected Command Surface line {subcommand_usage:?}: {usage}"
            );
        }
        assert_eq!(
            guide_subcommand_hint(),
            "describe | decide | status | migration-audit | rollout-audit | govern-simulate"
        );
    }

    #[test]
    fn guide_subcommand_help_lookup_projects_full_command_surface_lines() {
        for subcommand in [
            "describe",
            "decide",
            "status",
            "migration-audit",
            "rollout-audit",
            "govern-simulate",
        ] {
            let usage = guide_command_surface_usage_line_for(subcommand);
            assert_eq!(
                Some(usage),
                COMMAND_GUIDE.usage_line_for_subcommand(subcommand),
                "guide {subcommand} help should come from the Command Surface"
            );
        }
    }

    #[test]
    fn guide_missing_flag_value_reports_subcommand_usage() {
        let error =
            run_guide_describe(&args(&["--catalog-dir", "--json"])).expect_err("parse error");

        assert_guide_error_projects_only_subcommand_usage(
            &error,
            "describe",
            "guide describe: --catalog-dir requires a value",
        );
    }

    #[test]
    fn guide_unknown_arg_reports_subcommand_usage() {
        let error = run_guide_status(&args(&["--bogus"])).expect_err("parse error");

        assert_guide_error_projects_only_subcommand_usage(
            &error,
            "status",
            "guide status: unrecognized argument '--bogus'",
        );
    }

    #[test]
    fn guide_missing_required_flags_report_subcommand_usage() {
        let decide_error = run_guide_decide(&args(&[])).expect_err("missing decision file");
        assert_guide_error_projects_only_subcommand_usage(
            &decide_error,
            "decide",
            "guide decide: --protocol-file is required",
        );

        // `guide status` without --phase no longer errors: it falls back to
        // the authoritative `state.yaml` phase (or `1-discovery` when no root
        // is provided). Only `--protocol-file` is required on `decide`.
        run_guide_status(&args(&[])).expect("status falls back to 1-discovery");

        let govern_error = run_guide_govern_simulate(&args(&[])).expect_err("missing bundle file");
        assert_guide_error_projects_only_subcommand_usage(
            &govern_error,
            "govern-simulate",
            "guide govern-simulate: --bundle-file is required",
        );

        let rollout_error = run_guide_rollout_audit(&args(&[])).expect_err("missing manifest file");
        assert_guide_error_projects_only_subcommand_usage(
            &rollout_error,
            "rollout-audit",
            "guide rollout-audit: --manifest-file is required",
        );
    }

    #[test]
    fn guide_status_accepts_explicit_json_mode() {
        let status_args = args(&[
            "--json",
            "--phase",
            "3-plan",
            "--catalog-dir",
            real_catalog_dir().to_str().expect("catalog path utf-8"),
        ]);
        let result = run_guide_status(&status_args);

        assert!(result.is_ok(), "explicit --json should parse");
    }

    #[test]
    fn post_retirement_migration_audit_is_read_only_and_reports_historical_plan_drift() {
        let envelope = run_migration_audit(Some(&real_catalog_dir()), None);
        assert!(!envelope.ok, "the historical 110-item plan must not be replayed over the 68-item operational catalog");
        let audit = envelope.data.expect("migration audit payload");
        assert_eq!(audit.catalog_count, 68);
        assert_eq!(audit.classified_count, 68);
        assert_eq!(audit.shadow_parity.equivalent_count, 68);
        assert_eq!(audit.shadow_parity.drift_count, 0);
        assert!(!audit.shadow_parity.mutation_allowed);
        assert!(!audit.deletion_baseline.retirement_allowed);
        assert_eq!(audit.manifest.entries.len(), 68);
        assert!(audit.manifest.manifest_digest.starts_with("sha256:"));
        assert!(audit.issues.iter().any(|issue| {
            matches!(
                issue.code,
                forge_core_decisions::WorkflowMigrationIssueCode::CatalogCountMismatch
            )
        }));
    }

    #[test]
    fn default_migration_audit_uses_frozen_110_item_legacy_subject() {
        let envelope = run_migration_audit(None, None);
        assert!(envelope.ok, "frozen audit: {:?}", envelope.error);
        let audit = envelope.data.expect("migration audit payload");
        assert_eq!(audit.catalog_count, 110);
        assert_eq!(audit.classified_count, 110);
        assert_eq!(audit.shadow_parity.equivalent_count, 110);
        assert_eq!(audit.shadow_parity.drift_count, 0);
        assert!(!audit.shadow_parity.mutation_allowed);
    }

    #[test]
    fn migration_audit_parser_accepts_explicit_sources_and_rejects_unknown_flags() {
        let plan = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/policies/workflow-migration-foundation-v0.yaml");
        let result = run_guide_migration_audit(&args(&[
            "--catalog-dir",
            real_catalog_dir().to_str().expect("catalog utf-8"),
            "--plan-file",
            plan.to_str().expect("plan utf-8"),
            "--json",
        ]));
        let error = result.expect_err("historical plan must be rejected after retirement");
        assert_eq!(error.exit_code(), 2);

        let error = run_guide_migration_audit(&args(&["--mutate-legacy"]))
            .expect_err("authority-like flag must fail");
        assert!(error.to_string().contains("unrecognized argument"));
        assert!(error.to_string().contains("guide migration-audit"));
    }

    #[test]
    fn govern_simulate_parser_requires_both_closed_contracts_and_rejects_unknown_flags() {
        let missing_input = run_guide_govern_simulate(&args(&["--bundle-file", "bundle.yaml"]))
            .expect_err("input file is required");
        assert_guide_error_projects_only_subcommand_usage(
            &missing_input,
            "govern-simulate",
            "guide govern-simulate: --input-file is required",
        );

        let unknown = run_guide_govern_simulate(&args(&["--invent-completion"]))
            .expect_err("unknown authority flag");
        assert_guide_error_projects_only_subcommand_usage(
            &unknown,
            "govern-simulate",
            "guide govern-simulate: unrecognized argument '--invent-completion'",
        );
    }

    #[test]
    fn rollout_audit_parser_accepts_repeated_batches_and_rejects_unknown_flags() {
        let missing_value = run_guide_rollout_audit(&args(&["--manifest-file", "--json"]))
            .expect_err("manifest value is required");
        assert_guide_error_projects_only_subcommand_usage(
            &missing_value,
            "rollout-audit",
            "guide rollout-audit: --manifest-file requires a value",
        );

        let unknown = run_guide_rollout_audit(&args(&["--grant-authority"]))
            .expect_err("unknown authority flag");
        assert_guide_error_projects_only_subcommand_usage(
            &unknown,
            "rollout-audit",
            "guide rollout-audit: unrecognized argument '--grant-authority'",
        );
    }

    // ------------------------------------------------------------------------
    // Wave 2 (D4 / FRUST-031): funnel-of-autonomy contact_density enforcement
    // ------------------------------------------------------------------------
    // The policy must carry a binding contact_density per phase so the agent
    // host knows whether asking is expected (discovery/spec), conditional
    // (plan/evolve), or a funnel violation (build/ready — near-silent lane).
    // Regression: previously discovery/spec/plan were lumped as one
    // "human-heavy" block, causing agents to over-ask during mechanical
    // plan/build execution.

    fn resolve_payload(phase: Option<&str>) -> crate::project_cmd::ProjectResolvePayload {
        crate::project_cmd::ProjectResolvePayload {
            project_id: "test-project".to_string(),
            project_root: "/tmp/test".to_string(),
            link_path: None,
            sidecar_root: "/tmp/test-sidecar".to_string(),
            state_root: "/tmp/test-sidecar/.forge-method".to_string(),
            state_exists: true,
            layout: crate::project_cmd::ProjectLayoutKind::Sidecar,
            current_phase: phase.map(str::to_string),
        }
    }

    #[test]
    fn contact_density_is_high_in_discovery_and_specification() {
        for phase in ["1-discovery", "2-specification"] {
            let policy = resolve_enforcement_policy(&resolve_payload(Some(phase)))
                .expect("accepted funnel policy");
            assert_eq!(
                policy.contact_density,
                FunnelContactDensity::High,
                "phase {phase} must be high-contact (agent provokes/extracts/confirms)"
            );
        }
    }

    #[test]
    fn contact_density_is_medium_in_plan_and_evolve() {
        // CRITICAL: plan must NOT be lumped with discovery/spec. The human
        // approves sprint slicing, then the agent executes mechanical work
        // without asking for procedural confirmation. Evolve is the same shape
        // (correct-course feedback, then proceed).
        for phase in ["3-plan", "6-evolve"] {
            let policy = resolve_enforcement_policy(&resolve_payload(Some(phase)))
                .expect("accepted funnel policy");
            assert_eq!(
                policy.contact_density,
                FunnelContactDensity::Medium,
                "phase {phase} must be medium-contact (approve, then execute)"
            );
        }
    }

    #[test]
    fn contact_density_is_low_in_build_verify_and_ready_operate() {
        // The near-silent lane. Asking for permission on already-aligned,
        // gated, mechanical work here is a funnel violation, not a courtesy.
        for phase in ["4-build-verify", "5-ready-operate"] {
            let policy = resolve_enforcement_policy(&resolve_payload(Some(phase)))
                .expect("accepted funnel policy");
            assert_eq!(
                policy.contact_density,
                FunnelContactDensity::Low,
                "phase {phase} must be low-contact (near-silent contractual execution)"
            );
        }
    }

    #[test]
    fn contact_density_defaults_to_medium_for_route_and_missing_phase() {
        // Route (rank 0) and missing/unparseable phase both fall back safely.
        let route_policy = resolve_enforcement_policy(&resolve_payload(Some("0-route")))
            .expect("accepted route policy");
        assert_eq!(route_policy.contact_density, FunnelContactDensity::Medium);
        let none_policy =
            resolve_enforcement_policy(&resolve_payload(None)).expect("accepted discovery policy");
        assert_eq!(
            none_policy.contact_density,
            FunnelContactDensity::High,
            "missing phase falls back to discovery, which is high-contact"
        );
    }

    #[test]
    fn enforcement_policy_projects_typed_funnel_controls() {
        let policy = resolve_enforcement_policy(&resolve_payload(Some("4-build-verify")))
            .expect("accepted build-verify policy");
        assert_eq!(policy.lane, FunnelLane::Rigorous);
        assert_eq!(
            policy.ambiguity_pressure,
            FunnelAmbiguityPressure::GateReview
        );
        assert_eq!(
            policy.procedural_confirmation,
            FunnelProceduralConfirmation::Forbidden
        );
        assert!(policy
            .automatic_gates
            .contains(&FunnelAutomaticGate::ProtectedBoundary));
    }

    #[test]
    fn malformed_project_phase_fails_closed() {
        let error = resolve_enforcement_policy(&resolve_payload(Some("invented-phase")))
            .expect_err("unknown phase must not fall back");
        assert!(error.contains("unsupported current phase"));
    }

    #[test]
    fn enforcement_policy_with_contact_density_roundtrips_through_json() {
        // Non-regression for JSON consumers (host agents, MCP projection):
        // the new field must serialize and deserialize cleanly alongside the
        // existing fields, and old payloads without it must still parse.
        let policy = resolve_enforcement_policy(&resolve_payload(Some("4-build-verify")))
            .expect("accepted build-verify policy");
        let json = serde_json::to_string(&policy).expect("serialize");
        assert!(
            json.contains("\"contact_density\":\"low\""),
            "contact_density must appear in JSON output: {json}"
        );
        let back: EnforcementPolicy = serde_json::from_str(&json).expect("deserialize own output");
        assert_eq!(policy, back);

        // An older payload (pre-Wave-2) missing contact_density must still be
        // tolerable by deserialization — but since the field is non-optional,
        // we document the contract: callers built against this struct must
        // supply it. The JSON shape change is additive for any reader using a
        // dynamic parser; static consumers must rebuild against the new struct.
        let legacy_json = r#"{"claim_required":false,"lane":"fast","automatic_gates":[]}"#;
        let legacy_result: Result<EnforcementPolicy, _> = serde_json::from_str(legacy_json);
        assert!(
            legacy_result.is_err(),
            "legacy payload without contact_density must fail closed (typed contract, not silent default): {legacy_result:?}"
        );
    }
}
