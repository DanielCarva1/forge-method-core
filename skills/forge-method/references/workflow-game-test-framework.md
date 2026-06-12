# workflow: game-test-framework

trigger:
  - game project needs a test framework before automation
  - engine/platform test strategy is unclear

inputs:
  - engine architecture
  - game PRD or GDD
  - platform constraints
  - current test tooling

steps:
  1. choose engine-appropriate test layers
  2. define deterministic seams for mechanics, content, UI, saves, and multiplayer if relevant
  3. map manual playtest evidence to automated checks where practical
  4. record commands, fixtures, and limitations

outputs:
  - game test framework plan
  - test layer map
  - command/evidence expectations

done_when:
  - test layers and commands are explicit
  - manual vs automated coverage is clear
  - first automation target is known

blocked_when:
  - engine/tooling cannot be identified
  - no stable success signal exists

handoff:
  - preserve framework plan, commands, first automation target, and known limitations
