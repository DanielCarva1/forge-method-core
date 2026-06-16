# facilitation: quick-dev

purpose:
  Guide small scoped changes through enough product thinking, implementation, review, and evidence without forcing a full PRD cycle.

open_floor:
  "What is the smallest change that would count as done, and what should stay untouched?"

source_material:
  Ask for the current behavior, desired behavior, relevant files or screens, acceptance signal, constraints, validation command, and any risky edge cases.

follow_up_batches:
  - scope: "What exact behavior changes, and what is explicitly out?"
  - proof: "What will we observe, test, or inspect to know it worked?"
  - constraints: "Which files, APIs, users, data, or release risks should not be disturbed?"
  - autonomy: "Can this proceed headlessly, or is a human choice still missing?"
  - handoff: "Should this become a build-story, direct patch, or a larger PRD/UX/architecture flow?"

conversation_stages:
  - scope_snap: "Compress the request into one spec-lite paragraph plus non-goals."
  - risk_scan: "Check whether the change secretly needs product, UX, architecture, security, or data decisions."
  - implementation_path: "Choose direct mechanical work or build-story handoff."
  - verification_path: "Name checks, review criteria, evidence, and rollback/recovery."
  - closeout: "Record artifact, touched files, validation, and next workflow."

elicitation_options:
  - smallest_done: "Ask what must be true for the user to stop caring."
  - counter_scope: "Ask what a rushed agent might accidentally change."
  - proof_first: "Ask for the check before writing implementation steps."
  - upgrade_trigger: "Ask which signal would force full PRD, UX, architecture, or security planning."

facilitator_moves:
  - "Keep momentum, but do not pretend a vague request is safe just because it sounds small."
  - "If the change needs taste, accessibility, or interaction judgment, route to UX instead of quick-dev."
  - "If the change changes product promises, route to product-requirements instead of quick-dev."
  - "If implementation is already safe and bounded, stop asking procedural questions and build."

quality_bar:
  - "The human sees one crisp scope and a clear proof target."
  - "The agent gets compact spec-lite, non-goals, validation, and evidence requirements."
  - "The workflow can escalate when the request is not actually small."

anti_patterns:
  - "Do not use quick-dev for strategy, architecture, security, or broad refactors."
  - "Do not skip evidence because the request is small."
  - "Do not ask for permission to continue once scope and proof are settled."

paths:
  fast_path: "Spec-lite, direct implementation, validation, evidence, checkpoint."
  deep_path: "Escalate to product-requirements, ux-plan, architecture, or build-story when scope is not safely small."

checkpoint_options:
  - quick-dev
  - product-requirements
  - ux-plan
  - architecture
  - build-story

domain_examples:
  - tiny_fix: "A scoped bug or copy fix has clear acceptance and check command; produce spec-lite, patch, evidence, and review route."
  - small_feature: "A narrow behavior is known but not story-sized; define non-goals, files likely touched, validation, and next workflow."
  - risky_shortcut: "The request sounds quick but has hidden architecture or product risk; stop at spec-lite and route to PRD, UX, or architecture."

artifact_rules:
  Persist request, scope, non-goals, acceptance evidence, touched files, validation, review, risks, and next workflow.

headless:
  Proceed only when scope, non-goals, and validation are clear. Otherwise write a quick-dev artifact with the missing decision.
