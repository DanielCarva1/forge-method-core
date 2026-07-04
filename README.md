# Forge Method Core

**A typed-contract protocol that lets many people and many AI agents build the
same product together — without anyone managing versioning, merge conflicts, or
"who is allowed to touch what" by hand.**

Forge Method Core is a small, fast runtime written in Rust. It is not a chatbot
and it does not ship a model. It is the **governance layer and the method** that
any host agent (Codex, Cursor, Claude, OpenCode, pi.dev, VS Code, or the future
Forge app) calls through its shell. Bring your own model — or several models, run
by several different people. Forge keeps them coordinated.

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

Forge enforces **hard gates** (you cannot skip a phase, you cannot write a file
you have not claimed, a contract must be valid before it is authority) and
otherwise gives the agent **freedom within those gates**. No persona scripts, no
rigid step-by-step theater. As the host model gets smarter, execution inside the
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
- **Conflict detection** — before any write, the agent checks whether its own
  active claim covers every target path. Peer-claimed and unclaimed writes are
  refused.
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
<https://github.com/Stable-Studio/forge-method-rust/releases>.

```bash
# Linux / macOS
tar xzf forge-core-<arch>-<os>.tar.gz
install -m 0755 forge-core ~/.local/bin/

# Windows (PowerShell)
Expand-Archive forge-core-x86_64-windows.zip $env:LOCALAPPDATA\Programs\forge-core
# then add $env:LOCALAPPDATA\Programs\forge-core to your PATH
```

Verify it landed on your PATH:

```bash
forge-core validate --root .
# forge_core_validation_passed checks=NN diagnostics=0
```

Every release asset ships with three siblings so the supply chain is auditable
end-to-end:

- `.sha256` — SHA-256 checksum (integrity)
- `.sigstore` — sigstore bundle: signature + Fulcio certificate + Rekor
  transparency-log entry (proves the asset was built by Stable Studio's CI)
- `forge-core-<version>.cdx.json` — CycloneDX SBOM (transitive dependency
  inventory for CVE scanning)

**Verify integrity** (proves the bytes were not tampered in transit):

```bash
sha256sum -c forge-core-<arch>-<os>.tar.gz.sha256
```

**Verify identity** (proves the asset was built by Stable Studio's CI and is
recorded in the public Rekor transparency log):

```bash
cosign verify-blob \
  --bundle forge-core-<arch>-<os>.tar.gz.sigstore \
  --certificate-identity-regexp 'https://github.com/Stable-Studio/forge-method-rust/.github/' \
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
git clone https://github.com/Stable-Studio/forge-method-rust.git forge-method-rust
cd forge-method-rust

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

**Custom gates.** To encode project-specific checks (Bruno YAML parse, QA
package validator, report linter, secret scan, fixture/data-safety validator,
Testmo dry-run, …), run `forge-core preflight init` once — it writes
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
  - name: bruno_yaml_parse
    command: ["npx", "@bruno/api", "validate", "--strict"]
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
- Dual-lane autonomy router exposed as `forge-core autonomy route` for fast vs
  rigorous lane selection.
- Seven evolve-phase governance contracts: `autonomy_policy`,
  `verification_goal`, `agent_run`, `memory`, `checkpoint`, `eval_run`, and
  `telemetry`.
- GitHub Actions CI is present for fmt, clippy, tests, and validation.

**Not yet (roadmap)**

- **MCP server** — `forge-core mcp serve` is implemented (stdio JSON-RPC,
  tool-call attestation, fail-closed allowlist). Roadmap: snapshotting /
  hardening and a richer tool surface. The CLI remains the intended agent
  boundary by design.
- **State derivation layer** — `forge_core_store::derive_state` is now the
  sole authority constructor for claim state, replaying the append-only WAL
  with torn-tail auto-repair. The ephemeral `claims-active/*.yaml` cache is
  no longer an authority path (inspect it via `claim status --from-cache`).
  The effect WAL is also implemented and tested. Snapshot/rotation as a read
  cache (P3.3) remains a later perf layer.
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

```
crates/
  forge-core-contracts/   typed YAML contract types (the vocabulary)
  forge-core-decisions/      guide, claims, conflict detection, phase transitions
  forge-core-store/       reference index + validation store
  forge-core-validate/    schema + semantic validator
  forge-core-schema/      JSON-schema projections
  forge-core-kernel/     operation planning + execution
  forge-core-cli/         the forge-core binary
contracts/
  workflows/              the 110-workflow catalog (the method)
  spec/  plan/  stories/  discovery artifacts (typed YAML)
  claims/                 claim fixtures + schemas
skill/forge-method/        host skill wiring
docs/                      live ADRs + design specs (code-cited)
```

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
