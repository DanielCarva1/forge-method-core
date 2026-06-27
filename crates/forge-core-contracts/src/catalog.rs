use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Serde document wrapper for the workflow catalog on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CatalogDocument {
    pub schema_version: String,
    pub catalog: Catalog,
}

/// The typed workflow catalog. The orchestrator-guide (slice 2) classifies
/// human intent against the catalog's [`CatalogEntry::triggers`] and dispatches
/// onto the matching workflow. This is the routing surface DC1 is built on.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Catalog {
    #[serde(default)]
    pub entries: Vec<CatalogEntry>,
}

impl Catalog {
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up an entry by canonical id.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&CatalogEntry> {
        self.entries.iter().find(|e| e.id.0 == id)
    }
}

/// A single routing entry in the workflow catalog.
///
/// Derived from (and kept in sync with) a [`crate::workflow::Workflow`], but
/// flattened to only the fields the orchestrator needs to route intent:
/// identity, phase, the workflow file it points at, and the trigger /
/// prerequisite / output signals used for matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CatalogEntry {
    /// Canonical workflow id (matches `Workflow::id`).
    pub id: StableId,
    /// Funnel phase tags this workflow is eligible in (mirrors `Workflow::phases`).
    #[serde(default)]
    pub phases: Vec<StableId>,
    /// Repo-relative path to the workflow document this entry routes to.
    pub workflow_ref: RepoPath,
    /// Predicate expressions used to match incoming intent (mirrors
    /// `Workflow::trigger`). The orchestrator scores entries on these.
    #[serde(default)]
    pub triggers: Vec<String>,
    /// Prerequisites that must hold before this workflow can run.
    #[serde(default)]
    pub prerequisites: Vec<String>,
    /// Outputs the workflow is expected to produce (for downstream matching).
    #[serde(default)]
    pub outputs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_find_and_empty() {
        let empty = Catalog::default();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);

        let mut cat = Catalog::default();
        cat.entries.push(CatalogEntry {
            id: StableId("plan-sprint".into()),
            phases: vec![StableId("3-plan".into())],
            workflow_ref: RepoPath("contracts/workflows/plan-sprint.yaml".into()),
            triggers: vec!["state.phase == 3-plan".into()],
            prerequisites: vec!["specification artifact exists".into()],
            outputs: vec!["sprint plan artifact".into()],
        });
        assert_eq!(cat.len(), 1);
        assert!(cat.find("plan-sprint").is_some());
        assert!(cat.find("nope").is_none());
    }
}
