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
        .join(format!("project-link-hardening-e2e-{label}-{n}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create fresh parent");
    root
}

struct ProjectFixture {
    app: PathBuf,
    sidecar_root: PathBuf,
    state_root: PathBuf,
}

fn sidecar_fixture(label: &str, create_state: bool) -> ProjectFixture {
    let parent = fresh_parent(label);
    let app = parent.join("consumer-app");
    let sidecar_root = parent.join("forge-app");
    let state_root = sidecar_root.join(".forge-method");

    std::fs::create_dir_all(&app).expect("create consumer app root");
    std::fs::create_dir_all(&sidecar_root).expect("create sidecar root");
    if create_state {
        std::fs::create_dir_all(&state_root).expect("create sidecar state root");
    }
    write_project_link(
        &app,
        "consumer-app",
        "../forge-app",
        "../forge-app/.forge-method",
    );

    ProjectFixture {
        app,
        sidecar_root,
        state_root,
    }
}

fn write_project_link(app: &Path, project_id: &str, sidecar_root: &str, state_root: &str) {
    std::fs::write(
        app.join(".forge-method.yaml"),
        format!(
            "schema_version: forge_project_link_v1\nproject_id: {project_id}\nsidecar_root: {sidecar_root}\nstate_root: {state_root}\n",
        ),
    )
    .expect("write project link");
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
    assert_eq!(json["ok"], true, "{label} should report ok: {json:#}");
    json
}

fn assert_failure(output: &std::process::Output, label: &str) -> serde_json::Value {
    assert!(
        !output.status.success(),
        "{label} should fail closed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(output);
    assert_eq!(json["ok"], false, "{label} should report not ok: {json:#}");
    json
}

fn error_message(json: &serde_json::Value) -> String {
    json["error"]["message"]
        .as_str()
        .expect("json error message")
        .to_string()
}

fn assert_message_mentions_all(json: &serde_json::Value, terms: &[&str]) {
    let message = error_message(json);
    let lower = message.to_ascii_lowercase();
    for term in terms {
        assert!(
            lower.contains(&term.to_ascii_lowercase()),
            "error message should mention '{term}': {message}"
        );
    }
}

#[test]
fn resolve_accepts_relative_sidecar_link_when_state_exists() {
    let fixture = sidecar_fixture("relative-state-exists", true);
    let app_arg = fixture.app.display().to_string();

    let output = bin()
        .args(["project", "resolve", "--root", &app_arg, "--json"])
        .output()
        .expect("run project resolve");

    let json = assert_success(&output, "project resolve with relative sidecar link");
    assert_eq!(json["command"], "project.resolve");
    assert_eq!(json["data"]["project_id"], "consumer-app");
    assert_eq!(json["data"]["layout"], "sidecar");
    assert_eq!(json["data"]["state_exists"], true);
    assert!(Path::new(json["data"]["sidecar_root"].as_str().unwrap()).ends_with("forge-app"));
    assert!(Path::new(json["data"]["state_root"].as_str().unwrap())
        .ends_with(Path::new("forge-app").join(".forge-method")));
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "project resolve must not create consumer-local state"
    );
}

#[test]
fn resolve_rejects_state_root_outside_sidecar_root() {
    let parent = fresh_parent("outside-sidecar");
    let app = parent.join("consumer-app");
    let sidecar_root = parent.join("forge-app");
    let other_state_root = parent.join("forge-other").join(".forge-method");

    std::fs::create_dir_all(&app).expect("create consumer app root");
    std::fs::create_dir_all(sidecar_root.join(".forge-method"))
        .expect("create valid-looking sidecar state root");
    std::fs::create_dir_all(&other_state_root).expect("create outside state root");
    write_project_link(
        &app,
        "consumer-app",
        "../forge-app",
        "../forge-other/.forge-method",
    );

    let output = bin()
        .args([
            "project",
            "resolve",
            "--root",
            &app.display().to_string(),
            "--json",
        ])
        .output()
        .expect("run project resolve");

    let json = assert_failure(&output, "project resolve with outside state_root");
    assert_message_mentions_all(&json, &["state_root", "sidecar_root"]);
    assert!(
        !app.join(".forge-method").exists(),
        "failed resolve must not create consumer-local state"
    );
}

#[test]
fn resolve_rejects_consumer_local_state_root() {
    let parent = fresh_parent("consumer-local-state");
    let app = parent.join("consumer-app");
    std::fs::create_dir_all(app.join(".forge-method"))
        .expect("create misconfigured consumer-local state root");
    write_project_link(&app, "consumer-app", ".", ".forge-method");

    let output = bin()
        .args([
            "project",
            "resolve",
            "--root",
            &app.display().to_string(),
            "--json",
        ])
        .output()
        .expect("run project resolve");

    let json = assert_failure(&output, "project resolve with consumer-local state_root");
    assert_message_mentions_all(&json, &["state_root"]);
    let message = error_message(&json).to_ascii_lowercase();
    assert!(
        message.contains("consumer")
            || message.contains("local")
            || message.contains("sidecar_root"),
        "error should explain why consumer-local state is unsafe/actionable: {message}"
    );
}

#[test]
fn claim_status_rejects_missing_resolved_state_root_and_does_not_create_local_state() {
    let fixture = sidecar_fixture("missing-runtime-state", false);
    assert!(
        !fixture.state_root.exists(),
        "fixture should start without runtime state"
    );

    let output = bin()
        .args([
            "claim",
            "status",
            "--root",
            &fixture.app.display().to_string(),
            "--now-unix",
            &NOW.to_string(),
        ])
        .output()
        .expect("run claim status");

    let json = assert_failure(&output, "claim status with missing resolved state_root");
    assert_message_mentions_all(&json, &["state_root"]);
    assert!(
        !fixture.app.join(".forge-method").exists(),
        "failed claim status must not create consumer-local .forge-method"
    );
    assert!(
        !fixture.state_root.exists(),
        "failed claim status must not create missing sidecar runtime state"
    );
    assert!(
        fixture.sidecar_root.exists(),
        "fixture sidecar root should still exist for a precise missing-state assertion"
    );
}
