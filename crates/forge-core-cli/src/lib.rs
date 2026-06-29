pub mod autonomy_cmd;
pub mod claim;
pub mod contract_cmd;
pub mod coordination;
pub(crate) mod crypto_hashing;
pub(crate) mod crypto_ocsp;
pub(crate) mod crypto_rekor;
pub(crate) mod crypto_sigstore;
pub(crate) mod crypto_slsa_transparency;
pub(crate) mod crypto_tuf;
pub(crate) mod effect_index;
pub mod eval_cmd;
pub(crate) mod execute_operation;
pub(crate) mod file_io;
pub mod graph_cmd;
pub mod guide;
pub(crate) mod host_adapter_manifest;
pub(crate) mod host_adapter_projection;
pub(crate) mod host_adapter_types;
pub(crate) mod host_adapter_verification;
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
// Re-export the two shared helpers still consumed via `crate::*` paths
// from peer modules after R1.HostAdapterVerification moved the
// host-adapter verification entrypoints (and with them all remaining
// callers of the crypto helpers) into `host_adapter_verification`:
//   - `crate::hex_sha256` is used by `execute_operation`
//   - `crate::read_required_file` is used by `crypto_tuf`
// All other previously re-exported helpers are now consumed only inside
// `host_adapter_verification`, which imports them directly from their
// respective modules (`crypto_sigstore`, `crypto_slsa_transparency`,
// `crypto_ocsp`, `crypto_tuf`, `file_io`, `crypto_hashing`,
// `host_command`).
pub(crate) use crypto_hashing::{hex_bytes, hex_sha256, normalize_sha256_display};
pub(crate) use file_io::read_required_file;
// Re-export the public validate API at the crate root so `main.rs`,
// `tests/validate.rs`, and `forge-contract-validator` keep importing
// `run_validate`, `ValidateSummary`, `ValidateCheck`, `ValidateDiagnostic`,
// and `ValidationStatus` directly from `forge_core_cli`.
pub use validate::{
    run_validate, ValidateCheck, ValidateDiagnostic, ValidateSummary, ValidationStatus,
};
// Re-export all host-adapter types at the crate root so `main.rs` and
// `tests/validate.rs` keep importing `HostAdapterManifest`, etc., directly
// from `forge_core_cli` after the types moved into `host_adapter_types`.
pub use host_adapter_types::*;
// Re-export the host adapter manifest builder at the crate root so
// `main.rs`, `tests/validate.rs`, and the projection/policy/admission
// builders still in `lib.rs` keep calling `run_host_adapter_manifest()`
// directly from `forge_core_cli` after the builder moved into the
// `host_adapter_manifest` module.
pub use host_adapter_manifest::run_host_adapter_manifest;
// Re-export the host adapter projection/policy/admission builders at the
// crate root so `main.rs`, `tests/validate.rs`, and
// `forge-contract-validator` keep importing `run_host_adapter_projection`,
// `run_host_adapter_process_security_policy`,
// `run_host_adapter_invocation_admission`,
// `run_host_adapter_distribution_policy`, and
// `run_host_adapter_distribution_admission` directly from
// `forge_core_cli` after the builders moved into the
// `host_adapter_projection` module.
pub use host_adapter_projection::{
    run_host_adapter_distribution_admission, run_host_adapter_distribution_policy,
    run_host_adapter_invocation_admission, run_host_adapter_process_security_policy,
    run_host_adapter_projection,
};

// Re-export all of the `run_host_adapter_*_verification` entrypoints at the
// crate root so `main.rs`, `tests/validate.rs`, and
// `forge-contract-validator` keep importing
// `forge_core_cli::run_host_adapter_*_verification` directly after the
// functions moved into the `host_adapter_verification` module.
pub use host_adapter_verification::*;
