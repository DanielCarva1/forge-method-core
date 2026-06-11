# workflow: devops-deployment-plan

trigger:
  - project needs deployment, CI, environment, or operational setup
  - release readiness depends on infrastructure

inputs:
  - architecture artifact
  - runtime requirements
  - environments
  - deployment target

steps:
  1. define environments and deployment path
  2. identify build, test, secret, and rollback needs
  3. define CI or release checks
  4. save DevOps plan

outputs:
  - deployment plan
  - environment checklist
  - release checks

done_when:
  - deployment path is explicit
  - rollback or recovery is defined
  - release checks are known

blocked_when:
  - deployment target is unknown
  - secrets or environments are inaccessible

handoff:
  - preserve deployment path, checks, environments, and blockers
