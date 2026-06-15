# workflow: track-decision

trigger:
  - user asks which Forge track or module should guide the project
  - existing project route is ambiguous or changing

inputs:
  - project objective
  - current state
  - candidate tracks/modules
  - constraints and risk profile

steps:
  1. compare tracks against objective, complexity, risk, and expected artifacts
  2. select one track/module and record rejected alternatives
  3. map required, optional, and track-specific artifacts
  4. for enterprise, include security, privacy, risk, quality, NFR, traceability, release, conditional DevOps/compliance/observability, and waivers
  5. write track decision artifact and run `artifact enterprise-check` when enterprise is selected
  6. route the next workflow

outputs:
  - track decision artifact
  - required workflow map
  - track artifact map
  - next workflow

done_when:
  - selected track/module is explicit
  - rejected tracks have reasons
  - required artifacts and evidence gates are mapped
  - next workflow is actionable

blocked_when:
  - project objective is too vague to distinguish tracks
  - required constraints are unknown
  - enterprise is selected but required artifact/evidence map is missing

handoff:
  - preserve selected track, rejected tracks, required/conditional artifacts, evidence map, waiver policy, readiness gate, and next workflow
