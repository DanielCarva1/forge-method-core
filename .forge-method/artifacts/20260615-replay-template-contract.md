# Replay Template Contract

created_at: 2026-06-15T13:27:00+00:00
workflow: runtime-builder
status: replay-template-contract

## Problem

Post-parity replay audit found one remaining human-facing case that routed correctly and asserted its facilitation pack, but did not assert the compact artifact template:

- `forge_experience_not_example_project`

The route was right, but the agent handoff contract was weaker than Forge needs. A future regression could keep the human route green while dropping the template that tells the next agent what artifact shape to preserve.

## Contract

For parity replay cases:

- human-facing guided cases must declare `expected_template` when the routed workflow has a catalog template;
- mechanical build cases stay exempt because their proof is the work order/autonomous loop contract;
- fixture coverage tests verify `expected_template` against `catalog/workflows.json`;
- `parity replay` fails if a human-facing guided payload returns a template but the fixture did not declare it.

## Changes

- Added `expected_template: correct-course-artifact` to `forge_experience_not_example_project`.
- Added runtime replay failure for missing template assertions in human-facing guided cases.
- Added unit coverage for fixture/catalog template consistency and the negative replay failure path.

## Validation Target

- targeted fixture/replay unit tests
- parity replay
- full unit suite
- smoke-runtime and smoke-install because the replay fixture and runtime script ship with the installed skill

## Next Action

Continue post-parity polish by checking persona lens and command/automation assertions. A route should only count as parity when the human guidance, compact artifact shape, and required automation handoff are all protected by replay or gate evidence.
