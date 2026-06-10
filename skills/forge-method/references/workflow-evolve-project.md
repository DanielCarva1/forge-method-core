# workflow: evolve-project

trigger:
  - state.phase == 6-evolve
  - ready project receives feedback, defects, or new intent

inputs:
  - current ready state
  - feedback or defect
  - existing artifacts
  - recent evidence

steps:
  1. classify the change as defect, enhancement, pivot, or new module
  2. decide whether discovery, specification, planning, or direct build is required
  3. create new stories or update artifacts
  4. transition to the earliest phase that can handle the change safely

outputs:
  - change classification
  - new or updated stories
  - updated state

done_when:
  - next phase is explicit
  - state contains the next action
  - no feedback item remains unclassified

blocked_when:
  - feedback contradicts current ready state
  - requested change would invalidate core constraints

handoff:
  - preserve classification and next phase

