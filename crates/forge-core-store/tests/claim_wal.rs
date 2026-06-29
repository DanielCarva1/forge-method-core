use forge_core_contracts::claim::{
    ActorRole, ClaimContract, ClaimIdentity, ClaimKind, ClaimLease, ClaimScope, ClaimScopeKind,
    ClaimStatus, ClaimStatusRecord, ExpiryAction, ExpiryPolicy, ReclaimPolicy,
};
use forge_core_contracts::{ClaimId, RepoPath, ScopeId, StableId};
use forge_core_store::claim_wal::{
    append_claim_wal_record, claim_wal_path, recover_claim_wal, replay_claim_wal,
    rotate_claim_wal_if_needed, ClaimWalManifestPayload, ClaimWalOperation, ClaimWalProjection,
    ClaimWalRecovery, ClaimWalRotationOptions, ClaimWalRotationReason, ClaimWalRotationResult,
    ClaimWalStopReason,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const HEADER_LEN: usize = 24;
const HEADER_CRC_OFFSET: usize = 20;
const FLAG_SKIPPABLE_UNKNOWN: u16 = 0b0000_0001;
const FLAG_PAYLOAD_JSON: u16 = 0b0000_0100;

fn temp_state(test_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-claim-wal-{test_name}-{}-{nanos}",
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

fn append_raw(path: &Path, bytes: &[u8]) {
    let mut existing = fs::read(path).expect("read existing WAL");
    existing.extend_from_slice(bytes);
    fs::write(path, existing).expect("append raw bytes");
}

fn rewrite_header_crc(bytes: &mut [u8], offset: usize) {
    let crc = crc32c::crc32c(&bytes[offset..offset + HEADER_CRC_OFFSET]);
    bytes[offset + HEADER_CRC_OFFSET..offset + HEADER_LEN].copy_from_slice(&crc.to_le_bytes());
}

fn rewrite_payload_crc(bytes: &mut [u8], offset: usize) {
    let payload_len = u32::from_le_bytes(
        bytes[offset + 16..offset + 20]
            .try_into()
            .expect("payload len bytes"),
    ) as usize;
    let payload_start = offset + HEADER_LEN;
    let payload_end = payload_start + payload_len;
    let mut covered = Vec::new();
    covered.extend_from_slice(&bytes[offset..offset + HEADER_CRC_OFFSET]);
    covered.extend_from_slice(&bytes[payload_start..payload_end]);
    let crc = crc32c::crc32c(&covered);
    bytes[payload_end..payload_end + 4].copy_from_slice(&crc.to_le_bytes());
}

#[test]
fn claim_wal_appends_fmw1_records_with_monotonic_sequences() {
    let root = temp_state("monotonic");
    let first = claim("S1", "alice", "src/lib.rs", ClaimStatus::Active);
    let second = claim("S1", "alice", "src/lib.rs", ClaimStatus::Released);

    let first_append = append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &first,
        "2027-01-15T08:00:00Z",
    )
    .expect("append first claim WAL record");
    let second_append = append_claim_wal_record(
        &root,
        ClaimWalOperation::Release,
        &second,
        "2027-01-15T08:01:00Z",
    )
    .expect("append second claim WAL record");

    assert_eq!(first_append.seq, 1);
    assert_eq!(second_append.seq, 2);
    let bytes = fs::read(claim_wal_path(&root)).expect("read WAL");
    assert_eq!(&bytes[0..4], b"FMW1");

    let recovery = recover_claim_wal(&root, false).expect("recover claim WAL");
    assert_eq!(recovery.stop_reason, ClaimWalStopReason::CleanEof);
    assert_eq!(recovery.records.len(), 2);
    assert_eq!(recovery.records[0].operation, ClaimWalOperation::Acquire);
    assert_eq!(recovery.records[1].operation, ClaimWalOperation::Release);
    assert!(!recovery.repaired);
}

#[test]
fn claim_wal_replay_projects_last_record_per_claim_id() {
    let root = temp_state("projection-last-wins");
    let first_active = claim("S1", "alice", "src/lib.rs", ClaimStatus::Active);
    let released = claim("S1", "alice", "src/lib.rs", ClaimStatus::Released);
    let reacquired = claim("S1", "bob", "src/lib.rs", ClaimStatus::Active);
    let independent = claim("S2", "cara", "src/other.rs", ClaimStatus::Active);

    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &first_active,
        "2027-01-15T08:00:00Z",
    )
    .expect("append first active");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Release,
        &released,
        "2027-01-15T08:01:00Z",
    )
    .expect("append release");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &reacquired,
        "2027-01-15T08:02:00Z",
    )
    .expect("append reacquire");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &independent,
        "2027-01-15T08:03:00Z",
    )
    .expect("append independent claim");

    let projection = replay_claim_wal(&root, true).expect("replay WAL projection");

    assert_eq!(projection.recovery.records.len(), 4);
    assert_eq!(projection.claims.len(), 2);
    let s1 = projection
        .claims
        .iter()
        .find(|claim| claim.scope.id.0 == "S1")
        .expect("S1 projected claim");
    assert_eq!(s1.status.value, ClaimStatus::Active);
    assert_eq!(s1.claim.claimant_agent_id.0, "bob");
    let s2 = projection
        .claims
        .iter()
        .find(|claim| claim.scope.id.0 == "S2")
        .expect("S2 projected claim");
    assert_eq!(s2.claim.claimant_agent_id.0, "cara");
}

#[test]
fn claim_wal_replay_projects_reconcile_status_records() {
    let root = temp_state("projection-reconcile-status");
    let active = claim("S1", "alice", "src/lib.rs", ClaimStatus::Active);
    let stale = claim("S1", "alice", "src/lib.rs", ClaimStatus::Stale);
    let handoff_required = claim("S1", "alice", "src/lib.rs", ClaimStatus::HandoffRequired);

    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &active,
        "2027-01-15T08:00:00Z",
    )
    .expect("append acquire");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::ReconcileStatus,
        &stale,
        "2027-01-15T08:02:00Z",
    )
    .expect("append stale reconcile");

    let projection = replay_claim_wal(&root, true).expect("replay stale projection");
    let projected = projection
        .latest_by_claim_id
        .get("claim.story.S1.S1")
        .expect("latest claim");
    assert_eq!(projected.claim_contract.status.value, ClaimStatus::Stale);
    assert!(
        projection
            .active_by_claim_id
            .contains_key("claim.story.S1.S1"),
        "stale claims remain open/live for conflict purposes"
    );

    append_claim_wal_record(
        &root,
        ClaimWalOperation::ReconcileStatus,
        &handoff_required,
        "2027-01-15T08:10:00Z",
    )
    .expect("append handoff-required reconcile");
    let projection = replay_claim_wal(&root, true).expect("replay handoff projection");
    let projected = projection
        .latest_by_claim_id
        .get("claim.story.S1.S1")
        .expect("latest claim");
    assert_eq!(
        projected.claim_contract.status.value,
        ClaimStatus::HandoffRequired
    );
    assert!(
        !projection
            .active_by_claim_id
            .contains_key("claim.story.S1.S1"),
        "handoff-required claims leave active indexes but remain latest authority"
    );
}

#[test]
fn claim_wal_replay_allows_handoff_recorded_after_materialized_handoff_required() {
    let root = temp_state("projection-handoff-after-reconcile");
    let active = claim("S1", "alice", "src/lib.rs", ClaimStatus::Active);
    let handoff_required = claim("S1", "alice", "src/lib.rs", ClaimStatus::HandoffRequired);
    let handoff_recorded = claim("S1", "alice", "src/lib.rs", ClaimStatus::HandoffRecorded);

    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &active,
        "2027-01-15T08:00:00Z",
    )
    .expect("append acquire");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::ReconcileStatus,
        &handoff_required,
        "2027-01-15T08:10:00Z",
    )
    .expect("append handoff-required reconcile");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::HandoffRecorded,
        &handoff_recorded,
        "2027-01-15T08:11:00Z",
    )
    .expect("append handoff recorded");

    let projection = replay_claim_wal(&root, true).expect("replay handoff recorded projection");

    assert!(
        projection.diagnostics.is_empty(),
        "{:?}",
        projection.diagnostics
    );
    assert_eq!(
        projection
            .latest_by_claim_id
            .get("claim.story.S1.S1")
            .expect("latest claim")
            .claim_contract
            .status
            .value,
        ClaimStatus::HandoffRecorded
    );
    assert!(projection
        .handoff_recorded_by_claim_id
        .contains_key("claim.story.S1.S1"));
}

#[test]
fn claim_wal_repair_truncates_torn_tail_to_valid_prefix() {
    let root = temp_state("torn-tail");
    let active = claim("S1", "alice", "src/lib.rs", ClaimStatus::Active);
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &active,
        "2027-01-15T08:00:00Z",
    )
    .expect("append claim WAL record");
    let path = claim_wal_path(&root);
    let valid_len = fs::metadata(&path).expect("metadata").len();
    append_raw(&path, b"FMW1partial");

    let read_only = recover_claim_wal(&root, false).expect("read-only recover claim WAL");
    assert_eq!(read_only.stop_reason, ClaimWalStopReason::TruncatedHeader);
    assert_eq!(fs::metadata(&path).expect("metadata").len(), valid_len + 11);

    let repaired = recover_claim_wal(&root, true).expect("repair claim WAL");
    assert_eq!(repaired.records.len(), 1);
    assert_eq!(repaired.stop_reason, ClaimWalStopReason::TruncatedHeader);
    assert!(repaired.repaired);
    assert_eq!(fs::metadata(&path).expect("metadata").len(), valid_len);
}

#[test]
fn claim_wal_stops_at_payload_checksum_failure_without_resync() {
    let root = temp_state("checksum");
    let first = claim("S1", "alice", "src/lib.rs", ClaimStatus::Active);
    let second = claim("S2", "bob", "src/other.rs", ClaimStatus::Active);
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &first,
        "2027-01-15T08:00:00Z",
    )
    .expect("append first");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &second,
        "2027-01-15T08:01:00Z",
    )
    .expect("append second");

    let path = claim_wal_path(&root);
    let mut bytes = fs::read(&path).expect("read WAL");
    let first_len = usize::try_from(
        recover_claim_wal(&root, false)
            .expect("recover clean WAL")
            .records[0]
            .record_len,
    )
    .expect("record length fits usize");
    bytes[first_len + 30] ^= 0b0000_0001;
    fs::write(&path, bytes).expect("write corrupted WAL");

    let recovery = recover_claim_wal(&root, false).expect("recover corrupted WAL");
    assert_eq!(
        recovery.stop_reason,
        ClaimWalStopReason::PayloadChecksumMismatch
    );
    assert_eq!(recovery.records.len(), 1);
    assert_eq!(recovery.records[0].payload.claim_contract.scope.id.0, "S1");
}

#[test]
fn claim_wal_rejects_sequence_gap_even_with_valid_crc() {
    let root = temp_state("seq-gap");
    let first = claim("S1", "alice", "src/lib.rs", ClaimStatus::Active);
    let second = claim("S2", "bob", "src/other.rs", ClaimStatus::Active);
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &first,
        "2027-01-15T08:00:00Z",
    )
    .expect("append first");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &second,
        "2027-01-15T08:01:00Z",
    )
    .expect("append second");

    let path = claim_wal_path(&root);
    let recovery = recover_claim_wal(&root, false).expect("recover clean WAL");
    let second_offset =
        usize::try_from(recovery.records[1].offset).expect("record offset fits usize");
    let mut bytes = fs::read(&path).expect("read WAL");
    bytes[second_offset + 8..second_offset + 16].copy_from_slice(&3_u64.to_le_bytes());
    rewrite_header_crc(&mut bytes, second_offset);
    rewrite_payload_crc(&mut bytes, second_offset);
    fs::write(&path, bytes).expect("write seq gap WAL");

    let recovery = recover_claim_wal(&root, false).expect("recover seq gap WAL");
    assert_eq!(recovery.stop_reason, ClaimWalStopReason::SequenceGap);
    assert_eq!(recovery.records.len(), 1);
}

#[test]
fn claim_wal_skips_unknown_skippable_records_and_stops_on_unskippable() {
    let root = temp_state("unknown-records");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &claim("S1", "alice", "src/lib.rs", ClaimStatus::Active),
        "2027-01-15T08:00:00Z",
    )
    .expect("append first");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Heartbeat,
        &claim("S1", "alice", "src/lib.rs", ClaimStatus::Active),
        "2027-01-15T08:01:00Z",
    )
    .expect("append skippable candidate");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Release,
        &claim("S1", "alice", "src/lib.rs", ClaimStatus::Released),
        "2027-01-15T08:02:00Z",
    )
    .expect("append third");

    let path = claim_wal_path(&root);
    let recovery = recover_claim_wal(&root, false).expect("recover clean WAL");
    let second_offset =
        usize::try_from(recovery.records[1].offset).expect("record offset fits usize");
    let mut bytes = fs::read(&path).expect("read WAL");
    bytes[second_offset + 5] = 99;
    bytes[second_offset + 6..second_offset + 8]
        .copy_from_slice(&(FLAG_SKIPPABLE_UNKNOWN | FLAG_PAYLOAD_JSON).to_le_bytes());
    rewrite_header_crc(&mut bytes, second_offset);
    rewrite_payload_crc(&mut bytes, second_offset);
    fs::write(&path, &bytes).expect("write skippable unknown WAL");

    let recovery = recover_claim_wal(&root, false).expect("recover skippable unknown WAL");
    assert_eq!(recovery.stop_reason, ClaimWalStopReason::CleanEof);
    assert_eq!(recovery.records.len(), 2);
    assert_eq!(recovery.records[0].seq, 1);
    assert_eq!(recovery.records[1].seq, 3);
    assert_eq!(recovery.records[1].operation, ClaimWalOperation::Release);

    let mut unskippable = bytes;
    unskippable[second_offset + 6..second_offset + 8]
        .copy_from_slice(&FLAG_PAYLOAD_JSON.to_le_bytes());
    rewrite_header_crc(&mut unskippable, second_offset);
    rewrite_payload_crc(&mut unskippable, second_offset);
    fs::write(&path, unskippable).expect("write unskippable unknown WAL");

    let recovery = recover_claim_wal(&root, false).expect("recover unskippable unknown WAL");
    assert_eq!(
        recovery.stop_reason,
        ClaimWalStopReason::UnsupportedRecordType
    );
    assert_eq!(recovery.records.len(), 1);
}

fn rotation_options_by_record_count(max_records: usize) -> ClaimWalRotationOptions {
    ClaimWalRotationOptions {
        max_wal_bytes: u64::MAX,
        max_records,
        max_replay_millis: u64::MAX,
    }
}

fn assert_record_count_rotation_result(rotation: &ClaimWalRotationResult) {
    assert!(rotation.rotated);
    assert_eq!(rotation.reason, Some(ClaimWalRotationReason::RecordCount));
    assert_eq!(rotation.last_seq_in_snapshot, 3);
    assert_eq!(rotation.checkpoint_seq, Some(4));
    assert_eq!(rotation.compacted_records, 3);
    assert!(rotation
        .snapshot_path
        .as_ref()
        .expect("snapshot path")
        .exists());
    assert!(rotation
        .archived_wal_path
        .as_ref()
        .expect("archive path")
        .exists());
    assert!(rotation
        .manifest_path
        .as_ref()
        .expect("manifest path")
        .exists());
}

fn assert_raw_checkpoint_and_archive_headers(root: &Path, rotation: &ClaimWalRotationResult) {
    let active_wal_bytes = fs::read(claim_wal_path(root)).expect("read rotated active WAL");
    assert_eq!(
        active_wal_bytes[5], 4,
        "rotated active WAL must start with checkpoint_ref record type 4"
    );
    assert_eq!(
        u64::from_le_bytes(
            active_wal_bytes[8..16]
                .try_into()
                .expect("checkpoint seq bytes")
        ),
        4
    );
    let archived_wal_bytes = fs::read(rotation.archived_wal_path.as_ref().expect("archive path"))
        .expect("read archived WAL");
    assert_eq!(
        archived_wal_bytes[5], 1,
        "archived WAL should preserve original first acquire record"
    );
    assert_eq!(
        u64::from_le_bytes(
            archived_wal_bytes[8..16]
                .try_into()
                .expect("archived first seq bytes")
        ),
        1
    );
    let manifest_bytes = fs::read(rotation.manifest_path.as_ref().expect("manifest path"))
        .expect("read rotation manifest");
    let manifest: ClaimWalManifestPayload =
        serde_json::from_slice(&manifest_bytes).expect("decode rotation manifest");
    assert_eq!(manifest.schema_version, "0.1");
    assert_eq!(manifest.active_wal_path, "wal/claims.fmw1");
    assert_eq!(manifest.checkpoint_seq, 4);
    assert_eq!(manifest.last_seq_in_snapshot, 3);
}

fn assert_rotated_recovery(recovery: &ClaimWalRecovery) {
    assert_eq!(recovery.stop_reason, ClaimWalStopReason::CleanEof);
    assert_eq!(recovery.records.len(), 0);
    assert_eq!(recovery.last_observed_seq, 4);
    assert_eq!(recovery.valid_record_count, 1);
    assert_eq!(
        recovery
            .checkpoint
            .as_ref()
            .expect("checkpoint record")
            .payload
            .last_seq_in_snapshot,
        3
    );
}

fn assert_rotated_projection(projection: &ClaimWalProjection) {
    assert_eq!(projection.claims.len(), 2);
    assert!(projection
        .active_by_claim_id
        .contains_key("claim.story.S1.S1"));
    assert!(projection
        .released_by_claim_id
        .contains_key("claim.story.S2.S2"));
    assert_eq!(
        projection
            .latest_by_claim_id
            .get("claim.story.S2.S2")
            .expect("latest S2")
            .claim_contract
            .status
            .value,
        ClaimStatus::Released
    );
}

#[test]
fn claim_wal_rotation_writes_snapshot_checkpoint_and_replays_from_snapshot() {
    let root = temp_state("rotation-record-count");
    let s1_active = claim("S1", "alice", "src/lib.rs", ClaimStatus::Active);
    let s2_active = claim("S2", "bob", "src/other.rs", ClaimStatus::Active);
    let s2_released = claim("S2", "bob", "src/other.rs", ClaimStatus::Released);

    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &s1_active,
        "2027-01-15T08:00:00Z",
    )
    .expect("append S1 acquire");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &s2_active,
        "2027-01-15T08:01:00Z",
    )
    .expect("append S2 acquire");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Release,
        &s2_released,
        "2027-01-15T08:02:00Z",
    )
    .expect("append S2 release");

    let rotation = rotate_claim_wal_if_needed(
        &root,
        "2027-01-15T08:03:00Z",
        &rotation_options_by_record_count(2),
    )
    .expect("rotate WAL by record count");

    assert_record_count_rotation_result(&rotation);

    let recovery = recover_claim_wal(&root, false).expect("recover rotated WAL");
    assert_raw_checkpoint_and_archive_headers(&root, &rotation);
    assert_rotated_recovery(&recovery);

    let projection = replay_claim_wal(&root, false).expect("replay rotated WAL");
    assert_rotated_projection(&projection);

    let heartbeat = append_claim_wal_record(
        &root,
        ClaimWalOperation::Heartbeat,
        &s1_active,
        "2027-01-15T08:04:00Z",
    )
    .expect("append heartbeat after rotation");
    assert_eq!(heartbeat.seq, 5);

    let projection = replay_claim_wal(&root, false).expect("replay after post-rotation append");
    assert_eq!(projection.recovery.records.len(), 1);
    assert_eq!(projection.recovery.last_observed_seq, 5);
    assert!(projection
        .active_by_claim_id
        .contains_key("claim.story.S1.S1"));
    assert!(projection
        .released_by_claim_id
        .contains_key("claim.story.S2.S2"));
}

#[test]
fn claim_wal_rotation_triggers_by_size_threshold() {
    let root = temp_state("rotation-size");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &claim("S1", "alice", "src/lib.rs", ClaimStatus::Active),
        "2027-01-15T08:00:00Z",
    )
    .expect("append acquire");
    let wal_len = fs::metadata(claim_wal_path(&root))
        .expect("WAL metadata")
        .len();

    let rotation = rotate_claim_wal_if_needed(
        &root,
        "2027-01-15T08:01:00Z",
        &ClaimWalRotationOptions {
            max_wal_bytes: wal_len.saturating_sub(1),
            max_records: usize::MAX,
            max_replay_millis: u64::MAX,
        },
    )
    .expect("rotate by size threshold");

    assert!(rotation.rotated);
    assert_eq!(rotation.reason, Some(ClaimWalRotationReason::WalSizeBytes));
}

#[test]
fn claim_wal_rotation_is_noop_below_thresholds() {
    let root = temp_state("rotation-noop");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &claim("S1", "alice", "src/lib.rs", ClaimStatus::Active),
        "2027-01-15T08:00:00Z",
    )
    .expect("append acquire");

    let rotation = rotate_claim_wal_if_needed(
        &root,
        "2027-01-15T08:01:00Z",
        &rotation_options_by_record_count(10),
    )
    .expect("check rotation threshold");

    assert!(!rotation.rotated);
    assert_eq!(rotation.reason, None);
    assert_eq!(rotation.snapshot_path, None);
    assert_eq!(rotation.archived_wal_path, None);
    assert_eq!(rotation.manifest_path, None);
    let recovery = recover_claim_wal(&root, false).expect("recover unrotated WAL");
    assert_eq!(recovery.records.len(), 1);
    assert_eq!(recovery.checkpoint, None);
}

#[test]
fn claim_wal_checkpoint_missing_snapshot_fails_closed_and_blocks_append() {
    let root = temp_state("rotation-missing-snapshot");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &claim("S1", "alice", "src/lib.rs", ClaimStatus::Active),
        "2027-01-15T08:00:00Z",
    )
    .expect("append acquire");
    let rotation = rotate_claim_wal_if_needed(
        &root,
        "2027-01-15T08:01:00Z",
        &rotation_options_by_record_count(0),
    )
    .expect("force rotation");
    fs::remove_file(rotation.snapshot_path.expect("snapshot path")).expect("delete snapshot");

    let recovery = recover_claim_wal(&root, false).expect("recover checkpoint without snapshot");
    assert_eq!(
        recovery.stop_reason,
        ClaimWalStopReason::CheckpointSnapshotInvalid
    );
    assert_eq!(recovery.records.len(), 0);
    assert_eq!(recovery.last_good_offset, 0);

    let append = append_claim_wal_record(
        &root,
        ClaimWalOperation::Heartbeat,
        &claim("S1", "alice", "src/lib.rs", ClaimStatus::Active),
        "2027-01-15T08:02:00Z",
    );
    assert!(
        append.is_err(),
        "append must fail closed after snapshot loss"
    );
}

#[test]
fn claim_wal_checkpoint_snapshot_crc_mismatch_fails_closed() {
    let root = temp_state("rotation-bad-snapshot-crc");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &claim("S1", "alice", "src/lib.rs", ClaimStatus::Active),
        "2027-01-15T08:00:00Z",
    )
    .expect("append acquire");
    let rotation = rotate_claim_wal_if_needed(
        &root,
        "2027-01-15T08:01:00Z",
        &rotation_options_by_record_count(0),
    )
    .expect("force rotation");
    let snapshot_path = rotation.snapshot_path.expect("snapshot path");
    let mut snapshot = fs::read(&snapshot_path).expect("read snapshot");
    let last = snapshot.last_mut().expect("non-empty snapshot");
    *last ^= 0b0000_0001;
    fs::write(&snapshot_path, snapshot).expect("write corrupted snapshot");

    let recovery = recover_claim_wal(&root, false).expect("recover checkpoint with bad snapshot");
    assert_eq!(
        recovery.stop_reason,
        ClaimWalStopReason::CheckpointSnapshotInvalid
    );

    let projection = replay_claim_wal(&root, false).expect("replay corrupted snapshot prefix");
    assert_eq!(
        projection.recovery.stop_reason,
        ClaimWalStopReason::CheckpointSnapshotInvalid
    );
    assert!(projection.claims.is_empty());
}
