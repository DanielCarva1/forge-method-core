use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use forge_core_contracts::common::StableId;
use forge_core_contracts::tool_effect::{AccessMode, EffectTargetKind};
use forge_core_store::{
    recover_effect_wal, sha256_content_hash, EffectWalOriginal, EffectWalRecord,
    EffectWalRecoveryReason, EffectWalRecoveryStatus, EffectWalStage, EffectWalTargetMetadata,
};

const WAL_RELATIVE_PATH: &str = ".forge-method/effects.wal.ndjson";

fn temp_repo(test_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "forge-wal-recovery-{test_name}-{}-{nanos}",
        std::process::id()
    ))
}

fn wal_path(root: &Path) -> PathBuf {
    root.join(WAL_RELATIVE_PATH)
}

fn write_wal(root: &Path, lines: &[String]) {
    let path = wal_path(root);
    fs::create_dir_all(path.parent().expect("WAL has parent")).expect("create WAL parent");
    fs::write(path, lines.concat()).expect("write WAL");
}

fn record(
    tx_id: &str,
    stage: EffectWalStage,
    target_ref: Option<&str>,
    original: Option<&[u8]>,
) -> EffectWalRecord {
    EffectWalRecord {
        schema_version: "0.1".to_string(),
        tx_id: tx_id.to_string(),
        stage,
        effect_id: StableId(format!("effect.{tx_id}")),
        target_ref: target_ref.map(str::to_string),
        physical_target_ref: target_ref.map(str::to_string),
        target_metadata: target_ref.map(|_| EffectWalTargetMetadata {
            operation_id: StableId(format!("operation.{tx_id}")),
            target_kind: EffectTargetKind::FilePath,
            access_mode: AccessMode::Write,
            content_hash: Some(sha256_content_hash(b"after")),
            byte_len: 5,
            actor_agent_id: StableId("agent.test".to_string()),
            actor_role: forge_core_contracts::claim::ActorRole::Driver,
            destructive: false,
            redaction_hint: StableId("raw_content_not_indexed".to_string()),
        }),
        original: original.map(|content| EffectWalOriginal {
            existed: true,
            content: content.to_vec(),
            content_hash: sha256_content_hash(content),
        }),
        diagnostic: None,
        execution_provenance: None,
        replay_binding: None,
        replay_completion: None,
    }
}

fn line(record: &EffectWalRecord) -> String {
    let mut value = serde_json::to_string(record).expect("serialize WAL record");
    value.push('\n');
    value
}

#[test]
fn recovery_ignores_truncated_final_record_and_recovers_prior_incomplete_tx() {
    let root = temp_repo("truncated-final");
    let target_ref = "state/incomplete.txt";
    fs::create_dir_all(root.join("state")).expect("create state dir");
    fs::write(root.join(target_ref), b"after").expect("seed mutated target");

    write_wal(
        &root,
        &[
            line(&record("tx-incomplete", EffectWalStage::Begin, None, None)),
            line(&record(
                "tx-incomplete",
                EffectWalStage::BeforeImage,
                Some(target_ref),
                Some(b"before"),
            )),
            "{\"schema_version\":\"0.1\",\"tx_id\":\"tx-incomplete\"".to_string(),
        ],
    );

    let result = recover_effect_wal(&root, WAL_RELATIVE_PATH);

    assert_eq!(result.status, EffectWalRecoveryStatus::Recovered);
    assert_eq!(result.recovered_transactions, vec!["tx-incomplete"]);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("ignored truncated final WAL line")));
    assert_eq!(
        fs::read(root.join(target_ref)).expect("read target"),
        b"before"
    );
}

#[test]
fn recovery_rejects_malformed_complete_line() {
    let root = temp_repo("malformed-line");
    write_wal(
        &root,
        &[
            line(&record("tx-good", EffectWalStage::Begin, None, None)),
            "not json\n".to_string(),
        ],
    );

    let result = recover_effect_wal(&root, WAL_RELATIVE_PATH);

    assert_eq!(result.status, EffectWalRecoveryStatus::RecoveryFailed);
    assert!(result
        .reasons
        .contains(&EffectWalRecoveryReason::WalParseFailed));
}

#[test]
fn recovery_rolls_back_only_incomplete_group_when_committed_and_incomplete_are_mixed() {
    let root = temp_repo("mixed-groups");
    fs::create_dir_all(root.join("state")).expect("create state dir");
    fs::write(root.join("state/committed.txt"), b"committed-after").expect("seed committed");
    fs::write(root.join("state/incomplete.txt"), b"incomplete-after").expect("seed incomplete");

    write_wal(
        &root,
        &[
            line(&record("tx-committed", EffectWalStage::Begin, None, None)),
            line(&record(
                "tx-committed",
                EffectWalStage::BeforeImage,
                Some("state/committed.txt"),
                Some(b"committed-before"),
            )),
            line(&record(
                "tx-committed",
                EffectWalStage::WriteApplied,
                Some("state/committed.txt"),
                None,
            )),
            line(&record("tx-committed", EffectWalStage::Commit, None, None)),
            line(&record("tx-incomplete", EffectWalStage::Begin, None, None)),
            line(&record(
                "tx-incomplete",
                EffectWalStage::BeforeImage,
                Some("state/incomplete.txt"),
                Some(b"incomplete-before"),
            )),
            line(&record(
                "tx-incomplete",
                EffectWalStage::WriteApplied,
                Some("state/incomplete.txt"),
                None,
            )),
        ],
    );

    let result = recover_effect_wal(&root, WAL_RELATIVE_PATH);

    assert_eq!(result.status, EffectWalRecoveryStatus::Recovered);
    assert_eq!(result.recovered_transactions, vec!["tx-incomplete"]);
    assert_eq!(
        fs::read(root.join("state/committed.txt")).expect("read committed"),
        b"committed-after"
    );
    assert_eq!(
        fs::read(root.join("state/incomplete.txt")).expect("read incomplete"),
        b"incomplete-before"
    );
}

#[test]
fn recovery_is_idempotent_after_recovered_rollback_marker_is_written() {
    let root = temp_repo("idempotent");
    fs::create_dir_all(root.join("state")).expect("create state dir");
    fs::write(root.join("state/incomplete.txt"), b"after").expect("seed incomplete");
    write_wal(
        &root,
        &[
            line(&record("tx-incomplete", EffectWalStage::Begin, None, None)),
            line(&record(
                "tx-incomplete",
                EffectWalStage::BeforeImage,
                Some("state/incomplete.txt"),
                Some(b"before"),
            )),
        ],
    );

    let first = recover_effect_wal(&root, WAL_RELATIVE_PATH);
    let second = recover_effect_wal(&root, WAL_RELATIVE_PATH);

    assert_eq!(first.status, EffectWalRecoveryStatus::Recovered);
    assert_eq!(second.status, EffectWalRecoveryStatus::Noop);
    assert!(second
        .reasons
        .contains(&EffectWalRecoveryReason::NoRecoveryNeeded));
    assert_eq!(
        fs::read(root.join("state/incomplete.txt")).expect("read incomplete"),
        b"before"
    );
}
