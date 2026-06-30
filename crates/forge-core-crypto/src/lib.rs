//! `forge-core-crypto` — Cryptographic verification primitives for the
//! Forge Method core.
//!
//! This crate groups together all of the cryptographic verification
//! logic that the Forge Method core uses to attest artifact provenance,
//! signatures, sigstore bundles, DSSE payloads, fulcio certificate
//! identity, certificate transparency SCTs, CRL/OCSP revocation status,
//! TUF trusted root freshness, and rekor log inclusion.
//!
//! ## Layout
//!
//! - [`hashing`] — canonical SHA-256 digests and hex helpers.
//! - [`rekor`] — rekor log entry parsing and inclusion proof verification.
//! - [`ocsp`] — OCSP response decoding, freshness and signature verification.
//! - [`sigstore`] — sigstore trust policy, fulcio chain, bundle/DSSE verify.
//! - [`slsa_transparency`] — SLSA statement and transparency log proofs.
//! - [`tuf`] — TUF metadata freshness role verification.
//! - [`file_io`] — shared file I/O helpers (`read_required_file`, etc.).
//! - [`host_command`] — admission safety predicates (`source_ref_is_immutable`,
//!   `version_like`).
//! - [`host_adapter_types`] — public types for host-adapter verification
//!   (inputs and statuses).
//! - [`host_adapter_verification`] — the `run_host_adapter_*_verification`
//!   entrypoints invoked by the `forge` CLI and integration tests.

pub mod file_io;
pub mod hashing;
pub mod host_adapter_types;
pub mod host_adapter_verification;
pub mod host_command;
pub mod ocsp;
pub mod rekor;
pub mod sigstore;
pub mod slsa_transparency;
pub mod tuf;

// Re-export the host-adapter verification entrypoints at the crate root so
// external callers (the `forge-core-cli`, `tests`, `forge-contract-validator`)
// can keep importing `forge_core_crypto::run_host_adapter_*_verification`
// directly, mirroring the historical `forge_core_cli::` API surface. The
// `#[allow(unused_imports)]` silences the false-positive "unused import"
// lint that fires because nothing inside this crate consumes the
// re-exported items via `crate::*` paths; they exist solely for downstream
// crates.
#[allow(unused_imports)]
pub use host_adapter_types::*;
#[allow(unused_imports)]
pub use host_adapter_verification::*;

// Re-export the small set of crypto helpers that historical callers in
// `forge-core-cli` (e.g. `execute_operation`, `host_adapter_projection`)
// used to import via `forge_core_cli::hex_sha256` /
// `forge_core_cli::valid_sha256_digest` / `forge_core_cli::read_required_file`.
// Keeping these at the crate root preserves those call sites without any
// rewrite.
pub use file_io::{read_public_key_file, read_required_file, read_signature_file};
pub use hashing::{hex_bytes, hex_sha256, normalize_sha256_display, valid_sha256_digest};
