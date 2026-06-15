# workflow: storytelling

trigger:
  - user asks for narrative, pitch, brand story, game story, or presentation arc
  - artifact needs emotional coherence

inputs:
  - audience
  - desired feeling
  - facts or constraints
  - medium
  - optional pitch or deck context

steps:
  1. define premise and audience promise
  2. create story arc and pressure point
  3. align tone, proof, stakes, payoff, and call-to-action
  4. add presentation_outline when the medium is pitch, deck, or slides
  5. save storytelling artifact

outputs:
  - story frame
  - optional presentation outline
  - tone constraints
  - narrative artifact

done_when:
  - arc has beginning, tension, and payoff
  - pitch/deck requests include audience shift, proof, and call-to-action
  - tone matches audience
  - next production step is known

blocked_when:
  - audience or medium is unknown
  - factual constraint contradicts the arc

handoff:
  - preserve story frame, medium, outline, tone, constraints, proof, call-to-action, and next artifact
