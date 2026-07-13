//! Canonical command surface metadata for `forge-core`.
//!
//! This crate owns command facts that are shared by CLI help, CLI dispatch
//! metadata, MCP tool projection, and generated command documentation. Host
//! adapter projection can migrate to this same seam in a later slice. It
//! deliberately does **not** own command handlers or parsing implementation;
//! those remain behind the CLI module seam so the surface metadata can stay
//! dependency-free and reusable.

use core::fmt;

/// Coarse authority class for a top-level command path.
///
/// The class is intentionally conservative for command paths that contain
/// mixed read/write subcommands. Gate decisions must still rely on the
/// concrete parser/runtime path; this metadata exists to keep projections
/// honest and drift-resistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandAuthority {
    /// The command path is read-only.
    ReadOnly,
    /// The command path mutates Forge runtime state.
    MutatesForgeState,
    /// The command path contains both read-only and mutating subcommands.
    MixedBySubcommand,
    /// The command path starts a protocol adapter rather than a normal envelope command.
    AdapterProtocol,
}

impl CommandAuthority {
    /// Whether this authority class may mutate Forge runtime state.
    #[must_use]
    pub const fn may_mutate(self) -> bool {
        matches!(self, Self::MutatesForgeState | Self::MixedBySubcommand)
    }

    /// Stable snake-case identifier used by generated docs and adapters.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::MutatesForgeState => "mutates_forge_state",
            Self::MixedBySubcommand => "mixed_by_subcommand",
            Self::AdapterProtocol => "adapter_protocol",
        }
    }
}

impl fmt::Display for CommandAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a command path emits machine-readable output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonMode {
    /// The command supports the normal `CliEnvelope` JSON/text switch.
    EnvelopeOptional,
    /// The command owns a protocol stream on stdout and cannot print normal envelopes there.
    ProtocolStream,
}

impl JsonMode {
    /// Stable snake-case identifier used by generated docs and adapters.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EnvelopeOptional => "envelope_optional",
            Self::ProtocolStream => "protocol_stream",
        }
    }
}

impl fmt::Display for JsonMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How the command path is visible to MCP and future adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum McpVisibility {
    /// The command is not exposed by default but remains available to an explicit allowlist.
    AllowlistOnly,
    /// The command is part of the default read-only MCP surface.
    DefaultReadOnly,
    /// The command is part of the opt-in default mutating MCP surface.
    DefaultMutate,
}

impl McpVisibility {
    /// Whether this visibility contributes to the default read-only MCP projection.
    #[must_use]
    pub const fn is_default_read_only(self) -> bool {
        matches!(self, Self::DefaultReadOnly)
    }

    /// Whether this visibility contributes to the default mutating MCP projection.
    #[must_use]
    pub const fn is_default_mutate(self) -> bool {
        matches!(self, Self::DefaultMutate)
    }

    /// Stable snake-case identifier used by generated docs and adapters.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AllowlistOnly => "allowlist_only",
            Self::DefaultReadOnly => "default_read_only",
            Self::DefaultMutate => "default_mutate",
        }
    }
}

impl fmt::Display for McpVisibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One canonical top-level command path.
///
/// `usage_lines` are the exact lines rendered by global CLI help. They live
/// here, not in the CLI handler table, so renaming a command or changing a
/// flag updates help, adapter projection tests, and generated docs from the
/// same seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandSpec {
    /// The `argv[1]` token that selects this command.
    pub name: &'static str,
    /// Canonical global help usage lines, without trailing newlines.
    pub usage_lines: &'static [&'static str],
    /// Coarse command authority class.
    pub authority: CommandAuthority,
    /// JSON/envelope output mode.
    pub json_mode: JsonMode,
    /// MCP adapter visibility classification.
    pub mcp_visibility: McpVisibility,
}

impl CommandSpec {
    /// The first usage line, suitable for compact docs and projections.
    #[must_use]
    pub fn canonical_usage(&self) -> &'static str {
        self.usage_lines.first().copied().unwrap_or("")
    }

    /// Render a usage line for local command-tree help.
    ///
    /// Global help owns fully qualified lines (`forge-core claim acquire ...`).
    /// Local help usually keeps a command-tree header (`forge-core claim
    /// <subcommand> [options]`) and then renders child usage lines without the
    /// repeated `forge-core <command>` prefix. Keeping that projection here
    /// prevents every CLI module from hand-rolling the same string slicing.
    #[must_use]
    pub fn local_usage_line(&self, line: &'static str) -> &'static str {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("forge-core ") else {
            return trimmed;
        };
        let Some(rest) = rest.strip_prefix(self.name) else {
            return trimmed;
        };
        rest.strip_prefix(' ').unwrap_or(trimmed)
    }

    /// Iterate over usage lines projected for local command-tree help.
    #[must_use = "iterators are lazy; consume the iterator to render projected usage lines"]
    pub fn local_usage_lines(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.usage_lines
            .iter()
            .map(|line| self.local_usage_line(line))
    }

    /// Iterate over concrete subcommand names present in usage lines.
    ///
    /// Placeholder first tokens such as `<subcommand>`, `[--root ...]`, and
    /// `(route|policy|...)` are deliberately ignored; this keeps unknown-
    /// subcommand hints honest for command trees with real children.
    #[must_use = "iterators are lazy; consume the iterator to render concrete subcommand names"]
    pub fn concrete_subcommand_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        let mut seen = Vec::new();
        self.local_usage_lines()
            .filter_map(|line| line.split_whitespace().next())
            .filter(|token| is_concrete_subcommand_token(token))
            .filter(move |token| {
                if seen.contains(token) {
                    false
                } else {
                    seen.push(*token);
                    true
                }
            })
    }

    /// Iterate over local usage lines below a concrete subcommand path.
    ///
    /// This supports nested command trees such as `research source add` while
    /// keeping prefix stripping and path matching owned by the Command Surface
    /// seam instead of re-implemented by each CLI adapter.
    #[must_use = "iterators are lazy; consume the iterator to render projected usage lines"]
    pub fn local_usage_lines_under_subcommand_path<'a>(
        &'a self,
        path: &'a [&'a str],
    ) -> impl Iterator<Item = &'static str> + 'a {
        self.local_usage_lines()
            .filter_map(move |line| strip_local_usage_path(line, path))
    }

    /// Lookup the fully qualified usage line for a concrete subcommand.
    #[must_use]
    pub fn usage_line_for_subcommand(&self, subcommand: &str) -> Option<&'static str> {
        self.usage_lines
            .iter()
            .map(|line| line.trim_start())
            .find(|line| self.local_usage_line(line).split_whitespace().next() == Some(subcommand))
    }

    /// Lookup the fully qualified usage line for a concrete subcommand path.
    #[must_use]
    pub fn usage_line_for_subcommand_path(&self, path: &[&str]) -> Option<&'static str> {
        if path.is_empty() {
            return None;
        }
        self.usage_lines
            .iter()
            .map(|line| line.trim_start())
            .find(|line| strip_local_usage_path(self.local_usage_line(line), path).is_some())
    }

    /// Render a compact unknown-subcommand hint below a concrete path.
    #[must_use]
    pub fn concrete_child_hint_under_subcommand_path(&self, path: &[&str]) -> String {
        let mut names = Vec::new();
        for name in self
            .local_usage_lines_under_subcommand_path(path)
            .filter_map(|line| line.split_whitespace().next())
            .filter(|token| is_concrete_subcommand_token(token))
        {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names.join(" | ")
    }

    /// Render a compact unknown-subcommand hint from concrete usage lines.
    #[must_use]
    pub fn concrete_subcommand_hint(&self) -> String {
        self.concrete_child_hint_under_subcommand_path(&[])
    }
}

fn is_concrete_subcommand_token(token: &str) -> bool {
    !token.starts_with('<') && !token.starts_with('[') && !token.starts_with('(')
}

fn strip_local_usage_path<'a>(line: &'a str, path: &[&str]) -> Option<&'a str> {
    let mut rest = line.trim_start();
    for expected in path {
        let mut parts = rest.splitn(2, char::is_whitespace);
        let token = parts.next()?;
        if token != *expected {
            return None;
        }
        rest = parts.next().unwrap_or_default().trim_start();
    }
    Some(rest)
}

#[rustfmt::skip]
pub const COMMAND_GUIDE: CommandSpec = CommandSpec {
    name: "guide",
    usage_lines:     &[
                "       forge-core guide describe [--catalog-dir <path>] [--json|--no-json]",
                "       forge-core guide decide --decision-file <path> [--catalog-dir <path>] [--gates-file <path>] [--json|--no-json]",
                "       forge-core guide status --phase <phase> [--catalog-dir <path>] [--json|--no-json]",
                "       forge-core guide migration-audit [--catalog-dir <path>] [--plan-file <yaml>] [--json|--no-json]",
                "       forge-core guide rollout-audit --manifest-file <yaml> [--batch-file <yaml>]... [--catalog-dir <path>] [--plan-file <yaml>] [--json|--no-json]",
                "       forge-core guide govern-simulate --bundle-file <yaml> --input-file <yaml> [--legacy-workflow-file <yaml>] [--json|--no-json]",
            ],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

/// Trusted P5c workflow-governance runtime. `next`, `resume`, and `shadow` are
/// read-only; initialization, evidence/applicability recording, human
/// decisions, waivers, and completion append to the kernel-owned governance
/// ledger under its exclusive lock.
pub const COMMAND_WORKFLOW: CommandSpec = CommandSpec {
    name: "workflow",
    usage_lines: &[
        "       forge-core workflow init [--root <path>] [--json|--no-json]",
        "       forge-core workflow next [--root <path>] [--json|--no-json]",
        "       forge-core workflow action-packets [--root <path>] [--json|--no-json]",
        "       forge-core workflow action authorize --root <path> --packet-digest <sha256> --input-file <closed-input.json> --credential-id <id> [--json|--no-json]",
        "       forge-core workflow action apply --root <path> --origin-envelope-file <signed-json> [--json|--no-json]",
        "       forge-core workflow intent record --root <path> --origin-envelope-file <signed-json> [--json|--no-json]",
        "       forge-core workflow resume [--root <path>] [--json|--no-json]",
        "       forge-core workflow release-status [--root <path>] [--json|--no-json]",
        "       forge-core workflow retirement-status [--root <path>] [--json|--no-json]",
        "       forge-core workflow release-upgrade [--root <path>] --target-release-id <id> --expected-current-release-digest <sha256> --expected-head-digest <sha256> --expected-snapshot-digest <sha256> [--json|--no-json]",
        "       forge-core workflow shadow [--root <path>] [--json|--no-json]",
        "       forge-core workflow applicability-authorize [--root <path>] --request-file <json> --attestation-file <json> [--json|--no-json]",
        "       forge-core workflow capability-authorize [--root <path>] --request-file <json> --attestation-file <json> [--json|--no-json]",
        "       forge-core workflow complete [--root <path>] --if-snapshot <sha256> [--principal <id>] [--json|--no-json]",
        "       forge-core workflow decision-resolve [--root <path>] --request-file <json> --attestation-file <json> [--json|--no-json]",
        "       forge-core workflow evidence-authorize [--root <path>] --request-file <json> --attestation-file <json> [--json|--no-json]",
        "       forge-core workflow signal-authorize [--root <path>] --request-file <json> --attestation-file <json> [--json|--no-json]",
        "       forge-core workflow waiver-authorize [--root <path>] --request-file <json> --attestation-file <json> [--json|--no-json]",
        "       forge-core workflow credential provision --root <path> --credential-id <id> --principal-id <id> --agent-id <id> --profile <human|agent|runtime> [--json|--no-json]",
        "       forge-core workflow credential rotate --root <path> --replaces <old-id> --credential-id <new-id> --principal-id <id> --agent-id <id> --profile <human|agent|runtime> [--json|--no-json]",
        "       forge-core workflow credential revoke --root <path> --credential-id <id> [--json|--no-json]",
        "       forge-core workflow credential status --root <path> [--json|--no-json]",
        "       forge-core workflow credential sign --root <path> --credential-id <id> --kind <applicability|capability|decision|evidence|signal|waiver> --request-file <json> [--output-file <json>] [--json|--no-json]",
        "       forge-core workflow broker trust --root <path> --issuer-id <id> --profile <human|reviewer|runtime> --public-key-file <hex> --ceremony-ref <ref> --ceremony-file <artifact> [--json|--no-json]",
        "       forge-core workflow broker rotate --root <path> --replaces <old-id> --issuer-id <new-id> --profile <human|reviewer|runtime> --public-key-file <hex> --ceremony-ref <ref> --ceremony-file <artifact> [--json|--no-json]",
        "       forge-core workflow broker revoke --root <path> --issuer-id <id> [--json|--no-json]",
        "       forge-core workflow broker status --root <path> [--json|--no-json]",
    ],
    authority: CommandAuthority::MixedBySubcommand,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_CLAIM: CommandSpec = CommandSpec {
    name: "claim",
    usage_lines:     &[
                "       forge-core claim acquire [--root <path>] --scope <kind> --id <scope-id> --agent <id> [--principal-id <id>] [--path <repo-path>...] [--role worker] [--ttl 600] [--heartbeat-interval 120] [--claims-dir <path>] [--now-unix <epoch>] [--no-sync] [--json|--no-json]",
                "       forge-core claim heartbeat [--root <path>] --id <claim-id> --agent <id> [--claims-dir <path>] [--now-unix <epoch>] [--no-sync] [--json|--no-json]",
                "       forge-core claim release [--root <path>] --id <claim-id> --agent <id> [--claims-dir <path>] [--now-unix <epoch>] [--no-sync] [--json|--no-json]",
                "       forge-core claim handoff [--root <path>] --id <claim-id> --agent <id> --summary <text> [--evidence <path>...] [--claims-dir <path>] [--now-unix <epoch>] [--no-sync] [--json|--no-json]",
                "       forge-core claim status [--root <path>] [--claims-dir <path>] [--now-unix <epoch>] [--from-cache] [--json|--no-json]",
                "       forge-core claim reconcile [--root <path>] [--claims-dir <path>] [--now-unix <epoch>] [--loop] [--interval-ms 30000] [--max-ticks <n>] [--no-sync] [--json|--no-json]",
                "       forge-core claim check-write [--root <path>] --agent <id> --target <path> [--target <path>...] [--claims-dir <path>] [--now-unix <epoch>] [--json|--no-json]",
            ],
    authority: CommandAuthority::MixedBySubcommand,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::DefaultMutate,
};

pub const COMMAND_AUTONOMY: CommandSpec = CommandSpec {
    name: "autonomy",
    usage_lines:     &["       forge-core autonomy route --policy-file <path> [--goal-file <path>] [--tool-class <snake_case>]... [--failure-streak <n>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

/// Agent-facing projection over the pure Assurance Case / Obligation Engine
/// Interface. Both subcommands are read-only: persistence belongs to the host
/// Adapter, which may store the returned Assurance Case for later resume.
pub const COMMAND_ASSURANCE: CommandSpec = CommandSpec {
    name: "assurance",
    usage_lines: &[
        "       forge-core assurance (--input-file <path>|--case-file <path>) [--root <path>] [--json|--no-json]",
        "       forge-core assurance derive --input-file <path> [--root <path>] [--json|--no-json]",
        "       forge-core assurance resume --case-file <path> [--root <path>] [--json|--no-json]",
    ],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::DefaultReadOnly,
};

/// P6 Domain Pack inspection, deterministic projection, governed learning, and
/// lifecycle surface. Trust provisioning, reviewer rotation, and promotion
/// mutate external monotonic anchors; `apply` activates project state; status
/// commands may complete an interrupted crash-safe replacement before reading.
pub const COMMAND_DOMAIN_PACK: CommandSpec = CommandSpec {
    name: "domain-pack",
    usage_lines: &[
        "       forge-core domain-pack validate --manifest-file <path> --content-file <path> [--artifact-root <path>] [--forge-core-version <semver>] [--json|--no-json]",
        "       forge-core domain-pack compose --request-file <path> [--artifact-root <path>] [--json|--no-json]",
        "       forge-core domain-pack resolve --request-file <path> --registry-file <path> [--json|--no-json]",
        "       forge-core domain-pack learning capture --candidate-file <yaml> --state-root <.forge-method> [--json|--no-json]",
        "       forge-core domain-pack learning status --state-root <.forge-method> [--json|--no-json]",
        "       forge-core domain-pack learning evaluate --dossier-file <yaml> [--candidate-file <yaml>]... [--review-file <yaml>]... [--conflict-file <yaml>]... [--json|--no-json]",
        "       forge-core domain-pack learning conflict-check --dossier-file <yaml> [--candidate-file <yaml>]... [--review-file <yaml>]... [--conflict-file <yaml>]... [--json|--no-json]",
        "       forge-core domain-pack learning trust-provision --operator-root <dir> --reviewer-registry-file <yaml> --reviewed-registry-file <yaml> --project-root <dir> --state-root <.forge-method> --operator-acknowledge-trust-on-first-use I_UNDERSTAND_REVIEW_TRUST_ON_FIRST_USE [--json|--no-json]",
        "       forge-core domain-pack learning reviewer-rotate --operator-root <dir> --reviewer-registry-file <current-yaml> --proposed-reviewer-registry-file <yaml> --project-root <dir> --state-root <.forge-method> [--json|--no-json]",
        "       forge-core domain-pack learning registry-check --operator-root <dir> --reviewer-registry-file <yaml> --reviewed-registry-file <yaml> --project-root <dir> --state-root <.forge-method> [--json|--no-json]",
        "       forge-core domain-pack learning promote --operator-root <dir> --reviewer-registry-file <yaml> --reviewed-registry-file <current-yaml> --proposed-registry-file <yaml> --dossier-file <yaml> --candidate-file <yaml> [--candidate-file <yaml>]... [--conflict-file <yaml>]... --decision-file <yaml> --authorization-file <yaml> --review-file <yaml> --review-file <yaml> --project-root <dir> --state-root <.forge-method> [--json|--no-json]",
        "       forge-core domain-pack trust-provision --operator-root <path> --trust-policy-file <path> --registry-file <path> --project-root <path> [--artifact-root <path>] [--state-root <.forge-method>] --operator-acknowledge-trust-on-first-use I_UNDERSTAND_TRUST_ON_FIRST_USE [--json|--no-json]",
        "       forge-core domain-pack status [--state-root <.forge-method>] [--json|--no-json]",
        "       forge-core domain-pack recover [--state-root <.forge-method>] [--json|--no-json]",
        "       forge-core domain-pack preflight --preflight-file <path> --trust-policy-file <path> --registry-file <path> --reviewer-registry-file <path> --reviewed-registry-file <path> --resolution-request-file <path> --composition-request-file <path> --trust-input-file <path> --project-root <path> [--artifact-root <path>] [--state-root <.forge-method>] [--json|--no-json]",
        "       forge-core domain-pack apply --preflight-file <path> --trust-policy-file <path> --registry-file <path> --reviewer-registry-file <path> --reviewed-registry-file <path> --resolution-request-file <path> --composition-request-file <path> --trust-input-file <path> --project-root <path> [--artifact-root <path>] [--state-root <.forge-method>] [--json|--no-json]",
    ],
    authority: CommandAuthority::MixedBySubcommand,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_CONTRACT: CommandSpec = CommandSpec {
    name: "contract",
    usage_lines: &[
        "       forge-core contract validate --kind <kind> --file <path> [--json|--no-json]",
    ],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_ISOLATION: CommandSpec = CommandSpec {
    name: "isolation",
    usage_lines: &[
        "       forge-core isolation propose [--root <path>] --agent <id> --branch <name> --worktree-path <path> --base-ref <ref> [--id <id>] [--merge-policy rebase|merge|squash] [--claim <claim-id>] [--isolation-dir <path>] [--now-unix <epoch>] [--json|--no-json]",
        "       forge-core isolation status [--root <path>] [--agent <id>] [--isolation-dir <path>] [--json|--no-json]",
        "       forge-core isolation merge-plan [--root <path>] --id <isolation-id> [--isolation-dir <path>] [--now-unix <epoch>] [--json|--no-json]",
        "       forge-core isolation transition [--root <path>] --id <isolation-id> --to proposed|active|merging|merged|abandoned [--isolation-dir <path>] [--now-unix <epoch>] [--json|--no-json]",
    ],
    authority: CommandAuthority::MixedBySubcommand,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_MEMORY: CommandSpec = CommandSpec {
    name: "memory",
    usage_lines:     &[
                "       forge-core memory ingest  --entry-file <path> --policy-file <path> [--root <path>] [--memory-dir <path>] [--json|--no-json]",
                "       forge-core memory list    [--root <path>] [--now-unix <epoch>] [--memory-dir <path>] [--json|--no-json]",
                "       forge-core memory forget  --entry-id <id> [--root <path>] [--memory-dir <path>] [--json|--no-json]",
                "       forge-core memory promote --entry-id <id> --policy-file <path> --evidence <ref>... [--root <path>] [--memory-dir <path>] [--json|--no-json]",
                "       forge-core memory review  (deferred — requires F07 governance)",
            ],
    authority: CommandAuthority::MixedBySubcommand,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::DefaultMutate,
};

pub const COMMAND_GOVERNANCE: CommandSpec = CommandSpec {
    name: "governance",
    usage_lines:     &[
                "       forge-core governance record   --conflict-file <path> [--root <path>] [--governance-dir <path>] [--json|--no-json]",
                "       forge-core governance conflicts [--status pending|resolved|escalated] [--root <path>] [--governance-dir <path>] [--json|--no-json]",
                "       forge-core governance arbitrate --conflict-id <id> --policy-file <path> --arbiter <principal> (--awarded-to <principal> | --both-released | --split-scope) [--root <path>] [--governance-dir <path>] [--json|--no-json]",
                "       forge-core governance escalate  --conflict-id <id> --policy-file <path> --principal <principal> [--root <path>] [--governance-dir <path>] [--json|--no-json]",
            ],
    authority: CommandAuthority::MixedBySubcommand,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_COORDINATION: CommandSpec = CommandSpec {
    name: "coordination",
    usage_lines:     &["       forge-core coordination validate [--suite <path>] [--repo-root <path>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_PROJECT: CommandSpec = CommandSpec {
    name: "project",
    usage_lines:     &[
                "       forge-core project init [--root <path>] [--project-id <id>] [--sidecar-root <path>] [--state-root <path>] [--json|--no-json]",
                "       forge-core project resolve [--root <path>] [--json|--no-json]",
            ],
    authority: CommandAuthority::MixedBySubcommand,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_GRAPH: CommandSpec = CommandSpec {
    name: "graph",
    usage_lines:     &[
                "       forge-core graph validate --root <project> --graph <path> [--json|--no-json]",
                "       forge-core graph run --root <project> --graph <path> --dry-run [--agent <id>] [--claims-dir <path>] [--now-unix <epoch>] [--json|--no-json]",
            ],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::DefaultReadOnly,
};

pub const COMMAND_EVAL: CommandSpec = CommandSpec {
    name: "eval",
    usage_lines:     &["       forge-core eval compare [--root <project>] [--suite <path>] --baseline <single-agent|graph|mas|manual> --candidate <single-agent|graph|mas|manual> [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

/// Default suite used by `forge-core eval compare` when `--suite` is omitted.
///
/// This is command-surface metadata because both the eval implementation and
/// help text must describe the same default path.
pub const COMMAND_EVAL_DEFAULT_SUITE: &str =
    "docs/fixtures/eval-run-v0/eval-compare-smoke-suite.yaml";

pub const COMMAND_EVAL_HARNESS: CommandSpec = CommandSpec {
    name: "eval-harness",
    usage_lines:     &["       forge-core eval-harness --config <yaml> [--root <path>] [--corpus <yaml>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_TELEMETRY: CommandSpec = CommandSpec {
    name: "telemetry",
    usage_lines:     &["       forge-core telemetry export [--root <project>] [--contract <path>] [--output <path>] [--format jsonl|otel-json] [--trace-id <id>|--run-id <id>|--latest-run] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

/// Default telemetry contract used by `forge-core telemetry export` when
/// `--contract` is omitted.
///
/// This is command-surface metadata because both the telemetry implementation
/// and help text must describe the same default path.
pub const COMMAND_TELEMETRY_DEFAULT_CONTRACT_PATH: &str = "contracts/examples/telemetry.yaml";

/// Human-readable description of the implicit telemetry trace source.
pub const COMMAND_TELEMETRY_DEFAULT_TRACE_SOURCE: &str =
    "resolved <state_root>/traces/events.ndjson";

pub const COMMAND_PREVIEW: CommandSpec = CommandSpec {
    name: "preview",
    usage_lines:     &["       forge-core preview [--root <path>] --operation <path> [--recorded-at <value>] [--agent-id <id>] [--principal-id <id>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::DefaultReadOnly,
};

pub const COMMAND_READY: CommandSpec = CommandSpec {
    name: "ready",
    usage_lines:     &["       forge-core ready [--root <path>] --operation <path> [--recorded-at <value>] [--agent-id <id>] [--principal-id <id>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::DefaultReadOnly,
};

pub const COMMAND_EXPLAIN: CommandSpec = CommandSpec {
    name: "explain",
    usage_lines: &[
        "       forge-core explain [--root <path>] (--last-run | --run-id <id>) [--json|--no-json]",
    ],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::DefaultReadOnly,
};

pub const COMMAND_COST: CommandSpec = CommandSpec {
    name: "cost",
    usage_lines:     &["       forge-core cost [--root <path>] [--run-id <id> | --last-run] [--graph-id <id>] [--principal <id>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_RISK_AUDIT: CommandSpec = CommandSpec {
    name: "risk-audit",
    usage_lines: &[
        "       forge-core risk-audit [--root <path>] --rules <path> [--json|--no-json]",
    ],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_VALIDATE: CommandSpec = CommandSpec {
    name: "validate",
    usage_lines: &["       forge-core validate [--root <path>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_PREFLIGHT: CommandSpec = CommandSpec {
    name: "preflight",
    usage_lines:     &[
        "       forge-core preflight [--root <path>] [--json|--no-json] [--profile <name>] [--gate <name>]... [--expected-anchor <count>]",
        "       forge-core preflight init [--root <path>] [--profile <name>] [--json|--no-json]",
    ],
    authority: CommandAuthority::MixedBySubcommand,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_EXECUTE_OPERATION: CommandSpec = CommandSpec {
    name: "execute-operation",
    usage_lines:     &["       forge-core execute-operation --root <path> --operation <path> [--command <path>] [--effect <path>] [--payload <target_ref>=<path>] [--max-payload-bytes <bytes>] [--allow-payload-outside-root] [--recorded-at <value>] [--tx-id-prefix <value>] [--require-risk-audit <path>] [--require-citation] [--no-sync] [--json|--no-json]"],
    authority: CommandAuthority::MutatesForgeState,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::DefaultMutate,
};

pub const COMMAND_REBUILD_EFFECT_INDEX: CommandSpec = CommandSpec {
    name: "rebuild-effect-index",
    usage_lines:     &["       forge-core rebuild-effect-index [--root <path>] [--wal <path>] [--index <path>] [--lock <path>] [--recorded-at <value>] [--no-sync] [--json|--no-json]"],
    authority: CommandAuthority::MutatesForgeState,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_QUERY_EFFECT_INDEX: CommandSpec = CommandSpec {
    name: "query-effect-index",
    usage_lines:     &["       forge-core query-effect-index [--root <path>] [--index <path>] [--logical-ref <ref>] [--effect-id <id>] [--operation-id <id>] [--target-kind <kind>] [--consumer-use <discovery|diagnostics|handoff_context>] [--context] [--max-context-groups <n>] [--adapter-kind <codex|cursor|claude|opencode|vscode|pidev|forge_standalone|custom>] [--adapter-trigger <evidence_discovery|diagnostics|handoff_preparation|manual_inspection>] [--latest] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::DefaultReadOnly,
};

pub const COMMAND_HOST_ADAPTER_MANIFEST: CommandSpec = CommandSpec {
    name: "host-adapter-manifest",
    usage_lines: &["       forge-core host-adapter-manifest [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_PROJECTION: CommandSpec = CommandSpec {
    name: "host-adapter-projection",
    usage_lines:     &["       forge-core host-adapter-projection [--target <mcp_tools|borrowed_shell|app_ui>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_PROCESS_POLICY: CommandSpec = CommandSpec {
    name: "host-adapter-process-policy",
    usage_lines:     &["       forge-core host-adapter-process-policy [--target <mcp_stdio|borrowed_shell|app_bridge>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_ADMIT_INVOCATION: CommandSpec = CommandSpec {
    name: "host-adapter-admit-invocation",
    usage_lines:     &["       forge-core host-adapter-admit-invocation --command <name> [--target <mcp_stdio|borrowed_shell|app_bridge>] [--explicit] [--argv <arg>] [--cwd <path>] [--env-key <key>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_DISTRIBUTION_POLICY: CommandSpec = CommandSpec {
    name: "host-adapter-distribution-policy",
    usage_lines: &["       forge-core host-adapter-distribution-policy [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_ADMIT_DISTRIBUTION: CommandSpec = CommandSpec {
    name: "host-adapter-admit-distribution",
    usage_lines:     &["       forge-core host-adapter-admit-distribution --artifact <name> [--target <codex|cursor|claude|opencode|vscode|pidev|forge_standalone|custom>] [--channel <stable|canary|dev>] [--sha256 <digest>] [--signature-ref <ref>] [--provenance-ref <ref>] [--source-ref <ref>] [--version <value>] [--compatible-core-version <value>] [--rollback-ref <ref>] [--update-summary-ref <ref>] [--explicit-canary-opt-in] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_ARTIFACT: CommandSpec = CommandSpec {
    name: "host-adapter-verify-artifact",
    usage_lines:     &["       forge-core host-adapter-verify-artifact --artifact-path <path> --sha256 <digest> [--signature-ref <ref>] [--provenance-ref <ref>] [--source-ref <ref>] [--version <value>] [--compatible-core-version <value>] [--rollback-ref <ref>] [--update-summary-ref <ref>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_PROVENANCE: CommandSpec = CommandSpec {
    name: "host-adapter-verify-provenance",
    usage_lines:     &["       forge-core host-adapter-verify-provenance --artifact-path <path> --provenance-path <path> --signature-path <path> --public-key-path <path> --transparency-log-path <path> --sha256 <digest> --expected-builder-id <id> --expected-source-uri <uri> --expected-source-ref <ref> [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_REKOR_ENTRY: CommandSpec = CommandSpec {
    name: "host-adapter-verify-rekor-entry",
    usage_lines:     &["       forge-core host-adapter-verify-rekor-entry --log-entry-path <path> --public-key-path <path> --expected-log-id <id> [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_TRUST_POLICY: CommandSpec = CommandSpec {
    name: "host-adapter-verify-sigstore-trust-policy",
    usage_lines:     &["       forge-core host-adapter-verify-sigstore-trust-policy --policy-path <path> [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_FULCIO_CERTIFICATE_IDENTITY: CommandSpec = CommandSpec {
    name: "host-adapter-verify-fulcio-certificate-identity",
    usage_lines:     &["       forge-core host-adapter-verify-fulcio-certificate-identity --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> [--issuer-certificate-path <path>] --verification-time-unix <seconds> [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_BUNDLE_SUBJECT: CommandSpec = CommandSpec {
    name: "host-adapter-verify-sigstore-bundle-subject",
    usage_lines:     &["       forge-core host-adapter-verify-sigstore-bundle-subject --bundle-path <path> --artifact-path <path> --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> [--issuer-certificate-path <path>] --rekor-log-entry-path <path> --rekor-public-key-path <path> --expected-rekor-log-id <id> [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_DSSE_IN_TOTO_SUBJECT: CommandSpec = CommandSpec {
    name: "host-adapter-verify-sigstore-dsse-in-toto-subject",
    usage_lines:     &["       forge-core host-adapter-verify-sigstore-dsse-in-toto-subject --bundle-path <path> --artifact-path <path> --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> [--issuer-certificate-path <path>] --rekor-log-entry-path <path> --rekor-public-key-path <path> --expected-rekor-log-id <id> [--expected-predicate-type <type>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_TIMESTAMP_AUTHORITY: CommandSpec = CommandSpec {
    name: "host-adapter-verify-sigstore-timestamp-authority",
    usage_lines:     &["       forge-core host-adapter-verify-sigstore-timestamp-authority --trust-policy-path <path> --certificate-path <path> [--rekor-log-entry-path <path>] [--rekor-public-key-path <path>] [--expected-rekor-log-id <id>] [--rfc3161-timestamp-token-path <path>] [--rfc3161-timestamped-signature-path <path>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_TRANSPARENCY_SCT: CommandSpec = CommandSpec {
    name: "host-adapter-verify-certificate-transparency-sct",
    usage_lines:     &["       forge-core host-adapter-verify-certificate-transparency-sct --trust-policy-path <path> --certificate-path <path> --sct-path <path> [--sct-path <path>] --verification-time-unix-ms <milliseconds> [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_REVOCATION_POLICY: CommandSpec = CommandSpec {
    name: "host-adapter-verify-certificate-revocation-policy",
    usage_lines:     &["       forge-core host-adapter-verify-certificate-revocation-policy --trust-policy-path <path> --certificate-path <path> --trusted-signing-time-unix <seconds> [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_TUF_TRUSTED_ROOT_FRESHNESS: CommandSpec = CommandSpec {
    name: "host-adapter-verify-tuf-trusted-root-freshness",
    usage_lines:     &["       forge-core host-adapter-verify-tuf-trusted-root-freshness --trust-policy-path <path> --root-metadata-path <path> [--timestamp-metadata-path <path>] [--snapshot-metadata-path <path>] [--targets-metadata-path <path>] --update-start-time-unix <seconds> [--min-root-version <n>] [--min-timestamp-version <n>] [--min-snapshot-version <n>] [--min-targets-version <n>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_CRL_STATUS: CommandSpec = CommandSpec {
    name: "host-adapter-verify-certificate-crl-status",
    usage_lines:     &["       forge-core host-adapter-verify-certificate-crl-status --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> --crl-path <path> --verification-time-unix <seconds> [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_OCSP_STATUS: CommandSpec = CommandSpec {
    name: "host-adapter-verify-certificate-ocsp-status",
    usage_lines:     &["       forge-core host-adapter-verify-certificate-ocsp-status --trust-policy-path <path> --certificate-path <path> --issuer-certificate-path <path> --ocsp-response-path <path> --verification-time-unix <seconds> [--expected-nonce-hex <hex>] [--json|--no-json]"],
    authority: CommandAuthority::ReadOnly,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_START: CommandSpec = CommandSpec {
    name: "start",
    usage_lines: &["       forge-core start [--root <path>] [--agent-id <id>] [--json|--no-json]"],
    authority: CommandAuthority::MutatesForgeState,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_MCP: CommandSpec = CommandSpec {
    name: "mcp",
    usage_lines: &[
        "       forge-core mcp serve [--allowlist <yaml>] [--principal-registry <yaml>] [--deployment-policy <yaml>] [--snapshot <state-relative-yaml>] [--replay-anchor <absolute-operator-json>] [--enable-trusted-single-effect|--enable-trusted-operation-wide] [--root <path>] [--json|--no-json]",
        "       forge-core mcp snapshot --root <path> --operation <ref> --assurance <ref> [--command <ref>] --principal-registry <yaml> --credential-id <id> --nonce <value> [--output <state-relative-yaml>] [--now-unix <i64>] [--json|--no-json]",
        "       forge-core mcp credential <provision|rotate|revoke|sign> [operator-owned options] [--json|--no-json]",
        "       forge-core mcp readiness --root <path> --allowlist <yaml> --principal-registry <yaml> --deployment-policy <yaml> --snapshot <state-relative-yaml> --replay-anchor <absolute-operator-json> --secret-dir <path> --credential-id <id> [--client-config-output <json>] [--json|--no-json]",
        "       forge-core mcp replay-anchor <provision|verify|advance> --root <path> --anchor <absolute-operator-json> [--deployment-id <id>] [--json|--no-json]",
    ],
    authority: CommandAuthority::AdapterProtocol,
    json_mode: JsonMode::ProtocolStream,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

pub const COMMAND_RESEARCH: CommandSpec = CommandSpec {
    name: "research",
    usage_lines:     &[
                "       forge-core research source add  --source-file <path> --policy-file <path> [--root <path>] [--json|--no-json]",
                "       forge-core research source list [--root <path>] [--json|--no-json]",
                "       forge-core research check       [--root <path>] [--evidence-file <path>] [--json|--no-json]",
                "       forge-core research graph       [--root <path>] [--json|--no-json]",
                "       forge-core research cite        --source-id <id> [--root <path>] [--evidence-file <path>] [--json|--no-json]",
            ],
    authority: CommandAuthority::MixedBySubcommand,
    json_mode: JsonMode::EnvelopeOptional,
    mcp_visibility: McpVisibility::AllowlistOnly,
};

/// The complete, ordered metadata table for `forge-core` commands.
///
/// Order matches global help output. Dispatch-specific handler pointers are
/// intentionally added in `forge-core-cli`; this crate owns only reusable
/// command facts.
#[rustfmt::skip]
pub const COMMANDS: &[CommandSpec] = &[
    COMMAND_GUIDE,
    COMMAND_WORKFLOW,
    COMMAND_CLAIM,
    COMMAND_AUTONOMY,
    COMMAND_ASSURANCE,
    COMMAND_DOMAIN_PACK,
    COMMAND_CONTRACT,
    COMMAND_ISOLATION,
    COMMAND_MEMORY,
    COMMAND_GOVERNANCE,
    COMMAND_COORDINATION,
    COMMAND_PROJECT,
    COMMAND_GRAPH,
    COMMAND_EVAL,
    COMMAND_EVAL_HARNESS,
    COMMAND_TELEMETRY,
    COMMAND_PREVIEW,
    COMMAND_READY,
    COMMAND_EXPLAIN,
    COMMAND_COST,
    COMMAND_RISK_AUDIT,
    COMMAND_VALIDATE,
    COMMAND_PREFLIGHT,
    COMMAND_EXECUTE_OPERATION,
    COMMAND_REBUILD_EFFECT_INDEX,
    COMMAND_QUERY_EFFECT_INDEX,
    COMMAND_HOST_ADAPTER_MANIFEST,
    COMMAND_HOST_ADAPTER_PROJECTION,
    COMMAND_HOST_ADAPTER_PROCESS_POLICY,
    COMMAND_HOST_ADAPTER_ADMIT_INVOCATION,
    COMMAND_HOST_ADAPTER_DISTRIBUTION_POLICY,
    COMMAND_HOST_ADAPTER_ADMIT_DISTRIBUTION,
    COMMAND_HOST_ADAPTER_VERIFY_ARTIFACT,
    COMMAND_HOST_ADAPTER_VERIFY_PROVENANCE,
    COMMAND_HOST_ADAPTER_VERIFY_REKOR_ENTRY,
    COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_TRUST_POLICY,
    COMMAND_HOST_ADAPTER_VERIFY_FULCIO_CERTIFICATE_IDENTITY,
    COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_BUNDLE_SUBJECT,
    COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_DSSE_IN_TOTO_SUBJECT,
    COMMAND_HOST_ADAPTER_VERIFY_SIGSTORE_TIMESTAMP_AUTHORITY,
    COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_TRANSPARENCY_SCT,
    COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_REVOCATION_POLICY,
    COMMAND_HOST_ADAPTER_VERIFY_TUF_TRUSTED_ROOT_FRESHNESS,
    COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_CRL_STATUS,
    COMMAND_HOST_ADAPTER_VERIFY_CERTIFICATE_OCSP_STATUS,
    COMMAND_START,
    COMMAND_MCP,
    COMMAND_RESEARCH,
];

/// Look up command metadata by top-level command name.
#[must_use]
pub fn command_by_name(name: &str) -> Option<&'static CommandSpec> {
    COMMANDS.iter().find(|command| command.name == name)
}

/// Iterate over every registered command name.
pub fn command_names() -> impl Iterator<Item = &'static str> {
    COMMANDS.iter().map(|command| command.name)
}

/// Iterate over the default read-only MCP tool projection.
pub fn mcp_default_read_only_tool_names() -> impl Iterator<Item = &'static str> {
    COMMANDS
        .iter()
        .filter(|command| command.mcp_visibility.is_default_read_only())
        .map(|command| command.name)
}

/// Iterate over the opt-in default mutating MCP tool projection.
pub fn mcp_default_mutate_tool_names() -> impl Iterator<Item = &'static str> {
    COMMANDS
        .iter()
        .filter(|command| command.mcp_visibility.is_default_mutate())
        .map(|command| command.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_are_unique() {
        let mut names: Vec<&str> = command_names().collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate command surface names");
    }

    #[test]
    fn every_command_has_canonical_usage() {
        for command in COMMANDS {
            assert!(
                !command.usage_lines.is_empty(),
                "{} has no usage",
                command.name
            );
            assert!(
                command
                    .canonical_usage()
                    .trim_start()
                    .starts_with("forge-core "),
                "{} canonical usage should start with forge-core: {:?}",
                command.name,
                command.canonical_usage()
            );
        }
    }

    #[test]
    fn local_usage_lines_strip_only_the_current_command_prefix() {
        assert_eq!(
            COMMAND_CLAIM.local_usage_line(COMMAND_CLAIM.usage_lines[0]),
            "acquire [--root <path>] --scope <kind> --id <scope-id> --agent <id> [--principal-id <id>] [--path <repo-path>...] [--role worker] [--ttl 600] [--heartbeat-interval 120] [--claims-dir <path>] [--now-unix <epoch>] [--no-sync] [--json|--no-json]"
        );
        assert_eq!(
            COMMAND_PROJECT.local_usage_line(COMMAND_PROJECT.usage_lines[0]),
            "init [--root <path>] [--project-id <id>] [--sidecar-root <path>] [--state-root <path>] [--json|--no-json]"
        );
        assert_eq!(
            COMMAND_PROJECT.local_usage_line(COMMAND_CLAIM.usage_lines[0]),
            COMMAND_CLAIM.usage_lines[0].trim_start(),
            "a command must not strip another command's prefix"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One table-driven assertion covers the complete public command registry.
    fn concrete_subcommand_helpers_project_claim_and_project_children() {
        assert_eq!(
            COMMAND_CLAIM
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec![
                "acquire",
                "heartbeat",
                "release",
                "handoff",
                "status",
                "reconcile",
                "check-write"
            ]
        );
        assert_eq!(
            COMMAND_PROJECT
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec!["init", "resolve"]
        );
        assert_eq!(
            COMMAND_GUIDE
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec![
                "describe",
                "decide",
                "status",
                "migration-audit",
                "rollout-audit",
                "govern-simulate"
            ]
        );
        assert_eq!(
            COMMAND_CONTRACT
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec!["validate"]
        );
        assert_eq!(
            COMMAND_COORDINATION
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec!["validate"]
        );
        assert_eq!(
            COMMAND_GOVERNANCE
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec!["record", "conflicts", "arbitrate", "escalate"]
        );
        assert_eq!(
            COMMAND_ISOLATION
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec!["propose", "status", "merge-plan", "transition"]
        );
        assert_eq!(
            COMMAND_MEMORY
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec!["ingest", "list", "forget", "promote", "review"]
        );
        assert_eq!(
            COMMAND_AUTONOMY
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec!["route"]
        );
        assert_eq!(
            COMMAND_ASSURANCE
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec!["derive", "resume"]
        );
        assert_eq!(
            COMMAND_DOMAIN_PACK
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec![
                "validate",
                "compose",
                "resolve",
                "learning",
                "trust-provision",
                "status",
                "recover",
                "preflight",
                "apply"
            ]
        );
        assert_eq!(
            COMMAND_PREFLIGHT
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec!["init"]
        );
        assert_eq!(
            COMMAND_CLAIM.concrete_subcommand_hint(),
            "acquire | heartbeat | release | handoff | status | reconcile | check-write"
        );
    }

    #[test]
    fn workflow_surface_projects_release_pin_commands() {
        assert_eq!(
            COMMAND_WORKFLOW
                .concrete_subcommand_names()
                .collect::<Vec<_>>(),
            vec![
                "init",
                "next",
                "action-packets",
                "action",
                "intent",
                "resume",
                "release-status",
                "retirement-status",
                "release-upgrade",
                "shadow",
                "applicability-authorize",
                "capability-authorize",
                "complete",
                "decision-resolve",
                "evidence-authorize",
                "signal-authorize",
                "waiver-authorize",
                "credential",
                "broker"
            ]
        );
    }

    #[test]
    fn subcommand_usage_lookup_returns_fully_qualified_usage() {
        assert_eq!(
            COMMAND_CLAIM.usage_line_for_subcommand("status"),
            Some("forge-core claim status [--root <path>] [--claims-dir <path>] [--now-unix <epoch>] [--from-cache] [--json|--no-json]")
        );
        assert_eq!(COMMAND_CLAIM.usage_line_for_subcommand("missing"), None);
    }

    #[test]
    fn preflight_surface_exposes_init_and_mixed_authority() {
        assert_eq!(
            COMMAND_PREFLIGHT.authority,
            CommandAuthority::MixedBySubcommand
        );
        assert_eq!(
            COMMAND_PREFLIGHT.usage_line_for_subcommand_path(&["init"]),
            Some("forge-core preflight init [--root <path>] [--profile <name>] [--json|--no-json]")
        );
    }

    #[test]
    fn nested_subcommand_path_helpers_project_research_source_children() {
        assert_eq!(
            COMMAND_RESEARCH
                .local_usage_lines_under_subcommand_path(&["source"])
                .collect::<Vec<_>>(),
            vec![
                "add  --source-file <path> --policy-file <path> [--root <path>] [--json|--no-json]",
                "list [--root <path>] [--json|--no-json]",
            ]
        );
        assert_eq!(
            COMMAND_RESEARCH.usage_line_for_subcommand_path(&["source", "list"]),
            Some("forge-core research source list [--root <path>] [--json|--no-json]")
        );
        assert_eq!(
            COMMAND_RESEARCH.usage_line_for_subcommand_path(&["missing"]),
            None
        );
        assert_eq!(
            COMMAND_RESEARCH.concrete_child_hint_under_subcommand_path(&[]),
            "source | check | graph | cite"
        );
        assert_eq!(
            COMMAND_RESEARCH.concrete_child_hint_under_subcommand_path(&["source"]),
            "add | list"
        );
    }

    #[test]
    fn mcp_default_projection_is_disjoint_and_registered() {
        let read_only: std::collections::HashSet<&str> =
            mcp_default_read_only_tool_names().collect();
        let mutate: std::collections::HashSet<&str> = mcp_default_mutate_tool_names().collect();
        assert!(
            !read_only.is_empty(),
            "default read-only MCP projection is empty"
        );
        assert!(!mutate.is_empty(), "default mutate MCP projection is empty");
        if let Some(name) = read_only.intersection(&mutate).next() {
            panic!("MCP default projection classifies {name:?} as both read-only and mutate");
        }
        for name in read_only.iter().chain(mutate.iter()) {
            assert!(
                command_by_name(name).is_some(),
                "default MCP tool {name:?} is not registered"
            );
        }
    }

    #[test]
    fn default_mutate_projection_uses_mutating_authority() {
        for name in mcp_default_mutate_tool_names() {
            let command = command_by_name(name).expect("registered default mutate command");
            assert!(
                command.authority.may_mutate(),
                "default mutate MCP tool {name:?} must have mutating authority metadata"
            );
        }
    }

    #[test]
    fn default_read_only_projection_uses_read_only_authority() {
        for name in mcp_default_read_only_tool_names() {
            let command = command_by_name(name).expect("registered default read-only command");
            assert_eq!(
                command.authority,
                CommandAuthority::ReadOnly,
                "default read-only MCP tool {name:?} must not expose mutating or mixed-authority argv through the pass-through adapter"
            );
        }
    }

    #[test]
    fn metadata_identifiers_are_stable_snake_case() {
        assert_eq!(CommandAuthority::ReadOnly.as_str(), "read_only");
        assert_eq!(
            CommandAuthority::MutatesForgeState.as_str(),
            "mutates_forge_state"
        );
        assert_eq!(
            CommandAuthority::MixedBySubcommand.as_str(),
            "mixed_by_subcommand"
        );
        assert_eq!(
            CommandAuthority::AdapterProtocol.as_str(),
            "adapter_protocol"
        );
        assert_eq!(JsonMode::EnvelopeOptional.as_str(), "envelope_optional");
        assert_eq!(JsonMode::ProtocolStream.as_str(), "protocol_stream");
        assert_eq!(McpVisibility::AllowlistOnly.as_str(), "allowlist_only");
        assert_eq!(McpVisibility::DefaultReadOnly.as_str(), "default_read_only");
        assert_eq!(McpVisibility::DefaultMutate.as_str(), "default_mutate");
    }
}
