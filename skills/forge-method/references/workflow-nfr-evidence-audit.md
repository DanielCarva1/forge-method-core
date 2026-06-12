# workflow: nfr-evidence-audit

trigger:
  - non-functional requirements need evidence
  - release/readiness depends on security, performance, reliability, accessibility, or compliance proof

inputs:
  - NFR list or requirements
  - risk register
  - evidence artifacts
  - release criteria

steps:
  1. list required NFR claims and proof type
  2. compare available evidence against each claim
  3. mark pass, gap, waiver, or blocked
  4. update release/readiness gate expectations

outputs:
  - NFR evidence audit
  - gaps and waivers
  - gate updates

done_when:
  - every NFR claim has evidence status
  - gaps and waivers are explicit
  - release gate can consume the result

blocked_when:
  - NFR claims are absent or unbounded
  - required evidence cannot be produced or waived

handoff:
  - preserve audit path, claim statuses, gaps, waivers, and gate updates
