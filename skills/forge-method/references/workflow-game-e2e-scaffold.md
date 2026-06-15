# workflow: game-e2e-scaffold

trigger:
  - game project needs an end-to-end smoke path
  - playable slice needs launch-to-result verification

inputs:
  - playable slice definition
  - engine/platform commands
  - test framework plan
  - acceptance checks
  - release/readiness gate expectations

steps:
  1. define the shortest launch-to-result path
  2. scaffold or specify setup, action, assertion, teardown, and deterministic success signal
  3. decide automated, semi-automated, or manual evidence mode
  4. record command, evidence capture, failure repair policy, and release/readiness gate link
  5. run `artifact test-check --path <game-e2e-artifact>`

outputs:
  - game E2E scaffold
  - smoke path
  - evidence requirement
  - release gate link

done_when:
  - launch-to-result path is explicit
  - setup/action/assertion/teardown are explicit
  - observable success signal is stable
  - check mode and evidence are clear
  - release/readiness gate can consume the result
  - test-check proof passes or waiver is explicit

blocked_when:
  - playable slice cannot launch
  - no observable success condition exists
  - evidence cannot be captured and no manual proof is acceptable

handoff:
  - preserve launch command, smoke path, setup/action/assertion/teardown, success signal, evidence mode, gate link, and next automation action
