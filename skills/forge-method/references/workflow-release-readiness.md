# workflow: release-readiness

trigger:
  - batch is ready to publish or hand to users
  - release gate needs final evidence

inputs:
  - done stories
  - quality gate output
  - release notes
  - operational plans

steps:
  1. verify stories, evidence, artifacts, risks, and checks
  2. confirm version and changelog intent
  3. run final release validation once
  4. write release readiness artifact

outputs:
  - release readiness artifact
  - final validation result
  - publish or hold decision

done_when:
  - release check is final for the batch
  - publish decision is explicit
  - ready or hold state is written

blocked_when:
  - git or version metadata is inconsistent
  - required validation fails

handoff:
  - preserve readiness path, validation output, publish decision, and blockers
