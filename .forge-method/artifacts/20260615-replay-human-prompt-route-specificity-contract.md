# Replay Human Prompt Route Specificity Contract

## Problem

After route, pack, template, Persona Lens, state-update, and command assertions were protected, parity replay could still pass with human-facing guidance that read like an internal agent note.

The audit found the pattern across guided cases:

- facilitated `human_prompt` values often started as "I should..." statements
- guided flows did not consistently ask a concrete first question
- route reasons explained the broad decision but did not always preserve a compact signal/route summary for future agents

That contradicts the Forge split: rich guided experience for humans, compact state-machine handoff for agents.

## Contract

- Human-facing guided cases with a `facilitation_pack` must include `First question:` and a real question mark.
- Facilitated prompts must not start as internal agent notes or contain `I should` phrasing.
- Every replayed `route_reason` must include a compact `Signals:` and `Route:` summary.
- Replay actual output includes `human_prompt`, `route_reason`, and `state_updates` so regressions are debuggable without rerunning raw JSON manually.

## Runtime Change

- Added a final Guidance Engine prompt normalizer for facilitated workflows.
- Added workflow/classification-specific first questions for correct-course, problem solving, brainstorm, research, builder, game, creative, quality, document, story, product, lifecycle, and fallback routes.
- Added route-reason enrichment with detected signals and final route.
- Hardened parity replay assertions for human-facing prompt quality and route specificity.

## Proof

- Targeted prompt/reason replay tests passed.
- Parity replay passed: 90/90.
- Manual replay audit passed:
  - `cases: 90`
  - `facilitated: 88`
  - `missing_first_question: 0`
  - `internal_i_should: 0`
  - `missing_signals_route: 0`
- Full runtime tests passed: 91/91.
- `smoke-runtime`, `verify-fast`, `smoke-install`, `artifact verify`, and installed parity replay passed.

## Next Audit Thread

Continue post-parity Forge polish by auditing whether guided prompt questions are sufficiently workflow-specific and whether mechanical-build prompts should use separate human/status wording instead of facilitation wording.
