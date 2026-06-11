# workflow: security-plan

trigger:
  - project has authentication, authorization, secrets, data, or deployment risk
  - track == enterprise

inputs:
  - architecture artifact
  - data flows
  - threat assumptions
  - deployment constraints

steps:
  1. identify assets, trust boundaries, and threats
  2. define security requirements and checks
  3. record secrets and access rules
  4. save security plan

outputs:
  - security plan
  - required checks
  - unresolved risks

done_when:
  - security risks have checks or owners
  - secrets and access boundaries are explicit
  - release gate can inspect the plan

blocked_when:
  - sensitive data classification is unknown
  - security owner or constraint is missing

handoff:
  - preserve security path, risks, checks, and owners
