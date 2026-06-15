# workflow: readiness-check

trigger:
  - planning is complete
  - build phase is about to begin

inputs:
  - requirements
  - UX plan or explicit UX non-goal
  - architecture
  - epics or story backlog
  - validation strategy
  - open risks and inputs

steps:
  1. verify required artifacts exist
  2. check unresolved inputs, findings, and risks
  3. ensure stories have acceptance criteria and checks
  4. map PRD/spec, UX, architecture, risk, stories, validation, open inputs, and review findings
  5. write readiness matrix artifact
  6. move ready work into build phase

outputs:
  - readiness matrix artifact
  - blocked items or build clearance
  - updated state

done_when:
  - build can start without hidden planning decisions
  - first story is ready
  - readiness matrix links required sources to checks and evidence

blocked_when:
  - required artifact is missing
  - story checks are undefined
  - open input or review finding blocks build

handoff:
  - preserve readiness matrix path, blockers, first story, required checks, and next workflow
