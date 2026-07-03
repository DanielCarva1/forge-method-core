# ADR-0011 - `forge-core-eventlog` e o trait `EventSourced`

- **Status**: Accepted (V1.A implementada — crate `forge-core-eventlog` com `trait EventSourced`, `EventEnvelope`, `event_envelope!`; V2.A migrou `forge-core-memory`/`-research`/`-governance`/`-store` JSONL half)
- **Date**: 2026-07-02
- **Track**: V1.A / V2.A — collapse de boilerplate event-sourcing
- **Supersedes**: none
- **Superseded by**: none

## Contexto

Quatro crates no workspace copiaram boilerplate de event-sourcing quase idêntico:
`forge-core-memory`, `forge-core-research`, `forge-core-governance`, e a metade JSONL de
`forge-core-store`. Cada um tinha: um `<X>Event` enum com envelope `sequence`/`at_unix`, um
`<X>Projection { sequence, BTreeMap, superseded, diagnostics }`, um `apply_event` com guarda
de out-of-order, um `replay` fold livre, `project`/`project_locked` (cold-read NDJSON com
tolerância a torn-tail), `next_sequence`, `now_unix`, um shim `append_bytes`, e um quarteto
de erros `{Lock, Append, Serialize, Read}`. O measurement em `forge-core-research` mostrou
que ~62% da crate era o template, não o domínio.

A tentação seria fundir os logs num único log compartilhado para economizar código. Mas
ADR-0010 cravou que os **logs devem permanecer separados** — fundir trust domains distintos
(memory = confiança; research = proveniência de citação) num único event-sourced log
reabre a classe de bug Model B que ADR-0002 tornou irrepresentável. O que estava duplicado
eram as **mecânicas**, não a **separação**.

Havia também um bug latente: a cópia do `forge-core-memory` recomputava
`text.lines().count()` dentro do loop de parse, em cada erro — O(n²) quando o tail estava
torn e toda linha erro. A cópia do `forge-core-research` já havia hoisted o count para fora;
as duas cópias divergiam silenciosamente.

## Decisao

Nova crate `forge-core-eventlog` que absorve as mecânicas (não a separação). O coração é o
trait `EventSourced`:

```rust
pub trait EventSourced {
    type Event: Serialize + DeserializeOwned + Clone + EventEnvelope;
    type Projection: Default + Clone;
    type Diagnostic: Clone;
    fn apply(projection: &mut Self::Projection, event: &Self::Event);
    fn record_diagnostic(projection: &mut Self::Projection, diagnostic: Self::Diagnostic);
    fn sequence_of(projection: &Self::Projection) -> u64;
    fn advance_sequence(projection: &mut Self::Projection, new_sequence: u64);
    fn diagnostic_out_of_order_event_ignored(...) -> Self::Diagnostic;
    fn diagnostic_torn_final_line_skipped(...) -> Self::Diagnostic;
}
```

A crate provê as mecânicas genéricas sobre esse trait:

- `replay` — o fold puro de Fowler (discard e rebuild).
- `project_locked` — cold-read NDJSON com tolerância a torn-tail, sem lock.
- `apply_event` — o corpo compartilhado do fold com guarda out-of-order.
- `next_sequence` / `now_unix` — alocação de sequência e wall-clock.
- `append_event` — serialize → `append_json_line_with_durability` (reuso de `forge-core-store`).
- `EventLogLock` — wrapper RAII sobre `acquire_effect_store_lock`.
- `EventLogError<D>` — o sexteto `{Lock, Append, Serialize, Read, Parse, ProjectionDiagnostic}`, genérico sobre o tipo `Diagnostic` do domínio (default `String`).

Os **associated types são plain `type` aliases, não GATs** — o trait funciona em stable Rust
1.85 sem ginástica de lifetimes. Isto é o padrão eventsourced/evented (o `eventsourced` de
hseeberger é o modelo, mas deliberadamente mais simples: sem tipo `Command`, sem async, sem
persistência de estado evoluído).

A macro `event_envelope!` gera os accessors `sequence()`/`at_unix()` + o impl de
`EventEnvelope` para o enum `Event` do domínio. É `macro_rules!`, **não proc-macro** — zero
build-time cost, nenhuma crate `*-derive` extra, alinhado com a direção Rust Project Goal
2025H1 de reduzir o custo de build de proc-macros. O fold `apply` fica escrito à mão (é
específico do domínio).

## Rationale (o trade-off real)

A alternativa — fundir os logs num só — foi rejeitada por ADR-0010: a fronteira semântica
entre trust domains é o produto inteiro do F14. Esta ADR collapse as **mecânicas**
(`project_locked` triplicado, o quarteto de erros ×7, `event_envelope` ×N) enquanto deixa
cada domínio com seu próprio arquivo de log, lock e projeção. O `project_locked` genérico é
parametrizado por `log_relative_path`/`lock_relative_path` por chamada — nunca assume um
log compartilhado.

O bug O(n²) do memory é corrigido no `project_locked` único: o `total_lines` count fica
hoisted fora do loop. As quatro cópias não podem mais divergir sobre como contar linhas ou
como tratar o tail torn.

## Consequencias

**Positivas:**

- As mecânicas colapsam num só lugar. Um 5º PEP (Policy Enforcement Point) passa a ser um
  PDP (a função `apply`) + 2 braços de `apply_event`, não uma crate de ~1200 linhas.
- O bug O(n²) do memory é corrigido na única cópia `project_locked`; todos os domínios
  migrados herdam o fix.
- Os quartetos de erro ×7 viram um sexteto genérico; o tipo `Diagnostic` do domínio viaja
  pelo `EventLogError<D>` até a fronteira.
- ADR-0010 é honrado byte a byte: cada domínio mantém seu log, lock e projeção. Esta crate
  colapsa mecânicas, não separação.

**Negativas:**

- `EventSourced` tem métodos "factored out" (`sequence_of`, `advance_sequence`, os
  `diagnostic_*`) que expõem detalhes da projeção ao trait. Aceito porque a `Projection` é
  um associated type opaco; o domínio sabe qual campo é o watermark, o trait não.
- `append_event` faz um serialize duplo (event → bytes → `Value` → store helper). Trade
  documentado: corretude e aderência às convenções do store sobre micro-otimização (logs
  de evento são baixo-volume, escala humana).

## Anti-objetivos

- **Não** funde logs: cada domínio continua com seu arquivo, lock e projeção (ADR-0010).
- **Não** introduz `Command` tipo nem async — o kernel permanece determinístico (ADR-0001).
- **Não** é proc-macro: `event_envelope!` é `macro_rules!` por design.

## Referencias

- `eventsourced` (Heiko Seeberger): https://docs.rs/eventsourced
- `evented` (successor de eventsourced): https://docs.rs/evented
- Capital One — building an event-sourcing crate (case study):
  https://www.capitalone.com/tech/software-engineering/event-sourcing-implementation/
- Rust Project Goals 2025H1 (reduce proc-macro build cost).
- In-repo: ADR-0010 (log separation honrado), ADR-0001 (kernel determinístico, sem async),
  `crates/forge-core-eventlog/src/{lib,projection,error,lock,macros}.rs`.
