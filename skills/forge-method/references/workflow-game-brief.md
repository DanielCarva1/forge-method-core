# workflow: game-brief

trigger:
  - user starts a game project
  - game direction is not yet durable
  - user asks to create, update, validate, or brainstorm a game brief

inputs:
  - full game idea, references, constraints, fears, and anti-goals
  - target player and platform or engine assumption
  - source material, research notes, prototypes, or campaign material
  - existing brief when updating or validating

steps:
  1. capture the whole picture before narrow questions
  2. extract player_fantasy, core_loop, player_verbs, target_player, pillars, and references
  3. separate dream_game, vertical_slice, mvp_playable_proof, parked_scope, and rejected_directions
  4. record assumptions, open_questions, research_needed, and decision_log
  5. write game-brief-artifact and run artifact game-check
  6. route to game-context, gdd, quick-prototype, research, or game-sprint-planning

outputs:
  - living game brief
  - decision log
  - parked scope and rejected directions
  - playable proof target and next workflow

done_when:
  - player fantasy, core loop, verbs, pillars, target player, and platform/engine are explicit
  - smallest playable proof is concrete enough to guide prototype or sprint planning
  - assumptions and open questions are marked instead of invented
  - artifact game-check passes

blocked_when:
  - target player or player fantasy is unknown
  - prototype scope is too broad to name a playable proof
  - required legal, domain, market, or technical research is unresolved

handoff:
  - preserve brief path, source material, player fantasy, core loop, playable proof, decision log, parked scope, assumptions, open questions, validation verdict, and next workflow
