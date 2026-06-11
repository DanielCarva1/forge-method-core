# workflow: grill-gate

trigger:
  - state.phase in 1-discovery | 2-specification | 3-plan
  - phase output is about to unlock mechanical work
  - user asks to grill a decision or plan

inputs:
  - `.forge-method/state.yaml`
  - current phase artifact
  - `CONTEXT.md`
  - relevant ADRs
  - reality/evidence decision artifact when the phase makes a product or market claim
  - acceptance criteria and constraints

steps:
  1. restate the phase decision in one compact paragraph
  2. compare it against glossary terms, ADRs, reality/evidence stance, constraints, and acceptance criteria
  3. identify contradictions, missing definitions, risky assumptions, and irreversible choices
  4. ask one human question only when the contradiction cannot be resolved from artifacts
  5. write a compact grill artifact with decision, risks, assumptions, and next action
  6. update state with the grill artifact and the next safe workflow

outputs:
  - grill decision artifact
  - resolved assumptions
  - next safe workflow

done_when:
  - phase decision is internally consistent
  - unresolved human decisions are durable inputs
  - `last_grill_artifact` points to the compact artifact
  - mechanical work can continue without procedural confirmation

blocked_when:
  - required artifact is missing
  - glossary or ADR conflict cannot be resolved from project artifacts
  - human decision changes scope, promise, or risk tolerance

handoff:
  - preserve grill artifact path, accepted assumptions, remaining risks, and next workflow
