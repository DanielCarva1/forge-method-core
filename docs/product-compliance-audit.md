# Product promise compliance audit

Status: audited against source workspace `0.12.0`, including current P7E release
workflow, P7F structural evidence checker, P7G CI topology, and P7H documentation.
This source audit does not itself prove publication or full-P7 completion;
selected release assets and field evidence must be verified separately.

## Executive finding

Forge has substantial typed governance, persistence, Domain Pack, external
broker, accepted-intent, universal-assurance, and replacement-agent evidence.
Current source also hardens release identity/package smoke and CI tier timing.

It still does **not** prove that an ordinary user and production host completed
the full chat-only journey from a published `0.12.0` package. The external
broker protocol verifies configured signatures; the deployment must establish
physical identity and separation. The P7F checker validates bundle structure and
bytes only. Direct editor/shell writes remain outside Forge governance.

Use the canonical [four-identity table](../README.md#four-identitiesdo-not-collapse-them).
Source checkpoint, latest prebuilt, project workflow release pin, and Domain
Pack effective epoch are not interchangeable.

## Promise matrix

| Promise | Current evidence | Verdict |
|---|---|---|
| One idempotent bootstrap entry | `start` E2E covers empty, existing, damaged, and resumed projects | Proven for bootstrap |
| Human works only in chat | Action packets and external-broker apply remove human request/attestation editing | Protocol implemented; no published production-host journey |
| Unknown unknowns become visible | Eight durable universal lenses use closed epistemic states | Protocol proven; exhaustive discovery is impossible |
| Domain Packs cannot silently rewrite core | Generic composition, trust, lifecycle, degradation, deletion, and effective-epoch tests | Proven within the cooperative same-user threat model |
| Reference pack supports demand discovery and governed local acquisition | Search/explain retain exact reviewed/package bindings; `acquire apply` derives resolver/composer/trust/preflight internals and enters the lifecycle TCB only after explicit operator approval | Implemented in source; remote public catalog/download remains open and cumulative E2E is pending |
| Replacement agent resumes durable truth | Resume reconstructs release pin, intent/assurance epoch, effective epoch, lens states, evidence, and next action | Proven at host-neutral protocol boundary |
| Core can update after pack adoption | Exact-CAS rebase revalidates target Core composition/trust, commits a new immutable generation, appends one joined `0.4` workflow event, and recovers a lifecycle-first interruption from a persisted plan | Implemented in source; cumulative active/degraded/replacement-process evidence is pending |
| Selected prebuilt contains source `0.12.0` features | Source cannot establish current public availability; `v0.4.0` is only the historical predecessor to this candidate | Verify exact `v0.12.0` release assets, manifest, sidecars, and clean-install evidence |
| Release identity is coherent | Source workflow binds exact tag/commit/workspace/CLI/manifest, rechecks payload/checksum/signature, and schema-validates a release-level SBOM | Implemented in source; publication is established only by the matching verified tag run and assets |
| Packaged install works | Source workflow extracts native x86_64 Linux/Windows and Intel/Apple Silicon macOS archives and smokes binary/wrapper version plus `start`, `init`, `resume`, `release-status`, `next` | Implemented release gate; requires successful matching tag run for release evidence |
| CI is fast without deleting evidence | Source topology owns Tier 0/focused/platform/expensive commands, preserves native Linux/Windows/macOS evidence, and emits timing JSON | Topology implemented; budget compliance requires actual consecutive CI evidence |
| Real-host bundle is trustworthy | Checker enforces bounds, closed fields, path/digest integrity, fixed scenarios, argv logs, governed-write links, and review fields | Structural/content-integrity only; no host, semantics, independence, publication, or P7F certification |
| Forge governs all product writes | Claims/gates/Admission/WAL/receipts cover Forge-mediated writes | Limited: direct editor/shell/host writes are ungoverned and must be disclosed |
| Idea-to-operated-product method | Runtime phase advancement ends at BuildVerify | Not yet complete |

## Evidence boundaries

Documentation, fixtures, and protocol E2E cannot close a public product claim.
A real-host claim needs a clean published installation, public commands and
shipped adapter, genuine chat-only interaction, conflict before overlapping
mediated writes, replacement-session reconstruction, and credible independent
review. The [bundle checker](real-host-proof.md) only packages bounded evidence
for that review.

Release controls become release evidence only after a matching tag run succeeds
and assets are independently verified. CI budgets become performance evidence
only from the timing artifacts of corresponding runs. An SBOM proves component
inventory shape/version, not absence of vulnerabilities.

## Remaining program

The typed plan and stories remain authoritative for sequence/exit criteria:

1. Execute or verify P7E release correctness at the intended tag; do not infer
   publication from source or tag text alone.
2. Run and independently review P7F on a supported production host; structural
   checker success is insufficient.
3. Collect P7G consecutive-run timing and inventory/failure-injection evidence.
4. Keep P7H promises, payload, generated help, links, and fork obligations
   aligned as source changes.
5. Plan P8 Ready/Operate/Evolve and P9 Domain Pack SDK/ecosystem only from
   evidence-backed remaining gaps.

This status audit cannot grant runtime authority or mark P7 complete.
