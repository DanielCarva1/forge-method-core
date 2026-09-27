# Forge desktop shell

Independent Tauri application, currently a development shell, not a published
user release. It does not require Codex Desktop. A Codex CLI adapter supports
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
server, CDN or runtime download. Packaging uses the exact Tauri CLI version in
`apps/desktop/package-lock.json`; this build-only dependency does not add a
frontend runtime. This is a small first slice, not a commitment against using a
frontend framework when component complexity warrants it.

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
- On Windows the adapter uses the installed executable under
  `%LOCALAPPDATA%/Programs/forge-core/bin/forge-core.exe`. Host configuration can
  override it with an absolute `FORGE_CORE_EXE`; the webview cannot choose commands
  or executables. No PATH search occurs inside the selected project.
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

The Windows adapter locates the native executable in the standard per-user npm
Codex installation. `FORGE_CODEX_EXE` may specify an absolute standalone Codex
executable as host configuration; it is never supplied by webview content. The app
does not install/update Codex or use the Codex Desktop bundled runtime. Sign in
through the supported Codex CLI login flow. The current slice requires a ChatGPT
account; API-key login, ZCode and in-app login are not implemented.

The integration was exercised with standalone Codex CLI 0.154.0. The installed
0.144.6 authenticated successfully but the provider rejected its use of
gpt-6-astra with an explicit CLI-upgrade requirement. The UI translates that
failure into an update instruction, without forwarding raw provider errors.
Authentication alone does not establish model/client compatibility.

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
