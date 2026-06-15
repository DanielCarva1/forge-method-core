# workflow: teach-testing

trigger:
  - user asks to learn testing, QA, test architecture, or quality strategy
  - user needs explanation before choosing a test workflow

inputs:
  - learner goal
  - product or code context
  - current testing knowledge
  - quality risk or decision to explain

steps:
  1. identify the learner's immediate decision or confusion
  2. explain the smallest useful testing concept in project terms
  3. connect the concept to quality engagement mode, risk, evidence, and one concrete workflow
  4. preserve examples, misconceptions, caveats, and the recommended next test workflow

outputs:
  - testing explanation
  - applied examples
  - recommended engagement model
  - recommended next test workflow

done_when:
  - the concept is explained in the project's context
  - the explanation changes the next quality decision
  - the learner has a concrete next workflow or check
  - assumptions and caveats are recorded

blocked_when:
  - there is no product or learning objective
  - the question requires external policy or certification detail not available locally

handoff:
  - preserve explanation path, examples, misconceptions, engagement model, caveats, and next test workflow
