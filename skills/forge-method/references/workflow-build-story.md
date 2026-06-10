# workflow: build-story

trigger:
  - state.phase == 4-build-verify
  - sprint has a ready story
  - user asks Codex to continue autonomous development

inputs:
  - `.forge-method/state.yaml`
  - `.forge-method/sprint.yaml`
  - active story
  - acceptance criteria
  - relevant source files
  - required checks

steps:
  1. select the next ready story
  2. restate scope in one short paragraph
  3. inspect only files needed for the story
  4. implement the smallest correct change
  5. run required checks
  6. perform code review
  7. repair failures or review findings
  8. write evidence
  9. update sprint status
  10. delete ephemeral task docs only after evidence is recorded

outputs:
  - code changes
  - check results
  - evidence entry
  - updated sprint
  - updated state

done_when:
  - all acceptance criteria are satisfied
  - required checks pass or documented exceptions are accepted
  - code review has no blocking findings
  - evidence is written
  - sprint status is updated

blocked_when:
  - missing credential
  - destructive action requires explicit user approval
  - spec contradicts itself
  - acceptance criteria are missing
  - required external service is unavailable

handoff:
  - preserve exact next action
  - preserve failing command and output summary if blocked
  - preserve touched files and evidence path

