# workflow: playtest-plan

trigger:
  - game prototype or vertical slice needs player feedback
  - mechanic or UX assumption must be tested

inputs:
  - prototype build
  - target players
  - assumptions
  - feedback questions

steps:
  1. define test goals and player tasks
  2. choose observation and feedback format
  3. define pass/fail signals
  4. save playtest plan

outputs:
  - playtest plan
  - feedback questions
  - success signals

done_when:
  - test can be run by another agent or human
  - signals map to design decisions
  - next evidence step is known

blocked_when:
  - playable artifact is unavailable
  - target player is unknown

handoff:
  - preserve playtest path, tasks, signals, and feedback plan
