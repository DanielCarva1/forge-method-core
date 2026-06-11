# workflow: readiness-check

trigger:
  - planning is complete
  - build phase is about to begin

inputs:
  - requirements
  - architecture
  - epics or story backlog
  - validation strategy
  - open risks and inputs

steps:
  1. verify required artifacts exist
  2. check unresolved inputs, findings, and risks
  3. ensure stories have acceptance criteria and checks
  4. write implementation readiness artifact
  5. move ready work into build phase

outputs:
  - readiness artifact
  - blocked items or build clearance
  - updated state

done_when:
  - build can start without hidden planning decisions
  - first story is ready
  - readiness evidence exists

blocked_when:
  - required artifact is missing
  - story checks are undefined

handoff:
  - preserve readiness path, blockers, first story, and required checks
