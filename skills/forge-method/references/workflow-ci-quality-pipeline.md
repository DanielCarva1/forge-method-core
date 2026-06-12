# workflow: ci-quality-pipeline

trigger:
  - CI quality gate needs setup or review
  - release/readiness depends on automated checks

inputs:
  - repository commands
  - test framework plan
  - release criteria
  - CI provider constraints

steps:
  1. list required fast, full, and release checks
  2. define CI jobs, order, caching, artifacts, and failure policy
  3. map checks to gate/readiness requirements
  4. record local parity command

outputs:
  - CI quality pipeline plan
  - gate command map
  - failure policy

done_when:
  - required CI checks are explicit
  - local parity commands are known
  - gate failure policy is recorded

blocked_when:
  - CI provider or command surface is unavailable
  - required checks cannot run locally or in CI

handoff:
  - preserve pipeline plan, job/check mapping, local commands, and failure policy
