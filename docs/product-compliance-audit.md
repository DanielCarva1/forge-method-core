# Product promise compliance audit

Status: audited against source checkpoint `0.12.0` after P7b implementation;
cumulative release gates remain pending.
Decision: the public authority bridge and unified durable Assurance Case are
implemented; the complete real-host product journey is not yet proven.

## Executive finding

P6 proves important internals: typed policy evaluation, opaque authority,
content-addressed Domain Pack composition, reviewed learning, immutable lifecycle
generations, crash recovery, representative-evidence rules, and replacement-agent
ledger continuity.

It does **not** yet prove that an ordinary user and production host can perform
the complete journey through a tagged release. P7a.2 adds deterministic action
packets, closed-input request preparation, operator-managed external broker
trust, and one-call application of a host-signed origin event. The local signer
remains only a cooperative same-principal proxy. Forge verifies that a
configured broker signed the bound event; the host deployment, not Forge,
establishes physical presence, actor independence, and runtime integrity.
Pack acquisition and representative real-host proof remain open.

P7b brings accepted human intent and all eight universal lenses into the same
durable projection used by `workflow next` and `resume`. The agent drafts a
typed representative slice; independent review must admit its exact latest
definition, and separately originated runtime evidence must cover every
declared scenario. Research and partial execution cannot be promoted to
verified by caller confidence. This reuses the existing workflow evidence
ledger rather than creating a second authority store.

## Promise matrix

| Promise | Current evidence | Verdict |
|---|---|---|
| One idempotent bootstrap entry | `start` E2E covers empty, existing, damaged, and resumed projects | Proven for bootstrap |
| Human works only in chat | Action packets and one-call broker-origin apply remove human request/attestation editing, but no representative production-host journey is published | Protocol implemented; product proof pending P7f |
| Unknown unknowns become visible | Eight universal lenses are explicit in durable `next`/`resume` projection with closed epistemic states | Protocol proven; no system can guarantee every unknown was discovered |
| Domain Packs cannot silently rewrite core | Generic composition, trust, lifecycle, degradation, and deletion tests | Proven inside the cooperative same-user threat model |
| Reference pack helps an ignorant user discover the needed method | The game pack is a source fixture; no intent-to-pack discovery or coordinate install exists | Not yet operational |
| Replacement agent resumes durable truth | Workflow ledger/effective epoch resume reconstructs accepted intent, assurance epoch, eight lenses, governed evidence, and next action | Proven at the host-neutral protocol boundary |
| Core can update after pack adoption | Release upgrade blocks whenever an active generation exists and no rebase command is public | Missing |
| Prebuilt installation contains current P6/P7a/P7b | Source is `0.12.0`; latest published prebuilt is older | Missing until a correct release is published |
| Release identity is supply-chain coherent | The hardened source workflow checks out the exact tag, requires matching workspace/binary version and prior CI, builds a manifested adoption archive, signs it, and requires an SBOM | Implemented in source; no `0.12.0` release published yet |
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

1. Finish the P7b cumulative gate and publish the coherent `0.12.0` source checkpoint.
2. P7c domain-demand discovery and pack acquisition.
3. P7d coordinated core/pack rebase.
4. P7e release and distribution correctness.
5. P7f real-host and mediated-mutation proof.
6. P7g fast, evidence-preserving delivery tiers.
7. P7h promise hardening, documentation, and fork guidance.
8. P8 Ready/Operate/Evolve.
9. P9 Domain Pack SDK and ecosystem.

This document is a status audit, not runtime authority. Typed policy, ledger,
registry, lifecycle, and release contracts remain the executable sources of
truth.
