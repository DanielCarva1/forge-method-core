# R-SCM — Supply Chain Hardening (SBOM + sigstore)

**Status**: ✅ R-SCM.1, R-SCM.2, R-SCM.3 COMPLETE (pending first release with artifacts)
**Data**: 2026-07-01
**Meta**: Segurança supply chain 8 → 10 (after first tagged release with new workflow)

## Threat model

Um usuário instala `forge-core` (via GitHub Release asset ou `cargo install`).
R-SCM endereça três perguntas que SHA256 sozinho não responde:

1. **Identidade**: o binário veio mesmo do Stable Studio CI, ou de um fork
   malicioso que copiou a assinatura? → **sigstore keyless signing** responde.
2. **Provenance das dependências**: quais libs transitivas (ed25519, p256,
   sigstore-tsa, rcgen) estão no binário? → **SBOM CycloneDX** responde.
3. **Build attestation**: o binário foi produzido pelo nosso workflow no
   commit X, em runner não-comprometido? → **SLSA provenance** responde (futuro).

## Design (deletion test aplicado a cada camada)

- **SHA256 only** (já temos): deletar = perde integridade pós-download. Mantém.
- **SBOM CycloneDX**: deletar = sem auditoria CVE de deps transitivas, crítico
  para projeto crypto-heavy. Mantém.
- **sigstore keyless signing**: deletar = "trust on first use" não prova
  identidade do publisher. Mantém.
- **SLSA provenance via slsa-framework**: deletar = pode-se assinar e dar SBOM
  mas não atestar o build. Adiável para R-SCM.4.

## Plano incremental

### R-SCM.1 — SBOM CycloneDX por target

- Adicionar `cargo-cyclonedx` ao workflow (instalado on-demand no job de release)
- Para cada target, rodar `cargo cyclonedx -p forge-core-cli --format json --output-pattern package`
- Upload `forge-core-<target>.bom.json` como release asset
- **Complexidade**: baixa (uma tool existente, sem crypto)
- **Valor**: habilita auditoria CVE automatizada via Dependant/OSV

### R-SCM.2 — sigstore keyless signing (cosign)

- Instalar `cosign` no runner (já vem no `ubuntu-latest` via GitHub Actions OIDC)
- Para cada binary asset, `cosign sign-blob --yes <binary>` (keyless via OIDC)
- Upload `<binary>.sig` + `<binary>.bundle` (sigstore bundle para verificação offline)
- Requer `permissions: id-token: write` no job
- **Complexidade**: média (precisa de `cosign verify-blob` documentado para o user)
- **Valor**: prova identidade do publisher, user verifica com Rekor transparency log

### R-SCM.3 — Documentação de verificação + papers

- README: seção "Supply chain verification" com comandos `cosign verify-blob`,
  `cyclonedx validate`, link para certificado OIDC do Stable Studio
- `contracts/research/`: cite SLSA v1.0 (NIST), sigstore (ACM CCS 2023),
  CycloneDX 1.5 spec
- Papers chineses relevantes: incluir se aplicável (F-sci separado cobre isso)
- **Complexidade**: baixa (documentação)
- **Valor**: user sabe o que verificar e como

### R-SCM.4 — SLSA provenance via slsa-framework (adiado)

- `slsa-framework/slsa-github-generator/.github/workflows/generator_container_slsa3.yml@v2.0.0`
- Gera `forge-core-<target>.intoto.jsonl` (SLSA L3 attestation)
- **Complexidade**: alta (job adicional, integração com artifact upload)
- **Valor**: SLSA L3, mas só necessário se queremos listar em SLSA registry

## Critério de 10/10

- [ ] R-SCM.1 implementado: SBOM gerado e publicado como asset
- [ ] R-SCM.2 implementado: cada binary assinado via sigstore keyless
- [ ] R-SCM.3 documentado: README tem seção de verificação funcional
- [ ] Release v0.2.0 (ou patch v0.1.1) produz: binary + sha256 + sbom + sigstore
- [ ] `cosign verify-blob` documentado e testado manualmente
