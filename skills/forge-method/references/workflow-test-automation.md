# workflow: test-automation

trigger:
  - user asks to expand automated test coverage
  - risk register identifies behavior needing repeatable checks

inputs:
  - test strategy or framework
  - risk register
  - target stories
  - current commands

steps:
  1. choose automation targets by risk and maintainability
  2. define fixtures, data setup, assertions, and command scope
  3. implement or plan command-level checks with evidence links
  4. attach evidence, manual remainders, and gate impact to story/gate

outputs:
  - automation plan or changes
  - test commands
  - evidence links
  - manual remainders

done_when:
  - automation targets are justified by risk
  - checks have commands or explicit implementation stories
  - evidence path is clear
  - remaining manual checks or waivers are explicit

blocked_when:
  - target behavior is not observable
  - fixtures or environment cannot be controlled

handoff:
  - preserve automation targets, fixtures, commands, evidence links, remaining manual checks, and gate impact
