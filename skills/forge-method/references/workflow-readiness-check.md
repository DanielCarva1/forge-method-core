# workflow: readiness-check

trigger:
  - planning is complete
  - build phase is about to begin

inputs:
  - requirements
  - UX plan or explicit UX non-goal
  - architecture
  - track decision and track-required artifact map
  - epics or story backlog
  - validation strategy
  - open risks and inputs

steps:
  1. verify required artifacts exist
  2. check track-specific artifacts, unresolved inputs, findings, and risks
  3. ensure stories have acceptance criteria and checks
  4. map PRD/spec, UX, architecture, risk, stories, validation, enterprise evidence, open inputs, and review findings
  5. write readiness matrix artifact and run `artifact enterprise-check` when selected_track is enterprise
  6. move ready work into build phase only when missing sources are explicit

outputs:
  - readiness matrix artifact
  - blocked items or build clearance
  - track artifact coverage
  - updated state

done_when:
  - build can start without hidden planning decisions
  - first story is ready
  - readiness matrix links required sources to checks and evidence
  - enterprise artifact gaps are resolved, blocked, or waived with owner and release impact

blocked_when:
  - required artifact is missing
  - story checks are undefined
  - open input or review finding blocks build
  - enterprise security/privacy/risk/quality/release evidence is missing without waiver

handoff:
  - preserve readiness matrix path, blockers, enterprise evidence status, first story, required checks, waivers, and next workflow
