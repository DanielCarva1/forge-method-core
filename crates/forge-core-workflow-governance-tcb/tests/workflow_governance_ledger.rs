use forge_core_contracts::{
    PhaseAdvancedEvent, ProjectImportedEvent, StableId, WorkflowGovernanceEvent,
    WorkflowGovernanceLedgerRecord, WorkflowGovernanceReceiptDocument,
    WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION,
};
use forge_core_workflow_governance_tcb::{
    append_workflow_governance_event_tcb, initialize_workflow_governance_ledger_tcb,
    lock_workflow_governance_ledger_tcb, recover_workflow_governance_ledger,
    workflow_governance_record_digest, WorkflowGovernanceLedgerError,
    WorkflowGovernanceLedgerIdentity, WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES,
    WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::time::{SystemTime, UNIX_EPOCH};

fn id(value: &str) -> StableId {
    StableId(value.to_owned())
}

fn identity() -> WorkflowGovernanceLedgerIdentity {
    WorkflowGovernanceLedgerIdentity {
        project_id: id("project-alpha"),
        bundle_id: id("bundle-core"),
        bundle_digest: "sha256:bundle".to_owned(),
    }
}

fn imported() -> WorkflowGovernanceEvent {
    WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
        source_ref: "project/state.yaml".to_owned(),
        source_digest: "sha256:source".to_owned(),
        snapshot_digest: "sha256:snapshot-0".to_owned(),
        initial_phase: id("discover"),
    })
}

fn advanced(index: usize) -> WorkflowGovernanceEvent {
    WorkflowGovernanceEvent::PhaseAdvanced(PhaseAdvancedEvent {
        from_phase: Some(id("discover")),
        to_phase: id("define"),
        snapshot_digest: format!("sha256:snapshot-{index}"),
    })
}

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-governance-ledger-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create test root");
    root
}

fn wal_path(root: &Path) -> PathBuf {
    root.join(WORKFLOW_GOVERNANCE_WAL_RELATIVE_PATH)
}

fn read_documents(root: &Path) -> Vec<WorkflowGovernanceReceiptDocument> {
    fs::read_to_string(wal_path(root))
        .expect("read ledger")
        .lines()
        .map(|line| serde_json::from_str(line).expect("parse receipt"))
        .collect()
}

fn write_documents(root: &Path, documents: &[WorkflowGovernanceReceiptDocument]) {
    let mut bytes = Vec::new();
    for document in documents {
        bytes.extend(serde_json::to_vec(document).expect("encode receipt"));
        bytes.push(b'\n');
    }
    fs::write(wal_path(root), bytes).expect("rewrite ledger");
}

#[test]
fn published_all_events_fixture_has_a_valid_canonical_hash_chain() {
    let raw = include_str!(
        "../../../docs/fixtures/workflow-governance-golden-path-v0/ledger-all-events.yaml"
    );
    let document: forge_core_contracts::WorkflowGovernanceLedgerDocument =
        yaml_serde::from_str(raw).expect("published ledger fixture");
    let mut expected_previous: Option<String> = None;
    for record in &document.workflow_governance_ledger.records {
        assert_eq!(record.previous_record_digest, expected_previous);
        assert_eq!(
            workflow_governance_record_digest(record).expect("canonical record digest"),
            record.record_digest
        );
        expected_previous = Some(record.record_digest.clone());
    }
}

#[test]
fn initialize_append_and_recover_ordered_chain() {
    let root = temp_root("init-recover");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    assert_eq!(first.sequence, 1);
    assert_eq!(first.previous_record_digest, None);
    assert!(!first.record_id.0.is_empty());
    assert!(!first.record_digest.is_empty());

    let second = append_workflow_governance_event_tcb(
        &root,
        &first.record_digest,
        &identity(),
        1,
        advanced(1),
    )
    .expect("append");
    let projection = recover_workflow_governance_ledger(&root).expect("recover");
    assert_eq!(projection.records, vec![first.clone(), second.clone()]);
    assert_eq!(
        projection.head_digest.as_deref(),
        Some(second.record_digest.as_str())
    );
    assert_eq!(projection.next_sequence, 3);
    assert_eq!(projection.next_state_version, 2);
    assert!(root.join("locks/workflow-governance.lock").is_file());
    fs::remove_dir_all(root).expect("cleanup");

    let blocked = temp_root("init-atomic-failure");
    fs::create_dir_all(wal_path(&blocked)).expect("block WAL target with directory");
    assert!(
        initialize_workflow_governance_ledger_tcb(&blocked, &identity(), 0, imported()).is_err()
    );
    assert!(
        wal_path(&blocked).is_dir(),
        "failed initialization must not replace the target"
    );
    let wal_parent = wal_path(&blocked)
        .parent()
        .expect("WAL parent")
        .to_path_buf();
    assert!(
        fs::read_dir(wal_parent)
            .expect("read WAL parent")
            .all(|entry| {
                let name = entry.expect("directory entry").file_name();
                let name = name.to_string_lossy();
                !name.contains(".forge-tmp") && !name.contains(".forge-bak")
            }),
        "failed initialization must not leave replacement artifacts"
    );
    fs::remove_dir_all(blocked).expect("cleanup blocked initialization");
}

#[test]
fn batch_commits_two_events_together_and_exposes_prepared_projection() {
    let root = temp_root("batch-two-events");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let original_wal = fs::read(wal_path(&root)).expect("read original WAL");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock ledger");

    let mut batch = ledger
        .begin_unchecked_tcb_batch(&first.record_digest, &identity())
        .expect("begin batch");
    let second = batch.push_event(1, advanced(1)).expect("prepare second");
    assert_eq!(batch.projection().records.len(), 2);
    assert_eq!(batch.projection().head_digest, Some(second.record_digest));
    let third = batch.push_event(2, advanced(2)).expect("prepare third");
    assert_eq!(batch.projection().records.len(), 3);
    assert_eq!(batch.projection().head_digest, Some(third.record_digest));
    assert_eq!(
        fs::read(wal_path(&root)).expect("read uncommitted WAL"),
        original_wal,
        "preparation must not change durable bytes"
    );

    let committed = batch.commit().expect("commit batch");
    assert_eq!(committed.len(), 2);
    let projection = ledger.recover().expect("recover committed batch");
    assert_eq!(projection.records.len(), 3);
    assert_eq!(&projection.records[1..], committed.as_slice());
    drop(ledger);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn batch_error_on_second_event_does_not_persist_first() {
    let root = temp_root("batch-second-error");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 2, imported())
        .expect("initialize");
    let original_wal = fs::read(wal_path(&root)).expect("read original WAL");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock ledger");
    {
        let mut batch = ledger
            .begin_unchecked_tcb_batch(&first.record_digest, &identity())
            .expect("begin batch");
        batch.push_event(3, advanced(1)).expect("prepare first");
        assert!(matches!(
            batch.push_event(1, advanced(2)),
            Err(WorkflowGovernanceLedgerError::StateVersionRegression {
                previous: 3,
                found: 1
            })
        ));
        assert_eq!(batch.projection().records.len(), 2);
    }
    assert_eq!(
        fs::read(wal_path(&root)).expect("read WAL after dropped batch"),
        original_wal
    );
    assert_eq!(ledger.recover().expect("recover").records.len(), 1);
    drop(ledger);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn batch_stale_head_fails_before_any_wal_mutation() {
    let root = temp_root("batch-stale-head");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let original_wal = fs::read(wal_path(&root)).expect("read original WAL");
    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock ledger");
    assert!(matches!(
        ledger.begin_unchecked_tcb_batch("sha256:stale", &identity()),
        Err(WorkflowGovernanceLedgerError::HeadMismatch { .. })
    ));
    assert_eq!(
        fs::read(wal_path(&root)).expect("read WAL after rejected batch"),
        original_wal
    );
    drop(ledger);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn batch_capacity_preflight_leaves_wal_unchanged() {
    let root = temp_root("batch-capacity-preflight");
    let mut near_capacity_import = imported();
    let WorkflowGovernanceEvent::ProjectImported(event) = &mut near_capacity_import else {
        unreachable!("imported helper must return project_imported")
    };
    event.source_ref = "x".repeat(
        usize::try_from(WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES).expect("capacity fits usize") - 2_048,
    );
    let first =
        initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, near_capacity_import)
            .expect("initialize near byte capacity");
    let original_wal = fs::read(wal_path(&root)).expect("read original WAL");
    assert!(
        WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES
            - u64::try_from(original_wal.len()).expect("WAL length fits u64")
            < 8_192,
        "fixture must leave less space than the prepared event requires"
    );
    let mut oversized_event = advanced(1);
    let WorkflowGovernanceEvent::PhaseAdvanced(event) = &mut oversized_event else {
        unreachable!("advanced helper must return phase_advanced")
    };
    event.snapshot_digest = "y".repeat(8_192);

    let mut ledger = lock_workflow_governance_ledger_tcb(&root).expect("lock ledger");
    let mut batch = ledger
        .begin_unchecked_tcb_batch(&first.record_digest, &identity())
        .expect("begin batch");
    assert!(matches!(
        batch.push_event(1, oversized_event),
        Err(WorkflowGovernanceLedgerError::CapacityBytes { .. })
    ));
    assert_eq!(batch.projection().records.len(), 1);
    assert!(matches!(
        batch.commit(),
        Err(WorkflowGovernanceLedgerError::EmptyBatch)
    ));
    assert_eq!(
        fs::read(wal_path(&root)).expect("read WAL after capacity rejection"),
        original_wal
    );
    drop(ledger);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn digest_is_deterministic_and_ignores_only_record_digest() {
    let root = temp_root("digest");
    let record = initialize_workflow_governance_ledger_tcb(&root, &identity(), 7, imported())
        .expect("initialize");
    let expected = workflow_governance_record_digest(&record).expect("digest");
    let mut with_other_stored_digest = record.clone();
    with_other_stored_digest.record_digest = "untrusted-cache".to_owned();
    assert_eq!(
        workflow_governance_record_digest(&with_other_stored_digest).expect("digest"),
        expected
    );
    with_other_stored_digest.state_version += 1;
    assert_ne!(
        workflow_governance_record_digest(&with_other_stored_digest).expect("digest"),
        expected
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recovery_rejects_payload_tamper() {
    let root = temp_root("tamper");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    let mut documents = read_documents(&root);
    if let WorkflowGovernanceEvent::ProjectImported(event) =
        &mut documents[0].workflow_governance_receipt.event
    {
        event.source_ref = "attacker.yaml".to_owned();
    }
    write_documents(&root, &documents);
    assert!(matches!(
        recover_workflow_governance_ledger(&root),
        Err(WorkflowGovernanceLedgerError::RecordDigestMismatch { line: 1, .. })
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recovery_rejects_torn_blank_malformed_and_unknown_schema_lines() {
    for (name, mutation, expected) in [
        ("torn", "torn", "torn"),
        ("blank", "blank", "blank"),
        ("malformed", "malformed", "malformed"),
        ("schema", "schema", "schema"),
    ] {
        let root = temp_root(name);
        initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
            .expect("initialize");
        let path = wal_path(&root);
        match mutation {
            "torn" => {
                let mut bytes = fs::read(&path).expect("read");
                assert_eq!(bytes.pop(), Some(b'\n'));
                fs::write(&path, bytes).expect("write");
            }
            "blank" => fs::write(&path, b" \t\n").expect("write"),
            "malformed" => fs::write(&path, b"{not-json}\n").expect("write"),
            "schema" => {
                let mut docs = read_documents(&root);
                docs[0].schema_version = "999".to_owned();
                write_documents(&root, &docs);
            }
            _ => unreachable!(),
        }
        let error = recover_workflow_governance_ledger(&root).expect_err("must reject");
        assert!(
            matches!(
                (&error, expected),
                (WorkflowGovernanceLedgerError::TornTail { .. }, "torn")
                    | (WorkflowGovernanceLedgerError::BlankLine { .. }, "blank")
                    | (
                        WorkflowGovernanceLedgerError::MalformedRecord { .. },
                        "malformed"
                    )
                    | (
                        WorkflowGovernanceLedgerError::UnsupportedSchema { .. },
                        "schema"
                    )
            ),
            "unexpected {error:?}"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn recovery_rejects_sequence_gap_and_wrong_previous_digest() {
    for wrong_sequence in [true, false] {
        let root = temp_root(if wrong_sequence { "gap" } else { "previous" });
        let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
            .expect("initialize");
        append_workflow_governance_event_tcb(
            &root,
            &first.record_digest,
            &identity(),
            1,
            advanced(1),
        )
        .expect("append");
        let mut docs = read_documents(&root);
        if wrong_sequence {
            docs[1].workflow_governance_receipt.sequence = 3;
        } else {
            docs[1].workflow_governance_receipt.previous_record_digest =
                Some("sha256:wrong".to_owned());
        }
        write_documents(&root, &docs);
        let error = recover_workflow_governance_ledger(&root).expect_err("must reject");
        assert!(if wrong_sequence {
            matches!(error, WorkflowGovernanceLedgerError::SequenceGap { .. })
        } else {
            matches!(
                error,
                WorkflowGovernanceLedgerError::PreviousDigestMismatch { .. }
            )
        });
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn append_rejects_stale_head_state_rollback_and_identity_mismatch() {
    let root = temp_root("cas-identity");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 2, imported())
        .expect("initialize");
    assert!(matches!(
        append_workflow_governance_event_tcb(&root, "sha256:stale", &identity(), 2, advanced(1)),
        Err(WorkflowGovernanceLedgerError::HeadMismatch { .. })
    ));
    assert!(matches!(
        append_workflow_governance_event_tcb(
            &root,
            &first.record_digest,
            &identity(),
            1,
            advanced(2)
        ),
        Err(WorkflowGovernanceLedgerError::StateVersionRegression { .. })
    ));

    let mut wrong_project = identity();
    wrong_project.project_id = id("other-project");
    assert!(matches!(
        append_workflow_governance_event_tcb(
            &root,
            &first.record_digest,
            &wrong_project,
            2,
            advanced(3)
        ),
        Err(WorkflowGovernanceLedgerError::ProjectMismatch { .. })
    ));
    let mut wrong_bundle = identity();
    wrong_bundle.bundle_digest = "sha256:other-bundle".to_owned();
    assert!(matches!(
        append_workflow_governance_event_tcb(
            &root,
            &first.record_digest,
            &wrong_bundle,
            2,
            advanced(4)
        ),
        Err(WorkflowGovernanceLedgerError::BundleMismatch { .. })
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recovery_rejects_duplicate_record_id_after_valid_rehash() {
    let root = temp_root("duplicate");
    let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    append_workflow_governance_event_tcb(&root, &first.record_digest, &identity(), 1, advanced(1))
        .expect("append");
    let mut docs = read_documents(&root);
    docs[1].workflow_governance_receipt.record_id =
        docs[0].workflow_governance_receipt.record_id.clone();
    docs[1].workflow_governance_receipt.record_digest =
        workflow_governance_record_digest(&docs[1].workflow_governance_receipt).expect("rehash");
    write_documents(&root, &docs);
    assert!(matches!(
        recover_workflow_governance_ledger(&root),
        Err(WorkflowGovernanceLedgerError::DuplicateRecordId { line: 2, .. })
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recovery_rejects_project_bundle_and_state_regression_even_when_rehashed() {
    for mutation in ["project", "bundle", "state"] {
        let root = temp_root(mutation);
        let first = initialize_workflow_governance_ledger_tcb(&root, &identity(), 2, imported())
            .expect("initialize");
        append_workflow_governance_event_tcb(
            &root,
            &first.record_digest,
            &identity(),
            3,
            advanced(1),
        )
        .expect("append");
        let mut docs = read_documents(&root);
        match mutation {
            "project" => docs[1].workflow_governance_receipt.project_id = id("evil"),
            "bundle" => docs[1].workflow_governance_receipt.bundle_id = id("evil-bundle"),
            "state" => docs[1].workflow_governance_receipt.state_version = 1,
            _ => unreachable!(),
        }
        docs[1].workflow_governance_receipt.record_digest =
            workflow_governance_record_digest(&docs[1].workflow_governance_receipt)
                .expect("rehash");
        write_documents(&root, &docs);
        let error = recover_workflow_governance_ledger(&root).expect_err("must reject");
        assert!(matches!(
            (mutation, error),
            (
                "project",
                WorkflowGovernanceLedgerError::ProjectMismatch { .. }
            ) | (
                "bundle",
                WorkflowGovernanceLedgerError::BundleMismatch { .. }
            ) | (
                "state",
                WorkflowGovernanceLedgerError::StateVersionRegression { .. }
            )
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn oversized_ledger_fails_before_unbounded_read() {
    let root = temp_root("capacity");
    let path = wal_path(&root);
    fs::create_dir_all(path.parent().expect("wal parent")).expect("create wal parent");
    let file = fs::File::create(&path).expect("create wal");
    file.set_len(WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES + 1)
        .expect("create sparse oversized WAL");
    assert!(matches!(
        recover_workflow_governance_ledger(&root),
        Err(WorkflowGovernanceLedgerError::CapacityBytes { .. })
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn record_count_capacity_is_enforced_on_an_otherwise_valid_chain() {
    let root = temp_root("record-capacity");
    let path = wal_path(&root);
    fs::create_dir_all(path.parent().expect("wal parent")).expect("create wal parent");
    let mut output = Vec::new();
    let mut previous = None;
    for index in 1..=10_001_u64 {
        let event = if index == 1 {
            imported()
        } else {
            advanced(usize::try_from(index).expect("index"))
        };
        let mut record = WorkflowGovernanceLedgerRecord {
            record_id: id(&format!("r{index}")),
            sequence: index,
            project_id: id("p"),
            bundle_id: id("b"),
            bundle_digest: "d".to_owned(),
            state_version: index - 1,
            previous_record_digest: previous,
            record_digest: String::new(),
            recorded_at_unix: 0,
            event,
        };
        record.record_digest = workflow_governance_record_digest(&record).expect("digest");
        previous = Some(record.record_digest.clone());
        let document = WorkflowGovernanceReceiptDocument {
            schema_version: WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION.to_owned(),
            workflow_governance_receipt: record,
        };
        output.extend(serde_json::to_vec(&document).expect("encode"));
        output.push(b'\n');
    }
    assert!(
        u64::try_from(output.len()).expect("fixture length")
            <= WORKFLOW_GOVERNANCE_LEDGER_MAX_BYTES,
        "fixture should exercise record count rather than byte capacity"
    );
    fs::write(&path, output).expect("write generated chain");
    assert!(matches!(
        recover_workflow_governance_ledger(&root),
        Err(WorkflowGovernanceLedgerError::CapacityRecords { found: 10_001, .. })
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn concurrent_under_lock_appends_are_serialized_without_lost_updates() {
    let root = Arc::new(temp_root("concurrent"));
    initialize_workflow_governance_ledger_tcb(root.as_ref(), &identity(), 0, imported())
        .expect("initialize");
    let barrier = Arc::new(Barrier::new(9));
    let mut threads = Vec::new();
    for index in 0..8 {
        let root = Arc::clone(&root);
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            let mut ledger = (0..400)
                .find_map(
                    |_| match lock_workflow_governance_ledger_tcb(root.as_ref()) {
                        Ok(ledger) => Some(ledger),
                        Err(WorkflowGovernanceLedgerError::Lock { .. }) => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                            None
                        }
                        Err(error) => panic!("unexpected lock failure: {error}"),
                    },
                )
                .expect("acquire lock within bounded retry window");
            let projection = ledger.recover().expect("recover under lock");
            let head = projection.head_digest.expect("initialized head");
            ledger
                .append_unchecked_tcb_event(&head, &identity(), 1, advanced(index + 1))
                .expect("serialized append")
        }));
    }
    barrier.wait();
    let appended: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().expect("thread"))
        .collect();
    let projection = recover_workflow_governance_ledger(root.as_ref()).expect("recover");
    assert_eq!(projection.records.len(), 9);
    assert_eq!(
        appended
            .iter()
            .map(|record| &record.record_id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        8
    );
    for (index, record) in projection.records.iter().enumerate() {
        assert_eq!(record.sequence, u64::try_from(index + 1).expect("sequence"));
    }
    fs::remove_dir_all(root.as_ref()).expect("cleanup");
}

#[test]
fn receipt_schema_constant_is_the_store_wire_schema() {
    let root = temp_root("schema-wire");
    initialize_workflow_governance_ledger_tcb(&root, &identity(), 0, imported())
        .expect("initialize");
    assert_eq!(
        read_documents(&root)[0].schema_version,
        WORKFLOW_GOVERNANCE_LEDGER_SCHEMA_VERSION
    );
    fs::remove_dir_all(root).expect("cleanup");
}
