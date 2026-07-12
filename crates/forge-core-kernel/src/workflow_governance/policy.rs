//! Repository-admitted workflow-governance release loading.
//!
//! Raw registry YAML is deliberately non-authoritative.  This module reads one
//! fixed embedded registry, resolves only its embedded references, validates
//! the complete closed chain, and then mints opaque admitted release values.

use forge_core_contracts::{
    StableId, WorkflowGovernanceBundleDocument, WorkflowGovernanceReleaseIdentity,
    WorkflowGovernanceReleaseRegistryDocument, WorkflowGovernanceReleaseRegistryEntry,
    WorkflowReceiptCarryover, WorkflowReleaseAdmissionProof, WorkflowReleaseRegistryAuthority,
    WorkflowReleaseRegistryProvenance, WorkflowReleaseRegistrySource,
    WorkflowRuntimeBundleIdentity,
};
use forge_core_decisions::{
    embedded_text, evaluate_workflow_release_registry, validate_workflow_governance_bundle,
    workflow_policy_set_digest, workflow_runtime_bundle_digest,
    WorkflowReleaseRegistryEvaluationAuthority, WorkflowReleaseRegistryEvaluationStatus,
    WorkflowReleaseRegistryIssue,
};
use forge_core_store::sha256_content_hash;

pub const ADMITTED_GOLDEN_PATH_BUNDLE_REF: &str =
    "contracts/workflow-governance/golden-path-v0.yaml";
pub const ADMITTED_WORKFLOW_RELEASE_REGISTRY_REF: &str =
    "contracts/migration/workflow-governance-release-registry-v0.yaml";

/// Opaque repository-admitted policy bundle retained for P5c API compatibility.
///
/// No public constructor or serde implementation exists.
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
}

/// One release admitted by the kernel-owned embedded registry loader.
///
/// It intentionally has no public constructor and implements neither `Clone`
/// nor serde. A raw registry, manifest, bundle, event, or audit projection
/// cannot be converted into this authority.
///
/// ```compile_fail
/// use forge_core_kernel::AdmittedWorkflowGovernanceRelease;
/// fn clone_release(value: AdmittedWorkflowGovernanceRelease) {
///     let _ = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use forge_core_kernel::AdmittedWorkflowGovernanceRelease;
/// let _: AdmittedWorkflowGovernanceRelease = serde_json::from_str("{}").unwrap();
/// ```
pub struct AdmittedWorkflowGovernanceRelease {
    release: WorkflowGovernanceReleaseIdentity,
    runtime_bundle: WorkflowRuntimeBundleIdentity,
    bundle: WorkflowGovernanceBundleDocument,
    predecessor_release_id: Option<StableId>,
    predecessor_release_digest: Option<String>,
    receipt_carryover: WorkflowReceiptCarryover,
}

impl AdmittedWorkflowGovernanceRelease {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.runtime_bundle.bundle_id.0
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.runtime_bundle.bundle_digest
    }

    #[must_use]
    pub const fn release(&self) -> &WorkflowGovernanceReleaseIdentity {
        &self.release
    }

    #[must_use]
    pub const fn runtime_bundle(&self) -> &WorkflowRuntimeBundleIdentity {
        &self.runtime_bundle
    }

    #[must_use]
    pub const fn receipt_carryover(&self) -> WorkflowReceiptCarryover {
        self.receipt_carryover
    }

    pub(crate) const fn document(&self) -> &WorkflowGovernanceBundleDocument {
        &self.bundle
    }

    pub(crate) fn is_adjacent_successor_of(&self, source: &Self) -> bool {
        self.release.lineage_id == source.release.lineage_id
            && self.predecessor_release_id.as_ref() == Some(&source.release.release_id)
            && self.predecessor_release_digest.as_deref()
                == Some(source.release.release_digest.as_str())
    }
}

/// Opaque installed registry. Its public observations are audit data only.
pub struct AdmittedWorkflowGovernanceReleaseRegistry {
    registry_id: StableId,
    registry_version: String,
    registry_digest: String,
    releases: Vec<AdmittedWorkflowGovernanceRelease>,
}

impl AdmittedWorkflowGovernanceReleaseRegistry {
    #[must_use]
    pub fn registry_provenance(&self) -> WorkflowReleaseRegistryProvenance {
        WorkflowReleaseRegistryProvenance {
            registry_id: self.registry_id.clone(),
            registry_version: self.registry_version.clone(),
            registry_digest: self.registry_digest.clone(),
        }
    }

    #[must_use]
    pub fn registry_digest(&self) -> &str {
        &self.registry_digest
    }

    pub(crate) fn genesis(&self) -> &AdmittedWorkflowGovernanceRelease {
        &self.releases[0]
    }

    pub(crate) fn release_by_id(
        &self,
        release_id: &StableId,
    ) -> Option<&AdmittedWorkflowGovernanceRelease> {
        self.releases
            .iter()
            .find(|release| release.release.release_id == *release_id)
    }

    pub(crate) fn adjacent_successor(
        &self,
        source: &AdmittedWorkflowGovernanceRelease,
    ) -> Option<&AdmittedWorkflowGovernanceRelease> {
        self.releases
            .iter()
            .find(|candidate| candidate.is_adjacent_successor_of(source))
    }

    pub(crate) fn admission_proof(
        &self,
        source: &AdmittedWorkflowGovernanceRelease,
        target: &AdmittedWorkflowGovernanceRelease,
        snapshot_digest: &str,
    ) -> Result<WorkflowReleaseAdmissionProof, AdmittedWorkflowGovernanceReleaseError> {
        Self::admission_proof_with_provenance(
            &self.registry_provenance(),
            source,
            target,
            snapshot_digest,
        )
    }

    pub(crate) fn admission_proof_with_provenance(
        provenance: &WorkflowReleaseRegistryProvenance,
        source: &AdmittedWorkflowGovernanceRelease,
        target: &AdmittedWorkflowGovernanceRelease,
        snapshot_digest: &str,
    ) -> Result<WorkflowReleaseAdmissionProof, AdmittedWorkflowGovernanceReleaseError> {
        let proof_id = StableId(format!(
            "proof.workflow-governance.release-admission.{}",
            target.release.release_id.0
        ));
        let canonical = serde_json_canonicalizer::to_vec(&(
            &proof_id,
            provenance,
            &source.release,
            &source.runtime_bundle,
            &target.release,
            &target.runtime_bundle,
            target.receipt_carryover,
            snapshot_digest,
        ))
        .map_err(|error| AdmittedWorkflowGovernanceReleaseError::Canonicalize(error.to_string()))?;
        Ok(WorkflowReleaseAdmissionProof {
            proof_id,
            proof_digest: sha256_content_hash(&canonical),
            snapshot_digest: snapshot_digest.to_owned(),
            from_policy_set_digest: source.runtime_bundle.policy_set_digest.clone(),
            to_policy_set_digest: target.runtime_bundle.policy_set_digest.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmittedWorkflowGovernanceBundleError {
    EmbeddedBundleMissing,
    Parse(String),
    Invalid(Vec<forge_core_decisions::WorkflowGovernanceIssue>),
    Canonicalize(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmittedWorkflowGovernanceReleaseError {
    EmbeddedRegistryMissing,
    RegistryParse(String),
    EmbeddedBundleMissing(String),
    BundleParse { reference: String, source: String },
    RegistryInvalid(Vec<WorkflowReleaseRegistryIssue>),
    RegistryAuthorityMismatch,
    RegistryShapeMismatch,
    RuntimeBundleInvalid(String),
    RuntimeBundleDigestMismatch(String),
    PolicySetDigestMismatch(String),
    UnsupportedEntryAuthority,
    UnsupportedEntrySource,
    Canonicalize(String),
}

/// Load the exact P5c golden-path bundle.
///
/// # Errors
/// Returns a typed error when the fixed embedded artifact is unavailable or
/// invalid. Project-local files are never consulted.
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
    let digest = workflow_runtime_bundle_digest(&document)
        .map_err(AdmittedWorkflowGovernanceBundleError::Canonicalize)?;
    Ok(AdmittedWorkflowGovernanceBundle { document, digest })
}

/// Admit the sole fixed embedded release registry.
///
/// # Errors
/// Fails closed unless the registry describes exactly the P5c genesis and its
/// one policy-equivalent adjacent foundation successor. Raw evaluation remains
/// non-authoritative; only this private-field loader mints admitted releases.
pub fn load_admitted_workflow_governance_release_registry(
) -> Result<AdmittedWorkflowGovernanceReleaseRegistry, AdmittedWorkflowGovernanceReleaseError> {
    let raw = embedded_text(ADMITTED_WORKFLOW_RELEASE_REGISTRY_REF)
        .ok_or(AdmittedWorkflowGovernanceReleaseError::EmbeddedRegistryMissing)?;
    let document: WorkflowGovernanceReleaseRegistryDocument =
        yaml_serde::from_str(raw).map_err(|error| {
            AdmittedWorkflowGovernanceReleaseError::RegistryParse(error.to_string())
        })?;
    let mut bundles =
        Vec::with_capacity(document.workflow_governance_release_registry.releases.len());
    for entry in &document.workflow_governance_release_registry.releases {
        bundles.push(load_entry_bundle(entry)?);
    }
    let evaluation = evaluate_workflow_release_registry(&document, &bundles);
    if evaluation.status != WorkflowReleaseRegistryEvaluationStatus::StructurallyValid
        || !evaluation.issues.is_empty()
    {
        return Err(AdmittedWorkflowGovernanceReleaseError::RegistryInvalid(
            evaluation.issues,
        ));
    }
    if evaluation.authority != WorkflowReleaseRegistryEvaluationAuthority::NonAuthoritative {
        return Err(AdmittedWorkflowGovernanceReleaseError::RegistryAuthorityMismatch);
    }
    let registry = document.workflow_governance_release_registry;
    if registry.releases.len() != 2
        || evaluation.successor_policy_count != 15
        || registry.releases[0].predecessor.is_some()
        || registry.default_successor_release_id != registry.releases[1].release.release_id
    {
        return Err(AdmittedWorkflowGovernanceReleaseError::RegistryShapeMismatch);
    }
    let mut releases = Vec::with_capacity(2);
    for (entry, bundle) in registry.releases.into_iter().zip(bundles) {
        releases.push(admit_entry(entry, bundle)?);
    }
    let genesis = &releases[0];
    let successor = &releases[1];
    if !matches!(
        genesis.receipt_carryover,
        WorkflowReceiptCarryover::NotApplicable
    ) || !matches!(
        successor.receipt_carryover,
        WorkflowReceiptCarryover::PreservePolicyEquivalent
    ) || !successor.is_adjacent_successor_of(genesis)
        || genesis.runtime_bundle.policy_set_digest != successor.runtime_bundle.policy_set_digest
        || genesis.bundle.workflow_governance_bundle.policies
            != successor.bundle.workflow_governance_bundle.policies
        || registry.registry_id == genesis.release.release_id
        || registry.registry_id == successor.release.release_id
        || registry.registry_id == genesis.runtime_bundle.bundle_id
        || registry.registry_id == successor.runtime_bundle.bundle_id
        || genesis.release.release_id == genesis.runtime_bundle.bundle_id
        || successor.release.release_id == successor.runtime_bundle.bundle_id
        || evaluation.registry_digest == genesis.release.release_digest
        || evaluation.registry_digest == successor.release.release_digest
        || evaluation.registry_digest == genesis.runtime_bundle.bundle_digest
        || evaluation.registry_digest == successor.runtime_bundle.bundle_digest
        || genesis.release.release_digest == genesis.runtime_bundle.bundle_digest
        || successor.release.release_digest == successor.runtime_bundle.bundle_digest
    {
        return Err(AdmittedWorkflowGovernanceReleaseError::RegistryShapeMismatch);
    }
    Ok(AdmittedWorkflowGovernanceReleaseRegistry {
        registry_id: registry.registry_id,
        registry_version: registry.registry_version,
        registry_digest: evaluation.registry_digest,
        releases,
    })
}

fn load_entry_bundle(
    entry: &WorkflowGovernanceReleaseRegistryEntry,
) -> Result<WorkflowGovernanceBundleDocument, AdmittedWorkflowGovernanceReleaseError> {
    let reference = entry.runtime_bundle.embedded_ref.0.as_str();
    let raw = embedded_text(reference).ok_or_else(|| {
        AdmittedWorkflowGovernanceReleaseError::EmbeddedBundleMissing(reference.to_owned())
    })?;
    yaml_serde::from_str(raw).map_err(
        |error| AdmittedWorkflowGovernanceReleaseError::BundleParse {
            reference: reference.to_owned(),
            source: error.to_string(),
        },
    )
}

fn admit_entry(
    entry: WorkflowGovernanceReleaseRegistryEntry,
    bundle: WorkflowGovernanceBundleDocument,
) -> Result<AdmittedWorkflowGovernanceRelease, AdmittedWorkflowGovernanceReleaseError> {
    if entry.authority != WorkflowReleaseRegistryAuthority::CandidateOnly {
        return Err(AdmittedWorkflowGovernanceReleaseError::UnsupportedEntryAuthority);
    }
    if !matches!(
        entry.source,
        WorkflowReleaseRegistrySource::ImplicitP5cGenesis
            | WorkflowReleaseRegistrySource::EmbeddedManifest { .. }
    ) {
        return Err(AdmittedWorkflowGovernanceReleaseError::UnsupportedEntrySource);
    }
    let issues = validate_workflow_governance_bundle(&bundle);
    if !issues.is_empty() {
        return Err(
            AdmittedWorkflowGovernanceReleaseError::RuntimeBundleInvalid(
                entry.release.release_id.0,
            ),
        );
    }
    let bundle_digest = workflow_runtime_bundle_digest(&bundle)
        .map_err(AdmittedWorkflowGovernanceReleaseError::Canonicalize)?;
    // `expected_digest` binds the exact embedded YAML bytes and is already
    // checked by the registry evaluator. Runtime ledger identity is the JCS
    // digest of the parsed typed bundle and must remain formatting-independent.
    if bundle_digest != entry.runtime_bundle.identity.bundle_digest
        || bundle.workflow_governance_bundle.id != entry.runtime_bundle.identity.bundle_id
    {
        return Err(
            AdmittedWorkflowGovernanceReleaseError::RuntimeBundleDigestMismatch(
                entry.release.release_id.0,
            ),
        );
    }
    let policy_set_digest = workflow_policy_set_digest(&bundle.workflow_governance_bundle.policies)
        .map_err(AdmittedWorkflowGovernanceReleaseError::Canonicalize)?;
    if policy_set_digest != entry.runtime_bundle.identity.policy_set_digest {
        return Err(
            AdmittedWorkflowGovernanceReleaseError::PolicySetDigestMismatch(
                entry.release.release_id.0,
            ),
        );
    }
    let (predecessor_release_id, predecessor_release_digest) =
        entry.predecessor.map_or((None, None), |predecessor| {
            (
                Some(predecessor.release_id),
                Some(predecessor.release_digest),
            )
        });
    Ok(AdmittedWorkflowGovernanceRelease {
        release: entry.release,
        runtime_bundle: entry.runtime_bundle.identity,
        bundle,
        predecessor_release_id,
        predecessor_release_digest,
        receipt_carryover: entry.receipt_carryover,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_registry_admits_only_genesis_and_policy_equivalent_successor() {
        let registry = load_admitted_workflow_governance_release_registry().expect("registry");
        let genesis = registry.genesis();
        let successor = registry
            .adjacent_successor(genesis)
            .expect("foundation successor");
        assert_eq!(
            genesis.document().workflow_governance_bundle.policies.len(),
            15
        );
        assert_eq!(
            successor
                .document()
                .workflow_governance_bundle
                .policies
                .len(),
            15
        );
        assert_ne!(
            genesis.release.release_digest,
            genesis.runtime_bundle.bundle_digest
        );
        assert_ne!(
            successor.release.release_digest,
            successor.runtime_bundle.bundle_digest
        );
        assert_ne!(registry.registry_digest(), successor.release.release_digest);
        assert!(successor.is_adjacent_successor_of(genesis));
        assert_eq!(
            genesis.runtime_bundle.policy_set_digest,
            successor.runtime_bundle.policy_set_digest
        );
    }

    #[test]
    fn p5c_bundle_loader_remains_exactly_the_registry_genesis() {
        let p5c = load_admitted_workflow_governance_bundle().expect("P5c bundle");
        let registry = load_admitted_workflow_governance_release_registry().expect("registry");
        assert_eq!(p5c.id(), registry.genesis().runtime_bundle.bundle_id.0);
        assert_eq!(
            p5c.digest(),
            registry.genesis().runtime_bundle.bundle_digest
        );
    }

    #[test]
    fn admission_proof_remains_verifiable_from_historical_registry_provenance() {
        let registry = load_admitted_workflow_governance_release_registry().expect("registry");
        let historical = WorkflowReleaseRegistryProvenance {
            registry_id: registry.registry_provenance().registry_id,
            registry_version: "0.0.9".to_owned(),
            registry_digest: format!("sha256:{}", "7".repeat(64)),
        };
        let first = AdmittedWorkflowGovernanceReleaseRegistry::admission_proof_with_provenance(
            &historical,
            registry.genesis(),
            registry
                .adjacent_successor(registry.genesis())
                .expect("foundation successor"),
            &format!("sha256:{}", "8".repeat(64)),
        )
        .expect("historical proof");
        let second = AdmittedWorkflowGovernanceReleaseRegistry::admission_proof_with_provenance(
            &historical,
            registry.genesis(),
            registry
                .adjacent_successor(registry.genesis())
                .expect("foundation successor"),
            &first.snapshot_digest,
        )
        .expect("historical proof replay");
        assert_eq!(first, second);
        assert_ne!(
            first,
            registry
                .admission_proof(
                    registry.genesis(),
                    registry
                        .adjacent_successor(registry.genesis())
                        .expect("foundation successor"),
                    &first.snapshot_digest,
                )
                .expect("current proof")
        );
    }
}
