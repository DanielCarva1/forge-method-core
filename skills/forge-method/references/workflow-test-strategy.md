# workflow: test-strategy

trigger:
  - state.phase == 3-plan
  - user asks how work will be validated
  - a story lacks checks or evidence expectations

inputs:
  - specification artifact
  - story list
  - known risks
  - available test commands

steps:
  1. identify risk areas
  2. define automated checks when possible
  3. define manual inspection checks when automation is unavailable
  4. attach checks to stories
  5. define release evidence expectations

outputs:
  - validation plan
  - story check updates
  - release gate expectations

done_when:
  - every executable story has a validation path
  - release evidence expectations are explicit
  - unavailable automation is documented

blocked_when:
  - the project lacks a way to inspect success
  - required external systems are unavailable

handoff:
  - preserve check commands, manual checks, and release evidence expectations
