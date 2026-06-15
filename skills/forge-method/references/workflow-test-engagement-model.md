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
  1. classify the engagement as advice, design, implementation, review, audit, or release gate
  2. identify lifecycle phase, risk level, source artifacts, constraints, and evidence needs
  3. choose the next test workflow and required template
  4. record mode, blocked conditions, and handoff criteria

outputs:
  - quality engagement model
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
  - preserve engagement mode, selected workflow, required evidence, blocked conditions, and open risks
