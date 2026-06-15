# workflow: test-framework

trigger:
  - project needs a test framework or harness
  - existing checks are ad hoc or missing

inputs:
  - architecture notes
  - implementation stack
  - risk register
  - current commands
  - package/config files and existing tests

steps:
  1. detect framework from package/config/test files or record the stack-based recommendation
  2. define test layers, ownership, and first risk checks
  3. choose harnesses and data strategy from the accepted stack
  4. define fixture architecture: pure helper, framework wrapper, composition surface, cleanup, and evidence
  5. record command contract, semantic locator policy, limitations, and maintenance rules
  6. run artifact test-framework with fixture architecture, commands, evidence, repair policy, and next_workflow
  7. run `artifact test-check --path <test-framework-artifact>`

outputs:
  - test framework plan
  - fixture architecture
  - first-check backlog
  - command contract
  - framework detection proof

done_when:
  - framework detection or recommendation is explicit
  - test layers and commands are explicit
  - fixture architecture is framework-neutral or stack-bound by decision
  - first checks are tied to risk
  - artifact test-framework registered the durable framework artifact
  - test-check proof passes or waiver is explicit
  - limitations are documented

blocked_when:
  - implementation stack is unknown
  - no executable surface exists
  - framework cannot be detected and no recommendation can be justified

handoff:
  - preserve framework detection, fixture architecture, command contract, semantic locator policy, first checks, limitations, and validation command
