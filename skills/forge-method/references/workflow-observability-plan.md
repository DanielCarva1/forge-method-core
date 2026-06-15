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
  4. link signal evidence, gaps, support owners, and release impact
  5. save observability plan

outputs:
  - observability plan
  - signal checklist
  - support actions
  - release impact

done_when:
  - critical failures have observable signals
  - support path is defined
  - release gate can inspect monitoring evidence or gaps
  - ready gate can inspect the plan

blocked_when:
  - production environment is unknown
  - signals cannot be collected

handoff:
  - preserve signal map, alert needs, dashboards, support action, evidence links, gaps, and release impact
