# workflow: agent-analyze

trigger:
  - user asks to analyze an agent profile, skill, or persona behavior
  - generated agent behavior needs quality review before use

inputs:
  - agent profile or skill
  - intended workflow
  - constraints and failure cases
  - validation expectations

steps:
  1. inspect role, triggers, permissions, and handoff
  2. test against expected and adversarial scenarios
  3. identify missing state, unclear boundaries, and unsafe autonomy
  4. recommend patch, eval, or rejection

outputs:
  - agent analysis
  - findings
  - recommended changes

done_when:
  - behavior risks are explicit
  - required changes or evals are known
  - ownership boundary is clear

blocked_when:
  - source agent artifact is unavailable
  - intended behavior is undefined

handoff:
  - preserve analyzed path, findings, recommendation, and validation command
