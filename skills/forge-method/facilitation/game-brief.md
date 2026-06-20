# facilitation: game-brief

purpose:
  Turn a game idea into a brief the user recognizes as their own, before GDD, architecture, or implementation.

open_floor:
  "Antes de eu fazer perguntas pequenas: despeja a ideia inteira. Qual é a fantasia do jogador, onde joga, o que já existe na sua cabeça, e o que você não quer que isso vire?"

source_material:
  Ask for pitch decks, notes, prototypes, reference games, mechanics lists, art direction, campaign material, or constraints.

follow_up_batches:
  - core_fantasy: "What should the player feel and pretend to be doing?"
  - loop: "What does the player repeatedly do, decide, risk, and earn?"
  - verbs: "Which concrete player verbs prove the fantasy in minute-to-minute play?"
  - pillars: "Which 2-4 pillars must survive scope cuts?"
  - audience_platform: "Who is this for, and where does it run?"
  - scope_mvp: "What is the smallest playable proof of the core hypothesis?"
  - visual_preview: "Which table, HUD, map, character sheet, scene, or first screen should the human see before the brief becomes GDD or stories?"
  - references: "Which comparable games matter, and what are we taking or refusing from each?"
  - living_brief: "Are we creating, updating, validating, or handing off the brief?"

conversation_stages:
  - whole_picture: "Let the human dump the fantasy, references, jokes, constraints, and fears before asking narrow design questions."
  - still_in_head: "Ask what else is still in their head before narrowing; late details often contain the real taste signal."
  - mode_choice: "Offer fast path with [ASSUMPTION] tags or coaching path with step-by-step pressure, based on urgency and energy."
  - taste_calibration: "Mirror the intended feeling, audience, and reference games; ask what must be borrowed, inverted, or avoided."
  - preview_options: "Show one narrow visual proof or 2-3 contrasting table/screen/play examples when the direction is still broad."
  - play_shape: "Extract player fantasy, repeated loop, verbs, risks, rewards, and the moment-to-moment promise."
  - scope_cut: "Separate dream game, vertical slice, and smallest playable proof without shaming ambition."
  - living_contract: "Record decision log, rejected directions, assumptions, open questions, and research needed."
  - commit: "Run artifact game-brief, run artifact game-check, and choose next workflow."

elicitation_options:
  - reference_compare: "Pick two reference games and ask what to take, refuse, and improve."
  - player_fantasy_ladder: "Ask what the player feels in minute 1, hour 1, and after mastery."
  - scope_knife: "Cut features until the core loop is still playable and recognizable."
  - council: "Use council when genre, audience, tech, or taste tradeoffs are expensive."

facilitator_moves:
  - "Protect the emotional seed of the game while challenging impossible scope."
  - "Do not accelerate unless the human explicitly says this is simple, urgent, or time-boxed."
  - "When energy is high, mirror it with sharper language and useful humor; never turn frustration into apology theater."
  - "Replace genre labels with concrete player verbs and decisions."
  - "Name the difference between content depth and a playable core."
  - "When the idea is huge, park expansions instead of deleting them."
  - "Treat the brief as a living workspace: update and validate instead of rewriting from scratch."
  - "If reference/domain/technical risk changes the brief, route research before pretending the brief is accepted."

quality_bar:
  - "The human recognizes the game as their game, not a generic genre template."
  - "The brief can feed GDD, UX, PRD, prototype, and story creation without re-discovery."
  - "The MVP proves the core fantasy instead of a random technical slice."
  - "The human sees enough of the table/screen/play shape to correct fantasy, usability, and immersion before downstream work."
  - "The artifact names player fantasy, loop, verbs, pillars, references, playable proof, parked scope, decision log, assumptions, open questions, and next workflow."
  - "artifact game-brief registers the living brief before downstream game production uses it."
  - "artifact game-check passes before the brief becomes input to GDD, prototype, or sprint planning."

anti_patterns:
  - "Do not jump to engine, architecture, or implementation before the player fantasy and loop are clear."
  - "Do not treat reference games as a cloning checklist."
  - "Do not invent lore, mechanics, or audience facts that the human did not provide."
  - "Do not call a game brief done when it only names genre, engine, and feature list."
  - "Do not delete ambitious ideas; park them with revisit triggers."

paths:
  fast_path: "Draft/update the brief with [ASSUMPTION] tags, run artifact game-brief, run artifact game-check, and ask for correction."
  deep_path: "Run domain/market/technical research, compare references, validate the living brief, then route GDD, prototype, or game-sprint-planning."

checkpoint_options:
  - continue
  - game-sprint-planning
  - quick-prototype
  - visual-alignment-prototype
  - gdd
  - domain-scan
  - council

artifact_rules:
  Use artifact game-brief for durable brief output; persist source_material, player_fantasy, core_loop, player_verbs, pillars, references, first_visual_preview, dream_game, vertical_slice, mvp_playable_proof, parked_scope, rejected alternatives, decision_log, assumptions, open_questions, validation_verdict, and next_workflow.

domain_examples:
  - tabletop: "Fantasy, loop, rules surface, player roles, table flow, reference systems, first playable session, and parked sourcebook depth."
  - tactics: "Minute-one decision, combat loop, feedback, unit verbs, map constraints, first playable encounter, and balance proof."
  - narrative: "Player role, premise, dialogue/content units, emotional arc, failure states, first scene, and rejected lore sprawl."

headless:
  Create/update/validate based on available material, run game-check when possible, and return artifact paths plus open questions; do not invent unavailable source facts.
