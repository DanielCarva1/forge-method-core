# facilitation: test-architecture

purpose:
  Turn broad quality concerns into the right test architecture workflow: engagement model, framework, CI, ATDD, automation, review, NFR audit, or traceability gate.

open_floor:
  "What quality question are we answering: what to test, how to test it, how CI enforces it, whether coverage is enough, or whether release evidence passes?"

source_material:
  Ask for requirements, stories, risk register, architecture notes, existing tests, CI config, evidence, release criteria, and known incidents.

follow_up_batches:
  - engagement: "Is this advice, design, implementation, review, audit, or a release gate?"
  - risk: "Which failure would be expensive, user-visible, unsafe, or hard to detect?"
  - fixture_architecture: "Which helper should stay framework-neutral, which wrapper owns framework context, and how are fixtures composed and cleaned up?"
  - ci_contract: "Which commands run locally, in fast CI, in full CI, and at release gate?"
  - traceability_phase: "Are we mapping planned coverage, or making a release decision from actual evidence?"
  - proof: "What evidence would prove the claim, and what evidence exists now?"
  - automation: "Which checks should be automated, manual, semi-automated, or waived?"
  - waiver: "Who owns a gap, why can it ship, when is it revisited, and what release impact remains?"
  - gate: "What pass/concerns/fail/missing-evidence/waived decision should downstream agents consume?"

conversation_stages:
  - classify: "Name the quality job: teach, engage, design, framework, CI, ATDD, automate, review, audit, or gate."
  - risk_model: "Map user-visible risks and engineering risks before choosing test types."
  - engagement_model: "Choose advice, design, implementation, review, audit, or gate before writing the artifact."
  - fixture_design: "Prefer pure helpers, thin framework wrappers, explicit composition, lifecycle cleanup, and command evidence."
  - evidence_design: "Define the proof path: examples, fixtures, commands, CI gates, traces, or waivers."
  - command_map: "Split local, fast, full, release, and investigation commands."
  - operating_model: "Decide how the team or agent will create, run, maintain, and trust the tests."
  - gate_handoff: "Persist phase, coverage status, decision, commands, missing evidence, waivers, and next workflow."

elicitation_options:
  - teach_first: "Explain testing concepts with the user's feature before asking for strategy decisions."
  - risk_sort: "Rank risks by likelihood, impact, detectability, and cost of late discovery."
  - pyramid_slice: "Place proof at unit, integration, contract, E2E, performance, security, accessibility, or manual level."
  - fixture_split: "Separate reusable pure utilities from framework-bound fixtures before scaffolding."
  - ci_gate_map: "Map commands to merge, story-done, release, and investigation gates."
  - trace_phase_check: "Ask whether this is phase 1 coverage mapping or phase 2 release decision."
  - waiver_test: "Ask what evidence justifies shipping despite a known gap."

facilitator_moves:
  - "Translate vague 'write tests' requests into risks and evidence."
  - "Do not shame the human for not knowing test terminology; teach with their domain."
  - "Prefer maintainable proof over impressive but brittle coverage."
  - "Route broad quality requests through engagement model before picking framework, automation, review, audit, or gate."
  - "Keep fixture architecture framework-neutral unless the project has already chosen a stack."
  - "Do not let missing evidence sound like pass; call it missing evidence or waiver."
  - "Make gate decisions explicit enough for release-readiness to consume."

quality_bar:
  - "The chosen quality workflow matches the user's actual need."
  - "The output includes risks, commands/evidence, ownership, gate stance, and follow-up."
  - "Fixture architecture separates helper, wrapper, composition, cleanup, and evidence."
  - "Traceability distinguishes planned coverage from release evidence."
  - "Waivers name owner, rationale, expiry/revisit trigger, and release impact."
  - "A future agent can run or inspect the proof without asking what quality means here."

anti_patterns:
  - "Do not list generic test types without mapping them to product risk."
  - "Do not treat CI setup, ATDD, automation, review, NFR audit, and traceability as the same workflow."
  - "Do not turn framework examples into separate Forge entrypoints."
  - "Do not call a gate passed when high-risk evidence is missing."
  - "Do not hide weak evidence behind a green-sounding recommendation."

paths:
  fast_path: "Classify quality mode, select the specific workflow, and write one artifact with gaps and next action."
  deep_path: "Run engagement model, framework, ATDD/automation, review, NFR audit, and traceability gate in order."

checkpoint_options:
  - teach-testing
  - test-engagement-model
  - test-framework
  - ci-quality-pipeline
  - atdd-plan
  - test-automation
  - test-review
  - nfr-evidence-audit
  - traceability-gate
  - release-readiness

domain_examples:
  - teach-testing: "The user asks how testing works; explain the smallest useful concept with project examples, then route to strategy/framework/review."
  - test-strategy: "The project needs a risk-based proof mix before stories, framework, automation, or release gates."
  - test-engagement-model: "The user says quality is weak but not how; classify advice/design/implementation/review/audit/gate before planning."
  - test-framework: "A stack exists but checks are scattered; define layers, fixture architecture, data setup, command contract, and first risk checks."
  - ci-quality-pipeline: "Local checks exist but release is manual; map fast/full/release checks to CI jobs, artifacts, local parity, and failure policy."
  - atdd-plan: "Story acceptance is vague; write examples, edge cases, risk coverage, and proof paths before implementation."
  - test-automation: "A high-risk behavior is manual-only; choose repeatable targets by risk, fixtures, data setup, assertions, commands, and evidence."
  - test-review: "Tests pass but confidence is unclear; compare assertions against acceptance and risks, then issue findings and gate recommendation."
  - nfr-evidence-audit: "Release claims performance/security/reliability/accessibility/compliance; map each claim to evidence, gap, waiver, or block."
  - traceability-gate: "Release needs proof; phase 1 maps requirements/risks to checks, phase 2 decides pass/concerns/fail/missing-evidence/waived."

artifact_rules:
  Persist risk mapping, engagement mode, commands, evidence status, gate decision, waivers, and follow-up stories.
  Use the narrow workflow template when available; use `skill:templates/test-architecture-artifact.md` only for legacy or mixed-mode quality artifacts.

headless:
  Use current repo/test evidence first. If evidence is missing, return a gap matrix and the smallest command or artifact needed to unblock.
