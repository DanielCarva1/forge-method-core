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
  1. compare candidate tracks against objective, complexity, risk, and expected artifacts
  2. select one track/module and record rejected alternatives
  3. map required and optional workflows for the selected track
  4. write track decision artifact
  5. route the next workflow

outputs:
  - track decision artifact
  - required workflow map
  - next workflow

done_when:
  - selected track/module is explicit
  - rejected tracks have reasons
  - next workflow is actionable

blocked_when:
  - project objective is too vague to distinguish tracks
  - required constraints are unknown

handoff:
  - preserve selected track, rejected tracks, required artifacts, and next workflow
