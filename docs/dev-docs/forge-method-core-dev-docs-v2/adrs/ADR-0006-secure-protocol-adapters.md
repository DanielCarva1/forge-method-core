# ADR-0006 - MCP and A2A as secure adapters

- **Status**: Accepted (2026-07-02; expanded from `proposto`)

## Context

The Forge Method core is a governance layer over shared agentic state: it
serializes writes by path (lane claims, ADR 0023), resolves principal
conflicts as structured objects (F07, ADR 0007), and keeps the kernel as the
single source of truth for mutation. But the core is only useful if external
agents can call it — and the agent ecosystem in 2025-26 converged on two de
facto protocols:

- **MCP (Model Context Protocol, Anthropic)** — JSON-RPC over stdio; this is
  the channel through which clients like Claude Desktop discover and invoke
  tools.
- **A2A (Agent-to-Agent)** — interoperability between agents from different
  vendors.

Exposing the Forge core through these protocols is the "Features community"
front (it closes the last pending P1 feature — `followups_v0_1_to_10.md:23`).
Without it, Forge is an island: agents can only use it via CLI subprocess,
losing the tool discovery and composition that the MCP ecosystem offers.

The problem is that open protocols are, by construction, **surfaces for
tool poisoning and capability leakage**. The tau-bench (tool-call evaluation)
documents that MCP clients trust tools/list and tools/call without separating
"the server can do X" from "this caller may request X"; the "Tool Poisoning"
paper (Kolahal et al., 2025) shows that malicious tools hide in metadata
(`_meta`) that the client renders without a sandbox. Adding MCP to Forge
without discipline re-introduces, through the back door, exactly the classes
of bug that ADR 0023 (memory trust) and ADR 0007 (governance) made
unrepresentable: authority without provenance, mutation without declared
intent, anonymous callers mutating shared state.

This ADR formalizes the design of F08 (expanding the original stub). The
central principle is the same as ADR 0024 (PDP/PEP): the protocol surface is a
dumb PEP; every authorization and mutation decision lives in the kernel.

## Decision

### 1. Adapters are not a source of truth and do not mutate the store directly

The MCP server (crate `forge-core-protocol-mcp`) is an **adapter** over the
existing `command_registry::COMMANDS`
(`crates/forge-core-cli/src/command_registry.rs:68`), not a second
implementation of the engine. Each MCP tool is a pass-through wrapper:

1. receives `(tool_name, arguments)` from the MCP client;
2. maps it to an argv `&[String]` in the format that the corresponding
   `CommandSpec::handler` already accepts;
3. invokes the handler and captures the `CliEnvelope` JSON it emits on stdout
   (`crates/forge-core-contracts/src/envelope.rs:77`);
4. returns the envelope as the tool result.

No domain logic lives in the adapter. The adapter earns its keep in the
**deletion test** (Ousterhout): removing it costs callers programmatic access
over stdio JSON-RPC, but costs no functionality — the underlying commands
remain available via the CLI. The adapter is deep because it concentrates the
coupling to `rmcp` (Rust MCP SDK) in a single seam; without it, the coupling
would spread across every command handler.

**Rejected: implementing the engine inside the adapter.** This would duplicate
the logic of every command and break the deletion test (removing the adapter
would destroy functionality). It is exactly the anti-pattern that ADR 0024
fights: a PEP with PDP logic.

### 2. Every mutation goes through the kernel and an OperationContract

The inviolable principle (from the original stub, kept): **the adapter does
not mutate the store directly.** Every mutation flows through the kernel
(`execute-operation`, `claim acquire`) and carries an `OperationContract` that
declares the authorized intent. The adapter merely forwards; the kernel
remains the only PDP for mutation, consistent with ADR 0023/0024.

This solves the tool poisoning vector at the schema level: a malicious MCP
client that requests `execute-operation` without an `OperationContract` is
rejected at the **MutateGate** (the enforcement point at the adapter boundary,
see term in `CONTEXT.md`) before the kernel is ever reached. Fail-closed.

**Rejected: trusting the client to validate authority.** The tau-bench shows
that MCP clients are not reliable PDPs — they render tool metadata without a
sandbox. Authority must be verified on the Forge side, never delegated to the
caller.

### 3. Allowlist = the capability surface

The set of MCPTools that a server instance exposes is declared explicitly in
`mcp-allowlist.yaml` (Allowlist, see `CONTEXT.md`). A tool absent from the
Allowlist is invisible in `tools/list` and rejected in `tools/call` —
fail-closed. The Allowlist separates "Forge can do X" from "this MCP client
may request X": it is the capability boundary.

The Allowlist is **data, not code** (mirrors the `risk-audit-v0` risk-audit
model in `CONTEXT.md:25`): adding a tool to a server does not require a Rust
change. Declaring an empty or restricted Allowlist is the safe default state.

**Rejected: exposing all commands by default.** This breaks the principle of
least privilege and makes the MCP server a full mirror of the CLI — the attack
surface grows unnecessarily. The default must be as restrictive as possible.

### 4. Attestation (signed tool calls) — the caller identity model

stdio JSON-RPC carries no HTTP headers; there is no `Authorization:`. Proof of
*who called* must come in the request body. Decision: each `tools/call`
carries a **Tool-Call Attestation** — a detached ed25519 signature over the
canonical form of the tool-call intent:

```
canonical = serde_json_canonicalizer::canon({
  "tool": <tool_name>,
  "arguments": <arguments_object>,
  "nonce": <opaque>,
  "ts": <unix_seconds>
})
sig = ed25519.sign(caller_private_key, canonical)
```

carried in the `_meta.attestation` field of the JSON-RPC message (the field
that the MCP spec reserves for extensions). The adapter verifies the signature
against an authorized public key configured on the server (reuses
`forge-core-crypto`, which already pinned `ed25519-dalek 2.2` in the workspace
`Cargo.toml:36`).

**Default policy** (a hard-to-reverse decision, recorded here):

- **Mutate MCPTools** (`execute-operation`, `claim acquire`): Tool-Call
  Attestation **mandatory**. No valid signature = rejected at the MutateGate.
- **Read-only MCPTools** (`preview`, `ready`, `graph`, `explain`,
  `memory list`, `query-effect-index`): attestation **optional** under the
  default policy (the server may harden via configuration).

The signature proves origin (who); the `OperationContract` proves authorized
intent (what). Both are required for a mutation — neither alone is sufficient.
This is the MCP/stdio analog of a signed HTTP request (`Signature:` header of
Sigstore / HTTP Signatures), transposed to a transport without headers.

**Rejected: bearer token in `_meta`.** Tokens over local stdio are either
public (no value) or shared secrets (phishing/leakage). A detached signature
proves possession of the private key without revealing it, and binds to the
intent (not replayable by another tool). Reusing `forge-core-crypto` keeps
zero new deps.

**Rejected: mandatory attestation for everything (including read-only).**
Excessive hardening breaks the primary use case (Claude Desktop reading Forge
state without per-call crypto setup). The door is open to harden via config;
the default follows the principle of least friction on the non-mutating axis.

### 5. Reuse `rmcp` (Rust MCP SDK), do not hand-roll JSON-RPC

The adapter uses `rmcp` (`docs.rs/rmcp`, version 1.7) for JSON-RPC transport
over stdio. Hand-rolling JSON-RPC was rejected: `rmcp` already maps the
`#[tool]` macro to expose a fn as a tool and serializes params/results
automatically — it is the mechanism behind `tools/list` and `tools/call`. The
workspace already pinned `tokio` (`Cargo.toml:64`,
`features = ["rt","time"]`); the adapter locally adds `io-util` + `macros`
(required for a stdio server) without touching the workspace pin that other
crates depend on.

## Consequences

- **Interoperability without surrendering authority.** External agents
  (Claude Desktop, etc.) discover and invoke Forge via MCP, but the source of
  truth stays in the kernel. The adapter is a PEP, not a PDP (ADR 0024).
- **Tool poisoning mitigated by design.** A malicious tool hiding in `_meta`
  gains no mutation without `OperationContract` + attestation; an anonymous
  caller is rejected at the MutateGate. The protocol attack surface is not the
  store mutation surface.
- **Trace and audit consistent.** Every mutating MCP call goes through the
  same kernel as the CLI, so it generates the same WAL/telemetry trail. There
  is no divergent "MCP path" and "CLI path".
- **Capability is data.** The Allowlist makes the per-instance tool surface a
  deployment decision, not a code decision. A server restricted to read-only
  is one line of YAML.
- **Isolated coupling.** The `rmcp`/tokio-stdio dependency lives in a single
  crate (`forge-core-protocol-mcp`); the rest of the workspace does not see it.
- **Cost: crypto friction in the mutate case.** Requiring attestation on mutate
  is more setup than read-only; accepted as a security trade-off. The door to
  relax it (configurable policy) exists, but the default is fail-closed.

## Scope of this story (F08.1-F08.7)

- ✅ F08.1: this ADR (Accepted) + terms in `CONTEXT.md` (Secure Protocol
  Adapters, MCPTool, Allowlist, MutateGate, Tool-Call Attestation).
- ⏳ F08.2: create crate `forge-core-protocol-mcp` (`lib/server/allowlist/
  attestation.rs`); pin `rmcp` in `[workspace.dependencies]`.
- ⏳ F08.3: MCP server over `COMMANDS` (read-only: preview/ready/graph/
  explain/memory list/query-effect-index; mutate: execute-operation/
  claim acquire).
- ⏳ F08.4: Allowlist enforcement (`mcp-allowlist.yaml`); MutateGate
  fail-closed without `OperationContract`; validator with typed diagnostics.
- ⏳ F08.5: Tool-Call Attestation (verify ed25519 via `forge-core-crypto`);
  mandatory mutate, optional read-only.
- ⏳ F08.6: CLI `forge-core mcp serve [--allowlist <yaml>]`; register in
  `command_registry::COMMANDS`.
- ⏳ F08.7: fixtures + E2E (Allowlist deny / mutate without contract /
  read-only without attestation); anchor 122 preserved.

## References

- MCP (Model Context Protocol, Anthropic):
  https://modelcontextprotocol.io/specification
- tau-bench (tool-call evaluation, Anthropic 2025):
  https://github.com/anthropics/tau-bench
- Kolahal et al. — Tool Poisoning (2025, arXiv 2506.09566):
  https://arxiv.org/abs/2506.09566
- HTTP Signatures (W3C draft) — signed request precedent:
  https://datatracker.ietf.org/doc/draft-ietf-httpbis-message-signatures/
- Sigstore (cosign `--signature` detached) — detached sig precedent:
  https://docs.sigstore.dev/cosign/sign/overview/
- shuttle.dev — How to build a stdio MCP server in Rust (tutorial, 2025):
  https://shuttle.dev/blog/2025/07/18/how-to-build-a-stdio-mcp-server-in-rust
- rmcp (Rust MCP SDK): https://docs.rs/rmcp
- In-repo: ADR 0023 (memory trust model), ADR 0024 (PDP/PEP),
  `command_registry.rs:68` (the adapter seam), `envelope.rs:77`
  (`CliEnvelope` — the return type of each tool),
  `CONTEXT.md` (F08 terms: MCPTool, Allowlist, MutateGate, Tool-Call
  Attestation).
