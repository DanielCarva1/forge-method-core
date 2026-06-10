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
  4. create compact agent-facing workflow docs
  5. add deterministic scripts only where reliability matters
  6. generate skill metadata
  7. run skill/plugin validation
  8. create smoke test or eval spec

outputs:
  - generated skill/module files
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
