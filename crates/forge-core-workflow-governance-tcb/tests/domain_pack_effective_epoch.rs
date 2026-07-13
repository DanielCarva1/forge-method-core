use forge_core_contracts::{
    DomainPackGenerationTransitionedEvent, ProjectImportedEvent, StableId,
    WorkflowDomainPackGenerationIdentity, WorkflowEffectiveBundleIdentity, WorkflowGovernanceEvent,
    WorkflowReceiptCarryover, WorkflowRuntimeBundleIdentity,
    WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION, WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION,
};
use forge_core_workflow_governance_tcb::{
    append_workflow_governance_event_tcb, domain_pack_receipt_carryover,
    initialize_workflow_governance_ledger_tcb, lock_workflow_governance_ledger_tcb,
    recover_workflow_governance_ledger, transition_workflow_domain_pack_generation_tcb,
    transition_workflow_governance_release_tcb, WorkflowGovernanceLedgerError,
    WorkflowGovernanceLedgerIdentity,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn bare_digest(byte: char) -> String {
    byte.to_string().repeat(64)
}

fn root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "forge-workflow-domain-epoch-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("temp state root");
    path
}

fn ledger_identity() -> WorkflowGovernanceLedgerIdentity {
    WorkflowGovernanceLedgerIdentity {
        project_id: StableId("project.domain-epoch".to_owned()),
        bundle_id: StableId("bundle.core".to_owned()),
        bundle_digest: digest('1'),
    }
}

fn runtime(id: &str, digest_byte: char, policy_byte: char) -> WorkflowRuntimeBundleIdentity {
    WorkflowRuntimeBundleIdentity {
        bundle_id: StableId(id.to_owned()),
        bundle_digest: digest(digest_byte),
        policy_set_digest: digest(policy_byte),
    }
}

fn core_only() -> WorkflowEffectiveBundleIdentity {
    let core = runtime("bundle.core", '1', '2');
    WorkflowEffectiveBundleIdentity {
        core_runtime_bundle: core.clone(),
        effective_runtime_bundle: core,
        domain_pack_generation: None,
        receipt_context_digest: digest('3'),
    }
}

fn generation(number: u64) -> WorkflowEffectiveBundleIdentity {
    WorkflowEffectiveBundleIdentity {
        core_runtime_bundle: runtime("bundle.core", '1', '2'),
        effective_runtime_bundle: runtime("bundle.effective", '4', '5'),
        domain_pack_generation: Some(WorkflowDomainPackGenerationIdentity {
            generation: number,
            active_lock_digest: digest('6'),
            composition_digest: digest('7'),
            base_core_bundle_digest: digest('8'),
            supply_chain_registry_digest: digest('9'),
            reviewer_registry_digest: bare_digest('a'),
            reviewed_registry_digest: bare_digest('b'),
        }),
        receipt_context_digest: digest('c'),
    }
}

fn removed_core_only_generation(number: u64) -> WorkflowEffectiveBundleIdentity {
    let mut value = generation(number);
    value.effective_runtime_bundle = value.core_runtime_bundle.clone();
    value.receipt_context_digest = digest('f');
    value
}

fn imported() -> WorkflowGovernanceEvent {
    WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
        source_ref: "project".to_owned(),
        source_digest: digest('d'),
        snapshot_digest: digest('e'),
        initial_phase: StableId("1-discovery".to_owned()),
    })
}

fn transition(
    from: WorkflowEffectiveBundleIdentity,
    to: WorkflowEffectiveBundleIdentity,
    head: &str,
) -> DomainPackGenerationTransitionedEvent {
    DomainPackGenerationTransitionedEvent {
        receipt_carryover: domain_pack_receipt_carryover(&from, &to),
        from_effective_bundle: from,
        to_effective_bundle: to,
        prior_ledger_head_digest: head.to_owned(),
    }
}

#[test]
fn historical_v1_replays_and_dedicated_transition_starts_v2_epoch() {
    let root = root("v1-v2");
    let identity = ledger_identity();
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity, 0, imported())
        .expect("v1 genesis");
    let first_line =
        fs::read_to_string(root.join("wal/workflow-governance.ndjson")).expect("read v1 ledger");
    assert!(first_line.contains(&format!(
        "\"schema_version\":\"{WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION}\""
    )));
    assert_eq!(
        recover_workflow_governance_ledger(&root)
            .unwrap()
            .records
            .len(),
        1
    );

    let event = transition(core_only(), generation(1), &first.record_digest);
    let record = transition_workflow_domain_pack_generation_tcb(
        &root,
        &first.record_digest,
        &identity,
        1,
        event,
    )
    .expect("dedicated transition");
    let projection = recover_workflow_governance_ledger(&root).expect("mixed replay");
    assert_eq!(
        projection.head_digest.as_deref(),
        Some(record.record_digest.as_str())
    );
    assert_eq!(
        projection
            .active_effective_bundle_identity()
            .and_then(|value| value.domain_pack_generation)
            .map(|value| value.generation),
        Some(1)
    );
    let ledger =
        fs::read_to_string(root.join("wal/workflow-governance.ndjson")).expect("read mixed ledger");
    assert!(ledger
        .lines()
        .nth(1)
        .expect("second line")
        .contains(&format!(
            "\"schema_version\":\"{WORKFLOW_GOVERNANCE_EFFECTIVE_LEDGER_SCHEMA_VERSION}\""
        )));

    // Removal still creates a new active generation even when its effective
    // policies are core-only. It must advance the epoch and invalidate the
    // receipts derived under the removed pack.
    let from = projection
        .active_effective_bundle_identity()
        .expect("active first generation");
    let to = removed_core_only_generation(2);
    let removal = transition(from, to, &record.record_digest);
    assert_eq!(
        removal.receipt_carryover,
        WorkflowReceiptCarryover::InvalidateAll
    );
    let removed = transition_workflow_domain_pack_generation_tcb(
        &root,
        &record.record_digest,
        &identity,
        2,
        removal,
    )
    .expect("core-only removal generation");
    assert_eq!(
        recover_workflow_governance_ledger(&root)
            .unwrap()
            .active_effective_bundle_identity()
            .unwrap()
            .domain_pack_generation
            .unwrap()
            .generation,
        2
    );
    assert!(!removed.record_digest.is_empty());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn core_release_upgrade_is_blocked_while_any_domain_generation_is_active() {
    use forge_core_contracts::{
        ReleaseUpgradedEvent, WorkflowGovernanceReleaseIdentity, WorkflowReleaseAdmissionProof,
        WorkflowReleaseRegistryProvenance,
    };
    let root = root("rebase");
    let identity = ledger_identity();
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity, 0, imported())
        .expect("genesis");
    let epoch = transition_workflow_domain_pack_generation_tcb(
        &root,
        &first.record_digest,
        &identity,
        1,
        transition(core_only(), generation(1), &first.record_digest),
    )
    .expect("domain epoch");
    let target_runtime = runtime("bundle.next", 'd', 'e');
    let target = WorkflowGovernanceLedgerIdentity {
        project_id: identity.project_id.clone(),
        bundle_id: target_runtime.bundle_id.clone(),
        bundle_digest: target_runtime.bundle_digest.clone(),
    };
    let event = ReleaseUpgradedEvent {
        from_release: WorkflowGovernanceReleaseIdentity {
            lineage_id: StableId("lineage.core".to_owned()),
            release_id: StableId("release.current".to_owned()),
            release_version: "1.0.0".to_owned(),
            release_digest: digest('4'),
        },
        to_release: WorkflowGovernanceReleaseIdentity {
            lineage_id: StableId("lineage.core".to_owned()),
            release_id: StableId("release.next".to_owned()),
            release_version: "2.0.0".to_owned(),
            release_digest: digest('5'),
        },
        from_runtime_bundle: runtime("bundle.core", '1', '2'),
        to_runtime_bundle: target_runtime,
        registry_provenance: WorkflowReleaseRegistryProvenance {
            registry_id: StableId("registry.core".to_owned()),
            registry_version: "2.0.0".to_owned(),
            registry_digest: digest('6'),
        },
        admission_proof: WorkflowReleaseAdmissionProof {
            proof_id: StableId("proof.next".to_owned()),
            proof_digest: digest('7'),
            snapshot_digest: digest('8'),
            from_policy_set_digest: digest('2'),
            to_policy_set_digest: digest('e'),
        },
        receipt_carryover: WorkflowReceiptCarryover::InvalidateAll,
        prior_ledger_head_digest: epoch.record_digest.clone(),
    };
    let error = transition_workflow_governance_release_tcb(
        &root,
        &epoch.record_digest,
        &identity,
        &target,
        2,
        event,
    )
    .expect_err("active generation requires explicit rebase");
    assert!(matches!(
        error,
        WorkflowGovernanceLedgerError::ReleaseTransitionInvalid { .. }
    ));
    let unchanged = recover_workflow_governance_ledger(&root).expect("rejected upgrade is atomic");
    assert_eq!(unchanged.records.len(), 2);
    assert_eq!(
        unchanged.head_digest.as_deref(),
        Some(epoch.record_digest.as_str())
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn generic_injection_and_generation_regression_are_blocked() {
    let root = root("blocked");
    let identity = ledger_identity();
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity, 0, imported())
        .expect("genesis");
    let first_event = transition(core_only(), generation(1), &first.record_digest);
    let injection = append_workflow_governance_event_tcb(
        &root,
        &first.record_digest,
        &identity,
        1,
        WorkflowGovernanceEvent::DomainPackGenerationTransitioned(first_event.clone()),
    )
    .expect_err("generic transition injection");
    assert!(matches!(
        injection,
        WorkflowGovernanceLedgerError::DomainPackTransitionRequiresDedicatedAuthority
    ));

    let first_epoch = transition_workflow_domain_pack_generation_tcb(
        &root,
        &first.record_digest,
        &identity,
        1,
        first_event,
    )
    .expect("first epoch");
    let active = generation(1);
    let regression = transition(active.clone(), active, &first_epoch.record_digest);
    let error = transition_workflow_domain_pack_generation_tcb(
        &root,
        &first_epoch.record_digest,
        &identity,
        2,
        regression,
    )
    .expect_err("same generation is a fork/regression");
    assert!(matches!(
        error,
        WorkflowGovernanceLedgerError::DomainPackTransitionInvalid { .. }
    ));
    assert_eq!(
        recover_workflow_governance_ledger(&root)
            .expect("unchanged ledger")
            .records
            .len(),
        2
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn caller_selected_receipt_preservation_is_rejected_on_context_drift() {
    let root = root("carryover");
    let identity = ledger_identity();
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity, 0, imported())
        .expect("genesis");
    let mut event = transition(core_only(), generation(1), &first.record_digest);
    assert_eq!(
        event.receipt_carryover,
        WorkflowReceiptCarryover::InvalidateAll
    );
    event.receipt_carryover = WorkflowReceiptCarryover::PreservePolicyEquivalent;
    let error = transition_workflow_domain_pack_generation_tcb(
        &root,
        &first.record_digest,
        &identity,
        1,
        event,
    )
    .expect_err("caller cannot preserve drifted receipts");
    assert!(matches!(
        error,
        WorkflowGovernanceLedgerError::DomainPackTransitionInvalid { .. }
    ));

    // Holding the lower-level lock exposes the same pure authority boundary
    // needed by the future cross-store Adapter reconciliation.
    let locked = lock_workflow_governance_ledger_tcb(&root).expect("lock remains usable");
    assert_eq!(locked.recover().unwrap().records.len(), 1);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn reviewer_registry_digest_domains_accept_only_bare_lowercase_sha256_hex() {
    let accepted_root = root("bare-review-digests");
    let identity = ledger_identity();
    let first = initialize_workflow_governance_ledger_tcb(&accepted_root, &identity, 0, imported())
        .expect("genesis");
    transition_workflow_domain_pack_generation_tcb(
        &accepted_root,
        &first.record_digest,
        &identity,
        1,
        transition(core_only(), generation(1), &first.record_digest),
    )
    .expect("P6c reviewer and reviewed registry digests are bare lowercase hex");
    fs::remove_dir_all(accepted_root).expect("cleanup accepted root");

    let invalid = [
        ("prefixed", digest('a'), bare_digest('b')),
        ("uppercase", "A".repeat(64), bare_digest('b')),
        ("short", bare_digest('a'), "b".repeat(63)),
    ];
    for (label, reviewer_digest, reviewed_digest) in invalid {
        let state_root = root(label);
        let first =
            initialize_workflow_governance_ledger_tcb(&state_root, &identity, 0, imported())
                .expect("genesis");
        let mut target = generation(1);
        let generation = target
            .domain_pack_generation
            .as_mut()
            .expect("target generation");
        generation.reviewer_registry_digest = reviewer_digest;
        generation.reviewed_registry_digest = reviewed_digest;
        let error = transition_workflow_domain_pack_generation_tcb(
            &state_root,
            &first.record_digest,
            &identity,
            1,
            transition(core_only(), target, &first.record_digest),
        )
        .expect_err("wrong reviewer digest domain must fail closed");
        assert!(matches!(
            error,
            WorkflowGovernanceLedgerError::DomainPackTransitionInvalid { .. }
        ));
        fs::remove_dir_all(state_root).expect("cleanup invalid root");
    }
}
