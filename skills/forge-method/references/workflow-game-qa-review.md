# workflow: game-qa-review

trigger:
  - game story is ready for review
  - prototype, mechanic, UX, or content needs validation

inputs:
  - story acceptance criteria
  - game artifacts
  - build or prototype evidence
  - playtest or QA notes

steps:
  1. verify acceptance criteria
  2. check playability, feedback, stability, and scope
  3. record findings
  4. approve, block, or request repair

outputs:
  - QA review result
  - findings
  - evidence references

done_when:
  - findings are recorded
  - approval or blocker is explicit
  - story can move safely

blocked_when:
  - build or evidence is missing
  - acceptance criteria cannot be tested

handoff:
  - preserve review result, findings, evidence, and repair path
