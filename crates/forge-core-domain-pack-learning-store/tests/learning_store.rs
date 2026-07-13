use forge_core_contracts::DomainPackLocalLearningCandidateDocument;
use forge_core_domain_pack_learning_store::{
    candidate_self_digest, capture_local_learning, learning_store_status, LearningCaptureAuthority,
    LearningCaptureDisposition, LearningObjectIntegrity, LearningStoreError,
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
fn status_rehashes_and_reports_tampered_regular_object() {
    let root = TempRoot::new("tamper");
    let receipt = capture_local_learning(root.path(), &candidate_bytes(None)).expect("capture");
    fs::write(root.path().join(receipt.object_relative_path), b"tampered").expect("tamper object");

    let projection = learning_store_status(root.path()).expect("integrity projection");
    assert_eq!(
        projection.records[0].integrity,
        LearningObjectIntegrity::DigestMismatch
    );
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
