//! Pure P6b trust and runtime-capability policy evaluation.
//!
//! This module deliberately performs no IO and grants no authority.  It turns
//! already resolved, structurally checked, supply-chain assessed packages plus
//! explicit policy observations into a deterministic candidate assessment.
//! Capability declarations are never treated as runtime availability.

use forge_core_contracts::{
    DomainPackCapabilityKind, DomainPackCapabilitySandboxPolicy, DomainPackLifecycleTrustDecision,
    DomainPackLockedCapabilityBinding, DomainPackResolvedPackage,
    DomainPackRuntimeCapabilityBinding, DomainPackRuntimeCapabilityGap,
    DomainPackRuntimeCapabilityGapCode, DomainPackRuntimeCapabilityRegistry,
    DomainPackRuntimeCapabilityStatus, DomainPackRuntimeProvider, DomainPackSandboxDecision,
    DomainPackSupplyChainAssessment, DomainPackTrustDisposition, DomainPackTrustPolicy, StableId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// One capability use that survived structural composition.
///
/// `subject_ref` is the exact evaluator, adapter, lifecycle transition, or
/// other contribution that asks for `capability_ref`.  A pack-level capability
/// declaration without one of these demands cannot make a runtime binding
/// available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCapabilityDemand {
    pub subject_ref: StableId,
    pub capability_ref: StableId,
    pub kind: DomainPackCapabilityKind,
}

/// A selected package together with the observations made by the structural
/// and supply-chain boundaries.  This decision module does not attempt to
/// recreate either assessment from authored declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackTrustSelectedPackage {
    pub package: DomainPackResolvedPackage,
    pub structurally_valid: bool,
    pub supply_chain: DomainPackSupplyChainAssessment,
    pub capability_demands: Vec<DomainPackCapabilityDemand>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackTrustEvaluationInput {
    pub project_id: StableId,
    pub selected: Vec<DomainPackTrustSelectedPackage>,
    pub trust_policy: DomainPackTrustPolicy,
    pub capability_registry: DomainPackRuntimeCapabilityRegistry,
    pub sandbox_policy: DomainPackCapabilitySandboxPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackTrustEvaluationStatus {
    Approved,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackTrustIssueCode {
    PolicyDefaultNotDeny,
    ProjectMismatch,
    DuplicateSelectedPackage,
    StructuralAssessmentFailed,
    SupplyChainAssessmentMismatch,
    SupplyChainRejected,
    BelowMinimumAssurance,
    TrustRejected,
    AmbiguousTrustRule,
    DuplicateCapabilityDemand,
    DuplicateRuntimeBinding,
    CapabilityDenied,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackTrustIssue {
    pub code: DomainPackTrustIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackTrustEvaluation {
    pub status: DomainPackTrustEvaluationStatus,
    pub trust_decisions: Vec<DomainPackLifecycleTrustDecision>,
    pub verified_capability_bindings: Vec<DomainPackLockedCapabilityBinding>,
    pub capability_gaps: Vec<DomainPackRuntimeCapabilityGap>,
    pub issues: Vec<DomainPackTrustIssue>,
    pub evaluation_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PackageKey {
    publisher: StableId,
    name: StableId,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DemandKey {
    package: PackageKey,
    subject_ref: StableId,
    capability_ref: StableId,
}

#[derive(Serialize)]
struct TrustDigestSubject<'a> {
    status: DomainPackTrustEvaluationStatus,
    trust_decisions: &'a [DomainPackLifecycleTrustDecision],
    verified_capability_bindings: &'a [DomainPackLockedCapabilityBinding],
    capability_gaps: &'a [DomainPackRuntimeCapabilityGap],
    issues: &'a [DomainPackTrustIssue],
}

/// Evaluates trust and executable capability bindings with default deny.
///
/// A binding is verified only when all of these observations match exactly:
/// selected package/version, package digest, subject ref, capability ref,
/// capability kind, `Available` status, `CoreBuiltin` provider, and sandbox
/// allowlist membership.  Every external provider is denied even if allowlisted.
#[must_use]
pub fn evaluate_domain_pack_trust(
    input: &DomainPackTrustEvaluationInput,
) -> DomainPackTrustEvaluation {
    let mut issues = Vec::new();
    let mut gaps = Vec::new();
    let mut decisions = Vec::new();
    let mut verified = Vec::new();

    let policy_is_default_deny =
        input.trust_policy.default_disposition == DomainPackTrustDisposition::Reject;
    if !policy_is_default_deny {
        issue(
            &mut issues,
            DomainPackTrustIssueCode::PolicyDefaultNotDeny,
            "trust_policy.default_disposition",
            "P6b trust policy must default to reject",
        );
    }
    if input.capability_registry.project_id != input.project_id {
        issue(
            &mut issues,
            DomainPackTrustIssueCode::ProjectMismatch,
            "capability_registry.project_id",
            "runtime capability registry belongs to a different project",
        );
    }

    let allowed_binding_ids: BTreeSet<_> = input
        .sandbox_policy
        .allowed_builtin_binding_ids
        .iter()
        .cloned()
        .collect();
    let mut selected = input.selected.iter().collect::<Vec<_>>();
    selected.sort_by_key(|selected| package_key(&selected.package));

    let mut selected_keys = BTreeSet::new();
    let mut accepted_dispositions = BTreeMap::new();
    let mut demands: BTreeMap<
        DemandKey,
        (&DomainPackCapabilityDemand, &DomainPackResolvedPackage),
    > = BTreeMap::new();

    for selected_package in selected {
        let package = &selected_package.package;
        let key = package_key(package);
        let path = package_path(&key);
        let duplicate = !selected_keys.insert(key.clone());
        if duplicate {
            issue(
                &mut issues,
                DomainPackTrustIssueCode::DuplicateSelectedPackage,
                &path,
                "selected package coordinate and version is duplicated",
            );
        }

        let content_digest = package.package.content.canonical_sha256.as_str();
        let (mut disposition, rule_ref, ambiguous) = select_rule(
            &input.trust_policy,
            package,
            &package.package.package_digest,
            content_digest,
        );
        if ambiguous {
            issue(
                &mut issues,
                DomainPackTrustIssueCode::AmbiguousTrustRule,
                format!("{path}.trust_rule"),
                "equally specific matching trust rules disagree",
            );
            disposition = DomainPackTrustDisposition::Reject;
        }

        let assessment_matches = selected_package.supply_chain.package_digest
            == package.package.package_digest
            && selected_package.supply_chain.registry_record_digest
                == package.registry_record_digest;
        if !selected_package.structurally_valid {
            issue(
                &mut issues,
                DomainPackTrustIssueCode::StructuralAssessmentFailed,
                format!("{path}.structurally_valid"),
                "package did not pass structural assessment",
            );
            disposition = DomainPackTrustDisposition::Reject;
        }
        if !assessment_matches {
            issue(
                &mut issues,
                DomainPackTrustIssueCode::SupplyChainAssessmentMismatch,
                format!("{path}.supply_chain"),
                "supply-chain assessment does not bind the selected package and registry record",
            );
            disposition = DomainPackTrustDisposition::Reject;
        }
        let supply_chain_verified = selected_package.supply_chain.publisher_signature_verified
            && selected_package
                .supply_chain
                .registry_signature_threshold_verified
            && selected_package.supply_chain.namespace_grant_verified
            && !selected_package.supply_chain.revoked;
        if !supply_chain_verified {
            issue(
                &mut issues,
                DomainPackTrustIssueCode::SupplyChainRejected,
                format!("{path}.supply_chain"),
                "package supply-chain assessment is incomplete or revoked",
            );
            disposition = DomainPackTrustDisposition::Reject;
        }
        if package.source_assurance < input.trust_policy.minimum_activation_assurance {
            issue(
                &mut issues,
                DomainPackTrustIssueCode::BelowMinimumAssurance,
                format!("{path}.source_assurance"),
                "selected package is below the trust policy minimum activation assurance",
            );
            disposition = DomainPackTrustDisposition::Reject;
        }
        if duplicate || !policy_is_default_deny {
            disposition = DomainPackTrustDisposition::Reject;
        }
        if disposition == DomainPackTrustDisposition::Reject {
            issue(
                &mut issues,
                DomainPackTrustIssueCode::TrustRejected,
                format!("{path}.disposition"),
                "selected package is rejected by trust policy",
            );
        }

        decisions.push(DomainPackLifecycleTrustDecision {
            package_digest: package.package.package_digest.clone(),
            disposition,
            rule_ref,
        });
        accepted_dispositions.insert(key.clone(), disposition);

        let mut package_demands = selected_package
            .capability_demands
            .iter()
            .collect::<Vec<_>>();
        package_demands.sort_by(|left, right| {
            left.subject_ref
                .cmp(&right.subject_ref)
                .then(left.capability_ref.cmp(&right.capability_ref))
                .then(format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
        });
        for demand in package_demands {
            let demand_key = DemandKey {
                package: key.clone(),
                subject_ref: demand.subject_ref.clone(),
                capability_ref: demand.capability_ref.clone(),
            };
            if demands
                .insert(demand_key.clone(), (demand, package))
                .is_some()
            {
                issue(
                    &mut issues,
                    DomainPackTrustIssueCode::DuplicateCapabilityDemand,
                    demand_path(&demand_key),
                    "capability demand is duplicated",
                );
            }
        }
    }

    let mut bindings = input
        .capability_registry
        .bindings
        .iter()
        .collect::<Vec<_>>();
    bindings.sort_by_key(|binding| binding_sort_key(binding));
    let mut binding_ids = BTreeSet::new();
    for binding in &bindings {
        if !binding_ids.insert(binding.binding_id.clone()) {
            issue(
                &mut issues,
                DomainPackTrustIssueCode::DuplicateRuntimeBinding,
                format!("capability_registry.bindings.{}", binding.binding_id.0),
                "runtime capability binding id is duplicated",
            );
        }
    }

    for (key, (demand, package)) in &demands {
        let exact_ref = package_ref_matches(&package.identity, &key.package);
        debug_assert!(exact_ref);
        let candidates = bindings
            .iter()
            .copied()
            .filter(|binding| {
                binding.subject_ref == demand.subject_ref
                    && binding.capability_ref == demand.capability_ref
            })
            .collect::<Vec<_>>();

        let disposition = accepted_dispositions
            .get(&key.package)
            .copied()
            .unwrap_or(DomainPackTrustDisposition::Reject);
        let outcome = evaluate_binding_candidates(
            key,
            demand,
            package,
            disposition,
            &candidates,
            &allowed_binding_ids,
        );
        match outcome {
            BindingOutcome::Allowed(binding, provider_id) => {
                verified.push(DomainPackLockedCapabilityBinding {
                    binding_id: binding.binding_id.clone(),
                    pack: binding.pack.clone(),
                    package_digest: binding.package_digest.clone(),
                    subject_ref: binding.subject_ref.clone(),
                    capability_ref: binding.capability_ref.clone(),
                    provider_id,
                    implementation_digest: binding.implementation_digest.clone(),
                    decision: DomainPackSandboxDecision::AllowedBoundBuiltin,
                });
            }
            BindingOutcome::Gap(code, message) => {
                gaps.push(gap(key, code, message));
                issue(
                    &mut issues,
                    DomainPackTrustIssueCode::CapabilityDenied,
                    demand_path(key),
                    "runtime capability demand is not satisfied by an exact allowed built-in",
                );
            }
        }
    }

    // Registry entries that were never demanded remain inert.  External
    // providers are called out explicitly even in this set so they can never be
    // mistaken for availability by a downstream projection.
    for binding in bindings {
        let key = DemandKey {
            package: PackageKey {
                publisher: binding.pack.publisher.clone(),
                name: binding.pack.name.clone(),
                version: binding.pack.version.clone(),
            },
            subject_ref: binding.subject_ref.clone(),
            capability_ref: binding.capability_ref.clone(),
        };
        if demands.contains_key(&key) {
            continue;
        }
        let code = if matches!(
            binding.provider,
            DomainPackRuntimeProvider::CoreBuiltin { .. }
        ) {
            DomainPackRuntimeCapabilityGapCode::UndeclaredBinding
        } else {
            DomainPackRuntimeCapabilityGapCode::ExternalProviderDenied
        };
        gaps.push(gap(
            &key,
            code,
            "runtime registry binding has no exact structurally assessed demand",
        ));
    }

    decisions.sort_by(|left, right| {
        left.package_digest
            .cmp(&right.package_digest)
            .then(left.rule_ref.cmp(&right.rule_ref))
    });
    verified.sort_by(|left, right| left.binding_id.cmp(&right.binding_id));
    gaps.sort_by_key(gap_sort_key);
    gaps.dedup();
    issues.sort();
    issues.dedup();

    let status = if issues.is_empty() && gaps.is_empty() {
        DomainPackTrustEvaluationStatus::Approved
    } else {
        DomainPackTrustEvaluationStatus::Blocked
    };
    let digest_subject = TrustDigestSubject {
        status,
        trust_decisions: &decisions,
        verified_capability_bindings: &verified,
        capability_gaps: &gaps,
        issues: &issues,
    };
    let evaluation_digest = canonical_digest(&digest_subject);

    DomainPackTrustEvaluation {
        status,
        trust_decisions: decisions,
        verified_capability_bindings: verified,
        capability_gaps: gaps,
        issues,
        evaluation_digest,
    }
}

fn select_rule(
    policy: &DomainPackTrustPolicy,
    package: &DomainPackResolvedPackage,
    package_digest: &str,
    content_digest: &str,
) -> (DomainPackTrustDisposition, StableId, bool) {
    let mut matches = policy
        .rules
        .iter()
        .filter(|rule| {
            rule.pack.publisher == package.identity.publisher
                && rule.pack.name == package.identity.name
                && rule
                    .package_digest
                    .as_ref()
                    .is_none_or(|digest| digest == package_digest)
                && rule
                    .content_digest
                    .as_ref()
                    .is_none_or(|digest| digest == content_digest)
        })
        .map(|rule| {
            let specificity =
                u8::from(rule.package_digest.is_some()) + u8::from(rule.content_digest.is_some());
            (specificity, rule)
        })
        .collect::<Vec<_>>();
    matches.sort_by(|(left_specificity, left), (right_specificity, right)| {
        right_specificity
            .cmp(left_specificity)
            .then(left.rule_id.cmp(&right.rule_id))
    });
    let Some((specificity, first)) = matches.first().copied() else {
        return (
            policy.default_disposition,
            StableId("domain-pack.trust.default-disposition".to_owned()),
            false,
        );
    };
    let ambiguous = matches
        .iter()
        .take_while(|(candidate_specificity, _)| *candidate_specificity == specificity)
        .any(|(_, rule)| rule.disposition != first.disposition);
    (first.disposition, first.rule_id.clone(), ambiguous)
}

enum BindingOutcome<'a> {
    Allowed(&'a DomainPackRuntimeCapabilityBinding, StableId),
    Gap(DomainPackRuntimeCapabilityGapCode, &'static str),
}

fn evaluate_binding_candidates<'a>(
    key: &DemandKey,
    demand: &DomainPackCapabilityDemand,
    package: &DomainPackResolvedPackage,
    disposition: DomainPackTrustDisposition,
    candidates: &[&'a DomainPackRuntimeCapabilityBinding],
    allowed_binding_ids: &BTreeSet<StableId>,
) -> BindingOutcome<'a> {
    if candidates.is_empty() {
        return BindingOutcome::Gap(
            DomainPackRuntimeCapabilityGapCode::MissingBinding,
            "no runtime registry binding exists for the exact subject and capability refs",
        );
    }
    let exact_pack = candidates
        .iter()
        .copied()
        .filter(|binding| version_ref_matches(&binding.pack, &key.package))
        .collect::<Vec<_>>();
    if exact_pack.is_empty() {
        return BindingOutcome::Gap(
            DomainPackRuntimeCapabilityGapCode::UndeclaredBinding,
            "runtime binding belongs to a different package coordinate or version",
        );
    }
    let exact_digest = exact_pack
        .iter()
        .copied()
        .filter(|binding| binding.package_digest == package.package.package_digest)
        .collect::<Vec<_>>();
    if exact_digest.is_empty() {
        return BindingOutcome::Gap(
            DomainPackRuntimeCapabilityGapCode::PackageDigestMismatch,
            "runtime binding package digest does not match the selected package",
        );
    }
    let exact_kind = exact_digest
        .iter()
        .copied()
        .filter(|binding| binding.kind == demand.kind)
        .collect::<Vec<_>>();
    if exact_kind.is_empty() {
        return BindingOutcome::Gap(
            DomainPackRuntimeCapabilityGapCode::KindMismatch,
            "runtime binding capability kind does not match the structural demand",
        );
    }
    if let Some(binding) = exact_kind
        .iter()
        .copied()
        .find(|binding| binding.status == DomainPackRuntimeCapabilityStatus::Revoked)
    {
        let _ = binding;
        return BindingOutcome::Gap(
            DomainPackRuntimeCapabilityGapCode::Revoked,
            "runtime binding is revoked",
        );
    }
    if exact_kind
        .iter()
        .any(|binding| binding.status == DomainPackRuntimeCapabilityStatus::Disabled)
    {
        return BindingOutcome::Gap(
            DomainPackRuntimeCapabilityGapCode::Disabled,
            "runtime binding is disabled",
        );
    }
    if exact_kind
        .iter()
        .all(|binding| binding.status != DomainPackRuntimeCapabilityStatus::Available)
    {
        return BindingOutcome::Gap(
            DomainPackRuntimeCapabilityGapCode::Unavailable,
            "runtime binding is unavailable",
        );
    }
    let available = exact_kind
        .into_iter()
        .filter(|binding| binding.status == DomainPackRuntimeCapabilityStatus::Available)
        .collect::<Vec<_>>();
    if available.iter().any(|binding| {
        !matches!(
            binding.provider,
            DomainPackRuntimeProvider::CoreBuiltin { .. }
        )
    }) {
        return BindingOutcome::Gap(
            DomainPackRuntimeCapabilityGapCode::ExternalProviderDenied,
            "external runtime providers are denied in P6b",
        );
    }
    if disposition != DomainPackTrustDisposition::ActivateDeclarativeKnowledgeAndBoundBuiltIns {
        return BindingOutcome::Gap(
            DomainPackRuntimeCapabilityGapCode::UndeclaredBinding,
            "trust disposition does not permit bound built-in execution",
        );
    }
    let Some(binding) = available
        .into_iter()
        .filter(|binding| allowed_binding_ids.contains(&binding.binding_id))
        .min_by_key(|binding| binding.binding_id.clone())
    else {
        return BindingOutcome::Gap(
            DomainPackRuntimeCapabilityGapCode::UndeclaredBinding,
            "exact built-in binding is not allowlisted by the sandbox policy",
        );
    };
    let DomainPackRuntimeProvider::CoreBuiltin { provider_id } = &binding.provider else {
        unreachable!("available candidates were restricted to built-ins")
    };
    BindingOutcome::Allowed(binding, provider_id.clone())
}

fn package_key(package: &DomainPackResolvedPackage) -> PackageKey {
    PackageKey {
        publisher: package.identity.publisher.clone(),
        name: package.identity.name.clone(),
        version: package.identity.version.clone(),
    }
}

fn package_ref_matches(
    identity: &forge_core_contracts::DomainPackIdentity,
    key: &PackageKey,
) -> bool {
    identity.publisher == key.publisher
        && identity.name == key.name
        && identity.version == key.version
}

fn version_ref_matches(
    version_ref: &forge_core_contracts::DomainPackVersionReference,
    key: &PackageKey,
) -> bool {
    version_ref.publisher == key.publisher
        && version_ref.name == key.name
        && version_ref.version == key.version
}

fn package_path(key: &PackageKey) -> String {
    format!(
        "selected.{}.{}/{}",
        key.publisher.0, key.name.0, key.version
    )
}

fn demand_path(key: &DemandKey) -> String {
    format!(
        "{}.capability_demands.{}.{}",
        package_path(&key.package),
        key.subject_ref.0,
        key.capability_ref.0
    )
}

fn binding_sort_key(
    binding: &DomainPackRuntimeCapabilityBinding,
) -> (StableId, StableId, StableId) {
    (
        binding.binding_id.clone(),
        binding.subject_ref.clone(),
        binding.capability_ref.clone(),
    )
}

fn gap(
    key: &DemandKey,
    code: DomainPackRuntimeCapabilityGapCode,
    message: impl Into<String>,
) -> DomainPackRuntimeCapabilityGap {
    DomainPackRuntimeCapabilityGap {
        code,
        pack: forge_core_contracts::DomainPackVersionReference {
            publisher: key.package.publisher.clone(),
            name: key.package.name.clone(),
            version: key.package.version.clone(),
        },
        subject_ref: key.subject_ref.clone(),
        capability_ref: key.capability_ref.clone(),
        message: message.into(),
    }
}

fn gap_sort_key(
    gap: &DomainPackRuntimeCapabilityGap,
) -> (StableId, StableId, String, StableId, StableId, String) {
    (
        gap.pack.publisher.clone(),
        gap.pack.name.clone(),
        gap.pack.version.clone(),
        gap.subject_ref.clone(),
        gap.capability_ref.clone(),
        format!("{:?}", gap.code),
    )
}

fn issue(
    issues: &mut Vec<DomainPackTrustIssue>,
    code: DomainPackTrustIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(DomainPackTrustIssue {
        code,
        path: path.into(),
        message: message.into(),
    });
}

fn canonical_digest<T: Serialize>(value: &T) -> String {
    serde_json_canonicalizer::to_vec(value).map_or_else(
        |_| {
            format!(
                "sha256:{:x}",
                Sha256::digest(b"domain-pack-trust-encoding-failed")
            )
        },
        |bytes| format!("sha256:{:x}", Sha256::digest(bytes)),
    )
}
