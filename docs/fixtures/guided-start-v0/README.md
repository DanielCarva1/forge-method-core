# guided-start-v0 — Reference scenarios for `forge-core start`

These scenarios document the five [`BootstrapState`](../../../crates/forge-core-cli/src/start_cmd.rs)s
that `forge-core start` diagnoses, and the concrete next step each emits. They
are **reference material for agents and humans**, not test fixtures loaded by
code — the authoritative behavioural checks live in
`crates/forge-core-cli/tests/start_cli_e2e.rs` and the unit tests in
`start_cmd.rs`.

## What `start` is

`forge-core start` is a **read-only, idempotent diagnostic** (F12 Guided
Start). It inspects the real state of a Consumer Project Repo and emits a
[`CliEnvelope`](../../../crates/forge-core-contracts/src/envelope.rs) payload
describing where the project is on the bootstrap path and what the concrete
next step is. It never executes effects and never creates files — it composes
with `project init` (which it *recommends*, never invokes) and with `guide`
(to which it *hands off* once prerequisites exist).

The agent — not the CLI — is the interface between human and product. `start`
emits a payload; the agent decides the action.

## The five bootstrap states

Each state implies one concrete next step. `start` recomputes from the real
project on every call, so re-running after an advance jumps to the correct
state.

| # | state | signal | next step |
|---|---|---|---|
| 1 | `no_link` | no `.forge-method.yaml` | `forge-core project init` |
| 2 | `link_present_no_sidecar` | link parses, state root missing | `forge-core project resolve` (diagnose) |
| 3 | `sidecar_ready_no_contract` | state tree healthy, no spec yet | author a minimal operation contract |
| 4 | `contract_present` | at least one spec exists | `forge-core guide describe` |
| 5 | `preview_run` | a preview has run | `forge-core guide describe` (terminal) |

## Scenario walkthroughs

### Scenario A — empty repo (`no_link`)

A directory with no Forge Project Link. `start` cannot resolve anything and
emits a non-zero exit (`env_config`, code 5) with no payload — there is no
project context to report. The agent surfaces `forge-core project init` as
the obvious first action.

### Scenario B — broken sidecar (`link_present_no_sidecar`)

A `.forge-method.yaml` exists and parses, but the sidecar/state tree it
points at does not (deleted, never created, or wrong path). `start` reports
the state and recommends `forge-core project resolve` to diagnose before
proceeding. This is a *recovery* state, not a happy path.

### Scenario C — ready, no spec yet (`sidecar_ready_no_contract`)

The happy-path first run: state tree is healthy but nothing has been
authored on top of it. `start` points at two hand-picked **starter fixtures**
(not the whole directory) so a new agent has an obvious entry point:

- `docs/fixtures/operation-contract-v0/observe-project-status.yaml` — the
  simplest read-only shape (`autonomy.mode: observe`).
- `docs/fixtures/operation-contract-v0/execute-trivial-write.yaml` — the
  simplest write shape (shows `authority.mutation_policy`).

`start` does **not** generate the spec — the authority boundary is the
validated contract, not a template. The agent (with the human) authors it,
then validates with `forge-core preview --operation <path>`.

### Scenario D — contract present (`contract_present`)

At least one operation-contract-looking file exists. `start` declares its
bootstrap job done and hands off to `guide`: `forge-core guide describe`,
then `guide status --phase discovery` (the first phase), and reminds to
validate the contract with `preview --operation`. `start` does not recommend
workflows or phases — that is `guide`'s job.

### Scenario E — preview run (`preview_run`)

A preview has already been produced (the sidecar `traces/` dir is
non-empty). This is the **terminal** bootstrap state: the project is
onboarded. `start` points at `guide describe` + `guide status --phase
<current>` for ongoing orientation and is explicit that it has nothing more
to add. Re-running `start` keeps reporting `preview_run`.

## Anti-script-de-novela (G1)

`start` is parametric, not prescriptive: it adapts its output to the
project's real state and never dictates spec content. The
`sidecar_ready_no_contract` payload points at canonical reference scenarios
and the validation command, but the agent (with the human) decides *what*
to specify. See `progress/g1_policies_script_novela_audit.md` for the G1
discipline this honours.
