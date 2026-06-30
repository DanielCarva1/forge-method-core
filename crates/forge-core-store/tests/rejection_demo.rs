//! S0.4 rejection demo — the central integrity promise, proved mechanically.
//!
//! Promise under test (DC2, story S0.4):
//!   "An out-of-authority effect is REJECTED before it touches the WAL,
//!    not merely logged after the fact."
//!
//! This is the differentiator vs the Python v2, which claimed append-only state
//! but derived state from hand-edited files. Here the WAL is the source of truth
//! and an unauthorized mutation never reaches it.
//!
//! We exercise the FULL validate-before-write path that includes WAL locking
//! (`apply_file_effect_transaction_with_wal_lock`) and assert three things:
//!   1. status == Blocked (rejected, not applied)
//!   2. the rejection carries a TYPED `reason_code`, never a panic
//!   3. the WAL file is byte-for-byte unchanged (hash before == hash after),
//!      AND the target file is untouched — i.e. state projection is unchanged
//!      by the attempt.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use forge_core_contracts::claim::ActorRole;
use forge_core_contracts::common::{RepoPath, StableId};
use forge_core_contracts::tool_effect::{
    AccessMode, ConflictCode, ConflictDetection, ConflictPolicy, EffectActor, EffectKind,
    EffectNotification, EffectRead, EffectRepair, EffectTargetKind, EffectWrite, InverseKind,
    InverseMetadata, InverseSource, RepairStrategy,
};
use forge_core_contracts::{ToolEffectContract, ToolEffectContractDocument};
use forge_core_store::{
    apply_file_effect_transaction_with_wal_lock, sha256_content_hash, EffectApplicationPayload,
    EffectApplicationReason, EffectApplicationStatus,
};

const WAL_RELATIVE_PATH: &str = ".forge-method/ledger.ndjson";
const LOCK_RELATIVE_PATH: &str = ".forge-method/.store.lock";

fn temp_repo(test_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "forge-s04-rejection-{test_name}-{}-{nanos}",
        std::process::id()
    ))
}

/// A minimal, fully-formed effect contract with a single write and a single read.
fn stale_write_effect(
    write_ref: &str,
    read_ref: &str,
    stale_read_hash: String,
) -> ToolEffectContractDocument {
    ToolEffectContractDocument {
        schema_version: "0.1".to_string(),
        tool_effect_contract: ToolEffectContract {
            id: StableId("effect.s04.rejection_demo".to_string()),
            contract_ref: RepoPath("contracts/effects/tool-effect-contract-v0.yaml".to_string()),
            effect_kind: EffectKind::ArtifactWrite,
            operation_ref: StableId("op.s04.rejection_demo".to_string()),
            actor: EffectActor {
                agent_id: StableId("codex-test".to_string()),
                role: ActorRole::Driver,
            },
            read_set: vec![EffectRead {
                target_kind: EffectTargetKind::FilePath,
                reference: read_ref.to_string(),
                expected_hash: Some(stale_read_hash),
                expected_version: None,
                required_for_plan: true,
            }],
            write_set: vec![EffectWrite {
                target_kind: EffectTargetKind::FilePath,
                reference: write_ref.to_string(),
                access_mode: AccessMode::Write,
                // a Write requires the caller to assert the hash it expects to overwrite.
                // We assert a hash for content the caller NEVER observed — stale authority.
                expected_hash: Some(sha256_content_hash(b"content-the-caller-thinks-is-there")),
                expected_version: None,
                destructive: false,
            }],
            conflict_detection: ConflictDetection {
                check_against: StableId("filesystem".to_string()),
                granularity: StableId("path".to_string()),
                conflict_codes: vec![ConflictCode::ReadTargetChanged],
                policy: ConflictPolicy::Block,
            },
            notification: EffectNotification {
                required: false,
                recipients: vec![],
                request_contract_ref: None,
            },
            repair: EffectRepair {
                strategy: RepairStrategy::None,
                automatic_repair_allowed: false,
                inverse_operation_ref: None,
                stop_if_inverse_missing: false,
                inverse: InverseMetadata {
                    kind: InverseKind::None,
                    source: InverseSource::Unavailable,
                    reference: None,
                    input_mapping_refs: vec![],
                    validation_gate_refs: vec![],
                    review_required: false,
                },
            },
        },
    }
}

fn payload(reference: &str, content: &[u8]) -> EffectApplicationPayload {
    EffectApplicationPayload {
        target_ref: reference.to_string(),
        content: content.to_vec(),
        content_hash: sha256_content_hash(content),
    }
}

/// Snapshot the WAL: its sha256 over bytes (empty file hashes the empty string)
/// plus its line count. Two equal snapshots mean the WAL was not mutated.
fn wal_snapshot(root: &Path) -> (String, usize) {
    let wal = root.join(WAL_RELATIVE_PATH);
    let bytes = fs::read(&wal).unwrap_or_default();
    let hash = sha256_content_hash(&bytes);
    let lines = bytes.iter().filter(|b| **b == b'\n').count();
    (hash, lines)
}

#[test]
fn s04_out_of_authority_write_is_rejected_before_touching_wal() {
    let root = temp_repo("stale-write");
    fs::create_dir_all(root.join(".forge-method")).expect("create root dir");

    // A target file that already exists on disk, with known content "v1".
    let target_ref = ".forge-method/artifacts/target.txt";
    if let Some(parent) = Path::new(target_ref).parent() {
        fs::create_dir_all(root.join(parent)).expect("create target parent");
    }
    fs::write(root.join(target_ref), b"v1").expect("seed target file");

    // The WAL is initialized empty (append-only log starts empty).
    fs::write(root.join(WAL_RELATIVE_PATH), b"").expect("init empty WAL");

    // ── BEFORE the attempt ───────────────────────────────────────────────
    let wal_before = wal_snapshot(&root);
    let target_hash_before = sha256_content_hash(&fs::read(root.join(target_ref)).unwrap());

    // The effect claims the target's current content hashes to something it has
    // NEVER actually read (a stale/foreign authority). This is exactly an
    // out-of-authority write: the caller has no fresh authority over this state.
    let effect = stale_write_effect(target_ref, target_ref, sha256_content_hash(b"original"));

    // ── THE ATTEMPT: full validate-before-write path WITH WAL lock ───────
    let result = apply_file_effect_transaction_with_wal_lock(
        &root,
        &effect,
        &[payload(target_ref, b"OVERWRITE-ATTEMPT")],
        WAL_RELATIVE_PATH,
        LOCK_RELATIVE_PATH,
        "tx-s04-stale-write",
    );

    // ── ASSERTION 1: rejected, not applied ───────────────────────────────
    assert_eq!(
        result.status,
        EffectApplicationStatus::Blocked,
        "out-of-authority write must be Blocked, got {:?}",
        result.status
    );

    // ── ASSERTION 2: typed reason_code, never a panic ────────────────────
    // The freshness check failed: the caller's asserted hash did not match the
    // real on-disk content. This is the typed rejection, not a panic/crash.
    assert!(
        result
            .reasons
            .contains(&EffectApplicationReason::ExpectedHashMismatch),
        "expected a typed ExpectedHashMismatch reason, got reasons = {:?}",
        result.reasons
    );

    // ── ASSERTION 3a: target file untouched ──────────────────────────────
    let target_after = fs::read(root.join(target_ref)).expect("target still exists");
    assert_eq!(
        target_after, b"v1",
        "target must be unchanged; the overwrite never landed"
    );
    assert_eq!(
        sha256_content_hash(&target_after),
        target_hash_before,
        "target hash must equal the pre-attempt hash"
    );

    // ── ASSERTION 3b: WAL byte-for-byte unchanged (the core promise) ─────
    let wal_after = wal_snapshot(&root);
    assert_eq!(
        wal_before, wal_after,
        "WAL must be unchanged by the rejected attempt (before={wal_before:?} after={wal_after:?})"
    );

    fs::remove_dir_all(&root).ok();
}
