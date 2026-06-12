# workflow: traceability-gate

trigger:
  - user asks for traceability or coverage from requirements to tests
  - release gate needs requirement/test/evidence mapping

inputs:
  - requirements or stories
  - tests and checks
  - evidence artifacts
  - risk register

steps:
  1. map requirements and risks to checks and evidence
  2. identify uncovered, weakly covered, and over-tested areas
  3. decide pass, conditional pass, fail, or waive
  4. record gate decision and follow-up work

outputs:
  - traceability matrix
  - coverage gaps
  - gate decision

done_when:
  - every high-risk requirement has coverage status
  - gate decision is explicit
  - follow-up work is recorded

blocked_when:
  - requirements or evidence are missing
  - coverage cannot be tied to a stable artifact

handoff:
  - preserve matrix path, gate decision, gaps, waivers, and follow-up stories
