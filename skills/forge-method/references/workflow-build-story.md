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
  - decision sources
  - domain context when the active track has one
  - relevant source files
  - required checks

steps:
  1. select the next ready story
  2. restate scope in one short paragraph
  3. inspect only files needed for the story
  4. preserve domain-specific acceptance, proof, and non-goals
  5. implement the smallest correct change
  6. run required checks
  7. perform code review
  8. record review findings with `review add`
  9. repair failures or review findings
  10. resolve or waive review findings
  11. write evidence
  12. update sprint status
  13. continue to the next ready story or ready gate without procedural confirmation
  14. delete ephemeral task docs only after evidence is recorded

outputs:
  - code changes
  - review findings or explicit clean review
  - check results
  - evidence entry
  - updated sprint
  - updated state

done_when:
  - all acceptance criteria are satisfied
  - required checks pass or documented exceptions are accepted
  - linked review findings are resolved or waived
  - evidence is written
  - sprint status is updated
  - next story or ready gate is explicit

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
  - never ask for procedural ok/continue between mechanical steps
