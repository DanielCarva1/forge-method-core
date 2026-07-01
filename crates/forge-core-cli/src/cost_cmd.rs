//! `forge-core cost` — F13 Budget/Cost Accounting.
//!
//! Loads `TraceEvents` from the project trace log (via `query_trace_events`),
//! hands them to `forge_core_trace::aggregate_costs`, and prints the report.
//! The aggregator is pure (unit-tested in `forge-core-trace`); this module
//! owns argv parsing, project/state-root resolution, and output formatting.
//!
//! Scope flags select which events feed the report:
//!
//! - `--run-id <id>`     → `CostScope::Run`    (all events for that run)
//! - `--graph-id <id>`   → `CostScope::Graph`  (post-filter; trace query is
//!                         run-scoped, so `graph_id` is applied client-side)
//! - `--principal <id>`  → `CostScope::Principal` (post-filter by actor field)
//! - `--last-run`        → `CostScope::Run` for the most recent run
//! - (none)              → `CostScope::All`    (every scanned event)

use crate::cli_error::ExitError;
use crate::cli_util::usage;
use crate::project_cmd::{resolve_project, ProjectResolvePayload};
use forge_core_contracts::CliEnvelope;
use forge_core_store::{query_trace_events, TraceEventQuery};
use forge_core_trace::{aggregate_costs, CostReport, CostScope};
use std::path::PathBuf;

/// Usage line for `forge-core cost`.
pub const COST_USAGE_LINE: &str =
    "forge-core cost [--root <path>] [--run-id <id> | --last-run] [--graph-id <id>] [--principal <id>] [--allow-bootstrap-core] [--json|--no-json]";

/// Runs the `forge-core cost` subcommand.
///
/// # Errors
///
/// Returns `ExitError::usage` on a malformed argv and `ExitError::env_config`
/// when project resolution fails or the sidecar state root is missing.
pub fn run_cost_command(args: &[String]) -> Result<(), ExitError> {
    let mut root = PathBuf::from(".");
    let mut run_id: Option<String> = None;
    let mut graph_id: Option<String> = None;
    let mut principal_id: Option<String> = None;
    let mut latest_run = false;
    let mut allow_bootstrap_core = false;
    let mut want_json = true;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = next_path_or_err(args, index)?;
            }
            "--run-id" => {
                index += 1;
                run_id = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--graph-id" => {
                index += 1;
                graph_id = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--principal" => {
                index += 1;
                principal_id = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--last-run" => latest_run = true,
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--no-json" | "--text" => want_json = false,
            "--json" => want_json = true,
            "--help" | "-h" => {
                println!("{COST_USAGE_LINE}");
                return Ok(());
            }
            _ => return Err(ExitError::usage(usage())),
        }
        index += 1;
    }

    let command = "cost";
    let resolved = resolve_project(&root, allow_bootstrap_core)
        .map_err(|error| ExitError::env_config(format!("cost: project resolve failed: {error}")))?;
    let state_root = existing_state_root(&resolved)?;

    // --last-run without --run-id: let the trace store resolve the most
    // recent run server-side via latest_run=true.
    let effective_latest = latest_run && run_id.is_none();
    let query = TraceEventQuery {
        run_id: run_id.clone(),
        latest_run: effective_latest,
        ..TraceEventQuery::default()
    };
    let result = query_trace_events(&state_root, &query);

    // Apply graph/principal post-filters (trace query is run-scoped) and
    // derive the report scope label.
    let (scope, scope_id, events) = select_scope(
        result.events,
        run_id.as_deref(),
        graph_id.as_deref(),
        principal_id.as_deref(),
    );
    let report = aggregate_costs(&events, scope, &scope_id);

    if want_json {
        let envelope = CliEnvelope::ok(command, report);
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope).expect("serialize cost report")
        );
    } else {
        print_human(&report);
    }
    Ok(())
}

fn existing_state_root(resolved: &ProjectResolvePayload) -> Result<PathBuf, ExitError> {
    let state_root = PathBuf::from(&resolved.state_root);
    if !resolved.state_exists && !resolved.bootstrap_core_exception {
        return Err(ExitError::env_config(format!(
            "cost: sidecar state root {} does not exist",
            state_root.display(),
        )));
    }
    Ok(state_root)
}

fn select_scope(
    events: Vec<forge_core_trace::TraceEvent>,
    run_id: Option<&str>,
    graph_id: Option<&str>,
    principal_id: Option<&str>,
) -> (CostScope, String, Vec<forge_core_trace::TraceEvent>) {
    let mut filtered = events;
    if let Some(graph) = graph_id {
        filtered.retain(|event| event.graph_id.as_deref() == Some(graph));
    }
    if let Some(principal) = principal_id {
        filtered.retain(|event| event.actor.principal_id == principal);
    }
    let (scope, scope_id) = match (principal_id, graph_id, run_id) {
        (Some(principal), _, _) => (CostScope::Principal, principal.to_string()),
        (None, Some(graph), _) => (CostScope::Graph, graph.to_string()),
        (None, None, Some(run)) => (CostScope::Run, run.to_string()),
        (None, None, None) => (CostScope::All, "*".to_string()),
    };
    (scope, scope_id, filtered)
}

fn print_human(report: &CostReport) {
    println!(
        "cost scope={:?} scope_id={} events={} model_calls={} tool_calls={} estimated_tokens={}",
        report.scope,
        report.scope_id,
        report.totals.event_count,
        report.totals.model_calls,
        report.totals.tool_calls,
        report.totals.estimated_tokens,
    );
    if !report.by_run.is_empty() {
        println!("by_run:");
        for entry in &report.by_run {
            println!(
                "  {} events={} tokens={}",
                entry.key, entry.totals.event_count, entry.totals.estimated_tokens,
            );
        }
    }
    if !report.by_agent.is_empty() {
        println!("by_agent:");
        for entry in &report.by_agent {
            println!(
                "  {} events={} tokens={}",
                entry.key, entry.totals.event_count, entry.totals.estimated_tokens,
            );
        }
    }
}

fn next_path_or_err(args: &[String], index: usize) -> Result<PathBuf, ExitError> {
    Ok(PathBuf::from(next_arg_or_err(args, index)?))
}

fn next_arg_or_err(args: &[String], index: usize) -> Result<&str, ExitError> {
    args.get(index).map(String::as_str).ok_or_else(|| {
        ExitError::usage(format!("cost: missing value for flag at position {index}"))
    })
}
