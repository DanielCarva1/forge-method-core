//! Catalog loading and phase-eligibility filtering.
//!
//! The engine loads the operational compatibility catalog under
//! `contracts/workflows/` into a [`Catalog`] at runtime, then filters it to the
//! workflows eligible in the project's current phase. Retired projections are
//! absent from this routing substrate. A separate evidence-only frozen loader
//! preserves the complete historical 110-workflow catalog for migration and
//! release-admission recomputation; it must never be used for routing.
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

/// Full typed workflow plus its stable repo-relative reference. P5 migration
/// analysis needs the complete legacy state machine, not only the routing view.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LoadedWorkflowDocument {
    pub workflow_ref: RepoPath,
    pub document: WorkflowDocument,
}

/// Accumulating full-document loader used by migration analysis and catalog
/// projection. A non-empty error set makes the complete inventory unusable.
#[derive(Debug, Clone, Default)]
pub struct WorkflowDocumentLoadReport {
    pub workflows: Vec<LoadedWorkflowDocument>,
    pub errors: Vec<CatalogFileError>,
}

impl WorkflowDocumentLoadReport {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty()
    }
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
    let loaded = load_workflow_documents(dir);
    CatalogLoadReport {
        catalog: Catalog {
            entries: loaded
                .workflows
                .iter()
                .map(catalog_entry_from_workflow)
                .collect(),
        },
        errors: loaded.errors,
    }
}

/// Load every full workflow document under `dir` in deterministic path order.
#[must_use]
pub fn load_workflow_documents(dir: &Path) -> WorkflowDocumentLoadReport {
    let mut report = WorkflowDocumentLoadReport::default();
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
        match load_one_document(&path, dir) {
            Ok(workflow) => report.workflows.push(workflow),
            Err(error) => report.errors.push(CatalogFileError {
                path: RepoPath(rel),
                reason: error.to_string(),
            }),
        }
    }

    report
}

fn load_one_document(path: &Path, dir: &Path) -> Result<LoadedWorkflowDocument, CatalogLoadError> {
    let text = fs::read_to_string(path).map_err(|source| CatalogLoadError::Read {
        source: source.to_string(),
    })?;
    // Resolve a stable repo-relative reference (`contracts/workflows/<name>`).
    let workflow_ref = path.strip_prefix(dir).map_or_else(
        |_| path.to_string_lossy().into_owned(),
        |rel| rel.to_string_lossy().into_owned(),
    );
    parse_workflow_document_yaml(&workflow_ref, &text)
}

/// Hand-rolled error enum for the catalog YAML loader. Replaces the legacy
/// `Result<_, String>` signature so callers get typed variants. Converted to
/// the `CatalogFileError.reason` wire field (still a `String`) at the boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogLoadError {
    /// `fs::read_to_string` failed; carries the lossy io error string.
    Read { source: String },
    /// `yaml_serde::from_str` failed; carries the lossy deserialize error string.
    Deserialize { source: String },
}

impl std::fmt::Display for CatalogLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { source } => write!(formatter, "read error: {source}"),
            Self::Deserialize { source } => {
                write!(formatter, "deserialize error: {source}")
            }
        }
    }
}

impl std::error::Error for CatalogLoadError {}

/// Parse a single workflow YAML document from its text. Shared by the disk
/// loader ([`load_one`]) and the embedded loader ([`load_embedded_catalog`])
/// so both paths produce identical [`CatalogEntry`]s.
fn parse_workflow_document_yaml(
    workflow_ref: &str,
    text: &str,
) -> Result<LoadedWorkflowDocument, CatalogLoadError> {
    let doc: WorkflowDocument =
        yaml_serde::from_str(text).map_err(|source| CatalogLoadError::Deserialize {
            source: source.to_string(),
        })?;
    Ok(LoadedWorkflowDocument {
        workflow_ref: RepoPath(format!("contracts/workflows/{workflow_ref}")),
        document: doc,
    })
}

fn catalog_entry_from_workflow(workflow: &LoadedWorkflowDocument) -> CatalogEntry {
    let legacy = &workflow.document.workflow;
    CatalogEntry {
        id: legacy.id.clone(),
        phases: legacy.phases.clone(),
        workflow_ref: workflow.workflow_ref.clone(),
        triggers: legacy.trigger.clone(),
        prerequisites: legacy.inputs.clone(),
        outputs: legacy.outputs.clone(),
    }
}

// ============================================================================
// Embedded operational catalog compiled INTO the binary via
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

/// Evidence-only snapshot of the complete pre-retirement legacy catalog.
///
/// These bytes remain available solely so trusted migration and release
/// evaluators can recompute historical digests and semantic evidence after a
/// projection is removed from the operational compatibility catalog. Public
/// routing must use [`EMBEDDED_WORKFLOWS`], never this directory.
static EMBEDDED_FROZEN_LEGACY_WORKFLOWS: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/../../contracts/evidence/workflow-retirement/legacy-catalog");

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
    let loaded = load_embedded_workflow_documents();
    CatalogLoadReport {
        catalog: Catalog {
            entries: loaded
                .workflows
                .iter()
                .map(catalog_entry_from_workflow)
                .collect(),
        },
        errors: loaded.errors,
    }
}

/// Load the complete workflow documents compiled into the binary.
#[must_use]
pub fn load_embedded_workflow_documents() -> WorkflowDocumentLoadReport {
    load_embedded_documents_from(&EMBEDDED_WORKFLOWS)
}

/// Load the complete evidence-only pre-retirement workflow snapshot.
///
/// Returned references intentionally retain their historical logical
/// `contracts/workflows/<name>.yaml` identities. The physical evidence archive
/// path is not a routable workflow namespace.
#[must_use]
pub fn load_embedded_frozen_legacy_workflow_documents() -> WorkflowDocumentLoadReport {
    load_embedded_documents_from(&EMBEDDED_FROZEN_LEGACY_WORKFLOWS)
}

/// Load the catalog projection of the complete evidence-only snapshot.
///
/// This function exists for deterministic migration/release recomputation. It
/// is not a compatibility or agent-routing surface.
#[must_use]
pub fn load_embedded_frozen_legacy_catalog() -> CatalogLoadReport {
    let loaded = load_embedded_frozen_legacy_workflow_documents();
    CatalogLoadReport {
        catalog: Catalog {
            entries: loaded
                .workflows
                .iter()
                .map(catalog_entry_from_workflow)
                .collect(),
        },
        errors: loaded.errors,
    }
}

/// Return exact frozen source bytes under their historical logical refs.
///
/// Trusted historical evaluators use this to satisfy raw content-addressed
/// bindings after a retired workflow has left the operational embedded tree.
/// The returned paths remain `contracts/workflows/...`; the physical archive
/// location is intentionally not observable as workflow authority.
#[must_use]
pub fn embedded_frozen_legacy_workflow_source_bytes() -> Vec<(RepoPath, &'static [u8])> {
    let mut files = Vec::new();
    collect_bytes(&EMBEDDED_FROZEN_LEGACY_WORKFLOWS, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
        .into_iter()
        .map(|(name, bytes)| (RepoPath(format!("contracts/workflows/{name}")), bytes))
        .collect()
}

fn load_embedded_documents_from(dir: &Dir<'static>) -> WorkflowDocumentLoadReport {
    let mut report = WorkflowDocumentLoadReport::default();
    let mut files: Vec<(String, &str)> = Vec::new();
    collect_yaml(dir, &mut files);
    files.sort();
    for (name, text) in &files {
        match parse_workflow_document_yaml(name, text) {
            Ok(workflow) => report.workflows.push(workflow),
            Err(error) => report.errors.push(CatalogFileError {
                path: RepoPath(name.clone()),
                reason: error.to_string(),
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

fn collect_bytes(dir: &'static Dir<'static>, out: &mut Vec<(String, &'static [u8])>) {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(child) => collect_bytes(child, out),
            DirEntry::File(file) if file.path().extension().is_some_and(|ext| ext == "yaml") => {
                out.push((file.path().to_string_lossy().into_owned(), file.contents()));
            }
            DirEntry::File(_) => {}
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
    fn embedded_operational_catalog_loads_cleanly_and_matches_disk_count() {
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
    fn operational_catalog_contains_only_68_non_retired_workflows() {
        let report = load_catalog(&real_catalog_dir());
        assert!(
            report.is_clean(),
            "load errors: {:?}",
            report.errors.iter().map(|e| &e.reason).collect::<Vec<_>>()
        );
        assert_eq!(report.catalog.len(), 68, "expected 68 operational entries");
    }

    #[test]
    fn frozen_legacy_snapshot_preserves_complete_historical_catalog() {
        let frozen_documents = load_embedded_frozen_legacy_workflow_documents();
        let frozen_catalog = load_embedded_frozen_legacy_catalog();
        assert!(
            frozen_documents.is_clean() && frozen_catalog.is_clean(),
            "frozen legacy evidence must parse cleanly"
        );
        assert_eq!(frozen_documents.workflows.len(), 110);
        assert_eq!(frozen_catalog.catalog.len(), 110);
        let frozen_sources = embedded_frozen_legacy_workflow_source_bytes();
        assert_eq!(frozen_sources.len(), 110);
        assert!(frozen_sources.iter().all(|(path, bytes)| {
            path.0.starts_with("contracts/workflows/") && !bytes.is_empty()
        }));
        assert!(frozen_documents
            .workflows
            .iter()
            .all(|workflow| workflow.workflow_ref.0.starts_with("contracts/workflows/")));
    }

    #[test]
    fn operational_catalog_is_exact_frozen_subset_after_retirement() {
        use std::collections::BTreeSet;

        let operational = load_embedded_catalog();
        let frozen = load_embedded_frozen_legacy_catalog();
        assert!(operational.is_clean() && frozen.is_clean());
        let operational_ids = operational
            .catalog
            .entries
            .iter()
            .map(|entry| entry.id.0.as_str())
            .collect::<BTreeSet<_>>();
        let frozen_ids = frozen
            .catalog
            .entries
            .iter()
            .map(|entry| entry.id.0.as_str())
            .collect::<BTreeSet<_>>();
        assert!(operational_ids.is_subset(&frozen_ids));
        let removed_ids = frozen_ids
            .difference(&operational_ids)
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(removed_ids.len(), 42);

        let runtime_raw = crate::embedded_text(
            "contracts/workflow-governance/runtime-agent-native-continuity-v0.yaml",
        )
        .expect("final P5d.4 runtime bundle");
        let runtime: forge_core_contracts::WorkflowGovernanceBundleDocument =
            yaml_serde::from_str(runtime_raw).expect("typed final runtime bundle");
        let admitted_workflow_ids = runtime
            .workflow_governance_bundle
            .policies
            .iter()
            .map(|policy| policy.compatibility_workflow_id.0.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(removed_ids, admitted_workflow_ids);

        let frozen_documents = load_embedded_frozen_legacy_workflow_documents();
        for operational_workflow in load_embedded_workflow_documents().workflows {
            let frozen_workflow = frozen_documents
                .workflows
                .iter()
                .find(|candidate| {
                    candidate.document.workflow.id == operational_workflow.document.workflow.id
                })
                .expect("every operational workflow remains byte-semantically frozen");
            assert_eq!(&operational_workflow, frozen_workflow);
        }
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
        // brainstorming remains on the operational compatibility surface.
        assert!(find_entry(&report.catalog, &StableId("brainstorming".into())).is_some());
        assert!(find_entry(&report.catalog, &StableId("does-not-exist".into())).is_none());
    }
}
