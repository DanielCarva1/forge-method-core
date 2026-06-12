# ADR 0008: Guidance Engine

## Status

Accepted

## Context

Forge Method already has durable state, preflight, resume guidance, workflows, Grill Gate, Correct-Course Continuation, and Mechanical Autonomy. The weak spot was human routing: a stale `next_action` could dominate even when the latest user message clearly corrected the route, expressed confusion, asked for brainstorm/research, or introduced new intent.

The runtime needs one canonical subsystem that reads the latest human message and durable state together, then chooses the next workflow without relying on chat memory.

## Decision

Create **Guidance Engine** as the canonical route interpreter behind `guide --question --json`.

Guidance Engine returns:

- `intent_classification`
- `signals`
- `recommended_phase`
- `recommended_workflow`
- `recommended_action`
- `human_prompt`
- `alternatives`
- `state_update_required`
- `state_updates`
- `commands`

`guide --question --json` is authoritative when the latest human message is substantive. A correction, complaint, doubt, brainstorm request, research request, or new intent may override an older `next_action`. A ready project with fresh critique or new intent should enter `6-evolve` or `correct-course`, not keep repeating release/publication work.

Hot Start must run `guide --question --json` after `preflight`, `start`, and `resume --json` whenever the invocation contains substantive human intent beyond merely starting the runtime.

Corrections about Forge Method's own human experience have precedence over generic runtime-builder routing. If the user says the method skipped facilitation, did not ask enough, created technical artifacts too early, or kept asking procedural confirmations, Guidance Engine must route to `correct-course` first. `runtime-builder` is the repair path after the failed behavior is named and preserved evidence is recorded.

## Boundaries

- Human Experience uses Guidance Engine output to speak clearly, ask one useful question, and offer alternatives.
- Agent Runtime consumes the compact JSON contract and workflow references.
- Correct-Course handles rejected routes and method failures.
- Evolve handles new intent for ready projects.
- Guide is the command surface; Guidance Engine is the routing subsystem inside it.

## Consequences

Tests must cover transcript-shaped inputs, not only phase transitions. Runtime docs and skill instructions must teach agents to follow Guidance Engine output instead of stale chat state or stale `next_action`.

Internal benchmark notes may inform expected behavior, but product-facing docs must describe Forge Method by its own runtime model and vocabulary.
