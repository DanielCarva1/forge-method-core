# workflow: editorial-review

trigger:
  - user asks for prose, structure, or clarity review
  - docs need human-readable quality without changing runtime behavior

inputs:
  - target document
  - intended audience
  - tone constraints
  - source-of-truth docs

steps:
  1. identify audience, promise, and structure
  2. flag ambiguity, unsupported claims, weak sequencing, and tone mismatch
  3. recommend edits or apply scoped edits when requested
  4. preserve technical meaning and source-of-truth boundaries

outputs:
  - editorial findings or patch
  - claim/source notes
  - follow-up doc actions

done_when:
  - clarity issues are actionable
  - technical meaning is preserved
  - follow-up edits are scoped

blocked_when:
  - intended audience is unknown
  - source-of-truth conflict cannot be resolved

handoff:
  - preserve target path, findings, edits applied, and unresolved source conflicts
