//! S1.3 acceptance test: bulk-deserialize all 110 migrated workflow YAML files
//! into the typed `WorkflowDocument`. Proves schema + migration are correct
//! together. Any file that fails to deserialize (unknown field, bad type) fails
//! this test by name, so regressions are localized.
//!
//! The workflow files live in the workspace root `contracts/workflows/`,
//! reachable from this crate's tests as `../../contracts/workflows/`.
use forge_core_contracts::{Phase, WorkflowDocument};
use std::fs;
use std::path::{Path, PathBuf};

fn workflows_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/workflows")
        .canonicalize()
        .expect("contracts/workflows dir must exist (run scripts/migrate_workflows.py)")
}

#[test]
fn all_migrated_workflows_deserialize_into_typed_schema() {
    let dir = workflows_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {dir:?}: {e}"))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "yaml"))
        .collect();
    assert!(
        !files.is_empty(),
        "no workflow YAML found under {dir:?} — did the migration run?"
    );
    files.sort();

    let mut deserialized = 0usize;
    let mut failures = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        match serde_yaml::from_str::<WorkflowDocument>(&text) {
            Ok(doc) => {
                // Sanity: every workflow must have a non-empty id and all 7 fields.
                assert!(
                    !doc.workflow.id.0.is_empty(),
                    "{}: empty workflow id",
                    path.file_name().unwrap().to_string_lossy()
                );
                // Every workflow is a compact state machine: it must declare at least
                // one trigger, one step, and one done_when, else it carries no method.
                assert!(
                    !doc.workflow.trigger.is_empty(),
                    "{}: workflow has no trigger conditions",
                    path.file_name().unwrap().to_string_lossy()
                );
                assert!(
                    !doc.workflow.steps.is_empty(),
                    "{}: workflow has no directional steps",
                    path.file_name().unwrap().to_string_lossy()
                );
                assert!(
                    !doc.workflow.done_when.is_empty(),
                    "{}: workflow has no done_when conditions",
                    path.file_name().unwrap().to_string_lossy()
                );
                deserialized += 1;
            }
            Err(e) => failures.push(format!(
                "{}: {e}",
                path.file_name().unwrap().to_string_lossy()
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "deserialization failures:\n{}",
        failures.join("\n")
    );
    assert_eq!(deserialized, files.len(), "count mismatch");
    // The catalog is exactly 110 workflows. Pin this so a partial migration is caught.
    assert_eq!(
        files.len(),
        110,
        "expected exactly 110 workflow files, got {}",
        files.len()
    );
}

#[test]
fn all_phase_tags_are_canonical_or_anytime() {
    // S1.5 soundness: every phase tag injected from the authoritative catalog
    // must be EITHER a parseable canonical Phase OR the "anytime" wildcard.
    // Catches typos / garbage in the phase mapping.
    let dir = workflows_dir();
    let mut bad_tags = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "yaml") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let doc: WorkflowDocument = serde_yaml::from_str(&text).unwrap();
        for tag in &doc.workflow.phases {
            let raw = tag.0.trim();
            if raw == "anytime" {
                continue;
            }
            if Phase::parse(raw).is_none() {
                bad_tags.push(format!(
                    "{}: unparsable phase tag {raw:?}",
                    path.file_name().unwrap().to_string_lossy()
                ));
            }
        }
    }
    assert!(
        bad_tags.is_empty(),
        "bad phase tags:\n{}",
        bad_tags.join("\n")
    );
}

#[test]
fn workflow_catalog_has_unique_ids() {
    let dir = workflows_dir();
    let mut ids = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "yaml") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let doc: WorkflowDocument = serde_yaml::from_str(&text)
            .unwrap_or_else(|e| panic!("{}: {e}", path.file_name().unwrap().to_string_lossy()));
        ids.push(doc.workflow.id.0);
    }
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        ids.len(),
        "duplicate workflow ids detected: {ids:?}"
    );
}
