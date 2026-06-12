# workflow: workflow-validate

trigger:
  - workflow, module, catalog, facilitation pack, or eval behavior changed
  - runtime-builder work needs structural proof before story completion

inputs:
  - changed workflow references
  - workflow catalog metadata
  - module manifests
  - facilitation packs when referenced
  - relevant tests or evals

steps:
  1. validate every changed workflow file has required state-machine sections
  2. validate packaged module workflow ids resolve to a workflow reference or catalog alias
  3. validate catalog metadata has phase, required flag, and outputs
  4. validate referenced facilitation packs exist
  5. run focused tests or evals that prove the route

outputs:
  - workflow validation result
  - catalog consistency findings
  - missing test or eval notes

done_when:
  - workflow validation passes
  - module workflow ids resolve
  - catalog metadata and facilitation pack references are valid

blocked_when:
  - a module references an unknown packaged workflow
  - catalog metadata conflicts with the workflow reference
  - required proof is missing

handoff:
  - preserve changed workflow ids, validation commands, remaining findings, and next repair
