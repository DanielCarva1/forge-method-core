# workflow: gdd

trigger:
  - game brief exists
  - game needs a durable design document

inputs:
  - game brief
  - MDA Trace
  - mechanics notes
  - narrative notes
  - technical constraints

steps:
  1. carry forward or update MDA Trace before expanding design
  2. define pillars, loop, systems, content, and progression
  3. capture controls, UX, and feedback rules
  4. connect narrative, mechanics, engine/profile assumptions, and validation proof
  5. list prototype and playable slice requirements
  6. save GDD artifact

outputs:
  - GDD artifact
  - updated or preserved MDA Trace
  - playable slice scope
  - design risks

done_when:
  - target aesthetics, dynamics, mechanics, and proof are traceable from brief to GDD
  - core systems are explicit
  - playable slice is bounded
  - build stories can reference GDD sections

blocked_when:
  - core mechanic is unresolved
  - target platform changes design constraints

handoff:
  - preserve GDD path, MDA Trace, pillars, playable slice scope, risks, proof checks, and next workflow
