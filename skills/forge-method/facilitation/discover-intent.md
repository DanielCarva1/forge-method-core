# facilitation: discover-intent

purpose:
  Draw out product intent until it can close discovery and feed specification without guessing.

open_floor:
  "Me da o quadro inteiro primeiro: pra quem e, o que muda para essa pessoa, o que ja esta fixo, o que nao pode virar, o que ainda esta aberto, e qual prova fecha discovery?"

source_material:
  Ask for notes, examples, sketches, links, constraints, failed attempts, prior decisions, anti-references, and any transcript that triggered the idea.

follow_up_batches:
  - audience: "Who benefits first, who decides, and whose pain or desire is concrete enough to design for?"
  - outcome: "What should be different for that audience when this works?"
  - constraints: "What is fixed: time, budget, platform, data, integrations, legal, taste, or operating model?"
  - non_goals: "What must this not become, even if that would be easier to build?"
  - success_signal: "What proof would make the idea worth specifying instead of continuing discovery?"
  - open_questions: "Which unknowns are acceptable assumptions, and which ones need research, brainstorm, or Grill Gate before spec?"

conversation_stages:
  - open_dump: "Let the human describe the idea in ordinary language before turning it into method terms."
  - mirror: "Restate audience, outcome, taste, constraints, non-goals, and open questions in the user's words."
  - reality_check: "Identify impossible, unsafe, legal, market, and evidence risks before commitment."
  - closeout_shape: "Derive source_input, source_answer, audience, outcome, constraints, non_goals, success_signal, open_questions, grill_gate_handoff, decision_log, and next_workflow."
  - route: "Recommend the next workflow and one or two alternatives with clear tradeoffs."
  - commit: "Run `artifact discovery-closeout`, then `artifact discovery-check --path <discovery-closeout-artifact>` before moving to specification."

elicitation_options:
  - example_mining: "Ask for examples that feel close, examples that feel wrong, and why."
  - anti_goal: "Ask what this must not become even if it would be easier to build."
  - first_user: "Ask who benefits first, not the abstract total market."
  - evidence_probe: "Ask what would convince a skeptical builder the problem is real."
  - closeout_probe: "Ask one consolidated question that fills missing closeout fields instead of interrogating field by field."

facilitator_moves:
  - "Protect the human's raw idea before compressing it into artifacts."
  - "Challenge impossible or unsafe promises early and preserve the useful seed."
  - "Ask one consolidated question when gaps are obvious; do not interrogate field by field."
  - "Name uncertainty as assumptions rather than pretending discovery is complete."
  - "Convert the accepted answer into discovery-closeout arguments instead of hand-writing a loose intent artifact."

quality_bar:
  - "The human recognizes the captured idea as theirs."
  - "The next route is justified by intent and risk, not keyword matching alone."
  - "A future agent can continue from durable closeout fields without replaying the chat."
  - "`artifact discovery-check` passes before phase 2 starts."

anti_patterns:
  - "Do not start technical planning before audience, outcome, constraints, non-goals, success signal, and open questions are known."
  - "Do not flatten taste-heavy ideas into generic product language."
  - "Do not treat market scarcity as evidence of viability."
  - "Do not hand-roll discovery closeout markdown when the runtime command exists."

paths:
  fast_path: "Batch remaining gaps into one consolidated question, then write assumptions explicitly and generate the discovery closeout."
  deep_path: "Run reality/evidence, market/domain/technical scans, then close discovery with Grill Gate and discovery-check."

checkpoint_options:
  - continue
  - reality-evidence-gate
  - brainstorm
  - problem-solving
  - council

domain_examples:
  - new_product: "Shape audience, outcome, constraints, non-goals, success signal, and open questions before PRD or stories exist."
  - broad_game: "Extract player fantasy, first playable proof, content source, AI posture, and parked scope before game brief."
  - internal_tool: "Clarify operator, repeated pain, current workaround, fixed constraints, and proof of useful workflow before implementation."

artifact_rules:
  Use `artifact discovery-closeout` with source_input, source_answer, audience, outcome, constraints, non_goals, success_signal, open_questions, grill_gate_handoff, decision_log, and next_workflow. Register the artifact and preserve the path for the next workflow.

headless:
  Infer only from provided material. Mark missing facts as assumptions and return blocked only when the route or closeout fields cannot be selected.
