# V1 Readiness Audit

Forge Method Core v1 is ready when a new user can install it, start a project, recover state after context loss, complete implementation work with evidence, and mark the project ready for use.

## Readiness Criteria

| Area | Criteria | Evidence |
| --- | --- | --- |
| Install | Windows and POSIX installers copy the skill and runtime files. | `scripts/smoke-install.ps1`, `scripts/smoke-install.sh` |
| Start | The runtime routes empty workspaces, existing projects, project parent folders, and runtime repos without writing accidental state. | `preflight`, `start`, unit tests |
| Project Creation | A normal project can be scaffolded from a packaged module. | `project create`, install/runtime smokes |
| Durable State | Project state, sprint state, stories, evidence, artifacts, checkpoints, reviews, inputs, and ledger are file-backed. | `.forge-method/` project layout and audit checks |
| Context Recovery | A new session can recover state, read order, commands, done conditions, and blocking conditions from files. | `context plan`, `context recover`, `context recover --compact` |
| Autonomous Build | Build-phase work can select a story, record review findings, require evidence, and block done state until findings are resolved. | story/review/evidence tests |
| Ready Phase | Projects can enter `5-ready-operate` only after audit and release evidence. | `ready`, quality gate tests |
| Distribution | The package validates as a Codex plugin, can be installed into a personal marketplace, prints Codex plugin deeplinks for activation/sharing, and can also be cloned, installed locally as a skill fallback, validated, and published through a versioned release. | plugin validation, plugin-local smoke, `release check`, CI, GitHub release |
| Product Surface | Product docs and runtime copy stay independent and do not create naming noise from comparison research. | repository search before release |

## Maturity Classification

Current maturity target for v1:

```txt
Assisted production runtime
```

Meaning:

- suitable for real projects with an agent/operator in the loop
- durable enough to survive context resets and terminal restarts
- auditable through files, checks, evidence, and CI
- not yet a marketplace-polished one-click product

## Final Release Gate

Before a v1 release is called stable:

1. `python -m unittest discover -s tests`
2. `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
3. `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
4. `powershell -ExecutionPolicy Bypass -File .\scripts\verify-all.ps1`
5. product-surface scan for comparison-name noise
6. GitHub CI passes on Windows and Linux
7. GitHub release points to the tagged commit

## Known Post-V1 Productization

- marketplace-backed plugin installation
- public marketplace listing metadata
- signed release artifacts
- richer visual onboarding
- broader real-project fixture coverage
