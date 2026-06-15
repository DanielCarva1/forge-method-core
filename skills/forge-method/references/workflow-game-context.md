# workflow: game-context

trigger:
  - game project needs source-of-truth context for future agents
  - game artifacts exist but the next workflow lacks player/engine/slice handoff

inputs:
  - game brief, GDD, mechanics, narrative, UX, PRD, architecture, stories, playtest notes
  - engine profile
  - playable slice target
  - validation evidence

steps:
  1. load current game artifacts and state
  2. extract player fantasy, core loop, references, engine profile, and playable slice
  3. map design artifacts to stories, checks, evidence, and open risks
  4. write game context artifact
  5. route the next game workflow or session prep

outputs:
  - game context artifact
  - source map
  - next game workflow

done_when:
  - player fantasy, loop, engine profile, playable slice, and validation proof are explicit
  - next workflow and files to load are named

blocked_when:
  - no accepted game source artifact exists
  - engine/profile or playable slice is unknown and blocks handoff

handoff:
  - preserve game context path, engine profile, playable slice, source artifacts, checks, and next workflow
