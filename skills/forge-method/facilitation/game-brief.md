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
  - pillars: "Which 2-4 pillars must survive scope cuts?"
  - audience_platform: "Who is this for, and where does it run?"
  - scope_mvp: "What is the smallest playable proof of the core hypothesis?"
  - references: "Which comparable games matter, and what are we taking or refusing from each?"

conversation_stages:
  - whole_picture: "Let the human dump the fantasy, references, jokes, constraints, and fears before asking narrow design questions."
  - taste_calibration: "Mirror the intended feeling, audience, and reference games; ask what must be borrowed, inverted, or avoided."
  - play_shape: "Extract player fantasy, repeated loop, verbs, risks, rewards, and the moment-to-moment promise."
  - scope_cut: "Separate dream game, vertical slice, and smallest playable proof without shaming ambition."
  - commit: "Write the brief, decision log, parked addendum, assumptions, and next workflow."

elicitation_options:
  - reference_compare: "Pick two reference games and ask what to take, refuse, and improve."
  - player_fantasy_ladder: "Ask what the player feels in minute 1, hour 1, and after mastery."
  - scope_knife: "Cut features until the core loop is still playable and recognizable."
  - council: "Use council when genre, audience, tech, or taste tradeoffs are expensive."

facilitator_moves:
  - "Protect the emotional seed of the game while challenging impossible scope."
  - "Replace genre labels with concrete player verbs and decisions."
  - "Name the difference between content depth and a playable core."
  - "When the idea is huge, park expansions instead of deleting them."

quality_bar:
  - "The human recognizes the game as their game, not a generic genre template."
  - "The brief can feed GDD, UX, PRD, prototype, and story creation without re-discovery."
  - "The MVP proves the core fantasy instead of a random technical slice."

anti_patterns:
  - "Do not jump to engine, architecture, or implementation before the player fantasy and loop are clear."
  - "Do not treat reference games as a cloning checklist."
  - "Do not invent lore, mechanics, or audience facts that the human did not provide."

paths:
  fast_path: "Draft brief with [ASSUMPTION] tags and ask for correction."
  deep_path: "Run domain/market research, compare references, then write brief plus addendum."

checkpoint_options:
  - continue
  - quick-prototype
  - gdd
  - domain-scan
  - council

artifact_rules:
  Persist brief, decision log, parked depth/addendum, assumptions, rejected alternatives, MVP, and next workflow.

headless:
  Create/update/validate based on available material. Return artifact paths and open questions; do not invent unavailable source facts.
