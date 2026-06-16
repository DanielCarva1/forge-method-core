# Next Help Oracle Surface

- kind: runtime-builder
- status: next-help-oracle-surface
- phase: 6-evolve
- workflow: runtime-builder

## Problem

`resume --json` recommends `next` as a compact continuation command, but `next` only printed the next human step and workflow. It did not expose the Help Oracle reason, context boundary, commands, state-update hint, stale-state guard, or quality summary in a machine-readable form.

That created an agentic gap: a future agent could follow the recommended command and lose the route diagnostics that explain why the old `next_action` is safe, stale, overridden, blocked, or mechanical.

## Contract

- `next --json` returns a compact Help Oracle surface.
- The payload includes action, autonomy, human next step, required workflow, recommended phase, reason, quality, commands, context boundary, state update requirement, stale-state guard, alternatives, mechanical work order, and Codex goal handoff.
- Text `next` still starts with the human next step and required workflow.
- Text `next` also prints the route reason and context boundary.
- If quality fails, text `next` prints compact surface-prefixed quality errors.
- Existing anti-prompt behavior remains: mechanical work still recommends `/goal` without asking for procedural confirmation.

## Human Experience

Humans still get a terse next move, but now it explains why that route is correct when stale state or ready/evolve ambiguity would otherwise look suspicious.

## Agent Contract

Agents can use `next --json` as the compact follow-up to `resume --json` without falling back to chat memory or parsing human text. It preserves the same Help Oracle reasoning available in snapshot/resume while staying smaller than a full snapshot.

## Proof

- Human input blocking exposes `answer_required_input`, `discover-intent`, reason, context boundary, and input commands in `next --json`.
- Ready-state stale `next_action` exposes Guidance Engine routing reason and omits the stale publish action.
- Active evolve workflow exposes runtime-builder route reason and context boundary.
- Broken workflow quality is visible in text `next` and `next --json`.
- Mechanical story work exposes goal handoff and autonomous work order through `next --json`.

## Touched Files

- `skills/forge-method/scripts/forge_method_runtime.py`
- `tests/test_runtime.py`
- `CHANGELOG.md`

## Next

Continue the post-parity Forge audit by checking whether `guide` and Help Oracle route diagnostics are consistently mirrored in persisted recovery artifacts and capability indexes.
