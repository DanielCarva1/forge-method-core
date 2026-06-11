# workflow: mechanics-design

trigger:
  - game needs rules, systems, economy, balance, or progression
  - mechanic must be testable before build

inputs:
  - game brief or GDD
  - target player behavior
  - constraints
  - prototype target

steps:
  1. define rules, resources, feedback, and failure states
  2. map progression and balance assumptions
  3. define prototype tests
  4. save mechanics artifact

outputs:
  - mechanics artifact
  - balance assumptions
  - prototype tests

done_when:
  - mechanic can be prototyped
  - success and failure states are clear
  - tests are defined

blocked_when:
  - player behavior target is unknown
  - rule conflicts with technical limits

handoff:
  - preserve mechanics path, assumptions, tests, and next prototype story
