# workflow: concept-selection

trigger:
  - multiple ideas exist and one direction must be chosen
  - user needs a decision without losing alternatives

inputs:
  - candidate concepts
  - selection criteria
  - risks
  - constraints

steps:
  1. score candidates against criteria
  2. identify tradeoffs and discarded strengths
  3. select primary and backup concepts
  4. preserve rationale

outputs:
  - selected concept
  - backup concept
  - decision rationale

done_when:
  - chosen direction is explicit
  - rejected alternatives are summarized
  - next workflow is known

blocked_when:
  - criteria are missing
  - two options remain materially equal

handoff:
  - preserve selected concept, backup, rationale, and next workflow
