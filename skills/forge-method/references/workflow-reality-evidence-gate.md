# workflow: reality-evidence-gate

trigger:
  - new idea, product bet, feature promise, or market claim appears in discovery
  - grill gate needs feasibility, ethics, safety, or evidence checks
  - user asks whether an idea is viable

inputs:
  - user idea or phase artifact
  - target user or audience
  - constraints, risks, and known evidence
  - optional market, domain, or technical scan

steps:
  1. test physical, biological, legal, ethical, and safety possibility before market appeal
  2. check whether the user pain and target user are real enough to proceed
  3. compare obvious alternatives and reasons this may already be solved or rejected
  4. assign a compact viability stance: blocked, weak, plausible, or strong
  5. identify the minimum evidence needed before specification or build
  6. save only the compact decision artifact and next validation action

outputs:
  - reality/evidence decision artifact
  - viability stance and core reason
  - required follow-up scans or proof

done_when:
  - impossible, cruel, unsafe, or illegal ideas are blocked without market inflation
  - plausible ideas name the evidence needed next
  - decision artifact is compact enough for handoff
  - next workflow is known

blocked_when:
  - audience or problem is unknown and changes viability
  - required domain, market, legal, or technical evidence is inaccessible
  - human must choose a scope, risk tolerance, or ethical boundary

handoff:
  - preserve stance, score if used, decisive evidence, unresolved risks, and next scan
