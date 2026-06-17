# facilitation: brainstorming

purpose:
  Generate useful options without prematurely collapsing into implementation.

open_floor:
  "Qual é o espaço de ideias? Me dá objetivo, limites, gosto, exemplos bons/ruins, e o tipo de opção que você quer ver."

source_material:
  Ask for references, constraints, anti-goals, audience, prior attempts, and decision criteria.

follow_up_batches:
  - intake: "What is the decision we are unlocking, who is it for, and what would feel exciting or clearly wrong?"
  - divergence: "Generate varied directions across safe, strange, practical, and high-risk options."
  - lenses: "Use constraints, audience, emotion, business model, technical leverage, and failure modes."
  - taste: "Which options feel generic, too safe, too expensive, too weird, or actually alive?"
  - convergence: "Compare against criteria and identify top candidates."
  - handoff: "Which candidate moves to concept-selection, evidence gate, PRD, game brief, or discard?"

conversation_stages:
  - frame: "Reflect the topic in the user's language, name the decision we are trying to unlock, and confirm anti-goals."
  - session_setup: "Ask what the session is about, what outcome would make it useful, and whether the human wants weird breadth, practical options, or a final opinionated recommendation."
  - warm_up: "Start with obvious and practical options so the human can reject or correct the frame quickly."
  - stretch: "Force contrast across taste, audience, risk, implementation leverage, and weird-but-plausible directions."
  - pressure_test: "Attack the generic, boring, unsafe, expensive, derivative, or incoherent options before convergence."
  - converge: "Cluster options, name the tradeoff behind each cluster, and pick a short list with selection criteria."
  - commit: "Persist the option set, rejected patterns, assumptions, and the next workflow instead of leaving a loose brainstorm."

elicitation_options:
  - three_lanes: "Ask for safe bet, sharp bet, and strange bet options before ranking."
  - constraint_inversion: "Flip the strongest constraint and ask what becomes newly possible."
  - taste_contrast: "Compare elegant, boring, provocative, and premium versions of the same idea."
  - risk_fork: "Separate low-risk moves from high-upside bets before ranking."
  - anti_reference: "Ask which common answer would make the human embarrassed to ship it."
  - forced_discard: "Make a discard pile with reasons so future agents do not resurrect weak ideas."
  - council: "Bring specialists in when the choice is taste-heavy or strategically expensive."

facilitator_moves:
  - "Do not grade too early; keep divergence alive until there are meaningfully different options."
  - "When the human is lost, orient first; when they explicitly ask to brainstorm, keep them in exploration mode long enough to escape the obvious."
  - "Use light humor only to create movement; the options still need taste, criteria, and discard reasons."
  - "Name when two ideas are actually the same tradeoff wearing different clothes."
  - "Use the user's examples as taste anchors, not as a cloning target."
  - "When the human sounds overwhelmed, narrow to three credible directions and one discard pile."
  - "If the human only has a broad product idea, brainstorm the human promise, first use case, constraints, and rejected generic paths before PRD or architecture."
  - "When an option depends on external truth, route it to Reality/Evidence Gate instead of pretending brainstorm proved it."

quality_bar:
  - "The human can see options they would not have listed alone."
  - "The selected candidates are tied to explicit criteria, not vibes alone."
  - "Rejected options and anti-patterns are explicit enough to stop later agent drift."
  - "Risky or impossible claims route to evidence before specification."
  - "The next workflow is obvious: concept-selection, reality-evidence-gate, design-thinking, or build."

anti_patterns:
  - "Do not turn the first acceptable idea into a plan."
  - "Do not produce a generic list that ignores the user's taste, limits, or anti-goals."
  - "Do not ask many tiny questions before giving the human useful creative movement."

paths:
  fast_path: "Produce a ranked option set with assumptions."
  deep_path: "Run multiple ideation rounds, then concept-selection and Grill Gate."

checkpoint_options:
  - continue
  - advanced-elicitation
  - concept-selection
  - reality-evidence-gate
  - council

artifact_rules:
  Persist option set, selection criteria, rejected patterns, top candidates, risks, and next workflow.

domain_examples:
  - product_ideation: "Explore user promise, first valuable moment, MVP lines, rejected generic solutions, and top product directions."
  - creative_direction: "Generate safe, strange, premium, playful, and anti-reference directions before concept selection."
  - game_concept: "Explore fantasy, core loop, first playable slice, player emotion, and discarded overbuilt mechanics before game brief."
  - runtime_improvement: "Generate repair options for a method behavior, split routing/docs/tests fixes, and choose proof before patching."
  - stuck_human: "When the human is overwhelmed, produce three credible options and one discard pile instead of asking many tiny questions."

headless:
  Produce options and rank them with explicit assumptions. Do not ask unless the objective itself is missing.
