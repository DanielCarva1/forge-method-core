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
  1. define test layers and ownership
  2. choose harnesses, fixtures, data strategy, and command shape
  3. map high-risk behavior to first checks
  4. record setup, limitations, and maintenance rules

outputs:
  - test framework plan
  - first-check backlog
  - command contract

done_when:
  - test layers and commands are explicit
  - first checks are tied to risk
  - limitations are documented

blocked_when:
  - implementation stack is unknown
  - no executable surface exists

handoff:
  - preserve framework plan, command contract, first checks, and limitations
