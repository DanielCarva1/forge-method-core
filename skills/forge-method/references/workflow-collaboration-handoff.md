# workflow: collaboration-handoff

trigger:
  - work moves between teammates, agents, chats, branches, or Product Areas
  - user asks for handoff, PR summary, continuation note, or parallel work status
  - branch/PR/evidence context must survive chat reset

inputs:
  - active Product Area
  - owner or next actor
  - branch/PR
  - decisions, files, checks, evidence, blockers, and next command

steps:
  1. capture Product Area, owner, branch/PR, touched surfaces, and decisions
  2. record checks run, evidence written, open blockers, and review/merge status
  3. name the next accountable human or agent and first command
  4. update state, sprint, story, or checkpoint reference when needed

outputs:
  - collaboration handoff artifact
  - next actor and first command

done_when:
  - another human or agent can continue without replaying chat or guessing branch/PR status
  - blockers and owners are explicit

blocked_when:
  - branch/PR, Product Area, or next owner is unknowable
  - validation evidence is missing for completed claims

handoff:
  - preserve Product Area, branch/PR, owner, decisions, checks, evidence, blockers, review status, and first command
