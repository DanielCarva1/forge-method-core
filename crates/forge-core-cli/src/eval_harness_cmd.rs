//! `forge-core eval-harness` — F05 eval compare harness driver.
//!
//! Loads an `EvalHarnessConfig` (`--config <yaml>`), validates it, runs every
//! task in the shared corpus against the baseline and candidate arms (each a
//! subprocess), and prints the comparison report produced by the pure
//! `generate_comparison_report`. The heavy lifting (grader, canonicalisation,
//! subprocess spawn) lives in `forge-core-eval-harness`; this module owns argv
//! parsing, path resolution, and output formatting.
//!
//! Naming: the existing `forge-core eval compare` compares precomputed runs;
//! this command RUNS arms, so it is `eval-harness` (executes) to stay
//! unambiguous. `--root` anchors relative `corpus_ref`/`run_dir` paths; the
//! harness does not write Forge runtime state, so no `resolve_project` is
//! required for F05.5.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use forge_core_contracts::CliEnvelope;
use forge_core_contracts::EvalRunContractDocument;
use forge_core_eval::EvalComparisonReport;
use forge_core_eval_harness::{
    execute_run, generate_comparison_report, load_router_corpus, parse_harness_config,
    validate_harness_config_document, EvalHarnessArm, HarnessDiagnosticSeverity,
};

use crate::cli_error::ExitError;
use crate::cli_util::eval_harness_usage;

/// Runs the `forge-core eval-harness` subcommand.
///
/// # Errors
///
/// Returns `ExitError::usage` on a malformed argv, a config that fails
/// validation, or an unreadable config/corpus file.
///
/// # Panics
///
/// Panics if the comparison report cannot be serialized to JSON (a serde-derived
/// struct; never expected in practice).
pub fn run_eval_harness_command(args: &[String]) -> Result<(), ExitError> {
    let mut root = PathBuf::from(".");
    let mut config_path: Option<PathBuf> = None;
    let mut corpus_override: Option<PathBuf> = None;
    let mut want_json = true;

    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                index += 1;
                config_path = Some(next_path_or_err(args, index)?);
            }
            "--root" => {
                index += 1;
                root = next_path_or_err(args, index)?;
            }
            "--corpus" => {
                index += 1;
                corpus_override = Some(next_path_or_err(args, index)?);
            }
            "--no-json" | "--text" => want_json = false,
            "--json" => want_json = true,
            "--help" | "-h" => {
                println!("{}", eval_harness_usage());
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(eval_harness_usage()));
            }
        }
        index += 1;
    }

    let Some(config_path) = config_path else {
        return Err(ExitError::usage(eval_harness_usage()));
    };

    let config_yaml = std::fs::read_to_string(&config_path).map_err(|source| {
        ExitError::usage(format!(
            "eval-harness: read config {}: {source}",
            config_path.display()
        ))
    })?;
    let document = parse_harness_config(&config_yaml).map_err(|source| {
        ExitError::usage(format!(
            "eval-harness: parse config {}: {source}",
            config_path.display()
        ))
    })?;

    let diagnostics = validate_harness_config_document(&document);
    let has_errors = diagnostics
        .iter()
        .any(|d| d.severity == HarnessDiagnosticSeverity::Error);
    if has_errors {
        let messages: Vec<String> = diagnostics
            .iter()
            .filter(|d| d.severity == HarnessDiagnosticSeverity::Error)
            .map(|d| format!("{}: {}", d.path, d.message))
            .collect();
        return Err(ExitError::usage(format!(
            "eval-harness: config validation failed:\n  - {}",
            messages.join("\n  - ")
        )));
    }

    let config = &document.eval_harness_config;
    let corpus_path = corpus_override.unwrap_or_else(|| root.join(&config.corpus_ref.0));
    let corpus_yaml = std::fs::read_to_string(&corpus_path).map_err(|source| {
        ExitError::usage(format!(
            "eval-harness: read corpus {}: {source}",
            corpus_path.display()
        ))
    })?;
    let tasks = load_router_corpus(&corpus_yaml).map_err(|source| {
        ExitError::usage(format!(
            "eval-harness: load corpus {}: {source}",
            corpus_path.display()
        ))
    })?;

    if tasks.len() < config.policy.minimum_task_count {
        return Err(ExitError::usage(format!(
            "eval-harness: corpus has {} tasks, below minimum_task_count {}",
            tasks.len(),
            config.policy.minimum_task_count
        )));
    }

    let run_dir = root.join(&config.run_dir.0);
    let evaluated_at = now_iso();
    let baseline_arm = &config.arms[0];
    let candidate_arm = &config.arms[1];

    let baseline_documents = run_arm_tasks(baseline_arm, &tasks, &run_dir, &evaluated_at);
    let candidate_documents = run_arm_tasks(candidate_arm, &tasks, &run_dir, &evaluated_at);

    let report = generate_comparison_report(config, &baseline_documents, &candidate_documents);

    emit_trace_events(
        root.as_path(),
        config_path.as_path(),
        config.id.0.as_str(),
        &report,
    );

    if want_json {
        let envelope = CliEnvelope::ok("eval-harness", &report);
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope)
                .expect("EvalComparisonReport is serde-derived and serialisable")
        );
    } else {
        print_human(&report);
    }
    Ok(())
}

fn run_arm_tasks(
    arm: &EvalHarnessArm,
    tasks: &[forge_core_eval_harness::EvalTask],
    run_dir: &std::path::Path,
    evaluated_at: &str,
) -> Vec<EvalRunContractDocument> {
    tasks
        .iter()
        .map(|task| execute_run(arm, task, run_dir, evaluated_at))
        .collect()
}

fn print_human(report: &EvalComparisonReport) {
    println!(
        "eval-harness status={:?} baseline={:?} candidate={:?} recommendation={:?} tasks={}",
        report.status, report.baseline, report.candidate, report.recommendation, report.task_count
    );
    let b = &report.baseline_summary;
    let c = &report.candidate_summary;
    println!(
        "  baseline: runs={} success_rate_bps={} mean_cost_micros={} mean_wall_ms={}",
        b.run_count, b.success_rate_bps, b.mean_cost_usd_micros, b.mean_wall_time_ms
    );
    println!(
        "  candidate: runs={} success_rate_bps={} mean_cost_micros={} mean_wall_ms={}",
        c.run_count, c.success_rate_bps, c.mean_cost_usd_micros, c.mean_wall_time_ms
    );
    if !report.diagnostics.is_empty() {
        println!("  diagnostics:");
        for d in &report.diagnostics {
            println!("    [{:?}] {}: {}", d.severity, d.path, d.message);
        }
    }
}

/// Emits eval-harness trace events best-effort. Only persists when `<root>`
/// is a Forge project (has `.forge-method`), mirroring the risk-audit CLI so
/// the command never pollutes an unrelated tree. Trace failures are
/// non-fatal: logged to stderr, never change the comparison outcome.
fn emit_trace_events(
    root: &std::path::Path,
    config_path: &std::path::Path,
    config_id: &str,
    report: &EvalComparisonReport,
) {
    use forge_core_store::append_trace_event;

    let trace_state_root = root.join(".forge-method");
    if !trace_state_root.is_dir() {
        return;
    }
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let trace_id = format!("eval-harness-{config_id}-{now_unix}");
    let recorded_at = format!("unix:{now_unix}");
    let config_ref = config_path.to_string_lossy().to_string();
    let ctx = crate::eval_harness_trace::EvalHarnessTraceContext {
        trace_id: &trace_id,
        run_id: &trace_id,
        recorded_at: &recorded_at,
        principal_id: "forge-core",
        agent_id: "eval-harness",
        config_ref: &config_ref,
    };
    let events = crate::eval_harness_trace::build_eval_harness_events(
        &ctx,
        &crate::eval_harness_trace::EvalHarnessOutcome {
            status: report.status,
            baseline: report.baseline,
            candidate: report.candidate,
            task_count: report.task_count,
            baseline_success_bps: report.baseline_summary.success_rate_bps,
            candidate_success_bps: report.candidate_summary.success_rate_bps,
            recommendation: format!("{:?}", report.recommendation),
            diagnostic_count: report.diagnostics.len(),
        },
    );
    for event in &events {
        if let Err(source) = append_trace_event(&trace_state_root, event) {
            eprintln!("forge-core: eval-harness trace append failed (non-fatal): {source}");
        }
    }
}

/// Wall-clock stamp for `evaluated_at`. Uses unix seconds (no chrono dep); the
/// field is free-form text and the comparison never keys on its format.
fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("unix:{secs}")
}

fn next_path_or_err(args: &[String], index: usize) -> Result<PathBuf, ExitError> {
    Ok(PathBuf::from(next_arg_or_err(args, index)?))
}

fn next_arg_or_err(args: &[String], index: usize) -> Result<&str, ExitError> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| ExitError::usage(eval_harness_usage()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_command_surface::COMMAND_EVAL_HARNESS;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn missing_flag_value_reports_eval_harness_usage() {
        let error = run_eval_harness_command(&args(&["eval-harness", "--config"]))
            .expect_err("missing config value must fail before config I/O");
        assert_eval_harness_usage_error(&error);
    }

    #[test]
    fn missing_config_reports_eval_harness_usage() {
        let error = run_eval_harness_command(&args(&["eval-harness"]))
            .expect_err("missing required config must fail before config I/O");
        assert_eval_harness_usage_error(&error);
    }

    #[test]
    fn unknown_arg_reports_eval_harness_usage() {
        let error = run_eval_harness_command(&args(&["eval-harness", "--frobnicate"]))
            .expect_err("unknown argument must fail before config I/O");
        assert_eval_harness_usage_error(&error);
    }

    fn assert_eval_harness_usage_error(error: &ExitError) {
        for line in COMMAND_EVAL_HARNESS.usage_lines {
            let projected = line.trim_start();
            assert!(
                error.message().contains(projected),
                "eval-harness usage error should include projected Command Surface line {projected:?}: {error}"
            );
        }
        assert!(
            !error.message().contains("forge-core execute-operation"),
            "eval-harness usage error must not include unrelated mutating command usage: {error}"
        );
    }
}
