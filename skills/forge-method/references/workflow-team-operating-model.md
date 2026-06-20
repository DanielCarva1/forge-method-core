# workflow: team-operating-model

trigger:
  - multiple humans or agents will use Forge in one product
  - user asks for GitHub org/repo setup, team workflow, ownership, reviews, or agent usage
  - work should be trunk-based or coordinated across collaborators

inputs:
  - product/project goal
  - GitHub org and repo shape
  - collaborators, owners, and roles
  - review, merge, CI, release, and agent-use preferences

steps:
  1. define Root Integrator Project, repo shape, roles, and decision rights
  2. choose monorepo-first operating model unless split criteria are already met
  3. record trunk-based branch, PR, review, CI, and release expectations
  4. route product-area-map before parallel story/build work
  5. route platform or quality follow-up when GitHub/CI details need implementation

outputs:
  - team operating model artifact
  - owner/review policy
  - next collaboration workflow

done_when:
  - future teammate knows where to work, who reviews, what blocks merge, and how agents hand off work
  - Product Area mapping is next or already linked

blocked_when:
  - team owner, repo shape, branch policy, or merge authority is unknown
  - collaboration policy conflicts with existing project state

handoff:
  - preserve root repo, owners, trunk policy, review/check expectations, agent-use rules, next workflow, and blockers
