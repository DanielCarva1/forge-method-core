use assert_cmd::Command;
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

fn fresh_root(label: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let root = repo_root()
        .join("target")
        .join(format!("project-resolve-e2e-{label}-{n}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create fresh root");
    root
}

#[test]
fn project_resolve_finds_sidecar_via_project_link() {
    let parent = fresh_root("sidecar");
    let app = parent.join("darkest-roguelite");
    let state = parent.join("forge-darkest-roguelite").join(".forge-method");
    std::fs::create_dir_all(&app).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        app.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: darkest-roguelite\nsidecar_root: ../forge-darkest-roguelite\nstate_root: ../forge-darkest-roguelite/.forge-method\n",
    )
    .unwrap();

    let output = bin()
        .args(["project", "resolve", "--root", &app.display().to_string()])
        .unwrap();

    assert!(
        output.status.success(),
        "project resolve should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "project.resolve");
    assert_eq!(json["data"]["project_id"], "darkest-roguelite");
    assert_eq!(json["data"]["layout"], "sidecar");
    assert_eq!(json["data"]["state_exists"], true);
    assert!(Path::new(json["data"]["state_root"].as_str().unwrap())
        .ends_with(Path::new("forge-darkest-roguelite").join(".forge-method")));
}

#[test]
fn project_resolve_without_link_fails_closed_for_consumer_repo() {
    let app = fresh_root("missing-link");

    let output = bin()
        .args(["project", "resolve", "--root", &app.display().to_string()])
        .output()
        .expect("run project resolve");

    assert_eq!(output.status.code(), Some(5));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], false);
    assert_eq!(json["exit_reason"], "env_config");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains(".forge-method.yaml"));
}

#[test]
fn project_resolve_allows_core_bootstrap_exception_explicitly() {
    let root = repo_root();

    let output = bin()
        .args([
            "project",
            "resolve",
            "--root",
            &root.display().to_string(),
            "--allow-bootstrap-core",
        ])
        .unwrap();

    assert!(
        output.status.success(),
        "bootstrap resolve should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["project_id"], "forge-method-core");
    assert_eq!(json["data"]["layout"], "bootstrap_core_local");
    assert_eq!(json["data"]["bootstrap_core_exception"], true);
}
