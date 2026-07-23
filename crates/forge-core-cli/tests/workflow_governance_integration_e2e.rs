//! Real-consumer P5c proof: trusted initialization, automatic selection,
//! replacement-agent resume, read-only shadow, and authority bypass rejection.

use assert_cmd::Command;
use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    workflow_broker_event_signing_bytes, workflow_broker_host_event_descriptor_digest,
    AuthorizedWorkflowBrokerControlPlane, WorkflowBrokerEventEnvelope, WorkflowBrokerIssuerProfile,
    WorkflowBrokerSemanticInput, WORKFLOW_BROKER_EVENT_SCHEMA_VERSION,
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
use std::path::PathBuf;
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
        AuthorizedWorkflowBrokerControlPlane::from_document_for_binding(
            document.clone(),
            &audience,
            &project_id,
            &workflow_id,
        )
        .expect("strict broker registry fixture");
        let path = consumer
            .state
            .parent()
            .expect("operator root")
            .join("operator/workflow-broker-registry.yaml");
        fs::create_dir_all(path.parent().expect("registry parent")).expect("registry directory");
        fs::write(
            path,
            yaml_serde::to_string(&document).expect("strict broker registry YAML"),
        )
        .expect("preconfigured external broker registry");
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
    assert_eq!(initialized["data"]["current_phase"], "1-discovery");
    assert_eq!(initialized["data"]["state_version"], 0);
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
    assert!(
        !packets.is_empty(),
        "current guidance must expose its actions"
    );
    assert!(packets.iter().all(|packet| {
        packet["packet_digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:") && digest.len() == 71)
            && packet["binding"]["ledger_head_digest"] == next["data"]["ledger_head_digest"]
            && packet["required_authority"]["approval_boundary"]
                .as_str()
                .is_some_and(|value| value.ends_with("_broker"))
    }));

    let resumed = assert_ok(&consumer.run(&["resume"]));
    for field in [
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
#[allow(clippy::too_many_lines)] // One public ceremony proves both local denial and permitted one-call commit.
fn local_action_authorize_prepares_signs_and_commits_without_intermediate_authority_files() {
    let consumer = Consumer::new();
    assert_ok(&consumer.run(&["init"]));
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
    let initialized = assert_ok(&consumer.run(&["init"]));
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
    assert!(text.contains("workflow next"));
    assert!(text.contains("workflow resume"));
    assert!(text.contains("workflow action authorize"));
    assert!(text.contains("workflow action apply"));
    assert!(text.contains("workflow intent record"));
    assert!(!text.contains("workflow applicability-authorize"));
    assert!(!text.contains("workflow capability-authorize"));
    assert!(!text.contains("workflow evidence-authorize"));
    assert!(!text.contains("workflow observe-artifact"));
    assert!(!text.contains("--principal-registry"));
    assert!(!text.contains("--workflow"));
}
