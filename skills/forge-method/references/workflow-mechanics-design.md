# workflow: mechanics-design

trigger:
  - game needs rules, systems, economy, balance, or progression
  - mechanic must be testable before build

inputs:
  - game brief or GDD
  - MDA Trace
  - target player behavior
  - constraints
  - prototype target

steps:
  1. map target aesthetics to desired dynamics and supporting mechanics
  2. define rules, resources, feedback, and failure states
  3. map player decisions, progression, economy, and balance assumptions
  4. define prototype and playtest signals
  5. save mechanics artifact

outputs:
  - mechanics artifact
  - MDA Trace changes
  - balance assumptions
  - prototype and playtest tests

done_when:
  - mechanic supports a named player experience hypothesis
  - mechanic can be prototyped
  - success and failure states are clear
  - tests and evidence are defined

blocked_when:
  - target aesthetics or desired dynamics are unknown
  - player behavior target is unknown
  - rule conflicts with technical limits

handoff:
  - preserve mechanics path, MDA Trace, assumptions, playable slice, tests, and next prototype story
