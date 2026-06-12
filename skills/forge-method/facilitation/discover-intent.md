# facilitation: discover-intent

purpose:
  Draw out the full product intent before specification or planning.

open_floor:
  "Me dá o quadro inteiro primeiro: o que você quer criar, pra quem, por que agora, e o que faria isso valer a pena?"

source_material:
  Ask for any existing notes, examples, sketches, links, constraints, failed attempts, or prior decisions before drilling into details.

follow_up_batches:
  - outcome: "What must be true when this works?"
  - audience: "Who actually uses it, and what pain or desire makes them care?"
  - constraints: "What is fixed: time, budget, platform, data, integrations, legal, taste?"
  - proof: "What is the smallest evidence that would make this less speculative?"

conversation_stages:
  - open_dump: "Let the human describe the idea in ordinary language before turning it into method terms."
  - mirror: "Restate the intent, audience, taste, and non-goals using the user's words."
  - reality_check: "Identify impossibility, safety, legal, market, and evidence risks before commitment."
  - route: "Recommend the track/workflow and one or two alternatives with clear tradeoffs."
  - commit: "Persist intent, assumptions, constraints, success criteria, and next workflow."

elicitation_options:
  - example_mining: "Ask for examples that feel close, examples that feel wrong, and why."
  - anti_goal: "Ask what this must not become even if it would be easier to build."
  - first_user: "Ask who benefits first, not the abstract total market."
  - evidence_probe: "Ask what would convince a skeptical builder the problem is real."

facilitator_moves:
  - "Protect the human's raw idea before compressing it into artifacts."
  - "Challenge impossible or unsafe promises early and preserve the useful seed."
  - "Ask one consolidated question when gaps are obvious; do not interrogate field by field."
  - "Name uncertainty as assumptions rather than pretending discovery is complete."

quality_bar:
  - "The human recognizes the captured idea as theirs."
  - "The next route is justified by intent and risk, not keyword matching alone."
  - "A future agent can continue from durable intent without replaying the chat."

anti_patterns:
  - "Do not start technical planning before intent, constraints, and success are known."
  - "Do not flatten taste-heavy ideas into generic product language."
  - "Do not treat market scarcity as evidence of viability."

paths:
  fast_path: "Batch remaining gaps into one consolidated question, then write assumptions explicitly."
  deep_path: "Run reality/evidence, market/domain/technical scans, then close discovery with Grill Gate."

checkpoint_options:
  - continue
  - reality-evidence-gate
  - brainstorm
  - problem-solving
  - council

artifact_rules:
  Persist intent, constraints, success criteria, assumptions, rejected directions, and next workflow.

headless:
  Infer only from provided material. Mark missing facts as assumptions and return blocked only when the route cannot be selected.
