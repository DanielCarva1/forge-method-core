# workflow: performance-plan

trigger:
  - game or interactive product has performance risk
  - target platform or frame budget matters

inputs:
  - target platform
  - engine profile
  - performance goal
  - current architecture
  - known bottlenecks

steps:
  1. define frame, load, memory, network, or input-latency budget
  2. identify likely bottlenecks
  3. define critical gameplay scenarios
  4. define measurement commands or manual checks
  5. save performance plan

outputs:
  - performance plan
  - measurement checks
  - critical scenarios
  - risk list

done_when:
  - budget is measurable
  - checks are assigned
  - next optimization story is known

blocked_when:
  - platform target is unknown
  - performance cannot be measured

handoff:
  - preserve engine profile, budget, scenarios, checks, bottlenecks, and next story
