# workflow: gdd

trigger:
  - game brief exists
  - game needs a durable design document

inputs:
  - game brief
  - mechanics notes
  - narrative notes
  - technical constraints

steps:
  1. define pillars, loop, systems, content, and progression
  2. capture controls, UX, and feedback rules
  3. list prototype and vertical slice requirements
  4. save GDD artifact

outputs:
  - GDD artifact
  - vertical slice scope
  - design risks

done_when:
  - core systems are explicit
  - vertical slice is bounded
  - build stories can reference GDD sections

blocked_when:
  - core mechanic is unresolved
  - target platform changes design constraints

handoff:
  - preserve GDD path, pillars, slice scope, risks, and next story
