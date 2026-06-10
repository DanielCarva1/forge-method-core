# workflow: ready-release

trigger:
  - state.phase == 4-build-verify and build scope is complete
  - user asks whether the project is ready
  - state.phase == 5-ready-operate

inputs:
  - state
  - sprint summary
  - story files
  - evidence
  - checks

steps:
  1. run runtime audit
  2. confirm no story remains in progress or review
  3. write release evidence
  4. update state to `5-ready-operate`
  5. write usage/support/future backlog notes if needed

outputs:
  - release evidence
  - ready state
  - next operate/evolve action

done_when:
  - audit passes
  - release evidence exists
  - readiness is `ready`

blocked_when:
  - audit fails
  - active work remains
  - release evidence is missing

handoff:
  - preserve release evidence path and next operate/evolve action

