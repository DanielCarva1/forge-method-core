# Replay Mutating Command Contract

Date: 2026-06-15
Workflow: runtime-builder
Status: implemented

## Problem

Parity replay already checked whether an expected command was present, but a transcript with multiple mutating commands could pass by asserting only one of them.

The concrete case was `method_frustration_ready`:

- actual mutating commands: `transition-evolve`, `correct-course`
- previous fixture assertion: only `correct-course`

That meant the correct-course behavior could pass without proving the important state transition back into evolution.

## Contract

- `expected_command` remains valid for a single command expectation.
- `expected_commands` is required when guidance returns multiple mutating commands.
- Replay fails if a fixture omits command expectations for mutating guide commands.
- Replay fails if the declared mutating command list is partial or out of sequence.
- Replay output now exposes `mutating_commands` so audit reports show the state-changing command surface directly.

## Runtime Changes

- Added `MUTATING_GUIDE_COMMANDS`.
- Added `mutating_command_names`.
- Added parity validation for `expected_commands`.
- Added exact mutating-command comparison when multiple mutating commands are returned.
- Updated `method_frustration_ready` to assert `["transition-evolve", "correct-course"]`.

## Validation

- Targeted negative tests pass for missing and partial multi-command assertions.
- Parity replay passes 90/90 cases.
- Post-patch audit found:
  - `missing_mutating_expectations`: none
  - `partial_multi_expectations`: none
  - only multi-command case: `method_frustration_ready`

## Next

Continue post-parity polish by checking other agent-action surfaces that may still be only indirectly asserted, especially state update contents, route reasons, and human prompt quality.
