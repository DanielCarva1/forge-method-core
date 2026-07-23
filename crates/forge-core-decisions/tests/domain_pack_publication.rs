use forge_core_contracts::{
    DomainPackArtifactBinding, DomainPackAuthorArtifactRefs, DomainPackAuthorProvenanceTemplate,
    DomainPackAuthorRawSidecars, DomainPackAuthorSkeletonRequest,
    DomainPackAuthorSkeletonRequestDocument, DomainPackAuthorTestRequest,
    DomainPackAuthorTestRequestDocument, DomainPackCandidateAuthority,
    DomainPackCatalogPublicationStatus, DomainPackCatalogRevocationRequest,
    DomainPackCatalogRevocationRequestDocument, DomainPackCoreBinding,
    DomainPackExternalSignatureAlgorithm, DomainPackExternalSigningEvidence,
    DomainPackExternalSigningEvidenceDocument, DomainPackExternalSigningEvidenceStatus,
    DomainPackExternalSigningRequestInput, DomainPackExternalSigningRequestInputDocument,
    DomainPackExternalSigningRequestSource, DomainPackIdentity, DomainPackPackageRevocation,
    DomainPackProjectRequirements, DomainPackProjectRequirementsDocument,
    DomainPackPublicationIssueCode, DomainPackPublicationPackageRequest,
    DomainPackPublicationPackageRequestDocument, DomainPackPublicationPackageStatus,
    DomainPackRegistryArtifactSet, DomainPackRegistryMirror, DomainPackRegistryMirrorTransport,
    DomainPackRegistryPackageRecord, DomainPackRegistryTrustRole,
    DomainPackRemoteArtifactDescriptor, DomainPackRemoteArtifactKind,
    DomainPackRemoteArtifactMediaType, DomainPackRevocationReason, DomainPackSourceKind,
    DomainPackSupplyChainRegistry, DomainPackSupplyChainRegistryDocument, RepoPath, StableId,
    WorkflowGovernanceBundle, DOMAIN_PACK_AUTHORING_SCHEMA_VERSION,
    DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION, DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION,
    DOMAIN_PACK_SCHEMA_VERSION,
};
use forge_core_decisions::{
    assess_domain_pack_external_signing_evidence, prepare_domain_pack_publication,
    propose_domain_pack_catalog_revocation, request_domain_pack_external_signing,
};
use sha2::{Digest, Sha256};

fn digest(hex: char) -> String {
    format!("sha256:{}", hex.to_string().repeat(64))
}

fn canonical_digest<T: serde::Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn digest_without_field<T: serde::Serialize>(value: &T, field: &str) -> String {
    let mut json = serde_json::to_value(value).expect("JSON value");
    json.as_object_mut()
        .expect("object")
        .remove(field)
        .expect("digest field");
    let bytes = serde_json_canonicalizer::to_vec(&json).expect("canonical JSON");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn skeleton_request() -> DomainPackAuthorSkeletonRequestDocument {
    let bundle = WorkflowGovernanceBundle {
        id: StableId("core.bundle".to_owned()),
        policies: Vec::new(),
    };
    DomainPackAuthorSkeletonRequestDocument {
        schema_version: DOMAIN_PACK_AUTHORING_SCHEMA_VERSION.to_owned(),
        domain_pack_author_skeleton_request: DomainPackAuthorSkeletonRequest {
            request_id: StableId("publication.authoring".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            publisher: StableId("example".to_owned()),
            name: StableId("publication".to_owned()),
            namespace: StableId("example.publication".to_owned()),
            version: "1.0.0".to_owned(),
            forge_core_version: "1.85.0".to_owned(),
            core: DomainPackCoreBinding {
                bundle_id: bundle.id.clone(),
                bundle_digest: canonical_digest(&bundle),
                policy_set_digest: canonical_digest(&bundle.policies),
                bundle,
            },
            requirements: DomainPackProjectRequirementsDocument {
                schema_version: DOMAIN_PACK_SCHEMA_VERSION.to_owned(),
                domain_pack_project_requirements: DomainPackProjectRequirements {
                    project_id: StableId("example.project".to_owned()),
                    requirement_set_id: StableId("example.requirements".to_owned()),
                    required_domains: Vec::new(),
                },
            },
            provenance: DomainPackAuthorProvenanceTemplate {
                source_kind: DomainPackSourceKind::LocalCandidate,
                source_uri: "https://example.invalid/source".to_owned(),
                source_revision: "draft.1".to_owned(),
                source_digest: digest('0'),
                authors: vec![StableId("example.author".to_owned())],
                license_spdx_expression: "MIT".to_owned(),
            },
            artifact_refs: DomainPackAuthorArtifactRefs {
                manifest_ref: RepoPath("packs/example/manifest.yaml".to_owned()),
                content_ref: RepoPath("packs/example/content.yaml".to_owned()),
                license_ref: RepoPath("packs/example/LICENSE.yaml".to_owned()),
            },
        },
    }
}

fn descriptor(
    kind: DomainPackRemoteArtifactKind,
    binding: DomainPackArtifactBinding,
) -> DomainPackRemoteArtifactDescriptor {
    DomainPackRemoteArtifactDescriptor {
        kind,
        object_path: RepoPath(format!("objects/sha256/{}", &binding.raw_sha256[7..])),
        binding,
        byte_length: 1,
        media_type: DomainPackRemoteArtifactMediaType::ApplicationYaml,
    }
}

#[allow(clippy::too_many_lines)]
fn package_request(tamper_descriptor: bool) -> DomainPackPublicationPackageRequestDocument {
    let skeleton = forge_core_decisions::generate_domain_pack_author_skeleton(&skeleton_request());
    let template = skeleton
        .domain_pack_author_skeleton
        .template
        .expect("generic template");
    let author_test = DomainPackAuthorTestRequestDocument {
        schema_version: DOMAIN_PACK_AUTHORING_SCHEMA_VERSION.to_owned(),
        domain_pack_author_test_request: DomainPackAuthorTestRequest {
            request_id: StableId("publication.author-test".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            candidate: template.candidate.clone(),
            composition_request: template.composition_request,
            raw_sidecars: vec![DomainPackAuthorRawSidecars {
                pack: forge_core_contracts::DomainPackVersionReference {
                    publisher: template
                        .candidate
                        .manifest
                        .domain_pack_manifest
                        .identity
                        .publisher
                        .clone(),
                    name: template
                        .candidate
                        .manifest
                        .domain_pack_manifest
                        .identity
                        .name
                        .clone(),
                    version: template
                        .candidate
                        .manifest
                        .domain_pack_manifest
                        .identity
                        .version
                        .clone(),
                },
                manifest_raw: template.manifest.raw_bytes,
                content_raw: template.content.raw_bytes,
                license_raw: template.license.raw_bytes,
            }],
            compatibility: None,
            learning: None,
            reviewed_registry: None,
        },
    };
    let manifest = descriptor(
        DomainPackRemoteArtifactKind::Manifest,
        template.candidate.manifest_binding.clone(),
    );
    let content = descriptor(
        DomainPackRemoteArtifactKind::Content,
        DomainPackArtifactBinding {
            artifact_ref: template
                .candidate
                .manifest
                .domain_pack_manifest
                .content
                .content_ref
                .clone(),
            raw_sha256: template
                .candidate
                .manifest
                .domain_pack_manifest
                .content
                .raw_sha256
                .clone(),
            canonical_sha256: template
                .candidate
                .manifest
                .domain_pack_manifest
                .content
                .canonical_sha256
                .clone(),
        },
    );
    let mut license = descriptor(
        DomainPackRemoteArtifactKind::License,
        template
            .candidate
            .manifest
            .domain_pack_manifest
            .provenance
            .license_text
            .clone(),
    );
    if tamper_descriptor {
        license.object_path = RepoPath("objects/sha256/not-the-license".to_owned());
    }
    let record = DomainPackRegistryPackageRecord {
        identity: template
            .candidate
            .manifest
            .domain_pack_manifest
            .identity
            .clone(),
        package_digest: digest('1'),
        manifest_digest: manifest.binding.raw_sha256.clone(),
        content_digest: content.binding.raw_sha256.clone(),
        license_digest: license.binding.raw_sha256.clone(),
        fixture_digests: Vec::new(),
        artifacts: DomainPackRegistryArtifactSet {
            manifest,
            content,
            license,
            fixtures: Vec::new(),
        },
        namespace_grant_id: StableId("grant.example".to_owned()),
        publisher_credential_id: StableId("credential.example".to_owned()),
        publisher_signature_hex: String::new(),
        record_digest: digest('2'),
    };
    DomainPackPublicationPackageRequestDocument {
        schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
        domain_pack_publication_package_request: DomainPackPublicationPackageRequest {
            request_id: StableId("publication.package".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            registry_id: StableId("registry.example".to_owned()),
            audience: StableId("audience.example".to_owned()),
            author_test,
            record,
        },
    }
}

fn current_catalog() -> DomainPackSupplyChainRegistryDocument {
    let manifest = descriptor(
        DomainPackRemoteArtifactKind::Manifest,
        DomainPackArtifactBinding {
            artifact_ref: RepoPath("packs/example/manifest.yaml".to_owned()),
            raw_sha256: digest('a'),
            canonical_sha256: digest('b'),
        },
    );
    let content = descriptor(
        DomainPackRemoteArtifactKind::Content,
        DomainPackArtifactBinding {
            artifact_ref: RepoPath("packs/example/content.yaml".to_owned()),
            raw_sha256: digest('c'),
            canonical_sha256: digest('d'),
        },
    );
    let license = descriptor(
        DomainPackRemoteArtifactKind::License,
        DomainPackArtifactBinding {
            artifact_ref: RepoPath("packs/example/LICENSE".to_owned()),
            raw_sha256: digest('e'),
            canonical_sha256: digest('f'),
        },
    );
    let record = DomainPackRegistryPackageRecord {
        identity: DomainPackIdentity {
            publisher: StableId("example.publisher".to_owned()),
            name: StableId("example-pack".to_owned()),
            namespace: StableId("example.domain".to_owned()),
            version: "1.0.0".to_owned(),
        },
        package_digest: digest('1'),
        manifest_digest: manifest.binding.raw_sha256.clone(),
        content_digest: content.binding.raw_sha256.clone(),
        license_digest: license.binding.raw_sha256.clone(),
        fixture_digests: Vec::new(),
        artifacts: DomainPackRegistryArtifactSet {
            manifest,
            content: content.clone(),
            license,
            fixtures: Vec::new(),
        },
        namespace_grant_id: StableId("grant.example".to_owned()),
        publisher_credential_id: StableId("credential.example".to_owned()),
        publisher_signature_hex: "00".to_owned(),
        record_digest: digest('2'),
    };
    DomainPackSupplyChainRegistryDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_supply_chain_registry: DomainPackSupplyChainRegistry {
            registry_id: StableId("registry.example".to_owned()),
            registry_version: "1".to_owned(),
            audience: StableId("audience.example".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            generation: 7,
            previous_snapshot_digest: Some(digest('3')),
            issued_at_unix: 10,
            expires_at_unix: 100,
            publisher_credentials: Vec::new(),
            namespace_grants: Vec::new(),
            mirrors: vec![DomainPackRegistryMirror {
                mirror_id: StableId("mirror.example".to_owned()),
                priority: 0,
                transport: DomainPackRegistryMirrorTransport::Https {
                    base_url: "https://mirror.example.invalid/domain-packs".to_owned(),
                },
            }],
            packages: vec![record],
            revocations: vec![DomainPackPackageRevocation {
                record_digest: digest('4'),
                reason: DomainPackRevocationReason::OperatorPolicy,
                explanation: "earlier immutable revocation".to_owned(),
                revoked_at_unix: 12,
            }],
            snapshot_digest: digest('5'),
            signatures: Vec::new(),
        },
    }
}

#[test]
fn package_preparation_requires_c7_1_compatibility_and_exact_descriptors() {
    let no_compatibility = prepare_domain_pack_publication(&package_request(false));
    assert_eq!(
        no_compatibility
            .domain_pack_publication_package_candidate
            .status,
        DomainPackPublicationPackageStatus::Blocked
    );
    assert!(no_compatibility
        .domain_pack_publication_package_candidate
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackPublicationIssueCode::MissingCompatibilityEvidence));

    let malformed_descriptor = prepare_domain_pack_publication(&package_request(true));
    assert!(malformed_descriptor
        .domain_pack_publication_package_candidate
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackPublicationIssueCode::ArtifactBindingMismatch));
}

#[test]
fn revocation_carries_prior_facts_and_requests_only_external_registry_signing() {
    let request = DomainPackCatalogRevocationRequestDocument {
        schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
        domain_pack_catalog_revocation_request: DomainPackCatalogRevocationRequest {
            request_id: StableId("revoke.example".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            current: current_catalog(),
            registry_version: "2".to_owned(),
            issued_at_unix: 101,
            expires_at_unix: 200,
            revocation: DomainPackPackageRevocation {
                record_digest: digest('2'),
                reason: DomainPackRevocationReason::PackageTamper,
                explanation: "new immutable revocation".to_owned(),
                revoked_at_unix: 101,
            },
        },
    };
    let candidate = propose_domain_pack_catalog_revocation(&request);
    let projection = &candidate.domain_pack_catalog_publication_candidate;
    assert_eq!(
        projection.status,
        DomainPackCatalogPublicationStatus::CatalogCandidateReady
    );
    assert_eq!(projection.cumulative_revocations.len(), 2);
    assert!(projection
        .cumulative_revocations
        .iter()
        .any(|fact| fact.record_digest == digest('4')
            && fact.explanation == "earlier immutable revocation"));
    assert_eq!(
        projection
            .catalog
            .as_ref()
            .expect("catalog candidate")
            .domain_pack_supply_chain_registry
            .snapshot_digest,
        ""
    );

    // A self-digested but already snapshot-bound catalog is not a C7.2 signing
    // subject: C7.2 emits unsigned candidates only.
    let mut noncandidate_catalog = candidate.clone();
    let noncandidate_projection =
        &mut noncandidate_catalog.domain_pack_catalog_publication_candidate;
    noncandidate_projection
        .catalog
        .as_mut()
        .expect("catalog candidate")
        .domain_pack_supply_chain_registry
        .snapshot_digest = digest('6');
    let noncandidate_digest =
        digest_without_field(noncandidate_projection, "catalog_candidate_digest");
    noncandidate_projection.catalog_candidate_digest = noncandidate_digest;
    let blocked_signing =
        request_domain_pack_external_signing(&DomainPackExternalSigningRequestInputDocument {
            schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
            domain_pack_external_signing_request_input: DomainPackExternalSigningRequestInput {
                request_id: StableId("registry.already.bound".to_owned()),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                source: DomainPackExternalSigningRequestSource::RegistrySnapshot {
                    catalog: noncandidate_catalog,
                    signer_key_id: StableId("registry.signer".to_owned()),
                    role: DomainPackRegistryTrustRole::RegistrySigner,
                },
            },
        });
    assert_eq!(
        blocked_signing.domain_pack_external_signing_request.status,
        forge_core_contracts::DomainPackExternalSigningRequestStatus::Blocked
    );

    let signing =
        request_domain_pack_external_signing(&DomainPackExternalSigningRequestInputDocument {
            schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
            domain_pack_external_signing_request_input: DomainPackExternalSigningRequestInput {
                request_id: StableId("registry.signing".to_owned()),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                source: DomainPackExternalSigningRequestSource::RegistrySnapshot {
                    catalog: candidate,
                    signer_key_id: StableId("registry.signer".to_owned()),
                    role: DomainPackRegistryTrustRole::RegistrySigner,
                },
            },
        });
    assert_eq!(
        signing.domain_pack_external_signing_request.status,
        forge_core_contracts::DomainPackExternalSigningRequestStatus::ExternalEvidenceRequired
    );
    assert!(!serde_json::to_string(&signing)
        .expect("signing request JSON")
        .contains("private_key"));
}

#[test]
fn external_evidence_is_bound_but_never_cryptographically_verified_here() {
    let revocation_request = DomainPackCatalogRevocationRequestDocument {
        schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
        domain_pack_catalog_revocation_request: DomainPackCatalogRevocationRequest {
            request_id: StableId("revoke.for.evidence".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            current: current_catalog(),
            registry_version: "2".to_owned(),
            issued_at_unix: 101,
            expires_at_unix: 200,
            revocation: DomainPackPackageRevocation {
                record_digest: digest('2'),
                reason: DomainPackRevocationReason::PackageTamper,
                explanation: "candidate evidence target".to_owned(),
                revoked_at_unix: 101,
            },
        },
    };
    let catalog = propose_domain_pack_catalog_revocation(&revocation_request);
    let signing =
        request_domain_pack_external_signing(&DomainPackExternalSigningRequestInputDocument {
            schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
            domain_pack_external_signing_request_input: DomainPackExternalSigningRequestInput {
                request_id: StableId("registry.evidence.request".to_owned()),
                authority: DomainPackCandidateAuthority::CandidateOnly,
                source: DomainPackExternalSigningRequestSource::RegistrySnapshot {
                    catalog,
                    signer_key_id: StableId("registry.signer".to_owned()),
                    role: DomainPackRegistryTrustRole::RegistrySigner,
                },
            },
        });
    let mut evidence = DomainPackExternalSigningEvidenceDocument {
        schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
        domain_pack_external_signing_evidence: DomainPackExternalSigningEvidence {
            evidence_id: StableId("registry.signature.evidence".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            signing_request_digest: signing
                .domain_pack_external_signing_request
                .signing_request_digest
                .clone(),
            signer_key_id: StableId("registry.signer".to_owned()),
            algorithm: DomainPackExternalSignatureAlgorithm::Ed25519,
            signature_hex: "0".repeat(128),
            supplied_at_unix: 102,
            evidence_digest: String::new(),
        },
    };
    evidence
        .domain_pack_external_signing_evidence
        .evidence_digest = digest_without_field(
        &evidence.domain_pack_external_signing_evidence,
        "evidence_digest",
    );
    let assessment = assess_domain_pack_external_signing_evidence(&signing, &evidence);
    assert_eq!(
        assessment
            .domain_pack_external_signing_evidence_assessment
            .status,
        DomainPackExternalSigningEvidenceStatus::UnverifiedEvidenceBound
    );
    let serialized = serde_json::to_string(&assessment).expect("assessment JSON");
    assert!(!serialized.contains("verified_signature"));
    assert!(!serialized.contains("published"));
}
