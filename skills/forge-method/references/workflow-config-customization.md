# workflow: config-customization

trigger:
  - user asks to customize Forge behavior for a project or team
  - project conventions should affect future workflows
  - user asks for overrides, capability index, config inspect, or config validation

inputs:
  - desired convention
  - team or local scope
  - current config
  - validation rules

steps:
  1. choose team or local config scope
  2. map the request to supported override keys or custom capability entries
  3. reject unsupported behavior before writing config
  4. write supported keys only
  5. run config validate
  6. run config index when agents need a compact effective registry
  7. document runtime-visible impact in state, artifact, or checkpoint

outputs:
  - config update
  - validation result
  - optional capability index
  - impact summary

done_when:
  - config validates
  - unsupported keys are absent
  - valid overrides affect inspect, guide metadata, or capability index predictably
  - next workflow can read convention or index

blocked_when:
  - requested setting has no supported key
  - team and local scopes conflict
  - override points to missing workflow, pack, template, agent, or capability reference

handoff:
  - preserve config path, changed keys, scope, validation result, index path, and affected workflow or agent
