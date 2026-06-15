# P2 Scope Decisions And Forge Polish Plan

- kind: scope-decision
- created_at: 2026-06-15
- scope: P2 parity decisions and Forge human/agent polish

## Decisions

| Item | Decision | Rationale | Revisit trigger |
|---|---|---|---|
| Persistent personal memory agents | non-goal | Forge owns project-local durable state, artifacts, checkpoints, context packs, and ledgers. Broad personal companion memory would change the product boundary and privacy model. | A separate Forge personal workspace product is explicitly approved. |
| Presentation/deck craft | folded/deferred | Forge routes pitch and deck narrative to `storytelling` with a `presentation-craft` lens, but visual deck production is not required for the current Codex-native method runtime. | Users repeatedly request launch/pitch/deck production as part of Forge projects. |
| Isolated Docker eval runner | deferred | Local evals, parity replay, smokes, install smoke, and CI cover the current plugin. Docker isolation adds setup cost before untrusted execution or reproducibility requires it. | Forge needs to run untrusted project code or reproducible cross-machine eval suites. |
| Hook wrappers | deferred | Hook experiments were useful, but adding hook surfaces now increases Codex plugin overhead. Keep event concepts for a future native app/runtime. | A standalone Forge app or concrete plugin lifecycle need requires deterministic hooks. |
| API/browser utility layer | deferred as public surface | TEA depth now records provider-specific API/browser utilities inside `test-framework`, CI, automation, and review artifacts. Generic utility commands would duplicate Codex/browser/plugin capabilities. | Multiple projects need the same provider-specific utility workflow and fixtures. |

## Polish Direction

The translated parity surface is no longer missing the big benchmark families, but the next quality bar is Forge-specific:

- Human Experience should feel guided, specific, adaptive, and warm/direct, not a catalog dump.
- Agent Runtime should stay compact: workflow refs remain state machines; rich language belongs in facilitation packs, guide output, patch notes, and human-facing docs.
- Each polish change needs a transcript fixture, compact contract, or validation gate so it cannot regress into taste-only copy.

## Next Work

1. Audit facilitation packs for thin or generic human guidance.
2. Audit compact workflow refs for bloat, missing handoff, or misleading agent instructions.
3. Add post-parity patch notes and status summary.
4. Validate with replay, workflow checks, unit tests, and install smoke before release planning.
