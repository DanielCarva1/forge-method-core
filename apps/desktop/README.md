# Forge desktop shell

Independent Tauri application. Desktop `0.1.33` is the current source version
and [published alpha](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.33-alpha.1).
Its installer was downloaded without authentication, hash-checked, installed
over `0.1.32` and tested in the hidden native app.
It does not require Codex Desktop. A Codex CLI adapter supports
conversation in an explicitly confirmed project. The native identity check alone
is not an agent connection.

For session recovery, read the latest checkpoint below and the
[selective orchestration agreement](../../docs/agents/orchestration.md).

Approved references, visual rules and remaining illustration gaps are maintained
in [design/README.md](design/README.md). The conversation shell uses those rules
without fabricating previews or project progress.

## Run and test

From the repository root (Rust and Windows WebView2 prerequisites required):

```powershell
cargo run --manifest-path apps/desktop/src-tauri/Cargo.toml
node --test apps/desktop/tests/connection.test.mjs
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
```

Optional browser checks use an existing Playwright installation (no automatic
downloads): set `PLAYWRIGHT_MODULE` to its module directory if it is not on the
Node resolution path, then run `node apps/desktop/tests/browser.cjs`. This starts
a temporary loopback-only test server and closes it afterward. Browser test
doubles do not prove native IPC or an agent connection.

On Windows, set `FORGE_DESKTOP_EXE` to the built development executable and run
`node apps/desktop/tests/native.cjs` to check the actual WebView-to-Rust identity
call. This test enables loopback WebView debugging only for its child process,
then closes the app and removes its temporary WebView profile.
The normal application does not enable a debugging port.
Use `apps/desktop/tests/native-hidden.ps1` with the same environment variables
when the test must run on an isolated Windows desktop without taking focus.

The app has an independent Cargo workspace and lockfile so desktop dependencies
do not expand core builds. Static frontend assets need no npm install, dev
server, CDN or runtime download. Packaging uses the exact Tauri CLI and Codex
CLI versions in `apps/desktop/package-lock.json`. Codex CLI is a native Windows
x64 runtime resource, not a frontend dependency, and increases installer size.
`npm ci` fetches it during the build; installed app startup does not download it.
For the current Windows installer, `npm run build:nsis` also stages the pinned
released `forge-core` executable after checking both archive and binary hashes.
The release build uses `src-tauri/tauri.bundle.conf.json` to include it without
requiring that download for ordinary `cargo check` or debug builds.
This is not a commitment against using a frontend framework when warranted.

To repeat the Windows NSIS build from the repository with the same Tauri CLI:

```powershell
Set-Location apps/desktop
npm ci
npm run build:nsis
```

The installer is emitted under the configured Cargo target directory at
`release/bundle/nsis/Forge_<version>_x64-setup.exe`. Set `CARGO_TARGET_DIR` when
the workspace should use a dedicated build cache. The command builds a local
candidate only; it does not tag, sign, upload or publish anything. This pins the
build entry point, not the installer bytes: two consecutive unsigned NSIS builds
currently produce different hashes, so each candidate must be hashed after it is
built and the published candidate must be the exact artifact later verified.

## Boundaries

- `app_info` is an identity read with no project access.
- Rust exposes `inspect_project` as a read-only call to `forge-core project
  resolve`. The visible **Continuar nesta pasta** action calls `start_project`,
  which delegates to the core's `start` command and validates the resulting
  project through the same resolver. The core remains the link/state owner;
  the UI never infers progress from its compatibility phase field.
- Project selection accepts a local folder path through the native Windows
  folder picker or the text field. Selection alone is read-only; confirmation
  opens a healthy Forge project or prepares an existing unlinked folder,
  whether empty or already containing work. The core creates the Forge Project
  Link and sibling sidecar when needed, but refuses to recreate damaged linked
  state or use a nonexistent folder. Failed lookups hide earlier results. The **Meus projetos**
  screen keeps up to eight local shortcuts after successful confirmation, in
  `forge.projects.v1` localStorage. Each shortcut is checked again through
  `inspect_project` before use; the list is not project state or a discovery of
  every Forge project on the machine. Removing a shortcut changes only this list.
  If local storage fails, opening a project still works, but the shortcut may
  not survive restart.
- The 0.1.12 Windows bundle prefers its pinned `forge-core 0.13.2` executable.
  An absolute `FORGE_CORE_EXE` host override takes precedence for development;
  an older source build without the bundled file falls back to the executable
  under `%LOCALAPPDATA%/Programs/forge-core/bin/forge-core.exe`. The app does
  not replace global installations. The webview cannot choose commands or
  executables. No PATH search occurs inside the selected project.
- Each CLI invocation has a 15-second child timeout inside a 20-second bounded
  read/retry operation, hides the subprocess console, terminates unfinished
  children, and does not expose stderr. Ordinary read responses are limited to
  64 KiB; the on-demand read-only workflow report has a larger explicit limit.
- No shell/filesystem plugins, network listener, credentials or project database.
- Forge retains project-state ownership; Codex retains conversation history.
- Preview content must never share privileged application IPC access.
- The approved C direction uses lavender, deep plum, coral and editorial art.
  This shell establishes layout and typography, not the final illustration assets.
  The application icon uses approved direction C: coral/lavender petals on a plum
  rounded tile. `src-tauri/icons/source.png` is the retained raster master;
  `src-tauri/icons/export.ps1` exports the PNG and multi-resolution Windows ICO
  (16, 20, 24, 32, 48, 64, 128 and 256 pixels) using Windows System.Drawing.
  The built-in image tool produced the isolated asset from the approved C board,
  then removed the baked checkerboard to obtain actual alpha transparency.
  Prompt intent: preserve direction C, isolate one icon, no wordmark or labels,
  transparent corners. This icon change does not publish an installer.
- Responsive styling does not establish mobile agent execution or remote access.
- Documentation is English; the initial user-facing UI is Portuguese.

Work tracking: GitHub issues #81, #82 and #83. Full agent conversation is #85.

For the optional real-project check, set `FORGE_TEST_PROJECT` to the exact absolute
path of an existing linked project before running `tests/native.cjs`. This checks
the native resolver, displayed root, invalid/unlinked folders and stale identity
removal. It does not start an agent, initialize state or prove conversation readiness.

## Codex conversation

For the `0.1.11` source, the Windows adapter prefers Codex CLI `0.157.1`
bundled from the pinned npm package. It falls back to an installed Codex
Desktop or per-user npm executable only if the bundled file is absent.
`FORGE_CODEX_EXE` remains a development override and takes precedence; webview
content cannot set it. The app does not modify global CLI installations. Sign
in through the Codex CLI login flow; a ChatGPT account is required. API-key
login, ZCode, in-app login and automatic Codex updates are not implemented.
In published 0.1.11, `forge-core` remains separate; source 0.1.12 bundles a
pinned released core for its installer. WebView2 and account credentials are
not bundled. A genuinely clean-machine installation remains unproven.

Earlier integration used standalone Codex CLI 0.154.0. The older machine CLI
0.144.6 authenticated but the provider rejected its use of gpt-6-astra with
an explicit upgrade requirement. The `0.1.11` installed candidate instead
sent and restored a real response with bundled 0.157.1 and no host override.
The UI still translates upgrade failures without exposing raw provider errors;
pinning a version does not guarantee future model/client compatibility.

Rust starts `codex app-server --listen stdio://`, initializes it once, reads only
the account type for admission and starts one thread bound to the resolved root.
The returned root is checked again. No credentials or raw protocol errors are
forwarded to the webview. Curated events drive streaming text and turn status;
the webview receives no general RPC/shell API. The installed start-forge skill is
requested through agent instructions, not reimplemented in the UI. This is
guidance, not evidence that every agent action complies with the protocol.

As requested by the maintainer, the agent uses approvalPolicy=never and
sandbox=danger-full-access. This is NOT filesystem isolation: Codex tools can
affect files outside the selected project. The UI warns about file changes, and
the project binding prevents accidental routing, not malicious-agent containment.
The local desktop user and configured CLI are trusted. Do not embed untrusted
preview content in this privileged webview. Unsupported interactive RPC requests
receive explicit errors, never silent approval.

Requests time out after 30 seconds (90 seconds for cold initialization);
individual protocol lines are limited to 16 MiB. There is no turn-duration
timeout: users may interrupt long tasks. Only
one turn is admitted at a time. Disconnect/normal window close requests an active
turn interruption for up to three seconds and stops the owned app-server. This
does not roll back completed file changes or guarantee cleanup after an OS crash.
The session slot is not held while awaiting provider RPCs. Cleanup is serialized
separately so repeated close requests cannot bypass an in-progress shutdown.

Codex owns saved history. The UI stores only a thread bookmark, scoped to the
resolved project ID and canonical folder, in `forge.conversation.v1` localStorage
keys. Reconnect reads the stored thread summary and validates its folder and ID
before resuming; the returned transcript is validated again and rendered as text.
After project confirmation, the first **Enviar** connects/resumes before sending;
if connection fails, the draft remains in the composer and no turn is sent.
**Abrir conversa** remains available to view a saved transcript before writing.
Interrupted/failed-turn answers are marked incomplete. No turn is sent by resume.
Users can explicitly start another conversation without deleting Codex history.
A failed resume never silently falls back to a new thread. No conversation list,
credentials, decisions or Forge state are copied into this bookmark store. The
read-only conversation picker queries Codex's own on-device index by the exact
confirmed project folder and does not maintain a second history database.

A failed bookmark write keeps the latest reference in memory for reconnects and
warns that restarting the app may reveal the older saved conversation. It does
not claim persistence. Resume requests `excludeTurns: true`, then reads full turns
in ascending pages of one turn. This stable protocol was confirmed in the Codex
0.154.0 generated schema. Recovery is bounded to 4,096 pages, 128 MiB of serialized
page data and 60 seconds; the per-frame limit is 16 MiB. A single
oversized turn still fails visibly without truncation; users can continue through
Codex CLI. Malformed/partial pages, duplicate turn IDs and repeated cursors are
rejected without returning a partial transcript. Older servers that ignore the
flag and return nonempty bounded full history retain the direct projection path;
unsupported requests fail explicitly rather than silently starting another thread.
This is not unrestricted recovery or installed-app verification.
Codex may not persist an empty thread until
its first turn; unavailable empty references receive the same explicit recovery.
Provider subscriptions/usage limits still apply.
Responsive desktop layout does not provide remote mobile connectivity (#96).

Opt-in real-agent test: additionally set `FORGE_TEST_AGENT=1` before running the
native test. It consumes model usage, asks a read-only project question, verifies
streamed deltas, interrupts a second turn, sends another message, and disconnects.
It then reloads the WebView and restores the transcript through a fresh Codex
transport without submitting another turn. Full installed-app restart/upgrade
verification belongs to the package journey, not this reload check.
It creates a Codex conversation; no fake response is used. Without this flag,
agent execution is explicitly NOT_RUN. Browser doubles test event-ordering and
error recovery only, never authentication or actual agent behavior.

For a cheaper real-agent connectivity check, set `FORGE_TEST_AGENT_SMOKE=1`
instead of `FORGE_TEST_AGENT=1`. It sends one short no-tools message in a
disposable newly initialized project, requires a real assistant reply, then
reloads the WebView and checks transcript restoration without resending.
It consumes model usage and does not test interruption or a full app restart.

Protocol reference: https://developers.openai.com/codex/app-server/

## Recorded work (#86)

The manual Last Forge Record panel calls `inspect_progress`, resolving project
identity before a bounded `workflow resume` query. It reuses the existing CLI
query boundary: 15 seconds and 64 KiB per command, hidden child and cleanup.
Two sequential commands mean at most two query timeouts; there is no interval,
background resume refresh or new project-state store. `workflow report` is not
cheaper: it calls the same backend resume observer. The observer captures project
state, so automatic refresh on every conversation event is deliberately avoided.

The adapter validates project identity, summary/context versions and read-only
authority before exposing a narrow work record. Unknown versions fail visibly
rather than inventing compatibility. The observed v10 summary can contain an
old accepted focus marked current; therefore the UI labels it recorded work,
not live progress, and labels its timestamp as retrieval time. It reports open
decision count without pretending to provide a decision form. Backend strings
are rendered as plain text, not translated or interpreted as instructions.

Project changes and agent turn/disconnection events invalidate displayed or
pending readbacks. Failures hide earlier results but never disable conversation.
Full workflow stage/decision interaction remains separate work. Package release
scope and readiness are tracked in GitHub issue #97, not per-commit version bumps.

## Session checkpoint — 2026-09-15

Work paused at the maintainer's request. This checkpoint is a resumption aid,
not another plan or a release-readiness claim. Package scope remains in
[#97](https://github.com/DanielCarva1/forge-method-core/issues/97#issuecomment-5674332707);
the latest recovery evidence is in
[#93](https://github.com/DanielCarva1/forge-method-core/issues/93#issuecomment-5674461395).

### Source and delivered slices

- Checkout: `D:\Forge-method-core`, branch `codex/desktop-shell`.
- Latest implementation: `be00ce31`, pushed to origin. Worktree was clean before
  this documentation update. No desktop release has been published.
- Core remains `0.13.3`; independent desktop development version is `0.1.0`.
- `6ddf54de`: approved conversation visual direction.
- `9f9dcbbc`: persistent light/dark/system and contrast preferences.
- `9a9ec87d`: readable status icons and keyboard-focus recovery.
- `0d9cbd9a`: manual, bounded Last Forge Record panel. It is a recorded work
  snapshot, not a claim about the agent's live activity; no background polling.
- `be00ce31`: reopen Codex-owned conversation history with a project-scoped UI
  bookmark, folder/thread validation, incomplete-answer labels and explicit
  new-conversation choice. Resume never sends another turn automatically.

### Verification completed

- PASS: 10 desktop Rust tests and 7 Node unit tests.
- PASS: focused browser suite including mobile overflow, enlarged text,
  keyboard focus, appearance persistence, progress invalidation, transcript
  restoration, explicit new conversation and bookmark-write failure recovery.
- PASS: real Tauri window with authenticated standalone Codex CLI `0.154.0`:
  streamed reply, interruption, subsequent turn, disconnect, WebView reload
  and restored transcript through a fresh Codex process without a new turn.
- PASS: final rebuilt native smoke for project resolution and preferences.
  Model execution was not repeated for the final presentation adjustments.
- PASS: separate native/standards and UI/spec reviews; findings addressed.
- NOT_RUN: installed-app process restart, installation/upgrade journey and
  published-artifact verification. WebView reload is not a full app restart.
- No CI runs were returned for this branch at the final check. No core-wide
  test suite or core release was triggered for these desktop-only changes.

### Resume here, without repeating finished work

1. Read #93 and the accepted Package 1 scope in #97. Inspect current status
   and source before editing; use `eng` and `ask-matt`.
2. Resolve bounded recovery for long conversations. The current transport
   rejects frames over 1 MiB; full-history resume can exceed it. An explicit
   non-destructive CLI fallback is implemented, but pagination is not. Inspect
   the supported installed Codex protocol before choosing an implementation;
   do not simply remove bounds or duplicate history in Forge.
3. Continue #97: package Windows + Codex, verify full close/reopen and upgrade
   preservation using the actual package, then publish a coherent update.
   Tauri currently has `bundle.active=false`. The existing core release workflow
   binds `v*` tags to the core version; do not use it blindly for desktop `0.1.0`.
4. Preserve source ownership: Codex owns conversation history; Forge owns
   project state; UI storage contains preferences, conversation references and
   recently confirmed project shortcuts.
   If bookmark writes fail, same-session reconnect uses the in-memory reference;
   after app restart an older saved reference can remain, as the UI warns.

### Fast local verification and paths

```powershell
$env:CARGO_TARGET_DIR='D:\forge-method-core-build-cache\main-target'
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked -j2
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --offline --locked -j2
node --test apps/desktop/tests/connection.test.mjs apps/desktop/tests/conversation-reference.test.mjs
$env:PLAYWRIGHT_MODULE='C:\Users\User\.cache\codex-runtimes\codex-primary-runtime\dependencies\node\node_modules\playwright'
node apps/desktop/tests/browser.cjs
$env:FORGE_DESKTOP_EXE='D:\forge-method-core-build-cache\main-target\debug\forge-desktop.exe'
$env:TEMP='D:\Temp\User'
$env:FORGE_TEST_PROJECT='D:\Forge-method-core'
node apps/desktop/tests/native.cjs
```

Real-model testing is opt-in (`FORGE_TEST_AGENT=1`) and consumes subscription
usage. The verified CLI override was
`D:\Temp\User\forge-cli-0154-probe\package\vendor\x86_64-pc-windows-msvc\bin\codex.exe`
via `FORGE_CODEX_EXE`; recheck this temporary path before use. Default npm CLI
`0.144.6` previously failed because it did not support the selected Astra model.
No global CLI replacement was made. Reuse build/browser caches; avoid downloads
or broad builds because disk space and time are constrained.

Approved art references are under
`D:\Forge-method-core\apps\desktop\design\references`; the native development
executable is the `FORGE_DESKTOP_EXE` path above, not a published installer.

### Working agreement for the next session

Use simple Portuguese in conversation and English in documentation. Keep changes
small and auditable, but report meaningful package progress rather than requesting
approval after every commit. Publish working packages, not every small slice.
Never add AI authorship/co-authorship. The maintainer explicitly authorizes
delegating suitable narrow tasks to cheaper available models such as Luna;
use Spark only if actually available. Keep coordination and integration with the
main agent, pass compact task-local context, and avoid redundant reviews or
large test runs. No new agent work is needed while this session is paused.

## Session checkpoint — 2026-09-19

### Accepted working agreement and current slice

The maintainer resumed work and accepted selective model delegation, with the
parent responsible for routing, rerouting, integration, verification and honest
economics. The maintainer wants one long conversation across context compactions,
not manual model switching or separate worker chats. The durable agreement is
`docs/agents/orchestration.md`, linked from root `AGENTS.md` for recovery.
This supersedes the previous checkpoint's pause; it does not change package scope.

The maintainer clarified that this is an alpha product: publish coherent,
verified building blocks rather than one release per story or waiting for the
entire planned product. Installation/update work remains required. The focused
Rust test ladder and final pre-merge/package checks are recorded in
`docs/agents/orchestration.md`; desktop-only iteration uses its own Cargo workspace.

The completed atomic activity is documentation and a bounded read-only metering
probe. Long-history recovery implementation has NOT started in this slice.
No app code, global Codex configuration, Forge runtime state, commit, push or
publication was changed. Core/app versions and the earlier checkpoint's test
results remain historical; they were not revalidated here.

### Changed files and durable recovery

- `AGENTS.md`: mandatory recovery pointer after compaction/context reset.
- `docs/agents/orchestration.md`: accepted responsibility, routing hypotheses,
  quality gates, cost boundaries, bounded pilot and recovery procedure.
- `docs/research/orchestration-usage-2026-09-19.json`: sanitized, timestamped usage
  observation, not raw conversations, credentials, a bill or a savings claim.
- This README: latest session checkpoint and continuation pointer.
- User-authorized memory note (outside Git):
  `C:\Users\User\.codex\memories\extensions\ad_hoc\notes\2026-09-19T180800-forge-orchestration-agreement.md`.
  This is a retrieval pointer; do not assume a memory index has already ingested it.

At inspection, source was `codex/desktop-shell`, HEAD `92011132`, initially clean.
These documentation/evidence edits are intentionally uncommitted. Preserve them.
Read current `git status` rather than assuming this snapshot remains current.

### Metering findings and limitations

PASS: local JSONL logs expose `token_usage_record` with per-response `usage`,
per-turn cumulative `turn_token_usage`, and cumulative `thread_token_usage`.
Observed fields: input, cached input, cache-write input, output, reasoning output,
and total tokens. In this bounded sample, total equals input plus output; do not
add cached input or reasoning output again. Per-response sums reconciled with
thread totals in all three inspected logs, with no duplicate response IDs across
them. This is an observed local format, not a guaranteed future host contract.

PASS: `turn_context` identified parent `gpt-6-astra` and both inspected children
as `gpt-5.6-luna`; usage events do not themselves contain model names. Mixed-model
threads will require chronological attribution, not a last-model label.
`root_turn_id` separated the current documentation/probe slice from the earlier
research child. The evidence JSON retains per-slice and whole-thread values
separately; never sum cumulative snapshots or attribute earlier discussion to
implementation work. Repeated input on distinct responses still counts.

The parent turn was still running at the evidence cutoff. Its counts are PARTIAL:
later verification, documentation and final-response usage are not included.
Do not report the observation as the final cost of this slice. An initial worker
snapshot was superseded by parent readback after the worker finished; use the
saved evidence cutoff, not preliminary chat totals. No compaction was observed
in the inspected records; recovery from a real reset has not been exercised.

Account allowance at 2026-09-19 21:06:42 UTC: Pro, 1% used in a 10,080-minute
window. This is rounded and shared account-wide; it is not a per-task cost.
API-equivalent BRL cost: NOT_RUN (tariff/FX inputs not established in this slice).
Actual per-task Pro BRL attribution: UNKNOWN. Savings versus Astra-only or Sol-only:
NOT_MEASURED. No comparison arm has been run and no economic advantage is claimed.

### Delegation and verification evidence

- One bounded Luna/medium read-only worker, `usage_probe`, completed; no active
  worker remains from this slice. Parent owns all edits and final verification.
- Worker thread: `01a0bb7d-f5c9-75a1-8aee-4dd752fa9137`; parent:
  `01a0bb60-4f1b-7541-932b-779226f93ba6`. Prior research child:
  `01a0bb67-df87-7fd0-a532-e1530a64a972` (excluded from current slice accounting).
- Read-only verification used PowerShell `Get-Content | ConvertFrom-Json`,
  filtering `type=token_usage_record`, summing per-response counters with
  `Measure-Object -Sum`, and checking response IDs with `Group-Object`.
  Exact local source paths, numeric observations and cutoff are in the JSON.
- Documentation verification: `git diff --check`, root-to-agreement/checkpoint
  pointer checks, evidence JSON parsing/reconciliation, and memory-note readback.
- NOT_RUN: app tests/build/native smoke, real compaction recovery, protocol
  investigation, Forge runtime activation and pricing/FX conversion. No app
  behavior or authoritative Forge progression is claimed by these edits.

### Exact next step (do not repeat this discovery)

1. Read this checkpoint and the orchestration agreement; inspect Git status.
2. Close the current pilot's accounting on a later turn using its saved
   `root_turn_id=01a0bb7d-6674-7420-8dd3-e54aef4957cc`, after its final usage is
   available. Include parent and probe, exclude earlier research. Establish dated
   official model tariffs, counter billing semantics and a sourced USD/BRL rate
   before computing any explicitly labeled API-equivalent BRL estimate. Keep
   this bounded; unavailable billing evidence must not trigger endless discovery.
3. Resume the accepted desktop work from #93/#97 and the 2026-09-15 checkpoint:
   activate Forge once as applicable, inspect the installed Codex history protocol
   and existing implementation, and pass `/eng` before code changes. Decide one
   coherent implementation slice and its effort limit before worker dispatch.
   Do not split coupled transport/state work artificially to use cheaper models.
4. Keep history ownership in Codex and bounds explicit. Installation/upgrade
   verification follows long-history recovery; no publication without approval.

The working agreement is now durable; automatic reload after a future compaction
cannot be guaranteed. Root instructions and these explicit pointers provide the
recovery path without requiring the human to reconstruct the agreement.

## Development checkpoint — 2026-09-19: bounded long-history recovery

The first #93 implementation slice is integrated locally, uncommitted. It does
not close #93 or constitute an installed alpha package. Next: native long-history
recovery verification, then the accepted #97 Windows installation/update package.
Do not repeat the schema discovery or the earlier documentation-only pilot.

### Changes and evidence

- `src-tauri/src/agent.rs`: lean resume, bounded ascending full-turn pagination,
  identity checks before/after resume, explicit compatibility/limit failures,
  duplicate-turn/cursor guards and deterministic adapter contract tests.
- `src-tauri/src/history.rs`: shared full-history projection; explicit partial
  `itemsView` is rejected in both legacy and paginated paths.
- `src-tauri/src/codex_transport.rs`: unchanged 1 MiB bound, named sanitized
  rejection and actual LinesCodec multi-frame regression test.
- PASS: affected-crate `cargo check`; focused agent/history/transport tests;
  full `forge-desktop` crate **19 passed**, repeated in canonical checkout after
  integration; formatting check and `git diff --check`.
- PASS: **7 Node tests** and existing `tests/browser.cjs` checks (mocked IPC),
  including restoration, interruption display, bookmarks, accessibility and
  no silent new-thread fallback. UI contract unchanged.
- The >1 MiB recovery test uses a fake Protocol at the adapter boundary. A
  separate real codec test accepts multiple bounded frames above 1 MiB total.
  These are NOT an OS-process stdio integration test or native WebView proof.
- NOT_RUN: new live Codex long-history/native restart evidence, installed upgrade,
  full core workspace, clippy, release build and publication. Prior live CLI
  evidence in the older checkpoint is not evidence for this new implementation.

Commands use `--manifest-path apps/desktop/src-tauri/Cargo.toml -p forge-desktop
--offline --locked -j2`, with `CARGO_TARGET_DIR=D:\forge-method-core-build-cache\main-target`.
Focused filters: `agent::tests`, `history::tests`, `codex_transport::tests`.
Browser dependency: `PLAYWRIGHT_MODULE=C:\Users\User\.cache\codex-runtimes\codex-primary-runtime\dependencies\node\node_modules\playwright`.

### Governance and recovery

- Installed Forge **0.12.1** successfully started/resumed Solo Cooperative;
  source remains **0.13.3**. No executable upgrade was performed.
- Current Work `focus.desktop-long-history` superseded the old core-work focus
  through public `current-work prepare/update`; it remains active pending native
  verification. No story or whole-workflow completion is claimed.
- Public Quick Cycle checkpoint readback shows four stage closeouts; validation/
  delivery remains open. One rejected checkpoint attempt changed the immutable
  compactness reason; retry preserved its original value and succeeded.
- Isolation `desktop-long-history`, branch `codex/desktop-long-history`, worktree
  `D:\.forge-worktrees\desktop-history\long-history`; linked claim
  `claim.story.desktop-long-history.desktop-long-history`.
- Governed promotion applied only the three Rust files, preserving all earlier
  documentation/evidence changes. Canonical/worktree hashes match; receipt
  `sha256:59f058f2027788c3d58b6509ce5c3542ca42868b9c41a39d7d2404d43b0ad269`
  has `readback_verified=true`. Four unknown assurance claims were carried,
  not relabeled as verified. No Git commit, push, merge or publication occurred.
- Diagnostic artifacts outside the repo: `D:\Temp\User\forge-desktop-history-apply.json`
  and `forge-desktop-history-promotion-final.json`. Stable protocol schemas:
  `D:\Temp\User\forge-cli-0154-schema-stable-20260919-183128-255\v2`.
  Temp artifacts may expire; schema authority is the exact CLI generator.

### Orchestration / economics

One reused worker `history_protocol`, requested **Sol/medium**, performed protocol
investigation and the coupled Rust implementation. Parent owned continuity,
schema/implementation gates, review, integration, browser tests and canonical
readback. No worker remains active. Review required corrections for active-state
preservation, cautious error wording, smaller pages, partial item views and
duplicate turns; two test expectations/fixtures were corrected. Count this work
and all parent coordination when measuring the slice, not just final passing tests.
No causal savings or BRL cost is established. The earlier usage JSON is for the
documentation/probe pilot, NOT this development slice. Current parent usage remains
incomplete until the turn ends. A real context compaction occurred and the durable
agreement/checkpoint were reread; perfect automatic recovery is not promised.

### Exact next step

Verify recovery through actual Codex stdio and native UI using a controlled long
conversation, including interruption/reopen and compatibility failure, without
replaying tools or treating unit tests as native evidence. Keep the existing
single-turn/aggregate/time limits explicit. Then build the coherent Windows alpha
package (#97) and test installation/update preserving history references and
preferences. Publication still requires human authorization; no per-story release.

## Validation checkpoint — 2026-09-19: native process restart

**PARTIAL overall; PASS for native recovery with a deterministic external stdio
fixture.** The earlier short live-Codex interruption/reload evidence was not
repeated. No product code changed in this validation slice.

The current source was built with the focused desktop `cargo build` command
above (PASS, 15.61s). Development executable SHA-256:
`c28e10e4c02e6f5185df3656808b13027b61e07d6168d695cb41fa7fc15d1841`.
The real, unchanged Tauri app, native WebView, Rust adapter and stdio transport
were exercised against a **fake Codex executable**, not a provider connection.

- PASS: 3 full ascending pages, 6 exact rendered bodies, **1,200,150 bytes** total;
  roles, order, SHA-256 hashes and the final incomplete-answer marker verified.
- PASS: graceful WM_CLOSE of the actual `Tauri Window|Forge`, exit code 0,
  followed by a fresh process with the same WebView profile and saved bookmark.
  App PIDs were 43368 then 24852. Both launches restored identical body hashes.
- PASS: each stdio trace shows lean resume and three `limit:1, itemsView:full,
  sortDirection:asc` requests. No `thread/start`, `turn/start`, or replay request.
  The persisted interrupted status was simulated; no new live interruption was
  generated. No owned test processes remained after cleanup.
- The first harness sent WM_CLOSE to both the main window and Tao's event-target
  window and required forced cleanup. Targeting only the actual main window
  fixed the harness; the corrected run needed no forced termination. This was
  not a product patch or evidence of a confirmed product-close defect.

Durable sanitized result:
`docs/research/desktop-native-history-2026-09-19.json`, SHA-256
`80b760052f97bcf3088782e4226778e99bebf3a73658a2ac1a7a30e72a3840e0`.
Temporary reproduction files (may expire):
`D:\Temp\User\forge-native-fake-history-20260919\native-long-history.cjs`,
`fake-codex.cs`, `wmclose.cs`, and `fake-requests.ndjson`.
Harness environment: `FORGE_DESKTOP_EXE` points to the built executable and
`PLAYWRIGHT_MODULE` uses the cached module documented above. The harness owns an
isolated WebView profile; do not substitute a personal profile.

### Real Codex boundary — still NOT_RUN

Real CLI 0.154.0 found the synthetic persisted conversations but returned no
turns. Two worker attempts were followed by two parent, source-informed format
corrections (`user_message.kind` and `item_completed` events); none established
a valid turn-bearing fixture. Isolated account read returned no ChatGPT account.
Personal credentials and personal session files were not accessed or copied.
An initial resume probe attempted a Responses websocket and received 401; it
sent no `turn/start` and produced no generated output. Subsequent probes used
dead-loopback proxies and did not resume or start turns. No model-cost savings
or paid-usage verdict is inferred from these observations.

The two initial worker attempts are retained in
`docs/research/desktop-real-codex-fixture-attempts-2026-09-19.json`; the two parent
attempts used temporary roots ending in `20260919-c` and `20260919-d`.
Fixture investigation used the upstream Codex
[rollout helpers](https://github.com/openai/codex/blob/main/codex-rs/app-server/tests/common/rollout.rs),
[thread-read tests](https://github.com/openai/codex/blob/main/codex-rs/app-server/tests/suite/v2/thread_read.rs)
and protocol definitions. Current upstream source is not proof of this installed
binary's persisted-format compatibility.

Next missing acceptance evidence is specifically **real Codex long-history
recovery**, not another mock/native restart run. Use a known-valid controlled
Codex history or an upstream-valid fixture/authentication route; do not continue
guessing rollout formats or generate megabytes of paid responses. #93 remains
open; installed-package and upgrade evidence for #97 remain outstanding.
One reused Sol worker handled the harness; parent handled build, evidence review,
two unsuccessful fixture corrections and this checkpoint. No active worker,
commit, push or publication remains from this slice.


## Completed-stage usage measurement — 2026-09-19

Sanitized evidence: `docs/research/orchestration-development-usage-2026-09-19.json`.
This supersedes the pilot-only limitation for these two completed turns, not
the earlier pilot artifact itself. Includes parent and Sol implementation,
review and failed attempts attributed to each turn.

| Stage | Astra total tokens | Sol total tokens | Combined |
| --- | ---: | ---: | ---: |
| Development | 5,716,409 | 5,422,699 | 11,139,108 |
| Native validation | 4,892,376 | 3,147,241 | 8,039,617 |

Combined: 19,178,725 processed tokens across 200 responses; 19,105,163 input,
including 18,716,288 cached input, plus 73,562 output. Cached and reasoning
counts must not be added again. These are repeated response inputs, not unique
conversation size. Current measurement work, earlier pilot/research, unrelated
stages and RAM diagnosis are excluded; this is not whole-project accounting.

**BRL cost, Pro allowance attribution and comparative savings remain UNKNOWN.**
Parent Astra exceeds Sol in raw total tokens; this is a warning to reduce
coordination overhead, not evidence of a price comparison. Use compact worker
context, one coherent executor, bounded investigations and risk-focused review.
No Astra-only/Sol-only control was run; do not claim equivalent quality or savings.

Continuation: a read-only existing-chat inventory using CLI 0.154.0 timed out
before initialize responded (20 seconds); no thread was selected or resumed.
Diagnose initialization with captured stderr before retrying. Real Codex
long-history acceptance remains NOT_RUN; do not repeat synthetic-format guesses.


## Real conversation recovery recheck — 2026-09-20

**PASS.** The unchanged debug app and authenticated standalone Codex CLI 0.154.0
were exercised through the native WebView with an isolated profile. A populated
conversation received a reply, interrupted a second reply, completed a subsequent
reply, disconnected, reloaded the WebView, and resumed without resending a turn.
Both expected messages were present before and after reload; restored transcript
length was 194 characters. No project files or Forge records were changed by the
conversation prompts.

An earlier empty-conversation attempt was not a valid persistence test: an empty
thread is not evidence for recovery of a persisted populated conversation. Its
failure does not establish a regression and is superseded by this populated
conversation result. Intermittent standalone initialization timeouts were also
not reproduced as a deterministic product defect; no configuration workaround
or timeout increase is justified.

The full `tests/native.cjs` command first stopped in the unrelated Last Forge
Record readback before reaching its agent section. The agent/recovery section was
then isolated and passed. Existing focused Rust/Node tests and the deterministic
long-history native fixture remain the evidence for bounded pagination. Issue #93
is behaviorally verified but remains unpublished; proceed through the coherent
alpha packaging/publication work in #97 rather than creating more history logic.


## Windows alpha package checkpoint — 2026-09-20

**PASS with release limitations.** `tauri.conf.json` now owns a Windows x64
NSIS current-user bundle. Downgrades are refused, WebView2 uses the standard
download bootstrapper when needed, and the existing app identifier is unchanged.
No updater plugin, parallel release engine, embedded Codex binary or new state
store was added. Codex CLI remains an external authenticated prerequisite.

The initial proof used the official Tauri CLI 2.11.0 from a temporary tool
folder. The same exact build-tool version is now pinned by
`apps/desktop/package.json` and `package-lock.json`, so `npm ci` followed by
`npm run build:nsis` repeats the repository-owned package path without a
global CLI. The initial CLI built the 0.1.0 package, which installed under
`%LOCALAPPDATA%\Forge`. The
installed app opened this linked project, connected authenticated Codex 0.154.0,
received a real short reply, and stored dark theme, enhanced contrast and the
conversation bookmark. A second NSIS package used a temporary configuration
overlay for version 0.1.1; it installed over 0.1.0 at the same location. After a
full process restart, Windows reported 0.1.1, both preferences remained, the same
bookmark remained and the populated conversation resumed without a new turn.
The owned test installation then uninstalled cleanly (exit 0, install directory
and uninstall registration absent). Source version remains 0.1.0.

Evidence: `docs/research/desktop-windows-package-2026-09-20.json`. Package hashes:

- 0.1.0: `15FEE75969CECBE34F0E39286582BEE6C354C624CC27B24CDE2C70372B2AA273`
- 0.1.1 test overlay: `8B9688C3CA93FBDD3F46B7598D2C20A825D2E620BD3D95E56E706AB61E668DB7`

Verification: focused `cargo check` PASS; desktop Rust tests **19/19 PASS**;
connection Node tests **5/5 PASS**; release build, install, installed real-Codex
journey, upgrade recovery and uninstall PASS. The full native test first stopped
in the separate Last Forge Record readback before reaching its agent section;
the package journey isolated and passed the installed conversation path.

Limitations: both local installers are **unsigned**, unpublished, Windows x64
only, and not downloadable releases. Update currently means installing the newer
NSIS package; no in-app automatic updater exists. Do not publish or call the alpha
released until the maintainer accepts the package/release content and the release
path verifies the downloadable artifact.

The repository-owned build entry point was then verified with a clean `npm ci`
and pinned `tauri-cli 2.11.0`. Two consecutive 0.1.0 builds both succeeded, but
their SHA-256 values differed (`86E971...2331` and `BAFC6C...38A7`), as did their
sizes. Therefore the process is repeatable but the NSIS output is not currently
bit-for-bit reproducible. The exact candidate hash must be recorded only after
the final build; this limitation blocks any claim that an independently rebuilt
installer is byte-identical, not the already-passed install/update behavior.

## UI checkpoint — 2026-09-21

The accepted focus is now the product's approachable desktop screens. The existing
Home and conversation views are joined by **Explorar** (eight approved illustrated
themes, search, and editable conversation starters) and **Meus projetos** (up to
eight recently confirmed local shortcuts). This is UI on top of the existing
project resolver, not a new Forge project registry. The latter revalidates every
shortcut when opened and handles unavailable projects, removal, empty state and
storage failures. No native folder picker exists yet: the first opening of a
project still requires its folder path.

Browser checks passed for routing, category search, shortcut persistence and
revalidation, unavailable project, removal, storage failure, narrow layout,
keyboard, appearance and existing conversation behavior. Node connection tests
5/5 passed; `git diff --check` passed. These checks use a browser double for
native calls and do not establish a newly installed app. No Rust code was changed
in this UI slice. Current Work `focus.desktop-ui-screens` was updated through the
public Forge command. Next smallest step: visual folder selection for a first
project, then project/progress presentation. No commit, push or publication.

## UI package continuation — 2026-09-21

The project screen now has **Escolher pasta…**, which invokes the official
native Tauri folder dialog. The selected local path is displayed and passed
through the existing `inspect_project` validation; cancelling or a dialog
error does not replace a confirmed project. Folder selection is unavailable
while a Codex conversation is connected. The text path field remains as an
alternative. This adds no project registry or filesystem access capability to
the WebView.

Verification for the accumulated desktop package: browser UI checks PASS
(including selected path, cancellation, error, keyboard order and connection
lock); Node connection tests 5/5 PASS; desktop Rust tests 19/19 PASS; desktop
`cargo check`, formatting, strict Clippy and `git diff --check` PASS. The
repository-owned Windows x64 NSIS release build PASS, and a fresh native
WebView smoke PASS for screen access, app identity and persisted appearance.
The native OS folder-dialog **selection itself is NOT_RUN in automated UI
testing**; the browser test uses a native-call double, while Rust compilation
and the release build cover plugin registration. The installer remains local,
unsigned and unpublished. Next UI slice: friendly project/progress presentation.

## Update delivery ledger — 2026-09-21

**Code is not an update download.** Commit `7262fdf1` was pushed to
`codex/desktop-shell`; no release tag or downloadable installer was published.
The local `0.1.0` NSIS build is a verification artifact, not an update for an
existing `0.1.0` installation. The desktop source version was `0.1.0` at the
time of this ledger; a later checkpoint records the `0.1.1` candidate work.

For the next coherent Windows alpha update (#97), retain this explicit ledger:

1. **DONE:** source package committed and pushed; local Windows x64 NSIS build,
   native WebView smoke and focused tests passed within the recorded limits.
2. **DONE for source delivery:** the UI slice and actual Windows folder
   selection were checked and committed/pushed as `92bf259d`. Preserve the
   intermittent native record lookup failure in the release limitations.
3. **DONE in source:** desktop `0.1.1` is set in Tauri, Cargo and Cargo.lock;
   draft release notes exist. This alone is not an available update.
4. **PARTIAL:** final checks, a replacement candidate with its exact hash and
   a silent `0.1.0` to `0.1.1` install-over test are recorded below. Installed
   runtime, project shortcut and preference continuity passed on an isolated
   invisible Windows desktop. Conversation continuity remains NOT_RUN.
5. **PENDING — maintainer publication decision:** present the release content,
   supported scope and limitations for acceptance. Only then publish the agreed
   GitHub release/installer and verify the downloaded file has the recorded hash
   and completes a short installed-app journey. A commit or branch push alone
   does not satisfy this step.

Until step 5, users cannot obtain this update from a release page. There is no
automatic in-app updater; updating the current alpha means installing the newer
NSIS package. Do not mark #97 delivered merely because a local installer exists.

## Project record presentation — 2026-09-21

The workspace now presents the confirmed project and the latest Forge record
before the technical connection details. The record has a separate, readable
status, recorded activity, recorded next step and open-decision callout; its
states are projections of `inspect_progress`, not agent-chat inference or a
live completion meter. Missing focus or unavailable data does not show a stale
record. No Rust query or domain contract was changed.

Browser checks PASS for current, stale, blocked, completed, abandoned, absent,
zero decisions, malformed/missing details, lookup failure, mobile width,
keyboard and existing conversation behavior. A development build PASS. The
native WebView read the actual linked project and displayed its record and
decision text in four subsequent runs; an initial run had timed out waiting
for a successful record, so native readback is **not claimed perfectly stable**.
Direct CLI `workflow resume --json` succeeded in roughly 5–8 seconds with an
approximately 56 KB response. The intermittent native failure was not
reproduced reliably enough to justify changing timeouts or query architecture.
An isolated native WebView run opened the actual Windows folder dialog; closing
that dialog returned "Seleção cancelada. Nenhum projeto foi alterado." in the
app. This proves native open/cancel, **not** folder selection: an attempted
keyboard selection did not complete, so selection remains NOT_RUN. The
one-off harness used a temporary WebView profile and did not install or publish
anything. Real Codex interaction was NOT_RUN in this UI-only slice. This work
remains uncommitted and unpublished; the update ledger above still applies.
Next smallest task: verify an actual Windows folder selection, then continue
the remaining UI screens before selecting a release candidate.

## Alpha 0.1.1 preparation — 2026-09-22

The native Windows folder dialog was driven through an isolated test app using
the real project path `D:\Forge-method-core`: the dialog displayed that folder,
returned the canonical path, and the app displayed **Projeto encontrado** and
the confirmed root. Replacing the field with a nonexistent folder hid the
earlier project and reported the invalid path. The test script's initial
over-escaped path was corrected before this PASS; it was a harness error, not
evidence of an app defect. The dialog cancellation proof remains separate.

Desktop source version is now `0.1.1` in Tauri and Cargo; Cargo.lock followed
through focused Cargo verification. The Codex client version now reads the
Cargo package version rather than retaining a hard-coded `0.1.0`. These source
changes are **not** an installed update or a published release. Draft content
and known limitations are in `RELEASE_NOTES-0.1.1.md`.

Visual review of Home, Explore, My Projects and the project/record/conversation
screen found no additional concrete layout defect to justify redesign. Browser
checks PASS; they use doubles for native calls. Focused desktop `cargo check`,
agent module tests (12), full desktop crate tests (19), Node tests (7),
formatting and strict Clippy PASS. Native `0.1.1` WebView smoke passed for
project lookup, record readback, invalid/unlinked folders and preferences, but
one earlier run of this package returned a generic record lookup failure. That
intermittent failure remains under diagnosis and is not represented as stable
PASS. Real Codex conversation was NOT_RUN in this source slice. The local
candidate and silent upgrade are recorded below. Source commit `92bf259d` was
pushed to `origin/codex/desktop-shell`; publication remains pending.

## Local installer candidate — 2026-09-22

The user requested **headless-only** further verification while using the
machine. Do not launch more visible native app tests or interactive installers
without a new arrangement. The earlier native folder selection PASS predates
this request; no Forge desktop process was left running afterward.

The pinned repository build command produced one unsigned Windows x64 NSIS
candidate: `D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.1_x64-setup.exe`,
4,468,745 bytes, SHA-256
`3D630CE98FDBCF794399EA422FB808EE9A2263B88649369067BF561E7B8CCB51`.
This exact file was not rebuilt or modified after hashing. `0.1.0` was silently
installed into an initially absent `%LOCALAPPDATA%\Forge`; the same candidate
then silently upgraded it. Both installer processes exited 0, Windows
uninstall registration reports `0.1.1`, the installed executable reports
product version `0.1.1`, and no Forge desktop process remained. The candidate
hash was unchanged after installation. This is a **PASS for silent package
upgrade/version readback**, not a full installed-app journey.

The first isolated-desktop launch exited because its one-off test helper closed
the Windows desktop handle too early; keeping that handle open corrected the
helper. The installed `0.1.1` app then launched on an invisible Windows desktop,
reported version `0.1.1`, resolved `D:\Forge-method-core`, read its record, and
retained theme, contrast and the My Projects shortcut after a full process
restart. A second controlled headless run set theme, contrast and the project
shortcut in installed `0.1.0`, silently installed the **same** hashed `0.1.1`
candidate over it, and confirmed all three survived the cross-version upgrade.
Opening the saved project shortcut again returned **Projeto encontrado** with
the same canonical root. No app window appeared on the user's current desktop;
the owned hidden processes were stopped, and local `0.1.1` remains installed.
This is a **PASS for installed runtime and preference/project shortcut upgrade
continuity**, not conversation continuity: no Codex conversation was run across
versions, so that remains **NOT_RUN**. The native development smoke also had an
intermittent generic record lookup failure; five direct CLI reads succeeded and
subsequent native smokes passed, but the cause is not established. Keep this as
a known alpha limitation, not a resolved bug. Publication awaits maintainer
acceptance of `RELEASE_NOTES-0.1.1.md` and downloadable-artifact verification.

## Read-conflict fix and replacement candidate — 2026-09-22

Headless reproduction isolated the earlier generic record error: 15 sequential
installed-app `inspect_progress` calls and 12 sequential UI consultations
passed, but paired calls could return one success and one failure. The same
paired `forge-core workflow resume --json` calls returned `conflict` from the
governance ledger or `rejected_by_gate` from claim-WAL quiescence. A narrower
probe also observed `rejected_by_gate` when the read-only replacement snapshot
changed during inspection. These are actual CLI envelopes, not a frontend
formatting or 64 KiB output-limit failure. The original single native smoke
failure was not traced at the time, so its exact trigger cannot be proven
retroactively; this reproduced the same user-visible error.

The desktop adapter now retries only the identified transient read conflicts,
within four attempts and a 20-second overall bound; permanent errors still
fail. Its record command also serializes its own in-process resume reads so
the app cannot contend with itself. Forge core and its locking contract were
not changed. The typed-envelope classification test passed, as did desktop
`cargo check`, all 20 desktop Rust tests, strict Clippy, seven Node tests,
browser smoke, five headless pairs of native app calls, four app-versus-CLI
contention rounds, invalid-root rejection, and installed UI record readback.
Direct concurrent `forge-core` CLI calls can still return conflict; this is a
desktop resilience fix, not a claim that core now serializes every reader.

The earlier candidate with SHA-256
`3D630CE98FDBCF794399EA422FB808EE9A2263B88649369067BF561E7B8CCB51`
is **superseded** and retained under the build cache's `nsis/superseded/`
directory, not for publication. The replacement candidate is
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.1_x64-setup.exe`,
4,478,683 bytes, SHA-256
`DC7429FB4FDE6190EDED7323B45F190D980D1E23A3EAB4CDCE040E6F88FA08C8`.
It installed silently over the prior local `0.1.1` binary (which changed),
then was tested again as an exact `0.1.0` to `0.1.1` silent upgrade. The `0.1.0`
baseline package in the current build cache hashed
`7BBA88E3B9D772A2F8D1EB383F06425954C2E1B0FC76C06467EAE2973C6D22F8`;
this is distinct from the earlier initial-package hash above. All three
installer operations exited 0. The final installed `0.1.1` app launched on an
invisible Windows desktop; dark theme, enhanced contrast, saved project,
project resolution and record readback passed. Three more pairs of installed
app record queries passed. No test app process remains. The replacement
candidate has not been published or download-verified, and a real Codex
conversation was not run across this exact upgrade.

## Project stage and accepted outcome presentation — 2026-09-23

The existing manual **Last Forge Record** consultation now also presents the
project lifecycle stage in plain Portuguese and the accepted Work Focus outcome.
Both values come from the same validated `workflow resume` response as the
existing activity and next-step fields. The desktop adapter requires matching
resume, journey-guidance and Work Focus phases before returning the projection;
it does not infer a stage from chat, create another progress store or add
polling. Unknown future phase labels use a neutral explanation instead of
inventing completion.

PASS: four focused Rust projection tests, desktop `cargo check`, browser UI
checks, seven Node tests, and a real native WebView readback on an invisible
Windows desktop. The native app resolved `D:\Forge-method-core`, displayed
**Descoberta**, and displayed the accepted desktop-UI outcome returned by Forge.
Visual review used a headless screenshot; no window appeared on the user's
current desktop and no Forge desktop process remained afterward.

This is an incremental presentation slice toward #92, not the complete decision
history or artifact preview in #91. The unpublished `0.1.1` installer candidate
above is still the exact artifact built from commit `92c608fe`; it does **not**
contain this later UI slice. Do not silently treat a source commit as a newer
download or rebuild the recorded candidate in place. Package these later UI
changes only in a subsequent explicitly versioned candidate.

## First project from the desktop — 2026-09-26

The user tried an unlinked folder from Explore and found that the shell could
only inspect existing Forge projects, leaving the idea composer unsendable.
Source now separates read-only **Conferir projeto** from explicit **Iniciar
projeto nesta pasta**. The latter delegates to `forge-core start`, validates
its returned project identity through `project resolve`, and then enables the
normal Codex connection. A selected Explore idea remains editable as a draft
before connection; **Enviar** stays disabled until Codex is connected. An
unlinked folder now receives a specific instruction instead of a generic
lookup error. Linked state loss is still rejected by the core; the UI does not
silently recreate it.

A separate cold-start problem appeared in the native check: the first Codex
app-server `initialize` timed out under the former 30-second request bound,
while a direct protocol probe completed after almost a minute; the immediate
repeat took about 2 seconds. Only the initialization request now permits 90
seconds; turn requests retain their 30-second bound.

PASS: desktop `cargo check`, focused project and transport tests, all 24
desktop Rust tests, seven Node unit tests, browser UI checks, and hidden-desktop
native smoke. The native smoke used a disposable folder, confirmed that
inspection did not initialize it, explicitly initialized it through the app,
verified the link and sibling sidecar, connected an authenticated Codex CLI,
and observed **Enviar** enabled. No message was sent to a real model in this
slice, so reply delivery remains **NOT_RUN**. The smoke used a process-local
`FORGE_CODEX_EXE` override to the authenticated Codex Desktop bundled CLI
`0.155.0-alpha.16`; this internal path can change with app updates. The
standard npm Codex CLI here remains `0.144.6` and previously failed with the
selected Astra model. No global CLI installation or configuration changed.

The source and debug executable now include this change. The installed
`0.1.1` app and the previously hashed, unpublished `0.1.1` installer do not.
No release candidate was replaced or published. Commit and push are pending.

## First-time UI audit and correction — 2026-09-26

At the maintainer's request, the source UI was walked as a new user and compared
with the approved boards. Findings and evidence provenance are in
`design/ux-audit-2026-09-26.md`. In particular, the folder picker previously
triggered an existing-project lookup immediately, so an unlinked new folder
failed before the user could choose to start a project. It now only selects the
path; starting or reopening is a separate explicit choice. Home leads with
discovering ideas; the workspace states the two setup steps in plain language,
gives the conversation more space and collapses secondary technical detail.

PASS: browser UI checks, visual screenshot review, fresh desktop debug build,
hidden native WebView smoke with an existing project and a disposable new one,
and authenticated Codex connection with Send enabled. The native folder dialog
itself and a real reply were NOT_RUN in this slice. The installed app and the
unpublished installer remain older; there is no newly published update. The
revised UI still needs a further human design review against the approved board.

A follow-up integration audit is recorded in `design/ux-audit-2026-09-26.md`.
The short real-agent native smoke passed: one assistant reply reached the UI
and both sides returned after WebView reload without another turn. This closes
the earlier **NOT_RUN** for basic reply delivery only; it does not establish
long-history, interruption or installer-upgrade coverage in this source slice.
The Tauri desktop surface still exposes only a narrow project/chat/read-only
record path, not every Forge core workflow capability.

## Project entry clarity — 2026-09-26

The workspace now asks whether the person is starting a new project or opening
one they already have. It shows only the matching action, explains what will
happen to the selected folder, and adjusts the project heading accordingly.
Explore and the main Create link select the new-project path; My Projects and
return-to-work links select the existing-project path. Recent-project shortcuts
explicitly use read-only reopening. The folder picker still selects only a
path; it does not initialize or inspect until the person confirms an action.

An interaction regression was caught by a browser check: a pending project
lookup could re-enable its controls through the conversation module. This was
fixed without changing the native project commands. Browser UI checks now pass
for both entry modes, shortcuts, keyboard access, responsive layout and the
pending-control case. A fresh debug build and hidden native WebView smoke pass
for real linked-project readback, explicit disposable project creation, Codex
connection readiness and appearance persistence. Browser screenshots are UI
evidence only; the native Windows folder dialog and a new real model reply
were **NOT_RUN** in this slice. The installed app and unpublished installer
still predate these source changes. No commit, push or publication was made.

## Conversation and record visual pass — 2026-09-26

The conversation now presents user and Codex messages as separate, labeled
bubbles with decorative avatars. The Forge mark is reused for agent messages;
no new artwork or generated response was introduced. The project context is
now a visible **Onde estamos** section with an explicit **Consultar registro**
button instead of being concealed in a disclosure. The key recorded stage,
outcome and next step remain visible after consultation; activity and decision
detail sit behind an optional disclosure to keep the context rail readable.
The readback remains manual and read-only, and every field continues to come
only from the existing validated Forge projection.

PASS: browser UI checks including resumed-message markup, keyboard traversal,
390px layout and enlarged mobile text; seven Node unit tests; fresh desktop
debug build; hidden native WebView smoke reading a real project record and
initializing a disposable new project. Browser visual captures used mocked
messages and record data and are **not** proof of native content. A new real
model turn and actual Windows folder-dialog selection were **NOT_RUN** in this
pass. The installed app and unpublished installer remain older; no commit,
push or release publication was performed.

## Compact project setup after confirmation — 2026-09-26

After `start_project` or `inspect_project` confirms a project, the workspace
collapses the folder-selection form and keeps the confirmed project, manual
Forge record and conversation visible. **Trocar de projeto** reopens the form;
the form remains open when validation fails. The header changes from onboarding
instructions to the current workspace, and keyboard focus moves to **Conectar
Codex** after a successful confirmation. Recent-project shortcuts still pass
through read-only validation. No project identity or record is inferred from a
saved shortcut or from conversation text.

PASS: browser UI checks for new/existing modes, successful collapse, reopening,
failed lookup, keyboard flow and responsive states; seven Node unit tests; a
fresh desktop debug build; and a hidden native WebView smoke confirming both
collapse and reopening around real linked-project readback, failed lookup,
disposable project initialization and Forge record consultation. A new real
Codex turn and the Windows folder dialog were **NOT_RUN** in this slice. The
source/debug build changed; the installed app and unpublished installer did
not. No commit, push or release publication was made.

Next smallest slice: review the connected conversation state against the
approved board and remove any redundant setup copy without changing native
project or Codex behavior; then test the actual Windows folder dialog separately.

## Connected conversation and native folder choice — 2026-09-26

The connected workspace now keeps the confirmed project and its read-only
Forge record visible, while hiding setup-only instructions and actions until
disconnect. Its empty conversation invites an actual first message instead of
saying “after connecting.” A return from My Projects no longer resets the
confirmed project heading to the pre-project question. No project, Codex, or
Forge core command changed in this slice.

**PASS:** focused browser UI regression; debug desktop build; hidden native
WebView smoke with a real Forge record, disposable project creation and
authenticated Codex connection. Visual inspection used a native screenshot
captured while connected. The optional Windows dialog check now uses
`tests/folder-dialog.py` (Python 3.12 with `pywinauto`) on the isolated desktop,
without switching to the user's visible desktop or sending global input. It
cancelled the dialog, then selected both a linked project and an existing
unlinked folder. Selection only filled the path; opening remained explicit.
The unlinked-folder check failed validation, hid the previous project and did
not create a Forge link. The isolated native run passed with
`FORGE_TEST_FOLDER_DIALOG=select` and `FORGE_TEST_NEW_CONNECT=1`.

**NOT_RUN in this slice:** a new real Codex message/reply, long-history and
interruption validation, installer upgrade and published-download verification.
Earlier real-reply evidence is recorded above and is not replaced by the
connected-but-empty check. The installed app and unpublished installer are
still older than this source/debug build. No commit, push or publication was
performed. Next smallest slice: review the updated connected screen with the
maintainer, then continue the remaining UI/backend coverage audit before
packaging the next approved alpha update.

## One-folder project entry — 2026-09-26

This checkpoint supersedes the earlier split **new/existing** entry wording.
The person chooses one existing folder and confirms **Continuar nesta pasta**.
`start_project` delegates to the existing `forge-core start` path: a healthy
Forge-linked project opens, while an empty or pre-existing unlinked folder
receives its Project Link and sibling sidecar. The chosen folder is the Codex
working directory when the person connects. Picking a folder alone still does
not write anything. Recent-project shortcuts remain read-only revalidation,
not an alternate bootstrap path. Missing folders and damaged linked authority
fail closed; a failed confirmation clears the previously shown project.

The setup remains available during a conversation as **Trocar de projeto**.
Opening it disconnects an idle Codex session; if a turn is running, the person
must first confirm interruption and possible retained file changes. This is a
project-switch safety step, not a requirement to manually disconnect before
choosing another project. The UI still has an explicit **Conectar Codex** step
after folder confirmation; it does not silently import arbitrary Codex chats.

**PASS:** browser UI regression, including one-folder entry, shortcut
revalidation, idle switch and declined running-turn switch; seven Node unit
tests; frontend syntax/diff checks; fresh desktop debug build. The hidden
native WebView smoke selected folders in the real Windows dialog, opened a
linked Forge project, prepared an existing unlinked folder without changing
its existing file, prepared an empty folder, rejected a missing folder without
showing stale identity, connected authenticated Codex to the confirmed folder,
and switched away from an idle connection. The native test ran on an isolated
desktop and did not take focus from the user's desktop.

**NOT_RUN in this slice:** a new Codex message/reply, interruption of a real
running turn, damaged-link rejection in the native UI, long-history recovery,
installer upgrade and published-download verification. Earlier real-reply
evidence above remains separate. The installed app and unpublished installer
are older than the current source/debug build. No commit, push or publication
was performed.

## Send-first conversation — 2026-09-26

This supersedes the preceding checkpoint's explicit connection prerequisite.
After confirming a folder, the person can type and press **Enviar**. The UI
opens or resumes the project-scoped Codex conversation first, then sends exactly
one turn. **Abrir conversa** remains an optional way to inspect prior messages
without sending. Connection failure keeps the draft and permits retry; it does
not submit a turn. The post-confirmation status now reflects the actual ready
project instead of incorrectly asking for a folder again.

**PASS:** browser UI regression including failed first connection, retained
draft and one send on retry; seven Node unit tests; JavaScript syntax and diff
checks; fresh desktop debug build. On an isolated Windows desktop, the native
WebView selected a linked and a pre-existing unlinked folder, prepared a fresh
folder, and the first **Enviar** opened authenticated Codex, received one real
reply, then restored both sides after WebView reload without resending. The
isolated test did not take focus from the user's desktop. Browser visual
captures were reviewed as UI/layout evidence only, not native IPC proof.

**Still open:** full visual/product coverage, real running-turn switch,
long-history and full-process restart/installer-upgrade checks. The installed
app and unpublished installer remain older. No commit, push or publication.

## Returning to an active conversation — 2026-09-26

Native visual review found a real navigation defect: returning through **Criar**
reopened the folder form over an already confirmed project. The workspace now
preserves its compact project state on ordinary return. **Abrir outro projeto**
from Meus projetos explicitly opens the folder form; if a Codex turn is active,
declining the interruption keeps the person on Meus projetos and leaves the
session untouched. The optional **Abrir conversa** button is visually secondary
to **Enviar**, which is the primary send-first action.

**PASS:** browser regression for both return and deliberate project switch,
including declined running-turn confirmation; a fresh debug build; and hidden
native WebView smoke checking ordinary return with a confirmed project. No
model message was sent in this final navigation smoke. The earlier real
send-first reply test remains separate evidence. Installed app, installer,
commit, push and publication are unchanged.

## Automatic project context — 2026-09-26

After a folder is confirmed, **Onde estamos** now reads the existing Forge
record automatically; the person no longer has to discover a separate button
before seeing the recorded objective, stage and next step. **Consultar registro**
remains available for a later refresh. A conversation event marks the displayed
record stale rather than implying that the Forge ledger changed in lockstep
with chat. Delayed responses from a previous project or an invalidated read are
discarded; the refresh control becomes usable again. No core state, schema or
Codex transport changed.

**PASS:** browser UI regression including automatic first read, project-switch
race and conversation invalidation; seven Node unit tests; JavaScript syntax
and diff checks; debug desktop build; hidden native WebView smoke showing the
real Forge record automatically on two subsequent runs. The native test did
not take focus from the user's desktop. No new real Codex turn was sent.

**PARTIAL:** the first native smoke after this change failed the automatic
record read. A direct `forge-core workflow resume --root D:\Forge-method-core
--json` succeeded immediately afterward, and two repeat native smokes passed.
The failure's native error code was not captured by the UI test, so its cause
and recurrence rate remain unknown. The UI shows an explicit non-success state
and offers **Consultar registro** for retry; the native smoke now exercises
that retry if the automatic attempt fails and logs the partial result rather
than silently claiming first-attempt success. Do not call the intermittent
behavior fixed based on the passing repeats.

**Next:** continue the remaining visual/product coverage, starting with the
actual decisions and project-stage presentation from read-only Forge authority;
do not invent decision text from counts or simulated agent activity. Preview,
full-process history, package upgrade and published-download checks remain
separate. No commit, push, installer or publication was performed. No worker
was routed in this slice; model-level usage and BRL cost are UNKNOWN because
attributable per-turn counters were not available here.

## Local project preview — 2026-09-26

The confirmed-project workspace now puts the conversation beside project
context and a **Veja o que foi criado** panel. A person can choose a local
file, refresh its preview, and prepare a change request in the existing
conversation without sending it automatically or discarding an existing draft.
The native command revalidates the Forge project and the file's resolved
location before reading. It shows bounded PNG/JPEG/GIF/WebP images and small
UTF-8 text files; HTML is displayed as literal text, not executed. Unsupported,
outside-project, unavailable and oversized files fail without changing them.
The panel explicitly says that a local preview is not publication proof.

**PASS:** focused Rust `cargo check -p forge-desktop`, three preview module
tests and 27 crate tests, desktop debug build, browser UI regression including
refresh, cancel, real image display, draft-preserving change request and error
states, JavaScript syntax, Rust formatting and diff checks. Hidden native
WebView smoke read and refreshed a disposable text fixture, showed an image,
rejected a file outside the project, and selected a file through the actual
Windows dialog. The native run used an isolated desktop and did not take focus
from the user's desktop. Browser mocks and screenshots are visual/interaction
evidence only, not native IPC proof.

**Limits:** this is a local file preview, not website execution, PDF viewing,
artifact discovery, published-state verification, or a complete implementation
of issue #91. The user still chooses a file manually. No new real Codex reply
was sent in this slice; earlier real-reply evidence is separate. The prior
intermittent project-record read remains unexplained, although the latest
native smoke passed. Full UI coverage, long-history recovery, installer
upgrade and published-download checks remain open. Installed app and installer
remain older than source/debug build. No commit, push or publication occurred.

**Next:** present actual recorded decisions and lifecycle stage in a clear,
read-only UI (#92), then finish remaining visual/product coverage. Keep
coordination/rework and model costs honest: no worker was routed in this slice;
attributable per-model token counters and BRL cost are UNKNOWN.

## Recorded direction and pending questions — 2026-09-26

The **Onde estamos** panel now reads more of the same authoritative, bounded
`workflow resume` response. It shows the current recorded objective and its
constraints as **Direção e combinados do projeto**, with the source marked as a
cooperative agent-carried Forge record, not independent proof that a human
approved it. Material supersession and non-material clarification are labeled
as revisions; only the current accepted text is displayed. The original text
is preserved rather than translated or summarized by a second UI-only source.
The panel separately presents the count of durable pending decisions and
questions calculated by the current evaluation. The latter are explicitly
marked as suggestions, not accepted agreements. Stage remains available even
when no current Work Focus exists. Dense details are expandable, while the
conversation stays the primary action.

**Objective and files:** continue toward a complete, welcoming desktop UI on
top of Forge's existing authority. This slice changed
`src-tauri/src/progress.rs`, `ui/progress.mjs`, `ui/index.html`, `ui/styles.css`,
`tests/browser.cjs`, `tests/native.cjs`, and this checkpoint/design guidance.
There are no active workers or delegated edits. The working tree also contains
earlier uncommitted desktop changes; they were not reset or overwritten.

**PASS:** `cargo check -p forge-desktop`, six focused progress tests, all 29
desktop-crate tests, browser UI regression for absent/pending/revised states,
source separation, literal HTML text and project-state handling, JavaScript
syntax, Rust formatting, diff checks, debug build and hidden native WebView
readback of the real accepted objective. The latest native run also repeated
project onboarding and local preview checks without taking focus from the
user's desktop. The browser screenshot was inspected for hierarchy; it uses a
mocked native response and is not readback evidence.

**PARTIAL for #92:** this shows the current accepted objective proposal, not
the full history of resolved decision records. The bounded resume summary
does not carry readable resolved-decision text; `workflow report` is a large
audit response (about 1.6 MB on this project), not suitable for the desktop's
bounded 64 KiB query. Do not reconstruct decision text from opaque IDs or
describe candidate questions as accepted. The real project currently has no
recovered pending decisions, so that state is covered by controlled UI and
Rust fixtures, not native project data. Current source/debug build is still
newer than the installed app and unpublished installer. No new Codex turn,
commit, push, installer or publication occurred in this slice.

**Next:** close the remaining decision-history gap with a bounded authoritative
read interface if the user journey needs historical resolved choices, then
continue full visual/product review against the approved art and real native
journeys. Full-process history and update-on-installed-app tests remain open.
No worker was routed; per-model token attribution and BRL cost are UNKNOWN.

## Preview-first workspace hierarchy — 2026-09-26

When a valid local file preview has loaded, the desktop workspace now places
**Veja o que foi criado** above the project record, beside the conversation.
Without a loaded preview, the project record remains first. Narrow screens
retain the original reading order: project, conversation, preview. Refresh and
file-dialog cancellation keep the successful preview position; switching
projects or a failed preview clears it. This is a layout adjustment toward the
approved conversation artwork, not a claim of full visual parity or complete UI.

**PASS:** browser interaction and responsive-layout checks; JavaScript syntax;
desktop debug build; hidden native WebView smoke, including the actual Windows
file dialog and an assertion that the loaded preview is above project context
at desktop width. The native run also confirmed project onboarding, bounded
workflow readback, local text/image preview, outside-project rejection and
appearance persistence. It did not take focus from the user's desktop.

**NOT_RUN:** a new real Codex conversation in this slice. Browser screenshots
use mocked native responses and are visual evidence only. The installed app
has not been updated. Full visual/product coverage, complete decision history,
long-history restart and package upgrade remain open. No commit, push,
installer or publication occurred. No worker was routed; per-model tokens and
BRL cost remain UNKNOWN.

**Next:** continue the visual and interaction review against the approved art
using the native app and controlled states, then close remaining UI gaps before
asking the maintainer for product feedback. Preserve the existing backend
authority and the separate alpha-release approval boundary.

## First-use navigation and idea safety — 2026-09-26

Following **Como funciona** now expands its existing help section and focuses
its summary, including when that URL is loaded directly. Selecting another
Explore theme replaces an untouched theme suggestion, but never silently
overwrites a person's own unsent draft. The workspace explains when it kept
that draft; the notice clears when the person edits or leaves the screen.
No project or conversation persistence changed.

**PASS:** browser regression for theme switching, handwritten-draft retention,
help navigation, direct URL and existing project/chat controls; JavaScript
syntax and diff checks; desktop debug build; hidden native WebView smoke for
the same first-use path, real project onboarding, recorded-work readback,
Windows file selection, preview and appearance persistence. The native test
ran without opening a window on the user's desktop. One browser-test retry
corrected a test prerequisite (the previous search filter had hidden the
next category); another corrected an asynchronous navigation assertion.

**NOT_RUN:** a new real Codex turn in this slice. This debug build is not the
installed app. Full UI/product coverage, richer conversation presentation,
complete resolved-decision history, long-history restart, installer upgrade
and publication remain open. No commit, push or release occurred. No worker
was routed; attributable per-model tokens and BRL cost are UNKNOWN.

**Next:** inspect and improve the readability of real Codex responses in the
conversation without adding a second conversation store or unsafe HTML
rendering. Preserve the original message text, streaming/resume behavior and
the existing project authority.

## Readable Codex responses — 2026-09-26

The conversation now formats completed Codex text into readable headings,
lists and code blocks, with limited inline emphasis/code. Live streamed deltas
and incomplete historical answers remain literal until completion. A single
**Ver texto original** control switches completed answers back to the exact
text received from Codex; it does not modify Codex history or Forge project
state. The formatter creates DOM nodes using `textContent`; agent HTML and
links are never executed. This is not a full Markdown implementation.

**PASS:** browser regression for delta-to-complete replacement, resumed and
incomplete messages, exact original-text toggle, literal malicious HTML and
unsafe link text, keyboard order, mobile width and code readability; screenshot
inspection in the dark theme; JavaScript syntax; desktop debug build; hidden
native WebView presentation fixture plus the existing project, preview, folder
dialog and appearance smoke. The native test ran without taking focus from the
user's desktop. The first native fixture attempt encountered an initial-page
navigation race; waiting for the home screen before evaluation resolved it.

**NOT_RUN:** a new real Codex reply in this slice. The browser's conversation
data and the native presentation fixture are controlled test content, not
provider evidence. Full Markdown parity, full UI/product coverage, resolved
decision history, long-history restart, installer upgrade and publication
remain open. The installed app is older than this debug build. No commit,
push or release occurred; no worker was routed. Per-model tokens and BRL cost
are UNKNOWN.

**Next:** exercise a substantial resumed conversation in the native UI for
readability and performance, then continue the remaining visual/product flows
against the approved art. Keep frontend presentation separate from Forge and
Codex authority.

## Long-conversation navigation — 2026-09-26

The conversation history now scrolls inside a bounded, keyboard-focusable
region, leaving the message composer accessible below it. A restored history
opens at its latest message. New responses follow the bottom only while the
person is already near it; reading older messages is not interrupted by a new
reply. Sending a new message returns to the latest part of the conversation.
This is presentation only: Codex still owns the conversation and Forge project
state is unchanged.

**PASS:** browser regression with a controlled 160-message resumed history,
including end position, preservation of earlier reading position, new-reply
following, keyboard order and narrow width; JavaScript syntax and desktop
debug build; hidden native WebView smoke with a controlled 100-line history
layout, actual folder/file dialogs, project onboarding, Forge record readback,
preview and appearance. The native smoke did not take focus from the desktop.
An initial native test assertion ran while the workspace was hidden; the test
was corrected to navigate to the visible workspace before measuring layout.

**NOT_RUN:** a substantial real Codex history or new real Codex reply in this
slice. Controlled fixtures establish UI behavior, not actual provider history
or end-to-end restart performance. The installed application is still older
than this debug build. No commit, push, installer or publication occurred; no
worker was routed. Per-model token and BRL attribution remain UNKNOWN.

**Next:** resume an existing substantial Codex conversation in the hidden
native app, without sending a new turn, to check real-history readback and
rendering time. Then continue remaining UI coverage and visual review against
the approved art; do not infer product completion from fixture-only checks.

## Real Codex history and visible composer — 2026-09-26

A completed, existing Forge-project Codex thread with 427 turns and 2,345
user/agent messages exposed a real recovery limit. Direct app-server readback
showed about 95.6 MB of full-turn responses, including tool traces, with one
response about 10.2 MB. The previous 1 MiB frame limit failed in the hidden
native app. A reduced `itemsView: summary` was investigated but omitted real
intermediate user and assistant messages; it was **not** substituted for full
history. The full-turn path remains Codex-owned, paginated and bounded; its
limits are now 16 MiB per frame and 128 MiB total page data. Red-before-green
Rust regressions cover both limits, and the existing oversize/aggregate guards
remain active.

**PASS:** `cargo check -p forge-desktop`, the two focused regressions, all 31
desktop Rust tests, browser UI regression, desktop debug build, and hidden
native WebView. The real thread resumed with all 2,345 projected messages in
original order, without sending a turn; the first display took 7.9–12.7
seconds in passing runs. One passing run disconnected, reloaded the WebView,
resumed again and preserved first/last message text and total count. Visual
inspection found the composer below the viewport after connection; the UI now
scrolls the outer page just enough to show **Enviar** while keeping the history
at its latest message. The final hidden native run passed that visibility
assertion and the reload/readback checks. Controlled browser fixtures remain
separate from the real Codex evidence.

**PARTIAL:** repeated real-history probes were not perfectly reliable: one
second resume timed out waiting for success, and another first resume reported
that the Codex connection closed. Later runs succeeded, but the intermittent
cause is not yet identified. A WebView reload is not a full process restart;
no message was sent or replayed, and no installed-version upgrade was tested.
The installed app is older than this debug build. No commit, push, installer
or publication occurred; no worker was routed. Per-model tokens and BRL cost
are UNKNOWN. A local native screenshot of the final visible-composer state is
`C:\ForgeFast\forge-real-history-send-visible-20260926.png`; it contains real
conversation content and should not be published as a generic fixture.

**Next:** investigate the intermittent native resume failure using bounded
diagnostics, then continue the remaining UI journeys against the approved art
and prepare an alpha package only after the source slice is coherent. Keep
the publication approval boundary separate.

## Project identity in the workspace — 2026-09-26

The confirmed folder now names the workspace instead of the generic creation
heading; the Forge project ID and exact path remain visible in the project
panel. The back link leads to **Meus projetos** while a project is in use.
Changing the selected path, including a failed attempt to open a shortcut,
restores the creation heading and link, so the previous project is not shown
as current. This is UI presentation of the already validated project; no
project identity, files, Codex history or Forge state were changed.

**PASS:** browser regression for opening, switching, failed revalidation and
mobile layout, including a red-before-green long-folder-name overflow check;
visual inspection of the controlled workspace screenshot;
desktop debug build; hidden native WebView smoke asserting the real confirmed
folder title, reset after invalid folder, and a new project's title. The
native smoke also passed its existing Forge record, onboarding, preview and
appearance checks. It did not take focus from the user's desktop.

**PARTIAL / NOT_RUN:** the intermittent real-Codex long-history resume cause
remains unknown; this slice did not run a real Codex conversation or full
process restart. The installed app is still older than the debug build. No
commit, push, installer or publication occurred; no worker was routed. Model
token attribution and BRL cost remain UNKNOWN.

**Next:** run a bounded real-history diagnostic that distinguishes Codex child
exit, frame/JSON failure and request timeout without recording conversation
content; then continue the remaining UI journeys, especially saved
conversations and review/decision surfaces, against actual backend data.

## Previous conversations picker — 2026-09-26

The workspace now offers **Escolher uma conversa anterior**. It asks Codex for
top-level conversations in the confirmed project's folder, presents their
titles and dates, supports additional pages, and resumes the selected thread
through the existing validated connection path. Listing or selecting does not
send a new turn. An active Codex thread is shown but cannot be taken over here.
The existing last-conversation bookmark remains the default; a failed explicit
resume does not replace it. Only the Codex on-device state index is searched,
so the UI does not promise every historical thread on every installation.
This is an adapter/UI addition, not a new history store or Forge-core change.

**PASS:** desktop crate `cargo check` and two filtered list tests before the
final UI adjustments; all 33 desktop Rust tests, including Rust projection of
subagent exclusion; browser regression covering list pagination, active-thread
exclusion, literal untrusted titles, failed selection, bookmark preservation
and explicit resume without a send; debug build; hidden native WebView read-only lookup of 18 real project conversations
without opening one or sending a turn. The hidden native smoke also passed
project onboarding, record readback, preview and appearance. `git diff --check`
found no whitespace errors. The native run did not take focus from the desktop.

**NOT_RUN / PARTIAL:** clicking a real indexed conversation in this new picker
was not exercised natively; browser selection uses a controlled Codex double.
The separate prior real-history resume passed in the app, but its intermittent
failure remains unexplained. Complete process restart, installed-version
upgrade and current-source installer distribution remain unverified. The
installed app remains older than the debug build. No commit, push, installer,
publication or worker routing occurred in this slice. Per-model tokens and BRL
cost remain UNKNOWN.

**Next:** choose one small real indexed conversation through the hidden native
picker without sending, verify exact selected-thread readback and failure
behavior, then continue visual/interaction review of remaining UI journeys
against approved art. Investigate the long-history intermittent failure before
calling conversation recovery robust. Keep package/release approval separate.

## Real picker resume and bounded pages — 2026-09-26

The previously unverified selection path now passed in the hidden native app:
after listing the real on-device Codex index, the UI resumed a selected
two-message conversation in the exact Forge project, with no `send_message`
call. A successful explicit selection updated only the existing last-thread
bookmark. The test records IDs and counts, not the private message bodies.
An initial test attempt failed because direct mutation of Tauri's exposed
`core.invoke` was not used by the UI; the test wrapper was corrected to replace
the top-level bridge object. This was a test-instrumentation error, not a
product connection failure.

Visual inspection of the original real-list capture showed 18 full-title rows
in one disclosure, pushing the actual conversation far below the fold. The
picker now requests six indexed entries at a time, replaces rather than appends
rows, offers previous/next pages, keeps the current page on a lookup failure,
and truncates only the visual title to two lines. The complete title remains
the button's accessible name and native tooltip. Listing is still opt-in and
read-only, with no Forge-side history database. A real hidden WebView capture
of the bounded list is `C:\ForgeFast\forge-picker-paged-final-20260926.png`;
it contains real conversation titles and is not a generic fixture.

**PASS:** focused desktop `cargo check` and two list tests; all 33 desktop
Rust tests; browser list regression for next/previous paging, failed page
retention, narrow width, focus, safe literal titles and exact resume; debug
build; hidden native WebView first/second/first page navigation and selected
real-thread readback without sending. Existing onboarding, Forge record and
preview smoke remained green. `git diff --check` should be repeated after this
checkpoint edit. No worker was routed; model identity and per-model usage are
not observable in this slice, so BRL cost remains UNKNOWN.

**PARTIAL / NOT_RUN:** the native selected-thread test covered a short history,
not the previously observed intermittent 2,345-message recovery. It did not
send a real turn, restart the full process, test an installed-version upgrade,
or publish the current UI. The long-history intermittent cause remains unknown.
No commit, push, installer or release occurred. The installed app remains
older than this debug build.

**Next:** investigate the long-history intermittent recovery with a bounded
red-capable diagnostic separating Codex child exit, malformed/oversized frame
and request timeout, without logging conversation content. Then continue
review/decision and preview journeys against real backend data and the approved
art. Distribution remains a separate approval-gated alpha package.

## Clearer recovery and discuss-a-question action — 2026-09-26

A controlled native attempt to resume this *currently active* Codex task failed
twice. A direct app-server probe (initialize, account/read, thread/read,
thread/resume; no turn sent and no message bodies logged) returned `-32600` with
`already has an active writer`. This is an expected concurrent-writer refusal,
**not evidence of the earlier intermittent long-history failure**. The UI had
mistakenly presented every resume rejection as history/CLI incompatibility. The
transport now maps only the verified active-writer shape to a plain-language
"conversation open elsewhere" message; other rejections keep the prior safe
fallback, and protocol details/thread IDs are not shown to the user.

The existing read-only Forge record now lets the user move a suggested question
into the conversation draft with **Conversar sobre isso**. Existing draft text
is retained, no turn is sent, and this does not record a decision or promote a
suggestion into an accepted agreement. The record and the composer retain
their separate owners; no Forge-core changes or new decision store were added.

**PASS:** focused `cargo check` and two active-writer tests; all 35 desktop Rust
tests; browser regression including no-send and draft preservation; debug
build; hidden native WebView smoke for project onboarding, record readback,
preview and appearance; `git diff --check` (line-ending warnings only). The
native smoke did not take focus from the user's desktop.

**PARTIAL / NOT_RUN:** the new active-writer UI message was not re-exercised
through native WebView in this slice; the direct protocol error plus unit path
are the evidence. A completed real long history was not identified or resumed,
so the historical intermittent cause remains unknown. The native smoke did not
send a Codex message. The installed app is still older than the debug build;
no commit, push, installer or publication occurred. No worker was routed;
per-model usage and BRL cost are UNKNOWN.

**Next:** use a completed, non-active real long conversation for bounded resume
diagnostics without transcript capture, then continue the decision and preview
journeys with real backend data and compare screens to approved art. Keep any
alpha installer/publication behind the separate maintainer approval.

## Read-only direction history in the desktop UI — 2026-09-26

The project record now offers an opt-in **Histórico de direções** view. It
queries the existing `workflow report` only when opened, validates project and
continuity identity, and returns a small read-only projection of accepted
objective revisions to the WebView. Each entry distinguishes the current
direction from an earlier version, identifies cooperative versus human-intent
record origin, and keeps detail collapsed until requested. Suggestions and
unresolved questions remain outside this history. A 4 MiB CLI-response bound
and 100-entry display bound fail or disclose truncation rather than silently
claiming complete history. No Forge-core state, decision, or evidence was
written. No new registry or governance implementation was introduced.

**PASS:** focused desktop `cargo check` and seven history-filtered Rust tests;
all 38 desktop Rust tests; browser history regression with opt-in loading,
current/prior distinction, safe literal text, and disclosure; hidden native
WebView smoke read four real revisions from this repository's Forge report,
alongside existing project/record/preview smoke. The native test was headless
and did not take focus. The controlled UI capture is
`C:\ForgeFast\forge-direction-history-20260926.png`; it uses mock text, not
real user decisions.

**PARTIAL / NOT_RUN:** this is accepted *objective direction* history, not the
full semantic decision-history review in issue #92: the core report's separate
decision audit contains opaque refs/status, not ready-to-display choice text.
Human-intent origin was covered by a Rust fixture, not a native project with
such a record. The screenshot still shows a dense project sidebar when the
record is expanded; visual refinement remains. Long real Codex recovery,
installed upgrade and publication remain unverified. No commit, push, release
or worker dispatch occurred. Per-model token attribution and BRL cost remain
UNKNOWN.

**Next:** inspect whether the existing Forge policy/report can resolve
decision-audit refs into user-facing accepted/pending choice text without
fabrication; if not, keep the honest partial view and improve the visual
layout of the record/preview alongside conversation. Separately diagnose a
completed real long-history resume. Approval is still required before release.

## Separated workspace panels — 2026-09-26

Visual comparison with `design/references/conversation-approved.png` exposed a
concrete hierarchy problem: the entire Forge record lived inside the project
folder/switching panel, so opening history stretched setup far down the page
and pushed local preview lower. The workspace now separates **project**, **local
preview**, and **record** into independent panels. The record is absent from
view until a project is confirmed. The conversation remains the dominant
desktop column; preview and record sit in the context column, with a loaded
preview rising above setup. Narrow layouts retain natural DOM order: project,
conversation, preview, record. Existing read-only data, controls and command
owners were not changed. This is a visual/DOM change, not a new backend layer.

**PASS:** browser suite covered initial hidden state, confirmed-record panel,
desktop and mobile panel order, preview repositioning, history, project switch,
keyboard and overflow; a debug rebuild embedded the new HTML; hidden native
WebView smoke passed real project/record readback, four real direction revisions,
preview and appearance without taking focus. Controlled browser capture:
`C:\ForgeFast\forge-workspace-separated-record-20260926.png` (mock text, dark
mode). `git diff --check` passed after this checkpoint edit; Git reported
line-ending conversion warnings only.

**PARTIAL / NOT_RUN:** the separated layout is closer to the approved board,
not visual completion: empty preview and long expanded record still need
polish, and the capture is not a real product preview. Actual Codex sending,
long-history recovery, installed upgrade and publication were not run here.
No commit, push, release or worker dispatch occurred; model/token attribution
and BRL cost remain UNKNOWN.

**Next:** design the smallest authoritative read-only mapping from historical
decision refs to their questions and selected alternatives; do not reuse current
simulation candidates as historical truth. Then continue the chat/preview/record
journey against approved art and exercise a completed real long-history resume.
Publication remains approval-gated.

Read-only mapping check: `WorkflowReplacementDecisionAudit` carries policy and
decision refs, status and selected-alternative ref, while `DecisionRequest`
defines the actual question and alternative descriptions. The current report
does not embed those historical descriptions in its decision audit. Current
simulation candidates cannot stand in for historical resolved decisions. A
complete semantic decision review therefore needs a small authoritative core
projection or another existing read-only resolver; desktop must not guess the
text from IDs. This does not affect the already implemented direction history.

## Compact project context and clearer navigation — 2026-09-27

Compared the current light-mode workspace capture with
`design/references/conversation-approved.png`. A confirmed project still used a
large folder/setup card, duplicating status and path above contextual work. The
project path and its confirmation detail now start collapsed in **Ver pasta
confirmada**; **Trocar de projeto** remains directly available. The workspace
heading has less empty space above the conversation. Navigation now uses
**Explorar** and **Minha conversa**, so an opened project is not misleadingly
marked **Criar**. The folder value, Forge confirmation, commands and project
switch rules did not change.

**PASS:** browser suite including collapsed/opened path, keyboard, mobile text,
overflow, project switch and existing flows; rebuilt desktop debug executable;
hidden native WebView smoke including real Forge record and four real direction
revisions, onboarding and bounded local preview. `git diff --check` passed
with only line-ending warnings. Controlled browser capture:
`C:\ForgeFast\forge-preview-compact-20260927.png` (mock project and text, not a
real website). The native smoke did not take focus from the user's desktop.

**PARTIAL / NOT_RUN:** this is layout and wording, not the full approved visual
board. The preview still supports bounded text and raster images, not a rendered
website; a quick CSP probe showed that sandboxed `srcdoc` can display text but
cannot apply an HTML file's inline styling under the app's present CSP. Do not
weaken the app CSP merely to make an attractive but incomplete preview.
Actual Codex sending, completed real long-history recovery, installed upgrade
and publication were not run in this slice. No commit, push, release or worker
dispatch occurred. Per-model token attribution and BRL cost remain UNKNOWN.

**Next:** continue a real, safe project-result review loop and accepted-decision
view without inventing data; keep HTML preview isolated from app privileges.
Separately diagnose a completed, non-active real long conversation. The alpha
installer and publication remain approval-gated.

## Conversation recovery and enlarged local preview — 2026-09-27

The first-use workspace no longer offers **Começar outra conversa** before a
previous conversation is known. Once a conversation is connected, the existing
choice appears; a failed local bookmark read leaves it available so a user can
start afresh. This changes UI visibility only, not Codex history or storage.

The local preview now has **Abrir prévia**, a larger modal view of the same
validated raster image or literal text. The modal identifies the local file,
states that publication is unverified, closes with Escape, and can prepare a
change request in the existing conversation without sending it. HTML remains
literal text; it is not executed or presented as a rendered website. The modal
clears on file/project changes and navigation. No new native privilege was
added for this view.

**PASS:** browser suite covers first-use visibility, keyboard order, modal
text/image, Escape, change-request handoff without sending, and invalid-file
states. A rebuilt debug executable passed hidden native WebView smoke, including
modal literal-text handling and bounded Forge project/preview readback. A
completed real Codex conversation with **2,345 messages** resumed without
sending a turn; first display took 9.9 seconds in this run. Reloading the
WebView preserved message count, first/last order and the composer. Two earlier
hidden runs of the same completed conversation also passed (9.3 and 10.2
seconds to first display). Tests ran without taking focus from the desktop.

**PARTIAL / NOT_RUN:** this proves recovery of this real long conversation on
this debug build, not that the old intermittent failure is impossible, nor
process-restart or installed-version continuity. No real Codex reply was sent
in these tests. The preview is still not a safely isolated rendered website;
the full accepted-decision history lacks user-facing historical prose in the
current core report. Installed upgrade, final alpha package, commit, push and
publication were not run. No worker was dispatched. Per-model token usage and
BRL cost for this slice remain UNKNOWN.

**Next:** make the visible conversation/preview/record journey cohesive against
the approved art while keeping data authority unchanged; separately design an
isolated HTML preview rather than weakening the app CSP. Preserve the approval
gate for installer publication.

## Confirmed-project workspace hierarchy — 2026-09-27

Compared the loaded workspace against `design/references/conversation-approved.png`.
The prior confirmed-project screen still spent a large vertical block repeating
setup text above the real conversation and preview. Once a project is confirmed,
the workspace now shows a compact project title and back link; the redundant
eyebrow and introduction are hidden. The conversation remains a flexible card
that can grow with its controls and never lets the composer escape its border.
Before confirmation, the full two-step guidance remains visible. This is a
visual hierarchy change only; no project, Codex, or Forge record behavior changed.

**PASS:** browser suite including a confirmed-project geometry assertion that
the composer stays inside its card, existing first-use/navigation/mobile/
keyboard/conversation tests, and `git diff --check`. A fresh debug build passed
hidden native WebView smoke with real Forge project/record and bounded preview
readback; actual Codex sending was not enabled in this smoke. Controlled browser
capture: `C:\ForgeFast\forge-preview-layout-20260927.png` (mock project and
literal text, not a real website). The native test did not take focus.

**PARTIAL:** the loaded workspace is less setup-heavy but not the finished
approved board. Its website preview, semantic historical decisions and overall
visual polish still require work. No installer update, commit, push, release or
worker dispatch occurred. Model-specific token usage and BRL cost are UNKNOWN.

**Next:** build an actually isolated, local website-result preview that cannot
inherit app privileges or silently fetch remote resources, then validate it
against a real project result and the approved board. Do not present raw HTML
as a rendered website or weaken the app CSP to shortcut this requirement.

## Isolated local HTML preview — 2026-09-27

Selecting an `.html` or `.htm` file now shows a **visual local preview** with
its local CSS, raster images and WOFF fonts. **Ver código** exposes the literal
source, and **Abrir prévia** enlarges either view. The preview remains static:
links cannot be clicked, scripts do not run, forms do not submit, and this does
not confirm that the site works as a deployed or interactive product. JavaScript-
rendered single-page apps may therefore appear incomplete or blank.

The selected page's own directory is served read-only through a separate
Tauri custom-protocol origin. Canonical path checks reject traversal and
symlinks outside that directory; file types and sizes are bounded. The child
document receives a restrictive CSP and an empty iframe sandbox; the app's
main CSP allows framing only this local origin, without relaxing its script or
style policy. The site session is revoked on project change and when another
file is inspected. This is a small native preview boundary, not a second
project store or a browser with general filesystem/network access.

**PASS:** `cargo check -p forge-desktop`, five focused preview tests, all 40
desktop crate tests, browser UI suite, and fresh hidden native WebView smoke.
The native fixture rendered local CSS and a local PNG; a script could not set
state in the parent, and the parent could not read the iframe document. A
controlled timed meta-refresh to loopback made no request. Source switching,
modal enlargement, text/image fallback and outside-project rejection passed.
Native capture: `C:\ForgeFast\forge-native-site-preview-20260927.png`. It is a
controlled fixture, **not** an actual user's generated site or the approved
visual board. The file picker response in this smoke was simulated; a separate
earlier native selection check is recorded above. Tests ran on a hidden Windows
desktop without taking focus.

**PARTIAL / NOT_RUN:** rendering of a real generated website, interaction in a
site, JavaScript-driven output, and every possible browser navigation/resource
mechanism are not proven by this fixture. No actual Codex reply, installed
upgrade, commit, push, or publication was run here. UI completeness is not
claimed. No worker was dispatched; model-token attribution and BRL cost remain
UNKNOWN.

**Next:** open a real agent-produced static site in this preview, compare its
appearance to the source site's own browser rendering, and correct concrete
fidelity or isolation gaps. Then continue the conversation/record visual
journey and semantic accepted-decision presentation without fabricated data.

## Preview-first result panel — 2026-09-27

Compared the current confirmed-project screen with the approved conversation
reference. Once a local file is loaded, the result panel now places **Abrir
prévia** beside its heading, shows the actual visual/text result before file
tools, and hides introductory instructions that no longer apply. The file path
and the warning that publication is unverified remain visible below the result.
Choosing another file, refreshing, viewing code, and requesting a change remain
available. Empty and invalid-file states still keep the chooser visible; the
preview never invents a generated result.

**PASS:** browser suite covers loaded-panel reading order, keyboard/mobile,
HTML/text/image views, source switching and invalid files. A fresh debug build
passed hidden native WebView smoke, including the sandboxed HTML fixture and
real Forge project readback. Controlled browser capture:
`C:\ForgeFast\forge-html-preview-ui-20260927.png`. The image uses a mock
project/site, not a real agent output; its broken external image is intentional
in the fixture. The native test did not take focus.

**PARTIAL / NOT_RUN:** the approved art is still a reference, not pixel-matched
output; real generated-site fidelity and actual Codex sending were not tested
in this slice. The installed app was not replaced. No commit, push, installer
or release was produced. No worker was dispatched; per-model token usage and
BRL cost remain UNKNOWN.

**Next:** exercise the preview with a real locally generated static site if an
eligible project artifact is available, then improve the conversation/record
hierarchy against real project data. Do not substitute the controlled fixture
for a user project's result or claim full UI completion.

## Readable record timeline — 2026-09-27

The **Onde estamos** panel now places the Forge-recorded current activity and
next step under **Agora, no registro** and **Depois, no registro**. The recorded
work title, outcome, and decision count remain available under **Mais detalhes
deste trabalho** instead of occupying the first reading path. **Entender isto
na conversa** prepares a plain-language explanation request in the existing
composer; it preserves a handwritten draft and does not send or change Forge
state. The request explicitly asks the agent to compare the possibly stale
record with the conversation. This is presentation of actual Forge fields,
not a generated summary or a new progress store.

The static HTML preview also now explains why remote images or interactive
parts may be missing. The warning appears only for HTML and does not replace
the publication-status notice.

**PASS:** browser suite covers current/next visibility, optional supporting
detail, explanation draft/focus/no-send, HTML-specific warning, source/media
views, and mobile overflow. A fresh debug build passed hidden native WebView
smoke with real Forge record readback and visible current/next fields. Native
capture using this repository's real Forge record:
`C:\ForgeFast\forge-native-record-timeline-20260927.png`. That image also
shows an explicitly opened long direction section; its prose is raw record
content, not a human-friendly generated explanation. The native test did not
take focus.

One browser run briefly observed the stale Explore card count immediately
after clearing search; rerunning passed. The test now waits for the synchronous
filter's observable card state before asserting count. This is a test timing
observation, not evidence of a user-visible filtering defect.

**PARTIAL / NOT_RUN:** no eligible user-generated static site was found in the
current worktree or named auxiliary workspaces, so real-site preview fidelity
remains unverified. The explanation request was not sent to Codex. Installed
app update, commit, push, installer and publication were not done. No worker
was dispatched; model-token attribution and BRL cost remain UNKNOWN.

**Next:** verify a real result when available, then continue reducing the
technical/raw-record burden in the project journey without inventing status or
silently rewriting the canonical Forge record. Full UI completion remains open.

## Recorded stage at a glance — 2026-09-27

The project heading now shows a small **Etapa no registro** label only after
the existing Forge readback succeeds. If Forge marks that record stale, the
label says so; if there is no recorded work, a read fails, a project changes,
or a message invalidates the prior readback, the label is hidden. This uses the
same phase mapping already shown in **Onde estamos**; it does not invent a
four-step progress bar from the concept art. In the result panel, **Atualizar
prévia** stays hidden until a file has actually been chosen; a failed read can
still be retried.

**PASS:** browser suite checked the phase label for current, stale and absent
records, preview empty/loaded controls, keyboard/mobile behavior, and existing
conversation flows. A fresh debug build passed hidden native WebView smoke,
including visible phase readback from this repository's actual Forge record.
Controlled browser capture: `C:\ForgeFast\forge-phase-badge-20260927.png`.

**PARTIAL / NOT_RUN:** this is recorded stage, not live agent progress or proof
that a phase is complete. Real generated-site fidelity, an actual Codex reply
in this build, installed update, commit, push and publication were not run.
No worker was dispatched; per-model token usage and BRL cost remain UNKNOWN.

**Next:** close larger UI gaps with actual user journeys: a nontechnical first
run through folder choice, conversation, result and record, followed by a real
site/result artifact when available. Keep the concept art's fictional content
out of the product UI.

## Conversation file actions and real Codex smoke — 2026-09-27

Completed agent messages now present supported local Markdown file references
as buttons that open the existing read-only project preview. The frontend only
recognizes plausible local file paths; native `inspect_preview` remains the
authority for project containment, file type, and size. External URLs and
unsupported links remain literal text rather than navigating the app. Both a
successful result and a validation failure bring the preview panel into view.
The original reply remains available in the conversation's raw-text view.

Explore now reconciles search-card visibility when revisited. This addresses
a possible mismatch when the browser changes the search field without firing
`input`; the earlier intermittent browser-test timeout did not establish a
single root cause, so continued observation is warranted.

**PASS:** browser suite passed three consecutive runs after the change. It
covers relative file references, a rejected outside-project path, literal
external/malicious links, keyboard order, and the Explore revisit case. A fresh
desktop debug build passed hidden native WebView smoke with actual Forge
project readback. A controlled completed-message file action opened a project
file through native preview and rejected a path outside that project.
Separately, a real Codex smoke in
this session sent one controlled prompt, received one reply, then restored
both sides after WebView reload without resending. That smoke used a
process-local override to an authenticated Codex CLI; it was not an installed
package or out-of-box CLI-discovery test. `git diff --check` found no whitespace
errors (line-ending notices only).

**PARTIAL / NOT_RUN:** the real Codex smoke preceded the last frontend-only
file-action changes; the new file button was checked with controlled browser
and native fixtures, not via a real Codex reply containing a link. Markdown
links with parentheses in their destinations
and remote links are not supported as actions. Generated-site fidelity, an
installed update, commit, push, installer, and publication remain open. No
worker was dispatched; per-model token usage and BRL cost remain UNKNOWN.

**Next:** inspect the actual conversation, result, and record together against
the approved visual direction and a real local output, then close concrete
interaction and presentation gaps. UI completeness is not claimed.

## Conversation entry and longer drafts — 2026-09-27

The conversation entry now explains that **Abrir conversa** is optional: a
person can open it without sending, or simply write and send to open it at
that point. The idle connection notice is visually quieter, while active and
error states retain their distinct status treatment. The composer expands for
longer drafts up to a bounded height before scrolling, including drafts
prepared by other UI actions; no extra draft store was introduced.

**PASS:** focused browser suite covers the optional-opening explanation,
growing composer, narrow text sizing and existing conversation/recovery flows.
A fresh debug build passed hidden native WebView smoke, including a growing
draft in WebView2 and the project/file-preview checks. No Rust source changed
in this slice, so no workspace Rust test suite was run.

**PARTIAL / NOT_RUN:** this does not close the full conversation/record/preview
visual journey, nor prove a version upgrade or real generated-site fidelity.
The installed application was not replaced; no commit, push or release was
made. No worker was dispatched; per-model token usage and BRL cost remain
UNKNOWN.

## Safer, more readable completed replies — 2026-09-27

Completed Codex replies now also render ordinary Markdown tables, quoted
passages and separators without inserting agent HTML. Wide tables have a
keyboard-focusable horizontal scroll region; the full conversation page does
not widen on a narrow viewport. Streaming/incomplete text remains literal,
and **Ver texto original** still exposes the exact reply.

**PASS:** browser suite checked table cells, blockquote, separator, local-file
action inside a table, literal malicious/external links, keyboard order and
mobile overflow. A fresh debug build passed hidden native WebView smoke with
a controlled formatted reply and existing native project/preview checks.
Visual capture of the controlled reply:
`C:\ForgeFast\forge-formatted-message-20260927.png`.

**PARTIAL / NOT_RUN:** this is deliberately a bounded Markdown presentation,
not every Markdown extension or a trusted external-link opener. The capture
uses test content, not a real Codex-produced table. Real generated-site
fidelity, installed update, commit, push and publication remain open. No
worker was dispatched; per-model token usage and BRL cost remain UNKNOWN.

## Visible agent-access boundary — 2026-09-27

The confirmed-project composer now visibly explains the existing Codex
session's access before Send. The app currently starts app-server with
`approvalPolicy: never` and `sandbox: danger-full-access`; its developer
instruction asks the agent to stay within the project, but that instruction
is not a filesystem sandbox. The UI therefore says the agent can run commands
and change files without per-action confirmation, including outside the
selected folder. No permission setting or transport behavior was silently
changed. The warning is absent before project selection and stays visible
while a project is active.

**PASS:** browser suite checked warning visibility and wording with a selected
project, its absence before selection, and existing mobile/keyboard flows. A
fresh debug build passed hidden native WebView smoke with the warning visible.
The first native run in this slice observed the known intermittent automatic
Forge-record read failure and passed only after the manual **Consultar
registro** retry. The native test now captures the underlying Tauri error
when that occurs; two subsequent instrumented runs passed on first read, so
the exact cause of this occurrence remains unknown.

**PARTIAL / NOT_RUN:** a visible warning is not interactive approval, a
sandbox, or proof that every Codex tool interaction is supported. The
transport still rejects unsupported interactive requests. The installed app
was not updated, and no commit, push or release occurred. No worker was
dispatched; per-model token usage and BRL cost remain UNKNOWN.

## Unconfirmed send recovery — 2026-09-27

If the desktop-to-Codex `send_message` call fails, the locally displayed user
message is now marked **envio não confirmado**, not shown as an ordinary sent
turn. The draft returns to the composer, and Send cannot silently reconnect
and retry it. The person must explicitly open the same conversation and review
its returned history first. Starting a new conversation, selecting a different
thread, or receiving a response that does not confirm resume leaves the guard
in place. This is an in-process recovery guard, not a durable delivery receipt;
the Codex server may have accepted a message even when the IPC call failed.

**PASS:** browser suite exercises rejected send, retained draft, visible
unconfirmed state, no second `send_message`, rejection of a new conversation
and unconfirmed resume, and recovery only after explicit same-thread history
readback. A fresh desktop debug build passed hidden native WebView smoke with
actual Forge project readback and existing preview checks. `git diff --check`
will be run at the close of this slice. No Rust source changed in this slice,
so no workspace Rust suite was run.

**PARTIAL / NOT_RUN:** the native smoke did not force a real Codex send failure
or consume a model reply; this specific failure path is proven with the
controlled browser IPC fixture. The guard is not persisted after app restart,
but startup never auto-resends a draft. The installed app was not updated; no
commit, push, installer, or release occurred. No worker was dispatched;
per-model token usage and BRL cost remain UNKNOWN.

**Next:** continue the concrete conversation/record/preview UI review against
the approved visual direction, then close remaining #91/#92/#93 product gaps
before calling the UI complete. Confirm the access-policy decision before
changing permissions or exposing interactive approval controls.

## Readable local Markdown results — 2026-09-27

A selected `.md` file now opens as readable, safely formatted local text in the
preview panel and expanded view. **Ver texto original** exposes the exact file
text, and **Ver leitura** returns to the formatted view. The existing
conversation formatter is reused without its local-file action callback:
Markdown links stay literal, and HTML is not executed. Other text files,
images and the isolated static-HTML preview retain their prior behavior.
The same native project/file containment and 32 KiB text limit still apply.

**PASS:** browser suite covers formatted headings/list, original-text toggle,
expanded view, literal external link and script text, then regression of image
and isolated HTML views. A fresh debug build passed hidden native WebView smoke
using a real project `.md` file and actual native `inspect_preview` readback,
plus the existing Forge record and preview checks. No real Codex turn was sent.
No Rust source changed in this slice; no workspace Rust suite was run.

**PARTIAL / NOT_RUN:** this improves review of a nonvisual deliverable but does
not make the full #91 preview journey or all UI complete. Dynamic JavaScript
sites still cannot be previewed as working apps, and publication state is not
inferred from a local file. The installed application was not updated; no
commit, push, installer or release occurred. No worker was dispatched;
per-model token usage and BRL cost remain UNKNOWN.

**Next:** close the remaining preview/recovery/decision UX gaps against the
actual issue acceptance criteria, including native restart evidence and
honest origin labels; do not mark #91/#92/#93 complete from this slice.

## Project identity consistency — 2026-09-27

The active workspace and confirmed-project card now use the selected folder's
name consistently. The Forge internal project ID remains available under
**Ver pasta confirmada**, labeled as technical identification; this changes no
project identity, persistence, or routing behavior.

**PASS:** browser suite covers both initially opened and switched projects;
desktop debug build and hidden native WebView smoke passed with the real
repository, including the project label and collapsed technical ID.
`git diff --check` passed. The native smoke did not send a real Codex turn.

**PARTIAL / NOT_RUN:** this is only one correction within the broader UI
completion package. No installed update, commit, push, or release occurred.
No worker was dispatched; per-model token usage and BRL cost remain UNKNOWN.

**Next:** evaluate and complete the connected user journey as one coherent
package: create/open project, converse, inspect outputs and Forge record,
recover after interruption/restart, and verify the resulting screens in the
hidden native app. Keep installation/publication a separate approved boundary.

## Connected conversation, result and restart package — 2026-09-27

The app now asks its Codex agent to name only reviewable files it actually
created or materially changed, with project-relative Markdown links. Existing
completed-reply file actions resolve those links through the native bounded
preview; this adds no preview authority or new project store. The confirmed
workspace keeps the preview above secondary project details from the start,
avoiding a layout jump when a file loads. Its empty state uses the approved
Forge icon and explains local review without implying that an artifact exists.
The header and panels received a restrained visual pass toward the approved
conversation board; narrow text and keyboard behavior remain covered.

**PASS:** desktop `cargo check`, focused `agent::tests` (16), all 40 desktop
crate tests, browser UI suite, debug build, and hidden native smoke. In a real
Codex turn inside a disposable project, the agent created `site/index.html`;
the reply's file action opened that exact file in the isolated native HTML
preview, and **Pedir mudança** returned an unsent draft to the same chat. A
separate hidden native run closed and relaunched the entire app process using
the same temporary WebView profile: the real Codex conversation returned in
the same message order with zero `send_message` calls during recovery. Local
browser captures: `C:\ForgeFast\forge-confirmed-layout-20260927.png` and
`C:\ForgeFast\forge-preview-layout-new-20260927.png`; real native record
capture: `C:\ForgeFast\forge-native-journey-20260927.png`. The browser captures
use mocked native commands; the native capture is not a model-generated site.

The first artifact-journey test assertion fired before the second reply had
arrived; the harness was corrected to wait for the second agent message and
the rerun passed. That first attempt still consumed a real model turn. Exact
model-token and BRL attribution remain UNKNOWN; this package cannot claim cost
savings. No worker was dispatched. `git diff --check`, desktop
`cargo fmt --check`, and Node syntax review passed.

**PARTIAL / NOT_RUN:** restart was tested within the same debug build, not
across an installed version upgrade. A successfully linked file is still an
agent-reported path validated by native preview, not proof of publication.
Only static HTML without scripts is rendered visually; dynamic sites remain
unsupported. The full decision semantics of #92 and all error/recovery cases
of #93 are not closed. The installed app was not replaced; no commit, push,
installer, or release occurred.

**Next:** review the integrated UI diff, complete the remaining decision and
failure-state experience without inventing project data, then perform package
verification and propose an alpha update for maintainer approval.

## Decision options in the project record — 2026-09-27

The read-only project record now shows each currently suggested decision with
its concrete alternatives, consequences, and Forge recommendation. It labels
these as suggestions, not accepted choices or recovered historical decisions.
Each option can prepare an explanatory question in the existing conversation;
this neither sends a turn nor records a choice. The native adapter validates
the alternative set and recommended reference before displaying it. No new
decision store, approval control, or core mutation was added.

**PASS:** desktop `cargo check`, 10 focused `progress::tests`, all 41 desktop
crate tests, browser UI suite, debug build and hidden native WebView smoke.
The browser fixture checked recommendation, consequences, literal script text,
and unsent conversation drafts. Visual capture:
`C:\ForgeFast\forge-decision-options-20260927.png` (browser fixture, not live
Forge candidate data). Hidden native smoke read the real Forge record; this
project currently returned no candidate decision request.

**PARTIAL / NOT_RUN:** the real native rendering of a nonempty decision candidate
was not exercised because the available real project had none; browser coverage
used a controlled fixture. Historical accepted decisions still lack readable
question/alternative snapshots in the existing report and are not inferred
from current suggestions. Error/recovery cases and full UI review remain.
No installed update, commit, push, installer, or release occurred. No worker
was dispatched; per-model token usage and BRL cost remain UNKNOWN.

**Next:** verify a real candidate decision in a suitable disposable Forge
project, or explicitly retain this native-coverage gap; then address concrete
error/recovery UX gaps and review the integrated desktop diff before preparing
an alpha update for approval.

## Record lookup recovery — 2026-09-27

The project record now distinguishes a temporary busy/timeout read from a
missing project state, using only known, curated native errors. It tells the
person whether to retry or recheck the selected folder and never implies that
the Forge state was recreated. Unexpected or malformed responses retain the
safe generic error. A failed read hides the old record rather than presenting
it as current; conversation remains available.

**PASS:** browser UI tests cover busy, missing-state, malformed and generic
failure paths; a fresh desktop debug build passed the hidden native smoke with
real Forge record readback. `git diff --check` and Node syntax review remain
the final local checks for this slice. No Rust source changed here, so no Rust
suite was rerun for this copy-only change.

**NOT_RUN / limits:** a real busy or missing-state error was not induced in the
native app; those branches were tested with controlled browser responses. A
fresh disposable core project at
`C:\ForgeFast\forge-candidate-probe-20260927` was initialized to look for a
real decision candidate, but its valid `workflow resume` returned zero
candidates. Its Forge sidecar is
`C:\ForgeFast\forge-forge-candidate-probe-20260927`. Cleanup of these two
owned test folders was blocked by the command policy; they remain untouched.
No real Codex turn, installed update, commit, push, installer or release in
this slice. No worker was dispatched; token and BRL attribution remain UNKNOWN.

**Next:** review the full accumulated desktop diff for product/contract gaps,
then verify the highest-risk native journeys before proposing the alpha
candidate. Retain the nonempty-decision native coverage gap unless a suitable
real core fixture is available without inventing or mutating history.

## Integrated preview and board review — 2026-09-27

Review of the accumulated desktop UI found a real local-preview boundary gap:
the site protocol served files from the selected site directory even when the
request omitted its session token. The protocol now requires the current token
for every file, including assets, and tests reject tokenless, wrong-token and
traversal paths. This keeps local site content scoped to the explicitly
selected preview session. Script and network restrictions remain unchanged.

The protected preview still renders static HTML with **relative** local CSS
and images. Root-absolute asset paths such as `/assets/site.css` no longer
resolve; the UI now says so rather than silently implying broad site support.
Dynamic JavaScript sites remain unsupported. The native smoke initially
**FAIL**ed on its root-absolute CSS fixture after the boundary was tightened;
the fixture was changed to the supported relative-path case and the rerun
passed. This is an intentional, disclosed compatibility limitation, not proof
that every generated site will render.

The confirmed-project board now places the recorded work directly beneath the
result preview, with secondary folder/connection controls afterward. DOM and
keyboard order follow the same order on desktop and narrow screens; before a
project is ready, folder setup remains first. The first-visit copy now says
an existing folder without Forge can be prepared without deleting files.
Visual fixture capture: `C:\ForgeFast\forge-record-above-project-20260927.png`
(browser-mocked project content, not a native project screenshot).

**PASS:** desktop `cargo check`, focused `preview_site::tests`, all 41 desktop
crate tests, browser UI suite including narrow-screen order, debug build, and
hidden native WebView smoke with actual static HTML/CSS/image readback and
board-order assertion. Native smoke did not send a real Codex turn.

**PARTIAL / NOT_RUN:** the integrated diff still needs package-level review and
release gates; no version-upgrade continuity check was run. A published build
does not yet include this work. No commit, push, installer or release occurred.
No worker was dispatched; exact model-token and BRL attribution are UNKNOWN.

**Next:** complete the package-level diff review, check the remaining native
user journeys and release notes, then prepare a single alpha candidate for
maintainer approval without publishing it first.

## Package review and real Windows dialogs — 2026-09-27

The accumulated desktop diff was reviewed against the current one-folder and
read-only-record contracts. A project error still named a removed button;
the Rust error and UI's safe-message mapping now say **Continuar nesta pasta**
in Minha conversa. Installer metadata now describes both new and returning
projects. `RELEASE_NOTES-0.1.1.md` was corrected: the old local package is
superseded, the current source can prepare an unlinked folder, and a new
candidate has not yet been built or published.

**PASS:** desktop `cargo check`; two focused `project::tests`; all 41 desktop
crate tests; debug build; strict desktop Clippy; browser UI suite; `cargo fmt
--check`; and `git diff --check`. The hidden native WebView smoke drove the
actual Windows folder and file dialogs on an isolated desktop, selected a
linked project, an existing unlinked folder, and a preview file. Selection
alone did not initialize the unlinked folder, and its pre-existing file
survived. No test window appeared on the user's active desktop.

**NOT_RUN / limits:** an installed-version upgrade of this source, real Codex
conversation across that upgrade, and native rendering of a nonempty real
decision suggestion remain unverified. The prior `0.1.1` installer is not a
candidate for this changed source. No commit, push, installer build or
publication occurred in this slice. No worker was dispatched; per-model
tokens and BRL attribution remain UNKNOWN.

**Next:** preserve the superseded installer, build one new local `0.1.1` NSIS
candidate after package gates, record its hash, and test that exact candidate
headlessly over the prior installed alpha. Publication requires maintainer
approval and subsequent download verification.

## Local 0.1.1 candidate from the integrated UI — 2026-09-27

The previous unpublished `0.1.1` installer was copied to the `superseded`
bundle directory and its earlier SHA-256 was confirmed before the new build.
The pinned `npm run build:nsis` command produced **one** new unsigned Windows
x64 installer at
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.1_x64-setup.exe`.
Its size is 4,604,779 bytes and SHA-256 is
`06D205A56AF9379E90A020BE409A0B2DF32AA09468485155F5BB7CB2C4923CE5`.
The hash was rechecked after installation and native verification.

**PASS:** desktop package gates (41 Rust tests, strict Clippy, formatting,
browser suite and hidden native smoke) and release build. The exact candidate
silently installed over the prior local `0.1.1` with exit code 0; the installed
executable reports product/file version `0.1.1`. The installed app passed the
hidden native WebView smoke, including real Windows folder and file dialogs,
project onboarding, Forge record, preview and appearance. In a second hidden
run, the installed app sent one real message to Codex from a disposable
project, received one reply, and restored both sides after WebView reload
without resending. No Forge desktop process was left running.

**NOT_RUN / limits:** the conversation was created **after** this install, so
its continuity across an installer upgrade is unproved. An exact `0.1.0` to
this candidate upgrade and native rendering of a nonempty real decision
suggestion are also unproved. This locally installed candidate is **not** a
published update; it has not been downloaded from a release. No commit, push
or publication occurred. No worker was dispatched; exact per-model tokens and
BRL cost are UNKNOWN.

**Next:** present the candidate's delivered UI and explicit limits to the
maintainer for approval before publishing. If more source changes are made,
this hash becomes superseded and a new candidate must be built/tested. After
approval, publish and verify the downloaded file and installed result.

## Conversation-first control cleanup — 2026-09-27

Comparison with the approved conversation board showed a concrete hierarchy
problem: an optional **Abrir conversa** button, its explanation and history
controls occupied the top of an otherwise empty chat. Current source groups
those optional actions under **Conversas e histórico** while leaving writing
and first Send as the direct path. The Codex badge now sits below the chat
heading. When connection hides that disclosure, keyboard focus moves to the
visible status instead of being stranded in hidden content. The native
conversation contract, storage and send behavior were not changed.

**PASS:** browser UI suite, including send/retry, history choice, keyboard
focus, long history, narrow screens, dark mode and failure recovery; debug
build; `git diff --check`; hidden native WebView smoke with real Windows folder
and file dialogs, onboarding, record and preview. The native smoke also sent
one real Codex message from a disposable project, received a reply and
restored both sides after a WebView reload without resending. Browser visual
fixture: `C:\ForgeFast\forge-conversation-tools-20260927.png` (controlled
conversation data, not a real project capture).

**NOT_RUN / limits:** the installed `0.1.1` and its SHA-256 candidate above
predate these source edits and are now **superseded for current-source
publication**. No replacement release build, installed upgrade, commit, push
or publication was done. The approved board's rich filled-result appearance
has not been proven with a comparable real native project. No worker was
dispatched; exact per-model tokens and BRL cost are UNKNOWN.

**Next:** assess the filled conversation and preview against the approved
board with an honest native capture, fix concrete remaining visual/interaction
gaps, then rebuild and verify a new alpha candidate only after that package is
stable. Keep publication behind maintainer approval.

The follow-up visual/interaction pass hid **Interromper** when idle and fixed
unconfirmed-send recovery after the connection controls moved into the
disclosure: the app opens it and focuses the visible **Abrir conversa** action.
A ready project now says **SUA CONVERSA** before connecting. Browser tests
cover these states and pass, as does `git diff --check`. A debug build and
hidden native smoke passed after the control changes; the final ready-project
label adjustment was browser-verified, not rebuilt into that smoke binary.
Native capture `C:\ForgeFast\forge-native-site-board-20260927.png` shows an
actual WebView displaying controlled static HTML and an empty chat; it is
**not** evidence of a filled real-agent result matching the approved concept.

## Filled conversation and long-page preview — 2026-09-27

The hidden native app sent two real Codex turns from a disposable project.
The second produced a local static HTML file, which the app opened from the
reply; both sides of the conversation survived a WebView reload without
resending. Capture `C:\ForgeFast\forge-native-filled-artifact-20260927.png`
shows that actual WebView and generated file, not the approved design itself.
The capture exposed a concrete gap: the enlarged, inert iframe showed only
the top of a long page. Enabling iframe pointer events was **rejected** after
a controlled native test: clicking an external link replaced its content with
`chrome-error://chromewebdata/`. The tested implementation instead retains
`pointer-events: none` and adds **Mostrar mais da página**, which expands the
iframe in bounded 800-pixel steps, up to 8000 pixels, inside the scrollable
parent dialog. Scripts and page interaction remain disabled; code view is
available. This is an inspection aid, not full-site or published-page proof.

**PASS:** browser UI suite; debug desktop build; hidden native WebView smoke
with controlled long HTML, local CSS/image, script-blocking, origin isolation,
dialog expansion, no external request during expansion, project onboarding,
Forge record and appearance. `git diff --check` found no whitespace errors.
The real Codex artifact journey passed before the expansion change; its
native fixture was re-run after that change, but the real Codex journey was
not repeated. No native app process remained running.

**NOT_RUN / limits:** no current-source release build, replacement installer,
upgrade continuity test, commit, push or publication. The installed `0.1.1`
candidate is superseded by these edits. A page longer than the bounded
expanded view is still incomplete; dynamic sites are not represented by the
scriptless preview. No worker was dispatched. Exact model-specific tokens
and BRL cost remain UNKNOWN.

**Next:** assess the remaining filled-result design and interaction gaps,
then stabilize one coherent UI package. At that point run final package gates,
build one new installer candidate, test its upgrade over the installed alpha,
and present its contents and limitations for approval before publication.

## Compact conversation and replacement alpha candidate — 2026-09-27

The real filled-conversation capture was compared with the approved board.
The composer occupied too much space when empty; it now starts at one line
and grows for longer drafts. A completed-turn status is shorter and visually
quieter, while error/working states and the explicit Codex access warning
remain visible. Browser captures are
`C:\ForgeFast\forge-workspace-compact-20260927.png` (light, controlled empty
project) and `C:\ForgeFast\forge-conversation-compact-20260927.png` (dark,
controlled conversation); neither is a real Codex response.

**PASS before packaging:** browser UI suite including growing drafts,
keyboard and narrow layouts; 41 desktop Rust tests; strict desktop Clippy;
`cargo fmt --check`; debug build; hidden native WebView smoke; `git diff
--check`. The previous candidate was copied to
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\superseded\Forge_0.1.1_x64-setup-superseded-06D205A5.exe` and its original
SHA-256 was confirmed. The pinned `npm run build:nsis` then produced **one**
replacement unsigned Windows x64 installer at
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.1_x64-setup.exe`.
It is 4,602,922 bytes; SHA-256
`9050AC0B91AE06E852F34A5CB493D8A3C8CE43CB9B742642962DF3CAE066E55D`.

**PASS after packaging:** that exact candidate installed silently over the
previous local `0.1.1` (exit 0); Windows reports installed product version
`0.1.1`, and the candidate hash remained unchanged. Hidden native smoke on
the **installed** executable passed project onboarding, real Windows folder
and file dialogs, Forge record, preview security/expansion and appearance.
A separate installed-app run sent two real Codex turns from a disposable
project; the second created local HTML, opened it from the reply, and returned
an unsent change request to the conversation. Both sides survived WebView
reload without resending. No app process remained running.

**NOT_RUN / limits:** the real conversation was created after installation;
continuity of a populated chat **across** this installer upgrade was not
proved. Exact `0.1.0` to this candidate upgrade, nonempty real decision
suggestion rendering, published download and downloaded-byte verification
remain unproved. The preview is scriptless/non-interactive and does not prove
a deployed site. No commit, push or release publication occurred. No worker
was dispatched; model-specific token counts and BRL attribution are UNKNOWN.

**Next:** present the candidate and the explicit limits in
`RELEASE_NOTES-0.1.1.md` to the maintainer for approval before publication.
Do not rebuild or overwrite the hashed candidate in place unless later app
changes supersede it; if they do, repeat the final gates/install verification.
Continue UI coverage as a separate follow-up slice without representing this
alpha building block as complete product readiness.

## Populated conversation across the local candidate upgrade — 2026-09-27

A controlled hidden-desktop test closed the upgrade gap **for the two local
`0.1.1` candidate builds**. It used one disposable WebView2 profile and one
new project, not the user's Codex conversations. The archived installer with
SHA-256 `06D205A56AF9379E90A020BE409A0B2DF32AA09468485155F5BB7CB2C4923CE5`
was silently installed. Its app sent one real no-tools Codex turn and received
one reply. After closing that app, the current candidate with SHA-256
`9050AC0B91AE06E852F34A5CB493D8A3C8CE43CB9B742642962DF3CAE066E55D`
installed over it. The new installed app reopened the same project/profile,
explicitly resumed the saved conversation, and displayed exactly the original
user/reply pair; no new Send occurred and no extra message appeared during
observation. The native resume path uses `thread/resume`, and a desktop Rust
unit test checks that it does not issue `turn/start`. Both installer processes
exited 0. The installed executable was restored to the current candidate's
verified hash, and the disposable profile/project/Forge sidecar were removed.
The test-only `tests/upgrade-continuity.cjs` and hidden runner selector remain
for repeatable future candidate comparisons; they are not bundled UI changes.

**PASS:** populated conversation readback across this local install-over;
the current candidate installer hash unchanged; `node --check` for the new
test; `git diff --check`; no Forge desktop process left running.

**NOT_RUN / limits:** this does not prove an exact `0.1.0` → current `0.1.1`
upgrade or a published download. Nonempty real decision-suggestion rendering
remains unverified. No commit, push or publication occurred. No worker was
dispatched; model-specific token/BRL attribution remains UNKNOWN.

**Next:** present the same hashed candidate and updated release notes for
maintainer approval. Do not publish before that approval. Future UI edits are
possible, but application-source edits would supersede this candidate and
require a new final package verification.

## Post-candidate visual audit without rebuilding — 2026-09-27

The browser UI suite passed while capturing current Home, Explore and My
Projects with controlled data. The captures and concrete comparison with
`design/references/explore-approved.png` are in
`design/ux-audit-2026-09-26.md` (visual follow-up section). Explore retains
all eight approved illustrations and its clear discovery route, but its
unillustrated header/search and plain ending lack the approved botanical
framing. Home's CSS bloom is comparatively generic; there is no approved Home
board to claim pixel fidelity. My Projects truthfully displays one local
shortcut rather than fabricated project cards. These are design observations,
not a native behavior test or a reason to rewrite the already-tested alpha
candidate without a coherent next package.

The two older disposable candidate-probe folders recorded above were verified
inside `C:\ForgeFast` and removed; they contained no user project. The local
installer candidate hash remains
`9050AC0B91AE06E852F34A5CB493D8A3C8CE43CB9B742642962DF3CAE066E55D`.
No application source, installer, installed binary, commit, push or release
was changed in this visual-audit slice. No worker was dispatched; model-specific
tokens and BRL attribution remain UNKNOWN.

**Next:** obtain maintainer approval or requested changes to the alpha notes
before publishing. Then treat the botanical Home/Explore framing as a separate
UI package with real exported art, responsive/accessible verification and a
new installer candidate if application source changes.

## Botanical Home/Explore UI package — 2026-09-27

The follow-up visual package is now in the working tree. It reuses decorative
regions of the approved Explore board: Home's generic CSS flower is gone;
Explore has a landscape header, a wider search field and foliage-framed
open-ended action. The eight category cards and real navigation are unchanged.
No backend or project/agent contract changed. An initial screenshot revealed
source-board text in the crop and foliage overlapping callout text; both were
corrected. At mobile widths the header crop is hidden and callout foliage is
subdued; forced-colors mode removes the decorative crops.

**PASS:** browser UI suite, final desktop/mobile screenshots in the design
audit, debug `forge-desktop` build with the existing locked/offline cache,
hidden native WebView smoke including Explore search/callout checks, `node
--check` for the native test, and `git diff --check`. The first native run
failed because the new search check left its filter active for a later test;
the test was corrected and the full hidden smoke passed. No visible app was
launched on the user's desktop. No Rust source changed in this slice, so no
Rust test suite was repeated.

**NOT_RUN / limits:** no new NSIS candidate, install-over, real Codex reply,
full accessibility audit, commit, push or publication in this slice. The
previously hashed local `0.1.1` candidate remains intact but is **superseded
for current-source publication**; its release notes now say so. The core-only
`.github/workflows/release.yml` cannot be treated as a desktop release gate;
desktop artifact provenance/publication needs a separate explicit path. No
worker was dispatched; exact per-model token counts and BRL cost are UNKNOWN.

**Next:** review the remaining UI journey and stabilize the current source as
one coherent alpha package. Then repeat final package gates, build one new
installer candidate, verify install-over and real conversation continuity,
and present its exact contents/limits for maintainer approval. Do not publish
or repurpose the superseded candidate.

## Native onboarding repair and current alpha candidate — 2026-09-27

An installed-app run against an empty disposable folder exposed a real gap:
`forge-core start` created the project link and sidecar but returned a
structured next step to run `workflow init`. The desktop had declared the
project ready without executing that step, so its first record lookup failed.
`start_project` now follows **only** an exact, project-matched `workflow init`
argv from the core, then validates the initialized project ID. The folder
picker and read-only inspection remain non-mutating; damaged linked state is
not silently repaired. A native test now requires a fresh project's record to
load after the same **Continuar nesta pasta** action.

A separate installed-app run without `FORGE_CODEX_EXE` reached the agent send
but timed out waiting for a reply with this machine's older npm Codex CLI
(`0.144.6`). The debug app now discovers the current Codex Desktop CLI under
the user's local installation before trying npm, while an explicit override
remains first. This is a best-effort local CLI discovery, not bundling Codex
or a guarantee that future internal Codex Desktop layouts remain the same.
The current Codex Desktop CLI was `0.155.0-alpha.16` in this test. A debug
hidden-native run **without** override then sent one real message, received a
reply and resumed both sides after WebView reload. The hidden runner's outer
timeout was extended so a future inner test failure can be reported instead
of orphaning its child process.

**PASS before packaging:** 43 desktop Rust tests, strict desktop Clippy,
`cargo fmt --check`, seven Node unit tests, browser UI suite, debug build,
hidden native smoke with real Codex reply and new-project record, and `git
diff --check`. The failed installed-app run above was diagnostic, not counted
as a pass. No core workspace-wide test/build was run for these desktop changes.

The exact current Windows x64 unsigned NSIS candidate is
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.1_x64-setup.exe`
(4,619,155 bytes; SHA-256
`F8E04F44E314DB7D530AD251731ACA69C76C6E239FF0FF0E3D461D2E55E70568`).
The preceding `3F7EA430` candidate was archived before this one was built.
This candidate installed silently over it with exit code 0; installed product
version is `0.1.1` and installed executable SHA-256 is
`6E8EF73A3A8EB0927B3556A13838A5924CBE31A00E4A9752698B1F2F4002AABD`.

**PASS after packaging:** the **installed** executable passed hidden native
WebView smoke, real Windows folder/file dialogs, fresh-folder onboarding and
record readback, preview boundaries, and one real Codex reply **without** a
CLI override, followed by WebView reload without resending. A separate
hidden-desktop upgrade test installed the previous local `3F7EA430`
candidate, created a real Codex turn in a disposable project/profile, then
installed this exact `F8E04F44` candidate and restored the same user/reply
pair without a new Send. Both installer exits were 0. The final candidate
hash was rechecked afterward; no Forge desktop process remained. The
disposable upgrade project, sidecar and profile were removed after path
verification inside `C:\ForgeFast`.

**NOT_RUN / limits:** exact `0.1.0` → this `0.1.1` upgrade, public download
and downloaded-byte readback, nonempty real decision-suggestion rendering,
full accessibility audit, commit, push and publication. The core-only release
workflow is not a desktop publication gate. Source UI still needs further
product coverage; this alpha package is a tested building block, not complete
UI. No worker was dispatched; exact per-model token counts and BRL cost are
UNKNOWN. `RELEASE_NOTES-0.1.1.md` is the review draft for this hash.

**Next:** present this candidate and its limits for maintainer approval.
Without that approval, do not publish, tag or claim a downloadable update.
Before publication, establish a desktop-specific source/artifact provenance
path rather than using the core-only `v*` release workflow, and recheck that
the exact approved candidate remains unchanged. Any application-source edit
supersedes this candidate and requires fresh package gates/install tests.

## Desktop alpha publication approval — 2026-09-27

The maintainer approved publication of the exact 0.1.1 Windows x64 alpha
candidate with SHA-256 `F8E04F44E314DB7D530AD251731ACA69C76C6E239FF0FF0E3D461D2E55E70568`
and the limitations in `RELEASE_NOTES-0.1.1.md`. This is authorization for
the source commit/push and publication needed for this candidate, not a claim
that those steps already happened. The package was built from this desktop
working tree before commit; its bytes cannot be inferred from a later rebuild.
Do not edit application source before publication. If it changes, rebuild and
reapprove a new candidate.

Use a `desktop-v0.1.1-alpha.1` tag for this desktop release, outside the
core-only `v*` workflow trigger. The desktop version is independent of the
core workspace version (`0.13.3` here). Publish the **prebuilt, previously
tested** installer, not a new GitHub Actions build. Record its source commit,
size and SHA-256 in the GitHub prerelease; attach a matching `.sha256` file.
After upload, download the asset to a fresh local path, compare size and
SHA-256, and install/test that downloaded file headlessly. A local candidate
hash alone is not downloaded-byte proof. The desktop package remains unsigned
and does not bundle `forge-core` or Codex CLI.

**Next:** audit and commit only the desktop files in this package, push the
source commit, create/push the desktop-specific tag, publish the approved
candidate as a GitHub prerelease, then download, hash-check and smoke-test
that exact public asset. Keep the goal active afterward: broader UI coverage
and polish are still pending. Model-specific tokens and BRL cost are UNKNOWN.

## Desktop 0.1.1 alpha published and public-download verification — 2026-09-27

The approved desktop source package was committed as
`c2e6d94b82ec2c3021d308bfa7c2c16d9111863d` on `codex/desktop-shell`,
pushed to `origin`, and tagged with the separate annotated
`desktop-v0.1.1-alpha.1` tag. The tag resolves to that commit and does not
match the core release workflow's `v*` trigger. The GitHub prerelease is
<https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.1-alpha.1>.
It contains only `Forge_0.1.1_x64-setup.exe` and its `.sha256` file. The
release target commit is the source commit above; its body notes that this
was a local prebuilt package, not a reproducible CI build.

**PASS:** draft-upload readback returned 4,619,155 bytes and SHA-256
`F8E04F44E314DB7D530AD251731ACA69C76C6E239FF0FF0E3D461D2E55E70568`.
After publication, an unauthenticated direct public download of both assets
to `C:\ForgeFast\forge-alpha-public-download` returned the same bytes/hash
and matching `.sha256` content. That downloaded installer exited 0 when
installed silently over the existing local alpha. Installed product version
was `0.1.1`, and installed executable SHA-256 was
`6E8EF73A3A8EB0927B3556A13838A5924CBE31A00E4A9752698B1F2F4002AABD`.
Basic hidden-native UI smoke after public-file installation passed (Home,
Explore, first-use draft, appearance reload and WebView IPC); no visible app
window was launched. A prior draft-downloaded install of the same exact
file passed a full hidden-native smoke with real Windows dialogs, existing
and new project onboarding, local previews, a real Codex reply and resume.
The test-only disposable project, sidecar and draft-download directory were
removed after path verification; the public-download directory was retained.
No Forge desktop process remains.

**Diagnostic failures and limits:** the first attempt to run the downloaded
binary's native smoke had no Playwright module environment configured; this
was a test setup failure. With Playwright configured, a newly initialized
empty project failed a test assertion that requires a visible stage; its
record was actually readable, but the UI intentionally hides the stage when
record status is `absent`. A subsequent test against this repository timed
out waiting for a record read. Direct `forge-core 0.12.1 workflow resume`
on this large project then took 64.2 seconds on a cold run and 11.72 seconds
on retry; the app's read timeout is 20 seconds. After retrying the full
hidden-native smoke on the same installed binary, all project, preview,
dialog, real-Codex and resume checks passed. The release body discloses
the cold-read limit; it is **not fixed** in this version. The exact
`0.1.0`→`0.1.1` upgrade, nonempty real decision-suggestion native rendering
and a full accessibility audit remain NOT_RUN. An unsigned installer and
separate `forge-core`/Codex CLI requirements remain known alpha limits.

The working tree was clean immediately after the source commit/push; this
postpublication checkpoint and release-note status update are local doc-only
edits and are not part of the release tag. No further commit/push was made.
No worker was dispatched; per-model tokens and BRL cost remain UNKNOWN.

**Next:** resume UI work as a separate coherent package, starting with the
record-loading UX for slow/cold large projects and verifying that a fresh
project's `absent` stage is explained without exposing a misleading empty
field. Keep publication, new installer and source commit separate until the
next package is verified and approved.

## Next desktop source package: record-loading clarity — 2026-09-27

The source tree has moved to desktop `0.1.2` in `src-tauri/Cargo.toml`, its
lockfile entry, and `tauri.conf.json`. This is a **development version**, not
an installer or published update; the installed/public version remains
`0.1.1`. Existing postpublication documentation edits were preserved.

The cold-read failure observed in the published alpha was traced to the
desktop's uniform 20-second outer/15-second per-attempt CLI budget. A direct
read of this large repository took 64.2 seconds on one cold run and 11.72
seconds on retry. The UI did not cause that core latency. The desktop now
keeps fast-failing 20/15-second budgets for project identity/start, but uses
a bounded 90/85-second budget for the read-only workflow resume/report.
This is a targeted wait allowance, not a change to the core or a claim that
all slow reads will succeed. While a read remains pending beyond six seconds,
the UI says the record is taking longer and that conversation can continue;
it does not invent progress. Timers are cleared on completion, project
change or invalidation. History consultation gets the same honest delayed
message. A fresh project's `absent` state now shows **Sem trabalho
registrado**, labels Discovery as where Forge starts rather than recorded
work, and invites the user to describe their idea in chat. It does not show
an empty current-work timeline or treat a suggestion as a decision.

**PASS:** targeted desktop `cargo check`; `project::tests` (3/3) and
`progress::tests` (10/10) filters, then full desktop Rust tests (43/43)
before the metadata version bump; version-bumped targeted check/tests and
`cargo fmt --check`; strict desktop Clippy before the metadata-only bump;
seven Node unit tests; browser UI suite including delayed read, stale
response and absent state; and a fresh `0.1.2` debug build with hidden native
smoke against an actual Forge project and real Windows folder/file dialogs.
The native smoke verified a new project's initialized record and the new
absent-state guidance. Real Codex turn was NOT_RUN in this package; the
conversation transport was unchanged. `git diff --check` passed with only
line-ending notices. No full core workspace run or release build was made.

Browser-controlled visual captures:
`C:\ForgeFast\forge-record-empty-20260927.png` and
`C:\ForgeFast\forge-record-slow-20260927.png`. These show readable status and
help in the dark theme, but are not native screenshots or proof of final
visual fidelity. The approved conversation reference is
`design/references/conversation-approved.png`; the current conversation
screen still lacks much of that board's botanical/light visual treatment.

**Remaining risk:** a read over 85 seconds can still time out, and an old
read-only request may continue after the user switches projects until its
bounded process exits. There is no controlled 64-second native reproduction
for the new budget; the prior measured cold read, code path, browser wait
test and normal native readback are the current evidence. No source commit,
push, installer or publication for `0.1.2` occurred. No worker was
dispatched; model-specific tokens and BRL cost remain UNKNOWN.

**Next:** make the conversation/result/record surface more faithful to the
approved artwork without turning illustrative mock data into product facts.
Capture current light-theme native or controlled-browser visuals, compare
with the approved board, choose one coherent visual package, implement and
verify it, then decide whether the `0.1.2` package is ready for final build
and a new separately approved release candidate.

## Desktop 0.1.2 conversation visual pass — 2026-09-27

The conversation workspace now borrows text-free foliage crops from the
already-approved Explore art. This frames the real conversation, local preview,
and Forge record more closely to the approved light board without copying its
fictional project, site preview, or progress. The crops are non-interactive,
hidden on narrow/forced-color layouts, and subdued in dark mode. The initial
crop accidentally included part of the source board's caption; that was
removed before the final captures. No conversation, record, preview, or core
behavior changed in this visual pass.

**PASS:** browser suite after the final CSS, including desktop/mobile overflow,
enlarged text, forced colors, dark theme, navigation, conversation and preview;
fresh desktop `0.1.2` debug build; and hidden native smoke of real Windows
folder/file dialogs, project onboarding, Forge readback, local preview and
appearance. The native runner did not open a visible window. Actual Codex send
was NOT_RUN in this pass because the transport was unchanged. The captures
`C:\ForgeFast\forge-conversation-light-visual-final-20260927.png` and
`C:\ForgeFast\forge-conversation-dark-visual-final-20260927.png` are controlled
browser fixtures, not native screenshots or real project content. They were
inspected against `design/references/conversation-approved.png`; they improve
the framing but do not establish finished visual fidelity or accessibility.

The `0.1.2` source package remains uncommitted and unpublished. The public
installer is still `0.1.1`. No worker was dispatched; per-model token and BRL
cost attribution remain UNKNOWN.

**Next:** inspect the integrated `0.1.2` diff and the remaining UI coverage
against the approved direction; fix concrete gaps before choosing a single
release candidate. Keep commit/push, installer generation, and publication
separate, with fresh approval for a future release.

## Desktop 0.1.2 project-recognition pass — 2026-09-27

An integrated UI review found that My Projects still displayed the Forge
internal `project_id` as the card title, while the workspace already used the
folder name. A shared display helper now makes the folder name consistent in
the project list, workspace, and Codex connection status. The full path stays
visible in each list card and its accessible Open label, so equal folder names
can be distinguished. Forge IDs still back identity, bookmarks and native
shortcut revalidation. The list's generic star became the approved Forge icon,
and the heading gained a text-free approved-art crop on wide, non-forced-color
screens. No project registry or backend behavior changed.

**PASS:** browser suite after the patch covers display name, visible path,
accessible project actions, reload, shortcut revalidation, failures, mobile
overflow and connection copy; fresh `0.1.2` debug build and hidden native
smoke cover a real linked project, new-project onboarding, the real Windows
folder/file dialogs, Forge record and local preview. Native readback checks
the saved card title and icon. The browser visual capture
`C:\ForgeFast\forge-projects-friendly-20260927.png` was inspected; it uses a
controlled project fixture, not a real native project. `git diff --check`
passed with line-ending notices. Rust source was not changed in this pass;
therefore the crate suite was not repeated. A real Codex send was NOT_RUN.

The GitHub UI acceptance criteria in #91, #92 and #93 were reread. The
existing UI remains partial for dynamic visual previews, complete accepted
decision snapshots and all restart/failure paths; this package does not mark
those issues complete. The `0.1.2` tree remains local and uncommitted. No
worker was dispatched, and per-model tokens and BRL cost remain UNKNOWN.

**Next:** choose the highest-impact remaining journey gap from #91/#92/#93,
implement and verify it in a coherent package; then review all `0.1.2` changes
as a whole before any installer candidate. Commit, push and publication still
require their separate approval boundaries.

## Desktop 0.1.2 uncertain-send recovery — 2026-09-27

Review against issue #93 found a concrete restart gap: the rejected-send guard
existed only in WebView memory. A crash or app close after native Send began
could erase the warning and allow a manual retry without history review. The UI
now writes a project-scoped **thread ID only** to local storage before calling
`send_message`; it removes that marker after native acknowledgement. If Send is
rejected or the app closes while delivery is uncertain, a reopened app blocks
re-sending and starting another conversation until the same Codex thread is
explicitly resumed for review. A marker that cannot be read fails closed. The
marker is not message content, a Forge project record, or proof of delivery;
Codex remains the history authority. If local storage cannot write it, the
message remains a draft and is **not sent**; the UI explains that the app
could not protect this delivery across a restart. If review succeeds but clearing the
marker fails, the app warns that another review may be needed next time.

**PASS:** eight focused Node tests including marker scoping/validation;
browser suite including marker-before-native-Send, removal on acknowledgement,
rejected Send and a separate browser context restored from the same storage
(blocked retry/new thread, explicit same-thread resume, no automatic Send);
fresh `0.1.2` debug build and hidden native smoke of existing project,
onboarding, record, preview and real Windows dialogs. The browser protocol
responses are controlled doubles, not evidence of a real ambiguous Codex
delivery. A real process crash during Send, unavailable local storage across
restart, and an actual Codex reply were NOT_RUN in this pass. No Rust source or
core workflow changed; no Rust suite was repeated for this frontend change.

The whole `0.1.2` tree is still local, uncommitted, and unpublished. Issue #93
is improved but not closed: native crash/restart and all failure branches still
need stronger evidence. No worker was dispatched; per-model token and BRL
attribution remain UNKNOWN.

Integrated review found and closed a fail-open branch: a storage-write failure
previously allowed native Send to proceed with only an in-memory guard. The
write now precedes both the local pending bubble and native Send; failure
keeps the draft and does not invoke Send. Browser regression covers this
branch. Native crash during Send remains NOT_RUN, so #93 is still partial.

The same review found that a stale record request could clear the six-second
notice timer of a newer project request. Each request now clears only its own
timer. A browser regression switches projects with both reads pending,
resolves the stale read first, and confirms the new read still shows its
honest delayed notice. The complete browser suite and eight Node tests passed
after both fixes; this frontend-only review did not repeat native smoke or
Rust compilation. `git diff --check` remains the final local patch check.

**Next:** run a bounded native ambiguous-send/restart fixture if it can be
driven without a real Codex turn or disruptive visible window; otherwise
retain that gap explicitly. Then perform final package gates and decide
whether `0.1.2` is ready for a candidate. Do not publish a new installer
without the maintainer's approval.

## Desktop 0.1.2 native restart guard — 2026-09-27

The bounded fixture is now `tests/native-restart-guard.cjs`, run through the
hidden desktop runner. It opens a real linked Forge folder in a real Tauri
WebView, intercepts **only** the Codex connect/send commands with a controlled
bridge, leaves Send pending, then kills and restarts the entire app process
with the same isolated WebView profile. On reopen, the thread-only marker
survived; the UI blocked manual replay and a new thread; explicit same-thread
history review cleared it without calling Send. The fixture never forwarded
a Codex command to the real CLI. It removes only its own temporary profile.

**PASS:** fresh `0.1.2` debug build; hidden native process restart guard;
hidden full native smoke with real Windows folder/file dialogs, real Forge
readback and onboarding; a second hidden smoke listed six real Codex project
conversations without sending; prior browser suite and eight Node tests.
The Tauri `core.invoke` function is frozen, so test instrumentation must use
a replaceable `core` facade. Two ineffective direct assignments and a spread
that dropped non-enumerable `Channel` were fixed in the native harness. This
is a test-evidence correction, not an app-runtime change.

**NOT_RUN:** an actual ambiguous Codex delivery or real Codex reply after a
process crash. The controlled native fixture proves WebView/process storage
and UI guards, not transport exactly-once semantics. Issue #93 remains
partial. No commit, push, installer or publication occurred for `0.1.2`.
No worker was dispatched; model-specific tokens and BRL cost remain UNKNOWN.

**Next:** inspect the final integrated source/test diff and run proportionate
package gates for `0.1.2`; decide what is release-ready and list remaining
UI limitations before preparing a candidate. Keep source commit, installer
candidate and publication as separate approval steps.

## Desktop 0.1.2 local NSIS candidate — 2026-09-27

The scoped desktop package gates passed: `cargo fmt --check`, all 43 desktop
Rust tests, strict desktop Clippy, eight Node tests and the complete browser
UI suite. The pinned `npm run build:nsis` produced one local Windows x64
unsigned NSIS file:
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.2_x64-setup.exe`
(4,621,545 bytes; SHA-256
`D86273A6ABC5F6F7D383A48C626DE5FD173C68344EE9B6C2DC700894A88B5FBC`).
The release executable reports product/file version `0.1.2`. Hidden native
smoke on that release executable passed project onboarding, real Windows
folder/file dialogs, Forge record, preview and appearance. The release
executable also passed the controlled full-process uncertain-send restart
fixture. These are payload checks, **not installation-over checks**.

The draft `RELEASE_NOTES-0.1.2.md` separates delivered improvements from
remaining alpha limits. It also corrects an outdated prepublication sentence
in the published `0.1.1` package record. The `0.1.2` installer is not
approved or published. Its exact install over public `0.1.1`, installed
binary smoke, real Codex reply/continuity across that upgrade and downloaded
asset verification remain **NOT_RUN**. Dynamic site preview, complete
accepted-decision details and whole-product visual/accessibility acceptance
remain outside this building block. No core workspace suite was run for this
desktop-only package; no source commit or push occurred. No worker was
dispatched; model-specific tokens and BRL attribution remain UNKNOWN.

**Next:** review the final source/test/doc diff, recheck the candidate hash,
then decide with the maintainer whether to install this local candidate over
the existing alpha for an upgrade test and whether the draft notes are
approved for publication. Do not claim the installer is available to users
before an approved upload and downloaded-file check.

## Desktop record provenance check — 2026-09-27

The maintainer's delayed approval for `0.1.1` was checked against GitHub:
`desktop-v0.1.1-alpha.1` is already a published prerelease with its installer
and checksum asset. This approval is **not** approval to replace the locally
installed `0.1.1` with the distinct `0.1.2` candidate or to publish `0.1.2`.
The `0.1.2` candidate hash was rechecked and remains
`D86273A6ABC5F6F7D383A48C626DE5FD173C68344EE9B6C2DC700894A88B5FBC`.

A controlled browser regression now covers a valid record with **no accepted
direction** but one recovered pending decision and a current suggestion: the
accepted-direction panel stays hidden, the pending/suggested panel stays
visible with provenance language, and reading it sends nothing. The complete
browser suite passed after this test-only edit. No application source or
installer payload changed, and no installation was attempted. The backend's
historical decision projection exposes references/status, not the original
human-readable question and option text; joining a later suggestion to an
earlier decision would fabricate provenance. Full decision-history UI remains
open rather than being simulated from current suggestions.

**Next:** wait for explicit approval to install `0.1.2` over the currently
installed `0.1.1`, then run a hidden installed-binary upgrade smoke. If not
approved, continue other UI work without altering the user's installation.
Source commit/push and `0.1.2` publication remain separate decisions. No
worker was dispatched; per-model token and BRL attribution remain UNKNOWN.

## Desktop 0.1.2 real Codex reply from local release payload — 2026-09-27

The unchanged `0.1.2` release executable was exercised in the hidden native
desktop with a disposable Forge folder and isolated WebView profile. Its
first Send opened a **real Codex** conversation, received a reply, restored
the user/reply pair after WebView reload, and restored their order again after
full app-process restart without another Send. The hidden run also passed
new-folder onboarding, read-only Forge record, local preview boundaries and
native UI checks. It did **not** test an installed upgrade or an ambiguous
transport result. The test prompt explicitly forbade tools and file edits.

The first run stopped before Send because the native test harness wrongly
required an active workflow direction in a newly linked empty folder. The
app correctly rendered `Sem trabalho registrado`; the harness now asserts
that honest empty state separately from a populated record. The rerun passed.
Only the test harness and release-note text changed; no application source or
candidate installer was rebuilt. The disposable root was checked to be a
direct child of `C:\ForgeFast`, then removed with its temporary marker. No
Forge desktop process remained after the run. The candidate installer hash
remains the one recorded above; installed `0.1.1` was untouched.

**Next:** obtain explicit approval before the exact `0.1.2` installer is
installed over `0.1.1`; run hidden installed-binary and cross-version
continuity checks. If that is declined, continue UI work without changing the
user's installed app. Source commit/push and public release remain separate
approval boundaries. No worker was dispatched; per-model token and BRL
attribution remain UNKNOWN.

## Desktop integrated review and viewport finding — 2026-09-27

Reviewed the pending chat send guard, conversation bookmark, project-name
display, record timeout and empty-state changes against the existing tests
and local release payload. No additional correctness defect was confirmed.
The controlled visual review did identify a concrete experience gap: with a
confirmed project at 1280 x 844, the conversation begins around document
y=261 but **Enviar** begins around y=1217. A quick CSS sticky probe brought
the button into the first viewport only by covering the message history, so
it was discarded. Exact evidence and the next cohesive layout acceptance
scope are in `design/README.md`. No application source changed in this review;
temporary measurement code was removed from the browser test. The `0.1.2`
candidate remains byte-identical, and the installed `0.1.1` remains intact.

**Next:** with maintainer approval, test this exact `0.1.2` installer over
installed `0.1.1` before any source changes. Then tackle the chat viewport
layout as a cohesive follow-on UI package rather than an overlay workaround.
No worker was dispatched; per-model token and BRL attribution remain UNKNOWN.

## Desktop confirmed-project viewport package — 2026-09-27

The earlier local `0.1.2` NSIS file is **superseded**, not a candidate for
installation or publication: the application source changed after it was
built. Do not interpret the maintainer's delayed approval of the already
published `0.1.1` as approval for a different `0.1.2` artifact. The installed
`0.1.1` was not changed in this package.

In a confirmed project, the heading and chat card now reserve visible space
for the invitation or bounded message history, composer, access warning and
**Enviar** at the default 1180 x 820 desktop size. The optional conversation
history explanation moved into its existing disclosure; it no longer consumes
the message scroll area. A selected-idea draft stays in the composer while its
redundant confirmation notice remains accessible but visually hidden. The
scrollable message area does not overlap the composer. No backend, Codex
transport or Forge-record authority changed.

**PASS:** complete controlled browser UI suite, including 1180 x 820 and
1280 x 844 geometry, mobile/enlarged-text, keyboard and controlled 160-message
history; fresh `forge-desktop` debug build; hidden native WebView smoke at
1180 x 820, including preserved draft, visible Send/invitation, real Forge
project and record readback, onboarding, preview and appearance. Controlled
browser capture: `C:\ForgeFast\forge-composer-viewport-20260927-v2.png`.
Hidden native capture: `C:\ForgeFast\forge-native-viewport-20260927-v2.png`.
The first hidden native run caught the preserved-draft banner pushing Send
below the viewport; this was corrected and the complete hidden smoke passed
on the rebuilt debug executable. A real Codex turn was NOT_RUN in this latest
layout run; earlier real-turn evidence belongs to the superseded release
payload. The browser suite, eight Node tests and `git diff --check` passed
again after the final UI and documentation edits.

**Next:** review the integrated pending `0.1.2` source/tests/docs, run final
package checks only at that boundary, build and hash a new installer candidate,
then seek separate approval before installing over `0.1.1` or publishing.
No source commit, push or `0.1.2` release occurred. No worker was dispatched;
per-model token and BRL attribution remain UNKNOWN.

## Desktop 0.1.2 rebuilt local candidate — 2026-09-27

The current `0.1.2` source passed `cargo fmt --check`, all 43 desktop-crate
Rust tests, strict desktop Clippy, eight Node tests, and the full controlled
browser suite. The pinned NSIS build completed after the viewport changes.
Current local unsigned candidate:
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.2_x64-setup.exe`
(4,620,623 bytes; SHA-256
`75F9DCBFB1CAF09D97DDD5ADCFCC79C45CF313788952CF10420292313662180C`).
The release executable reports product/file version `0.1.2`. The earlier
`D86273A6...B5FBC` candidate is superseded and must not be used for this
source.

**PASS:** hidden native smoke on the new release executable, including visible
Send/invitation at the default window size, real Forge project/record readback,
onboarding, safe preview and appearance. A hidden full-process restart with a
controlled Codex bridge preserved the uncertain-send marker and blocked replay.
The first guard invocation omitted `FORGE_TEST_PROJECT`; that harness setup
error was corrected and the guarded run passed. This build did not send a
real Codex turn. Installer-over-`0.1.1`, installed-binary smoke, cross-version
conversation continuity, public upload and downloaded-file verification are
**NOT_RUN**. The public/installed `0.1.1` remains unchanged.

**Next:** review the exact candidate and release notes with the maintainer;
obtain explicit approval before installing over `0.1.1` for the hidden upgrade
test. Publication is a separate step after the installed candidate passes.
The pending source is still uncommitted/unpushed; do not call this local file
an available update. No worker was dispatched; model-specific tokens and BRL
attribution remain UNKNOWN.

## Desktop pre-upgrade read-only audit — 2026-09-27

Before touching the user's installation, rechecked the installed executable at
`%LOCALAPPDATA%\Forge\forge-desktop.exe`: product/file version `0.1.1`, SHA-256
`6E8EF73A3A8EB0927B3556A13838A5924CBE31A00E4A9752698B1F2F4002AABD`.
No Forge desktop process was running. The new local `0.1.2` candidate still
hashes to `75F9DCBFB1CAF09D97DDD5ADCFCC79C45CF313788952CF10420292313662180C`.
Read-only review of the pending UI/native diff and #97/#95/#91 acceptance
boundaries found no additional confirmed defect; this does not complete those
stories. The installed app and candidate bytes were not changed. Explicit
approval for installing this exact candidate is pending. If approved, create
a disposable real-Codex turn with installed `0.1.1` using the existing
`upgrade-continuity.cjs` fixture, install the exact candidate invisibly, then
resume the same turn and run installed-binary smoke. Keep publication separate.

## Desktop 0.1.1 pre-upgrade conversation fixture — 2026-09-27

Prepared the **pre-upgrade half only** of the existing `upgrade-continuity.cjs`
test on an isolated hidden desktop. The actually installed `0.1.1` binary
opened a disposable Forge project, sent one no-tools/no-file-edits prompt to
real Codex and displayed exactly one user turn and one reply containing the
expected test marker. The test process exited and no Forge desktop process
remains. The installed executable still hashes to
`6E8EF73A3A8EB0927B3556A13838A5924CBE31A00E4A9752698B1F2F4002AABD`;
the local `0.1.2` installer still hashes to
`75F9DCBFB1CAF09D97DDD5ADCFCC79C45CF313788952CF10420292313662180C`.

Retain the disposable project at
`C:\ForgeFast\forge-upgrade-011-to-012-20260927-project` and isolated WebView
profile at `C:\ForgeFast\forge-upgrade-011-to-012-20260927-profile` until the
post-upgrade resume check or a decision to abandon it. Only these test-owned
paths and any directly associated Forge sidecar should be cleaned after
verification, with resolved paths checked first. This is not cross-version
continuity proof yet: `0.1.2` has not been installed. Explicit permission to
install the exact candidate is still pending. No source, installer bytes,
public release, or user's normal WebView profile changed.

## Desktop 0.1.1-to-0.1.2 pre-install continuity — 2026-09-27

The **local release executable** built with the current `0.1.2` candidate
(not an installed binary) opened the same disposable Forge project and isolated
WebView profile used by installed `0.1.1`. It resumed the real Codex
conversation and found exactly the original user/reply pair with its test
marker; after a bounded wait, no extra turn appeared. No Send was invoked.
The hidden process exited, and the installed `0.1.1` and NSIS candidate hashes
remain unchanged. This is PASS for cross-version conversation-format readback,
but **NOT_RUN** for installer-over-update and installed `0.1.2` readback.

The existing test harness had printed "new installed binary" even when pointed
at a local executable; that test-only log text now says "new binary". The
fixture remains available for a later installed-binary resume check. Explicit
approval to install the exact `0.1.2` candidate is still pending. No app source
or installer bytes changed, and nothing was published.

## Desktop focused zoom-equivalent and native rerun — 2026-09-27

The local `0.1.2` release executable resumed the same disposable real-Codex
conversation under an emulated 590 x 410 CSS viewport. A long unsent draft
remained intact, Send was reachable without sideways page overflow, and the
original two messages stayed unchanged. This **PASS** is a zoom-equivalent
WebView check, not an actual OS/browser zoom, virtual-keyboard, or complete
accessibility audit. The isolated fixture was retained for the eventual
installed-upgrade check. No message was sent in this rerun.

The broad hidden native smoke was rerun against the same release executable
without the experimental viewport override and **PASSED**: project onboarding,
record readback, conversation/UI states, local preview and appearance. During
diagnostic iterations with the override in that broad harness, a CSS-load
timing assertion and a project-ready wait failed intermittently; the CSS test
now waits for the actual computed style, and the onboarding wait reports its
status if it fails. The override was kept only in the focused continuity test.
Those transient harness failures are not claimed as proven product defects or
silently counted as passes. The broad smoke's actual Codex-turn branch was
**NOT_RUN** in this rerun; the separate continuity fixture covers its prior
real turn. Test code and this checkpoint changed, but application source and
candidate bytes did not. The candidate SHA-256 remains
`75F9DCBFB1CAF09D97DDD5ADCFCC79C45CF313788952CF10420292313662180C`.

The delayed human approval was explicitly for publishing `0.1.1`, which is
already public; it does **not** authorize installing or publishing `0.1.2`.
The installed app remains `0.1.1`. **Next:** await explicit approval for
installing this exact `0.1.2` candidate, then perform a hidden over-install,
resume the retained real conversation, and run installed-binary smoke. Review
release content and seek separate `0.1.2` publication approval afterward.
No worker was dispatched; model-specific token/BRL attribution remains UNKNOWN.

## Desktop 0.1.2 real Windows dialogs — 2026-09-27

The unchanged local `0.1.2` release executable passed the complete hidden
native smoke with `FORGE_TEST_FOLDER_DIALOG=select` and
`FORGE_TEST_PREVIEW_DIALOG=select`. The actual Windows folder dialog was opened,
cancelled, then used to select the linked Forge project and an existing
unlinked folder. Selection alone did not create or inspect a project; the
single **Continuar nesta pasta** action did so, and the pre-existing file was
preserved. The actual Windows file dialog selected a local project file for
read-only preview; outside-path rejection and the other smoke assertions also
passed. No foreground window was shown. The Codex message branch was
**NOT_RUN** in this smoke; separate real-turn continuity evidence above still
applies. No installed upgrade, commit, push or publication occurred.

The NSIS candidate still has SHA-256
`75F9DCBFB1CAF09D97DDD5ADCFCC79C45CF313788952CF10420292313662180C`.
The installed application remains `0.1.1`. **Next:** with explicit approval
for installing this exact `0.1.2` candidate, test its hidden over-install and
resume the retained real conversation in the installed binary. This approval
has not arrived; the earlier `0.1.1` publication approval cannot substitute.

## Desktop new-idea project boundary and rebuilt 0.1.2 candidate — 2026-09-27

Found a concrete UI boundary error: with a confirmed project open, choosing a
fresh Explore theme populated the draft but left the old project active. A
browser regression first failed on that observable state. The Home
**Conversar sobre uma ideia** action had the same ambiguity. The UI now treats
those explicit fresh-idea actions as a return to folder choice. It keeps the
draft, clears the old project and displayed transcript, and disables Send until
a folder is confirmed. The normal **Minha conversa** navigation still returns
to the current project. If Codex is connected, it disconnects before the
switch; when a turn is running, declining the existing interruption prompt
leaves the old project and draft unchanged. No native Forge-core contract or
conversation-history authority changed.

**PASS:** red-before-green controlled browser checks for the old-project
leakage, Home/Explore fresh-idea paths, connected disconnect, declined busy
switch, cleared transcript and no implicit project creation. Complete browser
suite, eight Node tests, JS syntax, `git diff --check`, fresh debug build and
hidden native WebView smoke passed. After the final frontend change, the pinned
release NSIS build also passed. The release executable passed a full hidden
native smoke with the actual Windows folder and file dialogs, real Forge
project/record readback and the new-idea boundary. Native screenshot reviewed:
`C:/ForgeFast/forge-new-idea-native-20260927.png` (a disposable project and
draft; not a published asset). The new release executable also resumed the
retained real Codex user/reply pair created by installed `0.1.1` without a new
send. The current candidate is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.2_x64-setup.exe`
(4,621,851 bytes; SHA-256
`C88219654ED0525C49DB6A6D6CDADB1386607B59587774D1B6390C176AD42B5A`).
The prior `75F9DCBF...662180C` candidate is superseded. At this checkpoint,
a fresh real Codex turn after the new UI change was **NOT_RUN**; the separate
resumed conversation was not sent again.

**Pending:** this exact new installer has not been installed over `0.1.1`,
and installed-binary upgrade continuity has not been checked. Source is still
uncommitted/unpushed; no release was published. The earlier request for
permission to install the old exact hash is stale and must not authorize this
new file. **Next:** obtain explicit approval for installing this exact
`C8821965...42B5A` candidate, test hidden over-install and resume the saved
conversation using the installed binary; then review and seek separate
publication approval. No worker was dispatched; per-model token and BRL
attribution remain UNKNOWN.

## Desktop 0.1.2 real reply on the rebuilt release executable — 2026-09-27

The current local release executable (the binary built with NSIS candidate
SHA-256 `C88219654ED0525C49DB6A6D6CDADB1386607B59587774D1B6390C176AD42B5A`)
passed a hidden native real-Codex smoke in a disposable project. The first
Send opened a conversation, delivered one no-tools/no-file-edits prompt,
displayed its reply, disconnected, reloaded the WebView and restored exactly
the user/reply pair without a second send. This is **PASS** for the ordinary
project-to-chat path on the current release executable. It is not a real send
*through* the separate new-idea project-switch path; that path passed native
UI/project-boundary checks. It is also not an installed-upgrade or full-process
restart check. No user project, installed `0.1.1`, candidate bytes, source,
commit, push or public release changed in this probe. Per-model token/BRL
cost remains UNKNOWN; no usage number is invented.

**Next:** with explicit approval for the exact current candidate, run a hidden
over-install onto `0.1.1` and verify the retained pre-upgrade real conversation
from the installed `0.1.2`. The old-hash approval request remains stale.

## Desktop fresh idea to real reply — 2026-09-27

The same current `0.1.2` local release executable passed a second hidden
native end-to-end check of the newly corrected route. With an existing project
open, **Explorar → Arte e criação** preserved its suggested draft, cleared the
old project, required a newly selected disposable folder, initialized it via
the real Forge core, and sent one separate no-tools/no-file-edits prompt to
real Codex in that new project. Exactly one user message and one agent reply
were displayed, and the connection was closed. This **PASS** verifies the
fresh-idea → different real project → conversation path, not just browser
navigation or a mocked IPC response. The controlled prompt replaced the
suggested artistic draft before Send; the suggested draft itself was not sent.

This did not change the installed `0.1.1`, public release, NSIS candidate bytes
or application source. The candidate remains SHA-256
`C88219654ED0525C49DB6A6D6CDADB1386607B59587774D1B6390C176AD42B5A`.
The test harness gained an opt-in real-send branch and a clearer NOT_RUN log.
Installed-upgrade continuity remains **NOT_RUN** until explicit approval for
this exact candidate. No commit/push/publication occurred; per-model token
and BRL cost remain UNKNOWN.

## Desktop open-another-project boundary — 2026-09-27

Found a second project-switch UI error: **Meus projetos → Abrir outro projeto**
opened folder choice but retained the previous path, confirmed-project panel,
and conversation until someone edited the input. A controlled browser
regression first failed on the retained path. The navigation now reuses the
same reset as a fresh Explore idea **after** any running-turn interruption is
accepted and the connection is closed. It clears the prior path and displayed
transcript and disables Send until another folder is confirmed. Refusing the
running-turn interruption still leaves the prior project/draft unchanged.
No native Forge core or Codex protocol behavior changed.

**PASS:** complete controlled browser suite after the red test; fresh desktop
debug build; full hidden native smoke of that debug executable with the new
project-switch assertion; pinned release NSIS build; full hidden native smoke
of the new release executable with the same assertion; and `git diff --check`.
Those two native runs used real Forge project/record readback but did **not**
send a new real Codex message. The earlier real-turn checks belonged to the
superseded release executable; they must not be presented as a fresh real-turn
check of these new installer bytes. The new local candidate is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.2_x64-setup.exe`
(4,623,866 bytes; SHA-256
`F8C57185E3CDD96C5329A8D70C5DC426EAD793BB5DFEF894E02EB93A3B251EA1`).
The previous `C8821965...42B5A` candidate and any request to install its
exact bytes are obsolete. The installed app remains `0.1.1`; **no** `0.1.2`
install, commit, push, or publication occurred. The retained disposable
`0.1.1` real-Codex conversation fixture is still available for an eventual
installed-binary upgrade check. No worker was dispatched; per-model token/BRL
attribution remains UNKNOWN.

**Next:** ask for explicit approval of a hidden over-install of this exact new
`F8C57185...B251EA1` candidate, then resume the retained conversation in the
actually installed `0.1.2` binary and run installed-binary smoke. Review code
and release contents and seek separate publication approval afterward.

## Desktop saved-project shortcut audit — 2026-09-27

A read-heavy check questioned whether a saved-project shortcut might switch
folders while Codex remained connected. The owning controls actually disable
the path field while connected, which routes the shortcut through the existing
`prepareProjectSwitch` disconnect guard. A controlled browser test now proves
that lock, one disconnect, and read-only revalidation of the selected saved
project; the complete browser suite passes. The suspected cross-project-send
bug was **not reproduced**, so no application-source change was made. This
test-only change does not affect the pinned NSIS bytes: SHA-256 remains
`F8C57185E3CDD96C5329A8D70C5DC426EAD793BB5DFEF894E02EB93A3B251EA1`.
Explicit approval to silently install that exact candidate for upgrade testing
has been requested and is still pending. No install, commit, push, publication,
or new real Codex send occurred in this audit.

## Desktop current-candidate pre-install conversation readback — 2026-09-27

The **current** `0.1.2` local release executable, built with NSIS candidate
SHA-256 `F8C57185E3CDD96C5329A8D70C5DC426EAD793BB5DFEF894E02EB93A3B251EA1`,
passed the hidden `upgrade-continuity.cjs` **resume** stage against the retained
disposable project and isolated WebView profile. Installed `0.1.1` had created
the real Codex user/reply pair there. The new local executable displayed
exactly that pair and, after a bounded wait, no extra turn; it invoked no Send.
This is **PASS** for current-candidate cross-version conversation readback,
but **NOT_RUN** for installing `0.1.2` over `0.1.1` and reading the conversation
from the actually installed binary. The fixture remains intact for that gate.
The installed app and public `0.1.1` remain unchanged. No new model call,
commit, push, or publication occurred. Explicit approval for the exact hidden
over-install is still pending; the earlier `0.1.1` release approval does not
authorize it. Next after approval: silently install this exact candidate,
verify installed hash/version and repeat the resume check from the installed
path, then run installed-binary native smoke.

## Desktop current-candidate real Codex turn — 2026-09-27

The same current `0.1.2` local release executable (NSIS SHA-256
`F8C57185E3CDD96C5329A8D70C5DC426EAD793BB5DFEF894E02EB93A3B251EA1`)
passed a hidden native smoke with a fresh disposable Forge project and one
no-tools/no-file-edits real Codex prompt. The first Send opened a conversation,
displayed one reply, then a WebView reload restored the user/reply pair without
resending. Other native smoke assertions for project onboarding, record,
preview, appearance and frontend-to-Rust identity passed in the same run.
This is **PASS** for a fresh local-binary conversation, not for an installed
`0.1.2` update, cross-process restart, or a real send through every UI entry
route. The test submitted one real Codex turn; the number of internal model
requests, per-model token count and BRL cost remain UNKNOWN. The isolated test
profile was removed by the harness; the
retained pre-upgrade fixture was not changed. No installation, commit, push or
publication occurred. The exact hidden over-install still awaits approval.

## Desktop next UI gap: historical decisions — 2026-09-27

Read-only source inspection for issue #92 confirmed why the current desktop
must not label opaque decision IDs as past agreements. The core's
`WorkflowReplacementDecisionAudit` exposes policy/decision refs, status,
record digests and selected-alternative ref, but **not** the human-readable
question or alternative. The underlying `DecisionNeedRaisedEvent` also carries
a `question_digest`; policy `decision_rules` contain the semantic text, and the
core's resolution path checks a rule against the selected alternative. The
desktop `workflow report` projection consumed today does not bind historical
audit entries to that text. A current simulation suggestion is not a safe
substitute for a past accepted choice, especially if policy material changed.

For full #92 coverage, the next coherent implementation slice needs a bounded
**read-only authoritative projection/resolver** that proves each historical
question/choice belongs to its ledger entry and preserves origin, status and
revisions; only then should the UI render accepted, pending and revised
decisions in plain language. Do not add a second decision store or write to
governance state. This is an engineering direction from source inspection,
**not** an implemented feature or native proof. Keep the `0.1.2` candidate
frozen for the pending installation gate; no source or installer bytes changed
in this investigation. The current #92 direction-history view remains partial.

A further read-only command-path check found `decision-resolve` in the CLI,
but no ordinary `decision-need` command or production construction of a
`DecisionNeedRaisedEvent`; the constructors found in this checkout are tests
and a retained fixture. This is **not** proof that historical ledgers cannot
contain such events (the kernel can read them), but it means a desktop-only
rendering change would not create a usable normal journey for them. Before
implementing the full #92 decision view, establish the supported production
authoring path and a ledger-to-text binding; do not infer accepted human
decisions from current candidate questions. No application or installer bytes
were changed by this read-only investigation.

## Desktop current-candidate real artifact continuation — 2026-09-27

An opt-in hidden native smoke of the current `0.1.2` release executable
reached the real Codex artifact turn in a disposable project, and Codex wrote
`site/index.html` plus local CSS. The broad test runner was then interrupted
before a final PASS/FAIL could be recorded; its log stops after the earlier
native preview checks. On reopening that same profile, the artifact turn's
last agent message was **incomplete** and had no file link. Do not count the
interrupted broad run as a completed end-to-end artifact test.

A new focused hidden-native continuation (`tests/artifact-resume.cjs`) resumed
that exact conversation without replaying file creation. It sent one bounded
no-tools follow-up asking only for a relative link to the already-created
file, received the reply, opened the local HTML from the reply's file action,
confirmed the preview's `site\\index.html` origin and unpublished label, and
returned **Pedir mudança neste arquivo** to the same unsent composer. Message
count did not increase when the change request was prepared. **PASS** for
resumed real Codex reply -> native local preview -> unsent change request;
**NOT_RUN** for an uninterrupted original artifact turn through its own final
reply in this candidate. The test profile is disposable, not a user project.
No application source, installer bytes, installation or public release changed.
The candidate remains SHA-256
`F8C57185E3CDD96C5329A8D70C5DC426EAD793BB5DFEF894E02EB93A3B251EA1`.
The hidden installed-upgrade gate still awaits explicit approval. Per-model
tokens and BRL cost for this smoke (initial greeting, artifact request and
follow-up) are UNKNOWN.

## Desktop 0.1.2 installed upgrade gate — 2026-09-27

The maintainer authorized a silent local install of the exact `0.1.2` NSIS
candidate and then approved publication without repeated per-step approval.
The pinned installer SHA-256 was checked immediately before installation:
`F8C57185E3CDD96C5329A8D70C5DC426EAD793BB5DFEF894E02EB93A3B251EA1`.
Installed `0.1.1` executable hash was
`6E8EF73A3A8EB0927B3556A13838A5924CBE31A00E4A9752698B1F2F4002AABD`;
no Forge process was running. Silent NSIS over-install exited 0 without a
visible window. The installed executable at `%LOCALAPPDATA%\Forge\forge-desktop.exe`
now reports product/file version `0.1.2` and SHA-256
`E21FFB1CFF1710A49197F651D66D449EC1D7FE2DB579F885A78ED20B36C40774`.
The installer hash remained unchanged.

**PASS:** `upgrade-continuity.cjs` ran headlessly with the **installed** binary,
retained disposable pre-upgrade project and isolated WebView profile. It
restored exactly the real Codex user/reply pair created under installed `0.1.1`
without a new Send or extra turn. A separate complete `native.cjs` hidden
smoke against the installed binary passed actual Forge record readback,
project onboarding, actual Windows folder/file dialogs, preview, appearance,
project switching and frontend-to-Rust identity. That second run did not send
a real Codex message. Focused Node syntax and conversation-reference tests and
`git diff --check` passed. The public downloaded-file check is **NOT_RUN**
until after upload. No source commit, push, tag or publication had occurred at
this checkpoint. Per-model token/BRL cost remains UNKNOWN.

**Next:** commit the reviewed desktop package once, push once, tag and publish
the exact pinned prebuilt installer as a prerelease (not a CI rebuild), then
download that public asset to a fresh path, compare size/hash and install/test
the downloaded bytes headlessly. Keep the unsigned/no-auto-update and
separately-required `forge-core`/Codex limits in the release text.

## Desktop 0.1.2 alpha published and public-download verification — 2026-09-27

The maintainer approved publication and asked not to run slow GitHub CI for
every small change. The reviewed desktop package was committed as
`31594f953c611f5a6be8d651dbde623ba4e1b610` on `codex/desktop-shell` and
pushed once. Local prepublication checks included eight Node tests, the full
controlled browser UI suite, the installed-binary hidden native smoke, real
cross-version conversation resume, and staged `git diff --check`. Earlier
package-boundary desktop Rust, Clippy and release build checks are recorded
above. The annotated `desktop-v0.1.2-alpha.1` tag points to that source commit;
its name does not match the core release workflow's `v*` trigger. No desktop
CI rebuild was requested. The published prerelease is
<https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.2-alpha.1>.

**PASS:** the release contains only `Forge_0.1.2_x64-setup.exe` and its
`.sha256` sidecar. The GitHub asset reports 4,623,866 bytes and SHA-256
`F8C57185E3CDD96C5329A8D70C5DC426EAD793BB5DFEF894E02EB93A3B251EA1`.
An unauthenticated direct download of both assets to fresh
`C:\ForgeFast\forge-alpha-012-public-20260927` returned exactly that size/hash
and matching sidecar content. The downloaded installer exited 0 when silently
installed; installed product version remained `0.1.2` and executable SHA-256
remained `E21FFB1CFF1710A49197F651D66D449EC1D7FE2DB579F885A78ED20B36C40774`.
The hidden installed-binary resume check again found exactly the real
pre-upgrade user/reply pair without a new Send. The public artifact is now
available; this is not a claim of full UI coverage or exactly-once delivery.

**Remaining:** visual/product coverage beyond this package, authoritative
historical decision details for #92, an untested real process-crash mid-send,
and no automatic updater/signature. Codex CLI and `forge-core` are still
separate dependencies. Per-model token count and BRL cost are UNKNOWN. Next
development slice should target a coherent remaining UI journey, not rebuild
or republish this verified `0.1.2` package for each small edit.

## Desktop next-goal slice: visible conversation recovery — 2026-09-27

The maintainer set a new ongoing objective: make the Windows desktop journey
understandable end-to-end for a nontechnical person, from choosing a folder
through conversation, real result preview, a change request in the same chat,
and recovery after restart. Mobile and self-contained distribution follow as
separate later steps. The first bounded slice addresses #93's recovery entry:
the app already stored a project-scoped Codex conversation bookmark and could
resume read-only, but hid that action inside a closed **Conversas e histórico**
disclosure. A returning user instead saw an empty chat and could send before
seeing past work.

The current local source adds a visible **Continuar conversa anterior** button
in the conversation heading only when a saved project-scoped reference exists.
For an unconfirmed previous Send it reads **Conferir envio anterior**. Both
reuse `connectCurrent` without sending, and the latter retains the native
unconfirmed-send guard. No new history store, Forge record, auto-connect, or
automatic replay was added. A project without a verified reference does not
show the shortcut. The history disclosure remains for selecting another chat
or starting a new one.

**PASS:** the browser suite first failed on the missing visible action; after
the source change, it passed recovery with and without an uncertain send,
restart-like reload, no automatic Send, keyboard, long history, mobile and
layout checks. `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml
-p forge-desktop --offline --locked -j2` passed with the pinned target cache.
`native-restart-guard.cjs` then passed in the hidden native WebView with a full
process restart: the visible shortcut resumed the same controlled thread and
did not send. The controlled bridge does not prove a real Codex ambiguous
delivery. No GitHub CI, installer build, commit, push or publication was done;
the installed/public `0.1.2` is unchanged. Files touched: `ui/index.html`,
`ui/chat.mjs`, `ui/styles.css`, `tests/browser.cjs`,
`tests/native-restart-guard.cjs`, and this checkpoint. One primary executor,
no subagents. Per-model usage and BRL cost for this slice are UNKNOWN.

**Next smallest step:** inspect the real result-to-change journey (#91) from
the user's perspective and close one concrete remaining gap, with a focused
native proof. Do not infer full UI completion or issue closure from this
recovery shortcut alone.

## Desktop next-goal slice: find a real result from a Codex reply — 2026-09-27

Issue #91's local preview already opened an agent-supplied Markdown file link
and validated it natively against the confirmed project. A real Codex artifact
attempt in the preceding alpha had required an extra no-tools follow-up just
to obtain a Markdown link. Source inspection showed that completed replies
formatted inline-code references such as `site/index.html` as inert code,
despite using the same safe path parser for explicit links. This was a
discoverability gap, not a missing preview engine.

The current local source reuses `message-format.mjs`'s local-file action for
supported inline-code paths in completed agent messages. URLs, ordinary
non-path code, and fenced code remain inert. Clicking still goes through
`previewLinkedFile` and the native project/file validation; it does not infer
that the file exists or has been published. No new path parser, file store,
network navigation or privileged preview access was added. `tests/browser.cjs`
first failed because only the original three link actions existed, then passed
with the inline-code action, safe URL handling, keyboard order and project
path readback. A pinned offline desktop debug build and the full hidden native
smoke passed; its controlled native inline-code action reopened a real local
file, while the external URL produced no action. The native smoke did not
send a new real Codex turn; an uninterrupted real artifact-to-change round
trip with this local source is still **NOT_RUN**.

Files additionally touched: `ui/message-format.mjs`, `tests/native.cjs`,
`design/README.md`, and this checkpoint. One primary executor; no subagents.
No installer, commit, push or publication. The installed/public `0.1.2` is
unchanged. Per-model token usage and BRL cost remain UNKNOWN.

**Next smallest step:** run a bounded hidden native end-to-end exercise with
one real artifact turn, inspect the result and prepare/send a change request
in the same Codex conversation. If that path is nondeterministic or hangs,
record the limit and repair only an observed product defect; do not add a
second chat or preview authority to paper over test instability.

## Desktop real result-to-change journey — 2026-09-27

The next bounded test ran headlessly in an isolated native WebView and a fresh
disposable project. Real Codex created `site/index.html`; the app opened it
from the completed reply in the isolated local preview. The preview's
**Pedir mudança neste arquivo** action prepared a draft without sending it.
The test then sent a specific edit request, received the next reply in the
same visible conversation, and read back the changed title from the actual
file. After disconnect and WebView reload, the same chat's messages were
restored without another Send. **PASS** for this exercised path. The first
test run stopped at the prepared draft; the second run added and passed the
actual follow-up and file readback. Both used the hidden native runner and
the local debug executable; no visible app window was brought forward.

This does not establish that arbitrary Codex outputs will contain a suitable
path, that all generated sites render correctly, or that a mid-send crash is
exactly-once. The folder picker response in this smoke was simulated; its
separate real-dialog evidence remains in the earlier checkpoint. The test now
allows a longer timeout only when the real artifact journey is explicitly
enabled. No GitHub CI, installer, commit, push, or publication was done. The
installed/public `0.1.2` remains unchanged. Usage by model and BRL cost for
this run are **UNKNOWN**; a PASS must not be used as a savings estimate.

**Next:** review the remaining product-visible gap in #92's historical
decision details and the end-to-end UX against the approved design, then
prepare a coherent alpha package once the integrated state merits it.

## Desktop short-window conversation review — 2026-09-27

Reviewed current Home, Explore, confirmed conversation and recorded-history
screens in headless browser captures against the approved conversation board.
Home and Explore retain the approved botanical/palette direction. A concrete
defect appeared at 1280×720 with a multiline draft: the fixed-height sticky
chat card compressed the empty invitation so its last line was clipped by the
composer border. A geometry assertion failed before the fix. The CSS now
lets the confirmed chat card use normal page scrolling at desktop heights up
to 760px and gives its history a 150px minimum; taller windows keep the
existing sticky layout. The visible resume button also meets the documented
48px target. The same browser suite then passed, and the after-capture shows
the invitation and Send fully inside the card. A fresh pinned offline debug
build and hidden native smoke passed; the latter covered native WebView,
project onboarding, Forge readback and local previews, but did **not** send a
new real Codex turn. No GitHub CI was run for this UI-only edit.

Issue #92 remains **PARTIAL**: the app displays accepted objective revisions,
current lifecycle phase, pending counts and suggested questions separately.
The backend `workflow report` decision audit currently supplies policy and
alternative references, status and ledger digests, not user-facing wording
for every historical resolution. No resolved choice was relabeled into an
invented plain-language agreement. That needs an authority-backed projection
before the complete decisions view can be claimed. Browser captures are
controlled-data layout evidence, not native data proof. Local changes remain
uncommitted and unpublished; public `0.1.2` is unchanged. Model usage and BRL
cost remain **UNKNOWN**.

**Next:** identify a safe authority-backed way to show accepted and revised
decision details for #92, or document the precise missing contract and move
the integrated UI package toward the next alpha without pretending #92 is
done. Keep package, installed-app and publication evidence separate.

## Desktop 0.1.3 local installed-candidate gate — 2026-09-27

The #92 audit confirmed the current core replacement decision history
contains `policy_ref`, `decision_ref`, unresolved/resolved status, ledger
digests, sequence and optional `selected_alternative_ref`, but not immutable
user-facing question/choice wording. The live report on this repository had
zero decision-history records. Current simulation questions cannot safely be
reconstructed as historical agreements. Therefore #92 remains PARTIAL;
desktop did not add a duplicate decision store or display invented wording.

The coherent UI package was versioned to `0.1.3` in the desktop manifest,
lockfile and Tauri config. `RELEASE_NOTES-0.1.3.md` describes the changes and
limits. **PASS before packaging:** offline locked desktop `cargo check`, 10
focused progress tests, all 43 desktop Rust tests, eight Node tests,
`cargo fmt --check`, strict desktop Clippy, browser UI suite including the
1280×720 clipping regression, and `git diff --check`. A single offline NSIS
build produced
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.3_x64-setup.exe`
at 4,623,640 bytes with SHA-256
`6F5CDE59A85393BBAB77394739E8D5FA4271E4C88856593683069BDD25464EAB`.
The release executable reports version 0.1.3 and passed hidden native smoke
plus the controlled full-process uncertain-send restart guard. The latter
does not prove a real mid-send crash.

**PASS for local installed upgrade:** a first attempt to start the existing
installed 0.1.2 WebView in a disposable upgrade profile timed out before
Home appeared. No Codex turn was sent. An unchanged retry with diagnostics
passed and created exactly one real Codex user/reply pair in
`C:/ForgeFast/forge-upgrade-012-to-013-20260927-project`, using the retained
isolated profile beside it. No Forge Desktop process was running when the
installer hash was rechecked and the exact candidate installed silently
over 0.1.2. Installer exit was 0; the installed executable at
`%LOCALAPPDATA%/Forge/forge-desktop.exe` reports 0.1.3 and SHA-256
`6F3193CF8B4F11BB29FE4A8355AB1BB1996E1DD14F1B08E96D05222C739A9E77`.
That installed executable restored the old-version pair without sending
another turn. Its hidden native smoke passed project onboarding, actual
Windows folder and file dialogs, real Forge record readback, local previews,
project switching and appearance. The transient WebView-start cause is
unknown; do not erase it from release evidence.

The public `0.1.2` release is unchanged; `0.1.3` is installed locally but
has **not** been committed, pushed, tagged or published. A public download
and byte-for-byte comparison are NOT_RUN. The earlier real artifact-to-
change test ran on the same source in a local debug build, not on installed
0.1.3. No per-model token counts or BRL cost can be attributed; UNKNOWN.
One primary executor, no workers. Changed source lives in `ui/chat.mjs`,
`ui/index.html`, `ui/message-format.mjs`, `ui/styles.css`; related browser,
native/restart/upgrade tests, design/readme notes and version files changed.

**Next:** review the integrated diff and release text, then decide whether
the accepted publication authorization covers this exact 0.1.3 content.
If publishing, commit/push once, tag with the desktop-only prefix (not the
core `v*` trigger), upload only the already-tested installer and matching
sidecar, then download fresh bytes and recheck hash and installed behavior.
Do not run GitHub CI for each small desktop change.

## Desktop 0.1.3 public alpha readback — 2026-09-27

The integrated source was committed as
`698946204713848b6401ae334ab835032e61afba` on
`codex/desktop-shell` and pushed. Annotated tag
`desktop-v0.1.3-alpha.1` points to that commit and was pushed. The public
release is
https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.3-alpha.1.
No GitHub CI was manually run for this package.

**PASS, published bytes:** GitHub lists the installer and SHA-256 sidecar as
uploaded assets. A fresh unauthenticated download to
`C:/ForgeFast/forge-alpha-013-public-20260927` returned a 4,623,640-byte
installer with SHA-256
`6F5CDE59A85393BBAB77394739E8D5FA4271E4C88856593683069BDD25464EAB`,
matching the one locally tested candidate and the downloaded sidecar. The
downloaded executable was silently installed over local 0.1.3 (exit 0);
the installed product still reports 0.1.3 and SHA-256
`6F3193CF8B4F11BB29FE4A8355AB1BB1996E1DD14F1B08E96D05222C739A9E77`.
The isolated real Codex pair originally created on installed 0.1.2 was
restored once more from the downloaded 0.1.3 installation, with exactly
one user/reply pair and no new Send. An initial attempt at this last
readback could not load Playwright because `PLAYWRIGHT_MODULE` was unset; it was
rerun with the documented cached Playwright module and passed. This is a
harness environment correction, not an app failure.

**Limits:** #92 plain-language decision history remains PARTIAL; the core
does not provide immutable question/choice text for historical decisions.
The artifact-to-change journey passed on a local debug build, not the
installed public binary. A real mid-send crash, full accessibility audit
and complete visual acceptance remain NOT_RUN. The earlier one-time 0.1.2
WebView startup timeout has no diagnosed cause. Per-model usage and BRL
cost remain UNKNOWN. No workers were used in this package.

**Next:** review #92's authoritative decision-history contract with core
ownership before exposing any richer historical wording; continue the
nontechnical end-to-end desktop journey and visual/accessibility review in
one coherent, focused package. Do not imply that this published alpha is
the full Forge product or rebuild the verified 0.1.3 installer for a docs
follow-up.

## Desktop published journey and cold-start fix — 2026-09-27

The downloaded and installed public 0.1.3 binary was exercised headlessly
with real Codex in a disposable project. **PASS:** the same conversation
created `site/index.html`, opened its local scriptless preview, accepted a
follow-up change request, changed the file on disk and restored the chat
after WebView reload without another Send. The screenshot is
`C:/ForgeFast/forge-artifact-public-013.png`. This closes the earlier gap
where the full artifact-to-change journey had only run on a debug build.
It does not prove a real mid-send crash or arbitrary Codex-generated output.

The first run against a new `C:/ForgeFast/forge-artifact-public-013-seed`
timed out waiting 15 seconds for **Projeto pronto**; no Codex message was
sent. An independent fresh CLI fixture measured `forge-core start` at 38.43
seconds, `workflow init` at 4.32 seconds and `project resolve` at 0.02
seconds. The desktop's ordinary project query timeout was 15 seconds per
attempt / 20 seconds total, so it could abort an explicit first-time
`start` after that command began creating authority. The unchanged
installed app passed the journey on retry, once that seed had been
initialized. This is a concrete cold-onboarding defect in published 0.1.3,
not a reason to claim its first attempt passed.

**Local fix, not yet published:** `src-tauri/src/project.rs` now uses a
bounded 85-second attempt / 90-second total only for explicit `start` and
`workflow init`; read-only project queries retain their 15/20-second
limits. The UI says that first-time preparation can take a while, and the
native smoke reports setup status on timeout. A new never-used folder
`C:/ForgeFast/forge-cold-native-fixed-20260927` passed on its first action
in a hidden native debug build, including Forge readback; no real Codex
turn was requested in that regression run. Offline locked desktop `cargo
check`, four focused project tests, all 44 desktop Rust tests, eight Node
tests, `cargo fmt --check`, full controlled browser suite and `git diff
--check` passed. The earlier public-binary artifact journey had one failed
cold setup attempt, then passed on an unchanged retry. No GitHub CI or
installer was built for this local fix. Current modified files are
`src-tauri/src/project.rs`, `ui/main.mjs`, `tests/native.cjs`,
`tests/browser.cjs` and this checkpoint; no worker was used. Per-model
token counts and BRL cost remain
UNKNOWN.

**Next:** confirm the cold-onboarding path and visual experience in a
coherent next alpha package, including the production build/upgrade gates
before publication. Keep the 0.1.3 installed/public state distinct from
this local fix. Continue #92 only with authoritative historical decision
content; do not fabricate it in the UI.

## Desktop 0.1.4 installed candidate gate — 2026-09-27

The cold-onboarding fix was versioned to desktop 0.1.4 without changing
the separate Forge core. The production package source passed offline
locked desktop `cargo check`, four focused project tests, all 44 desktop
Rust tests, eight Node tests, formatting, strict desktop Clippy, full
controlled browser UI suite and `git diff --check`. One NSIS build
produced `D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.4_x64-setup.exe`
at 4,621,382 bytes, SHA-256
`06EA9E99C22D56FE39069BC73C0FF0C45023A64FF79F064A3A361BB68B82BBDE`.
The release executable reports 0.1.4. Its hidden native smoke passed a
never-used project folder, real Windows folder and file dialogs, actual
Forge record readback, a real Codex-created local page, preview, change
request in the same chat and WebView reload without resending.

**PASS, installed upgrade:** before installing, the installed 0.1.3
restored the disposable real Codex pair created in 0.1.2. The exact
candidate hash was rechecked, then the installer exited 0 over 0.1.3.
Installed 0.1.4 reports SHA-256
`A41160EE51F9F0E91841CDDABE8D05A8EDA79DC8FD878F4CEF4D99EDF8ED1E1C`.
It restored the same pair with no new Send. A second hidden native smoke
on installed 0.1.4 prepared another never-used folder on the first
action, exercised the real Windows folder/file dialogs and read back the
Forge record. Real Codex artifact creation was NOT_RUN on that installed
binary, but passed on the release executable from the same build.

The 0.1.4 candidate is installed locally but is **not** yet committed,
tagged or public. Fresh public download and byte comparison are NOT_RUN.
The first-attempt cold failure of public 0.1.3 and its measured 38.43
second `forge-core start` are documented above; do not erase that
history. #92 plain-language decision history, full visual/accessibility
acceptance and real mid-send crash remain open. No GitHub CI was run, no
worker used, and per-model usage/BRL cost are UNKNOWN. Modified paths:
`src-tauri/src/project.rs`, desktop version/lock/config, `ui/main.mjs`,
`tests/native.cjs`, `tests/browser.cjs`, this README and
`RELEASE_NOTES-0.1.4.md`.

**Next:** review staged diff and release notes; under the maintainer's
standing explicit publication authorization, commit/push once, use a
desktop-only tag, upload only the one tested candidate and SHA sidecar,
then download fresh public bytes, verify the hash, reinstall that exact
download and confirm installed continuity. Do not rebuild the candidate
or run GitHub CI for a small desktop-only update.

## Desktop 0.1.4 public alpha readback — 2026-09-27

The tested source was committed as
`9c06f344e52f8065371c5fc8cfcf4ce64d77f036` on
`codex/desktop-shell` and pushed. Annotated tag
`desktop-v0.1.4-alpha.1` points to that source commit and was pushed.
Release: https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.4-alpha.1.
No GitHub CI was manually run.

**PASS, public bytes and installation:** GitHub lists the one NSIS
installer and matching SHA-256 sidecar as uploaded assets. A fresh
unauthenticated download to `C:/ForgeFast/forge-alpha-014-public-20260927`
returned 4,621,382 bytes with SHA-256
`06EA9E99C22D56FE39069BC73C0FF0C45023A64FF79F064A3A361BB68B82BBDE`,
identical to the locally tested candidate and downloaded sidecar. The
downloaded file installed silently over local 0.1.4 with exit code 0;
the installed product remains version 0.1.4, executable SHA-256
`A41160EE51F9F0E91841CDDABE8D05A8EDA79DC8FD878F4CEF4D99EDF8ED1E1C`.
The isolated real Codex pair carried through prior versions was restored
again with no new Send. This is not a full real artifact journey on the
publicly downloaded binary; that journey passed on the release executable
from the same build. The published installer is unsigned and still
requires separately installed Forge core and authenticated Codex CLI.

**Open:** #92 historical decision wording lacks an authoritative core
contract; full visual/accessibility acceptance, real mid-send crash,
self-contained distribution and automatic update remain unfinished.
Per-model token use and BRL cost remain UNKNOWN. No worker was used.

**Next:** return to the complete nontechnical journey and its visible
record, especially #92's authority-backed history and accessibility;
avoid adding a duplicate decision store or treating the 0.1.4 bugfix as
full-product completion. Package future improvements coherently, with
focused Rust checks and native proof before publication.

## Desktop accessibility semantics slice — 2026-09-27

The active objective remains the complete, approachable Windows journey:
choose a folder, prepare the project, converse with Codex, preview a real
result, request changes in the same chat, and resume after restart. The
published/installed version is still 0.1.4; this slice is local source only.

**PASS, controlled browser:** a headless axe-core 4.13.0 scan of Início,
Explorar, Meus projetos and Minha conversa found two serious invalid ARIA
labels on unnamed generic `div` elements (project shortcuts and messages).
The UI now gives the shortcuts, theme cards, conversation picker and recorded
work timeline explicit group semantics, while the messages container relies
on its already named parent conversation-history region. A repeat scan found
zero reported violations across the four initial/empty screens, mocked
loaded-project-with-record and loaded-text-preview states, and a mocked
active chat in light and dark themes. The active-chat scan exposed an
additional moderate heading-order
problem when a Codex reply began with `##`; its formatter now starts each
reply at the conversation's `h3` level and prevents skipped levels while
retaining the original Markdown text. The existing controlled browser
suite passed, including semantic and heading-order regressions, and `git diff
--check` passed. A fresh offline locked desktop debug build passed. The hidden
native WebView verified the same heading normalization, then a full hidden
native smoke against a small, already-linked disposable project passed Forge
record readback, fresh-folder onboarding, local preview, and UI behavior.
Actual Codex conversation was NOT_RUN in this native slice. No visible app
window opened. No Rust source changed, so Rust tests were NOT_RUN.

An earlier hidden native run against the much larger Forge repository timed
out in the test harness after 35 seconds waiting for record readback. The
app's record-query limit is 90 seconds, so this did not establish an app
failure. The harness wait now permits 105 seconds. A subsequent direct
`workflow resume` on that repository completed in 6.36 seconds, and a hidden
native smoke read its real record successfully. This establishes a passing
retry, not the cause of the earlier delay or a guarantee against recurrence.

**Limits:** the latest automated scan left 15/25/14/35/59/63/45/45
color-contrast nodes incomplete for the initial, loaded and active-chat states because
gradient backgrounds prevent automatic determination. Direct WCAG-style
palette-endpoint calculations found at least 5.31:1 for muted text on the
sampled light gradient stops and 6.56:1 for muted text on the sampled dark
surfaces; these are not a complete pixel-level or screen-reader audit.
Manual contrast and full screen-reader acceptance remain NOT_RUN. The
loaded project, text preview and chat were simulated, not a native WebView
or actual Codex response. `#92` is still partial: core audit history carries decision digests
and references, not authoritative historical question/choice wording. Do not
invent these from chat text or add a parallel desktop decision store.

The native visual review exposed a separate readability problem: the real
Forge objective is long, technical and in English. A first attempt to show
its raw text prominently was rejected after a native screenshot. The current
UI instead shows a short Portuguese notice that a direction was recorded,
with its provenance and non-approval caveat; the original wording remains
available on demand. “Entender esta direção na conversa” only prepares an
editable Portuguese question in the existing Codex composer. It does not
send, approve or change the record. The controlled browser suite passed this
behavior, including preserving an existing draft. The current source passed
an offline locked desktop debug build, then hidden native smoke against the
real Forge repository: record readback, original text initially collapsed,
new explanation action visible, existing onboarding and local preview.
Actual Codex conversation was NOT_RUN in this slice. The screenshots and
automated axe results are not a full visual or screen-reader acceptance.

The existing core decision audit exposes digests/references rather than
authoritative historical question and choice wording. Therefore `#92` is
still partial; this UI change does not invent a decision history or silently
expand the core contract. No commit, push, release, or installer in this
slice. No workers used. Per-model token usage and BRL cost remain UNKNOWN.
Local changes: `ui/index.html`, `ui/message-format.mjs`, `ui/progress.mjs`,
`ui/styles.css`, `tests/browser.cjs`, `tests/native.cjs`, and this checkpoint.
**Next:** continue the nontechnical UI review, establish the smallest
authoritative contract for historical decisions if `#92` needs exact wording,
then package a coherent alpha with release gates and native proof.

## Desktop 0.1.5 alpha preparation — 2026-09-27

A hidden native viewport capture at 1165×820 showed that the empty preview
placeholder occupied most of the right column, leaving the useful Forge
record below the fold. The empty preview is now a compact horizontal note;
the existing file-loaded preview is unchanged. A controlled 1280×720 browser
assertion requires the record to begin inside the first viewport. The full
controlled browser suite passed. After an offline locked desktop debug build,
the hidden native smoke against the real Forge repository passed again, and a
new 1165×820 native screenshot shows the record heading in the first view.
The app did not open on the user's visible desktop. Actual Codex conversation
remains NOT_RUN for this slice.

Version 0.1.5 is staged in the desktop Cargo manifest/lock and Tauri config;
`RELEASE_NOTES-0.1.5.md` is a candidate, not a published release. The existing
local UI/accessibility changes remain uncommitted. No NSIS candidate has been
built, no installation/upgrade has been attempted, and public availability
is NOT_RUN. No worker was used; model-token and BRL costs remain UNKNOWN.

**Next:** run the desktop-only package gate, build one NSIS candidate, test
that exact candidate including a real Codex artifact-to-change journey and
upgrade continuity, then commit/push/tag and publish under the maintainer's
standing release authorization. Keep the history/decision and distribution
limitations in the release notes; do not run GitHub CI for this branch slice.

## Desktop 0.1.5 installed candidate gate — 2026-09-27

The desktop-only package source passed offline locked `cargo check`, all 44
desktop Rust tests, eight Node tests, `cargo fmt --check`, strict desktop
Clippy, the controlled browser suite, the headless axe scan (no reported
violations; gradient contrast incomplete), and `git diff --check`. One pinned
NSIS build produced
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.5_x64-setup.exe`
at 4,620,348 bytes, SHA-256
`F4793ABBEA90B9A951A94E48572FDDEFC2B7EC6B980C1B380D8EC0C24B9922D9`.
A matching `.sha256` sidecar was generated. No subsequent rebuild is intended
for this candidate.

**PASS, release executable:** hidden native smoke against the real Forge
repository and test-owned new folders passed the actual Windows folder/file
dialogs, Forge record readback and onboarding, and a real Codex-created HTML
page opened in the local preview. A second real turn changed the page in the
same chat. WebView reload restored the conversation without a new Send.
Screenshot: `C:/ForgeFast/forge-ui-audit-20260927/release-015-real-artifact.png`.

**PASS, installed upgrade:** the previously installed 0.1.4 restored the
retained disposable real Codex user/reply pair without sending. No Forge app
process was running; the candidate hash was rechecked immediately before
installation. Silent NSIS installation exited 0 over 0.1.4. Installed 0.1.5
reports product/file version 0.1.5 and executable SHA-256
`37B0BCE7D3DFD88CDEEF506C905D7BB49DBE7824ADCA94ED2339186CE886D402`.
The same pair was restored after upgrade with no new Send. A separate hidden
native smoke on the installed executable passed real folder/file dialogs,
Forge onboarding, record readback and preview. Real Codex artifact creation
was NOT_RUN on that installed binary, but passed on the release executable
from the same build. The installer hash stayed unchanged after installation.

The 0.1.5 candidate is installed locally but **not** committed, tagged or
public. Fresh public download/byte comparison are NOT_RUN. #92 exact historical
decision wording, manual accessibility acceptance, real in-flight crash,
self-contained distribution and auto-update remain open. No GitHub CI or
worker was run. Per-model token use and BRL cost are UNKNOWN.

**Next:** review the integrated diff and candidate notes, commit/push once,
push a desktop-only tag that does not trigger the core `v*` release workflow,
upload exactly this installer and sidecar, then download fresh public bytes,
verify the hash and installed continuity. Do not rebuild the candidate.

## Desktop 0.1.5 public alpha readback — 2026-09-27

The tested source was committed as
`3676e45c59a67ebffcee85815f4f5f8d38a559be` on `codex/desktop-shell`
and pushed. Annotated tag `desktop-v0.1.5-alpha.1` points to that commit and
was pushed. Release:
https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.5-alpha.1.
The branch and desktop-only tag do not match the repository's main/master CI
or core `v*` release triggers; no GitHub CI was manually run.

**PASS, public bytes and installation:** GitHub lists the one NSIS installer
and matching SHA-256 sidecar. A fresh unauthenticated download to
`C:/ForgeFast/forge-alpha-015-public-20260927` returned 4,620,348 bytes with
SHA-256 `F4793ABBEA90B9A951A94E48572FDDEFC2B7EC6B980C1B380D8EC0C24B9922D9`,
identical to the installed candidate and downloaded sidecar. The downloaded
installer exited 0 when silently installed over local 0.1.5; the installed
product remains version 0.1.5, executable SHA-256
`37B0BCE7D3DFD88CDEEF506C905D7BB49DBE7824ADCA94ED2339186CE886D402`.
The retained disposable real Codex user/reply pair was restored again with
no new Send. The complete real artifact-to-change journey passed on the
release executable from the same build, not anew on the public download.

One attempted `--version` probe accidentally launched the release executable
outside the isolated desktop; that process was identified and stopped. All
subsequent app checks used the hidden desktop harness. This did not change
the candidate installer bytes or the installed/public version.

The installer is unsigned, needs separate Forge core and authenticated Codex
CLI, and does not auto-update. #92 exact historical decision wording, manual
accessibility acceptance, real in-flight crash and self-contained distribution
remain open. Model-specific usage and BRL cost remain UNKNOWN. No worker used.

**Next:** continue the full nontechnical journey beyond this alpha, prioritizing
authority-backed readable decision history and full visual/accessibility
acceptance; keep changes integrated with actual Codex chat and native proof.

## Desktop decision-history feasibility slice — 2026-09-27

The full nontechnical Windows journey remains the active objective; this is
local source work after published 0.1.5, not a new installer or release.
For #92, the read-only core audit was inspected before adding UI state.
`DecisionNeedRaisedEvent` stores a decision reference and question digest but
not the question text; `DecisionResolvedEvent` stores the selected alternative
reference but not its description. The continuity report projects those same
references, digests and status. The current admitted policy contains readable
rules, but using its current text for an older receipt would silently relabel
history after a policy revision. A live `forge-core workflow report --root
D:\Forge-method-core --json` returned zero decision-history entries in this
project, so there is no real resolved-decision fixture here to validate prose
against. Exact historical wording remains **NOT_RUN/unavailable**, not inferred
from chat or today's policy.

The desktop now states explicitly when a recovered pending decision lacks its
original question in this screen and asks for source consultation before a
choice. This is a UI honesty fix, **not** #92 completion. Changed files:
`ui/progress.mjs`, `tests/browser.cjs`, this checkpoint. JavaScript syntax,
the controlled browser suite and `git diff --check` passed. Native WebView,
Rust, manual screen-reader/contrast and a real decision fixture were NOT_RUN;
no commit, push, installer, release or worker was used. Model-token usage and
BRL cost remain UNKNOWN.

**Next:** establish a minimal immutable wording contract for *future* decision
receipts at the core boundary, with legacy ref-only entries explicitly marked
unavailable; validate that contract with a focused core test before projecting
it into the Desktop. Do not add a second desktop decision store or reconstruct
old words from current policy text. Then run headless native UI validation and
package the next coherent alpha, rather than running GitHub CI for this small
slice.

## Desktop automatic preview refresh after a Codex change — 2026-09-27

Reassessment of the preceding proposed core contract: production
`DecisionResolved` receipts exist, but `DecisionNeedRaised` is constructed only
in tests in the current source, and this project's live report has no decision
history entries. Adding a wording snapshot immediately would not complete a
useful #92 screen for the Solo Cooperative journey and could introduce a new
core migration without a real fixture. #92 remains partial; do not infer old
decision prose from current policy. The next product-visible slice instead
addresses the real result-to-change journey.

When Codex reports a completed turn, an already loaded local result now
refreshes through the existing native `inspect_preview` path. The selected
file remains inside the confirmed project and is not guessed from reply text.
Interrupted/failed turns, absent/failed previews, active selection and an
open enlarged-preview dialog do not trigger an automatic read. The manual
refresh action remains available. Changed files: `ui/chat.mjs`,
`ui/preview.mjs`, `tests/browser.cjs`, `tests/native.cjs`, plus the earlier
`ui/progress.mjs` honesty copy and this checkpoint.

**PASS:** JavaScript syntax, full controlled browser suite, offline locked
desktop debug build, and a hidden native WebView journey. The native journey
used the real Codex to create a local HTML page, opened its sandboxed preview,
requested a title change in the same conversation, and observed the new title
appear in the already open preview without clicking manual refresh. It also
restored the real conversation after WebView reload without resending. The
native folder/file picker choice in this run was simulated; earlier 0.1.5
candidate evidence covered actual Windows dialogs. No visible app window
opened. `git diff --check` and final diff review remain for this package.
No Rust source changed, so focused Rust tests were NOT_RUN. Manual visual,
screen-reader and complete contrast acceptance remain NOT_RUN. No commit,
push, new installer, release or worker in this slice. Model-specific token
usage and BRL cost remain UNKNOWN.

**Next:** inspect the automatic-refresh diff and its boundary cases, run the
remaining focused desktop checks, then choose the next coherent UI package
before building a single 0.1.6 alpha candidate. Keep the full nontechnical
journey goal active; do not force a core decision-history migration merely to
fill a screen without authoritative data.

## Desktop 0.1.6 installed candidate gate — 2026-09-27

The 0.1.6 UI package refreshes a previously opened local result after a
successful Codex turn without guessing a new file. If the enlarged preview is
open, it remains open and refreshes on close. A pending decision whose original
question is unavailable is labeled honestly. #92 remains partial.

**PASS, source:** offline locked desktop-only `cargo check`, focused progress
tests (10), all 44 desktop Rust tests, eight Node tests, `cargo fmt --check`,
strict desktop Clippy, full controlled browser suite, JavaScript syntax and
`git diff --check`. No Rust source or separate core workspace was changed.
No GitHub CI was run. The 0.1.6 version is in the Desktop Cargo manifest,
lockfile and Tauri config. `RELEASE_NOTES-0.1.6.md` documents content and
limits. The installer was built **once**, not rebuilt after hashing:
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.6_x64-setup.exe`,
4,623,315 bytes, SHA-256
`CD50E95002A5D9E923E206AD86DB864732612DEE53424E711F91313A6AC0BD9D`.
A matching `.sha256` sidecar exists beside it.

**PASS, release executable:** hidden native smoke used real Codex to create a
local HTML page, open its isolated preview, change its visible title in the
same conversation and observe the new title without clicking manual refresh.
It restored the conversation after WebView reload without sending again. A
separate hidden native test checked deferred refresh while the enlarged
preview is open. No visible app window opened.

**PASS, installed upgrade:** installed 0.1.5 created a disposable real Codex
user/reply pair. The candidate hash was rechecked; the silent NSIS install
over 0.1.5 exited 0. Installed 0.1.6 reports product version 0.1.6 and
executable SHA-256
`3625F44B812D17529A23F369D52E0240BB57692A0F5ED39467EA5B7A4ED6ACFA`.
The same pair was restored on installed 0.1.6 without a new Send. A separate
hidden installed-binary smoke passed actual Windows folder/file dialogs,
Forge onboarding/readback and local preview. The new real Codex artifact
journey was NOT_RUN from the installed binary; it passed on the release
executable of the same build. Manual screen-reader/complete contrast and a
real in-flight process crash remain NOT_RUN. No worker used; model-specific
tokens and BRL cost are UNKNOWN.

The 0.1.6 candidate is **installed locally but not committed, tagged or
public**. Public download and byte comparison are NOT_RUN.

**Next:** review the integrated source and release notes, commit/push once,
tag a desktop-only release, upload exactly this installer and sidecar, then
download the public bytes fresh, verify their hash, and confirm installed
conversation continuity again. Do not rebuild the candidate or manually run
GitHub CI for this branch/tag package.

## Desktop 0.1.6 public alpha readback — 2026-09-27

The preceding installed-candidate checkpoint records the pre-publication
state. It is superseded by this readback. The reviewed Desktop package was
committed as `814f47f8b1fbc520c9daa10397cf420c94d3dd30`, pushed, and
tagged `desktop-v0.1.6-alpha.1`. The [public alpha release](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.6-alpha.1)
contains the **same one** NSIS installer and its `.sha256` sidecar. No
rebuild was made after the candidate test.

**PASS, public bytes:** an unauthenticated download of the installer and
sidecar to `C:/ForgeFast/forge-alpha-016-public-20260927` matched the local
candidate: 4,623,315 bytes and SHA-256
`CD50E95002A5D9E923E206AD86DB864732612DEE53424E711F91313A6AC0BD9D`.
The public asset digest reported by GitHub also matches. The downloaded
installer silently reinstalled over 0.1.6 with exit 0. The installed
executable still reports 0.1.6 and its hash remained
`3625F44B812D17529A23F369D52E0240BB57692A0F5ED39467EA5B7A4ED6ACFA`.
The disposable real Codex pair originally created in installed 0.1.5 was
restored again without a new Send. No visible application window opened.

**Limits:** manual visual/contrast/screen-reader acceptance, a real
in-flight crash and separate clean-machine core/Codex installation remain
NOT_RUN. Forge historical decision wording remains partial (#92). The alpha
installer is unsigned, current-user only, and has no auto-updater. GitHub CI
was not manually run. No worker participated in this package; model-specific
tokens and BRL cost remain UNKNOWN.

**Next product slice:** inspect the current Windows UI journey against the
approved visual references and the development plan, then choose a coherent
screen/integration gap with an objective native acceptance test. Avoid
inventing state or adding core complexity solely to fill a screen. Keep the
overall nontechnical journey goal active; mobile and self-contained
distribution are later stages.

## Desktop 0.1.6 installed end-to-end journey — 2026-09-27

**PASS:** the installed, publicly downloaded `0.1.6` executable at
`C:/Users/User/AppData/Local/Forge/forge-desktop.exe` passed the complete
hidden native `apps/desktop/tests/native-hidden.ps1` run with
`FORGE_TEST_FOLDER_DIALOG=select`, `FORGE_TEST_PREVIEW_DIALOG=select`,
`FORGE_TEST_AGENT_SMOKE=1`, `FORGE_TEST_ARTIFACT_JOURNEY=1`, and
`FORGE_TEST_PROCESS_RESTART=1`. Playwright drove the actual Tauri WebView on
an isolated Windows desktop; the user's visible desktop was not activated.
The test profile and projects were disposable and cleaned by the harness.

The run exercised actual Windows folder/file picker selection; onboarding a
folder with existing work without changing its file; onboarding a new empty
project and reading its Forge record; first Send with real authenticated
Codex; creation of `site/index.html`; opening its local sandboxed preview
from the completed reply; asking for a visible title change in the same
conversation; automatic preview refresh; recovery after WebView reload; and
recovery after a full native process restart. The restored first/last
messages and count matched, and the restart path observed zero new Send
calls. The script exited 0. This closes the previous evidence gap for a
**new real Codex artifact turn from the installed binary**. It does not prove
that an arbitrary Codex response will always include a usable file action.

Manual visual, contrast and screen-reader acceptance across all screens,
an in-flight process crash/ambiguous Send, clean-machine installation of
the separate Forge core and Codex CLI, and auto-update remain NOT_RUN or
unfinished as previously recorded. The test does not establish publication
of the created page or safe interaction with a JavaScript website. No source
files, installer, tag or release assets changed in this slice. No worker was
used; model-specific token usage and BRL cost remain UNKNOWN.

**Next:** use the approved visual boards and the actual app screens to find
and repair a concrete nontechnical UX gap. Keep native acceptance at the
package boundary and avoid manually starting GitHub CI for each change.

## Desktop 0.1.7 uncertain-send recovery — 2026-09-27

The installed 0.1.6 end-to-end journey above remains valid for the normal
path. This package fixes an exceptional but consequential UI recovery gap:
resuming a Codex conversation previously cleared the local uncertain-Send
marker even if its history did not show the unconfirmed message. It could
make another Send available before the person had actually reviewed what
happened. The UI now retains the project-scoped marker and disables Send
until the person chooses **Já conferi o envio** after opening that same
conversation. That action does not resend anything. If local storage cannot
save the acknowledgement, the protection remains. Keyboard focus moves to
the updated status when the review button disappears. No Forge state or
Codex transcript is copied into a second store.

**PASS, source:** `0.1.7` is set in the Desktop Cargo manifest, lockfile and
Tauri config. Eight Node tests, the controlled browser suite, offline locked
desktop-only `cargo check`, all 44 desktop Rust tests, formatting, strict
desktop Clippy and `git diff --check` passed. No separate core workspace
or GitHub CI run. `RELEASE_NOTES-0.1.7.md` describes content and limits.

**PASS, hidden native:** a controlled Codex bridge demonstrated full process
restart during a pending Send, no replay/new thread, persistence of the
warning, and explicit review before another Send. A real authenticated Codex
probe withheld only the UI acknowledgement after the native `turn/start`
response, then killed the app. On restart, the same thread resumed without
replay and Send stayed blocked; the accepted prompt was *not visible* in the
restored history in that run. This proves why native acceptance cannot be
treated as durable delivery. The installed recovery screen was captured at
`C:/ForgeFast/forge-review-017-20260927.png`; its review action, explanation
and disabled Send were visually inspected. The tests ran on a hidden Windows
desktop without activating the user's visible desktop.

**PASS, installed upgrade:** the single NSIS candidate at
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.7_x64-setup.exe`
is 4,622,466 bytes, SHA-256
`BE0F80E5D0175DA05C90C97FEDC38BE588E9F6E80F00A9064AB0F588F02421B5`.
Its `.sha256` sidecar sits beside it. The hash was checked before silent
installation over 0.1.6 (exit 0). Installed version is 0.1.7 and executable
SHA-256 is
`4958CE1F052EBD28703948782C3294CEDCB926FC24BBE83F26352D73786CF203`.
The installed binary restored the existing disposable real Codex conversation
across the upgrade with zero new Sends, then passed both hidden recovery
tests. No installer rebuild after hashing.

**Limits:** a real lost-acknowledgement test is not a proof of
exactly-once delivery for every Codex version or crash timing. The person
must review ambiguous work; no automatic resend is claimed. Complete manual
screen-reader/contrast acceptance, separate clean-machine core/Codex
installation, auto-update and mobile remain unfinished. Model-specific
tokens and BRL cost remain UNKNOWN; no worker was used. Do not rebuild this
installer or manually trigger GitHub CI for this package.

## Desktop 0.1.7 public release readback — 2026-09-27

**PASS:** source package committed/pushed as `e2b1d2a7`; annotated tag
`desktop-v0.1.7-alpha.1` points to that commit. Prerelease URL:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.7-alpha.1`.
GitHub reported the installer as 4,622,466 bytes with SHA-256
`BE0F80E5D0175DA05C90C97FEDC38BE588E9F6E80F00A9064AB0F588F02421B5`.
A fresh unauthenticated download to
`C:/ForgeFast/forge-alpha-017-public-20260927` matched that exact size,
hash and `.sha256` sidecar. No rebuild occurred. The downloaded installer
silently installed over the local 0.1.7 candidate with exit 0; installed
version is 0.1.7 and executable SHA-256 is
`4958CE1F052EBD28703948782C3294CEDCB926FC24BBE83F26352D73786CF203`.
The installed download passed hidden native resume of the pre-upgrade real
Codex user/reply pair with zero new Sends and the controlled full-process
uncertain-Send restart guard with explicit review. The real-Codex lost-ack
probe was run earlier on the same executable hash, not repeated after the
identical downloaded reinstall. No GitHub CI was manually started. Manual
complete accessibility/visual acceptance, separate clean-machine core/Codex
setup, auto-update and mobile remain NOT_RUN/unfinished. Model-specific
tokens and BRL cost remain UNKNOWN.

**Next smallest product slice:** inspect the actual installed UI against
approved visual direction and pick one coherent, user-visible gap in the
nontechnical folder → project → conversation → local-result journey. Reproduce
it with a hidden native acceptance test before changing code. Keep the
overall journey goal active and do not add Forge core complexity to fill UI.

## Desktop 0.1.8 project context and preview hierarchy — 2026-09-27

**Observed defect:** in installed 0.1.7 at 1180 × 820, the empty local
preview consumed the top of the workspace's right column; the real Forge
record activity was below the first viewport. Hidden native screenshot:
`C:/ForgeFast/forge-017-empty-preview-before.png`. A new browser assertion
reproduced this as FAIL before editing. This was not an absent backend state:
the real record existed but was visually secondary to a placeholder.

**Implemented:** with no file selected, the actual Forge record precedes a
compact, honest preview prompt. Its header puts the read action beside the
heading and removes duplicate prose; status still warns that the record may
lag behind the conversation. Once a file is chosen or linked from a completed
Codex reply, the preview moves ahead of the record. DOM/keyboard order and
visual grid order agree in both states; choosing a file focuses its heading.
No invented project progress, preview, transcript store or core command was
added. Changed UI: `index.html`, `styles.css`, `preview.mjs`; acceptance in
`browser.cjs` and `native.cjs`. Desktop version is `0.1.8` in its Cargo
manifest, lockfile and Tauri config; `RELEASE_NOTES-0.1.8.md` describes the
candidate and limits.

**PASS:** browser suite, eight Node tests, JS syntax, offline locked
desktop-only `cargo check`, 44 desktop Rust tests, `cargo fmt --check`, strict
desktop Clippy and `git diff --check`. A debug and then the release executable
passed hidden native WebView smokes. The installed 0.1.8 candidate passed
native real Forge readback, the actual Windows file picker, safe preview and
message-to-file action (controlled reply fixture), and resumed an existing
real Codex user/reply pair without a new Send after silent upgrade from 0.1.7.
The installed screenshot `C:/ForgeFast/forge-018-record-native.png` shows the
actual phase and activity, plus the start of the next step, beside the empty
conversation. Tests ran on an isolated Windows desktop, not the user's
visible one. A new real Codex artifact turn was NOT_RUN for this UI-only
package; earlier installed 0.1.6 journey evidence remains separate.

**Candidate:** one NSIS installer at
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.8_x64-setup.exe`,
4,624,036 bytes, SHA-256
`733CA93C5D0389BAC67C7213BF5CB71B0AF8A3C384E5A59CFAE9578842F93CDC`.
Its sidecar is beside it. Installed candidate version is 0.1.8 and executable
SHA-256 is
`B3BFB9CC7C13AEF235143A88EC958C4F81949B0EDA6CB09CB35BF00A70FF1BF0`.
No rebuild after hashing. GitHub CI was not manually run. Complete manual
screen-reader/contrast acceptance, clean-machine core/Codex setup,
auto-update and mobile remain unfinished. No worker was used; model-specific
tokens and BRL cost are UNKNOWN.

**Public readback:** source package committed/pushed as `57699188`; annotated
tag `desktop-v0.1.8-alpha.1` points to that commit. Prerelease URL:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.8-alpha.1`.
GitHub reported installer size 4,624,036 bytes and SHA-256
`733CA93C5D0389BAC67C7213BF5CB71B0AF8A3C384E5A59CFAE9578842F93CDC`.
A fresh unauthenticated download to
`C:/ForgeFast/forge-alpha-018-public-20260927` matched the size, hash and
checksum sidecar. The downloaded installer silently reinstalled with exit 0;
installed version remained 0.1.8 and executable SHA-256 remained
`B3BFB9CC7C13AEF235143A88EC958C4F81949B0EDA6CB09CB35BF00A70FF1BF0`.
The installed public bytes resumed the pre-upgrade real Codex user/reply pair
with zero new Sends. Native record/preview and actual Windows file picker
passed on the same executable hash before this identical reinstall; they
were not repeated afterward. No rebuild or manual GitHub CI. The release
notes contain the limits; no full-product readiness claim is made.

**Next product slice:** continue auditing the installed nontechnical
conversation → actual result → change request journey against the approved
visual direction. Choose a substantive user-visible gap and verify it in
the hidden native app; do not add Forge core complexity just to populate a
screen. Keep the overall journey goal active. Model-specific tokens and BRL
cost remain UNKNOWN.

## Desktop result-resume slice in progress — 2026-09-27

**Objective:** make an actual local file cited by the latest completed Codex
reply easy to find again after resuming a conversation. In the current public
0.1.8 app, the transcript restores but the transient preview does not; the
person must find its link in chat or browse for the file. The product goal
remains the nontechnical folder → conversation → real result → change request
→ full restart journey; this is one small UI slice, not completion of it.

**Source pending, not installed or published:** `ui/chat.mjs`,
`ui/message-format.mjs`, `ui/index.html`, `ui/styles.css` and
`tests/browser.cjs`. A compact **Conferir arquivo citado** action appears
only when the latest completed agent message contains exactly one distinct
supported local-file candidate. It also appears on resumed history, and
remains available in raw-text view. Multiple files, an incomplete answer or
a later user message do not cause a guessed result. The path is derived from
the Codex-owned transcript in memory; Forge does not copy the transcript or
store a new file bookmark. Clicking it uses the existing native,
project-bound `inspect_preview` validation and may still report that the
file is unavailable or outside the project. It is a cited file, not a claim
that a result was built or published.

**Evidence:** the controlled browser suite failed on the missing resume
action before the change, then passed after it; eight Node tests, JS syntax
and `git diff --check` passed. An offline locked Desktop-only debug build
passed; no Rust source changed, and no workspace-wide Rust checks were run.
The focused hidden native test `tests/native-result-shortcut.cjs` now passes:
it uses a controlled Codex-history response, a real project-contained file,
the real native `inspect_preview`, a full app-process stop/restart, and an
outside-project path rejected by native validation. It sends no Codex turn.
`tests/native-hidden.ps1` accepts this focused test. The visual capture
`C:/ForgeFast/forge-result-shortcut-native.png` shows the cited-file action
and honest local-preview language in the actual WebView. The first hidden
test attempt proved behavior but failed cleanup on a transient WebView lock;
after bounded cleanup retries were added, two runs passed. One old disposable
profile under `D:/Temp/User/forge-result-shortcut-ktYUZC` may remain because
an attempted manual removal was policy-blocked; do not claim cleanup of it.
No real Codex artifact turn was run for this UI slice; its native history
response was controlled. No worker was used; model-specific tokens and BRL
cost are UNKNOWN. No commit, push, version bump, installer or release for this
pending slice; 0.1.8 remains the latest public build.

**Next smallest step:** review the integrated diff and the next substantial
gap in the nontechnical result/change-request journey, rather than publishing
this tiny action alone. For the next coherent alpha package, re-run the full
native journey with real Codex, verify the installed upgrade/restart, then
version, commit/push and publish under the existing release rules. Do not
manually start GitHub CI for each UI edit.

## Desktop 0.1.9 candidate verification — 2026-09-27

The resumed-file shortcut now joins the actual result/change-request journey:
the hidden native focused test also checks that **Pedir mudança neste arquivo**
prepares a draft in the same restored conversation, without sending a turn.
The standalone browser suite, eight Node tests, Desktop-only offline locked
check and all 44 Desktop Rust tests, formatting, strict Desktop Clippy, JS
syntax and `git diff --check` passed. The Desktop-only debug and release
builds passed. No separate Forge core workspace or manual GitHub CI run.

**PASS, real Codex on the same UI source before the 0.1.9 metadata bump:**
the complete hidden native journey
initialized an empty folder, sent to authenticated Codex, created actual
`site/index.html`, previewed it, asked for and received a visible change in
the same conversation, reloaded and then restarted the entire app process.
The real conversation returned in order without resend. The first attempted
command omitted the harness's required `FORGE_TEST_PROJECT` and executed no
real project journey; the corrected command passed. This was a disposable
project, not a user workspace. The new shortcut is conditional on a unique
citation; that condition was proved separately in the controlled hidden
native test, not asserted for every real model reply.

**PASS, release candidate and installed upgrade:** one NSIS candidate at
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.9_x64-setup.exe`
is 4,621,160 bytes, SHA-256
`E16351D7DAE468968FDF43D0F3811C835DEBB428C407A2D8ED2D5185ADE1DDA9`;
its sidecar is beside it. The release executable passed the focused hidden
native restart/preview/path-rejection test. The same installer silently
upgraded the local 0.1.8 installation with exit 0. Installed version is
0.1.9; installed executable SHA-256 is
`6EA8B9B72E46A3806F93AA316FABC367BFE04095B0A1DCE6E81C7AE960A25DBD`.
The installed binary again passed the focused hidden native test and resumed
an older disposable real Codex user/reply pair without a new Send. That
history fixture predates 0.1.8; the installation itself was 0.1.8 → 0.1.9.
No installer rebuild occurred after hashing. The exact candidate is not yet
publicly available; do not call it published until upload and downloaded-file
verification pass.

**Limits and next step:** complete manual screen-reader/contrast acceptance,
clean-machine separate core/Codex setup, auto-update, self-contained
distribution and mobile remain unfinished. No worker was used; per-model
tokens and BRL cost remain UNKNOWN. Review the package diff, commit and push
selected source/docs, tag and publish the exact candidate under the
maintainer's standing release permission, then verify fresh public downloads
and installed readback. Do not trigger GitHub CI manually.

## Desktop 0.1.9 public release readback — 2026-09-27

**PASS:** source package committed and pushed as `899d0542`; the annotated
tag `desktop-v0.1.9-alpha.1` points to that commit. Public prerelease:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.9-alpha.1`.
GitHub reports installer size 4,621,160 bytes and SHA-256
`E16351D7DAE468968FDF43D0F3811C835DEBB428C407A2D8ED2D5185ADE1DDA9`.
A fresh unauthenticated download to
`C:/ForgeFast/forge-alpha-019-public-20260927` matched that exact size,
hash and `.sha256` sidecar. The downloaded file silently reinstalled with
exit 0; installed version remained 0.1.9 and executable SHA-256 remained
`6EA8B9B72E46A3806F93AA316FABC367BFE04095B0A1DCE6E81C7AE960A25DBD`.
These installed downloaded bytes passed the focused hidden native full
process restart, file preview, prepared change without Send and outside-path
rejection test. The older disposable real Codex user/reply pair had already
resumed on the same installed executable hash before identical downloaded
reinstallation. No installer rebuild or manually triggered GitHub CI.

**Next product slice:** continue toward a complete nontechnical journey by
auditing the installed app against the approved visual references and the
development plan, prioritizing a concrete gap over cosmetic churn. Keep
Forge record truth, Codex history, and local file validation separate.
Complete manual accessibility acceptance, clean-machine core/Codex setup,
self-contained distribution, auto-update and mobile remain open. No worker
used in this package; model-specific tokens and BRL cost remain UNKNOWN.

## Desktop preview clarity and navigation targets — 2026-09-27

After the public 0.1.9 readback, a source-only UI audit found that the enlarged
preview displayed site-specific restrictions even for plain text, Markdown and
images. It now always states the local/unpublished boundary, while the
site-only instructions appear only for HTML previews. Three navigation hit
targets (main navigation, confirmed-workspace back link and confirmed-folder
disclosure) now meet the design's 48px minimum. This changes no native command,
preview sandbox, conversation history or Forge state.

**PASS:** the controlled browser suite checks copy by preview type and hit
targets. An offline locked Desktop-only debug build passed. The hidden native
WebView smoke also checked these exact states on the updated source, plus real
project onboarding and bounded local HTML preview. **NOT_RUN:** a new real
Codex reply, manual screen-reader/contrast acceptance and a packaged update.
The installed/public 0.1.9 installer does not include this source-only slice.
No Rust source changed, so Rust tests were not rerun. No commit, push, version
bump or release was made for this slice; model-specific tokens and BRL cost
remain UNKNOWN.

**Next:** continue testing the actual nontechnical result/change journey for
a substantive gap; group source changes into a coherent alpha before the next
installer. Keep the overall journey goal active.

## Desktop multiple cited files — 2026-09-27

A completed Codex reply can cite several local files. Previously the preview
offered a shortcut only for exactly one distinct citation; otherwise the
person had to return to the transcript or navigate the Windows file picker.
The preview now shows a collapsed **Arquivos citados na resposta** list when
the latest completed reply cites two or more distinct supported paths. It
never picks a "result" automatically. Each choice uses the existing native
project-bound `inspect_preview`; a cited outside or unavailable path still
fails safely. The list remains available after opening one file, survives a
restored conversation, and clears when the person switches project or starts
a different conversation. A long list renders at most 20 choices, explicitly
pointing back to the transcript for the rest. The transcript remains owned by
Codex; there is no new persisted UI history or Forge record.

**PASS:** browser suite covers distinct/repeated citations, native call
arguments, clearing on new conversation/project, and existing short/long
history behavior. Eight Node tests, syntax checks and `git diff --check`
passed. A Desktop-only offline locked debug build and focused hidden native
full-process restart test passed: two controlled citations reopened a real
file through native validation; an outside path was rejected. The native
screenshot `C:/ForgeFast/forge-multi-citations-native.png` was visually checked.
The fixture simulates Codex history; a new real model turn was **NOT_RUN** in
this slice. No Rust source changed or Rust tests were rerun. This and the
preceding preview-copy slice remain uncommitted source; public/installed
0.1.9 is unchanged. No model-specific token or BRL counters were available.

**Next:** validate a larger nontechnical result/change package using a real
Codex turn on the updated source, then prepare one coherent alpha candidate
and installed upgrade rather than publishing every small UI edit. Manual
screen-reader/contrast acceptance and clean-machine/self-contained setup
remain separate work.

## Desktop 0.1.10 candidate and installed upgrade — 2026-09-27

The source-only preview clarity and multiple-citation changes are packaged as
0.1.10 alpha.1. Before the metadata bump, a hidden native journey used an
authenticated, isolated Codex CLI 0.157.1 at
`C:/ForgeFast/forge-codex-0157/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe`.
It initialized a disposable project, sent to real Codex, created actual
`site/index.html`, previewed it, asked for a visible change in the same chat,
and restored the conversation after WebView reload without another Send.
Capture: `C:/ForgeFast/forge-0110-real-artifact-native.png`. This test did not
exercise several citations or full process restart with real Codex; the
controlled hidden native test covered those using native file reads.

**PASS:** browser suite, eight Node tests, Desktop-only check, 17 focused
agent tests, all 44 Desktop Rust tests, format check, strict Desktop Clippy,
release build and the focused release-executable hidden native test. The
first locked check after changing Cargo version could not update Cargo.lock;
one offline non-locked check updated the local lockfile, then locked tests
passed. No separate core workspace or manually triggered GitHub CI run.

**One candidate:**
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.10_x64-setup.exe`,
4,621,292 bytes, SHA-256
`4327DC375F0374E14A65531E0E36335CAA4CC8551962178B83BD3A44B474F8C6`.
Its sidecar is beside it; do not rebuild after hashing. It silently upgraded
the local 0.1.9 installation with exit 0. Installed version is 0.1.10,
executable SHA-256
`2B1A87BE769DAF86A2ABD255A6A8C2089B76C86B1B7E5E0FBBFBD8254E049533`.
The installed candidate again passed the focused hidden native full-process
restart, multiple-citation, prepared-change and outside-path rejection test.
The installed version did not run a fresh real Codex turn.

**Next:** review source and notes, selectively commit/push, tag and publish
the exact candidate under the maintainer's standing alpha release permission;
then verify an unauthenticated public download and the same downloaded bytes
on installation. Until that readback passes, 0.1.10 is installed locally but
not publicly available. Manual screen-reader/contrast acceptance, compatible
Codex/core setup on a clean machine, self-contained installer, auto-update
and mobile remain unfinished. Model-specific token and BRL costs are UNKNOWN.

## Desktop 0.1.10 public release readback — 2026-09-27

**PASS:** source commit `53c0baaf` was pushed; annotated tag
`desktop-v0.1.10-alpha.1` resolves to it. Public prerelease:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.10-alpha.1`.
GitHub's asset digest is SHA-256
`4327DC375F0374E14A65531E0E36335CAA4CC8551962178B83BD3A44B474F8C6`
for 4,621,292 bytes. A fresh unauthenticated download into
`C:/ForgeFast/forge-alpha-0110-public-20260927` matched this size, hash and
sidecar. The downloaded installer, not a rebuilt file, silently reinstalled
with exit 0. Installed version remained 0.1.10 and executable SHA-256 remained
`2B1A87BE769DAF86A2ABD255A6A8C2089B76C86B1B7E5E0FBBFBD8254E049533`.
Those installed downloaded bytes passed the focused hidden native full-process
restart, multiple-citation selection, prepared change and outside-path
rejection test. No GitHub CI was manually started. The prior real Codex
create/change journey used the same UI source before version metadata changed;
it was not repeated on the downloaded installer.

**Next product work:** make the nontechnical journey usable without a
developer-provided Codex CLI override. This alpha still needs a compatible
separate CLI and Forge core; default machine discovery may select an older CLI.
Do not call it clean-machine or self-contained. Investigate a safe, maintainable
setup/update path before another UI-only release. Manual screen-reader/contrast
acceptance, auto-update and mobile remain open. Model-specific token and BRL
costs remain UNKNOWN.

## Desktop 0.1.11 bundled Codex candidate — 2026-09-27

**Objective and scope:** remove the ordinary user's need to supply a compatible
Codex CLI while preserving Forge/Codex ownership, host override for development
and safe fallback on older source. No subagents were used; model routing and
cost attribution remain with the parent. Model-specific usage and BRL are
UNKNOWN, not zero.

The source pins `@openai/codex@0.157.1` in Desktop's npm lockfile and includes
its full Windows x64 native vendor tree as a Tauri resource. Runtime resolution
prefers the bundled executable after an explicit `FORGE_CODEX_EXE` override and
before older per-user installations. The upstream tag's LICENSE and NOTICE are
included; no binary is tracked in Git. `apps/desktop/third-party/codex-0.157.1/README.md`
records provenance. Source version is 0.1.11; public release remains 0.1.10
until a public download is checked. No project-state or UI-data path changed.

**PASS:** pinned `npm ci`; Desktop `cargo check`, 18 agent tests, all 45 Desktop
Rust tests, strict Desktop Clippy, eight Node tests, browser suite and NSIS
release build. A hidden native run of the debug app with `FORGE_CODEX_EXE`
unset sent to real authenticated Codex and restored the reply after WebView
reload. The exact NSIS candidate silently upgraded installed 0.1.10 with exit
0; installed resource `codex-cli/bin/codex.exe` reports 0.157.1 and hashes to
`8CB0E69E99FF2A158C54815DB82D0F2E524D8F301BC30184722CFD1AE5973574`.
Installed app without override again sent a real reply and restored it after
WebView reload. A focused hidden native controlled-history full-process restart
then reopened a local file, retained distinct citations and rejected an
outside path. The first native attempt failed only because Playwright was not
configured in that test shell; it passed when the existing module path was set.
No GitHub CI was manually triggered. Rust work stayed scoped to Desktop.

**One candidate:**
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.11_x64-setup.exe`,
111,295,705 bytes, SHA-256
`00E4E5F1C749C395A5844A50A926B88F6D62833934911F998B19D96CF6F1E6E4`.
Do not rebuild after hashing. Installed executable SHA-256 is
`FCF8AFB036789F070891541BBA3A66CC7CC194D9DC685CAE19C0BF1C14BD3A04`.
Exact limitations are in `apps/desktop/RELEASE_NOTES-0.1.11.md`: Forge core
still separate, no in-app sign-in, auto-updater, signing, clean-machine proof,
manual accessibility acceptance or mobile. A future provider change can make
the pinned Codex stale; a fresh real artifact create/change and full process
restart of that same real conversation are NOT_RUN on installed 0.1.11.

**Next exact step:** review the source/notes and Git diff, selectively commit
and push, tag this source, publish only the exact tested installer candidate
under the maintainer's standing alpha permission, then fresh-download it
without authentication, compare size/SHA-256 and install those downloaded
bytes before calling 0.1.11 publicly available. After that, continue the
journey with Forge core installation and genuinely clean-machine setup.

## Desktop 0.1.11 public release readback — 2026-09-27

**PASS:** source commit `7c5b83d5d5e317d9d9b812cf17bb4915e7decb8e`
was pushed and annotated tag `desktop-v0.1.11-alpha.1` resolves to it.
Public prerelease:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.11-alpha.1`.
GitHub reports the installer asset at 111,295,705 bytes with SHA-256
`00E4E5F1C749C395A5844A50A926B88F6D62833934911F998B19D96CF6F1E6E4`.
A fresh unauthenticated public download into
`C:/ForgeFast/forge-alpha-0111-public-20260927` matched the tested candidate's
size, hash and sidecar. The downloaded installer silently reinstalled with
exit 0. Installed product version is 0.1.11; installed app and Codex executable
hashes matched the earlier tested candidate. Installed downloaded bytes then
passed hidden native project onboarding and real authenticated Codex send/
response/reload with `FORGE_CODEX_EXE` unset. No manually triggered GitHub CI.

**Next product work:** bundle or provide a safe guided installation/update path
for `forge-core`, then test the nontechnical journey on a genuinely clean Windows
profile or machine. Do not call this alpha self-contained until that proof passes.
In-app ChatGPT sign-in, auto-update, signing, accessibility acceptance and
mobile remain open. The pinned Codex CLI needs deliberate compatibility review
for future alpha packages. Model-specific token and BRL costs remain UNKNOWN.

## Desktop 0.1.12 bundled core candidate — 2026-09-27

**Objective:** remove the separate `forge-core` install from the ordinary
Windows Desktop journey without changing Forge's project authority or replacing
the machine's existing installation. Source `0.1.12` pins the published core
`v0.13.2` Windows archive and the extracted binary by SHA-256. The canonical
`npm run build:nsis` stages it with `scripts/prepare-core.ps1`, then Tauri's
additional `tauri.bundle.conf.json` includes it only in the installer. Runtime
selection is explicit `FORGE_CORE_EXE`, then app-bundled core, then the prior
per-user installation. `apps/desktop/third-party/forge-core-0.13.2/README.md`
records provenance. The bundled core does not change Codex history or the
Forge sidecar format. No subagents were used; model-specific tokens and BRL are
UNKNOWN, not zero.

**PASS:** the public v0.13.2 archive downloaded without authentication matched
SHA-256 `27976049225D8650758D2593B5CB06C0FC20870216383C16C7FBC70478AD23BB`;
its `forge-core.exe` is version 0.13.2 and SHA-256
`CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`.
Before bundling, the installed 0.1.11 app passed hidden native onboarding,
record readback and real Codex send with that binary as a host override. The
new staging script downloaded and verified a fresh archive, and a second fresh
pass after moving aside its cache reproduced the same binary hash. The first
`npm run build:nsis` attempt failed because `Get-FileHash` was unavailable in
the npm child PowerShell; the script was changed to use .NET hashing, then the
canonical npm build passed. Desktop-focused check/5 project tests, all 46
Desktop Rust tests, strict Desktop Clippy, eight Node tests, browser suite and
NSIS release build passed. No core workspace build or manual GitHub CI run.

**One candidate:**
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.12_x64-setup.exe`,
123,060,687 bytes, SHA-256
`34003537959E05A97377C2C55FD2A788B89BB3707BCAE6A18843B74101B107AC`.
Do not rebuild after hashing. The NSIS archive contains both CLI executables and
license/notice resources. It silently upgraded installed 0.1.11 with exit 0;
installed Desktop version is 0.1.12, app SHA-256
`EA432B88E2FE58C26ED413757B90DF7098008E59E55C1CE93593ED93C19E3D21`.
Bundled core and Codex hashes match the verified source binaries.

**PASS native journey:** on the installed candidate, with both executable
overrides unset and an empty alternate `LOCALAPPDATA`, a hidden Windows desktop
initialized a fresh folder, read its Forge record, sent to authenticated real
Codex, created real local HTML, previewed it, changed it in the same chat,
reloaded WebView and then restored all real messages in order after a full app
process restart without another Send. The earlier pass with no real agent also
proved project onboarding when the fallback global core path was absent. The
captured result `C:/ForgeFast/forge-0112-bundled-journey.png` was visually
checked. The folder-dialog result in these runs was simulated; this account's
existing Codex login and WebView2 were reused, so this is not clean-machine
sign-in/install proof. Manual accessibility acceptance, auto-update, signing
and mobile remain open. Source core is now 0.13.3 but this Desktop pins the
last verified released 0.13.2.

**Next exact step:** review the diff and release notes, selectively commit/push,
tag/publish the exact candidate under standing alpha permission, download the
public asset without authentication, compare size/hash and install those exact
bytes. Then audit first-use/authentication and update UX on a genuinely fresh
Windows account; do not claim it proved clean-machine setup yet.

## Desktop 0.1.12 public release readback — 2026-09-27

**PASS:** source commit `1ae391b0d9dfff4733bfff9608d28b984962f394`
was pushed; annotated tag `desktop-v0.1.12-alpha.1` resolves to it. Public
prerelease:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.12-alpha.1`.
GitHub reports 123,060,687 bytes and SHA-256
`34003537959E05A97377C2C55FD2A788B89BB3707BCAE6A18843B74101B107AC`
for the installer. Fresh unauthenticated download and sidecar in
`C:/ForgeFast/forge-alpha-0112-public-20260927` matched the tested candidate.
The downloaded installer silently reinstalled with exit 0. Installed version
0.1.12 and app/core/Codex executable hashes exactly matched the earlier tested
candidate. Those installed downloaded bytes passed hidden native folder
onboarding and real Codex send/reply/reload/full-process restart with both
executable overrides unset and an alternate empty `LOCALAPPDATA`. No manual
GitHub CI was started. The full real create/preview/change journey had already
passed on the same installed executable/resource hashes before the public
download; it was not repeated after the identical public download.

**Next product work:** inspect first-use ChatGPT authentication and update UX
for people without any Codex setup, then run a genuinely fresh Windows account
or VM test including installer, folder dialog, sign-in, artifact creation/change
and restart. Current tests reused this account's existing Codex login and
WebView2, so clean-machine setup is NOT_RUN. Manual accessibility acceptance,
auto-update, signing and mobile remain open. Source core 0.13.3 is not the
bundled 0.13.2. Model-specific token and BRL costs remain UNKNOWN.

## Desktop first-use ChatGPT access worktree checkpoint — 2026-09-27

**Active goal:** finish the approachable Windows journey from folder choice
through a real Codex conversation, local result, requested change and restart.
The public installed release remains **0.1.12**; the following changes are
source-only in the dirty `codex/desktop-shell` checkout, not committed or
published. No subagents were used; model-specific tokens and BRL remain UNKNOWN.

The app now offers a Codex-managed ChatGPT device-code sign-in when a signed-out
person first tries to send or search history, rather than instructing them to
install/open Codex CLI. The draft stays in place and is **not** automatically
sent after authorization. Forge only displays the temporary code and official
verification address; it does not receive the password or tokens. The native
backend validates the exact `https://auth.openai.com/codex/device` URL before
offering to open it, and supports check/cancel/close cleanup. The login panel
has a layout exception so it cannot overflow the fixed-height chat card.
Files: `src-tauri/src/agent.rs`, `src-tauri/src/main.rs`, `ui/chat.mjs`,
`ui/index.html`, `ui/styles.css`, `tests/browser.cjs`,
`tests/native-auth.cjs`, `tests/native-hidden.ps1`, and this checkpoint.

**PASS:** bundled Codex CLI 0.157.1 app-server schema confirmed the device-code
request/response and completion event; an isolated `CODEX_HOME` probe returned
signed-out account, device challenge at the official URL and a successful
cancel, without opening a browser. Desktop `cargo check`, focused login unit
test, all 47 Desktop Rust tests, strict Desktop Clippy, JS syntax and browser
suite passed. The browser suite simulates authorization completion and confirms
exactly one send after it. A hidden native WebView test with a fresh isolated
Codex home and **real** bundled Codex executable confirmed signed-out detection,
challenge display, read-before-completion, cancel, zero sends and preserved
draft. Native visual screenshot was inspected; the first fixed-height layout
overflow was corrected and a second native run passed after the correction.
No manual GitHub CI or core workspace build was run.

**NOT_RUN:** actual completion of ChatGPT authorization through a browser on a
fresh Windows account, authenticated send after that new login, default-browser
launch from the button, installer/update proof for these uncommitted changes,
and full clean-machine setup. Do not call in-app sign-in or 0.1.13 released yet.
The headless test intentionally cancels the challenge and cannot prove those
steps. Existing installed 0.1.12 remains the public alpha.

**Next exact step:** run a voluntary end-to-end sign-in on a genuinely fresh
Windows account or VM, then send a real message, restart and inspect the result.
If it passes, bump Desktop to 0.1.13, run focused final checks and one NSIS
candidate, upgrade the installed 0.1.12, review diff/release notes, publish
under the standing alpha authorization and byte-check the public download.

### Continuation: native login-completion wiring — 2026-09-27

The browser-opening command now calls Windows `ShellExecuteW` with the `open`
verb and the exact vetted HTTPS address; it no longer depends on undocumented
`explorer.exe URL` behavior. Microsoft documents success as a return greater
than 32, but an actual browser launch on a fresh account is still **NOT_RUN**.
Added a no-network fake app-server fixture under `tests/fixtures/` and a second
mode of the hidden native authentication test. That mode proved the native
`account/login/completed` notification reaches the WebView, triggers account
readback, hides the panel, preserves the draft and enables sending without
silently sending it. The real bundled Codex signed-out/cancel test still passes
separately. The fixture proves integration wiring, **not** provider login or
fresh-machine readiness. This checkpoint remains uncommitted and public 0.1.12
is unchanged. Next exact step remains a voluntary real browser authorization
on an isolated/fresh Windows account, followed by package candidate validation.

### Continuation: tested 0.1.13 installer candidate — 2026-09-27

The Desktop source was bumped to **0.1.13** and one NSIS candidate was built:
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.13_x64-setup.exe`,
123,011,405 bytes, SHA-256
`76D758454DFFB2508029E4EF81AFC5AA6EBD82A31015AFD894EEB5E548FF0D33`.
It installed silently over the public 0.1.12 with exit 0. The installed app
reports file/product version 0.1.13 and the bundled core/Codex hashes remain
identical to the verified 0.1.12 resources. This is an installed **local**
candidate, not yet a public 0.1.13 download.

**PASS on installed candidate, hidden native desktop, executable overrides
unset:** isolated signed-out Codex home offered the genuine device challenge,
preserved the draft and canceled without a Send. A separate no-network fixture
proved the native completion notification and account readback/UI transition.
The existing authenticated account also sent one real Codex turn from a fresh
temporary Forge project and restored the ordered exchange after WebView reload
and full process restart, without resending. The actual folder dialog result
was simulated in this regression. Focused Desktop Rust checks, 47 tests,
strict Clippy and browser tests had passed before the candidate build. No
core workspace build or manual GitHub CI was triggered.

**NOT_RUN:** actual completion of a new ChatGPT browser authorization, opening
the browser from the button, and fresh Windows account/VM setup. Fixture
completion is not provider proof. Manual accessibility acceptance,
auto-update, signing and mobile remain open. The user approved publishing
alpha building blocks with explicit limitations. No subagents were used; model
specific tokens and BRL cost are UNKNOWN.

**Next exact step:** review and selectively commit/push the 0.1.13 source and
release notes, publish the exact tested candidate under the standing alpha
authorization, then unauthenticated download/hash check and installed-byte
readback. Later obtain voluntary genuine fresh-account sign-in proof.

### Desktop 0.1.13 public release readback — 2026-09-27

**PASS:** commit `c14ea34bcb27a85891e207118a32d24f533fd840` was
pushed and annotated tag `desktop-v0.1.13-alpha.1` resolves to it. Public
prerelease:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.13-alpha.1`.
GitHub reports 123,011,405 bytes and SHA-256
`76D758454DFFB2508029E4EF81AFC5AA6EBD82A31015AFD894EEB5E548FF0D33`.
An unauthenticated public download and sidecar in
`C:/ForgeFast/forge-alpha-0113-public-20260927` matched the locally tested
candidate exactly. The downloaded installer silently reinstalled with exit 0.
Installed product version is 0.1.13; app, core and Codex resource hashes
match the previously tested installed candidate. A hidden native test of the
installed downloaded bytes passed onboarding, real authenticated Codex
send/reply, WebView reload and full process restart without resending, with
both executable overrides unset. No manual GitHub CI was started.

**Remaining:** a genuinely new ChatGPT login through the browser and fresh
Windows account/VM are still NOT_RUN; the actual default-browser button is
NOT_RUN. The device-code start/cancel and controlled completion fixture passed
separately but cannot prove real provider authorization. Manual accessibility
acceptance, automatic updates, signing and mobile remain open. The next
product slice is a fresh-account sign-in/installer journey if a controlled
account or VM is available, followed by concrete fixes; do not add proxy
complexity solely to disguise this proof gap. Routing and BRL usage remain
UNKNOWN because model-attributed counters are not available. No workers were
used in this slice.

### Desktop visual-language pass after 0.1.13 — 2026-09-27

**Active goal:** an approachable Windows journey from choosing a folder through a
real Codex conversation, local result/change and restart, without inventing
Forge state. Public/installed 0.1.13 remains unchanged. The current checkout
has **uncommitted source changes** in `apps/desktop/ui/index.html`,
`apps/desktop/ui/progress.mjs`, `apps/desktop/tests/browser.cjs`, and
`apps/desktop/tests/native.cjs`; preserve them on continuation. No worker is
active. Model-attributed token/BRL cost is UNKNOWN.

A hidden native run of the **published 0.1.13** used the actual Windows folder
dialog twice: canceled first, then selected `D:/Forge-method-core`, confirmed
that selection did not itself accept the project, and continued through Forge
project readback/onboarding. PASS. The preview file picker in that run remained
simulated; no real Codex send was requested in this specific dialog test.

A visual review of the native conversation screen found that the visible
progress panel overused the internal word “registro”. Source-only copy now
uses “Atualizar andamento”, “Agora”, “Próximo passo” and plain-language state
labels, while retaining Forge provenance, stale warnings, recorded decisions
and exact backend values. The UI does not claim the latest chat was committed
to Forge. Browser suite PASS after this change. One debug native build and
hidden native test PASS with a real folder selection and Forge readback; a
viewport screenshot was inspected at `C:/ForgeFast/forge-0113-copy-viewport.png`.
A final one-word correction to the stale-phase label was browser-tested PASS
but not recompiled/re-run natively. No Rust source changed; no Rust workspace
suite or GH CI was run. No commit, push, version bump, or installer publication
was done for this source-only UI pass.

**Next exact step:** continue the visual-language review of the project record
and empty/error states, run the browser suite and a single native debug check
for the consolidated UI slice, then decide whether it forms a coherent next
alpha package. A genuinely new ChatGPT authorization in a controlled Windows
account/VM and default-browser button remain NOT_RUN; do not infer them from
fixture completion or this dialog test.

### Desktop record-language consolidation and native pickers — 2026-09-27

The same uncommitted UI slice now also simplifies the project-record loading,
empty, slow and failure messages: the main action is “Atualizar andamento”,
errors say what stayed unchanged, and decision counts say only what Forge shows
in its current accompaniment. Exact recorded activities/outcomes and provenance
are still displayed, not synthesized from the latest chat. Files remain
`apps/desktop/ui/index.html`, `ui/progress.mjs`, `tests/browser.cjs`,
`tests/native.cjs`, and this README. Public/installed 0.1.13 is unchanged.

**PASS:** JS syntax, full browser suite, one incremental Desktop debug build,
and hidden native WebView regression with the updated UI. The native run used
both real Windows dialogs: cancel/select for the project folder and selection
of a real local preview file; then read Forge project state and safely rendered
local content. A final native viewport screenshot was inspected at
`C:/ForgeFast/forge-record-copy-final.png`. A read-only Sandbox feasibility
probe showed Windows 10 Pro with a hypervisor, but optional-feature status
requires elevation and was not established. No Rust source changed, no
workspace-wide tests, GH CI, commit/push, version bump or installer build was
run. The native picker regression did not send a real Codex message; the
published 0.1.13 real-send/restart proof remains separate.

**Next exact step:** inspect the remaining first-use/empty conversation screens
with the same plain-language standard, correct only concrete issues, then
run one browser/native regression for the consolidated package. After that,
consider a 0.1.14 alpha if the package is coherent. Genuine browser completion
of a new ChatGPT login and fresh-account/VM setup are still NOT_RUN and need a
controlled account/interactive authorization; do not simulate them as proof.
No subagents are active; token attribution and BRL economics remain UNKNOWN.

### Desktop first-use clarity pass — 2026-09-27

The dirty UI package also removes a redundant disabled “Entrar com ChatGPT”
button after a device code is shown and replaces the raw signed-out transport
error with a clear statement that the message was not sent. History-search
errors give an equivalent human-facing instruction. The login panel no longer
claims the account will need connecting only once on this device; it says the
human chooses when to send after entering. It avoids repeated code-entry
instructions while retaining the password-safety reminder. Files additionally
changed: `ui/chat.mjs`, `ui/styles.css`, `tests/native-auth.cjs` (plus earlier
files); no backend/auth protocol was changed.

**PASS:** full browser suite after the final copy edit. Before that final
sentence edit, a debug native build with an isolated signed-out CODEX_HOME
passed genuine device-code start/cancel, preserved the draft and proved the
initial login button disappears while the challenge is active. Screenshot
`C:/ForgeFast/forge-first-login-revised.png` was inspected. The last copy-only
sentence edits have not been rebuilt or retested natively; do that once for a
consolidated package, not after every wording adjustment. The real browser
authorization completion remains NOT_RUN, as does fresh-account/VM setup.
No subagents; model/token/BRL attribution UNKNOWN. Source changes remain
uncommitted and are not present in installed public 0.1.13.

**Next exact step:** review the integrated dirty diff, run one final debug
native first-use plus real conversation/preview/restart regression for this UI
package, then decide on a coherent 0.1.14 candidate and release notes. Do not
claim the controlled completion fixture is real account login proof.

### Desktop 0.1.14 installed candidate — 2026-09-27

The first-use/progress-language UI slice was consolidated and versioned as
Desktop **0.1.14**. One NSIS candidate was built at
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.14_x64-setup.exe`:
123,057,736 bytes, SHA-256
`BA7E26638A91224AC9DB939EF4BA2C844B74DB9C1F7BECE3D2D93FAF0BF2970C`.
It installed silently over the public 0.1.13 with exit 0. Installed product
version 0.1.14 and core/Codex hashes matched the intended bundle. This is a
**local tested candidate**, not yet a public 0.1.14 download.

**PASS:** Desktop cargo check, one focused login unit test (one selected), all
47 Desktop Rust tests, strict Desktop Clippy, JS/browser suite, native
isolated signed-out device-code start/cancel (no Send), and the installed
candidate's full hidden native journey with both executable overrides unset.
That journey used real Windows folder and preview-file dialogs, a real
existing authenticated Codex account, a real HTML artifact and preview, a
follow-up change in the same conversation, WebView reload and full process
restart with ordered history and no resend. The installed screenshot
`C:/ForgeFast/forge-0114-installed-artifact.png` was visually inspected. The
separate debug run had passed the same journey before packaging. No core
workspace build or manual GitHub CI was run.

**NOT_RUN:** genuine completion of a new ChatGPT authorization in a browser,
the browser-opening button and fresh Windows account/VM setup. The signed-out
native run cancels the challenge; the existing account proves conversation
behavior but not new-account entry. Manual accessibility acceptance,
automatic updates, signing and mobile remain open. No subagents were used;
model-specific tokens and BRL cost are UNKNOWN. Current source is dirty, with
0.1.14 code/release notes not yet committed, pushed, tagged or published.

**Next exact step:** review/selectively commit and push 0.1.14, publish the
exact tested candidate under the maintainer's standing alpha authorization,
then unauthenticated download/hash check, install the downloaded bytes and
native smoke. Later test a genuine new-account authorization voluntarily.

### Desktop 0.1.14 public release readback — 2026-09-27

**PASS:** source commit `ac5bf4dc0cb9826c7f4d23cefd11d0f3b13bec32`
was pushed; annotated tag `desktop-v0.1.14-alpha.1` resolves to it.
Public prerelease:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.14-alpha.1`.
GitHub reports 123,057,736 bytes and SHA-256
`BA7E26638A91224AC9DB939EF4BA2C844B74DB9C1F7BECE3D2D93FAF0BF2970C`.
An unauthenticated public download and sidecar in
`C:/ForgeFast/forge-alpha-0114-public-20260927` matched the exact tested
candidate. The downloaded installer silently reinstalled with exit 0. The
installed version is 0.1.14 and executable SHA-256
`3FA6E29DBF05DD502FCA82332974A68066048A134974D1748A1290C1E52A1E02`,
identical to the locally tested installation. Those downloaded installed bytes
passed a hidden native Windows folder/file-dialog and real authenticated
Codex send/reply/reload/full-process-restart regression with executable
overrides unset. The complete real artifact-create/preview/change journey had
already passed on the identical locally tested installed executable/resources
before the public download; it was not repeated after the byte-identical
public reinstall. No manual GitHub CI was started.

**Remaining:** genuine browser completion of a new ChatGPT login,
default-browser launch, and clean Windows account/VM setup are NOT_RUN. The
installed app's sign-in challenge/cancel is proven, but fixture completion is
not provider authorization. Manual accessibility acceptance, auto-update,
signing and mobile remain open. Core source 0.13.3 remains separate from the
pinned bundled core 0.13.2. Model-specific token/BRL economics are UNKNOWN.
No workers were used. Next product step: audit current UI coverage against
approved user journeys and original art, identify the highest-impact missing
screen/state and implement it without adding another authority store; test
fresh-account authorization when a controlled interactive account/VM is
available. No source changes are pending after this checkpoint commit.

### Desktop UI coverage: first visit to My Projects — 2026-09-27

The current UI audit is recorded in `design/ux-audit-2026-09-26.md` under
“Current journey coverage review.” It maps the approved user journeys and
visual references. The first concrete gap selected was the empty My Projects
screen: it offered only an existing-project route. Current source now explains
that a project folder may be empty or pre-existing and offers two honest
routes, **Explorar ideias** and **Escolher uma pasta**. The heading changes to
**Abrir outro projeto** only after a local shortcut exists. No new project
store, automatic send or Forge authority was introduced.

The same first-use package also replaces the browser's implicit missing-folder
validation bubble with a visible instruction and field focus when the person
presses **Continuar nesta pasta** without a path. Whitespace-only paths stay
local; choosing or typing a folder clears the invalid state.

**PASS:** full browser UI suite, including empty/populated labels, both routes,
shortcut revalidation and narrow layout; visual review of the controlled empty
screen at `C:/ForgeFast/forge-projects-empty-final.png`; incremental Desktop
debug build; hidden native WebView smoke with real Forge project onboarding and
readback. A second hidden native run specifically proved missing-folder
feedback, real Windows folder-dialog cancellation/selection and subsequent
onboarding. The native smoke did not send a Codex turn. No Rust source changed,
so no Rust workspace suite, release build or manual GH CI was run. This source
is **not** in the installed/public 0.1.14.

**NOT_RUN:** genuine new-account ChatGPT browser completion, default-browser
button, manual accessibility acceptance, and a new real Codex send in this
slice. No subagents; per-model token/BRL economics remain UNKNOWN. Source
changes are local and uncommitted. **Next exact step:** run a bounded, real
Codex-on-disposable-project first-use probe to determine whether the agent
actually invokes the installed Forge guidance and updates authoritative
project continuity, rather than only chatting and producing a file. Inspect
the real thread/tool evidence and Forge readback; if it fails, diagnose the
integration before more visual polish. Group the verified correction with
this first-use UI work into a coherent alpha package, not a release per label.

### Desktop first-use Codex-to-Forge runtime alignment — 2026-09-27

A real disposable-project probe found that the app's project commands used
bundled `forge-core 0.13.2`, while the Codex agent's shell found an older
machine-wide `forge-core 0.12.1`. Current source passes its already-resolved
project runtime to project-bound Codex subprocesses, prepends only that
directory to their `PATH`, and sets `FORGE_CORE_EXE`. It changes neither the
global PATH nor the login-only subprocess. The Start Forge skill is still
host-installed, not bundled for a clean machine.

**PASS:** focused runtime-PATH unit test, all 48 Desktop Rust unit tests,
Desktop `cargo check`, strict Desktop Clippy, incremental debug build, full
browser UI suite, and hidden native first-use/folder-dialog smoke. Two bounded
real Codex turns were inspected through local tool traces: the first proved
the 0.12.1 mismatch; the second, after the source fix, resolved and ran core
0.13.2 for `start` and `workflow resume`, consulted discovery guidance, asked
a relevant product question, and restored the conversation after WebView
reload without resending. The second turn used a disposable project; no app
release or manual GH CI was started. The test harness now reads the **last**
agent message, not an earlier progress message, for this real-turn assertion.

**PASS:** a separate hidden native run with a real Codex reply restored the
ordered conversation after WebView reload and full desktop process restart,
without sending the prompt again. This restart probe used the bounded
no-tools prompt, not another Forge-activation turn.

**PARTIAL:** the activation agent tried Forge's `decision_required` request,
but its shell tool rejected the composed command by host policy before
execution. That request is read-only by design; no durable decision write
should have been expected before the human answers. The typed runtime return
remains unverified. **NOT_RUN:**
fresh-account browser authorization, default-browser launch, and clean-machine
skill/core/Codex setup. No subagents; model-specific token/BRL economics remain
UNKNOWN. Source changes remain local/uncommitted; installed/public 0.1.14 is
unchanged. **Next exact step:** package and verify the coherent first-use 0.1.15
alpha, then investigate the rejected decision-request command without
weakening the app's authorization boundary or inventing acceptance.

### Desktop 0.1.15 installed alpha candidate — 2026-09-27

The coherent first-use package was versioned 0.1.15. The focused browser UI
suite, Desktop cargo check, all 48 Desktop Rust unit tests and strict Desktop
Clippy passed. One NSIS release build produced
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.15_x64-setup.exe`:
123,028,746 bytes, SHA-256
`4491A0B935B190F02A0990A2C276FA1A258FB18D2F892FA07C18C6B5FC8B47DB`.
It silently upgraded the installed 0.1.14 with exit 0. Installed product
version is 0.1.15; installed executable SHA-256 is
`48F8485E7D24D5DD0DEF1DD3A99AF782388F33FAF4FD27BBC1E132990A54CDC2`;
installed bundled Forge core SHA-256 is
`CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`.

**PASS:** on those installed bytes, with core/Codex overrides unset, hidden
native WebView exercised real Windows folder-dialog cancellation/selection,
project readback, a real Codex first Send, WebView reload and full process
restart without resending. The Codex trace for thread
`01a0e40a-6889-7f70-a9d1-c0a7f7d323cf` confirms it invoked the installed
bundled Forge core path for `start`; the reply used the project's discovery
state, asked a relevant question, and made no files or publications. The
installed test used the host-installed Start Forge skill. No manual GH CI ran.

**PARTIAL:** the typed read-only `decision_required` return remains unproven
because the earlier controlled agent command was rejected before execution.
No durable write is expected before the human chooses. **NOT_RUN:**
fresh-account browser login completion, default-browser button, clean-machine
skill availability, manual accessibility acceptance and mobile. This is a
tested local candidate, not yet a public installer. Model-specific tokens and
BRL cost remain UNKNOWN. **Next exact step:** selectively commit/push this
package, publish the exact tested installer under the maintainer's standing
alpha authorization, verify an unauthenticated download against this SHA-256,
install those downloaded bytes and rerun a bounded hidden native smoke.

### Desktop 0.1.15 public release readback — 2026-09-27

**PASS:** source commit `3e8110582dd05788420273a77b4a2b3808f9633d`
was pushed; annotated tag `desktop-v0.1.15-alpha.1` resolves to that commit.
Public prerelease:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.15-alpha.1`.
GitHub reports one installer asset of 123,028,746 bytes with SHA-256
`4491A0B935B190F02A0990A2C276FA1A258FB18D2F892FA07C18C6B5FC8B47DB`.
An unauthenticated download to
`C:/ForgeFast/forge-alpha-0115-public-20260927/Forge_0.1.15_x64-setup.exe`
matched the exact local candidate. Reinstalling that downloaded file silently
returned exit 0. Installed product version 0.1.15 and executable SHA-256
`48F8485E7D24D5DD0DEF1DD3A99AF782388F33FAF4FD27BBC1E132990A54CDC2`
match the locally tested installation; bundled core and Codex hashes are
`CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`
and `8CB0E69E99FF2A158C54815DB82D0F2E524D8F301BC30184722CFD1AE5973574`.

**PASS:** those downloaded installed bytes passed hidden native folder-dialog
cancel/select, project readback, real Codex send/reply, WebView reload and full
process restart without resending, with executable overrides unset. This
public-download regression used a bounded no-tools prompt; the separate
byte-identical local-candidate probe established Forge activation/core 0.13.2.
No manual GH CI was started. **NOT_RUN:** genuine new-account browser login
completion, default-browser button, clean-machine skill setup, manual
accessibility acceptance and mobile. The typed read-only decision request
remains unproven; no durable decision write is expected before the human
chooses. Per-model tokens/BRL cost remain UNKNOWN.

**Next exact step:** investigate the rejected read-only decision-request command
without weakening authorization; then verify clean-machine first use,
including Start Forge guidance and browser login. Continue improving the
nontechnical UI journey in coherent packages rather than treating 0.1.15 as
feature complete.

### Desktop 0.1.15 full installed journey and decision-request correction — 2026-09-27

**PASS:** a further hidden native run on the installed public 0.1.15 bytes,
with executable overrides unset, completed the full temporary-project path:
real Windows folder picker, Forge onboarding, real authenticated Codex chat,
local HTML creation, isolated in-app preview, a follow-up change to that file
in the same conversation, preview refresh, actual Windows preview-file picker,
WebView reload, and full desktop process restart with ordered history and no
resend. The harness exited 0; its temporary project was cleaned up. A separate
installed native screenshot at `C:/ForgeFast/forge-0115-project-native.png`
was visually inspected. It shows the real project/chat/Forge context layout;
that image alone does not prove the full interaction. No code change or manual
GH CI was needed for this regression.

**Correction:** the attempted `workflow intent accept-cooperative` input in the
earlier recipe-site probe used the `decision_required` variant. The installed
Start Forge guidance and current kernel/CLI tests show this variant validates
the packet and returns a typed Decision Request **without a ledger write**.
Therefore the previous expectation of a durable pending-decision write at
that stage was wrong. The agent's composed shell command was blocked by host
policy before Forge ran, so the typed return remains **NOT_RUN** in the app;
the question in chat is not equivalent proof. No accepted audience or new
durable objective is claimed. Do not weaken command safety or manufacture a
write to make this test pass.

**Next exact step:** test a bounded read-only `decision_required` call through
the real app agent with separate file/command/cleanup operations if needed,
then tackle clean-machine first use. Continue direct UI/visual review with
the approved artwork; the one inspected native project screen is not a
complete visual acceptance of all screens. Model-specific BRL cost remains
UNKNOWN.

### Desktop visual polish after 0.1.15 — 2026-09-27

Current controlled-browser captures of Início, Explorar, empty Meus projetos,
conversation and preview were reviewed against the approved art direction.
The installed native project/chat screen was reviewed separately. One concrete
issue in the conversation capture was fixed locally: decorative foliage no
longer enters the footer. `ui/styles.css` moves the crop inside the workspace,
and `tests/browser.cjs` now asserts that its lower edge clears the footer.
The full browser suite and incremental debug build passed; hidden native
WebView smoke passed without a Codex send, and a dark-theme full-page native
capture shows the footer unobscured. See
`design/ux-audit-2026-09-26.md` for capture paths and scope. No Rust source,
release build, installer or manual GH CI was changed/run; installed/public
0.1.15 remains unchanged. Keep this local UI fix for the next coherent package,
not a one-line release. The read-only typed decision request through a real
app agent and clean-machine setup remain **NOT_RUN**; model-specific BRL cost
remains UNKNOWN.

### Desktop 0.1.16 candidate: packaged Start Forge guidance — 2026-09-27

**Objective/phase:** close the host-installed Start Forge skill dependency for
the Windows desktop conversation, while retaining the small 0.1.15 visual
polish in one coherent alpha package. The active larger goal remains the
nontechnical folder-to-chat-to-result-to-restart journey; this is not its
completion.

**Changed:** `third-party/start-forge-0.13.2/SKILL.md` is an exact copy of the
`v0.13.2` tag (SHA-256
`10581E17D5DBB98BDA3E0F3BC0B6A152736499451E1424E093DBECFAFD8F0B06`).
`tauri.bundle.conf.json` includes it, `prepare-core.ps1` verifies its hash,
and `src/agent.rs` gives Codex its actual packaged path instead of asking for
the host skill. Release builds fail connection with an actionable error if
that resource is absent; debug builds retain the installed-skill fallback.
The native auth fixture now captures and checks those instructions without
using credentials. Desktop version is 0.1.16; release notes are in
`RELEASE_NOTES-0.1.16.md`. The earlier local footer-decoration fix and UI
audit are included. No other project store, engine or global skill changed.

**PASS:** focused Desktop `cargo check`, focused skill-path test, all 49
Desktop unit tests, strict Desktop Clippy, browser UI suite, pinned-core
preparation, NSIS release build, installation over public 0.1.15, installed
version/core/skill hashes, hidden native WebView smoke, and native simulated
login/first-send instruction capture. A further hidden real Codex activation
on the installed candidate returned a relevant Portuguese project reply,
restored it after WebView reload without resending, and its Codex trace
`01a0e428-7a32-7be2-82d7-55190410b04f` shows reads of the installed
bundled skill (first read truncated; subsequent section reads). It did not
write a product file or publish. The candidate installer at
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.16_x64-setup.exe`
is 123,136,684 bytes, SHA-256
`EFA67725B375DC33AAB811381DA692288D65320D7C2175691253C39006F8BA10`.
Installed executable SHA-256 is
`4A53F71AF2CD938236DDC629A478BF6E2E9C5E908BBF6C721E84B80F88B993D9`.
No manual GH CI was started.

**Measurement:** the one real activation trace reports cumulative
530,161 input tokens (476,800 cached subset), 3,959 output tokens (1,881
reasoning subset) on observed `gpt-6-sol`. This is not an isolated before/after
benchmark, and no API-equivalent BRL or attributable Pro spending is claimed:
both remain UNKNOWN. Parent work and earlier tests are not included in that
Codex-thread counter. The shared weekly allowance has no defensible per-test
attribution here.

**Limits/cleanup:** fresh-account browser completion, genuinely clean-machine
first use, typed read-only Decision Request, manual accessibility and mobile
remain NOT_RUN. The real activation used the current machine's authenticated
account. A disposable probe under
`C:/ForgeFast/forge-skill-probe-0116-20260927/` remains because the host policy
rejected recursive cleanup after path verification; it contains only the
temporary project and Forge sidecar, not user work. Do not claim cleanup.
This candidate is installed locally but not committed, pushed or published.

**Next exact step:** run the final relevant test set and inspect the integrated
diff; then selectively commit/push this coherent 0.1.16 package. Under the
maintainer's standing alpha-publication authorization, publish only the exact
tested candidate and verify an unauthenticated download, byte hash, and
installation. Do not rerun the costly real activation merely to check GitHub
bytes; use a bounded native regression on the downloaded installer. After
publication, resume the typed decision-request and remaining first-use gaps.

### Desktop 0.1.16 public release readback — 2026-09-27

**PASS:** commit `1afa856cee75c6687ba4f802072f8f71e874abf8` was pushed.
Annotated tag `desktop-v0.1.16-alpha.1` resolves to that commit. The public
prerelease is
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.16-alpha.1`.
GitHub reports one installer asset, 123,136,684 bytes, digest SHA-256
`EFA67725B375DC33AAB811381DA692288D65320D7C2175691253C39006F8BA10`.
An unauthenticated direct download to
`C:/ForgeFast/forge-alpha-0116-public-20260927/Forge_0.1.16_x64-setup.exe`
matched the exact local candidate. Silent reinstall of those downloaded bytes
returned exit 0; installed version 0.1.16, executable, bundled core and skill
hashes matched the tested installation. A hidden native fixture-login and
first-send regression on the downloaded installation again confirmed the
installed skill path and file hash. It did not complete a provider login or
execute the skill with a real agent; the earlier byte-identical candidate test
did the real authenticated activation. No manual GH CI ran.

The public installer is now available. The unsigned/no-auto-update and
clean-machine/login/Decision Request/accessibility/mobile limits in the
release notes still apply. The temporary probe directory mentioned above
remains because recursive cleanup was rejected by host policy. The broader
goal is still active. **Next exact step:** investigate the typed read-only
Decision Request path through a bounded real app-agent command, without
weakening safety, then verify fresh-account/clean-machine first use when a
suitable fixture or human login is available. Avoid repeating the full Start
Forge activation just to gather another token count.

### Decision-request command boundary diagnosis — 2026-09-27

The earlier failure was at the **host shell-tool policy**, before Forge ran:
the real agent had composed temporary JSON creation, the read-only
`workflow intent accept-cooperative` call and `Remove-Item` cleanup into one
PowerShell command. Re-reading that exact Codex trace confirms the rejection
was `CreateProcess ... blocked by policy`, not a Forge validation response.
No authorization or shell policy was weakened.

**PASS (installed core, standalone; not app-agent proof):** on the disposable
`C:/ForgeFast/forge-skill-probe-0116-20260927/project` fixture, the installed
bundled `forge-core 0.13.2` published a current Solo Cooperative objective
packet. A 538-byte UTF-8 `decision_required` input was written outside the
project in a separate tool call, and a separate direct Forge CLI call returned
`ok: true`, `status: decision_required`, the two exact alternatives and the
recommended option. Before and after, all 15 fixture files had the same
combined SHA-256 manifest
`784e45b45e7066ecba6907cd6f2df72ed6ca64a50c2e5ef5028db3f73f8ca6b6`;
no ledger write occurred. The input file was deleted separately with an
exact-file patch and its absence verified. Recursive removal of the earlier
fixture directory remains host-policy-blocked, so the fixture remains.

**Source change, not installed:** `src-tauri/src/agent.rs` now tells the app
agent to write JSON, invoke Forge and clean up in separate tool calls, never
as one composed shell command, and not to bypass host policy if blocked.
The focused Rust test, Desktop `cargo check`, and all 49 Desktop unit tests
pass. `tests/native-auth.cjs` will assert the new instruction at the next
native package check. No new real Codex turn, desktop build, installer,
commit, push or publication was made in this slice. The public/installed
desktop is still 0.1.16 and does **not** contain this new instruction.

**PARTIAL:** the installed core's typed read-only return is proven; the app
agent following the revised instruction in a real turn is still NOT_RUN. The
previous real agent did ask the relevant question in chat without a typed
return. Do not claim an app flow fix until a native app-agent check exercises
it. This slice collected no new model-token counter or BRL estimate; those
remain UNKNOWN. **Next exact step:** continue the user-facing desktop journey
review, then package the agent-instruction correction with a meaningful UI
improvement. Verify the native protocol fixture on that candidate; decide
whether one additional real decision turn is justified before release rather
than repeatedly spending a full Start Forge activation on this narrow check.

### Automatic project-record readback after a conversation turn — 2026-09-27

**Goal/phase:** make the folder-to-conversation journey easier without adding
another source of truth. A finished agent turn now automatically asks Forge for
the project's current record, instead of requiring the person to click
“Atualizar andamento” after every response. The record is invalidated while
the turn runs; completed, interrupted, and failed turns trigger a read-only
refresh. A failed or unavailable Forge read leaves no inferred result and the
manual retry remains available. Disconnection alone does not imply a settled
turn and does not trigger this read.

**Changed, uncommitted:** `ui/chat.mjs` and `ui/progress.mjs` implement the
project-bound readback; `tests/browser.cjs` checks running/terminal states and
the resulting Forge record. The independent agent-command-boundary correction
and native instruction assertion described above are still in the same local
diff. No version bump, installer or publication has been made for either.

**PASS:** controlled browser suite, including the new automatic readback
assertion; all eight frontend unit tests; Desktop debug build; hidden native
WebView smoke (layout, first-use help, draft preservation, appearance and
frontend-to-Rust identity). The native fixture also opened a disposable Forge
project, simulated a terminal agent event, observed a fresh real Rust/Forge
`inspect_progress` call and displayed the unchanged authoritative empty
record. It sent no real Codex turn and did not create fake progress. The first
native attempt failed because the harness clicked a control inside a closed
disclosure; the test was corrected to use its existing `openConversation`
helper, then the full hidden native smoke passed. The earlier Desktop
`cargo check`, focused agent test and all 49 crate tests passed after the Rust
instruction change. The read-only decision request was proven with installed
`forge-core` alone, not through a real app agent. No new provider token counts
were collected; cost and per-task subscription consumption remain UNKNOWN.

**Next exact step:** inspect the integrated diff and prepare one coherent next
alpha candidate. Verify the revised developer instruction in the native Codex
protocol fixture. Do not claim actual agent obedience from that fixture; a
bounded real decision turn remains separately NOT_RUN. Preserve the existing
disposable fixture because recursive cleanup was blocked by host policy; do
not bypass that policy.

### Desktop 0.1.17 alpha candidate — 2026-09-27

**Scope:** one coherent package of automatic Forge record readback after a
terminal conversation event and the separated-tool-call instruction for
temporary Forge JSON. Version/lock/config are 0.1.17; release scope and
limitations are in `RELEASE_NOTES-0.1.17.md`. The larger folder-to-chat-to-
result-to-restart goal remains active.

**PASS:** Desktop `cargo check`, all 49 crate tests, strict Clippy, eight
frontend unit tests, controlled browser suite, pinned-core verification, and
one NSIS release build. The unsigned candidate at
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.17_x64-setup.exe`
is 123,090,502 bytes, SHA-256
`FD53B3133956F15FC4A34E0B4EA521FD67E3D3F902DE551C66BCA0C94AA7138A`.
Silent installation over public 0.1.16 returned exit 0; installed app version
0.1.17, executable SHA-256
`CA2C1657B54A4EA8EB95B0C19E30363AD16BCAD442AC6AFD9A536D84599FEB50`.
Bundled core SHA-256 is the pinned
`CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`;
bundled skill SHA-256 is the pinned
`10581E17D5DBB98BDA3E0F3BC0B6A152736499451E1424E093DBECFAFD8F0B06`.
The installed hidden native WebView passed the disposable-project journey,
including a controlled terminal agent event followed by an actual Rust/Forge
record read. The installed native Codex-protocol fixture captured the revised
instruction and skill path with fake auth completion; it did not log in to a
provider or run a real agent. No manual GitHub CI ran.

**Limits:** app-agent obedience for a typed Decision Request, fresh-account
browser login completion, a clean Windows machine and manual accessibility
remain NOT_RUN. This slice used no new real model turn; task-specific BRL and
Pro allowance costs remain UNKNOWN. The fixture retained under `C:/ForgeFast`
remains because host policy blocked recursive cleanup. The candidate is only
installed locally, not yet committed/pushed/published.

**Next exact step:** inspect final staged diff and selectively commit/push
0.1.17; under the maintainer's standing publication authorization, publish
only the byte-exact tested installer, then verify an unauthenticated download
hash and reinstall those downloaded bytes. Do not rerun NSIS or real Start
Forge activation merely to produce a second candidate or token sample.

### Desktop 0.1.17 public release readback — 2026-09-27

**PASS:** commit `893ed600ffeb27fc1de9eef058745396835f0ce9` was pushed.
Annotated tag `desktop-v0.1.17-alpha.1` resolves to that commit and was
pushed. The public prerelease is
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.17-alpha.1`.
GitHub reports one installer asset, 123,090,502 bytes, digest SHA-256
`FD53B3133956F15FC4A34E0B4EA521FD67E3D3F902DE551C66BCA0C94AA7138A`.
An unauthenticated direct download to
`C:/ForgeFast/forge-alpha-0117-public-20260927/Forge_0.1.17_x64-setup.exe`
matched the exact tested candidate. Silent reinstall of those downloaded
bytes returned exit 0; installed app version 0.1.17, executable SHA-256
`CA2C1657B54A4EA8EB95B0C19E30363AD16BCAD442AC6AFD9A536D84599FEB50`,
bundled core and skill hashes all matched the tested installation. A hidden
native fixture-login/first-send regression on the downloaded installation
again confirmed the new developer instruction and installed skill path/hash.
It did not complete provider login or run a real agent. No manual GitHub CI
was started.

The installer is now available publicly. The unsigned/no-auto-update,
real app-agent Decision Request, fresh-account/clean-machine and manual
accessibility limits in the release notes still apply. The disposable Forge
probe directory remains because recursive cleanup was rejected by host
policy. The broader goal remains active. **Next exact step:** review the
remaining user-facing journey gaps with native evidence. Prioritize a bounded
real typed Decision Request in the app agent, then fresh-account/clean-machine
first use when a suitable isolated fixture exists. Do not let this narrow
validation displace core UI completion or create a second authority store.

### Pending-question navigation review — 2026-09-27

**Goal/phase:** continue the full nontechnical desktop journey after public
0.1.17, rather than treating release verification as product completion.
Read-only inspection of the live Forge root's `workflow resume` returned a
current work focus, an active objective, one candidate question and no
recovered pending decision. The installed 0.1.17 native screen showed the
question below optional direction/history material, so a person could miss
it while reading the large technical record. This was an observed UI ordering
problem, not a Forge data problem. The app did not alter the root record.

**Changed, uncommitted, not installed:** `ui/index.html` puts the questions
before the technical direction/history and adds a visible-on-demand shortcut
near the record state. `ui/progress.mjs` shows the shortcut only when Forge
actually returns pending/suggested questions; clicking it focuses the real
question heading without sending or choosing anything. The prepared question
and option drafts explicitly ask the agent for clear Portuguese explanation,
while preserving the original Forge text and saying that no option was chosen.
`ui/styles.css` explicitly hides the shortcut when the HTML `hidden` flag is
set; the first browser run exposed that the general `button` display rule
otherwise overrode the hidden state. `tests/browser.cjs` and `tests/native.cjs`
cover order, focus, absent/error states and non-sending behavior.

**PASS:** browser UI suite; Desktop debug build; hidden native WebView smoke
against the actual Forge root with a real nonempty candidate question and
accepted direction. Native readback rendered four genuine direction revisions
on demand in a separate read-only run. The new shortcut focused the original
question section; discussing a real option only prepared a Portuguese draft,
did not send a turn, and the fixture restored its earlier draft. Native empty
and file-preview paths also passed. These are source/debug and controlled
event tests, **not** proof that the public 0.1.17 contains the changes or that
a real agent answered the question. No new provider tokens were consumed;
task-specific BRL and Pro costs remain UNKNOWN.

**Next exact step:** continue a purposeful UI review of the real conversation
and result states against the approved visuals, then group any concrete fixes
with this navigation change into a coherent next alpha. Before packaging,
verify the native real-agent decision-request path only if its likely value
justifies another potentially expensive Start Forge activation; do not make
that narrow test a substitute for user-facing UI completion. Keep this goal
active and preserve the current five-file source/test diff plus this checkpoint.

The native Explore capture from the debug build was also inspected against
`design/references/explore-approved.png`. Its eight illustrated categories,
search, header art and open-idea action retain the approved visual language;
no concrete Explore change was justified in this slice. Local screenshot:
`C:/ForgeFast/forge-visual-audit-0117/explore-current.png`. A further browser
test caught and fixed a real hidden-state regression in the new shortcut:
the general button CSS initially displayed it even with `hidden`. The final
browser suite and a new hidden native run against the real nonempty question
passed after `#record-questions-shortcut[hidden]` was made explicit. The native
run also prepared a Portuguese explanation draft from a real English Forge
candidate without sending or selecting anything. The source diff now includes
six files plus this checkpoint. **Next exact step:** review a representative
conversation-with-result composition against the approved conversation art,
then decide whether this UI package needs another concrete fix before a
version bump and installer candidate. Public/installed 0.1.17 remains unchanged.

### Result-preview hierarchy follow-up — 2026-09-27

The prior real-conversation/local-result capture and current installed 0.1.17
site-preview capture were compared with the approved conversation art. A
concrete density issue remained: the long site-safety paragraph sat between
the result and its change action. The source UI now keeps a visible,
keyboard-operable “Prévia protegida: o site não é interativo” disclosure, with
the longer restrictions available on expansion. It resets closed when a new
file/project is selected. The protection itself is unchanged: native preview
still blocks scripts, external requests and interaction. This is not a claim
that the browser fixture reproduces a real agent conversation.

**PASS:** browser UI suite (including disclosure, enlarged-mobile-text and
accessibility checks); Desktop debug build; hidden native WebView smoke against
the real Forge root and disposable local HTML, including opening the disclosure
and checking the isolated preview. A first browser run caught the new note's
too-small mobile text; it was corrected before the passing rerun. Screenshot:
`C:/ForgeFast/forge-visual-audit-0117/preview-safety-expanded.png`.
`git diff --check` passed. No Rust source or backend behavior changed, so
workspace-wide Rust tests/CI were not run. No new provider turn or measured
token cost occurred; BRL/Pro task attribution is UNKNOWN.

**State:** seven modified files remain uncommitted and unpublished on
`codex/desktop-shell`; public/installed 0.1.17 does not contain these UI
changes. No subagents are active. **Next exact step:** inspect the complete
seven-file diff and test evidence, then decide if the pending-question and
result-preview changes form the next alpha package; if yes, bump the desktop
version, prepare/review notes, run package-boundary checks and test one
installer candidate before publication. The broader end-to-end objective
remains active; a real app-agent decision request, fresh-account login,
clean-machine install and manual accessibility are still NOT_RUN.

### Desktop 0.1.18 alpha candidate — 2026-09-27

The pending-question navigation and protected-result-preview disclosure are
grouped as a single user-facing alpha package. Desktop version/lock/config
are now 0.1.18; `RELEASE_NOTES-0.1.18.md` describes scope and limits. The
full nontechnical folder-to-chat-to-result-to-restart goal remains active.

**PASS:** browser UI suite, eight frontend unit tests, Desktop `cargo check`,
all 49 Desktop crate tests, strict Clippy and `git diff --check`. One NSIS
release build produced the candidate at
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.18_x64-setup.exe`,
123,069,091 bytes, SHA-256
`5BF97B83BEF0D96D38F70D532D76E9AB8FFEA4F5ED7D69292AF0AFF2AFE5001C`.
Its silent installation over public 0.1.17 returned exit 0. Installed app
version is 0.1.18 with executable SHA-256
`305CC60D29E0B393D01CAB3BA5D4513CB81DA3784AE0635E24A5F704D879B88C`.
Installed core and skill retain pinned SHA-256
`CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`
and `10581E17D5DBB98BDA3E0F3BC0B6A152736499451E1424E093DBECFAFD8F0B06`.
Installed hidden native WebView smoke passed with real Forge root readback and
disposable preview, without an actual Codex send. The candidate hash remained
unchanged after installation. No manual GitHub CI ran.

**NOT_RUN:** real conversation continuity across 0.1.17 to 0.1.18, real typed
decision turn, fresh-account login, clean-machine install and manual
accessibility. Model/API-equivalent BRL and Pro quota attributable to this
package remain UNKNOWN. No active subagents. **Next exact step:** review the
complete scoped diff, selectively commit/push the 0.1.18 package, and under
the maintainer's standing release authorization publish only this tested
installer. Verify an unauthenticated download size/hash and reinstall those
downloaded bytes. Never rebuild after hashing merely to publish.

### Desktop 0.1.18 public release readback — 2026-09-27

**PASS:** source commit `a8933d178433c1e241a6c48fe9c1500969a9a901`
was pushed. Annotated tag `desktop-v0.1.18-alpha.1` resolves to that commit
and was pushed. The public prerelease is
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.18-alpha.1`.
GitHub reports one installer, 123,069,091 bytes, digest SHA-256
`5BF97B83BEF0D96D38F70D532D76E9AB8FFEA4F5ED7D69292AF0AFF2AFE5001C`,
plus its checksum sidecar. An unauthenticated direct download to
`C:/ForgeFast/forge-alpha-0118-public-20260927/Forge_0.1.18_x64-setup.exe`
matched the exact tested candidate size/hash. Silent reinstall of those
downloaded bytes returned exit 0; installed app version 0.1.18, executable
SHA-256 `305CC60D29E0B393D01CAB3BA5D4513CB81DA3784AE0635E24A5F704D879B88C`,
and bundled core SHA-256 matched the tested installation. A hidden native
smoke on the downloaded installation again passed the disposable-project,
real Forge-record readback and local preview checks. No manual GitHub CI ran.

The release is publicly available, but a real Codex conversation persisted
*across this specific upgrade* was NOT_RUN. The typed app-agent decision turn,
fresh-account/clean-machine and manual accessibility limits in the release
notes still apply. No new provider-model turn was run for this package;
task-specific token/BRL and Pro allowance attribution remain UNKNOWN.
The broader nontechnical end-to-end goal remains active. **Next exact step:**
use an existing disposable Codex-owned conversation/profile, if one is still
intact, to verify real installed 0.1.18 resume and project-result continuity
without creating a new expensive agent turn. If no safe fixture exists, record
that limit rather than fabricating one, then continue the remaining UI
coverage and first-use path.

### Installed 0.1.18 existing conversation readback — 2026-09-27

**PASS:** a pre-existing disposable Codex-owned thread from the earlier
0.1.15-to-0.1.16 upgrade fixture was still intact in
`C:/ForgeFast/forge-upgrade-015-to-016-20260927-project` and its preserved
WebView profile. The installed, publicly downloaded 0.1.18 app opened that
same project on a hidden Windows desktop, resumed the actual user/reply pair,
confirmed both messages and their marker, waited for stability, and verified
that no additional turn was created. The probe sent nothing and consumed no
new provider-model tokens. This proves read-only continuity from that older
fixture into 0.1.18; it does **not** prove a turn persisted specifically
through the 0.1.17-to-0.1.18 installer transition or a fresh first-use login.
No project files were intentionally edited.

**Next exact step:** inspect a preserved real project-result fixture for a
read-only installed 0.1.18 resume/preview check if it still exists. If not,
move to the remaining first-use/decision-flow UI rather than sending an
expensive new agent turn solely to recreate a fixture. The goal stays active.

### Installed 0.1.18 real result after reopen — 2026-09-27

A preserved disposable project/profile from an earlier real Codex artifact
journey was found at
`D:/Temp/User/forge-desktop-webview-ENrGgT/new-project` (profile is its parent).
Its Codex-owned thread already contained a completed relative link to the
locally generated `site/index.html`. The test-only `artifact-resume.cjs` now
accepts `FORGE_ARTIFACT_READ_ONLY=1`: if that link is absent it fails instead
of asking the model to repair the conversation.

**PASS:** on the installed, publicly downloaded 0.1.18 app, hidden native
Windows runs twice in separate processes resumed the same real conversation,
opened its linked HTML through the project-bound isolated preview, showed the
honest unpublished origin and prepared **Pedir mudança neste arquivo** in the
same unsent composer. No new turn was sent; the message count did not change.
The HTML SHA-256 before/after the first run remained
`E118B0C85EC32EDB763CBE9607919407DB15AA4577FBBFFD4759B7AC8420C907`.
This is genuine persisted-chat/result continuity on 0.1.18, not a controlled
agent fixture. It still does not prove a fresh 0.1.17-to-0.1.18 real-turn
upgrade, fresh login, or that a person accepts the full UI. No model tokens
were consumed by these read-only runs; monetary task cost remains UNKNOWN.

**Next exact step:** complete the remaining real first-use/decision path only
with a bounded fixture and cost justification; otherwise prioritize another
concrete UX gap found in native use. Keep packaging separate from tiny test
changes, and do not rerun the expensive Start Forge activation without cause.

### Real-result empty-record clarity and bundled-core handoff — 2026-09-27

The publicly installed 0.1.18 app was resumed headlessly on the preserved
real Codex conversation and generated `site/index.html`. Its screenshot showed
a genuine local result and chat alongside an absent Forge record, but the UI
suggested no work existed and displayed a discovery-stage panel. This is a
presentation error, not evidence that the result or conversation was lost.
The source UI now says precisely that the Forge *steps* are absent, preserves
the visible conversation/files, and hides the unrecorded stage. No new state
store or fabricated project data was added. A read-only native screenshot of
the corrected source build is
`C:/ForgeFast/forge-empty-record-with-artifact-no-fake-stage.png`.

A separate read-only inspection of a prior real first-use Codex thread found
that the agent used a globally installed `forge-core.exe` rather than the
app-bundled runtime. The app already resolved and exposed its runtime to the
Codex process, but this observation shows that environment preparation alone
did not guarantee the agent's chosen executable. The source now puts the exact
resolved executable path in the agent's developer instructions and forbids
silently switching to PATH/Cargo/global/WSL copies. This is a guidance fix,
not proof that a fresh real Codex turn will obey it or that a clean machine is
yet self-sufficient.

**PASS:** browser UI suite; corrected-source hidden native read-only real
artifact/Forge-record composition; corrected-source hidden generic native
smoke; scoped Desktop debug build; Desktop `cargo check -p forge-desktop`;
focused `agent_prefers_bundled_start_forge_guidance` (1 test executed); all
49 Desktop crate tests; `node --check` for the new opt-in native auth assertion;
`git diff --check`. The current source edits are
uncommitted/unpublished, and public/installed 0.1.18 does not include them.
The repo's pre-existing Rust formatting differences mean `cargo fmt --check`
is not clean; only the changed Rust lines were formatted locally, without
reformatting unrelated modules. No new provider turn was sent for this slice;
its task-specific model token cost is zero for the read-only probes, while
API-equivalent BRL and Pro allowance attribution remain UNKNOWN. No active
subagents.

**NOT_RUN:** real Codex first-use with the new explicit bundled-core path,
fresh account/clean-machine install, actual app-agent decision turn, manual
accessibility, and a new installer candidate. **Next exact step:** inspect the
complete diff and verify the native protocol's `developerInstructions` carry
the same bundled runtime path; use a bounded fixture before deciding whether
a new real Codex turn is worth its cost. Then group the verified UI and
first-use fixes into the next alpha package instead of publishing each tiny
edit. The wider nontechnical end-to-end goal remains active.

### Bundled-core instruction readback in a native protocol fixture — 2026-09-27

The current Desktop debug executable was rebuilt. For a **headless native
fixture only**, the already-installed, hash-pinned Forge core and Start Forge
skill were staged beside that executable in the build cache, with no
`FORGE_CORE_EXE` override. The fake Codex device-login/server fixture captured
the real `thread/start` request from the native app. Its developer
instructions named an existing executable resolving to the very same bundled
core next to the pinned skill, rather than a global Forge installation.
The assertion initially compared two equivalent Windows paths literally and
failed because Tauri supplied the skill with a `\\?\` prefix; the test was
corrected to compare canonical paths, then passed. This was a test-path
normalization issue, not an app runtime failure.

**PASS:** rebuilt Desktop debug binary; staged core/skill hashes
`CFD6F81B...7541EDF` and `10581E17...8F0B06`; headless native login/send
protocol fixture with the new bundled-path assertion. The test sent no
provider request: Codex login and server responses were simulated. Source is
still uncommitted/unpublished; the installed public app is unchanged.
**NOT_RUN:** actual model execution of the instruction, fresh-account or
clean-machine use. **Next exact step:** review and close this coherent source
slice, then prepare the next alpha package with release-boundary checks and
one candidate installer. No new provider tokens were consumed by the fixture;
monetary/Pro attribution remains UNKNOWN.

### Desktop 0.1.19 alpha candidate — 2026-09-27

The concrete real-result empty-record UI correction and explicit bundled-core
agent handoff are grouped as the next alpha package. Desktop manifest, lock
and Tauri config are 0.1.19; `RELEASE_NOTES-0.1.19.md` describes the scope and
limits. The broader nontechnical journey remains active.

**PASS:** eight frontend unit tests; browser UI suite; Desktop `cargo check`,
all 49 Desktop crate tests and strict Clippy; exact pinned core and skill
staging; one NSIS release build. The single candidate at
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.19_x64-setup.exe`
is 123,086,775 bytes, SHA-256
`E96CC0BCE5664DD56140F48CE8FDB0831A65EDE9F8D17ACAD36FEAAD0FEC44A9`.
Silent installation over public 0.1.18 returned exit 0. Installed app version
is 0.1.19, executable SHA-256
`52C36C1CBDEC97CB03C297088D1CCAF4EAA83DE45C2464B4008BB291B8BBFEE1`.
Installed bundled core/skill hashes are the pinned 0.13.2 values. The
candidate hash was unchanged after installation. Installed hidden native
smoke passed real Forge project onboarding, preview and record readback;
installed hidden fake-login protocol captured the exact bundled executable
in the agent instructions; installed hidden read-only actual Codex conversation
resumed its linked HTML result and prepared a change request without sending.
No manual GitHub CI was run. Current source is not yet committed or published.

**NOT_RUN:** a new real Codex turn obeying the bundled-path instruction,
fresh-account login, clean-machine install, manual accessibility, and a real
agent decision turn. No new provider turn was run for this package; task BRL
and Pro allowance attribution are UNKNOWN. No active subagents. **Next exact
step:** inspect the complete scoped diff, selectively commit/push the 0.1.19
package, publish only this tested candidate under the standing maintainer
authorization, then verify its unauthenticated downloaded bytes and install
that exact download. Do not rebuild the candidate merely to publish it.

### Desktop 0.1.19 public release readback — 2026-09-27

Source commit `32090e5c4a1eef16b29caab6e243b3109f8e0837` was pushed.
Annotated tag `desktop-v0.1.19-alpha.1` resolves to that commit and was
pushed. The public prerelease is
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.19-alpha.1`.
GitHub reports one installer, 123,086,775 bytes, SHA-256
`E96CC0BCE5664DD56140F48CE8FDB0831A65EDE9F8D17ACAD36FEAAD0FEC44A9`,
plus its checksum sidecar. An unauthenticated direct download to
`C:/ForgeFast/forge-alpha-0119-public-20260927/Forge_0.1.19_x64-setup.exe`
matched the exact tested candidate size/hash. Silent reinstallation of those
downloaded bytes returned exit 0; installed app version and executable/core
hashes matched the tested candidate. Hidden native WebView smoke on that
downloaded installation passed real Forge record/project readback, onboarding
and isolated local preview. No manual GitHub CI was run.

This proves publication and installed-byte continuity, not a fresh real Codex
turn obeying the new bundled-path guidance, fresh-account/clean-machine setup,
manual accessibility or a real agent decision. No new provider-model turn was
run for this package; task-specific BRL and Pro allowance attribution remain
UNKNOWN. The full nontechnical end-to-end goal stays active. **Next exact
step:** use the now-installed 0.1.19 app to examine a bounded first-use or
decision-turn path without repeating the expensive full Start Forge activation
solely for a fixture; if a safe real-agent proof is not cost-justified, work
on the next concrete UX/integration gap found in native use.

### Real same-chat change and restart on installed 0.1.19 — 2026-09-27

The preserved disposable `D:/Temp/User/forge-desktop-webview-ENrGgT/new-project`
fixture was used for one bounded real Codex turn in the publicly downloaded
and installed 0.1.19 app. Its existing conversation already linked the genuine
`site/index.html`. A new one-shot headless native test
(`tests/native-real-change.cjs`) opened that file, used **Pedir mudança neste
arquivo**, sent exactly one user request to change only the visible heading to
“Jardim de ideias renovado”, and waited for the real reply. It did not retry
after a timeout. The app's protected preview refreshed to the new heading,
and a fresh app process resumed the expanded conversation and reopened the
changed file without replaying the turn. The HTML SHA-256 changed from
`E118B0C85EC32EDB763CBE9607919407DB15AA4577FBBFFD4759B7AC8420C907`
to `B4B8D2C561376990E38D273972530AF31CA42C43022751AE302F3B0EED784931`;
CSS SHA-256 remained
`707213DE71BE4FF0797855EB3F15B78A3D779550B715DDA7BC376327C0099B4D`.
The real agent's tool trace shows only read/patch/check of the intended HTML.

The preserved Codex thread is `01a0e25f-27b4-7f73-b3e8-9298bccdde2f`,
turn `01a0e47e-ab41-7cc1-85e8-a463f2b82426`, from 20:11:41 to
20:12:45 UTC (64 seconds). Its final *per-turn* usage is 123,614 input
tokens **including** 98,816 cached input (24,798 uncached), and 871 output
tokens **including** 304 reasoning output; total 124,485. The observed model
was `gpt-6-sol`, high effort. No subagents, reroutes or parent repair turns
were used. The account's shared weekly used-percent snapshot was 47% before
and 47% after, too coarse for per-turn Pro attribution. An API-equivalent BRL
estimate is UNKNOWN without an established applicable tariff and dated FX;
this is not a claim of zero monetary/subscription cost.

A read-only installed-core `workflow resume` after the edit still returned
`current_work.status=absent`. The app correctly does not invent a Forge Work
Focus from the Codex chat or file. This real result therefore proves the
folder/chat/result/change/restart path, **not** that the agent has activated
or advanced Forge governance for this task. The turn did not invoke Forge core,
so it also does not prove obedience to the new bundled-runtime path. The
earlier history has an extra agent segment, so the one-shot test counts actual
user/agent items rather than assuming alternating pairs; its first preflight
failed without sending, then the corrected preflight passed.

Visual inspection of the installed real-result screenshot found a small but
concrete copy error: the resumed-chat banner told the person to inspect the
last Forge record even though this project has no recorded steps. Source UI
now says “Conversa retomada. Você pode continuar de onde parou.” Browser UI
suite and a rebuilt debug app's headless native read-only real-result resume
passed. Corrected-source screenshot:
`C:/ForgeFast/forge-current-resume-copy-real-artifact.png`. No Rust source
changed and no broad Rust suite or new installer was run for this copy/test
slice. This UI copy is not in public 0.1.19. No active subagents.

**Next exact step:** inspect and commit the focused test/UI-copy diff, leaving
the goal active; then investigate a meaningful first-use Forge-governance
path with a cost-bounded real turn only if needed, or a concrete native UX gap.
Do not repeat this one-shot edit against the already-changed fixture.

### Installed 0.1.19 real first-use Forge activation — 2026-09-27

A second bounded real-agent probe used the **installed public 0.1.19** binary,
headless Windows desktop and a newly selected disposable project. The person
asked for a small recipe site but had not chosen its audience, and explicitly
asked the agent to define the first step before writing files. The app opened
one Codex conversation, delivered one answer, then restored the same user/reply
after WebView reload without resending. The agent oriented the project in
plain Portuguese and asked whether the first version is for personal use,
family/friends or the public; it recommended personal use. It did not write,
publish or install anything. The test profile/project were removed afterward.

The preserved Codex thread is `01a0e485-cd5c-7873-9082-bb0c93da6bfb`.
Its tool trace **confirms the actual bundled executable path**
`C:/Users/User/AppData/Local/Forge/forge-core/forge-core.exe`, not PATH/Cargo,
global installer or WSL. That binary returned `forge-core 0.13.2`. The agent
read the packaged Start Forge guidance, invoked that exact executable for
`start`, followed the published `workflow resume` handoff, and used it for
`guide status` in Discovery. The authoritative resume reported
`current_work.status=absent` and `current_phase=1-discovery`; the agent asked
for the missing product choice rather than inventing a Work Focus. This is
real bundled-runtime compliance **for this one turn**, not a guarantee for
every future agent or a clean-account/clean-machine proof.

**PASS:** installed native UI/Forge onboarding, real Codex first reply,
bundle-path/version tool trace, truthful Discovery orientation and WebView
reload continuity. The turn ran 20:19:30–20:20:54 UTC (84 seconds), observed
`gpt-6-sol` with high effort, no subagents or retries. Final per-turn usage:
386,159 input tokens **including** 341,760 cached (44,399 uncached); 2,817
output tokens **including** 1,051 reasoning; total 388,976. The packaged
Start Forge skill is ~51.7 KB/833 lines and required multiple bounded reads;
this is a measurable first-use context cost, not yet proof that a shortcut or
skill split would save Pro allowance without quality loss. Shared weekly Pro
used-percent was 47% before and after (rounded and shared). API-equivalent
BRL remains UNKNOWN without established applicable tariff/FX, and Pro
per-turn consumption cannot be inferred from those snapshots. No manual GH CI.

**NOT_RUN:** a real accepted Work Focus/decision turn, fresh ChatGPT device
login, clean-machine install or manual accessibility acceptance. **Next exact
step:** use this evidence to choose the next concrete gap: exercise an
agent-mediated product decision through the app and verify Forge record
readback, rather than adding a second state store or repeating a generic
activation probe. Keep the full desktop goal active.

### Real product direction, interrupted Work Focus, and clearer record — 2026-09-27

The existing disposable project `D:/Temp/User/forge-desktop-webview-ENrGgT/new-project`
and its real Codex conversation `01a0e25f-27b4-7f73-b3e8-9298bccdde2f`
received **one** product-decision message through the installed public 0.1.19
app, on a hidden Windows desktop. The test user explicitly chose personal use,
no account/sharing in the first version, and asked for the direction and next
work to be recorded without editing or publishing. `tests/native-forge-decision.cjs`
is one-shot: it checks the absent starting Work Focus and accepted direction
before Send, and it never retries an uncertain or timed-out send.

The agent invoked the installed bundled `forge-core 0.13.2`, successfully
accepted an initial **product direction** in Forge, and did not change the HTML
or CSS. Authoritative `workflow resume` readback shows the accepted objective
outcome “Evoluir o Jardim de ideias para uma primeira versão de uso pessoal,
na qual uma pessoa possa anotar e organizar as próprias ideias”, plus the
agreed constraints and open uncertainties. Its `current_work.status` remains
`absent`: the agent had **not** recorded the next Work Focus. The native test's
360-second reply wait expired, closing the app and aborting that agent turn.
There is no final reply and no proof of a fully completed decision-to-work
journey. Do not repeat the same request blindly. The temp input file
`D:/Temp/User/forge-objective-input.json` was left after the agent's cleanup
command was blocked by host policy; it contains this synthetic test proposal,
not a project file. The pre-existing project files were unchanged.

**PASS:** separate read-only headless native checks against the installed
binary and a rebuilt debug binary showed the accepted direction from Forge in
the app, with no invented Work Focus and no send. The real state exposed a
concrete UX ambiguity: the old UI said “Sem etapas no Forge” while also showing
an accepted direction. The source UI now distinguishes “Direção registrada;
próximo trabalho pendente”, explains what is missing, and retains “Sem etapas
no Forge” when *no* direction exists. The browser UI suite passed; incremental
offline desktop debug build passed; the rebuilt debug app's native read-only
check passed. No Rust source changed, so no workspace Rust suite or package
build was run. This wording is **not** in the installed/public 0.1.19.

The interrupted turn was `01a0e48c-1690-70e1-b18f-f11846e72119`
(20:26:24–20:32:21 UTC), observed `gpt-6-sol` high, no subagents or retry.
Its final *per-turn* usage was 575,849 input tokens **including** 540,672
cached input (35,177 uncached), and 5,570 output **including** 2,701 reasoning;
581,419 total. It made 12 tool calls; two attempts around preparing/cleaning
the temporary proposal were rejected by host policy, and `--help` was not
supported by the core subcommand. The shared Pro weekly used-percent was 47%
before and after (rounded and shared), so per-task subscription consumption
and an API-equivalent BRL amount remain UNKNOWN, not zero. This costly,
interrupted path is a performance/operability gap, not evidence that the UI
sent twice or that Forge lost the accepted direction.

**Next exact step:** inspect the incomplete turn and the existing accepted
objective before any continuation; then complete *one* bounded Work Focus
through the same app/conversation without resending the accepted decision, and
verify Forge and UI readback after restart. If the host path repeatedly stalls,
address the specific friction rather than adding a second state store. Keep
the desktop goal active. No active subagents.

### Accepted Work Focus and easy return to an earlier result — 2026-09-27

The interrupted test conversation was continued **once**, in the installed
public 0.1.19 app, with a new message saying the personal-use direction was
already recorded. The preflight read Forge's accepted objective and absent
Work Focus; it did not resend or supersede the accepted direction. The real
agent accepted `focus.jardim-ideias-v1-pessoal-planejamento` through the
installed bundled `forge-core 0.13.2`, then replied in ordinary Portuguese.
Authoritative `workflow resume` returned `current_work.status=current`, title
“Planejar a primeira versão pessoal do Jardim de ideias”, intended outcome for
one person without account/sharing, and a next step to review the existing page
and propose a minimum way to record, organize and retain ideas between visits.
The HTML and CSS remained unchanged; nothing was published. The temporary
proposal file from the interrupted turn was removed by the agent after a
successful exact-file cleanup, not by deleting project content.

**PASS:** installed native UI showed exactly that Forge title, outcome and next
step. A fresh native process then reopened the same Codex conversation and
read the same accepted direction and Work Focus, with an instrumented zero
`send_message` calls and unchanged project-file hashes. The agent's continuation
ran 20:39:41–20:43:50 UTC (249 seconds), observed `gpt-6-sol` high, 13 tool
calls, no retry/subagents. Its final *per-turn* usage was 1,030,485 input
tokens **including** 1,012,096 cached (18,389 uncached), and 6,273 output
**including** 3,508 reasoning; 1,036,758 total. Combined with the preceding
interrupted attempt, the two-turn decision-to-Work-Focus path cost 1,618,177
observed tokens, but that is **not** a controlled comparison with Astra or
another workflow. Shared Pro weekly used-percent stayed 47% before and after
this turn, too coarse/shared for per-task subscription attribution. BRL
API-equivalent remains UNKNOWN without a dated applicable tariff and FX.

Visual native review after restart found another concrete friction: the latest
planning-only reply hid the earlier file-result shortcut, leaving an empty
preview despite a real page in the same conversation. `ui/chat.mjs` now derives
the **last cited local result** from completed agent messages in the current
Codex transcript, even if a later reply contains no file; it does not create
another persisted state store or open a file automatically. The native
project-bound `inspect_preview` still validates any clicked path. Browser UI
tests passed for a restored file result followed by a planning reply; rebuilt
debug app passed headless native restart, zero-send, Forge-record, previous-file
shortcut and actual HTML preview checks. Visual screenshot:
`C:/ForgeFast/forge-real-work-focus-and-result.png`. This UI change, like the
clearer absent-record wording above, is **not** in installed/public 0.1.19.
No Rust source changed; no workspace-wide suite or installer was built.

The preserved conversation began under an older desktop instruction that said
to use the installed Start Forge skill. In this continuation the agent read the
global skill, despite current 0.1.19 source instructing new conversations to
use the packaged skill; it still used the exact bundled core binary. The fresh
0.1.19 first-use thread above did use the packaged guidance. Thus this test
proves state and UI continuity across versions, not that legacy conversations
inherit new skill-path instructions.

**Next exact step:** review the focused local diff and remaining native UX
states, then prepare one coherent desktop alpha package with these visual
continuity fixes. Before publication, run the appropriate final package gates
and verify the candidate installed over the previous version; do not treat this
debug build or screenshot as a released installer. Keep the full desktop goal
active. No active subagents, no commit/push or release in this slice.

### Desktop 0.1.20 alpha candidate and installed upgrade — 2026-09-27

The scoped continuity fixes above are packaged as Desktop 0.1.20, without
changing the separate Forge core. During review, restoration of a long chat
was improved: the last-file shortcut is recomputed once after the transcript
is loaded, not for every restored message. A controlled browser case restored
202 messages in order and retained the earlier file shortcut. This is a
performance precaution, not a measured latency claim. The release scope and
limits are in `RELEASE_NOTES-0.1.20.md`.

**PASS:** browser UI suite; eight frontend unit tests; Desktop `cargo check`,
all 49 Desktop crate tests, strict Clippy, `git diff --check`, pinned core
staging, and one NSIS release build. The candidate at
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.20_x64-setup.exe`
is 123,075,607 bytes, SHA-256
`282125B2A99E339233A3678602829FC5D6311E2D4A1F62F636C2FCD8FAF9AFE7`.
Silent installation over public 0.1.19 returned exit 0. The installed app
reports version 0.1.20, executable SHA-256
`11651B7F81A6BF96FE2948FAF689B5CD918CC37B8439408C659B511DE2E3350B`.
The installed bundled core retains pinned SHA-256
`CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`.
The candidate hash was unchanged after installation.

Headless native read-only verification of the **installed** 0.1.20 app
reopened the pre-upgrade real Codex conversation, read the exact accepted
Forge direction and current Work Focus, and opened its earlier local HTML
result despite a later planning reply. Instrumented `send_message` count was
zero; project HTML/CSS hashes were unchanged. Installed hidden native WebView
smoke also passed navigation, formatting, controlled long-history scrolling,
appearance reload and frontend-to-Rust identity. Its first-use path was a
controlled fixture and did not resolve a real project; the separate real
conversation readback did use a real linked project. No manual GH CI or new
provider-model turn was run.

**NOT_RUN:** clean-machine install, fresh ChatGPT account/login, manual
accessibility acceptance, mobile, or an additional real agent turn under the
new version. No controlled Astra/Sol cost comparison was performed. Task BRL
and Pro per-turn consumption are UNKNOWN; this package consumed no new
provider-model turn. No active subagents. Source changes, candidate, and
release notes are still local at this checkpoint, not committed/pushed or
publicly available. **Next exact step:** review/selectively commit and push
the 0.1.20 package, publish only the tested candidate under maintainer
authorization, then download unauthenticated and verify the exact bytes and
installed build. Keep the full nontechnical journey goal active.

### Desktop 0.1.20 public release readback — 2026-09-27

Source commit `fd5e1acd63756744c772b99c5b5d144834217014` was pushed. Annotated
tag `desktop-v0.1.20-alpha.1` points to that commit and was pushed. The public
prerelease is
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.20-alpha.1`.
GitHub reports one installer, 123,075,607 bytes, and its checksum sidecar.
An unauthenticated direct download to
`C:/ForgeFast/forge-alpha-0120-public-20260927/Forge_0.1.20_x64-setup.exe`
matched the exact tested candidate SHA-256
`282125B2A99E339233A3678602829FC5D6311E2D4A1F62F636C2FCD8FAF9AFE7`.
Silent reinstallation of **those downloaded bytes** returned exit 0;
installed 0.1.20 executable and bundled core hashes matched the candidate
readback above. A hidden native read-only restart against that downloaded
installation again passed real Codex conversation, exact Forge direction/Work
Focus and earlier HTML result, with zero sends and unchanged project files.
No manual GitHub CI or new provider-model turn was run.

This proves public availability and installed-byte continuity, not
clean-machine/fresh-account setup, mobile, accessibility acceptance or a
complete autonomous product journey. Cost per real agent turn remains
UNKNOWN for this package because there was no new turn. **Next exact step:**
leave the released 0.1.20 installed and use the app to progress one real
project from the accepted Work Focus toward a visible implemented result,
avoiding another broad activation or duplicate product decision. Find and
fix only a concrete blocker exposed by that journey. The full desktop goal
remains active. No active subagents.

### Interrupted real implementation and earlier-result recovery — 2026-09-27

The installed 0.1.20 app sent one controlled request in the existing disposable
`D:/Temp/User/forge-desktop-webview-ENrGgT/new-project` conversation. It chose a
personal, no-account/no-sharing ideas list stored only in that browser/device,
authorized changes only in this disposable folder, and explicitly required an
ordinary JavaScript-enabled browser check because Forge's protected preview
does not run scripts. The agent accepted the new Forge Work Focus
`focus.jardim-ideias-v1-funcional`, but after 600 seconds it had not edited the
page or returned a final response; the one-shot test closed the native process.
The prompt was not resent. A **different** continuation message, after a
pre-send fixture correction, was sent once at 21:31:20 UTC. It instructed the
agent to continue from the accepted Work Focus without repeating the decision.
The agent changed `site/index.html`, `site/assets/site.css`, and added
`site/assets/site.js`; after another 600 seconds without a final response, that
turn was also interrupted. Do not resend either message blindly.

**PASS, bounded evidence:** `node --check` of the generated JS; an independent
isolated Chromium run added, listed, reloaded, removed and reloaded an idea from
the local HTML with no page errors. The installed native app re-opened the real
conversation and exact Forge Work Focus without sending or changing files. The
rebuilt debug app rendered the new page in Forge's protected preview; the
preview itself remains noninteractive by design. **NOT_RUN:** agent final reply, agent-recorded evidence
or Work Focus completion, clean-machine setup, and a provider-native acceptance
test beyond this controlled fixture. The authoritative Work Focus still says
`current` with zero evidence; its next-step text is stale relative to files
changed just before the interruption. Do not present this as a completed Forge
cycle or a public user-facing project.

This journey exposed a reproducible UI gap in **installed** 0.1.20: after a
fresh restart with an incomplete latest agent reply and no preview already
open, the earlier completed file-result shortcut is hidden. `ui/chat.mjs` now
finds the last completed cited local file even when the latest reply is
incomplete and avoids recomputing it on every streaming token. Controlled
browser UI suite, debug native full-process interrupted-reply fixture, and
debug native read-only real-conversation restart all passed. The installed
0.1.20 real fixture failed the closed-preview shortcut assertion; the rebuilt
debug app passed it, read the exact Forge direction/Work Focus, and showed the
new HTML preview with zero sends. `git diff --check` passed. No Rust source was
changed. This fix is local **only**, not in the installed/public release.
Visual debug screenshot: `C:/ForgeFast/forge-personal-ideas-native-20260927.png`.

The initial real turn observed `gpt-6-sol` high and 1,006,578 input tokens
including 901,504 cached, plus 6,926 output including 4,329 reasoning
(1,013,504 total). The continuation observed 1,197,332 input including
1,160,704 cached, plus 10,814 output including 3,480 reasoning (1,208,146
total). These are separate per-turn provider counters, not summed cumulative
snapshots; combined observed total is 2,221,650 tokens for two interrupted
turns and one partial file implementation. The shared Pro weekly counter was
48% around this work, too coarse/shared to attribute a per-task allowance
change. API-equivalent BRL and subscription cost per turn are **UNKNOWN**;
there is no controlled Astra/Sol comparison. This is not evidence of an
economical routing win. No subagents were used.

**Next exact step:** do not start a third real Codex turn on this fixture
without first reducing the long-thread/turn-cost and completion friction.
Review the local shortcut diff, preserve the existing generated fixture, and
decide a bounded app-side improvement that helps interrupted long conversations
finish or resume without duplicate sends. Then verify native restart and
package a coherent alpha update; do not claim the local fix is installed yet.

### Desktop 0.1.21 interrupted-conversation recovery candidate — 2026-09-27

The installed-app journey identified a second UI gap: after a restart, a real
incomplete Codex reply was labeled merely “Conversa retomada”. The UI now uses
the Codex-owned restored message's `incomplete` flag to say the last answer was
interrupted, advise checking the conversation and project files before asking
for continuation, and state that nothing was resent. A previously unconfirmed
Send retains its stronger existing warning. The earlier-result shortcut fix
above is included in the same alpha package. No new Forge state store or
automatic resend was added.

**PASS:** browser UI suite covers completed versus interrupted restoration and
zero automatic resends; eight frontend unit tests; Desktop `cargo check`, all
49 Desktop crate tests, strict Clippy, `git diff --check`; rebuilt debug app
headless native real-conversation restart and controlled fresh-process
interrupted-reply fixture. The release build generated one NSIS candidate:
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.21_x64-setup.exe`,
123,045,449 bytes, SHA-256
`D98EEAB8215BBA5C64EBDFC8345FC41B67D8293FBD18C42BED2945AC44A41F66`.
Silent installation over 0.1.20 returned exit 0; installed app executable
SHA-256 is `7BE48E9959C3B7DED377B5564B5A3EEE575652E8B723917AFB7DD205C4D4B4CC`,
bundled core remains pinned at
`CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`,
and the candidate's hash was unchanged after installation. The **installed**
0.1.21 app passed hidden native smoke and read-only restart of the exact real
Codex conversation, interrupted warning, prior HTML result and authoritative
Forge direction/Work Focus, with zero sends and unchanged project files.

**NOT_RUN:** a fresh real agent turn under 0.1.21, clean-machine/fresh-account
setup, manual accessibility acceptance, mobile, or a complete Forge Work Focus
closeout. The long-thread token cost remains unsolved. Release notes are in
`RELEASE_NOTES-0.1.21.md`. The installer is currently a **local tested
candidate**, not yet committed, pushed, or publicly available. No manual GH CI
was run and no new provider-model turn was spent on this package. No active
subagents. **Next exact step:** review/selectively commit and push the package,
publish only the tested candidate under the maintainer's standing alpha
authorization, download it unauthenticated, verify exact bytes, and test that
downloaded file after installation. Do not rebuild the candidate merely to
publish it. Keep the full desktop journey goal active.

### Desktop 0.1.21 public release readback — 2026-09-27

Source commit `ef6ef5f4f9ad442ed528ae3a49438be570af298c` was pushed to
`codex/desktop-shell`. Annotated tag `desktop-v0.1.21-alpha.1` points to that
commit and was pushed. The public prerelease is
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.21-alpha.1`.
GitHub reports the installer and checksum sidecar; the installer asset is
123,045,449 bytes. An unauthenticated direct download to
`C:/ForgeFast/forge-alpha-0121-public-20260927/Forge_0.1.21_x64-setup.exe`
matched the exact locally tested candidate SHA-256
`D98EEAB8215BBA5C64EBDFC8345FC41B67D8293FBD18C42BED2945AC44A41F66`.
The downloaded sidecar matched too. Silent installation of **those downloaded
bytes** returned exit 0; installed executable and bundled core hashes matched
the candidate readback above. A hidden native read-only restart of the exact
real Codex conversation again passed interruption warning, earlier HTML
result, Forge direction/Work Focus, zero sends and unchanged project files.
No manual GitHub CI or additional provider-model turn was run.

This confirms public availability and installed-byte continuity of the
interruption recovery, not completion of the agent's interrupted Work Focus or
economic improvement. The two real Sol turns are still costly and lacked a
final response. A read-only trace audit of the continuation shows 11 model
responses/tool calls; per-response input grew from 101,045 to 115,201 tokens,
mostly cached. The agent implemented the page, ran browser checks, and began
the Forge Work Focus update at 21:39:53 UTC. The harness's fixed 600-second
limit interrupted it at 21:41:20 UTC, just after preparing the update input;
this evidence does **not** prove the app or Codex was stalled. The local Codex
0.157.1 app-server schema exposes `thread/compact/start` and token-usage
notifications, but this package does not call either; potential context-cost
benefit, continuity quality, and quota effect remain untested.

**Next exact step:** replace the fixture's crude fixed wait with a bounded
progress-aware observation that does not kill an actively advancing turn;
test that logic without sending a provider turn. Then decide whether a
controlled, same-thread compaction pilot can reduce repeated context cost
without losing the Forge handoff or project facts. Only after that, complete
one real result/change/closeout journey in the installed app. Do not add a
second persistence system. The full desktop goal remains active.

### Desktop real-result closeout and observer checkpoint — 2026-09-27

The disposable native project at
`D:/Temp/User/forge-desktop-webview-ENrGgT/new-project` now has a functional
personal ideas page. A third message in the **same** Codex chat
(`01a0e25f-27b4-7f73-b3e8-9298bccdde2f`) asked only to finish the existing
Forge record and answer with the result; it was sent **once**, at 22:05:41 UTC,
and completed at 22:10:35 UTC. Do not resend it. The final reply linked
`site/index.html` and identified the protected Forge preview as noninteractive.
The authoritative Forge Work Focus `focus.jardim-ideias-v1-funcional` is now
`completed`, record digest
`sha256:f43e52e3b1213f2ee3e5b4ae7f3b0927373158e240fa30727cbbfde435719509`,
with explicit Quick Cycle closeouts for all five lifecycle stages. Those
closeouts are cooperative agent evidence, not independent compliance proof.

**PASS:** Earlier independent Chromium local-file checks covered add, list,
reload/persist, remove and reload. The installed public 0.1.21 app passed a
hidden native **read-only** restart of this same real chat: completed Forge
Work Focus and direction visible, final local-file button actionable, protected
preview displaying the result, zero new sends, unchanged project files. Their
SHA-256 hashes are `2B580FEAB7A754ECDFBD239AD5D5CFD48F83FA4F2DB30CAEEF7E5930F3E9B120`
(`site/index.html`), `1783A7A76F0201516400F17BD4945265DB9BDF76D3F0D8D687F1793FF8794BDA`
(`site/assets/site.css`), and `358A78547E688CDE64831DAA0263483F0ABEF9D3F76C26A3454E3CF6EE566643`
(`site/assets/site.js`). The one-shot finalization harness initially reported
a **test-only false failure**: it searched the rendered message's `innerText`
for raw Markdown syntax, while the correct UI renders the link as a button.
Its assertion now checks that button; the provider turn was not repeated.

The native test harness now uses a progress-aware bounded observer rather than
a fixed 600-second wait. Three focused Node tests, script syntax checks and
`git diff --check` pass. This is test/checkpoint work only; no product binary
changed and no new installer is warranted. The third real `gpt-6-sol` high turn
reported 957,949 input tokens including 831,360 cached and 4,629 output
including 2,158 reasoning (962,578 total). The three-turn journey reports
3,184,228 observed tokens in total, but no controlled Astra/Sol comparison.
The shared Pro weekly gauge remained 49% before/after the last turn; rounded
and shared values cannot attribute per-turn allowance use. Task BRL cost and
API-equivalent cost are **UNKNOWN**; do not equate raw token totals with a Pro
bill or a saving. No subagents were used.

**NOT_RUN:** a real request to change this result in the same chat, clean-machine
or fresh-account setup, manual accessibility acceptance, or mobile. The
finished Forge record is not a publication of the disposable page. **Next
exact step:** run one bounded, reversible change request in the existing native
chat and verify the modified local file, Forge's new/current Work Focus,
native result link and restart. Do not resend either interrupted implementation
turn or the completed finalization turn. Keep the full desktop goal active.

### Desktop completed-work wording polish — 2026-09-27

A native screenshot of the completed real Work Focus exposed a concrete
confusion: the page displayed "Trabalho registrado concluído" next to a
"Descoberta" project stage and labeled the completed activity "Agora".
The UI now labels that card "ETAPA GERAL DO PROJETO", explains that a
specific completed work item does not automatically change the general
stage, and changes "Agora" to "Resultado registrado" for completed work
("Último registro" for abandoned work). It does not change Forge state.

**PASS:** the controlled browser UI suite, `node --check`, `git diff --check`,
incremental Desktop debug build, and hidden native read-only restart on the
same real completed conversation. The native run showed the exact Forge
record, actionable result button, protected preview and zero sends. The
debug screenshot is `C:/ForgeFast/forge-completed-debug-20260927.png`.
The first browser attempt could not find its expected Playwright browser;
rerunning with the already installed Microsoft Edge executable passed.
**NOT_RUN:** real change request after this result, new installer, or testing
the new wording in the installed 0.1.21 release. These wording edits are
source/debug-only and require a future coherent alpha package before users
receive them. No new provider-model turn or Rust source change was made.
**Next exact step:** use the same native chat for one bounded change request
on this disposable result, or investigate a concrete blocker if one appears;
then package the accepted UI changes with any related fixes. Do not spend a
new provider turn merely to reconfirm the already completed result.

### Desktop usable local HTML action — 2026-09-27

The same real-result screenshot exposed a larger usability gap: the protected
Forge preview shows an HTML page but intentionally cannot run its JavaScript.
The source/debug app now offers **Usar no navegador** only for a validated
local HTML result. This is an explicit action, never automatic. The UI warns
that a normal browser can execute code and use the network. The native command
rechecks the exact Forge project, canonicalizes the selected file, rejects
outside-project paths and non-HTML files, then asks Windows to open it with
the default HTML handler. Forge's protected preview remains unchanged.

**PASS:** Desktop `cargo check -p forge-desktop`, five focused preview tests,
all 50 Desktop crate tests, browser UI suite (including no automatic open,
explicit action, error handling and enlarged text), `node --check`,
`git diff --check`, incremental debug build, and a hidden native read-only
restart of the real completed conversation. The native test saw the new
action and invoked the registered command with a missing file; it rejected
the file without launching anything. The resulting screenshot is
`C:/ForgeFast/forge-browser-action-debug-20260927.png`.

**NOT_RUN:** positive Windows default-browser launch. That side effect was
deliberately not triggered because it could open a visible browser tab on the
maintainer's active desktop despite a hidden test desktop. The source/debug
change is not installed or published; installed public 0.1.21 has no action.
No real Codex provider turn was spent on this slice. **Next exact step:**
perform final package checks and prepare a coherent 0.1.22 alpha candidate
for the completed-work wording and usable HTML action; verify the installer
over 0.1.21 and disclose the untested positive browser launch before public
release. Keep the broader nontechnical journey goal active.

### Desktop 0.1.22 usable-result candidate — 2026-09-27

The completed-work wording and explicit HTML browser action above are now a
coherent 0.1.22 alpha package. The version was bumped in Desktop Cargo and
Tauri config; release notes are in `RELEASE_NOTES-0.1.22.md`.

**PASS:** 11 Node unit tests; browser UI suite (including no automatic open,
error path, large text); Desktop `cargo check`, all 50 crate tests and strict
Clippy; `git diff --check`; hidden native debug real-chat readback; one NSIS
release build. The single candidate is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.22_x64-setup.exe`,
123,049,545 bytes, SHA-256
`3C619DBCE8451760C091BF34B754FFE3B7D0E467786348789F3FA91C402669C1`.
Silent installation over public 0.1.21 returned exit 0 and left the candidate
hash unchanged. The installed app executable SHA-256 is
`849B3E2ED14280C287321F169EA5937B157C37D7BAFB3FF2FE6DB729B2DD0CD7`;
bundled core remains pinned to 0.13.2 with SHA-256
`CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`.
The installed 0.1.22 passed hidden native smoke and read-only restart of the
same real Codex chat: completed Forge state, final local file, HTML-only
browser action, invalid-file rejection, and zero sends. The first release
command rejected unsupported CLI arguments before building; a corrected
single build produced the candidate above.

**NOT_RUN:** a positive default-browser launch (could surface on the active
desktop), clean-machine/fresh-account setup, manual accessibility acceptance,
mobile, and a new real change request after the final result. No new
provider-model turn was spent on 0.1.22; per-task BRL cost remains UNKNOWN.
At this checkpoint the source and tested candidate are **local only**, not
committed/pushed or published. **Next exact step:** review/selectively commit
and push 0.1.22, publish the exact candidate under the maintainer's standing
alpha authorization, then download unauthenticated and compare bytes before
calling it available. Do not rebuild merely to publish. The wider journey
goal remains active.

### Desktop 0.1.22 public release readback — 2026-09-27

Source commit `fa73bfe2e1b4b01505d2b41ee5bb042609432df1` was pushed.
Annotated tag `desktop-v0.1.22-alpha.1` points to it and was pushed. The
public prerelease is
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.22-alpha.1`.
GitHub lists one installer (123,049,545 bytes) and its checksum sidecar. An
unauthenticated direct download to
`C:/ForgeFast/forge-alpha-0122-public-20260927/Forge_0.1.22_x64-setup.exe`
matched the exact tested candidate SHA-256
`3C619DBCE8451760C091BF34B754FFE3B7D0E467786348789F3FA91C402669C1`;
the downloaded sidecar matched too. Silent installation of **those downloaded
bytes** returned exit 0. Installed app and bundled core hashes matched the
candidate readback. A hidden native read-only restart of the real completed
Codex conversation again passed Forge state, final file, browser-action
visibility, invalid-file rejection and zero sends. No manual GitHub CI or
new provider-model turn was run.

This proves public installer availability and installed-byte continuity, not
successful launch into the default browser, clean-machine/fresh-account
setup, or a fresh change request after the completed result. `cargo fmt
--check` was attempted but does not pass on the Desktop workspace's existing
unformatted Rust files (including portions of the touched preview module);
no broad formatting rewrite was made. Compilation, all crate tests and strict
Clippy passed. **Next exact step:** choose a bounded change to the disposable
result in the same chat, then verify the changed file, Forge record and native
restart without duplicate sends. Separately, test the positive browser-open
action only in a safe isolated environment that cannot surface a window on the
maintainer's active desktop. Keep the full nontechnical journey goal active.

### Desktop same-chat change and readback — 2026-09-27

The installed public 0.1.22 app sent **one** change request in the existing
real Codex conversation of disposable project
`D:/Temp/User/forge-desktop-webview-ENrGgT/new-project`: add a local idea
search without changing the personal-use direction or erasing saved ideas.
The turn ran from 22:40:04Z to 22:51:28Z, with no retry or
subagents. The final answer linked `site/index.html`; the HTML and JavaScript
bytes changed. Forge now reports a distinct completed Work Focus,
`focus.jardim-ideias-busca-local`, digest
`sha256:00b0924028633ae339b82fcd6f9d27f6023f6114204d45aa148a532b82d4d83d`,
while the accepted objective record digest remained unchanged.

**PASS:** a separate headless Edge/Chromium test opened the local page with
JavaScript, saved three ideas, filtered case/accent-insensitively, confirmed
no-results and clear-search behavior, verified storage bytes unchanged by
search and preserved on reload, and saw no page errors or HTTP requests.
A second hidden **installed** native restart opened the same real conversation,
read the new Forge record, followed the final file link, found the search
control in the protected preview, and observed zero sends and unchanged
HTML/CSS/JS hashes. Its read-only browser-command check rejected a missing
file without launching a browser. Screenshot:
`C:/ForgeFast/forge-search-completed-native-20260927.png`.
`node --check` for the three touched test files and `git diff --check` passed.
No production code or installer was changed in this slice.

**Usage evidence:** provider rollout
`C:/Users/User/.codex/sessions/2026/09/27/rollout-2026-09-27T07-18-02-01a0e25f-27b4-7f73-b3e8-9298bccdde2f.jsonl`
shows `gpt-6-sol` for this turn. Subtracting the previous completed turn's
cumulative `token_count` from the final one yields 2,689,396 input tokens
(2,540,672 reported cached, a subset of input), 11,892 output tokens
(4,610 reported reasoning, a subset of output), 2,701,288 total. These are
provider-reported, per-conversation deltas, **not** a Pro bill. The account's
rounded shared weekly usage read 49% both before and after, so the subscription
quota impact of this turn is not attributable from that display. No verified
tariff for this Codex model, charged-token semantics or dated FX basis was
established; API-equivalent BRL cost is **UNKNOWN**, not zero. This long-chat
turn illustrates substantial repeated cached input; it does not establish a
Sol-versus-Astra savings claim. Parent supervision included the independent
browser/native checks and one failed patch-context attempt before the native
test edit; no product repair or duplicated provider turn was needed.

**NOT_RUN:** successful Windows default-browser launch, fresh-account and
clean-machine setup, manual accessibility acceptance, mobile/self-contained
distribution. The protected Forge preview intentionally does not execute page
JavaScript; functional behavior was tested in an isolated browser. **Next exact
step:** review the full nontechnical journey against current UI and open
issues, then take the highest-impact remaining gap as one coherent product
slice. Test positive browser launch only without surfacing a window on the
maintainer's desktop. Do not resend this completed change request.

### Installed 0.1.22 first-use sign-in boundary — 2026-09-27

Two headless native runs used the **installed public 0.1.22 executable** with
fresh temporary WebView profiles, disposable project folders and isolated
`CODEX_HOME`, without using the maintainer's signed-in Codex state. The real
bundled Codex CLI produced a device-code sign-in challenge. The app kept the
person's unsent draft, showed that nothing had been sent, displayed the
Codex-owned access address, and canceled sign-in without losing the draft.
Screenshot: `C:/ForgeFast/forge-first-login-installed-20260927.png`.

A second run used a **simulated completion** after the challenge. It verified
that the draft remained intact and the first attempted send supplied the
installed bundled Start Forge skill and exact bundled Forge executable path.
Both `native-auth.cjs` runs passed through `native-hidden.ps1` without touching
the visible desktop or sending a real provider-model turn. Their temporary
profiles were removed. This extends package-level first-use evidence, but it
does **not** prove a real new account can finish authorization or that the
agent then follows the supplied skill. **NOT_RUN:** actual login completion,
clean-machine install, and positive default-browser launch. **Next exact
step:** make the real completion path testable without borrowing or changing
the maintainer's login; then verify the signed-in first response and Forge
activation in a disposable project, or move to a concrete UI defect if that
test environment is unavailable. Keep the full nontechnical journey goal active.

### First-use empty-record wording — 2026-09-27

Visual inspection of the installed fresh-profile screenshot showed the empty
Forge record claiming that "the conversation and files continue here" even for
a new folder with neither. This is misleading for a first-time person. The
source UI now says there is **no recorded progress**, explains that recorded
work will appear there later, and directs the person to start **or** continue
in the conversation. It does not fabricate a stage, conversation or file.
The same wording is used for the header badge and the record's empty state.

**PASS:** `node --check` of the touched frontend/test scripts,
`git diff --check`, the browser UI suite, incremental Desktop debug build,
and a hidden native first-use run with an isolated unsigned-in `CODEX_HOME`.
The native run confirmed the new copy from real Forge readback before the
Codex device-login challenge, then canceled sign-in without sending or losing
the draft. Screenshot:
`C:/ForgeFast/forge-empty-record-copy-debug-20260927.png`.
No provider-model turn or Rust source change was made. The first browser run
failed only because its old expected status text had not been updated; the
test was corrected and the full suite passed. One initial source patch failed
on an unmatched context line before the successful edit.

**NOT_RUN:** installed/public behavior of this new copy. Installed 0.1.22
still has the old text; a coherent later alpha package is required. Real
new-account login completion and positive browser launch remain unverified.
**Next exact step:** inspect the remaining first-use UI for a concrete
contradiction or missing action, then package the accepted first-use changes
together rather than publishing each copy adjustment separately. Keep the
full journey goal active.

### Searchable recent projects — 2026-09-27

The active nontechnical desktop-journey goal remains in force. `Meus projetos`
now keeps up to 50 **local shortcuts** (formerly eight), provides an
accent-insensitive search over displayed name and folder path, shows a live
match count and a clear no-results action, and clears the search after opening
a project. This is not a second project registry: opening still revalidates the
exact folder with the native Forge inspection command. The ready message now
says the person may start **or continue** the conversation.

**PASS:** `node --check` for touched scripts; `git diff --check`; full
`node apps/desktop/tests/browser.cjs` suite, including a 60-shortcut fixture
capped at 50, accent-insensitive and same-name-path search, no-results,
revalidation and narrow-window overflow; incremental offline
`cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml -p forge-desktop`;
and `native-project-search.cjs` via `native-hidden.ps1` against the debug
executable and a real disposable Forge project. Native readback found and
opened that exact project without sending a message or surfacing a window.
Visual readback: `C:/ForgeFast/forge-project-search-debug-20260927.png`.
No Rust source or real provider turn was involved.

**NOT_RUN:** this change in the publicly installed app, real fresh-account
sign-in completion, positive default-browser launch, clean-machine install,
manual accessibility acceptance, mobile and self-contained distribution.
Installed/public 0.1.22 remains unchanged. This slice was executed by the
parent without subagents; no per-model API-equivalent BRL amount or Pro-quota
impact is defensibly attributable from the available counters. **Next exact
step:** review this diff and prepare a coherent next alpha package containing
the first-use wording and project-finding improvements; run package-boundary
checks, install/update the single candidate headlessly, verify its hash and
download before calling the new version available.

### Desktop 0.1.23 candidate and installed upgrade — 2026-09-27

The first-use wording and searchable recent-project shortcuts are packaged as
Desktop 0.1.23. Version changes are scoped to Desktop Cargo, lockfile and Tauri
config. The release scope and limits are in `RELEASE_NOTES-0.1.23.md`.

**PASS:** eight frontend unit tests, full browser UI suite, Desktop offline
`cargo check`, all 50 Desktop crate tests, strict Clippy, `git diff --check`,
and one NSIS release build. The candidate is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.23_x64-setup.exe`,
123,073,611 bytes, SHA-256
`EA27A6081F8F3A4F4318487833C08F624FDE606F2A5A9E94DA08F11EA1C1818A`.
Silent installation over public 0.1.22 returned exit 0 and did not change
the candidate hash. Installed app SHA-256 is
`F362B88D44F9DDDB0DD7D6B8CB4CCF0CF91D90AF98DCFBF880E260CF3EEB2076`;
bundled core 0.13.2 remains
`CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`.
Hidden native installed smoke passed identity, navigation, long-history fixture
and appearance; the real disposable Forge project was found among 50 shortcuts
and reopened through native inspection with zero sends. A first-use simulated
login completion verified the bundled Start Forge instructions and exact core
path without attempting a real account login or provider-model turn. No
manual GitHub CI was run.

**NOT_RUN:** clean-machine setup, real fresh-account login completion,
successful default-browser launch, manual accessibility acceptance, mobile,
self-contained distribution, or a new provider turn through 0.1.23. The parent
executed this slice without subagents; per-task API-equivalent BRL cost and
Pro-quota impact remain UNKNOWN, not zero. The tested candidate is **local**, not
public yet. **Next exact step:** review and selectively commit/push this release
package, publish only the tested candidate under the maintainer's standing
alpha authorization, then download without authentication and verify exact
bytes and installed identity before calling it available. Keep the full
nontechnical journey goal active.

### Desktop 0.1.23 public release readback — 2026-09-27

The tested package commit `0e3e1d79` was pushed. Annotated tag
`desktop-v0.1.23-alpha.1` points to it and was pushed. The public prerelease is
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.23-alpha.1`.
GitHub lists one 123,073,611-byte installer and its checksum sidecar. An
unauthenticated download to
`C:/ForgeFast/forge-alpha-0123-public-20260927/Forge_0.1.23_x64-setup.exe`
matched the exact tested candidate SHA-256
`EA27A6081F8F3A4F4318487833C08F624FDE606F2A5A9E94DA08F11EA1C1818A`;
the downloaded sidecar matched too. Silent installation of those downloaded
bytes returned exit 0. Installed app version reads 0.1.23 and executable hash
matches candidate readback:
`F362B88D44F9DDDB0DD7D6B8CB4CCF0CF91D90AF98DCFBF880E260CF3EEB2076`.
The installed app again passed hidden native shortcut search and exact Forge
project revalidation without sending a message. No manual GitHub CI or real
provider-model turn was run.

This proves public installer availability and installed-byte continuity for
the tested Windows machine, not clean-machine setup, real new-account login
completion, positive default-browser launch, manual accessibility acceptance,
or a full project journey in 0.1.23. **Next exact step:** progress the full
nontechnical journey by testing the highest-impact remaining real-user gap
without surfacing windows on the maintainer's active desktop. Do not resend
the already completed disposable change request. Keep the goal active.

### Quieter real-project record — 2026-09-27

Read-only hidden native inspection of the **installed public 0.1.23** reopened
the existing real Codex conversation in the disposable personal-ideas project,
opened its final HTML result, and prepared a same-chat change request without
sending. The composer received focus and was visible in the viewport. The
first run of an older test fixture failed because it expected the *previous*
page heading; setting the already-known current heading made the readback
pass. No product failure, duplicate send, or file change was inferred from
that stale test expectation. Screenshots:
`C:/ForgeFast/forge-0123-real-journey-readback-20260927.png` and
`C:/ForgeFast/forge-0123-change-composer-20260927.png`.

Visual comparison with `design/references/conversation-approved.png` showed
that the record exposed an additional long explanation of the agent-recorded
objective in the default view. The source UI now keeps that supporting
material behind **Objetivo registrado**, with the agent-origin/no-human-
approval caveat visible on expansion and the original text one further
expansion away. The project stage and recorded activity/next step remain
visible; the stage badge now says **Etapa do projeto** rather than exposing
Forge terminology. The history disclosure says **Histórico do objetivo**.
The browser tests check collapsed-by-default, expansion, refresh reset and
the absence of fabricated agreement; the native read-only test checks the
real Forge objective, preview, composer focus/visibility and zero sends.

**PASS:** `node --check` on touched scripts, `git diff --check`, full browser
UI suite, incremental offline Desktop debug build, and hidden native real-chat
readback against the debug build. The installed/public 0.1.23 **does not**
contain this record simplification. No Rust source change or provider-model
turn was made. No subagents were used; per-task BRL and Pro-quota impact remain
UNKNOWN. **Next exact step:** review and commit this UI/readback slice, then
continue the full nontechnical journey. Bundle it with a coherent later alpha
rather than claiming an installer update from a source commit. The real
fresh-account login, clean-machine setup and safe positive browser launch
remain unverified.

### First-use folder clarity — 2026-09-27

The full nontechnical desktop journey remains the active objective. A hidden
native visual audit of installed 0.1.23 covered Home, Explore, selected idea,
and the empty conversation without sending a provider turn. The project input
displayed `D:\MeusProjetos\MeuSite` as a placeholder, which could look like
an already selected folder. The source UI now says **Nenhuma pasta escolhida**
and explicitly offers the folder picker or a pasted path. The path remains
empty until the user actually chooses or enters one. Native screenshot hooks
were added for repeatable visual readback.

**PASS:** two full headless browser UI suite runs, `node --check` on touched
scripts, `git diff --check`, and visual inspection of
`C:/ForgeFast/forge-empty-folder-clarity-20260927.png`. The browser test
asserts that the empty-state placeholder is not passed as a real path. The
installed native 0.1.23 visual smoke passed twice **before** this source copy
change. An incremental offline Desktop debug build of the changed UI passed.
Two hidden native debug runs then passed: the revised first-use screenshot
(`C:/ForgeFast/forge-debug-firstuse-folder-clarity-20260927.png`), real
Windows folder-dialog cancel and selection, onboarding of disposable empty
`C:/ForgeFast/forge-firstuse-folder-20260927`, bounded Forge record readback,
and authenticated Codex connection/disconnection with **zero messages sent**.
No Rust source change, provider-model turn, or subagent was involved. Per-task
BRL and Pro-quota impact remain UNKNOWN, not zero.

**Pending local changes:** `ui/index.html`, `tests/browser.cjs`, and
`tests/native.cjs`; no commit, push, version bump or installer update in this
slice. **Next exact step:** test a first real Codex message and result from a
new disposable project (the folder selection and idle connection already
passed), then bundle this clarity change with a coherent Desktop alpha and
run package-boundary checks. Do not imply a source commit updated the
installed app.

### Real desktop creation-and-change journey — 2026-09-27

The source debug build passed a hidden native run with the actual Windows
folder picker and real Codex. A disposable project received one initial
message, then Codex created `site/index.html` with local CSS. The app opened
that file in its isolated HTML preview, exposed a **Pedir mudança neste
arquivo** action without sending prematurely, sent the follow-up in the same
conversation, and refreshed the rendered page after the file changed. A
WebView reload restored both sides of the conversation without resending.
The screenshot `C:/ForgeFast/forge-debug-artifact-journey-20260927.png`
visually confirms the real generated page inside the app. The test's generated
project lives under a temporary WebView test profile; it did not modify a
maintainer project.

**PASS:** hidden native `native.cjs` with `FORGE_TEST_PROJECT`, real folder
selection, `FORGE_TEST_AGENT_SMOKE=1`, and
`FORGE_TEST_ARTIFACT_JOURNEY=1`; all assertions passed. This proves the
tested debug-build journey on this authenticated Windows machine, not a fresh
account, clean machine, installed candidate, or mobile. No subagent was used.
Per-task BRL and Pro-quota impact remain UNKNOWN. **Next exact step:** prepare
the coherent Desktop 0.1.24 alpha package for the project-record and first-use
clarity changes, run package-boundary checks, install the single candidate
headlessly over 0.1.23, and verify its hash. Do not claim publication until
the public download matches that candidate.

### Desktop 0.1.24 candidate installed locally — 2026-09-27

The 0.1.24 package combines the quieter project record and unambiguous
first-use folder field. `RELEASE_NOTES-0.1.24.md` records the exact changes
and limits. **PASS:** eight frontend unit tests, the full headless browser
suite, Desktop offline `cargo check`, all 50 Desktop crate tests, strict
Clippy, `git diff --check`, and one NSIS release build. No core-workspace or
GitHub CI run was triggered for these Desktop UI changes.

The one candidate is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.24_x64-setup.exe`,
123,068,738 bytes, SHA-256
`EC12F1AC8347FD54163716EDDC91455772EA83003E9CFA4835C60E6CE967B803`.
Its silent installation over 0.1.23 returned exit 0, did not change its hash,
and installed version 0.1.24 (executable SHA-256
`13E7E514C1741D6F95E1254BCB50062896A2E87FA3FB0FC848B2243CAA739EAF`).
The **installed** candidate passed a hidden native run of actual Windows
folder selection, Forge project onboarding and record readback, plus an idle
authenticated Codex connection/disconnection with zero sends. A real
Codex-generated page, same-chat change and reload had passed in the source
debug build before packaging. Clean-machine setup, fresh-account login and
positive external-browser opening remain unverified.

**State:** installed locally, not yet committed or public. No subagents were
used. Per-task BRL and Pro-quota impact remain UNKNOWN. **Next exact step:**
review and selectively commit/push this package, publish only the already
tested candidate under the maintainer's standing alpha authorization, then
download anonymously and compare exact bytes before calling it available.

### Desktop 0.1.24 public release readback — 2026-09-27

Package commit `49104d05` was pushed. Annotated tag
`desktop-v0.1.24-alpha.1` resolves to that commit and was pushed. The public
prerelease is
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.24-alpha.1`.
GitHub lists the single 123,068,738-byte installer and its checksum sidecar.
An unauthenticated download to
`C:/ForgeFast/forge-alpha-0124-public-20260927/Forge_0.1.24_x64-setup.exe`
matched the tested candidate SHA-256
`EC12F1AC8347FD54163716EDDC91455772EA83003E9CFA4835C60E6CE967B803`;
the downloaded sidecar matched. Silent installation of those downloaded
bytes returned exit 0. Installed app version is 0.1.24 and executable SHA-256
is `13E7E514C1741D6F95E1254BCB50062896A2E87FA3FB0FC848B2243CAA739EAF`.
No manual GitHub CI was run. The release is available, but this is not proof
of clean-machine setup or first-login completion.

**Next exact step:** continue the full nontechnical journey with a read-only
installed-app pass on a real persistent conversation and its project record;
then select the highest-impact remaining gap (fresh-account login,
external-browser opening, or accessible navigation) from actual evidence.
Do not repeat provider turns solely to reprove the unchanged chat path.

### Installed 0.1.24 real-history readback — 2026-09-27

The installed public 0.1.24 opened the existing disposable personal-ideas
project and its real 33-message Codex conversation. The first attempt stopped
at a **test-fixture assumption** requiring at least 50 messages; this chat has
33. No app failure or duplicate provider turn was inferred. `tests/native.cjs`
now permits an explicitly supplied positive minimum while retaining 50 as the
default, and the same installed-app test passed with a minimum of 30. It
displayed the actual Forge record, resumed the conversation in about 4.6 s,
kept the composer and Send visible, then reloaded the WebView and preserved
message count plus first/last order. No Send action was invoked. Native
screenshots are `C:/ForgeFast/forge-0124-installed-record-20260927.png` and
`C:/ForgeFast/forge-0124-installed-history-20260927.png`.

**PASS:** `node --check tests/native.cjs` and hidden native installed-app
readback/reload with the actual project and thread. **NOT_RUN:** a fresh
account's login completion, clean-machine installation, positive launch in
the default browser, manual accessibility acceptance, mobile and offline
self-contained distribution. This post-release change touches only the test
harness and checkpoint, not the installed product. No subagent or provider
turn was used; task-level BRL and Pro-quota impact remain UNKNOWN.
**Next exact step:** exercise a fresh-account login completion or a safely
isolated positive external-browser launch, whichever can be verified without
taking the maintainer's active desktop, then address any concrete failure.

### Default-browser handoff correction — 2026-09-27

The installed 0.1.24 app still hands a `.html` file path to Windows
`ShellExecuteW`. Windows resolves that by the file's application association,
which may be an editor rather than the person's default browser. The source
Desktop implementation now queries the executable associated with the HTTPS
protocol, converts the already project-validated HTML path into an encoded
`file:` URL, and passes that URL as one argument to the browser executable.
It fails with an actionable status when no browser association can be
resolved; it does not silently fall back to a non-browser HTML handler. The
protected in-app preview and project-bound file validation remain unchanged.
Relevant Windows API references: `AssocQueryStringW` and `ASSOCSTR_EXECUTABLE`
in Microsoft Learn (`learn.microsoft.com/en-us/windows/win32/api/shlwapi/`).

**PASS:** focused Desktop `cargo check`; five preview tests (one opt-in
association probe ignored by default); the explicit opt-in Windows association
probe resolved an existing executable without launching it; all 50 regular
Desktop crate tests; strict Clippy; full headless browser UI suite;
`git diff --check`; incremental Desktop debug build; hidden native debug
smoke for project selection, bounded preview and Forge record. An existing
headless axe scan found no automatic WCAG/best-practice violations across
eight seeded UI states, but could not determine contrast for many elements
over gradients, so accessibility acceptance is **PARTIAL**, not complete.
`cargo fmt --check` on the whole Desktop crate fails on pre-existing
formatting outside this change; no broad formatting rewrite was made.

**NOT_RUN:** positive external-browser launch on the maintainer's active
Windows desktop, fresh-account login completion, clean-machine install,
manual accessibility acceptance. This source change is **not** in the
installed/public 0.1.24 package. Current local modifications are
`src-tauri/src/preview.rs`, `ui/preview.mjs` and this checkpoint. No subagent
or provider-model turn was used; per-task BRL and Pro-quota impact remain
UNKNOWN. **Next exact step:** arrange an isolated positive browser-launch
readback that cannot surface on the active desktop; if that is not possible,
keep it NOT_RUN and package only with an explicit limitation after a coherent
next Desktop update. Do not claim that association lookup proves navigation.

### Isolated browser rendering readback — 2026-09-27

The registered HTTPS browser executable on this Windows account is Chrome.
Playwright launched that exact executable **headlessly** with an isolated
browser context and opened the real generated project's `file:` URL. A new
idea could be saved and remained visible after page reload (`1` before and
after), confirming that the local HTML, script and browser storage work in
that browser without taking focus from the maintainer's desktop. The
association probe and URL-encoding unit test above cover the two new native
handoff inputs. This still does **not** exercise the Forge button or its
native command spawning a visible browser, so that positive end-to-end path
remains **NOT_RUN** rather than silently promoted to PASS. No provider turn
was sent and the persistent project files were not edited by this browser
test; only the isolated browser profile stored the test idea.

**Next exact step:** keep the actual Forge-to-browser launch limitation
explicit, then either verify it in an isolated Windows desktop session or
include this narrowly scoped fix in the next coherent Desktop alpha with a
clear unverified-path note. Continue the remaining first-use/login and visual
journey against the app itself; do not treat headless HTML rendering as full
native acceptance.

### First-use and completed-work clarity — 2026-09-27

The hidden native first-use test passed with an isolated `CODEX_HOME` and
fixture login completion: the person's unsent draft survived, no early
message was sent, and the first post-login send supplied the bundled Start
Forge guidance path to the Codex thread. **Real provider login completion
remains NOT_RUN**; the fixture is not a substitute for authenticating a new
account. No maintainer credentials were changed.

Visual readback of the real project showed that the completed-work badge
could be read as the *whole project* being finished while the Forge project
stage still said Discovery. The source UI now says “Esta parte foi concluída”
and explains “O projeto pode continuar”; it does not change or infer any
Forge state. The full headless browser UI suite passed, including the
completed-state assertion. An incremental Desktop debug build passed, and a
hidden native run against the existing real project passed project/record,
preview and first-use smoke without a provider turn. The resulting native
screenshot is `C:/ForgeFast/forge-next-record-completed-20260927.png`.

**State:** source-only, not installed/public. Local changes are
`src-tauri/src/preview.rs`, `ui/preview.mjs`, `ui/progress.mjs`,
`tests/browser.cjs` and this checkpoint. Browser-launch button, real fresh
login and clean-machine install remain NOT_RUN. No subagent was used;
per-task BRL and Pro-quota impact remain UNKNOWN. **Next exact step:** review
the integrated five-file diff, then prepare a coherent Desktop alpha package
with explicit limitations rather than treating this source build as an app
update. The native browser handoff itself needs an isolated positive test or
must remain a declared limitation of that package.

### Desktop 0.1.25 candidate installed locally — 2026-09-27

The 0.1.25 package combines default-browser handoff correction and clearer
completed-work wording. `RELEASE_NOTES-0.1.25.md` states both changes and
limits. Version changed only in Desktop Cargo manifest/lock and Tauri config.

**PASS:** focused Desktop `cargo check` and five preview tests; all 50
regular Desktop crate tests; strict Clippy; eight frontend unit tests;
the full headless browser suite from the preceding slice; `git diff --check`;
one NSIS release build. No core-workspace or GitHub CI run was triggered.
The exact candidate is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.25_x64-setup.exe`,
123,042,265 bytes, SHA-256
`F76671E32C235F614C059DD3C6CA7537B6C6C55B22E00BF4E471AC096B930CC3`.
The `.sha256` sidecar sits beside it. Silent installation over public
0.1.24 returned exit 0 without changing the candidate hash; installed
version is 0.1.25 and executable SHA-256 is
`A6DCDA319C241CDD089D3FF33A44204C8ACB6CC5424F842537AA268EE6CCAC39`.

The **installed** candidate passed hidden native selection with the real
Windows folder dialog, Forge onboarding/record, safe local preview and
navigation. The test flag `FORGE_TEST_AGENT_SMOKE=1` also sent **one real
simple Codex prompt** in a disposable project and verified a reply plus
WebView-reload continuity. That provider turn was not needed for the browser
change and should not be repeated casually; it used no tools or file edits.
A separate hidden native read-only run reopened the existing real 33-message
conversation and preserved message order after reload without a send.

**NOT_RUN:** Forge button launching the external browser, fresh-account real
provider login, clean-machine setup, manual accessibility acceptance,
mobile and offline self-contained distribution. This is **installed locally,
not committed or public**. No subagent was used; task-level BRL and Pro-quota
impact remain UNKNOWN. **Next exact step:** review and selectively commit/push
the package, then publish this exact tested installer under the maintainer's
standing alpha authorization and verify an unauthenticated download's exact
bytes before calling 0.1.25 available. Do not rebuild the candidate.

### Desktop 0.1.25 public release readback — 2026-09-27

Package commit `40c0a484` was pushed on `codex/desktop-shell`. Annotated tag
`desktop-v0.1.25-alpha.1` resolves to it and was pushed. The public
prerelease is
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.25-alpha.1`.
An unauthenticated download to
`C:/ForgeFast/forge-alpha-0125-public-20260927/Forge_0.1.25_x64-setup.exe`
matched the tested candidate: 123,042,265 bytes and SHA-256
`F76671E32C235F614C059DD3C6CA7537B6C6C55B22E00BF4E471AC096B930CC3`.
The downloaded checksum sidecar matched. Silent installation of the
downloaded bytes returned exit 0; installed version is 0.1.25 and executable
SHA-256 remains
`A6DCDA319C241CDD089D3FF33A44204C8ACB6CC5424F842537AA268EE6CCAC39`.
No manual GitHub CI was run. This proves public availability and byte
continuity on this machine, not a clean-machine installation.

**State:** 0.1.25 is published and installed. The actual Forge button
launching the external browser remains NOT_RUN, as do real fresh-account
login completion and manual accessibility acceptance. No subagent was used;
per-task BRL and Pro-quota impact remain UNKNOWN. **Next exact step:** return
to the whole nontechnical journey and select a remaining high-impact gap
from native readback rather than treating publication as full product
completion. Prioritize an isolated external-browser launch test if it can
avoid the active desktop; otherwise retain the explicit limitation and work
on first-use/visual acceptance. Keep mobile and offline distribution for
later stages of the same goal.

### Installed real-result continuity after full restart — 2026-09-27

An isolation probe attempted to give Windows Chrome a temporary
`LOCALAPPDATA` before opening the real generated HTML headlessly. The page
rendered, but Chrome did **not** create a user-data profile under that
temporary directory. Thus this environment variable does not establish a
safe separate browser instance here. The Chromium user-data-dir guide
documents `--user-data-dir` for Windows and `CHROME_USER_DATA_DIR` only for
Linux (`chromium.googlesource.com/chromium/src/+/HEAD/docs/user_data_dir.md`).
The Forge button remains **NOT_RUN** rather than risking a tab on the
maintainer's active desktop. No registry/default-browser settings were
changed.

Instead, a new opt-in hidden native read-only test against installed 0.1.25
used the actual 33-message personal-ideas Codex thread. It reopened the
real reply's `site/index.html` link through the project-bound preview,
rendered the changed heading, prepared a change request without sending,
fully stopped and restarted the Desktop process with the same isolated
WebView profile, then repeated the conversation/file action. Message count
and first/last order were unchanged, and the generated HTML SHA-256 did not
change. **PASS:** `node --check tests/native-real-result-readonly.cjs`,
`git diff --check`, and `tests/native-hidden.ps1` with
`FORGE_NATIVE_TEST_SCRIPT=native-real-result-readonly.cjs`. No provider turn
was sent. New local files changed: `tests/native-real-result-readonly.cjs`,
`tests/native-hidden.ps1`, and this checkpoint. These changes are test-only,
not in the published 0.1.25 installer. No subagent was used; task-level BRL
and Pro-quota impact remain UNKNOWN.

**Next exact step:** inspect the still-unverified first-use and accessibility
paths for a concrete UI problem that can be tested in the native app without
altering the maintainer's account or foreground desktop. Do not rerun the
same real conversation merely for reassurance. Keep the browser-button
limitation explicit until truly isolated positive evidence is available.

### First-use large-text reflow — 2026-09-27

A headless 360 CSS-pixel / 200% root-text inspection found a concrete visual
problem: the Home and Explore headings clipped or split awkwardly, and the
absolute-positioned appearance disclosure could cover the start of the active
screen. `ui/styles.css` now gives both headings explicit wrapping, scales their
mobile type to preserve whole words at this text size, and keeps appearance in
normal flow on narrow screens. The Home introduction uses the mobile text size.
No engine/domain behavior changed.

**PASS:** a new `tests/browser.cjs` regression failed before the CSS fix on the
Home heading, then the complete mocked-IPC browser suite passed afterward.
It checks both screens at 360px/200% for heading clipping, horizontal overflow,
and appearance overlap. Headless Chromium visual readback of Home and Explore at
that size and Explore at normal mobile text showed the headings legible;
`git diff --check` passed. Those screenshots live under
`C:/ForgeFast/forge-reflow-*20260927.png` and
`C:/ForgeFast/forge-mobile-explore-verified-20260927.png`. These are browser
checks, **not native WebView acceptance**. No Rust tests were run for CSS-only
changes. Installed/public 0.1.25 does not include these source edits.

**Current local state:** `ui/styles.css`, `tests/browser.cjs`,
`tests/native-hidden.ps1`, this README and untracked
`tests/native-real-result-readonly.cjs`; no commit/push/publication in this
slice. No subagent used; per-task BRL and Pro-quota cost remain UNKNOWN.
**Next exact step:** inspect the integrated diff and run a hidden native source
build/smoke at the next coherent Desktop package boundary, then decide whether
these visual fixes and the read-only continuity test form 0.1.26. Keep the
external-browser button and fresh-account login as NOT_RUN until genuinely
isolated evidence exists; do not claim a browser check proves native behavior.

### Hidden native reflow verification — 2026-09-27

A source Desktop debug executable was rebuilt with `cargo build --manifest-path
apps/desktop/src-tauri/Cargo.toml -p forge-desktop --offline --locked -j2` using
the existing target cache. The hidden native WebView smoke now checks the same
360px / 200% text reflow for Home and Explore before its existing journey.
**PASS:** native headings did not clip, no horizontal overflow appeared, and
appearance controls did not overlap headings. The rest of the no-project native
smoke passed (formatting, Explore, controlled long history, first-use draft,
missing-folder guidance, appearance reload, frontend-to-Rust identity/retry).
`node --check tests/native.cjs` and `git diff --check` passed. The test ran
on an isolated Windows desktop and WebView profile, not the maintainer's
foreground desktop. Real project resolution was NOT_RUN in this specific run
because no project path was supplied; it was previously covered by the
installed 0.1.25 smoke. This is native layout evidence, not manual
accessibility acceptance or an installed update.

**Local state:** source edits now include `ui/styles.css`, `tests/browser.cjs`,
`tests/native.cjs`, plus the prior read-only result test and its hidden-runner
allowlist, and this README. No commit/push/release. Per-task BRL and Pro-quota
cost remain UNKNOWN. **Next exact step:** group these verified UI and
continuity improvements into a coherent next Desktop alpha only when release
scope is settled; review diff, bump version, run package-boundary checks, then
test an exact installer before publication. External-browser button, real
fresh-account login, clean-machine installation and manual accessibility
acceptance remain explicit NOT_RUN.

### Desktop 0.1.26 installed candidate — 2026-09-27

The package boundary is now 0.1.26. UI changes are the narrow-screen/large-text
Home and Explore reflow and non-overlapping appearance control; the four-screen
360px/200% regression also covers Projects and Workspace. The separate
read-only real-result restart test is source-only test coverage, not a new
runtime feature. `RELEASE_NOTES-0.1.26.md` states scope and limits. Version
changed only in the Desktop Cargo manifest/lock and Tauri config. The README
intro now identifies 0.1.25 as the prior published alpha.

**PASS:** focused Desktop `cargo check`; all 50 active Desktop crate tests
(one conditional browser-association test ignored); strict Clippy; eight
frontend unit tests; complete browser UI suite; JS syntax and `git diff
--check`. A single NSIS candidate was built with pinned staged core 0.13.2:
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.26_x64-setup.exe`,
123,088,230 bytes, SHA-256
`A5739B90347393B2F44F53D358622E9E5E0AF8E3A7D15248F32B95F257267DBA`.
Its `.sha256` sidecar is adjacent. Silent installation over public 0.1.25
returned exit 0; the candidate hash stayed unchanged. Installed version is
0.1.26 and executable SHA-256 is
`AAC88FEAE37B81E6ACCBBAFC07B142E0B69DBEA0645F115BBBF50E2F8DEB85A9`.
The **installed** app passed the hidden native first-use/layout smoke for all
four screens, plus controlled history, draft, missing-folder, appearance and
frontend-to-Rust checks. No provider turn was sent. Real project resolution
was NOT_RUN in this particular installed-app smoke; installed 0.1.25 had
already passed that path and the runtime code is unchanged in this package.
No core-workspace test or GitHub CI was run for this UI/test-only package.

**State:** installed locally, not committed or public. The Forge button
launching an external browser, fresh-account real login, clean-machine setup,
manual accessibility acceptance, mobile and offline distribution remain
NOT_RUN or unfinished. No subagent was used; per-task BRL and Pro-quota cost
remain UNKNOWN. **Next exact step:** review/selectively commit and push the
0.1.26 package, publish the exact tested installer under standing alpha
publication authorization, and check an unauthenticated download's size/hash
before calling it available. Do not rebuild the candidate.

### Desktop 0.1.26 public release readback — 2026-09-27

Package commit `7c268b0e` was pushed on `codex/desktop-shell`; annotated tag
`desktop-v0.1.26-alpha.1` points to it and was pushed. Public prerelease:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.26-alpha.1`.
Unauthenticated download of installer and `.sha256` sidecar to
`C:/ForgeFast/forge-alpha-0126-public-20260927` matched the locally tested
candidate: 123,088,230 bytes and SHA-256
`A5739B90347393B2F44F53D358622E9E5E0AF8E3A7D15248F32B95F257267DBA`.
Silent installation of the downloaded bytes returned exit 0; installed version
is 0.1.26 and executable SHA-256 is
`AAC88FEAE37B81E6ACCBBAFC07B142E0B69DBEA0645F115BBBF50E2F8DEB85A9`.
No manual GitHub CI was run. This proves public byte continuity and successful
update on this machine, not a clean-machine installation.

**State:** 0.1.26 is public and installed. Actual external-browser-button
launch, real fresh-account login, clean-machine setup, manual accessibility
acceptance, mobile and self-contained distribution remain NOT_RUN/unfinished.
No subagent was used; per-task BRL and Pro-quota impact remain UNKNOWN.
**Next exact step:** continue the full nontechnical journey, focusing on the
remaining high-impact first-use/preview and conversation gaps rather than
repeating already proved browser or read-only history tests. Avoid publishing
another alpha for test-only changes; collect a coherent user-visible package.

### Installed first-use audit and clearer empty states — 2026-09-27

A hidden native audit of installed 0.1.26 captured Home, Explore, and the
first-use conversation on an isolated Windows desktop. The real Windows folder
dialog then canceled and selected a fresh disposable directory
`D:/Temp/User/forge-0126-new-project-audit`. Forge initialized that exact empty
folder through the existing action, returned its authoritative empty record,
and enabled the composer without sending a provider turn. The same smoke
checked onboarding of a separate folder with an existing file (unchanged),
local protected preview, path rejection and frontend-to-Rust identity. All
passed. Screenshots are under `C:/ForgeFast/forge-0126-*-audit.png`.
The installed project's initial screen repeated **Sem andamento registrado**
without making the next action obvious; the empty preview also assumed the
person knew which file to inspect. These were concrete clarity issues, not
backend failures.

**Source-only UI correction:** `ui/progress.mjs` and `ui/index.html` now say the
next step is not yet recorded and invite the person to begin in the adjacent
conversation, while keeping the distinction between Forge's record and files.
`ui/chat.mjs` and the HTML empty preview now explain that an agent-cited file
can be opened here, or a file can be chosen from the project folder. No
fictitious progress or result was added. Browser/native test expectations were
updated, including explicit assertions for the guidance. `node --check`,
`git diff --check`, full mocked-IPC browser suite and source-built hidden native
folder/onboarding/preview smoke passed. Native screenshot
`C:/ForgeFast/forge-next-empty-record-audit.png` showed the clearer copy.
No Rust behavior changed, so no new Rust check/test was run for this UI text
slice. Other concurrent Rust work on this machine made the source debug build
slow (~5m); no process was killed.

**Current state:** installed/public 0.1.26 remains unchanged. Local uncommitted
edits: `ui/progress.mjs`, `ui/index.html`, `ui/chat.mjs`,
`tests/browser.cjs`, `tests/native.cjs`, `tests/native-auth.cjs`,
`tests/artifact-resume.cjs`, and this README. The disposable project folder
remains intentionally available for another bounded first-use test; it is
not a user project. No subagent or provider turn was used; task-level BRL and
Pro-quota impact remain UNKNOWN. **Next exact step:** inspect a real
conversation/result screen for a concrete remaining interaction or clarity
gap, then integrate any related fix before considering the next alpha. Do not
publish a new installer solely for these text changes. Keep the external
browser button, fresh-account real login, clean-machine install and manual
accessibility acceptance as NOT_RUN.

### Real-result conversation audit — 2026-09-27

The source-built hidden native app resumed the existing disposable Codex chat
`01a0e25f-27b4-7f73-b3e8-9298bccdde2f` in
`D:/Temp/User/forge-desktop-webview-ENrGgT/new-project`. This chat has 33 real
messages and an actual local `site/index.html` result. The read-only test
opened the agent-cited file, displayed the updated "Jardim de ideias renovado"
page in the protected preview, prepared a change request in the same
conversation, then fully restarted the native process and repeated the check.
It verified message count/order, composer focus and viewport visibility, and
unchanged file SHA-256. **PASS:** no Send, provider turn or file edit occurred.
Screenshots: `C:/ForgeFast/forge-next-real-result-preview.png` and
`C:/ForgeFast/forge-next-real-result-change.png`. The latter was inspected
visually: the request draft, Send button, local result, and change action were
visible together. This is native read-only continuity evidence, **not** a new
real provider response or proof of external-browser launching.

The screenshot exposed one plain-language gap: "Publicação não verificada"
is ambiguous for a new user. Source `ui/index.html` now says the file is from
the chosen project folder and the preview does not confirm publication on the
internet. Related browser/native assertions were updated. The complete
mocked-IPC browser suite passed, as did JS syntax checks and `git diff
--check`. A direct run of `artifact-resume.cjs` without its required fixture
variables exited before testing; it is **NOT_RUN**, not a product failure.
The native real-result pass above preceded this last copy edit; no rebuilt
native binary or installer yet contains the latest sentence.

**Current state:** public/installed 0.1.26 is unchanged. Nine local files are
modified: `ui/progress.mjs`, `ui/index.html`, `ui/chat.mjs`, `tests/browser.cjs`,
`tests/native.cjs`, `tests/native-auth.cjs`, `tests/artifact-resume.cjs`,
`tests/native-real-result-readonly.cjs`, and this README. No commit, push or
release this slice. No subagent was used; task-level BRL and Pro-quota impact
remain UNKNOWN. **Next exact step:** review the integrated diff, rebuild the
source Desktop only at the next coherent package boundary, and verify the
final preview copy plus first-use empty state in the hidden native app before
version bump/installer work. Continue inspecting higher-impact #91/#92 UX
gaps; do not publish an alpha solely for wording. External-browser launch,
fresh-account login, clean-machine install and manual accessibility remain
NOT_RUN.

### Desktop 0.1.27 installed candidate — 2026-09-27

The coherent user-visible package combines clearer first-project guidance,
the result's local/publication wording, and a **Pedir mudança neste arquivo**
action immediately below the displayed result rather than buried after
technical preview notes. `RELEASE_NOTES-0.1.27.md` describes scope and limits.
Version changed only in the Desktop Cargo manifest/lock and Tauri config. No
Forge engine, Codex protocol or project-state behavior was changed. The
published alpha remains 0.1.26 until 0.1.27 is uploaded and downloaded back.

**PASS:** integrated diff review; complete mocked-IPC browser suite; eight
frontend unit tests; Desktop `cargo check`, 50 passed crate tests (one
conditional browser-association test ignored), strict Clippy; JS syntax and
`git diff --check`; source-built hidden native first-use/authoritative-record
smoke; and read-only native restart of a real 33-message conversation and
actual HTML result. The change action was in the visible viewport before
scrolling, then focused the composer without sending. The same source build
showed the final plain-language preview copy. No new provider turn was spent.
Rust checks were run once at the package boundary, not after each UI edit.

One NSIS candidate was built with the pinned staged Forge core 0.13.2:
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.27_x64-setup.exe`,
123,106,078 bytes, SHA-256
`F25D5B43D28D0FCD7E01B3D1ABE093E21D7AB49A61A59C3B6897AB894D83C724`.
Its `.sha256` sidecar is adjacent. Silent installation over public 0.1.26
returned exit 0 and left candidate bytes unchanged. The installed executable
reports 0.1.27 and has SHA-256
`410C0208EADD831DD3622BE24628F8025C1FAC3BCEDE57F583CF5764805C418A`.
The installed app passed hidden native smoke with an actual Windows folder
dialog and, separately, with an actual file dialog. The installed read-only
real-result/restart check also passed with no Send or file change. One run
using **both** real dialogs timed out after the Markdown preview step; it
passed on retry, including protected HTML/CSS/image rendering. The timeout
cause is unknown. The hidden runner now prints its log on timeout, and the
native test offers opt-in step diagnostics for any recurrence. Do not erase
that first incomplete run from the evidence or call its cause resolved.

**State:** 0.1.27 is installed locally, not yet committed or published. No
GitHub CI was run. External-browser button launch, fresh-account login,
clean-machine install, manual accessibility acceptance, mobile and offline
self-contained distribution remain NOT_RUN/unfinished. No subagent was used;
per-task BRL and Pro-quota impact remain UNKNOWN. **Next exact step:** review
and selectively commit/push this package, tag/release the exact tested
installer under the maintainer's standing alpha publication authorization,
then download unauthenticated bytes and verify size/hash and installed
readback. Do not rebuild this candidate.

### Desktop 0.1.27 public release readback — 2026-09-27

Package commit `53d88b6a` was pushed on `codex/desktop-shell` with the
maintainer's Git identity; annotated tag `desktop-v0.1.27-alpha.1` resolves to that
commit and was pushed. Public prerelease:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.27-alpha.1`.
It contains the exact tested installer and `.sha256` sidecar. Unauthenticated
download to `C:/ForgeFast/forge-alpha-0127-public-20260927` matched the
candidate: 123,106,078 bytes and SHA-256
`F25D5B43D28D0FCD7E01B3D1ABE093E21D7AB49A61A59C3B6897AB894D83C724`.
Silent installation of those downloaded bytes returned exit 0, left them
unchanged, and produced installed 0.1.27 executable SHA-256
`410C0208EADD831DD3622BE24628F8025C1FAC3BCEDE57F583CF5764805C418A`.
The downloaded installation passed the hidden read-only real-result/restart
test again with the existing 33-message Codex chat and no Send. No manual
GitHub CI was run. This proves public byte continuity and an update on this
machine, not a clean-machine install or a fresh provider reply.

**State:** 0.1.27 is public and installed. The combined two-dialog native
timeout remains an unresolved intermittent test observation despite separate
and retry PASS. External-browser launch, fresh-account login,
clean-machine installation, manual accessibility acceptance, mobile, and
offline self-contained distribution remain NOT_RUN or unfinished. No
subagent was used; per-task BRL and Pro-quota impact remain UNKNOWN.
**Next exact step:** return to the larger journey, not another wording-only
release. Inspect #91/#92 acceptance and the actual installed UI for a
high-impact remaining gap; do not repeat the same real provider conversation
without a new hypothesis. If the combined hidden-dialog timeout recurs,
use `FORGE_NATIVE_DIAGNOSTICS=1` and the runner's printed log to identify
the exact stalled step before attributing it to product behavior.

### Desktop nonvisual result card — 2026-09-27

Active objective: complete the approachable Windows journey from folder and
Forge onboarding through Codex conversation, real result, change request and
restart. This slice addresses #91's nonvisual-result alternative; it does
not close the whole issue or the objective. A project-bound file that cannot
be rendered (for example PDF) now returns only its validated relative path
and size, without reading its bytes or launching another app. The screen
shows a plain-language file card and can prepare a same-conversation request.
Common document, archive and media references in an agent reply can lead to
that card. Out-of-project paths remain rejected, and unsupported content
does not become executable preview content. No Forge or Codex state was
invented or changed.

**Changed files, uncommitted:** `src-tauri/src/preview.rs`,
`ui/index.html`, `ui/preview.mjs`, `ui/message-format.mjs`,
`ui/styles.css`, `tests/browser.cjs`, `tests/native.cjs`,
`tests/artifact-resume.cjs`, and this README. Desktop 0.1.27 public/installed
is unchanged; only the source debug build contains this work.

**PASS:** focused Desktop `cargo check` and five preview tests (one optional
browser-association test ignored); complete Desktop crate (50 pass, one
ignored); eight frontend unit tests; complete mocked-IPC browser suite;
source-built hidden native smoke on an isolated Windows desktop. The native
test used a real Forge-onboarded temporary project and the real Rust preview
command for a controlled local PDF fixture; it found the file, showed no
document bytes, and prepared a draft without sending. A visual screenshot was
inspected at `C:/ForgeFast/forge-nonvisual-result-source-20260927.png`.
`git diff --check` passed after the final wording/heading refinement.
The file-picker response for this
PDF was simulated; the native picker for other files passed in the prior
0.1.27 package. A real Codex reply citing a PDF, actual PDF rendering,
external viewer launch, provider Send, and clean-machine install are NOT_RUN.
The existing combined two-dialog timeout remains unexplained. No subagent
was used; requested/observed model for this direct parent slice is the
current parent only, without accessible per-task token counters. API-equivalent
BRL and Pro quota impact are UNKNOWN, not zero.

**Next exact step:** continue the #91 acceptance audit with the installed
0.1.27 journey and this source build, identify the next high-impact gap
without manufacturing historical decisions for #92, then consolidate a
coherent alpha package only after that gap is addressed and its native
evidence is collected. Do not publish this single slice as a release.

### Desktop 0.1.28 installed candidate — 2026-09-27

The nonvisual-result path is now an end-to-end alpha building block: the
source-built native app passed the actual Windows PDF file-picker flow on an
isolated desktop, without opening or executing the PDF. The same source
build also resumed the existing 33-message real Codex conversation, opened
its actual HTML result, drafted a change request and survived a full
process restart without Send or file change. This extends #91 but does
not close #91/#92 or the larger Windows objective.

Desktop version is 0.1.28 in Cargo manifest/lock and Tauri config.
`RELEASE_NOTES-0.1.28.md` describes the scope and limits. At the package
boundary, focused `cargo check` and preview tests, all 50 Desktop crate
tests (one ignored), strict Desktop Clippy, eight frontend unit tests,
the full mocked-IPC browser suite, and `git diff --check` passed. An
additional `cargo fmt --check` did not pass because it requests formatting
across pre-existing, untouched Desktop Rust code; no crate-wide rewrite
was made for this alpha. This is formatting debt, not a test failure in
the new behavior.

One NSIS candidate was built with the pinned Forge core 0.13.2:
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.28_x64-setup.exe`,
123,102,085 bytes, SHA-256
`C8ED4DD0F4B87027FA3C66DCD2715E44C7CEA8D0390A6D58E107ED2B66DDC5B5`.
Its adjacent `.sha256` sidecar names the same file/hash. Silent
installation of this exact candidate over public 0.1.27 exited 0; the
candidate bytes remained unchanged. The installed 0.1.28 executable is
SHA-256 `F03422B1172DE05AE433BF9FEDD5FE6FFC5C40AAEE2CCF557C0751D0EB71750B`.
The installed app passed hidden native onboarding, actual PDF picker,
nonvisual card and read-only real-result/restart checks. No new provider
turn, external file launch, GitHub CI or core-workspace tests were run.
The previous combined two-dialog timeout remains without a known cause.

**State:** source edits and release notes are local, uncommitted; the
candidate is installed locally but not yet public. No subagent was used;
task-level model token attribution, API-equivalent BRL and Pro quota
impact remain UNKNOWN. **Next exact step:** selectively commit/push this
package with the maintainer Git identity, tag the exact code state, publish
the already tested installer and sidecar under the standing alpha
authorization, then download both unauthenticated and verify size/hash and
installed readback. Do not rebuild the candidate.

### Desktop 0.1.28 public release readback — 2026-09-27

Package commit `d1da280d` was pushed on `codex/desktop-shell` using the
maintainer identity. Annotated tag `desktop-v0.1.28-alpha.1` resolves to
that commit. Public prerelease:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.28-alpha.1`.
GitHub lists the exact installer and SHA-256 sidecar. An unauthenticated
download of both to `C:/ForgeFast/forge-alpha-0128-public-20260927`
matched the tested candidate: 123,102,085 bytes, SHA-256
`C8ED4DD0F4B87027FA3C66DCD2715E44C7CEA8D0390A6D58E107ED2B66DDC5B5`.
Silent installation of those downloaded bytes exited 0 and left them
unchanged. The installed executable reports 0.1.28 and SHA-256
`F03422B1172DE05AE433BF9FEDD5FE6FFC5C40AAEE2CCF557C0751D0EB71750B`.
The downloaded installation passed the hidden read-only real-result/restart
test again against the existing 33-message Codex chat, with no Send or file
edit. No manual GitHub CI was run. This proves public byte continuity and
same-machine update, not a clean-machine installation.

**State:** 0.1.28 is public and installed; working tree was clean before
this post-release checkpoint. Goal remains active: the broader user
journey, #91 and #92 are not declared complete. The combined two-dialog
timeout remains an unexplained historical observation. External-browser
launch, fresh-account login, clean-machine install, manual accessibility,
mobile and offline self-contained distribution remain NOT_RUN/unfinished.
No subagent was used; per-task BRL and subscription-quota impact are
UNKNOWN. **Next exact step:** inspect the next highest-impact Windows
journey gap in actual 0.1.28, especially whether a person can understand
the project record and resume without technical help. Do not invent
historical decision wording not present in Forge authority. Avoid another
alpha solely for copy or repeating the same provider turn without a
specific hypothesis.

### Desktop record recovery after Codex disconnect — 2026-09-27

Active objective remains the complete, approachable Windows journey. In the
source after public 0.1.28, a Codex turn hid the Forge record pending a fresh
read, but `disconnected` and `update_required` events did not trigger that
read. The record could remain hidden until a manual click. The event bridge
now re-reads the independent, authoritative Forge record after either event;
it does not assume progress, send a message, or change project files.

**Changed, uncommitted:** `ui/chat.mjs`, `tests/browser.cjs`,
`tests/native.cjs`, and this README. The browser regression failed before
the fix because no read occurred, then the complete mocked-IPC browser suite
passed. A source debug build and the hidden native Windows smoke passed with
controlled terminal events and real Rust-to-Forge record reads. The native
test did not send a real Codex turn; actual unexpected provider disconnect
and update-required incidents remain NOT_RUN. No Rust source changed, so no
crate or workspace tests were repeated. `git diff --check` passed. The
installed/public 0.1.28 still does not include this fix. No subagent was
used; task-level tokens, API-equivalent BRL, and subscription-quota impact
remain UNKNOWN.

**Next exact step:** inspect the remaining project-record/resume journey for
one higher-impact issue that can be verified without inventing missing Forge
history. Then review the integrated diff, run appropriate focused and native
checks, and package a coherent next alpha rather than publishing this small
event fix alone. Keep the active goal open.

### Desktop 0.1.29 installed recovery candidate — 2026-09-28

The second concrete journey gap was a stale preview after an abnormal Codex
turn: an interrupted or failed turn can leave file changes on disk, but the
opened file was re-read only after normal completion. The UI now calls the
existing project-bound preview read after all terminal events, without
inferring a new result or retrying a failed preview. An enlarged preview
still waits for the reader to close it. This joins the record-recovery fix
above as one recovery package; no Forge core, conversation protocol, or
project-state implementation changed.

**PASS:** the browser test reproduced the stale preview before the fix,
then the complete mocked-IPC browser suite passed. A source and installed
hidden native Windows smoke used controlled Codex terminal events to re-read
an actually changed local file and the existing authoritative Forge record.
Desktop `cargo check`, all 50 regular crate tests (one ignored optional
browser association test), strict Clippy, eight frontend unit tests, JS
syntax checks and `git diff --check` passed. The installed candidate also
resumed the existing real 33-message Codex chat, showed its actual HTML
result, drafted a change request and survived a full process restart with
no Send or file edit. No new real Codex turn or GitHub CI was run. Actual
unexpected provider failure/disconnection remains NOT_RUN.

Desktop version is 0.1.29 in Cargo manifest/lock and Tauri config. The
single NSIS candidate with pinned Forge core 0.13.2 is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.29_x64-setup.exe`,
123,071,538 bytes, SHA-256
`299CCC8718241168318E9BCFA98A6AAC187573A01EEF44EA3FB3711F45DD14A1`.
The adjacent sidecar matches. Silent installation over public 0.1.28
returned exit 0 without changing the candidate bytes. Installed 0.1.29
executable SHA-256 is
`EF66C040BF1B4E86181F121388EE5A817EE2AF59CA356EA5F2EAECF8791A0959`.
`RELEASE_NOTES-0.1.29.md` states scope and limits. The package is installed
locally but **not committed, pushed or published**. No subagent was used;
task-level model tokens, API-equivalent BRL and Pro-quota impact remain
UNKNOWN. The older combined two-dialog timeout, external-browser launch,
fresh-account login completion, clean-machine setup, manual accessibility,
mobile and offline self-contained distribution remain open or NOT_RUN.

**Next exact step:** review the integrated diff, selectively commit/push
with the maintainer identity, tag the exact code state, publish the tested
installer and sidecar under the standing alpha authorization, then download
both unauthenticated and verify exact bytes and installed readback. Do not
rebuild the candidate. Keep the larger Windows journey goal active.

### Desktop 0.1.29 public release readback — 2026-09-28

Package commit `4392865c` was pushed on `codex/desktop-shell` with the
maintainer identity. Annotated tag `desktop-v0.1.29-alpha.1` resolves to
that commit. Public prerelease:
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.29-alpha.1`.
GitHub lists the exact installer and SHA-256 sidecar. Unauthenticated
downloads to `C:/ForgeFast/forge-alpha-0129-public-20260928` matched the
tested candidate: 123,071,538 bytes, SHA-256
`299CCC8718241168318E9BCFA98A6AAC187573A01EEF44EA3FB3711F45DD14A1`,
and matching sidecar text. Silent installation of those downloaded bytes
returned exit 0 and left the file unchanged. The installed executable
reports 0.1.29 and SHA-256
`EF66C040BF1B4E86181F121388EE5A817EE2AF59CA356EA5F2EAECF8791A0959`.
The downloaded installation again passed a hidden read-only real-result
and full-process-restart check against the existing 33-message Codex chat,
with no Send or file edit. No manual GitHub CI was run. This proves public
byte continuity and same-machine update, not a clean-machine installation
or real provider error recovery.

**State:** 0.1.29 is public and installed. The broad Windows journey goal
remains active; #91/#92 have not been declared complete. The earlier
combined two-dialog timeout, positive external-browser launch, fresh-account
sign-in completion, clean-machine install and manual accessibility acceptance
remain unresolved or NOT_RUN. Mobile and offline self-contained distribution
are later stages. No subagent was used in this package; per-task model
tokens, API-equivalent BRL and Pro-quota impact remain UNKNOWN.
**Next exact step:** audit remaining #91/#92 acceptance against the installed
0.1.29 journey and prioritize the largest truthful user-facing gap. In
particular, do not treat suggested decisions as agreements or fabricate
missing historical wording. Avoid another release solely for copy or a
repeat of the same real Codex turn without a specific new hypothesis.

### Desktop #91/#92 authority audit — 2026-09-28

The broad Windows journey goal remains active. This is a read-only acceptance
audit after public 0.1.29, not a new product release or a claim that either
issue is finished. The worktree was clean at `71e7c843` before this note.

**#91 (result and changes): PARTIAL.** Public 0.1.29 can reopen the real
33-message Codex chat, render its changed local HTML result in a protected
preview, prepare a change request in the same composer and retain that draft
through a full process restart. A real local PDF can be selected and shown as
a nonvisual file card without opening its bytes. Native tests cover an
out-of-project rejection, unavailable and updated previews, blocked scripts
and external requests. These are concrete installed-app checks, but the
positive external-browser launch and a real Codex reply citing a nonvisual
file remain NOT_RUN; do not close #91 on the existing evidence alone.

**#92 (decisions and stages): PARTIAL.** The UI derives the current stage and
activity from `workflow resume`, and reads accepted objective text plus
revision origin/history from `workflow report`. It separates pending-record
counts from suggested questions; suggestions are not presented as agreements.
The core's `DecisionNeedRaisedEvent` retains a `question_digest`, not the
question text. Its `DecisionResolvedEvent` retains the selected alternative
reference, not readable choice wording. Replacement `decision_history` and
resume `human_decisions.recovered_pending` project those IDs/statuses only.
The desktop therefore cannot truthfully display a readable list of accepted
decision wording from the current authoritative interface. Do not recover
wording from chat by inference or add desktop-owned decision state. Current
installed-core 0.13.2 read-only check against the real 33-message fixture
returned objective revision 1, one objective-history entry, zero durable
pending decisions, zero decision-history entries and completed current work;
that fixture cannot prove pending/resolved/revised decision presentation.
The Solo Cooperative `DecisionRequired` objective branch is intentionally
read-only and its kernel test asserts unchanged ledger bytes. Thus its
question is not a recovered durable decision at all; presenting it as one
would be a product error, not merely a missing UI field.

**Next exact step:** leave #92 explicitly partial rather than adding a desktop
decision database or silently making the read-only Solo branch durable. Audit
the next directly testable Windows journey gap in #91, starting with the
positive external-browser action only in an isolated environment where the
default browser cannot forward the request to the maintainer's active desktop;
the existing hidden WebView alone does not guarantee that. Fix a reproduced
failure, not an assumed one. If full accepted-decision history is
later prioritized, design that as an explicit core-authority contract change
with provenance and migration, not a copy-only Desktop patch. Use focused
Rust checks only for affected crates and hidden native UI tests; no release
solely for this checkpoint. No agent was delegated; task-level tokens,
API-equivalent BRL and subscription-quota impact remain UNKNOWN.

### Desktop preview read/turn overlap — 2026-09-28

Active objective remains the complete, approachable Windows journey. A
concrete #91 gap was reproduced: when a Codex terminal event arrived while
the selected-file preview read was still in flight, the UI discarded that
event because `pending` was true and the visible result was temporarily
hidden. The old read could then display stale file bytes. This is distinct
from the already-released 0.1.29 fix for turns that end after a preview read.

**Source-only, uncommitted:** `ui/preview.mjs` now queues one fresh native
project-bound read when a turn stops during an in-flight preview read. It
coalesces terminal events and re-reads only after a successful first read;
an unavailable or invalid file is not retried automatically. Switching
projects clears the queue. `tests/browser.cjs` reproduces the exact ordering
with a held stale read: it failed before the fix and passed after it.
`tests/native.cjs` exercises the same ordering in the hidden Windows WebView
using a real Forge-onboarded project file and real Rust preview reads, with
controlled Codex terminal events and no Send.

**PASS:** full mocked-IPC browser suite, including the negative case where a
failed in-flight file read is not retried and the reader can retry manually;
the browser suite also verifies that a queued refresh from one project cannot
leak into the next project after a switch;
incremental offline Desktop debug
build; hidden native Windows smoke including the new real-file overlap check,
normal and abnormal terminal refreshes, onboarding, protected HTML preview,
nonvisual file card, and project-bound rejection. Native output explicitly
marked the real Codex conversation NOT_RUN in this smoke. No Rust source
changed, so focused Rust crate tests were not repeated; no installer was
built or published. The positive external-browser action still cannot be
safely tested against the maintainer's default browser from only a hidden
WebView, because the browser may forward the request to the active desktop.
No subagent was used; per-task tokens, API-equivalent BRL and Pro quota
impact remain UNKNOWN.

**Next exact step:** inspect the integrated diff, then investigate one more
meaningful user-facing recovery gap before consolidating a coherent alpha
package. Do not package this race fix alone or call #91/#92 complete.

### Desktop 0.1.30 candidate checkpoint — 2026-09-28

The active objective remains the complete, approachable Windows journey. A
second concrete #91 race was reproduced: a turn could stop while the file
picker was open; canceling it preserved a stale selected-file preview.
`ui/preview.mjs` now refreshes that same file after cancellation, while a
new selection receives its normal read. This is packaged with the earlier
in-flight preview-read fix. No desktop-owned result or decision state was
added. The edited source is `ui/preview.mjs`, `tests/browser.cjs`,
`tests/native.cjs`, the three Desktop version files, and
`RELEASE_NOTES-0.1.30.md`.

**PASS:** both browser regressions failed before their respective fixes and
passed afterward; the complete browser suite, eight frontend unit tests,
JavaScript syntax checks, Desktop `cargo check -p forge-desktop`, five focused
preview tests, all 50 non-ignored Desktop tests, strict Desktop Clippy, and
hidden source-native smoke passed. The hidden native test used real project
files and Rust reads with controlled Codex terminal events. A separate
read-only run reopened an existing real 33-message Codex conversation,
previewed its actual HTML, prepared a change request, and retained that draft
through a full process restart; no Send or file edit occurred. The separate
core workspace and manual GitHub CI were not rerun for these Desktop changes.

One installer candidate was built and silently installed over public 0.1.29:
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.30_x64-setup.exe`,
123,098,780 bytes, SHA-256
`204A103BB2D78E82816DD6A93A6E7DEC0E7FD6048681C79CEBF1E8EAA010B92D`.
The candidate hash remained unchanged; installed `forge-desktop.exe` reports
0.1.30 and SHA-256
`C1F01E9E3612DF2D8B14EA0C669073E8F26FFA3F51952AF0685CBA0B6CB17F79`.
The installed candidate passed hidden native smoke and the real read-only
conversation/restart run. This is same-machine installed proof, not public
download or clean-machine proof. Real provider behavior in the precise race
windows, positive default-browser launch, fresh-account login, manual screen
reader acceptance, and full #91/#92 acceptance remain NOT_RUN or PARTIAL.
No subagent was used in this package. Model-attributed tokens, API-equivalent
BRL and subscription-quota impact remain UNKNOWN.

**Next exact step:** selectively commit and push this reviewed 0.1.30 package,
tag the exact code, publish the already tested installer and matching sidecar
as an alpha prerelease, then download the public bytes unauthenticated,
compare hashes, and test installation/readback without rebuilding the
candidate. Keep the broad journey goal active.

### Desktop 0.1.30 public readback — 2026-09-28

Code commit `4f50e5b3` was pushed on `codex/desktop-shell` and tagged
`desktop-v0.1.30-alpha.1`. The tested installer and matching sidecar were
published as the [0.1.30 alpha prerelease](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.30-alpha.1).
No installer rebuild or manual GitHub CI run occurred after local tests.

An unauthenticated download of both public assets returned a 123,098,780-byte
installer with SHA-256
`204A103BB2D78E82816DD6A93A6E7DEC0E7FD6048681C79CEBF1E8EAA010B92D`,
matching the sidecar and the originally tested candidate. Silent installation
of the downloaded bytes returned exit 0; the file stayed unchanged and the
installed 0.1.30 executable hash matched
`C1F01E9E3612DF2D8B14EA0C669073E8F26FFA3F51952AF0685CBA0B6CB17F79`.
The downloaded installation passed the hidden read-only real 33-message
conversation/restart check and the full hidden native smoke, including the
two preview overlap tests. Real Codex Send was NOT_RUN in these public-byte
checks. This proves same-machine public-byte continuity and installed readback,
not clean-machine setup or the remaining #91/#92 acceptance. No subagent was
used; model-attributed tokens, API-equivalent BRL and subscription-quota
impact remain UNKNOWN.

**State:** 0.1.30 is public and installed. The broad Windows journey goal
remains active. Next, prioritize a remaining #91 user-facing gap with a
safe, specific native test hypothesis, then tackle the core-authority design
needed for readable decision history in #92 only if the product genuinely
needs it. Do not invent missing decision wording in the Desktop UI.

### Bounded installed-UI audit after 0.1.30 — 2026-09-28

The active journey goal is not complete. The installed 0.1.30 app was inspected
headlessly with the existing real 33-message Codex result and its actual local
HTML. Screenshots are `D:/Temp/User/forge-0130-real-preview.png` and
`D:/Temp/User/forge-0130-change-request.png`; the read-only native test
passed, including full process restart and unchanged project file. The
installed app's project-record view was also captured at
`D:/Temp/User/forge-0130-record-collapsed.png` and
`D:/Temp/User/forge-0130-record-full.png`; hidden native smoke passed. The
screens show a visible same-conversation change action, an explicit local/not-
published distinction, and the real Forge stage rather than invented progress.
No new visual defect was established by this audit, so no speculative UI edit
was made. The focused ignored Windows test for resolving a configured HTTPS
default-browser executable passed, but it deliberately does **not** launch
the browser; positive external launch remains NOT_RUN to avoid disturbing
the maintainer's active desktop. No real Codex Send occurred in this audit.

**Next exact step:** stop repeating read-only visual checks. Examine #92's
accepted-decision requirement against the core's existing durable decision
contract and decide a minimal authority-preserving path before changing UI.
If no readable decision text is durably available, do not fabricate it from
chat or create desktop-owned state. No subagent, model-attributed task tokens,
API-equivalent BRL or Pro-quota attribution was available; costs are UNKNOWN.

### #92 decision-authority boundary — 2026-09-28

Read-only contract inspection confirmed that the current decision audit cannot
provide a plain-language accepted-decision list. `DecisionNeedRaisedEvent`
stores `question_digest`, not the question or its alternatives;
`DecisionResolvedEvent` stores `selected_alternative_ref`, not its wording.
`WorkflowReplacementDecisionAudit` projects policy/decision references,
status, record digests and the selected alternative reference. Searching the
Rust source found `DecisionNeedRaisedEvent` construction only in test fixtures;
the event can also enter through the external typed governance interface, but
the current Solo Cooperative path does not synthesize a readable accepted
decision from a suggested question. The Desktop correctly renders the
accepted objective revisions and separates suggested questions from durable
pending counts, but it cannot satisfy #92's pending/accepted/revised decision
wording criterion by copy changes alone. This is a backend contract boundary,
not an invitation to infer agreement from chat. No source code, native app or
installed release was changed in this read-only audit.

Further contract inspection refined that conclusion: each decision ledger
record has a `bundle_digest`, and the typed policy's `WorkflowDecisionRule`
contains the question and alternative descriptions. A core-owned readback may
be able to resolve readable wording **conditionally** by loading the exact
historical bundle, checking the question against `question_digest`, and
matching the selected alternative reference. Current bundle wording alone
must never be attached to an older receipt after a policy/domain-pack change.
This has not been implemented or verified; historical bundle availability and
Solo decision recording still need investigation. It is therefore premature
to require a new ledger event field or to claim the current UI can show the
wording.

**Next exact step:** determine whether the exact historical policy bundle is
retained and readable for each decision receipt. Only then choose between a
verified read-only join and a minimal new core-owned decision record, with
origin, acceptance and revision semantics. Do not modify the UI as though
this data already exists. The broad Windows journey goal remains active;
costs remain UNKNOWN.

### Desktop expanded-preview navigation refresh — 2026-09-28

The broad Windows journey goal remains active. A concrete #91 stale-result
path was reproduced: if a Codex turn stopped while an enlarged preview was
open, changing app route before closing that dialog cleared its queued
refresh. Returning to the conversation could show the old file contents.

**Source-only, uncommitted:** `ui/preview.mjs` now lets the normal dialog
close handler honor the queued refresh on route change instead of discarding
it. `tests/browser.cjs` reproduced the path RED before the fix and passed
after it; `tests/native.cjs` exercised the route change against a real
Forge-onboarded project file and native Rust preview read. The full browser
suite, hidden source-built native smoke, JavaScript syntax checks and
`git diff --check` passed. Controlled Codex terminal events were used; a
real provider turn in this precise sequence was NOT_RUN. No Rust source
changed, so Rust crate tests were not rerun; no installer was built or
published. Earlier checkpoint-only edits to this README are still local.
No subagent was used; model-attributed tokens, API-equivalent BRL and Pro
quota impact remain UNKNOWN.

**Next exact step:** review this small integrated diff and decide whether it
belongs with another concrete #91 recovery fix in a later alpha package.
Do not repeat read-only visual checks or present this source-only fix as
installed. If the maintainer asks to pause the active goal, pause it and
stop work.

### Expanded-preview fix review — 2026-09-28

The integrated diff was reviewed after the native and browser checks. The
change is limited to the route-change handler: it no longer discards a queued
file refresh before the dialog's existing close handler can perform it.
Project changes still clear that queue in `setPreviewProject`, so a refresh
cannot carry over to a different project. The browser regression and hidden
native test both cover leaving the expanded preview after a controlled turn
completion, returning to the workspace, and seeing the changed project file.
`git diff --check` passed; Git reported only working-tree line-ending warnings.

**Decision:** keep the reviewed source-only fix and tests for a later coherent
alpha package. It is not installed or published, and a real Codex provider
turn in this precise timing remains NOT_RUN. No further UI change is justified
by this review alone. The next bounded product step should target one
remaining end-to-end journey gap with a reproducible user-visible failure,
not another broad audit. No commit, push, build or installer was made here.

### #92 historical decision wording feasibility — 2026-09-28

The closed kernel release registry keeps its admitted core bundle documents,
including earlier releases. The Domain Pack lifecycle writes generation
material as immutable files and its raw inventory walks the historical ledger
chain, checking each reachable generation manifest. However, the exposed
admitted-generation view loads only the **active** generation; there is no
existing kernel read API that reconstructs the exact historical effective
bundle for an arbitrary workflow decision record. A desktop-side join against
the current bundle would therefore be unsafe after a policy or Domain Pack
change.

There is a second binding limit: `DecisionNeedRaisedEvent.question_digest`
contains a digest rather than question text, and the existing audit projection
does not prove that it equals the historical policy rule's question. The
kernel's own ledger fixture uses a well-formed synthetic digest instead of a
digest derived from the rule wording. A future read-only resolver must return
wording only after it can verify the exact historical effective bundle **and**
the question digest and selected alternative reference; otherwise it must
leave wording unavailable rather than guess. This inspection did not change
contracts, runtime code or the installed app, and did not establish that Solo
Cooperative currently records every human product decision in this ledger.

**Next bounded step:** define and test the core-owned readback contract with
explicit unavailable cases before any Desktop wording change. Do not build
another Desktop-owned decision store. No Codex turn, Rust build, commit or
publication was performed; task-level cost attribution remains UNKNOWN.

### Desktop conversation-first package in progress — 2026-09-28

The maintainer directed work back to a complete, friendly, attractive UI
connected to the existing backend rather than further speculative backend
investigation. The active source package now includes the earlier expanded-
preview route-refresh fix plus two conversation improvements: the composer
places writing and **Enviar** together near the approved visual reference;
before a project is confirmed, a drafted idea instead offers **Escolher pasta
para continuar**. That action opens the existing folder picker, keeps the
draft, and does not start a project or send a message. Selecting a folder
still requires the existing explicit **Continuar nesta pasta** confirmation.
First-use guidance now allows writing and choosing a folder in either order.

Browser regression reproduced the previous composer separation and passed
after the layout fix. The full browser UI suite passed after the first-use
flow change. A source-built Windows app passed hidden native smoke with real
Forge onboarding, project-record readback and controlled Codex terminal
events. Native screenshots of first-use and workspace were captured under
`D:/Temp/User/forge-new-*-20260928.png`; the first-use screenshot was reviewed.
JavaScript syntax checks and `git diff --check` passed before the version bump.
Real Codex Send in this precise new first-use sequence is NOT_RUN; existing
provider-send evidence is separate. No Rust source changed; the code is not
yet installed or published. The Desktop version is bumped in source to
0.1.31 for one candidate after package validation. No subagent was used;
model-attributed tokens, API-equivalent BRL and Pro-quota impact are UNKNOWN.

**Next exact step:** finish package validation, write 0.1.31 release notes,
build one NSIS candidate, install that exact file over public 0.1.30, then
publish and verify the downloaded bytes under the maintainer's standing
publication authorization. Do not repeatedly rebuild candidates or run
GitHub CI for each frontend edit. The broader UI/product journey remains
active after this alpha package.

### Desktop 0.1.31 installed candidate — 2026-09-28

The single NSIS candidate is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.31_x64-setup.exe`,
123,079,445 bytes, SHA-256
`A167B06A826B52D9FDCD544D5CA07D41F7356F81ADC2442CF868B2D171EED887`.
It was silently installed over public 0.1.30 (exit 0); its hash was unchanged.
The installed 0.1.31 executable reports 0.1.31 and has SHA-256
`49713D34D8EF62382B7A68AEEEF617DEF6160D36FEF18D82CCA3A288218F92CB`.

**PASS:** browser UI suite, eight frontend unit tests, JavaScript syntax,
Desktop `cargo check`, six focused preview tests (one optional association
test ignored), all 50 non-ignored Desktop tests, strict Desktop Clippy,
`git diff --check`, release build, and hidden native smoke on the installed
candidate. The hidden installed run used a real Windows folder dialog opened
from the draft and canceled without losing it; it also validated real Forge
onboarding and read-only project-file/record behavior. No manual GitHub CI or
separate core workspace run occurred. Real Codex Send in the precise new
first-use sequence remains NOT_RUN, as does a real provider turn in the
preview timing window. Clean-machine install and full-product UI acceptance
remain NOT_RUN/PARTIAL. No subagent was used; attributable per-model tokens,
BRL equivalent and Pro-quota impact remain UNKNOWN.

**Next exact step:** selectively commit and push the reviewed package, tag
the code, publish this same tested installer and matching hash sidecar as an
alpha prerelease, download both public files without authentication, compare
bytes and test the downloaded installer. Do not rebuild the candidate.

### Desktop 0.1.31 public readback — 2026-09-28

Code commit `da947c78` was pushed on `codex/desktop-shell` and tagged
`desktop-v0.1.31-alpha.1`. The tested installer and matching SHA-256 sidecar
were published as the [0.1.31 alpha prerelease](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.31-alpha.1).
No installer rebuild or manual GitHub CI run occurred after the local package
checks. An unauthenticated download of both assets returned a 123,079,445-byte
installer with SHA-256
`A167B06A826B52D9FDCD544D5CA07D41F7356F81ADC2442CF868B2D171EED887`,
matching the sidecar and the originally tested candidate. Installing those
downloaded bytes silently returned exit 0; the public file hash stayed
unchanged and the installed 0.1.31 executable retained SHA-256
`49713D34D8EF62382B7A68AEEEF617DEF6160D36FEF18D82CCA3A288218F92CB`.
The downloaded installation passed hidden native smoke, including the real
folder-dialog cancel through the new idea action, real Forge onboarding,
record readback, and controlled preview-refresh checks. This proves
same-machine public-byte continuity and installed readback, not a clean
machine or real Codex Send in the precise new flow. No subagent was used;
model-attributed tokens, API-equivalent BRL and Pro-quota impact remain
UNKNOWN.

**State and next product step:** 0.1.31 is public and installed. The broad
friendly-UI/connected-backend journey remains active; this alpha does not
close it. Continue with a concrete, high-impact UI coverage gap from the
existing backend and approved design, using native screenshots and a user
journey rather than revisiting #92 wording feasibility or other low-return
contract speculation first. Keep releases as coherent building blocks.

### Desktop conversation result follow-through — 2026-09-28

The broad friendly-UI objective is **not blocked** by an unresolved release
question. Current uncommitted work makes a completed Codex reply with exactly
one explicitly cited project file offer **Conferir arquivo citado** directly
beside that reply. This matters after a different preview is already loaded:
the old side-panel shortcut then disappears, but the new reply's action still
opens the new file. Several cited files remain separate choices; the UI never
guesses one. Raw-message mode keeps the action. Clicking uses the existing
native project-bound preview and does not send a message.
The interruption-resume notice is shorter while still saying that the last
reply is incomplete, prior changes may remain, and nothing was resent; the
native screenshot now leaves more room for the conversation.

**Changed files:** `ui/chat.mjs`, `ui/styles.css`, `tests/browser.cjs`,
`tests/native-result-shortcut.cjs`, and this checkpoint. Browser UI suite,
JavaScript syntax, `git diff --check`, debug Desktop build, and hidden native
result-shortcut/restart smoke passed. The native smoke read a real project file,
rejected an outside-project path, kept multiple citations distinct, and
prepared a change without sending. Visual readback was captured at
`D:/Temp/User/forge-inline-result-20260928.png` and
`D:/Temp/User/forge-result-open-20260928.png` (native preview). The app currently installed
for the user remains public 0.1.31; these source edits are **not** installed or
published. No Rust source changed; no workspace-wide Rust suite or GitHub CI
was run. A real Codex response in this exact new UI path is NOT_RUN. No
subagent was used; model-attributed tokens, BRL equivalent and Pro-quota impact
remain UNKNOWN.

**Next exact step:** continue a substantial UI journey gap from the approved
design, integrating this change into a coherent next alpha package rather than
publishing it alone. Keep the native run hidden. At the package boundary,
review the combined diff, run focused checks then final applicable gates,
build/install/test one candidate and publish under the maintainer's standing
authorization. Do not mark the broad goal blocked for a minor NOT_RUN path.

### Desktop 0.1.32 installed candidate — 2026-09-28

The conversation now offers an inline list when a completed Codex reply cites
several distinct project files. The user chooses an explicit file; the app
does not guess, send a turn, or bypass the existing project-bound native
preview. The one-file shortcut, raw-message mode, reopened conversation,
keyboard navigation and already-open preview are covered by browser tests.
The same two-file choice and process restart passed hidden native smoke on
the installed app. A read-only attempt against a pre-existing real 33-message
Codex thread identified three cited files, but the subsequent provider resume
timed out before reaching the new chooser. That exact real-thread validation
is **NOT_RUN**, not a product failure or a pass. No real Send occurred.

The single NSIS candidate is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.32_x64-setup.exe`,
123,103,870 bytes, SHA-256
`8FB546E59839952143148D4A4C27C38C747EAB6DE522CDB91BBB36BB86D6522E`.
Silent installation over public 0.1.31 returned exit 0; the candidate hash
stayed unchanged. The installed 0.1.32 executable has SHA-256
`4E5F6C9A21F041978641786E9DFBFF62F2C5FE8B8EB0CBC1ED849256939F08D8`.
The installed app passed hidden focused result/restart smoke and the full
hidden native smoke, including a real Windows folder choice and real Forge
onboarding. Browser UI suite, frontend tests, Desktop `cargo check`, all 50
non-ignored Desktop tests, strict Desktop Clippy, NSIS release build and
`git diff --check` passed. The separate core workspace and manual GitHub CI
were not run for this frontend package. Clean-machine setup, fresh-account
login, manual accessibility acceptance and real Codex Send in this precise
flow remain NOT_RUN/PARTIAL. No subagent was used; attributable per-model
tokens, BRL equivalent and Pro-quota impact remain UNKNOWN.

**Next exact step:** review the combined diff, selectively commit and push,
tag the source, publish the same tested candidate and SHA-256 sidecar as an
alpha prerelease, then download both assets without authentication, verify
the bytes and reinstall/test the downloaded file. Do not rebuild or run
manual GitHub CI for this frontend package. Continue the broader UI journey
after the package; the goal is not blocked by the real-provider timeout.

### Desktop 0.1.32 public readback — 2026-09-28

Code commit `7b41137e` was pushed on `codex/desktop-shell` and tagged
`desktop-v0.1.32-alpha.1`. The same tested candidate and SHA-256 sidecar were
published as the [0.1.32 alpha prerelease](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.32-alpha.1).
An unauthenticated download of both assets returned a 123,103,870-byte
installer with SHA-256
`8FB546E59839952143148D4A4C27C38C747EAB6DE522CDB91BBB36BB86D6522E`,
matching the sidecar and tested candidate. Installing the downloaded bytes
silently returned exit 0; their hash remained unchanged, and the installed
executable retained SHA-256
`4E5F6C9A21F041978641786E9DFBFF62F2C5FE8B8EB0CBC1ED849256939F08D8`.
The downloaded installation passed hidden native result/restart smoke,
including distinct choices for two restored citations, outside-project
rejection and no Send. The installed candidate had already passed the full
hidden native folder/onboarding smoke. No rebuild or manual GitHub CI run
occurred after package validation. This is same-machine public-byte and
installed-app proof, not clean-machine setup or a real Codex Send in this
exact result-flow. No subagent was used; attributable per-model tokens,
API-equivalent BRL and Pro-quota impact remain UNKNOWN.

**Next product step:** continue the broader connected, attractive UI journey
from a concrete remaining gap and the approved design. Do not treat this
alpha release as completion of the Forge Desktop goal.

### Narrow workspace navigation in source — 2026-09-28

After 0.1.32 public readback, the next uncommitted UI slice adds direct
**Conversa**, **Prévia** and **Andamento** choices for confirmed projects at
700px or narrower. They show existing panels rather than copying project or
conversation state; folder controls remain with Andamento. Opening a cited
file switches to Prévia; asking for a file change or a Forge-record explanation
returns to the same conversation and focuses the unsent draft. At wider widths,
all existing panels remain visible. This is a bounded responsive-shell part of
#94, not a claim that a remote mobile client or full mobile acceptance exists.

**Changed, uncommitted:** `ui/mobile-workspace.mjs`, `ui/index.html`,
`ui/styles.css`, `ui/preview.mjs`, `ui/progress.mjs`,
`tests/browser.cjs`, `tests/native.cjs`, `tests/native-auth.cjs`,
`tests/artifact-resume.cjs`, `design/README.md` and this checkpoint.
The browser UI suite passed, including state preservation, mobile result-open,
change-request and record-explanation returns, desktop resize, 200% narrow
text without document overflow, and keyboard activation of panel choices.
The panel choices retain keyboard focus; programmatically opening a cited
result can focus the preview heading. JavaScript
syntax and `git diff --check` passed. An offline Desktop debug build passed;
hidden native smoke on that debug build passed with real folder onboarding,
real Forge record readback, 390px panel switching and the existing controlled
preview/conversation checks. The native narrow screenshots were reviewed at
`D:/Temp/User/forge-mobile-workspace-next.png`,
`D:/Temp/User/forge-mobile-progress-revised.png` and
`D:/Temp/User/forge-mobile-preview-next.png`. This review caught a desktop-
only phrase ("conversation beside") and a duplicate phase badge at narrow
width; both were corrected, and the browser suite, offline debug build and
hidden native smoke passed again. No Rust source changed, no
core workspace suite or manual GitHub CI ran, and the installed/public app is
still 0.1.32 without these changes. A real Codex Send in this new narrow view,
mobile-device access and manual screen-reader acceptance remain NOT_RUN. No
subagent was used; attributable per-model tokens, BRL equivalent and Pro-quota
impact remain UNKNOWN.

**Next exact step:** assess the remaining #94 journey gap, especially manual
screen-reader focus and a narrow project with a loaded result; then choose a
coherent 0.1.33 package and validate its exact candidate before publication.
Do not claim the installed app has these source edits yet.

### Desktop 0.1.33 installed candidate — 2026-09-28

The narrow workspace package now also has native visual proof with a loaded
real local HTML file at `D:/Temp/User/forge-mobile-loaded-preview-next.png`.
At 390px, the isolated page, **Pedir mudança neste arquivo**, file controls
and safe-preview notice remained readable with no document overflow. This is
not evidence of interactive site behavior or mobile-device distribution.
The responsive changes, tests and wording above were reviewed as one package.

The single NSIS candidate is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.33_x64-setup.exe`,
123,105,178 bytes, SHA-256
`90D94DE8984F92BE810999A1B06F6EC523D60061490B28BF58288AC9959AE80B`.
Silent installation over public 0.1.32 returned exit 0; the candidate hash
stayed unchanged. The installed 0.1.33 executable reports 0.1.33 and has
SHA-256
`A9B072C9924EE8E55B9596810E69AF19D5BE43384ACD18A22B476390BDFFA188`.
Installed-app hidden native smoke passed with real Windows folder choice,
Forge onboarding and record readback, 390px panel switching, isolated local
HTML preview and the existing controlled conversation/preview paths.
The browser UI suite, JavaScript syntax, `git diff --check`, Desktop Cargo
check, all 50 non-ignored Desktop tests, strict Desktop Clippy and one NSIS
release build passed. The separate core workspace and manual GitHub CI were
not run for this UI package. Real Codex Send in this exact narrow flow,
clean-machine setup, mobile-device access and manual screen-reader acceptance
remain NOT_RUN/PARTIAL. No subagent was used; attributable per-model tokens,
BRL equivalent and Pro-quota impact remain UNKNOWN.

**Next exact step:** selectively commit and push, tag the code, publish this
same candidate plus SHA-256 sidecar as an alpha prerelease, then download both
assets without authentication, compare bytes and install/test the public file.
Do not rebuild the candidate. Continue the broader Forge UI goal afterward.

### Desktop 0.1.33 public readback — 2026-09-28

Code commit `1d61a822` was pushed on `codex/desktop-shell` and tagged
`desktop-v0.1.33-alpha.1`. The single tested candidate and its SHA-256 sidecar
were published as the [0.1.33 alpha prerelease](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.33-alpha.1).
Unauthenticated downloads returned a 123,105,178-byte installer with SHA-256
`90D94DE8984F92BE810999A1B06F6EC523D60061490B28BF58288AC9959AE80B`,
matching both the sidecar and the candidate. Installing the downloaded bytes
silently over the installed candidate returned exit 0, did not change the
downloaded hash and produced installed executable SHA-256
`A9B072C9924EE8E55B9596810E69AF19D5BE43384ACD18A22B476390BDFFA188`.
The downloaded installation passed the full hidden native smoke with real
folder selection, onboarding and Forge record readback, narrow workspace
switching, isolated local preview and controlled conversation events. Actual
Codex Send remains NOT_RUN in this exact flow; clean-machine setup, mobile
device access and manual screen-reader acceptance are also NOT_RUN/PARTIAL.
No rebuild or manual GitHub CI occurred after candidate validation. No
subagent was used; attributable per-model tokens, API-equivalent BRL and
Pro-quota impact remain UNKNOWN.

**Next product step:** continue the connected UI journey from a concrete
remaining gap in the approved visual direction. Do not treat this alpha
package as completion of the Forge Desktop goal.

### Desktop 0.1.33 real narrow journey — 2026-09-28

The installed public 0.1.33 binary was run on an isolated hidden Windows
desktop at 390 x 844 against a disposable project. The opt-in native test now
selects the actual narrow conversation pane, sends a short real Codex turn,
creates `site/index.html` with CSS using a second real turn, opens that local
result through its direct file action or individual cited-file choice, requests
a change through the same conversation, verifies the changed file and refreshed
isolated preview, then reloads the WebView and restarts the app process. The
final full journey passed without resending on resume. The real generated
preview was visually reviewed at
`D:/Temp/User/forge-0133-real-narrow-result.png`; it is readable, but its
content is a disposable agent-generated test page, not approved Forge art.

Only `tests/native.cjs` changed to cover this path and handle panels hidden by
the narrow navigation. JavaScript syntax and `git diff --check` passed; no Rust
source, package bytes or product UI changed, so no new installer was built.
Early retries exposed test assumptions about which pane was visible, whether
the agent returned a Markdown link rather than multiple cited files, and
whether a hidden project-status node could be awaited as visible. Those were
test-harness mismatches and were corrected. One initial real-reply wait timed
out after 180 seconds; its cause was not established, although later real
turns and the full journey passed. This is same-machine proof for one
disposable project, not a guarantee for all provider sessions. Mobile-device
access, manual screen-reader acceptance and clean-machine setup remain
NOT_RUN/PARTIAL. No subagent was used; model-attributed tokens, API-equivalent
BRL and Pro-quota impact remain UNKNOWN. The repeated real-agent probes were
not free and are included as rework, not hidden as one successful attempt.

**Next product step:** work on a remaining user-visible gap rather than
repeating the now-proven real-Codex narrow journey. Keep the Forge Desktop
goal active; no new release is needed for this test-only change.

### Desktop narrow enlarged-text check — 2026-09-28

The installed public 0.1.33 binary was checked again on an isolated hidden
Windows desktop at 390 x 844 with the root text enlarged to 36px. In a
confirmed disposable project, a long unsent draft remained in the composer,
the document had no horizontal overflow, and keyboard Tab from the composer
focused a fully visible **Enviar** target at least 44px in both dimensions.
The native screenshot at `D:/Temp/User/forge-0133-narrow-zoom.png` was reviewed:
text wraps without clipping, although the always-visible Codex access warning
and long draft require vertical scrolling at this large size. No Send was made
and no model usage was incurred by this check. The focused hidden native smoke
and JavaScript syntax passed. This is Windows WebView zoom evidence, not an
actual mobile-device virtual-keyboard, safe-area or screen-reader test; those
parts of #95 remain NOT_RUN. No product CSS/Rust change or new installer was
needed because this check found no lost control or clipping. The only changed
source is the optional native zoom assertion in `tests/native.cjs`; no
subagent was used and per-model cost attribution remains UNKNOWN.

**Next product step:** inspect a different remaining user-visible journey or
accessibility gap; do not turn this passing zoom probe into a claim that #95 or
mobile access is complete.

### Desktop 0.1.34 candidate preparation — 2026-09-28

The active Forge Desktop goal continues. This UI-only package makes two
concrete journeys clearer: short newly completed cited-file lists open for
individual preview selection (without guessing one result), and an absent
Forge record offers **Conversar sobre meu projeto** into the same unsent
composer. Restored/long file lists remain folded, and a manual fold persists.
The redundant absent-state badge is hidden; native Forge record/status remains
the authority. The bundled core stays intentionally pinned to 0.13.2.

Files changed: `ui/chat.mjs`, `ui/index.html`, `ui/progress.mjs`,
`ui/styles.css`, `tests/browser.cjs`, `tests/native.cjs`, Desktop version files,
`design/README.md`, this checkpoint and `RELEASE_NOTES-0.1.34.md`. No subagent
is active. JavaScript syntax, Desktop focused `cargo check`, all 50 non-ignored
Desktop tests and browser UI suite passed; the 0.1.33 debug hidden native smoke
also passed with the new UI source, including real onboarding, narrow record
shortcut and no Send. Actual Codex Send was not repeated for this UI-only slice;
the previous public 0.1.33 real journey remains the evidence for that path.
Attributable model tokens, API-equivalent BRL and Pro-quota impact remain
UNKNOWN; no savings claim is made.

Strict Desktop Clippy passed. The single NSIS candidate is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.34_x64-setup.exe`,
123,142,636 bytes, SHA-256
`D905C644CF8ECA72C3EC2B6053F51E9567DF06E889643FC0010DC2DA26692E22`.
Silent installation over public 0.1.33 returned exit 0 without changing the
candidate bytes. The installed 0.1.34 executable has SHA-256
`9881933D91ACEB74695974EB0081EC94426B4400A0BBF1DA970BAD82E807DB60`.
Installed-candidate hidden native smoke passed with actual Windows folder
selection, new-project Forge onboarding and record readback, narrow record
shortcut, isolated local preview and controlled conversation events. Actual
Codex Send was NOT_RUN in this package; prior public 0.1.33 real-journey proof
is separate. Clean-machine/mobile-device/manual screen-reader acceptance
remain NOT_RUN/PARTIAL.

**Next exact step:** selectively commit/push/tag, publish this same candidate
under the user's standing alpha authorization, download it without
authentication, compare bytes, install the downloaded bytes and repeat hidden
native smoke. Do not rebuild or call 0.1.34 available before public readback.

### Desktop 0.1.34 public readback — 2026-09-28

Code commit `c7bb5d4a` was pushed on `codex/desktop-shell` and tagged
`desktop-v0.1.34-alpha.1`. The tested candidate and its SHA-256 sidecar were
published as the [0.1.34 alpha prerelease](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.34-alpha.1).
Unauthenticated downloads returned a 123,142,636-byte installer, SHA-256
`D905C644CF8ECA72C3EC2B6053F51E9567DF06E889643FC0010DC2DA26692E22`,
matching candidate and sidecar. Installing those downloaded bytes silently
over the installed candidate returned exit 0; the file remained unchanged and
the installed executable reports 0.1.34 with SHA-256
`9881933D91ACEB74695974EB0081EC94426B4400A0BBF1DA970BAD82E807DB60`.
The downloaded installation passed hidden native smoke with actual folder
selection, Forge onboarding and record readback, narrow record shortcut,
isolated local preview and controlled conversation events. The new cited-file
behavior passed browser fixtures; actual Codex Send was NOT_RUN in this
0.1.34 slice, while the separate public 0.1.33 real-Codex journey remains
documented above. Clean-machine setup, mobile-device use and manual
screen-reader acceptance remain NOT_RUN/PARTIAL. No subagent was used and
model-attributed tokens, API-equivalent BRL and Pro-quota impact remain
UNKNOWN. No candidate rebuild or manual GitHub CI run followed publication.

**Next product step:** continue the open UI coverage gap using the approved
visual references and actual user journeys; do not treat these alpha packages
as completion of the Forge Desktop objective.

### Desktop 0.1.35 narrow workspace package in progress — 2026-09-28

The active objective continues after the public 0.1.34 delivery. A real narrow
native capture showed that the Andamento view also appended the entire folder
and connection card, repeating the project name and lengthening the screen.
The existing mobile pane switch now offers a fourth **Projeto** choice over the
same original project panel. Andamento shows only the Forge record; Projeto
keeps folder switching and connection inspection one choice away. At 480px and
below the choices are a two-column grid, including 200% enlarged text. The
already-present workspace heading owns the visible project name in narrow
mode; the project card does not repeat it. The first-use screen and wide layout
remain unchanged. No new project store, route, backend command or agent state
was added.

Changed source: `ui/index.html`, `ui/mobile-workspace.mjs`, `ui/styles.css`,
`tests/browser.cjs`, `tests/native.cjs`, Desktop version files, design and
release notes. Browser suite PASS; an embedded-UI debug build passed hidden
native smoke with real project/record readback and folder-picker reachability.
Native screenshots reviewed:
`D:/Temp/User/forge-0135-progress-project-grid.png` and
`D:/Temp/User/forge-0135-project-pane-final.png`. The first native attempt
used the previous debug binary and could not find the new tab; rebuilding the
debug binary resolved this test-setup mismatch. One browser run also exposed
enlarged-text clipping on the new Andamento tab; CSS was corrected and the
suite passed. No real Codex turn was repeated for this UI-only slice. No
subagent is active; model-attributed tokens, API-equivalent BRL and Pro-quota
impact remain UNKNOWN.

Final Desktop `cargo check`, all 50 non-ignored Desktop tests, strict Clippy,
JavaScript syntax and `git diff --check` passed. The single NSIS candidate is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.35_x64-setup.exe`,
123,135,285 bytes, SHA-256
`B2D24C149529E5DE7FC77C47F10796DB2067F0E58240587B23E2B98652259384`.
Silent installation over public 0.1.34 returned exit 0 without changing the
candidate bytes; the installed 0.1.35 executable has SHA-256
`16FF5987A7138A17E612F87EF4BC512F27E7CFBD9568233F3C8C379FFFF349E7`.
Installed-candidate hidden native smoke passed with actual Windows folder
selection, new-project Forge onboarding/record readback, separate narrow
project controls and progress view, and isolated preview. Actual Codex Send
remains NOT_RUN for this UI-only package; prior real 0.1.33 evidence is
separate. Clean-machine/mobile-device/manual screen-reader acceptance remain
NOT_RUN/PARTIAL.

**Next exact step:** selectively commit/push/tag, publish this same candidate
under the standing alpha authorization, download unauthenticated, compare
bytes, install the downloaded file and repeat hidden native smoke. Do not
rebuild or call 0.1.35 available before public readback.

### Desktop 0.1.35 public readback — 2026-09-28

Code commit `d5509f10` was pushed on `codex/desktop-shell` and tagged
`desktop-v0.1.35-alpha.1`. The single tested candidate plus SHA-256 sidecar
were published as the [0.1.35 alpha prerelease](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.35-alpha.1).
Unauthenticated downloads returned a 123,135,285-byte installer, SHA-256
`B2D24C149529E5DE7FC77C47F10796DB2067F0E58240587B23E2B98652259384`,
matching candidate and sidecar. Installing those downloaded bytes silently
returned exit 0, left the downloaded file unchanged and produced installed
executable version 0.1.35, SHA-256
`16FF5987A7138A17E612F87EF4BC512F27E7CFBD9568233F3C8C379FFFF349E7`.
The downloaded installation passed hidden native smoke with actual Windows
folder selection, Forge onboarding and record readback, the separate narrow
Conversa/Prévia/Andamento/Projeto panes, preserved folder controls and isolated
preview. This proves the packaged same-machine Windows journey in scope, not
actual mobile-device use or a new real Codex turn; those remain NOT_RUN here.
Clean-machine setup and manual screen-reader acceptance remain NOT_RUN/PARTIAL.
No subagent was used; model-attributed tokens, API-equivalent BRL and Pro-quota
impact remain UNKNOWN. No candidate rebuild or manual GitHub CI run followed
publication.

**Next product step:** continue the broader Forge UI objective from a concrete
remaining user journey, especially #91 nonvisual/browser handoff or #92
authoritative decision reading. These alpha releases do not complete the
product. Preserve the same hidden-native, focused-test and honest-evidence
boundaries.

### Desktop 0.1.36 PDF result handoff — 2026-09-28

The active Forge Desktop objective continues. A hidden native capture of a
project `report.pdf` showed a real usability gap: the app identified the file
but offered only conversation, not a way to see it. This package adds an
explicit **Abrir PDF no navegador** action beside the existing project-file
card. No external app opens merely by selecting or refreshing a file. The
native command rechecks the confirmed project, canonical path containment,
PDF extension and header before requesting the user's default browser. The
isolated in-app HTML preview and other unsupported-file behavior are retained;
no new project store or PDF renderer was added. The bundled core stays 0.13.2.

Changed source: `src-tauri/src/preview.rs`, `src-tauri/src/main.rs`,
`ui/preview.mjs`, `ui/index.html`, `tests/browser.cjs`, `tests/native.cjs`,
Desktop version files and `RELEASE_NOTES-0.1.36.md`. The focused Rust PDF
validation test passed, as did 51 non-ignored Desktop tests and the browser
suite. A debug-build hidden native run passed actual Windows PDF file-picker
selection, visible PDF action and conversation draft in a newly onboarded
disposable project. The reviewed screenshot is
`D:/Temp/User/forge-0136-pdf-handoff.png`. Actual browser launch was NOT_RUN
because it can surface on the maintainer's desktop; simulated UI invocation
and native path/header checks are not a substitute for that observation.
Actual Codex Send was not repeated for this result-view change. No subagent
was used. Attributable model tokens, API-equivalent BRL and Pro-quota impact
remain UNKNOWN.

**Next exact step:** run package-boundary static checks, build one 0.1.36 NSIS
candidate, silently install it over public 0.1.35, repeat the hidden native
project/PDF smoke, record the exact hash, then selectively commit/push/tag and
publish under the standing alpha authorization. Download unauthenticated,
compare bytes, install that download and repeat the hidden native smoke. Do
not rebuild the candidate between verification and publication.

Package-boundary Desktop `cargo check`, strict Clippy, JavaScript syntax and
`git diff --check` passed. Repository-wide Desktop `cargo fmt --check` found
pre-existing formatting differences in `agent.rs` and other untouched source;
no broad formatting rewrite was made. The single 0.1.36 NSIS candidate is
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.36_x64-setup.exe`,
123,068,768 bytes, SHA-256
`DAC51CBF4B8653A4DF0223510CDECA4AF9B7D61752926473F0F40DCB1357B575`.
Silent installation over public 0.1.35 returned exit 0 without changing the
candidate bytes. The installed 0.1.36 executable has SHA-256
`CCAF472DB21B0DDB9B2D51B9B2609ABA8982559084389768A0EA479ACB85CF98`.
Installed-candidate hidden native smoke passed with actual Windows folder and
PDF file selection, disposable-project Forge onboarding, record readback and
isolated HTML preview. Actual external browser launch and actual Codex Send
remain NOT_RUN for this package. The next step is publication and downloaded
file readback, not rebuilding the candidate.

### Desktop 0.1.36 public readback — 2026-09-28

Code commit `fcf32bf7` was pushed on `codex/desktop-shell` and tagged
`desktop-v0.1.36-alpha.1`. The single tested installer and SHA-256 sidecar
were published as the [0.1.36 alpha prerelease](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.36-alpha.1).
Unauthenticated downloads returned a 123,068,768-byte installer with SHA-256
`DAC51CBF4B8653A4DF0223510CDECA4AF9B7D61752926473F0F40DCB1357B575`,
identical to the candidate and sidecar. Installing the downloaded file
silently returned exit 0 and did not change it. The installed executable
reports 0.1.36 and SHA-256
`CCAF472DB21B0DDB9B2D51B9B2609ABA8982559084389768A0EA479ACB85CF98`.
Downloaded-installation hidden native smoke passed with actual Windows folder
and PDF file selection, new-project onboarding, Forge record and isolated
preview. External PDF browser launch and actual Codex Send were NOT_RUN for
this package; they are not implied by successful app-command validation or
older real-Codex evidence. Clean-machine setup, mobile-device use and manual
screen-reader acceptance remain NOT_RUN/PARTIAL. No subagent was used and
model-attributed tokens, API-equivalent BRL and Pro-quota impact remain
UNKNOWN. No candidate rebuild or manual GitHub CI run followed publication.

**Next product step:** continue the full Forge UI objective; the PDF handoff
closes one result-access gap, not all visual coverage. Avoid inventing
historical decision text from current policy or a digest; inspect the
authoritative core contract and its Desktop pin before addressing #92.

### Result action hierarchy after 0.1.36 — 2026-09-28

The active full Forge Desktop objective continues. The installed 0.1.36 PDF
capture showed that **Conversar sobre este arquivo** came before the main
**Abrir PDF no navegador** action, which was buried below file-origin copy.
The current source makes opening the PDF the first, coral action and keeps
conversation secondary; the DOM/keyboard order matches the visible order.
Other unsupported files emphasize conversation, while HTML/text/image
results emphasize **Pedir mudança neste arquivo** and retain the optional
HTML browser action in its previous position. PDF copy is shorter and does
not claim the PDF was rendered inside Forge. No native command, project
authority or stored state changed.

Changed locally, **not yet committed or installed**: `ui/index.html`,
`ui/preview.mjs`, `tests/browser.cjs`, `tests/native.cjs`, and this checkpoint.
The browser suite passed, including PDF/HTML action order, narrow 390px
pane switching, 200% enlarged text and a 48px PDF target. Rebuilt debug UI
passed hidden native Windows smoke with real project onboarding and actual
PDF file-picker selection; its 390px/200% PDF state had no horizontal overflow
and retained the selected file when switching panes. Screenshot
`D:/Temp/User/forge-0137-pdf-primary-final.png` was reviewed against the
approved conversation board. `git diff --check` passed. Actual external
browser opening and actual Codex Send were NOT_RUN for this UI-only slice.
No subagent was used; model-attributed tokens, API-equivalent BRL and Pro
quota impact remain UNKNOWN. This source change does not update the public
0.1.36 installer.

Read-only #92 contract review also reconfirmed the core boundary: durable
decision audit carries references, digests and status, not verified historical
question/choice prose. The Desktop cannot present those as accepted agreements
without a core-owned historical readback; this did not justify a fake UI list
or desktop-owned decision store.

**Next exact step:** continue a different high-value, user-visible journey
before packaging this visual refinement with other coherent UI work. For #92,
first establish a verified historical core readback (or explicitly unavailable
status) before rendering decision wording. Do not rebuild/publish merely for
this action-order slice; preserve the local diff.

### Desktop 0.1.37 responsive actions package — 2026-09-28

The source-only PDF action-order refinement above is now combined with a
native-discovered 200% text issue: at 390px, the narrow workspace navigation
broke **Conversa** and **Andamento** inside the words. The 0.1.37 CSS keeps
the two-column navigation at ordinary text size (including the 360px minimum
window) and gives each destination a full row when enlarged text requires it.
Browser tests assert both arrangements and intact labels; a hidden native
390px/200% screenshot at
`D:/Temp/User/forge-0137-pdf-narrow-zoom-final.png` was reviewed. No project,
agent, backend or storage contract changed. This package remains separate
from a full product-readiness claim.

Changed: `ui/index.html`, `ui/preview.mjs`, `ui/styles.css`, browser/native
tests, Desktop version files, `RELEASE_NOTES-0.1.37.md`, and this checkpoint.
JavaScript syntax, browser suite, Desktop `cargo check`, 51 non-ignored
Desktop tests, strict Clippy and `git diff --check` passed. A debug build and
then the installed NSIS candidate passed hidden native Windows smoke with
actual folder/PDF picker selection, onboarding and Forge record readback.
Actual external PDF browser launch and actual Codex Send were NOT_RUN for
this UI-only package. No subagent was used; attributable model tokens,
API-equivalent BRL and Pro-quota impact remain UNKNOWN.

One NSIS candidate was built at
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.37_x64-setup.exe`,
123,159,645 bytes, SHA-256
`4A0A4C688B22E624EFE6166DDB6053D63037AFDE8C0F067C73C9C1513CA93C3D`.
Silent installation over public 0.1.36 returned exit 0 and left the
candidate hash unchanged. Installed 0.1.37 executable SHA-256 is
`21DBE1B874DD5D887CB701D8A1A264A6B5C862E0172EF9EFC4FC51DE8A48005B`.
Installed-candidate native smoke passed. Do not rebuild the candidate;
selectively commit/push/tag and publish under standing alpha authorization,
then download unauthenticated, compare bytes, reinstall that download and
repeat native smoke. Continue other high-value UI/backend journeys after
this package; #92 historical wording remains subject to core readback.

### Desktop 0.1.37 public readback — 2026-09-28

Code commit `1e0fdfd0` was pushed on `codex/desktop-shell`, tagged
`desktop-v0.1.37-alpha.1`, and the single tested installer plus SHA-256
sidecar were published in the [0.1.37 alpha prerelease](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.37-alpha.1).
Unauthenticated downloads returned 123,159,645 bytes and SHA-256
`4A0A4C688B22E624EFE6166DDB6053D63037AFDE8C0F067C73C9C1513CA93C3D`,
identical to the tested candidate and sidecar. Silent installation of that
download returned exit 0 without changing its hash. The installed executable
reports 0.1.37 and SHA-256
`21DBE1B874DD5D887CB701D8A1A264A6B5C862E0172EF9EFC4FC51DE8A48005B`.
Downloaded-installation hidden native smoke passed with actual Windows folder
and PDF selection, new-project onboarding, Forge record and isolated preview.
Actual external PDF browser launch and actual Codex Send were NOT_RUN for this
UI package; clean-machine, mobile device and manual screen-reader acceptance
remain unverified. No candidate rebuild or manual GitHub CI run followed
publication. Model-specific usage and BRL economics remain UNKNOWN.

**Next product step:** continue the full Desktop objective beyond action
hierarchy/accessibility. Prioritize a user-visible path not yet proven with
real Codex on the installed version, then a truthful UI for historical
project decisions only after a core-owned historical readback exists. Keep
the installed 0.1.37 release distinct from future source-only work.

### Installed 0.1.37 real-Codex continuation — 2026-09-28

Two opt-in hidden native runs exercised a narrow 390px window and a fresh
disposable Forge project against the installed, publicly downloaded 0.1.37
executable. In both, **Enviar** delivered one actual Codex reply and WebView
reload recovered both user and reply without a second Send. The second run
also passed a full native process restart: the same conversation reopened in
order, with `send_message` invocation count zero during restoration. These
are same-machine, existing-auth proof of conversation continuity; they do
not repeat the older file-creation/change journey or prove clean-machine
sign-in. No real user project was modified.

Both broad runs finished with a **test-harness FAIL after the real journey**,
not a demonstrated product failure. The first waited for `#project-status`
to be visually displayed after a project was ready, although the narrow UI
intentionally hides that status node. The second reached the full-restart
PASS, then tried to fill the project-folder field while the project setup
was collapsed. `tests/native.cjs` now waits for authoritative status text
and opens setup (disconnecting the resumed agent first) when combining
restart with the separate new-idea check. Syntax, diff check and the ordinary
hidden native new-idea journey passed after this harness edit. The combined
restart + new-idea tail was **NOT_RUN after correction**; do not count the
whole broad script as PASS. A third real Codex turn was intentionally not
spent merely to rerun that low-return test tail. The two earlier turns and
test retries are rework; model-attributed token/BRL/quota data remain UNKNOWN.

Changed after publication: `tests/native.cjs` and this checkpoint only.
The installed/public 0.1.37 bytes are unchanged. Next product slice: make
the stage/decision view more understandable from authoritative data without
fabricating historical accepted wording; inspect #92's core-owned readback
before changing contracts. Keep real-agent continuation evidence distinct
from the release's earlier smoke checks.

### Pending-choice handoff after 0.1.37 — 2026-09-28

The full Desktop goal remains active. For #92, the current record already
shows the authoritative current stage, Work Focus, suggested questions and
versioned accepted objective. The core's workflow decision audit, however,
retains policy/decision references, status, selected alternative reference
and digests; `DecisionNeedRaisedEvent.question_digest` is not the original
question. The Desktop's pinned core 0.13.2 exposes no verified historical
question/choice prose. Do not label these references as human-approved
agreements or reconstruct text from current policy material.

The source now offers **Entender escolhas em aberto** only when the Forge
readback reports at least one recovered pending decision. It appends an
unsent request to the existing conversation asking the agent to consult the
original source, report inability to recover it honestly, and avoid treating
suggestions as chosen. It preserves the user's draft, switches from the
narrow progress pane to the conversation, and never records a decision.
Suggestions without a recovered pending decision do not expose this action;
failed/invalid refresh hides it. No core write, extra project store or
automatic provider turn was introduced.

Changed locally from public 0.1.37: `ui/index.html`, `ui/progress.mjs`,
`ui/styles.css`, `tests/browser.cjs`, `tests/native.cjs`, and this checkpoint.
The browser suite passed the pending-only, suggestion-only, narrow-pane,
draft-preservation and failure paths. Rebuilt debug UI passed hidden native
Windows smoke with actual project/Forge readback. A separate **controlled**
native IPC response with one pending decision exercised the new action,
confirmed no Send and restored actual IPC afterward; screenshot
`D:/Temp/User/forge-0138-pending-action.png` was reviewed. This controlled
response does not prove the real project has a recovered pending decision,
nor that a historical question can be recovered. Actual Codex Send for the
new action was NOT_RUN. No Rust source changed; no Rust test loop was needed
for this UI-only slice. The installed/public 0.1.37 is unchanged. No subagent
was used; per-model tokens, API-equivalent BRL and Pro allowance impact are
UNKNOWN.

**Next exact step:** inspect the core-owned historical decision readback and
its release/pinning path for a verifiable question-and-choice projection.
Only then make the decision history itself readable in the Desktop; do not
solve the gap with desktop-owned history or inferred wording. Keep this
tested source for a coherent future alpha package rather than publishing a
new installer for one button.

### Desktop 0.1.38 preview-first workspace — 2026-09-28

Core historical decision audit was inspected: it retains references,
digests and statuses, but not verified original question-and-choice prose.
No new historical decision claim or Desktop-owned store was added. The
pending-choice handoff above remains deliberately source-aware and unsent.

The workspace now shows a compact empty **Prévia do resultado** and
**Escolher arquivo** above the Forge record. The record's stage is compact
without hiding its text; its activity and next step remain readable below.
Loaded results stay in that position, so visual, DOM and keyboard order no
longer swap after file selection. The short preview copy still distinguishes
local preview from publication. At 1180px the installed native screenshot
`D:/Temp/User/forge-0138-installed.png` was reviewed: preview choice, stage,
start of activity, conversation and Send appear in the first window.

Browser suite, JS syntax and `git diff --check` passed. Desktop `cargo check`,
51 non-ignored Desktop tests and strict Clippy passed; there was no workspace
Rust suite for this UI/version-only change. One release NSIS candidate was
built at
`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.38_x64-setup.exe`,
123,097,865 bytes, SHA-256
`5506BA4E61DA833A8FA381559E22DA8D3F9F4C645212EB0D16B02A0B2AB05797`.
Silent install over public 0.1.37 returned exit 0; candidate hash did not
change. Installed executable reports 0.1.38, SHA-256
`EDA191ACC39A624AF77EFC78CF1881D8751F0184879F819EC6F9B4CBC5B4AA82`.
Installed hidden native smoke passed, including actual Windows folder/PDF
selection, onboarding and Forge record readback. Actual Codex Send for the
pending-choice action was NOT_RUN; its controlled IPC proof does not verify
historical prose. No manual GitHub CI was run. Per-model token/BRL/Pro-quota
metrics remain UNKNOWN.

**Publication boundary:** selectively commit/push the reviewed 0.1.38 source,
tag and publish this exact candidate under the standing alpha authorization,
then download without authentication, verify size/hash, reinstall that
download and repeat hidden native smoke. Do not rebuild the candidate.
Afterward, keep progressing on meaningful remaining product journeys rather
than treating this visual refinement as completion of the full Desktop goal.

### Desktop 0.1.38 public readback — 2026-09-28

Commit `c33ed7dc` was pushed on `codex/desktop-shell`, tagged
`desktop-v0.1.38-alpha.1`, and the single tested installer plus SHA-256
sidecar were published in the [0.1.38 alpha prerelease](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.38-alpha.1).
Unauthenticated downloads returned 123,097,865 bytes and SHA-256
`5506BA4E61DA833A8FA381559E22DA8D3F9F4C645212EB0D16B02A0B2AB05797`,
identical to the tested candidate and sidecar. Silent installation of that
download returned exit 0 without changing its hash. The installed executable
reports 0.1.38 and SHA-256
`EDA191ACC39A624AF77EFC78CF1881D8751F0184879F819EC6F9B4CBC5B4AA82`.
Downloaded-installation hidden native smoke passed. Actual Codex Send for the
pending-choice action, clean-machine setup, mobile-device use and manual
screen-reader acceptance remain unverified. The historical decision text
is not reconstructed from references. No manual GitHub CI run. Per-model
tokens, BRL-equivalent cost and Pro-quota impact remain UNKNOWN.

**Next product slice:** keep the installed release distinct from future source
work. Audit a user journey that has not been proven end to end on the current
alpha (especially using a real result from a Codex turn and returning to it
after restart); fix a concrete UX/backend connection gap if observed. Do not
add a second history store or present inferred historical decisions as fact.

### Installed 0.1.38 real-result journey — 2026-09-28

One additional hidden Windows run against the downloaded and installed public
0.1.38 used a disposable project and three actual Codex turns: a no-tools
reply, creation of local `site/index.html`, and a change to its visible title.
The app displayed the generated page in its isolated preview, prepared a
change request without sending it, then displayed the changed title after
the explicit Send. A WebView reload and a full native process restart both
restored the real conversation in order without re-sending any turn. The
native harness reported PASS. Screenshot
`D:/Temp/User/forge-0138-real-result.png` was reviewed; it shows the real
agent reply, local file action, preview, change action and composer in the
same workspace. No real user project was modified. The fixture is removed
by the harness after the run.

This proves the result/change/conversation-continuity path on the installed
alpha, not automatic re-opening of the selected preview after restart. The
pending-choice action still has only a controlled IPC proof, and historical
decision prose is not available through the pinned core. Model-attributed
tokens, BRL-equivalent cost and Pro quota impact remain UNKNOWN. The next
useful product check is whether a returning nontechnical person can find a
previous result without knowing its filename; verify that journey before
adding any new persistence or duplicated project history.

A separate installed 0.1.38 hidden native **controlled-history** test passed
after full process restart: a restored file citation reopened its
project-bound preview and prepared a change without sending; an
outside-project path was refused, and two distinct citations stayed
individually selectable. This is not a replay of the real Codex fixture and
does not prove automatic preview selection. Together with the real-result
journey, it supports explicit recovery via the conversation, while the exact
returning-user experience still warrants observation.

### Desktop Explore layout follow-up — 2026-09-28

The returning-user result shortcut was checked on the installed 0.1.38 with a
hidden native controlled-history run. The existing restored file action was
visible and opened the project-bound preview after process restart; no second
result store or guessed file was added. A separate visual comparison found a
concrete gap on Explore: the search sat below the invitation even at desktop
width, pushing the approved theme art down. Source now places invitation and
search in one row from 1160px, while narrower widths retain the stacked form.
The decorative art was moved above the search label to keep the label legible.

The browser suite passed with explicit 1536/1280/1180/1100px layout bounds,
filtering, keyboard and mobile checks. A debug desktop rebuild and hidden
native smoke passed; its 1180px screenshot
`D:/Temp/User/forge-explore-native-1180.png` was visually reviewed. This is
**source/debug-build work only**: the published/installed 0.1.38 installer
still has the prior layout. No Rust core changes or broad Rust test suite were
needed. Actual Codex Send was NOT_RUN in this follow-up; the earlier installed
real-result journey remains the evidence for that path. Per-model tokens,
BRL-equivalent cost and Pro-quota impact remain UNKNOWN.

Next: continue closing concrete UI/BE journeys rather than treating this layout
refinement as completion. Bundle it with a coherent future desktop alpha,
validate that installer, and only then describe the new layout as installed.
