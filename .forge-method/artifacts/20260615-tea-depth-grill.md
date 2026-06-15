# P1.6 Test Architecture Enterprise Depth Grill

- kind: grill
- created_at: 2026-06-15
- scope: P1.6 Test Architecture Enterprise Depth

## Frame

P1.6 is not a new testing product surface. The `test-architect` module already has workflow ids, but its contracts are too shallow compared with the benchmark: broad quality requests can route correctly, yet the selected workflow often lacks enough mode, risk, fixture, evidence, waiver, and gate semantics for future agents.

## Questions Resolved

1. Should Forge create many public quality commands for each TEA-style workflow?
   Recommended answer: no. `$forge-method` remains the entrypoint. The Guidance Engine routes quality intent to compact workflows such as `test-engagement-model`, `test-framework`, `ci-quality-pipeline`, `atdd-plan`, `test-automation`, `test-review`, `nfr-evidence-audit`, and `traceability-gate`.

2. What should happen before writing quality artifacts?
   Recommended answer: choose a Quality Engagement Model first when intent is broad. The model decides whether the user needs advice, design, implementation, review, audit, or a release gate.

3. How should Forge translate fixture architecture without becoming a Playwright-specific tool?
   Recommended answer: define Fixture Architecture as a framework-neutral contract: pure helper, framework wrapper, composition surface, lifecycle cleanup, and command evidence. Framework examples belong in project artifacts, not in the public route surface.

4. What is the highest-risk semantic gap?
   Recommended answer: gate semantics. `traceability-gate` must distinguish design-time coverage mapping from release-time gate decision, and it must make pass, concerns, fail, missing evidence, and waiver explicit.

## Decisions

- Canonical family: Test Architecture Enterprise Depth.
- Canonical term: Quality Engagement Model.
- Canonical fixture term: Fixture Architecture.
- Canonical release term: Traceability Gate.
- Add richer TEA contracts by expanding existing workflows and templates first; add new workflow ids only if current ids cannot express a required route.
- Treat waiver as a first-class gate outcome only when the risk owner, missing evidence, rationale, expiry/revisit trigger, and release impact are recorded.

## Implementation Direction

- Expand `test-architecture.md` facilitation with engagement modes, fixture architecture, CI command contracts, ATDD/test design matrix, NFR evidence, two-phase traceability, gate decisions, and waivers.
- Expand compact workflow refs and templates without adding long persona prose.
- Add narrower templates only when a single generic `test-architecture-artifact` would make future agents reinterpret fields.
- Add Guidance Engine routes and replay fixtures for each quality mode.
- Keep route precedence so runtime-builder P1.6 requests remain builder work while ordinary quality requests route to test architecture workflows.
