# workflow: retrospective

trigger:
  - user asks to retro an increment, release, sprint, or project phase
  - ready/evolve work needs learning converted into action

inputs:
  - completed stories or release evidence
  - review findings and failures
  - checkpoints and decisions
  - user feedback or operating notes

steps:
  1. collect outcomes, evidence, surprises, and unresolved pain
  2. separate keep, change, stop, and try items
  3. convert actions into stories, inputs, risks, or rejected work
  4. write retrospective artifact
  5. route evolve-project, plan-sprint, session-prep, or ready-release

outputs:
  - retrospective artifact
  - action items
  - next workflow

done_when:
  - learning is tied to evidence or observed work
  - actions have owner/workflow and next state
  - rejected or deferred items are explicit

blocked_when:
  - no completed increment or evidence exists
  - feedback is too vague to generate action

handoff:
  - preserve retro artifact path, action items, deferred items, and next workflow
