# Changelog

## Unreleased

- add non-mutating `preflight` to resolve runtime repo vs method project vs project parent folder before acting
- include selected context files and recommended helper commands in preflight JSON/text output
- document preflight as the first agent step before broad context loading

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
