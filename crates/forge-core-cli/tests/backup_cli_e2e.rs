use assert_cmd::Command;
use serde_json::Value;

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary must exist")
}

#[test]
fn backup_is_registered_and_help_lists_create_and_verify() {
    let output = bin()
        .args(["backup", "--help"])
        .output()
        .expect("run backup help");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    assert!(stdout.contains("create --root"));
    assert!(stdout.contains("verify --root"));
    assert!(stdout.contains("--authority <configured-id>"));
    assert!(!stdout.contains("--receipt-store"));
    assert!(!stdout.contains("--replay-anchor"));
}

#[test]
fn malformed_verify_surface_is_usage_error() {
    bin().args(["backup", "verify", "--json"]).assert().code(2);
    bin()
        .args(["backup", "verify", "--unknown"])
        .assert()
        .code(2);
}

#[test]
fn attacker_cannot_select_a_self_created_receipt_store() {
    bin()
        .args([
            "backup",
            "verify",
            "--root",
            ".",
            "--archive",
            "attacker.forge-backup",
            "--authority",
            "production",
            "--receipt-store",
            "attacker-receipts",
            "--replay-anchor",
            "attacker-anchor.json",
            "--json",
        ])
        .assert()
        .code(2);
}

#[test]
fn restore_is_registered_without_caller_selectable_trust_paths() {
    let output = bin()
        .args(["restore", "--help"])
        .output()
        .expect("run restore help");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    assert!(stdout.contains("preflight --root"));
    assert!(stdout.contains("apply --root"));
    assert!(stdout.contains("--authority <configured-id>"));
    assert!(!stdout.contains("--receipt-store"));
    assert!(!stdout.contains("--replay-anchor"));
}

#[test]
fn restore_rejects_direct_trust_path_selection() {
    bin()
        .args([
            "restore",
            "apply",
            "--root",
            ".",
            "--archive",
            "attacker.forge-backup",
            "--authority",
            "production",
            "--receipt-store",
            "attacker-receipts",
            "--replay-anchor",
            "attacker-anchor.json",
            "--json",
        ])
        .assert()
        .code(2);
}

#[cfg(target_os = "linux")]
fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(target_os = "linux")]
fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

#[cfg(target_os = "linux")]
fn run_with_machine_catalog(fake_etc: &std::path::Path, args: &[&str]) -> std::process::Output {
    let binary = assert_cmd::cargo::cargo_bin("forge-core");
    let mut command = std::process::Command::new("unshare");
    command.args([
        "-Urnm",
        "sh",
        "-c",
        "mount --bind \"$1\" /etc && shift && exec \"$@\"",
        "sh",
    ]);
    command.arg(fake_etc).arg(binary).args(args);
    command
        .output()
        .expect("run CLI in isolated machine config namespace")
}

#[cfg(target_os = "linux")]
#[test]
#[allow(clippy::too_many_lines)]
fn cli_rejects_wal_append_without_anchor_advance() {
    use forge_core_contracts::{
        BackupEntryKind, BackupManifestDocument, BackupReceipt, BackupReceiptDocument, PrincipalId,
        BACKUP_RECEIPT_SCHEMA_VERSION,
    };
    use forge_core_store::replay_anchor::provision_replay_anchor;
    use forge_core_store::replay_wal::{
        initialize_replay_wal, replay_wal_manifest_path, replay_wal_path, reserve_replay_nonce,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "forge-backup-cli-current-{}-{unique}",
        std::process::id()
    ));
    let project_root = root.join("project");
    let state_root = root.join("forge-app/.forge-method");
    let operator_root = root.join("operator");
    let receipt_store = operator_root.join("receipts");
    let archive_path = root.join("archives/project.forge-backup");
    let fake_etc = root.join("etc");
    fs::create_dir_all(&project_root).expect("project root");
    fs::create_dir_all(&state_root).expect("state root");
    fs::create_dir_all(&receipt_store).expect("receipt store");
    fs::create_dir_all(archive_path.parent().expect("archive parent")).expect("archive dir");
    fs::create_dir_all(fake_etc.join("forge-core")).expect("fake machine config");
    let project_link =
        include_bytes!("../../../contracts/fixtures/backup-manifest/valid/project-link.yaml")
            .to_vec();
    fs::write(project_root.join(".forge-method.yaml"), &project_link).expect("Project Link");
    initialize_replay_wal(&state_root).expect("initialize replay WAL");
    let replay_anchor_path = operator_root.join("replay-anchor.json");
    let provisioned =
        provision_replay_anchor(&state_root, &replay_anchor_path, "deployment.cli-backup")
            .expect("provision replay anchor");
    let replay_manifest = fs::read(replay_wal_manifest_path(&state_root)).expect("manifest");
    let replay_wal = fs::read(replay_wal_path(&state_root)).expect("WAL");
    let anchor_bytes = fs::read(&replay_anchor_path).expect("anchor");

    let mut manifest: BackupManifestDocument = serde_json::from_str(include_str!(
        "../../../contracts/fixtures/backup-manifest/valid/empty-pre-rotation-v1.yaml"
    ))
    .expect("manifest fixture");
    let mut materials = vec![
        (BackupEntryKind::ProjectLink, project_link),
        (BackupEntryKind::RootLedger, Vec::new()),
        (BackupEntryKind::ReplayWalManifest, replay_manifest),
        (BackupEntryKind::ReplayWal, replay_wal),
    ];
    for (kind, bytes) in &materials {
        let entry = manifest
            .backup_manifest
            .entries
            .iter_mut()
            .find(|entry| entry.material == *kind)
            .expect("manifest entry");
        entry.byte_length = bytes.len() as u64;
        entry.sha256 = digest(bytes);
        if *kind == BackupEntryKind::ProjectLink {
            manifest.backup_manifest.project.project_link_sha256 = entry.sha256.clone();
        }
    }
    let observation = &mut manifest
        .backup_manifest
        .external_authority_observations
        .replay_rollback_anchor;
    observation.schema_version = provisioned.anchor.schema_version.clone();
    observation.protected_anchor_identity = "operator://replay/cli-backup".to_owned();
    observation.deployment_id = provisioned.anchor.deployment_id.clone();
    observation.epoch = provisioned.anchor.epoch.clone();
    observation.generation = provisioned.anchor.generation;
    observation.previous_anchor_digest = provisioned.anchor.previous_anchor_digest.clone();
    observation.anchor_document_sha256 = digest(&anchor_bytes);
    observation.replay_wal_manifest_digest = provisioned.anchor.head.manifest_digest.clone();
    observation.replay_wal_prefix_digest = provisioned.anchor.head.wal_prefix_digest.clone();
    observation.replay_wal_last_seq = provisioned.anchor.head.last_seq;
    observation.replay_wal_record_count = provisioned.anchor.head.record_count as u64;
    observation.replay_wal_byte_length = provisioned.anchor.head.byte_len;
    manifest.backup_manifest.manifest_set_digest = manifest.set_digest().expect("set digest");
    materials.sort_by(|left, right| {
        let left_entry = manifest
            .backup_manifest
            .entries
            .iter()
            .find(|entry| entry.material == left.0)
            .expect("left entry");
        let right_entry = manifest
            .backup_manifest
            .entries
            .iter()
            .find(|entry| entry.material == right.0)
            .expect("right entry");
        (left.0, &left_entry.logical_path).cmp(&(right.0, &right_entry.logical_path))
    });
    let manifest_bytes = serde_json::to_vec(&manifest).expect("manifest bytes");
    let mut archive_bytes = b"FORGE-BACKUP-V1\0".to_vec();
    push_u64(&mut archive_bytes, manifest_bytes.len() as u64);
    archive_bytes.extend_from_slice(&manifest_bytes);
    push_u64(&mut archive_bytes, materials.len() as u64);
    for (kind, bytes) in &materials {
        let name = manifest
            .backup_manifest
            .entries
            .iter()
            .find(|entry| entry.material == *kind)
            .expect("member entry")
            .logical_path
            .as_bytes();
        push_u64(&mut archive_bytes, name.len() as u64);
        push_u64(&mut archive_bytes, bytes.len() as u64);
        archive_bytes.extend_from_slice(name);
        archive_bytes.extend_from_slice(bytes);
    }
    fs::write(&archive_path, &archive_bytes).expect("archive");

    let backup = &manifest.backup_manifest;
    let observations = &backup.external_authority_observations;
    let mut receipt = BackupReceiptDocument {
        schema_version: BACKUP_RECEIPT_SCHEMA_VERSION.to_owned(),
        backup_receipt: BackupReceipt {
            archive_sha256: digest(&archive_bytes),
            manifest_set_digest: backup.manifest_set_digest.clone(),
            project_id: backup.project.project_link.project_id.clone(),
            project_link_sha256: backup.project.project_link_sha256.clone(),
            workflow_release: backup.workflow_release.clone(),
            effective_epoch: backup.effective_epoch.clone(),
            replay_monotonic_head: observations.replay_rollback_anchor.clone(),
            domain_pack_supply_chain: None,
            domain_pack_reviewed_learning: None,
            archived_principal_registry_raw_sha256: None,
            archived_broker_registry_raw_sha256: None,
            created_at_unix: 1,
            receipt_digest: format!("sha256:{}", "0".repeat(64)),
        },
    };
    receipt.backup_receipt.receipt_digest = receipt.digest().expect("receipt digest");
    let receipt_name = format!(
        "{}.receipt.json",
        backup
            .manifest_set_digest
            .strip_prefix("sha256:")
            .expect("digest prefix")
    );
    fs::write(
        receipt_store.join(receipt_name),
        serde_json::to_vec(&receipt).expect("receipt bytes"),
    )
    .expect("receipt");
    let catalog = serde_json::json!({
        "schema_version": "forge_backup_authority_catalog_v1",
        "authorities": [{
            "authority_id": "cli-test",
            "receipt_store": receipt_store,
            "replay_anchor_path": replay_anchor_path,
            "protected_anchor_identity": "operator://replay/cli-backup",
            "domain_pack_operator_root": null,
            "domain_pack_operator_root_identity": null
        }]
    });
    fs::write(
        fake_etc.join("forge-core/backup-authorities.json"),
        serde_json::to_vec(&catalog).expect("catalog bytes"),
    )
    .expect("catalog");
    let root_arg = project_root.to_string_lossy();
    let archive_arg = archive_path.to_string_lossy();
    let args = [
        "backup",
        "verify",
        "--root",
        root_arg.as_ref(),
        "--archive",
        archive_arg.as_ref(),
        "--authority",
        "cli-test",
        "--json",
    ];
    let current = run_with_machine_catalog(&fake_etc, &args);
    assert!(
        current.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&current.stdout),
        String::from_utf8_lossy(&current.stderr)
    );
    let current_envelope: Value = serde_json::from_slice(&current.stdout).expect("success JSON");
    assert_eq!(current_envelope["ok"], true);

    reserve_replay_nonce(
        &state_root,
        &PrincipalId("principal.cli-backup".to_owned()),
        "forge://backup/cli-test",
        "cli-backup-nonce-000001",
        &format!("sha256:{}", "a".repeat(64)),
        &format!("sha256:{}", "b".repeat(64)),
    )
    .expect("append WAL without anchor advance");
    let stale = run_with_machine_catalog(&fake_etc, &args);
    assert!(!stale.status.success());
    let stale_envelope: Value = serde_json::from_slice(&stale.stdout).expect("rejection JSON");
    assert_eq!(stale_envelope["ok"], false);
    assert!(stale_envelope["error"]["message"]
        .as_str()
        .expect("error message")
        .contains("advance required"));
    let _ = fs::remove_dir_all(root);
}
