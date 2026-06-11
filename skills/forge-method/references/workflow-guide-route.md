# workflow: guide-route

trigger:
  - user asks what Forge Method should do next
  - state route is unclear to the human

inputs:
  - preflight result
  - current state or missing-state route
  - user question

steps:
  1. inspect state, route, track, next story, blockers, and readiness
  2. recommend one next action and optional alternatives
  3. suggest a track when no project state exists
  4. keep runtime commands as implementation details

outputs:
  - human guidance
  - optional track recommendation
  - next command for the agent

done_when:
  - next action is concrete
  - required human choice is explicit
  - no broad docs are loaded

blocked_when:
  - route requires a workspace or project choice
  - user asks for a conflicting direction

handoff:
  - preserve route, selected track, next command, and unresolved choices
