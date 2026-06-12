# workflow: test-engagement-model

trigger:
  - user asks what kind of test architecture help is needed
  - quality request is broad or ambiguous

inputs:
  - product/spec context
  - risk level
  - lifecycle phase
  - available checks

steps:
  1. classify the engagement as advisory, design, implementation, review, audit, or gate
  2. identify constraints, required artifacts, and evidence needs
  3. choose the next test workflow
  4. record mode and handoff criteria

outputs:
  - test engagement decision
  - next quality workflow
  - evidence expectations

done_when:
  - quality mode is explicit
  - next workflow is selected
  - evidence expectations are known

blocked_when:
  - product risk cannot be described
  - no artifact or system is available to evaluate

handoff:
  - preserve engagement mode, selected workflow, evidence expectations, and open risks
