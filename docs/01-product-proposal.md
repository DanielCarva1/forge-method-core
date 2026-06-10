# Product Proposal: Forge Method Core

## Thesis

Forge Method Core is the Codex-native runtime repository for Forge Method. It turns an intent into artifacts, implementation, validation, release, and future evolution.

The goal is not to clone BMAD. The goal is to preserve what BMAD gets right and rebuild the runtime around Codex primitives.

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

## Sources And Influences

- BMAD: mature phase-based agent workflows, Builder, Creative Suite, Game Dev Studio, Test Architect, and dev loop automation.
- GitHub Spec Kit: spec-driven development with Spec -> Plan -> Tasks -> Implement.
- Kiro Specs: requirements/design/tasks with task tracking and execution accountability.
- Aider: repo maps and automatic lint/test feedback loops.
- SWE-agent: agent-computer interface matters as much as prompting.
- OpenAI Codex Skills: progressive disclosure, skill packaging, references, scripts, and assets.
- OpenAI repair loops: Review -> Repair -> Validate as a repeatable loop.

## Core Improvement Over BMAD-Style Docs

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

`Forge Method` is descriptive but may sound too official. Candidate names:

- Forge Method Runtime
- Artifact Method Runtime
- Creation Runtime
- Forgeflow
- MethodOS
- Project Forge

The strongest product name is probably `Forge Method Runtime`: it carries the "thing that creates things" idea without depending on OpenAI branding.

The runtime repository is `forge-method-core`. The user-facing Codex skill remains `$forge-method`.

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
