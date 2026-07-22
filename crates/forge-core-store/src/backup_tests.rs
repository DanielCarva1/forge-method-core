use super::*;
use crate::replay_anchor::{provision_replay_anchor, ReplayAnchorDocument};
use crate::replay_wal::{
    initialize_replay_wal, replay_wal_manifest_path, replay_wal_path, reserve_replay_nonce,
};
use forge_core_contracts::{BackupEntryKind, PrincipalId};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("forge-backup-{label}-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn snapshot() -> CapturedBackupSnapshot {
    let fixture = include_str!(
        "../../../contracts/fixtures/backup-manifest/valid/empty-pre-rotation-v1.yaml"
    );
    let mut manifest: BackupManifestDocument =
        serde_json::from_str(fixture).expect("parse fixture");
    let link =
        include_bytes!("../../../contracts/fixtures/backup-manifest/valid/project-link.yaml")
            .to_vec();
    let materials = [
        (BackupEntryKind::ProjectLink, link),
        (BackupEntryKind::RootLedger, Vec::new()),
        (BackupEntryKind::ReplayWalManifest, b"manifest-v1".to_vec()),
        (BackupEntryKind::ReplayWal, Vec::new()),
    ];
    let mut members = Vec::new();
    for (kind, bytes) in materials {
        let entry = manifest
            .backup_manifest
            .entries
            .iter_mut()
            .find(|entry| entry.material == kind)
            .expect("fixture entry");
        entry.byte_length = bytes.len() as u64;
        entry.sha256 = sha256(&bytes);
        if kind == BackupEntryKind::ProjectLink {
            manifest.backup_manifest.project.project_link_sha256 = entry.sha256.clone();
        }
        members.push(CapturedBackupMember {
            entry: entry.clone(),
            bytes,
        });
    }
    let replay_manifest = manifest
        .backup_manifest
        .entries
        .iter()
        .find(|entry| entry.material == BackupEntryKind::ReplayWalManifest)
        .expect("replay manifest")
        .sha256
        .clone();
    let replay = manifest
        .backup_manifest
        .entries
        .iter()
        .find(|entry| entry.material == BackupEntryKind::ReplayWal)
        .expect("replay WAL")
        .clone();
    let anchor = &mut manifest
        .backup_manifest
        .external_authority_observations
        .replay_rollback_anchor;
    anchor.replay_wal_manifest_digest = replay_manifest;
    anchor.replay_wal_prefix_digest = replay.sha256;
    anchor.replay_wal_byte_length = replay.byte_length;
    manifest.backup_manifest.manifest_set_digest = manifest.set_digest().expect("set digest");
    members.sort_by(|left, right| {
        (left.entry.material, &left.entry.logical_path)
            .cmp(&(right.entry.material, &right.entry.logical_path))
    });
    CapturedBackupSnapshot { manifest, members }
}

const TEST_AUTHORITY_ID: &str = "backup-test-authority";
const TEST_REPLAY_IDENTITY: &str = "operator://replay/backup-test";

struct VerificationFixture {
    root: TempDir,
    project_root: PathBuf,
    state_root: PathBuf,
    archive_path: PathBuf,
    authority: TrustedBackupAuthority,
}

fn replace_snapshot_member(
    captured: &mut CapturedBackupSnapshot,
    kind: BackupEntryKind,
    bytes: Vec<u8>,
) {
    let entry = captured
        .manifest
        .backup_manifest
        .entries
        .iter_mut()
        .find(|entry| entry.material == kind)
        .expect("manifest entry");
    entry.byte_length = bytes.len() as u64;
    entry.sha256 = sha256(&bytes);
    let member = captured
        .members
        .iter_mut()
        .find(|member| member.entry.material == kind)
        .expect("archive member");
    member.entry = entry.clone();
    member.bytes = bytes;
}

fn verification_fixture(label: &str) -> VerificationFixture {
    let root = TempDir::new(label);
    let project_root = root.0.join("project");
    let state_root = root.0.join("forge-app/.forge-method");
    let operator_root = root.0.join("operator");
    let receipt_store = operator_root.join("receipts");
    fs::create_dir_all(&project_root).expect("project root");
    fs::create_dir_all(&state_root).expect("state root");
    fs::create_dir_all(&receipt_store).expect("receipt store");
    let link_bytes =
        include_bytes!("../../../contracts/fixtures/backup-manifest/valid/project-link.yaml");
    fs::write(project_root.join(".forge-method.yaml"), link_bytes).expect("Project Link");
    initialize_replay_wal(&state_root).expect("initialize replay WAL");
    let replay_anchor_path = operator_root.join("replay-anchor.json");
    let provisioned =
        provision_replay_anchor(&state_root, &replay_anchor_path, "deployment.backup-test")
            .expect("provision replay anchor");

    let mut captured = snapshot();
    replace_snapshot_member(
        &mut captured,
        BackupEntryKind::ReplayWalManifest,
        fs::read(replay_wal_manifest_path(&state_root)).expect("replay manifest"),
    );
    replace_snapshot_member(
        &mut captured,
        BackupEntryKind::ReplayWal,
        fs::read(replay_wal_path(&state_root)).expect("replay WAL"),
    );
    let anchor_bytes = fs::read(&replay_anchor_path).expect("anchor bytes");
    let ReplayAnchorDocument {
        schema_version,
        deployment_id,
        epoch,
        generation,
        previous_anchor_digest,
        head,
    } = provisioned.anchor;
    let observation = &mut captured
        .manifest
        .backup_manifest
        .external_authority_observations
        .replay_rollback_anchor;
    observation.schema_version = schema_version;
    observation.protected_anchor_identity = TEST_REPLAY_IDENTITY.to_owned();
    observation.deployment_id = deployment_id;
    observation.epoch = epoch;
    observation.generation = generation;
    observation.previous_anchor_digest = previous_anchor_digest;
    observation.anchor_document_sha256 = sha256(&anchor_bytes);
    observation.replay_wal_manifest_digest = head.manifest_digest;
    observation.replay_wal_prefix_digest = head.wal_prefix_digest;
    observation.replay_wal_last_seq = head.last_seq;
    observation.replay_wal_record_count = head.record_count as u64;
    observation.replay_wal_byte_length = head.byte_len;
    captured.manifest.backup_manifest.manifest_set_digest =
        captured.manifest.set_digest().expect("manifest set digest");

    let archive_path = root.0.join("archives/project.forge-backup");
    publish_captured_snapshot(&captured, &archive_path, &receipt_store)
        .expect("publish protected backup");
    let authority = TrustedBackupAuthority {
        authority_id: TEST_AUTHORITY_ID.to_owned(),
        receipt_store: fs::canonicalize(receipt_store).expect("canonical receipt store"),
        replay_anchor_path: fs::canonicalize(replay_anchor_path).expect("canonical replay anchor"),
        protected_anchor_identity: TEST_REPLAY_IDENTITY.to_owned(),
        domain_pack_operator: None,
    };
    VerificationFixture {
        root,
        project_root,
        state_root,
        archive_path,
        authority,
    }
}

fn verification_request(fixture: &VerificationFixture) -> BackupVerifyRequest {
    BackupVerifyRequest {
        project_root: fixture.project_root.clone(),
        archive_path: fixture.archive_path.clone(),
        authority_id: TEST_AUTHORITY_ID.to_owned(),
        current_principal_registry: None,
        current_broker_registry: None,
    }
}

#[test]
fn domain_pack_generation_catalog_is_authoritative_backup_material() {
    let fixture =
        include_str!("../../../contracts/fixtures/backup-manifest/valid/multi-generation-v1.yaml");
    let manifest: BackupManifestDocument = serde_json::from_str(fixture).expect("parse fixture");
    let catalog = manifest
        .backup_manifest
        .entries
        .iter()
        .find(|entry| entry.material == BackupEntryKind::DomainPackGenerationCatalog)
        .expect("catalog entry");
    assert_eq!(
        classify_authoritative_member(
            &catalog.logical_path,
            &manifest.backup_manifest.project.archive_layout,
        ),
        Some(BackupEntryKind::DomainPackGenerationCatalog)
    );
}

#[test]
fn machine_catalog_resolution_mints_only_an_opaque_canonical_capability() {
    let fixture = verification_fixture("catalog-resolution");
    let catalog_path = fixture.root.0.join("backup-authorities.json");
    let catalog = serde_json::json!({
        "schema_version": BACKUP_AUTHORITY_CATALOG_SCHEMA_VERSION,
        "authorities": [{
            "authority_id": TEST_AUTHORITY_ID,
            "receipt_store": fixture.authority.receipt_store,
            "replay_anchor_path": fixture.authority.replay_anchor_path,
            "protected_anchor_identity": TEST_REPLAY_IDENTITY,
            "domain_pack_operator_root": null,
            "domain_pack_operator_root_identity": null
        }]
    });
    fs::write(
        &catalog_path,
        serde_json::to_vec(&catalog).expect("serialize catalog"),
    )
    .expect("write catalog");
    let resolved = resolve_backup_authority_from_catalog(TEST_AUTHORITY_ID, &catalog_path)
        .expect("resolve configured authority");
    assert_eq!(resolved.authority_id, TEST_AUTHORITY_ID);
    verify_project_backup_with_authority(&verification_request(&fixture), &resolved)
        .expect("resolved capability verifies");
}
#[test]
fn configured_capability_verifies_but_public_api_cannot_select_attacker_receipts() {
    let fixture = verification_fixture("configured-capability");
    let request = verification_request(&fixture);
    verify_project_backup_with_authority(&request, &fixture.authority)
        .expect("configured authority verifies");

    let mut attacker_request = request;
    attacker_request.authority_id = format!("attacker-{}", std::process::id());
    let rejection = verify_project_backup(&attacker_request)
        .expect_err("public API must not accept an attacker receipt-store path");
    assert!(matches!(
        rejection,
        BackupError::InvalidPath { .. } | BackupError::Io { .. } | BackupError::Receipt { .. }
    ));
}

#[test]
fn replay_identity_mismatch_and_unanchored_wal_append_are_rejected() {
    let fixture = verification_fixture("replay-current");
    let request = verification_request(&fixture);
    let mut wrong_identity = TrustedBackupAuthority {
        authority_id: fixture.authority.authority_id.clone(),
        receipt_store: fixture.authority.receipt_store.clone(),
        replay_anchor_path: fixture.authority.replay_anchor_path.clone(),
        protected_anchor_identity: "operator://replay/substituted".to_owned(),
        domain_pack_operator: None,
    };
    let identity_error = verify_project_backup_with_authority(&request, &wrong_identity)
        .expect_err("configured replay identity mismatch must fail");
    assert!(identity_error.to_string().contains("replay identity"));

    wrong_identity.protected_anchor_identity = TEST_REPLAY_IDENTITY.to_owned();
    reserve_replay_nonce(
        &fixture.state_root,
        &PrincipalId("principal.backup-test".to_owned()),
        "forge://backup/test",
        "backup-nonce-000001",
        &format!("sha256:{}", "a".repeat(64)),
        &format!("sha256:{}", "b".repeat(64)),
    )
    .expect("append replay WAL without advancing anchor");
    let stale_error = verify_project_backup_with_authority(&request, &wrong_identity)
        .expect_err("AdvanceRequired must reject an otherwise valid old backup");
    assert!(stale_error.to_string().contains("advance required"));
}

#[test]
fn configured_domain_pack_operator_identity_is_exact() {
    let root = TempDir::new("domain-identity");
    let receipt_store = root.0.join("receipts");
    fs::create_dir_all(&receipt_store).expect("receipt store");
    let archive_path = root.0.join("archives/project.forge-backup");
    publish_captured_snapshot(&snapshot(), &archive_path, &receipt_store).expect("publish");
    let mut verified = verify_backup_archive(&archive_path, &receipt_store).expect("verify");
    let provisioned: BackupManifestDocument = serde_json::from_str(include_str!(
        "../../../contracts/fixtures/backup-manifest/valid/no-active-provisioned-v1.yaml"
    ))
    .expect("provisioned fixture");
    verified.receipt.backup_receipt.domain_pack_supply_chain = provisioned
        .backup_manifest
        .external_authority_observations
        .domain_pack_supply_chain;
    verified
        .receipt
        .backup_receipt
        .domain_pack_reviewed_learning = provisioned
        .backup_manifest
        .external_authority_observations
        .domain_pack_reviewed_learning;
    let operator = ConfiguredDomainPackOperator {
        root: fs::canonicalize(&root.0).expect("canonical operator root"),
        root_identity: "operator://domain-pack/substituted".to_owned(),
    };
    let rejection = verify_domain_pack_authorities(&verified, Some(&operator))
        .expect_err("Domain Pack operator-root identity mismatch must fail");
    assert!(rejection.to_string().contains("operator-root identity"));
}

#[test]
fn durable_archive_then_protected_receipt_round_trip_and_idempotence() {
    let root = TempDir::new("round-trip");
    let archive_dir = root.0.join("archives");
    let receipt_dir = root.0.join("receipts");
    fs::create_dir_all(&receipt_dir).expect("receipt dir");
    let archive = archive_dir.join("project.forge-backup");
    let captured = snapshot();

    let first =
        publish_captured_snapshot(&captured, &archive, &receipt_dir).expect("publish backup");
    assert!(!first.already_published);
    assert!(archive.is_file());
    assert!(first.receipt_path.is_file());
    let verified = verify_backup_archive(&archive, &receipt_dir).expect("verify backup");
    assert_eq!(verified.member_count(), 4);
    assert_eq!(verified.archive_sha256(), first.archive_sha256);

    let second =
        publish_captured_snapshot(&captured, &archive, &receipt_dir).expect("idempotent publish");
    assert!(second.already_published);
    assert_eq!(second.archive_sha256, first.archive_sha256);
}

#[test]
fn archive_orphan_retry_completes_receipt_but_receipt_without_archive_fails_closed() {
    let root = TempDir::new("orphan-recovery");
    let archive = root.0.join("archives/project.forge-backup");
    let receipt_dir = root.0.join("receipts");
    fs::create_dir_all(&receipt_dir).expect("receipt dir");
    let captured = snapshot();
    let published =
        publish_captured_snapshot(&captured, &archive, &receipt_dir).expect("publish backup");

    fs::remove_file(&published.receipt_path).expect("simulate archive-before-receipt crash");
    let recovered = publish_captured_snapshot(&captured, &archive, &receipt_dir)
        .expect("complete orphaned archive");
    assert!(recovered.already_published);
    assert!(recovered.receipt_path.is_file());

    fs::remove_file(&archive).expect("simulate impossible receipt-first state");
    assert!(verify_backup_archive(&archive, &receipt_dir).is_err());
    assert!(matches!(
        publish_captured_snapshot(&captured, &archive, &receipt_dir),
        Err(BackupError::ExistingDifferent { .. })
    ));
}

#[test]
fn existing_different_archive_is_never_overwritten() {
    let root = TempDir::new("existing-different");
    let archive = root.0.join("archives/project.forge-backup");
    let receipt_dir = root.0.join("receipts");
    fs::create_dir_all(archive.parent().expect("archive parent")).expect("archive dir");
    fs::create_dir_all(&receipt_dir).expect("receipt dir");
    fs::write(&archive, b"operator-owned-existing-bytes").expect("existing archive");
    assert!(publish_captured_snapshot(&snapshot(), &archive, &receipt_dir).is_err());
    assert_eq!(
        fs::read(&archive).expect("existing archive retained"),
        b"operator-owned-existing-bytes"
    );
}
#[test]
fn archive_and_receipt_tampering_fail_closed() {
    let root = TempDir::new("tamper");
    let archive_dir = root.0.join("archives");
    let receipt_dir = root.0.join("receipts");
    fs::create_dir_all(&receipt_dir).expect("receipt dir");
    let archive = archive_dir.join("project.forge-backup");
    let published =
        publish_captured_snapshot(&snapshot(), &archive, &receipt_dir).expect("publish backup");

    let mut bytes = fs::read(&archive).expect("read archive");
    *bytes.last_mut().expect("nonempty archive") ^= 1;
    fs::write(&archive, &bytes).expect("tamper archive");
    assert!(verify_backup_archive(&archive, &receipt_dir).is_err());

    fs::remove_file(&archive).expect("remove tampered archive");
    fs::remove_file(&published.receipt_path).expect("remove receipt");
    let published =
        publish_captured_snapshot(&snapshot(), &archive, &receipt_dir).expect("republish backup");
    let mut receipt = fs::read(&published.receipt_path).expect("read receipt");
    *receipt.last_mut().expect("nonempty receipt") ^= 1;
    fs::write(&published.receipt_path, receipt).expect("tamper receipt");
    assert!(verify_backup_archive(&archive, &receipt_dir).is_err());
}

#[test]
fn receipt_store_must_be_disjoint_and_windows_names_fail_before_extraction() {
    let root = TempDir::new("preflight");
    let archive_dir = root.0.join("archives");
    fs::create_dir_all(&archive_dir).expect("archive dir");
    let archive = archive_dir.join("project.forge-backup");
    assert!(publish_captured_snapshot(&snapshot(), &archive, &archive_dir).is_err());

    let entries = vec![
        BackupEntry {
            material: BackupEntryKind::Artifact,
            logical_path: "sidecar/.forge-method/artifacts/Name.yaml".to_owned(),
            entry_type: BackupArchiveEntryType::RegularFile,
            byte_length: 0,
            sha256: sha256(&[]),
        },
        BackupEntry {
            material: BackupEntryKind::Artifact,
            logical_path: "sidecar/.forge-method/artifacts/name.yaml".to_owned(),
            entry_type: BackupArchiveEntryType::RegularFile,
            byte_length: 0,
            sha256: sha256(&[]),
        },
    ];
    assert!(preflight_destination_names(&entries, BackupDestinationPlatform::Posix).is_ok());
    assert!(preflight_destination_names(&entries, BackupDestinationPlatform::Windows).is_err());
}

#[cfg(unix)]
#[test]
fn hardlinked_archive_is_never_trusted() {
    let root = TempDir::new("hardlink");
    let archive_dir = root.0.join("archives");
    let receipt_dir = root.0.join("receipts");
    fs::create_dir_all(&receipt_dir).expect("receipt dir");
    let archive = archive_dir.join("project.forge-backup");
    publish_captured_snapshot(&snapshot(), &archive, &receipt_dir).expect("publish backup");
    fs::hard_link(&archive, archive_dir.join("alias.forge-backup")).expect("hard link");
    assert!(verify_backup_archive(&archive, &receipt_dir).is_err());
}

#[test]
fn bounded_reader_rejects_oversized_content_without_unbounded_allocation() {
    let root = TempDir::new("bounded-read");
    let path = root.0.join("large.bin");
    fs::write(&path, vec![0_u8; 65]).expect("write fixture");
    assert!(matches!(
        read_file_bounded(&path, 64),
        Err(BackupError::ResourceLimit { maximum: 64, .. })
    ));
}

#[cfg(unix)]
#[test]
fn nofollow_reader_rejects_symbolic_links() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new("nofollow-read");
    let target = root.0.join("target.bin");
    let alias = root.0.join("alias.bin");
    fs::write(&target, b"secret").expect("write target");
    symlink(&target, &alias).expect("create symlink");
    assert!(open_nofollow_read(&alias).is_err());
    assert!(read_file_bounded(&alias, 64).is_err());
}

#[test]
fn complete_source_walk_captures_every_public_file_and_never_reads_private_key_material() {
    let root = TempDir::new("complete-source-walk");
    let project_root = root.0.join("project");
    let sidecar_root = root.0.join("forge-app");
    fs::create_dir_all(&project_root).expect("project root");
    fs::create_dir_all(sidecar_root.join(".forge-method/evidence")).expect("state evidence");
    fs::create_dir_all(sidecar_root.join("operator/private-keys")).expect("private directory");
    fs::write(
        project_root.join(".forge-method.yaml"),
        include_bytes!("../../../contracts/fixtures/backup-manifest/valid/project-link.yaml"),
    )
    .expect("Project Link");
    fs::write(
        sidecar_root.join(".forge-method/ledger.ndjson"),
        b"{\"seq\":1}\n",
    )
    .expect("ledger");
    fs::write(
        sidecar_root.join(".forge-method/evidence/result.json"),
        b"{\"ok\":true}",
    )
    .expect("evidence");
    fs::write(
        sidecar_root.join("operator/private-keys/external-broker.key"),
        [0xA5_u8; 17],
    )
    .expect("private material fixture");

    let manifest: BackupManifestDocument = serde_json::from_str(include_str!(
        "../../../contracts/fixtures/backup-manifest/valid/empty-pre-rotation-v1.yaml"
    ))
    .expect("manifest fixture");
    let files = capture_source_walk(
        &project_root,
        &sidecar_root,
        &manifest.backup_manifest.project.archive_layout,
    )
    .expect("complete source walk");
    let paths = files
        .iter()
        .map(|file| file.metadata.logical_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![
            "project/.forge-method.yaml",
            "sidecar/.forge-method/evidence/result.json",
            "sidecar/.forge-method/ledger.ndjson",
            "sidecar/operator/private-keys/external-broker.key",
        ]
    );
    let private = files
        .iter()
        .find(|file| file.metadata.logical_path.contains("/private-keys/"))
        .expect("private path retained as exclusion metadata");
    assert!(private.bytes.is_none());
    assert_eq!(private.metadata.byte_length, 17);
    assert!(files
        .iter()
        .filter(|file| !file.metadata.logical_path.contains("/private-keys/"))
        .all(|file| file.bytes.is_some()));
}

#[test]
fn archive_publication_rejects_one_omitted_manifest_member() {
    let root = TempDir::new("omitted-member");
    let archive = root.0.join("archives/project.forge-backup");
    let receipt_dir = root.0.join("receipts");
    fs::create_dir_all(&receipt_dir).expect("receipt directory");
    let mut captured = snapshot();
    captured
        .members
        .retain(|member| member.entry.material != BackupEntryKind::ReplayWal);

    let rejection = publish_captured_snapshot(&captured, &archive, &receipt_dir)
        .expect_err("manifest member omission must fail closed");
    assert!(matches!(rejection, BackupError::Archive { .. }));
    assert!(!archive.exists());
}

#[test]
fn configured_verification_rejects_cross_project_link_substitution() {
    let fixture = verification_fixture("cross-project-link");
    let substituted = br"schema_version: forge_project_link_v1
project_id: substituted-project
sidecar_root: ../forge-app
state_root: ../forge-app/.forge-method
";
    fs::write(fixture.project_root.join(".forge-method.yaml"), substituted)
        .expect("substitute Project Link");

    let rejection =
        verify_project_backup_with_authority(&verification_request(&fixture), &fixture.authority)
            .expect_err("cross-project Project Link substitution must fail");
    assert!(matches!(rejection, BackupError::Archive { .. }));
}
