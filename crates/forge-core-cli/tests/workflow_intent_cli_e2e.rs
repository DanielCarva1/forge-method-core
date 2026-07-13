//! Public P7b human-intent journey through the external broker boundary.

use assert_cmd::Command;
use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    workflow_broker_event_signing_bytes, WorkflowBrokerEventEnvelope, WorkflowBrokerIssuerProfile,
    WorkflowBrokerSemanticInput, WORKFLOW_BROKER_EVENT_SCHEMA_VERSION,
};
use forge_core_contracts::{PrincipalId, StableId, WorkflowHumanIntentRevision};
use forge_core_decisions::workflow_human_intent_digest;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ISSUER_ID: &str = "broker.host.human.intent-v1";
const ORIGIN_PRINCIPAL_ID: &str = "principal.human.product-owner";
const SEPARATION_DOMAIN: &str = "human-session.primary";

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary")
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn ok(output: &std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "command failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON output")
}

fn failed(output: &std::process::Output) -> Value {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON failure output")
}

fn run(app: &str, tail: &[&str]) -> std::process::Output {
    let mut command = bin();
    command.arg("workflow");
    command.args(tail);
    command.args(["--root", app, "--json"]);
    command.output().expect("workflow command")
}

#[allow(clippy::too_many_arguments)] // Explicit broker wire coordinates keep each adversarial fixture auditable.
fn signed_envelope(
    key: &SigningKey,
    audience: &str,
    project_id: &str,
    packet_digest: &str,
    semantic_input: WorkflowBrokerSemanticInput,
    issued_at_unix: u64,
    expires_at_unix: u64,
    nonce: &str,
) -> WorkflowBrokerEventEnvelope {
    let mut envelope = WorkflowBrokerEventEnvelope {
        schema_version: WORKFLOW_BROKER_EVENT_SCHEMA_VERSION.to_owned(),
        audience: audience.to_owned(),
        issuer_id: StableId(ISSUER_ID.to_owned()),
        issuer_profile: WorkflowBrokerIssuerProfile::Human,
        origin_principal_id: PrincipalId(ORIGIN_PRINCIPAL_ID.to_owned()),
        separation_domain: StableId(SEPARATION_DOMAIN.to_owned()),
        event_kind: semantic_input.kind(),
        project_id: StableId(project_id.to_owned()),
        action_packet_digest: packet_digest.to_owned(),
        semantic_input,
        issued_at_unix,
        expires_at_unix,
        nonce: nonce.to_owned(),
        signature: String::new(),
    };
    let bytes = workflow_broker_event_signing_bytes(&envelope).expect("broker signing bytes");
    envelope.signature = hex(&key.sign(&bytes).to_bytes());
    envelope
}

fn write_envelope(parent: &Path, name: &str, envelope: &WorkflowBrokerEventEnvelope) -> PathBuf {
    let path = parent.join(name);
    fs::write(
        &path,
        serde_json::to_vec_pretty(envelope).expect("serialize broker envelope"),
    )
    .expect("write broker envelope");
    path
}

fn file_snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, current: &Path, output: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = fs::read_dir(current)
            .expect("read state directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("state entries");
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, output);
            } else if path.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .expect("state-relative path")
                    .to_string_lossy()
                    .replace('\\', "/");
                output.insert(relative, fs::read(path).expect("state file"));
            }
        }
    }
    let mut output = BTreeMap::new();
    visit(root, root, &mut output);
    output
}

fn intent_packet(next: &Value) -> &Value {
    let packets = next["data"]["authorization"]["action_packets"]
        .as_array()
        .expect("action packets");
    assert_eq!(packets.len(), 1, "missing intent exposes one action only");
    let packet = &packets[0];
    assert_eq!(packet["authorization_kind"], "intent_revision");
    assert_eq!(
        packet["required_authority"]["approval_boundary"],
        "human_approval_broker"
    );
    packet
}

#[test]
#[allow(clippy::too_many_lines)] // One public multiprocess story keeps authority, persistence, and zero-write rejection auditable.
fn human_intent_record_is_external_origin_bound_durable_and_fail_closed() {
    let parent = std::env::temp_dir().join(format!(
        "forge-workflow-intent-public-e2e-{}-{}",
        std::process::id(),
        now()
    ));
    let _ = fs::remove_dir_all(&parent);
    let app = parent.join("app");
    fs::create_dir_all(&app).expect("app root");
    fs::write(app.join("README.md"), "# agent-built product\n").expect("README");
    let _ = std::process::Command::new("git")
        .args(["init", &app.display().to_string()])
        .output();
    let app_arg = app.display().to_string();
    let started = ok(&bin()
        .args(["start", "--root", &app_arg, "--json"])
        .output()
        .expect("start"));
    let state_root = PathBuf::from(
        started["data"]["project"]["state_root"]
            .as_str()
            .expect("state root"),
    );
    ok(&run(&app_arg, &["init"]));

    let key = SigningKey::from_bytes(&[42; 32]);
    let public_key = parent.join("human-broker.pub");
    let ceremony = parent.join("human-broker-ceremony.md");
    fs::write(&public_key, hex(&key.verifying_key().to_bytes())).expect("public key");
    fs::write(
        &ceremony,
        "operator enrolled an external human-presence broker\n",
    )
    .expect("ceremony");
    let trusted = ok(&run(
        &app_arg,
        &[
            "broker",
            "trust",
            "--issuer-id",
            ISSUER_ID,
            "--profile",
            "human",
            "--public-key-file",
            &public_key.display().to_string(),
            "--ceremony-ref",
            "operator://ceremony/human-intent/v1",
            "--ceremony-file",
            &ceremony.display().to_string(),
        ],
    ));
    let audience = trusted["data"]["audience"]
        .as_str()
        .expect("broker audience");

    let fresh = ok(&run(&app_arg, &["next"]));
    assert_eq!(
        fresh["data"]["durable_assurance"]["status"],
        "missing_human_intent"
    );
    assert!(fresh["data"]["durable_assurance"]["projection"].is_null());
    let packet = intent_packet(&fresh);
    let packet_digest = packet["packet_digest"]
        .as_str()
        .expect("intent packet digest");
    let project_id = fresh["data"]["project_id"].as_str().expect("project id");

    let semantic_input = WorkflowBrokerSemanticInput::IntentRevision {
        desired_outcome: "A dependable agent-built game that is fun on first launch".to_owned(),
        constraints: vec!["Runs offline on the target machine".to_owned()],
        preferences: vec!["Short feedback loops".to_owned()],
        unacceptable_outcomes: vec!["A first launch with broken core controls".to_owned()],
        uncertainties: vec!["Final art direction is not selected".to_owned()],
        conversation_ref: "conversation://codex/thread/product-intent/turn-17".to_owned(),
        conversation_digest: format!("sha256:{}", "c".repeat(64)),
    };
    let issued = now();
    let envelope = signed_envelope(
        &key,
        audience,
        project_id,
        packet_digest,
        semantic_input.clone(),
        issued,
        issued + 120,
        "human-intent-e2e-nonce-0001",
    );
    let envelope_path = write_envelope(&parent, "intent-revision.json", &envelope);
    let recorded = ok(&run(
        &app_arg,
        &[
            "intent",
            "record",
            "--origin-envelope-file",
            &envelope_path.display().to_string(),
        ],
    ));
    assert_eq!(recorded["command"], "workflow.intent.record");

    // A replacement process reconstructs the durable projection from the
    // ledger, without relying on this test's in-memory envelope or chat state.
    let resumed = ok(&run(&app_arg, &["resume"]));
    assert_eq!(
        resumed["data"]["durable_assurance"]["status"],
        "intent_accepted"
    );
    let projection = &resumed["data"]["durable_assurance"]["projection"];
    let expected_intent = WorkflowHumanIntentRevision {
        intent_id: StableId(format!("intent.workflow.{project_id}")),
        revision: 1,
        desired_outcome: "A dependable agent-built game that is fun on first launch".to_owned(),
        constraints: vec!["Runs offline on the target machine".to_owned()],
        preferences: vec!["Short feedback loops".to_owned()],
        unacceptable_outcomes: vec!["A first launch with broken core controls".to_owned()],
        uncertainties: vec!["Final art direction is not selected".to_owned()],
        source_conversation_ref: "conversation://codex/thread/product-intent/turn-17".to_owned(),
        source_conversation_digest: format!("sha256:{}", "c".repeat(64)),
    };
    let expected_digest = workflow_human_intent_digest(&expected_intent).expect("intent digest");
    assert_eq!(
        projection["intent"],
        serde_json::to_value(&expected_intent).expect("expected intent JSON")
    );
    assert_eq!(projection["binding"]["intent_revision"], 1);
    assert_eq!(projection["binding"]["assurance_epoch"], 1);
    assert_eq!(projection["binding"]["intent_digest"], expected_digest);
    assert_eq!(projection["readiness"], "unknown");
    let lenses = projection["lenses"].as_array().expect("universal lenses");
    let expected_lenses = [
        "intended_outcome",
        "critical_journeys",
        "system_integrity",
        "quality_attributes",
        "operability",
        "lifecycle_coverage",
        "risk_and_failure",
        "evidence_representativeness",
    ];
    assert_eq!(lenses.len(), expected_lenses.len());
    for (lens, expected) in lenses.iter().zip(expected_lenses) {
        assert_eq!(lens["lens"], expected);
        assert_eq!(lens["claim_status"], "unknown");
        assert_eq!(lens["evidence_refs"], serde_json::json!([]));
        assert!(lens["evaluator_ref"].is_null());
    }
    assert_eq!(
        projection["blocker_lenses"],
        serde_json::to_value(expected_lenses).expect("blocker lens JSON")
    );

    // A valid generic Assurance Case remains proposal-only: even placing it
    // in the project cannot rewrite durable human intent, epoch, or lenses.
    fs::write(
        app.join("assurance-case.yaml"),
        include_str!("../../../contracts/assurance/artifact-only-progress-assurance.yaml"),
    )
    .expect("proposal-only assurance file");
    let after_proposal = ok(&run(&app_arg, &["resume"]));
    assert_eq!(
        after_proposal["data"]["durable_assurance"]["projection"],
        (*projection).clone()
    );

    let next_packet = after_proposal["data"]["authorization"]["action_packets"]
        .as_array()
        .expect("next action packets")
        .iter()
        .find(|candidate| candidate["authorization_kind"] == "intent_revision")
        .expect("next intent revision packet");
    let next_packet_digest = next_packet["packet_digest"]
        .as_str()
        .expect("next packet digest");

    let wrong_kind = signed_envelope(
        &key,
        audience,
        project_id,
        next_packet_digest,
        WorkflowBrokerSemanticInput::Signal {
            active: true,
            basis_refs: vec!["README.md".to_owned()],
        },
        now(),
        now() + 120,
        "human-intent-e2e-wrong-kind-0002",
    );
    let wrong_path = write_envelope(&parent, "wrong-kind.json", &wrong_kind);
    let before_wrong = file_snapshot(&state_root);
    let wrong = failed(&run(
        &app_arg,
        &[
            "intent",
            "record",
            "--origin-envelope-file",
            &wrong_path.display().to_string(),
        ],
    ));
    assert!(wrong["error"]["message"]
        .as_str()
        .expect("wrong-kind error")
        .contains("only an intent_revision envelope"));
    assert_eq!(file_snapshot(&state_root), before_wrong);

    let stale_now = now();
    let stale = signed_envelope(
        &key,
        audience,
        project_id,
        next_packet_digest,
        semantic_input,
        stale_now - 1_000,
        stale_now - 900,
        "human-intent-e2e-stale-event-0003",
    );
    let stale_path = write_envelope(&parent, "stale-intent.json", &stale);
    let before_stale = file_snapshot(&state_root);
    let stale_failure = failed(&run(
        &app_arg,
        &[
            "intent",
            "record",
            "--origin-envelope-file",
            &stale_path.display().to_string(),
        ],
    ));
    assert!(stale_failure["error"]["message"]
        .as_str()
        .expect("stale error")
        .contains("workflow action rejected"));
    assert_eq!(file_snapshot(&state_root), before_stale);

    let _ = fs::remove_dir_all(parent);
}
