//! Pure deterministic P6b exact-lock compatibility evaluation.
//!
//! Compatibility is evaluated from complete old/new lock candidates and an
//! independently supplied sealed Core binding.  It never mutates lifecycle
//! state and its report remains candidate-only.

use forge_core_contracts::{
    DomainPackCandidateAuthority, DomainPackCapabilityImpact, DomainPackCompatibilityIssue,
    DomainPackCompatibilityIssueCode, DomainPackCompatibilityReport,
    DomainPackCompatibilityReportDocument, DomainPackCompatibilityStatus, DomainPackCompositionGap,
    DomainPackCompositionGapCode, DomainPackCoreBinding, DomainPackExactLockDocument,
    DomainPackLifecycleOperation, DomainPackLockedCapabilityBinding, DomainPackLockedPackage,
    DomainPackReceiptMigrationPolicy, DomainPackRequirementImpact,
    DomainPackRequirementImpactStatus, DomainPackRuntimeCapabilityGap,
    DomainPackRuntimeCapabilityGapCode, DomainPackSandboxDecision, DomainPackSemanticChange,
    DomainPackSemanticChangeKind, DomainPackSourceAssurance, StableId,
    DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCompatibilityInput {
    pub report_id: StableId,
    pub operation: DomainPackLifecycleOperation,
    /// Independent immutable binding for the universal Core expected at both
    /// sides of the transition.
    pub sealed_core: DomainPackCoreBinding,
    pub from_lock: Option<DomainPackExactLockDocument>,
    pub to_lock: DomainPackExactLockDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PackageCoordinateKey {
    publisher: StableId,
    name: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CapabilityKey {
    publisher: StableId,
    name: StableId,
    version: String,
    subject_ref: StableId,
    capability_ref: StableId,
}

#[derive(Serialize)]
struct CompatibilityDigestSubject<'a> {
    report_id: &'a StableId,
    authority: DomainPackCandidateAuthority,
    operation: &'a DomainPackLifecycleOperation,
    from_lock_digest: &'a Option<String>,
    to_lock_digest: &'a str,
    from_composition_digest: &'a Option<String>,
    to_composition_digest: &'a str,
    changes: &'a [DomainPackSemanticChange],
    requirement_impacts: &'a [DomainPackRequirementImpact],
    capability_impacts: &'a [DomainPackCapabilityImpact],
    receipt_policy: DomainPackReceiptMigrationPolicy,
    universal_core_unchanged: bool,
    status: DomainPackCompatibilityStatus,
    issues: &'a [DomainPackCompatibilityIssue],
}

/// Compares exact locks and produces a stable candidate compatibility report.
///
/// Install, upgrade, and rollback fail closed on requirement, capability, or
/// trust regressions.  Remove may produce `Degraded` when the only regression
/// is an explicit unresolved capability gap; it still cannot alter universal
/// Core or weaken trust silently.
#[must_use]
pub fn evaluate_domain_pack_compatibility(
    input: &DomainPackCompatibilityInput,
) -> DomainPackCompatibilityReportDocument {
    let old = input
        .from_lock
        .as_ref()
        .map(|document| &document.domain_pack_exact_lock);
    let new = &input.to_lock.domain_pack_exact_lock;
    let mut changes = Vec::new();
    let mut requirement_impacts = Vec::new();
    let mut capability_impacts = Vec::new();
    let mut issues = Vec::new();
    let mut fatal = false;

    if !lock_digest_valid(new) {
        compatibility_issue(
            &mut issues,
            DomainPackCompatibilityIssueCode::InvalidLockDigest,
            "to_lock.lock_digest",
            "target lock digest does not match its canonical payload",
        );
        fatal = true;
    }
    if let Some(old) = old {
        if !lock_digest_valid(old) {
            compatibility_issue(
                &mut issues,
                DomainPackCompatibilityIssueCode::InvalidLockDigest,
                "from_lock.lock_digest",
                "source lock digest does not match its canonical payload",
            );
            fatal = true;
        }
    }

    let new_core_matches = new.payload.core == input.sealed_core;
    let old_core_matches = old.is_none_or(|old| old.payload.core == input.sealed_core);
    let universal_core_unchanged = new_core_matches && old_core_matches;
    if !universal_core_unchanged {
        compatibility_issue(
            &mut issues,
            DomainPackCompatibilityIssueCode::CoreChanged,
            "lock.payload.core",
            "source or target lock differs from the independently sealed universal Core",
        );
        fatal = true;
    }

    let old_packages = old.map_or_else(BTreeMap::new, |lock| package_map(&lock.payload.packages));
    let new_packages = package_map(&new.payload.packages);
    let all_package_keys = old_packages
        .keys()
        .chain(new_packages.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    for key in all_package_keys {
        let before = old_packages.get(&key).copied();
        let after = new_packages.get(&key).copied();
        let subject = format!("{}.{}", key.publisher.0, key.name.0);
        match (before, after) {
            (None, Some(after)) => semantic_change(
                &mut changes,
                DomainPackSemanticChangeKind::PackAdded,
                &subject,
                None,
                Some(after.package_digest.clone()),
                "package added to exact lock",
            ),
            (Some(before), None) => semantic_change(
                &mut changes,
                DomainPackSemanticChangeKind::PackRemoved,
                &subject,
                Some(before.package_digest.clone()),
                None,
                "package removed from exact lock",
            ),
            (Some(before), Some(after)) => {
                if before.identity.namespace != after.identity.namespace {
                    compatibility_issue(
                        &mut issues,
                        DomainPackCompatibilityIssueCode::NamespaceChanged,
                        format!("packages.{subject}.identity.namespace"),
                        "package namespace changed across an exact-lock transition",
                    );
                    fatal = true;
                }
                if before.identity.version != after.identity.version {
                    semantic_change(
                        &mut changes,
                        DomainPackSemanticChangeKind::PackVersionChanged,
                        &subject,
                        Some(canonical_digest(&before.identity.version)),
                        Some(canonical_digest(&after.identity.version)),
                        "selected package version changed",
                    );
                }
                if package_content_fingerprint(before) != package_content_fingerprint(after) {
                    semantic_change(
                        &mut changes,
                        DomainPackSemanticChangeKind::PackContentChanged,
                        &subject,
                        Some(package_content_fingerprint(before)),
                        Some(package_content_fingerprint(after)),
                        "content-addressed package artifacts changed",
                    );
                }
                if canonical_digest(&before.dependencies) != canonical_digest(&after.dependencies) {
                    semantic_change(
                        &mut changes,
                        DomainPackSemanticChangeKind::DependencyChanged,
                        &subject,
                        Some(canonical_digest(&before.dependencies)),
                        Some(canonical_digest(&after.dependencies)),
                        "resolved dependency set changed",
                    );
                }
                if before.source_assurance != after.source_assurance {
                    semantic_change(
                        &mut changes,
                        DomainPackSemanticChangeKind::TrustChanged,
                        &subject,
                        Some(canonical_digest(&assurance_name(before.source_assurance))),
                        Some(canonical_digest(&assurance_name(after.source_assurance))),
                        "source assurance changed",
                    );
                    if after.source_assurance < before.source_assurance {
                        compatibility_issue(
                            &mut issues,
                            DomainPackCompatibilityIssueCode::TrustDegraded,
                            format!("packages.{subject}.source_assurance"),
                            "target source assurance is weaker than the active lock",
                        );
                        fatal = true;
                    }
                }
            }
            (None, None) => unreachable!("key is drawn from one of the maps"),
        }
    }

    if new
        .payload
        .packages
        .iter()
        .any(|package| package.source_assurance == DomainPackSourceAssurance::ExplicitlyUntrusted)
    {
        compatibility_issue(
            &mut issues,
            DomainPackCompatibilityIssueCode::TrustDegraded,
            "to_lock.payload.packages",
            "target lock contains an explicitly untrusted package",
        );
        fatal = true;
    }

    if let Some(old) = old {
        compare_lock_policy_bindings(old, new, &mut changes, &mut issues, &mut fatal);
        if old.payload.requirements_digest != new.payload.requirements_digest {
            compatibility_issue(
                &mut issues,
                DomainPackCompatibilityIssueCode::RequirementsChangedWithoutIntent,
                "to_lock.payload.requirements_digest",
                "persistent project requirements changed inside a package lifecycle transition",
            );
            fatal = true;
        }
    }

    let old_gaps = old.map_or_else(BTreeMap::new, |lock| {
        gap_map(&lock.payload.unresolved_capability_gaps)
    });
    let new_gaps = gap_map(&new.payload.unresolved_capability_gaps);
    build_requirement_impacts(&old_gaps, &new_gaps, &mut requirement_impacts);
    let old_composition_gaps = old.map_or_else(BTreeMap::new, |lock| {
        composition_gap_map(&lock.payload.unresolved_composition_gaps)
    });
    let new_composition_gaps = composition_gap_map(&new.payload.unresolved_composition_gaps);
    build_composition_gap_impacts(
        &old_composition_gaps,
        &new_composition_gaps,
        &mut requirement_impacts,
    );

    let old_bindings = old.map_or_else(BTreeMap::new, |lock| {
        capability_map(&lock.payload.verified_capability_bindings)
    });
    let new_bindings = capability_map(&new.payload.verified_capability_bindings);
    build_capability_impacts(
        &old_bindings,
        &new_bindings,
        &old_gaps,
        &new_gaps,
        &mut capability_impacts,
        &mut changes,
    );

    let is_remove = matches!(input.operation, DomainPackLifecycleOperation::Remove { .. });
    if !new_composition_gaps.is_empty() {
        for (key, gap) in &new_composition_gaps {
            compatibility_issue(
                &mut issues,
                if gap.code == DomainPackCompositionGapCode::MissingDomain {
                    DomainPackCompatibilityIssueCode::MissingRequiredDomain
                } else {
                    DomainPackCompatibilityIssueCode::MissingRequiredCapability
                },
                format!(
                    "to_lock.payload.unresolved_composition_gaps.{}.{}.{}",
                    key.0, key.1, key.2
                ),
                gap.message.clone(),
            );
        }
        if !is_remove {
            fatal = true;
        }
    }
    if !new_gaps.is_empty() {
        for (key, gap) in &new_gaps {
            compatibility_issue(
                &mut issues,
                if matches!(
                    gap.code,
                    DomainPackRuntimeCapabilityGapCode::Disabled
                        | DomainPackRuntimeCapabilityGapCode::Revoked
                        | DomainPackRuntimeCapabilityGapCode::ExternalProviderDenied
                ) {
                    DomainPackCompatibilityIssueCode::ExecutableCapabilityDenied
                } else {
                    DomainPackCompatibilityIssueCode::MissingRequiredCapability
                },
                format!(
                    "to_lock.payload.unresolved_capability_gaps.{}.{}",
                    key.subject_ref.0, key.capability_ref.0
                ),
                gap.message.clone(),
            );
        }
        if !is_remove {
            fatal = true;
        }
    }

    changes.sort_by(semantic_change_sort_key);
    changes.dedup();
    requirement_impacts.sort_by(requirement_impact_sort_key);
    requirement_impacts.dedup();
    capability_impacts.sort_by(capability_impact_sort_key);
    capability_impacts.dedup();
    issues.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(format!("{:?}", left.code).cmp(&format!("{:?}", right.code)))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();

    let status = if fatal {
        DomainPackCompatibilityStatus::Blocked
    } else if !new_gaps.is_empty() || !new_composition_gaps.is_empty() {
        DomainPackCompatibilityStatus::Degraded
    } else {
        DomainPackCompatibilityStatus::Compatible
    };
    let receipt_policy = if old.is_some() && changes.is_empty() && capability_impacts.is_empty() {
        DomainPackReceiptMigrationPolicy::PreserveExactEquivalent
    } else {
        DomainPackReceiptMigrationPolicy::InvalidateAll
    };
    let from_lock_digest = old.map(|lock| lock.lock_digest.clone());
    let from_composition_digest = old.map(|lock| lock.payload.composition_digest.clone());
    let digest_subject = CompatibilityDigestSubject {
        report_id: &input.report_id,
        authority: DomainPackCandidateAuthority::CandidateOnly,
        operation: &input.operation,
        from_lock_digest: &from_lock_digest,
        to_lock_digest: &new.lock_digest,
        from_composition_digest: &from_composition_digest,
        to_composition_digest: &new.payload.composition_digest,
        changes: &changes,
        requirement_impacts: &requirement_impacts,
        capability_impacts: &capability_impacts,
        receipt_policy,
        universal_core_unchanged,
        status,
        issues: &issues,
    };
    let report_digest = canonical_digest(&digest_subject);

    DomainPackCompatibilityReportDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_compatibility_report: DomainPackCompatibilityReport {
            report_id: input.report_id.clone(),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            operation: input.operation.clone(),
            from_lock_digest,
            to_lock_digest: new.lock_digest.clone(),
            from_composition_digest,
            to_composition_digest: new.payload.composition_digest.clone(),
            changes,
            requirement_impacts,
            capability_impacts,
            receipt_policy,
            universal_core_unchanged,
            status,
            issues,
            report_digest,
        },
    }
}

fn compare_lock_policy_bindings(
    old: &forge_core_contracts::DomainPackExactLock,
    new: &forge_core_contracts::DomainPackExactLock,
    changes: &mut Vec<DomainPackSemanticChange>,
    issues: &mut Vec<DomainPackCompatibilityIssue>,
    fatal: &mut bool,
) {
    if old.payload.trust_policy_digest != new.payload.trust_policy_digest {
        semantic_change(
            changes,
            DomainPackSemanticChangeKind::TrustChanged,
            "trust_policy",
            Some(old.payload.trust_policy_digest.clone()),
            Some(new.payload.trust_policy_digest.clone()),
            "trust policy binding changed",
        );
    }
    if old.payload.sandbox_policy_digest != new.payload.sandbox_policy_digest {
        semantic_change(
            changes,
            DomainPackSemanticChangeKind::SandboxChanged,
            "sandbox_policy",
            Some(old.payload.sandbox_policy_digest.clone()),
            Some(new.payload.sandbox_policy_digest.clone()),
            "sandbox policy binding changed",
        );
    }
    if old.payload.capability_registry_digest != new.payload.capability_registry_digest {
        semantic_change(
            changes,
            DomainPackSemanticChangeKind::CapabilityChanged,
            "capability_registry",
            Some(old.payload.capability_registry_digest.clone()),
            Some(new.payload.capability_registry_digest.clone()),
            "runtime capability registry binding changed",
        );
    }
    if old.payload.registry_snapshot_digest != new.payload.registry_snapshot_digest
        && old.payload.resolution_digest == new.payload.resolution_digest
    {
        compatibility_issue(
            issues,
            DomainPackCompatibilityIssueCode::RegistryChangedWithoutResolution,
            "to_lock.payload.registry_snapshot_digest",
            "registry snapshot changed without a corresponding resolution change",
        );
        *fatal = true;
    }
}

fn package_map(
    packages: &[DomainPackLockedPackage],
) -> BTreeMap<PackageCoordinateKey, &DomainPackLockedPackage> {
    packages
        .iter()
        .map(|package| {
            (
                PackageCoordinateKey {
                    publisher: package.identity.publisher.clone(),
                    name: package.identity.name.clone(),
                },
                package,
            )
        })
        .collect()
}

fn package_content_fingerprint(package: &DomainPackLockedPackage) -> String {
    #[derive(Serialize)]
    struct Fingerprint<'a> {
        package_digest: &'a str,
        manifest_binding: &'a forge_core_contracts::DomainPackArtifactBinding,
        content_binding: &'a forge_core_contracts::DomainPackContentBinding,
        license_binding: &'a forge_core_contracts::DomainPackArtifactBinding,
        fixture_bindings: &'a [forge_core_contracts::DomainPackArtifactBinding],
    }
    canonical_digest(&Fingerprint {
        package_digest: &package.package_digest,
        manifest_binding: &package.manifest_binding,
        content_binding: &package.content_binding,
        license_binding: &package.license_binding,
        fixture_bindings: &package.fixture_bindings,
    })
}

fn capability_key(binding: &DomainPackLockedCapabilityBinding) -> CapabilityKey {
    CapabilityKey {
        publisher: binding.pack.publisher.clone(),
        name: binding.pack.name.clone(),
        version: binding.pack.version.clone(),
        subject_ref: binding.subject_ref.clone(),
        capability_ref: binding.capability_ref.clone(),
    }
}

fn capability_map(
    bindings: &[DomainPackLockedCapabilityBinding],
) -> BTreeMap<CapabilityKey, &DomainPackLockedCapabilityBinding> {
    bindings
        .iter()
        .map(|binding| (capability_key(binding), binding))
        .collect()
}

fn gap_key(gap: &DomainPackRuntimeCapabilityGap) -> CapabilityKey {
    CapabilityKey {
        publisher: gap.pack.publisher.clone(),
        name: gap.pack.name.clone(),
        version: gap.pack.version.clone(),
        subject_ref: gap.subject_ref.clone(),
        capability_ref: gap.capability_ref.clone(),
    }
}

fn gap_map(
    gaps: &[DomainPackRuntimeCapabilityGap],
) -> BTreeMap<CapabilityKey, &DomainPackRuntimeCapabilityGap> {
    gaps.iter().map(|gap| (gap_key(gap), gap)).collect()
}

type CompositionGapKey = (String, String, String);

fn composition_gap_map(
    gaps: &[DomainPackCompositionGap],
) -> BTreeMap<CompositionGapKey, &DomainPackCompositionGap> {
    gaps.iter()
        .map(|gap| {
            (
                (
                    gap.requirement_ref.0.clone(),
                    gap.subject_ref.0.clone(),
                    format!("{:?}", gap.code),
                ),
                gap,
            )
        })
        .collect()
}

fn build_composition_gap_impacts(
    old: &BTreeMap<CompositionGapKey, &DomainPackCompositionGap>,
    new: &BTreeMap<CompositionGapKey, &DomainPackCompositionGap>,
    impacts: &mut Vec<DomainPackRequirementImpact>,
) {
    for key in old
        .keys()
        .chain(new.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        let status = match (old.contains_key(&key), new.contains_key(&key)) {
            (false, true) => DomainPackRequirementImpactStatus::NewlyMissing,
            (true, true) => DomainPackRequirementImpactStatus::StillMissing,
            (true, false) => DomainPackRequirementImpactStatus::NewlySatisfied,
            (false, false) => continue,
        };
        let gap = new.get(&key).or_else(|| old.get(&key)).expect("key exists");
        impacts.push(DomainPackRequirementImpact {
            requirement_ref: gap.requirement_ref.clone(),
            subject_ref: gap.subject_ref.clone(),
            status,
            explanation: match status {
                DomainPackRequirementImpactStatus::NewlyMissing => {
                    "required domain contribution became unavailable"
                }
                DomainPackRequirementImpactStatus::StillMissing => {
                    "required domain contribution remains unavailable"
                }
                DomainPackRequirementImpactStatus::NewlySatisfied => {
                    "required domain contribution became available"
                }
                DomainPackRequirementImpactStatus::Satisfied => {
                    "required domain contribution remains available"
                }
            }
            .to_owned(),
        });
    }
}

fn build_requirement_impacts(
    old: &BTreeMap<CapabilityKey, &DomainPackRuntimeCapabilityGap>,
    new: &BTreeMap<CapabilityKey, &DomainPackRuntimeCapabilityGap>,
    impacts: &mut Vec<DomainPackRequirementImpact>,
) {
    for key in old
        .keys()
        .chain(new.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        let status = match (old.contains_key(&key), new.contains_key(&key)) {
            (false, true) => DomainPackRequirementImpactStatus::NewlyMissing,
            (true, true) => DomainPackRequirementImpactStatus::StillMissing,
            (true, false) => DomainPackRequirementImpactStatus::NewlySatisfied,
            (false, false) => continue,
        };
        impacts.push(DomainPackRequirementImpact {
            requirement_ref: key.capability_ref.clone(),
            subject_ref: key.subject_ref.clone(),
            status,
            explanation: match status {
                DomainPackRequirementImpactStatus::NewlyMissing => {
                    "required runtime capability became unavailable"
                }
                DomainPackRequirementImpactStatus::StillMissing => {
                    "required runtime capability remains unavailable"
                }
                DomainPackRequirementImpactStatus::NewlySatisfied => {
                    "required runtime capability became available"
                }
                DomainPackRequirementImpactStatus::Satisfied => {
                    "required runtime capability remains available"
                }
            }
            .to_owned(),
        });
    }
}

fn build_capability_impacts(
    old_bindings: &BTreeMap<CapabilityKey, &DomainPackLockedCapabilityBinding>,
    new_bindings: &BTreeMap<CapabilityKey, &DomainPackLockedCapabilityBinding>,
    old_gaps: &BTreeMap<CapabilityKey, &DomainPackRuntimeCapabilityGap>,
    new_gaps: &BTreeMap<CapabilityKey, &DomainPackRuntimeCapabilityGap>,
    impacts: &mut Vec<DomainPackCapabilityImpact>,
    changes: &mut Vec<DomainPackSemanticChange>,
) {
    let keys = old_bindings
        .keys()
        .chain(new_bindings.keys())
        .chain(old_gaps.keys())
        .chain(new_gaps.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for key in keys {
        let before = old_bindings
            .get(&key)
            .map(|binding| binding.decision)
            .or_else(|| old_gaps.get(&key).map(|gap| gap_decision(gap.code)));
        let after = new_bindings
            .get(&key)
            .map(|binding| binding.decision)
            .or_else(|| new_gaps.get(&key).map(|gap| gap_decision(gap.code)));
        let implementation_changed = old_bindings
            .get(&key)
            .zip(new_bindings.get(&key))
            .is_some_and(|(old, new)| {
                old.binding_id != new.binding_id
                    || old.provider_id != new.provider_id
                    || old.implementation_digest != new.implementation_digest
                    || old.package_digest != new.package_digest
            });
        if before == after && !implementation_changed {
            continue;
        }
        impacts.push(DomainPackCapabilityImpact {
            capability_ref: key.capability_ref.clone(),
            before,
            after,
            explanation: if implementation_changed {
                "exact built-in capability binding changed"
            } else {
                "runtime capability decision changed"
            }
            .to_owned(),
        });
        let before_fingerprint = old_bindings
            .get(&key)
            .map(|binding| canonical_digest(*binding))
            .or_else(|| old_gaps.get(&key).map(|gap| canonical_digest(*gap)));
        let after_fingerprint = new_bindings
            .get(&key)
            .map(|binding| canonical_digest(*binding))
            .or_else(|| new_gaps.get(&key).map(|gap| canonical_digest(*gap)));
        semantic_change(
            changes,
            DomainPackSemanticChangeKind::CapabilityChanged,
            &key.capability_ref.0,
            before_fingerprint,
            after_fingerprint,
            if implementation_changed {
                "exact built-in capability binding changed"
            } else {
                "runtime capability decision changed"
            },
        );
    }
}

fn gap_decision(code: DomainPackRuntimeCapabilityGapCode) -> DomainPackSandboxDecision {
    match code {
        DomainPackRuntimeCapabilityGapCode::MissingBinding
        | DomainPackRuntimeCapabilityGapCode::PackageDigestMismatch
        | DomainPackRuntimeCapabilityGapCode::KindMismatch => {
            DomainPackSandboxDecision::Unavailable
        }
        DomainPackRuntimeCapabilityGapCode::UndeclaredBinding
        | DomainPackRuntimeCapabilityGapCode::ExternalProviderDenied
        | DomainPackRuntimeCapabilityGapCode::Disabled => DomainPackSandboxDecision::DeniedByPolicy,
        DomainPackRuntimeCapabilityGapCode::Unavailable => DomainPackSandboxDecision::Unavailable,
        DomainPackRuntimeCapabilityGapCode::Revoked => DomainPackSandboxDecision::Revoked,
    }
}

fn lock_digest_valid(lock: &forge_core_contracts::DomainPackExactLock) -> bool {
    valid_digest_wire(&lock.lock_digest) && canonical_digest(&lock.payload) == lock.lock_digest
}

fn valid_digest_wire(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn assurance_name(assurance: DomainPackSourceAssurance) -> &'static str {
    match assurance {
        DomainPackSourceAssurance::ExplicitlyUntrusted => "explicitly_untrusted",
        DomainPackSourceAssurance::LocalExplicit => "local_explicit",
        DomainPackSourceAssurance::SupplyChainVerified => "supply_chain_verified",
    }
}

fn semantic_change(
    changes: &mut Vec<DomainPackSemanticChange>,
    kind: DomainPackSemanticChangeKind,
    subject_ref: &str,
    before_digest: Option<String>,
    after_digest: Option<String>,
    explanation: &str,
) {
    changes.push(DomainPackSemanticChange {
        kind,
        subject_ref: StableId(subject_ref.to_owned()),
        before_digest,
        after_digest,
        explanation: explanation.to_owned(),
    });
}

fn compatibility_issue(
    issues: &mut Vec<DomainPackCompatibilityIssue>,
    code: DomainPackCompatibilityIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(DomainPackCompatibilityIssue {
        code,
        path: path.into(),
        message: message.into(),
    });
}

fn semantic_change_sort_key(
    left: &DomainPackSemanticChange,
    right: &DomainPackSemanticChange,
) -> std::cmp::Ordering {
    left.kind
        .cmp(&right.kind)
        .then(left.subject_ref.cmp(&right.subject_ref))
        .then(left.before_digest.cmp(&right.before_digest))
        .then(left.after_digest.cmp(&right.after_digest))
}

fn requirement_impact_sort_key(
    left: &DomainPackRequirementImpact,
    right: &DomainPackRequirementImpact,
) -> std::cmp::Ordering {
    left.requirement_ref
        .cmp(&right.requirement_ref)
        .then(left.subject_ref.cmp(&right.subject_ref))
        .then(format!("{:?}", left.status).cmp(&format!("{:?}", right.status)))
}

fn capability_impact_sort_key(
    left: &DomainPackCapabilityImpact,
    right: &DomainPackCapabilityImpact,
) -> std::cmp::Ordering {
    left.capability_ref
        .cmp(&right.capability_ref)
        .then(format!("{:?}", left.before).cmp(&format!("{:?}", right.before)))
        .then(format!("{:?}", left.after).cmp(&format!("{:?}", right.after)))
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    serde_json_canonicalizer::to_vec(value).map_or_else(
        |_| {
            format!(
                "sha256:{:x}",
                Sha256::digest(b"domain-pack-compatibility-encoding-failed")
            )
        },
        |bytes| format!("sha256:{:x}", Sha256::digest(bytes)),
    )
}
