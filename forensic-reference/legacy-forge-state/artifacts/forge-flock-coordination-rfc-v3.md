# RFC: Forge Method — Coordination Protocol for Human+Agent Flocks (v3)

- kind: design-rfc
- status: draft-for-maintainer-review
- author: human (Daniel Carvalhal) + Pi agent
- created_at: 2026-06-22
- supersedes: `forge-multi-agent-design-rfc.md` (v2) — v3 reframes the vision after maintainer feedback
- related: `forge-runtime-audit.md` (codebase audit)
- applies_to: `forge-method-core` (github.com/DanielCarva1/forge-method-core), consumed byte-identical by Pi and Codex runtimes today; designed to be consumed by Claude Code, OpenCode, and the future Forge App
- runtime_baseline: v1.34.1

> **Thesis (one line):** Make Forge the **trunk-based-development equivalent for the human+agent world** — a runtime-agnostic coordination protocol where any number of humans, each operating any number of agents (across any runtime: Pi, Codex, Claude Code, OpenCode, the future Forge App), work on one repo without corrupting shared state, without rogue commits, and without anyone being a bottleneck.
>
> **Thesis (three lines):**
> 1. The unit is a **flock**: a human + their agents (Codex chats + subagents, Pi + subagents, etc.), and the protocol scales from one flock to a whole org.
> 2. Coordination is a **file protocol**, not runtime-API calls — so it's agnostic to Pi/Codex/Claude/OpenCode/the future Forge App, and agnostic to model (GPT, Opus, ZAI, anything). The `.forge-method/` is to human+agent coordination what `.git/` is to code coordination.
> 3. Autonomy is **progressive and cyclic**: human directs hard at the start (POC → converge → lock spec), agents execute autonomously once locked (verification gates, not approval gates), and **evolve loops back to directed mode** for each new feature. The human is never a bottleneck in the build phase; never absent in the decision phase.

---

## 1. TL;DR

The maintainer's vision, refined across three rounds of dialogue, is bigger than "multi-agent on one project":

- **Multiple humans, each with multiple agents**, on the same repo/org — like a real engineering team, but every engineer brings a fleet.
- **Commit-safe by construction**: no one ever "com o cu na mão" wondering if an agent's commit will turn the codebase into a mess. Claims, branch policy, reviewer gates, and verification gates guarantee safety structurally.
- **Scales to large repos and orgs**, like trunk-based development does for human teams.
- **Runtime-agnostic**: today Pi + Codex; tomorrow Claude Code, OpenCode, and the Forge App (the maintainer's own harness, multi-provider: GPT/Opus/ZAI/etc). The protocol must not depend on any runtime's API.
- **Guided, fun, partner-like experience**: the agent is an excited expert friend who "matches the human's energy," researches when asked, shows how things will look/work, and closes blocks cleanly — not a bureaucratic form.
- **Progressive autonomy**: heavy human direction early (decisions, POCs), full agent autonomy once the spec is locked (build/review/E2E/auto-fix), and the evolve phase loops back to directed mode for each new feature.

This RFC specifies how to get there without breaking the existing single-agent runtime (backward-compatible, opt-in), reusing what Forge already does well, and adding the minimum that's missing: **concurrency-safe state, a flock coordination substrate, progressive-autonomy modes, and a partner-grade human experience.**

---

## 2. Problem & Vision

### 2.1 The reframe — trunk-based for the human+agent world

Trunk-based development is the industry answer to "how do many humans write code to one repo without chaos?" Short-lived branches, small PRs, CODEOWNERS, required checks, merge authority. It scales.

The 2026 problem is **one level up**: now every human brings N agents. A team of 10 humans is really a team of 10 humans + 30–100 agents, all touching the same repo. Git's branch policy handles human-human conflicts, but it has **no concept of agent identity, no concept of "who owns this lane right now," no concept of "this agent's work is autonomous, that one needs approval," no concept of "this spec is locked so stop asking."** That's the gap Forge fills.

> **Forge's product position:** the runtime-agnostic, human-first **coordination protocol** for flocks of humans+agents. Vendors (Claude Code, Codex, Cursor, Copilot) each have their own multi-agent — but locked to their ecosystem. Forge is the **open substrate** that lets a Codex agent, a Pi agent, a Claude Code agent, and a future Forge App agent all collaborate on one repo, with their humans, safely.

### 2.2 The Forge App direction (runtime-agnostic by necessity)

The maintainer is building the **Forge App** — its own runtime+harness, multi-provider (GPT, Opus, ZAI, the list is long), no dependency on Pi/Codex/Claude/opencode. This isn't a future concern to patch in; it's a **design constraint that cleans the architecture**:

- The coordination protocol **cannot** call runtime APIs. It must be pure files (YAML, Markdown, NDJSON) that any runtime reads/writes.
- The facilitation packs and workflow refs **cannot** assume a model. They describe *behavior* (what to ask, when to research, how to close a block) that any capable model can follow.
- The "how an agent is spawned" is the **only** runtime-specific part — and it's behind an interface. Pi uses its `subagent` tool; Codex uses native; Claude Code reads AGENTS.md/CLAUDE.md; OpenCode is CLI; the Forge App does it natively. **Coordination is identical across all of them.**

This is why expansion to Claude Code and OpenCode is **easy, not hard**: they just need to (a) read/write the `.forge-method/` protocol files, and (b) emit an `AGENTS.md`/`CLAUDE.md`/equivalent that points to the protocol. No deep integration.

### 2.3 Hard constraints (non-negotiable)
- **C1. Preserve each agent's state machine.** The FSM is the soul of Forge.
- **C2. Backward-compatible.** A project with no multi-agent opt-in behaves exactly as v1.34.1. Zero breaking changes for existing users mid-flight.
- **C3. Runtime-agnostic.** The protocol works on Pi, Codex, Claude Code, OpenCode, and the future Forge App, with no runtime-specific coupling in the coordination layer.
- **C4. Model-agnostic.** Works on GPT, Opus, ZAI, anything capable. Facilitation is behavior, not model features.
- **C5. Opt-in and facilitated.** Multi-agent is surfaced when it makes sense, through dialogue — never automatic/silent.
- **C6. Commit-safe by construction.** No agent can land broken or conflicting work. Claims + branch policy + reviewer gate + verification gates guarantee it structurally.
- **C7. Quality packaging intact.** Gates, evals, decision-source traceability, guidance-safety guardrails keep working and get stronger.
- **C8. Partner-grade experience.** The agent matches energy, researches when asked, shows previews, closes blocks — it's an excited expert friend, not a form.

### 2.4 The blocker (confirmed by code audit)
`write_flat_yaml` (`scripts/forge_method_runtime.py:857`) is a full overwrite, zero locking, no version field. Every state-mutating command is read-modify-write. **Two concurrent agents on `state.yaml` = silent data loss.** This is the only true blocker; everything else is additive.

### 2.5 Protocol positioning — Forge sits ABOVE A2A and MCP (positioning addendum §2)

Forge does not compete with the 2025-2026 open agent-protocol layer; it occupies the layer above it, which nobody owns.

```
COORDINATION & GOVERNANCE  ← FORGE (who may write what; is the spec locked; progressive autonomy)
AGENT INTEROP / MESSAGING  ← A2A (agent-to-agent delegation; agents as opaque services)
CAPABILITY / CONTEXT       ← MCP (model ↔ tools/data)
```

- **MCP** (Anthropic, 2024): model/app ↔ data/tools. "USB-C for AI."
- **A2A** (Google → Linux Foundation, 2025; v1.0.1 May 2026): agent ↔ agent delegation via JSON-RPC/HTTP. **Opacity principle** — agents collaborate without exposing internal state.
- **Forge:** human+agent flock ↔ shared repo state + governance, via files synced through git.

No open protocol owns the "human+agent flock coordinating around one repo's state, commit-safe by construction, runtime-agnostic" layer. A2A explicitly refuses it (opacity). **Forge is A2A-compatible by construction**: it never asks an agent to expose its private memory, only to declare its coordination moves (lane claimed, handoff emitted) in the protocol files. (Full argument in addendum §3; agent-discovery boundary in addendum §4.)

### 2.6 Independent validation — this is not a bet (deep-research 2026-06-22)

Multiple independent teams converged on Forge's exact architecture:
- **grite** (arXiv:2606.19616) built append-only event log in git + CRDT projection + advisory leases; measured N=32 agents, **78%→0% duplicate work, 3× throughput, byte-identical convergence**.
- **CooperBench** (arXiv:2601.13295): two-agent cooperation succeeds **25% vs 50% solo** — coordination, not coding, is the bottleneck.
- **CoAgent** (arXiv:2606.15376): "notify, don't lock or abort" — the LLM judges whether a conflict matters.
- **Shopify Aquifer/River:** durable event log as substrate; 59,918 sessions, 3,536 PRs/30 days, 7,000+ people.

Forge formalizes the industry's convergent answer as an open, runtime-agnostic, human-first protocol. Reality-evidence-gate stance **UPGRADES toward STRONG**. Full synthesis: `.forge-method/artifacts/20260622-deep-research-multi-agent-at-scale.md`.

---

## 3. State of the Art (2026 research) — condensed

Full citations in §11. Headline findings that shaped this RFC:

- **STORM (Liu et al., arXiv:2605.20563):** shared-workspace + write-time consistency **beats** git-worktree isolation. Two valid framings, both correct in context: **+18.7 on Commit0-Lite, +1.4 on PaperBench** are the *headline averages* across all repos (reached 87.6 / 78.2 combined with single-agent runs); **+34.6 points** is the delta on the *high-coupling-code subset*, where GitWorktree collapses to 36.3% pass-rate and STORM holds 70.9% — STORM is the only method that does not break down under coupling. Agents don't need a frozen whole-workspace snapshot — STORM mediates interactions with the shared workspace so conflicting edits are detected/resolved at write time and only the manager commits. **Refutes the "worktree per area" instinct; write-time optimistic concurrency on the read-set is the better primitive.** (Both STORM figures verified against abstract + body via deep-research 2026-06-22.)
- **Vendor convergence (Feb–May 2026):** Cursor/Claude Code/Devin/Windsurf/Codex all shipped parallel agents with the same primitives — shared task list + dependency tracking, peer messaging, file locking, plan-approval, worktree isolation. Claude Code's Agent Teams: 2–16 sessions, one lead, peer-to-peer.
- **Addy Osmani (Code Agent Orchestra):** "delegate tasks, not judgment"; "verification is the bottleneck, not generation"; Ralph Loop = stateless-but-iterative with external memory; **human-curated specs only** (ETH Zurich: LLM-written AGENTS.md *reduces* success ~3%).
- **Agent Experience (AX, GitHub Build 2026):** the design discipline for *persistent* collaborators — legibility, auditability, context persistence, accountability. "Chat for ambiguity; canvases for inspectable work." Session modes Interactive → Plan → Autopilot.
- **Spec-driven development (GitHub Spec Kit, Microsoft, PBC, MDA):** structured Markdown specs as the alignment layer; typed contracts; anti-patterns explicit.
- **Distributed primitives (tianpan, munderdifflin, agentpatterns, ESAA):** append-only event log + single committer + optimistic concurrency + file-based locks + git-push-as-enforcement.

---

## 4. Audit of Forge Today — condensed (full detail in `forge-runtime-audit.md`)

**Strengths to preserve:**
- **S1.** Append-safe event log (`ledger.ndjson`, `index.ndjson` via `open("a")`). **This is the coordination backbone.**
- **S2.** Write-time guidance-safety guardrails (anti-patterns checked on every write). Rare and valuable.
- **S3.** Rich facilitation packs (34 packs, structured: `open_floor`, `elicitation_options`, `facilitator_moves`, `anti_patterns`).
- **S4.** Workflows machine-validated with compactness caps.
- **S5.** Collaboration vocabulary exists (`team-operating-model`, `product-area-map`, `trunk-based-plan`, `collaboration-handoff`, `council-decision`, `repo-split-plan`).
- **S6.** Decision-source traceability (stories cite `decision_sources`; gate blocks done stories lacking a source).
- **S7.** The `grill-gate` workflow references Matt Pocock's grill-with-docs state machine (S0→S6: one question at a time, recommended answer, check code before asking, update glossary inline) — excellent block-closing mechanic. It IS semi-default today (triggers in phases 1/2/3 and before unlocking mechanical work), but **not wired as a default at every decision-close point** — it does not fire before every handoff / phase transition / decision-lock (confirmed gap, §6.5; refined by reality-evidence-gate 2026-06-22).

**Critical gaps:**
- **G1.** `state.yaml` write = full overwrite, zero concurrency control. (BLOCKER)
- **G2.** `handoff`/`checkpoint` mutate `state.yaml.next_action` — workers clobber the driver. (BLOCKER)
- **G3.** No agent registry / per-agent state. (BLOCKER)
- **G4.** No owner/agent attribution in ledger/stories.
- **G5.** Council `agent-team`/`parallel`/`subagent` modes are descriptive labels, no real worker spawning.
- **G6.** No claim/lock primitive for lanes (Product Areas / stories).
- **G7.** `sprint.yaml` read-modify-write shared across agents.
- **H1.** **No proactive "research affordance" on every interaction surface** — refined by reality-evidence-gate (2026-06-22): a dedicated `facilitation/evidence-research.md` pack EXISTS and ~10 packs route to research, so earlier drafts' claim of "zero facilitation packs mention research" was **overstated**. The verified gap is narrower: no pack's `open_floor`/input prompt proactively tells the human "you can say 'I don't know — research who does this and how, and recommend'" on every surface (targeted grep for that affordance phrase = 0 matches). The fix is to extend existing packs with a proactive affordance, not build research from zero. (The maintainer's #9 complaint, reframed.)
- **H2.** Clarifying-question UX isn't first-class (no batch, no quality gate).
- **H3.** No general teach/explain workflow.
- **H4.** Progress visibility is pull-based, not ambient.
- **H5.** Facilitation packs are **structurally excellent but tonally flat** — missing the "excited expert friend who matches your energy" presence the maintainer wants (#5).
- **D1.** No JSON schema / typed contract layer.
- **D2.** Anti-patterns are only 4 regexes; no multi-agent or autonomy-phase class.
- **D4.** No explicit inter-agent `agent-contract` artifact type.

---

## 5. Design Principles (the "sacred nos")

1. **Single-writer on the integration FSM.** Only the current *driver* mutates `state.yaml`/`sprint.yaml`/`stories`. Workers emit *requests*; the driver *applies*. (STORM: only the manager commits.)
2. **Append-only is the backbone.** `ledger.ndjson`, `index.ndjson` already are. Handoffs/checkpoints become append-only too (fix G2).
3. **Write-time conflict control over isolation for state.** Version counters + optimistic concurrency (STORM), not worktrees, for Forge state. Worktrees are for *code* (the product), not for Forge state.
4. **Workers never mutate integration state.** A worker wanting a phase/story change writes a `handoff-request` (append-safe); the driver applies it. (ESAA.)
5. **Lane = write boundary.** Each Product Area / story is a lane with one claimant at a time. Two agents never edit the same artifact. (Anthropic file-lock + git-push; the maintainer's #10 "lanes" intuition, formalized.)
6. **Hybrid FSM + events.** The FSM governs each agent *individually*; events govern collaboration *between* them.
7. **Runtime-agnostic by construction.** The coordination protocol is pure files. No runtime API calls in the coordination layer. (Forces the Forge App to be a first-class participant, not a special case.)
8. **Human-curated specs.** No agent writes GDD/PRD/mechanics/AGENTS.md without a human approval gate. (ETH Zurich.) Enforced, not advisory — the POC-converge-lock flow depends on it.
9. **Delegate tasks, not judgment.** Agents do scoped work with clear pass/fail. Humans keep architecture, "what NOT to build," taste, full-system review. (Addy Osmani.)
10. **Verification is the bottleneck, not generation.** Lean into gates, evals, evidence, reviewer subagents. Strengthen, never weaken.
11. **Chat for ambiguity; canvases (artifacts) for inspectable work.** A long chat thread is not state. Forge artifacts are the durable, inspectable surface. (AX.)
12. **Progressive Autonomy (the trust funnel).** Human-led early (POC → converge → lock spec); agent-led once locked (verification gates, not approval gates); **evolve loops back to human-led** for each new feature. The spec-lock is the handoff between modes.
13. **Commit-safe by construction.** Claims + trunk-based branch policy + CODEOWNERS-style review authority + reviewer gate + verification gates. No agent lands broken or conflicting work, ever — structurally, not by hope.
14. **Partner-grade presence.** The agent matches the human's energy, is an excited expert friend, shows previews ("how it'll look / work / be built"), and treats the conversation as creative collaboration — not a form to fill. (The maintainer's #5.)
15. **Research is always available — and the human knows it.** Every interaction surface reminds the human they can say "I don't know, research who does this and how, and recommend." The agent never guesses when it could research. (Fixes H1.)
16. **Grill closes blocks by default.** Before any handoff / phase transition / decision-lock, the agent runs a grill pass (one question at a time, recommended answer, check artifacts first) so no loose ends survive. (Fixes H5; uses Matt Pocock's grill-with-docs mechanic.)
17. **Pi ↔ Codex ↔ Claude ↔ OpenCode ↔ Forge App parity by construction.** One core, one protocol, one set of facilitation packs. A change lands once.
18. **One driver for the integration FSM across machines; CRDT convergence for coordination state.** Only the current driver writes `state.yaml`/`sprint.yaml` (the claim is a git-synced file); coordination state (registry, claims, completion-set, handoffs) converges via CRDT projection from an append-only, git-synced log — byte-identical across machines without a central writer. A `state.yaml` merge conflict is a typed signal of a driver-claim race, never a manual-resolution task. (grite, arXiv:2606.19616; STORM. Addendum §5, §10.)
19. **Locks alone are insufficient — track completion state too.** An arriving agent must see what is DONE, not just what is CLAIMED. The protocol carries both lane claims and task-completion records; only the combination drives redundant work to zero. (grite — the locks-only arm had the highest redundant-rediscovery rate. Addendum §11.)
20. **Notify, don't blindly abort.** When a write touches an agent's read-set, the runtime notifies and lets the agent judge whether its plan is invalidated — patching only the affected step, not restarting. The LLM can separate real conflicts from semantically-irrelevant interference, a capability classical transactions lack. (CoAgent, arXiv:2606.15376. Addendum §12.)

---

## 6. Proposed Architecture: "Forge v3 — Flock Coordination"

### 6.1 The big picture: `.forge-method/` as the coordination protocol

Just as `.git/` is the substrate every git client agrees on, `.forge-method/` becomes the substrate every flock participant (human, agent, runtime) agrees on. **Any runtime that reads/writes these files correctly is a first-class participant.** No runtime controls another; everyone follows the protocol.

```
                        ┌─────────────────────────────────┐
                        │  .forge-method/  (THE PROTOCOL) │
                        │  state.yaml {version}           │
                        │  agents/registry.yaml (fleet)   │
                        │  claims/<lane>.lock             │
                        │  handoffs/ · requests.ndjson     │
                        │  ledger.ndjson (event log)       │
                        │  artifacts/ · evidence/ · inputs/│
                        └─────────────────────────────────┘
           ┌──────────────────┬──────────────────┬───────────────┐
           ▼                  ▼                  ▼               ▼
   ┌──────────────┐   ┌──────────────┐   ┌────────────┐  ┌────────────┐
   │ Human A       │   │ Human B       │   │ Human C    │  │ Human D     │
   │ + Pi agent    │   │ + Codex chats │   │ + Claude   │  │ + Forge App │
   │ + subagents   │   │ + subagents   │   │   Code     │  │ (GPT/Opus/  │
   │               │   │               │   │ + subs     │  │  ZAI/...)   │
   └──────────────┘   └──────────────┘   └────────────┘  └────────────┘
```

Each box is independent — different machines, different runtimes, different models. They coordinate **only** through the protocol files (synced via git, same as code). No runtime commands another. This is what makes it scale to orgs and makes the Forge App a peer, not a special case.

### 6.2 Layered model

```
┌────────────────────────────────────────────────────────────────┐
│  LAYER 5 — RUNTIME ADAPTERS (swappable, not coupled)            │
│  Pi · Codex · Claude Code · OpenCode · Forge App (future)      │
│  Each: reads/writes the protocol + spawns its own agents        │
└────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────┐
│  LAYER 4 — PARTNER EXPERIENCE (model-agnostic behavior)         │
│  EARLY: POC-converge-lock · research-always-on · grill-closes   │
│         · match-energy · teach · clarifying-question batches    │
│  LATE:  /chronicle · ambient progress (async observer)          │
└────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────┐
│  LAYER 3b — PROGRESSIVE AUTONOMY (cyclic, per-feature)          │
│  Facilitated → Plan → Autopilot → (Evolve) → Facilitated        │
│  lock signal · verification gates (NOT approval gates) once locked │
└────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────┐
│  LAYER 3 — GUIDED ORCHESTRATION (flock-native)                  │
│  COUNCIL (the meeting): standup mode (status, cross-dep sync)   │
│    + decision mode (debate, dissent)                            │
│  ORCHESTRATION (separate): spawn workers within-runtime         │
│  build-story-work-order as typed contract · reviewer auto-trig  │
└────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────┐
│  LAYER 2 — FLOCK COORDINATION (append-safe, runtime-agnostic)   │
│  agents/registry.yaml · per-agent FSM snapshots                 │
│  claims/<lane>.lock (area + story) · handoff-requests.ndjson    │
│  intent annotations (glossary) · CODEOWNERS-style merge auth    │
└────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────┐
│  LAYER 1 — CONCURRENCY-SAFE STATE (STORM pattern)               │
│  state.yaml {version} + optimistic concurrency                  │
│  agent_id attribution on every ledger entry + mutating command  │
└────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────┐
│  LAYER 1.5 — CRDT PROJECTION (NEW, grite pattern)               │
│  materializes registry · claims · completion-set · handoff-reqs │
│  from the append-only log; byte-identical across machines       │
└────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────┐
│  SUBSTRATE — append-only event log (UNCHANGED)                  │
│  ledger.ndjson · coordination-log (NEW) · artifacts/index.ndjson│
└────────────────────────────────────────────────────────────────┘
```

### 6.3 Layer 1 — concurrency-safe state (the critical fix)

`state.yaml` gains an additive `version` field. Every read returns the version; every write declares `expected_version`. On mismatch → typed conflict error with current content + diff. Loser re-reads and retries. (STORM; tianpan.)

```python
# write_state (conceptual, backward-compatible)
def write_state(root, state, *, expected_version=None, agent_id="default"):
    current = read_state(root)
    if expected_version is not None and str(current.get("version","0")) != str(expected_version):
        raise StateConflict(current, diff(current, state))  # retryable, typed
    state["version"] = str(int(current.get("version","0")) + 1)
    write_flat_yaml(state_path(root), state)
    append_ledger(root, "state.written", {"version": state["version"], "agent_id": agent_id})
```

- **Backward compat:** `expected_version=None` (default) → v1.34.1 behavior (clobber). Existing single-agent users unaffected.
- **Migration (decided):** existing projects get `version: "0"` added on next `status`/`resume` — auto, once, in a Pi+Codex-compatible format (the core is byte-identical, so one migration works on both). **(Maintainer answer #3.)**
- **`--agent-id` flag** on all mutating commands; defaults to `"default"`. Every ledger entry carries it. Single-agent users see `"default"` — identical logs.

### 6.4 Layer 2 — flock coordination (lanes, claims, registry)

```
.forge-method/
├── state.yaml              # integration FSM (ONE writer at a time = current driver)
├── sprint.yaml             # integration sprint (driver writes)
├── ledger.ndjson           # append-only event log (ALL fleet agents, attributed)
├── agents/                 # NEW (presence = multi-agent mode opt-in)
│   ├── registry.yaml       # dynamic fleet roster (agents/flocks join & leave)
│   ├── <agent_id>.yaml     # per-agent FSM SNAPSHOT (write-by-owner)
│   └── ...
├── claims/                 # NEW (lanes — file-based coordination)
│   ├── <area>.lock         # Product Area lane: {agent_id, flock, ts, expires}
│   └── <story-id>.lock     # Story lane
├── handoffs/               # append-only batons (CHANGED: no longer mutate state)
│   └── *.md
├── requests.ndjson         # NEW: append-only worker→driver state-change requests
└── CODEOWNERS              # NEW (or .forge-method/owners): merge authority per area
```

**Fleet registry** (presence = opt-in; dynamic; **runtime-agnostic**):
```yaml
# .forge-method/agents/registry.yaml
driver: codex-main-alice        # current integration-state writer (a claim, reassignable)
flocks:                          # humans and their agents
  alice:
    runtime_hint: codex          # informational; protocol doesn't depend on it
    agents:
      - {agent_id: codex-main-alice, role: driver, areas: [game-build]}
      - {agent_id: codex-sub-alice-1, role: worker, areas: [game-build], parent: codex-main-alice}
  bob:
    runtime_hint: pi
    agents:
      - {agent_id: pi-main-bob, role: worker, areas: [art-direction]}
      - {agent_id: pi-sub-bob-1, role: worker, areas: [art-direction], parent: pi-main-bob}
lanes:                           # the work slots
  - {id: game-build, owner_role: driver, claimant: codex-main-alice}
  - {id: art-direction, owner_role: worker, claimant: pi-main-bob}
```

**Key points:**
- **`flocks`** group a human's agents. Org-scale: Alice's flock + Bob's flock + Carol's flock, all in one repo, each with their own runtime/model.
- **`runtime_hint` is informational.** The protocol never branches on it. A Pi agent, a Codex agent, a Claude Code agent, and a Forge App agent all follow the same rules. **(This is what makes Layer 5 swappable and the Forge App a peer.)**
- **`driver` is a claim, not a person.** Whichever agent holds the integration-write claim owns `state.yaml` writes *right now*. If it exits, the claim releases and another (or the human) reassigns. (Maintainer #1 confirmed.)
- **Lanes = the maintainer's #10 intuition, formalized.** An agent arriving: reads the lanes, picks an unclaimed one (claim), works; if all claimed, opens a new lane or waits; on exit, releases the lane + writes a handoff so the next arrival continues from where it stopped. Two granularities: Product Area lane and Story lane (prevents two agents on the same story).
- **Claims have TTL + heartbeat** (proposed: 30 min TTL, heartbeat on each write) so a crashed agent's lane auto-releases. (Maintainer #10 — "caso um agente pare de trabalhar, ele libera a lane." TTL=30min confirmed separately per reality-evidence-gate handoff.)
- **CODEOWNERS-style merge authority** per Product Area: who can approve a merge into `main` for that area. This is the commit-safety guarantee at the git layer — claims protect work-in-progress, CODEOWNERS protects the trunk. (Maintainer #2 — "sem ficar com o cu na mão de agente dar commit.")

**Handoffs become append-only (fix G2):** `cmd_handoff` no longer mutates `state.next_action`. It writes the baton `.md` AND appends a `handoff-request` to `requests.ndjson`. The driver polls and applies approved changes via a version-checked `transition`. Single-agent legacy mode (`--update-state=true`, default when no registry) preserves today's behavior.

### 6.5 Layer 3b — Progressive Autonomy + the Evolve Loop (cyclic)

Autonomy is **phase-coupled and cyclic per feature**, not a one-way line for the whole project. (Maintainer #4 — "o evolve provavelmente joga num fluxo inicial de novo.")

```
        ┌──────────────────────────────────────────────┐
        │  per FEATURE / per EPIC                      │
        ▼                                              │
   ┌─────────┐   POC converge   ┌─────────┐  lock  ┌──────────┐
   │FACILIT- │ ───────────────▶ │  PLAN   │ ─────▶ │ AUTOPILOT│
   │  ATED   │  human directs   │ human   │  spec  │ agents   │
   │ human   │  hard, iterates  │ reviews │  locked│ verify+  │
   │ decides │  on previews     │  plan   │        │ auto-fix │
   └─────────┘                  └─────────┘        └────┬─────┘
        ▲                                              │
        │            EVOLVE (new feature)              │
        └──────────────────────────────────────────────┘
                feature shipped → loop back to FACILITATED
```

| Mode | When | Human role | Gate type |
|---|---|---|---|
| **Facilitated** | discovery, spec, early plan, **and every evolve entry** | **Director** — decides heavily, iterates on POCs | **Approval** (visual-proof, GDD gates, council) |
| **Plan** | POC convergence | **Reviewer** — approves the plan | Plan-approval (`build-story-work-order`) |
| **Autopilot** | build-verify of *locked* work | **Observer** — async, reads `/chronicle` | **Verification** (tests, lint, E2E, code-review, evals) |

**The lock signal.** A spec/PRD/GDD that passed its gate is *locked*. Recorded as `spec.locked {artifact, agent_id, ts}` in the ledger. Every agent knows downstream work is autonomous. The PRD-complete state is the canonical lock (Maintainer #4 — "depois de tudo concordado, criar o PRD completo pronto pra autonomia").

**The verification-gate loop (Autopilot).** Once locked, build never asks "may I?" — it asks "did it pass?":
```
claim a locked story → implement → run verification gate:
  ├─ tests pass?       no → auto-fix (bounded) → re-run
  ├─ lint/types pass?  no → auto-fix → re-run
  ├─ E2E pass?         no → auto-fix → re-run
  ├─ code-review (reviewer subagent) pass?  no → address findings → re-run
  └─ evals pass?       no → auto-fix or escalate
all pass → mark done + evidence → release lane → next story
escalate to human ONLY when: gate fails AND retries exhausted AND cannot self-recover
```
(Maintainer #7 confirmed: 3 retries/then escalate; auto-fix touches **implementation only**, never spec — a spec gap re-enters the human in Facilitated mode.)

**The Evolve Loop.** When the product ships and a feature is added, the evolve phase (already in Forge) **restarts the feature in Facilitated mode** — full interview, facilitation, POC. The autonomy doesn't persist forever; it's earned per-feature by locking a spec, and re-earned for the next feature. (Maintainer #4 confirmed.)

**Anti-pattern this forbids:** an agent asking for approval during Autopilot on locked work. Encoded: *"do not request human approval for work whose spec is locked; run verification gates instead."*

**Human re-entry triggers:** gate fails beyond budget / agent detects a spec gap / any agent raises `correct-course` / human interrupts.

### 6.6 Layer 4 — partner-grade experience (the identity, protected & extended)

**The tone shift (Maintainer #5):** facilitation packs gain an explicit **presence directive** — the agent is an *excited expert friend who matches the human's energy*, listens to what they want, and is eager to show how it can be done and how it'll look. Not a bureaucratic interviewer. This is a behavior spec any capable model can follow (model-agnostic), layered onto the existing rich pack structure.

**Research always available (fix H1, Maintainer #9):** the runtime already has a dedicated `evidence-research.md` facilitation pack and ~10 packs that route to research — the gap is NOT absence of research support (earlier drafts overstated this). The verified gap is that no pack's `open_floor`/input prompt proactively carries the affordance on every interaction surface. The fix: every interaction surface — system prompt, facilitation open_floor, input prompts — carries an explicit affordance: *"At any point you can say 'I don't know — research who does this, how, what succeeds, new trends, and tell me your recommendation.' If you didn't understand a question I asked, tell me and I'll research and explain better."* The agent never guesses when it could research; the human never feels trapped by their own uncertainty. (Extension of existing packs, not greenfield — reality-evidence-gate 2026-06-22.)

**Grill closes blocks by default (fix H5, Maintainer #9):** wire Matt Pocock's grill-with-docs state machine (S0→S6: one question at a time, recommended answer, check artifacts/code first, update glossary inline) as the **default gate before any handoff / phase transition / decision-lock**. No block closes with loose ends. Forge already has `workflow-grill-gate.md` (semi-default in phases 1/2/3 today); the fix is to make it fire at *every* decision-close point, not just early phases — extending the trigger, not building from zero. (Refined by reality-evidence-gate 2026-06-22.)

**Early-phase (human-led) suite:**
- **POC-converge-lock** as the default exploration mode (the maintainer's working habit, made first-class): agent builds a preview → shows "how it'll look / work / be built" → human iterates → on convergence, agent documents the decided direction and patches GDD/PRD/mechanics/sprint. The accepted visual-proof is the lock signal.
- **Clarifying-question batching** (fix H2): `input add --batch-of N` + question-quality gate (cites known/unknown/blocking).
- **Teach/explain** (fix H3): a general `teach` workflow — the agent explains its reasoning/decisions while it works (Amazon Science "interrogability").

**Build-phase (agent-led, async) suite:**
- **`/chronicle`** durable summary (AX): reads ledger + checkpoints + evidence → "what happened while you were away."
- **Ambient progress** (fix H4): `chronicle` + `sprint-status` as a push-style brief; human checks every 5–10 min, never hovers (Addy factory model).

### 6.7 Layer 5 — runtime adapters (swappable)

The protocol is runtime-agnostic; each runtime is a thin adapter that (a) reads/writes the protocol files and (b) spawns its own agents within its own capabilities. **No runtime commands another.**

| Runtime | Spawns via | Reads protocol via | Status |
|---|---|---|---|
| **Pi** | `subagent` tool (worktree:true for code) | vendored skill | ✅ now |
| **Codex** | native multi-agent + chats | plugin | ✅ now |
| **Claude Code** | Agent Teams / Task tool | AGENTS.md + protocol files | 🟢 easy (emit AGENTS.md → protocol) |
| **OpenCode** | CLI subagents | config + protocol files | 🟢 easy |
| **Forge App (future)** | native harness | native | 🟢 designed-in (it's the reference adapter) |

**Why expansion is easy:** the only runtime-specific code is "how do I spawn an agent here?" Everything else — registry, claims, handoffs, requests, facilitation, gates, progressive autonomy — is pure files any runtime reads. Claude Code/OpenCode integration = emit an `AGENTS.md`/config that points them at the protocol and teaches them the claim/handoff conventions.

### 6.8 Multi-agent & autonomy anti-patterns encoded (fix D2)
Extend the write-time guidance-safety patterns with the new classes:
- "do not write integration state without holding the driver claim"
- "do not write integration state without checking `expected_version`"
- "do not persist a worker transcript as integration memory"
- "do not act outside your claimed lane"
- "do not write spec/context (GDD/PRD/AGENTS.md) without a human-approval gate"
- "do not request human approval for work whose spec is locked; run verification gates instead"
- "do not guess when research is available"

Leverages the runtime's *existing* guardrail engine — write-time enforcement, zero new infra.

---

## 7. Compatibility: the runtime matrix

- **Core is byte-identical v1.34.1** on Pi and Codex (diff-confirmed). Any change lands **once** in `forge-method-core`.
- The new flags (`--agent-id`, `--expected-version`) and files (`agents/registry.yaml`, `claims/`, `requests.ndjson`) are host-agnostic. A Pi worker and a Codex worker hand off through them with no bridge.
- **Claude Code / OpenCode:** add an `AGENTS.md` (and CLAUDE.md for Claude) generated from the protocol + facilitation packs. They read the protocol, claim lanes, write handoffs. No deep integration. (Aligns with 2026 AGENTS.md convention; note ETH Zurich — the AGENTS.md must be **human-curated/approved**, so Forge emits a *draft* the human approves.)
- **Forge App:** designed-in from day one — it's the reference adapter. Multi-provider (GPT/Opus/ZAI/...) is invisible to the protocol because facilitation is model-agnostic behavior.
- **Worker spawning differs by host** but the **coordination artifacts are identical.** Council records which host ran which worker.

---

## 8. Roadmap (incremental: A → B → C, as agreed)

### Phase A — Design & RFC (this document)
- [x] Deep research (12+ queries, 3 deep-dives).
- [x] Codebase audit (`forge-runtime-audit.md`).
- [x] RFC v1, v2, **v3 (this — flock-scale reframe + Forge App direction + partner experience + grill/research fixes + evolve loop)**.
- [ ] **Maintainer sign-off on v3.**
- [ ] Publish RFC to `forge-method-core` repo.

### Phase B — POC: validate ALL 20 v2 design principles through a minimal prototype

**Maintainer correction (2026-06-23, see `20260623-spec-correction-phase-b-poc-framing.md`):** the original "use v1.34.1 as-is, find gaps" framing only re-confirms what `forge-runtime-audit.md` already documented — it does not validate whether the v2 principles actually work. You cannot validate v2 without implementing it. The POC commits to **all 20 principles (1-20)**, not just the latest additions 18/19/20 — those three were the most recent deep-research additions, but the maintainer's decision is whether to commit to the entire v2 design and re-enter Phase 1 for it. Principles 18/19/20 interact with the earlier 17 in ways the papers did not study as a combined system; several earlier principles (#2 append-only, #7 runtime-agnostic, #10 verification, #16 grill-default) are only partially present in v1.34.1 and need validation under multi-agent stress + the new substrate.

Build the **minimum POC** needed to exercise all 20 principles under real multi-agent load. Two complementary layers:

- **Code substrate POC** (state/coordination principles #1-#7, #13, #17-#20): prototypical implementations of `agent_id` attribution, `version` field + optimistic concurrency, append-only handoffs, claim primitive with TTL, fleet registry, CRDT projection. Validates the principles that need code substrate to be exercisable.
- **Behavior/facilitation POC** (partner-experience principles #8, #9, #11, #14, #15, #16): research-always-on affordance, grill-as-default at every decision-close point, match-energy, clarifying-question batching, teach/explain. Tested via facilitation packs + prompts (no code substrate required).

Then run concurrent agents THROUGH the POC:
- [ ] Run `team-operating-model` → declare driver + worker flocks on a chosen validation target (flexible — mutant-run-horde-lab is one example, NOT a binding dependency).
- [ ] Run `product-area-map` → lane boundaries.
- [ ] Run `trunk-based-plan` → branch policy + CODEOWNERS-style merge authority (the commit-safety layer).
- [ ] Stress-test concurrent agents THROUGH the POC; observe which principles hold, which break, and what the 12+ papers didn't predict (replicate grite metrics: dup-work rate, conflicting edits, throughput).
- [ ] Test the **partner experience:** does the agent match energy, offer research proactively, grill-close blocks at every decision-close? (Independently testable from the code substrate.)
- [ ] Output: `gap-report.md` with a POC verdict — **accept** (proceed to Phase 1 re-entry), **iterate** (rework specific principles, re-POC), or **reject** (design unsound; re-open spec).

### Phase C — Candidate backlog for the Phase 1 re-entry (NOT the direct next step)

**Maintainer correction (2026-06-23, see `20260623-spec-correction-phase-b-poc-framing.md`):** the original framing went straight from Phase B gap-report to Phase C implementation in core. That bypassed this RFC's own §6.5 + Principle 12 (Evolve Loop = restart feature in Facilitated mode — full interview, facilitation, POC). v2 is a large enough layer to warrant the full Forge Method flow, not a direct code-drop.

**Correct sequence after a POC-accept verdict:**
1. Forge Method re-routes **evolve → Phase 1 (discovery)** for v2 as a new layer (this is what the runtime should do per the design — see defect `evolve-reentry-routing-gap`, logged at commit `391d99b`).
2. Full **interview → PRD → architecture** cycle, using the v2 RFC + POC gap-report as primary inputs.
3. Then **build** the architecture-confirmed priorities.

The 13-step list below becomes a **candidate backlog** that feeds the Phase 1 interview/PRD/architecture — it will be re-prioritized, possibly restructured, during Phase 1. It is NOT the direct next step after Phase B.

**Candidate backlog (was Phase C; now input to Phase 1 re-entry), ordered by impact ÷ effort from the audit:**

1. **R2:** `agent_id` attribution + `--agent-id` flag (HIGH/LOW). **Likely ships first.**
2. **R1:** `version` field + optimistic-concurrency opt-in + **auto-migration of existing projects** (CRITICAL/LOW). Highest leverage.
3. **R3:** `handoff`/`checkpoint` append-only via `--update-state` flag (CRITICAL/MED).
4. **R6:** encode multi-agent + autonomy anti-patterns in guidance safety (HIGH/LOW).
5. **R4:** `agents/registry.yaml` (flock-native) + per-agent state (CRITICAL/MED).
6. **R5:** `claims/` lanes (area + story) + claim check + TTL/heartbeat (HIGH/MED).
7. **H1:** research-always-available affordance across facilitation + system prompt (HIGH/LOW). **The maintainer's #9 fix — high value, low effort.**
8. **H5:** grill-as-default block-closing gate + partner-grade presence directive in packs (HIGH/MED).
9. **D4/R7:** typed `agent-contract` artifact + `contract-check` eval (HIGH/MED).
10. **G5 (split):** (a) **council as a meeting** — add `standup` mode (status + cross-dependency surfacing + hard-problem sharing + overlap coordination) distinct from the existing `decision` mode; (b) **orchestration as spawning** — make `agent-team`/`parallel`/`subagent` actually spawn within-runtime. These were conflated under "council" in v1–v2; they are different things (meeting ≠ spawning). (HIGH/HIGH). Flagship.
11. **6.7 Evolve Loop:** wire evolve phase to restart features in Facilitated mode (MED/MED).
12. **D1:** JSON schema for workflows/templates (MED/MED).
13. **AGENTS.md emitter** for Claude Code / OpenCode integration (MED/LOW — unlocks those runtimes).

Each ships as a versioned, backward-compatible bump. Existing projects ignore the new files/flags → v1.34.1 behavior.

---

## 9. Risks & Mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| Adding `version` changes `state.yaml` shape | HIGH | Additive key; `read_flat_yaml` ignores unknown keys. Auto-migrate is one-time, Pi+Codex-compatible. |
| Making handoff/checkpoint non-mutating changes behavior | HIGH | Default `--update-state=true` preserves legacy; only fleet mode sets false. |
| `--agent-id` noise for single-agent users | MED | Optional, `"default"` fallback. `FORGE_AGENT_ID` env injects for fleets. |
| Per-agent state fragments the source of truth | MED | `state.yaml` stays authoritative integration state; per-agent files are WIP. Driver reconciles on merge. |
| Ledger grows unbounded under fleets | LOW | Rotation/snapshot as non-breaking follow-up. |
| Org-scale: merge conflicts across flocks | MED | Trunk-based policy + short-lived lanes + CODEOWNERS + reviewer gate. Conflicts caught at PR, not at codebase ruin. |
| A flock's agent goes rogue (bad commits) | MED | Branch protection on `main`; agents push to feature branches only; reviewer gate + verification gates before merge. "Com o cu na mão" eliminated by structure. |
| Runtime drift (Pi vs Codex vs Claude vs ...) | LOW | Protocol is pure files; core is byte-identical. CI diffs the core across runtimes. |
| Over-engineering: solo users don't want this | MED | Everything opt-in via `agents/registry.yaml` presence. Default = single-agent v1.34.1. |
| AGENTS.md auto-generation harms agents (ETH Zurich) | MED | Forge emits a *draft* AGENTS.md; human must approve at a gate before it's canonical. Never auto-applied. |
| Partner-tone directive misfires on weak models | LOW | It's behavior guidance, not a hard constraint. Weak models produce flatter tone but still function. |

---

## 10. Open Questions (resolved + remaining)

**Resolved by maintainer (this round):**
- ✅ **#1** Driver = reassignable claim (whoever holds integration-write claim owns state now).
- ✅ **#2** Vision = flock coordination at org-scale, trunk-based-equivalent; runtime-agnostic (Forge App is a peer, not special).
- ✅ **#3** Migrate `version` to existing projects, Pi+Codex-compatible.
- ✅ **#4** Progressive Autonomy is cyclic — evolve loops back to Facilitated per feature.
- ✅ **#5** Partner-grade tone: excited expert friend, matches energy.
- ✅ **#6, #7** Autonomy phase-coupled; 3 retries then escalate; auto-fix = implementation only.
- ✅ **#9** Research-always-available affordance + grill-as-default block-closer (both confirmed real gaps).
- ✅ **#10** Lanes pattern = claims + handoff (formalized the maintainer's intuition).
- ✅ **Forge App direction** = runtime-agnostic-by-construction; Claude Code/OpenCode are easy extensions.

**Remaining (need maintainer decision before the relevant Phase C step):**
1. **Cross-runtime spawning:** confirmed *not needed* (agents don't command across machines). But: should council *suggest* "a Pi agent could do X" and let the human spawn it manually, or stay silent? (Proposed: suggest, with a one-click hand-off draft the human approves.)
2. **Claim TTL & heartbeat default:** proposed 30 min TTL + heartbeat-on-write. Confirm.
3. **Flock discovery:** how does a new agent learn it's part of a flock? Proposed: env `FORGE_FLEET=on` + `FORGE_AGENT_ID` + `FORGE_FLOCK=<human-id>` set by the spawner. Confirm.
4. **Naming:** `agents/registry.yaml` (proposed) vs alternatives. Confirm.
5. **AGENTS.md emission scope:** emit for Claude Code + OpenCode in Phase C step 13, or hold for post-v1? (Proposed: emit, but human-approved-gated.)
6. **`/chronicle` scope:** lightweight (ledger + checkpoints) or rich (+ artifact diffs, unresolved inputs)?
7. **Lock override:** can the human manually force-lock / force-unlock a spec outside a gate? (Proposed: yes, via a `forge lock --force` / `forge unlock` with a durable input recording why.)

---

## 11. References (selected)

**Multi-agent orchestration (2026)**
- Liu et al., *Multi-agent Collaboration with State Management (STORM)*, arXiv:2605.20563 — shared-workspace + write-time consistency > worktree isolation. **Both figures valid:** +18.7/+1.4 headline averages; +34.6 on high-coupling-code subset (where GitWorktree breaks down). The foundational insight, independently validated by grite (below).
- AgentMarketCap, *Five Vendors, Four Weeks* + *The Two-Week Window* — vendor convergence on parallel agents.
- Addy Osmani, *The Code Agent Orchestra* (O'Reilly AI CodeCon, Mar 2026) — delegate-tasks-not-judgment; verification bottleneck; Ralph Loop; factory model.
- egesabanci/agent-collab, moranbickel/Peer-Worker-Convergence — open protocols for multi-agent repo work.

**Human-in-the-loop & AX (2026)**
- Alto / GitHub, *Agent Experience (AX)* — legibility, auditability, context persistence; canvases vs chat; session modes.
- Amazon Science, *Designing UX for agentic AI: human-AI coordination.*
- design@tive, *Make Reasoning Interrogable* + *Shape Dialogue Patterns.*
- codeongrass, *3-Tier Risk Framework* — operation-specific autonomy tiers.
- Anthropic, *2026 Agentic Coding Trends Report* — devs "fully delegate" only 0–20%.

**Agent-facing docs & contracts (2026)**
- GitHub Spec Kit + Microsoft, *Spec-Driven Development.*
- stewie-sh/pbc-spec, derive-build/derive-spec, sno-ai/mda, dcaponi/agentic-app-spec.
- Addy Osmani, *How to write a good spec for AI agents.*
- Gloaguen et al. (ETH Zurich), *Evaluating AGENTS.md* (arXiv:2602.11988) — LLM-written context reduces success.

**Distributed primitives (foundational)**
- tianpan, *Multi-User Shared Agent State* — optimistic concurrency + event sourcing.
- munderdifflin, *Append-only event log for a hive of agents* — single committer.
- agentpatterns.ai, *File-Based Agent Coordination* — Anthropic case study, file locks + git-push.
- ESAA, *Event Sourcing for Agents* — agents emit events, orchestrator applies effects.

**Agent interoperability protocols (2025-2026) — the layer Forge sits above**
- Google → Linux Foundation, *Agent2Agent (A2A) Protocol* (v1.0.1, May 2026; a2a-protocol.org) — agent-to-agent delegation via JSON-RPC/HTTP; opacity principle (agents collaborate without exposing internal state). Forge is compatible by construction; it occupies the coordination/governance layer A2A declines.
- Anthropic, *Model Context Protocol (MCP)* (modelcontextprotocol.io) — model/app ↔ data/tools. "USB-C for AI." Complementary; Forge agents use MCP for tools.

**Multi-agent at scale — independent validation (2026)**
- Sarkar (ASU), *grite* (arXiv:2606.19616) — append-only event log in git + CRDT projection + advisory leases; N=32 agents, 78%→0% duplicate work, 3× throughput, byte-identical convergence. THE validation of Forge's architecture.
- Khatua/Zhu/Tran (Stanford/SAP), *CooperBench* (arXiv:2601.13295) — two-agent cooperation 25% vs 50% solo; coordination, not coding, is the bottleneck.
- Lyu et al. (SJTU), *CoAgent* (arXiv:2606.15376) — notify-don't-lock-or-abort; LLM-as-conflict-judge.
- Guo/Wang/Chen, *SyncMind* (arXiv:2502.06994) — the out-of-sync failure mode.
- Shopify Engineering, *Under the River* (2026-05-28) — durable event log as substrate; 59,918 sessions, 3,536 PRs/30 days.

**Trunk-based development (foundational, the analogy)**
- Trunk-based development literature — short-lived branches, small PRs, CODEOWNERS, required checks, merge authority. The model Forge extends to the human+agent world.

---

## Appendix A — What this RFC does NOT propose
- A new server, daemon, or database. Everything is files.
- Forcing multi-agent/fleet on anyone. Opt-in per project.
- Rewriting the state machine. Preserved and strengthened.
- Removing the human from the loop. The opposite: Progressive Autonomy *concentrates* human involvement where it matters (decisions) and withdraws it where it doesn't (locked build).
- Cross-runtime control of one agent by another. Agents coordinate through the protocol only; no runtime commands another.
- Letting agents write specs unsupervised. Explicit anti-pattern.
- Coupling to any runtime or model. The protocol is runtime+model-agnostic by construction (enables the Forge App).
- **Automating human-human governance.** Forge governs agent-agent and human-agent coordination within the protocol. Human-human governance (planning ceremonies, priority disputes, org structure) remains a human process, supported by git/CODEOWNERS/PR review. Forge records human governance outcomes; it never decides them.

## Appendix B — What changes for a single-agent existing user
**Nothing.** No `agents/registry.yaml` → no version checks, no claims, no registry, legacy `--update-state=true`. Byte-compatible with v1.34.1. The load-bearing guarantee for not breaking existing users mid-development.

## Appendix C — The one-paragraph pitch
> Forge Method is the open, runtime-agnostic coordination protocol for the human+agent world — what trunk-based development is for human teams, extended to teams where every human operates a fleet of agents across any runtime (Pi, Codex, Claude Code, OpenCode, or the Forge App). It guarantees commit-safety by construction (claims, branch policy, reviewer gates, verification gates), scales from one person to whole orgs, and pairs that structural rigor with a partner-grade guided experience: an excited expert friend who researches when asked, shows previews, closes blocks cleanly, and gets out of the way once the spec is locked — then loops back to guided mode for every new feature.
