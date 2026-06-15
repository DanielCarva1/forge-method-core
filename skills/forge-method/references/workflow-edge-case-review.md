# workflow: edge-case-review

trigger:
  - user asks for adversarial, edge-case, or failure-mode review
  - plan/spec/workflow needs stress testing

inputs:
  - target artifact
  - success criteria
  - known constraints
  - risk tolerance
  - requested mode: boundary, failure, misuse, or validate

steps:
  1. restate intended behavior and risk tolerance
  2. enumerate boundary conditions, misuse cases, and failure modes
  3. classify severity, likelihood, and detectability
  4. identify missing checks, docs, workflow guards, or waivers
  5. recommend fixes, waivers, or follow-up stories

outputs:
  - edge-case findings
  - missing check map
  - risk ranking
  - waiver or follow-up story map
  - recommended follow-up

done_when:
  - important failure modes are explicit
  - recommendations are actionable
  - owner or next workflow is known
  - accepted waivers include rationale and revisit trigger

blocked_when:
  - target artifact is unavailable
  - success criteria are too vague to stress test

handoff:
  - preserve target path, findings, severity, detectability, missing checks, waivers, and follow-up stories
