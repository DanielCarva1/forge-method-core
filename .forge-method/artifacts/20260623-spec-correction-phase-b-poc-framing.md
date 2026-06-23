# Spec Correction — Phase B POC framing + Phase C re-entry

- kind: spec-correction
- created_at: 2026-06-23T05:45:35Z
- corrected_by: maintainer (Daniel Carvalhal) + Codex agent
- correction_class: logical-flaw-in-phased-plan
- affects: RFC v3 §8, handoff-forge-v2-evolution.md, 20260622-spec-lock-forge-v2.md, handoffs/20260623-051321-handoff.md, state.yaml
- ledger_intent: `spec.corrected {artifact: "forge-flock-coordination-rfc-v3.md", section: "§8 Phase B + Phase C", correction_artifact: "20260623-spec-correction-phase-b-poc-framing.md", reason: "Phase B reframed from 'use v1 as-is, find gaps' to 'POC validating ALL 20 v2 design principles (1-20, not just additions 18/19/20)'. Phase C reframed to candidate backlog for Phase 1 re-entry per §6.5/Principle 12"}`

## The flaw the maintainer caught

The original phased plan (RFC v3 §8 + downstream handoffs/locks) framed Phase B as:

> "Use Forge AS-IS (v1.34.1, no core changes), run 2-3 concurrent agents on a real target, measure where it hurts, output gap-report → feeds Phase C implementation."

This framing has a load-bearing logical flaw: **you cannot validate v2 additions without implementing them.** Testing v1.34.1 under multi-agent stress only re-confirms what `forge-runtime-audit.md` (G1/G2/G3) already documented theoretically. It does not test whether the new principles 18 (CRDT-hybrid single-driver), 19 (completion-state), 20 (notify-don't-lock) actually fix the problems they are designed to fix. It validates the **problem** (which the audit already established) but not the **solution** (which is the actual risk before commitment).

The maintainer's exact framing (2026-06-23):

> "não tem como testar todas as implementações novas pra verificar gaps sem implementar, então o agente vacilou tbm"

(You can't test all the new implementations to verify gaps without implementing them — so the agent slipped too.)

## The internal contradiction that the original §8 had with RFC §6.5 / Principle 12

The RFC's own design rules said:

- **§6.5 The Evolve Loop:** "When the product ships and a feature is added, the evolve phase restarts the feature in Facilitated mode — full interview, facilitation, POC."
- **Principle 12:** "Progressive Autonomy is cyclic. Evolve loops back to human-led for each new feature. The spec-lock is the handoff between modes."

But §8 Phase C said: "implement step-by-step directly in forge-method-core" — bypassing the re-interview / PRD / architecture that the Evolve Loop mandates for big new layers. v2 (flock coordination + concurrency-safe state + partner experience + 13-step backlog) is exactly the kind of large layer that warrants the full Forge Method flow.

So §8 internally contradicted §6.5/Principle 12. The original Phase B + Phase C sequence was inconsistent with the RFC's own autonomy design.

## Scope of the POC: ALL 20 principles, not just additions 18/19/20

**Second maintainer correction (2026-06-23):** the POC commits to the **entire v2 design (principles 1-20)**, not just the latest 3 additions (18/19/20). The maintainer's framing:

> "é pra testar POC do 1 ao 20, nao só 18, 19 e 20. esses tres só foram adicionados depois, mas tudo que ta la vai entrar na POC antes de decidirmos se vamos planejar e por no core"

(POC tests 1 through 20, not just 18/19/20. Those three were only added later; everything that's there goes into the POC before we decide whether to plan and put it into core.)

Reasoning:
- The decision after the POC is whether to commit to the **whole** v2 design and re-enter Phase 1 (interview/PRD/architecture) for it — not whether to ship just the latest 3 additions.
- Principles 18/19/20 **interact** with the earlier 17 in ways the papers did not study as a combined system. A POC scoped only to the new 3 would miss emergent gaps at the interaction boundaries.
- Several of the earlier principles are only partially present in v1.34.1 and need validation under multi-agent stress + the new substrate:
  - **#2 (append-only backbone):** ledger/index already append-safe — POC validates they hold under concurrent fleet load.
  - **#7 (runtime-agnostic by construction):** already true by design — POC validates the cross-runtime path actually works when agents from different runtimes coordinate.
  - **#10 (verification bottleneck):** gates exist — POC validates they survive concurrent agents hitting them.
  - **#16 (grill closes blocks by default):** semi-default in phases 1/2/3 — POC validates it fires at every decision-close point per the v2 design.

**POC has two complementary layers, together validating all 20:**
1. **Code substrate POC** — minimal prototypical implementations for the state/coordination principles (#1-#7, #13, #17-#20): `agent_id` attribution, `version` field + optimistic concurrency, append-only handoffs, claim primitive with TTL, registry, CRDT projection. This is the substrate needed to *exercise* those principles under real load.
2. **Behavior/facilitation POC** — validation of the partner-experience principles (#8, #9, #11, #14, #15, #16) via facilitation packs + prompts: research-always-on affordance, grill-as-default, match-energy, clarifying-question batching. These don't need code substrate; they need behavior testing.

## What changed (consistent across all live docs)

### Phase B reframe: read-only stress-test → POC of all 20 principles

- **Old:** Use v1.34.1 as-is, no core changes, run multi-agent, find where it hurts, output gap-report → Phase C.
- **New:** Build a minimal POC validating **all 20 v2 design principles (1-20)** — for net-new principles, prototypical implementations; for principles partially present in v1.34.1, validation they hold under multi-agent stress + the new substrate; for behavioral principles, facilitation-pack-level tests. Run concurrent agents THROUGH the POC. Find gaps in the v2 design itself — which principles hold, which break, what the 12+ research papers didn't predict. Output `gap-report.md` with a POC verdict (accept / iterate / reject).

### Post-Phase-B reframe: direct Phase C implementation → Phase 1 re-entry

- **Old:** Phase C = implement the RFC step-by-step directly in forge-method-core (13 additive steps).
- **New:** After a POC-accept verdict, Forge Method re-routes **evolve → Phase 1 (discovery)** for v2 as a new layer: full interview → PRD → architecture → build. The 13-step Phase C list becomes a **candidate backlog** that feeds the Phase 1 interview/PRD/architecture (will be re-prioritized, possibly restructured), not the direct next step.

This:
- Brings RFC §8 into alignment with RFC §6.5 + Principle 12.
- Closes defect `evolve-reentry-routing-gap` (logged at commit `391d99b`): the guidance engine must route new-demand-in-evolve to discovery, not to builder. The original Phase C framing implicitly assumed the builder route; the corrected framing uses the discovery route.
- Matches the maintainer's mental model: research → strategy → grill → **POC** → restart Forge Method Phase 1 (interview/PRD/architecture) → build.

## Files edited (5)

1. `.forge-method/artifacts/forge-flock-coordination-rfc-v3.md` — §8 Phase B reframed as POC; §8 Phase C reframed as candidate backlog for Phase 1 re-entry (13-step list preserved as backlog).
2. `.forge-method/artifacts/handoff-forge-v2-evolution.md` — Phase B + Phase C sections reframed (lines 82-105).
3. `.forge-method/artifacts/20260622-spec-lock-forge-v2.md` — Conditions + Next workflow reframed (lines 34-48).
4. `.forge-method/handoffs/20260623-051321-handoff.md` — Summary + Next Action reframed (lines 11, 15).
5. `.forge-method/state.yaml` — `next_action` field updated to reflect the corrected Phase B POC framing.

## Files NOT edited (point-in-time historical records — left intact)

These artifacts captured point-in-time thinking on 2026-06-22 and are referenced by the spec-lock as lock-basis. They are historical records of the gate process, not live direction. Editing them would falsify history. They are noted here as context; the live docs above carry the corrected direction forward.

- `.forge-method/artifacts/20260622-reality-evidence-gate-forge-v2.md` — line 64 has the original "use only existing Forge features" framing for Phase B. Note: line 77 R-EVID-3 (HIGH) already flagged that "skipping Phase B and jumping to Phase C violates the RFC's own Progressive Autonomy principle" — the gate was internally pointing at the same contradiction the maintainer caught.
- `.forge-method/artifacts/20260622-grill-gate-forge-v2-open-questions.md` — unaffected (Q1-Q7 are about design decisions, not about the phased plan).
- `.forge-method/artifacts/20260622-spec-lock-forge-v2.md` — historical lock-basis record; the live doc was edited above, but the lock-basis it documents (reality-evidence-gate + grill-gate) still holds.

## What does NOT change

- The v2 RFC v3 design itself (principles 1-20, 7-layer architecture, constraints C1-C8, positioning above A2A/MCP, grite/STORM/CoAgent evidence) is unaffected. Only the **phased plan** (§8) was wrong, not the design.
- The spec-lock on RFC v3 stands. This correction brings the phased plan into alignment with the design; it does not re-open any of the 20 principles or 8 constraints.
- All 7 open questions (Q1-Q7) resolved by grill-gate remain resolved.
- Phase B remains non-negotiable (now as a POC, not a stress-test). reality-evidence-gate R-EVID-3 (HIGH) is honored.

## Handoff

- preserve: Phase B = POC validating ALL 20 v2 principles (not v1 stress-test, not narrowed to 18/19/20). Phase C = candidate backlog for Phase 1 re-entry (not direct implementation). The 13-step list is preserved as backlog. Spec-lock stands.
- do_not: treat this as a spec re-open; edit the historical artifacts; skip Phase B; go directly to Phase C implementation.
- next_workflow: pick the Phase B validation target → build the minimal POC substrate → run team-operating-model/product-area-map/trunk-based-plan THROUGH the POC → gap-report.md with verdict.
