# Forge Core M2.1 claim bus resolver plan

Date: 2026-06-29
Branch: `codex/forge-project-link-hardening`

## Status

CB-S3 docs lane updated. ProjectLink/sidecar hardening acceptance is now
documented for the follow-up slice. The expired-claim handoff protocol gap
exposed by the Hostfully sidecar is closed by this handoff slice once the
implementation lane lands; it is not a future TODO.

## Why this comes before graph M2.1

M2 graph is useful, but the protocol promise that matters most for Daniel's
multi-project/multi-agent use is: agents in a repo must coordinate on that
repo's resolved Forge runtime state, not on a hard-coded local directory.

Current gap:

- `project resolve` correctly finds the sidecar state root.
- `$forge-method` startup script correctly uses `<state_root>/claims-active`.
- But raw `forge-core claim ...` commands still default to `contracts/claims`.

That means a host agent can accidentally inspect or mutate the wrong claim bus.

## Product goal

Make `forge-core claim acquire|heartbeat|release|status|check-write|handoff`
resolve the project by default and use the resolved sidecar state. Claim-bus
operations use:

```txt
<resolved_state_root>/claims-active
```

Handoff records are written under
`<resolved_state_root>/handoffs/expired-claims/`. For all claim commands,
`--claims-dir` remains the advanced override.

The handoff command's official contract is:

```txt
forge-core claim handoff --id <claim-id-or-scope-id> --agent <id> --summary <text> [--evidence <path>...] [--root <path>] [--claims-dir <path>] [--now-unix <epoch>] [--no-json]
```

## Non-goals

- No change to the pure claim engine.
- No graph operation preflight in this slice.
- No `anyhow`, `thiserror`, or CLI parser framework migration.

## Stories

### CB-S1 - claim CLI resolver-owned defaults

Ownership:

- `crates/forge-core-cli/src/main.rs`

Acceptance:

- All claim subcommands accept `--root <path>` and `--allow-bootstrap-core`.
- If `--claims-dir <path>` is omitted, the command resolves the project and
  uses `<state_root>/claims-active`.
- If `--claims-dir <path>` is supplied, existing explicit behavior is preserved.
- Missing project link without explicit `--claims-dir` fails closed with a clear
  project-resolve error.
- `claim --help` and per-command help mention `--root`, `--allow-bootstrap-core`,
  and `--claims-dir`.

### CB-S2 - sidecar e2e coverage

Ownership:

- `crates/forge-core-cli/tests/claim_cli_sidecar_e2e.rs`

Acceptance:

- Consumer app with `.forge-method.yaml` can run raw `forge-core claim acquire`
  without `--claims-dir`; claim file is written under sibling sidecar
  `.forge-method/claims-active`.
- `claim status` without `--claims-dir` reads that sidecar bus.
- `claim check-write` without `--claims-dir` uses that sidecar bus.
- No consumer-local `.forge-method` is created.
- Explicit `--claims-dir` still writes to the override directory.
- Missing project link without `--claims-dir` exits non-zero.

### CB-S3 - docs and durable usage examples

Ownership:

- `README.md`
- `docs/planning/forge-core-m2-claim-bus-resolver-plan.md`

Acceptance:

- README claim examples use `--root .` instead of hard-coded `--claims-dir`.
- README explains `--claims-dir` is an advanced override.
- Sidecar section says raw claim CLI now resolves the project state root.
- README documents official expired-claim handoff recovery and forbids manual
  claim-file moving as the recovery path.

### CB-S4 - expired handoff recovery command

Ownership:

- `crates/forge-core-decisions/src/claim_engine.rs`
- `crates/forge-core-cli/src/claim.rs`
- `crates/forge-core-cli/src/main.rs`

Acceptance:

- Expired `handoff_required` claims intentionally block heartbeat, release, and
  acquire for the affected scope until recovery context is recorded.
- `forge-core claim handoff` records recovery context under sidecar state
  `handoffs/expired-claims/`.
- Handoff marks the old claim `handoff_recorded` and reopens the scope for a
  new claim.
- Recovery is never manual claim-file moving.
- Default claim commands resolve sidecar state with `--root`; `--claims-dir` is
  an advanced override for tests, migrations, and emergency repair.
- The Hostfully sidecar incident is the motivating case and is closed by this
  handoff protocol fix once the implementation lands.

### PL-H1 - ProjectLink sidecar hardening rules

Ownership:

- `README.md`
- `CONTEXT.md`
- `docs/planning/forge-core-m2-claim-bus-resolver-plan.md`

Acceptance:

- Consumer `.forge-method.yaml` must point `state_root` under `sidecar_root`.
- Consumer `state_root` must not be local `<consumer>/.forge-method`; local
  state is allowed only for the Forge core bootstrap exception.
- Runtime and claim commands fail closed when the resolved state root does not
  exist instead of silently creating consumer-local state.
- `--claims-dir` remains an explicit advanced override for tests, migrations,
  and emergency repair.
- The isolation invariant is explicit: multiple projects, users, and agents
  must not contaminate each other's Forge data.

## Parallel split

| Lane | Agent | Write scope | Output |
|---|---|---|---|
| L1 CLI | worker | `crates/forge-core-cli/src/main.rs` | Resolver-owned claim argv/defaults/help |
| L2 E2E | worker | `crates/forge-core-cli/tests/claim_cli_sidecar_e2e.rs` | Sidecar/override/missing-link CLI tests |
| L3 Docs | worker | `README.md`, this plan | User-facing examples and status |
| L4 Handoff | worker | claim engine/CLI files | Expired-claim handoff recovery command |
| L5 Docs | worker | `README.md`, `CONTEXT.md`, this plan | ProjectLink/sidecar hardening acceptance rules |
| R1 Review | validator | read-mostly | Fail-closed semantics and sidecar isolation |

## Verification

Targeted:

```powershell
cargo test -p forge-core-cli --test claim_cli_sidecar_e2e
cargo test -p forge-core-cli --test claim_e2e
cargo check -p forge-core-cli
```

Closing:

```powershell
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -W clippy::pedantic
cargo fmt --all -- --check
forge-core validate --root . --json
git diff --check
```

## Closed Hostfully gap

Hostfully exposed the missing recovery path when its sidecar hit an expired
`handoff_required` claim and had to preserve ad-hoc context under
`D:\forge-hostfully-related-work\.forge-method\handoffs\expired-claims\`.
That gap is closed by the official handoff protocol above: the command records
context in `handoffs/expired-claims/`, transitions the old claim to
`handoff_recorded`, and reopens the scope without manual file moves.

## OperationContract-aware graph dry-run follow-up

M2 graph dry-run must not trust graph YAML alone for mutation safety. The
follow-up acceptance criteria are:

- operation refs resolve relative to the resolved project root;
- missing or invalid OperationContract refs fail closed during dry-run;
- dry-run reports per-node preview/readiness metadata from the OperationContract;
- `RuntimeReadyStatus::NotReady` blocks nodes when preview says the operation is
  blocked, awaiting human input, gate-required, review-required, or when a
  mutating operation is not ready;
- effective `mutation_capable` is derived from OperationContract authority and
  side-effect policy, not trusted from graph YAML alone;
- failed verifier nodes block downstream effective mutations;
- dry-run remains non-mutating and does not append trace/effect records;
- no `anyhow` or `thiserror`.

## Next slices after this

1. Project link hardening implementation for the PL-H1 rules above.
2. OperationContract-aware graph dry-run.
3. Graph claim preflight.
4. Eval compare baseline.

## Graph claim preflight follow-up

OperationContract-aware dry-run closes the "graph YAML lied about mutation"
gap, but a green mutating graph also needs live writer authority. The graph
claim preflight slice adds that gate:

- read-only graphs can pass without `--agent`;
- effective mutating nodes require claim preflight;
- missing `--agent`, no covering self claim, expired self claim, peer-owned
  claim, unreadable claim bus, or unsupported glob write targets block dry-run;
- default claim bus is `<resolved_state_root>/claims-active`;
- `--claims-dir` remains an advanced override and `--now-unix` makes expiry
  deterministic in tests;
- ToolEffect file-backed writes are claim targets using the same physical-ref
  mapping as the effect store (`file_path`, `artifact_id`, `evidence_id`,
  `ledger_stream`, and `request_stream`);
- when no file-backed writes exist, dry-run falls back to OperationContract
  coordination target paths;
- dry-run remains non-mutating: no claim writes, no local `.forge-method` in
  consumer repos, no handoff, trace, ledger, or effect append.
