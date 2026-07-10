//! Hand-rolled error enums for the MCP adapter (project convention: no
//! `anyhow`/`thiserror`; no `Result<_, String>`).
//!
//! All enums derive `Debug, Clone, PartialEq, Eq` and store a lossy `String`
//! for the source/message when needed (the legacy pattern in this workspace).

use std::fmt;

/// Top-level adapter error: a failure in the MCP server that is not a
/// domain-specific allowlist/attestation error (those have their own enums
/// in their modules and are converted at boundaries).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpAdapterError {
    /// A tool-call referenced a tool name that is not registered.
    UnknownTool(String),
    /// A tool-call was denied by the Allowlist (not listed).
    DeniedByAllowlist { tool: String, reason: String },
    /// A mutate tool-call was denied at the `MutateGate` (no valid
    /// `OperationContract`).
    DeniedByMutateGate { tool: String, reason: String },
    /// A tool-call was denied because Tool-Call Attestation was required and
    /// missing or invalid.
    DeniedByAttestation { tool: String, reason: String },
    /// The underlying CLI command handler returned a non-zero exit
    /// (rejection). The captured `CliEnvelope` JSON is carried so the caller
    /// can surface the structured self-correction data.
    CommandRejected {
        tool: String,
        exit_reason: String,
        envelope_json: String,
    },
    /// Argument mapping failed (could not serialize/deserialize the MCP
    /// arguments into a CLI argv).
    ArgumentMapping(String),
    /// A trusted in-process mutation executor rejected or failed a verified
    /// call. The public stdio path does not produce this in P4b.2a.
    MutationExecution(String),
    /// Configuration error (e.g. allowlist file unreadable, attestation key
    /// unparsable).
    Config(String),
}

impl fmt::Display for McpAdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTool(t) => write!(f, "unknown MCP tool: {t}"),
            Self::DeniedByAllowlist { tool, reason } => {
                write!(f, "tool {tool} denied by allowlist: {reason}")
            }
            Self::DeniedByMutateGate { tool, reason } => {
                write!(f, "mutate tool {tool} denied at MutateGate: {reason}")
            }
            Self::DeniedByAttestation { tool, reason } => {
                write!(f, "tool {tool} denied: attestation {reason}")
            }
            Self::CommandRejected {
                tool,
                exit_reason,
                envelope_json,
            } => {
                write!(
                    f,
                    "MCP tool {tool} rejected (exit_reason={exit_reason}): {envelope_json}"
                )
            }
            Self::ArgumentMapping(m) => write!(f, "argument mapping failed: {m}"),
            Self::MutationExecution(m) => write!(f, "in-process mutation execution failed: {m}"),
            Self::Config(m) => write!(f, "MCP adapter config error: {m}"),
        }
    }
}

impl std::error::Error for McpAdapterError {}

/// Failure to run the stdio MCP server loop itself (transport-level, not
/// domain-level). Domain-level tool-call failures are [`McpAdapterError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerRunError {
    /// The configured capability surface violates the process-security policy.
    Config(String),
    /// The `rmcp` transport returned an IO or protocol error.
    Transport(String),
    /// The tokio runtime could not be built or driven.
    Runtime(String),
}

impl fmt::Display for ServerRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(m) => write!(f, "MCP server config error: {m}"),
            Self::Transport(m) => write!(f, "MCP transport error: {m}"),
            Self::Runtime(m) => write!(f, "tokio runtime error: {m}"),
        }
    }
}

impl std::error::Error for ServerRunError {}
