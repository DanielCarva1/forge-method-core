use forge_core_contracts::{
    DomainPackArtifactBinding, DomainPackCandidateAuthority, DomainPackCapabilityKind,
    DomainPackCapabilitySandboxPolicy, DomainPackCompositionRequestDocument,
    DomainPackExternalExecutionPolicy, DomainPackPackageBinding, DomainPackResolvedPackage,
    DomainPackRuntimeCapabilityBinding, DomainPackRuntimeCapabilityGapCode,
    DomainPackRuntimeCapabilityRegistry, DomainPackRuntimeCapabilityStatus,
    DomainPackRuntimeProvider, DomainPackSandboxDefaultDecision, DomainPackSourceAssurance,
    DomainPackSupplyChainAssessment, DomainPackTrustDisposition, DomainPackTrustPolicy,
    DomainPackTrustRule, DomainPackVersionReference, RepoPath, StableId,
};
use forge_core_decisions::{
    evaluate_domain_pack_trust, DomainPackCapabilityDemand, DomainPackTrustEvaluationInput,
    DomainPackTrustEvaluationStatus, DomainPackTrustSelectedPackage,
};

const PACKAGE_DIGEST: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const RECORD_DIGEST: &str =
    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const IMPLEMENTATION_DIGEST: &str =
    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn selected_package() -> DomainPackTrustSelectedPackage {
    let path = repo_root().join("docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml");
    let request: DomainPackCompositionRequestDocument =
        yaml_serde::from_str(&std::fs::read_to_string(path).expect("composition request fixture"))
            .expect("typed composition request");
    let candidate = &request.domain_pack_composition_request.candidates[0];
    let manifest = &candidate.manifest.domain_pack_manifest;
    DomainPackTrustSelectedPackage {
        package: DomainPackResolvedPackage {
            identity: manifest.identity.clone(),
            package: DomainPackPackageBinding {
                package_ref: RepoPath("packages/foundation.pack".to_owned()),
                package_digest: PACKAGE_DIGEST.to_owned(),
                manifest: candidate.manifest_binding.clone(),
                content: manifest.content.clone(),
                license: manifest.provenance.license_text.clone(),
                fixtures: candidate
                    .content
                    .domain_pack_content
                    .fixtures
                    .iter()
                    .map(|fixture| fixture.artifact.clone())
                    .collect(),
            },
            registry_record_digest: RECORD_DIGEST.to_owned(),
            namespace_grant_id: StableId("grant.sample.foundation".to_owned()),
            source_assurance: DomainPackSourceAssurance::SupplyChainVerified,
            dependencies: manifest.dependencies.clone(),
            deterministic_order: 0,
        },
        structurally_valid: true,
        supply_chain: DomainPackSupplyChainAssessment {
            package_digest: PACKAGE_DIGEST.to_owned(),
            registry_record_digest: RECORD_DIGEST.to_owned(),
            publisher_signature_verified: true,
            registry_signature_threshold_verified: true,
            namespace_grant_verified: true,
            revoked: false,
        },
        capability_demands: vec![DomainPackCapabilityDemand {
            subject_ref: StableId("sample.foundation.adapter.repository-view".to_owned()),
            capability_ref: StableId(
                "sample.foundation.capability.repository-inspection".to_owned(),
            ),
            kind: DomainPackCapabilityKind::Evaluator,
        }],
    }
}

fn evidence() -> DomainPackArtifactBinding {
    DomainPackArtifactBinding {
        artifact_ref: RepoPath("evidence/builtin.yaml".to_owned()),
        raw_sha256: IMPLEMENTATION_DIGEST.to_owned(),
        canonical_sha256: IMPLEMENTATION_DIGEST.to_owned(),
    }
}

fn builtin_binding(
    selected: &DomainPackTrustSelectedPackage,
) -> DomainPackRuntimeCapabilityBinding {
    DomainPackRuntimeCapabilityBinding {
        binding_id: StableId("binding.repository-inspector".to_owned()),
        pack: DomainPackVersionReference {
            publisher: selected.package.identity.publisher.clone(),
            name: selected.package.identity.name.clone(),
            version: selected.package.identity.version.clone(),
        },
        package_digest: PACKAGE_DIGEST.to_owned(),
        subject_ref: selected.capability_demands[0].subject_ref.clone(),
        capability_ref: selected.capability_demands[0].capability_ref.clone(),
        kind: DomainPackCapabilityKind::Evaluator,
        provider: DomainPackRuntimeProvider::CoreBuiltin {
            provider_id: StableId("core.repository-inspector".to_owned()),
        },
        implementation_digest: IMPLEMENTATION_DIGEST.to_owned(),
        status: DomainPackRuntimeCapabilityStatus::Available,
        evidence: evidence(),
    }
}

fn input() -> DomainPackTrustEvaluationInput {
    let selected = selected_package();
    let binding = builtin_binding(&selected);
    DomainPackTrustEvaluationInput {
        project_id: StableId("project.neutral-composition".to_owned()),
        trust_policy: DomainPackTrustPolicy {
            policy_id: StableId("policy.domain-pack-trust".to_owned()),
            policy_version: "1".to_owned(),
            audience: StableId("forge-core".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            registry_keys: vec![],
            required_registry_signature_threshold: 1,
            minimum_activation_assurance: DomainPackSourceAssurance::SupplyChainVerified,
            rules: vec![DomainPackTrustRule {
                rule_id: StableId("rule.foundation-exact".to_owned()),
                pack: forge_core_contracts::DomainPackCoordinate {
                    publisher: selected.package.identity.publisher.clone(),
                    name: selected.package.identity.name.clone(),
                },
                package_digest: Some(PACKAGE_DIGEST.to_owned()),
                content_digest: Some(selected.package.package.content.canonical_sha256.clone()),
                disposition:
                    DomainPackTrustDisposition::ActivateDeclarativeKnowledgeAndBoundBuiltIns,
            }],
            default_disposition: DomainPackTrustDisposition::Reject,
        },
        capability_registry: DomainPackRuntimeCapabilityRegistry {
            registry_id: StableId("registry.runtime-capabilities".to_owned()),
            registry_version: "1".to_owned(),
            project_id: StableId("project.neutral-composition".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            bindings: vec![binding],
        },
        sandbox_policy: DomainPackCapabilitySandboxPolicy {
            policy_id: StableId("policy.domain-pack-sandbox".to_owned()),
            policy_version: "1".to_owned(),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            default_decision: DomainPackSandboxDefaultDecision::Deny,
            allowed_builtin_binding_ids: vec![StableId("binding.repository-inspector".to_owned())],
            external_execution: DomainPackExternalExecutionPolicy::DenyAll,
        },
        selected: vec![selected],
    }
}

#[test]
fn only_exact_available_allowlisted_core_builtin_is_verified() {
    let evaluation = evaluate_domain_pack_trust(&input());
    assert_eq!(evaluation.status, DomainPackTrustEvaluationStatus::Approved);
    assert!(evaluation.capability_gaps.is_empty());
    assert!(evaluation.issues.is_empty());
    assert_eq!(evaluation.verified_capability_bindings.len(), 1);
    assert_eq!(
        evaluation.verified_capability_bindings[0].decision,
        forge_core_contracts::DomainPackSandboxDecision::AllowedBoundBuiltin
    );
    assert!(evaluation.evaluation_digest.starts_with("sha256:"));
}

#[test]
fn declaration_or_allowlist_alone_never_proves_availability() {
    let mut no_binding = input();
    no_binding.capability_registry.bindings.clear();
    let missing = evaluate_domain_pack_trust(&no_binding);
    assert_eq!(missing.status, DomainPackTrustEvaluationStatus::Blocked);
    assert!(missing.verified_capability_bindings.is_empty());
    assert_eq!(
        missing.capability_gaps[0].code,
        DomainPackRuntimeCapabilityGapCode::MissingBinding
    );

    let mut not_allowlisted = input();
    not_allowlisted
        .sandbox_policy
        .allowed_builtin_binding_ids
        .clear();
    let denied = evaluate_domain_pack_trust(&not_allowlisted);
    assert!(denied.verified_capability_bindings.is_empty());
    assert_eq!(
        denied.capability_gaps[0].code,
        DomainPackRuntimeCapabilityGapCode::UndeclaredBinding
    );
}

#[test]
fn external_provider_is_denied_even_when_available_and_allowlisted() {
    let mut candidate = input();
    candidate.capability_registry.bindings[0].provider = DomainPackRuntimeProvider::Mcp {
        provider_id: StableId("external.mcp".to_owned()),
    };
    let evaluation = evaluate_domain_pack_trust(&candidate);
    assert_eq!(evaluation.status, DomainPackTrustEvaluationStatus::Blocked);
    assert!(evaluation.verified_capability_bindings.is_empty());
    assert_eq!(
        evaluation.capability_gaps[0].code,
        DomainPackRuntimeCapabilityGapCode::ExternalProviderDenied
    );
}

#[test]
fn invalid_supply_chain_or_non_deny_default_fails_closed() {
    let mut candidate = input();
    candidate.selected[0].supply_chain.revoked = true;
    candidate.trust_policy.default_disposition = DomainPackTrustDisposition::InspectOnly;
    let evaluation = evaluate_domain_pack_trust(&candidate);
    assert_eq!(evaluation.status, DomainPackTrustEvaluationStatus::Blocked);
    assert_eq!(
        evaluation.trust_decisions[0].disposition,
        DomainPackTrustDisposition::Reject
    );
    assert!(evaluation.verified_capability_bindings.is_empty());
}
