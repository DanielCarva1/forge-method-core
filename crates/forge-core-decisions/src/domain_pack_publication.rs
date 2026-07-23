//! Pure C7.2 candidate-only Domain Pack publication workflow decisions.
//!
//! This module binds authored candidate material to immutable C6.2 descriptors,
//! requests external signing, checks review readiness, and proposes catalog or
//! revocation successors. It has no key custody, signature construction or
//! verification, transport, remote publication, catalog admission, anchoring,
//! lifecycle, installation, or activation capability.

use std::collections::BTreeSet;

use forge_core_contracts::{
    domain_pack_unsigned_catalog_document, DomainPackAuthorCompatibilityStatus,
    DomainPackAuthorTestStatus, DomainPackCandidateAuthority, DomainPackCatalogEvolutionBase,
    DomainPackCatalogEvolutionRequestDocument, DomainPackCatalogPublicationCandidate,
    DomainPackCatalogPublicationCandidateDocument, DomainPackCatalogPublicationStatus,
    DomainPackCatalogRevocationRequestDocument, DomainPackExternalSignatureAlgorithm,
    DomainPackExternalSigningEvidenceAssessment,
    DomainPackExternalSigningEvidenceAssessmentDocument, DomainPackExternalSigningEvidenceDocument,
    DomainPackExternalSigningEvidenceStatus, DomainPackExternalSigningProtocol,
    DomainPackExternalSigningRequest, DomainPackExternalSigningRequestDocument,
    DomainPackExternalSigningRequestInputDocument, DomainPackExternalSigningRequestSource,
    DomainPackExternalSigningRequestStatus, DomainPackExternalSigningSubject,
    DomainPackPackageRevocation, DomainPackPublicationIssue, DomainPackPublicationIssueCode,
    DomainPackPublicationPackageCandidate, DomainPackPublicationPackageCandidateDocument,
    DomainPackPublicationPackageRequestDocument, DomainPackPublicationPackageStatus,
    DomainPackPublicationProvenanceBinding, DomainPackPublicationReviewReadiness,
    DomainPackPublicationReviewReadinessDocument, DomainPackPublicationReviewRequestDocument,
    DomainPackPublicationReviewStatus, DomainPackRegistryPackageRecord,
    DomainPackSupplyChainRegistryDocument, StableId, DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
    DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    evaluate_domain_pack_author_test, evaluate_domain_pack_promotion,
    DomainPackPromotionEvaluationInput, DomainPackPromotionReadinessStatus,
};

const MAX_PUBLICATION_PACKAGES: usize = 4_096;
const MAX_PUBLICATION_REVOCATIONS: usize = 4_096;
const MAX_PUBLICATION_REVIEWS: usize = 32;

/// Prepare one registry-record candidate from a freshly evaluated C7.1 request.
/// A successful result is only a coherent candidate: record-digest and eventual
/// publisher-signature verification remain owned by the supply-chain authority.
#[must_use]
pub fn prepare_domain_pack_publication(
    request_document: &DomainPackPublicationPackageRequestDocument,
) -> DomainPackPublicationPackageCandidateDocument {
    let request = &request_document.domain_pack_publication_package_request;
    let author_test_report = evaluate_domain_pack_author_test(&request.author_test);
    let mut issues = Vec::new();

    require_id(&mut issues, "request.request_id", &request.request_id);
    require_id(&mut issues, "request.registry_id", &request.registry_id);
    require_id(&mut issues, "request.audience", &request.audience);
    if request_document.schema_version != DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::InvalidContract,
            "schema_version",
            "unsupported C7.2 publication schema version",
        );
    }
    if author_test_report.domain_pack_author_test_report.status
        != DomainPackAuthorTestStatus::CandidateReady
    {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::AuthorTestBlocked,
            "author_test",
            "the freshly evaluated C7.1 author test is blocked",
        );
    }
    match &author_test_report
        .domain_pack_author_test_report
        .compatibility
    {
        Some(readiness) if readiness.status == DomainPackAuthorCompatibilityStatus::Compatible => {}
        Some(_) => issue(
            &mut issues,
            DomainPackPublicationIssueCode::CompatibilityBlocked,
            "author_test.compatibility",
            "publication requires compatible exact-lock evidence",
        ),
        None => issue(
            &mut issues,
            DomainPackPublicationIssueCode::MissingCompatibilityEvidence,
            "author_test.compatibility",
            "publication requires explicit exact-lock compatibility evidence",
        ),
    }
    validate_record_against_author_test(&request.record, &request.author_test, &mut issues);
    finish_issues(&mut issues);

    let status = if issues.is_empty() {
        DomainPackPublicationPackageStatus::PreparedCandidate
    } else {
        DomainPackPublicationPackageStatus::Blocked
    };
    let provenance = if issues.is_empty() {
        Some(provenance_binding(
            &request.record,
            &request.author_test,
            &author_test_report,
        ))
    } else {
        None
    };
    let mut candidate = DomainPackPublicationPackageCandidate {
        request_id: request.request_id.clone(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
        registry_id: request.registry_id.clone(),
        audience: request.audience.clone(),
        status,
        author_test: request.author_test.clone(),
        author_test_report,
        record: if issues.is_empty() {
            Some(request.record.clone())
        } else {
            None
        },
        provenance,
        issues,
        package_candidate_digest: String::new(),
    };
    candidate.package_candidate_digest = package_candidate_digest_value(&candidate);
    DomainPackPublicationPackageCandidateDocument {
        schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
        domain_pack_publication_package_candidate: candidate,
    }
}

/// Re-evaluate P6c promotion material for one prepared candidate. The result is
/// an evidence-readiness projection only; it neither verifies reviewer signatures
/// nor turns reviews into a catalog or lifecycle authority.
#[must_use]
pub fn evaluate_domain_pack_publication_review_readiness(
    request_document: &DomainPackPublicationReviewRequestDocument,
) -> DomainPackPublicationReviewReadinessDocument {
    let request = &request_document.domain_pack_publication_review_request;
    let package = &request.package.domain_pack_publication_package_candidate;
    let evaluation = evaluate_domain_pack_promotion(&DomainPackPromotionEvaluationInput {
        dossier: &request.learning.dossier,
        candidates: &request.learning.candidates,
        independent_reviews: &request.learning.independent_reviews,
        conflicts: &request.learning.conflicts,
    });
    let mut issues = Vec::new();

    require_id(&mut issues, "request.request_id", &request.request_id);
    if request_document.schema_version != DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::InvalidContract,
            "schema_version",
            "unsupported C7.2 publication schema version",
        );
    }
    validate_package_candidate(&request.package, "package", &mut issues);
    if request.learning.independent_reviews.len() > MAX_PUBLICATION_REVIEWS {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::ResourceLimitExceeded,
            "learning.independent_reviews",
            "too many independent-review records",
        );
    }
    if let Some(record) = &package.record {
        let dossier = &request.learning.dossier.domain_pack_promotion_dossier;
        let dossier_fixture_digests = dossier
            .fixture_bindings
            .iter()
            .map(|fixture| fixture.raw_sha256.clone())
            .collect::<Vec<_>>();
        if dossier.pack.publisher != record.identity.publisher
            || dossier.pack.name != record.identity.name
            || dossier.pack.version != record.identity.version
            || dossier.package_digest != record.package_digest
            || dossier.manifest_digest != record.manifest_digest
            || dossier.content_digest != record.content_digest
            || dossier.license_digest != record.license_digest
            || dossier_fixture_digests != record.fixture_digests
        {
            issue(
                &mut issues,
                DomainPackPublicationIssueCode::ReviewEvidenceMismatch,
                "learning.dossier",
                "promotion dossier does not bind the exact prepared package record",
            );
        }
    }
    match evaluation.status {
        DomainPackPromotionReadinessStatus::ReadyForTrustedReview => {}
        DomainPackPromotionReadinessStatus::ReviewRequired => issue(
            &mut issues,
            DomainPackPublicationIssueCode::ReviewReadinessBlocked,
            "learning",
            "independent review evidence remains required",
        ),
        DomainPackPromotionReadinessStatus::Blocked => issue(
            &mut issues,
            DomainPackPublicationIssueCode::ReviewReadinessBlocked,
            "learning",
            "promotion evidence evaluation is blocked",
        ),
    }
    finish_issues(&mut issues);

    let status = if issues.is_empty() {
        DomainPackPublicationReviewStatus::EvidenceReady
    } else if evaluation.status == DomainPackPromotionReadinessStatus::ReviewRequired {
        DomainPackPublicationReviewStatus::ReviewRequired
    } else {
        DomainPackPublicationReviewStatus::Blocked
    };
    let mut readiness = DomainPackPublicationReviewReadiness {
        request_id: request.request_id.clone(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
        package_candidate_digest: package.package_candidate_digest.clone(),
        dossier_digest: Some(
            request
                .learning
                .dossier
                .domain_pack_promotion_dossier
                .dossier_digest
                .clone(),
        ),
        promotion_evaluation_digest: evaluation.evaluation_digest,
        status,
        learning: request.learning.clone(),
        independent_reviews: request.learning.independent_reviews.clone(),
        issues,
        review_readiness_digest: String::new(),
    };
    readiness.review_readiness_digest = review_readiness_digest_value(&readiness);
    DomainPackPublicationReviewReadinessDocument {
        schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
        domain_pack_publication_review_readiness: readiness,
    }
}

/// Produce an external-signing request without assembling signing bytes or
/// communicating with a signer. The named protocol refers callers to the
/// existing authority-owned canonical payload builders.
#[must_use]
pub fn request_domain_pack_external_signing(
    input_document: &DomainPackExternalSigningRequestInputDocument,
) -> DomainPackExternalSigningRequestDocument {
    let input = &input_document.domain_pack_external_signing_request_input;
    let mut issues = Vec::new();
    require_id(&mut issues, "request.request_id", &input.request_id);
    if input_document.schema_version != DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::InvalidContract,
            "schema_version",
            "unsupported C7.2 publication schema version",
        );
    }

    let subject = match &input.source {
        DomainPackExternalSigningRequestSource::PublisherRecord {
            package,
            publisher_credential_id,
        } => {
            validate_package_candidate(package, "source.package", &mut issues);
            let candidate = &package.domain_pack_publication_package_candidate;
            let subject = candidate.record.as_ref().map(|record| {
                if &record.publisher_credential_id != publisher_credential_id {
                    issue(
                        &mut issues,
                        DomainPackPublicationIssueCode::SigningRequestBlocked,
                        "source.publisher_credential_id",
                        "requested publisher credential does not match the package record",
                    );
                }
                if !record.publisher_signature_hex.is_empty() {
                    issue(
                        &mut issues,
                        DomainPackPublicationIssueCode::SigningRequestBlocked,
                        "source.package.record.publisher_signature_hex",
                        "external publisher signing requests require an unsigned record",
                    );
                }
                DomainPackExternalSigningSubject::PublisherRecord {
                    registry_id: candidate.registry_id.clone(),
                    audience: candidate.audience.clone(),
                    package_candidate_digest: candidate.package_candidate_digest.clone(),
                    record: record.clone(),
                    publisher_credential_id: publisher_credential_id.clone(),
                }
            });
            if subject.is_none() {
                issue(
                    &mut issues,
                    DomainPackPublicationIssueCode::SigningRequestBlocked,
                    "source.package.record",
                    "a prepared exact package record is required for signing",
                );
            }
            subject
        }
        DomainPackExternalSigningRequestSource::RegistrySnapshot {
            catalog,
            signer_key_id,
            role,
        } => {
            require_id(&mut issues, "source.signer_key_id", signer_key_id);
            validate_catalog_candidate(catalog, "source.catalog", &mut issues);
            let candidate = &catalog.domain_pack_catalog_publication_candidate;
            let subject = candidate.catalog.as_ref().map(|catalog| {
                DomainPackExternalSigningSubject::RegistrySnapshot {
                    catalog_candidate_digest: candidate.catalog_candidate_digest.clone(),
                    catalog: catalog.clone(),
                    signer_key_id: signer_key_id.clone(),
                    role: *role,
                }
            });
            if subject.is_none() {
                issue(
                    &mut issues,
                    DomainPackPublicationIssueCode::SigningRequestBlocked,
                    "source.catalog.catalog",
                    "a catalog candidate is required for registry signing",
                );
            }
            subject
        }
    };
    finish_issues(&mut issues);
    let status = if issues.is_empty() {
        DomainPackExternalSigningRequestStatus::ExternalEvidenceRequired
    } else {
        DomainPackExternalSigningRequestStatus::Blocked
    };
    let mut request = DomainPackExternalSigningRequest {
        request_id: input.request_id.clone(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
        protocol: DomainPackExternalSigningProtocol::ExistingSupplyChainAuthorityV1,
        status,
        subject: if issues.is_empty() { subject } else { None },
        issues,
        signing_request_digest: String::new(),
    };
    request.signing_request_digest = signing_request_digest_value(&request);
    DomainPackExternalSigningRequestDocument {
        schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
        domain_pack_external_signing_request: request,
    }
}

/// Bind opaque, externally supplied signature evidence to a C7.2 request. This
/// performs only syntax and cross-document checks; it deliberately does not
/// verify any signature or construct a signed package/catalog.
#[must_use]
pub fn assess_domain_pack_external_signing_evidence(
    request_document: &DomainPackExternalSigningRequestDocument,
    evidence_document: &DomainPackExternalSigningEvidenceDocument,
) -> DomainPackExternalSigningEvidenceAssessmentDocument {
    let request = &request_document.domain_pack_external_signing_request;
    let evidence = &evidence_document.domain_pack_external_signing_evidence;
    let mut issues = Vec::new();

    validate_signing_request(request_document, &mut issues);
    validate_signing_evidence(evidence_document, &mut issues);
    if request.status != DomainPackExternalSigningRequestStatus::ExternalEvidenceRequired {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::SigningRequestBlocked,
            "signing_request.status",
            "external evidence cannot bind a blocked signing request",
        );
    }
    if evidence.signing_request_digest != request.signing_request_digest {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::SigningEvidenceMismatch,
            "evidence.signing_request_digest",
            "external evidence does not bind this exact signing request",
        );
    }
    if let Some(subject) = &request.subject {
        let expected = match subject {
            DomainPackExternalSigningSubject::PublisherRecord {
                publisher_credential_id,
                ..
            } => publisher_credential_id,
            DomainPackExternalSigningSubject::RegistrySnapshot { signer_key_id, .. } => {
                signer_key_id
            }
        };
        if &evidence.signer_key_id != expected {
            issue(
                &mut issues,
                DomainPackPublicationIssueCode::SigningEvidenceMismatch,
                "evidence.signer_key_id",
                "external evidence signer does not match the signing request",
            );
        }
    }
    finish_issues(&mut issues);
    let status = if issues.is_empty() {
        DomainPackExternalSigningEvidenceStatus::UnverifiedEvidenceBound
    } else {
        DomainPackExternalSigningEvidenceStatus::Blocked
    };
    let mut assessment = DomainPackExternalSigningEvidenceAssessment {
        evidence_id: evidence.evidence_id.clone(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
        signing_request: request_document.clone(),
        evidence: evidence_document.clone(),
        status,
        issues,
        assessment_digest: String::new(),
    };
    assessment.assessment_digest = signing_assessment_digest_value(&assessment);
    DomainPackExternalSigningEvidenceAssessmentDocument {
        schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
        domain_pack_external_signing_evidence_assessment: assessment,
    }
}

/// Construct an unsigned C6.2 catalog successor carrying a bound publisher
/// signature *claim* and independent-review evidence. The existing authority
/// later computes the authoritative snapshot digest, verifies every detached
/// signature, and decides whether to anchor or publish the catalog.
#[must_use]
pub fn propose_domain_pack_catalog_evolution(
    request_document: &DomainPackCatalogEvolutionRequestDocument,
) -> DomainPackCatalogPublicationCandidateDocument {
    let request = &request_document.domain_pack_catalog_evolution_request;
    let mut issues = Vec::new();
    require_id(&mut issues, "request.request_id", &request.request_id);
    if request_document.schema_version != DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::InvalidContract,
            "schema_version",
            "unsupported C7.2 publication schema version",
        );
    }
    validate_package_candidate(&request.package, "package", &mut issues);
    validate_review_readiness(&request.review, &request.package, &mut issues);
    let signed_record = signed_record_from_publisher_evidence(
        &request.package,
        &request.publisher_signature,
        &mut issues,
    );

    let mut catalog = signed_record.and_then(|record| {
        catalog_from_evolution_base(&request.base, &request.package, &record, &mut issues)
    });
    if let Some(candidate) = &catalog {
        if candidate.validate_remote_acquisition_metadata().is_err() {
            issue(
                &mut issues,
                DomainPackPublicationIssueCode::CatalogMetadataInvalid,
                "catalog",
                "catalog candidate has malformed C6.2 immutable descriptor metadata",
            );
        }
    }
    finish_issues(&mut issues);
    if !issues.is_empty() {
        catalog = None;
    }
    catalog_candidate_document(request.request_id.clone(), catalog, issues)
}

/// Append one immutable revocation fact to an unsigned catalog successor. Prior
/// facts are retained verbatim and cannot be removed or rewritten by this pure
/// workflow. The authority anchor independently enforces the same property
/// against its protected history before any admission.
#[must_use]
pub fn propose_domain_pack_catalog_revocation(
    request_document: &DomainPackCatalogRevocationRequestDocument,
) -> DomainPackCatalogPublicationCandidateDocument {
    let request = &request_document.domain_pack_catalog_revocation_request;
    let current = &request.current.domain_pack_supply_chain_registry;
    let mut issues = Vec::new();
    require_id(&mut issues, "request.request_id", &request.request_id);
    if request_document.schema_version != DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::InvalidContract,
            "schema_version",
            "unsupported C7.2 publication schema version",
        );
    }
    validate_current_catalog(&request.current, "current", &mut issues);
    if request.registry_version.trim().is_empty() {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::InvalidContract,
            "registry_version",
            "successor registry version must not be blank",
        );
    }
    if request.issued_at_unix >= request.expires_at_unix {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::InvalidContract,
            "expires_at_unix",
            "catalog validity window must be increasing",
        );
    }
    validate_revocation(&request.revocation, "revocation", &mut issues);
    if !current
        .packages
        .iter()
        .any(|record| record.record_digest == request.revocation.record_digest)
    {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::RevocationRecordMissing,
            "revocation.record_digest",
            "a revocation must name an exact package record in the current catalog",
        );
    }
    let mut revocations = current.revocations.clone();
    if revocations
        .iter()
        .any(|fact| fact.record_digest == request.revocation.record_digest)
    {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::DuplicateRevocation,
            "revocation.record_digest",
            "an existing cumulative revocation fact cannot be rewritten or repeated",
        );
    } else {
        revocations.push(request.revocation.clone());
    }
    validate_cumulative_revocations(&revocations, "current.revocations", &mut issues);
    sort_revocations(&mut revocations);

    let catalog = current.generation.checked_add(1).map(|generation| {
        domain_pack_unsigned_catalog_document(
            request.current.schema_version.clone(),
            current.registry_id.clone(),
            request.registry_version.clone(),
            current.audience.clone(),
            generation,
            Some(current.snapshot_digest.clone()),
            request.issued_at_unix,
            request.expires_at_unix,
            current.publisher_credentials.clone(),
            current.namespace_grants.clone(),
            current.mirrors.clone(),
            current.packages.clone(),
            revocations,
        )
    });
    if catalog.is_none() {
        issue(
            &mut issues,
            DomainPackPublicationIssueCode::CatalogGenerationMismatch,
            "current.generation",
            "catalog generation overflows while creating a successor",
        );
    }
    if let Some(candidate) = &catalog {
        if candidate.validate_remote_acquisition_metadata().is_err() {
            issue(
                &mut issues,
                DomainPackPublicationIssueCode::CatalogMetadataInvalid,
                "catalog",
                "catalog candidate has malformed C6.2 immutable descriptor metadata",
            );
        }
    }
    finish_issues(&mut issues);
    catalog_candidate_document(
        request.request_id.clone(),
        if issues.is_empty() { catalog } else { None },
        issues,
    )
}

fn validate_record_against_author_test(
    record: &DomainPackRegistryPackageRecord,
    author_test: &forge_core_contracts::DomainPackAuthorTestRequestDocument,
    issues: &mut Vec<DomainPackPublicationIssue>,
) {
    let request = &author_test.domain_pack_author_test_request;
    let candidate = &request.candidate;
    let manifest = &candidate.manifest.domain_pack_manifest;
    let content = &candidate.content.domain_pack_content;
    require_id(
        issues,
        "record.identity.publisher",
        &record.identity.publisher,
    );
    require_id(issues, "record.identity.name", &record.identity.name);
    require_id(
        issues,
        "record.identity.namespace",
        &record.identity.namespace,
    );
    if record.identity.version.trim().is_empty() {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidIdentifier,
            "record.identity.version",
            "package version must not be blank",
        );
    }
    if record.identity != manifest.identity
        || content.pack.publisher != record.identity.publisher
        || content.pack.name != record.identity.name
        || content.pack.version != record.identity.version
        || content.namespace != record.identity.namespace
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::ArtifactBindingMismatch,
            "record.identity",
            "registry record identity does not match the authored manifest/content candidate",
        );
    }
    let artifacts = &record.artifacts;
    let content_matches = artifacts.content.binding.artifact_ref == manifest.content.content_ref
        && artifacts.content.binding.raw_sha256 == manifest.content.raw_sha256
        && artifacts.content.binding.canonical_sha256 == manifest.content.canonical_sha256;
    if artifacts.manifest.binding != candidate.manifest_binding
        || !content_matches
        || artifacts.license.binding != manifest.provenance.license_text
        || record.manifest_digest != artifacts.manifest.binding.raw_sha256
        || record.content_digest != artifacts.content.binding.raw_sha256
        || record.license_digest != artifacts.license.binding.raw_sha256
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::ArtifactBindingMismatch,
            "record.artifacts",
            "manifest, content, license, or digest fields do not bind the exact authored bytes",
        );
    }
    let fixture_bindings = content
        .fixtures
        .iter()
        .map(|fixture| fixture.artifact.clone())
        .collect::<Vec<_>>();
    let descriptor_bindings = artifacts
        .fixtures
        .iter()
        .map(|fixture| fixture.binding.clone())
        .collect::<Vec<_>>();
    let fixture_raw_digests = artifacts
        .fixtures
        .iter()
        .map(|fixture| fixture.binding.raw_sha256.clone())
        .collect::<Vec<_>>();
    if fixture_bindings != descriptor_bindings || record.fixture_digests != fixture_raw_digests {
        issue(
            issues,
            DomainPackPublicationIssueCode::FixtureBindingMismatch,
            "record.artifacts.fixtures",
            "fixture descriptors and record digests do not match the authored content fixtures",
        );
    }
    if !record.publisher_signature_hex.is_empty() {
        issue(
            issues,
            DomainPackPublicationIssueCode::PublisherSignaturePresentBeforeRequest,
            "record.publisher_signature_hex",
            "package preparation requires an unsigned record before external signing",
        );
    }
    if !sha256_token(&record.record_digest) {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidDigest,
            "record.record_digest",
            "record digest must be sha256: followed by 64 lowercase hexadecimal characters",
        );
    }
    let temp = domain_pack_unsigned_catalog_document(
        DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        StableId("candidate.registry".to_owned()),
        "candidate".to_owned(),
        StableId("candidate.audience".to_owned()),
        1,
        None,
        0,
        1,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![record.clone()],
        Vec::new(),
    );
    if temp.validate_remote_acquisition_metadata().is_err() {
        issue(
            issues,
            DomainPackPublicationIssueCode::ArtifactBindingMismatch,
            "record.artifacts",
            "record fails the established C6.2 immutable artifact-descriptor validation",
        );
    }
}

fn provenance_binding(
    record: &DomainPackRegistryPackageRecord,
    author_test: &forge_core_contracts::DomainPackAuthorTestRequestDocument,
    report: &forge_core_contracts::DomainPackAuthorTestReportDocument,
) -> DomainPackPublicationProvenanceBinding {
    let manifest = &author_test
        .domain_pack_author_test_request
        .candidate
        .manifest
        .domain_pack_manifest;
    DomainPackPublicationProvenanceBinding {
        author_test_report_digest: report.domain_pack_author_test_report.report_digest.clone(),
        source_kind: manifest.provenance.source_kind,
        source_uri: manifest.provenance.source_uri.clone(),
        source_revision: manifest.provenance.source_revision.clone(),
        source_digest: manifest.provenance.source_digest.clone(),
        authors: manifest.provenance.authors.clone(),
        license_spdx_expression: manifest.provenance.license_spdx_expression.clone(),
        manifest_raw_sha256: record.manifest_digest.clone(),
        content_raw_sha256: record.content_digest.clone(),
        license_raw_sha256: record.license_digest.clone(),
    }
}

fn validate_package_candidate(
    document: &DomainPackPublicationPackageCandidateDocument,
    path: &str,
    issues: &mut Vec<DomainPackPublicationIssue>,
) {
    let candidate = &document.domain_pack_publication_package_candidate;
    if document.schema_version != DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidContract,
            format!("{path}.schema_version"),
            "unsupported package-candidate schema version",
        );
    }
    if package_candidate_digest_value(candidate) != candidate.package_candidate_digest {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidDigest,
            format!("{path}.package_candidate_digest"),
            "package candidate digest does not bind the supplied candidate",
        );
    }
    if candidate.status != DomainPackPublicationPackageStatus::PreparedCandidate
        || candidate.record.is_none()
        || candidate.provenance.is_none()
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::PackageCandidateBlocked,
            path,
            "a prepared exact package candidate is required",
        );
        return;
    }
    let Some(record) = candidate.record.clone() else {
        return;
    };
    // Re-run C7.1 plus all package bindings from retained source evidence. A
    // candidate digest is an integrity commitment, not permission to invent a
    // prepared package status for later signing or catalog evolution.
    let recomputed =
        prepare_domain_pack_publication(&DomainPackPublicationPackageRequestDocument {
            schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
            domain_pack_publication_package_request:
                forge_core_contracts::DomainPackPublicationPackageRequest {
                    request_id: candidate.request_id.clone(),
                    authority: DomainPackCandidateAuthority::CandidateOnly,
                    registry_id: candidate.registry_id.clone(),
                    audience: candidate.audience.clone(),
                    author_test: candidate.author_test.clone(),
                    record,
                },
        });
    if recomputed != *document {
        issue(
            issues,
            DomainPackPublicationIssueCode::PackageCandidateBlocked,
            path,
            "package candidate is not reproducible from its exact C7.1 source evidence",
        );
    }
}

fn validate_review_readiness(
    document: &DomainPackPublicationReviewReadinessDocument,
    package: &DomainPackPublicationPackageCandidateDocument,
    issues: &mut Vec<DomainPackPublicationIssue>,
) {
    let readiness = &document.domain_pack_publication_review_readiness;
    if document.schema_version != DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION
        || review_readiness_digest_value(readiness) != readiness.review_readiness_digest
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidDigest,
            "review.review_readiness_digest",
            "review readiness document does not bind its supplied evidence",
        );
    }
    // Retaining the exact P6c input lets this workflow reproduce readiness,
    // rather than letting a self-digested caller assertion bypass the existing
    // promotion evaluator. The re-evaluation still does not verify reviewers
    // or grant promotion authority.
    let recomputed = evaluate_domain_pack_publication_review_readiness(
        &DomainPackPublicationReviewRequestDocument {
            schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
            domain_pack_publication_review_request:
                forge_core_contracts::DomainPackPublicationReviewRequest {
                    request_id: readiness.request_id.clone(),
                    authority: DomainPackCandidateAuthority::CandidateOnly,
                    package: package.clone(),
                    learning: readiness.learning.clone(),
                },
        },
    );
    if recomputed != *document {
        issue(
            issues,
            DomainPackPublicationIssueCode::ReviewEvidenceMismatch,
            "review",
            "review readiness is not reproducible from its exact P6c evidence",
        );
    }
    if readiness.status != DomainPackPublicationReviewStatus::EvidenceReady {
        issue(
            issues,
            DomainPackPublicationIssueCode::ReviewReadinessBlocked,
            "review.status",
            "catalog evolution requires independently reviewed candidate evidence",
        );
    }
    if readiness.package_candidate_digest
        != package
            .domain_pack_publication_package_candidate
            .package_candidate_digest
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::ReviewEvidenceMismatch,
            "review.package_candidate_digest",
            "review readiness does not bind the exact package candidate",
        );
    }
}

fn signed_record_from_publisher_evidence(
    package: &DomainPackPublicationPackageCandidateDocument,
    assessment_document: &DomainPackExternalSigningEvidenceAssessmentDocument,
    issues: &mut Vec<DomainPackPublicationIssue>,
) -> Option<DomainPackRegistryPackageRecord> {
    let assessment = &assessment_document.domain_pack_external_signing_evidence_assessment;
    if assessment_document.schema_version != DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION
        || signing_assessment_digest_value(assessment) != assessment.assessment_digest
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidDigest,
            "publisher_signature.assessment_digest",
            "external-signing assessment does not bind its supplied evidence",
        );
        return None;
    }
    // An assessment is caller-supplied candidate evidence. Recompute its
    // syntax/cross-reference projection so a self-consistent forged status
    // cannot turn arbitrary text into a publisher-signature claim. This is not
    // cryptographic verification; that remains an authority-owned operation.
    let recomputed = assess_domain_pack_external_signing_evidence(
        &assessment.signing_request,
        &assessment.evidence,
    );
    if recomputed != *assessment_document {
        issue(
            issues,
            DomainPackPublicationIssueCode::SigningEvidenceMismatch,
            "publisher_signature",
            "publisher-signature assessment is not reproducible from its request and evidence",
        );
        return None;
    }
    if assessment.status != DomainPackExternalSigningEvidenceStatus::UnverifiedEvidenceBound {
        issue(
            issues,
            DomainPackPublicationIssueCode::SigningEvidenceMismatch,
            "publisher_signature.status",
            "a bound external publisher-signature evidence record is required",
        );
        return None;
    }
    let candidate = &package.domain_pack_publication_package_candidate;
    let Some(record) = &candidate.record else {
        issue(
            issues,
            DomainPackPublicationIssueCode::PackageCandidateBlocked,
            "package.record",
            "catalog evolution requires a prepared package record",
        );
        return None;
    };
    let signing_request = &assessment
        .signing_request
        .domain_pack_external_signing_request;
    let Some(DomainPackExternalSigningSubject::PublisherRecord {
        registry_id,
        audience,
        package_candidate_digest,
        record: requested_record,
        publisher_credential_id,
    }) = &signing_request.subject
    else {
        issue(
            issues,
            DomainPackPublicationIssueCode::SigningEvidenceMismatch,
            "publisher_signature.signing_request.subject",
            "publisher evidence must bind a publisher-record signing request",
        );
        return None;
    };
    if registry_id != &candidate.registry_id
        || audience != &candidate.audience
        || package_candidate_digest != &candidate.package_candidate_digest
        || requested_record != record
        || publisher_credential_id != &record.publisher_credential_id
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::SigningEvidenceMismatch,
            "publisher_signature.signing_request.subject",
            "publisher evidence does not bind the exact prepared package record",
        );
        return None;
    }
    let evidence = &assessment.evidence.domain_pack_external_signing_evidence;
    let mut signed = record.clone();
    signed
        .publisher_signature_hex
        .clone_from(&evidence.signature_hex);
    Some(signed)
}

fn catalog_from_evolution_base(
    base: &DomainPackCatalogEvolutionBase,
    package: &DomainPackPublicationPackageCandidateDocument,
    record: &DomainPackRegistryPackageRecord,
    issues: &mut Vec<DomainPackPublicationIssue>,
) -> Option<DomainPackSupplyChainRegistryDocument> {
    let candidate = &package.domain_pack_publication_package_candidate;
    let (
        schema_version,
        registry_id,
        registry_version,
        audience,
        generation,
        predecessor,
        issued_at_unix,
        expires_at_unix,
        credentials,
        grants,
        mirrors,
        mut packages,
        mut revocations,
    ) = match base {
        DomainPackCatalogEvolutionBase::Genesis {
            registry_id,
            audience,
            registry_version,
            issued_at_unix,
            expires_at_unix,
            publisher_credentials,
            namespace_grants,
            mirrors,
        } => (
            DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
            registry_id.clone(),
            registry_version.clone(),
            audience.clone(),
            1,
            None,
            *issued_at_unix,
            *expires_at_unix,
            publisher_credentials.clone(),
            namespace_grants.clone(),
            mirrors.clone(),
            Vec::new(),
            Vec::new(),
        ),
        DomainPackCatalogEvolutionBase::Successor {
            current,
            registry_version,
            issued_at_unix,
            expires_at_unix,
        } => {
            validate_current_catalog(current, "base.current", issues);
            let current = &current.domain_pack_supply_chain_registry;
            let generation = current.generation.checked_add(1);
            if generation.is_none() {
                issue(
                    issues,
                    DomainPackPublicationIssueCode::CatalogGenerationMismatch,
                    "base.current.generation",
                    "catalog generation overflows while creating a successor",
                );
            }
            (
                current_schema_version_or_default(current),
                current.registry_id.clone(),
                registry_version.clone(),
                current.audience.clone(),
                generation.unwrap_or(0),
                Some(current.snapshot_digest.clone()),
                *issued_at_unix,
                *expires_at_unix,
                current.publisher_credentials.clone(),
                current.namespace_grants.clone(),
                current.mirrors.clone(),
                current.packages.clone(),
                current.revocations.clone(),
            )
        }
    };
    if registry_id != candidate.registry_id || audience != candidate.audience {
        issue(
            issues,
            DomainPackPublicationIssueCode::CatalogIdentityMismatch,
            "base",
            "catalog registry id and audience must match the package signing target",
        );
    }
    if registry_version.trim().is_empty() || issued_at_unix >= expires_at_unix {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidContract,
            "base",
            "catalog version must be nonblank and validity window must increase",
        );
    }
    if packages.len() >= MAX_PUBLICATION_PACKAGES {
        issue(
            issues,
            DomainPackPublicationIssueCode::ResourceLimitExceeded,
            "base.packages",
            "catalog package limit exceeded",
        );
    }
    if packages
        .iter()
        .any(|existing| existing.identity == record.identity)
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::DuplicatePackage,
            "package.record.identity",
            "catalog already contains this exact publisher/name/namespace/version record",
        );
    } else {
        packages.push(record.clone());
    }
    if revocations
        .iter()
        .any(|fact| fact.record_digest == record.record_digest)
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::CumulativeRevocationMismatch,
            "package.record.record_digest",
            "a cumulatively revoked exact record cannot be reintroduced",
        );
    }
    validate_cumulative_revocations(&revocations, "base.revocations", issues);
    sort_revocations(&mut revocations);
    validate_registry_membership(record, &credentials, &grants, issues);
    if issues.iter().any(|problem| {
        matches!(
            problem.code,
            DomainPackPublicationIssueCode::CatalogIdentityMismatch
                | DomainPackPublicationIssueCode::CatalogGenerationMismatch
                | DomainPackPublicationIssueCode::InvalidContract
                | DomainPackPublicationIssueCode::DuplicatePackage
                | DomainPackPublicationIssueCode::CumulativeRevocationMismatch
                | DomainPackPublicationIssueCode::ResourceLimitExceeded
        )
    }) {
        return None;
    }
    Some(domain_pack_unsigned_catalog_document(
        schema_version,
        registry_id,
        registry_version,
        audience,
        generation,
        predecessor,
        issued_at_unix,
        expires_at_unix,
        credentials,
        grants,
        mirrors,
        packages,
        revocations,
    ))
}

fn current_schema_version_or_default(
    current: &forge_core_contracts::DomainPackSupplyChainRegistry,
) -> String {
    // Registry schema is held on the wrapper; all established C6.2 records use
    // the lifecycle schema. A successor never infers a new schema from payload.
    let _ = current;
    DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned()
}

fn validate_current_catalog(
    current: &DomainPackSupplyChainRegistryDocument,
    path: &str,
    issues: &mut Vec<DomainPackPublicationIssue>,
) {
    let registry = &current.domain_pack_supply_chain_registry;
    if current.schema_version != DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidContract,
            format!("{path}.schema_version"),
            "current catalog must use the established lifecycle registry schema",
        );
    }
    require_id(issues, format!("{path}.registry_id"), &registry.registry_id);
    require_id(issues, format!("{path}.audience"), &registry.audience);
    if !sha256_token(&registry.snapshot_digest) {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidDigest,
            format!("{path}.snapshot_digest"),
            "current catalog snapshot digest is malformed",
        );
    }
    if current.validate_remote_acquisition_metadata().is_err() {
        issue(
            issues,
            DomainPackPublicationIssueCode::CatalogMetadataInvalid,
            path,
            "current catalog has malformed C6.2 immutable descriptor metadata",
        );
    }
    if registry.packages.len() > MAX_PUBLICATION_PACKAGES
        || registry.revocations.len() > MAX_PUBLICATION_REVOCATIONS
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::ResourceLimitExceeded,
            path,
            "current catalog exceeds the C7.2 bounded candidate limits",
        );
    }
    validate_cumulative_revocations(
        &registry.revocations,
        &format!("{path}.revocations"),
        issues,
    );
}

fn validate_registry_membership(
    record: &DomainPackRegistryPackageRecord,
    credentials: &[forge_core_contracts::DomainPackPublisherCredential],
    grants: &[forge_core_contracts::DomainPackNamespaceGrant],
    issues: &mut Vec<DomainPackPublicationIssue>,
) {
    let credential = credentials
        .iter()
        .find(|credential| credential.credential_id == record.publisher_credential_id);
    if credential.is_none_or(|credential| credential.publisher != record.identity.publisher) {
        issue(
            issues,
            DomainPackPublicationIssueCode::CatalogMetadataInvalid,
            "record.publisher_credential_id",
            "catalog credentials do not contain an exact publisher credential for this record",
        );
    }
    let grant = grants
        .iter()
        .find(|grant| grant.grant_id == record.namespace_grant_id);
    if !grant.is_some_and(|grant| {
        grant.publisher == record.identity.publisher
            && namespace_is_within(&record.identity.namespace.0, &grant.namespace_prefix.0)
    }) {
        issue(
            issues,
            DomainPackPublicationIssueCode::CatalogMetadataInvalid,
            "record.namespace_grant_id",
            "catalog grants do not cover this record namespace and publisher",
        );
    }
}

fn validate_catalog_candidate(
    document: &DomainPackCatalogPublicationCandidateDocument,
    path: &str,
    issues: &mut Vec<DomainPackPublicationIssue>,
) {
    let candidate = &document.domain_pack_catalog_publication_candidate;
    if document.schema_version != DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION
        || catalog_candidate_digest_value(candidate) != candidate.catalog_candidate_digest
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidDigest,
            format!("{path}.catalog_candidate_digest"),
            "catalog candidate digest does not bind the supplied candidate",
        );
    }
    if candidate.status != DomainPackCatalogPublicationStatus::CatalogCandidateReady
        || candidate.catalog.is_none()
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::SigningRequestBlocked,
            path,
            "an unsigned catalog candidate is required for registry signing",
        );
        return;
    }
    let Some(catalog) = candidate.catalog.as_ref() else {
        return;
    };
    let registry = &catalog.domain_pack_supply_chain_registry;
    if !registry.snapshot_digest.is_empty() || !registry.signatures.is_empty() {
        issue(
            issues,
            DomainPackPublicationIssueCode::SigningRequestBlocked,
            format!("{path}.catalog"),
            "registry signing requires an unsigned C7.2 catalog with no snapshot digest or signatures",
        );
    }
    if candidate.cumulative_revocations != registry.revocations {
        issue(
            issues,
            DomainPackPublicationIssueCode::CumulativeRevocationMismatch,
            format!("{path}.cumulative_revocations"),
            "candidate cumulative revocations must exactly match the catalog facts",
        );
    }
    let mut sorted_revocations = registry.revocations.clone();
    sort_revocations(&mut sorted_revocations);
    if registry.revocations != sorted_revocations {
        issue(
            issues,
            DomainPackPublicationIssueCode::CumulativeRevocationMismatch,
            format!("{path}.catalog.revocations"),
            "catalog candidate revocations must use deterministic record-digest ordering",
        );
    }
    validate_cumulative_revocations(
        &registry.revocations,
        &format!("{path}.catalog.revocations"),
        issues,
    );
    if catalog.validate_remote_acquisition_metadata().is_err() {
        issue(
            issues,
            DomainPackPublicationIssueCode::CatalogMetadataInvalid,
            format!("{path}.catalog"),
            "catalog candidate has malformed C6.2 immutable descriptor metadata",
        );
    }
}

fn validate_signing_request(
    document: &DomainPackExternalSigningRequestDocument,
    issues: &mut Vec<DomainPackPublicationIssue>,
) {
    let request = &document.domain_pack_external_signing_request;
    if document.schema_version != DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION
        || signing_request_digest_value(request) != request.signing_request_digest
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidDigest,
            "signing_request.signing_request_digest",
            "signing request digest does not bind the supplied request",
        );
    }
    require_id(issues, "signing_request.request_id", &request.request_id);
}

fn validate_signing_evidence(
    document: &DomainPackExternalSigningEvidenceDocument,
    issues: &mut Vec<DomainPackPublicationIssue>,
) {
    let evidence = &document.domain_pack_external_signing_evidence;
    if document.schema_version != DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION
        || signing_evidence_digest_value(evidence) != evidence.evidence_digest
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidDigest,
            "evidence.evidence_digest",
            "external signing evidence digest does not bind the supplied evidence",
        );
    }
    require_id(issues, "evidence.evidence_id", &evidence.evidence_id);
    require_id(issues, "evidence.signer_key_id", &evidence.signer_key_id);
    if !sha256_token(&evidence.signing_request_digest) {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidDigest,
            "evidence.signing_request_digest",
            "external evidence must bind a SHA-256 signing-request digest",
        );
    }
    if evidence.algorithm != DomainPackExternalSignatureAlgorithm::Ed25519
        || !lower_hex(&evidence.signature_hex)
        || evidence.signature_hex.len() != 128
    {
        issue(
            issues,
            DomainPackPublicationIssueCode::MalformedExternalSignature,
            "evidence.signature_hex",
            "external evidence must contain one lowercase hexadecimal Ed25519 signature",
        );
    }
}

fn validate_revocation(
    revocation: &DomainPackPackageRevocation,
    path: &str,
    issues: &mut Vec<DomainPackPublicationIssue>,
) {
    if !sha256_token(&revocation.record_digest) {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidDigest,
            format!("{path}.record_digest"),
            "revocation record digest is malformed",
        );
    }
    if revocation.explanation.trim().is_empty() {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidContract,
            format!("{path}.explanation"),
            "revocation explanation must not be blank",
        );
    }
}

fn validate_cumulative_revocations(
    revocations: &[DomainPackPackageRevocation],
    path: &str,
    issues: &mut Vec<DomainPackPublicationIssue>,
) {
    if revocations.len() > MAX_PUBLICATION_REVOCATIONS {
        issue(
            issues,
            DomainPackPublicationIssueCode::ResourceLimitExceeded,
            path,
            "cumulative revocation limit exceeded",
        );
    }
    let mut seen = BTreeSet::new();
    for (index, revocation) in revocations.iter().enumerate() {
        validate_revocation(revocation, &format!("{path}[{index}]"), issues);
        if !seen.insert(revocation.record_digest.as_str()) {
            issue(
                issues,
                DomainPackPublicationIssueCode::DuplicateRevocation,
                format!("{path}[{index}].record_digest"),
                "each exact record may occur in cumulative revocation facts once",
            );
        }
    }
}

fn sort_revocations(revocations: &mut [DomainPackPackageRevocation]) {
    revocations.sort_by(|left, right| left.record_digest.cmp(&right.record_digest));
}

fn catalog_candidate_document(
    request_id: StableId,
    catalog: Option<DomainPackSupplyChainRegistryDocument>,
    mut issues: Vec<DomainPackPublicationIssue>,
) -> DomainPackCatalogPublicationCandidateDocument {
    finish_issues(&mut issues);
    let status = if catalog.is_some() && issues.is_empty() {
        DomainPackCatalogPublicationStatus::CatalogCandidateReady
    } else {
        DomainPackCatalogPublicationStatus::Blocked
    };
    let cumulative_revocations = catalog
        .as_ref()
        .map(|catalog| {
            catalog
                .domain_pack_supply_chain_registry
                .revocations
                .clone()
        })
        .unwrap_or_default();
    let mut candidate = DomainPackCatalogPublicationCandidate {
        request_id,
        authority: DomainPackCandidateAuthority::CandidateOnly,
        status,
        catalog: if status == DomainPackCatalogPublicationStatus::CatalogCandidateReady {
            catalog
        } else {
            None
        },
        cumulative_revocations,
        issues,
        catalog_candidate_digest: String::new(),
    };
    candidate.catalog_candidate_digest = catalog_candidate_digest_value(&candidate);
    DomainPackCatalogPublicationCandidateDocument {
        schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
        domain_pack_catalog_publication_candidate: candidate,
    }
}

fn package_candidate_digest_value(candidate: &DomainPackPublicationPackageCandidate) -> String {
    canonical_digest_without_field(
        candidate,
        "package_candidate_digest",
        b"forge-domain-pack-publication-package-canonical-encoding-failed",
    )
}

fn review_readiness_digest_value(readiness: &DomainPackPublicationReviewReadiness) -> String {
    canonical_digest_without_field(
        readiness,
        "review_readiness_digest",
        b"forge-domain-pack-publication-review-canonical-encoding-failed",
    )
}

fn signing_request_digest_value(request: &DomainPackExternalSigningRequest) -> String {
    canonical_digest_without_field(
        request,
        "signing_request_digest",
        b"forge-domain-pack-publication-signing-request-canonical-encoding-failed",
    )
}

fn signing_evidence_digest_value(
    evidence: &forge_core_contracts::DomainPackExternalSigningEvidence,
) -> String {
    canonical_digest_without_field(
        evidence,
        "evidence_digest",
        b"forge-domain-pack-publication-signing-evidence-canonical-encoding-failed",
    )
}

fn signing_assessment_digest_value(
    assessment: &DomainPackExternalSigningEvidenceAssessment,
) -> String {
    canonical_digest_without_field(
        assessment,
        "assessment_digest",
        b"forge-domain-pack-publication-signing-assessment-canonical-encoding-failed",
    )
}

fn catalog_candidate_digest_value(candidate: &DomainPackCatalogPublicationCandidate) -> String {
    canonical_digest_without_field(
        candidate,
        "catalog_candidate_digest",
        b"forge-domain-pack-publication-catalog-canonical-encoding-failed",
    )
}

fn canonical_digest_without_field<T: Serialize>(value: &T, field: &str, fallback: &[u8]) -> String {
    let result = serde_json::to_value(value)
        .ok()
        .and_then(|mut value| {
            value
                .as_object_mut()
                .and_then(|object| object.remove(field))?;
            serde_json_canonicalizer::to_vec(&value).ok()
        })
        .unwrap_or_else(|| fallback.to_vec());
    format!("sha256:{:x}", Sha256::digest(result))
}

fn sha256_token(value: &str) -> bool {
    value.len() == 71 && value.starts_with("sha256:") && lower_hex(&value["sha256:".len()..])
}

fn lower_hex(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn namespace_is_within(namespace: &str, prefix: &str) -> bool {
    namespace == prefix
        || namespace
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn require_id(
    issues: &mut Vec<DomainPackPublicationIssue>,
    path: impl Into<String>,
    value: &StableId,
) {
    if value.0.trim().is_empty() {
        issue(
            issues,
            DomainPackPublicationIssueCode::InvalidIdentifier,
            path,
            "stable identifier must not be blank",
        );
    }
}

fn issue(
    issues: &mut Vec<DomainPackPublicationIssue>,
    code: DomainPackPublicationIssueCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(DomainPackPublicationIssue {
        code,
        path: path.into(),
        message: message.into(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
    });
}

fn finish_issues(issues: &mut Vec<DomainPackPublicationIssue>) {
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.message.cmp(&right.message))
    });
    issues.dedup();
}
