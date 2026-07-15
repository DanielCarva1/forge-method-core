//! Pure P7c domain-demand discovery.
//!
//! The matcher consumes only closed candidate data. It does not discover files,
//! call a model, trust a registry, select a package, or mutate lifecycle state.

use forge_core_contracts::{
    validate_domain_pack_reviewed_registry_entry, DomainPackCandidateAuthority,
    DomainPackDiscoveryGap, DomainPackDiscoveryGapCode, DomainPackDiscoveryMatch,
    DomainPackDiscoveryProjection, DomainPackDiscoveryProjectionDocument,
    DomainPackDiscoveryRequestDocument, DomainPackDiscoveryStatus, DomainPackReviewedEligibility,
    StableId, DOMAIN_PACK_DISCOVERY_SCHEMA_VERSION, DOMAIN_PACK_SCHEMA_VERSION,
};
use semver::{Version, VersionReq};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const MAX_DOMAIN_PACK_DISCOVERY_CANDIDATES: usize = 64;
pub const MAX_DOMAIN_PACK_DISCOVERY_REQUIREMENTS: usize = 64;
pub const MAX_DOMAIN_PACK_DISCOVERY_UNCERTAINTIES: usize = 64;
pub const MAX_DOMAIN_PACK_DISCOVERY_CAPABILITIES_PER_REQUIREMENT: usize = 64;
pub const MAX_DOMAIN_PACK_DISCOVERY_DECLARATIONS_PER_CANDIDATE: usize = 1_024;
pub const MAX_DOMAIN_PACK_DISCOVERY_MATCHES: usize = 256;
pub const MAX_DOMAIN_PACK_DISCOVERY_TEXT_BYTES: usize = 4_096;
pub const MAX_DOMAIN_PACK_DISCOVERY_DIAGNOSTICS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DomainPackDiscoveryIssueCode {
    UnsupportedSchemaVersion,
    BlankField,
    InvalidStableId,
    InvalidDigest,
    ProjectMismatch,
    MissingDomainDemand,
    InvalidVersionRequirement,
    InvalidCandidateVersion,
    CandidateContentMismatch,
    CandidateContentDigestMismatch,
    CandidateReviewMetadataMismatch,
    DuplicateCandidateVersion,
    ResourceLimitExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackDiscoveryIssue {
    pub code: DomainPackDiscoveryIssueCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPackDiscoveryRejection {
    pub issues: Vec<DomainPackDiscoveryIssue>,
}

/// Match an intent-bound domain demand against exact reviewed package content.
///
/// A successful projection remains `candidate_only`: it can explain available
/// coordinates but cannot trust, install, activate, or select one.
///
/// # Errors
/// Returns all structural and semantic request issues in deterministic order.
pub fn discover_domain_packs(
    document: &DomainPackDiscoveryRequestDocument,
) -> Result<DomainPackDiscoveryProjectionDocument, DomainPackDiscoveryRejection> {
    let request = &document.domain_pack_discovery_request;
    let mut issues = Vec::new();
    if document.schema_version != DOMAIN_PACK_DISCOVERY_SCHEMA_VERSION {
        issue(
            &mut issues,
            DomainPackDiscoveryIssueCode::UnsupportedSchemaVersion,
            "schema_version",
            "unsupported Domain Pack discovery schema version",
        );
    }
    let binding = &request.assurance_binding;
    for (path, value) in [(
        "request.provenance.source_ref",
        request.provenance.source_ref.as_str(),
    )] {
        if value.trim().is_empty() {
            issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::BlankField,
                path,
                "value must not be blank",
            );
        }
    }
    for (path, value) in [
        ("request.request_id", request.request_id.0.as_str()),
        (
            "request.assurance_binding.project_id",
            binding.project_id.0.as_str(),
        ),
        (
            "request.assurance_binding.intent_id",
            binding.intent_id.0.as_str(),
        ),
        (
            "request.requirements.requirement_set_id",
            request.requirements.requirement_set_id.0.as_str(),
        ),
    ] {
        if !valid_id(value) {
            issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::InvalidStableId,
                path,
                "invalid stable id",
            );
        }
    }
    for (path, digest) in [
        (
            "request.assurance_binding.intent_digest",
            binding.intent_digest.as_str(),
        ),
        (
            "request.assurance_binding.accepted_record_digest",
            binding.accepted_record_digest.as_str(),
        ),
        (
            "request.assurance_binding.snapshot_digest",
            binding.snapshot_digest.as_str(),
        ),
        (
            "request.assurance_binding.ledger_head_before_acceptance",
            binding.ledger_head_before_acceptance.as_str(),
        ),
        (
            "request.provenance.source_digest",
            request.provenance.source_digest.as_str(),
        ),
    ] {
        if !valid_digest(digest) {
            issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::InvalidDigest,
                path,
                "expected sha256:<64 lowercase hex>",
            );
        }
    }
    if binding.assurance_epoch == 0
        || binding.intent_revision == 0
        || binding.accepted_sequence == 0
        || binding.accepted_state_version == 0
    {
        issue(
            &mut issues,
            DomainPackDiscoveryIssueCode::BlankField,
            "request.assurance_binding",
            "accepted assurance binding counters must be positive",
        );
    }
    if request.requirements.project_id != binding.project_id {
        issue(
            &mut issues,
            DomainPackDiscoveryIssueCode::ProjectMismatch,
            "request.requirements.project_id",
            "requirements belong to a different project",
        );
    }
    if request.requirements.required_domains.is_empty() {
        issue(
            &mut issues,
            DomainPackDiscoveryIssueCode::MissingDomainDemand,
            "request.requirements.required_domains",
            "at least one typed domain requirement is required",
        );
    }
    let resource_limit_exceeded = request.requirements.required_domains.len()
        > MAX_DOMAIN_PACK_DISCOVERY_REQUIREMENTS
        || request.candidates.len() > MAX_DOMAIN_PACK_DISCOVERY_CANDIDATES
        || request.uncertainties.len() > MAX_DOMAIN_PACK_DISCOVERY_UNCERTAINTIES
        || request.provenance.source_ref.len() > MAX_DOMAIN_PACK_DISCOVERY_TEXT_BYTES
        || request
            .uncertainties
            .iter()
            .any(|uncertainty| uncertainty.len() > MAX_DOMAIN_PACK_DISCOVERY_TEXT_BYTES)
        || request
            .requirements
            .required_domains
            .iter()
            .any(|requirement| {
                requirement.required_capability_refs.len()
                    > MAX_DOMAIN_PACK_DISCOVERY_CAPABILITIES_PER_REQUIREMENT
            })
        || request
            .candidates
            .iter()
            .any(|candidate| !candidate_within_resource_limits(candidate));
    if resource_limit_exceeded {
        issue(
            &mut issues,
            DomainPackDiscoveryIssueCode::ResourceLimitExceeded,
            "request",
            "discovery input exceeds a bounded collection limit",
        );
        issues.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.message.cmp(&right.message))
        });
        return Err(DomainPackDiscoveryRejection { issues });
    }

    let mut requirement_ids = BTreeSet::new();
    for (index, requirement) in request.requirements.required_domains.iter().enumerate() {
        let path = format!("request.requirements.required_domains[{index}]");
        if !valid_id(&requirement.id.0) || !valid_id(&requirement.domain_id.0) {
            issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::InvalidStableId,
                &path,
                "requirement id and domain id must be valid stable ids",
            );
        }
        for (capability_index, capability) in
            requirement.required_capability_refs.iter().enumerate()
        {
            if !valid_id(&capability.0) {
                issue(
                    &mut issues,
                    DomainPackDiscoveryIssueCode::InvalidStableId,
                    format!("{path}.required_capability_refs[{capability_index}]"),
                    "invalid capability stable id",
                );
            }
        }
        if !requirement_ids.insert(requirement.id.0.as_str()) {
            issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::BlankField,
                format!("{path}.id"),
                "requirement id occurs more than once",
            );
        }
        if VersionReq::parse(&requirement.pack_version_requirement).is_err() {
            issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::InvalidVersionRequirement,
                format!("{path}.pack_version_requirement"),
                "invalid semantic-version requirement",
            );
        }
    }

    let mut candidate_versions = BTreeSet::new();
    for (index, candidate) in request.candidates.iter().enumerate() {
        let path = format!("request.candidates[{index}]");
        let entry = &candidate.reviewed_entry;
        for (field, value) in [
            ("publisher", entry.pack.publisher.0.as_str()),
            ("name", entry.pack.name.0.as_str()),
        ] {
            if !valid_id(value) {
                issue(
                    &mut issues,
                    DomainPackDiscoveryIssueCode::InvalidStableId,
                    format!("{path}.reviewed_entry.pack.{field}"),
                    "invalid pack stable id",
                );
            }
        }
        for (domain_index, domain) in candidate
            .content
            .domain_pack_content
            .provided_domains
            .iter()
            .enumerate()
        {
            if !valid_id(&domain.id.0) {
                issue(
                    &mut issues,
                    DomainPackDiscoveryIssueCode::InvalidStableId,
                    format!(
                        "{path}.content.domain_pack_content.provided_domains[{domain_index}].id"
                    ),
                    "invalid provided domain stable id",
                );
            }
        }
        for (capability_index, capability) in candidate
            .content
            .domain_pack_content
            .provided_capabilities
            .iter()
            .enumerate()
        {
            if !valid_id(&capability.id.0) {
                issue(
                    &mut issues,
                    DomainPackDiscoveryIssueCode::InvalidStableId,
                    format!("{path}.content.domain_pack_content.provided_capabilities[{capability_index}].id"),
                    "invalid provided capability stable id",
                );
            }
        }
        let key = (
            entry.pack.publisher.0.as_str(),
            entry.pack.name.0.as_str(),
            entry.pack.version.as_str(),
        );
        if !candidate_versions.insert(key) {
            issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::DuplicateCandidateVersion,
                &path,
                "candidate coordinate and version occurs more than once",
            );
        }
        if Version::parse(&entry.pack.version).is_err() {
            issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::InvalidCandidateVersion,
                format!("{path}.reviewed_entry.pack.version"),
                "candidate version is not semantic version",
            );
        }
        for (field, digest) in [
            ("package_digest", entry.package_digest.as_str()),
            (
                "supply_chain_record_digest",
                entry.supply_chain_record_digest.as_str(),
            ),
            ("manifest_digest", entry.manifest_digest.as_str()),
            ("content_digest", entry.content_digest.as_str()),
            ("license_digest", entry.license_digest.as_str()),
        ] {
            if !valid_digest(digest) {
                issue(
                    &mut issues,
                    DomainPackDiscoveryIssueCode::InvalidDigest,
                    format!("{path}.reviewed_entry.{field}"),
                    "expected sha256:<64 lowercase hex>",
                );
            }
        }
        for (field, digest) in [
            (
                "promotion_decision_digest",
                entry.promotion_decision_digest.as_str(),
            ),
            ("authorization_digest", entry.authorization_digest.as_str()),
            ("entry_digest", entry.entry_digest.as_str()),
        ] {
            if !valid_raw_digest(digest) {
                issue(
                    &mut issues,
                    DomainPackDiscoveryIssueCode::InvalidDigest,
                    format!("{path}.reviewed_entry.{field}"),
                    "expected 64 lowercase hexadecimal SHA-256 characters",
                );
            }
        }
        if candidate.content.schema_version != DOMAIN_PACK_SCHEMA_VERSION
            || candidate.content.domain_pack_content.pack != entry.pack
        {
            issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::CandidateContentMismatch,
                format!("{path}.content"),
                "candidate content does not bind the exact pack identity",
            );
        }
        match canonical_digest(&candidate.content) {
            Ok(digest) if digest == entry.content_digest => {}
            Ok(_) => issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::CandidateContentDigestMismatch,
                format!("{path}.reviewed_entry.content_digest"),
                "candidate content digest does not match canonical content bytes",
            ),
            Err(message) => issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::CandidateContentDigestMismatch,
                format!("{path}.content"),
                message,
            ),
        }
        let mut entry_subject = entry.clone();
        entry_subject.entry_digest.clear();
        if canonical_raw_digest(&entry_subject).as_deref() != Ok(entry.entry_digest.as_str()) {
            issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::CandidateReviewMetadataMismatch,
                format!("{path}.reviewed_entry.entry_digest"),
                "reviewed entry digest is not canonical",
            );
        }
        for contract_issue in validate_domain_pack_reviewed_registry_entry(entry) {
            issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::CandidateReviewMetadataMismatch,
                format!("{path}.{}", contract_issue.path),
                contract_issue.message,
            );
        }
    }
    for (index, uncertainty) in request.uncertainties.iter().enumerate() {
        if uncertainty.trim().is_empty() {
            issue(
                &mut issues,
                DomainPackDiscoveryIssueCode::BlankField,
                format!("request.uncertainties[{index}]"),
                "uncertainty must not be blank",
            );
        }
    }

    if !issues.is_empty() {
        issues.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.message.cmp(&right.message))
        });
        return Err(DomainPackDiscoveryRejection { issues });
    }

    let indexed_candidates = request
        .candidates
        .iter()
        .filter(|candidate| {
            candidate.reviewed_entry.eligibility == DomainPackReviewedEligibility::EligibleReviewed
        })
        .filter_map(|candidate| {
            let version = Version::parse(&candidate.reviewed_entry.pack.version).ok()?;
            let domains = candidate
                .content
                .domain_pack_content
                .provided_domains
                .iter()
                .map(|domain| &domain.id)
                .collect::<BTreeSet<_>>();
            let capabilities = candidate
                .content
                .domain_pack_content
                .provided_capabilities
                .iter()
                .map(|capability| &capability.id)
                .collect::<BTreeSet<_>>();
            Some((candidate, version, domains, capabilities))
        })
        .collect::<Vec<_>>();
    let mut matches = Vec::new();
    let mut gaps = Vec::new();
    for requirement in &request.requirements.required_domains {
        let eligible_domain_candidates = indexed_candidates
            .iter()
            .filter(|(_, _, domains, _)| domains.contains(&requirement.domain_id))
            .collect::<Vec<_>>();
        if eligible_domain_candidates.is_empty() {
            gaps.push(gap(
                requirement.id.clone(),
                requirement.domain_id.clone(),
                DomainPackDiscoveryGapCode::NoEligibleReviewedPack,
                "no eligible reviewed package declares the required domain",
            ));
            continue;
        }
        let Ok(version_requirement) = VersionReq::parse(&requirement.pack_version_requirement)
        else {
            continue;
        };
        let version_candidates = eligible_domain_candidates
            .into_iter()
            .filter(|(_, version, _, _)| version_requirement.matches(version))
            .collect::<Vec<_>>();
        if version_candidates.is_empty() {
            gaps.push(gap(
                requirement.id.clone(),
                requirement.domain_id.clone(),
                DomainPackDiscoveryGapCode::VersionIncompatible,
                "eligible reviewed packages exist but none satisfies the required version",
            ));
            continue;
        }
        let capability_candidates = version_candidates
            .into_iter()
            .filter(|(_, _, _, capabilities)| {
                requirement
                    .required_capability_refs
                    .iter()
                    .all(|required| capabilities.contains(required))
            })
            .collect::<Vec<_>>();
        if capability_candidates.is_empty() {
            gaps.push(gap(
                requirement.id.clone(),
                requirement.domain_id.clone(),
                DomainPackDiscoveryGapCode::MissingRequiredCapability,
                "version-compatible packages do not declare every required capability",
            ));
            continue;
        }
        if capability_candidates.len()
            > MAX_DOMAIN_PACK_DISCOVERY_MATCHES.saturating_sub(matches.len())
        {
            return Err(DomainPackDiscoveryRejection {
                issues: vec![DomainPackDiscoveryIssue {
                    code: DomainPackDiscoveryIssueCode::ResourceLimitExceeded,
                    path: "projection.matches".to_owned(),
                    message: "discovery match projection exceeds its bounded output limit"
                        .to_owned(),
                }],
            });
        }
        for (candidate, _, _, _) in capability_candidates {
            let mut matched_capability_refs = requirement.required_capability_refs.clone();
            matched_capability_refs.sort();
            matches.push(DomainPackDiscoveryMatch {
                candidate_id: derived_candidate_id(candidate),
                requirement_ref: requirement.id.clone(),
                domain_id: requirement.domain_id.clone(),
                pack: candidate.reviewed_entry.pack.clone(),
                package_digest: candidate.reviewed_entry.package_digest.clone(),
                supply_chain_record_digest: candidate
                    .reviewed_entry
                    .supply_chain_record_digest
                    .clone(),
                reviewed_entry_digest: candidate.reviewed_entry.entry_digest.clone(),
                content_digest: candidate.reviewed_entry.content_digest.clone(),
                matched_capability_refs,
            });
        }
    }
    matches.sort_by(|left, right| {
        left.requirement_ref
            .cmp(&right.requirement_ref)
            .then_with(|| left.pack.publisher.cmp(&right.pack.publisher))
            .then_with(|| left.pack.name.cmp(&right.pack.name))
            .then_with(|| left.pack.version.cmp(&right.pack.version))
            .then_with(|| left.package_digest.cmp(&right.package_digest))
    });
    gaps.sort_by(|left, right| {
        left.requirement_ref
            .cmp(&right.requirement_ref)
            .then_with(|| left.code.cmp(&right.code))
    });
    let status = if gaps.is_empty() {
        DomainPackDiscoveryStatus::Matched
    } else {
        DomainPackDiscoveryStatus::GapsPresent
    };
    let mut uncertainties = request.uncertainties.clone();
    uncertainties.sort();
    let demand_digest =
        canonical_demand_digest(document).map_err(|message| DomainPackDiscoveryRejection {
            issues: vec![DomainPackDiscoveryIssue {
                code: DomainPackDiscoveryIssueCode::CandidateContentDigestMismatch,
                path: "request".to_owned(),
                message,
            }],
        })?;
    let mut projection = DomainPackDiscoveryProjection {
        request_id: request.request_id.clone(),
        demand_digest,
        authority: DomainPackCandidateAuthority::CandidateOnly,
        assurance_binding: request.assurance_binding.clone(),
        uncertainties,
        status,
        matches,
        gaps,
        projection_digest: String::new(),
    };
    projection.projection_digest =
        canonical_digest(&projection).map_err(|message| DomainPackDiscoveryRejection {
            issues: vec![DomainPackDiscoveryIssue {
                code: DomainPackDiscoveryIssueCode::CandidateContentDigestMismatch,
                path: "projection".to_owned(),
                message,
            }],
        })?;
    Ok(DomainPackDiscoveryProjectionDocument {
        schema_version: DOMAIN_PACK_DISCOVERY_SCHEMA_VERSION.to_owned(),
        domain_pack_discovery_projection: projection,
    })
}

/// Verify that a persisted discovery projection retains its canonical binding.
#[must_use]
pub fn verify_domain_pack_discovery_projection(
    document: &DomainPackDiscoveryProjectionDocument,
) -> bool {
    let value = &document.domain_pack_discovery_projection;
    if document.schema_version != DOMAIN_PACK_DISCOVERY_SCHEMA_VERSION
        || value.authority != DomainPackCandidateAuthority::CandidateOnly
        || !valid_id(&value.request_id.0)
        || !valid_id(&value.assurance_binding.project_id.0)
        || !valid_id(&value.assurance_binding.intent_id.0)
        || value.assurance_binding.assurance_epoch == 0
        || value.assurance_binding.intent_revision == 0
        || value.assurance_binding.accepted_sequence == 0
        || value.assurance_binding.accepted_state_version == 0
        || !valid_digest(&value.demand_digest)
        || !valid_digest(&value.assurance_binding.intent_digest)
        || !valid_digest(&value.assurance_binding.accepted_record_digest)
        || !valid_digest(&value.assurance_binding.snapshot_digest)
        || !valid_digest(&value.assurance_binding.ledger_head_before_acceptance)
        || !valid_digest(&value.projection_digest)
        || value.matches.len() > MAX_DOMAIN_PACK_DISCOVERY_MATCHES
        || value.gaps.len() > MAX_DOMAIN_PACK_DISCOVERY_REQUIREMENTS
        || value.uncertainties.len() > MAX_DOMAIN_PACK_DISCOVERY_UNCERTAINTIES
        || value.matches.iter().any(|item| {
            item.matched_capability_refs.len()
                > MAX_DOMAIN_PACK_DISCOVERY_CAPABILITIES_PER_REQUIREMENT
        })
        || value
            .uncertainties
            .iter()
            .any(|item| item.trim().is_empty() || item.len() > MAX_DOMAIN_PACK_DISCOVERY_TEXT_BYTES)
        || matches!(value.status, DomainPackDiscoveryStatus::Matched)
            && (value.matches.is_empty() || !value.gaps.is_empty())
        || matches!(value.status, DomainPackDiscoveryStatus::GapsPresent) && value.gaps.is_empty()
    {
        return false;
    }
    let match_requirements = value
        .matches
        .iter()
        .map(|item| &item.requirement_ref)
        .collect::<BTreeSet<_>>();
    if value.gaps.iter().any(|gap| {
        !valid_id(&gap.requirement_ref.0)
            || !valid_id(&gap.domain_id.0)
            || gap.message.trim().is_empty()
            || gap.message.len() > MAX_DOMAIN_PACK_DISCOVERY_TEXT_BYTES
            || gap.next_action.trim().is_empty()
            || gap.next_action.len() > MAX_DOMAIN_PACK_DISCOVERY_TEXT_BYTES
            || match_requirements.contains(&gap.requirement_ref)
    }) || value.matches.iter().any(|item| {
        !valid_id(&item.candidate_id.0)
            || !valid_id(&item.requirement_ref.0)
            || !valid_id(&item.domain_id.0)
            || !valid_id(&item.pack.publisher.0)
            || !valid_id(&item.pack.name.0)
            || Version::parse(&item.pack.version).is_err()
            || !valid_digest(&item.package_digest)
            || !valid_digest(&item.supply_chain_record_digest)
            || !valid_raw_digest(&item.reviewed_entry_digest)
            || !valid_digest(&item.content_digest)
            || item
                .matched_capability_refs
                .iter()
                .any(|capability| !valid_id(&capability.0))
    }) {
        return false;
    }
    let mut sorted_matches = value.matches.clone();
    sorted_matches.sort_by(|left, right| {
        left.requirement_ref
            .cmp(&right.requirement_ref)
            .then_with(|| left.pack.publisher.cmp(&right.pack.publisher))
            .then_with(|| left.pack.name.cmp(&right.pack.name))
            .then_with(|| left.pack.version.cmp(&right.pack.version))
            .then_with(|| left.package_digest.cmp(&right.package_digest))
    });
    let mut sorted_gaps = value.gaps.clone();
    sorted_gaps.sort_by(|left, right| {
        left.requirement_ref
            .cmp(&right.requirement_ref)
            .then_with(|| left.code.cmp(&right.code))
    });
    let mut sorted_uncertainties = value.uncertainties.clone();
    sorted_uncertainties.sort();
    if sorted_matches != value.matches
        || sorted_gaps != value.gaps
        || sorted_uncertainties != value.uncertainties
    {
        return false;
    }
    let mut projection = value.clone();
    let claimed_digest = std::mem::take(&mut projection.projection_digest);
    canonical_digest(&projection).is_ok_and(|actual| actual == claimed_digest)
}

fn derived_candidate_id(
    candidate: &forge_core_contracts::DomainPackDiscoveryCandidate,
) -> StableId {
    let entry = &candidate.reviewed_entry;
    let mut hasher = Sha256::new();
    for component in [
        entry.pack.publisher.0.as_bytes(),
        entry.pack.name.0.as_bytes(),
        entry.pack.version.as_bytes(),
        entry.package_digest.as_bytes(),
        entry.entry_digest.as_bytes(),
    ] {
        hasher.update((component.len() as u64).to_be_bytes());
        hasher.update(component);
    }
    StableId(format!("candidate.{:x}", hasher.finalize()))
}

fn gap(
    requirement_ref: StableId,
    domain_id: StableId,
    code: DomainPackDiscoveryGapCode,
    message: &str,
) -> DomainPackDiscoveryGap {
    let next_action = match code {
        DomainPackDiscoveryGapCode::NoEligibleReviewedPack => {
            "Acquire or review an exact Domain Pack candidate before lifecycle preflight."
        }
        DomainPackDiscoveryGapCode::VersionIncompatible => {
            "Provide an eligible reviewed package version satisfying the declared requirement."
        }
        DomainPackDiscoveryGapCode::MissingRequiredCapability => {
            "Provide reviewed content declaring every required capability or revise the accepted requirement."
        }
    };
    DomainPackDiscoveryGap {
        requirement_ref,
        domain_id,
        code,
        message: message.to_owned(),
        next_action: next_action.to_owned(),
    }
}

fn issue(
    issues: &mut Vec<DomainPackDiscoveryIssue>,
    code: DomainPackDiscoveryIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    if issues.len() < MAX_DOMAIN_PACK_DISCOVERY_DIAGNOSTICS {
        issues.push(DomainPackDiscoveryIssue {
            code,
            path: path.into(),
            message: message.into(),
        });
    }
}

fn candidate_within_resource_limits(
    candidate: &forge_core_contracts::DomainPackDiscoveryCandidate,
) -> bool {
    let content = &candidate.content.domain_pack_content;
    let entry = &candidate.reviewed_entry;
    [
        content.provided_domains.len(),
        content.provided_capabilities.len(),
        content.workflow_overlay.policies.len(),
        content.hazards.len(),
        content.lifecycle_models.len(),
        content.evaluators.len(),
        content.fixtures.len(),
        content.adapters.len(),
        entry.fixture_digests.len(),
        entry.independent_review_digests.len(),
        entry.compatibility.evaluator_protocol_versions.len(),
        entry.compatibility.predecessor_content_digests.len(),
        entry.compatibility.migration_evidence_refs.len(),
    ]
    .into_iter()
    .all(|length| length <= MAX_DOMAIN_PACK_DISCOVERY_DECLARATIONS_PER_CANDIDATE)
        && content
            .provided_domains
            .iter()
            .all(|domain| domain.description.len() <= MAX_DOMAIN_PACK_DISCOVERY_TEXT_BYTES)
        && content
            .provided_capabilities
            .iter()
            .all(|capability| capability.description.len() <= MAX_DOMAIN_PACK_DISCOVERY_TEXT_BYTES)
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

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_raw_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Serialize)]
struct DomainPackDemandDigestBasis<'a> {
    schema_version: &'a str,
    request_id: &'a StableId,
    authority: DomainPackCandidateAuthority,
    assurance_binding: &'a forge_core_contracts::DurableAssuranceEpochBinding,
    requirements: forge_core_contracts::DomainPackProjectRequirements,
    provenance: &'a forge_core_contracts::DomainPackDemandProvenance,
    uncertainties: Vec<String>,
}

fn canonical_demand_digest(
    document: &DomainPackDiscoveryRequestDocument,
) -> Result<String, String> {
    let request = &document.domain_pack_discovery_request;
    let mut requirements = request.requirements.clone();
    requirements.required_domains.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.domain_id.cmp(&right.domain_id))
            .then_with(|| {
                left.pack_version_requirement
                    .cmp(&right.pack_version_requirement)
            })
    });
    for requirement in &mut requirements.required_domains {
        requirement.required_capability_refs.sort();
    }
    let mut uncertainties = request.uncertainties.clone();
    uncertainties.sort();
    canonical_digest(&DomainPackDemandDigestBasis {
        schema_version: &document.schema_version,
        request_id: &request.request_id,
        authority: request.authority,
        assurance_binding: &request.assurance_binding,
        requirements,
        provenance: &request.provenance,
        uncertainties,
    })
}

fn canonical_digest<T: Serialize>(value: &T) -> Result<String, String> {
    let canonical = serde_json_canonicalizer::to_vec(value).map_err(|error| error.to_string())?;
    Ok(format!("sha256:{:x}", Sha256::digest(canonical)))
}

fn canonical_raw_digest<T: Serialize>(value: &T) -> Result<String, String> {
    let canonical = serde_json_canonicalizer::to_vec(value).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(canonical)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::{
        DomainCapabilityDeclarationAuthority, DomainPackContent, DomainPackContentDocument,
        DomainPackDiscoveryCandidate, DomainPackDomainRequirement, DomainPackProjectRequirements,
        DomainPackPromotionStage, DomainPackProvidedCapability, DomainPackProvidedDomain,
        DomainPackReviewedCompatibility, DomainPackReviewedRegistryEntry,
        DomainPackRevocationBinding, DomainPackVersionReference, DurableAssuranceEpochBinding,
        WorkflowGovernancePolicyOverlay, DOMAIN_PACK_SCHEMA_VERSION,
    };

    fn hash(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn raw_hash(character: char) -> String {
        character.to_string().repeat(64)
    }

    fn candidate(
        version: &str,
        eligibility: DomainPackReviewedEligibility,
    ) -> DomainPackDiscoveryCandidate {
        let pack = DomainPackVersionReference {
            publisher: StableId("forge.reference".to_owned()),
            name: StableId("neutral-domain".to_owned()),
            version: version.to_owned(),
        };
        let content = DomainPackContentDocument {
            schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
            domain_pack_content: DomainPackContent {
                pack: pack.clone(),
                namespace: StableId("forge.reference.neutral".to_owned()),
                workflow_overlay: WorkflowGovernancePolicyOverlay {
                    id: StableId("overlay.neutral".to_owned()),
                    base_bundle_id: StableId("bundle.core".to_owned()),
                    policies: Vec::new(),
                },
                provided_domains: vec![DomainPackProvidedDomain {
                    id: StableId("domain.neutral".to_owned()),
                    description: "Neutral test domain".to_owned(),
                    policy_refs: Vec::new(),
                    hazard_refs: Vec::new(),
                    lifecycle_model_refs: Vec::new(),
                }],
                provided_capabilities: vec![DomainPackProvidedCapability {
                    id: StableId("capability.neutral.review".to_owned()),
                    kind: forge_core_contracts::DomainPackCapabilityKind::HumanReview,
                    description: "Review capability".to_owned(),
                    evidence_rule_refs: Vec::new(),
                    authority: DomainCapabilityDeclarationAuthority::DeclarationOnly,
                }],
                hazards: Vec::new(),
                lifecycle_models: Vec::new(),
                evaluators: Vec::new(),
                fixtures: Vec::new(),
                adapters: Vec::new(),
            },
        };
        let content_digest = canonical_digest(&content).expect("content digest");
        let stage = match eligibility {
            DomainPackReviewedEligibility::EligibleReviewed => DomainPackPromotionStage::Reviewed,
            DomainPackReviewedEligibility::IneligibleDeprecated => {
                DomainPackPromotionStage::Deprecated
            }
            DomainPackReviewedEligibility::IneligibleRevoked => DomainPackPromotionStage::Revoked,
            DomainPackReviewedEligibility::IneligibleSuperseded => {
                DomainPackPromotionStage::Superseded
            }
        };
        let mut reviewed_entry = DomainPackReviewedRegistryEntry {
            pack,
            package_digest: hash('a'),
            supply_chain_record_digest: hash('b'),
            manifest_digest: hash('c'),
            content_digest,
            license_digest: hash('d'),
            fixture_digests: vec![hash('3')],
            stage,
            eligibility,
            promotion_decision_digest: raw_hash('e'),
            authorization_digest: raw_hash('f'),
            independent_review_digests: vec![raw_hash('1'), raw_hash('2')],
            compatibility: DomainPackReviewedCompatibility {
                forge_core_requirement: ">=0.12.0, <1.0.0".to_owned(),
                pack_schema_requirement: "^0.1".to_owned(),
                evaluator_protocol_versions: Vec::new(),
                predecessor_content_digests: Vec::new(),
                breaking_change: false,
                migration_evidence_refs: Vec::new(),
            },
            deprecation: None,
            revocation: matches!(
                eligibility,
                DomainPackReviewedEligibility::IneligibleRevoked
            )
            .then(|| DomainPackRevocationBinding {
                reason: "reviewed fixture revocation".to_owned(),
                effective_at_unix: 1,
                authorization_digest: raw_hash('7'),
            }),
            supersession: None,
            entry_digest: String::new(),
        };
        reviewed_entry.entry_digest = canonical_raw_digest(&reviewed_entry).expect("entry digest");
        DomainPackDiscoveryCandidate {
            reviewed_entry,
            content,
        }
    }

    fn request(
        candidates: Vec<DomainPackDiscoveryCandidate>,
    ) -> DomainPackDiscoveryRequestDocument {
        DomainPackDiscoveryRequestDocument {
            schema_version: DOMAIN_PACK_DISCOVERY_SCHEMA_VERSION.to_owned(),
            domain_pack_discovery_request: forge_core_contracts::DomainPackDiscoveryRequest {
                request_id: StableId("discovery.neutral".to_owned()),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                assurance_binding: DurableAssuranceEpochBinding {
                    project_id: StableId("project.neutral".to_owned()),
                    assurance_epoch: 1,
                    intent_id: StableId("intent.neutral".to_owned()),
                    intent_revision: 1,
                    intent_digest: hash('d'),
                    accepted_record_digest: hash('4'),
                    accepted_sequence: 1,
                    accepted_state_version: 1,
                    snapshot_digest: hash('5'),
                    ledger_head_before_acceptance: hash('6'),
                },
                requirements: DomainPackProjectRequirements {
                    project_id: StableId("project.neutral".to_owned()),
                    requirement_set_id: StableId("requirements.neutral".to_owned()),
                    required_domains: vec![DomainPackDomainRequirement {
                        id: StableId("requirement.neutral".to_owned()),
                        domain_id: StableId("domain.neutral".to_owned()),
                        pack_version_requirement: ">=1.0.0, <2.0.0".to_owned(),
                        required_capability_refs: vec![StableId(
                            "capability.neutral.review".to_owned(),
                        )],
                    }],
                },
                provenance: forge_core_contracts::DomainPackDemandProvenance {
                    source: forge_core_contracts::DomainPackDemandSource::HostProposal,
                    source_ref: "conversation://neutral".to_owned(),
                    source_digest: hash('e'),
                },
                uncertainties: vec!["Exact package remains operator-selected".to_owned()],
                candidates,
            },
        }
    }

    #[test]
    fn matching_is_deterministic_and_candidate_only() {
        let first = candidate("1.0.0", DomainPackReviewedEligibility::EligibleReviewed);
        let mut second = candidate("1.1.0", DomainPackReviewedEligibility::EligibleReviewed);
        second.reviewed_entry.package_digest = hash('0');
        second.reviewed_entry.entry_digest.clear();
        second.reviewed_entry.entry_digest =
            canonical_raw_digest(&second.reviewed_entry).expect("second entry digest");
        let forward = discover_domain_packs(&request(vec![first.clone(), second.clone()]))
            .expect("forward discovery");
        let reverse =
            discover_domain_packs(&request(vec![second, first])).expect("reverse discovery");
        assert_eq!(forward, reverse);
        assert!(verify_domain_pack_discovery_projection(&forward));
        let projection = forward.domain_pack_discovery_projection;
        assert_eq!(
            projection.authority,
            DomainPackCandidateAuthority::CandidateOnly
        );
        assert_eq!(projection.status, DomainPackDiscoveryStatus::Matched);
        assert_eq!(projection.matches.len(), 2);
        assert!(projection.gaps.is_empty());
        assert_eq!(
            projection.uncertainties,
            vec!["Exact package remains operator-selected"]
        );
    }

    #[test]
    fn ineligible_or_incomplete_candidates_leave_explicit_gaps() {
        let ineligible = candidate("1.0.0", DomainPackReviewedEligibility::IneligibleRevoked);
        let projection = discover_domain_packs(&request(vec![ineligible]))
            .expect("gap projection")
            .domain_pack_discovery_projection;
        assert_eq!(projection.status, DomainPackDiscoveryStatus::GapsPresent);
        assert_eq!(
            projection.gaps[0].code,
            DomainPackDiscoveryGapCode::NoEligibleReviewedPack
        );
    }

    #[test]
    fn persisted_projection_tampering_is_detected() {
        let mut projection = discover_domain_packs(&request(vec![candidate(
            "1.0.0",
            DomainPackReviewedEligibility::EligibleReviewed,
        )]))
        .expect("projection");
        projection.domain_pack_discovery_projection.matches[0].package_digest = hash('0');
        assert!(!verify_domain_pack_discovery_projection(&projection));
    }

    #[test]
    fn invalid_bounds_and_review_metadata_fail_closed() {
        let mut empty = request(Vec::new());
        empty
            .domain_pack_discovery_request
            .requirements
            .required_domains
            .clear();
        let rejection = discover_domain_packs(&empty).expect_err("empty domain demand");
        assert!(rejection
            .issues
            .iter()
            .any(|issue| { issue.code == DomainPackDiscoveryIssueCode::MissingDomainDemand }));

        let mut bounded = request(Vec::new());
        bounded.domain_pack_discovery_request.uncertainties =
            vec!["bounded".to_owned(); MAX_DOMAIN_PACK_DISCOVERY_UNCERTAINTIES + 1];
        let rejection = discover_domain_packs(&bounded).expect_err("oversized request");
        assert!(rejection
            .issues
            .iter()
            .any(|issue| { issue.code == DomainPackDiscoveryIssueCode::ResourceLimitExceeded }));

        let mut invalid_id = request(Vec::new());
        invalid_id.domain_pack_discovery_request.request_id =
            StableId("INVALID ID WITH SPACE".to_owned());
        let rejection = discover_domain_packs(&invalid_id).expect_err("invalid stable id");
        assert!(rejection
            .issues
            .iter()
            .any(|issue| { issue.code == DomainPackDiscoveryIssueCode::InvalidStableId }));

        let mut tampered = candidate("1.0.0", DomainPackReviewedEligibility::EligibleReviewed);
        tampered.reviewed_entry.package_digest = hash('0');
        let rejection = discover_domain_packs(&request(vec![tampered]))
            .expect_err("non-canonical reviewed entry");
        assert!(rejection.issues.iter().any(|issue| {
            issue.code == DomainPackDiscoveryIssueCode::CandidateReviewMetadataMismatch
        }));
    }

    #[test]
    fn amplified_match_output_fails_at_the_projection_bound() {
        let candidates = (0..MAX_DOMAIN_PACK_DISCOVERY_CANDIDATES)
            .map(|index| {
                candidate(
                    &format!("1.0.{index}"),
                    DomainPackReviewedEligibility::EligibleReviewed,
                )
            })
            .collect::<Vec<_>>();
        let mut amplified = request(candidates);
        let template = amplified
            .domain_pack_discovery_request
            .requirements
            .required_domains[0]
            .clone();
        amplified
            .domain_pack_discovery_request
            .requirements
            .required_domains = (0..17)
            .map(|index| {
                let mut requirement = template.clone();
                requirement.id = StableId(format!("requirement.neutral.{index}"));
                requirement
            })
            .collect();
        let rejection =
            discover_domain_packs(&amplified).expect_err("amplified output must fail closed");
        assert!(rejection
            .issues
            .iter()
            .any(|issue| { issue.code == DomainPackDiscoveryIssueCode::ResourceLimitExceeded }));
    }

    #[test]
    fn malformed_or_equivocated_candidates_fail_closed() {
        let first = candidate("1.0.0", DomainPackReviewedEligibility::EligibleReviewed);
        let second = first.clone();
        let rejection = discover_domain_packs(&request(vec![first, second]))
            .expect_err("duplicate candidate must fail");
        assert!(rejection.issues.iter().any(|issue| {
            issue.code == DomainPackDiscoveryIssueCode::DuplicateCandidateVersion
        }));
    }
}
