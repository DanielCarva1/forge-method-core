//! Pure, deterministic P6a Domain Pack candidate validation and composition.
//!
//! Raw artifacts are caller-supplied sidecars: typed YAML alone can never
//! prove the raw digest authored in a manifest. All output remains
//! `candidate_only`; this module performs no discovery, IO, installation, or
//! authority minting.

use std::collections::{BTreeMap, BTreeSet};

use forge_core_contracts::{
    DomainAdapterDeclaration, DomainEvaluatorDeclaration, DomainEvaluatorImplementation,
    DomainHazard, DomainLifecycleModel, DomainPackArtifactBinding, DomainPackCandidateAuthority,
    DomainPackCandidateInput, DomainPackComposedIdentity, DomainPackCompositionGap,
    DomainPackCompositionGapCode, DomainPackCompositionIssue, DomainPackCompositionIssueCode,
    DomainPackCompositionProjection, DomainPackCompositionProjectionDocument,
    DomainPackCompositionRequestDocument, DomainPackCompositionStatus,
    DomainPackContributionIndexEntry, DomainPackContributionKind, DomainPackCoordinate,
    DomainPackIdentity, DomainPackProjectRequirements, DomainPackVersionReference, StableId,
    WorkflowGovernanceBundle, WorkflowGovernanceBundleDocument, DOMAIN_PACK_SCHEMA_VERSION,
    WORKFLOW_GOVERNANCE_SCHEMA_VERSION,
};
use semver::{Version, VersionReq};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::validate_workflow_governance_bundle;

pub const MAX_DOMAIN_PACK_CANDIDATES: usize = 64;
pub const MAX_DOMAIN_PACK_CONTRIBUTIONS: usize = 16_384;
pub const MAX_DOMAIN_PACK_DEPENDENCIES_PER_PACK: usize = 64;
pub const MAX_DOMAIN_PACK_DEPENDENCY_DEPTH: usize = 32;
pub const MAX_DOMAIN_PACK_DIAGNOSTICS: usize = 1_024;
pub const MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DOMAIN_PACK_REQUIRED_DOMAINS: usize = 1_024;
pub const MAX_DOMAIN_PACK_CAPABILITIES_PER_REQUIREMENT: usize = 256;
pub const MAX_DOMAIN_PACK_IDENTIFIER_BYTES: usize = 256;

/// Exact byte sidecars for one candidate. The key must exactly equal both
/// manifest identity and content version reference.
#[derive(Debug, Clone, Copy)]
pub struct DomainPackCandidateMaterial<'a> {
    pub publisher: &'a str,
    pub name: &'a str,
    pub version: &'a str,
    pub manifest_raw: &'a [u8],
    pub content_raw: &'a [u8],
    pub license_raw: &'a [u8],
}

/// Validate one candidate with exact raw byte evidence. Diagnostics are
/// stable-sorted and capped. An empty result means structurally composable,
/// never installed, reviewed, trusted, or active.
#[must_use]
pub fn validate_domain_pack_candidate(
    candidate: &DomainPackCandidateInput,
    material: &DomainPackCandidateMaterial<'_>,
    forge_core_version: &str,
) -> Vec<DomainPackCompositionIssue> {
    let mut issues = Vec::new();
    validate_candidate(candidate, material, forge_core_version, &mut issues);
    finish_issues(issues)
}

/// Compose core plus caller-supplied candidates without IO or authority.
/// Input and diagnostic ordering do not affect output bytes or digest.
#[must_use]
pub fn compose_domain_packs(
    request: &DomainPackCompositionRequestDocument,
    materials: &[DomainPackCandidateMaterial<'_>],
) -> DomainPackCompositionProjectionDocument {
    let input = &request.domain_pack_composition_request;
    let mut issues = Vec::new();
    let mut gaps = Vec::new();

    if request.schema_version != DOMAIN_PACK_SCHEMA_VERSION {
        issue(
            &mut issues,
            DomainPackCompositionIssueCode::UnsupportedSchemaVersion,
            "schema_version",
            format!(
                "expected {DOMAIN_PACK_SCHEMA_VERSION}, found {}",
                request.schema_version
            ),
        );
    }
    if input.request_id.0.trim().is_empty() {
        issue(
            &mut issues,
            DomainPackCompositionIssueCode::InvalidIdentity,
            "domain_pack_composition_request.request_id",
            "request id must not be blank",
        );
    }
    if input.candidates.len() > MAX_DOMAIN_PACK_CANDIDATES {
        resource_issue(
            &mut issues,
            "domain_pack_composition_request.candidates",
            format!(
                "candidate count {} exceeds {MAX_DOMAIN_PACK_CANDIDATES}",
                input.candidates.len()
            ),
        );
    }
    if materials.len() > MAX_DOMAIN_PACK_CANDIDATES {
        resource_issue(
            &mut issues,
            "candidate_materials",
            format!(
                "material count {} exceeds {MAX_DOMAIN_PACK_CANDIDATES}",
                materials.len()
            ),
        );
    }
    validate_digest(
        &mut issues,
        "domain_pack_composition_request.core.bundle_digest",
        &input.core.bundle_digest,
    );
    validate_digest(
        &mut issues,
        "domain_pack_composition_request.core.policy_set_digest",
        &input.core.policy_set_digest,
    );
    if input.core.bundle_id != input.core.bundle.id {
        issue(
            &mut issues,
            DomainPackCompositionIssueCode::CoreShadow,
            "domain_pack_composition_request.core.bundle_id",
            "sealed core bundle id does not match the embedded core bundle",
        );
    }
    let core_canonical = canonical_digest(&input.core.bundle);
    if core_canonical.as_deref() != Some(input.core.bundle_digest.as_str()) {
        issue(
            &mut issues,
            DomainPackCompositionIssueCode::ContentBindingMismatch,
            "domain_pack_composition_request.core.bundle_digest",
            "core bundle canonical digest mismatch",
        );
    }
    if canonical_digest(&input.core.bundle.policies).as_deref()
        != Some(input.core.policy_set_digest.as_str())
    {
        issue(
            &mut issues,
            DomainPackCompositionIssueCode::ContentBindingMismatch,
            "domain_pack_composition_request.core.policy_set_digest",
            "core policy set canonical digest mismatch",
        );
    }

    let mut material_by_key = BTreeMap::new();
    let mut material_count_by_key = BTreeMap::<String, usize>::new();
    for material in materials {
        let key = material_key(material.publisher, material.name, material.version);
        *material_count_by_key.entry(key.clone()).or_default() += 1;
        if let Some(previous) = material_by_key.get(&key).copied() {
            if material_order_key(material) < material_order_key(&previous) {
                material_by_key.insert(key, *material);
            }
        } else {
            material_by_key.insert(key, *material);
        }
    }
    for (key, count) in material_count_by_key {
        if count > 1 {
            issue(
                &mut issues,
                DomainPackCompositionIssueCode::DuplicatePack,
                format!("candidate_materials.{key}"),
                format!("{count} raw materials declare the same exact pack identity"),
            );
        }
    }

    let mut candidate_by_coordinate = BTreeMap::new();
    let mut candidate_by_key = BTreeMap::new();
    let mut candidate_count_by_coordinate = BTreeMap::<String, usize>::new();
    for candidate in &input.candidates {
        let identity = &candidate.manifest.domain_pack_manifest.identity;
        let key = identity_key(identity);
        let coordinate = coordinate_key(&coordinate(identity));
        *candidate_count_by_coordinate
            .entry(coordinate.clone())
            .or_default() += 1;
        insert_candidate_deterministically(
            &mut candidate_by_coordinate,
            coordinate.clone(),
            candidate,
        );
        insert_candidate_deterministically(&mut candidate_by_key, key.clone(), candidate);
        match material_by_key.get(&key) {
            Some(material) => {
                validate_candidate(candidate, material, &input.forge_core_version, &mut issues)
            }
            None => issue(
                &mut issues,
                DomainPackCompositionIssueCode::ContentBindingMismatch,
                format!("domain_pack_composition_request.candidates.{key}"),
                "exact raw manifest/content/license sidecar is required",
            ),
        }
        if candidate
            .content
            .domain_pack_content
            .workflow_overlay
            .base_bundle_id
            != input.core.bundle_id
        {
            issue(&mut issues, DomainPackCompositionIssueCode::CoreShadow,
                format!("domain_pack_composition_request.candidates.{key}.content.workflow_overlay.base_bundle_id"),
                "pack overlay must target the exact sealed core bundle id");
        }
    }
    for key in material_by_key.keys() {
        if !candidate_by_key.contains_key(key) {
            issue(
                &mut issues,
                DomainPackCompositionIssueCode::ContentBindingMismatch,
                format!("candidate_materials.{key}"),
                "raw sidecar has no exact typed candidate",
            );
        }
    }
    for (coordinate, count) in candidate_count_by_coordinate {
        if count > 1 {
            issue(
                &mut issues,
                DomainPackCompositionIssueCode::DuplicatePack,
                format!("domain_pack_composition_request.candidates.{coordinate}"),
                format!("{count} candidates select the same pack coordinate"),
            );
        }
    }

    validate_dependencies_and_conflicts(&candidate_by_coordinate, &mut issues);
    let order = deterministic_pack_order(&candidate_by_coordinate, &mut issues);
    let mut replacement_by_source = BTreeMap::new();
    let mut replaced_targets = BTreeSet::new();
    validate_replacements(
        &candidate_by_coordinate,
        &mut replacement_by_source,
        &mut replaced_targets,
        &mut issues,
    );

    let mut contribution_index = Vec::new();
    let mut global_refs = core_refs(&input.core.bundle);
    let mut pack_namespaces = BTreeSet::new();
    let mut ordered_packs = Vec::new();
    let mut pack_policies = Vec::new();
    let mut total_contributions = 0usize;

    for (position, coordinate) in order.iter().enumerate() {
        let Some(candidate) = candidate_by_coordinate.get(coordinate).copied() else {
            continue;
        };
        let manifest = &candidate.manifest.domain_pack_manifest;
        let content = &candidate.content.domain_pack_content;
        let namespace = manifest.identity.namespace.0.as_str();
        if !pack_namespaces.insert(namespace.to_owned()) {
            issue(
                &mut issues,
                DomainPackCompositionIssueCode::DuplicateNamespace,
                format!("pack.{coordinate}.namespace"),
                format!("namespace {namespace} is selected more than once"),
            );
        }
        let content_digest = canonical_digest(&candidate.content).unwrap_or_default();
        let manifest_digest = canonical_digest(&candidate.manifest).unwrap_or_default();
        ordered_packs.push(DomainPackComposedIdentity {
            identity: manifest.identity.clone(),
            content_digest,
            manifest_digest,
            deterministic_order: u32::try_from(position).unwrap_or(u32::MAX),
        });
        let pack_ref = content.pack.clone();
        index_content(
            candidate,
            &pack_ref,
            &replacement_by_source,
            &replaced_targets,
            &mut contribution_index,
            &mut global_refs,
            &mut total_contributions,
            &mut issues,
        );
        let mut policies = content
            .workflow_overlay
            .policies
            .iter()
            .filter(|policy| !replaced_targets.contains(&format!("{coordinate}#{}", policy.id.0)))
            .cloned()
            .collect::<Vec<_>>();
        policies.sort_by(|a, b| {
            a.routing
                .priority
                .cmp(&b.routing.priority)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        pack_policies.extend(policies);
    }
    if total_contributions > MAX_DOMAIN_PACK_CONTRIBUTIONS {
        resource_issue(
            &mut issues,
            "composition.contributions",
            format!(
                "contribution count {total_contributions} exceeds {MAX_DOMAIN_PACK_CONTRIBUTIONS}"
            ),
        );
    }

    validate_domain_references(&candidate_by_coordinate, &global_refs, &mut issues);
    derive_requirement_gaps(
        &input.requirements,
        &candidate_by_coordinate,
        &global_refs,
        &mut gaps,
        &mut issues,
    );

    let mut composed_policies = input.core.bundle.policies.clone();
    let mut next_priority = composed_policies
        .iter()
        .map(|p| p.routing.priority)
        .max()
        .unwrap_or(0);
    for mut policy in pack_policies {
        match next_priority.checked_add(1) {
            Some(priority) => {
                next_priority = priority;
                policy.routing.priority = priority;
                composed_policies.push(policy);
            }
            None => {
                resource_issue(
                    &mut issues,
                    "composition.policies.routing.priority",
                    "effective routing priority exceeds u16",
                );
                break;
            }
        }
    }
    composed_policies.sort_by(|a, b| {
        a.routing
            .priority
            .cmp(&b.routing.priority)
            .then_with(|| a.id.0.cmp(&b.id.0))
    });
    let composed_bundle = WorkflowGovernanceBundle {
        id: StableId(format!(
            "bundle.domain-pack-candidate.{}",
            input.request_id.0
        )),
        policies: composed_policies,
    };
    let composed_document = WorkflowGovernanceBundleDocument {
        schema_version: WORKFLOW_GOVERNANCE_SCHEMA_VERSION.to_owned(),
        workflow_governance_bundle: composed_bundle.clone(),
    };
    for found in validate_workflow_governance_bundle(&composed_document) {
        issue(
            &mut issues,
            DomainPackCompositionIssueCode::InvalidComposedBundle,
            format!("composed_bundle.{}", found.path),
            found.message,
        );
    }

    contribution_index.sort_by(index_cmp);
    ordered_packs.sort_by_key(|p| p.deterministic_order);
    gaps.sort_by(|a, b| gap_key(a).cmp(&gap_key(b)));
    issues = finish_issues(issues);
    if issues.len() >= MAX_DOMAIN_PACK_DIAGNOSTICS {
        // The cap itself is deterministic; the resource diagnostic sorts with
        // all others and survives truncation because its path sorts first.
        issues.truncate(MAX_DOMAIN_PACK_DIAGNOSTICS);
    }
    let status = if issues.is_empty() && gaps.is_empty() {
        DomainPackCompositionStatus::Composable
    } else {
        DomainPackCompositionStatus::Blocked
    };
    let provided_domain_refs = sorted_unique(input.candidates.iter().flat_map(|c| {
        c.content
            .domain_pack_content
            .provided_domains
            .iter()
            .map(|d| d.id.clone())
    }));
    let declared_capability_refs = sorted_unique(input.candidates.iter().flat_map(|c| {
        c.content
            .domain_pack_content
            .provided_capabilities
            .iter()
            .map(|d| d.id.clone())
    }));

    #[derive(Serialize)]
    struct DigestSubject<'a> {
        schema_version: &'a str,
        request_id: &'a StableId,
        authority: DomainPackCandidateAuthority,
        core_bundle_digest: &'a str,
        core_policy_set_digest: &'a str,
        forge_core_version: &'a str,
        requirements: &'a DomainPackProjectRequirements,
        ordered_packs: &'a [DomainPackComposedIdentity],
        contribution_index: &'a [DomainPackContributionIndexEntry],
        provided_domain_refs: &'a [StableId],
        declared_capability_refs: &'a [StableId],
        composed_bundle: &'a WorkflowGovernanceBundle,
        gaps: &'a [DomainPackCompositionGap],
        issues: &'a [DomainPackCompositionIssue],
    }
    let composition_digest = canonical_digest(&DigestSubject {
        schema_version: DOMAIN_PACK_SCHEMA_VERSION,
        request_id: &input.request_id,
        authority: DomainPackCandidateAuthority::CandidateOnly,
        core_bundle_digest: &input.core.bundle_digest,
        core_policy_set_digest: &input.core.policy_set_digest,
        forge_core_version: &input.forge_core_version,
        requirements: &input.requirements,
        ordered_packs: &ordered_packs,
        contribution_index: &contribution_index,
        provided_domain_refs: &provided_domain_refs,
        declared_capability_refs: &declared_capability_refs,
        composed_bundle: &composed_bundle,
        gaps: &gaps,
        issues: &issues,
    })
    .unwrap_or_else(|| sha256_bytes(b"domain-pack-composition-encoding-failed"));

    DomainPackCompositionProjectionDocument {
        schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
        domain_pack_composition_projection: DomainPackCompositionProjection {
            request_id: input.request_id.clone(),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            status,
            core_bundle_digest: input.core.bundle_digest.clone(),
            ordered_packs,
            contribution_index,
            provided_domain_refs,
            declared_capability_refs,
            composed_bundle: issues.is_empty().then_some(composed_bundle),
            gaps,
            issues,
            composition_digest,
        },
    }
}

fn validate_candidate(
    candidate: &DomainPackCandidateInput,
    material: &DomainPackCandidateMaterial<'_>,
    forge_core_version: &str,
    issues: &mut Vec<DomainPackCompositionIssue>,
) {
    let manifest_doc = &candidate.manifest;
    let content_doc = &candidate.content;
    let manifest = &manifest_doc.domain_pack_manifest;
    let content = &content_doc.domain_pack_content;
    let key = identity_key(&manifest.identity);
    if manifest_doc.schema_version != DOMAIN_PACK_SCHEMA_VERSION
        || content_doc.schema_version != DOMAIN_PACK_SCHEMA_VERSION
    {
        issue(
            issues,
            DomainPackCompositionIssueCode::UnsupportedSchemaVersion,
            format!("pack.{key}.schema_version"),
            "manifest and content schema must equal the supported Domain Pack schema",
        );
    }
    validate_identity(&manifest.identity, issues);
    if content.pack.publisher != manifest.identity.publisher
        || content.pack.name != manifest.identity.name
        || content.pack.version != manifest.identity.version
        || content.namespace != manifest.identity.namespace
    {
        issue(
            issues,
            DomainPackCompositionIssueCode::InvalidIdentity,
            format!("pack.{key}.content.pack"),
            "manifest identity, content pack reference, and namespace must match exactly",
        );
    }
    if material.publisher != manifest.identity.publisher.0
        || material.name != manifest.identity.name.0
        || material.version != manifest.identity.version
    {
        issue(
            issues,
            DomainPackCompositionIssueCode::ContentBindingMismatch,
            format!("pack.{key}.material"),
            "raw material key does not match candidate identity",
        );
    }
    for (name, bytes) in [
        ("manifest", material.manifest_raw),
        ("content", material.content_raw),
        ("license", material.license_raw),
    ] {
        if bytes.len() > MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES {
            resource_issue(
                issues,
                format!("pack.{key}.{name}_raw"),
                format!(
                    "raw bytes {} exceed {MAX_DOMAIN_PACK_RAW_DOCUMENT_BYTES}",
                    bytes.len()
                ),
            );
        }
    }
    validate_binding(
        issues,
        &format!("pack.{key}.content"),
        &manifest.content.raw_sha256,
        &manifest.content.canonical_sha256,
        material.content_raw,
        content_doc,
    );
    validate_binding(
        issues,
        &format!("pack.{key}.manifest"),
        &candidate.manifest_binding.raw_sha256,
        &candidate.manifest_binding.canonical_sha256,
        material.manifest_raw,
        manifest_doc,
    );
    validate_typed_raw(
        issues,
        &format!("pack.{key}.content"),
        material.content_raw,
        content_doc,
    );
    validate_license_binding(
        issues,
        &format!("pack.{key}.provenance.license_text"),
        &manifest.provenance.license_text,
        material.license_raw,
    );
    validate_digest(
        issues,
        &format!("pack.{key}.provenance.source_digest"),
        &manifest.provenance.source_digest,
    );
    if manifest.provenance.source_uri.trim().is_empty()
        || manifest.provenance.source_revision.trim().is_empty()
        || manifest.provenance.authors.is_empty()
        || manifest
            .provenance
            .license_spdx_expression
            .trim()
            .is_empty()
        || !valid_repo_ref(&candidate.manifest_binding.artifact_ref.0)
        || !valid_repo_ref(&manifest.content.content_ref.0)
        || !valid_repo_ref(&manifest.provenance.license_text.artifact_ref.0)
    {
        issue(
            issues,
            DomainPackCompositionIssueCode::InvalidProvenance,
            format!("pack.{key}.provenance"),
            "source, revision, author, SPDX license, and safe relative artifact refs are required",
        );
    }
    if manifest.dependencies.len() > MAX_DOMAIN_PACK_DEPENDENCIES_PER_PACK {
        resource_issue(
            issues,
            format!("pack.{key}.dependencies"),
            format!(
                "dependency count {} exceeds {MAX_DOMAIN_PACK_DEPENDENCIES_PER_PACK}",
                manifest.dependencies.len()
            ),
        );
    }
    if Version::parse(&manifest.identity.version).is_err() {
        issue(
            issues,
            DomainPackCompositionIssueCode::InvalidIdentity,
            format!("pack.{key}.identity.version"),
            "pack version must be strict SemVer",
        );
    }
    validate_requirement(
        issues,
        &format!("pack.{key}.compatibility.pack_schema_requirement"),
        &manifest.compatibility.pack_schema_requirement,
        Some(&schema_version()),
    );
    let forge = Version::parse(forge_core_version).ok();
    validate_requirement(
        issues,
        &format!("pack.{key}.compatibility.forge_core_requirement"),
        &manifest.compatibility.forge_core_requirement,
        forge.as_ref(),
    );
    for (index, dependency) in manifest.dependencies.iter().enumerate() {
        validate_requirement(
            issues,
            &format!("pack.{key}.dependencies[{index}].version_requirement"),
            &dependency.version_requirement,
            None,
        );
        if let Some(digest) = &dependency.required_content_digest {
            validate_digest(
                issues,
                &format!("pack.{key}.dependencies[{index}].required_content_digest"),
                digest,
            );
        }
    }
    for (index, conflict) in manifest.conflicts.iter().enumerate() {
        validate_requirement(
            issues,
            &format!("pack.{key}.conflicts[{index}].version_requirement"),
            &conflict.version_requirement,
            None,
        );
    }
    for (index, slot) in manifest.replacement_slots.iter().enumerate() {
        validate_digest(
            issues,
            &format!("pack.{key}.replacement_slots[{index}].target_digest"),
            &slot.target_digest,
        );
        validate_requirement(
            issues,
            &format!("pack.{key}.replacement_slots[{index}].replacement_version_requirement"),
            &slot.replacement_version_requirement,
            None,
        );
    }
    for (index, declaration) in manifest.replacement_declarations.iter().enumerate() {
        validate_digest(
            issues,
            &format!("pack.{key}.replacement_declarations[{index}].target_digest"),
            &declaration.target_digest,
        );
    }
    // Fixture bytes are deliberately not part of the P6a material boundary.
    // The content binding authenticates this declaration, while a later
    // artifact-loading boundary must verify the declared raw/canonical hashes
    // against the actual sidecar before treating it as evidence.
    for fixture in &content.fixtures {
        let path = format!("pack.{key}.fixtures.{}.artifact", fixture.id.0);
        if !valid_repo_ref(&fixture.artifact.artifact_ref.0) {
            issue(
                issues,
                DomainPackCompositionIssueCode::ContentBindingMismatch,
                format!("{path}.artifact_ref"),
                "fixture artifact ref must be a safe repository-relative path",
            );
        }
        validate_digest(
            issues,
            &format!("{path}.raw_sha256"),
            &fixture.artifact.raw_sha256,
        );
        validate_digest(
            issues,
            &format!("{path}.canonical_sha256"),
            &fixture.artifact.canonical_sha256,
        );
    }
}

fn validate_identity(identity: &DomainPackIdentity, issues: &mut Vec<DomainPackCompositionIssue>) {
    let key = identity_key(identity);
    for (field, value) in [
        ("publisher", &identity.publisher.0),
        ("name", &identity.name.0),
        ("namespace", &identity.namespace.0),
    ] {
        if !valid_id(value) {
            issue(
                issues,
                DomainPackCompositionIssueCode::InvalidIdentity,
                format!("pack.{key}.identity.{field}"),
                "identity must be lowercase ASCII segments separated by dots or hyphens",
            );
        }
    }
    if identity.namespace.0 == "core"
        || identity.namespace.0.starts_with("core.")
        || identity.namespace.0 == "forge.core"
        || identity.namespace.0.starts_with("forge.core.")
    {
        issue(
            issues,
            DomainPackCompositionIssueCode::CoreShadow,
            format!("pack.{key}.identity.namespace"),
            "pack namespace cannot claim the sealed core namespace",
        );
    }
}

fn validate_binding<T: Serialize>(
    issues: &mut Vec<DomainPackCompositionIssue>,
    path: &str,
    expected_raw: &str,
    expected_canonical: &str,
    raw: &[u8],
    typed: &T,
) {
    validate_digest(issues, &format!("{path}.raw_sha256"), expected_raw);
    validate_digest(
        issues,
        &format!("{path}.canonical_sha256"),
        expected_canonical,
    );
    if sha256_bytes(raw) != expected_raw {
        issue(
            issues,
            DomainPackCompositionIssueCode::ContentBindingMismatch,
            format!("{path}.raw_sha256"),
            "raw sidecar digest mismatch",
        );
    }
    if canonical_digest(typed).as_deref() != Some(expected_canonical) {
        issue(
            issues,
            DomainPackCompositionIssueCode::ContentBindingMismatch,
            format!("{path}.canonical_sha256"),
            "typed canonical JCS digest mismatch",
        );
    }
}

fn validate_typed_raw<T>(
    issues: &mut Vec<DomainPackCompositionIssue>,
    path: &str,
    raw: &[u8],
    typed: &T,
) where
    T: serde::de::DeserializeOwned + PartialEq,
{
    let observed = std::str::from_utf8(raw)
        .ok()
        .and_then(|text| yaml_serde::from_str::<T>(text).ok());
    if observed.as_ref() != Some(typed) {
        issue(
            issues,
            DomainPackCompositionIssueCode::ContentBindingMismatch,
            format!("{path}.raw"),
            "raw sidecar does not parse to the exact embedded typed document",
        );
    }
}

fn validate_license_binding(
    issues: &mut Vec<DomainPackCompositionIssue>,
    path: &str,
    binding: &DomainPackArtifactBinding,
    raw: &[u8],
) {
    validate_digest(issues, &format!("{path}.raw_sha256"), &binding.raw_sha256);
    validate_digest(
        issues,
        &format!("{path}.canonical_sha256"),
        &binding.canonical_sha256,
    );
    if binding.raw_sha256 != sha256_bytes(raw) {
        issue(
            issues,
            DomainPackCompositionIssueCode::ContentBindingMismatch,
            path,
            "license artifact raw sidecar digest mismatch",
        );
    }
    let canonical = std::str::from_utf8(raw)
        .ok()
        .and_then(|text| yaml_serde::from_str::<serde_json::Value>(text).ok())
        .and_then(|value| canonical_digest(&value));
    if canonical.as_deref() != Some(binding.canonical_sha256.as_str()) {
        issue(
            issues,
            DomainPackCompositionIssueCode::ContentBindingMismatch,
            path,
            "license artifact canonical YAML semantics digest mismatch",
        );
    }
}

fn validate_dependencies_and_conflicts(
    candidates: &BTreeMap<String, &DomainPackCandidateInput>,
    issues: &mut Vec<DomainPackCompositionIssue>,
) {
    for (coordinate, candidate) in candidates {
        let manifest = &candidate.manifest.domain_pack_manifest;
        let mut seen = BTreeSet::new();
        for dependency in &manifest.dependencies {
            let dep_coordinate = coordinate_key(&dependency.pack);
            if !seen.insert(dep_coordinate.clone()) {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::DuplicatePack,
                    format!("pack.{coordinate}.dependencies.{dep_coordinate}"),
                    "dependency coordinate occurs more than once",
                );
            }
            let Some(selected) = candidates.get(&dep_coordinate) else {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::MissingDependency,
                    format!("pack.{coordinate}.dependencies.{dep_coordinate}"),
                    "exact dependency is not selected",
                );
                continue;
            };
            let selected_manifest = &selected.manifest.domain_pack_manifest;
            let matches = VersionReq::parse(&dependency.version_requirement)
                .ok()
                .zip(Version::parse(&selected_manifest.identity.version).ok())
                .is_some_and(|(req, version)| req.matches(&version));
            if !matches {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::IncompatibleDependency,
                    format!("pack.{coordinate}.dependencies.{dep_coordinate}"),
                    "selected dependency version does not satisfy requirement",
                );
            }
            if dependency
                .required_content_digest
                .as_ref()
                .is_some_and(|expected| {
                    canonical_digest(&selected.content).as_deref() != Some(expected.as_str())
                })
            {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::IncompatibleDependency,
                    format!(
                        "pack.{coordinate}.dependencies.{dep_coordinate}.required_content_digest"
                    ),
                    "selected dependency content digest does not match exact pin",
                );
            }
        }
        for conflict in &manifest.conflicts {
            let other_coordinate = coordinate_key(&conflict.pack);
            if let Some(other) = candidates.get(&other_coordinate) {
                let matched = VersionReq::parse(&conflict.version_requirement)
                    .ok()
                    .zip(Version::parse(&other.manifest.domain_pack_manifest.identity.version).ok())
                    .is_some_and(|(req, version)| req.matches(&version));
                if matched {
                    issue(
                        issues,
                        DomainPackCompositionIssueCode::DeclaredConflict,
                        format!("pack.{coordinate}.conflicts.{other_coordinate}"),
                        conflict.explanation.clone(),
                    );
                }
            }
        }
    }
}

fn deterministic_pack_order(
    candidates: &BTreeMap<String, &DomainPackCandidateInput>,
    issues: &mut Vec<DomainPackCompositionIssue>,
) -> Vec<String> {
    let mut indegree = candidates
        .keys()
        .map(|key| (key.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = candidates
        .keys()
        .map(|key| (key.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (coordinate, candidate) in candidates {
        for dep in &candidate.manifest.domain_pack_manifest.dependencies {
            let dependency = coordinate_key(&dep.pack);
            if candidates.contains_key(&dependency)
                && outgoing
                    .get_mut(&dependency)
                    .is_some_and(|set| set.insert(coordinate.clone()))
            {
                *indegree.get_mut(coordinate).expect("candidate is indexed") += 1;
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::new();
    let mut depth = candidates
        .keys()
        .map(|key| (key.clone(), 1usize))
        .collect::<BTreeMap<_, _>>();
    while let Some(key) = ready.pop_first() {
        order.push(key.clone());
        for next in outgoing.get(&key).into_iter().flatten() {
            let next_depth = depth[&key].saturating_add(1);
            depth
                .entry(next.clone())
                .and_modify(|found| *found = (*found).max(next_depth));
            let degree = indegree.get_mut(next).expect("outgoing target is indexed");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(next.clone());
            }
        }
    }
    if order.len() != candidates.len() {
        issue(
            issues,
            DomainPackCompositionIssueCode::DependencyCycle,
            "composition.dependencies",
            "dependency graph contains a cycle",
        );
        for key in candidates.keys() {
            if !order.contains(key) {
                order.push(key.clone());
            }
        }
    }
    if depth.values().copied().max().unwrap_or(0) > MAX_DOMAIN_PACK_DEPENDENCY_DEPTH {
        resource_issue(
            issues,
            "composition.dependencies.depth",
            format!("dependency depth exceeds {MAX_DOMAIN_PACK_DEPENDENCY_DEPTH}"),
        );
    }
    order
}

fn validate_replacements(
    candidates: &BTreeMap<String, &DomainPackCandidateInput>,
    replacements: &mut BTreeMap<String, StableId>,
    replaced_targets: &mut BTreeSet<String>,
    issues: &mut Vec<DomainPackCompositionIssue>,
) {
    let contribution_maps = candidates
        .iter()
        .map(|(coordinate, candidate)| (coordinate.clone(), contribution_map(candidate)))
        .collect::<BTreeMap<_, _>>();
    let mut targets = BTreeSet::new();
    for (source_coordinate, source) in candidates {
        let source_manifest = &source.manifest.domain_pack_manifest;
        for declaration in &source_manifest.replacement_declarations {
            let target_coordinate = coordinate_key(&declaration.target_pack);
            let target_key = format!("{target_coordinate}#{}", declaration.target_slot_ref.0);
            if !targets.insert(target_key.clone()) {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::ReplacementNotBilateral,
                    format!("pack.{source_coordinate}.replacements.{target_key}"),
                    "replacement slot has multiple claimants",
                );
            }
            let Some(target) = candidates.get(&target_coordinate) else {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::ReplacementNotBilateral,
                    format!("pack.{source_coordinate}.replacements.{target_key}"),
                    "target pack is not selected",
                );
                continue;
            };
            let Some(slot) = target
                .manifest
                .domain_pack_manifest
                .replacement_slots
                .iter()
                .find(|slot| slot.id == declaration.target_slot_ref)
            else {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::ReplacementNotBilateral,
                    format!("pack.{source_coordinate}.replacements.{target_key}"),
                    "target did not publish the named slot",
                );
                continue;
            };
            let source_coordinate_value = coordinate(&source_manifest.identity);
            let allowed = slot.allowed_replacers.contains(&source_coordinate_value)
                && VersionReq::parse(&slot.replacement_version_requirement)
                    .ok()
                    .zip(Version::parse(&source_manifest.identity.version).ok())
                    .is_some_and(|(req, version)| req.matches(&version));
            let target_map = &contribution_maps[&target_coordinate];
            let source_map = &contribution_maps[source_coordinate];
            let target_match =
                target_map
                    .get(&declaration.target_ref.0)
                    .is_some_and(|(kind, digest)| {
                        *kind == declaration.contribution_kind
                            && digest == &declaration.target_digest
                    });
            let replacement_match = source_map
                .get(&declaration.replacement_ref.0)
                .is_some_and(|(kind, _)| *kind == declaration.contribution_kind);
            if declaration.contribution_kind != DomainPackContributionKind::Policy {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::ReplacementTargetMismatch,
                    format!("pack.{source_coordinate}.replacements.{target_key}"),
                    "schema 0.1 composition supports whole-policy replacement only",
                );
            } else if !allowed
                || slot.contribution_kind != declaration.contribution_kind
                || slot.target_ref != declaration.target_ref
                || slot.target_digest != declaration.target_digest
                || !target_match
                || !replacement_match
            {
                issue(issues, DomainPackCompositionIssueCode::ReplacementTargetMismatch,
                    format!("pack.{source_coordinate}.replacements.{target_key}"),
                    "source declaration and target opt-in slot are not an exact compatible bilateral agreement");
            } else {
                let source_key = format!("{source_coordinate}#{}", declaration.replacement_ref.0);
                let target_contribution_key =
                    format!("{target_coordinate}#{}", declaration.target_ref.0);
                replacements.insert(source_key, declaration.target_ref.clone());
                replaced_targets.insert(target_contribution_key);
            }
        }
    }

    // A replacement chain is deterministic, but a cycle has no surviving
    // contribution and therefore cannot define candidate semantics.
    let edges = candidates
        .iter()
        .flat_map(|(source_coordinate, source)| {
            let replacements = &*replacements;
            source
                .manifest
                .domain_pack_manifest
                .replacement_declarations
                .iter()
                .filter_map(move |declaration| {
                    let source_key =
                        format!("{source_coordinate}#{}", declaration.replacement_ref.0);
                    replacements.get(&source_key).map(|_| {
                        (
                            source_key,
                            format!(
                                "{}#{}",
                                coordinate_key(&declaration.target_pack),
                                declaration.target_ref.0
                            ),
                        )
                    })
                })
        })
        .collect::<BTreeMap<_, _>>();
    for start in edges.keys() {
        let mut seen = BTreeSet::new();
        let mut current = start;
        while let Some(next) = edges.get(current) {
            if !seen.insert(current.clone()) || next == start {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::ReplacementTargetMismatch,
                    format!("composition.replacements.{start}"),
                    "replacement graph must be acyclic",
                );
                break;
            }
            current = next;
        }
    }
}

fn index_content(
    candidate: &DomainPackCandidateInput,
    pack_ref: &DomainPackVersionReference,
    replacements: &BTreeMap<String, StableId>,
    replaced_targets: &BTreeSet<String>,
    index: &mut Vec<DomainPackContributionIndexEntry>,
    global_refs: &mut BTreeSet<String>,
    count: &mut usize,
    issues: &mut Vec<DomainPackCompositionIssue>,
) {
    let content = &candidate.content.domain_pack_content;
    let coordinate = format!("{}::{}", pack_ref.publisher.0, pack_ref.name.0);
    let namespace_prefix = format!("{}.", content.namespace.0);
    let mut allow_foreign_namespace = false;
    macro_rules! add {
        ($kind:expr, $id:expr, $value:expr) => {{
            let id: &StableId = $id;
            *count += 1;
            let contribution_key = format!("{coordinate}#{}", id.0);
            if replaced_targets.contains(&contribution_key) {
                continue;
            }
            let replacement_target = replacements.get(&contribution_key);
            let explicitly_replaces_same_ref =
                replacement_target.is_some_and(|target| target == id);
            if !valid_id(&id.0)
                || (!id.0.starts_with(&namespace_prefix)
                    && !explicitly_replaces_same_ref
                    && !allow_foreign_namespace)
            {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::PackShadow,
                    format!("pack.{coordinate}.contribution.{}", id.0),
                    format!(
                        "contribution id must be owned by namespace {}",
                        content.namespace.0
                    ),
                );
            }
            if !global_refs.insert(id.0.clone()) {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::DuplicateContribution,
                    format!("pack.{coordinate}.contribution.{}", id.0),
                    "contribution shadows core or another pack",
                );
            }
            if let Some(target) = replacement_target {
                global_refs.insert(target.0.clone());
            }
            index.push(DomainPackContributionIndexEntry {
                pack: pack_ref.clone(),
                kind: $kind,
                contribution_ref: id.clone(),
                contribution_digest: canonical_digest($value).unwrap_or_default(),
                replaces_ref: replacement_target.cloned(),
            });
        }};
    }
    for policy in &content.workflow_overlay.policies {
        let policy_key = format!("{coordinate}#{}", policy.id.0);
        if replaced_targets.contains(&policy_key) {
            *count += 1
                + policy.obligations.len()
                + policy.claims.len()
                + policy.evaluators.len()
                + policy.capability_requirements.len()
                + 1;
            continue;
        }
        allow_foreign_namespace = replacements.contains_key(&policy_key);
        add!(DomainPackContributionKind::Policy, &policy.id, policy);
        for item in &policy.obligations {
            add!(DomainPackContributionKind::Obligation, &item.id, item);
        }
        for item in &policy.claims {
            add!(DomainPackContributionKind::Claim, &item.id, item);
        }
        add!(
            DomainPackContributionKind::Playbook,
            &policy.advisory_playbook.id,
            &policy.advisory_playbook
        );
        for item in &policy.evaluators {
            add!(DomainPackContributionKind::Evaluator, &item.id, item);
        }
        for item in &policy.capability_requirements {
            add!(DomainPackContributionKind::Capability, &item.id, item);
        }
        allow_foreign_namespace = false;
    }
    for item in &content.hazards {
        add!(DomainPackContributionKind::Hazard, &item.id, item);
    }
    for item in &content.lifecycle_models {
        add!(DomainPackContributionKind::LifecycleModel, &item.id, item);
    }
    for item in &content.evaluators {
        add!(DomainPackContributionKind::Evaluator, &item.id, item);
    }
    for item in &content.fixtures {
        add!(DomainPackContributionKind::Fixture, &item.id, item);
    }
    for item in &content.provided_capabilities {
        add!(DomainPackContributionKind::Capability, &item.id, item);
    }
    for item in &content.adapters {
        add!(DomainPackContributionKind::Adapter, &item.id, item);
    }
    for item in &content.provided_domains {
        add!(DomainPackContributionKind::Domain, &item.id, item);
    }
}

fn contribution_map(
    candidate: &DomainPackCandidateInput,
) -> BTreeMap<String, (DomainPackContributionKind, String)> {
    let mut entries = Vec::new();
    let content = &candidate.content.domain_pack_content;
    macro_rules! add {
        ($kind:expr, $iter:expr) => {
            for value in $iter {
                entries.push((
                    value.id.0.clone(),
                    ($kind, canonical_digest(value).unwrap_or_default()),
                ));
            }
        };
    }
    add!(
        DomainPackContributionKind::Policy,
        &content.workflow_overlay.policies
    );
    for policy in &content.workflow_overlay.policies {
        add!(DomainPackContributionKind::Obligation, &policy.obligations);
        add!(DomainPackContributionKind::Claim, &policy.claims);
        entries.push((
            policy.advisory_playbook.id.0.clone(),
            (
                DomainPackContributionKind::Playbook,
                canonical_digest(&policy.advisory_playbook).unwrap_or_default(),
            ),
        ));
        add!(DomainPackContributionKind::Evaluator, &policy.evaluators);
        add!(
            DomainPackContributionKind::Capability,
            &policy.capability_requirements
        );
    }
    add!(DomainPackContributionKind::Hazard, &content.hazards);
    add!(
        DomainPackContributionKind::LifecycleModel,
        &content.lifecycle_models
    );
    add!(DomainPackContributionKind::Evaluator, &content.evaluators);
    add!(DomainPackContributionKind::Fixture, &content.fixtures);
    add!(
        DomainPackContributionKind::Capability,
        &content.provided_capabilities
    );
    add!(DomainPackContributionKind::Adapter, &content.adapters);
    add!(
        DomainPackContributionKind::Domain,
        &content.provided_domains
    );
    entries.into_iter().collect()
}

fn validate_domain_references(
    candidates: &BTreeMap<String, &DomainPackCandidateInput>,
    refs: &BTreeSet<String>,
    issues: &mut Vec<DomainPackCompositionIssue>,
) {
    for (coordinate, candidate) in candidates {
        let content = &candidate.content.domain_pack_content;
        for domain in &content.provided_domains {
            refs_exist(
                coordinate,
                &domain.id,
                domain
                    .policy_refs
                    .iter()
                    .chain(&domain.hazard_refs)
                    .chain(&domain.lifecycle_model_refs),
                refs,
                issues,
            );
        }
        for capability in &content.provided_capabilities {
            refs_exist(
                coordinate,
                &capability.id,
                capability.evidence_rule_refs.iter(),
                refs,
                issues,
            );
        }
        for hazard in &content.hazards {
            validate_hazard(coordinate, hazard, refs, issues);
        }
        for lifecycle in &content.lifecycle_models {
            validate_lifecycle(coordinate, lifecycle, refs, issues);
        }
        for evaluator in &content.evaluators {
            validate_evaluator(coordinate, evaluator, refs, issues);
        }
        for fixture in &content.fixtures {
            refs_exist(
                coordinate,
                &fixture.id,
                fixture.subject_refs.iter(),
                refs,
                issues,
            );
        }
        for adapter in &content.adapters {
            validate_adapter(coordinate, adapter, refs, issues);
        }
    }
}

fn validate_hazard(
    coordinate: &str,
    hazard: &DomainHazard,
    refs: &BTreeSet<String>,
    issues: &mut Vec<DomainPackCompositionIssue>,
) {
    refs_exist(
        coordinate,
        &hazard.id,
        hazard
            .trigger_refs
            .iter()
            .chain(&hazard.mitigation_obligation_refs)
            .chain(&hazard.evidence_claim_refs),
        refs,
        issues,
    );
}
fn validate_lifecycle(
    coordinate: &str,
    model: &DomainLifecycleModel,
    refs: &BTreeSet<String>,
    issues: &mut Vec<DomainPackCompositionIssue>,
) {
    let states = model
        .states
        .iter()
        .map(|state| state.id.0.as_str())
        .collect::<BTreeSet<_>>();
    if !states.contains(model.initial_state_ref.0.as_str())
        || model
            .terminal_state_refs
            .iter()
            .any(|id| !states.contains(id.0.as_str()))
    {
        issue(
            issues,
            DomainPackCompositionIssueCode::InvalidLifecycleModel,
            format!("pack.{coordinate}.lifecycle.{}", model.id.0),
            "initial and terminal state refs must resolve locally",
        );
    }
    for state in &model.states {
        refs_exist(
            coordinate,
            &state.id,
            state
                .entry_obligation_refs
                .iter()
                .chain(&state.exit_claim_refs),
            refs,
            issues,
        );
    }
    for transition in &model.transitions {
        if !states.contains(transition.from_state_ref.0.as_str())
            || !states.contains(transition.to_state_ref.0.as_str())
        {
            issue(
                issues,
                DomainPackCompositionIssueCode::InvalidLifecycleModel,
                format!(
                    "pack.{coordinate}.lifecycle.{}.transition.{}",
                    model.id.0, transition.id.0
                ),
                "transition state refs must resolve locally",
            );
        }
        refs_exist(
            coordinate,
            &transition.id,
            transition
                .guard_claim_refs
                .iter()
                .chain(&transition.required_capability_refs),
            refs,
            issues,
        );
    }
    let mut reachable = BTreeSet::from([model.initial_state_ref.0.clone()]);
    loop {
        let before = reachable.len();
        for transition in &model.transitions {
            if reachable.contains(&transition.from_state_ref.0) {
                reachable.insert(transition.to_state_ref.0.clone());
            }
        }
        if reachable.len() == before {
            break;
        }
    }
    if model
        .states
        .iter()
        .any(|state| !reachable.contains(&state.id.0))
        || !model
            .terminal_state_refs
            .iter()
            .any(|terminal| reachable.contains(&terminal.0))
    {
        issue(
            issues,
            DomainPackCompositionIssueCode::InvalidLifecycleModel,
            format!("pack.{coordinate}.lifecycle.{}", model.id.0),
            "every lifecycle state and at least one terminal must be reachable from the initial state",
        );
    }
}
fn validate_evaluator(
    coordinate: &str,
    evaluator: &DomainEvaluatorDeclaration,
    refs: &BTreeSet<String>,
    issues: &mut Vec<DomainPackCompositionIssue>,
) {
    if evaluator.accepted_evidence_kinds.is_empty() {
        issue(
            issues,
            DomainPackCompositionIssueCode::InvalidEvaluatorDeclaration,
            format!("pack.{coordinate}.evaluator.{}", evaluator.id.0),
            "evaluator must accept at least one evidence kind",
        );
    }
    if let DomainEvaluatorImplementation::Adapter { adapter_ref, .. } = &evaluator.implementation {
        refs_exist(
            coordinate,
            &evaluator.id,
            std::iter::once(adapter_ref),
            refs,
            issues,
        );
    }
}
fn validate_adapter(
    coordinate: &str,
    adapter: &DomainAdapterDeclaration,
    refs: &BTreeSet<String>,
    issues: &mut Vec<DomainPackCompositionIssue>,
) {
    refs_exist(
        coordinate,
        &adapter.id,
        adapter.required_capability_refs.iter(),
        refs,
        issues,
    );
}
fn refs_exist<'a>(
    coordinate: &str,
    owner: &StableId,
    refs_iter: impl Iterator<Item = &'a StableId>,
    refs: &BTreeSet<String>,
    issues: &mut Vec<DomainPackCompositionIssue>,
) {
    for reference in refs_iter {
        if !refs.contains(&reference.0) {
            issue(
                issues,
                DomainPackCompositionIssueCode::DanglingReference,
                format!("pack.{coordinate}.reference.{}.{}", owner.0, reference.0),
                "reference is absent from core plus selected packs",
            );
        }
    }
}

fn derive_requirement_gaps(
    requirements: &DomainPackProjectRequirements,
    candidates: &BTreeMap<String, &DomainPackCandidateInput>,
    refs: &BTreeSet<String>,
    gaps: &mut Vec<DomainPackCompositionGap>,
    issues: &mut Vec<DomainPackCompositionIssue>,
) {
    for (field, value) in [
        ("project_id", &requirements.project_id.0),
        ("requirement_set_id", &requirements.requirement_set_id.0),
    ] {
        if !valid_id(value) {
            issue(
                issues,
                DomainPackCompositionIssueCode::InvalidIdentity,
                format!("requirements.{field}"),
                "persistent requirement identity must be a bounded lowercase ASCII stable id",
            );
        }
    }
    if requirements.required_domains.len() > MAX_DOMAIN_PACK_REQUIRED_DOMAINS {
        resource_issue(
            issues,
            "requirements.required_domains",
            format!(
                "required domain count {} exceeds {MAX_DOMAIN_PACK_REQUIRED_DOMAINS}",
                requirements.required_domains.len()
            ),
        );
    }
    let mut requirement_ids = BTreeSet::new();
    for requirement in requirements
        .required_domains
        .iter()
        .take(MAX_DOMAIN_PACK_REQUIRED_DOMAINS)
    {
        for (field, value) in [
            ("id", &requirement.id.0),
            ("domain_id", &requirement.domain_id.0),
        ] {
            if !valid_id(value) {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::InvalidIdentity,
                    format!("requirements.{}.{field}", requirement.id.0),
                    "persistent requirement reference must be a bounded lowercase ASCII stable id",
                );
            }
        }
        if !requirement_ids.insert(requirement.id.0.clone()) {
            issue(
                issues,
                DomainPackCompositionIssueCode::DuplicateContribution,
                format!("requirements.{}", requirement.id.0),
                "domain requirement id occurs more than once",
            );
        }
        if requirement.required_capability_refs.len() > MAX_DOMAIN_PACK_CAPABILITIES_PER_REQUIREMENT
        {
            resource_issue(
                issues,
                format!("requirements.{}.required_capability_refs", requirement.id.0),
                format!(
                    "required capability count {} exceeds {MAX_DOMAIN_PACK_CAPABILITIES_PER_REQUIREMENT}",
                    requirement.required_capability_refs.len()
                ),
            );
        }
        let req = VersionReq::parse(&requirement.pack_version_requirement);
        if req.is_err() {
            issue(
                issues,
                DomainPackCompositionIssueCode::InvalidVersionRequirement,
                format!("requirements.{}.pack_version_requirement", requirement.id.0),
                "invalid SemVer requirement",
            );
        }
        let supplied = candidates.values().any(|candidate| {
            candidate
                .content
                .domain_pack_content
                .provided_domains
                .iter()
                .any(|domain| domain.id == requirement.domain_id)
                && req
                    .as_ref()
                    .ok()
                    .zip(
                        Version::parse(&candidate.manifest.domain_pack_manifest.identity.version)
                            .ok()
                            .as_ref(),
                    )
                    .is_some_and(|(req, version)| req.matches(version))
        });
        if !supplied {
            gap(
                gaps,
                DomainPackCompositionGapCode::MissingDomain,
                &requirement.id,
                &requirement.domain_id,
                "required domain has no selected compatible pack",
            );
        }
        let mut capability_refs = BTreeSet::new();
        for capability in requirement
            .required_capability_refs
            .iter()
            .take(MAX_DOMAIN_PACK_CAPABILITIES_PER_REQUIREMENT)
        {
            if !valid_id(&capability.0) {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::InvalidIdentity,
                    format!(
                        "requirements.{}.required_capability_refs.{}",
                        requirement.id.0, capability.0
                    ),
                    "capability reference must be a bounded lowercase ASCII stable id",
                );
            }
            if !capability_refs.insert(capability.0.clone()) {
                issue(
                    issues,
                    DomainPackCompositionIssueCode::DuplicateContribution,
                    format!(
                        "requirements.{}.required_capability_refs.{}",
                        requirement.id.0, capability.0
                    ),
                    "capability requirement occurs more than once",
                );
            }
            if !refs.contains(&capability.0) {
                gap(
                    gaps,
                    DomainPackCompositionGapCode::MissingCapability,
                    &requirement.id,
                    capability,
                    "required domain capability is absent",
                );
            }
        }
    }
}

fn core_refs(bundle: &WorkflowGovernanceBundle) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for policy in &bundle.policies {
        refs.insert(policy.id.0.clone());
        refs.insert(policy.advisory_playbook.id.0.clone());
        for value in &policy.obligations {
            refs.insert(value.id.0.clone());
        }
        for value in &policy.claims {
            refs.insert(value.id.0.clone());
        }
        for value in &policy.evaluators {
            refs.insert(value.id.0.clone());
        }
        for value in &policy.capability_requirements {
            refs.insert(value.id.0.clone());
        }
        for value in &policy.decision_rules {
            refs.insert(value.id.0.clone());
        }
    }
    refs
}

fn coordinate(identity: &DomainPackIdentity) -> DomainPackCoordinate {
    DomainPackCoordinate {
        publisher: identity.publisher.clone(),
        name: identity.name.clone(),
    }
}
fn insert_candidate_deterministically<'a>(
    map: &mut BTreeMap<String, &'a DomainPackCandidateInput>,
    key: String,
    candidate: &'a DomainPackCandidateInput,
) {
    let candidate_key = canonical_digest(candidate).unwrap_or_default();
    match map.get(&key) {
        Some(previous) if canonical_digest(*previous).unwrap_or_default() <= candidate_key => {}
        _ => {
            map.insert(key, candidate);
        }
    }
}
fn material_order_key(material: &DomainPackCandidateMaterial<'_>) -> (String, String, String) {
    (
        sha256_bytes(material.manifest_raw),
        sha256_bytes(material.content_raw),
        sha256_bytes(material.license_raw),
    )
}
fn coordinate_key(value: &DomainPackCoordinate) -> String {
    format!("{}::{}", value.publisher.0, value.name.0)
}
fn identity_key(value: &DomainPackIdentity) -> String {
    material_key(&value.publisher.0, &value.name.0, &value.version)
}
fn material_key(publisher: &str, name: &str, version: &str) -> String {
    format!("{publisher}::{name}@{version}")
}
fn schema_version() -> Version {
    Version::new(0, 1, 0)
}
fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DOMAIN_PACK_IDENTIFIER_BYTES
        && value.is_ascii()
        && !value.starts_with(['.', '-'])
        && !value.ends_with(['.', '-'])
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
        && !value.contains("..")
        && !value.contains("--")
}
fn valid_repo_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.starts_with(['/', '\\'])
        && !value.contains('\\')
        && !value
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        && !value.contains(':')
}
fn validate_requirement(
    issues: &mut Vec<DomainPackCompositionIssue>,
    path: &str,
    raw: &str,
    selected: Option<&Version>,
) {
    match VersionReq::parse(raw) {
        Ok(req) if selected.is_some_and(|version| !req.matches(version)) => issue(
            issues,
            if path.contains("forge_core") {
                DomainPackCompositionIssueCode::IncompatibleForgeCore
            } else {
                DomainPackCompositionIssueCode::IncompatiblePackSchema
            },
            path,
            format!(
                "selected version {} does not satisfy {raw}",
                selected.expect("checked")
            ),
        ),
        Ok(_) if selected.is_none() && path.contains(".compatibility.") => issue(
            issues,
            if path.contains("forge_core") {
                DomainPackCompositionIssueCode::IncompatibleForgeCore
            } else {
                DomainPackCompositionIssueCode::IncompatiblePackSchema
            },
            path,
            "selected compatibility version is not valid SemVer",
        ),
        Ok(_) => {}
        Err(error) => issue(
            issues,
            DomainPackCompositionIssueCode::InvalidVersionRequirement,
            path,
            format!("invalid SemVer requirement: {error}"),
        ),
    }
}
fn validate_digest(issues: &mut Vec<DomainPackCompositionIssue>, path: &str, value: &str) {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        issue(
            issues,
            DomainPackCompositionIssueCode::ContentBindingMismatch,
            path,
            "digest must be sha256: followed by 64 lowercase hex characters",
        );
    }
}
fn canonical_digest<T: Serialize>(value: &T) -> Option<String> {
    serde_json_canonicalizer::to_vec(value)
        .ok()
        .map(|bytes| sha256_bytes(&bytes))
}
fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn sorted_unique(values: impl Iterator<Item = StableId>) -> Vec<StableId> {
    values
        .map(|id| (id.0.clone(), id))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}
fn index_cmp(
    a: &DomainPackContributionIndexEntry,
    b: &DomainPackContributionIndexEntry,
) -> std::cmp::Ordering {
    (
        a.pack.publisher.0.as_str(),
        a.pack.name.0.as_str(),
        a.pack.version.as_str(),
        a.kind,
        a.contribution_ref.0.as_str(),
    )
        .cmp(&(
            b.pack.publisher.0.as_str(),
            b.pack.name.0.as_str(),
            b.pack.version.as_str(),
            b.kind,
            b.contribution_ref.0.as_str(),
        ))
}
fn issue(
    issues: &mut Vec<DomainPackCompositionIssue>,
    code: DomainPackCompositionIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(DomainPackCompositionIssue {
        code,
        path: path.into(),
        message: message.into(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
    });
}
fn resource_issue(
    issues: &mut Vec<DomainPackCompositionIssue>,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issue(
        issues,
        DomainPackCompositionIssueCode::ResourceLimitExceeded,
        path,
        message,
    );
}
fn finish_issues(mut issues: Vec<DomainPackCompositionIssue>) -> Vec<DomainPackCompositionIssue> {
    issues.sort_by(|a, b| {
        (format!("{:?}", a.code), a.path.as_str(), a.message.as_str()).cmp(&(
            format!("{:?}", b.code),
            b.path.as_str(),
            b.message.as_str(),
        ))
    });
    issues.dedup_by(|a, b| a.code == b.code && a.path == b.path && a.message == b.message);
    issues.truncate(MAX_DOMAIN_PACK_DIAGNOSTICS);
    issues
}
fn gap(
    gaps: &mut Vec<DomainPackCompositionGap>,
    code: DomainPackCompositionGapCode,
    requirement: &StableId,
    subject: &StableId,
    message: impl Into<String>,
) {
    gaps.push(DomainPackCompositionGap {
        code,
        requirement_ref: requirement.clone(),
        subject_ref: subject.clone(),
        message: message.into(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
    });
}
fn gap_key(gap: &DomainPackCompositionGap) -> (String, &str, &str, &str) {
    (
        format!("{:?}", gap.code),
        gap.requirement_ref.0.as_str(),
        gap.subject_ref.0.as_str(),
        gap.message.as_str(),
    )
}
