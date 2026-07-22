//! Trusted persistence boundary for governed Domain Pack lifecycle state.
//!
//! One complete immutable generation is activated by flipping exactly one
//! crash-recoverable pointer under a retained OS lock. Serialized plans,
//! locks, reports, and receipts remain evidence. The commit token has no public
//! constructor; the policy/kernel integration must mint it only after fresh
//! supply-chain, resolution, composition, trust, capability, sandbox, and
//! compatibility checks.

use forge_core_authority::{
    domain_pack_registry_snapshot_digest, AnchoredDomainPackSupplyChainSnapshot,
    AnchoredReviewedDomainPackRegistrySnapshot,
};
use forge_core_contracts::domain_pack_learning::DomainPackSemanticAssurance;
use forge_core_contracts::{
    DomainPackAcquisitionCatalogDocument, DomainPackActivePointer, DomainPackActivePointerDocument,
    DomainPackArtifactBinding, DomainPackCandidateApprovalRequirement,
    DomainPackCandidateAuthority, DomainPackCapabilitySandboxPolicyDocument,
    DomainPackCompatibilityReport, DomainPackCompatibilityStatus, DomainPackComposedIdentity,
    DomainPackCompositionGap, DomainPackCompositionProjectionDocument,
    DomainPackCompositionRequestDocument, DomainPackCompositionStatus, DomainPackExactLockDocument,
    DomainPackExpectedLifecycleState, DomainPackInitializedProjectGenerationMaterial,
    DomainPackInitializedProjectIntentDocument, DomainPackInitializedProjectOperation,
    DomainPackInitializedProjectStateBinding, DomainPackLifecycleLedgerRecord,
    DomainPackLifecycleOperation, DomainPackLifecyclePreflightDocument,
    DomainPackLifecyclePreflightStatus, DomainPackLifecycleReceipt,
    DomainPackLifecycleReceiptDocument, DomainPackLockedPackage, DomainPackRecoveryReport,
    DomainPackRecoveryReportDocument, DomainPackRecoveryStatus, DomainPackRemoteArtifactMediaType,
    DomainPackResolutionRequestDocument, DomainPackResolutionStatus,
    DomainPackRuntimeCapabilityRegistryDocument, DomainPackSourceAssurance,
    DomainPackSupplyChainRegistryDocument, DomainPackTrustPolicyDocument, StableId,
    WorkflowGovernanceBundle, DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION,
    DOMAIN_PACK_INITIALIZED_PROJECT_SCHEMA_VERSION, DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
    DOMAIN_PACK_SCHEMA_VERSION,
};
use forge_core_decisions::{
    compose_domain_packs, domain_pack_resolution_projection_digest,
    evaluate_domain_pack_compatibility, evaluate_domain_pack_trust,
    join_reviewed_registry_to_resolution, resolve_domain_packs, DomainPackCandidateMaterial,
    DomainPackCapabilityDemand, DomainPackCompatibilityInput,
    DomainPackReviewedResolutionJoinStatus, DomainPackTrustEvaluationInput,
    DomainPackTrustEvaluationStatus,
};
use forge_core_store::crash_replace::{
    CrashReplaceError, CrashReplaceRecovery, CrashReplaceRecoveryAction,
};
use forge_core_store::retained_lifecycle::{
    DomainPackLifecycleCompletionInput, RetainedDomainPackActivePointerWitness,
    RetainedDomainPackExpectedActivePointer, RetainedDomainPackLifecycleCompletion,
    RetainedDomainPackLifecycleStore, RetainedLifecycleIoError,
};
use forge_core_store::retained_project_tree::{RetainedProjectTree, RetainedProjectTreeError};
use forge_core_store::{
    acquire_effect_store_lock, sha256_content_hash, EffectStoreLockError,
    RetainedCrashReplaceSession,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DOMAIN_PACK_STATE_RELATIVE_ROOT: &str = "domain-packs";
pub const DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH: &str = "domain-packs/active.lock.yaml";
pub const DOMAIN_PACK_LIFECYCLE_LOCK_RELATIVE_PATH: &str = "locks/domain-packs.lifecycle.lock";
pub const DOMAIN_PACK_MAX_LOCK_BYTES: u64 = 4 * 1024 * 1024;
pub const DOMAIN_PACK_MAX_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;
pub const DOMAIN_PACK_MAX_LEDGER_RECORDS: usize = 10_000;
pub const DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_FILES: usize = 100_000;
pub const DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;
pub const DOMAIN_PACK_MAX_SUPPLY_CHAIN_VERIFICATION_AGE_SECONDS: u64 = 300;
pub const DOMAIN_PACK_MAX_CLOCK_FUTURE_SKEW_SECONDS: u64 = 30;

/// Candidate-only remote artifact admission and its confined immutable cache.
pub mod acquisition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackLifecycleStateProjection {
    pub active_pointer: Option<DomainPackActivePointerDocument>,
    pub active_lock: Option<DomainPackExactLockDocument>,
    pub ledger_records: Vec<DomainPackLifecycleLedgerRecord>,
}

/// Exact immutable bytes in the complete reachable lifecycle closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRawLifecycleFile {
    relative_path: String,
    raw_bytes: Vec<u8>,
}

impl DomainPackRawLifecycleFile {
    #[must_use]
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    #[must_use]
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw_bytes
    }
}

/// Integrity-checked raw lifecycle inventory captured under one retained Store
/// root/lock capability. The sorted file list carries no independent authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackRawLifecycleInventory {
    files: Vec<DomainPackRawLifecycleFile>,
}

impl DomainPackRawLifecycleInventory {
    #[must_use]
    pub fn files(&self) -> &[DomainPackRawLifecycleFile] {
        &self.files
    }
}

/// Non-authoritative, integrity-checked source material for deriving a Core
/// rebase candidate while the lifecycle lock remains held by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackActiveRebaseSource {
    pub pointer: DomainPackActivePointerDocument,
    pub exact_lock: DomainPackExactLockDocument,
    pub resolution_request: DomainPackResolutionRequestDocument,
    pub composition_request: DomainPackCompositionRequestDocument,
    pub trust_input: DomainPackTrustEvaluationInput,
    pub lifecycle_operation: DomainPackLifecycleOperation,
}

/// Exact initialized-project state retained for deterministic high-level request
/// derivation. This is candidate source material only: it grants no package
/// trust, preflight, installation, activation, or mutation authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackInitializedProjectDerivationSource {
    pub expected_state: DomainPackInitializedProjectStateBinding,
    pub active_pointer: DomainPackActivePointerDocument,
    pub active_lock: DomainPackExactLockDocument,
    pub active_generation: DomainPackInitializedProjectGenerationMaterial,
    /// Exact historical rollback evidence, populated only when the supplied
    /// intent names a retained receipt and immutable target lock.
    pub rollback_target: Option<DomainPackInitializedProjectRollbackSource>,
    pub active_composition: DomainPackCompositionProjectionDocument,
    pub resolution_request: DomainPackResolutionRequestDocument,
    pub composition_request: DomainPackCompositionRequestDocument,
    pub trust_input: DomainPackTrustEvaluationInput,
    pub lifecycle_operation: DomainPackLifecycleOperation,
}

/// Historical immutable generation source selected by one exact rollback intent.
/// It remains candidate-only evidence and cannot apply or activate a generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackInitializedProjectRollbackSource {
    pub target_lock: DomainPackExactLockDocument,
    pub target_generation: DomainPackInitializedProjectGenerationMaterial,
}

/// Move-only token required by the mechanical commit path.
///
/// Its constructor is intentionally private. A later integration function in
/// this crate mints it only from opaque verified supply-chain admission plus a
/// freshly recomputed ready preflight.
#[derive(Debug)]
pub struct DomainPackLifecycleCommitAuthority {
    preflight_digest: String,
    project_snapshot: Arc<RetainedProjectTree>,
    supply_chain_verified_at_unix: u64,
    supply_chain_expires_at_unix: u64,
    verified_artifacts: Vec<OwnedDomainPackImmutableArtifact>,
    acquisition_catalog: DomainPackAcquisitionCatalogDocument,
    resolution_request: DomainPackResolutionRequestDocument,
    composition_request: DomainPackCompositionRequestDocument,
    trust_input: DomainPackTrustEvaluationInput,
}

/// Exact immutable bytes admitted by the TCB for the lifecycle object store.
/// A binding without its bytes can never be activated.
pub struct DomainPackImmutableArtifact<'a> {
    pub binding: &'a DomainPackArtifactBinding,
    pub raw_bytes: &'a [u8],
}

#[derive(Debug)]
struct OwnedDomainPackImmutableArtifact {
    binding: DomainPackArtifactBinding,
    raw_bytes: Vec<u8>,
}

/// Opaque proof that one bounded project-tree snapshot matched the lifecycle
/// request. The Store-owned capability retains every accepted root, directory,
/// file identity, namespace binding, and exact byte sequence through commit.
pub struct VerifiedDomainPackProjectSnapshot {
    project_tree: Arc<RetainedProjectTree>,
}

impl fmt::Debug for VerifiedDomainPackProjectSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedDomainPackProjectSnapshot")
            .field("snapshot_digest", &self.project_tree.snapshot_digest())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct PreparedDomainPackLifecycleTransaction {
    preflight: DomainPackLifecyclePreflightDocument,
    previous_pointer: Option<DomainPackActivePointerDocument>,
    previous_lock: Option<DomainPackExactLockDocument>,
    record: DomainPackLifecycleLedgerRecord,
    next_pointer: DomainPackActivePointerDocument,
    receipt: DomainPackLifecycleReceiptDocument,
    rollback_target: Option<DomainPackLifecycleReceiptDocument>,
}

/// Fresh non-authoritative inputs that the TCB recomputes before minting commit
/// authority. `anchored_snapshot` and `anchored_reviewed_snapshot` are opaque
/// monotonic capabilities; all other fields are untrusted material whose exact
/// output must match the prepared preflight.
///
/// Serialized or cloned audit evidence cannot substitute the reviewed-registry
/// capability:
///
/// ```compile_fail
/// use forge_core_authority::{
///     AnchoredReviewedDomainPackRegistrySnapshot,
///     VerifiedDomainPackPromotionAuthorizationAudit,
/// };
/// fn requires_opaque(_: &AnchoredReviewedDomainPackRegistrySnapshot) {}
/// fn audit_is_not_authority(audit: &VerifiedDomainPackPromotionAuthorizationAudit) {
///     requires_opaque(audit);
/// }
/// ```
pub struct DomainPackLifecycleAuthorizationContext<'a> {
    pub anchored_snapshot: &'a AnchoredDomainPackSupplyChainSnapshot,
    pub anchored_reviewed_snapshot: &'a AnchoredReviewedDomainPackRegistrySnapshot,
    pub project_snapshot: &'a VerifiedDomainPackProjectSnapshot,
    pub trust_policy_document: &'a DomainPackTrustPolicyDocument,
    pub registry_document: &'a DomainPackSupplyChainRegistryDocument,
    pub resolution_request: &'a DomainPackResolutionRequestDocument,
    pub composition_request: &'a DomainPackCompositionRequestDocument,
    pub materials: &'a [DomainPackCandidateMaterial<'a>],
    pub artifacts: &'a [DomainPackImmutableArtifact<'a>],
    pub trust_input: &'a DomainPackTrustEvaluationInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackGenerationManifest {
    schema_version: String,
    generation: u64,
    record_digest: String,
    lock_digest: String,
    preflight_digest: String,
    compatibility_report_digest: String,
    receipt_digest: String,
    object_raw_digests: Vec<String>,
}

/// Hash one bounded project tree through a sealed Store-owned retained witness.
/// Forge state, VCS internals, build output, and dependency caches are excluded
/// consistently with the lifecycle Project Snapshot Adapter.
///
/// # Errors
///
/// Fails closed when the root or any accepted component is invalid, the tree
/// exceeds bounds, an identity or byte sequence changes, or traversal cannot be
/// completed descriptor-relatively.
pub fn domain_pack_project_snapshot_digest(
    project_root: impl AsRef<Path>,
) -> Result<String, DomainPackLifecycleStoreError> {
    let project_tree = RetainedProjectTree::capture_allowing_store_owned_file_anchors(
        project_root,
        DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_FILES,
        DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_BYTES,
    )?;
    Ok(project_tree.snapshot_digest().to_owned())
}

/// Capture and retain an opaque proof of one exact project-tree snapshot.
///
/// # Errors
///
/// Fails closed when the project root is invalid, unbounded, unstable, or the
/// retained tree digest differs from `expected_digest`.
pub fn verify_domain_pack_project_snapshot(
    project_root: impl AsRef<Path>,
    expected_digest: &str,
) -> Result<VerifiedDomainPackProjectSnapshot, DomainPackLifecycleStoreError> {
    let project_tree = Arc::new(
        RetainedProjectTree::capture_allowing_store_owned_file_anchors(
            project_root,
            DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_FILES,
            DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_BYTES,
        )?,
    );
    if project_tree.snapshot_digest() != expected_digest {
        return Err(DomainPackLifecycleStoreError::StaleExpectedState {
            expected: expected_digest.to_owned(),
            actual: project_tree.snapshot_digest().to_owned(),
        });
    }
    Ok(VerifiedDomainPackProjectSnapshot { project_tree })
}

struct LoadedDomainPackLifecycleState {
    projection: DomainPackLifecycleStateProjection,
    completion: Option<RetainedDomainPackLifecycleCompletion>,
}

#[derive(Debug)]
pub struct LockedDomainPackLifecycle {
    store: RetainedDomainPackLifecycleStore,
    project_snapshot: Arc<RetainedProjectTree>,
    state: DomainPackLifecycleStateProjection,
    active_pointer_authority: RetainedDomainPackExpectedActivePointer,
    completion_authority: Option<RetainedDomainPackLifecycleCompletion>,
    recovery: CrashReplaceRecovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ActiveDomainPackGenerationMaterial {
    pointer: DomainPackActivePointerDocument,
    lock: DomainPackExactLockDocument,
    composition: DomainPackCompositionProjectionDocument,
    lifecycle_operation: DomainPackLifecycleOperation,
    effective_bundle: WorkflowGovernanceBundle,
    pointer_raw_digest: String,
    generation_manifest_raw_digest: String,
    lock_raw_digest: String,
    preflight_raw_digest: String,
    initialized_generation: Option<DomainPackInitializedProjectGenerationMaterial>,
    rebase_inputs: Option<ActiveDomainPackRebaseInputs>,
    admission_kind: ActiveDomainPackGenerationAdmissionKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ActiveDomainPackRebaseInputs {
    resolution_request: DomainPackResolutionRequestDocument,
    composition_request: DomainPackCompositionRequestDocument,
    trust_input: DomainPackTrustEvaluationInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ActiveDomainPackGenerationAdmissionKind {
    Ready,
    DegradedEmpty,
}

/// Move-only admission of the exact durable Domain Pack generation currently
/// active under the retained lifecycle OS lock.
///
/// There is deliberately no public constructor, `Clone`, or serde surface.
/// Callers cannot turn a serialized pointer, lock, preflight, composition, or
/// audit into execution authority. A consumer must keep this value alive for
/// its transaction and obtain a freshly revalidated borrowed view.
pub struct AdmittedActiveDomainPackGeneration {
    lifecycle_store: RetainedDomainPackLifecycleStore,
    project_snapshot: Arc<RetainedProjectTree>,
    active_pointer_authority: RetainedDomainPackExpectedActivePointer,
    completion_authority: RetainedDomainPackLifecycleCompletion,
    material: ActiveDomainPackGenerationMaterial,
}

impl fmt::Debug for AdmittedActiveDomainPackGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedActiveDomainPackGeneration")
            .finish_non_exhaustive()
    }
}

/// Borrowed execution inputs from a freshly revalidated admitted generation.
///
/// This view cannot outlive the admitted value that retains the lifecycle OS
/// lock. It exposes only the effective bundle and exact identity bindings a
/// later kernel join needs; it is not independently constructible authority.
pub struct AdmittedReadyDomainPackGenerationView<'a> {
    material: &'a ActiveDomainPackGenerationMaterial,
}

impl fmt::Debug for AdmittedReadyDomainPackGenerationView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedReadyDomainPackGenerationView")
            .finish_non_exhaustive()
    }
}

/// Typed admission of a legitimate empty-package generation produced by a
/// governed remove or rollback-to-empty operation. Its composition gaps remain
/// blocking data and must never be silently treated as normal core-only
/// readiness.
pub struct AdmittedDegradedEmptyDomainPackGenerationView<'a> {
    material: &'a ActiveDomainPackGenerationMaterial,
}

impl fmt::Debug for AdmittedDegradedEmptyDomainPackGenerationView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedDegradedEmptyDomainPackGenerationView")
            .field(
                "gaps",
                &self
                    .material
                    .composition
                    .domain_pack_composition_projection
                    .gaps,
            )
            .finish_non_exhaustive()
    }
}

/// Freshly revalidated active generation. The variant is authoritative: only
/// an exactly composable generation is `Ready`; a clean, empty-package
/// remove/rollback generation is explicitly `DegradedEmpty` with typed gaps.
pub enum AdmittedActiveDomainPackGenerationView<'a> {
    Ready(AdmittedReadyDomainPackGenerationView<'a>),
    DegradedEmpty(AdmittedDegradedEmptyDomainPackGenerationView<'a>),
}

impl fmt::Debug for AdmittedActiveDomainPackGenerationView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready(_) => formatter.write_str("AdmittedActiveDomainPackGenerationView::Ready"),
            Self::DegradedEmpty(view) => formatter
                .debug_tuple("AdmittedActiveDomainPackGenerationView::DegradedEmpty")
                .field(view)
                .finish(),
        }
    }
}

/// Borrowed proof that no Domain Pack generation is active while the
/// lifecycle OS lock remains retained by [`LockedDomainPackLifecycle`].
pub struct AdmittedCoreOnlyDomainPackLifecycleView<'a> {
    _lifecycle: &'a LockedDomainPackLifecycle,
}

impl fmt::Debug for AdmittedCoreOnlyDomainPackLifecycleView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedCoreOnlyDomainPackLifecycleView")
            .finish_non_exhaustive()
    }
}

impl AdmittedActiveDomainPackGeneration {
    /// Re-read and cross-link the complete active generation before exposing a
    /// borrowed consumer view. The lifecycle OS lock remains retained by
    /// `self` for the lifetime of the returned view and the consumer's join.
    ///
    /// # Errors
    ///
    /// Fails closed if any pointer, ledger, generation, lock, preflight,
    /// composition, receipt, or immutable object changed since admission.
    pub fn verified_view(
        &self,
    ) -> Result<AdmittedActiveDomainPackGenerationView<'_>, DomainPackLifecycleStoreError> {
        self.lifecycle_store
            .revalidate_lifecycle_completion(&self.completion_authority)?;
        self.lifecycle_store
            .revalidate_expected_active_pointer(&self.active_pointer_authority)?;
        let current =
            load_active_generation_material(&self.lifecycle_store, &self.project_snapshot)?;
        if current != self.material {
            return Err(DomainPackLifecycleStoreError::StaleExpectedState {
                expected: active_generation_material_digest(&self.material)?,
                actual: active_generation_material_digest(&current)?,
            });
        }
        Ok(match self.material.admission_kind {
            ActiveDomainPackGenerationAdmissionKind::Ready => {
                AdmittedActiveDomainPackGenerationView::Ready(
                    AdmittedReadyDomainPackGenerationView {
                        material: &self.material,
                    },
                )
            }
            ActiveDomainPackGenerationAdmissionKind::DegradedEmpty => {
                AdmittedActiveDomainPackGenerationView::DegradedEmpty(
                    AdmittedDegradedEmptyDomainPackGenerationView {
                        material: &self.material,
                    },
                )
            }
        })
    }
}

impl AdmittedActiveDomainPackGenerationView<'_> {
    fn material(&self) -> &ActiveDomainPackGenerationMaterial {
        match self {
            Self::Ready(view) => view.material,
            Self::DegradedEmpty(view) => view.material,
        }
    }

    /// Exact effective core-plus-packs bundle from the committed composition.
    #[must_use]
    pub fn effective_bundle(&self) -> &WorkflowGovernanceBundle {
        &self.material().effective_bundle
    }

    /// Blocking composition gaps for a governed empty-package generation.
    /// Ready generations always return an empty slice.
    #[must_use]
    pub fn degraded_gaps(&self) -> &[DomainPackCompositionGap] {
        match self {
            Self::Ready(_) => &[],
            Self::DegradedEmpty(view) => {
                &view
                    .material
                    .composition
                    .domain_pack_composition_projection
                    .gaps
            }
        }
    }

    #[must_use]
    pub const fn is_degraded_empty(&self) -> bool {
        matches!(self, Self::DegradedEmpty(_))
    }

    #[must_use]
    pub fn base_core_bundle_digest(&self) -> &str {
        &self
            .material()
            .composition
            .domain_pack_composition_projection
            .core_bundle_digest
    }

    #[must_use]
    pub fn generation_id(&self) -> u64 {
        self.material()
            .pointer
            .domain_pack_active_pointer
            .generation
    }

    #[must_use]
    pub fn lock_digest(&self) -> &str {
        &self.material().lock.domain_pack_exact_lock.lock_digest
    }

    /// Digest of the crash-recoverable active pointer captured by this generation.
    #[must_use]
    pub fn lifecycle_pointer_digest(&self) -> &str {
        &self
            .material()
            .pointer
            .domain_pack_active_pointer
            .pointer_digest
    }

    /// Immutable lifecycle-ledger head selecting this generation.
    #[must_use]
    pub fn lifecycle_head_digest(&self) -> &str {
        &self
            .material()
            .pointer
            .domain_pack_active_pointer
            .lifecycle_head_digest
    }

    /// Lifecycle operation that produced this immutable generation.
    #[must_use]
    pub fn lifecycle_operation(&self) -> &DomainPackLifecycleOperation {
        &self.material().lifecycle_operation
    }

    /// Exact sealed Core binding used to compose the active generation.
    #[must_use]
    pub fn core_binding(&self) -> &forge_core_contracts::DomainPackCoreBinding {
        &self.material().lock.domain_pack_exact_lock.payload.core
    }

    /// Exact lock retained by the active lifecycle generation.
    #[must_use]
    pub fn exact_lock(&self) -> &DomainPackExactLockDocument {
        &self.material().lock
    }

    /// Candidate-only resolver input persisted after full lifecycle admission.
    #[must_use]
    pub fn rebase_resolution_request(&self) -> Option<&DomainPackResolutionRequestDocument> {
        self.material()
            .rebase_inputs
            .as_ref()
            .map(|inputs| &inputs.resolution_request)
    }

    /// Candidate-only composition input persisted after full lifecycle admission.
    #[must_use]
    pub fn rebase_composition_request(&self) -> Option<&DomainPackCompositionRequestDocument> {
        self.material()
            .rebase_inputs
            .as_ref()
            .map(|inputs| &inputs.composition_request)
    }

    /// Last admitted trust/capability input. Fresh operator roots must still be
    /// reverified before any rebase authority can be minted.
    #[must_use]
    pub fn rebase_trust_input(&self) -> Option<&DomainPackTrustEvaluationInput> {
        self.material()
            .rebase_inputs
            .as_ref()
            .map(|inputs| &inputs.trust_input)
    }

    #[must_use]
    pub fn composition_digest(&self) -> &str {
        &self
            .material()
            .composition
            .domain_pack_composition_projection
            .composition_digest
    }

    #[must_use]
    pub fn supply_chain_registry_digest(&self) -> &str {
        &self
            .material()
            .lock
            .domain_pack_exact_lock
            .payload
            .registry_snapshot_digest
    }

    #[must_use]
    pub fn reviewer_registry_digest(&self) -> &str {
        &self
            .material()
            .lock
            .domain_pack_exact_lock
            .payload
            .reviewer_registry_digest
    }

    #[must_use]
    pub fn reviewed_registry_digest(&self) -> &str {
        &self
            .material()
            .lock
            .domain_pack_exact_lock
            .payload
            .reviewed_registry_digest
    }

    #[must_use]
    pub fn active_package_identities(&self) -> &[DomainPackComposedIdentity] {
        &self
            .material()
            .composition
            .domain_pack_composition_projection
            .ordered_packs
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DomainPackLifecycleStoreError {
    InvalidArgument {
        field: &'static str,
        reason: String,
    },
    InvalidDigest {
        field: &'static str,
        value: String,
    },
    InvalidDocument {
        path: PathBuf,
        reason: String,
    },
    StaleExpectedState {
        expected: String,
        actual: String,
    },
    PreflightBlocked {
        reason: String,
    },
    ResourceLimit {
        resource: &'static str,
        maximum: u64,
    },
    Lock {
        reason: String,
    },
    CrashReplace {
        reason: String,
    },
    Io {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for DomainPackLifecycleStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument { field, reason } => {
                write!(formatter, "invalid domain-pack lifecycle {field}: {reason}")
            }
            Self::InvalidDigest { field, value } => {
                write!(
                    formatter,
                    "invalid domain-pack lifecycle digest {field}: {value}"
                )
            }
            Self::InvalidDocument { path, reason } => write!(
                formatter,
                "invalid domain-pack lifecycle document {}: {reason}",
                path.display()
            ),
            Self::StaleExpectedState { expected, actual } => write!(
                formatter,
                "stale domain-pack lifecycle state: expected {expected}; actual {actual}"
            ),
            Self::PreflightBlocked { reason } => {
                write!(
                    formatter,
                    "domain-pack lifecycle preflight blocked: {reason}"
                )
            }
            Self::ResourceLimit { resource, maximum } => write!(
                formatter,
                "domain-pack lifecycle {resource} exceeds limit {maximum}"
            ),
            Self::Lock { reason } => {
                write!(formatter, "domain-pack lifecycle lock failed: {reason}")
            }
            Self::CrashReplace { reason } => {
                write!(
                    formatter,
                    "domain-pack lifecycle pointer replacement failed: {reason}"
                )
            }
            Self::Io { path, reason } => write!(
                formatter,
                "domain-pack lifecycle I/O {} failed: {reason}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for DomainPackLifecycleStoreError {}

impl From<EffectStoreLockError> for DomainPackLifecycleStoreError {
    fn from(value: EffectStoreLockError) -> Self {
        Self::Lock {
            reason: value.to_string(),
        }
    }
}

impl From<RetainedLifecycleIoError> for DomainPackLifecycleStoreError {
    fn from(value: RetainedLifecycleIoError) -> Self {
        match value {
            RetainedLifecycleIoError::InvalidRelativePath { path } => Self::InvalidArgument {
                field: "state_path",
                reason: path,
            },
            RetainedLifecycleIoError::SizeLimit { maximum, .. } => Self::ResourceLimit {
                resource: "document bytes",
                maximum,
            },
            RetainedLifecycleIoError::Identity { path, reason }
            | RetainedLifecycleIoError::Io { path, reason } => Self::Io { path, reason },
            other => Self::Io {
                path: PathBuf::from(DOMAIN_PACK_STATE_RELATIVE_ROOT),
                reason: other.to_string(),
            },
        }
    }
}

impl From<RetainedProjectTreeError> for DomainPackLifecycleStoreError {
    fn from(value: RetainedProjectTreeError) -> Self {
        match value {
            RetainedProjectTreeError::InvalidRoot { path, reason }
            | RetainedProjectTreeError::Identity { path, reason }
            | RetainedProjectTreeError::Io { path, reason } => Self::Io { path, reason },
            RetainedProjectTreeError::ResourceLimit { resource, maximum } => {
                Self::ResourceLimit { resource, maximum }
            }
            other => Self::Io {
                path: PathBuf::from("project_snapshot"),
                reason: other.to_string(),
            },
        }
    }
}

impl From<CrashReplaceError> for DomainPackLifecycleStoreError {
    fn from(value: CrashReplaceError) -> Self {
        Self::CrashReplace {
            reason: value.to_string(),
        }
    }
}

/// Acquire the fixed lifecycle lock for an embedded `.forge-method` directory,
/// reconcile interrupted pointer replacement, and verify the complete active
/// generation and immutable ledger chain.
///
/// Call [`lock_domain_pack_lifecycle_for_project`] when the Forge state is a
/// detached sidecar rather than a direct child of the governed project root.
///
/// # Errors
///
/// Returns a typed lock, recovery, confinement, integrity, or I/O error when
/// the retained state cannot be proven complete.
pub fn lock_domain_pack_lifecycle(
    state_root: impl AsRef<Path>,
) -> Result<LockedDomainPackLifecycle, DomainPackLifecycleStoreError> {
    let state_root = canonical_state_root(state_root.as_ref())?;
    let project_root = state_root
        .parent()
        .ok_or_else(|| invalid("state_root", "canonical state root has no project parent"))?;
    lock_domain_pack_lifecycle_for_canonical_project(project_root, &state_root)
}

/// Acquire the fixed lifecycle lock while binding project evidence to an
/// explicit governed project root.
///
/// This is the authoritative entry point for detached sidecar layouts. Store,
/// operator, and recovery files beside the `.forge-method` directory are not
/// project evidence and therefore cannot contaminate the retained project tree.
///
/// # Errors
///
/// Returns a typed lock, recovery, confinement, integrity, or I/O error when
/// either root or the retained state cannot be proven complete.
pub fn lock_domain_pack_lifecycle_for_project(
    project_root: impl AsRef<Path>,
    state_root: impl AsRef<Path>,
) -> Result<LockedDomainPackLifecycle, DomainPackLifecycleStoreError> {
    let state_root = canonical_state_root(state_root.as_ref())?;
    let project_root = fs::canonicalize(project_root.as_ref())
        .map_err(|error| io_error(project_root.as_ref(), error))?;
    if !project_root.is_dir() {
        return Err(invalid(
            "project_root",
            "must be an existing canonical project directory",
        ));
    }
    lock_domain_pack_lifecycle_for_canonical_project(&project_root, &state_root)
}

fn lock_domain_pack_lifecycle_for_canonical_project(
    project_root: &Path,
    state_root: &Path,
) -> Result<LockedDomainPackLifecycle, DomainPackLifecycleStoreError> {
    let project_snapshot = Arc::new(
        RetainedProjectTree::capture_allowing_store_owned_file_anchors(
            project_root,
            DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_FILES,
            DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_BYTES,
        )?,
    );
    let lock = acquire_effect_store_lock(state_root, DOMAIN_PACK_LIFECYCLE_LOCK_RELATIVE_PATH)?;
    let store = lock.into_domain_pack_lifecycle_store_for_project(&project_snapshot)?;
    let (loaded, recovery, active_pointer_authority) =
        load_current_state_under_lock(&store, &project_snapshot)?;
    Ok(LockedDomainPackLifecycle {
        store,
        project_snapshot,
        state: loaded.projection,
        active_pointer_authority,
        completion_authority: loaded.completion,
        recovery,
    })
}

impl LockedDomainPackLifecycle {
    fn revalidate_retained_completion(&self) -> Result<(), DomainPackLifecycleStoreError> {
        match (
            self.state.active_pointer.is_some(),
            self.completion_authority.as_ref(),
        ) {
            (true, Some(completion)) => {
                self.store.revalidate_lifecycle_completion(completion)?;
                Ok(())
            }
            (false, None) => Ok(()),
            _ => Err(stale(
                "retained completion authority differs from lifecycle projection",
                &self.state,
            )),
        }
    }

    #[must_use]
    pub fn projection(&self) -> &DomainPackLifecycleStateProjection {
        &self.state
    }

    /// Capture the complete active and historical lifecycle closure as exact
    /// bytes without releasing or reacquiring the retained Store authority.
    ///
    /// # Errors
    ///
    /// Fails closed if any generation, ledger record, receipt, replay input, or
    /// immutable object is absent, substituted, malformed, or cross-linked to a
    /// different lifecycle history.
    pub fn raw_inventory(
        &self,
    ) -> Result<DomainPackRawLifecycleInventory, DomainPackLifecycleStoreError> {
        self.revalidate_retained_completion()?;
        self.store
            .revalidate_expected_active_pointer(&self.active_pointer_authority)?;
        let current = load_current_state_under_lock(&self.store, &self.project_snapshot)?
            .0
            .projection;
        if current != self.state {
            return Err(stale(
                "Domain Pack lifecycle changed before raw inventory capture",
                &current,
            ));
        }
        load_raw_lifecycle_inventory(&self.store, &self.project_snapshot, &current)
    }

    /// Revalidate that the lifecycle remains uninitialized while retaining
    /// this handle's OS lock. The returned proof borrows the lock-owning
    /// handle, so a core-only effective workflow admission cannot outlive it.
    ///
    /// # Errors
    ///
    /// Fails closed if a generation is active or durable lifecycle state
    /// differs from the state recovered when this lock was acquired.
    pub fn verified_core_only_view(
        &self,
    ) -> Result<AdmittedCoreOnlyDomainPackLifecycleView<'_>, DomainPackLifecycleStoreError> {
        self.revalidate_retained_completion()?;
        self.store
            .revalidate_expected_active_pointer(&self.active_pointer_authority)?;
        let current = load_current_state_under_lock(&self.store, &self.project_snapshot)?
            .0
            .projection;
        if current != self.state
            || current.active_pointer.is_some()
            || current.active_lock.is_some()
            || !current.ledger_records.is_empty()
        {
            return Err(stale(
                "Domain Pack lifecycle is not a stable core-only state",
                &current,
            ));
        }
        Ok(AdmittedCoreOnlyDomainPackLifecycleView { _lifecycle: self })
    }

    /// Consume the lifecycle handle and admit the exact durable active
    /// generation while transferring its retained OS lock to the opaque
    /// execution seam.
    ///
    /// # Errors
    ///
    /// Fails closed when no generation is active, the state changed after lock
    /// acquisition, or any durable generation cross-link is invalid.
    pub fn admit_active_generation(
        self,
    ) -> Result<AdmittedActiveDomainPackGeneration, DomainPackLifecycleStoreError> {
        self.revalidate_retained_completion()?;
        self.store
            .revalidate_expected_active_pointer(&self.active_pointer_authority)?;
        if self.state.active_pointer.is_none() && self.state.active_lock.is_none() {
            return Err(blocked("no active Domain Pack generation"));
        }
        let Self {
            store,
            project_snapshot,
            state,
            active_pointer_authority,
            completion_authority,
            recovery: _,
        } = self;
        let completion_authority = completion_authority.ok_or_else(|| {
            stale(
                "active generation has no retained completion authority",
                &state,
            )
        })?;
        let material = load_active_generation_material(&store, &project_snapshot)?;
        if state.active_pointer.as_ref() != Some(&material.pointer)
            || state.active_lock.as_ref() != Some(&material.lock)
        {
            return Err(stale(
                "active generation changed after lifecycle lock acquisition",
                &state,
            ));
        }
        Ok(AdmittedActiveDomainPackGeneration {
            lifecycle_store: store,
            project_snapshot,
            active_pointer_authority,
            completion_authority,
            material,
        })
    }

    /// Typed non-authoritative account of the pointer recovery performed while
    /// acquiring this retained lifecycle lock.
    #[must_use]
    pub fn recovery_report(&self) -> DomainPackRecoveryReportDocument {
        let status = match self.recovery.action {
            CrashReplaceRecoveryAction::Noop => DomainPackRecoveryStatus::Clean,
            CrashReplaceRecoveryAction::RemovedUncommittedNext
            | CrashReplaceRecoveryAction::AbortedToPrevious
            | CrashReplaceRecoveryAction::RestoredPrevious => {
                DomainPackRecoveryStatus::RecoveredPrior
            }
            CrashReplaceRecoveryAction::CommittedInitial
            | CrashReplaceRecoveryAction::CleanedCommitted => {
                DomainPackRecoveryStatus::RecoveredTarget
            }
            _ => DomainPackRecoveryStatus::BlockedAmbiguous,
        };
        let repaired_artifact_refs = if status == DomainPackRecoveryStatus::Clean {
            Vec::new()
        } else {
            vec![DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH.to_owned()]
        };
        DomainPackRecoveryReportDocument {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            domain_pack_recovery_report: DomainPackRecoveryReport {
                authority: DomainPackCandidateAuthority::CandidateOnly,
                status,
                active_state: self
                    .state
                    .active_pointer
                    .as_ref()
                    .map(|pointer| pointer.domain_pack_active_pointer.clone()),
                lifecycle_head_digest: self.state.active_pointer.as_ref().map(|pointer| {
                    pointer
                        .domain_pack_active_pointer
                        .lifecycle_head_digest
                        .clone()
                }),
                repaired_artifact_refs,
                issues: Vec::new(),
            },
        }
    }

    /// Bind one high-level initialized-project intent to the exact active
    /// lifecycle generation while retaining the lifecycle OS lock.
    ///
    /// The returned material remains candidate-only input to the existing
    /// resolver, composer, trust ceremony, preflight, CAS, apply, receipt, and
    /// recovery path. In particular, install and upgrade selection evidence
    /// still requires the explicit operator-candidate approval ceremony before
    /// trust evaluation or activation.
    ///
    /// # Errors
    ///
    /// Fails closed when the intent is not candidate-only, the project or any
    /// generation/lock/head/snapshot binding is stale or substituted, the
    /// lifecycle is uninitialized, or the active generation predates retained
    /// deterministic derivation inputs.
    #[allow(clippy::too_many_lines)]
    pub fn initialized_project_source(
        &self,
        intent: &DomainPackInitializedProjectIntentDocument,
    ) -> Result<DomainPackInitializedProjectDerivationSource, DomainPackLifecycleStoreError> {
        if intent.schema_version != DOMAIN_PACK_INITIALIZED_PROJECT_SCHEMA_VERSION {
            return Err(invalid(
                "initialized_intent.schema_version",
                "unsupported initialized-project schema version",
            ));
        }
        let intent = &intent.domain_pack_initialized_project_intent;
        if intent.authority != DomainPackCandidateAuthority::CandidateOnly {
            return Err(blocked(
                "initialized-project intent must remain candidate-only",
            ));
        }
        match &intent.operation {
            DomainPackInitializedProjectOperation::Install { selection }
            | DomainPackInitializedProjectOperation::Upgrade { selection, .. } => {
                if selection.approval
                    != DomainPackCandidateApprovalRequirement::ExplicitOperatorApprovalRequired
                {
                    return Err(blocked(
                        "candidate selection must require explicit operator approval",
                    ));
                }
            }
            DomainPackInitializedProjectOperation::Rollback { .. }
            | DomainPackInitializedProjectOperation::Remove { .. }
            | DomainPackInitializedProjectOperation::RebaseCore { .. } => {}
        }

        self.store
            .validate_project_tree(&self.project_snapshot)
            .map_err(|error| {
                stale_project_snapshot(
                    &intent.expected_state.project_snapshot_digest,
                    &format!("project changed before initialized request derivation: {error}"),
                )
            })?;
        self.store
            .revalidate_expected_active_pointer(&self.active_pointer_authority)?;
        self.revalidate_retained_completion().map_err(|error| {
            blocked(&format!(
                "retained lifecycle completion changed before initialized request derivation: {error}"
            ))
        })?;
        let current = load_current_state_under_lock(&self.store, &self.project_snapshot)?
            .0
            .projection;
        if current != self.state {
            return Err(stale(
                "Domain Pack lifecycle changed before initialized request derivation",
                &current,
            ));
        }
        let material = load_active_generation_material(&self.store, &self.project_snapshot)?;
        if current.active_pointer.as_ref() != Some(&material.pointer)
            || current.active_lock.as_ref() != Some(&material.lock)
        {
            return Err(stale(
                "initialized derivation material differs from active lifecycle state",
                &current,
            ));
        }
        let pointer = &material.pointer.domain_pack_active_pointer;
        let expected_state = DomainPackInitializedProjectStateBinding {
            generation: pointer.generation,
            active_lock_digest: pointer.active_lock_digest.clone(),
            lifecycle_head_digest: pointer.lifecycle_head_digest.clone(),
            project_snapshot_digest: self.project_snapshot.snapshot_digest().to_owned(),
        };
        if intent.project_id != pointer.project_id
            || intent.project_id != material.lock.domain_pack_exact_lock.payload.project_id
            || intent.expected_state != expected_state
        {
            return Err(DomainPackLifecycleStoreError::StaleExpectedState {
                expected: format!("{:?}", intent.expected_state),
                actual: format!("{expected_state:?}"),
            });
        }
        let active_generation = material.initialized_generation.clone().ok_or_else(|| {
            blocked("active generation predates persisted initialized-project derivation material")
        })?;
        let inputs = material.rebase_inputs.as_ref().ok_or_else(|| {
            blocked("active generation predates persisted initialized-project derivation inputs")
        })?;
        if inputs
            .resolution_request
            .domain_pack_resolution_request
            .project_id
            != intent.project_id
            || inputs
                .composition_request
                .domain_pack_composition_request
                .requirements
                .project_id
                != intent.project_id
            || inputs.trust_input.project_id != intent.project_id
        {
            return Err(blocked(
                "persisted derivation inputs differ from the exact initialized project",
            ));
        }
        let resolution_request = inputs.resolution_request.clone();
        let composition_request = inputs.composition_request.clone();
        let trust_input = inputs.trust_input.clone();
        let rollback_target = match &intent.operation {
            DomainPackInitializedProjectOperation::Rollback {
                target_receipt_digest,
                target_lock_digest,
            } => Some(load_initialized_project_rollback_source(
                &self.store,
                &self.project_snapshot,
                target_receipt_digest,
                target_lock_digest,
            )?),
            DomainPackInitializedProjectOperation::Install { .. }
            | DomainPackInitializedProjectOperation::Upgrade { .. }
            | DomainPackInitializedProjectOperation::Remove { .. }
            | DomainPackInitializedProjectOperation::RebaseCore { .. } => None,
        };
        Ok(DomainPackInitializedProjectDerivationSource {
            expected_state,
            active_pointer: material.pointer.clone(),
            active_lock: material.lock.clone(),
            active_generation,
            rollback_target,
            active_composition: material.composition,
            resolution_request,
            composition_request,
            trust_input,
            lifecycle_operation: material.lifecycle_operation,
        })
    }

    /// Load candidate-only inputs retained by the active generation for an
    /// explicit Core rebase. Legacy generations without these inputs return a
    /// typed blocked result and remain authoritative but non-rebasable.
    ///
    /// # Errors
    ///
    /// Fails closed when the active generation is absent, stale, malformed, or
    /// predates persisted rebase inputs.
    pub fn active_rebase_source(
        &self,
    ) -> Result<DomainPackActiveRebaseSource, DomainPackLifecycleStoreError> {
        self.revalidate_retained_completion()?;
        self.store
            .revalidate_expected_active_pointer(&self.active_pointer_authority)?;
        let material = load_active_generation_material(&self.store, &self.project_snapshot)?;
        let inputs = material
            .rebase_inputs
            .ok_or_else(|| blocked("active generation predates persisted Core-rebase inputs"))?;
        Ok(DomainPackActiveRebaseSource {
            pointer: material.pointer,
            exact_lock: material.lock,
            resolution_request: inputs.resolution_request,
            composition_request: inputs.composition_request,
            trust_input: inputs.trust_input,
            lifecycle_operation: material.lifecycle_operation,
        })
    }

    /// Structurally prepare a transaction while the exact lifecycle lock is
    /// retained. This does not mint commit authority.
    ///
    /// # Errors
    ///
    /// Fails when the preflight is blocked, state is stale, rollback evidence
    /// is unreachable, or any digest/integrity check fails.
    #[allow(clippy::too_many_lines)] // Linear construction keeps receipt/ledger bindings auditable.
    pub fn prepare_candidate(
        &self,
        preflight: DomainPackLifecyclePreflightDocument,
    ) -> Result<PreparedDomainPackLifecycleTransaction, DomainPackLifecycleStoreError> {
        validate_preflight(&preflight)?;
        let body = &preflight.domain_pack_lifecycle_preflight;
        let request = &body.request.domain_pack_lifecycle_request;
        if request.project_id != body.proposed_lock.domain_pack_exact_lock.payload.project_id {
            return Err(invalid("project_id", "request and proposed lock differ"));
        }
        verify_expected_state(&request.expected_state, &self.state)?;
        let observed_at_unix = trusted_now_unix()?;
        let rollback_target = match &request.operation {
            DomainPackLifecycleOperation::Rollback {
                target_receipt_digest,
                target_lock_digest,
            } => Some(load_committed_receipt(
                &self.store,
                &self.state,
                target_receipt_digest,
                target_lock_digest,
            )?),
            _ => None,
        };

        let previous_pointer = self.state.active_pointer.clone();
        self.revalidate_retained_completion()?;
        self.store
            .revalidate_expected_active_pointer(&self.active_pointer_authority)?;
        match (&previous_pointer, &self.active_pointer_authority) {
            (Some(expected), RetainedDomainPackExpectedActivePointer::Present(witness)) => {
                let retained: DomainPackActivePointerDocument = parse_yaml(
                    &self
                        .store
                        .display_path(Path::new(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH)),
                    witness.raw_bytes(),
                )?;
                if &retained != expected {
                    return Err(stale(
                        "retained active-pointer authority differs from lifecycle state",
                        &self.state,
                    ));
                }
            }
            (None, RetainedDomainPackExpectedActivePointer::Absent(_)) => {}
            _ => {
                return Err(stale(
                    "active-pointer presence changed before transaction preparation",
                    &self.state,
                ));
            }
        }
        let generation = previous_pointer.as_ref().map_or(0, |pointer| {
            pointer.domain_pack_active_pointer.generation + 1
        });
        let prior_head = previous_pointer.as_ref().map(|pointer| {
            pointer
                .domain_pack_active_pointer
                .lifecycle_head_digest
                .clone()
        });
        let prior_pointer_digest = previous_pointer
            .as_ref()
            .map(|pointer| pointer.domain_pack_active_pointer.pointer_digest.clone());
        let mut record = DomainPackLifecycleLedgerRecord {
            sequence: generation,
            previous_record_digest: prior_head.clone(),
            operation: request.operation.clone(),
            request_digest: body.request_digest.clone(),
            preflight_digest: body.preflight_digest.clone(),
            from_pointer_digest: prior_pointer_digest,
            to_generation: generation,
            active_lock_digest: body
                .proposed_lock
                .domain_pack_exact_lock
                .lock_digest
                .clone(),
            compatibility_report_digest: body
                .compatibility_report
                .domain_pack_compatibility_report
                .report_digest
                .clone(),
            principal_id: request.principal_id.clone(),
            observed_at_unix,
            record_digest: String::new(),
        };
        record.record_digest = digest_record(&record)?;
        let mut pointer = DomainPackActivePointer {
            project_id: request.project_id.clone(),
            generation,
            active_lock_digest: record.active_lock_digest.clone(),
            lifecycle_head_digest: record.record_digest.clone(),
            pointer_digest: String::new(),
        };
        pointer.pointer_digest = digest_pointer(&pointer)?;
        let next_pointer = DomainPackActivePointerDocument {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            domain_pack_active_pointer: pointer.clone(),
        };
        let lock = &body.proposed_lock.domain_pack_exact_lock.payload;
        let mut receipt = DomainPackLifecycleReceipt {
            receipt_id: StableId(format!("domain-pack.lifecycle.receipt.{generation}")),
            operation: request.operation.clone(),
            principal_id: request.principal_id.clone(),
            request_digest: body.request_digest.clone(),
            preflight_digest: body.preflight_digest.clone(),
            resolution_digest: lock.resolution_digest.clone(),
            composition_digest: lock.composition_digest.clone(),
            compatibility_report_digest: record.compatibility_report_digest.clone(),
            trust_policy_digest: lock.trust_policy_digest.clone(),
            reviewer_registry_digest: lock.reviewer_registry_digest.clone(),
            reviewed_registry_digest: lock.reviewed_registry_digest.clone(),
            capability_registry_digest: lock.capability_registry_digest.clone(),
            sandbox_policy_digest: lock.sandbox_policy_digest.clone(),
            from_state: previous_pointer
                .as_ref()
                .map(|value| value.domain_pack_active_pointer.clone()),
            to_state: pointer,
            prior_ledger_head_digest: prior_head,
            new_ledger_head_digest: record.record_digest.clone(),
            applied_object_digests: staged_digests(body),
            observed_at_unix,
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = digest_receipt(&receipt)?;
        let receipt = DomainPackLifecycleReceiptDocument {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            domain_pack_lifecycle_receipt: receipt,
        };
        Ok(PreparedDomainPackLifecycleTransaction {
            preflight,
            previous_pointer,
            previous_lock: self.state.active_lock.clone(),
            record,
            next_pointer,
            receipt,
            rollback_target,
        })
    }

    /// Consume an opaque authorization and activate the complete prepared
    /// generation. All immutable objects are durable before the pointer flip.
    ///
    /// # Errors
    ///
    /// Fails closed on stale authority/project/state, persistence/recovery
    /// failure, or any post-commit integrity mismatch.
    #[allow(clippy::needless_pass_by_value, clippy::too_many_lines)] // Move-only authority must be consumed exactly once.
    pub fn commit(
        &mut self,
        prepared: PreparedDomainPackLifecycleTransaction,
        authority: DomainPackLifecycleCommitAuthority,
    ) -> Result<DomainPackLifecycleReceiptDocument, DomainPackLifecycleStoreError> {
        let body = &prepared.preflight.domain_pack_lifecycle_preflight;
        if authority.preflight_digest != body.preflight_digest {
            return Err(invalid(
                "commit_authority",
                "authority does not bind the prepared preflight",
            ));
        }
        validate_supply_chain_freshness(
            authority.supply_chain_verified_at_unix,
            authority.supply_chain_expires_at_unix,
        )?;
        self.store
            .validate_project_tree(&authority.project_snapshot)
            .map_err(|error| {
                stale_project_snapshot(
                    authority.project_snapshot.snapshot_digest(),
                    &format!("project changed before lifecycle commit: {error}"),
                )
            })?;
        // Reconcile and consume one exact present/absence session after all
        // policy work and immediately before persistence. The long-lived writer
        // and completion authorities retained at lock acquisition must still
        // name that state.
        self.revalidate_retained_completion()?;
        let (loaded, _recovery, active_pointer_authority) =
            load_current_state_under_lock(&self.store, &self.project_snapshot)?;
        self.state = loaded.projection;
        self.active_pointer_authority = active_pointer_authority;
        self.store
            .revalidate_expected_active_pointer(&self.active_pointer_authority)?;
        verify_expected_state(
            &body.request.domain_pack_lifecycle_request.expected_state,
            &self.state,
        )?;
        if self.state.active_pointer != prepared.previous_pointer {
            return Err(stale(
                "prepared pointer changed under retained lock",
                &self.state,
            ));
        }

        let manifest = materialize_generation(
            &self.store,
            &prepared,
            &authority.verified_artifacts,
            &authority.acquisition_catalog,
            &authority.resolution_request,
            &authority.composition_request,
            &authority.trust_input,
        )?;
        self.store
            .validate_project_tree(&authority.project_snapshot)
            .map_err(|error| {
                stale_project_snapshot(
                    authority.project_snapshot.snapshot_digest(),
                    &format!("project changed while materializing lifecycle generation: {error}"),
                )
            })?;
        validate_materialized_transaction(
            &self.store,
            &prepared,
            &manifest,
            &authority.verified_artifacts,
            &authority.acquisition_catalog,
            &authority.resolution_request,
            &authority.composition_request,
            &authority.trust_input,
        )?;
        publish_committed_receipt(&self.store, &prepared.receipt)?;

        let mut committed_records = self.state.ledger_records.clone();
        committed_records.push(prepared.record.clone());
        let committed_state = DomainPackLifecycleStateProjection {
            active_pointer: Some(prepared.next_pointer.clone()),
            active_lock: Some(body.proposed_lock.clone()),
            ledger_records: committed_records,
        };
        let completion_input = DomainPackLifecycleCompletionInput {
            project_id: body
                .request
                .domain_pack_lifecycle_request
                .project_id
                .0
                .as_str(),
            project_snapshot_digest: &body
                .request
                .domain_pack_lifecycle_request
                .project_snapshot_digest,
            generation: prepared.next_pointer.domain_pack_active_pointer.generation,
            ledger_record_digest: &prepared.record.record_digest,
            lock_digest: &body.proposed_lock.domain_pack_exact_lock.lock_digest,
            preflight_digest: &body.preflight_digest,
            compatibility_report_digest: &body
                .compatibility_report
                .domain_pack_compatibility_report
                .report_digest,
            receipt_digest: &prepared
                .receipt
                .domain_pack_lifecycle_receipt
                .receipt_digest,
            object_raw_digests: &manifest.object_raw_digests,
        };
        let pointer_bytes = yaml_bytes(&prepared.next_pointer)?;
        self.store.validate_current()?;
        let installed_pointer = self.store.replace_active_pointer(
            &self.active_pointer_authority,
            &pointer_bytes,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
        )?;

        match self.store.publish_lifecycle_completion(
            &authority.project_snapshot,
            self.active_pointer_authority.present(),
            &installed_pointer,
            completion_input,
        ) {
            Ok(completion) => {
                // The immutable selector publication inside Store is the success
                // linearization point. Only in-memory authority/state moves follow.
                self.active_pointer_authority =
                    RetainedDomainPackExpectedActivePointer::Present(installed_pointer);
                self.completion_authority = Some(completion);
                self.state = committed_state;
                Ok(prepared.receipt)
            }
            Err(completion_error) => {
                let rollback = rollback_pointer_after_project_drift(
                    &self.store,
                    &self.active_pointer_authority,
                    &installed_pointer,
                );
                Err(invalid(
                    "commit_completion",
                    &format!(
                        "lifecycle commit failed before immutable completion selector publication \
                         ({completion_error}); exact installed-pointer rollback result: \
                         {rollback:?}; immutable generation, receipt, and unselected \
                         completion material remain discoverable recovery debt"
                    ),
                ))
            }
        }
    }
}

fn rollback_pointer_after_project_drift(
    store: &RetainedDomainPackLifecycleStore,
    previous_pointer: &RetainedDomainPackExpectedActivePointer,
    installed_pointer: &RetainedDomainPackActivePointerWitness,
) -> Result<Vec<PathBuf>, DomainPackLifecycleStoreError> {
    store
        .rollback_active_pointer(installed_pointer, previous_pointer.present())
        .map_err(Into::into)
}

/// Mint the move-only commit capability from an opaque verified registry
/// snapshot and the exact deterministic policy result bound into `prepared`.
///
/// This is intentionally the only public authorization path. Serialized
/// supply-chain assessments and preflight documents cannot manufacture the
/// opaque snapshot, and every selected/locked package is joined to one exact
/// verified registry record before authority is created.
///
/// # Errors
///
/// Returns a blocked or integrity error unless every supply-chain, monotonic
/// anchor, operation-intent, resolution, composition, trust, capability,
/// project, artifact, and compatibility binding recomputes exactly.
#[allow(clippy::too_many_lines)] // One explicit TCB proof chain is preferable to hidden partial authority.
pub fn authorize_prepared_domain_pack_lifecycle(
    prepared: &PreparedDomainPackLifecycleTransaction,
    context: &DomainPackLifecycleAuthorizationContext<'_>,
) -> Result<DomainPackLifecycleCommitAuthority, DomainPackLifecycleStoreError> {
    validate_preflight(&prepared.preflight)?;
    let body = &prepared.preflight.domain_pack_lifecycle_preflight;
    let verified_snapshot = context.anchored_snapshot.verified_snapshot();
    let reviewed_snapshot = context.anchored_reviewed_snapshot;
    let resolution = &body.resolution.domain_pack_resolution_projection;
    let composition = &body.composition.domain_pack_composition_projection;
    let lock = &body.proposed_lock.domain_pack_exact_lock;
    validate_supply_chain_freshness(
        verified_snapshot.verified_at_unix(),
        verified_snapshot.expires_at_unix(),
    )?;

    if body.authority != DomainPackCandidateAuthority::CandidateOnly
        || resolution.authority != DomainPackCandidateAuthority::CandidateOnly
        || composition.authority != DomainPackCandidateAuthority::CandidateOnly
    {
        return Err(blocked("candidate projections use an unknown authority"));
    }
    let is_remove = matches!(
        body.request.domain_pack_lifecycle_request.operation,
        DomainPackLifecycleOperation::Remove { .. }
    );
    // Present cumulative revocation is evaluated before resolution replay or any
    // historical lifecycle evidence can reactivate a package. Remove remains the
    // sole exception so an already-active revoked package cannot trap removal.
    if !is_remove
        && lock.payload.packages.iter().any(|locked| {
            context
                .anchored_snapshot
                .is_currently_revoked(&locked.registry_record_digest)
        })
    {
        return Err(blocked(
            "current cumulative supply-chain revocation blocks package activation",
        ));
    }
    if resolution.status != DomainPackResolutionStatus::Resolved || !resolution.issues.is_empty() {
        return Err(blocked("resolution is not clean and resolved"));
    }
    let is_historical_empty_rollback = matches!(
        body.request.domain_pack_lifecycle_request.operation,
        DomainPackLifecycleOperation::Rollback { .. }
    ) && lock.payload.packages.is_empty()
        && prepared.rollback_target.is_some();
    let is_empty_core_rebase = matches!(
        body.request.domain_pack_lifecycle_request.operation,
        DomainPackLifecycleOperation::RebaseCore { .. }
    ) && lock.payload.packages.is_empty();
    let composition_allowed = composition.issues.is_empty()
        && if is_remove || is_historical_empty_rollback || is_empty_core_rebase {
            matches!(
                composition.status,
                DomainPackCompositionStatus::Composable | DomainPackCompositionStatus::Blocked
            )
        } else {
            composition.status == DomainPackCompositionStatus::Composable
                && composition.gaps.is_empty()
        };
    if !composition_allowed {
        return Err(blocked("composition is not clean and composable"));
    }
    let registry_digest = domain_pack_registry_snapshot_digest(context.registry_document)
        .map_err(|error| blocked(&format!("registry digest verification failed: {error}")))?;
    if registry_digest != verified_snapshot.snapshot_digest()
        || lock.payload.registry_snapshot_digest != verified_snapshot.snapshot_digest()
        || lock.payload.reviewer_registry_digest != reviewed_snapshot.reviewer_registry_digest()
        || lock.payload.reviewed_registry_digest != reviewed_snapshot.registry_digest()
        || lock.payload.trust_policy_digest != verified_snapshot.trust_policy_digest()
    {
        return Err(blocked(
            "verified supply-chain, reviewed, reviewer, or trust-policy digest differs from exact lock",
        ));
    }
    if context.trust_policy_document.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION
        || context.trust_policy_document.domain_pack_trust_policy
            != context.trust_input.trust_policy
        || canonical_digest(context.trust_policy_document)?
            != verified_snapshot.trust_policy_digest()
    {
        return Err(blocked(
            "fresh trust input differs from the operator-verified trust policy",
        ));
    }
    let capability_registry_document = DomainPackRuntimeCapabilityRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_runtime_capability_registry: context.trust_input.capability_registry.clone(),
    };
    let sandbox_policy_document = DomainPackCapabilitySandboxPolicyDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_capability_sandbox_policy: context.trust_input.sandbox_policy.clone(),
    };
    if canonical_digest(&capability_registry_document)? != lock.payload.capability_registry_digest
        || canonical_digest(&sandbox_policy_document)? != lock.payload.sandbox_policy_digest
    {
        return Err(blocked(
            "runtime capability registry or sandbox policy differs from exact lock",
        ));
    }
    if lock.payload.resolution_digest != resolution.resolution_digest
        || lock.payload.composition_digest != composition.composition_digest
    {
        return Err(blocked(
            "resolution or composition digest differs from exact lock",
        ));
    }
    let resolution_input = &context.resolution_request.domain_pack_resolution_request;
    let composition_input = &context.composition_request.domain_pack_composition_request;
    let lifecycle_request = &body.request.domain_pack_lifecycle_request;
    validate_operation_intent(
        &lifecycle_request.operation,
        prepared.previous_lock.as_ref(),
        &body.proposed_lock,
        resolution_input,
        prepared.rollback_target.as_ref(),
    )?;
    if canonical_digest(&body.request)? != body.request_digest
        || body.observed_state != lifecycle_request.expected_state
        || resolution.request_id != resolution_input.request_id
        || composition.request_id != composition_input.request_id
        || resolution_input.project_id != lock.payload.project_id
        || resolution_input.core != lock.payload.core
        || composition_input.core != lock.payload.core
        || composition_input.requirements
            != resolution_input
                .requirements
                .domain_pack_project_requirements
        || lock.payload.roots != resolution_input.roots
        || resolution_input.registry_snapshot_digest != lock.payload.registry_snapshot_digest
        || canonical_digest(&resolution_input.requirements)? != lock.payload.requirements_digest
        || canonical_digest(context.resolution_request)?
            != lifecycle_request.resolution_request_digest
    {
        return Err(blocked(
            "resolution/composition inputs differ from exact lock or lifecycle request",
        ));
    }
    let expected_project_snapshot = match &lifecycle_request.expected_state {
        DomainPackExpectedLifecycleState::Uninitialized {
            project_snapshot_digest,
        }
        | DomainPackExpectedLifecycleState::Initialized {
            project_snapshot_digest,
            ..
        } => project_snapshot_digest,
    };
    if expected_project_snapshot != &lifecycle_request.project_snapshot_digest {
        return Err(blocked(
            "lifecycle request and expected state bind different project snapshots",
        ));
    }
    context.project_snapshot.project_tree.revalidate()?;
    if context.project_snapshot.project_tree.snapshot_digest()
        != lifecycle_request.project_snapshot_digest
    {
        return Err(blocked(
            "retained project snapshot differs from lifecycle request",
        ));
    }
    let report = &body.compatibility_report.domain_pack_compatibility_report;
    if report.to_lock_digest != lock.lock_digest
        || report.operation != body.request.domain_pack_lifecycle_request.operation
    {
        return Err(blocked(
            "compatibility report does not bind the exact lifecycle operation and lock",
        ));
    }

    let mut recomputed_resolution =
        resolve_domain_packs(context.resolution_request, context.registry_document);
    let reviewed_join =
        join_reviewed_registry_to_resolution(&recomputed_resolution, reviewed_snapshot.registry());
    if reviewed_join.reviewed_registry_digest != reviewed_snapshot.registry_digest() {
        return Err(blocked(
            "reviewed eligibility join differs from opaque reviewed registry",
        ));
    }
    for selected in &mut recomputed_resolution
        .domain_pack_resolution_projection
        .selected
    {
        let verified = verified_snapshot.entries().iter().find(|entry| {
            let record = entry.record();
            record.record_digest == selected.registry_record_digest
                && record.identity == selected.identity
                && record.package_digest == selected.package.package_digest
                && record.manifest_digest == selected.package.manifest.raw_sha256
                && record.content_digest == selected.package.content.raw_sha256
                && record.license_digest == selected.package.license.raw_sha256
                && record.fixture_digests
                    == selected
                        .package
                        .fixtures
                        .iter()
                        .map(|fixture| fixture.raw_sha256.clone())
                        .collect::<Vec<_>>()
                && record.artifacts.manifest.binding == selected.package.manifest
                && record.artifacts.content.binding.artifact_ref
                    == selected.package.content.content_ref
                && record.artifacts.content.binding.raw_sha256
                    == selected.package.content.raw_sha256
                && record.artifacts.content.binding.canonical_sha256
                    == selected.package.content.canonical_sha256
                && record.artifacts.license.binding == selected.package.license
                && record.artifacts.fixtures.len() == selected.package.fixtures.len()
                && record
                    .artifacts
                    .fixtures
                    .iter()
                    .zip(&selected.package.fixtures)
                    .all(|(descriptor, binding)| &descriptor.binding == binding)
                && record.namespace_grant_id == selected.namespace_grant_id
        });
        if verified.is_none() {
            return Err(blocked(
                "structurally resolved package is absent from opaque verified snapshot",
            ));
        }
        selected.source_assurance = DomainPackSourceAssurance::SupplyChainVerified;
        let Some(review) = reviewed_join.joins.iter().find(|review| {
            review.publisher == selected.identity.publisher.0
                && review.name == selected.identity.name.0
                && review.version == selected.identity.version
                && review.package_digest == selected.package.package_digest
                && review.registry_record_digest == selected.registry_record_digest
        }) else {
            return Err(blocked(
                "resolved package is absent from exact reviewed-registry join",
            ));
        };
        match review.status {
            DomainPackReviewedResolutionJoinStatus::EligibleReviewed => {
                let Some(entry_digest) = review.reviewed_entry_digest.as_ref() else {
                    return Err(blocked("eligible reviewed join lacks an entry digest"));
                };
                let Some(entry) = reviewed_snapshot
                    .registry()
                    .domain_pack_reviewed_registry
                    .entries
                    .iter()
                    .find(|entry| &entry.entry_digest == entry_digest)
                else {
                    return Err(blocked(
                        "eligible reviewed join is absent from opaque reviewed registry",
                    ));
                };
                selected.semantic_assurance = DomainPackSemanticAssurance::Reviewed;
                selected.reviewed_entry_digest = Some(entry.entry_digest.clone());
                selected.promotion_authorization_digest = Some(entry.authorization_digest.clone());
            }
            DomainPackReviewedResolutionJoinStatus::IneligibleDeprecated
            | DomainPackReviewedResolutionJoinStatus::IneligibleRevoked
            | DomainPackReviewedResolutionJoinStatus::IneligibleSuperseded
                if is_remove =>
            {
                let previous = prepared
                    .previous_lock
                    .as_ref()
                    .and_then(|lock| {
                        lock.domain_pack_exact_lock
                            .payload
                            .packages
                            .iter()
                            .find(|package| {
                                package.identity == selected.identity
                                    && package.package_digest == selected.package.package_digest
                                    && package.registry_record_digest
                                        == selected.registry_record_digest
                            })
                    })
                    .ok_or_else(|| {
                        blocked(
                            "remove may retain an ineligible package only from the exact previous lock",
                        )
                    })?;
                if previous.source_assurance != DomainPackSourceAssurance::SupplyChainVerified
                    || previous.semantic_assurance != DomainPackSemanticAssurance::Reviewed
                    || previous.reviewed_entry_digest.is_none()
                    || previous.promotion_authorization_digest.is_none()
                {
                    return Err(blocked(
                        "retained ineligible package lacks prior dual-axis assurance",
                    ));
                }
                selected.semantic_assurance = previous.semantic_assurance;
                selected.reviewed_entry_digest = previous.reviewed_entry_digest.clone();
                selected.promotion_authorization_digest =
                    previous.promotion_authorization_digest.clone();
            }
            _ => {
                return Err(blocked(
                    "selected package is not exactly eligible in the reviewed registry",
                ));
            }
        }
    }
    let promoted = &mut recomputed_resolution.domain_pack_resolution_projection;
    promoted.resolution_digest = domain_pack_resolution_projection_digest(
        context.resolution_request,
        verified_snapshot.snapshot_digest(),
        promoted,
    );
    if recomputed_resolution != body.resolution {
        return Err(blocked(
            "fresh deterministic resolution differs from prepared preflight",
        ));
    }
    validate_reviewed_operation_transition(
        &lifecycle_request.operation,
        prepared.previous_lock.as_ref(),
        &lock.payload.packages,
        &reviewed_join,
    )?;
    let recomputed_composition =
        compose_domain_packs(context.composition_request, context.materials);
    if recomputed_composition != body.composition {
        return Err(blocked(
            "fresh raw-sidecar composition differs from prepared preflight",
        ));
    }
    let compatibility_input = DomainPackCompatibilityInput {
        report_id: report.report_id.clone(),
        operation: body.request.domain_pack_lifecycle_request.operation.clone(),
        sealed_core: lock.payload.core.clone(),
        from_lock: prepared.previous_lock.clone(),
        to_lock: body.proposed_lock.clone(),
    };
    let fresh_compatibility = evaluate_domain_pack_compatibility(&compatibility_input);
    if fresh_compatibility != body.compatibility_report {
        let prepared_report = &body.compatibility_report.domain_pack_compatibility_report;
        let fresh_report = &fresh_compatibility.domain_pack_compatibility_report;
        return Err(blocked(&format!(
            "fresh compatibility evaluation differs from prepared preflight in fields: {}; prepared from lock/composition={:?}/{:?}; fresh={:?}/{:?}",
            compatibility_drift_fields(prepared_report, fresh_report),
            prepared_report.from_lock_digest,
            prepared_report.from_composition_digest,
            fresh_report.from_lock_digest,
            fresh_report.from_composition_digest,
        )));
    }

    let mut verified_records = verified_snapshot
        .entries()
        .iter()
        .map(|entry| (entry.record().record_digest.as_str(), entry.record()))
        .collect::<std::collections::BTreeMap<_, _>>();
    if verified_records.len() != verified_snapshot.entries().len() {
        return Err(blocked(
            "verified snapshot contains duplicate record digests",
        ));
    }
    if resolution.selected.len() != lock.payload.packages.len()
        || body.supply_chain_assessments.len() != lock.payload.packages.len()
    {
        return Err(blocked(
            "resolution, supply-chain assessments, and exact lock differ in cardinality",
        ));
    }

    let mut assessments = body
        .supply_chain_assessments
        .iter()
        .map(|assessment| (assessment.registry_record_digest.as_str(), assessment))
        .collect::<std::collections::BTreeMap<_, _>>();
    if assessments.len() != body.supply_chain_assessments.len() {
        return Err(blocked("supply-chain assessment is duplicated"));
    }
    for locked in &lock.payload.packages {
        if locked.source_assurance != DomainPackSourceAssurance::SupplyChainVerified {
            return Err(blocked("locked package is not supply-chain verified"));
        }
        if locked.semantic_assurance != DomainPackSemanticAssurance::Reviewed
            || locked.reviewed_entry_digest.is_none()
            || locked.promotion_authorization_digest.is_none()
        {
            return Err(blocked(
                "locked package lacks exact reviewed semantic assurance",
            ));
        }
        let Some(record) = verified_records.remove(locked.registry_record_digest.as_str()) else {
            return Err(blocked(
                "locked package has no exact verified registry record",
            ));
        };
        if record.identity != locked.identity
            || record.package_digest != locked.package_digest
            || record.manifest_digest != locked.manifest_binding.raw_sha256
            || record.content_digest != locked.content_binding.raw_sha256
            || record.license_digest != locked.license_binding.raw_sha256
            || record.artifacts.manifest.binding != locked.manifest_binding
            || record.artifacts.content.binding.artifact_ref != locked.content_binding.content_ref
            || record.artifacts.content.binding.raw_sha256 != locked.content_binding.raw_sha256
            || record.artifacts.content.binding.canonical_sha256
                != locked.content_binding.canonical_sha256
            || record.artifacts.license.binding != locked.license_binding
            || record.artifacts.fixtures.len() != locked.fixture_bindings.len()
            || record.namespace_grant_id != locked.namespace_grant_id
        {
            return Err(blocked(
                "locked package differs from verified registry record",
            ));
        }
        let fixture_digests = locked
            .fixture_bindings
            .iter()
            .map(|binding| binding.raw_sha256.clone())
            .collect::<Vec<_>>();
        if fixture_digests != record.fixture_digests
            || !record
                .artifacts
                .fixtures
                .iter()
                .zip(&locked.fixture_bindings)
                .all(|(descriptor, binding)| &descriptor.binding == binding)
        {
            return Err(blocked(
                "locked fixtures differ from verified registry record",
            ));
        }
        let Some(assessment) = assessments.remove(locked.registry_record_digest.as_str()) else {
            return Err(blocked(
                "locked package has no exact supply-chain assessment",
            ));
        };
        if assessment.package_digest != locked.package_digest
            || !assessment.publisher_signature_verified
            || !assessment.registry_signature_threshold_verified
            || !assessment.namespace_grant_verified
            || assessment.revoked
        {
            return Err(blocked("supply-chain assessment is not fully accepted"));
        }
        let selected = resolution
            .selected
            .iter()
            .find(|selected| selected.registry_record_digest == locked.registry_record_digest);
        let Some(selected) = selected else {
            return Err(blocked("locked package was not selected by resolution"));
        };
        if selected.identity != locked.identity
            || selected.package.package_digest != locked.package_digest
            || selected.package.manifest != locked.manifest_binding
            || selected.package.content != locked.content_binding
            || selected.package.license != locked.license_binding
            || selected.package.fixtures != locked.fixture_bindings
            || selected.namespace_grant_id != locked.namespace_grant_id
            || selected.dependencies != locked.dependencies
            || selected.deterministic_order != locked.deterministic_order
            || selected.source_assurance != DomainPackSourceAssurance::SupplyChainVerified
            || selected.semantic_assurance != locked.semantic_assurance
            || selected.reviewed_entry_digest != locked.reviewed_entry_digest
            || selected.promotion_authorization_digest != locked.promotion_authorization_digest
        {
            return Err(blocked("resolved and locked package bindings differ"));
        }
        let composed = composition.ordered_packs.iter().find(|composed| {
            composed.identity == locked.identity
                && composed.deterministic_order == locked.deterministic_order
        });
        let Some(composed) = composed else {
            return Err(blocked("locked package was not included in composition"));
        };
        if composed.manifest_digest != locked.manifest_binding.canonical_sha256
            || composed.content_digest != locked.content_binding.canonical_sha256
        {
            return Err(blocked("composed and locked artifact bindings differ"));
        }
    }
    if composition.ordered_packs.len() != lock.payload.packages.len() {
        return Err(blocked("composition and exact lock differ in cardinality"));
    }
    if lock.payload.unresolved_composition_gaps != composition.gaps {
        return Err(blocked(
            "exact lock does not preserve all unresolved composition gaps",
        ));
    }
    if normalized_artifact_bindings(&body.staged_artifacts)
        != expected_staged_artifact_bindings(&lock.payload.packages)
    {
        return Err(blocked(
            "staged artifact bindings do not exactly cover the locked package set",
        ));
    }
    if !assessments.is_empty() {
        return Err(blocked(
            "preflight contains an unselected supply-chain assessment",
        ));
    }

    if context.trust_input.project_id != lock.payload.project_id
        || context.trust_input.selected.len() != resolution.selected.len()
    {
        return Err(blocked(
            "fresh trust input does not cover the exact resolved project",
        ));
    }
    for selected in &context.trust_input.selected {
        let Some(resolved) = resolution.selected.iter().find(|resolved| {
            resolved.registry_record_digest == selected.package.registry_record_digest
        }) else {
            return Err(blocked("trust input contains an unresolved package"));
        };
        let Some(assessment) = body.supply_chain_assessments.iter().find(|assessment| {
            assessment.registry_record_digest == selected.package.registry_record_digest
        }) else {
            return Err(blocked(
                "trust input package lacks verified supply-chain assessment",
            ));
        };
        if !selected.structurally_valid
            || &selected.package != resolved
            || &selected.supply_chain != assessment
        {
            return Err(blocked(
                "trust input package differs from resolution or verified assessment",
            ));
        }
        let expected_demands =
            derive_domain_pack_capability_demands(&selected.package, composition_input)?;
        if normalized_capability_demands(&selected.capability_demands)
            != normalized_capability_demands(&expected_demands)
        {
            return Err(blocked(
                "trust input capability demands differ from raw composed package semantics",
            ));
        }
    }

    let trust_evaluation = evaluate_domain_pack_trust(context.trust_input);
    if trust_evaluation.status != DomainPackTrustEvaluationStatus::Approved
        || !trust_evaluation.issues.is_empty()
        || trust_evaluation.trust_decisions != body.trust_decisions
        || trust_evaluation.verified_capability_bindings
            != lock.payload.verified_capability_bindings
        || trust_evaluation.capability_gaps != body.capability_gaps
        || trust_evaluation.capability_gaps != lock.payload.unresolved_capability_gaps
    {
        return Err(blocked(
            "fresh trust, capability, or sandbox evaluation differs from preflight lock",
        ));
    }

    let verified_artifacts = verify_immutable_artifacts(context.artifacts, &body.staged_artifacts)?;
    Ok(DomainPackLifecycleCommitAuthority {
        preflight_digest: body.preflight_digest.clone(),
        project_snapshot: Arc::clone(&context.project_snapshot.project_tree),
        supply_chain_verified_at_unix: verified_snapshot.verified_at_unix(),
        supply_chain_expires_at_unix: verified_snapshot.expires_at_unix(),
        verified_artifacts,
        acquisition_catalog: DomainPackAcquisitionCatalogDocument {
            schema_version: DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION.to_owned(),
            forge_core_version: context
                .resolution_request
                .domain_pack_resolution_request
                .forge_core_version
                .clone(),
            core: context
                .resolution_request
                .domain_pack_resolution_request
                .core
                .clone(),
            registry: context.registry_document.clone(),
            candidates: context
                .resolution_request
                .domain_pack_resolution_request
                .candidates
                .clone(),
        },
        resolution_request: context.resolution_request.clone(),
        composition_request: context.composition_request.clone(),
        trust_input: context.trust_input.clone(),
    })
}

#[allow(clippy::too_many_lines)] // One immutable generation binds and syncs every authority input together.
fn materialize_generation(
    store: &RetainedDomainPackLifecycleStore,
    prepared: &PreparedDomainPackLifecycleTransaction,
    artifacts: &[OwnedDomainPackImmutableArtifact],
    acquisition_catalog: &DomainPackAcquisitionCatalogDocument,
    resolution_request: &DomainPackResolutionRequestDocument,
    composition_request: &DomainPackCompositionRequestDocument,
    trust_input: &DomainPackTrustEvaluationInput,
) -> Result<DomainPackGenerationManifest, DomainPackLifecycleStoreError> {
    let body = &prepared.preflight.domain_pack_lifecycle_preflight;
    let record_token = digest_token(&prepared.record.record_digest, "record.record_digest")?;

    for artifact in artifacts {
        let object_token = digest_token(&artifact.binding.raw_sha256, "artifact.raw_sha256")?;
        let object_path = Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT)
            .join("objects")
            .join(object_token);
        write_immutable_under_root(store, &object_path, &artifact.raw_bytes)?;
        let stored =
            read_required_state_bytes(store, &object_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
        if sha256_content_hash(&stored) != artifact.binding.raw_sha256 {
            return Err(invalid(
                "artifact.raw_sha256",
                "object store post-write digest mismatch",
            ));
        }
    }

    let generation_root = generation_root(
        prepared.next_pointer.domain_pack_active_pointer.generation,
        &prepared.record.record_digest,
    )?;
    for (name, bytes) in [
        ("lock.yaml", yaml_bytes(&body.proposed_lock)?),
        ("preflight.yaml", yaml_bytes(&prepared.preflight)?),
        (
            "compatibility.yaml",
            yaml_bytes(&body.compatibility_report)?,
        ),
        ("receipt.yaml", yaml_bytes(&prepared.receipt)?),
        ("catalog.yaml", yaml_bytes(acquisition_catalog)?),
        ("resolution-request.yaml", yaml_bytes(resolution_request)?),
        ("composition-request.yaml", yaml_bytes(composition_request)?),
        ("trust-input.yaml", yaml_bytes(trust_input)?),
    ] {
        write_immutable_under_root(store, &generation_root.join(name), &bytes)?;
    }

    let mut object_raw_digests = artifacts
        .iter()
        .map(|artifact| artifact.binding.raw_sha256.clone())
        .collect::<Vec<_>>();
    object_raw_digests.sort();
    object_raw_digests.dedup();
    let manifest = DomainPackGenerationManifest {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        generation: prepared.next_pointer.domain_pack_active_pointer.generation,
        record_digest: prepared.record.record_digest.clone(),
        lock_digest: body
            .proposed_lock
            .domain_pack_exact_lock
            .lock_digest
            .clone(),
        preflight_digest: body.preflight_digest.clone(),
        compatibility_report_digest: body
            .compatibility_report
            .domain_pack_compatibility_report
            .report_digest
            .clone(),
        receipt_digest: prepared
            .receipt
            .domain_pack_lifecycle_receipt
            .receipt_digest
            .clone(),
        object_raw_digests,
    };
    write_immutable_under_root(
        store,
        &generation_root.join("generation.yaml"),
        &yaml_bytes(&manifest)?,
    )?;
    validate_generation_directory(store, &generation_root, &manifest)?;
    validate_prepared_generation(
        store,
        &generation_root,
        prepared,
        &manifest,
        acquisition_catalog,
        resolution_request,
        composition_request,
        trust_input,
    )?;

    let ledger_path = Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT)
        .join("ledger")
        .join(format!("{record_token}.yaml"));
    write_immutable_under_root(store, &ledger_path, &yaml_bytes(&prepared.record)?)?;
    Ok(manifest)
}

#[allow(clippy::too_many_arguments)]
fn validate_materialized_transaction(
    store: &RetainedDomainPackLifecycleStore,
    prepared: &PreparedDomainPackLifecycleTransaction,
    manifest: &DomainPackGenerationManifest,
    artifacts: &[OwnedDomainPackImmutableArtifact],
    acquisition_catalog: &DomainPackAcquisitionCatalogDocument,
    resolution_request: &DomainPackResolutionRequestDocument,
    composition_request: &DomainPackCompositionRequestDocument,
    trust_input: &DomainPackTrustEvaluationInput,
) -> Result<(), DomainPackLifecycleStoreError> {
    let generation_root = generation_root(
        prepared.next_pointer.domain_pack_active_pointer.generation,
        &prepared.record.record_digest,
    )?;
    validate_generation_directory(store, &generation_root, manifest)?;
    validate_prepared_generation(
        store,
        &generation_root,
        prepared,
        manifest,
        acquisition_catalog,
        resolution_request,
        composition_request,
        trust_input,
    )?;
    let record_token = digest_token(&prepared.record.record_digest, "record.record_digest")?;
    let ledger_path = Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT)
        .join("ledger")
        .join(format!("{record_token}.yaml"));
    if read_required_state_bytes(store, &ledger_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?
        != yaml_bytes(&prepared.record)?
    {
        return Err(invalid(
            "ledger.record",
            "materialized ledger record differs before pointer commit",
        ));
    }
    for artifact in artifacts {
        let object_path = Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT)
            .join("objects")
            .join(digest_token(
                &artifact.binding.raw_sha256,
                "artifact.raw_sha256",
            )?);
        if read_required_state_bytes(store, &object_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?
            .as_slice()
            != artifact.raw_bytes.as_slice()
        {
            return Err(invalid(
                "artifact.raw_sha256",
                "materialized immutable object differs before pointer commit",
            ));
        }
    }
    store.validate_current()?;
    Ok(())
}

fn validate_prepared_generation(
    store: &RetainedDomainPackLifecycleStore,
    generation_root: &Path,
    prepared: &PreparedDomainPackLifecycleTransaction,
    manifest: &DomainPackGenerationManifest,
    acquisition_catalog: &DomainPackAcquisitionCatalogDocument,
    resolution_request: &DomainPackResolutionRequestDocument,
    composition_request: &DomainPackCompositionRequestDocument,
    trust_input: &DomainPackTrustEvaluationInput,
) -> Result<(), DomainPackLifecycleStoreError> {
    let body = &prepared.preflight.domain_pack_lifecycle_preflight;
    for (name, expected) in [
        ("generation.yaml", yaml_bytes(manifest)?),
        ("lock.yaml", yaml_bytes(&body.proposed_lock)?),
        ("preflight.yaml", yaml_bytes(&prepared.preflight)?),
        (
            "compatibility.yaml",
            yaml_bytes(&body.compatibility_report)?,
        ),
        ("receipt.yaml", yaml_bytes(&prepared.receipt)?),
        ("catalog.yaml", yaml_bytes(acquisition_catalog)?),
        ("resolution-request.yaml", yaml_bytes(resolution_request)?),
        ("composition-request.yaml", yaml_bytes(composition_request)?),
        ("trust-input.yaml", yaml_bytes(trust_input)?),
    ] {
        let path = generation_root.join(name);
        let actual = read_required_state_bytes(store, &path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
        if actual != expected {
            return Err(DomainPackLifecycleStoreError::InvalidDocument {
                path: store.display_path(&path),
                reason: "published generation file differs from admitted bytes".to_owned(),
            });
        }
    }
    Ok(())
}

fn publish_committed_receipt(
    store: &RetainedDomainPackLifecycleStore,
    receipt: &DomainPackLifecycleReceiptDocument,
) -> Result<(), DomainPackLifecycleStoreError> {
    let token = digest_token(
        &receipt.domain_pack_lifecycle_receipt.receipt_digest,
        "receipt.receipt_digest",
    )?;
    let receipt_path = Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT)
        .join("receipts")
        .join(format!("{token}.yaml"));
    write_immutable_under_root(store, &receipt_path, &yaml_bytes(receipt)?)
}

fn generation_root(
    generation: u64,
    record_digest: &str,
) -> Result<PathBuf, DomainPackLifecycleStoreError> {
    let token = digest_token(record_digest, "generation.record_digest")?;
    Ok(Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT)
        .join("generations")
        .join(format!("{generation:020}-{token}")))
}

fn validate_generation_directory(
    store: &RetainedDomainPackLifecycleStore,
    generation_root: &Path,
    expected: &DomainPackGenerationManifest,
) -> Result<(), DomainPackLifecycleStoreError> {
    let path = generation_root.join("generation.yaml");
    let bytes = read_required_state_bytes(store, &path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
    let actual: DomainPackGenerationManifest = parse_yaml(&store.display_path(&path), &bytes)?;
    if &actual != expected {
        return Err(invalid(
            "generation_manifest",
            "published generation differs from prepared manifest",
        ));
    }
    Ok(())
}

fn load_current_state_under_lock(
    store: &RetainedDomainPackLifecycleStore,
    project_snapshot: &RetainedProjectTree,
) -> Result<
    (
        LoadedDomainPackLifecycleState,
        CrashReplaceRecovery,
        RetainedDomainPackExpectedActivePointer,
    ),
    DomainPackLifecycleStoreError,
> {
    let session = store.reconcile_active_pointer(DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
    let recovery = session.recovery().clone();
    let loaded = load_state_under_lock(store, project_snapshot, &session)?;
    if let Some(completion) = &loaded.completion {
        let selected = completion.active_pointer_witness().ok_or_else(|| {
            stale(
                "selected completion omitted active-pointer authority",
                &loaded.projection,
            )
        })?;
        store.revalidate_active_pointer(selected)?;
        store.revalidate_lifecycle_completion(completion)?;
    }
    let active_pointer_authority = store.consume_reconciled_active_pointer(session)?;
    match (
        &loaded.projection.active_pointer,
        active_pointer_authority.raw_bytes(),
    ) {
        (Some(expected), Some(bytes)) => {
            let retained: DomainPackActivePointerDocument = parse_yaml(
                &store.display_path(Path::new(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH)),
                bytes,
            )?;
            if &retained != expected {
                return Err(stale(
                    "exact reconciled active pointer differs after completion validation",
                    &loaded.projection,
                ));
            }
        }
        (None, None) => {}
        _ => {
            return Err(stale(
                "active-pointer presence changed while consuming reconciliation authority",
                &loaded.projection,
            ));
        }
    }
    store.validate_current()?;
    Ok((loaded, recovery, active_pointer_authority))
}

#[allow(clippy::too_many_lines)] // Full-generation cross-link validation is deliberately linear.
fn load_state_under_lock(
    store: &RetainedDomainPackLifecycleStore,
    project_snapshot: &RetainedProjectTree,
    active_pointer: &RetainedCrashReplaceSession<'_>,
) -> Result<LoadedDomainPackLifecycleState, DomainPackLifecycleStoreError> {
    let pointer_path = PathBuf::from(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH);
    let Some(pointer_bytes) = active_pointer.raw_bytes() else {
        project_snapshot.revalidate_without_store_owned_file_anchors()?;
        for directory in ["ledger", "generations", "receipts", "objects", "staging"] {
            let path = Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT).join(directory);
            if store.directory_exists(&path)? {
                return Err(DomainPackLifecycleStoreError::InvalidDocument {
                    path: store.display_path(&path),
                    reason: "lifecycle residue exists without an active pointer".to_owned(),
                });
            }
        }
        return Ok(LoadedDomainPackLifecycleState {
            projection: DomainPackLifecycleStateProjection {
                active_pointer: None,
                active_lock: None,
                ledger_records: Vec::new(),
            },
            completion: None,
        });
    };
    let pointer: DomainPackActivePointerDocument =
        parse_yaml(&store.display_path(&pointer_path), pointer_bytes)?;
    validate_schema(&pointer.schema_version, "active_pointer.schema_version")?;
    let pointer_value = &pointer.domain_pack_active_pointer;
    if pointer_value.pointer_digest != digest_pointer(pointer_value)? {
        return Err(invalid("active_pointer.pointer_digest", "digest mismatch"));
    }
    let records = load_ledger_chain(
        store,
        &pointer_value.lifecycle_head_digest,
        pointer_value.generation,
    )?;
    let head = records
        .last()
        .ok_or_else(|| invalid("ledger.head", "active pointer has no ledger head"))?;
    if head.record_digest != pointer_value.lifecycle_head_digest
        || head.active_lock_digest != pointer_value.active_lock_digest
        || head.to_generation != pointer_value.generation
    {
        return Err(invalid(
            "ledger.head",
            "head record does not bind the active pointer generation and lock",
        ));
    }

    let root = generation_root(pointer_value.generation, &head.record_digest)?;
    let manifest_path = root.join("generation.yaml");
    let manifest: DomainPackGenerationManifest = parse_yaml(
        &manifest_path,
        &read_required_state_bytes(store, &manifest_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?,
    )?;
    if manifest.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION
        || manifest.generation != pointer_value.generation
        || manifest.record_digest != head.record_digest
        || manifest.lock_digest != pointer_value.active_lock_digest
        || manifest.preflight_digest != head.preflight_digest
        || manifest.compatibility_report_digest != head.compatibility_report_digest
    {
        return Err(invalid(
            "generation_manifest",
            "manifest does not bind pointer and ledger head",
        ));
    }

    let lock_path = root.join("lock.yaml");
    let lock: DomainPackExactLockDocument = parse_yaml(
        &lock_path,
        &read_required_state_bytes(store, &lock_path, DOMAIN_PACK_MAX_LOCK_BYTES)?,
    )?;
    validate_exact_lock(&lock)?;
    if lock.domain_pack_exact_lock.lock_digest != pointer_value.active_lock_digest
        || lock.domain_pack_exact_lock.payload.project_id != pointer_value.project_id
    {
        return Err(invalid(
            "active_lock",
            "generation lock differs from pointer",
        ));
    }

    let preflight_path = root.join("preflight.yaml");
    let preflight: DomainPackLifecyclePreflightDocument = parse_yaml(
        &preflight_path,
        &read_required_state_bytes(store, &preflight_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?,
    )?;
    validate_preflight(&preflight)?;
    let preflight_value = &preflight.domain_pack_lifecycle_preflight;
    if preflight_value.preflight_digest != manifest.preflight_digest
        || preflight_value.proposed_lock != lock
        || preflight_value.request_digest != head.request_digest
        || preflight_value
            .request
            .domain_pack_lifecycle_request
            .operation
            != head.operation
    {
        return Err(invalid(
            "generation.preflight",
            "preflight does not bind lock, request, and ledger head",
        ));
    }

    let compatibility_path = root.join("compatibility.yaml");
    let compatibility: forge_core_contracts::DomainPackCompatibilityReportDocument = parse_yaml(
        &compatibility_path,
        &read_required_state_bytes(store, &compatibility_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?,
    )?;
    if compatibility != preflight_value.compatibility_report
        || compatibility.domain_pack_compatibility_report.report_digest
            != manifest.compatibility_report_digest
        || compatibility.domain_pack_compatibility_report.operation != head.operation
    {
        return Err(invalid(
            "generation.compatibility",
            "compatibility report does not bind preflight and ledger head",
        ));
    }

    let receipt_path = root.join("receipt.yaml");
    let receipt: DomainPackLifecycleReceiptDocument = parse_yaml(
        &receipt_path,
        &read_required_state_bytes(store, &receipt_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?,
    )?;
    let receipt_value = &receipt.domain_pack_lifecycle_receipt;
    if receipt_value.receipt_digest != manifest.receipt_digest
        || digest_receipt(receipt_value)? != manifest.receipt_digest
        || receipt_value.new_ledger_head_digest != head.record_digest
        || receipt_value.to_state != *pointer_value
        || receipt_value.operation != head.operation
        || receipt_value.preflight_digest != head.preflight_digest
        || receipt_value.trust_policy_digest
            != lock.domain_pack_exact_lock.payload.trust_policy_digest
        || receipt_value.reviewer_registry_digest
            != lock.domain_pack_exact_lock.payload.reviewer_registry_digest
        || receipt_value.reviewed_registry_digest
            != lock.domain_pack_exact_lock.payload.reviewed_registry_digest
        || receipt_value.capability_registry_digest
            != lock
                .domain_pack_exact_lock
                .payload
                .capability_registry_digest
        || receipt_value.sandbox_policy_digest
            != lock.domain_pack_exact_lock.payload.sandbox_policy_digest
    {
        return Err(invalid(
            "generation.receipt",
            "receipt is not the completion evidence for the active pointer",
        ));
    }

    let mut expected_objects = preflight_value
        .staged_artifacts
        .iter()
        .map(|binding| binding.raw_sha256.clone())
        .collect::<Vec<_>>();
    expected_objects.sort();
    expected_objects.dedup();
    if manifest.object_raw_digests != expected_objects {
        return Err(invalid(
            "generation.objects",
            "generation object manifest differs from staged artifacts",
        ));
    }
    for digest in &manifest.object_raw_digests {
        let token = digest_token(digest, "generation.object_raw_digest")?;
        let object_path = Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT)
            .join("objects")
            .join(token);
        let bytes = read_required_state_bytes(store, &object_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
        if sha256_content_hash(&bytes) != *digest {
            return Err(invalid("generation.objects", "object digest mismatch"));
        }
    }

    let completion_input = DomainPackLifecycleCompletionInput {
        project_id: pointer_value.project_id.0.as_str(),
        project_snapshot_digest: &preflight_value
            .request
            .domain_pack_lifecycle_request
            .project_snapshot_digest,
        generation: pointer_value.generation,
        ledger_record_digest: &head.record_digest,
        lock_digest: &manifest.lock_digest,
        preflight_digest: &manifest.preflight_digest,
        compatibility_report_digest: &manifest.compatibility_report_digest,
        receipt_digest: &manifest.receipt_digest,
        object_raw_digests: &manifest.object_raw_digests,
    };
    let completion = store.validate_selected_lifecycle_completion(
        project_snapshot,
        active_pointer,
        completion_input,
    )?;
    // The immutable selector, not a mutable self-declaring record or a later
    // pathname reopen, is the success authority. Its exact handles and lifetime
    // anchors remain owned by the higher-level lifecycle guard.
    Ok(LoadedDomainPackLifecycleState {
        projection: DomainPackLifecycleStateProjection {
            active_pointer: Some(pointer),
            active_lock: Some(lock),
            ledger_records: records,
        },
        completion: Some(completion),
    })
}

#[allow(clippy::too_many_lines)] // Historical authority is deliberately checked in one linear closure walk.
fn load_raw_lifecycle_inventory(
    store: &RetainedDomainPackLifecycleStore,
    project_snapshot: &RetainedProjectTree,
    state: &DomainPackLifecycleStateProjection,
) -> Result<DomainPackRawLifecycleInventory, DomainPackLifecycleStoreError> {
    store.validate_current()?;
    let mut files = BTreeMap::<String, Vec<u8>>::new();
    let Some(active_pointer) = state.active_pointer.as_ref() else {
        if state.active_lock.is_some() || !state.ledger_records.is_empty() {
            return Err(invalid(
                "raw_inventory",
                "uninitialized inventory contains reachable lifecycle authority",
            ));
        }
        return Ok(DomainPackRawLifecycleInventory { files: Vec::new() });
    };
    let active_pointer_raw = inventory_read(
        store,
        Path::new(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH),
        DOMAIN_PACK_MAX_DOCUMENT_BYTES,
        &mut files,
    )?;
    let captured_pointer: DomainPackActivePointerDocument = parse_yaml(
        &store.display_path(Path::new(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH)),
        &active_pointer_raw,
    )?;
    if &captured_pointer != active_pointer {
        return Err(invalid(
            "raw_inventory.active_pointer",
            "captured pointer differs from the retained lifecycle projection",
        ));
    }

    let mut prior_state: Option<DomainPackActivePointer> = None;
    for record in &state.ledger_records {
        let record_token = digest_token(&record.record_digest, "ledger.record_digest")?;
        let ledger_path = Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT)
            .join("ledger")
            .join(format!("{record_token}.yaml"));
        let ledger_raw = inventory_read(
            store,
            &ledger_path,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
            &mut files,
        )?;
        let persisted_record: DomainPackLifecycleLedgerRecord =
            parse_yaml(&store.display_path(&ledger_path), &ledger_raw)?;
        if &persisted_record != record || digest_record(&persisted_record)? != record.record_digest
        {
            return Err(invalid(
                "raw_inventory.ledger",
                "persisted ledger record differs from the reachable chain",
            ));
        }

        let generation_root = generation_root(record.to_generation, &record.record_digest)?;
        let generation_path = generation_root.join("generation.yaml");
        let generation_raw = inventory_read(
            store,
            &generation_path,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
            &mut files,
        )?;
        let manifest: DomainPackGenerationManifest =
            parse_yaml(&store.display_path(&generation_path), &generation_raw)?;
        if manifest.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION
            || manifest.generation != record.to_generation
            || manifest.record_digest != record.record_digest
            || manifest.lock_digest != record.active_lock_digest
            || manifest.preflight_digest != record.preflight_digest
            || manifest.compatibility_report_digest != record.compatibility_report_digest
        {
            return Err(invalid(
                "raw_inventory.generation_manifest",
                "historical manifest differs from its ledger record",
            ));
        }
        for completion_leaf in ["completion.record", "completion.selector"] {
            inventory_read(
                store,
                &generation_root.join(completion_leaf),
                DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_BYTES,
                &mut files,
            )?;
        }

        let lock_path = generation_root.join("lock.yaml");
        let lock_raw = inventory_read(store, &lock_path, DOMAIN_PACK_MAX_LOCK_BYTES, &mut files)?;
        let lock: DomainPackExactLockDocument =
            parse_yaml(&store.display_path(&lock_path), &lock_raw)?;
        validate_exact_lock(&lock)?;
        if lock.domain_pack_exact_lock.lock_digest != manifest.lock_digest {
            return Err(invalid(
                "raw_inventory.lock",
                "historical exact lock differs from its manifest",
            ));
        }

        let preflight_path = generation_root.join("preflight.yaml");
        let preflight_raw = inventory_read(
            store,
            &preflight_path,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
            &mut files,
        )?;
        let preflight: DomainPackLifecyclePreflightDocument =
            parse_yaml(&store.display_path(&preflight_path), &preflight_raw)?;
        validate_preflight(&preflight)?;
        let preflight_value = &preflight.domain_pack_lifecycle_preflight;
        if preflight_value.preflight_digest != manifest.preflight_digest
            || preflight_value.proposed_lock != lock
            || preflight_value.request_digest != record.request_digest
            || preflight_value
                .request
                .domain_pack_lifecycle_request
                .operation
                != record.operation
        {
            return Err(invalid(
                "raw_inventory.preflight",
                "historical preflight differs from its lock or ledger record",
            ));
        }

        let compatibility_path = generation_root.join("compatibility.yaml");
        let compatibility_raw = inventory_read(
            store,
            &compatibility_path,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
            &mut files,
        )?;
        let compatibility: forge_core_contracts::DomainPackCompatibilityReportDocument =
            parse_yaml(&store.display_path(&compatibility_path), &compatibility_raw)?;
        if compatibility != preflight_value.compatibility_report
            || compatibility.domain_pack_compatibility_report.report_digest
                != manifest.compatibility_report_digest
            || compatibility.domain_pack_compatibility_report.operation != record.operation
        {
            return Err(invalid(
                "raw_inventory.compatibility",
                "historical compatibility report differs from preflight or ledger",
            ));
        }

        let receipt_path = generation_root.join("receipt.yaml");
        let receipt_raw = inventory_read(
            store,
            &receipt_path,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
            &mut files,
        )?;
        let receipt: DomainPackLifecycleReceiptDocument =
            parse_yaml(&store.display_path(&receipt_path), &receipt_raw)?;
        let receipt_value = &receipt.domain_pack_lifecycle_receipt;
        let expected_prior_head = prior_state
            .as_ref()
            .map(|pointer| pointer.lifecycle_head_digest.clone());
        let expected_prior_pointer = prior_state
            .as_ref()
            .map(|pointer| pointer.pointer_digest.clone());
        if receipt_value.receipt_digest != manifest.receipt_digest
            || digest_receipt(receipt_value)? != manifest.receipt_digest
            || receipt_value.from_state.as_ref() != prior_state.as_ref()
            || receipt_value.prior_ledger_head_digest.as_ref() != expected_prior_head.as_ref()
            || record.from_pointer_digest.as_ref() != expected_prior_pointer.as_ref()
            || receipt_value.to_state.project_id != lock.domain_pack_exact_lock.payload.project_id
            || receipt_value.to_state.generation != record.to_generation
            || receipt_value.to_state.active_lock_digest != record.active_lock_digest
            || receipt_value.to_state.lifecycle_head_digest != record.record_digest
            || digest_pointer(&receipt_value.to_state)? != receipt_value.to_state.pointer_digest
            || receipt_value.new_ledger_head_digest != record.record_digest
            || receipt_value.receipt_id
                != StableId(format!("domain-pack.lifecycle.receipt.{}", record.sequence))
            || receipt_value.operation != record.operation
            || receipt_value.request_digest != record.request_digest
            || receipt_value.preflight_digest != record.preflight_digest
            || receipt_value.compatibility_report_digest != record.compatibility_report_digest
            || receipt_value.resolution_digest
                != lock.domain_pack_exact_lock.payload.resolution_digest
            || receipt_value.composition_digest
                != lock.domain_pack_exact_lock.payload.composition_digest
            || receipt_value.applied_object_digests != staged_digests(preflight_value)
            || receipt_value.principal_id != record.principal_id
            || receipt_value.observed_at_unix != record.observed_at_unix
            || receipt_value.trust_policy_digest
                != lock.domain_pack_exact_lock.payload.trust_policy_digest
            || receipt_value.reviewer_registry_digest
                != lock.domain_pack_exact_lock.payload.reviewer_registry_digest
            || receipt_value.reviewed_registry_digest
                != lock.domain_pack_exact_lock.payload.reviewed_registry_digest
            || receipt_value.capability_registry_digest
                != lock
                    .domain_pack_exact_lock
                    .payload
                    .capability_registry_digest
            || receipt_value.sandbox_policy_digest
                != lock.domain_pack_exact_lock.payload.sandbox_policy_digest
        {
            return Err(invalid(
                "raw_inventory.receipt",
                "historical receipt severs lifecycle state continuity",
            ));
        }
        let committed_receipt_path = Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT)
            .join("receipts")
            .join(format!(
                "{}.yaml",
                digest_token(&manifest.receipt_digest, "receipt.receipt_digest")?
            ));
        let committed_receipt_raw = inventory_read(
            store,
            &committed_receipt_path,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
            &mut files,
        )?;
        if committed_receipt_raw != receipt_raw {
            return Err(invalid(
                "raw_inventory.receipt",
                "committed receipt differs from its immutable generation",
            ));
        }

        let catalog_path = generation_root.join("catalog.yaml");
        let catalog_raw = inventory_read(
            store,
            &catalog_path,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
            &mut files,
        )?;
        let catalog: DomainPackAcquisitionCatalogDocument =
            parse_yaml(&store.display_path(&catalog_path), &catalog_raw)?;
        let resolution_path = generation_root.join("resolution-request.yaml");
        let resolution_raw = inventory_read(
            store,
            &resolution_path,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
            &mut files,
        )?;
        let resolution_request: DomainPackResolutionRequestDocument =
            parse_yaml(&store.display_path(&resolution_path), &resolution_raw)?;
        let composition_path = generation_root.join("composition-request.yaml");
        let composition_raw = inventory_read(
            store,
            &composition_path,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
            &mut files,
        )?;
        let composition_request: DomainPackCompositionRequestDocument =
            parse_yaml(&store.display_path(&composition_path), &composition_raw)?;
        let trust_path = generation_root.join("trust-input.yaml");
        let trust_raw = inventory_read(
            store,
            &trust_path,
            DOMAIN_PACK_MAX_DOCUMENT_BYTES,
            &mut files,
        )?;
        let trust_input: DomainPackTrustEvaluationInput =
            parse_yaml(&store.display_path(&trust_path), &trust_raw)?;
        let catalog_registry_digest = domain_pack_registry_snapshot_digest(&catalog.registry)
            .map_err(|error| {
                invalid(
                    "raw_inventory.catalog.registry",
                    &format!("registry digest verification failed: {error}"),
                )
            })?;

        if catalog.schema_version != DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION
            || catalog.forge_core_version
                != resolution_request
                    .domain_pack_resolution_request
                    .forge_core_version
            || catalog.core != resolution_request.domain_pack_resolution_request.core
            || catalog.candidates != resolution_request.domain_pack_resolution_request.candidates
            || catalog_registry_digest
                != catalog
                    .registry
                    .domain_pack_supply_chain_registry
                    .snapshot_digest
            || catalog_registry_digest
                != resolution_request
                    .domain_pack_resolution_request
                    .registry_snapshot_digest
            || catalog
                .registry
                .domain_pack_supply_chain_registry
                .snapshot_digest
                != resolution_request
                    .domain_pack_resolution_request
                    .registry_snapshot_digest
        {
            return Err(invalid(
                "raw_inventory.catalog",
                "historical acquisition catalog does not exactly bind its resolver input",
            ));
        }
        validate_schema(
            &catalog.registry.schema_version,
            "raw_inventory.catalog.registry.schema_version",
        )?;
        validate_schema(
            &resolution_request.schema_version,
            "raw_inventory.resolution_request.schema_version",
        )?;
        validate_schema_version(
            &composition_request.schema_version,
            DOMAIN_PACK_SCHEMA_VERSION,
            "raw_inventory.composition_request.schema_version",
        )?;
        let lifecycle_request = &preflight_value.request.domain_pack_lifecycle_request;
        let resolution_projection = &preflight_value.resolution.domain_pack_resolution_projection;
        let composition_projection = &preflight_value
            .composition
            .domain_pack_composition_projection;
        let resolution_input = &resolution_request.domain_pack_resolution_request;
        let composition_input = &composition_request.domain_pack_composition_request;
        let lock_value = &lock.domain_pack_exact_lock;
        if canonical_digest(&resolution_request)? != lifecycle_request.resolution_request_digest
            || resolution_input.request_id != resolution_projection.request_id
            || resolution_input.authority != resolution_projection.authority
            || resolution_input.project_id != lifecycle_request.project_id
            || resolution_input.project_id != lock_value.payload.project_id
            || resolution_input.core != lock_value.payload.core
            || resolution_input.roots != lock_value.payload.roots
            || resolution_input.registry_snapshot_digest
                != lock_value.payload.registry_snapshot_digest
            || resolution_projection.resolution_digest != lock_value.payload.resolution_digest
            || canonical_digest(&resolution_input.requirements)?
                != lock_value.payload.requirements_digest
            || composition_input.request_id != composition_projection.request_id
            || composition_input.authority != composition_projection.authority
            || composition_input.forge_core_version != resolution_input.forge_core_version
            || composition_input.core != lock_value.payload.core
            || composition_projection.composition_digest != lock_value.payload.composition_digest
            || composition_input.requirements
                != resolution_input
                    .requirements
                    .domain_pack_project_requirements
        {
            return Err(invalid(
                "raw_inventory.rebase_inputs",
                "historical resolution or composition input differs from its generation",
            ));
        }

        let mut expected_objects = preflight_value
            .staged_artifacts
            .iter()
            .map(|binding| binding.raw_sha256.clone())
            .collect::<Vec<_>>();
        expected_objects.sort();
        expected_objects.dedup();
        if manifest.object_raw_digests != expected_objects {
            return Err(invalid(
                "raw_inventory.objects",
                "historical manifest does not preserve the complete artifact closure",
            ));
        }
        let mut object_bytes = BTreeMap::<String, Vec<u8>>::new();
        for digest in &manifest.object_raw_digests {
            let object_path = Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT)
                .join("objects")
                .join(digest_token(digest, "generation.object_raw_digest")?);
            let bytes = inventory_read(
                store,
                &object_path,
                DOMAIN_PACK_MAX_DOCUMENT_BYTES,
                &mut files,
            )?;
            if sha256_content_hash(&bytes) != *digest {
                return Err(invalid(
                    "raw_inventory.objects",
                    "historical immutable object digest mismatch",
                ));
            }
            object_bytes.insert(digest.clone(), bytes);
        }
        let materials = lock_value
            .payload
            .packages
            .iter()
            .map(|package| {
                let manifest_raw = object_bytes
                    .get(&package.manifest_binding.raw_sha256)
                    .ok_or_else(|| blocked("historical package manifest object is missing"))?;
                let content_raw = object_bytes
                    .get(&package.content_binding.raw_sha256)
                    .ok_or_else(|| blocked("historical package content object is missing"))?;
                let license_raw = object_bytes
                    .get(&package.license_binding.raw_sha256)
                    .ok_or_else(|| blocked("historical package license object is missing"))?;
                Ok(DomainPackCandidateMaterial {
                    publisher: &package.identity.publisher.0,
                    name: &package.identity.name.0,
                    version: &package.identity.version,
                    manifest_raw,
                    content_raw,
                    license_raw,
                })
            })
            .collect::<Result<Vec<_>, DomainPackLifecycleStoreError>>()?;
        if compose_domain_packs(&composition_request, &materials) != preflight_value.composition {
            return Err(invalid(
                "raw_inventory.composition",
                "historical composition input does not reproduce its projection",
            ));
        }
        validate_persisted_trust_input(
            &trust_input,
            resolution_input.project_id.clone(),
            &preflight_value.resolution,
            &preflight_value.supply_chain_assessments,
            composition_input,
            lock_value,
            preflight_value,
        )?;

        prior_state = Some(receipt_value.to_state.clone());
    }

    if prior_state.as_ref() != Some(&active_pointer.domain_pack_active_pointer) {
        return Err(invalid(
            "raw_inventory.active_pointer",
            "active pointer differs from the terminal historical receipt",
        ));
    }
    for (relative_path, expected) in &files {
        let maximum = if relative_path.ends_with("/completion.record")
            || relative_path.ends_with("/completion.selector")
        {
            DOMAIN_PACK_MAX_PROJECT_SNAPSHOT_BYTES
        } else {
            DOMAIN_PACK_MAX_DOCUMENT_BYTES
        };
        let observed = read_required_state_bytes(store, Path::new(relative_path), maximum)?;
        if &observed != expected {
            return Err(invalid(
                "raw_inventory.stability",
                "lifecycle file changed while capturing the retained inventory",
            ));
        }
    }
    let final_state = load_current_state_under_lock(store, project_snapshot)?
        .0
        .projection;
    if &final_state != state {
        return Err(stale(
            "Domain Pack lifecycle changed during raw inventory capture",
            &final_state,
        ));
    }
    Ok(DomainPackRawLifecycleInventory {
        files: files
            .into_iter()
            .map(|(relative_path, raw_bytes)| DomainPackRawLifecycleFile {
                relative_path,
                raw_bytes,
            })
            .collect(),
    })
}

fn raw_inventory_required<'a>(
    inventory: &'a DomainPackRawLifecycleInventory,
    path: &Path,
) -> Result<&'a [u8], DomainPackLifecycleStoreError> {
    raw_inventory_optional(inventory, path)?.ok_or_else(|| {
        DomainPackLifecycleStoreError::InvalidDocument {
            path: path.to_path_buf(),
            reason: "canonical lifecycle inventory omitted a required file".to_owned(),
        }
    })
}

fn raw_inventory_optional<'a>(
    inventory: &'a DomainPackRawLifecycleInventory,
    path: &Path,
) -> Result<Option<&'a [u8]>, DomainPackLifecycleStoreError> {
    let relative = path
        .to_str()
        .ok_or_else(|| invalid("raw_inventory.path", "lifecycle path is not UTF-8"))?
        .replace('\\', "/");
    Ok(inventory
        .files
        .iter()
        .find(|file| file.relative_path == relative)
        .map(|file| file.raw_bytes.as_slice()))
}

fn inventory_read(
    store: &RetainedDomainPackLifecycleStore,
    path: &Path,
    maximum: u64,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<Vec<u8>, DomainPackLifecycleStoreError> {
    let bytes = read_required_state_bytes(store, path, maximum)?;
    let relative_path = path
        .to_str()
        .ok_or_else(|| invalid("raw_inventory.path", "lifecycle path is not UTF-8"))?
        .replace('\\', "/");
    if let Some(previous) = files.insert(relative_path, bytes.clone()) {
        if previous != bytes {
            return Err(invalid(
                "raw_inventory.path",
                "one lifecycle path resolved to different exact bytes",
            ));
        }
    }
    Ok(bytes)
}

#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
fn validate_persisted_trust_input(
    trust_input: &DomainPackTrustEvaluationInput,
    project_id: StableId,
    resolution: &forge_core_contracts::DomainPackResolutionProjectionDocument,
    assessments: &[forge_core_contracts::DomainPackSupplyChainAssessment],
    composition: &forge_core_contracts::DomainPackCompositionRequest,
    lock: &forge_core_contracts::DomainPackExactLock,
    preflight: &forge_core_contracts::DomainPackLifecyclePreflight,
) -> Result<(), DomainPackLifecycleStoreError> {
    if trust_input.project_id != project_id
        || trust_input.selected.len() != resolution.domain_pack_resolution_projection.selected.len()
    {
        return Err(blocked(
            "persisted trust input does not cover the exact historical project",
        ));
    }
    for selected in &trust_input.selected {
        let Some(resolved) = resolution
            .domain_pack_resolution_projection
            .selected
            .iter()
            .find(|resolved| {
                resolved.registry_record_digest == selected.package.registry_record_digest
            })
        else {
            return Err(blocked(
                "persisted trust input contains an unresolved package",
            ));
        };
        let Some(assessment) = assessments.iter().find(|assessment| {
            assessment.registry_record_digest == selected.package.registry_record_digest
        }) else {
            return Err(blocked(
                "persisted trust input package lacks its supply-chain assessment",
            ));
        };
        let expected_demands =
            derive_domain_pack_capability_demands(&selected.package, composition)?;
        if !selected.structurally_valid
            || &selected.package != resolved
            || &selected.supply_chain != assessment
            || normalized_capability_demands(&selected.capability_demands)
                != normalized_capability_demands(&expected_demands)
        {
            return Err(blocked(
                "persisted trust input differs from resolution, assessment, or raw capability demands",
            ));
        }
    }
    let trust_policy_document = DomainPackTrustPolicyDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_trust_policy: trust_input.trust_policy.clone(),
    };
    let capability_registry_document = DomainPackRuntimeCapabilityRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_runtime_capability_registry: trust_input.capability_registry.clone(),
    };
    let sandbox_policy_document = DomainPackCapabilitySandboxPolicyDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_capability_sandbox_policy: trust_input.sandbox_policy.clone(),
    };
    if canonical_digest(&trust_policy_document)? != lock.payload.trust_policy_digest
        || canonical_digest(&capability_registry_document)?
            != lock.payload.capability_registry_digest
        || canonical_digest(&sandbox_policy_document)? != lock.payload.sandbox_policy_digest
    {
        return Err(blocked(
            "persisted trust, capability, or sandbox policy differs from the exact lock",
        ));
    }
    let evaluation = evaluate_domain_pack_trust(trust_input);
    if evaluation.status != DomainPackTrustEvaluationStatus::Approved
        || !evaluation.issues.is_empty()
        || evaluation.trust_decisions != preflight.trust_decisions
        || evaluation.verified_capability_bindings != lock.payload.verified_capability_bindings
        || evaluation.capability_gaps != preflight.capability_gaps
        || evaluation.capability_gaps != lock.payload.unresolved_capability_gaps
    {
        return Err(blocked(
            "persisted trust input does not reproduce the historical trust decision",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // Keep the durable execution boundary explicit and auditable.
fn load_active_generation_material(
    store: &RetainedDomainPackLifecycleStore,
    project_snapshot: &RetainedProjectTree,
) -> Result<ActiveDomainPackGenerationMaterial, DomainPackLifecycleStoreError> {
    let state = load_current_state_under_lock(store, project_snapshot)?
        .0
        .projection;
    let inventory = load_raw_lifecycle_inventory(store, project_snapshot, &state)?;
    let pointer = state
        .active_pointer
        .ok_or_else(|| blocked("no Domain Pack generation is active"))?;
    let lock = state
        .active_lock
        .ok_or_else(|| blocked("active pointer has no exact lock"))?;
    let head = state
        .ledger_records
        .last()
        .ok_or_else(|| blocked("active pointer has no lifecycle ledger head"))?;
    let generation_root = generation_root(
        pointer.domain_pack_active_pointer.generation,
        &head.record_digest,
    )?;

    let pointer_path = PathBuf::from(DOMAIN_PACK_ACTIVE_LOCK_RELATIVE_PATH);
    let pointer_raw = raw_inventory_required(&inventory, &pointer_path)?;
    let generation_manifest_path = generation_root.join("generation.yaml");
    let generation_manifest_raw = raw_inventory_required(&inventory, &generation_manifest_path)?;
    let lock_path = generation_root.join("lock.yaml");
    let lock_raw = raw_inventory_required(&inventory, &lock_path)?;
    let preflight_path = generation_root.join("preflight.yaml");
    let preflight_raw = raw_inventory_required(&inventory, &preflight_path)?;
    let preflight: DomainPackLifecyclePreflightDocument =
        parse_yaml(&preflight_path, preflight_raw)?;
    let rebase_input_paths = [
        generation_root.join("resolution-request.yaml"),
        generation_root.join("composition-request.yaml"),
        generation_root.join("trust-input.yaml"),
    ];
    let rebase_input_bytes = rebase_input_paths
        .iter()
        .map(|path| raw_inventory_optional(&inventory, path))
        .collect::<Result<Vec<_>, _>>()?;
    let rebase_inputs = match rebase_input_bytes.as_slice() {
        [None, None, None] => None,
        [Some(resolution), Some(composition), Some(trust)] => {
            let resolution_request: DomainPackResolutionRequestDocument =
                parse_yaml(&rebase_input_paths[0], resolution)?;
            let composition_request: DomainPackCompositionRequestDocument =
                parse_yaml(&rebase_input_paths[1], composition)?;
            let trust_input: DomainPackTrustEvaluationInput =
                parse_yaml(&rebase_input_paths[2], trust)?;
            let lifecycle_request = &preflight
                .domain_pack_lifecycle_preflight
                .request
                .domain_pack_lifecycle_request;
            if canonical_digest(&resolution_request)? != lifecycle_request.resolution_request_digest
                || resolution_request.domain_pack_resolution_request.project_id
                    != lifecycle_request.project_id
                || composition_request
                    .domain_pack_composition_request
                    .requirements
                    != resolution_request
                        .domain_pack_resolution_request
                        .requirements
                        .domain_pack_project_requirements
                || trust_input.project_id != lifecycle_request.project_id
            {
                return Err(blocked(
                    "persisted rebase inputs differ from the admitted lifecycle generation",
                ));
            }
            Some(ActiveDomainPackRebaseInputs {
                resolution_request,
                composition_request,
                trust_input,
            })
        }
        _ => {
            return Err(blocked(
                "persisted rebase inputs are incomplete for the active generation",
            ));
        }
    };
    let composition = preflight
        .domain_pack_lifecycle_preflight
        .composition
        .clone();
    let catalog_path = generation_root.join("catalog.yaml");
    let catalog_raw = raw_inventory_optional(&inventory, &catalog_path)?;
    let initialized_generation = match (catalog_raw, &rebase_inputs) {
        (Some(raw), Some(inputs)) => {
            let catalog: DomainPackAcquisitionCatalogDocument = parse_yaml(&catalog_path, raw)?;
            let resolution_projection =
                preflight.domain_pack_lifecycle_preflight.resolution.clone();
            let generation = DomainPackInitializedProjectGenerationMaterial {
                requirements: inputs
                    .resolution_request
                    .domain_pack_resolution_request
                    .requirements
                    .clone(),
                catalog,
                resolution_request: inputs.resolution_request.clone(),
                resolution_projection,
                composition_request: inputs.composition_request.clone(),
                composition_projection: composition.clone(),
            };
            validate_initialized_generation_material(&generation, &lock)?;
            Some(generation)
        }
        (None, None) => None,
        _ => {
            return Err(blocked(
                "persisted initialized-project generation material is incomplete",
            ));
        }
    };
    let composition_value = &composition.domain_pack_composition_projection;
    let lock_value = &lock.domain_pack_exact_lock;
    let operation = &preflight
        .domain_pack_lifecycle_preflight
        .request
        .domain_pack_lifecycle_request
        .operation;
    let admission_kind = classify_active_generation(composition_value, lock_value, operation)?;
    let effective_bundle = composition_value
        .composed_bundle
        .clone()
        .ok_or_else(|| blocked("active generation has no effective composed bundle"))?;
    if composition_value.core_bundle_digest != lock_value.payload.core.bundle_digest
        || composition_value.composition_digest != lock_value.payload.composition_digest
        || composition_value.ordered_packs.len() != lock_value.payload.packages.len()
    {
        return Err(blocked(
            "active composition differs from its exact durable lock",
        ));
    }
    for (composed, locked) in composition_value
        .ordered_packs
        .iter()
        .zip(&lock_value.payload.packages)
    {
        if composed.identity != locked.identity
            || composed.content_digest != locked.content_binding.canonical_sha256
            || composed.manifest_digest != locked.manifest_binding.canonical_sha256
            || composed.deterministic_order != locked.deterministic_order
        {
            return Err(blocked(
                "active package identity differs from its exact durable lock",
            ));
        }
    }

    Ok(ActiveDomainPackGenerationMaterial {
        pointer,
        lock,
        lifecycle_operation: operation.clone(),
        composition,
        effective_bundle,
        pointer_raw_digest: sha256_content_hash(pointer_raw),
        generation_manifest_raw_digest: sha256_content_hash(generation_manifest_raw),
        lock_raw_digest: sha256_content_hash(lock_raw),
        preflight_raw_digest: sha256_content_hash(preflight_raw),
        initialized_generation,
        rebase_inputs,
        admission_kind,
    })
}

fn load_initialized_project_rollback_source(
    store: &RetainedDomainPackLifecycleStore,
    project_snapshot: &RetainedProjectTree,
    target_receipt_digest: &str,
    target_lock_digest: &str,
) -> Result<DomainPackInitializedProjectRollbackSource, DomainPackLifecycleStoreError> {
    let state = load_current_state_under_lock(store, project_snapshot)?
        .0
        .projection;
    let receipt = load_committed_receipt(store, &state, target_receipt_digest, target_lock_digest)?;
    let receipt_value = &receipt.domain_pack_lifecycle_receipt;
    let record = state
        .ledger_records
        .iter()
        .find(|record| {
            record.record_digest == receipt_value.new_ledger_head_digest
                && record.active_lock_digest == target_lock_digest
                && record.to_generation == receipt_value.to_state.generation
        })
        .ok_or_else(|| blocked("rollback receipt has no exact immutable generation"))?;
    let inventory = load_raw_lifecycle_inventory(store, project_snapshot, &state)?;
    let generation_root = generation_root(record.to_generation, &record.record_digest)?;
    let lock_path = generation_root.join("lock.yaml");
    let lock_raw = raw_inventory_required(&inventory, &lock_path)?;
    let target_lock: DomainPackExactLockDocument = parse_yaml(&lock_path, lock_raw)?;
    if target_lock.domain_pack_exact_lock.lock_digest != target_lock_digest {
        return Err(blocked(
            "rollback target lock differs from immutable lifecycle history",
        ));
    }
    let catalog_path = generation_root.join("catalog.yaml");
    let catalog_raw = raw_inventory_required(&inventory, &catalog_path)?;
    let catalog: DomainPackAcquisitionCatalogDocument = parse_yaml(&catalog_path, catalog_raw)?;
    let resolution_path = generation_root.join("resolution-request.yaml");
    let resolution_raw = raw_inventory_required(&inventory, &resolution_path)?;
    let resolution_request: DomainPackResolutionRequestDocument =
        parse_yaml(&resolution_path, resolution_raw)?;
    let composition_request_path = generation_root.join("composition-request.yaml");
    let composition_request_raw = raw_inventory_required(&inventory, &composition_request_path)?;
    let composition_request: DomainPackCompositionRequestDocument =
        parse_yaml(&composition_request_path, composition_request_raw)?;
    let preflight_path = generation_root.join("preflight.yaml");
    let preflight_raw = raw_inventory_required(&inventory, &preflight_path)?;
    let preflight: DomainPackLifecyclePreflightDocument =
        parse_yaml(&preflight_path, preflight_raw)?;
    let generation = DomainPackInitializedProjectGenerationMaterial {
        requirements: resolution_request
            .domain_pack_resolution_request
            .requirements
            .clone(),
        catalog,
        resolution_request,
        resolution_projection: preflight.domain_pack_lifecycle_preflight.resolution.clone(),
        composition_request,
        composition_projection: preflight
            .domain_pack_lifecycle_preflight
            .composition
            .clone(),
    };
    validate_initialized_generation_material(&generation, &target_lock)?;
    Ok(DomainPackInitializedProjectRollbackSource {
        target_lock,
        target_generation: generation,
    })
}

fn validate_initialized_generation_material(
    generation: &DomainPackInitializedProjectGenerationMaterial,
    lock: &DomainPackExactLockDocument,
) -> Result<(), DomainPackLifecycleStoreError> {
    let exact_lock = &lock.domain_pack_exact_lock;
    let resolution = &generation
        .resolution_projection
        .domain_pack_resolution_projection;
    let composition = &generation
        .composition_projection
        .domain_pack_composition_projection;
    let catalog_registry_digest =
        domain_pack_registry_snapshot_digest(&generation.catalog.registry).map_err(|error| {
            blocked(&format!(
                "catalog registry digest verification failed: {error}"
            ))
        })?;
    if generation.catalog.schema_version != DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION
        || generation.catalog.forge_core_version
            != generation
                .resolution_request
                .domain_pack_resolution_request
                .forge_core_version
        || generation.catalog.core != exact_lock.payload.core
        || catalog_registry_digest
            != generation
                .catalog
                .registry
                .domain_pack_supply_chain_registry
                .snapshot_digest
        || catalog_registry_digest != exact_lock.payload.registry_snapshot_digest
        || generation
            .catalog
            .registry
            .domain_pack_supply_chain_registry
            .snapshot_digest
            != exact_lock.payload.registry_snapshot_digest
        || generation.catalog.candidates
            != generation
                .resolution_request
                .domain_pack_resolution_request
                .candidates
        || generation
            .resolution_request
            .domain_pack_resolution_request
            .project_id
            != exact_lock.payload.project_id
        || generation
            .resolution_request
            .domain_pack_resolution_request
            .core
            != exact_lock.payload.core
        || generation
            .resolution_request
            .domain_pack_resolution_request
            .requirements
            != generation.requirements
        || generation
            .resolution_request
            .domain_pack_resolution_request
            .registry_snapshot_digest
            != exact_lock.payload.registry_snapshot_digest
        || generation
            .composition_request
            .domain_pack_composition_request
            .forge_core_version
            != generation
                .resolution_request
                .domain_pack_resolution_request
                .forge_core_version
        || generation
            .composition_request
            .domain_pack_composition_request
            .core
            != exact_lock.payload.core
        || generation
            .composition_request
            .domain_pack_composition_request
            .requirements
            != generation.requirements.domain_pack_project_requirements
        || resolution.resolution_digest != exact_lock.payload.resolution_digest
        || composition.composition_digest != exact_lock.payload.composition_digest
        || canonical_digest(&generation.requirements)? != exact_lock.payload.requirements_digest
    {
        return Err(blocked(
            "persisted initialized-project generation material does not bind its exact lock",
        ));
    }
    Ok(())
}

fn classify_active_generation(
    composition: &forge_core_contracts::DomainPackCompositionProjection,
    lock: &forge_core_contracts::DomainPackExactLock,
    operation: &DomainPackLifecycleOperation,
) -> Result<ActiveDomainPackGenerationAdmissionKind, DomainPackLifecycleStoreError> {
    if composition.status == DomainPackCompositionStatus::Composable
        && composition.gaps.is_empty()
        && composition.issues.is_empty()
    {
        return Ok(ActiveDomainPackGenerationAdmissionKind::Ready);
    }
    let legitimate_empty_operation = matches!(
        operation,
        DomainPackLifecycleOperation::Remove { .. }
            | DomainPackLifecycleOperation::Rollback { .. }
            | DomainPackLifecycleOperation::RebaseCore { .. }
    );
    let legitimate_degraded_empty = legitimate_empty_operation
        && composition.status == DomainPackCompositionStatus::Blocked
        && composition.issues.is_empty()
        && !composition.gaps.is_empty()
        && lock.payload.packages.is_empty()
        && composition.ordered_packs.is_empty()
        && composition.gaps == lock.payload.unresolved_composition_gaps
        && composition.composed_bundle.is_some();
    if legitimate_degraded_empty {
        return Ok(ActiveDomainPackGenerationAdmissionKind::DegradedEmpty);
    }
    Err(blocked(
        "active generation is neither ready nor a governed degraded empty-package remove/rollback",
    ))
}

fn active_generation_material_digest(
    material: &ActiveDomainPackGenerationMaterial,
) -> Result<String, DomainPackLifecycleStoreError> {
    canonical_digest(material)
}

fn load_ledger_chain(
    store: &RetainedDomainPackLifecycleStore,
    head: &str,
    expected_generation: u64,
) -> Result<Vec<DomainPackLifecycleLedgerRecord>, DomainPackLifecycleStoreError> {
    let mut reverse = Vec::new();
    let mut cursor = Some(head.to_owned());
    while let Some(digest) = cursor.take() {
        if reverse.len() >= DOMAIN_PACK_MAX_LEDGER_RECORDS {
            return Err(DomainPackLifecycleStoreError::ResourceLimit {
                resource: "ledger records",
                maximum: DOMAIN_PACK_MAX_LEDGER_RECORDS as u64,
            });
        }
        let token = digest_token(&digest, "ledger.record_digest")?;
        let path = Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT)
            .join("ledger")
            .join(format!("{token}.yaml"));
        let bytes = read_required_state_bytes(store, &path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?;
        let record: DomainPackLifecycleLedgerRecord = parse_yaml(&path, &bytes)?;
        if record.record_digest != digest || digest_record(&record)? != digest {
            return Err(invalid("ledger.record_digest", "record digest mismatch"));
        }
        cursor.clone_from(&record.previous_record_digest);
        reverse.push(record);
    }
    reverse.reverse();
    for (index, record) in reverse.iter().enumerate() {
        let sequence = u64::try_from(index).unwrap_or(u64::MAX);
        if record.sequence != sequence {
            return Err(invalid("ledger.sequence", "sequence is not contiguous"));
        }
        if index == 0 && record.previous_record_digest.is_some() {
            return Err(invalid(
                "ledger.previous_record_digest",
                "genesis has predecessor",
            ));
        }
        if index > 0
            && record.previous_record_digest.as_deref()
                != Some(reverse[index - 1].record_digest.as_str())
        {
            return Err(invalid("ledger.previous_record_digest", "chain mismatch"));
        }
    }
    if reverse.last().map(|record| record.sequence) != Some(expected_generation) {
        return Err(invalid("ledger.head", "generation and ledger head differ"));
    }
    Ok(reverse)
}

fn validate_preflight(
    document: &DomainPackLifecyclePreflightDocument,
) -> Result<(), DomainPackLifecycleStoreError> {
    validate_schema(&document.schema_version, "preflight.schema_version")?;
    let body = &document.domain_pack_lifecycle_preflight;
    if body.status != DomainPackLifecyclePreflightStatus::Ready || !body.issues.is_empty() {
        return Err(DomainPackLifecycleStoreError::PreflightBlocked {
            reason: "status must be ready with zero issues".to_owned(),
        });
    }
    validate_exact_lock(&body.proposed_lock)?;
    let expected = digest_preflight(document)?;
    if body.preflight_digest != expected {
        return Err(invalid("preflight.preflight_digest", "digest mismatch"));
    }
    let report = &body.compatibility_report.domain_pack_compatibility_report;
    let operation = &body.request.domain_pack_lifecycle_request.operation;
    let historical_empty_rollback =
        matches!(operation, DomainPackLifecycleOperation::Rollback { .. })
            && body
                .proposed_lock
                .domain_pack_exact_lock
                .payload
                .packages
                .is_empty();
    let allowed = match operation {
        DomainPackLifecycleOperation::Remove { .. } => matches!(
            report.status,
            DomainPackCompatibilityStatus::Compatible | DomainPackCompatibilityStatus::Degraded
        ),
        DomainPackLifecycleOperation::Rollback { .. } if historical_empty_rollback => true,
        DomainPackLifecycleOperation::RebaseCore { .. } => matches!(
            report.status,
            DomainPackCompatibilityStatus::Compatible | DomainPackCompatibilityStatus::Degraded
        ),
        _ => report.status == DomainPackCompatibilityStatus::Compatible,
    };
    let core_binding_allowed =
        if matches!(operation, DomainPackLifecycleOperation::RebaseCore { .. }) {
            !report.universal_core_unchanged
        } else {
            report.universal_core_unchanged
        };
    if !allowed || !core_binding_allowed {
        return Err(DomainPackLifecycleStoreError::PreflightBlocked {
            reason: "compatibility or sealed-core invariant blocks operation".to_owned(),
        });
    }
    Ok(())
}

fn validate_exact_lock(
    document: &DomainPackExactLockDocument,
) -> Result<(), DomainPackLifecycleStoreError> {
    validate_schema(&document.schema_version, "exact_lock.schema_version")?;
    let expected = canonical_digest(&document.domain_pack_exact_lock.payload)?;
    if document.domain_pack_exact_lock.lock_digest != expected {
        return Err(invalid("exact_lock.lock_digest", "payload digest mismatch"));
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // Lifecycle intents are audited together to keep cross-operation prohibitions explicit.
fn validate_operation_intent(
    operation: &DomainPackLifecycleOperation,
    from_lock: Option<&DomainPackExactLockDocument>,
    to_lock: &DomainPackExactLockDocument,
    resolution: &forge_core_contracts::DomainPackResolutionRequest,
    rollback_target: Option<&DomainPackLifecycleReceiptDocument>,
) -> Result<(), DomainPackLifecycleStoreError> {
    let from_packages = from_lock
        .map(|lock| lock.domain_pack_exact_lock.payload.packages.as_slice())
        .unwrap_or_default();
    let to_packages = &to_lock.domain_pack_exact_lock.payload.packages;
    match operation {
        DomainPackLifecycleOperation::Install { root } => {
            if find_locked_package(from_packages, root).is_some()
                || find_locked_package(to_packages, root).is_none()
                || !resolution.roots.iter().any(|candidate| {
                    candidate.pack == *root
                        && candidate.reason
                            == forge_core_contracts::DomainPackResolutionRootReason::InstallIntent
                })
            {
                return Err(blocked(
                    "install must add an absent pack and select the exact install-intent root",
                ));
            }
        }
        DomainPackLifecycleOperation::Upgrade {
            pack,
            expected_from,
            target_requirement,
            required_content_digest,
        } => {
            let Some(previous) = find_locked_package(from_packages, pack) else {
                return Err(blocked("upgrade target is absent from the active lock"));
            };
            let Some(target) = find_locked_package(to_packages, pack) else {
                return Err(blocked("upgrade target is absent from the proposed lock"));
            };
            let root_matches = resolution.roots.iter().any(|root| {
                root.pack == *pack
                    && root.version_requirement == *target_requirement
                    && root.required_content_digest == *required_content_digest
                    && root.reason
                        == forge_core_contracts::DomainPackResolutionRootReason::UpgradeIntent
            });
            if previous.identity.version != *expected_from
                || !root_matches
                || previous.package_digest == target.package_digest
                || required_content_digest
                    .as_ref()
                    .is_some_and(|digest| target.content_binding.canonical_sha256 != *digest)
            {
                return Err(blocked(
                    "upgrade intent does not match the active version, resolution root, or target content",
                ));
            }
        }
        DomainPackLifecycleOperation::Remove { pack } => {
            if find_locked_package(from_packages, pack).is_none()
                || find_locked_package(to_packages, pack).is_some()
                || resolution.roots.iter().any(|root| root.pack == *pack)
            {
                return Err(blocked(
                    "remove must delete an active pack from both roots and proposed lock",
                ));
            }
        }
        DomainPackLifecycleOperation::Rollback {
            target_receipt_digest,
            target_lock_digest,
        } => {
            let Some(target) = rollback_target else {
                return Err(blocked("rollback lacks a reachable committed receipt"));
            };
            if target.domain_pack_lifecycle_receipt.receipt_digest != *target_receipt_digest
                || target
                    .domain_pack_lifecycle_receipt
                    .to_state
                    .active_lock_digest
                    != *target_lock_digest
                || to_lock.domain_pack_exact_lock.lock_digest != *target_lock_digest
            {
                return Err(blocked(
                    "rollback target receipt and proposed lock are not the exact historical state",
                ));
            }
        }
        DomainPackLifecycleOperation::RebaseCore {
            target_release_id,
            expected_from_core_digest,
            target_core_digest,
        } => {
            let Some(previous_lock) = from_lock else {
                return Err(blocked(
                    "core rebase requires an initialized lifecycle lock",
                ));
            };
            let previous = &previous_lock.domain_pack_exact_lock.payload;
            let target = &to_lock.domain_pack_exact_lock.payload;
            if target_release_id.0.trim().is_empty()
                || previous.core.bundle_digest != *expected_from_core_digest
                || target.core.bundle_digest != *target_core_digest
                || previous.core == target.core
                || previous.roots != target.roots
                || previous.requirements_digest != target.requirements_digest
                || previous.packages != target.packages
                || resolution.current_lock.as_ref() != Some(previous_lock)
                || resolution.roots != previous.roots
            {
                return Err(blocked(
                    "core rebase must bind the initialized source lock, exact target Core, and unchanged package roots/material",
                ));
            }
        }
    }
    Ok(())
}

fn validate_reviewed_operation_transition(
    operation: &DomainPackLifecycleOperation,
    from_lock: Option<&DomainPackExactLockDocument>,
    to_packages: &[DomainPackLockedPackage],
    reviewed_join: &forge_core_decisions::DomainPackReviewedResolutionProjection,
) -> Result<(), DomainPackLifecycleStoreError> {
    if matches!(
        operation,
        DomainPackLifecycleOperation::Rollback { .. }
            | DomainPackLifecycleOperation::RebaseCore { .. }
    ) && to_packages.is_empty()
        && reviewed_join.joins.is_empty()
    {
        // The exact historical remove-last lock contains no package that could
        // require semantic review. Operation-intent validation has already
        // bound this lock to a reachable immutable receipt, so the empty set is
        // vacuously eligible without creating an install/upgrade shortcut.
        return Ok(());
    }
    if !matches!(operation, DomainPackLifecycleOperation::Remove { .. }) {
        if !reviewed_join.all_selected_eligible
            || to_packages.is_empty()
            || reviewed_join
                .joins
                .iter()
                .any(|join| join.status != DomainPackReviewedResolutionJoinStatus::EligibleReviewed)
        {
            return Err(blocked(
                "install, upgrade, rollback, and core rebase require every selected package to be eligible-reviewed",
            ));
        }
        return Ok(());
    }

    let Some(from_lock) = from_lock else {
        return Err(blocked("remove requires an initialized exact lock"));
    };
    let previous = &from_lock.domain_pack_exact_lock.payload.packages;
    for target in to_packages {
        let Some(prior) = previous.iter().find(|prior| {
            prior.identity == target.identity
                && prior.package_digest == target.package_digest
                && prior.registry_record_digest == target.registry_record_digest
        }) else {
            return Err(blocked(
                "remove cannot introduce a package absent from the previous lock",
            ));
        };
        if prior != target {
            return Err(blocked(
                "remove must preserve every remaining package byte-exactly",
            ));
        }
        let Some(join) = reviewed_join.joins.iter().find(|join| {
            join.publisher == target.identity.publisher.0
                && join.name == target.identity.name.0
                && join.version == target.identity.version
                && join.package_digest == target.package_digest
                && join.registry_record_digest == target.registry_record_digest
        }) else {
            return Err(blocked(
                "remaining remove package lacks an exact reviewed join",
            ));
        };
        if !matches!(
            join.status,
            DomainPackReviewedResolutionJoinStatus::EligibleReviewed
                | DomainPackReviewedResolutionJoinStatus::IneligibleDeprecated
                | DomainPackReviewedResolutionJoinStatus::IneligibleRevoked
                | DomainPackReviewedResolutionJoinStatus::IneligibleSuperseded
        ) {
            return Err(blocked(
                "remove cannot retain a package without reviewed history",
            ));
        }
    }
    Ok(())
}

fn find_locked_package<'a>(
    packages: &'a [DomainPackLockedPackage],
    pack: &forge_core_contracts::DomainPackCoordinate,
) -> Option<&'a DomainPackLockedPackage> {
    packages.iter().find(|candidate| {
        candidate.identity.publisher == pack.publisher && candidate.identity.name == pack.name
    })
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ImmutableArtifactByteSemantics {
    LifecycleYaml,
    Remote(DomainPackRemoteArtifactMediaType),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImmutableArtifactByteValidationError {
    Bounds,
    Length,
    RawDigest,
    Utf8,
    Syntax,
    CanonicalDigest,
    Canonicalization,
}

/// Verify immutable artifact bytes before a caller may take ownership.
///
/// This is deliberately the sole byte-validation implementation used by both the
/// lifecycle and remote-acquisition paths. The latter supplies an exact signed
/// length and media type; the lifecycle path retains its established YAML-only
/// semantics.
pub(crate) fn verify_immutable_artifact_bytes(
    binding: &DomainPackArtifactBinding,
    raw_bytes: &[u8],
    expected_byte_length: Option<u64>,
    semantics: ImmutableArtifactByteSemantics,
) -> Result<(), ImmutableArtifactByteValidationError> {
    let byte_length = u64::try_from(raw_bytes.len()).unwrap_or(u64::MAX);
    if byte_length > DOMAIN_PACK_MAX_DOCUMENT_BYTES {
        return Err(ImmutableArtifactByteValidationError::Bounds);
    }
    if expected_byte_length.is_some_and(|expected| expected != byte_length) {
        return Err(ImmutableArtifactByteValidationError::Length);
    }
    if sha256_content_hash(raw_bytes) != binding.raw_sha256 {
        return Err(ImmutableArtifactByteValidationError::RawDigest);
    }

    let canonical = match semantics {
        ImmutableArtifactByteSemantics::LifecycleYaml
        | ImmutableArtifactByteSemantics::Remote(
            DomainPackRemoteArtifactMediaType::ApplicationYaml,
        ) => {
            let text = std::str::from_utf8(raw_bytes)
                .map_err(|_| ImmutableArtifactByteValidationError::Utf8)?;
            let semantic: serde_json::Value = yaml_serde::from_str(text)
                .map_err(|_| ImmutableArtifactByteValidationError::Syntax)?;
            canonical_digest(&semantic)
                .map_err(|_| ImmutableArtifactByteValidationError::Canonicalization)?
        }
        ImmutableArtifactByteSemantics::Remote(
            DomainPackRemoteArtifactMediaType::ApplicationJson,
        ) => {
            let semantic: serde_json::Value = serde_json::from_slice(raw_bytes)
                .map_err(|_| ImmutableArtifactByteValidationError::Syntax)?;
            canonical_digest(&semantic)
                .map_err(|_| ImmutableArtifactByteValidationError::Canonicalization)?
        }
        ImmutableArtifactByteSemantics::Remote(DomainPackRemoteArtifactMediaType::TextPlain) => {
            let text = std::str::from_utf8(raw_bytes)
                .map_err(|_| ImmutableArtifactByteValidationError::Utf8)?;
            canonical_digest(&text)
                .map_err(|_| ImmutableArtifactByteValidationError::Canonicalization)?
        }
        ImmutableArtifactByteSemantics::Remote(
            DomainPackRemoteArtifactMediaType::ApplicationOctetStream,
        ) => sha256_content_hash(raw_bytes),
    };
    if canonical != binding.canonical_sha256 {
        return Err(ImmutableArtifactByteValidationError::CanonicalDigest);
    }
    Ok(())
}

fn verify_immutable_artifacts(
    artifacts: &[DomainPackImmutableArtifact<'_>],
    expected: &[DomainPackArtifactBinding],
) -> Result<Vec<OwnedDomainPackImmutableArtifact>, DomainPackLifecycleStoreError> {
    let expected = normalized_artifact_bindings(expected);
    let mut supplied = artifacts
        .iter()
        .map(|artifact| artifact.binding.clone())
        .collect::<Vec<_>>();
    supplied = normalized_artifact_bindings(&supplied);
    if artifacts.len() != expected.len() || supplied != expected {
        return Err(blocked(
            "immutable artifact bytes do not exactly cover staged bindings",
        ));
    }
    let mut owned = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        match verify_immutable_artifact_bytes(
            artifact.binding,
            artifact.raw_bytes,
            None,
            ImmutableArtifactByteSemantics::LifecycleYaml,
        ) {
            Ok(()) => {}
            Err(
                ImmutableArtifactByteValidationError::Bounds
                | ImmutableArtifactByteValidationError::RawDigest,
            ) => {
                return Err(blocked(
                    "immutable artifact bytes exceed bounds or differ from raw binding",
                ));
            }
            Err(ImmutableArtifactByteValidationError::Utf8) => {
                return Err(blocked("immutable artifact is not UTF-8 YAML"));
            }
            Err(
                ImmutableArtifactByteValidationError::Syntax
                | ImmutableArtifactByteValidationError::Length,
            ) => {
                return Err(blocked("immutable artifact is not valid bounded YAML"));
            }
            Err(ImmutableArtifactByteValidationError::CanonicalDigest) => {
                return Err(blocked(
                    "immutable artifact canonical semantics differ from staged binding",
                ));
            }
            Err(ImmutableArtifactByteValidationError::Canonicalization) => {
                return Err(blocked("immutable artifact canonicalization failed"));
            }
        }
        owned.push(OwnedDomainPackImmutableArtifact {
            binding: artifact.binding.clone(),
            raw_bytes: artifact.raw_bytes.to_vec(),
        });
    }
    owned.sort_by(|left, right| {
        left.binding
            .artifact_ref
            .0
            .cmp(&right.binding.artifact_ref.0)
    });
    Ok(owned)
}

fn load_committed_receipt(
    store: &RetainedDomainPackLifecycleStore,
    state: &DomainPackLifecycleStateProjection,
    receipt_digest: &str,
    target_lock_digest: &str,
) -> Result<DomainPackLifecycleReceiptDocument, DomainPackLifecycleStoreError> {
    let token = digest_token(receipt_digest, "rollback.target_receipt_digest")?;
    let path = Path::new(DOMAIN_PACK_STATE_RELATIVE_ROOT)
        .join("receipts")
        .join(format!("{token}.yaml"));
    let receipt: DomainPackLifecycleReceiptDocument = parse_yaml(
        &path,
        &read_required_state_bytes(store, &path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?,
    )?;
    let value = &receipt.domain_pack_lifecycle_receipt;
    let historical_record = state.ledger_records.iter().find(|record| {
        record.record_digest == value.new_ledger_head_digest
            && record.active_lock_digest == target_lock_digest
            && record.to_generation == value.to_state.generation
    });
    if value.receipt_digest != receipt_digest
        || digest_receipt(value)? != receipt_digest
        || digest_pointer(&value.to_state)? != value.to_state.pointer_digest
        || value.to_state.lifecycle_head_digest != value.new_ledger_head_digest
        || value.to_state.active_lock_digest != target_lock_digest
        || historical_record.is_none()
        || state.active_pointer.as_ref().is_some_and(|active| {
            active.domain_pack_active_pointer.pointer_digest == value.to_state.pointer_digest
        })
    {
        return Err(blocked(
            "rollback receipt is invalid or unreachable from committed history",
        ));
    }
    let historical_record = historical_record.expect("checked above");
    if historical_record.operation != value.operation
        || historical_record.preflight_digest != value.preflight_digest
        || historical_record.request_digest != value.request_digest
        || historical_record.compatibility_report_digest != value.compatibility_report_digest
    {
        return Err(blocked(
            "rollback receipt fields differ from their reachable ledger record",
        ));
    }
    let generation = generation_root(
        historical_record.to_generation,
        &historical_record.record_digest,
    )?;
    let canonical_path = generation.join("receipt.yaml");
    let canonical: DomainPackLifecycleReceiptDocument = parse_yaml(
        &canonical_path,
        &read_required_state_bytes(store, &canonical_path, DOMAIN_PACK_MAX_DOCUMENT_BYTES)?,
    )?;
    if canonical != receipt {
        return Err(blocked(
            "rollback receipt differs from the exact immutable historical generation",
        ));
    }
    Ok(receipt)
}

fn verify_expected_state(
    expected: &DomainPackExpectedLifecycleState,
    actual: &DomainPackLifecycleStateProjection,
) -> Result<(), DomainPackLifecycleStoreError> {
    let matches = match (expected, actual.active_pointer.as_ref()) {
        (DomainPackExpectedLifecycleState::Uninitialized { .. }, None) => true,
        (
            DomainPackExpectedLifecycleState::Initialized {
                generation,
                active_lock_digest,
                lifecycle_head_digest,
                ..
            },
            Some(pointer),
        ) => {
            let value = &pointer.domain_pack_active_pointer;
            value.generation == *generation
                && value.active_lock_digest == *active_lock_digest
                && value.lifecycle_head_digest == *lifecycle_head_digest
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(stale(&format!("{expected:?}"), actual))
    }
}

fn staged_digests(body: &forge_core_contracts::DomainPackLifecyclePreflight) -> Vec<String> {
    let mut values = body
        .staged_artifacts
        .iter()
        .flat_map(|binding| [binding.raw_sha256.clone(), binding.canonical_sha256.clone()])
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn normalized_artifact_bindings(
    bindings: &[DomainPackArtifactBinding],
) -> Vec<DomainPackArtifactBinding> {
    let mut normalized = bindings.to_vec();
    normalized.sort_by(|left, right| {
        left.artifact_ref
            .0
            .cmp(&right.artifact_ref.0)
            .then(left.raw_sha256.cmp(&right.raw_sha256))
            .then(left.canonical_sha256.cmp(&right.canonical_sha256))
    });
    normalized.dedup();
    normalized
}

fn expected_staged_artifact_bindings(
    packages: &[DomainPackLockedPackage],
) -> Vec<DomainPackArtifactBinding> {
    let bindings = packages
        .iter()
        .flat_map(|package| {
            let content = DomainPackArtifactBinding {
                artifact_ref: package.content_binding.content_ref.clone(),
                raw_sha256: package.content_binding.raw_sha256.clone(),
                canonical_sha256: package.content_binding.canonical_sha256.clone(),
            };
            std::iter::once(package.manifest_binding.clone())
                .chain(std::iter::once(content))
                .chain(std::iter::once(package.license_binding.clone()))
                .chain(package.fixture_bindings.iter().cloned())
        })
        .collect::<Vec<_>>();
    normalized_artifact_bindings(&bindings)
}

/// Derive the exact runtime-capability demands encoded by one resolved raw
/// package candidate and the project requirements. This is deterministic,
/// read-only evidence and grants no runtime or lifecycle authority.
///
/// # Errors
///
/// Fails closed when the composition request lacks the exact selected package
/// or a semantic contribution references an undeclared capability.
pub fn derive_domain_pack_capability_demands(
    package: &forge_core_contracts::DomainPackResolvedPackage,
    composition: &forge_core_contracts::DomainPackCompositionRequest,
) -> Result<Vec<DomainPackCapabilityDemand>, DomainPackLifecycleStoreError> {
    let candidate = composition
        .candidates
        .iter()
        .find(|candidate| {
            let pack = &candidate.content.domain_pack_content.pack;
            pack.publisher == package.identity.publisher
                && pack.name == package.identity.name
                && pack.version == package.identity.version
        })
        .ok_or_else(|| blocked("resolved package has no exact raw composition candidate"))?;
    let content = &candidate.content.domain_pack_content;
    let capability_kinds = content
        .provided_capabilities
        .iter()
        .map(|capability| (capability.id.0.as_str(), capability.kind))
        .collect::<BTreeMap<_, _>>();
    let mut demands = Vec::new();
    let mut add = |subject_ref: &StableId,
                   capability_ref: &StableId|
     -> Result<(), DomainPackLifecycleStoreError> {
        let Some(kind) = capability_kinds.get(capability_ref.0.as_str()).copied() else {
            return Err(blocked(
                "raw package semantics demand an undeclared capability",
            ));
        };
        demands.push(DomainPackCapabilityDemand {
            subject_ref: subject_ref.clone(),
            capability_ref: capability_ref.clone(),
            kind,
        });
        Ok(())
    };
    // Workflow capability requirements are abstract governance probes and do
    // not carry a Domain Pack `capability_ref`; treating their own ids as
    // runtime bindings would fabricate a cross-vocabulary mapping. P6b derives
    // executable demands only from fields with explicit capability refs.
    for lifecycle in &content.lifecycle_models {
        for transition in &lifecycle.transitions {
            for capability_ref in &transition.required_capability_refs {
                add(&transition.id, capability_ref)?;
            }
        }
    }
    for adapter in &content.adapters {
        for capability_ref in &adapter.required_capability_refs {
            add(&adapter.id, capability_ref)?;
        }
    }
    for requirement in &composition.requirements.required_domains {
        for capability_ref in &requirement.required_capability_refs {
            if capability_kinds.contains_key(capability_ref.0.as_str()) {
                add(&requirement.id, capability_ref)?;
            }
        }
    }
    Ok(normalized_capability_demands(&demands))
}

fn normalized_capability_demands(
    demands: &[DomainPackCapabilityDemand],
) -> Vec<DomainPackCapabilityDemand> {
    let mut normalized = demands.to_vec();
    normalized.sort_by(|left, right| {
        left.subject_ref
            .0
            .cmp(&right.subject_ref.0)
            .then(left.capability_ref.0.cmp(&right.capability_ref.0))
            .then(format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
    });
    normalized.dedup();
    normalized
}

fn digest_preflight(
    document: &DomainPackLifecyclePreflightDocument,
) -> Result<String, DomainPackLifecycleStoreError> {
    let mut subject = document.clone();
    subject
        .domain_pack_lifecycle_preflight
        .preflight_digest
        .clear();
    canonical_digest(&subject)
}

fn digest_pointer(
    pointer: &DomainPackActivePointer,
) -> Result<String, DomainPackLifecycleStoreError> {
    let mut subject = pointer.clone();
    subject.pointer_digest.clear();
    canonical_digest(&subject)
}

fn digest_record(
    record: &DomainPackLifecycleLedgerRecord,
) -> Result<String, DomainPackLifecycleStoreError> {
    let mut subject = record.clone();
    subject.record_digest.clear();
    canonical_digest(&subject)
}

fn digest_receipt(
    receipt: &DomainPackLifecycleReceipt,
) -> Result<String, DomainPackLifecycleStoreError> {
    let mut subject = receipt.clone();
    subject.receipt_digest.clear();
    canonical_digest(&subject)
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, DomainPackLifecycleStoreError> {
    let bytes = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| invalid("canonicalization", &error.to_string()))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn yaml_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, DomainPackLifecycleStoreError> {
    yaml_serde::to_string(value)
        .map(String::into_bytes)
        .map_err(|error| invalid("serialization", &error.to_string()))
}

fn parse_yaml<T: serde::de::DeserializeOwned>(
    path: &Path,
    bytes: &[u8],
) -> Result<T, DomainPackLifecycleStoreError> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        DomainPackLifecycleStoreError::InvalidDocument {
            path: path.to_path_buf(),
            reason: error.to_string(),
        }
    })?;
    yaml_serde::from_str(text).map_err(|error| DomainPackLifecycleStoreError::InvalidDocument {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })
}

fn write_immutable_under_root(
    store: &RetainedDomainPackLifecycleStore,
    path: &Path,
    content: &[u8],
) -> Result<(), DomainPackLifecycleStoreError> {
    store
        .write_immutable(path, content, DOMAIN_PACK_MAX_DOCUMENT_BYTES)
        .map_err(Into::into)
}

fn read_optional_state_bytes(
    store: &RetainedDomainPackLifecycleStore,
    path: &Path,
    maximum: u64,
) -> Result<Option<Vec<u8>>, DomainPackLifecycleStoreError> {
    store.read_optional(path, maximum).map_err(Into::into)
}

fn read_required_state_bytes(
    store: &RetainedDomainPackLifecycleStore,
    path: &Path,
    maximum: u64,
) -> Result<Vec<u8>, DomainPackLifecycleStoreError> {
    store.read_required(path, maximum).map_err(Into::into)
}

fn validate_supply_chain_freshness(
    verified_at_unix: u64,
    expires_at_unix: u64,
) -> Result<(), DomainPackLifecycleStoreError> {
    let now_unix = trusted_now_unix()?;
    if verified_at_unix > now_unix.saturating_add(DOMAIN_PACK_MAX_CLOCK_FUTURE_SKEW_SECONDS) {
        return Err(blocked("supply-chain verification time is in the future"));
    }
    if now_unix.saturating_sub(verified_at_unix)
        > DOMAIN_PACK_MAX_SUPPLY_CHAIN_VERIFICATION_AGE_SECONDS
    {
        return Err(blocked("supply-chain verification is stale"));
    }
    if now_unix > expires_at_unix {
        return Err(blocked("verified supply-chain snapshot has expired"));
    }
    Ok(())
}

pub(crate) fn trusted_now_unix() -> Result<u64, DomainPackLifecycleStoreError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("system_clock", "time is before Unix epoch"))
        .map(|duration| duration.as_secs())
}

fn canonical_state_root(path: &Path) -> Result<PathBuf, DomainPackLifecycleStoreError> {
    let canonical = fs::canonicalize(path).map_err(|error| io_error(path, error))?;
    if !canonical.is_dir()
        || canonical
            .file_name()
            .is_none_or(|name| name != std::ffi::OsStr::new(".forge-method"))
    {
        return Err(invalid(
            "state_root",
            "must be an existing canonical .forge-method directory",
        ));
    }
    Ok(canonical)
}

fn digest_token<'a>(
    digest: &'a str,
    field: &'static str,
) -> Result<&'a str, DomainPackLifecycleStoreError> {
    let Some(token) = digest.strip_prefix("sha256:") else {
        return Err(DomainPackLifecycleStoreError::InvalidDigest {
            field,
            value: digest.to_owned(),
        });
    };
    if token.len() != 64
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(DomainPackLifecycleStoreError::InvalidDigest {
            field,
            value: digest.to_owned(),
        });
    }
    Ok(token)
}

fn validate_schema(value: &str, field: &'static str) -> Result<(), DomainPackLifecycleStoreError> {
    validate_schema_version(value, DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION, field)
}

fn validate_schema_version(
    value: &str,
    expected: &str,
    field: &'static str,
) -> Result<(), DomainPackLifecycleStoreError> {
    if value == expected {
        Ok(())
    } else {
        Err(DomainPackLifecycleStoreError::InvalidArgument {
            field,
            reason: format!("expected {expected}, found {value}"),
        })
    }
}

fn stale(
    expected: &str,
    actual: &DomainPackLifecycleStateProjection,
) -> DomainPackLifecycleStoreError {
    DomainPackLifecycleStoreError::StaleExpectedState {
        expected: expected.to_owned(),
        actual: format!("{:?}", actual.active_pointer),
    }
}

fn stale_project_snapshot(expected: &str, actual: &str) -> DomainPackLifecycleStoreError {
    DomainPackLifecycleStoreError::StaleExpectedState {
        expected: expected.to_owned(),
        actual: actual.to_owned(),
    }
}

fn invalid(field: &'static str, reason: &str) -> DomainPackLifecycleStoreError {
    DomainPackLifecycleStoreError::InvalidArgument {
        field,
        reason: reason.to_owned(),
    }
}

fn compatibility_drift_fields(
    expected: &DomainPackCompatibilityReport,
    observed: &DomainPackCompatibilityReport,
) -> String {
    let expected = serde_json::to_value(expected).unwrap_or(serde_json::Value::Null);
    let observed = serde_json::to_value(observed).unwrap_or(serde_json::Value::Null);
    match (expected.as_object(), observed.as_object()) {
        (Some(expected), Some(observed)) => {
            let fields = expected
                .iter()
                .filter_map(|(field, value)| {
                    (observed.get(field) != Some(value)).then_some(field.as_str())
                })
                .collect::<Vec<_>>();
            if fields.is_empty() {
                "unknown".to_owned()
            } else {
                fields.join(",")
            }
        }
        _ => "document-shape".to_owned(),
    }
}

fn blocked(reason: &str) -> DomainPackLifecycleStoreError {
    DomainPackLifecycleStoreError::PreflightBlocked {
        reason: reason.to_owned(),
    }
}

#[allow(clippy::needless_pass_by_value)] // Call sites transfer the terminal I/O error into text.
fn io_error(path: &Path, error: io::Error) -> DomainPackLifecycleStoreError {
    DomainPackLifecycleStoreError::Io {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

#[allow(dead_code)]
fn is_safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}
