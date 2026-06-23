# RFC: Guided Multi-Agent for Forge Method (v2)

- kind: design-rfc
- status: draft-for-maintainer-review
- author: human (Daniel Carvalhal) + Pi agent
- created_at: 2026-06-22
- related: `.forge-method/artifacts/forge-runtime-audit.md` (codebase audit)
- applies_to: `forge-method-core` (github.com/DanielCarva1/forge-method-core), consumed byte-identical by Pi and Codex runtimes
- runtime_baseline: v1.34.1
- revision: v2 — adds Progressive Autonomy (POC → converge → lock → autonomous build) and Fleet-Native coordination (N agents across both runtimes)

> One-line thesis: **Make Forge safely multi-agent without breaking its identity as a guided, human-facilitated runtime — by adding a thin concurrency-safe layer on top of the existing append-safe event log, not by rewriting the state machine.**
>
> Two-line thesis (v2): **The human directs hard at the start (POC, converge, lock spec); agents execute autonomously at the end (verify, build, review, E2E, auto-fix). The spec-lock gate is the handoff from human-led to agent-led. Coordination handles a dynamic fleet of N agents across both runtimes, not a static pair.**

---

## 1. TL;DR

Forge already has the right bones: an append-only event log (`ledger.ndjson`), sophisticated write-time guidance guardrails, rich human facilitation packs, and a conceptual multi-agent vocabulary (`team-operating-model`, `product-area-map`, `collaboration-handoff`, `council-decision`). The one missing piece is **concurrency safety on the mutable state** (`state.yaml`, `sprint.yaml`, `stories/*.yaml`), which today is a full-overwrite with zero locking — a silent last-writer-wins trap for any second agent.

This RFC proposes **four additive, opt-in, backward-compatible changes** that unlock safe multi-agent without touching the single-agent experience:

1. **Version field + optimistic concurrency check** on `state.yaml` (write-time conflict control, STORM-pattern).
2. **Agent attribution** (`agent_id`) on every ledger entry and mutating command.
3. **Append-only handoffs/checkpoints** (stop clobbering `next_action`; workers emit *requests*, the driver *applies*).
4. **Agent registry + product-area claims** (isolated per-agent FSM snapshots; file-lock coordination).

Everything is gated on the *presence* of an `agents/registry.yaml`. If absent → v1.34.1 single-agent behavior, byte-for-byte. Existing users are untouched.

The design deliberately keeps Forge **guided and human-facilitated** (agents ask, research, teach) rather than a silent autonomous pipeline. Multi-agent is *opt-in per project*, surfaced through facilitation, not forced.

---

## 2. Problem & Constraints

### 2.1 The user's real situation
- Two Forge runtimes on one project: one customized for **Codex** (plugin), one customized for **Pi** (vendored skill). Both must keep using Forge.
- Agents from **both** runtimes should work on the **same project concurrently** while Forge orchestrates and documents everything.
- The runtime is **shared open-source** (`forge-method-core`); many other people use it on both sides. Changes must not break them mid-development.
- Forge is fundamentally a **guided, human-facilitated** runtime — not a silent autonomous orchestrator. Its identity is: agents ask good questions, do research, guide the human, teach, and produce well-organized, high-quality artifacts. Multi-agent must amplify that, not erode it.

### 2.2 Hard constraints (non-negotiable)
- **C1. Preserve each agent's state machine.** The FSM is the soul of Forge. Multi-agent must not flatten or bypass it.
- **C2. Backward-compatible.** A project with no multi-agent opt-in behaves exactly as v1.34.1. Zero breaking changes for existing users mid-flight.
- **C3. Pi ↔ Codex parity.** Both runtimes consume the same `.forge-method/` and the same core. A change lands once.
- **C4. Opt-in, facilitated.** Multi-agent is surfaced when it makes sense, through dialogue with the human — never automatic/silent.
- **C5. Quality packaging intact.** Gates, evals, decision-source traceability, guidance-safety guardrails all keep working and get *stronger*, not weaker.

### 2.3 The blocker (confirmed by code audit)
`write_flat_yaml` (`scripts/forge_method_runtime.py:857`) is `path.write_text(...)` — a full overwrite. There is **no flock, no FileLock, no version field, no optimistic-concurrency check** anywhere in the 16,851-line runtime (grep-confirmed). Every state-mutating command (`transition`, `story_start`, `handoff`, `checkpoint`, `council_run`, `correct_course`, `input_answer`) follows `load_state → mutate dict → write_state`. **Two concurrent agents here = silent data loss.** This is the only true blocker.

---

## 3. State of the Art (2026 research)

Researched 12+ queries across orchestration, human-in-the-loop UX, agent-facing docs, and quality/evals. Full citations in §11. Headline findings:

### 3.1 Multi-agent orchestration — the STORM revolution
**The single most important discovery.** Liu et al. (2026), *"Multi-agent Collaboration with State Management"*, arXiv:2605.20563.

- Empirically compared on Commit0-Lite + PaperBench across Sonnet 4.6, Qwen 3.6, DeepSeek V4.
- **STORM (shared workspace + write-time consistency check) beats git-worktree isolation**: +18.7 on Commit0-Lite, +1.4 on PaperBench.
- GitWorktree **collapses** under high file-coupling: 36.3% vs STORM 70.9% (+34.6 pts). It's the *only* method that doesn't break down as coupling rises.
- **Key insight (refutes my earlier recommendation):** agents do *not* need a frozen snapshot of the whole workspace. They only need the files they've actually **read** to remain unchanged while they reason. This is *"local state consistency."*
- Mechanism: every file has a monotonic version counter. A write is valid **iff** every file the agent read still has the version it observed. On conflict → reject with a unified diff + stale-dependency list, agent retries from a fresh baseline.
- **Only the manager commits.** Engineers share one workspace, coordinate through intent annotations + write-time checks.
- Scales to 8 engineers with **constant wall-clock time** (parallel). Scaling is limited by *decomposition quality*, not the framework.

> **Implication for Forge:** my earlier "git worktree per Product Area" recommendation is the *weaker* pattern. The better design is **shared `.forge-method/` + write-time consistency checks on the mutable state files**, with version counters and optimistic concurrency. Worktrees remain useful for *code* isolation (the game prototype), but Forge's own state should be coordinated the STORM way.

### 3.2 The vendor convergence (Feb–May 2026)
In ~4 weeks, every major coding agent shipped the same architecture (AgentMarketCap):
- Cursor → agents in own VMs + browser.
- Claude Code → **Agent Teams** (2–16 sessions, shared task list + dependency tracking + peer messaging + file locking).
- Devin → manages other Devins (coordinator + isolated VMs).
- Windsurf → side-by-side Cascade panes.
- Codex → App with built-in worktrees + multi-agent v2 spec.

> **Convergent primitives:** shared task list with dependency tracking, peer-to-peer messaging (not all-through-lead), file locking, plan-approval gates, worktree isolation for code. Forge's existing `council-decision` modes (`parallel`, `agent-team`, `subagent`) name these — but currently execute nothing (orchestration theater, see audit G5).

### 3.3 Human-guided experience (the identity we must protect)
- **Addy Osmani, "Code Agent Orchestra" (O'Reilly AI CodeCon, Mar 2026):** *"Delegate the tasks, not the judgment."* Agents excel at scoped work with tight evaluation functions. Humans keep: architecture, "what NOT to build," reviewing with full-system context, taste. **"The bottleneck has shifted from generation to verification."** → Forge's gate/eval/correct-course machinery is exactly the verification infrastructure that matters.
- **Ralph Loop (Huntley/Carson):** stateless-but-iterative — pick task → implement → validate → commit → reset context → repeat. Continuity via *external* memory (git, task state file, AGENTS.md). **This is structurally identical to the Forge loop** (story → build → verify → evidence → checkpoint). Forge already *is* a Ralph-Loop-compatible runtime.
- **ETH Zurich (Gloaguen et al., 2026):** LLM-generated `AGENTS.md` files **reduce** success ~3% and **raise** inference cost 20%+. Developer-written context files give ~+4%. → **Anti-pattern: never let an agent write spec/context without human approval.** Forge's facilitated + gated + correct-course flow already protects this; we must keep specs human-curated (or human-approved-at-a-gate).
- **Amazon Science + design@tive "interrogability":** the best agentic UX *teaches while it works* and exposes reasoning as inspectable (not just explainable after the fact). Forge has the bones (council transcript, evidence, checkpoint) but no general "teach/explain" workflow.
- **3-tier risk framework (codeongrass):** operation-specific (not agent-specific) autonomy tiers — lightweight confirm / plan-preview+approval / full autonomy with audit. Maps cleanly onto Forge's `autonomy_mode` + per-workflow `do_not_prompt`.

### 3.4 Agent Experience (AX) — the long-running collaboration lens
**Valentina Alto / GitHub Copilot app (Build 2026):** AX is the design discipline for *persistent* agent collaborators. UX optimizes screens; Agent UX optimizes conversations; **AX optimizes collaboration over days** along four axes: legibility, auditability, context persistence, accountability.

AX primitives GitHub ships (relevant to Forge):
- **My Work** control centre (active sessions, PRs, automations in one view).
- **Git worktrees** for isolation (per-session branch).
- **Canvases** — bidirectional work surfaces where agent updates and human steers on the same surface.
- **Plan before act** — agent proposes plan; human reviews/edits before code changes.
- **Agent Merge** — monitors CI, reviewers, failing checks; configurable auto-green.
- **Session modes:** Interactive · Plan · Autopilot (change mid-session).
- **`/chronicle`** — durable summary across app/CLI/sessions.
- **Rubber duck agent** — separate model adversarially reviews plan/impl/tests.

> **The key AX maxim:** *chat is for instruction & ambiguity; canvases are where intent becomes inspectable work.* A long chat thread of corrections fails once an agent runs for hours. Forge's artifacts (GDD, PRD, sprint-plan, story-work-order, evidence) ARE canvases. This is a strength to lean into.

### 3.5 Agent-facing documentation & contracts (2026)
- **GitHub Spec Kit** — `main.md` structured Markdown with embedded formal defs (data model, API shapes, state machines, scenarios). Four-phase compile. Microsoft backs SDD as the AI-native alignment layer.
- **PBC (.pbc.md)** — Markdown-first lintable behavior contract; start fluent, formalize over time.
- **MDA Open Spec** — one `.mda` source compiles to SKILL.md / AGENTS.md / MCP-SERVER.md / CLAUDE.md. YAML frontmatter with `doc-id`, `version`, `requires`, `depends-on`, `relationships`. JSON-Schema validated + Sigstore-signed.
- **Addy Osmani "how to write a good spec for AI agents":** structured Markdown with explicit boundaries ("never touch X"), named anti-patterns, escalation paths, rationale, worked examples.
- **Context engineering (LoadSys/WeBuild-AI):** the *how* — delivering the right info in the right format at the right time. SDD is the *what*.

> **Implication for Forge:** workflow references are already structured (7 required sections + compactness caps) — rare discipline. The gap is they're *not typed* (no JSON schema) and anti-patterns are minimal (4 regexes). Aligning with PBC/MDA/Spec-Kit direction = machine-checkable contracts.

---

## 4. Audit of Forge Today (summary — full detail in `forge-runtime-audit.md`)

### 4.1 Strengths to preserve (S1–S6)
- **S1.** Append-safe event log (`ledger.ndjson`, `artifacts/index.ndjson` use `open("a")` POSIX-atomic). **This is the multi-agent foundation.**
- **S2.** Guidance-safety guardrails enforced at *write time* (4 anti-patterns checked on every `write_state`, story, input, review, artifact index). Rare and valuable.
- **S3.** Facilitation packs are genuinely strong human-UX (34 packs, rich schema: `open_floor`, `elicitation_options`, `facilitator_moves`, `quality_bar`, `anti_patterns`, `paths`).
- **S4.** Workflows are machine-validated with compactness limits (7 required sections, max lines/words/bullets).
- **S5.** Collaboration vocabulary already exists (`team-operating-model`, `product-area-map`, `trunk-based-plan`, `collaboration-handoff`, `repo-split-plan`, `council-decision` with `parallel`/`agent-team`/`subagent` modes).
- **S6.** Decision-source traceability (stories cite `decision_sources`; gate blocks done stories lacking a source).

### 4.2 Gaps (G1–G7, D1–D4, H1–H4)
**Multi-agent blockers:**
- **G1 (CRITICAL):** `state.yaml` write is full-overwrite, zero concurrency control.
- **G2 (CRITICAL):** `handoff` and `checkpoint` *mutate* `state.yaml.next_action` — workers clobber the driver.
- **G3 (CRITICAL):** no agent registry / per-agent state.
- **G4 (HIGH):** no owner/agent attribution in ledger or stories.
- **G5 (HIGH):** council orchestration is descriptive, not executable (modes are labels, no real worker spawning).
- **G6 (MED):** no claim/lock primitive for Product Areas.
- **G7 (MED):** `sprint.yaml` is read-modify-write shared across agents.

**Human-experience gaps:**
- **H1:** research is advisory, not enforced ("research before asking" is a runtime option, not a guarantee).
- **H2:** clarifying-question UX isn't a first-class runtime primitive (no question-quality gate, no batch-N mechanic).
- **H3:** no general "teach/explain" workflow beyond `teach-testing`.
- **H4:** progress visibility is pull-based, not ambient.

**Agent-doc gaps:**
- **D1:** no JSON schema / typed contract layer.
- **D2:** anti-patterns are only 4 regexes; no multi-agent anti-pattern class encoded.
- **D3:** workflow `handoff:` section is unstructured prose.
- **D4:** no explicit inter-agent `agent-contract` artifact type.

---

## 5. Design Principles (the "sacred nos")

Distilled from research + identity:

1. **Single-writer on the integration FSM.** Only the designated *driver* mutates `state.yaml`/`sprint.yaml`/`stories`. Workers emit *requests*; the driver *applies*. (STORM: only the manager commits.)
2. **Append-only is the backbone.** `ledger.ndjson`, `index.ndjson` already are. Handoffs/checkpoints must become append-only too (fix G2).
3. **Write-time conflict control over isolation.** Prefer STORM's optimistic concurrency (version counters) over git-worktree isolation for *state*. Worktrees are for *code* (the game), not for Forge state. (3.1)
4. **Workers never mutate integration state.** A worker wanting a phase/story change writes a `handoff-request` (append-safe); the driver polls and applies. (ESAA: "agents emit validated events; orchestrator applies effects.")
5. **Product Area = write boundary.** Each area has one owner with a claim. Two agents never edit the same artifact. (Anthropic file-lock + git-push pattern.)
6. **Hybrid FSM + events.** The FSM governs each agent *individually*; events govern collaboration *between* them. (aiagentautomation.site.)
7. **Human-curated specs.** No agent writes GDD/PRD/mechanics/AGENTS.md without a human approval gate. (ETH Zurich: LLM-written context reduces success.) Forge's gate+correct-course already enforces this; keep it sacred.
8. **Delegate tasks, not judgment.** Agents do scoped work with clear pass/fail. Humans keep architecture, "what NOT to build," taste, full-system review. (Addy Osmani.)
9. **Opt-in and facilitated.** Multi-agent surfaces only when the human accepts it, through facilitation dialogue. Default is single-agent v1.34.1.
10. **Verification is the bottleneck, not generation.** Lean into gates, evals, evidence, reviewer subagents. Strengthen, never weaken.
11. **Chat for ambiguity; artifacts (canvases) for inspectable work.** A long chat thread is not state. Forge artifacts are the durable, inspectable surface. (AX.)
12. **Pi ↔ Codex parity by construction.** One core, one change, both runtimes.
13. **Progressive Autonomy (the trust funnel).** Human involvement is phase-dependent, not constant. Early phases (discovery/spec/plan) are human-led and facilitated: POC → verify direction → iterate → converge → lock spec. Late phases (build-verify of *locked* work) are agent-led and autonomous: the loop runs verification gates (tests/review/E2E), auto-adjusts on failure, and escalates to the human only on unrecoverable failure. The spec-lock gate is the explicit handoff from approval-mode to autopilot-mode. (AX session modes Interactive→Plan→Autopilot; Addy factory model; Ralph Loop.)
14. **Verification gates, not approval gates, in the build phase.** Once a story/PRD is locked, downstream build never asks "may I?" — it asks "did it pass?" Gates are tests, lint, E2E, code review (by a reviewer subagent), and evals. Failure → auto-fix → retry, bounded. Human re-enters only when a gate fails AND the agent cannot self-recover.
15. **Fleet-native coordination.** The unit is not "Codex vs Pi" — it is a dynamic fleet of N agents: a Codex main chat plus its subagents plus its other chats, a Pi main chat plus its subagents, all sharing one `.forge-method/`. Agents register on start, claim work, release claims, deregister on exit. The driver role is about *who owns integration-state writes right now*, not a fixed org chart. Any agent from any runtime can be a worker; the integration state has exactly one writer at a time. (STORM scales to 8 engineers sharing one workspace; this generalizes to a cross-runtime fleet.)

---

## 6. Proposed Architecture: "Forge v2 — Guided Multi-Agent"

### 6.1 Layered model

```
┌────────────────────────────────────────────────────────────────┐
│  LAYER 4 — HUMAN EXPERIENCE (AX, phase-aware)                  │
│  EARLY: POC-converge-lock · research-before-asking · teach      │
│  LATE:  /chronicle · ambient progress (async observer)          │
└────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────┐
│  LAYER 3b — PROGRESSIVE AUTONOMY (the trust funnel)             │
│  Interactive → Plan → Autopilot · lock signal · verification    │
│  gates (NOT approval gates) once spec is locked                 │
└────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────┐
│  LAYER 3 — GUIDED ORCHESTRATION (fleet-native)                  │
│  council-decision (REAL spawn, cross-runtime) · agent-contract  │
│  build-story-work-order as typed contract · reviewer auto-trig  │
└────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────┐
│  LAYER 2 — COORDINATION (append-safe)                           │
│  agents/registry.yaml · per-agent FSM snapshots                 │
│  claims/<area>.lock · handoff-requests.ndjson · intent annotations │
└────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────┐
│  LAYER 1 — CONCURRENCY-SAFE STATE (STORM pattern)               │
│  state.yaml {version} + optimistic concurrency                  │
│  agent_id attribution on every ledger entry + mutating command  │
└────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────┐
│  SUBSTRATE — append-only event log (UNCHANGED)                  │
│  ledger.ndjson · artifacts/index.ndjson · evidence/ · inputs/   │
└────────────────────────────────────────────────────────────────┘
```

### 6.2 Layer 1 — concurrency-safe state (the critical fix)

**`state.yaml` gains an additive `version` field.** Every read returns the version; every write must declare `expected_version`. On mismatch → typed conflict error with the current content + a diff of what changed. The losing agent re-reads and retries. (Exact STORM mechanism; tianpan's optimistic-concurrency recommendation.)

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

- **Backward compat:** `expected_version=None` (the default for all existing callers) → behaves exactly as v1.34.1 (clobber). Existing single-agent users unaffected.
- **Multi-agent callers** pass `--expected-version` (CLI) or the env `FORGE_STATE_VERSION`. A conflict surfaces as a typed, retryable error — never silent loss.

**`--agent-id` flag** added to all mutating commands; defaults to `"default"`. Every `append_ledger` entry carries `agent_id`. Enables forensics ("who did this and when?"). Single-agent users see `"default"` everywhere — identical logs.

### 6.3 Layer 2 — coordination (fleet-native, append-safe)

```
.forge-method/
├── state.yaml              # integration FSM (ONE writer at a time = current driver)
├── sprint.yaml             # integration sprint (driver writes)
├── ledger.ndjson           # append-only event log (ALL fleet agents, attributed)
├── agents/                 # NEW (presence = multi-agent mode opt-in)
│   ├── registry.yaml       # dynamic fleet roster (agents join/leave at runtime)
│   ├── <agent_id>.yaml     # per-agent FSM SNAPSHOT (one file per agent, write-by-owner)
│   └── ...                 # e.g. codex-main, codex-sub-1, codex-chat-2, pi-main, pi-sub-1
├── claims/                 # NEW (file-based coordination, Anthropic + STORM pattern)
│   ├── <area>.lock         # Product Area claim: {agent_id, ts, expires}
│   └── <story-id>.lock     # Story claim: prevents two agents on the same story
├── handoffs/               # append-only batons (CHANGED: no longer mutate state)
│   └── *.md
└── requests.ndjson         # NEW: append-only worker→driver state-change requests
```

**Fleet registry** (presence = opt-in; dynamic — agents register/deregister at runtime):
```yaml
# .forge-method/agents/registry.yaml
driver: codex-main          # current integration-state writer (one at a time)
fleet:
  - agent_id: codex-main
    runtime: codex
    role: driver            # owns state.yaml/sprint.yaml writes THIS moment
    product_areas: [game-build]
    state_file: agents/codex-main.yaml
    joined: "2026-06-22T20:01Z"
  - agent_id: codex-sub-1
    runtime: codex
    parent: codex-main      # spawned by codex-main
    role: worker
    product_areas: [game-build]
    state_file: agents/codex-sub-1.yaml
  - agent_id: pi-main
    runtime: pi
    role: worker
    product_areas: [art-direction]
    state_file: agents/pi-main.yaml
  - agent_id: pi-sub-1
    runtime: pi
    parent: pi-main
    role: worker
    product_areas: [art-direction]
    state_file: agents/pi-sub-1.yaml
```
If this file is absent → v1.34.1 single-agent behavior. **Zero migration required for existing users.**

**Dynamic join/leave:** `forge agent register --id <id> --runtime <pi|codex> --role <driver|worker> --area <id>` appends to the roster; `forge agent deregister --id <id>` removes it. Subagents register with a `parent` pointer. The roster is append-corrected (each change is a ledger event + registry rewrite by the driver only).

**Driver role is a claim, not a person.** `driver:` points at whichever agent currently holds the integration-write claim. If that agent exits, the claim is released and another agent (or the human) reassigns it. This keeps the single-writer invariant (STORM: only the manager commits) while allowing the fleet to be dynamic.

**Per-agent FSM snapshot:** each agent keeps its *own* phase/status/next_action for *its own* work in `agents/<agent_id>.yaml`, written only by that agent. The driver reconciles these into the integration `state.yaml` on merge. Two agents never write the same YAML file.

**Claims (two granularities):** `forge claim --area art-direction --agent-id pi-main` (Product Area lock) and `forge claim --story p003 --agent-id codex-sub-1` (story lock). State/story-mutating commands check: if a registry exists and the calling `--agent-id` doesn't own the relevant claim → reject with a typed error. Claims expire (default TTL, e.g. 30 min) so a crashed agent doesn't hold work forever; renewal is a heartbeat. (Anthropic file-lock + git-push pattern; STORM reservation to prevent ping-pong.)

**Handoffs become append-only (fixes G2):** `cmd_handoff` no longer does `state["next_action"] = ...; write_state(...)`. Instead it writes the handoff `.md` (the baton) AND appends a `handoff-request` line to `requests.ndjson`:
```json
{"ts":"...","agent_id":"pi-art-direction","kind":"handoff","next_action_proposed":"Start art-direction sprint","product_area":"art-direction"}
```
The driver polls `requests.ndjson` and applies approved changes via a normal version-checked `transition`. **Single-agent legacy mode** (`--update-state=true`, default when no registry) preserves today's behavior.

### 6.4 Layer 3 — guided orchestration (make council real)

- **`council-decision` modes `agent-team`/`parallel`/`subagent` actually spawn workers** (fix G5). On Pi, via the `subagent` tool (worktree:true for code isolation). On Codex, via its native multi-agent. The council artifact records participants, dissent, merge contract, and the spawned workers' outputs.
- **`build-story-work-order` promoted to a typed `agent-contract`** (fix D4): declares `agent_id`, `permitted_write_paths[]`, `permitted_commands[]`, `product_area`, `dependencies[]`, `merge_contract`, `merge_owner`, `acceptance`. A new `contract-check` eval validates an agent's actions against its contract.
- **Reviewer subagent wired into the loop** (AX "rubber duck" / Addy "@reviewer teammate"): every story completion triggers a read-only review before the driver marks `done`. Forge already has `quality-reviewer` profile — formalize the auto-trigger.

### 6.5 Layer 4 — human experience (protect & extend the identity)

This layer is phase-aware (see §6.7 Progressive Autonomy). In early phases it is rich and facilitated; in the build phase it shifts to ambient/async.

**Early-phase (human-led) — make the guidance *spectacular*, as the maintainer demands:**
- **POC-driven convergence as the default exploration mode.** The maintainer's habit — "POC everything, verify direction, iterate until convinced, THEN document" — becomes a first-class flow. Forge already has `visual-alignment-prototype`, `quick-prototype`, `visual-proof-before-prd`, and `correct-course`. Promote these into a smooth `poc-converge-lock` stage: agent builds a preview → shows "how it'll look / how it'll work / how it'll be built" → human iterates → on convergence, agent documents the decided direction and patches GDD/PRD/mechanics/sprint-plan to match. The runtime treats an accepted visual-proof as the lock signal.
- **Research-before-asking enforced** (fix H1): a workflow option `require_research: true` that refuses to open a human-input until at least one research artifact exists for the topic. The agent comes to the human *informed*, never cold.
- **Clarifying-question batching** (fix H2): `input add` gains a `--batch-of N` mode and a question-quality gate (does the question cite what's known, what's unknown, what's blocking?).
- **Teach/explain workflow** (fix H3): a general `teach` workflow that explains the agent's current reasoning/decisions to the human — not just `teach-testing`. The agent teaches while it works (Amazon Science "interrogability").

**Build-phase (agent-led, async) — keep the human oriented without blocking them:**
- **`/chronicle` durable summary** (AX): a new `chronicle` command that reads `ledger.ndjson` + recent checkpoints + evidence and produces a "what happened while you were away" brief. Bridges the doomscrolling gap when N agents run overnight.
- **Ambient progress** (fix H4): `chronicle` + `sprint-status` produce a push-style brief. The human monitors every 5–10 min (Addy factory model), never hovers.
- **Plan-before-act** stays available but is *optional* in autopilot — a story whose contract is already locked can proceed straight to execution.

### 6.6 Multi-agent anti-patterns encoded (fix D2)
Extend `WORKFLOW_MISLEADING_GUIDANCE_PATTERNS` (currently 4) with the multi-agent class, enforced at write time:
- "do not write integration state without holding the driver role"
- "do not write integration state without checking `expected_version`"
- "do not persist a worker transcript as integration memory"
- "do not act outside your declared Product Area"
- "do not write spec/context (GDD/PRD/AGENTS.md) without a human-approval gate"

This leverages the runtime's *existing* excellent guardrail engine — write-time enforcement, zero new infra.

### 6.7 Progressive Autonomy — the human→agent trust funnel (cross-cutting)

This is the maintainer's core working insight, made structural. Autonomy is **not a fixed setting** — it is a function of **how locked the spec is**. The runtime progresses through three modes, mirroring AX session modes (Interactive → Plan → Autopilot):

| Mode | When | Human role | Gate type | Forge phases |
|---|---|---|---|---|
| **Interactive / Facilitated** | discovery, spec, early plan | **Director** — decides heavily, iterates on POCs | **Approval gates** (visual-proof, GDD gates, council) | 0-route, 1-discovery, 2-specification, 3-plan |
| **Plan** | POC convergence | **Reviewer** — approves the plan before execution | **Plan-approval gate** (build-story-work-order) | late 3-plan |
| **Autopilot** | build-verify of locked work | **Observer** — async, monitors chronicle | **Verification gates** (tests, lint, E2E, code-review, evals) | 4-build-verify |

**The lock signal.** A story/PRD/GDD that passed its gate is *locked*. Locking is the explicit handoff from human-led to agent-led. Concretely: `status: accepted` on a visual-proof, a passed GDD gate, or an approved `build-story-work-order` flips the relevant work into Autopilot. The runtime records the lock as a ledger event (`spec.locked {artifact, agent_id, ts}`) so every agent knows downstream work is autonomous.

**The verification-gate loop (Autopilot).** Once locked, the build loop never asks "may I?" — it asks "did it pass?":
```
pick locked story (via claim) → implement → run verification gate:
  ├─ tests pass?       no → auto-fix (bounded retries) → re-run
  ├─ lint/types pass?  no → auto-fix → re-run
  ├─ E2E pass?         no → auto-fix → re-run
  ├─ code-review (reviewer subagent) pass?  no → address findings → re-run
  └─ evals pass?       no → auto-fix or escalate
all pass → mark done + evidence → release claim → next story
escalate to human ONLY when: gate fails AND retries exhausted AND cannot self-recover
```
This is the Ralph Loop (Huntley/Carson) made explicit: stateless-but-iterative, continuity via external memory (git, ledger, story state, AGENTS.md). Forge's existing `build-story` + `gate` + `evidence` + `quality-reviewer` already implement most of it — the addition is making the loop **autonomous by default once locked**, with the reviewer subagent auto-triggered.

**Why this protects the identity instead of eroding it.** The maintainer worried multi-agent would dilute the "guided, human-facilitated" soul. The opposite is true under Progressive Autonomy: the guidance is **concentrated where it matters** (early, when direction is uncertain) and **withdrawn where it doesn't** (late, when the spec is locked and asking permission just adds latency). The human gets a *spectacular* guided experience exactly when they're making decisions, and gets out of the way exactly when they'd be a bottleneck. This is "delegate tasks, not judgment" (Addy Osmani) realized as a runtime invariant.

**Anti-pattern this forbids.** An agent asking for permission during Autopilot (on already-locked work) is a guidance-safety violation: it re-introduces an approval gate where a verification gate was agreed. Encode it: "do not request human approval for work whose spec is locked; run verification gates instead."

**Human re-entry triggers (escape hatches).** Autopilot is not unconditional. The human is pulled back in when: (a) a verification gate fails beyond retry budget, (b) an agent detects a *spec gap* (the locked spec doesn't cover this case → it's a discovery-phase question, not a build-phase one), (c) a `correct-course` is raised by any agent, or (d) the human explicitly interrupts. These are durable inputs, same as today.

---

## 7. Compatibility: Pi ↔ Codex

- The core (`forge_method_runtime.py`) is **byte-identical v1.34.1** on both (diff-confirmed). Any change lands **once** in `forge-method-core`; both adapters consume it.
- The new `--agent-id` and `--expected-version` flags are CLI-level → work identically whether invoked by a Pi agent or a Codex agent.
- Worker-spawning differs by host (Pi uses its `subagent` tool; Codex uses native multi-agent), but the **coordination artifacts** (`agents/registry.yaml`, `claims/`, `requests.ndjson`, council decision) are host-agnostic Markdown/YAML/NDJSON. A Pi worker and a Codex worker can hand off to each other through these files.
- **Risk:** host-specific worker semantics. **Mitigation:** the runtime defines the *contract* (what a worker must emit/claim/hand-off); the host decides *how* to spawn. Council records which host ran which worker.

---

## 8. Roadmap (incremental, A → B → C, as agreed)

### Phase A — Design & RFC (this document)
- [x] Deep research (12+ queries, 3 fetched deep-dives).
- [x] Codebase audit (`forge-runtime-audit.md`).
- [x] This RFC draft.
- [ ] **Maintainer review & iteration** (you). Open questions in §10.
- [ ] Publish RFC to `forge-method-core` repo for traceability.

### Phase B — Prototype on mutant-run-horde-lab (no core changes yet)
Use *only what Forge already has* to validate the model in the real world and discover what's *actually* missing before touching the core:
- [ ] Run `team-operating-model` → artifact declaring Pi (art-direction, worker) + Codex (game-build, driver).
- [ ] Run `product-area-map` → `art-direction` vs `game-build` with owners/contracts/paths/checks.
- [ ] Run `trunk-based-plan` → branch policy (worktree for the game code).
- [ ] Establish the **single-driver convention** by hand (Pi art agent writes only artifacts/inputs/handoffs; Codex owns state).
- [ ] Stress-test: have both agents run concurrently, observe where it hurts.
- [ ] Output: a `gap-report.md` listing exactly which Layer-1/2 features would have prevented each pain.

### Phase C — Core implementation (validated, additive)
Sequenced by impact ÷ effort (from audit top-10):
1. **R2:** `agent_id` attribution on ledger + `--agent-id` flag (HIGH impact, LOW effort). **Ship first.**
2. **R1:** `version` field + optimistic-concurrency opt-in (CRITICAL, LOW). The highest-leverage change.
3. **R3:** make `handoff`/`checkpoint` append-only via `--update-state` flag (CRITICAL, MED). Removes the biggest clobber vector.
4. **R6:** encode multi-agent anti-patterns in guidance safety (HIGH, LOW).
5. **R4:** `agents/registry.yaml` + per-agent state (CRITICAL, MED). The core isolation primitive.
6. **R5:** `claims/` file-lock + claim check (HIGH, MED).
7. **D4/R7:** typed `agent-contract` artifact + `contract-check` eval (HIGH, MED).
8. **G5:** make council `agent-team`/`parallel` actually spawn (HIGH, HIGH). Flagship.
9. **D1:** JSON schema for workflows/templates (MED, MED).
10. **H1:** enforced research-before-asking option (MED, MED).

Each ships as a versioned, backward-compatible bump. Existing projects ignore the new files/flags and behave as before.

---

## 9. Risks & Mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| Adding `version` changes `state.yaml` shape | HIGH | Additive key only. `read_flat_yaml` ignores unknown keys. Verified. |
| Making handoff/checkpoint non-mutating changes behavior | HIGH | Default `--update-state=true` preserves legacy; only multi-agent mode sets false. |
| `--agent-id` noise for single-agent users | MED | Optional everywhere, `"default"` fallback. Env `FORGE_AGENT_ID` injects for multi-agent setups. |
| Per-agent state fragments the source of truth | MED | `state.yaml` stays authoritative integration state; per-agent files are WIP snapshots. Driver reconciles on merge. |
| Ledger grows unbounded under multi-agent | LOW | Rotation/snapshot as a separate non-breaking follow-up (munderdifflin notes this). |
| Pi ↔ Codex drift | LOW | One core, byte-identical. Diff-gated in CI. |
| Over-engineering: users don't want multi-agent | MED | Everything opt-in via `agents/registry.yaml` presence. Default experience unchanged. Facilitation asks "is this worth parallelizing?" before enabling. |
| Workers fight over the same story (duplicate work) | MED | `claims/` lock + `story_start` ownership check. STORM-style write-time rejection. |

---

## 10. Open Questions for the Maintainer (you)

These need your decision before Phase C:

1. **Driver election:** should the driver be declared statically in `registry.yaml` (my proposal), or elected dynamically (e.g., first-claimer)? Static is simpler and safer; dynamic is more flexible.
2. **Cross-runtime worker spawning:** should `council-decision` try to spawn a worker on the *other* runtime (e.g., Pi council asks Codex to run a worker), or stay within-runtime and let humans bridge? Cross-runtime is powerful but couples the runtimes.
3. **Version field migration:** add `version: "0"` to existing `state.yaml` files on next `status`/`resume` (auto-migrate), or only to new projects? Auto-migrate is smoother; opt-in is safer.
4. **Spec human-approval enforcement:** make the "no agent writes GDD/PRD without a gate" an *enforced* anti-pattern (blocks the write), or keep it *advisory* (facilitation warns)? Enforced is stronger but could frustrate solo-agent users.
5. **`/chronicle` scope:** lightweight (read ledger + checkpoints) or rich (also diff artifacts, surface unresolved inputs)? Rich is more useful but heavier.
6. **Default autonomy tier for multi-agent:** should workers default to Plan mode (must propose before acting) rather than the current auto-mode? AX evidence favors Plan-default for delegation.
7. **Naming:** `agents/registry.yaml` vs `.forge-method/team.yaml` vs something else? Affects discoverability.

---

## 11. References (selected)

**Multi-agent orchestration (2026)**
- Liu et al., *Multi-agent Collaboration with State Management (STORM)*, arXiv:2605.20563 — the central insight: shared-workspace + write-time consistency > worktree isolation.
- AgentMarketCap, *Five Vendors, Four Weeks: How February 2026 Killed the Sequential Coding Agent* — vendor convergence.
- Addy Osmani, *The Code Agent Orchestra* (O'Reilly AI CodeCon, Mar 2026) — subagents → agent teams → orchestration; quality gates; Ralph Loop; delegate-tasks-not-judgment.
- *Anthropic multi-agent coordination patterns* (Apr 2026, via rel8.pl) — 5 patterns + decision matrix + failure modes. *(Note: deep-fetch blocked by provider rate limits; to verify against primary source.)*
- egesabanci/agent-collab, moranbickel/Peer-Worker-Convergence — open protocols for multi-agent repo work.

**Human-in-the-loop & AX (2026)**
- Alto / GitHub, *Agent Experience (AX)* — legibility, auditability, context persistence, accountability; canvases vs chat.
- Amazon Science, *Designing UX for agentic AI: human-AI coordination* — coordination as the core UX challenge.
- design@tive, *Make Reasoning Interrogable* + *Shape Dialogue Patterns* — interrogability, clarification as workflow.
- codeongrass, *3-Tier Risk Framework* — operation-specific autonomy tiers.
- Anthropic, *2026 Agentic Coding Trends Report* — developers "fully delegate" only 0–20% of tasks.

**Agent-facing docs & contracts (2026)**
- GitHub, *Spec Kit* + Microsoft, *Spec-Driven Development* — structured Markdown specs as the alignment layer.
- stewie-sh/pbc-spec, derive-build/derive-spec, sno-ai/mda, dcaponi/agentic-app-spec — typed contract formats.
- Addy Osmani, *How to write a good spec for AI agents* — boundaries, anti-patterns, escalation paths.
- Gloaguen et al. (ETH Zurich), *Evaluating AGENTS.md* (arXiv:2602.11988) — LLM-written context *reduces* success.

**Distributed-systems primitives (foundational)**
- tianpan, *Multi-User Shared Agent State* — optimistic concurrency + event sourcing.
- munderdifflin, *Append-only event log for a hive of agents* — one JSONL, single committer.
- agentpatterns.ai, *File-Based Agent Coordination* — Anthropic case study, file locks + git-push.
- ESAA, *Event Sourcing for Agents* — agents emit events, orchestrator applies effects.

---

## Appendix A — What this RFC does NOT propose
- A new server, daemon, or database. Everything is files on disk (Forge's existing substrate).
- Forcing multi-agent on anyone. It's opt-in per project, surfaced through facilitation.
- Rewriting the state machine. The FSM is preserved and strengthened.
- Removing the human from the loop. The opposite: multi-agent *increases* the need for plan-approval, review, and chronicle (AX).
- Letting agents write specs unsupervised. Explicitly an anti-pattern (ETH Zurich).

## Appendix B — What changes for a single-agent existing user
**Nothing.** No `agents/registry.yaml` → no version checks, no claims, no registry, legacy `--update-state=true` on handoff/checkpoint. Byte-compatible with v1.34.1. This is the load-bearing guarantee for not breaking your other users mid-development.
