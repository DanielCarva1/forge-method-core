# Changelog

## Unreleased

## 2.0.5

Hotfix: checkpoint and handoff continuation text no longer becomes official route state by default. `checkpoint --next-action` and `handoff --next-action` now write durable memory with non-authoritative suggestion labels unless `--update-state` is passed. Fleet request application now ignores legacy checkpoint/handoff route mutations that lack `state_update_authorized`, preventing an agent's own checkpoint from becoming the next Forge instruction.

## 2.0.4

Hotfix: Bash verification scripts now validate required option values before reading `$2` under `set -u`. `verify-fast.sh` and `verify-all.sh` now return a clean `Missing value for --flag` error instead of an `unbound variable` crash when flags such as `--test`, `--match`, `--workers`, `--timeout`, `--report`, or `--junit` are passed without a value.

## 2.0.3

Hotfix: `scripts/verify-fast.sh` now handles empty `--test` and `--match` arrays under Bash `set -u` / nounset. This fixes Git Bash and strict Bash runs where an empty declared array could be treated as unbound before the runner started.

## 2.0.2

Hotfix: `$forge-update` now resolves the installed plugin package from the personal Codex marketplace before printing version summaries, so legacy skill installs do not report stale `1.34.1` patch notes after refreshing to the v2 package. Version detection now prefers the `VERSION` file when it is newer than the plugin manifest, protecting updates from stale manifest metadata.

## 2.0.1

Hotfix: `agent_id` was not threaded through `append_request` in fleet-mode handoff and checkpoint. Requests showed `"default"` instead of the actual agent identity. Both `cmd_handoff` and `cmd_checkpoint` now pass `agent_id=_resolve_agent_id(args)` to `append_request`, matching the `append_ledger` call that was already correct.

## 2.0.0 — Flock Coordination

Forge Method Core v2.0.0 ships multi-agent flock coordination — the runtime-agnostic coordination protocol for human+agent fleets:

**Critical blockers fixed (G1/G2/G3):**
- **G1:** `state.yaml` gains `version` field + optimistic concurrency (`write_state` accepts `expected_version`; `VersionConflict` on stale version — conflict DETECTED, not silently lost). Backward compatible: `expected_version=None` preserves v1.34.1 behavior.
- **G2:** `handoff`/`checkpoint` are append-only in fleet mode (`agents/registry.yaml` present) — workers emit requests to `requests.ndjson` instead of mutating `state.next_action`. `--update-state` flag overrides for single-agent legacy.
- **G3:** `agents/registry.yaml` + `is_fleet_mode()` detection + `--agent-id` flag threaded through 36+ mutating commands + `FORGE_AGENT_ID` env support + agent attribution in every ledger entry.

**Coordination primitives (Principle 5, 18, 19):**
- `claim`/`unclaim`/`heartbeat` commands — lane claims with 30min TTL, collision detection, first-come first-served.
- `lanes` command — show all lane claim statuses (free/claimed/expired).
- `forge-commit` wrapper — stages ONLY files in the caller's claimed lanes (GAP-1 fix from POC).
- `requests poll`/`apply` — driver reads and applies pending worker requests with version check.

**Guidance safety (Principle D2):**
- Multi-agent anti-patterns encoded: write-without-expected-version, act-outside-claimed-lane, persist-worker-transcript-as-integration-memory.
- Autonomy anti-patterns encoded: approve-during-autopilot, spec-write-without-gate.

**Partner experience (Principle 14, 15, 16):**
- Research-always-on affordance in all 34 facilitation packs.
- Partner-grade presence directive in all packs + agent profiles.
- Grill-as-default extended to every decision-close point (handoff, transition, spec-lock).

**Integration quality (POC gaps):**
- Integration gate: `tsc --noEmit` + smoke script in `gate` (GAP-5).
- Type-augmentation conflict detection: scans for cross-lane `declare global`/`declare module` duplicates (GAP-2).
- `council standup` — fleet coordination summary (lane status, cross-deps, blockers).
- `contract create` — typed agent-contract artifacts with input/output/verification contracts.
- `spawn` — runtime-agnostic spawn directive emitter (writes to `spawns/<id>.yaml`; harness reads and spawns).

**Architecture:**
- `emit-agents-md` — generates AGENTS.md draft for Claude Code/OpenCode (human-approval gated).
- JSON Schema (Draft 2020-12) for workflow definitions.
- Evolve-reentry-routing-gap fixed: guidance engine routes evolve-phase build-intent to discovery (not builder).

**Validation:** 141/141 existing tests pass. 14/14 v2 unit tests pass. smoke-runtime + smoke-install pass. Gate: 24/24 evals + Integration: passed. Backward compatible (C2).

**Proof of concept:** Validated via Stable Investments POC (3 concurrent agents, 25 stories, commit-to-main chaos, zero state clobbers, zero lane collisions, typecheck exit 0).

## 1.34.1

Forge Method Core v1.34.1 ships an update-skill hotfix:

- make `$forge-update` migrate legacy/local installs through `codex plugin marketplace add DanielCarva1/forge-method-core --ref main` instead of only explaining that the install shape is unsupported
- fall back to the main-package refresh path when `codex plugin marketplace upgrade` fails
- fetch the latest patch notes feed from GitHub during migration so users still see version and feature summaries after updating
- keep failed migrations non-destructive and print the exact manual command when Codex CLI update commands fail

## 1.34.0

Forge Method Core v1.34.0 ships guided team collaboration, Product Areas, and repo split planning:

- add `Product Area`, `Root Integrator Project`, `Team Operating Model`, `Collaboration Handoff`, `Repo Split`, and `Standalone Method Project` as canonical collaboration terms
- separate Product Areas from Forge Modules, keeping Forge Modules reserved for runtime/workflow packaging and Product Areas for the product being built
- add guided workflows for `team-operating-model`, `product-area-map`, `trunk-based-plan`, `collaboration-handoff`, and `repo-split-plan`
- route team, GitHub org, trunk-based, CODEOWNERS, monorepo, multi-repo, ownership, handoff, modularization, and standalone repo requests through collaboration guidance before build work
- make monorepo-first Root Integrator Projects the default, with repo split only after owner, contract, validation, release boundary, and integration cost are explicit
- define repo split behavior so an extracted Product Area receives its own standalone `.forge-method/` context while the root integrator keeps compact integration evidence
- add optional Product Area, owner, branch, PR, dependencies, and handoff fields to story, sprint, status, and build work templates without adding solo-project overhead

## 1.33.0

Forge Method Core v1.33.0 ships MDA Game Lens and manual update support:

- add `MDA Lens` and `MDA Trace` to Game Studio guidance so player feeling, dynamics, mechanics, UI/feedback, and proof stay connected
- make generated game briefs include `mda_trace` by default, with legacy warnings and incomplete-MDA validation failures in `artifact game-check`
- improve game routing for MDA, player-feel, fun, mechanics/dynamics, and playtest prompts
- update game facilitation packs, workflow state machines, and templates to preserve MDA decisions through GDD, mechanics, UX, playtest, and lifecycle handoffs
- add `$forge-update` as an operational maintenance skill for explicit Git marketplace updates with concise patch notes
- document Manual Update and Operational Maintenance Skills while preserving `$forge-method` as the single product entrypoint

## 1.32.0

Forge Method Core v1.32.0 ships guided early visual proof and platform operations coverage:

- make early visual proof a recurring expectation in initial discovery/spec/game/product/UX routes, so broad ideas produce visible prototype direction before requirements harden
- add `visual-alignment-prototype` routing, facilitation, template, workflow metadata, artifact fields, and replay coverage for early visual feedback loops
- add `platform-ops` routing, facilitation, template, module, and workflow support for infrastructure, CI/CD, database, secrets, deploy, observability, and rollback conversations
- extend discovery, spec, research, and game brief artifacts with visible proof and visual reference fields so user-facing direction can be corrected before build
- add Forge Guideline Auditor as a reusable Codex/Core skill, plus `guideline-audit` workflow routing, facilitation, template, work-order fields, and regression tests for guideline/work-order/permanent-implementation requests before durable agent work
- replace monolithic unit-test discovery in `verify-fast`/`verify-all` with a responsive per-test runner that prints progress, applies per-test timeouts, supports focused tests, and reports the slowest tests without reducing smoke or validation coverage
- add intelligent test-suite observability: JSON/JUnit reports, debug mode with retained per-test logs, substring filtering, report-driven failure/slowest re-runs, and updated validation docs so agents stop falling back to opaque `unittest discover`

## 1.31.2

Forge Method Core v1.31.2 ships a guided research drift hotfix:

- keep strategic Forge standalone app, stack, interface-language, and Rust codebase-standard conversations in `research-needed` / `technical-feasibility-scan` instead of collapsing them into generic runtime-builder automation
- preserve `6-evolve` as the recommended phase when a ready project opens a research evolution cycle
- stop treating performance wording such as "mais rapido e performativo" as a rushed fast-path request
- add transcript regression coverage for long research, study, iteration, Dioxus/Tauri/Zed/pi.dev, and Rust codebase-reference guidance

## 1.31.1

Forge Method Core v1.31.1 ships a public-install hotfix:

- hide committed Forge core project state when Forge is running from an installed public package, so normal users are routed to choose a project workspace instead of being offered core-runtime continuation
- require maintainer intent for core runtime state through `FORGE_METHOD_CORE_DEV=1` or local `.forge-method/core-dev.local`, plus `--allow-runtime-state`
- tolerate UTF-8 BOM in `.codex-plugin/plugin.json` so Windows-authored plugin manifests still activate runtime-package protections

## 1.31.0

Forge Method Core v1.31.0 ships the parity closure and runtime utility increment:

- translate the remaining P2 parity utility surfaces as opt-in Forge contracts: isolated eval runner, hook/event plan, and API/browser utility workflow with packs, templates, scripts, and replay coverage
- route version/GitHub/tag publication skepticism to `release-readiness`, with replay coverage for a user claiming the repo still shows an older version
- add the current systematic parity completion audit artifact, separating translated/proved guidance families from P2 deferred surfaces and non-goals
- persist Help Oracle route diagnostics in recovery briefs and the capability index, so fresh agents can see required workflow, reason, context boundary, stale-state guard, and route surfaces without relying on chat history
- add `next --json` and route diagnostics in text `next`, so agents following the recommended continuation command keep Help Oracle reason, context boundary, commands, quality, and mechanical goal handoff without parsing a full snapshot
- expose compact quality summary in `resume`, `context plan`, and `context health`, so fresh-chat recovery blocks on project quality failures instead of reporting healthy context while gate-rejected workflow/config/builder/agent failures remain hidden
- expose compact quality summary in existing-project `reload` text and JSON, so stale-chat recovery surfaces cannot hide workflow/config/builder/agent quality failures that gate would reject
- expose compact quality summary in `start`, `status --brief`, and existing-project `preflight`, so bootstrap cannot say only `Audit: passed` while workflow/config/builder/agent quality would fail gate
- expose plugin installation diagnostics in snapshot, resume, context plan, context health, preflight, and reload output so agents can see outdated local Codex plugin versions and repair commands during bootstrap without running `doctor` separately
- run semantic artifact validators from the shared artifact surface, so workflow-declared spec/research/game/test/doc/discovery/enterprise artifacts cannot pass `artifact verify`, snapshot quality, or the quality gate when their specialized `artifact *-check` would fail
- expose workflow validation errors in snapshot quality so agents can see workflow/catalog/facilitation failures before relying on compact runtime state
- validate written capability-index files through config validation, snapshot quality, and the quality gate so compact agent capability contracts cannot become stale or misleading
- make local builder extension validation part of snapshot quality and the quality gate, so malformed `.forge-method/skills/*/SKILL.md` files cannot pass gate-only validation
- make the quality gate consume the full agent validation surface by sharing agent profile, elicitation technique, and Persona Lens checks through `agent_validation_errors`
- make the quality gate consume the full workflow validation surface by including workflow catalog metadata checks in `workflow_validation_errors`, so missing catalog templates or route metadata cannot pass gate-only validation
- add a product-facing docs independence guard so runtime-repo audit blocks public Markdown from describing Forge as a clone, fork, or variant of another framework while allowing Git clone/install language
- add a durable runtime guidance source guard so artifact index summaries, human input prompts, review findings, and story work fields reject misleading agent guidance before they enter snapshots, context packs, or runtime JSON
- add a recovery memory guidance guard so checkpoints, latest-checkpoint mirrors, context packs, and recovery briefs reject misleading agent guidance before future sessions consume them
- add a state guidance write guard so `write_state`, `audit`, and `gate` reject misleading next-action or route-reason guidance before it becomes durable context for future agents
- add a config and capability-index guidance safety guard so project conventions, custom capability summaries, agent profile text, and generated capability indexes reject misleading runtime guidance before future agents consume it
- add a Runtime Guidance Payload safety guard so parity replay rejects stale-route instructions in Guidance Engine payloads and preflight/reload/guide JSON are covered by the same contract
- add a Help Oracle guidance safety guard so runtime resume/snapshot/audit output rejects stale-chat or stale-state instructions while preserving durable-state-first recovery guidance
- add a workflow guidance safety guard so compact workflow refs fail validation if they tell agents to rely on chat memory, follow stale state, ask procedural continue confirmations, or dump catalogs
- add a facilitation specificity guard: human-facing packs now require `domain_examples`, workflow validation rejects generic packs, and every packaged pack has at least three situational examples
- trim remaining Guidance Engine test overhead by validating JSON contracts through direct runtime calls while keeping `guide` subprocess coverage for human text, empty-workspace, config/tracks, and mechanical CLI behavior
- tighten builder routing so test-loop optimization wording no longer false-routes to `skill-convert`, and convert lifecycle/game/TEA guidance contract loops to direct runtime calls while preserving CLI coverage elsewhere
- optimize Guidance Engine parity fixture tests by using the runtime replay contract directly, preserving broad guidance-route coverage while cutting the slow transcript replay path from minutes to seconds
- add first-class document and enterprise utility generators with `artifact doc-index`, `artifact doc-shard`, `artifact enterprise-track-map`, `artifact enterprise-readiness`, and `artifact enterprise-release-gate`, plus lifecycle/document handoffs, tests, and source/install smoke coverage
- add first-class test utility generators with `artifact test-framework`, `artifact test-automation`, and `artifact game-e2e-scaffold`, Test Architecture/game lifecycle handoffs, tests, and source/install smoke coverage
- add first-class game artifact generators with `artifact game-brief` and `artifact game-sprint-plan`, game facilitation/workflow handoffs, tests, and source/install smoke coverage
- add first-class research scan generator with `artifact research-scan` for market/domain/technical evidence closeouts, Evidence Research guidance, workflow handoffs, tests, and source/install smoke coverage
- add first-class spec kernel generator with `artifact spec-kernel`, write-spec workflow handoff, product-planning guidance, source/install smoke coverage, and phase-closeout generator audit
- improve discover-intent human guidance so first facilitation answers are shaped into `artifact discovery-closeout` fields before specification
- add first-class discovery closeout generator with `artifact discovery-closeout`, packaged `discovery-closeout-artifact` template, workflow metadata, and source/install smoke coverage
- add discovery closeout quality gate with `artifact discovery-check`, required Grill Gate handoff fields, and source/install smoke coverage for valid closeout artifacts before specification
- require generated-project discovery closeout before specification: an answered `initial-facilitation` input must have a durable discovery-intent artifact before `transition --phase 2-specification`
- require initial-facilitation answer paths to stay in guided discovery, keep zero stories, and expose clean first-question guidance
- require runtime and install smokes to assert generated-project first facilitation plus workspace open/reload selection output
- require install smoke to assert installed `guide` output exposes `Guidance:` and `First question:` lines for guided starts
- surface guided first questions as dedicated non-JSON `guide` output lines and render mechanical-build prompts as autonomous `Status:` text
- add workflow-specific first questions for guided replay coverage and require mechanical-build prompts to use autonomous status/evidence wording
- require parity replay to validate human-facing facilitated prompts with concrete first questions and compact `Signals:`/`Route:` reason summaries
- require parity replay to validate `state_updates` handoff coherence and Persona Lens route-reason markers, including replay output for `route_reason` and `state_updates`
- require parity replay fixtures to assert full mutating command sequences with `expected_commands` when guidance returns multiple state-changing commands
- require parity replay fixtures to assert council recommendations, Codex Goal handoff, and autonomous work-order flags, while keeping runtime meta-audit wording on `runtime-builder` instead of council routing
- require parity replay fixtures to assert `expected_persona_lens` whenever guidance returns a Persona Lens, with whole-phrase alias scoring and stronger QA/problem-solving precedence over generic architecture mentions
- require parity replay fixtures to assert `expected_template` for human-facing guided cases with catalog templates, protecting compact agent handoff artifacts from route-only regressions
- require parity replay fixtures to assert `expected_facilitation_pack` for human-facing guided cases, closing weak transcript coverage where correct routes could pass without protecting rich human guidance
- add Stale Guidance Guard to `artifact verify` so active parity/audit/plan artifacts warn on old mixed-verdict markers, and clean post-parity audit/plan wording that could route agents back to closed work
- fold presentation-master requests into `storytelling` with a `presentation-craft` Persona Lens, pitch/deck structure fields, and replay proof while keeping visual deck production deferred
- add Game Brief & Sprint Depth with living `game-brief` template/modes, `game-sprint-planning`, `artifact game-check`, richer game facilitation prompts, game-specific sprint planning routing, and replay coverage
- add Research Guidance Depth with specific market/domain/technical routing, `research-scan-artifact`, source-quality and contradiction fields, `artifact research-check`, and replay coverage
- add Spec Kernel Depth with `artifact spec-check`, `write-spec` spec-kernel template/modes, stable capability ID and preservation-map contract, companion/decision-log fields, product-planning facilitation depth, and replay coverage for create/update/validate/distill spec requests
- add Enterprise Artifact Map Depth with `artifact enterprise-check`, required and conditional artifact maps for enterprise track decisions, readiness and release gate consumption, waiver policy fields, enterprise templates, and replay coverage
- add E2E/Test Automation Depth with `artifact test-check`, framework detection proof, API/E2E scenario fields, semantic locator and visible outcome contracts, no-hardcoded-wait policy, run/fix evidence, game E2E artifact, and replay coverage for generated E2E routing
- add Document Utility Freshness Depth with `artifact doc-check`, source fingerprint/mtime fields, index/shard stale-check modes, original-document handling, precedence rules, stale waiver, and replay coverage for doc freshness routing
- add Module Distribution Depth with `module-distribution`, a compact distribution artifact, setup/config boundary fields, capability/help registry handoff, install/upgrade/legacy cleanup proof, and replay coverage for distributable module requests
- harden Game Production Depth with dedicated `game-story-creation` and `game-sprint-status` artifacts, workflow-specific human microcopy, game dev-story routing to mechanical `build-story`, and replay coverage for game create/status/dev/review/test/e2e transcripts
- add Council Orchestration Depth with a dedicated `council-decision` pack/template/modes, party-mode routing, richer live debate output, compact dissent/orchestration artifacts, and JSON worker/merge contracts
- add Document Review Depth with specialized `editorial-review` and `edge-case-review` templates, modes, document-utility facilitation, Guidance Engine precedence over generic quality review, and replay proof
- add Build Story Autonomy Depth with a structured `build-story` work order template, start/continue/review/evidence modes, full mechanical command map, JSON `loop` and `do_not_prompt` fields, and no-procedural-prompt Codex Goal handoff
- add Sprint Planning Depth with a dedicated `plan-sprint` artifact template, sequence/rebalance/validate modes, richer story-lifecycle guidance for sprint goals/deferred work, Guidance Engine precedence over generic quality wording, and replay proof
- add Story Decision Source Gate so implementation-ready stories require approved decision artifacts, persist explicit `decision_sources`, autoattach a single clear source, and require `--source` when multiple sources are available
- add Agent Compactness Guard with `workflow compactness`, stricter workflow/facilitation shape checks, audit integration, smoke-runtime coverage, and unit proof for progressive disclosure
- add CIS Facilitation Depth with dedicated design-thinking, innovation-strategy, and storytelling packs/templates/modes plus narrow creative routing and replay proof
- add Brainstorming Depth with richer guided divergence/convergence facilitation, option lanes, taste and anti-reference pressure testing, discard pile, compact artifact template, catalog modes, and replay proof for broad ideas before PRD
- add Context Boundary Recovery for fresh chats, network drops, reloads, and stale context: `reload`, `resume`, Help Oracle JSON, and post-command ledger now expose compact context boundaries, with a context-recovery facilitation pack/template and replay proof
- add Architecture Guidance Depth with architecture artifact template, create/update/validate/tradeoff metadata, deeper PRD/UX/security/interface/test-hook/story-impact facilitation, and Guidance Engine precedence for product architecture over generic quality routing
- add post-command Help Oracle guidance for progress-changing runtime commands: interactive mutations now print the next required workflow, alternatives, and stale-state guard, while path-output commands record the same compact contract in `ledger.ndjson`

## 1.30.0

Forge Method Core v1.30.0 ships the guided human experience increment:

- improve first-run human guidance so broad ideas start with a full brain dump, "what else is still in your head?", anti-goals, and fast-path versus coaching-path choice before artifacts narrow the product
- add observable Guidance Engine style contracts for human pace, including coaching, fast-path, diagnostic, divergent, evidence-first, repair, and mechanical modes
- tighten no-state routing so confused users go to problem-solving, explicit brainstorms stay divergent, research requests enter evidence-first scans, and frustrated guidance feedback triggers correct-course instead of stale project creation
- expand guidance stress coverage for broad game ideas, rushed/simple requests, lost users, brainstorm, research, drift, and frustrated/cold guidance, with source and installed-plugin validation
- document focused verification loops for short development checks while keeping full unit, runtime smoke, install smoke, and installed guidance stress for broader runtime changes

## 1.29.0

Forge Method Core v1.29.0 ships guided workflow depth as one coherent runtime batch:

- add Parity Closure Utilities with routeable `investigation`, `working-backwards-challenge`, `sprint-status`, `checkpoint-preview`, and `adversarial-review` workflows, templates, module membership, replay fixtures, and adversarial routing precedence
- polish Guidance Engine human output with a contextual `guide` lede, runtime-builder routing for human-experience plus agent-doc polish, and quieter Reality/Evidence Gate behavior for correction/runtime requests
- add Game Studio Depth with `game-context`, `engine-setup`, expanded GDD/narrative/mechanics/prototype/playtest/performance/game QA contracts, route fixtures, and compact game templates
- add Test Architecture Enterprise Depth with Quality Engagement Model, Fixture Architecture, narrow quality templates, two-phase Traceability Gate semantics, waiver contract, and TEA replay fixtures
- record P2 scope decisions: broad personal memory is non-goal, presentation/deck craft is deferred, Docker eval runner is deferred, hook wrappers are deferred, and generic API/browser utility surface is deferred into provider-specific test artifacts
- add Builder Factory guided depth with `module-ideation`, `agent-builder`, `workflow-builder`, `module-builder`, and `module-validate` workflows, facilitation pack, templates, Guidance Engine routes, and parity replay cases
- add Project Configuration override model and generated Capability Index with validation for workflow metadata, agent profile metadata, project conventions, and custom capability entries
- add Persona Lens and elicitation layer with PM, Architect, Analyst/Researcher, UX, QA, Game, Builder, Tech Writer, and CIS coach routing, council participant selection, and compact `persona_lens` runtime output
- add packaged `parity replay` for internal Guidance Engine fixture coverage across help, confusion, brainstorm, research, PRD, UX, architecture, quick-dev, story cycle, correct-course, builder, CIS/creative, game, and test architecture requests
- run parity replay during install smoke so the installed `$forge-method` validates the same guidance fixtures as the source checkout
- add `story-creation` workflow, artifact template, and Guidance Engine story-flow routing
- block mechanical build resume for implementation-ready stories that lack decision-source artifacts
- add tests proving ready build stories need decision sources and mechanical loops do not ask procedural continue prompts
- route PRD, UX, and quick-dev requests through Guidance Engine product-flow with executable workflow transitions
- add `quick-dev` spec-lite workflow, facilitation pack, and compact artifact template
- add product requirements and UX artifact templates plus create/update/validate workflow metadata
- add Help Oracle output to snapshot/resume/next/transition so the runtime reports the required next workflow from durable state
- preserve active `6-evolve` runtime-builder work even when readiness is still `ready`
- enforce facilitation pack coverage for human-facing workflows during workflow validation
- add facilitation packs for research, creative direction, product planning, UX, architecture, story lifecycle, decision gates, and enterprise readiness
- require initial human facilitation before creating stories for new projects
- route method-experience criticism to `correct-course` before `runtime-builder`
- make `correct-course` update active workflow and route metadata to avoid stale agent guidance
- recommend Codex Goal handoff for autonomous mechanical resume paths

## 1.28.0

Forge Method Core v1.28.0 hardens guidance audit routing and local install diagnostics:

- route method/runtime audit requests, dead-code concerns, misleading agent docs, and human-guided experience questions to `runtime-builder`
- add transcript coverage for runtime audit guidance
- make `doctor` print repair commands for stale local plugin installs
- document the current guidance parity verdict, script audit results, hook/tracing references, and isolated experiment plan
- clean up shell and PowerShell smoke/install script warnings
- bump release metadata to `1.28.0`

## 1.27.0

Forge Method Core v1.27.0 adds the native Guidance Engine:

- route substantive human messages through `guide --question --json`
- classify correction, confusion, brainstorm, research, creative, game, quality, builder, mechanical build, support, and evolution intent
- make ready projects enter `6-evolve` or `correct-course` when new critique or intent overrides stale release work
- update Hot Start to run Guidance Engine after resume when the invocation contains substantive intent
- add transcript fixtures and an internal benchmark artifact for guided human routing
- keep the emergency reload entrypoint from v1.26.4 in the same delivery batch

## 1.26.4

Forge Method Core v1.26.4 adds emergency reload support:

- add read-only `reload` runtime command for a fresh bootstrap contract
- add `$forge-reload` as a tiny escape hatch for stale chat instructions
- install and smoke-test both fallback skills
- include `reload` in startup update checks

## 1.26.3

Forge Method Core v1.26.3 tightens startup:

- forbid broad project reads before `preflight`/`start`
- remove `Project state: missing` from human startup output
- forbid stale "waiting for initialization details" phrasing
- frame missing-state folders as Forge setup choices

## 1.26.2

Forge Method Core v1.26.2 adds an anti-cache invocation contract:

- require every `$forge-method` invocation to run the launcher before answering
- forbid answering from stale chat state or previous initialization prompts
- make current filesystem and launcher output authoritative

## 1.26.1

Forge Method Core v1.26.1 tightens the first-run human voice:

- replace meta-instructions in empty and brownfield startup copy with direct human-facing language
- tell agents not to replace the runtime opening with dry `.forge-method/state.yaml` initialization wording
- keep the 1.26 Reality/Evidence Gate behavior unchanged

## 1.26.0

Forge Method Core v1.26 adds a stronger first-run Human Experience and canonical Reality/Evidence Gate:

- add Human Voice Layer, Adaptive Energy, and Tasteful Pushback guidance for non-JSON human surfaces
- make empty-workspace and brownfield startup explain Forge before showing technical state
- add `human_experience` and `reality_evidence_gate` payloads for guide/preflight JSON
- add Reality/Evidence Gate plus market, domain, and technical feasibility scan workflows
- keep runtime artifacts compact while allowing human-facing guidance to be warmer and more direct

## 1.25.0

Forge Method Core v1.25 adds single-pass self-update for marketplace installs:

- add launcher self-update before `start`, `preflight`, `guide`, and `resume`
- add a one-time patch notes feed from `release-notes/latest.json`
- keep JSON stdout clean by writing update messages to stderr
- add update policy environment overrides and a legacy install migration hint
- continue Forge startup after update without forcing a second chat initialization

## 1.24.0

Forge Method Core v1.24 adds mechanical autonomy and Grill Gate phase closeout:

- add Mechanical Work Order output for autonomous story, review, repair, ready, and goal handoff loops
- add Grill Gate and Correct-Course Continuation docs for decision-phase closure and late contradictions
- add autonomy and commit policy defaults for project state/config

## 1.23.0

Forge Method Core v1.23 expands the runtime into guided tracks, optional council decisions, builder customization, creative/game/enterprise workflows, and compact agent-facing artifacts:

- add guide, tracks, Agent Council, builder, and config helper commands behind the single `$forge-method` entrypoint
- add Human Experience and Agent Runtime glossary, operating model, and Agent Council ADR
- add compact planning, builder, creative, game, and enterprise workflow references
- add creative technique and game/enterprise artifact templates
- add optional persona and council role fields to packaged agent profiles
- allow explicit self-hosting initialization in the runtime repo with `project create --allow-runtime-state`
- preserve the story phase workflow when starting non-build stories instead of always routing to build
- prefer the active skill/plugin directory for runtime helper commands, with the legacy user install path as fallback
- route existing codebases without Forge Method state into brownfield discovery before specification, planning, or build
- add a repo marketplace catalog so Codex users can install Forge Method Core from GitHub
- add read-only context health guidance for early checkpoint and compact-recovery decisions
- add marketplace listing metadata and a first-run onboarding flow asset
- add onboarding asset validation to fast and full verification
- document the publication boundary between local/workspace distribution and public directory submission

## 1.22.0

Forge Method Core v1.22 broadens real-project fixture coverage:

- add Windows and POSIX fixture matrix smokes for all packaged modules
- verify each module can create an example project and a normal project, pass quality gates, and generate compact recovery
- verify parent preflight decisions and objective-to-module recommendations across core, software, creative, game, runtime, test, and launch workflows
- include the fixture matrix in full release verification

## 1.21.0

Forge Method Core v1.21 hardens published plugin distribution:

- add a clone/install smoke that validates a Git-cloned package can install as a Codex plugin
- verify the cloned plugin manifest, marketplace entry, runtime preflight, project creation, and quality gate
- document the post-tag distribution smoke for Windows and POSIX release validation

## 1.20.0

Forge Method Core v1.20 improves first-project routing:

- add structured `decision` options to `preflight` JSON for existing projects, empty workspaces, and runtime repos
- print human-readable decision options in `preflight` text output
- include safe commands for opening existing projects or creating new projects from the selected objective
- add unit coverage for workspace, runtime-repo, and empty-workspace decision contracts

## 1.19.0

Forge Method Core v1.19 improves self-diagnosis:

- add plugin installation readiness to `doctor` JSON and text output
- report personal marketplace path, plugin source path, installed version, and Codex deeplinks
- tolerate UTF-8 BOM in marketplace and plugin manifest JSON files written by Windows tooling
- add unit coverage for plugin installation diagnosis with an isolated personal marketplace

## 1.18.0

Forge Method Core v1.18 improves plugin onboarding:

- print Codex plugin detail and share deeplinks from local plugin installers
- validate deeplink output in PowerShell and POSIX plugin-local smokes
- make the POSIX plugin installer resolve a working Python command instead of assuming `python3`
- clarify quickstart and distribution docs around plugin activation, skill invocation, and workspace sharing

## 1.17.1

Forge Method Core v1.17.1 hardens plugin distribution paths:

- compute local marketplace roots from `.agents/plugins/marketplace.json` so non-default marketplaces register the correct root
- write plugin `source.path` relative to the marketplace root instead of assuming a fixed install parent
- extend plugin-local smoke coverage to catch wrong marketplace registration guidance
- document personal, repo/team, workspace-shared, and public-listing distribution boundaries

## 1.17.0

Forge Method Core v1.17 hardens onboarding, operational readiness, and release maturity:

- expand `doctor` into an operational readiness report for project/runtime detection, toolchain status, and verification tier guidance
- add installed runtime launchers for Windows and POSIX shells so users do not need to know the runtime script path
- add a direct quickstart and v1 readiness audit so installation, startup, recovery, build, ready, and release maturity are explicit
- clarify installer next steps and add glossary terms for assisted production runtime, readiness audit, and operational readiness
- update v1 roadmap/backlog language so delivered capabilities are not still listed as future work
- reposition distribution docs around Codex plugin-first install, with manual skill install as the local fallback
- add local plugin marketplace installers and smoke coverage for plugin-first distribution

## 1.16.0

Forge Method Core v1.16 adds route-safe resume and compact handoff recovery:

- add non-mutating `preflight` to resolve runtime repo vs method project vs project parent folder before acting
- include selected context files and recommended helper commands in preflight JSON/text output
- document preflight as the first agent step before broad context loading
- add structured `resume` guidance for required input, review findings, stories, ready gate, and operation routes
- add `context recover --compact` for budget-preserving handoff briefs

## 1.15.0

Forge Method Core v1.15 adds release batching and operational planning:

- add fast verification scripts for normal development without install smokes
- document release batching so small stories do not become separate published versions
- add `release plan` to suggest story, batch, hotfix, or breaking version cadence without publishing
- add `release check` to validate local release readiness without publishing
- add `status --brief` and `status --json` for compact operational state summaries
- add `story export/import` for JSON story backlog batches
- add module recommendation and `project create --module auto` for objective-based project setup

## 1.14.0

Forge Method Core v1.14 adds durable review findings:

- new `review add/list/resolve/waive` helper commands store review findings under `.forge-method/reviews/`
- open review findings are included in snapshots, context packs, recovery briefs, and context load plans
- `story done` refuses completion while linked review findings remain open
- audit and quality gate fail when a done story still has an open review finding
- direct and installed smokes exercise review finding creation and resolution

## 1.13.0

Forge Method Core v1.13 adds real project scaffolding:

- new `project create` helper creates a normal method project from a packaged module
- new `project list` helper lists method projects under a folder
- project creation seeds state, kickoff story, project brief, artifact eval, checkpoint, context pack, and context load plan
- created projects start in discovery with `discover-intent` instead of remaining in route-only setup
- tests and smokes prove generated projects pass the quality gate with required evals

## 1.12.0

Forge Method Core v1.12 adds plan-aware context loading:

- new `context plan` helper writes `.forge-method/context/load-plan.json`
- load plans rank state, sprint, workflow, story, human input, agent profiles, artifacts, and evidence by priority
- load plans enforce a character budget and defer lower-priority files instead of loading everything
- recovery briefs include the load plan in read order and resume commands
- snapshots expose the load plan path when present

## 1.11.0

Forge Method Core v1.11 adds state-routed agent profiles:

- packaged compact profiles for facilitation, research, specification, planning, implementation, quality review, and operation
- new `agent list/show/recommend/validate` helper commands
- snapshots, context packs, and recovery briefs include recommended profiles for the current state
- quality gates validate packaged and project-local agent profiles
- smokes cover agent profiles for direct and installed runtime paths

## 1.10.0

Forge Method Core v1.10 adds durable human input control:

- new `input add/list/answer/defer` helper for project-blocking questions
- required open input sets `human_input_required` and routes `next` to the prompt
- answering or deferring input recalculates whether autonomous work can continue
- snapshots and context artifacts include open human input
- tests prove required input blocks and releases runtime state without chat memory

## 1.9.0

Forge Method Core v1.9 adds a machine-readable runtime snapshot:

- new `snapshot` helper emits deterministic JSON for agents and automation
- snapshot includes state, sprint counts, next story, route recommendation, quality findings, context paths, and recent artifacts
- tests prove a build-phase project exposes the next ready story without parsing human text
- direct and installed runtime smokes exercise snapshot output

## 1.8.0

Forge Method Core v1.8 improves context recovery:

- new `context recover` helper writes `.forge-method/context/recovery.md`
- recovery briefs include read order, resume commands, state, checkpoints, failed checks, touched files, and recent artifacts
- context packs now include recovery signals from recent checkpoints
- direct and installed runtime smokes cover recovery brief generation
- tests prove failed checks and touched files survive context reset through files

## 1.7.0

Forge Method Core v1.7 adds runnable examples for packaged modules:

- new `example list` helper for inspecting modules that can seed example projects
- new `example create` helper that initializes a project from a module
- seeded examples include state, sprint, story, artifact, artifact eval, checkpoint, context pack, and project guidance
- direct and installed runtime smokes prove example projects pass the quality gate with required evals
- PowerShell verification scripts now respect `PYTHON` or resolve an available Python command before running checks

## 1.6.0

Forge Method Core v1.6 strengthens local eval coverage:

- eval kinds now include `workflow-routing`, `workflow-trigger`, and `artifact-exists`
- generated workflows with triggers create trigger evals alongside routing evals
- artifacts can create existence evals with `artifact add --eval`
- the quality gate now benefits from objective workflow trigger and artifact availability checks
- direct runtime smokes exercise artifact existence evals before ready/release

## 1.5.0

Forge Method Core v1.5 adds a unified project quality gate:

- `gate` runs project audit, artifact verification, workflow validation, and local evals together
- `--require-evals` prevents a project from passing without configured eval coverage
- `--strict` can promote artifact freshness warnings to failures
- passing gates can write evidence and refresh the current context pack
- direct and installed runtime smokes run the gate before ready/release

## 1.4.0

Forge Method Core v1.4 improves artifact governance:

- artifact lifecycle metadata for durable and ephemeral artifacts
- `artifact capture` for preserving results before deleting temporary task docs
- `artifact verify` for missing artifact and stale summary checks
- project audit fails on missing active artifacts but permits captured ephemeral artifacts
- context packs and artifact listings expose artifact status and lifecycle

## 1.3.0

Forge Method Core v1.3 improves context recovery and long-running work:

- durable `checkpoint` command for structured progress memory
- latest checkpoint mirror under `.forge-method/context/latest-checkpoint.md`
- context packs include the latest checkpoint
- context packs include artifact summaries and linked story artifact summaries
- checkpoint smoke coverage across direct runtime and installed runtime paths

## 1.2.0

Forge Method Core v1.2 improves the first-run and resume experience:

- deterministic `start` helper for project routing
- existing project detection from nested folders
- known project listing from a parent workspace
- runtime repo protection during startup
- CI uses Node 24-compatible actions and an explicit Windows runner image
- tests covering start/resume routing without creating accidental project state

## 1.1.0

Forge Method Core v1.1 adds agent-facing hardening for project evolution:

- project workflow generation with state-machine validation
- project module generation
- local routing eval creation, listing, and execution
- artifact-to-story linking with audit coverage
- context pack size limits for controlled handoffs
- broader Windows and Linux smoke coverage for generated workflows and evals

## 1.0.0

Forge Method Core v1 establishes the first complete runtime foundation:

- file-backed project state under `.forge-method/`
- phase transitions from route through ready/operate
- story lifecycle with evidence-required done state
- artifact registry and context packs
- handoff and audit commands
- packaged module manifests
- workflow state-machine validation
- project guidance templates and local subagent profiles
- Windows and macOS/Linux installers
- Windows and Linux CI verification
- local `verify-all` scripts
