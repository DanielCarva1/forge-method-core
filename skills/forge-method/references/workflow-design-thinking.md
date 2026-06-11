# workflow: design-thinking

trigger:
  - user asks to understand a user problem or experience
  - creative or product direction is uncertain

inputs:
  - target user
  - problem context
  - current assumptions
  - constraints

steps:
  1. identify user needs and friction
  2. frame the problem
  3. ideate solution directions
  4. choose prototype or artifact path

outputs:
  - problem frame
  - user needs
  - selected direction

done_when:
  - problem statement is concrete
  - user need is explicit
  - next artifact is known

blocked_when:
  - target user is unknown
  - problem frame conflicts with constraints

handoff:
  - preserve user need, problem frame, selected direction, and next artifact
