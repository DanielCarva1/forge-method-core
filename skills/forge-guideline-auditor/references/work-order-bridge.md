# Work Order Bridge

Use this when turning a guideline into executable agent work.

## Required Fields

- `work_order_id`
- `source_guideline`
- `source_gap`
- `goal`
- `allowed_files`
- `forbidden_files`
- `acceptance_evidence`
- `checks`
- `rollback`
- `human_acceptance_question`
- `implementation_block`

## Rules

- One work order should close one bounded gap.
- Allowed and forbidden files must be explicit.
- Checks must be named before edits start.
- Evidence must be durable, not chat-only.
- Rollback must be possible or the work order must say why not.
- If the human cannot accept the result without reading code, the evidence is weak.

## Implementation Block Values

- `blocked_until_guideline_exists`
- `blocked_until_human_acceptance`
- `allowed_docs_only`
- `allowed_disposable_spike`
- `allowed_permanent_implementation`
