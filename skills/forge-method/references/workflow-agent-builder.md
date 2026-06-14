# workflow: agent-builder

trigger:
  - user wants to create, edit, rebuild, or design an agent profile or agent skill
  - a module plan names an agent as the next build target

inputs:
  - agent idea or existing agent path
  - target module or standalone use
  - persona, outcome, autonomy, memory, and safety expectations
  - capabilities, tools, scripts, and validation needs

steps:
  1. classify build mode: create, edit, rebuild, or headless plan
  2. determine agent type: stateless, project-memory, or autonomous-supporting
  3. define identity, outcome, non-negotiable, capabilities, boundaries, and handoff
  4. identify deterministic script opportunities and dependency cost
  5. write agent build plan, generated paths, and quality follow-up

outputs:
  - agent build plan
  - capability contract
  - memory and autonomy decision
  - quality follow-up

done_when:
  - agent identity and success criteria are concrete
  - capabilities describe outcomes instead of unnecessary procedure
  - next workflow is `agent-analyze`, `module-builder`, or `workflow-validate`

blocked_when:
  - agent role overlaps existing workflows without a routing rule
  - autonomy or memory boundary is unsafe or unspecified
  - source agent cannot be read for edit or rebuild

handoff:
  - preserve agent name, target path, type decision, capabilities, boundaries, generated paths, and validation command
