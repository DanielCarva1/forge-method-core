# Post-Parity Functionality And Experience Audit

- kind: runtime-builder
- status: queued
- phase: 6-evolve
- workflow: agent-analyze

## Source Request

After parity work is complete, run a focused audit to prove the Forge Method now works for both sides of the product:

- human-facing guided experience
- agent-facing compact runtime, state, workflows, helpers, and automation

## Audit Scope

- Phase transitions and state-machine guards.
- Guidance Engine routing and intent detection.
- Track/module/area detection.
- Helper commands and recovery surfaces.
- Runtime automation scripts and smoke scripts.
- Facilitation packs, human prompts, first questions, and guided flow material.
- Agent-facing workflow refs, state handoff, recovery artifacts, capability index, and ledger/evidence behavior.
- Installed skill/plugin parity with the repository runtime.

## Human Experience Proof

- Broad idea starts with guided discovery/facilitation before technical planning.
- Doubt, frustration, and correction route to problem-solving, correct-course, research, or brainstorm instead of stale continuation.
- Creative, game, builder, quality, and support intents get domain-appropriate first questions.
- Humans can recover after reload/network drop and understand what happened without knowing internal phase names.

## Agent Contract Proof

- `preflight`, `start`, `resume --json`, `guide --question --json`, `next --json`, `context health`, and `context recover` agree on route and quality.
- State changes are durable in `.forge-method/state.yaml`, sprint/story files, checkpoints, evidence, or ledger.
- Capability index and recovery artifacts expose available route surfaces and context boundaries.
- Mechanical build paths remain autonomous and do not ask procedural continue questions.
- Installed skill/plugin runtime matches repository behavior for the same fixtures.

## Required Validation Shape

- Transcript replay fixtures for human flows.
- Unit tests for transitions, route classifiers, helper surfaces, and state updates.
- Smoke tests for runtime, install/plugin, reload, recovery, and scripts.
- Audit/gate coverage for workflow refs, facilitation packs, capability index, artifacts, agents, and evals.
- At least one end-to-end human scenario and one end-to-end agent recovery scenario.

## Done When

- The audit artifact has a pass/fail matrix for each feature area.
- All failures either have fixes, explicit follow-up stories, or rejected scope with rationale.
- Validation evidence proves both human guidance and agent runtime behavior.
- The final state can be resumed by a fresh Codex chat without relying on conversation memory.
