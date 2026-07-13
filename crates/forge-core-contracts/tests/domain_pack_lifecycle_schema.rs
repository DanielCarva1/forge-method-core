use forge_core_contracts::{
    DomainPackActivePointerDocument, DomainPackCapabilitySandboxPolicyDocument,
    DomainPackCompatibilityReportDocument, DomainPackExactLockDocument,
    DomainPackLifecycleLedgerDocument, DomainPackLifecycleReceiptDocument,
    DomainPackManifestDocument, DomainPackRecoveryReportDocument,
    DomainPackResolutionProjectionDocument, DomainPackResolutionRequestDocument,
    DomainPackRuntimeCapabilityRegistryDocument, DomainPackSupplyChainRegistryDocument,
    DomainPackTrustPolicyDocument, DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
};

const ROOT: &str = "../../docs/fixtures/domain-pack-lifecycle-v0";

fn fixture(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(ROOT);
    std::fs::read_to_string(root.join(path)).expect("fixture is readable")
}

#[test]
fn representative_v03_documents_are_closed_and_candidate_only() {
    let trust: DomainPackTrustPolicyDocument =
        yaml_serde::from_str(&fixture("valid/trust-policy.yaml")).expect("trust policy");
    let registry: DomainPackSupplyChainRegistryDocument =
        yaml_serde::from_str(&fixture("valid/supply-chain-registry.yaml"))
            .expect("supply-chain registry");
    let capabilities: DomainPackRuntimeCapabilityRegistryDocument =
        yaml_serde::from_str(&fixture("valid/runtime-capability-registry.yaml"))
            .expect("capability registry");
    let sandbox: DomainPackCapabilitySandboxPolicyDocument =
        yaml_serde::from_str(&fixture("valid/capability-sandbox-policy.yaml"))
            .expect("sandbox policy");
    let compatibility: DomainPackCompatibilityReportDocument =
        yaml_serde::from_str(&fixture("valid/compatibility-report.yaml"))
            .expect("compatibility report");
    let pointer: DomainPackActivePointerDocument =
        yaml_serde::from_str(&fixture("valid/active-pointer.yaml")).expect("active pointer");
    let ledger: DomainPackLifecycleLedgerDocument =
        yaml_serde::from_str(&fixture("valid/lifecycle-ledger.yaml")).expect("lifecycle ledger");
    let recovery: DomainPackRecoveryReportDocument =
        yaml_serde::from_str(&fixture("valid/recovery-report.yaml")).expect("recovery report");
    let resolution_request: DomainPackResolutionRequestDocument =
        yaml_serde::from_str(&fixture("valid/resolution-request.yaml"))
            .expect("resolution request");
    let resolution_projection: DomainPackResolutionProjectionDocument =
        yaml_serde::from_str(&fixture("valid/resolution-projection.yaml"))
            .expect("resolution projection");
    let exact_lock: DomainPackExactLockDocument =
        yaml_serde::from_str(&fixture("valid/exact-lock.yaml")).expect("exact lock");
    let receipt: DomainPackLifecycleReceiptDocument =
        yaml_serde::from_str(&fixture("valid/lifecycle-receipt.yaml")).expect("lifecycle receipt");

    for schema in [
        trust.schema_version,
        registry.schema_version,
        capabilities.schema_version,
        sandbox.schema_version,
        compatibility.schema_version,
        pointer.schema_version,
        ledger.schema_version,
        recovery.schema_version,
        resolution_request.schema_version,
        resolution_projection.schema_version,
        exact_lock.schema_version,
        receipt.schema_version,
    ] {
        assert_eq!(schema, DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION);
    }
}

#[test]
fn authority_escalation_self_grant_and_external_execution_shortcuts_are_rejected() {
    assert!(yaml_serde::from_str::<DomainPackManifestDocument>(&fixture(
        "adversarial/manifest-self-grant.invalid.yaml"
    ))
    .is_err());
    assert!(
        yaml_serde::from_str::<DomainPackTrustPolicyDocument>(&fixture(
            "adversarial/trust-policy-active-authority.invalid.yaml"
        ))
        .is_err()
    );
    assert!(
        yaml_serde::from_str::<DomainPackCapabilitySandboxPolicyDocument>(&fixture(
            "adversarial/sandbox-external-allow.invalid.yaml"
        ))
        .is_err()
    );
    assert!(
        yaml_serde::from_str::<DomainPackRecoveryReportDocument>(&fixture(
            "adversarial/recovery-authoritative.invalid.yaml"
        ))
        .is_err()
    );
}
