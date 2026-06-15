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
  3. connect narrative, mechanics, engine/profile assumptions, and validation proof
  4. list prototype and playable slice requirements
  5. save GDD artifact

outputs:
  - GDD artifact
  - playable slice scope
  - design risks

done_when:
  - core systems are explicit
  - playable slice is bounded
  - build stories can reference GDD sections

blocked_when:
  - core mechanic is unresolved
  - target platform changes design constraints

handoff:
  - preserve GDD path, pillars, playable slice scope, risks, proof checks, and next workflow
