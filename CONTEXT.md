# Context Glossary

## Forge Method Core

The repository and distributable runtime package. It contains the plugin manifest, skill, workflow references, helper scripts, templates, tests, and documentation.

## Forge Method

The user-facing method invoked from Codex through the `$forge-method` skill. It is the operating experience, not the repository name.

## Creation Runtime

A state-machine system that turns intent into artifacts, implementation, validation, release, and future evolution.

## Start Route

The deterministic entry route that resolves whether the current workspace is an existing method project, the runtime repo, or a folder that can create or contain method projects.

## Runtime Snapshot

A machine-readable JSON summary of current project state, sprint counts, next story, route recommendation, quality findings, context paths, and recent artifacts.

## Operational Status

A compact status summary for humans and agents. It highlights route recommendation, next action, next story, open required input, open review findings, audit status, recommended agents, and context load plan without dumping the full snapshot.

## Human Input

A durable request for user judgment under `.forge-method/inputs/`. Required open human input sets `human_input_required` and blocks autonomous progression until answered or deferred.

## Review Finding

A durable review issue under `.forge-method/reviews/`. Open findings are tied to one story, appear in context recovery, and must be resolved or waived before the story can be marked `done`.

## Method Project

A project initialized with `.forge-method/` state. A method project may be software, product, creative, game, automation, or runtime-module work.

## Example Project

A runnable method project seeded from a packaged module. It exists to prove the module can initialize state, stories, artifacts, evals, checkpoint memory, and quality gates before users adapt the module to real work.

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

A compact recovery artifact under `.forge-method/context/` containing only the current state, active story, next action, and recent evidence needed to resume work.

## Context Load Plan

A machine-readable recovery artifact under `.forge-method/context/load-plan.json`. It ranks the files an agent should load now, with reason, priority, estimated size, and deferred items when the context budget is full.

## Recovery Brief

A focused resume artifact under `.forge-method/context/recovery.md`. It lists read order, resume commands, current state, recent checkpoints, failed checks, touched files, and recent artifacts for a new agent context.

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

## Quality Gate

A deterministic verification command that runs project audit, artifact verification, workflow validation, and local evals before a project advances or is declared ready.

## Validation Tier

The amount of verification chosen for the current risk. Fast validation covers normal development, targeted smokes cover touched runtime surfaces, and full verification is reserved for release, install/package changes, or broad runtime changes.

## Agent-Facing Workflow

A compact Markdown state machine loaded only when the current runtime state requires it.

## Agent Profile

A compact routing manifest that tells the runtime when a focused agent role is useful, which inputs it needs, which outputs it must return, and what handoff state must be preserved.
