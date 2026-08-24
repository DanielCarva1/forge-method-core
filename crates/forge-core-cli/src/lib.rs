//! `forge-core-cli` — Binary entrypoint crate for the Forge Method core.
//!
//! Historically a single 7400-line `lib.rs` god-file, this crate now contains
//! only the presentation-layer concerns (argv parsing in `main.rs`, the
//! `validate` / `execute_operation` / `effect_index` host surfaces, and the
//! host-adapter manifest + projection builders that the CLI owns). All
//! cryptographic verification primitives, host-adapter verification
//! entrypoints, and host-adapter data types live in the
//! [`forge_core_crypto`] crate. This crate retains the historical public
//! re-exports so downstream `forge_core_cli::...` imports remain compatible.

// The CLI dispatchers (`*_cmd.rs`, `claim.rs`, `isolation.rs`) are long by
// construction: each one parses a flat argv list, assembles a typed input,
// invokes the engine, and emits an envelope. Splitting them to fit under
// `clippy::too_many_lines` would obscure the linear argument-parsing flow.
// Envelope emitters take `CliEnvelope<T>` by value intentionally: ownership
// moves into the emitter so the caller never has to keep the envelope alive.
#![allow(clippy::too_many_lines)]
#![allow(clippy::needless_pass_by_value)]

pub mod assurance_cmd;
pub mod autonomy_cmd;
pub mod backup_cmd;
pub mod claim;
pub mod cli_error;
pub mod cli_util;
pub mod command_registry;
pub mod contract_cmd;
pub mod coordination;
pub mod cost_cmd;
mod credential_custody;
pub mod domain_pack_cmd;
pub mod domain_pack_learning_cmd;
pub mod effect_index;
mod effect_vocabulary;
pub mod eval_cmd;
pub mod eval_harness_cmd;
pub mod eval_harness_trace;
pub mod execute_operation;
pub mod governance_cmd;
pub mod graph_cmd;
pub mod guide;
pub(crate) mod host_adapter_manifest;
pub mod host_adapter_policy_cmd;
pub(crate) mod host_adapter_projection;
pub mod host_adapter_verify_cmd;
pub(crate) mod host_command;
pub mod host_conformance_cmd;
pub mod host_support_matrix_cmd;
pub mod io_util;
pub mod isolation;
pub mod m1_cmd;
pub mod mcp_cmd;
pub(crate) mod mcp_credential_cmd;
pub(crate) mod mcp_readiness_cmd;
pub(crate) mod mcp_replay_anchor_cmd;
pub(crate) mod mcp_snapshot_cmd;
pub mod memory_cmd;
pub mod preflight_cmd;
pub mod product_lifecycle_cmd;
pub mod project_cmd;
pub mod project_profile;
pub mod project_reinitialize_cmd;
pub mod research_cmd;
pub mod restore_cmd;
pub mod risk_audit_cmd;
pub mod risk_audit_trace;
pub mod start_cmd;
pub mod telemetry_cmd;
pub mod tracing_init;
pub mod validate;
mod workflow_action_cmd;
mod workflow_autonomy_cmd;
mod workflow_broker_cmd;
pub mod workflow_cmd;
mod workflow_credential_cmd;
mod workflow_episode_cmd;
mod workflow_evidence_cmd;
mod workflow_intent_cmd;
mod workflow_promotion_cmd;

// Re-export the execute-operation API at the crate root so that the binary
// entrypoint (`main.rs`) and integration tests (`tests/validate.rs`) keep
// importing `ExecuteOperationInput`, `PayloadFileSpec`, `PayloadLoadPolicy`,
// `run_execute_operation` directly from `forge_core_cli`.
pub use execute_operation::{
    run_execute_operation, ExecuteOperationContractPathKind, ExecuteOperationError,
    ExecuteOperationInput, PayloadFileSpec, PayloadLoadPolicy,
};
// Re-export the effect-index API at the crate root so `main.rs` and
// `tests/validate.rs` keep importing the input structs and entrypoints
// directly from `forge_core_cli`.
pub use effect_index::{
    run_query_effect_index, run_query_effect_index_context, run_rebuild_effect_index,
    QueryEffectIndexInput, RebuildEffectIndexInput,
};
// Re-export the public validate API at the crate root so `main.rs`,
// `tests/validate.rs`, and `forge-contract-validator` keep importing
// `run_validate`, `ValidateSummary`, `ValidateCheck`, `ValidateDiagnostic`,
// and `ValidationStatus` directly from `forge_core_cli`.
pub use forge_core_validate::{load_authorized_markdown, MarkdownFileLoadError};
pub use validate::{
    run_validate, ValidateCheck, ValidateDiagnostic, ValidateSummary, ValidationStatus,
};
// Re-export the host adapter manifest builder at the crate root so
// `main.rs`, `tests/validate.rs`, and the projection/policy/admission
// builders keep calling `run_host_adapter_manifest()` directly from
// `forge_core_cli`.
pub use host_adapter_manifest::run_host_adapter_manifest;
// Re-export the host adapter projection/policy/admission builders at the
// crate root so `main.rs`, `tests/validate.rs`, and
// `forge-contract-validator` keep importing `run_host_adapter_projection`,
// `run_host_adapter_process_security_policy`,
// `run_host_adapter_invocation_admission`,
// `run_host_adapter_distribution_policy`, and
// `run_host_adapter_distribution_admission` directly from
// `forge_core_cli`.
pub use host_adapter_projection::{
    run_host_adapter_distribution_admission, run_host_adapter_distribution_policy,
    run_host_adapter_invocation_admission, run_host_adapter_process_security_policy,
    run_host_adapter_projection,
};

// Preserve the public facade that existed before the crypto implementation
// moved into its own crate. Keep the modules and crate-root items explicit so
// the CLI-private `host_command` builder cannot accidentally hide or widen the
// compatibility surface.
pub use forge_core_crypto::host_command::{source_ref_is_immutable, version_like};
pub use forge_core_crypto::{
    file_io, hashing, host_adapter_types, host_adapter_verification, ocsp, rekor, sigstore,
    slsa_transparency, tuf,
};
pub use forge_core_crypto::{
    hex_bytes, hex_sha256, normalize_sha256_display, read_public_key_file, read_required_file,
    read_signature_file, valid_sha256_digest,
};
pub use forge_core_crypto::{host_adapter_types::*, host_adapter_verification::*};
