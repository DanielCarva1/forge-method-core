# workflow: context-recovery

trigger:
  - context was compacted or reset
  - user asks to continue
  - agent is unsure where the project stands

inputs:
  - `.forge-method/state.yaml`
  - `.forge-method/sprint.yaml`
  - context health
  - active story file
  - recent evidence
  - latest context pack

steps:
  1. run `status`
  2. run `context health`
  3. if health is `compact` or `blocked`, run compact recovery before broader reading
  4. run `audit`
  5. generate or refresh context pack
  6. read only the active workflow reference
  7. continue from state next action

outputs:
  - context health result
  - context pack
  - current state summary
  - next action

done_when:
  - current phase, workflow, story, and next action are known
  - context health is `ok` or compact recovery exists
  - no broad doc reload is needed

blocked_when:
  - state files contradict each other
  - active story points to a missing file
  - required context cannot fit and no compact recovery can be written

handoff:
  - preserve context health level, context pack path, and audit result
