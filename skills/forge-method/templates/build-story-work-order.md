# Build Story Work Order

- kind: build-story-work-order
- created_at:
- story_id:
- story_title:
- decision_sources:
- mode: start/continue/review/evidence/headless

## Scope

- acceptance:
- non_goals:
- source_files:
- risk_notes:

## Domain Context

- track:
- playable_slice_or_user_flow:
- domain_checks:
- domain_evidence:

## Mechanical Loop

1. start_or_resume_story:
2. load_story_context:
3. implement_acceptance:
4. run_checks:
5. review:
6. repair_or_waive_findings:
7. write_evidence:
8. mark_story_done:
9. update_sprint_or_ready_gate:

## Commands

- story_start:
- context_plan:
- status:
- story_review:
- review_list:
- evidence_add:
- story_done:
- resume:

## Evidence

- checks_run:
- review_findings:
- fixes:
- evidence_path:
- manual_exceptions:

## Stop Only When

- missing_external_credential_or_access:
- destructive_action_requires_approval:
- unavailable_external_service:
- explicit_scope_change:

## Do Not Prompt

- ok_continue_between_mechanical_steps:
- after_story_start_if_acceptance_work_can_continue:
- because_chat_memory_is_missing_when_durable_files_are_enough:

## Handoff

- next_story_or_gate:
- state_update:
- sprint_update:
- commit_policy:
