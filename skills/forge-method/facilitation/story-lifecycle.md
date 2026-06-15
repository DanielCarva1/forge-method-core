# facilitation: story-lifecycle

purpose:
  Move approved decisions into epics, sprint plans, implementation-ready stories, review, ready state, and retrospective learning.

open_floor:
  "What is the next executable slice, and what evidence proves it is ready to build or ready to use?"

source_material:
  Ask for spec, PRD, UX plan, architecture, track decision, enterprise artifact map, risk register, sprint state, story files, evidence, review findings, and release criteria.

follow_up_batches:
  - readiness: "Which decision artifacts are approved, missing, or contradicted?"
  - sprint_goal: "What should this batch prove for a user, risk, or learning milestone?"
  - slice: "What user value or risk proof should the next story batch deliver?"
  - acceptance: "What acceptance criteria and checks make each story implementation-ready?"
  - source_map: "Which PRD/spec/UX/architecture/test artifact justifies each ready story?"
  - evidence_map: "What evidence will prove each story is done without re-asking the human to continue?"
  - enterprise_map: "If the selected track is enterprise, which security/privacy/risk/NFR/release artifacts are required, missing, or waived?"
  - capacity: "What is the timebox, review bandwidth, or attention budget for this sprint?"
  - sequence: "What must happen first because of dependency, risk, or learning?"
  - closeout: "What gate, evidence, release, or retrospective closes this loop?"

conversation_stages:
  - load_decisions: "Load approved artifacts before creating or changing stories."
  - check_readiness: "Block story creation if requirements, architecture, UX, or validation are insufficient."
  - source_map: "Attach each ready story to source artifacts, acceptance evidence, and checks."
  - plan_slice: "Create epics/stories or sprint plan around user value, dependency, risk, capacity, and learning."
  - defer_honestly: "Mark work planned, blocked, or deferred instead of pretending everything is ready."
  - guide_execution: "Keep mechanical build autonomous once stories are ready."
  - close_loop: "Run gate, ready-release, or retrospective and route next evolution."

elicitation_options:
  - story_readiness: "Ask whether each story has user value, constraints, acceptance, checks, and evidence."
  - dependency_sort: "Order stories by risk, learning, dependency, and user-visible value."
  - sprint_cut: "Choose the smallest coherent batch that proves the sprint goal without mixing ready and speculative work."
  - rebalance: "Move work between ready, planned, blocked, and deferred when capacity or evidence changes."
  - no_story_yet: "Explain which missing decision artifact prevents honest story creation."
  - release_proof: "Map done work to gate, evidence, and ready criteria."
  - implementation_ready_cut: "Split ready stories from planned/deferred work when evidence or decisions are missing."

facilitator_moves:
  - "Do not create implementation stories before decision artifacts are good enough."
  - "Do not ask the human for procedural continue during mechanical build loops."
  - "Keep acceptance criteria observable and tied to evidence."
  - "Make the sprint goal about proof, not activity count."
  - "Treat a missing PRD/spec/UX/architecture/test source as a blocker, not a prompt to improvise."
  - "Use correct-course when late contradictions invalidate story assumptions."

quality_bar:
  - "Stories are implementation-ready and sequenced by value/risk."
  - "Mechanical work can continue from compact state without chat memory."
  - "Ready/release claims have evidence, checks, and next-operation guidance."
  - "Every ready build story has a decision-source map and validation map."
  - "When multiple decision artifacts exist, each ready story names the exact `decision_sources` that justify it."
  - "Sprint plans separate ready, planned, blocked, and deferred work with a clear reason for each bucket."
  - "Enterprise readiness carries required artifact coverage into build/release gates instead of hiding it as a generic risk."

anti_patterns:
  - "Do not use stories as a substitute for discovery or spec."
  - "Do not mark ready because code changed; require evidence."
  - "Do not let stale next_action override sprint/story reality."
  - "Do not turn all planned ideas into ready stories."
  - "Do not call a backlog dump a sprint plan."
  - "Do not let a ready story inherit a vague global source when several artifacts could justify different slices."

paths:
  fast_path: "Create or route the smallest story batch with checks and evidence expectations."
  deep_path: "Run create-epics, plan-sprint, readiness-check, build-story, gate, ready-release, retrospective."

checkpoint_options:
  - create-epics
  - plan-sprint
  - readiness-check
  - build-story
  - ready-release
  - release-readiness
  - retrospective

artifact_rules:
  Persist decision sources, story order, acceptance, checks, evidence, blockers, release status, and next workflow.

domain_examples:
  - story_creation: "Create implementation-ready stories only after accepted PRD/spec/UX/architecture/test sources exist."
  - sprint_planning: "Turn accepted decisions into a sprint goal, ordered story batch, validation plan, and deferred list before build."
  - sprint_rebalance: "If capacity or evidence changes, keep ready stories executable and move speculative work back to planned or blocked."
  - mechanical_loop: "When a story is ready in build phase, continue through start, implementation, review, fixes, checks, evidence, and ready gate without asking for procedural permission."
  - readiness_guard: "If a ready story lacks source artifacts or validation map, block with story-creation/readiness-check instead of starting build."
  - source_disambiguation: "When PRD, UX, architecture, and test artifacts all exist, pass the specific source that justifies each story."
  - enterprise_readiness: "When the track is enterprise, readiness-check maps risk-register, security-plan, privacy-data-plan, test-strategy, ci-quality-pipeline, nfr-evidence-audit, traceability-gate, release-readiness, conditional DevOps/compliance/observability, and waivers."

headless:
  Continue mechanical story work when artifacts are approved. Stop only for real blockers: missing decisions, access, destructive approval, unavailable services, or explicit scope change.
