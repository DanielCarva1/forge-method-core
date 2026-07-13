# Product promise compliance audit

Status: audited against source checkpoint `0.10.0` after P6d and the first P7 productization slice.
Decision: the protocol checkpoint is strong; the complete public product journey is not yet proven.

## Executive finding

P6 proves important internals: typed policy evaluation, opaque authority,
content-addressed Domain Pack composition, reviewed learning, immutable lifecycle
generations, crash recovery, representative-evidence rules, and replacement-agent
ledger continuity.

It does **not** yet prove that an ordinary user and host agent can perform the
complete journey through the shipped public surface. P7a.1 removes private key
and registry construction from the public credential lifecycle, but exact
request/action-packet generation, one-call authorization, pack acquisition, and
representative real-host proof remain open. The local signer is also only a
cooperative same-principal proxy; it does not prove human presence or reviewer
independence without a host/operator broker.

## Promise matrix

| Promise | Current evidence | Verdict |
|---|---|---|
| One idempotent bootstrap entry | `start` E2E covers empty, existing, damaged, and resumed projects | Proven for bootstrap |
| Human works only in chat | Cooperative credential lifecycle/signing exists, but request/action-packet generation, high-level authorize, and a human-presence broker remain separate | Not yet operational |
| Unknown unknowns become visible | Eight universal lenses exist in the read-only Obligation Engine | Proven in isolation, not integrated into the durable workflow loop |
| Domain Packs cannot silently rewrite core | Generic composition, trust, lifecycle, degradation, and deletion tests | Proven inside the cooperative same-user threat model |
| Reference pack helps an ignorant user discover the needed method | The game pack is a source fixture; no intent-to-pack discovery or coordinate install exists | Not yet operational |
| Replacement agent resumes durable truth | Workflow ledger/effective epoch resume is strong | Partial: unified intent and Assurance Case are not in that authority |
| Core can update after pack adoption | Release upgrade blocks whenever an active generation exists and no rebase command is public | Missing |
| Prebuilt installation contains current P6/P7a.1 | Source is `0.10.0`; latest published prebuilt is older | Missing until a correct release is published |
| Release identity is supply-chain coherent | The hardened source workflow now checks out the exact tag, requires matching workspace/binary version and prior CI, builds a manifested adoption archive, signs it, and requires an SBOM | Implemented in source; no `0.10.0` release published yet |
| Forge governs all product writes | Claims/gates protect Forge-mediated mutations | Limited: direct editor/shell writes remain outside Forge and must not be described as governed |
| Idea-to-operated-product method | Runtime phase advancement ends at BuildVerify | Not yet complete |

## Acceptance rule

Documentation, fixture-only harnesses, and protocol-level E2E are valuable but
cannot close a product claim. A public-journey claim closes only when a clean
release installation, using only public commands and the shipped host adapter,
can reproduce it without private Rust helpers or hand-authored authority files.

## Remediation program

The authoritative sequence and exit criteria are maintained in
[`contracts/plan/agent-native-guidance-plan.yaml`](../contracts/plan/agent-native-guidance-plan.yaml):

1. P7a workflow authority bridge.
2. P7b unified durable Assurance Case.
3. P7c domain-demand discovery and pack acquisition.
4. P7d coordinated core/pack rebase.
5. P7e release and distribution correctness.
6. P7f real-host and mediated-mutation proof.
7. P7g fast, evidence-preserving delivery tiers.
8. P7h promise hardening, documentation, and fork guidance.
9. P8 Ready/Operate/Evolve.
10. P9 Domain Pack SDK and ecosystem.

This document is a status audit, not runtime authority. Typed policy, ledger,
registry, lifecycle, and release contracts remain the executable sources of
truth.
