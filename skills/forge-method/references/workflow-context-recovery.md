# workflow: context-recovery

trigger:
  - context was compacted or reset
  - user asks to continue
  - agent is unsure where the project stands

inputs:
  - `.forge-method/state.yaml`
  - `.forge-method/sprint.yaml`
  - active story file
  - recent evidence
  - latest context pack

steps:
  1. run `status`
  2. run `audit`
  3. generate or refresh context pack
  4. read only the active workflow reference
  5. continue from state next action

outputs:
  - context pack
  - current state summary
  - next action

done_when:
  - current phase, workflow, story, and next action are known
  - no broad doc reload is needed

blocked_when:
  - state files contradict each other
  - active story points to a missing file

handoff:
  - preserve context pack path and audit result
