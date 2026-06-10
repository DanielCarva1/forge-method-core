# workflow: game-project

trigger:
  - user starts a game project
  - state.module == game-studio

inputs:
  - game idea
  - engine preference
  - platform target
  - core mechanic
  - art/audio constraints

steps:
  1. define game type and target player
  2. define core mechanic
  3. create prototype goal
  4. create game brief
  5. create or update GDD
  6. create technical architecture for selected engine
  7. plan vertical slice
  8. move implementation tasks into sprint

outputs:
  - game brief
  - GDD
  - engine architecture
  - vertical slice plan
  - sprint stories

done_when:
  - game concept, mechanic, and target engine are explicit
  - first playable/vertical slice has acceptance criteria
  - implementation stories exist

blocked_when:
  - engine choice changes architecture materially
  - playable target is undefined
  - required assets are unavailable

