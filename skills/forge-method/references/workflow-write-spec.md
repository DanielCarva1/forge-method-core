# workflow: write-spec

trigger:
  - state.phase == 2-specification
  - discovery intent exists

inputs:
  - intent artifact
  - constraints
  - success criteria
  - relevant domain notes

steps:
  1. define the target outcome
  2. define functional requirements
  3. define non-functional requirements
  4. define acceptance criteria
  5. identify risks and open assumptions
  6. save spec artifact
  7. update state next action toward planning

outputs:
  - specification artifact
  - acceptance criteria
  - risk list
  - updated state

done_when:
  - requirements are testable
  - acceptance criteria can become stories
  - unresolved assumptions are explicit

blocked_when:
  - a requirement conflicts with a stated constraint
  - acceptance criteria cannot be tested or inspected

handoff:
  - preserve spec path, assumptions, and planning inputs

