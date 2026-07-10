//! `forge-core-protocol-mcp` — Secure MCP (Model Context Protocol) adapter
//! for the Forge Method core (F08; ADR-0006).
//!
//! This crate is a **protocol adapter**, not a second implementation of the
//! engine. The inviolable rule (ADR-0006 Decision 1): an adapter is never the
//! source of truth and never mutates the store directly. Every mutation must
//! flow through a host-neutral kernel boundary and an `OperationContract`.
//!
//! # Architecture (ADR-0006)
//!
//! - **[`server`]** — the MCP server over stdio JSON-RPC (`rmcp`). Read-only
//!   tools map `(tool_name, arguments)` to a pinned `forge-core` subprocess and
//!   return its `CliEnvelope`. Mutating calls never use that subprocess path.
//! - **[`allowlist`]** — the capability surface (`mcp-allowlist.yaml`). A tool
//!   absent from the Allowlist is invisible to `tools/list` and rejected on
//!   `tools/call` — fail-closed (ADR-0006 Decision 3).
//! - **[`attestation`]** — Tool-Call Attestation primitives (ADR-0006
//!   Decision 4): detached ed25519 signatures over canonicalized tool-call
//!   intents. Signature-only verification is origin proof, not authorization.
//! - **[`principal_registry`]** — the operator-owned identity and authority
//!   compatibility projection over the host-neutral `forge-core-authority`
//!   crate. It selects the verification key, checks audience/tool/freshness,
//!   and constructs opaque execution authorization.
//! - **[`mutation_executor`]** provides the typed in-process handoff for a
//!   verified `execute-operation` call. It structurally excludes
//!   caller-selected root, durability, payload-scope, and transaction-id
//!   controls. P4b.2c consumes the seam only in a dormant provenance-bound commit
//!   and recovery path; MCP stdio mutation remains blocked pending explicit
//!   deployment policy and trusted adapter wiring.
//! - **[`deployment_policy`]** validates the operator's closed, typed
//!   deployment posture. Trusted mutation policies remain dormant until the
//!   loader and startup-reconciliation checkpoints are complete.
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
pub mod deployment_policy;
pub mod error;
pub mod mutation_executor;
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
pub use deployment_policy::{
    EffectScopePolicy, MaterialLoadingPolicy, McpDeploymentActivationState, McpDeploymentMode,
    McpDeploymentPolicy, McpDeploymentPolicyDocument, McpDeploymentPolicyError,
    McpDeploymentPolicyIssue, McpDeploymentPolicyIssueCode, PublicMutationPolicy,
    RootBindingPolicy, SnapshotLoadingPolicy, StartupReconciliationPolicy,
    ValidatedMcpDeploymentPolicy, MCP_DEPLOYMENT_POLICY_SCHEMA_VERSION,
    MCP_EXECUTION_COMMIT_PROTOCOL,
};
pub use error::{McpAdapterError, ServerRunError};
pub use mutation_executor::{
    McpExecutionRequest, McpMutationExecutionError, McpMutationExecutionResult,
    McpMutationExecutionStatus, McpMutationExecutor, McpMutationPayloadBinding,
    McpMutationRequestError, VerifiedMcpExecutionCall, MCP_EXECUTE_OPERATION_TOOL,
};
pub use principal_registry::{
    AuthorizedPrincipal, AuthorizedPrincipalAudit, AuthorizedPrincipalRegistry,
    PrincipalAuthorizationError, PrincipalCredentialStatus, PrincipalRegistryContract,
    PrincipalRegistryDocument, PrincipalRegistryEntry, PrincipalRegistryError,
    PrincipalRegistryIssue, PrincipalRegistryIssueCode, VerifiedExecutionAuthorization,
    VerifiedExecutionAuthorizationAudit, DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
    DEFAULT_MAX_FUTURE_SKEW_SECONDS, PRINCIPAL_REGISTRY_SCHEMA_VERSION,
};
pub use server::{ForgeMcpServer, McpServerConfig};
