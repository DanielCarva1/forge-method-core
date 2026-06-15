# workflow: quick-prototype

trigger:
  - game or product idea needs a fast proof
  - scope should stay intentionally small

inputs:
  - prototype goal
  - success signal
  - constraints
  - engine profile
  - available assets or code

steps:
  1. define the smallest playable proof
  2. choose asset stubs and engine/runtime assumptions
  3. create prototype story and proof check
  4. run or specify the proof check
  5. record outcome and next decision

outputs:
  - prototype scope
  - prototype story
  - proof command or manual check
  - result evidence

done_when:
  - proof target is testable
  - outcome is recorded
  - next decision is known

blocked_when:
  - prototype cannot be tested
  - scope exceeds one focused proof

handoff:
  - preserve prototype goal, player action, story, proof check, result, and next decision
