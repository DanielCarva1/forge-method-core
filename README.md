# Forge Method Core

**A typed-contract protocol that lets many people and many AI agents build the
same product together — without anyone managing versioning, merge conflicts, or
"who is allowed to touch what" by hand.**

Forge Method Core is a small, fast runtime written in Rust. It is not a chatbot
and it does not ship a model. It is the **governance layer and the method** that
any host agent (Codex, Cursor, Claude, OpenCode, pi.dev, VS Code, or the future
Forge app) calls through its shell. Bring your own model — or several models, run
by several different people. Forge keeps them coordinated.

### How to start (and when to start again)

The single entry point is `forge-core start`. Run it once per chat/session on
a project — it bootstraps, repairs, or routes depending on the real state on
disk:

```bash
forge-core start --root <repo> --json
```

- **Fresh repo** (no `.forge-method.yaml`): `start` creates the Project Link
  + sibling sidecar and seeds the authoritative phase record (`1-discovery`),
  so the agent gets a ready project in one command.
- **Broken sidecar** (link exists but state root missing): `start` repairs it
  idempotently. A link pointing at a non-default sidecar fails closed rather
  than silently overwriting.
- **Healthy project**: `start` reports the current bootstrap state and the
  concrete next step (typically `guide describe` or authoring the first
  operation contract).

Re-run `forge-core start` whenever you open a new chat on the project to pick
up exactly where things left off. The runtime owns the current phase
(`state.yaml`); you do not pass it on the command line.

### The orchestrator loop: `start → decide → execute`

Once bootstrapped, the agent follows a three-step loop where each command
feeds the next:

1. **`forge-core start --root <repo> --json`** — bootstraps/orients, returns
   the current phase.
2. **`forge-core guide decide --root <repo> --decision-file <intent.yaml> --json`**
   — the intelligent router. Returns the recommended workflow **and a binding
   `enforcement_policy`**: whether a claim is required (`claim_required`), the
   autonomy lane (`fast`/`rigorous`), and which gates the runtime will attach.
   The agent reads the policy and complies (acquire a claim if required).
3. **`forge-core execute-operation --root <repo> --operation <op.yaml> --json`**
   — executes the operation. The kernel runs `ClaimCoverageGate` + `PhaseGate`
   before any WAL append, enforcing the policy the guide emitted.

This is the "protocol that guarantees quality" made concrete: the agent is
guided through the best path, and the runtime refuses anything that violates
coordination — proportionally, so low-autonomy work never gets constrained.

### One command per chat (the `start-forge` skill)

For the best experience, wire the **`start-forge` skill** (`skill/start-forge/SKILL.md`)
into your host agent (Codex, Zed, Claude, Cursor, …). It collapses the flow above
into a single invocation: the agent runs `forge-core start`, and if the project is
new it performs `project init` and re-orients; if the project is in progress it
routes straight to discovery/guide. One command per chat — re-run it only when you
open a new chat on the project.

Forge ships the skill as a canonical file but **does not assume an install path**:
save it wherever your agent runtime reads skills from (common conventions include
`~/.agents/skills/` for Codex/Zed, an MCP tool, or a project-local directory).
Forge itself ships no installer and never writes to your agent's directories.

---

## The promise

You work at a company. Forty engineers each run two or three AI coding agents in
the same repository. Without rules, this is chaos: silent overwrites, duplicate
work, three agents editing the same file, broken merges, "who broke the
contract?".

Forge replaces that chaos with **one source of truth enforced at the boundary**:

- Every agent **claims** the file or story it is about to work on, before it
  writes. Two live claims cannot cover the same repo path, even if their story
  ids differ.
- Every artifact is a **typed YAML contract** — discoverable, validatable, and
  machine-readable. No prose that two agents read differently.
- The whole build — from a one-sentence idea to shipped software — runs through
  a **single method**: a funnel that starts human-heavy and becomes autonomous as
  it converges.
- The protocol is **model-agnostic by construction**: agents talk to each other
  through files on disk, not shared memory. Any LLM that can read and write YAML
  and follow the protocol is a first-class participant.

The result: **people stop worrying about versioning.** Coordination is not a
meeting or a branch policy — it is a property of the system.

### It is also a complete build method

Governance is only half of it. Forge is the **full workflow** from idea to
delivery, expressed as a typed catalog of 110 workflows spanning seven phases:

```
0-route → 1-discovery → 2-specification → 3-plan → 4-build-verify → 5-ready-operate → 6-evolve
```

A creative idea enters at `1-discovery`, gets interrogated, scoped, designed, and
broken into stories; the stories are built, verified, and shipped; the shipped
system evolves. Each phase has named entry and exit gates, so an agent never
guesses whether it is "done".

---

## How it works

### 1. Scale-with-the-model, not lock-in

Forge enforces **proportional gates** — the runtime scales enforcement to the
work's risk, never constraining the agent unnecessarily. A solo agent doing
read-only discovery moves fast (no claim required); an agent executing durable
mutation in `4-build-verify` must hold a covering claim and be past discovery.
Two `OperationGate`s run inside the kernel, before any WAL append, attached
automatically based on the project's resolved phase:

- **ClaimCoverageGate** — an `Execute`/`Repair`/`Plan` operation is refused
  when its write targets are not covered by the writer's active claim, or when
  a target is held by another agent's live claim. Low-autonomy work (Observe,
  Facilitate, Research) is never claim-gated.
- **PhaseGate** — durable mutation is refused while the project is still in
  `0-route`/`1-discovery` (the human-heavy interrogation phase).

This is the "scale with agent, never constrain" rule made executable: the
agent reads its binding policy from `guide decide` and complies; the runtime
enforces regardless. As the host model gets smarter, execution inside the
gates improves automatically — Forge never caps the model's ceiling.

### 2. Typed contracts, not documents

Everything that changes project progress is a YAML contract: discoveries, specs,
stories, claims, completion artifacts, evals. They are written **for agents to
consume**, not humans to read. This is deliberate: structured YAML is both more
accurate for models and cheaper to retrieve than markdown prose.

### 3. The funnel of autonomy

At `1-discovery` the agent iterates hard with the human — asks, challenges,
records rejections. Simple ideas skip straight to build. Complex ideas earn their
planning. As the project moves toward `4-build-verify`, the agent becomes
increasingly autonomous and may fan out to subagents. By `5-ready-operate`, the
human reviews evidence, not process.

### 4. Multi-agent governance

This is what removes the merge-conflict tax:

- **Claim acquisition** — an agent declares intent on a scope (a story, a lane, a
  path) and gets a lease with a TTL.
- **Claim coverage enforced at the write path** — the kernel's
  `ClaimCoverageGate` runs before any WAL append: a durable-mutation operation
  whose targets are not covered by the writer's active claim is refused. Two
  agents therefore cannot write the same path, because the second one cannot
  acquire a covering claim and cannot execute without one.
- **Worktree isolation** — parallel workers operate in isolated git worktrees so
  their builds never contend.
- **Coordination eval** — a gate that scores whether a session left the repo
  coherent.

### 5. Integrity by construction

Authority is non-malleable and origin-bound. State is not hand-edited; it is
**derived**. Malformed contracts fail closed rather than silently corrupting
state. A core invariant: *the protocol's own rules cannot be rewritten by an
agent at runtime.*

---

## Features

**The method (catalog of 110 typed workflows)** — discovery, brainstorming,
requirements, architecture, planning, story creation, build, adversarial review,
edge-case review, reality-evidence gates, readiness checks, traceability, and
more. Each carries `trigger`, `inputs`, `steps`, `outputs`, and the phases it
belongs to.

**The guide** — a state-aware router that, given the current phase and a decision
request, returns the workflow(s) the agent should run next. Describe the whole
catalog, decide a single step, or report phase status.

**The claim engine** — acquire, heartbeat, release, and inspect claims; check
write targets against every active claim in the shared claims directory.

**Conflict detection** — path-coverage matching that prevents overlapping work,
with root-path and Unicode-aware case handling.

**Worktree isolation** — each worker gets an isolated checkout; the protocol
governs the merge back.

**The validator** — a schema + semantic pass that rejects contracts that violate
the method's own invariants, at write time.

**Risk audit gate** — a fail-closed inspection pass that scans consumer
source for AI-induced anti-patterns (fail-soft, exception swallowing,
security slop, false tests). Rules are parametric YAML contracts
(`risk-audit-v0`) carried out of band, so adding a rule never requires a
Rust change. Anti-pattern matches are surfaced as typed diagnostics and
the command exits non-zero when any error-severity finding lands, while
still returning the full summary so agents can self-correct without
re-running.

The same gate is available as `--require-risk-audit <policy.yaml>` on
`execute-operation`, running before any WAL write so a failed audit
leaves the repository untouched.

**Cost accounting** — `forge-core cost` aggregates the cost fields already
carried by trace events (model calls, tool calls, estimated tokens) by run,
graph, agent, or principal, so a host can answer "what did this run cost?"
without re-walking the trace log.

**Supply-chain surface (host-adapter)** — a set of commands for projecting the
protocol onto host environments (MCP tools, borrowed shells, app UI) and for
verifying distribution artifacts (provenance, signatures, transparency logs).
This is the spine that lets the protocol run safely across untrusted hosts.

**Evolve-phase governance contracts** — newly typed contracts for the autonomous
loop and the fast+quality lane:

- `autonomy_policy` — declares autonomy modes, tool-class risk, and escalation
  rules for a run, phase, lane, repo, or agent role.
- `verification_goal` — captures machine-checkable evidence goals such as tests,
  lint, CI, and residual-risk status.
- `agent_run` — a run-graph for multi-agent work: workers, steps, claims,
  dependencies, and handoff status.
- `memory` — playbook / memory entries with provenance, freshness, promotion
  status, and supersession links.
- `checkpoint` — resume / rewind manifests for sessions, including WAL and git
  anchors.
- `eval_run` — outcome observability for pass/fail, latency, token/cost, and
  regression metrics.
- `telemetry` — export manifest for JSONL / OpenTelemetry-style evidence streams.

---

## Install

Pick one option. The download path needs no Rust toolchain; the build path is
for contributors and anyone who wants to compile from source.

### Option 1 — download a prebuilt binary (recommended)

Prebuilt binaries for Linux (x86_64, aarch64), macOS (Intel, Apple Silicon),
and Windows are published on every tagged release at
<https://github.com/DanielCarva1/forge-method-core/releases>.

```bash
# Linux / macOS
tar xzf forge-core-<arch>-<os>.tar.gz
install -m 0755 forge forge-core ~/.local/bin/

# Windows (PowerShell)
Expand-Archive forge-core-x86_64-windows.zip $env:LOCALAPPDATA\Programs\forge-core
# then add $env:LOCALAPPDATA\Programs\forge-core to your PATH
```

Each archive contains **both** `forge-core` (the binary) and `forge` (a thin
wrapper that delegates to `forge-core` in the same directory). The `forge`
wrapper exists so the `start-forge` skill and other tooling that look up
`forge` on PATH find it without any manual aliasing. You can use either name
interchangeably; `forge-core` is always the real binary.

Verify it landed on your PATH:

```bash
forge-core validate --root .
# forge_core_validation_passed checks=NN diagnostics=0

forge validate --root .
# same result — `forge` just delegates to `forge-core`
```

Every release asset ships with three siblings so the supply chain is auditable
end-to-end:

- `.sha256` — SHA-256 checksum (integrity)
- `.sigstore` — sigstore bundle: signature + Fulcio certificate + Rekor
  transparency-log entry (proves the asset was built by the release CI)
- `forge-core-<version>.cdx.json` — CycloneDX SBOM (transitive dependency
  inventory for CVE scanning)

**Verify integrity** (proves the bytes were not tampered in transit):

```bash
sha256sum -c forge-core-<arch>-<os>.tar.gz.sha256
```

**Verify identity** (proves the asset was built by the release CI and is
recorded in the public Rekor transparency log):

```bash
cosign verify-blob \
  --bundle forge-core-<arch>-<os>.tar.gz.sigstore \
  --certificate-identity-regexp 'https://github.com/DanielCarva1/forge-method-core/.github/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  forge-core-<arch>-<os>.tar.gz
```

**Audit dependencies** (CVE scan via [grype](https://github.com/anchore/grype)
or Trivy against the SBOM):

```bash
grype sbom:./forge-core-<version>.cdx.json
```

### Option 2 — build from source

For contributors, or if you prefer to compile yourself. You need a Rust
toolchain (1.85+, edition 2021) and `cargo`.

```bash
git clone https://github.com/DanielCarva1/forge-method-core.git forge-method-core
cd forge-method-core

# install the forge-core binary into ~/.cargo/bin
cargo install --path crates/forge-core-cli
```

Verify it landed on your PATH:

```bash
forge-core validate --root .
# forge_core_validation_passed checks=NN diagnostics=0
```

> The crate is not yet published to crates.io, so `cargo install forge-core-cli`
> (without `--path`) will not work yet. Use one of the two options above.

### Update an existing source installation

Forge changes are delivered as usable, validated commits. To update a clone
without creating an implicit merge commit, pull the latest published checkpoint
and reinstall the binary:

```bash
git pull --ff-only
cargo install --path crates/forge-core-cli --force
forge-core validate --root .
```

Users of prebuilt binaries should replace both `forge-core` and its `forge`
wrapper from the same tagged release archive. Do not mix files from different
release versions.

### What you get

A single binary, `forge-core`, plus the `contracts/` tree (the catalog, schemas,
and example claims). The binary is self-contained — no runtime, no daemon, no
network.

---

## Quick start

### Initialize a consumer project

From the repo that should be governed by Forge, run:

```bash
forge-core project init --root <repo>
```

The command creates the consumer pointer and sibling sidecar state root:

```txt
<parent>/
  <project>/
    .forge-method.yaml
  forge-<project>/
    .forge-method/
```

It must not create `<project>/.forge-method/` inside the consumer repo. The
expected pointer has the same shape as the resolved project link:

```yaml
schema_version: forge_project_link_v1
project_id: <project>
sidecar_root: ../forge-<project>
state_root: ../forge-<project>/.forge-method
```

`project init` is safe to rerun when the existing link already resolves to the
same sidecar/state root. It fails closed instead of rewriting or guessing when
`.forge-method.yaml` points somewhere else, or when the requested consumer repo
already has unsafe consumer-local state at `<project>/.forge-method`. Do not use
`--allow-bootstrap-core` for consumer projects.

Forge core itself is the temporary bootstrap exception. When bootstrapping
`<repo-root>` against its local state, commands that resolve local
`.forge-method/` state must pass `--allow-bootstrap-core`.

### Resolve the Forge runtime state

Consumer projects keep product code and Forge runtime state separate. The
product repo carries only a small `.forge-method.yaml` pointer; the real runtime
state lives in a sibling sidecar:

```txt
<parent>/
  <project>/
    .forge-method.yaml
  forge-<project>/
    .forge-method/
      state.yaml
      claims-active/
      handoffs/
        expired-claims/
      artifacts/
      evidence/
```

Example pointer:

```yaml
schema_version: forge_project_link_v1
project_id: my-project
sidecar_root: ../forge-my-project
state_root: ../forge-my-project/.forge-method
```

Acceptance rules for consumer project links:

- Prefer `forge-core project init --root <repo>` for first use; hand-written
  links are for review, fixtures, and migrations.
- `project init` is idempotent for an existing link that resolves to the same
  sidecar/state root.
- `project init` fails closed on a conflicting existing link or an unsafe
  consumer-local state root such as `<consumer>/.forge-method`.
- `state_root` must resolve under `sidecar_root` and end in `.forge-method`;
  the normal value is `<sidecar_root>/.forge-method`.
- `state_root` must not be local product-repo state like
  `<consumer>/.forge-method`. Local state is reserved for the explicit Forge
  core bootstrap exception only.
- Runtime and claim commands fail closed when the resolved state root does not
  already exist. They must not silently create consumer-local Forge state.
- `--claims-dir` remains an explicit advanced override for tests, migrations,
  and emergency repair.
- These rules keep projects, users, and agents from contaminating one another's
  Forge data.

Resolve it before work:

```bash
forge-core project resolve --root . --json
```

Raw `forge-core claim ...` commands use the same resolver by default: they
resolve the project state root from `--root .` and read or write the resolved
`claims-active/` bus and sidecar state. For ordinary consumer projects, that
means claims and handoff records land in the sidecar state directory rather than
a product-repo-local state folder. If the resolved sidecar state root is
missing, the command fails closed instead of creating `<consumer>/.forge-method`
as a fallback.

State-bearing operation/effect commands follow the same rule. By default,
`execute-operation`, `rebuild-effect-index`, and `query-effect-index` resolve
the Forge Project Link from `--root .`; operation contracts and payload files are
read relative to the consumer project root, while Forge WAL, metadata index,
evidence, and `.forge-method/artifacts/*` writes are stored under the sibling
sidecar state. A missing Project Link or missing sidecar state root fails closed.

The Forge core repository is a temporary bootstrap exception and may resolve its
local `.forge-method/` explicitly:

```bash
forge-core project resolve --root . --allow-bootstrap-core --json
```

### Ask the guide what to do

```bash
forge-core guide describe              # list every workflow in the catalog
forge-core guide status --phase 1-discovery   # what's required in this phase?
```

### Derive agent-native assurance guidance

This is a host-agent surface, not a human authoring workflow. The human states
the goal in chat; the host agent constructs the typed input, calls Forge, and
explains the result. The human does not edit YAML or choose a workflow.

```bash
forge-core assurance derive --input-file <obligation-engine-input.yaml> --json
```

The response contains the complete validated `assurance_case`, a compact
`guidance` projection, and a content-addressed `resume_token`. A host persists
the returned Assurance Case and a replacement agent can recover the same
governed state with:

```bash
forge-core assurance resume --case-file <assurance-case.yaml> --json
```

`blocked` readiness is valid guidance and therefore does not make the command
fail. It means the host should follow the ranked technical or evidence action;
Forge requests human attention only when a due irreducible Decision Request
exists. The Adapter is read-only and does not authorize project mutation.

The default read-only MCP surface also exposes the `assurance` tool. Its
order-independent flag form is suitable for pass-through adapters:

```text
forge-core assurance --input-file <path> --root <project> --json
```

### Evaluate execution admission in Rust (P4a)

P4a adds a pure policy decision point for agents integrating Forge as a Rust
library:

```rust
let decision = forge_core_decisions::evaluate_execution_admission(&document)?;
```

The request content-addresses the exact Assurance Case and
Operation/Command/Effect contracts, then binds them to trusted principal,
single-use replay, claim/gate revision, and commit-guarantee observations. Any
deterministic issue blocks admission and is returned for agent self-correction.
The executable corpus lives under `docs/fixtures/execution-admission-v0/`.

P4b now consumes this decision in the explicit trusted single-effect MCP path:
the kernel repeats mutable-authority Admission under retained locks immediately
before the effect WAL begins. Read-only remains the default, and broader
operation-wide/saga mutation is not claimed.

### Operate trusted MCP without hand-editing authority (P4b.4)

The built-in MCP surface remains read-only unless the operator explicitly
enables the exact trusted single-effect posture. The normal path is agent
operated; the human does not author a key, registry entry, snapshot, signature,
or client configuration.

```bash
forge-core mcp credential provision \
  --root <project> \
  --registry <absolute-operator-dir>/principal-registry.yaml \
  --secret-dir <absolute-operator-dir>/secrets \
  --credential-id key.agent.1 --principal-id principal.agent \
  --agent-id agent --role driver --audience forge-core:mcp:local

forge-core mcp snapshot \
  --root <project> --operation <operation-ref> --assurance <assurance-ref> \
  --principal-registry <absolute-operator-dir>/principal-registry.yaml \
  --credential-id key.agent.1 --nonce <fresh-nonce>

forge-core mcp credential sign \
  --root <project> \
  --registry <absolute-operator-dir>/principal-registry.yaml \
  --secret-dir <absolute-operator-dir>/secrets \
  --credential-id key.agent.1 \
  --snapshot runtime/mcp-execution-snapshot.yaml \
  --arguments-json <exact-tools-call-arguments.json>

forge-core mcp readiness \
  --root <project> \
  --allowlist <trusted-allowlist.yaml> \
  --principal-registry <absolute-operator-dir>/principal-registry.yaml \
  --deployment-policy <trusted-policy.yaml> \
  --snapshot runtime/mcp-execution-snapshot.yaml \
  --secret-dir <absolute-operator-dir>/secrets \
  --credential-id key.agent.1 \
  --client-config-output <absolute-operator-dir>/client-config.json
```

`mcp readiness` fails closed unless the Project Link, exact allowlist, active
credential, operator key, audience, fresh content-bound snapshot, replay WAL,
and startup reconciliation agree. Its generated JSON pins the current binary
and every trusted server argument. A replacement agent resumes by rerunning the
same readiness command from durable paths, not chat history.

The mutating `tools/call` carries `_meta.attestation` fields
`credential_id`, `audience`, `execution_intent_digest`, `nonce`, `ts`,
`signature`, and `public_key_hex`. Forge selects the authoritative public key
from the registry, checks credential status, exact audience and tool grant,
applies a 300-second age / 30-second future-skew window, and verifies the
canonical call signature against that selected key. A caller-selected key and
`AttestationPolicy::NeverRequired` cannot bypass mutation authorization.

The official `rmcp` end-to-end test builds this setup, initializes from the
generated client configuration, lists the exact `execute-operation` tool,
transports the attestation over stdio, and applies one governed sidecar effect.
It also proves that effect/replay WALs stay in the Project Link state root and
that no consumer-local `.forge-method` is created.

Read-only tool subprocesses no longer resolve `forge-core` through `PATH` or
inherit the host's full environment. The server pins its current executable,
uses the canonical repo root as cwd and `--root`, clears the environment before
copying a small OS/runtime allowlist, and disconnects child stdin from the MCP
protocol stream.

**Migration note for Rust and custom MCP consumers.** Rust consumers that
construct `McpServerConfig` with a struct literal must provide the principal
registry, mutation executor, reconciled deployment, freshness windows, pinned
binary, and root (prefer `McpServerConfig::default_read_only()` when possible).
Attestation fields remain optional on legacy read-only calls. Loading a
registry alone never enables mutation; the exact policy, snapshot, allowlist,
reconciliation proof, executor, and explicit opt-in must all agree.

### Reserve replay nonces in Rust (P4b.1b)

`forge_core_store::replay_wal` now exposes the durable replay substrate needed
by the future Execution Assurance Kernel:

- `initialize_replay_wal` creates the manifest/WAL marker pair;
- `reserve_replay_nonce` durably binds a principal/audience/nonce key to the
  canonical execution-intent and immutable commit-descriptor digests;
- `acquire_replay_commit_guard` validates that reservation while retaining the
  caller's effect-store lock, and `ReplayCommitGuard::consume` completes the
  compare-and-swap transition;
- `recover_replay_wal` verifies framing and can repair only an incomplete final
  header or payload; other corruption fails closed.

The authority files live under the supplied existing Forge state root at
`wal/replay.fmr1`, `replay-wal.manifest.json`, and
`locks/replay.wal.lock`. That root is a trust boundary: keep it outside
agent-writable project artifacts or protect it with equivalent OS permissions.
The on-disk key is an unkeyed SHA-256 hash of principal, audience, and nonce, so
it is **pseudonymous, not confidential**; guessable inputs remain guessable.

Replay is intentionally bounded to 8 MiB and 10,000 records and fails closed at
either limit (the record cap alone allows at most 5,000 completed
reserve/consume lifecycles when each uses two records; the byte cap may allow
fewer, and unconsumed reservations change that mix). There is no
compaction or rotation yet. Runtime reserve never recreates a missing pair and
a missing manifest/WAL half-pair is detected. The explicit initializer still
cannot distinguish first bootstrap from deletion or rollback of the complete
pair, so it must remain an operator-controlled action; enforced deployment also
needs an externally protected epoch/head or explicit initialization policy.
The effect-lock-first guard does not make the effect WAL and replay WAL one
physically atomic transaction. P4b.2c now closes that crash window with typed
pending receipts, a persisted pseudonymous replay binding, and deterministic
idempotent reconciliation.

This began as a **Rust API only** checkpoint. P4b.3c now consumes it only under
explicit reconciled trusted single-effect MCP deployment; read-only remains the
default and missing replay authority fails startup. See
[`contracts/spec/replay-protection-wal-v0.yaml`](contracts/spec/replay-protection-wal-v0.yaml)
and
[`contracts/spec/execution-trust-boundary-v0.yaml`](contracts/spec/execution-trust-boundary-v0.yaml)
for the exact guarantees and remaining boundaries.

### Preserve verified authority in-process (P4b.2a)

`forge-core-authority` now owns detached attestation verification, the
operator principal registry, and the opaque
`VerifiedExecutionAuthorization` capability without depending on MCP or a host
transport. The proof has private fields, no public constructor, and no
`Clone`, `Serialize`, or `Deserialize` implementation. Its audit projection is
redacted: raw nonce and signature material never leave the authority boundary.

The MCP Adapter keeps its previous attestation and registry module paths as
compatibility re-exports. The authority crate exposes the adapter-neutral
`ExecutionExecutor`, which accepts one non-cloneable `VerifiedExecutionCall`
in-process; MCP re-exports compatibility names such as `McpMutationExecutor`
and `VerifiedMcpExecutionCall`. Its structured request admits only the
operation, command/effect refs, payload bindings, risk-audit rules, and citation
requirement. Caller-selected root, sync behavior, payload escape/size limits,
transaction identity, commit timestamp, output flags, and unknown arguments
are rejected before the executor.

This seam remains inert without a reconciled activation proof. Read-only tools
retain the pinned subprocess path; trusted mutation stays in process. P4b.3c
tests prove public verified dispatch reaches the injected executor without
spawning a CLI child, while incomplete configurations fail closed. See
[`contracts/spec/execution-authority-handoff-v0.yaml`](contracts/spec/execution-authority-handoff-v0.yaml).

### Prepare and admit one execution before the effect WAL (P4b.2b)

`forge-core-kernel` now exposes an internal Rust-only preparation path that
consumes `VerifiedExecutionCall` without making authority serializable. A
`TrustedExecutionEnvironment` canonicalizes an existing project and its
Project Link resolved sidecar state root and pins the exact operator audience. The kernel,
not the adapter, derives a canonical commit descriptor covering the project,
audience, Operation/Command/Effect tokens, payload paths and hashes, effect
lock/WAL paths, transaction id, and synchronous durability.

Preparation acquires the fixed effect-store lock, runs a read-only file-effect
preflight, durably reserves the nonce, then converts the effect lock and replay
reservation into an owned effect-lock-first replay guard. At the late boundary
it repeats the preflight byte-for-byte, captures only the mutable Assurance
Case/claim/gate/state-version/time snapshot, reconstructs all principal,
replay, contract, freshness, and commit observations inside the kernel, and
runs `evaluate_execution_admission`.

An admitted result becomes a non-cloneable, non-deserializable
`LateAdmittedExecutionTransaction` that still owns both locks and the exact
Admission document. Only this typestate can enter the P4b.2c commit method. The
P4b.2b tests
prove valid admission, signed-request tamper rejection, audience separation,
claim revision blocking, filesystem drift rejection, snapshot failure, and
lock retention while creating neither a project write nor an effect-WAL file.
Preparation does record one replay reservation; failed/dropped attempts remain
seen rather than making replay authority erasable.

Claim/gate revisions are exact typed snapshots, not an OS sandbox against a
same-user bypass writer. Public MCP mutation is disabled by default and only
the P4b.3c reconciled single-effect deployment can consume this path. See
[`contracts/spec/prepared-execution-transaction-v0.yaml`](contracts/spec/prepared-execution-transaction-v0.yaml).

### Commit one admitted effect with provenance and recovery (P4b.2c)

`LateAdmittedExecutionTransaction::commit` consumes the opaque admitted value
directly. At that immediate call it repeats file preflight under the retained
effect lock, captures a new bounded Assurance/claim/gate/state/time snapshot,
and evaluates Execution Admission again. Drift or a new policy block occurs
before the first effect-WAL record.

An admitted commit canonicalizes complete evidence for verified authorization,
both Admission evaluations, all three preflights, the commit descriptor and
digest, and the replay reservation. Raw nonce values in the persisted Admission
projections are replaced with the verified nonce fingerprint. The resulting
content-addressed provenance and pseudonymous replay binding are fsynced in the
effect-WAL `begin` record before any project write. The kernel then commits
exactly one effect, consumes replay while both locks are held, releases only the
replay lock, and appends a typed `replay_consumed` marker under the effect lock.

If the process stops after effect `commit`, the durable receipt is explicitly
pending rather than safe to retry. `reconcile_prepared_execution_commits`
recovers incomplete effects, strictly verifies provenance, consumes the exact
replay reservation by key hash, and appends the missing completion marker. An
incomplete final marker is safely truncated under the effect lock and an
already-consumed exact replay is idempotent. Effect-WAL compaction retains every
provenance-bound transaction until a future governed archival boundary exists.

The two WALs remain separate files, whole replay-pair rollback detection still
needs an externally protected epoch/head, and operation-wide/saga semantics are
unsupported. This path remains Rust-only and dormant; MCP/CLI mutation and the
legacy `execute_operation` path are unchanged. See
[`contracts/spec/execution-provenance-commit-v0.yaml`](contracts/spec/execution-provenance-commit-v0.yaml).

### Route work through the dual-lane autonomy router

The flagship evolve-phase command is the risk router. It reads an
`autonomy_policy` contract, optionally reads a `verification_goal`, and returns
whether the proposed change belongs in the **fast** lane or the **rigorous**
lane:

```bash
forge-core autonomy route --policy-file <p> [--goal-file <g>] [--failure-streak <n>]
```

A manual policy fails closed to the rigorous lane:

```yaml
# policy-manual.yaml
schema_version: "0.1"
autonomy_policy_contract:
  id: p1
  applies_to:
    kind: run
    ids: [run-1]
  default_mode: manual
  tool_classes: []
  escalation:
    on_repeated_failure: 3
    on_high_risk_path: true
    on_semantic_uncertainty: true
    max_retries_before_human: 3
    cooldown_seconds: 60
  evidence_basis: null
```

```bash
forge-core autonomy route --policy-file policy-manual.yaml --no-json
# lane: rigorous
```

### Claim your work, then write

```bash
# an agent claims the file it is about to edit
forge-core claim acquire \
  --root . \
  --scope story --id my-feature \
  --agent codex-worker-1 \
  --path src/auth.rs

# before writing, prove this agent owns every target
forge-core claim check-write --root . --agent codex-worker-1 --target src/auth.rs

# ... do the work ...

# release when done
forge-core claim release --root . --id claim.story.my-feature.my-feature --agent codex-worker-1
```

`--claims-dir` is an advanced override for tests, migrations, and emergency
repair. Omit it for normal repo work so the CLI uses the resolved Forge project
state root and its sidecar `claims-active/` directory.

### Recover an expired handoff-required claim

Expired `handoff_required` claims intentionally block heartbeat, release, and
new acquire attempts for the affected scope until the abandoned context is
recorded. Do not recover by manually moving claim files. Use the official
handoff command:

```bash
forge-core claim handoff --id <claim-id-or-scope-id> --agent <id> --summary <text> [--evidence <path>...] [--root <path>] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]
```

The command records the recovery context under sidecar state
`handoffs/expired-claims/`, marks the old claim `handoff_recorded`, and reopens
the scope so a new claim can be acquired. As with other claim commands, prefer
`--root .` for normal work so Forge resolves the sidecar state; `--claims-dir`
is only the advanced override for tests, migrations, and emergency repair.

### Validate the project

```bash
forge-core project resolve --root . --json
forge-core validate              # checks every contract in the tree
forge-core execute-operation --root . --operation contracts/operations/ship.yaml
```

When running these state-bearing commands inside the Forge core repository
itself, pass `--allow-bootstrap-core`; ordinary consumer projects should not use
that flag.

All commands accept `--json` for machine consumption — that is how host agents
call them.

### Preflight — is this branch shippable?

`forge-core preflight` is the single command that answers "is this branch
shippable?" It is **project-agnostic**: it auto-detects the project profile
from manifest markers and selects the gates accordingly, so it works on a
Rust workspace, a Node service, a Python tool, or a QA-only repo without
configuration.

| Profile | Markers | Gates |
|---|---|---|
| `rust` | `Cargo.toml` | `type_check`, `format`, `clippy_pedantic`, `test`, `validate`, `regression_anchor` |
| `node` | `package.json` | `validate`, `regression_anchor` (+ custom) |
| `python` | `pyproject.toml` / `setup.py` / `requirements.txt` | `validate`, `regression_anchor` (+ custom) |
| `go` | `go.mod` | `validate`, `regression_anchor` (+ custom) |
| `generic` | none of the above | `validate`, `regression_anchor` (+ custom) |

Cargo gates (`type_check`, `clippy_pedantic`, `test`, `format`) are **skipped**
(not failed) under any non-Rust profile, so a Node or QA-only project never
reports a misleading "Cargo.toml not found" error.

```bash
forge-core preflight                    # auto-detect profile, run default gates
forge-core preflight --profile generic  # force a profile
forge-core preflight --json             # machine-readable report
```

**Custom gates.** To encode project-specific checks (API contract test, QA
package validator, report linter, secret scan, fixture/data-safety validator,
suite dry-run, …), run `forge-core preflight init` once — it writes
`.forge-method/preflight.yaml` for the detected profile — then add shell-command
gates. A gate's verdict is its exit code (0 = pass, non-zero = fail), mirroring
how `pre-commit`'s `language: system` hooks and CI runners work:

```yaml
# .forge-method/preflight.yaml
schema_version: forge_preflight_profile_v1
profile: generic
gates:
  - name: validate
    command: []
    requirement: required
  - name: api_contract_test
    command: ["npx", "my-api-cli", "test", "--suite", "contracts"]
    requirement: required
  - name: qa_package_validate
    command: ["python", "scripts/qa_validate.py"]
    requirement: required
  - name: secret_scan
    command: ["trufflehog", "filesystem", "."]
    requirement: optional
```

`forge-core preflight init [--root <path>] [--profile <name>]` writes this
file for the detected (or forced) profile with the built-in defaults; the
agent calls it during onboarding. A human never needs to edit anything by
hand for the common case — the file exists so projects can add their own
gates.

---

## Status (current)

**Proven / working today**

- The workspace verification suite is green locally: `cargo check`,
  `cargo clippy`, `cargo fmt`, and `cargo test`.
- The full 7-phase method and 110-workflow catalog.
- Claim engine, conflict detection, worktree isolation, coordination eval —
  validated end to end with parallel workers.
- Multi-agent governance on the happy path: multiple agents, disjoint files,
  coordinated by claims.
- Strict write ownership: acquire rejects overlapping live path claims, and
  `claim check-write` rejects unclaimed targets instead of treating them as
  writable by default.
- Self-hardening batch landed: TTL-overflow safety, RFC-3339 calendar
  validation, lockfile stale-owner reclaim, WAL fsync hardening, path-safety,
  symlink escape checks, and TOCTOU revalidation.
- P4b.1a trusted-principal derivation, P4b.1b bounded durable replay, P4b.2a
  opaque in-process authority handoff, P4b.2b prepared late Admission, and
  P4b.2c provenance-bound one-effect commit plus crash reconciliation are
  the internal authority substrate consumed only by explicit P4b.3c activation.
- P4b.3a adds a strict typed deployment policy: read-only is active by default,
  while a coherent trusted single-effect posture is only
  `policy_validated_dormant` and cannot enable the server.
- P4b.3b adds canonical bounded local loaders for typed execution contracts,
  risk-audit rules, complete authority snapshots, and payloads whose SHA-256
  digest is carried in the signed request. The in-process executor validates
  the bundle and still returns a dormant rejection without reserving replay or
  writing a WAL.
- P4b.3c adds explicit trusted single-effect activation. Startup resolves the
  Project Link sidecar state, verifies replay, reconciles incomplete commits,
  and binds the exact policy/root/audience/registry/allowlist before listening.
  Risk-audit and citation requirements run in the kernel before replay reserve;
  successful execution commits one effect with provenance and replay evidence.
- Dual-lane autonomy router exposed as `forge-core autonomy route` for fast vs
  rigorous lane selection.
- Seven evolve-phase governance contracts: `autonomy_policy`,
  `verification_goal`, `agent_run`, `memory`, `checkpoint`, `eval_run`, and
  `telemetry`.
- GitHub Actions CI is present for fmt, clippy, tests, and validation.

**Operational MCP proof complete**

- **MCP operational UX** — `forge-core mcp serve` supports read-only stdio by
  default and explicit reconciled single-effect mutation through P4b.3c.
  P4b.4a binds the complete mutable authority snapshot into the signed intent,
  and P4b.4b adds `forge-core mcp snapshot` to derive and atomically refresh it
  from authoritative project/sidecar state without manual YAML. P4b.4c adds
  operator-owned credential provision/rotation/revocation and in-process
  signing without emitting private keys. P4b.4d adds `forge-core mcp readiness`,
  generates the exact stdio client configuration, survives replacement-agent
  reruns, and is proven through the official `rmcp` client from initialization
  and `tools/list` through a signed applied sidecar mutation. Operation-wide,
  saga, externally anchored replay rollback detection, and hostile-user
  isolation remain intentionally absent.

**Not yet (roadmap)**
- **State derivation layer** — `forge_core_store::derive_state` is now the
  sole authority constructor for claim state, replaying the append-only WAL
  with torn-tail auto-repair. The ephemeral `claims-active/*.yaml` cache is
  no longer an authority path (inspect it via `claim status --from-cache`).
  The effect WAL is also implemented and tested. Snapshot/rotation (P3.3) is
  **shipped** — the WAL emits snapshot + checkpoint_ref + rotation records
  (record type 4) at thresholds of 64MiB / 100k records / 250ms append time,
  with archive + manifest; covered by `claim_wal_rotation_*` tests. (Earlier
  README drafts called this "a later perf layer"; that was stale — the
  correctness spine for rotation landed with the v1.1 self-healing batch.)
- **Product-ready bootstrap proof** — ✅ Proven end-to-end. A fresh consumer
  repo (`git init` + README, no `contracts/` tree) runs the full flow:
  `forge-core start` → `project init` → `project resolve` → `claim acquire` →
  `claim check-write` (owner allowed, intruder blocked) → `claim release` →
  `validate` (passes clean, 0 diagnostics) → `execute-operation` (resolves).
  Shared contract definitions are served from the binary (embedded), so a
  consumer needs no local `contracts/` tree. Covered by the regression test
  `bootstrap_consumer_e2e.rs`.


### Patch notes — evolve phase

- Dual-lane risk router: `forge-core autonomy route` returns fast vs rigorous
  lane decisions from `autonomy_policy` + optional `verification_goal`.
- Seven new governance contracts: `autonomy_policy`, `verification_goal`,
  `agent_run`, `memory`, `checkpoint`, `eval_run`, and `telemetry`.
- Multi-agent ops visibility starts with the `agent_run` run-graph contract.
- Self-evolve memory now has typed provenance, freshness, promotion, and
  supersession fields.
- Outcome observability is represented by `eval_run` and `telemetry` contracts.
- Durability hardening landed for WAL fsync, path-safety, symlink escape checks,
  and TOCTOU revalidation.
- GitHub Actions CI is included.

### Patch notes — v0.1.0

- Initial typed-contract runtime: contracts, engine, store, validator, CLI.
- 110-workflow catalog migrated and eligible.
- Guide (describe / decide / status) with router eval corpus.
- Claim engine + conflict detection + worktree isolation + coordination eval.
- Integrity spine: non-malleable, origin-bound authority; write-time rejection.
- Parallel-write proof: three concurrent workers on disjoint files, merged clean.

---

## Design principles (the locked decisions)

1. **Usable product first.** Governance is a property of building correctly, not
   the destination.
2. **Typed YAML contracts**, never prose, for every machine-consumed artifact.
3. **The guide is an intelligent, state-aware router** — not a naive embedding
   match.
4. **Scale-with-the-model** — hard gates, freedom inside them, no model shipped.
5. **Multi-LLM interop via files**, not shared memory.
6. **Funnel of autonomy** — heavy human contact early, silence late.
7. **No persona role-play** — agents are smart; give them directions and gates.
8. **The CLI is the agent boundary** — humans touch only meta commands.

---

## Repository layout

The Rust workspace layout is generated from Cargo metadata and checked in CI:
[`docs/generated/workspace-layout.md`](docs/generated/workspace-layout.md).
The command surface reference is generated from
`forge_core_command_surface::COMMANDS` and checked in CI:
[`docs/generated/command-surface.md`](docs/generated/command-surface.md).
Regenerate them with:

```bash
python scripts/generate-workspace-layout.py
python scripts/generate-workspace-layout.py --check
cargo run -p forge-core-command-surface --example generate_command_surface_docs
cargo run -p forge-core-command-surface --example generate_command_surface_docs -- --check
```

Other top-level authority surfaces:

- `contracts/workflows/` — the 110-workflow catalog (the method).
- `contracts/spec/`, `contracts/plan/`, `contracts/stories/` — typed discovery
  and delivery artifacts.
- `contracts/claims/` — claim fixtures and schemas.
- `skill/start-forge/` — single-command bootstrap skill (run once per chat).
- `docs/` — test fixtures and generated layout/command references.

The authority is always the `contracts/` tree plus the running `forge-core`
binary. Historical research digests, agent session journals, milestone plans,
and the legacy forensic reference that motivated the Rust migration live in the
sibling **[Forge-method-archive](../Forge-method-archive)** repository — they
are not runtime authority.

## License

Forge Method Core is licensed under the [Apache License, Version 2.0](LICENSE).

This license was chosen (over MIT) for its explicit patent grant
(Section 3), which protects both contributors and users from patent
litigation. See [NOTICE](NOTICE) for any third-party notices, and the
[Apache 2.0 summary](https://choosealicense.com/licenses/apache-2.0/)
for a plain-language overview.

---

*Forge Method Core is a standalone protocol and runtime. It is built to be
adopted by any agent, any team, any model — and to stay out of their way.*
