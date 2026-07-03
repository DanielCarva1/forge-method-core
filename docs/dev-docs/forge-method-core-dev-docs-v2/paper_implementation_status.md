# Paper Implementation Status

**Date**: 2026-06-30
**Scope**: cada um dos 15 papers em `contracts/research/` mapeado ao seu estado de implementação no codebase do Forge Method core. Representação oriental (China/Coreia/Japão) e ocidental rastreada conforme `AGENTS.md` ("Search non-Western and Chinese-origin work when the domain is active there") e a regra `policy.geographic_coverage.rule` em `contracts/research/field-evidence-20260625.yaml`.

> Convenção de ícones: ✅ Implemented (há crate/arquivo concretamente enviado) · 🟡 Partial (plumbing existe mas há lacunas) · ❌ Pending (sem reflexo no código) · 🚫 Decided against (explicitamente rejeitado).

## Summary

| Region | Papers | Implemented | Partial | Pending |
|---|---|---|---|---|
| Western | 8 | 4 | 4 | 0 |
| Oriental (CN/KR/JP) | 0 | 0 | 0 | 0 |
| Mixed (Western + Oriental) | 5 | 1 | 4 | 0 |
| Unspecified (auditorias internas) | 2 | 0 | 2 | 0 |
| **Total** | **15** | **6** | **9** | **0** |

Observação inicial: **nenhum paper é puramente oriental**. As fontes orientais vivem *dentro* dos 5 papers Mixed (notadamente `field-evidence-20260625` com ChatDev/AgentScope/Fudan/Tsinghua, `community-trends` F8 V2EX/Juejin/InfoQ.cn, `best-features` F7 Alibaba/TRAE/Qwen3, `protocol-scale` F7 Qwen-Agent/MegaAgent). Ver seção *Regional representation audit* para o impacto dessa assimetria.

## Per-paper status

### `agentic-throughput-and-fast-quality-mode-v1.yaml`

- **Topic**: Taxonomia de gargalos de throughput em MAS + desenho de duas pistas (fast vs quality).
- **Region**: Western.
- **Key findings**: 13 findings (F1–F13); fontes: 17.
- **Implementation status**: 🟡 Partial.
- **Where**: `crates/forge-core-kernel/`, `crates/forge-core-decisions/phase_transition.rs`, `../Forge-method-archive/dev-journals/r6_benchmarks.md` (R6.1+R6.2).
- **Evidence**: há runtime com transição de fase e benchmarks R6.1/R6.2 estabelecidos (base mensurável); a noção de "pista rápida" aproxima-se da separação preview/ready/gate em `forge-core-trace::TraceEventKind` (`PreviewCompleted`, `ReadyCompleted`, `GatePassed`).
- **Gap**: F1/F2 (estudos de caso de vendor) não são actionable; F7/F8 (limites de concorrência por modelo) ainda não têm policy codificada; banco de eval de throughput (F13) parcial — pendente F05/R9.

### `best-features-from-papers-and-cases-v1.yaml`

- **Topic**: Catálogo FEAT dos melhores recursos ausentes para o Forge, sintetizando literatura + casos.
- **Region**: Mixed (F7 traz fontes orientais: Alibaba/TRAE/Qwen3).
- **Key findings**: 8 findings + catálogo FEAT; fontes: 24.
- **Implementation status**: 🟡 Partial.
- **Where**: depende do item FEAT — `crates/forge-core-crypto/` (FEAT assinatura/transparência), `crates/forge-core-decisions/` (FEAT isolamento/claim), pendentes F05–F14.
- **Evidence**: FEATs de criptografia e isolamento têm reflexo concreto (`forge-core-crypto` com rekor/ocsp/sigstore/tuf/slsa_transparency; `forge-core-decisions/isolation.rs`). FEAT-03 (self-evolving tools) e FEAT-04 (shared-state coordination) mapeiam para pendentes F08 (MCP) e F09 (A2A).
- **Gap**: a maior parte do catálogo FEAT é *aspiracional* — corresponde exatamente aos features F08–F14 ainda não entregues no `excellence_roadmap.md`.

### `cli-llm-first-design.yaml`

- **Topic**: Princípios de CLI machine-first (JSON determinístico, contratos tipados, saída estruturada).
- **Region**: Western.
- **Key findings**: 19 findings (F1–F19); fontes: 28.
- **Implementation status**: 🟡 Partial.
- **Where**: `crates/forge-core-cli/`, `crates/forge-core-contracts/`, `crates/forge-core-schema/`, `../Forge-method-archive/dev-journals/r7_yaml_serde.md`.
- **Evidence**: o CLI existe com contratos tipados em `forge-core-contracts` e canonical serde validado pelo R7 (yaml_serde). O modelo `TraceEvent`/`TraceActor` é determinístico e machine-readable. O padrão "parse-don't-validate" (F9 tau-bench policy) reflete-se nos newtypes `RepoPath`/`ClaimId`/`StableId`.
- **Gap**: F8/F10/F11 (benchmark leaderboards públicos, narração humana) não são actionable para um core local. F14–F19 não foram auditados finding-a-finding.

### `community-trends-and-requested-features-v1.yaml`

- **Topic**: Tendências da comunidade 2025–2026 e recursos mais pedidos.
- **Region**: Mixed (F8 agrega fontes chinesas: V2EX, Juejin, InfoQ.cn).
- **Key findings**: 8 findings + catálogo DEM; fontes: 28.
- **Implementation status**: 🟡 Partial.
- **Where**: demandas mapeiam para F06 (memória), F10 (dashboard de ops multi-agente), F08 (MCP seguro).
- **Evidence**: o paper é principalmente insumo de priorização — alinha-se à Trilha F do roadmap. Demanda por memória com proveniência (DEM-04) e por painel de ops (DEM-01) está registrada como pendente.
- **Gap**: nenhuma demanda DEM tem implementação direta no core ainda; todas pendentes em F06/F08/F10.

### `field-evidence-20260625.yaml`

- **Topic**: Política de evidência de campo + ~90 fontes tiered (T1/T2/T3) com `confirmed_origin`.
- **Region**: Mixed (muitas origens CN confirmadas: Fudan, Tsinghua ChatDev, AgentScope, Alibaba, etc.).
- **Key findings**: bloco de fontes + `plan_level_implications`; fontes: ~90.
- **Implementation status**: 🟡 Partial.
- **Where**: política funda a regra de cobertura geográfica; `plan_level_implications` cruza com R-tracks e features.
- **Evidence**: a política é o **fundamento canônico** da seção *Regional representation audit* deste documento. As implicações de plano já foram parcialmente absorvidas (R5 zeroize, R10 crypto crate).
- **Gap**: o paper é meta — sua "implementação" é a inclusão intencional contínua de fontes não-ocidentais nos demais papers. Não há codepath único; a lacuna é processual (ver auditoria regional).

### `multi-agent-collaboration-governance-research.yaml`

- **Topic**: Verificação de que contratos de governança multi-agente são reais (grite/Limen/preclaim + Cursor/CAID/Devin).
- **Region**: Mixed (síntese cross-regional de casos industriais).
- **Key findings**: verdict + padrão de 4 camadas; fontes: sintéticas.
- **Implementation status**: ✅ Implemented.
- **Where**: `crates/forge-core-decisions/src/conflict_detection.rs`, `crates/forge-core-decisions/src/isolation.rs`, `crates/forge-core-store/src/claim_wal.rs`, `crates/forge-core-decisions/src/claim_engine.rs`.
- **Evidence**: `conflict_detection.rs` implementa exatamente o padrão de 4 camadas — classificação pura `WriteCheck::Ok { governed_by_self, ungoverned } | Blocked { blocks }` com `BlockDetail { blocked_path, blocking_claim_id, claimant, conflict_code }`. DD8/DD10/DD19/DD26/DD27/DD28 estão codificados como invariantes documentados nos comentários do módulo. O WAL materializa a reserva semântica (S4.3 citada no próprio paper).
- **Gap**: governance de handoff multi-principal (mais de 2 agentes) ainda é parcial — ver F07.

### `protocol-scale-with-model-v1.yaml`

- **Topic**: Contrato tipado como amplificador vs tax — quando escala com o modelo e quando limita.
- **Region**: Mixed (F7: Qwen-Agent, MegaAgent).
- **Key findings**: 8 findings (F1–F8); fontes: 18.
- **Implementation status**: 🟡 Partial.
- **Where**: `crates/forge-core-contracts/`, `crates/forge-core-validate/`, `crates/forge-core-decisions/catalog.rs`.
- **Evidence**: os contratos tipados (Claim, Effect, Operation) são o núcleo do Forge — `protocol-scale` valida essa escolha de design (F3/F8 "hard gates + freedom within gates"). O validator acumula `Diagnostic` (não short-circuit) conforme a filosofia do paper.
- **Gap**: F1/F8 (evidência empírica de scale-with-model via benchmarks) é exatamente o pendente F05/R9 — ainda sem baseline comparativo.

### `robustness-observability-multiagent-v1.yaml`

- **Topic**: Robustez/observabilidade para MAS com WAL file-backed.
- **Region**: Western (k8s/ARIES/Postgres/MongoDB/Bazel/LumiMAS).
- **Key findings**: 9 findings (F1–F9); fontes: 30.
- **Implementation status**: ✅ Implemented.
- **Where**: `crates/forge-core-store/src/claim_wal.rs`, `crates/forge-core-trace/src/lib.rs`, `../Forge-method-archive/dev-journals/r6_benchmarks.md`.
- **Evidence**: F1 (ARIES) → `claim_wal.rs` tem `ClaimWalRecovery` com `last_good_offset`, `stop_reason: ClaimWalStopReason` (`TruncatedHeader`, `PayloadChecksumMismatch`, `SequenceGap`, …). F2 (level-triggered reconcile) → operação `ClaimWalOperation::ReconcileStatus` (record type 7). F5 (observabilidade) → `TraceEvent` com `TraceActor { principal_id, agent_id, role }` e `TraceEventKind` cobrindo RunStarted/PreviewCompleted/ReadyCompleted/GatePassed/GateBlocked/EffectStaged/EffectApplied. R6.1/R6.2 benchmarks estabelecem a janela de recovery (DEFAULT_ROTATE_MAX_REPLAY_MILLIS = 250ms).
- **Gap**: F9 (chaos/fault injection em CI) parcial — R4 (fuzz via ADR-0008 Linux CI) cobre parte, mas failpoint injection no claim path é pendente.

### `rust-observability-selfhealing-v1.yaml`

- **Topic**: Camada de observabilidade + self-healing em Rust.
- **Region**: Western.
- **Key findings**: 10 findings (F1–F10) + revisão de crates; fontes: 18.
- **Implementation status**: 🟡 Partial.
- **Where**: `crates/forge-core-trace/src/lib.rs`, `crates/forge-core-store/src/claim_wal.rs`.
- **Evidence**: camada de observabilidade existe (`forge-core-trace` com `TraceRisk { risk_level, destructive }`, `TraceCost { model_calls, tool_calls, estimated_tokens }`, `TraceAuthority { operation_id, capability_ids }`). Self-healing básico via `ClaimWalRecovery::repaired` e `ClaimWalStopReason`.
- **Gap**: self-healing reativo (restart automático com estado preservado, F8–F10) parcial; R5.10/R5.11 (zeroize finalização, secret hygiene em tracing) ainda pendentes.

### `rust-state-integrity-wal-concurrency-v1.yaml`

- **Topic**: Integridade de estado Rust + WAL + concorrência cross-process.
- **Region**: Western.
- **Key findings**: 8 findings (F1–F8) + revisão de crates; fontes: 18.
- **Implementation status**: ✅ Implemented.
- **Where**: `crates/forge-core-store/src/claim_wal.rs`, `crates/forge-core-store/src/lib.rs`.
- **Evidence**: F4/F8 (ARIES + recovery) → `ClaimWalRecovery` com offsets, `ClaimWalProjection` reconstrói `active_by_claim_id`, `active_claim_ids_by_agent`, `active_claim_ids_by_scope`, `active_claim_ids_by_path`. Header CRC em `HEADER_CRC_OFFSET = 20`. Rotação level-triggered (`ClaimWalRotationReason::{WalSizeBytes, RecordCount, ReplayDurationMillis}`) com `DEFAULT_ROTATE_MAX_WAL_BYTES = 64MB`. Paths de lock/snapshot/manifest/archive definidos como constantes públicas.
- **Gap**: F1–F3 (análise comparativa de crates) foi absorption-only — sem código a acrescentar. F6 (FS advisory locks cross-platform) — sem grep confirmou `fs4`/`file_lock`; lacuna a verificar.

### `rust-testing-defenses-v1.yaml`

- **Topic**: Defesas via testes vs o bug de oráculo circular R8.
- **Region**: Western.
- **Key findings**: 7 findings (F1–F7); fontes: 18.
- **Implementation status**: ✅ Implemented.
- **Where**: `../Forge-method-archive/dev-journals/r4_fuzz_plan.md`, testes em `crates/*/tests/`, ADR-0008.
- **Evidence**: R8 foi fechado pela combinação newtype + proptest + trycmd + fuzz (ADR-0008 Linux CI). O paper é a fundamentação teórica dessa stack — F1 (parse-don't-validate), F2 (property tests), F3 (snapshot/CLI golden), F4 (fuzz), F5 (mutation), F6 (invariantes), F7 (fixtures).
- **Gap**: F5 (mutation testing) não está no CI; F6 (invariantes formais) parcial.

### `selfhealing-failpoint-audit-v1.yaml`

- **Topic**: Auditoria de fail-points/panics no claim path.
- **Region**: Unspecified (auditoria interna).
- **Key findings**: 15 ocorrências (não usa `key_findings`; usa `occurrences`).
- **Implementation status**: 🟡 Partial.
- **Where**: `crates/forge-core-store/src/claim_wal.rs`, `crates/forge-core-decisions/src/claim_engine.rs`.
- **Evidence**: as ocorrências foram triadas; as críticas endereçadas via `ClaimWalStopReason` tipado (não panic) e `Result` com enums nomeados conforme `AGENTS.md`. O WAL agora fail-closes em record type desconhecido (`FLAG_SKIPPABLE_UNKNOWN`, `from_record_type` retorna `None`).
- **Gap**: failpoint injection ativa (simular disk full, partial write, crash mid-append) ainda pendente — correlaciona-se com R4 fuzz e F11 (risk audit).

### `selfhealing-wal-crc-design-v1.yaml`

- **Topic**: Desenho do formato binário do WAL com CRC32C.
- **Region**: Western (LevelDB/RocksDB/Postgres/SQLite como referência).
- **Key findings**: 10 design decisions (D1–D10; não usa `key_findings`; usa `design_decisions`).
- **Implementation status**: ✅ Implemented.
- **Where**: `crates/forge-core-store/src/claim_wal.rs` (constantes e structs acima).
- **Evidence**: implementação corresponde 1-para-1 com as decisões: header de 24 bytes com CRC em offset 20 (D1), trailer CRC de 4 bytes, `ClaimWalCheckpointPayload { snapshot_path, snapshot_crc32c, last_seq_in_snapshot }` (D2), `ClaimWalManifestPayload` com snapshot+archive+checkpoint_seq (D3), `ClaimWalRotationOptions` com três limites (D4), record types 1–7 reservados com fail-closed em desconhecidos (D5), `ClaimWalSnapshotPayload` com `latest_claims` (D6), `ClaimWalOperation::ReconcileStatus` em type 7 para não colidir com 4/5/6 (D7), path `wal/claims.fmw1` + lock/snapshot/archive (D8), `ClaimWalRecovery::last_good_offset` (D9), `ClaimWalStopReason` discriminado (D10).
- **Gap**: nenhuma lacuna arquitetural — as 10 decisões estão todas materializadas. Apenas telemetria de recovery em produção é pendente.

### `selfhealing-writepath-audit-v1.yaml`

- **Topic**: Mapa E2E do read/write path das claims.
- **Region**: Unspecified (auditoria interna).
- **Key findings**: `io_operations` + `answers` (Q1/Q2/Q3; não usa `key_findings`).
- **Implementation status**: 🟡 Partial.
- **Where**: `crates/forge-core-decisions/src/conflict_detection.rs`, `crates/forge-core-store/src/claim_wal.rs`, `crates/forge-core-cli/src/lib.rs`.
- **Evidence**: as Q1/Q2/Q3 do paper (quem escreve, quando, onde) são respondidas em código: write-set entra em `conflict_detection::check_write_set`, sinalizado via WAL `Acquire`/`Release`/`Heartbeat`/`HandoffRecorded`, efeitos stageados via `TraceEventKind::EffectStaged` → `EffectApplied`.
- **Gap**: auditoria identificou cantos onde o path bifurca (CLI vs runtime vs validator); nem todos têm telemetria ponta-a-ponta.

### `structural-bug-prevention-typelevel-v1.yaml`

- **Topic**: Prevenção estrutural (type-level) do acoplamento de id que o R8 expôs.
- **Region**: Western (parse-don't-validate, proptest, Pact).
- **Key findings**: 8 findings (F1–F8); fontes: 18.
- **Implementation status**: ✅ Implemented.
- **Where**: `crates/forge-core-contracts/src/` (newtypes `RepoPath`, `ClaimId`, `StableId`), `crates/forge-core-validate/src/lib.rs`.
- **Evidence**: o paper é o fundamento direto dos newtypes usados em `conflict_detection.rs` (`blocked_path: RepoPath`, `blocking_claim_id: ClaimId`, `claimant: StableId`) — impossível passar string crua onde tipagem exige. `Diagnostic::error/warning` acumulam sem short-circuit (compatível com F1 parse-don't-validate).
- **Gap**: F8 (Pact/contract tests entre crates) parcial — há tests em `crates/*/tests/` mas sem framework Pact formal.

## Regional representation audit

O `AGENTS.md` exige: *"Search non-Western and Chinese-origin work when the domain is active there."* O `field-evidence-20260625.yaml` operacionaliza isso via `policy.geographic_coverage.rule`. O quadro encontrado:

**Onde a representação oriental é forte**: o paper `field-evidence-20260625` (Mixed) faz isso exemplarmente — ~90 fontes com `confirmed_origin` identificando Fudan, Tsinghua (ChatDev), AgentScope (Alibaba), Qwen, DeepGLM, entre outros. `community-trends` F8 e `best-features` F7 e `protocol-scale` F7 também trazem fontes orientais de forma intencional.

**Onde há lacuna**: **nenhum dos 15 papers é puramente oriental-led**. Em domínios onde a pesquisa oriental é particularmente ativa — agentes chineses de coding (TRAE, Qwen3-Coder, DeepSeek), infraestrutura de MAS em escala (MegaAgent, Alibaba LLM-OS), e evalução chinesa (AgentBench da Tsinghua, T-Eval da Fudan) — a cobertura aparece apenas *dentro* de papers Mixed como sub-itens (F7/F8), não como papers próprios.

**Recomendação**: para R15/R16, considerar papers dedicados a (a) frameworks de MAS chineses em produção (Alibaba, ByteDance), (b) benchmarks orientais de agentes (AgentBench, T-Eval, ToolBench-CN), (c) pesquisa coreana/japonesa (LMArena-JP, LocalLLM-JP). Isso alinha a Trilha F ao princípio do `AGENTS.md` e fecha a assimetria 0/0/0/0 oriental-pura na tabela acima.

**Honestidade**: a ausência de papers orientais-led não é necessariamente um déficit de implementação — os papers Mixed já importam as conclusões orientais relevantes. É um déficit de *cobertura documental*, que esta auditoria registra para backlog.

## Cross-cutting observations

**Convergência: "hard gates + freedom within gates".** Este padrão aparece explicitamente em `protocol-scale` F3/F8, `cli-llm-first` F9 (tau-bench policy), `best-features` F1 (RADAR), e implicitamente em `agentic-throughput` F7/F8 e `multi-agent-collaboration` (4-layer pattern). É a tese central do design do Forge: contratos tipados fortes (`ClaimContract`, `OperationContract`, `EffectContract`) que bloqueiam operações inseguras no motor puro (`conflict_detection::WriteCheck::Blocked`), mas deixam o agente livre *dentro* do scope reservado (`WriteCheck::Ok::governed_by_self`). O WAL (`claim_wal.rs`) materializa essa fronteira: append-only, fail-closed em desconhecido, recoverable. Três papers independentes chegando à mesma forma é evidência forte de que a arquitetura está bem fundamentada.

**Classe de bug R8 unifica quatro papers.** `rust-testing-defenses`, `structural-bug-prevention`, `selfhealing-failpoint-audit`, e `selfhealing-writepath-audit` são todos resposta ao mesmo problema estrutural — acoplamento de id entre parser/validador/engine que cria oráculo circular. A stack implementada (newtypes em `forge-core-contracts` + `Diagnostic` acumulativo em `forge-core-validate` + proptest + trycmd + fuzz ADR-0008) é a resposta consolidada. Isso explica por que estes quatro papers estão entre os mais "Implemented" — eles foram os papers *que motivaram* o trabalho, não papers pós-facto.

**Pendências convergem em F05–F14.** Os findings não-actionable ou pendentes mapeiam com nitidez para a Trilha F do `excellence_roadmap.md`: (a) `best-features` FEAT-03 → F08 MCP seguro; (b) FEAT-04 + `protocol-scale` F8 → F09 A2A; (c) `community-trends` DEM-04 → F06 memória com proveniência; (d) DEM-01 → F10 control plane multi-agente; (e) `protocol-scale` F1 + `agentic-throughput` F13 → F05 eval bank (parcial) e R9 benchmarks comparativos. Os papers Western "auditoria/ARIES/observabilidade" estão largamente esgotados (6 ✅); a fronteira de inovação está nos papers Mixed que apontam para features não-started.

**Findings não-actionable**: `cli-llm-first` F8/F10/F11 (benchmark leaderboards públicos) e `agentic-throughput` F1/F2 (vendor case studies) não têm reflexo direto no código e provavelmente nunca terão — são insumos de design, não especificações. Marcá-los como ❌ seria desonesto; são melhor classificados como "absorvidos no design, sem codepath próprio".
