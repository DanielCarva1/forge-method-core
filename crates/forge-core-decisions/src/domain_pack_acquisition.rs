//! Pure P7c reviewed-package acquisition selection planning.
//!
//! This module proves only that an exact candidate came from the presented
//! current discovery projection. The resulting plan always requires the P6
//! trust and lifecycle ceremonies and grants no mutation authority.

use crate::{
    discover_domain_packs, domain_pack_resolution_projection_digest, resolve_domain_packs,
    verify_domain_pack_discovery_projection, MAX_DOMAIN_PACK_CANDIDATES,
    MAX_DOMAIN_PACK_DISCOVERY_CAPABILITIES_PER_REQUIREMENT, MAX_DOMAIN_PACK_DISCOVERY_REQUIREMENTS,
};
use forge_core_contracts::{
    DomainPackAcquisitionCeremony, DomainPackAcquisitionDerivationInput,
    DomainPackAcquisitionDerivedInputs, DomainPackAcquisitionDerivedInputsDocument,
    DomainPackAcquisitionPlan, DomainPackAcquisitionPlanDocument, DomainPackAcquisitionPlanStatus,
    DomainPackAcquisitionPlanningInput, DomainPackCandidateAuthority, DomainPackCompositionRequest,
    DomainPackCompositionRequestDocument, DomainPackDependencySourcePolicy,
    DomainPackDuplicateVersionPolicy, DomainPackPrereleasePolicy,
    DomainPackProjectRequirementsDocument, DomainPackResolutionCandidate,
    DomainPackResolutionPolicy, DomainPackResolutionRequest, DomainPackResolutionRequestDocument,
    DomainPackResolutionRoot, DomainPackResolutionRootReason, DomainPackResolutionStatus,
    DomainPackSemanticAssurance, DomainPackSourceAssurance, DomainPackUnrelatedUpdatePolicy,
    DomainPackVersionSelectionPolicy, DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION,
    DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION, DOMAIN_PACK_SCHEMA_VERSION,
};
use semver::{Version, VersionReq};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DomainPackAcquisitionIssueCode {
    UnsupportedSchemaVersion,
    InvalidStableId,
    InvalidDigest,
    InvalidDiscoveryRequest,
    InvalidDiscoveryProjection,
    DiscoveryReplayMismatch,
    StaleDiscoveryBinding,
    CandidateAbsent,
    CandidateAmbiguous,
    RequirementBlocked,
    InvalidAcquisitionPlan,
    CandidateMaterialAbsent,
    CandidateMaterialAmbiguous,
    CandidateMaterialMismatch,
    ResolutionBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackAcquisitionIssue {
    pub code: DomainPackAcquisitionIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackAcquisitionRejection {
    pub issues: Vec<DomainPackAcquisitionIssue>,
}

/// Bind an exact selected candidate to the presented discovery state.
///
/// # Errors
/// Returns deterministic typed issues for stale, ambiguous, malformed, or
/// blocked selections.
pub fn plan_domain_pack_acquisition(
    input: &DomainPackAcquisitionPlanningInput,
) -> Result<DomainPackAcquisitionPlanDocument, DomainPackAcquisitionRejection> {
    let intent_document = &input.intent;
    let intent = &intent_document.domain_pack_acquisition_intent;
    let discovery = &input.discovery.domain_pack_discovery_projection;
    let mut issues = Vec::new();
    if intent_document.schema_version != DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION {
        push(
            &mut issues,
            DomainPackAcquisitionIssueCode::UnsupportedSchemaVersion,
            "intent.schema_version",
            "unsupported acquisition schema version",
        );
    }
    for (path, value) in [
        ("intent.acquisition_id", intent.acquisition_id.0.as_str()),
        ("intent.candidate_id", intent.candidate_id.0.as_str()),
        ("intent.requirement_ref", intent.requirement_ref.0.as_str()),
    ] {
        if !valid_id(value) {
            push(
                &mut issues,
                DomainPackAcquisitionIssueCode::InvalidStableId,
                path,
                "invalid stable id",
            );
        }
    }
    for (path, value) in [
        (
            "intent.discovery_projection_digest",
            intent.discovery_projection_digest.as_str(),
        ),
        ("intent.demand_digest", intent.demand_digest.as_str()),
        (
            "intent.expected_project_snapshot_digest",
            intent.expected_project_snapshot_digest.as_str(),
        ),
    ] {
        if !valid_digest(value) {
            push(
                &mut issues,
                DomainPackAcquisitionIssueCode::InvalidDigest,
                path,
                "expected sha256:<64 lowercase hex>",
            );
        }
    }
    if !verify_domain_pack_discovery_projection(&input.discovery) {
        return Err(single_issue(
            DomainPackAcquisitionIssueCode::InvalidDiscoveryProjection,
            "discovery",
            "discovery projection failed integrity or semantic validation",
        ));
    }
    let replayed_discovery = discover_domain_packs(&input.request).map_err(|rejection| {
        let detail = rejection.issues.first().map_or_else(
            || "unknown discovery request error".to_owned(),
            |issue| format!("{}: {}", issue.path, issue.message),
        );
        single_issue(
            DomainPackAcquisitionIssueCode::InvalidDiscoveryRequest,
            "request",
            format!("discovery request failed replay: {detail}"),
        )
    })?;
    if replayed_discovery != input.discovery {
        return Err(single_issue(
            DomainPackAcquisitionIssueCode::DiscoveryReplayMismatch,
            "discovery",
            "presented projection is not the exact deterministic result of the presented request",
        ));
    }
    if intent.authority != DomainPackCandidateAuthority::CandidateOnly
        || intent.assurance_binding != discovery.assurance_binding
        || intent.discovery_projection_digest != discovery.projection_digest
        || intent.demand_digest != discovery.demand_digest
        || intent.expected_project_snapshot_digest != discovery.assurance_binding.snapshot_digest
    {
        push(
            &mut issues,
            DomainPackAcquisitionIssueCode::StaleDiscoveryBinding,
            "intent",
            "selection does not bind the exact presented discovery state",
        );
    }
    if discovery
        .gaps
        .iter()
        .any(|gap| gap.requirement_ref == intent.requirement_ref)
    {
        push(
            &mut issues,
            DomainPackAcquisitionIssueCode::RequirementBlocked,
            "intent.requirement_ref",
            "a blocked discovery requirement cannot be acquired",
        );
    }
    let selected = discovery
        .matches
        .iter()
        .filter(|candidate| {
            candidate.candidate_id == intent.candidate_id
                && candidate.requirement_ref == intent.requirement_ref
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        push(
            &mut issues,
            DomainPackAcquisitionIssueCode::CandidateAbsent,
            "intent.candidate_id",
            "candidate is absent from the exact discovery projection",
        );
    } else if selected.len() > 1 {
        push(
            &mut issues,
            DomainPackAcquisitionIssueCode::CandidateAmbiguous,
            "intent.candidate_id",
            "candidate occurs more than once for the selected requirement",
        );
    }
    if !issues.is_empty() {
        issues.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.message.cmp(&right.message))
        });
        return Err(DomainPackAcquisitionRejection { issues });
    }

    let Some(selected) = selected.into_iter().next() else {
        return Err(DomainPackAcquisitionRejection {
            issues: vec![DomainPackAcquisitionIssue {
                code: DomainPackAcquisitionIssueCode::CandidateAbsent,
                path: "intent.candidate_id".to_owned(),
                message: "candidate is absent from the exact discovery projection".to_owned(),
            }],
        });
    };
    let mut plan = DomainPackAcquisitionPlan {
        acquisition_id: intent.acquisition_id.clone(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
        assurance_binding: intent.assurance_binding.clone(),
        discovery_request_digest: canonical_digest(&input.request).map_err(|message| {
            single_issue(
                DomainPackAcquisitionIssueCode::InvalidDigest,
                "request",
                message,
            )
        })?,
        discovery_projection_digest: intent.discovery_projection_digest.clone(),
        demand_digest: intent.demand_digest.clone(),
        requirements: input
            .request
            .domain_pack_discovery_request
            .requirements
            .clone(),
        selected: selected.clone(),
        operation: intent.operation,
        expected_project_snapshot_digest: intent.expected_project_snapshot_digest.clone(),
        status: DomainPackAcquisitionPlanStatus::TrustCeremonyRequired,
        required_ceremonies: required_ceremonies(),
        plan_digest: String::new(),
    };
    plan.plan_digest =
        canonical_digest(&plan).map_err(|message| DomainPackAcquisitionRejection {
            issues: vec![DomainPackAcquisitionIssue {
                code: DomainPackAcquisitionIssueCode::InvalidDigest,
                path: "plan".to_owned(),
                message,
            }],
        })?;
    let document = DomainPackAcquisitionPlanDocument {
        schema_version: DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_acquisition_plan: plan,
    };
    if !verify_domain_pack_acquisition_plan(&document) {
        return Err(single_issue(
            DomainPackAcquisitionIssueCode::InvalidDigest,
            "plan",
            "derived acquisition plan failed its own integrity invariants",
        ));
    }
    Ok(document)
}

/// Derive deterministic P6 resolver and composer inputs for an exact install
/// candidate without granting trust or lifecycle authority.
///
/// # Errors
/// Returns fail-closed issues when the plan is invalid, package material does
/// not match the selected candidate, or the existing resolver blocks it.
pub fn derive_domain_pack_acquisition_inputs(
    input: &DomainPackAcquisitionDerivationInput,
) -> Result<DomainPackAcquisitionDerivedInputsDocument, DomainPackAcquisitionRejection> {
    if !verify_domain_pack_acquisition_plan(&input.plan) {
        return Err(single_issue(
            DomainPackAcquisitionIssueCode::InvalidAcquisitionPlan,
            "plan",
            "acquisition plan failed integrity or semantic validation",
        ));
    }
    let replayed_plan = plan_domain_pack_acquisition(&input.planning_input).map_err(|_| {
        single_issue(
            DomainPackAcquisitionIssueCode::InvalidAcquisitionPlan,
            "planning_input",
            "original discovery planning input no longer produces an admissible plan",
        )
    })?;
    if replayed_plan != input.plan {
        return Err(single_issue(
            DomainPackAcquisitionIssueCode::InvalidAcquisitionPlan,
            "planning_input",
            "original discovery state and selection intent do not reproduce the exact acquisition plan",
        ));
    }
    if !derivation_input_within_limits(input) {
        return Err(single_issue(
            DomainPackAcquisitionIssueCode::CandidateMaterialMismatch,
            "candidates",
            format!(
                "acquisition material exceeds bounded package or registry limits (maximum {MAX_DOMAIN_PACK_CANDIDATES} per collection)"
            ),
        ));
    }
    validate_candidate_materials(&input.candidates)?;
    let plan = &input.plan.domain_pack_acquisition_plan;
    let selected = &plan.selected;
    let coordinate_candidates = input
        .candidates
        .iter()
        .filter(|candidate| {
            let identity = &candidate.input.manifest.domain_pack_manifest.identity;
            identity.publisher == selected.pack.publisher
                && identity.name == selected.pack.name
                && identity.version == selected.pack.version
        })
        .collect::<Vec<_>>();
    let candidate = match coordinate_candidates.as_slice() {
        [] => {
            return Err(single_issue(
                DomainPackAcquisitionIssueCode::CandidateMaterialAbsent,
                "candidates",
                "selected coordinate and version are absent from package material",
            ));
        }
        [candidate] => *candidate,
        _ => {
            return Err(single_issue(
                DomainPackAcquisitionIssueCode::CandidateMaterialAmbiguous,
                "candidates",
                "selected coordinate and version occur more than once in package material",
            ));
        }
    };
    let content_digest = canonical_digest(&candidate.input.content).map_err(|message| {
        single_issue(
            DomainPackAcquisitionIssueCode::CandidateMaterialMismatch,
            "candidates.selected.content",
            message,
        )
    })?;
    if candidate.package.package_digest != selected.package_digest
        || candidate.package.content.canonical_sha256 != selected.content_digest
        || candidate
            .registry_record_digest
            .as_deref()
            .is_none_or(|digest| digest != selected.supply_chain_record_digest)
        || content_digest != selected.content_digest
    {
        return Err(single_issue(
            DomainPackAcquisitionIssueCode::CandidateMaterialMismatch,
            "candidates.selected",
            "selected package, content, or registry record digest does not match the acquisition plan",
        ));
    }
    let Some(requirement) = plan
        .requirements
        .required_domains
        .iter()
        .find(|requirement| requirement.id == selected.requirement_ref)
    else {
        return Err(single_issue(
            DomainPackAcquisitionIssueCode::InvalidAcquisitionPlan,
            "plan.requirements",
            "selected requirement is absent from the acquisition plan",
        ));
    };
    let root = DomainPackResolutionRoot {
        pack: forge_core_contracts::DomainPackCoordinate {
            publisher: selected.pack.publisher.clone(),
            name: selected.pack.name.clone(),
        },
        version_requirement: requirement.pack_version_requirement.clone(),
        required_content_digest: Some(selected.content_digest.clone()),
        reason: DomainPackResolutionRootReason::InstallIntent,
    };
    let roots = vec![root];
    let candidates = sorted_candidates(&input.candidates).map_err(|message| {
        single_issue(
            DomainPackAcquisitionIssueCode::CandidateMaterialMismatch,
            "candidates",
            message,
        )
    })?;
    let resolution_request = DomainPackResolutionRequestDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_resolution_request: DomainPackResolutionRequest {
            request_id: derived_id("resolution", &plan.acquisition_id.0),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            project_id: plan.requirements.project_id.clone(),
            forge_core_version: input.forge_core_version.clone(),
            core: input.core.clone(),
            requirements: DomainPackProjectRequirementsDocument {
                schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
                domain_pack_project_requirements: plan.requirements.clone(),
            },
            roots,
            current_lock: None,
            policy: DomainPackResolutionPolicy {
                selection: DomainPackVersionSelectionPolicy::MinimalChangeThenHighestCompatible,
                prerelease: DomainPackPrereleasePolicy::ExplicitOnly,
                duplicate_version: DomainPackDuplicateVersionPolicy::RejectDivergentContent,
                dependency_source: DomainPackDependencySourcePolicy::ExactPublisherOnly,
                unrelated_updates: DomainPackUnrelatedUpdatePolicy::PreserveLocked,
            },
            registry_snapshot_digest: input
                .registry
                .domain_pack_supply_chain_registry
                .snapshot_digest
                .clone(),
            candidates: candidates.clone(),
        },
    };
    let resolution_projection = resolve_domain_packs(&resolution_request, &input.registry);
    let resolution = &resolution_projection.domain_pack_resolution_projection;
    if resolution.status != DomainPackResolutionStatus::Resolved
        || !resolution.rejected.is_empty()
        || resolution.selected.iter().any(|resolved| {
            resolved.source_assurance != DomainPackSourceAssurance::ExplicitlyUntrusted
                || resolved.semantic_assurance != DomainPackSemanticAssurance::Unreviewed
                || resolved.reviewed_entry_digest.is_some()
                || resolved.promotion_authorization_digest.is_some()
        })
        || !resolution.selected.iter().any(|resolved| {
            resolved.identity.publisher == selected.pack.publisher
                && resolved.identity.name == selected.pack.name
                && resolved.identity.version == selected.pack.version
                && resolved.package.package_digest == selected.package_digest
        })
    {
        let details = resolution
            .issues
            .iter()
            .map(|issue| format!("{}: {}", issue.path, issue.message))
            .chain(resolution.rejected.iter().map(|rejected| {
                format!(
                    "rejected {}::{}@{}: {:?}",
                    rejected.identity.publisher.0,
                    rejected.identity.name.0,
                    rejected.identity.version,
                    rejected.reasons
                )
            }))
            .collect::<Vec<_>>();
        let detail = if details.is_empty() {
            "selected package was not resolved".to_owned()
        } else {
            details.join("; ")
        };
        return Err(single_issue(
            DomainPackAcquisitionIssueCode::ResolutionBlocked,
            "resolution",
            detail,
        ));
    }
    let mut composition_candidates = Vec::with_capacity(resolution.selected.len());
    for resolved in &resolution.selected {
        let matching_material = candidates
            .iter()
            .filter(|candidate| {
                candidate.input.manifest.domain_pack_manifest.identity == resolved.identity
                    && candidate.package == resolved.package
                    && candidate.registry_record_digest.as_deref()
                        == Some(resolved.registry_record_digest.as_str())
            })
            .collect::<Vec<_>>();
        let [candidate] = matching_material.as_slice() else {
            return Err(single_issue(
                DomainPackAcquisitionIssueCode::CandidateMaterialMismatch,
                "resolution.selected",
                "resolved package does not join uniquely to exact candidate and registry material",
            ));
        };
        composition_candidates.push(candidate.input.clone());
    }
    let composition_request = DomainPackCompositionRequestDocument {
        schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
        domain_pack_composition_request: DomainPackCompositionRequest {
            request_id: derived_id("composition", &plan.acquisition_id.0),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            forge_core_version: input.forge_core_version.clone(),
            core: input.core.clone(),
            requirements: plan.requirements.clone(),
            candidates: composition_candidates,
        },
    };
    let mut derived = DomainPackAcquisitionDerivedInputs {
        acquisition_id: plan.acquisition_id.clone(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
        acquisition_plan_digest: plan.plan_digest.clone(),
        derivation_input_digest: canonical_derivation_input_digest(input).map_err(|message| {
            single_issue(
                DomainPackAcquisitionIssueCode::InvalidDigest,
                "derivation_input",
                message,
            )
        })?,
        resolution_request,
        resolution_projection,
        composition_request,
        derivation_digest: String::new(),
    };
    derived.derivation_digest = canonical_digest(&derived).map_err(|message| {
        single_issue(
            DomainPackAcquisitionIssueCode::InvalidDigest,
            "derived_inputs",
            message,
        )
    })?;
    let document = DomainPackAcquisitionDerivedInputsDocument {
        schema_version: DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION.to_owned(),
        domain_pack_acquisition_derived_inputs: derived,
    };
    if !verify_domain_pack_acquisition_derived_inputs(&document, input) {
        return Err(single_issue(
            DomainPackAcquisitionIssueCode::InvalidDigest,
            "derived_inputs",
            "derived P6 inputs failed their own integrity invariants",
        ));
    }
    Ok(document)
}

/// Verify persisted candidate-only P6 inputs before a host proceeds to trust
/// and lifecycle derivation.
#[must_use]
pub fn verify_domain_pack_acquisition_derived_inputs(
    document: &DomainPackAcquisitionDerivedInputsDocument,
    input: &DomainPackAcquisitionDerivationInput,
) -> bool {
    if !derivation_input_within_limits(input) || !verify_domain_pack_acquisition_plan(&input.plan) {
        return false;
    }
    if validate_candidate_materials(&input.candidates).is_err()
        || plan_domain_pack_acquisition(&input.planning_input).as_ref() != Ok(&input.plan)
    {
        return false;
    }
    let derived = &document.domain_pack_acquisition_derived_inputs;
    let plan = &input.plan.domain_pack_acquisition_plan;
    let resolution_request = &derived.resolution_request.domain_pack_resolution_request;
    let resolution = &derived
        .resolution_projection
        .domain_pack_resolution_projection;
    let composition = &derived.composition_request.domain_pack_composition_request;
    if document.schema_version != DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION
        || derived.authority != DomainPackCandidateAuthority::CandidateOnly
        || derived.acquisition_id != plan.acquisition_id
        || derived.acquisition_plan_digest != plan.plan_digest
        || !canonical_derivation_input_digest(input)
            .is_ok_and(|digest| digest == derived.derivation_input_digest)
        || !valid_digest(&derived.derivation_input_digest)
        || !valid_digest(&derived.derivation_digest)
        || derived.resolution_request.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION
        || derived.composition_request.schema_version != DOMAIN_PACK_SCHEMA_VERSION
        || resolution_request.request_id != derived_id("resolution", &plan.acquisition_id.0)
        || composition.request_id != derived_id("composition", &plan.acquisition_id.0)
        || resolution_request.authority != DomainPackCandidateAuthority::CandidateOnly
        || composition.authority != DomainPackCandidateAuthority::CandidateOnly
        || resolution.authority != DomainPackCandidateAuthority::CandidateOnly
        || resolution.status != DomainPackResolutionStatus::Resolved
        || !resolution.rejected.is_empty()
        || resolution.selected.len() > MAX_DOMAIN_PACK_CANDIDATES
        || resolution.selected.iter().any(|resolved| {
            resolved.source_assurance != DomainPackSourceAssurance::ExplicitlyUntrusted
                || resolved.semantic_assurance != DomainPackSemanticAssurance::Unreviewed
                || resolved.reviewed_entry_digest.is_some()
                || resolved.promotion_authorization_digest.is_some()
        })
        || resolution_request.current_lock.is_some()
        || resolution_request.project_id != plan.requirements.project_id
        || resolution_request.project_id != composition.requirements.project_id
        || resolution_request
            .requirements
            .domain_pack_project_requirements
            != plan.requirements
        || composition.requirements != plan.requirements
        || resolution_request.forge_core_version != input.forge_core_version
        || composition.forge_core_version != input.forge_core_version
        || resolution_request.core != input.core
        || composition.core != input.core
        || resolution_request.registry_snapshot_digest
            != input
                .registry
                .domain_pack_supply_chain_registry
                .snapshot_digest
        || resolution.resolution_digest
            != domain_pack_resolution_projection_digest(
                &derived.resolution_request,
                &resolution_request.registry_snapshot_digest,
                resolution,
            )
    {
        return false;
    }
    let Ok(expected_candidates) = sorted_candidates(&input.candidates) else {
        return false;
    };
    if resolution_request.candidates != expected_candidates {
        return false;
    }
    let Some(requirement) = plan
        .requirements
        .required_domains
        .iter()
        .find(|requirement| requirement.id == plan.selected.requirement_ref)
    else {
        return false;
    };
    let expected_root = DomainPackResolutionRoot {
        pack: forge_core_contracts::DomainPackCoordinate {
            publisher: plan.selected.pack.publisher.clone(),
            name: plan.selected.pack.name.clone(),
        },
        version_requirement: requirement.pack_version_requirement.clone(),
        required_content_digest: Some(plan.selected.content_digest.clone()),
        reason: DomainPackResolutionRootReason::InstallIntent,
    };
    if resolution_request.roots != [expected_root]
        || resolve_domain_packs(&derived.resolution_request, &input.registry)
            != derived.resolution_projection
    {
        return false;
    }
    let expected_composition_candidates = resolution
        .selected
        .iter()
        .map(|resolved| {
            expected_candidates.iter().find(|candidate| {
                candidate.input.manifest.domain_pack_manifest.identity == resolved.identity
                    && candidate.package == resolved.package
                    && candidate.registry_record_digest.as_deref()
                        == Some(resolved.registry_record_digest.as_str())
            })
        })
        .collect::<Option<Vec<_>>>();
    if expected_composition_candidates.is_none_or(|expected| {
        expected
            .into_iter()
            .map(|candidate| &candidate.input)
            .ne(composition.candidates.iter())
    }) {
        return false;
    }
    let mut digest_subject = derived.clone();
    let claimed_digest = std::mem::take(&mut digest_subject.derivation_digest);
    canonical_digest(&digest_subject).is_ok_and(|actual| actual == claimed_digest)
}

fn derivation_input_within_limits(input: &DomainPackAcquisitionDerivationInput) -> bool {
    let registry = &input.registry.domain_pack_supply_chain_registry;
    !input.candidates.is_empty()
        && input.candidates.len() <= MAX_DOMAIN_PACK_CANDIDATES
        && registry.packages.len() <= MAX_DOMAIN_PACK_CANDIDATES
        && registry.publisher_credentials.len() <= MAX_DOMAIN_PACK_CANDIDATES
        && registry.namespace_grants.len() <= MAX_DOMAIN_PACK_CANDIDATES
        && registry.revocations.len() <= MAX_DOMAIN_PACK_CANDIDATES
        && registry.signatures.len() <= MAX_DOMAIN_PACK_CANDIDATES
}

fn validate_candidate_materials(
    candidates: &[DomainPackResolutionCandidate],
) -> Result<(), DomainPackAcquisitionRejection> {
    for (index, candidate) in candidates.iter().enumerate() {
        let manifest_digest = canonical_digest(&candidate.input.manifest).map_err(|message| {
            single_issue(
                DomainPackAcquisitionIssueCode::CandidateMaterialMismatch,
                format!("candidates[{index}].manifest"),
                message,
            )
        })?;
        let content_digest = canonical_digest(&candidate.input.content).map_err(|message| {
            single_issue(
                DomainPackAcquisitionIssueCode::CandidateMaterialMismatch,
                format!("candidates[{index}].content"),
                message,
            )
        })?;
        let manifest = &candidate.input.manifest.domain_pack_manifest;
        let content = &candidate.input.content.domain_pack_content;
        let expected_fixtures = content
            .fixtures
            .iter()
            .map(|fixture| fixture.artifact.clone())
            .collect::<Vec<_>>();
        if manifest_digest != candidate.input.manifest_binding.canonical_sha256
            || candidate.package.manifest != candidate.input.manifest_binding
            || content_digest != manifest.content.canonical_sha256
            || candidate.package.content != manifest.content
            || candidate.package.license != manifest.provenance.license_text
            || candidate.package.fixtures != expected_fixtures
            || content.pack.publisher != manifest.identity.publisher
            || content.pack.name != manifest.identity.name
            || content.pack.version != manifest.identity.version
            || content.namespace != manifest.identity.namespace
        {
            return Err(single_issue(
                DomainPackAcquisitionIssueCode::CandidateMaterialMismatch,
                format!("candidates[{index}]"),
                "candidate manifest, content, package bindings, or identity equivocate",
            ));
        }
    }
    Ok(())
}

fn sorted_candidates(
    candidates: &[DomainPackResolutionCandidate],
) -> Result<Vec<DomainPackResolutionCandidate>, String> {
    let mut indexed = candidates
        .iter()
        .map(|candidate| canonical_digest(candidate).map(|digest| (digest, candidate.clone())))
        .collect::<Result<Vec<_>, _>>()?;
    indexed.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(indexed
        .into_iter()
        .map(|(_, candidate)| candidate)
        .collect())
}

fn canonical_derivation_input_digest(
    input: &DomainPackAcquisitionDerivationInput,
) -> Result<String, String> {
    let mut normalized = input.clone();
    normalized.candidates = sorted_candidates(&input.candidates)?;
    canonical_digest(&normalized)
}

fn derived_id(kind: &str, acquisition_id: &str) -> forge_core_contracts::StableId {
    let mut hasher = Sha256::new();
    hasher.update((acquisition_id.len() as u64).to_be_bytes());
    hasher.update(acquisition_id.as_bytes());
    forge_core_contracts::StableId(format!("acquisition.{kind}.{:x}", hasher.finalize()))
}

/// Verify a persisted candidate-only acquisition plan before any derivation or
/// lifecycle orchestration consumes it.
#[must_use]
pub fn verify_domain_pack_acquisition_plan(document: &DomainPackAcquisitionPlanDocument) -> bool {
    let plan = &document.domain_pack_acquisition_plan;
    if document.schema_version != DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION
        || plan.authority != DomainPackCandidateAuthority::CandidateOnly
        || plan.status != DomainPackAcquisitionPlanStatus::TrustCeremonyRequired
        || plan.required_ceremonies != required_ceremonies()
        || !valid_id(&plan.acquisition_id.0)
        || !valid_id(&plan.assurance_binding.project_id.0)
        || !valid_id(&plan.assurance_binding.intent_id.0)
        || plan.assurance_binding.assurance_epoch == 0
        || plan.assurance_binding.intent_revision == 0
        || plan.assurance_binding.accepted_sequence == 0
        || plan.assurance_binding.accepted_state_version == 0
        || !valid_digest(&plan.discovery_request_digest)
        || !valid_digest(&plan.discovery_projection_digest)
        || !valid_digest(&plan.demand_digest)
        || !valid_digest(&plan.expected_project_snapshot_digest)
        || !valid_digest(&plan.plan_digest)
        || !valid_digest(&plan.assurance_binding.intent_digest)
        || !valid_digest(&plan.assurance_binding.accepted_record_digest)
        || !valid_digest(&plan.assurance_binding.snapshot_digest)
        || !valid_digest(&plan.assurance_binding.ledger_head_before_acceptance)
        || plan.expected_project_snapshot_digest != plan.assurance_binding.snapshot_digest
        || plan.requirements.project_id != plan.assurance_binding.project_id
        || !valid_id(&plan.requirements.requirement_set_id.0)
        || plan.requirements.required_domains.is_empty()
        || plan.requirements.required_domains.len() > MAX_DOMAIN_PACK_DISCOVERY_REQUIREMENTS
        || plan
            .requirements
            .required_domains
            .iter()
            .any(|requirement| {
                !valid_id(&requirement.id.0)
                    || !valid_id(&requirement.domain_id.0)
                    || VersionReq::parse(&requirement.pack_version_requirement).is_err()
                    || requirement.required_capability_refs.len()
                        > MAX_DOMAIN_PACK_DISCOVERY_CAPABILITIES_PER_REQUIREMENT
                    || requirement
                        .required_capability_refs
                        .iter()
                        .any(|capability| !valid_id(&capability.0))
            })
        || !valid_id(&plan.selected.candidate_id.0)
        || !valid_id(&plan.selected.requirement_ref.0)
        || !valid_id(&plan.selected.domain_id.0)
        || !valid_id(&plan.selected.pack.publisher.0)
        || !valid_id(&plan.selected.pack.name.0)
        || Version::parse(&plan.selected.pack.version).is_err()
        || !valid_digest(&plan.selected.package_digest)
        || !valid_digest(&plan.selected.supply_chain_record_digest)
        || !valid_raw_digest(&plan.selected.reviewed_entry_digest)
        || !valid_digest(&plan.selected.content_digest)
        || plan
            .selected
            .matched_capability_refs
            .iter()
            .any(|capability| !valid_id(&capability.0))
    {
        return false;
    }
    let requirement_ids = plan
        .requirements
        .required_domains
        .iter()
        .map(|requirement| &requirement.id)
        .collect::<BTreeSet<_>>();
    if requirement_ids.len() != plan.requirements.required_domains.len()
        || plan
            .requirements
            .required_domains
            .iter()
            .any(|requirement| {
                requirement
                    .required_capability_refs
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != requirement.required_capability_refs.len()
            })
    {
        return false;
    }
    let matching_requirements = plan
        .requirements
        .required_domains
        .iter()
        .filter(|requirement| requirement.id == plan.selected.requirement_ref)
        .collect::<Vec<_>>();
    if matching_requirements.len() != 1 {
        return false;
    }
    let requirement = matching_requirements[0];
    let mut required_capabilities = requirement.required_capability_refs.clone();
    required_capabilities.sort();
    let selected_version_matches = Version::parse(&plan.selected.pack.version)
        .ok()
        .zip(VersionReq::parse(&requirement.pack_version_requirement).ok())
        .is_some_and(|(version, requirement)| requirement.matches(&version));
    if requirement.domain_id != plan.selected.domain_id
        || required_capabilities != plan.selected.matched_capability_refs
        || !selected_version_matches
    {
        return false;
    }
    let mut digest_subject = plan.clone();
    let claimed_digest = std::mem::take(&mut digest_subject.plan_digest);
    canonical_digest(&digest_subject).is_ok_and(|actual| actual == claimed_digest)
}

fn required_ceremonies() -> Vec<DomainPackAcquisitionCeremony> {
    vec![
        DomainPackAcquisitionCeremony::OperatorTrustPolicy,
        DomainPackAcquisitionCeremony::SupplyChainRegistryVerification,
        DomainPackAcquisitionCeremony::IndependentReviewedRegistryVerification,
        DomainPackAcquisitionCeremony::RuntimeCapabilityApproval,
        DomainPackAcquisitionCeremony::LifecyclePreflight,
    ]
}

fn single_issue(
    code: DomainPackAcquisitionIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) -> DomainPackAcquisitionRejection {
    DomainPackAcquisitionRejection {
        issues: vec![DomainPackAcquisitionIssue {
            code,
            path: path.into(),
            message: message.into(),
        }],
    }
}

fn push(
    issues: &mut Vec<DomainPackAcquisitionIssue>,
    code: DomainPackAcquisitionIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    if issues.len() < 64 {
        issues.push(DomainPackAcquisitionIssue {
            code,
            path: path.into(),
            message: message.into(),
        });
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.is_ascii()
        && !value.starts_with(['.', '-'])
        && !value.ends_with(['.', '-'])
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_raw_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|error| error.to_string())?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover_domain_packs;
    use forge_core_contracts::{
        DomainPackAcquisitionDerivationInput, DomainPackAcquisitionIntent,
        DomainPackAcquisitionIntentDocument, DomainPackAcquisitionOperation,
        DomainPackArtifactBinding, DomainPackCandidateInput, DomainPackCompatibility,
        DomainPackCredentialStatus, DomainPackDiscoveryRequestDocument, DomainPackIdentity,
        DomainPackNamespaceGrant, DomainPackPackageBinding, DomainPackPublisherCredential,
        DomainPackRegistryPackageRecord, DomainPackResolutionCandidate,
        DomainPackSupplyChainRegistry, DomainPackSupplyChainRegistryDocument, RepoPath, StableId,
        DOMAIN_PACK_DISCOVERY_SCHEMA_VERSION, DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
    };

    fn input() -> DomainPackAcquisitionPlanningInput {
        let request: DomainPackDiscoveryRequestDocument = yaml_serde::from_str(include_str!(
            "../../../contracts/domain-pack-discovery/neutral-reviewed-match.yaml"
        ))
        .expect("discovery corpus");
        assert_eq!(request.schema_version, DOMAIN_PACK_DISCOVERY_SCHEMA_VERSION);
        let discovery = discover_domain_packs(&request).expect("discovery projection");
        let projection = &discovery.domain_pack_discovery_projection;
        let selected = &projection.matches[0];
        DomainPackAcquisitionPlanningInput {
            intent: DomainPackAcquisitionIntentDocument {
                schema_version: DOMAIN_PACK_ACQUISITION_SCHEMA_VERSION.to_owned(),
                domain_pack_acquisition_intent: DomainPackAcquisitionIntent {
                    acquisition_id: StableId("acquisition.neutral".to_owned()),
                    authority: DomainPackCandidateAuthority::CandidateOnly,
                    assurance_binding: projection.assurance_binding.clone(),
                    discovery_projection_digest: projection.projection_digest.clone(),
                    demand_digest: projection.demand_digest.clone(),
                    candidate_id: selected.candidate_id.clone(),
                    requirement_ref: selected.requirement_ref.clone(),
                    operation: DomainPackAcquisitionOperation::Install,
                    expected_project_snapshot_digest: projection
                        .assurance_binding
                        .snapshot_digest
                        .clone(),
                },
            },
            request,
            discovery,
        }
    }

    fn derivation_input() -> DomainPackAcquisitionDerivationInput {
        let planning = input();
        let plan = plan_domain_pack_acquisition(&planning).expect("acquisition plan");
        let selected = &plan.domain_pack_acquisition_plan.selected;
        let discovery_candidate = &planning.request.domain_pack_discovery_request.candidates[0];
        let base: forge_core_contracts::DomainPackCompositionRequestDocument =
            yaml_serde::from_str(include_str!(
                "../../../docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml"
            ))
            .expect("foundation composition request");
        let base = base.domain_pack_composition_request;
        let mut candidate_input: DomainPackCandidateInput = base.candidates[0].clone();
        let namespace = discovery_candidate
            .content
            .domain_pack_content
            .namespace
            .clone();
        candidate_input.manifest.domain_pack_manifest.identity = DomainPackIdentity {
            publisher: selected.pack.publisher.clone(),
            name: selected.pack.name.clone(),
            namespace,
            version: selected.pack.version.clone(),
        };
        candidate_input.manifest.domain_pack_manifest.compatibility = DomainPackCompatibility {
            pack_schema_requirement: "^0.1".to_owned(),
            forge_core_requirement: ">=0.12.0, <1.0.0".to_owned(),
        };
        candidate_input.content = discovery_candidate.content.clone();
        candidate_input.manifest_binding = DomainPackArtifactBinding {
            artifact_ref: RepoPath("packages/neutral/manifest.yaml".to_owned()),
            raw_sha256: format!("sha256:{}", "a".repeat(64)),
            canonical_sha256: format!("sha256:{}", "b".repeat(64)),
        };
        candidate_input.manifest.domain_pack_manifest.content =
            forge_core_contracts::DomainPackContentBinding {
                content_ref: RepoPath("packages/neutral/content.yaml".to_owned()),
                raw_sha256: format!("sha256:{}", "c".repeat(64)),
                canonical_sha256: selected.content_digest.clone(),
            };
        candidate_input.manifest_binding.canonical_sha256 =
            canonical_digest(&candidate_input.manifest).expect("manifest digest");
        let package = DomainPackPackageBinding {
            package_ref: RepoPath("packages/neutral/package.yaml".to_owned()),
            package_digest: selected.package_digest.clone(),
            manifest: candidate_input.manifest_binding.clone(),
            content: candidate_input
                .manifest
                .domain_pack_manifest
                .content
                .clone(),
            license: candidate_input
                .manifest
                .domain_pack_manifest
                .provenance
                .license_text
                .clone(),
            fixtures: Vec::new(),
        };
        let resolution_candidate = DomainPackResolutionCandidate {
            input: candidate_input,
            package: package.clone(),
            registry_record_digest: Some(selected.supply_chain_record_digest.clone()),
        };
        let registry = DomainPackSupplyChainRegistryDocument {
            schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
                registry_id: StableId("registry.acquisition.fixture".to_owned()),
                registry_version: "1.0.0".to_owned(),
                audience: StableId("forge.fixture".to_owned()),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                generation: 1,
                previous_snapshot_digest: None,
                issued_at_unix: 100,
                expires_at_unix: 200,
                publisher_credentials: vec![DomainPackPublisherCredential {
                    credential_id: StableId("credential.acquisition.fixture".to_owned()),
                    publisher: selected.pack.publisher.clone(),
                    public_key_hex: "00".repeat(32),
                    status: DomainPackCredentialStatus::Active,
                    valid_from_unix: 0,
                    valid_until_unix: 300,
                }],
                namespace_grants: vec![DomainPackNamespaceGrant {
                    grant_id: StableId("grant.acquisition.fixture".to_owned()),
                    publisher: selected.pack.publisher.clone(),
                    namespace_prefix: selected.pack.publisher.clone(),
                    valid_from_unix: 0,
                    valid_until_unix: 300,
                }],
                packages: vec![DomainPackRegistryPackageRecord {
                    identity: resolution_candidate
                        .input
                        .manifest
                        .domain_pack_manifest
                        .identity
                        .clone(),
                    package_digest: package.package_digest.clone(),
                    manifest_digest: package.manifest.canonical_sha256.clone(),
                    content_digest: package.content.canonical_sha256.clone(),
                    license_digest: package.license.canonical_sha256.clone(),
                    fixture_digests: Vec::new(),
                    namespace_grant_id: StableId("grant.acquisition.fixture".to_owned()),
                    publisher_credential_id: StableId("credential.acquisition.fixture".to_owned()),
                    publisher_signature_hex: "00".repeat(64),
                    record_digest: selected.supply_chain_record_digest.clone(),
                }],
                revocations: Vec::new(),
                snapshot_digest: format!("sha256:{}", "d".repeat(64)),
                signatures: Vec::new(),
            },
        };
        DomainPackAcquisitionDerivationInput {
            planning_input: planning,
            plan,
            forge_core_version: "0.12.0".to_owned(),
            core: base.core,
            registry,
            candidates: vec![resolution_candidate],
        }
    }

    fn add_unrelated_candidate(input: &mut DomainPackAcquisitionDerivationInput) {
        let mut candidate = input.candidates[0].clone();
        candidate.input.manifest.domain_pack_manifest.identity.name =
            StableId("unrelated-method".to_owned());
        candidate
            .input
            .manifest
            .domain_pack_manifest
            .identity
            .namespace = StableId("publisher.neutral.unrelated".to_owned());
        candidate.input.content.domain_pack_content.pack.name =
            StableId("unrelated-method".to_owned());
        candidate.input.content.domain_pack_content.namespace =
            StableId("publisher.neutral.unrelated".to_owned());
        let content_digest = canonical_digest(&candidate.input.content).expect("content digest");
        candidate
            .input
            .manifest
            .domain_pack_manifest
            .content
            .canonical_sha256 = content_digest;
        candidate.package.content = candidate
            .input
            .manifest
            .domain_pack_manifest
            .content
            .clone();
        candidate.input.manifest_binding.canonical_sha256 =
            canonical_digest(&candidate.input.manifest).expect("manifest digest");
        candidate.package.manifest = candidate.input.manifest_binding.clone();
        candidate.package.package_ref = RepoPath("packages/unrelated/package.yaml".to_owned());
        candidate.package.package_digest = format!("sha256:{}", "e".repeat(64));
        candidate.registry_record_digest = Some(format!("sha256:{}", "f".repeat(64)));
        let mut record = input.registry.domain_pack_supply_chain_registry.packages[0].clone();
        record.identity = candidate
            .input
            .manifest
            .domain_pack_manifest
            .identity
            .clone();
        record.package_digest = candidate.package.package_digest.clone();
        record.manifest_digest = candidate.package.manifest.canonical_sha256.clone();
        record.content_digest = candidate.package.content.canonical_sha256.clone();
        record.record_digest = candidate
            .registry_record_digest
            .clone()
            .expect("registry record digest");
        input
            .registry
            .domain_pack_supply_chain_registry
            .packages
            .push(record);
        input.candidates.push(candidate);
    }

    #[test]
    fn exact_current_candidate_produces_trust_required_plan() {
        let plan = plan_domain_pack_acquisition(&input()).expect("acquisition plan");
        let plan = plan.domain_pack_acquisition_plan;
        assert_eq!(plan.authority, DomainPackCandidateAuthority::CandidateOnly);
        assert_eq!(
            plan.status,
            DomainPackAcquisitionPlanStatus::TrustCeremonyRequired
        );
        assert_eq!(plan.required_ceremonies.len(), 5);
        assert!(verify_domain_pack_acquisition_plan(
            &plan_domain_pack_acquisition(&input()).expect("persisted acquisition plan")
        ));
    }

    #[test]
    fn exact_material_derives_candidate_only_p6_inputs() {
        let input = derivation_input();
        let mut document =
            derive_domain_pack_acquisition_inputs(&input).expect("derived P6 inputs");
        assert!(verify_domain_pack_acquisition_derived_inputs(
            &document, &input
        ));
        let derived = &document.domain_pack_acquisition_derived_inputs;
        assert_eq!(
            derived.authority,
            DomainPackCandidateAuthority::CandidateOnly
        );
        assert_eq!(
            derived
                .resolution_projection
                .domain_pack_resolution_projection
                .status,
            DomainPackResolutionStatus::Resolved
        );
        assert_eq!(
            derived
                .composition_request
                .domain_pack_composition_request
                .candidates
                .len(),
            1
        );
        assert!(valid_digest(&derived.derivation_digest));
        document
            .domain_pack_acquisition_derived_inputs
            .composition_request
            .domain_pack_composition_request
            .forge_core_version = "0.13.0".to_owned();
        assert!(!verify_domain_pack_acquisition_derived_inputs(
            &document, &input
        ));

        let mut tampered = input;
        tampered.candidates[0].package.package_digest = format!("sha256:{}", "e".repeat(64));
        let rejection = derive_domain_pack_acquisition_inputs(&tampered)
            .expect_err("package digest drift must fail before resolver derivation");
        assert!(rejection.issues.iter().any(|issue| {
            issue.code == DomainPackAcquisitionIssueCode::CandidateMaterialMismatch
        }));

        let mut forged_source = derivation_input();
        forged_source
            .planning_input
            .request
            .domain_pack_discovery_request
            .uncertainties
            .push("changed after the plan was produced".to_owned());
        let rejection = derive_domain_pack_acquisition_inputs(&forged_source)
            .expect_err("plan must replay from its exact original discovery state");
        assert!(rejection
            .issues
            .iter()
            .any(|issue| { issue.code == DomainPackAcquisitionIssueCode::InvalidAcquisitionPlan }));
    }

    #[test]
    fn package_material_order_does_not_change_derived_inputs() {
        let mut forward = derivation_input();
        add_unrelated_candidate(&mut forward);
        let first = derive_domain_pack_acquisition_inputs(&forward).expect("forward derivation");
        let mut reverse = forward;
        reverse.candidates.reverse();
        let second = derive_domain_pack_acquisition_inputs(&reverse).expect("reverse derivation");
        assert_eq!(first, second);
    }

    #[test]
    fn stale_or_unknown_selection_fails_closed() {
        let mut stale = input();
        stale.intent.domain_pack_acquisition_intent.candidate_id =
            StableId("candidate.absent".to_owned());
        let rejection =
            plan_domain_pack_acquisition(&stale).expect_err("unknown candidate must fail");
        assert!(rejection
            .issues
            .iter()
            .any(|issue| issue.code == DomainPackAcquisitionIssueCode::CandidateAbsent));

        let mut stale_snapshot = input();
        stale_snapshot
            .intent
            .domain_pack_acquisition_intent
            .expected_project_snapshot_digest = format!("sha256:{}", "f".repeat(64));
        let rejection = plan_domain_pack_acquisition(&stale_snapshot)
            .expect_err("stale project snapshot must fail");
        assert!(rejection
            .issues
            .iter()
            .any(|issue| { issue.code == DomainPackAcquisitionIssueCode::StaleDiscoveryBinding }));

        let mut replay_mismatch = input();
        replay_mismatch
            .request
            .domain_pack_discovery_request
            .uncertainties
            .push("new host uncertainty".to_owned());
        let rejection = plan_domain_pack_acquisition(&replay_mismatch)
            .expect_err("projection must replay from the exact request");
        assert!(rejection.issues.iter().any(|issue| {
            issue.code == DomainPackAcquisitionIssueCode::DiscoveryReplayMismatch
        }));

        let mut tampered = input();
        tampered.discovery.domain_pack_discovery_projection.matches[0].package_digest =
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_owned();
        let rejection =
            plan_domain_pack_acquisition(&tampered).expect_err("tampered projection must fail");
        assert!(rejection.issues.iter().any(|issue| {
            issue.code == DomainPackAcquisitionIssueCode::InvalidDiscoveryProjection
        }));

        let mut plan = plan_domain_pack_acquisition(&input()).expect("acquisition plan");
        plan.domain_pack_acquisition_plan.selected.package_digest =
            format!("sha256:{}", "e".repeat(64));
        assert!(!verify_domain_pack_acquisition_plan(&plan));
    }
}
