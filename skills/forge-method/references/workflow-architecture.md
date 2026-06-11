# workflow: architecture

trigger:
  - product requirements exist
  - track requires technical design before build

inputs:
  - product requirements
  - current codebase inventory
  - constraints
  - validation expectations

steps:
  1. identify system boundaries and dependencies
  2. choose implementation approach
  3. record data, integration, security, and deployment notes
  4. define risks and validation hooks
  5. save architecture artifact

outputs:
  - architecture artifact
  - risk list
  - validation hooks

done_when:
  - build stories can reference architecture constraints
  - major risks have checks or mitigations
  - no required decision is hidden

blocked_when:
  - architecture choice depends on unanswered human input
  - implementation path has incompatible constraints

handoff:
  - preserve architecture path, decisions, risks, and build constraints
