use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("forge-restore-{label}-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp directory");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn manifest() -> BackupManifestDocument {
    serde_json::from_str(include_str!(
        "../../../contracts/fixtures/backup-manifest/valid/empty-pre-rotation-v1.yaml"
    ))
    .expect("manifest fixture")
}

fn member(relative: &str, bytes: &[u8], material: BackupEntryKind) -> RestoreMember {
    RestoreMember {
        entry: BackupEntry {
            material,
            logical_path: format!("sidecar/{relative}"),
            entry_type: forge_core_contracts::BackupArchiveEntryType::RegularFile,
            byte_length: bytes.len() as u64,
            sha256: sha256(bytes),
        },
        relative_destination: PathBuf::from(relative),
        bytes: bytes.to_vec(),
    }
}

#[test]
fn restore_member_paths_reject_empty_absolute_parent_and_dot_components() {
    for hostile in ["", "/absolute", "../escape", "a/../escape", "./relative"] {
        assert!(
            normalized_relative_path(hostile).is_err(),
            "{hostile:?} must not become a restore destination"
        );
    }
    assert_eq!(
        normalized_relative_path(".forge-method/wal/replay.fmr1").expect("normalized path"),
        PathBuf::from(".forge-method/wal/replay.fmr1")
    );
}

#[test]
fn atomic_destination_publication_never_replaces_an_existing_entry() {
    let root = TempDir::new("destination-collision");
    let staging = root.0.join("staging");
    let destination = root.0.join("forge-app");
    fs::create_dir_all(&staging).expect("staging directory");
    fs::write(staging.join("member"), b"restored").expect("staged member");
    fs::write(&destination, b"operator-owned").expect("colliding destination");

    let rejection = publish_directory_create_new(&staging, &destination)
        .expect_err("destination collision must fail closed");
    assert!(matches!(rejection, RestoreError::Collision { .. }));
    assert_eq!(
        fs::read(&destination).expect("destination retained"),
        b"operator-owned"
    );
    assert!(staging.is_dir());
}

#[test]
fn interrupted_staging_accepts_only_an_exact_member_prefix() {
    let root = TempDir::new("staging-prefix");
    let staging = root.0.join("staging");
    fs::create_dir_all(staging.join(".forge-method/wal")).expect("staging tree");
    let members = vec![
        member(
            ".forge-method/ledger.ndjson",
            b"{\"seq\":1}\n",
            BackupEntryKind::RootLedger,
        ),
        member(
            ".forge-method/wal/replay.fmr1",
            b"replay",
            BackupEntryKind::ReplayWal,
        ),
    ];
    fs::write(
        staging.join(".forge-method/ledger.ndjson"),
        &members[0].bytes,
    )
    .expect("partial exact member");
    verify_staging_prefix(&staging, &members).expect("exact interrupted prefix is resumable");

    fs::write(staging.join(".forge-method/unexpected"), b"hostile")
        .expect("unexpected staged file");
    assert!(matches!(
        verify_staging_prefix(&staging, &members),
        Err(RestoreError::Interrupted { .. })
    ));
    fs::remove_file(staging.join(".forge-method/unexpected")).expect("remove extra file");

    fs::write(
        staging.join(".forge-method/wal/replay.fmr1"),
        b"substituted",
    )
    .expect("substituted staged member");
    assert!(matches!(
        verify_staging_prefix(&staging, &members),
        Err(RestoreError::Interrupted { .. })
    ));
}

#[test]
fn exact_destination_rejects_omission_and_unclassified_residue() {
    let root = TempDir::new("destination-exactness");
    let destination = root.0.join("forge-app");
    fs::create_dir_all(destination.join(".forge-method/wal")).expect("destination tree");
    let members = vec![
        member(
            ".forge-method/ledger.ndjson",
            b"{\"seq\":1}\n",
            BackupEntryKind::RootLedger,
        ),
        member(
            ".forge-method/wal/replay.fmr1",
            b"replay",
            BackupEntryKind::ReplayWal,
        ),
    ];
    fs::write(
        destination.join(".forge-method/ledger.ndjson"),
        &members[0].bytes,
    )
    .expect("one restored member");
    assert!(verify_sidecar_exact(&destination, &members, &manifest()).is_err());

    fs::write(
        destination.join(".forge-method/wal/replay.fmr1"),
        &members[1].bytes,
    )
    .expect("second restored member");
    fs::write(destination.join(".forge-method/forged.json"), b"{}").expect("unclassified residue");
    assert!(matches!(
        verify_sidecar_exact(&destination, &members, &manifest()),
        Err(RestoreError::Collision { .. })
    ));
}

#[test]
fn protected_journal_collision_requires_identical_transaction_bindings() {
    let root = TempDir::new("journal-collision");
    let path = root.0.join("receipts/restore-journals/app/archive.json");
    let expected = RestoreJournalDocument {
        schema_version: RESTORE_JOURNAL_SCHEMA_VERSION.to_owned(),
        operation_nonce: "0123456789abcdef0123456789abcdef".to_owned(),
        project_id: "app".to_owned(),
        project_link_sha256: format!("sha256:{}", "1".repeat(64)),
        archive_sha256: format!("sha256:{}", "2".repeat(64)),
        manifest_set_digest: format!("sha256:{}", "3".repeat(64)),
        destination_sidecar: root.0.join("forge-app").display().to_string(),
        staging_path: root.0.join("staging").display().to_string(),
    };
    publish_or_validate_journal(&path, &expected).expect("publish journal");
    publish_or_validate_journal(&path, &expected).expect("identical journal is idempotent");

    let mut substituted = expected.clone();
    substituted.project_id = "other-project".to_owned();
    fs::write(
        &path,
        serde_json::to_vec(&substituted).expect("encode substituted journal"),
    )
    .expect("substitute journal");
    assert!(matches!(
        publish_or_validate_journal(&path, &expected),
        Err(RestoreError::Interrupted { .. })
    ));
}

#[test]
fn restore_receipt_digest_is_project_destination_and_replay_bound() {
    let backup = manifest().backup_manifest;
    let mut receipt = RestoreReceiptDocument {
        schema_version: RESTORE_RECEIPT_SCHEMA_VERSION.to_owned(),
        restore_receipt: RestoreReceipt {
            operation_nonce: "0123456789abcdef0123456789abcdef".to_owned(),
            archive_sha256: format!("sha256:{}", "a".repeat(64)),
            backup_receipt_digest: format!("sha256:{}", "b".repeat(64)),
            manifest_set_digest: backup.manifest_set_digest,
            project_id: backup.project.project_link.project_id.0,
            project_link_sha256: backup.project.project_link_sha256,
            workflow_release: backup.workflow_release,
            effective_bundle: backup.effective_epoch.effective_bundle,
            replay_monotonic_head: backup
                .external_authority_observations
                .replay_rollback_anchor,
            destination_sidecar: "/replacement/forge-app".to_owned(),
            sidecar_root_path_sha256: format!("sha256:{}", "c".repeat(64)),
            state_root_path_sha256: format!("sha256:{}", "d".repeat(64)),
            sidecar_inventory_digest: format!("sha256:{}", "e".repeat(64)),
            restored_at_unix: 1,
            receipt_digest: String::new(),
        },
    };
    receipt.restore_receipt.receipt_digest =
        restore_receipt_digest(&receipt).expect("receipt digest");
    validate_same_restore_receipt(&receipt, &receipt).expect("self-bound receipt");

    let mutations: [fn(&mut RestoreReceiptDocument); 5] = [
        |value| {
            value.restore_receipt.operation_nonce = "fedcba9876543210fedcba9876543210".to_owned();
        },
        |value| value.restore_receipt.project_id = "other-project".to_owned(),
        |value| value.restore_receipt.destination_sidecar = "/other/forge-app".to_owned(),
        |value| {
            value.restore_receipt.sidecar_root_path_sha256 = format!("sha256:{}", "f".repeat(64));
        },
        |value| value.restore_receipt.replay_monotonic_head.generation += 1,
    ];
    for mutate in mutations {
        let mut tampered = receipt.clone();
        mutate(&mut tampered);
        assert!(matches!(
            validate_same_restore_receipt(&receipt, &tampered),
            Err(RestoreError::Tampered { .. })
        ));
    }
    let mut rebound = receipt.clone();
    rebound.restore_receipt.sidecar_root_path_sha256 = format!("sha256:{}", "f".repeat(64));
    rebound.restore_receipt.receipt_digest = String::new();
    rebound.restore_receipt.receipt_digest =
        restore_receipt_digest(&rebound).expect("rebound receipt digest");
    assert!(matches!(
        validate_same_restore_receipt(&receipt, &rebound),
        Err(RestoreError::Tampered { .. })
    ));
}

fn completion_authority_document() -> RestoreCompletionAuthorityDocument {
    let backup = manifest().backup_manifest;
    RestoreCompletionAuthorityDocument {
        schema_version: RESTORE_COMPLETION_AUTHORITY_SCHEMA_VERSION.to_owned(),
        operation_nonce: "0123456789abcdef0123456789abcdef".to_owned(),
        project_id: backup.project.project_link.project_id.0,
        workflow_release: backup.workflow_release,
        effective_bundle: backup.effective_epoch.effective_bundle,
        source: RestoreCompletionSourceIdentity {
            archive_sha256: format!("sha256:{}", "a".repeat(64)),
            backup_receipt_digest: format!("sha256:{}", "b".repeat(64)),
            manifest_set_digest: backup.manifest_set_digest,
            backup_created_at_unix: 7,
        },
        protected_authority_root: RestoreRootPathBinding {
            configured_path: "/authority".to_owned(),
            configured_path_sha256: format!("sha256:{}", "0".repeat(64)),
        },
        sidecar: RestoreCompletionSidecarBinding {
            destination_sidecar: "/sidecar".to_owned(),
            retained_parent: RestoreRootPathBinding {
                configured_path: "/".to_owned(),
                configured_path_sha256: format!("sha256:{}", "1".repeat(64)),
            },
            root_leaf: "sidecar".to_owned(),
            root_path_sha256: format!("sha256:{}", "3".repeat(64)),
            state_root_relative: DESTINATION_STATE_LEAF.to_owned(),
            state_root_path_sha256: format!("sha256:{}", "4".repeat(64)),
            inventory_digest: format!("sha256:{}", "5".repeat(64)),
            inventory: vec![RestoreCompletionInventoryEntry::File {
                relative_path: ".forge-method/ledger.ndjson".to_owned(),
                byte_length: 3,
                sha256: format!("sha256:{}", "7".repeat(64)),
            }],
        },
        journal: RestoreProtectedDocumentBinding {
            relative_path: "restore-journals/app/archive.json".to_owned(),
            parent_relative: "restore-journals/app".to_owned(),
            content_sha256: format!("sha256:{}", "a".repeat(64)),
        },
        receipt: RestoreProtectedDocumentBinding {
            relative_path: "restores/app/archive.json".to_owned(),
            parent_relative: "restores/app".to_owned(),
            content_sha256: format!("sha256:{}", "d".repeat(64)),
        },
        project_link: RestoreProjectLinkBinding {
            project_root: RestoreRootPathBinding {
                configured_path: "/project".to_owned(),
                configured_path_sha256: format!("sha256:{}", "d".repeat(64)),
            },
            leaf: PROJECT_LINK_LEAF.to_owned(),
            content_sha256: format!("sha256:{}", "0".repeat(64)),
        },
        replay_anchor: RestoreReplayAnchorBinding {
            configured_root: RestoreRootPathBinding {
                configured_path: "/replay-root".to_owned(),
                configured_path_sha256: format!("sha256:{}", "0".repeat(64)),
            },
            parent_relative: "authority".to_owned(),
            lock_leaf: "anchor.json.lock".to_owned(),
            anchor_leaf: "anchor.json".to_owned(),
            anchor_digest: format!("sha256:{}", "5".repeat(64)),
        },
        transaction: RestoreTransactionAuthorityBinding {
            lock_relative: "restore-locks/app.lock".to_owned(),
        },
        quiescence: RestoreQuiescenceBinding {
            destination_state: "/sidecar/.forge-method".to_owned(),
            destination_state_path_sha256: format!("sha256:{}", "4".repeat(64)),
        },
    }
}

#[test]
fn restore_completion_authority_is_content_addressed_and_operation_bound() {
    let authority = completion_authority_document();
    validate_same_restore_completion_authority(&authority, &authority)
        .expect("self-bound completion authority");
    let bytes = canonical_restore_completion_authority_bytes(&authority)
        .expect("encode canonical completion authority");
    let relative =
        completion_authority_relative_path(Path::new("restore-completions/app/archive"), &bytes)
            .expect("content address");
    let digest = sha256(&bytes);
    assert_eq!(
        relative.file_stem().and_then(|value| value.to_str()),
        digest_token(&digest).ok()
    );

    let mut other_operation = authority.clone();
    other_operation.operation_nonce = "fedcba9876543210fedcba9876543210".to_owned();
    assert!(matches!(
        validate_same_restore_completion_authority(&authority, &other_operation),
        Err(RestoreError::Tampered { .. })
    ));
    let other_bytes = serde_json::to_vec(&other_operation).expect("encode other operation");
    assert_ne!(
        relative,
        completion_authority_relative_path(
            Path::new("restore-completions/app/archive"),
            &other_bytes,
        )
        .expect("other content address")
    );
}

#[test]
fn atomic_destination_publication_rejects_caller_created_matching_tree() {
    let root = TempDir::new("matching-destination-collision");
    let staging = root.0.join("staging");
    let destination = root.0.join("forge-app");
    fs::create_dir_all(&staging).expect("staging directory");
    fs::create_dir_all(&destination).expect("caller destination directory");
    fs::write(staging.join("member"), b"restored").expect("staged member");
    fs::write(destination.join("member"), b"restored").expect("matching caller member");

    let rejection = publish_directory_create_new(&staging, &destination)
        .expect_err("matching caller-created destination must never be accepted");
    assert!(matches!(rejection, RestoreError::Collision { .. }));
    assert_eq!(
        fs::read(destination.join("member")).expect("caller destination retained"),
        b"restored"
    );
    assert_eq!(
        fs::read(staging.join("member")).expect("Store staging retained"),
        b"restored"
    );
}

#[cfg(unix)]
#[test]
fn exact_sidecar_capability_rejects_same_byte_member_replacement() {
    let root = TempDir::new("retained-sidecar-member");
    let destination = root.0.join("forge-app");
    fs::create_dir_all(destination.join(".forge-method")).expect("destination state tree");
    let members = vec![member(
        ".forge-method/ledger.ndjson",
        b"{\"seq\":1}\n",
        BackupEntryKind::RootLedger,
    )];
    fs::write(
        destination.join(".forge-method/ledger.ndjson"),
        &members[0].bytes,
    )
    .expect("restored member");

    let parent = RestoreRetainedDirectory::open_root(&root.0).expect("retain destination parent");
    let retained_root = parent
        .open_directory(Path::new("forge-app"))
        .expect("retain destination root");
    let state = retained_root
        .open_directory(Path::new(".forge-method"))
        .expect("retain destination state root");
    let capability = verify_sidecar_exact_retained(
        &parent,
        Path::new("forge-app"),
        &destination,
        &members,
        &manifest(),
        &state.identity,
    )
    .expect("retain exact sidecar tree");

    let member_path = destination.join(".forge-method/ledger.ndjson");
    fs::remove_file(&member_path).expect("replace retained member name");
    fs::write(&member_path, &members[0].bytes).expect("write matching replacement bytes");
    assert!(matches!(
        capability.revalidate(),
        Err(RestoreError::Tampered { .. } | RestoreError::Io { .. })
    ));
}

#[cfg(unix)]
#[test]
fn retained_journal_capability_rejects_same_byte_leaf_replacement() {
    let root = TempDir::new("retained-journal-leaf");
    let authority = RestoreRetainedDirectory::open_root(&root.0).expect("retain authority root");
    let relative = PathBuf::from("restore-journals/app/archive.json");
    let path = root.0.join(&relative);
    let expected = RestoreJournalDocument {
        schema_version: RESTORE_JOURNAL_SCHEMA_VERSION.to_owned(),
        operation_nonce: "0123456789abcdef0123456789abcdef".to_owned(),
        project_id: "app".to_owned(),
        project_link_sha256: format!("sha256:{}", "1".repeat(64)),
        archive_sha256: format!("sha256:{}", "2".repeat(64)),
        manifest_set_digest: format!("sha256:{}", "3".repeat(64)),
        destination_sidecar: root.0.join("forge-app").display().to_string(),
        staging_path: root.0.join("staging").display().to_string(),
    };
    let capability = publish_or_validate_journal_retained(&authority, &relative, &path, &expected)
        .expect("publish retained journal");
    let exact_bytes = capability.retained.bytes.clone();

    fs::remove_file(&path).expect("replace retained journal name");
    fs::write(&path, exact_bytes).expect("write matching replacement journal bytes");
    assert!(matches!(
        capability.revalidate(),
        Err(RestoreError::Tampered { .. } | RestoreError::Io { .. })
    ));
}

#[cfg(unix)]
#[test]
fn retained_receipt_capability_rejects_same_byte_leaf_replacement() {
    let root = TempDir::new("retained-receipt-leaf");
    let authority = RestoreRetainedDirectory::open_root(&root.0).expect("retain authority root");
    let relative = PathBuf::from("restores/app/archive.json");
    let path = root.0.join(&relative);
    let backup = manifest().backup_manifest;
    let mut document = RestoreReceiptDocument {
        schema_version: RESTORE_RECEIPT_SCHEMA_VERSION.to_owned(),
        restore_receipt: RestoreReceipt {
            operation_nonce: "0123456789abcdef0123456789abcdef".to_owned(),
            archive_sha256: format!("sha256:{}", "a".repeat(64)),
            backup_receipt_digest: format!("sha256:{}", "b".repeat(64)),
            manifest_set_digest: backup.manifest_set_digest,
            project_id: backup.project.project_link.project_id.0,
            project_link_sha256: backup.project.project_link_sha256,
            workflow_release: backup.workflow_release,
            effective_bundle: backup.effective_epoch.effective_bundle,
            replay_monotonic_head: backup
                .external_authority_observations
                .replay_rollback_anchor,
            destination_sidecar: root.0.join("forge-app").display().to_string(),
            sidecar_root_path_sha256: format!("sha256:{}", "c".repeat(64)),
            state_root_path_sha256: format!("sha256:{}", "d".repeat(64)),
            sidecar_inventory_digest: format!("sha256:{}", "e".repeat(64)),
            restored_at_unix: 1,
            receipt_digest: String::new(),
        },
    };
    document.restore_receipt.receipt_digest =
        restore_receipt_digest(&document).expect("receipt digest");
    let bytes = serde_json::to_vec(&document).expect("receipt bytes");
    let retained = publish_private_file_create_new_retained(&authority, &relative, &path, &bytes)
        .expect("publish retained receipt");
    let capability = RetainedRestoreDocument { retained, document };
    capability
        .revalidate()
        .expect("revalidate retained receipt");
    let exact_bytes = capability.retained.bytes.clone();

    fs::remove_file(&path).expect("replace retained receipt name");
    fs::write(&path, exact_bytes).expect("write matching replacement receipt bytes");
    assert!(matches!(
        capability.revalidate(),
        Err(RestoreError::Tampered { .. } | RestoreError::Io { .. })
    ));
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
#[test]
fn failed_publication_retains_authoritative_placeholder_and_exact_recovery_marker() {
    let root = TempDir::new("publication-recovery-marker");
    let destination = root.0.join("forge-app");
    fs::create_dir(&destination).expect("create published sidecar");
    fs::write(destination.join("member"), b"committed").expect("write committed member");

    let parent = RestoreRetainedDirectory::open_root(&root.0).expect("retain destination parent");
    let committed = parent
        .open_directory(Path::new("forge-app"))
        .expect("retain committed sidecar");
    let isolation =
        restore_isolate_unquiesced_publication(&parent, Path::new("forge-app"), &committed)
            .expect("isolate failed publication");

    isolation
        .revalidate(&parent, Path::new("forge-app"), &committed.identity)
        .expect("revalidate recovery publication");
    let authoritative = parent
        .open_directory(Path::new("forge-app"))
        .expect("authoritative Store placeholder");
    assert_eq!(
        authoritative.identity,
        isolation.authoritative_placeholder.identity
    );
    assert!(
        authoritative
            .direct_entries()
            .expect("read authoritative placeholder")
            .is_empty(),
        "failed publication must not return an attacker-controlled destination"
    );
    assert_eq!(
        fs::read(root.0.join(&isolation.recovery_leaf).join("member"))
            .expect("read discoverable recovery member"),
        b"committed"
    );
}

#[cfg(unix)]
#[test]
fn retained_protected_document_rejects_whole_authority_root_replacement() {
    let root = TempDir::new("retained-protected-root");
    let authority_path = root.0.join("authority");
    fs::create_dir(&authority_path).expect("create authority root");
    let authority =
        RestoreRetainedDirectory::open_root(&authority_path).expect("retain authority root");
    let relative = PathBuf::from("restore-journals/app/archive.json");
    let path = authority_path.join(&relative);
    let expected = RestoreJournalDocument {
        schema_version: RESTORE_JOURNAL_SCHEMA_VERSION.to_owned(),
        operation_nonce: "0123456789abcdef0123456789abcdef".to_owned(),
        project_id: "app".to_owned(),
        project_link_sha256: format!("sha256:{}", "1".repeat(64)),
        archive_sha256: format!("sha256:{}", "2".repeat(64)),
        manifest_set_digest: format!("sha256:{}", "3".repeat(64)),
        destination_sidecar: root.0.join("forge-app").display().to_string(),
        staging_path: root.0.join("staging").display().to_string(),
    };
    let capability = publish_or_validate_journal_retained(&authority, &relative, &path, &expected)
        .expect("publish retained journal");
    let exact_bytes = capability.retained.bytes.clone();

    let displaced = root.0.join("authority-displaced");
    fs::rename(&authority_path, &displaced).expect("displace retained authority root");
    fs::create_dir_all(path.parent().expect("replacement parent"))
        .expect("create replacement authority tree");
    fs::write(&path, exact_bytes).expect("write matching replacement document");

    assert!(matches!(
        capability.revalidate(),
        Err(RestoreError::Tampered { .. } | RestoreError::Io { .. })
    ));
}

#[cfg(unix)]
#[test]
fn retained_replay_anchor_rejects_same_byte_leaf_replacement() {
    let root = TempDir::new("retained-replay-anchor");
    let configured_root_path = root.0.join("configured");
    let parent_relative = PathBuf::from("replay-authority");
    let parent_path = configured_root_path.join(&parent_relative);
    fs::create_dir_all(&parent_path).expect("create replay authority parent");
    let anchor_leaf = PathBuf::from("anchor.json");
    let lock_leaf = PathBuf::from("anchor.json.lock");
    let anchor_path = parent_path.join(&anchor_leaf);
    let lock_path = parent_path.join(&lock_leaf);
    let anchor_bytes = b"retained-anchor".to_vec();
    fs::write(&anchor_path, &anchor_bytes).expect("write replay anchor");
    fs::write(&lock_path, b"").expect("write replay lock");

    let configured_root = RestoreRetainedDirectory::open_root(&configured_root_path)
        .expect("retain replay configured root");
    let parent = configured_root
        .open_directory_path(&parent_relative)
        .expect("retain replay parent");
    let (lock_handle, lock_identity) = parent
        .open_direct_file_read_write_retained(&lock_leaf)
        .expect("retain replay lock leaf");
    let (anchor_handle, anchor_identity) = parent
        .open_direct_file_retained(&anchor_leaf)
        .expect("retain replay anchor leaf");
    let capability = RetainedReplayAnchorAuthority {
        configured_root,
        parent,
        parent_relative,
        anchor_path: anchor_path.clone(),
        anchor_leaf,
        anchor_handle,
        anchor_identity,
        anchor_digest: sha256(&anchor_bytes),
        anchor_bytes: anchor_bytes.clone(),
        lock_path,
        lock_leaf,
        lock_handle,
        lock_identity,
    };
    capability
        .revalidate_filesystem()
        .expect("revalidate retained replay authority");

    fs::remove_file(&anchor_path).expect("replace retained replay anchor name");
    fs::write(&anchor_path, anchor_bytes).expect("write matching replay anchor replacement");
    assert!(matches!(
        capability.revalidate_filesystem(),
        Err(RestoreError::Tampered { .. } | RestoreError::Io { .. })
    ));
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
struct TestRestoreCompletion {
    authority_root: RestoreRetainedDirectory,
    completion_directory_relative: PathBuf,
    completion_directory_path: PathBuf,
    anchor_directory_relative: PathBuf,
    anchor_directory_path: PathBuf,
    selector_relative: PathBuf,
    selector_path: PathBuf,
    completion_path: PathBuf,
    completion_bytes: Vec<u8>,
    capability: RetainedRestoreCompletion,
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
fn publish_test_restore_completion(root: &TempDir) -> TestRestoreCompletion {
    let authority_root =
        RestoreRetainedDirectory::open_root(&root.0).expect("retain completion authority root");
    let completion_directory_relative = PathBuf::from("restore-completions/app/archive");
    let completion_directory_path = root.0.join(&completion_directory_relative);
    let anchor_directory_relative = PathBuf::from("restore-completion-anchors/app/archive");
    let anchor_directory_path = root.0.join(&anchor_directory_relative);
    let selector_relative = PathBuf::from("restore-completion-selectors/app/archive.json");
    let selector_path = root.0.join(&selector_relative);

    let mut document = completion_authority_document();
    document.protected_authority_root =
        restore_root_path_binding(&authority_root).expect("bind protected authority root");
    let completion_bytes = canonical_restore_completion_authority_bytes(&document)
        .expect("canonical completion bytes");
    let completion_relative =
        completion_authority_relative_path(&completion_directory_relative, &completion_bytes)
            .expect("completion content address");
    let completion_path = root.0.join(&completion_relative);
    let completion_retained = publish_private_file_create_new_retained(
        &authority_root,
        &completion_relative,
        &completion_path,
        &completion_bytes,
    )
    .expect("publish completion record");
    let completion = RetainedRestoreDocument {
        retained: completion_retained,
        document: document.clone(),
    };
    completion
        .revalidate()
        .expect("revalidate completion before anchoring");

    authority_root
        .create_dir_all_synced(&anchor_directory_relative)
        .expect("create completion anchor directory");
    let store_root =
        retained_store_authority_root(&authority_root).expect("retain Store authority root");
    let completion_anchor = store_root
        .retain_file_lifetime_anchor(
            &anchor_directory_relative,
            &completion.retained.handle,
            &completion.retained.identity,
            &completion.retained.digest,
            u64::try_from(completion.retained.bytes.len()).expect("completion length"),
        )
        .expect("anchor exact completion record");
    let selector_document = RestoreCompletionSelectorDocument {
        schema_version: RESTORE_COMPLETION_SELECTOR_SCHEMA_VERSION.to_owned(),
        operation_nonce: document.operation_nonce.clone(),
        project_id: document.project_id.clone(),
        completion: RestoreCompletionRecordSelection {
            relative_path: slash_path(&completion_relative).expect("completion relative path"),
            content_sha256: sha256(&completion_bytes),
            byte_length: u64::try_from(completion_bytes.len()).expect("completion length"),
            leaf_anchor: completion_anchor.binding().clone(),
        },
        parent_root_anchor: RestoreCompletionParentRootAnchor {
            protected_authority_root: document.protected_authority_root.clone(),
            completion_parent_relative: slash_path(&completion_directory_relative)
                .expect("completion parent relative path"),
            completion_parent_path_sha256: restore_path_digest(&completion_directory_path)
                .expect("completion parent path digest"),
            selector_relative: slash_path(&selector_relative).expect("selector relative path"),
            selector_parent_relative: slash_path(
                selector_relative.parent().expect("selector parent"),
            )
            .expect("selector parent relative path"),
        },
        project: document.project_link.clone(),
        replay: document.replay_anchor.clone(),
        transaction: RestoreCompletionTransactionSelection {
            transaction_lock_relative: document.transaction.lock_relative.clone(),
            journal: document.journal.clone(),
            receipt: document.receipt.clone(),
        },
    };
    validate_restore_completion_selector(
        &selector_document,
        &document,
        &authority_root,
        &completion_directory_relative,
        &selector_relative,
    )
    .expect("validate completion selector");
    let selector_bytes = canonical_restore_completion_selector_bytes(&selector_document)
        .expect("canonical selector bytes");
    let selector_retained = publish_private_file_create_new_retained(
        &authority_root,
        &selector_relative,
        &selector_path,
        &selector_bytes,
    )
    .expect("publish completion selector");
    let capability = RetainedRestoreCompletion {
        selector: RetainedRestoreDocument {
            retained: selector_retained,
            document: selector_document,
        },
        completion,
        completion_anchor,
    };
    capability
        .revalidate()
        .expect("revalidate selected completion");

    TestRestoreCompletion {
        authority_root,
        completion_directory_relative,
        completion_directory_path,
        anchor_directory_relative,
        anchor_directory_path,
        selector_relative,
        selector_path,
        completion_path,
        completion_bytes,
        capability,
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
#[test]
fn hidden_restore_completion_selector_fails_closed_with_visible_residue() {
    let root = TempDir::new("hidden-completion-selector");
    let state = publish_test_restore_completion(&root);
    fs::remove_file(&state.selector_path).expect("hide completion selector");

    assert!(state.capability.revalidate().is_err());
    assert!(matches!(
        load_restore_completion_authority_retained(
            &state.authority_root,
            &state.completion_directory_relative,
            &state.completion_directory_path,
            &state.anchor_directory_relative,
            &state.anchor_directory_path,
            &state.selector_relative,
            &state.selector_path,
        ),
        Err(RestoreError::Interrupted { .. })
    ));
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
#[test]
fn retained_reader_rejects_byte_identical_selector_substitution() {
    let root = TempDir::new("completion-selector-substitution");
    let state = publish_test_restore_completion(&root);
    let selector_bytes = state.capability.selector.retained.bytes.clone();
    fs::remove_file(&state.selector_path).expect("remove exact completion selector");
    fs::write(&state.selector_path, selector_bytes)
        .expect("install byte-identical selector substitute");

    assert!(state.capability.revalidate().is_err());
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
#[test]
fn byte_identical_completion_recreation_cannot_reuse_content_address() {
    let root = TempDir::new("completion-byte-recreation");
    let state = publish_test_restore_completion(&root);
    fs::remove_file(&state.completion_path).expect("hide exact completion record");
    fs::write(&state.completion_path, &state.completion_bytes)
        .expect("recreate byte-identical completion record");

    assert!(state.capability.revalidate().is_err());
    assert!(load_restore_completion_authority_retained(
        &state.authority_root,
        &state.completion_directory_relative,
        &state.completion_directory_path,
        &state.anchor_directory_relative,
        &state.anchor_directory_path,
        &state.selector_relative,
        &state.selector_path,
    )
    .is_err());
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
#[test]
fn completion_parent_replacement_cannot_remint_selector_authority() {
    let root = TempDir::new("completion-parent-replacement");
    let state = publish_test_restore_completion(&root);
    let displaced = root.0.join("completion-parent-displaced");
    fs::rename(&state.completion_directory_path, &displaced)
        .expect("displace exact completion parent");
    fs::create_dir_all(&state.completion_directory_path).expect("recreate completion parent");
    fs::write(&state.completion_path, &state.completion_bytes)
        .expect("recreate completion under replacement parent");

    assert!(load_restore_completion_authority_retained(
        &state.authority_root,
        &state.completion_directory_relative,
        &state.completion_directory_path,
        &state.anchor_directory_relative,
        &state.anchor_directory_path,
        &state.selector_relative,
        &state.selector_path,
    )
    .is_err());
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
#[test]
fn selector_rollback_is_rejected_by_operation_project_replay_and_transaction_bindings() {
    let root = TempDir::new("completion-selector-rollback");
    let state = publish_test_restore_completion(&root);
    let mut rolled_back = state.capability.selector.document.clone();
    rolled_back.operation_nonce = "fedcba9876543210fedcba9876543210".to_owned();
    rolled_back.project_id = "other-project".to_owned();
    rolled_back.replay.anchor_digest = format!("sha256:{}", "9".repeat(64));
    rolled_back.transaction.transaction_lock_relative = "restore-locks/other.lock".to_owned();
    let bytes = canonical_restore_completion_selector_bytes(&rolled_back)
        .expect("canonical rolled-back selector");
    fs::remove_file(&state.selector_path).expect("remove current selector");
    fs::write(&state.selector_path, bytes).expect("install rolled-back selector");

    assert!(load_restore_completion_authority_retained(
        &state.authority_root,
        &state.completion_directory_relative,
        &state.completion_directory_path,
        &state.anchor_directory_relative,
        &state.anchor_directory_path,
        &state.selector_relative,
        &state.selector_path,
    )
    .is_err());
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
#[test]
fn retained_selector_rejects_whole_authority_root_substitution() {
    let root = TempDir::new("completion-root-substitution");
    let state = publish_test_restore_completion(&root);
    let displaced = root.0.with_extension("displaced");
    fs::rename(&root.0, &displaced).expect("displace completion authority root");
    fs::create_dir_all(&root.0).expect("create replacement authority root");

    assert!(state.capability.revalidate().is_err());

    fs::remove_dir_all(&root.0).expect("remove replacement authority root");
    fs::rename(displaced, &root.0).expect("restore test authority root");
}

#[test]
fn completion_success_has_no_decisive_io_after_selector_commit() {
    let source = include_str!("restore.rs");
    let start = source
        .find("let selector_retained = match publish_private_file_create_new_retained(")
        .expect("selector publication source");
    let end = source[start..]
        .find("\n}\n\nfn isolate_verified_sidecar_after_completion_error")
        .map(|offset| start + offset)
        .expect("completion publisher end");
    let after_commit = &source[start..end];
    let success = after_commit
        .find("// The fixed protected selector publication is the restore success")
        .expect("selector success linearization comment");
    let pure_tail = &after_commit[success..];
    for decisive in [".revalidate(", ".sync_", "open_", "read_"] {
        assert!(
            !pure_tail.contains(decisive),
            "decisive I/O {decisive:?} must not follow selector success"
        );
    }
}

#[test]
fn completion_contract_uses_exact_anchor_and_never_persisted_platform_identity() {
    let source = include_str!("restore.rs");
    assert!(source.contains("retain_file_lifetime_anchor("));
    assert!(source.contains("open_file_lifetime_anchor("));
    assert!(source.contains(
        "protected restore receipt exists without its atomically committed completion selector"
    ));
    assert!(!source.contains("restore_identity_digest"));
    assert!(!source.contains("canonical_digest()"));
}
