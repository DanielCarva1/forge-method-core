# workflow: project-context

trigger:
  - user asks to document this project
  - project needs a compact source-of-truth context artifact
  - brownfield code or docs must be summarized for future agents

inputs:
  - state and sprint
  - current context pack
  - source docs and architecture notes
  - conventions, commands, and important paths

steps:
  1. identify source-of-truth artifacts and stale or missing docs
  2. summarize purpose, users, architecture shape, conventions, commands, and validation expectations
  3. map important files and artifacts to future-agent responsibilities
  4. write project context artifact
  5. route session-prep, readiness-check, or the next domain workflow

outputs:
  - project context artifact
  - source map
  - next workflow

done_when:
  - future agent can understand the project without chat replay
  - source-of-truth and stale docs are named
  - next workflow is explicit

blocked_when:
  - source files or docs needed for context are unavailable
  - project ownership or purpose is contradictory

handoff:
  - preserve context artifact path, source map, load hints, validation commands, and next workflow
