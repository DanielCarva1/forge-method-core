# Facilitation Specificity Guard

- created_at: 2026-06-16T01:31:00+00:00
- status: facilitation-specificity-guard
- workflow: runtime-builder
- lifecycle: durable

## Problem

The post-parity polish plan called out a real Forge-specific risk: facilitation packs could satisfy structural sections and compactness limits while still being too generic to guide a human well. That would preserve agent-facing shape but weaken the human experience promise.

## Decision

- Make `domain_examples:` a required facilitation section.
- Require at least three entries under `domain_examples:` for every facilitation pack.
- Count both list-style entries and compact YAML-style mapping entries, because existing packs use both readable formats.
- Keep the rule in workflow validation and compactness checks so human guidance quality is machine-checked without adding verbosity to compact workflow refs.
- Add short situational examples to the remaining packs that lacked the canonical section.

## Result

- All 29 packaged facilitation packs now expose at least three domain examples.
- `workflow validate` fails if a future human-facing pack omits `domain_examples:`.
- `workflow compactness --json` reports `facilitation_limits.min_domain_examples`.
- Focused unit coverage proves generic packs are rejected even when they contain the normal required sections.

## Validation

Focused validation passed:

- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow compactness`
- `python -m unittest tests.test_runtime.RuntimeTests.test_facilitation_specificity_guard_rejects_generic_packs tests.test_runtime.RuntimeTests.test_packaged_modules_and_workflows_validate -v`

Full validation will be recorded in the evidence artifact for this increment.

## Next Gap

Continue P2 polish by auditing compact workflow refs for misleading agent guidance and stale next-step language, using validation or replay proof before changing prose.
