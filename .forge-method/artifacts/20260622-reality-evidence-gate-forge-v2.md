# Reality/Evidence Gate — Forge Method v2.0 (Flock Coordination)

- kind: reality-evidence-decision
- created_at: 2026-06-22
- scope: Forge Method v2.0 evolution (RFC v3 + audit + handoff)
- status: plausible-to-strong (conditional)
- target_claim: "Forge becomes the open, runtime-agnostic coordination protocol for human+agent flocks — trunk-based-development equivalent for the human+agent world."
- reviewer: Codex agent (reality-evidence-gate workflow)
- inputs_read:
  - `.forge-method/artifacts/forge-flock-coordination-rfc-v3.md`
  - `.forge-method/artifacts/forge-runtime-audit.md`
  - `.forge-method/artifacts/handoff-forge-v2-evolution.md`
  - `skills/forge-method/references/workflow-reality-evidence-gate.md`
  - `skills/forge-method/scripts/forge_method_runtime.py` (L857-864)
  - `skills/forge-method/facilitation/*.md` (research-affordance grep)

## Claim under test

The v2.0 bet rests on five load-bearing claims:
1. Multi-agent on a shared repo is the 2026 reality (vendor convergence).
2. Shared-workspace + write-time consistency beats worktree isolation (STORM).
3. Forge's current runtime has a CRITICAL concurrency blocker (G1: state.yaml overwrite).
4. LLM-written context/spec reduces agent success (ETH Zurich → human-curated specs).
5. The maintainer's #9 complaint (no research affordance, grill not default) is a real, verified gap.

## Evidence verified

| # | Claim | Source | Verdict | Note |
|---|---|---|---|---|
| 1 | STORM exists + thesis sound | arXiv:2605.20563 (Liu et al., 19 May 2026) | ✅ REAL | State management > worktree isolation; write-time conflict detection. Direction confirmed. |
| 1a | STORM "+34.6 pts in coupled code" | RFC v3 §3 | ✅ VERIFIED REAL (subset) | Deep-research (2026-06-22) confirmed +34.6 is the *high-coupling-code subset* delta (GitWorktree collapses to 36.3% there; STORM holds 70.9%). +18.7/+1.4 are the *headline averages* across all repos. **Both figures are correct in context.** The gate's earlier "inflated" verdict was itself too aggressive — walked back. |
| 2 | ETH Zurich: LLM-written AGENTS.md reduces success | arXiv:2602.11988 (Gloaguen et al., 12 Feb 2026) | ✅ REAL (direction) | Abstract: context files "tend to reduce task success rates" + ">20% inference cost." "~3%" figure needs paper body confirmation; direction is sound. |
| 3 | G1 blocker: state.yaml full overwrite, no concurrency control | `forge_method_runtime.py:857-864` | ✅ CONFIRMED | `write_flat_yaml` does `path.write_text(...)`; no flock/lock/version. Two concurrent writers = silent data loss. BLOCKER is real and demonstrable. |
| 4 | G2/G3 blockers (handoff mutates state; no agent registry) | audit §G2/G3 | ✅ PLAUSIBLE | G2 mechanism confirmed by audit line cites; not re-verified line-by-line this pass. Trust audit pending Phase B. |

## Evidence refuted / needs reframing

| # | Claim | Source | Verdict | Note |
|---|---|---|---|---|
| 5 | #9: "zero facilitation packs mention research affordance" | handoff L137; RFC H1 | ❌ OVERSTATED | A dedicated `facilitation/evidence-research.md` pack EXISTS, +10 packs route to research. The precise gap (verified by targeted grep for the proactive "say I don't know → research" affordance phrase = 0 matches) is narrower: **no pack proactively tells the human they may ask for research on every interaction surface.** Gap is real; the evidence as written is imprecise and must be corrected before it becomes a spec input. |
| 6 | Grill-gate "not wired as default" | RFC §6.5, S7 | ⚠️ PARTIALLY TRUE | `workflow-grill-gate.md` triggers in phases 1/2/3 and before unlocking mechanical work — so it IS semi-default early. The real gap: it does NOT fire before every handoff/phase-transition/decision-lock as the RFC wants. Reframe from "not default" → "not default across all decision-close points." |

## Evidence not falsifiable cheaply (directional only)

- Vendor convergence Feb–May 2026 (Cursor/Claude Code/Devin/Windsurf/Codex parallel agents): plausible, widely observed, but not per-vendor verified here. Feeds market-scarcity argument; does not prove Forge's specific open-substrate bet.
- Addy Osmani "Code Agent Orchestra," AX (GitHub Build 2026), Anthropic 2026 trends: talks/blogs; directional. Cannot hard-verify citations without source fetch per item.
- Maintainer-habit → Progressive Autonomy mapping: interpretive, not falsifiable.

## Viability stance

**PLAUSIBLE → leaning STRONG, conditional.**

- **Physical/technical possibility:** ✅ clear. Pure-file protocol; the blocker (G1) is a well-understood problem with a known solution (optimistic concurrency, STORM-validated).
- **Legal/ethical/safety:** ✅ clean. Dev tool; ETH Zurich concern structurally handled (human-curated spec gate).
- **User pain:** ✅ REAL and demonstrable. G1/G2/G3 mean concurrent agents corrupt state TODAY — a live bug, not hypothetical. Multi-agent is the 2026 baseline (vendor convergence).
- **Alternatives / market:** ⚠️ the differentiation (open, runtime-agnostic substrate) is a genuine gap BECAUSE every vendor locks multi-agent to their ecosystem. The open question is market SIZE: how many teams run mixed-runtime fleets vs standardizing on one vendor? At minimum the bet serves the maintainer (Pi + Codex + future Forge App) — so it is not vapor.
- **Core reason:** the bet is technically sound, the pain is demonstrable, and the differentiation is real. It is NOT yet STRONG because (a) the evidentiary foundation has two integrity errors (STORM number, #9 grep) that must be corrected, and (b) the market-size claim for the open-substrate positioning is unverified beyond the maintainer himself.

## Minimum evidence required before specification or build

1. **~~Fix the STORM citation~~ RESOLVED (walked back):** deep-research confirmed BOTH STORM figures are valid — +34.6 (high-coupling subset), +18.7/+1.4 (headline averages). The gate's original "inflated" verdict was too aggressive; corrected in RFC v3 §3/§11 on 2026-06-22. (See `20260622-deep-research-multi-agent-at-scale.md`.)
2. **Reframe H1 / #9** in RFC v3 §6.6 and handoff: research packs exist; the missing piece is the proactive affordance on every surface. Precision matters — this becomes an anti-pattern and a spec input.
3. **Reframe grill-gate gap** (§6.5, §S7): "not default at every decision-close point," not "not default."
4. **Phase B empirical validation** (already prescribed by handoff): run concurrent agents on mutant-run-horde-lab using only existing Forge features → produce `gap-report.md` proving where flock coordination hurts. This is what converts PLUASIBLE → STRONG. Do NOT skip.
5. **Optional market scan** (strengthens, not blocks): one piece of evidence that mixed-runtime fleets exist beyond the maintainer (e.g., public teams running Codex + Claude Code together). If absent, the bet remains maintainer-serving (still valid, smaller TAM).

## Decisive evidence (what makes this viable at all)

- STORM validates the core primitive (state management over isolation) — the architectural spine is evidence-backed.
- G1 is a live, reproducible bug — the pain is not invented.
- Forge already has the append-only backbone (S1: ledger/index.ndjson) and the collaboration vocabulary (S5) — most of the substrate exists; v2 is additive, not a rewrite.

## Unresolved risks

- **R-EVID-1 (MED):** STORM misquotation, if uncorrected, undermines every other citation's trust. Cheap to fix; must fix.
- **R-EVID-2 (MED):** #9 overstatement, if propagated into the spec as "build research affordance from zero," misjudges the work (it's an extension of existing packs, not greenfield).
- **R-EVID-3 (HIGH):** Skipping Phase B and jumping to Phase C implementation violates the RFC's own Progressive Autonomy principle (human-led validation before autonomous build). The handoff flags this as an anti-pattern; the gate agrees.
- **R-EVID-4 (MED):** Open-substrate TAM is unverified. If only the maintainer ever runs a mixed fleet, the protocol still works but the "org-scale" vision is aspirational.

## Next scan / workflow

- **Immediate (blocking spec):** correct citations (#1, #2, #3 above) — owner: maintainer + agent, ~30 min.
- **Then:** run `grill-gate` on RFC §10's 7 open questions (the questions survive the gate; the evidence errors do not change the open-question list, only the framing of H1-related items).
- **Then:** Phase B empirical gap-report on mutant-run-horde-lab (the real evidence gate before any Phase C code lands).
- **Optional parallel:** market-scan for mixed-runtime fleet adoption (R-EVID-4).

## Handoff

- preserve: stance (plausible→strong conditional), 3 evidence-integrity fixes required before spec, Phase B is non-negotiable, STORM/ETH/G1 verified-real, #9 and grill-default need reframing.
- do_not: proceed to PRD/spec without fixing citations #1-#3; skip Phase B; treat #9 as greenfield.
- next_workflow: grill-gate (RFC §10 open questions) — AFTER the 3 citation fixes land.
