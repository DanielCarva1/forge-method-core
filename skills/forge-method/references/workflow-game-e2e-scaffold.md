# workflow: game-e2e-scaffold

trigger:
  - game project needs an end-to-end smoke path
  - playable slice needs launch-to-result verification

inputs:
  - playable slice definition
  - engine/platform commands
  - test framework plan
  - acceptance checks

steps:
  1. define the shortest launch-to-result path
  2. scaffold or specify setup, action, assertion, and teardown
  3. decide automated, semi-automated, or manual evidence mode
  4. attach E2E proof to release/readiness gate

outputs:
  - game E2E scaffold
  - smoke path
  - evidence requirement

done_when:
  - launch-to-result path is explicit
  - check mode and evidence are clear
  - release/readiness gate can consume the result

blocked_when:
  - playable slice cannot launch
  - no observable success condition exists

handoff:
  - preserve smoke path, command or manual proof, evidence target, and next automation action
