# Product Proposal: Forge Method Core

## Thesis

Forge Method Core is the Codex-native runtime repository for Forge Method. It turns an intent into artifacts, implementation, validation, release, and future evolution.

The goal is to make long-horizon agentic creation reliable: every important decision, task, evidence item, and phase transition must survive context resets and be auditable from files.

## Product Promise

The runtime should let a user open Codex and say:

```txt
Start Forge Method in this workspace.
```

Then Codex should:

1. identify whether this is the runtime itself or a project created by the runtime
2. show existing projects and ask whether to open one or create a new one
3. keep durable state in files
4. move through state-machine phases
5. automate non-human-dependent development work
6. ask for human input only at meaningful decision points
7. validate work before marking it done
8. eventually reach a "ready for use" state instead of staying forever in implementation

## Core Product Rule

Agent-facing docs should be short state machines, not long narrative manuals.

Long-form explanation belongs in human docs. Runtime execution belongs in compact workflow files:

```md
trigger:
inputs:
state:
transitions:
outputs:
gates:
blocked_when:
done_when:
```

This keeps context smaller, makes behavior auditable, and prevents the agent from confusing product state with runtime development state.

## Naming

The runtime repository is `forge-method-core`. The user-facing Codex skill is `$forge-method`.

## Required Capabilities

The runtime must support:

- software projects
- product planning
- creative direction
- game design
- test architecture
- runtime/module building
- evidence-backed implementation
- project lifecycle from idea to ready-to-use

## Non-Goals

- Do not rely on one slash command as the whole system.
- Do not load massive docs into context.
- Do not infer state from conversation history.
- Do not keep building forever after the product is ready.
- Do not mix runtime development state with child project state.
