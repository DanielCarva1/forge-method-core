use forge_core_contracts::{
    DomainPackCandidateAuthority, DomainPackCatalogPublicationCandidate,
    DomainPackCatalogPublicationCandidateDocument, DomainPackCatalogPublicationStatus,
    DomainPackExternalSignatureAlgorithm, DomainPackExternalSigningEvidence,
    DomainPackExternalSigningEvidenceDocument, DomainPackPublicationIssue,
    DomainPackPublicationIssueCode, StableId, DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION,
};

fn digest(hex: char) -> String {
    format!("sha256:{}", hex.to_string().repeat(64))
}

#[test]
fn publication_contracts_deny_unknown_fields_and_invented_authority() {
    let issue = DomainPackPublicationIssue {
        code: DomainPackPublicationIssueCode::ArtifactBindingMismatch,
        path: "record.artifacts".to_owned(),
        message: "exact descriptor mismatch".to_owned(),
        authority: DomainPackCandidateAuthority::CandidateOnly,
    };
    let mut issue_value = serde_json::to_value(issue).expect("issue JSON");
    assert_eq!(issue_value["authority"], "candidate_only");
    issue_value["authority"] = serde_json::json!("admitted");
    assert!(serde_json::from_value::<DomainPackPublicationIssue>(issue_value).is_err());

    let document = DomainPackCatalogPublicationCandidateDocument {
        schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
        domain_pack_catalog_publication_candidate: DomainPackCatalogPublicationCandidate {
            request_id: StableId("catalog.candidate".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            status: DomainPackCatalogPublicationStatus::Blocked,
            catalog: None,
            cumulative_revocations: Vec::new(),
            issues: Vec::new(),
            catalog_candidate_digest: digest('a'),
        },
    };
    let mut value = serde_json::to_value(document).expect("candidate JSON");
    value["domain_pack_catalog_publication_candidate"]["published"] = serde_json::json!(true);
    assert!(
        serde_json::from_value::<DomainPackCatalogPublicationCandidateDocument>(value).is_err()
    );
}

#[test]
fn external_signature_evidence_is_detached_candidate_evidence_only() {
    let evidence = DomainPackExternalSigningEvidenceDocument {
        schema_version: DOMAIN_PACK_PUBLICATION_SCHEMA_VERSION.to_owned(),
        domain_pack_external_signing_evidence: DomainPackExternalSigningEvidence {
            evidence_id: StableId("evidence.external".to_owned()),
            authority: DomainPackCandidateAuthority::CandidateOnly,
            signing_request_digest: digest('b'),
            signer_key_id: StableId("publisher.credential".to_owned()),
            algorithm: DomainPackExternalSignatureAlgorithm::Ed25519,
            signature_hex: "0".repeat(128),
            supplied_at_unix: 42,
            evidence_digest: digest('c'),
        },
    };
    let serialized = serde_json::to_string(&evidence).expect("evidence JSON");
    assert!(serialized.contains("candidate_only"));
    assert!(!serialized.contains("private_key"));
    assert!(!serialized.contains("verified_signature"));

    let mut value = serde_json::to_value(evidence).expect("evidence JSON value");
    value["domain_pack_external_signing_evidence"]["private_key"] = serde_json::json!("forbidden");
    assert!(serde_json::from_value::<DomainPackExternalSigningEvidenceDocument>(value).is_err());
}
