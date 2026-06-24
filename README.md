<p align="center">
  <strong>FORGE METHOD</strong><br>
  <em>Elevate what you can build.</em>
</p>

<p align="center">
  <a href="https://github.com/DanielCarva1/forge-method-core/releases/tag/v2.0.3"><img src="https://img.shields.io/badge/version-2.0.3-ff6b35" alt="v2.0.3"></a>
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT License">
  <img src="https://img.shields.io/badge/tests-141%2F141%20pass-brightgreen" alt="141/141 tests pass">
  <img src="https://img.shields.io/badge/multi--agent-Codex%20%7C%20Claude%20%7C%20OpenCode%20%7C%20Pi-orange" alt="Multi-runtime">
</p>

---

Forge Method is an open-source framework that makes you a better builder — whether you're shipping your first side project or running a fleet of agents across a team. It guides you through the entire creative process: from fuzzy idea to validated, tested, shipped work. It sharpens your thinking, catches weak assumptions, and refuses to let you ship slop.

And in v2.0.0, it does this for **teams of humans and agents working together** — not just solo.

> *Forge Method exists to break gatekeepers. Everyone deserves access to software best practices, structured thinking, and AI-augmented building — not just people who can afford enterprise tooling or elite consultants. It's open source. It's free. It stays free.*

---

## What Forge does for you

**It makes you think better before you build faster.**

Most AI coding tools are good at motion — generating code, moving fast, producing volume. Forge is built for **direction**. It slows you down when the idea is vague. It grills your assumptions before they harden into expensive plans. It shows you alternatives you didn't consider. Then, when the direction is locked, it gets out of your way and lets the agent build at full speed — with verification gates, not ceremony.

The result: you ship work that's **thought through, tested, and durable** — not AI-generated slop that falls apart on first contact with reality.

### The phases

Forge guides every project through the same proven arc:

```
discovery → specification → plan → build → verify → ready → evolve
```

Each phase has guided workflows, facilitation packs, quality gates, and durable artifacts. You never lose the thread when a chat resets — the state is in files, not memory.

### Anti-slop by construction

- **Grill Gate** — before any big decision, Forge asks one question at a time, checks your answers against evidence, and refuses to let you skip with loose ends.
- **Reality/Evidence Gate** — is this idea physically possible? Technically feasible? Is there real user pain? Forge checks before you waste time building the impossible.
- **Verification Gates** — tests, lint, types, evals. Code doesn't ship until it passes. No exceptions.
- **Human-curated specs** — agents don't write your PRD or architecture behind your back. You decide. (Backed by ETH Zurich research: LLM-written specs reduce success.)

---

## What's new in v2.0.0 — Build with your team (humans + agents)

Forge Method was already great for solo work. **Now it also coordinates teams of humans, each running multiple AI agents, all on the same repo — without chaos.**

This is flock coordination: the `.forge-method/` directory becomes the coordination layer that `.git/` is for code. Every runtime — Codex, Claude Code, OpenCode, Pi — reads and writes the same protocol files. No runtime controls another. No lock-in.

### The three breakthroughs

| What was broken | What v2 does | Why it matters |
|---|---|---|
| **Two agents writing state = silent data loss** | `version` field + optimistic concurrency. Conflict DETECTED, not lost. | You can trust your project state when 5 agents are working simultaneously. |
| **Worker handoffs clobbered the driver's state** | Fleet-mode append-only handoffs. Workers emit requests; driver applies. | Agents coordinate through files, not through hoping they don't step on each other. |
| **No way to know who did what** | `agents/registry.yaml` + `--agent-id` on every command + full ledger attribution. | Full audit trail. You always know which agent touched what. |

### Lane claims — your agents won't trip over each other

Agents **claim lanes** (product areas) before writing. Two agents never edit the same lane. Claims auto-expire (30min TTL). `forge-commit` stages only files in your claimed lanes — no more `git add -A` cross-contamination.

This is trunk-based development, extended to the human+agent world.

### Proven in chaos

We stress-tested v2 with a **proof of concept**: 3 agents building a comedy horse e-commerce ("Stable Investments"), committing directly to `main`, zero coordination, zero branches, zero communication.

**25/25 stories built. Typecheck clean. Zero state clobbers. Zero lane collisions.** The 5 gaps we found are fixed in this release.

---

## The science (not vibes — peer-reviewed)

Every v2 design decision is backed by research or production case studies:

| Paper | What it proved | How Forge uses it |
|---|---|---|
| **grite** (arXiv:2606.19616) | Event log + CRDT: 78%→0% duplicate work, 3× throughput across 32 agents | Lane claims + completion-state + append-only handoffs |
| **STORM** (arXiv:2605.20563) | Shared-workspace + write-time consistency beats worktree isolation | Optimistic concurrency instead of worktree-per-agent |
| **CoAgent** (arXiv:2606.15376) | "Notify, don't lock" — LLMs judge whether conflicts matter | VersionConflict returns diff + retry guidance |
| **ETH Zurich** (arXiv:2602.11988) | LLM-written context files reduce agent success ~3% | AGENTS.md is a human-approved draft, never auto-applied |
| **CooperBench** (arXiv:2601.13295) | Coordination — not coding — is the bottleneck in multi-agent work | Forge IS the coordination layer |

---

## Install

### Quick start

```bash
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core

# Windows
.\install.ps1

# macOS / Linux
./install.sh
```

This installs Forge Method to `~/.agents/skills/`. Works with **any runtime**.

### By runtime

| Runtime | Install | Status |
|---|---|---|
| **Codex** | `codex plugin marketplace add DanielCarva1/forge-method-core --ref main` | ✅ Production |
| **Claude Code** | `./install.sh` then `forge emit-agents-md --runtime claude` | ✅ Ready |
| **OpenCode** | `./install.sh` | ✅ Ready |
| **Pi.dev** | `./install.sh` | ✅ Ready |

### Upgrade from v1.x

```txt
$forge-update
```

That's it. Existing projects auto-migrate (transparent `version: "0"` added on next resume). No opt-in needed — v1 behavior is preserved when no fleet registry exists.

---

## Start building

Open your project folder and run:

```txt
$forge-method
```

Forge detects your workspace, asks what you're building, and guides you through the full arc — from idea to shipped.

### Want multi-agent? Add a registry.

Create `.forge-method/agents/registry.yaml`:

```yaml
driver: alice-agent
flocks:
  alice:
    agents:
      - {agent_id: alice-agent, role: driver, areas: [backend]}
      - {agent_id: bob-agent, role: worker, areas: [frontend], parent: alice-agent}
lanes:
  - {id: backend, claimant: null}
  - {id: frontend, claimant: null}
```

Now your agents claim lanes, hand off safely, and coordinate through the protocol. Every runtime reads the same files.

---

## Open source. Anti-gatekeeper. Forever.

Forge Method is MIT licensed. It exists because building software with AI should be accessible to everyone — not locked behind enterprise pricing or proprietary platforms. The coordination protocol is open. The facilitation packs are open. The best practices encoded in the workflows are open.

Fork it. Extend it. Build your enterprise on it. Teach with it. The only thing you can't do is close it.

---

## Validation

| Check | Result |
|---|---|
| Existing test suite | **141/141 pass** (zero regressions) |
| v2 unit tests | **14/14 pass** |
| smoke-runtime | **Pass** |
| smoke-install | **Pass** |
| Gate (audit + evals + integration) | **24/24 evals + Integration: pass** |
| Backward compatible (C2) | **Verified** — v1 projects work unchanged |
| POC stress test | **3 agents, 25 stories, zero clobbers** |

---

## Credits

Designed by Daniel Carvalhal. Validated through deep research, stress-tested in live multi-agent chaos, and built with AI agents that use Forge Method to build Forge Method.
