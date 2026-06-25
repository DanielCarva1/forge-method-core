# Rust-Only Forge Core System Design

## Purpose

Define the target architecture for rewriting Forge Core as a Rust Borrowed Shell Runtime, not as incremental patches on the Python runtime and not as the standalone Forge app.

The design goal is product excellence:

- rich guided human experience through agents
- compact typed contracts for agents
- runtime authority owned by Forge Core
- host compatibility without Python glue
- installable host packages for borrowed shells
- no Markdown authority
- no monolithic core file

## Non-Negotiables

- Python is not an implementation target for new Forge Core.
- The existing Python runtime is a read-only behavioral reference.
- Host agents are clients, not route authorities.
- Markdown is render/export only.
- Typed YAML/JSON contracts are the source of truth.
- Rust crates own runtime authority, validation, state, gates, lanes, and host operation surfaces.
- If someone claims Rust cannot support a host, they must prove it with a failing prototype or documented platform blocker.
- The standalone app is a separate product that may consume Forge Core later; it is not this runtime shape.

## Lessons From `C:\forge-rust-app`

The Rust app has the right architectural instincts:

- separate `plan/` method state from `app/` product code
- use Cargo workspace crates instead of one large file
- define a shared API boundary before parallel work
- keep Dioxus UI, Axum server, agent loop, providers, store, guidance, gates, and core separate
- expose operation surfaces that can serve multiple frontends
- use Rust/Serde types as contracts
- keep method state file-backed but not prompt-shaped
- make lane ownership and crate boundaries explicit

The core rewrite should adopt the architectural lessons, not the app scope. This repo is the motor that uses borrowed shells. The Python runtime must not remain the acceptance center. It can provide regression examples, but the Rust contracts become the source of truth.

## Target Workspace Shape

```text
forge-core-rs/
  Cargo.toml
  crates/
    forge-core/
    forge-contracts/
    forge-store/
    forge-guidance/
    forge-gates/
    forge-catalog/
    forge-render/
    forge-cli/
    forge-mcp/
    forge-server/
    forge-adapters/
    forge-testkit/
    forge-packages/
  contracts/
    schemas/
    workflows/
    behavior/
    artifacts/
    recovery/
  fixtures/
    guidance/
    state/
    gates/
    recovery/
  docs/
```

## Crate Boundaries

### `forge-core`

Stable domain foundation.

Owns:

- phases
- statuses
- workflow identifiers
- state machine transitions
- route mutation authority
- operation modes
- side-effect policies
- domain errors

Does not own:

- filesystem
- HTTP
- MCP
- model providers
- UI

### `forge-contracts`

Typed public contracts.

Owns:

- `OperationContract`
- `CommandContract`
- `WorkflowContract`
- `BehaviorContract`
- `KnowledgeContract`
- `ArtifactContract`
- `RecoveryContract`
- `StateTransitionContract`
- JSON Schema export
- Serde compatibility tests

This crate is the first implementation slice.

### `forge-store`

File-backed durable state.

Owns:

- `.forge-method/state.yaml`
- sprint/story/input/review files
- artifact index
- evidence records
- ledger append
- optimistic concurrency
- lane claims
- requests
- heartbeat
- cache metadata

### `forge-guidance`

Human-intent routing and behavior selection.

Owns:

- intent classification
- signal detection
- route selection
- operation contract assembly
- behavior contract selection
- no-write observe mode
- correct-course intake versus commit boundary

Does not write state directly.

### `forge-gates`

Validation and release authority.

Owns:

- quality gates
- reality/evidence gates
- grill gates
- ready/release gates
- contract validation
- audit rules
- eval runner

### `forge-catalog`

Capability and workflow registry.

Owns:

- workflow metadata
- product areas/tracks
- behavior contract references
- schema validation for packaged contracts
- compactness checks

### `forge-render`

Render-only output.

Owns:

- Markdown render
- README/install/release-note output
- status summaries

Render output is not runtime authority.

### `forge-cli`

Host-neutral command surface.

Owns:

- CLI commands
- JSON stdin/stdout
- exit codes
- shell-friendly install/update helpers

CLI JSON is canonical from the first release of the Rust core.

### `forge-mcp`

MCP server surface for host agents.

Owns read tools:

- `forge_guide`
- `forge_gate`
- `forge_oracle`
- `forge_doctor`

Owns explicit write tools:

- `forge_apply_transition`
- `forge_record_artifact`
- `forge_record_evidence`
- `forge_record_decision`
- `forge_claim_lane`
- `forge_release_lane`
- `forge_update_heartbeat`

The tool surface is intentionally small and deferred-loading.

MCP is canonical from the first release of the Rust core and must call the same operation layer as CLI JSON.

### `forge-server`

Optional HTTP/SSE surface for hosts that benefit from a local service. This is not the standalone app UI.

Owns:

- Axum routes
- OpenAPI
- SSE streams
- health checks

### `forge-adapters`

Host adapter protocol helpers and examples.

Targets:

- Codex
- Cursor
- Claude
- OpenCode
- VS Code
- pi.dev
- CLI-only

Adapters call the Rust surfaces. They do not contain method logic.

### `forge-packages`

Installable host packages.

Owns:

- Codex plugin package metadata
- Cursor extension package metadata
- OpenCode integration metadata
- VS Code extension metadata
- pi.dev integration metadata
- Claude/Claude Code instructions or extension package when available
- generated thin host instructions

Packages call Rust surfaces and carry no method authority.

### `forge-testkit`

Shared fixtures and assertions.

Owns:

- state fixtures
- contract snapshots
- drift tests
- host adapter conformance tests
- long-chat recovery simulations
- golden operation contracts

## Operation Surface

Forge should expose a small operation set. Read operations are:

```text
guide   classify intent and return operation contract
gate    validate readiness, quality, authority, and evidence
oracle  answer state/context questions without mutation
doctor  diagnose install, host package, version, and contract health
```

Every host uses the same operation semantics through CLI, MCP, HTTP/SSE, or app-internal calls.

The current design hypothesis is that `guide` is the default read-only protocol orchestrator. It routes agents toward `oracle`, `gate`, `doctor`, or an explicit write operation. It must not mutate state directly and must not become a generic "do whatever is next" command.

Host agents should treat `guide` as the normal first operation for substantive human input, stale-chat recovery, ambiguous continuation, correction, frustration, brainstorm, research, planning, or route selection. `oracle`, `gate`, and `doctor` stay available as direct read-only fast paths for explicit state questions, validation checks, and diagnostics.

In Rust, this means `forge-guidance` assembles an `OperationContract` but delegates specialized checks:

- context answers and project explanations to `oracle`
- readiness, authority, quality, and evidence validation to `gate`
- install/version/host/package health to `doctor`
- state mutation to explicit write operations owned by `forge-core` and `forge-store`

Explicit write operations are:

```text
apply_transition  apply an authorized phase, workflow, status, or route transition
record_artifact   create or update artifact metadata under contract
record_evidence   append validation, test, research, or decision evidence
record_decision   persist an explicit human or gate decision
claim_lane        claim a multi-agent work lane
release_lane      release a multi-agent work lane
update_heartbeat  refresh active lane or session liveness
```

There is no generic `advance` operation in the Rust core. Mechanical autonomy is an authorized sequence of explicit write operations, not a broad continuation command.

## Authority Model

All mutation requires explicit authority:

```yaml
authority_source:
  - human_explicit
  - workflow_gate
  - operation_contract
  - state_machine
  - system_maintenance
```

Forbidden:

- host-agent freehand route mutation
- checkpoint suggestion becoming next action
- Markdown heading becoming state authority
- model-generated command running without side-effect policy

## Host Compatibility Strategy

Rust can support hosts through:

- compiled CLI binary with JSON mode
- MCP server
- local HTTP/SSE server
- file-backed contracts
- generated thin host instructions
- signed releases

No host compatibility strategy should require Python.

Each host must have a conformance test that proves it preserves Forge authority instead of reinterpreting it.

## First Implementation Slice

Build `forge-contracts` first.

Must include:

- Rust structs/enums for operation and command contracts
- Serde JSON/YAML roundtrip tests
- JSON Schema export
- golden fixtures for:
  - observe/read-only
  - repair/ask-before-write
  - execute/write-allowed
  - facilitate/ask-one-question
- no Markdown parsing
- no Python dependency

Done when:

- `cargo test -p forge-contracts` passes
- schemas are generated
- fixtures are readable by CLI/app/adapters
- contract examples are small enough for agent context

## Second Implementation Slice

Build `forge-core`.

Must include:

- phase enum
- workflow id type
- state struct
- route mutation policy
- transition validation
- authority validation
- operation-mode rules

Done when:

- illegal transitions are rejected
- mutating operations require authority
- observe mode cannot write
- route changes cannot come from suggestions

## Third Implementation Slice

Build `forge-store`.

Must include:

- state file load/write
- ledger append
- optimistic concurrency
- lane claims
- heartbeat
- requests
- cache metadata

Done when:

- concurrent write conflict is deterministic
- expired lane claims release
- ledger is append-only
- all writes go through authority validation

## Acceptance Criteria

The rewrite is not accepted because it resembles the old runtime. It is accepted when:

- host agents cannot mutate route without contract authority
- guidance can be rich without large prompt documents
- operation contracts are small, typed, and schema-validated
- Codex/Cursor/Claude/OpenCode can consume the same operation surface
- multi-agent lanes are safe under concurrency
- Markdown is never required for runtime decisions
- tests prove drift resistance and recovery from stale chat
- app and plugin surfaces consume the same core contracts
