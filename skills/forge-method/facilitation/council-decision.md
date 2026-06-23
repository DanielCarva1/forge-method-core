# facilitation: council-decision

> **Presence:** The agent is an excited expert friend who matches the human's energy. This is creative collaboration, not a form to fill.

purpose:
  Run a rich multi-perspective decision session for the human while preserving only a compact decision and orchestration contract for future agents.

open_floor:
  "Vamos colocar gente esperta na sala, mas sem transformar o projeto em teatro. Qual decisao precisa de mais de uma perspectiva, e qual dissent mudaria o proximo passo?"
  "At any point you can say 'I don't know — research who does this, how, what succeeds, new trends, and tell me your recommendation.' If you didn't understand a question I asked, tell me and I'll research and explain better."

source_material:
  Ask for the decision topic, relevant artifacts, constraints, available agent roles, evidence gaps, and whether the work can split into independent outputs.

follow_up_batches:
  - decision: "What decision is actually being made, and what is not being decided?"
  - participants: "Which perspectives can change the answer: product, research, architecture, implementation, quality, operations, taste?"
  - dissent: "What objection would make us pause, reroute, or gather evidence?"
  - orchestration: "Should roles debate serially, run independent worker outputs in parallel, or create a handoff for real subagents?"

conversation_stages:
  - frame: "Name the decision, stakes, constraints, and confidence target."
  - specialist_round: "Let each participant make a short useful move, not a generic opinion."
  - dissent_round: "Preserve disagreement as risk, evidence needed, or rejected option."
  - convergence: "Choose recommendation, next action, owner, and proof."
  - orchestration_contract: "Record mode, worker outputs, merge owner, and what must not be persisted."

elicitation_options:
  - party_mode: "Use a live council when the human needs a vivid discussion."
  - dissent_first: "Start with the strongest objection before converging."
  - parallel_split: "Split independent research, quality, architecture, and implementation checks only when outputs can merge cleanly."
  - false_consensus_check: "Ask what everyone is politely ignoring."

facilitator_moves:
  - "Keep the live discussion vivid and human-readable; keep the artifact short."
  - "Make each role change the decision quality or remove it from the council."
  - "Use real subagents only when the runtime supports them and the outputs are independent."
  - "When parallelism is not safe, say so and keep a sequential council."

quality_bar:
  - "The human can see why each perspective mattered."
  - "The durable artifact records recommendation, dissent, evidence needed, orchestration mode, and next action."
  - "The transcript is not required future context."
  - "Parallel/subagent mode does not change the artifact contract."

anti_patterns:
  - "Do not persist a long roleplay transcript as project memory."
  - "Do not include participants that only restate the same point."
  - "Do not claim parallel execution when outputs depend on each other."
  - "Do not let council become a required blocker for normal workflow progress."

paths:
  fast_path: "Run debate/decision mode with 2-3 relevant roles, write compact decision, continue."
  deep_path: "Run dissent-first council, split independent worker outputs, merge into a decision artifact, then route to story/risk/correct-course."

checkpoint_options:
  - continue
  - concept-selection
  - story-creation
  - risk-register
  - correct-course

artifact_rules:
  Persist topic, participants, mode, recommendation, dissent, evidence needed, worker output contracts, merge owner, and next action. Do not persist the full live transcript.

headless:
  If no human is present, run a compact sequential council and write only the decision artifact.

domain_examples:
  forge_runtime: "Facilitator frames the human defect, Planner sequences the patch, Quality Reviewer defines replay/smoke proof."
  product_strategy: "PM lens weighs value and scope, Researcher names evidence, Architect/Implementer checks feasibility."
  quality_gate: "Quality Reviewer attacks evidence, Implementer names repair cost, Operator checks support risk."
