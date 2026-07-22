//! Closed C7.2 candidate-only Domain Pack publication workflow contracts.
//!
//! These documents prepare exact package, external-signing, independent-review,
//! catalog-evolution, and revocation evidence. They deliberately do not own
//! private keys, produce or verify signatures, admit a catalog, perform remote
//! publication, or grant lifecycle installation or activation authority.

use crate::{
    DomainPackAuthorLearningEvidence, DomainPackAuthorTestReportDocument,
    DomainPackAuthorTestRequestDocument, DomainPackCandidateAuthority,
    DomainPackIndependentReviewDocument, DomainPackNamespaceGrant, DomainPackPackageRevocation,
    DomainPackPublisherCredential, DomainPackRegistryMirror, DomainPackRegistryPackageRecord,
    DomainPackRegistrySignature, DomainPackRegistryTrustRole, DomainPackSourceKind,
    DomainPackSupplyChainRegistryDocument, StableId,
};
use serde::{Deserialize, Serialize};

pub const DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION: &str = "0.1";

/// The signing protocol is named so an external signer must obtain the exact
/// payload from the existing supply-chain authority API. C7.2 never materializes
/// signing bytes or takes key material on its wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackExternalSigningProtocol {
    ExistingSupplyChainAuthorityV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackExternalSignatureAlgorithm {
    Ed25519,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPublicationPackageRequestDocument {
    pub schema_version: String,
    pub domain_pack_publication_package_request: DomainPackPublicationPackageRequest,
}

/// Binds a C7.1 author-test request to the exact registry record that a later,
/// independent authority owner will digest and sign. The record has no detached
/// publisher signature at this preparation stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPublicationPackageRequest {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub registry_id: StableId,
    pub audience: StableId,
    pub author_test: DomainPackAuthorTestRequestDocument,
    pub record: DomainPackRegistryPackageRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackPublicationPackageStatus {
    PreparedCandidate,
    Blocked,
}

/// Exact source provenance copied from the authored manifest and additionally
/// bound to the generated C7.1 report and immutable package descriptors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPublicationProvenanceBinding {
    pub author_test_report_digest: String,
    pub source_kind: DomainPackSourceKind,
    pub source_uri: String,
    pub source_revision: String,
    pub source_digest: String,
    pub authors: Vec<StableId>,
    pub license_spdx_expression: String,
    pub manifest_raw_sha256: String,
    pub content_raw_sha256: String,
    pub license_raw_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPublicationPackageCandidateDocument {
    pub schema_version: String,
    pub domain_pack_publication_package_candidate: DomainPackPublicationPackageCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPublicationPackageCandidate {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub registry_id: StableId,
    pub audience: StableId,
    pub status: DomainPackPublicationPackageStatus,
    /// The exact source request is retained so later C7.2 steps can rerun C7.1
    /// evaluation rather than treating this candidate status as a caller claim.
    pub author_test: DomainPackAuthorTestRequestDocument,
    /// The report is freshly derived from the retained C7.1 request; it is not
    /// accepted as a caller assertion.
    pub author_test_report: DomainPackAuthorTestReportDocument,
    /// The immutable subject is present only after all C7.1 and C7.2 package
    /// bindings are coherent. Its `record_digest` remains an unverified claim
    /// until the existing supply-chain authority recomputes it.
    pub record: Option<DomainPackRegistryPackageRecord>,
    pub provenance: Option<DomainPackPublicationProvenanceBinding>,
    pub issues: Vec<DomainPackPublicationIssue>,
    pub package_candidate_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPublicationReviewRequestDocument {
    pub schema_version: String,
    pub domain_pack_publication_review_request: DomainPackPublicationReviewRequest,
}

/// C7.2 re-evaluates the durable P6c graph rather than treating a present
/// reviewer document as an approval. All review evidence stays candidate-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPublicationReviewRequest {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub package: DomainPackPublicationPackageCandidateDocument,
    pub learning: DomainPackAuthorLearningEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackPublicationReviewStatus {
    EvidenceReady,
    ReviewRequired,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPublicationReviewReadinessDocument {
    pub schema_version: String,
    pub domain_pack_publication_review_readiness: DomainPackPublicationReviewReadiness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPublicationReviewReadiness {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub package_candidate_digest: String,
    pub dossier_digest: Option<String>,
    pub promotion_evaluation_digest: String,
    pub status: DomainPackPublicationReviewStatus,
    /// The full P6c input is retained so a later C7.2 catalog proposal can
    /// reproduce this candidate-only readiness projection instead of trusting
    /// a caller-supplied status field.
    pub learning: DomainPackAuthorLearningEvidence,
    /// Exact reviewer records are copied as evidence, never converted into a
    /// verified review or promotion authority by this workflow.
    pub independent_reviews: Vec<DomainPackIndependentReviewDocument>,
    pub issues: Vec<DomainPackPublicationIssue>,
    pub review_readiness_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackExternalSigningRequestInputDocument {
    pub schema_version: String,
    pub domain_pack_external_signing_request_input: DomainPackExternalSigningRequestInput,
}

/// The source is deliberately a C7.2 candidate, rather than arbitrary bytes.
/// That prevents a signing request from silently discarding the package/review
/// or catalog-evolution bindings that the workflow has already checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainPackExternalSigningRequestSource {
    PublisherRecord {
        package: DomainPackPublicationPackageCandidateDocument,
        publisher_credential_id: StableId,
    },
    RegistrySnapshot {
        catalog: DomainPackCatalogPublicationCandidateDocument,
        signer_key_id: StableId,
        role: DomainPackRegistryTrustRole,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackExternalSigningRequestInput {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub source: DomainPackExternalSigningRequestSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackExternalSigningRequestStatus {
    ExternalEvidenceRequired,
    Blocked,
}

/// Exact C6.2 subject information handed to an external signer. The named
/// protocol tells the signer to call the existing authority-owned canonical
/// signing-byte builder; this document never contains signing bytes or keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainPackExternalSigningSubject {
    PublisherRecord {
        registry_id: StableId,
        audience: StableId,
        package_candidate_digest: String,
        record: DomainPackRegistryPackageRecord,
        publisher_credential_id: StableId,
    },
    RegistrySnapshot {
        catalog_candidate_digest: String,
        catalog: DomainPackSupplyChainRegistryDocument,
        signer_key_id: StableId,
        role: DomainPackRegistryTrustRole,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackExternalSigningRequestDocument {
    pub schema_version: String,
    pub domain_pack_external_signing_request: DomainPackExternalSigningRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackExternalSigningRequest {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub protocol: DomainPackExternalSigningProtocol,
    pub status: DomainPackExternalSigningRequestStatus,
    pub subject: Option<DomainPackExternalSigningSubject>,
    pub issues: Vec<DomainPackPublicationIssue>,
    pub signing_request_digest: String,
}

/// Externally supplied detached-signature evidence. `UnverifiedEvidence` is
/// intentional: only the supply-chain authority owner may verify cryptographic
/// correctness and admit the result to a catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackExternalSigningEvidenceDocument {
    pub schema_version: String,
    pub domain_pack_external_signing_evidence: DomainPackExternalSigningEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackExternalSigningEvidence {
    pub evidence_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub signing_request_digest: String,
    pub signer_key_id: StableId,
    pub algorithm: DomainPackExternalSignatureAlgorithm,
    pub signature_hex: String,
    pub supplied_at_unix: u64,
    pub evidence_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackExternalSigningEvidenceStatus {
    UnverifiedEvidenceBound,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackExternalSigningEvidenceAssessmentDocument {
    pub schema_version: String,
    pub domain_pack_external_signing_evidence_assessment:
        DomainPackExternalSigningEvidenceAssessment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackExternalSigningEvidenceAssessment {
    pub evidence_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub signing_request: DomainPackExternalSigningRequestDocument,
    pub evidence: DomainPackExternalSigningEvidenceDocument,
    pub status: DomainPackExternalSigningEvidenceStatus,
    pub issues: Vec<DomainPackPublicationIssue>,
    pub assessment_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCatalogEvolutionRequestDocument {
    pub schema_version: String,
    pub domain_pack_catalog_evolution_request: DomainPackCatalogEvolutionRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainPackCatalogEvolutionBase {
    Genesis {
        registry_id: StableId,
        audience: StableId,
        registry_version: String,
        issued_at_unix: u64,
        expires_at_unix: u64,
        publisher_credentials: Vec<DomainPackPublisherCredential>,
        namespace_grants: Vec<DomainPackNamespaceGrant>,
        mirrors: Vec<DomainPackRegistryMirror>,
    },
    Successor {
        current: DomainPackSupplyChainRegistryDocument,
        registry_version: String,
        issued_at_unix: u64,
        expires_at_unix: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCatalogEvolutionRequest {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub base: DomainPackCatalogEvolutionBase,
    pub package: DomainPackPublicationPackageCandidateDocument,
    pub publisher_signature: DomainPackExternalSigningEvidenceAssessmentDocument,
    pub review: DomainPackPublicationReviewReadinessDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCatalogRevocationRequestDocument {
    pub schema_version: String,
    pub domain_pack_catalog_revocation_request: DomainPackCatalogRevocationRequest,
}

/// The current catalog can be supplied as untrusted candidate evidence, but all
/// prior revocation facts must occur verbatim in the proposed successor. A later
/// anchor still verifies the catalog and enforces its own protected history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCatalogRevocationRequest {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub current: DomainPackSupplyChainRegistryDocument,
    pub registry_version: String,
    pub issued_at_unix: u64,
    pub expires_at_unix: u64,
    pub revocation: DomainPackPackageRevocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackCatalogPublicationStatus {
    CatalogCandidateReady,
    Blocked,
}

/// A full C6.2 catalog-shaped candidate. `snapshot_digest` is deliberately
/// empty and `signatures` are absent until the existing authority owner computes
/// the canonical snapshot, verifies detached evidence, and decides admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCatalogPublicationCandidateDocument {
    pub schema_version: String,
    pub domain_pack_catalog_publication_candidate: DomainPackCatalogPublicationCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackCatalogPublicationCandidate {
    pub request_id: StableId,
    pub authority: DomainPackCandidateAuthority,
    pub status: DomainPackCatalogPublicationStatus,
    pub catalog: Option<DomainPackSupplyChainRegistryDocument>,
    /// C7.2 carries these exact candidate facts to make revocation carry-forward
    /// auditable. It does not calculate an authority-side revocation digest.
    pub cumulative_revocations: Vec<DomainPackPackageRevocation>,
    pub issues: Vec<DomainPackPublicationIssue>,
    pub catalog_candidate_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackPublicationIssueCode {
    InvalidContract,
    InvalidIdentifier,
    InvalidDigest,
    AuthorTestBlocked,
    MissingCompatibilityEvidence,
    CompatibilityBlocked,
    ArtifactBindingMismatch,
    FixtureBindingMismatch,
    ProvenanceMismatch,
    PublisherSignaturePresentBeforeRequest,
    RecordDigestUnverified,
    PackageCandidateBlocked,
    ReviewEvidenceMismatch,
    ReviewReadinessBlocked,
    SigningRequestBlocked,
    SigningEvidenceMismatch,
    MalformedExternalSignature,
    CatalogIdentityMismatch,
    CatalogGenerationMismatch,
    CatalogPredecessorMismatch,
    DuplicatePackage,
    CumulativeRevocationMismatch,
    DuplicateRevocation,
    RevocationRecordMissing,
    CatalogMetadataInvalid,
    ResourceLimitExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainPackPublicationIssue {
    pub code: DomainPackPublicationIssueCode,
    pub path: String,
    pub message: String,
    pub authority: DomainPackCandidateAuthority,
}

/// Builds an unsigned catalog document whose fields are deliberately suitable
/// only as a future authority signing subject. Keeping this helper in the
/// contract layer avoids making a serializable "published" or "admitted" state
/// representable in C7.2.
#[must_use]
pub fn domain_pack_unsigned_catalog_document(
    schema_version: String,
    registry_id: StableId,
    registry_version: String,
    audience: StableId,
    generation: u64,
    previous_snapshot_digest: Option<String>,
    issued_at_unix: u64,
    expires_at_unix: u64,
    publisher_credentials: Vec<DomainPackPublisherCredential>,
    namespace_grants: Vec<DomainPackNamespaceGrant>,
    mirrors: Vec<DomainPackRegistryMirror>,
    packages: Vec<DomainPackRegistryPackageRecord>,
    revocations: Vec<DomainPackPackageRevocation>,
) -> DomainPackSupplyChainRegistryDocument {
    DomainPackSupplyChainRegistryDocument {
        schema_version,
        domain_pack_supply_chain_registry: crate::DomainPackSupplyChainRegistry {
            registry_id,
            registry_version,
            audience,
            authority: DomainPackCandidateAuthority::CandidateOnly,
            generation,
            previous_snapshot_digest,
            issued_at_unix,
            expires_at_unix,
            publisher_credentials,
            namespace_grants,
            mirrors,
            packages,
            revocations,
            snapshot_digest: String::new(),
            signatures: Vec::<DomainPackRegistrySignature>::new(),
        },
    }
}
