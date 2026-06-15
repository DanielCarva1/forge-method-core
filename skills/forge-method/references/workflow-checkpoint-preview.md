# workflow: checkpoint-preview

trigger:
  - user asks to preview checkpoint, summarize before handoff, or verify what memory will preserve
  - substantial work is about to end and checkpoint quality matters

inputs:
  - current state
  - touched files
  - decisions
  - checks and failed checks
  - artifacts and evidence
  - proposed next action

steps:
  1. draft the checkpoint title, summary, decisions, checks, touched files, artifacts, and next action
  2. compare draft against state and recent evidence
  3. remove chat-only claims and stale or unverified details
  4. identify missing proof before checkpoint write
  5. hand off to checkpoint or session-prep

outputs:
  - checkpoint preview
  - state delta
  - missing proof list
  - next action

done_when:
  - checkpoint content is compact, factual, and evidence-backed
  - next action matches current state
  - missing proof is resolved or recorded

blocked_when:
  - state or evidence contradicts the proposed checkpoint
  - required validation has not run
  - touched files or decisions are unknown

handoff:
  - preserve preview text, missing proof, state delta, and checkpoint command inputs
