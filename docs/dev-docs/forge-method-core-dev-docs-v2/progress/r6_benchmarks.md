# R6 — Benchmarks (criterion)

**Data**: 2026-06-30
**Status**: R6.1 ✅ completo (store hot paths); R6.2 pendente (crypto hot paths)

## Objetivo

Cobrir hot paths com `criterion` para estabelecer baseline de performance,
validar a promessa "rápido e performativo" e servir de âncora para futuras
regressões. Sem medir, "rápido" é claim não evidenciada; com medir, vira fato.

## Setup

- `criterion = "0.5.1"` adicionado a `[workspace.dependencies]` com
  `default-features = false` e features `["plotters", "cargo_bench_support"]`.
- Per-crate `[[bench]]` entries com `harness = false`:
  - `crates/forge-core-store/benches/claim_wal.rs`
  - `crates/forge-core-store/benches/reference_index.rs`

## Padrões técnicos adotados (pitfalls documentados)

1. **`OnceLock<Mutex<HashMap<(Kind, Size), StateRoot>>>`** — `criterion` chama o
   closure de `bench_with_input` múltiplas vezes para calibrar. Sem cache,
   cada chamada re-popula o WAL do zero (1000 appends × N iterações = freeze).
   Com cache, populate roda uma vez por `(kind, size)` e reaproveita.

2. **`Box::leak(state.into_boxed_path())`** — produz `&'static Path`, eliminando
   lifetime noise na hora de passar state root pra iterator de benches.

3. **`--sample-size` mínimo prático = 10** (criterion 0.5), `--warm-up-time` ≥ 1.
   Abaixo disso, criterion panica com `assertion failed: dur.as_nanos() > 0`.

4. **`FmtSpan::ENTER`** é obrigatório em `tracing_init.rs` — sem ele, spans são
   invisíveis para o subscriber (não aparecem em logs mesmo com level certo).

## Baselines medidos (dev profile, Windows 11 / WSL)

### `claim_wal/append`

| Tamanho | Latência típica |
|---|---|
| 1 | 32ms |
| 100 | 37ms |
| 1000 | 41ms |

### `claim_wal/replay`

| Tamanho | Latência |
|---|---|
| 1 | 157µs |
| 100 | 719µs |
| 1000 | 7.2ms (~138K elem/s) |

### `reference_index/build`

| Caso | Latência |
|---|---|
| workspace (árvore real deste repo) | ~1.5ms |
| minimal (workspace vazio) | ~205µs |

## Achado crítico de performance (não é bug)

**`sync_data()` (fsync) no Windows = 25–50ms típico, picos de 300ms.** O custo
de cada append (32ms) é quase inteiramente o `fsync`. Recovery scan de 1000
records adiciona só ~9ms acima do append.

Isso **não é bug** do forge. O WAL precisa de `fsync` para garantir durabilidade
pós-crash — sem isso, você perde claims em queda de energia.

### Otimizações reais possíveis (cada uma é system design change)

1. **Tiered durability**: flag `--no-sync` para benchmarks/tests/dev (opt-in).
2. **Batch appends**: agrupar N appends em um único `fsync` (muda semântica).
3. **Async fsync** em background thread (complica recovery; ameaça durabilidade).

### Recomendação

Documentar o WAL como **durability-bound** no README técnico. No Linux, espera-se
que `fsync` seja bem mais rápido (5–15ms típico em SSD).

A flag `--no-sync` é um ganho de ergonomia real (Trilha B / F15-ish) e deve ser
implementada antes do final de F15. Não otimizar o fsync em si — é custo de SO.

## R6.2 (✅ completo)

Crypto hot paths em `crates/forge-core-crypto/benches/rekor.rs`:
- `parse_signed_checkpoint` (parse puro, ~2-3µs)
- `parse_rekor_log_entry` (JSON+base64 duplo, ~6-7µs)
- `verify_rekor_full_path/aux_{0,10,100}` (caminho público completo
  `run_host_adapter_rekor_verification`, parametrizado por profundidade
  do inclusion proof: ~420µs / ~450µs / ~655µs)

Decisão de design (aplicada via `improve-codebase-architecture`):
- Os helpers internos `verify_rekor_checkpoint` e `verify_merkle_inclusion`
  permanecem `pub(crate)` porque nenhum caller externo os usa isoladamente.
- São medidos indiretamente via o entrypoint público
  `run_host_adapter_rekor_verification`, que reflete o uso real.
- Deletion test: expor `verify_*` como `pub` só pra benchmark seria shallow.

Achado de performance: o custo dominante (~400µs) é a verificação p256
ECDSA no signed checkpoint. O Merkle walk scales O(log n) com cada hash
auxiliar adicionando ~2µs. parse é quase grátis (~6µs).

## R6.3 (✅ completo)

Benchmarks `serde_yaml::from_str` vs `serde_yml::from_str` vs
`yaml_serde::from_str` no tipo de produção `OperationContractDocument`,
sobre o fixture `docs/fixtures/operation-contract-v0/facilitate-first-product-idea.yaml`
(3.025 bytes, 94 linhas, contratos com structs aninhadas, optionals,
`deny_unknown_fields`, enums e arrays — o payload que Forge parseia em
cada `validate` / `execute-operation` / `claim`).

Local: `crates/forge-core-validate/benches/yaml_deserialize.rs`.
`serde_yaml` e `serde_yml` são dev-deps apenas deste bench; não fazem parte
de código de produção.

### Resultados (Windows release build, sample_size=150)

| Crate                  | Tempo (mediana) | Throughput    |
|------------------------|-----------------|---------------|
| `serde_yaml` 0.9 (legacy) | 92.9 µs         | 23.3 MiB/s    |
| `serde_yml` 0.0.12 (fork) | 93.4 µs         | 23.2 MiB/s    |
| `yaml_serde` 0.10.4 (Forge usa) | 99.7 µs         | 21.7 MiB/s    |

`yaml_serde` fica ~7% mais lento que as duas alternativas neste fixture.
Intervalos se sobrepõem levemente no limite inferior do `yaml_serde` com o
limite superior dos outros dois, mas a mediana é consistentemente mais alta
across runs.

### Decisão

A migração R7 para `yaml_serde` **não é revertida**. Justificativa:

1. **Não é hot path.** Parse de contrato de ~3KB em ~100µs é uma operação
   por chamada CLI. Workloads reais do Forge validam 1-N contratos por
   comando, não milhões; o custo absoluto é dominado por I/O de arquivo e
   crypto (ver R6.1 e R6.2), não por parse YAML.
2. **Diferença dentro do ruído operacional.** 7% em ~100µs = ~7µs por
   contrato. Mesmo validando 1000 contratos, isso adiciona ~7ms — abaixo
   do threshold onde usuários percebem latência.
3. **Manutenção e segurança ganhas com R7 superam o custo.** `serde_yaml`
   está em modo de manutenção desde 2024; `serde_yml` introduziu regressões
   de segurança (RUSTSEC) no passado. `yaml_serde` é a aposta ativamente
   mantida que motivou R7. Reverter colocaria dependências sem mantenedor.
4. **Resultado documenta trade-off, não regressão escondida.** Se no futuro
   `yaml_serde` regredir mais (>30%), há baseline pra acionar reavaliação.

### Reavaliação futura

Reabrir R6.3 se:
- `yaml_serde` passar a custar >2x o baseline do `serde_yml`, ou
- Workflow de validação em lote (>100 contratos/comando) virar caso de uso
  principal, ou
- `serde_yml` publicar release estável sem advisories e reativar manutenção.

### Como rodar

```bash
cargo bench -p forge-core-validate --bench yaml_deserialize
```

## R6.4 (✅ completo)

CI workflow `.github/workflows/perf.yml`:
- cron diário 06:00 UTC + `workflow_dispatch` + PRs com label `perf`
- Roda `cargo bench -p forge-core-store` (claim_wal + reference_index) e
  `cargo bench -p forge-core-crypto --bench rekor` com `--save-baseline ci-perf`
- Output `.txt` + `target/criterion/` upados como artifact (30 dias)
- Comparação automática contra main via criterion-compare action é possível
  mas deferida — diff manual via artifact é suficiente pra detectar regressões
  óbvias (>20% em qualquer direção).

## Como rodar

```bash
cargo bench -p forge-core-store
# ou focado:
cargo bench -p forge-core-store -- claim_wal/append
```
