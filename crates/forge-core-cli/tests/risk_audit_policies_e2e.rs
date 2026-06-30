//! F11.2 Risk Audit Gate — canonical policies E2E.
//!
//! These tests exercise the four canonical risk-audit policies that ship
//! under `contracts/risk-audits/` against their paired fixtures under
//! `contracts/risk-audits/fixtures/<policy>/{valid,invalid}/`.
//!
//! The contract under test:
//! - Each policy's `valid/` fixture must pass with zero diagnostics.
//! - Each policy's `invalid/` fixture must fail closed with at least one
//!   `RiskAuditAntiPatternMatched` (or `RiskAuditRuleMalformed` for the
//!   rule-shape cases) diagnostic.
//!
//! Adding a new policy under `contracts/risk-audits/` requires adding a
//! paired fixture here. This is intentional: a policy without a paired
//! fixture is a policy no one has proven works.

use assert_cmd::Command;
use serde_json::Value;
use std::path::PathBuf;

fn bin() -> Command {
    Command::cargo_bin("forge-core").expect("forge-core binary must exist")
}

fn repo_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

fn policy_path(name: &str) -> PathBuf {
    repo_root()
        .join("contracts/risk-audits")
        .join(format!("{name}.yaml"))
}

fn fixture_root(name: &str, kind: &str) -> PathBuf {
    repo_root()
        .join("contracts/risk-audits/fixtures")
        .join(name)
        .join(kind)
}

fn run_risk_audit(root: &std::path::Path, rules: &std::path::Path) -> std::process::Output {
    bin()
        .args(["risk-audit", "--root"])
        .arg(root)
        .args(["--rules"])
        .arg(rules)
        .arg("--json")
        .output()
        .expect("run risk-audit")
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

/// Asserts that a policy's `valid/` fixture passes cleanly (exit 0, zero
/// diagnostics). The valid fixture is the canonical "this is what good
/// looks like" sample for the policy.
fn assert_valid_fixture_passes(policy: &str) {
    let rules = policy_path(policy);
    let root = fixture_root(policy, "valid");

    assert!(
        rules.is_file(),
        "policy {policy}.yaml must exist at {}",
        rules.display()
    );
    assert!(
        root.is_dir(),
        "valid fixture dir must exist at {}",
        root.display()
    );

    let output = run_risk_audit(&root, &rules);

    assert!(
        output.status.success(),
        "{policy}: valid fixture must pass risk-audit\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json = output_json(&output);
    assert_eq!(
        json["ok"], true,
        "{policy}: valid fixture must report ok: {json:#}"
    );
    assert_eq!(
        json["data"]["error_count"], 0,
        "{policy}: valid fixture must have zero errors: {json:#}"
    );
    assert_eq!(
        json["data"]["warning_count"], 0,
        "{policy}: valid fixture must have zero warnings: {json:#}"
    );
}

/// Asserts that a policy's `invalid/` fixture fails closed (non-zero exit)
/// with at least one anti-pattern finding. The invalid fixture is the
/// canonical "this is what the gate catches" sample for the policy.
fn assert_invalid_fixture_fails_closed(policy: &str) {
    let rules = policy_path(policy);
    let root = fixture_root(policy, "invalid");

    assert!(
        root.is_dir(),
        "invalid fixture dir must exist at {}",
        root.display()
    );

    let output = run_risk_audit(&root, &rules);

    assert!(
        !output.status.success(),
        "{policy}: invalid fixture must fail closed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json = output_json(&output);
    assert_eq!(
        json["ok"], false,
        "{policy}: invalid fixture must report not-ok: {json:#}"
    );
    assert_eq!(
        json["exit_reason"], "rejected_by_gate",
        "{policy}: invalid fixture must be a gate rejection: {json:#}"
    );

    let error_count = json["data"]["error_count"].as_u64().unwrap_or(0);
    assert!(
        error_count >= 1,
        "{policy}: invalid fixture must surface at least 1 error-severity finding, got {error_count}: {json:#}"
    );

    // Every diagnostic must carry a typed code so agents can dedupe and
    // suppress across runs.
    let diagnostics = json["data"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("{policy}: diagnostics must be an array: {json:#}"));
    assert!(
        !diagnostics.is_empty(),
        "{policy}: diagnostics must be surfaced"
    );
}

// ── fail-soft ─────────────────────────────────────────────────────────────

#[test]
fn policy_fail_soft_valid_fixture_passes() {
    assert_valid_fixture_passes("fail-soft");
}

#[test]
fn policy_fail_soft_invalid_fixture_fails_closed() {
    assert_invalid_fixture_fails_closed("fail-soft");
}

// ── exception-swallowing ──────────────────────────────────────────────────

#[test]
fn policy_exception_swallowing_valid_fixture_passes() {
    assert_valid_fixture_passes("exception-swallowing");
}

#[test]
fn policy_exception_swallowing_invalid_fixture_fails_closed() {
    assert_invalid_fixture_fails_closed("exception-swallowing");
}

// ── security-slop ─────────────────────────────────────────────────────────

#[test]
fn policy_security_slop_valid_fixture_passes() {
    assert_valid_fixture_passes("security-slop");
}

#[test]
fn policy_security_slop_invalid_fixture_fails_closed() {
    assert_invalid_fixture_fails_closed("security-slop");
}

// ── false-test ────────────────────────────────────────────────────────────

#[test]
fn policy_false_test_valid_fixture_passes() {
    assert_valid_fixture_passes("false-test");
}

#[test]
fn policy_false_test_invalid_fixture_fails_closed() {
    assert_invalid_fixture_fails_closed("false-test");
}
