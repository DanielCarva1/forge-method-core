# workflow: risk-register

trigger:
  - project has unresolved technical, product, delivery, security, or operational risks
  - enterprise or release work needs risk visibility

inputs:
  - requirements
  - architecture
  - open findings
  - team constraints

steps:
  1. list risks with category and impact
  2. assign mitigation, owner, or accepted status
  3. link risks to stories or checks
  4. save risk register

outputs:
  - risk register
  - mitigations
  - linked checks

done_when:
  - high risks have mitigation or explicit acceptance
  - risks are linked to checks or stories
  - next action is known

blocked_when:
  - risk owner is required but unknown
  - risk cannot be evaluated with available evidence

handoff:
  - preserve risk register path, high risks, owners, and next mitigation
