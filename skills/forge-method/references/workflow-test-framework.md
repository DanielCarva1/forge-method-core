# workflow: test-framework

trigger:
  - project needs a test framework or harness
  - existing checks are ad hoc or missing

inputs:
  - architecture notes
  - implementation stack
  - risk register
  - current commands

steps:
  1. define test layers, ownership, and first risk checks
  2. choose harnesses and data strategy from the accepted stack
  3. define fixture architecture: pure helper, framework wrapper, composition surface, cleanup, and evidence
  4. record command contract, limitations, and maintenance rules

outputs:
  - test framework plan
  - fixture architecture
  - first-check backlog
  - command contract

done_when:
  - test layers and commands are explicit
  - fixture architecture is framework-neutral or stack-bound by decision
  - first checks are tied to risk
  - limitations are documented

blocked_when:
  - implementation stack is unknown
  - no executable surface exists

handoff:
  - preserve framework plan, fixture architecture, command contract, first checks, and limitations
