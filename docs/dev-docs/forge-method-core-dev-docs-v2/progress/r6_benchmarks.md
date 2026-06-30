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

## R6.2 (pendente)

Crypto hot paths em `crates/forge-core-crypto/benches/`:
- `verify_rekor_checkpoint`
- `verify_merkle_inclusion`
- `parse_rekor_log_entry` (parse + verify combinados)

Depende de nada crítico; pode ser feito em paralelo com R7/R5/R4.

## Como rodar

```bash
cargo bench -p forge-core-store
# ou focado:
cargo bench -p forge-core-store -- claim_wal/append
```
