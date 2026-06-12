# workflow: game-ux-design

trigger:
  - game project needs game-specific UX decisions
  - player interaction, onboarding, HUD, controls, or accessibility are unclear

inputs:
  - game brief or GDD
  - target platform
  - player loop
  - input constraints

steps:
  1. identify primary player tasks and friction points
  2. map screens, controls, HUD, feedback, and onboarding needs
  3. define accessibility and platform-specific constraints
  4. record UX risks and prototype checks

outputs:
  - game UX plan
  - interaction map
  - UX validation checks

done_when:
  - core loop interactions are explicit
  - platform/input constraints are recorded
  - UX checks can be attached to stories

blocked_when:
  - player loop is unknown
  - target platform or input model is unavailable

handoff:
  - preserve UX plan path, screen/control assumptions, accessibility constraints, and checks
