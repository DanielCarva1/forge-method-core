# workflow: market-scan

trigger:
  - reality/evidence gate needs market or competitor evidence
  - user asks whether people would pay for or adopt the idea
  - product claim depends on scarcity, differentiation, or demand

inputs:
  - product idea or requirement
  - target audience
  - geography, segment, price, or channel constraints
  - known alternatives and assumptions

steps:
  1. identify existing alternatives, substitutes, and common workarounds
  2. separate market scarcity from validated demand
  3. estimate adoption friction, buying path, and trust barriers
  4. name the strongest invalidation evidence to seek
  5. write a compact market scan with sources or explicit source gaps

outputs:
  - market scan artifact
  - alternative map
  - demand and adoption risks
  - next evidence step

done_when:
  - audience and alternatives are explicit
  - demand claim has evidence or is labeled assumption
  - next validation step is concrete

blocked_when:
  - target audience is undefined
  - current sources are required but unavailable
  - market claim depends on regulated, legal, or private data

handoff:
  - preserve audience, alternatives, demand evidence, assumptions, and next proof
