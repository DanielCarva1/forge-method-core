# workflow: engine-architecture

trigger:
  - game needs engine or technical architecture decisions
  - prototype or vertical slice depends on tooling

inputs:
  - game brief or GDD
  - target platform
  - engine preference
  - engine setup artifact if present
  - technical constraints

steps:
  1. choose engine path or confirm current engine profile
  2. verify setup assumptions or route engine-setup first
  3. define project structure, game loop, core systems, data/content boundaries, and integration points
  4. identify performance, asset, save, input, and platform constraints
  5. save engine architecture artifact

outputs:
  - engine architecture artifact
  - engine profile
  - technical risks
  - prototype constraints

done_when:
  - engine decision is explicit
  - core systems are mapped
  - performance budget is measurable
  - build setup story is known

blocked_when:
  - engine choice requires human decision
  - target platform is unknown

handoff:
  - preserve engine profile, architecture path, constraints, performance budget, and setup/prototype story
