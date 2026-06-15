# workflow: editorial-review

trigger:
  - user asks for prose, structure, or clarity review
  - docs need human-readable quality without changing runtime behavior

inputs:
  - target document
  - intended audience
  - tone constraints
  - source-of-truth docs
  - requested mode: prose, structure, tone, edit, or validate

steps:
  1. identify reader job, promise, tone, and structure
  2. separate source facts from prose choices
  3. flag ambiguity, unsupported claims, weak sequencing, and tone mismatch
  4. recommend edits or apply scoped edits when requested
  5. preserve technical meaning and source-of-truth boundaries

outputs:
  - editorial findings or patch
  - reader-job notes
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
  - preserve target path, audience, findings, edits applied, unsupported claims, and unresolved source conflicts
