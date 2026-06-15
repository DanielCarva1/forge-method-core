# Replay Persona Lens Contract

created_at: 2026-06-15T00:00:00+00:00
workflow: runtime-builder
status: replay-persona-lens-contract

## Problem

The parity replay already protected workflow route, facilitation pack, and compact template expectations, but Persona Lens output was only asserted in a few explicit persona cases.

That left a weak spot in the human experience contract: a route could stay green while the selected lens changed, disappeared, or was chosen by a brittle alias collision.

The audit found three concrete risks:

- `architect` matched as a substring inside `architecture`;
- "before architecture" and "antes de arquitetura" could select Architect even when architecture was only a future step;
- testing-oriented phrases such as "test framework" and "test architecture" could lose to generic architecture scoring.

## Contract

For parity replay cases:

- when guidance returns `persona_lens.id`, the fixture must declare `expected_persona_lens`;
- the replay command fails if a returned lens is not asserted;
- persona alias scoring must use whole-token or whole-phrase matches, not substring matches;
- "before architecture" context must not select Architect unless the human explicitly asks for an architect/lens role;
- problem-solving and QA language must beat generic architecture mentions when those are the actual human intent.

## Changes

- Added whole-phrase matching for persona IDs, titles, and aliases.
- Added an Architect guard for "before architecture" / "antes de arquitetura" context.
- Expanded QA aliases for test framework and fixture/test architecture language.
- Expanded problem-solving aliases for stuck/unblock/travado/destravar language.
- Added `expected_persona_lens` to every replay case where guidance returns a lens.
- Added a negative unit test proving `parity replay` fails when a returned lens is not asserted.

## Validation Target

- targeted persona/replay unit tests
- `parity replay`
- full unit suite
- `artifact verify`
- `smoke-runtime`
- `verify-fast`
- `smoke-install`
- `gate --require-evals`

## Next Action

Continue post-parity Forge polish by checking remaining automation, council, and persona handoff assertions. Route-only success should not count as parity unless human guidance, compact artifacts, and automation handoff are all protected by replay or gate evidence.
