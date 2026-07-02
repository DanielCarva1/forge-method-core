//! F08.7 — end-to-end CLI test for `forge-core mcp`.
//!
//! Drives the REAL `forge-core` binary through the `mcp` command surface
//! (ADR-0006):
//! - `mcp --help` prints usage.
//! - `mcp serve --allowlist <valid>` loads the canonical fixture Allowlist
//!   and would serve (we validate the parse path, not the long-running loop).
//! - `mcp serve --allowlist <invalid>` rejects with a `CliEnvelope` error
//!   before the protocol loop starts (fail-closed on bad config).
//! - `mcp serve` with no `--allowlist` defaults to read-only (safe surface).
//!
//! The fixture is `contracts/examples/mcp-allowlist.yaml` (canonical example).
//! No mocks: the binary parses the real YAML and validates it against the real
//! `command_registry::COMMANDS`. Mirrors the `memory_cli_e2e` structure.

use assert_cmd::Command;
use serde_json::Value;
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

fn example(name: &str) -> PathBuf {
    repo_root().join("contracts").join("examples").join(name)
}

fn fresh_tmp(label: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = repo_root()
        .join("target")
        .join(format!("mcp-cli-e2e-{label}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create fresh tmp dir");
    dir
}

fn output_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout should be JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// `mcp --help` prints usage listing the `serve` subcommand.
#[test]
fn mcp_help_lists_serve_subcommand() {
    let output = bin()
        .args(["mcp", "--help"])
        .output()
        .expect("run forge-core mcp --help");
    assert!(output.status.success(), "mcp --help should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("serve"),
        "help must mention serve: {stdout}"
    );
    assert!(
        stdout.contains("--allowlist"),
        "help must mention --allowlist: {stdout}"
    );
}

/// `mcp bogus` (unknown subcommand) exits non-zero with an envelope.
#[test]
fn mcp_unknown_subcommand_exits_nonzero() {
    let output = bin()
        .args(["mcp", "definitely-not-a-subcommand", "--json"])
        .output()
        .expect("run forge-core mcp bogus");
    assert!(
        !output.status.success(),
        "unknown subcommand must exit non-zero"
    );
    let env = output_json(&output);
    assert_eq!(env["ok"], false);
    assert!(
        env["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("unknown subcommand")),
        "envelope must report unknown subcommand: {env}"
    );
}

/// `mcp serve --allowlist <missing-file>` fails closed with an env-config
/// envelope (the file does not exist).
#[test]
fn mcp_serve_missing_allowlist_fails_closed() {
    let tmp = fresh_tmp("missing-allowlist");
    let bogus = tmp.join("does-not-exist.yaml");
    let output = bin()
        .args([
            "mcp",
            "serve",
            "--allowlist",
            bogus.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run forge-core mcp serve with missing allowlist");
    assert!(
        !output.status.success(),
        "missing allowlist must exit non-zero"
    );
    let env = output_json(&output);
    assert_eq!(env["ok"], false);
    let msg = env["error"]["message"].as_str().unwrap_or("no message");
    assert!(
        msg.contains("failed to read allowlist"),
        "envelope must report read failure: {msg}"
    );
}

/// `mcp serve --allowlist <invalid-yaml>` fails closed with an
/// `InvalidDecisionShape` envelope listing the parse diagnostic.
#[test]
fn mcp_serve_malformed_allowlist_fails_closed() {
    let tmp = fresh_tmp("malformed-allowlist");
    let bad = tmp.join("bad.yaml");
    std::fs::write(&bad, "tools: [this is not: valid: yaml\n").unwrap();
    let output = bin()
        .args([
            "mcp",
            "serve",
            "--allowlist",
            bad.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run forge-core mcp serve with malformed allowlist");
    assert!(
        !output.status.success(),
        "malformed allowlist must exit non-zero"
    );
    let env = output_json(&output);
    assert_eq!(env["ok"], false);
    let msg = env["error"]["message"].as_str().unwrap_or("no message");
    assert!(
        msg.contains("allowlist validation failed"),
        "envelope must report validation failure: {msg}"
    );
}

/// `mcp serve --allowlist <unknown-tool>` fails closed with a validation
/// error naming the unknown tool (a typo is caught at load, not at call time).
#[test]
fn mcp_serve_allowlist_with_unknown_tool_fails_closed() {
    let tmp = fresh_tmp("unknown-tool");
    let bad = tmp.join("unknown.yaml");
    std::fs::write(
        &bad,
        "tools:\n  - name: preview\n  - name: not-a-real-command\n",
    )
    .unwrap();
    let output = bin()
        .args([
            "mcp",
            "serve",
            "--allowlist",
            bad.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run forge-core mcp serve with unknown tool");
    assert!(!output.status.success());
    let env = output_json(&output);
    assert_eq!(env["ok"], false);
    let msg = env["error"]["message"].as_str().unwrap_or("no message");
    assert!(
        msg.contains("not-a-real-command"),
        "envelope must name the unknown tool: {msg}"
    );
}

/// The canonical example fixture `contracts/examples/mcp-allowlist.yaml` is
/// valid: it loads and passes validation. This guards the fixture against
/// drift (a renamed command would break it).
#[test]
fn canonical_mcp_allowlist_fixture_is_valid() {
    let fixture = example("mcp-allowlist.yaml");
    assert!(
        fixture.exists(),
        "fixture must exist: {}",
        fixture.display()
    );
    let yaml = std::fs::read_to_string(&fixture).unwrap();
    let known: Vec<&str> = forge_core_cli::command_registry::COMMANDS
        .iter()
        .map(|c| c.name)
        .collect();
    let (allowlist, report) = forge_core_protocol_mcp::Allowlist::from_yaml_str(&yaml, &known);
    assert!(
        !report.has_errors(),
        "canonical fixture must validate: {:?}",
        report.diagnostics()
    );
    // The canonical fixture includes both read-only and mutate tools.
    assert!(allowlist.get("preview").is_some());
    assert!(allowlist.get("execute-operation").is_some());
}

/// Read-only default Allowlist has no mutate tools (the safe surface).
/// This pins ADR-0006 Decision 3: declaring an empty/restricted Allowlist is
/// the safe default, not full exposure.
#[test]
fn default_allowlist_is_read_only() {
    let al = forge_core_protocol_mcp::Allowlist::default_read_only();
    assert!(al.iter().all(|t| !t.policy.is_mutate()));
    assert!(al.get("execute-operation").is_none());
}

/// The global usage text mentions the `mcp` command (the registry test
/// already checks this, but confirm end-to-end via the binary).
#[test]
fn mcp_appears_in_global_help() {
    let output = bin().arg("--help").output().expect("run forge-core --help");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mcp serve"),
        "global help must list mcp serve"
    );
}
