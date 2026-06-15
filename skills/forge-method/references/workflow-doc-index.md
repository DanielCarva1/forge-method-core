# workflow: doc-index

trigger:
  - user asks to index docs
  - large docs need navigation for future agents

inputs:
  - documentation root
  - audience or use case
  - existing context map
  - important artifacts
  - source-of-truth and generated-doc rules

steps:
  1. discover docs and classify by purpose
  2. read docs before describing them; do not infer purpose from filenames
  3. identify source-of-truth files, generated docs, stale duplicates, and precedence rules
  4. record source fingerprint, source mtime, compact descriptions, and navigation rules
  5. run `artifact doc-check --path <document-utility-artifact>`

outputs:
  - doc index or map
  - stale/duplicate notes
  - navigation guidance
  - source fingerprint and stale-check proof

done_when:
  - key docs are findable
  - source-of-truth boundaries are clear
  - index descriptions are content-derived
  - stale-check proof passes or a waiver is explicit
  - future agents know what to read first

blocked_when:
  - documentation root is unavailable
  - ownership/source-of-truth cannot be determined
  - source doc is newer than the index artifact and no update was made

handoff:
  - preserve index path, source-of-truth rules, source fingerprint, stale notes, validation command, and next doc action
