//! Authority-invariant tests for `derive_state` — the sole claim-state
//! constructor.
//!
//! These cover the spec acceptance criteria that the WAL replay is the only
//! authority path and that tampering fails closed rather than honoring a forged
//! lease:
//! - **ac1** a hand-edited (CRC-tampered) payload stops recovery at the
//!   tampered record; the forged lease in it is never honored.
//! - **ac4** a single-bit flip in a payload body stops at checksum failure.
//!
//! They sit at the store layer (pure WAL + `derive_state`, no CLI) and mirror
//! the helpers in `claim_wal.rs` exactly so the byte-level framing stays in
//! lockstep with the canonical WAL tests.

use forge_core_contracts::claim::{
    ActorRole, ClaimContract, ClaimIdentity, ClaimKind, ClaimLease, ClaimScope, ClaimScopeKind,
    ClaimStatus, ClaimStatusRecord, ExpiryAction, ExpiryPolicy, ReclaimPolicy,
};
use forge_core_contracts::{ClaimId, RepoPath, ScopeId, StableId};
use forge_core_store::claim_wal::{
    append_claim_wal_record, claim_wal_path, recover_claim_wal, ClaimWalOperation,
    ClaimWalStopReason,
};
use forge_core_store::derive_state::{derive_state, DeriveStateError};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const HEADER_LEN: usize = 24;

fn temp_state(test_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-claims-wal-recovery-{test_name}-{}-{nanos}",
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
            claimant_principal_id: None,
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

/// Flip the trailing 4-byte payload CRC of the record at `offset` so the
/// stored CRC no longer matches the (untouched) payload. Mirrors the
/// `rewrite_payload_crc` byte math from `claim_wal.rs`, but writes a
/// deliberately wrong CRC instead of recomputing it.
fn corrupt_payload_crc(bytes: &mut [u8], offset: usize) {
    let payload_len = u32::from_le_bytes(
        bytes[offset + 16..offset + 20]
            .try_into()
            .expect("payload len bytes"),
    ) as usize;
    let payload_end = offset + HEADER_LEN + payload_len;
    // Invert one CRC byte so the trailing 4 bytes can't match crc32c(payload).
    bytes[payload_end] ^= 0b0000_0001;
}

/// Flip a single bit inside the payload body (not the CRC) of the record at
/// `offset`. The stored CRC stays the same, so the recomputed CRC over the
/// now-altered payload no longer matches → checksum failure.
fn flip_single_payload_bit(bytes: &mut [u8], offset: usize) {
    let payload_start = offset + HEADER_LEN;
    bytes[payload_start] ^= 0b0000_0001;
}

/// Byte offset of the second record, recovered from a clean two-record WAL.
fn second_record_offset(root: &PathBuf) -> usize {
    let recovery = recover_claim_wal(root, false).expect("recover clean WAL");
    usize::try_from(recovery.records[1].offset).expect("record offset fits usize")
}

#[test]
fn tampered_payload_is_ignored_and_derive_state_errors() {
    // AC1: a forged lease planted by flipping the second record's payload CRC
    // is never honored — derive_state refuses to return a partial projection
    // that includes the tampered record.
    let root = temp_state("tampered-payload");
    let first = claim("S1", "alice", "src/lib.rs", ClaimStatus::Active);
    let second = claim("S2", "bob", "src/other.rs", ClaimStatus::Active);
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &first,
        "2027-01-15T08:00:00Z",
    )
    .expect("append first claim WAL record");
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &second,
        "2027-01-15T08:01:00Z",
    )
    .expect("append second claim WAL record");

    let path = claim_wal_path(&root);
    let second_offset = second_record_offset(&root);
    let mut bytes = fs::read(&path).expect("read WAL");
    corrupt_payload_crc(&mut bytes, second_offset);
    fs::write(&path, bytes).expect("write tampered WAL");

    let error = derive_state(&root).expect_err("derive_state must error on tampered payload");
    match error {
        DeriveStateError::RecoveryStopped {
            stop_reason,
            last_good_offset,
            ..
        } => {
            assert_eq!(
                stop_reason,
                ClaimWalStopReason::PayloadChecksumMismatch,
                "tampered payload must surface as a checksum mismatch"
            );
            assert_eq!(
                last_good_offset,
                u64::try_from(second_offset).expect("offset fits u64"),
                "recovery stopped exactly at the tampered record"
            );
        }
        other => panic!(
            "expected RecoveryStopped(PayloadChecksumMismatch), got {other:?} \
             — derive_state honored a tampered prefix instead of failing closed"
        ),
    }
}

#[test]
fn single_bit_flip_in_payload_stops_at_checksum_failure() {
    // AC4: a single-bit flip inside the payload body breaks its CRC coverage;
    // derive_state stops at the checksum failure rather than trusting the byte.
    let root = temp_state("single-bit-flip");
    let active = claim("S1", "alice", "src/lib.rs", ClaimStatus::Active);
    append_claim_wal_record(
        &root,
        ClaimWalOperation::Acquire,
        &active,
        "2027-01-15T08:00:00Z",
    )
    .expect("append claim WAL record");

    let path = claim_wal_path(&root);
    let mut bytes = fs::read(&path).expect("read WAL");
    flip_single_payload_bit(&mut bytes, 0);
    fs::write(&path, bytes).expect("write bit-flipped WAL");

    let error = derive_state(&root).expect_err("derive_state must error on bit-flipped payload");
    match error {
        DeriveStateError::RecoveryStopped { stop_reason, .. } => {
            assert_eq!(
                stop_reason,
                ClaimWalStopReason::PayloadChecksumMismatch,
                "a single payload bit flip must surface as a checksum mismatch"
            );
        }
        other => panic!("expected RecoveryStopped(PayloadChecksumMismatch), got {other:?}"),
    }
}

#[test]
fn clean_wal_projects_all_records() {
    // Baseline sanity: a clean WAL of 2 acquires + 1 release projects to two
    // claims, one active and one released. Guards against the authority tests
    // passing trivially because projection itself is broken.
    let root = temp_state("clean-wal");
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

    let projection = derive_state(&root).expect("derive_state on clean WAL");
    assert_eq!(
        projection.recovery.stop_reason,
        ClaimWalStopReason::CleanEof,
        "clean WAL must replay to clean EOF"
    );
    assert_eq!(projection.claims.len(), 2, "two distinct claims projected");
    assert!(
        projection
            .active_by_claim_id
            .contains_key("claim.story.S1.S1"),
        "S1 must be active"
    );
    assert!(
        projection
            .released_by_claim_id
            .contains_key("claim.story.S2.S2"),
        "S2 must be released"
    );
    assert!(
        !projection
            .active_by_claim_id
            .contains_key("claim.story.S2.S2"),
        "released S2 must not appear active"
    );
}
