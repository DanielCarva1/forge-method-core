# Collaboration Handoff — Forge Method v2.0 Evolution

- kind: collaboration-handoff
- created_at: 2026-06-22
- origin: mutant-run-horde-lab (opened in wrong workspace by mistake)
- destination: forge-method-core repo (DanielCarva1/forge-method-core) — local working copy at `/mnt/c/Users/Danie/OneDrive/Documentos/ody`
- **STATUS: the 4 files below are ALREADY PLACED in this repo's `.forge-method/artifacts/`.** You do NOT need to copy anything. Read them and continue.
- owner: Daniel Carvalhal (maintainer)
- next_actor: a fresh agent session in this (forge-method-core / ody) workspace
- branch: (create when ready) `feat/forge-v2-flock-coordination`
- pull_request: (none yet)
- product_area: forge-method-core / runtime evolution
- first_command: read the 3 artifacts below → confirm context → then run `grill-gate` on the RFC to close the 7 open questions one at a time

---

## Why this handoff exists

The maintainer and a Pi agent did a deep design session for **Forge Method v2.0** (multi-agent / flock coordination), but it happened in the **mutant-run-horde-lab** workspace by mistake. All design artifacts were written there and have now been **transplanted into this repo (forge-method-core / `ody`)** under `.forge-method/artifacts/`. This handoff preserves the session context so a fresh agent in the right workspace can continue without replaying the chat.

---

## What was produced (the artifacts — ALREADY in `.forge-method/artifacts/` here)

All four files live in this repo's `.forge-method/artifacts/`. Read them there:

1. **`forge-flock-coordination-rfc-v3.md`** (43KB) — THE design RFC. Supersedes v1/v2. Contains: vision (Forge as trunk-based-development equivalent for human+agent flocks), 17 sacred design principles, 7-layer architecture, Progressive Autonomy (cyclic, with Evolve Loop), runtime-agnostic protocol (Pi/Codex/Claude/OpenCode/Forge-App), 13-step Phase C roadmap, risks, and 7 open questions.
2. **`forge-runtime-audit.md`** (19.6KB) — Read-only codebase audit of forge-method-core v1.34.1. File:line evidence for: 6 strengths to preserve, 3 CRITICAL multi-agent blockers (state overwrite G1, handoff clobbers state G2, no agent registry G3), 4 HIGH gaps, plus gaps in human-experience (H1-H5) and agent-docs (D1-D4). 7 ranked refactor opportunities + top-10 recommendations.
3. **`forge-multi-agent-design-rfc.md`** (44KB) — RFC v2 (historical). Superseded by v3 but kept for traceability of how the thinking evolved.

**Do not lose any of these.** They are the entire accumulated design context.

---

## The big picture (what v2.0 is, in 5 lines)

1. **Forge becomes the open, runtime-agnostic coordination protocol for human+agent flocks** — what trunk-based development is for human teams, extended to teams where every human operates a fleet of agents (across Pi/Codex/Claude/OpenCode/the future Forge App).
2. **Commit-safe by construction:** claims (lanes), trunk-based branch policy, CODEOWNERS merge authority, reviewer gate, verification gates. No agent ever lands broken/conflicting work. The maintainer's "sem ficar com o cu na mão."
3. **Progressive Autonomy (cyclic):** human directs hard early (POC → converge → lock spec), agents run autonomously once locked (verification gates, not approval gates), and **evolve loops back to directed mode** for each new feature. Autonomy is earned per-feature by locking a spec.
4. **Runtime + model agnostic:** the coordination protocol is pure files (YAML/Markdown/NDJSON). No runtime controls another. The future Forge App (own harness, GPT/Opus/ZAI) is a first-class peer. Expansion to Claude Code/OpenCode is easy via `.agents/skills/SKILL.md` convention (confirmed: OpenCode reads it).
5. **Partner-grade experience:** the agent matches the human's energy, is an excited expert friend, always offers research when the human is unsure, and grills to close blocks (Matt Pocock's grill-with-docs S0→S6 as default gate).

---

## Resolved decisions (the maintainer confirmed these)

- ✅ **#1** Driver = a reassignable claim (whoever holds integration-write claim owns state right now).
- ✅ **#2** Vision = flock coordination at org-scale; runtime-agnostic; Forge App is a peer.
- ✅ **#3** Migrate `version` field to existing projects, Pi+Codex-compatible format.
- ✅ **#4** Progressive Autonomy is cyclic — evolve loops back to Facilitated per feature.
- ✅ **#5** Partner-grade tone: excited expert friend, matches energy.
- ✅ **#6, #7** Autonomy phase-coupled; 3 retries then escalate; auto-fix = implementation only (never spec).
- ✅ **#9** Research-always-available affordance (confirmed missing by grep) + grill-as-default block-closer (Matt Pocock mechanic, currently opt-in must become default).
- ✅ **#10** Lanes pattern = claims + handoff, with TTL (30 min, confirmed) + heartbeat-on-write.
- ✅ **TTL = 30 min** confirmed (lane claim auto-expires if not renewed; crashed agent's work auto-releases).
- ✅ **Council clarification:** "council" = THE MEETING (standup: status + cross-dependency sync + hard-problem sharing; decision: debate with dissent map). It is NOT orchestration/spawning — those are separate concepts the v1/v2 RFC wrongly conflated. Both still needed.
- ✅ **Forge-installable-as-runtime per harness** (not just AGENTS.md emission): Pi (skill), Codex (plugin), OpenCode (`.agents/skills/`), Claude Code (`.agents/skills/` + CLAUDE.md), Forge App (native). Confirmed OpenCode reads `.agents/skills/SKILL.md`.

---

## The 7 open questions STILL UNRESOLVED (must resolve before Phase C)

These are in RFC §10. The maintainer has NOT decided them yet. **Run `grill-gate` to close them one at a time** (this also demonstrates fix #9 working live):

1. **Cross-runtime spawning:** confirmed agents don't command across machines. But: should council **suggest** "a Pi agent could do X" and let the human spawn it manually, or stay silent? (Proposed: suggest + one-click hand-off draft.)
2. **Claim TTL & heartbeat default:** proposed 30 min TTL + heartbeat-on-write. (Confirm — maintainer already said 30 min OK.)
3. **Flock discovery mechanism:** how does a new agent learn it's part of a flock? Proposed: env `FORGE_FLEET=on` + `FORGE_AGENT_ID` + `FORGE_FLOCK=<human-id>` set by the spawner. (Confirm.)
4. **Naming:** `agents/registry.yaml` (proposed) vs alternatives. (Confirm.)
5. **AGENTS.md emission scope:** emit for Claude Code + OpenCode in Phase C, or hold? (Proposed: emit, human-approved-gated — never auto-applied, per ETH Zurich finding.)
6. **`/chronicle` scope:** lightweight (ledger + checkpoints) or rich (+ artifact diffs, unresolved inputs)? (Decide.)
7. **Lock override:** can the human manually force-lock / force-unlock a spec outside a gate? (Proposed: yes, via `forge lock --force` with durable input recording why.)

---

## Critical next steps (in order — follow Forge's own rules)

### Phase A — finish the spec (the files are now in this repo; just continue here)
- [x] **Run `grill-gate` on the RFC** to close the 7 open questions (one at a time, recommended answer, check artifacts first). This is also the live demo of fix #9. **DONE 2026-06-22** → all 7 have recommended answers grounded in constraints/evidence/code; 2 flagged as judgment calls (Q2 TTL configurability, Q6 chronicle richness) for maintainer confirm; 1 additional citation fix (A1, applied). See `20260622-grill-gate-forge-v2-open-questions.md`.
- [x] Run `reality-evidence-gate` on the v2.0 product claims (is flock coordination really needed? is STORM evidence sound?). **DONE 2026-06-22** → stance PLAUSIBLE→STRONG conditional; 3 citation fixes applied to RFC v3 (STORM +18.7/+1.4 not +34.6; H1/#9 reframed — research packs exist, gap is the proactive affordance; grill-gate reframed to "not default at every decision-close point"). See `20260622-reality-evidence-gate-forge-v2.md`.
- [ ] (transplant step is DONE — files are already in place)

### Phase B — empirical validation (validation target is flexible — mutant-run-horde-lab is one example, NOT a binding dependency; the v2 evolution is independent of that game project)
Use ONLY what Forge already has, to discover empirically what's missing:
- [ ] Run `team-operating-model` → declare driver (Codex, game-build) + worker (Pi, art-direction).
- [ ] Run `product-area-map` → `game-build` vs `art-direction` lanes.
- [ ] Run `trunk-based-plan` → branch policy + CODEOWNERS-style merge authority.
- [ ] Stress-test concurrent agents; observe where it hurts.
- [ ] Test the **partner experience** (research affordance, grill-close, match-energy) — testable NOW without core changes.
- [ ] Produce `gap-report.md` → feeds Phase C priorities.

### Phase C — implement in forge-method-core (validated, additive, backward-compatible)
Ordered by impact ÷ effort (from audit top-10). **Each ships as a versioned bump; existing projects ignore new files → v1.34.1 behavior.**
1. `agent_id` attribution + `--agent-id` flag (HIGH/LOW) — ship first.
2. `version` field + optimistic concurrency + auto-migrate existing projects (CRITICAL/LOW).
3. `handoff`/`checkpoint` append-only via `--update-state` flag (CRITICAL/MED).
4. Encode multi-agent + autonomy anti-patterns in guidance safety (HIGH/LOW).
5. `agents/registry.yaml` (fleet-native) + per-agent state (CRITICAL/MED).
6. `claims/` lanes (area + story) + claim check + TTL/heartbeat (HIGH/MED).
7. Research-always-available affordance across facilitation + system prompt (HIGH/LOW). **The maintainer's #9 fix.**
8. Grill-as-default block-closing gate + partner-grade presence directive (HIGH/MED).
9. Typed `agent-contract` artifact + `contract-check` eval (HIGH/MED).
10. (split) (a) Council as a meeting — add `standup` mode; (b) orchestration spawning within-runtime (HIGH/HIGH). Flagship.
11. Evolve Loop wiring (MED/MED).
12. JSON schema for workflows/templates (MED/MED).
13. OpenCode/Claude Code integration: Forge as `.agents/skills/` skill + custom tools (MED/LOW).

---

## Constraints (non-negotiable, from maintainer)

- **C2 Backward-compatible:** existing users (many people, on both Pi and Codex) must not break mid-development. A project with no opt-in = v1.34.1 behavior, byte-compatible.
- **C3 Runtime-agnostic:** the protocol works on Pi, Codex, Claude Code, OpenCode, future Forge App. No runtime controls another.
- **C4 Model-agnostic:** works on GPT/Opus/ZAI/anything capable. Facilitation is behavior, not model features.
- **C5 Opt-in & facilitated:** multi-agent is surfaced through dialogue, never automatic.
- **C6 Commit-safe by construction:** claims + branch policy + reviewer gate + verification gates. Structural guarantee, not hope.
- **The core is byte-identical v1.34.1** on Pi and Codex (diff-confirmed). Any change lands ONCE in forge-method-core.
- **forge-standalone-app** (`/mnt/c/Users/Danie/OneDrive/Documentos/forge-standalone-app`, a Rust/Cargo project) consumes core contracts via `contracts/forge-method-core`. **Parity between Python core and Rust app must be maintained.**

---

## Anti-patterns to NOT commit (the maintainer flagged these)

- ❌ Jumping to build without locking the spec (violates Progressive Autonomy — our own rule).
- ❌ Committing v2.0 changes to the core without empirical Phase B validation.
- ❌ Working in the wrong workspace (we're in mutant-run-horde-lab, not forge-method-core).
- ❌ Skipping the grill-gate / reality-evidence-gate before PRD.
- ❌ Letting agents write spec/context (GDD/PRD/AGENTS.md) without a human-approval gate (ETH Zurich: reduces success).

---

## Decisions made together in this session (context the next agent needs)

- We agreed on A→B→C phasing (design → validate → implement). Do not skip B.
- We discovered (research) that **shared-workspace + write-time consistency beats git-worktree-isolation** (STORM paper, arXiv:2605.20563, +34.6 pts in coupled code). This refuted an earlier "worktree per area" instinct.
- We discovered the **vendor convergence** (Feb–May 2026): every major coding agent shipped parallel agents with shared task list + peer messaging + file locking + plan-approval. Forge's vocabulary for this already exists; only execution is missing.
- We confirmed the maintainer's working habit (POC everything → converge → lock spec → autonomous build) maps exactly to **Progressive Autonomy** (AX session modes Interactive→Plan→Autopilot + Ralph Loop). This is the design's spine.
- We confirmed the maintainer's #9 complaint (agent doesn't offer research, doesn't grill-close blocks) is a REAL gap — **reframed by reality-evidence-gate (2026-06-22)**: a dedicated `evidence-research.md` pack and ~10 routing packs already exist, so "zero facilitation packs mention research affordance" was **overstated**. The verified gap is narrower: no pack proactively carries the "say I don't know → research" affordance on every interaction surface (targeted grep = 0 matches). Grill-gate exists and IS semi-default in phases 1/2/3; the gap is it doesn't fire at *every* decision-close point. (See `20260622-reality-evidence-gate-forge-v2.md`.)

---

## First command for the next agent

```
forge resume
# then read, in this repo's .forge-method/artifacts/:
#   handoff-forge-v2-evolution.md   (this file — the START-HERE summary)
#   forge-flock-coordination-rfc-v3.md   (THE RFC)
#   forge-runtime-audit.md                (the codebase audit)
# confirm you have the context, then:
forge grill-gate   # close the 7 open questions one at a time
```

> Note: these files are currently **untracked in git** (not committed). ~~The spec is not locked yet (grill-gate + reality-evidence-gate must run first, per our own Progressive Autonomy rule).~~ **STATUS 2026-06-22: SPEC IS NOW LOCKED** — reality-evidence-gate (PLAUSIBLE→STRONG conditional) + grill-gate (7/7 resolved, maintainer sign-off) both passed; see `20260622-spec-lock-forge-v2.md`. The citation corrections are applied. The spec is ready to commit + branch when the maintainer chooses; next step is Phase B (empirical gap-report on a flexible validation target), NOT Phase C.
