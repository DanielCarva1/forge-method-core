# workflow: correct-course

trigger:
  - new evidence contradicts current plan
  - user asks to pivot, simplify, or rescue work

inputs:
  - current state
  - affected artifacts
  - new constraint or evidence
  - open stories

steps:
  1. identify what changed
  2. classify impact on requirements, architecture, plan, and active stories
  3. preserve decisions and discarded paths
  4. update affected artifacts or create new stories
  5. set the next safe action

outputs:
  - correction artifact
  - updated stories or blockers
  - next action

done_when:
  - impacted scope is explicit
  - stale work is updated, deferred, or blocked
  - state points to the next safe workflow

blocked_when:
  - correction requires a human tradeoff
  - impacted work cannot be identified

handoff:
  - preserve changed decision, impacted artifacts, story updates, and next action
