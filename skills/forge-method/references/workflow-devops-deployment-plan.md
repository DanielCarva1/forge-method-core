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
  4. link deployment evidence, owners, blockers, and release impact
  5. save DevOps plan

outputs:
  - deployment plan
  - environment checklist
  - release checks
  - release impact

done_when:
  - deployment path is explicit
  - rollback or recovery is defined
  - release checks are known
  - release gate can inspect evidence or blockers

blocked_when:
  - deployment target is unknown
  - secrets or environments are inaccessible

handoff:
  - preserve deployment path, checks, environments, rollback, evidence links, blockers, and release impact
