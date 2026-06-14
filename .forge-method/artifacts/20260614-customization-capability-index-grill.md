# P1.2 Customization And Capability Index Grill

- created_at: 2026-06-14T23:25:00+00:00
- workflow: runtime-builder
- plan: P1.2 Customization and Capability Index

## Objective

Close the customization and registry gap without adding decorative docs. Project customization must affect runtime-visible behavior, and future agents need a compact effective capability view.

## Benchmark Finding

The internal benchmark exposes customization as a first-class guided capability: it discovers configurable agents/workflows, chooses team or personal scope, writes override files, and verifies merged behavior. It also ships a compact help/registry table that agents can consult without reading every workflow.

Forge should not copy that public product shape. Forge needs the same product capability through native state, config, workflow catalog, Guidance Engine, validation, and generated artifacts.

## Grill Questions Resolved

1. Where does customization live?
   Recommended answer: in Project Configuration under `.forge-method/config/`, not scattered workflow docs.
   Resolution: accepted.

2. What is the precedence model?
   Recommended answer: packaged defaults, then team config, then local config.
   Resolution: accepted and recorded in ADR 0009.

3. Is the capability index manual or generated?
   Recommended answer: generated. Manual registry files become stale and mislead agents.
   Resolution: accepted.

4. Which layer owns routing?
   Recommended answer: Guidance Engine consumes effective workflow metadata, while Project Configuration owns override values and validation.
   Resolution: accepted.

5. What proves the change?
   Recommended answer: tests must show invalid overrides fail, valid overrides alter Guidance Engine metadata, and `config index` emits a compact install-safe artifact.
   Resolution: accepted.

## Required Implementation Shape

- Add validated override keys for workflow metadata, agent profile metadata, project conventions, and capability entries.
- Apply workflow metadata overrides before Guidance Engine builds `workflow_metadata` and facilitation pack output.
- Add `config index` to print and optionally write the generated Capability Index.
- Add facilitation and compact workflow guidance for `config-customization`.
- Add replay/test fixtures that route customization requests to `config-customization`.

## Boundaries

- Human Experience gets richer facilitation through `config-customization`.
- Agent Runtime gets compact JSON, validation errors, catalog metadata, and the generated Capability Index.
- Guidance Engine routes customization intent; it does not own config persistence.
- Project Configuration stores supported overrides only; unsupported keys remain errors.
