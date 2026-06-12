# workflow: game-story-creation

trigger:
  - game project needs implementation-ready stories
  - game epics exist but story context is not executable

inputs:
  - game brief or GDD
  - PRD or epics
  - architecture notes
  - validation plan

steps:
  1. choose the next playable slice or risk-reduction target
  2. create stories with player value, implementation notes, and checks
  3. attach context links, assets, and constraints
  4. order stories by dependency, risk, and proof value

outputs:
  - game story files
  - story order
  - validation checks

done_when:
  - each story is implementation-ready
  - checks and evidence are attached
  - active/next story is known

blocked_when:
  - target playable slice is unclear
  - checks cannot prove story success

handoff:
  - preserve story IDs, order, context links, checks, and next build-story action
