# Grill Gate — Forge Method v2.0 Open Questions (RFC §10)

- kind: grill-with-docs
- created_at: 2026-06-22
- scope: RFC v3 §10 — 7 remaining open questions + contradictions surfaced
- status: recommended-answers-ready-for-maintainer-lock
- inputs_read:
  - `.forge-method/artifacts/forge-flock-coordination-rfc-v3.md` (citation-corrected)
  - `.forge-method/artifacts/forge-runtime-audit.md`
  - `.forge-method/artifacts/20260622-reality-evidence-gate-forge-v2.md`
  - `skills/forge-method/references/workflow-grill-gate.md`
  - `skills/forge-method/scripts/forge_method_runtime.py` (env vars L40/911; AGENTS.md L127)
- mechanic: Matt Pocock grill-with-docs S0→S6 — one question at a time, recommended answer, check artifacts/code before asking, glossary inline

## Method

Each of the 7 open questions is restated, tested against the 8 hard constraints (C1–C8), the 17 design principles, the codebase audit, and the corrected evidence. A recommended answer is given with rationale. All 7 resolve from artifacts — none require a blocking human contradiction-rescue. Two carry a genuine judgment element (flagged), where the maintainer's preference is the deciding vote. One additional citation error in the RFC was surfaced by the grill (appendix).

---

## Q1. Cross-runtime spawning — should council suggest "a Pi agent could do X" + hand-off draft, or stay silent?

**Recommended answer: ACCEPT the proposal — suggest + one-click hand-off draft, gated.**

Finding: C3 (runtime-agnostic, no runtime commands another) is preserved because suggesting ≠ spawning — the human spawns manually. C5 (opt-in/facilitated) is honored because the suggestion appears in council dialogue, not auto-executed. C8 + Principle 15 (research always available, partner-grade) actually *require* the suggestion: an agent that silently swallows "a Pi agent could do this better" is not partner-grade.

Risk: unprompted suggestions become nagging. Gate the suggestion on TWO conditions: (a) the task is demonstrably outside the current agent's lane or runtime capability, AND (b) another runtime is registered in the flock (agents/registry.yaml non-empty). Suggest, attach a pre-filled hand-off draft, human approves/ignores.

Decision: **suggest + hand-off draft, double-gated.** Staying silent violates C8.

---

## Q2. Claim TTL & heartbeat default — proposed 30 min TTL + heartbeat-on-write. Confirm?

**Recommended answer: ACCEPT 30 min as DEFAULT, but make it configurable + add explicit heartbeat command.**

Finding: heartbeat-on-write keeps the claim fresh during active work. But 30 min TTL fails for long non-writing work (40-min test suite, deep research/thinking without ledger writes) — the claim auto-releases mid-work and a second agent grabs the lane. C7 (quality packaging) favors configurability over magic numbers.

Decision: `claims.ttl_minutes` in project config (default 30). Add `forge heartbeat --lane <id>` for explicit renewal during long non-writing ops. **[JUDGMENT CALL — maintainer confirm: configurable default 30, or hardcoded 30?]**

**MAINTAINER DECISION (2026-06-22): 30 min accepted. Critical semantic refinement — TTL expiry = HANDOFF EMISSION, not silent release.** When a claim expires (agent crashed, forgot, or went to do something else), the system writes a handoff artifact capturing the in-progress state. When any agent (the original or a new one) next looks for a lane, it finds the handoff and continues from there — no lost work. The strategy's purpose is explicitly to **prevent abandoned lanes accumulating infinitely without reason**; the handoff-on-expiry is what makes auto-release safe. 30 min default kept; configurability retained as low-cost option. The handoff-on-TTL-expiry behavior is a load-bearing semantic, not an implementation detail.

---

## Q3. Flock discovery — env FORGE_FLEET=on + FORGE_AGENT_ID + FORGE_FLOCK=<human-id> set by spawner. Confirm?

**Recommended answer: ACCEPT, with fallback chain + dual opt-in.**

Finding: codebase confirms precedent — runtime already reads `FORGE_METHOD_CORE_DEV` (L40/911) and the updater reads `FORGE_METHOD_*` vars. So `FORGE_*` is the established namespace. Glossary-consistent (flock = human + their agents).

Risk: env vars don't propagate if the spawner forgets to set them. Resolution: fallback chain `--agent-id` flag > `FORGE_AGENT_ID` env > `"default"`. Multi-agent mode triggers on EITHER `FORGE_FLEET=on` env OR presence of `agents/registry.yaml` (dual opt-in — env for spawner-driven, file for human-declared).

Decision: **ACCEPT env-based discovery + flag fallback + dual opt-in trigger.**

---

## Q4. Naming — agents/registry.yaml vs alternatives?

**Recommended answer: ACCEPT agents/registry.yaml.**

Finding: codebase confirms `agents/` dir already exists (openai.yaml + profiles/). Co-locating the roster with per-agent state files (`agents/<agent_id>.yaml`, §6.4) is the least-surprising layout. The protocol already uses subdirs (claims/, handoffs/, evidence/). Alternatives (fleet.yaml, flat agents.yaml) fragment the fleet metadata from per-agent state.

Decision: **agents/registry.yaml** for the roster; `agents/<agent_id>.yaml` for per-agent snapshots. No viable alternative beats co-location.

---

## Q5. AGENTS.md emission — emit for Claude Code + OpenCode in Phase C step 13, or hold?

**Recommended answer: ACCEPT emit — but DOUBLE-gated (opt-in flag + draft review), never auto-applied.**

Finding: codebase confirms AGENTS.md is recognized (`PRODUCT_FACING_DOC_EXACT_PATHS` L127) but NO emission logic exists — greenfield. The ETH Zurich evidence (verified real, arXiv:2602.11988) is decisive: LLM-written context files REDUCE success (~3%, +>20% inference cost). So emission MUST be human-approved. C2 (backward-compatible) adds: existing users must not get surprise AGENTS.md changes.

Decision: `forge emit agents-md --runtime {claude-code|opencode}` (opt-in flag) produces a DRAFT the human reviews and applies manually. Two gates: opt-in flag + human approval before canonicalization. Honors ETH Zurich + C2. Holding emission entirely would block the Claude Code/OpenCode expansion (Principle 17 parity).

---

## Q6. /chronicle scope — lightweight (ledger + checkpoints) or rich (+ artifact diffs, unresolved inputs)?

**Recommended answer: START LIGHTWEIGHT (v1); gate the rich form behind Phase B evidence.**

Finding: no chronicle command exists — greenfield. Principle 11 (canvases for inspectable work) + AX ambient-progress justify the feature. But rich (artifact diffs, unresolved-input resolution state) is high-surface, high-maintenance, and unvalidated. The RFC's own A→B→C discipline says: validate the thin version before building the rich one.

Decision: **v1 chronicle = ledger (time-windowed) + checkpoints + story-status transitions.** Rich form is a follow-up ONLY if Phase B gap-report shows the lightweight version leaves the human polling. Avoids over-building before empirical signal. **[JUDGMENT CALL — maintainer confirm: lightweight-first, or commit to rich upfront?]**

**MAINTAINER DECISION (2026-06-22): RICH FROM v1.** Override of the lightweight-first recommendation. v1 chronicle = ledger (time-windowed) + checkpoints + story-status transitions + artifact diffs + unresolved inputs. Trade-off accepted: higher Phase C effort (shifts from MED/MED toward MED/HIGH) and more failure modes (diff computation, unresolved-input resolution state) in exchange for a more useful day-one surface. The maintainer prefers the fuller artifact; Phase B may still refine which rich fields earn their cost.

---

## Q7. Lock override — can human force-lock/unlock a spec outside a gate?

**Recommended answer: ACCEPT forge lock --force / forge unlock --force — with MANDATORY durable rationale.**

Finding: Principle 8 (human-curated specs) + Principle 12 (spec-lock = Facilitated→Autopilot handoff) require that the human is never trapped by the gate process. C7 (decision-source traceability) requires the override leave an audit trail. The proposal satisfies both.

Decision: `forge lock --force --reason "<text>"` and `forge unlock --force --reason "<text>"` — the `--reason` is REQUIRED (not optional), recorded in ledger as `spec.force-locked {artifact, agent_id, ts, reason}`. Human authority preserved; audit trail never broken. Blocking the override would violate Principle 8.

---

## Glossary additions (inline, per S0→S6 mechanic)

- **flock** (refined): a human + their agents (across runtimes); the unit of coordination. Scales from one flock to an org. Tied via `FORGE_FLOCK=<human-id>` or registry entry.
- **lane** (refined): a write boundary. Two granularities: Product Area lane (`claims/<area>.lock`) and Story lane (`claims/<story-id>.lock`). One claimant at a time. TTL-bounded.
- **claim** (refined): the right to write a lane, held by an agent_id. Reassignable (driver claim) or exclusive (story lane). Auto-expires on TTL; refreshed by heartbeat-on-write or explicit `forge heartbeat`.
- **lock signal** (refined): a spec/PRD/GDD that passed its gate; recorded as `spec.locked` in ledger. The canonical Facilitated→Autopilot handoff. Force-able by human with durable rationale.
- **research affordance** (corrected): the proactive "say I don't know → I research and recommend" affordance on every interaction surface. NOT the absence of research packs (those exist); the extension of packs with a proactive prompt.

---

## Appendix — additional contradiction surfaced by the grill (not in the 7)

**A1 (MINOR — citation error in RFC §6.4):** the RFC attributes the claim TTL/heartbeat to "Maintainer #9," but #9 in §10 is the research-affordance + grill-default complaint. The lane/TTL intuition is maintainer #10 ("lanes pattern = claims + handoff"). The reality-evidence-gate handoff (L54) already records TTL=30min as a separate confirmed decision. **Fix: §6.4 line "Claims have TTL + heartbeat (Maintainer #9)" → "(Maintainer #10, confirmed separately)."** Owner: agent, 1 line.

**A2 (MINOR — Phase B dependency):** Q2 (TTL configurability) and Q6 (chronicle richness) both ultimately answer to Phase B empirical evidence. The grill gives defaults now so spec can lock, but the maintainer should treat both as "revisable after mutant-run-horde-lab gap-report." This is consistent with the RFC's own A→B→C discipline; flagging so it isn't forgotten.

---

## Implementation contract (what "spec locked" means after this grill)

- All 7 open questions have a recommended answer grounded in constraints/evidence/code.
- ~~2 flagged as judgment calls (Q2 configurability, Q6 chronicle richness)~~ **RESOLVED by maintainer 2026-06-22:** Q2 = 30 min + handoff-on-TTL-expiry semantic (load-bearing); Q6 = rich from v1.
- 1 additional citation fix (A1) applied.
- **Maintainer sign-off given 2026-06-22. RFC v3 spec is LOCKABLE** (per Progressive Autonomy: spec-lock is the handoff from Facilitated to Autopilot).
- THEN Phase B (empirical gap-report) runs — it may refine Q2/Q6 details, which is the designed behavior.
- Phase C implementation does NOT start until Phase B produces the gap-report (the RFC's own anti-pattern #2).
- **Phase B target is flexible** — `mutant-run-horde-lab` (named in the handoff) is one example validation project, NOT a binding requirement. The v2 evolution is independent of that game project. Phase B may run on any suitable multi-agent stress-test target the maintainer chooses.

## Handoff

- preserve: 7 resolved answers (Q2/Q6 maintainer-decided), 1 citation fix (A1, applied), glossary refinements, Phase B target flexibility, spec-lockable status.
- do_not: start Phase C before Phase B gap-report; treat Q2/Q6 as final beyond empirical refinement; auto-apply AGENTS.md (Q5); treat mutant-run-horde-lab as a binding dependency.
- next_workflow: spec.locked recorded → Phase B (team-operating-model + product-area-map + trunk-based-plan on a chosen validation target → stress-test concurrent agents → gap-report.md).
