# ADR-0008 - Fuzz runs on Linux CI, not Windows local

- **Status**: Accepted

## Contexto

O track R4 (`cargo-fuzz`) precisa de um ambiente onde o fuzz rode de forma
reproduzível e contínua. Tentativas no host de desenvolvimento Windows-MSVC
mostraram duas limitações conhecidas do cargo-fuzz nessa plataforma:

1. **ASAN default (`-Zsanitizer=address`)** falha em runtime com
   `STATUS_DLL_NOT_FOUND (0xc0000135)`. O toolchain `nightly-x86_64-pc-windows-msvc`
   não embarca `clang_rt.asan_dynamic.dll`; apenas a static lib existe, e o
   linker MSVC não consegue resolver a DLL dinâmica esperada pelo binário
   instrumentado.

2. **Coverage-only (`-s none`)** falha em link com erros
   `LNK2001: __stop___sancov_pcs` indefinido. O `-Zbuild-std` rebuilda a
   stdlib sem coverage instrumentation, mas os crates externos (tokio, hyper,
   reqwest, etc.) ainda assim são compilados com coverage e referenciam os
   símbolos de seção que não existem mais no link final.

WSL2 Ubuntu está disponível no host mas não tem toolchain Rust instalado;
instalar Rust/nightly lá só para rodar fuzz adicionaria um ambiente extra a
manter sem trazer benefício sobre um CI GitHub Actions Linux.

## Decisao

Fuzz é executado em **CI GitHub Actions Linux (`ubuntu-latest`)**, não
localmente em Windows. A configuração vive em
`.github/workflows/fuzz.yml` e roda:

- Schedule noturno (cron) — captura regressões silenciosas
- `workflow_dispatch` — execução manual sob demanda
- Pull requests com label `fuzz` — gate opcional quando um PR mexe em
  parsers críticos

Cada um dos 4 targets (`parse_signed_checkpoint`,
`parse_rekor_log_entry`, `decode_ocsp_response`, `decode_prefix`) roda por
um tempo limitado (5 min cada no CI) com os seeds commitados em
`fuzz/corpus/<target>/`.

Harnesses `.rs` + `fuzz/Cargo.toml` + `fuzz/.gitignore` + corpus seeds
permanecem commitados no repo. Desenvolvedores em Linux/WSL podem rodar
`cargo +nightly fuzz run <target>` localmente; desenvolvedores em Windows
devem usar o CI ou um ambiente Linux.

## Consequencias

- **Adoção madura**: cargo-fuzz é upstream Linux-first; essa escolha
  alinha com a manutenção do upstream.
- **Custo CI**: workflow noturno consome ~20 min de GitHub Actions por dia.
  Aceitável dado o valor de regressão contínua.
- **Feedback loop**: PRs normais não bloqueiam em fuzz (apenas com label
  `fuzz`). Bug encontrado no cron vira issue separado.
- **Documentação**: README de dev-docs precisa explicar "como rodar fuzz
  localmente em Linux" e "como interpretar um artefato de crash".
- **Reversibilidade**: se o suporte Windows do cargo-fuzz amadurecer (DLL
  ASAN embarcada, fix do `__stop___sancov_pcs` em `-s none`), este ADR pode
  ser revertido sem mudança de código — apenas removendo a documentação da
  limitação.
