# Builder Factory grill with docs

- kind: grill-with-docs
- created_at: 2026-06-14
- scope: P1.1 Builder Factory
- status: closed-for-implementation

## Objective

Challenge the P1.1 Builder Factory plan against Forge Method Core language, ADRs, runtime docs, and the internal benchmark before patching runtime behavior.

## Evidence Read

- `CONTEXT.md`
- `docs/adr/0002-single-entrypoint-with-state-routing.md`
- `docs/adr/0005-mechanical-autonomy-and-grill-gates.md`
- `docs/adr/0008-guidance-engine.md`
- `skills/forge-method/references/workflow-runtime-builder.md`
- `skills/forge-method/facilitation/runtime-builder.md`
- `.forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md`
- `.forge-method/artifacts/20260613-systematic-parity-plan.md`
- local benchmark sandbox: `%TEMP%/forge-bmad-sandbox`

## Grill Questions

### 1. Does P1.1 need a new public command surface?

Recommended answer: no.

Finding: ADR 0002 keeps `$forge-method` as the single public entrypoint. The runtime can expose helper commands for agents, but human routing must happen through Guidance Engine and workflow catalog metadata.

Decision: add routeable workflow ids and runtime guidance; do not add public slash commands.

### 2. Where does human richness belong?

Recommended answer: in a facilitation pack, not in compact workflow files.

Finding: `CONTEXT.md` defines Facilitation Pack as the human-facing guide and Agent-Facing Workflow as compact state machine. `workflow-runtime-builder.md` is intentionally small.

Decision: create `builder-factory.md` as the rich human guide. Keep each `workflow-*.md` under the compact state-machine contract.

### 3. Is Builder Factory a term worth adding?

Recommended answer: yes.

Finding: the P1.1 plan uses Builder Factory as a coherent family, but `CONTEXT.md` did not define it. Without a glossary entry, future agents may treat it as an implementation nickname rather than product language.

Decision: add Builder Factory to `CONTEXT.md` as the Forge-native guided depth family for module, agent, workflow, and validation creation.

### 4. What behavior should be translated from the benchmark?

Recommended answer: guided creation loop, classification before scaffold, durable decision artifact, validation report, and next workflow handoff.

Finding: the benchmark builder registry has distinct module ideation, agent build/analyze, workflow build/analyze/convert, module create, and module validate capabilities. Forge already has analysis/conversion utilities; missing P1.1 work is the creation and validation factory path.

Decision: add `module-ideation`, `agent-builder`, `workflow-builder`, `module-builder`, and `module-validate` with templates and replay fixtures.

### 5. What must prove this is not decorative docs?

Recommended answer: catalog validation, Guidance Engine routing, parity replay, source tests, install smoke, and evidence.

Finding: the systematic plan requires route, pack, workflow doc, template, tests, installed validation, and checkpoint for each translated capability.

Decision: update `workflow validate`, `parity replay`, unit tests, and installed smoke coverage.

## Boundaries

- Human Experience: rich guided prompts, open floor, challenge questions, review stages.
- Guidance Engine: classify human request and select the narrow Builder Factory workflow.
- Agent Runtime: compact workflow docs, JSON guidance output, catalog metadata, transition command, templates.
- Correct-Course: only for rejected or failed routes.
- Evolve: lifecycle phase for improving Forge itself.
- Guide: command surface; Guidance Engine remains the routing subsystem behind it.

## Implementation Contract

- Add workflow refs for all five Builder Factory workflows.
- Add one facilitation pack shared by the family.
- Add templates for builder plan, module manifest, and validation report.
- Update catalog and runtime-builder module membership.
- Update routing so build/create/ideate/validate requests choose narrow workflows while analysis requests keep existing routes.
- Add replay cases and tests that fail if the routes collapse back to generic `runtime-builder`.

