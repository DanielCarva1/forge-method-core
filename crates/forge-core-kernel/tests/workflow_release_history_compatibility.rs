use forge_core_contracts::{
    ReleaseUpgradedEvent, StableId, WorkflowGovernanceEvent, WorkflowGovernanceReceiptDocument,
    WorkflowGovernanceReleaseIdentity, WorkflowReceiptCarryover, WorkflowReleaseAdmissionProof,
    WorkflowRuntimeBundleIdentity,
};
use forge_core_kernel::{
    WorkflowGovernanceAdapterError, WorkflowGovernanceProjectAdapter,
    WorkflowGovernanceReleasePinOrigin,
};
use forge_core_store::sha256_content_hash;
use forge_core_workflow_governance_tcb::{
    recover_workflow_governance_ledger, transition_workflow_governance_release_tcb,
    workflow_governance_record_digest, WorkflowGovernanceLedgerError,
    WorkflowGovernanceLedgerIdentity, WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const FROZEN_HISTORY: &[u8] = include_bytes!("fixtures/p5d2-foundation-history.ndjson");
const FROZEN_HISTORY_DIGEST: &str =
    "sha256:f629f77e8718d258f6ac0bfad911d88824af0f74188a2a1060fa87bad78ed761";
const GENESIS_RECORD_DIGEST: &str =
    "sha256:07d5cea9c8169c97ca3b4e310dcf40f034675cd21e1b630f01710801bdf1955d";
const FOUNDATION_RECORD_DIGEST: &str =
    "sha256:0d0d4fd5379d7e76532302f16aef98dce1c0db4b3cfd70fd34c2ccde73a51d45";
const FOUNDATION_RELEASE_ID: &str = "workflow-governance.release.foundation-v0";
const FOUNDATION_BUNDLE_ID: &str = "bundle.workflow-governance.release-foundation-v0";
const FUTURE_CANDIDATE_RELEASE_ID: &str = "workflow-governance.release.reviewed-batch-v0";

fn unique_root(label: &str) -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "forge-p5d3-history-{label}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::SeqCst)
    ))
}

fn install_wal(root: &Path, wal: &[u8]) -> PathBuf {
    let state = root.join(".forge-method");
    fs::create_dir_all(state.join("wal")).expect("test WAL directory");
    fs::write(state.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH), wal).expect("test WAL");
    state
}

fn wal_documents(wal: &[u8]) -> Vec<WorkflowGovernanceReceiptDocument> {
    wal.split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).expect("typed WAL record"))
        .collect()
}

fn write_documents(path: &Path, documents: &[WorkflowGovernanceReceiptDocument]) {
    let mut wal = Vec::new();
    for document in documents {
        wal.extend(serde_json::to_vec(document).expect("record JSON"));
        wal.push(b'\n');
    }
    fs::write(path, wal).expect("rewritten WAL");
}

fn recompute_admission_proof(event: &mut ReleaseUpgradedEvent) {
    let canonical = serde_json_canonicalizer::to_vec(&(
        &event.admission_proof.proof_id,
        &event.registry_provenance,
        &event.from_release,
        &event.from_runtime_bundle,
        &event.to_release,
        &event.to_runtime_bundle,
        event.receipt_carryover,
        &event.admission_proof.snapshot_digest,
    ))
    .expect("canonical admission proof");
    event.admission_proof.proof_digest = sha256_content_hash(&canonical);
}

fn recompute_record(document: &mut WorkflowGovernanceReceiptDocument) {
    document.workflow_governance_receipt.record_digest =
        workflow_governance_record_digest(&document.workflow_governance_receipt)
            .expect("record digest");
}

struct UpgradedProject {
    root: PathBuf,
    state: PathBuf,
    adapter: WorkflowGovernanceProjectAdapter,
}

impl UpgradedProject {
    fn new(label: &str) -> Self {
        let root = unique_root(label);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".forge-method")).expect("state root");
        fs::write(root.join("README.md"), b"P5d.2 history compatibility\n").expect("project basis");
        let root = root.canonicalize().expect("canonical project root");
        let state = root.join(".forge-method");
        let adapter = WorkflowGovernanceProjectAdapter::new(
            StableId(format!("project.p5d3.history.{label}")),
            &root,
            &state,
        )
        .expect("adapter");
        adapter.initialize().expect("P5c genesis");
        let status = adapter.release_status().expect("genesis status");
        let target = status
            .available_successor
            .as_ref()
            .expect("foundation successor")
            .release_id
            .clone();
        adapter
            .release_upgrade(
                &target,
                &status.active.release.release_digest,
                &status.ledger_head_digest,
                &status.snapshot_digest,
            )
            .expect("P5d.2 foundation upgrade");
        Self {
            root,
            state,
            adapter,
        }
    }

    fn wal_path(&self) -> PathBuf {
        self.state.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)
    }
}

impl Drop for UpgradedProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn frozen_p5d2_foundation_history_keeps_exact_bytes_and_record_identities() {
    assert_eq!(sha256_content_hash(FROZEN_HISTORY), FROZEN_HISTORY_DIGEST);
    let root = unique_root("frozen");
    let state = install_wal(&root, FROZEN_HISTORY);
    let projection = recover_workflow_governance_ledger(&state).expect("frozen P5d.2 WAL");

    assert_eq!(projection.records.len(), 2);
    assert_eq!(projection.records[0].record_digest, GENESIS_RECORD_DIGEST);
    assert_eq!(
        projection.records[1].record_digest,
        FOUNDATION_RECORD_DIGEST
    );
    assert_eq!(
        projection.head_digest.as_deref(),
        Some(FOUNDATION_RECORD_DIGEST)
    );
    assert_eq!(
        projection
            .genesis_identity()
            .expect("genesis identity")
            .bundle_id
            .0,
        "bundle.workflow-governance.golden-path-v0"
    );
    assert_eq!(
        projection
            .active_identity()
            .expect("active identity")
            .bundle_id
            .0,
        FOUNDATION_BUNDLE_ID
    );
    let WorkflowGovernanceEvent::ReleaseUpgraded(upgrade) = &projection.records[1].event else {
        panic!("second record must be the P5d.2 release upgrade");
    };
    assert_eq!(upgrade.to_release.release_id.0, FOUNDATION_RELEASE_ID);
    assert_eq!(
        upgrade.prior_ledger_head_digest,
        projection.records[0].record_digest
    );
    assert_eq!(
        upgrade.admission_proof.snapshot_digest,
        match &projection.records[0].event {
            WorkflowGovernanceEvent::ProjectImported(imported) => imported.snapshot_digest.clone(),
            _ => panic!("genesis must be project_imported"),
        }
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn frozen_history_and_provenance_byte_mutants_fail_closed() {
    for (label, from, to) in [
        (
            "history",
            b"\"state_version\":1".as_slice(),
            b"\"state_version\":2".as_slice(),
        ),
        (
            "provenance",
            b"\"registry_version\":\"0.1.0\"".as_slice(),
            b"\"registry_version\":\"0.1.1\"".as_slice(),
        ),
    ] {
        let root = unique_root(label);
        let mut mutant = FROZEN_HISTORY.to_vec();
        let offset = mutant
            .windows(from.len())
            .position(|window| window == from)
            .expect("mutation subject");
        mutant.splice(offset..offset + from.len(), to.iter().copied());
        let state = install_wal(&root, &mutant);
        assert!(matches!(
            recover_workflow_governance_ledger(&state),
            Err(WorkflowGovernanceLedgerError::RecordDigestMismatch { line: 2, .. })
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn historical_registry_provenance_authenticates_only_with_its_exact_proof() {
    let project = UpgradedProject::new("historical-provenance");
    let mut documents = wal_documents(&fs::read(project.wal_path()).expect("WAL"));
    let record = &mut documents[1].workflow_governance_receipt;
    let WorkflowGovernanceEvent::ReleaseUpgraded(upgrade) = &mut record.event else {
        panic!("release upgrade");
    };
    upgrade.registry_provenance.registry_version = "0.0.9".to_owned();
    upgrade.registry_provenance.registry_digest = format!("sha256:{}", "7".repeat(64));
    recompute_admission_proof(upgrade);
    recompute_record(&mut documents[1]);
    write_documents(&project.wal_path(), &documents);

    let status = project
        .adapter
        .release_status()
        .expect("authenticated historical provenance");
    assert_eq!(status.active.release.release_id.0, FOUNDATION_RELEASE_ID);
    assert_eq!(status.active.registry.registry_version, "0.0.9");
    assert_eq!(
        status.active.pin_origin,
        WorkflowGovernanceReleasePinOrigin::LedgerTransition
    );

    let WorkflowGovernanceEvent::ReleaseUpgraded(upgrade) =
        &mut documents[1].workflow_governance_receipt.event
    else {
        panic!("release upgrade");
    };
    upgrade.registry_provenance.registry_digest = format!("sha256:{}", "6".repeat(64));
    // Re-hash the record but deliberately retain the proof bound to the prior
    // provenance. Structural WAL recovery succeeds; kernel admission must not.
    recompute_record(&mut documents[1]);
    write_documents(&project.wal_path(), &documents);
    assert!(recover_workflow_governance_ledger(&project.state).is_ok());
    assert!(matches!(
        project.adapter.release_status(),
        Err(WorkflowGovernanceAdapterError::ReleaseChainInvalid)
    ));
}

#[test]
fn foundation_pin_governs_next_resume_readiness_and_completion_paths() {
    let project = UpgradedProject::new("foundation-paths");
    let status = project.adapter.release_status().expect("foundation status");
    assert_eq!(status.active.release.release_id.0, FOUNDATION_RELEASE_ID);
    assert_eq!(
        status.active.runtime_bundle.bundle_id.0,
        FOUNDATION_BUNDLE_ID
    );
    assert!(status.available_successor.is_none());
    assert!(status.upgrade_argv.is_none());

    let next = project.adapter.next().expect("foundation next");
    let resumed = project.adapter.resume().expect("foundation resume");
    assert_eq!(
        serde_json::to_value(&next).expect("next JSON"),
        serde_json::to_value(&resumed).expect("resume JSON")
    );
    assert_eq!(next.release.release.release_id.0, FOUNDATION_RELEASE_ID);
    assert_eq!(next.bundle_id.0, FOUNDATION_BUNDLE_ID);
    assert!(next
        .boundary_rechecks
        .iter()
        .all(|recheck| recheck.simulation.bundle_id == FOUNDATION_BUNDLE_ID));

    assert!(matches!(
        project.adapter.prepare_completion(),
        Err(WorkflowGovernanceAdapterError::PolicyIncomplete)
    ));
    let after_completion_probe = project.adapter.release_status().expect("pin retained");
    assert_eq!(
        after_completion_probe.active.release.release_id.0,
        FOUNDATION_RELEASE_ID
    );
}

#[test]
fn old_registry_exposes_no_candidate_and_rejects_a_structurally_valid_future_pin() {
    let project = UpgradedProject::new("future-pin");
    let status = project.adapter.release_status().expect("foundation status");
    assert!(status.available_successor.is_none());
    assert!(matches!(
        project.adapter.release_upgrade(
            &StableId(FUTURE_CANDIDATE_RELEASE_ID.to_owned()),
            &status.active.release.release_digest,
            &status.ledger_head_digest,
            &status.snapshot_digest,
        ),
        Err(WorkflowGovernanceAdapterError::UnknownRelease(id))
            if id == FUTURE_CANDIDATE_RELEASE_ID
    ));

    let projection = recover_workflow_governance_ledger(&project.state).expect("foundation WAL");
    let WorkflowGovernanceEvent::ReleaseUpgraded(foundation) =
        &projection.records.last().expect("upgrade record").event
    else {
        panic!("foundation upgrade");
    };
    let source_identity = projection.active_identity().expect("foundation identity");
    let candidate_runtime = WorkflowRuntimeBundleIdentity {
        bundle_id: StableId("bundle.workflow-governance.reviewed-batch-v0".to_owned()),
        bundle_digest: format!("sha256:{}", "8".repeat(64)),
        policy_set_digest: format!("sha256:{}", "9".repeat(64)),
    };
    let target_identity = WorkflowGovernanceLedgerIdentity {
        project_id: source_identity.project_id.clone(),
        bundle_id: candidate_runtime.bundle_id.clone(),
        bundle_digest: candidate_runtime.bundle_digest.clone(),
    };
    let event = ReleaseUpgradedEvent {
        from_release: foundation.to_release.clone(),
        to_release: WorkflowGovernanceReleaseIdentity {
            lineage_id: foundation.to_release.lineage_id.clone(),
            release_id: StableId(FUTURE_CANDIDATE_RELEASE_ID.to_owned()),
            release_version: "0.2.0-candidate".to_owned(),
            release_digest: format!("sha256:{}", "a".repeat(64)),
        },
        from_runtime_bundle: foundation.to_runtime_bundle.clone(),
        to_runtime_bundle: candidate_runtime,
        registry_provenance: foundation.registry_provenance.clone(),
        admission_proof: WorkflowReleaseAdmissionProof {
            proof_id: StableId("proof.workflow-governance.unadmitted-future".to_owned()),
            proof_digest: format!("sha256:{}", "b".repeat(64)),
            snapshot_digest: status.snapshot_digest,
            from_policy_set_digest: foundation.to_runtime_bundle.policy_set_digest.clone(),
            to_policy_set_digest: format!("sha256:{}", "9".repeat(64)),
        },
        receipt_carryover: WorkflowReceiptCarryover::InvalidateAll,
        prior_ledger_head_digest: projection.head_digest.clone().expect("head"),
    };
    transition_workflow_governance_release_tcb(
        &project.state,
        projection.head_digest.as_deref().expect("head"),
        &source_identity,
        &target_identity,
        projection.next_state_version,
        event,
    )
    .expect("structurally valid but non-admitted transition");

    assert!(matches!(
        project.adapter.release_status(),
        Err(WorkflowGovernanceAdapterError::ReleaseChainInvalid)
    ));
    assert!(matches!(
        project.adapter.next(),
        Err(WorkflowGovernanceAdapterError::ReleaseChainInvalid)
    ));
}
