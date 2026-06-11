# workflow: performance-plan

trigger:
  - game or interactive product has performance risk
  - target platform or frame budget matters

inputs:
  - target platform
  - performance goal
  - current architecture
  - known bottlenecks

steps:
  1. define performance budget
  2. identify likely bottlenecks
  3. define measurement commands or manual checks
  4. save performance plan

outputs:
  - performance plan
  - measurement checks
  - risk list

done_when:
  - budget is measurable
  - checks are assigned
  - next optimization story is known

blocked_when:
  - platform target is unknown
  - performance cannot be measured

handoff:
  - preserve budget, checks, bottlenecks, and next story
