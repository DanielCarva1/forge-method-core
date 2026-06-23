# POC Acceptance — Phase B Complete

- kind: poc-acceptance
- created_at: 2026-06-23T07:45:00Z
- verdict: ACCEPT
- accepted_by: maintainer (Daniel Carvalhal)
- poc_target: Stable Investments (comedy horse e-commerce) — https://github.com/DanielCarva1/hot-take
- gap_report: .forge-method/artifacts/20260623-phase-b-poc-gap-report.md
- substrate_tested: forge_v2_poc.py (v1.34 FSM + all 20 v2 principles, simplified, 520 lines)

## Verdict rationale

15/17 v2 principles held under chaotic concurrent load (3 agents, 25 stories, commit-to-main, no communication). Core mechanisms all worked: version-aware state (0 clobbers), lane claims (0 collisions), driver-only writes (0 unauthorized), append-only handoffs (0 corruptions), research adaptation (0 human-blocked moments). Integrated typecheck exit 0.

5 new gaps identified — all refinements, not fundamental design flaws:
- GAP-1: git-level lane enforcement (HIGH)
- GAP-2: shared-types coordination (HIGH)
- GAP-3: config coordination (MEDIUM)
- GAP-4: shared-file auto-wiring (MEDIUM)
- GAP-5: integration gate (MEDIUM)

## What this triggers

Per RFC v3 §6.5 + Principle 12 (Evolve Loop), v2 now re-enters **Phase 1 (discovery)** as a new feature layer. The full Forge Method flow (discovery → spec → plan → build) runs for the v2 implementation, using RFC v3 + this gap-report as primary inputs.

This is NOT the old "Phase C direct implementation" framing (corrected 2026-06-23). The formal flow must run — interview → PRD → architecture → build.

## Phase C candidate backlog (informed by POC — 18 items)

Original 13 from RFC v3 §8 + 5 new from POC gap-report:

| # | Source | Item | Priority |
|---|---|---|---|
| 1 | RFC R2 | `agent_id` attribution + `--agent-id` flag | HIGH/LOW |
| 2 | RFC R1 | `version` field + optimistic concurrency + auto-migration | CRITICAL/LOW |
| 3 | RFC R3 | `handoff`/`checkpoint` append-only via `--update-state` | CRITICAL/MED |
| 4 | RFC R6 | multi-agent + autonomy anti-patterns in guidance safety | HIGH/LOW |
| 5 | RFC R4 | `agents/registry.yaml` + per-agent state | CRITICAL/MED |
| 6 | RFC R5 | `claims/` lanes + claim check + TTL/heartbeat | HIGH/MED |
| 7 | RFC H1 | research-always-available affordance | HIGH/LOW |
| 8 | RFC H5 | grill-as-default + partner-grade presence | HIGH/MED |
| 9 | RFC D4 | typed `agent-contract` + `contract-check` eval | HIGH/MED |
| 10 | RFC G5 | council (standup mode) + orchestration spawning | HIGH/HIGH |
| 11 | RFC §6.7 | Evolve Loop wiring | MED/MED |
| 12 | RFC D1 | JSON schema for workflows/templates | MED/MED |
| 13 | RFC §6.7 | AGENTS.md emitter for Claude Code/OpenCode | MED/LOW |
| 14 | **GAP-1** | **git-level lane enforcement** (`forge commit` wrapper / pre-commit hook checking claims) | **HIGH/MED** |
| 15 | **GAP-2** | **shared-types coordination** (contract lane / type-conflict detection in gate) | **HIGH/MED** |
| 16 | **GAP-3** | **config coordination** (`defaults.yaml` / config lane) | MED/LOW |
| 17 | **GAP-4** | **shared-file auto-wiring** (eliminate `index.ts` contention via `tryLoadLane` convention) | MED/LOW |
| 18 | **GAP-5** | **integration gate** (`tsc --noEmit` + smoke test in gate) | MED/LOW |

## Ledger intent

`poc.accepted {verdict: "accept", target: "stable-investments", gap_report: "20260623-phase-b-poc-gap-report.md", principles_held: "15/17", new_gaps: 5, next: "Phase 1 re-entry for v2 layer"}`

## Handoff

- next: Phase 1 discovery for v2 layer (discover-intent → PRD → architecture → build)
- do_not: skip discovery; jump to direct implementation; ignore the 5 new gaps
- inputs: RFC v3, gap-report, forge-runtime-audit.md, the hot-take POC repo (working reference)
