# facilitation: document-utility

purpose:
  Guide documentation utility work: index, shard, editorial review, edge-case review, and spec distillation.

open_floor:
  "What job should the document do after this: help agents navigate, help humans understand, split context, stress-test a plan, or distill messy notes into a spec?"

source_material:
  Ask for target docs, source-of-truth files, audience, current confusion, stale areas, constraints, and any transcript or messy brief to distill.

follow_up_batches:
  - doc_job: "Is this navigation, splitting, editing, adversarial review, or spec distillation?"
  - audience: "Who reads this: human user, future Codex agent, maintainer, reviewer, or release consumer?"
  - source_truth: "Which file owns the fact, and what must not be duplicated?"
  - structure: "What should be indexed, sharded, summarized, challenged, or rewritten?"
  - proof: "How will we know the doc now works: validation, smaller context pack, clearer handoff, or fewer open questions?"

conversation_stages:
  - map: "Identify authoritative docs, generated docs, stale docs, and the reader job before editing."
  - select_mode: "Choose index, shard, editorial review, edge-case review, or spec distillation."
  - transform: "Apply the smallest doc change that improves navigation, comprehension, or machine handoff."
  - verify: "Check links, ownership, duplicated claims, stale language, and expected artifact shape."
  - handoff: "Persist changed paths, unresolved ownership, and the next workflow."

elicitation_options:
  - reader_switch: "Ask how the document should differ for a human, future agent, maintainer, or reviewer."
  - stale_claim_hunt: "Find claims contradicted by current state, tests, or release notes."
  - shard_boundary: "Ask which section has a different lifecycle or reader than the rest."
  - spec_kernel: "Extract objective, facts, assumptions, risks, open questions, and next workflow from messy text."

facilitator_moves:
  - "Read before editing; do not churn docs without knowing ownership."
  - "Prefer compact machine handoff for agents and clearer prose for humans."
  - "Separate source-of-truth updates from generated summaries."
  - "When claims are ambiguous, record the ambiguity instead of smoothing it over."

quality_bar:
  - "The reader can find the right file and trust which claims are current."
  - "The transformed doc has a clear job and does not duplicate another source of truth."
  - "Future agents get smaller, sharper context rather than more prose to scan."

anti_patterns:
  - "Do not edit public docs with internal benchmark framing."
  - "Do not create new docs when one authoritative doc should be fixed."
  - "Do not hide unresolved ownership or stale claims behind nicer wording."

paths:
  fast_path: "Patch the smallest doc/index/spec artifact that resolves navigation or handoff."
  deep_path: "Map docs, split or distill content, run edge-case/editorial review, then update context and ADRs only when decisions are real."

checkpoint_options:
  - doc-index
  - doc-shard
  - editorial-review
  - edge-case-review
  - spec-distillation
  - workflow-validate

domain_examples:
  - doc-index: "Agents waste context finding docs; produce a compact map of source-of-truth files and read order."
  - doc-shard: "One doc is too large for recovery; split into focused shards and update the index without orphaning facts."
  - editorial-review: "Human-facing docs are unclear; flag structure, tone, unsupported claims, and scoped edits."
  - edge-case-review: "A plan looks plausible; stress-test boundaries, failure modes, missing checks, and waivers."
  - spec-distillation: "Transcript or messy brief is long; extract objective, facts, assumptions, risks, open questions, and next workflow."

artifact_rules:
  Persist target paths, source-of-truth boundaries, stale/duplicate notes, edits/findings, and next workflow.
  Use `skill:templates/document-utility-artifact.md` as the default artifact shape unless a narrower project template exists.

headless:
  Read the existing docs first. If ownership is unclear, return a source-of-truth question instead of making broad doc churn.
