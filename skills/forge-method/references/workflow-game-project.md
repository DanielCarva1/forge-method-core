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
  5. create game context handoff
  6. choose engine profile and setup path
  7. create or update GDD
  8. create technical architecture for selected engine
  9. plan playable slice and move tasks into sprint

outputs:
  - game brief
  - GDD
  - game context
  - engine setup
  - engine architecture
  - playable slice plan
  - sprint stories

done_when:
  - game concept, mechanic, and target engine are explicit
  - first playable slice has acceptance criteria and proof
  - implementation stories exist

blocked_when:
  - engine choice changes architecture materially
  - playable target is undefined
  - required assets are unavailable

handoff:
  - preserve game brief path, game context, engine profile, playable slice target, and next implementation story
