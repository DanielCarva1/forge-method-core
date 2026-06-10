# workflow: creative-session

trigger:
  - user asks for brainstorming, innovation, design thinking, storytelling, or creative direction
  - state.module == creative-studio

inputs:
  - creative question
  - target audience
  - constraints
  - desired output format
  - optional prior artifacts

steps:
  1. identify creative mode: brainstorm, design-thinking, innovation, storytelling, or presentation
  2. generate divergent options
  3. cluster and label options
  4. apply selection criteria
  5. produce a concrete artifact
  6. decide whether artifact feeds specification, remains reference, or becomes ephemeral
  7. update artifact index

outputs:
  - creative artifact
  - selected direction
  - rejected/parked options summary
  - next action

done_when:
  - selected direction is explicit
  - artifact is saved or summarized into state
  - next workflow is known

blocked_when:
  - user preference is required between materially different directions
  - target audience or output format is missing

