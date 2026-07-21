#![allow(clippy::missing_errors_doc)]

//! Static, host-neutral validation for the checked-in workspace architecture contracts.
//!
//! These documents are descriptive candidate inputs. Loading them cannot select a
//! host or grant signing, trust, installation, activation, mutation, private-key,
//! release, phase-transition, protected-anchor, or core-authority capabilities.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use forge_core_contracts::{CrateId, WORKSPACE_CRATE_COUNT};
use serde::{Deserialize, Serialize};

const EXPECTED_CRATE_COUNT: usize = WORKSPACE_CRATE_COUNT;
const SELECTED_HOST: &str = "none";
const RUST_CORE_PATH: &str = "contracts/architecture/rust-core.yaml";
const BOUNDARIES_PATH: &str = "contracts/architecture/crate-boundaries.yaml";
const POLICY_PATH: &str = "contracts/policies/workspace-crate-boundary-v0.yaml";
const AUTHORITY_CRATES: [&str; 2] = ["forge-core-authority", "forge-core-kernel"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceArchitectureIssue {
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceArchitectureReport {
    pub selected_host: String,
    pub declared_crates: Vec<String>,
    pub issues: Vec<WorkspaceArchitectureIssue>,
}

impl WorkspaceArchitectureReport {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustCoreDocument {
    schema_version: String,
    architecture: String,
    selected_host: String,
    authority_model: AuthorityModel,
    crates: Vec<OwnershipCrate>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityModel {
    host_client_authority: String,
    core_authority: String,
    candidate_documents_are_inert: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnershipCrate {
    name: String,
    owns: String,
    does_not_own: String,
    authority_boundary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundariesDocument {
    schema_version: String,
    contract: String,
    selected_host: String,
    authority_boundary: BoundaryAuthority,
    crates: Vec<BoundaryCrate>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundaryAuthority {
    host_client_crates: Vec<String>,
    compatibility_client_crates: Vec<String>,
    core_authority_crates: Vec<String>,
    candidate_documents_are_inert: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundaryCrate {
    name: String,
    manifest_path: String,
    depends_on: Vec<String>,
    owns: String,
    does_not_own: String,
    authority_boundary: String,
    reviewed_authority_dependencies: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyDocument {
    schema_version: String,
    policy: String,
    status: String,
    selected_host: String,
    workspace_crate_boundary: CandidateBoundary,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateBoundary {
    crates: Vec<CandidateCrate>,
    reviewed_authority_edges: Vec<AuthorityEdge>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateCrate {
    name: String,
    manifest_path: String,
    depends_on: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityEdge {
    from: String,
    to: String,
}

/// Load all canonical workspace-architecture contracts and reject partial,
/// divergent, malformed, host-selecting, or unreviewed-authority projections.
pub fn validate_workspace_architecture_contracts(
    root: impl AsRef<Path>,
) -> Result<WorkspaceArchitectureReport, String> {
    let root = root.as_ref();
    let rust_core: RustCoreDocument = load(root, RUST_CORE_PATH)?;
    let boundaries: BoundariesDocument = load(root, BOUNDARIES_PATH)?;
    let policy: PolicyDocument = load(root, POLICY_PATH)?;
    let mut issues = Vec::new();

    if rust_core.schema_version != "0.1"
        || rust_core.architecture != "rust_core"
        || rust_core.selected_host != SELECTED_HOST
        || !rust_core.authority_model.candidate_documents_are_inert
        || rust_core
            .authority_model
            .host_client_authority
            .trim()
            .is_empty()
        || rust_core.authority_model.core_authority.trim().is_empty()
    {
        issues.push(issue(
            "invalid_rust_core_identity",
            "rust-core.yaml must be hostless, inert, and schema 0.1",
        ));
    }
    if boundaries.schema_version != "0.1"
        || boundaries.contract != "crate_boundaries"
        || boundaries.selected_host != SELECTED_HOST
        || !boundaries.authority_boundary.candidate_documents_are_inert
    {
        issues.push(issue(
            "invalid_boundary_identity",
            "crate-boundaries.yaml must be hostless, inert, and schema 0.1",
        ));
    }
    if policy.schema_version != "0.1"
        || policy.policy != "workspace_crate_boundary"
        || policy.status != "active"
        || policy.selected_host != SELECTED_HOST
    {
        issues.push(issue(
            "invalid_policy_identity",
            "candidate policy must be active, hostless workspace_crate_boundary schema 0.1",
        ));
    }

    let architecture = ownership_map(&rust_core.crates, "rust-core", &mut issues);
    let boundary = boundary_map(&boundaries.crates, &mut issues);
    let candidate = candidate_map(&policy.workspace_crate_boundary.crates, &mut issues);
    require_exhaustive("rust-core", &architecture, &mut issues);
    require_exhaustive("crate-boundaries", &boundary, &mut issues);
    require_exhaustive("candidate policy", &candidate, &mut issues);
    require_known_crates("rust-core", &architecture, &mut issues);
    require_known_crates("crate-boundaries", &boundary, &mut issues);
    require_known_crates("candidate policy", &candidate, &mut issues);

    let architecture_names = architecture.keys().cloned().collect::<BTreeSet<_>>();
    let boundary_names = boundary.keys().cloned().collect::<BTreeSet<_>>();
    let candidate_names = candidate.keys().cloned().collect::<BTreeSet<_>>();
    if architecture_names != boundary_names || boundary_names != candidate_names {
        issues.push(issue("crate_set_mismatch", "canonical architecture, boundaries, and candidate policy must name the same exhaustive crate set"));
    }

    for (name, ownership) in &architecture {
        match boundary.get(name) {
            Some(crate_boundary) if crate_boundary.ownership == *ownership => {}
            Some(_) => issues.push(issue(
                "ownership_mismatch",
                format!(
                    "{name} ownership or authority boundary differs between canonical contracts"
                ),
            )),
            None => {}
        }
    }
    for (name, crate_boundary) in &boundary {
        match candidate.get(name) {
            Some(candidate_crate)
                if candidate_crate.manifest_path == crate_boundary.manifest_path
                    && candidate_crate.depends_on == crate_boundary.depends_on => {}
            Some(_) => issues.push(issue(
                "candidate_projection_mismatch",
                format!("{name} candidate graph differs from crate-boundaries.yaml"),
            )),
            None => {}
        }
    }

    let expected_host_clients = BTreeSet::from([
        "forge-core-cli".to_string(),
        "forge-core-protocol-mcp".to_string(),
    ]);
    let actual_host_clients = boundaries
        .authority_boundary
        .host_client_crates
        .into_iter()
        .collect::<BTreeSet<_>>();
    if actual_host_clients != expected_host_clients {
        issues.push(issue(
            "host_client_boundary_mismatch",
            "host/client authority must enumerate only CLI and MCP adapters",
        ));
    }
    if boundaries
        .authority_boundary
        .compatibility_client_crates
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != BTreeSet::from(["forge-contract-validator"])
    {
        issues.push(issue(
            "compatibility_boundary_mismatch",
            "compatibility client authority must enumerate only forge-contract-validator",
        ));
    }
    if boundaries
        .authority_boundary
        .core_authority_crates
        .into_iter()
        .collect::<BTreeSet<_>>()
        != AUTHORITY_CRATES.into_iter().map(str::to_string).collect()
    {
        issues.push(issue(
            "core_authority_boundary_mismatch",
            "core authority must enumerate authority and kernel exactly",
        ));
    }

    let expected_reviewed = candidate
        .iter()
        .flat_map(|(name, crate_boundary)| {
            crate_boundary
                .depends_on
                .iter()
                .filter(|&dependency| AUTHORITY_CRATES.contains(&dependency.as_str()))
                .map(|dependency| (name.clone(), dependency.clone()))
        })
        .collect::<BTreeSet<_>>();
    let policy_reviewed = policy
        .workspace_crate_boundary
        .reviewed_authority_edges
        .into_iter()
        .map(|edge| (edge.from, edge.to))
        .collect::<BTreeSet<_>>();
    let boundary_reviewed = boundary
        .iter()
        .flat_map(|(name, crate_boundary)| {
            crate_boundary
                .reviewed_authority_dependencies
                .iter()
                .map(move |dependency| (name.clone(), dependency.clone()))
        })
        .collect::<BTreeSet<_>>();
    if policy_reviewed != expected_reviewed || boundary_reviewed != expected_reviewed {
        issues.push(issue(
            "unreviewed_authority_edge",
            "every and only direct edge into core authority must be explicitly reviewed",
        ));
    }

    Ok(WorkspaceArchitectureReport {
        selected_host: SELECTED_HOST.to_string(),
        declared_crates: candidate_names.into_iter().collect(),
        issues,
    })
}

fn load<T: for<'de> Deserialize<'de>>(root: &Path, relative: &str) -> Result<T, String> {
    let path = root.join(relative);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    yaml_serde::from_str(&text).map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Ownership {
    owns: String,
    does_not_own: String,
    authority_boundary: String,
}

#[derive(Debug)]
struct BoundaryProjection {
    ownership: Ownership,
    manifest_path: String,
    depends_on: BTreeSet<String>,
    reviewed_authority_dependencies: BTreeSet<String>,
}

#[derive(Debug)]
struct CandidateProjection {
    manifest_path: String,
    depends_on: BTreeSet<String>,
}

fn ownership_map(
    crates: &[OwnershipCrate],
    label: &str,
    issues: &mut Vec<WorkspaceArchitectureIssue>,
) -> BTreeMap<String, Ownership> {
    crates
        .iter()
        .filter_map(|crate_policy| {
            if crate_policy.name.trim().is_empty()
                || crate_policy.owns.trim().is_empty()
                || crate_policy.does_not_own.trim().is_empty()
                || !matches!(
                    crate_policy.authority_boundary.as_str(),
                    "core" | "host_client" | "compatibility"
                )
            {
                issues.push(issue(
                    "invalid_ownership",
                    format!("{label} contains an incomplete ownership declaration"),
                ));
                return None;
            }
            Some((
                crate_policy.name.clone(),
                Ownership {
                    owns: crate_policy.owns.clone(),
                    does_not_own: crate_policy.does_not_own.clone(),
                    authority_boundary: crate_policy.authority_boundary.clone(),
                },
            ))
        })
        .collect()
}

fn boundary_map(
    crates: &[BoundaryCrate],
    issues: &mut Vec<WorkspaceArchitectureIssue>,
) -> BTreeMap<String, BoundaryProjection> {
    crates
        .iter()
        .filter_map(|crate_policy| {
            let deps = crate_policy
                .depends_on
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            let reviewed = crate_policy
                .reviewed_authority_dependencies
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            if crate_policy.name.trim().is_empty()
                || crate_policy.manifest_path.trim().is_empty()
                || crate_policy.owns.trim().is_empty()
                || crate_policy.does_not_own.trim().is_empty()
                || deps.len() != crate_policy.depends_on.len()
                || reviewed.len() != crate_policy.reviewed_authority_dependencies.len()
            {
                issues.push(issue(
                    "invalid_boundary_crate",
                    "crate-boundaries.yaml contains an incomplete or duplicate crate declaration",
                ));
                return None;
            }
            Some((
                crate_policy.name.clone(),
                BoundaryProjection {
                    ownership: Ownership {
                        owns: crate_policy.owns.clone(),
                        does_not_own: crate_policy.does_not_own.clone(),
                        authority_boundary: crate_policy.authority_boundary.clone(),
                    },
                    manifest_path: crate_policy.manifest_path.clone(),
                    depends_on: deps,
                    reviewed_authority_dependencies: reviewed,
                },
            ))
        })
        .collect()
}

fn candidate_map(
    crates: &[CandidateCrate],
    issues: &mut Vec<WorkspaceArchitectureIssue>,
) -> BTreeMap<String, CandidateProjection> {
    crates
        .iter()
        .filter_map(|crate_policy| {
            let deps = crate_policy
                .depends_on
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            if crate_policy.name.trim().is_empty()
                || crate_policy.manifest_path.trim().is_empty()
                || deps.len() != crate_policy.depends_on.len()
            {
                issues.push(issue(
                    "invalid_candidate_crate",
                    "candidate policy contains an incomplete or duplicate crate declaration",
                ));
                return None;
            }
            Some((
                crate_policy.name.clone(),
                CandidateProjection {
                    manifest_path: crate_policy.manifest_path.clone(),
                    depends_on: deps,
                },
            ))
        })
        .collect()
}

fn require_exhaustive<T>(
    label: &str,
    crates: &BTreeMap<String, T>,
    issues: &mut Vec<WorkspaceArchitectureIssue>,
) {
    if crates.len() != EXPECTED_CRATE_COUNT {
        issues.push(issue("partial_workspace_projection", format!("{label} declares {} crates; exactly {EXPECTED_CRATE_COUNT} are required and eight-crate projections are rejected", crates.len())));
    }
}

fn require_known_crates<T>(
    label: &str,
    crates: &BTreeMap<String, T>,
    issues: &mut Vec<WorkspaceArchitectureIssue>,
) {
    let expected = CrateId::ALL
        .iter()
        .map(|id| id.package_name().to_string())
        .collect::<BTreeSet<_>>();
    let actual = crates.keys().cloned().collect::<BTreeSet<_>>();
    if actual != expected {
        issues.push(issue(
            "unknown_or_missing_crate",
            format!("{label} must declare the complete canonical workspace crate set"),
        ));
    }
}

fn issue(code: impl Into<String>, detail: impl Into<String>) -> WorkspaceArchitectureIssue {
    WorkspaceArchitectureIssue {
        code: code.into(),
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_architecture_is_exhaustive_hostless_and_reviewed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = validate_workspace_architecture_contracts(root).expect("contracts load");
        assert!(report.is_clean(), "{:?}", report.issues);
        assert_eq!(report.selected_host, "none");
        assert_eq!(report.declared_crates.len(), EXPECTED_CRATE_COUNT);
    }
}
