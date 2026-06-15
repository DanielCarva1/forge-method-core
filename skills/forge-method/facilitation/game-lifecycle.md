# facilitation: game-lifecycle

purpose:
  Guide game-specific production work after the initial brief: game context, engine setup, GDD, narrative, mechanics, UX, PRD, prototype, stories, sprint status, playtest, performance, QA, retrospective, and game testing.

open_floor:
  "What part of the game lifecycle is unclear right now: player fantasy, playable slice, engine setup, GDD, mechanics, narrative, UX, stories, playtest, performance, QA, or sprint learning?"

source_material:
  Ask for brief, GDD, references, prototype notes, engine/platform constraints, engine setup notes, story files, playtest notes, performance targets, QA findings, and build evidence.

follow_up_batches:
  - lifecycle_stage: "Are we shaping, planning, building, validating, or learning from a finished slice?"
  - player_proof: "What player behavior or feeling must this stage protect?"
  - sprint_plan: "Which playable slice goal, story order, decision sources, and validation plan define this sprint?"
  - engine_profile: "Which engine/profile assumptions shape structure, commands, assets, and performance?"
  - playable_slice: "What can the player actually do when this stage is done?"
  - production_scope: "What is MVP, what is parked, and what must be rejected now?"
  - evidence: "Which playtest, automation, E2E, or manual proof shows this is real?"
  - e2e_smoke: "What is the shortest launch-to-result path, and what signal proves the player outcome happened?"
  - next_slice: "Which story or workflow moves the playable slice forward?"

conversation_stages:
  - locate_stage: "Name the lifecycle stage and why the broad game brief is no longer enough."
  - load_sources: "Read the existing brief, GDD, mechanics, UX, PRD, architecture, sprint, and evidence relevant to that stage."
  - engine_context: "Record the engine profile only where it changes structure, commands, assets, tests, or performance."
  - ask_stage_questions: "Ask only the questions that unlock the selected stage, not the whole game again."
  - plan_slice: "For sprint planning, order stories by player value, risk, dependencies, proof value, and deferred scope."
  - sprint_contract: "For game-sprint-planning, shape fields for artifact game-sprint-plan before story creation."
  - produce_handoff: "Create the lifecycle artifact, story order, status, retrospective, or test plan with source links."
  - transition: "Recommend the next workflow and whether durable state should enter it."

elicitation_options:
  - playable_slice: "Ask what the player can actually do at the end of this stage."
  - sprint_order: "Rank candidate stories by player value, uncertainty burned down, dependencies, and evidence produced."
  - dependency_walk: "Trace mechanics, UX, content, engine, assets, tests, and story dependencies."
  - risk_cut: "Identify which uncertainty should be proven before more content is created."
  - engine_profile_check: "Compare Godot/Unity/Unreal/Phaser or custom-engine constraints only when engine choice affects the next step."
  - playtest_signal: "Convert subjective fun/feel into observable tasks, signals, and design decisions."
  - e2e_scaffold: "Separate launch command, setup, player action, assertion, teardown, evidence mode, and release gate link."
  - retrospective_loop: "Convert observed play/build pain into next sprint decisions."

facilitator_moves:
  - "Do not re-run discovery when the user asks for a production-stage artifact."
  - "Tie every story or test back to player behavior or production risk."
  - "Treat game sprint planning as playable-slice planning, not generic backlog grooming."
  - "Keep game-specific context in the handoff so build-story does not become generic software work."
  - "Treat engine setup as a profile and proof contract, not as a reason to multiply Forge entrypoints."
  - "Use playable slice language instead of vague vertical-slice ambition when the next build must be testable."
  - "When a stage lacks source material, mark assumptions and open questions instead of inventing design."

quality_bar:
  - "The output is stage-specific and executable by the next agent."
  - "Player experience, production constraints, and validation are all visible."
  - "The route advances the game lifecycle instead of looping in ideation."
  - "Sprint planning preserves playable slice goal, decision sources, ordered story batch, validation plan, deferred scope, next story, and sprint update."
  - "artifact game-sprint-plan registers the playable-slice sprint before game-story-creation or build-story consumes it."
  - "Game E2E proof has a stable launch command, observable success signal, evidence capture mode, and release gate handoff."

anti_patterns:
  - "Do not collapse UX, PRD, story creation, status, and test requests into generic game-brief."
  - "Do not route game sprint planning to generic plan-sprint when player slice order and source decisions matter."
  - "Do not create implementation stories that lack player proof or acceptance evidence."
  - "Do not let engine details erase the intended player feeling."
  - "Do not call an E2E scaffold done without setup/action/assertion/teardown and evidence mode."

paths:
  fast_path: "Choose the specific game workflow, produce a compact artifact with assumptions, and route the next story."
  deep_path: "Run brief/GDD/PRD/UX/test framework in sequence, then create implementation-ready stories."

checkpoint_options:
  - game-ux-design
  - game-prd
  - game-context
  - engine-setup
  - gdd
  - narrative-design
  - mechanics-design
  - quick-prototype
  - playtest-plan
  - performance-plan
  - game-qa-review
  - game-story-creation
  - game-sprint-planning
  - game-sprint-status
  - game-retrospective
  - game-test-framework
  - game-test-automation
  - game-e2e-scaffold
  - build-story

domain_examples:
  - game-context: "A future agent needs the real game state; summarize player fantasy, loop, engine profile, playable slice, artifacts, proof, and next workflow."
  - engine-setup: "Engine is selected but setup is not durable; define structure, first-run command, asset pipeline, validation, and performance assumptions."
  - gdd: "Brief is accepted; expand into pillars, systems, content, progression, UX/feedback, engine assumptions, playable slice, and proof."
  - narrative-design: "Story or world matters; bind premise, player role, content units, tone, and quest scope to mechanics and slice."
  - mechanics-design: "Rules or balance are the risk; map player decisions, feedback, resources, failure states, prototype tests, and evidence."
  - game-ux-design: "Player cannot understand combat feedback; produce HUD/control/onboarding assumptions and UX checks before story work."
  - game-prd: "GDD has ideas but no implementation boundaries; convert pillars into MVP requirements, parked scope, and acceptance evidence."
  - quick-prototype: "A big idea needs proof; choose the smallest playable player action, asset stubs, proof command/manual check, and next decision."
  - game-story-creation: "Next playable slice is clear; create stories with player value, engine notes, asset assumptions, and checks."
  - game-sprint-planning: "A slice needs sequencing; order story batch by player value, risk, dependencies, decision sources, validation, deferred scope, and next story."
  - game-sprint-status: "Team asks what is actually playable; summarize done/blocked/deferred stories against the slice target."
  - game-retrospective: "Playtest or sprint finished; convert learning into keep/change/stop actions and backlog updates."
  - playtest-plan: "Prototype exists; define target players, tasks, observation method, pass/fail signals, and decision map."
  - performance-plan: "Frame time, memory, load, input latency, or multiplayer risk matters; define budget, scenarios, checks, and optimization story."
  - game-qa-review: "A slice/story needs review; inspect playability, feedback, stability, performance, scope, evidence, and repair route."
  - game-test-framework: "Engine exists but QA is ad hoc; define test layers for mechanics, saves, UI, content, and multiplayer if relevant."
  - game-test-automation: "Manual playtest found a repeatable failure; select deterministic setup, command, assertion, and evidence path."
  - game-e2e-scaffold: "Release needs launch-to-result proof; define launch command, setup/action/assertion/teardown, observable success signal, evidence mode, and readiness gate link."

artifact_rules:
  Persist lifecycle stage, source docs, decisions, parked scope, validation evidence, E2E smoke proof, next story/workflow, and unresolved risks.
  Use artifact game-sprint-plan for game-sprint-planning handoffs.
  Use `skill:templates/game-lifecycle-artifact.md` as the default artifact shape unless a narrower project template exists.

headless:
  Route to the narrowest game workflow supported by current artifacts. If source material is thin, write assumptions and open questions instead of inventing game facts.
