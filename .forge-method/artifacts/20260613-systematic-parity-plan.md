# Systematic parity plan

- kind: internal-parity-plan
- created_at: 2026-06-13
- source_audit: `.forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md`
- benchmark_artifact: `.forge-method/artifacts/guidance-engine-benchmark.md`
- sandbox: `%TEMP%/forge-bmad-sandbox`
- docs_cache: `%TEMP%/forge-bmad-docs`
- status: planning-complete-for-next-implementation-batches

## Purpose

This plan turns the BMAD-to-Forge parity audit into an execution program.

The goal is not to copy BMAD public product language. The goal is to translate useful behavior into Forge Method principles:

- rich, guided, tasteful human experience;
- compact, deterministic, file-backed Agent Runtime;
- state-machine workflow docs;
- scripts, hooks, fixtures, gates, evidence, checkpoints, and install validation where reliability matters;
- one public entrypoint: `$forge-method`.

## Source Snapshot

Primary sources already captured locally:

- BMAD Builder docs cache: `%TEMP%/forge-bmad-docs/builder.txt`
- Creative Intelligence Suite docs cache: `%TEMP%/forge-bmad-docs/cis.txt`
- Game Dev Studio docs cache: `%TEMP%/forge-bmad-docs/game.txt`
- Test Architecture Enterprise docs cache: `%TEMP%/forge-bmad-docs/test-arch.txt`
- BMAD sandbox skills/config: `%TEMP%/forge-bmad-sandbox`

Observed sandbox registry inventory:

- `bmad-help.csv`: 67 entries
- `skill-manifest.csv`: 69 entries
- registry families: Core, BMad Builder, Creative Intelligence Suite, Game Dev Studio, Test Architecture Enterprise
- Builder registry entries: setup, agent build/analyze, workflow build/analyze/convert, module ideate/create/validate
- GDS registry gaps relevant to Forge: project context, implementation readiness, code review, retrospective, document project

## Translation Unit

Every translated capability must produce a Forge-native unit. A unit is complete only when the relevant parts below exist:

1. Catalog route:
   - `skills/forge-method/catalog/workflows.json` entry when the behavior is routeable.
   - `module/*.yaml` membership when the behavior belongs to a track.
   - Guidance Engine signal/routing when humans naturally ask for it.

2. Human experience:
   - facilitation pack with stages, elicitation options, facilitator moves, quality bar, anti-patterns, and headless rule.
   - concise human prompt/route output.
   - no requirement that the human know internal phase names.

3. Agent runtime:
   - compact workflow reference with `trigger`, `inputs`, `steps`, `outputs`, `done_when`, `blocked_when`, `handoff`.
   - template when the workflow creates an artifact.
   - state update command if the workflow is executable from `guide`.

4. Proof:
   - unit test or replay fixture for route behavior.
   - workflow validation coverage.
   - install smoke coverage when packaged behavior changes.
   - evidence and checkpoint after implementation.

5. Public boundary:
   - product-facing docs describe Forge by its own model.
   - BMAD remains an internal benchmark only.

## Completion Model

Capability states:

- `translated`: Forge has route, human pack, compact workflow contract, artifact/template if needed, tests, installed validation, and evidence.
- `strong`: Forge already exceeds or matches the behavior, but may still need richer examples or replay coverage.
- `partial`: Forge has a concept but lacks one or more translation-unit pieces.
- `deferred`: useful but intentionally postponed with rationale and revisit trigger.
- `non-goal`: out of Forge scope; document why and what Forge uses instead.

The full parity goal is complete only when every audit row is `translated`, `strong`, `deferred`, or `non-goal` with explicit rationale and proof.

## Current Status

P0 through P1.7 are closed:

- Help/Next-Step Oracle: implemented and validated.
- Post-command Help Oracle: implemented after 1.29.0 so progress-changing commands record compact next-workflow guidance in `ledger.ndjson`, and interactive mutations print the next required workflow, alternatives, and stale-state guard.
- Architecture Guidance Depth: implemented after 1.29.0 with architecture artifact template, create/update/validate/tradeoff metadata, deeper PRD/UX/security/interface/test-hook/story-impact facilitation, and replay precedence over generic quality routing.
- Context Boundary Recovery: implemented after 1.29.0 with fresh-chat boundary metadata in reload/resume/Help Oracle, context-recovery pack/template, compact recovery contract, and replay coverage for interrupted chat/network context.
- Brainstorming Depth: implemented after 1.29.0 with richer guided divergence/convergence facilitation, option lanes, taste and anti-reference pressure testing, discard pile, compact brainstorming artifact, modes, and replay proof before PRD.
- CIS Facilitation Depth: implemented after 1.29.0 with dedicated design-thinking, innovation-strategy, and storytelling templates/packs/modes plus narrow creative routing so specific CIS requests do not collapse into generic creative-session.
- Agent Compactness Guard: implemented after 1.29.0 with `workflow compactness`, stricter workflow/facilitation shape checks, audit integration, smoke-runtime coverage, and unit proof that compact agent state machines stay separate from rich human packs.
- Human facilitation coverage gate: implemented and validated.
- PRD/UX/Quick Dev depth: implemented with workflows, packs, templates, routes, and fixtures.
- Story lifecycle proof: implemented with `story-creation`, decision-source guard, and mechanical no-procedural-confirmation tests.
- Parity replay harness: implemented as packaged `parity replay`; installed smoke validates the packaged replay matrix.
- Builder Factory: implemented with `module-ideation`, `agent-builder`, `workflow-builder`, `module-builder`, `module-validate`, `builder-factory` facilitation, templates, Guidance Engine routes, and parity replay coverage.
- Customization and Capability Index: implemented with Project Configuration, Override Model, Capability Index, `config index`, config validation, replay coverage, install smoke proof, and evidence.
- Persona and Elicitation Layer: implemented with Persona Lens overlays, elicitation technique index, persona facilitation pack, Guidance Engine `persona_lens` output, council participant routing, replay coverage, compactness guards, and evidence.
- Lifecycle Closure: implemented with `track-decision`, `project-context`, `session-prep`, `code-review`, `retrospective`, `research-closeout`, readiness matrix template, lifecycle facilitation, Guidance Engine routes, replay coverage, compact workflow refs, and Capability Index exposure.
- Game Studio Depth: implemented with `game-context`, `engine-setup`, expanded GDD/narrative/mechanics/prototype/playtest/performance/QA contracts, game-lifecycle facilitation, Guidance Engine routes, replay fixtures, compact workflow refs, templates, and validation coverage.
- Test Architecture Enterprise Depth: implemented with Quality Engagement Model, Fixture Architecture, narrow TEA templates, expanded quality facilitation, two-phase Traceability Gate semantics, waiver contract, Guidance Engine routes, replay fixtures, compact workflow refs, and validation coverage.
- Parity Closure Utilities: implemented with `investigation`, `working-backwards-challenge`, `sprint-status`, `checkpoint-preview`, `adversarial-review`, compact refs, templates, routes, module membership, and parity replay coverage.

The remaining work is a planned sequence, not ad hoc patching.

## P1 Execution Batches

### P1.1 Builder Factory

Status: translated in the 2026-06-14 Builder Factory batch.

Scope:

- `module-ideation`
- `agent-builder`
- `workflow-builder`
- `module-builder`
- `module-validate`

Why:

Builder is the clearest missing parity cluster. BMAD has a coached creation loop for agents, workflows, modules, validation, and setup. Forge currently has `builder-scaffold`, `agent-analyze`, `workflow-analyze`, `skill-convert`, `workflow-validate`, and low-level install scripts, but no end-to-end builder factory experience.

Forge translation:

- Human: guided module/agent/workflow ideation and quality challenge before scaffolding.
- Agent: compact builder workflows, templates, module metadata, validation scripts, and install proof.
- Runtime: route builder requests to narrow builder workflows instead of generic `runtime-builder`.

Deliverables:

- workflow refs for all five workflows;
- facilitation pack `builder-factory` or expanded `builder-utility`;
- templates for agent, workflow, module plan, module manifest, validation report;
- catalog metadata with modes and followed-by links;
- Guidance Engine routing and parity replay cases;
- module validation command or extension of existing `builder validate`;
- install smoke coverage if packaged validation changes.

Done when:

- source tests pass: done;
- `parity replay` includes builder factory cases: done;
- workflow validate passes: done;
- installed smoke validates new packaged workflows: done;
- evidence and checkpoint recorded: done.

### P1.2 Customization And Capability Index

Scope:

- Forge override model for workflow metadata, facilitation packs, templates, agent profiles, project conventions;
- user-editable compact capability index;
- config validation and conflict reporting;
- install persistence behavior.

Why:

BMAD exposes customization and registry/help behavior as first-class capabilities. Forge has `config-customization`, `config inspect`, and `config validate`, but lacks a coherent override contract and generated capability index.

Forge translation:

- Human: explain what can be customized and what cannot.
- Agent: deterministic merge order, conflict errors, and compact generated index.

Deliverables:

- ADR or artifact for override precedence;
- validation rules for allowed override keys;
- generated capability index command or artifact;
- docs for project-local vs package-level customization;
- fixtures for conflicting overrides and stale help/index entries.

Done when:

- invalid customization fails loudly;
- valid customization changes runtime-visible behavior predictably;
- generated index is compact and install-safe.

Status:

- implemented in P1.2 with Project Configuration, Override Model, Capability Index, `config index`, `config-customization` facilitation, replay fixture, and targeted tests;
- validation evidence recorded.

### P1.3 Persona And Elicitation Layer

Status: translated in the 2026-06-14 Persona Lens and Elicitation Layer batch.

Scope:

- optional human-facing persona descriptors for PM, Architect, Analyst/Researcher, UX, QA, Game, Builder, Tech Writer;
- advanced elicitation technique index;
- richer Council participant routing;
- CIS coach-style routes for brainstorming, design thinking, innovation strategy, problem solving, storytelling.

Why:

The benchmark is stronger at making the human feel guided by specialized roles. Forge already has compact agent profiles and some persona fields, but the human persona layer is not systematic.

Forge translation:

- Human: richer voice and role choice only in live guidance/council/facilitation.
- Agent: profiles stay compact; persona text must not bloat state, workflow docs, or recovery packs.

Deliverables:

- persona overlay artifact or pack;
- routeable elicitation options in facilitation packs;
- council participant routing;
- replay cases for role/persona selection;
- guard that compact runtime output does not include long persona narration.

Done when:

- humans can ask for a PM/Architect/UX/QA/Game/Builder lens and get a useful route: done;
- future agents still receive compact profiles and workflow contracts: done.

### P1.4 Product, Context, Review, And Retrospective Closure

Status: translated in the 2026-06-15 Lifecycle Closure batch.

Scope:

- track decision artifact;
- project-context/document-project workflow;
- session-prep workflow;
- general retrospective workflow;
- code-review workflow or explicit build-story review stage;
- architecture/readiness matrix linking PRD, UX, architecture, risk, stories, validation;
- research closeout handoff.

Why:

P0 fixed routing and first-cycle guidance, but several lifecycle closeout behaviors remain partial. These are the workflows that keep future agents from drifting after discovery, implementation, or release.

Forge translation:

- Human: understandable status/next-step rituals after each major artifact.
- Agent: compact context packs, readiness matrices, review findings, retrospective action items, and next workflow state.

Deliverables:

- workflow refs and packs for project-context, session-prep, retrospective, code-review;
- readiness matrix template;
- research closeout template;
- route/replay fixtures;
- context compactness checks where useful.

Done when:

- a future user can ask "document this project", "prep next session", "review this code", or "retro this increment" and Forge routes correctly: done;
- output is durable and compact enough for a future agent: done.

### P1.5 Game Studio Depth

Status: translated in the 2026-06-15 Game Studio Depth batch.

Scope:

- game project context;
- engine setup workflows/templates for likely engines only if in Forge scope;
- GDD, narrative, mechanics richer packs/templates;
- quick prototype proof contract;
- playtest and performance evidence templates;
- end-to-end game replay from brief to first playable slice.

Why:

Forge has many game workflow ids, but depth is uneven. BMAD Game Dev Studio provides game-specific lifecycle rituals; Forge must preserve game domain specificity while staying Codex-native.

Forge translation:

- Human: player fantasy, loop, constraints, engine choice, proof target, and playtest learning are explicit.
- Agent: compact game artifacts, story order, validation maps, and proof commands.

Deliverables:

- game-context and engine-setup workflows: done;
- expanded packs/templates for GDD, narrative, mechanics, prototype, playtest, performance, and game QA: done;
- replay fixture coverage for game lifecycle routes: done;
- optional smoke example without heavy engine setup: deferred until a specific lightweight engine/app fixture exists.

Done when:

- game projects no longer collapse to generic software planning: done;
- first playable slice has decision sources, story order, and validation proof: done in compact templates/workflow contracts.

### P1.6 Test Architecture Enterprise Depth

Status: translated in the 2026-06-15 Test Architecture Enterprise Depth batch.

Scope:

- engagement model expansion;
- fixture architecture pattern;
- test design/ATDD matrix;
- CI and automation command contracts;
- NFR evidence matrix;
- two-phase traceability and gate decision semantics;
- waiver policy.

Why:

TEA is the largest benchmark surface. Forge has the workflow ids and some packs, but depth and proof semantics are not complete.

Forge translation:

- Human: choose the correct quality engagement and understand risk tradeoffs.
- Agent: explicit risk/test/evidence/waiver/gate artifacts, command maps, and release implications.

Deliverables:

- expanded `test-architecture` pack and templates: done;
- fixture architecture section in `test-framework`: done;
- traceability template with design-time and release-time phases: done;
- NFR matrix template: done;
- replay fixtures for each quality mode: done;
- install/runtime smoke only where packaged behavior changes: pending final batch validation.

Done when:

- quality requests route to the right quality artifact: done;
- release gates can distinguish pass, concerns, fail, missing evidence, and explicit waiver: done.

### P1.7 Parity Closure Utilities

Status: translated in the 2026-06-15 Parity Closure Utilities batch.

Scope:

- investigation/root-cause routing;
- working-backwards/PRFAQ-style challenge;
- sprint status ritual;
- checkpoint preview;
- adversarial/red-team review.

Why:

The parity audit still had small but important guided-flow gaps after the larger Builder, lifecycle, game, and TEA batches. These utilities are the exact workflows humans naturally ask for when they are stuck, checking progress, challenging assumptions, or preparing a handoff.

Forge translation:

- Human: short, direct route prompts that make the next guided ritual obvious without requiring phase names.
- Agent: compact workflow refs, templates, catalog/module metadata, state transition commands, and replay fixtures.

Deliverables:

- workflow refs for all five workflows: done;
- artifact templates for all five workflows: done;
- catalog and module membership: done;
- Guidance Engine signal/routing: done;
- parity replay cases: done;
- adversarial precedence fix so explicit red-team requests do not fall into generic quality review: done.

Done when:

- route fixtures pass: done;
- `parity replay` passes: done;
- workflow validation passes: done;
- full source and install validation recorded: pending final batch validation.

## P2 Decisions

P2 items are not implementation blockers. Decisions are recorded in `.forge-method/artifacts/20260615-p2-scope-decisions-and-polish-plan.md`:

- Persistent personal memory agents:
  - non-goal as broad personal memory;
  - Forge translation is project-local durable memory only.
- Presentation/deck craft:
  - deferred unless Forge scope expands into pitch/deck workflows.
- Isolated Docker eval runner:
  - deferred unless local evals need untrusted execution or reproducibility beyond Codex runtime.
- Hook wrappers:
  - deferred for future standalone Forge app/runtime; avoid adding Codex overhead now.
- API/browser utility layer:
  - deferred as generic utility layer; translate only through provider-specific test workflows when needed.

## Execution Rules

1. Do not implement a batch until its target rows are identified in the audit and the Forge translation unit is clear.
2. Do not add a workflow id without a route, pack, compact workflow doc, and proof plan unless it is explicitly agent-only.
3. Do not add human richness to compact workflow docs. Put human facilitation depth in packs and guide output.
4. Do not add public slash commands. `$forge-method` remains the entrypoint; runtime commands are agent surface.
5. Do not claim full parity until every audit row has status, rationale, and proof.
6. When a benchmark behavior is not a Forge goal, record `non-goal` with rationale instead of leaving it as an open gap.

## Validation Ladder

For planning-only updates:

- `artifact verify --root .`
- `audit --root .`

For workflow/catalog/routing updates:

- `python -m unittest discover -s tests`
- `python skills/forge-method/scripts/forge_method_runtime.py workflow validate`
- `python skills/forge-method/scripts/forge_method_runtime.py parity replay`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`

For install/package behavior:

- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`

For broad release:

- `scripts/verify-all`
- fixture matrix smoke
- clone/plugin install smoke from the intended ref
- release check

## Completion Audit Checklist

Before marking the full parity objective complete, inspect current evidence for each audit row:

- row has state: `translated`, `strong`, `deferred`, or `non-goal`;
- translated human-facing row has facilitation pack and replay/fixture coverage;
- translated agent-facing row has compact workflow/state/runtime contract;
- required templates/scripts/tests exist;
- installed `$forge-method` sees packaged behavior when relevant;
- artifact/evidence/checkpoint records the batch;
- public docs do not describe Forge as a clone, fork, or variant.

## Immediate Next Step

Next implementation batch: real-use transcript hardening for the remaining partial/strong-ish audit rows after the 1.29.0 release, Architecture Guidance Depth cleanup, Context Boundary Recovery, Brainstorming Depth, CIS Facilitation Depth, and Agent Compactness Guard.

Do not tag or publish from a partial test run. Start from this plan:

1. collect or synthesize focused transcripts for the next partial row;
2. add the smallest guidance/runtime/packs change that makes the transcript route and feel correct;
3. prove it with parity replay, unit coverage, runtime smoke, and install smoke when packaging is touched;
4. record evidence, checkpoint, state, and changelog notes without claiming full parity until the completion audit passes.
