# workflow: write-spec

trigger:
  - state.phase == 2-specification
  - discovery intent exists
  - user asks to create, update, validate, or distill a spec kernel

inputs:
  - intent artifact
  - source artifacts or mixed notes
  - constraints
  - success criteria
  - relevant domain notes
  - existing spec kernel when updating

steps:
  1. choose create, update, distill, or validate mode
  2. separate load-bearing source claims from wrapper prose
  3. preserve or assign stable capability IDs
  4. derive artifact spec-kernel arguments: source_artifacts, why, capabilities, constraints, non_goals, success_signal, preservation_map, validation_verdict, and next_workflow
  5. move bulky load-bearing detail into companions or adopted source refs
  6. record assumptions, open questions, preservation map, and decision log
  7. run `artifact spec-kernel` to write the compact spec artifact
  8. run `artifact spec-check --path <spec-kernel-artifact>`
  9. route the next workflow

outputs:
  - spec kernel artifact
  - companion/source map
  - validation verdict
  - next workflow

done_when:
  - each capability has id, intent, and success
  - constraints bend design decisions
  - non-goals and success signal are explicit
  - load-bearing claims are preserved or marked open
  - `artifact spec-check` passes

blocked_when:
  - intent is too thin to distill
  - a requirement conflicts with a stated constraint
  - success signal cannot be tested or demonstrated
  - load-bearing source ownership is unknowable

handoff:
  - preserve spec path, companions, sources, capability IDs, decision log, assumptions, open questions, validation verdict, and next workflow
