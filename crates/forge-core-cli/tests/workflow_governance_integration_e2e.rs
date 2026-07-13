//! Real-consumer P5c proof: trusted initialization, automatic selection,
//! replacement-agent resume, read-only shadow, and authority bypass rejection.

use assert_cmd::Command;
use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    AttestationInput, CanonicalIntent, PrincipalCredentialStatus, PrincipalRegistryContract,
    PrincipalRegistryDocument, PrincipalRegistryEntry, WorkflowApplicabilityAuthorizationRequest,
    WorkflowEvidenceAuthorizationRequest, PRINCIPAL_REGISTRY_SCHEMA_VERSION,
};
use forge_core_contracts::operation::CallerRole;
use forge_core_contracts::{
    PrincipalId, ReadinessTarget, StableId, WorkflowContentAddressedReference,
    WorkflowEvaluatorProvider, WorkflowEvidenceKind, WorkflowEvidenceOutcome,
    WorkflowEvidenceStrength, WorkflowEvidenceSubjectKind,
};
use forge_core_store::sha256_content_hash;
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const WORKFLOW_AUDIENCE: &str = "forge-core:workflow:cli-e2e";
const HUMAN_CREDENTIAL: &str = "credential.workflow.cli-e2e-human";

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

struct SignedCliAuthority {
    key: SigningKey,
}

impl SignedCliAuthority {
    fn install(consumer: &Consumer) -> Self {
        let key = SigningKey::from_bytes(&[81; 32]);
        let document = PrincipalRegistryDocument {
            schema_version: PRINCIPAL_REGISTRY_SCHEMA_VERSION.to_owned(),
            principal_registry: PrincipalRegistryContract {
                audience: WORKFLOW_AUDIENCE.to_owned(),
                principals: vec![PrincipalRegistryEntry {
                    credential_id: HUMAN_CREDENTIAL.to_owned(),
                    principal_id: PrincipalId("principal.workflow.cli-e2e-human".to_owned()),
                    agent_id: StableId("agent.workflow.cli-e2e-human-console".to_owned()),
                    role: CallerRole::Human,
                    public_key_hex: hex(&key.verifying_key().to_bytes()),
                    allowed_tools: vec![StableId("workflow".to_owned())],
                    authority_grants: [
                        "workflow.applicability.assess",
                        "workflow.evidence.authorize_human",
                    ]
                    .into_iter()
                    .map(|grant| StableId(grant.to_owned()))
                    .collect(),
                    status: PrincipalCredentialStatus::Active,
                }],
            },
        };
        let path = consumer
            .state
            .parent()
            .expect("operator root")
            .join("operator/workflow-principal-registry.yaml");
        fs::create_dir_all(path.parent().expect("registry parent")).expect("registry directory");
        fs::write(
            path,
            yaml_serde::to_string(&document).expect("registry YAML"),
        )
        .expect("trusted principal registry");
        Self { key }
    }

    fn attestation<T: Serialize>(&self, action: &str, request: &T) -> AttestationInput {
        static NONCE: AtomicU64 = AtomicU64::new(0);
        let issued = i64::try_from(now()).expect("i64 clock");
        let mut attestation = AttestationInput {
            credential_id: Some(HUMAN_CREDENTIAL.to_owned()),
            audience: Some(WORKFLOW_AUDIENCE.to_owned()),
            execution_intent_digest: None,
            nonce: format!(
                "workflow-cli-e2e-{action}-{issued}-{}",
                NONCE.fetch_add(1, Ordering::SeqCst)
            ),
            ts: issued,
            signature: String::new(),
            public_key_hex: hex(&self.key.verifying_key().to_bytes()),
        };
        let intent = CanonicalIntent {
            tool: "workflow".to_owned(),
            arguments: serde_json::json!({
                "action": action,
                "request": serde_json::to_value(request).expect("request JSON"),
            }),
            credential_id: attestation.credential_id.clone(),
            audience: attestation.audience.clone(),
            execution_intent_digest: None,
            nonce: attestation.nonce.clone(),
            ts: attestation.ts,
        };
        attestation.signature = hex(&self
            .key
            .sign(&intent.canonical_bytes().expect("canonical intent"))
            .to_bytes());
        attestation
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

fn required_u64(value: &Value, field: &str) -> u64 {
    value[field]
        .as_u64()
        .unwrap_or_else(|| panic!("guidance field '{field}' must be an integer: {value:#}"))
}

fn basis_digest(root: &Path, refs: &[String]) -> String {
    let mut basis = refs
        .iter()
        .map(|subject_ref| WorkflowContentAddressedReference {
            subject_ref: subject_ref.replace('\\', "/"),
            subject_digest: sha256_content_hash(
                &fs::read(root.join(subject_ref)).expect("applicability basis"),
            ),
        })
        .collect::<Vec<_>>();
    basis.sort_by(|left, right| {
        left.subject_ref
            .cmp(&right.subject_ref)
            .then_with(|| left.subject_digest.cmp(&right.subject_digest))
    });
    sha256_content_hash(&serde_json_canonicalizer::to_vec(&basis).expect("canonical basis"))
}

#[test]
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

    let packet_set = assert_ok(&consumer.run(&["action-packets"]));
    let human_packet = packet_set["data"]["packets"]
        .as_array()
        .expect("action packet list")
        .iter()
        .find(|packet| packet["required_authority"]["approval_boundary"] == "human_approval_broker")
        .expect("fresh discovery exposes a human broker packet");
    let human_packet_digest = human_packet["packet_digest"]
        .as_str()
        .expect("human packet digest");
    let human_input_value = match human_packet["authorization_kind"]
        .as_str()
        .expect("human authorization kind")
    {
        "applicability" => serde_json::json!({
            "kind": "applicability",
            "applicable": true,
            "basis_refs": ["README.md"]
        }),
        "decision" => serde_json::json!({
            "kind": "decision",
            "selected_alternative_ref": human_packet["input_contract"]["alternatives"][0]["id"]
        }),
        "evidence" => {
            let subject_kind = human_packet["input_contract"]["subject_kinds"][0]
                .as_str()
                .expect("human evidence subject kind");
            let subject_ref = match subject_kind {
                "artifact" => "README.md",
                "repository_state" | "project_snapshot" => human_packet["binding"]["project_id"]
                    .as_str()
                    .expect("packet project id"),
                _ => "subject.workflow.one-call-human",
            };
            serde_json::json!({
                "kind": "evidence",
                "outcome": "pass",
                "subject_kind": subject_kind,
                "subject_ref": subject_ref,
                "scenario_ref": "README.md"
            })
        }
        "waiver" => serde_json::json!({
            "kind": "waiver",
            "reason": "negative boundary test only"
        }),
        other => panic!("unexpected human packet kind: {other}"),
    };
    let human_input = consumer.write_json("human-closed-input.json", &human_input_value);
    let human_input_arg = human_input.display().to_string();
    let rejected_local_human = bin()
        .args([
            "workflow",
            "action",
            "authorize",
            "--root",
            &root,
            "--packet-digest",
            human_packet_digest,
            "--input-file",
            &human_input_arg,
            "--credential-id",
            "credential.workflow.one-call-operator",
            "--json",
        ])
        .output()
        .expect("reject local human-boundary action");
    assert!(!rejected_local_human.status.success());
    assert!(String::from_utf8_lossy(&rejected_local_human.stdout).contains("external human"));

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
// One uninterrupted multiprocess flow keeps every request, signed attestation,
// and replacement-agent assertion visibly bound to the preceding CLI output.
#[allow(clippy::too_many_lines)]
fn signed_cli_flow_completes_first_policy_and_resumes_capability_gap() {
    let consumer = Consumer::new();
    let authority = SignedCliAuthority::install(&consumer);
    let initialized = assert_ok(&consumer.run(&["init"]));
    assert_eq!(initialized["data"]["current_phase"], "1-discovery");

    let discover = assert_ok(&consumer.run(&["next"]));
    let discover = &discover["data"];
    assert_eq!(
        discover["selected_policy_ref"],
        "policy.workflow.discover-intent"
    );
    let observed = now();
    let evidence_request = WorkflowEvidenceAuthorizationRequest {
        project_id: StableId(required_str(discover, "project_id").to_owned()),
        policy_bundle_digest: required_str(discover, "bundle_digest").to_owned(),
        policy_ref: StableId(required_str(discover, "selected_policy_ref").to_owned()),
        claim_ref: StableId("claim.workflow.discover-intent.intent-grounded".to_owned()),
        evaluator_ref: StableId("evaluator.workflow.discover-intent.intent-review".to_owned()),
        provider: WorkflowEvaluatorProvider::AuthorizedHuman,
        kind: WorkflowEvidenceKind::HumanAcceptance,
        strength: WorkflowEvidenceStrength::AuthoritativeAcceptance,
        outcome: WorkflowEvidenceOutcome::Pass,
        subject_kind: WorkflowEvidenceSubjectKind::ProjectSnapshot,
        subject_ref: required_str(discover, "project_id").to_owned(),
        subject_digest: required_str(discover, "snapshot_digest").to_owned(),
        scenario_digest: sha256_content_hash(b"cli-e2e:accepted-product-intent"),
        state_version: required_u64(discover, "state_version"),
        current_phase: StableId(required_str(discover, "current_phase").to_owned()),
        snapshot_digest: required_str(discover, "snapshot_digest").to_owned(),
        ledger_head_digest: required_str(discover, "ledger_head_digest").to_owned(),
        readiness_target: ReadinessTarget::Explore,
        observed_at_unix: observed,
        expires_at_unix: Some(observed + 3_600),
    };
    let evidence_attestation = authority.attestation("evidence_authorize", &evidence_request);
    let evidence_request_path = consumer.write_json("evidence-request.json", &evidence_request);
    let evidence_attestation_path =
        consumer.write_json("evidence-attestation.json", &evidence_attestation);
    let evidence_request_arg = evidence_request_path.display().to_string();
    let evidence_attestation_arg = evidence_attestation_path.display().to_string();
    assert_ok(&consumer.run(&[
        "evidence-authorize",
        "--request-file",
        &evidence_request_arg,
        "--attestation-file",
        &evidence_attestation_arg,
    ]));

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

    let applicability = &applicability["data"];
    let basis_refs = vec!["README.md".to_owned()];
    let applicability_observed = now();
    let applicability_request = WorkflowApplicabilityAuthorizationRequest {
        project_id: StableId(required_str(applicability, "project_id").to_owned()),
        policy_bundle_digest: required_str(applicability, "bundle_digest").to_owned(),
        policy_ref: StableId(required_str(applicability, "selected_policy_ref").to_owned()),
        state_version: required_u64(applicability, "state_version"),
        current_phase: StableId(required_str(applicability, "current_phase").to_owned()),
        snapshot_digest: required_str(applicability, "snapshot_digest").to_owned(),
        ledger_head_digest: required_str(applicability, "ledger_head_digest").to_owned(),
        applicable: true,
        evaluator_ref: StableId("evaluator.workflow.applicability.human".to_owned()),
        authority_scope: StableId("workflow.applicability.assess".to_owned()),
        basis_digest: basis_digest(&consumer.app, &basis_refs),
        basis_refs,
        observed_at_unix: applicability_observed,
        expires_at_unix: applicability_observed + 3_600,
    };
    let applicability_attestation =
        authority.attestation("applicability_assess", &applicability_request);
    let applicability_request_path =
        consumer.write_json("applicability-request.json", &applicability_request);
    let applicability_attestation_path =
        consumer.write_json("applicability-attestation.json", &applicability_attestation);
    let applicability_request_arg = applicability_request_path.display().to_string();
    let applicability_attestation_arg = applicability_attestation_path.display().to_string();
    assert_ok(&consumer.run(&[
        "applicability-authorize",
        "--request-file",
        &applicability_request_arg,
        "--attestation-file",
        &applicability_attestation_arg,
    ]));

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
    assert!(text.contains("workflow applicability-authorize"));
    assert!(text.contains("workflow capability-authorize"));
    assert!(text.contains("workflow evidence-authorize"));
    assert!(!text.contains("workflow observe-artifact"));
    assert!(!text.contains("--principal-registry"));
    assert!(!text.contains("--workflow"));
}
