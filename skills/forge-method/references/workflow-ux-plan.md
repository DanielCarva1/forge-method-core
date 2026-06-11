# workflow: ux-plan

trigger:
  - user-facing experience is part of the product
  - requirements mention interface, onboarding, content, or human workflow

inputs:
  - product requirements
  - target user
  - brand or tone constraints
  - workflow goals

steps:
  1. map primary user tasks
  2. define screens, states, copy tone, and interaction rules
  3. identify empty, error, loading, and success states
  4. save UX plan artifact

outputs:
  - UX plan artifact
  - interaction states
  - copy and tone constraints

done_when:
  - primary workflow is testable
  - key states are listed
  - implementation stories can reference UX decisions

blocked_when:
  - audience or product posture is unknown
  - visual or content direction conflicts with requirements

handoff:
  - preserve UX plan path, states, copy constraints, and risks
