# workflow: game-test-automation

trigger:
  - game project needs automated checks for a playable slice
  - manual playtest evidence should be converted into repeatable checks

inputs:
  - game test framework plan
  - story or slice target
  - available tooling
  - risk list

steps:
  1. choose the smallest high-value automation target
  2. define fixtures, deterministic setup, and expected result
  3. implement or plan the check command
  4. attach automation evidence to story/release gate

outputs:
  - automation plan or test change
  - command
  - evidence link

done_when:
  - automation target has a command or explicit plan
  - result can be reproduced
  - evidence is attached to the relevant story or gate

blocked_when:
  - deterministic setup is impossible
  - tooling access is unavailable

handoff:
  - preserve command, fixture notes, evidence path, and remaining manual checks
