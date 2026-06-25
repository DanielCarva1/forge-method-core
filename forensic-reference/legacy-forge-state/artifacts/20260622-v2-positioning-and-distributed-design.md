# Forge v2.0 — Positioning & Distributed-State Design Addendum

- kind: design-rfc-addendum
- created_at: 2026-06-22
- parent: `.forge-method/artifacts/forge-flock-coordination-rfc-v3.md`
- status: ready-for-integration-into-RFC
- trigger: reality-evidence-gate + maintainer "fácil demais" intuition surfaced 3 blind spots (positioning, distributed state, human governance) that the grill-gate did not test
- sources_verified:
  - A2A: `github.com/a2aproject/A2A` (v1.0.1, 28 May 2026; 24.4k stars; Linux Foundation; JSON-RPC 2.0 over HTTP(S); agent discovery via Agent Cards; **opacity** principle — agents collaborate without exposing internal state/memory/tools)
  - MCP: `modelcontextprotocol.io` (Anthropic; open standard; model↔data/tools connection; "USB-C for AI"; supported by Claude, ChatGPT, VS Code, Cursor)

---

## 1. External scan — the 2025-2026 agent-protocol landscape the RFC missed

The RFC's §3 research covered *parallel coding agents in IDEs* (Cursor/Claude Code/Devin/Codex/Windsurf) and *workspace-state research* (STORM). It did NOT cover the **agent-interoperability protocol layer**, which is the directly adjacent space to Forge's "coordination protocol" thesis. Two established open standards dominate that layer:

| Protocol | Scope | Transport | Unit | State stance |
|---|---|---|---|---|
| **MCP** (Anthropic, 2024) | model/app ↔ data/tools/workflows | JSON-RPC | capability connection | n/a (tool layer) |
| **A2A** (Google → Linux Foundation, 2025) | agent ↔ agent delegation/messaging | JSON-RPC 2.0 / HTTP(S) / SSE | opaque agent services | **opacity** — no shared internal state |
| **Forge** (this RFC) | human+agent flock ↔ shared repo state + governance | **files** (YAML/MD/NDJSON) via git | flock working on one repo | **explicit shared coordination state** |

These are **three different layers**, not three competitors. The RFC's thesis is defensible — but only if it makes the layering argument explicitly. Today it does not, so it reads as naive to anyone who knows A2A exists.

---

## 2. Forge positioning — the three-layer model

```
┌──────────────────────────────────────────────────────────────┐
│  COORDINATION & GOVERNANCE LAYER   ←── FORGE lives here       │
│  Who owns integration state now (driver claim)?              │
│  Who may write which lane? Is the spec locked?               │
│  Progressive autonomy, handoffs, verification gates.         │
│  Substrate: .forge-method/ files synced via git.             │
└──────────────────────────────────────────────────────────────┘
┌──────────────────────────────────────────────────────────────┐
│  AGENT INTEROP / MESSAGING LAYER   ←── A2A lives here        │
│  Agent-to-agent task delegation, discovery, negotiation.     │
│  Agents as opaque services. Substrate: network RPC.          │
└──────────────────────────────────────────────────────────────┘
┌──────────────────────────────────────────────────────────────┐
│  CAPABILITY / CONTEXT LAYER        ←── MCP lives here        │
│  Model ↔ data sources, tools, workflows. "USB-C for AI."     │
└──────────────────────────────────────────────────────────────┘
```

**Forge is the layer ABOVE A2A and MCP.** A Forge-coordinated agent uses MCP to reach tools, A2A to delegate a task to a different vendor's agent, and Forge to know whether it's allowed to write the integration state right now. They compose; they don't compete.

**The defensible differentiator:** A2A and MCP are *network/RPC* protocols about *capability and messaging*. Forge is a *file/governance* protocol about *shared mutable state and who may touch it*. No other open protocol owns the "human+agent flock coordinating around one repo's state, commit-safe by construction, runtime-agnostic" layer. A2A explicitly refuses this — its opacity principle says agents don't share state. **Forge's whole point is the shared coordination state A2A declines to provide.**

---

## 3. Reconciling the apparent conflict: A2A opacity vs Forge shared state

A reader familiar with A2A will object: *"A2A says agents must be opaque; Forge says agents share integration state. Contradiction."*

**Resolution (must be in the RFC):** the two stances address DIFFERENT state.
- **A2A opacity** = an agent's *private* memory, logic, and tool implementations are not exposed to other agents. An agent is a black-box service.
- **Forge shared state** = the *explicit coordination artifacts* (integration FSM, lane claims, handoff-requests, spec-lock signal). These are NOT any agent's private memory — they are the protocol's own durable surface, like `.git/` is git's surface.

An agent in a Forge flock remains fully opaque in the A2A sense: its reasoning, context, and tools are private. What it shares is its *coordination intent* (which lane it claims, what handoff it emits) — exactly the minimum needed for safe concurrent work. **Forge is A2A-compatible by construction: it never asks an agent to expose its internals, only to declare its coordination moves in the protocol files.**

---

## 4. Agent discovery: Forge registry vs A2A Agent Cards

These overlap and the RFC must differentiate.

- **A2A Agent Card** = network-scoped descriptor of an agent's capabilities + connection endpoint. Lets ANY agent on the network discover and delegate to it. Dynamic, service-oriented.
- **Forge `agents/registry.yaml`** = flock-scoped roster of agents working on ONE repo, with lane assignments and the driver claim. Repo-local, governance-oriented.

**They are complementary, not redundant.** A Forge agent MAY expose an A2A Agent Card so agents in OTHER flocks (or standalone network agents) can delegate to it via A2A. Forge's registry answers "who is working on this repo's lanes right now"; A2A's card answers "what can this agent do for me over the network." A future Forge feature could auto-emit Agent Cards for fleet agents — but that is a Layer-5 adapter concern, not a protocol-coupling concern. **Decision: keep Forge registry flock-scoped; treat A2A-Card emission as an optional Layer-5 capability, not a v2.0 requirement.**

---

## 5. Distributed-state design — solving the multi-machine merge problem (RFC buraco 2)

### The problem, precisely
The RFC's Layer-1 optimistic concurrency (`version` field + `expected_version` check) handles concurrent writers **within one checkout/process**. But the org-scale vision is agents on **different machines** syncing `.forge-method/` via `git push/pull`. `state.yaml` and `sprint.yaml` are mutable, non-append-only files. Two flocks both running `transition` and pushing → **git merge conflict in live runtime state**, blocking all agents until a human hand-edits YAML.

### The resolution — already latent in the RFC's own principles, just not connected
The RFC has the pieces. They compose to solve the distributed case. This addendum makes the composition explicit:

**Principle 1 (single-writer on integration FSM) IS the distributed answer.** Only the current *driver* writes `state.yaml`/`sprint.yaml`. In the distributed case, the driver claim is itself a **git-synced file** (`.forge-method/claims/driver.lock` with `{agent_id, flock, ts, expires}`). To take the driver role, an agent commits the claim file and pushes; all other flocks pull and see the new driver. Workers (including other flocks' workers) NEVER write integration state — they emit append-only entries to `handoffs/` and `requests.ndjson`, which the driver polls and applies via version-checked `transition`.

**Consequence: state.yaml merge conflicts should NEVER occur under discipline**, because there is exactly one driver at a time across all machines, and only the driver writes it. The append-only files (`ledger.ndjson`, `requests.ndjson`, `handoffs/`) merge cleanly by construction (that is why S1 in the audit is load-bearing).

**When a conflict DOES occur**, it is the *signal* of a discipline violation or a claim-transfer race. The resolution path:
1. The `version` counter (Layer 1) catches the divergence at git-merge time.
2. The typed `StateConflict` error surfaces it with a diff.
3. The loser re-reads, discovers the driver claim has moved, and either (a) re-requests as a worker, or (b) re-takes the driver claim if the previous one expired (TTL).
4. **A merge conflict in `state.yaml` is NEVER silently resolved; it is always a durable event** in the ledger (`state.merge-conflict {versions, agents, resolution}`), because it indicates either a bug or a claim-transfer protocol gap to fix.

### What this ADDS to the RFC (not in v3 today)
- **Explicit statement:** the driver claim is a git-synced file; only the holder writes integration state; workers always emit append-only requests — across machines, not just within one checkout.
- **Conflict semantics:** a state.yaml merge conflict is a typed signal, not a manual-YAML-fix task; the version counter + ledger event handle it deterministically.
- **Claim-transfer protocol across machines:** `claims/driver.lock` commits + pushes; TTL (30 min, Q2) covers the crash case; handoff-on-TTL-expiry (Q2 maintainer decision) ensures continuity.
- **New principle (proposed Principle 18):** *"Integration state has exactly one writer across all machines — the current driver. Workers emit append-only requests; the driver applies. A state.yaml merge conflict is a typed signal of a protocol race, never a manual-resolution task."*

### What this does NOT require
- No server/daemon. Still pure files.
- No rewrite of Layer 1. The version counter + append-only backbone already exist (S1) or are additive (R1).
- No change to single-agent users (no registry = no driver claim file = legacy mode).

---

## 6. Human-human governance — explicit scoping (RFC buraco 3)

**Decision: OUT OF SCOPE for v2.0, explicitly.**

Forge governs **agent-agent and human-agent coordination within the protocol**. Human-human governance — planning ceremonies, priority disputes, org structure, roadmap arbitration between flocks — remains a **human process**, supported by git, CODEOWNERS, PR review, and the team's own operating model. Forge does not replace engineering management.

The RFC's `team-operating-model` and `trunk-based-plan` workflows CAPTURE human governance decisions as artifacts; they do not AUTOMATE human-human authority. That line is intentional and must be stated, otherwise the "org-scale" vision overpromises.

**Explicit anti-pattern to encode:** *"Forge does not arbitrate disputes between humans. Two flocks in conflict route to the team's human governance process; Forge records the outcome, never decides it."*

---

## 7. Updates required to RFC v3

This addendum should land as RFC edits before commit:
1. **New §2.5 "Protocol positioning"** — the three-layer model (§2 above) + A2A/MCP acknowledgment.
2. **§2.6 "Reconciling opacity and shared state"** — §3 above (A2A-compatibility argument).
3. **§6.4 strengthening** — driver claim as git-synced file; cross-machine discipline (§5 above).
4. **§6.4 / §9 risk table** — replace the one-line "merge conflicts across flocks" hand-wave with the typed-signal resolution (§5).
5. **§6.4 / new §6.9 "Agent discovery boundary"** — Forge registry (flock-scoped) vs A2A Agent Card (network-scoped); A2A emission is Layer-5 optional (§4).
6. **New principle §5 #18** — single-driver-across-machines (§5).
7. **Appendix A (what this RFC does NOT propose)** — add: "automate human-human governance; Forge records human governance outcomes, never decides them" (§6).
8. **§11 references** — add A2A (a2a-protocol.org spec) + MCP (modelcontextprotocol.io).
9. **New §3.5 "Independent validation"** — cite grite (arXiv:2606.19616), CooperBench (arXiv:2601.13295), CoAgent (arXiv:2606.15376), Shopify Aquifer/River as independent convergence on Forge's architecture (§9 below).
10. **New Layer 1.5 in §6.2 architecture** — CRDT projection from the append-only coordination log (§10, upgrade A).
11. **New principles §5 #19 and #20** — completion-state (§11, upgrade B) and notify-don't-lock (§12, upgrade C).
12. **§8 roadmap / new stance** — default to serial execution + parallelism only where the task graph proves independence; practical ceiling 2-3 concurrent code-writing agents (§13).
13. **Appendix C pitch** — reframe: "coordination is the bottleneck, not coding" (CooperBench 25% cooperating vs 50% solo); Forge provides verifiable commitments (§13).

---

## 8. Impact on prior session decisions

- **Reality-evidence-gate stance:** UPGRADES from "PLAUSIBLE→STRONG conditional" toward STRONG. The thesis is MORE defensible after positioning, not less — Forge owns a layer no open protocol owns. The condition (Phase B empirical) stands.
- **Grill-gate 7 questions:** UNCHANGED. The positioning + distributed design do not re-open any of the 7. Q3 (flock discovery) is *strengthened* — FORGE_* env is now clearly flock-scoped, distinct from A2A network discovery.
- **Spec-lock:** the lock holds, but the spec is not COMMUNICABLE (or commitable) as-is — it needs the §7 edits integrated first. Lock scope expands from "vision + architecture" to "vision + architecture + positioning + distributed semantics."

## 9. Independent validation — the architecture is not a bet (deep-research 2026-06-22)

Source: `.forge-method/artifacts/20260622-deep-research-multi-agent-at-scale.md` (research-scan, registered). The convergent finding: multiple independent teams arrived at Forge's exact architecture.

- **grite (Sarkar, ASU, arXiv:2606.19616, June 2026)** — built append-only event log in git refs + CRDT projection + advisory leases. Measured N=32 agents: **78%→0% duplicate work, 3× throughput, byte-identical convergence proven**. The single most validating result: Forge's design, independently built and empirically tested.
- **CooperBench (Stanford/SAP, arXiv:2601.13295, Jan 2026)** — two-agent cooperation succeeds only **25% vs ~50% solo**. Agents fail at *coordination, not coding*. Reframes the entire value proposition: Forge's job is verifiable commitments, not better code generation.
- **CoAgent (SJTU, arXiv:2606.15376, June 2026)** — "notify, don't lock or abort": the LLM can judge whether a conflict actually invalidates its plan. A primitive classical transactions lack (basis for upgrade C).
- **Shopify Aquifer/River (2026-05-28)** — durable event log as substrate; 59,918 sessions, 3,536 PRs/30 days, 7,000+ people. "Cells die, machines die. The conversation doesn't." Largest production deployment of the pattern.
- **Stripe Minions** — 1,000+ PRs/week, devbox-isolated fleet. Full-environment isolation enables unattended autonomy.

**Net effect on the thesis:** the reality-evidence-gate stance UPGRADES from "PLAUSIBLE→STRONG conditional" toward **STRONG**. The architecture is the industry's convergent answer; Forge formalizes it as an open, runtime-agnostic, human-first protocol rather than a vendor-locked feature.

**Critical constraint surfaced (drives upgrade B):** grite's arms showed **locks alone are insufficient** — the locks-only arm had the *highest* redundant-rediscovery rate. Only locks + shared completion state drove failure modes to zero.

---

## 10. Upgrade A — CRDT hybrid (integration FSM stays single-driver; coordination state converges via CRDT)

**Decision:** Hybrid, not all-CRDT, not all-single-driver.

**Source:** grite (arXiv:2606.19616) + research top-10 #1.

**Rationale:** the Integration FSM (`state.yaml`: phase/status/workflow/story) is a true state machine — transitions are NOT commutative; you cannot merge "phase=3-plan" with "phase=4-build." There is one correct integration state at a time → **single-driver (one writer) is right for it.** But the *coordination state* (registry membership, lane claims, task-completion, handoff-requests, events) IS naturally CRDT-friendly: set-union (who's in the flock), last-writer-wins keyed by total order (who holds a claim), append-only (events). grite proved these converge **byte-identical without a central writer.**

**The design:**
- **Layer 1 (substrate):** append-only event log in git (existing `ledger.ndjson` + new coordination-log). Nothing but the log is hand-edited; everything else is a projection.
- **Layer 1.5 (NEW): CRDT projection** — materializes registry, claims, completion-set, handoff-requests from the log. Deterministic, byte-identical across machines. [grite pattern]
- **Layer 3 (integration FSM):** single-driver via `claims/driver.lock` (git-synced). ONE writer for `state.yaml`/`sprint.yaml` across all machines.

**Why this realizes Principle 6 (Hybrid FSM + events):** the RFC already gestured at "FSM + events." The research tells us the event side must be **CRDT-projected**, not naive last-write-wins. This is an upgrade, not a contradiction.

**Principle 18 (revised):** *"Integration FSM has one driver across machines; coordination state converges via CRDT projection from an append-only, git-synced log. A `state.yaml` merge conflict is a typed signal of a driver-claim race, never a manual-resolution task."*

---

## 11. Upgrade B — completion-state (locks alone are insufficient)

**Decision:** MANDATORY. Add shared task-completion state to the protocol.

**Source:** grite (arXiv:2606.19616) — the locks-only arm had the HIGHEST redundant-rediscovery rate; only locks + shared completion state zeroed the failures. (Counter-intuitive; empirically proven.)

**Rationale:** an agent arriving at a lane needs to see not just "is this lane free?" (claim/lock) but "is this task already DONE?" Without completion state, two agents rediscover the same finished work. The grite numbers (78%→0% redundant) come from this exact combination.

**The design:**
- A typed event `task.completed {task_id, agent_id, ts, proof_ref}` in the append-only log.
- The CRDT projection materializes a **done-set** queryable in O(1).
- Arrival protocol: check completion FIRST (skip if done) → then claim (work if free) → on finish, emit completion.
- Composes with Q2's handoff-on-TTL-expiry: when a claim expires and emits a handoff, the handoff + completion-records tell the next agent exactly what is done and what is pending.

**Principle 19 (NEW):** *"Locks alone are insufficient. The protocol tracks both lane claims AND task-completion state, so an arriving agent sees what is done, not just what is claimed."* [grite]

---

## 12. Upgrade C — notify, don't lock-or-abort (LLM-as-conflict-judge)

**Decision:** ENTERS, as a Layer-1 enhancement, not a replacement for claims.

**Source:** CoAgent (arXiv:2606.15376) — "notify, don't lock or abort."

**Rationale:** claims/lanes still prevent two agents *starting* the same work (the "who owns this" question — real need). But once working, if another agent writes a file my agent READ, classical optimistic concurrency aborts the whole transaction. CoAgent shows the LLM can judge "this change doesn't actually affect my plan" and continue, OR "this invalidates step 3, redo just that" — strictly better than blind abort.

**The design:**
- Claims prevent duplicate **starts**; notify-don't-lock prevents wasteful **aborts mid-work**. They compose.
- Every mutating write appends an event; agents whose read-set overlaps the write's path get a notification; the agent's runtime asks the LLM "does this change invalidate your current plan?" — cheap, high value.
- The saga-style compensation part (register an inverse for every write) is heavier — **deferred to post-v1.** The notify-and-judge is the cheap 80%.

**Principle 20 (NEW):** *"Notify, don't blindly abort. When a write touches an agent's read-set, the runtime notifies and lets the agent judge whether its plan is invalidated — patching only the affected step, not restarting."* [CoAgent]

---

## 13. Defaults & pitch reframe

**Default stance (from research top-10 #8):**
- **Serial execution by default; parallelism only where the task graph proves independence.** Practical ceiling is **2-3 concurrent code-writing agents** (human review is the bottleneck, not compute). Integration tax is nonlinear: 2 agents ≈ 1.5×, 8 agents ≈ 5× (Factory; Helge Sverre; When Parallelism Pays Off, arXiv:2606.00953). Broad parallelism loses to serial-with-targeted-parallelism. Forge assumes this; it does not promise "N agents."

**Pitch reframe (from CooperBench, arXiv:2601.13295):**
- Agents solo succeed ~50%; cooperating drop to 25%. The failure is **coordination, not coding** — "the curse of coordination." Forge's value is NOT better code generation; it is **verifiable commitments** (completion-records, signed claims, insertion-point contracts, the done-set) that turn conversation into checkable shared state. This flips the 25% back up. That is the selling argument.

---

## Handoff (updated)
- preserve: three-layer positioning; A2A-opacity reconciliation; distributed single-driver semantics (Principle 18); **CRDT projection for coordination state (upgrade A)**; **completion-state (upgrade B / Principle 19)**; **notify-don't-lock (upgrade C / Principle 20)**; serial-first default; coordination-is-the-bottleneck pitch; human-governance out-of-scope; registry vs Agent-Card boundary; independent validation (grite/CooperBench/CoAgent/Shopify).
- do_not: commit RFC v3 without the §7 edits (now items 1-13); claim Forge competes with A2A/MCP; leave the distributed merge case as a hand-wave; treat locks-without-completion-state as sufficient; promise "N agents."
- next: integrate §7 items 1-13 into RFC v3 (each citing its gate source) → `audit` + `gate` → commit → Phase B (empirical gap-report, now with grite's exact protocol-experiment to replicate).
