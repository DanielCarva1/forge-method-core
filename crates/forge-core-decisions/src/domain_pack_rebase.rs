//! Pure P7D read-only Core/Domain-Pack rebase planning.
//!
//! This module deliberately stops before mutation. It binds every observed
//! authority head into one deterministic candidate plan; the apply coordinator
//! must independently revalidate target-side authority and exact CAS.

use forge_core_contracts::{
    DomainPackCandidateAuthority, DomainPackRebaseActiveGeneration, DomainPackRebaseApplyStatus,
    DomainPackRebaseCheckStatus, DomainPackRebaseCompatibilityProjection, DomainPackRebasePlan,
    DomainPackRebasePlanDocument, DomainPackRebasePlanInput, DomainPackRebaseSemanticChange,
    DomainPackRebaseSemanticChangeKind, DomainPackReceiptMigrationPolicy, StableId,
    DOMAIN_PACK_REBASE_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainPackRebasePlanError {
    NonAdjacentReleaseIdentity,
    MissingActiveGeneration,
    ActiveGenerationBindingMismatch(&'static str),
    InvalidDigest(&'static str),
    Canonicalization(String),
}

impl fmt::Display for DomainPackRebasePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonAdjacentReleaseIdentity => formatter.write_str(
                "source and target must be distinct releases in the same admitted lineage",
            ),
            Self::MissingActiveGeneration => {
                formatter.write_str("an active Domain Pack generation is required")
            }
            Self::ActiveGenerationBindingMismatch(field) => {
                write!(
                    formatter,
                    "active Domain Pack generation does not match {field}"
                )
            }
            Self::InvalidDigest(field) => {
                write!(formatter, "{field} is not a canonical SHA-256 digest")
            }
            Self::Canonicalization(source) => {
                write!(formatter, "rebase plan canonicalization failed: {source}")
            }
        }
    }
}

impl std::error::Error for DomainPackRebasePlanError {}

/// Derive a deterministic, non-authoritative exact-CAS rebase plan.
///
/// An internally consistent result is apply-ready only for opaque TCB
/// revalidation. It grants no mutation authority: target-core composition,
/// trust, review, capability, requirement, lifecycle, and joined-workflow
/// checks still fail closed at apply time.
///
/// # Errors
///
/// Fails closed when release lineage, active-generation cross-links, or digest
/// shapes are inconsistent.
pub fn plan_domain_pack_rebase(
    input: &DomainPackRebasePlanInput,
) -> Result<DomainPackRebasePlanDocument, DomainPackRebasePlanError> {
    validate_input(input)?;
    let generation = input
        .effective_identity
        .domain_pack_generation
        .as_ref()
        .ok_or(DomainPackRebasePlanError::MissingActiveGeneration)?;
    let policy_changed = input.source_core.policy_set_digest != input.target_core.policy_set_digest;
    let mut semantic_changes = vec![
        DomainPackRebaseSemanticChange {
            kind: DomainPackRebaseSemanticChangeKind::CoreReleaseChanged,
            subject_ref: input.target_release.release_id.clone(),
            before_digest: Some(input.source_release.release_digest.clone()),
            after_digest: Some(input.target_release.release_digest.clone()),
            explanation: "the exact adjacent admitted Core release changes".to_owned(),
        },
        DomainPackRebaseSemanticChange {
            kind: DomainPackRebaseSemanticChangeKind::CoreRuntimeBundleChanged,
            subject_ref: input.target_core.bundle_id.clone(),
            before_digest: Some(input.source_core.bundle_digest.clone()),
            after_digest: Some(input.target_core.bundle_digest.clone()),
            explanation: "the sealed universal Core bundle changes; the active pack set is not yet composed against it".to_owned(),
        },
    ];
    if policy_changed {
        semantic_changes.push(DomainPackRebaseSemanticChange {
            kind: DomainPackRebaseSemanticChangeKind::CorePolicySetChanged,
            subject_ref: input.target_core.bundle_id.clone(),
            before_digest: Some(input.source_core.policy_set_digest.clone()),
            after_digest: Some(input.target_core.policy_set_digest.clone()),
            explanation: "the admitted universal policy set changes".to_owned(),
        });
    }
    semantic_changes.push(DomainPackRebaseSemanticChange {
        kind: DomainPackRebaseSemanticChangeKind::PackCompatibilityPending,
        subject_ref: StableId("domain-pack.active-generation".to_owned()),
        before_digest: Some(input.active_lock_digest.clone()),
        after_digest: None,
        explanation: "package identities are retained only as candidates until resolver, composer, trust, review, capability, and requirement authority revalidate them against the target Core".to_owned(),
    });

    // The plan records every mandatory target-side revalidation in the
    // compatibility projection. They are apply-time TCB checks, not unresolved
    // authority gaps: apply may attempt them but cannot bypass any failure.
    let actionable_gaps = Vec::new();

    let exact_cas = forge_core_contracts::DomainPackRebaseExactCas {
        expected_current_release_digest: input.source_release.release_digest.clone(),
        expected_workflow_ledger_head_digest: input.workflow_ledger_head_digest.clone(),
        expected_project_snapshot_digest: input.project_snapshot_digest.clone(),
        expected_effective_bundle_digest: input
            .effective_identity
            .effective_runtime_bundle
            .bundle_digest
            .clone(),
        expected_receipt_context_digest: input.effective_identity.receipt_context_digest.clone(),
        expected_generation: input.generation,
        expected_lifecycle_pointer_digest: input.lifecycle_pointer_digest.clone(),
        expected_lifecycle_head_digest: input.lifecycle_head_digest.clone(),
        expected_active_lock_digest: input.active_lock_digest.clone(),
        expected_composition_digest: input.composition_digest.clone(),
        expected_supply_chain_registry_digest: input.supply_chain_registry_digest.clone(),
        expected_reviewer_registry_digest: input.reviewer_registry_digest.clone(),
        expected_reviewed_registry_digest: input.reviewed_registry_digest.clone(),
    };
    let mut plan = DomainPackRebasePlan {
        plan_id: StableId(format!(
            "domain-pack.rebase.{}.{}",
            input.source_release.release_id.0, input.target_release.release_id.0
        )),
        authority: DomainPackCandidateAuthority::CandidateOnly,
        mutation_allowed: true,
        apply_status: DomainPackRebaseApplyStatus::ReadyForTcbRevalidation,
        project_id: input.project_id.clone(),
        source_release: input.source_release.clone(),
        target_release: input.target_release.clone(),
        source_core: input.source_core.clone(),
        target_core: input.target_core.clone(),
        active_generation: DomainPackRebaseActiveGeneration {
            generation: input.generation,
            lifecycle_operation: input.lifecycle_operation.clone(),
            degraded_empty: input.active_package_count == 0
                && !input.active_composition_gaps.is_empty(),
            active_package_count: input.active_package_count,
            active_composition_gaps: input.active_composition_gaps.clone(),
        },
        exact_cas,
        compatibility: DomainPackRebaseCompatibilityProjection {
            adjacent_core_release_admitted: true,
            core_policy_set_changed: policy_changed,
            package_set_retained_as_candidate: true,
            target_core_pack_compatibility: DomainPackRebaseCheckStatus::RequiresTargetRevalidation,
            policy_recomposition: DomainPackRebaseCheckStatus::RequiresTargetRevalidation,
            capability_revalidation: DomainPackRebaseCheckStatus::RequiresTargetRevalidation,
            requirement_revalidation: DomainPackRebaseCheckStatus::RequiresTargetRevalidation,
            supply_chain_revalidation: DomainPackRebaseCheckStatus::RequiresTargetRevalidation,
            semantic_review_revalidation: DomainPackRebaseCheckStatus::RequiresTargetRevalidation,
            workflow_receipt_carryover: input.target_workflow_receipt_carryover,
            domain_pack_receipt_carryover: DomainPackReceiptMigrationPolicy::InvalidateAll,
        },
        semantic_changes,
        actionable_gaps,
        plan_digest: String::new(),
    };
    plan.plan_digest = plan_digest(&plan)?;
    debug_assert_eq!(generation.generation, plan.exact_cas.expected_generation);
    Ok(DomainPackRebasePlanDocument {
        schema_version: DOMAIN_PACK_REBASE_SCHEMA_VERSION.to_owned(),
        domain_pack_rebase_plan: plan,
    })
}

/// Verify the integrity and closed apply-ready invariants of a persisted
/// rebase plan before crash recovery uses it as non-authoritative evidence.
#[must_use]
pub fn verify_domain_pack_rebase_plan(document: &DomainPackRebasePlanDocument) -> bool {
    let plan = &document.domain_pack_rebase_plan;
    if document.schema_version != DOMAIN_PACK_REBASE_SCHEMA_VERSION
        || plan.authority != DomainPackCandidateAuthority::CandidateOnly
        || !plan.mutation_allowed
        || plan.apply_status != DomainPackRebaseApplyStatus::ReadyForTcbRevalidation
        || !plan.actionable_gaps.is_empty()
        || plan.source_release == plan.target_release
        || plan.source_release.lineage_id != plan.target_release.lineage_id
        || plan.source_core == plan.target_core
        || plan.exact_cas.expected_generation == u64::MAX
    {
        return false;
    }
    let mut subject = plan.clone();
    let claimed = std::mem::take(&mut subject.plan_digest);
    is_canonical_sha256(&claimed) && plan_digest(&subject).is_ok_and(|actual| actual == claimed)
}

fn validate_input(input: &DomainPackRebasePlanInput) -> Result<(), DomainPackRebasePlanError> {
    if input.source_release.lineage_id != input.target_release.lineage_id
        || input.source_release.release_id == input.target_release.release_id
    {
        return Err(DomainPackRebasePlanError::NonAdjacentReleaseIdentity);
    }
    let generation = input
        .effective_identity
        .domain_pack_generation
        .as_ref()
        .ok_or(DomainPackRebasePlanError::MissingActiveGeneration)?;
    let bindings = [
        (generation.generation == input.generation, "generation"),
        (
            generation.active_lock_digest == input.active_lock_digest,
            "active lock digest",
        ),
        (
            generation.composition_digest == input.composition_digest,
            "composition digest",
        ),
        (
            generation.base_core_bundle_digest == input.source_core.bundle_digest,
            "source Core bundle digest",
        ),
        (
            generation.supply_chain_registry_digest == input.supply_chain_registry_digest,
            "supply-chain registry digest",
        ),
        (
            generation.reviewer_registry_digest == input.reviewer_registry_digest,
            "reviewer registry digest",
        ),
        (
            generation.reviewed_registry_digest == input.reviewed_registry_digest,
            "reviewed registry digest",
        ),
    ];
    if let Some((_, field)) = bindings.into_iter().find(|(matches, _)| !matches) {
        return Err(DomainPackRebasePlanError::ActiveGenerationBindingMismatch(
            field,
        ));
    }
    for (field, digest) in [
        (
            "source release digest",
            input.source_release.release_digest.as_str(),
        ),
        (
            "target release digest",
            input.target_release.release_digest.as_str(),
        ),
        (
            "source Core bundle digest",
            input.source_core.bundle_digest.as_str(),
        ),
        (
            "target Core bundle digest",
            input.target_core.bundle_digest.as_str(),
        ),
        (
            "workflow ledger head digest",
            input.workflow_ledger_head_digest.as_str(),
        ),
        (
            "project snapshot digest",
            input.project_snapshot_digest.as_str(),
        ),
        (
            "lifecycle pointer digest",
            input.lifecycle_pointer_digest.as_str(),
        ),
        (
            "lifecycle head digest",
            input.lifecycle_head_digest.as_str(),
        ),
        ("active lock digest", input.active_lock_digest.as_str()),
        ("composition digest", input.composition_digest.as_str()),
        (
            "supply-chain registry digest",
            input.supply_chain_registry_digest.as_str(),
        ),
        (
            "effective bundle digest",
            input
                .effective_identity
                .effective_runtime_bundle
                .bundle_digest
                .as_str(),
        ),
        (
            "receipt context digest",
            input.effective_identity.receipt_context_digest.as_str(),
        ),
    ] {
        if !is_canonical_sha256(digest) {
            return Err(DomainPackRebasePlanError::InvalidDigest(field));
        }
    }
    for (field, digest) in [
        (
            "reviewer registry digest",
            input.reviewer_registry_digest.as_str(),
        ),
        (
            "reviewed registry digest",
            input.reviewed_registry_digest.as_str(),
        ),
    ] {
        if !is_canonical_raw_sha256(digest) {
            return Err(DomainPackRebasePlanError::InvalidDigest(field));
        }
    }
    Ok(())
}

fn plan_digest(plan: &DomainPackRebasePlan) -> Result<String, DomainPackRebasePlanError> {
    let mut subject = plan.clone();
    subject.plan_digest.clear();
    let canonical = serde_json_canonicalizer::to_vec(&subject)
        .map_err(|error| DomainPackRebasePlanError::Canonicalization(error.to_string()))?;
    Ok(format!("sha256:{:x}", Sha256::digest(&canonical)))
}

fn is_canonical_sha256(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(is_canonical_raw_sha256)
}

fn is_canonical_raw_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
