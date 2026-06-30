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
use crate::cli_util::{
    next_arg_or_err, next_path_or_err, parse_metadata_adapter_trigger_or_err,
    parse_metadata_consumer_use_or_err, parse_runtime_kind_or_err, parse_target_kind_or_err,
    parse_usize_or_err, resolve_stateful_roots_or_err, usage,
};
use forge_core_contracts::{tool_effect::EffectTargetKind, StableId};
use forge_core_store::{
    build_effect_metadata_context, query_effect_target_metadata_index,
    rebuild_effect_target_metadata_index_with_lock, EffectMetadataConsumerUse,
    EffectMetadataContextBuildOptions, EffectMetadataContextBuildResult,
    EffectTargetMetadataIndexQuery, EffectTargetMetadataIndexQueryResult,
    EffectTargetMetadataIndexRebuildResult,
};

/// Inputs for [`run_rebuild_effect_index`].
#[derive(Debug, Clone)]
pub struct RebuildEffectIndexInput {
    pub root: PathBuf,
    pub wal_relative_path: String,
    pub index_relative_path: String,
    pub lock_relative_path: String,
    pub recorded_at: Option<String>,
}

impl Default for RebuildEffectIndexInput {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            wal_relative_path: ".forge-method/wal/effects.ndjson".to_string(),
            index_relative_path: ".forge-method/index/effect-targets.ndjson".to_string(),
            lock_relative_path: ".forge-method/locks/effects.lock".to_string(),
            recorded_at: None,
        }
    }
}

/// Rebuild the effect-target metadata index on disk under a lock.
#[must_use]
pub fn run_rebuild_effect_index(
    input: RebuildEffectIndexInput,
) -> EffectTargetMetadataIndexRebuildResult {
    rebuild_effect_target_metadata_index_with_lock(
        &input.root,
        &input.wal_relative_path,
        &input.index_relative_path,
        &input.lock_relative_path,
        input.recorded_at.as_deref(),
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
pub fn run_rebuild_effect_index_command(args: &[String]) -> Result<(), ExitError> {
    let mut input = RebuildEffectIndexInput::default();
    let mut allow_bootstrap_core = false;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                input.root = next_path_or_err(args, index)?;
            }
            "--wal" => {
                index += 1;
                input.wal_relative_path = next_arg_or_err(args, index)?.to_string();
            }
            "--index" => {
                index += 1;
                input.index_relative_path = next_arg_or_err(args, index)?.to_string();
            }
            "--lock" => {
                index += 1;
                input.lock_relative_path = next_arg_or_err(args, index)?.to_string();
            }
            "--allow-bootstrap-core" => {
                allow_bootstrap_core = true;
            }
            "--recorded-at" => {
                index += 1;
                input.recorded_at = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(usage()));
            }
        }
        index += 1;
    }

    let roots =
        resolve_stateful_roots_or_err("rebuild-effect-index", &input.root, allow_bootstrap_core)?;
    input.root = roots.effect_store_root;

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

pub fn run_query_effect_index_command(args: &[String]) -> Result<(), ExitError> {
    let mut input = QueryEffectIndexInput::default();
    let mut allow_bootstrap_core = false;
    let mut json = false;
    let mut context = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                input.root = next_path_or_err(args, index)?;
            }
            "--index" => {
                index += 1;
                input.index_relative_path = next_arg_or_err(args, index)?.to_string();
            }
            "--logical-ref" => {
                index += 1;
                input.logical_ref = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--effect-id" => {
                index += 1;
                input.effect_id = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--operation-id" => {
                index += 1;
                input.operation_id = Some(next_arg_or_err(args, index)?.to_string());
            }
            "--target-kind" => {
                index += 1;
                input.target_kind = Some(parse_target_kind_or_err(next_arg_or_err(args, index)?)?);
            }
            "--consumer-use" => {
                index += 1;
                input.consumer_use =
                    parse_metadata_consumer_use_or_err(next_arg_or_err(args, index)?)?;
            }
            "--context" => context = true,
            "--allow-bootstrap-core" => {
                allow_bootstrap_core = true;
            }
            "--max-context-groups" => {
                index += 1;
                input.context_options.max_groups =
                    parse_usize_or_err(next_arg_or_err(args, index)?)?;
            }
            "--adapter-kind" => {
                index += 1;
                input.context_options.adapter_kind =
                    Some(parse_runtime_kind_or_err(next_arg_or_err(args, index)?)?);
            }
            "--adapter-trigger" => {
                index += 1;
                input.context_options.adapter_trigger =
                    parse_metadata_adapter_trigger_or_err(next_arg_or_err(args, index)?)?;
            }
            "--latest" => input.latest_per_target = true,
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(usage()));
            }
        }
        index += 1;
    }

    let roots =
        resolve_stateful_roots_or_err("query-effect-index", &input.root, allow_bootstrap_core)?;
    input.root = roots.effect_store_root;
    emit_query_effect_index_result(input, context, json)
}

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
