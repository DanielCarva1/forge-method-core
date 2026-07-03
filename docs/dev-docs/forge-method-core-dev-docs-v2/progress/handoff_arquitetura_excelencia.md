# Handoff — Arquitetura de Excelência

Data: 2026-07-02
Estado: **V0–V4 completos e verdes** (cargo check + clippy -W pedantic + fmt --check + test suite, 1138+ testes passando).

## Resumo

Refatora arquitetural completa endereçando os 7 problemas identificados pela
`improve-codebase-architecture` skill, organizada em 5 versões (V0–V4) com
paralelização máxima via subagentes. Fundamentada em padrões-ouro comprovados:
rustc `DiagCtxt`, `eventsourced`/`evented`, DataFusion facade, Tokio
`Service`/`Layer` (variante síncrona), typestate builder (rustls
`dangerous()` pattern), annotate-snippets, clippy/deno_lint const-table de
códigos.

## Done (commits)

| Commit | Versão | Resumo |
|--------|--------|--------|
| `7ab83d91` | V0 | Rename `runtime`→`kernel`, `engine`→`decisions` (+ docstrings + CONTEXT.md) |
| `06f89133` | V1 | 4 fundações: `forge-core-eventlog` (trait EventSourced), acumulador canônico de diagnósticos, seams internos do kernel, consolidação CliEnvelope emit |
| `bd73a006` | V2 | 4 migrações: 3 PEP crates → eventlog, 4 famílias de diagnóstico → canônico, OperationGate + typestate Context, TypedFailure no envelope |
| `59a1314d` | V3 | Gates (risk-audit + citation) movidos para o kernel; allowlist MCP como projeção validada de COMMANDS |
| `3b3838f0` | V4 | Fricções menores + ADRs 0011–0014 + CONTEXT.md |

## Mapeamento dos 7 problemas → solução

1. **Boilerplate de event-sourcing copiado 4×** → `forge-core-eventlog`
   (trait `EventSourced`); ~62% de `forge-core-research` (o template) sumiu;
   logs continuam separados (ADR-0010 honrada). Bug O(n²) do torn-line no
   memory consertado na única `project_locked` compartilhada.
2. **5 famílias de diagnósticos clonadas** → canônico em `forge-core-validate`;
   `format!("{:?}")` que degradava `DiagnosticCode` na fronteira do CLI foi
   removido; `DiagnosticCodeDef` const-table + macro (rustc Lint pattern).
3. **Gates de mutação no CLI (bypassable)** → movidos para o kernel via
   `impl OperationGate` em `builtin_gates.rs`; typestate `<Unverified>` vs
   `<Audited>` torna bypass erro de compilação; `.dangerous_unchecked()`
   feature-flagged (rustls pattern) para o caso legítimo de teste.
4. **CliEnvelope emit copiado 7×** → única `emit_envelope`/`emit_envelope_with`;
   drift da `autonomy_cmd` (imprimia "lane") resolvida como override legítimo;
   `coordination.rs` dobrado do estilo return-tuple.
5. **Mutate path cruzava 4 vocabulários de erro com 2 colapsagens lossy** →
   `TypedFailure` adjacently-tagged no envelope; `From<&ExecuteOperationError>`
   preserva variantes; modo `--json` agora emite envelope estruturado com exit
   code DD10 em vez de string lossy + exit 1.
6. **Kernel flat de 2103 linhas sem seams** → 4 módulos privados
   (planning/staging/evidence/wal_orchestration) + fachada `pub use`;
   `runtime`/`engine` renomeados para `kernel`/`decisions` (nomes refletem o
   papel); interface pública 1:1 preservada.
7. **Fricções menores** → `forge-contract-validator` documentado como shim;
   `unwrap()`s em paths críticos → erros tipados (fail-closed no attestation
   era NATIVAMENTE fail-closed, só test helpers tinham unwrap); rename
   `OperationReferencePolicyDocument` → `OperationCrossReferencePolicyDocument`;
   allowlist MCP validada contra COMMANDS (dev-dep cycle benigno).

## Critical findings (surpresas da execução)

1. **`emit_envelope_or_err` NÃO era unused** (contra o relatório da revisão
   inicial): `claim.rs`/`isolation.rs` o usam (~20 sites) com contrato de
   texto DIFERENTE das 7 cópias gêmeas. Reescrevê-lo teria quebrado
   silenciosamente o output de claim/isolation. Decisão: adicionar
   `emit_envelope` irmã, deixar o legário intacto documentado.
2. **Attestation `unwrap()`s eram em TEST HELPERS**, não na fronteira de
   segurança. O path de verificação de produção já era fail-closed
   (canonicalize failure → `AttestationError` → `AttestationGateOutcome::Invalid`
   → `rejection_result`). Convertidos a `.expect()` comentados.
3. **ADR-0009 já existia** — o achado "missing" da revisão inicial era falso
   alarme (snapshot pré-criação). 13 citações verificadas corretas.
4. **V2.B cresceu o `DiagnosticCode` enum** em vez de ir direto para
   const-table completa — escolha pragmática (as 4 famílias clonadas tinham
   seus próprios enums de código que precisavam fundir; aditivo com
   `#[serde(rename)]` preservando wire strings foi o caminho sem flag-day).
   A const-table (`DiagnosticCodeDef` + macro) fica como o seam para a migração
   futura completa do enum.
5. **`DiagnosticCode` PascalCase → snake_case no wire** (efeito colateral de
   matar o `format!("{:?}")`): códigos agora serializam `yaml_read_failed` em
   vez de `YamlReadFailed`. Intencional, melhor identificador estável.
6. **Cycle benigno em V3.B**: adapter ganha `forge-core-cli` como DEV-dep
   (para validar allowlist contra COMMANDS). Cargo permite cycles de dev-dep
   em path-dep workspaces; só `cargo publish` os bloquearia (não aplicável).

## Pending decisions

- **Const-table completa vs enum híbrido** (V2.B follow-up): hoje o
  `DiagnosticCode` enum cresceu com variantes graph/eval/harness, e a
  const-table (`DiagnosticCodeDef`) é o seam paralelo. Migrar o enum inteiro
  para const-table é trabalho futuro (tocaria todo call site de match). A
  pesquisa endossa const-table (rustc/clippy/deno_lint); o enum funciona hoje.
- **`emit_envelope_or_err` legacy**: com `emit_envelope` canônico
  estabelecido, migrar claim/isolation para o novo e deprecate o legário é
  follow-up. Prioridade baixa (o legário funciona, está documentado).
- **Snapshots incrementais no eventlog**: a pesquisa mostrou que snapshot
  full-state periódico (já existente no `claim_wal`) é o padrão-ouro de facto;
  snapshot delta-only é raramente adotado. Não implementar a menos que um
  domínio cresça a ponto de replay ficar caro.

## Next steps

1. **Context-aware follow-up**: nada bloqueante. O refactor está
   arquiteturalmente completo e verde. Work futuro é incremental.
2. **`/clear` recomendado**: esta sessão acumulou contexto pesado (múltiplos
   subagentes, vários arquivos grandes lidos). Próxima tarefa = contexto limpo.
3. Se for continuar nesta sessão, focar em uma das pending decisions acima.

## Key files

### Novos (criados pelo refactor)
- `crates/forge-core-eventlog/src/{lib,projection,lock,error,macros,tests}.rs` — a trait + mecânicas genéricas
- `crates/forge-core-kernel/src/gate.rs` — `OperationGate` trait + `GateRejection`
- `crates/forge-core-kernel/src/builtin_gates.rs` — `RiskAuditGate` + `CitationGate` impls
- `crates/forge-core-kernel/src/{planning,staging,evidence,wal_orchestration}.rs` — seams internos
- `crates/forge-core-contracts/src/typed_failure.rs` — `TypedFailure` adjacently-tagged
- `crates/forge-core-validate/src/{codes,failure}.rs` — const-table + serde constants
- `docs/dev-docs/forge-method-core-dev-docs-v2/adrs/ADR-0011..0014-*.md`

### Significativamente modificados
- `crates/forge-core-kernel/src/lib.rs` — fachada fina (`pub use module::*`)
- `crates/forge-core-kernel/src/wal_orchestration.rs` — typestate Context + gate preamble
- `crates/forge-core-cli/src/execute_operation.rs` — gates viraram config (`.with_gate`), `From<&ExecuteOperationError> for TypedFailure`, DD10 exit codes
- `crates/forge-core-cli/src/cli_util.rs` — `emit_envelope`/`emit_envelope_with` canônicos
- `crates/forge-core-memory/{lib,admission,promote,retention,error}.rs` — implementa `EventSourced`, deleta boilerplate
- `crates/forge-core-research/{lib,admission,error}.rs` — idem (`graph.rs` intocado)
- `crates/forge-core-governance/{lib,record,arbitrate,escalate,error}.rs` — idem
- `crates/forge-core-validate/src/lib.rs` — `error_count`/`warning_count`, variantes adicionadas
- `crates/forge-core-validate/src/risk_audit.rs` — ganhou `collect_risk_audit_targets` (movida do CLI)
- `crates/forge-core-protocol-mcp/src/allowlist.rs` — validação contra COMMANDS
- `CONTEXT.md` — termos canônicos (Kernel, Decisions, EventSourced, OperationGate, TypedFailure, DiagnosticCodeDef)

## Anti-objetivos honrados (não fizemos)

- Não introduzimos `tower`/`tokio` no kernel síncrono (variante síncrona do pattern).
- Não migrámos risk-audit YAML para Cedar/OPA (over-engineering hoje).
- Não adotámos `miette`/`codespan` (codespan deprecated; annotate-snippets é renderização fora de escopo).
- Não migrámos `claim_wal` para `forge-core-eventlog` (framing binário distinto).
- Não introduzimos proc-macros (`macro_rules!` alinhado ao Rust Project Goal 2025H1).
