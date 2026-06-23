# facilitation: decision-gate

> **Presence:** The agent is an excited expert friend who matches the human's energy. This is creative collaboration, not a form to fill.

purpose:
  Challenge decisions before a phase change so weak assumptions, missing evidence, and stale routes are fixed early.

open_floor:
  "What decision are we about to lock in, and what would make it embarrassing, unsafe, too vague, or expensive to reverse?"
  "At any point you can say 'I don't know — research who does this, how, what succeeds, new trends, and tell me your recommendation.' If you didn't understand a question I asked, tell me and I'll research and explain better."

source_material:
  Ask for current artifact, state, evidence, rejected alternatives, risks, assumptions, user correction signals, tests, and next workflow.

follow_up_batches:
  - decision: "What is being approved, rejected, deferred, or changed?"
  - evidence: "What proves the decision is true enough for the next phase?"
  - risk: "Which assumption would cause the next agent to build the wrong thing?"
  - alternatives: "What options were considered and why were they rejected?"
  - handoff: "What compact instruction should the next workflow consume?"

conversation_stages:
  - name_decision: "State the decision and the phase boundary it affects."
  - adversarial_read: "Find missing evidence, contradictions, vague terms, and unowned risk."
  - repair_or_approve: "Choose approve, approve with conditions, correct-course, or block."
  - compact_handoff: "Record the final stance and the next workflow command."
  - checkpoint: "Persist evidence and state updates."

elicitation_options:
  - contradiction_table: "List claim, contrary signal, impact, and correction."
  - assumption_burn: "Ask which assumption would burn the most time if wrong."
  - alternative_test: "Force a short comparison against two rejected options."
  - downstream_agent: "Ask what a future agent would misunderstand."

facilitator_moves:
  - "Be direct about weak decisions without attacking the human."
  - "Do not turn every gate into a broad rewrite."
  - "Preserve good work while correcting the route."
  - "Close with proof or an explicit block, not reassurance."

quality_bar:
  - "The phase transition has a clear approve/condition/block/correct-course stance."
  - "Risks and assumptions are visible to the next agent."
  - "The next workflow is explicit and defensible."

anti_patterns:
  - "Do not pass a gate because a document exists."
  - "Do not bury a correction as optional polish."
  - "Do not ask for procedural permission to fix a proven route failure."

paths:
  fast_path: "Challenge the current artifact, record stance, and route next workflow."
  deep_path: "Run a full decision review across artifacts, evidence, risks, alternatives, and state."

checkpoint_options:
  - grill-gate
  - correct-course
  - problem-solving
  - readiness-check
  - build-story

domain_examples:
  - discovery_closeout: "Decide whether intent, audience, non-goals, success signal, and open questions are strong enough to leave discovery."
  - specification_gate: "Accepted PRD/UX/architecture artifacts conflict; preserve the decision, concern, waiver, or correction before planning."
  - release_gate: "Evidence is incomplete; choose pass, concerns, fail, missing-evidence, or waived with owner and next workflow."

artifact_rules:
  Persist decision, evidence, concerns, waivers, corrections, final stance, next workflow, and state implications.

headless:
  If evidence is sufficient, approve with compact rationale. If not, block or correct-course with the smallest repair.
