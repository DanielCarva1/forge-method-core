# facilitation: architecture-planning

purpose:
  Turn requirements, constraints, and quality risks into architecture decisions that future stories can safely execute.

open_floor:
  "Which technical decision, if guessed wrong, would make the product hard to build, unsafe, slow, expensive, or unpleasant to change?"

source_material:
  Ask for requirements, UX plan, existing code, platform constraints, data model, integrations, security/privacy notes, scale expectations, test strategy, and prior decisions.

follow_up_batches:
  - system_shape: "What are the boundaries, components, data flows, and external dependencies?"
  - constraints: "What stack, deployment, security, privacy, latency, cost, or team constraint matters?"
  - tradeoffs: "Which options are plausible, and why reject each one?"
  - evidence: "What spike, benchmark, test, or source proves the risky decision?"
  - handoff: "What must story authors and implementers know compactly?"

conversation_stages:
  - load_requirements: "Start from requirements, UX, risks, and existing code."
  - identify_decisions: "Name irreversible, expensive, or cross-cutting choices."
  - compare_options: "Evaluate options with constraints and evidence."
  - set_contracts: "Define interfaces, data ownership, test hooks, and operational concerns."
  - handoff: "Persist decisions, rejected options, open risks, and story guidance."

elicitation_options:
  - failure_mode: "Ask what technical failure would hurt users or future agents most."
  - boundary_map: "Separate UI, domain, persistence, integration, automation, and ops boundaries."
  - tradeoff_table: "Compare options by simplicity, risk, reversibility, and evidence."
  - story_impact: "Trace how each decision changes epics, tests, and implementation order."

facilitator_moves:
  - "Do not design architecture before requirements and user workflows are clear."
  - "Do not hide assumptions as decisions."
  - "Prefer reversible simplicity unless evidence supports heavier machinery."
  - "Write architecture for story authors, not for ceremony."

quality_bar:
  - "Architecture decisions are traceable to requirements, risks, and constraints."
  - "Rejected options and open risks are visible."
  - "The next agent can create stories and tests without re-arguing core boundaries."

anti_patterns:
  - "Do not use trendy infrastructure as proof of quality."
  - "Do not ignore security, privacy, observability, or testability until release."
  - "Do not create generic diagrams with no implementation consequence."

paths:
  fast_path: "Record essential decisions, rejected options, and story/test implications."
  deep_path: "Run architecture, security/privacy/devops/test strategy, then readiness check."

checkpoint_options:
  - architecture
  - engine-architecture
  - security-plan
  - privacy-data-plan
  - test-strategy
  - readiness-check

artifact_rules:
  Persist components, decisions, rejected options, constraints, risks, test hooks, operational notes, and next workflow.

headless:
  Inspect existing code and artifacts first. Mark missing requirements as blockers rather than inventing architecture.
