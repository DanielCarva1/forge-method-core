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
  - proof: "What evidence would prove the claim, and what evidence exists now?"
  - automation: "Which checks should be automated, manual, semi-automated, or waived?"
  - gate: "What pass/fail/waiver decision should downstream agents consume?"

conversation_stages:
  - classify: "Name the quality job: teach, engage, design, framework, CI, ATDD, automate, review, audit, or gate."
  - risk_model: "Map user-visible risks and engineering risks before choosing test types."
  - evidence_design: "Define the proof path: examples, fixtures, commands, CI gates, traces, or waivers."
  - operating_model: "Decide how the team or agent will create, run, maintain, and trust the tests."
  - gate_handoff: "Persist decision, commands, missing evidence, waivers, and next workflow."

elicitation_options:
  - teach_first: "Explain testing concepts with the user's feature before asking for strategy decisions."
  - risk_sort: "Rank risks by likelihood, impact, detectability, and cost of late discovery."
  - pyramid_slice: "Place proof at unit, integration, contract, E2E, performance, security, accessibility, or manual level."
  - waiver_test: "Ask what evidence justifies shipping despite a known gap."

facilitator_moves:
  - "Translate vague 'write tests' requests into risks and evidence."
  - "Do not shame the human for not knowing test terminology; teach with their domain."
  - "Prefer maintainable proof over impressive but brittle coverage."
  - "Make gate decisions explicit enough for release-readiness to consume."

quality_bar:
  - "The chosen quality workflow matches the user's actual need."
  - "The output includes risks, commands/evidence, ownership, gate stance, and follow-up."
  - "A future agent can run or inspect the proof without asking what quality means here."

anti_patterns:
  - "Do not list generic test types without mapping them to product risk."
  - "Do not treat CI setup, ATDD, automation, review, NFR audit, and traceability as the same workflow."
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
  - test-engagement-model: "The user says quality is weak but not how; classify advisory/design/implementation/review/audit/gate before planning."
  - test-framework: "A stack exists but checks are scattered; define layers, fixtures, data setup, and command contract."
  - ci-quality-pipeline: "Local checks exist but release is manual; map fast/full/release checks to CI jobs and failure policy."
  - atdd-plan: "Story acceptance is vague; write concrete examples and proof paths before implementation."
  - test-automation: "A high-risk behavior is manual-only; choose repeatable targets by risk and maintainability."
  - test-review: "Tests pass but confidence is unclear; compare assertions against acceptance and risks, then issue findings."
  - nfr-evidence-audit: "Release claims performance/security/reliability; map each claim to evidence, gap, waiver, or block."
  - traceability-gate: "Release needs proof; map requirements and risks to checks/evidence and return pass/conditional/fail/waive."

artifact_rules:
  Persist risk mapping, mode, commands, evidence status, gate decision, waivers, and follow-up stories.
  Use `skill:templates/test-architecture-artifact.md` as the default artifact shape unless a narrower project template exists.

headless:
  Use current repo/test evidence first. If evidence is missing, return a gap matrix and the smallest command or artifact needed to unblock.
