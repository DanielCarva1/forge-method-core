# workflow: game-prd

trigger:
  - game concept needs production requirements before planning
  - GDD needs conversion into implementation constraints

inputs:
  - game brief
  - MDA Trace
  - GDD or mechanics notes
  - platform constraints
  - production scope

steps:
  1. translate game pillars and MDA Trace into functional requirements
  2. split requirements into player, content, systems, platform, and ops needs
  3. mark MVP, later, and rejected scope
  4. define measurable acceptance and evidence expectations

outputs:
  - game PRD
  - MDA-backed requirement map
  - scoped requirement list
  - acceptance/evidence map

done_when:
  - player-experience requirements trace to target aesthetics, dynamics, mechanics, and proof
  - MVP requirements are explicit
  - non-MVP scope is parked
  - stories can be created from requirements

blocked_when:
  - game pillars are missing
  - desired player experience cannot be converted into testable requirements
  - scope cannot be reduced to a playable increment

handoff:
  - preserve PRD path, MDA Trace, MVP scope, parked scope, and acceptance/evidence map
