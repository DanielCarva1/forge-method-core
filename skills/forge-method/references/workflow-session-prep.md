# workflow: session-prep

trigger:
  - user asks to prep the next session
  - context is getting long and the next agent needs a compact start
  - work is about to pause or change hands

inputs:
  - state and sprint
  - latest checkpoint
  - context load plan
  - open inputs, review findings, stories, evidence, and artifacts

steps:
  1. read current state and latest checkpoint
  2. list open blockers, active story, review findings, and recent evidence
  3. choose the minimal files to load next
  4. write session prep artifact
  5. name the first command or workflow for the next session

outputs:
  - session prep artifact
  - compact read order
  - first command or workflow

done_when:
  - a future agent can start from files, not chat memory
  - read order and next command are explicit
  - unresolved blockers are named

blocked_when:
  - state is missing or contradictory
  - open blockers cannot be classified from durable files

handoff:
  - preserve session prep path, read order, next command, blockers, and next workflow
