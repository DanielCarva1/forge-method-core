use forge_core_contracts::PrincipalId;
use forge_core_store::replay_wal::{
    acquire_replay_commit_guard, consume_replay_nonce_non_boundary, initialize_replay_wal,
    recover_replay_wal, replay_nonce_key_hash, replay_wal_manifest_path, replay_wal_path,
    reserve_replay_nonce as reserve_replay_nonce_bound, ReplayConsumeResult,
    ReplayReservationState, ReplayReserveResult, ReplayWalCapacityKind, ReplayWalError,
    ReplayWalStopReason, REPLAY_WAL_MAX_BYTES,
};
use forge_core_store::{acquire_effect_store_lock, try_acquire_effect_store_lock};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Barrier};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

const AUDIENCE: &str = "forge://workspace/project-alpha";

fn principal() -> PrincipalId {
    PrincipalId("principal.codex-main".to_owned())
}

fn digest(hex_digit: char) -> String {
    format!("sha256:{}", hex_digit.to_string().repeat(64))
}

fn commit_digest() -> String {
    digest('0')
}

fn reserve_replay_nonce(
    state_root: impl AsRef<Path>,
    principal_id: &PrincipalId,
    audience: &str,
    nonce: &str,
    intent_digest: &str,
) -> Result<ReplayReserveResult, ReplayWalError> {
    reserve_replay_nonce_bound(
        state_root,
        principal_id,
        audience,
        nonce,
        intent_digest,
        &commit_digest(),
    )
}

fn consume_replay_nonce(
    state_root: impl AsRef<Path>,
    principal_id: &PrincipalId,
    audience: &str,
    nonce: &str,
    intent_digest: &str,
    expected_revision: u64,
) -> Result<ReplayConsumeResult, ReplayWalError> {
    consume_replay_nonce_non_boundary(
        state_root,
        principal_id,
        audience,
        nonce,
        intent_digest,
        &commit_digest(),
        expected_revision,
    )
}

fn temp_root(test_name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-replay-wal-{test_name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create test root");
    root
}

#[test]
fn key_hash_is_deterministic_domain_separated_and_unambiguous() {
    let principal = principal();
    let first = replay_nonce_key_hash(&principal, AUDIENCE, "nonce-001").expect("hash key");
    let repeated = replay_nonce_key_hash(&principal, AUDIENCE, "nonce-001").expect("hash key");
    let other_nonce = replay_nonce_key_hash(&principal, AUDIENCE, "nonce-002").expect("hash key");
    let split_left = replay_nonce_key_hash(&PrincipalId("ab".into()), "c", "d").expect("hash");
    let split_right = replay_nonce_key_hash(&PrincipalId("a".into()), "bc", "d").expect("hash");

    assert_eq!(first, repeated);
    assert_eq!(
        first, "sha256:b28e4c59371dba134e88afe19509ca4ea082336dc9c7be94932762537c51ed2f",
        "persisted key derivation is a compatibility contract"
    );
    assert_ne!(first, other_nonce);
    assert_ne!(
        split_left, split_right,
        "length prefixes bind component boundaries"
    );
    assert!(first.starts_with("sha256:"));
    assert_eq!(first.len(), 71);
}

#[test]
fn first_reservation_is_revision_one_and_recovers() {
    let root = temp_root("reserve-recover");
    initialize_replay_wal(&root).expect("initialize replay WAL");
    let digest = digest('a');

    let result = reserve_replay_nonce(&root, &principal(), AUDIENCE, "nonce-reserve-001", &digest)
        .expect("reserve nonce");

    assert_eq!(result.seq, 1);
    assert!(result.bytes_appended > 0);
    assert_eq!(result.reservation.revision, 1);
    assert_eq!(result.reservation.state, ReplayReservationState::Reserved);

    let recovery = recover_replay_wal(&root, false).expect("recover WAL");
    assert!(recovery.is_clean());
    assert_eq!(recovery.last_observed_seq, 1);
    assert_eq!(recovery.valid_record_count, 1);
    assert_eq!(recovery.reservations.len(), 1);
    assert_eq!(
        recovery
            .reservations
            .get(&result.reservation.key_hash)
            .expect("reservation")
            .revision,
        1
    );

    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn initialization_requires_existing_root_and_durable_pair() {
    let parent = temp_root("root-required-parent");
    let missing_root = parent.join("missing");
    let error = reserve_replay_nonce_bound(
        &missing_root,
        &principal(),
        AUDIENCE,
        "nonce-missing-root",
        &digest('a'),
        &commit_digest(),
    )
    .expect_err("state root must preexist");
    assert!(matches!(error, ReplayWalError::StateRootUnavailable { .. }));
    assert!(
        !missing_root.exists(),
        "replay code must not create state root"
    );

    let root = temp_root("initialize-pair");
    let first = initialize_replay_wal(&root).expect("initialize pair");
    assert!(first.initialized);
    let canonical_root = fs::canonicalize(&root).expect("canonical test root");
    assert_eq!(first.manifest_path.parent(), Some(canonical_root.as_path()));
    assert!(first.wal_path.starts_with(canonical_root.join("wal")));
    assert!(first.manifest_path.exists());
    assert!(first.wal_path.exists());
    let second = initialize_replay_wal(&root).expect("initialization is idempotent");
    assert!(!second.initialized);

    fs::remove_dir_all(parent).expect("clean parent root");
    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn manifest_without_wal_fails_closed() {
    let root = temp_root("manifest-without-wal");
    initialize_replay_wal(&root).expect("initialize pair");
    fs::remove_file(replay_wal_path(&root)).expect("remove WAL only");

    let error = recover_replay_wal(&root, true).expect_err("missing WAL must fail closed");
    assert!(matches!(
        error,
        ReplayWalError::InitializationMismatch {
            manifest_exists: true,
            wal_exists: false,
            ..
        }
    ));
    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn wal_without_manifest_fails_closed() {
    let root = temp_root("wal-without-manifest");
    initialize_replay_wal(&root).expect("initialize pair");
    fs::remove_file(replay_wal_manifest_path(&root)).expect("remove manifest only");

    let error = reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-after-marker-loss",
        &digest('a'),
    )
    .expect_err("missing manifest must fail closed");
    assert!(matches!(
        error,
        ReplayWalError::InitializationMismatch {
            manifest_exists: false,
            wal_exists: true,
            ..
        }
    ));
    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn reservation_does_not_reinitialize_a_wholly_deleted_pair() {
    let root = temp_root("deleted-initialization-pair");
    initialize_replay_wal(&root).expect("initialize replay authority");
    fs::remove_file(replay_wal_path(&root)).expect("remove replay WAL");
    fs::remove_file(replay_wal_manifest_path(&root)).expect("remove replay manifest");

    let error = reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-before-initialization",
        &digest('a'),
    )
    .expect_err("runtime reservation must not recreate replay authority");

    assert!(matches!(error, ReplayWalError::NotInitialized { .. }));
    assert!(!replay_wal_path(&root).exists());
    assert!(!replay_wal_manifest_path(&root).exists());
    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn corrupt_manifest_is_not_treated_as_initialization() {
    let root = temp_root("corrupt-manifest");
    initialize_replay_wal(&root).expect("initialize pair");
    fs::write(replay_wal_manifest_path(&root), b"{}").expect("corrupt manifest");

    let error = recover_replay_wal(&root, false).expect_err("invalid manifest must fail closed");
    assert!(matches!(error, ReplayWalError::InvalidManifest { .. }));
    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn duplicate_nonce_is_permanently_rejected_even_for_same_binding() {
    let root = temp_root("duplicate");
    initialize_replay_wal(&root).expect("initialize replay WAL");
    let digest = digest('b');
    reserve_replay_nonce(&root, &principal(), AUDIENCE, "nonce-duplicate", &digest)
        .expect("initial reservation");

    let duplicate = reserve_replay_nonce(&root, &principal(), AUDIENCE, "nonce-duplicate", &digest)
        .expect_err("duplicate must fail");
    assert!(matches!(
        duplicate,
        ReplayWalError::DuplicateNonce {
            revision: 1,
            state: ReplayReservationState::Reserved,
            ..
        }
    ));

    consume_replay_nonce(&root, &principal(), AUDIENCE, "nonce-duplicate", &digest, 1)
        .expect("consume reservation");
    let after_consume =
        reserve_replay_nonce(&root, &principal(), AUDIENCE, "nonce-duplicate", &digest)
            .expect_err("consumed nonce remains rejected");
    assert!(matches!(
        after_consume,
        ReplayWalError::DuplicateNonce {
            revision: 2,
            state: ReplayReservationState::Consumed,
            ..
        }
    ));

    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn consume_is_cas_transition_and_exact_retry_is_idempotent() {
    let root = temp_root("consume-idempotent");
    initialize_replay_wal(&root).expect("initialize replay WAL");
    let intent_digest = digest('c');
    reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-consume",
        &intent_digest,
    )
    .expect("reserve nonce");

    let first = consume_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-consume",
        &intent_digest,
        1,
    )
    .expect("consume nonce");
    assert!(first.appended);
    assert!(first.bytes_appended > 0);
    assert_eq!(first.seq, 2);
    assert_eq!(first.reservation.revision, 2);
    assert_eq!(first.reservation.state, ReplayReservationState::Consumed);

    let mismatched_commit = consume_replay_nonce_non_boundary(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-consume",
        &intent_digest,
        &digest('e'),
        1,
    )
    .expect_err("idempotence requires the exact commit binding");
    assert!(matches!(
        mismatched_commit,
        ReplayWalError::CommitDigestMismatch { .. }
    ));

    let retry = consume_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-consume",
        &intent_digest,
        1,
    )
    .expect("same consume retry is idempotent");
    assert!(!retry.appended);
    assert_eq!(retry.bytes_appended, 0);
    assert_eq!(retry.seq, first.seq);
    assert_eq!(retry.reservation, first.reservation);

    let recovery = recover_replay_wal(&root, false).expect("recover WAL");
    assert_eq!(recovery.valid_record_count, 2, "retry must not append");
    assert_eq!(recovery.last_observed_seq, 2);

    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn consume_rejects_missing_wrong_digest_and_wrong_revision() {
    let root = temp_root("consume-errors");
    let intent_digest = digest('d');
    initialize_replay_wal(&root).expect("initialize replay WAL");

    let missing = consume_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-missing",
        &intent_digest,
        1,
    )
    .expect_err("missing reservation must fail");
    assert!(matches!(missing, ReplayWalError::ReservationMissing { .. }));

    reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-errors",
        &intent_digest,
    )
    .expect("reserve nonce");
    let wrong_digest = consume_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-errors",
        &digest('e'),
        1,
    )
    .expect_err("wrong digest must fail");
    assert!(matches!(
        wrong_digest,
        ReplayWalError::IntentDigestMismatch { .. }
    ));

    let wrong_commit = consume_replay_nonce_non_boundary(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-errors",
        &intent_digest,
        &digest('f'),
        1,
    )
    .expect_err("wrong commit digest must fail");
    assert!(matches!(
        wrong_commit,
        ReplayWalError::CommitDigestMismatch { .. }
    ));

    let wrong_revision = consume_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-errors",
        &intent_digest,
        2,
    )
    .expect_err("wrong CAS revision must fail");
    assert!(matches!(
        wrong_revision,
        ReplayWalError::RevisionMismatch {
            expected: 2,
            actual: 1,
            ..
        }
    ));

    let recovery = recover_replay_wal(&root, false).expect("recover WAL");
    assert_eq!(recovery.valid_record_count, 1, "rejections must not append");
    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn raw_key_material_is_not_persisted() {
    let root = temp_root("privacy");
    initialize_replay_wal(&root).expect("initialize replay WAL");
    let principal = PrincipalId("principal.private-value".to_owned());
    let audience = "forge://private-audience";
    let nonce = "raw-super-secret-replay-nonce";
    reserve_replay_nonce(&root, &principal, audience, nonce, &digest('f')).expect("reserve nonce");

    let bytes = fs::read(replay_wal_path(&root)).expect("read WAL bytes");
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains(&principal.0));
    assert!(!text.contains(audience));
    assert!(!text.contains(nonce));

    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn incomplete_tail_is_reported_then_safely_repaired_before_append() {
    let root = temp_root("torn-tail");
    initialize_replay_wal(&root).expect("initialize replay WAL");
    reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-before-tail",
        &digest('1'),
    )
    .expect("reserve first nonce");
    let wal_path = replay_wal_path(&root);
    let good_len = fs::metadata(&wal_path).expect("WAL metadata").len();
    OpenOptions::new()
        .append(true)
        .open(&wal_path)
        .expect("open WAL")
        .write_all(b"FM")
        .expect("append incomplete header");

    let diagnostic = recover_replay_wal(&root, false).expect("diagnose torn WAL");
    assert!(!diagnostic.is_clean());
    assert_eq!(diagnostic.stop_reason, ReplayWalStopReason::TruncatedHeader);
    assert_eq!(diagnostic.last_good_offset, good_len);
    assert!(!diagnostic.repaired);

    let second = reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-after-tail",
        &digest('2'),
    )
    .expect("append path repairs safe tail");
    assert_eq!(second.seq, 2);
    let recovered = recover_replay_wal(&root, false).expect("recover repaired WAL");
    assert!(recovered.is_clean());
    assert_eq!(recovered.valid_record_count, 2);
    assert_eq!(recovered.last_observed_seq, 2);

    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn incomplete_payload_repair_discards_only_the_uncommitted_final_frame() {
    let root = temp_root("torn-payload");
    initialize_replay_wal(&root).expect("initialize replay WAL");
    reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-complete-frame",
        &digest('a'),
    )
    .expect("reserve first nonce");
    let first_len = fs::metadata(replay_wal_path(&root))
        .expect("first WAL metadata")
        .len();
    reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-torn-frame",
        &digest('b'),
    )
    .expect("reserve second nonce");
    let wal_path = replay_wal_path(&root);
    let full_len = fs::metadata(&wal_path).expect("full WAL metadata").len();
    let file = OpenOptions::new()
        .write(true)
        .open(&wal_path)
        .expect("open WAL for truncation");
    file.set_len(full_len - 3).expect("truncate final frame");

    let diagnostic = recover_replay_wal(&root, false).expect("diagnose torn payload");
    assert_eq!(
        diagnostic.stop_reason,
        ReplayWalStopReason::TruncatedPayload
    );
    assert_eq!(diagnostic.last_good_offset, first_len);
    let repaired = recover_replay_wal(&root, true).expect("repair torn payload");
    assert!(repaired.is_clean());
    assert!(repaired.repaired);
    assert_eq!(
        fs::metadata(&wal_path).expect("WAL metadata").len(),
        first_len
    );

    let replacement = reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-torn-frame",
        &digest('b'),
    )
    .expect("incomplete reservation was never durable");
    assert_eq!(replacement.seq, 2);

    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn valid_checksum_with_sequence_gap_still_fails_closed() {
    let root = temp_root("sequence-gap");
    initialize_replay_wal(&root).expect("initialize replay WAL");
    reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-sequence-gap",
        &digest('c'),
    )
    .expect("reserve nonce");
    let wal_path = replay_wal_path(&root);
    let mut bytes = fs::read(&wal_path).expect("read WAL");
    rewrite_first_frame_sequence(&mut bytes, 2);
    fs::write(&wal_path, &bytes).expect("write sequence gap with valid checksums");

    let recovery = recover_replay_wal(&root, true).expect("inspect sequence gap");
    assert!(!recovery.is_clean());
    assert!(!recovery.repaired);
    assert_eq!(recovery.stop_reason, ReplayWalStopReason::SequenceGap);
    let blocked = reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-after-sequence-gap",
        &digest('d'),
    )
    .expect_err("sequence gap blocks append");
    assert!(matches!(
        blocked,
        ReplayWalError::RecoveryStopped {
            stop_reason: ReplayWalStopReason::SequenceGap,
            ..
        }
    ));

    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn checksum_corruption_fails_closed_and_is_never_repaired() {
    let root = temp_root("corrupt-checksum");
    initialize_replay_wal(&root).expect("initialize replay WAL");
    reserve_replay_nonce(&root, &principal(), AUDIENCE, "nonce-corrupt", &digest('3'))
        .expect("reserve nonce");
    let wal_path = replay_wal_path(&root);
    let mut bytes = fs::read(&wal_path).expect("read WAL");
    let last = bytes.last_mut().expect("nonempty WAL");
    *last ^= 0xff;
    fs::write(&wal_path, &bytes).expect("corrupt checksum");
    let corrupt_len = fs::metadata(&wal_path).expect("WAL metadata").len();

    let recovery = recover_replay_wal(&root, true).expect("inspect corrupt WAL");
    assert!(!recovery.is_clean());
    assert!(!recovery.repaired);
    assert_eq!(
        recovery.stop_reason,
        ReplayWalStopReason::PayloadChecksumMismatch
    );
    assert_eq!(
        fs::metadata(&wal_path).expect("WAL metadata").len(),
        corrupt_len
    );

    let blocked = reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-after-corruption",
        &digest('4'),
    )
    .expect_err("corruption must block authority changes");
    assert!(matches!(
        blocked,
        ReplayWalError::RecoveryStopped {
            stop_reason: ReplayWalStopReason::PayloadChecksumMismatch,
            ..
        }
    ));
    assert_eq!(
        fs::metadata(&wal_path).expect("WAL metadata").len(),
        corrupt_len
    );

    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn concurrent_reservations_stress_shared_boundary_reuse_without_false_quiescing() {
    const ROUNDS: usize = 16;
    const WORKERS: usize = 16;

    let root = Arc::new(temp_root("concurrent-stress"));
    initialize_replay_wal(root.as_ref()).expect("initialize replay WAL");
    let digest = digest('5');

    for round in 0..ROUNDS {
        let barrier = Arc::new(Barrier::new(WORKERS));
        let nonce = format!("nonce-concurrent-{round}");
        let mut workers = Vec::new();
        for _ in 0..WORKERS {
            let root = Arc::clone(&root);
            let barrier = Arc::clone(&barrier);
            let digest = digest.clone();
            let nonce = nonce.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                reserve_replay_nonce(root.as_ref(), &principal(), AUDIENCE, &nonce, &digest)
            }));
        }

        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().expect("worker must not panic"))
            .collect();
        let winners = results.iter().filter(|result| result.is_ok()).count();
        let duplicates = results
            .iter()
            .filter(|result| matches!(result, Err(ReplayWalError::DuplicateNonce { .. })))
            .count();
        assert_eq!(winners, 1, "round {round} must have exactly one winner");
        assert_eq!(
            duplicates,
            WORKERS - 1,
            "round {round} returned a non-duplicate admission error: {results:?}"
        );
    }

    let recovery = recover_replay_wal(root.as_ref(), false).expect("recover WAL");
    assert_eq!(recovery.valid_record_count, ROUNDS);
    assert_eq!(recovery.reservations.len(), ROUNDS);

    fs::remove_dir_all(root.as_ref()).expect("clean test root");
}

#[test]
fn distinct_nonce_reservations_are_monotonically_sequenced() {
    let root = temp_root("sequence");
    initialize_replay_wal(&root).expect("initialize replay WAL");
    let first = reserve_replay_nonce(&root, &principal(), AUDIENCE, "nonce-seq-1", &digest('6'))
        .expect("reserve first");
    let second = reserve_replay_nonce(&root, &principal(), AUDIENCE, "nonce-seq-2", &digest('7'))
        .expect("reserve second");
    let third = reserve_replay_nonce(
        &root,
        &principal(),
        "forge://other-audience",
        "nonce-seq-1",
        &digest('8'),
    )
    .expect("audience scopes nonce identity");

    assert_eq!((first.seq, second.seq, third.seq), (1, 2, 3));
    assert_ne!(first.reservation.key_hash, third.reservation.key_hash);
    let recovery = recover_replay_wal(&root, false).expect("recover WAL");
    assert_eq!(
        recovery
            .records
            .iter()
            .map(|record| record.seq)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );

    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn oversized_wal_is_rejected_before_unbounded_read() {
    let root = temp_root("byte-capacity");
    initialize_replay_wal(&root).expect("initialize pair");
    let wal_path = replay_wal_path(&root);
    OpenOptions::new()
        .write(true)
        .open(&wal_path)
        .expect("open WAL")
        .set_len(REPLAY_WAL_MAX_BYTES + 1)
        .expect("make sparse oversized WAL");

    let error = recover_replay_wal(&root, false).expect_err("oversized WAL must fail closed");
    assert!(matches!(
        error,
        ReplayWalError::CapacityExceeded {
            kind: ReplayWalCapacityKind::Bytes,
            limit: REPLAY_WAL_MAX_BYTES,
            ..
        }
    ));
    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn commit_guard_holds_replay_lock_and_exactly_consumes_binding() {
    const EFFECT_LOCK: &str = "locks/effects/operation-alpha.lock";
    let root = temp_root("commit-guard");
    initialize_replay_wal(&root).expect("initialize replay WAL");
    let intent = digest('a');
    let commit = digest('b');
    reserve_replay_nonce_bound(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-guarded",
        &intent,
        &commit,
    )
    .expect("reserve guarded nonce");
    let effect_lock = acquire_effect_store_lock(&root, EFFECT_LOCK).expect("acquire effect lock");
    let guard = acquire_replay_commit_guard(
        &root,
        &effect_lock,
        EFFECT_LOCK,
        &principal(),
        AUDIENCE,
        "nonce-guarded",
        &intent,
        &commit,
        1,
    )
    .expect("acquire effect-first replay guard");
    assert_eq!(guard.effect_lock().path(), effect_lock.path());
    assert_eq!(guard.reservation().revision, 1);
    assert_eq!(guard.reservation().commit_digest, commit);

    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker_root = root.clone();
    let worker_intent = intent.clone();
    let worker_commit = commit.clone();
    let worker = std::thread::spawn(move || {
        started_tx.send(()).expect("signal worker start");
        let result = consume_replay_nonce_non_boundary(
            &worker_root,
            &principal(),
            AUDIENCE,
            "nonce-guarded",
            &worker_intent,
            &worker_commit,
            1,
        );
        done_tx.send(result).expect("send worker result");
    });
    started_rx.recv().expect("worker started");
    assert!(
        done_rx.recv_timeout(Duration::from_millis(150)).is_err(),
        "concurrent consume must block while guard owns replay lock"
    );

    let consumed = guard.consume().expect("consume guarded reservation");
    assert!(consumed.appended);
    assert_eq!(consumed.reservation.revision, 2);
    let retry = done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("worker unblocked after guard consume")
        .expect("exact retry succeeds idempotently");
    assert!(!retry.appended);
    assert_eq!(retry.reservation, consumed.reservation);
    worker.join().expect("worker must not panic");

    drop(effect_lock);
    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn commit_guard_rejects_wrong_effect_lock_scope_and_lock_order() {
    const EFFECT_LOCK: &str = "locks/effects/expected.lock";
    let root = temp_root("guard-scope");
    initialize_replay_wal(&root).expect("initialize replay WAL");
    let intent = digest('c');
    let commit = digest('d');
    reserve_replay_nonce_bound(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-scope",
        &intent,
        &commit,
    )
    .expect("reserve nonce");
    let effect_lock = acquire_effect_store_lock(&root, EFFECT_LOCK).expect("effect lock");
    let mismatch = acquire_replay_commit_guard(
        &root,
        &effect_lock,
        "locks/effects/wrong.lock",
        &principal(),
        AUDIENCE,
        "nonce-scope",
        &intent,
        &commit,
        1,
    )
    .expect_err("wrong effect scope must fail");
    assert!(matches!(
        mismatch,
        ReplayWalError::EffectLockScopeMismatch { .. }
    ));
    drop(effect_lock);

    let replay_as_effect = acquire_effect_store_lock(
        &root,
        forge_core_store::replay_wal::REPLAY_WAL_LOCK_RELATIVE_PATH,
    )
    .expect("acquire replay path through typed effect lock for negative test");
    let order = acquire_replay_commit_guard(
        &root,
        &replay_as_effect,
        forge_core_store::replay_wal::REPLAY_WAL_LOCK_RELATIVE_PATH,
        &principal(),
        AUDIENCE,
        "nonce-scope",
        &intent,
        &commit,
        1,
    )
    .expect_err("replay lock cannot masquerade as effect lock");
    assert!(matches!(
        order,
        ReplayWalError::EffectLockOrderViolation { .. }
    ));
    drop(replay_as_effect);
    assert!(try_acquire_effect_store_lock(&root, EFFECT_LOCK).is_ok());

    fs::remove_dir_all(root).expect("clean test root");
}

#[test]
fn blank_key_parts_and_noncanonical_digest_are_rejected_without_wal() {
    let root = temp_root("invalid-input");
    let blank_nonce = reserve_replay_nonce(&root, &principal(), AUDIENCE, " ", &digest('9'))
        .expect_err("blank nonce must fail");
    assert!(matches!(
        blank_nonce,
        ReplayWalError::InvalidInput { field: "nonce", .. }
    ));

    let invalid_digest = reserve_replay_nonce(
        &root,
        &principal(),
        AUDIENCE,
        "nonce-invalid-digest",
        "SHA256:not-canonical",
    )
    .expect_err("noncanonical digest must fail");
    assert!(matches!(
        invalid_digest,
        ReplayWalError::InvalidInput {
            field: "intent_digest",
            ..
        }
    ));
    assert!(!replay_wal_path(&root).exists());

    fs::remove_dir_all(root).expect("clean test root");
}

fn rewrite_first_frame_sequence(bytes: &mut [u8], sequence: u64) {
    const HEADER_LEN: usize = 24;
    const HEADER_CRC_OFFSET: usize = 20;
    bytes[8..16].copy_from_slice(&sequence.to_le_bytes());
    let header_crc = crc32c::crc32c(&bytes[..HEADER_CRC_OFFSET]);
    bytes[HEADER_CRC_OFFSET..HEADER_LEN].copy_from_slice(&header_crc.to_le_bytes());
    let payload_len =
        u32::from_le_bytes(bytes[16..20].try_into().expect("four-byte payload length"));
    let payload_len = usize::try_from(payload_len).expect("payload length fits usize");
    let payload_end = HEADER_LEN + payload_len;
    let mut covered = Vec::with_capacity(HEADER_CRC_OFFSET + payload_len);
    covered.extend_from_slice(&bytes[..HEADER_CRC_OFFSET]);
    covered.extend_from_slice(&bytes[HEADER_LEN..payload_end]);
    let payload_crc = crc32c::crc32c(&covered);
    bytes[payload_end..payload_end + 4].copy_from_slice(&payload_crc.to_le_bytes());
}
