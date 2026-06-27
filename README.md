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
  writes. Two agents cannot hold conflicting claims.
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
- **Conflict detection** — before any write, the agent checks whether another
  active claim covers the target path. Conflicting writes are refused.
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

**Supply-chain surface (host-adapter)** — a set of commands for projecting the
protocol onto host environments (MCP tools, borrowed shells, app UI) and for
verifying distribution artifacts (provenance, signatures, transparency logs).
This is the spine that lets the protocol run safely across untrusted hosts.

---

## Install

You need a Rust toolchain (1.85+, edition 2021) and `cargo`.

### Option A — build from source (recommended)

```bash
git clone <your-repo-url> forge-method-rust
cd forge-method-rust

# install the forge-core binary into ~/.cargo/bin
cargo install --path crates/forge-core-cli
```

Verify it landed on your PATH:

```bash
forge-core validate
# forge_core_validation_passed checks=NN diagnostics=0
```

### Option B — prebuilt binary

If you already have a release build, copy it somewhere on your PATH:

```bash
cp target/release/forge-core* /usr/local/bin/   # Linux/macOS
# on Windows/WSL the artifact is forge-core.exe
```

### What you get

A single binary, `forge-core`, plus the `contracts/` tree (the catalog, schemas,
and example claims). The binary is self-contained — no runtime, no daemon, no
network.

---

## Quick start

### Initialize the coordination bus

Pick a directory where active claims will live (gitignore it):

```bash
mkdir -p .forge-method/claims-active
echo ".forge-method/claims-active/" >> .gitignore
```

### Ask the guide what to do

```bash
forge-core guide describe              # list every workflow in the catalog
forge-core guide status --phase 1-discovery   # what's required in this phase?
```

### Claim your work, then write

```bash
# an agent claims the file it is about to edit
forge-core claim acquire \
  --scope story --id my-feature \
  --agent codex-worker-1 \
  --path src/auth.rs

# before writing, check no one else holds it
forge-core claim check-write --agent codex-worker-1 --target src/auth.rs

# ... do the work ...

# release when done
forge-core claim release --id claim.story.my-feature.my-feature --agent codex-worker-1
```

### Validate the project

```bash
forge-core validate              # checks every contract in the tree
forge-core execute-operation --root . --operation contracts/operations/ship.yaml
```

All commands accept `--json` for machine consumption — that is how host agents
call them.

---

## Status (v0.1.0)

**Proven / working today**

- 394 tests green across the workspace; `cargo check`, `clippy -D warnings`,
  `cargo fmt` all clean.
- The full 7-phase method and 110-workflow catalog.
- Claim engine, conflict detection, worktree isolation, coordination eval —
  validated end to end with parallel workers.
- Multi-agent governance on the happy path: multiple agents, disjoint files,
  coordinated by claims.
- Self-hardening batch landed: TTL-overflow safety, RFC-3339 calendar
  validation, lockfile stale-owner reclaim.

**Not yet (roadmap)**

- **MCP server** — today host agents call `forge-core` over the shell; a native
  MCP surface is the next slice (v0.2). The CLI is the intended agent boundary
  by design.
- **Append-only WAL + state derivation** — current state is reconstructed from
  the claims bus on each invocation. A durability layer (CRC-checked WAL,
  prefix recovery, `derive_state` as the sole constructor) is designed and
  queued for v0.2.
- **License** — not yet chosen; set one before public release.

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
  forge-core-engine/      guide, claims, conflict detection, phase transitions
  forge-core-store/       reference index + validation store
  forge-core-validate/    schema + semantic validator
  forge-core-schema/      JSON-schema projections
  forge-core-runtime/     operation planning + execution
  forge-core-cli/         the forge-core binary
contracts/
  workflows/              the 110-workflow catalog (the method)
  spec/  plan/  stories/  discovery artifacts (typed YAML)
  claims/                 claim fixtures + schemas
skills/                   host skill wiring
forensic-reference/       migrated legacy reference (NOT runtime authority)
```

`forensic-reference/` is historical context only. The authority is always the
`contracts/` tree plus the running `forge-core` binary.

---

*Forge Method Core is a standalone protocol and runtime. It is built to be
adopted by any agent, any team, any model — and to stay out of their way.*
