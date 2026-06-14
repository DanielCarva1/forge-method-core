# workflow: module-validate

trigger:
  - user wants to validate, audit, or check a Forge module or runtime extension
  - module-builder completed and needs structural plus quality proof

inputs:
  - module manifest
  - workflow catalog entries
  - workflow refs, packs, templates, scripts, and tests
  - expected install or smoke command

steps:
  1. run structural validation for workflow refs, catalog metadata, packs, templates, and module membership
  2. inspect each routed capability for trigger accuracy, output quality, and handoff completeness
  3. compare module behavior against replay fixtures or expected scenarios
  4. record pass, fail, waivers, and required fixes in a validation report
  5. hand off the next repair or release validation step

outputs:
  - module validation report
  - structural findings
  - quality findings
  - pass/fail or waiver decision

done_when:
  - validation commands passed or findings are actionable
  - each capability has route, pack when human-facing, compact workflow, template when needed, and proof
  - next workflow or release check is explicit

blocked_when:
  - module files cannot be resolved
  - validation command is unavailable
  - quality findings are too broad to assign ownership

handoff:
  - preserve validation report path, command output, unresolved findings, waivers, and next repair workflow
