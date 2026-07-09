use assert_cmd::Command;
use forge_core_contracts::{AssuranceCaseDocument, ReadinessVerdict};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary must exist")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

fn obligation_fixture(name: &str) -> PathBuf {
    repo_root()
        .join("docs/fixtures/obligation-engine-v0")
        .join(name)
}

fn fresh_dir(label: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = repo_root().join("target").join(format!(
        "assurance-adapter-e2e-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create fresh dir");
    dir
}

fn output_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout should be JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn derive_json(fixture: &str) -> Value {
    let output = bin()
        .args(["assurance", "derive", "--input-file"])
        .arg(obligation_fixture(fixture))
        .arg("--json")
        .output()
        .expect("run assurance derive");
    assert!(
        output.status.success(),
        "derive should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output_json(&output)
}

#[test]
fn chat_goal_can_begin_exploration_without_hiding_future_gaps() {
    let envelope = derive_json("chat-goal-explore.yaml");
    let response = &envelope["data"]["response"];

    assert_eq!(envelope["command"], "assurance derive");
    assert_eq!(envelope["data"]["result"], "guidance");
    assert_eq!(response["guidance"]["target"], "explore");
    assert_eq!(response["guidance"]["verdict"], "ready");
    assert_eq!(
        response["guidance"]["human_attention"]["status"],
        "not_required"
    );
    assert_eq!(response["guidance"]["next_action"]["kind"], "proceed");
    assert!(
        response["assurance_case"]["assurance_case"]["capability_gaps"]
            .as_array()
            .is_some_and(|gaps| !gaps.is_empty())
    );
    assert!(response["resume_token"]
        .as_str()
        .is_some_and(|token| token.starts_with("sha256:")));
}

#[test]
fn flag_only_surface_supports_order_independent_mcp_invocation() {
    let fixture = obligation_fixture("chat-goal-explore.yaml");
    let relative = fixture
        .strip_prefix(repo_root())
        .expect("fixture below repo root");
    let output = bin()
        .args(["assurance", "--input-file"])
        .arg(relative)
        .arg("--json")
        .arg("--root")
        .arg(repo_root())
        .output()
        .expect("run flag-only assurance Adapter");

    assert!(output.status.success(), "flag-only Adapter should succeed");
    let envelope = output_json(&output);
    assert_eq!(envelope["data"]["result"], "guidance");
    assert_eq!(
        envelope["data"]["response"]["guidance"]["target"],
        "explore"
    );
}

#[test]
fn empty_consumer_reaches_guidance_and_replacement_agent_resume_without_human_artifacts() {
    let consumer = fresh_dir("empty-consumer");
    assert_eq!(
        std::fs::read_dir(&consumer)
            .expect("read empty consumer")
            .count(),
        0
    );

    let host_dir = consumer.join(".host-agent");
    std::fs::create_dir_all(&host_dir).expect("host creates private working state");
    let input_path = host_dir.join("obligation-engine-input.yaml");
    std::fs::copy(obligation_fixture("chat-goal-explore.yaml"), &input_path)
        .expect("host materializes typed proposal");

    let derived_output = bin()
        .args(["assurance", "--root"])
        .arg(&consumer)
        .args([
            "--input-file",
            ".host-agent/obligation-engine-input.yaml",
            "--json",
        ])
        .output()
        .expect("derive from empty consumer");
    assert!(derived_output.status.success(), "derive should succeed");
    let derived = output_json(&derived_output);
    let response = &derived["data"]["response"];
    assert_eq!(response["guidance"]["verdict"], "ready");

    let case: AssuranceCaseDocument =
        serde_json::from_value(response["assurance_case"].clone()).expect("typed case");
    let case_path = host_dir.join("assurance-case.yaml");
    std::fs::write(
        &case_path,
        yaml_serde::to_string(&case).expect("serialize durable case"),
    )
    .expect("host persists durable case");

    let resumed_output = bin()
        .args([
            "assurance",
            "--case-file",
            ".host-agent/assurance-case.yaml",
        ])
        .arg("--root")
        .arg(&consumer)
        .arg("--json")
        .output()
        .expect("replacement agent resumes");
    assert!(resumed_output.status.success(), "resume should succeed");
    let resumed = output_json(&resumed_output);

    assert_eq!(
        response["resume_token"],
        resumed["data"]["response"]["resume_token"]
    );
    assert_eq!(
        response["guidance"],
        resumed["data"]["response"]["guidance"]
    );
}

#[test]
fn blocked_readiness_is_valid_guidance_not_a_command_failure() {
    let envelope = derive_json("novel-domain-execute.yaml");
    let response = &envelope["data"]["response"];

    assert_eq!(envelope["ok"], true);
    assert_eq!(response["guidance"]["verdict"], "blocked");
    assert_eq!(
        response["guidance"]["human_attention"]["status"],
        "not_required"
    );
    assert!(!response["guidance"]["blocker_refs"]
        .as_array()
        .expect("blockers")
        .is_empty());
}

#[test]
fn persisted_case_resumes_with_identical_token_guidance_and_authority_state() {
    let derived = derive_json("artifact-only-release.yaml");
    let response = &derived["data"]["response"];
    let case: AssuranceCaseDocument =
        serde_json::from_value(response["assurance_case"].clone()).expect("typed case");
    let dir = fresh_dir("resume");
    let case_path = dir.join("assurance-case.yaml");
    std::fs::write(
        &case_path,
        yaml_serde::to_string(&case).expect("serialize case"),
    )
    .expect("persist case as the host would");

    let output = bin()
        .args(["assurance", "resume", "--case-file"])
        .arg(&case_path)
        .arg("--json")
        .output()
        .expect("run assurance resume");
    assert!(output.status.success(), "resume should succeed");
    let resumed = output_json(&output);
    let resumed_response = &resumed["data"]["response"];

    assert_eq!(resumed_response["source"], "resumed");
    assert_eq!(response["resume_token"], resumed_response["resume_token"]);
    assert_eq!(response["guidance"], resumed_response["guidance"]);
    assert_eq!(
        response["assurance_case"],
        resumed_response["assurance_case"]
    );
}

#[test]
fn inconsistent_host_input_returns_structured_self_correction_issues() {
    let dir = fresh_dir("invalid-input");
    let input_path = dir.join("invalid-input.yaml");
    let input = std::fs::read_to_string(obligation_fixture("verified-release.yaml"))
        .expect("read fixture")
        .replacen("schema_version: \"0.1\"", "schema_version: \"999\"", 1);
    std::fs::write(&input_path, input).expect("write invalid input");

    let output = bin()
        .args(["assurance", "derive", "--input-file"])
        .arg(&input_path)
        .arg("--json")
        .output()
        .expect("run invalid derive");
    assert_eq!(output.status.code(), Some(3));
    let envelope = output_json(&output);

    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["data"]["result"], "input_rejected");
    assert!(envelope["data"]["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .any(|issue| issue["code"] == "unsupported_schema_version"));
}

#[test]
fn resume_rejects_semantically_incoherent_case_with_diagnostics() {
    let derived = derive_json("novel-domain-execute.yaml");
    let mut case: AssuranceCaseDocument =
        serde_json::from_value(derived["data"]["response"]["assurance_case"].clone())
            .expect("typed case");
    assert_eq!(
        case.assurance_case.readiness.verdict,
        ReadinessVerdict::Blocked
    );
    case.assurance_case.readiness.blocker_refs.clear();
    let dir = fresh_dir("invalid-resume");
    let case_path = dir.join("invalid-case.yaml");
    std::fs::write(
        &case_path,
        yaml_serde::to_string(&case).expect("serialize case"),
    )
    .expect("write invalid case");

    let output = bin()
        .args(["assurance", "resume", "--case-file"])
        .arg(&case_path)
        .arg("--json")
        .output()
        .expect("run invalid resume");
    assert_eq!(output.status.code(), Some(3));
    let envelope = output_json(&output);

    assert_eq!(envelope["data"]["result"], "case_rejected");
    assert!(!envelope["data"]["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .is_empty());
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenPathDocument {
    schema_version: String,
    scenario: GoldenPathScenario,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenPathScenario {
    id: String,
    human_input: String,
    human_actions: Vec<String>,
    forbidden_human_actions: Vec<String>,
    host_actions: Vec<String>,
    obligation_engine_input_ref: String,
    expected_first_projection: ExpectedProjection,
    resume: ResumeExpectation,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedProjection {
    target: String,
    verdict: String,
    human_attention: String,
    first_action_kind: String,
    future_gap_required: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResumeExpectation {
    persisted_value: String,
    continuity_key: String,
    second_agent_command: String,
    required_equal_fields: Vec<String>,
}

#[test]
fn conversational_golden_path_fixture_keeps_human_chat_only_and_resume_explicit() {
    let path =
        repo_root().join("docs/fixtures/assurance-adapter-v0/conversational-golden-path.yaml");
    let text = std::fs::read_to_string(&path).expect("read golden path fixture");
    let fixture: GoldenPathDocument = yaml_serde::from_str(&text).expect("typed fixture");

    assert_eq!(fixture.schema_version, "0.1");
    assert_eq!(fixture.scenario.id, "conversational_golden_path");
    assert!(!fixture.scenario.human_input.trim().is_empty());
    assert_eq!(fixture.scenario.human_actions, ["state_goal_in_chat"]);
    assert!(fixture
        .scenario
        .forbidden_human_actions
        .contains(&"edit_forge_yaml".to_owned()));
    assert!(fixture
        .scenario
        .forbidden_human_actions
        .contains(&"select_workflow".to_owned()));
    assert!(fixture
        .scenario
        .host_actions
        .contains(&"persist_returned_assurance_case".to_owned()));
    assert!(repo_root()
        .join(&fixture.scenario.obligation_engine_input_ref)
        .is_file());
    assert_eq!(fixture.scenario.expected_first_projection.target, "explore");
    assert_eq!(fixture.scenario.expected_first_projection.verdict, "ready");
    assert_eq!(
        fixture.scenario.expected_first_projection.human_attention,
        "not_required"
    );
    assert_eq!(
        fixture.scenario.expected_first_projection.first_action_kind,
        "proceed"
    );
    assert!(
        fixture
            .scenario
            .expected_first_projection
            .future_gap_required
    );
    assert_eq!(fixture.scenario.resume.persisted_value, "assurance_case");
    assert_eq!(fixture.scenario.resume.continuity_key, "resume_token");
    assert_eq!(
        fixture.scenario.resume.second_agent_command,
        "assurance resume"
    );
    assert_eq!(
        fixture.scenario.resume.required_equal_fields,
        ["resume_token", "guidance", "assurance_case"]
    );
}
