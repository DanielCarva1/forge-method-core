//! P5a read-only workflow migration inventory, classification, and shadow parity.
//!
//! This Module deliberately does not execute workflows or retire legacy fields.
//! It makes the migration surface explicit, links every legacy workflow to
//! candidate governance targets, and proves the compatibility projection in
//! shadow mode before P5b is allowed to move authority.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use forge_core_contracts::{
    Catalog, CatalogEntry, LegacyWorkflowField, LegacyWorkflowFieldMapping,
    LegacyWorkflowFieldRole, StableId, Workflow, WorkflowCompatibilityField,
    WorkflowGoldenPathSelection, WorkflowMigrationAuthority, WorkflowMigrationDisposition,
    WorkflowMigrationPlanDocument, WorkflowRetirementGate, WorkflowShadowMode,
    WORKFLOW_MIGRATION_PLAN_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::catalog::LoadedWorkflowDocument;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMigrationAuditStatus {
    ReadyForShadow,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationAudit {
    pub status: WorkflowMigrationAuditStatus,
    pub catalog_count: usize,
    pub classified_count: usize,
    pub unresolved_count: usize,
    pub golden_path_count: usize,
    pub domain_pack_candidate_count: usize,
    pub compatibility_playbook_count: usize,
    pub quarantined_count: usize,
    pub shadow_parity: WorkflowShadowParitySummary,
    pub deletion_baseline: WorkflowDeletionBaseline,
    pub manifest: WorkflowMigrationManifest,
    pub issues: Vec<WorkflowMigrationIssue>,
}

/// Deterministic typed migration manifest for the complete legacy catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationManifest {
    pub schema_version: String,
    pub plan_id: String,
    pub catalog_digest: String,
    pub manifest_digest: String,
    pub entries: Vec<WorkflowMigrationAssessment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowShadowParitySummary {
    pub mode: WorkflowShadowMode,
    pub mutation_allowed: bool,
    pub equivalent_count: usize,
    pub drift_count: usize,
    pub exact_fields: Vec<WorkflowCompatibilityField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDeletionBaseline {
    pub catalog_digest: String,
    pub phases: usize,
    pub triggers: usize,
    pub inputs: usize,
    pub steps: usize,
    pub outputs: usize,
    pub done_when: usize,
    pub blocked_when: usize,
    pub handoff: usize,
    pub module_bindings: usize,
    pub retirement_allowed: bool,
    pub required_future_gates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationAssessment {
    pub workflow_id: String,
    pub workflow_ref: String,
    pub disposition: WorkflowMigrationDisposition,
    pub authority: WorkflowMigrationAuthority,
    pub legacy_field_counts: LegacyWorkflowFieldCounts,
    pub target_links: WorkflowMigrationTargetLinks,
    pub parity: WorkflowShadowParity,
    pub classification_reason: String,
    pub golden_path_selection: Option<WorkflowGoldenPathSelection>,
    pub issues: Vec<WorkflowMigrationIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyWorkflowFieldCounts {
    pub phases: usize,
    pub triggers: usize,
    pub inputs: usize,
    pub steps: usize,
    pub outputs: usize,
    pub done_when: usize,
    pub blocked_when: usize,
    pub handoff: usize,
    pub module_binding: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationTargetLinks {
    pub policy_id: String,
    pub obligation_ids: Vec<String>,
    pub claim_id: String,
    pub playbook_id: String,
    pub evaluator_id: String,
    pub compatibility_workflow_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowShadowParity {
    Equivalent,
    Drift,
    MissingLegacyProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMigrationIssue {
    pub code: WorkflowMigrationIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMigrationIssueCode {
    UnsupportedSchemaVersion,
    InvalidPlan,
    DuplicatePlanReference,
    UnknownWorkflowReference,
    ClassificationOverlap,
    CatalogCountMismatch,
    CatalogDigestMismatch,
    DuplicateWorkflowId,
    WorkflowSchemaVersionMismatch,
    WorkflowShapeInvalid,
    CompatibilityProjectionMissing,
    CompatibilityProjectionDrift,
    CatalogEntryWithoutWorkflow,
}

/// Evaluate the complete P5a migration foundation without IO or mutation.
#[must_use]
pub fn evaluate_workflow_migration(
    plan_document: &WorkflowMigrationPlanDocument,
    workflows: &[LoadedWorkflowDocument],
    legacy_catalog: &Catalog,
) -> WorkflowMigrationAudit {
    let plan = &plan_document.workflow_migration_plan;
    let mut issues = validate_plan(plan_document);
    let mut ordered = workflows.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.document
            .workflow
            .id
            .0
            .cmp(&right.document.workflow.id.0)
    });

    let known_ids = ordered
        .iter()
        .map(|loaded| loaded.document.workflow.id.0.as_str())
        .collect::<BTreeSet<_>>();
    validate_plan_workflow_references(plan_document, &known_ids, &mut issues);
    validate_catalog_shape(
        &ordered,
        legacy_catalog,
        &plan.expected_workflow_schema_version,
        &mut issues,
    );

    if plan.expected_catalog_count != ordered.len() {
        push_issue(
            &mut issues,
            WorkflowMigrationIssueCode::CatalogCountMismatch,
            "workflow_migration_plan.expected_catalog_count",
            format!(
                "plan expects {} workflows but inventory loaded {}",
                plan.expected_catalog_count,
                ordered.len()
            ),
        );
    }

    let golden = plan
        .golden_path_selections
        .iter()
        .map(|selection| (selection.workflow_id.0.as_str(), selection))
        .collect::<BTreeMap<_, _>>();
    let domain = id_set(&plan.domain_pack_candidate_ids);
    let quarantine = plan
        .quarantine
        .iter()
        .map(|entry| (entry.workflow_id.0.as_str(), entry.reason.as_str()))
        .collect::<BTreeMap<_, _>>();
    let legacy_by_id = catalog_by_id(legacy_catalog);
    let mut assessments = Vec::with_capacity(ordered.len());
    for loaded in &ordered {
        assessments.push(assess_workflow(
            loaded,
            plan_document,
            &golden,
            &domain,
            &quarantine,
            &legacy_by_id,
        ));
    }

    let assessed_ids = assessments
        .iter()
        .map(|assessment| assessment.workflow_id.as_str())
        .collect::<BTreeSet<_>>();
    for entry in &legacy_catalog.entries {
        if !assessed_ids.contains(entry.id.0.as_str()) {
            push_issue(
                &mut issues,
                WorkflowMigrationIssueCode::CatalogEntryWithoutWorkflow,
                format!("catalog.{}", entry.id.0),
                "legacy catalog entry has no corresponding workflow document",
            );
        }
    }

    let golden_path_count =
        count_disposition(&assessments, WorkflowMigrationDisposition::GoldenPath);
    let domain_pack_candidate_count = count_disposition(
        &assessments,
        WorkflowMigrationDisposition::DomainPackCandidate,
    );
    let compatibility_playbook_count = count_disposition(
        &assessments,
        WorkflowMigrationDisposition::CompatibilityPlaybook,
    );
    let quarantined_count =
        count_disposition(&assessments, WorkflowMigrationDisposition::Quarantined);
    let unresolved_count = assessments
        .iter()
        .filter(|assessment| !assessment.issues.is_empty())
        .count();
    let equivalent_count = assessments
        .iter()
        .filter(|assessment| assessment.parity == WorkflowShadowParity::Equivalent)
        .count();
    let drift_count = assessments.len().saturating_sub(equivalent_count);
    let classified_count = assessments.len().saturating_sub(unresolved_count);
    issues.extend(
        assessments
            .iter()
            .flat_map(|assessment| assessment.issues.iter().cloned()),
    );
    let catalog_digest = catalog_digest(&ordered).unwrap_or_else(|message| {
        push_issue(
            &mut issues,
            WorkflowMigrationIssueCode::InvalidPlan,
            "workflow_inventory.digest",
            message,
        );
        "sha256:unavailable".to_owned()
    });
    if catalog_digest != plan.expected_catalog_digest {
        push_issue(
            &mut issues,
            WorkflowMigrationIssueCode::CatalogDigestMismatch,
            "workflow_migration_plan.expected_catalog_digest",
            format!(
                "planned catalog digest {} does not match loaded digest {catalog_digest}",
                plan.expected_catalog_digest
            ),
        );
    }
    let deletion_baseline = deletion_baseline(&ordered, catalog_digest);
    let manifest_digest =
        manifest_digest(&plan.id.0, &deletion_baseline.catalog_digest, &assessments)
            .unwrap_or_else(|message| {
                push_issue(
                    &mut issues,
                    WorkflowMigrationIssueCode::InvalidPlan,
                    "workflow_migration_manifest.digest",
                    message,
                );
                "sha256:unavailable".to_owned()
            });
    let status = if issues.is_empty() && unresolved_count == 0 && drift_count == 0 {
        WorkflowMigrationAuditStatus::ReadyForShadow
    } else {
        WorkflowMigrationAuditStatus::Blocked
    };
    let manifest = WorkflowMigrationManifest {
        schema_version: WORKFLOW_MIGRATION_PLAN_SCHEMA_VERSION.to_owned(),
        plan_id: plan.id.0.clone(),
        catalog_digest: deletion_baseline.catalog_digest.clone(),
        manifest_digest,
        entries: assessments,
    };
    WorkflowMigrationAudit {
        status,
        catalog_count: ordered.len(),
        classified_count,
        unresolved_count,
        golden_path_count,
        domain_pack_candidate_count,
        compatibility_playbook_count,
        quarantined_count,
        shadow_parity: WorkflowShadowParitySummary {
            mode: plan.compatibility_projection.mode,
            mutation_allowed: plan.compatibility_projection.mutation_allowed,
            equivalent_count,
            drift_count,
            exact_fields: plan.compatibility_projection.exact_fields.clone(),
        },
        deletion_baseline,
        manifest,
        issues,
    }
}

fn validate_plan(document: &WorkflowMigrationPlanDocument) -> Vec<WorkflowMigrationIssue> {
    let mut issues = Vec::new();
    let plan = &document.workflow_migration_plan;
    if document.schema_version != WORKFLOW_MIGRATION_PLAN_SCHEMA_VERSION {
        push_issue(
            &mut issues,
            WorkflowMigrationIssueCode::UnsupportedSchemaVersion,
            "schema_version",
            format!("unsupported schema version {}", document.schema_version),
        );
    }
    if plan.id.0.trim().is_empty() || plan.expected_catalog_count == 0 {
        push_issue(
            &mut issues,
            WorkflowMigrationIssueCode::InvalidPlan,
            "workflow_migration_plan",
            "plan id must be non-blank and expected catalog count must be positive",
        );
    }
    if !is_sha256_digest(&plan.expected_catalog_digest) {
        push_issue(
            &mut issues,
            WorkflowMigrationIssueCode::InvalidPlan,
            "workflow_migration_plan.expected_catalog_digest",
            "expected catalog digest must be sha256 followed by 64 lowercase hex characters",
        );
    }
    if plan.expected_workflow_schema_version.trim().is_empty() {
        push_issue(
            &mut issues,
            WorkflowMigrationIssueCode::InvalidPlan,
            "workflow_migration_plan.expected_workflow_schema_version",
            "workflow schema version must be non-blank",
        );
    }
    validate_field_mappings(&plan.field_mappings, &mut issues);
    validate_exact_compatibility_fields(&plan.compatibility_projection.exact_fields, &mut issues);
    if plan.compatibility_projection.mode != WorkflowShadowMode::ReadOnlyExactProjection
        || plan.compatibility_projection.mutation_allowed
    {
        push_issue(
            &mut issues,
            WorkflowMigrationIssueCode::InvalidPlan,
            "workflow_migration_plan.compatibility_projection",
            "P5a requires read_only_exact_projection with mutation_allowed=false",
        );
    }
    let retirement = &plan.retirement_policy;
    let required_retirement_gates = BTreeSet::from([
        WorkflowRetirementGate::ExecutableCoverage,
        WorkflowRetirementGate::ShadowParity,
        WorkflowRetirementGate::DeletionTest,
        WorkflowRetirementGate::HumanReview,
    ]);
    let observed_retirement_gates = retirement
        .required_gates
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if observed_retirement_gates != required_retirement_gates
        || observed_retirement_gates.len() != retirement.required_gates.len()
        || retirement.retirement_allowed_during_foundation
    {
        push_issue(
            &mut issues,
            WorkflowMigrationIssueCode::InvalidPlan,
            "workflow_migration_plan.retirement_policy",
            "P5a requires every retirement gate and forbids retirement during foundation",
        );
    }
    for (field, value) in [
        ("policy", &plan.target_namespaces.policy),
        ("obligation", &plan.target_namespaces.obligation),
        ("claim", &plan.target_namespaces.claim),
        ("playbook", &plan.target_namespaces.playbook),
        ("evaluator", &plan.target_namespaces.evaluator),
    ] {
        if value.trim().is_empty() || value.chars().any(char::is_whitespace) {
            push_issue(
                &mut issues,
                WorkflowMigrationIssueCode::InvalidPlan,
                format!("workflow_migration_plan.target_namespaces.{field}"),
                "target namespace must be non-blank and contain no whitespace",
            );
        }
    }
    issues
}

fn validate_field_mappings(
    mappings: &[LegacyWorkflowFieldMapping],
    issues: &mut Vec<WorkflowMigrationIssue>,
) {
    let expected = [
        (
            LegacyWorkflowField::Phases,
            LegacyWorkflowFieldRole::PhaseProjection,
        ),
        (
            LegacyWorkflowField::Trigger,
            LegacyWorkflowFieldRole::RoutingSignal,
        ),
        (
            LegacyWorkflowField::Inputs,
            LegacyWorkflowFieldRole::InputObservationCandidate,
        ),
        (
            LegacyWorkflowField::Steps,
            LegacyWorkflowFieldRole::AdvisoryPlaybook,
        ),
        (
            LegacyWorkflowField::Outputs,
            LegacyWorkflowFieldRole::ArtifactProjection,
        ),
        (
            LegacyWorkflowField::DoneWhen,
            LegacyWorkflowFieldRole::CompletionClaimCandidate,
        ),
        (
            LegacyWorkflowField::BlockedWhen,
            LegacyWorkflowFieldRole::BlockingGapCandidate,
        ),
        (
            LegacyWorkflowField::Handoff,
            LegacyWorkflowFieldRole::ContinuityProjection,
        ),
        (
            LegacyWorkflowField::Module,
            LegacyWorkflowFieldRole::GroupingProjection,
        ),
    ];
    let mut seen = BTreeMap::new();
    for mapping in mappings {
        if seen.insert(mapping.field, mapping.role).is_some() {
            push_issue(
                issues,
                WorkflowMigrationIssueCode::DuplicatePlanReference,
                format!("workflow_migration_plan.field_mappings.{:?}", mapping.field),
                "legacy workflow field is mapped more than once",
            );
        }
    }
    for (field, role) in expected {
        if seen.get(&field) != Some(&role) {
            push_issue(
                issues,
                WorkflowMigrationIssueCode::InvalidPlan,
                format!("workflow_migration_plan.field_mappings.{field:?}"),
                format!("field must map exactly to {role:?}"),
            );
        }
    }
}

fn validate_exact_compatibility_fields(
    fields: &[WorkflowCompatibilityField],
    issues: &mut Vec<WorkflowMigrationIssue>,
) {
    let expected = BTreeSet::from([
        WorkflowCompatibilityField::Id,
        WorkflowCompatibilityField::Phases,
        WorkflowCompatibilityField::WorkflowRef,
        WorkflowCompatibilityField::Triggers,
        WorkflowCompatibilityField::Prerequisites,
        WorkflowCompatibilityField::Outputs,
    ]);
    let observed = fields.iter().copied().collect::<BTreeSet<_>>();
    if observed.len() != fields.len() || observed != expected {
        push_issue(
            issues,
            WorkflowMigrationIssueCode::InvalidPlan,
            "workflow_migration_plan.compatibility_projection.exact_fields",
            "compatibility projection must contain every legacy catalog field exactly once",
        );
    }
}

fn validate_plan_workflow_references(
    document: &WorkflowMigrationPlanDocument,
    known_ids: &BTreeSet<&str>,
    issues: &mut Vec<WorkflowMigrationIssue>,
) {
    let plan = &document.workflow_migration_plan;
    let mut seen = BTreeMap::<&str, &'static str>::new();
    for (group, id) in plan
        .golden_path_selections
        .iter()
        .map(|selection| ("golden_path", &selection.workflow_id))
        .chain(
            plan.domain_pack_candidate_ids
                .iter()
                .map(|id| ("domain_pack_candidate", id)),
        )
        .chain(
            plan.quarantine
                .iter()
                .map(|entry| ("quarantine", &entry.workflow_id)),
        )
    {
        if !known_ids.contains(id.0.as_str()) {
            push_issue(
                issues,
                WorkflowMigrationIssueCode::UnknownWorkflowReference,
                format!("workflow_migration_plan.{group}.{}", id.0),
                "plan references a workflow absent from the complete inventory",
            );
        }
        if let Some(previous) = seen.insert(id.0.as_str(), group) {
            push_issue(
                issues,
                if previous == group {
                    WorkflowMigrationIssueCode::DuplicatePlanReference
                } else {
                    WorkflowMigrationIssueCode::ClassificationOverlap
                },
                format!("workflow_migration_plan.{group}.{}", id.0),
                format!("workflow is already classified in {previous}"),
            );
        }
    }
    for (index, entry) in plan.quarantine.iter().enumerate() {
        if entry.reason.trim().is_empty() {
            push_issue(
                issues,
                WorkflowMigrationIssueCode::InvalidPlan,
                format!("workflow_migration_plan.quarantine[{index}].reason"),
                "quarantine reason must be non-blank",
            );
        }
    }
    for (index, selection) in plan.golden_path_selections.iter().enumerate() {
        let unique_coverage = selection.coverage.iter().collect::<BTreeSet<_>>();
        if selection.rationale.trim().is_empty()
            || selection.coverage.is_empty()
            || unique_coverage.len() != selection.coverage.len()
        {
            push_issue(
                issues,
                WorkflowMigrationIssueCode::InvalidPlan,
                format!("workflow_migration_plan.golden_path_selections[{index}]"),
                "golden-path selection requires a rationale and non-empty unique coverage areas",
            );
        }
    }
}

fn validate_catalog_shape(
    workflows: &[&LoadedWorkflowDocument],
    catalog: &Catalog,
    expected_schema_version: &str,
    issues: &mut Vec<WorkflowMigrationIssue>,
) {
    let mut workflow_ids = BTreeSet::new();
    for loaded in workflows {
        let workflow = &loaded.document.workflow;
        if !workflow_ids.insert(workflow.id.0.as_str()) {
            push_issue(
                issues,
                WorkflowMigrationIssueCode::DuplicateWorkflowId,
                format!("workflow.{}", workflow.id.0),
                "workflow id occurs more than once",
            );
        }
        if loaded.document.schema_version != expected_schema_version {
            push_issue(
                issues,
                WorkflowMigrationIssueCode::WorkflowSchemaVersionMismatch,
                format!("{}.schema_version", loaded.workflow_ref.0),
                format!(
                    "workflow schema version {} does not match planned version {expected_schema_version}",
                    loaded.document.schema_version
                ),
            );
        }
        if loaded.document.schema_version.trim().is_empty()
            || workflow.id.0.trim().is_empty()
            || workflow.trigger.is_empty()
            || workflow.steps.is_empty()
            || workflow.done_when.is_empty()
        {
            push_issue(
                issues,
                WorkflowMigrationIssueCode::WorkflowShapeInvalid,
                loaded.workflow_ref.0.clone(),
                "workflow must have schema version, id, trigger, steps, and done_when",
            );
        }
    }
    let mut catalog_ids = BTreeSet::new();
    for entry in &catalog.entries {
        if !catalog_ids.insert(entry.id.0.as_str()) {
            push_issue(
                issues,
                WorkflowMigrationIssueCode::DuplicateWorkflowId,
                format!("catalog.{}", entry.id.0),
                "legacy catalog id occurs more than once",
            );
        }
    }
}

fn assess_workflow(
    loaded: &LoadedWorkflowDocument,
    plan_document: &WorkflowMigrationPlanDocument,
    golden: &BTreeMap<&str, &WorkflowGoldenPathSelection>,
    domain: &BTreeSet<&str>,
    quarantine: &BTreeMap<&str, &str>,
    legacy_by_id: &BTreeMap<&str, &CatalogEntry>,
) -> WorkflowMigrationAssessment {
    let workflow = &loaded.document.workflow;
    let id = workflow.id.0.as_str();
    let golden_path_selection = golden.get(id).map(|selection| (*selection).clone());
    let (disposition, reason) = if let Some(reason) = quarantine.get(id) {
        (
            WorkflowMigrationDisposition::Quarantined,
            (*reason).to_owned(),
        )
    } else if golden_path_selection.is_some() {
        (
            WorkflowMigrationDisposition::GoldenPath,
            "selected for representative agent-native golden-path coverage".to_owned(),
        )
    } else if domain.contains(id) {
        (
            WorkflowMigrationDisposition::DomainPackCandidate,
            "domain-specific knowledge candidate; preserve as compatibility until P6".to_owned(),
        )
    } else {
        (
            WorkflowMigrationDisposition::CompatibilityPlaybook,
            "preserved as a non-authoritative compatibility playbook during incremental migration"
                .to_owned(),
        )
    };
    let projected = catalog_entry(loaded);
    let mut assessment_issues = Vec::new();
    let parity = match legacy_by_id.get(id) {
        None => {
            push_issue(
                &mut assessment_issues,
                WorkflowMigrationIssueCode::CompatibilityProjectionMissing,
                format!("workflow.{id}.compatibility_projection"),
                "legacy catalog has no entry for workflow",
            );
            WorkflowShadowParity::MissingLegacyProjection
        }
        Some(legacy) if **legacy == projected => WorkflowShadowParity::Equivalent,
        Some(_) => {
            push_issue(
                &mut assessment_issues,
                WorkflowMigrationIssueCode::CompatibilityProjectionDrift,
                format!("workflow.{id}.compatibility_projection"),
                "derived shadow projection differs from the current legacy catalog entry",
            );
            WorkflowShadowParity::Drift
        }
    };
    WorkflowMigrationAssessment {
        workflow_id: id.to_owned(),
        workflow_ref: loaded.workflow_ref.0.clone(),
        disposition,
        authority: WorkflowMigrationAuthority::LegacyCompatibilityOnly,
        legacy_field_counts: field_counts(workflow),
        target_links: target_links(plan_document, id),
        parity,
        classification_reason: reason,
        golden_path_selection,
        issues: assessment_issues,
    }
}

fn catalog_entry(loaded: &LoadedWorkflowDocument) -> CatalogEntry {
    let workflow = &loaded.document.workflow;
    CatalogEntry {
        id: workflow.id.clone(),
        phases: workflow.phases.clone(),
        workflow_ref: loaded.workflow_ref.clone(),
        triggers: workflow.trigger.clone(),
        prerequisites: workflow.inputs.clone(),
        outputs: workflow.outputs.clone(),
    }
}

fn field_counts(workflow: &Workflow) -> LegacyWorkflowFieldCounts {
    LegacyWorkflowFieldCounts {
        phases: workflow.phases.len(),
        triggers: workflow.trigger.len(),
        inputs: workflow.inputs.len(),
        steps: workflow.steps.len(),
        outputs: workflow.outputs.len(),
        done_when: workflow.done_when.len(),
        blocked_when: workflow.blocked_when.len(),
        handoff: workflow.handoff.len(),
        module_binding: usize::from(workflow.module.is_some()),
    }
}

fn target_links(
    plan_document: &WorkflowMigrationPlanDocument,
    workflow_id: &str,
) -> WorkflowMigrationTargetLinks {
    let namespaces = &plan_document.workflow_migration_plan.target_namespaces;
    WorkflowMigrationTargetLinks {
        policy_id: format!("{}{workflow_id}", namespaces.policy),
        obligation_ids: vec![
            format!("{}{workflow_id}.inputs", namespaces.obligation),
            format!("{}{workflow_id}.completion", namespaces.obligation),
            format!("{}{workflow_id}.blockers", namespaces.obligation),
        ],
        claim_id: format!("{}{workflow_id}.complete", namespaces.claim),
        playbook_id: format!("{}{workflow_id}", namespaces.playbook),
        evaluator_id: format!("{}{workflow_id}.completion", namespaces.evaluator),
        compatibility_workflow_id: workflow_id.to_owned(),
    }
}

fn catalog_by_id(catalog: &Catalog) -> BTreeMap<&str, &CatalogEntry> {
    catalog
        .entries
        .iter()
        .map(|entry| (entry.id.0.as_str(), entry))
        .collect()
}

fn id_set(ids: &[StableId]) -> BTreeSet<&str> {
    ids.iter().map(|id| id.0.as_str()).collect()
}

fn count_disposition(
    assessments: &[WorkflowMigrationAssessment],
    disposition: WorkflowMigrationDisposition,
) -> usize {
    assessments
        .iter()
        .filter(|assessment| assessment.disposition == disposition)
        .count()
}

fn catalog_digest(workflows: &[&LoadedWorkflowDocument]) -> Result<String, String> {
    let canonical = serde_json_canonicalizer::to_vec(&workflows)
        .map_err(|error| format!("cannot canonicalize workflow inventory: {error}"))?;
    let digest = Sha256::digest(canonical);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(hex, "{byte:02x}").map_err(|error| error.to_string())?;
    }
    Ok(format!("sha256:{hex}"))
}

fn manifest_digest(
    plan_id: &str,
    catalog_digest: &str,
    entries: &[WorkflowMigrationAssessment],
) -> Result<String, String> {
    #[derive(Serialize)]
    struct ManifestDigestInput<'a> {
        schema_version: &'a str,
        plan_id: &'a str,
        catalog_digest: &'a str,
        entries: &'a [WorkflowMigrationAssessment],
    }

    let canonical = serde_json_canonicalizer::to_vec(&ManifestDigestInput {
        schema_version: WORKFLOW_MIGRATION_PLAN_SCHEMA_VERSION,
        plan_id,
        catalog_digest,
        entries,
    })
    .map_err(|error| format!("cannot canonicalize workflow migration manifest: {error}"))?;
    let digest = Sha256::digest(canonical);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(hex, "{byte:02x}").map_err(|error| error.to_string())?;
    }
    Ok(format!("sha256:{hex}"))
}

fn deletion_baseline(
    workflows: &[&LoadedWorkflowDocument],
    catalog_digest: String,
) -> WorkflowDeletionBaseline {
    let mut baseline = WorkflowDeletionBaseline {
        catalog_digest,
        phases: 0,
        triggers: 0,
        inputs: 0,
        steps: 0,
        outputs: 0,
        done_when: 0,
        blocked_when: 0,
        handoff: 0,
        module_bindings: 0,
        retirement_allowed: false,
        required_future_gates: vec![
            "executable_coverage".to_owned(),
            "shadow_parity".to_owned(),
            "deletion_test".to_owned(),
            "human_review".to_owned(),
        ],
    };
    for loaded in workflows {
        let counts = field_counts(&loaded.document.workflow);
        baseline.phases += counts.phases;
        baseline.triggers += counts.triggers;
        baseline.inputs += counts.inputs;
        baseline.steps += counts.steps;
        baseline.outputs += counts.outputs;
        baseline.done_when += counts.done_when;
        baseline.blocked_when += counts.blocked_when;
        baseline.handoff += counts.handoff;
        baseline.module_bindings += counts.module_binding;
    }
    baseline
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn push_issue(
    issues: &mut Vec<WorkflowMigrationIssue>,
    code: WorkflowMigrationIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(WorkflowMigrationIssue {
        code,
        path: path.into(),
        message: message.into(),
    });
}
