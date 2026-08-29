#![allow(clippy::missing_errors_doc)]

//! Static, host-neutral validation for the checked-in workspace architecture contracts.
//!
//! These documents are descriptive candidate inputs. Loading them cannot select a
//! host or grant signing, trust, installation, activation, mutation, private-key,
//! release, phase-transition, protected-anchor, or core-authority capabilities.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use forge_core_contracts::{
    validate_workspace_projection, CrateId, WorkspaceCrateObservation, WorkspaceProjection,
    WORKSPACE_CRATE_COUNT,
};
use serde::{Deserialize, Serialize};

const EXPECTED_CRATE_COUNT: usize = WORKSPACE_CRATE_COUNT;
const SELECTED_HOST: &str = "none";
const RUST_CORE_PATH: &str = "contracts/architecture/rust-core.yaml";
const BOUNDARIES_PATH: &str = "contracts/architecture/crate-boundaries.yaml";
const POLICY_PATH: &str = "contracts/policies/workspace-crate-boundary-v0.yaml";
const SOLO_SPEC_PATH: &str = "contracts/spec/solo-dogfood-readiness-v0.yaml";
const PRODUCT_PLAN_PATH: &str = "contracts/plan/product-gap-closure-plan.yaml";
const PRODUCT_CAMPAIGN_PATH: &str = "contracts/plan/product-gap-closure-campaign-v1.yaml";
const PRODUCT_INVENTORY_PATH: &str = "contracts/plan/product-gap-closure-story-inventory-v1.yaml";
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

#[derive(Debug, Deserialize)]
struct CargoMetadataDocument {
    packages: Vec<CargoMetadataPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadataPackage {
    id: String,
    name: String,
    manifest_path: PathBuf,
    dependencies: Vec<CargoMetadataDependency>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadataDependency {
    name: String,
    #[serde(default)]
    kind: Option<String>,
}

#[derive(Debug)]
struct OwnedWorkspaceObservation {
    package_name: String,
    path: String,
    manifest_path: String,
    declared_dependencies: BTreeSet<String>,
    production_dependencies: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CurrentProductAuthorityDocument {
    current_product_authority: CurrentProductAuthorityProjection,
}

#[derive(Debug, Deserialize)]
struct ProductPlanDocument {
    current_product_authority: CurrentProductAuthorityProjection,
    first_executable_slice: Option<FirstExecutableSlice>,
}

#[derive(Debug, Deserialize)]
struct FirstExecutableSlice {
    first_item: String,
    status: String,
    authority_ref: String,
}

#[derive(Debug, Deserialize)]
struct ProductCampaignDocument {
    current_product_authority: CurrentProductAuthorityProjection,
    solo_execution: SoloExecution,
}

#[derive(Debug, Deserialize)]
struct SoloExecution {
    authority: String,
    items: Vec<SoloExecutionItem>,
}

#[derive(Debug, Deserialize)]
struct SoloExecutionItem {
    id: String,
    status: String,
    checkpoint: CampaignCheckpoint,
}

#[derive(Debug, Deserialize)]
struct CampaignCheckpoint {
    kind: String,
    state_ref: String,
    evidence_refs: Vec<String>,
    remaining_work: Vec<String>,
    base_commit: String,
    updated_at: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct CurrentProductAuthorityProjection {
    authority_ref: String,
    authority_revision: u64,
    milestone_id: String,
    milestone: String,
    milestone_state: String,
    milestone_qualified: bool,
    readiness_profile: String,
    executable_item_ids: Vec<String>,
    strict_external_state: String,
    current_summary: CurrentProductSummary,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct CurrentProductSummary {
    completed_item_ids: Vec<String>,
    next_item_id: Option<String>,
    remaining_item_ids: Vec<String>,
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
    if root.join(SOLO_SPEC_PATH).exists() {
        validate_current_product_authority(root, &mut issues)?;
    }

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

/// Parse read-only `cargo metadata --locked --no-deps` output and compare the
/// real workspace paths and dependency graph with the canonical Rust boundary.
pub fn validate_cargo_metadata_workspace(
    root: impl AsRef<Path>,
    metadata_json: &[u8],
) -> Result<Vec<WorkspaceArchitectureIssue>, String> {
    let root = fs::canonicalize(root.as_ref())
        .map_err(|error| format!("cannot canonicalize workspace root: {error}"))?;
    let metadata: CargoMetadataDocument = serde_json::from_slice(metadata_json)
        .map_err(|error| format!("cargo metadata emitted invalid JSON: {error}"))?;
    let policy: PolicyDocument = load(&root, POLICY_PATH)?;
    let mut issues = Vec::new();
    let candidate = candidate_map(&policy.workspace_crate_boundary.crates, &mut issues);
    let member_ids = metadata
        .workspace_members
        .into_iter()
        .collect::<BTreeSet<_>>();
    let member_names = metadata
        .packages
        .iter()
        .filter(|package| member_ids.contains(&package.id))
        .map(|package| package.name.clone())
        .collect::<BTreeSet<_>>();

    let mut owned = metadata
        .packages
        .into_iter()
        .filter(|package| member_ids.contains(&package.id))
        .map(|package| {
            let manifest_metadata =
                fs::symlink_metadata(&package.manifest_path).map_err(|error| {
                    format!(
                        "cannot inspect workspace manifest {}: {error}",
                        package.manifest_path.display()
                    )
                })?;
            if !manifest_metadata.is_file() || manifest_metadata.file_type().is_symlink() {
                return Err(format!(
                    "workspace manifest is not a regular file: {}",
                    package.manifest_path.display()
                ));
            }
            let manifest = fs::canonicalize(&package.manifest_path).map_err(|error| {
                format!(
                    "cannot canonicalize workspace manifest {}: {error}",
                    package.manifest_path.display()
                )
            })?;
            let relative = manifest
                .strip_prefix(&root)
                .map_err(|_| format!("workspace manifest escapes root: {}", manifest.display()))?;
            if relative.file_name().and_then(|name| name.to_str()) != Some("Cargo.toml") {
                return Err(format!(
                    "workspace manifest is not a Cargo.toml file: {}",
                    manifest.display()
                ));
            }
            let crate_path = relative.parent().ok_or_else(|| {
                format!(
                    "workspace manifest has no crate directory: {}",
                    manifest.display()
                )
            })?;
            let path = crate_path
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let manifest_path = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let mut declared_dependencies = BTreeSet::new();
            let mut production_dependencies = BTreeSet::new();
            for dependency in package
                .dependencies
                .into_iter()
                .filter(|dependency| member_names.contains(&dependency.name))
            {
                if dependency.kind.as_deref() != Some("dev") {
                    production_dependencies.insert(dependency.name.clone());
                }
                declared_dependencies.insert(dependency.name);
            }
            Ok(OwnedWorkspaceObservation {
                package_name: package.name,
                path,
                manifest_path,
                declared_dependencies,
                production_dependencies: production_dependencies.into_iter().collect(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    owned.sort_by(|left, right| left.package_name.cmp(&right.package_name));

    let observed_names = owned
        .iter()
        .map(|package| package.package_name.clone())
        .collect::<BTreeSet<_>>();
    let candidate_names = candidate.keys().cloned().collect::<BTreeSet<_>>();
    if observed_names != candidate_names {
        issues.push(issue(
            "cargo_workspace_contract_mismatch",
            "Cargo and the workspace boundary policy declare different crate sets",
        ));
    }
    for package in &owned {
        if let Some(declared) = candidate.get(&package.package_name) {
            if declared.manifest_path != package.manifest_path
                || declared.depends_on != package.declared_dependencies
            {
                issues.push(issue(
                    "cargo_workspace_contract_mismatch",
                    format!(
                        "{} Cargo manifest path or dependency graph differs from the workspace boundary policy",
                        package.package_name
                    ),
                ));
            }
        }
    }

    let dependency_refs = owned
        .iter()
        .map(|package| {
            package
                .production_dependencies
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let observations = owned
        .iter()
        .zip(&dependency_refs)
        .map(|(package, dependencies)| WorkspaceCrateObservation {
            package_name: &package.package_name,
            path: &package.path,
            workspace_dependencies: dependencies,
            enabled_features: &[],
        })
        .collect::<Vec<_>>();

    issues.extend(
        validate_workspace_projection(&WorkspaceProjection {
            crates: &observations,
        })
        .into_iter()
        .map(|violation| {
            issue(
                "cargo_workspace_boundary_violation",
                format!("{violation:?}"),
            )
        }),
    );
    Ok(issues)
}

fn validate_current_product_authority(
    root: &Path,
    issues: &mut Vec<WorkspaceArchitectureIssue>,
) -> Result<(), String> {
    let spec: CurrentProductAuthorityDocument = load(root, SOLO_SPEC_PATH)?;
    let summary = &spec.current_product_authority.current_summary;
    let mut projected = summary.completed_item_ids.clone();
    projected.extend(summary.remaining_item_ids.iter().cloned());
    let expected_next = summary.remaining_item_ids.first();
    if projected != spec.current_product_authority.executable_item_ids
        || summary.next_item_id.as_ref() != expected_next
        || (spec.current_product_authority.milestone_qualified
            && !summary.remaining_item_ids.is_empty())
    {
        issues.push(issue(
            "current_product_summary_invalid",
            "completed plus remaining items must preserve executable order, the next item must be the first remaining item, and an incomplete milestone cannot be qualified",
        ));
    }
    let plan: ProductPlanDocument = load(root, PRODUCT_PLAN_PATH)?;
    if plan.current_product_authority != spec.current_product_authority {
        issues.push(issue(
            "current_product_authority_mismatch",
            "plan.current_product_authority diverges from the rank-1 Solo specification",
        ));
    }
    let plan_slice_is_valid = match (&summary.next_item_id, &plan.first_executable_slice) {
        (Some(next_item), Some(first_slice)) => {
            let expected_ref = format!(
                "contracts/spec/solo-dogfood-readiness-v0.yaml#implementation_decisions.delivery_sequence[{next_item}]"
            );
            first_slice.first_item == *next_item
                && first_slice.status == "in_progress"
                && first_slice.authority_ref == expected_ref
        }
        (None, None) => true,
        _ => false,
    };
    if !plan_slice_is_valid {
        let detail = if summary.next_item_id.is_none() {
            "the terminal summary cannot retain an active first executable slice"
        } else {
            "the plan's active first slice must identify the current summary's next item, remain in progress, and bind its exact specification entry"
        };
        issues.push(issue("current_product_plan_order_mismatch", detail));
    }
    let campaign: ProductCampaignDocument = load(root, PRODUCT_CAMPAIGN_PATH)?;
    if campaign.current_product_authority != spec.current_product_authority {
        issues.push(issue(
            "current_product_authority_mismatch",
            "campaign.current_product_authority diverges from the rank-1 Solo specification",
        ));
    }
    validate_solo_campaign(&spec.current_product_authority, &campaign, issues);

    let inventory: CurrentProductAuthorityDocument = load(root, PRODUCT_INVENTORY_PATH)?;
    if inventory.current_product_authority != spec.current_product_authority {
        issues.push(issue(
            "current_product_authority_mismatch",
            "inventory.current_product_authority diverges from the rank-1 Solo specification",
        ));
    }
    Ok(())
}

fn validate_solo_campaign(
    authority: &CurrentProductAuthorityProjection,
    campaign: &ProductCampaignDocument,
    issues: &mut Vec<WorkspaceArchitectureIssue>,
) {
    let expected_authority = format!("{PRODUCT_CAMPAIGN_PATH}#solo_execution");
    if campaign.solo_execution.authority != expected_authority {
        issues.push(issue(
            "current_product_campaign_authority_mismatch",
            "solo_execution.authority must point to the canonical campaign execution block",
        ));
    }
    let item_ids = campaign
        .solo_execution
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    if item_ids != authority.executable_item_ids {
        issues.push(issue(
            "current_product_campaign_order_mismatch",
            "the Solo campaign must contain every executable item exactly once in specification order",
        ));
    }

    for item in &campaign.solo_execution.items {
        let expected_status = if authority
            .current_summary
            .completed_item_ids
            .contains(&item.id)
        {
            Some("completed")
        } else if authority.current_summary.next_item_id.as_ref() == Some(&item.id) {
            Some("in_progress")
        } else {
            None
        };
        if expected_status.is_some_and(|expected| item.status != expected)
            || (!authority
                .current_summary
                .remaining_item_ids
                .contains(&item.id)
                && expected_status.is_none())
        {
            issues.push(issue(
                "current_product_campaign_state_mismatch",
                format!(
                    "{} campaign state diverges from the completed/next/remaining summary",
                    item.id
                ),
            ));
        }
        if expected_status.is_none()
            && !matches!(
                item.status.as_str(),
                "planned" | "implemented_pending_evidence"
            )
        {
            issues.push(issue(
                "current_product_campaign_state_mismatch",
                format!(
                    "{} is remaining after the next item and must be planned or implemented_pending_evidence",
                    item.id
                ),
            ));
        }

        let expected_ref = format!(
            "contracts/spec/solo-dogfood-readiness-v0.yaml#implementation_decisions.delivery_sequence[{}]",
            item.id
        );
        let checkpoint = &item.checkpoint;
        let base_commit_is_valid = checkpoint.base_commit.len() == 40
            && checkpoint
                .base_commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        let updated_at_is_valid = checkpoint.updated_at.len() == 10
            && checkpoint.updated_at.as_bytes().get(4) == Some(&b'-')
            && checkpoint.updated_at.as_bytes().get(7) == Some(&b'-');
        if checkpoint.kind != "campaign-item-checkpoint"
            || checkpoint.state_ref != expected_ref
            || checkpoint.evidence_refs.is_empty()
            || !base_commit_is_valid
            || !updated_at_is_valid
            || (item.status == "completed" && !checkpoint.remaining_work.is_empty())
            || (item.status != "completed" && checkpoint.remaining_work.is_empty())
        {
            issues.push(issue(
                "current_product_campaign_checkpoint_invalid",
                format!("{} has an incomplete or inconsistent checkpoint", item.id),
            ));
        }
    }
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
                    "core" | "host_client" | "compatibility" | "tooling"
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

    struct ScratchRoot(PathBuf);

    impl Drop for ScratchRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn workspace_contract_fixture(label: &str) -> ScratchRoot {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forge-workspace-architecture-{label}-{}-{nonce}",
            std::process::id()
        ));
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for relative in [
            RUST_CORE_PATH,
            BOUNDARIES_PATH,
            POLICY_PATH,
            SOLO_SPEC_PATH,
            PRODUCT_PLAN_PATH,
            PRODUCT_CAMPAIGN_PATH,
            PRODUCT_INVENTORY_PATH,
        ] {
            let destination = root.join(relative);
            fs::create_dir_all(destination.parent().expect("fixture parent"))
                .expect("create fixture directory");
            fs::copy(source.join(relative), destination).expect("copy workspace contract");
        }
        ScratchRoot(root)
    }

    fn yaml_inline_strings(values: &[String]) -> String {
        format!(
            "[{}]",
            values
                .iter()
                .map(|value| format!("\"{value}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    fn yaml_optional_string(value: Option<&String>) -> String {
        value.map_or_else(|| "null".to_string(), |item| format!("\"{item}\""))
    }

    fn replace_once(text: &str, from: &str, to: &str, label: &str) -> String {
        let changed = text.replacen(from, to, 1);
        assert_ne!(changed, text, "{label} must change the fixture");
        changed
    }

    fn replace_first_after(text: String, anchor: &str, from: &str, to: &str) -> String {
        let anchor_offset = text.find(anchor).expect("fixture anchor");
        let value_offset = text[anchor_offset..]
            .find(from)
            .map(|offset| anchor_offset + offset)
            .expect("fixture value after anchor");
        let mut changed = text;
        changed.replace_range(value_offset..value_offset + from.len(), to);
        changed
    }

    fn write_terminal_summary(path: &Path, authority: &CurrentProductAuthorityProjection) {
        let text = fs::read_to_string(path).expect("read authority projection");
        let current = &authority.current_summary;
        let current_next = current
            .next_item_id
            .as_ref()
            .map_or_else(|| "null".to_string(), |item| format!("\"{item}\""));
        let terminal = text
            .replacen(
                &format!(
                    "    completed_item_ids: {}",
                    yaml_inline_strings(&current.completed_item_ids)
                ),
                &format!(
                    "    completed_item_ids: {}",
                    yaml_inline_strings(&authority.executable_item_ids)
                ),
                1,
            )
            .replacen(
                &format!("    next_item_id: {current_next}"),
                "    next_item_id: null",
                1,
            )
            .replacen(
                &format!(
                    "    remaining_item_ids: {}",
                    yaml_inline_strings(&current.remaining_item_ids)
                ),
                "    remaining_item_ids: []",
                1,
            );
        assert_ne!(terminal, text, "fixture authority summary must change");
        fs::write(path, terminal).expect("write terminal authority projection");
    }

    fn cargo_metadata_fixture(root: &Path, extra_dependency: Option<(&str, &str)>) -> Vec<u8> {
        let policy: PolicyDocument = load(root, POLICY_PATH).expect("policy fixture");
        let declared = policy
            .workspace_crate_boundary
            .crates
            .into_iter()
            .map(|crate_policy| (crate_policy.name, crate_policy.depends_on))
            .collect::<BTreeMap<_, _>>();
        let packages = forge_core_contracts::WORKSPACE_CRATE_BOUNDARIES
            .iter()
            .map(|boundary| {
                let name = boundary.id.package_name();
                let production_dependencies = boundary
                    .dependencies
                    .iter()
                    .map(|dependency| dependency.package_name())
                    .collect::<BTreeSet<_>>();
                let mut dependencies = declared[name]
                    .iter()
                    .map(|dependency| {
                        serde_json::json!({
                            "name": dependency,
                            "kind": if production_dependencies.contains(dependency.as_str()) {
                                serde_json::Value::Null
                            } else {
                                serde_json::Value::String("dev".to_string())
                            }
                        })
                    })
                    .collect::<Vec<_>>();
                if let Some((from, to)) = extra_dependency {
                    if name == from {
                        dependencies.push(serde_json::json!({ "name": to, "kind": null }));
                    }
                }
                serde_json::json!({
                    "id": format!("path+file:///workspace/{name}#0.0.0"),
                    "name": name,
                    "manifest_path": root.join(boundary.path).join("Cargo.toml"),
                    "dependencies": dependencies
                })
            })
            .collect::<Vec<_>>();
        let workspace_members = packages
            .iter()
            .map(|package| package["id"].clone())
            .collect::<Vec<_>>();
        serde_json::to_vec(&serde_json::json!({
            "packages": packages,
            "workspace_members": workspace_members
        }))
        .expect("metadata fixture")
    }

    #[test]
    fn checked_in_architecture_is_exhaustive_hostless_and_reviewed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = validate_workspace_architecture_contracts(root).expect("contracts load");
        assert!(report.is_clean(), "{:?}", report.issues);
        assert_eq!(report.selected_host, "none");
        assert_eq!(report.declared_crates.len(), EXPECTED_CRATE_COUNT);
    }

    #[test]
    fn current_product_authority_rejects_an_internally_stale_summary() {
        let fixture = workspace_contract_fixture("stale-summary");
        let spec_path = fixture.0.join(SOLO_SPEC_PATH);
        let authority: CurrentProductAuthorityDocument =
            load(&fixture.0, SOLO_SPEC_PATH).expect("load Solo authority");
        let summary = &authority.current_product_authority.current_summary;
        let divergent_next = authority
            .current_product_authority
            .executable_item_ids
            .iter()
            .find(|item| Some(*item) != summary.next_item_id.as_ref())
            .expect("an executable item different from the current next item");
        let spec = fs::read_to_string(&spec_path).expect("read Solo specification");
        let stale = replace_once(
            &spec,
            &format!(
                "    next_item_id: {}",
                yaml_optional_string(summary.next_item_id.as_ref())
            ),
            &format!("    next_item_id: \"{divergent_next}\""),
            "stale next item",
        );
        fs::write(spec_path, stale).expect("write stale Solo summary");

        let report = validate_workspace_architecture_contracts(&fixture.0).expect("contracts load");
        assert!(
            report.issues.iter().any(|issue| {
                issue.code == "current_product_summary_invalid"
                    && issue.detail.contains("next item")
            }),
            "{:?}",
            report.issues
        );
    }

    #[test]
    fn current_product_authority_rejects_stale_plan_order() {
        let fixture = workspace_contract_fixture("stale-plan-order");
        let plan_path = fixture.0.join(PRODUCT_PLAN_PATH);
        let authority: CurrentProductAuthorityDocument =
            load(&fixture.0, SOLO_SPEC_PATH).expect("load Solo authority");
        let plan_document: ProductPlanDocument =
            load(&fixture.0, PRODUCT_PLAN_PATH).expect("load product plan");
        let active_slice = plan_document
            .first_executable_slice
            .as_ref()
            .expect("current fixture has an active slice");
        let divergent_item = authority
            .current_product_authority
            .executable_item_ids
            .iter()
            .find(|item| **item != active_slice.first_item)
            .expect("an executable item different from the active slice");
        let plan = fs::read_to_string(&plan_path).expect("read product plan");
        let stale = replace_once(
            &plan,
            &format!("  first_item: \"{}\"", active_slice.first_item),
            &format!("  first_item: \"{divergent_item}\""),
            "stale plan order",
        );
        fs::write(plan_path, stale).expect("write stale product plan");

        let report = validate_workspace_architecture_contracts(&fixture.0).expect("contracts load");
        assert!(
            report.issues.iter().any(|issue| {
                issue.code == "current_product_plan_order_mismatch"
                    && issue.detail.contains("next item")
            }),
            "{:?}",
            report.issues
        );
    }

    #[test]
    fn current_product_authority_rejects_active_plan_after_terminal_summary() {
        let fixture = workspace_contract_fixture("terminal-active-plan");
        let spec: CurrentProductAuthorityDocument =
            load(&fixture.0, SOLO_SPEC_PATH).expect("load Solo authority");
        for relative in [
            SOLO_SPEC_PATH,
            PRODUCT_PLAN_PATH,
            PRODUCT_CAMPAIGN_PATH,
            PRODUCT_INVENTORY_PATH,
        ] {
            write_terminal_summary(&fixture.0.join(relative), &spec.current_product_authority);
        }

        let report = validate_workspace_architecture_contracts(&fixture.0).expect("contracts load");
        assert!(
            report.issues.iter().any(|issue| {
                issue.code == "current_product_plan_order_mismatch"
                    && issue.detail.contains("terminal")
            }),
            "{:?}",
            report.issues
        );
    }

    #[test]
    fn current_product_authority_rejects_campaign_state_that_diverges_from_summary() {
        let fixture = workspace_contract_fixture("campaign-state");
        let campaign_path = fixture.0.join(PRODUCT_CAMPAIGN_PATH);
        let authority: CurrentProductAuthorityDocument =
            load(&fixture.0, SOLO_SPEC_PATH).expect("load Solo authority");
        let campaign_document: ProductCampaignDocument =
            load(&fixture.0, PRODUCT_CAMPAIGN_PATH).expect("load campaign");
        let next_item = authority
            .current_product_authority
            .current_summary
            .next_item_id
            .as_ref()
            .expect("current fixture has a next item");
        let active_item = campaign_document
            .solo_execution
            .items
            .iter()
            .find(|item| &item.id == next_item)
            .expect("campaign item for the current next item");
        let campaign = fs::read_to_string(&campaign_path).expect("read campaign");
        let divergent = replace_first_after(
            campaign,
            &format!("    - id: \"{}\"", active_item.id),
            &format!("      status: \"{}\"", active_item.status),
            "      status: \"completed\"",
        );
        fs::write(campaign_path, divergent).expect("write divergent campaign");

        let report = validate_workspace_architecture_contracts(&fixture.0).expect("contracts load");
        assert!(
            report.issues.iter().any(|issue| {
                issue.code == "current_product_campaign_state_mismatch"
                    && issue.detail.contains(&active_item.id)
            }),
            "{:?}",
            report.issues
        );
    }

    #[test]
    fn current_product_authority_rejects_divergent_solo_execution_authority() {
        let fixture = workspace_contract_fixture("campaign-authority");
        let campaign_path = fixture.0.join(PRODUCT_CAMPAIGN_PATH);
        let campaign = fs::read_to_string(&campaign_path).expect("read campaign");
        let expected = format!("{PRODUCT_CAMPAIGN_PATH}#solo_execution");
        let divergent = replace_once(
            &campaign,
            &format!("  authority: \"{expected}\""),
            "  authority: \"contracts/plan/not-the-campaign.yaml#solo_execution\"",
            "divergent Solo execution authority",
        );
        fs::write(campaign_path, divergent).expect("write divergent campaign authority");

        let report = validate_workspace_architecture_contracts(&fixture.0).expect("contracts load");
        assert!(
            report
                .issues
                .iter()
                .any(|issue| { issue.code == "current_product_campaign_authority_mismatch" }),
            "{:?}",
            report.issues
        );
    }

    #[test]
    fn cargo_metadata_rejects_a_real_dependency_outside_the_boundary() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let metadata =
            cargo_metadata_fixture(&root, Some(("forge-core-contracts", "forge-core-kernel")));
        let issues = validate_cargo_metadata_workspace(&root, &metadata).expect("metadata parses");
        assert!(issues.iter().any(|issue| {
            issue.code == "cargo_workspace_boundary_violation"
                && issue.detail.contains("UndeclaredDependencyEdge")
        }));
    }

    #[test]
    fn cargo_metadata_accepts_the_canonical_workspace_projection() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let metadata = cargo_metadata_fixture(&root, None);
        let issues = validate_cargo_metadata_workspace(&root, &metadata).expect("metadata parses");
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn checked_in_cargo_workspace_matches_the_canonical_boundary() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = std::process::Command::new("cargo")
            .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
            .current_dir(&root)
            .output()
            .expect("run cargo metadata");
        assert!(
            output.status.success(),
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let issues =
            validate_cargo_metadata_workspace(&root, &output.stdout).expect("metadata parses");
        assert!(issues.is_empty(), "{issues:?}");
    }
}
