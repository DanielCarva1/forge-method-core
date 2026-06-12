# facilitation: product-planning

purpose:
  Convert intent and evidence into requirements, spec boundaries, acceptance criteria, and decision logs without losing the human problem.

open_floor:
  "What user pain or desire are we solving, and what would make the human proud of the resulting product?"

source_material:
  Ask for discovery artifacts, evidence scans, target users, constraints, UX/taste notes, non-goals, risks, existing specs, and any correction signals.

follow_up_batches:
  - user: "Who is this for, what hurts, and what changes when it works?"
  - value: "What is the smallest valuable outcome, not just the smallest build?"
  - boundaries: "What is in MVP, parked, rejected, or legally/ethically constrained?"
  - acceptance: "What observable behavior proves each requirement?"
  - change_mode: "Are we creating, updating, validating, or distilling a spec?"
  - addendum: "What changed since the last decision, and what previous claim does it replace?"
  - findings: "Which requirement is untestable, unsupported, contradictory, or too broad?"

conversation_stages:
  - orient: "Load discovery/evidence and state the planning mode."
  - mode_select: "Choose create, update, validate, or addendum before writing requirements."
  - elicit_outcomes: "Pull out users, jobs, pain, taste, success, and constraints."
  - structure_requirements: "Turn outcomes into requirements, non-goals, assumptions, and acceptance evidence."
  - decision_log: "Record accepted, rejected, replaced, and deferred decisions with reason."
  - conflict_scan: "Find contradictions with evidence, UX, architecture, security, or scope."
  - validate_findings: "Name blocking findings, non-blocking warnings, and routeable gaps."
  - handoff: "Persist spec/requirements plus decision log and next workflow."

elicitation_options:
  - proud_test: "Ask what result would make the user say this has taste."
  - mvp_line: "Split must-have, should-have, parked, and explicitly rejected."
  - conflict_table: "List current claim, contrary evidence, and resolution."
  - acceptance_walk: "For each requirement, ask what a tester or user would observe."
  - addendum_delta: "Ask what changed, why it changed, and who needs the new truth."
  - findings_review: "Walk every requirement through clear, testable, feasible, sourced, and scoped."

facilitator_moves:
  - "Do not let feature lists hide the user pain."
  - "Do not invent requirements when discovery is missing."
  - "Keep non-goals and rejected scope as first-class output."
  - "Route to UX, architecture, or research when the spec depends on unresolved judgment."
  - "When updating, preserve the previous decision and explain the replacement."
  - "When validating, produce findings instead of silently rewriting the spec."

quality_bar:
  - "Requirements are tied to user value and acceptance evidence."
  - "A future agent can create architecture/stories without asking what matters."
  - "Contradictions and assumptions are visible."
  - "Create/update/validate modes leave a durable decision log or addendum."
  - "Validation findings separate blockers, warnings, and follow-up workflows."

anti_patterns:
  - "Do not skip from idea to stories."
  - "Do not turn vague taste into generic requirements."
  - "Do not validate a spec by checking only formatting."
  - "Do not overwrite old product decisions without a dated addendum."
  - "Do not treat implementation tasks as requirements unless they express user-observable behavior."

paths:
  fast_path: "Write a compact spec kernel with assumptions and next workflow."
  deep_path: "Create/update/validate requirements with decision log, UX handoff, architecture handoff, and Grill Gate."

checkpoint_options:
  - write-spec
  - product-requirements
  - ux-plan
  - architecture
  - grill-gate
  - create-epics

artifact_rules:
  Persist users, jobs, requirements, non-goals, acceptance evidence, decisions, conflicts, and next workflow.

domain_examples:
  - prd_create: "Create a new PRD from discovery, evidence, users, success metrics, requirements, and non-goals."
  - prd_update: "Add an addendum when scope, user, risk, or acceptance criteria changes."
  - prd_validate: "Return findings for vague requirements, missing evidence, contradictions, and untestable acceptance."

headless:
  Distill only what sources support. Mark gaps as open questions or required human input.
