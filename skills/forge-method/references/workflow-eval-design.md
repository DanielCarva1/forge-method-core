# workflow: eval-design

trigger:
  - generated or changed runtime behavior needs a local check
  - workflow, artifact, or routing behavior must be protected

inputs:
  - behavior to protect
  - target workflow or artifact
  - expected result
  - failure mode

steps:
  1. choose eval kind
  2. define target, query, and expected result
  3. write eval file
  4. run evals
  5. update evidence or story checks

outputs:
  - eval file
  - eval result
  - evidence or story check

done_when:
  - eval passes
  - failure would catch a real regression
  - story records the check

blocked_when:
  - expected result is subjective
  - target cannot be inspected locally

handoff:
  - preserve eval path, target, expected result, and check command
