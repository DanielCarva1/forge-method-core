//! Allowlist — the capability surface of an MCP server instance (ADR-0006
//! Decision 3).
//!
//! The Allowlist declares the explicit, named set of `MCPTools` a given server
//! instance is permitted to expose. A tool absent from the Allowlist is
//! invisible to `tools/list` and rejected on `tools/call` — fail-closed. It
//! is data, not code: a YAML file (`mcp-allowlist.yaml`) that mirrors the
//! risk-audit contract model.
//!
//! The Allowlist separates "Forge can do X" (every entry in
//! `command_registry::COMMANDS`) from "this MCP client may ask Forge to do X"
//! (the Allowlist subset). Declaring an empty or read-only Allowlist is the
//! safe default.
//!
//! # F08.2 scope
//!
//! This module defines the types and the canonical tool-name defaults. Full
//! YAML parsing + typed diagnostics land in F08.4 (validator in
//! `forge-core-validate`). For F08.2/F08.3 the server is constructed from an
//! in-memory `Allowlist` so the adapter compiles and tools can be exercised
//! before the config layer is wired.

use std::fmt;

/// The kind of an [`AllowedTool`]: read-only (safe to expose without an
/// `OperationContract`) or mutate (gated by the `MutateGate` + Tool-Call
/// Attestation, ADR-0006 Decisions 2 & 4).
///
/// This is the policy classification that drives the `MutateGate`. It is
/// declared per-tool-name, not derived from the command, so the Allowlist is
/// the single source for "is this tool a mutation?".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllowlistPolicy {
    /// Read-only: no store mutation. Tool-Call Attestation optional under the
    /// default policy (ADR-0006 Decision 4).
    ReadOnly,
    /// Mutate: writes to the store. Requires an `OperationContract` AND a
    /// valid Tool-Call Attestation.
    Mutate,
}

impl AllowlistPolicy {
    #[must_use]
    pub fn is_mutate(self) -> bool {
        matches!(self, Self::Mutate)
    }
}

impl fmt::Display for AllowlistPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadOnly => f.write_str("read-only"),
            Self::Mutate => f.write_str("mutate"),
        }
    }
}

/// One row in the Allowlist: a tool name (matching the MCP tool name AND the
/// underlying `command_registry::COMMANDS` entry name) plus its policy
/// classification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AllowedTool {
    /// The MCP tool name. Matches a `CommandSpec::name` in
    /// `command_registry::COMMANDS` (e.g. `"preview"`, `"execute-operation"`).
    pub name: String,
    /// Whether this tool mutates the store.
    pub policy: AllowlistPolicy,
}

/// The set of `MCPTools` a server instance exposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allowlist {
    tools: Vec<AllowedTool>,
}

/// Failures loading/building an Allowlist. F08.4 will expand this with typed
/// diagnostics in `forge-core-validate`; F08.2 keeps a coarse enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowlistError {
    /// A tool name in the Allowlist does not match any registered command.
    UnknownTool(String),
    /// Duplicate tool name.
    Duplicate(String),
    /// The allowlist file could not be read or parsed.
    Parse(String),
}

impl fmt::Display for AllowlistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTool(t) => write!(f, "allowlist references unknown tool: {t}"),
            Self::Duplicate(t) => write!(f, "duplicate tool in allowlist: {t}"),
            Self::Parse(m) => write!(f, "allowlist parse error: {m}"),
        }
    }
}

impl std::error::Error for AllowlistError {}

/// Canonical read-only tool names (projection of `command_registry::COMMANDS`
/// that performs no store mutation). See ADR-0006 / CONTEXT.md "`MCPTool`".
///
/// `explain` is the "trace" surface; `query-effect-index` is the read side of
/// the effect index. `memory list` is the read verb of the memory PEP.
pub const DEFAULT_READONLY_TOOLS: &[&str] = &[
    "preview",
    "ready",
    "graph",
    "explain",
    "memory",
    "query-effect-index",
];

/// Canonical mutate tool names (require `OperationContract` + Tool-Call
/// Attestation at the `MutateGate`). See ADR-0006 Decisions 2 & 4.
pub const DEFAULT_MUTATE_TOOLS: &[&str] = &["execute-operation", "claim"];

impl Allowlist {
    /// Build an Allowlist from an explicit list of tools. Deduplicates and
    /// preserves insertion order.
    ///
    /// # Errors
    ///
    /// Returns [`AllowlistError::Duplicate`] if the same tool name appears
    /// twice.
    pub fn from_tools(tools: Vec<AllowedTool>) -> Result<Self, AllowlistError> {
        let mut seen = std::collections::HashSet::new();
        for t in &tools {
            if !seen.insert(t.name.clone()) {
                return Err(AllowlistError::Duplicate(t.name.clone()));
            }
        }
        Ok(Self { tools })
    }

    /// The default read-only Allowlist: every tool in
    /// [`DEFAULT_READONLY_TOOLS`] as `ReadOnly`. Safe to expose without an
    /// `OperationContract`.
    #[must_use]
    pub fn default_read_only() -> Self {
        let tools = DEFAULT_READONLY_TOOLS
            .iter()
            .map(|n| AllowedTool {
                name: (*n).to_string(),
                policy: AllowlistPolicy::ReadOnly,
            })
            .collect();
        // No duplicates in the const array by construction.
        Self { tools }
    }

    /// The default Allowlist exposing both read-only and mutate tools. Used
    /// when the operator opts in to full surface; mutate tools remain gated
    /// by the `MutateGate` + attestation at call time.
    #[must_use]
    pub fn default_with_mutate() -> Self {
        let mut tools: Vec<AllowedTool> = DEFAULT_READONLY_TOOLS
            .iter()
            .map(|n| AllowedTool {
                name: (*n).to_string(),
                policy: AllowlistPolicy::ReadOnly,
            })
            .collect();
        tools.extend(DEFAULT_MUTATE_TOOLS.iter().map(|n| AllowedTool {
            name: (*n).to_string(),
            policy: AllowlistPolicy::Mutate,
        }));
        Self { tools }
    }

    /// Look up a tool by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&AllowedTool> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// Iterate over the allowed tools.
    pub fn iter(&self) -> impl Iterator<Item = &AllowedTool> {
        self.tools.iter()
    }

    /// Number of allowed tools.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the Allowlist is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_read_only_has_no_mutate() {
        let al = Allowlist::default_read_only();
        assert!(al.iter().all(|t| t.policy == AllowlistPolicy::ReadOnly));
        assert!(al.get("preview").is_some());
        assert!(al.get("execute-operation").is_none());
    }

    #[test]
    fn default_with_mutate_includes_both_classes() {
        let al = Allowlist::default_with_mutate();
        assert!(al.get("preview").is_some_and(|t| !t.policy.is_mutate()));
        assert!(al
            .get("execute-operation")
            .is_some_and(|t| t.policy.is_mutate()));
    }

    #[test]
    fn from_tools_rejects_duplicate() {
        let tools = vec![
            AllowedTool {
                name: "preview".into(),
                policy: AllowlistPolicy::ReadOnly,
            },
            AllowedTool {
                name: "preview".into(),
                policy: AllowlistPolicy::ReadOnly,
            },
        ];
        let err = Allowlist::from_tools(tools).unwrap_err();
        assert_eq!(err, AllowlistError::Duplicate("preview".into()));
    }

    #[test]
    fn unknown_absent_tool_is_none_not_error() {
        let al = Allowlist::default_read_only();
        // Absent = None; fail-closed at the server layer checks `is_none()`.
        assert!(al.get("definitely-not-a-tool").is_none());
    }
}
