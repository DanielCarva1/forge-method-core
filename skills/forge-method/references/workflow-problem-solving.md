# workflow: problem-solving

trigger:
  - user has a stuck, ambiguous, or messy problem
  - user is frustrated but has not identified the wrong route yet
  - direct implementation would be premature

inputs:
  - raw user description
  - problem statement
  - symptoms
  - constraints
  - known failed attempts

steps:
  1. capture current vs desired behavior
  2. bound where the problem appears and does not appear
  3. separate symptoms, likely causes, and unknowns
  4. generate candidate explanations
  5. choose one reversible probe or repair
  6. define success signal and next workflow

outputs:
  - problem-solving artifact
  - current vs desired frame
  - candidate causes
  - chosen probe
  - next action

done_when:
  - next action is reversible
  - assumptions are explicit
  - success signal is known

blocked_when:
  - problem has no observable signal
  - required evidence is inaccessible

handoff:
  - preserve problem frame, hypotheses, probes, and next action
