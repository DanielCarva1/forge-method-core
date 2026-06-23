# Guideline Audit — Forge v2.0 Protocol Design Edits

- kind: guideline-audit
- created_at: 2026-06-22
- workflow: guideline-audit (6-evolve)
- auditor: Codex agent (after runtime correction — agent had escaped the runtime and hand-edited durable protocol architecture without a governing guideline)
- layer: agent substrate + product governance (the `.forge-method/` protocol is the agent substrate; v2 design governs runtime behavior)

## Gap

Work trying to become work: integrate the v2 design upgrades surfaced this session — (A) CRDT-vs-single-driver hybrid, (B) completion-state, (C) notify-don't-lock — plus the A2A/MCP positioning and the distributed-state design — into the load-bearing RFC v3, then commit the strengthened spec. This edits durable protocol architecture.

## Risk if an agent implements without a guideline

This already happened (the process failure this audit corrects): an agent (me) hand-edited the RFC, the addendum, and gate artifacts directly, bypassing the runtime — no transition, no artifact registration, no governing rule on what's allowed to change. The risk: unbounded edits to load-bearing design without acceptance evidence, no rollback boundary, and a future agent cannot tell what was governance-approved vs improvised. The facilitation anti-pattern "Do not start durable architecture without a guideline or explicit waiver" was violated.

## Governing guideline

**Status: MISSING — declared here, to be promoted to a durable guideline doc.**

Proposed guideline **`GL-v2-protocol-design`** (governs all v2 protocol design edits until Phase C implementation):

1. **Boundary:** v2 work is **docs/spec only** until Phase B empirical gap-report lands. No runtime code (`.py`) changes. No new `.forge-method/` protocol files created in the runtime. The RFC + addenda + gate artifacts are the only editable surface.
2. **Evidence-before-edit:** every RFC edit that changes architecture must cite a gate artifact (reality-evidence / grill / research) as its decision source. No unsupported architectural claim lands.
3. **Compactness preserved:** the RFC stays a compact state-machine-style doc (per AGENTS.md). Deep detail lives in addenda, referenced from the RFC — not inlined into bloat.
4. **Runtime-tracked:** all artifacts register via `artifact add`; state changes via `transition`; the runtime index is the source of truth, not chat.
5. **Backward-compat sacred (C2):** no v2 edit may imply a breaking change to existing single-agent users. Every additive feature must be opt-in.
6. **No cross-layer bleed:** positioning (A2A/MCP) is product-governance; distributed design is agent-substrate; they edit different RFC sections and don't entangle.

## Acceptance evidence (human can judge without reading code)

- The RFC v3 + addendum, read cold by a fresh agent, lets it execute Phase B without guessing boundaries. (The quality bar of guideline-audit.)
- Every architectural change cites a gate artifact path as decision source.
- `audit` + `gate` runtime commands pass on the project after edits.
- The runtime artifact index lists all v2 artifacts; nothing load-bearing lives only in chat.
- Backward-compat: a diff of the RFC shows no existing-behavior removal — only additions + corrections.

## Work-order candidate (bounded)

- **allowed files:** `.forge-method/artifacts/forge-flock-coordination-rfc-v3.md`, `.forge-method/artifacts/20260622-v2-positioning-and-distributed-design.md`, the 4 gate/research artifacts (corrections only), and a new `GL-v2-protocol-design.md` guideline doc.
- **forbidden files:** any `skills/forge-method/scripts/*.py` (Phase C, not started), any existing `.forge-method/state.yaml`/`sprint.yaml` semantics change, any `docs/adr/*` without a separate ADR workflow.
- **checks:** `python scripts/test-runner.py` stays green (no code changed, but confirms no accidental breakage); `audit` passes; `gate` passes.
- **rollback:** the v2 artifacts are currently untracked in git; commit is the point of no return — rollback = `git revert` the spec commit. Pre-commit, rollback = delete the artifact edits.
- **human acceptance question:** "Does the strengthened RFC + addendum let a fresh agent execute Phase B without guessing, and does every architectural change cite a gate artifact?"

## Implementation status

**Permanent implementation ALLOWED (docs/spec only), bounded by this guideline.** Not blocked, not disposable-spike — this is the governed spec-strengthening that precedes Phase B. The guideline above is the governing rule; violating it re-triggers this audit.

## Validation result

Pending: run `audit` + `gate` after the RFC integration edits land. Register this audit via `artifact add`.

## Next action

1. Promote `GL-v2-protocol-design` to a guideline doc (or ADR) so it outlives this audit.
2. Execute the work-order: integrate upgrades A/B/C + positioning + distributed design into RFC v3, each edit citing its gate source.
3. Run `audit` + `gate`.
4. Commit the strengthened spec (the point this whole session was driving toward).
5. Phase B (empirical gap-report) — next, governed by the same guideline.
