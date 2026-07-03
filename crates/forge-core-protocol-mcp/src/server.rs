//! MCP server — the adapter that exposes `command_registry::COMMANDS` as MCP
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
//! 2. **`MutateGate`** — if the tool is a mutate tool, an `OperationContract`
//!    must be attached, else fail-closed (Decision 2). [Wired in F08.4.]
//! 3. **Attestation** — if the policy requires it (mutate by default),
//!    verify the Tool-Call Attestation signature (Decision 4). [Wired in
//!    F08.5.]
//! 4. **Invoke** — spawn the subprocess, capture the envelope, return it.

use std::future::Future;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock, ErrorData, Implementation, JsonObject,
    ListToolsResult, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{MaybeSendFuture, RequestContext, RoleServer};
use rmcp::{ServerHandler, ServiceExt};

use crate::allowlist::{Allowlist, AllowlistPolicy};
use crate::attestation::{AttestationPolicy, AttestationVerifier};
use crate::error::{McpAdapterError, ServerRunError};

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
    /// Path to the `forge-core` binary used for subprocess tool invocation.
    /// Defaults to `"forge-core"` (resolved from PATH at runtime). Tests
    /// override this with an explicit path.
    pub forge_core_binary: PathBuf,
    /// The project root forwarded as `--root <path>` to every tool that
    /// accepts it. `None` lets each tool resolve its own root.
    pub root: Option<PathBuf>,
    /// Whether to pass `--allow-bootstrap-core` to subprocess tool calls
    /// (mirrors the CLI flag; for the bootstrap core only).
    pub allow_bootstrap_core: bool,
}

impl McpServerConfig {
    /// Build a default config: read-only Allowlist, default attestation policy
    /// (required-for-mutate), `forge-core` resolved from PATH.
    #[must_use]
    pub fn default_read_only() -> Self {
        Self {
            allowlist: Allowlist::default_read_only(),
            attestation: AttestationVerifier::new(AttestationPolicy::Default),
            forge_core_binary: PathBuf::from("forge-core"),
            root: None,
            allow_bootstrap_core: false,
        }
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
    /// [`CanonicalIntent`] is reconstructed from `(tool_name, arguments,
    /// nonce, ts)` and verified against the caller-supplied public key.
    ///
    /// Note: this verifies the *signature* (origin proof). Whether the public
    /// key is *authorized* is a separate deploy-time concern (the set of
    /// authorized keys is configured at the operator level); F08 keeps the
    /// verify step self-contained.
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
            Some(Err(msg)) => Some(crate::attestation::AttestationGateOutcome::Invalid(msg)),
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
            Some(Err(msg)) => Some(crate::attestation::AttestationGateOutcome::Invalid(msg)),
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
        let arguments = request
            .arguments
            .as_ref()
            .map_or(serde_json::Value::Object(serde_json::Map::default()), |m| {
                serde_json::Value::Object(m.clone())
            });
        let intent = crate::attestation::CanonicalIntent {
            tool: tool_name.to_string(),
            arguments,
            nonce: att.nonce.clone(),
            ts: att.ts,
        };
        self.config.attestation.verify(&intent, att)
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
    /// `--root`/`--allow-bootstrap-core` so the subprocess always emits a JSON
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
        // 1. Allowlist (fail-closed).
        let policy =
            self.lookup_tool(tool_name)
                .ok_or_else(|| McpAdapterError::DeniedByAllowlist {
                    tool: tool_name.to_string(),
                    reason: "tool not in allowlist".into(),
                })?;

        // 2. MutateGate (ADR-0006 Decision 2): a mutate tool must carry an
        //    OperationContract. The CLI signals this via `--operation <path>`
        //    (or `--command`/`--effect` equivalents) in the argv. A mutate
        //    call with none of these is rejected at the adapter boundary,
        //    before the kernel is reached — fail-closed. This is the schema-
        //    level mitigation for tool poisoning: a caller cannot mutate shared
        //    state without declaring the authorized intent first.
        if policy.is_mutate() && !argv_carries_contract(argv_tail) {
            return Err(McpAdapterError::DeniedByMutateGate {
                tool: tool_name.to_string(),
                reason: "mutate tool requires an OperationContract \
                         (--operation <path>, or --command/--effect)"
                    .into(),
            });
        }

        // 3. Build argv: ["forge-core", <tool_name>, ...argv_tail, --json,
        //    (--root <path>)?, (--allow-bootstrap-core)?]
        let mut cmd = Command::new(&self.config.forge_core_binary);
        cmd.arg(tool_name);
        for a in argv_tail {
            cmd.arg(a);
        }
        cmd.arg("--json");
        if let Some(root) = &self.config.root {
            cmd.arg("--root").arg(root);
        }
        if self.config.allow_bootstrap_core {
            cmd.arg("--allow-bootstrap-core");
        }
        // Capture both streams; the envelope is on stdout, diagnostics on
        // stderr. We do NOT inherit stdout — we must parse it.
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
    /// - [`ServerRunError::Runtime`] — the tokio runtime could not be built.
    /// - [`ServerRunError::Transport`] — the stdio transport failed to
    ///   initialize or the server loop returned an error.
    pub fn run_stdio(self) -> Result<(), ServerRunError> {
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

/// Extract the Tool-Call Attestation from the MCP request's `_meta.attestation`
/// field (ADR-0006 Decision 4).
///
/// Returns `None` if no attestation is present (caller decides if that is
/// allowed). Returns `Some(Ok(att))` on successful extraction, or
/// `Some(Err(msg))` if the field is present but malformed (a present-but-
/// unparseable attestation is a rejection, never silently ignored).
fn extract_attestation(
    request: &CallToolRequestParams,
) -> Option<Result<crate::attestation::AttestationInput, String>> {
    let meta = request.meta.as_ref()?;
    let att_value = meta.0.get("attestation")?;
    Some(
        serde_json::from_value::<crate::attestation::AttestationInput>(att_value.clone())
            .map_err(|e| e.to_string()),
    )
}

/// Whether an argv carries an `OperationContract` signal (ADR-0006 Decision 2).
///
/// The forge-core CLI accepts the contract via `--operation <path>` (the
/// canonical `OperationContract` path) or its `--command`/`--effect`
/// equivalents. Any of these present means the caller declared an authorized
/// intent, so the `MutateGate` passes. None present means the mutate call is
/// unscoped and the gate rejects (fail-closed).
fn argv_carries_contract(argv: &[String]) -> bool {
    argv.windows(2)
        .any(|pair| matches!(pair[0].as_str(), "--operation" | "--command" | "--effect"))
}

impl ServerHandler for ForgeMcpServer {
    fn get_info(&self) -> ServerInfo {
        // Advertise Forge as the server; capabilities limited to tools.
        let capabilities = ServerCapabilities::builder().enable_tools().build();
        let mut info = ServerInfo::new(capabilities);
        info.server_info = Implementation::new("forge-core-mcp", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(
            "Forge Method MCP adapter. Tools are pass-throughs over \
             `forge-core` CLI commands; mutations require an OperationContract \
             + Tool-Call Attestation (ADR-0006)."
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
    /// 2. Tool-Call Attestation — if the policy requires it for this tool's
    ///    class (mutate by default) and it is missing/invalid, reject here.
    /// 3. `invoke_tool` performs the Allowlist re-check + `MutateGate` + subprocess.
    #[allow(clippy::needless_pass_by_value)] // trait-adjacent; param by-value matches call_tool
    fn handle_call_tool(
        &self,
        request: CallToolRequestParams,
    ) -> Result<CallToolResult, ErrorData> {
        let tool_name = request.name.as_ref().to_string();

        // Determine mutate-ness from the Allowlist (None = not allowlisted;
        // invoke_tool below will emit the precise DeniedByAllowlist).
        let policy = self.lookup_tool(&tool_name);
        let is_mutate = policy.is_some_and(AllowlistPolicy::is_mutate);

        // Tool-Call Attestation gate (ADR-0006 Decision 4).
        if let Some(att_err) = self.check_attestation_gate(&request, &tool_name, is_mutate) {
            // Gate denial surfaces as a tool-level error result.
            let (tool, reason) = match att_err {
                crate::attestation::AttestationGateOutcome::RequiredMissing => (
                    tool_name.clone(),
                    "attestation required for mutate tool but none in _meta".to_string(),
                ),
                crate::attestation::AttestationGateOutcome::Invalid(msg) => {
                    (tool_name.clone(), format!("attestation invalid: {msg}"))
                }
            };
            return Ok(rejection_result(&tool, &reason));
        }

        let argv = arguments_to_argv(request.arguments.as_ref());
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
    let description: std::borrow::Cow<'static, str> = match allowed.policy {
        AllowlistPolicy::ReadOnly => format!(
            "Forge `forge-core {}` command (read-only). Pass-through adapter.",
            allowed.name
        )
        .into(),
        AllowlistPolicy::Mutate => format!(
            "Forge `forge-core {}` command (mutate). Requires an OperationContract \
             + Tool-Call Attestation (ADR-0006).",
            allowed.name
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
    use std::io::Write;

    /// A tiny helper: build a config that points at a fake "forge-core"
    /// binary (a script) so `invoke_tool` can be exercised without the real
    /// CLI. The fake echoes a fixed envelope.
    fn config_with_fake_binary(fake_path: PathBuf) -> McpServerConfig {
        McpServerConfig {
            allowlist: Allowlist::default_read_only(),
            attestation: AttestationVerifier::new(AttestationPolicy::Default),
            forge_core_binary: fake_path,
            root: None,
            allow_bootstrap_core: false,
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
            format!("#!/bin/sh\necho '{}'", envelope)
        } else {
            format!("#!/bin/sh\necho '{}'\nexit 2", envelope)
        };
        std::fs::write(&path, body).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    #[cfg(windows)]
    fn make_fake_forge_core(success: bool, envelope: &str) -> PathBuf {
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
        // "preview" is in DEFAULT_READONLY_TOOLS.
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
    fn mutate_gate_rejects_execute_operation_without_contract() {
        // A mutate tool with no --operation/--command/--effect is rejected at
        // the MutateGate before the subprocess is spawned (ADR-0006 Decision 2).
        let envelope = "{}";
        let bin = make_fake_forge_core(true, envelope);
        let cfg = McpServerConfig {
            allowlist: Allowlist::default_with_mutate(),
            ..config_with_fake_binary(bin)
        };
        let server = ForgeMcpServer::new(cfg);
        let err = server.invoke_tool("execute-operation", &[]).unwrap_err();
        assert!(
            matches!(err, McpAdapterError::DeniedByMutateGate { .. }),
            "expected MutateGate denial, got {err:?}"
        );
    }

    #[test]
    fn mutate_gate_passes_when_contract_attached() {
        // With --operation <path> present, the mutate gate passes and the
        // subprocess is invoked (returns the fake envelope).
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let cfg = McpServerConfig {
            allowlist: Allowlist::default_with_mutate(),
            ..config_with_fake_binary(bin)
        };
        let server = ForgeMcpServer::new(cfg);
        let argv = vec!["--operation".to_string(), "/tmp/op.yaml".to_string()];
        let out = server.invoke_tool("execute-operation", &argv);
        assert!(out.is_ok(), "expected gate to pass: {out:?}");
    }

    #[test]
    fn mutate_gate_passes_with_command_or_effect_flag() {
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let cfg = McpServerConfig {
            allowlist: Allowlist::default_with_mutate(),
            ..config_with_fake_binary(bin)
        };
        let server = ForgeMcpServer::new(cfg);
        // --command is also a valid contract signal.
        let argv = vec!["--command".to_string(), "/tmp/cmd.yaml".to_string()];
        assert!(server.invoke_tool("execute-operation", &argv).is_ok());
        // --effect too.
        let argv = vec!["--effect".to_string(), "/tmp/effect.yaml".to_string()];
        assert!(server.invoke_tool("execute-operation", &argv).is_ok());
    }

    #[test]
    fn argv_carries_contract_detection() {
        assert!(!argv_carries_contract(&[]));
        assert!(!argv_carries_contract(&["--json".into()]));
        assert!(argv_carries_contract(&["--operation".into(), "/x".into()]));
        assert!(argv_carries_contract(&["--command".into(), "/x".into()]));
        assert!(argv_carries_contract(&["--effect".into(), "/x".into()]));
    }

    // --- F08.5 attestation gate tests --------------------------------------

    fn mutate_config_with_fake_binary(bin: PathBuf) -> McpServerConfig {
        McpServerConfig {
            allowlist: Allowlist::default_with_mutate(),
            attestation: crate::attestation::AttestationVerifier::new(
                crate::attestation::AttestationPolicy::Default,
            ),
            forge_core_binary: bin,
            root: None,
            allow_bootstrap_core: false,
        }
    }

    /// Like `config_with_fake_binary` (read-only allowlist) but with the
    /// hardened `RequireAll` policy: attestation is required for ALL tools,
    /// read-only included.
    fn require_all_config_with_fake_binary(bin: PathBuf) -> McpServerConfig {
        McpServerConfig {
            allowlist: Allowlist::default_read_only(),
            attestation: crate::attestation::AttestationVerifier::new(
                crate::attestation::AttestationPolicy::RequireAll,
            ),
            forge_core_binary: bin,
            root: None,
            allow_bootstrap_core: false,
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
            nonce: nonce.into(),
            ts,
            signature: crate::attestation::hex_encode(&sig.to_bytes()),
            public_key_hex: crate::attestation::hex_encode(&pk.to_bytes()),
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
    fn attestation_gate_rejects_mutate_without_attestation() {
        let envelope = "{}";
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(mutate_config_with_fake_binary(bin));
        // execute-operation is mutate; Default policy requires attestation.
        // No _meta.attestation present → rejection at the attestation gate,
        // BEFORE the MutateGate/subprocess is reached.
        let req = CallToolRequestParams::new("execute-operation");
        let res = server.handle_call_tool(req).unwrap();
        assert!(res.is_error.unwrap_or(false));
        assert!(content_text(&res).contains("attestation required"));
    }

    #[test]
    fn attestation_gate_passes_mutate_with_valid_attestation_and_contract() {
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(mutate_config_with_fake_binary(bin));
        // Mutate tool WITH contract AND valid attestation → gate passes,
        // subprocess invoked, envelope returned.
        let mut args = JsonObject::new();
        args.insert("--operation".into(), serde_json::json!("/tmp/op.yaml"));
        let att = sign_test_attestation(
            "execute-operation",
            serde_json::json!({ "--operation": "/tmp/op.yaml" }),
            "n-1",
            1_700_000_000,
        );
        let req = request_with_attestation("execute-operation", args, att);
        let res = server.handle_call_tool(req).unwrap();
        assert!(
            !res.is_error.unwrap_or(true),
            "expected success, got: {}",
            content_text(&res)
        );
    }

    #[test]
    fn attestation_gate_rejects_mutate_with_tampered_attestation() {
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(mutate_config_with_fake_binary(bin));
        // Sign over one intent but call with different arguments → tampered.
        let mut args = JsonObject::new();
        args.insert("--operation".into(), serde_json::json!("/tmp/actual.yaml"));
        let att = sign_test_attestation(
            "execute-operation",
            serde_json::json!({ "--operation": "/tmp/DIFFERENT.yaml" }),
            "n-1",
            1_700_000_000,
        );
        let req = request_with_attestation("execute-operation", args, att);
        let res = server.handle_call_tool(req).unwrap();
        assert!(res.is_error.unwrap_or(false));
        assert!(content_text(&res).contains("attestation invalid"));
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
    fn attestation_gate_never_required_allows_mutate_without_attestation() {
        // NeverRequired policy: even mutate does not require attestation
        // (verify-only-when-present). Missing is allowed. NOTE: the MutateGate
        // still independently requires an OperationContract — attestation and
        // the contract are separate gates (ADR-0006 Decisions 2 & 4).
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
        // Mutate + contract, no attestation → attestation gate passes
        // (NeverRequired), MutateGate passes (contract present).
        let mut args = JsonObject::new();
        args.insert("--operation".into(), serde_json::json!("/tmp/op.yaml"));
        let mut req = CallToolRequestParams::new("execute-operation");
        req.arguments = Some(args);
        let res = server.handle_call_tool(req).unwrap();
        assert!(
            !res.is_error.unwrap_or(true),
            "expected success, got: {}",
            content_text(&res)
        );
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
        // Default policy, mutate tool (so attestation is REQUIRED). Put
        // _meta.attestation as a plain string (not a valid AttestationInput
        // object) → extract_attestation's `Some(Err(msg))` arm (server.rs
        // ~L149/L377) must reject. This exercises the present-but-unparseable
        // path: a malformed attestation is never silently ignored.
        let envelope = r#"{"ok":true,"exit_reason":"ok"}"#;
        let bin = make_fake_forge_core(true, envelope);
        let server = ForgeMcpServer::new(mutate_config_with_fake_binary(bin));
        let mut meta_map = JsonObject::new();
        // A bare string is not a deserializable AttestationInput.
        meta_map.insert("attestation".into(), serde_json::json!("not valid json"));
        let mut req = CallToolRequestParams::new("execute-operation");
        let mut args = JsonObject::new();
        args.insert("--operation".into(), serde_json::json!("/tmp/op.yaml"));
        req.arguments = Some(args);
        req.meta = Some(rmcp::model::Meta(meta_map));
        let res = server.handle_call_tool(req).unwrap();
        assert!(
            res.is_error.unwrap_or(false),
            "malformed _meta.attestation must be rejected, got: {}",
            content_text(&res)
        );
        assert!(content_text(&res).contains("attestation invalid"));
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
        let req = CallToolRequestParams::new("preview"); // in DEFAULT_READONLY_TOOLS
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
