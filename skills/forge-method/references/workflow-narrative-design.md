# workflow: narrative-design

trigger:
  - game needs story, world, characters, or quests
  - narrative affects mechanics or player goals

inputs:
  - game brief or GDD
  - target feeling
  - mechanics constraints
  - content scope

steps:
  1. define premise, world, characters, and player role
  2. align narrative with core loop
  3. define quest/content units, tone, delivery format, and production constraints
  4. map narrative beats to mechanics, UI, and playable slice
  5. save narrative design artifact

outputs:
  - narrative artifact
  - content scope
  - mechanic links

done_when:
  - narrative supports player action
  - content scope is bounded
  - playable slice content is explicit
  - next story is known

blocked_when:
  - story contradicts mechanics
  - content scope is too large

handoff:
  - preserve narrative path, player role, content units, mechanic links, slice content, and next workflow
