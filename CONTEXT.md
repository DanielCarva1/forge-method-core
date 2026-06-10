# Context Glossary

## Forge Method Core

The repository and distributable runtime package. It contains the plugin manifest, skill, workflow references, helper scripts, templates, tests, and documentation.

## Forge Method

The user-facing method invoked from Codex through the `$forge-method` skill. It is the operating experience, not the repository name.

## Creation Runtime

A state-machine system that turns intent into artifacts, implementation, validation, release, and future evolution.

## Start Route

The deterministic entry route that resolves whether the current workspace is an existing method project, the runtime repo, or a folder that can create or contain method projects.

## Method Project

A project initialized with `.forge-method/` state. A method project may be software, product, creative, game, automation, or runtime-module work.

## Runtime Repo

The Forge Method Core repository itself. The runtime repo must not be confused with a method project created by the runtime.

## State Ledger

The durable source of project truth: `state.yaml`, `projects.yaml`, `sprint.yaml`, story files, and `ledger.ndjson`.

## Evidence Ledger

Files under `.forge-method/evidence/` plus ledger events proving that work was completed, checks ran, or a release gate passed.

## Context Pack

A compact recovery artifact under `.forge-method/context/` containing only the current state, active story, next action, and recent evidence needed to resume work.

## Checkpoint

A structured progress memory written during long-running work. It captures summary, decisions, checks, touched files, artifacts, and next action without requiring chat replay.

## Eval

A small local check under `.forge-method/evals/` that proves a workflow target exists, validates structurally, and matches the expected route for a query.

## Story

A bounded executable unit of work with acceptance criteria, status, optional checks, and required evidence before `done`.

## Artifact Link

A durable relationship between an artifact and a story. Linked artifacts are checked during audit so story context does not silently disappear.

## Artifact Lifecycle

The state of a project artifact. Durable artifacts must remain available. Ephemeral artifacts may be deleted only after their result is captured in the artifact index, story, evidence, or checkpoint.

## Ready Gate

The transition into `5-ready-operate`. It requires audit success, no active implementation/review stories, release evidence, and readiness state.

## Quality Gate

A deterministic verification command that runs project audit, artifact verification, workflow validation, and local evals before a project advances or is declared ready.

## Agent-Facing Workflow

A compact Markdown state machine loaded only when the current runtime state requires it.
