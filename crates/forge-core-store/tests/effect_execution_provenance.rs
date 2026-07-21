use forge_core_contracts::tool_effect::EffectTargetKind;
use forge_core_contracts::{StableId, ToolEffectContractDocument};
use forge_core_store::{
    acquire_effect_store_lock, apply_file_effect_transaction_with_provenance_under_lock,
    compact_effect_wal, pending_effect_replay_commits_under_lock, recover_effect_wal,
    repair_effect_wal_tail_under_lock, sha256_content_hash, EffectApplicationPayload,
    EffectApplicationStatus, EffectExecutionProvenance, EffectExecutionProvenanceError,
    EffectReplayCommitBinding, EffectWalCompactionReason, EffectWalRecord, EffectWalRecoveryStatus,
    EffectWalStage,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const LOCK_REF: &str = ".forge-method/locks/effects.lock";
const WAL_REF: &str = ".forge-method/wal/effects.ndjson";
const TARGET_REF: &str = "out/committed.txt";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-effect-provenance-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(root.join(".forge-method")).expect("state root");
    fs::create_dir_all(root.join("out")).expect("target parent");
    root
}

fn effect() -> ToolEffectContractDocument {
    let path = repo_root().join("contracts/effects/story-artifact-write-effect.yaml");
    let mut effect: ToolEffectContractDocument =
        yaml_serde::from_str(&fs::read_to_string(path).expect("effect fixture"))
            .expect("parse effect");
    effect.tool_effect_contract.actor.agent_id = StableId("codex-main".to_owned());
    effect
        .tool_effect_contract
        .read_set
        .retain(|read| read.target_kind != EffectTargetKind::FilePath);
    effect.tool_effect_contract.write_set.truncate(1);
    let write = &mut effect.tool_effect_contract.write_set[0];
    TARGET_REF.clone_into(&mut write.reference);
    write.target_kind = EffectTargetKind::FilePath;
    effect
}

fn payload() -> EffectApplicationPayload {
    let content = b"provenance-bound\n".to_vec();
    EffectApplicationPayload {
        target_ref: TARGET_REF.to_owned(),
        content_hash: sha256_content_hash(&content),
        content,
    }
}

fn binding() -> EffectReplayCommitBinding {
    EffectReplayCommitBinding::new(
        sha256_content_hash(b"key"),
        sha256_content_hash(b"intent"),
        sha256_content_hash(b"commit"),
        1,
    )
}

fn wal_path(root: &Path) -> PathBuf {
    root.join(WAL_REF)
}

#[test]
fn canonical_provenance_detects_document_tampering() {
    let mut provenance = EffectExecutionProvenance::new(json!({
        "schema_version": "0.1",
        "decision": "admitted"
    }))
    .expect("provenance");
    provenance.verify().expect("valid digest");
    provenance.document["decision"] = json!("tampered");
    assert!(matches!(
        provenance.verify(),
        Err(EffectExecutionProvenanceError::DigestMismatch { .. })
    ));
}

#[test]
fn provenance_bound_commit_is_pending_and_compaction_retains_complete_evidence() {
    let root = temp_root("retention");
    let effect = effect();
    let payload = payload();
    let lock = acquire_effect_store_lock(&root, LOCK_REF).expect("effect lock");
    let provenance = EffectExecutionProvenance::new(json!({
        "schema_version": "0.1",
        "tx_id": "tx-provenance",
        "complete": true
    }))
    .expect("provenance");
    let application = apply_file_effect_transaction_with_provenance_under_lock(
        &root,
        &lock,
        LOCK_REF,
        &effect,
        &[payload],
        WAL_REF,
        "tx-provenance",
        provenance,
        binding(),
    );
    assert_eq!(
        application.status,
        EffectApplicationStatus::Applied,
        "{application:?}"
    );

    let pending = pending_effect_replay_commits_under_lock(&root, &lock, LOCK_REF, WAL_REF)
        .expect("pending projection");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tx_id, "tx-provenance");

    let compaction = compact_effect_wal(&root, WAL_REF);
    assert_eq!(compaction.dropped_records, 0);
    assert!(compaction
        .reasons
        .contains(&EffectWalCompactionReason::ProvenanceRecordsRetained));
    let records: Vec<EffectWalRecord> = fs::read_to_string(wal_path(&root))
        .expect("WAL")
        .lines()
        .map(|line| serde_json::from_str(line).expect("record"))
        .collect();
    assert_eq!(
        records.first().map(|record| record.stage),
        Some(EffectWalStage::Begin)
    );
    assert_eq!(
        records.last().map(|record| record.stage),
        Some(EffectWalStage::Commit)
    );
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn pending_projection_fails_closed_on_tampered_provenance() {
    let root = temp_root("tampered-wal");
    let effect = effect();
    let lock = acquire_effect_store_lock(&root, LOCK_REF).expect("effect lock");
    let provenance = EffectExecutionProvenance::new(json!({
        "schema_version": "0.1",
        "tx_id": "tx-tampered"
    }))
    .expect("provenance");
    let application = apply_file_effect_transaction_with_provenance_under_lock(
        &root,
        &lock,
        LOCK_REF,
        &effect,
        &[payload()],
        WAL_REF,
        "tx-tampered",
        provenance,
        binding(),
    );
    assert_eq!(
        application.status,
        EffectApplicationStatus::Applied,
        "{application:?}"
    );

    let wal = wal_path(&root);
    let mut records: Vec<EffectWalRecord> = fs::read_to_string(&wal)
        .expect("WAL")
        .lines()
        .map(|line| serde_json::from_str(line).expect("record"))
        .collect();
    records[0]
        .execution_provenance
        .as_mut()
        .expect("provenance")
        .document["tx_id"] = json!("tx-attacker");
    let mut content = String::new();
    for record in records {
        content.push_str(&serde_json::to_string(&record).expect("serialize"));
        content.push('\n');
    }
    fs::write(&wal, content).expect("tampered WAL");
    assert!(pending_effect_replay_commits_under_lock(&root, &lock, LOCK_REF, WAL_REF).is_err());
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn torn_effect_tail_is_repaired_before_incomplete_effect_rollback() {
    let root = temp_root("torn-before-commit");
    let effect = effect();
    let lock = acquire_effect_store_lock(&root, LOCK_REF).expect("effect lock");
    let provenance = EffectExecutionProvenance::new(json!({
        "schema_version": "0.1",
        "tx_id": "tx-torn"
    }))
    .expect("provenance");
    let application = apply_file_effect_transaction_with_provenance_under_lock(
        &root,
        &lock,
        LOCK_REF,
        &effect,
        &[payload()],
        WAL_REF,
        "tx-torn",
        provenance,
        binding(),
    );
    assert_eq!(application.status, EffectApplicationStatus::Applied);
    assert!(root.join(TARGET_REF).exists());

    let wal = wal_path(&root);
    let text = fs::read_to_string(&wal).expect("WAL");
    let mut lines: Vec<&str> = text.lines().collect();
    let commit_line = lines.pop().expect("commit line");
    let commit: EffectWalRecord = serde_json::from_str(commit_line).expect("commit JSON");
    assert_eq!(commit.stage, EffectWalStage::Commit);
    fs::write(
        &wal,
        format!(
            "{}\n{}",
            lines.join("\n"),
            &commit_line[..commit_line.len() / 2]
        ),
    )
    .expect("torn commit tail");

    assert!(
        repair_effect_wal_tail_under_lock(&root, &lock, LOCK_REF, WAL_REF)
            .expect("repair torn tail")
    );
    let recovery = recover_effect_wal(&root, WAL_REF);
    assert_eq!(recovery.status, EffectWalRecoveryStatus::Recovered);
    assert!(!root.join(TARGET_REF).exists());
    assert!(
        pending_effect_replay_commits_under_lock(&root, &lock, LOCK_REF, WAL_REF)
            .expect("pending after rollback")
            .is_empty()
    );
    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn retained_effect_lock_on_a_rejects_replacement_b_before_any_publication() {
    let root = temp_root("retained-a-replaced-by-b");
    fs::create_dir_all(root.join(".forge-method/wal")).expect("A WAL parent");
    fs::write(wal_path(&root), b"{\"torn\":").expect("A torn WAL");
    let lock = acquire_effect_store_lock(&root, LOCK_REF).expect("lock A");
    let displaced = root.with_extension("authority-a");
    fs::rename(&root, &displaced).expect("move A aside");
    fs::create_dir_all(root.join(".forge-method/wal")).expect("B WAL parent");
    fs::create_dir_all(root.join("out")).expect("B target parent");
    let b_wal = b"{\"replacement_b\":";
    fs::write(wal_path(&root), b_wal).expect("B torn WAL");

    assert!(repair_effect_wal_tail_under_lock(&root, &lock, LOCK_REF, WAL_REF).is_err());
    assert!(pending_effect_replay_commits_under_lock(&root, &lock, LOCK_REF, WAL_REF).is_err());
    let application = apply_file_effect_transaction_with_provenance_under_lock(
        &root,
        &lock,
        LOCK_REF,
        &effect(),
        &[payload()],
        WAL_REF,
        "tx-replacement-b",
        EffectExecutionProvenance::new(json!({"schema_version": "0.1"})).expect("provenance"),
        binding(),
    );
    assert_eq!(application.status, EffectApplicationStatus::Blocked);
    assert_eq!(fs::read(wal_path(&root)).expect("B WAL unchanged"), b_wal);
    assert!(
        !root.join(TARGET_REF).exists(),
        "replacement B must not receive target write"
    );
    assert_eq!(
        fs::read(wal_path(&displaced)).expect("A WAL unchanged"),
        b"{\"torn\":"
    );
    assert!(
        !displaced.join(TARGET_REF).exists(),
        "A must not publish after scope refusal"
    );

    drop(lock);
    fs::remove_dir_all(root).expect("cleanup B");
    fs::remove_dir_all(displaced).expect("cleanup A");
}

#[cfg(unix)]
#[test]
fn retained_effect_lock_rejects_unlinked_and_recreated_lock_file() {
    let root = temp_root("recreated-effect-lock");
    fs::create_dir_all(root.join(".forge-method/wal")).expect("WAL parent");
    let torn = b"{\"torn\":";
    fs::write(wal_path(&root), torn).expect("torn WAL");
    let lock = acquire_effect_store_lock(&root, LOCK_REF).expect("original lock");
    fs::remove_file(root.join(LOCK_REF)).expect("unlink held lock inode");
    fs::write(root.join(LOCK_REF), b"replacement").expect("recreate lock path");

    assert!(repair_effect_wal_tail_under_lock(&root, &lock, LOCK_REF, WAL_REF).is_err());
    let application = apply_file_effect_transaction_with_provenance_under_lock(
        &root,
        &lock,
        LOCK_REF,
        &effect(),
        &[payload()],
        WAL_REF,
        "tx-recreated-lock",
        EffectExecutionProvenance::new(json!({"schema_version": "0.1"})).expect("provenance"),
        binding(),
    );
    assert_eq!(application.status, EffectApplicationStatus::Blocked);
    assert_eq!(fs::read(wal_path(&root)).expect("WAL unchanged"), torn);
    assert!(!root.join(TARGET_REF).exists());

    drop(lock);
    fs::remove_dir_all(root).expect("cleanup");
}
