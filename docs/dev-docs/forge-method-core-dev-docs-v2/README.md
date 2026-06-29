# Forge Method Core v2 - pacote de desenvolvimento auditado

Data: 2026-06-28
Janela cientifica aplicada para papers: 2025-10-28 a 2026-06-28

Este pacote transforma a pesquisa auditada, as fontes usadas e as recomendacoes arquiteturais em documentacao de desenvolvimento para o `forge-method-core`.

## Como usar

1. Leia `00_master_development_doc.md` para a direcao geral.
2. Use `01_feature_specs.md` como backlog de produto e engenharia.
3. Use `02_implementation_plan.md` para sequenciar a implementacao.
4. Use `03_architecture_and_contracts.md` para os contratos novos.
5. Use `04_rust_refactor_guide.md` antes de pedir para agentes mexerem no Rust.
6. Use `05_eval_and_quality_plan.md` para transformar pesquisa em gates, evals e CI.
7. Use `06_protocol_security_plan.md` para MCP, A2A, identidade e governance.
8. Use `adrs/` para registrar decisoes tecnicas iniciais.
9. Use `data/` para importar backlog e evidence ledger em issue tracker, planilha ou dashboard.
10. Use `schemas/` como rascunho de contratos YAML v0.

## Premissa central

Forge nao deve tentar ser o melhor agente. Forge deve ser o kernel de coordenacao, verificacao e governanca que permite usar qualquer agente com contrato, evidencia, gates, rollback, trace e auditoria.

## Nao objetivos

- Nao transformar multi-agent em default de produto.
- Nao mover semantica viva para Rust manual antes de estabilizar contrato.
- Nao aceitar MCP/A2A como seguro por default.
- Nao tratar memoria como vector DB sem policy.
- Nao vender velocidade sem preview, verify e undo.
