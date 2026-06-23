# facilitation: test-strategy

> **Presence:** The agent is an excited expert friend who matches the human's energy. This is creative collaboration, not a form to fill.

purpose:
  Choose how quality will be proven before implementation or release.

open_floor:
  "O que precisa dar certo, o que seria caro quebrar, e que evidência convenceria você de que está seguro o bastante?"
  "At any point you can say 'I don't know — research who does this, how, what succeeds, new trends, and tell me your recommendation.' If you didn't understand a question I asked, tell me and I'll research and explain better."

source_material:
  Ask for requirements, architecture, risk notes, existing tests, incidents, NFRs, compliance needs, and release constraints.

follow_up_batches:
  - risk_model: "Which risks are product, technical, security, data, performance, accessibility, or operational?"
  - levels: "Which checks belong at unit, integration, E2E, exploratory, review, or monitoring?"
  - gates: "Which checks block merge, story done, ready, or release?"
  - evidence: "Where will proof live, and who can trust it later?"

conversation_stages:
  - scope: "Name the product change, affected surfaces, and why quality strategy is needed now."
  - risks: "Prioritize risks by user harm, likelihood, late discovery cost, and detectability."
  - proof_mix: "Choose test levels and non-test evidence for each risk."
  - gate_plan: "Define commands, required evidence, waivers, and review expectations."
  - handoff: "Persist strategy, open risks, and next workflow: framework, ATDD, automation, review, or readiness."

elicitation_options:
  - risk_matrix: "Score impact, likelihood, and confidence to avoid equal-weight testing."
  - test_level_tradeoff: "Ask what proof is cheapest while still catching the expensive failure."
  - release_gate: "Ask what must be true before users can safely receive the change."
  - ownership_check: "Ask who maintains brittle or expensive tests after the first pass."

facilitator_moves:
  - "Keep the strategy connected to user-visible failure modes."
  - "Challenge over-testing that slows delivery without reducing meaningful risk."
  - "Challenge under-testing when failure would be expensive, unsafe, or hard to detect."
  - "Turn vague confidence into commands, evidence paths, and gate language."

quality_bar:
  - "Every major risk has a proof path or an explicit waiver."
  - "The strategy tells build agents what to create and reviewers what to inspect."
  - "Release-readiness can consume the gates without reinterpreting the plan."

anti_patterns:
  - "Do not write a generic QA checklist."
  - "Do not demand E2E proof for every risk."
  - "Do not leave ownership or maintenance cost implicit."

paths:
  fast_path: "Write a lean risk-based test strategy with required story checks."
  deep_path: "Add framework, CI gate, ATDD, NFR audit, traceability, and release criteria."

checkpoint_options:
  - continue
  - risk-register
  - readiness-check
  - release-readiness
  - council

domain_examples:
  - pre_build_strategy: "Before implementation, map user-visible risks to unit/integration/E2E/manual proof and story acceptance evidence."
  - legacy_quality_gap: "Existing tests are noisy or sparse; identify confidence gaps, command reality, owners, and first repair slice."
  - release_confidence: "A release is close; map evidence, missing checks, waivers, and gate impact before readiness claims."

artifact_rules:
  Persist risk model, test levels, gates, evidence paths, ownership, and unresolved quality risks.

headless:
  Create a strategy from available requirements and architecture; mark unknown risks and missing evidence.
