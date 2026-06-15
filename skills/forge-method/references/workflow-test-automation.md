# workflow: test-automation

trigger:
  - user asks to expand automated test coverage
  - risk register identifies behavior needing repeatable checks

inputs:
  - test strategy or framework
  - risk register
  - target stories
  - current commands
  - detected framework and existing test patterns

steps:
  1. detect or confirm the existing framework and command surface
  2. choose automation targets by risk, maintainability, and observability
  3. define API checks, E2E workflows, fixtures, data setup, semantic locators, assertions, and command scope
  4. implement or plan independent checks with no hardcoded waits and visible-outcome assertions
  5. run or specify run-and-fix result, evidence links, manual remainders, and gate impact
  6. run `artifact test-check --path <test-automation-artifact>`

outputs:
  - automation plan or changes
  - test commands
  - evidence links
  - manual remainders
  - generated-test summary

done_when:
  - automation targets are justified by risk
  - checks have commands or explicit implementation stories
  - API/E2E checks use maintainable framework patterns
  - tests are independent and avoid hardcoded waits
  - run-and-fix result or waiver is recorded
  - evidence path is clear
  - remaining manual checks or waivers are explicit

blocked_when:
  - target behavior is not observable
  - fixtures or environment cannot be controlled
  - framework cannot run and no manual/semi-automated evidence mode is acceptable

handoff:
  - preserve framework, scenarios, API/E2E checks, fixtures, locators/assertions, commands, run result, evidence links, remaining manual checks, and gate impact
