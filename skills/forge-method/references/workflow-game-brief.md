# workflow: game-brief

trigger:
  - user starts a game project
  - game direction is not yet durable
  - user asks to create, update, validate, or brainstorm a game brief

inputs:
  - full game idea, references, constraints, fears, anti-goals
  - target player, platform/engine, source material
  - existing brief when updating or validating

steps:
  1. capture the whole picture before narrow questions
  2. extract player_fantasy, core_loop, player_verbs, target_player, pillars, and references
  3. separate dream_game, vertical_slice, mvp_playable_proof, parked_scope, and rejected_directions
  4. record assumptions, open_questions, research_needed, and decision_log
  5. run artifact game-brief with required fields and next_workflow
  6. run artifact game-check --path <game-brief-artifact>
  7. route to game-context, gdd, quick-prototype, research, or game-sprint-planning

outputs:
  - living game brief
  - decision log
  - parked scope and rejected directions
  - playable proof target and next workflow

done_when:
  - fantasy, loop, verbs, pillars, target, and platform/engine are explicit
  - playable proof can guide prototype or sprint planning
  - assumptions and open questions are marked instead of invented
  - artifact game-brief registered the durable brief
  - artifact game-check passes

blocked_when:
  - target player or player fantasy is unknown
  - prototype scope is too broad to name a playable proof
  - required legal, domain, market, or technical research is unresolved

handoff:
  - preserve brief path, source material, fantasy, loop, playable proof, decisions, parked scope, assumptions, questions, verdict, and next workflow
