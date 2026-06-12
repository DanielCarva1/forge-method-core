# facilitation: correct-course

purpose:
  Recover when the current route, artifact, plan, or implementation is wrong.

open_floor:
  "O que exatamente ficou errado, o que isso invalida, e qual parte ainda vale preservar?"

source_material:
  Ask for the rejected output, current state, impacted artifacts, user correction, tests/evidence, and constraints.

follow_up_batches:
  - failure: "What failed: route, scope, taste, evidence, implementation, or communication?"
  - impact: "Which artifacts, stories, decisions, and user expectations are affected?"
  - preserve: "What should not be thrown away?"
  - repair: "What is the smallest correction that makes the route true again?"

conversation_stages:
  - stop: "Acknowledge the correction plainly and name the current route or claim that failed."
  - impact_scan: "Inspect state, artifacts, stories, docs, and recent evidence before proposing a repair."
  - preserve: "Separate valid work from invalid assumptions so the user does not feel the whole system is gaslighting them."
  - options: "Offer conservative repair, deeper redesign, and explicit defer/waive only when each is real."
  - commit: "Write a correction artifact or story update with changed decisions, preserved decisions, checks, and next action."

elicitation_options:
  - blast_radius: "Ask what user-facing promise, artifact, workflow, or release state is now untrusted."
  - contradiction_table: "List current claim, contrary evidence, and required correction."
  - rewind_point: "Find the last durable state that was still valid."
  - human_choice: "Ask only when multiple repairs have real product tradeoffs."

facilitator_moves:
  - "Do not defend the previous path; use evidence and fix the route."
  - "Name emotional frustration as a signal, then translate it into product behavior."
  - "Keep the repair narrow unless the evidence shows a systemic failure."
  - "Close with proof, not reassurance."

quality_bar:
  - "The human can tell exactly what was wrong and what will change."
  - "Durable state no longer points at the stale or contradicted action."
  - "The correction prevents the same class of failure in a future transcript."

anti_patterns:
  - "Do not say the work is complete when a stated product requirement is still weaker than the benchmark."
  - "Do not bury the correction as optional polish if it affects the product's core promise."
  - "Do not ask procedural permission to repair a clearly broken route."

paths:
  fast_path: "Write correction artifact and continue with conservative repair."
  deep_path: "Run impact matrix, update affected artifacts/stories, then Grill Gate before build."

checkpoint_options:
  - continue
  - problem-solving
  - runtime-builder
  - council
  - human-input

domain_impact_examples:
  - software: "Scope or architecture changed; inspect spec, architecture, story checks, tests, and release gate before continuing build."
  - game: "Player fantasy, loop, platform, or playable slice changed; inspect brief, GDD, PRD, game stories, playtest evidence, and sprint status."
  - creative: "Taste direction or audience changed; inspect selected concept, rejected options, creative brief, and decision log."
  - enterprise: "Security, privacy, compliance, CI, NFR, or observability claim changed; inspect risk register, evidence, waivers, and release readiness."
  - runtime: "Method behavior failed; inspect Guidance Engine route, workflow catalog, facilitation pack, fixtures, evals, and state transition."

artifact_rules:
  Persist correction summary, impact, preserved decisions, changed decisions, affected artifacts, and next action.

headless:
  Choose the conservative correction if artifacts prove it. Stop only when a real human tradeoff is required.
