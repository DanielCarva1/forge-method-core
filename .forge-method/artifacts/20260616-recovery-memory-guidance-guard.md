# Recovery Memory Guidance Guard

Date: 2026-06-16
Workflow: agent-analyze
Phase: 6-evolve

## Audit Finding

Durable recovery memory is loaded early by future agents. Checkpoints, latest checkpoint mirrors, context packs, and recovery briefs can combine human-provided text with agent-facing route guidance.

Before this guard, `checkpoint` could write misleading guidance into recovery memory, and later `context pack` or `context recover` could redistribute that text into the exact files agents read first after a context reset.

## Change

The runtime now validates recovery memory guidance at three boundaries:

- `checkpoint` validates the final checkpoint Markdown before writing both the timestamped checkpoint and `context/latest-checkpoint.md`.
- `context pack` and `context recover` validate the final generated Markdown before writing context/recovery files.
- `audit` scans existing checkpoint files and context recovery files so legacy contamination fails `audit` and `gate`.

Checked files:

- `.forge-method/checkpoints/*.md`
- `.forge-method/context/latest-checkpoint.md`
- `.forge-method/context/current-pack.md`
- `.forge-method/context/recovery.md`
- `.forge-method/context/recovery-compact.md`

## Regression Proof

New tests:

- `test_checkpoint_rejects_misleading_recovery_memory_text_before_write`
- `test_audit_and_recover_reject_preexisting_misleading_checkpoint_memory`

Validation passed:

- `python -m unittest discover -s tests` - 112 tests
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
- source parity replay - 91/91
- workflow validate
- workflow compactness
- artifact verify
- audit
- gate --require-evals - 20/20

## Handoff

Continue the broader audit by checking artifact summaries, human input prompts, review findings, and story fields that are later copied into agent-facing context packs or runtime JSON.
