use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Canonical file name for a consumer repo's pointer to its Forge Runtime Sidecar.
pub const PROJECT_LINK_FILE_NAME: &str = ".forge-method.yaml";

/// Schema version for the sidecar pointer contract.
pub const PROJECT_LINK_SCHEMA_VERSION: &str = "forge_project_link_v1";

/// A consumer project pointer to its Forge Runtime Sidecar.
///
/// This document lives in the consumer project repo root. It is intentionally
/// small: runtime state stays in the sidecar, not inside the consumer repo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectLinkDocument {
    /// Must be [`PROJECT_LINK_SCHEMA_VERSION`].
    pub schema_version: String,
    /// Stable product/project id used by the sidecar state.
    pub project_id: StableId,
    /// Sidecar directory, normally `../forge-<project-id>`.
    pub sidecar_root: RepoPath,
    /// Forge runtime state directory, normally `../forge-<project-id>/.forge-method`.
    pub state_root: RepoPath,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_link_round_trips_yaml() {
        let raw = "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n";
        let doc: ProjectLinkDocument = yaml_serde::from_str(raw).unwrap();
        assert_eq!(doc.schema_version, PROJECT_LINK_SCHEMA_VERSION);
        assert_eq!(doc.project_id.0, "app");
        let serialized = yaml_serde::to_string(&doc).unwrap();
        let reparsed: ProjectLinkDocument = yaml_serde::from_str(&serialized).unwrap();
        assert_eq!(reparsed, doc);
    }

    #[test]
    fn project_link_rejects_unknown_fields() {
        let raw = "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\nextra: nope\n";
        let err = yaml_serde::from_str::<ProjectLinkDocument>(raw).unwrap_err();
        assert!(err.to_string().contains("extra"));
    }

    #[test]
    fn project_link_requires_state_root() {
        let raw =
            "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\n";
        let err = yaml_serde::from_str::<ProjectLinkDocument>(raw).unwrap_err();
        assert!(err.to_string().contains("state_root"));
    }
}
