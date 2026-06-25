# Borrowed Shell Runtime Requirements

## Purpose

Define requirements for the Rust Forge Core as an installable engine that works inside borrowed shells: Codex, Cursor, Claude, OpenCode, VS Code, pi.dev, CLI, and other host tools.

This is not the standalone Forge app. The standalone app can consume the same core later, but this product shape is the engine plus host packages.

## Product Boundary

Forge owns:

- method state
- typed contracts
- guidance routing
- behavior contracts
- gates
- transition authority
- artifact/recovery authority
- lanes and multi-agent coordination
- operation surfaces
- install/update/doctor behavior

The host owns:

- chat UI
- editor UI
- host-native tool execution
- host account/session model
- host extension marketplace rules

The host must not own Forge method decisions.

## Required Host Targets

Initial compatibility targets:

- Codex
- Cursor
- Claude/Claude Code
- OpenCode
- VS Code
- pi.dev
- CLI-only

Each target needs a documented install path, invocation path, update path, and conformance test.

## Required Operation Surface

Forge Core should expose a small stable operation set split between read operations and explicit write operations:

```yaml
operations:
  guide:
    purpose: orchestrate the next safe protocol step from human input, state, gates, and contracts
    mutation: false
  gate:
    purpose: validate contracts, state, evidence, readiness, and authority
    mutation: false
  oracle:
    purpose: answer questions about current state/context without mutation
    mutation: false
  doctor:
    purpose: diagnose install, host package, version, permissions, and contract health
    mutation: false
  update:
    purpose: update host package/core binary through the host-supported path
    mutation: system-only
  apply_transition:
    purpose: apply an authorized phase, workflow, status, or route transition
    mutation: gated
  record_artifact:
    purpose: create or update artifact metadata under an authorized contract
    mutation: gated
  record_evidence:
    purpose: append validation, test, research, or decision evidence
    mutation: gated
  record_decision:
    purpose: persist an explicit human or gate decision
    mutation: gated
  claim_lane:
    purpose: claim a multi-agent work lane
    mutation: gated
  release_lane:
    purpose: release a multi-agent work lane
    mutation: gated
  update_heartbeat:
    purpose: refresh active lane or session liveness
    mutation: gated
```

The exact explicit write operation names are subject to implementation design, but the core must not expose a generic `advance` operation. Every host must consume the same semantics.

`guide` is the default intelligent entry operation for agents. It is read-only and may recommend `oracle`, `gate`, `doctor`, or a write operation, but it does not mutate state by itself.

The current design hypothesis is that `guide` should be the read-only protocol orchestrator. It should not replace `oracle`, `gate`, or `doctor`; it decides when one of them is the correct next operation.

For normal host-agent usage, `guide` is called before the agent decides, mutates, or continues from ambiguous context. Direct calls to `oracle`, `gate`, and `doctor` are allowed fast paths only when the human or host explicitly asks for state/context, validation, or diagnostics.

`guide` returns an Operation Contract. It must include at minimum:

- autonomy mode
- recommended next operation
- allowed actions
- forbidden actions
- side-effect policy
- state mutation policy
- authority source
- required human input flag
- human-facing prompt or status summary
- stop conditions
- exact next contracts/files to load

## Canonical Host Surfaces

Forge ships two canonical Host Surfaces from the start:

- CLI JSON
- MCP

CLI JSON is the universal debug and extension surface. MCP is the agent-native surface. Both call the same Rust operation layer and must produce equivalent contracts for equivalent inputs.

## Install Requirements

Forge must support:

- signed or checksummed Rust binaries
- host package manifests
- no Python dependency
- Windows/macOS/Linux install paths
- version detection
- doctor diagnostics
- rollback or repair guidance
- clear update summary
- offline-friendly local project state

## Contract Requirements

All host surfaces must consume typed contracts:

- Operation Contract
- Command Contract
- Workflow Contract
- Behavior Contract
- Artifact Contract
- Recovery Contract
- State Transition Contract
- Host Package Contract

Markdown may be generated for README, installation instructions, and release notes. Temporary ADR/design Markdown is allowed only before the equivalent runtime contract exists. Markdown is not runtime authority and must not define host behavior, agent guidance, workflow logic, gates, recovery, commands, or state transitions.

## Human Experience Requirements

The host adapter must let Forge produce:

- guided conversation
- one useful first question when facilitation is required
- energy matching
- direct correction of bad ideas
- concise status when the human wants only state
- rich explanation when the human is exploring
- no procedural confirmation for clearly mechanical work

The human experience comes from Behavior Contracts interpreted by the agent, not from long Markdown prompt scripts.

## Funnel Autonomy Requirements

Forge must implement Funnel Autonomy:

- early discovery has high human interaction and low mutation autonomy
- research, brainstorm, product direction, game direction, UX, and correct-course flows ask useful questions before committing state
- specification and planning close decisions through gates before build
- build/story/test loops become mechanically autonomous after accepted contracts exist
- release and operation regain gate pressure because blast radius is higher
- the agent may explore and propose broadly, but state mutation and route authority stay explicit

Funnel Autonomy is not "ask before everything." It is a phase-aware autonomy contract.

`guide` is responsible for translating Funnel Autonomy into the current operation contract. Early ambiguity should produce facilitation, research, visual alignment, or correct-course prompts before mutation. Settled build/story/test work should produce autonomous execution contracts composed of explicit write operations. Release and production-sensitive work should raise gate pressure again.

## Agent Experience Requirements

The agent must receive:

- compact operation contract
- allowed actions
- forbidden actions
- side-effect policy
- authority source
- commands with side effects
- required human-answer flag
- stop conditions
- exact files/contracts to load next

The agent must not need to read large Markdown docs to know what it may do.

## Host Conformance Requirements

Each host package must prove:

- it can install Forge
- it can find the Rust binary or service
- it can call `guide`
- it can call `oracle`
- it blocks or asks before mutating actions
- it preserves Forge output fields without rewriting authority
- it can update
- it can diagnose stale/old installation
- it can recover in a fresh chat/session

## Non-Requirements

This product shape does not require:

- Forge-owned desktop UI
- custom editor
- model provider UI
- session database for chat history
- full standalone app distribution

Those belong to the standalone app, not this borrowed-shell runtime.

## Open Grill Questions

- Which host is the first acceptance target?
- How much host-specific instruction is allowed before it becomes prompt debt?
- What is the minimum install/update story that counts as usable for non-developers?
