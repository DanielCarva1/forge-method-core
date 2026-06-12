# workflow: doc-index

trigger:
  - user asks to index docs
  - large docs need navigation for future agents

inputs:
  - documentation root
  - audience or use case
  - existing context map
  - important artifacts

steps:
  1. discover docs and classify by purpose
  2. identify source-of-truth files and stale/duplicate areas
  3. create or update a compact index/map
  4. record navigation rules for future agents

outputs:
  - doc index or map
  - stale/duplicate notes
  - navigation guidance

done_when:
  - key docs are findable
  - source-of-truth boundaries are clear
  - future agents know what to read first

blocked_when:
  - documentation root is unavailable
  - ownership/source-of-truth cannot be determined

handoff:
  - preserve index path, source-of-truth rules, stale notes, and next doc action
