# workflow: game-retrospective

trigger:
  - game milestone or sprint completed
  - playtest, build, or story cycle produced learning

inputs:
  - completed stories
  - playtest or QA evidence
  - sprint status
  - open risks

steps:
  1. identify what changed for player experience, production, and quality
  2. separate useful learning from process noise
  3. decide keep/change/stop actions
  4. update backlog, risks, and next workflow

outputs:
  - retrospective notes
  - action items
  - backlog/risk updates

done_when:
  - learning is converted into action
  - next sprint/story adjustment is explicit
  - evidence is linked

blocked_when:
  - milestone evidence is missing
  - actions cannot be assigned to backlog or workflow changes

handoff:
  - preserve retrospective path, decisions, action items, and next planning/build action
