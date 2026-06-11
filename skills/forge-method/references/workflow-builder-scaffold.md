# workflow: builder-scaffold

trigger:
  - user asks to create a workflow, module, agent, skill, template, or eval
  - state.module == runtime-builder

inputs:
  - artifact kind
  - id and title
  - purpose
  - validation need

steps:
  1. classify generated artifact
  2. scaffold the smallest valid file set
  3. add eval or validation hook
  4. run builder validation
  5. record generated paths

outputs:
  - generated extension
  - validation result
  - ledger entry

done_when:
  - generated artifact validates
  - required state-machine sections exist when relevant
  - next action is known

blocked_when:
  - artifact id conflicts
  - validation cannot be defined

handoff:
  - preserve generated paths, validation command, and next action
