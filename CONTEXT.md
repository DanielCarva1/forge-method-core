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

## Architectural direction

- The **Project Snapshot Module** concentrates state derivation.
- The **Obligation Engine Module** derives required claims, gaps, decisions, and
  next-best actions from Intent Proposal plus Project Snapshot.
- The **Execution Assurance Kernel** governs authority and durable mutation.
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
