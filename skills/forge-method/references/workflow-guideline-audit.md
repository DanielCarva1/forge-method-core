# workflow: guideline-audit

trigger:
  - user asks to audit an external-framework-to-Forge, product, agent-native, guideline, or work-order gap
  - user asks to create or validate a guideline before implementation
  - a request would create permanent architecture, Rust crates, agent docs, permissions, release/versioning, or durable product behavior without a governing guideline

inputs:
  - source gap, matrix row, product doc, or implementation request
  - governing docs such as AGENTS.md, state, sprint, latest checkpoint, and relevant artifacts
  - existing guideline catalog or docs
  - acceptance evidence expected by the human

steps:
  1. load durable state and scoped source docs
  2. classify layer: human experience, agent substrate, machine contract, product governance, or release governance
  3. name the risk if agents implement immediately
  4. find the governing guideline or declare the missing guideline
  5. define acceptance evidence the human can judge without reading code
  6. create or update the guideline when needed
  7. create a work-order candidate only after the guideline/evidence is clear
  8. validate the guideline/work-order structure
  9. write evidence/checkpoint for meaningful project changes

outputs:
  - guideline audit artifact
  - guideline document or gap finding
  - work-order candidate
  - validation result
  - evidence/checkpoint path when project files changed

done_when:
  - the governing guideline is identified or the missing guideline is named
  - acceptance evidence is observable
  - implementation status is explicit: blocked, docs-only, disposable spike, or permanent implementation allowed

blocked_when:
  - no source gap or implementation request is clear
  - acceptance evidence cannot be stated
  - requested work would bypass an unresolved human gate

handoff:
  - preserve source gap, guideline id/path, work-order candidate, allowed/forbidden files, checks, evidence target, rollback, and human acceptance question
