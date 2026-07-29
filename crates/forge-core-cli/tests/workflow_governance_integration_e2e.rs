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
    workflow_broker_expected_audience, PrincipalId, RuntimeKind, StableId,
    WorkflowBrokerBoundOperation, WorkflowBrokerCredentialProfile, WorkflowBrokerCredentialPurpose,
    WorkflowBrokerCredentialStatus, WorkflowBrokerCustodyKind, WorkflowBrokerHostBinding,
    WorkflowBrokerHostInteractionKind, WorkflowBrokerNativeHostProvenance,
    WorkflowBrokerPublicCredentialMetadata, WorkflowBrokerPublicKeyAlgorithm,
    WorkflowBrokerPublicRegistryDocument, WorkflowEvidenceOutcome, WorkflowEvidenceSubjectKind,
    WORKFLOW_BROKER_PUBLIC_REGISTRY_SCHEMA_VERSION, WORKFLOW_BROKER_REQUIRED_EVENT_SCHEMA_VERSION,
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
    Command::cargo_bin("forge-core").expect("forge-core binary")
}

struct Consumer {
    parent: PathBuf,
    app: PathBuf,
    state: PathBuf,
}

impl Consumer {
    fn new() -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let sequence = SEQ.fetch_add(1, Ordering::SeqCst);
        let parent = std::env::temp_dir().join(format!(
            "forge-workflow-p5c-e2e-{}-{sequence}",
            std::process::id()
        ));
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

    let resumed = assert_ok(&consumer.run(&["resume"]));
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

    let decision =
        consumer.write_json("decision after accepted.json", &cooperative_decision_json());
    let before_decision = state_tree_snapshot(&consumer.state);
    let consumed = run_cooperative_input(&consumer, &packet_digest, &decision);
    assert_eq!(consumed.status.code(), Some(4));
    assert_eq!(
        json(&consumed)["command"],
        "workflow.intent.accept_cooperative"
    );
    assert_eq!(state_tree_snapshot(&consumer.state), before_decision);
}

#[test]
fn cooperative_evidence_cli_executes_the_workflow_next_packet_and_survives_restart() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));
    let next = assert_ok(&consumer.run(&["next"]));
    let packet_digest = next["data"]["authorization"]["action_packets"][0]["packet_digest"]
        .as_str()
        .expect("cooperative objective packet")
        .to_owned();
    let objective = consumer.write_json(
        "cooperative evidence objective.json",
        &serde_json::json!({
            "kind": "unambiguous",
            "proposal": {
                "outcome": "Admit honest same-owner representative evidence",
                "constraints": ["remain host neutral"],
                "unacceptable_outcomes": ["claim independent review"],
                "open_uncertainties": []
            },
            "carrying_principal": "principal.agent.cli-e2e",
            "host_provenance": {
                "host_id": "host.cli-e2e",
                "host_version": "test",
                "session_ref": "session.cli-e2e",
                "interaction_ref": "turn.evidence",
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

    let next = assert_ok(&consumer.run(&["next"]));
    let packet = &next["data"]["cooperative_evidence_action_packet"];
    assert_eq!(
        packet["argv"],
        serde_json::json!([
            "forge-core",
            "workflow",
            "evidence",
            "admit-cooperative",
            "--root",
            ".",
            "--input-file",
            "${FORGE_COOPERATIVE_EVIDENCE_INPUT_FILE}",
            "--json"
        ])
    );
    assert_eq!(packet["input_file_must_be_outside_project_snapshot"], true);
    assert_eq!(
        packet["route"]["policy_ref"],
        "policy.workflow.discover-intent"
    );
    assert_eq!(packet["route"]["source_provider"], "authorized_human");
    assert_eq!(packet["route"]["provider"], "repository_inspector");
    assert_eq!(packet["kernel_derived_outcome"], "pass");
    let mut offer = packet["offer_template"].clone();
    offer["offer_id"] = serde_json::json!("offer.cli-e2e.pass");
    let input = consumer.write_json("cooperative evidence offer.json", &offer);
    let admitted = assert_ok(&run_cooperative_evidence(&consumer, &input));
    assert_eq!(
        admitted["data"]["event"]["type"],
        "cooperative_evidence_observed"
    );
    assert_eq!(
        admitted["data"]["event"]["payload"]["disposition"],
        "admitted"
    );
    assert_eq!(
        admitted["data"]["event"]["payload"]["admitted_evidence"]["outcome"],
        "pass"
    );

    let restarted = assert_ok(&consumer.run(&["next"]));
    let audit = restarted["data"]["cooperative_evidence"]
        .as_array()
        .expect("cooperative evidence audit");
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["current_status"], "supporting");
    assert!(audit[0]["proves"].as_array().is_some_and(|proofs| proofs
        .iter()
        .any(|proof| proof == "kernel_verified_project_state_readback")));
    assert!(audit[0]["does_not_prove"]
        .as_array()
        .is_some_and(|limits| limits
            .iter()
            .any(|limit| limit == "independent_semantic_review")));
    assert!(audit[0]["does_not_prove"]
        .as_array()
        .is_some_and(|limits| limits.iter().any(|limit| limit == "selected_source_claim")));
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
    let material_packet = initial["data"]["next"]["authorization"]["objective_management_packet"]
        ["packet_digest"]
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

    let clarification_packet = material["data"]["next"]["authorization"]
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
    let replacement = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(
        replacement["data"]["active_cooperative_objective"]["revision"],
        3
    );
    assert_eq!(
        replacement["data"]["active_cooperative_objective"]["revision_reason"],
        "The owner added execution detail without changing direction"
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
    let resumed_ready = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(resumed_ready["data"], ready["data"]);

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
    let resumed_applicability = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(resumed_applicability["data"], applicability["data"]);

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
    let resumed_gap = assert_ok(&consumer.run(&["resume"]));
    assert_eq!(resumed_gap["data"], capability_gap["data"]);
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
    let resumed = assert_ok(&consumer.run(&["resume"]));
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
    assert!(text.contains("workflow resume"));
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
