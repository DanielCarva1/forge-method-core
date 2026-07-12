//! Repository-admitted workflow-governance policy loading.
//!
//! The trusted lane never accepts a caller-selected bundle path. The canonical
//! P5c bundle is compiled into the binary, semantically validated, canonicalized,
//! and content-addressed before the Adapter can use it.

use forge_core_contracts::WorkflowGovernanceBundleDocument;
use forge_core_decisions::{
    embedded_text, validate_workflow_governance_bundle, WorkflowGovernanceIssue,
};
use forge_core_store::sha256_content_hash;

pub const ADMITTED_GOLDEN_PATH_BUNDLE_REF: &str =
    "contracts/workflow-governance/golden-path-v0.yaml";

/// Opaque repository-admitted policy bundle.
///
/// No public constructor or serde implementation exists. In particular, a
/// bundle parsed from caller YAML cannot be converted into this type.
pub struct AdmittedWorkflowGovernanceBundle {
    document: WorkflowGovernanceBundleDocument,
    digest: String,
}

impl AdmittedWorkflowGovernanceBundle {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.document.workflow_governance_bundle.id.0
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub(crate) const fn document(&self) -> &WorkflowGovernanceBundleDocument {
        &self.document
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmittedWorkflowGovernanceBundleError {
    EmbeddedBundleMissing,
    Parse(String),
    Invalid(Vec<WorkflowGovernanceIssue>),
    Canonicalize(String),
}

/// Load the sole repository-admitted P5c golden-path bundle.
///
/// # Errors
/// Returns a typed error when the embedded build artifact is unavailable,
/// malformed, semantically invalid, or cannot be canonicalized.
pub fn load_admitted_workflow_governance_bundle(
) -> Result<AdmittedWorkflowGovernanceBundle, AdmittedWorkflowGovernanceBundleError> {
    let raw = embedded_text(ADMITTED_GOLDEN_PATH_BUNDLE_REF)
        .ok_or(AdmittedWorkflowGovernanceBundleError::EmbeddedBundleMissing)?;
    let document: WorkflowGovernanceBundleDocument = yaml_serde::from_str(raw)
        .map_err(|error| AdmittedWorkflowGovernanceBundleError::Parse(error.to_string()))?;
    let issues = validate_workflow_governance_bundle(&document);
    if !issues.is_empty() {
        return Err(AdmittedWorkflowGovernanceBundleError::Invalid(issues));
    }
    let canonical = serde_json_canonicalizer::to_vec(&document)
        .map_err(|error| AdmittedWorkflowGovernanceBundleError::Canonicalize(error.to_string()))?;
    Ok(AdmittedWorkflowGovernanceBundle {
        document,
        digest: sha256_content_hash(&canonical),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admitted_bundle_is_embedded_valid_and_content_addressed() {
        let first = load_admitted_workflow_governance_bundle().expect("admitted bundle");
        let second = load_admitted_workflow_governance_bundle().expect("admitted bundle again");
        assert_eq!(first.id(), "bundle.workflow-governance.golden-path-v0");
        assert_eq!(first.digest(), second.digest());
        assert!(first.digest().starts_with("sha256:"));
        assert_eq!(first.digest().len(), 71);
        assert_eq!(
            first.document().workflow_governance_bundle.policies.len(),
            15
        );
    }
}
