# Runtime Guidance Payload Safety Guard

- created_at: 2026-06-16T02:39:00+00:00
- status: runtime-guidance-payload-safety-guard
- workflow: agent-analyze
- lifecycle: durable

## Problem

The previous guard protected Help Oracle output, but Guidance Engine payloads, preflight JSON, reload JSON, and parity replay cases were still outside the same stale-route safety contract.

That meant a guided transcript could pass route, prompt, pack, template, and command assertions while still carrying an unsafe agent-facing instruction such as trusting chat memory before durable files.

## Decision

- Generalize the Help Oracle safety validator into a runtime guidance payload validator.
- Run the validator from `parity_case_failures`, so every replayed guided transcript must pass the stale-route safety contract.
- Add direct proof that `preflight --json`, `reload --json`, and `guide` payloads pass the same contract.
- Exclude raw human question context and executable command strings from recursive safety scanning to avoid treating user text as runtime instruction.
- Rewrite context-recovery copy from "before trusting prior chat context" to "without trusting prior chat context", and change its first question to ask which prior chat assumption should be discarded.

## Result

- Parity replay now protects both human-facing guided output and agent-facing JSON handoff from stale-route instructions.
- Context recovery guidance remains rich for the human but no longer implies that prior chat context can become the trusted source.
- Runtime guidance safety now covers Help Oracle, Guidance Engine replay payloads, preflight JSON, and reload JSON.

## Validation

- focused runtime guidance safety tests passed
- `python -m unittest discover -s tests`
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow compactness`
- `python skills\forge-method\scripts\forge_method_runtime.py audit --root .`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`

## Next Gap

Continue the broader Forge audit for dead code, stale artifacts, misleading agent docs, and runtime surfaces that still depend on convention instead of deterministic validation.
