//! `forge-core-cli` — Binary entrypoint crate for the Forge Method core.
//!
//! Historically a single 7400-line `lib.rs` god-file, this crate now contains
//! only the presentation-layer concerns (argv parsing in `main.rs`, the
//! `validate` / `execute_operation` / `effect_index` host surfaces, and the
//! host-adapter manifest + projection builders that the CLI owns). All
//! cryptographic verification primitives, host-adapter verification
//! entrypoints, and host-adapter data types have been moved to the
//! [`forge_core_crypto`](::forge_core_crypto) crate, which this crate
//! re-exports transitively at the crate root so existing call sites in
//! `main.rs`, `tests/validate.rs`, and `forge-contract-validator` keep
//! resolving `forge_core_cli::run_host_adapter_*_verification` and
//! `forge_core_cli::HostAdapter*` unchanged.

pub mod autonomy_cmd;
pub mod claim;
pub mod contract_cmd;
pub mod coordination;
pub(crate) mod effect_index;
pub mod eval_cmd;
pub(crate) mod execute_operation;
pub mod graph_cmd;
pub mod guide;
pub(crate) mod host_adapter_manifest;
pub(crate) mod host_adapter_projection;
// `host_command` here (the CLI's manifest/projection-side helpers) shadows
// the same-named module re-exported from `forge_core_crypto` (which contains
// only the two admission predicates `source_ref_is_immutable` and
// `version_like`). The CLI's own `host_command` already defines those two
// predicates locally for its projection/admission callers, so the shadow is
// intentional and safe.
#[allow(hidden_glob_reexports)]
pub(crate) mod host_command;
pub mod io_util;
pub mod isolation;
pub mod m1_cmd;
pub mod project_cmd;
pub mod telemetry_cmd;
pub(crate) mod validate;

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

// Re-export everything from `forge_core_crypto` at the crate root so the
// historical API surface (`forge_core_cli::run_host_adapter_*_verification`,
// `forge_core_cli::HostAdapter*` types, `forge_core_cli::hex_sha256`,
// `forge_core_cli::read_required_file`, ...) keeps resolving after the
// cryptographic primitives moved into their own crate.
//
// The modules that stay inside `forge-core-cli`
// (`host_adapter_manifest`, `host_adapter_projection`, `execute_operation`)
// reference crypto types/helpers via `crate::HostAdapter*` and
// `crate::hex_sha256` paths; the wildcard re-export below makes those paths
// resolve.
#[allow(clippy::wildcard_imports)]
pub use forge_core_crypto::*;
