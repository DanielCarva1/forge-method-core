//! Opaque join between the admitted universal workflow release and one
//! Domain Pack generation admitted by its independent lifecycle TCB.
//!
//! The join never mutates or extends the five-release, 42-policy core
//! registry. It derives a project-local effective runtime identity whose
//! provenance is carried as a separate workflow-ledger epoch.

use super::AdmittedWorkflowGovernanceRelease;
use forge_core_contracts::{
    DomainPackCompositionGap, DomainPackGenerationTransitionedEvent,
    WorkflowDomainPackGenerationIdentity, WorkflowEffectiveBundleIdentity,
    WorkflowGovernanceBundle, WorkflowGovernanceBundleDocument, WorkflowRuntimeBundleIdentity,
    WORKFLOW_GOVERNANCE_SCHEMA_VERSION,
};
use forge_core_decisions::{workflow_policy_set_digest, workflow_runtime_bundle_digest};
use forge_core_domain_pack_tcb::{
    AdmittedActiveDomainPackGenerationView, AdmittedCoreOnlyDomainPackLifecycleView,
};
use forge_core_store::sha256_content_hash;
use forge_core_workflow_governance_tcb::domain_pack_receipt_carryover;
use serde::Serialize;
use std::fmt;

/// Opaque kernel admission of one effective core-plus-packs policy bundle.
///
/// It has no public constructor, `Clone`, or serde implementation. The only
/// constructor consumes the already opaque P5 core admission and a borrowed
/// view whose lifetime is protected by the Domain Pack lifecycle OS lock.
/// The returned capability cannot be widened to outlive that lock owner:
///
/// ```compile_fail
/// use forge_core_domain_pack_tcb::AdmittedActiveDomainPackGeneration;
/// use forge_core_kernel::workflow_governance::{
///     admit_effective_workflow_governance_bundle,
///     AdmittedEffectiveWorkflowGovernanceBundle,
///     AdmittedWorkflowGovernanceRelease,
///     WorkflowDomainPackContextView,
/// };
/// fn escape<'a>(
///     core: &AdmittedWorkflowGovernanceRelease,
///     active: &'a AdmittedActiveDomainPackGeneration,
/// ) -> AdmittedEffectiveWorkflowGovernanceBundle<'static> {
///     let view = active.verified_view().unwrap();
///     admit_effective_workflow_governance_bundle(
///         core,
///         WorkflowDomainPackContextView::Active(view),
///     ).unwrap()
/// }
/// ```
pub struct AdmittedEffectiveWorkflowGovernanceBundle<'a> {
    document: WorkflowGovernanceBundleDocument,
    identity: WorkflowEffectiveBundleIdentity,
    domain_context: WorkflowDomainPackContextView<'a>,
}

impl fmt::Debug for AdmittedEffectiveWorkflowGovernanceBundle<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmittedEffectiveWorkflowGovernanceBundle")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl AdmittedEffectiveWorkflowGovernanceBundle<'_> {
    #[must_use]
    pub const fn identity(&self) -> &WorkflowEffectiveBundleIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn domain_pack_generation(&self) -> Option<&WorkflowDomainPackGenerationIdentity> {
        self.identity.domain_pack_generation.as_ref()
    }

    /// Exact effective policy document. This is an audit/evaluation view; a
    /// cloned document cannot recreate this opaque admission.
    #[must_use]
    pub const fn document(&self) -> &WorkflowGovernanceBundleDocument {
        &self.document
    }

    /// Blocking gaps from a governed empty-package remove/rollback generation.
    /// Core-only and ready active generations return an empty slice.
    #[must_use]
    pub fn domain_pack_gaps(&self) -> &[DomainPackCompositionGap] {
        match &self.domain_context {
            WorkflowDomainPackContextView::CoreOnly(_) => &[],
            WorkflowDomainPackContextView::Active(view) => view.degraded_gaps(),
        }
    }

    #[must_use]
    pub fn is_domain_pack_degraded(&self) -> bool {
        !self.domain_pack_gaps().is_empty()
    }
}

/// Move-only borrowed lifecycle context required for every effective workflow
/// admission. Both variants retain a Rust borrow of the handle that owns the
/// lifecycle OS lock; the resulting effective authority therefore cannot
/// outlive the exact core-only or active-generation observation it joined.
pub enum WorkflowDomainPackContextView<'a> {
    CoreOnly(AdmittedCoreOnlyDomainPackLifecycleView<'a>),
    Active(AdmittedActiveDomainPackGenerationView<'a>),
}

impl fmt::Debug for WorkflowDomainPackContextView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoreOnly(_) => formatter.write_str("WorkflowDomainPackContextView::CoreOnly"),
            Self::Active(view) => formatter
                .debug_tuple("WorkflowDomainPackContextView::Active")
                .field(view)
                .finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EffectiveWorkflowGovernanceBundleError {
    Canonicalization(String),
    CoreBundleDigestMismatch,
    EffectiveCorePrefixMismatch,
    EffectiveBundleIdentityMismatch,
}

impl fmt::Display for EffectiveWorkflowGovernanceBundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonicalization(source) => {
                write!(
                    formatter,
                    "effective workflow bundle canonicalization failed: {source}"
                )
            }
            Self::CoreBundleDigestMismatch => formatter.write_str(
                "active Domain Pack generation does not bind the admitted inner core bundle",
            ),
            Self::EffectiveCorePrefixMismatch => formatter.write_str(
                "active Domain Pack effective policies do not preserve the admitted core prefix",
            ),
            Self::EffectiveBundleIdentityMismatch => formatter
                .write_str("active Domain Pack effective bundle has an invalid runtime identity"),
        }
    }
}

impl std::error::Error for EffectiveWorkflowGovernanceBundleError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowDomainPackReleaseRebaseDecision {
    /// No Domain Pack generation is active; the normal core upgrade may run.
    NoActiveGeneration,
    /// Any active generation, including a core-only generation produced by
    /// removal, requires one explicit coordinated rebase before core release
    /// transition. P6d deliberately has no unsafe cross-store 2PC shortcut.
    RebaseRequired,
}

#[derive(Serialize)]
struct ReceiptContextSubject<'a> {
    core_runtime_bundle: &'a WorkflowRuntimeBundleIdentity,
    effective_runtime_bundle: &'a WorkflowRuntimeBundleIdentity,
    domain_pack: Option<DomainPackReceiptContextSubject<'a>>,
}

#[derive(Serialize)]
struct DomainPackReceiptContextSubject<'a> {
    base_core_bundle_digest: &'a str,
    active_lock_digest: &'a str,
    composition_digest: &'a str,
    supply_chain_registry_digest: &'a str,
    reviewer_registry_digest: &'a str,
    reviewed_registry_digest: &'a str,
    active_packages: &'a [forge_core_contracts::DomainPackComposedIdentity],
}

/// Derive the exact effective workflow authority under the retained Domain
/// Pack lifecycle lock. A verified core-only context preserves byte-identical
/// universal behavior while preventing a concurrent install race.
///
/// The Domain Pack core digest intentionally identifies the inner
/// `WorkflowGovernanceBundle`, as defined by P6a. The independently admitted
/// P5 `WorkflowRuntimeBundleIdentity` continues to identify the enclosing
/// document. Comparing both prevents conflating the two digest domains without
/// changing the P6a wire contract.
pub fn admit_effective_workflow_governance_bundle<'a>(
    core: &AdmittedWorkflowGovernanceRelease,
    domain_context: WorkflowDomainPackContextView<'a>,
) -> Result<AdmittedEffectiveWorkflowGovernanceBundle<'a>, EffectiveWorkflowGovernanceBundleError> {
    let core_document = core.document();
    let core_bundle = &core_document.workflow_governance_bundle;
    let inner_core_digest = canonical_digest(core_bundle)?;
    let core_policy_digest = workflow_policy_set_digest(&core_bundle.policies)
        .map_err(EffectiveWorkflowGovernanceBundleError::Canonicalization)?;
    if inner_core_digest.trim().is_empty()
        || core_policy_digest != core.runtime_bundle().policy_set_digest
        || workflow_runtime_bundle_digest(core_document)
            .map_err(EffectiveWorkflowGovernanceBundleError::Canonicalization)?
            != core.runtime_bundle().bundle_digest
        || core_bundle.id != core.runtime_bundle().bundle_id
    {
        return Err(EffectiveWorkflowGovernanceBundleError::EffectiveBundleIdentityMismatch);
    }

    let (effective_bundle, generation, receipt_domain_context) =
        if let WorkflowDomainPackContextView::Active(active) = &domain_context {
            let effective = active.effective_bundle();
            validate_domain_core_join(
                core_bundle,
                &inner_core_digest,
                active.base_core_bundle_digest(),
                effective,
            )?;
            let generation = WorkflowDomainPackGenerationIdentity {
                generation: active.generation_id(),
                active_lock_digest: active.lock_digest().to_owned(),
                composition_digest: active.composition_digest().to_owned(),
                base_core_bundle_digest: active.base_core_bundle_digest().to_owned(),
                supply_chain_registry_digest: active.supply_chain_registry_digest().to_owned(),
                reviewer_registry_digest: active.reviewer_registry_digest().to_owned(),
                reviewed_registry_digest: active.reviewed_registry_digest().to_owned(),
            };
            let context = DomainPackReceiptContextSubject {
                base_core_bundle_digest: active.base_core_bundle_digest(),
                active_lock_digest: active.lock_digest(),
                composition_digest: active.composition_digest(),
                supply_chain_registry_digest: active.supply_chain_registry_digest(),
                reviewer_registry_digest: active.reviewer_registry_digest(),
                reviewed_registry_digest: active.reviewed_registry_digest(),
                active_packages: active.active_package_identities(),
            };
            (effective.clone(), Some(generation), Some(context))
        } else {
            (core_bundle.clone(), None, None)
        };

    let document = WorkflowGovernanceBundleDocument {
        schema_version: WORKFLOW_GOVERNANCE_SCHEMA_VERSION.to_owned(),
        workflow_governance_bundle: effective_bundle,
    };
    let effective_runtime = WorkflowRuntimeBundleIdentity {
        bundle_id: document.workflow_governance_bundle.id.clone(),
        bundle_digest: workflow_runtime_bundle_digest(&document)
            .map_err(EffectiveWorkflowGovernanceBundleError::Canonicalization)?,
        policy_set_digest: workflow_policy_set_digest(
            &document.workflow_governance_bundle.policies,
        )
        .map_err(EffectiveWorkflowGovernanceBundleError::Canonicalization)?,
    };
    let receipt_context_digest = canonical_digest(&ReceiptContextSubject {
        core_runtime_bundle: core.runtime_bundle(),
        effective_runtime_bundle: &effective_runtime,
        domain_pack: receipt_domain_context,
    })?;
    Ok(AdmittedEffectiveWorkflowGovernanceBundle {
        document,
        domain_context,
        identity: WorkflowEffectiveBundleIdentity {
            core_runtime_bundle: core.runtime_bundle().clone(),
            effective_runtime_bundle: effective_runtime,
            domain_pack_generation: generation,
            receipt_context_digest,
        },
    })
}

/// Derive the non-authoritative core-only epoch identity used as the source of
/// the first Domain Pack transition. This returns audit identity only, never an
/// executable bundle capability, so it does not replace the lifecycle-guarded
/// admission required for evaluation.
pub fn derive_core_only_workflow_effective_identity(
    core: &AdmittedWorkflowGovernanceRelease,
) -> Result<WorkflowEffectiveBundleIdentity, EffectiveWorkflowGovernanceBundleError> {
    let core_document = core.document();
    let core_bundle = &core_document.workflow_governance_bundle;
    let policy_set_digest = workflow_policy_set_digest(&core_bundle.policies)
        .map_err(EffectiveWorkflowGovernanceBundleError::Canonicalization)?;
    let runtime_digest = workflow_runtime_bundle_digest(core_document)
        .map_err(EffectiveWorkflowGovernanceBundleError::Canonicalization)?;
    if policy_set_digest != core.runtime_bundle().policy_set_digest
        || runtime_digest != core.runtime_bundle().bundle_digest
        || core_bundle.id != core.runtime_bundle().bundle_id
    {
        return Err(EffectiveWorkflowGovernanceBundleError::EffectiveBundleIdentityMismatch);
    }
    let runtime = core.runtime_bundle().clone();
    let receipt_context_digest = canonical_digest(&ReceiptContextSubject {
        core_runtime_bundle: &runtime,
        effective_runtime_bundle: &runtime,
        domain_pack: None,
    })?;
    Ok(WorkflowEffectiveBundleIdentity {
        core_runtime_bundle: runtime.clone(),
        effective_runtime_bundle: runtime,
        domain_pack_generation: None,
        receipt_context_digest,
    })
}

fn validate_domain_core_join(
    core_bundle: &WorkflowGovernanceBundle,
    admitted_inner_core_digest: &str,
    generation_base_core_digest: &str,
    effective: &WorkflowGovernanceBundle,
) -> Result<(), EffectiveWorkflowGovernanceBundleError> {
    if generation_base_core_digest != admitted_inner_core_digest {
        return Err(EffectiveWorkflowGovernanceBundleError::CoreBundleDigestMismatch);
    }
    if effective.policies.len() < core_bundle.policies.len()
        || effective.policies[..core_bundle.policies.len()] != core_bundle.policies
    {
        return Err(EffectiveWorkflowGovernanceBundleError::EffectiveCorePrefixMismatch);
    }
    Ok(())
}

/// Build the sole TCB-acceptable epoch event from two kernel-admitted
/// effective identities. Carryover is derived, never caller selected.
pub fn domain_pack_generation_transition_event(
    from: &WorkflowEffectiveBundleIdentity,
    to: &AdmittedEffectiveWorkflowGovernanceBundle<'_>,
    prior_ledger_head_digest: String,
) -> DomainPackGenerationTransitionedEvent {
    DomainPackGenerationTransitionedEvent {
        from_effective_bundle: from.clone(),
        to_effective_bundle: to.identity.clone(),
        receipt_carryover: domain_pack_receipt_carryover(from, &to.identity),
        prior_ledger_head_digest,
    }
}

/// Fail-closed decision for a core release upgrade while a project-local
/// generation is active. Every target requires an explicit coordinated Domain
/// Pack rebase transaction before the core release transition; this helper
/// never advertises a partial cross-store upgrade as safe.
pub fn evaluate_domain_pack_release_rebase(
    current: &AdmittedEffectiveWorkflowGovernanceBundle<'_>,
    _target_core: &AdmittedWorkflowGovernanceRelease,
) -> Result<WorkflowDomainPackReleaseRebaseDecision, EffectiveWorkflowGovernanceBundleError> {
    let Some(_generation) = current.domain_pack_generation() else {
        return Ok(WorkflowDomainPackReleaseRebaseDecision::NoActiveGeneration);
    };
    Ok(WorkflowDomainPackReleaseRebaseDecision::RebaseRequired)
}

fn canonical_digest<T: Serialize>(
    value: &T,
) -> Result<String, EffectiveWorkflowGovernanceBundleError> {
    let canonical = serde_json_canonicalizer::to_vec(value).map_err(|error| {
        EffectiveWorkflowGovernanceBundleError::Canonicalization(error.to_string())
    })?;
    Ok(sha256_content_hash(&canonical))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow_governance::load_admitted_workflow_governance_reviewed_release_registry;
    use forge_core_domain_pack_tcb::lock_domain_pack_lifecycle;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn state_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let project = std::env::temp_dir().join(format!(
            "forge-effective-domain-context-{label}-{}-{nonce}",
            std::process::id()
        ));
        let root = project.join(".forge-method");
        fs::create_dir_all(&root).expect("state root");
        root
    }

    #[test]
    fn carryover_requires_exact_runtime_and_context_equivalence() {
        let runtime = WorkflowRuntimeBundleIdentity {
            bundle_id: forge_core_contracts::StableId("bundle.test".to_owned()),
            bundle_digest: format!("sha256:{}", "1".repeat(64)),
            policy_set_digest: format!("sha256:{}", "2".repeat(64)),
        };
        let identity = WorkflowEffectiveBundleIdentity {
            core_runtime_bundle: runtime.clone(),
            effective_runtime_bundle: runtime,
            domain_pack_generation: None,
            receipt_context_digest: format!("sha256:{}", "3".repeat(64)),
        };
        assert_eq!(
            domain_pack_receipt_carryover(&identity, &identity),
            forge_core_contracts::WorkflowReceiptCarryover::PreservePolicyEquivalent
        );
        let mut drift = identity.clone();
        drift.receipt_context_digest = format!("sha256:{}", "4".repeat(64));
        assert_eq!(
            domain_pack_receipt_carryover(&identity, &drift),
            forge_core_contracts::WorkflowReceiptCarryover::InvalidateAll
        );
    }

    #[test]
    fn rebase_decision_without_generation_is_core_upgrade_safe() {
        let registry = load_admitted_workflow_governance_reviewed_release_registry()
            .expect("closed reviewed registry");
        let release = registry.latest_release();
        let root = state_root("core-only");
        let lifecycle = lock_domain_pack_lifecycle(&root).expect("lock empty lifecycle");
        let core_only = lifecycle
            .verified_core_only_view()
            .expect("verify core-only lifecycle");
        let admitted = admit_effective_workflow_governance_bundle(
            release,
            WorkflowDomainPackContextView::CoreOnly(core_only),
        )
        .expect("core-only effective admission");
        assert_eq!(admitted.domain_pack_generation(), None);
        assert_eq!(
            admitted.identity().core_runtime_bundle,
            admitted.identity().effective_runtime_bundle
        );
        assert_eq!(
            admitted
                .document()
                .workflow_governance_bundle
                .policies
                .len(),
            release.policy_count()
        );
        assert_eq!(
            evaluate_domain_pack_release_rebase(&admitted, release).expect("rebase decision"),
            WorkflowDomainPackReleaseRebaseDecision::NoActiveGeneration
        );
        assert!(admitted.domain_pack_gaps().is_empty());
        drop(admitted);
        drop(lifecycle);
        fs::remove_dir_all(root.parent().expect("project root")).expect("cleanup");
    }

    #[test]
    fn dual_digest_join_rejects_wrong_base_or_mutated_core_prefix() {
        let registry = load_admitted_workflow_governance_reviewed_release_registry()
            .expect("closed reviewed registry");
        let core = &registry
            .latest_release()
            .document()
            .workflow_governance_bundle;
        let inner_digest = canonical_digest(core).expect("inner core digest");
        assert!(validate_domain_core_join(core, &inner_digest, &inner_digest, core).is_ok());

        assert_eq!(
            validate_domain_core_join(
                core,
                &inner_digest,
                &format!("sha256:{}", "0".repeat(64)),
                core
            ),
            Err(EffectiveWorkflowGovernanceBundleError::CoreBundleDigestMismatch)
        );

        let mut truncated = core.clone();
        truncated.policies.pop();
        assert_eq!(
            validate_domain_core_join(core, &inner_digest, &inner_digest, &truncated),
            Err(EffectiveWorkflowGovernanceBundleError::EffectiveCorePrefixMismatch)
        );
    }
}
