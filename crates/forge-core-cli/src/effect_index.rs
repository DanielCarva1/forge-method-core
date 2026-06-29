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
pub fn run_query_effect_index_context(
    input: QueryEffectIndexInput,
) -> EffectMetadataContextBuildResult {
    let context_options = input.context_options.clone();
    let query_result = run_query_effect_index(input);
    build_effect_metadata_context(&query_result, &context_options)
}
