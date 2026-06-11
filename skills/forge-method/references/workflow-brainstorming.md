# workflow: brainstorming

trigger:
  - user asks for many ideas or directions
  - state.module == creative-studio

inputs:
  - creative question
  - constraints
  - audience
  - selection criteria

steps:
  1. generate diverse options
  2. cluster related ideas
  3. mark risky, safe, strange, and promising options
  4. select candidates for deeper work

outputs:
  - idea set
  - clusters
  - candidate directions

done_when:
  - options are meaningfully different
  - selection criteria are applied
  - next creative workflow is known

blocked_when:
  - prompt lacks audience or constraint
  - options cannot be compared

handoff:
  - preserve selected ideas, rejected patterns, criteria, and next workflow
