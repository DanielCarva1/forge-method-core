# workflow: problem-solving

trigger:
  - user has a stuck, ambiguous, or messy problem
  - direct implementation would be premature

inputs:
  - problem statement
  - symptoms
  - constraints
  - known failed attempts

steps:
  1. separate symptom from cause
  2. generate candidate explanations
  3. choose tests or probes
  4. define the next reversible action

outputs:
  - clarified problem
  - candidate causes
  - next test or action

done_when:
  - next action is reversible
  - assumptions are explicit
  - success signal is known

blocked_when:
  - problem has no observable signal
  - required evidence is inaccessible

handoff:
  - preserve problem frame, hypotheses, probes, and next action
