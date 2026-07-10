//! `forge-core-protocol-mcp` — Secure MCP (Model Context Protocol) adapter
//! for the Forge Method core (F08; ADR-0006).
//!
//! This crate is a **protocol adapter**, not a second implementation of the
//! engine. The inviolable rule (ADR-0006 Decision 1): an adapter is never the
//! source of truth and never mutates the store directly. Every mutation flows
//! through the kernel (`forge-core-cli` command handlers) and an
//! `OperationContract`.
//!
//! # Architecture (ADR-0006)
//!
//! - **[`server`]** ? the MCP server over stdio JSON-RPC (`rmcp`). Exposes a
//!   projection of `forge_core_command_surface::COMMANDS` as MCP tools. Each
//!   tool is a pass-through: map `(tool_name, arguments)` to argv, invoke the
//!   matching `forge-core` subprocess command, and return the `CliEnvelope`
//!   JSON as the tool result. No domain logic lives here.
//! - **[`allowlist`]** — the capability surface (`mcp-allowlist.yaml`). A tool
//!   absent from the Allowlist is invisible to `tools/list` and rejected on
//!   `tools/call` — fail-closed (ADR-0006 Decision 3).
//! - **[`attestation`]** — Tool-Call Attestation primitives (ADR-0006
//!   Decision 4): detached ed25519 signatures over canonicalized tool-call
//!   intents. Signature-only verification is origin proof, not authorization.
//! - **[`principal_registry`]** — the operator-owned identity and authority
//!   binding prepared for mutating calls. It selects the verification key,
//!   checks audience/tool/freshness, and rejects revoked or caller-selected
//!   identity. MCP stdio mutation remains blocked until the replay/kernel
//!   boundary can consume this typed result without discarding it.
//! - **[`error`]** — hand-rolled error enums (project convention: no
//!   `anyhow`/`thiserror`, no `Result<_, String>`).
//!
//! The MCP server is an identity/capability PEP (Policy Enforcement Point) per
//! ADR-0024. It does not replace kernel Execution Admission: the kernel remains
//! the only mutation PDP (Policy Decision Point).
//!
//! # Deletion test
//!
//! Removing this crate costs callers programmatic access over stdio JSON-RPC,
//! but costs no functionality — the underlying commands stay available via
//! the `forge-core` CLI. The adapter earns its keep by concentrating the
//! `rmcp`/tokio-stdio coupling in one seam.

pub mod allowlist;
pub mod attestation;
pub mod error;
pub mod principal_registry;
pub mod server;

pub use allowlist::{
    default_mutate_tool_names, default_read_only_tool_names, AllowedTool, Allowlist,
    AllowlistError, AllowlistPolicy,
};
pub use attestation::{
    AttestationError, AttestationGateOutcome, AttestationInput, AttestationPolicy,
    AttestationVerifier, CanonicalIntent,
};
pub use error::{McpAdapterError, ServerRunError};
pub use principal_registry::{
    AuthorizedPrincipal, AuthorizedPrincipalRegistry, PrincipalAuthorizationError,
    PrincipalCredentialStatus, PrincipalRegistryContract, PrincipalRegistryDocument,
    PrincipalRegistryEntry, PrincipalRegistryError, PrincipalRegistryIssue,
    PrincipalRegistryIssueCode, DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
    DEFAULT_MAX_FUTURE_SKEW_SECONDS, PRINCIPAL_REGISTRY_SCHEMA_VERSION,
};
pub use server::{ForgeMcpServer, McpServerConfig};
