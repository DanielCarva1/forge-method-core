# Spec Lock — Forge Method v2.0 (Flock Coordination RFC v3)

- kind: spec-lock
- created_at: 2026-06-22
- locked_by: maintainer (Daniel Carvalhal) + Codex agent
- locked_artifact: `.forge-method/artifacts/forge-flock-coordination-rfc-v3.md` (citation-corrected 2026-06-22)
- lock_basis:
  - `.forge-method/artifacts/20260622-reality-evidence-gate-forge-v2.md` (stance: PLAUSIBLE→STRONG conditional; 3 citation fixes applied)
  - `.forge-method/artifacts/20260622-grill-gate-forge-v2-open-questions.md` (7/7 open questions resolved; maintainer sign-off 2026-06-22)
- ledger_intent: `spec.locked {artifact: "forge-flock-coordination-rfc-v3.md", agent_id: "codex-main-daniel", ts: "2026-06-22", reason: "reality-evidence-gate passed (conditional); grill-gate closed 7/7 open questions with maintainer sign-off"}`

## What is locked

The Forge Method v2.0 design (RFC v3) is spec-locked: the vision (flock coordination protocol), the 8 hard constraints (C1–C8), the 17 design principles, the 7-layer architecture, Progressive Autonomy (cyclic), and all 10 §10 open questions (7 remaining resolved this session + 3 earlier) are decided.

## Decisions captured this session (load-bearing)

- **Q1** Cross-runtime spawning: suggest + hand-off draft, double-gated (task-outside-lane AND other-runtime-registered).
- **Q2** Claim TTL: 30 min default (configurable). **TTL expiry = HANDOFF EMISSION, not silent release** — load-bearing semantic. Purpose: prevent abandoned lanes accumulating infinitely.
- **Q3** Flock discovery: `FORGE_FLEET`/`FORGE_AGENT_ID`/`FORGE_FLOCK` env + `--agent-id` flag fallback + dual opt-in (env OR registry file presence).
- **Q4** Naming: `agents/registry.yaml` + `agents/<agent_id>.yaml`.
- **Q5** AGENTS.md emission: emit, double-gated (opt-in flag + human-approved draft). ETH Zurich honored.
- **Q6** /chronicle: **RICH from v1** (maintainer override of lightweight-first) — ledger + checkpoints + story-status + artifact diffs + unresolved inputs.
- **Q7** Lock override: `forge lock --force --reason` / `forge unlock --force --reason` (rationale mandatory).

## Citation integrity (applied before lock)

- STORM (arXiv:2605.20563): +18.7/+1.4 (not +34.6).
- ETH Zurich (arXiv:2602.11988): verified real, direction confirmed.
- H1/#9 reframed: research packs exist; gap is the proactive affordance.
- Grill-gate reframed: not default at every decision-close point.
- A1: RFC §6.4 TTL attribution #9 → #10.

## Conditions on the lock (must hold before Phase 1 re-entry)

1. **Phase B (POC of all 20 principles) is non-negotiable.** A prototypical implementation of the v2 design — both the code-substrate principles (#1-#7, #13, #17-#20) and the behavior/facilitation principles (#8, #9, #11, #14, #15, #16) — MUST be built and stress-tested before any broader v2 implementation lands. RFC anti-pattern #2 + reality-evidence-gate agree. **Reframed 2026-06-23 (maintainer correction, see `20260623-spec-correction-phase-b-poc-framing.md`):** the original "use v1 as-is to find gaps" framing only re-confirms the audit; the real POC validates all 20 v2 principles themselves. Scope is the full design (1-20), not just additions 18/19/20 — the maintainer's decision after POC is whether to commit to the whole design and re-enter Phase 1 for it.
2. Phase B may refine Q2 (TTL behavior details) and Q6 (which rich fields earn cost). This is the designed A→B→(Phase 1 re-entry) discipline, not a re-open of the spec.
3. **After POC-accept verdict, the runtime re-routes evolve → Phase 1 (discovery): full interview → PRD → architecture → build.** The original "Phase C direct implementation" framing bypassed RFC §6.5 + Principle 12 (Evolve Loop = restart feature in Facilitated mode — full interview, facilitation, POC). v2 is a large enough layer to warrant the full Forge Method flow. (Closes defect `evolve-reentry-routing-gap`, logged at commit `391d99b`.)
4. The 1.34.1 release-readiness hotfix (the active runtime state) is a SEPARATE thread; this v2 spec-lock does not disrupt it.

## Next workflow

Phase B — POC of all 20 v2 principles (target flexible):
- Build minimal POC across two layers: (a) code substrate for state/coordination principles (agent_id, version field, append-only handoffs, claim primitive, registry, CRDT projection); (b) behavior/facilitation for partner-experience principles (research affordance, grill-as-default, match-energy).
- `team-operating-model` → declare driver + worker flocks.
- `product-area-map` → lane boundaries.
- `trunk-based-plan` → branch policy + CODEOWNERS merge authority.
- Stress-test concurrent agents THROUGH the POC; observe which principles hold, which break.
- Test partner experience (research affordance, grill-close, match-energy) — independently testable.
- Output: `gap-report.md` with POC verdict (accept / iterate / reject) → on accept, re-enter Phase 1.
