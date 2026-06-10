# Runtime Architecture

## Native Codex Pieces

The runtime should be built from Codex-native primitives:

- Plugin: distribution unit for reusable skills and scripts.
- Skill: workflow entrypoint with progressive disclosure.
- `AGENTS.md`: short repo-level rules only.
- File-backed state: source of truth for phase, project, workflow, and next action.
- Scripts: deterministic status, initialization, validation, and transitions.
- Subagents: delegated review, QA, research, architecture, and creative passes.
- MCP/plugins: GitHub, browser, Figma, Canva, Drive, Slack, and other integrations when needed.

## Project State Layout

Every project using the runtime gets:

```txt
.forge-method/
  state.yaml
  projects.yaml
  sprint.yaml
  artifact-index.yaml
  evidence/
  handoffs/
  ephemeral/
  workflows/
```

The runtime itself is always separate from projects created by the runtime.

## Phase Model

### Phase 0: Route

Resolve context before doing work.

- Is this the runtime repo?
- Is this a project using the runtime?
- Is there an active project?
- Does the user want an existing project or a new one?

### Phase 1: Discovery

Interview and facilitation.

Outputs:

- intent brief
- constraints
- user goals
- domain notes

### Phase 2: Specification

Structured specs.

Outputs depend on module:

- product spec
- software requirements
- creative brief
- game design brief
- acceptance criteria

### Phase 3: Plan

Transform spec into executable plan.

Outputs:

- architecture notes
- task graph
- sprint plan
- risk/test plan

### Phase 4: Build And Verify

Autonomous execution loop.

Loop:

1. select next ready story
2. inspect scoped context
3. implement minimal diff
4. run checks
5. review
6. repair if needed
7. write evidence
8. update sprint state

### Phase 5: Ready / Operate

The project is usable.

This phase prevents the system from treating everything as permanently under construction.

Outputs:

- release evidence
- usage instructions
- support status
- future backlog

### Phase 6: Evolve

Future versions.

Inputs:

- user feedback
- bugs
- eval results
- analytics
- new ideas

## Workflow File Contract

Every workflow loaded by an agent should fit this structure:

```md
# workflow: name

trigger:
inputs:
state:
steps:
outputs:
done_when:
blocked_when:
handoff:
```

The workflow must be short enough to load cheaply and precise enough to prevent guessing.

## Context Strategy

The runtime should never ask the agent to read all docs.

It should build a context pack from:

- current state
- active workflow
- active story/spec
- artifact index
- relevant repo map
- failing checks
- last evidence entry

This follows the same practical lesson as repo-map based coding agents: context must be selected, not dumped.

## Verification Strategy

For implementation workflows, the default loop is:

```txt
Review -> Repair -> Validate
```

Validation evidence can include:

- tests
- type checks
- lint
- browser checks
- screenshots
- CI results
- code review findings

The runtime should not mark work done without evidence.

