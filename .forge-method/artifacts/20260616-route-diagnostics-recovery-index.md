# Route Diagnostics Recovery Index

- kind: runtime-builder
- status: route-diagnostics-recovery-index
- phase: 6-evolve
- workflow: config-customization

## Problem

`next --json` preserved Help Oracle diagnostics, but persisted recovery memory and the generated capability index did not expose the same route contract.

That meant a future agent could recover from files and still miss the required workflow, route reason, context boundary, stale-state guard, and available diagnostic surfaces.

## Contract

- `context recover` writes a Route Diagnostics section.
- `context recover --compact` keeps Route Diagnostics before lower-priority sections and preserves Commands under budget.
- `config index --json` and `config index --write --json` include a `route_diagnostics` contract.
- The contract names the route surfaces: `guide --question --json`, `resume --json`, `next --json`, and `context recover`.
- Capability-index validation still rejects stale or unsafe written indexes.

## Human Experience

Humans can ask what was happening after an interruption and get a route that is backed by durable files, not by chat memory.

## Agent Contract

Agents recovering from `.forge-method/context/recovery.md`, `.forge-method/context/recovery-compact.md`, or `.forge-method/context/capability-index.json` can see the same routing explanation that `next --json` exposes.

## Proof

- Focused regressions passed for capability-index route diagnostics.
- Focused regressions passed for full recovery Route Diagnostics.
- Focused regressions passed for compact recovery budget preservation.

## Touched Files

- `skills/forge-method/scripts/forge_method_runtime.py`
- `tests/test_runtime.py`
- `CHANGELOG.md`

## Next

Run full runtime validation, write validation evidence, then checkpoint and commit.
