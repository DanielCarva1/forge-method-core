# V1 Readiness Audit

Forge Method Core v1 is ready when a new user can install it, start a project, recover state after context loss, complete implementation work with evidence, and mark the project ready for use.

## Readiness Criteria

| Area | Criteria | Evidence |
| --- | --- | --- |
| Install | Windows and POSIX installers copy the skill and runtime files. | `scripts/smoke-install.ps1`, `scripts/smoke-install.sh` |
| Start | The runtime routes empty workspaces, existing projects, project parent folders, and runtime repos without writing accidental state, and exposes explicit decision options for the next user choice. | `preflight`, `start`, unit tests |
| Project Creation | A normal project can be scaffolded from every packaged module, and examples can be generated for every packaged module. | `project create`, `example create`, fixture matrix smoke |
| Durable State | Project state, sprint state, stories, evidence, artifacts, checkpoints, reviews, inputs, and ledger are file-backed. | `.forge-method/` project layout and audit checks |
| Context Recovery | A new session can recover state, read order, commands, done conditions, blocking conditions, and context budget risk from files. | `context plan`, `context health`, `context recover`, `context recover --compact` |
| Autonomous Build | Build-phase work can select a story, record review findings, require evidence, and block done state until findings are resolved. | story/review/evidence tests |
| Ready Phase | Projects can enter `5-ready-operate` only after audit and release evidence. | `ready`, quality gate tests |
| Distribution | The package validates as a Codex plugin, exposes a repo marketplace for GitHub installation, can be installed into a personal marketplace, prints Codex plugin deeplinks for activation/sharing, diagnoses the installed plugin package, provides listing/onboarding metadata, and can also be cloned from a Git ref, installed locally as a plugin or skill fallback, validated across packaged module fixtures, and published through a versioned release. | repo marketplace catalog, plugin validation, plugin-local smoke, onboarding asset validation, fixture matrix smoke, clone/install smoke, `doctor`, `release check`, CI, GitHub release |
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
- installable from a GitHub marketplace source, but not yet a public directory listing

## Final Release Gate

Before a v1 release is called stable:

1. `python scripts/test-runner.py --workers 4 --timeout 120 --report .forge-method/test-runs/manual.json`
2. `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
3. `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
4. `powershell -ExecutionPolicy Bypass -File .\scripts\verify-all.ps1`
5. `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-fixtures.ps1`
6. `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-plugin-clone-install.ps1 -Ref v<version> -ExpectedVersion <version>`
7. product-surface scan for comparison-name noise
8. GitHub CI passes on Windows and Linux
9. GitHub release points to the tagged commit

## Known Post-V1 Productization

- public directory submission/approval
- signed release artifacts
