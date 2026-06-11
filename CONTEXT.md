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

## Agent Runtime

The Forge Method surface meant for agents: compact skills, workflows, manifests, state files, evals, artifacts, gates, and recovery files. It optimizes for deterministic continuation and low context cost.

## Agent Profile

A compact routing manifest that tells the runtime when a focused agent role is useful, which inputs it needs, which outputs it must return, and what handoff state must be preserved.

## Agent Council

An optional Human Experience workflow where multiple specialist agents contribute perspectives to a high-risk, taste-heavy, or strategically important decision.

## Council Transcript

The live, human-visible discussion from Agent Council. It may be rich and exploratory, but it is not required future context.

## Council Decision Artifact

The compact durable artifact saved after Agent Council. It preserves participants, recommendation, agreements, disagreements, risks, decision, and next action without storing the full debate.
