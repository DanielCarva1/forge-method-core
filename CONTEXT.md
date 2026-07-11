# Forge Method Core Domain Context

This file is a navigation aid for agents working on the Forge codebase. It is
not runtime authority. Canonical product decisions remain typed under
`contracts/`.

## Product thesis

Forge is an agent-native governance control plane. A human communicates through
chat with a host agent. The host agent owns research, artifacts, implementation,
tests, and explanation. Forge owns project truth, obligations, authority,
evidence, continuity, and next-best-action guidance.

Forge governs what must be true and what must be proven. It does not script the
agent's words, persona, reasoning, or implementation strategy.

Canonical policy:
`contracts/policies/agent-native-product-constitution.yaml`.

Active architecture direction:
`contracts/spec/agent-native-assurance-architecture.yaml`.

Active implementation plan:
`contracts/plan/agent-native-guidance-plan.yaml`.

## Domain language

- **Human**: the source of intent, preferences, value judgments, trade-offs,
  acceptance, and exceptional authority. The human is not expected to edit
  Forge artifacts or operate the development toolchain.
- **Host Agent**: the model-driven worker that converses with the human and
  performs research, planning, artifact creation, implementation, verification,
  and explanation.
- **Forge**: the model-agnostic protocol and runtime that governs state,
  obligations, authority, evidence, and continuity.
- **Intent Proposal**: the host agent's typed interpretation of the human's
  desired outcome, constraints, preferences, unacceptable outcomes, and open
  uncertainty.
- **Project Snapshot**: a derived, evidence-backed view of the project's current
  state. It is not a hand-edited status document.
- **Obligation**: a condition that must become true or be explicitly waived by
  authorized judgment. It defines required outcomes, not procedural dialogue.
- **Assurance Claim**: a proposition about the product or process whose status
  is unknown, hypothesized, supported, verified, disproven, or waived.
- **Evidence**: provenance-bearing observation that supports or disproves an
  Assurance Claim. Representative execution is stronger than artifact presence.
- **Playbook**: a non-authoritative strategy an agent may use to satisfy one or
  more obligations.
- **Evaluator**: a deterministic or governed method for assessing evidence
  against an Assurance Claim.
- **Decision Request**: a question sent to the human only when value, preference,
  material trade-off, irreversible risk, cost, or external authority cannot be
  resolved from project evidence.
- **Capability Gap**: an explicit statement that the current agent, tools,
  environment, knowledge, or evaluators cannot reliably complete or verify an
  obligation.
- **Domain Pack**: an extension that contributes domain-specific obligations,
  hazards, playbooks, evaluators, evidence rules, and capability requirements.
- **Phase Projection**: a human- and agent-friendly summary of maturity derived
  from satisfied obligations. It is not the primary source of authority.
- **Execution Principal**: the authenticated and authorized caller identity and
  role derived by a trusted Adapter. A caller-selected key or a valid signature
  by itself is not an Execution Principal with mutation authority.
- **Execution Admission**: a deterministic commit-time decision that binds the
  ready Assurance Case, exact Operation/Command/Effect contracts, principal,
  replay reservation, claim/gate revisions, and commit guarantees. P4a is the
  pure policy decision point; it is not yet the runtime enforcement point.
- **Replay Reservation**: a durable, single-use binding between a fresh nonce,
  a revision, and the canonical execution-intent digest.
- **Commit Assurance**: verified guarantees that the chosen WAL or saga scope
  can recover, roll back, or compensate the complete authorized mutation.
- **Operation Effect Bundle**: a kernel-derived internal transaction envelope
  that preserves the declared effect identities while placing the complete
  disjoint file-backed write set under one effect lock, WAL, and recovery
  outcome. It is implementation, not caller-selected authority.

## Architectural direction

- The **Project Snapshot Module** concentrates state derivation.
- The **Obligation Engine Module** derives required claims, gaps, decisions, and
  next-best actions from Intent Proposal plus Project Snapshot.
- The **Execution Assurance Kernel** governs authority and durable mutation.
  The P4a decision module defines its fail-closed admission contract; the P4b
  Adapter/kernel integration is required before Forge claims runtime enforcement.
- The **Operation Effect Bundle Module** deepens the existing local effect-store
  transaction for multi-effect operations. Sagas remain a separate future
  boundary for external or irreversible commit domains.
- Host-specific integrations are **Adapters** at a host seam; deleting one must
  not change Forge domain behavior.
- Workflows migrate from authoritative step sequences into policies,
  obligations, playbooks, and evaluators.

These Modules should earn **Depth** by keeping their **Interface** smaller than
their **Implementation**, increasing **Leverage** for callers and **Locality** for
maintainers. Apply the deletion test before introducing additional crates or
pass-through layers.

## Core epistemic rule

Human ignorance is expected. Agent ignorance is expected. Hidden or unmanaged
ignorance is a governance failure.

Forge cannot guarantee discovery of every unknown unknown. It can require a
repeatable assurance process that makes consequential ignorance increasingly
likely to surface before completion is declared.
