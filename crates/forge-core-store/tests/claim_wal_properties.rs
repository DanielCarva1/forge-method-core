use forge_core_contracts::claim::{
    ActorRole, ClaimContract, ClaimIdentity, ClaimKind, ClaimLease, ClaimScope, ClaimScopeKind,
    ClaimStatus, ClaimStatusRecord, ExpiryAction, ExpiryPolicy, ReclaimPolicy,
};
use forge_core_contracts::{ClaimId, RepoPath, ScopeId, StableId};
use forge_core_store::claim_wal::{
    append_claim_wal_record, claim_wal_path, project_claim_wal_recovery, recover_claim_wal,
    replay_claim_wal, rotate_claim_wal_if_needed, ClaimWalOperation, ClaimWalPayload,
    ClaimWalRecord, ClaimWalRecovery, ClaimWalRotationOptions, ClaimWalStopReason, ProjectedClaim,
};
use proptest::prelude::*;
use proptest::test_runner::Config as ProptestConfig;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const HEADER_LEN: usize = 24;
const DUMMY_RECORD_LEN: u64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedPartition {
    Active,
    Released,
    HandoffRecorded,
    HistoricalOnly,
}

fn temp_state(test_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-claim-wal-props-{test_name}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create temp state root");
    root
}

fn claim(scope: &str, agent: &str, path: &str, status: ClaimStatus) -> ClaimContract {
    ClaimContract {
        id: ClaimId(format!("claim.story.{scope}.{scope}")),
        contract_ref: RepoPath(format!("claims-active/claim-story-{scope}-{scope}.yaml")),
        claim: ClaimIdentity {
            kind: ClaimKind::Story,
            claimant_agent_id: StableId(agent.to_string()),
            claimant_role: ActorRole::Worker,
            registry_ref: None,
        },
        scope: ClaimScope {
            kind: ClaimScopeKind::Story,
            id: ScopeId(scope.to_string()),
            product_area: None,
            paths: vec![RepoPath(path.to_string())],
        },
        lease: ClaimLease {
            acquired_at: "2027-01-15T08:00:00Z".to_string(),
            last_heartbeat_at: "2027-01-15T08:00:00Z".to_string(),
            expires_at: "2027-01-15T08:10:00Z".to_string(),
            ttl_seconds: 600,
            heartbeat_interval_seconds: 120,
            expected_state_version: 0,
        },
        status: ClaimStatusRecord {
            value: status,
            evaluated_at: "2027-01-15T08:00:00Z".to_string(),
            reason_code: None,
        },
        expiry_policy: ExpiryPolicy {
            on_expiry: ExpiryAction::RecordHandoffRequest,
            handoff_required: true,
            release_without_handoff_allowed: false,
            reclaim_policy: ReclaimPolicy::DriverReview,
            handoff_request_ref: Some(RepoPath(
                "contracts/requests/claim-expiry-handoff-request.yaml".to_string(),
            )),
        },
        evidence_refs: Vec::new(),
    }
}

fn record(seq: u64, operation: ClaimWalOperation, claim_contract: ClaimContract) -> ClaimWalRecord {
    ClaimWalRecord {
        seq,
        operation,
        payload: ClaimWalPayload {
            schema_version: "0.1".to_string(),
            operation,
            recorded_at: format!("2027-01-15T08:{seq:02}:00Z"),
            claim_contract,
        },
        offset: seq.saturating_sub(1).saturating_mul(DUMMY_RECORD_LEN),
        record_len: DUMMY_RECORD_LEN,
    }
}

fn recovery_from_records(records: Vec<ClaimWalRecord>) -> ClaimWalRecovery {
    let last_observed_seq = records.last().map_or(0, |record| record.seq);
    let original_len = DUMMY_RECORD_LEN.saturating_mul(u64::try_from(records.len()).unwrap_or(0));
    ClaimWalRecovery {
        wal_path: PathBuf::from("pure-projection-property.fmw1"),
        records,
        checkpoint: None,
        last_observed_seq,
        valid_record_count: usize::try_from(last_observed_seq).unwrap_or(usize::MAX),
        last_good_offset: original_len,
        original_len,
        repaired: false,
        stop_reason: ClaimWalStopReason::CleanEof,
    }
}

fn scenario_script(scenario: u8) -> (Vec<(ClaimWalOperation, ClaimStatus)>, ClaimStatus) {
    match scenario % 7 {
        0 => (
            vec![(ClaimWalOperation::Acquire, ClaimStatus::Active)],
            ClaimStatus::Active,
        ),
        1 => (
            vec![
                (ClaimWalOperation::Acquire, ClaimStatus::Active),
                (ClaimWalOperation::Heartbeat, ClaimStatus::Active),
            ],
            ClaimStatus::Active,
        ),
        2 => (
            vec![
                (ClaimWalOperation::Acquire, ClaimStatus::Active),
                (ClaimWalOperation::Release, ClaimStatus::Released),
            ],
            ClaimStatus::Released,
        ),
        3 => (
            vec![
                (ClaimWalOperation::Acquire, ClaimStatus::Active),
                (ClaimWalOperation::ReconcileStatus, ClaimStatus::Stale),
            ],
            ClaimStatus::Stale,
        ),
        4 => (
            vec![
                (ClaimWalOperation::Acquire, ClaimStatus::Active),
                (ClaimWalOperation::ReconcileStatus, ClaimStatus::Expired),
            ],
            ClaimStatus::Expired,
        ),
        5 => (
            vec![
                (ClaimWalOperation::Acquire, ClaimStatus::Active),
                (
                    ClaimWalOperation::ReconcileStatus,
                    ClaimStatus::HandoffRequired,
                ),
            ],
            ClaimStatus::HandoffRequired,
        ),
        _ => (
            vec![
                (ClaimWalOperation::Acquire, ClaimStatus::Active),
                (
                    ClaimWalOperation::ReconcileStatus,
                    ClaimStatus::HandoffRequired,
                ),
                (
                    ClaimWalOperation::HandoffRecorded,
                    ClaimStatus::HandoffRecorded,
                ),
            ],
            ClaimStatus::HandoffRecorded,
        ),
    }
}

fn expected_partition(status: ClaimStatus) -> ExpectedPartition {
    match status {
        ClaimStatus::Active | ClaimStatus::Stale => ExpectedPartition::Active,
        ClaimStatus::Released => ExpectedPartition::Released,
        ClaimStatus::HandoffRecorded => ExpectedPartition::HandoffRecorded,
        ClaimStatus::Expired | ClaimStatus::HandoffRequired => ExpectedPartition::HistoricalOnly,
    }
}

fn push_index(index: &mut BTreeMap<String, Vec<String>>, key: String, claim_id: &str) {
    index.entry(key).or_default().push(claim_id.to_string());
}

fn normalize_index(index: &mut BTreeMap<String, Vec<String>>) {
    for values in index.values_mut() {
        values.sort();
        values.dedup();
    }
}

fn build_tiny_wal(root: &Path) {
    append_claim_wal_record(
        root,
        ClaimWalOperation::Acquire,
        &claim("S1", "alice", "src/lib.rs", ClaimStatus::Active),
        "2027-01-15T08:00:00Z",
    )
    .expect("append S1 acquire");
    append_claim_wal_record(
        root,
        ClaimWalOperation::Acquire,
        &claim("S2", "bob", "src/other.rs", ClaimStatus::Active),
        "2027-01-15T08:01:00Z",
    )
    .expect("append S2 acquire");
    append_claim_wal_record(
        root,
        ClaimWalOperation::Release,
        &claim("S2", "bob", "src/other.rs", ClaimStatus::Released),
        "2027-01-15T08:02:00Z",
    )
    .expect("append S2 release");
}

fn write_wal_bytes(root: &Path, bytes: &[u8]) {
    let path = claim_wal_path(root);
    fs::create_dir_all(path.parent().expect("WAL path has parent")).expect("create WAL parent");
    fs::write(path, bytes).expect("write WAL bytes");
}

fn rotation_options_by_record_count(max_records: usize) -> ClaimWalRotationOptions {
    ClaimWalRotationOptions {
        max_wal_bytes: u64::MAX,
        max_records,
        max_replay_millis: u64::MAX,
    }
}

fn assert_projected_maps_match_without_offsets(
    label: &str,
    left: &BTreeMap<String, ProjectedClaim>,
    right: &BTreeMap<String, ProjectedClaim>,
) {
    assert_eq!(left.len(), right.len(), "{label} map length mismatch");
    for (claim_id, left_claim) in left {
        let right_claim = right
            .get(claim_id)
            .unwrap_or_else(|| panic!("{label} missing claim {claim_id}"));
        assert_eq!(
            left_claim.claim_contract, right_claim.claim_contract,
            "{label} claim contract mismatch for {claim_id}"
        );
        assert_eq!(
            left_claim.last_seq, right_claim.last_seq,
            "{label} last seq mismatch for {claim_id}"
        );
        assert_eq!(
            left_claim.last_operation, right_claim.last_operation,
            "{label} last operation mismatch for {claim_id}"
        );
        assert_eq!(
            left_claim.recorded_at, right_claim.recorded_at,
            "{label} recorded_at mismatch for {claim_id}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn projection_valid_lifecycle_scripts_match_reference_model(
        scenarios in proptest::collection::vec(0_u8..7, 1..=4),
    ) {
        let mut records = Vec::new();
        let mut expected_latest_status = BTreeMap::new();
        let mut expected_partitions = BTreeMap::new();
        let mut expected_agent_index = BTreeMap::new();
        let mut expected_scope_index = BTreeMap::new();
        let mut expected_path_index = BTreeMap::new();
        let mut next_seq = 1_u64;

        for (index, scenario) in scenarios.iter().enumerate() {
            let scope = format!("S{index}");
            let agent = format!("agent-{index}");
            let path = format!("src/story_{index}.rs");
            let (script, final_status) = scenario_script(*scenario);
            let claim_id = format!("claim.story.{scope}.{scope}");
            expected_latest_status.insert(claim_id.clone(), final_status);
            expected_partitions.insert(claim_id.clone(), expected_partition(final_status));
            if expected_partition(final_status) == ExpectedPartition::Active {
                push_index(&mut expected_agent_index, agent.clone(), &claim_id);
                push_index(&mut expected_scope_index, scope.clone(), &claim_id);
                push_index(&mut expected_path_index, path.clone(), &claim_id);
            }

            for (operation, status) in script {
                records.push(record(
                    next_seq,
                    operation,
                    claim(&scope, &agent, &path, status),
                ));
                next_seq = next_seq.saturating_add(1);
            }
        }
        normalize_index(&mut expected_agent_index);
        normalize_index(&mut expected_scope_index);
        normalize_index(&mut expected_path_index);

        let expected_record_count = records.len();
        let projection = project_claim_wal_recovery(recovery_from_records(records));
        prop_assert!(projection.diagnostics.is_empty());
        prop_assert_eq!(projection.latest_by_claim_id.len(), scenarios.len());
        prop_assert_eq!(projection.applied_records, expected_record_count);
        prop_assert_eq!(projection.last_applied_seq, next_seq.saturating_sub(1));
        prop_assert_eq!(projection.active_claim_ids_by_agent, expected_agent_index);
        prop_assert_eq!(projection.active_claim_ids_by_scope, expected_scope_index);
        prop_assert_eq!(projection.active_claim_ids_by_path, expected_path_index);

        let active_ids = projection
            .active_by_claim_id
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let released_ids = projection
            .released_by_claim_id
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let handoff_ids = projection
            .handoff_recorded_by_claim_id
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();

        for (claim_id, final_status) in expected_latest_status {
            let projected = projection
                .latest_by_claim_id
                .get(&claim_id)
                .expect("latest projected claim exists");
            prop_assert_eq!(projected.claim_contract.status.value, final_status);
            match expected_partitions
                .get(&claim_id)
                .expect("expected partition exists")
            {
                ExpectedPartition::Active => {
                    prop_assert!(active_ids.contains(&claim_id));
                    prop_assert!(!released_ids.contains(&claim_id));
                    prop_assert!(!handoff_ids.contains(&claim_id));
                }
                ExpectedPartition::Released => {
                    prop_assert!(!active_ids.contains(&claim_id));
                    prop_assert!(released_ids.contains(&claim_id));
                    prop_assert!(!handoff_ids.contains(&claim_id));
                }
                ExpectedPartition::HandoffRecorded => {
                    prop_assert!(!active_ids.contains(&claim_id));
                    prop_assert!(!released_ids.contains(&claim_id));
                    prop_assert!(handoff_ids.contains(&claim_id));
                }
                ExpectedPartition::HistoricalOnly => {
                    prop_assert!(!active_ids.contains(&claim_id));
                    prop_assert!(!released_ids.contains(&claim_id));
                    prop_assert!(!handoff_ids.contains(&claim_id));
                }
            }
        }
    }
}

#[test]
fn claim_wal_exhaustive_truncation_sweep_recovers_only_valid_prefix() {
    let source_root = temp_state("truncation-source");
    build_tiny_wal(&source_root);
    let clean_recovery = recover_claim_wal(&source_root, false).expect("recover clean source WAL");
    let original = fs::read(claim_wal_path(&source_root)).expect("read source WAL");
    let boundaries = clean_recovery
        .records
        .iter()
        .map(|record| {
            let offset = usize::try_from(record.offset).expect("record offset fits usize");
            let len = usize::try_from(record.record_len).expect("record length fits usize");
            offset + len
        })
        .collect::<Vec<_>>();

    let sweep_root = temp_state("truncation-sweep");
    for cut in 0..=original.len() {
        write_wal_bytes(&sweep_root, &original[..cut]);
        let bytes_before = fs::read(claim_wal_path(&sweep_root)).expect("read truncated WAL");
        let recovery = recover_claim_wal(&sweep_root, false).expect("recover truncated WAL");
        let bytes_after = fs::read(claim_wal_path(&sweep_root)).expect("read recovered WAL");

        assert_eq!(
            bytes_after, bytes_before,
            "read-only recovery mutated cut {cut}"
        );
        let expected_records = boundaries
            .iter()
            .take_while(|boundary| **boundary <= cut)
            .count();
        let last_good_offset = if expected_records == 0 {
            0
        } else {
            u64::try_from(boundaries[expected_records - 1]).expect("boundary fits u64")
        };
        assert_eq!(
            recovery.records.len(),
            expected_records,
            "record count mismatch at cut {cut}"
        );
        assert_eq!(
            recovery.last_good_offset, last_good_offset,
            "last good offset mismatch at cut {cut}"
        );
        let recovered_seqs = recovery
            .records
            .iter()
            .map(|record| record.seq)
            .collect::<Vec<_>>();
        let expected_seqs =
            (1..=u64::try_from(expected_records).expect("count fits u64")).collect::<Vec<_>>();
        assert_eq!(
            recovered_seqs, expected_seqs,
            "resync detected at cut {cut}"
        );

        let at_record_boundary = cut == 0 || boundaries.contains(&cut);
        let expected_stop_reason = if at_record_boundary {
            ClaimWalStopReason::CleanEof
        } else if cut.saturating_sub(usize::try_from(last_good_offset).expect("offset fits usize"))
            < HEADER_LEN
        {
            ClaimWalStopReason::TruncatedHeader
        } else {
            ClaimWalStopReason::TruncatedPayload
        };
        assert_eq!(
            recovery.stop_reason, expected_stop_reason,
            "stop reason mismatch at cut {cut}"
        );
    }
}

#[test]
fn claim_wal_append_repairs_torn_tail_then_repair_is_idempotent() {
    let root = temp_state("append-repairs-torn-tail");
    build_tiny_wal(&root);
    let path = claim_wal_path(&root);
    let mut bytes = fs::read(&path).expect("read clean WAL");
    bytes.extend_from_slice(b"FMW1partial");
    fs::write(&path, bytes).expect("write torn tail");

    let appended = append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &claim("S3", "cara", "src/new.rs", ClaimStatus::Active),
        "2027-01-15T08:03:00Z",
    )
    .expect("append after torn tail");
    assert_eq!(appended.seq, 4);

    let repaired_then_appended = recover_claim_wal(&root, false).expect("recover appended WAL");
    assert_eq!(
        repaired_then_appended.stop_reason,
        ClaimWalStopReason::CleanEof
    );
    let projected = project_claim_wal_recovery(repaired_then_appended.clone());
    assert!(
        projected.diagnostics.is_empty(),
        "repair-then-append projection should stay clean: {:?}",
        projected.diagnostics
    );
    let appended_claim = projected
        .latest_by_claim_id
        .get("claim.story.S3.S3")
        .expect("appended claim is projected");
    assert_eq!(appended_claim.last_seq, 4);
    assert_eq!(appended_claim.last_operation, ClaimWalOperation::Acquire);

    let before_idempotent_repair = fs::metadata(&path)
        .expect("metadata before idempotent repair")
        .len();
    let idempotent_repair = recover_claim_wal(&root, true).expect("recover clean WAL with repair");
    assert_eq!(idempotent_repair.stop_reason, ClaimWalStopReason::CleanEof);
    assert!(!idempotent_repair.repaired);
    assert_eq!(
        fs::metadata(&path)
            .expect("metadata after idempotent repair")
            .len(),
        before_idempotent_repair
    );
}

#[test]
fn claim_wal_checksum_tail_repair_is_idempotent_and_append_resumes_sequence() {
    let root = temp_state("checksum-tail-repair");
    build_tiny_wal(&root);
    let path = claim_wal_path(&root);
    let clean_recovery = recover_claim_wal(&root, false).expect("recover clean WAL");
    let third_offset =
        usize::try_from(clean_recovery.records[2].offset).expect("third record offset fits usize");
    let mut bytes = fs::read(&path).expect("read clean WAL");
    bytes[third_offset + HEADER_LEN] ^= 0b0000_0001;
    fs::write(&path, bytes).expect("write checksum-corrupted tail");

    let read_only = recover_claim_wal(&root, false).expect("read-only recover corrupt WAL");
    assert_eq!(
        read_only.stop_reason,
        ClaimWalStopReason::PayloadChecksumMismatch
    );
    assert_eq!(read_only.records.len(), 2);
    assert_eq!(read_only.last_good_offset, clean_recovery.records[2].offset);

    let repaired = recover_claim_wal(&root, true).expect("repair checksum-corrupted WAL");
    assert_eq!(
        repaired.stop_reason,
        ClaimWalStopReason::PayloadChecksumMismatch
    );
    assert!(repaired.repaired);
    assert_eq!(repaired.records.len(), 2);
    assert_eq!(
        fs::metadata(&path).expect("metadata after repair").len(),
        clean_recovery.records[2].offset
    );

    let second_repair = recover_claim_wal(&root, true).expect("repeat repair");
    assert_eq!(second_repair.stop_reason, ClaimWalStopReason::CleanEof);
    assert!(!second_repair.repaired);
    assert_eq!(second_repair.records.len(), 2);

    let appended = append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &claim("S3", "cara", "src/new.rs", ClaimStatus::Active),
        "2027-01-15T08:03:00Z",
    )
    .expect("append after checksum repair");
    assert_eq!(appended.seq, 3);
    let final_recovery = recover_claim_wal(&root, false).expect("recover final WAL");
    assert_eq!(final_recovery.stop_reason, ClaimWalStopReason::CleanEof);
    assert_eq!(final_recovery.records.len(), 3);
}

#[test]
fn claim_wal_second_rotation_snapshots_checkpoint_state_plus_suffix() {
    let root = temp_state("second-rotation");
    build_tiny_wal(&root);
    let first_rotation = rotate_claim_wal_if_needed(
        &root,
        "2027-01-15T08:03:00Z",
        &rotation_options_by_record_count(2),
    )
    .expect("first rotation");
    assert!(first_rotation.rotated);
    assert_eq!(first_rotation.last_seq_in_snapshot, 3);
    assert_eq!(first_rotation.checkpoint_seq, Some(4));

    let heartbeat = append_claim_wal_record(
        &root,
        ClaimWalOperation::Heartbeat,
        &claim("S1", "alice", "src/lib.rs", ClaimStatus::Active),
        "2027-01-15T08:04:00Z",
    )
    .expect("append heartbeat after first rotation");
    let s3_acquire = append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &claim("S3", "cara", "src/new.rs", ClaimStatus::Active),
        "2027-01-15T08:05:00Z",
    )
    .expect("append S3 acquire after first rotation");
    let s3_release = append_claim_wal_record(
        &root,
        ClaimWalOperation::Release,
        &claim("S3", "cara", "src/new.rs", ClaimStatus::Released),
        "2027-01-15T08:06:00Z",
    )
    .expect("append S3 release after first rotation");
    assert_eq!((heartbeat.seq, s3_acquire.seq, s3_release.seq), (5, 6, 7));

    let before_second_rotation =
        replay_claim_wal(&root, false).expect("replay before second rotation");
    let second_rotation = rotate_claim_wal_if_needed(
        &root,
        "2027-01-15T08:07:00Z",
        &rotation_options_by_record_count(1),
    )
    .expect("second rotation");
    assert!(second_rotation.rotated);
    assert_eq!(second_rotation.last_seq_in_snapshot, 7);
    assert_eq!(second_rotation.checkpoint_seq, Some(8));

    let recovery = recover_claim_wal(&root, false).expect("recover second-rotated WAL");
    assert_eq!(recovery.stop_reason, ClaimWalStopReason::CleanEof);
    assert_eq!(recovery.records.len(), 0);
    assert_eq!(recovery.last_observed_seq, 8);
    assert_eq!(
        recovery
            .checkpoint
            .as_ref()
            .expect("second checkpoint")
            .payload
            .last_seq_in_snapshot,
        7
    );

    let after_second_rotation = replay_claim_wal(&root, false).expect("replay second rotation");
    assert_projected_maps_match_without_offsets(
        "latest",
        &after_second_rotation.latest_by_claim_id,
        &before_second_rotation.latest_by_claim_id,
    );
    assert_projected_maps_match_without_offsets(
        "active",
        &after_second_rotation.active_by_claim_id,
        &before_second_rotation.active_by_claim_id,
    );
    assert_projected_maps_match_without_offsets(
        "released",
        &after_second_rotation.released_by_claim_id,
        &before_second_rotation.released_by_claim_id,
    );
    assert!(after_second_rotation
        .active_by_claim_id
        .contains_key("claim.story.S1.S1"));
    assert!(after_second_rotation
        .released_by_claim_id
        .contains_key("claim.story.S2.S2"));
    assert!(after_second_rotation
        .released_by_claim_id
        .contains_key("claim.story.S3.S3"));
}
