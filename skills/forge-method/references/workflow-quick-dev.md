# workflow: quick-dev

trigger:
  - user asks for a small scoped product or code change
  - user asks for quick dev, quick flow, spec-lite, or a fast implementation path
  - change is too small for full PRD/architecture/story planning but still needs a compact decision record

inputs:
  - latest human request
  - current state and active module
  - relevant existing files, artifacts, or defects
  - constraints, acceptance signal, and validation command

steps:
  1. clarify the smallest coherent change and what must not change
  2. write a compact spec-lite with assumptions, acceptance evidence, and rollback notes
  3. decide whether the change can proceed headlessly or needs human input
  4. implement the scoped change through build-story or direct mechanical work as appropriate
  5. review, run validation, write evidence, and choose the next workflow

outputs:
  - quick-dev artifact
  - spec-lite and acceptance evidence
  - implementation/review notes
  - validation result
  - next workflow

done_when:
  - scope is narrow and explicitly bounded
  - acceptance evidence exists
  - implementation and review are complete or a build-story handoff exists
  - next workflow is explicit

blocked_when:
  - scope expands into product strategy, architecture, UX, security, or broad refactor
  - acceptance evidence is unknown
  - required files, tools, or external services are unavailable

handoff:
  - preserve quick-dev artifact path, scope, non-goals, files touched, checks, evidence, risks, and next workflow
