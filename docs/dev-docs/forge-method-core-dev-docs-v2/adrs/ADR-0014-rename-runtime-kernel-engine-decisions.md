# ADR-0014 - Rename `runtime`→`kernel`, `engine`→`decisions`

- **Status**: Accepted (V0 implementada — crates renomeadas; CONTEXT.md termos canônicos já
  apontam para os novos nomes; docstrings corrigidos)
- **Date**: 2026-07-02
- **Track**: V0 — naming-match-roles
- **Supersedes**: none
- **Superseded by**: none

## Contexto

Dois nomes de crate estavam em desacordo com seus papéis, gerando atrito de navegação a cada
review de arquitetura.

1. **A crate que muta estado** chamava-se `forge-core-runtime`. Mas ADR-0001, `CONTEXT.md`
   e todo outro ADR já a chamavam de **"o kernel"** — a fonte única de verdade para mutação,
   determinística e auditável (ADR-0001), o único PDP para mutação (ADR-0024). O nome
   "runtime" é genérico (qualquer crate de execução pode ser "runtime"); "kernel" diz o
   papel. A divergência entre nome e papel era corrigida verbalmente em cada conversa e
   re-sugerida em cada review.

2. **A crate `forge-core-engine`** tinha um docstring que afirmava ela "sits above the
   runtime executor" — mas ela **não tem dependência alguma** sobre o runtime. São crates
   irmãs (siblings), não camadas empilhadas. Seus módulos (`phase_transition`,
   `claim_engine`, `isolation`, `autonomy_router`, `catalog`, `coordination_eval`,
   `guide_validation`) são **funções puras de decisão**: recebem dados, devolvem um veredito,
   sem IO, sem estado mutável, sem dependência no kernel de mutação. Descrevê-la como
   "acima" do runtime induzia callers a depender de uma relação de camada que não existe.

## Decisao

1. Renomear **`forge-core-runtime` → `forge-core-kernel`** — a crate de mutação, dona de
   `execute_operation` e do append do WAL. Todo path mutante state-bearing flui por ela.

2. Renomear **`forge-core-engine` → `forge-core-decisions`** — uma biblioteca de funções de
   decisão puras e determinísticas: lifecycle de claims, isolamento de worktree, gates de
   transição de fase, routing de autonomia, catálogo de workflows, avaliação de coordenação,
   validação de guide. Toma dados e devolve veredito; **sem IO, sem estado mutável, sem
   dependência no Kernel.** Apenas *decide* o que deveria ser permitido; o Kernel executa a
   mutação.

3. **Corrigir os docstrings.** O docstring de crate de `forge-core-decisions` agora afirma
   explicitamente: "no IO, no mutable state, and **no dependency on the mutation kernel**.
   The only crate-level dependency is the typed `forge_core_contracts` layer". E: "The two
   are sibling crates, not stacked layers — do not describe Decisions as 'sitting above' the
   Kernel."

4. **Documentar os termos canônicos** em `CONTEXT.md` (seções "Kernel (the mutation crate)"
   e "Decisions (the pure-function library)") — já adicionado em V0, apontando para os novos
   nomes e citando esta ADR.

## Rationale (o trade-off real)

A alternativa — deixar os nomes antigos e corrigir verbalmente a cada vez — foi rejeitada
pelo custo recorrente: toda review de arquitetura re-sugeria o rename, todo novato tinha que
ser corrigido sobre "runtime == kernel" e "engine não está acima de nada". O rename é um
custo mecânico de migração pago uma vez; o benefício é que nomes e papéis coincidem para
sempre. A dependência de `decisions` em `contracts` (e em nada mais no nível de crate)
confirma empiricamente o papel: é uma biblioteca de decisão sobre tipos de contrato, não uma
camada sobre o kernel.

## Consequencias

**Positivas:**

- **Nomes casam com papéis.** `forge-core-kernel` é a crate que muta; `forge-core-decisions`
  é a biblioteca de funções puras que decidem. Não há mais divergência nome-vs-papel para um
  reviewer verbalizar.
- **Reviews futuras de arquitetura não re-sugerem o rename.** O custo mecânico foi pago uma
  vez em V0.
- **Decisions é corretamente descrita como biblioteca irmã (sibling), não camada.** O
  docstring de crate previne a confusão "sits above" que induzia dependências equivocadas.
- Os termos canônicos em `CONTEXT.md` ancoram o vocabulário para o resto da documentação.

**Negativas:**

- Migração mecânica: import paths, `Cargo.toml` deps, `use` statements, refs em docs. Pago
  em V0; downstream crates já apontam para os novos nomes.
- Qualquer branch/PR pré-V0 que ainda referencia os nomes antigos precisa do rename
  aplicado. Aceito como custo normal de um rename.

## Anti-objetivos

- **Não** muda responsabilidades das crates — só nomes e docstrings. O que muta continua
  mutando; o que é decisão pura continua decisão pura.
- **Não** cria uma relação de camada entre `decisions` e `kernel` — elas permanecem
  siblings. `decisions` não depende de `kernel`.

## Referencias

- In-repo: ADR-0001 (kernel determinístico), ADR-0024 (kernel é único PDP para mutação),
  `CONTEXT.md` (seções "Kernel (the mutation crate)", "Decisions (the pure-function
  library)"), `crates/forge-core-kernel/src/lib.rs`,
  `crates/forge-core-decisions/src/lib.rs`.
