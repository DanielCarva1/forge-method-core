# Forge Method Context Glossary

## Consumer Project Repo

The application, game, library, or product repository being developed with Forge Method. It owns product source code and may carry a small Forge Project Link, but it does not own Forge runtime state. Consumer repos must not use a local `<consumer>/.forge-method` state root.

## Forge Runtime Sidecar

A sibling directory or repository that owns the Forge Method runtime state for one Consumer Project Repo. It contains the real `.forge-method/` tree, including state, artifacts, evidence, ledger, stories, and claims.

## Kernel (the mutation crate)

The Rust crate that is the single source of truth for mutation. Today this is
`forge-core-kernel` (renamed from `forge-core-runtime` in ADR-0014 to make the
name match the role every other ADR and this glossary already ascribes to "the
kernel"). It owns `execute_operation` and the WAL append; every state-bearing
mutating path flows through it. Per ADR-0001, the kernel stays deterministic
and auditable; per ADR-0024, it is the sole Policy Decision Point (PDP) for
mutation — adapters and the CLI are dumb Policy Enforcement Points (PEPs).

## Decisions (the pure-function library)

The Rust crate `forge-core-decisions` (renamed from `forge-core-engine` in
ADR-0014). A library of pure, deterministic decision functions — claims
lifecycle, worktree isolation, phase-transition gates, autonomy routing,
workflow catalog, coordination evaluation, guide validation. It takes data in
and returns a verdict; **no IO, no mutable state, and no dependency on the
Kernel.** It only *decides* what should be allowed; the Kernel performs the
mutation. The two are sibling crates, not stacked layers — do not describe
Decisions as "sitting above" the Kernel.

## EventSourced trait

The generic event-sourcing abstraction in `forge-core-eventlog` (ADR-0011) that
the PEP crates (`forge-core-memory`, `forge-core-research`, `forge-core-governance`,
and the JSONL half of `forge-core-store`) implement. A domain provides its
`Event`, `Projection`, and `Diagnostic` associated types plus an `apply` fold;
this crate supplies the mechanics (cold-read `project_locked` with torn-tail
tolerance, `replay`, `next_sequence`, `append_event`, `EventLogLock`). It
collapses the **mechanics** while preserving **log separation** — each domain
keeps its own log file, lock, and projection, honoring ADR-0010's "trust domains
stay in separate logs." Associated types are plain `type` aliases (not GATs),
synchronous (no async — ADR-0001), and the `event_envelope!` macro is
`macro_rules!` (not proc-macro).

## OperationGate

The trait for mutation preconditions the Kernel runs internally before any WAL
append (ADR-0013). Implementations live in the Kernel's `builtin_gates` module:
`RiskAuditGate` (fail-closed on Error-severity risk-audit findings) and
`CitationGate` (fail-closed on unresolved `source_id`s). The Kernel calls
`evaluate(&plan)` on each gate in its chain — fail-closed, first rejection wins —
*before* staging, command evidence, or WAL append. CLI flags
(`--require-risk-audit`/`--require-citation`) become **config** for which gates
to attach, not where the check runs, so a non-CLI caller (tests, future
in-process MCP) cannot silently bypass them. Modeled on Tower's `Service`/
`Layer` pattern but synchronous (ADR-0001). A bypass exists only via
`.dangerous_unchecked()` under the `dangerous-bypass` feature flag (the rustls
`dangerous()` pattern) — never silent.

## TypedFailure

The adjacently-tagged structured failure vocabulary carried by `CliEnvelope`
alongside the human-readable `error.message` (V2.D). The mutate path's failures
(risk-audit gate, citation gate, contract parse, payload scope) used to be
stringified at the collapse site, losing variant info that a programmatic
consumer (MCP/agent) then had to re-parse out of free text. `TypedFailure` rides
alongside `error.message` so consumers branch on `typed_failure.type` instead of
parsing prose. Serialized serde **adjacently-tagged**
(`{"type": "<variant>", "data": {...}}`), never `untagged` (which loses variant
fidelity — serde issue #1307). Field/variant names are wire-stable: renaming one
is a breaking change to the envelope contract.

## DiagnosticCodeDef

The const-table entry for a diagnostic code (ADR-0012), modeled on rustc's
`Lint` struct and the `clippy`/`deno_lint`/`dprint` const-table approach. A
struct of `code` (stable snake_case `&'static str` wire identifier, e.g.
`"memory_authority_floor"`) + `description` + `category` + `default_severity`.
Declared one-per-line via the `declare_diagnostic_code!` macro (a `macro_rules!`,
not proc-macro) as a `pub static`. This is the lookup seam for config-driven
severity overrides (the ESLint/rustc lint-level model: a code declares its
*default* level, and a consumer config may promote/demote it), resolved via the
read-only `DiagnosticRegistry`. The wire-format `code` is a stable string from
the start, never the `format!("{:?}")` Debug of an enum variant — keeping type
information across the JSON/MCP boundary.

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
  F06 NFR: *promote requires policy and raw evidence — never automatic.*

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
MINJA, PoisonedRAG). The variant is **deprecated, not removed** (Option A:
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


## Agent as Bidirectional Interpreter

In the Forge model the agent — not the CLI — is the interface between a human
and the product. The CLI's stdout is a contract consumed by an agent, never
rendered directly to an end user as the primary surface. The agent translates
in both directions: it renders product state into whatever the human asked to
know, and it translates the human's intent into the Forge actions that advance
the project. Consequence: a command does not need to "decide how to talk" to
multiple audiences. There is one audience (the agent), and the command's job is
to emit a payload that gives the agent everything it needs to form the next
action or question. A legible `--no-json` text rendering may exist as a
convenience, but it is not the design centre.

## Guided Start (start)

A read-only, idempotent diagnostic command (`forge-core start`) that advances
a Consumer Project Repo from an empty state to the point where the `guide`
surface can take over. `start` never executes effects and never creates files:
it inspects the real project state (is there a Project Link? a state tree? a
first operation spec?) and emits a payload describing where the project is and
what the concrete next step is — typically `project init`, then producing a
minimal operation spec, then invoking `guide describe`. The canonical operation
contract scenarios that inform the spec live in
`docs/fixtures/operation-contract-v0/`. `start` composes with `project init`
and `guide`; it does not duplicate either. It does not recommend workflows or
phases: once a project has the prerequisites for `guide` to operate, `start`
hands off to `guide` (Option A from the F12 grill). Because `start` is
read-only, it requires no claim and performs no check-write. When `start`
diagnoses the Forge core Bootstrap Core Exception, its payload must preserve an
explicit `project resolve --allow-bootstrap-core` reference so the agent sees
the same exception context that resolution used.

## Start Bootstrap State

The discrete conditions a Consumer Project Repo can be in along the path that
`start` diagnoses. Each implies one concrete next step, and `start` emits the
matching step in its payload. The states are ordered but `start` is read-only
and recomputes from the real project on every call, so re-running after an
advance jumps to the correct state:

- **no_link** — no Project Link present. Next: `forge-core project init`.
- **link_present_no_sidecar** — link exists but the sidecar/state tree does not.
  Next: diagnose via `project resolve`, or re-`init` if the link points nowhere.
- **sidecar_ready_no_contract** — state tree is healthy but no operation spec
  exists yet. Next: the agent produces a minimal operation contract, using the
  canonical examples as structural reference (the canonical scenarios live in
  `docs/fixtures/operation-contract-v0/`); `start` points at the contract
  location and the validation command but never generates or pre-fills the spec.
  The authority boundary is the validated contract, not a template.
- **contract_present** — at least one operation spec exists. Next: `forge-core
  guide describe` (handoff to the `guide` surface).
- **preview_run** — a preview has already been produced. Next: none; the project
  is onboarded, `start` directs to `guide`/`preview`/`ready`.

## Remaining Bootstrap Gaps

- First-use bootstrap of a repo without a Project Link is the host's
  responsibility: `forge-core start --root .` emits the correct `next_step`
  (typically `project init`), and the invoking agent/host is expected to
  follow it. The core ships no global install/start script; that wiring lives
  with the operator environment.
- Product readiness still depends on verified clean install, init, project resolution, and state-bearing command flow from a consumer repo.

## Secure Protocol Adapters (F08)

A family of protocol adapters (MCP today; A2A later) that expose Forge Method
commands to external clients (Claude Desktop, other agent hosts) over stdio
JSON-RPC. The adapters are governed by ADR-0006. The inviolable rule: an
adapter is **never the source of truth** and **never mutates the store
directly** — every mutation flows through the kernel and an `OperationContract`.
The adapter is an access surface, not a second implementation of the engine.

## MCPTool

A Forge CLI command exposed as a single MCP tool. Each MCPTool is a thin
pass-through adapter with no domain logic: it maps MCP `(tool_name, arguments)`
to an argv `&[String]`, invokes the matching `CommandSpec::handler` in
`command_registry::COMMANDS`, and returns the resulting `CliEnvelope` JSON as
the tool result. The set of MCPTools is a *projection* of `COMMANDS` filtered
by the Allowlist — adding a tool never duplicates command logic, and removing
the adapter costs callers programmatic access, not functionality (the deletion
test). Read-only MCPTools (preview, ready, graph, explain, memory list,
query-effect-index) and mutate MCPTools (execute-operation, claim acquire)
share the same wrapper shape; only the gate differs.

## Command Surface

The canonical command-language module that owns what a `forge-core` command is:
its path, usage metadata, authority class, JSON/text support, and adapter
exposure. The current shared seam is the `forge-core-command-surface` crate.
`forge-core-cli::command_registry` adds handler pointers to that metadata; the
MCP adapter projects allowlist defaults and tool descriptors from it, and
`docs/generated/command-surface.md` is generated from it. The `start`, `project`,
`guide`, `mcp`, `claim`, `autonomy`, `contract`, `isolation`, `research`,
`preflight`, `graph`, `memory`, `governance`, and `coordination` CLI help
paths project their usage lines from the same seam.
Command-tree help and
unknown-subcommand hints use `CommandSpec` projection helpers for local usage
lines, concrete subcommand names, nested subcommand-path lookup, and full
subcommand usage lookup instead of re-implementing `forge-core <command>`
prefix slicing in each CLI module.
`project init` / `project resolve` parse into typed options before initializing
or resolving the Project Link. Parser/handler lookup should continue migrating
toward this Command Surface rather than growing rival hand-written lists.

The host-adapter manifest remains a narrower security adapter for host-specific
authority metadata, required contracts, safe triggers, and policy references,
but each host command is anchored by name to the Command Surface and derives
generic JSON capability from it. Host-adapter projection may expose
`canonical_usage` as non-authoritative display metadata, but command meaning
and authority remain owned by the Command Surface plus validated Forge
contracts.

## Allowlist

The explicit, named set of MCPTools a given MCP server instance is permitted
to expose, declared in `mcp-allowlist.yaml`. A tool absent from the Allowlist
is invisible to `tools/list` and rejected on `tools/call` — fail-closed. The
Allowlist is the capability surface: it separates "Forge can do X" from "this
MCP client may ask Forge to do X". It is data, not code (adding a tool to a
server requires no Rust change), mirroring the risk-audit contract model.

## MutateGate

The enforcement point at the MCP boundary where a mutate MCPTool is coupled
to an `OperationContract`. A mutate tool-call without a valid `OperationContract`
attached is rejected at the gate before the kernel is reached — fail-closed.
This is where ADR-0006's "all mutation passes through the kernel and an
OperationContract" is enforced for external callers. The MutateGate composes
with Tool-Call Attestation (proven caller) on one side and the kernel's own
`OperationContract` authorization (authorized intent) on the other; neither
alone is sufficient for a mutation.

## Tool-Call Attestation (MCP)

A detached ed25519 signature over the canonicalized tool-call intent —
`{tool_name, arguments, nonce, timestamp}` serialized with
`serde_json_canonicalizer` — carried in the `tools/call` request and verified
against a configured authorized public key. This is the protocol-boundary
proof of *who called*, the MCP/stdio analog of a signed HTTP request (stdio
carries no headers, so the signature rides in the request body, in `_meta`).
Tool-Call Attestation is **required for mutate MCPTools** and **optional for
read-only ones** under the default policy. It is distinct from F07's in-ledger
*Principal Attestation* (a reviewer's attestation on a memory record); the two
compose but do not subsume each other — distinct concepts keep distinct names.

## Knowledge Orchestration (F14)

The F14 subsistem adds a research mode in which agents produce claims that are
always traceable to auditable sources, instead of opaque synthesis. Its trust
boundary mirrors F06: citation without backing is rejected fail-closed, as
promotion without raw evidence is rejected fail-closed. Decision record:
ADR-0010.

**ResearchSource**:
A source harvested at runtime by an agent during a run (paper, URL, local
doc), registered in the Source Ledger with provenance (`fetched_at`,
`content_hash`, `harvested_by`, `trace_ref`). Distinct population of trust
from the curated Field Evidence Registry.
_Avoid_: EvidenceSource (the curated kind), Source (too generic), Reference.

**Source Ledger**:
The append-only event-sourced log (`<state_root>/research/sources.ndjson`)
that is the source of truth for `ResearchSource`s, with a rebuildable
`ResearchProjection`. Separate from the memory log (ADR-0010).
_Avoid_: Source Registry (collides with Field Evidence Registry), Source Store.

**Field Evidence Registry**:
The curated, static `ContractDocument` (already exists as
`forge-core-contracts` `FieldEvidenceRegistry`/`EvidenceSource`) that backs
**design decisions of Forge itself**, with tiered (A/B/C) sources. One of the
two citation backings, alongside the Source Ledger. Validated as part of
anchor 122.
_Avoid_: Research Registry, Evidence List.

**SourceId**:
The opaque identifier of a source, project-wide unique. Resolves against the
**union** of two backings: curated (Field Evidence Registry) ∪ runtime (Source
Ledger). A `source_id` that resolves in neither is an invalid citation
(fail-closed). The shared reuse boundary between curated and runtime sources;
distinct Source kinds share the id namespace on purpose.
_Avoid_: ref, citation-id.

**Citation**:
The edge that links a claim (any node that carries a `source_id`) to a source.
Not a claim type; a constraint imposed on claims.
_Avoid_: reference, link, attribution.

**Citation Check**:
The fail-closed validator that rejects claims whose `source_id` does not
resolve against the joint backing. Runs offline (`research check` / `validate`)
and as a runtime gate on the mutable path (mirrors the risk-audit gate
pattern). In the MVP it attests only to **resolution**, not to quality/tier.
_Avoid_: source validator, reference checker.

**Evidence Graph**:
The projection `SourceId → claims that cite it`, computed by walking existing
artifacts. **Not a first-class type and not populated by the agent** — it is a
query/index, like `forge-core-store`'s `reference_index`.
_Avoid_: Knowledge Graph (overloaded paper term), Citation Graph.

**Research claim** (deliberately absent):
There is no `ResearchClaim` type. A claim is any node that exposes a
`source_id`. F14 defines the source side and a constraint over claims, not a
new claim type — this avoids type inflation and honours the deletion test
(see ADR-0010).

**ResearchPolicy**:
The parametric contract (YAML, mirroring `MemoryPolicy`) declaring what a
source/citation admission permits (`permitted_source_kinds`,
`require_content_hash`, `require_evidence_ref_on_cite`). The "research mode"
**is** the active policy, not a flow; there is no `research run` pipeline
(anti-script-de-novela, G1).
_Avoid_: Research Config, Research Pipeline.

### Flagged ambiguities

- "evidence" carries three distinct meanings in this repo and must not be
  conflated:
  1. `evidence_ref` in `MemoryEntry.provenance` (F06) — raw evidence (test
     output, log, diff) that justifies an authority promotion.
  2. `EvidenceTier` in the Field Evidence Registry — the strength grade
     (A/B/C) of a curated source.
  3. Resolution of `source_id` in the Citation Check — whether a cited source
     is registered and reachable (F14).
  These are three orthogonal axes; none implies another. A source may be
  resolved (3) and low-tier (2); a memory may be authority (1) without being
  cited, and vice-versa.

### Relationships (F14)

- A **ResearchSource** has exactly one **SourceId** (project-unique).
- A **claim** (any node with `source_id`) cites **1..N** **SourceId**s.
- A **SourceId** resolves in exactly one backing: either a Field Evidence
  Registry `EvidenceSource` **or** a Source Ledger `ResearchSource** — never
  both. A collision is a registration error.
- **Citation Check** consults Field Evidence Registry ∪ Source Ledger; rejects
  if it resolves in neither.
- **Evidence Graph** is derived from (claims × SourceIds); **ResearchPolicy**
  governs admission on both sides of the edge.
