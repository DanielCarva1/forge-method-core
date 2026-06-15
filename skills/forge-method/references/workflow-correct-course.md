# workflow: correct-course

trigger:
  - new evidence contradicts current plan
  - user says the route, scope, taste, artifact, or implementation is wrong
  - user asks to pivot, simplify, rewind, or rescue work

inputs:
  - current state
  - affected artifacts
  - new constraint or evidence
  - open stories

steps:
  1. identify what changed
  2. classify contradiction: route, scope, taste, evidence, implementation, communication, or state
  3. classify impact on requirements, architecture, plan, stories, evidence, and human trust
  4. preserve decisions and discarded paths
  5. choose repair: rollback, insert missing workflow, rewrite artifact, split scope, defer, or escalate
  6. update affected artifacts or create new stories
  7. set the next safe action

outputs:
  - correction artifact
  - impact map
  - selected repair path
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
