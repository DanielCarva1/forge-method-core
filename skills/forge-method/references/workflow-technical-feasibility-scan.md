# workflow: technical-feasibility-scan

trigger:
  - reality/evidence gate needs technical feasibility evidence
  - idea depends on automation, AI capability, hardware, sensors, integrations, scale, or performance
  - implementation promise may exceed available tools, data, permissions, budget, or physics

inputs:
  - product promise or technical requirement
  - target platform, data, APIs, models, hardware, infrastructure, and constraints
  - success threshold, failure cost, and safety/legal limits
  - decision the research must unlock

steps:
  1. frame the claim, decision_to_unlock, success threshold, and falsifier
  2. check physical possibility, tool capability, data availability, permission limits, and operational cost
  3. name feasibility_stance, riskiest_unknowns, and cheapest proof_path
  4. grade sources by recency, authority, directness, and bias
  5. run artifact research-scan with feasibility_stance, riskiest_unknowns, proof_path, contradictions_or_falsifiers, uncertainty, stance, and next_workflow
  6. run artifact research-check --path <research-scan-artifact>
  7. route to research-closeout, architecture, quick-prototype, or correct-course

outputs:
  - technical feasibility artifact
  - feasibility stance
  - riskiest unknowns
  - proof path and next workflow

done_when:
  - impossible or unsafe promises are blocked before planning
  - plausible promises have a concrete proof_path or prototype slice
  - required data, tools, permissions, and limits are explicit
  - artifact research-scan has registered the durable scan
  - artifact research-check passes

blocked_when:
  - required platform, data, credentials, or access is unknown
  - safety, legal, or privacy review is required before proof
  - success threshold or failure cost is undefined

handoff:
  - preserve artifact path, claim, sources, feasibility_stance, riskiest_unknowns, proof_path, uncertainty, stance, and next workflow
