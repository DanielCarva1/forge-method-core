# Forge Method Context Glossary

## Consumer Project Repo

The application, game, library, or product repository being developed with Forge Method. It owns product source code and may carry a small Forge Project Link, but it does not own Forge runtime state. Consumer repos must not use a local `<consumer>/.forge-method` state root.

## Forge Runtime Sidecar

A sibling directory or repository that owns the Forge Method runtime state for one Consumer Project Repo. It contains the real `.forge-method/` tree, including state, artifacts, evidence, ledger, stories, and claims.

## Forge Project Link

The small `.forge-method.yaml` file stored at a Consumer Project Repo root. It points to the Forge Runtime Sidecar and its `.forge-method/` state root. Its `state_root` must resolve under `sidecar_root` and must end in `.forge-method`, normally as `<sidecar_root>/.forge-method`.

## Project Init Bootstrap

Consumer repos should be bootstrapped with `forge-core project init --root <repo>`. The intended result is a `.forge-method.yaml` pointer in the Consumer Project Repo plus sibling sidecar state at `../forge-<project>/.forge-method`. The Consumer Project Repo should not receive a local `.forge-method/` directory.

The init command is expected to be idempotent for the same resolved link and to fail closed on a conflicting existing link or unsafe consumer-local state root.

## Bootstrap Core Exception

The temporary exception that allows `<repo-root>` to keep local `.forge-method/` state while the Forge core is still being developed by Forge itself. Commands that resolve this local state must opt in with `--allow-bootstrap-core`. This exception is explicit and must not be copied to consumer projects.

## Risk Audit

A fail-closed inspection pass over source code that detects AI-induced
anti-patterns (fail-soft, exception swallowing, security slop, false tests)
and accumulates typed `Diagnostic`s into a `ValidationReport`. Rules are
parametric YAML contracts (`risk-audit-v0`), never hardcoded imperative
sequences. The gate can run standalone (`forge-core risk-audit`) or be
attached as a precondition to a mutable operation (`RuntimeOperationExecutionContext`).
Findings carry per-file evidence so agents and humans can act on them.

## Anti-pattern (AI Code)

A named, parametrizable pattern in source code or test artifacts that is
forbidden because it correlates with AI-induced failure modes (fail-soft,
exception swallowing, security slop, false tests). Each anti-pattern is
declared in a `risk-audit-v0` contract with a `detector` (regex, glob,
AST node, external linter, or required file existence), a severity
(Error/Warning), an evidence requirement, and a fix hint. Anti-patterns
are data, not code: adding one must not require a Rust change.

## Project Link Hardening Rules

- `forge-core project init --root <repo>` is the normal first-use path for Consumer Project Repos.
- Init should be idempotent for the same resolved link and fail closed on a conflicting existing link or unsafe consumer-local state root.
- Consumer `state_root` must be inside the configured `sidecar_root` and end in `.forge-method`.
- Consumer `state_root` must not be `<consumer>/.forge-method`; only the Forge core bootstrap exception may use local runtime state.
- Runtime and claim commands fail closed when the resolved `state_root` does not exist; they must not silently create consumer-local state.
- State-bearing operation/effect commands (`execute-operation`, `rebuild-effect-index`, `query-effect-index`) resolve the same Project Link: product contracts and payload files are read from the Consumer Project Repo, but Forge WAL, metadata index, evidence, and `.forge-method/artifacts/*` writes land under the Forge Runtime Sidecar.
- `--claims-dir` remains an explicit advanced override for tests, migrations, and emergency repair.
- The goal is isolation: projects, users, and agents must not contaminate each other's Forge data.

## EvalArm

A labelled experimental condition that runs the same corpus of tasks. Today the labels are single-agent, graph, mas, and manual. Each arm is one variable in a comparison experiment: to claim that a multi-agent or graph architecture beats a single-agent baseline, the arms must differ only in the routing/coordination strategy while sharing the same task loader, the same tools, the same answer contract, and the same usage accounting. Arms are subprocesses; they do not evaluate themselves and they do not produce Forge contracts directly.

## EvalHarness

The executor that runs arms under control. It spawns each arm as a subprocess against the shared corpus, times the run externally, collects the raw output the arm produces, applies a uniform grader to compute the verdict, and is the sole producer of the canonical `EvalRunContractDocument` per run. Because the harness — not the arm — canonicalises and grades, the answer contract stays uniform across arms, which is the control mechanism that makes the comparison meaningful.

## Memory Document

A record admitted into the Forge memory store. A `MemoryEntry` (defined in
`forge-core-contracts/src/memory.rs`) carries content, provenance, freshness
(TTL + last-confirmed), a confidence score, and two trust attributes described
below. A `MemoryContract` is the container that groups entries under a scope
(Project / Repo / User / AgentRole). Memory Documents are never authoritative
by default; trust is earned via the two axes below, never inherited from
ingest.

## Memory Kind

The semantic class of a Memory Document, sharpening the four F06 terms:

- **Preference** — a stated want or default the agent should honour
  (`MemoryKind::Preference`). Low authority floor; high churn.
- **Decision** — a committed choice that constrains future work
  (`MemoryKind::Decision`). Stays `Provisional` until evidence-backed.
- **LessonLearned / PlaybookRule** — distilled operational knowledge
  (`MemoryKind::LessonLearned`, `PlaybookRule`). Promotable to `Authority`
  only with raw evidence (test output, log, diff).
- **FailurePattern** — a recurring anti-result to avoid
  (`MemoryKind::FailurePattern`).
- **GlossaryTerm** — a domain term definition (`MemoryKind::GlossaryTerm`).

"Fact" is **not** a `MemoryKind`. A record becomes a Fact only by reaching
`AuthorityLevel::Authority` (see Authority Axis); kind classifies content,
authority classifies trust. Conflating them is the Model B bug.

## Authority Axis (Trust Axis 1)

The ladder that says whether the agent may treat a Memory Document as ground
truth for autonomous action. The three rungs:

- **Raw** — freshly ingested, no evidence endorsement. Admitted for retrieval
  as context but never actionable as fact. This is the default on ingest.
- **Provisional** — evidence-backed candidate, pending stronger proof or
  review. May inform action but not the final word.
- **Authority** — the agent may act on it as ground truth. **Reaching
  `Authority` requires non-empty `evidence_refs` AND a satisfied promote
  policy.** It does not require review (the axes are orthogonal). This is the
  F06 NFR: *promote exige policy e evidência raw — nunca automático.*

The legacy single-axis `ApprovalState` (`Proposed/InReview/Approved/Rejected/
AutoPromoted`) is bridged, not deleted: `Approved` maps to `Provisional`,
`AutoPromoted` is a deprecated anti-pattern (see AutoPromoted Anti-pattern).
Mapping lives in one function (`authority_level_effective`), not in N callers.

## Review Axis (Trust Axis 2)

Orthogonal to authority: has a principal attested to this record's curation?

- **Unreviewed** — no principal has curated the record (default).
- **Reviewed** — a principal attested to the record via a `StableId`
  attestation (`reviewed_by`, `reviewed_at`). The attestation is modelled as
  a F07 principal attestation, **not a magic boolean**.

Review never implies authority promotion, and authority never implies review.
The two axes compose into a six-cell state space; all six are meaningful
(`Memory` may be *Reviewed but Raw* — a faithfully recorded observation whose
content is not endorsed as fact; or *Unreviewed but Authority* — a spec seeded
authoritative by provenance type, not human approval).

## Admission

The gate that decides whether a Memory Document enters the store at all
(`Raw`, `Unreviewed`). Admission checks the `MemoryPolicy`: is the `kind`
permitted? Is required provenance/evidence present? Admission fails closed if
the policy or evidence is missing. Admitting a document never sets authority
above `Raw` and never sets review above `Unreviewed` — those require their
own gates.

## Retention

The rules that govern how long an admitted Memory Document stays in the store.
Retention is TTL-driven (lazy sweep on read) plus explicit `forget`. The
forget operation is append-only logged (auditable): it removes the document
from both axes atomically and records the prior `(authority_level,
review_state)`.

## Promote

The authority-axis-only transition `Raw → Provisional → Authority`, gated by
the promote policy plus raw evidence. **Promote never touches the review
axis** — conflating them in the CLI re-introduces Model B through the back
door. The command is `forge-core memory promote --entry-id <id> --evidence
<ref>`; it refuses to set `Authority` without a satisfied evidence gate.

## Review (Attestation)

The review-axis-only transition `Unreviewed → Reviewed`, gated by a principal
attestation. Distinct command from promote (e.g. `forge-core memory review
--entry-id <id> --reviewer <stable-id>`). The reviewer `StableId` must be
permitted to attest under the F07 `GovernancePolicy`. A record may be
reviewed without ever being promoted, and promoted without ever being
reviewed.

## AutoPromoted Anti-pattern

The legacy `ApprovalState::AutoPromoted` variant (and the YAML token
`approval: auto_promoted`) is forbidden because it violates the F06 NFR:
it lets a record reach the top of trust with no policy and no evidence —
the exact surface memory/retrieval poisoning attacks exploit (AgentPoison,
MINJA, PoisonedRAG). The variant is **deprecated, not removed** (Opção A:
zero migration cost). It is detected and failed-closed by the
`deny_auto_promoted` risk-audit rule (`contracts/risk-audits/`), a parametric
YAML detector — no Rust change required to enforce it. The canonical example
`contracts/examples/memory.yaml` must not use `auto_promoted`.

## Principal Attestation

The Forge-native expression of "a principal reviewed this", modelled as a
`StableId` (the house newtype for non-R8-sensitive ids) plus a timestamp,
recorded in the F07 governance ledger. Reusing `StableId` — not inventing a
`PrincipalId` — honours the R8 type-discipline: distinct concepts get
distinct types only when comparison would be a bug; reviewer identity is
comparable to any other `StableId`, so it reuses the newtype.

## Remaining Bootstrap Gaps

- The global Forge skill/start script now calls `forge-core project init --root <repo>` when a first-use consumer repo lacks a Project Link, unless `-NoInit` is passed.
- Product readiness still depends on verified clean install, init, project resolution, and state-bearing command flow from a consumer repo.
