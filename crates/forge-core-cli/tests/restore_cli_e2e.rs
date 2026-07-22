#![cfg(target_os = "linux")]

use forge_core_contracts::{
    BackupEntryKind, BackupManifestDocument, BackupReceipt, BackupReceiptDocument,
    BACKUP_RECEIPT_SCHEMA_VERSION,
};
use forge_core_store::replay_anchor::provision_replay_anchor;
use forge_core_store::replay_wal::{
    initialize_replay_wal, replay_wal_manifest_path, replay_wal_path,
};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn run_with_machine_catalog(fake_etc: &Path, args: &[&str]) -> std::process::Output {
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
        .expect("run restore CLI in isolated machine config namespace")
}

struct MachineFixture {
    root: PathBuf,
    project_root: PathBuf,
    destination_sidecar: PathBuf,
    receipt_store: PathBuf,
    archive_path: PathBuf,
    replay_anchor_path: PathBuf,
    fake_etc: PathBuf,
    project_link_sha256: String,
    archive_sha256: String,
    manifest_set_digest: String,
    members: Vec<(String, Vec<u8>)>,
}

impl MachineFixture {
    #[allow(clippy::too_many_lines)]
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forge-restore-cli-{label}-{}-{unique}",
            std::process::id()
        ));
        let project_root = root.join("project");
        let destination_sidecar = root.join("forge-app");
        let authority_state_root = root.join("authority-state/.forge-method");
        let operator_root = root.join("operator");
        let receipt_store = operator_root.join("receipts");
        let archive_path = root.join("archives/project.forge-backup");
        let fake_etc = root.join("etc");
        fs::create_dir_all(&project_root).expect("project root");
        fs::create_dir_all(&authority_state_root).expect("authority state root");
        fs::create_dir_all(&receipt_store).expect("receipt store");
        fs::create_dir_all(archive_path.parent().expect("archive parent")).expect("archive dir");
        fs::create_dir_all(fake_etc.join("forge-core")).expect("fake machine config");
        let project_link =
            include_bytes!("../../../contracts/fixtures/backup-manifest/valid/project-link.yaml")
                .to_vec();
        fs::write(project_root.join(".forge-method.yaml"), &project_link).expect("Project Link");

        initialize_replay_wal(&authority_state_root).expect("initialize replay WAL");
        let replay_anchor_path = operator_root.join("replay-anchor.json");
        let provisioned = provision_replay_anchor(
            &authority_state_root,
            &replay_anchor_path,
            "deployment.cli-restore",
        )
        .expect("provision replay anchor");
        let replay_manifest =
            fs::read(replay_wal_manifest_path(&authority_state_root)).expect("replay manifest");
        let replay_wal = fs::read(replay_wal_path(&authority_state_root)).expect("replay WAL");
        let anchor_bytes = fs::read(&replay_anchor_path).expect("anchor bytes");

        let mut manifest: BackupManifestDocument = serde_json::from_str(include_str!(
            "../../../contracts/fixtures/backup-manifest/valid/empty-pre-rotation-v1.yaml"
        ))
        .expect("manifest fixture");
        let mut materials = vec![
            (BackupEntryKind::ProjectLink, project_link.clone()),
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
        observation
            .schema_version
            .clone_from(&provisioned.anchor.schema_version);
        "operator://replay/cli-restore".clone_into(&mut observation.protected_anchor_identity);
        observation
            .deployment_id
            .clone_from(&provisioned.anchor.deployment_id);
        observation.epoch.clone_from(&provisioned.anchor.epoch);
        observation.generation = provisioned.anchor.generation;
        observation
            .previous_anchor_digest
            .clone_from(&provisioned.anchor.previous_anchor_digest);
        observation.anchor_document_sha256 = digest(&anchor_bytes);
        observation
            .replay_wal_manifest_digest
            .clone_from(&provisioned.anchor.head.manifest_digest);
        observation
            .replay_wal_prefix_digest
            .clone_from(&provisioned.anchor.head.wal_prefix_digest);
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
                "authority_id": "cli-restore-test",
                "receipt_store": receipt_store,
                "replay_anchor_path": replay_anchor_path,
                "protected_anchor_identity": "operator://replay/cli-restore",
                "domain_pack_operator_root": null,
                "domain_pack_operator_root_identity": null
            }]
        });
        fs::write(
            fake_etc.join("forge-core/backup-authorities.json"),
            serde_json::to_vec(&catalog).expect("catalog bytes"),
        )
        .expect("catalog");
        let members = materials
            .into_iter()
            .filter_map(|(kind, bytes)| {
                let entry = manifest
                    .backup_manifest
                    .entries
                    .iter()
                    .find(|entry| entry.material == kind)
                    .expect("member metadata");
                entry
                    .logical_path
                    .strip_prefix("sidecar/")
                    .map(|relative| (relative.to_owned(), bytes))
            })
            .collect();

        Self {
            root,
            project_root,
            destination_sidecar,
            receipt_store,
            archive_path,
            replay_anchor_path,
            fake_etc,
            project_link_sha256: backup.project.project_link_sha256.clone(),
            archive_sha256: digest(&archive_bytes),
            manifest_set_digest: backup.manifest_set_digest.clone(),
            members,
        }
    }

    fn run(&self, action: &str) -> std::process::Output {
        let project_root = self.project_root.to_string_lossy();
        let archive = self.archive_path.to_string_lossy();
        run_with_machine_catalog(
            &self.fake_etc,
            &[
                "restore",
                action,
                "--root",
                project_root.as_ref(),
                "--archive",
                archive.as_ref(),
                "--authority",
                "cli-restore-test",
                "--json",
            ],
        )
    }

    fn assert_success(output: &std::process::Output) -> Value {
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("success JSON envelope")
    }

    fn assert_rejected(output: &std::process::Output) -> Value {
        assert!(
            !output.status.success(),
            "unexpected success: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let envelope: Value = serde_json::from_slice(&output.stdout).expect("rejection JSON");
        assert_eq!(envelope["ok"], false);
        envelope
    }
}

impl Drop for MachineFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn replacement_machine_restore_preserves_project_link_and_publishes_durable_project_receipt() {
    let fixture = MachineFixture::new("replacement-machine");
    assert!(!fixture.destination_sidecar.exists());
    let original_link =
        fs::read(fixture.project_root.join(".forge-method.yaml")).expect("original Project Link");

    let preflight = MachineFixture::assert_success(&fixture.run("preflight"));
    assert_eq!(preflight["data"]["destination_already_published"], false);
    let applied = MachineFixture::assert_success(&fixture.run("apply"));
    assert_eq!(applied["data"]["restored"], true);
    assert_eq!(applied["data"]["already_restored"], false);
    assert_eq!(
        fs::read(fixture.project_root.join(".forge-method.yaml")).expect("preserved Project Link"),
        original_link
    );
    for (relative, bytes) in &fixture.members {
        assert_eq!(
            fs::read(fixture.destination_sidecar.join(relative)).expect("restored member"),
            bytes.as_slice(),
            "{relative} must restore exact bytes"
        );
    }

    let receipt_path = PathBuf::from(
        applied["data"]["receipt_path"]
            .as_str()
            .expect("receipt path"),
    );
    let receipt: Value = serde_json::from_slice(&fs::read(&receipt_path).expect("restore receipt"))
        .expect("restore receipt JSON");
    assert_eq!(receipt["restore_receipt"]["project_id"], "app");
    assert_eq!(
        receipt["restore_receipt"]["project_link_sha256"],
        fixture.project_link_sha256
    );
    assert_eq!(
        receipt["restore_receipt"]["archive_sha256"],
        fixture.archive_sha256
    );
    assert_eq!(
        receipt["restore_receipt"]["manifest_set_digest"],
        fixture.manifest_set_digest
    );
    assert_eq!(
        receipt["restore_receipt"]["destination_sidecar"],
        fixture.destination_sidecar.display().to_string()
    );

    let repeated = MachineFixture::assert_success(&fixture.run("apply"));
    assert_eq!(repeated["data"]["already_restored"], true);
    assert_eq!(
        repeated["data"]["receipt_path"],
        applied["data"]["receipt_path"]
    );
    assert_eq!(
        repeated["data"]["receipt_digest"],
        applied["data"]["receipt_digest"]
    );
}

#[test]
fn restore_rejects_cross_project_substitution_without_replacing_project_link() {
    let fixture = MachineFixture::new("cross-project");
    let substituted = br"schema_version: forge_project_link_v1
project_id: other-project
sidecar_root: ../forge-app
state_root: ../forge-app/.forge-method
";
    fs::write(fixture.project_root.join(".forge-method.yaml"), substituted)
        .expect("substituted Project Link");

    let rejected = MachineFixture::assert_rejected(&fixture.run("preflight"));
    assert!(rejected["error"]["message"]
        .as_str()
        .expect("message")
        .contains("Project Link differs"));
    assert_eq!(
        fs::read(fixture.project_root.join(".forge-method.yaml")).expect("Project Link retained"),
        substituted
    );
    assert!(!fixture.destination_sidecar.exists());
}

#[test]
fn restore_diagnoses_backup_rollback_against_newer_protected_anchor() {
    let fixture = MachineFixture::new("rollback");
    let mut anchor: Value = serde_json::from_slice(
        &fs::read(&fixture.replay_anchor_path).expect("protected replay anchor"),
    )
    .expect("anchor JSON");
    anchor["generation"] = Value::from(
        anchor["generation"]
            .as_u64()
            .expect("generation")
            .saturating_add(1),
    );
    fs::write(
        &fixture.replay_anchor_path,
        serde_json::to_vec(&anchor).expect("anchor bytes"),
    )
    .expect("advance protected anchor");

    let rejected = MachineFixture::assert_rejected(&fixture.run("preflight"));
    assert!(rejected["error"]["message"]
        .as_str()
        .expect("message")
        .contains("older than current protected authority"));
    assert!(!fixture.destination_sidecar.exists());
}

#[test]
fn restore_destination_collision_preserves_operator_owned_bytes() {
    let fixture = MachineFixture::new("destination-collision");
    fs::write(&fixture.destination_sidecar, b"operator-owned").expect("colliding destination");

    let rejected = MachineFixture::assert_rejected(&fixture.run("preflight"));
    assert!(
        rejected["error"]["message"]
            .as_str()
            .expect("message")
            .contains("restore collision"),
        "collision rejection: {rejected:#}"
    );
    assert_eq!(
        fs::read(&fixture.destination_sidecar).expect("collision retained"),
        b"operator-owned"
    );
}

#[test]
fn interrupted_restore_with_exact_journal_and_partial_staging_resumes() {
    let fixture = MachineFixture::new("resume-partial-staging");
    let token = fixture
        .archive_sha256
        .strip_prefix("sha256:")
        .expect("archive digest token");
    let staging = fixture
        .root
        .join(format!(".forge-app.forge-restore-{token}"));
    let journal = fixture
        .receipt_store
        .join("restore-journals/app")
        .join(format!("{token}.json"));
    let (first_relative, first_bytes) = fixture.members.first().expect("one restore member");
    let first_path = staging.join(first_relative);
    fs::create_dir_all(first_path.parent().expect("member parent")).expect("partial staging");
    fs::write(&first_path, first_bytes).expect("first staged member");
    fs::create_dir_all(journal.parent().expect("journal parent")).expect("journal directory");
    fs::write(
        &journal,
        serde_json::to_vec(&serde_json::json!({
            "schema_version": "forge_project_state_restore_journal_v2",
            "operation_nonce": "0123456789abcdef0123456789abcdef",
            "project_id": "app",
            "project_link_sha256": fixture.project_link_sha256,
            "archive_sha256": fixture.archive_sha256,
            "manifest_set_digest": fixture.manifest_set_digest,
            "destination_sidecar": fixture.destination_sidecar.display().to_string(),
            "staging_path": staging.display().to_string()
        }))
        .expect("journal bytes"),
    )
    .expect("interrupted journal");

    let applied = MachineFixture::assert_success(&fixture.run("apply"));
    assert_eq!(applied["data"]["restored"], true);
    assert!(fixture.destination_sidecar.is_dir());
    assert!(!staging.exists());
    let retained_journal: Value =
        serde_json::from_slice(&fs::read(&journal).expect("retained restore journal"))
            .expect("retained restore journal JSON");
    assert_eq!(
        retained_journal["schema_version"],
        "forge_project_state_restore_journal_v2"
    );
    assert_eq!(
        retained_journal["operation_nonce"],
        "0123456789abcdef0123456789abcdef"
    );
}

#[test]
fn tampered_archive_and_restore_receipt_fail_closed() {
    let fixture = MachineFixture::new("tampered-material");
    let mut archive = fs::read(&fixture.archive_path).expect("archive bytes");
    *archive.last_mut().expect("nonempty archive") ^= 1;
    fs::write(&fixture.archive_path, archive).expect("tampered archive");
    MachineFixture::assert_rejected(&fixture.run("preflight"));

    let receipt_fixture = MachineFixture::new("tampered-restore-receipt");
    let applied = MachineFixture::assert_success(&receipt_fixture.run("apply"));
    let receipt_path = PathBuf::from(
        applied["data"]["receipt_path"]
            .as_str()
            .expect("receipt path"),
    );
    let mut receipt = fs::read(&receipt_path).expect("restore receipt bytes");
    *receipt.last_mut().expect("nonempty receipt") ^= 1;
    fs::write(&receipt_path, receipt).expect("tamper restore receipt");
    MachineFixture::assert_rejected(&receipt_fixture.run("preflight"));
}
