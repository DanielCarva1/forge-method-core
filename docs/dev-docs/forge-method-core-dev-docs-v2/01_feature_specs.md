# Forge Method Core v2 - feature specs

Data: 2026-06-28

Este arquivo detalha cada feature recomendada. Use como base para epics, issues e acceptance criteria.


## F01 - forge preview

Prioridade: P0  
Usuarios: todos  
Evidencias: P21,P22,P28,O03,C06  
Crates principais: forge-core-runtime, forge-core-store, forge-core-cli

Demanda: Medo de mutacao errada, necessidade de entender impacto antes da acao.

Produto: Comando e API que mostram plano, arquivos, comandos, authority, gates, efeitos e rollback antes de aplicar.

Criterios de aceite:

- Dado um OperationContract mutavel, preview retorna JSON deterministico com status, touched_refs, risk, gates, rollback_available e next_human_action.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F02 - forge ready

Prioridade: P0  
Usuarios: usuario comum, QA, dev, empresa  
Evidencias: P22,P23,P30,C06  
Crates principais: forge-core-runtime, forge-core-validate, forge-core-cli

Demanda: Confianca operacional e validacao antes de declarar pronto.

Produto: Gate unificado para tests, lint, typecheck, evals, security checks e readiness report.

Criterios de aceite:

- Um run so passa se todos os gates obrigatorios passarem; falhas retornam reasons tipadas e evidencias.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F03 - TraceEvent canonico e forge explain

Prioridade: P0  
Usuarios: todos  
Evidencias: P04,P07,P17,P24,P26,C06  
Crates principais: novo forge-core-trace, runtime, cli

Demanda: Saber o que aconteceu, por que aconteceu e como auditar.

Produto: Trace NDJSON machine-readable e explicacao humana por run.

Criterios de aceite:

- Toda operacao gera trace_id, node_id, actor_agent_id, principal_id, input_refs, output_refs, decision_reason e cost.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F04 - WorkflowGraph v0

Prioridade: P0  
Usuarios: power user, empresa  
Evidencias: P04,P05,P06,C03,C06  
Crates principais: novo forge-core-graph, runtime

Demanda: Orquestrar sem routing solto por prompt.

Produto: Grafo declarativo com nodes, edges, budgets, verifier nodes e replan boundaries.

Criterios de aceite:

- forge graph validate e forge graph run --dry-run funcionam sem executar efeitos; executor respeita dependencias e stop conditions.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F05 - Eval Compare single-agent baseline

Prioridade: P1  
Usuarios: power user, pesquisa, empresa  
Evidencias: P01,P02,P03,P07  
Crates principais: novo forge-core-eval

Demanda: Provar quando multi-agent vale o custo.

Produto: Harness para comparar single-agent anchor, graph workflow e MAS sob mesmo loader, tools, output contract e usage accounting.

Criterios de aceite:

- Relatorio mostra accuracy, cost, latency, trajectory length, failures e delta contra baseline.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F06 - Memory Policy

Prioridade: P1  
Usuarios: todos  
Evidencias: P09,P10,P11,P28,O03  
Crates principais: novo forge-core-memory

Demanda: Personalizacao sem memoria opaca ou perigosa.

Produto: Memory admission, retention, forget, promote, raw evidence e authority boundary.

Criterios de aceite:

- Nenhuma memoria vira authority automaticamente; promote exige policy e evidencia raw.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F07 - Multi-principal governance

Prioridade: P1  
Usuarios: times, empresas, open source  
Evidencias: P08,P24,P25,P26,C01  
Crates principais: contracts, validate, runtime, store

Demanda: Varios agentes e pessoas no mesmo estado sem overwrites silenciosos.

Produto: PrincipalId, IntentContract, ConflictContract, GovernancePolicy e arbitration ledger.

Criterios de aceite:

- Conflito entre principals vira objeto estruturado, nao merge manual silencioso.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F08 - Secure MCP adapter

Prioridade: P1  
Usuarios: power user, empresas  
Evidencias: O01,P17,P18,P19,P20,O03  
Crates principais: novo forge-core-protocol-mcp

Demanda: Conectar ferramentas reais com seguranca.

Produto: MCP server para preview, ready, graph, trace, memory e effect application com allowlist e attestation.

Criterios de aceite:

- Nenhuma tool MCP muta estado sem OperationContract e authority validada.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F09 - Secure A2A adapter

Prioridade: P2  
Usuarios: power user, empresas  
Evidencias: O02,P08,P17,P19,P20  
Crates principais: novo forge-core-protocol-a2a

Demanda: Interoperabilidade entre agentes de vendors diferentes.

Produto: A2A agent card e task surface para delegacao controlada.

Criterios de aceite:

- A2A nao substitui MCP nem vira subagent protocol interno; tarefa externa sempre tem PrincipalId e delegation chain.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F10 - Control Plane local

Prioridade: P2  
Usuarios: power user, QA, times  
Evidencias: P06,P07,P21,P28,O03,C01  
Crates principais: novo forge-core-ui ou cli

Demanda: Ver lanes, claims, traces, gates e risco em uma tela.

Produto: TUI ou HTML estatico lendo .forge-method sem SaaS obrigatorio.

Criterios de aceite:

- Mostra run status, active claims, stale claims, conflicts, gates, cost e next action.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F11 - Risk Audit Gate para codigo de IA

Prioridade: P1  
Usuarios: QA, dev, empresas  
Evidencias: P22,P23,P30  
Crates principais: validate, runtime, cli

Demanda: Detectar fail-soft, exception swallowing, security slop e teste falso.

Produto: Gate com checks deterministios e extensao para SAST/linters.

Criterios de aceite:

- Risk gate falha fechado em padroes proibidos e gera report com evidencia por arquivo.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F12 - Guided Start e Product UX

Prioridade: P2  
Usuarios: usuario comum, founder, dev iniciante  
Evidencias: P28,P29,O03  
Crates principais: cli, docs, templates

Demanda: Entrar no produto sem entender agentes, YAML ou protocolo.

Produto: Fluxo guiado com escolha de objetivo, risco, scaffold e primeiro preview.

Criterios de aceite:

- Usuario cria projeto Forge, ve spec minima, preview e ready sem editar YAML manualmente.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F13 - Budget and Cost Accounting

Prioridade: P2  
Usuarios: power user, empresas  
Evidencias: P01,P02,P16,O03  
Crates principais: trace, eval, runtime

Demanda: Controlar custo, rounds, model calls e tool calls.

Produto: Budget por run, graph node, agent, principal e tool.

Criterios de aceite:

- Run bloqueia ou pede confirmacao quando budget threshold e atingido.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F14 - Knowledge Orchestration mode

Prioridade: P3  
Usuarios: pesquisa, produto, analistas  
Evidencias: P13,P14,P15  
Crates principais: memory, trace, eval

Demanda: Research agents precisam de fontes, claims e evidencias, nao so resumo.

Produto: Modo research com evidence graph, source ledger e citation checks.

Criterios de aceite:

- Cada claim importante aponta para source_id e evidencia local ou web registrada.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.

## F15 - Rust ergonomics and codegen track

Prioridade: P0  
Usuarios: maintainers e agentes de codigo  
Evidencias: O04,O05,O06,O07,P31,C04,C05  
Crates principais: todos

Demanda: Reduzir sofrimento do agente escrevendo Rust manual repetitivo.

Produto: clap derive, thiserror, tracing, builders, fixtures, module split, codegen de contratos e snapshots.

Criterios de aceite:

- Novo comando ou contrato nao exige editar mais de dois pontos manuais fora de tests e docs.
- Deve produzir output JSON para uso por agentes e output humano para CLI.
- Deve registrar trace_id quando participar de run mutavel ou avaliavel.
- Deve falhar fechado quando faltar autoridade, input obrigatorio, evidencia ou gate.

Riscos:

- Criar UX bonita sem authority boundary real.
- Aumentar boilerplate Rust sem codegen ou builders.
- Permitir que adapter externo reinterprete o estado do Forge.

Implementacao minima:

1. Definir contrato YAML ou struct Rust com schema.
2. Criar fixture valida e fixture invalida.
3. Criar validator com diagnostics tipados.
4. Expor CLI ou API interna.
5. Adicionar snapshot de output.
6. Registrar evento no trace quando aplicavel.
