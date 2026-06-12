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
  2. define fixtures, data setup, and assertions
  3. implement or plan command-level checks
  4. attach evidence to story/gate

outputs:
  - automation plan or changes
  - test commands
  - evidence links

done_when:
  - automation targets are justified by risk
  - checks have commands or explicit implementation stories
  - evidence path is clear

blocked_when:
  - target behavior is not observable
  - fixtures or environment cannot be controlled

handoff:
  - preserve automation targets, commands, evidence links, and remaining manual checks
