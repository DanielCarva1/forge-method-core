use forge_core_contracts::{
    DomainPackCandidateAuthority, DomainPackCompositionGapCode, DomainPackCompositionIssueCode,
    DomainPackCompositionRequestDocument, DomainPackCompositionStatus, DomainPackContributionKind,
    DomainPackCoordinate, DomainPackDependency, DomainPackReplacementDeclaration,
    DomainPackReplacementSlot, StableId,
};
use forge_core_decisions::{
    compose_domain_packs, validate_domain_pack_candidate, DomainPackCandidateMaterial,
};
use sha2::{Digest, Sha256};

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_request(name: &str) -> DomainPackCompositionRequestDocument {
    let path = repo_root()
        .join("docs/fixtures/domain-pack-v0/requests")
        .join(name);
    yaml_serde::from_str(&std::fs::read_to_string(path).expect("request fixture"))
        .expect("typed request")
}

fn read_projection(name: &str) -> forge_core_contracts::DomainPackCompositionProjectionDocument {
    let path = repo_root()
        .join("docs/fixtures/domain-pack-v0/projections")
        .join(name);
    yaml_serde::from_str(&std::fs::read_to_string(path).expect("projection fixture"))
        .expect("typed projection")
}

#[derive(Clone)]
struct OwnedMaterial {
    manifest: Vec<u8>,
    content: Vec<u8>,
    license: Vec<u8>,
}

fn owned_materials(request: &DomainPackCompositionRequestDocument) -> Vec<OwnedMaterial> {
    request
        .domain_pack_composition_request
        .candidates
        .iter()
        .map(|candidate| {
            let manifest = &candidate.manifest.domain_pack_manifest;
            OwnedMaterial {
                manifest: std::fs::read(
                    repo_root()
                        .join("docs/fixtures/domain-pack-v0/manifests")
                        .join(format!("{}.yaml", manifest.identity.name.0)),
                )
                .expect("exact manifest bytes"),
                content: std::fs::read(repo_root().join(&manifest.content.content_ref.0))
                    .expect("exact content bytes"),
                license: std::fs::read(
                    repo_root().join(&manifest.provenance.license_text.artifact_ref.0),
                )
                .expect("exact license bytes"),
            }
        })
        .collect()
}

fn borrowed_materials<'a>(
    request: &'a DomainPackCompositionRequestDocument,
    owned: &'a [OwnedMaterial],
) -> Vec<DomainPackCandidateMaterial<'a>> {
    request
        .domain_pack_composition_request
        .candidates
        .iter()
        .zip(owned)
        .map(|(candidate, owned)| {
            let identity = &candidate.manifest.domain_pack_manifest.identity;
            DomainPackCandidateMaterial {
                publisher: &identity.publisher.0,
                name: &identity.name.0,
                version: &identity.version,
                manifest_raw: &owned.manifest,
                content_raw: &owned.content,
                license_raw: &owned.license,
            }
        })
        .collect()
}

fn compose(
    request: &DomainPackCompositionRequestDocument,
    owned: &[OwnedMaterial],
) -> forge_core_contracts::DomainPackCompositionProjectionDocument {
    compose_domain_packs(request, &borrowed_materials(request, owned))
}

fn canonical_digest<T: serde::Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn rebind_content(
    request: &mut DomainPackCompositionRequestDocument,
    owned: &mut [OwnedMaterial],
    index: usize,
) {
    let candidate = &mut request.domain_pack_composition_request.candidates[index];
    let bytes = yaml_serde::to_string(&candidate.content)
        .expect("serialize content")
        .into_bytes();
    candidate.manifest.domain_pack_manifest.content.raw_sha256 =
        format!("sha256:{:x}", Sha256::digest(&bytes));
    candidate
        .manifest
        .domain_pack_manifest
        .content
        .canonical_sha256 = canonical_digest(&candidate.content);
    owned[index].content = bytes;
    rebind_manifest(request, owned, index);
}

fn rebind_manifest(
    request: &mut DomainPackCompositionRequestDocument,
    owned: &mut [OwnedMaterial],
    index: usize,
) {
    let candidate = &mut request.domain_pack_composition_request.candidates[index];
    let bytes = yaml_serde::to_string(&candidate.manifest)
        .expect("serialize manifest")
        .into_bytes();
    candidate.manifest_binding.raw_sha256 = format!("sha256:{:x}", Sha256::digest(&bytes));
    candidate.manifest_binding.canonical_sha256 = canonical_digest(&candidate.manifest);
    owned[index].manifest = bytes;
}

#[test]
fn exact_two_pack_composition_is_deterministic_and_candidate_only() {
    let request = read_request("neutral-two-pack.yaml");
    let owned = owned_materials(&request);
    let projection = compose(&request, &owned);
    let result = &projection.domain_pack_composition_projection;
    assert_eq!(
        projection,
        read_projection("neutral-two-pack.expected.yaml")
    );

    assert_eq!(
        result.authority,
        DomainPackCandidateAuthority::CandidateOnly
    );
    assert_eq!(result.status, DomainPackCompositionStatus::Composable);
    assert!(result.issues.is_empty(), "{:?}", result.issues);
    assert!(result.gaps.is_empty(), "{:?}", result.gaps);
    assert_eq!(result.ordered_packs.len(), 2);
    assert_eq!(result.ordered_packs[0].identity.name.0, "foundation");
    assert_eq!(
        result.ordered_packs[1].identity.name.0,
        "assurance-extension"
    );
    assert_eq!(
        result
            .composed_bundle
            .as_ref()
            .expect("candidate bundle")
            .policies
            .len(),
        2
    );

    let mut reversed = request.clone();
    reversed
        .domain_pack_composition_request
        .candidates
        .reverse();
    let mut reversed_owned = owned.clone();
    reversed_owned.reverse();
    let reversed_projection = compose(&reversed, &reversed_owned);
    assert_eq!(projection, reversed_projection);
}

#[test]
fn removing_required_pack_preserves_explicit_domain_and_capability_gaps() {
    let request = read_request("neutral-extension-removed.yaml");
    let owned = owned_materials(&request);
    let projection = compose(&request, &owned);
    let result = &projection.domain_pack_composition_projection;
    assert_eq!(
        projection,
        read_projection("neutral-extension-removed.expected.yaml")
    );

    assert_eq!(result.status, DomainPackCompositionStatus::Blocked);
    assert!(result.issues.is_empty(), "{:?}", result.issues);
    assert!(result
        .gaps
        .iter()
        .any(|gap| gap.code == DomainPackCompositionGapCode::MissingDomain));
    assert!(result
        .gaps
        .iter()
        .any(|gap| gap.code == DomainPackCompositionGapCode::MissingCapability));
    assert!(
        result.composed_bundle.is_some(),
        "gaps are not malformed input"
    );
}

#[test]
fn tampered_or_missing_raw_sidecar_fails_closed() {
    let request = read_request("neutral-two-pack.yaml");
    let mut owned = owned_materials(&request);
    owned[0].content.extend_from_slice(b"\n# tampered\n");
    let projection = compose(&request, &owned);
    assert_eq!(
        projection.domain_pack_composition_projection.status,
        DomainPackCompositionStatus::Blocked
    );
    assert!(projection
        .domain_pack_composition_projection
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackCompositionIssueCode::ContentBindingMismatch));

    let candidate = &request.domain_pack_composition_request.candidates[0];
    let identity = &candidate.manifest.domain_pack_manifest.identity;
    let missing = DomainPackCandidateMaterial {
        publisher: &identity.publisher.0,
        name: &identity.name.0,
        version: &identity.version,
        manifest_raw: b"",
        content_raw: b"",
        license_raw: b"",
    };
    let issues = validate_domain_pack_candidate(candidate, &missing, "0.5.0");
    assert!(issues
        .iter()
        .any(|issue| issue.code == DomainPackCompositionIssueCode::ContentBindingMismatch));
}

#[test]
fn dependency_cycles_duplicates_and_core_shadowing_are_rejected() {
    let mut request = read_request("neutral-two-pack.yaml");
    let extension = request.domain_pack_composition_request.candidates[1]
        .manifest
        .domain_pack_manifest
        .identity
        .clone();
    request.domain_pack_composition_request.candidates[0]
        .manifest
        .domain_pack_manifest
        .dependencies
        .push(DomainPackDependency {
            pack: DomainPackCoordinate {
                publisher: extension.publisher.clone(),
                name: extension.name.clone(),
            },
            version_requirement: "^1.0".to_owned(),
            required_content_digest: None,
        });
    let mut owned = owned_materials(&request);
    rebind_manifest(&mut request, &mut owned, 0);
    let projection = compose(&request, &owned);
    assert!(projection
        .domain_pack_composition_projection
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackCompositionIssueCode::DependencyCycle));

    let mut duplicate = read_request("neutral-two-pack.yaml");
    duplicate.domain_pack_composition_request.candidates[1]
        .manifest
        .domain_pack_manifest
        .identity
        .namespace = duplicate.domain_pack_composition_request.candidates[0]
        .manifest
        .domain_pack_manifest
        .identity
        .namespace
        .clone();
    let mut owned = owned_materials(&duplicate);
    rebind_manifest(&mut duplicate, &mut owned, 1);
    let projection = compose(&duplicate, &owned);
    assert!(projection
        .domain_pack_composition_projection
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackCompositionIssueCode::DuplicateNamespace));

    let mut shadow = read_request("neutral-two-pack.yaml");
    shadow.domain_pack_composition_request.candidates[0]
        .content
        .domain_pack_content
        .workflow_overlay
        .base_bundle_id = forge_core_contracts::StableId("attacker.core".to_owned());
    let mut owned = owned_materials(&shadow);
    rebind_content(&mut shadow, &mut owned, 0);
    let projection = compose(&shadow, &owned);
    assert!(projection
        .domain_pack_composition_projection
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackCompositionIssueCode::CoreShadow));
}

#[test]
fn dangling_refs_and_cross_pack_duplicate_contributions_fail_closed() {
    let mut request = read_request("neutral-two-pack.yaml");
    request.domain_pack_composition_request.candidates[0]
        .content
        .domain_pack_content
        .provided_domains[0]
        .hazard_refs
        .push(forge_core_contracts::StableId(
            "sample.foundation.hazard.missing".to_owned(),
        ));
    let mut owned = owned_materials(&request);
    rebind_content(&mut request, &mut owned, 0);
    let projection = compose(&request, &owned);
    assert!(projection
        .domain_pack_composition_projection
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackCompositionIssueCode::DanglingReference));

    let mut duplicate = read_request("neutral-two-pack.yaml");
    let first_id = duplicate.domain_pack_composition_request.candidates[0]
        .content
        .domain_pack_content
        .workflow_overlay
        .policies[0]
        .id
        .clone();
    duplicate.domain_pack_composition_request.candidates[1]
        .content
        .domain_pack_content
        .workflow_overlay
        .policies[0]
        .id = first_id;
    let mut owned = owned_materials(&duplicate);
    rebind_content(&mut duplicate, &mut owned, 1);
    let projection = compose(&duplicate, &owned);
    assert!(projection
        .domain_pack_composition_projection
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackCompositionIssueCode::DuplicateContribution));
}

#[test]
fn duplicate_candidate_material_diagnostics_and_digest_ignore_permutation() {
    let request = read_request("neutral-two-pack.yaml");
    let owned = owned_materials(&request);
    let base = borrowed_materials(&request, &owned);
    let identity = &request.domain_pack_composition_request.candidates[0]
        .manifest
        .domain_pack_manifest
        .identity;
    let alternate_license = b"spdx: Apache-2.0\nnotice: duplicate candidate material\n";
    let duplicate = DomainPackCandidateMaterial {
        publisher: &identity.publisher.0,
        name: &identity.name.0,
        version: &identity.version,
        manifest_raw: base[0].manifest_raw,
        content_raw: base[0].content_raw,
        license_raw: alternate_license,
    };

    let mut forward = base.clone();
    forward.push(duplicate);
    let mut reversed = forward.clone();
    reversed.reverse();

    let forward_projection = compose_domain_packs(&request, &forward);
    let reversed_projection = compose_domain_packs(&request, &reversed);
    assert_eq!(forward_projection, reversed_projection);
    assert!(forward_projection
        .domain_pack_composition_projection
        .issues
        .iter()
        .any(|issue| {
            issue.code == DomainPackCompositionIssueCode::DuplicatePack
                && issue.path.starts_with("candidate_materials.")
                && !issue.path.contains('[')
        }));
}

#[test]
fn persistent_requirement_ids_and_semantic_limits_fail_closed() {
    let mut request = read_request("neutral-two-pack.yaml");
    let requirements = &mut request.domain_pack_composition_request.requirements;
    requirements.project_id = StableId("INVALID PROJECT".to_owned());
    requirements.requirement_set_id = StableId("requirements/unsafe".to_owned());
    requirements.required_domains[0].id = StableId("Requirement.Bad".to_owned());
    requirements.required_domains[0].domain_id = StableId("../domain".to_owned());
    requirements.required_domains[0].required_capability_refs = (0
        ..=forge_core_decisions::domain_pack::MAX_DOMAIN_PACK_CAPABILITIES_PER_REQUIREMENT)
        .map(|index| StableId(format!("sample.capability.required-{index}")))
        .collect();
    requirements.required_domains[0].required_capability_refs[0] =
        StableId("CAPABILITY.INVALID".to_owned());

    let owned = owned_materials(&request);
    let projection = compose(&request, &owned);
    let issues = &projection.domain_pack_composition_projection.issues;
    assert!(issues.iter().any(|issue| issue.code
        == DomainPackCompositionIssueCode::InvalidIdentity
        && issue.path == "requirements.project_id"));
    assert!(issues.iter().any(|issue| issue.code
        == DomainPackCompositionIssueCode::InvalidIdentity
        && issue.path == "requirements.requirement_set_id"));
    assert!(issues.iter().any(|issue| issue.code
        == DomainPackCompositionIssueCode::InvalidIdentity
        && issue.path.ends_with(".id")));
    assert!(issues.iter().any(|issue| issue.code
        == DomainPackCompositionIssueCode::InvalidIdentity
        && issue.path.ends_with(".domain_id")));
    assert!(issues.iter().any(|issue| issue.code
        == DomainPackCompositionIssueCode::ResourceLimitExceeded
        && issue.path.ends_with(".required_capability_refs")));
    assert_eq!(
        projection.domain_pack_composition_projection.status,
        DomainPackCompositionStatus::Blocked
    );
}

#[test]
fn fixture_artifact_refs_and_declared_digests_are_validated_without_io() {
    let mut request = read_request("neutral-two-pack.yaml");
    let fixture = &mut request.domain_pack_composition_request.candidates[0]
        .content
        .domain_pack_content
        .fixtures[0];
    fixture.artifact.artifact_ref.0 = "../outside.yaml".to_owned();
    fixture.artifact.raw_sha256 = "sha256:ABC".to_owned();
    fixture.artifact.canonical_sha256 = "not-a-digest".to_owned();
    let mut owned = owned_materials(&request);
    rebind_content(&mut request, &mut owned, 0);

    let projection = compose(&request, &owned);
    let issues = &projection.domain_pack_composition_projection.issues;
    assert!(issues.iter().any(|issue| {
        issue.code == DomainPackCompositionIssueCode::ContentBindingMismatch
            && issue.path.ends_with(".artifact_ref")
    }));
    assert!(issues.iter().any(|issue| {
        issue.code == DomainPackCompositionIssueCode::ContentBindingMismatch
            && issue.path.ends_with(".raw_sha256")
    }));
    assert!(issues.iter().any(|issue| {
        issue.code == DomainPackCompositionIssueCode::ContentBindingMismatch
            && issue.path.ends_with(".canonical_sha256")
    }));
}

fn configure_policy_replacement(
    request: &mut DomainPackCompositionRequestDocument,
    owned: &mut [OwnedMaterial],
) {
    let target_policy = request.domain_pack_composition_request.candidates[0]
        .content
        .domain_pack_content
        .workflow_overlay
        .policies[0]
        .clone();
    let target_ref = target_policy.id.clone();
    let target_digest = canonical_digest(&target_policy);
    let source_identity = request.domain_pack_composition_request.candidates[1]
        .manifest
        .domain_pack_manifest
        .identity
        .clone();
    let target_coordinate = DomainPackCoordinate {
        publisher: request.domain_pack_composition_request.candidates[0]
            .manifest
            .domain_pack_manifest
            .identity
            .publisher
            .clone(),
        name: request.domain_pack_composition_request.candidates[0]
            .manifest
            .domain_pack_manifest
            .identity
            .name
            .clone(),
    };
    request.domain_pack_composition_request.candidates[0]
        .manifest
        .domain_pack_manifest
        .replacement_slots
        .push(DomainPackReplacementSlot {
            id: StableId("sample.foundation.replacement.delivery-policy".to_owned()),
            contribution_kind: DomainPackContributionKind::Policy,
            target_ref: target_ref.clone(),
            target_digest: target_digest.clone(),
            allowed_replacers: vec![DomainPackCoordinate {
                publisher: source_identity.publisher.clone(),
                name: source_identity.name.clone(),
            }],
            replacement_version_requirement: "^1.1".to_owned(),
        });
    let target_claim_ref = target_policy.claims[0].id.clone();
    let source = &mut request.domain_pack_composition_request.candidates[1];
    source.content.domain_pack_content.workflow_overlay.policies[0] = target_policy;
    source.content.domain_pack_content.provided_domains[0].policy_refs = vec![target_ref.clone()];
    source.content.domain_pack_content.provided_domains[0]
        .hazard_refs
        .clear();
    source.content.domain_pack_content.provided_domains[0]
        .lifecycle_model_refs
        .clear();
    source.content.domain_pack_content.provided_capabilities[0].evidence_rule_refs =
        vec![target_claim_ref];
    source.content.domain_pack_content.hazards.clear();
    source.content.domain_pack_content.lifecycle_models.clear();
    source
        .manifest
        .domain_pack_manifest
        .replacement_declarations
        .push(DomainPackReplacementDeclaration {
            target_pack: target_coordinate,
            target_slot_ref: StableId("sample.foundation.replacement.delivery-policy".to_owned()),
            contribution_kind: DomainPackContributionKind::Policy,
            target_ref: target_ref.clone(),
            target_digest,
            replacement_ref: target_ref,
        });
    rebind_manifest(request, owned, 0);
    rebind_content(request, owned, 1);
}

#[test]
fn bilateral_policy_replacement_substitutes_while_hostile_variants_fail_closed() {
    let mut request = read_request("neutral-two-pack.yaml");
    let mut owned = owned_materials(&request);
    configure_policy_replacement(&mut request, &mut owned);
    let projection = compose(&request, &owned);
    let result = &projection.domain_pack_composition_projection;
    assert!(result.issues.is_empty(), "{:?}", result.issues);
    let policies = &result.composed_bundle.as_ref().unwrap().policies;
    assert_eq!(policies.len(), 1, "target policy must be removed");
    assert_eq!(
        result
            .contribution_index
            .iter()
            .filter(|entry| entry.kind == DomainPackContributionKind::Policy)
            .count(),
        1
    );
    assert!(result.contribution_index.iter().any(|entry| {
        entry.kind == DomainPackContributionKind::Policy
            && entry.replaces_ref.as_ref() == Some(&entry.contribution_ref)
            && entry.pack.name.0 == "assurance-extension"
    }));

    let mut unilateral = request.clone();
    unilateral.domain_pack_composition_request.candidates[0]
        .manifest
        .domain_pack_manifest
        .replacement_slots
        .clear();
    let mut unilateral_owned = owned_materials(&unilateral);
    rebind_manifest(&mut unilateral, &mut unilateral_owned, 0);
    rebind_content(&mut unilateral, &mut unilateral_owned, 1);
    assert!(compose(&unilateral, &unilateral_owned)
        .domain_pack_composition_projection
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackCompositionIssueCode::ReplacementNotBilateral));

    let mut mismatch = request.clone();
    mismatch.domain_pack_composition_request.candidates[1]
        .manifest
        .domain_pack_manifest
        .replacement_declarations[0]
        .target_digest = format!("sha256:{}", "0".repeat(64));
    let mut mismatch_owned = owned_materials(&mismatch);
    rebind_manifest(&mut mismatch, &mut mismatch_owned, 1);
    assert!(compose(&mismatch, &mismatch_owned)
        .domain_pack_composition_projection
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackCompositionIssueCode::ReplacementTargetMismatch));

    let mut multiple = request.clone();
    let declaration = multiple.domain_pack_composition_request.candidates[1]
        .manifest
        .domain_pack_manifest
        .replacement_declarations[0]
        .clone();
    multiple.domain_pack_composition_request.candidates[1]
        .manifest
        .domain_pack_manifest
        .replacement_declarations
        .push(declaration);
    let mut multiple_owned = owned_materials(&multiple);
    rebind_manifest(&mut multiple, &mut multiple_owned, 1);
    assert!(compose(&multiple, &multiple_owned)
        .domain_pack_composition_projection
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackCompositionIssueCode::ReplacementNotBilateral));

    let mut cycle = request.clone();
    let shared_ref = cycle.domain_pack_composition_request.candidates[0]
        .content
        .domain_pack_content
        .workflow_overlay
        .policies[0]
        .id
        .clone();
    let extension_policy_digest = canonical_digest(
        &cycle.domain_pack_composition_request.candidates[1]
            .content
            .domain_pack_content
            .workflow_overlay
            .policies[0],
    );
    let foundation_identity = cycle.domain_pack_composition_request.candidates[0]
        .manifest
        .domain_pack_manifest
        .identity
        .clone();
    let extension_identity = cycle.domain_pack_composition_request.candidates[1]
        .manifest
        .domain_pack_manifest
        .identity
        .clone();
    cycle.domain_pack_composition_request.candidates[1]
        .manifest
        .domain_pack_manifest
        .replacement_slots
        .push(DomainPackReplacementSlot {
            id: StableId("sample.extension.replacement.delivery-policy".to_owned()),
            contribution_kind: DomainPackContributionKind::Policy,
            target_ref: shared_ref.clone(),
            target_digest: extension_policy_digest.clone(),
            allowed_replacers: vec![DomainPackCoordinate {
                publisher: foundation_identity.publisher.clone(),
                name: foundation_identity.name.clone(),
            }],
            replacement_version_requirement: "^1.0".to_owned(),
        });
    cycle.domain_pack_composition_request.candidates[0]
        .manifest
        .domain_pack_manifest
        .replacement_declarations
        .push(DomainPackReplacementDeclaration {
            target_pack: DomainPackCoordinate {
                publisher: extension_identity.publisher,
                name: extension_identity.name,
            },
            target_slot_ref: StableId("sample.extension.replacement.delivery-policy".to_owned()),
            contribution_kind: DomainPackContributionKind::Policy,
            target_ref: shared_ref.clone(),
            target_digest: extension_policy_digest,
            replacement_ref: shared_ref,
        });
    let mut cycle_owned = owned_materials(&cycle);
    rebind_manifest(&mut cycle, &mut cycle_owned, 0);
    rebind_manifest(&mut cycle, &mut cycle_owned, 1);
    assert!(compose(&cycle, &cycle_owned)
        .domain_pack_composition_projection
        .issues
        .iter()
        .any(|issue| {
            issue.code == DomainPackCompositionIssueCode::ReplacementTargetMismatch
                && issue.message.contains("acyclic")
        }));
}
