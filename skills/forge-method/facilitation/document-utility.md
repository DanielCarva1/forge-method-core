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
  - editorial: "Is the main problem promise, order, voice, unsupported claim, density, or a confusing transition?"
  - edge_cases: "What boundary, misuse, failure, or recovery case would embarrass this artifact if ignored?"
  - proof: "How will we know the doc now works: validation, smaller context pack, clearer handoff, or fewer open questions?"
  - freshness: "What source file owns this fact, and what proof shows the index or shard was updated after that source?"

conversation_stages:
  - map: "Identify authoritative docs, generated docs, stale docs, and the reader job before editing."
  - select_mode: "Choose index, shard, editorial review, edge-case review, or spec distillation."
  - editorial_review: "Separate source facts from prose choices before changing tone, structure, or claims."
  - edge_case_review: "Turn boundary and failure scenarios into findings, missing checks, waivers, or follow-up stories."
  - transform: "Apply the smallest doc change that improves navigation, comprehension, or machine handoff."
  - verify: "Check links, ownership, duplicated claims, stale language, and expected artifact shape."
  - freshness_check: "Record source fingerprint, source mtime, precedence rule, and `artifact doc-check` result when indexing or sharding."
  - handoff: "Persist changed paths, unresolved ownership, and the next workflow."

elicitation_options:
  - reader_switch: "Ask how the document should differ for a human, future agent, maintainer, or reviewer."
  - stale_claim_hunt: "Find claims contradicted by current state, tests, or release notes."
  - editorial_pass: "Review promise, opening, order, sentence density, tone, claims, and source boundaries."
  - edge_case_hunter: "Enumerate boundary conditions, misuse cases, failure modes, missing checks, and recovery paths."
  - shard_boundary: "Ask which section has a different lifecycle or reader than the rest."
  - spec_kernel: "Extract objective, facts, assumptions, risks, open questions, and next workflow from messy text."

facilitator_moves:
  - "Read before editing; do not churn docs without knowing ownership."
  - "Prefer compact machine handoff for agents and clearer prose for humans."
  - "Separate source-of-truth updates from generated summaries."
  - "For editorial review, preserve meaning before improving style."
  - "For edge-case review, make every finding actionable: fix, check, waiver, risk, story, or reject."
  - "When claims are ambiguous, record the ambiguity instead of smoothing it over."

quality_bar:
  - "The reader can find the right file and trust which claims are current."
  - "The transformed doc has a clear job and does not duplicate another source of truth."
  - "Index and shard artifacts prove freshness with source fingerprint, source mtime, and a stale-check command."
  - "Future agents get smaller, sharper context rather than more prose to scan."
  - "Editorial findings include reader job, source boundary, severity, and scoped edit recommendation."
  - "Edge-case findings include scenario, failure mode, detectability, missing check, and follow-up route."

anti_patterns:
  - "Do not edit public docs with internal benchmark framing."
  - "Do not create new docs when one authoritative doc should be fixed."
  - "Do not hide unresolved ownership or stale claims behind nicer wording."
  - "Do not keep a source document and shards both authoritative without a precedence rule and waiver."
  - "Do not call a general critique an edge-case review without boundary/failure scenarios."
  - "Do not rewrite style in ways that change product commitments or runtime contracts."

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
  - doc-index: "Agents waste context finding docs; read each file, produce a compact source map, record fingerprints, and prove stale docs were checked."
  - doc-shard: "One doc is too large for recovery; split into focused shards, update the index, and decide delete/archive/keep for the original."
  - editorial-review: "Human-facing docs are unclear; preserve facts while flagging reader job, structure, tone, unsupported claims, and scoped edits."
  - editorial-structure: "A doc has the right facts but wrong order; propose a new flow and mark claims that need source ownership."
  - edge-case-review: "A plan looks plausible; stress-test boundaries, failure modes, missing checks, waivers, and follow-up stories."
  - edge-case-hunter: "A workflow works for happy paths; enumerate misuse, stale-state, missing-input, permission, and recovery cases."
  - spec-distillation: "Transcript or messy brief is long; extract objective, facts, assumptions, risks, open questions, and next workflow."

artifact_rules:
  Persist target paths, source-of-truth boundaries, source fingerprint, source mtime, precedence rules, stale/duplicate notes, edits/findings, validation command, and next workflow.
  Use `skill:templates/document-utility-artifact.md` as the default artifact shape unless a narrower project template exists.

headless:
  Read the existing docs first. If ownership is unclear, return a source-of-truth question instead of making broad doc churn.
