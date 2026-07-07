//! F11 Risk Audit Gate — CLI integration tests (`forge-core risk-audit`).
//!
//! These tests exercise the standalone CLI surface end-to-end: argv parsing,
//! rule-set loading, target walking, and the fail-closed envelope contract.
//! They use the canonical `valid-rust-antipatterns.yaml` fixture so the same
//! rule set that ships as a reference contract also serves as the regression
//! baseline.
//!
//! See `crates/forge-core-validate/src/risk_audit.rs` for the rule engine
//! unit tests.

use assert_cmd::Command;
use serde_json::Value;
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

fn fixture(name: &str) -> PathBuf {
    repo_root()
        .join("crates/forge-core-cli/tests/fixtures/risk-audit")
        .join(name)
}

/// Fresh, isolated parent dir under `target/` so parallel test runs do not
/// stomp on each other. Cleaned up on drop.
struct FreshParent {
    path: PathBuf,
}

impl FreshParent {
    fn new(label: &str) -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let path = repo_root()
            .join("target")
            .join(format!("risk-audit-e2e-{label}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create fresh parent");
        Self { path }
    }
}

impl Drop for FreshParent {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn output_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout should be json: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn run_risk_audit(root: &Path, rules: Option<&Path>) -> std::process::Output {
    let mut cmd = bin();
    cmd.args(["risk-audit", "--root"]).arg(root);
    if let Some(rules) = rules {
        cmd.args(["--rules"]).arg(rules);
    }
    cmd.arg("--json").output().expect("run risk-audit")
}

/// Writes a tiny clean Rust project (no anti-patterns) under `parent/<name>`.
fn write_clean_rust_project(parent: &Path, name: &str) -> PathBuf {
    let app = parent.join(name);
    fs::create_dir_all(app.join("src")).expect("create src dir");
    fs::write(app.join("README.md"), "# clean-app\n").expect("write README");
    fs::write(
        app.join("src/main.rs"),
        "fn main() {\n    println!(\"hello\");\n}\n",
    )
    .expect("write clean main.rs");
    app
}

/// Writes a Rust project that contains anti-patterns the reference rule set
/// flags: a `.unwrap()`, a `let _ =` swallowing a call, and a `.expect()`.
fn write_dirty_rust_project(parent: &Path, name: &str) -> PathBuf {
    let app = parent.join(name);
    fs::create_dir_all(app.join("src")).expect("create src dir");
    fs::write(app.join("README.md"), "# dirty-app\n").expect("write README");
    fs::write(
        app.join("src/main.rs"),
        // Contains: `.unwrap()` (error), `let _ = call()` (error), `.expect()` (warning).
        "fn main() {\n    let x = std::fs::read_to_string(\"x\").unwrap();\n    let _ = std::fs::read_to_string(\"y\");\n    let y = x.expect(\"loaded\");\n    println!(\"{y}\");\n}\n",
    )
    .expect("write dirty main.rs");
    app
}

#[test]
fn risk_audit_missing_rules_flag_fails_clearly() {
    // Without `--rules`, the command must fail closed with an `env-config`
    // exit reason: agents need an explicit rule set, never a silent default.
    let parent = FreshParent::new("missing-rules");
    let app = write_clean_rust_project(&parent.path, "clean-app");

    let output = run_risk_audit(&app, None);

    assert!(
        !output.status.success(),
        "risk-audit without --rules must fail closed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(&output);
    assert_eq!(json["ok"], false, "envelope ok must be false: {json:#}");
    assert_eq!(
        json["exit_reason"], "env_config",
        "missing rules is an env-config failure: {json:#}"
    );
    let message = json["error"]["message"]
        .as_str()
        .expect("error message present");
    assert!(
        message.contains("--rules"),
        "error must mention the missing --rules flag: {message}"
    );
}

#[test]
fn risk_audit_invalid_rules_yaml_fails_clearly() {
    // Malformed YAML must surface as `invalid-decision-shape`: the rule set
    // could not be parsed into a `RiskAuditRuleSet`, so the input shape is
    // wrong rather than the environment.
    let parent = FreshParent::new("invalid-yaml");
    let app = write_clean_rust_project(&parent.path, "clean-app");

    let rules_path = parent.path.join("bad.yaml");
    fs::write(
        &rules_path,
        "schema_version: risk-audit-v0\nrules: [this: is: not: valid\n",
    )
    .expect("write malformed yaml");

    let output = run_risk_audit(&app, Some(&rules_path));

    assert!(
        !output.status.success(),
        "risk-audit with malformed YAML must fail closed"
    );
    let json = output_json(&output);
    assert_eq!(json["ok"], false);
    assert_eq!(
        json["exit_reason"], "invalid_decision_shape",
        "malformed YAML is an invalid_decision_shape failure: {json:#}"
    );
}

#[test]
fn risk_audit_empty_ruleset_fails_closed() {
    // A structurally valid but empty rule set must fail closed with
    // `RiskAuditRuleSetEmpty`. An empty rule set is not a free pass: it means
    // the consumer repo was never actually audited.
    let parent = FreshParent::new("empty-ruleset");
    let app = write_clean_rust_project(&parent.path, "clean-app");

    let rules_path = parent.path.join("empty.yaml");
    fs::write(&rules_path, "schema_version: risk-audit-v0\nrules: []\n")
        .expect("write empty ruleset yaml");

    let output = run_risk_audit(&app, Some(&rules_path));

    assert!(
        !output.status.success(),
        "empty rule set must fail closed (fail-closed by design)"
    );
    let json = output_json(&output);
    assert_eq!(json["ok"], false);
    assert_eq!(
        json["exit_reason"], "invalid_decision_shape",
        "structurally invalid rule set is invalid_decision_shape: {json:#}"
    );
    let message = json["error"]["message"].as_str().expect("error message");
    assert!(
        message.to_lowercase().contains("at least one rule"),
        "error must explain the rule set requires at least one rule: {message}"
    );
}

#[test]
fn risk_audit_passes_when_no_anti_pattern_matches() {
    // Happy path: a clean Rust project with a README passes the canonical
    // anti-pattern rule set and exits 0 with an `ok` envelope.
    let parent = FreshParent::new("clean-pass");
    let app = write_clean_rust_project(&parent.path, "clean-app");

    let output = run_risk_audit(&app, Some(&fixture("valid-rust-antipatterns.yaml")));

    assert!(
        output.status.success(),
        "clean project should pass risk-audit\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(&output);
    assert_eq!(json["ok"], true, "envelope ok must be true: {json:#}");
    assert_eq!(json["command"], "risk-audit.run");
    assert_eq!(
        json["data"]["error_count"], 0,
        "clean project must have zero errors: {json:#}"
    );
    assert_eq!(
        json["data"]["warning_count"], 0,
        "clean project must have zero warnings: {json:#}"
    );
    assert!(
        json["data"]["rule_count"].as_u64().unwrap_or(0) >= 1,
        "rule_count should reflect the fixture: {json:#}"
    );
}

#[test]
fn risk_audit_fails_closed_when_anti_pattern_matched() {
    // Fail-closed path: a dirty Rust project triggers `no-unwrap` and
    // `no-empty-catch` (both errors) plus `no-expect` (warning). The command
    // must exit non-zero with exit_reason `rejected-by-gate` and still surface
    // the full summary so agents can act on every finding without re-running.
    let parent = FreshParent::new("dirty-fail");
    let app = write_dirty_rust_project(&parent.path, "dirty-app");

    let output = run_risk_audit(&app, Some(&fixture("valid-rust-antipatterns.yaml")));

    assert!(
        !output.status.success(),
        "dirty project must fail closed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = output_json(&output);
    assert_eq!(json["ok"], false, "envelope ok must be false: {json:#}");
    assert_eq!(
        json["exit_reason"], "rejected_by_gate",
        "anti-pattern match is a gate rejection: {json:#}"
    );

    let error_count = json["data"]["error_count"]
        .as_u64()
        .expect("error_count is a number");
    assert!(
        error_count >= 2,
        "dirty fixture triggers at least 2 error-severity findings (unwrap + let _ =), got {error_count}: {json:#}"
    );

    // Every diagnostic must carry its rule-derived code so agents can dedupe.
    let diagnostics = json["data"]["diagnostics"]
        .as_array()
        .expect("diagnostics is an array");
    assert!(
        !diagnostics.is_empty(),
        "diagnostics must be surfaced to the agent"
    );
    let codes: Vec<&str> = diagnostics
        .iter()
        .map(|d| d["code"].as_str().unwrap_or(""))
        .collect();
    assert!(
        codes.iter().any(|c| c.contains("AntiPatternMatched")),
        "at least one diagnostic must be an AntiPatternMatched finding: {codes:?}"
    );
}
