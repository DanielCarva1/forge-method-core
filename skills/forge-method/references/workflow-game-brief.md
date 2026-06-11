# workflow: game-brief

trigger:
  - user starts a game project
  - game direction is not yet durable

inputs:
  - game idea
  - target player
  - platform or engine preference
  - constraints

steps:
  1. define player fantasy and core loop
  2. choose genre, platform, and prototype scope
  3. identify risks and proof needs
  4. save game brief

outputs:
  - game brief
  - prototype scope
  - risk list

done_when:
  - player, loop, and prototype target are explicit
  - engine assumption is recorded
  - next game workflow is known

blocked_when:
  - target player is unknown
  - prototype scope is too broad

handoff:
  - preserve brief path, player fantasy, core loop, engine, and next workflow
