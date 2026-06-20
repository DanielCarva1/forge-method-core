# workflow: game-brief

trigger:
  - user starts a game project
  - game direction is not yet durable
  - user asks to create, update, validate, or brainstorm a game brief

inputs:
  - game idea, references, constraints, fears, anti-goals
  - target player, platform/engine, source material, existing brief

steps:
  1. capture the whole picture before narrow questions
  2. extract player_fantasy, core_loop, player_verbs, target_player, pillars, references, and first_visual_preview
  3. separate dream_game, vertical_slice, mvp_playable_proof, parked_scope, and rejected_directions
  4. record assumptions, open_questions, research_needed, and decision_log
  5. run artifact game-brief, then artifact game-check
  6. route to visual-alignment-prototype, game-context, gdd, quick-prototype, research, or game-sprint-planning

outputs:
  - living game brief
  - decision log
  - parked/rejected scope
  - first visual preview, playable proof, next workflow

done_when:
  - fantasy, loop, verbs, target, platform/engine, first_visual_preview, and playable proof are explicit
  - assumptions/open questions are marked instead of invented
  - artifact game-brief registered the durable brief
  - artifact game-check passes

blocked_when:
  - target player or player fantasy is unknown
  - prototype scope is too broad to name a playable proof
  - required legal, domain, market, or technical research is unresolved

handoff:
  - preserve brief path, fantasy, loop, first visual preview, playable proof, decisions, parked scope, assumptions, questions, verdict, and next workflow
