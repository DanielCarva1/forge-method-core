//! Subprocess executor for eval arms (the impure half of F05.3).
//!
//! For each (arm, task) the executor:
//!   1. writes a per-task input file at `{run_dir}/{arm}/{task}.task.yaml`,
//!   2. substitutes placeholders in the arm's argv,
//!   3. spawns the subprocess, measuring wall time externally,
//!   4. reads the raw JSON report the arm wrote at `{output_file}`,
//!   5. canonicalises it into an `EvalRunContractDocument`.
//!
//! The executor ALWAYS returns one contract per (arm, task). On any failure
//! (spawn error, nonzero exit, timeout, missing/invalid output) it returns an
//! `Error`-verdict contract via [`build_error_contract`], so no run is silently
//! dropped and the comparison stays complete.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use forge_core_contracts::eval_run::EvalFailureCluster;

use crate::{
    build_error_contract, build_run_contract, parse_raw_report, substitute_placeholders,
    EvalHarnessArm, EvalTask,
};
use forge_core_contracts::EvalRunContractDocument;

/// Poll interval for the timeout watchdog. Coarse is fine: the harness runs at
/// human-timescale (dozens of tasks), not micro-benchmark timescale.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Runs one (arm, task) end to end and returns the canonical contract. Never
/// panics; failures become `Error`-verdict contracts.
#[must_use]
pub fn execute_run(
    arm: &EvalHarnessArm,
    task: &EvalTask,
    run_dir: &Path,
    evaluated_at: &str,
) -> EvalRunContractDocument {
    let arm_dir = run_dir.join(arm.label.as_str());
    let _ = fs::create_dir_all(&arm_dir);
    let task_file = arm_dir.join(format!("{}.task.yaml", task.task_id.0));
    let output_file = arm_dir.join(format!("{}.out.json", task.task_id.0));
    let task_file_str = task_file.to_string_lossy();
    let output_file_str = output_file.to_string_lossy();

    // Write the task as a small YAML so a naive arm can read it. The expected
    // field is included so a deterministic mock arm (used in E2E) can echo it;
    // real arms are expected to produce the answer from the input alone.
    let task_payload = format!(
        "task_id: \"{}\"\ninput: \"{}\"\nexpected: \"{}\"\n",
        task.task_id.0,
        yaml_escape(&task.input),
        yaml_escape(&task.expected)
    );
    if fs::write(&task_file, task_payload).is_err() {
        return build_error_contract(
            arm.label,
            task,
            EvalFailureCluster::ToolError,
            format!("harness could not write task file at {task_file_str}"),
            evaluated_at,
        );
    }

    let argv = substitute_placeholders(
        &arm.command,
        &task_file_str,
        &task.task_id.0,
        &output_file_str,
    );
    let Some((program, rest)) = argv.split_first() else {
        return build_error_contract(
            arm.label,
            task,
            EvalFailureCluster::ToolError,
            "arm command was empty after substitution",
            evaluated_at,
        );
    };

    let start = Instant::now();
    let mut command = Command::new(program);
    command.args(rest);
    // The child's stdout/stderr would otherwise inherit the harness's streams
    // and pollute any JSON we print. We only need the exit status and the file
    // the arm writes at {output_file}.
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    let spawn_result = command.spawn();
    let mut child = match spawn_result {
        Ok(child) => child,
        Err(error) => {
            return build_error_contract(
                arm.label,
                task,
                EvalFailureCluster::ToolError,
                format!("failed to spawn arm '{program}': {error}"),
                evaluated_at,
            );
        }
    };

    let outcome = wait_with_timeout(&mut child, arm.timeout_ms, &start);
    let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);

    match outcome {
        WaitOutcome::TimedOut => build_error_contract(
            arm.label,
            task,
            EvalFailureCluster::Timeout,
            format!("arm exceeded timeout_ms ({:?})", arm.timeout_ms),
            evaluated_at,
        ),
        WaitOutcome::Exited(status) => {
            if !status.success() {
                return build_error_contract(
                    arm.label,
                    task,
                    EvalFailureCluster::BuildFailure,
                    format!("arm exited with status {status}"),
                    evaluated_at,
                );
            }
            read_and_canonicalize(arm, task, &output_file_str, elapsed_ms, evaluated_at)
        }
    }
}

/// Reads the arm's output file, parses the raw report, and canonicalises it.
/// Any failure becomes an `Error`-verdict contract. Extracted from
/// [`execute_run`] to keep that function under the pedantic line budget.
fn read_and_canonicalize(
    arm: &EvalHarnessArm,
    task: &EvalTask,
    output_file_str: &str,
    elapsed_ms: u64,
    evaluated_at: &str,
) -> EvalRunContractDocument {
    match fs::read_to_string(output_file_str) {
        Ok(json) => match parse_raw_report(&json) {
            Ok(report) => build_run_contract(
                arm.label,
                task,
                &report,
                elapsed_ms,
                evaluated_at,
                Some(output_file_str),
            ),
            Err(error) => build_error_contract(
                arm.label,
                task,
                EvalFailureCluster::ToolError,
                format!("arm output at {output_file_str} was not a valid report: {error}"),
                evaluated_at,
            ),
        },
        Err(error) => build_error_contract(
            arm.label,
            task,
            EvalFailureCluster::ToolError,
            format!("arm did not write output file at {output_file_str}: {error}"),
            evaluated_at,
        ),
    }
}

enum WaitOutcome {
    Exited(std::process::ExitStatus),
    TimedOut,
}

fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout_ms: Option<u64>,
    start: &Instant,
) -> WaitOutcome {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return WaitOutcome::Exited(status),
            Ok(None) => {}
            Err(_) => return WaitOutcome::Exited(std::process::ExitStatus::default()),
        }
        if let Some(limit) = timeout_ms {
            if start.elapsed().as_millis() >= u128::from(limit) {
                let _ = child.kill();
                let _ = child.wait();
                return WaitOutcome::TimedOut;
            }
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Minimal single-line YAML string escaping (quotes + backslash). Sufficient
/// for the task payload this harness writes; not a general YAML serializer.
fn yaml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::pedantic)]

    use super::*;
    use crate::{EvalTask, GraderKind};
    use forge_core_contracts::StableId;
    use forge_core_eval::EvalArmLabel;

    fn task() -> EvalTask {
        EvalTask {
            task_id: StableId("router-eval-000".to_string()),
            input: "help me brainstorm".to_string(),
            expected: "brainstorming".to_string(),
            grader_kind: GraderKind::ExactMatch,
        }
    }

    /// Runs `execute_run` against a tiny shell arm that writes the expected
    /// workflow, proving the whole spawn -> read -> grade -> canonicalise loop.
    #[cfg(unix)]
    #[test]
    fn execute_run_canonicalises_a_passing_shell_arm() {
        let dir = std::env::temp_dir().join(format!(
            "eval-harness-exec-pass-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let arm = EvalHarnessArm {
            label: EvalArmLabel::SingleAgent,
            command: vec![
                "sh".to_string(),
                "-c".to_string(),
                "echo '{\"output\":\"brainstorming\"}' > \"$1\"".to_string(),
                "placeholder".to_string(),
                "{output_file}".to_string(),
            ],
            timeout_ms: Some(10_000),
        };
        let document = execute_run(&arm, &task(), &dir, "2026-07-01T00:00:00Z");
        assert_eq!(
            document.eval_run_contract.outcome.value,
            forge_core_contracts::eval_run::EvalVerdict::Passed
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A nonexistent program yields an Error contract, never a panic.
    #[test]
    fn execute_run_returns_error_contract_when_program_missing() {
        let arm = EvalHarnessArm {
            label: EvalArmLabel::SingleAgent,
            command: vec![
                "definitely-not-a-real-program-xyz".to_string(),
                "{task_id}".to_string(),
            ],
            timeout_ms: Some(2_000),
        };
        let document = execute_run(
            &arm,
            &task(),
            Path::new("/tmp/eval-harness-exec-missing"),
            "2026-07-01",
        );
        assert_eq!(
            document.eval_run_contract.outcome.value,
            forge_core_contracts::eval_run::EvalVerdict::Error
        );
        assert_eq!(
            document.eval_run_contract.outcome.failure_cluster,
            Some(EvalFailureCluster::ToolError)
        );
    }

    #[test]
    fn yaml_escape_quotes_backslashes() {
        assert_eq!(yaml_escape("a\"b\\c"), "a\\\"b\\\\c");
    }

    /// A unique temp dir per test so parallel `cargo test` threads do not
    /// collide on `{run_dir}/{arm}` paths.
    fn fresh_run_dir(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("eval-harness-{label}-{nanos}"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn reclaim(dir: &std::path::Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A nonzero-exit arm yields a `BuildFailure` Error contract, never a panic.
    /// Cross-platform: `sh -c "exit 7"` on Unix, `cmd /C "exit /b 7"` on Windows.
    #[cfg(unix)]
    #[test]
    fn execute_run_returns_build_failure_on_nonzero_exit() {
        let dir = fresh_run_dir("buildfail");
        let arm = EvalHarnessArm {
            label: EvalArmLabel::SingleAgent,
            command: vec![
                "sh".to_string(),
                "-c".to_string(),
                "exit 7".to_string(),
                "{task_id}".to_string(),
            ],
            timeout_ms: Some(5_000),
        };
        let document = execute_run(&arm, &task(), &dir, "2026-07-01");
        assert_eq!(
            document.eval_run_contract.outcome.value,
            forge_core_contracts::eval_run::EvalVerdict::Error
        );
        assert_eq!(
            document.eval_run_contract.outcome.failure_cluster,
            Some(EvalFailureCluster::BuildFailure)
        );
        reclaim(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn execute_run_returns_build_failure_on_nonzero_exit() {
        let dir = fresh_run_dir("buildfail");
        // cmd's `exit /b 7` sets the process exit code without closing the
        // console host. `cmd /C` runs the command and returns its code.
        let arm = EvalHarnessArm {
            label: EvalArmLabel::SingleAgent,
            command: vec![
                "cmd".to_string(),
                "/C".to_string(),
                "exit /b 7".to_string(),
                "{task_id}".to_string(),
            ],
            timeout_ms: Some(5_000),
        };
        let document = execute_run(&arm, &task(), &dir, "2026-07-01");
        assert_eq!(
            document.eval_run_contract.outcome.value,
            forge_core_contracts::eval_run::EvalVerdict::Error
        );
        assert_eq!(
            document.eval_run_contract.outcome.failure_cluster,
            Some(EvalFailureCluster::BuildFailure)
        );
        reclaim(&dir);
    }

    /// A long-running arm killed after `timeout_ms` yields a `Timeout` Error
    /// contract. The sleep must comfortably exceed POLL_INTERVAL (50ms) and the
    /// timeout so the watchdog has at least one poll cycle to detect it.
    ///
    /// Unix-only: there is no reliable sub-second blocking builtin on Windows
    /// that behaves identically under `Command` spawn with redirected stdio
    /// (`ping` to an unroutable host exits non-zero before the budget, and
    /// `timeout /t` rejects `Stdio::null()` stdin). CI runs Linux (ADR-0008),
    /// so coverage is preserved where it matters; this mirrors the existing
    /// `#[cfg(unix)]` gate on `execute_run_canonicalises_a_passing_shell_arm`.
    #[cfg(unix)]
    #[test]
    fn execute_run_returns_timeout_when_arm_overruns_budget() {
        let dir = fresh_run_dir("timeout");
        let arm = EvalHarnessArm {
            label: EvalArmLabel::Mas,
            command: vec![
                "sh".to_string(),
                "-c".to_string(),
                "sleep 10".to_string(),
                "{task_id}".to_string(),
            ],
            // 200ms budget: well above POLL_INTERVAL, well below the 10s sleep.
            timeout_ms: Some(200),
        };
        let document = execute_run(&arm, &task(), &dir, "2026-07-01");
        assert_eq!(
            document.eval_run_contract.outcome.value,
            forge_core_contracts::eval_run::EvalVerdict::Error
        );
        assert_eq!(
            document.eval_run_contract.outcome.failure_cluster,
            Some(EvalFailureCluster::Timeout)
        );
        reclaim(&dir);
    }

    /// A successful run that forgets to write the output file yields a
    /// `ToolError` Error contract (the missing-output branch of
    /// `read_and_canonicalize`). Distinct from the spawn-failure test above:
    /// here the process exits zero, but the artifact is absent.
    #[cfg(unix)]
    #[test]
    fn execute_run_returns_tool_error_when_output_file_missing() {
        let dir = fresh_run_dir("nooutput");
        let arm = EvalHarnessArm {
            label: EvalArmLabel::SingleAgent,
            command: vec![
                "sh".to_string(),
                "-c".to_string(),
                "true".to_string(), // exits 0, writes nothing
                "{task_id}".to_string(),
            ],
            timeout_ms: Some(5_000),
        };
        let document = execute_run(&arm, &task(), &dir, "2026-07-01");
        assert_eq!(
            document.eval_run_contract.outcome.value,
            forge_core_contracts::eval_run::EvalVerdict::Error
        );
        assert_eq!(
            document.eval_run_contract.outcome.failure_cluster,
            Some(EvalFailureCluster::ToolError)
        );
        assert!(
            document.eval_run_contract.outcome.notes.is_some(),
            "missing-output ToolError must carry a note"
        );
        reclaim(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn execute_run_returns_tool_error_when_output_file_missing() {
        let dir = fresh_run_dir("nooutput");
        let arm = EvalHarnessArm {
            label: EvalArmLabel::SingleAgent,
            command: vec![
                "cmd".to_string(),
                "/C".to_string(),
                "rem noop".to_string(), // exits 0, writes nothing
                "{task_id}".to_string(),
            ],
            timeout_ms: Some(5_000),
        };
        let document = execute_run(&arm, &task(), &dir, "2026-07-01");
        assert_eq!(
            document.eval_run_contract.outcome.value,
            forge_core_contracts::eval_run::EvalVerdict::Error
        );
        assert_eq!(
            document.eval_run_contract.outcome.failure_cluster,
            Some(EvalFailureCluster::ToolError)
        );
        assert!(
            document.eval_run_contract.outcome.notes.is_some(),
            "missing-output ToolError must carry a note"
        );
        reclaim(&dir);
    }
}
