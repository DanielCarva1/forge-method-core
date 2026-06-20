# workflow: platform-ops-plan

trigger:
  - product depends on infrastructure, CI/CD, database, deployment, environments, secrets, observability, or rollback
  - platform assumptions would affect architecture, stories, release, or operations

inputs:
  - product requirements or discovery notes
  - architecture constraints
  - data and integration needs
  - deployment target and runtime constraints
  - validation and operate expectations

steps:
  1. map platform surfaces: app runtime, database/data, storage, integrations, CI/CD, environments, secrets, deploy, observability, rollback, support
  2. classify each surface as required now, deferred, unknown, or intentionally out
  3. name the riskiest operational assumptions and the proof each one needs
  4. choose next specialized workflow: devops-deployment-plan, ci-quality-pipeline, privacy-data-plan, security-plan, observability-plan, architecture, or readiness-check
  5. record owners, commands, evidence, and waivers

outputs:
  - platform surface map
  - database/data operations plan
  - environment and secrets policy
  - proof and owner map
  - next ops workflow

done_when:
  - no platform assumption needed for build is implicit
  - database, CI/CD, deploy, secrets, observability, and rollback are addressed, deferred, or waived
  - next specialized workflow is known

blocked_when:
  - deployment target, data boundary, or required access is unknown
  - operational risk changes product or architecture decisions

handoff:
  - preserve platform surfaces, decisions, deferred items, proof commands, owners, waivers, and next workflow
