# Changelog

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
