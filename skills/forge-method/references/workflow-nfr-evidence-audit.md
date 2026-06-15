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
  1. list required security, performance, reliability, accessibility, maintainability, and compliance claims
  2. compare available evidence against each claim
  3. mark pass, gap, missing evidence, waiver, or blocked
  4. record waiver owner/rationale/revisit trigger and update release gate expectations

outputs:
  - NFR evidence matrix
  - gaps and waivers
  - release impact
  - gate updates

done_when:
  - every NFR claim has evidence status
  - gaps and waivers are explicit
  - release impact is known
  - release gate can consume the result

blocked_when:
  - NFR claims are absent or unbounded
  - required evidence cannot be produced or waived

handoff:
  - preserve audit path, claim statuses, gaps, waivers, release impact, and gate updates
