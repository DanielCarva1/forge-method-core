use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use forge_core_contracts::tool_effect::{AccessMode, EffectRead, EffectTargetKind, EffectWrite};
use forge_core_contracts::{
    OperationContractDocument, RepoPath, StableId, ToolEffectContractDocument,
};
use forge_core_kernel::{
    apply_operation_effect_bundle_with_wal_lock, compose_operation_effect_bundle,
    OperationEffectBundleError, OPERATION_EFFECT_BUNDLE_SCHEMA_VERSION,
};
use forge_core_store::{
    recover_effect_wal, sha256_content_hash, EffectApplicationPayload, EffectApplicationStatus,
    EffectWalRecoveryStatus,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture<T: serde::de::DeserializeOwned>(path: &str) -> T {
    let text = fs::read_to_string(repo_root().join(path)).expect("read fixture");
    yaml_serde::from_str(&text).expect("parse fixture")
}

fn temp_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "forge-operation-effect-bundle-{label}-{}-{stamp}",
        std::process::id()
    ))
}

fn operation_and_effects() -> (
    OperationContractDocument,
    Vec<RepoPath>,
    Vec<ToolEffectContractDocument>,
) {
    let mut operation: OperationContractDocument =
        fixture("docs/fixtures/operation-contract-v0/execute-trivial-write.yaml");
    let mut first: ToolEffectContractDocument =
        fixture("contracts/effects/story-artifact-write-effect.yaml");
    let mut second = first.clone();
    let operation_id = operation.operation_contract.contract_id.clone();
    let refs = vec![
        RepoPath("contracts/effects/operation-wide-first.yaml".to_owned()),
        RepoPath("contracts/effects/operation-wide-second.yaml".to_owned()),
    ];
    operation
        .operation_contract
        .effect_contract_refs
        .clone_from(&refs);

    first.tool_effect_contract.id = StableId("effect.operation-wide.first".to_owned());
    first.tool_effect_contract.operation_ref = operation_id.clone();
    let shared_read = EffectRead {
        target_kind: EffectTargetKind::FilePath,
        reference: ".forge-method/input.txt".to_owned(),
        expected_hash: None,
        expected_version: None,
        required_for_plan: true,
    };
    first.tool_effect_contract.read_set = vec![shared_read.clone()];
    first.tool_effect_contract.write_set = vec![EffectWrite {
        target_kind: EffectTargetKind::FilePath,
        reference: ".forge-method/out/first.txt".to_owned(),
        access_mode: AccessMode::Create,
        expected_hash: None,
        expected_version: None,
        destructive: false,
    }];

    second.tool_effect_contract.id = StableId("effect.operation-wide.second".to_owned());
    second.tool_effect_contract.operation_ref = operation_id;
    second.tool_effect_contract.read_set = vec![shared_read];
    second.tool_effect_contract.write_set = vec![EffectWrite {
        target_kind: EffectTargetKind::FilePath,
        reference: ".forge-method/blocker/child.txt".to_owned(),
        access_mode: AccessMode::Create,
        expected_hash: None,
        expected_version: None,
        destructive: false,
    }];
    (operation, refs, vec![first, second])
}

fn payload(target_ref: &str, content: &[u8]) -> EffectApplicationPayload {
    EffectApplicationPayload {
        target_ref: target_ref.to_owned(),
        content: content.to_vec(),
        content_hash: sha256_content_hash(content),
    }
}

#[test]
fn bundle_is_deterministic_and_preserves_constituent_identity() {
    let (operation, refs, effects) = operation_and_effects();
    let root = repo_root();
    let first =
        compose_operation_effect_bundle(&root, &operation, &refs, &effects).expect("bundle");
    let second =
        compose_operation_effect_bundle(&root, &operation, &refs, &effects).expect("bundle");

    assert_eq!(first, second);
    assert_eq!(first.effect_refs(), refs);
    assert_eq!(
        first
            .effect_ids()
            .iter()
            .map(|id| id.0.as_str())
            .collect::<Vec<_>>(),
        vec![
            "effect.operation-wide.first",
            "effect.operation-wide.second"
        ]
    );
    assert_eq!(
        first.transaction_effect().schema_version,
        OPERATION_EFFECT_BUNDLE_SCHEMA_VERSION
    );
    assert_eq!(
        first
            .transaction_effect()
            .tool_effect_contract
            .write_set
            .len(),
        2
    );
}

#[test]
fn normalized_overlapping_write_targets_fail_closed() {
    let (operation, refs, mut effects) = operation_and_effects();
    effects[1].tool_effect_contract.write_set[0].reference =
        ".FORGE-METHOD\\out\\first.txt".to_owned();

    let error = compose_operation_effect_bundle(repo_root(), &operation, &refs, &effects)
        .expect_err("case-folded overlap must fail");
    assert!(matches!(
        error,
        OperationEffectBundleError::OverlappingWrite { .. }
    ));
}

#[test]
fn reordered_effect_refs_fail_closed_against_the_operation() {
    let (operation, mut refs, effects) = operation_and_effects();
    refs.swap(0, 1);

    let error = compose_operation_effect_bundle(repo_root(), &operation, &refs, &effects)
        .expect_err("reordered refs must not change declared operation semantics");
    assert_eq!(error, OperationEffectBundleError::EffectSetMismatch);
}

#[test]
fn logical_and_physical_aliases_of_the_same_write_fail_closed() {
    let (operation, refs, mut effects) = operation_and_effects();
    effects[0].tool_effect_contract.write_set[0].target_kind = EffectTargetKind::ArtifactId;
    effects[0].tool_effect_contract.write_set[0].reference = "collision".to_owned();
    effects[1].tool_effect_contract.write_set[0].target_kind = EffectTargetKind::FilePath;
    effects[1].tool_effect_contract.write_set[0].reference =
        ".forge-method/artifacts/collision.yaml".to_owned();

    let error = compose_operation_effect_bundle(repo_root(), &operation, &refs, &effects)
        .expect_err("logical and physical aliases must not evade overlap detection");
    assert!(matches!(
        error,
        OperationEffectBundleError::OverlappingWrite { .. }
    ));
}

#[test]
fn one_wal_transaction_rolls_back_writes_from_every_constituent_effect() {
    let (operation, refs, effects) = operation_and_effects();
    let root = temp_root("rollback");
    fs::create_dir_all(root.join(".forge-method/out")).expect("create output parent");
    fs::write(root.join(".forge-method/blocker"), b"not-a-directory").expect("create blocker");
    let bundle =
        compose_operation_effect_bundle(&root, &operation, &refs, &effects).expect("bundle");
    let wal_ref = ".forge-method/wal/effects.ndjson";
    let result = apply_operation_effect_bundle_with_wal_lock(
        &root,
        &bundle,
        &[
            payload(".forge-method/out/first.txt", b"first"),
            payload(".forge-method/blocker/child.txt", b"second"),
        ],
        wal_ref,
        ".forge-method/locks/effects.lock",
        "tx-operation-wide-rollback",
    );

    assert_eq!(result.status, EffectApplicationStatus::RolledBack);
    assert!(result.rolled_back);
    assert!(!root.join(".forge-method/out/first.txt").exists());
    assert_eq!(
        fs::read(root.join(".forge-method/blocker")).expect("read blocker"),
        b"not-a-directory"
    );
    let wal = fs::read_to_string(root.join(wal_ref)).expect("read WAL");
    assert!(wal.contains("tx-operation-wide-rollback"));
    assert!(wal.contains("\"stage\":\"rollback_complete\""));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn crash_recovery_rolls_back_the_complete_operation_bundle() {
    let (operation, refs, mut effects) = operation_and_effects();
    effects[1].tool_effect_contract.write_set[0].reference =
        ".forge-method/out/second.txt".to_owned();
    let root = temp_root("crash-recovery");
    fs::create_dir_all(root.join(".forge-method/out")).expect("create output parent");
    let bundle =
        compose_operation_effect_bundle(&root, &operation, &refs, &effects).expect("bundle");
    let wal_ref = ".forge-method/wal/effects.ndjson";
    let result = apply_operation_effect_bundle_with_wal_lock(
        &root,
        &bundle,
        &[
            payload(".forge-method/out/first.txt", b"first"),
            payload(".forge-method/out/second.txt", b"second"),
        ],
        wal_ref,
        ".forge-method/locks/effects.lock",
        "tx-operation-wide-crash",
    );
    assert_eq!(result.status, EffectApplicationStatus::Applied);

    let wal_path = root.join(wal_ref);
    let wal = fs::read_to_string(&wal_path).expect("read WAL");
    let mut lines = wal.lines().collect::<Vec<_>>();
    assert!(
        lines
            .last()
            .is_some_and(|line| line.contains("\"stage\":\"commit\"")),
        "fixture must end at the operation commit marker"
    );
    lines.pop();
    fs::write(&wal_path, format!("{}\n", lines.join("\n"))).expect("simulate missing commit");

    let recovery = recover_effect_wal(&root, wal_ref);
    assert_eq!(recovery.status, EffectWalRecoveryStatus::Recovered);
    assert_eq!(
        recovery.recovered_transactions,
        vec!["tx-operation-wide-crash"]
    );
    assert!(!root.join(".forge-method/out/first.txt").exists());
    assert!(!root.join(".forge-method/out/second.txt").exists());
    fs::remove_dir_all(root).expect("cleanup");
}
