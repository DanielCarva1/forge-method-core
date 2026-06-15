# P1.4 Lifecycle Closure Grill

- created_at: 2026-06-15T00:20:00+00:00
- workflow: runtime-builder
- plan: P1.4 Product, Context, Review, And Retrospective Closure

## Objective

Translate the remaining lifecycle-closeout behavior into Forge-native guided workflows without turning context packs, review commands, or checkpoints into verbose agent memory.

## Grill Questions Resolved

1. Is P1.4 a product-planning workflow?
   Recommended answer: no. The word "Product" in the batch title is audit taxonomy. The implementation target is runtime lifecycle closure.
   Resolution: accepted. Runtime-builder context and systematic parity plan signals must outrank domain words.

2. Are `context plan`, `review`, and `checkpoint` enough?
   Recommended answer: no. They are helper commands, not guided human workflows. P1.4 needs routeable workflows that explain why the closeout ritual matters and persist compact artifacts.
   Resolution: accepted and recorded in ADR 0011.

3. What is the canonical family name?
   Recommended answer: Lifecycle Closure. It covers track decision, project context, session prep, readiness matrix, code review, retrospective, and research closeout.
   Resolution: accepted and added to `CONTEXT.md`.

4. Should code review replace existing review findings?
   Recommended answer: no. `code-review` is the guided workflow; durable review findings remain the issue store used by audit, resume, and story completion guards.
   Resolution: accepted.

5. What proves this is compact enough for agents?
   Recommended answer: templates and runtime payloads must preserve route, sources, findings, decisions, checks, next workflow, and load hints, not full discussion transcripts.
   Resolution: accepted.

## Required Implementation Shape

- Add compact workflow refs for `track-decision`, `project-context`, `session-prep`, `code-review`, `retrospective`, and `research-closeout`.
- Add or update template coverage for project context, session prep, code review, retrospective, readiness matrix, track decision, and research closeout.
- Add a Lifecycle Closure facilitation pack.
- Add Guidance Engine routing and replay fixtures for document project, prep next session, review this code, retro this increment, research closeout, track decision, readiness matrix, and the internal P1.4 batch name.
- Preserve helper commands as implementation details behind workflows.

## Boundaries

- Lifecycle Closure owns guided closeout rituals and durable handoff artifacts.
- Context Pack remains generated runtime context, not the user-authored project context artifact.
- Review Finding remains the durable issue primitive; `code-review` can create findings but is not itself the issue store.
- Checkpoint remains progress memory; `session-prep` is the next-session plan derived from state and context.
