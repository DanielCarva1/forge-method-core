# workflow: observability-plan

trigger:
  - ready project needs monitoring, logging, metrics, or support signals
  - production operation depends on feedback loops

inputs:
  - architecture
  - user journeys
  - failure modes
  - operational goals

steps:
  1. identify critical signals
  2. define logs, metrics, traces, alerts, and dashboards
  3. map signals to support actions
  4. save observability plan

outputs:
  - observability plan
  - signal checklist
  - support actions

done_when:
  - critical failures have observable signals
  - support path is defined
  - ready gate can inspect the plan

blocked_when:
  - production environment is unknown
  - signals cannot be collected

handoff:
  - preserve signal map, alert needs, support action, and gaps
