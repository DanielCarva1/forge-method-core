//! Effect-target metadata index commands.
//!
//! Thin CLI-facing wrappers around the storage-layer index APIs in
//! `forge_core_store`:
//! - [`run_rebuild_effect_index`] — rebuild the on-disk effect-targets index
//!   from the WAL under an advisory file lock.
//! - [`run_query_effect_index`] — query the index for one logical ref /
//!   effect / operation / target kind.
//! - [`run_query_effect_index_context`] — same query, but additionally
//!   collapses the result rows into a per-target "context" view via
//!   [`forge_core_store::build_effect_metadata_context`].
//!
//! This module is `pub(crate)` and re-exported from the crate root so that
//! `main.rs` and `tests/validate.rs` keep importing the public types and
//! functions directly from `forge_core_cli`.

use std::path::PathBuf;

use crate::cli_error::ExitError;
use crate::cli_util::{command_surface_usage, resolve_stateful_roots_or_err};
use forge_core_command_surface::{
    CommandSpec, COMMAND_QUERY_EFFECT_INDEX, COMMAND_REBUILD_EFFECT_INDEX,
};
use forge_core_contracts::{runtime::RuntimeKind, tool_effect::EffectTargetKind, StableId};
use forge_core_store::{
    build_effect_metadata_context, query_effect_target_metadata_index,
    rebuild_effect_target_metadata_index_with_lock_with_durability, EffectMetadataAdapterTrigger,
    EffectMetadataConsumerUse, EffectMetadataContextBuildOptions, EffectMetadataContextBuildResult,
    EffectTargetMetadataIndexQuery, EffectTargetMetadataIndexQueryResult,
    EffectTargetMetadataIndexRebuildResult, WalDurability,
};

/// Inputs for [`run_rebuild_effect_index`].
#[derive(Debug, Clone)]
pub struct RebuildEffectIndexInput {
    pub root: PathBuf,
    pub wal_relative_path: String,
    pub index_relative_path: String,
    pub lock_relative_path: String,
    pub recorded_at: Option<String>,
    /// WAL durability for the rebuild append (ADR-0009). Default
    /// `SyncOnAppend`; CLI sets `NoSync` when the user passes `--no-sync`.
    pub durability: WalDurability,
}

impl Default for RebuildEffectIndexInput {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            wal_relative_path: ".forge-method/wal/effects.ndjson".to_string(),
            index_relative_path: ".forge-method/index/effect-targets.ndjson".to_string(),
            lock_relative_path: ".forge-method/locks/effects.lock".to_string(),
            recorded_at: None,
            durability: WalDurability::default(),
        }
    }
}

/// Rebuild the effect-target metadata index on disk under a lock.
#[must_use]
pub fn run_rebuild_effect_index(
    input: RebuildEffectIndexInput,
) -> EffectTargetMetadataIndexRebuildResult {
    rebuild_effect_target_metadata_index_with_lock_with_durability(
        &input.root,
        &input.wal_relative_path,
        &input.index_relative_path,
        &input.lock_relative_path,
        input.recorded_at.as_deref(),
        input.durability,
    )
}

/// Inputs for [`run_query_effect_index`] / [`run_query_effect_index_context`].
#[derive(Debug, Clone)]
pub struct QueryEffectIndexInput {
    pub root: PathBuf,
    pub index_relative_path: String,
    pub logical_ref: Option<String>,
    pub effect_id: Option<String>,
    pub operation_id: Option<String>,
    pub target_kind: Option<EffectTargetKind>,
    pub latest_per_target: bool,
    pub consumer_use: EffectMetadataConsumerUse,
    pub context_options: EffectMetadataContextBuildOptions,
}

impl Default for QueryEffectIndexInput {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            index_relative_path: ".forge-method/index/effect-targets.ndjson".to_string(),
            logical_ref: None,
            effect_id: None,
            operation_id: None,
            target_kind: None,
            latest_per_target: false,
            consumer_use: EffectMetadataConsumerUse::Discovery,
            context_options: EffectMetadataContextBuildOptions::default(),
        }
    }
}

/// Query the effect-target metadata index for matching rows.
pub fn run_query_effect_index(
    input: QueryEffectIndexInput,
) -> EffectTargetMetadataIndexQueryResult {
    query_effect_target_metadata_index(
        &input.root,
        &input.index_relative_path,
        &EffectTargetMetadataIndexQuery {
            logical_ref: input.logical_ref,
            effect_id: input.effect_id.map(StableId),
            operation_id: input.operation_id.map(StableId),
            target_kind: input.target_kind,
            latest_per_target: input.latest_per_target,
            consumer_use: input.consumer_use,
        },
    )
}

/// Query the index and collapse the rows into a per-target "context" view.
#[must_use]
pub fn run_query_effect_index_context(
    input: QueryEffectIndexInput,
) -> EffectMetadataContextBuildResult {
    let context_options = input.context_options.clone();
    let query_result = run_query_effect_index(input);
    build_effect_metadata_context(&query_result, &context_options)
}
/// Runs the `forge-core rebuild-effect-index` command.
///
/// # Errors
///
/// Returns `ExitError::usage` when an unknown flag is present or a value
/// helper reports a missing/malformed argument, `ExitError::failed` when
/// project resolution or the rebuild itself reports a failure.
///
/// # Panics
///
/// Panics in JSON mode if the rebuild result cannot be serialized. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_rebuild_effect_index_command(args: &[String]) -> Result<(), ExitError> {
    let command = &COMMAND_REBUILD_EFFECT_INDEX;
    let mut input = RebuildEffectIndexInput::default();
    let mut json = false;
    let mut no_sync = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                input.root = next_effect_index_path_or_err(args, index, command)?;
            }
            "--wal" => {
                index += 1;
                input.wal_relative_path =
                    next_effect_index_arg_or_err(args, index, command)?.to_string();
            }
            "--index" => {
                index += 1;
                input.index_relative_path =
                    next_effect_index_arg_or_err(args, index, command)?.to_string();
            }
            "--lock" => {
                index += 1;
                input.lock_relative_path =
                    next_effect_index_arg_or_err(args, index, command)?.to_string();
            }
            "--recorded-at" => {
                index += 1;
                input.recorded_at =
                    Some(next_effect_index_arg_or_err(args, index, command)?.to_string());
            }
            "--no-sync" => {
                no_sync = true;
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", effect_index_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(effect_index_usage(command)));
            }
        }
        index += 1;
    }

    let roots =
        resolve_stateful_roots_or_err("rebuild-effect-index", &input.root)?;
    input.root = roots.effect_store_root;
    if no_sync {
        // ADR-0009: emit a one-line stderr warning the first time the flag is
        // honoured, so a CI log makes the durability trade-off visible.
        // Suppressed in --json mode to keep stdout/stderr clean for machine
        // consumers (MCP, agents).
        if !json {
            eprintln!(
                "forge-core: --no-sync active; index rebuild append is not durable for this process"
            );
        }
        input.durability = WalDurability::NoSync;
    }

    let result = run_rebuild_effect_index(input);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).expect("serialize rebuild result")
        );
    } else {
        println!(
            "forge_core_effect_index_rebuild status={:?} rebuilt={} appended={} reasons={:?}",
            result.status, result.rebuilt_records, result.appended_records, result.reasons
        );
    }
    if result.status == forge_core_store::EffectTargetMetadataIndexRebuildStatus::Failed {
        return Err(ExitError::failed("effect index rebuild failed"));
    }
    Ok(())
}

/// Runs the `forge-core query-effect-index` command (with optional
/// `--context` flag for grouped context queries).
///
/// # Errors
///
/// Returns `ExitError::usage` when an unknown flag is present or a value
/// helper reports a missing/malformed argument, `ExitError::failed` when
/// project resolution or the underlying query reports a failure.
pub fn run_query_effect_index_command(args: &[String]) -> Result<(), ExitError> {
    let command = &COMMAND_QUERY_EFFECT_INDEX;
    let mut input = QueryEffectIndexInput::default();
    let mut json = false;
    let mut context = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                input.root = next_effect_index_path_or_err(args, index, command)?;
            }
            "--index" => {
                index += 1;
                input.index_relative_path =
                    next_effect_index_arg_or_err(args, index, command)?.to_string();
            }
            "--logical-ref" => {
                index += 1;
                input.logical_ref =
                    Some(next_effect_index_arg_or_err(args, index, command)?.to_string());
            }
            "--effect-id" => {
                index += 1;
                input.effect_id =
                    Some(next_effect_index_arg_or_err(args, index, command)?.to_string());
            }
            "--operation-id" => {
                index += 1;
                input.operation_id =
                    Some(next_effect_index_arg_or_err(args, index, command)?.to_string());
            }
            "--target-kind" => {
                index += 1;
                input.target_kind = Some(parse_effect_target_kind_or_err(
                    next_effect_index_arg_or_err(args, index, command)?,
                    command,
                )?);
            }
            "--consumer-use" => {
                index += 1;
                input.consumer_use = parse_effect_metadata_consumer_use_or_err(
                    next_effect_index_arg_or_err(args, index, command)?,
                    command,
                )?;
            }
            "--context" => context = true,
            "--max-context-groups" => {
                index += 1;
                input.context_options.max_groups = parse_effect_index_usize_or_err(
                    next_effect_index_arg_or_err(args, index, command)?,
                    command,
                )?;
            }
            "--adapter-kind" => {
                index += 1;
                input.context_options.adapter_kind = Some(parse_effect_index_runtime_kind_or_err(
                    next_effect_index_arg_or_err(args, index, command)?,
                    command,
                )?);
            }
            "--adapter-trigger" => {
                index += 1;
                input.context_options.adapter_trigger =
                    parse_effect_metadata_adapter_trigger_or_err(
                        next_effect_index_arg_or_err(args, index, command)?,
                        command,
                    )?;
            }
            "--latest" => input.latest_per_target = true,
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", effect_index_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(effect_index_usage(command)));
            }
        }
        index += 1;
    }

    let roots =
        resolve_stateful_roots_or_err("query-effect-index", &input.root)?;
    input.root = roots.effect_store_root;
    emit_query_effect_index_result(input, context, json)
}

#[must_use]
fn effect_index_usage(command: &CommandSpec) -> String {
    command_surface_usage(command)
}

fn next_effect_index_arg_or_err<'a>(
    args: &'a [String],
    index: usize,
    command: &CommandSpec,
) -> Result<&'a str, ExitError> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| ExitError::usage(effect_index_usage(command)))
}

fn next_effect_index_path_or_err(
    args: &[String],
    index: usize,
    command: &CommandSpec,
) -> Result<PathBuf, ExitError> {
    Ok(PathBuf::from(next_effect_index_arg_or_err(
        args, index, command,
    )?))
}

fn parse_effect_index_usize_or_err(value: &str, command: &CommandSpec) -> Result<usize, ExitError> {
    value
        .parse::<usize>()
        .map_err(|_| ExitError::usage(effect_index_usage(command)))
}

fn parse_effect_target_kind_or_err(
    value: &str,
    command: &CommandSpec,
) -> Result<EffectTargetKind, ExitError> {
    match value {
        "file_path" => Ok(EffectTargetKind::FilePath),
        "glob" => Ok(EffectTargetKind::Glob),
        "state_key" => Ok(EffectTargetKind::StateKey),
        "artifact_id" => Ok(EffectTargetKind::ArtifactId),
        "evidence_id" => Ok(EffectTargetKind::EvidenceId),
        "ledger_stream" => Ok(EffectTargetKind::LedgerStream),
        "request_stream" => Ok(EffectTargetKind::RequestStream),
        "completion_id" => Ok(EffectTargetKind::CompletionId),
        _ => Err(ExitError::usage(effect_index_usage(command))),
    }
}

fn parse_effect_index_runtime_kind_or_err(
    value: &str,
    command: &CommandSpec,
) -> Result<RuntimeKind, ExitError> {
    match value {
        "codex" => Ok(RuntimeKind::Codex),
        "cursor" => Ok(RuntimeKind::Cursor),
        "claude" => Ok(RuntimeKind::Claude),
        "opencode" => Ok(RuntimeKind::Opencode),
        "vscode" => Ok(RuntimeKind::Vscode),
        "pidev" => Ok(RuntimeKind::Pidev),
        "forge_standalone" => Ok(RuntimeKind::ForgeStandalone),
        "custom" => Ok(RuntimeKind::Custom),
        _ => Err(ExitError::usage(effect_index_usage(command))),
    }
}

fn parse_effect_metadata_consumer_use_or_err(
    value: &str,
    command: &CommandSpec,
) -> Result<EffectMetadataConsumerUse, ExitError> {
    match value {
        "discovery" => Ok(EffectMetadataConsumerUse::Discovery),
        "diagnostics" => Ok(EffectMetadataConsumerUse::Diagnostics),
        "handoff_context" => Ok(EffectMetadataConsumerUse::HandoffContext),
        _ => Err(ExitError::usage(effect_index_usage(command))),
    }
}

fn parse_effect_metadata_adapter_trigger_or_err(
    value: &str,
    command: &CommandSpec,
) -> Result<EffectMetadataAdapterTrigger, ExitError> {
    match value {
        "evidence_discovery" => Ok(EffectMetadataAdapterTrigger::EvidenceDiscovery),
        "diagnostics" => Ok(EffectMetadataAdapterTrigger::Diagnostics),
        "handoff_preparation" => Ok(EffectMetadataAdapterTrigger::HandoffPreparation),
        "manual_inspection" => Ok(EffectMetadataAdapterTrigger::ManualInspection),
        _ => Err(ExitError::usage(effect_index_usage(command))),
    }
}

/// Executes the resolved query (plain or context mode) and prints the
/// result, propagating non-zero exit codes through [`ExitError`].
///
/// # Errors
///
/// Returns `ExitError::failed` when the underlying query reports a
/// `Failed` status.
///
/// # Panics
///
/// Panics in JSON mode if the query result cannot be serialized. The
/// result type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn emit_query_effect_index_result(
    input: QueryEffectIndexInput,
    context: bool,
    json: bool,
) -> Result<(), ExitError> {
    if context {
        let result = run_query_effect_index_context(input);
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("serialize context result")
            );
        } else {
            println!(
                "forge_core_effect_index_context status={:?} total_groups={} returned_groups={} omitted_groups={} reasons={:?}",
                result.status, result.total_groups, result.returned_groups, result.omitted_groups, result.reasons
            );
        }
        if result.source_status == forge_core_store::EffectTargetMetadataIndexQueryStatus::Failed {
            return Err(ExitError::failed("effect index context query failed"));
        }
        return Ok(());
    }

    let result = run_query_effect_index(input);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).expect("serialize query result")
        );
    } else {
        println!(
            "forge_core_effect_index_query status={:?} scanned={} matched={} returned={} reasons={:?}",
            result.status,
            result.scanned_records,
            result.matched_records,
            result.returned_records,
            result.reasons
        );
    }
    if result.status == forge_core_store::EffectTargetMetadataIndexQueryStatus::Failed {
        return Err(ExitError::failed("effect index query failed"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn effect_index_usage_projects_command_surface_lines() {
        for command in [&COMMAND_REBUILD_EFFECT_INDEX, &COMMAND_QUERY_EFFECT_INDEX] {
            let usage = effect_index_usage(command);
            assert!(usage.starts_with("usage:\n"));
            for line in command.usage_lines {
                let projected = format!("  {}", line.trim_start());
                assert!(
                    usage.contains(&projected),
                    "effect-index usage for {:?} should include projected Command Surface line {projected:?}: {usage}",
                    command.name
                );
            }
        }
    }

    #[test]
    fn explicit_no_json_is_accepted_by_effect_index_help_paths() {
        run_rebuild_effect_index_command(&args(&["rebuild-effect-index", "--no-json", "--help"]))
            .expect("rebuild-effect-index accepts explicit --no-json");
        run_query_effect_index_command(&args(&["query-effect-index", "--no-json", "--help"]))
            .expect("query-effect-index accepts explicit --no-json");
    }

    #[test]
    fn rebuild_missing_value_reports_rebuild_usage() {
        let error = run_rebuild_effect_index_command(&args(&["rebuild-effect-index", "--root"]))
            .expect_err("missing root value must fail");
        assert!(
            error.message().contains("forge-core rebuild-effect-index"),
            "missing rebuild value should report rebuild-specific usage: {error}"
        );
        assert!(
            !error.message().contains("forge-core query-effect-index"),
            "rebuild usage must not include sibling query usage: {error}"
        );
    }

    #[test]
    fn query_invalid_value_reports_query_usage() {
        let error = run_query_effect_index_command(&args(&[
            "query-effect-index",
            "--target-kind",
            "not-a-target-kind",
        ]))
        .expect_err("invalid target kind must fail");
        assert!(
            error.message().contains("forge-core query-effect-index"),
            "invalid query value should report query-specific usage: {error}"
        );
        assert!(
            !error.message().contains("forge-core rebuild-effect-index"),
            "query usage must not include sibling rebuild usage: {error}"
        );
    }
}
