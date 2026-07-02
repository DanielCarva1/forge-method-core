# ADR-0007 - Multi-principal governance

Status: **Accepted** (2026-07-01; expanded from `proposto`)

## Contexto

Agentes de pessoas, orgs ou vendors diferentes podem trabalhar no mesmo shared
state. Lane claims nao bastam quando ha principals diferentes — elas serializam
writes por path, mas nao modelam *quem* declara a intent, nem fazem do conflito
um objeto estruturado (o NFR do F07: "conflito vira objeto estruturado, nao
merge silencioso").

Este ADR formaliza o design do F07 (expandindo o stub original). Tres frentes
de pesquisa paralela (modelos de governance RBAC/ReBAC/Cedar/Zanzibar; o seam
de deteccao de conflito no codebase; e a questao R8 `PrincipalId` vs `StableId`)
convergem numa arquitetura de tres camadas e resolvem uma contradicao com o
ADR 0002.

## Decisao

### 1. Modelo de tres camadas (formato GaaS)

O achado decisivo: **RBAC, ReBAC, ABAC, Cedar e Zanzibar respondem todos a
pergunta de um unico principal** ("P pode fazer A em R?") e **nao tem semantica
de contencao**. O requisito do F07 ("dois principals com intents sobrepostas →
emite um ConflictContract estruturado, nunca merge silencioso") e um problema de
**coordenacao**, nao de autorizacao. Confluir os dois e o anti-pattern central.

- **Camada de autorizacao (ReBAC/Cedar):** `GovernancePolicy` + `PrincipalId`
  modelam *quem e o principal* e *que autoridade ele tem*. Cedar
  `(principal, action, resource) → Decision` e o PDP (consistente com
  ADR 0002/0003). Esta camada **nao** detecta conflito A↔B.
- **Camada de coordenacao (intent-locks de Gray):** `IntentContract` = um
  intent-lock sobre um authority scope (sub-arvore de paths) com **expiração**
  (lease). Conflito = sobreposicao, detectada pela matriz de compatibilidade de
  locks. Precedentes: Gray 1976 (multiple-granularity intent locks); Calvin
  (ordenacao deterministica); Spanner (bound temporal). O campo `expires_at` e
  **load-bearing** (liveness/correctness), nao opcional.
- **Camada de conflito (objeto first-class, NAO merge silencioso):**
  `ConflictContract` e uma **entidade first-class** (refs das duas intents +
  scope contestado + reason + estado de resolucao). A literatura e decisiva:
  sistemas que resolvem silenciosamente (CRDTs, OT, Figma LWW, XACML combining
  algorithms, Zanzibar per-tuple LWW) **destroem o sinal de conflito**; sistemas
  que casam o requisito do F07 (Git markers, Apel semistructured merge,
  Berenson anomalies) fazem do conflito um **objeto nomeado, tipado, que para o
  fluxo**. O F07 esta na linhagem Git/Apel/Berenson.

Nenhum padrao de governance de agentes cobre contencao de recursos ainda (MAST
NeurIPS 2025 arXiv:2503.13657; GaaS arXiv:2508.18765 sao 2025-26, em formacao) —
o design e research-grounded, nao standards-compliant.

### 2. `PrincipalId` tipado (supersede a previsao do ADR 0002)

O ADR 0002 (Accepted) afirma "F07 does not introduce a rival PrincipalId type"
— uma *previsao* sobre o F07 feita para decidir a pergunta do F06. A spec do
F07 (`01_feature_specs.md:215`) e este ADR a contradizem.

**Decisao: introduzir `PrincipalId`** como newtype distinto
(`pub struct PrincipalId(pub String)`, `#[serde(transparent)]`, mesmos derives
que `ScopeId`/`ClaimId`). Justificativa R8: as estruturas de autorizacao do F07
(`IntentContract { principal, authority_scope }`, `ConflictContract { principal_a,
principal_b }`, `is_authorized(principal, resource)`) colocam um id de principal
e um id de recurso na mesma comparacao, onde um swap de campo/argumento e um bug
silencioso de seguranca — exatamente a classe que o split `ScopeId`/`ClaimId`
tornou irrepresentavel. Um `PrincipalId` distinto transforma esse swap em erro
de compilacao. Os precedentes industriais que o proprio ADR 0002 cita (AWS Cedar,
Google Zanzibar) impoem separacao tipada Principal/Resource pela mesma razao.

Isto **supersede formalmente a previsao do ADR 0002** (registro de decisao
datado, nao override silencioso). O campo `reviewed_by` (F06) migra de
`Option<StableId>` para `Option<PrincipalId>` para consistencia
(one-concept-one-type). Como `PrincipalId` e `#[serde(transparent)]`, YAML
legado (`reviewed_by: principal.daniel`) continua parseando — custo de migracao
zero, o padrao ScopeId comprovado.

**Rejeitado: type alias** (`type PrincipalId = StableId`). Aliases sao
transparentes — o compilador trata os dois como identicos, entao
`f(reviewer: PrincipalId)` ainda aceita um `run_id: StableId`. Protecao R8 zero,
ilucao de distincao.

### 3. Seam de deteccao de conflito (para F07.4)

A deteccao vive **no acquire do claim engine**
(`crates/forge-core-decisions/src/claim_engine.rs:317`, chamado de
`claim.rs:295`). Dois principals com intents sobrepostas em repo-paths **ja sao
bloqueados la** (`PathAlreadyClaimed`/`AlreadyClaimedByOther`) — o F07.4 apenas
reformula essa rejeicao flat num `ConflictContract` estruturado, reaproveitando
os dados de atribuicao que o acquire ja computa (`holder`, `blocking_claim_id`,
`expires_at`, path sobreposto). A camada WAL (`claim_wal.rs`) permanece um
serializador burro — sem politica no IO. Writes de memoria sao um gap *separado*
de capability-governance (o verb `memory review` deferido), nao um conflito de
path.

## Consequencias

- Conflitos viram objetos estruturados (`ConflictContract`), nao merge manual
  silencioso. O NFR do F07 e satisfeito na camada de schema.
- Overwrite silencioso fica bloqueado por design (`ConflictPolicy::EmitContract`
  e o default; `SilentLastWriterWins` gera warning de validacao).
- Arbitragem humana fica auditavel (`ConflictResolutionState::{Pending, Resolved,
  Escalated}` + ledger append-only no F07.5).
- Forge vira camada diferenciada de shared state agentic — research-grounded,
  alinhado com a direcao emergente de 2025-26 (MAST, GaaS).
- O `PrincipalId` tipado torna a classe de bug principal↔resource
  irrepresentavel em tempo de compilacao (R8).
- `reviewed_by` migra para `PrincipalId` sem custo (serde-transparent); o verb
  `memory review` (F06, deferido) fica desbloqueado conceitualmente — so depende
  agora do F07.4 (wire do governance) e F07.6 (CLI).

## Escopo desta story (F07.1-F07.3)

- ✅ F07.1: este ADR (Accepted; supersede da previsao do ADR 0002).
- ✅ F07.2: `PrincipalId` newtype em `common.rs`; migracao de `reviewed_by`.
- ✅ F07.3: `governance.rs` (`GovernancePolicy`, `IntentContract`,
  `ConflictContract` + enums) + validator com diagnostics tipados + fixtures.
- ⏳ F07.4: wire do `ConflictContract` no `claim_engine.rs:317`.
- ⏳ F07.5: arbitration ledger (append-only).
- ⏳ F07.6: CLI `forge-core governance intent/conflicts/arbitrate`.
- ⏳ F07.7: fixtures + E2E (2 principals disputando → ConflictContract emitido).

## Referencias

- Gray 1976 — Granularity of Locks (intent locks, MGL):
  https://www.cs.cmu.edu/~natassa/courses/15-721/papers/GrayLocks.pdf
- Sandhu RBAC96 (1996); NIST RBAC (2000):
  http://www.cs.toronto.edu/~jm/2507S/Readings/13.Sandhu96.pdf
- Berenson et al. 1995 — A Critique of ANSI SQL Isolation Levels (anomalies
  como fenomenos nomeados): https://doi.org/10.1145/568271.223785
- Calvin (Thomson et al. 2012):
  https://cs.yale.edu/homes/thomson/publications/calvin-sigmod12.pdf
- Zanzibar (Pang et al. 2019):
  https://www.usenix.org/system/files/atc19-pang.pdf
- Spanner (TrueTime / commit-wait):
  https://docs.cloud.google.com/spanner/docs/true-time-external-consistency
- Apel et al. — Semistructured merge:
  https://www.se.cs.uni-saarland.de/publications/docs/CBS%252B19.pdf
- XACML 3.0 (combining algorithms = resolucao silenciosa, o anti-pattern):
  https://docs.oasis-open.org/xacml/3.0/xacml-3.0-core-spec-cd-03-en.html
- Cedar (arXiv 2403.04651, 2024): https://arxiv.org/abs/2403.04651
- MAST (arXiv 2503.13657, NeurIPS 2025 — multi-agent failure modes):
  https://arxiv.org/pdf/2503.13657
- GaaS (arXiv 2508.18765, 2025): https://arxiv.org/abs/2508.18765
- Tian Pan — Conflict Resolution Patterns for Parallel AI Systems (2026):
  https://tianpan.co/blog/2026-05-02-multi-agent-conflict-resolution-disagreement-patterns
- In-repo: ADR 0002 (memory trust model — previsao supersedida); ADR 0003
  (PDP/PEP); `common.rs` (R8 + `ScopeId`/`ClaimId` precedent);
  `claim_engine.rs:317` (o seam do F07.4);
  `conflict_detection.rs:253` (`repo_paths_overlap`, o primitivo reusavel).
