# workflow: product-requirements

trigger:
  - state.phase == 2-specification
  - product intent exists

inputs:
  - discovery artifact
  - target users
  - constraints
  - success criteria
  - existing PRD/spec when updating or validating
  - evidence, UX notes, architecture constraints, and risk notes

steps:
  1. choose mode: create, update, validate, or addendum
  2. define or verify problem, users, goals, non-goals, constraints, and success metrics
  3. write or update functional requirements with acceptance evidence
  4. record decisions, rejected scope, contradictions, risks, and assumptions
  5. validate requirement testability and route unresolved UX, architecture, research, or quality gaps
  6. save product requirements artifact and decision/addendum log

outputs:
  - product requirements artifact
  - acceptance criteria
  - decision log or addendum
  - validation findings
  - open assumptions

done_when:
  - requirements are testable
  - non-goals and constraints are explicit
  - decisions and rejected scope are preserved
  - validation findings are resolved or routed
  - next architecture or planning workflow is known

blocked_when:
  - target user is unknown
  - success definition changes the product shape
  - evidence contradicts a core requirement without a decision

handoff:
  - preserve requirements path, mode, decisions, addendum, findings, assumptions, risks, and next workflow
