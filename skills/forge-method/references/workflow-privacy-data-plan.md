# workflow: privacy-data-plan

trigger:
  - project collects, stores, processes, or transmits user data
  - privacy or retention requirements matter

inputs:
  - data inventory
  - user flows
  - storage and integration notes
  - regulatory constraints

steps:
  1. classify data categories
  2. define collection, retention, deletion, and consent rules
  3. identify privacy risks and checks
  4. save privacy/data plan

outputs:
  - privacy and data plan
  - data classification
  - required checks

done_when:
  - data handling rules are explicit
  - retention and deletion are defined
  - release risks are recorded

blocked_when:
  - data categories are unknown
  - legal requirement needs human input

handoff:
  - preserve data plan path, classifications, risks, and open questions
