# facilitation: story-lifecycle

purpose:
  Move approved decisions into epics, sprint plans, implementation-ready stories, review, ready state, and retrospective learning.

open_floor:
  "What is the next executable slice, and what evidence proves it is ready to build or ready to use?"

source_material:
  Ask for spec, PRD, UX plan, architecture, risk register, sprint state, story files, evidence, review findings, and release criteria.

follow_up_batches:
  - readiness: "Which decision artifacts are approved, missing, or contradicted?"
  - slice: "What user value or risk proof should the next story batch deliver?"
  - acceptance: "What acceptance criteria and checks make each story implementation-ready?"
  - sequence: "What must happen first because of dependency, risk, or learning?"
  - closeout: "What gate, evidence, release, or retrospective closes this loop?"

conversation_stages:
  - load_decisions: "Load approved artifacts before creating or changing stories."
  - check_readiness: "Block story creation if requirements, architecture, UX, or validation are insufficient."
  - plan_slice: "Create epics/stories or sprint plan around user value and risk."
  - guide_execution: "Keep mechanical build autonomous once stories are ready."
  - close_loop: "Run gate, ready-release, or retrospective and route next evolution."

elicitation_options:
  - story_readiness: "Ask whether each story has user value, constraints, acceptance, checks, and evidence."
  - dependency_sort: "Order stories by risk, learning, dependency, and user-visible value."
  - no_story_yet: "Explain which missing decision artifact prevents honest story creation."
  - release_proof: "Map done work to gate, evidence, and ready criteria."

facilitator_moves:
  - "Do not create implementation stories before decision artifacts are good enough."
  - "Do not ask the human for procedural continue during mechanical build loops."
  - "Keep acceptance criteria observable and tied to evidence."
  - "Use correct-course when late contradictions invalidate story assumptions."

quality_bar:
  - "Stories are implementation-ready and sequenced by value/risk."
  - "Mechanical work can continue from compact state without chat memory."
  - "Ready/release claims have evidence, checks, and next-operation guidance."

anti_patterns:
  - "Do not use stories as a substitute for discovery or spec."
  - "Do not mark ready because code changed; require evidence."
  - "Do not let stale next_action override sprint/story reality."

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

headless:
  Continue mechanical story work when artifacts are approved. Stop only for real blockers: missing decisions, access, destructive approval, unavailable services, or explicit scope change.
