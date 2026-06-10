# workflow: discover-intent

trigger:
  - state.phase == 1-discovery
  - user intent is broad, ambiguous, or early-stage

inputs:
  - user intent
  - constraints
  - target user or audience
  - success definition

steps:
  1. identify the creation type: software, product, creative, game, automation, or runtime module
  2. capture user, problem, desired outcome, constraints, and non-goals
  3. convert vague terms into concrete project language
  4. write a concise intent artifact under `.forge-method/artifacts/`
  5. update state next action toward specification

outputs:
  - intent artifact
  - clarified constraints
  - success criteria
  - updated state

done_when:
  - project intent can be explained in one paragraph
  - success criteria exist
  - next specification workflow is known

blocked_when:
  - materially different project directions remain equally plausible
  - missing user/audience changes the entire product

handoff:
  - preserve intent, constraints, success criteria, and next workflow

