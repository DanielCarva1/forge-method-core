use forge_core_contracts::claim::ActorRole;
use forge_core_contracts::common::{RepoPath, StableId};
use forge_core_contracts::tool_effect::{
    AccessMode, ConflictCode, ConflictDetection, ConflictPolicy, EffectActor, EffectNotification,
    EffectRead, EffectRepair, EffectTargetKind, EffectWrite, InverseKind, InverseMetadata,
    InverseSource, RepairStrategy,
};
use forge_core_contracts::FieldEvidenceRegistry;
use forge_core_contracts::{
    ClaimContractDocument, CompletionContractDocument, ContractFamilyInventoryDocument,
    CoordinationEvalContractDocument, DecisionCloseContractDocument, GateContractDocument,
    HealthRecoveryContractDocument, OperationContractDocument, RequestContractDocument,
    RuntimeHandoffContractDocument, RuntimeRegistryEntryDocument, ToolEffectContract,
    ToolEffectContractDocument,
};
use forge_core_store::{
    acquire_effect_store_lock, append_effect_target_metadata_records, append_json_line,
    apply_file_effect_transaction, apply_file_effect_transaction_with_wal,
    apply_file_effect_transaction_with_wal_lock, build_effect_metadata_context,
    build_reference_index, collect_known_repo_paths, collect_validation_yaml_documents,
    compact_effect_wal, query_effect_target_metadata_index, rebuild_effect_target_metadata_index,
    recover_effect_wal, sha256_content_hash, try_acquire_effect_store_lock, AppendJsonLineError,
    EffectApplicationPayload, EffectApplicationReason, EffectApplicationStatus,
    EffectMetadataAdapterTrigger, EffectMetadataConsumerUse, EffectMetadataContextBuildOptions,
    EffectMetadataContextBuildReason, EffectMetadataContextBuildStatus,
    EffectMetadataForbiddenAuthority, EffectStoreLockError, EffectTargetMetadataIndexQuery,
    EffectTargetMetadataIndexQueryReason, EffectTargetMetadataIndexQueryStatus,
    EffectTargetMetadataIndexRebuildReason, EffectTargetMetadataIndexRebuildStatus,
    EffectTargetMetadataRecord, EffectTargetMetadataRecordKind, EffectWalCompactionReason,
    EffectWalCompactionStatus, EffectWalOriginal, EffectWalRecord, EffectWalRecoveryReason,
    EffectWalRecoveryStatus, EffectWalStage, ReferenceIndexBuilder, ReferenceIndexOptions,
};
use forge_core_validate::{
    validate_claim_cross_references, validate_completion_cross_references,
    validate_coordination_eval_cross_references, validate_decision_close_cross_references,
    validate_gate_cross_references, validate_health_recovery_cross_references,
    validate_inventory_references, validate_operation_cross_references,
    validate_request_cross_references, validate_runtime_handoff_cross_references,
    validate_runtime_registry_cross_references, validate_tool_effect_cross_references,
    validate_yaml_known_repo_references, validate_yaml_source_id_references, ReferenceIndex,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct TestAppendRecord {
    id: u64,
    status: String,
}

fn test_metadata_record(
    logical_ref: &str,
    effect_id: &str,
    operation_id: &str,
    target_kind: EffectTargetKind,
    byte_len: u64,
) -> EffectTargetMetadataRecord {
    EffectTargetMetadataRecord {
        schema_version: "0.1".to_string(),
        record_kind: EffectTargetMetadataRecordKind::EffectTarget,
        recorded_at: Some("2026-06-25T00:00:00Z".to_string()),
        operation_id: StableId(operation_id.to_string()),
        effect_id: StableId(effect_id.to_string()),
        logical_ref: logical_ref.to_string(),
        physical_ref: format!(".forge-method/artifacts/{logical_ref}.yaml"),
        target_kind,
        access_mode: AccessMode::Write,
        content_hash: Some(format!("hash-{byte_len}")),
        byte_len,
        actor_agent_id: StableId("codex-test".to_string()),
        actor_role: ActorRole::Runtime,
        destructive: false,
        redaction_hint: StableId("raw_content_not_indexed".to_string()),
    }
}

fn test_effect(
    writes: Vec<EffectWrite>,
    read_ref: &str,
    read_hash: String,
) -> ToolEffectContractDocument {
    ToolEffectContractDocument {
        schema_version: "0.1".to_string(),
        tool_effect_contract: ToolEffectContract {
            id: StableId("effect.test.file_application".to_string()),
            contract_ref: RepoPath("contracts/effects/tool-effect-contract-v0.yaml".to_string()),
            effect_kind: forge_core_contracts::tool_effect::EffectKind::ArtifactWrite,
            operation_ref: StableId("op.test.file_application".to_string()),
            actor: EffectActor {
                agent_id: StableId("codex-test".to_string()),
                role: ActorRole::Driver,
            },
            read_set: vec![EffectRead {
                target_kind: EffectTargetKind::FilePath,
                reference: read_ref.to_string(),
                expected_hash: Some(read_hash),
                expected_version: None,
                required_for_plan: true,
            }],
            write_set: writes,
            conflict_detection: ConflictDetection {
                check_against: StableId("filesystem".to_string()),
                granularity: StableId("path".to_string()),
                conflict_codes: vec![ConflictCode::ReadTargetChanged],
                policy: ConflictPolicy::Block,
            },
            notification: EffectNotification {
                required: false,
                recipients: vec![],
                request_contract_ref: None,
            },
            repair: EffectRepair {
                strategy: RepairStrategy::None,
                automatic_repair_allowed: false,
                inverse_operation_ref: None,
                stop_if_inverse_missing: false,
                inverse: InverseMetadata {
                    kind: InverseKind::None,
                    source: InverseSource::Unavailable,
                    reference: None,
                    input_mapping_refs: vec![],
                    validation_gate_refs: vec![],
                    review_required: false,
                },
            },
        },
    }
}

fn effect_write(
    reference: &str,
    access_mode: AccessMode,
    expected_hash: Option<String>,
) -> EffectWrite {
    effect_write_with_kind(
        EffectTargetKind::FilePath,
        reference,
        access_mode,
        expected_hash,
    )
}

fn effect_write_with_kind(
    target_kind: EffectTargetKind,
    reference: &str,
    access_mode: AccessMode,
    expected_hash: Option<String>,
) -> EffectWrite {
    EffectWrite {
        target_kind,
        reference: reference.to_string(),
        access_mode,
        expected_hash,
        expected_version: None,
        destructive: access_mode == AccessMode::Delete,
    }
}

fn payload(reference: &str, content: &[u8]) -> EffectApplicationPayload {
    EffectApplicationPayload {
        target_ref: reference.to_string(),
        content: content.to_vec(),
        content_hash: sha256_content_hash(content),
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

/// Copy a directory tree recursively into `target` (created if missing).
fn copy_dir_recursive(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("create target dir");
    for entry in fs::read_dir(source).expect("read source dir") {
        let entry = entry.expect("dir entry");
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path);
        } else {
            fs::copy(&source_path, &target_path).expect("copy file");
        }
    }
}

/// Build a temp validation root mirroring the Forge core repo's contract tree
/// plus the append-only ledger.
///
/// The core repo is now a normal consumer: its `.forge-method.yaml` Project
/// Link points at a sibling sidecar (`../forge-forge-method-core`) that holds
/// the runtime state, including `ledger.ndjson`. The completion contracts
/// reference `.forge-method/ledger.ndjson` as a repo-relative ref, so a
/// reference-index built from a single root must carry both the contract tree
/// (from the repo) and the ledger (from the sidecar) to resolve cleanly.
fn merged_validation_root(label: &str) -> PathBuf {
    let root = repo_root();
    let temp = temp_store_root(label);
    copy_dir_recursive(&root.join("contracts"), &temp.join("contracts"));
    copy_dir_recursive(
        &root
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0"),
        &temp
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0"),
    );
    let ledger_source = [
        root.join("../forge-forge-method-core/.forge-method/ledger.ndjson"),
        root.join(".forge-method").join("ledger.ndjson"),
    ]
    .into_iter()
    .find(|candidate| candidate.exists());
    let ledger_target = temp.join(".forge-method").join("ledger.ndjson");
    fs::create_dir_all(ledger_target.parent().expect("ledger parent"))
        .expect("create .forge-method dir");
    if let Some(ledger_source) = ledger_source {
        fs::copy(&ledger_source, &ledger_target).expect("copy ledger.ndjson");
    } else {
        fs::write(&ledger_target, []).expect("create empty validation ledger");
    }
    temp
}

fn read_yaml<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let text =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    yaml_serde::from_str(&text)
        .unwrap_or_else(|err| panic!("deserialize {}: {err}", path.display()))
}

fn temp_store_root(test_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "forge-core-store-{test_name}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn store_reference_index_satisfies_current_cross_file_validation() {
    let root = merged_validation_root("current-cross-file-validation");
    let index = build_reference_index(&root).expect("build reference index");

    let inventory: ContractFamilyInventoryDocument =
        read_yaml(&root.join("contracts/inventory/v0-contract-family-lock.yaml"));
    assert_report_ok(
        "inventory refs",
        validate_inventory_references(&inventory, &index),
    );

    for path in yaml_files(&root.join("docs/fixtures/operation-contract-v0")) {
        let operation: OperationContractDocument = read_yaml(&path);
        assert_report_ok(
            &format!("operation refs {}", path.display()),
            validate_operation_cross_references(&operation, &index),
        );
    }

    assert_cross_ref_instances::<ClaimContractDocument, _>(
        "contracts/claims",
        "claim-contract-v0.yaml",
        &index,
        validate_claim_cross_references,
    );
    assert_cross_ref_instances::<CompletionContractDocument, _>(
        "contracts/completion",
        "completion-contract-v0.yaml",
        &index,
        validate_completion_cross_references,
    );
    assert_cross_ref_instances::<GateContractDocument, _>(
        "contracts/gates",
        "gate-contract-v0.yaml",
        &index,
        validate_gate_cross_references,
    );
    assert_cross_ref_instances::<RequestContractDocument, _>(
        "contracts/requests",
        "request-contract-v0.yaml",
        &index,
        validate_request_cross_references,
    );
    assert_cross_ref_instances::<ToolEffectContractDocument, _>(
        "contracts/effects",
        "tool-effect-contract-v0.yaml",
        &index,
        validate_tool_effect_cross_references,
    );
    assert_cross_ref_instances::<DecisionCloseContractDocument, _>(
        "contracts/decisions",
        "decision-close-contract-v0.yaml",
        &index,
        validate_decision_close_cross_references,
    );
    assert_named_cross_ref::<RuntimeHandoffContractDocument, _>(
        "contracts/runtimes/cursor-browser-validation-runtime.yaml",
        &index,
        validate_runtime_handoff_cross_references,
    );
    assert_named_cross_ref::<RuntimeHandoffContractDocument, _>(
        "contracts/runtimes/cursor-browser-validation-missing-capability.yaml",
        &index,
        validate_runtime_handoff_cross_references,
    );
    assert_named_cross_ref::<RuntimeRegistryEntryDocument, _>(
        "contracts/runtimes/registry-cursor-browser-agent.yaml",
        &index,
        validate_runtime_registry_cross_references,
    );
    assert_cross_ref_instances::<HealthRecoveryContractDocument, _>(
        "contracts/recovery",
        "health-recovery-contract-v0.yaml",
        &index,
        validate_health_recovery_cross_references,
    );
    assert_cross_ref_instances::<CoordinationEvalContractDocument, _>(
        "contracts/evals",
        "coordination-eval-contract-v0.yaml",
        &index,
        validate_coordination_eval_cross_references,
    );
}

#[test]
fn standard_runtime_projection_refs_are_option_controlled() {
    let root = repo_root();
    let default_index = ReferenceIndexBuilder::new()
        .build(&root)
        .expect("default index");
    assert!(default_index.contains(".forge-method/agents/registry.yaml"));

    let strict_index = ReferenceIndexBuilder::with_options(ReferenceIndexOptions {
        include_standard_runtime_projections: false,
    })
    .build(&root)
    .expect("strict index");
    assert!(!strict_index.contains(".forge-method/agents/registry.yaml"));
}

#[test]
fn store_collects_validation_yaml_documents_for_source_id_checks() {
    let root = repo_root();
    let collection = collect_validation_yaml_documents(&root);
    assert!(
        collection.diagnostics.is_empty(),
        "parse diagnostics: {:?}",
        collection.diagnostics
    );
    assert!(collection.documents.len() >= 100);

    let evidence: FieldEvidenceRegistry = read_yaml(
        &root
            .join("contracts")
            .join("research")
            .join("field-evidence-20260625.yaml"),
    );
    let report = validate_yaml_source_id_references(&collection.documents, &evidence);
    assert_report_ok("source_id refs", report);
}

#[test]
fn store_known_paths_satisfy_current_generic_known_refs() {
    let root = repo_root();
    let collection = collect_validation_yaml_documents(&root);
    let known_paths = collect_known_repo_paths(&root);
    assert!(known_paths.contains("contracts"));
    assert!(known_paths.contains("docs/fixtures/operation-contract-v0"));

    let report = validate_yaml_known_repo_references(&collection.documents, &known_paths);
    assert_report_ok("known repo refs", report);
}

#[test]
fn append_json_line_creates_parent_and_appends_records() {
    let root = temp_store_root("append-json-line");
    let path = ".forge-method/evidence/commands/story-validation.ndjson";

    let first = TestAppendRecord {
        id: 1,
        status: "passed".to_string(),
    };
    let second = TestAppendRecord {
        id: 2,
        status: "failed".to_string(),
    };

    let target = append_json_line(&root, path, &first).expect("append first record");
    append_json_line(&root, path, &second).expect("append second record");

    let text = fs::read_to_string(&target).expect("read append target");
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    let parsed_first: TestAppendRecord = serde_json::from_str(lines[0]).expect("first json line");
    let parsed_second: TestAppendRecord = serde_json::from_str(lines[1]).expect("second json line");
    assert_eq!(parsed_first.id, 1);
    assert_eq!(parsed_first.status, "passed");
    assert_eq!(parsed_second.id, 2);
    assert_eq!(parsed_second.status, "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn append_json_line_rejects_path_escape() {
    let root = temp_store_root("append-json-line-path-escape");
    let record = TestAppendRecord {
        id: 1,
        status: "blocked".to_string(),
    };

    let error =
        append_json_line(&root, "../outside.ndjson", &record).expect_err("path escape should fail");

    assert!(matches!(
        error,
        AppendJsonLineError::InvalidRelativePath { .. }
    ));
    assert!(!root.exists());
}

#[test]
fn apply_file_effect_transaction_applies_create_write_and_append() {
    let root = temp_store_root("apply-file-effect-transaction");
    let read_ref = ".forge-method/input.txt";
    let replace_ref = ".forge-method/out/replace.txt";
    let append_ref = ".forge-method/evidence/log.ndjson";
    let create_ref = ".forge-method/artifacts/new.txt";
    fs::create_dir_all(root.join(".forge-method/out")).expect("create out dir");
    fs::create_dir_all(root.join(".forge-method/evidence")).expect("create evidence dir");
    fs::write(root.join(read_ref), b"input").expect("write read file");
    fs::write(root.join(replace_ref), b"old").expect("write replace file");
    fs::write(root.join(append_ref), b"before\n").expect("write append file");
    let effect = test_effect(
        vec![
            effect_write(create_ref, AccessMode::Create, None),
            effect_write(
                replace_ref,
                AccessMode::Write,
                Some(sha256_content_hash(b"old")),
            ),
            effect_write(append_ref, AccessMode::Append, None),
        ],
        read_ref,
        sha256_content_hash(b"input"),
    );
    let payloads = vec![
        payload(create_ref, b"created"),
        payload(replace_ref, b"new"),
        payload(append_ref, b"after\n"),
    ];

    let result = apply_file_effect_transaction(&root, &effect, &payloads);

    assert_eq!(result.status, EffectApplicationStatus::Applied);
    assert_eq!(result.reasons, vec![EffectApplicationReason::Applied]);
    assert_eq!(
        fs::read(root.join(create_ref)).expect("read created"),
        b"created"
    );
    assert_eq!(
        fs::read(root.join(replace_ref)).expect("read replaced"),
        b"new"
    );
    assert_eq!(
        fs::read(root.join(append_ref)).expect("read appended"),
        b"before\nafter\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_file_effect_transaction_projects_artifact_and_evidence_ids() {
    let root = temp_store_root("project-artifact-evidence");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("state.yaml"), b"state").expect("write state");
    let state_hash = sha256_content_hash(b"state");
    let effect = test_effect(
        vec![
            effect_write_with_kind(
                EffectTargetKind::ArtifactId,
                "story.current.result",
                AccessMode::Create,
                None,
            ),
            effect_write_with_kind(
                EffectTargetKind::EvidenceId,
                "story.validation",
                AccessMode::Append,
                None,
            ),
        ],
        "state.yaml",
        state_hash,
    );

    let result = apply_file_effect_transaction(
        &root,
        &effect,
        &[
            payload("story.current.result", b"artifact"),
            payload("story.validation", b"{\"ok\":true}\n"),
        ],
    );

    assert_eq!(result.status, EffectApplicationStatus::Applied);
    assert_eq!(
        fs::read(root.join(".forge-method/artifacts/story.current.result.yaml"))
            .expect("read artifact"),
        b"artifact"
    );
    assert_eq!(
        fs::read_to_string(root.join(".forge-method/evidence/story.validation.json"))
            .expect("read evidence"),
        "{\"ok\":true}\n"
    );
    assert_eq!(
        result.applied_refs,
        vec!["story.current.result", "story.validation"]
    );
    assert_eq!(result.metadata_records.len(), 2);
    assert_eq!(
        result.metadata_records[0].logical_ref,
        "story.current.result"
    );
    assert_eq!(
        result.metadata_records[0].physical_ref,
        ".forge-method/artifacts/story.current.result.yaml"
    );
    assert_eq!(
        result.metadata_records[0].target_kind,
        EffectTargetKind::ArtifactId
    );
    assert_eq!(
        result.metadata_records[0].content_hash,
        Some(sha256_content_hash(b"artifact"))
    );
    assert_eq!(result.metadata_records[0].byte_len, 8);
    assert_eq!(
        result.metadata_records[0].operation_id,
        StableId("op.test.file_application".to_string())
    );
    assert_eq!(
        result.metadata_records[0].actor_agent_id,
        StableId("codex-test".to_string())
    );
    assert_eq!(result.metadata_records[0].actor_role, ActorRole::Driver);
    assert_eq!(result.metadata_records[0].recorded_at, None);
    assert_eq!(result.metadata_records[1].logical_ref, "story.validation");
    assert_eq!(
        result.metadata_records[1].physical_ref,
        ".forge-method/evidence/story.validation.json"
    );
    assert_eq!(
        result.metadata_records[1].target_kind,
        EffectTargetKind::EvidenceId
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn append_effect_target_metadata_records_writes_append_only_index() {
    let root = temp_store_root("append-effect-target-metadata");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("state.yaml"), b"state").expect("write state");
    let state_hash = sha256_content_hash(b"state");
    let effect = test_effect(
        vec![effect_write_with_kind(
            EffectTargetKind::ArtifactId,
            "story.current.result",
            AccessMode::Create,
            None,
        )],
        "state.yaml",
        state_hash,
    );
    let mut result =
        apply_file_effect_transaction(&root, &effect, &[payload("story.current.result", b"body")]);
    result.metadata_records[0].recorded_at = Some("2026-06-25T00:00:00Z".to_string());

    let paths = append_effect_target_metadata_records(
        &root,
        ".forge-method/index/effect-targets.ndjson",
        &result.metadata_records,
    )
    .expect("append metadata records");

    assert_eq!(paths.len(), 1);
    let index_path = root.join(".forge-method/index/effect-targets.ndjson");
    let line = fs::read_to_string(&index_path).expect("read index");
    let records = line
        .lines()
        .map(|line| serde_json::from_str::<EffectTargetMetadataRecord>(line).expect("json record"))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].logical_ref, "story.current.result");
    assert_eq!(
        records[0].physical_ref,
        ".forge-method/artifacts/story.current.result.yaml"
    );
    assert_eq!(
        records[0].recorded_at,
        Some("2026-06-25T00:00:00Z".to_string())
    );
    assert_eq!(
        records[0].redaction_hint,
        StableId("raw_content_not_indexed".to_string())
    );
    assert_eq!(line.matches("body").count(), 0);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn query_effect_target_metadata_index_filters_and_returns_latest_view() {
    let root = temp_store_root("query-effect-target-metadata");
    let index_ref = ".forge-method/index/effect-targets.ndjson";
    let records = vec![
        test_metadata_record(
            "story.result",
            "effect.first",
            "op.first",
            EffectTargetKind::ArtifactId,
            1,
        ),
        test_metadata_record(
            "story.other",
            "effect.other",
            "op.other",
            EffectTargetKind::ArtifactId,
            2,
        ),
        test_metadata_record(
            "story.result",
            "effect.second",
            "op.second",
            EffectTargetKind::ArtifactId,
            3,
        ),
    ];
    append_effect_target_metadata_records(&root, index_ref, &records).expect("append index");

    let result = query_effect_target_metadata_index(
        &root,
        index_ref,
        &EffectTargetMetadataIndexQuery {
            logical_ref: Some("story.result".to_string()),
            latest_per_target: true,
            ..EffectTargetMetadataIndexQuery::default()
        },
    );

    assert_eq!(result.status, EffectTargetMetadataIndexQueryStatus::Queried);
    assert_eq!(
        result.reasons,
        vec![EffectTargetMetadataIndexQueryReason::QueryMatched]
    );
    assert_eq!(result.scanned_records, 3);
    assert_eq!(result.matched_records, 2);
    assert_eq!(result.returned_records, 1);
    assert_eq!(result.consumer_use, EffectMetadataConsumerUse::Discovery);
    assert!(!result.authority_boundary.is_workflow_authority);
    assert!(result
        .authority_boundary
        .allowed_uses
        .contains(&EffectMetadataConsumerUse::HandoffContext));
    assert!(result
        .authority_boundary
        .forbidden_authority
        .contains(&EffectMetadataForbiddenAuthority::PhaseTransition));
    assert_eq!(
        result.records[0].effect_id,
        StableId("effect.second".to_string())
    );
    assert_eq!(result.records[0].byte_len, 3);
    let index_text = fs::read_to_string(root.join(index_ref)).expect("read index");
    assert_eq!(index_text.lines().count(), 3);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn build_effect_metadata_context_groups_latest_records_without_authority() {
    let root = temp_store_root("metadata-context-builder");
    let index_ref = ".forge-method/index/effect-targets.ndjson";
    let records = vec![
        test_metadata_record(
            "story.result",
            "effect.first",
            "op.first",
            EffectTargetKind::ArtifactId,
            1,
        ),
        test_metadata_record(
            "story.result",
            "effect.second",
            "op.second",
            EffectTargetKind::ArtifactId,
            3,
        ),
        test_metadata_record(
            "story.evidence",
            "effect.evidence",
            "op.evidence",
            EffectTargetKind::EvidenceId,
            2,
        ),
    ];
    append_effect_target_metadata_records(&root, index_ref, &records).expect("append index");
    let query = query_effect_target_metadata_index(
        &root,
        index_ref,
        &EffectTargetMetadataIndexQuery {
            latest_per_target: false,
            consumer_use: EffectMetadataConsumerUse::HandoffContext,
            ..EffectTargetMetadataIndexQuery::default()
        },
    );

    let context = build_effect_metadata_context(
        &query,
        &EffectMetadataContextBuildOptions {
            max_groups: 1,
            adapter_trigger: EffectMetadataAdapterTrigger::HandoffPreparation,
            ..EffectMetadataContextBuildOptions::default()
        },
    );

    assert_eq!(context.status, EffectMetadataContextBuildStatus::Built);
    assert_eq!(
        context.source_consumer_use,
        EffectMetadataConsumerUse::HandoffContext
    );
    assert_eq!(context.total_groups, 2);
    assert_eq!(context.returned_groups, 1);
    assert_eq!(context.omitted_groups, 1);
    assert_eq!(
        context.reasons,
        vec![
            EffectMetadataContextBuildReason::ContextBuilt,
            EffectMetadataContextBuildReason::GroupsOmittedByLimit
        ]
    );
    assert!(!context.authority_boundary.is_workflow_authority);
    assert!(context.adapter_presentation.automatic_invocation_allowed);
    assert_eq!(
        context.adapter_presentation.trigger,
        EffectMetadataAdapterTrigger::HandoffPreparation
    );
    assert!(!context.adapter_presentation.may_create_workflow_authority);
    assert_eq!(context.groups[0].logical_ref, "story.result");
    assert_eq!(
        context.groups[0].latest_effect_id,
        StableId("effect.second".to_string())
    );
    assert_eq!(context.groups[0].record_count, 2);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn query_effect_target_metadata_index_fails_on_invalid_record() {
    let root = temp_store_root("query-effect-target-metadata-invalid");
    let index_ref = ".forge-method/index/effect-targets.ndjson";
    fs::create_dir_all(root.join(".forge-method/index")).expect("create index dir");
    fs::write(root.join(index_ref), b"{\"not\":\"metadata\"}\n").expect("write invalid index");

    let result = query_effect_target_metadata_index(
        &root,
        index_ref,
        &EffectTargetMetadataIndexQuery::default(),
    );

    assert_eq!(result.status, EffectTargetMetadataIndexQueryStatus::Failed);
    assert_eq!(
        result.reasons,
        vec![EffectTargetMetadataIndexQueryReason::IndexParseFailed]
    );
    assert!(result.diagnostics[0].contains("parse metadata index line 1 failed"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_file_effect_transaction_projects_request_stream_id() {
    let root = temp_store_root("project-request-stream");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("state.yaml"), b"state").expect("write state");
    let state_hash = sha256_content_hash(b"state");
    let effect = test_effect(
        vec![effect_write_with_kind(
            EffectTargetKind::RequestStream,
            "worker-requests",
            AccessMode::Append,
            None,
        )],
        "state.yaml",
        state_hash,
    );

    let result =
        apply_file_effect_transaction(&root, &effect, &[payload("worker-requests", b"{}\n")]);

    assert_eq!(result.status, EffectApplicationStatus::Applied);
    assert_eq!(
        fs::read_to_string(root.join(".forge-method/requests/worker-requests.ndjson"))
            .expect("read requests"),
        "{}\n"
    );
    assert_eq!(result.applied_refs, vec!["worker-requests"]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_file_effect_transaction_rejects_path_like_artifact_outside_projection() {
    let root = temp_store_root("reject-artifact-path");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("state.yaml"), b"state").expect("write state");
    let state_hash = sha256_content_hash(b"state");
    let effect = test_effect(
        vec![effect_write_with_kind(
            EffectTargetKind::ArtifactId,
            "src/not-an-artifact.yaml",
            AccessMode::Create,
            None,
        )],
        "state.yaml",
        state_hash,
    );

    let result = apply_file_effect_transaction(
        &root,
        &effect,
        &[payload("src/not-an-artifact.yaml", b"artifact")],
    );

    assert_eq!(result.status, EffectApplicationStatus::Blocked);
    assert!(result
        .reasons
        .contains(&EffectApplicationReason::InvalidTargetPath));
    assert!(!root.join("src/not-an-artifact.yaml").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_file_effect_transaction_blocks_read_access_mode_in_write_set() {
    let root = temp_store_root("apply-file-effect-read-write-mode");
    let read_ref = ".forge-method/input.txt";
    let target_ref = ".forge-method/artifacts/read-mode.txt";
    fs::create_dir_all(root.join(".forge-method")).expect("create root");
    fs::write(root.join(read_ref), b"input").expect("write read file");
    let effect = test_effect(
        vec![effect_write(target_ref, AccessMode::Read, None)],
        read_ref,
        sha256_content_hash(b"input"),
    );

    let result = apply_file_effect_transaction(&root, &effect, &[]);

    assert_eq!(result.status, EffectApplicationStatus::Blocked);
    assert!(result
        .reasons
        .contains(&EffectApplicationReason::UnsupportedAccessMode));
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("unsupported write access mode")));
    assert!(!root.join(target_ref).exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_file_effect_transaction_with_wal_blocks_missing_payload_before_wal_begin() {
    let root = temp_store_root("apply-file-effect-missing-payload-wal");
    let read_ref = ".forge-method/input.txt";
    let target_ref = ".forge-method/artifacts/missing-payload.txt";
    let wal_ref = ".forge-method/ledger/effects-wal.ndjson";
    fs::create_dir_all(root.join(".forge-method")).expect("create root");
    fs::write(root.join(read_ref), b"input").expect("write read file");
    let effect = test_effect(
        vec![effect_write(target_ref, AccessMode::Create, None)],
        read_ref,
        sha256_content_hash(b"input"),
    );

    let result =
        apply_file_effect_transaction_with_wal(&root, &effect, &[], wal_ref, "tx-missing-payload");

    assert_eq!(result.status, EffectApplicationStatus::Blocked);
    assert!(result
        .reasons
        .contains(&EffectApplicationReason::MissingPayloadForWrite));
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("missing payload")));
    assert!(!root.join(target_ref).exists());
    assert!(!root.join(wal_ref).exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_file_effect_transaction_blocks_stale_read_without_writing() {
    let root = temp_store_root("apply-file-effect-stale-read");
    let read_ref = ".forge-method/input.txt";
    let create_ref = ".forge-method/artifacts/new.txt";
    fs::create_dir_all(root.join(".forge-method")).expect("create root");
    fs::write(root.join(read_ref), b"changed").expect("write read file");
    let effect = test_effect(
        vec![effect_write(create_ref, AccessMode::Create, None)],
        read_ref,
        sha256_content_hash(b"original"),
    );

    let result = apply_file_effect_transaction(&root, &effect, &[payload(create_ref, b"created")]);

    assert_eq!(result.status, EffectApplicationStatus::Blocked);
    assert!(result
        .reasons
        .contains(&EffectApplicationReason::ExpectedHashMismatch));
    assert!(!root.join(create_ref).exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_file_effect_transaction_rolls_back_when_later_write_fails() {
    let root = temp_store_root("apply-file-effect-rollback");
    let read_ref = ".forge-method/input.txt";
    let first_ref = ".forge-method/out/first.txt";
    let blocked_ref = ".forge-method/blocker/child.txt";
    fs::create_dir_all(root.join(".forge-method")).expect("create root");
    fs::create_dir_all(root.join(".forge-method/out")).expect("create out dir");
    fs::write(root.join(read_ref), b"input").expect("write read file");
    fs::write(root.join(".forge-method/blocker"), b"not a directory").expect("write blocker");
    let effect = test_effect(
        vec![
            effect_write(first_ref, AccessMode::Create, None),
            effect_write(blocked_ref, AccessMode::Create, None),
        ],
        read_ref,
        sha256_content_hash(b"input"),
    );
    let payloads = vec![
        payload(first_ref, b"first"),
        payload(blocked_ref, b"blocked"),
    ];

    let result = apply_file_effect_transaction(&root, &effect, &payloads);

    assert_eq!(result.status, EffectApplicationStatus::RolledBack);
    assert!(result
        .reasons
        .contains(&EffectApplicationReason::ApplyFailed));
    assert!(!root.join(first_ref).exists());
    assert_eq!(
        fs::read(root.join(".forge-method/blocker")).expect("read blocker"),
        b"not a directory"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_file_effect_transaction_with_wal_records_commit() {
    let root = temp_store_root("apply-file-effect-wal-commit");
    let read_ref = ".forge-method/input.txt";
    let create_ref = "wal.artifact";
    let physical_create_ref = ".forge-method/artifacts/wal.artifact.yaml";
    let wal_ref = ".forge-method/ledger/effects-wal.ndjson";
    fs::create_dir_all(root.join(".forge-method")).expect("create root");
    fs::write(root.join(read_ref), b"input").expect("write read file");
    let effect = test_effect(
        vec![effect_write_with_kind(
            EffectTargetKind::ArtifactId,
            create_ref,
            AccessMode::Create,
            None,
        )],
        read_ref,
        sha256_content_hash(b"input"),
    );

    let result = apply_file_effect_transaction_with_wal(
        &root,
        &effect,
        &[payload(create_ref, b"created")],
        wal_ref,
        "tx-test-commit",
    );

    assert_eq!(result.status, EffectApplicationStatus::Applied);
    assert_eq!(
        fs::read(root.join(physical_create_ref)).expect("read created"),
        b"created"
    );
    let wal_text = fs::read_to_string(root.join(wal_ref)).expect("read wal");
    assert!(wal_text.contains("\"target_ref\":\"wal.artifact\""));
    assert!(
        wal_text.contains("\"physical_target_ref\":\".forge-method/artifacts/wal.artifact.yaml\"")
    );
    let stages: Vec<_> = wal_text
        .lines()
        .map(|line| {
            serde_json::from_str::<EffectWalRecord>(line)
                .expect("parse wal line")
                .stage
        })
        .collect();
    assert_eq!(
        stages,
        vec![
            EffectWalStage::Begin,
            EffectWalStage::BeforeImage,
            EffectWalStage::WriteApplied,
            EffectWalStage::Commit
        ]
    );
    let wal_records = wal_text
        .lines()
        .map(|line| serde_json::from_str::<EffectWalRecord>(line).expect("parse wal line"))
        .collect::<Vec<_>>();
    let write_applied = wal_records
        .iter()
        .find(|record| record.stage == EffectWalStage::WriteApplied)
        .expect("write applied record");
    let metadata = write_applied
        .target_metadata
        .as_ref()
        .expect("wal target metadata");
    assert_eq!(
        metadata.operation_id,
        effect.tool_effect_contract.operation_ref
    );
    assert_eq!(metadata.target_kind, EffectTargetKind::ArtifactId);
    assert_eq!(metadata.access_mode, AccessMode::Create);
    assert_eq!(metadata.content_hash, Some(sha256_content_hash(b"created")));
    assert_eq!(metadata.byte_len, 7);
    assert_eq!(metadata.actor_role, ActorRole::Driver);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebuild_effect_target_metadata_index_from_committed_wal() {
    let root = temp_store_root("rebuild-effect-metadata-index");
    let read_ref = ".forge-method/input.txt";
    let create_ref = "wal.artifact";
    let wal_ref = ".forge-method/ledger/effects-wal.ndjson";
    let index_ref = ".forge-method/index/rebuilt-effect-targets.ndjson";
    fs::create_dir_all(root.join(".forge-method")).expect("create root");
    fs::write(root.join(read_ref), b"input").expect("write read file");
    let effect = test_effect(
        vec![effect_write_with_kind(
            EffectTargetKind::ArtifactId,
            create_ref,
            AccessMode::Create,
            None,
        )],
        read_ref,
        sha256_content_hash(b"input"),
    );

    let application = apply_file_effect_transaction_with_wal(
        &root,
        &effect,
        &[payload(create_ref, b"created")],
        wal_ref,
        "tx-test-commit",
    );
    assert_eq!(application.status, EffectApplicationStatus::Applied);
    let _ = fs::remove_file(root.join(index_ref));

    let rebuild = rebuild_effect_target_metadata_index(
        &root,
        wal_ref,
        index_ref,
        Some("2026-06-25T00:00:00Z"),
    );

    assert_eq!(
        rebuild.status,
        EffectTargetMetadataIndexRebuildStatus::Rebuilt
    );
    assert_eq!(
        rebuild.reasons,
        vec![EffectTargetMetadataIndexRebuildReason::MetadataRebuilt]
    );
    assert_eq!(rebuild.rebuilt_records, 1);
    assert_eq!(rebuild.appended_records, 1);
    assert_eq!(rebuild.records[0].logical_ref, create_ref);
    assert_eq!(
        rebuild.records[0].physical_ref,
        ".forge-method/artifacts/wal.artifact.yaml"
    );
    assert_eq!(
        rebuild.records[0].recorded_at,
        Some("2026-06-25T00:00:00Z".to_string())
    );
    let index_text = fs::read_to_string(root.join(index_ref)).expect("read rebuilt index");
    assert!(index_text.contains("\"logical_ref\":\"wal.artifact\""));
    assert!(index_text.contains("\"physical_ref\":\".forge-method/artifacts/wal.artifact.yaml\""));
    assert!(!index_text.contains("created"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebuild_effect_target_metadata_index_noops_for_legacy_wal_without_metadata() {
    let root = temp_store_root("rebuild-effect-metadata-legacy-wal");
    let wal_ref = ".forge-method/ledger/effects-wal.ndjson";
    let index_ref = ".forge-method/index/rebuilt-effect-targets.ndjson";
    append_json_line(
        &root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-legacy".to_string(),
            stage: EffectWalStage::Begin,
            effect_id: StableId("effect.legacy".to_string()),
            target_ref: None,
            physical_target_ref: None,
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append legacy begin");
    append_json_line(
        &root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-legacy".to_string(),
            stage: EffectWalStage::WriteApplied,
            effect_id: StableId("effect.legacy".to_string()),
            target_ref: Some(".forge-method/artifacts/legacy.yaml".to_string()),
            physical_target_ref: Some(".forge-method/artifacts/legacy.yaml".to_string()),
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append legacy write applied");
    append_json_line(
        &root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-legacy".to_string(),
            stage: EffectWalStage::Commit,
            effect_id: StableId("effect.legacy".to_string()),
            target_ref: None,
            physical_target_ref: None,
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append legacy commit");

    let rebuild = rebuild_effect_target_metadata_index(
        &root,
        wal_ref,
        index_ref,
        Some("2026-06-25T00:00:00Z"),
    );

    assert_eq!(rebuild.status, EffectTargetMetadataIndexRebuildStatus::Noop);
    assert_eq!(
        rebuild.reasons,
        vec![EffectTargetMetadataIndexRebuildReason::NoCommittedMetadataRecords]
    );
    assert_eq!(rebuild.rebuilt_records, 0);
    assert!(!root.join(index_ref).exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn recover_effect_wal_restores_incomplete_transaction() {
    let root = temp_store_root("recover-effect-wal");
    let wal_ref = ".forge-method/ledger/effects-wal.ndjson";
    let target_ref = ".forge-method/out/replace.txt";
    fs::create_dir_all(root.join(".forge-method/out")).expect("create out");
    fs::write(root.join(target_ref), b"new").expect("write new content");
    append_json_line(
        &root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-incomplete".to_string(),
            stage: EffectWalStage::Begin,
            effect_id: StableId("effect.test.file_application".to_string()),
            target_ref: None,
            physical_target_ref: None,
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append begin");
    append_json_line(
        &root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-incomplete".to_string(),
            stage: EffectWalStage::BeforeImage,
            effect_id: StableId("effect.test.file_application".to_string()),
            target_ref: Some(target_ref.to_string()),
            physical_target_ref: Some(target_ref.to_string()),
            target_metadata: None,
            original: Some(EffectWalOriginal {
                existed: true,
                content: b"old".to_vec(),
                content_hash: sha256_content_hash(b"old"),
            }),
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append before image");
    append_json_line(
        &root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-incomplete".to_string(),
            stage: EffectWalStage::WriteApplied,
            effect_id: StableId("effect.test.file_application".to_string()),
            target_ref: Some(target_ref.to_string()),
            physical_target_ref: Some(target_ref.to_string()),
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append write applied");

    let recovery = recover_effect_wal(&root, wal_ref);

    assert_eq!(recovery.status, EffectWalRecoveryStatus::Recovered);
    assert_eq!(
        recovery.reasons,
        vec![EffectWalRecoveryReason::IncompleteTransactionRecovered]
    );
    assert_eq!(recovery.recovered_transactions, vec!["tx-incomplete"]);
    assert_eq!(
        fs::read(root.join(target_ref)).expect("read restored"),
        b"old"
    );
    let wal_text = fs::read_to_string(root.join(wal_ref)).expect("read wal");
    assert!(wal_text.contains("recovered_rollback"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn effect_store_lock_blocks_second_try_lock_until_released() {
    let root = temp_store_root("effect-store-lock");
    let lock_ref = ".forge-method/locks/effects.lock";

    let first = acquire_effect_store_lock(&root, lock_ref).expect("acquire first lock");
    let second = try_acquire_effect_store_lock(&root, lock_ref).expect_err("second lock blocks");
    assert!(matches!(second, EffectStoreLockError::WouldBlock { .. }));
    drop(first);

    let _third = try_acquire_effect_store_lock(&root, lock_ref).expect("lock released");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_file_effect_transaction_with_wal_lock_records_commit() {
    let root = temp_store_root("apply-file-effect-wal-lock");
    let read_ref = ".forge-method/input.txt";
    let create_ref = ".forge-method/artifacts/new.txt";
    let wal_ref = ".forge-method/ledger/effects-wal.ndjson";
    let lock_ref = ".forge-method/locks/effects.lock";
    fs::create_dir_all(root.join(".forge-method")).expect("create root");
    fs::write(root.join(read_ref), b"input").expect("write read file");
    let effect = test_effect(
        vec![effect_write(create_ref, AccessMode::Create, None)],
        read_ref,
        sha256_content_hash(b"input"),
    );

    let result = apply_file_effect_transaction_with_wal_lock(
        &root,
        &effect,
        &[payload(create_ref, b"created")],
        wal_ref,
        lock_ref,
        "tx-test-locked-commit",
    );

    assert_eq!(result.status, EffectApplicationStatus::Applied);
    assert_eq!(
        fs::read(root.join(create_ref)).expect("read created"),
        b"created"
    );
    assert!(root.join(lock_ref).exists());
    let wal_text = fs::read_to_string(root.join(wal_ref)).expect("read wal");
    assert!(wal_text.contains("\"commit\""));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn compact_effect_wal_drops_closed_records_and_keeps_incomplete() {
    let root = temp_store_root("compact-effect-wal");
    let wal_ref = ".forge-method/ledger/effects-wal.ndjson";
    let closed_effect = StableId("effect.closed".to_string());
    let incomplete_effect = StableId("effect.incomplete".to_string());

    append_json_line(
        &root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-closed".to_string(),
            stage: EffectWalStage::Begin,
            effect_id: closed_effect.clone(),
            target_ref: None,
            physical_target_ref: None,
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append closed begin");
    append_json_line(
        &root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-closed".to_string(),
            stage: EffectWalStage::Commit,
            effect_id: closed_effect,
            target_ref: None,
            physical_target_ref: None,
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append closed commit");
    append_json_line(
        &root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-incomplete".to_string(),
            stage: EffectWalStage::Begin,
            effect_id: incomplete_effect.clone(),
            target_ref: None,
            physical_target_ref: None,
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append incomplete begin");
    append_json_line(
        &root,
        wal_ref,
        &EffectWalRecord {
            schema_version: "0.1".to_string(),
            tx_id: "tx-incomplete".to_string(),
            stage: EffectWalStage::BeforeImage,
            effect_id: incomplete_effect,
            target_ref: Some(".forge-method/out/file.txt".to_string()),
            physical_target_ref: Some(".forge-method/out/file.txt".to_string()),
            target_metadata: None,
            original: Some(EffectWalOriginal {
                existed: false,
                content: Vec::new(),
                content_hash: sha256_content_hash(b""),
            }),
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        },
    )
    .expect("append incomplete before image");

    let result = compact_effect_wal(&root, wal_ref);

    assert_eq!(result.status, EffectWalCompactionStatus::Compacted);
    assert_eq!(
        result.reasons,
        vec![EffectWalCompactionReason::ClosedRecordsDropped]
    );
    assert_eq!(result.retained_records, 2);
    assert_eq!(result.dropped_records, 2);
    assert_eq!(result.incomplete_transactions, vec!["tx-incomplete"]);
    let wal_text = fs::read_to_string(root.join(wal_ref)).expect("read compacted wal");
    assert!(!wal_text.contains("tx-closed"));
    assert!(wal_text.contains("tx-incomplete"));
    let _ = fs::remove_dir_all(root);
}

fn assert_cross_ref_instances<T, F>(
    relative_dir: &str,
    definition_file: &str,
    index: &ReferenceIndex,
    validate: F,
) where
    T: serde::de::DeserializeOwned,
    F: Fn(&T, &ReferenceIndex) -> forge_core_validate::ValidationReport,
{
    let dir = repo_root().join(relative_dir);
    for path in yaml_files(&dir) {
        if path.file_name().and_then(|value| value.to_str()) == Some(definition_file) {
            continue;
        }
        let contract: T = read_yaml(&path);
        assert_report_ok(
            &format!("cross refs {}", path.display()),
            validate(&contract, index),
        );
    }
}

fn assert_named_cross_ref<T, F>(relative_path: &str, index: &ReferenceIndex, validate: F)
where
    T: serde::de::DeserializeOwned,
    F: Fn(&T, &ReferenceIndex) -> forge_core_validate::ValidationReport,
{
    let path = repo_root().join(relative_path);
    let contract: T = read_yaml(&path);
    assert_report_ok(
        &format!("cross refs {}", path.display()),
        validate(&contract, index),
    );
}

#[allow(clippy::needless_pass_by_value)]
fn assert_report_ok(label: &str, report: forge_core_validate::ValidationReport) {
    assert!(!report.has_errors(), "{label}: {:?}", report.diagnostics());
}

fn yaml_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("read dir {}: {err}", dir.display()))
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("yaml"))
        .collect();
    files.sort();
    files
}
