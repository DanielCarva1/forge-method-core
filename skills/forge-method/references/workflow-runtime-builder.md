# workflow: runtime-builder

trigger:
  - user asks to create a new method module, skill, workflow, or agent
  - state.module == runtime-builder

inputs:
  - module intent
  - target users
  - workflow boundaries
  - required tools
  - validation expectations

steps:
  1. classify artifact: skill, workflow, module, script, plugin, or eval
  2. define trigger and non-trigger cases
  3. define state-machine contract
  4. route creation work through module-ideation, agent-builder, workflow-builder, module-builder, or module-validate when narrow enough
  5. create compact agent-facing workflow docs
  6. add deterministic scripts only where reliability matters
  7. generate catalog/module metadata
  8. run workflow, module, skill, plugin, or install validation
  9. create smoke test or eval spec

outputs:
  - generated skill/module files
  - builder plan or validation report
  - validation result
  - smoke/eval plan
  - distribution note

done_when:
  - generated artifact validates
  - trigger behavior is documented
  - at least one smoke/eval path exists

blocked_when:
  - module purpose is too broad
  - trigger overlaps another module without routing rule
  - required external connector is unavailable

handoff:
  - preserve generated artifact paths, validation results, smoke/eval plan, and distribution note
