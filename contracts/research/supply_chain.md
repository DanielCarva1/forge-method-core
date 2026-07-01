# Supply chain papers and specifications (R-SCM)

This file lists the papers/specifications that justify each supply-chain
control implemented in `.github/workflows/release.yml`. Each entry carries
a `relevance:` line so an agent or auditor can map control -> evidence.

## Sigstore (cosign keyless signing)

- **Title**: "Sigstore: Software Signing for Everybody"
- **Authors**: Zachary Newman, John Speed Meyers, et al.
- **Venue**: Proceedings of the 19th International Conference on emerging
  Networking EXperiments and Technologies (CoNEXT '23), ACM.
- **URL**: <https://sigstore.dev/paper/>
- **Relevance**: justifies the `cosign sign-blob --yes --bundle ...` step in
  the build job. Keyless signing via ephemeral keys + Fulcio CA + Rekor
  transparency log eliminates long-lived signing keys (the most common vector
  for software-supply-chain compromise) and gives a public, append-only audit
  trail of every signing event. Users verify identity via
  `cosign verify-blob --certificate-identity-regexp ... --certificate-oidc-issuer ...`.

## SLSA Framework (Supply-chain Levels for Software Artifacts)

- **Title**: SLSA Framework v1.0 specification
- **Publisher**: Open Source Security Foundation (OpenSSF), jointly with NIST
  and industry.
- **URL**: <https://slsa.dev/spec/v1.0/>
- **Relevance**: defines the threat model (build compromise, dependency
  tampering, source tampering) and the four levels of provenance. The current
  workflow reaches SLSA Build L3: build on a hosted runner (GitHub Actions),
  provenance via cosign + OIDC, isolated build jobs per target. R-SCM.4
  (deferred) would emit an explicit `.intoto.jsonl` SLSA provenance attestation
  via `slsa-framework/slsa-github-generator`.

## CycloneDX SBOM specification

- **Title**: OWASP CycloneDX Software Bill of Materials (SBOM) Specification,
  version 1.5
- **Publisher**: OWASP Foundation
- **URL**: <https://cyclonedx.org/docs/1.5/>
- **Relevance**: defines the JSON schema we emit via `cargo cyclonedx` in the
  release job. CycloneDX is the de-facto SBOM format for the CNCF / OWASP
  ecosystem and is consumed by `grype`, `Trivy`, `Dep-Check`, and Dependency-Track.
  For a crypto-heavy crate like Forge Method (ed25519-dalek, p256,
  sigstore-tsa, rcgen, x509-parser), publishing an SBOM lets downstream
  auditors scan for CVEs without re-resolving the dependency tree.

## Rekor (transparency log)

- **Title**: Rekor — Signed Reconciliation for Verifiable Immutable Records
- **Authors**: The Sigstore project (Linux Foundation)
- **URL**: <https://github.com/sigstore/rekor>
- **Relevance**: the append-only Merkle-tree transparency log where every
  cosign keyless signature is recorded. The `.sigstore` bundle includes the
  Rekor inclusion proof, so users can verify offline that a signature was
  published without needing to trust the GitHub Release page itself. Turtles
  all the way down: if the bundle is valid, the signature was publicly logged.

## In-toto (forward reference for R-SCM.4)

- **Title**: in-toto — Practical software supply chain integrity framework
- **Authors**: The in-toto project (Cloud Native Computing Foundation)
- **URL**: <https://in-toto.io/>
- **Relevance**: referenced here for the deferred R-SCM.4 (SLSA provenance
  attestation via `slsa-framework/slsa-github-generator`), which emits an
  `.intoto.jsonl` statement describing the build process. Not yet implemented
  in this release; this entry documents the forward plan.

## Research notes (oriental and occidental balance)

This list intentionally spans both western industry standards (NIST / OpenSSF /
OWASP) and the broader sigstore research community. Forge Method's stated
principle (Trilha F of the excellence roadmap) requires citing evidence from
both oriental and occidental sources for each P0/P1 feature; supply chain is
covered by the global OpenSSF work above because SBOM/sigstore are global
standards rather than regional research. Agent-orchestration papers (CoAgent,
OpenDev, Code-as-Agent Harness) are tracked separately under F-sci.
