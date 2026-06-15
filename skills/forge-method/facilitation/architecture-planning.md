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
  - ux_contract: "Which UX/taste decisions create technical constraints, state needs, or latency/density expectations?"
  - tradeoffs: "Which options are plausible, and why reject each one?"
  - evidence: "What spike, benchmark, test, or source proves the risky decision?"
  - readiness: "Which stories, tests, gates, or operations checks depend on this architecture decision?"
  - handoff: "What must story authors and implementers know compactly?"

conversation_stages:
  - load_requirements: "Start from requirements, UX, risks, and existing code."
  - mode_select: "Choose create, update, validate, or tradeoff review before writing decisions."
  - trace_sources: "Tie every architecture decision to PRD/spec, UX, risk, evidence, or explicit assumption."
  - identify_decisions: "Name irreversible, expensive, or cross-cutting choices."
  - compare_options: "Evaluate options with constraints and evidence."
  - set_contracts: "Define interfaces, data ownership, security/privacy boundaries, test hooks, and operational concerns."
  - story_impact: "Translate decisions into story boundaries, readiness dependencies, and validation hooks."
  - validate_findings: "Name missing sources, unsafe assumptions, contradiction, and unproven risks."
  - handoff: "Persist decisions, rejected options, open risks, story guidance, and next workflow."

elicitation_options:
  - failure_mode: "Ask what technical failure would hurt users or future agents most."
  - boundary_map: "Separate UI, domain, persistence, integration, automation, and ops boundaries."
  - tradeoff_table: "Compare options by simplicity, risk, reversibility, and evidence."
  - source_trace: "For each decision, ask which PRD/UX/risk source justifies it."
  - assumption_burn_down: "Separate decisions, assumptions, spikes, and blocked questions."
  - story_impact: "Trace how each decision changes epics, tests, and implementation order."

facilitator_moves:
  - "Do not design architecture before requirements and user workflows are clear."
  - "Do not hide assumptions as decisions."
  - "Prefer reversible simplicity unless evidence supports heavier machinery."
  - "Write architecture for story authors, not for ceremony."
  - "When UX needs speed, density, offline behavior, or trust, convert it into an explicit technical constraint."
  - "When security/privacy/data ownership is vague, route to the relevant enterprise or test workflow before build."

quality_bar:
  - "Architecture decisions are traceable to requirements, risks, and constraints."
  - "Rejected options and open risks are visible."
  - "The next agent can create stories and tests without re-arguing core boundaries."
  - "UX, security/privacy, test hooks, and readiness implications are explicit."
  - "Validation findings distinguish blockers, warnings, assumptions, and follow-up workflows."

anti_patterns:
  - "Do not use trendy infrastructure as proof of quality."
  - "Do not ignore security, privacy, observability, or testability until release."
  - "Do not create generic diagrams with no implementation consequence."
  - "Do not let architecture override product or UX decisions without a recorded correct-course."
  - "Do not create stories from architecture if source trace or validation hooks are missing."

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

domain_examples:
  - architecture_create: "Create architecture from PRD, UX, constraints, components, data ownership, interfaces, risks, and validation hooks."
  - architecture_update: "Write an addendum when a new integration, data boundary, UX constraint, or deployment constraint changes previous decisions."
  - architecture_validate: "Return findings for missing source trace, unbounded risk, vague interfaces, missing test hooks, or story-impact gaps."
  - tradeoff_review: "Compare two plausible technical paths by reversibility, user impact, cost, risk, evidence, and story order."

headless:
  Inspect existing code and artifacts first. Mark missing requirements as blockers rather than inventing architecture.
