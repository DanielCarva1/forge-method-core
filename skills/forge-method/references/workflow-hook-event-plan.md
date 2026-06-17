# workflow: hook-event-plan

trigger:
  - user asks for hooks, event wrappers, lifecycle automation, or local runtime helpers
  - a workflow needs an opt-in event contract before adding scripts
  - repeated manual steps should become deterministic without slowing normal Forge startup

inputs:
  - event name and trigger source
  - desired automation outcome
  - allowed side effects and forbidden side effects
  - script path, timeout, payload, and rollback/cleanup policy
  - validation command and evidence path

steps:
  1. name the event and non-trigger cases
  2. classify the hook as advisory, validation, mutation, release, or recovery
  3. define payload fields, timeout, idempotency, logging, and failure behavior
  4. use `scripts/forge-hook-dispatch.ps1`/`.sh` as an explicit dispatcher only when the project opts in
  5. record validation proof, disabled-by-default policy, and next workflow

outputs:
  - hook/event contract
  - opt-in dispatcher command
  - side-effect boundary
  - validation and rollback notes

done_when:
  - hook is disabled by default and opt-in command is explicit
  - event, payload, timeout, and failure policy are compactly documented
  - automation cannot run silently during normal Codex startup
  - validation evidence or waiver is recorded

blocked_when:
  - hook would run hidden mutation without user/project opt-in
  - side effects cannot be bounded or rolled back
  - event source is ambiguous or overlaps an existing runtime command

handoff:
  - preserve event name, trigger, payload shape, dispatcher command, side-effect policy, evidence, and rollback notes
