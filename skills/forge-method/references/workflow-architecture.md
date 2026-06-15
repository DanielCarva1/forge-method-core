# workflow: architecture

trigger:
  - product requirements exist
  - track requires technical design before build

inputs:
  - product requirements
  - UX plan or taste/rejection notes
  - current codebase inventory
  - constraints
  - security, privacy, compliance, and operational risk notes
  - test strategy or validation expectations
  - validation expectations

steps:
  1. choose mode: create, update, validate, or tradeoff review
  2. trace product requirements, UX decisions, constraints, and risks into architecture decisions
  3. identify system boundaries, components, data ownership, interfaces, dependencies, and integration contracts
  4. compare plausible options and record rejected options with reasons
  5. record security, privacy, deployment, observability, performance, and test hooks
  6. map decisions to story boundaries, readiness dependencies, and validation findings
  7. save architecture artifact and next workflow

outputs:
  - architecture artifact
  - decision log
  - rejected options
  - interface and data contracts
  - security/privacy/ops notes
  - story impact map
  - validation hooks and findings

done_when:
  - decisions trace to product requirements, UX, constraints, or explicit risk
  - rejected options and tradeoffs are visible
  - build stories can reference architecture constraints
  - major risks have checks or mitigations
  - story creation/readiness can inspect architecture dependencies
  - no required decision is hidden

blocked_when:
  - product requirement or UX premise is missing
  - architecture choice depends on unanswered human input
  - implementation path has incompatible constraints
  - security, privacy, data, or integration risk cannot be bounded

handoff:
  - preserve architecture path, mode, source trace, decisions, rejected options, risks, interfaces, story impact, validation hooks, and next workflow
