# workflow: atdd-plan

trigger:
  - user asks for ATDD, examples, or acceptance test design
  - story acceptance is too vague for build-story

inputs:
  - requirements or story
  - user scenario
  - risk notes
  - available test layers

steps:
  1. extract behavior examples from accepted requirements or story criteria
  2. define given/when/then checks, edge cases, and risk coverage
  3. map each example to automated, manual, or deferred evidence
  4. attach examples, story check updates, and next automation target

outputs:
  - ATDD examples
  - acceptance test map
  - risk coverage map
  - story check updates

done_when:
  - examples cover core behavior and key edge cases
  - each example has a proof path
  - high-risk behavior is represented or explicitly waived
  - story checks are updated

blocked_when:
  - user behavior is unclear
  - no observable acceptance signal exists

handoff:
  - preserve examples, proof path, updated story checks, and open edge cases
