# Durable Runtime Guidance Source Guard

- status: implemented
- phase: 6-evolve
- workflow: agent-analyze
- scope: artifact index summaries, human input prompts, review finding text, and story work fields that are copied into runtime JSON or context packs

## Problem

The previous safety guards covered workflow refs, Guidance Engine payloads, Help Oracle output, state fields, config/capability data, checkpoints, context packs, and recovery briefs.

One gap remained: durable project records could still store misleading agent-facing guidance before the final context-pack guard saw it. That made `snapshot`, `resume`, `status --json`, and generated context packs depend on later validation instead of rejecting bad data at the write boundary.

## Contract

- artifact index `title` and `summary` must pass the guidance safety detector before index append
- human input `prompt` and `reason` must pass before write
- review finding `title`, `summary`, and `resolution` must pass before write
- story `title`, `acceptance_criteria`, and `blocker` must pass before write
- `audit` scans existing records so legacy contamination is detected even if it bypassed runtime writers
- artifact file creation through `artifact add` is blocked before the file is written when the artifact index payload is unsafe

## Implementation Notes

- Added shared record-field validation helpers around the existing guidance safety detector.
- Added write-boundary guards to `save_story`, `save_human_input`, `save_review_finding`, and `append_artifact_index`.
- Added pre-write validation in `write_artifact` so direct artifact creation stays atomic for unsafe summaries.
- Extended safe-context detection so descriptions like "fail validation if it instructs the agent to ..." are treated as blocking descriptions, not unsafe instructions.

## Validation

- `python -m unittest discover -s tests`: 115 tests passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`: passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`: passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py audit --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py artifact verify --root .`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`: passed
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay --json`: 91/91 passed
- `python skills\forge-method\scripts\forge_method_runtime.py gate --root . --require-evals`: 21/21 evals passed

## Next

Continue the post-parity Forge audit for dead code, stale docs, and remaining runtime surfaces that still rely on convention instead of deterministic validation.
