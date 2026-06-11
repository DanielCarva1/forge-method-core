# workflow: technical-feasibility-scan

trigger:
  - reality/evidence gate needs technical feasibility evidence
  - idea depends on automation, AI capability, hardware, sensors, integrations, or scale
  - implementation promise may exceed available tools, data, budget, or physics

inputs:
  - product promise or technical requirement
  - target platform and constraints
  - available data, APIs, hardware, models, or infrastructure
  - success threshold and failure cost

steps:
  1. check whether the promise is physically and technically possible
  2. identify required data, tools, integrations, permissions, and operational limits
  3. name the riskiest unknowns and cheapest proof
  4. recommend build, prototype, research, pivot, or block
  5. write a compact feasibility scan with proof path

outputs:
  - technical feasibility artifact
  - key constraints and unknowns
  - proof path or block reason

done_when:
  - impossible promises are blocked before planning
  - plausible promises have a proof path
  - risks are linked to checks, prototype, or story work

blocked_when:
  - required platform, data, or access is unknown
  - feasibility depends on external credentials or private systems
  - safety or legal review is needed before a proof

handoff:
  - preserve feasibility stance, riskiest unknowns, proof path, and blocking dependencies
