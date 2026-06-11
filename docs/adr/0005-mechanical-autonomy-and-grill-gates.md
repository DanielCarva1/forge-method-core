# ADR 0005: Mechanical Autonomy And Grill Gates

## Status

Accepted.

## Context

Forge Method should preserve rich human facilitation while removing procedural confirmations from mechanical work. Repeated prompts to continue the next story, accept a review loop, or run the next check create friction without adding product judgment.

## Decision

Forge uses Mechanical Autonomy by default for procedural work. Discovery, specification, and planning close with Grill Gate before they unlock long mechanical execution. Build and ready work then continue without procedural human confirmation while checks, reviews, evidence, and state updates remain mandatory.

Late contradictions use Correct-Course Continuation: write a compact correction artifact, choose the conservative interpretation that preserves the approved spec, and continue. Human input is reserved for credentials/access, destructive approval, explicit user scope change, or a contradiction that cannot be resolved from durable artifacts.

Long mechanical loops should prefer Codex Goal mode when available. Automations are reserved for recurring checks, monitoring, and follow-up work.

## Consequences

- Users spend attention on taste, scope, risk, and intent instead of ceremony.
- Runtime files must be stronger because later phases rely on earlier Grill Gates.
- The agent must record evidence and correct-course artifacts instead of silently improvising.
- Projects can still configure commit policy; automatic commits are off by default.
