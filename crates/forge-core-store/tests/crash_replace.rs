use forge_core_store::crash_replace::{
    recover_file_crash_safe_under_lock, replace_file_crash_safe_under_lock,
    replace_file_crash_safe_under_lock_with_fault, CrashReplaceError, CrashReplacePhase,
    CrashReplaceRecoveryAction,
};
use forge_core_store::{acquire_effect_store_lock, sha256_content_hash};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const LOCK: &str = "locks/domain-packs.lifecycle.lock";
const TARGET: &str = "packs/active.lock.yaml";
const MAX_BYTES: u64 = 64 * 1024;

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-crash-replace-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create temp root");
    root
}

fn target(root: &Path) -> PathBuf {
    root.join(TARGET)
}

fn sidecars(root: &Path) -> [PathBuf; 3] {
    let parent = target(root).parent().expect("target parent").to_path_buf();
    [
        parent.join(".active.lock.yaml.forge-next"),
        parent.join(".active.lock.yaml.forge-previous"),
        parent.join(".active.lock.yaml.forge-transaction"),
    ]
}

fn assert_no_sidecars(root: &Path) {
    for sidecar in sidecars(root) {
        assert!(
            fs::symlink_metadata(&sidecar).is_err(),
            "protocol sidecar must be cleaned: {}",
            sidecar.display()
        );
    }
}

#[test]
fn fresh_replace_is_digest_bound_and_cleans_protocol_artifacts() {
    let root = temp_root("fresh");
    let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
    let content = b"revision: 1\n";

    let result =
        replace_file_crash_safe_under_lock(&root, &lock, LOCK, TARGET, None, content, MAX_BYTES)
            .expect("fresh replacement");

    assert_eq!(result.previous_digest, None);
    assert_eq!(result.installed_digest, sha256_content_hash(content));
    assert_eq!(fs::read(target(&root)).expect("active bytes"), content);
    assert_no_sidecars(&root);

    let recovery = recover_file_crash_safe_under_lock(&root, &lock, LOCK, TARGET, MAX_BYTES)
        .expect("idempotent recovery");
    assert_eq!(recovery.action, CrashReplaceRecoveryAction::Noop);
    assert_eq!(recovery.target_digest, Some(sha256_content_hash(content)));
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn every_replacement_phase_recovers_exact_old_or_new_bytes() {
    let old = b"revision: 1\n";
    let new = b"revision: 2\n";
    for phase in [
        CrashReplacePhase::NextSynced,
        CrashReplacePhase::TransactionSynced,
        CrashReplacePhase::PreviousInstalled,
        CrashReplacePhase::TargetInstalled,
    ] {
        let root = temp_root(&format!("phase-{phase:?}"));
        let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
        replace_file_crash_safe_under_lock(&root, &lock, LOCK, TARGET, None, old, MAX_BYTES)
            .expect("install old");
        let old_digest = sha256_content_hash(old);

        let error = replace_file_crash_safe_under_lock_with_fault(
            &root,
            &lock,
            LOCK,
            TARGET,
            Some(&old_digest),
            new,
            MAX_BYTES,
            Some(phase),
        )
        .expect_err("fault must interrupt replacement");
        assert_eq!(error, CrashReplaceError::InjectedFault { phase });

        let recovery = recover_file_crash_safe_under_lock(&root, &lock, LOCK, TARGET, MAX_BYTES)
            .expect("recover interrupted replacement");
        let bytes = fs::read(target(&root)).expect("recovered active bytes");
        assert!(
            bytes == old || bytes == new,
            "phase {phase:?} recovered neither exact old nor exact new bytes"
        );
        if phase == CrashReplacePhase::TargetInstalled {
            assert_eq!(bytes, new, "installed target is the commit point");
            assert_eq!(
                recovery.action,
                CrashReplaceRecoveryAction::CleanedCommitted
            );
        } else {
            assert_eq!(bytes, old, "pre-commit failure must preserve old bytes");
        }
        assert_no_sidecars(&root);

        let second = recover_file_crash_safe_under_lock(&root, &lock, LOCK, TARGET, MAX_BYTES)
            .expect("recovery is idempotent");
        assert_eq!(second.action, CrashReplaceRecoveryAction::Noop);
        drop(lock);
        fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn initial_transaction_after_durable_marker_is_completed_by_recovery() {
    let root = temp_root("initial-recovery");
    let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
    let content = b"revision: 1\n";
    replace_file_crash_safe_under_lock_with_fault(
        &root,
        &lock,
        LOCK,
        TARGET,
        None,
        content,
        MAX_BYTES,
        Some(CrashReplacePhase::TransactionSynced),
    )
    .expect_err("fault after initial marker");

    let recovery = recover_file_crash_safe_under_lock(&root, &lock, LOCK, TARGET, MAX_BYTES)
        .expect("finish marker-bound initial transaction");
    assert_eq!(
        recovery.action,
        CrashReplaceRecoveryAction::CommittedInitial
    );
    assert_eq!(fs::read(target(&root)).expect("active bytes"), content);
    assert_no_sidecars(&root);
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn stale_cas_and_wrong_lock_scope_fail_before_replacement() {
    let root = temp_root("cas-lock");
    let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
    let old = b"revision: 1\n";
    replace_file_crash_safe_under_lock(&root, &lock, LOCK, TARGET, None, old, MAX_BYTES)
        .expect("install old");

    let stale = sha256_content_hash(b"not-current");
    let error = replace_file_crash_safe_under_lock(
        &root,
        &lock,
        LOCK,
        TARGET,
        Some(&stale),
        b"revision: 2\n",
        MAX_BYTES,
    )
    .expect_err("stale CAS must fail");
    assert!(matches!(
        error,
        CrashReplaceError::CompareAndSwapMismatch { .. }
    ));
    assert_eq!(fs::read(target(&root)).expect("old remains"), old);
    assert_no_sidecars(&root);

    let other_lock_path = "locks/other.lock";
    let error =
        recover_file_crash_safe_under_lock(&root, &lock, other_lock_path, TARGET, MAX_BYTES)
            .expect_err("wrong retained lock must fail");
    assert!(matches!(error, CrashReplaceError::LockScopeMismatch { .. }));
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn oversized_and_non_regular_protocol_state_fail_closed() {
    let root = temp_root("bounds-regular");
    let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
    let oversized = vec![b'x'; 17];
    let error =
        replace_file_crash_safe_under_lock(&root, &lock, LOCK, TARGET, None, &oversized, 16)
            .expect_err("oversized content must fail");
    assert!(matches!(error, CrashReplaceError::SizeLimit { .. }));
    assert!(!target(&root).exists());

    fs::create_dir_all(target(&root).parent().expect("target parent")).expect("target parent");
    fs::create_dir(sidecars(&root)[2].clone()).expect("transaction sidecar directory");
    let error = recover_file_crash_safe_under_lock(&root, &lock, LOCK, TARGET, MAX_BYTES)
        .expect_err("non-regular marker must fail closed");
    assert!(matches!(error, CrashReplaceError::Protocol { .. }));
    assert!(!target(&root).exists());
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}
