# facilitation: lifecycle-closure

purpose:
  Guide project closeout and continuation rituals: track decision, project context, session prep, readiness matrix, code review, retrospective, and research closeout.

open_floor:
  "Which lifecycle closure do we need now: choose the track, document the project, prep the next session, prove readiness, review code, run a retro, or close research?"

source_material:
  Ask for state, latest checkpoint, sprint/story files, artifacts, evidence, review findings, load plan, changed files, research notes, and the decision the next agent must preserve.

follow_up_batches:
  - route: "What workflow should become the next durable step, and what alternatives were rejected?"
  - sources: "Which artifacts are source-of-truth, stale, missing, or contradicted?"
  - track_artifacts: "If a track is selected, which required artifacts and gates does it impose?"
  - compactness: "What should future agents load, and what can remain out of recovery context?"
  - proof: "Which check, finding, matrix, or action item proves the closure is useful?"
  - ownership: "Who or what workflow owns the next action?"

conversation_stages:
  - classify_closure: "Choose the lifecycle closure workflow from the human wording."
  - gather_sources: "Load only the state, artifacts, findings, evidence, and changed files needed for the closure."
  - map_decision: "Preserve selected route, blockers, open questions, and rejected alternatives."
  - write_artifact: "Create a compact artifact using the workflow template."
  - route_next: "Name the next workflow, command, story, or human input."

elicitation_options:
  - source_map: "Map source artifact -> decision -> story/check/evidence."
  - gap_list: "Separate blocking gaps from warnings and future improvements."
  - next_session_cut: "Choose the smallest read set and first command for a future session."
  - review_triage: "Classify review findings by severity, evidence, owner, and repair route."
  - retro_loop: "Convert keep/change/stop/try into stories, inputs, or rejected work."

facilitator_moves:
  - "Do not confuse generated Context Pack with a durable Project Context Artifact."
  - "Do not store full discussion transcripts as future agent context."
  - "When reviewing code, create durable findings for actionable defects instead of burying them in prose."
  - "When prepping a session, name one next workflow and the exact files to load first."
  - "When closing research, record decision impact and unresolved uncertainty."
  - "When building a readiness matrix, link PRD/spec, UX, architecture, risk, stories, validation, open input, and review findings."

quality_bar:
  - "The artifact is compact enough for a future agent to use without chat history."
  - "The next workflow is explicit."
  - "Track decisions include required workflow and artifact maps; enterprise decisions use `artifact enterprise-track-map`, `artifact enterprise-readiness`, or `artifact enterprise-release-gate` for evidence gates and waivers."
  - "Source-of-truth artifacts and stale/missing artifacts are named."
  - "Open risks, findings, or human inputs are not hidden."

anti_patterns:
  - "Do not make lifecycle closure a decorative summary with no next action."
  - "Do not duplicate entire source docs into the closure artifact."
  - "Do not route code review to generic quality work when the human asked to inspect code/diff."
  - "Do not let a batch name like P1.4 Product override runtime-builder context."

paths:
  fast_path: "Classify the closure, write the compact artifact, and route the next workflow."
  deep_path: "Map sources, review gaps, write artifact, create findings or actions, update state/evidence, then checkpoint."

checkpoint_options:
  - track-decision
  - project-context
  - session-prep
  - readiness-check
  - code-review
  - retrospective
  - research-closeout

domain_examples:
  - session_handoff: "A new agent needs read order, blockers, first command, state mutation rules, and next workflow without replaying chat."
  - review_closeout: "A diff or artifact needs actionable findings, severity, source lines, repair route, and gate impact before readiness."
  - retrospective: "An increment shipped but learning is loose; capture keep/change/stop, evidence, follow-up stories, and release note impact."

artifact_rules:
  Persist route, source artifacts, key decisions, blockers, findings/actions, generator command, validation/evidence, load hints, and next workflow.

headless:
  Create the compact closure artifact from available files and continue the recommended workflow. Ask the human only when source-of-truth, ownership, or risk acceptance is unknowable from durable state.
