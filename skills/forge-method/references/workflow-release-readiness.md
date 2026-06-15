# workflow: release-readiness

trigger:
  - batch is ready to publish or hand to users
  - release gate needs final evidence

inputs:
  - done stories
  - quality gate output
  - release notes
  - track decision and readiness matrix
  - operational plans

steps:
  1. verify stories, evidence, artifacts, risks, and checks
  2. for enterprise, verify required artifacts, NFR evidence, traceability gate, waivers, and audit trail
  3. confirm version and changelog intent
  4. run final release validation once
  5. write release readiness artifact and run `artifact enterprise-check` when selected_track is enterprise

outputs:
  - release readiness artifact
  - final validation result
  - publish or hold decision
  - enterprise evidence gate

done_when:
  - release check is final for the batch
  - required enterprise evidence is pass, concerns, fail, or waived
  - publish decision is explicit
  - ready or hold state is written

blocked_when:
  - git or version metadata is inconsistent
  - required validation fails
  - enterprise evidence or waiver is missing

handoff:
  - preserve readiness path, validation output, enterprise evidence status, waivers, publish decision, and blockers
