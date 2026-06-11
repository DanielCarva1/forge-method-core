# workflow: storytelling

trigger:
  - user asks for narrative, pitch, brand story, game story, or presentation arc
  - artifact needs emotional coherence

inputs:
  - audience
  - desired feeling
  - facts or constraints
  - medium

steps:
  1. define premise and audience promise
  2. create story arc
  3. align tone, stakes, and payoff
  4. save storytelling artifact

outputs:
  - story frame
  - tone constraints
  - narrative artifact

done_when:
  - arc has beginning, tension, and payoff
  - tone matches audience
  - next production step is known

blocked_when:
  - audience or medium is unknown
  - factual constraint contradicts the arc

handoff:
  - preserve story frame, tone, constraints, and next artifact
