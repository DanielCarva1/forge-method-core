# workflow: config-customization

trigger:
  - user asks to customize Forge behavior for a project or team
  - project conventions should affect future workflows

inputs:
  - desired convention
  - team or local scope
  - current config
  - validation rules

steps:
  1. choose team or local config scope
  2. write supported keys only
  3. validate merged config
  4. document impact in state or artifact

outputs:
  - config update
  - validation result
  - impact summary

done_when:
  - config validates
  - unsupported keys are absent
  - next workflow can read the convention

blocked_when:
  - requested setting has no supported key
  - team and local scopes conflict

handoff:
  - preserve config path, changed keys, validation result, and scope
