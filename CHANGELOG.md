# Changelog

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
