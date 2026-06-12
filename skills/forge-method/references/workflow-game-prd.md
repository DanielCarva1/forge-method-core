# workflow: game-prd

trigger:
  - game concept needs production requirements before planning
  - GDD needs conversion into implementation constraints

inputs:
  - game brief
  - GDD or mechanics notes
  - platform constraints
  - production scope

steps:
  1. translate game pillars into functional requirements
  2. split requirements into player, content, systems, platform, and ops needs
  3. mark MVP, later, and rejected scope
  4. define measurable acceptance and evidence expectations

outputs:
  - game PRD
  - scoped requirement list
  - acceptance/evidence map

done_when:
  - MVP requirements are explicit
  - non-MVP scope is parked
  - stories can be created from requirements

blocked_when:
  - game pillars are missing
  - scope cannot be reduced to a playable increment

handoff:
  - preserve PRD path, MVP scope, parked scope, and acceptance/evidence map
