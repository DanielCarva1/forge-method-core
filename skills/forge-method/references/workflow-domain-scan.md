# workflow: domain-scan

trigger:
  - reality/evidence gate needs domain rules, ethics, safety, or expert context
  - product touches animals, health, education, finance, law, children, or regulated work
  - idea depends on behavior, culture, workflow, or specialist knowledge

inputs:
  - idea or requirement
  - affected users, subjects, or stakeholders
  - domain constraints and known risks
  - available sources or expert notes

steps:
  1. identify domain constraints, norms, harms, and trust requirements
  2. flag cruelty, manipulation, safety, legal, privacy, and duty-of-care risks
  3. compare the idea with accepted practices and rejected patterns
  4. define what expert or primary-source evidence is needed
  5. write a compact domain scan and next safe action

outputs:
  - domain scan artifact
  - risk and norms summary
  - expert evidence needs
  - next safe action

done_when:
  - major domain constraints are explicit
  - harmful or non-compliant directions are blocked or reframed
  - open evidence needs are named

blocked_when:
  - domain risk cannot be assessed from available context
  - legal, clinical, financial, or safety decision requires qualified review
  - affected user or subject is unknown

handoff:
  - preserve constraints, harms, accepted practices, required review, and safe next action
