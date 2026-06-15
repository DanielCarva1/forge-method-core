# workflow: test-review

trigger:
  - user asks whether tests are enough
  - story or release needs quality review

inputs:
  - changed code or artifact
  - tests and commands
  - risk register
  - acceptance criteria

steps:
  1. compare tests against acceptance and risk
  2. identify missing checks, weak assertions, and false confidence
  3. identify brittle/flaky patterns and evidence gaps
  4. classify findings by severity and recommend fix, waiver, or gate decision

outputs:
  - test review findings
  - quality score
  - gap list
  - gate recommendation

done_when:
  - test gaps are explicit
  - weak assertions or brittle patterns are named
  - severity and recommended action are recorded
  - gate recommendation is clear

blocked_when:
  - changed surface cannot be identified
  - tests or acceptance criteria are unavailable

handoff:
  - preserve findings, severity, commands reviewed, quality score, repair route, and gate recommendation
