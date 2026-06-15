# Discovery Closeout Human Guidance Contract

created_at: 2026-06-15
phase: 6-evolve
workflow: runtime-builder
status: discovery-closeout-human-guidance-improved

## Problem

The generated-project discovery route already required a durable discovery closeout before specification, but the human-facing `discover-intent` guidance still asked a generic outcome/constraint/proof question. That left agents with a better validator than conversation contract.

## Decision

`discover-intent` guidance now asks for the fields needed to generate `artifact discovery-closeout`:

- `audience`
- `outcome`
- `constraints`
- `non_goals`
- `success_signal`
- `open_questions`
- `grill_gate_handoff`
- `decision_log`
- `next_workflow`

The compact workflow reference remains a state machine. The richer field-gathering conversation lives in the facilitation pack and Guidance Engine human copy.

## Runtime Contract

- `first_guidance_question(..., "discover-intent")` returns a discovery-closeout-specific question.
- `guide --question --json` for generated-project initial facilitation keeps `recommended_workflow: discover-intent`.
- `human_experience.human_question` matches the closeout field-gathering question.
- `discover-intent.md` names `artifact discovery-closeout` and the critical closeout fields.

## Proof

- focused unit tests cover workflow-specific first questions, generated-project initial facilitation routing, and pack closeout fields.
- workflow catalog validation passes.
- full runtime, install, parity, unittest, verify-fast, audit, and gate evidence must be recorded before commit.
