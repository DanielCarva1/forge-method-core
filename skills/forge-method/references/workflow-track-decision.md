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
  4. for enterprise, include baseline/conditional artifacts, evidence gates, and waivers
  5. run `artifact enterprise-track-map --path <artifact>` then `artifact enterprise-check --path <artifact>`
  6. route the next workflow

outputs:
  - generated track decision artifact
  - workflow/artifact map
  - next workflow

done_when:
  - selected track/module is explicit
  - rejected tracks have reasons
  - required artifacts and evidence gates are mapped
  - enterprise artifact is registered when enterprise is selected
  - next workflow is actionable

blocked_when:
  - project objective is too vague to distinguish tracks
  - required constraints are unknown
  - enterprise is selected but required artifact/evidence map is missing

handoff:
  - preserve selected/rejected tracks, required/conditional artifacts, evidence map, waiver policy, readiness gate, and next workflow
