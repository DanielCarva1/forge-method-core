# workflow: game-sprint-status

trigger:
  - game sprint status is requested
  - game team needs progress, risks, and next slice summarized

inputs:
  - sprint.yaml
  - story files
  - evidence
  - known risks

steps:
  1. summarize done, active, blocked, and deferred game work
  2. compare progress against the playable slice target
  3. surface risks, missing evidence, and scope pressure
  4. recommend the next game workflow or story action

outputs:
  - sprint status summary
  - risk/status notes
  - next action

done_when:
  - current playable-slice progress is clear
  - blockers and missing evidence are explicit
  - next action is actionable

blocked_when:
  - sprint state is missing
  - story/evidence files contradict each other

handoff:
  - preserve status summary, blockers, risk notes, and next action
