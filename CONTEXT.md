# Forge Method Core context

This file is a concise navigation aid for agents working on Forge. It is not
runtime authority and does not duplicate release logs or contract internals.

## Authority and current scope

Use evidence in this order:

1. runtime receipts and admitted compiled material;
2. typed contracts under `contracts/`;
3. generated references;
4. explanatory prose;
5. retained historical notes.

The active source line is **`0.12.0-alpha.27`**. `Cargo.toml` is the package
version authority; `forge-core --version` identifies the executable actually on
`PATH`. `README.md` owns the canonical identity table. Historical prerelease
numbers belong in `CHANGELOG.md` and audit evidence, not in current guidance.

The current product milestone is **Solo Dogfood Ready** for one developer using
cooperative host agents. Its typed authority is:

- `contracts/spec/solo-dogfood-readiness-v0.yaml`

The current product and architecture direction is defined by:

- `contracts/policies/agent-native-product-constitution.yaml`
- `contracts/spec/agent-native-assurance-architecture.yaml`
- `contracts/plan/agent-native-guidance-plan.yaml`

External human-origin brokers, FIDO-backed presence, independently administered
reviewer/runtime identities, and compliance-grade signing are deferred to a
later enterprise profile. They must not appear in the normal Solo Cooperative
journey or block its closure. Existing strict-external implementation and
history may be retained for future reuse, but it is not the current product
roadmap.

## Product thesis

Forge is a local, model-agnostic governance control plane for agent-led product
work:

- the human stays in ordinary chat and supplies outcomes, preferences, material
  trade-offs, risk acceptance, and irreversible/external decisions;
- the host agent owns research, planning, implementation, testing,
  documentation, retries, and other reversible local tactics;
- Forge owns durable project truth, obligations, evidence, continuity,
  promotion boundaries, and next-best-action guidance.

Forge governs what must be true and what must be proven. It does not script the
agent's words, persona, reasoning, file order, or implementation strategy.

## Current operating model

1. Invoke the Start Forge skill once per chat.
2. Resolve the exact project root and installed executable.
3. Run `start`, execute only returned structured argv, then initialize/resume.
4. Require `readiness_profile=solo_cooperative` for current dogfooding.
5. Recover the active objective and explain the project in plain language.
6. Let the agent perform reversible local work autonomously.
7. Admit honest, claim-bound cooperative evidence without pretending it is
   independent or enterprise-grade.
8. Govern promotion into canonical project state with exact bindings,
   recovery, and receipts.
9. Ask the human only for an irreducible product decision or an
   irreversible/external effect.

Missing enterprise registries, brokers, signatures, or external actors are not
Solo Cooperative setup gaps. If they become blocking in this profile, treat it
as product drift rather than asking the solo developer to provision them.

## Core language

- **Human**: source of desired outcomes and irreducible value/risk decisions.
- **Host Agent**: model-driven worker that performs technical work and drives
  Forge through typed interfaces.
- **Solo Cooperative**: same-owner development with honest chat provenance and
  no claim of actor independence or enterprise compliance.
- **Intent Proposal**: typed interpretation of outcome, constraints,
  unacceptable outcomes, and uncertainties from chat.
- **Project Snapshot**: evidence-backed view of current project state; never a
  hand-edited status assertion.
- **Obligation**: condition that must become true or be explicitly dispositioned
  by the proper authority.
- **Assurance Claim**: proposition whose state may be unknown, supported,
  verified, disproven, or waived.
- **Evidence**: provenance-bearing observation bound to a claim and subject.
- **Capability Gap**: explicit inability to complete or verify an obligation.
- **Decision Request**: concise human question only for an irreducible choice.
- **Product Lifecycle**: the complete guided path from Analysis and Discovery
  through Product Planning, Solution Definition, Implementation, and Validation
  and Delivery. Every greenfield, brownfield, or material-change cycle follows
  this path with proportional depth.
- **Work Focus**: the currently accepted, bounded piece of work inside the active
  objective. It can point to an external issue, but the issue tracker is not
  Forge authority.
- **Current Work Context**: a compact read-only view composed from durable Forge
  state. It tells a host agent what is being pursued, where the work is, what was
  agreed, what blocks progress, and what should happen next without replaying the
  full history.
- **Quick Cycle**: the compact form of the Product Lifecycle for clear, bounded,
  reversible work. It visits every lifecycle stage in one small durable record
  instead of silently skipping stages or creating a document for each one.
- **Expansion Signal**: evidence that a Quick Cycle is no longer enough, such as
  unclear intent, broad impact, architectural choice, material risk, or a failed
  review that exposes an earlier misunderstanding. Only the affected stage is
  expanded unless the evidence invalidates more of the cycle.
- **Evolve**: a short user-triggered transition for a stable product. It records
  the desired material change and immediately opens a new Product Lifecycle in
  Analysis and Discovery. Feature development never happens inside Evolve.
- **Phase Projection**: maturity summary derived from obligations, not primary
  authority.
- **Governed Promotion**: admission of isolated agent work into canonical state
  with bound diff, evidence, effects, recovery, and receipts.
- **Domain Pack**: content-addressed extension that contributes domain-specific
  policies and evidence vocabulary without adding domain branches to core.

## Architecture map

- **Project Snapshot Module** derives bounded project truth.
- **Obligation Engine** derives claims, gaps, decisions, and ranked next actions
  from accepted intent plus the snapshot.
- **Workflow Governance Kernel** evaluates the admitted policy release and
  persists append-only continuity.
- **Execution Assurance Kernel** binds authority, effect, WAL/recovery, and
  durable receipts at a real mutation boundary.
- **Domain Pack lifecycle** composes immutable namespaced domain material into a
  project-local effective epoch.
- **Host adapters and skills** translate chat and tools; deleting one host
  adapter must not alter Forge domain behavior.

Critical state uses content addressing, append-only hash-linked history,
retained locks, compare-and-swap bindings, and explicit recovery. These protect
against accidents and ambiguous local outcomes; they are not a cryptographic
sandbox against a malicious process running as the same OS user.

## Documentation ownership

- `README.md`: product scope, identity, installation, and top-level boundaries.
- `docs/getting-started.md`: solo developer journey.
- `docs/agent-integration.md`: host integration loop.
- `docs/operator-guide.md`: advanced state/recovery and future enterprise trust
  operations; it is not normal solo onboarding.
- `docs/security-model.md`: explicit guarantees and residual threats.
- `docs/generated/`: machine-checked command and workspace references.
- `CHANGELOG.md`: historical source checkpoints.

Do not copy version/status paragraphs between guides. Link to the owning page or
contract instead.

## Core epistemic rule

Human ignorance is expected. Agent ignorance is expected. Hidden or unmanaged
ignorance is a governance failure. Forge cannot guarantee discovery of every
unknown unknown; it can make consequential uncertainty explicit before
completion is declared.
