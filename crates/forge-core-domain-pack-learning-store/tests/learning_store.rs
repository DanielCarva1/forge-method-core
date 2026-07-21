use forge_core_contracts::DomainPackLocalLearningCandidateDocument;
use forge_core_domain_pack_learning_store::{
    candidate_self_digest, capture_local_learning, learning_store_status,
    lock_domain_pack_learning_store, LearningCaptureAuthority, LearningCaptureDisposition,
    LearningObjectIntegrity, LearningStoreError, LEARNING_GENERATION_POINTER_RELATIVE_PATH,
    LEARNING_INDEX_RELATIVE_PATH,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const FIXTURE: &str = include_str!(
    "../../../docs/fixtures/domain-pack-learning-v0/valid/local-learning-candidate.yaml"
);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "forge-domain-learning-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn candidate_bytes(assertion: Option<&str>) -> Vec<u8> {
    let mut document: DomainPackLocalLearningCandidateDocument =
        yaml_serde::from_str(FIXTURE).expect("parse fixture");
    if let Some(assertion) = assertion {
        assertion.clone_into(&mut document.domain_pack_local_learning_candidate.assertion);
    }
    document
        .domain_pack_local_learning_candidate
        .candidate_digest = candidate_self_digest(&document).expect("compute candidate digest");
    yaml_serde::to_string(&document)
        .expect("serialize candidate")
        .into_bytes()
}

#[test]
fn captures_exact_bytes_and_lists_verified_non_authoritative_observation() {
    let root = TempRoot::new("capture");
    let raw = candidate_bytes(None);

    let receipt = capture_local_learning(root.path(), &raw).expect("capture candidate");
    assert_eq!(
        receipt.authority,
        LearningCaptureAuthority::NonAuthoritativeObservation
    );
    assert_eq!(receipt.disposition, LearningCaptureDisposition::Captured);
    assert_eq!(
        fs::read(root.path().join(&receipt.object_relative_path)).expect("read raw object"),
        raw
    );

    let projection = learning_store_status(root.path()).expect("list store");
    assert_eq!(
        projection.authority,
        LearningCaptureAuthority::NonAuthoritativeObservation
    );
    assert_eq!(projection.records.len(), 1);
    assert_eq!(
        projection.records[0].integrity,
        LearningObjectIntegrity::Verified
    );
}

#[test]
fn retained_guard_snapshots_exact_index_and_object_closure_without_relocking() {
    let root = TempRoot::new("raw-snapshot");
    let raw = candidate_bytes(None);
    let receipt = capture_local_learning(root.path(), &raw).expect("capture candidate");

    let guard = lock_domain_pack_learning_store(root.path()).expect("retain producer lock");
    let snapshot = guard.snapshot_raw_closure().expect("snapshot under guard");
    let index = snapshot.index().expect("captured index");
    assert_eq!(index.relative_path(), LEARNING_INDEX_RELATIVE_PATH);
    assert_eq!(
        index.raw_bytes(),
        fs::read(root.path().join(LEARNING_INDEX_RELATIVE_PATH))
            .expect("read exact index")
            .as_slice()
    );
    assert_eq!(snapshot.objects().len(), 1);
    assert_eq!(
        snapshot.objects()[0].relative_path(),
        receipt.object_relative_path
    );
    assert_eq!(snapshot.objects()[0].raw_bytes(), raw);

    assert!(matches!(
        capture_local_learning(root.path(), &candidate_bytes(None)),
        Err(LearningStoreError::Lock(_))
    ));
    drop(guard);
    capture_local_learning(root.path(), &candidate_bytes(None))
        .expect("dropping retained guard releases producer lock");
}

#[cfg(unix)]
#[test]
fn raw_snapshot_ignores_legacy_object_symlink_substitution() {
    let root = TempRoot::new("raw-snapshot-symlink");
    let raw = candidate_bytes(None);
    let receipt = capture_local_learning(root.path(), &raw).expect("capture candidate");
    let guard = lock_domain_pack_learning_store(root.path()).expect("retain producer lock");
    let object_path = root.path().join(receipt.object_relative_path);
    let moved = object_path.with_extension("moved");
    fs::rename(&object_path, &moved).expect("rename object");
    std::os::unix::fs::symlink(&moved, &object_path).expect("substitute object symlink");

    let snapshot = guard
        .snapshot_raw_closure()
        .expect("legacy object names cannot authorize the canonical closure");
    assert_eq!(snapshot.objects().len(), 1);
    assert_eq!(snapshot.objects()[0].raw_bytes(), raw);
}

#[cfg(unix)]
#[test]
fn raw_snapshot_rejects_learning_root_rename_and_replacement() {
    let root = TempRoot::new("raw-snapshot-root-swap");
    capture_local_learning(root.path(), &candidate_bytes(None))
        .expect("capture original candidate");
    let replacement = TempRoot::new("raw-snapshot-root-replacement");
    capture_local_learning(replacement.path(), &candidate_bytes(None))
        .expect("capture replacement candidate");
    let guard = lock_domain_pack_learning_store(root.path()).expect("pin original root");
    let moved = root.path().with_extension("moved");
    fs::rename(root.path(), &moved).expect("rename guarded root");
    fs::rename(replacement.path(), root.path()).expect("install well-formed replacement root");

    guard
        .snapshot_raw_closure()
        .expect_err("snapshot must reject a replacement root outside the retained lock namespace");
    drop(guard);
    fs::remove_dir_all(&moved).expect("remove original moved root");
}
#[test]
fn duplicate_exact_capture_is_idempotent() {
    let root = TempRoot::new("idempotent");
    let raw = candidate_bytes(None);

    let first = capture_local_learning(root.path(), &raw).expect("first capture");
    let second = capture_local_learning(root.path(), &raw).expect("repeat capture");

    assert_eq!(first.candidate_digest, second.candidate_digest);
    assert_eq!(
        second.disposition,
        LearningCaptureDisposition::AlreadyPresent
    );
    assert_eq!(
        learning_store_status(root.path())
            .expect("status")
            .records
            .len(),
        1
    );
}

#[test]
fn same_candidate_id_with_different_digest_is_explicit_conflict() {
    let root = TempRoot::new("conflict");
    capture_local_learning(root.path(), &candidate_bytes(None)).expect("first capture");

    let error = capture_local_learning(
        root.path(),
        &candidate_bytes(Some(
            "A contradictory observation for the same candidate id.",
        )),
    )
    .expect_err("candidate id equivocation must fail");

    assert!(matches!(
        error,
        LearningStoreError::CandidateIdConflict { .. }
    ));
}

#[test]
fn status_ignores_tampered_legacy_object_projection() {
    let root = TempRoot::new("tamper");
    let receipt = capture_local_learning(root.path(), &candidate_bytes(None)).expect("capture");
    fs::write(root.path().join(receipt.object_relative_path), b"tampered").expect("tamper object");

    let projection = learning_store_status(root.path()).expect("integrity projection");
    assert_eq!(
        projection.records[0].integrity,
        LearningObjectIntegrity::Verified
    );
}

#[test]
fn canonical_pointer_record_contains_the_complete_generation_closure() {
    let root = TempRoot::new("canonical-generation");
    let raw = candidate_bytes(None);
    capture_local_learning(root.path(), &raw).expect("capture");

    let pointer_raw = fs::read(root.path().join(LEARNING_GENERATION_POINTER_RELATIVE_PATH))
        .expect("read generation pointer");
    let pointer: serde_json::Value =
        serde_json::from_slice(&pointer_raw).expect("parse generation pointer");
    let authority_binding = pointer
        .get("store_authority_sha256")
        .and_then(serde_json::Value::as_str)
        .expect("pointer carries a Store-minted lineage binding");
    assert!(authority_binding.starts_with("sha256:"));
    assert_eq!(authority_binding.len(), 71);
    let operation_nonce = pointer
        .get("operation_nonce")
        .and_then(serde_json::Value::as_str)
        .expect("pointer carries a Store-minted operation nonce");
    assert_eq!(operation_nonce.len(), 64);
    assert!(operation_nonce.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let generation = pointer
        .get("generation")
        .expect("pointer embeds canonical generation");
    assert_eq!(
        generation
            .get("store_authority_sha256")
            .and_then(serde_json::Value::as_str),
        Some(authority_binding)
    );
    assert_eq!(
        generation
            .pointer("/index/records")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        generation
            .pointer("/objects/0/raw_utf8")
            .and_then(serde_json::Value::as_str)
            .map(str::as_bytes),
        Some(raw.as_slice())
    );
    let generation_path = pointer
        .get("generation_relative_path")
        .and_then(serde_json::Value::as_str)
        .expect("content-addressed generation path");
    let generation_raw =
        fs::read(root.path().join(generation_path)).expect("read content-addressed generation");
    let stored_generation: serde_json::Value =
        serde_json::from_slice(&generation_raw).expect("parse stored generation");
    assert_eq!(&stored_generation, generation);
}

#[test]
fn status_fails_closed_when_selected_generation_document_is_tampered() {
    let root = TempRoot::new("generation-tamper");
    capture_local_learning(root.path(), &candidate_bytes(None)).expect("capture");
    let pointer_raw = fs::read(root.path().join(LEARNING_GENERATION_POINTER_RELATIVE_PATH))
        .expect("read generation pointer");
    let pointer: serde_json::Value =
        serde_json::from_slice(&pointer_raw).expect("parse generation pointer");
    let generation_path = pointer
        .get("generation_relative_path")
        .and_then(serde_json::Value::as_str)
        .expect("content-addressed generation path");
    fs::write(root.path().join(generation_path), b"{}")
        .expect("tamper selected generation document");

    assert!(matches!(
        learning_store_status(root.path()),
        Err(LearningStoreError::CorruptGeneration(_))
    ));
}

#[test]
fn status_fails_closed_when_generation_pointer_is_substituted() {
    let root = TempRoot::new("pointer-substitution");
    capture_local_learning(root.path(), &candidate_bytes(None)).expect("capture");
    let pointer_path = root.path().join(LEARNING_GENERATION_POINTER_RELATIVE_PATH);
    let mut pointer: serde_json::Value =
        serde_json::from_slice(&fs::read(&pointer_path).expect("read generation pointer"))
            .expect("parse generation pointer");
    pointer["lock_relative_path"] = serde_json::Value::String("other.lock".to_owned());
    fs::write(
        &pointer_path,
        serde_json_canonicalizer::to_vec(&pointer).expect("encode substituted pointer"),
    )
    .expect("substitute generation pointer");

    assert!(matches!(
        learning_store_status(root.path()),
        Err(LearningStoreError::CorruptGeneration(_))
    ));
}

#[test]
fn hidden_generation_pointer_with_generation_and_projection_residue_fails_closed() {
    let root = TempRoot::new("hidden-pointer-residue");
    let raw = candidate_bytes(None);
    capture_local_learning(root.path(), &raw).expect("capture");
    let pointer_path = root.path().join(LEARNING_GENERATION_POINTER_RELATIVE_PATH);
    fs::rename(
        &pointer_path,
        pointer_path.with_extension("hidden-generation"),
    )
    .expect("hide authoritative generation pointer");

    assert!(matches!(
        learning_store_status(root.path()),
        Err(LearningStoreError::CorruptGeneration(_))
    ));
    assert!(matches!(
        capture_local_learning(root.path(), &raw),
        Err(LearningStoreError::CorruptGeneration(_))
    ));
}

#[test]
fn absent_generation_pointer_rejects_each_non_pristine_residue_class() {
    let generations = TempRoot::new("orphan-generation-residue");
    let generations_root = generations.path().join("domain-pack-learning/generations");
    fs::create_dir_all(&generations_root).expect("create generations directory");
    fs::write(generations_root.join("orphan"), b"{}").expect("write orphan generation");
    assert!(matches!(
        learning_store_status(generations.path()),
        Err(LearningStoreError::CorruptGeneration(_))
    ));

    let index = TempRoot::new("orphan-index-residue");
    let learning_root = index.path().join("domain-pack-learning");
    fs::create_dir_all(&learning_root).expect("create learning directory");
    fs::write(learning_root.join("index.json"), b"{}").expect("write orphan index");
    assert!(matches!(
        learning_store_status(index.path()),
        Err(LearningStoreError::CorruptGeneration(_))
    ));

    let objects = TempRoot::new("orphan-object-residue");
    fs::create_dir_all(objects.path().join("domain-pack-learning/objects"))
        .expect("create orphan objects directory");
    assert!(matches!(
        learning_store_status(objects.path()),
        Err(LearningStoreError::CorruptGeneration(_))
    ));
}

#[test]
fn pointer_recovery_and_load_share_one_store_reconciliation_session() {
    let source = include_str!("../src/lib.rs");
    let load = source
        .split_once("fn load_generation_authority")
        .expect("load function")
        .1
        .split_once("fn load_or_initialize_generation")
        .expect("load function end")
        .0;
    assert!(load.contains(".reconcile_file_crash_safe("));
    assert!(load.contains("pointer_session.raw_bytes()"));
    assert!(load.contains("pointer_session,"));
    assert!(!load.contains("recover_target("));
    assert!(!load.contains("read_optional_bounded("));

    let persist = source
        .split_once("fn persist_generation_pointer")
        .expect("persist function")
        .1
        .split_once("fn best_effort_project_generation")
        .expect("persist function end")
        .0;
    assert!(persist.contains("session.replace(&raw)"));
    assert!(!persist.contains("replace_file_crash_safe("));
}

#[test]
fn status_fails_closed_when_pointer_store_authority_binding_is_substituted() {
    let root = TempRoot::new("pointer-store-binding-substitution");
    capture_local_learning(root.path(), &candidate_bytes(None)).expect("capture");
    let pointer_path = root.path().join(LEARNING_GENERATION_POINTER_RELATIVE_PATH);
    let mut pointer: serde_json::Value =
        serde_json::from_slice(&fs::read(&pointer_path).expect("read generation pointer"))
            .expect("parse generation pointer");
    pointer["store_authority_sha256"] =
        serde_json::Value::String(format!("sha256:{}", "0".repeat(64)));
    fs::write(
        &pointer_path,
        serde_json_canonicalizer::to_vec(&pointer).expect("encode substituted pointer"),
    )
    .expect("substitute pointer Store authority binding");

    assert!(matches!(
        learning_store_status(root.path()),
        Err(LearningStoreError::CorruptGeneration(_))
    ));
}

#[cfg(unix)]
fn create_directory_link(link: &Path, target: &Path) {
    std::os::unix::fs::symlink(target, link).expect("create directory symlink");
}

#[cfg(windows)]
fn create_directory_link(link: &Path, target: &Path) {
    let status = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .status()
        .expect("invoke mklink");
    assert!(status.success(), "create directory junction");
}

#[cfg(unix)]
fn remove_directory_link(link: &Path) {
    fs::remove_file(link).expect("remove directory symlink");
}

#[cfg(windows)]
fn remove_directory_link(link: &Path) {
    fs::remove_dir(link).expect("remove directory junction");
}

#[test]
fn linked_or_junction_learning_root_escape_is_rejected() {
    let root = TempRoot::new("link-root");
    let outside = TempRoot::new("outside");
    let link = root.path().join("domain-pack-learning");
    create_directory_link(&link, outside.path());

    let result = capture_local_learning(root.path(), &candidate_bytes(None));
    remove_directory_link(&link);
    let error = result.expect_err("linked learning root must fail closed");
    assert!(matches!(error, LearningStoreError::InvalidStorePath { .. }));
}
