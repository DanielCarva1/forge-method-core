# workflow: council-decision

trigger:
  - decision is high-risk, taste-heavy, or benefits from specialist perspectives
  - user asks for Agent Council

inputs:
  - topic
  - current state
  - relevant artifacts
  - selected agent profiles
  - requested mode: debate, decision, parallel, agent-team, subagent, or validate

steps:
  1. select relevant agents and roles
  2. choose debate, decision, or orchestration mode from the topic and available runtime
  3. show human-facing debate live when useful
  4. preserve dissent, risks, evidence gaps, and recommendation
  5. define compact orchestration plan for sequential, parallel, agent-team, or subagent execution
  6. save compact council decision artifact
  7. update next action

outputs:
  - live council transcript for the human
  - compact council decision artifact
  - orchestration plan
  - dissent map
  - updated state pointer

done_when:
  - decision artifact exists
  - participant roles and dissent are recorded
  - orchestration mode and merge contract are recorded
  - next action is concrete
  - transcript is not required for future context

blocked_when:
  - topic is too vague
  - required participant role is unavailable
  - requested parallel/subagent mode cannot produce independent worker outputs

handoff:
  - preserve artifact path, participants, mode, worker outputs, merge contract, recommendation, dissent, evidence needed, and next action
