# V1 Roadmap

## V1 Objective

Forge Method Core v1 must make a project recoverable, executable, and auditable across context resets.

The v1 standard is:

1. install as a Codex skill/plugin package
2. initialize durable project state
3. route by state instead of chat memory
4. move through explicit phase transitions
5. manage stories as files
6. require evidence before done
7. generate compact context packs
8. audit state integrity
9. reach ready/operate as a real phase

## Runtime Core

Delivered surfaces:

- `forge-method` skill
- `forge_method_runtime.py`
- `.forge-method/state.yaml`
- `.forge-method/projects.yaml`
- `.forge-method/sprint.yaml`
- `.forge-method/stories/`
- `.forge-method/artifacts/`
- `.forge-method/checkpoints/`
- `.forge-method/evidence/`
- `.forge-method/context/`
- `.forge-method/evals/`
- `.forge-method/handoffs/`
- `.forge-method/agents/`
- `.forge-method/inputs/`
- `.forge-method/reviews/`
- `.forge-method/workflows/`
- `.forge-method/modules/`
- `.forge-method/ledger.ndjson`

## Required Commands

```powershell
init
preflight
start
status
snapshot
next
transition
project list/create
story add/list/export/import/start/review/done/block
input add/list/answer/defer
review add/list/resolve/waive
artifact add/list
artifact link-story
artifact capture/verify
evidence add
checkpoint
context pack
context plan
context recover
module list/recommend/show/create
agent list/show/recommend/validate
workflow list/validate/create
eval add/list/run
eval kinds: workflow-routing, workflow-trigger, artifact-exists
audit
gate
ready
release plan/check
handoff
doctor
```

## Workflow Pack

Required workflows:

- start-runtime
- discover-intent
- write-spec
- plan-sprint
- build-story
- creative-session
- game-project
- runtime-builder
- ready-release
- evolve-project
- context-recovery

## Evidence Standard

A done story must include evidence. Evidence must include:

- kind
- timestamp
- summary
- optional story id
- optional checks

## Context Standard

A context pack must include:

- project
- phase
- workflow
- active story
- next action
- active story acceptance criteria
- recent evidence paths
- recommended agent profiles

A context load plan must include:

- selected files in priority order
- reason for each file
- estimated character budget
- deferred files when the budget is full

A preflight must resolve project identity before the agent reads broad context. It must identify existing project state, runtime repo state, known child projects, required user choice, selected context files, and the next helper commands without writing state.

## Release Standard

A project can enter ready/operate only when:

- audit passes
- no story remains in progress or review
- release evidence is written
- state is updated to `5-ready-operate`

## V1 Hardening Backlog

- CI that runs unit tests and smoke tests
- richer artifact index
- workflow generator
- cross-platform installer
