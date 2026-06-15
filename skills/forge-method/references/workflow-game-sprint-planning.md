# workflow: game-sprint-planning

trigger:
  - user asks for game sprint planning
  - accepted brief, GDD, prototype, playtest, or architecture needs story order
  - next playable slice must become an executable story batch

inputs:
  - game brief, GDD, prototype, playtest, architecture, or prior sprint status
  - playable slice goal
  - candidate stories, risks, dependencies, engine or asset constraints
  - validation and manual playtest expectations

steps:
  1. choose the playable_slice_goal and decision_sources
  2. order stories by player_value_order, risk_order, dependencies, and proof value
  3. separate deferred_scope, blocked_items, and engine_or_asset_constraints
  4. define validation_plan, manual_playtest_plan, next_story, and sprint_update
  5. write game-sprint-plan-artifact and run artifact game-check
  6. route to game-story-creation, build-story, quick-prototype, or game-sprint-status

outputs:
  - playable slice sprint plan
  - story batch and next story
  - validation and playtest plan
  - deferred scope and blocked items

done_when:
  - story order protects the player fantasy and playable slice goal
  - every ready story has decision sources and validation evidence expectations
  - deferred and blocked scope are explicit
  - artifact game-check passes

blocked_when:
  - playable slice goal is unclear
  - decision sources are missing or contradictory
  - validation cannot prove player-facing success

handoff:
  - preserve plan path, playable slice goal, decision sources, ordered story batch, deferred scope, validation plan, next story, sprint update, and next workflow
