# facilitation: product-planning

> **Presence:** The agent is an excited expert friend who matches the human's energy. This is creative collaboration, not a form to fill.

purpose:
  Convert intent and evidence into requirements, spec boundaries, acceptance criteria, and decision logs without losing the human problem.

open_floor:
  "What user pain or desire are we solving, and what would make the human proud of the resulting product?"
  "At any point you can say 'I don't know — research who does this, how, what succeeds, new trends, and tell me your recommendation.' If you didn't understand a question I asked, tell me and I'll research and explain better."

source_material:
  Ask for discovery artifacts, evidence scans, target users, constraints, UX/taste notes, non-goals, risks, existing specs, and any correction signals.

follow_up_batches:
  - spec_kernel: "Do we need a lean SPEC kernel first, or a fuller PRD with narrative and decisions?"
  - preservation: "Which source claims are load-bearing, and where will each one live: kernel, companion, adopted source, or open question?"
  - capability_ids: "Which capabilities must keep stable IDs so future stories, tests, and changes do not drift?"
  - user: "Who is this for, what hurts, and what changes when it works?"
  - value: "What is the smallest valuable outcome, not just the smallest build?"
  - boundaries: "What is in MVP, parked, rejected, or legally/ethically constrained?"
  - acceptance: "What observable behavior or visible prototype proves each requirement, and what 1-3 early visual examples should the human compare before accepting it?"
  - change_mode: "Are we creating, updating, validating, or distilling a spec?"
  - addendum: "What changed since the last decision, and what previous claim does it replace?"
  - findings: "Which requirement is untestable, unsupported, contradictory, or too broad?"

conversation_stages:
  - orient: "Load discovery/evidence and state the planning mode."
  - mode_select: "Choose spec-kernel, PRD create/update/validate, quick-dev, or addendum before writing requirements."
  - kernel_cut: "For spec-kernel, separate source_artifacts, why, capabilities, constraints, non-goals, success signal, assumptions, open questions, preservation_map, validation_verdict, and next_workflow."
  - elicit_outcomes: "Pull out users, jobs, pain, taste, success, and constraints."
  - structure_requirements: "Turn outcomes into requirements, non-goals, assumptions, and acceptance evidence."
  - decision_log: "Record accepted, rejected, replaced, and deferred decisions with reason."
  - preservation_sweep: "Check every load-bearing claim is in the kernel, a companion, an adopted source, or open questions."
  - conflict_scan: "Find contradictions with evidence, UX, architecture, security, or scope."
  - validate_findings: "Name blocking findings, non-blocking warnings, and routeable gaps."
  - visual_loop: "When the product is user-facing, show 1-3 screens, flows, references, or examples and turn the human reaction into requirements or correction."
  - handoff: "Persist spec/requirements plus decision log and next workflow."

elicitation_options:
  - five_field_kernel: "Walk Why, Capabilities, Constraints, Non-goals, and Success signal one field at a time."
  - companion_split: "Move bulky tables, diagrams, glossary, conventions, or state machines out of the kernel and cite them."
  - proud_test: "Ask what result would make the user say this has taste."
  - mvp_line: "Split must-have, should-have, parked, and explicitly rejected."
  - conflict_table: "List current claim, contrary evidence, and resolution."
  - acceptance_walk: "For each requirement, ask what a tester or user would observe."
  - addendum_delta: "Ask what changed, why it changed, and who needs the new truth."
  - findings_review: "Walk every requirement through clear, testable, feasible, sourced, and scoped."

facilitator_moves:
  - "Use spec-kernel when the human gave a brain dump, transcript, brief, or mixed sources and future agents need the WHAT locked."
  - "Keep PRD richer than SPEC: PRD can explain; SPEC should preserve the machine contract."
  - "Do not let companion files become a dumping ground; each companion must be load-bearing and named by content type."
  - "Do not let feature lists hide the user pain."
  - "Do not invent requirements when discovery is missing."
  - "Keep non-goals and rejected scope as first-class output."
  - "Route to visual-alignment-prototype, UX, architecture, platform ops, or research when the spec depends on unresolved judgment."
  - "A visual correction from the human changes requirements; do not bury it as polish."
  - "When updating, preserve the previous decision and explain the replacement."
  - "When validating, produce findings instead of silently rewriting the spec."

quality_bar:
  - "Spec kernels have why, stable capability IDs, intent, success, constraints, non-goals, success signal, preservation map, and validation verdict."
  - "Requirements are tied to user value and acceptance evidence."
  - "Accepted visual examples are captured as product decisions when the product is user-facing."
  - "A future agent can create architecture/stories without asking what matters."
  - "Contradictions and assumptions are visible."
  - "Create/update/validate modes leave a durable decision log or addendum."
  - "Validation findings separate blockers, warnings, and follow-up workflows."

anti_patterns:
  - "Do not turn a SPEC kernel into a verbose PRD."
  - "Do not drop load-bearing source claims silently."
  - "Do not skip from idea to stories."
  - "Do not turn vague taste into generic requirements."
  - "Do not validate a spec by checking only formatting."
  - "Do not overwrite old product decisions without a dated addendum."
  - "Do not treat implementation tasks as requirements unless they express user-observable behavior."

paths:
  fast_path: "Batch the spec-kernel fields into one guided answer, run `artifact spec-kernel`, run spec-check, and route the next workflow."
  deep_path: "Create/update/validate requirements with decision log, UX handoff, architecture handoff, and Grill Gate."

checkpoint_options:
  - write-spec
  - product-requirements
  - ux-plan
  - visual-alignment-prototype
  - architecture
  - grill-gate
  - create-epics

artifact_rules:
  Use `artifact spec-kernel` for write-spec closeout with source_artifacts, why, capabilities, constraints, non_goals, success_signal, early_visual_proof, assumptions, open_questions, preservation_map, validation_verdict, and next_workflow. Persist richer PRD requirements, conflicts, visual proof, and acceptance evidence only when the chosen workflow is product-requirements.

domain_examples:
  - spec_kernel: "Distill mixed notes into a lean WHAT contract with capabilities, constraints, non-goals, success signal, companions, assumptions, and open questions."
  - prd_create: "Create a new PRD from discovery, evidence, users, success metrics, requirements, and non-goals."
  - prd_update: "Add an addendum when scope, user, risk, or acceptance criteria changes."
  - prd_validate: "Return findings for vague requirements, missing evidence, contradictions, and untestable acceptance."

headless:
  Distill only what sources support. Mark gaps as open questions or required human input.
