# workflow: product-requirements

trigger:
  - state.phase == 2-specification
  - product intent exists

inputs:
  - discovery artifact
  - target users
  - constraints
  - success criteria

steps:
  1. define problem, users, goals, non-goals, and constraints
  2. write functional requirements
  3. write acceptance criteria
  4. capture risks and open assumptions
  5. save product requirements artifact

outputs:
  - product requirements artifact
  - acceptance criteria
  - open assumptions

done_when:
  - requirements are testable
  - non-goals and constraints are explicit
  - next architecture or planning workflow is known

blocked_when:
  - target user is unknown
  - success definition changes the product shape

handoff:
  - preserve requirements path, assumptions, risks, and next workflow
