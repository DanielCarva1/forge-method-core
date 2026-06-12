# workflow: workflow-analyze

trigger:
  - user asks to analyze a workflow
  - workflow behavior feels confusing, incomplete, or unsafe

inputs:
  - workflow reference
  - module manifest
  - expected scenarios
  - validation output

steps:
  1. verify state-machine sections and module/catalog metadata
  2. test trigger, inputs, steps, outputs, done_when, blocked_when, and handoff
  3. compare workflow behavior against expected human and agent scenarios
  4. recommend patch, split, merge, or deprecation

outputs:
  - workflow analysis
  - gap list
  - recommended change

done_when:
  - workflow gaps are concrete
  - next change is scoped
  - validation path is known

blocked_when:
  - workflow file cannot be resolved
  - expected scenario is unknown

handoff:
  - preserve workflow path, findings, recommended change, and validation command
