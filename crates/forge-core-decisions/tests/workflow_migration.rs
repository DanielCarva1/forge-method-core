use forge_core_contracts::{WorkflowMigrationDisposition, WorkflowMigrationPlanDocument};
use forge_core_decisions::{
    evaluate_workflow_migration, load_catalog, load_workflow_documents,
    WorkflowMigrationAuditStatus, WorkflowMigrationIssueCode, WorkflowShadowParity,
};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_plan() -> WorkflowMigrationPlanDocument {
    yaml_serde::from_str(
        &std::fs::read_to_string(
            repo_root().join("contracts/policies/workflow-migration-foundation-v0.yaml"),
        )
        .expect("migration plan"),
    )
    .expect("typed migration plan")
}

fn evaluate(plan: &WorkflowMigrationPlanDocument) -> forge_core_decisions::WorkflowMigrationAudit {
    let catalog_dir = repo_root().join("contracts/evidence/workflow-retirement/legacy-catalog");
    let workflows = load_workflow_documents(&catalog_dir);
    assert!(
        workflows.is_clean(),
        "workflow load: {:?}",
        workflows.errors
    );
    let catalog = load_catalog(&catalog_dir);
    assert!(catalog.is_clean(), "catalog load: {:?}", catalog.errors);
    evaluate_workflow_migration(plan, &workflows.workflows, &catalog.catalog)
}

#[test]
fn p5a_classifies_complete_catalog_and_proves_exact_shadow_parity() {
    let audit = evaluate(&load_plan());
    assert_eq!(audit.status, WorkflowMigrationAuditStatus::ReadyForShadow);
    assert_eq!(audit.catalog_count, 110);
    assert_eq!(audit.classified_count, 110);
    assert_eq!(audit.unresolved_count, 0);
    assert_eq!(audit.golden_path_count, 15);
    assert_eq!(audit.domain_pack_candidate_count, 18);
    assert_eq!(audit.compatibility_playbook_count, 77);
    assert_eq!(audit.quarantined_count, 0);
    assert_eq!(audit.shadow_parity.equivalent_count, 110);
    assert_eq!(audit.shadow_parity.drift_count, 0);
    assert!(!audit.shadow_parity.mutation_allowed);
    assert!(!audit.deletion_baseline.retirement_allowed);
    assert!(audit
        .deletion_baseline
        .catalog_digest
        .starts_with("sha256:"));
    assert_eq!(audit.deletion_baseline.catalog_digest.len(), 71);
    assert!(audit.issues.is_empty());

    let discover = audit
        .manifest
        .entries
        .iter()
        .find(|assessment| assessment.workflow_id == "discover-intent")
        .expect("discover-intent assessment");
    assert_eq!(
        discover.disposition,
        WorkflowMigrationDisposition::GoldenPath
    );
    assert_eq!(discover.parity, WorkflowShadowParity::Equivalent);
    let selection = discover
        .golden_path_selection
        .as_ref()
        .expect("golden-path rationale");
    assert_eq!(
        selection.coverage,
        vec![forge_core_contracts::WorkflowGoldenPathCoverage::Intent]
    );
    assert!(!selection.rationale.trim().is_empty());
    assert_eq!(
        discover.target_links.policy_id,
        "policy.workflow.discover-intent"
    );
    assert_eq!(
        discover.target_links.playbook_id,
        "playbook.workflow.discover-intent"
    );

    let game = audit
        .manifest
        .entries
        .iter()
        .find(|assessment| assessment.workflow_id == "gdd")
        .expect("gdd assessment");
    assert_eq!(
        game.disposition,
        WorkflowMigrationDisposition::DomainPackCandidate
    );
}

#[test]
fn p5a_audit_is_byte_deterministic_and_independent_of_input_order() {
    let plan = load_plan();
    let catalog_dir = repo_root().join("contracts/evidence/workflow-retirement/legacy-catalog");
    let mut workflows = load_workflow_documents(&catalog_dir).workflows;
    let catalog = load_catalog(&catalog_dir).catalog;
    let first = evaluate_workflow_migration(&plan, &workflows, &catalog);
    workflows.reverse();
    let second = evaluate_workflow_migration(&plan, &workflows, &catalog);
    assert_eq!(
        serde_json::to_vec(&first).expect("first JSON"),
        serde_json::to_vec(&second).expect("second JSON")
    );
    assert_eq!(first.manifest.manifest_digest.len(), 71);
}

#[test]
fn p5a_blocks_unknown_overlap_and_unsafe_retirement_policy() {
    let mut plan = load_plan();
    plan.workflow_migration_plan.golden_path_selections.push(
        forge_core_contracts::WorkflowGoldenPathSelection {
            workflow_id: forge_core_contracts::StableId("unknown-workflow".to_owned()),
            leverage: forge_core_contracts::WorkflowSelectionTier::High,
            risk: forge_core_contracts::WorkflowSelectionTier::High,
            coverage: vec![forge_core_contracts::WorkflowGoldenPathCoverage::Intent],
            rationale: "adversarial unknown workflow".to_owned(),
        },
    );
    plan.workflow_migration_plan
        .domain_pack_candidate_ids
        .push(forge_core_contracts::StableId("discover-intent".to_owned()));
    plan.workflow_migration_plan
        .retirement_policy
        .retirement_allowed_during_foundation = true;
    let audit = evaluate(&plan);
    assert_eq!(audit.status, WorkflowMigrationAuditStatus::Blocked);
    assert!(audit
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowMigrationIssueCode::UnknownWorkflowReference));
    assert!(audit
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowMigrationIssueCode::ClassificationOverlap));
    assert!(audit
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowMigrationIssueCode::InvalidPlan));
}

#[test]
fn p5a_detects_legacy_projection_drift_without_mutation() {
    let plan = load_plan();
    let catalog_dir = repo_root().join("contracts/evidence/workflow-retirement/legacy-catalog");
    let workflows = load_workflow_documents(&catalog_dir);
    let mut catalog = load_catalog(&catalog_dir).catalog;
    let entry = catalog
        .entries
        .iter_mut()
        .find(|entry| entry.id.0 == "discover-intent")
        .expect("discover-intent catalog entry");
    entry.triggers.push("invented legacy drift".to_owned());
    let audit = evaluate_workflow_migration(&plan, &workflows.workflows, &catalog);
    assert_eq!(audit.status, WorkflowMigrationAuditStatus::Blocked);
    assert_eq!(audit.shadow_parity.drift_count, 1);
    let drift = audit
        .manifest
        .entries
        .iter()
        .find(|assessment| assessment.workflow_id == "discover-intent")
        .expect("drifted assessment");
    assert_eq!(drift.parity, WorkflowShadowParity::Drift);
    assert_eq!(
        drift.issues[0].code,
        WorkflowMigrationIssueCode::CompatibilityProjectionDrift
    );
    assert!(!audit.shadow_parity.mutation_allowed);
}

#[test]
fn p5a_deletion_baseline_detects_loss_outside_legacy_projection() {
    let plan = load_plan();
    let catalog_dir = repo_root().join("contracts/evidence/workflow-retirement/legacy-catalog");
    let mut workflows = load_workflow_documents(&catalog_dir).workflows;
    let catalog = load_catalog(&catalog_dir).catalog;
    let baseline = evaluate_workflow_migration(&plan, &workflows, &catalog);
    let workflow = workflows
        .iter_mut()
        .find(|loaded| loaded.document.workflow.id.0 == "discover-intent")
        .expect("discover-intent workflow");
    assert!(
        workflow.document.workflow.steps.len() > 1,
        "fixture must preserve a valid non-empty shape after deletion"
    );
    workflow.document.workflow.steps.pop();

    let audit = evaluate_workflow_migration(&plan, &workflows, &catalog);
    assert_eq!(audit.status, WorkflowMigrationAuditStatus::Blocked);
    assert_eq!(audit.shadow_parity.drift_count, 0);
    assert_eq!(
        audit.deletion_baseline.steps + 1,
        baseline.deletion_baseline.steps
    );
    assert_ne!(
        audit.deletion_baseline.catalog_digest,
        baseline.deletion_baseline.catalog_digest
    );
    assert!(audit
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowMigrationIssueCode::CatalogDigestMismatch));
}

#[test]
fn p5a_rejects_workflow_schema_drift() {
    let plan = load_plan();
    let catalog_dir = repo_root().join("contracts/evidence/workflow-retirement/legacy-catalog");
    let mut workflows = load_workflow_documents(&catalog_dir).workflows;
    let catalog = load_catalog(&catalog_dir).catalog;
    workflows[0].document.schema_version = "9.9".to_owned();

    let audit = evaluate_workflow_migration(&plan, &workflows, &catalog);
    assert_eq!(audit.status, WorkflowMigrationAuditStatus::Blocked);
    assert!(audit
        .issues
        .iter()
        .any(|issue| issue.code == WorkflowMigrationIssueCode::WorkflowSchemaVersionMismatch));
}

#[test]
fn published_plan_and_catalog_paths_exist() {
    for path in [
        "contracts/policies/workflow-migration-foundation-v0.yaml",
        "contracts/spec/workflow-migration-foundation-v0.yaml",
        "contracts/evidence/workflow-retirement/legacy-catalog",
    ] {
        assert!(
            Path::new(&repo_root().join(path)).exists(),
            "missing {path}"
        );
    }
}
