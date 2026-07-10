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
//! `forge_core_command_surface::COMMANDS`) from "this MCP client may ask Forge
//! to do X" (the Allowlist subset). Declaring an empty or read-only Allowlist
//! is the safe default.
//!
//! # YAML format
//!
//! ```yaml
//! tools:
//!   - name: preview
//!     policy: read-only
//!   - name: execute-operation
//!     policy: mutate
//! ```
//!
//! `policy` is `read-only` (default) or `mutate`. The shared Command Surface is
//! the authority floor: a command whose `CommandAuthority` may mutate cannot be
//! admitted with `read-only` policy. Mutating entries are representable for
//! future hardened transports, but MCP stdio startup currently rejects any
//! such surface. Unknown names and unsafe policies are reported as typed
//! `Diagnostic`s without short-circuiting, so the operator sees every issue at
//! once (the project convention).

use std::fmt;

use forge_core_command_surface::{
    command_by_name, mcp_default_mutate_tool_names, mcp_default_read_only_tool_names,
};
use forge_core_validate::{Diagnostic, DiagnosticCode, ValidationReport};
use serde::{Deserialize, Serialize};

/// The kind of an [`AllowedTool`]: read-only (safe to expose without an
/// `OperationContract`) or mutate (gated by the `MutateGate` + Tool-Call
/// Attestation, ADR-0006 Decisions 2 & 4).
///
/// This is the policy classification that drives transport admission. The
/// Command Surface is still the authority floor: explicit YAML can make a
/// read-only command stricter by declaring `mutate`, but it cannot make a
/// mutating or mixed-authority command weaker by declaring `read-only`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllowlistPolicy {
    /// Read-only: no store mutation. Tool-Call Attestation optional under the
    /// default policy (ADR-0006 Decision 4).
    ReadOnly,
    /// Mutate: writes to the store. Representable in policy, but blocked on the
    /// stdio transport until replay and late kernel admission are complete.
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
/// underlying shared Command Surface entry name) plus its policy
/// classification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AllowedTool {
    /// The MCP tool name. Matches a `CommandSpec::name` in
    /// `forge_core_command_surface::COMMANDS` (e.g. `"preview"`,
    /// `"execute-operation"`).
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

/// Canonical read-only tool names exposed by default.
///
/// This is a projection of `forge_core_command_surface::COMMANDS`, not a
/// second hand-maintained list. Renaming a command or changing its MCP
/// visibility now changes the default adapter surface through the shared
/// Command Surface seam.
pub fn default_read_only_tool_names() -> impl Iterator<Item = &'static str> {
    mcp_default_read_only_tool_names()
}

/// Canonical mutate tool names exposed by the opt-in default mutate surface.
///
/// These require an `OperationContract` + Tool-Call Attestation at the
/// `MutateGate` (ADR-0006 Decisions 2 & 4). The names are projected from the
/// shared Command Surface seam.
pub fn default_mutate_tool_names() -> impl Iterator<Item = &'static str> {
    mcp_default_mutate_tool_names()
}

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
    /// the shared Command Surface's default read-only MCP projection as
    /// `ReadOnly`. Safe to expose without an `OperationContract`.
    #[must_use]
    pub fn default_read_only() -> Self {
        let tools = default_read_only_tool_names()
            .map(|name| AllowedTool {
                name: name.to_string(),
                policy: AllowlistPolicy::ReadOnly,
            })
            .collect();
        // No duplicates by construction; command-surface tests assert this.
        Self { tools }
    }

    /// The default Allowlist exposing both read-only and mutate tools. Used
    /// when the operator opts in to full surface; mutate tools remain gated
    /// by the `MutateGate` + attestation at call time.
    #[must_use]
    pub fn default_with_mutate() -> Self {
        let mut tools: Vec<AllowedTool> = default_read_only_tool_names()
            .map(|name| AllowedTool {
                name: name.to_string(),
                policy: AllowlistPolicy::ReadOnly,
            })
            .collect();
        tools.extend(default_mutate_tool_names().map(|name| AllowedTool {
            name: name.to_string(),
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

    /// Parse an Allowlist from a YAML string (`mcp-allowlist.yaml` content).
    ///
    /// Validation accumulates typed diagnostics (project convention: never
    /// short-circuit on the first problem). The returned `ValidationReport`
    /// lets the caller decide via `report.has_errors()`. An unknown tool name
    /// (one not matching any registered command) is an error; declaring a
    /// command whose `CommandAuthority` may mutate as `read-only` is an error;
    /// a duplicate is an error; an empty tools list is an error.
    ///
    /// `known_commands` is the set of registered command names from
    /// `forge_core_command_surface::COMMANDS` — the validator checks each
    /// Allowlist entry against it so a typo is caught at load, not at call
    /// time.
    ///
    /// # Errors
    ///
    /// Never returns `Err` — parse failures become a single
    /// `McpAllowlistYamlParseFailed` diagnostic and an empty Allowlist, so the
    /// caller always gets a (possibly empty) `Allowlist` plus a report.
    #[must_use]
    pub fn from_yaml_str(yaml: &str, known_commands: &[&str]) -> (Self, ValidationReport) {
        let mut report = ValidationReport::default();
        // Parse the YAML envelope. If parsing fails, return an empty Allowlist
        // + a single error diagnostic (the whole document is unusable).
        let parsed: AllowlistYaml = match yaml_serde::from_str(yaml) {
            Ok(doc) => doc,
            Err(e) => {
                report.push(Diagnostic::error(
                    DiagnosticCode::McpAllowlistYamlParseFailed,
                    "mcp-allowlist.yaml",
                    format!("YAML parse failed: {e}"),
                ));
                return (Self { tools: Vec::new() }, report);
            }
        };

        // Empty tools list is an error (fail-closed: no implicit exposure).
        if parsed.tools.is_empty() {
            report.push(Diagnostic::error(
                DiagnosticCode::McpAllowlistEmpty,
                "mcp-allowlist.yaml",
                "allowlist must declare at least one tool (empty = fail-closed)",
            ));
            return (Self { tools: Vec::new() }, report);
        }

        let known: std::collections::HashSet<&str> = known_commands.iter().copied().collect();
        let mut seen = std::collections::HashSet::new();
        let mut tools = Vec::with_capacity(parsed.tools.len());
        for (i, entry) in parsed.tools.iter().enumerate() {
            let path = format!("mcp-allowlist.yaml#tools[{i}]");
            if !known.contains(entry.name.as_str()) {
                report.push(Diagnostic::error(
                    DiagnosticCode::McpAllowlistUnknownTool,
                    path,
                    format!(
                        "tool {:?} is not a registered forge-core command",
                        entry.name
                    ),
                ));
                continue;
            }
            if !seen.insert(entry.name.as_str()) {
                report.push(Diagnostic::error(
                    DiagnosticCode::McpAllowlistDuplicateTool,
                    path,
                    format!("tool {:?} declared more than once", entry.name),
                ));
                continue;
            }
            let policy = entry.policy();
            if let Some(command) = command_by_name(&entry.name) {
                if policy == AllowlistPolicy::ReadOnly && command.authority.may_mutate() {
                    report.push(Diagnostic::error(
                        DiagnosticCode::McpAllowlistUnsafeReadOnlyPolicy,
                        path,
                        format!(
                            "tool {:?} has command authority {} and must be declared with policy: mutate; read-only would bypass the MutateGate and Tool-Call Attestation",
                            entry.name, command.authority
                        ),
                    ));
                    continue;
                }
            }
            tools.push(AllowedTool {
                name: entry.name.clone(),
                policy,
            });
        }
        (Self { tools }, report)
    }
}

/// The YAML-deserialized shape of `mcp-allowlist.yaml`. `policy` is a string
/// (`read-only` default, `mutate`) so the YAML stays human-friendly; it maps
/// to the typed [`AllowlistPolicy`] via [`ToolEntryYaml::policy`].
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AllowlistYaml {
    #[serde(default)]
    tools: Vec<ToolEntryYaml>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolEntryYaml {
    name: String,
    #[serde(default)]
    policy: String,
}

impl ToolEntryYaml {
    /// Map the YAML policy string to the typed enum. Unknown strings default
    /// to `read-only` (fail-safe: never silently promote to mutate).
    fn policy(&self) -> AllowlistPolicy {
        match self.policy.as_str() {
            "mutate" => AllowlistPolicy::Mutate,
            _ => AllowlistPolicy::ReadOnly,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: every default MCP tool name must be projected from the
    /// shared Command Surface. This makes command rename / MCP visibility
    /// changes visible here without copying a second list into the adapter.
    #[test]
    fn default_tools_are_registered_commands() {
        let registered: std::collections::HashSet<&str> = known_commands().into_iter().collect();
        for name in default_read_only_tool_names().chain(default_mutate_tool_names()) {
            assert!(
                registered.contains(name),
                "default tool {name:?} is not a registered forge-core command \
                 (rename or MCP visibility drift in command surface)",
            );
        }
        let ro: std::collections::HashSet<&str> = default_read_only_tool_names().collect();
        for name in default_mutate_tool_names() {
            assert!(
                !ro.contains(name),
                "tool {name:?} appears in both read-only and mutate MCP projections",
            );
        }
    }

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

    /// The registered command names, projected from
    /// `forge_core_command_surface::COMMANDS`. This is NOT a hand-maintained
    /// list; it is derived at test time so the YAML-loader tests validate
    /// against the real shared registry.
    fn known_commands() -> Vec<&'static str> {
        forge_core_command_surface::command_names().collect()
    }

    #[test]
    fn yaml_parses_read_only_and_mutate() {
        let yaml = "\
tools:
  - name: preview
    policy: read-only
  - name: execute-operation
    policy: mutate
";
        let (al, report) = Allowlist::from_yaml_str(yaml, &known_commands());
        assert!(
            !report.has_errors(),
            "unexpected errors: {:?}",
            report.diagnostics()
        );
        assert!(al.get("preview").is_some_and(|t| !t.policy.is_mutate()));
        assert!(al
            .get("execute-operation")
            .is_some_and(|t| t.policy.is_mutate()));
    }

    #[test]
    fn yaml_rejects_read_only_policy_for_mutating_command_surface_authority() {
        let yaml = "\
tools:
  - name: preview
  - name: execute-operation
    policy: read-only
  - name: claim
";
        let (al, report) = Allowlist::from_yaml_str(yaml, &known_commands());

        assert!(report.has_errors());
        assert!(al.get("preview").is_some_and(|t| !t.policy.is_mutate()));
        assert!(al.get("execute-operation").is_none());
        assert!(al.get("claim").is_none());
        let unsafe_policy_count = report
            .diagnostics()
            .iter()
            .filter(|d| d.code == DiagnosticCode::McpAllowlistUnsafeReadOnlyPolicy)
            .count();
        assert_eq!(
            unsafe_policy_count, 2,
            "mutating and mixed-authority commands must both fail closed when declared read-only: {:?}",
            report.diagnostics()
        );
    }

    #[test]
    fn yaml_unknown_tool_is_error_diagnostic() {
        let yaml = "\
tools:
  - name: preview
  - name: not-a-real-command
";
        let (al, report) = Allowlist::from_yaml_str(yaml, &known_commands());
        assert!(report.has_errors());
        // The known tool is still admitted; only the unknown one is dropped.
        assert!(al.get("preview").is_some());
        assert!(al.get("not-a-real-command").is_none());
        assert!(report
            .diagnostics()
            .iter()
            .any(|d| d.code == DiagnosticCode::McpAllowlistUnknownTool));
    }

    #[test]
    fn yaml_duplicate_tool_is_error() {
        let yaml = "\
tools:
  - name: preview
  - name: preview
";
        let (_al, report) = Allowlist::from_yaml_str(yaml, &known_commands());
        assert!(report
            .diagnostics()
            .iter()
            .any(|d| d.code == DiagnosticCode::McpAllowlistDuplicateTool));
    }

    #[test]
    fn yaml_empty_tools_is_error() {
        let yaml = "tools: []\n";
        let (al, report) = Allowlist::from_yaml_str(yaml, &known_commands());
        assert!(report.has_errors());
        assert!(al.is_empty());
        assert!(report
            .diagnostics()
            .iter()
            .any(|d| d.code == DiagnosticCode::McpAllowlistEmpty));
    }

    #[test]
    fn yaml_malformed_is_parse_error() {
        let yaml = "tools: [this is not: valid: yaml\n";
        let (al, report) = Allowlist::from_yaml_str(yaml, &known_commands());
        assert!(report.has_errors());
        assert!(al.is_empty());
        assert!(report
            .diagnostics()
            .iter()
            .any(|d| d.code == DiagnosticCode::McpAllowlistYamlParseFailed));
    }

    #[test]
    fn yaml_unknown_policy_defaults_to_readonly() {
        // Fail-safe: an unrecognized policy string never silently promotes
        // to mutate.
        let yaml = "\
tools:
  - name: preview
    policy: bogus-value
";
        let (al, report) = Allowlist::from_yaml_str(yaml, &known_commands());
        assert!(!report.has_errors());
        assert!(al.get("preview").is_some_and(|t| !t.policy.is_mutate()));
    }
}
