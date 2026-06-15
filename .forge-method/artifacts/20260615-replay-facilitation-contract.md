# Replay Facilitation Contract

created_at: 2026-06-15T13:12:00+00:00
workflow: runtime-builder
status: replay-facilitation-contract

## Problem

Post-parity polish found three human-facing replay cases that routed to workflows with rich facilitation packs, but the fixture did not assert the expected pack:

- `help_next_step_orientation`
- `confused_user`
- `method_frustration_ready`

That made the route correct but the human-experience contract weaker than the Forge promise: the replay could pass even if future guidance stopped returning the rich pack.

## Contract

For parity replay cases:

- human-facing guided cases must declare `expected_facilitation_pack` when the routed workflow has a catalog facilitation pack;
- mechanical build cases are exempt because their core contract is autonomous execution, not a conversational facilitation pack;
- fixture coverage tests verify the declaration against `catalog/workflows.json`;
- `parity replay` itself fails if a human-facing guided case returns a pack but the fixture forgot to declare it.

## Changes

- Added missing pack/template assertions to the three weak replay cases.
- Added a runtime replay failure for missing pack assertions in human-facing guided cases.
- Added unit coverage for fixture/catalog consistency and the negative replay failure path.

## Validation Target

- targeted fixture/replay unit tests
- parity replay
- full unit suite
- runtime/install smoke because the fixture and runtime script ship inside the skill package

## Next Action

Keep strengthening replay contracts around the human/agent split: when a transcript proves a rich human behavior matters, assert the pack/template/persona output instead of relying on broad route success.
