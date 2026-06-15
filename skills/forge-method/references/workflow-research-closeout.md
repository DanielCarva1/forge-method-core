# workflow: research-closeout

trigger:
  - research or evidence gathering is complete enough to decide
  - user asks to close research or hand off findings

inputs:
  - research artifacts and sources
  - confidence and freshness notes
  - decision the research supports
  - unresolved uncertainty and risks

steps:
  1. summarize sources, confidence, and decision impact
  2. separate facts, assumptions, and unresolved uncertainty
  3. record rejected paths and what would change the decision
  4. write research closeout artifact
  5. route product-requirements, architecture, readiness-check, or another next workflow

outputs:
  - research closeout artifact
  - decision impact
  - next workflow

done_when:
  - sources and confidence are explicit
  - decision impact is clear
  - next workflow is named

blocked_when:
  - sources are missing or too weak for the decision
  - decision impact cannot be identified

handoff:
  - preserve sources, confidence, decision impact, uncertainty, rejected paths, and next workflow
