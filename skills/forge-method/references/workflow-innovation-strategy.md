# workflow: innovation-strategy

trigger:
  - user asks for new product, market, or concept possibilities
  - idea needs strategic differentiation

inputs:
  - opportunity area
  - audience
  - constraints
  - existing alternatives

steps:
  1. identify adjacent possibilities
  2. compare novelty, usefulness, and feasibility
  3. choose a focused innovation bet
  4. define proof needed

outputs:
  - opportunity map
  - selected bet
  - proof plan

done_when:
  - selected direction has a reason
  - feasibility and proof are explicit
  - next validation step is known

blocked_when:
  - market or audience is undefined
  - novelty conflicts with feasibility constraints

handoff:
  - preserve opportunity, selected bet, proof plan, and next step
