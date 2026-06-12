# workflow: edge-case-review

trigger:
  - user asks for adversarial, edge-case, or failure-mode review
  - plan/spec/workflow needs stress testing

inputs:
  - target artifact
  - success criteria
  - known constraints
  - risk tolerance

steps:
  1. enumerate boundary conditions and misuse/failure cases
  2. classify severity, likelihood, and detectability
  3. identify missing checks, docs, or workflow guards
  4. recommend fixes, waivers, or follow-up stories

outputs:
  - edge-case findings
  - risk ranking
  - recommended follow-up

done_when:
  - important failure modes are explicit
  - recommendations are actionable
  - owner or next workflow is known

blocked_when:
  - target artifact is unavailable
  - success criteria are too vague to stress test

handoff:
  - preserve target path, findings, severity, waivers, and follow-up stories
