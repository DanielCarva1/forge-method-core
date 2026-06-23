# Deep Research — Multi-Agent Software Dev at Enterprise Scale (for Forge v2.0)

- kind: research-synthesis
- created_at: 2026-06-22
- scope: how large orgs run many devs × many agents on one product trunk-based, without breaking
- method: research subagent, multi-source (papers + vendor docs + engineering blogs)
- status: complete, cited
- full_source: dispatched research subagent ses_10d6e3036ffe3EG3jHFy7D2j8D
- note: full synthesis preserved below verbatim for durability. See "Top 10 Transferable Insights" for the Forge-relevant distillation.

---

## HEADLINE FINDINGS (the discussable ones)

1. **Architecture VALIDATED, not gambled.** grite (arXiv:2606.19616, Sarkar/ASU, June 2026) independently built Forge's EXACT architecture — append-only event log in git refs + CRDT projection + advisory leases — and measured it: N=32 agents, 78%→0% duplicate work, 3× throughput, byte-identical convergence proven. Shopify Aquifer/River runs the durable-event-log-as-substrate pattern at scale (59,918 sessions, 3,536 PRs/30 days, 7,000+ people). Forge isn't speculating; it's formalizing the industry's convergent answer.

2. **SELF-CORRECTION on STORM.** The reality-evidence-gate (this morning) "corrected" +34.6 → +18.7/+1.4. The research confirms **+34.6 IS REAL** — it's STORM's high-coupling-repo subset (GitWorktree collapses to 36.3% there; STORM stays at 70.9%). +18.7/+1.4 are the headline AVERAGES across all repos. Both numbers are correct in context. I was too aggressive; the RFC's original +34.6 was defensible for the coupled-code claim. Owe Daniel an honest walk-back.

3. **"Locks alone are insufficient" — empirically.** grite's arms: locks-only had the HIGHEST redundant rediscovery rate; only locks + shared completion state drove failures to zero. **New requirement for Forge:** can't just do claims/lanes; must track shared task-COMPLETION state so an agent sees "this is already done," not just "this is claimed."

4. **CRDT projection > single-driver-only for the distributed case.** grite proves byte-identical convergence WITHOUT a single driver — multiple writers converge via CRDT semantics. My proposed Principle 18 (single-driver-across-machines) is still right for the INTEGRATION FSM, but CRDT is a STRONGER answer for the coordination state (registry, claims, task completion). Design upgrade, not contradiction.

5. **CooperBench (arXiv:2601.13295): coordination, not coding, is the bottleneck.** Two-agent success = 25% vs 50% solo. Agents fail at communication, commitments, expectations — "the curse of coordination." Reframes Forge's value: verifiable commitments, not better agents.

6. **CoAgent (arXiv:2606.15376): "notify, don't lock or abort."** The LLM can judge whether a conflict actually invalidates its plan (classical transactions can't). New primitive Forge could adopt.

7. **Practical ceiling = 2-3 concurrent code-writing agents.** Human review is the bottleneck, not compute. Broad parallelism loses to serial-with-targeted-parallelism (Factory, Helge Sverre). Forge should default to serial + parallelism only where the task graph proves independence.

8. **Failure modes are real and partly invisible:** silent corruption (one bad value poisons downstream, no exception thrown), agentic drift (semantic conflicts git can't catch), forensics nightmare (denied claims leave no trace in PR history).

---

## TOP 10 TRANSFERABLE INSIGHTS (Forge application)

1. Append-only event log in git + CRDT projection + advisory leases = 78%→0% dup, 3× throughput, byte-identical convergence. [grite] → This IS Forge's architecture, validated. WAL = single source of truth; projections derived, never hand-edited.
2. Locks alone insufficient — need locks AND shared completion state. [grite] → Forge must track task-completion in the shared log, not just lease ownership.
3. Write-time consistency (optimistic concurrency) in shared workspace beats worktree isolation on coupled code (+34.6). [STORM] → Mediate file writes; reject stale writes, return diffs, retry.
4. Notify, don't lock or abort. LLM judges if conflict matters. Saga compensation. [CoAgent] → Push change-notifications; register inverses for compensation.
5. Coordination is the bottleneck, not coding (25% vs 50% solo). [CooperBench] → Forge provides verifiable commitments (shared signatures, insertion-point contracts).
6. Progressive autonomy with bidirectional pacing (earn up, fall back on quality). [venutian-antfarm, Bounded Autonomy] → Forge POC→lock→autopilot maps to Crawl→Walk→Run.
7. CODEOWNERS + branch protection = machine-enforceable auth, zero custom tooling. Tiered (auto / AI+human / human-only). [Aido Labs, AgentPatterns] → Forge emits CODEOWNERS for agent-owned paths; enforcement at merge gate.
8. Integration tax is nonlinear (2 agents ≈ 1.5×, 8 agents ≈ 5×). Serial-with-targeted-parallelism wins. [Factory, When Parallelism Pays Off] → Forge defaults to sequential; parallelism only where task graph proves independence.
9. Every coordination action must be a typed, signed, mineable event. [grite, ESAA] → Event schema: actor_id, ts, event_kind, conflict_flag, lock_outcome. Log = audit + forensics substrate.
10. Durable event log as substrate; agents as profiles on top. "Cells die, machines die. The conversation doesn't." [Shopify] → Forge event log is substrate; Codex/Claude/local are profiles. New agent products = new bundles, not new platforms.

---

## KEY VENDOR PATTERNS (who's doing it, how)

- **Cursor:** worktree-per-agent, up to 8 parallel. Self-driving experiment found workers "couldn't communicate or provide feedback on the project as a whole."
- **Claude Code Agent Teams:** file-based under ~/.claude/ (task JSON + .lock via flock + inbox files). "Handful" of peers; dynamic workflows to "dozens-hundreds." No merge strategy, no durable state — the gap Forge fills.
- **Devin:** managed Devins in isolated VMs, coordinator resolves conflicts, up to 10 parallel. Nubank: "fleet on every repo."
- **Stripe Minions:** 1,000+ PRs/week, devbox-isolated (not worktree), 400+ MCP tools. Full env isolation = unattended autonomy.
- **Shopify Aquifer/River:** durable event log substrate, 60k sessions/30 days. The largest production validation.
- **Meta:** 50+ agent swarm for read-only codebase mapping; single-agent-per-model for writes with hibernate/wake for multiweek autonomy.
- **Factory:** serial-with-targeted-parallelism beats broad parallelism; milestones with validation gates.

## CRITICAL PAPERS (verified)
- **grite** (arXiv:2606.19616) — THE Forge-relevant paper. Append-only log + CRDT + leases, measured N=32.
- **STORM** (arXiv:2605.20563) — write-time consistency > worktree; +34.6 on coupled code (real), +18.7/+1.4 averages.
- **CoAgent** (arXiv:2606.15376) — notify-don't-lock; LLM-as-conflict-judge.
- **CooperBench** (arXiv:2601.13295) — curse of coordination; 25% vs 50% solo success.
- **SyncMind** (arXiv:2502.06994) — out-of-sync as fundamental failure mode.
- **grite, ESAA, kli, Rewind** — all converge on append-only log + projection + leases/CRDT.

## CONFIDENCE / GAPS
High confidence on grite/STORM/CoAgent/CooperBench (peer-reviewed, code released) and vendor patterns (official docs/blogs). Unverified: the "87% downstream poisoning in 4h" (attributed to unnamed Galileo AI sim, no citation). Poolside/Codeium fleet enterprise scale — no reports found. No source publishes a verified count of 100+ concurrent agents on one repo; practitioner ceiling cited is 2-5 for code-writing.
