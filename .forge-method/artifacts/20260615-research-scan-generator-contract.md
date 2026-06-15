# Research Scan Generator Contract

- created_at: 2026-06-15T22:07:00+00:00
- project: forge-method-core
- phase: 6-evolve
- workflow: runtime-builder
- status: research-scan-generator-added

## Problem

Research guidance had the right concept but could still fall back to hand-written markdown. Market, domain, and technical scans needed a first-class runtime generator so a future agent can close evidence with a compact, validated contract instead of improvising from chat.

## Runtime Contract

`artifact research-scan` writes and validates durable research scans before downstream planning. It requires:

- common fields: `workflow`, `mode`, `research_question`, `decision_to_unlock`, `claim`, `sources`, `source_gaps`, `evidence_grade`, `findings`, `contradictions_or_falsifiers`, `uncertainty`, `stance`, `validation`, `next_workflow`
- market fields: `alternatives`, `adoption_friction`, `demand_signal`
- domain fields: `domain_constraints`, `risks_or_harms`, `expert_review_needed`
- technical fields: `feasibility_stance`, `riskiest_unknowns`, `proof_path`

The generator runs the same `research_scan_findings` validator used by `artifact research-check`, registers a durable `research-scan` artifact, and can emit an artifact existence eval.

## Human Guidance Contract

Evidence Research now routes the human through a decision-oriented scan:

1. frame the claim and decision to unlock
2. pick market, domain, or technical lens
3. gather evidence without drifting into implementation
4. challenge contradictions and uncertainty
5. shape `artifact research-scan` fields
6. run `artifact research-check`
7. hand off stance and next workflow

## Validation

- focused generator test for market, domain, and technical scans passed
- existing `artifact research-check` contract test passed
- packaged workflow/facilitation validation test passed
- `workflow validate` passed
- `workflow compactness` passed
- `parity replay` passed with 90/90 cases
- `smoke-runtime.ps1` passed with source checkout research-scan coverage
- `smoke-install.ps1` passed with installed skill research-scan coverage
- full unittest discovery passed with 95 tests
- `verify-fast.ps1` passed

## Next Gap

Continue post-parity Forge polish by adding game-check generator coverage for game brief and sprint planning closeouts.
