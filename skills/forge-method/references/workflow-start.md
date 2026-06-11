# workflow: start-runtime

trigger:
  - user asks to start Forge Method
  - user asks to create or resume a method project
  - `.forge-method/state.yaml` is missing or needs routing

inputs:
  - current workspace path
  - optional `.forge-method/state.yaml`
  - optional `.forge-method/projects.yaml`

state:
  allowed_phases:
    - 0-route
    - 1-discovery
    - 2-specification
    - 3-plan
    - 4-build-verify
    - 5-ready-operate
    - 6-evolve

steps:
  1. check whether current workspace is the runtime repo
  2. check whether current workspace has `.forge-method/state.yaml`
  3. if state exists, summarize current project, phase, status, and next action
  4. if no state exists, briefly explain Forge Method and ask what the human wants to create
  5. if multiple known projects exist, show them and ask the user to pick one or create a new one
  6. after initialization, write state and report next action

outputs:
  - `.forge-method/state.yaml`
  - `.forge-method/sprint.yaml`
  - human route prompt
  - concise status summary

done_when:
  - context is resolved
  - active project is known
  - current phase is known
  - next valid action is known

blocked_when:
  - user must choose between multiple projects
  - target directory is unsafe or ambiguous
  - runtime repo and child project state conflict

handoff:
  - write the current project, phase, status, and next action into state
