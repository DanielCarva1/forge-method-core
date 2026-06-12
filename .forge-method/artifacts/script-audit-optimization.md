# Forge script audit and guidance optimization

- kind: audit
- story: script-audit-optimization-p1
- created_at: 2026-06-12
- scope: runtime scripts, install/smoke scripts, guidance routing, agent-facing docs, local plugin install state, experiment plan

## Verdict

Antes desta story, a experiencia humana guiada estava estruturalmente forte, mas ainda nao dava para dizer "tao boa quanto ou melhor" com rigor. O proprio `guide --question` classificou a pergunta atual como `operate-support`, sem sinais, quando deveria ter entendido audit/evolucao do proprio Forge. Isso era um bug de roteamento humano.

Depois do patch, a mesma fala rota para `builder-flow` / `runtime-builder`, com `skill:facilitation/runtime-builder.md`, mantendo o trabalho em `6-evolve`. Isso fecha o buraco encontrado, mas o veredito honesto continua: Forge esta comparavel em estrutura e melhor em estado/artefatos para agente; "melhor para humanos" ainda precisa de replay de transcripts reais e sessoes de usuario, nao so arquitetura.

## Script Inventory

- Total auditado: 20 scripts.
- Runtime principal: `skills/forge-method/scripts/forge_method_runtime.py`, 7466 linhas.
- Updater: `skills/forge-method/scripts/forge_method_updater.py`, 304 linhas.
- Onboarding verifier: `scripts/verify-onboarding-assets.py`, 93 linhas.
- Install/smoke/verify scripts: Windows e POSIX cobrem install local, smoke runtime, smoke install, plugin local, clone install, fixtures, verify-fast e verify-all.

## Tool Findings

- `uvx vulture ... --min-confidence 60`: nenhum codigo morto de alta/media confianca encontrado.
- AST manual scan: command handlers parecem "definition-only" se ignorar `argparse.set_defaults`, entao nao sao codigo morto real.
- `uvx radon cc -s -a`: complexidade media B. Hotspots principais:
  - `build_guidance_decision`: F(58)
  - `cmd_project_create`: E(38)
  - `cmd_gate`: E(31)
  - `audit_project`: D(29)
  - `build_resume_guidance`: D(27)
- `uvx ruff check ...`: limpo depois de remover variavel local nao usada e f-strings sem interpolacao.
- `uvx --from shellcheck-py shellcheck scripts/*.sh`: dois warnings corrigidos.
- `Invoke-ScriptAnalyzer`: sem warnings funcionais depois de filtrar `PSAvoidUsingWriteHost`, que e aceitavel para scripts CLI de smoke; `catch` vazio e parametros `Python` foram corrigidos.
- PowerShell parser, `bash -n`, `py_compile`: limpos.

## Runtime Fixes Applied

- Guidance Engine agora reconhece pedidos de audit do runtime/scripts, codigo morto, docs misleading/enganosos, experiencia guiada e `agente/agentes` como sinais de `builder-flow`.
- Adicionada fixture `runtime_guidance_audit_request` para impedir regressao.
- Benchmark interno atualizado para exigir que audit do metodo/runtime va para `runtime-builder`, nao suporte operacional.
- `doctor` agora retorna e imprime `repair_commands` quando o plugin local esta stale ou quebrado.
- `docs/00-quickstart.md` agora manda seguir `Repair:` quando `doctor` nao estiver `ready`.
- Shell scripts:
  - removida variavel nao usada em `install-plugin-local.sh`.
  - subcomando `story done` em `smoke-runtime.sh` ficou explicitamente quoted para shellcheck.
- PowerShell scripts:
  - `install-plugin-local.ps1` nao tem mais `catch` vazio.
  - `smoke-fixtures.ps1` e `smoke-plugin-clone-install.ps1` usam o parametro `-Python` de forma explicita.

## Environment Finding

`doctor` encontrou plugin local instalado em `1.22.0`, enquanto o runtime atual e `1.27.0`. Isso e risco real de agente usar instrucao antiga quando o entrypoint vier do plugin instalado. O `doctor` agora imprime:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-plugin-local.ps1
```

Depois que a validacao desta branch passar, o plugin local deve ser reinstalado para alinhar o ambiente.

## Market Technique Scan

- Claude Code Hooks: hooks rodam automaticamente em pontos de ciclo de vida, por sessao, turno, chamada de ferramenta, mudanca de arquivo, compactacao, worktree e outros eventos; handlers podem receber JSON e retornar decisoes. Isso sustenta uma futura camada Forge de hooks deterministas para preflight/start/guide/gate/file-change, mas nao justifica criar novo surface agora.
  - Source: https://code.claude.com/docs/en/hooks
- OpenAI Agents tracing: traces/spans registram geracoes, tool calls, handoffs, guardrails e eventos customizados. Isso sustenta um futuro `ledger`/trace mais rico para replay de sessoes e avaliacao de UX humana.
  - Source: https://openai.github.io/openai-agents-python/tracing/
- Ruff/uv: ferramentas Rust-backed para lint/package/tool execution. O audit usou `uvx` para adicionar ferramentas sem fixar dependencia no core.
  - Sources: https://docs.astral.sh/ruff/ and https://docs.astral.sh/uv/

## Experiment Plan

Nao misturar experimentos no core. Usar worktrees/branches separados depois do commit desta story:

- `codex/experiment-hooked-runtime`: prototipo de hook registry no runtime Python, com eventos `pre-command`, `post-command`, `guide-classified`, `state-written`, `artifact-written`, `gate-failed`, `context-compact`.
- `codex/experiment-rust-cli`: prototipo Rust com `clap`/`serde` para medir cold-start, parser ergonomics e binario unico. Criterio: so vale se reduzir latencia/packaging sem perder hackability.
- `codex/experiment-ts-transcript-harness`: harness TypeScript para replay de transcripts humanos e comparacao de outputs do Guidance Engine.
- `codex/experiment-app-harness`: UI/app local para visualizar state machine, ledger, route decisions e transcript replays. Isso e experiencia de operador, nao substituto do runtime CLI.

## Follow-up Risks

- `build_guidance_decision` esta complexo demais. Nao refatorar no escuro; primeiro extrair fixtures/replay suficientes para proteger comportamento.
- O monolito Python ainda e aceitavel para o core, mas a fronteira natural e separar routing/guidance, catalog validation, context, release e CLI parser em modulos quando houver cobertura maior.
- Plugin stale e problema de ambiente; `doctor` agora orienta, mas o fluxo ideal e um hook/launcher que reduza ainda mais chance de skill antiga responder.
