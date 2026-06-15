# workflow: sprint-status

trigger:
  - user asks for sprint status, backlog status, next story, or implementation progress
  - build/verify work needs a human-readable status ritual

inputs:
  - `.forge-method/sprint.yaml`
  - story files
  - review findings
  - recent evidence and checkpoints
  - current state and next action

steps:
  1. summarize story counts, active story, and blocked/review items
  2. identify the next executable story or missing decision source
  3. name validation, review, and evidence still required
  4. recommend one next action and up to three alternatives
  5. save a compact status artifact when durable handoff is useful

outputs:
  - sprint status summary
  - blocked/next story map
  - required validation or evidence
  - next action

done_when:
  - the human can see what happened, what is blocked, and what happens next
  - the agent has a compact next story or route
  - stale state is called out instead of silently followed

blocked_when:
  - sprint state is missing or contradictory
  - story files cannot be read
  - active story lacks acceptance criteria or decision source

handoff:
  - preserve sprint counts, active/next story, blockers, evidence gaps, and next workflow
