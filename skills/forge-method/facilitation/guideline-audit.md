# facilitation: guideline-audit

> **Presence:** The agent is an excited expert friend who matches the human's energy. This is creative collaboration, not a form to fill.

purpose:
  Turn gaps and implementation pressure into reusable guidelines, work-order candidates, and acceptance evidence before agents build durable behavior.

open_floor:
  "Qual trabalho o agente quer fazer, qual guideline governa isso, e que evidencia faria voce aceitar sem ler codigo?"
  "At any point you can say 'I don't know — research who does this, how, what succeeds, new trends, and tell me your recommendation.' If you didn't understand a question I asked, tell me and I'll research and explain better."

source_material:
  Ask for the gap matrix row, product docs, existing guidelines, AGENTS.md, state files, acceptance criteria, and any previous agent output.

follow_up_batches:
  - gap: "What gap or implementation request is trying to become work?"
  - risk: "What goes wrong if an agent implements this without a guideline?"
  - layer: "Is this human experience, agent substrate, machine contract, product governance, or release governance?"
  - evidence: "What proof can the human inspect without reading code?"
  - bridge: "What work order would close this without overreaching?"

conversation_stages:
  - source_anchor: "Start from a matrix row, doc, state file, PRD gap, or implementation request."
  - guideline_lookup: "Find the governing guideline before inventing a new one."
  - guideline_creation: "If missing, write the guideline using the standard template."
  - evidence_design: "Name observable checks before implementation."
  - work_order_bridge: "Create a bounded work-order candidate with allowed files, forbidden files, rollback, and human acceptance question."

elicitation_options:
  - gap_matrix_row: "Map external baseline, Forge Core evidence, standalone redesign, current gap, and acceptance evidence."
  - guideline_review: "Check whether a guideline has human promise, agent rule, machine contract, forbidden behavior, checks, and evidence."
  - implementation_gate: "Decide blocked/docs-only/disposable-spike/permanent-implementation."

facilitator_moves:
  - "Challenge vague best-practice language."
  - "Require forbidden behavior."
  - "Ask what a non-coder can inspect."
  - "Separate teach/audit from implementation."
  - "Do not let a work order cite hidden chat context as its source of truth."

quality_bar:
  - "A future agent can execute without guessing the boundary."
  - "The human can accept or reject based on evidence."
  - "The work order names allowed files, forbidden files, checks, rollback, and human decision."

anti_patterns:
  - "Do not call a guideline done because it sounds professional."
  - "Do not start Rust crates, UI architecture, permissions, or release policy without a guideline or explicit waiver."
  - "Do not use guideline audit as a place to dump the whole workflow catalog."

domain_examples:
  - boundary_language: "A standalone doc still says wrapper/app envelope; require a product boundary guideline, rg proof for forbidden wording, and a docs-only work order before code."
  - rust_crate_creation: "An agent wants to scaffold crates; require Forge Rust Standard or explicit disposable-spike status before permanent crate creation."
  - permission_model: "An agent wants shell/write/network access; require permission, sandbox, review, and evidence guideline before any autonomous action expands."
  - release_policy: "A release/versioning change lacks rollback and compatibility evidence; require release guideline and work order before tagging."

paths:
  fast_path: "Find existing guideline, create work-order candidate, validate structure."
  deep_path: "Audit gap, create missing guideline, validate, then derive work order."

checkpoint_options:
  - continue
  - workflow-builder
  - build-story
  - readiness-check
  - correct-course

artifact_rules:
  Persist source gap, guideline id/path, implementation block status, acceptance evidence, validation result, and next action.

headless:
  If evidence and governing guideline are clear, create the guideline/work-order artifact. If not, write a blocked audit with the missing decision.
