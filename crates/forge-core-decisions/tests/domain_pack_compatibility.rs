use forge_core_contracts::{
    DomainPackCompatibilityIssueCode, DomainPackCompatibilityStatus, DomainPackCompositionGap,
    DomainPackCompositionGapCode, DomainPackCompositionRequestDocument, DomainPackCoordinate,
    DomainPackExactLock, DomainPackExactLockDocument, DomainPackExactLockPayload,
    DomainPackLifecycleOperation, DomainPackLockedPackage, DomainPackReceiptMigrationPolicy,
    DomainPackRuntimeCapabilityGap, DomainPackRuntimeCapabilityGapCode,
    DomainPackSemanticChangeKind, DomainPackSourceAssurance, DomainPackVersionReference, StableId,
    DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION,
};
use forge_core_decisions::{evaluate_domain_pack_compatibility, DomainPackCompatibilityInput};
use sha2::{Digest, Sha256};

const A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const D: &str = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn request() -> DomainPackCompositionRequestDocument {
    let path = repo_root().join("docs/fixtures/domain-pack-v0/requests/neutral-two-pack.yaml");
    yaml_serde::from_str(&std::fs::read_to_string(path).expect("composition fixture"))
        .expect("typed composition request")
}

fn locked_package(assurance: DomainPackSourceAssurance) -> DomainPackLockedPackage {
    let request = request();
    let candidate = &request.domain_pack_composition_request.candidates[0];
    let manifest = &candidate.manifest.domain_pack_manifest;
    DomainPackLockedPackage {
        identity: manifest.identity.clone(),
        package_digest: A.to_owned(),
        manifest_binding: candidate.manifest_binding.clone(),
        content_binding: manifest.content.clone(),
        license_binding: manifest.provenance.license_text.clone(),
        fixture_bindings: candidate
            .content
            .domain_pack_content
            .fixtures
            .iter()
            .map(|fixture| fixture.artifact.clone())
            .collect(),
        namespace_grant_id: StableId("grant.sample.foundation".to_owned()),
        registry_record_digest: B.to_owned(),
        source_assurance: assurance,
        dependencies: manifest.dependencies.clone(),
        deterministic_order: 0,
    }
}

fn lock(packages: Vec<DomainPackLockedPackage>) -> DomainPackExactLockDocument {
    let core = request().domain_pack_composition_request.core;
    let payload = DomainPackExactLockPayload {
        project_id: StableId("project.neutral-composition".to_owned()),
        core,
        requirements_digest: A.to_owned(),
        roots: vec![],
        registry_snapshot_digest: B.to_owned(),
        trust_policy_digest: C.to_owned(),
        capability_registry_digest: D.to_owned(),
        sandbox_policy_digest: A.to_owned(),
        resolution_digest: B.to_owned(),
        composition_digest: C.to_owned(),
        packages,
        verified_capability_bindings: vec![],
        unresolved_composition_gaps: vec![],
        unresolved_capability_gaps: vec![],
    };
    let lock_digest = canonical_digest(&payload);
    DomainPackExactLockDocument {
        schema_version: DOMAIN_PACK_LIFECYCLE_SCHEMA_VERSION.to_owned(),
        domain_pack_exact_lock: DomainPackExactLock {
            payload,
            lock_digest,
        },
    }
}

fn operation(kind: &str) -> DomainPackLifecycleOperation {
    let pack = DomainPackCoordinate {
        publisher: StableId("sample".to_owned()),
        name: StableId("foundation".to_owned()),
    };
    match kind {
        "install" => DomainPackLifecycleOperation::Install { root: pack },
        "upgrade" => DomainPackLifecycleOperation::Upgrade {
            pack,
            expected_from: "1.0.0".to_owned(),
            target_requirement: "^2.0".to_owned(),
            required_content_digest: None,
        },
        "remove" => DomainPackLifecycleOperation::Remove { pack },
        "rollback" => DomainPackLifecycleOperation::Rollback {
            target_receipt_digest: A.to_owned(),
            target_lock_digest: B.to_owned(),
        },
        _ => panic!("unknown operation"),
    }
}

fn input(
    operation: DomainPackLifecycleOperation,
    from_lock: Option<DomainPackExactLockDocument>,
    to_lock: DomainPackExactLockDocument,
) -> DomainPackCompatibilityInput {
    DomainPackCompatibilityInput {
        report_id: StableId("report.compatibility".to_owned()),
        operation,
        sealed_core: to_lock.domain_pack_exact_lock.payload.core.clone(),
        from_lock,
        to_lock,
    }
}

fn add_gap(lock: &mut DomainPackExactLockDocument) {
    lock.domain_pack_exact_lock
        .payload
        .unresolved_capability_gaps
        .push(DomainPackRuntimeCapabilityGap {
            code: DomainPackRuntimeCapabilityGapCode::ExternalProviderDenied,
            pack: DomainPackVersionReference {
                publisher: StableId("sample".to_owned()),
                name: StableId("foundation".to_owned()),
                version: "1.0.0".to_owned(),
            },
            subject_ref: StableId("sample.foundation.adapter.repository-view".to_owned()),
            capability_ref: StableId(
                "sample.foundation.capability.repository-inspection".to_owned(),
            ),
            message: "external provider denied".to_owned(),
        });
    resign(lock);
}

fn add_composition_gap(lock: &mut DomainPackExactLockDocument) {
    lock.domain_pack_exact_lock
        .payload
        .unresolved_composition_gaps
        .push(DomainPackCompositionGap {
            code: DomainPackCompositionGapCode::MissingDomain,
            requirement_ref: StableId("requirement.gameplay-loop".to_owned()),
            subject_ref: StableId("domain.gameplay-loop".to_owned()),
            message: "required domain is no longer provided".to_owned(),
            authority: forge_core_contracts::DomainPackCandidateAuthority::CandidateOnly,
        });
    resign(lock);
}

fn resign(lock: &mut DomainPackExactLockDocument) {
    lock.domain_pack_exact_lock.lock_digest =
        canonical_digest(&lock.domain_pack_exact_lock.payload);
}

#[test]
fn exact_equivalence_is_compatible_deterministic_and_preserves_receipts() {
    let old = lock(vec![locked_package(
        DomainPackSourceAssurance::SupplyChainVerified,
    )]);
    let new = old.clone();
    let candidate = input(operation("upgrade"), Some(old), new);
    let first = evaluate_domain_pack_compatibility(&candidate);
    let second = evaluate_domain_pack_compatibility(&candidate);
    assert_eq!(first, second);
    let report = first.domain_pack_compatibility_report;
    assert_eq!(report.status, DomainPackCompatibilityStatus::Compatible);
    assert!(report.universal_core_unchanged);
    assert!(report.changes.is_empty());
    assert!(report.issues.is_empty());
    assert_eq!(
        report.receipt_policy,
        DomainPackReceiptMigrationPolicy::PreserveExactEquivalent
    );
    assert!(report.report_digest.starts_with("sha256:"));
}

#[test]
fn sealed_core_change_and_invalid_lock_digest_block() {
    let old = lock(vec![]);
    let mut new = old.clone();
    new.domain_pack_exact_lock.payload.core.bundle_id =
        StableId("bundle.hostile-replacement".to_owned());
    resign(&mut new);
    let mut candidate = input(operation("upgrade"), Some(old), new);
    // The sealed binding is independent of the target and intentionally keeps
    // the original Core.
    candidate.sealed_core = request().domain_pack_composition_request.core;
    let changed = evaluate_domain_pack_compatibility(&candidate);
    assert_eq!(
        changed.domain_pack_compatibility_report.status,
        DomainPackCompatibilityStatus::Blocked
    );
    assert!(changed
        .domain_pack_compatibility_report
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackCompatibilityIssueCode::CoreChanged));

    candidate.to_lock.domain_pack_exact_lock.lock_digest = A.to_owned();
    let invalid = evaluate_domain_pack_compatibility(&candidate);
    assert!(invalid
        .domain_pack_compatibility_report
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackCompatibilityIssueCode::InvalidLockDigest));
}

#[test]
fn install_upgrade_and_rollback_block_on_capability_gap_but_remove_degrades() {
    for kind in ["install", "upgrade", "rollback"] {
        let old = (kind != "install").then(|| lock(vec![]));
        let mut new = lock(vec![]);
        add_gap(&mut new);
        let report = evaluate_domain_pack_compatibility(&input(operation(kind), old, new))
            .domain_pack_compatibility_report;
        assert_eq!(
            report.status,
            DomainPackCompatibilityStatus::Blocked,
            "{kind}"
        );
        assert!(!report.requirement_impacts.is_empty());
        assert!(!report.capability_impacts.is_empty());
    }

    let old = lock(vec![locked_package(
        DomainPackSourceAssurance::SupplyChainVerified,
    )]);
    let mut new = lock(vec![]);
    add_gap(&mut new);
    let report = evaluate_domain_pack_compatibility(&input(operation("remove"), Some(old), new))
        .domain_pack_compatibility_report;
    assert_eq!(report.status, DomainPackCompatibilityStatus::Degraded);
    assert!(report
        .changes
        .iter()
        .any(|change| change.kind == DomainPackSemanticChangeKind::PackRemoved));

    let old = lock(vec![locked_package(
        DomainPackSourceAssurance::SupplyChainVerified,
    )]);
    let mut new = lock(vec![]);
    add_composition_gap(&mut new);
    let report = evaluate_domain_pack_compatibility(&input(operation("remove"), Some(old), new))
        .domain_pack_compatibility_report;
    assert_eq!(report.status, DomainPackCompatibilityStatus::Degraded);
    assert!(report
        .issues
        .iter()
        .any(|issue| { issue.code == DomainPackCompatibilityIssueCode::MissingRequiredDomain }));
    assert!(report.requirement_impacts.iter().any(|impact| {
        impact.status == forge_core_contracts::DomainPackRequirementImpactStatus::NewlyMissing
    }));
}

#[test]
fn trust_regression_and_persistent_requirement_change_block() {
    let old = lock(vec![locked_package(
        DomainPackSourceAssurance::SupplyChainVerified,
    )]);
    let mut new = lock(vec![locked_package(
        DomainPackSourceAssurance::LocalExplicit,
    )]);
    new.domain_pack_exact_lock.payload.requirements_digest = D.to_owned();
    resign(&mut new);
    let report = evaluate_domain_pack_compatibility(&input(operation("upgrade"), Some(old), new))
        .domain_pack_compatibility_report;
    assert_eq!(report.status, DomainPackCompatibilityStatus::Blocked);
    assert!(report
        .changes
        .iter()
        .any(|change| change.kind == DomainPackSemanticChangeKind::TrustChanged));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == DomainPackCompatibilityIssueCode::TrustDegraded));
    assert!(report.issues.iter().any(|issue| {
        issue.code == DomainPackCompatibilityIssueCode::RequirementsChangedWithoutIntent
    }));
}

fn canonical_digest<T: serde::Serialize>(value: &T) -> String {
    let bytes = serde_json_canonicalizer::to_vec(value).expect("canonical JSON");
    format!("sha256:{:x}", Sha256::digest(bytes))
}
