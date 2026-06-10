# workflow: plan-sprint

trigger:
  - state.phase == 3-plan
  - specification artifact exists

inputs:
  - specification artifact
  - acceptance criteria
  - current repository structure
  - known checks

steps:
  1. split acceptance criteria into stories
  2. identify dependencies and risks
  3. create or update story files
  4. define validation commands or inspection checks
  5. update sprint summary
  6. move ready implementation work into phase 4

outputs:
  - story files
  - sprint summary
  - validation plan
  - updated state

done_when:
  - each executable story has acceptance criteria
  - next ready story is known
  - checks are known or explicitly marked manual

blocked_when:
  - architecture choice changes story boundaries materially
  - validation cannot be defined

handoff:
  - preserve next story, validation plan, and unresolved risks

