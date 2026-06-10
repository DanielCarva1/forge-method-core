# Runtime Backlog

## Delivered In V1

### Runtime Identity And Routing

- runtime repo detection through `.codex-plugin/plugin.json`
- method project detection through `.forge-method/state.yaml`
- start route with known project listing
- runtime-vs-project doctor command
- status command that does not infer from chat history

### Durable State Engine

- state schema version
- project registry
- sprint summary
- story files
- evidence files
- append-only runtime ledger
- phase transition validation
- story transition validation

### Build And Verify Loop

- story add/list/start/review/done/block
- required evidence for done stories
- check recording
- audit command
- quality gate command
- ready gate command

### Context Continuity

- context pack generation
- handoff generation
- checkpoint generation
- active story tracking
- recent evidence tracking

### Artifact System

- artifact freshness checks
- ephemeral artifact capture
- artifact existence evals

### Eval System

- workflow routing evals
- workflow trigger evals

### Distribution

- plugin manifest
- user skill installer
- install smoke test
- runtime smoke test
- unit tests
- CI workflow

## V1 Hardening

### Cross-Platform Installers

- macOS installer
- Linux installer
- shell smoke tests

### Module Packs

- software builder module
- creative studio module
- game studio module
- runtime builder module
- test architect module
- launch/operate module

### Context Pack Builder

- include checkpoint memory
- include artifact summaries
- include failing command summaries
- include touched file summaries

### Release Quality

- signed releases
- versioned changelog
- plugin marketplace packaging
- example projects per module
