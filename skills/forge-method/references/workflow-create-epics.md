# workflow: create-epics

trigger:
  - requirements are ready for planning
  - architecture or UX constraints must shape stories

inputs:
  - requirements artifact
  - architecture artifact
  - UX plan artifact
  - validation strategy

steps:
  1. group requirements into epics
  2. define story boundaries and dependencies
  3. attach acceptance criteria and checks
  4. write epics artifact
  5. import or update story backlog

outputs:
  - epics artifact
  - story backlog
  - validation map

done_when:
  - every story has acceptance criteria
  - dependencies are explicit
  - first ready story is known

blocked_when:
  - architecture changes story boundaries
  - validation cannot be assigned

handoff:
  - preserve epics path, backlog path, dependencies, and first story
