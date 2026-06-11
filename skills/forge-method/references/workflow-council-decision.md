# workflow: council-decision

trigger:
  - decision is high-risk, taste-heavy, or benefits from specialist perspectives
  - user asks for Agent Council

inputs:
  - topic
  - current state
  - relevant artifacts
  - selected agent profiles

steps:
  1. select relevant agents and roles
  2. show human-facing debate live when possible
  3. capture agreements, disagreements, risks, and recommendation
  4. save compact council decision artifact
  5. update next action

outputs:
  - live council transcript for the human
  - compact council decision artifact
  - updated state pointer

done_when:
  - decision artifact exists
  - next action is concrete
  - transcript is not required for future context

blocked_when:
  - topic is too vague
  - required participant role is unavailable

handoff:
  - preserve artifact path, participants, recommendation, dissent, and next action
