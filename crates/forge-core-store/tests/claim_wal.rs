use forge_core_contracts::claim::{
    ActorRole, ClaimContract, ClaimIdentity, ClaimKind, ClaimLease, ClaimScope, ClaimScopeKind,
    ClaimStatus, ClaimStatusRecord, ExpiryAction, ExpiryPolicy, ReclaimPolicy,
};
use forge_core_contracts::{ClaimId, RepoPath, ScopeId, StableId};
use forge_core_store::claim_wal::{
    append_claim_wal_record, claim_wal_path, recover_claim_wal, replay_claim_wal,
    ClaimWalOperation, ClaimWalStopReason,
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
