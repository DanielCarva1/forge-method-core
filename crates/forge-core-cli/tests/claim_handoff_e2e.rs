use assert_cmd::Command;
use forge_core_cli::claim::ClaimHandoffArtifact;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};

const NOW: i64 = 1_800_000_000;
const TTL_SECONDS: i64 = 10;

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

fn fresh_parent(label: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let root = repo_root().join("target").join(format!(
        "claim-handoff-e2e-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create fresh parent");
    root
}

fn output_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout should be json: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(output: &Output, label: &str) -> Value {
    assert!(
        output.status.success(),
        "{label} should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(output);
    assert_eq!(json["ok"], true, "{label} should report ok");
    json
}

fn assert_failure(output: &Output, label: &str) -> Value {
    assert!(
        !output.status.success(),
        "{label} should fail closed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(output);
    assert_eq!(json["ok"], false, "{label} should report not ok");
    json
}

fn assert_rejected_with_code(output: &Output, label: &str, expected_code: &str) -> Value {
    let json = assert_failure(output, label);
    let code = json["error"]["code"].as_str().unwrap_or_default();
    let message = json["error"]["message"].as_str().unwrap_or_default();
    assert!(
        code.contains(expected_code) || message.contains(expected_code),
        "{label} should include rejection code '{expected_code}'\njson:\n{json:#}"
    );
    json
}

fn active_claims(json: &Value) -> &[Value] {
    json["data"]["active"]
        .as_array()
        .expect("active claims array")
}

fn entry_count(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }
    std::fs::read_dir(path)
        .expect("read directory")
        .filter_map(Result::ok)
        .count()
}

fn write_evidence(parent: &Path, label: &str) -> PathBuf {
    let evidence = parent.join(format!("{label}-evidence.txt"));
    std::fs::write(&evidence, format!("deterministic evidence for {label}\n"))
        .expect("write evidence");
    evidence
}

fn resolve_handoff_path(state_root: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return path;
    }

    let under_state_root = state_root.join(&path);
    if under_state_root.exists() {
        return under_state_root;
    }

    repo_root().join(path)
}

fn assert_handoff_artifact(
    json: &Value,
    state_root: &Path,
    summary: &str,
    evidence: &Path,
    expected_original_agent: &str,
    expected_scope_id: &str,
    expected_path: &str,
) -> PathBuf {
    let raw_path = json["data"]["handoff_path"]
        .as_str()
        .expect("handoff_path string");
    let handoff_path = resolve_handoff_path(state_root, raw_path);
    let expected_root = state_root.join("handoffs").join("expired-claims");

    assert!(
        handoff_path.exists(),
        "handoff artifact should exist at {}",
        handoff_path.display()
    );
    assert!(
        handoff_path.starts_with(&expected_root),
        "handoff artifact should be under {}, got {}",
        expected_root.display(),
        handoff_path.display()
    );

    let artifact = std::fs::read_to_string(&handoff_path).expect("read handoff artifact");
    assert!(
        artifact.contains(summary),
        "handoff artifact should include the operator summary\nartifact:\n{artifact}"
    );
    let evidence_name = evidence
        .file_name()
        .and_then(|name| name.to_str())
        .expect("utf-8 evidence file name");
    assert!(
        artifact.contains(evidence_name),
        "handoff artifact should include evidence reference '{evidence_name}'\nartifact:\n{artifact}"
    );
    let parsed: ClaimHandoffArtifact =
        yaml_serde::from_str(&artifact).expect("handoff artifact should deserialize");
    assert_eq!(
        parsed.claim_id,
        json["data"]["claim_id"].as_str().expect("claim_id string")
    );
    assert_eq!(parsed.scope_id, expected_scope_id);
    assert_eq!(parsed.original_claimant_agent_id, expected_original_agent);
    assert_eq!(parsed.previous_status, "active");
    assert_eq!(parsed.recorded_status, "handoff_recorded");
    assert_eq!(parsed.summary, summary);
    assert_eq!(
        parsed.claim_contract.claim.claimant_agent_id.0,
        expected_original_agent
    );
    assert_eq!(parsed.claim_contract.scope.id.0, expected_scope_id);
    assert!(
        parsed
            .claim_contract
            .scope
            .paths
            .iter()
            .any(|path| path.0 == expected_path),
        "embedded claim should preserve path {expected_path}: {parsed:#?}"
    );
    assert!(
        parsed.claim_contract.lease.expires_at > parsed.claim_contract.lease.acquired_at,
        "embedded claim should preserve a valid lease: {parsed:#?}"
    );

    handoff_path
}

struct ConsumerApp {
    app: PathBuf,
    state_root: PathBuf,
}

fn consumer_app(label: &str) -> ConsumerApp {
    let parent = fresh_parent(label);
    let app = parent.join("consumer-app");
    let sidecar = parent.join("forge-sidecar");
    let state_root = sidecar.join(".forge-method");

    std::fs::create_dir_all(&app).expect("create app root");
    std::fs::create_dir_all(&state_root).expect("create sidecar state root");
    std::fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: consumer-app\nsidecar_root: ../forge-sidecar\nstate_root: ../forge-sidecar/.forge-method\n",
    )
    .expect("write project link");

    ConsumerApp { app, state_root }
}

#[test]
#[allow(clippy::too_many_lines)]
fn expired_claim_handoff_records_artifact_and_unblocks_reacquire() {
    let parent = fresh_parent("explicit-claims-dir");
    let state_root = parent.join(".forge-method");
    let claims_dir = state_root.join("claims-active");
    let claims_arg = claims_dir.display().to_string();
    let evidence = write_evidence(&parent, "explicit");
    let evidence_arg = evidence.display().to_string();
    let path = "src/expired-owned.rs";
    let expired_at = NOW + TTL_SECONDS;
    let after_expiry = expired_at + 1;

    let acquire = bin()
        .args([
            "claim",
            "acquire",
            "--claims-dir",
            &claims_arg,
            "--scope",
            "story",
            "--id",
            "HANDOFF-E2E-EXPLICIT",
            "--agent",
            "alice",
            "--path",
            path,
            "--ttl",
            &TTL_SECONDS.to_string(),
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run initial claim acquire");
    let acquire_json = assert_success(&acquire, "initial claim acquire");
    let claim_id = acquire_json["data"]["claim_id"]
        .as_str()
        .expect("claim id")
        .to_string();

    let release = bin()
        .args([
            "claim",
            "release",
            "--claims-dir",
            &claims_arg,
            "--id",
            &claim_id,
            "--agent",
            "alice",
            "--now-unix",
            &after_expiry.to_string(),
        ])
        .output()
        .expect("run expired claim release");
    assert_rejected_with_code(
        &release,
        "expired claim release",
        "expired_requires_handoff",
    );

    let overlap_acquire = bin()
        .args([
            "claim",
            "acquire",
            "--claims-dir",
            &claims_arg,
            "--scope",
            "story",
            "--id",
            "HANDOFF-E2E-OVERLAP",
            "--agent",
            "bob",
            "--path",
            path,
            "--ttl",
            "600",
            "--now-unix",
            &after_expiry.to_string(),
        ])
        .output()
        .expect("run overlapping claim acquire");
    assert_rejected_with_code(
        &overlap_acquire,
        "overlapping claim acquire after expiry",
        "expired_requires_handoff",
    );

    let summary = "alice stopped after lease expiry; official handoff recorded";
    let handoff = bin()
        .args([
            "claim",
            "handoff",
            "--claims-dir",
            &claims_arg,
            "--id",
            &claim_id,
            "--agent",
            "alice",
            "--summary",
            summary,
            "--evidence",
            &evidence_arg,
            "--now-unix",
            &after_expiry.to_string(),
        ])
        .output()
        .expect("run official claim handoff");
    let handoff_json = assert_success(&handoff, "official claim handoff");
    assert_eq!(handoff_json["command"], "claim.handoff");
    assert_eq!(handoff_json["data"]["claim_id"], claim_id);
    assert_eq!(handoff_json["data"]["status"], "handoff_recorded");
    let handoff_path = assert_handoff_artifact(
        &handoff_json,
        &state_root,
        summary,
        &evidence,
        "alice",
        "HANDOFF-E2E-EXPLICIT",
        path,
    );

    let status_after_handoff = bin()
        .args([
            "claim",
            "status",
            "--claims-dir",
            &claims_arg,
            "--now-unix",
            &after_expiry.to_string(),
        ])
        .output()
        .expect("run status after handoff");
    let status_json = assert_success(&status_after_handoff, "status after handoff");
    assert!(
        active_claims(&status_json).is_empty(),
        "handoff-recorded claim must not remain active: {status_json:#}"
    );

    let reacquire = bin()
        .args([
            "claim",
            "acquire",
            "--claims-dir",
            &claims_arg,
            "--scope",
            "story",
            "--id",
            "HANDOFF-E2E-EXPLICIT",
            "--agent",
            "bob",
            "--path",
            path,
            "--ttl",
            "600",
            "--now-unix",
            &(after_expiry + 1).to_string(),
        ])
        .output()
        .expect("run reacquire after handoff");
    let reacquire_json = assert_success(&reacquire, "reacquire after handoff");
    assert_eq!(reacquire_json["data"]["status"], "active");
    assert_eq!(reacquire_json["data"]["agent_id"], "bob");
    assert!(
        handoff_path.exists(),
        "handoff artifact must remain durable after reacquire"
    );

    let status_after_reacquire = bin()
        .args([
            "claim",
            "status",
            "--claims-dir",
            &claims_arg,
            "--now-unix",
            &(after_expiry + 2).to_string(),
        ])
        .output()
        .expect("run status after reacquire");
    let status_json = assert_success(&status_after_reacquire, "status after reacquire");
    let active = active_claims(&status_json);
    assert!(
        active
            .iter()
            .any(|claim| claim["agent_id"] == "bob" && claim["scope_id"] == "HANDOFF-E2E-EXPLICIT"),
        "bob's reacquired claim should be active: {status_json:#}"
    );
    assert!(
        !active.iter().any(|claim| claim["agent_id"] == "alice"),
        "old expired claimant must not show as active: {status_json:#}"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn handoff_uses_sidecar_defaults_without_creating_consumer_state() {
    let fixture = consumer_app("sidecar-default");
    let app = fixture.app.display().to_string();
    let evidence = write_evidence(
        fixture.app.parent().expect("fixture parent"),
        "sidecar-default",
    );
    let evidence_arg = evidence.display().to_string();
    let after_expiry = NOW + TTL_SECONDS + 1;

    let acquire = bin()
        .args([
            "claim",
            "acquire",
            "--root",
            &app,
            "--scope",
            "story",
            "--id",
            "HANDOFF-E2E-SIDECAR",
            "--agent",
            "sidecar-alice",
            "--path",
            "src/sidecar.rs",
            "--ttl",
            &TTL_SECONDS.to_string(),
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run sidecar claim acquire");
    assert_success(&acquire, "sidecar claim acquire");
    assert!(
        fixture.state_root.join("claims-active").exists(),
        "claim acquire should create sidecar claims-active"
    );
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "claim acquire via --root must not create consumer-local .forge-method"
    );

    let summary = "sidecar default handoff resolution";
    let handoff = bin()
        .args([
            "claim",
            "handoff",
            "--root",
            &app,
            "--id",
            "HANDOFF-E2E-SIDECAR",
            "--agent",
            "sidecar-alice",
            "--summary",
            summary,
            "--evidence",
            &evidence_arg,
            "--now-unix",
            &after_expiry.to_string(),
        ])
        .output()
        .expect("run sidecar claim handoff");
    let handoff_json = assert_success(&handoff, "sidecar claim handoff");
    assert_eq!(
        handoff_json["data"]["claim_id"],
        "claim.story.HANDOFF-E2E-SIDECAR.HANDOFF-E2E-SIDECAR"
    );
    assert_eq!(handoff_json["data"]["status"], "handoff_recorded");
    let handoff_path = assert_handoff_artifact(
        &handoff_json,
        &fixture.state_root,
        summary,
        &evidence,
        "sidecar-alice",
        "HANDOFF-E2E-SIDECAR",
        "src/sidecar.rs",
    );
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "claim handoff via --root must not create consumer-local .forge-method"
    );

    let reacquire = bin()
        .args([
            "claim",
            "acquire",
            "--root",
            &app,
            "--scope",
            "story",
            "--id",
            "HANDOFF-E2E-SIDECAR",
            "--agent",
            "sidecar-bob",
            "--path",
            "src/sidecar.rs",
            "--ttl",
            "600",
            "--now-unix",
            &(after_expiry + 1).to_string(),
        ])
        .output()
        .expect("run sidecar reacquire after handoff");
    let reacquire_json = assert_success(&reacquire, "sidecar reacquire after handoff");
    assert_eq!(reacquire_json["data"]["agent_id"], "sidecar-bob");
    assert!(
        handoff_path.exists(),
        "sidecar handoff artifact must remain durable after reacquire"
    );
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "reacquire via --root must not create consumer-local .forge-method"
    );
}

#[test]
fn expired_claim_heartbeat_points_to_handoff_before_recovery() {
    let parent = fresh_parent("heartbeat-before-handoff");
    let claims_dir = parent.join(".forge-method").join("claims-active");
    let claims_arg = claims_dir.display().to_string();
    let path = "src/heartbeat-expired.rs";
    let expired_at = NOW + TTL_SECONDS;
    let after_expiry = expired_at + 1;

    let acquire = bin()
        .args([
            "claim",
            "acquire",
            "--claims-dir",
            &claims_arg,
            "--scope",
            "story",
            "--id",
            "HANDOFF-E2E-HEARTBEAT",
            "--agent",
            "alice",
            "--path",
            path,
            "--ttl",
            &TTL_SECONDS.to_string(),
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run initial claim acquire");
    let acquire_json = assert_success(&acquire, "initial claim acquire");
    let claim_id = acquire_json["data"]["claim_id"]
        .as_str()
        .expect("claim id")
        .to_string();

    let heartbeat = bin()
        .args([
            "claim",
            "heartbeat",
            "--claims-dir",
            &claims_arg,
            "--id",
            &claim_id,
            "--agent",
            "alice",
            "--now-unix",
            &after_expiry.to_string(),
        ])
        .output()
        .expect("run expired claim heartbeat");
    let heartbeat_json = assert_rejected_with_code(
        &heartbeat,
        "expired claim heartbeat",
        "expired_requires_handoff",
    );
    let message = heartbeat_json["error"]["message"]
        .as_str()
        .expect("error message string");
    assert!(
        message.contains("forge-core claim handoff --id"),
        "expired heartbeat should point to official recovery command: {heartbeat_json:#}"
    );
}

#[test]
fn handoff_empty_summary_fails_without_artifact() {
    let parent = fresh_parent("empty-summary");
    let state_root = parent.join(".forge-method");
    let claims_dir = state_root.join("claims-active");
    let claims_arg = claims_dir.display().to_string();
    let evidence = write_evidence(&parent, "empty-summary");
    let evidence_arg = evidence.display().to_string();
    let after_expiry = NOW + TTL_SECONDS + 1;

    let acquire = bin()
        .args([
            "claim",
            "acquire",
            "--claims-dir",
            &claims_arg,
            "--scope",
            "story",
            "--id",
            "HANDOFF-E2E-EMPTY-SUMMARY",
            "--agent",
            "alice",
            "--path",
            "src/empty-summary.rs",
            "--ttl",
            &TTL_SECONDS.to_string(),
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run setup acquire");
    let acquire_json = assert_success(&acquire, "setup acquire");
    let claim_id = acquire_json["data"]["claim_id"]
        .as_str()
        .expect("claim id")
        .to_string();

    let handoffs_dir = state_root.join("handoffs").join("expired-claims");
    let handoff = bin()
        .args([
            "claim",
            "handoff",
            "--claims-dir",
            &claims_arg,
            "--id",
            &claim_id,
            "--agent",
            "alice",
            "--summary",
            "   ",
            "--evidence",
            &evidence_arg,
            "--now-unix",
            &after_expiry.to_string(),
        ])
        .output()
        .expect("run handoff with empty summary");
    assert_rejected_with_code(&handoff, "handoff empty summary", "invalid_request");
    assert_eq!(
        entry_count(&handoffs_dir),
        0,
        "empty summary must not write a handoff artifact"
    );
}

#[test]
fn handoff_live_claim_fails_without_artifact() {
    let parent = fresh_parent("live-claim");
    let state_root = parent.join(".forge-method");
    let claims_dir = state_root.join("claims-active");
    let claims_arg = claims_dir.display().to_string();
    let evidence = write_evidence(&parent, "live");
    let evidence_arg = evidence.display().to_string();

    let acquire = bin()
        .args([
            "claim",
            "acquire",
            "--claims-dir",
            &claims_arg,
            "--scope",
            "story",
            "--id",
            "HANDOFF-E2E-LIVE",
            "--agent",
            "alice",
            "--path",
            "src/live.rs",
            "--ttl",
            "600",
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run setup acquire");
    let acquire_json = assert_success(&acquire, "setup acquire");
    let claim_id = acquire_json["data"]["claim_id"]
        .as_str()
        .expect("claim id")
        .to_string();

    let handoffs_dir = state_root.join("handoffs").join("expired-claims");
    let handoff = bin()
        .args([
            "claim",
            "handoff",
            "--claims-dir",
            &claims_arg,
            "--id",
            &claim_id,
            "--agent",
            "alice",
            "--summary",
            "live claim should not be handoff-recorded",
            "--evidence",
            &evidence_arg,
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run handoff for live claim");
    assert_rejected_with_code(&handoff, "handoff live claim", "illegal_transition");
    assert_eq!(
        entry_count(&handoffs_dir),
        0,
        "live claim handoff must not write a handoff artifact"
    );
}

#[test]
fn handoff_unknown_id_fails_closed_without_artifact() {
    let parent = fresh_parent("unknown-id");
    let state_root = parent.join(".forge-method");
    let claims_dir = state_root.join("claims-active");
    let claims_arg = claims_dir.display().to_string();
    let evidence = write_evidence(&parent, "unknown");
    let evidence_arg = evidence.display().to_string();

    let acquire = bin()
        .args([
            "claim",
            "acquire",
            "--claims-dir",
            &claims_arg,
            "--scope",
            "story",
            "--id",
            "HANDOFF-E2E-KNOWN",
            "--agent",
            "alice",
            "--path",
            "src/known.rs",
            "--ttl",
            &TTL_SECONDS.to_string(),
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run setup acquire");
    assert_success(&acquire, "setup acquire");

    let handoffs_dir = state_root.join("handoffs").join("expired-claims");
    let unknown = bin()
        .args([
            "claim",
            "handoff",
            "--claims-dir",
            &claims_arg,
            "--id",
            "HANDOFF-E2E-MISSING",
            "--agent",
            "alice",
            "--summary",
            "this id should not resolve",
            "--evidence",
            &evidence_arg,
            "--now-unix",
            &(NOW + TTL_SECONDS + 1).to_string(),
        ])
        .output()
        .expect("run handoff for unknown id");
    let unknown_json = assert_failure(&unknown, "handoff unknown id");
    let code = unknown_json["error"]["code"].as_str().unwrap_or_default();
    let message = unknown_json["error"]["message"]
        .as_str()
        .unwrap_or_default();
    assert!(
        code.contains("not_found")
            || code.contains("claim_not_found")
            || message.contains("not found")
            || message.contains("unknown"),
        "unknown id should report a closed not-found style failure: {unknown_json:#}"
    );
    assert_eq!(
        entry_count(&handoffs_dir),
        0,
        "unknown id must not write a handoff artifact"
    );
}
