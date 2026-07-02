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
//! - **[`server`]** — the MCP server over stdio JSON-RPC (`rmcp`). Exposes a
//!   projection of `forge_core_cli::command_registry::COMMANDS` as MCP tools.
//!   Each tool is a pass-through: map `(tool_name, arguments)` → argv
//!   `&[String]` → invoke the matching `CommandSpec::handler` → return the
//!   `CliEnvelope` JSON as the tool result. No domain logic lives here.
//! - **[`allowlist`]** — the capability surface (`mcp-allowlist.yaml`). A tool
//!   absent from the Allowlist is invisible to `tools/list` and rejected on
//!   `tools/call` — fail-closed (ADR-0006 Decision 3).
//! - **[`attestation`]** — Tool-Call Attestation verification (ADR-0006
//!   Decision 4): detached ed25519 signature over the canonicalized tool-call
//!   intent, verified against a configured authorized key. Required for mutate
//!   tools, optional for read-only under the default policy.
//! - **[`error`]** — hand-rolled error enums (project convention: no
//!   `anyhow`/`thiserror`, no `Result<_, String>`).
//!
//! The MCP server is a PEP (Policy Enforcement Point) per ADR-0003; the
//! kernel remains the only PDP (Policy Decision Point) for mutation.
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
pub mod server;

pub use allowlist::{
    AllowedTool, Allowlist, AllowlistError, AllowlistPolicy, DEFAULT_MUTATE_TOOLS,
    DEFAULT_READONLY_TOOLS,
};
pub use attestation::{
    AttestationError, AttestationInput, AttestationPolicy, AttestationVerifier, CanonicalIntent,
};
pub use error::{McpAdapterError, ServerRunError};
pub use server::{ForgeMcpServer, McpServerConfig};
