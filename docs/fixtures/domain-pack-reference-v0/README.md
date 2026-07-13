# Agent-built game reference Domain Pack (P6d)

This fixture is a real, generic-data Domain Pack for the unknown-unknowns failure mode: a human asks an agent to build a game but neither knows all the disciplines and delivery gates. The pack contributes discovery, vertical-slice, representative first-use playtest, and packaging/readiness governance without any game-specific Rust branch in Forge core.

All documents are `candidate_only` declarations. Static fixture data is not a signature, review result, installed package, runtime capability, execution receipt, or activation authority. P6c review and supply-chain authority must be produced dynamically by the trusted lifecycle path.

The composition request is rebased on the exact admitted `contracts/workflow-governance/golden-path-v0.yaml` genesis. It preserves the published raw document, canonical document, inner bundle, and policy-set digest distinctions; the Domain Pack core binding uses the inner bundle digest required by the generic composer.

Discovery keeps research and technical unknowns with the agent. Player/platform constraints, production feasibility, and material-risk review are separate claims with plural evidence and distinct-principal thresholds. External-authority production evidence uses the authoritative-acceptance floor. Only after **all** discovery claims are verified does the `all_claims_verified` product-direction rule ask the human to choose among feasible platform/scope directions, conservatively recommending one platform and one complete vertical slice.

First-use readiness requires both a representative uncoached session and a distinct independent-review lane. Packaging similarly separates deterministic package identity, representative installed-runtime behavior, and independent license/limitations/rollback review. A producer self-report, build artifact, or single automated check cannot satisfy these gates. The candidate therefore declares Forge Core `>=0.9,<1.0`.

## Layout

- `manifests/` and `content/`: the digest-bound reference candidate.
- `requirements/` and `requests/`: a persistent project need and a deterministic composition request.
- `projections/`: the complete deterministic expected composition, compared by exact typed equality.
- `artifacts/`: representative, adversarial, and ablation cases. Their existence never proves that a representative execution ran.
- `hostile/`: a digest-correct but semantically invalid candidate that attempts to own `forge.core.*`; generic Domain Pack validation rejects it.

The focused Rust proof recomputes every raw and canonical digest, compares the complete composition projection exactly, proves the `0.9` compatibility lower bound and explicit missing-domain/capability gaps after removal, rejects the hostile candidate, demonstrates that partial/self-reported discovery, first-use, and packaging evidence cannot complete their gates, and scans production Rust sources to prevent a reference-domain special case in core.
