# Context Glossary

## Forge Method Core

The repository and distributable runtime package. It contains the plugin manifest, skill, workflow references, helper scripts, templates, tests, and documentation.

## Forge Method

The user-facing method invoked from Codex through the `$forge-method` skill. It is the operating experience, not the repository name.

## Plugin Distribution

The primary package shape for Forge Method Core. It includes `.codex-plugin/plugin.json`, skills, scripts, assets, docs, templates, and a marketplace entry so Codex can install the runtime as a reusable workflow bundle.

## Marketplace Root

The directory used as the base for plugin marketplace resolution. A repo or team marketplace stores its catalog at `<marketplace-root>/.agents/plugins/marketplace.json` and plugin bundles under `<marketplace-root>/plugins/`; marketplace `source.path` values are relative to this root.

## Plugin Deeplink

A `codex://` URL printed by the local plugin installer to open the plugin detail page or workspace share flow in the Codex app. It improves onboarding without adding a separate runtime command surface.

## Plugin Installation Diagnostic

The `doctor` check that reads the personal marketplace, resolves the local plugin source, checks manifest and skill files, compares installed version with runtime version, and prints Codex deeplinks for activation and sharing.

## Hot Start Stub

The compact, stable `SKILL.md` surface that tells Codex how to invoke the launcher without carrying the full evolving runtime behavior. Normal product evolution should live in runtime scripts, workflow references, and release notes so an update can be used in the same start.

## Self-Update

The launcher behavior that checks a Git marketplace install for a newer Forge Method Core package before normal startup, applies the update when policy allows, and continues the same `preflight` or `start` flow without asking the user to initialize Forge twice.

## Manual Update

The explicit user-triggered update path for an installed Forge Method Core package. It is initiated through the `$forge-update` operational maintenance skill, runs the Codex Git marketplace upgrade, reads the local patch notes feed, and reports a short human summary without changing project state.

## Operational Maintenance Skill

A skill used to maintain or repair the installed runtime package rather than advance a Method Project. `$forge-reload` and `$forge-update` are operational maintenance skills; they are allowed exceptions to the single product entrypoint because they restore or update the package and then hand control back to `$forge-method`.

## Patch Notes Feed

A compact release summary read from `release-notes/latest.json` and printed after a successful self-update. It is human-facing, appears once per installed version, and must not pollute machine-readable stdout.

## Clone Install Smoke

A distribution smoke that clones a Git source or published ref, installs the cloned package into an isolated temporary plugin marketplace, verifies plugin metadata, runs preflight, creates a project, and runs the quality gate. It proves the install path a new user would take without relying on chat memory or the local development checkout.

## Creation Runtime

A state-machine system that turns intent into artifacts, implementation, validation, release, and future evolution.

## Assisted Production Runtime

A runtime mature enough for real projects with an agent/operator in the loop. It must be installable, recoverable after context loss, auditable from files, and release-validated, but it does not imply marketplace-polished one-click distribution.

## Start Route

The deterministic entry route that resolves whether the current workspace is an existing method project, the runtime repo, or a folder that can create or contain method projects.

## Preflight

A non-mutating route and context check. It identifies project identity, known project choices, required human choice, first files to read, and next helper commands before the agent loads broad context or starts work.

## Preflight Decision

A machine-readable decision contract returned by preflight. It lists safe options such as opening an existing project, choosing an external workspace, or creating a new project, with required user inputs and commands for the selected option.

## Runtime Snapshot

A machine-readable JSON summary of current project state, sprint counts, next story, route recommendation, quality findings, context paths, and recent artifacts.

## Operational Status

A compact status summary for humans and agents. It highlights route recommendation, next action, next story, open required input, open review findings, audit status, recommended agents, and context load plan without dumping the full snapshot.

## Resume Guidance

A structured, non-mutating decision for the next safe action. It identifies the action, target, whether it can proceed autonomously, minimal files to read, commands to run, done conditions, and blocking conditions.

## Guidance Engine

The canonical runtime subsystem that interprets the latest human message together with durable Forge state, sprint/story context, artifacts, and available workflows. It classifies intent, detects signals, recommends phase/workflow/action, returns a human prompt and alternatives, and says whether state must be updated before continuing.

## Workflow Catalog

The runtime metadata registry for packaged workflows. It records workflow id, reference alias, phase, required/optional status, follow-up workflows, expected outputs, and optional facilitation pack. Guidance Engine uses it to explain route decisions and future agents use it to validate module workflow references.

## Project Configuration

The validated team/local configuration under `.forge-method/config/` for one Method Project. It is the only supported project-local surface for runtime behavior overrides, conventions, and custom capability entries.

## Override Model

The deterministic customization contract that applies Project Configuration over packaged runtime defaults. Precedence is packaged defaults, then team config, then local config; only validated workflow metadata, agent profile metadata, project conventions, and capability index entries may override runtime-visible behavior.

## Capability Index

A generated compact summary of effective Forge capabilities for future agents. It is derived from packaged metadata plus valid Project Configuration, and should be regenerated instead of edited by hand.

## Lifecycle Closure

A guided workflow family that turns major project moments into durable next-step artifacts: track choice, project context, session prep, readiness, code review, retrospective, and research closeout. It wraps low-level runtime helpers with a human-facing ritual and a compact agent handoff.

## Track Decision Artifact

A durable artifact that records why one Forge track/module is the right route for the project or increment. It maps the selected track to required artifacts, optional workflows, rejected tracks, and the next workflow.

## Project Context Artifact

A durable source-of-truth summary for an existing or evolving project. It captures purpose, architecture shape, conventions, important artifacts, validation commands, and agent handoff boundaries without replacing the generated Context Pack.

## Root Integrator Project

A Method Project that coordinates a product made of multiple Product Areas or repos. It owns the integration map, cross-area contracts, release evidence, and collaboration conventions, but does not swallow standalone area state after a repo split.

## Product Area

A product-owned boundary inside a Method Project, usually mapped to paths, owners, contracts, dependencies, validation commands, and split criteria. Product Area is not a Forge Module; Forge Module remains the packaged runtime/workflow concept.

## Team Operating Model

The durable agreement for a multi-human Forge project: GitHub organization/repo shape, owners, review policy, trunk-based branch rules, CI expectations, agent usage, and release cadence.

## Collaboration Handoff

A compact artifact for handing work between humans or agents. It records product area, owner, branch or pull request, decisions, validation evidence, blockers, and the next accountable actor.

## Repo Split

The planned extraction of a Product Area into its own repository. A repo split requires an owner, public contract, validation boundary, release boundary, integration evidence, and a retained link back to the Root Integrator Project.

## Standalone Method Project

A Method Project whose `.forge-method/` state lives inside its own repo and can be operated without the Root Integrator Project. A Product Area becomes standalone after a repo split when its new repo receives compact context, contracts, validation commands, and integration handoff.

## Session Prep Artifact

A compact continuation brief for the next working session. It uses current state, checkpoints, open inputs, review findings, and load-plan guidance to name the exact next workflow and files to read.

## Readiness Matrix

A cross-artifact implementation readiness map linking PRD/spec, UX, architecture, risk, stories, validation, open inputs, and review findings. It proves whether build can start without hidden decisions.

## Research Closeout

A compact handoff after evidence work. It records sources, confidence, decision impact, unresolved uncertainty, rejected paths, and the next workflow so research does not remain as loose notes.

## Guided Depth Family

A related set of specialized workflows that deepens a broad track without changing the single Forge entrypoint. Current depth families include game lifecycle, test architecture, builder utility, and document utility. Each family keeps compact workflow state machines for agents and a separate facilitation pack for human conversation.

## Game Studio Depth

The guided depth family for game projects. It preserves player fantasy, playable slice, engine profile, design artifacts, production stories, playtest learning, and performance proof without collapsing game work into generic software planning.

## MDA Lens

The integrated game-design lens used by Game Studio to connect intended player experience to desired dynamics, supporting mechanics, UI/feedback signals, and proof. It is a facilitation and quality lens inside existing game workflows, not a separate workflow or track.

## MDA Trace

The compact agent-facing artifact field produced by game brief work and preserved by later game workflows. It records `target_aesthetics`, `player_experience_hypothesis`, `desired_dynamics`, `supporting_mechanics`, `feedback_and_ui_signals`, `proof_or_playtest`, and `unresolved_risks` so future agents do not reduce a game to a feature list.

## Playable Slice

A bounded build target that lets the player actually do the core action loop. It is smaller than the dream game and more concrete than a technical prototype.

## Engine Profile

A compact engine-specific setup contract used by `engine-setup`. It captures project structure, language/runtime assumptions, asset pipeline, test commands, performance budgets, and known engine risks without creating separate Forge entrypoints per engine.

## Game Context Artifact

A durable game-specific source-of-truth summary. It captures player fantasy, loop, references, chosen engine profile, design artifacts, playable slice, validation proof, and next game workflow for future agents.

## Builder Factory

The guided depth family for creating and validating Forge runtime extensions: module ideation, agent builder, workflow builder, module builder, and module validation. It gives humans a coached creation loop while future agents receive compact workflow contracts, templates, catalog metadata, and validation commands.

## Facilitation Pack

A human-facing conversation guide for a workflow. It contains open-floor prompts, source-material intake, follow-up question batches, stage-by-stage conversation scripts, elicitation options, facilitator moves, quality bars, anti-patterns, fast/deep paths, checkpoint options, artifact rules, and headless behavior. It is intentionally separate from agent-facing workflow state machines so human guidance can be rich without bloating agent recovery context.

Facilitation packs are part of the product contract, not optional polish. If a workflow is routed to humans through Guidance Engine, its pack must make the interaction feel guided, tasteful, and useful while the paired `workflow-*.md` remains compact for agents.

## Human Input

A durable request for user judgment under `.forge-method/inputs/`. Required open human input sets `human_input_required` and blocks autonomous progression until answered or deferred.

## Review Finding

A durable review issue under `.forge-method/reviews/`. Open findings are tied to one story, appear in context recovery, and must be resolved or waived before the story can be marked `done`.

## Method Project

A project initialized with `.forge-method/` state. A method project may be software, product, creative, game, automation, or runtime-module work.

## Example Project

A runnable method project seeded from a packaged module. It exists to prove the module can initialize state, stories, artifacts, evals, checkpoint memory, and quality gates before users adapt the module to real work.

## Fixture Matrix

A release smoke that creates an example project and a normal project for every packaged module, then verifies quality gates, compact recovery, parent preflight project choices, and objective-to-module recommendations. It proves module breadth instead of only the default software path.

## Project Scaffold

A normal method project created from a packaged module. It includes durable state, a kickoff story, project brief, artifact eval, checkpoint, context pack, and context load plan.

## Module Recommendation

A deterministic ranking of packaged modules against a project objective. It helps the agent ask one useful project-creation question and can drive `project create --module auto` when an objective is explicit.

## Runtime Repo

The Forge Method Core repository itself. The runtime repo must not be confused with a method project created by the runtime.

## State Ledger

The durable source of project truth: `state.yaml`, `projects.yaml`, `sprint.yaml`, story files, and `ledger.ndjson`.

## Evidence Ledger

Files under `.forge-method/evidence/` plus ledger events proving that work was completed, checks ran, or a release gate passed.

## Context Pack

A bounded project context artifact under `.forge-method/context/current-pack.md`. It contains current state, active story, next action, recent evidence, artifacts, review findings, and recovery signals for normal continuation.

## Context Load Plan

A machine-readable recovery artifact under `.forge-method/context/load-plan.json`. It ranks the files an agent should load now, with reason, priority, estimated size, and deferred items when the context budget is full.

## Context Health

A read-only runtime check that turns the current context load plan into a continuation level: `ok`, `watch`, `compact`, or `blocked`. It tells the agent when to keep working, checkpoint soon, write compact recovery, or split work before loading more context.

## Recovery Brief

A focused resume artifact under `.forge-method/context/recovery.md`. It lists read order, resume commands, current state, recent checkpoints, failed checks, touched files, and recent artifacts for a new agent context.

## Compact Recovery

A budget-preserving handoff artifact under `.forge-method/context/recovery-compact.md`. It keeps state, resume guidance, read order, commands, done conditions, and blocking conditions ahead of optional history.

## Checkpoint

A structured progress memory written during long-running work. It captures summary, decisions, checks, touched files, artifacts, and next action without requiring chat replay.

## Eval

A small local check under `.forge-method/evals/` that proves a workflow target exists, validates structurally, and matches the expected route for a query.

## Eval Kind

The objective type of a local eval. Supported kinds verify workflow routing, workflow trigger coverage, and artifact availability.

## Story

A bounded executable unit of work with acceptance criteria, status, optional checks, and required evidence before `done`.

## Story Backlog

A JSON batch of story definitions exported from or imported into a method project. It is used to move larger planned work as one durable package instead of typing each story manually.

## Artifact Link

A durable relationship between an artifact and a story. Linked artifacts are checked during audit so story context does not silently disappear.

## Artifact Lifecycle

The state of a project artifact. Durable artifacts must remain available. Ephemeral artifacts may be deleted only after their result is captured in the artifact index, story, evidence, or checkpoint.

## Ready Gate

The transition into `5-ready-operate`. It requires audit success, no active implementation/review stories, release evidence, and readiness state.

## Release Batch

A grouped set of related changes that is large enough to publish as one version. Small stories may be committed during development, but they should not each become a tag or GitHub release unless they are urgent fixes.

## Story Release

A version published for one completed story when the project is intentionally delivered story by story. It is a valid cadence, but it should not be used when several completed stories already belong to one larger product increment.

## Release Check

A non-publishing readiness check for a release batch. It verifies local version metadata, changelog readiness, manifest alignment, runtime version alignment when present, and git cleanliness when the folder is a checkout.

## Readiness Audit

A documented maturity check that maps install, start, project creation, durable state, context recovery, autonomous build, ready phase, distribution, and product-surface requirements to concrete evidence.

## Quality Gate

A deterministic verification command that runs project audit, artifact verification, workflow validation, and local evals before a project advances or is declared ready.

## Operational Readiness

The local environment and package state needed to work efficiently: toolchain status, route detection, audit status, and the recommended verification tier for the touched area.

## Validation Tier

The amount of verification chosen for the current risk. Fast validation covers normal development, targeted smokes cover touched runtime surfaces, and full verification is reserved for release, install/package changes, or broad runtime changes.

## Agent-Facing Workflow

A compact Markdown state machine loaded only when the current runtime state requires it.

## Human Experience

The Forge Method surface meant for people: conversational guidance, taste, agent personalities, live council debates, onboarding, tutorials, and user-facing explanation. It may be warmer and more expressive than runtime files.

Human Experience uses Guidance Engine output to decide what to say next. It should not invent routes from chat memory when durable state and the latest human intent disagree.

## Human Voice Layer

The Human Experience policy applied to non-JSON guidance. It makes Forge warm, direct, opinionated, and adaptive while keeping machine-readable runtime surfaces compact.

## Persona Lens

A human-facing role or coach overlay that shapes live guidance, council participant selection, and elicitation technique choice. It references compact Agent Profiles and workflows, but it is not itself an Agent Profile.

## Elicitation Technique

A named facilitation move used by a Persona Lens or Facilitation Pack to help a human think, decide, challenge, or converge. It is compact metadata for guidance, not long prompt narration or required future state.

## Adaptive Energy

The Forge behavior of matching the user's conversational energy without attacking the user. It may be playful, blunt, or profane toward ideas, bugs, and process when appropriate.

## Tasteful Pushback

The Forge behavior of challenging weak ideas, impossible promises, unsafe products, and bad assumptions directly while preserving respect for the human.

## Agent Runtime

The Forge Method surface meant for agents: compact skills, workflows, manifests, state files, evals, artifacts, gates, and recovery files. It optimizes for deterministic continuation and low context cost.

Agent Runtime consumes Guidance Engine output as compact JSON, commands, state update hints, and workflow references. It keeps the routing contract small enough for future agents to resume without replaying the conversation.

## Reality/Evidence Gate

A discovery and planning check that tests physical possibility, technical feasibility, user pain, ethics, safety, legal risk, alternatives, and minimum evidence before treating an idea as viable.

## Market Scan

A compact evidence workflow that checks audience, alternatives, demand assumptions, adoption friction, and invalidation signals before making market claims.

## Domain Scan

A compact evidence workflow that checks domain constraints, norms, harms, accepted practices, and required expert or primary-source review.

## Technical Feasibility Scan

A compact evidence workflow that checks whether a product promise is technically possible with the available data, tools, integrations, budget, platform, and operational constraints.

## Agent Profile

A compact routing manifest that tells the runtime when a focused agent role is useful, which inputs it needs, which outputs it must return, and what handoff state must be preserved.

## Agent Council

An optional Human Experience workflow where multiple specialist agents contribute perspectives to a high-risk, taste-heavy, or strategically important decision.

## Council Transcript

The live, human-visible discussion from Agent Council. It may be rich and exploratory, but it is not required future context.

## Council Decision Artifact

The compact durable artifact saved after Agent Council. It preserves participants, recommendation, agreements, disagreements, risks, decision, and next action without storing the full debate.

## Mechanical Autonomy

The default Forge behavior for procedural work after decision phases are settled. It lets the agent create stories, implement, review, repair, test, write evidence, update sprint state, and advance readiness without asking for procedural confirmation.

## Grill Gate

A phase-closing decision check for discovery, specification, and planning. It challenges the phase artifact against glossary terms, ADRs, constraints, risks, and acceptance criteria before mechanical work can proceed.

## Mechanical Work Order

A compact runtime payload returned by resume, next, and guide. It names the next autonomous step, required context, commands, done conditions, self-repair conditions, stop conditions, commit policy, and whether Codex Goal mode is recommended.

## Correct-Course Continuation

The policy for contradictions discovered during mechanical work. The agent writes a compact correct-course artifact, chooses the conservative interpretation that preserves the approved spec, updates state, and continues.

When the human message rejects the current route, names a failure, or shows strong frustration, Guidance Engine may route to Correct-Course Continuation even if the previous `next_action` points at release, publication, or normal operation.

## Codex Goal Handoff

A generated objective for Codex Goal mode. It turns a Forge Mechanical Work Order into durable success criteria for a long-running Codex task.

## Commit Policy

A project setting that controls automatic commits during mechanical work. The default is `off`; project configuration may choose `story` or `epic`.

## Test Architecture Enterprise Depth

The guided quality depth family for high-risk, enterprise, brownfield, or release-sensitive projects. It preserves engagement mode, risk model, fixture architecture, CI command contract, NFR evidence, traceability, gate decision, and waiver status without turning every quality request into generic test advice.

## Quality Engagement Model

The selected quality posture for a request: advice, design, implementation, review, audit, or release gate. It decides which test architecture workflow should run next and what evidence future agents must preserve.

## Fixture Architecture

A framework-neutral test utility contract: pure helper, framework wrapper, composition surface, lifecycle cleanup, and command evidence. Specific tools such as Playwright or Cypress are examples inside project artifacts, not separate Forge entrypoints.

## Traceability Gate

A two-phase quality workflow that first maps requirements and risks to planned checks, then makes a release-time decision from actual evidence. It is not a green checklist unless evidence exists.

## Gate Decision

The compact release-quality outcome consumed by readiness and release workflows. Valid meanings are pass, concerns, fail, missing evidence, or waived; waived requires owner, rationale, release impact, and revisit trigger.

## Quality Waiver

A documented decision to proceed despite a known quality gap. It must name the waived risk, missing evidence, accountable owner, rationale, expiry or revisit trigger, and downstream release impact.
