use forge_core_contracts::PrincipalId;
use forge_core_store::replay_anchor::{
    advance_replay_anchor, advance_replay_anchor_for_deployment, provision_replay_anchor,
    verify_replay_anchor, ReplayAnchorDocument, ReplayAnchorError, ReplayAnchorStatus,
};
use forge_core_store::replay_wal::{
    consume_replay_nonce_non_boundary, initialize_replay_wal, replay_wal_manifest_path,
    replay_wal_path, reserve_replay_nonce,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const AUDIENCE: &str = "forge://replay-anchor/test";
const NONCE: &str = "replay-anchor-nonce-000001";

fn digest(digit: char) -> String {
    format!("sha256:{}", digit.to_string().repeat(64))
}

fn temp_fixture(label: &str) -> (PathBuf, PathBuf, PathBuf) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-replay-anchor-{label}-{}-{unique}",
        std::process::id()
    ));
    let state = root.join("runtime/.forge-method");
    let operator = root.join("operator");
    fs::create_dir_all(&state).expect("state root");
    fs::create_dir_all(&operator).expect("operator root");
    initialize_replay_wal(&state).expect("initialize replay");
    let anchor = operator.join("replay-anchor.json");
    (root, state, anchor)
}

#[test]
fn external_anchor_advances_only_across_verified_wal_prefixes() {
    let (_root, state, anchor) = temp_fixture("lifecycle");
    let provisioned =
        provision_replay_anchor(&state, &anchor, "deployment.test").expect("provision anchor");
    assert_eq!(provisioned.anchor.generation, 1);
    assert_eq!(provisioned.anchor.head.last_seq, 0);
    assert_eq!(
        verify_replay_anchor(&state, &anchor)
            .expect("verify empty anchor")
            .status,
        ReplayAnchorStatus::Current
    );

    reserve_replay_nonce(
        &state,
        &PrincipalId("principal.test".to_owned()),
        AUDIENCE,
        NONCE,
        &digest('a'),
        &digest('b'),
    )
    .expect("reserve nonce");
    assert_eq!(
        verify_replay_anchor(&state, &anchor)
            .expect("verify extension")
            .status,
        ReplayAnchorStatus::AdvanceRequired
    );
    let reserved = advance_replay_anchor(&state, &anchor).expect("anchor reserve");
    assert!(reserved.changed);
    assert_eq!(reserved.anchor.generation, 2);
    assert_eq!(reserved.anchor.head.last_seq, 1);
    assert!(reserved.anchor.previous_anchor_digest.is_some());

    consume_replay_nonce_non_boundary(
        &state,
        &PrincipalId("principal.test".to_owned()),
        AUDIENCE,
        NONCE,
        &digest('a'),
        &digest('b'),
        1,
    )
    .expect("consume nonce");
    let consumed = advance_replay_anchor(&state, &anchor).expect("anchor consume");
    assert_eq!(consumed.anchor.generation, 3);
    assert_eq!(consumed.anchor.head.last_seq, 2);
    assert_eq!(
        verify_replay_anchor(&state, &anchor)
            .expect("current anchor")
            .status,
        ReplayAnchorStatus::Current
    );
}

#[test]
fn older_replay_pair_is_rejected_against_external_head() {
    let (_root, state, anchor) = temp_fixture("rollback");
    provision_replay_anchor(&state, &anchor, "deployment.rollback").expect("provision");
    let empty_wal = fs::read(replay_wal_path(&state)).expect("empty WAL");
    let original_manifest =
        fs::read(replay_wal_manifest_path(&state)).expect("original replay manifest");
    reserve_replay_nonce(
        &state,
        &PrincipalId("principal.test".to_owned()),
        AUDIENCE,
        NONCE,
        &digest('c'),
        &digest('d'),
    )
    .expect("reserve");
    advance_replay_anchor(&state, &anchor).expect("advance");

    fs::write(replay_wal_path(&state), empty_wal).expect("restore older WAL");
    fs::write(replay_wal_manifest_path(&state), original_manifest).expect("restore older manifest");
    let rejection = verify_replay_anchor(&state, &anchor).expect_err("rollback must fail");
    assert!(matches!(
        rejection,
        ReplayAnchorError::RollbackDetected {
            anchored_seq: 1,
            current_seq: 0
        }
    ));
}

#[test]
fn anchor_path_inside_state_root_is_rejected() {
    let (_root, state, _anchor) = temp_fixture("confinement");
    let rejection = provision_replay_anchor(
        &state,
        state.join("replay-anchor.json"),
        "deployment.invalid",
    )
    .expect_err("state-local anchor must fail");
    assert!(rejection
        .to_string()
        .contains("outside the Forge state root"));
}

#[test]
fn idempotent_advance_does_not_bump_generation() {
    let (_root, state, anchor) = temp_fixture("idempotent");
    provision_replay_anchor(&state, &anchor, "deployment.idempotent").expect("provision");
    let unchanged = advance_replay_anchor(&state, &anchor).expect("idempotent advance");
    assert!(!unchanged.changed);
    assert_eq!(unchanged.anchor.generation, 1);
}

#[test]
fn deployment_mismatch_never_advances_the_anchor() {
    let (_root, state, anchor) = temp_fixture("deployment-mismatch");
    provision_replay_anchor(&state, &anchor, "deployment.expected").expect("provision");
    reserve_replay_nonce(
        &state,
        &PrincipalId("principal.test".to_owned()),
        AUDIENCE,
        NONCE,
        &digest('e'),
        &digest('f'),
    )
    .expect("reserve");
    let error = advance_replay_anchor_for_deployment(&state, &anchor, "deployment.different")
        .expect_err("mismatched deployment must fail before replacement");
    assert!(matches!(
        error,
        ReplayAnchorError::DeploymentMismatch { .. }
    ));
    let document: ReplayAnchorDocument =
        serde_json::from_slice(&fs::read(&anchor).expect("anchor bytes")).expect("anchor JSON");
    assert_eq!(document.generation, 1);
    assert_eq!(document.head.last_seq, 0);
}

#[test]
fn oversized_deployment_identity_is_rejected_before_anchor_creation() {
    let (_root, state, anchor) = temp_fixture("oversized-deployment");
    let error = provision_replay_anchor(&state, &anchor, &"x".repeat(257))
        .expect_err("oversized deployment identity must fail");
    assert!(matches!(error, ReplayAnchorError::Invalid(_)));
    assert!(!anchor.exists());
}
