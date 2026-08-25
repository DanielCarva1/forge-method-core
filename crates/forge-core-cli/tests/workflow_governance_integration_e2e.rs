#![allow(clippy::too_many_lines)] // End-to-end journeys keep their ordered user-visible assertions together.

//! Real-consumer P5c proof: trusted initialization, automatic selection,
//! replacement-agent resume, read-only shadow, and authority bypass rejection.

#[path = "support/workflow_broker.rs"]
mod workflow_broker_test_support;

use assert_cmd::Command;
use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    workflow_broker_event_signing_bytes, workflow_broker_host_event_descriptor_digest,
    WorkflowBrokerEventEnvelope, WorkflowBrokerIssuerProfile, WorkflowBrokerSemanticInput,
    WORKFLOW_BROKER_EVENT_SCHEMA_VERSION,
};
use forge_core_contracts::{
    workflow_broker_expected_audience, DecisionNeedRaisedEvent, GovernedPromotionReceipt,
    PhaseAdvancedEvent, PolicyCompletedEvent, PrincipalId, ProjectImportedEvent, ReadinessTarget,
    RuntimeKind, StableId, WorkflowBrokerBoundOperation, WorkflowBrokerCredentialProfile,
    WorkflowBrokerCredentialPurpose, WorkflowBrokerCredentialStatus, WorkflowBrokerCustodyKind,
    WorkflowBrokerHostBinding, WorkflowBrokerHostInteractionKind,
    WorkflowBrokerNativeHostProvenance, WorkflowBrokerPublicCredentialMetadata,
    WorkflowBrokerPublicKeyAlgorithm, WorkflowBrokerPublicRegistryDocument,
    WorkflowEvidenceOutcome, WorkflowEvidenceSubjectKind, WorkflowGovernanceEvent,
    WorkflowGovernanceReceiptDocument, MAX_CURRENT_WORK_DETAIL_BYTES,
    MAX_CURRENT_WORK_PREPARATION_BYTES, MAX_CURRENT_WORK_SUMMARY_BYTES,
    MAX_CURRENT_WORK_SUMMARY_REFERENCE_ITEMS, MAX_WORK_FOCUS_ACCEPT_INPUT_BYTES,
    MAX_WORK_FOCUS_UPDATE_INPUT_BYTES, WORKFLOW_BROKER_PUBLIC_REGISTRY_SCHEMA_VERSION,
    WORKFLOW_BROKER_REQUIRED_EVENT_SCHEMA_VERSION,
};
use forge_core_workflow_governance_tcb::{
    lock_workflow_governance_ledger_tcb, WorkflowGovernanceLedgerIdentity,
};
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const PROJECT_ID: &str = "app";
const WORKFLOW_ID: &str = "workflow.governance";
const HUMAN_BROKER_ISSUER: &str = "broker.workflow.cli-e2e-human";
const HUMAN_BROKER_PRINCIPAL: &str = "principal.workflow.cli-e2e-human";

fn bin() -> Command {
    if let Some(path) = std::env::var_os("FORGE_WORKFLOW_E2E_BINARY") {
        return Command::new(path);
    }
    Command::cargo_bin("forge-core").expect("forge-core binary")
}

struct Consumer {
    parent: PathBuf,
    app: PathBuf,
    state: PathBuf,
}

impl Consumer {
    fn new() -> Self {
        Self::new_with_prefix("forge-workflow-p5c-e2e")
    }

    fn new_with_prefix(prefix: &str) -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let sequence = SEQ.fetch_add(1, Ordering::SeqCst);
        let parent =
            std::env::temp_dir().join(format!("{prefix}-{}-{sequence}", std::process::id()));
        let _ = fs::remove_dir_all(&parent);
        let app = parent.join("app");
        let sidecar = parent.join("forge-app");
        let state = sidecar.join(".forge-method");
        fs::create_dir_all(&app).expect("consumer app");
        fs::create_dir_all(&state).expect("consumer state");
        fs::write(app.join("README.md"), "consumer project\n").expect("consumer artifact");
        fs::write(
            app.join(".forge-method.yaml"),
            "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
        )
        .expect("project link");
        // Compatibility state is intentionally hostile/stale. P5c must derive
        // the initial phase from its receipt ledger, not this tolerant file.
        fs::write(
            state.join("state.yaml"),
            "current_phase: 4-build-verify\nskip_governance: true\n",
        )
        .expect("compat state");
        Self { parent, app, state }
    }

    fn new_start_ready_with_prefix(prefix: &str) -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let sequence = SEQ.fetch_add(1, Ordering::SeqCst);
        let parent = std::env::temp_dir().join(format!(
            "{prefix}-start-ready-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&parent);
        let app = parent.join("app");
        fs::create_dir_all(&app).expect("start-ready consumer app");
        fs::write(app.join("README.md"), "consumer project\n")
            .expect("start-ready consumer artifact");

        let output = bin()
            .args(["start", "--root"])
            .arg(&app)
            .arg("--json")
            .output()
            .expect("bootstrap complete consumer through start");
        let envelope = assert_ok(&output);
        assert_eq!(envelope["command"], "start");
        assert_eq!(envelope["data"]["actions_performed"][0], "initialized");
        let state = PathBuf::from(
            envelope["data"]["project"]["state_root"]
                .as_str()
                .expect("bootstrapped state root"),
        );
        assert!(state.join("state.yaml").is_file());
        Self { parent, app, state }
    }

    fn run(&self, tail: &[&str]) -> Output {
        let mut args = vec![
            "workflow".to_owned(),
            tail[0].to_owned(),
            "--root".to_owned(),
            self.app.display().to_string(),
        ];
        args.extend(tail[1..].iter().map(|value| (*value).to_owned()));
        bin().args(args).output().expect("run workflow command")
    }

    fn apply_episode(&self, input: &Path) -> Output {
        bin()
            .args(["workflow", "episode", "apply", "--root"])
            .arg(&self.app)
            .arg("--input-file")
            .arg(input)
            .arg("--json")
            .output()
            .expect("apply workflow episode")
    }

    fn prepare_episode(&self) -> Output {
        bin()
            .args(["workflow", "episode", "prepare", "--root"])
            .arg(&self.app)
            .arg("--json")
            .output()
            .expect("prepare workflow episode")
    }

    fn finalize_episode(input: &Path) -> Output {
        bin()
            .args(["workflow", "episode", "finalize", "--input-file"])
            .arg(input)
            .arg("--json")
            .output()
            .expect("finalize workflow episode")
    }

    fn write_json<T: Serialize>(&self, name: &str, value: &T) -> PathBuf {
        let path = self.parent.join(name);
        fs::write(
            &path,
            serde_json::to_vec_pretty(value).expect("serialize CLI fixture"),
        )
        .expect("write CLI fixture");
        path
    }
}

impl Drop for Consumer {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.parent);
    }
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON: {error}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_ok(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "command failed status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope = json(output);
    assert_eq!(envelope["ok"], true);
    envelope
}

fn assert_report_preserves_next(resumed: &Value, next: &Value, expected_readiness_profile: &str) {
    let mut report_base = resumed["data"].clone();
    let replacement = report_base
        .as_object_mut()
        .expect("report data object")
        .remove("replacement_continuity")
        .expect("report replacement continuity");
    let report_current_work = report_base
        .as_object_mut()
        .expect("report data object")
        .remove("current_work")
        .expect("report Current Work context");
    let mut next_base = next["data"].clone();
    let next_current_work = next_base
        .as_object_mut()
        .expect("next data object")
        .remove("current_work");
    assert_eq!(
        report_base, next_base,
        "report must preserve ordinary guidance and add recovery context separately"
    );
    assert!(next_current_work.is_none());
    assert_eq!(report_current_work["status"], "absent");
    assert_eq!(replacement["status"], "ready");
    assert_eq!(
        replacement["binding"]["readiness_profile"],
        expected_readiness_profile
    );
}

fn run_profile(consumer: &Consumer, action: &str, tail: &[String]) -> Output {
    let mut args = vec![
        "workflow".to_owned(),
        "profile".to_owned(),
        action.to_owned(),
        "--root".to_owned(),
        consumer.app.display().to_string(),
    ];
    args.extend(tail.iter().cloned());
    bin()
        .args(args)
        .output()
        .expect("run workflow profile command")
}

fn replace_with_legacy_profileless_genesis(consumer: &Consumer) {
    assert_ok(&consumer.run(&["init", "--readiness-profile", "strict_external"]));
    let wal = consumer.state.join("wal/workflow-governance.ndjson");
    let first = fs::read_to_string(&wal)
        .expect("initialized WAL")
        .lines()
        .next()
        .expect("genesis line")
        .to_owned();
    let document: WorkflowGovernanceReceiptDocument =
        serde_json::from_str(&first).expect("typed genesis");
    let record = document.workflow_governance_receipt;
    let WorkflowGovernanceEvent::ProjectImported(imported) = record.event else {
        panic!("first event must be project import")
    };
    let identity = WorkflowGovernanceLedgerIdentity {
        project_id: record.project_id,
        bundle_id: record.bundle_id,
        bundle_digest: record.bundle_digest,
    };
    fs::remove_file(&wal).expect("remove test-only explicit genesis");
    let mut ledger = lock_workflow_governance_ledger_tcb(&consumer.state).expect("lock legacy WAL");
    ledger
        .initialize_unchecked_tcb(
            &identity,
            0,
            WorkflowGovernanceEvent::ProjectImported(ProjectImportedEvent {
                readiness_profile: None,
                ..imported
            }),
        )
        .expect("write canonical profile-less genesis");
}

fn upgrade_to_latest(consumer: &Consumer) {
    for _ in 0..8 {
        let status = assert_ok(&consumer.run(&["release-status"]));
        if status["data"]["available_successor"].is_null() {
            assert_eq!(
                status["data"]["active"]["release"]["release_id"],
                "workflow-governance.release.universal-assurance-v0"
            );
            return;
        }
        let argv = status["data"]["upgrade_argv"]
            .as_array()
            .expect("release upgrade argv");
        assert_ok(&execute_structured_argv(argv));
    }
    panic!("release chain did not converge");
}

fn append_test_policy_completion(consumer: &Consumer, next: &Value) {
    let mut ledger =
        lock_workflow_governance_ledger_tcb(&consumer.state).expect("lock fixture ledger");
    let projection = ledger.recover().expect("recover fixture ledger");
    let head = projection.head_digest.clone().expect("fixture head");
    let identity = projection
        .active_identity()
        .expect("fixture identity")
        .clone();
    let policy_ref = StableId(
        next["data"]["selected_policy_ref"]
            .as_str()
            .expect("selected policy")
            .to_owned(),
    );
    let target: ReadinessTarget =
        serde_json::from_value(next["data"]["target"].clone()).expect("typed readiness target");
    let snapshot = next["data"]["snapshot_digest"]
        .as_str()
        .expect("snapshot")
        .to_owned();
    ledger
        .append_unchecked_tcb_event(
            &head,
            &identity,
            projection.next_state_version,
            WorkflowGovernanceEvent::PolicyCompleted(PolicyCompletedEvent {
                policy_ref,
                target,
                phase: StableId(
                    next["data"]["current_phase"]
                        .as_str()
                        .expect("phase")
                        .to_owned(),
                ),
                snapshot_digest: snapshot.clone(),
                ledger_head_digest: head.clone(),
                subject: forge_core_contracts::WorkflowEvidenceSubject {
                    kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
                    subject_ref: "project.current_snapshot".to_owned(),
                    subject_digest: snapshot,
                },
                dependency_receipt_digests: Vec::new(),
                evidence_receipt_digests: Vec::new(),
                grounding_anchor_digests: Vec::new(),
                unresolved_deferred_obligation_refs: Vec::new(),
                unresolved_deferred_capability_refs: Vec::new(),
                completed_at_unix: 1,
            }),
        )
        .expect("append fixture-only policy completion");
}

fn advance_fixture_to_policy(consumer: &Consumer, target_policy: &str) -> Value {
    for _ in 0..80 {
        let next = assert_ok(&consumer.run(&["next"]));
        if next["data"]["selected_policy_ref"] == target_policy {
            return next;
        }
        append_test_policy_completion(consumer, &next);
    }
    panic!("fixture did not reach {target_policy}");
}

fn execute_structured_argv(argv: &[Value]) -> Output {
    let tokens = argv
        .iter()
        .map(|value| value.as_str().expect("argv string").to_owned())
        .collect::<Vec<_>>();
    assert_eq!(tokens.first().map(String::as_str), Some("forge-core"));
    bin()
        .args(tokens.iter().skip(1))
        .output()
        .expect("execute exact argv")
}

fn execute_cooperative_packet(packet: &Value, input_path: &Path) -> Output {
    let input_token = packet["input_file_token"]
        .as_str()
        .expect("cooperative input token");
    let mut argv = packet["argv"]
        .as_array()
        .expect("cooperative packet argv")
        .clone();
    let mut replacements = 0;
    for token in &mut argv {
        if token.as_str() == Some(input_token) {
            *token = Value::String(input_path.display().to_string());
            replacements += 1;
        }
    }
    assert_eq!(
        replacements, 1,
        "hosts replace only the published input token"
    );
    execute_structured_argv(&argv)
}

fn advance_to_promotion_evidence_packet(
    consumer: &Consumer,
    mut next: Value,
    fixture: &str,
) -> Value {
    for _ in 0..80 {
        let status = next["data"]["status"].as_str().expect("guidance status");
        let packet = next["data"]["cooperative_evidence_action_packet"].clone();
        if status == "applicability_required" {
            assert_eq!(packet["route"]["target"], "policy_applicability");
            let mut offer = packet["offer_template"].clone();
            offer["offer_id"] =
                serde_json::json!(format!("offer.promotion-{fixture}-e2e.applicability"));
            offer["attestation"]["applicability_assessment"] = serde_json::json!({
                "outcome": "applicable",
                "summary": "The selected policy applies to this promotion fixture",
                "basis_paths": ["README.md"],
                "limitations": ["same-owner applicability assessment"]
            });
            let input_name = format!("promotion {fixture} applicability.json");
            let input = consumer.write_json(&input_name, &offer);
            assert_ok(&execute_cooperative_packet(&packet, &input));
        } else if packet["input_file_token"].is_string() {
            assert_ne!(packet["route"]["target"], "policy_applicability");
            return next;
        } else {
            assert_eq!(
                status, "ready_to_complete",
                "promotion fixture expected completion, applicability, or evidence: {next}"
            );
            let snapshot = next["data"]["snapshot_digest"]
                .as_str()
                .expect("completion snapshot");
            assert_ok(&consumer.run(&["complete", "--if-snapshot", snapshot]));
        }
        next = assert_ok(&consumer.run(&["next"]));
    }
    panic!("promotion fixture did not reach cooperative evidence");
}

fn append_test_phase(consumer: &Consumer) {
    let mut ledger =
        lock_workflow_governance_ledger_tcb(&consumer.state).expect("lock concurrent ledger");
    let projection = ledger.recover().expect("recover concurrent ledger");
    let head = projection.head_digest.clone().expect("current ledger head");
    let identity = projection
        .active_identity()
        .expect("active identity")
        .clone();
    ledger
        .append_unchecked_tcb_event(
            &head,
            &identity,
            projection.next_state_version,
            WorkflowGovernanceEvent::PhaseAdvanced(PhaseAdvancedEvent {
                from_phase: Some(StableId("1-discovery".to_owned())),
                to_phase: StableId("2-definition".to_owned()),
                snapshot_digest: format!("sha256:{}", "9".repeat(64)),
            }),
        )
        .expect("append concurrent phase event");
}

fn append_test_phase_transition(consumer: &Consumer, from: &str, to: &str, snapshot: &str) {
    let mut ledger =
        lock_workflow_governance_ledger_tcb(&consumer.state).expect("lock phase fixture ledger");
    let projection = ledger.recover().expect("recover phase fixture ledger");
    let head = projection.head_digest.clone().expect("phase fixture head");
    let identity = projection
        .active_identity()
        .expect("phase fixture identity")
        .clone();
    ledger
        .append_unchecked_tcb_event(
            &head,
            &identity,
            projection.next_state_version,
            WorkflowGovernanceEvent::PhaseAdvanced(PhaseAdvancedEvent {
                from_phase: Some(StableId(from.to_owned())),
                to_phase: StableId(to.to_owned()),
                snapshot_digest: snapshot.to_owned(),
            }),
        )
        .expect("append phase fixture transition");
}

fn prepared_episode_candidate(prepared: &Value) -> Value {
    let digest = |byte: char| format!("sha256:{}", byte.to_string().repeat(64));
    let mut input = prepared["data"]["apply_input_template"].clone();
    let episode = &mut input["document"]["post_build_verify_episode"];
    let release_digest = episode["release_subject"]["release_digest"]
        .as_str()
        .expect("prepared release digest")
        .to_owned();
    episode["episode_id"] = serde_json::json!("episode.notes.release.1");
    episode["deployment_observations"] = serde_json::json!([{
        "observation_id": "observation.notes.healthy",
        "release_digest": release_digest,
        "deployment": {
            "subject_ref": "deployment/notes",
            "subject_digest": digest('2')
        },
        "outcome": "healthy",
        "observed_at_unix": 1
    }]);
    episode["operational_evidence"] = serde_json::json!([{
        "evidence_id": "evidence.notes.verification",
        "release_digest": release_digest,
        "evidence": {
            "subject_ref": "evidence/notes",
            "subject_digest": digest('3')
        },
        "kind": "verification",
        "outcome": "supports_readiness",
        "observed_at_unix": 1
    }]);
    episode["evolution"] = serde_json::json!({
        "evolution_episode_id": "evolution.notes.1",
        "generation": 1,
        "release_digest": release_digest,
        "status": "dormant",
        "trigger": "planned_follow_up",
        "proposed_entry_phase": "1-discovery",
        "continuity_subject": {
            "subject_ref": "continuity/notes",
            "subject_digest": digest('4')
        }
    });
    episode["continuity"] = serde_json::json!({
        "context_recovery_subject": {
            "subject_ref": "recovery/notes",
            "subject_digest": digest('5')
        },
        "next_action_ref": "action.monitor-notes"
    });
    episode["episode_digest"] = serde_json::json!(digest('0'));
    input
}

fn state_tree_snapshot(root: &Path) -> Vec<(String, String, Vec<u8>)> {
    fn walk(root: &Path, current: &Path, entries: &mut Vec<(String, String, Vec<u8>)>) {
        let mut children = fs::read_dir(current)
            .unwrap_or_else(|error| panic!("read state tree {}: {error}", current.display()))
            .map(|entry| entry.expect("state tree entry").path())
            .collect::<Vec<_>>();
        children.sort();
        for path in children {
            let relative = path
                .strip_prefix(root)
                .expect("state entry beneath root")
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = fs::symlink_metadata(&path).expect("state entry metadata");
            if metadata.file_type().is_symlink() {
                entries.push((
                    relative,
                    "symlink".to_owned(),
                    fs::read_link(&path)
                        .expect("state symlink target")
                        .to_string_lossy()
                        .as_bytes()
                        .to_vec(),
                ));
            } else if metadata.is_dir() {
                entries.push((relative, "directory".to_owned(), Vec::new()));
                walk(root, &path, entries);
            } else {
                entries.push((
                    relative,
                    "file".to_owned(),
                    fs::read(&path).expect("state file bytes"),
                ));
            }
        }
    }

    let mut entries = Vec::new();
    walk(root, root, &mut entries);
    entries
}

fn run_cooperative_input(consumer: &Consumer, packet_digest: &str, input_path: &Path) -> Output {
    bin()
        .args([
            "workflow",
            "intent",
            "accept-cooperative",
            "--root",
            &consumer.app.display().to_string(),
            "--packet-digest",
            packet_digest,
            "--input-file",
            &input_path.display().to_string(),
            "--json",
        ])
        .output()
        .expect("run cooperative objective command")
}

fn run_current_work_accept(consumer: &Consumer, input_path: &Path) -> Output {
    bin()
        .args([
            "workflow",
            "current-work",
            "accept",
            "--root",
            &consumer.app.display().to_string(),
            "--input-file",
            &input_path.display().to_string(),
            "--json",
        ])
        .output()
        .expect("run Current Work acceptance command")
}

fn run_current_work_prepare(consumer: &Consumer) -> Output {
    bin()
        .args(["workflow", "current-work", "prepare", "--root"])
        .arg(&consumer.app)
        .arg("--json")
        .output()
        .expect("prepare Current Work input")
}

fn replace_exact_json_marker(value: &mut Value, marker: &str, replacement: &Value) {
    match value {
        Value::String(current) if current == marker => *value = replacement.clone(),
        Value::Array(items) => {
            for item in items {
                replace_exact_json_marker(item, marker, replacement);
            }
        }
        Value::Object(fields) => {
            for item in fields.values_mut() {
                replace_exact_json_marker(item, marker, replacement);
            }
        }
        _ => {}
    }
}

fn fill_prepared_current_work(template: &Value, focus_id: &str, title: &str) -> Value {
    let mut input = template.clone();
    for (marker, replacement) in [
        ("${FOCUS_ID}", serde_json::json!(focus_id)),
        ("${TITLE}", serde_json::json!(title)),
        (
            "${INTENDED_OUTCOME}",
            serde_json::json!("A replacement host can continue the accepted work"),
        ),
        (
            "${ACCEPTANCE_SUMMARY}",
            serde_json::json!("Prepared input applies through the existing public command"),
        ),
        ("${NON_GOALS_JSON}", serde_json::json!([])),
        ("${CANONICAL_REFS_JSON}", serde_json::json!(["CONTEXT.md"])),
        (
            "${AFFECTED_AREA_REFS_JSON}",
            serde_json::json!(["crates/forge-core-cli"]),
        ),
        ("${EXTERNAL_WORK_ITEM_REF_JSON}", Value::Null),
        ("${SELECTED_PRACTICE_REF_JSON}", Value::Null),
        ("${SELECTED_PRACTICE_REASON_JSON}", Value::Null),
        (
            "${CURRENT_ACTIVITY}",
            serde_json::json!("Apply the prepared candidate"),
        ),
        (
            "${NEXT_STEP}",
            serde_json::json!("Read Current Work back from Forge"),
        ),
        ("${CONTINUITY_JSON}", Value::Null),
        (
            "${RECORDED_BY}",
            serde_json::json!("principal.agent.cli-e2e"),
        ),
        (
            "${HOST_PROVENANCE_JSON}",
            serde_json::json!({
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.current-work-prepare-e2e",
                "interaction_ref": format!("turn.{focus_id}"),
                "conversation_digest": format!("sha256:{}", "c".repeat(64)),
                "observed_at_unix": 2
            }),
        ),
    ] {
        replace_exact_json_marker(&mut input, marker, &replacement);
    }
    input
}

fn run_current_work_update(consumer: &Consumer, input_path: &Path) -> Output {
    bin()
        .args([
            "workflow",
            "current-work",
            "update",
            "--root",
            &consumer.app.display().to_string(),
            "--input-file",
            &input_path.display().to_string(),
            "--json",
        ])
        .output()
        .expect("run Current Work update command")
}

fn run_current_work_detail(consumer: &Consumer, expected_head_digest: &str) -> Output {
    bin()
        .args([
            "workflow",
            "current-work",
            "detail",
            "--root",
            &consumer.app.display().to_string(),
            "--expected-head-digest",
            expected_head_digest,
            "--json",
        ])
        .output()
        .expect("run Current Work detail command")
}

fn run_current_work_detail_record(
    consumer: &Consumer,
    expected_head_digest: &str,
    record_digest: &str,
) -> Output {
    bin()
        .args([
            "workflow",
            "current-work",
            "detail",
            "--root",
            &consumer.app.display().to_string(),
            "--expected-head-digest",
            expected_head_digest,
            "--record-digest",
            record_digest,
            "--json",
        ])
        .output()
        .expect("run exact Current Work detail command")
}

fn work_focus_record_count(state_root: &Path) -> usize {
    lock_workflow_governance_ledger_tcb(state_root)
        .expect("open ledger for Work Focus count")
        .recover()
        .expect("recover ledger for Work Focus count")
        .records
        .iter()
        .filter(|record| matches!(&record.event, WorkflowGovernanceEvent::WorkFocusRecorded(_)))
        .count()
}

fn run_cooperative_evidence(consumer: &Consumer, input_path: &Path) -> Output {
    bin()
        .args([
            "workflow",
            "evidence",
            "admit-cooperative",
            "--root",
            &consumer.app.display().to_string(),
            "--input-file",
            &input_path.display().to_string(),
            "--json",
        ])
        .output()
        .expect("run cooperative evidence command")
}

fn run_autonomy_assessment(consumer: &Consumer, input_path: &Path) -> Output {
    bin()
        .args([
            "workflow",
            "autonomy",
            "assess",
            "--root",
            &consumer.app.display().to_string(),
            "--input-file",
            &input_path.display().to_string(),
            "--json",
        ])
        .output()
        .expect("run agent autonomy assessment")
}

fn cooperative_decision_json() -> Value {
    serde_json::json!({
        "kind": "decision_required",
        "decision_request": {
            "id": "decision.objective-scope",
            "question": "Should enterprise authority be included in the current objective?",
            "reason": "product_direction",
            "alternatives": [
                {
                    "id": "solo-first",
                    "description": "Keep this objective solo-first",
                    "consequences": ["Enterprise authority remains deferred"]
                },
                {
                    "id": "enterprise-now",
                    "description": "Include enterprise authority now",
                    "consequences": ["The objective becomes materially larger"]
                }
            ],
            "recommended_alternative_ref": "solo-first",
            "blocking": true,
            "blocks_before": "execute"
        }
    })
}

struct StrictHumanBroker {
    key: SigningKey,
    audience: String,
    host_binding: WorkflowBrokerHostBinding,
}

impl StrictHumanBroker {
    fn install(consumer: &Consumer) -> Self {
        let key = SigningKey::from_bytes(&[83; 32]);
        let admin_key = SigningKey::from_bytes(&[84; 32]);
        let project_id = StableId(PROJECT_ID.to_owned());
        let workflow_id = StableId(WORKFLOW_ID.to_owned());
        let audience = workflow_broker_expected_audience(&project_id, &workflow_id);
        let host_binding = WorkflowBrokerHostBinding {
            host_kind: RuntimeKind::ForgeStandalone,
            host_version: "0.12.0".to_owned(),
            adapter_id: StableId("adapter.forge-standalone.integration-e2e".to_owned()),
            adapter_version: "0.1.0".to_owned(),
            host_installation_id: StableId("host.installation.integration-e2e".to_owned()),
            protocol_version: "workflow-host-origin-v1".to_owned(),
        };
        let enrolled_at = now().saturating_sub(60);
        let mut credentials = vec![
            WorkflowBrokerPublicCredentialMetadata {
                credential_id: StableId("credential.workflow.cli-e2e-admin".to_owned()),
                broker_id: StableId("broker.workflow.cli-e2e-admin".to_owned()),
                subject_id: StableId("administrator.workflow.cli-e2e".to_owned()),
                purpose: WorkflowBrokerCredentialPurpose::RegistryAdministrator,
                profile: WorkflowBrokerCredentialProfile::Administrator,
                algorithm: WorkflowBrokerPublicKeyAlgorithm::Ed25519,
                public_key_hex: hex(&admin_key.verifying_key().to_bytes()),
                key_generation: 1,
                status: WorkflowBrokerCredentialStatus::Active,
                custody: WorkflowBrokerCustodyKind::HostIsolatedNonExportable,
                host_binding: host_binding.clone(),
                allowed_operations: Vec::new(),
                not_before_unix: enrolled_at,
                revoked_at_unix: None,
                predecessor_credential_id: None,
                enrollment_operation_id: StableId(
                    "admin.operation.workflow.cli-e2e-genesis".to_owned(),
                ),
                revocation_operation_id: None,
            },
            WorkflowBrokerPublicCredentialMetadata {
                credential_id: StableId("credential.workflow.cli-e2e-human".to_owned()),
                broker_id: StableId("broker.installation.workflow.cli-e2e-human".to_owned()),
                subject_id: StableId(HUMAN_BROKER_ISSUER.to_owned()),
                purpose: WorkflowBrokerCredentialPurpose::EventIssuer,
                profile: WorkflowBrokerCredentialProfile::Human,
                algorithm: WorkflowBrokerPublicKeyAlgorithm::Ed25519,
                public_key_hex: hex(&key.verifying_key().to_bytes()),
                key_generation: 1,
                status: WorkflowBrokerCredentialStatus::Active,
                custody: WorkflowBrokerCustodyKind::HostIsolatedNonExportable,
                host_binding: host_binding.clone(),
                allowed_operations: vec![
                    WorkflowBrokerBoundOperation::Applicability,
                    WorkflowBrokerBoundOperation::Evidence,
                    WorkflowBrokerBoundOperation::IntentRevision,
                ],
                not_before_unix: enrolled_at,
                revoked_at_unix: None,
                predecessor_credential_id: None,
                enrollment_operation_id: StableId(
                    "admin.operation.workflow.cli-e2e-genesis".to_owned(),
                ),
                revocation_operation_id: None,
            },
        ];
        credentials.sort_by(|left, right| left.credential_id.0.cmp(&right.credential_id.0));
        let document = WorkflowBrokerPublicRegistryDocument {
            schema_version: WORKFLOW_BROKER_PUBLIC_REGISTRY_SCHEMA_VERSION.to_owned(),
            audience: audience.clone(),
            project_id: project_id.clone(),
            workflow_id: workflow_id.clone(),
            registry_generation: 1,
            previous_registry_digest: None,
            required_event_schema_version: WORKFLOW_BROKER_REQUIRED_EVENT_SCHEMA_VERSION.to_owned(),
            credentials,
        };
        let operator_dir = consumer
            .state
            .parent()
            .expect("operator root")
            .join("operator");
        workflow_broker_test_support::install_strict_broker_genesis(
            &operator_dir,
            document,
            &admin_key,
        );
        Self {
            key,
            audience,
            host_binding,
        }
    }

    fn apply(
        &self,
        consumer: &Consumer,
        packet: &Value,
        semantic_input: WorkflowBrokerSemanticInput,
        label: &str,
    ) -> Value {
        static NONCE: AtomicU64 = AtomicU64::new(0);
        let sequence = NONCE.fetch_add(1, Ordering::SeqCst);
        let issued = now();
        let mut envelope = WorkflowBrokerEventEnvelope {
            schema_version: WORKFLOW_BROKER_EVENT_SCHEMA_VERSION.to_owned(),
            audience: self.audience.clone(),
            issuer_id: StableId(HUMAN_BROKER_ISSUER.to_owned()),
            issuer_profile: WorkflowBrokerIssuerProfile::Human,
            origin_principal_id: PrincipalId(HUMAN_BROKER_PRINCIPAL.to_owned()),
            separation_domain: StableId("human-session.workflow.cli-e2e".to_owned()),
            event_kind: semantic_input.kind(),
            project_id: StableId(
                packet["binding"]["project_id"]
                    .as_str()
                    .expect("packet project id")
                    .to_owned(),
            ),
            action_packet_digest: packet["packet_digest"]
                .as_str()
                .expect("packet digest")
                .to_owned(),
            semantic_input,
            native_host_provenance: Some(WorkflowBrokerNativeHostProvenance {
                host_kind: self.host_binding.host_kind,
                host_version: self.host_binding.host_version.clone(),
                adapter_id: self.host_binding.adapter_id.clone(),
                adapter_version: self.host_binding.adapter_version.clone(),
                interaction_kind: WorkflowBrokerHostInteractionKind::NativeHumanConfirmation,
                host_event_ref: format!("host-event-{label}-{sequence:04}"),
                host_session_ref: "host-session-workflow-cli-e2e".to_owned(),
                host_interaction_ref: format!("host-interaction-{label}-{sequence:04}"),
                host_event_descriptor_digest: format!("sha256:{}", "0".repeat(64)),
                host_observed_at_unix: issued,
            }),
            issued_at_unix: issued,
            // The full workspace runs several process-heavy E2Es concurrently;
            // use the longest envelope lifetime admitted by production policy.
            expires_at_unix: issued + 300,
            nonce: format!("workflow-cli-e2e-{label}-{issued}-{sequence}"),
            signature: String::new(),
        };
        let provenance = envelope
            .native_host_provenance
            .as_mut()
            .expect("native host provenance");
        provenance.host_event_descriptor_digest = workflow_broker_host_event_descriptor_digest(
            provenance,
            &envelope.project_id,
            &envelope.action_packet_digest,
            &envelope.semantic_input,
        )
        .expect("host event descriptor digest");
        envelope.signature = hex(&self
            .key
            .sign(&workflow_broker_event_signing_bytes(&envelope).expect("event signing bytes"))
            .to_bytes());
        let path = consumer.write_json(&format!("{label}-{sequence}.json"), &envelope);
        let root = consumer.app.display().to_string();
        assert_ok(
            &bin()
                .args([
                    "workflow",
                    "action",
                    "apply",
                    "--root",
                    &root,
                    "--origin-envelope-file",
                    &path.display().to_string(),
                    "--json",
                ])
                .output()
                .expect("apply strict broker action"),
        )
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len().saturating_mul(2)),
        |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
            output
        },
    )
}

fn required_str<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("guidance field '{field}' must be a string: {value:#}"))
}

fn action_packet<'a>(packet_set: &'a Value, kind: &str) -> &'a Value {
    packet_set["data"]["packets"]
        .as_array()
        .expect("action packet list")
        .iter()
        .find(|packet| packet["authorization_kind"] == kind)
        .unwrap_or_else(|| panic!("missing {kind} action packet: {packet_set:#}"))
}

#[test]
fn legacy_profileless_project_explicitly_adopts_solo_and_exact_retry_is_one_write() {
    let consumer =
        Consumer::new_start_ready_with_prefix("forge workflow legacy project with spaces");
    replace_with_legacy_profileless_genesis(&consumer);
    let wal = consumer.state.join("wal/workflow-governance.ndjson");
    let legacy_bytes = fs::read(&wal).expect("legacy WAL bytes");

    let started = assert_ok(
        &bin()
            .args([
                "start",
                "--root",
                consumer.app.to_str().expect("UTF-8 project root"),
                "--json",
            ])
            .output()
            .expect("run start against legacy project"),
    );
    assert_eq!(started["command"], "start");
    assert_eq!(
        fs::read(&wal).expect("WAL after start"),
        legacy_bytes,
        "start must never adopt or rewrite a profile-less ledger",
    );

    let repeated_init = assert_ok(&consumer.run(&["init"]));
    assert_eq!(
        repeated_init["data"]["readiness_profile"],
        "strict_external"
    );
    assert_eq!(fs::read(&wal).expect("WAL after init"), legacy_bytes);

    let status = assert_ok(&run_profile(&consumer, "status", &[]));
    assert_eq!(status["command"], "workflow.profile.status");
    assert_eq!(status["data"]["current_profile"], "strict_external");
    assert_eq!(status["data"]["legacy_profileless_genesis"], true);
    assert_eq!(status["data"]["solo_adoption"], "eligible");
    let argv = status["data"]["adopt_solo_argv"]
        .as_array()
        .expect("exact adoption argv");
    assert_eq!(
        argv[5].as_str().expect("root argv"),
        consumer.app.display().to_string()
    );

    let adopted = assert_ok(&execute_structured_argv(argv));
    assert_eq!(adopted["command"], "workflow.profile.adopt_solo");
    assert_eq!(adopted["data"]["status"], "adopted");
    assert_eq!(adopted["data"]["readiness_profile"], "solo_cooperative");
    assert_eq!(adopted["data"]["provenance"], "cooperative_same_owner");
    let adopted_bytes = fs::read(&wal).expect("adopted WAL bytes");
    assert!(adopted_bytes.starts_with(&legacy_bytes));

    fs::write(
        consumer.app.join("README.md"),
        "project changed after the durable adoption\n",
    )
    .expect("change project after adoption");
    let retried = assert_ok(&execute_structured_argv(argv));
    assert_eq!(retried["data"]["status"], "already_adopted");
    assert_eq!(
        retried["data"]["snapshot_digest"], adopted["data"]["snapshot_digest"],
        "the retry receipt must describe the original transition",
    );
    assert_eq!(fs::read(&wal).expect("retry WAL"), adopted_bytes);

    for action in ["next", "resume"] {
        let guidance = assert_ok(&consumer.run(&[action]));
        assert_eq!(guidance["data"]["readiness_profile"], "solo_cooperative");
        assert_eq!(
            guidance["data"]["durable_assurance"]["status"],
            "missing_objective"
        );
        assert_eq!(
            guidance["data"]["authorization"]["action_packets"][0]["required_authority"]
                ["approval_boundary"],
            "cooperative_same_owner"
        );
        assert!(guidance["data"]["authorization"]["setup_gaps"]
            .as_array()
            .is_some_and(Vec::is_empty));
    }
}

#[test]
fn legacy_profile_adoption_rejects_stale_snapshot_and_explicit_strict_without_writes() {
    let legacy = Consumer::new();
    replace_with_legacy_profileless_genesis(&legacy);
    let status = assert_ok(&run_profile(&legacy, "status", &[]));
    let argv = status["data"]["adopt_solo_argv"]
        .as_array()
        .expect("eligible argv")
        .clone();
    fs::write(
        legacy.app.join("README.md"),
        "changed after status
",
    )
    .expect("change project");
    let wal = legacy.state.join("wal/workflow-governance.ndjson");
    let before = fs::read(&wal).expect("WAL before stale apply");
    let stale = execute_structured_argv(&argv);
    assert_eq!(stale.status.code(), Some(4));
    assert_eq!(json(&stale)["exit_reason"], "conflict");
    assert_eq!(fs::read(&wal).expect("WAL after stale apply"), before);

    let explicit = Consumer::new();
    assert_ok(&explicit.run(&["init", "--readiness-profile", "strict_external"]));
    let before = state_tree_snapshot(&explicit.state);
    let status = assert_ok(&run_profile(&explicit, "status", &[]));
    assert_eq!(status["data"]["solo_adoption"], "ineligible");
    assert!(status["data"]["adopt_solo_argv"].is_null());
    assert_eq!(state_tree_snapshot(&explicit.state), before);

    let solo = Consumer::new();
    assert_ok(&solo.run(&["init", "--readiness-profile", "solo_cooperative"]));
    let before = state_tree_snapshot(&solo.state);
    let status = assert_ok(&run_profile(&solo, "status", &[]));
    assert_eq!(status["data"]["current_profile"], "solo_cooperative");
    assert_eq!(status["data"]["solo_adoption"], "already_solo");
    assert!(status["data"]["adopt_solo_argv"].is_null());
    assert_eq!(state_tree_snapshot(&solo.state), before);
}

#[test]
fn legacy_profile_adoption_rejects_concurrent_heads_and_retries_after_later_records() {
    let concurrent = Consumer::new();
    replace_with_legacy_profileless_genesis(&concurrent);
    let status = assert_ok(&run_profile(&concurrent, "status", &[]));
    let argv = status["data"]["adopt_solo_argv"]
        .as_array()
        .expect("eligible concurrent argv")
        .clone();
    append_test_phase(&concurrent);
    let wal = concurrent.state.join("wal/workflow-governance.ndjson");
    let before = fs::read(&wal).expect("WAL after concurrent head");
    let stale = execute_structured_argv(&argv);
    assert_eq!(stale.status.code(), Some(4));
    assert_eq!(json(&stale)["exit_reason"], "conflict");
    assert_eq!(
        fs::read(&wal).expect("WAL after rejected stale head"),
        before
    );

    let later = Consumer::new();
    replace_with_legacy_profileless_genesis(&later);
    let status = assert_ok(&run_profile(&later, "status", &[]));
    let argv = status["data"]["adopt_solo_argv"]
        .as_array()
        .expect("eligible retry argv")
        .clone();
    assert_ok(&execute_structured_argv(&argv));
    append_test_phase(&later);
    let wal = later.state.join("wal/workflow-governance.ndjson");
    let before = fs::read(&wal).expect("WAL with later record");
    let retry = execute_structured_argv(&argv);
    assert_eq!(retry.status.code(), Some(4));
    assert_eq!(json(&retry)["exit_reason"], "conflict");
    assert_eq!(
        fs::read(&wal).expect("WAL after rejected late retry"),
        before
    );
}

#[test]
#[allow(clippy::too_many_lines)] // One projection chain keeps next and packet CAS assertions auditable.
fn fresh_agent_resumes_same_automatically_selected_governance_state() {
    let consumer = Consumer::new();
    let initialized = assert_ok(&consumer.run(&["init"]));
    assert_eq!(initialized["data"]["readiness_profile"], "solo_cooperative");
    assert_eq!(initialized["data"]["current_phase"], "1-discovery");
    assert_eq!(initialized["data"]["state_version"], 0);
    let repeated = assert_ok(&consumer.run(&["init"]));
    assert_eq!(repeated["data"]["status"], "already_initialized");
    for field in [
        "readiness_profile",
        "head_digest",
        "state_version",
        "current_phase",
    ] {
        assert_eq!(
            repeated["data"][field], initialized["data"][field],
            "{field}"
        );
    }
    assert!(consumer
        .state
        .join("wal/workflow-governance.ndjson")
        .is_file());

    let next = assert_ok(&consumer.run(&["next"]));
    assert_eq!(
        next["data"]["selected_policy_ref"],
        "policy.workflow.discover-intent"
    );
    assert_eq!(next["data"]["current_phase"], "1-discovery");
    assert_eq!(next["data"]["authority"], "verified_project_snapshot");
    assert_eq!(next["data"]["readiness_profile"], "solo_cooperative");
    assert_eq!(
        next["data"]["durable_assurance"]["status"],
        "missing_objective"
    );
    assert_eq!(
        next["data"]["durable_assurance"]["blockers"][0]["code"],
        "missing_accepted_objective"
    );
    assert_eq!(
        next["data"]["authorization"]["action_packets"][0]["required_authority"]
            ["approval_boundary"],
        "cooperative_same_owner"
    );
    assert!(next["data"]["authorization"]["setup_gaps"]
        .as_array()
        .is_some_and(Vec::is_empty));
    assert_ne!(
        next["data"]["simulation"]["candidate_status"], "complete",
        "artifact-free fluent progress must not appear complete"
    );

    let action_packets = assert_ok(&consumer.run(&["action-packets"]));
    assert_eq!(
        action_packets["data"]["project_id"],
        next["data"]["project_id"]
    );
    assert_eq!(
        action_packets["data"]["snapshot_digest"],
        next["data"]["snapshot_digest"]
    );
    assert_eq!(
        action_packets["data"]["ledger_head_digest"],
        next["data"]["ledger_head_digest"]
    );
    let packets = action_packets["data"]["packets"]
        .as_array()
        .expect("typed workflow action packet list");
    assert_eq!(packets.len(), 1);
    assert_eq!(
        packets[0]["required_authority"]["approval_boundary"], "cooperative_same_owner",
        "solo profile projects only its same-owner objective packet"
    );
    assert_eq!(
        packets[0]["input_contract"]["kind"],
        "cooperative_objective"
    );
    assert_eq!(
        packets[0]["input_contract"]["input_encoding"],
        "utf8_json_file"
    );
    assert_eq!(
        packets[0]["input_contract"]["unknown_fields_allowed"],
        false
    );
    assert_eq!(
        packets[0]["input_contract"]["variants"][0]["variant"],
        "unambiguous"
    );
    assert_eq!(
        packets[0]["input_contract"]["variants"][1]["variant"],
        "decision_required"
    );
    assert_eq!(
        packets[0]["input_contract"]["limits"]["input_max_bytes"],
        128 * 1024
    );
    assert_eq!(
        packets[0]["input_contract"]["command_argv_template"],
        serde_json::json!([
            "forge-core",
            "workflow",
            "intent",
            "accept-cooperative",
            "--root",
            "<project-root>",
            "--packet-digest",
            "<packet-digest>",
            "--input-file",
            "<temporary-utf8-json-path>",
            "--json"
        ])
    );

    let summary_output = consumer.run(&["resume"]);
    let summary = assert_ok(&summary_output);
    assert_eq!(
        summary["data"]["schema_version"],
        "workflow_resume_summary_v10"
    );
    assert_eq!(summary["data"]["detail_level"], "summary");
    assert_eq!(
        summary["data"]["current_work"]["schema_version"],
        "current_work_context_v3"
    );
    assert_eq!(summary["data"]["current_work"]["status"], "absent");
    assert_eq!(
        summary["data"]["forge_core_version"],
        env!("CARGO_PKG_VERSION")
    );
    for field in [
        "authority",
        "status",
        "readiness_profile",
        "project_id",
        "current_phase",
        "target",
        "snapshot_digest",
        "ledger_head_digest",
        "state_version",
        "release",
        "bundle_id",
        "bundle_digest",
        "effective",
        "selected_policy_ref",
        "compatibility_workflow_id",
        "applicability",
    ] {
        assert_eq!(summary["data"][field], next["data"][field], "{field}");
    }
    assert!(
        summary["data"].get("simulation").is_none()
            && summary["data"].get("replacement_continuity").is_none(),
        "the default activation view must not repeat the full audit"
    );
    assert_eq!(
        summary["data"]["agent_autonomy"], next["data"]["agent_autonomy"],
        "resume v2 must retain the complete current autonomy contract"
    );
    assert_eq!(
        summary["data"]["current_evaluation"], next["data"]["simulation"],
        "resume v2 must retain every current verdict, gap, decision, issue, and next action"
    );
    assert_eq!(
        summary["data"]["boundary_rechecks"], next["data"]["boundary_rechecks"],
        "resume v2 must retain every current boundary recheck"
    );
    assert_eq!(
        summary["data"]["authorization"], next["data"]["authorization"],
        "resume v2 must retain complete current action packets and setup gaps"
    );
    assert_ne!(
        summary["data"]["schema_version"], "workflow_resume_summary_v1",
        "the changed wire contract must never be emitted as v1"
    );

    let resumed_output = consumer.run(&["report"]);
    let resumed = assert_ok(&resumed_output);
    assert!(
        summary_output.stdout.len() < resumed_output.stdout.len(),
        "the activation view must remain smaller than the historical report"
    );
    for field in [
        "readiness_profile",
        "selected_policy_ref",
        "snapshot_digest",
        "ledger_head_digest",
        "state_version",
        "current_phase",
    ] {
        assert_eq!(resumed["data"][field], next["data"][field], "{field}");
    }
    let continuity = &resumed["data"]["replacement_continuity"];
    assert_eq!(
        continuity["schema_version"],
        "workflow_replacement_continuity_v1"
    );
    assert_eq!(continuity["status"], "ready");
    assert_eq!(
        continuity["binding"]["ledger_head_digest"],
        next["data"]["ledger_head_digest"]
    );
    assert_eq!(
        continuity["binding"]["project_snapshot_digest"],
        next["data"]["snapshot_digest"]
    );
    assert!(continuity["objective_history"]
        .as_array()
        .is_some_and(Vec::is_empty));
    assert!(continuity["durable_pending_decisions"]
        .as_array()
        .is_some_and(Vec::is_empty));
    assert!(continuity["decision_history"]
        .as_array()
        .is_some_and(Vec::is_empty));
    assert!(
        resumed["data"]["simulation"]["candidate_decision_requests"].is_array(),
        "questions calculated now remain in the simulation and are not reported as recovered history"
    );
    assert!(continuity["isolations"]
        .as_array()
        .is_some_and(Vec::is_empty));
    assert!(continuity["promotions"]
        .as_array()
        .is_some_and(Vec::is_empty));
    assert_eq!(
        continuity["ranked_next_actions"][0]["governed_action"],
        next["data"]["simulation"]["candidate_next_actions"][0]
    );
    let second_process = assert_ok(&consumer.run(&["report"]));
    assert_eq!(
        second_process["data"]["replacement_continuity"]["ranked_action_digest"],
        continuity["ranked_action_digest"],
        "fresh processes must rank the same durable next action"
    );

    let shadow = assert_ok(&consumer.run(&["shadow"]));
    assert_eq!(shadow["data"]["mutation_allowed"], false);
    assert_eq!(shadow["data"]["retirement_allowed"], false);
    assert_eq!(
        shadow["data"]["selected_policy_ref"],
        "policy.workflow.discover-intent"
    );

    let forged = consumer.run(&["next", "--policy", "policy.workflow.ready-release"]);
    assert_eq!(forged.status.code(), Some(3));
    let forged = json(&forged);
    assert_eq!(forged["exit_reason"], "invalid_decision_shape");
    assert!(forged["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("forbidden")));

    for removed in [
        "observe-artifact",
        "run-evaluator",
        "capability-probe",
        "applicability-assess",
        "signal",
        "revoke",
    ] {
        let output = consumer.run(&[removed]);
        assert_eq!(output.status.code(), Some(3), "{removed}");
        let envelope = json(&output);
        assert_eq!(envelope["exit_reason"], "invalid_decision_shape");
        assert!(envelope["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("unknown workflow subcommand")));
    }

    // Workflow authorization always resolves the registry from the trusted
    // sidecar. A caller cannot inject a different trust root per operation.
    let caller_selected_registry = consumer.run(&[
        "evidence-authorize",
        "--principal-registry",
        "attacker-controlled.yaml",
        "--request-file",
        "request.json",
        "--attestation-file",
        "attestation.json",
    ]);
    assert_eq!(caller_selected_registry.status.code(), Some(3));
    let envelope = json(&caller_selected_registry);
    assert_eq!(envelope["exit_reason"], "invalid_decision_shape");
    assert!(envelope["error"]["message"]
        .as_str()
        .is_some_and(|message| {
            message.contains("unrecognized workflow argument '--principal-registry'")
        }));
}

#[test]
fn workflow_next_keeps_its_full_contract_and_rejects_resume_detail_flags() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));
    let next = assert_ok(&consumer.run(&["next"]));
    assert!(
        next["data"]["simulation"].is_object()
            && next["data"]["agent_autonomy"].is_object()
            && next["data"]["authorization"].is_object()
            && next["data"]["boundary_rechecks"].is_array(),
        "workflow next must remain the existing complete current-state projection"
    );

    let summary = consumer.run(&["next", "--summary"]);
    assert_eq!(summary.status.code(), Some(3));
    assert!(json(&summary)["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("unrecognized workflow argument '--summary'")));

    let full = consumer.run(&["next", "--full"]);
    assert_eq!(full.status.code(), Some(3));
    assert!(json(&full)["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("unrecognized workflow argument '--full'")));
}

#[test]
fn workflow_resume_does_not_create_a_missing_domain_pack_lock() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));
    let lock = consumer.state.join("locks/domain-packs.lifecycle.lock");
    fs::remove_file(&lock).expect("remove existing lifecycle lock");
    let before = state_tree_snapshot(&consumer.state);

    let resumed = consumer.run(&["report"]);
    assert!(
        !resumed.status.success(),
        "read-only resume must stop when its existing lock is absent"
    );
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        before,
        "resume must not create or alter Forge state while observing a missing lock"
    );
    assert!(!lock.exists(), "resume must not recreate the missing lock");
}

#[test]
fn cooperative_objective_cli_commits_once_and_fresh_next_reads_the_ledger() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));
    let next = assert_ok(&consumer.run(&["next"]));
    let packet_digest = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("cooperative packet digest")
        .to_owned();
    let input = consumer.write_json(
        "cooperative objective with spaces.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Use Forge to improve Forge as a solo developer with agents",
                "constraints": ["remain host neutral"],
                "unacceptable_outcomes": ["claim verified human origin"],
                "open_uncertainties": ["future team authority"]
            },
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.cli-e2e",
                "interaction_ref": "turn.cli-e2e",
                "conversation_digest": format!("sha256:{}", "a".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let accepted = assert_ok(&run_cooperative_input(&consumer, &packet_digest, &input));
    assert_eq!(accepted["data"]["status"], "accepted");
    assert_eq!(
        accepted["data"]["active_objective"]["authority_basis"],
        "cooperative_same_owner"
    );
    let wal = consumer.state.join("wal/workflow-governance.ndjson");
    let accepted_wal = fs::read(&wal).expect("accepted WAL");
    let retry = assert_ok(&run_cooperative_input(&consumer, &packet_digest, &input));
    assert_eq!(
        retry["data"], accepted["data"],
        "an exact operational retry must reproduce the same receipt/readback"
    );
    assert_eq!(
        fs::read(&wal).expect("WAL after retry"),
        accepted_wal,
        "exact retry must not append"
    );

    let divergent = consumer.write_json(
        "divergent cooperative objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Use Forge for a materially different objective",
                "constraints": ["remain host neutral"],
                "unacceptable_outcomes": ["claim verified human origin"],
                "open_uncertainties": ["future team authority"]
            },
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.cli-e2e",
                "interaction_ref": "turn.cli-e2e",
                "conversation_digest": format!("sha256:{}", "a".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let conflict = run_cooperative_input(&consumer, &packet_digest, &divergent);
    assert_eq!(conflict.status.code(), Some(4));
    let conflict = json(&conflict);
    assert_eq!(conflict["command"], "workflow.intent.accept_cooperative");
    assert_eq!(conflict["exit_reason"], "conflict");
    assert_eq!(
        fs::read(&wal).expect("WAL after divergent retry"),
        accepted_wal
    );

    let fresh = assert_ok(&consumer.run(&["next"]));
    assert_eq!(
        fresh["data"]["durable_assurance"]["status"],
        "objective_accepted"
    );
    assert_eq!(
        fresh["data"]["active_cooperative_objective"]["objective_digest"],
        accepted["data"]["active_objective"]["objective_digest"]
    );
    assert_eq!(
        fresh["data"]["selected_policy_ref"],
        "policy.workflow.discover-intent"
    );
    assert_eq!(
        fresh["data"]["status"], "ready_to_complete",
        "the already accepted same-owner objective must ground discover-intent without a redundant evidence command"
    );
    assert!(
        fresh["data"]["simulation"]["candidate_claim_results"]
            .as_array()
            .is_some_and(|claims| claims.iter().all(|claim| claim["status"] == "verified")),
        "the accepted objective must satisfy the discover-intent claim"
    );
    assert!(
        fresh["data"]["cooperative_evidence_action_packet"].is_null(),
        "Forge must not ask the agent to re-submit project-snapshot evidence for intent it already accepted"
    );
    assert!(
        fresh["data"]["cooperative_evidence_action_gap"].is_null(),
        "an accepted objective is not a missing-evidence gap"
    );
    let objective_record_digest = accepted["data"]["objective_record"]["record_digest"]
        .as_str()
        .expect("accepted objective record digest")
        .to_owned();
    assert_eq!(
        fresh["data"]["simulation"]["candidate_claim_results"][0]["accepted_grounding_refs"],
        serde_json::json!([format!(
            "cooperative-objective:{objective_record_digest}"
        )]),
        "the claim audit must identify the exact same-owner objective record, not imply broker proof"
    );

    let decision =
        consumer.write_json("decision after accepted.json", &cooperative_decision_json());
    let before_decision = state_tree_snapshot(&consumer.state);
    let rejected = run_cooperative_input(&consumer, &packet_digest, &decision);
    assert_eq!(rejected.status.code(), Some(4));
    assert_eq!(
        json(&rejected)["command"],
        "workflow.intent.accept_cooperative"
    );
    assert_eq!(state_tree_snapshot(&consumer.state), before_decision);

    let completion_snapshot = fresh["data"]["snapshot_digest"]
        .as_str()
        .expect("ready completion snapshot");
    let resumed_ready = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(
        resumed_ready["data"]["actions"]["recommended"]["kind"],
        "complete_workflow"
    );
    assert_eq!(
        resumed_ready["data"]["actions"]["completion"]["argv"],
        serde_json::json!([
            "forge-core",
            "workflow",
            "complete",
            "--root",
            fs::canonicalize(&consumer.app)
                .expect("canonical consumer root")
                .to_string_lossy(),
            "--if-snapshot",
            completion_snapshot,
            "--principal",
            "principal.agent.cli-e2e",
            "--json"
        ])
    );
    let completed = assert_ok(&consumer.run(&[
        "complete",
        "--if-snapshot",
        completion_snapshot,
        "--principal",
        "principal.agent.cli-e2e",
    ]));
    assert_eq!(
        completed["data"]["completed_record"]["event"]["payload"]["evidence_receipt_digests"],
        serde_json::json!([]),
        "same-owner project direction is grounding, not evaluator evidence"
    );
    assert_eq!(
        completed["data"]["completed_record"]["event"]["payload"]["grounding_anchor_digests"],
        serde_json::json!([objective_record_digest]),
        "completion must remain durably bound to the exact material objective anchor"
    );
    assert_eq!(
        completed["data"]["completed_record"]["event"]["payload"]["subject"]["kind"],
        "artifact"
    );
    assert_eq!(
        completed["data"]["completed_record"]["event"]["payload"]["subject"]["subject_digest"],
        objective_record_digest
    );

    fs::write(
        consumer.app.join("README.md"),
        "governed edit after discover-intent completion\n",
    )
    .expect("governed edit after completion");
    let after_edit = assert_ok(&consumer.run(&["next"]));
    assert_eq!(
        after_edit["data"]["selected_policy_ref"], "policy.workflow.domain-scan",
        "ordinary project edits must not reopen an objective-anchored completion"
    );
}

#[test]
fn current_work_prepare_is_read_only_and_selects_the_existing_accept_path() {
    let consumer = Consumer::new_with_prefix("forge-current-work-prepare-e2e");
    assert_ok(&consumer.run(&["init"]));
    let next = assert_ok(&consumer.run(&["next"]));
    let packet_digest = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("cooperative packet digest");
    let objective_input = consumer.write_json(
        "prepare objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Let a replacement host prepare Current Work without source knowledge",
                "constraints": ["keep preparation read-only and on demand"],
                "unacceptable_outcomes": ["duplicate the Work Focus schema in the host"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.current-work-prepare-e2e",
                "interaction_ref": "turn.accept-objective",
                "conversation_digest": format!("sha256:{}", "a".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    assert_ok(&run_cooperative_input(
        &consumer,
        packet_digest,
        &objective_input,
    ));

    let state_before = state_tree_snapshot(&consumer.state);
    let records_before = work_focus_record_count(&consumer.state);
    let prepared = assert_ok(&run_current_work_prepare(&consumer));
    assert_eq!(prepared["command"], "workflow.current_work_prepare");
    assert_eq!(
        prepared["data"]["schema_version"],
        "current_work_preparation_v1"
    );
    assert_eq!(prepared["data"]["authority"], "candidate_preparation_only");
    assert_eq!(prepared["data"]["current_work_status"], "absent");
    assert_eq!(prepared["data"]["operation"], "accept");
    assert_eq!(
        prepared["data"]["apply_input_schema_version"],
        "work_focus_accept_input_v3"
    );
    assert_eq!(
        prepared["data"]["binding"]["expected_work_focus"]["status"],
        "absent"
    );
    assert_eq!(
        prepared["data"]["maximum_input_bytes"],
        MAX_WORK_FOCUS_ACCEPT_INPUT_BYTES
    );
    assert_eq!(
        prepared["data"]["input_file_token"],
        "${CURRENT_WORK_INPUT_FILE}"
    );
    assert_eq!(
        prepared["data"]["apply_argv"]
            .as_array()
            .expect("apply argv array")
            .iter()
            .take(4)
            .cloned()
            .collect::<Vec<_>>(),
        serde_json::json!(["forge-core", "workflow", "current-work", "accept"])
            .as_array()
            .expect("expected argv array")
            .clone()
    );
    assert_eq!(
        prepared["data"]["apply_input_template"]["schema_version"],
        "work_focus_accept_input_v3"
    );
    assert_eq!(
        prepared["data"]["apply_input_template"]["expected_snapshot_digest"],
        prepared["data"]["binding"]["snapshot_digest"]
    );
    assert_eq!(
        prepared["data"]["input_file_must_be_outside_project_snapshot"],
        true
    );
    assert!(prepared["data"]["required_replacements"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
    assert!(
        serde_json::to_vec(&prepared["data"])
            .expect("preparation packet bytes")
            .len()
            <= MAX_CURRENT_WORK_PREPARATION_BYTES
    );
    assert!(prepared["data"]["readback_contract"]
        .as_str()
        .is_some_and(|value| value.contains("writes no Forge state")));
    assert_eq!(work_focus_record_count(&consumer.state), records_before);
    assert_eq!(state_tree_snapshot(&consumer.state), state_before);

    let accept_input = fill_prepared_current_work(
        &prepared["data"]["apply_input_template"],
        "focus.prepared-first",
        "Prepared first focus",
    );
    let accept_path = consumer.write_json("prepared-current-work-accept.json", &accept_input);
    assert_ok(&run_current_work_accept(&consumer, &accept_path));
    assert_eq!(work_focus_record_count(&consumer.state), records_before + 1);

    let state_after_accept = state_tree_snapshot(&consumer.state);
    let prepared_update = assert_ok(&run_current_work_prepare(&consumer));
    assert_eq!(prepared_update["data"]["current_work_status"], "current");
    assert_eq!(prepared_update["data"]["operation"], "supersede");
    assert_eq!(
        prepared_update["data"]["apply_input_schema_version"],
        "work_focus_update_input_v3"
    );
    assert_eq!(
        prepared_update["data"]["binding"]["expected_work_focus"]["record_digest"],
        prepared_update["data"]["apply_input_template"]["expected_work_focus"]["record_digest"]
    );
    assert_eq!(state_tree_snapshot(&consumer.state), state_after_accept);

    let update_input = fill_prepared_current_work(
        &prepared_update["data"]["apply_input_template"],
        "focus.prepared-second",
        "Prepared replacement focus",
    );
    let update_path = consumer.write_json("prepared-current-work-update.json", &update_input);
    assert_ok(&run_current_work_update(&consumer, &update_path));
    assert_eq!(work_focus_record_count(&consumer.state), records_before + 2);
    let state_after_update = state_tree_snapshot(&consumer.state);
    let stale_update = run_current_work_update(&consumer, &update_path);
    assert_eq!(stale_update.status.code(), Some(4));
    assert_eq!(json(&stale_update)["exit_reason"], "conflict");
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        state_after_update,
        "stale prepared bindings must not alter WAL or state"
    );
    let resume = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(
        resume["data"]["current_work"]["focus"]["focus_id"],
        "focus.prepared-second"
    );
}

#[test]
fn current_work_accepts_and_transitions_exact_focus_with_resume_readback() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));
    let next = assert_ok(&consumer.run(&["next"]));
    let packet_digest = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("cooperative packet digest");
    let objective_input = consumer.write_json(
        "current-work objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Use Forge to improve Forge through dogfooding",
                "constraints": ["keep slices small and testable"],
                "unacceptable_outcomes": ["depend on chat memory"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.current-work-e2e",
                "interaction_ref": "turn.accept-objective",
                "conversation_digest": format!("sha256:{}", "a".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    assert_ok(&run_cooperative_input(
        &consumer,
        packet_digest,
        &objective_input,
    ));

    let before = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(before["data"]["current_work"]["status"], "absent");
    let focus_input = consumer.write_json(
        "current-work accept.json",
        &serde_json::json!({
            "schema_version": "work_focus_accept_input_v1",
            "expected_snapshot_digest": before["data"]["snapshot_digest"],
            "expected_ledger_head_digest": before["data"]["ledger_head_digest"],
            "expected_state_version": before["data"]["state_version"],
            "expected_work_focus": { "status": "absent" },
            "focus": {
                "focus_id": "focus.issue-34.slice-1",
                "title": "Publish accepted Work Focus updates",
                "intended_outcome": "A replacement agent can see the active issue after one resume",
                "acceptance_summary": "The public command records one exact focus and resume reads it back",
                "non_goals": ["bind blockers in this slice"],
                "canonical_refs": ["contracts/spec/product-journey-guidance-v0.yaml"],
                "affected_area_refs": ["crates/forge-core-cli"],
                "external_work_item_ref": "https://github.com/DanielCarva1/forge-method-core/issues/34",
                "selected_practice_ref": "edge-case-review",
                "selected_practice_reason": "Stress the replacement-host journey without adding another workflow",
                "current_activity": "Add the first public Work Focus write path",
                "next_step": "Dogfood the command on Forge itself"
            },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.current-work-e2e",
                "interaction_ref": "turn.accept-current-work",
                "conversation_digest": format!("sha256:{}", "b".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );

    let accepted = assert_ok(&run_current_work_accept(&consumer, &focus_input));
    assert_eq!(accepted["command"], "workflow.current_work_accept");
    assert_eq!(accepted["data"]["current_work"]["status"], "current");
    assert_eq!(
        accepted["data"]["current_work"]["focus"]["focus_id"],
        "focus.issue-34.slice-1"
    );

    let wal = consumer.state.join("wal/workflow-governance.ndjson");
    let accepted_wal = fs::read(&wal).expect("WAL after accepted Work Focus");
    let stale_retry = run_current_work_accept(&consumer, &focus_input);
    assert_eq!(stale_retry.status.code(), Some(4));
    let stale_retry = json(&stale_retry);
    assert_eq!(stale_retry["command"], "workflow.current_work_accept");
    assert_eq!(stale_retry["exit_reason"], "conflict");
    assert_eq!(
        fs::read(&wal).expect("WAL after rejected stale Work Focus retry"),
        accepted_wal,
        "a stale retry must not append another Work Focus record"
    );

    let state_before_ordinary_resume = state_tree_snapshot(&consumer.state);
    let resumed = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        state_before_ordinary_resume,
        "ordinary continuation through read-only resume must not append a Work Focus event"
    );
    assert!(
        serde_json::to_vec(&resumed["data"]["current_work"])
            .expect("serialize Current Work summary")
            .len()
            <= MAX_CURRENT_WORK_SUMMARY_BYTES
    );
    assert_eq!(resumed["data"]["current_work"]["status"], "current");
    assert_eq!(
        resumed["data"]["current_work"]["focus"]["external_work_item_ref"],
        "https://github.com/DanielCarva1/forge-method-core/issues/34"
    );
    assert_eq!(
        resumed["data"]["current_work"]["focus"]["next_step"],
        "Dogfood the command on Forge itself"
    );
    assert_eq!(
        resumed["data"]["current_work"]["focus"]["selected_practice_ref"],
        "edge-case-review"
    );

    let supersede_input = consumer.write_json(
        "current-work supersede.json",
        &serde_json::json!({
            "schema_version": "work_focus_update_input_v1",
            "expected_snapshot_digest": accepted["data"]["snapshot_digest"],
            "expected_ledger_head_digest": accepted["data"]["ledger_head_digest"],
            "expected_state_version": accepted["data"]["state_version"],
            "expected_work_focus": {
                "status": "current",
                "record_digest": accepted["data"]["focus_record"]["record_digest"]
            },
            "change": {
                "kind": "supersede",
                "focus": {
                    "focus_id": "focus.issue-34.slice-2",
                    "title": "Finish Work Focus lifecycle transitions",
                    "intended_outcome": "A replacement agent sees the newly accepted slice",
                    "acceptance_summary": "Supersede and complete are exact conflict-safe transitions",
                    "non_goals": ["bind blockers in this slice"],
                    "canonical_refs": ["contracts/spec/product-journey-guidance-v0.yaml"],
                    "affected_area_refs": ["crates/forge-core-cli"],
                    "external_work_item_ref": "https://github.com/DanielCarva1/forge-method-core/issues/34",
                    "selected_practice_ref": "edge-case-review",
                    "selected_practice_reason": "Stress the replacement-host journey without adding another workflow",
                    "current_activity": "Exercise the supersede transition",
                    "next_step": "Complete this focus"
                }
            },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.current-work-e2e",
                "interaction_ref": "turn.supersede-current-work",
                "conversation_digest": format!("sha256:{}", "c".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let superseded = assert_ok(&run_current_work_update(&consumer, &supersede_input));
    assert_eq!(superseded["command"], "workflow.current_work_update");
    assert_eq!(
        superseded["data"]["current_work"]["focus"]["focus_id"],
        "focus.issue-34.slice-2"
    );

    let superseded_wal = fs::read(&wal).expect("WAL after superseded Work Focus");
    let stale_retry = run_current_work_update(&consumer, &supersede_input);
    assert_eq!(stale_retry.status.code(), Some(4));
    assert_eq!(json(&stale_retry)["exit_reason"], "conflict");
    assert_eq!(
        fs::read(&wal).expect("WAL after rejected stale supersede retry"),
        superseded_wal
    );

    let blocker_records = {
        let mut ledger = lock_workflow_governance_ledger_tcb(&consumer.state)
            .expect("open ledger for canonical blocker fixture");
        (0..5)
            .map(|index| {
                let projection = ledger.recover().expect("recover blocker fixture state");
                let identity = projection
                    .identity()
                    .expect("active blocker fixture identity");
                ledger
                    .append_unchecked_tcb_event(
                        projection
                            .head_digest
                            .as_deref()
                            .expect("blocker fixture head"),
                        &identity,
                        projection
                            .current_state_version()
                            .expect("blocker fixture state version"),
                        WorkflowGovernanceEvent::DecisionNeedRaised(DecisionNeedRaisedEvent {
                            policy_ref: StableId("policy.workflow.domain-scan".to_owned()),
                            decision_ref: StableId(format!(
                                "decision.current-work.blocker-{index}"
                            )),
                            authority_scope: StableId("workflow.decision.resolve".to_owned()),
                            question_digest: format!("sha256:{index:x}{}", "e".repeat(63)),
                        }),
                    )
                    .expect("append canonical blocker fixture")
            })
            .collect::<Vec<_>>()
    };
    let blocker_digests = blocker_records
        .iter()
        .map(|record| record.record_digest.clone())
        .collect::<Vec<_>>();
    let after_blocker = assert_ok(&consumer.run(&["resume"]));
    let wrong_type_input = consumer.write_json(
        "current-work reject wrong reference type.json",
        &serde_json::json!({
            "schema_version": "work_focus_update_input_v1",
            "expected_snapshot_digest": after_blocker["data"]["snapshot_digest"],
            "expected_ledger_head_digest": after_blocker["data"]["ledger_head_digest"],
            "expected_state_version": after_blocker["data"]["state_version"],
            "expected_work_focus": {
                "status": "current",
                "record_digest": superseded["data"]["focus_record"]["record_digest"]
            },
            "change": {
                "kind": "bind_references",
                "blocker_record_digests": [],
                "evidence_record_digests": [blocker_digests[0]]
            },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.current-work-e2e",
                "interaction_ref": "turn.reject-wrong-current-work-binding",
                "conversation_digest": format!("sha256:{}", "9".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let before_wrong_type = fs::read(&wal).expect("WAL before wrong binding type");
    let wrong_type = run_current_work_update(&consumer, &wrong_type_input);
    assert!(!wrong_type.status.success());
    assert_eq!(
        fs::read(&wal).expect("WAL after wrong binding type"),
        before_wrong_type,
        "a record of the wrong canonical kind must not be written as evidence"
    );
    let bind_input = consumer.write_json(
        "current-work bind references.json",
        &serde_json::json!({
            "schema_version": "work_focus_update_input_v1",
            "expected_snapshot_digest": after_blocker["data"]["snapshot_digest"],
            "expected_ledger_head_digest": after_blocker["data"]["ledger_head_digest"],
            "expected_state_version": after_blocker["data"]["state_version"],
            "expected_work_focus": {
                "status": "current",
                "record_digest": superseded["data"]["focus_record"]["record_digest"]
            },
            "change": {
                "kind": "bind_references",
                "blocker_record_digests": blocker_digests,
                "evidence_record_digests": []
            },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.current-work-e2e",
                "interaction_ref": "turn.bind-current-work",
                "conversation_digest": format!("sha256:{}", "f".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let bound = assert_ok(&run_current_work_update(&consumer, &bind_input));
    assert_eq!(bound["data"]["current_work"]["status"], "blocked");
    assert_eq!(bound["data"]["current_work"]["focus"]["blocker_count"], 5);
    assert_eq!(
        bound["data"]["current_work"]["focus"]["blocker_refs"]
            .as_array()
            .expect("bounded blocker summary")
            .len(),
        MAX_CURRENT_WORK_SUMMARY_REFERENCE_ITEMS
    );
    let detail = assert_ok(&run_current_work_detail(
        &consumer,
        bound["data"]["ledger_head_digest"]
            .as_str()
            .expect("bound ledger head"),
    ));
    assert_eq!(
        detail["data"]["blocker_refs"],
        serde_json::json!(blocker_digests)
    );
    assert!(
        serde_json::to_vec(&detail["data"])
            .expect("serialize Current Work detail")
            .len()
            <= MAX_CURRENT_WORK_DETAIL_BYTES
    );
    assert_eq!(
        detail["data"]["focus"]["selected_practice_reason"],
        "Stress the replacement-host journey without adding another workflow"
    );
    let state_before_stale_detail = state_tree_snapshot(&consumer.state);
    let stale_detail = run_current_work_detail(
        &consumer,
        after_blocker["data"]["ledger_head_digest"]
            .as_str()
            .expect("pre-binding ledger head"),
    );
    assert!(!stale_detail.status.success());
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        state_before_stale_detail
    );

    let complete_input = consumer.write_json(
        "current-work complete.json",
        &serde_json::json!({
            "schema_version": "work_focus_update_input_v1",
            "expected_snapshot_digest": bound["data"]["snapshot_digest"],
            "expected_ledger_head_digest": bound["data"]["ledger_head_digest"],
            "expected_state_version": bound["data"]["state_version"],
            "expected_work_focus": {
                "status": "current",
                "record_digest": bound["data"]["focus_record"]["record_digest"]
            },
            "change": {
                "kind": "complete",
                "completion_summary": "The focused lifecycle transition proof passed",
                "next_step": "Bind blockers and evidence explicitly"
            },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.current-work-e2e",
                "interaction_ref": "turn.complete-current-work",
                "conversation_digest": format!("sha256:{}", "d".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let completed = assert_ok(&run_current_work_update(&consumer, &complete_input));
    assert_eq!(completed["data"]["current_work"]["status"], "completed");

    let resumed = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(resumed["data"]["current_work"]["status"], "completed");
    assert_eq!(
        resumed["data"]["current_work"]["focus"]["current_activity"],
        "The focused lifecycle transition proof passed"
    );
    assert_eq!(
        resumed["data"]["current_work"]["focus"]["next_step"],
        "Bind blockers and evidence explicitly"
    );

    let next_focus_input = consumer.write_json(
        "current-work accept after completion.json",
        &serde_json::json!({
            "schema_version": "work_focus_accept_input_v1",
            "expected_snapshot_digest": completed["data"]["snapshot_digest"],
            "expected_ledger_head_digest": completed["data"]["ledger_head_digest"],
            "expected_state_version": completed["data"]["state_version"],
            "expected_work_focus": { "status": "absent" },
            "focus": {
                "focus_id": "focus.issue-35.replacement-dogfood",
                "title": "Prove the next replacement-host journey",
                "intended_outcome": "A completed focus does not prevent the next bounded slice",
                "acceptance_summary": "The next focus becomes current and links to the terminal predecessor",
                "non_goals": ["reopen the completed focus"],
                "canonical_refs": ["contracts/spec/product-journey-guidance-v0.yaml"],
                "affected_area_refs": ["crates/forge-core-cli"],
                "external_work_item_ref": "https://github.com/DanielCarva1/forge-method-core/issues/35",
                "selected_practice_ref": "edge-case-review",
                "selected_practice_reason": "Exercise the boundary after a completed slice",
                "current_activity": "Start the next focused journey",
                "next_step": "Run replacement-host dogfood"
            },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.current-work-e2e",
                "interaction_ref": "turn.accept-next-current-work",
                "conversation_digest": format!("sha256:{}", "8".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let next_focus = assert_ok(&run_current_work_accept(&consumer, &next_focus_input));
    assert_eq!(next_focus["data"]["current_work"]["status"], "current");
    assert_eq!(
        next_focus["data"]["current_work"]["focus"]["focus_id"],
        "focus.issue-35.replacement-dogfood"
    );
}

#[test]
fn quick_cycle_accept_and_complete_are_two_atomic_current_work_writes() {
    let consumer = Consumer::new_with_prefix("forge-quick-cycle-persistence-e2e");
    assert_ok(&consumer.run(&["init"]));
    let next = assert_ok(&consumer.run(&["next"]));
    let packet_digest = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("cooperative packet digest");
    let objective_input = consumer.write_json(
        "quick-cycle objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Persist one proportional dogfooding cycle without extra stores",
                "constraints": ["write only accepted current-work transitions"],
                "unacceptable_outcomes": ["leave partial state"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.quick-cycle-e2e",
                "interaction_ref": "turn.accept-objective",
                "conversation_digest": format!("sha256:{}", "1".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    assert_ok(&run_cooperative_input(
        &consumer,
        packet_digest,
        &objective_input,
    ));
    let before = assert_ok(&consumer.run(&["resume"]));
    let wal = consumer.state.join("wal/workflow-governance.ndjson");
    let records_before_accept = work_focus_record_count(&consumer.state);

    let accept_input = consumer.write_json(
        "quick-cycle accept.json",
        &serde_json::json!({
            "schema_version": "work_focus_accept_input_v2",
            "expected_snapshot_digest": before["data"]["snapshot_digest"],
            "expected_ledger_head_digest": before["data"]["ledger_head_digest"],
            "expected_state_version": before["data"]["state_version"],
            "expected_work_focus": { "status": "absent" },
            "focus": {
                "focus_id": "focus.quick-cycle.persistence-e2e",
                "title": "Persist a compact Quick Cycle",
                "intended_outcome": "One accepted focus owns the proportional lifecycle summary",
                "acceptance_summary": "Acceptance and completion each append one atomic record",
                "affected_area_refs": ["crates/forge-core-kernel"],
                "current_activity": "Accept the compact cycle",
                "next_step": "Complete all five lifecycle summaries"
            },
            "continuity": {
                "quick_cycle": {
                    "compactness_reason": "The behavior is bounded to one existing Current Work path",
                    "stage_closeouts": {},
                    "expansion_history": []
                }
            },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.quick-cycle-e2e",
                "interaction_ref": "turn.accept-cycle",
                "conversation_digest": format!("sha256:{}", "2".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let accepted = assert_ok(&run_current_work_accept(&consumer, &accept_input));
    assert_eq!(
        work_focus_record_count(&consumer.state),
        records_before_accept + 1
    );
    assert_eq!(
        accepted["data"]["focus_record"]["event"]["payload"]["quick_cycle"]["compactness_reason"],
        "The behavior is bounded to one existing Current Work path"
    );

    let blocker_record = {
        let mut ledger = lock_workflow_governance_ledger_tcb(&consumer.state)
            .expect("open ledger for final Quick Cycle reference");
        let projection = ledger.recover().expect("recover final reference state");
        let identity = projection.identity().expect("active ledger identity");
        ledger
            .append_unchecked_tcb_event(
                projection
                    .head_digest
                    .as_deref()
                    .expect("final reference head"),
                &identity,
                projection
                    .current_state_version()
                    .expect("final reference state version"),
                WorkflowGovernanceEvent::DecisionNeedRaised(DecisionNeedRaisedEvent {
                    policy_ref: StableId("policy.workflow.domain-scan".to_owned()),
                    decision_ref: StableId("decision.quick-cycle.final-reference".to_owned()),
                    authority_scope: StableId("workflow.decision.resolve".to_owned()),
                    question_digest: format!("sha256:{}", "4".repeat(64)),
                }),
            )
            .expect("append final Quick Cycle reference")
    };
    let after_reference = assert_ok(&consumer.run(&["resume"]));
    let before_consultation_key =
        &before["data"]["journey_guidance"]["catalog"]["consultation"]["key"];
    let focused_consultation_key =
        &after_reference["data"]["journey_guidance"]["catalog"]["consultation"]["key"];
    assert_ne!(before_consultation_key, focused_consultation_key);
    let blocker_digest = blocker_record.record_digest;
    let completion = |validation_summary: Option<&str>, interaction_ref: &str| {
        consumer.write_json(
            &format!("quick-cycle {interaction_ref}.json"),
            &serde_json::json!({
                "schema_version": "work_focus_update_input_v2",
                "expected_snapshot_digest": after_reference["data"]["snapshot_digest"],
                "expected_ledger_head_digest": after_reference["data"]["ledger_head_digest"],
                "expected_state_version": after_reference["data"]["state_version"],
                "expected_work_focus": {
                    "status": "current",
                    "record_digest": accepted["data"]["focus_record"]["record_digest"]
                },
                "change": {
                    "kind": "complete",
                    "completion_summary": "The proportional lifecycle was completed atomically",
                    "next_step": "Read the compact cycle through Current Work",
                    "continuity": {
                        "blocker_record_digests": [blocker_digest],
                        "quick_cycle": {
                            "compactness_reason": "The behavior is bounded to one existing Current Work path",
                            "stage_closeouts": {
                                "analysis_discovery": { "summary": "The user need and existing path were checked" },
                                "product_planning": { "summary": "The bounded acceptance rules were agreed" },
                                "solution_definition": { "summary": "Current Work remains the single owner" },
                                "implementation": { "summary": "The v2 input uses the existing atomic WAL" },
                                "validation_delivery": validation_summary.map(|summary| serde_json::json!({ "summary": summary }))
                            },
                            "expansion_history": []
                        }
                    }
                },
                "recorded_by": "principal.agent.cli-e2e",
                "host_provenance": {
                    "host_id": "host.cli-e2e",
                    "host_version": "test",
                    "session_ref": "session.quick-cycle-e2e",
                    "interaction_ref": interaction_ref,
                    "conversation_digest": format!("sha256:{}", "3".repeat(64)),
                    "observed_at_unix": 1
                }
            }),
        )
    };
    let incomplete_input = completion(None, "turn.reject-incomplete-cycle");
    let wal_after_accept = fs::read(&wal).expect("WAL before incomplete completion");
    let incomplete = run_current_work_update(&consumer, &incomplete_input);
    assert!(!incomplete.status.success());
    assert_eq!(
        fs::read(&wal).expect("WAL after incomplete completion"),
        wal_after_accept,
        "an incomplete Quick Cycle must not leave a partial record"
    );

    let complete_input = completion(
        Some("The focused contract, transition, and CLI journey passed"),
        "turn.complete-cycle",
    );
    let completed = assert_ok(&run_current_work_update(&consumer, &complete_input));
    assert_eq!(completed["data"]["current_work"]["status"], "completed");
    assert_eq!(
        completed["data"]["focus_record"]["event"]["payload"]["quick_cycle"]["stage_closeouts"]
            ["validation_delivery"]["summary"],
        "The focused contract, transition, and CLI journey passed"
    );
    assert_eq!(
        completed["data"]["focus_record"]["event"]["payload"]["blocker_record_digests"],
        serde_json::json!([blocker_digest])
    );
    assert_eq!(
        work_focus_record_count(&consumer.state),
        records_before_accept + 2,
        "normal Quick Cycle persistence is exactly accept plus complete"
    );

    let state_before_completed_resume = state_tree_snapshot(&consumer.state);
    let completed_resume = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(
        completed_resume["data"]["schema_version"],
        "workflow_resume_summary_v10"
    );
    assert_eq!(
        completed_resume["data"]["current_work"]["focus"]["quick_cycle"],
        serde_json::json!({
            "state": "completed",
            "stage_closeout_count": 5,
            "expansion_count": 0
        })
    );
    assert_eq!(
        completed_resume["data"]["journey_guidance"]["catalog"]["consultation"]["key"],
        *focused_consultation_key,
        "routine progress on the same Work Focus must not retrigger the catalog"
    );
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        state_before_completed_resume,
        "ordinary resume must derive Quick Cycle state without writing"
    );

    let successor_input = consumer.write_json(
        "quick-cycle successor.json",
        &serde_json::json!({
            "schema_version": "work_focus_accept_input_v2",
            "expected_snapshot_digest": completed["data"]["snapshot_digest"],
            "expected_ledger_head_digest": completed["data"]["ledger_head_digest"],
            "expected_state_version": completed["data"]["state_version"],
            "expected_work_focus": { "status": "absent" },
            "focus": {
                "focus_id": "focus.quick-cycle.readback-successor",
                "title": "Read the completed predecessor progressively",
                "intended_outcome": "Resume stays compact and detail opens one exact predecessor",
                "acceptance_summary": "The public read path does not list or rewrite history",
                "current_activity": "Inspect the bounded resume summary",
                "next_step": "Open predecessor detail only if needed"
            },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.quick-cycle-e2e",
                "interaction_ref": "turn.accept-readback-successor",
                "conversation_digest": format!("sha256:{}", "5".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let successor = assert_ok(&run_current_work_accept(&consumer, &successor_input));
    let state_before_readback = state_tree_snapshot(&consumer.state);
    let resumed = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(
        resumed["data"]["schema_version"],
        "workflow_resume_summary_v10"
    );
    assert_eq!(
        resumed["data"]["current_work"]["schema_version"],
        "current_work_context_v3"
    );
    assert!(resumed["data"]["current_work"]["focus"]
        .get("quick_cycle")
        .is_none());
    assert_ne!(
        resumed["data"]["journey_guidance"]["catalog"]["consultation"]["key"],
        *focused_consultation_key,
        "a successor Work Focus must publish a new catalog consultation key"
    );
    let current_detail = assert_ok(&run_current_work_detail_record(
        &consumer,
        successor["data"]["ledger_head_digest"]
            .as_str()
            .expect("successor head"),
        successor["data"]["focus_record"]["record_digest"]
            .as_str()
            .expect("successor record"),
    ));
    assert_eq!(
        current_detail["data"]["schema_version"],
        "current_work_detail_v3"
    );
    let predecessor_argv = current_detail["data"]["predecessor_detail_argv"]
        .as_array()
        .expect("published predecessor argv");
    assert_eq!(predecessor_argv[8], "--record-digest");
    assert_eq!(
        predecessor_argv[9],
        completed["data"]["focus_record"]["record_digest"]
    );
    let predecessor_detail = assert_ok(&run_current_work_detail_record(
        &consumer,
        successor["data"]["ledger_head_digest"]
            .as_str()
            .expect("successor head"),
        completed["data"]["focus_record"]["record_digest"]
            .as_str()
            .expect("completed predecessor record"),
    ));
    assert_eq!(
        predecessor_detail["data"]["focus"]["quick_cycle"]["stage_closeouts"]
            ["validation_delivery"]["summary"],
        "The focused contract, transition, and CLI journey passed"
    );
    assert!(predecessor_detail["data"]
        .get("predecessor_detail_argv")
        .is_none());
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        state_before_readback,
        "resume and both detail reads must not write state"
    );
}

#[test]
fn collaboration_v3_replaces_the_complete_plan_with_one_write_per_change() {
    let consumer = Consumer::new_with_prefix("forge-collaboration-persistence-e2e");
    assert_ok(&consumer.run(&["init"]));
    let next = assert_ok(&consumer.run(&["next"]));
    let packet_digest = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("cooperative packet digest");
    let objective_input = consumer.write_json(
        "collaboration objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Coordinate two bounded Forge lanes without another state store",
                "constraints": ["persist only material plan changes"],
                "unacceptable_outcomes": ["copy claim or isolation state"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.collaboration-e2e",
                "interaction_ref": "turn.accept-objective",
                "conversation_digest": format!("sha256:{}", "1".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    assert_ok(&run_cooperative_input(
        &consumer,
        packet_digest,
        &objective_input,
    ));
    let before = assert_ok(&consumer.run(&["resume"]));
    let initial_plan = serde_json::json!({
        "lanes": [
            {
                "lane_id": "lane.contract",
                "outcome": "Define the bounded collaboration contract",
                "isolation_id": "isolation.contract"
            },
            {
                "lane_id": "lane.persistence",
                "outcome": "Persist the complete collaboration plan",
                "depends_on": ["lane.contract"]
            }
        ]
    });
    let provenance = |interaction_ref: &str, digest_digit: &str| {
        serde_json::json!({
            "host_id": "host.cli-e2e",
            "host_version": "test",
            "session_ref": "session.collaboration-e2e",
            "interaction_ref": interaction_ref,
            "conversation_digest": format!("sha256:{}", digest_digit.repeat(64)),
            "observed_at_unix": 1
        })
    };
    let records_before = work_focus_record_count(&consumer.state);
    let accept_input = consumer.write_json(
        "collaboration accept.json",
        &serde_json::json!({
            "schema_version": "work_focus_accept_input_v3",
            "expected_snapshot_digest": before["data"]["snapshot_digest"],
            "expected_ledger_head_digest": before["data"]["ledger_head_digest"],
            "expected_state_version": before["data"]["state_version"],
            "expected_work_focus": { "status": "absent" },
            "focus": {
                "focus_id": "focus.collaboration.persistence-e2e",
                "title": "Persist a bounded collaboration plan",
                "intended_outcome": "One Work Focus coordinates independent lanes",
                "acceptance_summary": "Each material plan change is one atomic write",
                "affected_area_refs": ["crates/forge-core-kernel"],
                "current_activity": "Accept the initial lane plan",
                "next_step": "Assign the persistence isolation"
            },
            "continuity": { "collaboration": initial_plan },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": provenance("turn.accept-collaboration", "2")
        }),
    );
    assert!(
        fs::metadata(&accept_input)
            .expect("accept input metadata")
            .len()
            <= MAX_WORK_FOCUS_ACCEPT_INPUT_BYTES
    );
    let accepted = assert_ok(&run_current_work_accept(&consumer, &accept_input));
    assert_eq!(work_focus_record_count(&consumer.state), records_before + 1);
    assert_eq!(
        accepted["data"]["focus_record"]["event"]["payload"]["collaboration"],
        initial_plan
    );
    let state_after_accept = state_tree_snapshot(&consumer.state);
    let resumed = assert_ok(&consumer.run(&["resume"]));
    let current_work = &resumed["data"]["current_work"];
    assert_eq!(current_work["schema_version"], "current_work_context_v3");
    assert_eq!(current_work["focus"]["collaboration"]["lane_count"], 2);
    assert_eq!(
        current_work["focus"]["collaboration"]["ready_lane_count"],
        1
    );
    assert_eq!(
        current_work["focus"]["collaboration"]["blocked_lane_count"],
        1
    );
    assert_eq!(
        current_work["focus"]["collaboration"]["next_ready_lane"]["lane_id"],
        "lane.contract"
    );
    assert!(
        serde_json::to_vec(current_work)
            .expect("serialize Current Work")
            .len()
            <= MAX_CURRENT_WORK_SUMMARY_BYTES
    );
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        state_after_accept,
        "collaboration resume readback must not write state"
    );
    let detail = assert_ok(&run_current_work_detail_record(
        &consumer,
        accepted["data"]["ledger_head_digest"]
            .as_str()
            .expect("accepted ledger head"),
        accepted["data"]["focus_record"]["record_digest"]
            .as_str()
            .expect("accepted focus record"),
    ));
    assert_eq!(detail["data"]["schema_version"], "current_work_detail_v3");
    assert_eq!(
        detail["data"]["focus"]["collaboration"]["plan"],
        initial_plan
    );
    assert_eq!(
        detail["data"]["focus"]["collaboration"]["lanes"][0]["lane_id"],
        "lane.contract"
    );
    assert_eq!(
        detail["data"]["focus"]["collaboration"]["lanes"][0]["state"],
        "ready"
    );
    assert_eq!(
        detail["data"]["focus"]["collaboration"]["lanes"][1]["lane_id"],
        "lane.persistence"
    );
    assert_eq!(
        detail["data"]["focus"]["collaboration"]["lanes"][1]["state"],
        "blocked"
    );
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        state_after_accept,
        "collaboration detail readback must not write state"
    );

    let assigned_plan = serde_json::json!({
        "lanes": [
            {
                "lane_id": "lane.contract",
                "outcome": "Define the bounded collaboration contract",
                "isolation_id": "isolation.contract"
            },
            {
                "lane_id": "lane.persistence",
                "outcome": "Persist the complete collaboration plan",
                "depends_on": ["lane.contract"],
                "isolation_id": "isolation.persistence"
            }
        ]
    });
    let v2_reject_input = consumer.write_json(
        "collaboration v2 reject.json",
        &serde_json::json!({
            "schema_version": "work_focus_update_input_v2",
            "expected_snapshot_digest": accepted["data"]["snapshot_digest"],
            "expected_ledger_head_digest": accepted["data"]["ledger_head_digest"],
            "expected_state_version": accepted["data"]["state_version"],
            "expected_work_focus": {
                "status": "current",
                "record_digest": accepted["data"]["focus_record"]["record_digest"]
            },
            "change": {
                "kind": "checkpoint_collaboration",
                "current_activity": "Attempt a version-confused update",
                "next_step": "Reject before append",
                "continuity": { "collaboration": assigned_plan }
            },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": provenance("turn.reject-v2-collaboration", "3")
        }),
    );
    let wal = consumer.state.join("wal/workflow-governance.ndjson");
    let wal_after_accept = fs::read(&wal).expect("WAL after accept");
    assert!(!run_current_work_update(&consumer, &v2_reject_input)
        .status
        .success());
    assert_eq!(
        fs::read(&wal).expect("WAL after v2 reject"),
        wal_after_accept
    );

    let checkpoint_input = consumer.write_json(
        "collaboration checkpoint.json",
        &serde_json::json!({
            "schema_version": "work_focus_update_input_v3",
            "expected_snapshot_digest": accepted["data"]["snapshot_digest"],
            "expected_ledger_head_digest": accepted["data"]["ledger_head_digest"],
            "expected_state_version": accepted["data"]["state_version"],
            "expected_work_focus": {
                "status": "current",
                "record_digest": accepted["data"]["focus_record"]["record_digest"]
            },
            "change": {
                "kind": "checkpoint_collaboration",
                "current_activity": "Assign the persistence isolation",
                "next_step": "Supersede with the integration focus",
                "continuity": { "collaboration": assigned_plan }
            },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": provenance("turn.checkpoint-collaboration", "4")
        }),
    );
    assert!(
        fs::metadata(&checkpoint_input)
            .expect("checkpoint input metadata")
            .len()
            <= MAX_WORK_FOCUS_UPDATE_INPUT_BYTES
    );
    let checkpointed = assert_ok(&run_current_work_update(&consumer, &checkpoint_input));
    assert_eq!(work_focus_record_count(&consumer.state), records_before + 2);
    assert_eq!(
        checkpointed["data"]["focus_record"]["event"]["payload"]["collaboration"],
        assigned_plan
    );
    let wal_after_checkpoint = fs::read(&wal).expect("WAL after checkpoint");
    assert!(!run_current_work_update(&consumer, &checkpoint_input)
        .status
        .success());
    assert_eq!(
        fs::read(&wal).expect("WAL after stale retry"),
        wal_after_checkpoint
    );

    let integration_plan = serde_json::json!({
        "lanes": [{
            "lane_id": "lane.integration",
            "outcome": "Integrate the completed contract and persistence lanes",
            "isolation_id": "isolation.integration"
        }]
    });
    let supersede_input = consumer.write_json(
        "collaboration supersede.json",
        &serde_json::json!({
            "schema_version": "work_focus_update_input_v3",
            "expected_snapshot_digest": checkpointed["data"]["snapshot_digest"],
            "expected_ledger_head_digest": checkpointed["data"]["ledger_head_digest"],
            "expected_state_version": checkpointed["data"]["state_version"],
            "expected_work_focus": {
                "status": "current",
                "record_digest": checkpointed["data"]["focus_record"]["record_digest"]
            },
            "change": {
                "kind": "supersede",
                "focus": {
                    "focus_id": "focus.collaboration.integration-e2e",
                    "title": "Integrate the collaboration lanes",
                    "intended_outcome": "One bounded integration focus follows the lane work",
                    "acceptance_summary": "The successor carries only its own stable lane plan",
                    "current_activity": "Integrate the lane outcomes",
                    "next_step": "Complete the integration focus"
                },
                "continuity": { "collaboration": integration_plan }
            },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": provenance("turn.supersede-collaboration", "5")
        }),
    );
    let superseded = assert_ok(&run_current_work_update(&consumer, &supersede_input));
    assert_eq!(work_focus_record_count(&consumer.state), records_before + 3);
    assert_eq!(
        superseded["data"]["focus_record"]["event"]["payload"]["collaboration"],
        integration_plan
    );

    let complete_input = consumer.write_json(
        "collaboration complete.json",
        &serde_json::json!({
            "schema_version": "work_focus_update_input_v3",
            "expected_snapshot_digest": superseded["data"]["snapshot_digest"],
            "expected_ledger_head_digest": superseded["data"]["ledger_head_digest"],
            "expected_state_version": superseded["data"]["state_version"],
            "expected_work_focus": {
                "status": "current",
                "record_digest": superseded["data"]["focus_record"]["record_digest"]
            },
            "change": {
                "kind": "complete",
                "completion_summary": "The collaboration plan was integrated atomically",
                "next_step": "Expose progressive readback",
                "continuity": { "collaboration": integration_plan }
            },
            "recorded_by": "principal.agent.cli-e2e",
            "host_provenance": provenance("turn.complete-collaboration", "6")
        }),
    );
    let completed = assert_ok(&run_current_work_update(&consumer, &complete_input));
    assert_eq!(completed["data"]["current_work"]["status"], "completed");
    assert_eq!(work_focus_record_count(&consumer.state), records_before + 4);
    assert_eq!(
        completed["data"]["focus_record"]["event"]["payload"]["collaboration"],
        integration_plan
    );
}

#[test]
fn cooperative_objective_grounding_survives_restart_and_local_writes() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));
    let next = assert_ok(&consumer.run(&["next"]));
    let packet_digest = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("cooperative objective packet")
        .to_owned();
    let objective = consumer.write_json(
        "cooperative grounding objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Keep accepted solo intent stable while the agent updates local diagnostics",
                "constraints": ["remain host neutral"],
                "unacceptable_outcomes": ["claim verified human origin"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.cli-e2e",
                "interaction_ref": "turn.grounding",
                "conversation_digest": format!("sha256:{}", "d".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let accepted = assert_ok(&run_cooperative_input(
        &consumer,
        &packet_digest,
        &objective,
    ));
    let objective_record_digest = accepted["data"]["objective_record"]["record_digest"]
        .as_str()
        .expect("accepted objective record digest")
        .to_owned();

    fs::create_dir_all(consumer.app.join(".local")).expect("create local-only state directory");
    fs::write(
        consumer.app.join(".local/resume.json"),
        "first local-only resume report\n",
    )
    .expect("write local-only resume report");

    let grounded = assert_ok(&consumer.run(&["next"]));
    assert_eq!(grounded["data"]["status"], "ready_to_complete");
    assert!(grounded["data"]["cooperative_evidence_action_packet"].is_null());
    assert!(grounded["data"]["cooperative_evidence_action_gap"].is_null());
    assert_eq!(
        grounded["data"]["simulation"]["candidate_claim_results"][0]["accepted_grounding_refs"],
        serde_json::json!([format!("cooperative-objective:{objective_record_digest}")])
    );
    let supporting_snapshot = grounded["data"]["snapshot_digest"].clone();

    let restarted = assert_ok(&consumer.run(&["report"]));
    assert_eq!(restarted["data"]["status"], "ready_to_complete");
    assert!(restarted["data"]["cooperative_evidence_action_packet"].is_null());

    fs::write(
        consumer.app.join(".local/resume.json"),
        "updated local-only resume report\n",
    )
    .expect("update pre-existing local-only resume report");
    let local_updated = assert_ok(&consumer.run(&["report"]));
    assert_eq!(
        local_updated["data"]["snapshot_digest"],
        supporting_snapshot
    );
    assert_eq!(local_updated["data"]["status"], "ready_to_complete");
    assert!(local_updated["data"]["cooperative_evidence_action_packet"].is_null());

    fs::write(
        consumer.app.join("README.md"),
        "consumer project snapshot changed\n",
    )
    .expect("change governed project snapshot");
    let changed = assert_ok(&consumer.run(&["next"]));
    assert_ne!(changed["data"]["snapshot_digest"], supporting_snapshot);
    assert_eq!(changed["data"]["status"], "ready_to_complete");
    assert_eq!(
        changed["data"]["simulation"]["candidate_claim_results"][0]["accepted_grounding_refs"],
        grounded["data"]["simulation"]["candidate_claim_results"][0]["accepted_grounding_refs"],
        "project edits must not erase the still-active accepted objective"
    );
    assert!(changed["data"]["cooperative_evidence_action_packet"].is_null());
}

#[test]
#[allow(clippy::too_many_lines)]
fn solo_applicability_assessment_is_public_honest_and_basis_scoped() {
    let strict = Consumer::new_with_prefix("forge-workflow-strict-applicability-e2e");
    assert_ok(&strict.run(&["init", "--readiness-profile", "strict_external"]));
    assert!(
        assert_ok(&strict.run(&["next"]))["data"]["cooperative_evidence_action_packet"].is_null()
    );

    let consumer = Consumer::new_with_prefix("forge-workflow-solo-applicability-e2e");
    assert_ok(&consumer.run(&["init"]));
    upgrade_to_latest(&consumer);
    let objective_next = assert_ok(&consumer.run(&["next"]));
    let packet_digest = objective_next["data"]["authorization"]["action_packets"][0]
        ["packet_digest"]
        .as_str()
        .expect("cooperative objective packet")
        .to_owned();
    let objective = consumer.write_json(
        "applicability objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Assess policy applicability honestly from repository evidence",
                "constraints": ["same-owner agent only"],
                "unacceptable_outcomes": ["claim human or independent authority"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.applicability-e2e",
            "host_provenance": {
                "host_id": "host.applicability-e2e",
                "host_version": "test",
                "session_ref": "session.applicability-e2e",
                "interaction_ref": "turn.applicability-e2e",
                "conversation_digest": format!("sha256:{}", "a".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    assert_ok(&run_cooperative_input(
        &consumer,
        &packet_digest,
        &objective,
    ));

    let original_policy = "policy.workflow.investigation";
    let mut next = advance_fixture_to_policy(&consumer, original_policy);
    assert_eq!(next["data"]["status"], "applicability_required");
    let first_packet = next["data"]["cooperative_evidence_action_packet"].clone();
    assert_eq!(first_packet["route"]["target"], "policy_applicability");
    let argv = first_packet["argv"].as_array().expect("published argv");
    let root_index = argv
        .iter()
        .position(|value| value == "--root")
        .expect("explicit root flag");
    let published_root = PathBuf::from(
        argv[root_index + 1]
            .as_str()
            .expect("published project root"),
    );
    assert!(published_root.is_absolute());
    assert_eq!(published_root, consumer.app);
    assert!(next["data"]["authorization"]["action_packets"]
        .as_array()
        .is_some_and(Vec::is_empty));

    let malformed = consumer.parent.join("malformed applicability.json");
    fs::write(&malformed, b"{}").expect("malformed offer");
    let rejected = execute_cooperative_packet(&first_packet, &malformed);
    assert_eq!(rejected.status.code(), Some(2));
    assert_eq!(
        json(&rejected)["data"]["event"]["payload"]["rejection"],
        "malformed_or_oversized_offer"
    );

    let mut stale_offer = first_packet["offer_template"].clone();
    stale_offer["offer_id"] = serde_json::json!("offer.applicability.stale");
    stale_offer["attestation"]["applicability_assessment"] = serde_json::json!({
        "outcome": "inconclusive",
        "summary": "The available basis is not sufficient",
        "basis_paths": ["README.md"],
        "limitations": ["same-owner inspection"]
    });
    let stale_input = consumer.write_json("stale applicability.json", &stale_offer);
    let stale = execute_cooperative_packet(&first_packet, &stale_input);
    assert_eq!(stale.status.code(), Some(2));
    assert_eq!(
        json(&stale)["data"]["event"]["payload"]["rejection"],
        "binding_stale"
    );

    next = assert_ok(&consumer.run(&["resume"]));
    let inconclusive_packet = next["data"]["actions"]["cooperative_evidence_packet"].clone();
    let mut inconclusive_offer = inconclusive_packet["offer_template"].clone();
    inconclusive_offer["offer_id"] = serde_json::json!("offer.applicability.inconclusive");
    inconclusive_offer["attestation"]["applicability_assessment"] = serde_json::json!({
        "outcome": "inconclusive",
        "summary": "Repository evidence is presently inconclusive",
        "basis_paths": ["README.md"],
        "limitations": ["same-owner inspection, not a human judgment"]
    });
    let inconclusive_input =
        consumer.write_json("inconclusive applicability.json", &inconclusive_offer);
    let admitted_inconclusive = assert_ok(&execute_cooperative_packet(
        &inconclusive_packet,
        &inconclusive_input,
    ));
    assert_eq!(
        admitted_inconclusive["data"]["event"]["payload"]["admitted_evidence"]
            ["applicability_assessment"]["outcome"],
        "inconclusive"
    );
    let wal = consumer.state.join("wal/workflow-governance.ndjson");
    let wal_len = fs::metadata(&wal).expect("WAL metadata").len();
    assert_ok(&execute_cooperative_packet(
        &inconclusive_packet,
        &inconclusive_input,
    ));
    assert_eq!(fs::metadata(&wal).expect("WAL metadata").len(), wal_len);

    let mut conflicting_offer = inconclusive_offer.clone();
    conflicting_offer["attestation"]["applicability_assessment"]["outcome"] =
        serde_json::json!("applicable");
    let conflicting_input =
        consumer.write_json("conflicting applicability.json", &conflicting_offer);
    let conflict = execute_cooperative_packet(&inconclusive_packet, &conflicting_input);
    assert_eq!(conflict.status.code(), Some(2));
    assert_eq!(
        json(&conflict)["data"]["event"]["payload"]["rejection"],
        "conflicting_idempotency_key"
    );
    next = assert_ok(&consumer.run(&["next"]));
    assert_eq!(next["data"]["status"], "applicability_required");

    let not_applicable_packet = next["data"]["cooperative_evidence_action_packet"].clone();
    let mut not_applicable_offer = not_applicable_packet["offer_template"].clone();
    not_applicable_offer["offer_id"] = serde_json::json!("offer.applicability.not-applicable");
    not_applicable_offer["attestation"]["applicability_assessment"] = serde_json::json!({
        "outcome": "not_applicable",
        "summary": "The selected policy does not apply to this project",
        "basis_paths": ["README.md"],
        "limitations": ["same-owner inspection"]
    });
    let not_applicable_input = consumer.write_json("not applicable.json", &not_applicable_offer);
    assert_ok(&execute_cooperative_packet(
        &not_applicable_packet,
        &not_applicable_input,
    ));
    next = assert_ok(&consumer.run(&["next"]));
    assert_ne!(next["data"]["selected_policy_ref"], original_policy);

    fs::write(
        consumer.app.join("UNRELATED.md"),
        "not part of the assessment basis\n",
    )
    .expect("non-basis edit");
    next = assert_ok(&consumer.run(&["next"]));
    assert_ne!(next["data"]["selected_policy_ref"], original_policy);

    fs::write(consumer.app.join("README.md"), "changed assessment basis\n").expect("basis drift");
    next = assert_ok(&consumer.run(&["next"]));
    assert_eq!(next["data"]["selected_policy_ref"], original_policy);
    assert_eq!(next["data"]["status"], "applicability_required");

    let applicable_packet = next["data"]["cooperative_evidence_action_packet"].clone();
    let mut applicable_offer = applicable_packet["offer_template"].clone();
    applicable_offer["offer_id"] = serde_json::json!("offer.applicability.applicable");
    applicable_offer["attestation"]["applicability_assessment"] = serde_json::json!({
        "outcome": "applicable",
        "summary": "The selected policy applies to the current project",
        "basis_paths": ["README.md"],
        "limitations": ["same-owner inspection, no independent authority"]
    });
    let applicable_input = consumer.write_json("applicable.json", &applicable_offer);
    assert_ok(&execute_cooperative_packet(
        &applicable_packet,
        &applicable_input,
    ));
    next = assert_ok(&consumer.run(&["next"]));
    assert_eq!(next["data"]["selected_policy_ref"], original_policy);
    assert_eq!(next["data"]["applicability"], true);
    let applicability_audit = next["data"]["cooperative_evidence"]
        .as_array()
        .expect("cooperative evidence audit")
        .iter()
        .find(|audit| audit["admitted_evidence"]["offer_id"] == "offer.applicability.applicable")
        .expect("applicability audit");
    assert!(applicability_audit["supports_cooperative_claim_ref"].is_null());
    assert_eq!(applicability_audit["applicability_outcome"], "applicable");
    assert!(applicability_audit["does_not_prove"]
        .as_array()
        .is_some_and(
            |items| items.iter().any(|item| item == "policy_claim_satisfaction")
                && items.iter().any(|item| item == "capability_satisfaction")
                && items
                    .iter()
                    .any(|item| item == "human_applicability_judgment")
        ));
    assert!(next["data"]["simulation"]["candidate_claim_results"]
        .as_array()
        .is_some_and(|claims| claims.iter().all(|claim| claim["status"] != "verified")));
    assert!(next["data"]["simulation"]["candidate_capability_gaps"]
        .as_array()
        .is_some_and(|gaps| !gaps.is_empty()));

    fs::write(consumer.app.join("README.md"), "consumer project\n")
        .expect("restore old basis bytes");
    let no_old_outcome_revival = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(
        no_old_outcome_revival["data"]["selected_policy_ref"],
        original_policy
    );
    assert_eq!(
        no_old_outcome_revival["data"]["status"],
        "applicability_required"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn internal_fixture_reaches_investigation_then_public_solo_source_command_supersedes_assessments() {
    let strict = Consumer::new_with_prefix("forge-workflow-strict-source-e2e");
    let strict_initialized =
        assert_ok(&strict.run(&["init", "--readiness-profile", "strict_external"]));
    assert_eq!(
        strict_initialized["data"]["readiness_profile"],
        "strict_external"
    );
    let strict_next = assert_ok(&strict.run(&["next"]));
    assert!(strict_next["data"]["cooperative_evidence_action_packet"].is_null());

    let consumer = Consumer::new_with_prefix("forge-workflow-solo-source-e2e");
    let git_init = std::process::Command::new("git")
        .arg("init")
        .arg(&consumer.app)
        .output()
        .expect("initialize real Git metadata for excluded-basis checks");
    assert!(
        git_init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&git_init.stderr)
    );
    let initialized = assert_ok(&consumer.run(&["init"]));
    assert_eq!(initialized["data"]["readiness_profile"], "solo_cooperative");
    upgrade_to_latest(&consumer);

    let objective_next = assert_ok(&consumer.run(&["next"]));
    assert!(objective_next["data"]["authorization"]["setup_gaps"]
        .as_array()
        .is_some_and(Vec::is_empty));
    assert_eq!(
        objective_next["data"]["authorization"]["action_packets"][0]["required_authority"]
            ["approval_boundary"],
        "cooperative_same_owner"
    );
    let packet_digest = objective_next["data"]["authorization"]["action_packets"][0]
        ["packet_digest"]
        .as_str()
        .expect("cooperative objective packet")
        .to_owned();
    let objective = consumer.write_json(
        "source evidence objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Use agent inspection as honest solo evidence",
                "constraints": ["keep external authority optional"],
                "unacceptable_outcomes": ["claim independent review"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.source-e2e",
            "host_provenance": {
                "host_id": "host.source-e2e",
                "host_version": "test",
                "session_ref": "session.source-e2e",
                "interaction_ref": "turn.source-e2e",
                "conversation_digest": format!("sha256:{}", "e".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    assert_ok(&run_cooperative_input(
        &consumer,
        &packet_digest,
        &objective,
    ));

    // The fixture advances prior policies only. Every source assessment below
    // is submitted through the public CLI against the selected investigation policy.
    let mut next = advance_fixture_to_policy(&consumer, "policy.workflow.investigation");
    assert!(
        next["data"]["cooperative_evidence_action_packet"].is_object(),
        "the investigation fixture must expose a live cooperative evidence packet"
    );
    assert_eq!(
        next["data"]["cooperative_evidence_action_packet"]["route"]["target"],
        "policy_applicability"
    );
    let applicability_packet = next["data"]["cooperative_evidence_action_packet"].clone();
    let mut applicability_offer = applicability_packet["offer_template"].clone();
    applicability_offer["offer_id"] = serde_json::json!("offer.source-e2e.applicability");
    applicability_offer["attestation"]["applicability_assessment"] = serde_json::json!({
        "outcome": "applicable",
        "summary": "The repository investigation policy applies to this fixture",
        "basis_paths": ["README.md"],
        "limitations": ["same-owner agent applicability assessment"]
    });
    let applicability_input =
        consumer.write_json("source applicability.json", &applicability_offer);
    assert_ok(&execute_cooperative_packet(
        &applicability_packet,
        &applicability_input,
    ));
    next = assert_ok(&consumer.run(&["next"]));
    assert_eq!(
        next["data"]["cooperative_evidence_action_packet"]["route"]["target"],
        "source_claim"
    );
    let activation = assert_ok(&consumer.run(&["resume"]));
    let investigation_policy = activation["data"]["selected_policy_ref"].clone();
    let investigation_journey = activation["data"]["journey_guidance"].clone();
    assert_eq!(
        activation["data"]["agent_autonomy"], next["data"]["agent_autonomy"],
        "resume v2 must retain binding and input contract after objective acceptance"
    );
    assert!(activation["data"]["agent_autonomy"]["binding"].is_object());
    assert!(activation["data"]["agent_autonomy"]["input_contract"].is_object());
    assert_eq!(
        activation["data"]["actions"]["cooperative_evidence_packet"],
        next["data"]["cooperative_evidence_action_packet"],
        "resume v2 must retain the complete current cooperative packet"
    );
    assert_eq!(
        activation["data"]["actions"]["cooperative_evidence_gap"],
        next["data"]["cooperative_evidence_action_gap"],
        "resume v2 must retain the current cooperative gap alternative"
    );
    assert_eq!(
        activation["data"]["boundary_rechecks"], next["data"]["boundary_rechecks"],
        "resume v2 must retain the complete boundary-recheck projection without filtering"
    );
    let claims = next["data"]["simulation"]["candidate_claim_results"]
        .as_array()
        .expect("investigation claims");
    let claim_count = claims.len();
    assert_eq!(claim_count, 4);
    let mut claim_ids = claims
        .iter()
        .map(|claim| claim["claim_id"].as_str().expect("investigation claim id"))
        .collect::<Vec<_>>();
    claim_ids.sort_unstable();
    assert_eq!(
        claim_ids,
        vec![
            "claim.workflow.investigation.conclusion-calibrated",
            "claim.workflow.investigation.hypotheses-probed",
            "claim.workflow.investigation.next-action-safe",
            "claim.workflow.investigation.symptom-bounded",
        ]
    );

    for (relative, contents) in [
        (".git/HEAD", "ref: refs/heads/main\n"),
        (".forge-method/state.json", "{}\n"),
        (".local/journal.md", "local-only\n"),
        ("target/output.txt", "generated\n"),
        ("node_modules/pkg/index.js", "module.exports = {};\n"),
    ] {
        let path = consumer.app.join(relative);
        fs::create_dir_all(path.parent().expect("excluded basis parent"))
            .expect("create excluded basis parent");
        fs::write(path, contents).expect("write excluded basis fixture");
    }
    next = assert_ok(&consumer.run(&["next"]));

    for (suffix, basis_path) in [
        ("escape", "../README.md"),
        ("missing", "docs/does-not-exist.md"),
        ("git", ".git/HEAD"),
        ("state", ".forge-method/state.json"),
        ("local", ".local/journal.md"),
        ("target", "target/output.txt"),
        ("dependencies", "node_modules/pkg/index.js"),
    ] {
        let mut invalid_offer =
            next["data"]["cooperative_evidence_action_packet"]["offer_template"].clone();
        invalid_offer["offer_id"] = serde_json::json!(format!("offer.source-e2e.{suffix}"));
        invalid_offer["attestation"]["source_assessment"] = serde_json::json!({
            "outcome": "pass",
            "summary": "This assessment must be rejected because its basis is invalid",
            "basis_paths": [basis_path],
            "limitations": []
        });
        let input = consumer.write_json(
            &format!("invalid source evidence {suffix}.json"),
            &invalid_offer,
        );
        let rejected_output = run_cooperative_evidence(&consumer, &input);
        assert_eq!(rejected_output.status.code(), Some(2));
        let rejected = json(&rejected_output);
        assert_eq!(rejected["ok"], false);
        assert_eq!(
            rejected["data"]["event"]["payload"]["disposition"],
            "rejected"
        );
        assert_eq!(
            rejected["data"]["event"]["payload"]["rejection"],
            "invalid_assessment_basis"
        );
        next = assert_ok(&consumer.run(&["next"]));
        assert!(next["data"]["cooperative_evidence_action_packet"].is_object());
    }

    let failing_packet = &next["data"]["cooperative_evidence_action_packet"];
    let first_claim_ref = failing_packet["route"]["claim_ref"]
        .as_str()
        .expect("selected source claim")
        .to_owned();
    assert_eq!(failing_packet["route"]["kind"], "artifact_inspection");
    assert_eq!(failing_packet["route"]["strength"], "inspected_artifact");
    let mut failing_offer = failing_packet["offer_template"].clone();
    failing_offer["offer_id"] = serde_json::json!("offer.source-e2e.fail");
    failing_offer["attestation"]["source_assessment"] = serde_json::json!({
        "outcome": "fail",
        "summary": "The selected claim is not supported by the inspected artifact",
        "basis_paths": ["README.md"],
        "limitations": ["same-owner agent inspection, not independent review"]
    });
    let failing_input = consumer.write_json("failing source evidence.json", &failing_offer);
    let admitted_failure = assert_ok(&run_cooperative_evidence(&consumer, &failing_input));
    assert_eq!(
        admitted_failure["data"]["event"]["payload"]["admitted_evidence"]["outcome"],
        "fail"
    );
    next = assert_ok(&consumer.run(&["next"]));
    let failure_audit = next["data"]["cooperative_evidence"]
        .as_array()
        .expect("source audit")
        .iter()
        .find(|audit| audit["admitted_evidence"]["offer_id"] == "offer.source-e2e.fail")
        .expect("failure audit");
    assert_eq!(failure_audit["current_status"], "disproving");
    assert_eq!(
        failure_audit["does_not_satisfy_source_claim_ref"],
        first_claim_ref
    );
    assert!(failure_audit["supports_cooperative_claim_ref"].is_null());
    assert!(failure_audit["proves"].as_array().is_some_and(|proofs| {
        proofs.len() == 1 && proofs[0] == "kernel_verified_content_addressed_basis"
    }));
    assert!(next["data"]["cooperative_evidence_action_packet"].is_object());

    // A rejected event cannot supersede the latest admitted source assessment.
    let mut rejected_after_failure =
        next["data"]["cooperative_evidence_action_packet"]["offer_template"].clone();
    rejected_after_failure["offer_id"] = serde_json::json!("offer.source-e2e.rejected-after-fail");
    rejected_after_failure["attestation"]["source_assessment"] = serde_json::json!({
        "outcome": "pass",
        "summary": "Rejected traversal must not hide the admitted failure",
        "basis_paths": ["../README.md"],
        "limitations": []
    });
    let rejected_after_failure_input = consumer.write_json(
        "rejected source evidence after fail.json",
        &rejected_after_failure,
    );
    assert_eq!(
        run_cooperative_evidence(&consumer, &rejected_after_failure_input)
            .status
            .code(),
        Some(2)
    );
    next = assert_ok(&consumer.run(&["next"]));
    let failure_after_rejection = next["data"]["cooperative_evidence"]
        .as_array()
        .expect("source audit after rejection")
        .iter()
        .find(|audit| audit["admitted_evidence"]["offer_id"] == "offer.source-e2e.fail")
        .expect("failure remains auditable");
    assert_eq!(failure_after_rejection["current_status"], "disproving");

    let mut pass_record_digests = Vec::new();
    for index in 0..claim_count {
        let packet = &next["data"]["cooperative_evidence_action_packet"];
        assert_eq!(
            packet["route"]["assurance_effect"],
            "solo_source_claim_satisfied_by_agent_inspection"
        );
        assert_eq!(packet["route"]["source_provider"], "repository_inspector");
        assert_eq!(packet["route"]["kind"], "artifact_inspection");
        assert_eq!(packet["route"]["strength"], "inspected_artifact");
        assert!(packet["kernel_derived_outcome"].is_null());

        if index == 1 {
            let mut inconclusive_offer = packet["offer_template"].clone();
            inconclusive_offer["offer_id"] = serde_json::json!("offer.source-e2e.inconclusive");
            inconclusive_offer["attestation"]["source_assessment"] = serde_json::json!({
                "outcome": "inconclusive",
                "summary": "The inspected artifact does not establish this claim yet",
                "basis_paths": ["README.md"],
                "limitations": ["insufficient repository evidence"]
            });
            let inconclusive_input =
                consumer.write_json("inconclusive source evidence.json", &inconclusive_offer);
            let admitted_inconclusive =
                assert_ok(&run_cooperative_evidence(&consumer, &inconclusive_input));
            assert_eq!(
                admitted_inconclusive["data"]["event"]["payload"]["admitted_evidence"]["outcome"],
                "inconclusive"
            );
            next = assert_ok(&consumer.run(&["next"]));
            let audit = next["data"]["cooperative_evidence"]
                .as_array()
                .expect("inconclusive audit")
                .iter()
                .find(|audit| {
                    audit["admitted_evidence"]["offer_id"] == "offer.source-e2e.inconclusive"
                })
                .expect("inconclusive audit record");
            assert_eq!(audit["current_status"], "inconclusive");
            assert!(audit["does_not_satisfy_source_claim_ref"].is_string());
            assert!(audit["supports_cooperative_claim_ref"].is_null());
            assert!(audit["proves"].as_array().is_some_and(|proofs| {
                proofs.len() == 1 && proofs[0] == "kernel_verified_content_addressed_basis"
            }));
        }

        let packet = &next["data"]["cooperative_evidence_action_packet"];
        let mut offer = packet["offer_template"].clone();
        offer["offer_id"] = serde_json::json!(format!("offer.source-e2e.{index}"));
        offer["attestation"]["source_assessment"] = serde_json::json!({
            "outcome": "pass",
            "summary": format!("Claim {index} is supported by the inspected project artifact"),
            "basis_paths": ["README.md"],
            "limitations": ["same-owner agent inspection, not independent review"]
        });
        let input = consumer.write_json(&format!("source evidence {index}.json"), &offer);
        let admitted = assert_ok(&run_cooperative_evidence(&consumer, &input));
        assert_eq!(
            admitted["data"]["event"]["payload"]["admitted_evidence"]["outcome"],
            "pass"
        );
        assert!(
            admitted["data"]["event"]["payload"]["admitted_evidence"]["source_assessment"]
                ["basis_digest"]
                .as_str()
                .is_some_and(|digest| digest.starts_with("sha256:"))
        );
        pass_record_digests.push(
            admitted["data"]["record_digest"]
                .as_str()
                .expect("admitted source record digest")
                .to_owned(),
        );
        next = assert_ok(&consumer.run(&["next"]));

        if index == 0 {
            let failure = next["data"]["cooperative_evidence"]
                .as_array()
                .expect("superseded failure audit")
                .iter()
                .find(|audit| audit["admitted_evidence"]["offer_id"] == "offer.source-e2e.fail")
                .expect("superseded failure remains auditable");
            assert_eq!(failure["current_status"], "stale");
        }
        if index == 1 {
            let inconclusive = next["data"]["cooperative_evidence"]
                .as_array()
                .expect("superseded inconclusive audit")
                .iter()
                .find(|audit| {
                    audit["admitted_evidence"]["offer_id"] == "offer.source-e2e.inconclusive"
                })
                .expect("superseded inconclusive remains auditable");
            assert_eq!(inconclusive["current_status"], "stale");
        }
    }

    assert_eq!(
        next["data"]["selected_policy_ref"],
        "policy.workflow.investigation"
    );
    assert_eq!(next["data"]["applicability"], true);
    assert_eq!(next["data"]["status"], "ready_to_complete");
    assert!(next["data"]["simulation"]["candidate_capability_gaps"]
        .as_array()
        .is_some_and(Vec::is_empty));
    assert!(next["data"]["simulation"]["candidate_claim_results"]
        .as_array()
        .is_some_and(|claims| claims.iter().all(|claim| claim["status"] == "verified")));
    assert_eq!(
        next["data"]["durable_assurance"]["blockers"]
            .as_array()
            .map(Vec::len),
        Some(8),
        "release assurance stays visible without blocking ordinary solo execution"
    );

    let resumed = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(
        resumed["data"]["schema_version"],
        "workflow_resume_summary_v10"
    );
    let selected_policy_evidence = resumed["data"]["selected_policy_evidence"]
        .as_array()
        .expect("resume must retain current selected-policy evidence context");
    assert_eq!(
        selected_policy_evidence.len(),
        claim_count + 1,
        "resume must retain one applicability assessment and each current source assessment"
    );
    let first_source_assessment = selected_policy_evidence
        .iter()
        .find(|evidence| {
            evidence["source_assessment"]["summary"]
                == "Claim 0 is supported by the inspected project artifact"
        })
        .expect("replacement agent must receive the current source assessment summary");
    assert_eq!(first_source_assessment["target"], "source_claim");
    assert_eq!(first_source_assessment["outcome"], "pass");
    assert_eq!(
        first_source_assessment["source_assessment"]["limitations"],
        serde_json::json!(["same-owner agent inspection, not independent review"])
    );
    assert!(first_source_assessment["source_assessment"]["basis"]
        .as_array()
        .is_some_and(|basis| basis.len() == 1));
    assert!(selected_policy_evidence.iter().all(|evidence| {
        evidence["source_assessment"]["summary"]
            != "The selected claim is not supported by the inspected artifact"
            && evidence["source_assessment"]["summary"]
                != "The inspected artifact does not establish this claim yet"
    }));

    let completion_snapshot = next["data"]["snapshot_digest"]
        .as_str()
        .expect("ready completion snapshot")
        .to_owned();
    let completed = assert_ok(&consumer.run(&[
        "complete",
        "--if-snapshot",
        &completion_snapshot,
        "--principal",
        "principal.agent.source-e2e",
    ]));
    let mut bound_source_digests = completed["data"]["completed_record"]["event"]["payload"]
        ["evidence_receipt_digests"]
        .as_array()
        .expect("completion evidence receipt bindings")
        .iter()
        .map(|value| value.as_str().expect("bound digest").to_owned())
        .collect::<Vec<_>>();
    pass_record_digests.sort();
    bound_source_digests.sort();
    assert_eq!(bound_source_digests, pass_record_digests);

    let after_policy_change = assert_ok(&consumer.run(&["resume"]));
    assert_ne!(
        after_policy_change["data"]["selected_policy_ref"],
        investigation_policy
    );
    assert_eq!(
        after_policy_change["data"]["journey_guidance"], investigation_journey,
        "changing selected policy within the same phase cannot change journey guidance"
    );
}

#[test]
fn solo_deterministic_policy_publishes_and_executes_a_safe_evidence_packet() {
    let consumer = Consumer::new_with_prefix("forge-workflow-solo-deterministic-e2e");
    assert_ok(&consumer.run(&["init"]));
    upgrade_to_latest(&consumer);

    let objective_next = assert_ok(&consumer.run(&["next"]));
    let packet_digest = objective_next["data"]["authorization"]["action_packets"][0]
        ["packet_digest"]
        .as_str()
        .expect("cooperative objective packet")
        .to_owned();
    let objective = consumer.write_json(
        "deterministic evidence objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Run concrete technical checks in Solo Cooperative mode",
                "constraints": ["only safe local commands"],
                "unacceptable_outcomes": ["claim a check ran when it did not"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.deterministic-e2e",
            "host_provenance": {
                "host_id": "host.deterministic-e2e",
                "host_version": "test",
                "session_ref": "session.deterministic-e2e",
                "interaction_ref": "turn.deterministic-e2e",
                "conversation_digest": format!("sha256:{}", "d".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    assert_ok(&run_cooperative_input(
        &consumer,
        &packet_digest,
        &objective,
    ));

    let mut next =
        advance_fixture_to_policy(&consumer, "policy.workflow.technical-feasibility-scan");
    assert_eq!(next["data"]["status"], "applicability_required");
    let applicability_packet = next["data"]["cooperative_evidence_action_packet"].clone();
    let mut applicability_offer = applicability_packet["offer_template"].clone();
    applicability_offer["offer_id"] = serde_json::json!("offer.deterministic-e2e.applicability");
    applicability_offer["attestation"]["applicability_assessment"] = serde_json::json!({
        "outcome": "applicable",
        "summary": "This fixture needs a concrete technical feasibility check",
        "basis_paths": ["README.md"],
        "limitations": ["same-owner applicability assessment"]
    });
    let applicability_input =
        consumer.write_json("deterministic applicability.json", &applicability_offer);
    assert_ok(&execute_cooperative_packet(
        &applicability_packet,
        &applicability_input,
    ));

    next = assert_ok(&consumer.run(&["resume"]));
    let packet = &next["data"]["actions"]["cooperative_evidence_packet"];
    assert_eq!(packet["route"]["source_provider"], "deterministic_tool");
    assert_eq!(packet["route"]["provider"], "deterministic_tool");
    assert_eq!(packet["route"]["kind"], "deterministic_check");
    assert_eq!(packet["route"]["strength"], "deterministic_verification");
    assert_eq!(
        packet["route"]["assurance_effect"],
        "solo_source_claim_satisfied_by_kernel_execution"
    );
    assert!(packet["required_replacements"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item == "${FORGE_COMMAND_CONTRACT}")));

    let mut offer = packet["offer_template"].clone();
    offer["offer_id"] = serde_json::json!("offer.deterministic-e2e.git-version");
    offer["attestation"]["execution_request"] = serde_json::json!({
        "summary": "Git starts successfully in the selected project",
        "scenario_ref": "technical-feasibility.git-version",
        "limitations": ["This proves only the selected local command"],
        "command": {
            "schema_version": "0.1",
            "command_contract": {
                "id": "command.workflow.deterministic-e2e.git-version",
                "contract_ref": "contracts/commands/workflow-deterministic-check.yaml",
                "kind": "smoke",
                "executor": "git",
                "args": ["--version"],
                "cwd_policy": "project_root",
                "side_effect_policy": "read_only",
                "platforms": ["windows", "macos", "linux"],
                "timeout_ms": 10000,
                "env_policy": { "inherit": "minimal", "required": [], "forbidden": [] },
                "network_policy": "disabled",
                "output_policy": { "capture": "summary", "max_bytes": 4096 },
                "authority_required": [],
                "safety": {
                    "shell_string_allowed": false,
                    "writes_files": false,
                    "publishes": false,
                    "installs_packages": false
                }
            }
        }
    });
    let input = consumer.write_json("deterministic execution.json", &offer);
    let admitted = assert_ok(&execute_cooperative_packet(packet, &input));
    assert_eq!(
        admitted["data"]["event"]["payload"]["admitted_evidence"]["outcome"],
        "pass"
    );
    assert_eq!(
        admitted["data"]["event"]["payload"]["admitted_evidence"]["execution_assessment"]["status"],
        "succeeded"
    );
    assert_eq!(
        admitted["data"]["event"]["payload"]["admitted_evidence"]["execution_assessment"]
            ["exit_code"],
        0
    );
    let deterministic_record_digest = admitted["data"]["record_digest"]
        .as_str()
        .expect("deterministic evidence record digest")
        .to_owned();

    let after = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(
        after["data"]["status"], "ready_to_complete",
        "a successful kernel execution must satisfy both the selected claim and its proof-environment capability"
    );
    assert!(
        after["data"]["current_evaluation"]["candidate_capability_gaps"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    let selected_policy_evidence = after["data"]["selected_policy_evidence"]
        .as_array()
        .expect("deterministic resume evidence");
    let execution = selected_policy_evidence
        .iter()
        .find_map(|evidence| evidence["execution_assessment"].as_object())
        .expect("resume must retain the current deterministic execution assessment");
    assert_eq!(
        execution["summary"],
        "Git starts successfully in the selected project"
    );
    assert_eq!(execution["status"], "succeeded");
    assert_eq!(
        execution["limitations"],
        serde_json::json!(["This proves only the selected local command"])
    );
    assert!(execution["reasons"].is_array());
    for omitted in ["stdout", "stderr", "stdout_truncated", "stderr_truncated"] {
        assert!(
            !execution.contains_key(omitted),
            "compact resume must omit deterministic {omitted}"
        );
    }

    let completion_snapshot = after["data"]["snapshot_digest"]
        .as_str()
        .expect("deterministic completion snapshot")
        .to_owned();
    assert_ok(&consumer.run(&[
        "complete",
        "--if-snapshot",
        &completion_snapshot,
        "--principal",
        "principal.agent.deterministic-e2e",
    ]));
    let advanced = assert_ok(&consumer.run(&["next"]));
    assert!(advanced["data"]["boundary_rechecks"]
        .as_array()
        .is_some_and(|rechecks| rechecks.iter().all(|recheck| {
            recheck["policy_ref"] != "policy.workflow.technical-feasibility-scan"
        })), "a completed deterministic feasibility check must not block the next policy by forgetting its proof environment");

    let mut closeout = advance_fixture_to_policy(&consumer, "policy.workflow.research-closeout");
    if closeout["data"]["status"] == "applicability_required" {
        let applicability_packet = closeout["data"]["cooperative_evidence_action_packet"].clone();
        let mut applicability_offer = applicability_packet["offer_template"].clone();
        applicability_offer["offer_id"] =
            serde_json::json!("offer.deterministic-e2e.closeout-applicability");
        applicability_offer["attestation"]["applicability_assessment"] = serde_json::json!({
            "outcome": "applicable",
            "summary": "The completed feasibility check needs an explicit research closeout",
            "basis_paths": ["README.md"],
            "limitations": ["same-owner applicability assessment"]
        });
        let applicability_input =
            consumer.write_json("closeout applicability.json", &applicability_offer);
        assert_ok(&execute_cooperative_packet(
            &applicability_packet,
            &applicability_input,
        ));
    }
    closeout = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(closeout["data"]["status"], "blocked");
    let mut closeout_packet = closeout["data"]["actions"]["cooperative_evidence_packet"].clone();
    assert!(
        closeout_packet["available_prior_evidence"]
            .as_array()
            .is_some_and(|evidence| evidence.iter().any(|item| {
                item["record_digest"] == deterministic_record_digest
                    && item["kind"] == "deterministic_check"
                    && item["policy_ref"] == "policy.workflow.technical-feasibility-scan"
            })),
        "research closeout packet: {closeout_packet:#}"
    );

    let mut fabricated_offer = closeout_packet["offer_template"].clone();
    fabricated_offer["offer_id"] = serde_json::json!("offer.deterministic-e2e.closeout-fabricated");
    fabricated_offer["attestation"]["source_assessment"] = serde_json::json!({
        "outcome": "pass",
        "summary": "A fabricated receipt must not support the closeout",
        "basis_paths": [],
        "prior_evidence_record_digests": [format!("sha256:{}", "f".repeat(64))],
        "limitations": ["negative-path fixture"]
    });
    let fabricated_input =
        consumer.write_json("fabricated closeout receipt.json", &fabricated_offer);
    let rejected = execute_cooperative_packet(&closeout_packet, &fabricated_input);
    assert_eq!(rejected.status.code(), Some(2));
    assert_eq!(
        json(&rejected)["data"]["event"]["payload"]["rejection"],
        "invalid_assessment_basis"
    );

    closeout = assert_ok(&consumer.run(&["resume"]));
    closeout_packet = closeout["data"]["actions"]["cooperative_evidence_packet"].clone();
    let mut closeout_offer = closeout_packet["offer_template"].clone();
    closeout_offer["offer_id"] = serde_json::json!("offer.deterministic-e2e.closeout-questions");
    closeout_offer["attestation"]["source_assessment"] = serde_json::json!({
        "outcome": "pass",
        "summary": "The feasibility question was answered by the kernel-executed Git check",
        "basis_paths": [],
        "prior_evidence_record_digests": [deterministic_record_digest],
        "limitations": ["This does not test WSL or network failure"]
    });
    let closeout_input = consumer.write_json("closeout questions.json", &closeout_offer);
    let closeout_admitted = assert_ok(&execute_cooperative_packet(
        &closeout_packet,
        &closeout_input,
    ));
    assert_eq!(
        closeout_admitted["data"]["event"]["payload"]["admitted_evidence"]["source_assessment"]
            ["prior_evidence"][0]["record_digest"],
        deterministic_record_digest
    );

    let closeout_after = assert_ok(&consumer.run(&["resume"]));
    let questions_assessment = closeout_after["data"]["selected_policy_evidence"]
        .as_array()
        .expect("research closeout evidence")
        .iter()
        .find(|evidence| {
            evidence["claim_ref"] == "claim.workflow.research-closeout.questions-covered"
        })
        .expect("questions-covered assessment");
    assert_eq!(
        questions_assessment["source_assessment"]["prior_evidence"][0]["record_digest"],
        deterministic_record_digest
    );
}
#[test]
fn cooperative_objective_cli_supersedes_then_clarifies_with_replacement_readback() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));
    let initial_next = assert_ok(&consumer.run(&["next"]));
    let initial_packet = initial_next["data"]["authorization"]["action_packets"][0]
        ["packet_digest"]
        .as_str()
        .expect("initial packet")
        .to_owned();
    let initial_input = consumer.write_json(
        "initial objective history.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Use Forge for solo developer agent dogfooding",
                "constraints": ["remain host neutral"],
                "unacceptable_outcomes": ["claim verified human origin"],
                "open_uncertainties": ["future team authority"]
            },
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.cli-e2e",
                "interaction_ref": "turn.initial",
                "conversation_digest": format!("sha256:{}", "a".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let initial = assert_ok(&run_cooperative_input(
        &consumer,
        &initial_packet,
        &initial_input,
    ));
    let initial_digest = initial["data"]["active_objective"]["objective_digest"]
        .as_str()
        .expect("initial digest")
        .to_owned();
    let initial_completion_snapshot = initial["data"]["next"]["snapshot_digest"]
        .as_str()
        .expect("initial ready snapshot")
        .to_owned();
    assert_eq!(initial["data"]["next"]["status"], "ready_to_complete");
    assert_ok(&consumer.run(&[
        "complete",
        "--if-snapshot",
        &initial_completion_snapshot,
        "--principal",
        "principal.agent.cli-e2e",
    ]));
    let after_initial_completion = assert_ok(&consumer.run(&["next"]));
    assert_eq!(
        after_initial_completion["data"]["selected_policy_ref"],
        "policy.workflow.domain-scan"
    );
    let material_packet = after_initial_completion["data"]["authorization"]
        ["objective_management_packet"]["packet_digest"]
        .as_str()
        .expect("material packet")
        .to_owned();
    let material_input = consumer.write_json(
        "material objective correction.json",
        &serde_json::json!({
            "kind": "material_supersession",
            "proposal": {
                "outcome": "Make Forge excellent for solo developer dogfooding before teams",
                "constraints": ["remain host neutral"],
                "unacceptable_outcomes": ["claim verified human origin"],
                "open_uncertainties": ["future team authority"]
            },
            "supersession_reason": "The owner narrowed the immediate product direction",
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.cli-e2e",
                "interaction_ref": "turn.material",
                "conversation_digest": format!("sha256:{}", "b".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let material = assert_ok(&run_cooperative_input(
        &consumer,
        &material_packet,
        &material_input,
    ));
    assert_eq!(material["data"]["active_objective"]["revision"], 2);
    assert_eq!(
        material["data"]["active_objective"]["revision_kind"],
        "material_supersession"
    );
    assert_eq!(
        material["data"]["active_objective"]["previous_objective_digest"],
        initial_digest
    );
    assert_eq!(
        material["data"]["active_objective"]["revision_reason"],
        "The owner narrowed the immediate product direction"
    );
    assert_eq!(
        material["data"]["next"]["selected_policy_ref"], "policy.workflow.discover-intent",
        "a new material objective must invalidate the completion bound to the prior revision"
    );
    assert_eq!(
        material["data"]["next"]["status"], "ready_to_complete",
        "the replacement objective itself must immediately reground discover-intent"
    );
    assert!(
        material["data"]["next"]["cooperative_evidence_action_packet"].is_null(),
        "a replacement objective must not create a redundant evidence step"
    );

    let material_record_digest = material["data"]["objective_record"]["record_digest"]
        .as_str()
        .expect("material objective record")
        .to_owned();
    let material_completion_snapshot = material["data"]["next"]["snapshot_digest"]
        .as_str()
        .expect("material completion snapshot")
        .to_owned();
    let material_completed = assert_ok(&consumer.run(&[
        "complete",
        "--if-snapshot",
        &material_completion_snapshot,
        "--principal",
        "principal.agent.cli-e2e",
    ]));
    assert_eq!(
        material_completed["data"]["completed_record"]["event"]["payload"]
            ["grounding_anchor_digests"],
        serde_json::json!([material_record_digest])
    );
    let after_material_completion = assert_ok(&consumer.run(&["next"]));
    assert_eq!(
        after_material_completion["data"]["selected_policy_ref"],
        "policy.workflow.domain-scan"
    );
    let clarification_packet = after_material_completion["data"]["authorization"]
        ["objective_management_packet"]["packet_digest"]
        .as_str()
        .expect("clarification packet")
        .to_owned();
    let clarification_input = consumer.write_json(
        "non material objective clarification.json",
        &serde_json::json!({
            "kind": "non_material_clarification",
            "added_constraints": ["use focused verification per ticket"],
            "added_unacceptable_outcomes": [],
            "added_open_uncertainties": ["batch cadence remains adjustable"],
            "clarification_reason": "The owner added execution detail without changing direction",
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.cli-e2e",
                "interaction_ref": "turn.clarification",
                "conversation_digest": format!("sha256:{}", "c".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let clarified = assert_ok(&run_cooperative_input(
        &consumer,
        &clarification_packet,
        &clarification_input,
    ));
    assert_eq!(clarified["data"]["active_objective"]["revision"], 3);
    assert_eq!(
        clarified["data"]["active_objective"]["revision_kind"],
        "non_material_clarification"
    );
    assert_eq!(
        clarified["data"]["active_objective"]["proposal"]["outcome"],
        material["data"]["active_objective"]["proposal"]["outcome"]
    );
    assert!(
        clarified["data"]["active_objective"]["proposal"]["constraints"]
            .as_array()
            .expect("constraints")
            .iter()
            .any(|value| value == "use focused verification per ticket")
    );
    assert_eq!(
        clarified["data"]["next"]["selected_policy_ref"],
        "policy.workflow.domain-scan",
        "a non-material clarification must preserve the material anchor and not reopen discover-intent"
    );

    let before_stale = state_tree_snapshot(&consumer.state);
    let stale = run_cooperative_input(&consumer, &material_packet, &material_input);
    assert_eq!(stale.status.code(), Some(4));
    let stale_envelopes = serde_json::Deserializer::from_slice(&stale.stdout)
        .into_iter::<Value>()
        .collect::<Result<Vec<_>, _>>()
        .expect("stale output must contain JSON envelopes only");
    assert_eq!(
        stale_envelopes.len(),
        1,
        "stale management failure must emit one envelope"
    );
    let stale_envelope = &stale_envelopes[0];
    assert_eq!(stale_envelope["exit_reason"], "conflict");
    let message = stale_envelope["error"]["message"]
        .as_str()
        .expect("stale actionable message");
    assert!(message.contains("stale"));
    assert!(message.contains("run workflow next"));
    assert!(!message.to_ascii_lowercase().contains("human"));
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        before_stale,
        "stale objective-management packets must not alter WAL or state"
    );
    let summary = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(
        summary["data"]["omitted_history"]["superseded_objective_revisions"], 2,
        "the separately exposed active objective is not omitted history"
    );
    assert!(summary["data"]["omitted_history"]
        .get("objective_revisions")
        .is_none());
    let replacement = assert_ok(&consumer.run(&["report"]));
    assert_eq!(
        replacement["data"]["active_cooperative_objective"]["revision"],
        3
    );
    assert_eq!(
        replacement["data"]["active_cooperative_objective"]["revision_reason"],
        "The owner added execution detail without changing direction"
    );
    let history = replacement["data"]["replacement_continuity"]["objective_history"]
        .as_array()
        .expect("durable ordered objective history");
    assert_eq!(history.len(), 3);
    assert_eq!(history[0]["objective"]["revision"], 1);
    assert_eq!(history[0]["objective"]["revision_kind"], "initial");
    assert_eq!(history[0]["active"], false);
    assert_eq!(history[1]["objective"]["revision"], 2);
    assert_eq!(
        history[1]["objective"]["revision_kind"],
        "material_supersession"
    );
    assert_eq!(
        history[1]["objective"]["previous_objective_digest"],
        initial_digest
    );
    assert_eq!(history[1]["active"], false);
    assert_eq!(history[2]["objective"]["revision"], 3);
    assert_eq!(
        history[2]["objective"]["revision_kind"],
        "non_material_clarification"
    );
    assert_eq!(history[2]["active"], true);
    assert_eq!(
        replacement["data"]["replacement_continuity"]["binding"]["active_objective_revision"],
        3
    );
}

#[test]
fn public_episode_apply_routes_evolve_changes_and_resume_context() {
    let consumer = Consumer::new_with_prefix("forge-public-episode-e2e");
    assert_ok(&consumer.run(&["init", "--readiness-profile", "solo_cooperative"]));
    let discovery = assert_ok(&consumer.run(&["next"]));
    let packet_digest = discovery["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("objective packet")
        .to_owned();
    let objective = consumer.write_json(
        "episode objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Keep the stable notes app useful",
                "constraints": ["preserve existing notes"],
                "unacceptable_outcomes": ["lose the released product context"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.episode-e2e",
            "host_provenance": {
                "host_id": "host.episode-e2e",
                "host_version": "test",
                "session_ref": "session.episode-e2e",
                "interaction_ref": "turn.initial-objective",
                "conversation_digest": format!("sha256:{}", "8".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let accepted = assert_ok(&run_cooperative_input(
        &consumer,
        &packet_digest,
        &objective,
    ));
    let initial_digest = accepted["data"]["active_objective"]["objective_digest"]
        .as_str()
        .expect("initial objective digest")
        .to_owned();
    let snapshot = accepted["data"]["next"]["snapshot_digest"]
        .as_str()
        .expect("stable snapshot")
        .to_owned();
    append_test_phase_transition(&consumer, "1-discovery", "5-ready-operate", &snapshot);

    let mut prepared = assert_ok(&consumer.prepare_episode());
    assert_eq!(prepared["command"], "workflow.episode.prepare");
    assert_eq!(prepared["data"]["current_phase"], "5-ready-operate");
    assert_eq!(prepared["data"]["applicable_now"], true);
    assert_eq!(
        prepared["data"]["apply_input_template"]["expected_snapshot_digest"],
        prepared["data"]["binding"]["snapshot_digest"]
    );

    let (applied, input) = loop {
        let state_before_read_only_handoff = state_tree_snapshot(&consumer.state);
        let candidate = consumer.write_json(
            "episode candidate.json",
            &prepared_episode_candidate(&prepared),
        );
        let finalized_output = Consumer::finalize_episode(&candidate);
        assert!(
            finalized_output.stdout.len() < 32 * 1024,
            "finalized episode input is unexpectedly large"
        );
        let finalized = assert_ok(&finalized_output);
        assert_eq!(finalized["command"], "workflow.episode.finalize");
        assert_eq!(finalized["data"]["status"], "valid_candidate_only");
        assert_eq!(
            state_tree_snapshot(&consumer.state),
            state_before_read_only_handoff,
            "prepare/finalize handoff must not mutate Forge state"
        );
        let input = consumer.write_json("episode apply.json", &finalized["data"]["apply_input"]);
        let attempt = consumer.apply_episode(&input);
        if attempt.status.success() {
            break (assert_ok(&attempt), input);
        }
        let rejection = json(&attempt);
        assert_eq!(rejection["exit_reason"], "rejected_by_gate");

        let output = consumer.run(&["next"]);
        let next = json(&output);
        assert!(
            output.status.success(),
            "unexpected guidance failure: {next:#}"
        );
        append_test_policy_completion(&consumer, &next);
        prepared = assert_ok(&consumer.prepare_episode());
    };
    assert_eq!(applied["command"], "workflow.episode.apply");
    assert_eq!(applied["data"]["outcome"], "advanced_to_evolve");

    let resumed = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(resumed["data"]["current_phase"], "6-evolve");
    let evolve_head = resumed["data"]["ledger_head_digest"].clone();

    let follow_on = assert_ok(&consumer.prepare_episode());
    assert_eq!(follow_on["data"]["latest_episode"]["generation"], 1);
    assert_eq!(
        follow_on["data"]["apply_input_template"]["document"]["post_build_verify_episode"]
            ["generation"],
        2
    );
    assert_eq!(
        follow_on["data"]["apply_input_template"]["document"]["post_build_verify_episode"]
            ["previous_episode_digest"],
        follow_on["data"]["latest_episode"]["episode_digest"]
    );

    let retry = consumer.apply_episode(&input);
    let retry_envelope = json(&retry);
    assert!(!retry.status.success());
    assert_eq!(retry_envelope["command"], "workflow.episode.apply");
    assert_eq!(retry_envelope["exit_reason"], "conflict");

    let after_retry = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(after_retry["data"]["current_phase"], "6-evolve");
    assert_eq!(after_retry["data"]["ledger_head_digest"], evolve_head);

    let clarification_packet = after_retry["data"]["authorization"]["objective_management_packet"]
        ["packet_digest"]
        .as_str()
        .expect("Evolve objective-management packet")
        .to_owned();
    let clarification_input = consumer.write_json(
        "evolve non material clarification.json",
        &serde_json::json!({
            "kind": "non_material_clarification",
            "added_constraints": ["use focused verification for each accepted change"],
            "added_unacceptable_outcomes": [],
            "added_open_uncertainties": [],
            "clarification_reason": "The owner added execution detail without changing product direction",
            "carrying_principal": "principal.agent.episode-e2e",
            "host_provenance": {
                "host_id": "host.episode-e2e",
                "host_version": "test",
                "session_ref": "session.episode-e2e",
                "interaction_ref": "turn.non-material-clarification",
                "conversation_digest": format!("sha256:{}", "c".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let wal = consumer.state.join("wal/workflow-governance.ndjson");
    let before_clarification_lines = fs::read_to_string(&wal)
        .expect("WAL before Evolve clarification")
        .lines()
        .count();
    let clarified = assert_ok(&run_cooperative_input(
        &consumer,
        &clarification_packet,
        &clarification_input,
    ));
    assert_eq!(clarified["data"]["next"]["current_phase"], "6-evolve");
    assert_eq!(clarified["data"]["active_objective"]["revision"], 2);
    assert_eq!(
        clarified["data"]["active_objective"]["revision_kind"],
        "non_material_clarification"
    );
    assert_eq!(
        clarified["data"]["active_objective"]["previous_objective_digest"],
        initial_digest
    );
    let clarified_digest = clarified["data"]["active_objective"]["objective_digest"]
        .as_str()
        .expect("clarified objective digest")
        .to_owned();
    let clarified_wal = fs::read(&wal).expect("WAL after Evolve clarification");
    assert_eq!(
        String::from_utf8_lossy(&clarified_wal).lines().count(),
        before_clarification_lines + 1,
        "a non-material clarification should append only its objective revision"
    );
    let clarification_retry = assert_ok(&run_cooperative_input(
        &consumer,
        &clarification_packet,
        &clarification_input,
    ));
    assert_eq!(clarification_retry["data"], clarified["data"]);
    assert_eq!(
        fs::read(&wal).expect("WAL after exact clarification retry"),
        clarified_wal,
        "an exact clarification retry must not duplicate the objective revision"
    );

    let material_packet = clarified["data"]["next"]["authorization"]["objective_management_packet"]
        ["packet_digest"]
        .as_str()
        .expect("fresh Evolve objective-management packet")
        .to_owned();
    let material_input = consumer.write_json(
        "evolve material change.json",
        &serde_json::json!({
            "kind": "material_supersession",
            "proposal": {
                "outcome": "Let two family members share selected notes",
                "constraints": ["preserve existing notes", "ask before sharing"],
                "unacceptable_outcomes": ["silently lose prior product context"],
                "open_uncertainties": ["which notes are shareable"]
            },
            "supersession_reason": "The owner requested a new sharing capability for the stable product",
            "carrying_principal": "principal.agent.episode-e2e",
            "host_provenance": {
                "host_id": "host.episode-e2e",
                "host_version": "test",
                "session_ref": "session.episode-e2e",
                "interaction_ref": "turn.material-change",
                "conversation_digest": format!("sha256:{}", "e".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let before_lines = fs::read_to_string(&wal)
        .expect("WAL before Evolve reentry")
        .lines()
        .count();
    let material = assert_ok(&run_cooperative_input(
        &consumer,
        &material_packet,
        &material_input,
    ));
    assert_eq!(material["data"]["next"]["current_phase"], "1-discovery");
    assert_eq!(material["data"]["active_objective"]["revision"], 3);
    assert_eq!(
        material["data"]["active_objective"]["previous_objective_digest"],
        clarified_digest
    );
    let material_wal = fs::read(&wal).expect("WAL after Evolve reentry");
    assert_eq!(
        String::from_utf8_lossy(&material_wal).lines().count(),
        before_lines + 2,
        "one bounded transaction should append only the objective and Evolve-to-Discovery records"
    );
    let material_retry = assert_ok(&run_cooperative_input(
        &consumer,
        &material_packet,
        &material_input,
    ));
    assert_eq!(material_retry["data"], material["data"]);
    assert_eq!(
        fs::read(&wal).expect("WAL after exact Evolve retry"),
        material_wal,
        "an exact retry must not duplicate the objective or phase transition"
    );

    let replacement_resume = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(replacement_resume["data"]["current_phase"], "1-discovery");
    assert_eq!(
        replacement_resume["data"]["active_objective"]["previous_objective_digest"],
        clarified_digest,
        "replacement-agent readback must keep the previous product direction as context"
    );
}

#[test]
fn agent_autonomy_cli_is_read_only_typed_and_stale_after_objective_change() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));
    let next = assert_ok(&consumer.run(&["next"]));
    let packet = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("objective packet")
        .to_owned();
    let objective_input = consumer.write_json(
        "autonomy initial objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Improve Forge through solo developer agent dogfooding",
                "constraints": ["remain host neutral"],
                "unacceptable_outcomes": ["ask for routine implementation approval"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.autonomy",
                "interaction_ref": "turn.objective",
                "conversation_digest": format!("sha256:{}", "d".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let accepted = assert_ok(&run_cooperative_input(&consumer, &packet, &objective_input));
    let autonomy = &accepted["data"]["next"]["agent_autonomy"];
    let binding = autonomy["binding"].clone();
    assert_eq!(autonomy["status"], "active");
    assert_eq!(binding["assurance_epoch"], 1);
    assert_eq!(autonomy["assessment_argv"][0], "forge-core");
    assert_eq!(autonomy["input_contract"]["unknown_fields_allowed"], false);
    assert_eq!(
        autonomy["input_contract"]["temporary_input_must_be_outside_project_snapshot"],
        true
    );
    let assess = |name: &str, work: Value, effect: Value| {
        consumer.write_json(
            name,
            &serde_json::json!({
                "schema_version": "agent_autonomy_assessment_v1",
                "binding": binding.clone(),
                "work": work,
                "effect": effect
            }),
        )
    };
    let local = assess(
        "autonomy local edit.json",
        serde_json::json!({
            "kind": "agent_owned",
            "class": "reversible_local_editing",
            "summary": "edit a local Rust file and run a focused test"
        }),
        serde_json::json!({"scope": "local_reversible"}),
    );
    let objective = assess(
        "autonomy objective decision.json",
        serde_json::json!({
            "kind": "human_decision",
            "class": "product_objective_change",
            "summary": "expand the product objective to enterprise teams"
        }),
        serde_json::json!({"scope": "local_read_only"}),
    );
    let publication = assess(
        "autonomy publication decision.json",
        serde_json::json!({
            "kind": "agent_owned",
            "class": "reversible_local_editing",
            "summary": "publish the alpha release"
        }),
        serde_json::json!({
            "scope": "protected_effect",
            "effect": "publication"
        }),
    );
    let contradiction = assess(
        "autonomy contradiction.json",
        serde_json::json!({
            "kind": "agent_owned",
            "class": "reversible_local_editing",
            "summary": "claim a local edit while invoking an external read"
        }),
        serde_json::json!({"scope": "external_read_only"}),
    );
    let external_mutation = assess(
        "autonomy external mutation.json",
        serde_json::json!({
            "kind": "agent_owned",
            "class": "documentation",
            "summary": "post the result to Jira"
        }),
        serde_json::json!({"scope": "external_mutation"}),
    );
    let missing_effect = consumer.write_json(
        "autonomy missing effect.json",
        &serde_json::json!({
            "schema_version": "agent_autonomy_assessment_v1",
            "binding": binding.clone(),
            "work": {
                "kind": "agent_owned",
                "class": "reversible_local_editing",
                "summary": "edit locally"
            }
        }),
    );

    let before = state_tree_snapshot(&consumer.state);
    let local_result = assert_ok(&run_autonomy_assessment(&consumer, &local));
    assert_eq!(local_result["data"]["status"], "proceed_autonomously");
    assert_eq!(local_result["data"]["class"], "reversible_local_editing");
    assert!(local_result["data"].get("request").is_none());

    let objective_result = assert_ok(&run_autonomy_assessment(&consumer, &objective));
    assert_eq!(objective_result["data"]["status"], "decision_required");
    assert_eq!(
        objective_result["data"]["request"]["class"],
        "product_objective_change"
    );
    let objective_request = &objective_result["data"]["request"];
    assert!(objective_request["question"]
        .as_str()
        .is_some_and(|value| value.contains("expand the product objective")));
    assert!(objective_request["alternatives"]
        .as_array()
        .is_some_and(|items| items.len() >= 2));

    for path in [&publication, &contradiction, &external_mutation] {
        let result = assert_ok(&run_autonomy_assessment(&consumer, path));
        assert_eq!(result["data"]["status"], "decision_required");
        assert_eq!(
            result["data"]["request"]["class"],
            "irreversible_or_external_effect"
        );
        assert_ne!(
            result["data"]["request"]["id"],
            objective_result["data"]["request"]["id"]
        );
    }

    let missing = run_autonomy_assessment(&consumer, &missing_effect);
    assert_eq!(missing.status.code(), Some(3));
    let missing = json(&missing);
    assert_eq!(missing["command"], "workflow.autonomy.assess");
    assert_eq!(missing["exit_reason"], "invalid_decision_shape");
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        before,
        "all autonomy assessments must leave Forge state byte-exact"
    );

    let unknown = bin()
        .args(["workflow", "autonomy", "unknown", "--json"])
        .output()
        .expect("unknown autonomy command");
    assert_eq!(unknown.status.code(), Some(3));
    assert_eq!(json(&unknown)["command"], "workflow.autonomy");
    let conflicting_output = bin()
        .args(["workflow", "autonomy", "assess", "--json", "--text"])
        .output()
        .expect("conflicting output flags");
    assert_eq!(conflicting_output.status.code(), Some(3));
    assert_eq!(json(&conflicting_output)["command"], "workflow.autonomy");
    let text_unknown = bin()
        .args(["workflow", "autonomy", "unknown", "--text"])
        .output()
        .expect("text autonomy error");
    assert_eq!(text_unknown.status.code(), Some(3));
    assert!(serde_json::from_slice::<Value>(&text_unknown.stdout).is_err());

    let revision_packet = accepted["data"]["next"]["authorization"]["objective_management_packet"]
        ["packet_digest"]
        .as_str()
        .expect("revision packet")
        .to_owned();
    let revision = consumer.write_json(
        "autonomy objective supersession.json",
        &serde_json::json!({
            "kind": "material_supersession",
            "proposal": {
                "outcome": "Make Forge excellent for solo developer dogfooding before teams",
                "constraints": ["remain host neutral"],
                "unacceptable_outcomes": ["ask for routine implementation approval"],
                "open_uncertainties": []
            },
            "supersession_reason": "The owner narrowed the immediate direction",
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.autonomy",
                "interaction_ref": "turn.supersede",
                "conversation_digest": format!("sha256:{}", "e".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    assert_ok(&run_cooperative_input(
        &consumer,
        &revision_packet,
        &revision,
    ));
    let after_revision = state_tree_snapshot(&consumer.state);
    let stale = run_autonomy_assessment(&consumer, &local);
    assert_eq!(stale.status.code(), Some(4));
    let stale = json(&stale);
    assert_eq!(stale["command"], "workflow.autonomy.assess");
    assert_eq!(stale["exit_reason"], "conflict");
    assert!(stale["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("stale")));
    assert_eq!(state_tree_snapshot(&consumer.state), after_revision);
}

#[test]
fn cooperative_decision_validates_live_packet_and_keeps_the_entire_state_tree_byte_exact() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));
    let next = assert_ok(&consumer.run(&["next"]));
    let packet = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("cooperative packet")
        .to_owned();
    let input = consumer.write_json(
        "decision request with spaces.json",
        &cooperative_decision_json(),
    );
    let before = state_tree_snapshot(&consumer.state);

    let decision = assert_ok(&run_cooperative_input(&consumer, &packet, &input));
    assert_eq!(decision["command"], "workflow.intent.accept_cooperative");
    assert_eq!(decision["data"]["status"], "decision_required");
    assert_eq!(state_tree_snapshot(&consumer.state), before);

    let wrong = run_cooperative_input(&consumer, &format!("sha256:{}", "f".repeat(64)), &input);
    assert!(!wrong.status.success());
    assert_eq!(
        json(&wrong)["command"],
        "workflow.intent.accept_cooperative"
    );
    assert_eq!(state_tree_snapshot(&consumer.state), before);

    fs::write(consumer.app.join("README.md"), "stale packet snapshot\n").expect("change project");
    let stale = run_cooperative_input(&consumer, &packet, &input);
    assert!(!stale.status.success());
    assert_eq!(
        json(&stale)["command"],
        "workflow.intent.accept_cooperative"
    );
    assert_eq!(state_tree_snapshot(&consumer.state), before);

    let strict = Consumer::new();
    assert_ok(&strict.run(&["init", "--readiness-profile", "strict_external"]));
    let strict_next = assert_ok(&strict.run(&["next"]));
    let strict_packet = strict_next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("strict intent packet")
        .to_owned();
    let strict_input =
        strict.write_json("strict decision request.json", &cooperative_decision_json());
    let strict_before = state_tree_snapshot(&strict.state);
    let rejected = run_cooperative_input(&strict, &strict_packet, &strict_input);
    assert!(!rejected.status.success());
    let rejected = json(&rejected);
    assert_eq!(rejected["command"], "workflow.intent.accept_cooperative");
    assert!(rejected["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("solo_cooperative")));
    assert_eq!(state_tree_snapshot(&strict.state), strict_before);
}

#[test]
fn cooperative_cli_rejects_unbounded_ambiguous_or_special_inputs_with_exact_command_envelopes() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));
    let next = assert_ok(&consumer.run(&["next"]));
    let packet = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("cooperative packet")
        .to_owned();
    let before = state_tree_snapshot(&consumer.state);

    let unknown = consumer.write_json(
        "unknown field.json",
        &serde_json::json!({
            "kind": "decision_required",
            "decision_request": cooperative_decision_json()["decision_request"],
            "invented_authority": true
        }),
    );
    let duplicate = consumer.parent.join("duplicate fields.json");
    fs::write(
        &duplicate,
        br#"{"kind":"decision_required","kind":"unambiguous"}"#,
    )
    .expect("duplicate fixture");
    let invalid_utf8 = consumer.parent.join("invalid encoding.json");
    fs::write(&invalid_utf8, [0xff, 0xfe, 0xfd]).expect("invalid UTF-8 fixture");
    let oversize = consumer.parent.join("oversize.json");
    fs::write(&oversize, vec![b' '; 128 * 1024 + 1]).expect("oversize fixture");
    let directory = consumer.parent.join("input directory");
    fs::create_dir(&directory).expect("input directory fixture");

    let mut cases = vec![unknown, duplicate, invalid_utf8, oversize, directory];
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let valid = consumer.write_json("symlink target.json", &cooperative_decision_json());
        let link = consumer.parent.join("symlink input.json");
        symlink(valid, &link).expect("symlink fixture");
        cases.push(link);
        let fifo = consumer.parent.join("fifo input.json");
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo command");
        assert!(status.success(), "mkfifo fixture");
        cases.push(fifo);
    }

    for path in cases {
        let rejected = run_cooperative_input(&consumer, &packet, &path);
        assert!(
            !rejected.status.success(),
            "hostile input unexpectedly succeeded: {}",
            path.display()
        );
        assert_eq!(
            json(&rejected)["command"],
            "workflow.intent.accept_cooperative",
            "{}",
            path.display()
        );
    }

    let valid = consumer.write_json("duplicate flag valid.json", &cooperative_decision_json());
    let duplicate_flag = bin()
        .args([
            "workflow",
            "intent",
            "accept-cooperative",
            "--root",
            &consumer.app.display().to_string(),
            "--packet-digest",
            &packet,
            "--input-file",
            &valid.display().to_string(),
            "--input-file",
            &valid.display().to_string(),
            "--json",
        ])
        .output()
        .expect("duplicate input-file flag");
    assert!(!duplicate_flag.status.success());
    assert_eq!(
        json(&duplicate_flag)["command"],
        "workflow.intent.accept_cooperative"
    );
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        before,
        "file, encoding, JSON, and usage errors must not touch Forge state"
    );
}

#[test]
fn readiness_profile_reconfiguration_is_an_exact_conflict_envelope() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));

    let rejected = consumer.run(&["init", "--readiness-profile", "strict_external"]);
    assert_eq!(rejected.status.code(), Some(4));
    assert_eq!(
        json(&rejected),
        serde_json::json!({
            "schema_version": "0.1",
            "command": "workflow.init",
            "ok": false,
            "exit_reason": "conflict",
            "error": {
                "code": "conflict",
                "message": "workflow readiness profile cannot be reconfigured from solo_cooperative to strict_external after initialization"
            }
        })
    );
}

#[test]
#[allow(clippy::too_many_lines)] // One public ceremony proves both local denial and permitted one-call commit.
fn local_action_authorize_prepares_signs_and_commits_without_intermediate_authority_files() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init", "--readiness-profile", "strict_external"]));
    let root = consumer.app.display().to_string();
    let provisioned = bin()
        .args([
            "workflow",
            "credential",
            "provision",
            "--root",
            &root,
            "--credential-id",
            "credential.workflow.one-call-operator",
            "--principal-id",
            "principal.workflow.one-call-operator",
            "--agent-id",
            "agent.workflow.one-call-console",
            "--profile",
            "reviewer",
            "--json",
        ])
        .output()
        .expect("provision one-call credential");
    assert_ok(&provisioned);

    // Simulate the public registry already provisioned by a selected-host
    // adapter. The fixture writes only public metadata; both private keys remain
    // in memory and no Forge command is granted genesis trust authority.
    let broker = StrictHumanBroker::install(&consumer);

    let packet_set = assert_ok(&consumer.run(&["action-packets"]));
    let human_packet = packet_set["data"]["packets"]
        .as_array()
        .expect("action packet list")
        .first()
        .expect("fresh discovery exposes the human intent packet");
    assert_eq!(human_packet["authorization_kind"], "intent_revision");
    let fake_request = consumer.write_json("intent-local-request.json", &serde_json::json!({}));
    let rejected_local_human = bin()
        .args([
            "workflow",
            "credential",
            "sign",
            "--root",
            &root,
            "--credential-id",
            "credential.workflow.one-call-operator",
            "--kind",
            "intent_revision",
            "--request-file",
            &fake_request.display().to_string(),
            "--json",
        ])
        .output()
        .expect("reject local intent signing");
    assert!(!rejected_local_human.status.success());
    let rejected_local_human = String::from_utf8_lossy(&rejected_local_human.stdout);
    assert!(
        rejected_local_human.contains("Reusable attestation signing is intentionally unavailable")
    );
    assert!(!rejected_local_human.contains("credential sign --root"));

    broker.apply(
        &consumer,
        human_packet,
        WorkflowBrokerSemanticInput::IntentRevision {
            desired_outcome: "Exercise the permitted local action lane".to_owned(),
            constraints: Vec::new(),
            preferences: Vec::new(),
            unacceptable_outcomes: Vec::new(),
            uncertainties: Vec::new(),
            conversation_ref: "conversation://workflow/one-call".to_owned(),
            conversation_digest: format!("sha256:{}", "7".repeat(64)),
        },
        "human-intent",
    );

    let packet_set = assert_ok(&consumer.run(&["action-packets"]));

    let packet = packet_set["data"]["packets"]
        .as_array()
        .expect("action packet list")
        .iter()
        .find(|packet| {
            packet["authorization_kind"] == "signal"
                && packet["required_authority"]["approval_boundary"] == "operator_credential_broker"
        })
        .expect("fresh discovery exposes cooperative operator signal packet");
    let packet_digest = packet["packet_digest"]
        .as_str()
        .expect("packet digest")
        .to_owned();
    let signal_active = match packet["input_contract"]["transition"].as_str() {
        Some("activate") => true,
        Some("deactivate") => false,
        other => panic!("unexpected signal transition: {other:?}"),
    };
    let input = consumer.write_json(
        "closed-input.json",
        &serde_json::json!({
            "kind": "signal",
            "active": signal_active,
            "basis_refs": ["README.md"]
        }),
    );
    let input_arg = input.display().to_string();
    let applied = bin()
        .args([
            "workflow",
            "action",
            "authorize",
            "--root",
            &root,
            "--packet-digest",
            &packet_digest,
            "--input-file",
            &input_arg,
            "--credential-id",
            "credential.workflow.one-call-operator",
            "--json",
        ])
        .output()
        .expect("apply local one-call action");
    let receipt = assert_ok(&applied);
    assert_eq!(receipt["command"], "workflow.action.authorize");
    assert_eq!(receipt["data"]["event"]["type"], "signal_changed");
    assert!(
        !consumer.parent.join("request.json").exists()
            && !consumer.parent.join("attestation.json").exists(),
        "one-call action must not materialize request or attestation intermediates"
    );
}

#[test]
// One uninterrupted multiprocess flow keeps every strict broker envelope and
// replacement-agent assertion visibly bound to the preceding CLI output.
#[allow(clippy::too_many_lines)]
fn signed_cli_flow_completes_first_policy_and_resumes_capability_gap() {
    let consumer = Consumer::new();
    let initialized = assert_ok(&consumer.run(&["init", "--readiness-profile", "strict_external"]));
    assert_eq!(initialized["data"]["readiness_profile"], "strict_external");
    assert_eq!(initialized["data"]["current_phase"], "1-discovery");
    let broker = StrictHumanBroker::install(&consumer);
    let packet_set = assert_ok(&consumer.run(&["action-packets"]));
    let intent_packet = action_packet(&packet_set, "intent_revision");
    broker.apply(
        &consumer,
        intent_packet,
        WorkflowBrokerSemanticInput::IntentRevision {
            desired_outcome: "Complete the governed workflow".to_owned(),
            constraints: Vec::new(),
            preferences: Vec::new(),
            unacceptable_outcomes: Vec::new(),
            uncertainties: Vec::new(),
            conversation_ref: "conversation://workflow/signed-cli-flow".to_owned(),
            conversation_digest: format!("sha256:{}", "8".repeat(64)),
        },
        "initial-human-intent",
    );

    let discover = assert_ok(&consumer.run(&["next"]));
    assert_eq!(
        discover["data"]["selected_policy_ref"],
        "policy.workflow.discover-intent"
    );
    let packet_set = assert_ok(&consumer.run(&["action-packets"]));
    let evidence_packet = action_packet(&packet_set, "evidence");
    assert_eq!(
        evidence_packet["input_contract"]["claim_ref"],
        "claim.workflow.discover-intent.intent-grounded"
    );
    broker.apply(
        &consumer,
        evidence_packet,
        WorkflowBrokerSemanticInput::Evidence {
            outcome: WorkflowEvidenceOutcome::Pass,
            subject_kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
            subject_ref: required_str(&discover["data"], "project_id").to_owned(),
            scenario_ref: "README.md".to_owned(),
        },
        "discover-evidence",
    );

    // Every invocation starts a fresh forge-core process. Full guidance
    // equality proves operational recovery rather than digest-only continuity.
    let ready = assert_ok(&consumer.run(&["next"]));
    assert_eq!(ready["data"]["status"], "ready_to_complete");
    let resumed_ready = assert_ok(&consumer.run(&["report"]));
    assert_report_preserves_next(&resumed_ready, &ready, "strict_external");

    let completion_snapshot = required_str(&ready["data"], "snapshot_digest").to_owned();
    assert_ok(&consumer.run(&[
        "complete",
        "--if-snapshot",
        &completion_snapshot,
        "--principal",
        "principal.workflow.replacement-agent",
    ]));

    let applicability = assert_ok(&consumer.run(&["next"]));
    assert_eq!(
        applicability["data"]["selected_policy_ref"],
        "policy.workflow.domain-scan"
    );
    assert_eq!(applicability["data"]["status"], "applicability_required");
    let resumed_applicability = assert_ok(&consumer.run(&["report"]));
    assert_report_preserves_next(&resumed_applicability, &applicability, "strict_external");

    let packet_set = assert_ok(&consumer.run(&["action-packets"]));
    let applicability_packet = action_packet(&packet_set, "applicability");
    assert_eq!(
        applicability_packet["binding"]["policy_ref"],
        "policy.workflow.domain-scan"
    );
    broker.apply(
        &consumer,
        applicability_packet,
        WorkflowBrokerSemanticInput::Applicability {
            applicable: true,
            basis_refs: vec!["README.md".to_owned()],
        },
        "domain-scan-applicability",
    );

    let capability_gap = assert_ok(&consumer.run(&["next"]));
    assert_eq!(
        capability_gap["data"]["simulation"]["candidate_status"],
        "active"
    );
    assert!(
        capability_gap["data"]["simulation"]["candidate_capability_gaps"]
            .as_array()
            .is_some_and(|gaps| gaps.iter().any(|gap| {
                gap["id"] == "capability.workflow.domain-scan.qualified-review"
                    && gap["blocking"] == false
            }))
    );
    assert!(
        capability_gap["data"]["simulation"]["candidate_next_actions"]
            .as_array()
            .is_some_and(|actions| actions
                .iter()
                .any(|action| action["kind"] == "acquire_capability"))
    );
    let resumed_gap = assert_ok(&consumer.run(&["report"]));
    assert_report_preserves_next(&resumed_gap, &capability_gap, "strict_external");
}

#[cfg(unix)]
#[test]
fn workflow_init_ignores_gitignored_nested_build_cache_symlink() {
    let consumer = Consumer::new_with_prefix("forge-workflow-ignored-cache-e2e");
    fs::create_dir_all(consumer.app.join("fuzz")).expect("fuzz directory");
    fs::write(consumer.app.join("fuzz/.gitignore"), "target\n").expect("nested cache ignore rule");

    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&consumer.app)
            .args(args)
            .output()
            .expect("run Git fixture command");
        assert!(
            output.status.success(),
            "git {args:?} failed\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-b", "master"]);
    git(&["add", "README.md", ".forge-method.yaml", "fuzz/.gitignore"]);

    let external_cache = consumer.parent.join("external-fuzz-target");
    fs::create_dir(&external_cache).expect("external cache directory");
    std::os::unix::fs::symlink(&external_cache, consumer.app.join("fuzz/target"))
        .expect("ignored nested cache symlink");
    git(&["check-ignore", "--quiet", "fuzz/target"]);

    assert_ok(&consumer.run(&["init"]));
}
#[test]
fn project_snapshot_digest_excludes_sidecar_ledger_but_tracks_project_changes() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));
    let before = assert_ok(&consumer.run(&["next"]));
    let before_digest = before["data"]["snapshot_digest"]
        .as_str()
        .expect("snapshot")
        .to_owned();

    // A read-only resume and the sidecar WAL do not change project identity.
    let resumed = assert_ok(&consumer.run(&["report"]));
    assert_eq!(resumed["data"]["snapshot_digest"], before_digest);

    fs::write(consumer.app.join("README.md"), "material project change\n").expect("change project");
    let after = assert_ok(&consumer.run(&["next"]));
    assert_ne!(after["data"]["snapshot_digest"], before_digest);
}

#[test]
fn workflow_help_exposes_agent_surface_without_human_workflow_selection() {
    let output = bin()
        .args(["workflow", "--help"])
        .output()
        .expect("workflow help");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains(
        "workflow init [--root <path>] [--readiness-profile <solo_cooperative|strict_external>]"
    ));
    assert!(text.contains("workflow next"));
    assert!(text.contains("workflow resume [--root <path>] [--json|--no-json]"));
    assert!(text.contains("workflow report [--root <path>] [--json|--no-json]"));
    assert!(text.contains("workflow action authorize"));
    assert!(text.contains("workflow action apply"));
    assert!(text.contains("workflow intent record"));
    assert!(text.contains("workflow intent accept-cooperative"));
    assert!(!text.contains("workflow applicability-authorize"));
    assert!(!text.contains("workflow capability-authorize"));
    assert!(!text.contains("workflow evidence-authorize"));
    assert!(!text.contains("workflow observe-artifact"));
    assert!(!text.contains("--principal-registry"));
    assert!(!text.contains("--workflow"));
}

#[test]
fn promotion_preview_is_read_only_and_binds_a_real_linked_worktree_diff() {
    let consumer = Consumer::new();
    fs::write(
        consumer.app.join(".gitignore"),
        ".local/\ngenerated/\ntarget/\n",
    )
    .expect("write promotion ignore rules");
    fs::write(
        consumer.app.join(".gitattributes"),
        "LINE_ENDINGS.txt text eol=lf\nEVIL.txt filter=evil\n",
    )
    .expect("write promotion line-ending rules");
    fs::write(consumer.app.join("LINE_ENDINGS.txt"), "stable\n")
        .expect("write normalized tracked fixture");
    fs::write(consumer.app.join("EVIL.txt"), "stable\n").expect("write filter fixture");
    fs::write(consumer.app.join("DELETE_ME.txt"), "tracked deletion\n")
        .expect("write deletion fixture");
    let run_git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&consumer.app)
            .args(args)
            .output()
            .expect("run Git fixture command");
        assert!(
            output.status.success(),
            "git {:?} failed\nstdout={}\nstderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run_git(&["init", "-b", "master"]);
    run_git(&["config", "user.name", "Forge Promotion E2E"]);
    run_git(&[
        "config",
        "user.email",
        "forge-promotion-e2e@example.invalid",
    ]);
    run_git(&["add", "."]);
    run_git(&["commit", "-m", "initial fixture"]);
    fs::write(
        consumer.app.join(".git").join("info").join("exclude"),
        "info-only.tmp\n",
    )
    .expect("write repository-local ignore");
    #[cfg(unix)]
    let evil_filter_marker = {
        let marker = std::env::temp_dir().join(format!(
            "forge-promotion-filter-marker-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&marker);
        run_git(&[
            "config",
            "filter.evil.clean",
            &format!("touch {} && cat", marker.display()),
        ]);
        marker
    };

    assert_ok(&consumer.run(&["init"]));
    let next = assert_ok(&consumer.run(&["next"]));
    let packet_digest = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("cooperative objective packet")
        .to_owned();
    let objective = consumer.write_json(
        "promotion preview objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Preview isolated work before any canonical mutation",
                "constraints": ["remain host neutral", "derive source from the isolation contract"],
                "unacceptable_outcomes": ["grant apply authority from a preview"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.promotion-e2e",
            "host_provenance": {
                "host_id": "host.promotion-e2e",
                "host_version": "test",
                "session_ref": "session.promotion-e2e",
                "interaction_ref": "turn.promotion-preview",
                "conversation_digest": format!("sha256:{}", "9".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    assert_ok(&run_cooperative_input(
        &consumer,
        &packet_digest,
        &objective,
    ));

    let worktree = consumer.parent.join("wt").join("preview");
    fs::create_dir_all(worktree.parent().expect("worktree parent")).expect("worktree parent");
    let worktree_text = worktree.display().to_string();
    run_git(&[
        "worktree",
        "add",
        "-b",
        "agent/preview",
        &worktree_text,
        "master",
    ]);
    fs::write(
        worktree.join("README.md"),
        "consumer project\npreview change\n",
    )
    .expect("write isolated modification");
    fs::create_dir_all(worktree.join("src")).expect("create isolated source directory");
    fs::write(worktree.join("src/new.rs"), "pub fn preview_only() {}\n")
        .expect("write isolated new file");
    fs::write(worktree.join("LINE_ENDINGS.txt"), b"stable\r\n")
        .expect("write checkout-only line ending");
    fs::create_dir_all(worktree.join(".local")).expect("create ignored local journal directory");
    fs::write(
        worktree.join(".local/journal.md"),
        "must remain outside promotion\n",
    )
    .expect("write ignored local journal");
    fs::create_dir_all(worktree.join("generated")).expect("create ignored generated directory");
    fs::write(
        worktree.join("generated/output.rs"),
        "must remain outside promotion\n",
    )
    .expect("write ignored generated artifact");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let cache = consumer.parent.join("promotion-cache");
        fs::create_dir_all(&cache).expect("create external promotion cache");
        symlink(cache, worktree.join("generated/cache-link"))
            .expect("create ignored promotion cache symlink");
    }
    fs::write(worktree.join("untracked.tmp"), "unclaimed untracked file\n")
        .expect("write unclaimed untracked file");
    fs::write(
        worktree.join("info-only.tmp"),
        "repository-locally ignored\n",
    )
    .expect("write info/exclude fixture");
    fs::write(
        worktree.join("EVIL.txt"),
        "changed without running filter\n",
    )
    .expect("write malicious filter candidate");
    let staged_delete = std::process::Command::new("git")
        .arg("-C")
        .arg(&worktree)
        .args(["rm", "--cached", "DELETE_ME.txt"])
        .output()
        .expect("stage tracked deletion");
    assert!(
        staged_delete.status.success(),
        "stage deletion failed: {}",
        String::from_utf8_lossy(&staged_delete.stderr)
    );
    fs::write(
        worktree.join("DELETE_ME.txt"),
        "untracked replacement must not erase deletion status\n",
    )
    .expect("write deletion replacement");
    fs::create_dir_all(worktree.join("target")).expect("create excluded source cache");
    fs::write(
        worktree.join("target/checked-in.txt"),
        "must not be hidden\n",
    )
    .expect("write excluded source effect");

    let root = consumer.app.display().to_string();
    let now_unix = now().to_string();
    let claim = bin()
        .args([
            "claim",
            "acquire",
            "--root",
            &root,
            "--scope",
            "story",
            "--id",
            "promotion-preview-e2e",
            "--agent",
            "agent.promotion-e2e",
            "--principal-id",
            "principal.agent.promotion-e2e",
            "--path",
            "README.md",
            "--path",
            "src/new.rs",
            "--path",
            "generated/output.rs",
            "--path",
            "info-only.tmp",
            "--path",
            "DELETE_ME.txt",
            "--now-unix",
            &now_unix,
            "--json",
        ])
        .output()
        .expect("acquire promotion preview claim");
    let claim = assert_ok(&claim);
    let claim_id = claim["data"]["claim_id"]
        .as_str()
        .expect("claim id")
        .to_owned();
    let proposed = bin()
        .args([
            "isolation",
            "propose",
            "--root",
            &root,
            "--agent",
            "agent.promotion-e2e",
            "--branch",
            "agent/preview",
            "--worktree-path",
            "../wt/preview",
            "--base-ref",
            "master",
            "--claim",
            &claim_id,
            "--id",
            "isolation.promotion-e2e",
            "--now-unix",
            &now_unix,
            "--json",
        ])
        .output()
        .expect("propose promotion isolation");
    assert_ok(&proposed);
    let activated = bin()
        .args([
            "isolation",
            "transition",
            "--root",
            &root,
            "--id",
            "isolation.promotion-e2e",
            "--to",
            "active",
            "--now-unix",
            &now_unix,
            "--json",
        ])
        .output()
        .expect("activate promotion isolation");
    assert_ok(&activated);

    #[cfg(unix)]
    {
        let refused_filter = bin()
            .args([
                "workflow",
                "promotion",
                "preview",
                "--root",
                &root,
                "--isolation-id",
                "isolation.promotion-e2e",
                "--json",
            ])
            .output()
            .expect("refuse repository-configured filter");
        assert!(!refused_filter.status.success());
        let refused_filter = json(&refused_filter);
        assert!(
            refused_filter.to_string().contains("refuses to execute"),
            "unexpected filter refusal: {refused_filter}"
        );
        assert!(
            !evil_filter_marker.exists(),
            "promotion observation executed a repository-configured clean filter"
        );
        fs::write(worktree.join("EVIL.txt"), "stable\n").expect("restore filter fixture");
    }

    let canonical_before = state_tree_snapshot(&consumer.app);
    let forge_state_before = state_tree_snapshot(&consumer.state);
    let preview_output = bin()
        .args([
            "workflow",
            "promotion",
            "preview",
            "--root",
            &root,
            "--isolation-id",
            "isolation.promotion-e2e",
            "--json",
        ])
        .output()
        .expect("run governed promotion preview");
    let preview = assert_ok(&preview_output);
    assert_eq!(preview["command"], "workflow.promotion.preview");
    assert_eq!(
        preview["data"]["authority"],
        "read_only_candidate_no_apply_authority"
    );
    assert_eq!(preview["data"]["canonical_mutation_performed"], false);
    assert_eq!(preview["data"]["forge_state_mutation_performed"], false);
    assert_eq!(preview["data"]["status"], "blocked");
    assert_eq!(
        preview["data"]["source"]["git_worktree"]["branch_ref"],
        "refs/heads/agent/preview"
    );
    assert_eq!(
        preview["data"]["source"]["declared_worktree_path"],
        "../wt/preview"
    );

    let diff = preview["data"]["diff"]
        .as_array()
        .expect("promotion diff array");
    let diff_paths = diff
        .iter()
        .map(|entry| entry["path"].as_str().expect("diff path"))
        .collect::<Vec<_>>();
    assert_eq!(
        diff_paths,
        vec!["DELETE_ME.txt", "README.md", "src/new.rs"],
        "only Git-tracked changes and exactly claimed new files are promotable"
    );
    assert_eq!(
        diff[0]["effect"], "delete_regular_file",
        "a staged deletion must win over an untracked same-name replacement"
    );
    assert_eq!(diff[1]["effect"], "write_regular_file");
    assert_eq!(diff[2]["effect"], "create_regular_file");
    let unsupported = preview["data"]["unsupported_effects"]
        .as_array()
        .expect("typed unsupported effects");
    assert!(
        !unsupported
            .iter()
            .any(|effect| effect["kind"] == "excluded_source_root_content"),
        "an unclaimed cache must stay outside the preview instead of blocking it"
    );
    let gaps = preview["data"]["unresolved_gaps"]
        .as_array()
        .expect("typed unresolved gaps");
    assert!(!gaps
        .iter()
        .any(|gap| gap["code"] == "missing_linked_isolation_claim"));
    assert!(!gaps.iter().any(|gap| gap["code"] == "ungoverned_write_set"));

    assert_eq!(state_tree_snapshot(&consumer.app), canonical_before);
    assert_eq!(state_tree_snapshot(&consumer.state), forge_state_before);

    let repeated_output = bin()
        .args([
            "workflow",
            "promotion",
            "preview",
            "--root",
            &root,
            "--isolation-id",
            "isolation.promotion-e2e",
            "--json",
        ])
        .output()
        .expect("repeat governed promotion preview");
    let repeated = assert_ok(&repeated_output);
    assert_eq!(
        repeated["data"]["preview_digest"], preview["data"]["preview_digest"],
        "the same retained state must reproduce one stable preview identity"
    );
    assert_eq!(state_tree_snapshot(&consumer.app), canonical_before);
    assert_eq!(state_tree_snapshot(&consumer.state), forge_state_before);

    fs::write(
        consumer.app.join("CANONICAL_ADVANCE.txt"),
        "canonical branch advanced\n",
    )
    .expect("advance canonical fixture");
    run_git(&["add", "CANONICAL_ADVANCE.txt"]);
    run_git(&["commit", "-m", "advance canonical fixture"]);
    let diverged_before = state_tree_snapshot(&consumer.app);
    let state_before_diverged_preview = state_tree_snapshot(&consumer.state);
    let diverged = bin()
        .args([
            "workflow",
            "promotion",
            "preview",
            "--root",
            &root,
            "--isolation-id",
            "isolation.promotion-e2e",
            "--json",
        ])
        .output()
        .expect("reject isolation behind canonical head");
    assert!(!diverged.status.success());
    let diverged = json(&diverged);
    assert!(
        diverged.to_string().contains("not an ancestor"),
        "unexpected canonical-advance refusal: {diverged}"
    );
    assert_eq!(state_tree_snapshot(&consumer.app), diverged_before);
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        state_before_diverged_preview
    );

    run_git(&["worktree", "remove", "--force", &worktree_text]);
    let state_before_resume = state_tree_snapshot(&consumer.state);
    let replacement = assert_ok(&consumer.run(&["report"]));
    let continuity = &replacement["data"]["replacement_continuity"];
    assert_eq!(continuity["status"], "blocked");
    assert!(continuity["gaps"].as_array().is_some_and(|gaps| {
        gaps.iter().any(|gap| {
            gap["code"] == "worktree_missing" && gap["isolation_id"] == "isolation.promotion-e2e"
        })
    }));
    assert_eq!(
        continuity["isolations"][0]["validation"], "missing",
        "resume reports the missing real worktree instead of inventing continuation"
    );
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        state_before_resume,
        "read-only resume must not recreate or repair the missing worktree"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn promotion_apply_writes_once_reads_back_and_exact_retry_is_idempotent() {
    let consumer = Consumer::new();
    fs::write(consumer.app.join(".gitignore"), ".local/\ntarget/\n")
        .expect("write apply ignore rules");
    fs::write(
        consumer.app.join(".gitattributes"),
        "LINE_ENDINGS.txt text eol=lf\n",
    )
    .expect("write apply line-ending rules");
    fs::write(consumer.app.join("LINE_ENDINGS.txt"), "stable\n")
        .expect("write apply normalized fixture");
    let run_git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&consumer.app)
            .args(args)
            .output()
            .expect("run Git fixture command");
        assert!(
            output.status.success(),
            "git {:?} failed\nstdout={}\nstderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run_git(&["init", "-b", "master"]);
    run_git(&["config", "user.name", "Forge Promotion Apply E2E"]);
    run_git(&[
        "config",
        "user.email",
        "forge-promotion-apply-e2e@example.invalid",
    ]);
    run_git(&["add", "."]);
    run_git(&["commit", "-m", "initial fixture"]);

    assert_ok(&consumer.run(&["init"]));
    forge_core_store::replay_wal::initialize_replay_wal(&consumer.state)
        .expect("initialize project replay WAL required by promotion apply");
    let next = assert_ok(&consumer.run(&["next"]));
    let packet_digest = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("cooperative objective packet")
        .to_owned();
    let objective = consumer.write_json(
        "promotion apply objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Apply one exact isolated regular-file write",
                "constraints": ["retain exact claim authority", "read back canonical bytes"],
                "unacceptable_outcomes": ["copy files outside governed apply"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.promotion-apply-e2e",
            "host_provenance": {
                "host_id": "host.promotion-apply-e2e",
                "host_version": "test",
                "session_ref": "session.promotion-apply-e2e",
                "interaction_ref": "turn.promotion-apply",
                "conversation_digest": format!("sha256:{}", "e".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    assert_ok(&run_cooperative_input(
        &consumer,
        &packet_digest,
        &objective,
    ));
    let root = consumer.app.display().to_string();
    let now_unix = now().to_string();
    let claim = bin()
        .args([
            "claim",
            "acquire",
            "--root",
            &root,
            "--scope",
            "story",
            "--id",
            "promotion-apply-e2e",
            "--agent",
            "agent.promotion-apply-e2e",
            "--principal-id",
            "principal.agent.promotion-apply-e2e",
            "--path",
            "README.md",
            "--now-unix",
            &now_unix,
            "--json",
        ])
        .output()
        .expect("acquire promotion claim");
    let claim = assert_ok(&claim);
    let claim_id = claim["data"]["claim_id"]
        .as_str()
        .expect("claim id")
        .to_owned();

    let worktree = consumer.parent.join("wt").join("apply");
    fs::create_dir_all(worktree.parent().expect("worktree parent")).expect("worktree parent");
    let worktree_text = worktree.display().to_string();
    run_git(&[
        "worktree",
        "add",
        "-b",
        "agent/apply",
        &worktree_text,
        "master",
    ]);
    fs::write(
        worktree.join("README.md"),
        "consumer project\ngoverned apply\n",
    )
    .expect("write isolated modification");
    fs::write(worktree.join("LINE_ENDINGS.txt"), b"stable\r\n")
        .expect("write apply checkout-only line ending");
    fs::create_dir_all(worktree.join(".local")).expect("create ignored apply journal");
    fs::write(worktree.join(".local/journal.md"), "must not be promoted\n")
        .expect("write ignored apply journal");
    fs::write(worktree.join("untracked.tmp"), "must not be promoted\n")
        .expect("write unclaimed apply artifact");
    assert_ok(
        &bin()
            .args([
                "isolation",
                "propose",
                "--root",
                &root,
                "--agent",
                "agent.promotion-apply-e2e",
                "--branch",
                "agent/apply",
                "--worktree-path",
                "../wt/apply",
                "--base-ref",
                "master",
                "--claim",
                &claim_id,
                "--id",
                "isolation.promotion-apply-e2e",
                "--now-unix",
                &now_unix,
                "--json",
            ])
            .output()
            .expect("propose promotion isolation"),
    );
    assert_ok(
        &bin()
            .args([
                "isolation",
                "transition",
                "--root",
                &root,
                "--id",
                "isolation.promotion-apply-e2e",
                "--to",
                "active",
                "--now-unix",
                &now_unix,
                "--json",
            ])
            .output()
            .expect("activate promotion isolation"),
    );

    // Admit freshness-bound cooperative evidence immediately before preview;
    // claim/Git/isolation fixture setup must not consume its short live window.
    let next = advance_to_promotion_evidence_packet(
        &consumer,
        assert_ok(&consumer.run(&["next"])),
        "apply",
    );
    let packet = next["data"]["cooperative_evidence_action_packet"].clone();
    assert!(
        packet["input_file_token"].is_string(),
        "promotion apply requires an executable cooperative evidence packet: {next}"
    );
    let mut offer = packet["offer_template"].clone();
    offer["offer_id"] = serde_json::json!("offer.promotion-apply-e2e.pass");
    let offer_file = consumer.write_json("promotion apply evidence.json", &offer);
    assert_ok(&execute_cooperative_packet(&packet, &offer_file));

    let preview = assert_ok(
        &bin()
            .args([
                "workflow",
                "promotion",
                "preview",
                "--root",
                &root,
                "--isolation-id",
                "isolation.promotion-apply-e2e",
                "--json",
            ])
            .output()
            .expect("preview promotion apply"),
    );
    assert_eq!(
        preview["data"]["status"], "reviewable",
        "promotion preview: {preview}"
    );
    assert_eq!(
        preview["data"]["apply_eligibility"], "eligible_local_reversible",
        "promotion preview: {preview}"
    );
    assert_eq!(
        preview["data"]["write_set"],
        serde_json::json!(["README.md"]),
        "checkout normalization and unclaimed files must stay outside apply"
    );
    assert!(!preview["data"]["carried_assurance_gaps"]
        .as_array()
        .expect("carried assurance gaps")
        .is_empty());
    let preview_digest = preview["data"]["preview_digest"]
        .as_str()
        .expect("preview digest")
        .to_owned();
    let applied = assert_ok(
        &bin()
            .args([
                "workflow",
                "promotion",
                "apply",
                "--root",
                &root,
                "--isolation-id",
                "isolation.promotion-apply-e2e",
                "--expected-preview-digest",
                &preview_digest,
                "--json",
            ])
            .output()
            .expect("apply governed promotion"),
    );
    assert_eq!(applied["data"]["status"], "applied");
    assert_eq!(applied["data"]["canonical_mutation_performed"], true);
    assert_eq!(applied["data"]["receipt"]["readback_verified"], true);
    assert_eq!(
        fs::read_to_string(consumer.app.join("README.md")).expect("canonical readback"),
        "consumer project\ngoverned apply\n"
    );
    assert_eq!(
        fs::read(consumer.app.join("LINE_ENDINGS.txt")).expect("line ending readback"),
        b"stable\n",
        "Git-clean checkout line endings must not overwrite canonical bytes"
    );
    assert!(!consumer.app.join(".local/journal.md").exists());
    assert!(!consumer.app.join("untracked.tmp").exists());
    assert!(!consumer.app.join(".forge-method").exists());

    let retry = assert_ok(
        &bin()
            .args([
                "workflow",
                "promotion",
                "apply",
                "--root",
                &root,
                "--isolation-id",
                "isolation.promotion-apply-e2e",
                "--expected-preview-digest",
                &preview_digest,
                "--json",
            ])
            .output()
            .expect("retry governed promotion"),
    );
    assert_eq!(retry["data"]["status"], "already_committed");
    assert_eq!(retry["data"]["canonical_mutation_performed"], false);
    assert_eq!(
        retry["data"]["receipt"]["receipt_digest"],
        applied["data"]["receipt"]["receipt_digest"]
    );
    assert_ok(
        &bin()
            .args([
                "claim",
                "release",
                "--root",
                &root,
                "--id",
                &claim_id,
                "--agent",
                "agent.promotion-apply-e2e",
                "--now-unix",
                &now_unix,
                "--json",
            ])
            .output()
            .expect("release completed promotion claim"),
    );
    let replacement = assert_ok(&consumer.run(&["report"]));
    assert_eq!(
        replacement["data"]["replacement_continuity"]["status"], "ready",
        "a stale claim linked only to a completed promotion must not block new work"
    );
    let completed_claim_gap = replacement["data"]["replacement_continuity"]["gaps"]
        .as_array()
        .expect("continuity gaps")
        .iter()
        .find(|gap| {
            gap["isolation_id"] == "isolation.promotion-apply-e2e"
                && gap["code"] == "linked_claim_inactive"
        })
        .expect("completed promotion retains non-blocking claim history");
    assert_eq!(completed_claim_gap["blocking"], false);
    let completed = replacement["data"]["replacement_continuity"]["promotions"]
        .as_array()
        .expect("promotion continuity")
        .iter()
        .find(|promotion| promotion["preview_digest"] == preview_digest)
        .expect("completed promotion");
    assert_eq!(completed["status"], "completed");
    assert!(completed["recovery_argv"]
        .as_array()
        .is_none_or(Vec::is_empty));
    assert!(
        !replacement["data"]["replacement_continuity"]["ranked_next_actions"]
            .as_array()
            .expect("ranked actions")
            .iter()
            .any(|action| action["kind"] == "recover_promotion")
    );
    fs::write(
        consumer.app.join("AFTER_PROMOTION.txt"),
        "later independent project change\n",
    )
    .expect("later project change");
    let after_drift = assert_ok(&consumer.run(&["report"]));
    let historical = after_drift["data"]["replacement_continuity"]["promotions"]
        .as_array()
        .expect("historical promotion continuity")
        .iter()
        .find(|promotion| promotion["preview_digest"] == preview_digest)
        .expect("historical receipt remains visible");
    assert_eq!(
        historical["status"], "completed",
        "a valid historical receipt must not become corrupt merely because the project changed later"
    );
    fs::remove_file(consumer.app.join("AFTER_PROMOTION.txt")).expect("restore project fixture");

    let receipt_name = format!(
        "{}.json",
        preview_digest
            .strip_prefix("sha256:")
            .expect("canonical preview digest")
    );
    let receipt_path = consumer
        .state
        .join("promotion")
        .join("receipts")
        .join(receipt_name);
    let original_receipt_bytes = fs::read(&receipt_path).expect("persisted promotion receipt");
    let mut tampered_receipt: GovernedPromotionReceipt =
        serde_json::from_slice(&original_receipt_bytes).expect("typed promotion receipt");
    tampered_receipt.provenance_digest = format!("sha256:{}", "f".repeat(64));
    tampered_receipt.receipt_digest.clear();
    tampered_receipt.receipt_digest =
        forge_core_decisions::promotion_domain_digest("promotion.receipt.v1", &tampered_receipt)
            .expect("self-consistent tampered receipt digest");
    fs::write(
        &receipt_path,
        serde_json_canonicalizer::to_vec(&tampered_receipt).expect("canonical tampered receipt"),
    )
    .expect("write tampered receipt");
    let state_before_tampered_resume = state_tree_snapshot(&consumer.state);
    let tampered_resume = assert_ok(&consumer.run(&["report"]));
    assert_eq!(
        tampered_resume["data"]["replacement_continuity"]["status"],
        "blocked"
    );
    assert!(tampered_resume["data"]["replacement_continuity"]["gaps"]
        .as_array()
        .is_some_and(|gaps| gaps
            .iter()
            .any(|gap| gap["code"] == "promotion_state_invalid")));
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        state_before_tampered_resume,
        "resume must not repair a tampered receipt"
    );
    let tampered_retry = bin()
        .args([
            "workflow",
            "promotion",
            "apply",
            "--root",
            &root,
            "--isolation-id",
            "isolation.promotion-apply-e2e",
            "--expected-preview-digest",
            &preview_digest,
            "--json",
        ])
        .output()
        .expect("retry with self-consistent provenance tamper");
    assert!(!tampered_retry.status.success());
    let tampered_retry = json(&tampered_retry);
    assert!(tampered_retry["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("execution provenance digest differs")));
    fs::write(&receipt_path, &original_receipt_bytes).expect("restore exact receipt bytes");

    let replay = forge_core_store::replay_wal::recover_replay_wal(&consumer.state, false)
        .expect("clean committed replay WAL");
    let consume = replay
        .records
        .iter()
        .rev()
        .find(|record| {
            record.operation == forge_core_store::replay_wal::ReplayWalOperation::Consume
        })
        .expect("durable replay consume record");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&replay.wal_path)
        .expect("open replay WAL for crash fixture")
        .set_len(consume.offset)
        .expect("truncate exact consume frame");
    let state_before_replay_resume = state_tree_snapshot(&consumer.state);
    let replay_resume = assert_ok(&consumer.run(&["report"]));
    assert_eq!(
        replay_resume["data"]["replacement_continuity"]["status"],
        "blocked"
    );
    assert!(replay_resume["data"]["replacement_continuity"]["gaps"]
        .as_array()
        .is_some_and(|gaps| gaps
            .iter()
            .any(|gap| gap["code"] == "promotion_state_invalid")));
    assert_eq!(
        state_tree_snapshot(&consumer.state),
        state_before_replay_resume,
        "resume must not consume or repair a truncated replay record"
    );
    let unconsumed_retry = bin()
        .args([
            "workflow",
            "promotion",
            "apply",
            "--root",
            &root,
            "--isolation-id",
            "isolation.promotion-apply-e2e",
            "--expected-preview-digest",
            &preview_digest,
            "--json",
        ])
        .output()
        .expect("retry with receipt but no durable replay consume");
    assert!(!unconsumed_retry.status.success());
    let unconsumed_retry = json(&unconsumed_retry);
    assert_eq!(
        unconsumed_retry["typed_failure"]["type"],
        "recovery_required"
    );
    assert!(unconsumed_retry["typed_failure"]["data"]["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("not durably consumed")));
}

struct PromotionRecoveryFixture {
    consumer: Consumer,
    root: String,
    claim_id: String,
    preview_digest: String,
}

impl PromotionRecoveryFixture {
    #[allow(clippy::too_many_lines)]
    fn new() -> Self {
        let consumer = Consumer::new_with_prefix("forge workflow promotion recovery e2e");
        fs::write(consumer.app.join("NOTES.md"), "old notes\n").expect("second canonical file");
        let run_git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(&consumer.app)
                .args(args)
                .output()
                .expect("run Git recovery fixture command");
            assert!(
                output.status.success(),
                "git {:?} failed\nstdout={}\nstderr={}",
                args,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        };
        run_git(&["init", "-b", "master"]);
        run_git(&["config", "user.name", "Forge Promotion Recovery E2E"]);
        run_git(&[
            "config",
            "user.email",
            "forge-promotion-recovery-e2e@example.invalid",
        ]);
        run_git(&["add", "."]);
        run_git(&["commit", "-m", "initial recovery fixture"]);
        assert_ok(&consumer.run(&["init"]));
        forge_core_store::replay_wal::initialize_replay_wal(&consumer.state)
            .expect("initialize recovery replay WAL");
        let next = assert_ok(&consumer.run(&["next"]));
        let packet_digest = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
            .as_str()
            .expect("recovery objective packet")
            .to_owned();
        let objective = consumer.write_json(
            "promotion recovery objective.json",
            &serde_json::json!({
                "kind": "unambiguous",
                "proposal": {
                    "outcome": "Recover one exact interrupted two-file promotion",
                    "constraints": ["retain durable intent", "read back canonical bytes"],
                    "unacceptable_outcomes": ["apply third-party bytes", "duplicate committed effects"],
                    "open_uncertainties": []
                },
                "carrying_principal": "principal.agent.promotion-recovery-e2e",
                "host_provenance": {
                    "host_id": "host.promotion-recovery-e2e",
                    "host_version": "test",
                    "session_ref": "session.promotion-recovery-e2e",
                    "interaction_ref": "turn.promotion-recovery",
                    "conversation_digest": format!("sha256:{}", "d".repeat(64)),
                    "observed_at_unix": 1
                }
            }),
        );
        assert_ok(&run_cooperative_input(
            &consumer,
            &packet_digest,
            &objective,
        ));
        let root = consumer.app.display().to_string();
        let now_unix = now().to_string();
        let claim = bin()
            .args([
                "claim",
                "acquire",
                "--root",
                &root,
                "--scope",
                "story",
                "--id",
                "promotion-recovery-e2e",
                "--agent",
                "agent.promotion-recovery-e2e",
                "--principal-id",
                "principal.agent.promotion-recovery-e2e",
                "--path",
                "README.md",
                "--path",
                "NOTES.md",
                "--now-unix",
                &now_unix,
                "--json",
            ])
            .output()
            .expect("acquire promotion recovery claim");
        let claim = assert_ok(&claim);
        let claim_id = claim["data"]["claim_id"]
            .as_str()
            .expect("recovery claim id")
            .to_owned();
        let worktree = consumer.parent.join("wt").join("recover");
        fs::create_dir_all(worktree.parent().expect("recovery worktree parent"))
            .expect("recovery worktree parent");
        let worktree_text = worktree.display().to_string();
        run_git(&[
            "worktree",
            "add",
            "-b",
            "agent/recover",
            &worktree_text,
            "master",
        ]);
        fs::write(
            worktree.join("README.md"),
            "consumer project\nrecovered readme\n",
        )
        .expect("write recovery README");
        fs::write(worktree.join("NOTES.md"), "recovered notes\n").expect("write recovery NOTES");
        assert_ok(
            &bin()
                .args([
                    "isolation",
                    "propose",
                    "--root",
                    &root,
                    "--agent",
                    "agent.promotion-recovery-e2e",
                    "--branch",
                    "agent/recover",
                    "--worktree-path",
                    "../wt/recover",
                    "--base-ref",
                    "master",
                    "--claim",
                    &claim_id,
                    "--id",
                    "isolation.promotion-recovery-e2e",
                    "--now-unix",
                    &now_unix,
                    "--json",
                ])
                .output()
                .expect("propose recovery isolation"),
        );
        assert_ok(
            &bin()
                .args([
                    "isolation",
                    "transition",
                    "--root",
                    &root,
                    "--id",
                    "isolation.promotion-recovery-e2e",
                    "--to",
                    "active",
                    "--now-unix",
                    &now_unix,
                    "--json",
                ])
                .output()
                .expect("activate recovery isolation"),
        );
        let next = advance_to_promotion_evidence_packet(
            &consumer,
            assert_ok(&consumer.run(&["next"])),
            "recovery",
        );
        let packet = next["data"]["cooperative_evidence_action_packet"].clone();
        let mut offer = packet["offer_template"].clone();
        offer["offer_id"] = serde_json::json!("offer.promotion-recovery-e2e.pass");
        let offer_file = consumer.write_json("promotion recovery evidence.json", &offer);
        assert_ok(&execute_cooperative_packet(&packet, &offer_file));
        let preview = assert_ok(
            &bin()
                .args([
                    "workflow",
                    "promotion",
                    "preview",
                    "--root",
                    &root,
                    "--isolation-id",
                    "isolation.promotion-recovery-e2e",
                    "--json",
                ])
                .output()
                .expect("preview recovery promotion"),
        );
        assert_eq!(preview["data"]["status"], "reviewable", "{preview}");
        assert_eq!(
            preview["data"]["diff"].as_array().map(Vec::len),
            Some(2),
            "recovery fixture must exercise two files"
        );
        let preview_digest = preview["data"]["preview_digest"]
            .as_str()
            .expect("recovery preview digest")
            .to_owned();
        Self {
            consumer,
            root,
            claim_id,
            preview_digest,
        }
    }

    fn command(&self, action: &str) -> Command {
        let mut command = bin();
        command.args([
            "workflow",
            "promotion",
            action,
            "--root",
            &self.root,
            "--isolation-id",
            "isolation.promotion-recovery-e2e",
            "--expected-preview-digest",
            &self.preview_digest,
            "--json",
        ]);
        command
    }

    fn intent_path(&self) -> PathBuf {
        self.consumer
            .state
            .join("promotion")
            .join("intents")
            .join(format!(
                "{}.json",
                self.preview_digest
                    .strip_prefix("sha256:")
                    .expect("preview sha256")
            ))
    }

    fn effect_wal_path(&self) -> PathBuf {
        self.consumer.state.join("promotion").join("effects.ndjson")
    }
}

#[test]
fn promotion_apply_failure_supplies_one_safe_recovery_argv_and_recover_failure_stops() {
    let fixture = PromotionRecoveryFixture::new();
    assert!(
        fixture.root.contains(' '),
        "fixture must prove a path containing spaces"
    );
    let crashed = fixture
        .command("apply")
        .env("FORGE_TEST_PROMOTION_CRASH_AT", "after_begin")
        .output()
        .expect("leave incomplete promotion");
    assert_eq!(crashed.status.code(), Some(86));

    let retry = fixture
        .command("apply")
        .output()
        .expect("inspect apply retry guidance");
    assert!(!retry.status.success());
    let retry = json(&retry);
    assert_eq!(retry["typed_failure"]["type"], "recovery_required");
    assert_eq!(retry["typed_failure"]["data"]["can_recover"], true);
    let argv = retry["typed_failure"]["data"]["recovery_argv"]
        .as_array()
        .expect("structured recovery argv");
    assert_eq!(argv[3], "recover");
    assert_eq!(argv[5], fixture.root);
    assert_eq!(argv[7], "isolation.promotion-recovery-e2e");
    assert_eq!(argv[9], fixture.preview_digest);
    assert_eq!(argv[10], "--json");

    fs::write(
        fixture.consumer.app.join("README.md"),
        "third content blocks recovery\n",
    )
    .expect("inject ambiguous destination");
    let stopped = fixture
        .command("recover")
        .output()
        .expect("recover must stop");
    assert!(!stopped.status.success());
    let stopped = json(&stopped);
    assert_eq!(stopped["typed_failure"]["type"], "recovery_required");
    assert_eq!(stopped["typed_failure"]["data"]["can_recover"], false);
    assert!(stopped["typed_failure"]["data"]
        .get("recovery_argv")
        .is_none());
}

#[test]
fn promotion_recover_executes_a_real_legacy_v1_pre_begin_intent_honestly() {
    let fixture = PromotionRecoveryFixture::new();
    let crashed = fixture
        .command("apply")
        .env("FORGE_TEST_PROMOTION_CRASH_AT", "after_intent")
        .output()
        .expect("leave v2 intent before replay/effect begin");
    assert_eq!(crashed.status.code(), Some(86));

    let intent_path = fixture.intent_path();
    let mut intent: Value =
        serde_json::from_slice(&fs::read(&intent_path).expect("read durable intent"))
            .expect("parse durable intent");
    intent["schema_version"] = serde_json::json!("governed_promotion_intent_v1");
    intent
        .as_object_mut()
        .expect("intent object")
        .remove("preview");
    intent["replay"]["intent_digest"] = serde_json::json!("");
    let v1_intent_digest =
        forge_core_decisions::promotion_domain_digest("promotion.intent.v1", &intent)
            .expect("derive exact v1 wire digest");
    intent["replay"]["intent_digest"] = serde_json::json!(v1_intent_digest);
    assert!(
        intent.get("preview").is_none(),
        "v1 wire fixture must not retain a historical preview"
    );
    fs::write(
        &intent_path,
        serde_json_canonicalizer::to_vec(&intent).expect("canonical v1 intent"),
    )
    .expect("install exact v1 intent fixture");

    let state_before_resume = state_tree_snapshot(&fixture.consumer.state);
    let resumed = assert_ok(&fixture.consumer.run(&["report"]));
    let legacy = resumed["data"]["replacement_continuity"]["promotions"]
        .as_array()
        .expect("legacy promotion continuity")
        .iter()
        .find(|promotion| promotion["preview_digest"] == fixture.preview_digest)
        .expect("legacy v1 promotion must be reconstructed");
    assert_eq!(legacy["status"], "recoverable");
    assert_eq!(
        legacy["recovery_argv"],
        serde_json::json!([
            "forge-core",
            "workflow",
            "promotion",
            "recover",
            "--root",
            fixture.root.clone(),
            "--isolation-id",
            "isolation.promotion-recovery-e2e",
            "--expected-preview-digest",
            fixture.preview_digest.clone(),
            "--json"
        ])
    );
    assert_eq!(
        state_tree_snapshot(&fixture.consumer.state),
        state_before_resume,
        "legacy v1 resume must reconstruct guidance without creating replay or effect state"
    );

    let recovered = assert_ok(
        &fixture
            .command("recover")
            .output()
            .expect("recover legacy v1 intent"),
    );
    assert_eq!(recovered["data"]["status"], "recovered");
    assert_eq!(
        recovered["data"]["receipt"]["recovery_execution"]["recovery_kind"],
        "legacy_v1_pre_begin_fresh_execution_v1"
    );
    assert_eq!(
        recovered["data"]["receipt"]["recovery_execution"]["durable_intent_digest"],
        recovered["data"]["receipt"]["replay"]["intent_digest"]
    );
    assert_eq!(
        recovered["data"]["receipt"]["replay"]["intent_digest"],
        intent["replay"]["intent_digest"]
    );
    let repeated = assert_ok(
        &fixture
            .command("recover")
            .output()
            .expect("verify legacy receipt retry"),
    );
    assert_eq!(repeated["data"]["status"], "already_committed");
    assert_eq!(repeated["data"]["canonical_mutation_performed"], false);
}

#[test]
fn promotion_recover_blocks_bad_replay_authority_before_any_recovery_write() {
    for replay_case in ["missing", "consumed", "mismatched"] {
        let fixture = PromotionRecoveryFixture::new();
        let crashed = fixture
            .command("apply")
            .env("FORGE_TEST_PROMOTION_CRASH_AT", "after_begin")
            .output()
            .expect("leave effect begin with reserved replay");
        assert_eq!(crashed.status.code(), Some(86));

        let intent: Value =
            serde_json::from_slice(&fs::read(fixture.intent_path()).expect("read intent"))
                .expect("parse intent");
        let principal = PrincipalId(
            intent["principal_id"]
                .as_str()
                .expect("intent principal")
                .to_owned(),
        );
        let intent_digest = intent["replay"]["intent_digest"]
            .as_str()
            .expect("intent digest")
            .to_owned();
        let commit_digest = intent["replay"]["commit_digest"]
            .as_str()
            .expect("commit digest")
            .to_owned();
        let replay_path = forge_core_store::replay_wal::replay_wal_path(&fixture.consumer.state);
        match replay_case {
            "missing" => {
                fs::OpenOptions::new()
                    .write(true)
                    .open(&replay_path)
                    .expect("open replay WAL")
                    .set_len(0)
                    .expect("remove reservation");
            }
            "consumed" => {
                forge_core_store::replay_wal::consume_replay_nonce_non_boundary(
                    &fixture.consumer.state,
                    &principal,
                    "forge.workflow.promotion.apply.local-reversible.v1",
                    &fixture.preview_digest,
                    &intent_digest,
                    &commit_digest,
                    1,
                )
                .expect("consume replay before effect commit");
            }
            "mismatched" => {
                fs::OpenOptions::new()
                    .write(true)
                    .open(&replay_path)
                    .expect("open replay WAL")
                    .set_len(0)
                    .expect("remove exact reservation");
                forge_core_store::replay_wal::reserve_replay_nonce(
                    &fixture.consumer.state,
                    &principal,
                    "forge.workflow.promotion.apply.local-reversible.v1",
                    &fixture.preview_digest,
                    &format!("sha256:{}", "e".repeat(64)),
                    &commit_digest,
                )
                .expect("install mismatched valid reservation");
            }
            _ => unreachable!(),
        }

        let canonical_before = state_tree_snapshot(&fixture.consumer.app);
        let state_before = state_tree_snapshot(&fixture.consumer.state);
        let effect_wal_before =
            fs::read(fixture.effect_wal_path()).expect("read effect WAL before recovery");
        let refused = fixture
            .command("recover")
            .output()
            .expect("attempt recovery with invalid replay authority");
        assert!(!refused.status.success(), "{replay_case}");
        let refused = json(&refused);
        assert_eq!(refused["typed_failure"]["type"], "recovery_required");
        assert_eq!(refused["typed_failure"]["data"]["can_recover"], false);
        assert!(refused["typed_failure"]["data"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("could not be retained before recovery")));
        assert_eq!(
            state_tree_snapshot(&fixture.consumer.app),
            canonical_before,
            "{replay_case}: canonical bytes changed"
        );
        assert_eq!(
            fs::read(fixture.effect_wal_path()).expect("read effect WAL after refusal"),
            effect_wal_before,
            "{replay_case}: effect WAL changed"
        );
        assert_eq!(
            state_tree_snapshot(&fixture.consumer.state),
            state_before,
            "{replay_case}: Forge state changed"
        );
    }
}

#[test]
fn promotion_recover_rejects_semantically_corrupt_effect_wal_without_writing() {
    const CORRUPTIONS: &[&str] = &[
        "wrong_schema",
        "unexpected_terminal_ref",
        "extra_progress_record",
        "unknown_target",
        "physical_target",
        "operation",
        "actor",
        "role",
        "destructive",
        "content",
    ];
    let fixture = PromotionRecoveryFixture::new();
    let crashed = fixture
        .command("apply")
        .env("FORGE_TEST_PROMOTION_CRASH_AT", "after_commit")
        .output()
        .expect("leave committed effect before replay consume");
    assert_eq!(crashed.status.code(), Some(86));
    let wal_path = fixture.effect_wal_path();
    let original_wal = fs::read(&wal_path).expect("read valid effect WAL");

    for corruption in CORRUPTIONS {
        let mut records = String::from_utf8(original_wal.clone())
            .expect("UTF-8 WAL")
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("WAL record"))
            .collect::<Vec<_>>();
        let begin_index = records
            .iter()
            .position(|record| record["stage"] == "begin")
            .expect("begin record");
        let before_index = records
            .iter()
            .position(|record| record["stage"] == "before_image")
            .expect("before-image record");
        let write_index = records
            .iter()
            .position(|record| record["stage"] == "write_applied")
            .expect("write record");
        let commit_index = records
            .iter()
            .position(|record| record["stage"] == "commit")
            .expect("commit record");
        match *corruption {
            "wrong_schema" => {
                records[begin_index]["schema_version"] = serde_json::json!("9.9");
            }
            "unexpected_terminal_ref" => {
                records[commit_index]["target_ref"] = serde_json::json!("README.md");
            }
            "extra_progress_record" => {
                records.push(records[before_index].clone());
            }
            "unknown_target" => {
                records[write_index]["target_ref"] = serde_json::json!("EXTRA.md");
                records[write_index]["physical_target_ref"] = serde_json::json!("EXTRA.md");
            }
            "physical_target" => {
                records[write_index]["physical_target_ref"] = serde_json::json!("OTHER.md");
            }
            "operation" => {
                records[write_index]["target_metadata"]["operation_id"] =
                    serde_json::json!("objective.not-the-approved-operation");
            }
            "actor" => {
                records[write_index]["target_metadata"]["actor_agent_id"] =
                    serde_json::json!("agent.not-the-isolation-owner");
            }
            "role" => {
                records[write_index]["target_metadata"]["actor_role"] = serde_json::json!("human");
            }
            "destructive" => {
                records[write_index]["target_metadata"]["destructive"] = serde_json::json!(true);
            }
            "content" => {
                records[write_index]["target_metadata"]["content_hash"] =
                    serde_json::json!(format!("sha256:{}", "f".repeat(64)));
            }
            _ => unreachable!(),
        }
        let mut corrupted_wal = Vec::new();
        for record in &records {
            corrupted_wal.extend(serde_json::to_vec(record).expect("serialize corrupt record"));
            corrupted_wal.push(b'\n');
        }
        fs::write(&wal_path, corrupted_wal).expect("install corrupt WAL");

        let canonical_before = state_tree_snapshot(&fixture.consumer.app);
        let state_before = state_tree_snapshot(&fixture.consumer.state);
        let refused = fixture
            .command("recover")
            .output()
            .expect("reject corrupt effect WAL");
        assert!(!refused.status.success(), "{corruption}");
        let refused = json(&refused);
        assert_eq!(refused["typed_failure"]["type"], "recovery_required");
        assert_eq!(refused["typed_failure"]["data"]["can_recover"], false);
        assert_eq!(
            state_tree_snapshot(&fixture.consumer.app),
            canonical_before,
            "{corruption}: canonical state changed"
        );
        assert_eq!(
            state_tree_snapshot(&fixture.consumer.state),
            state_before,
            "{corruption}: Forge state changed"
        );
        fs::write(&wal_path, &original_wal).expect("restore valid WAL fixture");
    }

    let recovered = assert_ok(
        &fixture
            .command("recover")
            .output()
            .expect("recover after restoring exact WAL"),
    );
    assert_eq!(recovered["data"]["status"], "recovered");
}

#[test]
fn promotion_recover_converges_across_every_durable_crash_boundary() {
    const CRASH_POINTS: &[&str] = &[
        "after_intent",
        "after_replay_reservation",
        "after_begin",
        "after_before_image",
        "after_bytes_before_marker",
        "after_commit",
        "after_replay_consume",
        "after_readback",
        "after_receipt",
    ];
    for crash_point in CRASH_POINTS {
        let fixture = PromotionRecoveryFixture::new();
        let crashed = fixture
            .command("apply")
            .env("FORGE_TEST_PROMOTION_CRASH_AT", crash_point)
            .output()
            .expect("run crash-injected promotion subprocess");
        assert_eq!(
            crashed.status.code(),
            Some(86),
            "crash point {crash_point} did not terminate at its durable boundary\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&crashed.stdout),
            String::from_utf8_lossy(&crashed.stderr)
        );
        if *crash_point == "after_bytes_before_marker" {
            let current = [
                fs::read_to_string(fixture.consumer.app.join("README.md")).expect("README partial"),
                fs::read_to_string(fixture.consumer.app.join("NOTES.md")).expect("NOTES partial"),
            ];
            assert_eq!(
                current
                    .iter()
                    .filter(|value| value.contains("recovered"))
                    .count(),
                1,
                "partial crash must leave exactly one old and one new file"
            );
        }
        let recovered = assert_ok(
            &fixture
                .command("recover")
                .output()
                .expect("recover interrupted promotion"),
        );
        assert!(
            matches!(
                recovered["data"]["status"].as_str(),
                Some("recovered" | "already_committed")
            ),
            "unexpected recovery status at {crash_point}: {recovered}"
        );
        if matches!(*crash_point, "after_intent" | "after_replay_reservation") {
            assert_eq!(
                recovered["data"]["receipt"]["recovery_execution"]["recovery_kind"],
                "pre_begin_fresh_execution_v1",
                "pre-Begin recovery must disclose its fresh execution provenance link"
            );
            assert_eq!(
                recovered["data"]["receipt"]["recovery_execution"]["durable_intent_digest"],
                recovered["data"]["receipt"]["replay"]["intent_digest"]
            );
        }
        assert_eq!(
            fs::read_to_string(fixture.consumer.app.join("README.md")).expect("recovered README"),
            "consumer project\nrecovered readme\n"
        );
        assert_eq!(
            fs::read_to_string(fixture.consumer.app.join("NOTES.md")).expect("recovered NOTES"),
            "recovered notes\n"
        );
        let canonical_once = state_tree_snapshot(&fixture.consumer.app);
        let state_once = state_tree_snapshot(&fixture.consumer.state);
        let repeated = assert_ok(
            &fixture
                .command("recover")
                .output()
                .expect("repeat recovered promotion"),
        );
        assert_eq!(repeated["data"]["status"], "already_committed");
        assert_eq!(
            repeated["data"]["canonical_mutation_performed"], false,
            "terminal recovery must not write again"
        );
        assert_eq!(state_tree_snapshot(&fixture.consumer.app), canonical_once);
        assert_eq!(state_tree_snapshot(&fixture.consumer.state), state_once);
    }
}

#[test]
fn replacement_agent_ranks_exact_spaced_root_recovery_then_observes_completion() {
    let fixture = PromotionRecoveryFixture::new();
    let crashed = fixture
        .command("apply")
        .env("FORGE_TEST_PROMOTION_CRASH_AT", "after_intent")
        .output()
        .expect("crash promotion after durable intent");
    assert_eq!(crashed.status.code(), Some(86));
    let state_before = state_tree_snapshot(&fixture.consumer.state);

    let replacement = assert_ok(&fixture.consumer.run(&["report"]));
    let continuity = &replacement["data"]["replacement_continuity"];
    let promotion = continuity["promotions"]
        .as_array()
        .expect("promotion continuity")
        .iter()
        .find(|promotion| promotion["preview_digest"] == fixture.preview_digest)
        .expect("recoverable promotion");
    assert_eq!(promotion["status"], "recoverable");
    let expected_argv = serde_json::json!([
        "forge-core",
        "workflow",
        "promotion",
        "recover",
        "--root",
        fixture.root.clone(),
        "--isolation-id",
        "isolation.promotion-recovery-e2e",
        "--expected-preview-digest",
        fixture.preview_digest.clone(),
        "--json"
    ]);
    assert_eq!(promotion["recovery_argv"], expected_argv);
    assert_eq!(
        continuity["ranked_next_actions"][0]["kind"],
        "recover_promotion"
    );
    assert_eq!(continuity["ranked_next_actions"][0]["argv"], expected_argv);
    assert_eq!(
        state_tree_snapshot(&fixture.consumer.state),
        state_before,
        "resume inspection must not repair interrupted promotion state"
    );

    let argv = expected_argv.as_array().expect("structured recovery argv");
    assert_ok(&execute_structured_argv(argv));
    let completed = assert_ok(&fixture.consumer.run(&["report"]));
    let promotion = completed["data"]["replacement_continuity"]["promotions"]
        .as_array()
        .expect("completed promotion continuity")
        .iter()
        .find(|promotion| promotion["preview_digest"] == fixture.preview_digest)
        .expect("completed promotion");
    assert_eq!(promotion["status"], "completed");
    assert!(promotion["recovery_argv"].is_null());
}

#[test]
fn replacement_agent_blocks_tampered_intent_without_repairing_it() {
    let fixture = PromotionRecoveryFixture::new();
    let crashed = fixture
        .command("apply")
        .env("FORGE_TEST_PROMOTION_CRASH_AT", "after_intent")
        .output()
        .expect("crash promotion after durable intent");
    assert_eq!(crashed.status.code(), Some(86));
    let intent = fixture.intent_path();
    let mut bytes = fs::read(&intent).expect("read intent");
    let index = bytes
        .iter()
        .position(|byte| *byte == b'{')
        .expect("JSON object");
    bytes[index] = b'[';
    fs::write(&intent, &bytes).expect("tamper intent");
    let before = state_tree_snapshot(&fixture.consumer.state);

    let resumed = assert_ok(&fixture.consumer.run(&["report"]));
    assert_eq!(
        resumed["data"]["replacement_continuity"]["status"],
        "blocked"
    );
    assert!(resumed["data"]["replacement_continuity"]["gaps"]
        .as_array()
        .is_some_and(|gaps| gaps
            .iter()
            .any(|gap| gap["code"] == "promotion_state_invalid")));
    assert_eq!(
        state_tree_snapshot(&fixture.consumer.state),
        before,
        "resume must not rewrite a tampered intent"
    );
}

#[test]
fn replacement_agent_blocks_missing_wrong_owner_and_released_linked_claims() {
    let fixture = PromotionRecoveryFixture::new();
    let live = assert_ok(&fixture.consumer.run(&["report"]));
    let live_continuity = &live["data"]["replacement_continuity"];
    assert_eq!(live_continuity["claims"][0]["liveness"], "live");
    assert!(!live_continuity["gaps"].as_array().is_some_and(|gaps| {
        gaps.iter().any(|gap| {
            matches!(
                gap["code"].as_str(),
                Some(
                    "linked_claim_missing"
                        | "linked_claim_owner_mismatch"
                        | "linked_claim_expired"
                        | "linked_claim_inactive"
                )
            )
        })
    }));

    let isolation_path = fs::read_dir(fixture.consumer.state.join("contracts/isolations"))
        .expect("list isolation contracts")
        .map(|entry| entry.expect("isolation entry").path())
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("yaml"))
        .expect("one isolation contract");
    let original_contract = fs::read_to_string(&isolation_path).expect("read isolation contract");
    assert!(original_contract.contains(&fixture.claim_id));
    assert!(original_contract.contains("agent.promotion-recovery-e2e"));

    fs::write(
        &isolation_path,
        original_contract.replace(&fixture.claim_id, "claim.missing.replacement-e2e"),
    )
    .expect("point isolation at missing claim");
    let missing = assert_ok(&fixture.consumer.run(&["report"]));
    assert!(missing["data"]["replacement_continuity"]["gaps"]
        .as_array()
        .is_some_and(|gaps| gaps.iter().any(|gap| {
            gap["code"] == "linked_claim_missing"
                && gap["blocking"] == true
                && gap["isolation_id"] == "isolation.promotion-recovery-e2e"
        })));

    fs::write(
        &isolation_path,
        original_contract.replace(
            "agent.promotion-recovery-e2e",
            "agent.different-replacement-e2e",
        ),
    )
    .expect("point isolation at a different owner");
    let wrong_owner = assert_ok(&fixture.consumer.run(&["report"]));
    assert!(wrong_owner["data"]["replacement_continuity"]["gaps"]
        .as_array()
        .is_some_and(|gaps| gaps.iter().any(|gap| {
            gap["code"] == "linked_claim_owner_mismatch"
                && gap["blocking"] == true
                && gap["isolation_id"] == "isolation.promotion-recovery-e2e"
        })));
    fs::write(&isolation_path, &original_contract).expect("restore isolation contract");

    let released = bin()
        .args([
            "claim",
            "release",
            "--root",
            &fixture.root,
            "--id",
            &fixture.claim_id,
            "--agent",
            "agent.promotion-recovery-e2e",
            "--now-unix",
            &now().to_string(),
            "--json",
        ])
        .output()
        .expect("release linked claim");
    assert_ok(&released);

    let resumed = assert_ok(&fixture.consumer.run(&["report"]));
    let continuity = &resumed["data"]["replacement_continuity"];
    assert_eq!(continuity["status"], "blocked");
    assert!(continuity["gaps"].as_array().is_some_and(|gaps| {
        gaps.iter().any(|gap| {
            gap["code"] == "linked_claim_inactive"
                && gap["blocking"] == true
                && gap["isolation_id"] == "isolation.promotion-recovery-e2e"
        })
    }));
}

#[test]
fn promotion_recover_refuses_third_content_without_another_write() {
    let fixture = PromotionRecoveryFixture::new();
    let crashed = fixture
        .command("apply")
        .env("FORGE_TEST_PROMOTION_CRASH_AT", "after_bytes_before_marker")
        .output()
        .expect("leave partial two-file promotion");
    assert_eq!(crashed.status.code(), Some(86));
    fs::write(
        fixture.consumer.app.join("README.md"),
        "unrelated third-party bytes\n",
    )
    .expect("inject incompatible canonical content");
    let canonical_before = state_tree_snapshot(&fixture.consumer.app);
    let state_before = state_tree_snapshot(&fixture.consumer.state);
    let refused = fixture
        .command("recover")
        .output()
        .expect("attempt ambiguous recovery");
    assert!(!refused.status.success());
    let refused = json(&refused);
    assert_eq!(refused["typed_failure"]["type"], "recovery_required");
    assert!(refused["typed_failure"]["data"]["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("neither the recorded old nor exact new bytes")));
    assert_eq!(state_tree_snapshot(&fixture.consumer.app), canonical_before);
    assert_eq!(state_tree_snapshot(&fixture.consumer.state), state_before);
}

#[test]
fn promotion_recover_refuses_changed_destination_before_effect_begin() {
    let fixture = PromotionRecoveryFixture::new();
    let crashed = fixture
        .command("apply")
        .env("FORGE_TEST_PROMOTION_CRASH_AT", "after_intent")
        .output()
        .expect("leave durable intent before effect begin");
    assert_eq!(crashed.status.code(), Some(86));
    fs::write(
        fixture.consumer.app.join("README.md"),
        "changed after approval but before begin\n",
    )
    .expect("change canonical destination before recovery");
    let canonical_before = state_tree_snapshot(&fixture.consumer.app);
    let state_before = state_tree_snapshot(&fixture.consumer.state);
    let refused = fixture
        .command("recover")
        .output()
        .expect("attempt pre-begin recovery against changed destination");
    assert!(!refused.status.success());
    let refused = json(&refused);
    assert_eq!(refused["typed_failure"]["type"], "recovery_required");
    assert!(refused["typed_failure"]["data"]["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("exactly match the approved destination snapshot")));
    assert_eq!(state_tree_snapshot(&fixture.consumer.app), canonical_before);
    assert_eq!(
        state_tree_snapshot(&fixture.consumer.state),
        state_before,
        "refusal must happen before effect Begin or any other durable write"
    );
}

#[test]
fn latest_release_routes_strict_universal_assurance_away_from_solo_execution() {
    let solo = Consumer::new_with_prefix("forge-solo-universal-routing-e2e");
    assert_ok(&solo.run(&["init", "--readiness-profile", "solo_cooperative"]));
    upgrade_to_latest(&solo);

    let discover = advance_fixture_to_policy(&solo, "policy.workflow.discover-intent");
    assert_eq!(
        discover["data"]["selected_policy_ref"],
        "policy.workflow.discover-intent"
    );
    let packet_digest = discover["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("solo objective packet")
        .to_owned();
    let objective = solo.write_json(
        "solo universal routing objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Improve Forge through ordinary solo development with agents",
                "constraints": ["keep strict external assurance additive and later"],
                "unacceptable_outcomes": ["require an unavailable independent reviewer before ordinary implementation"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.same-owner.routing-e2e",
            "host_provenance": {
                "host_id": "host.routing-e2e",
                "host_version": "test",
                "session_ref": "session.routing-e2e",
                "interaction_ref": "turn.routing-e2e",
                "conversation_digest": format!("sha256:{}", "7".repeat(64)),
                "observed_at_unix": 1
            }
        }),
    );
    let accepted = assert_ok(&run_cooperative_input(&solo, &packet_digest, &objective));
    assert_eq!(accepted["data"]["next"]["status"], "ready_to_complete");
    let snapshot = accepted["data"]["next"]["snapshot_digest"]
        .as_str()
        .expect("discover completion snapshot")
        .to_owned();
    assert_ok(&solo.run(&[
        "complete",
        "--if-snapshot",
        &snapshot,
        "--principal",
        "principal.same-owner.routing-e2e",
    ]));

    let solo_next = assert_ok(&solo.run(&["next"]));
    assert_eq!(
        solo_next["data"]["selected_policy_ref"], "policy.workflow.domain-scan",
        "the strict independent-review policy must not dead-end ordinary solo execution"
    );
    assert!(
        solo_next["data"]["durable_assurance"]["projection"]["lenses"]
            .as_array()
            .is_some_and(|lenses| lenses.len() == 8),
        "release assurance must remain visible rather than being erased"
    );

    let strict = Consumer::new_with_prefix("forge-strict-universal-routing-e2e");
    assert_ok(&strict.run(&["init", "--readiness-profile", "strict_external"]));
    upgrade_to_latest(&strict);
    let strict_next = advance_fixture_to_policy(&strict, "policy.workflow.universal-assurance");
    assert_eq!(
        strict_next["data"]["selected_policy_ref"], "policy.workflow.universal-assurance",
        "strict external assurance must remain unchanged"
    );
    assert!(
        strict_next["data"]["simulation"]["candidate_capability_gaps"]
            .as_array()
            .is_some_and(|gaps| gaps.iter().any(|gap| {
                gap["id"] == "capability.workflow.universal-assurance.independent-review"
                    && gap["blocking"] == true
            }))
    );
}
