# workflow: code-review

trigger:
  - user asks to review code, a diff, or an implementation
  - story/release needs findings before repair or readiness

inputs:
  - changed files or diff
  - linked story and acceptance criteria
  - requirements, architecture, UX, risk, and validation evidence
  - tests and commands

steps:
  1. identify changed surface and expected behavior
  2. inspect for bugs, regressions, missing evidence, and weak tests
  3. classify findings by severity and actionability
  4. create durable review findings when blocking or repairable issues exist
  5. write code review artifact and route repair, readiness-check, or retrospective

outputs:
  - code review findings
  - durable review findings when needed
  - repair or gate recommendation

done_when:
  - actionable findings are explicit and evidenced
  - no finding is hidden in prose only
  - next workflow is repair, readiness-check, or closure

blocked_when:
  - changed surface cannot be identified
  - acceptance criteria or expected behavior are unavailable

handoff:
  - preserve findings, severity, affected files, evidence, commands reviewed, and next workflow
