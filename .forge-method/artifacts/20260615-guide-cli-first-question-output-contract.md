# Guide CLI First Question Output Contract

- kind: runtime-guidance-contract
- created_at: 2026-06-15T17:31:29Z
- phase: 6-evolve
- workflow: runtime-builder
- status: guide-cli-first-question-output-hardened

## Problem

Guidance Engine JSON already carried rich `human_prompt` contracts, including workflow-specific `First question:` text. The non-JSON `guide` output still printed that value as one `Prompt:` blob, which made the live human CLI less guided than the tested JSON contract.

## Contract

- Facilitated guidance prints the route explanation as `Guidance:`.
- Facilitated guidance prints the actionable opening as its own `First question:` line.
- `mechanical-build` guidance prints `Status:` with autonomous build/check/evidence wording.
- JSON payload shape remains unchanged.

## Proof

- Targeted tests cover runtime-builder facilitated text output and mechanical build text output.
- Parity replay still validates 90/90 JSON fixtures.
- Smoke/install validation confirms the installed skill still runs guidance replay.

## Next

Continue post-parity Forge polish by auditing installed reload/guide behavior in real project starts against the richer human prompt contract.
