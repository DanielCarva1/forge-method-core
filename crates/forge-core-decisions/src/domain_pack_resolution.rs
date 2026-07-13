//! Pure, deterministic P6b Domain Pack resolution.
//!
//! This module consumes an already obtained registry snapshot and performs no
//! IO and no cryptographic verification.  Matching a record is structural
//! admission evidence only; the trusted supply-chain boundary must verify the
//! snapshot and publisher signatures before relying on this projection.

use std::collections::{BTreeMap, BTreeSet};

use forge_core_contracts::{
    DomainPackCandidateAuthority, DomainPackCoordinate, DomainPackCredentialStatus,
    DomainPackExactLockDocument, DomainPackIdentity, DomainPackRegistryPackageRecord,
    DomainPackRejectedCandidate, DomainPackResolutionCandidate, DomainPackResolutionDependencyEdge,
    DomainPackResolutionIssue, DomainPackResolutionIssueCode, DomainPackResolutionProjection,
    DomainPackResolutionProjectionDocument, DomainPackResolutionRequestDocument,
    DomainPackResolutionRootReason, DomainPackResolutionStatus, DomainPackResolvedPackage,
    DomainPackSourceAssurance, DomainPackSupplyChainRegistryDocument, DomainPackVersionReference,
    DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION, DOMAIN_PACK_SCHEMA_VERSION,
};
use semver::{Version, VersionReq};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const MAX_DOMAIN_PACK_RESOLUTION_CANDIDATES: usize = 512;
pub const MAX_DOMAIN_PACK_RESOLUTION_VERSIONS_PER_COORDINATE: usize = 64;
pub const MAX_DOMAIN_PACK_RESOLUTION_ROOTS: usize = 64;
pub const MAX_DOMAIN_PACK_RESOLUTION_DEPENDENCIES_PER_PACK: usize = 64;
pub const MAX_DOMAIN_PACK_RESOLUTION_SEARCH_STATES: usize = 10_000;
pub const MAX_DOMAIN_PACK_RESOLUTION_DIAGNOSTICS: usize = 1_024;

#[derive(Clone)]
struct Admitted<'a> {
    candidate_index: usize,
    candidate: &'a DomainPackResolutionCandidate,
    record: &'a DomainPackRegistryPackageRecord,
    version: Version,
}

#[derive(Clone)]
struct Constraint {
    requirement: VersionReq,
    required_content_digest: Option<String>,
}

#[derive(Default)]
struct SearchFailure {
    code: Option<DomainPackResolutionIssueCode>,
    path: String,
    message: String,
    resource_exhausted: bool,
}

struct SearchContext<'a> {
    candidates: &'a BTreeMap<String, Vec<Admitted<'a>>>,
    locked: BTreeMap<String, String>,
    upgrade_targets: BTreeSet<String>,
    root_coordinates: BTreeSet<String>,
    states: usize,
}

/// Resolve exact Domain Pack versions against a structurally joined registry.
///
/// The result always remains `candidate_only`, and selected packages remain
/// `explicitly_untrusted` because this pure function has no cryptographic
/// authority. The lifecycle TCB may promote only exact records after consuming
/// an opaque verified supply-chain snapshot and then recomputes the digest.
#[must_use]
pub fn resolve_domain_packs(
    request: &DomainPackResolutionRequestDocument,
    registry: &DomainPackSupplyChainRegistryDocument,
) -> DomainPackResolutionProjectionDocument {
    let input = &request.domain_pack_resolution_request;
    let registry_value = &registry.domain_pack_supply_chain_registry;
    let mut issues = Vec::new();
    let mut rejection_codes = BTreeMap::<usize, BTreeSet<DomainPackResolutionIssueCode>>::new();

    validate_request_and_registry(request, registry, &mut issues);

    let equivocated_coordinates = equivocated_coordinates(registry, input);
    let mut admitted_by_coordinate = BTreeMap::<String, Vec<Admitted<'_>>>::new();
    let record_by_digest = registry_value
        .packages
        .iter()
        .map(|record| (record.record_digest.as_str(), record))
        .collect::<BTreeMap<_, _>>();
    let grant_by_id = registry_value
        .namespace_grants
        .iter()
        .map(|grant| (grant.grant_id.0.as_str(), grant))
        .collect::<BTreeMap<_, _>>();
    let credential_by_id = registry_value
        .publisher_credentials
        .iter()
        .map(|credential| (credential.credential_id.0.as_str(), credential))
        .collect::<BTreeMap<_, _>>();
    let revoked = registry_value
        .revocations
        .iter()
        .map(|revocation| revocation.record_digest.as_str())
        .collect::<BTreeSet<_>>();

    for (index, candidate) in input
        .candidates
        .iter()
        .take(MAX_DOMAIN_PACK_RESOLUTION_CANDIDATES)
        .enumerate()
    {
        let identity = candidate_identity(candidate);
        let coordinate = coordinate_key_parts(&identity.publisher.0, &identity.name.0);
        let reasons = rejection_codes.entry(index).or_default();
        validate_candidate_shape(candidate, &input.forge_core_version, reasons);

        if equivocated_coordinates.contains(&coordinate) {
            reasons.insert(DomainPackResolutionIssueCode::DuplicateVersionEquivocation);
        }
        let Some(record_digest) = candidate.registry_record_digest.as_deref() else {
            reasons.insert(DomainPackResolutionIssueCode::RegistryRecordMissing);
            continue;
        };
        let Some(record) = record_by_digest.get(record_digest).copied() else {
            reasons.insert(DomainPackResolutionIssueCode::RegistryRecordMissing);
            continue;
        };
        if revoked.contains(record_digest) {
            reasons.insert(DomainPackResolutionIssueCode::RevokedPackage);
        }
        if !record_matches_candidate(record, candidate) {
            reasons.insert(DomainPackResolutionIssueCode::RegistryRecordMismatch);
        }
        match grant_by_id
            .get(record.namespace_grant_id.0.as_str())
            .copied()
        {
            Some(grant)
                if grant.publisher == identity.publisher
                    && namespace_is_granted(&identity.namespace.0, &grant.namespace_prefix.0)
                    && grant.valid_from_unix <= registry_value.issued_at_unix
                    && registry_value.issued_at_unix <= grant.valid_until_unix => {}
            _ => {
                reasons.insert(DomainPackResolutionIssueCode::NamespaceNotGranted);
            }
        }
        match credential_by_id
            .get(record.publisher_credential_id.0.as_str())
            .copied()
        {
            Some(credential)
                if credential.publisher == identity.publisher
                    && credential.status == DomainPackCredentialStatus::Active
                    && credential.valid_from_unix <= registry_value.issued_at_unix
                    && registry_value.issued_at_unix <= credential.valid_until_unix => {}
            _ => {
                reasons.insert(DomainPackResolutionIssueCode::RegistryRecordMismatch);
            }
        }

        if reasons.is_empty() {
            if let Ok(version) = Version::parse(&identity.version) {
                admitted_by_coordinate
                    .entry(coordinate)
                    .or_default()
                    .push(Admitted {
                        candidate_index: index,
                        candidate,
                        record,
                        version,
                    });
            }
        }
    }

    for (coordinate, candidates) in &mut admitted_by_coordinate {
        candidates.sort_by(|left, right| {
            right.version.cmp(&left.version).then_with(|| {
                candidate_order_key(left.candidate).cmp(&candidate_order_key(right.candidate))
            })
        });
        if candidates.len() > MAX_DOMAIN_PACK_RESOLUTION_VERSIONS_PER_COORDINATE {
            resource_issue(
                &mut issues,
                &format!("candidates.{coordinate}"),
                "version count exceeds 64 for one coordinate",
            );
            for candidate in candidates
                .iter()
                .skip(MAX_DOMAIN_PACK_RESOLUTION_VERSIONS_PER_COORDINATE)
            {
                rejection_codes
                    .entry(candidate.candidate_index)
                    .or_default()
                    .insert(DomainPackResolutionIssueCode::ResourceLimitExceeded);
            }
            candidates.truncate(MAX_DOMAIN_PACK_RESOLUTION_VERSIONS_PER_COORDINATE);
        }
    }

    let mut selected = BTreeMap::<String, Admitted<'_>>::new();
    let mut edges = Vec::new();
    if issues.is_empty() {
        let mut constraints = BTreeMap::<String, Vec<Constraint>>::new();
        let mut upgrade_targets = BTreeSet::new();
        let mut root_coordinates = BTreeSet::new();
        for root in &input.roots {
            let key = coordinate_key(&root.pack);
            root_coordinates.insert(key.clone());
            if root.reason == DomainPackResolutionRootReason::UpgradeIntent {
                upgrade_targets.insert(key.clone());
            }
            if let Ok(requirement) = VersionReq::parse(&root.version_requirement) {
                constraints.entry(key).or_default().push(Constraint {
                    requirement,
                    required_content_digest: root.required_content_digest.clone(),
                });
            }
        }
        let locked = compatible_lock_preferences(input.current_lock.as_ref());
        let mut context = SearchContext {
            candidates: &admitted_by_coordinate,
            locked,
            upgrade_targets,
            root_coordinates,
            states: 0,
        };
        match search(&mut context, &constraints, BTreeMap::new()) {
            Ok(solution) => {
                selected = solution;
                match topological_edges_and_order(&selected) {
                    Ok(result) => edges = result.0,
                    Err(failure) => push_failure(&mut issues, failure),
                }
            }
            Err(failure) => push_failure(&mut issues, failure),
        }
    }

    let mut rejected = build_rejections(input, rejection_codes);
    rejected.sort_by(|left, right| {
        identity_key(&left.identity)
            .cmp(&identity_key(&right.identity))
            .then_with(|| left.package_digest.cmp(&right.package_digest))
    });
    finish_issues(&mut issues);

    let status = if issues.is_empty() {
        DomainPackResolutionStatus::Resolved
    } else {
        selected.clear();
        edges.clear();
        DomainPackResolutionStatus::Blocked
    };
    let selected_output = if status == DomainPackResolutionStatus::Resolved {
        build_selected(&selected)
    } else {
        Vec::new()
    };
    let digest = resolution_digest(
        request,
        &registry_value.snapshot_digest,
        status,
        &selected_output,
        &edges,
        &rejected,
        &issues,
    );

    DomainPackResolutionProjectionDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_resolution_projection: DomainPackResolutionProjection {
            request_id: input.request_id.clone(),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            status,
            selected: selected_output,
            dependency_edges: edges,
            rejected,
            issues,
            resolution_digest: digest,
        },
    }
}

fn validate_request_and_registry(
    request: &DomainPackResolutionRequestDocument,
    registry: &DomainPackSupplyChainRegistryDocument,
    issues: &mut Vec<DomainPackResolutionIssue>,
) {
    let input = &request.domain_pack_resolution_request;
    let registry_value = &registry.domain_pack_supply_chain_registry;
    if request.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION {
        issue(
            issues,
            DomainPackResolutionIssueCode::UnsupportedSchemaVersion,
            "schema_version",
            format!(
                "expected {DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION}, found {}",
                request.schema_version
            ),
        );
    }
    if registry.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION {
        issue(
            issues,
            DomainPackResolutionIssueCode::UnsupportedSchemaVersion,
            "registry.schema_version",
            format!(
                "expected {DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION}, found {}",
                registry.schema_version
            ),
        );
    }
    for (path, value) in [
        ("request_id", &input.request_id.0),
        ("project_id", &input.project_id.0),
        ("registry.registry_id", &registry_value.registry_id.0),
        ("registry.audience", &registry_value.audience.0),
    ] {
        if !valid_id(value) {
            issue(
                issues,
                DomainPackResolutionIssueCode::InvalidIdentity,
                path,
                "invalid stable id",
            );
        }
    }
    if input.project_id
        != input
            .requirements
            .domain_pack_project_requirements
            .project_id
    {
        issue(
            issues,
            DomainPackResolutionIssueCode::InvalidIdentity,
            "requirements.project_id",
            "requirements project differs from resolution project",
        );
    }
    if input.requirements.schema_version != DOMAIN_PACK_SCHEMA_VERSION {
        issue(
            issues,
            DomainPackResolutionIssueCode::UnsupportedSchemaVersion,
            "requirements.schema_version",
            "P6a requirements must use schema 0.1",
        );
    }
    if Version::parse(&input.forge_core_version).is_err() {
        issue(
            issues,
            DomainPackResolutionIssueCode::InvalidVersionRequirement,
            "forge_core_version",
            "forge core version must be strict semantic version",
        );
    }
    for (path, digest) in [
        (
            "registry_snapshot_digest",
            input.registry_snapshot_digest.as_str(),
        ),
        (
            "registry.snapshot_digest",
            registry_value.snapshot_digest.as_str(),
        ),
        ("core.bundle_digest", input.core.bundle_digest.as_str()),
        (
            "core.policy_set_digest",
            input.core.policy_set_digest.as_str(),
        ),
    ] {
        if !valid_digest(digest) {
            issue(
                issues,
                DomainPackResolutionIssueCode::InvalidDigest,
                path,
                "invalid sha256 digest",
            );
        }
    }
    if input.registry_snapshot_digest != registry_value.snapshot_digest {
        issue(
            issues,
            DomainPackResolutionIssueCode::RegistryDigestMismatch,
            "registry_snapshot_digest",
            "request does not bind the supplied registry snapshot",
        );
    }
    if registry_value.issued_at_unix > registry_value.expires_at_unix {
        issue(
            issues,
            DomainPackResolutionIssueCode::RegistryExpired,
            "registry.expires_at_unix",
            "registry validity interval is inverted",
        );
    }
    if input.roots.len() > MAX_DOMAIN_PACK_RESOLUTION_ROOTS {
        resource_issue(issues, "roots", "root count exceeds 64");
    }
    if input.candidates.len() > MAX_DOMAIN_PACK_RESOLUTION_CANDIDATES {
        resource_issue(issues, "candidates", "candidate count exceeds 512");
    }
    for (index, root) in input.roots.iter().enumerate() {
        if !valid_coordinate(&root.pack) {
            issue(
                issues,
                DomainPackResolutionIssueCode::InvalidIdentity,
                format!("roots.{index}.pack"),
                "invalid root coordinate",
            );
        }
        if VersionReq::parse(&root.version_requirement).is_err() {
            issue(
                issues,
                DomainPackResolutionIssueCode::InvalidVersionRequirement,
                format!("roots.{index}.version_requirement"),
                "invalid semantic-version requirement",
            );
        }
        if let Some(digest) = &root.required_content_digest {
            if !valid_digest(digest) {
                issue(
                    issues,
                    DomainPackResolutionIssueCode::InvalidDigest,
                    format!("roots.{index}.required_content_digest"),
                    "invalid sha256 digest",
                );
            }
        }
    }
    validate_current_lock(request, issues);
}

fn validate_current_lock(
    request: &DomainPackResolutionRequestDocument,
    issues: &mut Vec<DomainPackResolutionIssue>,
) {
    let input = &request.domain_pack_resolution_request;
    let Some(lock) = &input.current_lock else {
        return;
    };
    if lock.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION {
        issue(
            issues,
            DomainPackResolutionIssueCode::CurrentLockMismatch,
            "current_lock.schema_version",
            "current lock has an unsupported schema",
        );
    }
    if lock.domain_pack_exact_lock.payload.project_id != input.project_id
        || lock.domain_pack_exact_lock.payload.core.bundle_digest != input.core.bundle_digest
        || lock.domain_pack_exact_lock.payload.core.policy_set_digest
            != input.core.policy_set_digest
    {
        issue(
            issues,
            DomainPackResolutionIssueCode::CurrentLockMismatch,
            "current_lock.payload",
            "current lock is not compatible with project and sealed core",
        );
    }
    if !valid_digest(&lock.domain_pack_exact_lock.lock_digest) {
        issue(
            issues,
            DomainPackResolutionIssueCode::InvalidDigest,
            "current_lock.lock_digest",
            "invalid current lock digest",
        );
    }
}

fn validate_candidate_shape(
    candidate: &DomainPackResolutionCandidate,
    forge_core_version: &str,
    reasons: &mut BTreeSet<DomainPackResolutionIssueCode>,
) {
    let manifest_doc = &candidate.input.manifest;
    let content_doc = &candidate.input.content;
    let manifest = &manifest_doc.domain_pack_manifest;
    if manifest_doc.schema_version != DOMAIN_PACK_SCHEMA_VERSION
        || content_doc.schema_version != DOMAIN_PACK_SCHEMA_VERSION
    {
        reasons.insert(DomainPackResolutionIssueCode::UnsupportedSchemaVersion);
    }
    if !valid_identity(&manifest.identity)
        || content_doc.domain_pack_content.pack.publisher != manifest.identity.publisher
        || content_doc.domain_pack_content.pack.name != manifest.identity.name
        || content_doc.domain_pack_content.pack.version != manifest.identity.version
    {
        reasons.insert(DomainPackResolutionIssueCode::InvalidIdentity);
    }
    let Ok(version) = Version::parse(&manifest.identity.version) else {
        reasons.insert(DomainPackResolutionIssueCode::InvalidVersionRequirement);
        return;
    };
    if version.to_string() != manifest.identity.version {
        reasons.insert(DomainPackResolutionIssueCode::InvalidVersionRequirement);
    }
    let pack_schema = Version::parse(&format!("{DOMAIN_PACK_SCHEMA_VERSION}.0"))
        .expect("constant schema is semver-compatible");
    let forge_core = Version::parse(forge_core_version).ok();
    let pack_requirement = VersionReq::parse(&manifest.compatibility.pack_schema_requirement);
    let core_requirement = VersionReq::parse(&manifest.compatibility.forge_core_requirement);
    if pack_requirement.is_err()
        || core_requirement.is_err()
        || manifest
            .dependencies
            .iter()
            .any(|dependency| VersionReq::parse(&dependency.version_requirement).is_err())
        || manifest
            .conflicts
            .iter()
            .any(|conflict| VersionReq::parse(&conflict.version_requirement).is_err())
    {
        reasons.insert(DomainPackResolutionIssueCode::InvalidVersionRequirement);
    }
    if pack_requirement.is_ok_and(|requirement| !requirement.matches(&pack_schema))
        || core_requirement.is_ok_and(|requirement| {
            forge_core
                .as_ref()
                .is_none_or(|version| !requirement.matches(version))
        })
    {
        reasons.insert(DomainPackResolutionIssueCode::IncompatibleDependency);
    }
    if manifest.dependencies.len() > MAX_DOMAIN_PACK_RESOLUTION_DEPENDENCIES_PER_PACK {
        reasons.insert(DomainPackResolutionIssueCode::ResourceLimitExceeded);
    }
    let bindings = [
        &candidate.input.manifest_binding,
        &candidate.package.manifest,
        &candidate.package.license,
    ];
    if bindings.iter().any(|binding| {
        !valid_digest(&binding.raw_sha256) || !valid_digest(&binding.canonical_sha256)
    }) || !valid_digest(&candidate.package.package_digest)
        || !valid_digest(&candidate.package.content.raw_sha256)
        || !valid_digest(&candidate.package.content.canonical_sha256)
        || candidate.package.fixtures.iter().any(|binding| {
            !valid_digest(&binding.raw_sha256) || !valid_digest(&binding.canonical_sha256)
        })
    {
        reasons.insert(DomainPackResolutionIssueCode::InvalidDigest);
    }
    if candidate.package.manifest != candidate.input.manifest_binding
        || candidate.package.content != manifest.content
        || candidate.package.license != manifest.provenance.license_text
    {
        reasons.insert(DomainPackResolutionIssueCode::RegistryRecordMismatch);
    }
}

fn record_matches_candidate(
    record: &DomainPackRegistryPackageRecord,
    candidate: &DomainPackResolutionCandidate,
) -> bool {
    let fixture_digests = candidate
        .package
        .fixtures
        .iter()
        .map(|binding| binding.canonical_sha256.clone())
        .collect::<Vec<_>>();
    record.identity == *candidate_identity(candidate)
        && record.package_digest == candidate.package.package_digest
        && record.manifest_digest == candidate.package.manifest.canonical_sha256
        && record.content_digest == candidate.package.content.canonical_sha256
        && record.license_digest == candidate.package.license.canonical_sha256
        && sorted(record.fixture_digests.clone()) == sorted(fixture_digests)
        && valid_digest(&record.record_digest)
        && valid_digest(&record.package_digest)
        && valid_digest(&record.manifest_digest)
        && valid_digest(&record.content_digest)
        && valid_digest(&record.license_digest)
        && record
            .fixture_digests
            .iter()
            .all(|digest| valid_digest(digest))
}

fn equivocated_coordinates(
    registry: &DomainPackSupplyChainRegistryDocument,
    request: &forge_core_contracts::DomainPackResolutionRequest,
) -> BTreeSet<String> {
    let mut by_exact = BTreeMap::<String, BTreeSet<String>>::new();
    for record in &registry.domain_pack_supply_chain_registry.packages {
        by_exact
            .entry(identity_key(&record.identity))
            .or_default()
            .insert(record_equivocation_key(record));
    }
    for candidate in &request.candidates {
        by_exact
            .entry(identity_key(candidate_identity(candidate)))
            .or_default()
            .insert(candidate_equivocation_key(candidate));
    }
    by_exact
        .into_iter()
        .filter(|(_, variants)| variants.len() > 1)
        .filter_map(|(identity, _)| {
            let mut parts = identity.splitn(3, "::");
            Some(format!("{}::{}", parts.next()?, parts.next()?))
        })
        .collect()
}

fn search<'a>(
    context: &mut SearchContext<'a>,
    constraints: &BTreeMap<String, Vec<Constraint>>,
    selected: BTreeMap<String, Admitted<'a>>,
) -> Result<BTreeMap<String, Admitted<'a>>, SearchFailure> {
    if context.states >= MAX_DOMAIN_PACK_RESOLUTION_SEARCH_STATES {
        return Err(SearchFailure {
            code: Some(DomainPackResolutionIssueCode::ResourceLimitExceeded),
            path: "resolution.search_states".to_owned(),
            message: "deterministic search exhausted 10000 states".to_owned(),
            resource_exhausted: true,
        });
    }
    context.states += 1;

    if let Some(key) = constraints
        .keys()
        .find(|key| !selected.contains_key(*key))
        .cloned()
    {
        let Some(universe) = context.candidates.get(&key) else {
            return Err(missing_failure(context, &key));
        };
        let coordinate_constraints = constraints.get(&key).cloned().unwrap_or_default();
        let mut choices = universe
            .iter()
            .filter(|candidate| satisfies(candidate, &coordinate_constraints))
            .cloned()
            .collect::<Vec<_>>();
        if choices.is_empty() {
            return Err(missing_failure(context, &key));
        }
        order_choices(context, &key, &mut choices);
        let mut last_failure = None;
        for choice in choices {
            if conflicts_with_selected(&choice, &selected) {
                last_failure = Some(SearchFailure {
                    code: Some(DomainPackResolutionIssueCode::DeclaredConflict),
                    path: format!("selection.{key}"),
                    message: "candidate declares a conflict with the selected graph".to_owned(),
                    resource_exhausted: false,
                });
                continue;
            }
            let mut next_selected = selected.clone();
            next_selected.insert(key.clone(), choice.clone());
            if selected_conflict(&next_selected) {
                last_failure = Some(SearchFailure {
                    code: Some(DomainPackResolutionIssueCode::DeclaredConflict),
                    path: format!("selection.{key}"),
                    message: "selected graph contains a declared conflict".to_owned(),
                    resource_exhausted: false,
                });
                continue;
            }
            let mut next_constraints = constraints.clone();
            let mut dependency_error = None;
            for dependency in &choice
                .candidate
                .input
                .manifest
                .domain_pack_manifest
                .dependencies
            {
                let Ok(requirement) = VersionReq::parse(&dependency.version_requirement) else {
                    dependency_error = Some(SearchFailure {
                        code: Some(DomainPackResolutionIssueCode::InvalidVersionRequirement),
                        path: format!("selection.{key}.dependencies"),
                        message: "dependency requirement is invalid".to_owned(),
                        resource_exhausted: false,
                    });
                    break;
                };
                next_constraints
                    .entry(coordinate_key(&dependency.pack))
                    .or_default()
                    .push(Constraint {
                        requirement,
                        required_content_digest: dependency.required_content_digest.clone(),
                    });
            }
            if let Some(failure) = dependency_error {
                last_failure = Some(failure);
                continue;
            }
            if next_selected.iter().any(|(selected_key, candidate)| {
                next_constraints
                    .get(selected_key)
                    .is_some_and(|requirements| !satisfies(candidate, requirements))
            }) {
                last_failure = Some(SearchFailure {
                    code: Some(DomainPackResolutionIssueCode::IncompatibleDependency),
                    path: format!("selection.{key}"),
                    message: "candidate makes an already selected dependency incompatible"
                        .to_owned(),
                    resource_exhausted: false,
                });
                continue;
            }
            match search(context, &next_constraints, next_selected) {
                Ok(solution) => return Ok(solution),
                Err(failure) if failure.resource_exhausted => return Err(failure),
                Err(failure) => last_failure = Some(failure),
            }
        }
        Err(last_failure.unwrap_or_else(|| missing_failure(context, &key)))
    } else {
        if selected_conflict(&selected) {
            return Err(SearchFailure {
                code: Some(DomainPackResolutionIssueCode::DeclaredConflict),
                path: "selection".to_owned(),
                message: "selected graph contains a declared conflict".to_owned(),
                resource_exhausted: false,
            });
        }
        if graph_has_cycle(&selected) {
            return Err(SearchFailure {
                code: Some(DomainPackResolutionIssueCode::DependencyCycle),
                path: "selection".to_owned(),
                message: "selected dependency graph contains a cycle".to_owned(),
                resource_exhausted: false,
            });
        }
        Ok(selected)
    }
}

fn satisfies(candidate: &Admitted<'_>, constraints: &[Constraint]) -> bool {
    constraints.iter().all(|constraint| {
        constraint.requirement.matches(&candidate.version)
            && constraint
                .required_content_digest
                .as_ref()
                .is_none_or(|digest| {
                    digest == &candidate.candidate.package.content.canonical_sha256
                })
            && (candidate.version.pre.is_empty()
                || requirement_explicitly_allows_prerelease(&constraint.requirement))
    })
}

fn order_choices(context: &SearchContext<'_>, key: &str, choices: &mut Vec<Admitted<'_>>) {
    let locked_version = (!context.upgrade_targets.contains(key))
        .then(|| context.locked.get(key))
        .flatten();
    choices.sort_by(|left, right| {
        let left_locked =
            locked_version.is_some_and(|version| left.version.to_string() == *version);
        let right_locked =
            locked_version.is_some_and(|version| right.version.to_string() == *version);
        right_locked
            .cmp(&left_locked)
            .then_with(|| right.version.cmp(&left.version))
            .then_with(|| {
                candidate_order_key(left.candidate).cmp(&candidate_order_key(right.candidate))
            })
    });
}

fn conflicts_with_selected(
    candidate: &Admitted<'_>,
    selected: &BTreeMap<String, Admitted<'_>>,
) -> bool {
    candidate
        .candidate
        .input
        .manifest
        .domain_pack_manifest
        .conflicts
        .iter()
        .any(|conflict| {
            selected
                .get(&coordinate_key(&conflict.pack))
                .is_some_and(|other| {
                    VersionReq::parse(&conflict.version_requirement)
                        .is_ok_and(|requirement| requirement.matches(&other.version))
                })
        })
}

fn selected_conflict(selected: &BTreeMap<String, Admitted<'_>>) -> bool {
    selected
        .values()
        .any(|candidate| conflicts_with_selected(candidate, selected))
}

fn graph_has_cycle(selected: &BTreeMap<String, Admitted<'_>>) -> bool {
    fn visit(
        key: &str,
        selected: &BTreeMap<String, Admitted<'_>>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if visited.contains(key) {
            return false;
        }
        if !visiting.insert(key.to_owned()) {
            return true;
        }
        if let Some(candidate) = selected.get(key) {
            for dependency in &candidate
                .candidate
                .input
                .manifest
                .domain_pack_manifest
                .dependencies
            {
                if visit(
                    &coordinate_key(&dependency.pack),
                    selected,
                    visiting,
                    visited,
                ) {
                    return true;
                }
            }
        }
        visiting.remove(key);
        visited.insert(key.to_owned());
        false
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    selected
        .keys()
        .any(|key| visit(key, selected, &mut visiting, &mut visited))
}

fn topological_edges_and_order(
    selected: &BTreeMap<String, Admitted<'_>>,
) -> Result<(Vec<DomainPackResolutionDependencyEdge>, Vec<String>), SearchFailure> {
    let mut dependency_count = selected
        .iter()
        .map(|(key, candidate)| {
            (
                key.clone(),
                candidate
                    .candidate
                    .input
                    .manifest
                    .domain_pack_manifest
                    .dependencies
                    .len(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<String, BTreeSet<String>>::new();
    let mut edges = Vec::new();
    for (from_key, candidate) in selected {
        let from = version_reference(candidate_identity(candidate.candidate));
        for dependency in &candidate
            .candidate
            .input
            .manifest
            .domain_pack_manifest
            .dependencies
        {
            let to_key = coordinate_key(&dependency.pack);
            let Some(to_candidate) = selected.get(&to_key) else {
                return Err(SearchFailure {
                    code: Some(DomainPackResolutionIssueCode::MissingDependency),
                    path: format!("selection.{from_key}.dependencies"),
                    message: "selected dependency is absent".to_owned(),
                    resource_exhausted: false,
                });
            };
            dependents
                .entry(to_key)
                .or_default()
                .insert(from_key.clone());
            edges.push(DomainPackResolutionDependencyEdge {
                from: from.clone(),
                to: version_reference(candidate_identity(to_candidate.candidate)),
                required_content_digest: dependency.required_content_digest.clone(),
            });
        }
    }
    edges.sort_by_key(edge_key);
    let mut ready = dependency_count
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::new();
    while let Some(key) = ready.pop_first() {
        order.push(key.clone());
        for dependent in dependents.get(&key).into_iter().flatten() {
            if let Some(count) = dependency_count.get_mut(dependent) {
                *count -= 1;
                if *count == 0 {
                    ready.insert(dependent.clone());
                }
            }
        }
    }
    if order.len() != selected.len() {
        return Err(SearchFailure {
            code: Some(DomainPackResolutionIssueCode::DependencyCycle),
            path: "selection".to_owned(),
            message: "selected dependency graph contains a cycle".to_owned(),
            resource_exhausted: false,
        });
    }
    Ok((edges, order))
}

fn build_selected(selected: &BTreeMap<String, Admitted<'_>>) -> Vec<DomainPackResolvedPackage> {
    let Ok((_, order)) = topological_edges_and_order(selected) else {
        return Vec::new();
    };
    order
        .into_iter()
        .enumerate()
        .filter_map(|(position, key)| selected.get(&key).map(|candidate| (position, candidate)))
        .map(|(position, candidate)| DomainPackResolvedPackage {
            identity: candidate_identity(candidate.candidate).clone(),
            package: candidate.candidate.package.clone(),
            registry_record_digest: candidate.record.record_digest.clone(),
            namespace_grant_id: candidate.record.namespace_grant_id.clone(),
            // This pure module has performed only a structural registry join.
            // The TCB may promote this field after consuming an opaque,
            // cryptographically verified supply-chain snapshot.
            source_assurance: DomainPackSourceAssurance::ExplicitlyUntrusted,
            dependencies: sorted_dependencies(
                candidate
                    .candidate
                    .input
                    .manifest
                    .domain_pack_manifest
                    .dependencies
                    .clone(),
            ),
            deterministic_order: u32::try_from(position).unwrap_or(u32::MAX),
        })
        .collect()
}

fn build_rejections(
    input: &forge_core_contracts::DomainPackResolutionRequest,
    codes: BTreeMap<usize, BTreeSet<DomainPackResolutionIssueCode>>,
) -> Vec<DomainPackRejectedCandidate> {
    codes
        .into_iter()
        .filter(|(_, reasons)| !reasons.is_empty())
        .filter_map(|(index, reasons)| {
            input
                .candidates
                .get(index)
                .map(|candidate| (candidate, reasons))
        })
        .map(|(candidate, reasons)| DomainPackRejectedCandidate {
            identity: candidate_identity(candidate).clone(),
            package_digest: candidate.package.package_digest.clone(),
            reasons: reasons.into_iter().collect(),
        })
        .collect()
}

fn compatible_lock_preferences(
    lock: Option<&DomainPackExactLockDocument>,
) -> BTreeMap<String, String> {
    lock.into_iter()
        .flat_map(|lock| &lock.domain_pack_exact_lock.payload.packages)
        .map(|package| {
            (
                coordinate_key_parts(&package.identity.publisher.0, &package.identity.name.0),
                package.identity.version.clone(),
            )
        })
        .collect()
}

fn missing_failure(context: &SearchContext<'_>, key: &str) -> SearchFailure {
    let root = context.root_coordinates.contains(key);
    SearchFailure {
        code: Some(if root {
            DomainPackResolutionIssueCode::MissingRoot
        } else {
            DomainPackResolutionIssueCode::MissingDependency
        }),
        path: format!("selection.{key}"),
        message: if root {
            "no admitted candidate satisfies the root".to_owned()
        } else {
            "no admitted candidate satisfies the dependency".to_owned()
        },
        resource_exhausted: false,
    }
}

fn push_failure(issues: &mut Vec<DomainPackResolutionIssue>, failure: SearchFailure) {
    issue(
        issues,
        failure
            .code
            .unwrap_or(DomainPackResolutionIssueCode::IncompatibleDependency),
        failure.path,
        failure.message,
    );
}

fn issue(
    issues: &mut Vec<DomainPackResolutionIssue>,
    code: DomainPackResolutionIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    if issues.len() < MAX_DOMAIN_PACK_RESOLUTION_DIAGNOSTICS {
        issues.push(DomainPackResolutionIssue {
            code,
            path: path.into(),
            message: message.into(),
        });
    }
}

fn resource_issue(issues: &mut Vec<DomainPackResolutionIssue>, path: &str, message: &str) {
    issue(
        issues,
        DomainPackResolutionIssueCode::ResourceLimitExceeded,
        path,
        message,
    );
}

fn finish_issues(issues: &mut Vec<DomainPackResolutionIssue>) {
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.message.cmp(&right.message))
    });
    issues.dedup();
    issues.truncate(MAX_DOMAIN_PACK_RESOLUTION_DIAGNOSTICS);
}

#[derive(Serialize)]
struct ResolutionDigestSubject<'a> {
    schema_version: &'static str,
    request: DomainPackResolutionRequestDocument,
    registry_snapshot_digest: &'a str,
    authority: DomainPackCandidateAuthority,
    status: DomainPackResolutionStatus,
    selected: &'a [DomainPackResolvedPackage],
    dependency_edges: &'a [DomainPackResolutionDependencyEdge],
    rejected: &'a [DomainPackRejectedCandidate],
    issues: &'a [DomainPackResolutionIssue],
}

fn resolution_digest(
    request: &DomainPackResolutionRequestDocument,
    registry_snapshot_digest: &str,
    status: DomainPackResolutionStatus,
    selected: &[DomainPackResolvedPackage],
    dependency_edges: &[DomainPackResolutionDependencyEdge],
    rejected: &[DomainPackRejectedCandidate],
    issues: &[DomainPackResolutionIssue],
) -> String {
    canonical_digest(&ResolutionDigestSubject {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
        request: normalized_request(request),
        registry_snapshot_digest,
        authority: DomainPackCandidateAuthority::CandidateOnly,
        status,
        selected,
        dependency_edges,
        rejected,
        issues,
    })
    .unwrap_or_else(|| sha256_bytes(b"domain-pack-resolution-encoding-failed"))
}

/// Recompute the canonical digest after a trusted boundary has promoted only
/// the exact selected records admitted by an opaque supply-chain snapshot.
/// The digest itself remains evidence and grants no authority.
#[must_use]
pub fn domain_pack_resolution_projection_digest(
    request: &DomainPackResolutionRequestDocument,
    registry_snapshot_digest: &str,
    projection: &DomainPackResolutionProjection,
) -> String {
    resolution_digest(
        request,
        registry_snapshot_digest,
        projection.status,
        &projection.selected,
        &projection.dependency_edges,
        &projection.rejected,
        &projection.issues,
    )
}

fn normalized_request(
    request: &DomainPackResolutionRequestDocument,
) -> DomainPackResolutionRequestDocument {
    let mut normalized = request.clone();
    let input = &mut normalized.domain_pack_resolution_request;
    input.roots.sort_by(|left, right| {
        coordinate_key(&left.pack)
            .cmp(&coordinate_key(&right.pack))
            .then_with(|| left.version_requirement.cmp(&right.version_requirement))
            .then_with(|| {
                left.required_content_digest
                    .cmp(&right.required_content_digest)
            })
            .then_with(|| format!("{:?}", left.reason).cmp(&format!("{:?}", right.reason)))
    });
    input.candidates.sort_by_key(candidate_order_key);
    input
        .requirements
        .domain_pack_project_requirements
        .required_domains
        .sort_by(|left, right| left.id.cmp(&right.id));
    for requirement in &mut input
        .requirements
        .domain_pack_project_requirements
        .required_domains
    {
        requirement.required_capability_refs.sort();
    }
    if let Some(lock) = &mut input.current_lock {
        let payload = &mut lock.domain_pack_exact_lock.payload;
        payload.roots.sort_by(|left, right| {
            coordinate_key(&left.pack)
                .cmp(&coordinate_key(&right.pack))
                .then_with(|| left.version_requirement.cmp(&right.version_requirement))
        });
        payload.packages.sort_by(|left, right| {
            identity_key(&left.identity).cmp(&identity_key(&right.identity))
        });
        payload
            .verified_capability_bindings
            .sort_by(|left, right| left.binding_id.cmp(&right.binding_id));
        payload.unresolved_capability_gaps.sort_by(|left, right| {
            canonical_digest(left)
                .unwrap_or_default()
                .cmp(&canonical_digest(right).unwrap_or_default())
        });
    }
    normalized
}

fn requirement_explicitly_allows_prerelease(requirement: &VersionReq) -> bool {
    requirement
        .comparators
        .iter()
        .any(|comparator| !comparator.pre.is_empty())
}

fn namespace_is_granted(namespace: &str, prefix: &str) -> bool {
    valid_id(namespace)
        && valid_id(prefix)
        && (namespace == prefix
            || namespace
                .strip_prefix(prefix)
                .is_some_and(|suffix| suffix.starts_with('.')))
}

fn valid_identity(identity: &DomainPackIdentity) -> bool {
    valid_id(&identity.publisher.0) && valid_id(&identity.name.0) && valid_id(&identity.namespace.0)
}

fn valid_coordinate(coordinate: &DomainPackCoordinate) -> bool {
    valid_id(&coordinate.publisher.0) && valid_id(&coordinate.name.0)
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

fn candidate_identity(candidate: &DomainPackResolutionCandidate) -> &DomainPackIdentity {
    &candidate.input.manifest.domain_pack_manifest.identity
}

fn coordinate_key(coordinate: &DomainPackCoordinate) -> String {
    coordinate_key_parts(&coordinate.publisher.0, &coordinate.name.0)
}

fn coordinate_key_parts(publisher: &str, name: &str) -> String {
    format!("{publisher}::{name}")
}

fn identity_key(identity: &DomainPackIdentity) -> String {
    format!(
        "{}::{}::{}",
        identity.publisher.0, identity.name.0, identity.version
    )
}

fn version_reference(identity: &DomainPackIdentity) -> DomainPackVersionReference {
    DomainPackVersionReference {
        publisher: identity.publisher.clone(),
        name: identity.name.clone(),
        version: identity.version.clone(),
    }
}

fn record_equivocation_key(record: &DomainPackRegistryPackageRecord) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        record.package_digest,
        record.manifest_digest,
        record.content_digest,
        record.license_digest,
        record.record_digest
    )
}

fn candidate_equivocation_key(candidate: &DomainPackResolutionCandidate) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        candidate.package.package_digest,
        candidate.package.manifest.canonical_sha256,
        candidate.package.content.canonical_sha256,
        candidate.package.license.canonical_sha256,
        candidate.registry_record_digest.as_deref().unwrap_or("")
    )
}

fn candidate_order_key(candidate: &DomainPackResolutionCandidate) -> String {
    canonical_digest(candidate).unwrap_or_default()
}

fn edge_key(edge: &DomainPackResolutionDependencyEdge) -> String {
    format!(
        "{}::{}::{}>{}::{}::{}|{}",
        edge.from.publisher.0,
        edge.from.name.0,
        edge.from.version,
        edge.to.publisher.0,
        edge.to.name.0,
        edge.to.version,
        edge.required_content_digest.as_deref().unwrap_or("")
    )
}

fn sorted<T: Ord>(mut values: Vec<T>) -> Vec<T> {
    values.sort();
    values
}

fn sorted_dependencies(
    mut dependencies: Vec<forge_core_contracts::DomainPackDependency>,
) -> Vec<forge_core_contracts::DomainPackDependency> {
    dependencies.sort_by(|left, right| {
        coordinate_key(&left.pack)
            .cmp(&coordinate_key(&right.pack))
            .then_with(|| left.version_requirement.cmp(&right.version_requirement))
            .then_with(|| {
                left.required_content_digest
                    .cmp(&right.required_content_digest)
            })
    });
    dependencies
}

fn canonical_digest<T: Serialize>(value: &T) -> Option<String> {
    serde_json_canonicalizer::to_vec(value)
        .ok()
        .map(|bytes| sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
