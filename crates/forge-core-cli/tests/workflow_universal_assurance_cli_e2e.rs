//! Public P7b.2 journey from human intent to independently verified Assurance lenses.
//!
//! This intentionally drives only the agent-facing CLI and signed external
//! broker boundary. The representative-slice manifest is powerless until a
//! reviewer binds its exact bytes, and runtime observations stay partial until
//! every declared scenario passes from a separate origin domain.

use assert_cmd::Command;
use ed25519_dalek::{Signer, SigningKey};
use forge_core_authority::{
    workflow_broker_event_signing_bytes, WorkflowBrokerEventEnvelope, WorkflowBrokerIssuerProfile,
    WorkflowBrokerSemanticInput, WORKFLOW_BROKER_EVENT_SCHEMA_VERSION,
};
use forge_core_contracts::{
    PrincipalId, StableId, WorkflowEvidenceOutcome, WorkflowEvidenceSubjectKind,
    WorkflowRepresentativeEnvironment, WorkflowRepresentativeFailureMode,
    WorkflowRepresentativeScenarioReference, WorkflowRepresentativeSliceDefinition,
    WorkflowRepresentativeSliceDefinitionDocument, WORKFLOW_REPRESENTATIVE_SLICE_SCHEMA_VERSION,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const HUMAN_ISSUER: &str = "broker.assurance.human";
const REVIEWER_ISSUER: &str = "broker.assurance.reviewer";
const RUNTIME_ISSUER: &str = "broker.assurance.runtime";
const SAME_DOMAIN_RUNTIME_ISSUER: &str = "broker.assurance.runtime.same-domain";
const HUMAN_DOMAIN: &str = "domain.human.product-owner";
const REVIEWER_DOMAIN: &str = "domain.review.independent";
const RUNTIME_DOMAIN: &str = "domain.runtime.reference";
const RUNTIME_SUBJECT: &str = "runtime.reference.windows-amd";

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary")
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn runtime_subject_digest(subject_ref: &str) -> String {
    let identity = serde_json::json!({
        "schema_version": "workflow_broker_subject_identity_v1",
        "subject_kind": "runtime",
        "subject_ref": subject_ref,
    });
    digest(
        &serde_json_canonicalizer::to_vec(&identity).expect("canonical runtime subject identity"),
    )
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn output_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON envelope: {error}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn ok(output: &std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "command failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output_json(output)
}

fn workflow(app: &str, tail: &[&str]) -> std::process::Output {
    let mut command = bin();
    command.arg("workflow");
    command.args(tail);
    command.args(["--root", app, "--json"]);
    command.output().expect("workflow command")
}

fn upgrade_to_latest(app: &str) {
    for _ in 0..8 {
        let status = ok(&workflow(app, &["release-status"]));
        if status["data"]["available_successor"].is_null() {
            assert_eq!(
                status["data"]["active"]["release"]["release_id"],
                "workflow-governance.release.universal-assurance-v0"
            );
            return;
        }
        let argv = status["data"]["upgrade_argv"]
            .as_array()
            .expect("release upgrade argv")
            .iter()
            .map(|value| value.as_str().expect("argv item").to_owned())
            .collect::<Vec<_>>();
        ok(&bin().args(&argv[1..]).output().expect("release upgrade"));
    }
    panic!("release chain did not converge to the latest release");
}

#[derive(Clone, Copy)]
struct Broker<'a> {
    issuer_id: &'static str,
    profile: WorkflowBrokerIssuerProfile,
    principal_id: &'static str,
    separation_domain: &'static str,
    key: &'a SigningKey,
}

fn trust_broker(parent: &Path, app: &str, broker: &Broker<'_>) -> Value {
    let key_path = parent.join(format!("{}.pub", broker.issuer_id));
    let ceremony_path = parent.join(format!("{}.ceremony", broker.issuer_id));
    fs::write(&key_path, hex(&broker.key.verifying_key().to_bytes())).expect("public key");
    fs::write(
        &ceremony_path,
        format!("operator enrolled {} outside the agent\n", broker.issuer_id),
    )
    .expect("ceremony");
    let profile = match broker.profile {
        WorkflowBrokerIssuerProfile::Human => "human",
        WorkflowBrokerIssuerProfile::Reviewer => "reviewer",
        WorkflowBrokerIssuerProfile::Runtime => "runtime",
    };
    ok(&workflow(
        app,
        &[
            "broker",
            "trust",
            "--issuer-id",
            broker.issuer_id,
            "--profile",
            profile,
            "--public-key-file",
            &key_path.display().to_string(),
            "--ceremony-ref",
            &format!("operator://ceremony/{}", broker.issuer_id),
            "--ceremony-file",
            &ceremony_path.display().to_string(),
        ],
    ))
}

#[allow(clippy::too_many_arguments)] // Every signed wire coordinate remains visible in this authority proof.
fn envelope(
    broker: &Broker<'_>,
    audience: &str,
    project_id: &str,
    packet_digest: &str,
    semantic_input: WorkflowBrokerSemanticInput,
    nonce: &str,
) -> WorkflowBrokerEventEnvelope {
    let issued_at_unix = now();
    let mut envelope = WorkflowBrokerEventEnvelope {
        schema_version: WORKFLOW_BROKER_EVENT_SCHEMA_VERSION.to_owned(),
        audience: audience.to_owned(),
        issuer_id: StableId(broker.issuer_id.to_owned()),
        issuer_profile: broker.profile,
        origin_principal_id: PrincipalId(broker.principal_id.to_owned()),
        separation_domain: StableId(broker.separation_domain.to_owned()),
        event_kind: semantic_input.kind(),
        project_id: StableId(project_id.to_owned()),
        action_packet_digest: packet_digest.to_owned(),
        semantic_input,
        issued_at_unix,
        expires_at_unix: issued_at_unix + 300,
        nonce: format!("p7b2-{nonce}-nonce"),
        signature: String::new(),
    };
    let bytes = workflow_broker_event_signing_bytes(&envelope).expect("broker signing bytes");
    envelope.signature = hex(&broker.key.sign(&bytes).to_bytes());
    envelope
}

fn apply_envelope(
    app: &str,
    parent: &Path,
    label: &str,
    envelope: &WorkflowBrokerEventEnvelope,
) -> Value {
    let path = parent.join(format!("{label}.json"));
    fs::write(
        &path,
        serde_json::to_vec_pretty(envelope).expect("serialize broker envelope"),
    )
    .expect("write broker envelope");
    let output = workflow(
        app,
        &[
            "action",
            "apply",
            "--origin-envelope-file",
            &path.display().to_string(),
        ],
    );
    assert!(
        output.status.success(),
        "apply {label} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output_json(&output)
}

fn packets(app: &str) -> Value {
    ok(&workflow(app, &["action-packets"]))["data"]["packets"].clone()
}

fn packet_for_claim<'a>(packets: &'a Value, claim_ref: &str) -> &'a Value {
    packets
        .as_array()
        .expect("action packets")
        .iter()
        .find(|packet| packet["input_contract"]["claim_ref"] == claim_ref)
        .unwrap_or_else(|| panic!("missing packet for {claim_ref}: {packets:#}"))
}

#[derive(Clone, Copy)]
struct EvidenceInput<'a> {
    claim_ref: &'a str,
    broker: &'a Broker<'a>,
    outcome: WorkflowEvidenceOutcome,
    subject_kind: WorkflowEvidenceSubjectKind,
    subject_ref: &'a str,
    scenario_ref: &'a str,
    nonce: &'a str,
}

fn apply_evidence(
    app: &str,
    parent: &Path,
    audience: &str,
    project_id: &str,
    input: EvidenceInput<'_>,
) -> Value {
    let current = packets(app);
    let packet = packet_for_claim(&current, input.claim_ref);
    let signed = envelope(
        input.broker,
        audience,
        project_id,
        packet["packet_digest"].as_str().expect("packet digest"),
        WorkflowBrokerSemanticInput::Evidence {
            outcome: input.outcome,
            subject_kind: input.subject_kind,
            subject_ref: input.subject_ref.to_owned(),
            scenario_ref: input.scenario_ref.to_owned(),
        },
        input.nonce,
    );
    apply_envelope(app, parent, input.nonce, &signed)
}

fn claim_state<'a>(projection: &'a Value, claim_ref: &str) -> &'a str {
    projection["lenses"]
        .as_array()
        .expect("lenses")
        .iter()
        .flat_map(|lens| lens["claims"].as_array().expect("lens claims"))
        .find(|claim| claim["claim_ref"] == claim_ref)
        .and_then(|claim| claim["state"].as_str())
        .unwrap_or_else(|| panic!("missing durable claim {claim_ref}: {projection:#}"))
}

#[test]
#[allow(clippy::too_many_lines)] // One chronological public journey minimizes repeated expensive CLI setup.
fn reviewed_definition_and_separate_complete_runtime_verify_universal_lenses() {
    let parent = std::env::temp_dir().join(format!(
        "forge-universal-assurance-cli-{}-{}",
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
    ok(&bin()
        .args(["start", "--root", &app_arg, "--json"])
        .output()
        .expect("start"));
    ok(&workflow(&app_arg, &["init"]));
    upgrade_to_latest(&app_arg);

    let human_key = SigningKey::from_bytes(&[31; 32]);
    let reviewer_key = SigningKey::from_bytes(&[32; 32]);
    let runtime_key = SigningKey::from_bytes(&[33; 32]);
    let same_domain_runtime_key = SigningKey::from_bytes(&[34; 32]);
    let human = Broker {
        issuer_id: HUMAN_ISSUER,
        profile: WorkflowBrokerIssuerProfile::Human,
        principal_id: "principal.human.product-owner",
        separation_domain: HUMAN_DOMAIN,
        key: &human_key,
    };
    let reviewer = Broker {
        issuer_id: REVIEWER_ISSUER,
        profile: WorkflowBrokerIssuerProfile::Reviewer,
        principal_id: "principal.reviewer.independent",
        separation_domain: REVIEWER_DOMAIN,
        key: &reviewer_key,
    };
    let runtime = Broker {
        issuer_id: RUNTIME_ISSUER,
        profile: WorkflowBrokerIssuerProfile::Runtime,
        principal_id: "principal.runtime.reference",
        separation_domain: RUNTIME_DOMAIN,
        key: &runtime_key,
    };
    let same_domain_runtime = Broker {
        issuer_id: SAME_DOMAIN_RUNTIME_ISSUER,
        principal_id: "principal.runtime.same-domain",
        separation_domain: REVIEWER_DOMAIN,
        key: &same_domain_runtime_key,
        ..runtime
    };
    let trusted = trust_broker(&parent, &app_arg, &human);
    trust_broker(&parent, &app_arg, &reviewer);
    trust_broker(&parent, &app_arg, &runtime);
    trust_broker(&parent, &app_arg, &same_domain_runtime);
    let audience = trusted["data"]["audience"]
        .as_str()
        .expect("broker audience");

    let next = ok(&workflow(&app_arg, &["next"]));
    let intent_packet = &next["data"]["authorization"]["action_packets"][0];
    assert_eq!(intent_packet["authorization_kind"], "intent_revision");
    let project_id = next["data"]["project_id"].as_str().expect("project id");
    let intent = envelope(
        &human,
        audience,
        project_id,
        intent_packet["packet_digest"]
            .as_str()
            .expect("intent packet"),
        WorkflowBrokerSemanticInput::IntentRevision {
            desired_outcome: "A dependable agent-built game that is fun on first launch".to_owned(),
            constraints: vec!["Runs offline on the target machine".to_owned()],
            preferences: vec!["Short feedback loops".to_owned()],
            unacceptable_outcomes: vec!["Broken core controls on first launch".to_owned()],
            uncertainties: vec!["Final art direction is not selected".to_owned()],
            conversation_ref: "conversation://codex/universal-assurance/intent".to_owned(),
            conversation_digest: digest(b"bounded source conversation"),
        },
        "intent-0001",
    );
    apply_envelope(&app_arg, &parent, "intent-0001", &intent);
    let after_intent = ok(&workflow(&app_arg, &["resume"]));
    let intent_digest = after_intent["data"]["durable_assurance"]["projection"]["binding"]
        ["intent_digest"]
        .as_str()
        .expect("intent digest")
        .to_owned();

    let assurance = app.join("assurance");
    fs::create_dir_all(&assurance).expect("assurance directory");
    let first_ref = "assurance/first-launch.yaml";
    let second_ref = "assurance/recovery.yaml";
    let first = b"scenario: first launch reaches playable control\n";
    let second = b"scenario: failed launch recovers without corrupting progress\n";
    fs::write(app.join(first_ref), first).expect("first scenario");
    fs::write(app.join(second_ref), second).expect("second scenario");
    let manifest_ref = "assurance/representative-slice.yaml";
    let manifest = WorkflowRepresentativeSliceDefinitionDocument {
        schema_version: WORKFLOW_REPRESENTATIVE_SLICE_SCHEMA_VERSION.to_owned(),
        representative_slice: WorkflowRepresentativeSliceDefinition {
            intent_digest,
            critical_journey: "A novice reaches responsive core play on first launch".to_owned(),
            falsifier: "Either first launch or recovery leaves the game unusable".to_owned(),
            representative_environment: WorkflowRepresentativeEnvironment {
                runtime_subject_ref: RUNTIME_SUBJECT.to_owned(),
                runtime_subject_digest: runtime_subject_digest(RUNTIME_SUBJECT),
                expectation: "Target-class offline runtime with production-equivalent controls"
                    .to_owned(),
            },
            scenarios: vec![
                WorkflowRepresentativeScenarioReference {
                    scenario_ref: first_ref.to_owned(),
                    declared_scenario_digest: digest(first),
                    failure_mode_refs: vec![StableId("failure.first-launch".to_owned())],
                },
                WorkflowRepresentativeScenarioReference {
                    scenario_ref: second_ref.to_owned(),
                    declared_scenario_digest: digest(second),
                    failure_mode_refs: vec![StableId("failure.recovery".to_owned())],
                },
            ],
            material_failure_modes: vec![
                WorkflowRepresentativeFailureMode {
                    id: StableId("failure.first-launch".to_owned()),
                    description: "Core input is unusable on first launch".to_owned(),
                },
                WorkflowRepresentativeFailureMode {
                    id: StableId("failure.recovery".to_owned()),
                    description: "Recovery corrupts progress or remains unplayable".to_owned(),
                },
            ],
        },
    };
    fs::write(
        app.join(manifest_ref),
        yaml_serde::to_string(&manifest).expect("serialize representative slice"),
    )
    .expect("representative-slice manifest");

    // Capture the review packet, then advance the ledger with another exact
    // packet. The stale review must fail before any new receipt is written.
    let before_evidence = packets(&app_arg);
    let stale_definition_packet = packet_for_claim(
        &before_evidence,
        "claim.workflow.universal-assurance.representative-slice-defined",
    );
    let stale_definition = envelope(
        &reviewer,
        audience,
        project_id,
        stale_definition_packet["packet_digest"]
            .as_str()
            .expect("definition packet"),
        WorkflowBrokerSemanticInput::Evidence {
            outcome: WorkflowEvidenceOutcome::Pass,
            subject_kind: WorkflowEvidenceSubjectKind::Artifact,
            subject_ref: manifest_ref.to_owned(),
            scenario_ref: manifest_ref.to_owned(),
        },
        "definition-stale-0002",
    );
    apply_evidence(
        &app_arg,
        &parent,
        audience,
        project_id,
        EvidenceInput {
            claim_ref: "claim.workflow.universal-assurance.intended-outcome",
            broker: &human,
            outcome: WorkflowEvidenceOutcome::Pass,
            subject_kind: WorkflowEvidenceSubjectKind::HumanDecision,
            subject_ref: "human.intent.accepted",
            scenario_ref: first_ref,
            nonce: "lens-intent-0003",
        },
    );
    let stale_path = parent.join("definition-stale-0002.json");
    fs::write(
        &stale_path,
        serde_json::to_vec_pretty(&stale_definition).expect("stale envelope"),
    )
    .expect("stale envelope file");
    let stale_output = workflow(
        &app_arg,
        &[
            "action",
            "apply",
            "--origin-envelope-file",
            &stale_path.display().to_string(),
        ],
    );
    assert!(!stale_output.status.success(), "stale review must fail");
    assert!(output_json(&stale_output)["error"]["message"]
        .as_str()
        .expect("stale error")
        .contains("rejected"));

    apply_evidence(
        &app_arg,
        &parent,
        audience,
        project_id,
        EvidenceInput {
            claim_ref: "claim.workflow.universal-assurance.representative-slice-defined",
            broker: &reviewer,
            outcome: WorkflowEvidenceOutcome::Pass,
            subject_kind: WorkflowEvidenceSubjectKind::Artifact,
            subject_ref: manifest_ref,
            scenario_ref: manifest_ref,
            nonce: "definition-current-0004",
        },
    );

    // A runtime in the reviewer's separation domain can be recorded for
    // audit, but it cannot verify the representative execution claim.
    apply_evidence(
        &app_arg,
        &parent,
        audience,
        project_id,
        EvidenceInput {
            claim_ref: "claim.workflow.universal-assurance.representative-slice-executed",
            broker: &same_domain_runtime,
            outcome: WorkflowEvidenceOutcome::Pass,
            subject_kind: WorkflowEvidenceSubjectKind::Runtime,
            subject_ref: RUNTIME_SUBJECT,
            scenario_ref: first_ref,
            nonce: "execution-wrong-domain-0005",
        },
    );
    let wrong_domain = ok(&workflow(&app_arg, &["resume"]));
    assert_eq!(
        claim_state(
            &wrong_domain["data"]["durable_assurance"]["projection"],
            "claim.workflow.universal-assurance.representative-slice-executed"
        ),
        "supported"
    );

    apply_evidence(
        &app_arg,
        &parent,
        audience,
        project_id,
        EvidenceInput {
            claim_ref: "claim.workflow.universal-assurance.representative-slice-executed",
            broker: &runtime,
            outcome: WorkflowEvidenceOutcome::Pass,
            subject_kind: WorkflowEvidenceSubjectKind::Runtime,
            subject_ref: RUNTIME_SUBJECT,
            scenario_ref: first_ref,
            nonce: "execution-partial-0006",
        },
    );
    let partial = ok(&workflow(&app_arg, &["resume"]));
    assert_eq!(
        claim_state(
            &partial["data"]["durable_assurance"]["projection"],
            "claim.workflow.universal-assurance.representative-slice-executed"
        ),
        "supported"
    );
    apply_evidence(
        &app_arg,
        &parent,
        audience,
        project_id,
        EvidenceInput {
            claim_ref: "claim.workflow.universal-assurance.representative-slice-executed",
            broker: &runtime,
            outcome: WorkflowEvidenceOutcome::Pass,
            subject_kind: WorkflowEvidenceSubjectKind::Runtime,
            subject_ref: RUNTIME_SUBJECT,
            scenario_ref: second_ref,
            nonce: "execution-complete-0007",
        },
    );

    let lens_evidence = [
        (
            "claim.workflow.universal-assurance.critical-journeys",
            &runtime,
            WorkflowEvidenceSubjectKind::ProjectSnapshot,
            project_id,
        ),
        (
            "claim.workflow.universal-assurance.system-integrity",
            &reviewer,
            WorkflowEvidenceSubjectKind::ProjectSnapshot,
            project_id,
        ),
        (
            "claim.workflow.universal-assurance.quality-attributes",
            &runtime,
            WorkflowEvidenceSubjectKind::ProjectSnapshot,
            project_id,
        ),
        (
            "claim.workflow.universal-assurance.operability",
            &runtime,
            WorkflowEvidenceSubjectKind::Runtime,
            RUNTIME_SUBJECT,
        ),
        (
            "claim.workflow.universal-assurance.lifecycle-coverage",
            &runtime,
            WorkflowEvidenceSubjectKind::ProjectSnapshot,
            project_id,
        ),
        (
            "claim.workflow.universal-assurance.risk-and-failure",
            &reviewer,
            WorkflowEvidenceSubjectKind::ProjectSnapshot,
            project_id,
        ),
        (
            "claim.workflow.universal-assurance.evidence-representativeness",
            &runtime,
            WorkflowEvidenceSubjectKind::Runtime,
            RUNTIME_SUBJECT,
        ),
    ];
    for (index, (claim_ref, broker, subject_kind, subject_ref)) in
        lens_evidence.into_iter().enumerate()
    {
        let nonce = format!("lens-{index:02}-0008");
        apply_evidence(
            &app_arg,
            &parent,
            audience,
            project_id,
            EvidenceInput {
                claim_ref,
                broker,
                outcome: WorkflowEvidenceOutcome::Pass,
                subject_kind,
                subject_ref,
                scenario_ref: first_ref,
                nonce: &nonce,
            },
        );
    }

    let resumed = ok(&workflow(&app_arg, &["resume"]));
    let durable = &resumed["data"]["durable_assurance"];
    assert_eq!(durable["status"], "intent_accepted");
    let projection = &durable["projection"];
    assert_eq!(
        projection["readiness"], "ready",
        "universal assurance remained blocked: {projection:#}"
    );
    assert_eq!(projection["blocker_lenses"], serde_json::json!([]));
    assert_eq!(
        claim_state(
            projection,
            "claim.workflow.universal-assurance.representative-slice-defined"
        ),
        "verified"
    );
    assert_eq!(
        claim_state(
            projection,
            "claim.workflow.universal-assurance.representative-slice-executed"
        ),
        "verified"
    );
    assert!(projection["lenses"]
        .as_array()
        .expect("lenses")
        .iter()
        .all(|lens| lens["claim_status"] == "verified"));

    let _ = fs::remove_dir_all(parent);
}
