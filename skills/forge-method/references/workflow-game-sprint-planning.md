# workflow: game-sprint-planning

trigger:
  - user asks for game sprint planning
  - accepted brief, GDD, prototype, playtest, or architecture needs story order
  - next playable slice must become an executable story batch

inputs:
  - brief, GDD, prototype, playtest, architecture, or sprint status
  - playable slice goal
  - stories, risks, dependencies, engine/asset constraints
  - validation and playtest expectations

steps:
  1. choose the playable_slice_goal and decision_sources
  2. order stories by player_value_order, risk_order, dependencies, and proof value
  3. separate deferred_scope, blocked_items, and engine_or_asset_constraints
  4. define validation_plan, manual_playtest_plan, next_story, and sprint_update
  5. run artifact game-sprint-plan with required fields and next_workflow
  6. run artifact game-check --path <game-sprint-plan-artifact>
  7. route to game-story-creation, build-story, quick-prototype, or game-sprint-status

outputs:
  - playable slice sprint plan
  - story batch and next story
  - validation and playtest plan
  - deferred scope and blocked items

done_when:
  - story order protects the player fantasy and playable slice goal
  - ready stories have decision sources and validation expectations
  - deferred and blocked scope are explicit
  - artifact game-sprint-plan registered the durable sprint plan
  - artifact game-check passes

blocked_when:
  - playable slice goal is unclear
  - decision sources are missing or contradictory
  - validation cannot prove player-facing success

handoff:
  - preserve plan path, slice goal, sources, story order, deferred scope, validation, next story, sprint update, and next workflow
