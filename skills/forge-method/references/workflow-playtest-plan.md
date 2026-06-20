# workflow: playtest-plan

trigger:
  - game prototype or vertical slice needs player feedback
  - mechanic or UX assumption must be tested

inputs:
  - prototype build
  - MDA Trace
  - target players
  - assumptions
  - feedback questions
  - playable slice target

steps:
  1. define test goals and player tasks
  2. choose observation and feedback format
  3. define pass/fail signals tied to target aesthetics, dynamics, mechanics, and feedback/UI signals
  4. define logistics and evidence capture
  5. save playtest plan

outputs:
  - playtest plan
  - MDA proof map
  - feedback questions
  - success signals
  - decision map

done_when:
  - test can be run by another agent or human
  - feel/fun claims are observable enough to accept, reject, or revise
  - signals map to design decisions
  - next evidence step is known

blocked_when:
  - playable artifact is unavailable
  - target player is unknown

handoff:
  - preserve playtest path, MDA proof map, playable slice, tasks, signals, decision map, and feedback plan
