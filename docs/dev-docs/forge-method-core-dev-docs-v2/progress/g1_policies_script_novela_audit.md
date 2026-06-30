# G1 — Auditar `contracts/policies/*.yaml` por script-de-novela

**Data**: 2026-06-30
**Status**: ✅ completo
**Critério DoD**: Report declarando X/Y policies são framework paramétrico, Z são script prescritivo.

## Sumário executivo

**62/62 policies são framework paramétrico. 0/62 são script de novela.**

O conjunto inteiro de `contracts/policies/*.yaml` é consistentemente
declarativo: define invariantes, regras com IDs, gates de aceitação,
tradeoffs, evidência científica e failure-modes-prevented. Nenhum arquivo
prescreve sequência rígida de execução, palavras exatas para o agente
repetir, ou passo-a-passo imperativo ("faça A, depois B, depois C").

Isso confirma a diretiva central de Forge: o protocolo é um **guia
paramétrico que escala com a capacidade dos agentes**, não um script de
novela que competa letra-por-letra com o modelo.

## Metodologia

Para cada um dos 62 arquivos em `contracts/policies/`, aplicamos três
heurísticas em camadas:

### Heurística 1 — Padrões lexicais de script

Buscamos por construções sintáticas características de script prescritivo:

```
^[- ]*step[-_ ]?[0-9]           # step_1, step-2, Step 3
step[-_ ]?[0-9]+:               # step_1: (YAML key)
first.+then                     # "first do X then do Y"
then.+finally                   # "then X, finally Y"
after that                      # sequence-strict
next you                        # imperative next-step
tell the user                   # scripted human response
say.+exactly                    # word-for-word directive
respond with                    # canned response
word for word                   # verbatim directive
```

**Resultado**: 62 arquivos, 0 ocorrências reais. Um único falso positivo em
`legacy-validator-parity-matrix-boundary.yaml` (`matrix_first_overlap_tests_then_wrapper_decision`
— nome de estratégia, não script).

### Heurística 2 — Padrões semânticos de resposta canned

```
exactly this
use this (phrase|wording|text)
respond to the (human|user) with
say:
word[- ]for[- ]word
copy this
template response
prose response
scripted response
canned response
narrative step
```

**Resultado**: 0 ocorrências em todos os 62 arquivos.

### Heurística 3 — Strings de prosa longa (>200 chars)

Scripts de novela costumam conter blocos longos de prosa prescritiva.
Listamos strings >200 chars para inspeção manual.

**Resultado**: várias strings >200 chars existem, mas inspeção manual
confirma que todas são declarações técnicas densas (`summary:`, `purpose:`,
`inference_boundary:`, `failure_modes_prevented:`) descrevendo o que a
policy **faz**, não prescrevendo **como o agente deve falar ou em que
ordem deve agir**.

### Heurística 4 — Padrões de framework paramétrico (controle positivo)

Confirmamos presença de estruturas declarativas saudáveis:

```
^[- ]*id:                       # rules/principles nomeados
^rules:                         # bloco de regras
^principles:                    # bloco de princípios
^implications:                  # causa-efeito estruturado
^acceptance:                    # gate de aceitação
^tradeoffs:                     # tradeoffs explícitos
^evidence_basis:                # lastro científico
^invariants:                    # invariantes
^failure_modes_prevented:       # defense-in-depth negativa
```

**Resultado**: média de ~13 ocorrências por arquivo (mínimo 5, máximo 25).
Todos os 62 arquivos contêm múltiplos marcadores de framework paramétrico.

## Estrutura típica observada

Um policy Forge canônico (ex.: `rust-validation-authority.yaml`) segue o
formato:

```yaml
schema_version: "0.1"
policy: <name>
status: accepted | active | proposto
purpose: <declarativo, uma frase>

rules:                          # IDs nomeáveis, regras invariantes
  - id: <snake_case>
    rule: <declarativa>

acceptance:                     # gate, não prescrição
  - <critério verificável>

tradeoffs:                      # explicitação
  chosen:    { advantages, disadvantages }
  rejected:  [ { id, reason } ]

evidence_basis:                 # lastro científico
  direct_patterns: [ { source_id, supports } ]
  inference_boundary: <scope>

failure_modes_prevented:        # defense-in-depth negativa
  - <anti-objetivo>
```

Nenhum policy dita ordem de execução nem palavras a serem faladas. Todos
expressam o que deve ser verdadeiro, deixando o agente livre para usar sua
capacidade plena de planejamento e linguagem para alcançar os invariantes.

## Samples representativos lidos manualmente

Para validar a heurística, lemos por completo:

- `human-agent-interface.yaml` (57 linhas) — bússola do produto, 3
  princípios com implications. Framework puro.
- `rust-workspace-architecture.yaml` (178 linhas) — maior policy, define
  crates com responsabilidades + `must_not_depend_on` + `build_order`
  com gates de aceitação por estágio. Framework puro.
- `rust-validation-authority.yaml` (25 linhas) — menor policy, 4 regras
  com IDs + acceptance evidence. Framework puro.

## Por que isso importa para o produto

Forge promete ser um **protocolo-guia** que escala com a capacidade dos
agentes, não um **roteiro** que compete com a fluência do modelo. Se as
policies fossem scripts de novela:

1. **Acoplariam o protocolo a uma geração específica de modelo** — qualquer
   modelo novo tornaria o script obsoleto.
2. **Apagariam vantagem do agente** — em vez de usar LLM pra planejar e
   explicar, viraria um executor de passos determinísticos.
3. **Seriam frágeis a mudança de contexto** — scripts não se adaptam a
   situações não-previstas; invariantes sim.

A auditoria confirma que o conjunto atual honra a promessa. Policies são
invariantes testáveis e parametrizáveis, deixando o agente livre para
operar com sua capacidade máxima.

## Recomendações

1. **Manter a disciplina.** Qualquer nova policy que apareça em `contracts/policies/`
   deve ser validada contra as 4 heurísticas antes do merge. Sugerimos
   adicionar isso ao hook de validation futuro (fora do escopo de G1).
2. **G2 está liberado para prosseguir.** Como nenhuma policy é script, G2
   (fixtures que provam framework) pode focar em testar múltiplos inputs
   contra as policies existentes sem refactor prévio.
3. **Não criar ADR.** Esta auditoria confirma que o estado atual está
   alinhado com a intuição documentada em `human-agent-interface.yaml`.
   Não há decisão hard-to-reverse para registrar; a disciplina existente
   já captura o trade-off.

## Apêndice — Inventory completo

62 arquivos auditados (todos marcados `framework`):

`advanced-boundary-type-order.yaml`,
`certificate-revocation-policy-boundary.yaml`,
`certificate-transparency-sct-boundary.yaml`,
`coordination-eval-type-boundary.yaml`,
`explicit-crl-revocation-status-boundary.yaml`,
`explicit-ocsp-revocation-status-boundary.yaml`,
`filesystem-reference-index-adapter-boundary.yaml`,
`generic-known-ref-validation-boundary.yaml`,
`generic-yaml-evidence-ref-validation-boundary.yaml`,
`health-recovery-type-boundary.yaml`,
`host-adapter-manifest-boundary.yaml`,
`host-adapter-manifest-projection-boundary.yaml`,
`human-agent-interface.yaml`,
`installer-trust-and-distribution-boundary.yaml`,
`legacy-stdout-compatibility-boundary.yaml`,
`legacy-validator-parity-matrix-boundary.yaml`,
`legacy-validator-parity-retirement-boundary.yaml`,
`legacy-validator-retirement-policy-boundary.yaml`,
`mcp-local-process-security-boundary.yaml`,
`operation-contract-rust-type-strictness.yaml`,
`operation-executor-artifact-storage-projection-boundary.yaml`,
`operation-executor-index-compaction-boundary.yaml`,
`operation-executor-metadata-adapter-integration-boundary.yaml`,
`operation-executor-metadata-consumer-boundary.yaml`,
`operation-executor-metadata-context-builder-boundary.yaml`,
`operation-executor-metadata-index-boundary.yaml`,
`operation-executor-metadata-reader-boundary.yaml`,
`operation-executor-metadata-rebuild-boundary.yaml`,
`operation-executor-payload-adapter-boundary.yaml`,
`operation-executor-repair-cli-boundary.yaml`,
`release-artifact-verification-boundary.yaml`,
`request-effect-type-boundary.yaml`,
`runtime-command-evidence-recording-boundary.yaml`,
`runtime-command-runner-boundary.yaml`,
`runtime-effect-application-boundary.yaml`,
`runtime-effect-staging-boundary.yaml`,
`runtime-effect-transaction-boundary.yaml`,
`runtime-effect-wal-compaction-and-locking-boundary.yaml`,
`runtime-effect-wal-recovery-boundary.yaml`,
`runtime-handoff-type-boundary.yaml`,
`runtime-store-adapter-integration-boundary.yaml`,
`runtime-store-state-read-boundary.yaml`,
`rust-contract-type-order.yaml`,
`rust-only-backlog-reconciliation.yaml`,
`rust-runtime-execution-boundary.yaml`,
`rust-validation-authority.yaml`,
`rust-workspace-architecture.yaml`,
`schema-enum-parity-boundary.yaml`,
`schema-generation-authority.yaml`,
`side-contract-type-order.yaml`,
`signature-and-provenance-verification-boundary.yaml`,
`sigstore-bundle-subject-binding-boundary.yaml`,
`sigstore-dsse-in-toto-subject-boundary.yaml`,
`sigstore-fulcio-certificate-identity-boundary.yaml`,
`sigstore-rekor-backend-boundary.yaml`,
`sigstore-rfc3161-tsa-token-boundary.yaml`,
`sigstore-timestamp-authority-boundary.yaml`,
`sigstore-trusted-root-policy-boundary.yaml`,
`thin-cli-validation-surface-boundary.yaml`,
`tuf-trusted-root-freshness-boundary.yaml`,
`typed-cross-file-reference-validation-boundary.yaml`,
`validator-library-migration-boundary.yaml`.
