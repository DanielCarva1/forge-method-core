# ADR-0013 - `OperationGate` trait + typestate Context (gates no kernel)

- **Status**: Accepted (V2.C seam implementada — trait `OperationGate`, typestate `Context<Unverified>`/`<Audited>`, feature `dangerous-bypass`; V3.A preencheu com `RiskAuditGate`/`CitationGate` em `builtin_gates.rs`)
- **Date**: 2026-07-02
- **Track**: V2.C / V3.A — kernel owns its pre-WAL gates
- **Supersedes**: none
- **Superseded by**: none

## Contexto

Os gates de pré-WAL (risk-audit e citation) viviam em `execute_operation.rs` — dentro do
**CLI** — e eram gated apenas por argv flags (`--require-risk-audit`, `--require-citation`).
Um caller do kernel que não fosse o CLI — um teste que invoca `execute_operation` diretamente,
ou um futuro in-process MCP server — **bypassava-os silenciosamente**. Nada na assinatura de
`execute_operation` exigia que os checks tivessem rodado antes.

Isso violava diretamente ADR-0003: o kernel é o **único PDP (Policy Decision Point) para
mutação**. Se os preconditions de mutação são enforcement points vivendo no caller CLI, então
um caller diferente tem um kernel diferente — a própria propriedade que ADR-0003 cravou
irrepresentável. A mutação podia proceder sem que ninguém tivesse consultado o gate.

## Decisao

### 1. `trait OperationGate` no kernel — síncrono, Tower-inspired

```rust
pub trait OperationGate {
    fn evaluate(&self, plan: &RuntimePlan) -> Result<(), GateRejection>;
    fn name(&self) -> &'static str;
}
```

Modelado no padrão `Service`/`Layer` do Tower, mas **síncrono (sem async)** — honra o
constraint de kernel determinístico do ADR-0001. É object-safe (`&self`, dados owned): o
context armazena a chain como `Vec<Box<dyn OperationGate>>`. Um gate é um PEP (Policy
Enforcement Point) que o kernel consulta; o PDP (ruleset de risk-audit, política de
citation) é o que backing o gate. O kernel não sabe o que um gate checa — só sabe que ele
passa ou rejeita com um `GateRejection` tipado.

`GateRejection` é um enum (`RiskAuditFailed { error_count, finding_paths }`,
`CitationCheckFailed { unresolved_source_ids }`, `Custom { code, message }`): carrega
estrutura suficiente para o envelope (V2.D / `TypedFailure`) e o consumer MCP branchear, não
apenas uma string.

### 2. Typestate `Context<Unverified>` vs `<Audited>`

`RuntimeOperationExecutionContext` ganha um parâmetro de estado `<S>`:

- **`Unverified`** — o contexto ainda não passou pela gate chain. **Não pode chamar
  `execute_operation`.** Construído por `single_root`.
- **`Audited`** — o contexto passou pela configuração da chain. **Só este pode chamar
  `execute_operation`.**

A transição é `audited()` (depois de `.with_gate(Box::new(...))`). A assinatura de
`execute_operation` toma `&Context<Audited>` — o typestate faz "os gates foram configurados"
loud no nível de tipo. O planner é invocado antes da gate chain, então o gate recebe o
`RuntimePlan` (o que VAI acontecer), read-only.

### 3. `execute_operation` roda a gate chain internamente, antes do WAL

O preamble da `execute_operation` consulta cada gate em ordem de anexação, contra o plano:
primeiro a rejeitar vence (fail-closed). A rejeição bloqueia o WAL append inteiramente — a
mutação não tem efeito. Isto roda **antes** da própria autorização de `OperationContract` do
kernel e não a substitui.

### 4. Os dois gates viram `impl OperationGate` em `builtin_gates.rs`

`RiskAuditGate` e `CitationGate` são structs públicos no módulo `builtin_gates` do kernel.
Cada um carrega só config + os dados pré-resolvidos (ruleset, evidence registry, runtime
source ids, trace identity) e chama o evaluator **não modificado** em `forge-core-validate`
(`evaluate_risk_audit`, `validate_yaml_citation_references`). O risk-audit gate emite seus
próprios `TraceEvent`s para que `forge explain` narre a auditoria.

### 5. `.dangerous_unchecked()` — escape hatch, jamais silencioso

O bypass explícito segue o padrão `dangerous()` do `rustls`: só disponível sob a feature flag
`dangerous-bypass`, e emite um `tracing::warn!`. Um bypass é **visível no diff E na feature
config** — nunca silencioso. Para testes/legacy callers que genuinamente não precisam de
gates. Callers reais devem preferir `audited()`.

### 6. CLI flags viram config, não localização

`--require-risk-audit` / `--require-citation` decidem **quais gates anexar**, não **onde** o
check roda. O CLI ainda carrega/parseia o ruleset e resolve a metade runtime do Source Ledger
do gate de citation (porque o kernel não pode depender de `forge-core-research` — research
depende de volta via store/validate); esses inputs pré-resolvidos entram como **dados** nas
structs do gate.

## Rationale (o trade-off real)

A alternativa — deixar os gates no CLI e adicionar uma flag booleana `gates_run: bool` a
`execute_operation` — foi rejeitada: uma flag é runtime-state que qualquer caller pode
omitir ou mentir. O typestate move "os gates foram configurados" para o nível de tipo: o
compilador prova que `execute_operation` só é alcançável de um `Context<Audited>`. O
`.dangerous_unchecked()` é o único caminho para `Audited` sem gates, e é gated por feature —
então um binary de produção (sem a feature) fisicamente não pode bypassar.

## Consequencias

**Positivas:**

- O kernel **possui** seus gates de pré-WAL. Um caller de teste ou in-process MCP que invoca
  `execute_operation` diretamente precisa configurar a gate chain para chegar a `Audited`;
  não há caminho silencioso (default binário: sem `dangerous-bypass`, sem bypass).
- CLI flags viram config (quais gates anexar), não localização. ADR-0003 é honrado: o kernel
  é o único PDP para mutação, e agora isso é enforced em tipo, não em disciplina de caller.
- O escape hatch `.dangerous_unchecked()` torna qualquer bypass visível no diff (a chamada
  aparece) E na feature config (`--features dangerous-bypass`). Um reviewer que vê
  `dangerous_unchecked` sabe exatamente o que está acontecendo.
- `GateRejection` flui para `TypedFailure` (V2.D) no envelope — o consumer MCP branchear sem
  parsear prose.

**Negativas:**

- O context deixa de ser `Copy` (possui `Vec<Box<dyn OperationGate>>`). Callers passam por
  referência a `execute_operation`. Aceito: call sites históricos mantêm `single_root`
  (retorna `Unverified`) e adicionam `.audited()`.
- O gate de citation precisa da metade runtime do Source Ledger, que o kernel não pode
  resolver (dependência cíclica via research). Trade aceito: o CLI resolve e passa como dados
  (`runtime_ids: HashSet<String>`) — o kernel fica desacoplado de `forge-core-research`.

## Anti-objetivos

- **Não** introduz async (Tower `Service` é async): o kernel permanece determinístico
  (ADR-0001).
- **Não** remove os flags `--require-risk-audit`/`--require-citation` do CLI — eles viram
  config de quais gates anexar.
- **Não** substitui a autorização de `OperationContract` do kernel; os gates rodam antes e
  são complementares.

## Referencias

- Tokio `Service`/`Layer` (Tower) — pattern síncrono aqui: https://docs.rs/tower
- `rustls` `dangerous_configuration` (escape hatch explícito):
  https://docs.rs/rustls/latest/rustls/server/struct.ServerConfig.html
- Cliff Biffle — typestate pattern in Rust (Rust as a Language for Writing Real Things).
- In-repo: ADR-0001 (kernel determinístico, sem async), ADR-0003 (kernel é único PDP para
  mutação),
  `crates/forge-core-kernel/src/{gate.rs, builtin_gates.rs, wal_orchestration.rs (typestate)}`,
  ADR-0014 (rename `runtime`→`kernel`).
