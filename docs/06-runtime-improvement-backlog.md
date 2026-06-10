# Runtime Backlog

## Delivered In V1

### Runtime Identity And Routing

- runtime repo detection through `.codex-plugin/plugin.json`
- method project detection through `.forge-method/state.yaml`
- start route with known project listing
- runtime-vs-project doctor command
- status command that does not infer from chat history
- runtime snapshot for machine-readable agent routing

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
- recovery brief generation
- handoff generation
- checkpoint generation
- active story tracking
- recent evidence tracking
- checkpoint memory in context packs
- artifact summaries in context packs
- failing command summaries in context packs
- touched file summaries in context packs

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
- macOS installer
- Linux installer
- shell smoke tests
- unit tests
- CI workflow
- example projects per module

### Module Packs

- software builder module
- creative studio module
- game studio module
- runtime builder module
- test architect module
- launch/operate module

## V1 Hardening

### Release Quality

- signed releases
- versioned changelog
- plugin marketplace packaging
