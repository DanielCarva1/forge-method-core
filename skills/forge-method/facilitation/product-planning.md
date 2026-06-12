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

conversation_stages:
  - orient: "Load discovery/evidence and state the planning mode."
  - elicit_outcomes: "Pull out users, jobs, pain, taste, success, and constraints."
  - structure_requirements: "Turn outcomes into requirements, non-goals, assumptions, and acceptance evidence."
  - conflict_scan: "Find contradictions with evidence, UX, architecture, security, or scope."
  - handoff: "Persist spec/requirements plus decision log and next workflow."

elicitation_options:
  - proud_test: "Ask what result would make the user say this has taste."
  - mvp_line: "Split must-have, should-have, parked, and explicitly rejected."
  - conflict_table: "List current claim, contrary evidence, and resolution."
  - acceptance_walk: "For each requirement, ask what a tester or user would observe."

facilitator_moves:
  - "Do not let feature lists hide the user pain."
  - "Do not invent requirements when discovery is missing."
  - "Keep non-goals and rejected scope as first-class output."
  - "Route to UX, architecture, or research when the spec depends on unresolved judgment."

quality_bar:
  - "Requirements are tied to user value and acceptance evidence."
  - "A future agent can create architecture/stories without asking what matters."
  - "Contradictions and assumptions are visible."

anti_patterns:
  - "Do not skip from idea to stories."
  - "Do not turn vague taste into generic requirements."
  - "Do not validate a spec by checking only formatting."

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

headless:
  Distill only what sources support. Mark gaps as open questions or required human input.
