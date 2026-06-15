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
  1. choose quality engagement mode if not already known
  2. score risks by impact, likelihood, detectability, and late-discovery cost
  3. map each risk to unit, integration, contract, E2E, NFR, exploratory, review, or manual proof
  4. define merge, story-done, readiness, and release gates
  5. record commands, evidence paths, ownership, waivers, and next test workflow

outputs:
  - validation plan
  - risk proof map
  - story check updates
  - release gate expectations

done_when:
  - every executable story has a validation path
  - every major risk has proof or waiver
  - release evidence expectations are explicit
  - unavailable automation is documented

blocked_when:
  - the project lacks a way to inspect success
  - required external systems are unavailable

handoff:
  - preserve engagement mode, risk map, check commands, manual checks, waivers, and release evidence expectations
