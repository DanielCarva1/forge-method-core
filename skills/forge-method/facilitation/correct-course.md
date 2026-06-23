# facilitation: correct-course

> **Presence:** The agent is an excited expert friend who matches the human's energy. This is creative collaboration, not a form to fill.

purpose:
  Recover when the current route, artifact, plan, taste direction, scope, state, or implementation is wrong.

open_floor:
  "O que exatamente ficou errado, o que isso invalida, e qual parte ainda vale preservar?"
  "At any point you can say 'I don't know — research who does this, how, what succeeds, new trends, and tell me your recommendation.' If you didn't understand a question I asked, tell me and I'll research and explain better."

source_material:
  Ask for the rejected output, current state, impacted artifacts, user correction, tests/evidence, and constraints.

follow_up_batches:
  - failure: "What failed: route, scope, taste, evidence, implementation, state, or communication?"
  - contradiction: "What current claim conflicts with the human correction or evidence?"
  - impact: "Which artifacts, stories, decisions, tests, release claims, and user expectations are affected?"
  - preserve: "What should not be thrown away?"
  - repair: "Should the fix rollback, insert a missing workflow, rewrite an artifact, split scope, defer, or escalate?"
  - proof: "What fixture, eval, test, or transcript would catch this failure next time?"

conversation_stages:
  - stop: "Acknowledge the correction plainly and name the current route or claim that failed."
  - impact_scan: "Inspect state, artifacts, stories, docs, and recent evidence before proposing a repair."
  - preserve: "Separate valid work from invalid assumptions so the user does not feel the whole system is gaslighting them."
  - options: "Offer rollback, insert-missing-step, rewrite, scope-split, and explicit defer or waive only when each is real."
  - proof_design: "Name the regression proof before claiming the correction is durable."
  - commit: "Write a correction artifact or story update with changed decisions, preserved decisions, checks, and next action."

elicitation_options:
  - blast_radius: "Ask what user-facing promise, artifact, workflow, or release state is now untrusted."
  - contradiction_table: "List current claim, contrary evidence, and required correction."
  - rewind_point: "Find the last durable state that was still valid."
  - missing_step: "Identify the skipped workflow or facilitation move that would have prevented the failure."
  - repair_menu: "Compare rollback, insert, rewrite, split, defer, and escalate by risk and reversibility."
  - human_choice: "Ask only when multiple repairs have real product tradeoffs."

facilitator_moves:
  - "Do not defend the previous path; use evidence and fix the route."
  - "Name emotional frustration as a signal, then translate it into product behavior."
  - "Keep the repair narrow unless the evidence shows a systemic failure."
  - "When taste or human experience failed, treat it as a product requirement defect, not cosmetic polish."
  - "When implementation contradicts accepted decisions, route repair before celebrating green tests."
  - "Close with proof, not reassurance."

quality_bar:
  - "The human can tell exactly what was wrong and what will change."
  - "Durable state no longer points at the stale or contradicted action."
  - "The correction prevents the same class of failure in a future transcript."
  - "Preserved decisions and changed decisions are both explicit."
  - "The selected repair has a validation proof and next workflow."

anti_patterns:
  - "Do not say the work is complete when a stated product requirement is still weaker than the benchmark."
  - "Do not bury the correction as optional polish if it affects the product's core promise."
  - "Do not ask procedural permission to repair a clearly broken route."
  - "Do not keep building on a contradicted artifact because tests are green."
  - "Do not make the human know the phase name before the method can recover."

paths:
  fast_path: "Write correction artifact and continue with conservative repair."
  deep_path: "Run impact matrix, update affected artifacts/stories, add regression proof, then Grill Gate before build."

checkpoint_options:
  - continue
  - problem-solving
  - runtime-builder
  - council
  - human-input

domain_examples:
  - software: "Scope or architecture changed; inspect spec, architecture, story checks, tests, and release gate before continuing build."
  - game: "Player fantasy, loop, platform, or playable slice changed; inspect brief, GDD, PRD, game stories, playtest evidence, and sprint status."
  - creative: "Taste direction or audience changed; inspect selected concept, rejected options, creative brief, and decision log."
  - enterprise: "Security, privacy, compliance, CI, NFR, or observability claim changed; inspect risk register, evidence, waivers, and release readiness."
  - runtime: "Method behavior failed; inspect Guidance Engine route, workflow catalog, facilitation pack, fixtures, evals, and state transition."

artifact_rules:
  Persist trigger, failed claim, contradiction type, impact scope, affected artifacts, preserved decisions, changed decisions, selected repair, validation proof, and next action.

headless:
  Choose the conservative correction if artifacts prove it. Stop only when a real human tradeoff is required.
