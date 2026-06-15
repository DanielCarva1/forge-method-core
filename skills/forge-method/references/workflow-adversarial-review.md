# workflow: adversarial-review

trigger:
  - user asks for adversarial review, red-team review, assumption attack, or stress test
  - plan, spec, workflow, or artifact needs critique before commitment

inputs:
  - target artifact or decision
  - success criteria
  - assumptions and constraints
  - risk tolerance and owner

steps:
  1. identify the claims, assumptions, and promises being made
  2. attack the strongest failure paths, misuse cases, false positives, and missing evidence
  3. separate fatal flaws, repairable concerns, and acceptable risks
  4. recommend repair, waiver, further evidence, or rejection
  5. route to edge-case-review, risk-register, correct-course, or build-story

outputs:
  - adversarial findings
  - broken assumptions
  - repair, waiver, or rejection recommendation
  - next workflow

done_when:
  - major assumptions have been challenged
  - findings are actionable and severity-ranked
  - the next workflow and owner are explicit

blocked_when:
  - target artifact is missing
  - success criteria are too vague to attack
  - risk owner or tolerance is unknown for high-impact decisions

handoff:
  - preserve target path, challenged assumptions, findings, severity, recommendation, owner, and next workflow
