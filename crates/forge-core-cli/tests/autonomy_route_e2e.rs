use assert_cmd::Command;
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

fn example(name: &str) -> PathBuf {
    repo_root().join("contracts").join("examples").join(name)
}

fn fresh_dir(label: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = repo_root().join("target").join(format!(
        "autonomy-route-e2e-{label}-{}-{n}",
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

#[test]
fn autonomy_route_with_scoped_low_risk_tool_can_use_fast_lane() {
    let output = bin()
        .args(["autonomy", "route", "--policy-file"])
        .arg(example("autonomy-policy.yaml"))
        .arg("--goal-file")
        .arg(example("verification-goal.yaml"))
        .args(["--tool-class", "file_edit"])
        .output()
        .expect("run autonomy route");

    assert!(
        output.status.success(),
        "route should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(&output);
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "autonomy route");
    assert_eq!(json["data"]["lane"], "fast");
    assert_eq!(json["data"]["reason"], "low_risk_verified");
}

#[test]
fn autonomy_route_without_tool_scope_stays_conservative() {
    let output = bin()
        .args(["autonomy", "route", "--policy-file"])
        .arg(example("autonomy-policy.yaml"))
        .arg("--goal-file")
        .arg(example("verification-goal.yaml"))
        .output()
        .expect("run autonomy route");

    assert!(output.status.success(), "route should pass");
    let json = output_json(&output);
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["lane"], "rigorous");
    assert_eq!(json["data"]["reason"], "manual_mode");
}

#[test]
fn autonomy_route_invalid_tool_class_returns_structured_json() {
    let output = bin()
        .args(["autonomy", "route", "--policy-file"])
        .arg(example("autonomy-policy.yaml"))
        .args(["--tool-class", "browser_cookie_dump"])
        .output()
        .expect("run autonomy route");

    assert_eq!(output.status.code(), Some(3));
    let json = output_json(&output);
    assert_eq!(json["ok"], false);
    assert_eq!(json["command"], "autonomy route");
    assert_eq!(json["exit_reason"], "invalid_decision_shape");
    assert!(json["error"]["message"]
        .as_str()
        .expect("error message")
        .contains("browser_cookie_dump"));
}

#[test]
fn autonomy_route_missing_goal_file_returns_structured_json() {
    let missing_goal = fresh_dir("missing-goal").join("missing-goal.yaml");
    let output = bin()
        .args(["autonomy", "route", "--policy-file"])
        .arg(example("autonomy-policy.yaml"))
        .arg("--goal-file")
        .arg(&missing_goal)
        .output()
        .expect("run autonomy route");

    assert_eq!(output.status.code(), Some(3));
    let json = output_json(&output);
    assert_eq!(json["ok"], false);
    assert_eq!(json["command"], "autonomy route");
    assert!(json["error"]["message"]
        .as_str()
        .expect("error message")
        .contains("cannot read goal file"));
}

#[test]
fn autonomy_route_invalid_goal_yaml_returns_structured_json() {
    let dir = fresh_dir("invalid-goal-yaml");
    let goal = dir.join("invalid-goal.yaml");
    std::fs::write(&goal, "schema_version: [").expect("write invalid goal yaml");

    let output = bin()
        .args(["autonomy", "route", "--policy-file"])
        .arg(example("autonomy-policy.yaml"))
        .arg("--goal-file")
        .arg(&goal)
        .output()
        .expect("run autonomy route");

    assert_eq!(output.status.code(), Some(3));
    let json = output_json(&output);
    assert_eq!(json["ok"], false);
    assert_eq!(json["command"], "autonomy route");
    assert!(json["error"]["message"]
        .as_str()
        .expect("error message")
        .contains("goal file is not a valid verification_goal contract"));
}

#[test]
fn autonomy_route_text_success_reports_lane() {
    let output = bin()
        .args(["autonomy", "route", "--policy-file"])
        .arg(example("autonomy-policy.yaml"))
        .arg("--goal-file")
        .arg(example("verification-goal.yaml"))
        .args(["--tool-class", "file_edit", "--no-json"])
        .output()
        .expect("run autonomy route");

    assert!(output.status.success(), "text route should pass");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "lane: fast");
    assert!(output.stderr.is_empty());
}
