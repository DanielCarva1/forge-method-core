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
  1. extract behavior examples from acceptance criteria
  2. define given/when/then checks and edge cases
  3. map examples to automated or manual evidence
  4. attach examples to story and test plan

outputs:
  - ATDD examples
  - acceptance test map
  - story check updates

done_when:
  - examples cover core behavior and key edge cases
  - each example has a proof path
  - story checks are updated

blocked_when:
  - user behavior is unclear
  - no observable acceptance signal exists

handoff:
  - preserve examples, proof path, updated story checks, and open edge cases
