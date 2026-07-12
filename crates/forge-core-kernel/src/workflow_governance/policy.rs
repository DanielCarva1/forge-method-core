//! Repository-admitted workflow-governance release loading.
//!
//! Raw registry YAML is deliberately non-authoritative.  This module reads one
//! fixed embedded registry, resolves only its embedded references, validates
//! the complete closed chain, and then mints opaque admitted release values.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::OnceLock,
};

use forge_core_authority::{
    verify_workflow_release_admission_authorization, VerifiedWorkflowReleaseAdmissionAuthorization,
};
use forge_core_contracts::{
    RepoPath, StableId, WorkflowBehavioralArtifactReference, WorkflowBehavioralCorpusSetDocument,
    WorkflowBehavioralCoveragePolicyDocument, WorkflowBehavioralReviewSubjectDocument,
    WorkflowBehavioralScenarioCorpusDocument, WorkflowBehavioralScenarioExecution,
    WorkflowBehavioralShadowReportDocument, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceReleaseIdentity, WorkflowGovernanceReleaseManifestDocument,
    WorkflowGovernanceReleaseRegistryDocument, WorkflowGovernanceReleaseRegistryEntry,
    WorkflowMigrationBatchDocument, WorkflowReceiptCarryover,
    WorkflowReleaseAdmissionAuthorizationDocument, WorkflowReleaseAdmissionProof,
    WorkflowReleaseRegistryAuthority, WorkflowReleaseRegistryProvenance,
    WorkflowReleaseRegistrySource, WorkflowReleaseReviewIndexDocument,
    WorkflowReleaseReviewerRegistryDocument, WorkflowRuntimeBundleIdentity,
};
use forge_core_decisions::{
    embedded_text, embedded_yaml_paths, evaluate_workflow_release_admission_candidate,
    evaluate_workflow_release_registry, validate_workflow_governance_bundle,
    workflow_policy_set_digest, workflow_runtime_bundle_digest, WorkflowBehavioralBundleInput,
    WorkflowBehavioralReportIdentity, WorkflowReleaseAdmissionCandidateInput,
    WorkflowReleaseAdmissionEvaluationStatus, WorkflowReleaseRegistryEvaluationAuthority,
    WorkflowReleaseRegistryEvaluationStatus, WorkflowReleaseRegistryIssue,
};
use forge_core_store::sha256_content_hash;

pub const ADMITTED_GOLDEN_PATH_BUNDLE_REF: &str =
    "contracts/workflow-governance/golden-path-v0.yaml";
pub const ADMITTED_WORKFLOW_RELEASE_REGISTRY_REF: &str =
    "contracts/migration/workflow-governance-release-registry-v0.yaml";
pub const REVIEWED_WORKFLOW_RELEASE_REGISTRY_REF: &str =
    "contracts/migration/workflow-governance-release-registry-core-assurance-v0.yaml";
pub const WORKFLOW_RELEASE_REVIEW_INDEX_REF: &str =
    "contracts/migration/workflow-core-assurance-review-index-v0.yaml";
pub const WORKFLOW_RELEASE_REVIEWER_REGISTRY_REF: &str =
    "contracts/policies/workflow-release-reviewer-registry-v0.yaml";
pub const WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_REF: &str =
    "contracts/migration/workflow-core-assurance-admission-authorization-v0.yaml";
pub const REVIEWED_WORKFLOW_RUNTIME_BUNDLE_REF: &str =
    "contracts/workflow-governance/runtime-core-assurance-v0.yaml";
pub const WORKFLOW_RELEASE_ADMISSION_AUDIENCE: &str =
    "forge-core:workflow-release-admission:embedded";

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

    /// Reporting-only policy count for the admitted immutable bundle.
    #[must_use]
    pub fn policy_count(&self) -> usize {
        self.bundle.workflow_governance_bundle.policies.len()
    }

    /// Reporting-only membership observation; it cannot mint release authority.
    #[must_use]
    pub fn contains_workflow_policy(&self, workflow_id: &str) -> bool {
        self.bundle
            .workflow_governance_bundle
            .policies
            .iter()
            .any(|policy| policy.compatibility_workflow_id.0 == workflow_id)
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

    #[must_use]
    pub fn release_count(&self) -> usize {
        self.releases.len()
    }

    /// Last release in the closed append-only registry, for audit and explicit
    /// adjacent-upgrade selection inside trusted kernel adapters.
    ///
    /// # Panics
    /// This cannot panic for a value minted by either closed loader because
    /// both reject empty registries before constructing the opaque value.
    #[must_use]
    pub fn latest_release(&self) -> &AdmittedWorkflowGovernanceRelease {
        self.releases
            .last()
            .expect("admitted registry always has a closed non-empty shape")
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
    ReviewArtifactMissing(String),
    ReviewArtifactParse { reference: String, source: String },
    ReviewEvaluationBlocked,
    ReviewEvaluationDigestMismatch,
    ReviewAuthorizationBindingMismatch,
    ReviewAuthorizationInvalid(String),
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

/// Admit the fixed three-release registry after independently recomputing its
/// review and consuming the repository-verified authorization capability.
///
/// The historical two-release loader remains a separate exact path. No caller
/// can supply a registry, bundle, reviewer registry, authorization, or project
/// path to this function.
///
/// # Errors
/// Fails closed on any missing byte, review mismatch, signature failure,
/// non-adjacent promotion, policy drift, receipt carryover, or shape change.
#[allow(clippy::too_many_lines)]
pub fn load_admitted_workflow_governance_reviewed_release_registry(
) -> Result<AdmittedWorkflowGovernanceReleaseRegistry, AdmittedWorkflowGovernanceReleaseError> {
    static ADMITTED: OnceLock<
        Result<AdmittedWorkflowGovernanceReleaseRegistry, AdmittedWorkflowGovernanceReleaseError>,
    > = OnceLock::new();
    match ADMITTED.get_or_init(load_admitted_workflow_governance_reviewed_release_registry_uncached)
    {
        Ok(registry) => Ok(duplicate_admitted_registry(registry)),
        Err(error) => Err(error.clone()),
    }
}

#[allow(clippy::too_many_lines)]
fn load_admitted_workflow_governance_reviewed_release_registry_uncached(
) -> Result<AdmittedWorkflowGovernanceReleaseRegistry, AdmittedWorkflowGovernanceReleaseError> {
    let input = load_review_candidate_input()?;
    let evaluation = evaluate_workflow_release_admission_candidate(&input);
    if evaluation.status
        != WorkflowReleaseAdmissionEvaluationStatus::ReadyForIndependentAuthorization
        || !evaluation.issues.is_empty()
        || evaluation.predecessor_policy_count != 15
        || evaluation.candidate_policy_count != 20
        || evaluation.reviewed_workflow_count != 5
        || evaluation.quarantine_count != 3
    {
        return Err(AdmittedWorkflowGovernanceReleaseError::ReviewEvaluationBlocked);
    }

    let reviewer_raw = required_embedded_text(WORKFLOW_RELEASE_REVIEWER_REGISTRY_REF)?;
    let reviewer_registry: WorkflowReleaseReviewerRegistryDocument =
        parse_review_artifact(WORKFLOW_RELEASE_REVIEWER_REGISTRY_REF, reviewer_raw)?;
    let authorization_raw = required_embedded_text(WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_REF)?;
    let authorization: WorkflowReleaseAdmissionAuthorizationDocument = parse_review_artifact(
        WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_REF,
        authorization_raw,
    )?;
    let payload = &authorization
        .workflow_release_admission_authorization
        .payload;
    let index = &input.review_index.workflow_release_review_index;
    if payload.evaluation_digest != evaluation.evaluation_digest
        || payload.review_index_id != index.id
        || payload.review_index_version != index.index_version
        || payload.review_index_raw_digest
            != sha256_content_hash(
                required_embedded_text(WORKFLOW_RELEASE_REVIEW_INDEX_REF)?.as_bytes(),
            )
        || payload.review_index_canonical_digest != evaluation.review_index_digest
    {
        return Err(AdmittedWorkflowGovernanceReleaseError::ReviewEvaluationDigestMismatch);
    }
    if payload.promotion != index.promotion
        || payload.workflow_decisions != index.workflow_decisions
        || payload.quarantine_decisions != index.quarantine_decisions
        || payload.dimension_decisions != index.dimension_decisions
        || !payload.invalidate_all_receipts
    {
        return Err(AdmittedWorkflowGovernanceReleaseError::ReviewAuthorizationBindingMismatch);
    }
    let capability = verify_workflow_release_admission_authorization(
        &reviewer_registry,
        reviewer_raw.as_bytes(),
        &authorization,
        WORKFLOW_RELEASE_ADMISSION_AUDIENCE,
    )
    .map_err(|error| {
        AdmittedWorkflowGovernanceReleaseError::ReviewAuthorizationInvalid(error.to_string())
    })?;
    admit_reviewed_registry(input, capability)
}

fn duplicate_admitted_registry(
    registry: &AdmittedWorkflowGovernanceReleaseRegistry,
) -> AdmittedWorkflowGovernanceReleaseRegistry {
    AdmittedWorkflowGovernanceReleaseRegistry {
        registry_id: registry.registry_id.clone(),
        registry_version: registry.registry_version.clone(),
        registry_digest: registry.registry_digest.clone(),
        releases: registry
            .releases
            .iter()
            .map(|release| AdmittedWorkflowGovernanceRelease {
                release: release.release.clone(),
                runtime_bundle: release.runtime_bundle.clone(),
                bundle: release.bundle.clone(),
                predecessor_release_id: release.predecessor_release_id.clone(),
                predecessor_release_digest: release.predecessor_release_digest.clone(),
                receipt_carryover: release.receipt_carryover,
            })
            .collect(),
    }
}

fn admit_reviewed_registry(
    input: WorkflowReleaseAdmissionCandidateInput,
    capability: VerifiedWorkflowReleaseAdmissionAuthorization,
) -> Result<AdmittedWorkflowGovernanceReleaseRegistry, AdmittedWorkflowGovernanceReleaseError> {
    let promotion = &input.review_index.workflow_release_review_index.promotion;
    if capability.candidate_release() != &promotion.candidate_release
        || capability.candidate_runtime_bundle() != &promotion.candidate_runtime_bundle
        || capability.promoted_runtime_bundle() != &promotion.promoted_runtime_bundle
    {
        return Err(AdmittedWorkflowGovernanceReleaseError::ReviewAuthorizationBindingMismatch);
    }
    let registry = input.proposed_registry.workflow_governance_release_registry;
    if registry.releases.len() != 3
        || registry.default_successor_release_id != registry.releases[2].release.release_id
        || registry.releases[2].receipt_carryover != WorkflowReceiptCarryover::InvalidateAll
        || registry.releases[2].authority != WorkflowReleaseRegistryAuthority::CandidateOnly
    {
        return Err(AdmittedWorkflowGovernanceReleaseError::RegistryShapeMismatch);
    }
    let registry_digest =
        forge_core_decisions::workflow_release_registry_digest(
            &WorkflowGovernanceReleaseRegistryDocument {
                schema_version:
                    forge_core_contracts::WORKFLOW_GOVERNANCE_RELEASE_REGISTRY_SCHEMA_VERSION
                        .to_owned(),
                workflow_governance_release_registry: registry.clone(),
            },
        )
        .map_err(AdmittedWorkflowGovernanceReleaseError::Canonicalize)?;
    let mut releases = Vec::with_capacity(3);
    for (entry, bundle) in registry
        .releases
        .clone()
        .into_iter()
        .zip(input.registry_bundles)
    {
        releases.push(admit_entry(entry, bundle)?);
    }
    if releases[2]
        .document()
        .workflow_governance_bundle
        .policies
        .len()
        != 20
        || !releases[2].is_adjacent_successor_of(&releases[1])
        || releases[0].is_adjacent_successor_of(&releases[2])
    {
        return Err(AdmittedWorkflowGovernanceReleaseError::RegistryShapeMismatch);
    }
    // Moving the non-cloneable capability into this function is the authority
    // consumption boundary. Only reporting-safe observations remain here.
    let _authorization_audit = capability.audit();
    drop(capability);
    Ok(AdmittedWorkflowGovernanceReleaseRegistry {
        registry_id: registry.registry_id,
        registry_version: registry.registry_version,
        registry_digest,
        releases,
    })
}

#[allow(clippy::too_many_lines)]
fn load_review_candidate_input(
) -> Result<WorkflowReleaseAdmissionCandidateInput, AdmittedWorkflowGovernanceReleaseError> {
    let review_index: WorkflowReleaseReviewIndexDocument =
        load_review_artifact(WORKFLOW_RELEASE_REVIEW_INDEX_REF)?;
    let index = &review_index.workflow_release_review_index;
    if index.predecessor_registry.embedded_ref.0 != ADMITTED_WORKFLOW_RELEASE_REGISTRY_REF
        || index.proposed_registry.embedded_ref.0 != REVIEWED_WORKFLOW_RELEASE_REGISTRY_REF
        || index.promoted_runtime_bundle.embedded_ref.0 != REVIEWED_WORKFLOW_RUNTIME_BUNDLE_REF
    {
        return Err(AdmittedWorkflowGovernanceReleaseError::RegistryShapeMismatch);
    }
    let coverage_policy: WorkflowBehavioralCoveragePolicyDocument =
        load_binding(&index.coverage_policy)?;
    let corpus_set: WorkflowBehavioralCorpusSetDocument = load_binding(&index.corpus_set)?;
    let representative_corpus: WorkflowBehavioralScenarioCorpusDocument =
        load_binding(&index.representative_corpus)?;
    let adversarial_corpus: WorkflowBehavioralScenarioCorpusDocument =
        load_binding(&index.adversarial_corpus)?;
    let review_subject: WorkflowBehavioralReviewSubjectDocument = load_binding(
        index
            .review_subjects
            .first()
            .ok_or(AdmittedWorkflowGovernanceReleaseError::ReviewEvaluationBlocked)?,
    )?;
    let authored_shadow_report: WorkflowBehavioralShadowReportDocument =
        load_binding(&index.shadow_report)?;
    let migration_batches = index
        .migration_batches
        .iter()
        .map(load_binding)
        .collect::<Result<Vec<WorkflowMigrationBatchDocument>, _>>()?;
    let candidate_manifest: WorkflowGovernanceReleaseManifestDocument =
        load_binding(&index.release_manifest)?;
    let candidate_runtime_bundle = load_binding(&index.candidate_runtime_bundle)?;
    let promoted_runtime_bundle = load_binding(&index.promoted_runtime_bundle)?;
    let predecessor_registry = load_binding(&index.predecessor_registry)?;
    let proposed_registry: WorkflowGovernanceReleaseRegistryDocument =
        load_binding(&index.proposed_registry)?;

    let mut source_bytes = HashMap::new();
    for path in embedded_yaml_paths() {
        if let Some(text) = embedded_text(&path) {
            source_bytes.insert(RepoPath(path), text.as_bytes().to_vec());
        }
    }
    source_bytes.insert(
        index.evaluator_source.embedded_ref.clone(),
        include_bytes!("../../../forge-core-decisions/src/workflow_behavior.rs").to_vec(),
    );
    source_bytes.insert(
        index.frozen_history.embedded_ref.clone(),
        include_bytes!("../../tests/fixtures/p5d2-foundation-history.ndjson").to_vec(),
    );

    let mut behavioral_bundles = BTreeMap::new();
    for path in collect_behavioral_bundle_refs(&representative_corpus)
        .into_iter()
        .chain(collect_behavioral_bundle_refs(&adversarial_corpus))
    {
        let document: WorkflowGovernanceBundleDocument = load_review_artifact(&path)?;
        let bytes = source_bytes.get(&RepoPath(path.clone())).ok_or_else(|| {
            AdmittedWorkflowGovernanceReleaseError::ReviewArtifactMissing(path.clone())
        })?;
        let artifact = WorkflowBehavioralArtifactReference {
            id: document.workflow_governance_bundle.id.clone(),
            embedded_ref: RepoPath(path),
            expected_digest: sha256_content_hash(bytes),
        };
        behavioral_bundles.insert(
            workflow_runtime_bundle_digest(&document)
                .map_err(AdmittedWorkflowGovernanceReleaseError::Canonicalize)?,
            WorkflowBehavioralBundleInput { artifact, document },
        );
    }
    let report = &authored_shadow_report.workflow_behavioral_shadow_report;
    let report_identity = WorkflowBehavioralReportIdentity {
        report_id: report.id.clone(),
        report_version: report.report_version.clone(),
        corpus_set: WorkflowBehavioralArtifactReference {
            id: corpus_set.workflow_behavioral_corpus_set.id.clone(),
            embedded_ref: index.corpus_set.embedded_ref.clone(),
            expected_digest: index.corpus_set.raw_digest.clone(),
        },
        coverage_policy: WorkflowBehavioralArtifactReference {
            id: coverage_policy
                .workflow_behavioral_coverage_policy
                .id
                .clone(),
            embedded_ref: index.coverage_policy.embedded_ref.clone(),
            expected_digest: index.coverage_policy.raw_digest.clone(),
        },
    };
    let registry_bundles = proposed_registry
        .workflow_governance_release_registry
        .releases
        .iter()
        .map(|entry| load_review_artifact(&entry.runtime_bundle.embedded_ref.0))
        .collect::<Result<Vec<WorkflowGovernanceBundleDocument>, _>>()?;
    Ok(WorkflowReleaseAdmissionCandidateInput {
        review_index,
        report_identity,
        coverage_policy,
        corpus_set,
        representative_corpus,
        adversarial_corpus,
        review_subject,
        behavioral_bundles,
        authored_shadow_report,
        migration_batches,
        candidate_manifest,
        candidate_runtime_bundle,
        promoted_runtime_bundle,
        predecessor_registry,
        proposed_registry,
        registry_bundles,
        source_bytes,
    })
}

fn collect_behavioral_bundle_refs(
    corpus: &WorkflowBehavioralScenarioCorpusDocument,
) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for workflow in &corpus.workflow_behavioral_scenario_corpus.workflow_evidence {
        for scenario in &workflow.scenarios {
            match &scenario.execution {
                WorkflowBehavioralScenarioExecution::Single { input, .. } => {
                    refs.insert(input.bundle.embedded_ref.0.clone());
                }
                WorkflowBehavioralScenarioExecution::Resume {
                    checkpoint_input,
                    resumed_input,
                    ..
                } => {
                    refs.insert(checkpoint_input.bundle.embedded_ref.0.clone());
                    refs.insert(resumed_input.bundle.embedded_ref.0.clone());
                }
                WorkflowBehavioralScenarioExecution::Ablation {
                    control_input,
                    ablated_input,
                    ..
                } => {
                    refs.insert(control_input.bundle.embedded_ref.0.clone());
                    refs.insert(ablated_input.bundle.embedded_ref.0.clone());
                }
            }
        }
    }
    refs
}

fn load_binding<T: serde::de::DeserializeOwned>(
    binding: &forge_core_contracts::WorkflowReleaseReviewArtifactBinding,
) -> Result<T, AdmittedWorkflowGovernanceReleaseError> {
    load_review_artifact(&binding.embedded_ref.0)
}

fn load_review_artifact<T: serde::de::DeserializeOwned>(
    reference: &str,
) -> Result<T, AdmittedWorkflowGovernanceReleaseError> {
    let raw = required_embedded_text(reference)?;
    parse_review_artifact(reference, raw)
}

fn required_embedded_text(
    reference: &str,
) -> Result<&'static str, AdmittedWorkflowGovernanceReleaseError> {
    embedded_text(reference).ok_or_else(|| {
        AdmittedWorkflowGovernanceReleaseError::ReviewArtifactMissing(reference.to_owned())
    })
}

fn parse_review_artifact<T: serde::de::DeserializeOwned>(
    reference: &str,
    raw: &str,
) -> Result<T, AdmittedWorkflowGovernanceReleaseError> {
    yaml_serde::from_str(raw).map_err(|error| {
        AdmittedWorkflowGovernanceReleaseError::ReviewArtifactParse {
            reference: reference.to_owned(),
            source: error.to_string(),
        }
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

    #[test]
    fn verified_capability_cannot_authorize_a_different_promotion() {
        let mut input = load_review_candidate_input().expect("review candidate");
        let reviewer_raw =
            required_embedded_text(WORKFLOW_RELEASE_REVIEWER_REGISTRY_REF).expect("reviewers");
        let reviewers: WorkflowReleaseReviewerRegistryDocument =
            parse_review_artifact(WORKFLOW_RELEASE_REVIEWER_REGISTRY_REF, reviewer_raw)
                .expect("reviewers");
        let authorization: WorkflowReleaseAdmissionAuthorizationDocument =
            load_review_artifact(WORKFLOW_RELEASE_ADMISSION_AUTHORIZATION_REF)
                .expect("authorization");
        let capability = verify_workflow_release_admission_authorization(
            &reviewers,
            reviewer_raw.as_bytes(),
            &authorization,
            WORKFLOW_RELEASE_ADMISSION_AUDIENCE,
        )
        .expect("verified capability");
        input
            .review_index
            .workflow_release_review_index
            .promotion
            .promoted_runtime_bundle
            .bundle_digest = format!("sha256:{}", "0".repeat(64));
        assert!(matches!(
            admit_reviewed_registry(input, capability),
            Err(AdmittedWorkflowGovernanceReleaseError::ReviewAuthorizationBindingMismatch)
        ));
    }
}
