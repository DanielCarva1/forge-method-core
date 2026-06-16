# Workflow Guidance Safety Guard

- created_at: 2026-06-16T01:59:00+00:00
- status: workflow-guidance-safety-guard
- workflow: runtime-builder
- lifecycle: durable

## Problem

Compact workflow refs already had shape and size validation, but a workflow could still be structurally valid while telling a future agent to do the wrong thing: rely on chat memory, follow stale state, ask procedural continue confirmations, or dump catalogs instead of routing.

That is an agentic experience defect. Forge needs compact docs, but compact docs must also be safe to execute.

## Decision

- Add a workflow guidance safety guard inside workflow validation.
- Reject positive instructions that tell agents to:
  - rely on chat/conversation memory or context;
  - follow stale state, stale guidance, stale next action, or stale workflow;
  - ask for procedural `ok/continue` confirmations;
  - dump or show the full workflow catalog as guidance.
- Preserve legitimate negative guardrails such as "never ask for procedural ok/continue" and recovery workflows that explicitly discard stale chat.
- Keep this in runtime validation, not prose review, so future workflow refs cannot regress silently.

## Result

- All 99 packaged workflow refs pass the new guard.
- `validate_workflow_file` now reports line-level `misleading agent guidance` errors.
- Focused unit coverage proves unsafe refs fail and safe negative guardrails pass.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_workflow_guidance_safety_rejects_stale_agent_instructions -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_packaged_modules_and_workflows_validate -v`
- `python -m unittest discover -s tests`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow compactness`
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`

## Next Gap

Continue P2 polish by auditing runtime help/oracle output for the same safety boundary: compact agent commands should remain authoritative without making human-facing guidance dry.
