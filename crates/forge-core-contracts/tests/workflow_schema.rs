//! S1.2 acceptance test: workflow/phase/catalog types round-trip cleanly.
//!
//! Proves the schema the 110 workflows deserialize into is sound before
//! the catalog migration (S1.3-S1.6) begins.
use forge_core_contracts::{
    Catalog, CatalogDocument, CatalogEntry, Phase, RepoPath, StableId, WorkflowDocument,
};

const SAMPLE_PLAN_SPRINT_YAML: &str = r#"schema_version: "0.1"
workflow:
  id: plan-sprint
  phases:
    - 3-plan
  trigger:
    - state.phase == 3-plan
    - specification artifact exists
  inputs:
    - specification artifact
    - approved decision sources
  steps:
    - verify accepted sources before sequencing work
    - split acceptance criteria into story batches
    - move only implementation-ready work into phase 4
  outputs:
    - sprint plan artifact
    - ordered story batch
  done_when:
    - each executable story has acceptance criteria and decision sources
  blocked_when:
    - approved decision sources are missing
  handoff:
    - preserve story order and next story
"#;

const WORKFLOW_MISSING_PHASE_YAML: &str = r#"schema_version: "0.1"
workflow:
  id: unassigned-workflow
  trigger:
    - some condition
"#;

#[test]
fn workflow_round_trips_sample_plan_sprint() {
    let doc: WorkflowDocument =
        yaml_serde::from_str(SAMPLE_PLAN_SPRINT_YAML).expect("deserialize plan-sprint");
    assert_eq!(doc.workflow.id, StableId("plan-sprint".into()));
    assert_eq!(doc.workflow.phases, vec![StableId("3-plan".into())]);
    assert_eq!(doc.workflow.steps.len(), 3);
    assert_eq!(doc.workflow.outputs.len(), 2);

    // serialize -> deserialize -> equal (acceptance criterion).
    let again = yaml_serde::to_string(&doc).expect("serialize");
    let doc2: WorkflowDocument = yaml_serde::from_str(&again).expect("deserialize again");
    assert_eq!(doc, doc2, "round-trip not stable");
}

#[test]
fn workflow_without_phases_deserializes_to_empty_default() {
    // Workflows may carry no phase tags yet. They must still deserialize
    // cleanly, with phases defaulting to an empty set.
    let doc: WorkflowDocument =
        yaml_serde::from_str(WORKFLOW_MISSING_PHASE_YAML).expect("deserialize no-phase");
    assert_eq!(doc.workflow.id, StableId("unassigned-workflow".into()));
    assert!(doc.workflow.phases.is_empty());
    assert!(!doc.workflow.trigger.is_empty());
    assert!(doc.workflow.steps.is_empty());
}

#[test]
fn workflow_phase_tags_categorize_via_phase_parse() {
    let doc: WorkflowDocument =
        yaml_serde::from_str(SAMPLE_PLAN_SPRINT_YAML).expect("deserialize plan-sprint");
    // Every phase tag should categorize to a known Phase.
    let parsed: Vec<Phase> = doc
        .workflow
        .phases
        .iter()
        .filter_map(|t| Phase::parse(&t.0))
        .collect();
    assert_eq!(parsed, vec![Phase::Plan]);
    assert_eq!(parsed[0].rank(), 3);
}

#[test]
fn catalog_round_trips_and_finds_entry() {
    let cat_yaml = r#"schema_version: "0.1"
catalog:
  entries:
    - id: plan-sprint
      phases:
        - 3-plan
      workflow_ref: contracts/workflows/plan-sprint.yaml
      triggers:
        - state.phase == 3-plan
      prerequisites:
        - specification artifact exists
      outputs:
        - sprint plan artifact
"#;
    let doc: CatalogDocument = yaml_serde::from_str(cat_yaml).expect("deserialize catalog");
    assert_eq!(doc.catalog.len(), 1);
    let entry = doc.catalog.find("plan-sprint").expect("entry present");
    assert_eq!(
        entry.workflow_ref,
        RepoPath("contracts/workflows/plan-sprint.yaml".into())
    );
    assert_eq!(entry.triggers.len(), 1);
    assert_eq!(entry.phases, vec![StableId("3-plan".into())]);

    // round-trip
    let again = yaml_serde::to_string(&doc).expect("serialize catalog");
    let doc2: CatalogDocument = yaml_serde::from_str(&again).expect("deserialize again");
    assert_eq!(doc, doc2);
}

#[test]
fn catalog_entry_derives_from_workflow_fields() {
    // Validates the intended catalog-build relationship: an entry is a
    // routing-flattened view of a workflow. This is the shape slice 2's
    // orchestrator will consume.
    let wf_doc: WorkflowDocument =
        yaml_serde::from_str(SAMPLE_PLAN_SPRINT_YAML).expect("deserialize plan-sprint");
    let wf = &wf_doc.workflow;
    let entry = CatalogEntry {
        id: wf.id.clone(),
        phases: wf.phases.clone(),
        workflow_ref: RepoPath("contracts/workflows/plan-sprint.yaml".into()),
        triggers: wf.trigger.clone(),
        prerequisites: wf.inputs.clone(),
        outputs: wf.outputs.clone(),
    };
    let cat = Catalog {
        entries: vec![entry],
    };
    assert!(cat.find("plan-sprint").is_some());
}
