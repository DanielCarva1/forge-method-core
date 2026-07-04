# ADR-0006 - MCP e A2A como adapters seguros

- **Status**: Accepted (2026-07-02; expandido de `proposto`)

## Contexto

O Forge Method core e uma camada de governance sobre shared state agentic: ela
serialize writes por path (lane claims, ADR 0023), resolve conflitos de
principals como objetos estruturados (F07, ADR 0007) e mantem o kernel como
unica fonte de verdade para mutacao. Mas o core so e util se agentes externos
puderem chama-lo — e o ecossistema de agentes em 2025-26 convergiu para dois
protocolos de fato:

- **MCP (Model Context Protocol, Anthropic)** — JSON-RPC sobre stdio; e o
  canal pelo qual clientes como Claude Desktop descobem e invocam tools.
- **A2A (Agent-to-Agent)** — interoperabilidade entre agentes de vendors
  diferentes.

Expor o Forge core por esses protocolos e a frente "Features comunidade"
(fecha a ultima feature P1 pendente — `followups_v0_1_to_10.md:23`). Sem isso,
o Forge e uma ilha: agentes so o usam via CLI subprocess, perdendo a
descoberta de tools e a composicao que o ecossistema MCP oferece.

O problema e que protocolos abertos sao, por construcao, **superficies de
tool poisoning e capability leakage**. O tau-bench (tool-call evaluation)
documenta que clientes MCP confiam em tools/list e tools/call sem separar
"o servidor pode fazer X" de "este caller pode pedir X"; o paper "Tool
Poisoning" (Kolahal et al., 2025) mostra que tools maliciosas se escondem em
metadados (`_meta`) que o cliente renderiza sem sandbox. Adicionar MCP ao
Forge sem disciplina re-introduz, pela porta dos fundos, exatamente as classes
de bug que o ADR 0023 (memory trust) e ADR 0007 (governance) tornaram
irrepresentaveis: autoridade sem provenance, mutacao sem intent declarada,
caller anonimo mutando shared state.

Este ADR formaliza o design do F08 (expandindo o stub original). O principio
central e o mesmo do ADR 0024 (PDP/PEP): a superficie de protocolo e um PEP
burro; toda decisao de autorizacao e mutacao vive no kernel.

## Decisao

### 1. Adapters nao sao fonte de verdade e nao mutam store diretamente

O MCP server (crate `forge-core-protocol-mcp`) e um **adapter** sobre o
`command_registry::COMMANDS` existente
(`crates/forge-core-cli/src/command_registry.rs:68`), nao uma segunda
implementacao do engine. Cada MCP tool e um wrapper pass-through:

1. recebe `(tool_name, arguments)` do cliente MCP;
2. mapeia para um argv `&[String]` no formato que o `CommandSpec::handler`
   correspondente ja aceita;
3. invoca o handler e captura o `CliEnvelope` JSON que ele emite em stdout
   (`crates/forge-core-contracts/src/envelope.rs:77`);
4. retorna o envelope como o tool result.

Nenhuma logica de dominio vive no adapter. O adapter ganha sua vida no
**deletion test** (Ousterhout): remove-lo custa aos callers acesso
programatico sobre stdio JSON-RPC, mas nao custa funcionalidade — os
comandos subjacentes continuam disponiveis via CLI. O adapter e deep porque
concentra o acoplamento ao `rmcp` (Rust MCP SDK) num unico seam; sem ele, o
acoplamento se espalharia por cada command handler.

**Rejeitado: implementar o engine dentro do adapter.** Isto duplicaria a
logica de cada comando e quebraria o deletion test (remover o adapter
destruiria funcionalidade). E exatamente o anti-pattern que o ADR 0024
combate: PEP com logica de PDP.

### 2. Toda mutacao passa pelo kernel e por um OperationContract

O principio inviolavel (do stub original, mantido): **o adapter nao muta a
store diretamente.** Toda mutacao flui pelo kernel (`execute-operation`,
`claim acquire`) e carrega um `OperationContract` que declara a intent
autorizada. O adapter apenas encaminha; o kernel continua sendo o unico PDP
para mutacao, consistente com ADR 0023/0024.

Isso resolve o vector de tool poisoning no level do schema: um cliente MCP
malicioso que peça `execute-operation` sem `OperationContract` e rejeitado
no **MutateGate** (o ponto de enforcement na fronteira do adapter, ver
termo em `CONTEXT.md`) antes de o kernel ser alcancado. Fail-closed.

**Rejeitado: confiar no cliente para validar autoridade.** O tau-bench
mostra que clientes MCP nao sao PDPs confiaveis — eles renderizam metadados
de tools sem sandbox. A autoridade deve ser verificada do lado do Forge,
nunca delegada ao caller.

### 3. Allowlist = a superficie de capability

O conjunto de MCPTools que uma instancia do servidor expoe e declarado
explicitamente em `mcp-allowlist.yaml` (Allowlist, ver `CONTEXT.md`). Uma
tool ausente da Allowlist e invisivel em `tools/list` e rejeitada em
`tools/call` — fail-closed. A Allowlist separa "o Forge pode fazer X" de
"este cliente MCP pode pedir X": e a fronteira de capability.

A Allowlist e **dados, nao codigo** (mirrors o modelo de risk-audit
`risk-audit-v0` em `CONTEXT.md:25`): adicionar uma tool a um servidor nao
exige mudanca em Rust. Declarar uma Allowlist vazia ou restrita e o estado
seguro default.

**Rejeitado: expor todos os comandos por default.** Quebra o principio de
menor privilegio e faz do MCP server um espelho completo do CLI — a superficie
de ataque cresce sem necessidade. O default deve ser o mais restrito possivel.

### 4. Attestation (signed tool calls) — o modelo de identidade do caller

stdio JSON-RPC nao carrega headers HTTP; nao ha `Authorization:`. A prova de
*quem chamou* precisa vir no corpo da requisicao. Decisao: cada `tools/call`
carrega uma **Tool-Call Attestation** — uma assinatura ed25519 detached sobre
a forma canonica da intent do tool-call:

```
canonical = serde_json_canonicalizer::canon({
  "tool": <tool_name>,
  "arguments": <arguments_object>,
  "nonce": <opaque>,
  "ts": <unix_seconds>
})
sig = ed25519.sign(caller_private_key, canonical)
```

carregada no campo `_meta.attestation` da mensagem JSON-RPC (o campo que o
MCP spec reserva para extensoes). O adapter verifica a assinatura contra uma
chave publica autorizada configurada no servidor (reusa `forge-core-crypto`,
que ja pinou `ed25519-dalek 2.2` no workspace `Cargo.toml:36`).

**Politica default** (decisao hard-to-reverse, registrada aqui):

- **Mutate MCPTools** (`execute-operation`, `claim acquire`): Tool-Call
  Attestation **obrigatoria**. Sem assinatura valida = rejeitado no
  MutateGate.
- **Read-only MCPTools** (`preview`, `ready`, `graph`, `explain`,
  `memory list`, `query-effect-index`): attestation **opcional** sob a
  politica default (o servidor pode endurecer via configuracao).

A assinatura prova origem (quem); o `OperationContract` prova intent
autorizada (o que). Amb sao necessarios para uma mutacao — nenhum sozinho
suficiente. Isto e o MCP/stdio analogo de um HTTP request signed
(`Signature:` header de Sigstore / HTTP Signatures), transposto para um
transporte sem headers.

**Rejeitado: bearer token no `_meta`.** Tokens sobre stdio local sao ou
publicos (sem valor) ou secretos compartilhados (phishing/vazamento). Uma
assinatura detached prova posse da chave privada sem revela-la, e vincula a
intent (nao replayable por outra tool). Reuso de `forge-core-crypto` mantem
zero deps novas.

**Rejeitado: attestation obrigatoria para tudo (read-only incluido).**
Endurecimento excessivo quebra o caso de uso principal (Claude Desktop
lendo estado Forge sem setup cripto por chamada). A porta esta aberta para
endurecer via config; o default segue o principio de menor fricao no eixo
que nao muta.

### 5. Reuso do `rmcp` (Rust MCP SDK), nao hand-roll JSON-RPC

O adapter usa `rmcp` (`docs.rs/rmcp`, versao 1.7) para o transporte JSON-RPC
sobre stdio. Hand-rolling JSON-RPC rejeitado: o `rmcp` ja mapeia o macro
`#[tool]` para expor fn como tool e serializa params/results automaticamente —
e o mecanismo por tras de `tools/list` e `tools/call`. O workspace ja pinou
`tokio` (`Cargo.toml:64`, `features = ["rt","time"]`); o adapter adiciona
localmente `io-util` + `macros` (necessarios para stdio server) sem mexer no
pin do workspace, que outras crates dependem.

## Consequencias

- **Interoperabilidade sem entregar autoridade.** Agentes externos
  (Claude Desktop, etc.) descobrem e invocam Forge via MCP, mas a fonte de
  verdade continua no kernel. O adapter e um PEP, nao um PDP (ADR 0024).
- **Tool poisoning mitigado por design.** Um tool malicioso que se esconde
  em `_meta` nao ganha mutacao sem `OperationContract` + attestation; um
  caller anonimo e rejeitado no MutateGate. A superficie de ataque do
  protocolo nao e a superficie de mutacao da store.
- **Trace e audit consistentes.** Toda chamada MCP mutante passa pelo
  mesmo kernel que a CLI, entao gera o mesmo trail de WAL/telemetry. Nao ha
  "path MCP" e "path CLI" divergentes.
- **Capability e dados.** A Allowlist torna a superficie de tools por
  instancia uma decisao de deploy, nao de codigo. Um servidor restrito a
  read-only e uma linha de YAML.
- **Acoplamento isolado.** A dependencia de `rmcp`/tokio-stdio vive numa
  crate so (`forge-core-protocol-mcp`); o resto do workspace nao a ve.
- **Custo: fricao cripto no caso mutate.** Exigir attestation em mutate e
  mais setup que read-only; aceito como trade-off por seguranca. A porta
  para relaxar (politica configuravel) existe, mas o default e fail-closed.

## Escopo desta story (F08.1-F08.7)

- ✅ F08.1: este ADR (Accepted) + termos no `CONTEXT.md` (Secure Protocol
  Adapters, MCPTool, Allowlist, MutateGate, Tool-Call Attestation).
- ⏳ F08.2: criar crate `forge-core-protocol-mcp` (`lib/server/allowlist/
  attestation.rs`); pin de `rmcp` em `[workspace.dependencies]`.
- ⏳ F08.3: MCP server sobre `COMMANDS` (read-only: preview/ready/graph/
  explain/memory list/query-effect-index; mutate: execute-operation/
  claim acquire).
- ⏳ F08.4: Allowlist enforcement (`mcp-allowlist.yaml`); MutateGate
  fail-closed sem `OperationContract`; validator com diagnostics tipados.
- ⏳ F08.5: Tool-Call Attestation (verify ed25519 via `forge-core-crypto`);
  obrigatoria mutate, opcional read-only.
- ⏳ F08.6: CLI `forge-core mcp serve [--allowlist <yaml>]`; registro em
  `command_registry::COMMANDS`.
- ⏳ F08.7: fixtures + E2E (Allowlist deny / mutate sem contract /
  read-only sem attestation); anchor 122 preservada.

## Referencias

- MCP (Model Context Protocol, Anthropic):
  https://modelcontextprotocol.io/specification
- tau-bench (tool-call evaluation, Anthropic 2025):
  https://github.com/anthropics/tau-bench
- Kolahal et al. — Tool Poisoning (2025, arXiv 2506.09566):
  https://arxiv.org/abs/2506.09566
- HTTP Signatures (W3C draft) — precedente de request signed:
  https://datatracker.ietf.org/doc/draft-ietf-httpbis-message-signatures/
- Sigstore (cosign `--signature` detached) — precedente de detached sig:
  https://docs.sigstore.dev/cosign/sign/overview/
- shuttle.dev — How to build a stdio MCP server in Rust (tutorial, 2025):
  https://shuttle.dev/blog/2025/07/18/how-to-build-a-stdio-mcp-server-in-rust
- rmcp (Rust MCP SDK): https://docs.rs/rmcp
- In-repo: ADR 0023 (memory trust model), ADR 0024 (PDP/PEP),
  `command_registry.rs:68` (o seam do adapter), `envelope.rs:77`
  (`CliEnvelope` — o tipo de retorno de cada tool),
  `CONTEXT.md` (termos F08: MCPTool, Allowlist, MutateGate, Tool-Call
  Attestation).
