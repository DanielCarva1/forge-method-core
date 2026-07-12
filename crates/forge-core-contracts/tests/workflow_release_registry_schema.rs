use forge_core_contracts::{
    ProjectImportedEvent, ReleaseUpgradedEvent, StableId, WorkflowGovernanceEvent,
    WorkflowGovernanceReleaseIdentity, WorkflowGovernanceReleaseRegistryDocument,
    WorkflowReceiptCarryover, WorkflowReleaseAdmissionProof, WorkflowReleaseRegistryAuthority,
    WorkflowReleaseRegistryProvenance, WorkflowReleaseRegistrySource,
    WorkflowRuntimeBundleIdentity,
};

fn registry_text() -> &'static str {
    include_str!("../../../contracts/migration/workflow-governance-release-registry-v0.yaml")
}

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn release(value: &str, digest_byte: char) -> WorkflowGovernanceReleaseIdentity {
    WorkflowGovernanceReleaseIdentity {
        lineage_id: id("workflow-governance.core"),
        release_id: id(value),
        release_version: "0.1.0".to_owned(),
        release_digest: format!("sha256:{}", digest_byte.to_string().repeat(64)),
    }
}

fn bundle(value: &str, digest_byte: char) -> WorkflowRuntimeBundleIdentity {
    WorkflowRuntimeBundleIdentity {
        bundle_id: id(value),
        bundle_digest: format!("sha256:{}", digest_byte.to_string().repeat(64)),
        policy_set_digest: format!("sha256:{}", "c".repeat(64)),
    }
}

#[test]
fn canonical_registry_is_closed_candidate_only_foundation() {
    let document: WorkflowGovernanceReleaseRegistryDocument =
        yaml_serde::from_str(registry_text()).expect("canonical registry");
    let registry = document.workflow_governance_release_registry;
    assert_eq!(registry.releases.len(), 2);
    assert!(registry
        .releases
        .iter()
        .all(|entry| entry.authority == WorkflowReleaseRegistryAuthority::CandidateOnly));
    assert_eq!(
        registry
            .releases
            .iter()
            .filter(|entry| matches!(
                entry.source,
                WorkflowReleaseRegistrySource::ImplicitP5cGenesis
            ))
            .count(),
        1
    );
    assert_ne!(
        registry.releases[0].release.release_digest,
        registry.releases[0].runtime_bundle.identity.bundle_digest
    );
}

#[test]
fn unknown_fields_and_authored_admission_fail_to_deserialize() {
    let unknown = registry_text().replacen(
        "  registry_id:",
        "  invented_authority: true\n  registry_id:",
        1,
    );
    assert!(yaml_serde::from_str::<WorkflowGovernanceReleaseRegistryDocument>(&unknown).is_err());

    let elevated = registry_text().replacen("authority: candidate_only", "authority: admitted", 1);
    assert!(yaml_serde::from_str::<WorkflowGovernanceReleaseRegistryDocument>(&elevated).is_err());
}

#[test]
fn release_upgraded_event_binds_release_bundle_registry_proof_and_prior_head() {
    let event = WorkflowGovernanceEvent::ReleaseUpgraded(ReleaseUpgradedEvent {
        from_release: release("release.from", 'a'),
        to_release: release("release.to", 'b'),
        from_runtime_bundle: bundle("bundle.from", '1'),
        to_runtime_bundle: bundle("bundle.to", '2'),
        registry_provenance: WorkflowReleaseRegistryProvenance {
            registry_id: id("registry.v0"),
            registry_version: "0.1.0".to_owned(),
            registry_digest: format!("sha256:{}", "d".repeat(64)),
        },
        admission_proof: WorkflowReleaseAdmissionProof {
            proof_id: id("proof.upgrade.v0"),
            proof_digest: format!("sha256:{}", "e".repeat(64)),
            snapshot_digest: format!("sha256:{}", "f".repeat(64)),
            from_policy_set_digest: format!("sha256:{}", "c".repeat(64)),
            to_policy_set_digest: format!("sha256:{}", "c".repeat(64)),
        },
        receipt_carryover: WorkflowReceiptCarryover::PreservePolicyEquivalent,
        prior_ledger_head_digest: format!("sha256:{}", "9".repeat(64)),
    });
    let value = serde_json::to_value(&event).expect("serialize event");
    assert_eq!(value["type"], "release_upgraded");
    assert_eq!(
        value["payload"]["admission_proof"]["from_policy_set_digest"],
        value["payload"]["admission_proof"]["to_policy_set_digest"]
    );
    assert_eq!(
        serde_json::from_value::<WorkflowGovernanceEvent>(value.clone()).expect("round trip"),
        event
    );
}

#[test]
fn adding_event_variant_does_not_change_project_imported_bytes() {
    let event = WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
        source_ref: "legacy".to_owned(),
        source_digest: "sha256:source".to_owned(),
        snapshot_digest: "sha256:snapshot".to_owned(),
        initial_phase: id("explore"),
    });
    let bytes = serde_json::to_string(&event).expect("serialize legacy event");
    assert_eq!(
        bytes,
        r#"{"type":"project_imported","payload":{"source_ref":"legacy","source_digest":"sha256:source","snapshot_digest":"sha256:snapshot","initial_phase":"explore"}}"#
    );
}
