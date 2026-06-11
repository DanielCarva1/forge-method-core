# workflow: engine-architecture

trigger:
  - game needs engine or technical architecture decisions
  - prototype or vertical slice depends on tooling

inputs:
  - game brief or GDD
  - target platform
  - engine preference
  - technical constraints

steps:
  1. choose engine path or confirm current engine
  2. define project structure and core systems
  3. identify performance and asset constraints
  4. save engine architecture artifact

outputs:
  - engine architecture artifact
  - technical risks
  - prototype constraints

done_when:
  - engine decision is explicit
  - core systems are mapped
  - build setup story is known

blocked_when:
  - engine choice requires human decision
  - target platform is unknown

handoff:
  - preserve engine decision, architecture path, constraints, and setup story
