# ADR 0018: CLI JSON And MCP Surfaces

## Status

Accepted

## Context

Forge Core is being redesigned as a Rust Borrowed Shell Runtime. It must work across Codex, Cursor, Claude, OpenCode, VS Code, pi.dev, CLI, and future hosts without letting host-specific prompts or wrappers become method authority.

The first host surface choice affects installation, testability, interoperability, and how quickly hosts can consume the new core. A CLI-only design is simple and universal but may feel second-class for modern agent hosts. An MCP-only design is agent-native but less universal and harder to debug in minimal environments.

## Decision

Forge Core will ship **CLI JSON** and **MCP** as first-class canonical host surfaces from the beginning.

Both surfaces call the same Rust crates, operations, contracts, validators, and authority checks. Neither surface may contain method logic, routing shortcuts, or host-specific behavior that changes Forge semantics.

CLI JSON is the universal/debuggable surface:

- stable commands
- JSON stdin/stdout
- deterministic exit codes
- easy fixture testing
- usable by simple extensions and scripts

MCP is the agent-native surface:

- small macro-tool set
- deferred loading
- structured operation results
- designed for Codex/Cursor/Claude/OpenCode-like clients

## Consequences

The Rust workspace needs separate `forge-cli` and `forge-mcp` crates, both depending on the same operation layer.

Every acceptance test for a core operation must be runnable against the in-process operation layer, CLI JSON, and MCP surface. Host packages may add conformance tests, but they cannot redefine the operation contract.

The first implementation slice should define shared operation and command contracts before either surface grows features.
