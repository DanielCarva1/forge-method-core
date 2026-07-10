//! MCP server — the adapter that exposes the shared Command Surface as MCP
//! tools over stdio JSON-RPC (ADR-0006 Decision 1).
//!
//! # Architecture
//!
//! Each MCP tool is a pass-through over a `forge-core` CLI command. Because
//! the CLI command handlers emit their `CliEnvelope` directly to stdout
//! (e.g. `memory_cmd.rs` `emit()`), the adapter invokes commands as
//! **subprocesses** (`forge-core <command> <args> --json`) and captures the
//! JSON envelope from the child's stdout. This is:
//!
//! - **thread-safe** — each tool call is an isolated process, so concurrent
//!   `tools/call` requests do not share global stdout state;
//! - **isolated** — a panicking command handler cannot poison the MCP server;
//! - **honest** — it literally is an adapter over the CLI (the deletion test:
//!   remove the adapter, the CLI still works unchanged).
//!
//! The `forge-core` binary is guaranteed present because the MCP server itself
//! runs as `forge-core mcp serve`.
//!
//! # Enforcement order (per `tools/call`)
//!
//! 1. **Allowlist** — tool name must be in the Allowlist, else fail-closed
//!    (ADR-0006 Decision 3).
//! 2. **MCP stdio mutation boundary** — mutating tools remain blocked while
//!    P4b durable replay, principal propagation, and late kernel admission are
//!    incomplete. The principal registry is implemented and tested as a
//!    substrate; it does not lift this process-security policy by itself.
//! 3. **Read-only attestation policy** — optionally require/verify a
//!    signature-only attestation for non-mutating calls.
//! 4. **Invoke** — spawn only the admitted read-only subprocess, capture the
//!    envelope, and return it.

use std::future::Future;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

use forge_core_command_surface::{command_by_name, JsonMode};
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock, ErrorData, Implementation, JsonObject,
    ListToolsResult, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{MaybeSendFuture, RequestContext, RoleServer};
use rmcp::{ServerHandler, ServiceExt};

use crate::allowlist::{Allowlist, AllowlistPolicy};
use crate::attestation::{AttestationPolicy, AttestationVerifier};
use crate::error::{McpAdapterError, ServerRunError};
#[cfg(test)]
use crate::principal_registry::AuthorizedPrincipal;
use crate::principal_registry::{
    AuthorizedPrincipalRegistry, DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
    DEFAULT_MAX_FUTURE_SKEW_SECONDS,
};

/// Configuration for a [`ForgeMcpServer`] instance.
///
/// All fields are set by the CLI `mcp serve` subcommand (F08.6) or by tests.
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// The Allowlist — the capability surface. A tool absent from it is
    /// invisible/rejected (fail-closed, ADR-0006 Decision 3).
    pub allowlist: Allowlist,
    /// The Tool-Call Attestation policy + verifier.
    pub attestation: AttestationVerifier,
    /// Operator-owned credential registry. Required whenever the Allowlist
    /// exposes a mutating tool; caller-selected keys never populate it.
    pub principal_registry: Option<AuthorizedPrincipalRegistry>,
    /// Maximum accepted age for mutating attestations.
    pub max_attestation_age_seconds: u64,
    /// Maximum accepted future clock skew for mutating attestations.
    pub max_future_skew_seconds: u64,
    /// Absolute pinned path to the current `forge-core` binary used for
    /// read-only subprocess tool invocation. PATH lookup is rejected.
    pub forge_core_binary: PathBuf,
    /// The project root forwarded as `--root <path>` to every tool that
    /// accepts it and uses as the subprocess cwd. A live server requires an
    /// absolute repo-scoped path.
    pub root: Option<PathBuf>,
}

impl McpServerConfig {
    /// Build a default config: read-only Allowlist, default attestation policy
    /// (required-for-mutate), current executable pinned, current cwd scoped.
    #[must_use]
    pub fn default_read_only() -> Self {
        Self {
            allowlist: Allowlist::default_read_only(),
            attestation: AttestationVerifier::new(AttestationPolicy::Default),
            principal_registry: None,
            max_attestation_age_seconds: DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
            max_future_skew_seconds: DEFAULT_MAX_FUTURE_SKEW_SECONDS,
            forge_core_binary: std::env::current_exe()
                .unwrap_or_else(|_| PathBuf::from("forge-core")),
            root: std::env::current_dir().ok(),
        }
    }

    /// Validate the fail-closed stdio process-security boundary.
    ///
    /// # Errors
    ///
    /// Returns [`McpAdapterError::Config`] if mutation is exposed, the binary
    /// is not an absolute pinned path, or the repo-scoped root is absent or
    /// relative. P4b.1 validates operator identity, but live stdio mutation
    /// remains disabled until durable replay and kernel admission handoff land.
    pub fn validate_process_security(&self) -> Result<(), McpAdapterError> {
        let exposes_mutation = self.allowlist.iter().any(|tool| tool.policy.is_mutate());
        if exposes_mutation && self.principal_registry.is_none() {
            return Err(McpAdapterError::Config(
                "mutating MCP allowlist requires an operator principal registry".to_owned(),
            ));
        }
        if exposes_mutation {
            return Err(McpAdapterError::Config(
                "mutating MCP stdio remains disabled until durable replay and late kernel Execution Admission are enforced"
                    .to_owned(),
            ));
        }
        if !self.forge_core_binary.is_absolute() {
            return Err(McpAdapterError::Config(
                "MCP subprocess binary must be an absolute pinned path".to_owned(),
            ));
        }
        let Some(root) = self.root.as_ref() else {
            return Err(McpAdapterError::Config(
                "MCP server requires an absolute repo-scoped root".to_owned(),
            ));
        };
        if !root.is_absolute() {
            return Err(McpAdapterError::Config(
                "MCP server root must be absolute".to_owned(),
            ));
        }
        Ok(())
    }
}

/// The Forge MCP server. Holds config; the `rmcp` runtime is driven by
/// [`ForgeMcpServer::run_stdio`] (F08.3/F08.6).
#[derive(Debug, Clone)]
pub struct ForgeMcpServer {
    config: McpServerConfig,
}

impl ForgeMcpServer {
    #[must_use]
    pub fn new(config: McpServerConfig) -> Self {
        Self { config }
    }

    /// Build a server only when its stdio capability surface is safe to expose.
    ///
    /// # Errors
    ///
    /// Returns [`McpAdapterError::Config`] when a mutating Allowlist would
    /// violate the current fail-closed process-security boundary.
    pub fn try_new(config: McpServerConfig) -> Result<Self, McpAdapterError> {
        config.validate_process_security()?;
        Ok(Self { config })
    }

    /// Read-only access to the config.
    #[must_use]
    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }

    /// Whether a tool name is allowed by the Allowlist, and its policy.
    #[must_use]
    pub fn lookup_tool(&self, name: &str) -> Option<AllowlistPolicy> {
        self.config.allowlist.get(name).map(|t| t.policy)
    }

    /// Check the Tool-Call Attestation gate for an incoming `tools/call`
    /// request (ADR-0006 Decision 4).
    ///
    /// Returns `None` if the gate passes (attestation present+valid, OR the
    /// policy does not require attestation for this tool class). Returns
    /// `Some(outcome)` on rejection, which the caller surfaces as a tool-level
    /// error result.
    ///
    /// The attestation rides in `_meta.attestation` of the MCP request. It is
    /// deserialized into an [`AttestationInput`]; the signed
    /// [`CanonicalIntent`] is reconstructed from the request and verified
    /// against the caller-supplied public key.
    ///
    /// Note: this verifies the *signature* (origin proof). Whether the public
    /// key is *authorized* is a separate concern. This signature-only helper
    /// is used for read-only policy; mutation goes through
    /// `authorize_mutating_request` and the operator registry instead.
    #[must_use]
    fn check_attestation_gate(
        &self,
        request: &CallToolRequestParams,
        tool_name: &str,
        is_mutate: bool,
    ) -> Option<crate::attestation::AttestationGateOutcome> {
        let policy = self.config.attestation.policy();
        if !policy.requires_for(is_mutate) {
            // Not required: read-only under default policy, or NeverRequired.
            // If an attestation is present we still try to verify it (defense
            // in depth) but a missing one is allowed.
            return self.verify_present_attestation(request, tool_name);
        }
        // Required for this tool class: must be present and valid.
        match extract_attestation(request) {
            Some(Ok(att)) => match self.verify_attestation(request, tool_name, &att) {
                Ok(()) => None,
                Err(e) => Some(crate::attestation::AttestationGateOutcome::Invalid(
                    e.to_string(),
                )),
            },
            Some(Err(error)) => Some(crate::attestation::AttestationGateOutcome::Invalid(
                error.to_string(),
            )),
            None => Some(crate::attestation::AttestationGateOutcome::RequiredMissing),
        }
    }

    /// Verify an attestation when one is present but not required. Missing is
    /// allowed; present-but-invalid is a rejection.
    #[must_use]
    fn verify_present_attestation(
        &self,
        request: &CallToolRequestParams,
        tool_name: &str,
    ) -> Option<crate::attestation::AttestationGateOutcome> {
        match extract_attestation(request) {
            Some(Ok(att)) => match self.verify_attestation(request, tool_name, &att) {
                Ok(()) => None,
                Err(e) => Some(crate::attestation::AttestationGateOutcome::Invalid(
                    e.to_string(),
                )),
            },
            Some(Err(error)) => Some(crate::attestation::AttestationGateOutcome::Invalid(
                error.to_string(),
            )),
            None => None, // allowed when not required
        }
    }

    /// Reconstruct the [`CanonicalIntent`] from the request + attestation and
    /// verify it.
    fn verify_attestation(
        &self,
        request: &CallToolRequestParams,
        tool_name: &str,
        att: &crate::attestation::AttestationInput,
    ) -> Result<(), crate::attestation::AttestationError> {
        let intent = canonical_intent(request, tool_name, att);
        self.config.attestation.verify(&intent, att)
    }

    #[cfg(test)]
    fn authorize_mutating_request(
        &self,
        request: &CallToolRequestParams,
        tool_name: &str,
    ) -> Result<AuthorizedPrincipal, McpAdapterError> {
        let attestation = match extract_attestation(request) {
            Some(Ok(attestation)) => attestation,
            Some(Err(error)) => {
                return Err(McpAdapterError::DeniedByAttestation {
                    tool: tool_name.to_owned(),
                    reason: error.to_string(),
                });
            }
            None => {
                return Err(McpAdapterError::DeniedByAttestation {
                    tool: tool_name.to_owned(),
                    reason: "trusted principal attestation required for mutation".to_owned(),
                });
            }
        };
        let registry = self.config.principal_registry.as_ref().ok_or_else(|| {
            McpAdapterError::DeniedByAttestation {
                tool: tool_name.to_owned(),
                reason: "operator principal registry is not configured".to_owned(),
            }
        })?;
        let intent = canonical_intent(request, tool_name, &attestation);
        registry
            .authorize(
                &self.config.attestation,
                &intent,
                &attestation,
                current_unix_seconds(),
                self.config.max_attestation_age_seconds,
                self.config.max_future_skew_seconds,
            )
            .map_err(|error| McpAdapterError::DeniedByAttestation {
                tool: tool_name.to_owned(),
                reason: error.to_string(),
            })
    }

    /// Invoke a tool as a subprocess and return the captured `CliEnvelope`
    /// JSON.
    ///
    /// This is the adapter core (ADR-0006 Decision 1): map the MCP tool call
    /// to a `forge-core` argv, spawn the subprocess with `--json`, capture
    /// stdout, and return the envelope JSON string. The caller (the `rmcp`
    /// `tools/call` handler, F08.3) parses it into the MCP tool result.
    ///
    /// `argv_tail` is the already-assembled list of CLI flags/positional args
    /// for the command (e.g. `["--operation", "/path/op.yaml", "--root",
    /// "/proj"]`). The adapter appends `--json` and the configured
    /// `--root` so the subprocess always emits a JSON
    /// envelope.
    ///
    /// # Errors
    ///
    /// - [`McpAdapterError::UnknownTool`] — `tool_name` not in the Allowlist.
    /// - [`McpAdapterError::CommandRejected`] — the subprocess exited non-zero
    ///   (the captured envelope JSON is carried for self-correction).
    /// - [`McpAdapterError::Config`] — the subprocess could not be spawned or
    ///   its output read.
    pub fn invoke_tool(
        &self,
        tool_name: &str,
        argv_tail: &[String],
    ) -> Result<String, McpAdapterError> {
        self.config.validate_process_security()?;
        // 1. Allowlist (fail-closed).
        let policy =
            self.lookup_tool(tool_name)
                .ok_or_else(|| McpAdapterError::DeniedByAllowlist {
                    tool: tool_name.to_string(),
                    reason: "tool not in allowlist".into(),
                })?;

        debug_assert!(!policy.is_mutate(), "validated config is read-only");

        // 3. Build argv: ["forge-core", <tool_name>, ...argv_tail, --json,
        //    (--root <path>)?]
        let mut cmd = Command::new(&self.config.forge_core_binary);
        cmd.env_clear();
        copy_minimal_process_environment(&mut cmd);
        let root = self.config.root.as_ref().ok_or_else(|| {
            McpAdapterError::Config("MCP server requires an absolute repo-scoped root".to_owned())
        })?;
        cmd.current_dir(root);
        cmd.arg(tool_name);
        for a in argv_tail {
            cmd.arg(a);
        }
        cmd.arg("--json");
        if let Some(root) = &self.config.root {
            cmd.arg("--root").arg(root);
        }
        // Capture both streams; the envelope is on stdout, diagnostics on
        // stderr. We do NOT inherit stdout — we must parse it.
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd.output().map_err(|e| {
            McpAdapterError::Config(format!(
                "failed to spawn forge-core {}: {e}",
                self.config.forge_core_binary.display()
            ))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

        if !output.status.success() {
            // The envelope on stdout carries the structured exit_reason even
            // on rejection; surface it. If stdout is empty (e.g. binary not
            // found mid-run), synthesize an envelope-shaped error.
            let envelope_json = if stdout.trim().is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                format!(
                    "{{\"ok\":false,\"exit_reason\":\"env_config\",\"error\":{{\"message\":{:?}}}}}",
                    stderr.trim()
                )
            } else {
                stdout.clone()
            };
            let exit_reason = extract_exit_reason(&stdout)
                .unwrap_or_else(|| format!("exit-code-{}", output.status.code().unwrap_or(-1)));
            return Err(McpAdapterError::CommandRejected {
                tool: tool_name.to_string(),
                exit_reason,
                envelope_json,
            });
        }

        Ok(stdout)
    }

    /// Run the stdio JSON-RPC MCP server loop (`rmcp`). Drives `tools/list`
    /// and `tools/call` over stdin/stdout, compatible with MCP clients like
    /// Claude Desktop.
    ///
    /// This consumes `self` (the handler is moved into the `rmcp` runtime).
    /// It drives a tokio current-thread runtime internally so callers do not
    /// need to be inside a tokio context.
    ///
    /// # Errors
    ///
    /// - [`ServerRunError::Config`] — the configured capability surface is
    ///   unsafe to expose over stdio.
    /// - [`ServerRunError::Runtime`] — the tokio runtime could not be built.
    /// - [`ServerRunError::Transport`] — the stdio transport failed to
    ///   initialize or the server loop returned an error.
    pub fn run_stdio(self) -> Result<(), ServerRunError> {
        self.config
            .validate_process_security()
            .map_err(|error| ServerRunError::Config(error.to_string()))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| ServerRunError::Runtime(e.to_string()))?;
        runtime.block_on(async move {
            // `serve` resolves to a RunningService; `.waiting()` blocks until
            // the client disconnects or the transport closes (returns QuitReason).
            let service = self
                .serve(rmcp::transport::stdio())
                .await
                .map_err(|e| ServerRunError::Transport(e.to_string()))?;
            service
                .waiting()
                .await
                .map(|_| ())
                .map_err(|e| ServerRunError::Transport(e.to_string()))
        })
    }
}

/// Map an MCP `arguments` object (a JSON object) to a flat CLI argv list.
///
/// The mapping convention (simple, lossless, agent-friendly):
/// - Each key→value pair becomes `["--<key>", "<value>"]` if the value is a
///   string, or `["--<key>", "<json>"]` for non-string scalars/arrays/objects.
/// - A key whose value is the boolean `true` becomes the bare flag `["--<key>"]`
///   (so `{"--json": true}` → `["--json"]`); `false` is dropped.
/// - Keys are passed through verbatim (callers send `"--root"`, `"--operation"`,
///   ...); we do not synthesize the leading `--`. This keeps the adapter a pure
///   shape transform with no knowledge of any command's flag semantics.
///
/// This is intentionally dumb: the adapter does NOT validate flags against the
/// command's usage — the underlying `forge-core` command does. Invalid flags
/// surface as a rejection envelope from the subprocess.
fn arguments_to_argv(arguments: Option<&JsonObject>) -> Vec<String> {
    let Some(map) = arguments else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(map.len() * 2);
    for (key, value) in map {
        match value {
            serde_json::Value::Bool(true) => out.push(key.clone()),
            serde_json::Value::Bool(false) => {} // dropped
            serde_json::Value::String(s) => {
                out.push(key.clone());
                out.push(s.clone());
            }
            other => {
                out.push(key.clone());
                out.push(other.to_string());
            }
        }
    }
    out
}

fn canonical_intent(
    request: &CallToolRequestParams,
    tool_name: &str,
    attestation: &crate::attestation::AttestationInput,
) -> crate::attestation::CanonicalIntent {
    let arguments = request.arguments.as_ref().map_or(
        serde_json::Value::Object(serde_json::Map::default()),
        |arguments| serde_json::Value::Object(arguments.clone()),
    );
    crate::attestation::CanonicalIntent {
        tool: tool_name.to_owned(),
        arguments,
        credential_id: attestation.credential_id.clone(),
        audience: attestation.audience.clone(),
        execution_intent_digest: attestation.execution_intent_digest.clone(),
        nonce: attestation.nonce.clone(),
        ts: attestation.ts,
    }
}

#[cfg(test)]
fn current_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

fn copy_minimal_process_environment(command: &mut Command) {
    // Enough for Rust/Windows runtime basics and deterministic locale/temp
    // behavior, but intentionally excludes PATH and every provider credential.
    const SAFE_KEYS: &[&str] = &[
        "COMSPEC",
        "HOME",
        "LANG",
        "LC_ALL",
        "SYSTEMROOT",
        "TEMP",
        "TMP",
        "TMPDIR",
        "USERPROFILE",
        "WINDIR",
    ];
    for key in SAFE_KEYS {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
}

/// Extract the Tool-Call Attestation from the MCP request's `_meta.attestation`
/// field (ADR-0006 Decision 4).
///
/// Returns `None` if no attestation is present (caller decides if that is
/// allowed). Returns `Some(Ok(att))` on successful extraction, or
/// `Some(Err(..))` if the field is present but malformed (a present-but-
/// unparseable attestation is a rejection, never silently ignored).
fn extract_attestation(
    request: &CallToolRequestParams,
) -> Option<Result<crate::attestation::AttestationInput, AttestationExtractError>> {
    let meta = request.meta.as_ref()?;
    let att_value = meta.0.get("attestation")?;
    Some(
        serde_json::from_value::<crate::attestation::AttestationInput>(att_value.clone()).map_err(
            |source| AttestationExtractError::Malformed {
                source: source.to_string(),
            },
        ),
    )
}

/// Failures extracting a Tool-Call Attestation from a request's `_meta`.
/// Hand-rolled (no anyhow/thiserror).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestationExtractError {
    /// The `_meta.attestation` field was present but did not deserialize into
    /// an `AttestationInput`.
    Malformed {
        /// The underlying `serde_json` error, as a lossy String.
        source: String,
    },
}

impl std::fmt::Display for AttestationExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed { source } => {
                write!(f, "attestation present but malformed: {source}")
            }
        }
    }
}

impl std::error::Error for AttestationExtractError {}

impl ServerHandler for ForgeMcpServer {
    fn get_info(&self) -> ServerInfo {
        // Advertise Forge as the server; capabilities limited to tools.
        let capabilities = ServerCapabilities::builder().enable_tools().build();
        let mut info = ServerInfo::new(capabilities);
        info.server_info = Implementation::new("forge-core-mcp", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(
            "Forge Method MCP adapter. Tools are pass-throughs over \
             `forge-core` CLI commands. MCP stdio mutation remains disabled \
             until durable replay and late kernel Execution Admission are enforced."
                .into(),
        );
        info
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + MaybeSendFuture + '_ {
        let tools: Vec<Tool> = self
            .config
            .allowlist
            .iter()
            .map(mcp_tool_descriptor)
            .collect();
        std::future::ready(Ok(ListToolsResult::with_all_items(tools)))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(self.handle_call_tool(request))
    }
}

impl ForgeMcpServer {
    /// The synchronous body of `call_tool`, separated so it can be unit-tested
    /// without a live rmcp transport.
    ///
    /// Enforcement order (ADR-0006):
    /// 1. Allowlist lookup (also tells us mutate-ness).
    /// 2. Reject mutation under the active stdio process-security policy.
    /// 3. Apply the configured signature-only policy to a read-only call.
    /// 4. Re-check the Allowlist, then invoke the read-only command.
    #[allow(clippy::needless_pass_by_value)] // trait-adjacent; param by-value matches call_tool
    fn handle_call_tool(
        &self,
        request: CallToolRequestParams,
    ) -> Result<CallToolResult, ErrorData> {
        let tool_name = request.name.as_ref().to_string();

        let Some(policy) = self.lookup_tool(&tool_name) else {
            return Ok(rejection_result(&tool_name, "tool not in allowlist"));
        };
        let is_mutate = policy.is_mutate();
        if is_mutate {
            return Ok(rejection_result(
                &tool_name,
                "mutating MCP stdio is disabled until durable replay and late kernel Execution Admission are enforced",
            ));
        }
        let argv = arguments_to_argv(request.arguments.as_ref());
        // Optional/read-only Tool-Call Attestation gate (ADR-0006 Decision 4).
        if let Some(att_err) = self.check_attestation_gate(&request, &tool_name, false) {
            // Gate denial surfaces as a tool-level error result.
            let (tool, reason) = match att_err {
                crate::attestation::AttestationGateOutcome::RequiredMissing => (
                    tool_name.clone(),
                    "attestation required but none in _meta".to_string(),
                ),
                crate::attestation::AttestationGateOutcome::Invalid(msg) => {
                    (tool_name.clone(), format!("attestation invalid: {msg}"))
                }
            };
            return Ok(rejection_result(&tool, &reason));
        }

        match self.invoke_tool(&tool_name, &argv) {
            Ok(envelope_json) => Ok(CallToolResult::success(vec![ContentBlock::text(
                envelope_json,
            )])),
            // All three gate denials surface identically as a tool-level error
            // carrying the structured rejection payload.
            Err(
                McpAdapterError::DeniedByAllowlist { tool, reason }
                | McpAdapterError::DeniedByMutateGate { tool, reason }
                | McpAdapterError::DeniedByAttestation { tool, reason },
            ) => Ok(rejection_result(&tool, &reason)),
            Err(McpAdapterError::CommandRejected {
                tool: _,
                exit_reason: _,
                envelope_json,
            }) => {
                // The subprocess rejected (non-zero exit). Surface the envelope
                // JSON it emitted (it carries structured self-correction data)
                // and mark the MCP result as an error.
                Ok(CallToolResult::error(vec![ContentBlock::text(
                    envelope_json,
                )]))
            }
            Err(McpAdapterError::UnknownTool(t)) => Err(ErrorData::invalid_request(
                format!("unknown tool: {t}"),
                None,
            )),
            Err(McpAdapterError::ArgumentMapping(m)) => Err(ErrorData::invalid_request(
                format!("argument mapping failed: {m}"),
                None,
            )),
            Err(McpAdapterError::Config(m)) => Err(ErrorData::internal_error(m, None)),
        }
    }
}

/// Build the MCP [`Tool`] descriptor for one allowlisted tool. The
/// `input_schema` is intentionally permissive (an object with no required
/// fields) — the underlying `forge-core` command does the real validation, so
/// the adapter does not duplicate per-command flag schemas here.
fn mcp_tool_descriptor(allowed: &crate::allowlist::AllowedTool) -> Tool {
    let command = command_by_name(&allowed.name);
    let usage = command.map_or_else(
        || format!("forge-core {}", allowed.name),
        |spec| spec.canonical_usage().trim().to_string(),
    );
    let json_mode = command.map_or("unknown-json-mode", |spec| match spec.json_mode {
        JsonMode::EnvelopeOptional => "CliEnvelope JSON/text",
        JsonMode::ProtocolStream => "protocol stream",
    });
    let description: std::borrow::Cow<'static, str> = match allowed.policy {
        AllowlistPolicy::ReadOnly => format!(
            "Forge `{usage}` command (read-only). Pass-through adapter; output mode: {json_mode}."
        )
        .into(),
        AllowlistPolicy::Mutate => format!(
            "Forge `{usage}` command (mutate). MCP stdio invocation is currently \
             blocked pending durable replay and late kernel Execution Admission; \
             output mode: {json_mode}."
        )
        .into(),
    };
    let empty_schema = JsonObject::new();
    Tool::new(allowed.name.clone(), description, Arc::new(empty_schema))
        .with_title(format!("forge-core {}", allowed.name))
}

/// Build a MCP `CallToolResult` for a gate rejection (Allowlist/MutateGate/
/// Attestation). Uses `CallToolResult::error` so `is_error = true` per the
/// MCP spec for tool-side errors that are not protocol-level invalid requests.
fn rejection_result(tool: &str, reason: &str) -> CallToolResult {
    let payload = serde_json::json!({
        "ok": false,
        "exit_reason": "rejected_by_gate",
        "tool": tool,
        "reason": reason,
    });
    CallToolResult::error(vec![ContentBlock::text(payload.to_string())])
}

/// Best-effort extraction of `exit_reason` from an envelope JSON string, for
/// error reporting. Returns `None` if the field is absent or the JSON is
/// malformed — the caller treats that as a generic non-zero exit.
fn extract_exit_reason(envelope_json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(envelope_json.trim()).ok()?;
    v.get("exit_reason")?.as_str().map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny helper: build a config that points at a fake "forge-core"
    /// binary (a script) so `invoke_tool` can be exercised without the real
    /// CLI. The fake echoes a fixed envelope.
    fn config_with_fake_binary(fake_path: PathBuf) -> McpServerConfig {
        let root = fake_path
            .parent()
            .expect("fake binary has parent")
            .to_path_buf();
        McpServerConfig {
            allowlist: Allowlist::default_read_only(),
            attestation: AttestationVerifier::new(AttestationPolicy::Default),
            principal_registry: None,
            max_attestation_age_seconds: DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
            max_future_skew_seconds: DEFAULT_MAX_FUTURE_SKEW_SECONDS,
            forge_core_binary: fake_path,
            root: Some(root),
        }
    }

    #[cfg(unix)]
    fn make_fake_forge_core(success: bool, envelope: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "forge-f08-fake-{}-{}-{}-{}",
            if success { "ok" } else { "fail" },
            std::process::id(),
            n,
            envelope.len()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("forge-core");
        let body = if success {
            format!("#!/bin/sh\necho '{envelope}'")
        } else {
            format!("#!/bin/sh\necho '{envelope}'\nexit 2")
        };
        std::fs::write(&path, body).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    #[cfg(windows)]
    fn make_fake_forge_core(success: bool, envelope: &str) -> PathBuf {
        use std::io::Write;
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        // On Windows, write a .bat shim. Command::new resolves it via PATHEXT.
        let dir = std::env::temp_dir().join(format!(
            "forge-f08-fake-{}-{}-{}-{}",
            if success { "ok" } else { "fail" },
            std::process::id(),
            n,
            envelope.len()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("forge-core.bat");
        let body = if success {
            format!("@echo off\necho {envelope}")
        } else {
            format!("@echo off\necho {envelope}\nexit /b 2")
        };
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        path
    }

    #[test]
    fn invoke_tool_passes_allowlisted_tool_and_captures_envelope() {
        let envelope = r#"{"schema_version":"0.1","command":"preview","ok":true,"exit_reason":"ok","data":{"phase":"1"}}"#;
        let bin = make_fake_forge_core(true, envelope);
        let cfg = McpServerConfig {
            forge_core_binary: bin.clone(),
            ..config_with_fake_binary(bin.clone())
        };
        let server = ForgeMcpServer::new(cfg);
        // "preview" is in the shared Command Surface's default read-only MCP
        // projection.
        let out = server.invoke_tool("preview", &[]).expect("invoke ok");
        assert!(
            out.contains("\"ok\":true"),
            "expected ok envelope, got: {out}"
        );
        assert!(out.contains("preview"));
    }

    #[test]
    fn invoke_tool_denies_unallowlisted_tool() {
        let envelope = "{}";
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(config_with_fake_binary(bin));
        let err = server
            .invoke_tool("definitely-not-allowlisted", &[])
            .unwrap_err();
        assert!(matches!(err, McpAdapterError::DeniedByAllowlist { .. }));
    }

    #[test]
    fn direct_mutating_surface_is_blocked_before_spawn() {
        let envelope = "{}";
        let bin = make_fake_forge_core(true, envelope);
        let cfg = McpServerConfig {
            allowlist: Allowlist::default_with_mutate(),
            ..config_with_fake_binary(bin)
        };
        let server = ForgeMcpServer::new(cfg);
        let err = server.invoke_tool("execute-operation", &[]).unwrap_err();
        assert!(
            matches!(err, McpAdapterError::Config(_)),
            "expected process-security denial, got {err:?}"
        );
    }

    #[test]
    fn direct_mutate_invoke_rejects_without_verified_principal() {
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let cfg = McpServerConfig {
            allowlist: Allowlist::default_with_mutate(),
            ..config_with_fake_binary(bin)
        };
        let server = ForgeMcpServer::new(cfg);
        let argv = vec!["--operation".to_string(), "/tmp/op.yaml".to_string()];
        let error = server
            .invoke_tool("execute-operation", &argv)
            .expect_err("direct mutation must not bypass principal authorization");
        assert!(matches!(error, McpAdapterError::Config(_)));
    }

    #[test]
    fn read_only_server_requires_pinned_binary_and_root() {
        let mut config = McpServerConfig::default_read_only();
        config.forge_core_binary = PathBuf::from("forge-core");
        let relative_binary =
            ForgeMcpServer::try_new(config).expect_err("relative binary path must fail closed");
        assert!(relative_binary.to_string().contains("absolute pinned path"));

        let mut config = McpServerConfig::default_read_only();
        config.root = None;
        let missing_root =
            ForgeMcpServer::try_new(config).expect_err("missing MCP root must fail closed");
        assert!(missing_root.to_string().contains("repo-scoped root"));
    }

    // --- F08.5 attestation gate tests --------------------------------------

    fn mutate_config_with_fake_binary(bin: PathBuf) -> McpServerConfig {
        let root = bin.parent().expect("fake binary has parent").to_path_buf();
        McpServerConfig {
            allowlist: Allowlist::default_with_mutate(),
            attestation: crate::attestation::AttestationVerifier::new(
                crate::attestation::AttestationPolicy::Default,
            ),
            principal_registry: None,
            max_attestation_age_seconds: DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
            max_future_skew_seconds: DEFAULT_MAX_FUTURE_SKEW_SECONDS,
            forge_core_binary: bin,
            root: Some(root),
        }
    }

    fn registered_mutate_config(
        bin: PathBuf,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> McpServerConfig {
        let registry = AuthorizedPrincipalRegistry::from_document(
            crate::principal_registry::PrincipalRegistryDocument {
                schema_version: crate::principal_registry::PRINCIPAL_REGISTRY_SCHEMA_VERSION
                    .to_owned(),
                principal_registry: crate::principal_registry::PrincipalRegistryContract {
                    audience: "forge-core:mcp:stdio:test".to_owned(),
                    principals: vec![crate::principal_registry::PrincipalRegistryEntry {
                        credential_id: "key.codex-main.test".to_owned(),
                        principal_id: forge_core_contracts::PrincipalId(
                            "principal.codex-main".to_owned(),
                        ),
                        agent_id: forge_core_contracts::StableId("codex-main".to_owned()),
                        role: forge_core_contracts::operation::CallerRole::Driver,
                        public_key_hex: crate::attestation::hex_encode(
                            &signing_key.verifying_key().to_bytes(),
                        ),
                        allowed_tools: vec![forge_core_contracts::StableId(
                            "execute-operation".to_owned(),
                        )],
                        authority_grants: vec![forge_core_contracts::StableId(
                            "operation.execute".to_owned(),
                        )],
                        status: crate::principal_registry::PrincipalCredentialStatus::Active,
                    }],
                },
            },
        )
        .expect("test principal registry");
        McpServerConfig {
            principal_registry: Some(registry),
            ..mutate_config_with_fake_binary(bin)
        }
    }

    /// Like `config_with_fake_binary` (read-only allowlist) but with the
    /// hardened `RequireAll` policy: attestation is required for ALL tools,
    /// read-only included.
    fn require_all_config_with_fake_binary(bin: PathBuf) -> McpServerConfig {
        let root = bin.parent().expect("fake binary has parent").to_path_buf();
        McpServerConfig {
            allowlist: Allowlist::default_read_only(),
            attestation: crate::attestation::AttestationVerifier::new(
                crate::attestation::AttestationPolicy::RequireAll,
            ),
            principal_registry: None,
            max_attestation_age_seconds: DEFAULT_MAX_ATTESTATION_AGE_SECONDS,
            max_future_skew_seconds: DEFAULT_MAX_FUTURE_SKEW_SECONDS,
            forge_core_binary: bin,
            root: Some(root),
        }
    }

    /// Sign a Tool-Call Attestation for a tool call, returning the
    /// `AttestationInput` that would ride in `_meta.attestation`.
    fn sign_test_attestation(
        tool: &str,
        arguments: serde_json::Value,
        nonce: &str,
        ts: i64,
    ) -> crate::attestation::AttestationInput {
        use ed25519_dalek::{Signer, SigningKey};
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        let sk = SigningKey::from_bytes(&bytes);
        let intent = crate::attestation::CanonicalIntent {
            tool: tool.into(),
            arguments,
            credential_id: None,
            audience: None,
            execution_intent_digest: None,
            nonce: nonce.into(),
            ts,
        };
        // Test-only signing helper. Infallible-by-construction (intent built
        // from JSON just above); NOT the security boundary. The verification
        // path (server.rs verify_attestation → AttestationVerifier::verify) is
        // fail-closed: a canonicalization failure returns AttestationError and
        // surfaces as a tool-call rejection, never a panic.
        let canon = intent.canonical_bytes().expect("canonicalize test intent");
        let sig = sk.sign(&canon);
        let pk = sk.verifying_key();
        crate::attestation::AttestationInput {
            credential_id: intent.credential_id.clone(),
            audience: intent.audience.clone(),
            execution_intent_digest: intent.execution_intent_digest.clone(),
            nonce: nonce.into(),
            ts,
            signature: crate::attestation::hex_encode(&sig.to_bytes()),
            public_key_hex: crate::attestation::hex_encode(&pk.to_bytes()),
        }
    }

    fn sign_registered_mutation(
        signing_key: &ed25519_dalek::SigningKey,
        arguments: serde_json::Value,
        nonce: &str,
        ts: i64,
    ) -> crate::attestation::AttestationInput {
        use ed25519_dalek::Signer;
        let intent = crate::attestation::CanonicalIntent {
            tool: "execute-operation".to_owned(),
            arguments,
            credential_id: Some("key.codex-main.test".to_owned()),
            audience: Some("forge-core:mcp:stdio:test".to_owned()),
            execution_intent_digest: Some(format!("sha256:{}", "b".repeat(64))),
            nonce: nonce.to_owned(),
            ts,
        };
        let signature = signing_key.sign(&intent.canonical_bytes().expect("canonical intent"));
        crate::attestation::AttestationInput {
            credential_id: intent.credential_id,
            audience: intent.audience,
            execution_intent_digest: intent.execution_intent_digest,
            nonce: intent.nonce,
            ts: intent.ts,
            signature: crate::attestation::hex_encode(&signature.to_bytes()),
            public_key_hex: crate::attestation::hex_encode(&signing_key.verifying_key().to_bytes()),
        }
    }

    /// Build a `CallToolRequestParams` with `_meta.attestation` set.
    #[allow(clippy::needless_pass_by_value)] // test helper; att moved into meta
    fn request_with_attestation(
        tool: &str,
        arguments: JsonObject,
        att: crate::attestation::AttestationInput,
    ) -> CallToolRequestParams {
        use rmcp::model::Meta;
        let mut meta_map = JsonObject::new();
        meta_map.insert("attestation".into(), serde_json::to_value(&att).unwrap());
        let mut req = CallToolRequestParams::new(tool.to_string());
        req.arguments = (!arguments.is_empty()).then_some(arguments);
        req.meta = Some(Meta(meta_map));
        req
    }

    #[test]
    fn stdio_handler_rejects_mutation_before_attestation_or_spawn() {
        let envelope = "{}";
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(mutate_config_with_fake_binary(bin));
        // P4b.1 can derive identity, but the stdio mutation surface remains
        // disabled until replay + late kernel admission are integrated.
        let mut req = CallToolRequestParams::new("execute-operation");
        let mut arguments = JsonObject::new();
        arguments.insert("--operation".into(), serde_json::json!("/tmp/op.yaml"));
        req.arguments = Some(arguments);
        let res = server.handle_call_tool(req).unwrap();
        assert!(res.is_error.unwrap_or(false));
        assert!(content_text(&res).contains("mutating MCP stdio is disabled"));
    }

    #[test]
    fn registry_derives_principal_from_valid_mutating_attestation() {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[11; 32]);
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(registered_mutate_config(bin, &signing_key));
        let mut args = JsonObject::new();
        args.insert("--operation".into(), serde_json::json!("/tmp/op.yaml"));
        let att = sign_registered_mutation(
            &signing_key,
            serde_json::json!({ "--operation": "/tmp/op.yaml" }),
            "nonce-registered-0001",
            current_unix_seconds(),
        );
        let req = request_with_attestation("execute-operation", args, att);
        let principal = server
            .authorize_mutating_request(&req, "execute-operation")
            .expect("registry-authorized principal");
        assert_eq!(principal.principal_id.0, "principal.codex-main");
        assert_eq!(principal.agent_id.0, "codex-main");

        let result = server.handle_call_tool(req).expect("tool result");
        assert!(result.is_error.unwrap_or(false));
        assert!(content_text(&result).contains("mutating MCP stdio is disabled"));
    }

    #[test]
    fn caller_selected_key_cannot_authorize_mutation() {
        let registered_key = ed25519_dalek::SigningKey::from_bytes(&[11; 32]);
        let attacker_key = ed25519_dalek::SigningKey::from_bytes(&[12; 32]);
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(registered_mutate_config(bin, &registered_key));
        let mut args = JsonObject::new();
        args.insert("--operation".into(), serde_json::json!("/tmp/op.yaml"));
        let attestation = sign_registered_mutation(
            &attacker_key,
            serde_json::json!({ "--operation": "/tmp/op.yaml" }),
            "nonce-attacker-key-0001",
            current_unix_seconds(),
        );

        let request = request_with_attestation("execute-operation", args, attestation);
        let error = server
            .authorize_mutating_request(&request, "execute-operation")
            .expect_err("caller-selected key must fail registry authorization");
        assert!(error
            .to_string()
            .contains("caller key does not match credential"));
    }

    #[test]
    fn mutating_server_startup_requires_principal_registry() {
        let bin = make_fake_forge_core(true, "{}");
        let rejection = ForgeMcpServer::try_new(mutate_config_with_fake_binary(bin))
            .expect_err("mutating surface without registry must fail startup");

        assert!(matches!(rejection, McpAdapterError::Config(_)));
    }

    #[test]
    fn mutating_server_startup_remains_disabled_with_registry() {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[11; 32]);
        let bin = make_fake_forge_core(true, "{}");
        let rejection = ForgeMcpServer::try_new(registered_mutate_config(bin, &signing_key))
            .expect_err("P4b.1 identity alone must not enable stdio mutation");

        assert!(matches!(rejection, McpAdapterError::Config(_)));
        assert!(rejection.to_string().contains("durable replay"));
    }

    #[test]
    fn attestation_gate_rejects_mutate_with_tampered_attestation() {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[11; 32]);
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(registered_mutate_config(bin, &signing_key));
        // Sign over one intent but call with different arguments → tampered.
        let mut args = JsonObject::new();
        args.insert("--operation".into(), serde_json::json!("/tmp/actual.yaml"));
        let att = sign_registered_mutation(
            &signing_key,
            serde_json::json!({ "--operation": "/tmp/DIFFERENT.yaml" }),
            "nonce-registered-0002",
            current_unix_seconds(),
        );
        let req = request_with_attestation("execute-operation", args, att);
        let error = server
            .authorize_mutating_request(&req, "execute-operation")
            .expect_err("tampered intent must fail");
        assert!(error.to_string().contains("attestation signature invalid"));
    }

    #[test]
    fn attestation_gate_allows_readonly_without_attestation() {
        // Default policy: read-only tools do not require attestation.
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(McpServerConfig {
            allowlist: Allowlist::default_read_only(),
            ..mutate_config_with_fake_binary(bin)
        });
        let req = CallToolRequestParams::new("preview");
        let res = server.handle_call_tool(req).unwrap();
        assert!(!res.is_error.unwrap_or(true));
    }

    #[test]
    fn never_required_cannot_bypass_mutation_identity() {
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let cfg = McpServerConfig {
            allowlist: Allowlist::default_with_mutate(),
            attestation: crate::attestation::AttestationVerifier::new(
                crate::attestation::AttestationPolicy::NeverRequired,
            ),
            ..mutate_config_with_fake_binary(bin)
        };
        let server = ForgeMcpServer::new(cfg);
        let mut args = JsonObject::new();
        args.insert("--operation".into(), serde_json::json!("/tmp/op.yaml"));
        let mut req = CallToolRequestParams::new("execute-operation");
        req.arguments = Some(args);
        let res = server.handle_call_tool(req).unwrap();
        assert!(res.is_error.unwrap_or(false));
        assert!(content_text(&res).contains("mutating MCP stdio is disabled"));
    }

    // --- security-gap: RequireAll at the gate (no integration test) ---------

    #[test]
    fn require_all_gate_rejects_readonly_without_attestation() {
        // RequireAll: attestation required even for read-only tools. "preview"
        // is read-only (in Allowlist::default_read_only()) but with no
        // _meta.attestation the gate must reject BEFORE the subprocess runs.
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(require_all_config_with_fake_binary(bin));
        let req = CallToolRequestParams::new("preview");
        let res = server.handle_call_tool(req).unwrap();
        assert!(
            res.is_error.unwrap_or(false),
            "RequireAll must reject read-only without attestation, got: {}",
            content_text(&res)
        );
        assert!(content_text(&res).contains("attestation required"));
    }

    #[test]
    fn require_all_gate_passes_readonly_with_valid_attestation() {
        // Symmetric: RequireAll + read-only tool + valid signed attestation
        // → gate passes and the subprocess envelope is returned. Uses the
        // read-only allowlist (includes "preview").
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(require_all_config_with_fake_binary(bin));
        let att = sign_test_attestation("preview", serde_json::json!({}), "n-rall", 1_700_000_000);
        let req = request_with_attestation("preview", JsonObject::new(), att);
        let res = server.handle_call_tool(req).unwrap();
        assert!(
            !res.is_error.unwrap_or(true),
            "RequireAll + valid attestation must pass, got: {}",
            content_text(&res)
        );
    }

    // --- security-gap: present-but-invalid on read-only (verify_present) -----

    #[test]
    fn present_but_invalid_attestation_on_readonly_is_rejected() {
        // Default policy: read-only does not REQUIRE attestation, but if one is
        // present it must verify (defense in depth via verify_present_attestation
        // at server.rs ~L157). Sign a valid attestation, then tamper one byte of
        // the signature hex → the gate must reject even though attestation is
        // optional for read-only.
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(config_with_fake_binary(bin));
        let mut att =
            sign_test_attestation("preview", serde_json::json!({}), "n-tamper", 1_700_000_000);
        // Flip one hex nibble in the signature: take the char at index 0 and
        // toggle its low bit, staying within valid hex digits.
        let mut sig_chars: Vec<char> = att.signature.chars().collect();
        let orig = sig_chars[0];
        sig_chars[0] = if orig == '0' { '1' } else { '0' };
        att.signature = sig_chars.into_iter().collect();
        let req = request_with_attestation("preview", JsonObject::new(), att);
        let res = server.handle_call_tool(req).unwrap();
        assert!(
            res.is_error.unwrap_or(false),
            "tampered attestation on read-only must be rejected (defense in depth), got: {}",
            content_text(&res)
        );
        assert!(content_text(&res).contains("attestation invalid"));
    }

    // --- security-gap: malformed _meta.attestation (extract_attestation) ----

    #[test]
    fn malformed_meta_attestation_is_rejected() {
        // RequireAll policy, read-only tool (so attestation is REQUIRED). Put
        // _meta.attestation as a plain string (not a valid AttestationInput
        // object) → extract_attestation's `Some(Err(msg))` arm (server.rs
        // ~L149/L377) must reject. This exercises the present-but-unparseable
        // path: a malformed attestation is never silently ignored.
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(require_all_config_with_fake_binary(bin));
        let mut meta_map = JsonObject::new();
        // A bare string is not a deserializable AttestationInput.
        meta_map.insert("attestation".into(), serde_json::json!("not valid json"));
        let mut req = CallToolRequestParams::new("preview");
        req.meta = Some(rmcp::model::Meta(meta_map));
        let res = server.handle_call_tool(req).unwrap();
        assert!(
            res.is_error.unwrap_or(false),
            "malformed _meta.attestation must be rejected, got: {}",
            content_text(&res)
        );
        assert!(content_text(&res).contains("attestation present but malformed"));
    }

    #[test]
    fn invoke_tool_surfaces_rejection_envelope() {
        let envelope =
            r#"{"ok":false,"exit_reason":"rejected_by_gate","error":{"message":"nope"}}"#;
        let bin = make_fake_forge_core(false, envelope);
        let server = ForgeMcpServer::new(config_with_fake_binary(bin));
        let err = server.invoke_tool("preview", &[]).unwrap_err();
        match err {
            McpAdapterError::CommandRejected {
                exit_reason,
                envelope_json,
                ..
            } => {
                assert_eq!(exit_reason, "rejected_by_gate");
                assert!(envelope_json.contains("rejected_by_gate"));
            }
            other => panic!("expected CommandRejected, got {other:?}"),
        }
    }

    #[test]
    fn extract_exit_reason_parses_known_field() {
        let r = extract_exit_reason(r#"{"exit_reason":"conflict"}"#);
        assert_eq!(r.as_deref(), Some("conflict"));
    }

    #[test]
    fn extract_exit_reason_none_for_garbage() {
        assert!(extract_exit_reason("not json").is_none());
        assert!(extract_exit_reason(r#"{"no_field":1}"#).is_none());
    }

    #[test]
    fn arguments_to_argv_maps_strings_and_flags() {
        use rmcp::model::JsonObject;
        let mut input = JsonObject::new();
        input.insert("--root".into(), serde_json::json!("/tmp/proj"));
        input.insert("--json".into(), serde_json::json!(true));
        input.insert("--no-sync".into(), serde_json::json!(false));
        input.insert("--count".into(), serde_json::json!(3));
        let result = arguments_to_argv(Some(&input));
        // bool(false) dropped; bool(true) → bare flag; string → pair; number → json string.
        assert!(result.contains(&"--root".to_string()));
        assert!(result.contains(&"/tmp/proj".to_string()));
        assert!(result.contains(&"--json".to_string()));
        assert!(!result.contains(&"--no-sync".to_string()));
        assert!(result.contains(&"--count".to_string()));
        assert!(result.contains(&"3".to_string()));
    }

    #[test]
    fn arguments_to_argv_none_is_empty() {
        assert!(arguments_to_argv(None).is_empty());
    }

    #[test]
    fn handle_call_tool_success_returns_envelope() {
        use rmcp::model::CallToolRequestParams;
        let envelope = r#"{"ok":true,"exit_reason":"ok","data":{"phase":"1"}}"#;
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(config_with_fake_binary(bin));
        let req = CallToolRequestParams::new("preview"); // default read-only MCP projection
                                                         // Test the synchronous handler body directly (no RequestContext needed).
        let res = server.handle_call_tool(req).expect("call ok");
        assert!(!res.is_error.unwrap_or(false));
        assert!(content_text(&res).contains("\"ok\":true"));
    }

    #[test]
    fn handle_call_tool_denies_unallowlisted() {
        use rmcp::model::CallToolRequestParams;
        let bin = make_fake_forge_core(true, "{}");
        let server = ForgeMcpServer::new(config_with_fake_binary(bin));
        let req = CallToolRequestParams::new("definitely-not-allowlisted");
        let res = server
            .handle_call_tool(req)
            .expect("gate denial is Ok(result)");
        // Gate denial surfaces as a tool-level error result (is_error=true),
        // not a protocol Err(ErrorData).
        assert!(res.is_error.unwrap_or(false));
        assert!(content_text(&res).contains("rejected_by_gate"));
    }

    #[test]
    fn handle_call_tool_surfaces_command_rejection() {
        use rmcp::model::CallToolRequestParams;
        let envelope =
            r#"{"ok":false,"exit_reason":"rejected_by_gate","error":{"message":"nope"}}"#;
        let bin = make_fake_forge_core(false, envelope);
        let server = ForgeMcpServer::new(config_with_fake_binary(bin));
        let req = CallToolRequestParams::new("preview");
        let res = server.handle_call_tool(req).expect("Ok even on cmd reject");
        assert!(res.is_error.unwrap_or(false));
        assert!(content_text(&res).contains("rejected_by_gate"));
    }

    #[test]
    fn list_tools_advertises_allowlisted_set() {
        use rmcp::ServerHandler;
        let server = ForgeMcpServer::new(McpServerConfig::default_read_only());
        // get_info must advertise the server (smoke test the ServerHandler impl).
        let info = server.get_info();
        assert_eq!(info.server_info.name, "forge-core-mcp");
    }

    #[test]
    fn mcp_tool_descriptor_projects_command_surface_usage() {
        let tool = mcp_tool_descriptor(&crate::allowlist::AllowedTool {
            name: "start".to_string(),
            policy: AllowlistPolicy::ReadOnly,
        });
        let description = tool
            .description
            .as_ref()
            .expect("tool descriptor carries description");
        assert!(
            description.contains("forge-core start [--root <path>]"),
            "descriptor must include canonical Command Surface usage: {description}"
        );
        assert!(
            description.contains("CliEnvelope JSON/text"),
            "descriptor must include shared JSON mode metadata: {description}"
        );
    }

    fn content_text(result: &rmcp::model::CallToolResult) -> String {
        use rmcp::model::ContentBlock;
        result
            .content
            .iter()
            .map(|c| match c {
                ContentBlock::Text(t) => t.text.as_str(),
                _ => "",
            })
            .collect()
    }
}
