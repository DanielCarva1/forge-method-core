# Forge Method Core v2 - documento de desenvolvimento auditado

Data: 2026-06-28  
Janela cientifica para papers: 2025-10-28 a 2026-06-28

## 1. Objetivo

Este documento converte a revisao cientifica recente, os sinais de mercado/comunidade, as fontes oficiais de protocolo e a leitura da codebase atual em um plano de desenvolvimento para o Forge Method Core.

A decisao principal e simples: o Forge deve virar uma camada deterministica de coordenacao para trabalho com agentes. O produto deve expor poder de agente sem abrir mao de preview, verificacao, trace, rollback, memoria governada, seguranca de protocolo e controle de conflito.

## 2. Regras de auditoria

Papers cientificos entram como base principal apenas se estiverem dentro da janela de oito meses: 2025-10-28 a 2026-06-28. Fontes oficiais atuais entram como estado de ecossistema, nao como prova empirica. Rust docs entram como engenharia estavel. Sinais de comunidade/cases entram como demanda, nao como prova de eficacia.

Niveis de forca usados no ledger:

- A: evidencia principal para decisao de produto ou arquitetura.
- B: evidencia complementar ou fonte oficial atual.
- C: contexto util, mas nao suficiente para decisao central.

## 3. Leitura atual do campo

### 3.1 Multi-agent homogeneo nao deve ser default

OneFlow, BenchAgent e o estudo de incerteza em MAS convergem em uma conclusao: varios agentes com o mesmo modelo base, mudando so prompt, role ou posicao, nao vencem automaticamente um single-agent bem controlado. O Forge deve tratar single-agent como anchor obrigatorio e exigir evidencia para ativar MAS. Isso vira feature de produto em `forge eval compare`.

Decisao: toda feature multi-agent precisa informar qual vantagem esta buscando: heterogeneidade de modelos, ferramentas diferentes, permissoes diferentes, principals diferentes, multimodalidade, isolamento de contexto ou paralelismo real.

### 3.2 Orquestracao por grafo virou a forma mais defensavel

O survey de workflow optimization separa template, realized graph e execution trace. GraphBit e VMAO reforcam DAG explicito, execucao deterministica, verificador e replanejamento. Isso encaixa diretamente no Forge porque o `OperationContract` atual ja modela autoridade, execucao, gates e efeitos, mas precisa virar node dentro de um grafo maior.

Decisao: criar `WorkflowGraph` como entidade de primeira classe e manter `OperationContract` como um tipo de node ou contrato de operacao. Prompt nao decide routing estrutural sozinho.

### 3.3 Trace e eval precisam ser produto

AgentBeats mostra que avaliacao de agentes esta indo para interfaces padronizadas, judge agents e reproducibilidade via protocolos. O Forge ja tem evidence logs e reasons tipados, mas precisa formalizar `TraceEvent` canonico e um harness de eval.

Decisao: nenhum run sem trace. Nenhuma arquitetura nova sem comparacao contra baseline.

### 3.4 Memoria precisa de policy, nao so retrieval

MemFlow, MemRouter e Experience Compression Spectrum mostram que memoria boa depende de write policy, read routing, nivel de compressao, budget e validacao de suporte. Memoria nao deve criar autoridade automaticamente.

Decisao: criar `MemoryPolicy`, `MemoryRecord`, `MemoryPromotion` e `MemoryAuthorityBoundary` antes de qualquer memoria persistente rica.

### 3.5 Protocolo precisa de identidade e capability binding

MCP e A2A sao importantes para ecossistema. Mas os papers de security threat modeling, MCP security, AIP e AgentRFC mostram riscos de attestation ausente, trust propagation, composicao insegura, delegacao sem identidade e gaps de conformance.

Decisao: adapters MCP/A2A do Forge sao projections do kernel, nao fontes de verdade. Toda chamada mutavel precisa ser vinculada a PrincipalId, capability, scope, OperationContract e trace.

### 3.6 Demanda de comunidade esta em controle, integracao e governanca

O paper de Stack Overflow sobre agentes mostra dores em runtime integration, dependency management, orchestration complexity e evaluation reliability. A documentacao atual de plataformas agentic mostra demanda por custom agents, hooks, skills, sandboxes, memory, budgets, MCP, rollback e audit logs. Papers de usuarios comuns mostram que transparencia de seguranca e privacidade precisa ser acionavel e on-demand.

Decisao: Forge deve vender `build safely with agents`, nao `mais agentes conversando`.

## 4. Evidencia da codebase atual

| ID | Local | Achado |
|---|---|---|
| C01 | `README.md:52-70` | Forge v2 ja define .forge-method como camada de coordenacao para humanos e agentes, com registry, lane claims, handoffs append-only e optimistic concurrency. |
| C02 | `Cargo.toml:3-12` | Workspace Rust separado em forge-contract-validator, forge-core-contracts, forge-core-schema, forge-core-cli, forge-core-store, forge-core-validate e forge-core-kernel. |
| C03 | `crates/forge-core-contracts/src/operation.rs:13-44` | OperationContract modela autonomia, autoridade, coordenação, execucao, stop policy, comandos, efeitos, gates e diagnostics. |
| C04 | `crates/forge-core-store/src/lib.rs:21-34,127-177,299-324` | Store concentra indexacao, coleta de YAML, append JSONL, efeito transacional, WAL, lock e metadata. |
| C05 | `crates/forge-core-cli/src/main.rs:47-93` | CLI atual usa parsing manual por env::args, match command, loops de indice e process::exit. |
| C06 | `crates/forge-core-kernel/src/lib.rs:43-98,244-365` | RuntimePlan ja calcula status e reasons tipados com validacao, gates, human input, review e mutation policy. |

## 5. Feature map prioritario

| ID | Prioridade | Feature | Usuarios | Evidencias |
|---|---|---|---|---|
| F01 | P0 | forge preview | todos | P21,P22,P28,O03,C06 |
| F02 | P0 | forge ready | usuario comum, QA, dev, empresa | P22,P23,P30,C06 |
| F03 | P0 | TraceEvent canonico e forge explain | todos | P04,P07,P17,P24,P26,C06 |
| F04 | P0 | WorkflowGraph v0 | power user, empresa | P04,P05,P06,C03,C06 |
| F05 | P1 | Eval Compare single-agent baseline | power user, pesquisa, empresa | P01,P02,P03,P07 |
| F06 | P1 | Memory Policy | todos | P09,P10,P11,P28,O03 |
| F07 | P1 | Multi-principal governance | times, empresas, open source | P08,P24,P25,P26,C01 |
| F08 | P1 | Secure MCP adapter | power user, empresas | O01,P17,P18,P19,P20,O03 |
| F09 | P2 | Secure A2A adapter | power user, empresas | O02,P08,P17,P19,P20 |
| F10 | P2 | Control Plane local | power user, QA, times | P06,P07,P21,P28,O03,C01 |
| F11 | P1 | Risk Audit Gate para codigo de IA | QA, dev, empresas | P22,P23,P30 |
| F12 | P2 | Guided Start e Product UX | usuario comum, founder, dev iniciante | P28,P29,O03 |
| F13 | P2 | Budget and Cost Accounting | power user, empresas | P01,P02,P16,O03 |
| F14 | P3 | Knowledge Orchestration mode | pesquisa, produto, analistas | P13,P14,P15 |
| F15 | P0 | Rust ergonomics and codegen track | maintainers e agentes de codigo | O04,O05,O06,O07,P31,C04,C05 |

## 6. Produto final esperado

### Para usuario comum

O Forge deve parecer um modo seguro de construir com IA. A pessoa nao precisa saber o que e MCP, A2A, WAL ou WorkflowGraph. A experiencia precisa ser:

1. Comecar guiado.
2. Entender o plano.
3. Ver preview antes de mutacao.
4. Rodar verificacao.
5. Receber explicacao curta.
6. Desfazer quando algo der errado.

Features traduzidas: `forge start --guided`, `forge preview`, `forge ready`, `forge explain`, `forge undo`.

### Para vibe coder, indie maker e founder

O maior risco e publicar algo que parece funcionar, mas tem auth ruim, dados expostos, teste falso, dependencia insegura ou falha silenciosa. O Forge deve ser o cinto de seguranca.

Features traduzidas: security checklist, secrets gate, deploy gate, data risk gate, risk audit gate, ready gate e rollback.

### Para dev/QA profissional

A dor nao e gerar codigo. A dor e revisar, validar, rastrear, testar e explicar o que o agente fez. Forge deve virar control layer para agentes de codigo.

Features traduzidas: trace, eval, readiness report, AI risk audit, policy-as-code, CI integration, failure taxonomy e evidence ledger.

### Para power user de IA

Esse usuario quer controlar grafo, nodes, tools, budgets, memory, protocols, replay e evals. Ele aceita declaratividade e arquivos.

Features traduzidas: `forge graph`, `forge eval`, `forge memory`, `forge protocol mcp`, `forge protocol a2a`, control plane local.

### Para time/empresa

O valor e governanca de trabalho gerado por IA. O time precisa saber quem fez, com qual permissao, qual evidencia, qual risco e qual rollback.

Features traduzidas: PrincipalId, IntentContract, ConflictContract, GovernancePolicy, audit ledger, allowed capabilities, budgets e approval gates.

## 7. Decisoes de arquitetura

1. Rust fica no kernel deterministico.
2. Semantica viva fica declarativa ate estabilizar.
3. `OperationContract` vira node ou payload dentro de `WorkflowGraph`.
4. Todo run gera `TraceEvent`.
5. Toda feature multi-agent precisa de baseline single-agent.
6. MCP e A2A entram como adapters seguros, nao como autoridade.
7. Memoria precisa de policy e source evidence.
8. Multi-principal governance vira diferencial do Forge.
9. CLI usa argv manual em `main.rs` (sem `clap`, sem derive macros). Cada subcomando novo adiciona um braço no `match` de `main.rs` e uma fn `run_<command>(&[String])`. Ver `04_rust_refactor_guide.md`.
10. Erros e diagnostics devem ser enums tipados feitos à mão (sem `thiserror`, sem `anyhow`), derivando `Debug, Clone, PartialEq, Eq`.

## 8. Fonte ledger resumido

| ID | Data | Forca | Titulo | Area |
|---|---|---|---|---|
| P01 | 2026-01-18 | A | Rethinking the Value of Multi-Agent Workflow: A Strong Single Agent Baseline | multi-agent baseline |
| P02 | 2026-06-04 | A | Do More Agents Help? Controlled and Protocol-Aligned Evaluation of LLM Agent Workflows | multi-agent evaluation |
| P03 | 2026-02-04 | A | On the Uncertainty of Large Language Model-Based Multi-Agent Systems | MAS uncertainty |
| P04 | 2026-03-23 | A | From Static Templates to Dynamic Runtime Graphs: A Survey of Workflow Optimization for LLM Agents | workflow graphs |
| P05 | 2026-03-08 | A | GraphBit: A Graph-based Agentic Framework for Non-Linear Agent Orchestration | deterministic graph orchestration |
| P06 | 2026-03-12 | A | Verified Multi-Agent Orchestration: A Plan-Execute-Verify-Replan Framework for Complex Query Resolution | verify replan |
| P07 | 2026-06-11 | A | AgentBeats: Agentifying Agent Assessment for Openness, Standardization, and Reproducibility | agent evaluation |
| P08 | 2026-04-10 | A | MPAC: A Multi-Principal Agent Coordination Protocol for Interoperable Multi-Agent Collaboration | multi-principal governance |
| P09 | 2026-05-05 | A | MemFlow: Intent-Driven Memory Orchestration for Small Language Model Agents | memory orchestration |
| P10 | 2026-05-01 | A | MemRouter: Memory-as-Embedding Routing for Long-Term Conversational Agents | memory admission |
| P11 | 2026-04-17 | A | Experience Compression Spectrum: Unifying Memory, Skills, and Rules in LLM Agents | experience compression |
| P12 | 2026-02-02 | A | Kimi K2.5: Visual Agentic Intelligence | oriental swarm and multimodal agents |
| P13 | 2026-06-01 | A | K-BrowseComp: A Web Browsing Agent Benchmark Grounded in Korean Contexts | localized agent benchmarks |
| P14 | 2026-06-11 | A | EvoBrowseComp: Benchmarking Search Agents on Evolving Knowledge | fresh benchmarks |
| P15 | 2026-06-11 | A | Agents-K1: Towards Agent-native Knowledge Orchestration | knowledge orchestration |
| P16 | 2026-05-07 | B | Efficient Serving for Dynamic Agent Workflows with Prediction-based KV-Cache Management | serving and cost |
| P17 | 2026-02-11 | A | Security Threat Modeling for Emerging AI-Agent Protocols: A Comparative Analysis of MCP, A2A, Agora, and ANP | protocol security |
| P18 | 2026-01-24 | A | Breaking the Protocol: Security Analysis of the Model Context Protocol Specification and Prompt Injection Vulnerabilities in Tool-Integrated LLM Agents | MCP security |
| P19 | 2026-03-25 | A | AIP: Agent Identity Protocol for Verifiable Delegation Across MCP and A2A | identity and delegation |
| P20 | 2026-03-25 | B | AgentRFC: Security Design Principles and Conformance Testing for Agent Protocols | protocol conformance |
| P21 | 2025-10-29 | A | What Challenges Do Developers Face in AI Agent Systems? An Empirical Study on Stack Overflow | developer demand |
| P22 | 2026-04-19 | A | AIRA: AI-Induced Risk Audit: A Structured Inspection Framework for AI-Generated Code | AI generated code risk |
| P23 | 2026-06-07 | B | Governance Controls for AI-Generated Test Artifacts in Autonomous Software Testing | QA and governance |
| P24 | 2026-01-26 | A | Agentic Much? Adoption of Coding Agents on GitHub | coding agent adoption |
| P25 | 2026-02-09 | A | AIDev: Studying AI Coding Agents on GitHub | coding agent dataset |
| P26 | 2026-01-24 | B | Fingerprinting AI Coding Agents on GitHub | authorship and governance |
| P27 | 2026-02-16 | A | Configuring Agentic AI Coding Tools: An Exploratory Study | agent configuration demand |
| P28 | 2026-04-19 | A | What Security and Privacy Transparency Users Need from Consumer-Facing Generative AI | consumer trust |
| P29 | 2026-01-26 | B | Generative AI in Saudi Arabia: A National Survey of Adoption, Risks, and Public Perceptions | non-western consumer adoption |
| P30 | 2026-04-13 | B | Taking a Pulse on How Generative AI is Reshaping the Software Engineering Research Landscape | software engineering governance |
| P31 | 2026-05-22 | B | MISRust: Mapping MISRA-C++ Coding Guidelines to the Rust Programming Language | Rust safety guidance |
| O01 | current-2026 | B | Model Context Protocol docs: What is MCP? | official protocol |
| O02 | current-2026 | B | A2A Protocol docs | official protocol |
| O03 | current-2026 | B | GitHub Copilot cloud agent docs | market signal |
| O04 | current-2026 | B | clap derive docs | Rust CLI |
| O05 | current-2026 | B | tracing crate docs | Rust observability |
| O06 | current-2026 | B | thiserror crate docs | Rust errors |
| O07 | current-2026 | B | Rust API Guidelines | Rust API |

O ledger completo esta em `data/evidence_ledger.csv`.
