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
//!
//! # F08.2 scope
//!
//! The config types + the subprocess `invoke_tool` core (so the crate is
//! immediately exercisable and F08.3 has the adapter logic ready). The
//! `rmcp` stdio server loop (`run_stdio`) is stubbed — F08.3 wires the
//! `tools/list` + `tools/call` handlers; F08.6 wires the CLI entrypoint.

use std::path::PathBuf;
use std::process::Command;

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
        // Keep policy used so the lint stays quiet and the gate is visible.
        let _ = policy;

        // 2. Build argv: ["forge-core", <tool_name>, ...argv_tail, --json,
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
    /// and `tools/call` over stdin/stdout.
    ///
    /// F08.3/F08.6 wire this up. The stub here returns a not-implemented
    /// transport error so the crate compiles and the config/invoke paths are
    /// exercisable without a live `rmcp` loop.
    ///
    /// # Errors
    ///
    /// Returns [`ServerRunError::Transport`] until F08.3 implements the loop.
    pub fn run_stdio(&self) -> Result<(), ServerRunError> {
        // F08.3: tokio + rmcp ServerHandler serving tools/list + tools/call.
        Err(ServerRunError::Transport(
            "stdio MCP loop not implemented until F08.3".into(),
        ))
    }
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
    fn run_stdio_stub_returns_not_implemented() {
        let server = ForgeMcpServer::new(McpServerConfig::default_read_only());
        let err = server.run_stdio().unwrap_err();
        assert!(matches!(err, ServerRunError::Transport(_)));
    }
}
