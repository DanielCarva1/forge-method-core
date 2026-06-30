# Forge Method Context Glossary

## Consumer Project Repo

The application, game, library, or product repository being developed with Forge Method. It owns product source code and may carry a small Forge Project Link, but it does not own Forge runtime state. Consumer repos must not use a local `<consumer>/.forge-method` state root.

## Forge Runtime Sidecar

A sibling directory or repository that owns the Forge Method runtime state for one Consumer Project Repo. It contains the real `.forge-method/` tree, including state, artifacts, evidence, ledger, stories, and claims.

## Forge Project Link

The small `.forge-method.yaml` file stored at a Consumer Project Repo root. It points to the Forge Runtime Sidecar and its `.forge-method/` state root. Its `state_root` must resolve under `sidecar_root` and must end in `.forge-method`, normally as `<sidecar_root>/.forge-method`.

## Project Init Bootstrap

Consumer repos should be bootstrapped with `forge-core project init --root <repo>`. The intended result is a `.forge-method.yaml` pointer in the Consumer Project Repo plus sibling sidecar state at `../forge-<project>/.forge-method`. The Consumer Project Repo should not receive a local `.forge-method/` directory.

The init command is expected to be idempotent for the same resolved link and to fail closed on a conflicting existing link or unsafe consumer-local state root.

## Bootstrap Core Exception

The temporary exception that allows `<repo-root>` to keep local `.forge-method/` state while the Forge core is still being developed by Forge itself. Commands that resolve this local state must opt in with `--allow-bootstrap-core`. This exception is explicit and must not be copied to consumer projects.

## Risk Audit

A fail-closed inspection pass over source code that detects AI-induced
anti-patterns (fail-soft, exception swallowing, security slop, false tests)
and accumulates typed `Diagnostic`s into a `ValidationReport`. Rules are
parametric YAML contracts (`risk-audit-v0`), never hardcoded imperative
sequences. The gate can run standalone (`forge-core risk-audit`) or be
attached as a precondition to a mutable operation (`RuntimeOperationExecutionContext`).
Findings carry per-file evidence so agents and humans can act on them.

## Anti-pattern (AI Code)

A named, parametrizable pattern in source code or test artifacts that is
forbidden because it correlates with AI-induced failure modes (fail-soft,
exception swallowing, security slop, false tests). Each anti-pattern is
declared in a `risk-audit-v0` contract with a `detector` (regex, glob,
AST node, external linter, or required file existence), a severity
(Error/Warning), an evidence requirement, and a fix hint. Anti-patterns
are data, not code: adding one must not require a Rust change.

## Project Link Hardening Rules

- `forge-core project init --root <repo>` is the normal first-use path for Consumer Project Repos.
- Init should be idempotent for the same resolved link and fail closed on a conflicting existing link or unsafe consumer-local state root.
- Consumer `state_root` must be inside the configured `sidecar_root` and end in `.forge-method`.
- Consumer `state_root` must not be `<consumer>/.forge-method`; only the Forge core bootstrap exception may use local runtime state.
- Runtime and claim commands fail closed when the resolved `state_root` does not exist; they must not silently create consumer-local state.
- State-bearing operation/effect commands (`execute-operation`, `rebuild-effect-index`, `query-effect-index`) resolve the same Project Link: product contracts and payload files are read from the Consumer Project Repo, but Forge WAL, metadata index, evidence, and `.forge-method/artifacts/*` writes land under the Forge Runtime Sidecar.
- `--claims-dir` remains an explicit advanced override for tests, migrations, and emergency repair.
- The goal is isolation: projects, users, and agents must not contaminate each other's Forge data.

## Remaining Bootstrap Gaps

- The global Forge skill/start script now calls `forge-core project init --root <repo>` when a first-use consumer repo lacks a Project Link, unless `-NoInit` is passed.
- Product readiness still depends on verified clean install, init, project resolution, and state-bearing command flow from a consumer repo.
