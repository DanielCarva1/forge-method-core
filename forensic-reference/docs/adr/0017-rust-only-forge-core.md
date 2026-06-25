# ADR 0017: Rust-Only Forge Core

## Status

Accepted

## Context

Forge Method Core grew from a Python prototype into a large runtime with state, guidance, workflows, gates, artifacts, update logic, lanes, and tests. That path optimized for patch speed inside Codex, but it also created a monolithic script, prompt-shaped documents, weak type boundaries, and repeated drift between intended method authority and host-agent behavior.

The product goal is no longer "make the existing plugin easier to patch." The product goal is an excellent Forge runtime and future standalone app that can work across Codex, Cursor, Claude, OpenCode, VS Code, CLI, and desktop without treating any host agent as the authority source.

The existing Rust app at `C:\forge-rust-app` demonstrates the healthier direction: explicit crate boundaries, a shared API contract crate, Rust/Serde types, Dioxus/Axum separation, app/server/core boundaries, and method state as a sidecar rather than a pile of prompt documents.

## Decision

New Forge Core implementation work is Rust-only.

Python is not an allowed implementation target for the new Forge Core. This includes:

- state machine authority
- operation contracts
- command contracts
- guidance routing
- gates and validation
- artifact/recovery contracts
- lanes, locks, claims, heartbeat, and multi-agent coordination
- ledger/store behavior
- host operation surfaces
- schema validation

The existing Python runtime may be read as a behavioral reference while the Rust design is written, but it must not be expanded as the future core and must not become a required compatibility layer for the new architecture.

Host compatibility must be solved through Rust-native surfaces such as CLI, JSON stdin/stdout, MCP, HTTP/SSE, library crates, signed binaries, or app integrations. A claim that Rust cannot support a target host requires concrete proof.

## Consequences

Architecture work must start from the final Rust system design, not from incremental Python patches.

The new core should be organized around crates and contracts, not a single growing runtime file. A likely boundary model is:

- `forge-core`: phases, state machine, IDs, domain types, runtime authority.
- `forge-contracts` or `forge-api`: operation, command, workflow, behavior, artifact, recovery, and host adapter contracts.
- `forge-store`: state files, ledger, optimistic concurrency, lanes, claims, requests, cache.
- `forge-guidance`: intent signals, routing, operation contract assembly, behavior contract selection.
- `forge-gates`: quality, audit, grill, ready, evidence, transition gates.
- `forge-catalog`: workflow and capability catalog.
- `forge-render`: generated Markdown/release/readme/export surfaces.
- `forge-cli`: host-neutral command surface.
- `forge-mcp`: MCP/tool surface.
- `forge-server`: optional HTTP/SSE surface for app and host integrations.

Markdown remains non-authoritative. Typed contracts are the source of truth.

The current Python code should be frozen except for emergency user-impacting fixes while the Rust rewrite plan is created. Any additional Python feature work requires an explicit exception ADR.
