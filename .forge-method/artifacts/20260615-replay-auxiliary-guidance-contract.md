# Replay Auxiliary Guidance Contract

Date: 2026-06-15
Workflow: runtime-builder
Status: implemented

## Problem

The parity replay protected intent, phase, workflow, facilitation pack, template, persona lens, and commands, but it did not protect auxiliary guidance decisions.

That left a blind spot: a transcript could pass route parity while the runtime silently recommended council, Codex Goal handoff, or an autonomous mechanical work order.

The live trigger was this runtime-builder audit wording:

> check remaining automation council persona handoff assertions and replay contracts so route-only success does not count as parity

Before the fix, council-related words could pull the route toward lifecycle/council behavior even when the actual request was a runtime replay contract audit.

## Contract

- Runtime replay-contract and assertion wording is strong builder intent.
- Runtime-builder context plus replay-contract wording outranks domain words such as council and handoff.
- `council_recommended` is true only for explicit council routing or explicit council facilitation phrases.
- Parity replay fails when a fixture omits an expected `council_recommended` assertion while the runtime recommends council.
- Parity replay fails when a fixture omits an expected Codex Goal handoff assertion while the runtime recommends goal handoff.
- Parity replay fails when a fixture omits an expected mechanical work-order assertion while the runtime returns an autonomous work order.

## Runtime Changes

- Added replay-contract and auxiliary assertion phrases to builder signal detection.
- Added a narrow `council_recommended_for_guidance` helper.
- Added replay validation for:
  - `expected_council_recommended`
  - `expected_codex_goal_handoff_recommended`
  - `expected_mechanical_work_order_autonomous`
- Added actual replay output fields for council, Codex Goal handoff, and mechanical work-order autonomy.
- Added fixture coverage for the live meta-audit prompt.

## Validation

- Targeted replay assertion tests pass.
- Guidance for the live meta-audit prompt routes to `builder-flow / runtime-builder`.
- `parity replay` passes 90/90 cases.

## Next

Continue post-parity polish by looking for remaining route-only success surfaces where a compact agent handoff or automation flag changes behavior without an explicit replay expectation.
