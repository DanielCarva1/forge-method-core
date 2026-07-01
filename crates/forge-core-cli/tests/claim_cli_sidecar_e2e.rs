#![allow(clippy::too_many_lines)]
#![allow(clippy::doc_markdown)]
// End-to-end test that drives the full sidecar lifecycle (claim → heartbeat
// → release) in one function; splitting it would obscure the sequence.

use assert_cmd::Command;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const NOW: i64 = 1_800_000_000;

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
    let root = repo_root()
        .join("target")
        .join(format!("claim-cli-sidecar-e2e-{label}-{n}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create fresh parent");
    root
}

struct ConsumerApp {
    app: PathBuf,
    state_root: PathBuf,
}

fn consumer_app(label: &str) -> ConsumerApp {
    let parent = fresh_parent(label);
    let app = parent.join("app");
    let sidecar = parent.join("forge-app");
    let state_root = sidecar.join(".forge-method");

    std::fs::create_dir_all(&app).expect("create app root");
    std::fs::create_dir_all(&state_root).expect("create sidecar state root");
    std::fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");

    ConsumerApp { app, state_root }
}

fn output_json(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout should be json: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(output: &std::process::Output, label: &str) -> serde_json::Value {
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

fn yaml_file_count(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .expect("read claims dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "yaml"))
        .count()
}

#[test]
fn raw_claim_commands_default_to_sidecar_claim_bus() {
    let fixture = consumer_app("default-sidecar");
    let app = fixture.app.display().to_string();
    let claims_dir = fixture.state_root.join("claims-active");

    let acquire = bin()
        .args([
            "claim",
            "acquire",
            "--root",
            &app,
            "--scope",
            "story",
            "--id",
            "CB-S2-sidecar",
            "--agent",
            "sidecar-agent",
            "--path",
            "src/main.rs",
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run claim acquire");

    let acquire_json = assert_success(&acquire, "claim acquire");
    assert_eq!(acquire_json["command"], "claim.acquire");
    assert_eq!(
        acquire_json["data"]["claim_id"],
        "claim.story.CB-S2-sidecar.CB-S2-sidecar"
    );
    assert!(
        claims_dir.exists(),
        "claim acquire should create sidecar claims-active"
    );
    assert_eq!(yaml_file_count(&claims_dir), 1);
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "raw claim acquire must not create consumer-local .forge-method"
    );

    let status = bin()
        .args([
            "claim",
            "status",
            "--root",
            &app,
            "--now-unix",
            &(NOW + 1).to_string(),
        ])
        .output()
        .expect("run claim status");
    let status_json = assert_success(&status, "claim status");
    assert_eq!(status_json["command"], "claim.status");
    let active = status_json["data"]["active"]
        .as_array()
        .expect("active claims array");
    assert!(
        active.iter().any(|claim| {
            claim["agent_id"] == "sidecar-agent"
                && claim["paths"]
                    .as_array()
                    .is_some_and(|paths| paths.iter().any(|path| path == "src/main.rs"))
        }),
        "status should read the sidecar claim bus: {status_json:#}"
    );
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "raw claim status must not create consumer-local .forge-method"
    );

    let check_write = bin()
        .args([
            "claim",
            "check-write",
            "--root",
            &app,
            "--agent",
            "sidecar-agent",
            "--target",
            "src/main.rs",
            "--now-unix",
            &(NOW + 2).to_string(),
        ])
        .output()
        .expect("run claim check-write");
    let check_json = assert_success(&check_write, "claim check-write");
    assert_eq!(check_json["command"], "check-write");
    assert_eq!(check_json["data"]["allowed"], true);
    assert_eq!(
        check_json["data"]["ungoverned"].as_array().unwrap().len(),
        0
    );
    assert!(
        check_json["data"]["governed_by_self"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == "src/main.rs"),
        "check-write should authorize via the sidecar claim bus: {check_json:#}"
    );

    let heartbeat = bin()
        .args([
            "claim",
            "heartbeat",
            "--root",
            &app,
            "--id",
            "CB-S2-sidecar",
            "--agent",
            "sidecar-agent",
            "--now-unix",
            &(NOW + 3).to_string(),
        ])
        .output()
        .expect("run claim heartbeat");
    let heartbeat_json = assert_success(&heartbeat, "claim heartbeat");
    assert_eq!(heartbeat_json["command"], "claim.heartbeat");
    assert_eq!(heartbeat_json["data"]["status"], "active");
    assert_eq!(yaml_file_count(&claims_dir), 1);
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "raw claim heartbeat must not create consumer-local .forge-method"
    );

    let release = bin()
        .args([
            "claim",
            "release",
            "--root",
            &app,
            "--id",
            "CB-S2-sidecar",
            "--agent",
            "sidecar-agent",
            "--now-unix",
            &(NOW + 4).to_string(),
        ])
        .output()
        .expect("run claim release");
    let release_json = assert_success(&release, "claim release");
    assert_eq!(release_json["command"], "claim.release");
    assert_eq!(release_json["data"]["status"], "released");

    let status_after_release = bin()
        .args([
            "claim",
            "status",
            "--root",
            &app,
            "--now-unix",
            &(NOW + 5).to_string(),
        ])
        .output()
        .expect("run claim status after release");
    let status_after_release_json =
        assert_success(&status_after_release, "claim status after release");
    assert!(
        status_after_release_json["data"]["active"]
            .as_array()
            .expect("active claims array")
            .is_empty(),
        "released sidecar claim must not remain active: {status_after_release_json:#}"
    );
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "raw claim release/status must not create consumer-local .forge-method"
    );
}

#[test]
fn claim_reconcile_defaults_to_sidecar_claim_bus() {
    let fixture = consumer_app("reconcile-sidecar");
    let app = fixture.app.display().to_string();
    let claims_dir = fixture.state_root.join("claims-active");

    let acquire = bin()
        .args([
            "claim",
            "acquire",
            "--root",
            &app,
            "--scope",
            "story",
            "--id",
            "P23-sidecar",
            "--agent",
            "sidecar-agent",
            "--path",
            "src/reconcile.rs",
            "--ttl",
            "10",
            "--heartbeat-interval",
            "5",
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run claim acquire");
    assert_success(&acquire, "claim acquire");

    let reconcile = bin()
        .args([
            "claim",
            "reconcile",
            "--root",
            &app,
            "--now-unix",
            &(NOW + 5).to_string(),
        ])
        .output()
        .expect("run claim reconcile");
    let reconcile_json = assert_success(&reconcile, "claim reconcile");
    assert_eq!(reconcile_json["command"], "claim.reconcile");
    assert_eq!(reconcile_json["data"]["changed"], 1);
    assert_eq!(reconcile_json["data"]["transitions"][0]["to"], "stale");
    assert!(
        fixture.state_root.join("wal").join("claims.fmw1").exists(),
        "reconcile should append sidecar WAL"
    );
    assert_eq!(yaml_file_count(&claims_dir), 1);
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "claim reconcile must not create consumer-local .forge-method"
    );

    let status = bin()
        .args([
            "claim",
            "status",
            "--root",
            &app,
            "--now-unix",
            &(NOW + 6).to_string(),
        ])
        .output()
        .expect("run claim status");
    let status_json = assert_success(&status, "claim status after reconcile");
    assert_eq!(status_json["data"]["active"][0]["status"], "stale");
}

#[test]
fn explicit_claims_dir_override_preserves_existing_behavior() {
    let fixture = consumer_app("override");
    let app = fixture.app.display().to_string();
    let override_claims = fixture
        .app
        .parent()
        .expect("fixture parent")
        .join("override-claims");
    let override_claims_arg = override_claims.display().to_string();

    let acquire = bin()
        .args([
            "claim",
            "acquire",
            "--root",
            &app,
            "--claims-dir",
            &override_claims_arg,
            "--scope",
            "story",
            "--id",
            "CB-S2-override",
            "--agent",
            "override-agent",
            "--path",
            "README.md",
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run explicit override acquire");

    let acquire_json = assert_success(&acquire, "claim acquire with --claims-dir");
    assert_eq!(
        acquire_json["data"]["claim_id"],
        "claim.story.CB-S2-override.CB-S2-override"
    );
    assert_eq!(yaml_file_count(&override_claims), 1);
    assert!(
        !fixture.state_root.join("claims-active").exists(),
        "explicit --claims-dir must not write the resolved sidecar bus"
    );
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "explicit --claims-dir must not create consumer-local .forge-method"
    );
}

#[test]
fn missing_project_link_without_claims_dir_fails_closed() {
    let app = fresh_parent("missing-link");
    let app_arg = app.display().to_string();

    let output = bin()
        .args([
            "claim",
            "status",
            "--root",
            &app_arg,
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run claim status without project link");

    assert!(
        !output.status.success(),
        "missing project link should fail without --claims-dir\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(&output);
    assert_eq!(json["ok"], false);
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains(".forge-method.yaml"),
        "error should explain the missing project link: {json:#}"
    );
    assert!(
        !app.join(".forge-method").exists(),
        "failed claim status must not create consumer-local .forge-method"
    );
}
