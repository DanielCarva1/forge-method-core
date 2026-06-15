# workflow: working-backwards-challenge

trigger:
  - user asks for PRFAQ, working-backwards, press-release/FAQ, or customer-promise challenge
  - product requirements need a sharper customer promise before architecture or stories

inputs:
  - product intent or PRD
  - target user and painful moment
  - promised outcome
  - constraints, non-goals, evidence, and risks

steps:
  1. write the customer-facing promise in plain language
  2. list FAQ objections, trust gaps, adoption blockers, and support questions
  3. compare promise against evidence, UX, architecture, legal, and operational constraints
  4. decide whether to change product requirements, gather evidence, or reject the promise
  5. save the challenge artifact and route the next workflow

outputs:
  - PRFAQ challenge
  - customer promise stress test
  - decision impact
  - next workflow

done_when:
  - the promise is testable and believable
  - major objections have decisions, evidence requests, or rejected scope
  - product-requirements, UX, architecture, or research next step is explicit

blocked_when:
  - target user or promised outcome is unknown
  - the promise depends on evidence that is unavailable
  - a legal, safety, or feasibility contradiction cannot be resolved

handoff:
  - preserve promise, FAQ objections, evidence gaps, decision impact, rejected claims, and next workflow
