---
name: forge-guideline-auditor
description: Audit Forge and agent-native product gaps and turn them into reusable guidelines, work-order candidates, and acceptance evidence. Use before broad implementation, permanent architecture, Rust crate creation, agent documentation, permission/sandbox changes, release/versioning changes, method-gap work, guideline writing, work-order drafting, or any Forge Standalone task where agents may implement durable product behavior.
---

# Forge Guideline Auditor

Use this skill to prevent agents from jumping from ambition directly to code.

## References

Read only what the task needs:

- `references/gap-to-guideline-method.md` when auditing a product, method, or agent-native gap.
- `references/guideline-authoring-standard.md` when writing or reviewing a guideline.
- `references/work-order-bridge.md` when turning a guideline into a work order.

Use assets as templates:

- `assets/guideline-template.md`
- `assets/work-order-template.md`

Use `scripts/validate_guideline.py` after creating or editing a guideline or work order.

## Non-Negotiables

- Do not start permanent implementation without a governing guideline or explicit gap record.
- Every guideline must name human promise, agent rule, machine contract, forbidden behavior, checks, and acceptance evidence.
- Every work order must name source guideline/gap, allowed files, forbidden files, checks, evidence, rollback, and human acceptance question.
- Do not mutate `.forge-method/state.yaml` unless the current workflow/input is actually resolved.
- Treat Forge Method Core as reference/contract for Forge Standalone, not implementation source.
- Preserve durable evidence/checkpoints for meaningful product or method changes.

## State Machine

1. Load state:
   - `AGENTS.md`
   - `.forge-method/state.yaml`
   - `.forge-method/sprint.yaml`
   - `.forge-method/context/latest-checkpoint.md`
   - relevant docs only
2. Classify mode:
   - `audit-gap`
   - `write-guideline`
   - `validate-guideline`
   - `create-work-order`
3. Read scoped references.
4. Produce the durable artifact.
5. Validate required sections and evidence.
6. Report next action and whether implementation remains blocked.

## Required Outputs

For an audit:

- gap summary
- needed guideline
- risk controlled
- acceptance evidence
- work-order candidate

For a guideline:

- guideline document following `assets/guideline-template.md`
- validation result
- evidence/checkpoint when in a Forge project

For a work order:

- work order following `assets/work-order-template.md`
- explicit implementation block/allow decision
- validation result

## Write Rules

- Write guidelines under the owning project docs, usually `docs/`.
- Write Forge Method workflow additions under `skills/forge-method/references/`, `facilitation/`, `templates/`, and `catalog/`.
- Write evidence under `.forge-method/evidence/`.
- Write checkpoints under `.forge-method/checkpoints/` and update latest checkpoint when the project requires it.
- Keep skill instructions compact; move reusable detail into references.

## Validation

Run:

```powershell
python "<skill-dir>/scripts/validate_guideline.py" <path-to-guideline-or-work-order>
```

Passing validation does not mean the idea is good. It only means the artifact has the minimum structure future agents need.
