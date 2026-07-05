use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
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

fn fresh_project(label: &str) -> (PathBuf, PathBuf) {
    let (app, _sidecar_root, state_root) = fresh_project_layout(label, true);
    (app, state_root)
}

fn fresh_project_missing_sidecar(label: &str) -> (PathBuf, PathBuf, PathBuf) {
    fresh_project_layout(label, false)
}

fn fresh_project_layout(label: &str, create_sidecar: bool) -> (PathBuf, PathBuf, PathBuf) {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let parent = repo_root()
        .join("target")
        .join(format!("m1-cli-e2e-{label}-{n}"));
    let app = parent.join("app");
    let sidecar_root = parent.join("forge-app");
    let state_root = sidecar_root.join(".forge-method");
    let _ = fs::remove_dir_all(&parent);
    fs::create_dir_all(&app).expect("create app root");
    if create_sidecar {
        fs::create_dir_all(&state_root).expect("create sidecar state root");
    }
    copy_dir(&repo_root().join("contracts"), &app.join("contracts"));
    copy_dir(
        &repo_root().join("docs").join("fixtures"),
        &app.join("docs").join("fixtures"),
    );
    fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../forge-app\nstate_root: ../forge-app/.forge-method\n",
    )
    .expect("write project link");
    (app, sidecar_root, state_root)
}

fn copy_dir(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("create copied directory");
    for entry in fs::read_dir(source).expect("read source directory") {
        let entry = entry.expect("read directory entry");
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(&source_path, &target_path);
        } else if source_path.is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).expect("create copied file parent");
            }
            fs::copy(&source_path, &target_path).unwrap_or_else(|error| {
                panic!(
                    "copy fixture file {} -> {} failed: {error}",
                    source_path.display(),
                    target_path.display()
                )
            });
        }
    }
}

#[test]
fn m1_help_paths_project_command_surface_usage() {
    for (command, expected_usage, sibling_usage) in [
        (
            "preview",
            "forge-core preview [--root <path>] --operation <path>",
            "forge-core ready [--root <path>] --operation <path>",
        ),
        (
            "ready",
            "forge-core ready [--root <path>] --operation <path>",
            "forge-core preview [--root <path>] --operation <path>",
        ),
        (
            "explain",
            "forge-core explain [--root <path>] (--last-run | --run-id <id>)",
            "forge-core preview [--root <path>] --operation <path>",
        ),
    ] {
        let output = bin()
            .args([command, "--help"])
            .output()
            .expect("run M1 help");
        assert!(output.status.success(), "{command} --help should pass");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(expected_usage),
            "{command} help should include its Command Surface usage: {stdout}"
        );
        assert!(
            stdout.contains("[--json|--no-json]"),
            "{command} help should expose the shared JSON/text contract: {stdout}"
        );
        assert!(
            !stdout.contains(sibling_usage),
            "{command} help must not fall back to the global usage table: {stdout}"
        );
        assert!(
            !stdout.contains("forge-core execute-operation"),
            "{command} help must not include unrelated global commands: {stdout}"
        );
    }
}

#[test]
fn m1_preview_missing_sidecar_state_fails_without_creating_state() {
    let (app, sidecar_root, state_root) = fresh_project_missing_sidecar("preview-missing-sidecar");

    let output = bin()
        .args([
            "preview",
            "--root",
            &app.display().to_string(),
            "--operation",
            "docs/fixtures/operation-contract-v0/mechanical-story-execute.yaml",
            "--recorded-at",
            "2026-06-29T01:04:00Z",
            "--agent-id",
            "codex-test",
            "--principal-id",
            "principal.test",
            "--json",
        ])
        .output()
        .expect("run preview");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("env/config failure"), "{stderr}");
    assert!(stderr.contains("missing sidecar state_root"), "{stderr}");
    assert!(!sidecar_root.exists());
    assert!(!state_root.exists());
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn m1_ready_missing_sidecar_state_fails_without_creating_state() {
    let (app, sidecar_root, state_root) = fresh_project_missing_sidecar("ready-missing-sidecar");

    let output = bin()
        .args([
            "ready",
            "--root",
            &app.display().to_string(),
            "--operation",
            "docs/fixtures/operation-contract-v0/mechanical-story-execute.yaml",
            "--recorded-at",
            "2026-06-29T01:05:00Z",
            "--agent-id",
            "codex-test",
            "--principal-id",
            "principal.test",
            "--json",
        ])
        .output()
        .expect("run ready");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("env/config failure"), "{stderr}");
    assert!(stderr.contains("missing sidecar state_root"), "{stderr}");
    assert!(!sidecar_root.exists());
    assert!(!state_root.exists());
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn m1_explain_missing_sidecar_state_fails_without_creating_state() {
    let (app, sidecar_root, state_root) = fresh_project_missing_sidecar("explain-missing-sidecar");

    let output = bin()
        .args([
            "explain",
            "--root",
            &app.display().to_string(),
            "--last-run",
            "--json",
        ])
        .output()
        .expect("run explain");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("env/config failure"), "{stderr}");
    assert!(stderr.contains("missing sidecar state_root"), "{stderr}");
    assert!(!sidecar_root.exists());
    assert!(!state_root.exists());
    assert!(!app.join(".forge-method").exists());
}

#[test]
fn m1_preview_writes_trace_to_sidecar_not_consumer_repo() {
    let (app, sidecar) = fresh_project("preview");

    let output = bin()
        .args([
            "preview",
            "--root",
            &app.display().to_string(),
            "--operation",
            "docs/fixtures/operation-contract-v0/mechanical-story-execute.yaml",
            "--recorded-at",
            "2026-06-29T01:00:00Z",
            "--agent-id",
            "codex-test",
            "--principal-id",
            "principal.test",
            "--json",
        ])
        .unwrap();

    assert!(
        output.status.success(),
        "preview should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["project_id"], "app");
    assert_eq!(json["trace_appended"], true);
    assert_eq!(json["report"]["preview_mutates_state"], false);
    assert_eq!(json["report"]["status"], "ready");
    assert!(sidecar.join("traces").join("events.ndjson").exists());
    assert!(!app.join(".forge-method").exists());
    let trace =
        fs::read_to_string(sidecar.join("traces").join("events.ndjson")).expect("read trace log");
    assert_eq!(trace.lines().count(), 4);
    assert!(trace.contains("\"event_kind\":\"preview_completed\""));
    assert!(trace.contains("\"event_kind\":\"run_completed\""));
}

#[test]
fn m1_ready_fails_closed_and_explain_reads_last_run_from_sidecar() {
    let (app, sidecar) = fresh_project("ready-explain");

    let ready = bin()
        .args([
            "ready",
            "--root",
            &app.display().to_string(),
            "--operation",
            "docs/fixtures/operation-contract-v0/release-gate-required.yaml",
            "--recorded-at",
            "2026-06-29T01:02:00Z",
            "--agent-id",
            "codex-test",
            "--principal-id",
            "principal.test",
            "--json",
        ])
        .output()
        .expect("run ready");

    assert_eq!(ready.status.code(), Some(1));
    let ready_json: serde_json::Value = serde_json::from_slice(&ready.stdout).unwrap();
    assert_eq!(ready_json["report"]["ready"], false);
    assert_eq!(ready_json["report"]["status"], "not_ready");
    assert!(ready_json["report"]["blocking_reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason == "gate_pending"));
    assert!(sidecar.join("traces").join("events.ndjson").exists());

    let explain = bin()
        .args([
            "explain",
            "--root",
            &app.display().to_string(),
            "--last-run",
            "--json",
        ])
        .unwrap();

    assert!(
        explain.status.success(),
        "explain should pass: {}",
        String::from_utf8_lossy(&explain.stderr)
    );
    let explain_json: serde_json::Value = serde_json::from_slice(&explain.stdout).unwrap();
    assert_eq!(explain_json["query"]["returned_events"], 4);
    let explanation = explain_json["explanation"].as_str().unwrap();
    assert!(explanation.contains("op_fixture_release_gate_required"));
    assert!(explanation.contains("contracts/gates/release-missing-gate.yaml"));
}

#[test]
fn m1_default_run_ids_do_not_merge_repeated_operation_runs() {
    let (app, sidecar) = fresh_project("default-run-id");

    let first = bin()
        .args([
            "preview",
            "--root",
            &app.display().to_string(),
            "--operation",
            "docs/fixtures/operation-contract-v0/mechanical-story-execute.yaml",
            "--agent-id",
            "codex-test",
            "--principal-id",
            "principal.test",
            "--json",
        ])
        .unwrap();
    assert!(
        first.status.success(),
        "first preview should pass: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_json: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();

    let second = bin()
        .args([
            "preview",
            "--root",
            &app.display().to_string(),
            "--operation",
            "docs/fixtures/operation-contract-v0/mechanical-story-execute.yaml",
            "--agent-id",
            "codex-test",
            "--principal-id",
            "principal.test",
            "--json",
        ])
        .unwrap();
    assert!(
        second.status.success(),
        "second preview should pass: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_json: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();

    assert_ne!(first_json["run_id"], second_json["run_id"]);
    let trace =
        fs::read_to_string(sidecar.join("traces").join("events.ndjson")).expect("read trace log");
    assert_eq!(trace.lines().count(), 8);

    let explain = bin()
        .args([
            "explain",
            "--root",
            &app.display().to_string(),
            "--last-run",
            "--json",
        ])
        .unwrap();
    assert!(
        explain.status.success(),
        "explain should pass: {}",
        String::from_utf8_lossy(&explain.stderr)
    );
    let explain_json: serde_json::Value = serde_json::from_slice(&explain.stdout).unwrap();
    assert_eq!(explain_json["query"]["returned_events"], 4);
    assert!(explain_json["explanation"]
        .as_str()
        .unwrap()
        .contains(second_json["run_id"].as_str().unwrap()));
}
