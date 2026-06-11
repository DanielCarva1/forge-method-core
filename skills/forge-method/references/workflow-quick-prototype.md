# workflow: quick-prototype

trigger:
  - game or product idea needs a fast proof
  - scope should stay intentionally small

inputs:
  - prototype goal
  - success signal
  - constraints
  - available assets or code

steps:
  1. define the smallest playable or usable proof
  2. create prototype story
  3. run the proof check
  4. record outcome and next decision

outputs:
  - prototype scope
  - prototype story
  - result evidence

done_when:
  - proof target is testable
  - outcome is recorded
  - next decision is known

blocked_when:
  - prototype cannot be tested
  - scope exceeds one focused proof

handoff:
  - preserve prototype goal, story, result, and next decision
