# workflow: compliance-checklist

trigger:
  - project has policy, audit, contract, or regulatory obligations
  - enterprise track needs compliance proof

inputs:
  - requirements
  - security and privacy plans
  - known standards
  - release scope

steps:
  1. list applicable obligations
  2. map obligations to evidence or checks
  3. record gaps and owners
  4. save compliance checklist

outputs:
  - compliance checklist
  - evidence map
  - open gaps

done_when:
  - each obligation has evidence, waiver, or owner
  - gaps are explicit
  - release gate can inspect checklist

blocked_when:
  - applicable standard is unknown
  - legal or policy interpretation is required

handoff:
  - preserve checklist path, evidence map, gaps, and owners
