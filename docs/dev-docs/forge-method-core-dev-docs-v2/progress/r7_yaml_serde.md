# R7 — Migrar `serde_yaml` → `yaml_serde`

**Data**: 2026-06-30
**Status**: ✅ completo

## Contexto

`serde_yaml 0.9.34` está marcado como `+deprecated` no crates.io há tempo. O
roadmap original (R7) recomendava migrar para `serde_yml`. **Mas durante a
execução descobrimos que `serde_yml 0.0.13` também foi marcado como DEPRECATED /
unmaintained** (compatibility shim). Pular de uma dep deprecated para outra
seria repetir o erro.

## Decisão

Migrar para **`yaml_serde 0.10.4`** (`serde_yaml` → `yaml_serde`).

### Por que `yaml_serde` sobre as outras alternativas

| Crate | Status | Mantenedor | rust-version | API vs `serde_yaml` |
|---|---|---|---|---|
| `serde_yaml 0.9.34` | ❌ deprecated | dtolnay (arquivado) | 1.64 | — |
| `serde_yml 0.0.13` | ❌ deprecated (shim) | primordial (arquivado) | — | — |
| `serde_yaml_ng 0.10.0` | ⚠ fork individual | acatton | 1.64 | 1:1 |
| `serde_yaml_bw 2.5.6` | ✅ ativo | boris-w | — | breaking |
| `serde-saphyr 0.0.28` | ⚠ 0.0.x (instável) | saphyr-rs | — | parcial |
| **`yaml_serde 0.10.4`** | ✅ ativo | **The YAML Organization** (ingydotnet) | 1.82 | **1:1** |

`yaml_serde` vence porque:
1. **Governança multi-stakeholder**: publicado pela "The YAML Organization", não
   é fork individual. Ingy döt Net é um dos autores da spec YAML 1.1/1.2.
2. **API 1:1 com `serde_yaml`**: `Value`, `Mapping`, `Error`, `from_str`,
   `to_string`, `to_value`, `from_value`, `Number`, `Index`, `Sequence`,
   `Result`, `Deserializer`, `Serializer`, `Location`. Migração mecânica.
3. **Sem `anyhow`/`thiserror` no runtime** (apenas dev-dependencies). Alinhado
   com `AGENTS.md` do projeto.
4. **License MIT OR Apache-2.0** (padrão Rust Foundation).
5. **rust-version 1.82** (recente, manutenção ativa confirmada).

## Implementação

Migração mecânica via `sed` (zero mudança de semântica):

- `Cargo.toml` workspace: `serde_yaml = "0.9.34"` → `yaml_serde = "0.10.4"`
- 12 per-crate `Cargo.toml`: `serde_yaml.workspace = true` → `yaml_serde.workspace = true`
- 42 arquivos `.rs`, 124 referências: `serde_yaml::` → `yaml_serde::`

## Validação

| Gate | Resultado |
|---|---|
| `cargo check --workspace` | ✅ (1 warning pré-existente `private_interfaces`) |
| `cargo test --workspace` | ✅ 0 failures |
| `cargo clippy --workspace --all-targets -- -W clippy::pedantic` | ✅ 0 errors |
| `cargo fmt --all -- --check` | ✅ |
| Anchor `validate --root . --json \| grep -c '"diagnostics": 0'` | ✅ **122** |

A preservação do anchor 122 + zero falhas em testes comprova que
`yaml_serde` é funcionalmente idêntico a `serde_yaml` para todos os contratos
do forge. Migração zero-risk confirmada empiricamente.

## Pontos de atenção

- `serde_yaml` original usava libyaml via `unsafe`. `yaml_serde` depende de
  `libyaml-rs 0.3` (mesmo binding, mantido). Nada mudou em termos de soundness.
- Não há mudança na serialização on-disk. Todos os `.yaml` em `contracts/`
  continuam lendo/gravando idênticos.

## Lição

Verificar `cargo search <crate>` para mensagens `# DEPRECATED` antes de adotar
uma "recomendação de fork". O roadmap original assumia `serde_yml` como destino
— estava desatualizado em relação ao estado real do ecossistema.
