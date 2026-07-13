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
  + sibling sidecar and seeds the bootstrap compatibility phase record
  (`1-discovery`), so the agent gets a ready project in one command. The
  workflow ledger becomes P5 governance authority after `workflow init`.
- **Broken sidecar** (link exists but state root missing): `start` repairs it
  idempotently. A link pointing at a non-default sidecar fails closed rather
  than silently overwriting.
- **Healthy project**: `start` reports the current bootstrap state and returns
  structured argv for `workflow init`. Initialization is idempotent, including
  for an existing governed project; the agent then calls `workflow next`.

Re-run `forge-core start` whenever you open a new chat on the project. The host
agent executes the returned idempotent `workflow init` argv and then calls
`workflow next`; an integration that already knows the ledger is initialized
may use `workflow resume` to reconstruct the same durable guidance. Workflow,
phase, policy bundle, and readiness target are derived by the kernel; neither
the human nor the agent passes them on the command line. `state.yaml` remains a
compatibility projection and is not workflow-governance authority.

### The agent-native loop: `start → workflow init/resume → workflow next`

Once bootstrapped, the agent follows the governed loop:

1. **`forge-core start --root <repo> --json`** — bootstraps/orients, returns
   the project bootstrap state.
2. **`forge-core workflow init --root <repo> --json`** — idempotently creates
   the governed workflow ledger. In later chats use **`workflow resume`**.
3. **`forge-core workflow release-status --root <repo> --json`** — confirms
   the durable project release. If it returns `upgrade_argv`, the agent may
   execute that exact CAS-bound adjacent upgrade; installing a new binary alone
   never changes the pin.
4. **`forge-core workflow next --root <repo> --json`** — derives the current
   policy, obligations, evidence/capability gaps, decisions, and ranked next
   actions without caller-selected workflow or phase.
5. The host agent performs the returned action, records only authorized
   observations, and asks `workflow next` again. When an action needs a trusted
   repository mutation, `execute-operation` independently applies its existing
   Claim Coverage and Phase gates before any WAL append.

This makes quality governance concrete: the agent is guided through an
evidence-backed path, and the runtime refuses violations of authority it has
actually admitted. Forge reduces hidden gaps and false progress; it does not
claim that any protocol can guarantee product quality or discover every
unknown unknown.

`guide describe`, `guide status`, and `guide decide` remain available as
read-only compatibility and diagnostic surfaces for existing consumers. They
may describe eligible legacy workflows or validate a caller-authored legacy
recommendation, but they are non-authoritative for P5 workflow governance: they
cannot select the executable policy, advance phase, authorize completion, or
write the workflow ledger. New agent integrations should use `workflow
init|next|resume` instead.

### Governed domain knowledge (P6 Domain Packs)

When core does not know a product domain, it must expose the gap rather than
let a confident agent fabricate readiness. Domain Packs are closed,
content-addressed candidate inputs that contribute namespaced policies,
obligations, hazards, lifecycle models, advisory playbooks, evaluators,
fixtures, capability requirements, Adapter declarations, and provided domains.

Agents can validate an exact manifest/content pair or compose a complete
request against a sealed core binding:

```bash
forge-core domain-pack validate \
  --manifest-file <manifest.yaml> \
  --content-file <content.yaml> \
  --artifact-root <root> --json

forge-core domain-pack compose \
  --request-file <composition-request.yaml> \
  --artifact-root <root> --json
```

`validate`, `compose`, and `resolve` are read-only. Pure resolution remains
`candidate_only` and `explicitly_untrusted`; a registry-shaped YAML file cannot
claim cryptographic assurance. P6b adds `status`, `recover`, `preflight`, and `apply`. The mutating path
requires an operator-selected signed registry plus monotonic no-fork anchor
outside the project/state/artifact roots, publisher signatures, an exact lock,
every raw artifact sidecar, a fresh bounded project snapshot, default-deny
capability/sandbox policy, and compatibility recomputation under retained
locks. Only the opaque anchored TCB capability can activate one complete
record-addressed immutable generation after its exact raw objects are durable.
Operation intent is enforced: install adds its previously absent root, upgrade binds old
and target state, remove deletes an active coordinate, and rollback selects a
reachable receipt and byte-identical historical lock. Project domain requirements
live independently of packs, so removal may activate a deliberately degraded
lock that preserves explicit `missing_domain` / `missing_capability` gaps. See
`forge-core domain-pack --help` for the machine-oriented argument surface. The
reviewed real reference pack remains P6d rather than special-case Rust core
logic.

The first trust anchor requires an explicit operator provisioning ceremony;
`preflight` and `apply` never silently trust a registry on first use. The
ceremony places the crash-safe anchor beside the operator-selected registry;
it pins the exact trust-policy digest, and later invocations accept only its
direct signed successor under that same policy or a freshly reverified
exact-head replay. Keep that directory under operator control and
outside project, artifact, and `.forge-method` roots.

P6b's local filesystem boundary is cooperative between processes running as the
same OS principal. Static symlink, junction, reparse-point, traversal, and
special-file escapes fail closed, as does non-concurrent digest or protocol
state tampering. P6b does not claim isolation from a malicious process with the
same OS principal that race-replaces a validated filesystem node or mutates the
project after its final snapshot check. Use separate OS principals and
permissions, and a remote CAS where immutable artifacts cross a hostile trust
boundary, for hostile environments.

### One bootstrap command per chat (the `start-forge` skill)

For zero-config onboarding, wire the **`start-forge` skill**
(`skill/start-forge/SKILL.md`) into your host agent (Codex, Zed, Claude, Cursor,
…). It gives the agent one bootstrap invocation: run `forge-core start`, create
or repair the Project Link and sidecar when needed, and inspect the returned
state. Once the project is ready, the agent-native workflow continuation is
`workflow init` or `workflow resume`, `workflow release-status` (and its exact
returned upgrade argv when present), followed by `workflow next`. Re-run the
skill when opening a new chat; do not ask the human to operate the commands.

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

### It is growing into a complete build method

Governance is only half of the product vision. Forge preserves a frozen,
typed 110-workflow historical subject while exposing a 68-workflow operational
catalog spanning the intended path from idea to delivery across seven phases:

```
0-route → 1-discovery → 2-specification → 3-plan → 4-build-verify → 5-ready-operate → 6-evolve
```

The target experience interrogates and scopes an idea, designs and slices it,
builds and verifies representative behavior, proves readiness, and preserves
continuity after release. P5 admits 42 executable policies and retires their 42
legacy authority documents with signed deletion evidence. The remaining runtime
dispositions are 47 compatibility-only, three quarantined, and 18 future Domain
Pack candidates. Named gates reduce guesswork, but only admitted policies and
verified receipts can authorize progression or done.

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
agent reads its governed action from `workflow next`, while the execution
runtime independently derives and enforces its Claim Coverage and Phase gates.
As the host model gets smarter, execution inside the gates improves
automatically — Forge never caps the model's ceiling.

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

**The method (68 operational workflows plus a frozen 110-workflow historical
subject)** — discovery, brainstorming, requirements, architecture, planning,
story creation, build, adversarial review, edge-case review, reality-evidence
gates, readiness checks, traceability, and more. The 42 retired ids remain
available only as typed compatibility tombstones and immutable audit evidence.

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

**Domain Pack composition** — five closed schema-0.1 contract families and a
pure bounded composer validate exact raw/JCS manifest and content identities,
SemVer compatibility, dependencies, conflicts, cycles, namespaces, bilateral
whole-policy replacements, references, and
capability gaps across core plus multiple domain extensions. The neutral
two-pack/removal corpus proves deterministic composition and explicit ignorance
without changing the admitted kernel registry.

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

### Inspect the legacy guide compatibility surface

```bash
forge-core guide describe              # list every workflow in the catalog
forge-core guide status --phase 1-discovery   # what's required in this phase?
```

These read-only commands preserve existing catalog consumers and are useful for
migration diagnostics. Their caller-provided workflow/phase view is not P5
workflow-governance authority and is not the normal path for a new host agent.
Use `workflow init|next|resume` for executable agent-native guidance.

### Audit workflow migration safety (P5a)

This is an agent-facing, read-only migration surface. It inventories the frozen
110-document historical subject, classifies each one, derives stable links to future
policies/obligations/claims/playbooks/evaluators, and compares the derived
compatibility projection with the existing guide catalog:

```bash
forge-core guide migration-audit --json
```

The embedded defaults make the command usable from an installed binary; repo
maintainers can override them with `--catalog-dir` and `--plan-file`. A clean
result is `ready_for_shadow`, not permission to execute or retire workflows.
The manifest binds the complete catalog with SHA-256, requires exact legacy
projection parity, reports field-count deletion evidence, and keeps
`mutation_allowed: false` plus `retirement_allowed: false`. Catalog loss,
schema drift, projection drift, ambiguous classification, or an unsafe plan
fails closed with actionable typed issues.

The foundation classifies 15 representative golden-path workflows, 18
domain-pack candidates, and 77 compatibility-only playbooks. None of the 95
workflows outside the golden path is retirement-ready today. Procedural
`steps` remain advice. P5b supplied the closed contracts, a non-authoritative
simulation lane, and an opaque verified-kernel typestate seam. P5c connects that
seam to a trusted, receipt-backed Project Snapshot Adapter for the selected
15-policy golden path and has passed its signed-boundary, adversarial,
real-binary, and full-workspace gates. The remaining catalog is still migration
or compatibility evidence for P5d; no legacy field is retired by P5c.

### Audit a versioned catalog rollout candidate (P5d.1)

P5d.1 adds a closed release manifest, candidate-only migration batches, an
explicit quarantine/compatibility/domain disposition for every catalog entry,
and a deterministic derived scorecard:

```bash
forge-core guide rollout-audit \
  --manifest-file contracts/migration/workflow-governance-release-foundation-v0.yaml \
  --batch-file contracts/migration/workflow-governance-batch-golden-path-v0.yaml \
  --json
```

`--batch-file` is repeatable and may be absent when the manifest declares no
migration candidates. The evaluator reuses the complete P5a audit, verifies
canonical catalog/legacy/batch digests, ordered global policy composition, and
the actual embedded bytes behind representative, adversarial, and shadow
evidence references. Missing entries, compatibility shrinkage, domain leakage,
cross-batch conflicts, or unverified retirement fail closed.

Even a successful result has `authority: candidate_only`,
`evidence_assurance: content_integrity_only`, and states such as
`migration_candidate_structurally_valid`, `compatibility_only`, `quarantined`,
or `domain_pack_candidate`; it never means behaviorally sufficient,
`executable`, or `retired`. P5d.2 therefore does not trust this audit as
runtime authority; it separately admits only the exact already-proven P5c
policy set. P5d.3 adds typed behavioral evidence semantics before a genuinely
new policy batch can govern live work.

### Inspect and upgrade the project release (P5d.2)

The active release is a durable project property, not the newest artifact in
the installed binary:

```bash
forge-core workflow release-status --root <repo> --json
```

An unchanged P5c ledger returns `pin_origin: implicit_p5c_genesis` plus an
`upgrade_argv` for the sole embedded adjacent successor. The argv contains the
exact current release, ledger head, and project snapshot digests. The agent
executes it verbatim; humans do not select or edit governance files:

```bash
forge-core workflow release-upgrade \
  --root <repo> \
  --target-release-id workflow-governance.release.foundation-v0 \
  --expected-current-release-digest sha256:<64-lowercase-hex> \
  --expected-head-digest sha256:<64-lowercase-hex> \
  --expected-snapshot-digest sha256:<64-lowercase-hex> \
  --json
```

Registry, manifest, batch, bundle, and release path overrides are forbidden.
The kernel admits the fixed embedded registry through an opaque type, verifies
the target is the exact adjacent predecessor-bound successor, and appends one
hash-chained transition under the ledger lock. Stale CAS values do not modify
the WAL; replay after success returns `already_pinned` without appending.
`workflow next|resume` then reports the same target release to any replacement
agent.

P5d.2's foundation runtime bundle has a new identity but exactly the same 15
policy objects as P5c. The other 95 catalog workflows remain non-executable.
Interrupted Windows WAL replacement reconciles to the exact old or committed
file and fails closed on ambiguous/corrupt protocol state. This does not add an
external anti-rollback anchor or claim signature-based release supply-chain
trust.

### Audit the first reviewed candidate batch (P5d.3)

P5d.3 compiles five non-golden workflows (`adversarial-review`,
`risk-register`, `code-review`, `traceability-gate`, and
`nfr-evidence-audit`) from a typed policy overlay. It also keeps
`edge-case-review`, `track-decision`, and `release-readiness` in explicit
quarantine. Neither group is added to the opaque release registry, so
`workflow release-status` exposes no new successor and existing projects keep
their P5d.2 pin.

Recompute the candidate evidence and composition from exact repository bytes:

```bash
cargo run -p forge-core-decisions --example generate_workflow_core_assurance_evidence -- --check
cargo run -p forge-core-decisions --example generate_workflow_core_assurance_candidate -- --check
forge-core guide rollout-audit \
  --manifest-file contracts/migration/workflow-governance-release-core-assurance-candidate-v0.yaml \
  --batch-file contracts/migration/workflow-governance-batch-golden-path-v0.yaml \
  --batch-file contracts/migration/workflow-governance-batch-core-assurance-v0.yaml \
  --json
```

The derived shadow corpus contains exactly 35 scenarios: positive, negative,
ambiguity, false-completion, stale-evidence, resume, and ablation for each of
the five workflows. The checked-in report has zero mismatches/evaluation
errors and can produce only `behaviorally_consistent_candidate` with
`review_candidate` and `non_authoritative_shadow_evidence`. This proves exact
deterministic consistency, not independent semantic approval. P5d.4 must bind
an independent review to the final subject, corpus, report, batch, and manifest
before the kernel can admit a new policy set.

### Upgrade to the independently reviewed core-assurance release (P5d.4a)

P5d.4a closes that boundary. A content-addressed Review Index binds the exact
P5d.3 subject, corpora, shadow report, evaluator source, frozen history,
candidate/final bundles, and append-only registry. Two Ed25519 signatures from
distinct semantic-reviewer and release-authorizer principals, credentials, and
public keys authorize the same closed payload. The YAML documents remain
`candidate_only`; only the fixed kernel loader can combine a recomputed
`ready_for_independent_authorization` evaluation with the opaque verified
capability.

```bash
cargo run -p forge-core-decisions --example generate_workflow_core_assurance_admission -- --check
forge-core workflow release-status --root <project> --json
# execute the returned upgrade_argv: P5c -> foundation, then foundation -> core-assurance
forge-core workflow release-status --root <project> --json
```

The admitted successor contains the original fifteen policies plus the five
reviewed core-assurance policies. Because the policy set changes, the adjacent
upgrade is `invalidate_all`: old evidence/receipts and prepared completion
authority cannot cross the transition. `workflow resume` deterministically
returns the new pin, while `edge-case-review`, `track-decision`, and
`release-readiness` remain quarantined and absent from runtime routing. Forge
proves cryptographic role separation, not organizational independence; separate
key custody remains an explicit release-operation requirement.

### Continue with assurance-operations through sequential V2 admission (P5d.4b.1)

P5d.4b.1 keeps every P5d.4a V1 byte, digest, signature, and authorization
contract frozen. Later releases use the generic schema-0.2 Review Index and a
release-specific signed payload. The trusted loader starts with the historical
V1 admission and then verifies, consumes, and admits each V2 release in order;
a missing or invalid later authorization fails the complete load instead of
exposing a partially advanced registry.

The first P5d.4b batch adds thirteen assurance, quality, security, privacy,
compliance, observability, deployment, platform-operations, investigation, and
testing policies. Its two corpora prove seven scenario kinds for every workflow
(91 scenarios total), including resume and load-bearing ablation. The resulting
registry contains four adjacent releases and 33 executable policies. The full
110-workflow accounting is 33 migration candidates, 56 compatibility-only,
three quarantined, and 18 domain-pack candidates.

```bash
cargo run -p forge-core-decisions --example generate_workflow_assurance_operations_evidence -- --check
cargo run -p forge-core-decisions --example generate_workflow_assurance_operations_candidate -- --check
cargo run -p forge-core-decisions --example generate_workflow_assurance_operations_admission -- --check
forge-core validate --root .
```

Repository validation retains the frozen 139-check P5d.4a anchor and adds the
Batch A surfaces for a 156-check checkpoint.

### Complete core rollout with agent-native continuity (P5d.4b.2)

P5d.4b.2 applies the same sequential V2 boundary to `checkpoint-preview`,
`collaboration-handoff`, `research-closeout`, `retrospective`, `sprint-status`,
`project-context`, `spec-distillation`, `evolve-project`, and
`product-area-map`. These policies govern durable truth, handoff ownership,
evidence-linked learning, replacement-agent context recovery, product-area
mapping, and safe project evolution without prescribing speech or granting the
agent authority to approve its own claims.

```bash
cargo run -p forge-core-decisions --example generate_workflow_agent_native_continuity_evidence -- --check
cargo run -p forge-core-decisions --example generate_workflow_agent_native_continuity_candidate -- --check
cargo run -p forge-core-decisions --example generate_workflow_agent_native_continuity_admission -- --check
forge-core validate --root .
```

The release adds 63 scenarios and produces a five-release, 42-policy registry
while preserving the exact 33-policy predecessor prefix. P5d.4b is complete at
42 migration candidates, 47 compatibility-only workflows, three quarantines,
and 18 domain-pack candidates. The 47 compatibility-only workflows remain
explicitly non-executable, the quarantines remain absent from runtime, and the
18 domain workflows remain reserved for P6. Validation now preserves anchors
of 139 historical checks, 156 through Batch A, and 169 pre-retirement checks.

### Retire legacy authority with signed deletion evidence (P5d.5)

P5d.5 keeps a byte-identical evidence-only snapshot of all 110 legacy workflow
documents and removes exactly the 42 admitted replacements from the operational
catalog. Historical P5d review still recomputes against the frozen subject;
agent routing sees only the 68 non-retired workflows. A retired id never becomes
unknown or routable: `guide describe|status|decide` returns a typed tombstone
with its replacement policy, release, and argv.

```bash
cargo run -p forge-core-decisions --example generate_workflow_retirement_checkpoint -- --check
forge-core validate --root .
forge-core workflow retirement-status --root . --json
```

The checkpoint derives routing, readiness, verdict, receipt, and continuation
digests from the exact replacement policies; binds a repository compatibility
matrix and typed diagnostics; requires two independent Ed25519 roles; and is
held by a kernel-owned opaque capability. Caller/project artifact overrides do
not select retirement authority, and the status command is read-only. The final
scorecard deliberately has two axes: runtime is `42 executable / 47
compatibility-only / 3 quarantined / 18 domain-pack`, while legacy authority is
`42 retired / 68 retained`. Validation preserves the 169-check pre-retirement
anchor and passes 170 aggregate checks. P5 is complete; P6a Domain Pack contract
and composition now builds on that frozen authority boundary without changing it.

Runtime proof is intentionally split rather than overstated. The retirement
runtime-evidence test reexecutes 189 frozen P5d.4 scenarios for the 27 policies
added after the base release and requires exact report, outcome, and digest
equality in the promoted admitted runtime. The separate golden-path suite proves
the 15 base policies through signed authority, readiness, completion receipts,
and replacement-agent continuation. Their union is the admitted 42-policy set;
neither test alone claims an all-42 adapter path. The operational routing catalog
remains a separate 68-workflow surface.

P5d.5 is the pullable `0.5.0` package checkpoint. Consumers that read the
extended guide `describe`/`status` payload must support guide payload schema
`0.2`; the minimum compatible Forge package/consumer version for the retirement
surface is `0.5.0`. Workflow governance release identity intentionally remains
`0.4.0`: the package checkpoint does not rewrite the frozen five-release chain.

P6a is the pullable `0.6.0` package checkpoint. It adds Domain Pack schema
`0.1`, pure deterministic candidate composition, the neutral multi-pack/removal
corpus, and the read-only agent CLI. It does not change the guide payload's
minimum `0.5.0` retirement compatibility, rewrite workflow release identity,
or claim pack lifecycle/trust authority.

### Simulate workflow governance (P5b)

P5b adds a pure Workflow Governance Kernel Module with two deliberately
different lanes. The public CLI accepts caller-authored YAML only for
`simulation_only` planning. It derives candidate eligibility, progression,
claim/obligation state, completion, Capability Gaps, Decision Requests, and
ranked next actions without granting those candidates runtime authority:

```bash
forge-core guide govern-simulate \
  --bundle-file contracts/workflow-governance/kernel-v0.yaml \
  --input-file docs/fixtures/workflow-governance-kernel-v0/complete.yaml \
  --legacy-workflow-file contracts/evidence/workflow-retirement/legacy-catalog/build-story.yaml \
  --json
```

`active`, `blocked`, `complete`, and `ineligible` are candidate simulation
outcomes rather than command failures. Structural policy/input defects fail
closed, but even a candidate `complete` result cannot unlock progression,
completion, mutation, or Execution Admission. Caller-authored phase, evidence,
capability, decision, and freshness fields are proposals, never provenance.

The second lane accepts only an opaque trusted snapshot and returns an opaque
verified decision. It has no YAML/JSON constructor. Its authority inputs must
be derived from the Project Snapshot Adapter: the repository-owned admitted
policy bundle is loaded inside the Adapter/kernel and bound by canonical digest,
never caller-selected; phase and state come from the durable project snapshot;
prerequisite completion from completion receipts; capability availability from
registry/probe receipts; human decisions from authorized decision receipts;
evaluator outcomes from evidence receipts with provenance; and freshness from
trusted receipt and observation times rather than caller assertions. P5b
established this typestate boundary; P5c implements and integrates the live
Adapter and receipt sources for the selected golden path.

Playbooks remain available as flexible strategy. They have no authority field,
and deletion/replacement tests prove that changing their steps cannot change
candidate eligibility, progression, obligations, claims, or completion. The
optional legacy simulation Adapter preserves the existing `CatalogEntry` while
marking its projected status and blockers as compatibility simulation only.
The P5b simulation Module still performs no IO or project mutation. Runtime
authority enters only through the P5c Adapter described below.

### Exercise the executable golden path (P5c)

P5c is the completed agent-facing golden-path workflow-governance checkpoint. The human continues to
describe goals, answer value judgments, and review results in chat; the host
agent operates Forge. Neither party selects a workflow, phase, policy bundle,
or readiness target. These commands describe the published P5c integration surface:

```bash
forge-core workflow init --root <repo> --json
forge-core workflow next --root <repo> --json
forge-core workflow resume --root <repo> --json
forge-core workflow shadow --root <repo> --json
```

`next` is designed to route deterministically across the admitted 15-policy golden path and
returns obligations, gaps, Decision Requests, Capability Gaps, evidence state,
and ranked next actions. `resume` derives the same governed state from the
ledger for a replacement agent; chat history is not authority. `shadow` compares
the migrated and legacy projection for the same snapshot and always reports
`mutation_allowed: false` and `retirement_allowed: false`.

Authority-bearing observations use exact signed request/attestation pairs rather
than caller-selected facts or direct status editing:

```bash
forge-core workflow applicability-authorize --root <repo> \
  --request-file <signed-applicability-request.json> \
  --attestation-file <attestation.json> --json
forge-core workflow capability-authorize --root <repo> \
  --request-file <signed-capability-request.json> \
  --attestation-file <attestation.json> --json
forge-core workflow evidence-authorize --root <repo> \
  --request-file <signed-evidence-request.json> \
  --attestation-file <attestation.json> --json
forge-core workflow signal-authorize --root <repo> \
  --request-file <signed-signal-request.json> \
  --attestation-file <attestation.json> --json
forge-core workflow decision-resolve --root <repo> \
  --request-file <signed-decision-request.json> \
  --attestation-file <attestation.json> --json
forge-core workflow waiver-authorize --root <repo> \
  --request-file <signed-waiver-request.json> \
  --attestation-file <attestation.json> --json
```

The Adapter loads the operator-owned workflow principal registry from its fixed
state-sidecar location; the CLI cannot substitute a preferred registry. Signed
applicability, capability, evidence, signal, decision, and waiver intents bind the
project, admitted bundle, policy, state/snapshot, ledger head, authority scope,
and observation subject required by that action. Serialized request or receipt
data alone cannot mint authority. Representative-execution, readiness, and
release claims are non-waivable.

When guidance reaches `ready_to_complete`, the agent consumes it with the exact
snapshot returned by `next`:

```bash
forge-core workflow complete --root <repo> --if-snapshot <sha256> --json
```

The completion path re-locks and rechecks the admitted bundle digest, project snapshot,
ledger head, state version, phase, selected policy, target, and current evidence
before atomically appending completion, phase, and continuity receipts. The
adversarial and full-workspace gates prove this behavior end to end.

The intended authoritative history is the state-root-confined, fsynced, hash-chained
`wal/workflow-governance.ndjson`; `state.yaml` is compatibility-only. The chain
detects record tampering, malformed/torn tails, sequence gaps, and mismatched
heads inside the history it receives. It cannot distinguish a clean truncation
to a previously valid prefix from legitimate history, and P5c has no external
monotonic anchor for this ledger. A malicious same-user rollback of the entire
internally consistent ledger therefore remains outside the threat boundary.
This checkpoint targets the local CLI path; command-surface allowlist
metadata is not an end-to-end MCP workflow Adapter, and hostile-user isolation
is not claimed.
The released CLI confines raw workflow-ledger mutation to the dedicated
`forge-core-workflow-governance-tcb` crate, which is a direct dependency only
of the kernel Adapter. `forge-core-store`, the CLI, and MCP expose no semantic
append API, so Cargo feature unification cannot accidentally widen that
boundary. Direct same-user writes to the state root remain outside P5c's
process-isolation guarantee.

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

P4b consumes this decision in explicit trusted MCP paths: the kernel repeats
mutable-authority Admission under retained locks immediately before the effect
WAL begins. P4b.6c activates verified multi-effect `operation_wide_wal`
execution behind a separate typed policy and operator flag. Read-only remains
the default, exact single-effect remains compatible, and saga execution is not claimed.

### Compose one local transaction for multiple effects (P4b.6a)

`forge-core-kernel` can now derive an `OperationEffectBundle` from an operation's
complete ordered file-backed effect set:

```rust
let bundle = forge_core_kernel::compose_operation_effect_bundle(
    effect_store_root,
    &operation,
    &effect_refs,
    &effects,
)?;

let result = forge_core_kernel::apply_operation_effect_bundle_with_wal_lock(
    effect_store_root,
    &bundle,
    &payloads,
    ".forge-method/wal/effects.ndjson",
    ".forge-method/locks/effects.lock",
    transaction_id,
);
```

The Module validates the operation and every effect, binds each document to its
declared ref, resolves logical targets through the same physical mapping used
by the store, rejects overlapping aliases, and creates one internal
`operation_transaction` envelope. Applying that envelope uses one effect lock,
one WAL Begin/Commit, and before-images for the complete write set; both an
immediate later-write failure and crash recovery roll back all constituent
writes. Original effect ids and refs remain available for provenance.

The legacy runtime rejects multiple independently committed effects before any
side effect. P4b.6b consumes this substrate inside the opaque prepared kernel;
P4b.6c adds bounded MCP loading, snapshots, signing, readiness, and official
client activation for the complete ordered effect set. See
[`contracts/spec/operation-wide-transaction-v0.yaml`](contracts/spec/operation-wide-transaction-v0.yaml).

### Operate trusted MCP without hand-editing authority (P4b.4 + P4b.5)

The built-in MCP surface remains read-only unless the operator explicitly
enables an exact trusted scope: `--enable-trusted-single-effect` or the separate
`--enable-trusted-operation-wide`. The validated policy must match that flag.
The normal path is agent operated; the human does not author a key, registry
entry, snapshot, signature, or client configuration.

```bash
forge-core mcp credential provision \
  --root <project> \
  --registry <absolute-operator-dir>/principal-registry.yaml \
  --secret-dir <absolute-operator-dir>/secrets \
  --credential-id key.agent.1 --principal-id principal.agent \
  --agent-id agent --role driver --audience forge-core:mcp:local

forge-core mcp replay-anchor provision \
  --root <project> \
  --anchor <absolute-operator-dir>/replay-anchor.json \
  --deployment-id <trusted-policy-id>

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
  --replay-anchor <absolute-operator-dir>/replay-anchor.json \
  --secret-dir <absolute-operator-dir>/secrets \
  --credential-id key.agent.1 \
  --client-config-output <absolute-operator-dir>/client-config.json
```

`mcp readiness` fails closed unless the Project Link, exact allowlist, active
credential, operator key, audience, fresh content-bound snapshot, replay WAL,
external replay anchor, and startup reconciliation agree. Its generated JSON
pins the current binary and every trusted server argument. A replacement agent
resumes by rerunning the same readiness command from durable paths, not chat
history.

The replay anchor is strict bounded JSON in an operator-managed directory
outside both project and Forge state. It binds the deployment-policy id, a
random epoch, monotonic generation, manifest digest, and exact trusted WAL byte
prefix. Trusted startup and every mutation verify/advance it automatically;
restoring an older complete replay pair therefore fails closed while the
external anchor survives. Provisioning is trust-on-first-use, and an actor able
to roll back **both** stores remains outside this cooperative same-user
guarantee. Use independent OS permissions, remote compare-and-swap storage, or
equivalent isolation when that attacker is in scope.

P4b.5b carries the registry-verified `ExecutionPrincipal` tuple
(`principal_id`, `agent_id`, and `role`) through the complete governed path.
Required execution claims must match all three fields; the effect-WAL Begin
binds the tuple in canonical provenance; recovery cross-checks it against the
authorization audit; and a durable `effect_staged` trace is written before the
effect transaction begins. Applied kernel/MCP receipts expose both the
principal and trace event id. The portable tuple is evidence, not authority:
only the opaque registry-derived authorization capability can permit mutation.

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
It also proves that effect/replay WALs stay in the Project Link state root, the
external head advances through replay consume, and no consumer-local
`.forge-method` is created. The proof also checks that the receipt and durable
trace contain the exact registry principal rather than only an agent label.

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

The WAL authority files live under the supplied existing Forge state root at
`wal/replay.fmr1`, `replay-wal.manifest.json`, and
`locks/replay.wal.lock`. That root is a trust boundary: keep it outside
agent-writable project artifacts or protect it with equivalent OS permissions.
The on-disk key is an unkeyed SHA-256 hash of principal, audience, and nonce, so
it is **pseudonymous, not confidential**; guessable inputs remain guessable.

Replay is intentionally bounded to 8 MiB and 10,000 records and fails closed at
either limit (the record cap alone allows at most 5,000 completed
reserve/consume lifecycles when each uses two records; the byte cap may allow
fewer, and unconsumed reservations change that mix). There is no compaction or
rotation yet. Runtime reserve never recreates a missing pair and a missing
manifest/WAL half-pair is detected. The explicit initializer alone still cannot
distinguish first bootstrap from deletion or rollback of the complete pair, so
it remains operator-controlled. P4b.5a trusted MCP now requires the external
head described above and detects rollback relative to that surviving head.
The effect-lock-first guard does not make the effect WAL and replay WAL one
physically atomic transaction. P4b.2c now closes that crash window with typed
pending receipts, a persisted pseudonymous replay binding, and deterministic
idempotent reconciliation.

This began as a **Rust API only** checkpoint. P4b.3c now consumes it only under
explicit reconciled trusted single-effect MCP deployment; read-only remains the
default and missing replay authority fails startup. See
[`contracts/spec/replay-protection-wal-v0.yaml`](contracts/spec/replay-protection-wal-v0.yaml)
and
[`contracts/spec/replay-external-anchor-v0.yaml`](contracts/spec/replay-external-anchor-v0.yaml)
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
operation, ordered command/effect refs, payload bindings, risk-audit rules, and citation
requirement. Caller-selected root, sync behavior, payload escape/size limits,
transaction identity, commit timestamp, output flags, and unknown arguments
are rejected before the executor.

This seam remains inert without a reconciled activation proof. Read-only tools
retain the pinned subprocess path; trusted mutation stays in process. P4b.3c
tests prove public verified dispatch reaches the injected executor without
spawning a CLI child, while incomplete configurations fail closed. See
[`contracts/spec/execution-authority-handoff-v0.yaml`](contracts/spec/execution-authority-handoff-v0.yaml).

### Prepare and admit one transaction before the effect WAL (P4b.2b + P4b.6b)

`forge-core-kernel` now exposes an internal Rust-only preparation path that
consumes `VerifiedExecutionCall` without making authority serializable. A
`TrustedExecutionEnvironment` canonicalizes an existing project and its
Project Link resolved sidecar state root and pins the exact operator audience. The kernel,
not the adapter, derives a canonical commit descriptor covering the project,
audience, Operation/Command/Effect tokens, payload paths and hashes, effect
lock/WAL paths, transaction id, and synchronous durability.

P4b.6b extends the opaque request and material boundary to a complete ordered
effect set. The kernel matches every source ref to its content-addressed token,
the operation declaration, loaded document, and union of payload targets before
it acquires the lock or reserves replay. Two or more local effects are compiled
into the internal operation-wide envelope; single-effect construction remains
source-compatible.

Preparation acquires the fixed effect-store lock, runs a read-only file-effect
preflight, durably reserves the nonce, then converts the effect lock and replay
reservation into an owned effect-lock-first replay guard. At the late boundary
it repeats the preflight byte-for-byte, captures only the mutable Assurance
Case/claim/gate/state-version/time snapshot, reconstructs all principal,
replay, contract, freshness, and commit observations inside the kernel, and
runs `evaluate_execution_admission`. The derived commit facts are
`single_effect_wal`/`single_effect` or
`operation_wide_wal`/`whole_operation`, never caller-selected.

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
same-user bypass writer. Public MCP mutation is disabled by default. P4b.3c's
single-effect and P4b.6c's operation-wide deployments consume this path only
after scope-specific policy validation, explicit opt-in, and reconciliation. See
[`contracts/spec/prepared-execution-transaction-v0.yaml`](contracts/spec/prepared-execution-transaction-v0.yaml).

### Commit one admitted transaction with provenance and recovery (P4b.2c + P4b.6b)

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
effect-WAL `begin` record before any project write. For operation-wide commits,
the descriptor binds the derived envelope plus every original ref, id, and
token in declared order. The kernel commits one envelope, consumes replay while
both locks are held, releases only the replay lock, and appends a typed
`replay_consumed` marker under the effect lock. The receipt exposes the envelope
id and ordered constituent ids; the principal trace lists every source effect.

If the process stops after effect `commit`, the durable receipt is explicitly
pending rather than safe to retry. `reconcile_prepared_execution_commits`
recovers incomplete effects, strictly verifies provenance, consumes the exact
replay reservation by key hash, and appends the missing completion marker. An
incomplete final marker is safely truncated under the effect lock and an
already-consumed exact replay is idempotent. Effect-WAL compaction retains every
provenance-bound transaction until a future governed archival boundary exists.

The two WALs remain separate files. P4b.5a trusted MCP detects whole replay-pair
rollback relative to its independently protected external head, but cannot
detect coordinated rollback of both stores. P4b.6c exposes the operation-wide
prepared kernel through exact trusted MCP policy and official-client evidence;
saga semantics remain unsupported. See
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
  --principal-id principal.codex-worker-1 \
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

`--principal-id` preserves the verified actor across claim WAL projections and
conflict evidence. It is optional only to keep historical claims readable;
trusted MCP execution fails closed when its required claim lacks the exact
verified principal, agent, and role tuple.

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
- The complete 7-phase intent map and 110-workflow legacy catalog. Runtime
  authority is narrower and listed explicitly below; catalog presence is not
  executable coverage.
- P5a read-only workflow migration foundation: complete deterministic manifest,
  15 golden-path + 18 domain-pack-candidate + 77 compatibility classifications,
  exact 110/110 legacy projection parity, full-catalog deletion binding, and no
  runtime mutation or retirement authority.
- P5b workflow governance boundary: closed policy/evaluation contracts,
  semantic and dependency-graph validation, an explicit `simulation_only`
  CLI, deterministic candidate gaps/actions, and a non-deserializable opaque
  verified-kernel typestate seam.
- P5c executable golden path: a repository-owned 15-policy admitted bundle,
  trusted receipt-backed Project Snapshot Adapter, confined hash-chained
  governance ledger, signed applicability/capability/evidence/decision/waiver
  authority, late completion recheck, read-only legacy shadow comparison, and
  replacement-agent resume, isolated ledger TCB, atomic multi-record commits,
  and adversarial plus full-workspace proof. P5d subsequently completed the
  remaining reviewed core rollout and safe legacy retirement.
- P5d.1 versioned release foundation: closed release/batch/retirement proposal
  contracts, one explicit disposition per workflow, a canonical generated
  110-entry foundation manifest and 15-policy candidate batch, real embedded
  content/digest verification, aggregate repository validation, and
  `guide rollout-audit`. Its scorecard is structural and `candidate_only` with
  `content_integrity_only`; it does not activate a release or retire anything.
- P5d.2 opaque release admission and project pinning: a fixed embedded
  candidate registry is elevated only through a non-serializable kernel loader
  after exact P5c policy-set equivalence; unchanged P5c ledgers map to an
  implicit genesis release, and one CAS-bound, crash-recoverable ledger event
  moves them to the adjacent foundation release. Status/init/next/resume expose
  the same durable pin, local overrides are ignored, and no new catalog policy
  is admitted.
- P5d.3 first reviewed candidate batch: five typed core-assurance overlays,
  three explicit quarantines, an acyclic content-addressed review subject, and
  35 recomputable governed-outcome scenarios now derive a candidate-only
  shadow report. Append-only registry evolution and frozen upgraded-WAL tests
  protect predecessor history, while the live admission registry remains byte
  unchanged.
- P5d.4a independently authorizes the frozen five-policy candidate and admits
  the append-only 20-policy third release through an opaque kernel capability.
- P5d.4b.1 preserves that V1 path while sequential V2 admission adds the
  13-policy assurance-operations release. The admitted registry now has four
  releases and 33 policies; 91 scenarios and 156 aggregate validation checks
  pass with catalog accounting fixed at 33/56/3/18.
- P5d.4b.2 completes reviewed core rollout with nine agent-native continuity
  policies, 63 scenarios, and a five-release 42-policy registry. Final P5d.4b
  accounting is 42/47/3/18; compatibility-only, quarantined, and P6 workflows
  remain non-executable.
- P5d.5 completes P5 with a frozen 110-workflow evidence archive, a
  68-workflow operational catalog, exact all-42 signed retirement, policy-derived
  five-surface deletion proof, repository consumer fixtures, typed tombstones,
  opaque kernel admission, and the final 42/47/3/18 plus 42/68 scorecard.
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
- P4b.3a/P4b.6c provide strict typed deployment policy: read-only is active by
  default, while coherent trusted single-effect and operation-wide postures are
  `policy_validated_dormant` and cannot enable the server without matching opt-in.
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
  default, explicit reconciled single-effect mutation through P4b.3c, and
  scope-separated operation-wide mutation through P4b.6c.
  P4b.4a binds the complete mutable authority snapshot into the signed intent,
  and P4b.4b adds `forge-core mcp snapshot` to derive and atomically refresh it
  from authoritative project/sidecar state without manual YAML. P4b.4c adds
  operator-owned credential provision/rotation/revocation and in-process
  signing without emitting private keys. P4b.4d adds `forge-core mcp readiness`,
  generates the exact stdio client configuration, survives replacement-agent
  reruns, and is proven through the official `rmcp` client from initialization
  and `tools/list` through a signed applied sidecar mutation. P4b.5a adds the
  required operator-protected external replay head, policy identity binding,
  automatic startup/request advancement, and whole-pair rollback detection.
  P4b.5b propagates the verified Execution Principal through claims, conflict
  attribution, Admission, effect-WAL provenance, recovery, durable traces,
  receipts, and MCP evidence. P4b.6c adds complete ordered loading, snapshots,
  exact signing, readiness, generated config, and an official `rmcp` two-effect
  atomic sidecar proof. Saga and hostile-user isolation remain intentionally absent.

**Not yet (roadmap)**
- **P6 Domain Pack ecosystem** -- P5 is complete. The 18 domain candidates
  remain outside core authority until closed pack contracts, deterministic
  composition, provenance, compatibility, lifecycle, and conflict handling are
  implemented and independently reviewed.
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
- Historical 110-workflow catalog migrated and eligible at the time of v0.1.0;
  P5d.5 later froze that subject and reduced current routing to 68 workflows.
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

- `contracts/workflows/` — the 68-workflow operational catalog used by routing.
- `contracts/evidence/workflow-retirement/legacy-catalog/` — the immutable
  110-workflow historical audit subject; never a routing fallback.
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
