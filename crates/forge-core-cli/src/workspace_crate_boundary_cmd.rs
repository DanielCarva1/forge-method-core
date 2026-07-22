//! Read-only validation for the declared Cargo workspace crate boundary.
//!
//! This module invokes only `cargo metadata --no-deps`; it never builds or
//! executes a workspace binary. The policy document is candidate-only: a clean
//! report describes a checked-in crate graph and cannot select a host, admit a
//! candidate, or grant lifecycle, mutation, or trust authority.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use forge_core_contracts::{CliEnvelope, ExitReason};
use serde::{Deserialize, Serialize};

const COMMAND: &str = "workspace-crate-boundary";
const POLICY_REF: &str = "contracts/policies/workspace-crate-boundary-v0.yaml";
const REQUIRED_SELECTED_HOST: &str = "none";

/// Candidate-only, typed workspace boundary policy.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceCrateBoundaryDocument {
    schema_version: String,
    policy: String,
    status: String,
    selected_host: String,
    workspace_crate_boundary: WorkspaceCrateBoundary,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceCrateBoundary {
    crates: Vec<DeclaredCrate>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclaredCrate {
    name: String,
    manifest_path: String,
    depends_on: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    manifest_path: String,
    dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Deserialize)]
struct CargoDependency {
    name: String,
}

/// One fail-closed boundary mismatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceCrateBoundaryIssue {
    pub code: String,
    pub detail: String,
}

/// Read-only comparison of the declared and discovered workspace graphs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceCrateBoundaryReport {
    pub policy_ref: String,
    pub selected_host: String,
    pub declared_crates: Vec<String>,
    pub discovered_crates: Vec<String>,
    pub declared_dependencies: BTreeMap<String, Vec<String>>,
    pub discovered_dependencies: BTreeMap<String, Vec<String>>,
    pub issues: Vec<WorkspaceCrateBoundaryIssue>,
}

impl WorkspaceCrateBoundaryReport {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Load the typed candidate policy and compare it to Cargo's read-only metadata.
///
/// The returned envelope is deliberately a report only. No result from this
/// function carries admission, installation, activation, lifecycle, mutation,
/// signing, trust, protected-anchor, private-key, phase-transition, or
/// host-selection authority.
#[must_use]
pub fn run_workspace_crate_boundary(root: impl AsRef<Path>) -> CliEnvelope<WorkspaceCrateBoundaryReport> {
    let root = root.as_ref();
    match workspace_crate_boundary_report(root) {
        Ok(report) if report.is_clean() => CliEnvelope::ok(COMMAND, report),
        Ok(report) => CliEnvelope::reject(
            COMMAND,
            ExitReason::RejectedByGate,
            "workspace crate boundary differs from its candidate-only policy",
            report,
        ),
        Err(error) => CliEnvelope::err(COMMAND, ExitReason::InvalidDecisionShape, error),
    }
}

/// Construct a read-only boundary report, failing closed for malformed policy,
/// unreadable manifests, or failed Cargo metadata collection.
pub fn workspace_crate_boundary_report(
    root: impl AsRef<Path>,
) -> Result<WorkspaceCrateBoundaryReport, String> {
    let root = root.as_ref();
    let policy = load_policy(root)?;
    let metadata = load_metadata(root)?;
    let actual = inspect_workspace_manifests(root, metadata)?;

    let declared_crates = declared_crate_names(&policy)?;
    let declared_dependencies = declared_dependencies(&policy, &declared_crates)?;
    let (discovered_crates, discovered_dependencies) = workspace_graph(&actual)?;
    let mut issues = Vec::new();

    if policy.schema_version != "0.1"
        || policy.policy != "workspace_crate_boundary"
        || policy.status != "active"
    {
        issues.push(issue(
            "invalid_policy_identity",
            "workspace boundary policy must be active workspace_crate_boundary schema 0.1",
        ));
    }
    if policy.selected_host != REQUIRED_SELECTED_HOST {
        issues.push(issue(
            "selected_host_not_none",
            "candidate workspace boundary policy must keep selected_host as none",
        ));
    }
    compare_sets("crate_declarations", &declared_crates, &discovered_crates, &mut issues);
    compare_graphs(
        &declared_dependencies,
        &discovered_dependencies,
        &mut issues,
    );

    Ok(WorkspaceCrateBoundaryReport {
        policy_ref: POLICY_REF.to_string(),
        selected_host: REQUIRED_SELECTED_HOST.to_string(),
        declared_crates: declared_crates.into_iter().collect(),
        discovered_crates: discovered_crates.into_iter().collect(),
        declared_dependencies: into_sorted_vecs(declared_dependencies),
        discovered_dependencies: into_sorted_vecs(discovered_dependencies),
        issues,
    })
}

fn load_policy(root: &Path) -> Result<WorkspaceCrateBoundaryDocument, String> {
    let path = root.join(POLICY_REF);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    yaml_serde::from_str(&text)
        .map_err(|error| format!("cannot parse {} as typed boundary policy: {error}", path.display()))
}

fn load_metadata(root: &Path) -> Result<CargoMetadata, String> {
    let output = Command::new("cargo")
        .current_dir(root)
        .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
        .output()
        .map_err(|error| format!("cannot execute read-only cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "read-only cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cannot parse cargo metadata JSON: {error}"))
}

fn inspect_workspace_manifests(root: &Path, metadata: CargoMetadata) -> Result<Vec<CargoPackage>, String> {
    let member_ids = metadata.workspace_members.into_iter().collect::<BTreeSet<_>>();
    let mut members = Vec::new();
    for package in metadata.packages {
        if !member_ids.contains(&package.id) {
            continue;
        }
        let path = PathBuf::from(&package.manifest_path);
        if !path.starts_with(root) {
            return Err(format!("workspace manifest escapes repository root: {}", path.display()));
        }
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read workspace manifest {}: {error}", path.display()))?;
        if !text.contains("[package]") || !text.contains(&format!("name = \"{}\"", package.name)) {
            return Err(format!("workspace manifest disagrees with metadata: {}", path.display()));
        }
        members.push(package);
    }
    if members.is_empty() {
        return Err("cargo metadata reported no workspace packages".to_string());
    }
    Ok(members)
}

fn declared_crate_names(policy: &WorkspaceCrateBoundaryDocument) -> Result<BTreeSet<String>, String> {
    let mut names = BTreeSet::new();
    for crate_policy in &policy.workspace_crate_boundary.crates {
        if crate_policy.name.is_empty() || !names.insert(crate_policy.name.clone()) {
            return Err("workspace boundary policy has empty or duplicate crate names".to_string());
        }
    }
    Ok(names)
}

fn declared_dependencies(
    policy: &WorkspaceCrateBoundaryDocument,
    names: &BTreeSet<String>,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut graph = BTreeMap::new();
    for crate_policy in &policy.workspace_crate_boundary.crates {
        let path = Path::new(&crate_policy.manifest_path);
        if path.is_absolute() || path.components().any(|component| component.as_os_str() == "..") {
            return Err(format!("declared manifest path escapes repository: {}", crate_policy.manifest_path));
        }
        let dependencies = crate_policy.depends_on.iter().cloned().collect::<BTreeSet<_>>();
        if dependencies.len() != crate_policy.depends_on.len()
            || dependencies.iter().any(|dependency| !names.contains(dependency))
            || dependencies.contains(&crate_policy.name)
        {
            return Err(format!("invalid declared dependency set for {}", crate_policy.name));
        }
        graph.insert(crate_policy.name.clone(), dependencies);
    }
    Ok(graph)
}

fn workspace_graph(
    packages: &[CargoPackage],
) -> Result<(BTreeSet<String>, BTreeMap<String, BTreeSet<String>>), String> {
    let names = packages.iter().map(|package| package.name.clone()).collect::<BTreeSet<_>>();
    if names.len() != packages.len() {
        return Err("cargo metadata has duplicate workspace package names".to_string());
    }
    let graph = packages
        .iter()
        .map(|package| {
            let dependencies = package
                .dependencies
                .iter()
                .map(|dependency| dependency.name.clone())
                .filter(|dependency| names.contains(dependency))
                .collect();
            (package.name.clone(), dependencies)
        })
        .collect();
    Ok((names, graph))
}

fn compare_sets(
    label: &str,
    declared: &BTreeSet<String>,
    discovered: &BTreeSet<String>,
    issues: &mut Vec<WorkspaceCrateBoundaryIssue>,
) {
    let missing = discovered.difference(declared).cloned().collect::<Vec<_>>();
    let extra = declared.difference(discovered).cloned().collect::<Vec<_>>();
    if !missing.is_empty() || !extra.is_empty() {
        issues.push(issue(
            "crate_set_mismatch",
            format!("{label} missing={missing:?} extra={extra:?}"),
        ));
    }
}

fn compare_graphs(
    declared: &BTreeMap<String, BTreeSet<String>>,
    discovered: &BTreeMap<String, BTreeSet<String>>,
    issues: &mut Vec<WorkspaceCrateBoundaryIssue>,
) {
    for name in declared.keys().chain(discovered.keys()).collect::<BTreeSet<_>>() {
        let expected = declared.get(name).cloned().unwrap_or_default();
        let actual = discovered.get(name).cloned().unwrap_or_default();
        if expected != actual {
            issues.push(issue(
                "dependency_graph_mismatch",
                format!("{name}: declared={expected:?} discovered={actual:?}"),
            ));
        }
    }
}

fn into_sorted_vecs(
    graph: BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<String, Vec<String>> {
    graph
        .into_iter()
        .map(|(name, dependencies)| (name, dependencies.into_iter().collect()))
        .collect()
}

fn issue(code: impl Into<String>, detail: impl Into<String>) -> WorkspaceCrateBoundaryIssue {
    WorkspaceCrateBoundaryIssue {
        code: code.into(),
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_is_read_only_and_keeps_host_unselected() {
        let report = WorkspaceCrateBoundaryReport {
            policy_ref: POLICY_REF.to_string(),
            selected_host: REQUIRED_SELECTED_HOST.to_string(),
            declared_crates: vec!["forge-core-cli".to_string()],
            discovered_crates: vec!["forge-core-cli".to_string()],
            declared_dependencies: BTreeMap::new(),
            discovered_dependencies: BTreeMap::new(),
            issues: Vec::new(),
        };
        let serialized = serde_json::to_string(&report).expect("report serializes");
        assert!(report.is_clean());
        assert!(serialized.contains("\"selected_host\":\"none\""));
        for forbidden in ["sign", "install", "activate", "private_key", "mutation"] {
            assert!(!serialized.contains(forbidden), "report must not carry {forbidden} authority");
        }
    }

    #[test]
    fn graph_comparison_rejects_missing_and_extra_edges() {
        let declared = BTreeMap::from([(
            "forge-core-cli".to_string(),
            BTreeSet::from(["forge-core-contracts".to_string()]),
        )]);
        let discovered = BTreeMap::from([("forge-core-cli".to_string(), BTreeSet::new())]);
        let mut issues = Vec::new();
        compare_graphs(&declared, &discovered, &mut issues);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "dependency_graph_mismatch");
    }

    #[test]
    fn selected_host_other_than_none_is_reported() {
        let policy = WorkspaceCrateBoundaryDocument {
            schema_version: "0.1".to_string(),
            policy: "workspace_crate_boundary".to_string(),
            status: "active".to_string(),
            selected_host: "codex".to_string(),
            workspace_crate_boundary: WorkspaceCrateBoundary { crates: Vec::new() },
        };
        assert_ne!(policy.selected_host, REQUIRED_SELECTED_HOST);
    }
}
