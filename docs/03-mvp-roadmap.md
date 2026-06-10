# MVP Roadmap

## MVP Objective

Prove that a Codex-native runtime can:

1. install as a skill/plugin
2. initialize a project
3. keep durable state
4. choose the next workflow from state
5. run a build-story loop
6. mark a project ready for use

## Milestone 1: Runtime Core

Deliverables:

- `forge-method` skill
- `forge_method_runtime.py`
- `.forge-method/state.yaml`
- `init`, `status`, `next`, and `transition` commands
- Phase 0 through Phase 5 model

Acceptance:

- A fresh workspace can be initialized.
- Status does not depend on chat history.
- The runtime can say the next valid action.

## Milestone 2: Workflow Pack

Deliverables:

- `start-runtime`
- `route-project`
- `discover-intent`
- `write-spec`
- `plan-sprint`
- `build-story`
- `review-story`
- `ready-release`

Acceptance:

- Each workflow is a compact Markdown state machine.
- No workflow requires loading a long narrative doc.

## Milestone 3: Evidence And Sprint Loop

Deliverables:

- `sprint.yaml`
- `evidence/`
- `handoffs/`
- `ephemeral/`
- story status transitions

Acceptance:

- A story can move from ready -> in_progress -> review -> done.
- Evidence is written before done.
- Temporary task docs can be deleted after their result is registered.

## Milestone 4: Distribution

Deliverables:

- plugin manifest
- Windows installer
- optional macOS/Linux installer
- smoke test script
- example project

Acceptance:

- Another user can clone the repo, install the skill, open Codex, invoke the runtime, and initialize a project.

## Milestone 5: Modules

Initial modules:

- Software Builder
- Product Strategist
- Creative Studio
- Game Design Studio
- Test Architect
- Runtime Builder

Acceptance:

- Modules add workflows and templates without changing the core runtime.

## First End-To-End Smoke Test

1. Clone repo on a clean machine.
2. Run installer.
3. Open Codex in an empty folder.
4. Invoke `$forge-method`.
5. Initialize project `hello-method`.
6. Generate state.
7. Ask for next action.
8. Create one story.
9. Mark it ready.
10. Transition to Phase 5.

Passing this test proves the basic product path.

