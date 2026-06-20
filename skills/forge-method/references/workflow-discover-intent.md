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
  2. capture user, problem, desired outcome, constraints, non-goals, visible_or_operational_proof, and early_visual_feedback_loop
  3. convert vague terms into concrete project language
  4. run `artifact discovery-closeout` to write the accepted discovery artifact
  5. run `artifact discovery-check --path <discovery-closeout-artifact>`
  6. update state next action toward specification

outputs:
  - intent artifact
  - clarified constraints
  - success criteria
  - visible proof need
  - updated state

done_when:
  - project intent can be explained in one paragraph
  - success criteria exist
  - user-facing products name the next visible artifact or why visual proof is not applicable
  - discovery closeout artifact passes `artifact discovery-check`
  - next specification workflow is known

blocked_when:
  - materially different project directions remain equally plausible
  - missing user/audience changes the entire product

handoff:
  - preserve discovery closeout artifact path, constraints, non-goals, success criteria, visible proof need, Grill Gate handoff, and next workflow
