# Runtime Improvement Backlog

This backlog translates the BMAD comparison into concrete work for Forge Method Core.

## P0: Runtime Identity And Routing

Problem:

The agent must never confuse the runtime repo with a project created by the runtime.

Deliverables:

- detect runtime repo through `.codex-plugin/plugin.json`
- detect method project through `.forge-method/state.yaml`
- detect nested child project states
- ask a single routing question when ambiguous
- write route decision into state

Acceptance:

- Running the method inside the runtime repo reports `runtime-development`.
- Running it inside a child project reports that project state.
- Ambiguous state blocks with one clear question.

## P0: Durable State Engine

Problem:

Conversation memory is not a reliable source of project truth.

Deliverables:

- stronger state schema
- project registry
- sprint/story schema
- evidence ledger
- state transition validator

Acceptance:

- invalid phase transitions are rejected
- current status can be reconstructed after context reset
- all done states have evidence

## P0: Build-Story Loop

Problem:

Autonomous development needs a bounded loop.

Deliverables:

- ready story selector
- scoped context pack
- implementation step
- check runner
- review pass
- repair loop
- evidence writer

Acceptance:

- one ready story can be completed without reading unrelated docs
- failed checks keep the story in progress or blocked
- done requires acceptance criteria and evidence

## P1: BMAD-Core-Compatible Planning Spine

Deliverables:

- discovery workflow
- specification workflow
- planning workflow
- sprint initialization workflow
- ready/operate workflow

Acceptance:

- a project can move from idea to ready without custom commands for each step
- phase 5 exists and is reachable

## P1: Creative Studio Module

Deliverables:

- brainstorming workflow
- design-thinking workflow
- innovation workflow
- storytelling workflow
- presentation workflow

Acceptance:

- creative outputs are captured as artifacts
- selected ideas can feed specification
- unselected brainstorm docs can be marked ephemeral

## P1: Game Studio Module

Deliverables:

- game prototype workflow
- game brief workflow
- GDD workflow
- engine architecture workflow
- vertical slice workflow

Acceptance:

- game work preserves game-specific artifacts before becoming generic implementation
- engine choice becomes an adapter, not a fork of the method

## P1: Runtime Builder Module

Deliverables:

- create skill workflow
- create workflow state-machine workflow
- create module workflow
- create plugin package workflow
- eval discovery workflow

Acceptance:

- a new module can be generated as a Codex skill pack
- generated skill validates
- generated plugin validates
- trigger and artifact eval specs are created

## P2: Context Pack Builder

Deliverables:

- artifact index
- repo map
- active story context pack
- recent evidence summary
- failing check summary

Acceptance:

- implementation workflows load selected context, not whole docs
- context pack size is bounded and visible

## P2: Distribution Hardening

Deliverables:

- Windows installer
- macOS/Linux installer
- plugin marketplace instructions
- GitHub Actions smoke test
- example project

Acceptance:

- a friend can clone, install, invoke the skill, initialize a project, and run status
