# workflow: game-brief

trigger:
  - user starts a game project
  - game direction is not yet durable
  - user asks to create, update, validate, or brainstorm a game brief

inputs:
  - idea, references, constraints, fears, anti-goals
  - target player, platform/engine, source material, existing brief

steps:
  1. capture the whole picture before narrow questions
  2. extract player_fantasy, loop, verbs, target, pillars, refs, and first_visual_preview
  3. build mda_trace fields: aesthetics, experience hypothesis, dynamics, mechanics, feedback/UI, proof, risks
  4. split dream_game, vertical_slice, playable proof, parked_scope, and rejects
  5. record assumptions, questions, research_needed, and decision_log
  6. run artifact game-brief, then artifact game-check
  7. route visual, context, GDD, prototype, research, or sprint

outputs:
  - living game brief
  - MDA Trace
  - decision log
  - parked/rejected scope
  - visual preview, playable proof, next workflow

done_when:
  - fantasy, loop, verbs, target, platform/engine, visual, and proof are explicit
  - MDA Trace links feeling, dynamics, mechanics, feedback, and proof
  - assumptions/questions are marked instead of invented
  - artifact game-brief registered the durable brief
  - artifact game-check passes

blocked_when:
  - target player or player fantasy is unknown
  - feeling cannot connect to dynamics or mechanics
  - scope is too broad to name playable proof
  - required research is unresolved

handoff:
  - preserve brief path, fantasy, loop, MDA Trace, visual, proof, decisions, parked scope, assumptions, questions, verdict, and next workflow
