# Forge Core 10/10 Product Hardening Plan

Status: active technical plan  
Owner: Forge core maintainers and agents  
Last reviewed: 2026-07-05

## Purpose

This document turns the current audit findings into an evidence-backed,
incremental hardening plan. The goal is to make Forge Method Core excellent as
both a protocol runtime and a usable product without weakening its essence:

- typed contracts over prose,
- fail-closed mutation,
- sidecar-owned runtime state,
- append-only provenance,
- machine-readable CLI envelopes,
- and agent-first interoperability.

The plan is intentionally staged. It avoids a big-bang rewrite and keeps every
step independently verifiable.

## Audit baseline findings

The initial audit established these findings. The live repository state, not
stale documentation, remains authoritative for any future update:

- `cargo metadata --format-version=1 --no-deps` reports 18 workspace members.
- `cargo check --workspace` passes.
- `cargo fmt --all -- --check` passes.
- `cargo run -p forge-core-cli -- validate --root . --json` passes with zero
  diagnostics.
- `cargo test --workspace --no-fail-fast` and
  `cargo clippy --workspace --all-targets -- -D clippy::pedantic` fail on
  Windows before the test suite can run because
  `crates/forge-core-protocol-mcp/src/server.rs` uses `File::write_all`
  without importing `std::io::Write` in a Windows-only test helper.
- `AGENTS.md` still says the workspace has 10 crates; the live workspace has
  18 members.
- The README repository layout lists only a subset of the current crates.
- The global usage table says `forge-core start [--agent-id <id>]`, while the
  `start` parser rejects `--agent-id`.
- `project resolve --allow-bootstrap-core` recognizes the core repository's
  Bootstrap Core Exception, but `start --root .` without that flag reports
  `no_link`. The command behavior is defensible for ordinary consumer repos,
  but the product surface needs to make the Bootstrap Core Exception explicit
  and consistent.

## First hardening changeset evidence

This first implementation slice resolves the immediate red gate and the
highest-signal drift surfaces:

- The Windows-only MCP test helper imports `std::io::Write`.
- CI now has a Linux quality gate plus an explicit
  `ubuntu-latest` / `windows-latest` platform matrix for
  `cargo check --workspace --all-targets` and `cargo test --workspace`.
- Workspace layout is generated from Cargo metadata in
  `docs/generated/workspace-layout.md` and checked by CI.
- README and AGENTS no longer hand-maintain crate counts.
- `start` accepts optional `--agent-id`, advertises `--allow-bootstrap-core`,
  and explicitly diagnoses the Forge core Bootstrap Core Exception without
  treating arbitrary consumer-local `.forge-method/` state as safe.
- Local verification after this slice:
  - `python scripts/generate-workspace-layout.py --check`
  - `cargo fmt --all -- --check`
  - `cargo check --workspace --all-targets`
  - `cargo test --workspace --no-fail-fast`
  - `cargo clippy --workspace --all-targets -- -D clippy::pedantic`
  - `cargo run -p forge-core-cli -- validate --root . --json`

## Second hardening changeset evidence

The second implementation slice deepens the Command Surface seam without a
big-bang parser rewrite:

- A dependency-free `forge-core-command-surface` crate now owns canonical
  command names, usage lines, coarse authority class, JSON mode, and MCP
  visibility metadata.
- `forge-core-cli::command_registry` now adds handler pointers to the shared
  metadata instead of owning a second copy of command usage facts.
- The MCP adapter validates explicit allowlists against the shared command
  surface, not against a dev-only dependency on CLI internals.
- Default MCP read-only and mutate tool sets are projections of
  `McpVisibility`, not independent arrays inside the MCP adapter.
- `tools/list` descriptors now include canonical command usage and JSON mode
  projected from the shared Command Surface.
- Focused verification after this slice:
  - `cargo check -p forge-core-command-surface -p forge-core-protocol-mcp -p forge-core-cli --all-targets`
  - `cargo test -p forge-core-command-surface`
  - `cargo test -p forge-core-cli --lib command_registry`
  - `cargo test -p forge-core-protocol-mcp --lib allowlist`
  - `cargo test -p forge-core-protocol-mcp --lib mcp_tool_descriptor_projects_command_surface_usage`
- Workspace verification after this slice:
  - `python scripts/generate-workspace-layout.py --check`
  - `cargo fmt --all -- --check`
  - `cargo check --workspace --all-targets`
  - `cargo clippy --workspace --all-targets -- -D clippy::pedantic`
  - `CARGO_BUILD_JOBS=1 cargo test --workspace --no-fail-fast`
  - `cargo run -p forge-core-cli -- validate --root . --json`
  - `git diff --check`

## Third hardening changeset evidence

The third implementation slice completes the generated-docs part of Stage 4:

- `cargo run -p forge-core-command-surface --example generate_command_surface_docs`
  renders `docs/generated/command-surface.md` directly from
  `forge_core_command_surface::COMMANDS`.
- The generator supports `--check`, uses hand-rolled typed errors, and is wired
  into CI so command documentation cannot drift silently.
- `README.md` now points users and agents at both generated references:
  workspace layout and command surface.
- `CommandAuthority`, `JsonMode`, and `McpVisibility` now expose stable
  snake-case identifiers for generated docs and future adapter projections.

## Fourth hardening changeset evidence

The fourth implementation slice keeps the host-adapter manifest narrow while
anchoring it to the shared Command Surface seam:

- `host_command` now looks up each host-adapter command in
  `forge_core_command_surface` and derives `json_supported` from `JsonMode`.
  The manifest remains the richer host-security adapter for mutation class,
  authority class, required contracts, safe triggers, and policy references.
- This rejects the rival-registry failure mode without flattening host-specific
  security metadata into the generic command table.
- `start` now has a typed parser adapter (`StartCliOptions`,
  `StartParseOutcome`, `StartParseError`) before the handler invokes the
  read-only diagnostic core. This is the parser pilot requested by Stage 4 and
  preserves the existing command behavior.

## Fifth hardening changeset evidence

The fifth implementation slice completes the non-authoritative display side of
host-adapter projection:

- `HostAdapterProjectedCommand` now carries `canonical_usage` projected from
  `forge_core_command_surface`, so host UIs and MCP-facing surfaces do not need
  to invent or duplicate command usage strings.
- `run_host_adapter_projection` records `canonical_usage` in
  `projected_metadata_must_preserve`, making display metadata drift a testable
  projection concern while leaving authority in the manifest/contracts.
- Library and binary tests assert that projected usage for
  `execute-operation` and `query-effect-index` equals the shared Command
  Surface value.

## Sixth hardening changeset evidence

The sixth implementation slice migrates the MCP CLI adapter's parser/help seam:

- `mcp_cmd` now parses the top-level `mcp` command into typed `McpArgs` /
  `McpArgsError` before starting `serve` or emitting help/errors.
- `mcp --help` now projects the `mcp serve` usage line from
  `forge_core_command_surface::COMMAND_MCP` instead of maintaining another
  hand-written usage line in the adapter.
- Unit tests cover top-level MCP parser routing, help short-circuiting, and
  JSON/text preference preservation on parser errors.
- The MCP CLI E2E help test asserts that binary help includes the canonical
  Command Surface usage line.

## Seventh hardening changeset evidence

The seventh implementation slice closes two remaining `start` drift points:

- `start` local help now projects its usage line from
  `forge_core_command_surface::COMMAND_START` instead of maintaining a rival
  `START_USAGE_LINE` constant beside the parser.
- Command-registry and start-command unit tests assert that the local help seam
  and global Command Surface seam stay aligned.
- Bootstrap Core Exception payloads now include an explicit diagnostic
  reference (`forge-core project resolve --root <root> --allow-bootstrap-core --json`), so the `start` next step preserves the same exception context that made resolution succeed.

## Eighth hardening changeset evidence

The eighth implementation slice migrates the `project` command tree's local
help seam:

- `project_usage()` now keeps the local command-tree header but projects its
  `init` and `resolve` lines from `forge_core_command_surface::COMMAND_PROJECT`
  instead of maintaining a rival hand-written string.
- Unit tests lock all three help paths (`project --help`, `project init --help`,
  and `project resolve --help`) to the same projected usage text.
- The test asserts that `project resolve` help preserves
  `--allow-bootstrap-core`, keeping the `start`/`project resolve` bootstrap
  exception story visible from both entry points.

## Ninth hardening changeset evidence

The ninth implementation slice migrates the `project` command tree's
top-level parser seam:

- `dispatch()` now parses the top-level command into typed `ProjectArgs` /
  `ProjectArgsError` before routing to `init`, `resolve`, or help.
- Unknown-subcommand guidance now derives the `init | resolve` hint from
  `forge_core_command_surface::COMMAND_PROJECT` instead of maintaining another
  rival list beside the dispatcher.
- Unit tests cover top-level parser routing, help routing, and
  Command-Surface-derived unknown-subcommand hints.

## Tenth hardening changeset evidence

The tenth implementation slice migrates the `project resolve` parser seam:

- `project resolve` now parses into typed `ProjectResolveArgs` /
  `ProjectResolveArgsError` before calling `run_resolve`.
- The parser preserves the `--allow-bootstrap-core` flag as an explicit typed
  option, keeping the Bootstrap Core Exception path testable before runtime
  resolution starts.
- Unit tests cover typed option parsing, help short-circuiting, and exact
  missing-value / unknown-argument error variants.

## Eleventh hardening changeset evidence

The eleventh implementation slice migrates the `project init` parser seam:

- `project init` now parses into typed `ProjectInitArgs` /
  `ProjectInitArgsError` before calling `run_init`.
- The parser preserves `--root`, `--project-id`, `--sidecar-root`,
  `--state-root`, and JSON/text selection as explicit typed options instead of
  leaking raw argv handling into the init dispatcher.
- Unit tests cover typed option parsing, help short-circuiting, and exact
  missing-value / unknown-argument error variants.

## Twelfth hardening changeset evidence

The twelfth implementation slice migrates the `claim` help seam:

- `claim --help` keeps the local command-tree header but projects all
  subcommand usage lines from `forge-core-command-surface::COMMAND_CLAIM`.
- Individual `claim acquire|heartbeat|release|handoff|status|reconcile|check-write`
  help paths now render their first usage line from the same Command Surface
  seam instead of preserving rival hand-written strings.
- `COMMAND_CLAIM` now advertises parser-supported flags that were previously
  local-only (`--now-unix`, `--role`, `--ttl`, `--heartbeat-interval`, repeated
  `--target`) and regenerates `docs/generated/command-surface.md`.
- Unit tests lock the top-level and individual claim help projections to the
  shared Command Surface usage lines.

## Thirteenth hardening changeset evidence

The thirteenth implementation slice deepens the Command Surface projection
interface:

- `CommandSpec` now owns reusable projection helpers for local command-tree
  usage lines, concrete subcommand name extraction, unknown-subcommand hints,
  and full usage lookup for an individual subcommand.
- `project` and `claim` now use that interface instead of each owning ad hoc
  string slicing for `forge-core <command>` prefixes.
- `claim` unknown-subcommand errors now derive their hint from
  `COMMAND_CLAIM` instead of preserving a rival hard-coded subcommand list.
- Command Surface tests cover prefix-stripping locality, concrete child-name
  extraction, placeholder filtering, compact hint rendering, and full
  subcommand usage lookup.

## Fourteenth hardening changeset evidence

The fourteenth implementation slice reconciles the `contract` command with the
Command Surface:

- `COMMAND_CONTRACT` now advertises the real handler surface,
  `forge-core contract validate --kind <kind> --file <path> [--json|--no-json]`,
  instead of stale `catalog|snapshot|explain` placeholders.
- `contract --help`, `contract validate --help`, and unknown-subcommand hints
  now project from `COMMAND_CONTRACT` through the shared `CommandSpec`
  projection interface.
- `contract validate` now accepts explicit `--json` as the no-op counterpart to
  `--no-json`, matching the advertised JSON/text selection contract.
- Unit tests lock the local command-tree header, projected usage line, and
  compact `validate` hint to the Command Surface.

## Fifteenth hardening changeset evidence

The fifteenth implementation slice migrates the `memory` command-tree help
adapter to the Command Surface:

- `memory --help` now keeps the local command-tree header and state-directory
  explanation, but projects every subcommand usage line from
  `forge-core-command-surface::COMMAND_MEMORY`.
- `memory ingest|list|forget|promote --help` now render their first usage line
  from the same Command Surface seam, including the explicit
  `[--json|--no-json]` contract that the parser already accepts.
- `memory` unknown-subcommand errors now derive the compact
  `ingest | list | forget | promote | review` hint from `COMMAND_MEMORY`
  instead of preserving a rival hard-coded list.
- Unit tests lock the local command-tree header, projected subcommand usage
  lines, full subcommand usage lookup, and concrete child-name extraction to
  the shared `CommandSpec` projection interface.

## Sixteenth hardening changeset evidence

The sixteenth implementation slice reconciles the `coordination` command with
the Command Surface:

- `COMMAND_COORDINATION` now advertises the real handler surface,
  `forge-core coordination validate [--suite <path>] [--repo-root <path>] [--json|--no-json]`,
  instead of stale top-level `--root` / `--allow-bootstrap-core` flags that the
  coordination suite validator does not parse.
- `coordination --help`, `coordination validate --help`, and
  unknown-subcommand hints now project from `COMMAND_COORDINATION` through the
  shared `CommandSpec` projection interface.
- `coordination validate` now accepts explicit `--json` as the no-op
  counterpart to `--no-json`, matching the advertised JSON/text selection
  contract.
- Unit tests lock the local command-tree header, projected usage line, compact
  `validate` hint, full subcommand usage lookup, and explicit `--json` parsing
  to the Command Surface.

## Seventeenth hardening changeset evidence

The seventeenth implementation slice migrates the `governance` command-tree
help adapter to the Command Surface:

- `governance --help` now keeps the local command-tree header and state-directory
  explanation, but projects every subcommand usage line from
  `forge-core-command-surface::COMMAND_GOVERNANCE`.
- `governance record|conflicts|arbitrate|escalate --help` now render their first
  usage line from the same Command Surface seam, including the explicit
  `[--json|--no-json]` contract that the parser already accepts.
- `governance` unknown-subcommand errors now derive the compact
  `record | conflicts | arbitrate | escalate` hint from `COMMAND_GOVERNANCE`
  instead of preserving a rival hard-coded list.
- Unit tests lock the local command-tree header, projected subcommand usage
  lines, full subcommand usage lookup, and concrete child-name extraction to
  the shared `CommandSpec` projection interface.

## Eighteenth hardening changeset evidence

The eighteenth implementation slice reconciles the `autonomy` command-tree
help adapter with the actual Command Surface:

- `COMMAND_AUTONOMY` now advertises the real implemented handler,
  `forge-core autonomy route --policy-file <path> [--goal-file <path>] [--tool-class <snake_case>]... [--failure-streak <n>] [--json|--no-json]`,
  instead of stale placeholder children (`policy`, `admit`, `decision`) that do
  not exist as CLI adapters.
- `autonomy --help`, `autonomy route --help`, and unknown-subcommand hints now
  project from `COMMAND_AUTONOMY` through the shared `CommandSpec` projection
  interface.
- `autonomy route` now accepts explicit `--json` as the no-op counterpart to
  `--no-json`, matching the advertised JSON/text selection contract.
- Unit tests lock the local command-tree header, projected `route` usage line,
  compact `route` hint, full subcommand usage lookup, concrete child-name
  extraction, and explicit `--json` parsing to the same Command Surface seam.
- The `claim_e2e` fixture temp directory helper now includes a timestamp in
  addition to process id and sequence number, preventing recycled Windows PIDs
  from reusing stale `%TEMP%` claim ledgers and producing false red gates.

## Nineteenth hardening changeset evidence

The nineteenth implementation slice reconciles the `guide` command-tree help
adapter with the actual Command Surface:

- `COMMAND_GUIDE` now advertises the real implemented command tree,
  `forge-core guide describe`, `forge-core guide decide`, and
  `forge-core guide status`, instead of stale top-level `--root` /
  `--allow-bootstrap-core` flags that the guide adapter does not parse.
- `guide --help`, `guide describe|decide|status --help`, and
  unknown-subcommand hints now project from `COMMAND_GUIDE` through the shared
  `CommandSpec` projection interface.
- `guide describe|decide|status` now accept explicit `--json` as the no-op
  counterpart to `--no-json`, matching the advertised JSON/text selection
  contract.
- Unit tests lock the local command-tree header, projected subcommand usage
  lines, full subcommand usage lookup, compact `describe | decide | status`
  hint, concrete child-name extraction, and explicit `--json` parsing to the
  same Command Surface seam.

## Twentieth hardening changeset evidence

The twentieth implementation slice reconciles the `isolation` command-tree help
adapter with the actual Command Surface:

- `COMMAND_ISOLATION` now advertises the real implemented command tree,
  `forge-core isolation propose|status|merge-plan|transition`, instead of a
  stale top-level usage line. Its authority class is now `mixed_by_subcommand`
  because `status` is read-only while `propose` and `transition` write
  isolation contracts.
- `isolation --help`, `isolation propose|status|merge-plan|transition --help`,
  and unknown-subcommand hints now project from `COMMAND_ISOLATION` through the
  shared `CommandSpec` projection interface.
- `isolation propose|status|merge-plan|transition` now accept explicit `--json`
  as the no-op counterpart to `--no-json`, matching the advertised JSON/text
  selection contract.
- Unknown arguments in isolation subcommands now fail closed instead of being
  silently ignored by the parser adapter.
- Unit tests lock the local command-tree header, projected subcommand usage
  lines, full subcommand usage lookup, compact
  `propose | status | merge-plan | transition` hint, concrete child-name
  extraction, explicit `--json` parsing, and unknown-argument rejection to the
  same Command Surface seam.

## Twenty-first hardening changeset evidence

The twenty-first implementation slice reconciles the nested `research`
command-tree help adapter with the Command Surface and deepens the shared seam:

- `CommandSpec` now exposes nested subcommand-path projection helpers:
  `local_usage_lines_under_subcommand_path`,
  `usage_line_for_subcommand_path`, and
  `concrete_child_hint_under_subcommand_path`.
- The new helpers keep `research source add|list` prefix stripping, child-hint
  generation, and full usage lookup in the Command Surface module instead of in
  the CLI adapter.
- `research --help`, `research source --help`, and
  `research source add|list|check|graph|cite --help` now project their usage
  lines from `COMMAND_RESEARCH`, including the explicit `--json|--no-json`
  contract.
- Unknown top-level research subcommands now use the deduplicated
  `source | check | graph | cite` hint; unknown `research source` subcommands
  use the nested `add | list` hint from the same seam.
- Unit tests lock nested path lookup, source-child projection, deduplicated
  hints, command-tree headers, full subcommand usage lookup, and explicit
  `--json` parsing to the Command Surface interface.

## Twenty-second hardening changeset evidence

The twenty-second implementation slice reconciles the `preflight` command
Adapter with the Command Surface and fixes a user-facing help-path polish issue:

- `COMMAND_PREFLIGHT` now advertises both implemented command paths:
  `forge-core preflight ...` and `forge-core preflight init ...`.
- Its authority class is now `mixed_by_subcommand` because the default
  preflight run is read-only while `preflight init` writes the local
  `.forge-method/preflight.yaml` profile document.
- `preflight_usage()` and `preflight_init_usage()` now project canonical usage
  lines from `COMMAND_PREFLIGHT` while preserving the human profile/gate detail
  lines in the CLI help output.
- `forge-core preflight --help` now prints usage and exits successfully instead
  of being treated as an unknown argument by the run parser.
- Unit tests lock the Command Surface authority, the `init` subcommand lookup,
  the rendered usage lines, the retained profile/gate detail lines, and the
  successful help short-circuit.

## Twenty-third hardening changeset evidence

The twenty-third implementation slice removes another stale CLI usage Adapter
from the command help path:

- `cli_util::graph_usage()` now renders from `COMMAND_GRAPH` instead of a
  local `concat!(...)` string.
- This fixes the stale graph help contract that advertised only `[--json]`
  while the Command Surface and parser support the full `[--json|--no-json]`
  selection contract.
- The helper uses the same Command Surface projection formatter pattern as the
  broader registry/help migration while preserving the existing `usage:` header
  expected by the `graph` command tree.
- Unit tests lock `graph_usage()` to every `COMMAND_GRAPH.usage_lines` entry and
  assert the shared JSON/text contract remains visible.

## Twenty-fourth hardening changeset evidence

The twenty-fourth implementation slice removes the next local CLI usage Adapter
from the eval help path:

- `cli_util::eval_usage()` now renders its usage line from `COMMAND_EVAL`
  instead of a local `concat!(...)` string.
- The eval default suite path moved to `COMMAND_EVAL_DEFAULT_SUITE`, so the
  eval implementation and help text share the same command-surface fact.
- The helper preserves the eval-specific `default suite:` detail while keeping
  the canonical usage line, including `[--json|--no-json]`, owned by the shared
  Command Surface seam.
- Unit tests lock `eval_usage()` to every `COMMAND_EVAL.usage_lines` entry and
  assert the shared default suite path remains visible.

## Twenty-fifth hardening changeset evidence

The twenty-fifth implementation slice removes the next local CLI usage Adapter
from the telemetry help path:

- `cli_util::telemetry_usage()` now renders its usage line from
  `COMMAND_TELEMETRY` instead of a local `concat!(...)` string.
- The telemetry default contract path moved to
  `COMMAND_TELEMETRY_DEFAULT_CONTRACT_PATH`, so runtime behavior and help text
  share the same command-surface fact.
- The implicit trace-source help detail moved to
  `COMMAND_TELEMETRY_DEFAULT_TRACE_SOURCE`, keeping telemetry-specific detail
  behind the same Command Surface seam.
- Unit tests lock `telemetry_usage()` to every `COMMAND_TELEMETRY.usage_lines`
  entry and assert both telemetry default details remain visible.

## Twenty-sixth hardening changeset evidence

The twenty-sixth implementation slice removes the next local CLI usage Adapter
from the cost help path:

- `cli_util::cost_usage()` now renders from `COMMAND_COST` instead of relying
  on a local `COST_USAGE_LINE` in the cost command module.
- The cost command help path now uses the same Command Surface projection
  formatter as graph, eval, and telemetry while preserving the existing usage
  shape and JSON/text contract.
- Unit tests lock `cost_usage()` to every `COMMAND_COST.usage_lines` entry and
  assert `[--json|--no-json]` remains visible.

## Twenty-seventh hardening changeset evidence

The twenty-seventh implementation slice removes the next local CLI usage
Adapter from the risk-audit help path and reconciles a parser/help drift:

- `cli_util::risk_audit_usage()` now renders from `COMMAND_RISK_AUDIT` instead
  of a local `RISK_AUDIT_USAGE_LINE` in the risk-audit command module.
- The risk-audit parser now accepts explicit `--no-json` as the text-mode
  counterpart to `--json`, matching the Command Surface `[--json|--no-json]`
  contract.
- Unit tests lock `risk_audit_usage()` to every
  `COMMAND_RISK_AUDIT.usage_lines` entry and prove `--no-json` is treated as
  an explicit text-mode flag rather than an unknown argument.

## Twenty-eighth hardening changeset evidence

The twenty-eighth implementation slice removes the next local CLI usage
Adapter from the eval-harness help path:

- `cli_util::eval_harness_usage()` now renders from `COMMAND_EVAL_HARNESS`
  instead of a local `EVAL_HARNESS_USAGE_LINE` in the eval-harness command
  module.
- `forge-core eval-harness --help` now uses the same Command Surface projection
  formatter as graph, eval, telemetry, cost, and risk-audit while preserving the
  existing `usage:` help shape and JSON/text contract.
- Unit tests lock `eval_harness_usage()` to every
  `COMMAND_EVAL_HARNESS.usage_lines` entry and assert `[--json|--no-json]`
  remains visible.

## Twenty-ninth hardening changeset evidence

The twenty-ninth implementation slice deepens the Command Surface seam for the
host-adapter policy/admission/projection/manifest adapter:

- `cli_util::command_surface_usage(&CommandSpec)` is now a reusable projection
  helper, so command-specific adapters can render the canonical `usage:` shape
  without growing one helper per command.
- `host_adapter_policy_cmd.rs` now renders command-specific usage from
  `COMMAND_HOST_ADAPTER_DISTRIBUTION_POLICY`,
  `COMMAND_HOST_ADAPTER_ADMIT_DISTRIBUTION`,
  `COMMAND_HOST_ADAPTER_PROCESS_POLICY`,
  `COMMAND_HOST_ADAPTER_ADMIT_INVOCATION`, `COMMAND_HOST_ADAPTER_PROJECTION`,
  and `COMMAND_HOST_ADAPTER_MANIFEST` instead of falling back to global
  `forge-core --help` output.
- The same adapter now accepts explicit `--no-json` wherever the Command
  Surface advertises `[--json|--no-json]`, preserving parser/help alignment.
- Unit tests lock the host-adapter policy family to every projected
  `CommandSpec::usage_lines` entry and assert command-specific usage on
  required-input and unknown-flag errors.

## Thirtieth hardening changeset evidence

The thirtieth implementation slice deepens the Command Surface seam for the
host-adapter verification adapter:

- `host_adapter_verify_cmd.rs` now renders command-specific usage from the
  corresponding `COMMAND_HOST_ADAPTER_VERIFY_*` `CommandSpec` instead of
  falling back to global `forge-core --help` output.
- All host-adapter verification help paths now use the reusable
  `cli_util::command_surface_usage(&CommandSpec)` projection helper, keeping
  the command meaning in `forge-core-command-surface` and the verification
  adapter as a thin parser/executor.
- The same adapter now accepts explicit `--no-json` wherever the Command
  Surface advertises `[--json|--no-json]`, and numeric parse errors report the
  command-specific usage rather than the global command list.
- Unit tests lock every host-adapter verification command to its projected
  `CommandSpec::usage_lines`, assert command-specific required-input errors,
  assert command-specific numeric parse errors, and prove explicit `--no-json`
  reaches artifact verification instead of being treated as a usage error.

## Thirty-first hardening changeset evidence

The thirty-first implementation slice tightens the Stage 5 first-use proof for
the Forge core Bootstrap Core Exception:

- `start_cmd::tests::bootstrap_core_exception_is_diagnosed_without_consumer_link`
  now compares the reported `StartPayload.project` against
  `ProjectContext::from(resolve_project(root, true))`, proving `start` mirrors
  `project resolve --allow-bootstrap-core` instead of carrying a parallel
  project-context implementation.
- The same regression still proves consumer safety: the exception path is only
  reached for a Forge core-shaped root, and arbitrary local `.forge-method/`
  state remains `no_link`.
- Live smoke output from this repository now shows both
  `forge-core project resolve --root . --allow-bootstrap-core --json` and
  `forge-core start --root . --json` reporting `bootstrap_core_local` with the
  same state root, while `start` preserves an explicit reference back to the
  allowed resolve command.

## Thirty-second hardening changeset evidence

The thirty-second implementation slice closes an MCP policy-downgrade gap in
the Command Surface seam:

- Explicit `mcp-allowlist.yaml` loading now treats the shared
  `CommandAuthority` as an authority floor: a command whose authority may mutate
  cannot be admitted with `policy: read-only`.
- Unsafe declarations are accumulated as
  `DiagnosticCode::McpAllowlistUnsafeReadOnlyPolicy` diagnostics and the
  offending tool is dropped, preserving the project's validation style while
  keeping the MCP adapter fail-closed.
- Regression coverage proves both `execute-operation` (mutating authority) and
  `claim` (mixed authority at the top-level MCPTool seam) are rejected when an
  explicit allowlist tries to downgrade them to read-only, while independent
  safe entries continue to load.
- Command Surface tests now reject any default read-only MCP projection whose
  command authority is not `ReadOnly`; this moved the mixed top-level `memory`
  MCPTool out of the read-only default and into mutate-gated exposure until a
  future subcommand-level MCPTool seam exists.
- `CONTEXT.md` and ADR-0006 now state the rule in product language: the
  Allowlist is capability data, not a way to weaken the Command Surface,
  MutateGate, or Tool-Call Attestation.

## Thirty-third hardening changeset evidence

The thirty-third implementation slice removes the next shallow help adapter
from the M1 read-only command family:

- `preview`, `ready`, and `explain` help now render command-specific usage from
  the shared Command Surface instead of falling back to the global CLI usage
  table.
- Missing-value and unknown-argument usage errors in the M1 parser now report
  the relevant command surface (`forge-core preview`, `forge-core ready`, or
  `forge-core explain`), improving locality for agents and humans.
- The M1 parser now accepts explicit `--no-json` as the text-mode counterpart
  to `--json`, matching the Command Surface `[--json|--no-json]` contract used
  by CLI help, generated docs, and MCP descriptors.
- Unit coverage locks the three M1 commands to their projected Command Surface
  lines and proves the unknown `explain` flag no longer falls back to unrelated
  global or sibling-command usage.

## Thirty-fourth hardening changeset evidence

The thirty-fourth implementation slice removes the next shallow help adapter
from the effect-target metadata index command family:

- `rebuild-effect-index` and `query-effect-index` help now render
  command-specific usage from the shared Command Surface instead of falling
  back to the global CLI usage table.
- Missing-value, unknown-argument, and invalid-value usage errors in the
  effect-index parser now report the relevant command surface, preserving
  locality for the mutating rebuild adapter and the MCP default read-only query
  adapter.
- Both commands now accept explicit `--no-json` as the text-mode counterpart to
  `--json`, matching the generated Command Surface `[--json|--no-json]`
  contract.
- Unit coverage locks both commands to their projected Command Surface lines
  and proves sibling command usage does not leak into command-specific error
  messages.

## Thirty-fifth hardening changeset evidence

The thirty-fifth implementation slice removes the shallow help adapter from
the main mutating operation surface:

- `execute-operation` help now renders command-specific usage from the shared
  Command Surface instead of falling back to the global CLI usage table.
- Missing `--operation`, missing flag values, malformed `--payload`, invalid
  `--max-payload-bytes`, and unknown-argument usage errors now report the
  `execute-operation` command surface, preserving locality at the most
  important mutating CLI Adapter.
- `execute-operation` now accepts explicit `--no-json` as the text-mode
  counterpart to `--json`, matching the generated Command Surface
  `[--json|--no-json]` contract and the MCP descriptor projection.
- Unit coverage locks the command to its projected Command Surface line and
  proves unrelated command usage does not leak into mutating-path error
  messages.

## Thirty-sixth hardening changeset evidence

The thirty-sixth implementation slice removes the shallow help adapter from
the repository validation surface:

- `validate` help renders command-specific usage from the shared Command
  Surface instead of falling back to the global CLI usage table.
- Missing `--root` values and unknown-argument usage errors report the
  `validate` command surface, keeping the local acceptance gate easy to
  diagnose without unrelated command noise.
- `validate` accepts explicit `--no-json` as the text-mode counterpart to
  `--json`, matching the generated Command Surface `[--json|--no-json]`
  contract.
- Unit coverage locks the command to its projected Command Surface line and
  proves unrelated command usage does not leak into validation-path error
  messages.

## Thirty-seventh hardening changeset evidence

The thirty-seventh implementation slice closes the remaining shallow usage
paths in the standalone risk-audit gate:

- `risk-audit` already projected its help text from the shared Command Surface,
  but missing `--root` values, missing `--rules` values, and unknown arguments
  still fell back to the global CLI usage table.
- The parser now reports `risk-audit` command-specific usage for those malformed
  argv paths, matching the same single-source-of-truth pattern used by the
  other Stage 4 parser adapters.
- Unit coverage proves each malformed argv path reports the projected
  `risk-audit` surface and does not leak unrelated command usage into the
  standalone gate diagnostics.

Remaining Stage 4 work:

- Extend the typed parser adapter pattern only to high-value shallow parsers
  after `start` remains green under full gates.
- Audit the next high-value parser/help seam before migrating another command.

## Research base

### Rust and CLI codebases

- Cargo's CLI builds a structured `clap::Command` and carries a `verify_cli`
  test with `debug_assert()`, a useful pattern for making parser/help drift a
  test failure:
  <https://github.com/rust-lang/cargo/blob/master/src/bin/cargo/cli.rs>.
- ripgrep keeps a deep `Args` module that converts low-level argument matches
  into a high-level, cloneable configuration object. The lesson for Forge is
  that parsing should collapse into a small, typed interface before runtime
  behavior starts:
  <https://github.com/BurntSushi/ripgrep/blob/041544853c86dde91c49983e5ddd0aa799bd2831/crates/core/args.rs>.
- clap's own documentation shows `CommandFactory::command().debug_assert()` as
  the canonical test hook for validating command definitions:
  <https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html>.
- Cargo documents `cargo metadata --format-version=1` as the machine-readable
  workspace/package interface. Forge docs that describe the workspace shape
  must use this instead of hand-maintained crate counts:
  <https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>.
- The Cargo Book's CI guidance starts from build-and-test workflows. Forge must
  extend that baseline with explicit platform coverage because this failure is
  Windows-only:
  <https://doc.rust-lang.org/cargo/guide/continuous-integration.html>.
- GitHub Actions documents matrix jobs across operating systems. Forge should
  use an OS matrix so `cfg(windows)` code is compiled before release:
  <https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/run-job-variations>.

### Agent systems and provenance research

Western and Eastern research converge on the same architectural direction:
agent systems become trustworthy through typed process evidence, command/data
separation, and auditable provenance rather than by relying on model intent.

- The 2026 evidence-tracing survey argues that final-answer accuracy is not
  enough; agent behavior needs typed execution provenance spanning evidence,
  tool calls, memory, observations, actions, and recovery:
  <https://arxiv.org/html/2606.04990v2>.
- PROV-AGENT extends W3C PROV and incorporates MCP concepts for near-real-time
  agentic workflow provenance. This supports Forge's choice to keep MCP as a
  thin adapter over a canonical command/runtime surface:
  <https://arxiv.org/html/2508.02866v1>.
- "Trustworthy Agentic AI Requires Deterministic Architectural Boundaries"
  argues that high-stakes agents need deterministic mediation,
  command-data separation, and unforgeable provenance. This reinforces Forge's
  fail-closed adapter and kernel design:
  <https://doi.org/10.48550/arxiv.2602.09947>.
- MetaGPT shows that structured workflows/SOPs improve multi-agent software
  engineering coherence over naive agent chaining:
  <https://arxiv.org/html/2308.00352v7>.
- A-MEM and G-Memory show the Eastern research trend toward structured,
  evolving, provenance-aware memory for agents and multi-agent systems:
  <https://arxiv.org/abs/2502.12110> and
  <https://arxiv.org/pdf/2506.07398>.

## Design principles for the remediation

1. **One source of truth per interface.** If humans, agents, MCP, help text,
   and docs need the same command fact, that fact must live in one registry and
   be projected outward.
2. **No big-bang parser rewrite.** The current hand-rolled parsers encode many
   command-specific invariants. Replace drift first, then deepen parsers
   command-by-command.
3. **Platform behavior is product behavior.** Windows-only code must be
   compiled and tested in CI, not treated as a local afterthought.
4. **Documentation is generated where counts drift.** Crate lists and workspace
   topology must be produced from `cargo metadata --format-version=1`.
5. **Adapters stay thin.** MCP and future host adapters must project the
   canonical command surface. They must not own command semantics.
6. **Bootstrap exceptions must be explicit.** The core repo's local
   `.forge-method/` state is a documented exception. CLI output must make that
   exception visible when applicable instead of looking like a consumer repo
   failure.
7. **Every stage has an acceptance gate.** A stage is done only when the
   relevant command, test, generated file, or CI workflow proves it.

## Stage 1 — Repair the red Windows test gate

### Problem

The MCP server test module uses `File::write_all` in a Windows-only helper, but
the `Write` trait is not in scope under `#[cfg(windows)]`. Ubuntu CI misses the
failure because that helper is not compiled there.

### Safe fix

Add a scoped import inside the Windows helper:

```rust
#[cfg(windows)]
fn make_fake_forge_core(success: bool, envelope: &str) -> PathBuf {
    use std::io::Write;
    // ...
}
```

### Acceptance

- `cargo test -p forge-core-protocol-mcp --all-targets`
- `cargo test --workspace --no-fail-fast`
- `cargo clippy --workspace --all-targets -- -D clippy::pedantic`

## Stage 2 — Add Windows CI coverage

### Problem

The workflow currently runs on Ubuntu only. That lets Windows-only code fail in
local use even when CI is green.

### Safe fix

Split CI into:

- a Linux quality job for format, clippy, contract validation, and regression
  anchor;
- a platform test matrix for `ubuntu-latest` and `windows-latest` that runs:
  - `cargo check --workspace --all-targets`,
  - `cargo test --workspace`.

Keep `fail-fast: false` so platform-specific failures are all visible.

### Acceptance

- The workflow YAML contains a matrix with `ubuntu-latest` and
  `windows-latest`.
- Windows matrix job runs both required Cargo commands.
- Linux quality job preserves the current strict clippy and validation gates.

## Stage 3 — Generate workspace layout from Cargo metadata

### Problem

Hand-maintained crate counts and crate lists already drifted. This damages user
trust and agent navigation.

### Safe fix

Add a small script that reads:

```bash
cargo metadata --format-version=1 --no-deps
```

and writes a generated Markdown fragment, for example:

```text
docs/generated/workspace-layout.md
```

The fragment should include:

- workspace member count,
- package name,
- relative crate path,
- target kinds,
- direct workspace dependencies.

Then update README and AGENTS to point to the generated fragment instead of
embedding stale counts. If a short inline summary is needed, the generator
should rewrite that bounded region.

### Acceptance

- Running the generator on a clean tree is idempotent.
- The generated member count equals `cargo metadata`'s workspace member count.
- CI runs the generator in check mode and fails if generated docs are stale.

## Stage 4 — Deepen the command surface seam

### Problem

`command_registry::COMMANDS` is already a useful module, but it does not yet
fully prevent drift. Usage lines, command parser behavior, subcommand help,
MCP allowlists, and future docs can still disagree.

### Target module

Introduce or evolve a deep **Command Surface** module:

- **Interface**: one canonical registry of command paths, usage metadata,
  authority class, adapter exposure, JSON/text support, and a parser/handler
  adapter.
- **Implementation**: hand-rolled parsers can remain behind the interface
  initially; the interface should not force an immediate clap rewrite.
- **Adapters**:
  - CLI dispatch,
  - global help rendering,
  - MCP tool projection,
  - docs generation,
  - command-surface tests.

This follows Cargo's structured command builder and ripgrep's "parse once into
a high-level object" pattern while preserving Forge's existing error-envelope
discipline and hand-rolled error enums.

### Incremental path

1. Add command metadata fields to `CommandSpec` without changing handlers:
   `authority`, `json_mode`, `adapter_visibility`, and `canonical_usage`.
2. Add tests that compare:
   - global usage vs per-command help,
   - MCP default tools vs command registry,
   - documented command paths vs registry.
3. Move one small command (`start`) to a typed parser adapter as a pilot.
4. Repeat for high-value shallow parsers only after the pilot is green.

### Acceptance

- No command usage line is hand-written in more than one authoritative place.
- `forge-core --help`, MCP projection, and docs are all projections.
- A command rename breaks tests before it reaches users.
- The CLI remains backward compatible unless an intentional breaking change is
  documented.

## Stage 5 — Reconcile `start` with `project resolve --allow-bootstrap-core`

### Problem

The core repository is a Bootstrap Core Exception. `project resolve --root .
--allow-bootstrap-core --json` reports that correctly, but `start --root .`
without the flag reports `no_link`. That behavior protects consumer repos, but
the product experience is confusing inside the core repo.

### Safe fix

Make `start` internally attempt a diagnostic-only bootstrap-core resolution
after a missing-link result, but only when all of these are true:

- `<root>/.forge-method/` exists,
- `<root>/Cargo.toml` identifies the package/workspace as Forge core,
- normal resolution failed only because the Project Link is missing.

If those conditions hold, return an explicit Bootstrap Core Exception payload
instead of a consumer `no_link` payload. The next step should explain that
ordinary consumer repos should still run `project init`, while the core repo
can continue with `--allow-bootstrap-core`.

This preserves consumer safety: `start` must not silently create or accept
local `.forge-method/` state for arbitrary repos.

### Acceptance

- `forge-core start --root . --json` in this repository returns a payload that
  names the Bootstrap Core Exception.
- A fresh consumer repo with no link still returns `no_link` and recommends
  `project init`.
- A consumer repo with unsafe local `.forge-method/` state still fails closed
  or points at explicit repair; it must not be normalized as safe.
- `start` local help and `command_registry` project `[--allow-bootstrap-core]`
  from the shared Command Surface and do not advertise unsupported flags.

## Stage 6 — Preserve product essence while raising the score

The path to 10/10 is not "more features"; it is fewer inconsistent surfaces.

### Must preserve

- CLI envelope wire compatibility.
- No `anyhow` / no `thiserror`.
- Accumulating validation diagnostics.
- Sidecar-owned state for consumer repos.
- Thin adapters and kernel-owned mutation.
- Existing contracts and workflow catalog.

### Must improve

- Cross-platform gates.
- Generated docs for live workspace shape.
- Command surface locality and leverage.
- Bootstrap diagnostics.
- Host adapter conformance proof before claiming product readiness.

## Completion definition

This plan is complete only when:

1. All current green-loop commands pass locally.
2. CI covers both Linux and Windows for all targets/tests.
3. README/AGENTS workspace layout cannot drift without a generator check
   failing.
4. Command surface metadata drives CLI help, MCP projection, and generated
   command docs.
5. `start` gives safe, explicit bootstrap diagnostics for both consumer repos
   and the Forge core Bootstrap Core Exception.
6. The release/product docs describe only capabilities proven by tests or
   command output.
