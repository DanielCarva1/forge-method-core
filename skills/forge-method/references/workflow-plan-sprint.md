# workflow: plan-sprint

trigger:
  - state.phase == 3-plan
  - specification artifact exists

inputs:
  - specification artifact
  - approved decision sources
  - acceptance criteria
  - current repository structure
  - known checks

steps:
  1. verify accepted sources before sequencing work
  2. split acceptance criteria into story batches
  3. sort by user value, dependency, risk, and learning
  4. map each ready story to decision sources and validation evidence
  5. mark deferred or blocked work with reason
  6. update sprint summary and next executable story
  7. move only implementation-ready work into phase 4

outputs:
  - sprint plan artifact
  - ordered story batch
  - decision-source map
  - validation and evidence plan
  - updated state

done_when:
  - each executable story has acceptance criteria and decision sources
  - next ready story is known
  - blocked/deferred work has a reason
  - checks are known or explicitly marked manual

blocked_when:
  - approved decision sources are missing
  - architecture choice changes story boundaries materially
  - validation cannot be defined

handoff:
  - preserve story order, next story, source map, validation plan, deferred work, and unresolved risks
