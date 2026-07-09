//! F14.7 — end-to-end CLI test for `forge-core research`.
//!
//! Drives the REAL `forge-core` binary through the research source lifecycle
//! (source add → source list → cite resolved → cite unresolved → check → graph)
//! using the permanent fixtures in `contracts/examples/` and
//! `docs/fixtures/research-v0/`. No mocks: the binary writes to a fresh
//! `.forge-method` sidecar state root, the PEP appends to a real JSONL log, the
//! projection replays it, and the citation check resolves against the union of
//! the curated registry and the runtime ledger.
//!
//! Mirrors `memory_cli_e2e.rs` structure.

// E2E test files run the real binary through long lifecycle scenarios; the
// doc-comment / line-count pedantic lints are noise here, not signal. Matches
// the memory_cli_e2e.rs / governance_cli_e2e.rs convention.
#![allow(clippy::doc_markdown)]
#![allow(clippy::too_many_lines)]

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

/// Fresh per-test project root under target/ (auto-cleaned). Sets up the
/// canonical Forge sidecar layout: a consumer project dir with a
/// `.forge-method.yaml` pointing at a sibling sidecar whose `.forge-method`
/// state_root holds the research Source Ledger. `--root` is the returned
/// project dir.
fn fresh_sidecar(label: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let base = repo_root().join("target").join(format!(
        "research-cli-e2e-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    let project = base.join("project");
    let sidecar = base.join("sidecar");
    let state_root = sidecar.join(".forge-method");
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::create_dir_all(&state_root).expect("create state root");
    // Canonical Project Link: state_root inside sidecar_root (sibling of
    // project), ending with .forge-method — satisfies resolve_project.
    std::fs::write(
        project.join(".forge-method.yaml"),
        "schema_version: forge_project_link_v1\nproject_id: research-e2e\nsidecar_root: ../sidecar\nstate_root: ../sidecar/.forge-method\n",
    )
    .expect("write project link");
    project
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

/// The canonical F14 E2E: source add → source list → cite (resolved) → cite
/// (unresolved) → check → graph. Each step asserts the wire envelope.
#[test]
fn research_lifecycle_add_list_cite_check_graph() {
    let sidecar = fresh_sidecar("lifecycle");
    let source = example("research-source.yaml");
    let policy = example("research-policy.yaml");

    // 1. source add → admitted, sequence 1.
    let out = bin()
        .args(["research", "source", "add", "--source-file"])
        .arg(&source)
        .arg("--policy-file")
        .arg(&policy)
        .arg("--root")
        .arg(&sidecar)
        .output()
        .expect("run source add");
    assert!(
        out.status.success(),
        "source add should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let json = output_json(&out);
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "research source add");
    assert_eq!(json["data"]["source_id"], "research.source.swe-agent");
    assert_eq!(json["data"]["sequence"], 1);

    // 2. source list → the source is live.
    let out = bin()
        .args(["research", "source", "list", "--root"])
        .arg(&sidecar)
        .output()
        .expect("run source list");
    assert!(out.status.success(), "source list should pass");
    let json = output_json(&out);
    assert_eq!(json["data"]["count"], 1);
    let sources = json["data"]["sources"].as_array().expect("sources array");
    assert_eq!(sources[0]["source_id"], "research.source.swe-agent");
    assert_eq!(sources[0]["kind"], "paper");

    // 3. cite the admitted source → resolved in the runtime backing.
    let out = bin()
        .args([
            "research",
            "cite",
            "--source-id",
            "research.source.swe-agent",
        ])
        .arg("--root")
        .arg(&sidecar)
        .output()
        .expect("run cite resolved");
    assert!(out.status.success(), "cite resolved should pass");
    let json = output_json(&out);
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["resolved"], true);
    assert_eq!(json["data"]["backing"], "runtime");

    // 4. cite an unresolved id → RejectedByGate (exit 2).
    let out = bin()
        .args(["research", "cite", "--source-id", "ghost.unregistered"])
        .arg("--root")
        .arg(&sidecar)
        .output()
        .expect("run cite unresolved");
    assert_eq!(out.status.code(), Some(2), "unresolved cite exits 2");
    let json = output_json(&out);
    assert_eq!(json["ok"], false);
    assert_eq!(json["exit_reason"], "rejected_by_gate");
    assert_eq!(json["data"]["resolved"], false);

    // 5. check → citation check over the workspace. The repo's curated registry
    //    has no unresolved source_ids (anchor 125 invariant), and the runtime
    //    ledger now has the admitted source, so the check passes.
    let out = bin()
        .args(["research", "check", "--root"])
        .arg(repo_root())
        .output()
        .expect("run check");
    assert!(
        out.status.success(),
        "check should pass (no unresolved source_ids)\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = output_json(&out);
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "research check");

    // 6. graph → the evidence graph indexes citing claims. The repo's contracts
    //    cite curated source_ids (e.g. in the field-evidence-registry-backed
    //    families), so the graph is non-empty.
    let out = bin()
        .args(["research", "graph", "--root"])
        .arg(repo_root())
        .output()
        .expect("run graph");
    assert!(out.status.success(), "graph should pass");
    let json = output_json(&out);
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "research graph");
    // The graph is deterministic; source_count is whatever the repo contracts
    // cite. We assert shape, not an exact count (the count tracks the curated
    // registry, which the anchor 125 suite already pins).
    assert!(
        json["data"]["graph"].is_array(),
        "graph entries must be an array"
    );
}

/// The admission gate denies a source that violates the policy (missing
/// content_hash) and appends NOTHING to the ledger (fail-closed).
#[test]
fn research_source_add_denied_by_gate_appends_nothing() {
    let sidecar = fresh_sidecar("reject");
    let policy = example("research-policy.yaml");

    // A source with no content_hash — the policy requires it.
    let bad_source = sidecar.join("bad-source.yaml");
    std::fs::write(
        &bad_source,
        "id: research.source.bad\nkind: paper\ntitle: \"No hash\"\nlocator: \"https://example.org\"\nfetched_at: 1\nharvested_by: \"agent.1\"\n",
    )
    .expect("write bad source");

    let out = bin()
        .args(["research", "source", "add", "--source-file"])
        .arg(&bad_source)
        .arg("--policy-file")
        .arg(&policy)
        .arg("--root")
        .arg(&sidecar)
        .output()
        .expect("run source add rejected");

    // RejectedByGate ⇒ exit code 2.
    assert_eq!(out.status.code(), Some(2), "denied admission exits 2");
    let json = output_json(&out);
    assert_eq!(json["ok"], false);
    assert_eq!(json["exit_reason"], "rejected_by_gate");
    let reasons = json["data"].as_array().expect("reasons in data");
    assert!(
        reasons.iter().any(|r| r
            .as_str()
            .is_some_and(|s| s.contains("missing_content_hash"))),
        "denial reasons must include missing_content_hash: {reasons:?}"
    );

    // A subsequent list reports zero sources — the denial appended nothing.
    let out = bin()
        .args(["research", "source", "list", "--root"])
        .arg(&sidecar)
        .output()
        .expect("run list");
    let json = output_json(&out);
    assert_eq!(
        json["data"]["count"], 0,
        "denied admission appended nothing"
    );
}

/// `--no-json` produces human-readable text output (the dual-output NFR).
#[test]
fn research_source_list_text_mode_is_human_readable() {
    let sidecar = fresh_sidecar("text");
    // An empty ledger still lists cleanly in text mode.
    let out = bin()
        .args(["research", "source", "list", "--root"])
        .arg(&sidecar)
        .arg("--no-json")
        .output()
        .expect("run list text");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("research source list") && stdout.contains("ok"),
        "text output should be a human one-liner: {stdout}"
    );
}

/// Unknown subcommand → structured usage error (exit 3).
#[test]
fn research_unknown_subcommand_returns_usage_error() {
    let out = bin()
        .args(["research", "frobnicate"])
        .output()
        .expect("run unknown subcommand");
    assert_eq!(out.status.code(), Some(3));
    let json = output_json(&out);
    assert_eq!(json["ok"], false);
    assert_eq!(json["exit_reason"], "invalid_decision_shape");
    assert!(json["error"]["message"]
        .as_str()
        .expect("message")
        .contains("unknown subcommand 'frobnicate'"));
}
