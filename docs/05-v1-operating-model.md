# V1 Operating Model

Forge Method Core v1 is a file-backed runtime for long-horizon creation work. The agent may change, the context window may reset, and the terminal may close; the project state must still be recoverable.

## Runtime Shape

```txt
intent -> discovery -> specification -> planning -> build/verify -> ready/operate -> evolve
```

## Durable Runtime Surfaces

```txt
.forge-method/
  state.yaml              current phase, workflow, project, next action
  projects.yaml           project identity and registry metadata
  sprint.yaml             sprint summary and active story pointer
  stories/                one state file per story
  evidence/               proof that work happened and checks ran
  artifacts/              generated specs, briefs, plans, and release notes
  context/                compact context packs for future sessions
  handoffs/               continuation notes after large work blocks
  ledger.ndjson           append-only runtime event stream
```

## Agent-Facing Docs

Agent-facing docs are state machines. They should be short, explicit, and loaded only when the current state needs them.

Required sections:

```md
trigger:
inputs:
steps:
outputs:
done_when:
blocked_when:
handoff:
```

## Human-Facing Docs

Human-facing docs explain why the runtime exists, how to install it, and how to reason about it. They should not be required during normal execution.

## Completion Rule

A task is complete only when:

- required output exists
- acceptance criteria are satisfied
- evidence is written
- state is updated
- next action is known

## Context Rule

The agent should not load all project documentation. It should build a compact context pack from:

- current state
- active workflow
- active story
- relevant artifacts
- recent evidence
- failing checks
- next action

## Ready Rule

Phase 5 means the project is ready for use. It is not a pause in implementation; it is a distinct operating state with release evidence, usage notes, support status, and future backlog.
