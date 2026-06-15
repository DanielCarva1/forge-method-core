# Post-command Help Oracle hardened

- created_at: 2026-06-15T03:29:08+00:00
- project: forge-method-core
- phase: 6-evolve
- status: post-command-help-oracle-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the post-command Help Oracle hardening increment. Progress-changing runtime commands now record compact next-workflow guidance in ledger.ndjson, and interactive mutations emit the next required workflow, recommended phase, alternatives, facilitation, and stale-state guard for the human/agent immediately after the mutation.

## Decisions

- Treat the bmad-help audit row as translated for the post-command next-step contract, while keeping full parity open for remaining human-experience depth rows.
- Keep path-output commands stdout-stable; they record guidance in the ledger instead of printing extra text.

## Checks

- python -m unittest discover -s tests: 71 tests OK
- parity replay: 58/58 passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- smoke-install.ps1: passed
- artifact verify, audit, config validate: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- CHANGELOG.md

## Artifacts

- .forge-method/evidence/20260615-032848-validation-post-command-help-oracle-hardening-validation.md

## Next Action

Continue real-use transcript hardening for remaining partial and strong-ish parity rows; prioritize human guidance depth where routing is correct but the conversation still feels thin.
