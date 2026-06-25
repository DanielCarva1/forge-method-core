# Agent-Facing Contract Migration Plan

## Goal

Move Forge Method from prompt-shaped Markdown behavior toward a protocol-driven runtime where agents consume compact typed contracts and humans receive a rich guided experience through the agent.

The final state must preserve the two Forge principles:

- Humans get a rich, guided, expressive, well-explained experience.
- Agents get compact state machines, typed contracts, clear authority, and low prompt debt.

## Final System Design

### Forge Core

Owns runtime authority as Rust code:

- state machine transitions
- operation mode
- side-effect policy
- authority source
- command permission
- schema validation
- ledger events
- recovery contracts
- route stability

Forge Core is Rust-first. Python is not a valid implementation target for new core authority.

### Forge Adapters

Translate Forge Core contracts to host environments:

- Codex
- Cursor
- Claude
- OpenCode
- CLI
- future Forge standalone app

Adapters may explain, execute, and render contracts. They must not invent route authority.

### Forge Contracts

Versioned YAML or JSON contracts consumed by Core and adapters.

Contract families:

- Operation Contract
- Workflow Contract
- Behavior Contract
- Knowledge Contract
- Artifact Contract
- Recovery Contract
- State Transition Contract
- Command Contract
- Renderer Contract

Contracts should be compact, schema-validated, and Rust/Serde native.

### No Python Core Policy

The existing Python runtime is a behavioral reference, not a foundation to extend. The rewrite must not depend on Python as a bridge, adapter, orchestration layer, or long-lived compatibility layer.

Host compatibility must be solved through Rust-native surfaces:

- CLI with JSON stdin/stdout
- MCP server
- HTTP/SSE server
- native app integration
- signed binary distribution
- schema-validated files

### Forge Renderers

Generate README content, installation instructions, and release notes from typed contracts where useful. Rendered Markdown is output, not runtime input.

### Behavior Layer

Defines how an agent guides humans:

- tone
- energy matching
- first question
- follow-up style
- when to provoke
- when to slow down
- when to accelerate
- humor/friction boundaries
- handoff phrasing

Behavior contracts should be short structured data, not large prose instructions.

## Markdown Policy

Markdown is allowed only for:

- README
- installation instructions
- release notes
- temporary ADR/design notes before the equivalent runtime contract exists

Markdown must not be the source of truth for:

- route
- phase
- workflow
- next action
- operation mode
- behavior
- side effects
- state mutation
- command permission
- gate decisions
- recovery authority
- host behavior
- agent guidance

New runtime or agent-facing material must start as YAML/JSON contract, schema, fixture, or generated data. Markdown outside the allowlist is migration debt, not accepted architecture. It must be deleted or replaced by typed contracts. Until removal, it must not be loaded by runtime flows and any runtime-critical fields must move into typed contracts immediately.

## Critical Contracts

### Operation Contract

Returned by `preflight`, `resume`, `guide`, and future host adapters.

Minimum fields:

```yaml
schema_version:
operation_mode:
authority_source:
side_effect_policy:
autonomy_budget:
may_run_commands_now:
requires_human_answer:
allowed_actions:
forbidden_actions:
commands:
response_contract:
stop_conditions:
```

### Command Contract

Describes commands as capabilities instead of raw strings.

Minimum fields:

```yaml
name:
command:
side_effects:
may_run_now:
requires_human_confirmation:
writes_state:
changes_route:
expected_outputs:
failure_policy:
```

### Workflow Contract

Replaces Markdown workflow authority.

Minimum fields:

```yaml
id:
phase:
trigger:
inputs:
steps:
outputs:
done_when:
blocked_when:
handoff:
side_effect_policy:
state_transition_policy:
behavior_contract:
artifact_contracts:
```

### Behavior Contract

Replaces long facilitation prose as the operational source for human guidance.

Minimum fields:

```yaml
id:
default_tone:
energy_match:
first_question:
follow_up_moves:
facilitator_moves:
anti_patterns:
fast_path:
deep_path:
requires_human_answer:
forbidden_actions:
```

### Artifact Contract

Defines artifact structure, lifecycle, validation, and authority.

Minimum fields:

```yaml
kind:
path:
schema:
lifecycle:
authority:
required_fields:
validation:
next_workflow_policy:
renderers:
```

### Recovery Contract

Defines what future agents may trust after context loss.

Minimum fields:

```yaml
state_summary:
authoritative_next_action:
non_authoritative_suggestions:
recent_evidence:
open_inputs:
open_findings:
load_plan:
forbidden_inferences:
```

## Migration Sequence

### 1. Freeze Runtime Authority

Inventory every place where Markdown currently influences runtime behavior. Mark whether each surface is allowed render output or migration debt to delete/replace.

Output:

- authority inventory
- list of Markdown authority leaks
- first schema targets

### 2. Add Operation Contract

Extend `guide`, `resume`, and `preflight` with operation mode, side-effect policy, allowed actions, forbidden actions, authority source, and typed command metadata.

This addresses host-agent drift without blocking agent creativity.

### 3. Harden Route Mutation

Require explicit authority for route-changing operations:

- phase
- workflow
- next action
- active guidance mode
- artifact lifecycle

Checkpoint and handoff remain memory unless explicitly authorized.

### 4. Split Correct-Course Intake And Commit

Correct-course must distinguish:

- intake/facilitation: observe, ask, diagnose, no mutation
- commit: write artifact and update state after explicit authority

### 5. Convert Workflow Catalog And High-Risk Workflows

Move workflow authority from `workflow-*.md` into typed workflow contracts for:

- guidance-engine
- correct-course
- context-recovery
- checkpoint-preview
- story lifecycle
- visual alignment
- team collaboration
- game studio/MDA

Markdown references must be replaced by typed workflow contracts. Generated Markdown is allowed only as render output from those contracts.

### 6. Convert Facilitation Packs To Behavior Contracts

Replace long facilitation packs with compact behavior contracts. Preserve human richness through agent behavior, not long documents.

### 7. Add Artifact And Recovery Sidecars

For artifacts, checkpoints, handoffs, and context packs, replace Markdown authority with typed contract records. Runtime reads the typed contract, not Markdown headings.

### 8. Schema Validation And Tests

Add schema validation for each contract family and release tests for:

- route stability
- side-effect policy
- no-write observe mode
- human frustration handling
- energy matching
- checkpoint non-authority
- long-chat drift
- fresh-chat recovery
- adapter compatibility

### 9. Rust Core Boundary

After contracts stabilize, implement the Rust core boundary directly:

- state loading
- state validation
- operation contract validation
- route mutation authorization
- ledger append
- lanes, locks, and claims
- command side-effect policy

Python does not keep orchestration. Host adapters consume the Rust operation surface.

## Rollout Rules

- Remove Markdown debt as soon as the typed replacement exists and tests pass. Do not preserve old Markdown as reference material.
- Do not migrate by wrapping long prose in YAML.
- New runtime authority must be typed first.
- New runtime authority must be implemented in Rust.
- Do not add Python core features.
- Human experience regressions are release blockers.
- Agent drift tests are release blockers.
- Each release must keep existing projects recoverable.

## First Rust Implementation Slice

The first slice should be a Rust contract crate plus operation/command contract types and validation.

That slice is small enough to validate, but important enough to prove the new architecture:

- observe mode cannot mutate state
- correct-course intake asks before writing
- "what does Forge say?" returns report-only behavior
- commands expose side effects
- host agents receive less ambiguous instructions through a Rust-owned operation surface
