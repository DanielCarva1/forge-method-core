# Replay Workflow First Question Mechanical Status Contract

## Problem

The previous human-prompt contract guaranteed that facilitated routes asked a first question, but many workflows still shared broad family-level questions.

Audit result before this patch:

- `unique_first_questions: 12` across 88 facilitated replay cases
- broad questions were reused across product, game, quality, builder, document, and lifecycle workflows
- both `mechanical-build` replay cases still used internal "I should" wording

That kept routing correct, but weakened the Forge promise that guided human experience should feel specific to the work while agent handoff stays compact.

## Contract

- Workflows with guided replay coverage get workflow-specific first questions from the Guidance Engine.
- Facilitated replay cases must include the exact workflow-specific first question.
- `mechanical-build` human prompts are status wording, not facilitation:
  - no `First question:`
  - no internal `I should`
  - includes `Build is ready:` and `write evidence`

## Runtime Change

- Added `WORKFLOW_FIRST_QUESTIONS` for the covered product, story, builder, creative, game, quality, document, lifecycle, research, and recovery workflows.
- Kept classification-level fallback only for uncovered workflows.
- Added `mechanical_human_prompt_for_guidance` so autonomous build routes speak as status/execution handoff.
- Hardened replay assertions for workflow-specific questions and mechanical-build status prompts.

## Proof

- Targeted contract tests passed.
- Parity replay passed: 90/90.
- Manual replay audit passed:
  - `unique_first_questions: 67`
  - `cross_workflow_repeats: []`
  - mechanical prompt issues: `[]`
- Full runtime tests passed: 93/93.
- `smoke-runtime`, `verify-fast`, `smoke-install`, `artifact verify`, and installed parity replay passed.

## Next Audit Thread

Continue post-parity Forge polish by auditing live CLI `guide` output shape against these richer prompt contracts and whether the human-visible non-JSON output should surface the same first question more prominently.
