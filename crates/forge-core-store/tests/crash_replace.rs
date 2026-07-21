use forge_core_store::crash_replace::{
    CrashReplaceError, CrashReplaceRecovery, CrashReplaceRecoveryAction,
};
use forge_core_store::retained_crash_replace::{
    reconcile_file_crash_safe_under_owned_lock, reconcile_file_crash_safe_under_retained_lock,
    recover_file_crash_safe_under_retained_lock, replace_file_crash_safe_under_retained_lock,
    retain_file_crash_safe_expected_leaf_under_retained_lock,
};
use forge_core_store::{
    acquire_effect_store_lock, sha256_content_hash, EffectStoreLock,
    RetainedEffectStoreExpectedLeaf, RetainedEffectStoreLeafWitness,
};
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

fn retain_expected(
    lock: &EffectStoreLock,
    maximum: u64,
) -> Result<RetainedEffectStoreExpectedLeaf<'_>, CrashReplaceError> {
    retain_file_crash_safe_expected_leaf_under_retained_lock(lock, Path::new(TARGET), maximum)
}

fn replace_retained<'lock>(
    lock: &'lock EffectStoreLock,
    expected: &mut RetainedEffectStoreExpectedLeaf<'lock>,
    content: &[u8],
    maximum: u64,
) -> Result<RetainedEffectStoreLeafWitness<'lock>, CrashReplaceError> {
    replace_file_crash_safe_under_retained_lock(lock, Path::new(TARGET), expected, content, maximum)
}

fn recover_retained(
    lock: &EffectStoreLock,
    maximum: u64,
) -> Result<CrashReplaceRecovery, CrashReplaceError> {
    recover_file_crash_safe_under_retained_lock(lock, Path::new(TARGET), maximum)
}

#[test]
fn fresh_replace_is_digest_bound_and_cleans_protocol_artifacts() {
    let root = temp_root("fresh");
    let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
    let content = b"revision: 1\n";
    let mut expected = retain_expected(&lock, MAX_BYTES).expect("retain exact absence");
    assert!(expected.digest().is_none());

    let result =
        replace_retained(&lock, &mut expected, content, MAX_BYTES).expect("fresh replacement");

    assert_eq!(result.digest(), sha256_content_hash(content));
    assert_eq!(result.raw_bytes(), content);
    assert_eq!(fs::read(target(&root)).expect("active bytes"), content);
    assert_no_sidecars(&root);

    let recovery = recover_retained(&lock, MAX_BYTES).expect("idempotent recovery");
    assert_eq!(recovery.action, CrashReplaceRecoveryAction::Noop);
    assert_eq!(recovery.target_digest, Some(sha256_content_hash(content)));
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn exact_predecessor_and_reserved_target_authority_fail_closed() {
    let root = temp_root("exact-predecessor");
    let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
    let old = b"revision: 1\n";
    let mut absent = retain_expected(&lock, MAX_BYTES).expect("retain exact absence");
    let installed = replace_retained(&lock, &mut absent, old, MAX_BYTES).expect("install old");

    fs::remove_file(target(&root)).expect("remove exact predecessor name");
    fs::write(target(&root), old).expect("install same-digest substitute");
    let mut substituted = RetainedEffectStoreExpectedLeaf::Present(installed);
    let error = replace_retained(&lock, &mut substituted, b"revision: 2\n", MAX_BYTES)
        .expect_err("same-digest substitute must not satisfy exact predecessor authority");
    assert!(matches!(error, CrashReplaceError::Io { .. }));
    assert_eq!(fs::read(target(&root)).expect("substitute remains"), old);
    assert_no_sidecars(&root);

    let error = recover_file_crash_safe_under_retained_lock(
        &lock,
        Path::new("memory/events.ndjson"),
        MAX_BYTES,
    )
    .expect_err("EventLog target authority must not be minted");
    assert!(matches!(error, CrashReplaceError::ReservedStatePath { .. }));
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn exact_absence_authority_rejects_a_late_leaf() {
    let root = temp_root("exact-absence");
    let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
    let mut absent = retain_expected(&lock, MAX_BYTES).expect("retain exact absence");
    fs::create_dir_all(target(&root).parent().expect("target parent")).expect("target parent");
    fs::write(target(&root), b"late: true\n").expect("install late leaf");

    let error = replace_retained(&lock, &mut absent, b"revision: 1\n", MAX_BYTES)
        .expect_err("late leaf must invalidate exact absence authority");
    assert!(matches!(error, CrashReplaceError::Io { .. }));
    assert_eq!(
        fs::read(target(&root)).expect("late leaf remains"),
        b"late: true\n"
    );
    assert_no_sidecars(&root);
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

fn own_absent_session(root: &Path) -> forge_core_store::OwnedRetainedCrashReplaceSession {
    let lock = acquire_effect_store_lock(root, LOCK).expect("lifecycle lock");
    reconcile_file_crash_safe_under_owned_lock(lock, Path::new(TARGET), MAX_BYTES)
        .expect("move lock into owned reconciliation session")
}

#[test]
fn owned_reconciliation_session_requires_no_self_reference() {
    let root = temp_root("owned-session-lock");
    let session = own_absent_session(&root);
    assert!(session.digest().is_none());
    let installed = session
        .replace(b"revision: 1\n")
        .expect("consume owned session for replacement");
    assert_eq!(installed.raw_bytes(), b"revision: 1\n");
    installed
        .retained_store_io()
        .expect("owned exact read retains the effect lock");
    assert_eq!(
        fs::read(target(&root)).expect("installed target"),
        b"revision: 1\n"
    );
    drop(installed);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn reconciliation_session_outlives_io_and_consumes_exact_absence() {
    let root = temp_root("owned-session-absence");
    let lock = acquire_effect_store_lock(&root, "packs/lifecycle.lock").expect("lifecycle lock");
    let session = {
        let io = lock.retained_store_io().expect("retained packs I/O");
        let session = io
            .reconcile_file_crash_safe(Path::new("active.lock.yaml"), MAX_BYTES)
            .expect("reconcile exact absence");
        assert!(session.digest().is_none());
        assert!(session.raw_bytes().is_none());
        session
    };

    let installed = session
        .replace(b"revision: 1\n")
        .expect("consume exact absence once");
    assert_eq!(installed.raw_bytes(), b"revision: 1\n");
    assert_eq!(
        fs::read(target(&root)).expect("installed target"),
        b"revision: 1\n"
    );
    assert_no_sidecars(&root);
    drop(installed);
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn reconciliation_session_rejects_byte_identical_selector_substitution() {
    let root = temp_root("owned-session-substitute");
    let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
    let old = b"revision: 1\n";
    let mut absent = retain_expected(&lock, MAX_BYTES).expect("retain exact absence");
    let installed = replace_retained(&lock, &mut absent, old, MAX_BYTES).expect("install old");
    drop(installed);

    let session =
        reconcile_file_crash_safe_under_retained_lock(&lock, Path::new(TARGET), MAX_BYTES)
            .expect("reconcile exact old target");
    let old_digest = sha256_content_hash(old);
    assert_eq!(session.raw_bytes(), Some(&old[..]));
    assert_eq!(session.digest(), Some(old_digest.as_str()));

    let displaced = target(&root).with_extension("retained-old");
    fs::rename(target(&root), &displaced).expect("hide exact selector target");
    fs::write(target(&root), old).expect("install byte-identical substitute");
    let error = session
        .replace(b"revision: 2\n")
        .expect_err("byte-identical substitute must not satisfy retained session authority");
    assert!(matches!(error, CrashReplaceError::Io { .. }));
    assert_eq!(fs::read(target(&root)).expect("substitute remains"), old);
    assert_no_sidecars(&root);
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn reconciliation_session_exact_read_uses_finalization_handle() {
    let root = temp_root("owned-session-read");
    let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
    let content = b"revision: 1\n";
    let mut absent = retain_expected(&lock, MAX_BYTES).expect("retain exact absence");
    let installed =
        replace_retained(&lock, &mut absent, content, MAX_BYTES).expect("install target");
    drop(installed);

    let session =
        reconcile_file_crash_safe_under_retained_lock(&lock, Path::new(TARGET), MAX_BYTES)
            .expect("reconcile exact target");
    let exact = session
        .read_exact()
        .expect("consume session as exact read")
        .expect("present exact target");
    assert_eq!(exact.raw_bytes(), content);
    assert_eq!(exact.digest(), sha256_content_hash(content));
    drop(exact);

    fs::rename(target(&root), target(&root).with_extension("hidden"))
        .expect("hide authoritative selector");
    let absent_reader =
        reconcile_file_crash_safe_under_retained_lock(&lock, Path::new(TARGET), MAX_BYTES)
            .expect("reconcile hidden selector as absence");
    assert!(absent_reader
        .read_exact()
        .expect("consume exact hidden-selector read")
        .is_none());
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn reconciliation_session_absence_rejects_create_delete_aba_before_consume() {
    let root = temp_root("owned-session-absence-aba");
    let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
    let session =
        reconcile_file_crash_safe_under_retained_lock(&lock, Path::new(TARGET), MAX_BYTES)
            .expect("reconcile exact absence");

    fs::remove_file(target(&root)).expect("remove Store absence placeholder");
    fs::write(target(&root), b"transient attacker leaf\n").expect("create transient leaf");
    fs::remove_file(target(&root)).expect("delete transient leaf");

    let error = session
        .replace(b"revision: 1\n")
        .expect_err("create-delete ABA must invalidate the exact absence claim");
    assert!(matches!(error, CrashReplaceError::Io { .. }));
    assert!(!target(&root).exists());
    assert_no_sidecars(&root);
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn reconciliation_session_absence_rejects_late_selector() {
    let root = temp_root("owned-session-late-selector");
    let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
    let session =
        reconcile_file_crash_safe_under_retained_lock(&lock, Path::new(TARGET), MAX_BYTES)
            .expect("reconcile exact absence");

    fs::create_dir_all(target(&root).parent().expect("target parent")).expect("target parent");
    fs::write(target(&root), b"late: true\n").expect("install late selector");
    let error = session
        .replace(b"revision: 1\n")
        .expect_err("late selector must invalidate retained absence authority");
    assert!(matches!(error, CrashReplaceError::Io { .. }));
    assert_eq!(
        fs::read(target(&root)).expect("late selector remains"),
        b"late: true\n"
    );
    assert_no_sidecars(&root);
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn oversized_and_non_regular_protocol_state_fail_closed() {
    let root = temp_root("bounds-regular");
    let lock = acquire_effect_store_lock(&root, LOCK).expect("lifecycle lock");
    let oversized = vec![b'x'; 17];
    let mut absent = retain_expected(&lock, 16).expect("retain exact absence");
    let error = replace_retained(&lock, &mut absent, &oversized, 16)
        .expect_err("oversized content must fail");
    assert!(matches!(error, CrashReplaceError::SizeLimit { .. }));
    assert!(!target(&root).exists());

    fs::create_dir_all(target(&root).parent().expect("target parent")).expect("target parent");
    fs::create_dir(sidecars(&root)[2].clone()).expect("transaction sidecar directory");
    let error =
        recover_retained(&lock, MAX_BYTES).expect_err("non-regular marker must fail closed");
    assert!(
        matches!(error, CrashReplaceError::Io { .. }),
        "unexpected non-regular marker error: {error:?}"
    );
    assert!(!target(&root).exists());
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}
