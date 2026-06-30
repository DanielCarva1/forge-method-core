//! Catalog loading and phase-eligibility filtering.
//!
//! The engine loads the typed workflow catalog (the 110 `contracts/workflows/
//! *.yaml` documents produced by the S1.3 migration) into a [`Catalog`] at
//! runtime, then filters it to the workflows eligible in the project's current
//! phase. The host LLM reasons over the ELIGIBLE subset (not all 110) — this is
//! the routing substrate (DC1).
//!
//! ## Error model (accumulator, not short-circuit)
//!
//! Per the project's Diagnostic/ValidationReport convention, [`load_catalog`]
//! loads EVERY file and collects per-file errors into [`CatalogLoadReport`];
//! it does not abort on the first bad file. A non-empty `errors` vector means
//! the (partial) catalog is unusable and the caller decides how to surface it.

use forge_core_contracts::phase::Phase;
use forge_core_contracts::{Catalog, CatalogEntry, RepoPath, StableId, WorkflowDocument};
use std::fs;
use std::path::Path;

/// A per-file load error. `path` is repo-relative; `reason` is human-readable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogFileError {
    pub path: RepoPath,
    pub reason: String,
}

/// The accumulator result of loading a catalog directory: the successfully
/// parsed entries plus any per-file errors. `errors` empty => fully loaded.
#[derive(Debug, Clone, Default)]
pub struct CatalogLoadReport {
    pub catalog: Catalog,
    pub errors: Vec<CatalogFileError>,
}

impl CatalogLoadReport {
    /// True if every file parsed cleanly.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Load every `*.yaml` workflow document under `dir` into a typed [`Catalog`].
///
/// Deterministic and non-short-circuiting: a malformed file is recorded in
/// `report.errors` and skipped; the rest still load. The caller checks
/// [`CatalogLoadReport::is_clean`].
#[must_use]
pub fn load_catalog(dir: &Path) -> CatalogLoadReport {
    let mut report = CatalogLoadReport::default();
    let Ok(entries) = fs::read_dir(dir) else {
        report.errors.push(CatalogFileError {
            path: RepoPath(dir.to_string_lossy().into_owned()),
            reason: "catalog directory not readable".into(),
        });
        return report;
    };

    let mut files = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "yaml") {
                    files.push(path);
                }
            }
            Err(error) => report.errors.push(CatalogFileError {
                path: RepoPath(dir.to_string_lossy().into_owned()),
                reason: format!("catalog directory entry read error: {error}"),
            }),
        }
    }
    files.sort();

    for path in files {
        let rel = path.file_name().map_or_else(
            || path.to_string_lossy().into_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        match load_one(&path, dir) {
            Ok(entry) => report.catalog.entries.push(entry),
            Err(reason) => report.errors.push(CatalogFileError {
                path: RepoPath(rel),
                reason,
            }),
        }
    }

    report
}

fn load_one(path: &Path, dir: &Path) -> Result<CatalogEntry, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    // Resolve a stable repo-relative reference (`contracts/workflows/<name>`).
    let workflow_ref = path.strip_prefix(dir).map_or_else(
        |_| path.to_string_lossy().into_owned(),
        |rel| rel.to_string_lossy().into_owned(),
    );
    parse_workflow_yaml(&workflow_ref, &text)
}

/// Parse a single workflow YAML document from its text. Shared by the disk
/// loader ([`load_one`]) and the embedded loader ([`load_embedded_catalog`])
/// so both paths produce identical [`CatalogEntry`]s.
fn parse_workflow_yaml(workflow_ref: &str, text: &str) -> Result<CatalogEntry, String> {
    let doc: WorkflowDocument =
        yaml_serde::from_str(text).map_err(|e| format!("deserialize error: {e}"))?;
    let wf = doc.workflow;
    Ok(CatalogEntry {
        id: wf.id,
        phases: wf.phases,
        workflow_ref: RepoPath(format!("contracts/workflows/{workflow_ref}")),
        triggers: wf.trigger,
        prerequisites: wf.inputs,
        outputs: wf.outputs,
    })
}

// ============================================================================
// Embedded catalog — the 110 workflow documents compiled INTO the binary via
// `include_dir!`. This is what makes forge-core work zero-config on any
// machine: a freshly `cargo install`ed binary carries its full workflow
// catalog, so greenfield projects (no local `contracts/workflows/` tree) can
// still run `guide status`/`describe`/`decide` without a `--catalog-dir`.
// A local `contracts/workflows/` directory or an explicit `--catalog-dir`
// still overrides the embedded set for projects that ship custom workflows.
// ============================================================================
use include_dir::{include_dir, Dir, DirEntry};

static EMBEDDED_WORKFLOWS: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/../../contracts/workflows");

/// Load the catalog from the workflows compiled into the binary.
///
/// Produces [`CatalogEntry`]s identical to [`load_catalog`] on the shipped
/// `contracts/workflows/` tree, so callers can swap between disk and embedded
/// sources transparently.
///
/// # Panics
/// Never in practice: the embedded dir is a compile-time constant that always
/// exists (the build fails if `contracts/workflows/` is missing).
#[must_use]
pub fn load_embedded_catalog() -> CatalogLoadReport {
    let mut report = CatalogLoadReport::default();
    let mut files: Vec<(String, &str)> = Vec::new();
    collect_yaml(&EMBEDDED_WORKFLOWS, &mut files);
    files.sort();
    for (name, text) in &files {
        match parse_workflow_yaml(name, text) {
            Ok(entry) => report.catalog.entries.push(entry),
            Err(reason) => report.errors.push(CatalogFileError {
                path: RepoPath(name.clone()),
                reason,
            }),
        }
    }
    report
}

fn collect_yaml<'a>(dir: &Dir<'a>, out: &mut Vec<(String, &'a str)>) {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(d) => collect_yaml(d, out),
            DirEntry::File(f) => {
                let is_yaml = f.path().extension().is_some_and(|ext| ext == "yaml");
                if is_yaml {
                    let name = f.path().to_string_lossy().into_owned();
                    let text = std::str::from_utf8(f.contents()).unwrap_or("");
                    out.push((name, text));
                }
            }
        }
    }
}

/// Return the catalog entries eligible in `current` (a workflow is eligible if
/// any of its phase tags matches `current` or is the `"anytime"` wildcard).
#[must_use]
pub fn eligible_entries(catalog: &Catalog, current: Phase) -> Vec<&CatalogEntry> {
    catalog
        .entries
        .iter()
        .filter(|e| {
            e.phases
                .iter()
                .any(|tag| Phase::tag_eligible(&tag.0, current))
        })
        .collect()
}

/// Count of eligible entries (convenience for assertions/UI).
#[must_use]
pub fn eligible_count(catalog: &Catalog, current: Phase) -> usize {
    eligible_entries(catalog, current).len()
}

/// Look up an entry by id.
#[must_use]
pub fn find_entry<'a>(catalog: &'a Catalog, id: &StableId) -> Option<&'a CatalogEntry> {
    catalog.find(&id.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::phase::Phase;

    /// The real migrated catalog lives at <workspace>/contracts/workflows.
    fn real_catalog_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/workflows")
            .canonicalize()
            .expect("contracts/workflows must exist (run scripts/migrate_workflows.py)")
    }

    #[test]
    fn embedded_catalog_loads_cleanly_and_matches_disk_count() {
        // Regression for the greenfield blocker: the embedded catalog (compiled
        // into the binary via include_dir!) must load with zero errors and
        // carry exactly as many workflows as the on-disk catalog. This is what
        // makes forge-core work zero-config on any machine.
        let embedded = load_embedded_catalog();
        assert!(
            embedded.is_clean(),
            "embedded catalog must be clean, got errors: {:?}",
            embedded.errors
        );
        let disk = load_catalog(&real_catalog_dir());
        assert_eq!(
            embedded.catalog.entries.len(),
            disk.catalog.entries.len(),
            "embedded workflow count must equal on-disk count"
        );
        assert!(
            !embedded.catalog.entries.is_empty(),
            "embedded catalog must not be empty"
        );
    }

    #[test]
    fn loads_all_110_workflows_cleanly() {
        let report = load_catalog(&real_catalog_dir());
        assert!(
            report.is_clean(),
            "load errors: {:?}",
            report.errors.iter().map(|e| &e.reason).collect::<Vec<_>>()
        );
        assert_eq!(report.catalog.len(), 110, "expected 110 catalog entries");
    }

    #[test]
    fn eligibility_excludes_wrong_phase_and_includes_anytime() {
        let report = load_catalog(&real_catalog_dir());
        assert!(report.is_clean());

        // Discovery-eligible set must include 'anytime' workflows and
        // 1-discovery-tagged ones, but must NOT include e.g. a 5-ready-only one.
        let disc = eligible_entries(&report.catalog, Phase::Discovery);
        assert!(!disc.is_empty());
        // every returned entry is eligible in Discovery
        for e in &disc {
            let ok = e
                .phases
                .iter()
                .any(|t| Phase::tag_eligible(&t.0, Phase::Discovery));
            assert!(ok, "{} not actually eligible in Discovery", e.id.0);
        }
    }

    #[test]
    fn anytime_workflow_is_eligible_in_every_phase() {
        let report = load_catalog(&real_catalog_dir());
        assert!(report.is_clean());
        let anytime: Vec<&CatalogEntry> = report
            .catalog
            .entries
            .iter()
            .filter(|e| e.phases.iter().any(|t| t.0 == "anytime"))
            .collect();
        assert!(
            !anytime.is_empty(),
            "expected some anytime-tagged workflows"
        );
        // each anytime workflow must appear in the eligible set of every phase
        for phase in Phase::ALL {
            let eligible = eligible_entries(&report.catalog, phase);
            for aw in &anytime {
                assert!(
                    eligible.iter().any(|e| e.id == aw.id),
                    "anytime workflow {} missing from {} eligible set",
                    aw.id.0,
                    phase
                );
            }
        }
    }

    #[test]
    fn malformed_file_is_reported_not_panicked() {
        let tmp = std::env::temp_dir().join("forge_engine_catalog_test");
        let _ = fs::create_dir_all(&tmp);
        // one good file + one garbage file
        let good = tmp.join("good.yaml");
        let bad = tmp.join("bad.yaml");
        fs::write(
            &good,
            r#"schema_version: "0.1"
workflow:
  id: good-one
  phases:
    - 1-discovery
  trigger:
    - something
  steps:
    - do it
  outputs:
    - result
  done_when:
    - done
"#,
        )
        .unwrap();
        fs::write(&bad, "this: is: not: valid: yaml: [[[[").unwrap();

        let report = load_catalog(&tmp);
        assert!(!report.is_clean(), "garbage file should be reported");
        assert_eq!(report.errors.len(), 1, "exactly the bad file errors");
        assert_eq!(report.catalog.len(), 1, "the good file still loaded");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_entry_resolves_known_id() {
        let report = load_catalog(&real_catalog_dir());
        assert!(report.is_clean());
        // plan-sprint is one of the 110.
        assert!(find_entry(&report.catalog, &StableId("plan-sprint".into())).is_some());
        assert!(find_entry(&report.catalog, &StableId("does-not-exist".into())).is_none());
    }
}
