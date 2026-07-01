//! F05.7 — `forge-core eval-harness` E2E.
//!
//! Drives the full pipeline end to end: a valid config + tiny router corpus +
//! mock arms (subprocesses that copy a pre-written raw report into the output
//! path), then asserts the comparison report has the expected shape. Also
//! covers the fail-closed path (invalid config rejected with a nonzero exit).
//!
//! The mock arm is a plain file copy so it runs on both the Windows MSVC
//! target (via `cmd /c copy`) and the Linux CI target (via `cp`); no quoting
//! of JSON is needed because the report bytes are written by the test.

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

/// Builds a temp project root containing a config, a 3-case router corpus, and
/// a fixed mock raw-report the arms copy into their output paths.
struct HarnessScaffold {
    root: PathBuf,
    config_path: PathBuf,
    mock_report: PathBuf,
}

fn fresh_harness(label: &str, pass_two_arms_with_same_mock: bool) -> HarnessScaffold {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let root = repo_root()
        .join("target")
        .join(format!("eval-harness-e2e-{label}-{n}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");

    // 3-case router corpus: one task the mock answers correctly ("brainstorming"),
    // two it gets wrong. So every arm scores 1/3 -- enough to exercise grader
    // pass + fail and produce a real comparison.
    let corpus = root.join("corpus.yaml");
    fs::write(
        &corpus,
        r#"eval_corpus:
  - utterance: "help me brainstorm"
    expected_workflow: brainstorming
    phase: 1-discovery
  - utterance: "write the prd"
    expected_workflow: write-spec
    phase: 2-specification
  - utterance: "break the spec into sprints"
    expected_workflow: plan-sprint
    phase: 3-plan
"#,
    )
    .expect("write corpus");

    let mock_report = root.join("mock-report.json");
    fs::write(
        &mock_report,
        r#"{"output":"brainstorming","usage":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120,"estimated_cost_usd_micros":500,"num_tool_calls":1,"num_turns":1}}"#,
    )
    .expect("write mock report");

    // Native path; double backslashes so the YAML double-quoted scalar
    // unescapes back to a single (Windows paths need this in YAML).
    let mock_src = mock_report.to_string_lossy().replace('\\', "\\\\");
    let copy_command = if cfg!(windows) {
        format!(
            "[\"cmd\", \"/c\", \"copy\", \"/Y\", \"{mock_src}\", \"{{output_file}}\"]"
        )
    } else {
        format!("[\"cp\", \"{mock_src}\", \"{{output_file}}\"]")
    };

    let arms = if pass_two_arms_with_same_mock {
        format!(
            "  arms:\n    - label: \"single-agent\"\n      command: {copy_command}\n      timeout_ms: 15000\n    - label: \"mas\"\n      command: {copy_command}\n      timeout_ms: 15000\n"
        )
    } else {
        // Duplicate labels -> fail-closed validation.
        format!(
            "  arms:\n    - label: \"single-agent\"\n      command: {copy_command}\n    - label: \"single-agent\"\n      command: {copy_command}\n"
        )
    };

    let config_yaml = format!(
        r#"schema_version: "eval-harness-v0"
eval_harness_config:
  id: "eval.harness.e2e"
  corpus_ref: "corpus.yaml"
  run_dir: "eval-runs"
{arms}  policy:
    minimum_task_count: 3
    require_matching_tasks: true
"#,
    );
    let config_path = root.join("config.yaml");
    fs::write(&config_path, config_yaml).expect("write config");

    HarnessScaffold {
        root,
        config_path,
        mock_report,
    }
}

#[test]
fn eval_harness_runs_and_emits_report() {
    let scaffold = fresh_harness("run", true);
    let output = bin()
        .arg("eval-harness")
        .arg("--config")
        .arg(&scaffold.config_path)
        .arg("--root")
        .arg(&scaffold.root)
        .arg("--json")
        .output()
        .expect("run forge-core eval-harness");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "eval-harness should succeed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let value: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|error| {
            panic!(
                "stdout should be JSON envelope; parse error: {error}\nstdout:\n{stdout}\nstderr:\n{stderr}"
            )
        });
    let data = &value["data"];
    assert_eq!(data["task_count"], 3, "all 3 corpus tasks ran");
    assert_eq!(data["baseline"], "single-agent");
    assert_eq!(data["candidate"], "mas");
    // Mock answers "brainstorming" for every task: 1 pass, 2 fails per arm.
    assert_eq!(data["baseline_summary"]["successes"], 1);
    assert_eq!(data["candidate_summary"]["successes"], 1);

    let _ = fs::remove_dir_all(&scaffold.root);
}

#[test]
fn eval_harness_rejects_invalid_config() {
    let scaffold = fresh_harness("invalid", false);
    let output = bin()
        .arg("eval-harness")
        .arg("--config")
        .arg(&scaffold.config_path)
        .arg("--root")
        .arg(&scaffold.root)
        .output()
        .expect("run forge-core eval-harness");

    assert!(
        !output.status.success(),
        "invalid config must exit nonzero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("DuplicateArmLabel") || stderr.contains("not unique"),
        "stderr should mention the duplicate label diagnostic, got: {stderr}"
    );
    let _ = fs::remove_dir_all(&scaffold.root);
    let _ = &scaffold.mock_report;
}
