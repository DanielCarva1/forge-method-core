# workflow: engine-setup

trigger:
  - selected engine needs project setup before architecture or prototype work
  - human asks for engine template, folder layout, first run command, or setup proof

inputs:
  - game brief or GDD
  - selected engine and version if known
  - target platform
  - asset/tooling constraints
  - repository state

steps:
  1. choose or confirm the engine profile
  2. define project structure, language/runtime, package/tooling, and asset pipeline assumptions
  3. define first-run, smoke, and validation commands
  4. identify setup risks and engine-specific performance constraints
  5. write engine setup artifact and route engine-architecture or quick-prototype

outputs:
  - engine setup artifact
  - first-run command contract
  - setup risks

done_when:
  - engine profile, project structure, run command, and validation proof are explicit
  - next architecture or prototype story is known

blocked_when:
  - engine choice is undecided
  - local tooling cannot be installed or verified
  - target platform changes the setup materially

handoff:
  - preserve setup artifact path, engine profile, commands, risks, and next workflow/story
