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
- Rust also exposes `inspect_project`: a read-only call to the existing
  `forge-core project resolve` command. The backend remains the link/state owner;
  the UI never infers progress from its compatibility phase field.
- Project selection accepts a local folder path through the native Windows
  folder picker or the text field. Only already-linked projects with available
  state are identified. Failed lookups hide earlier results. The **Meus projetos**
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
- Each CLI query has a 15-second timeout and 64 KiB output limit, hides the subprocess
  console, terminates unfinished child processes, and does not expose stderr.
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

Requests time out after 30 seconds; individual protocol lines are limited to
1 MiB. There is no turn-duration timeout: users may interrupt long tasks. Only
one turn is admitted at a time. Disconnect/normal window close requests an active
turn interruption for up to three seconds and stops the owned app-server. This
does not roll back completed file changes or guarantee cleanup after an OS crash.
The session slot is not held while awaiting provider RPCs. Cleanup is serialized
separately so repeated close requests cannot bypass an in-progress shutdown.

Codex owns saved history. The UI stores only a thread bookmark, scoped to the
resolved project ID and canonical folder, in `forge.conversation.v1` localStorage
keys. Reconnect reads the stored thread summary and validates its folder and ID
before resuming; the returned transcript is validated again and rendered as text.
Interrupted/failed-turn answers are marked incomplete. No turn is sent by resume.
Users can explicitly start another conversation without deleting Codex history.
A failed resume never silently falls back to a new thread. No conversation list,
credentials, decisions or Forge state are copied into this bookmark store.

A failed bookmark write keeps the latest reference in memory for reconnects and
warns that restarting the app may reveal the older saved conversation. It does
not claim persistence. Resume requests `excludeTurns: true`, then reads full turns
in ascending pages of one turn. This stable protocol was confirmed in the Codex
0.154.0 generated schema. Recovery is bounded to 4,096 pages, 32 MiB of serialized
page data and 60 seconds; the existing 1 MiB frame limit is unchanged. A single
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
2. **PARTIAL:** the UI slice and actual Windows folder selection were checked.
   Commit and push remain pending; preserve the intermittent native record
   lookup failure in the release limitations.
3. **DONE in source:** desktop `0.1.1` is set in Tauri, Cargo and Cargo.lock;
   draft release notes exist. This alone is not an available update.
4. **PARTIAL:** final checks, one candidate build, exact hash and a silent
   `0.1.0` to `0.1.1` install-over test are recorded below. Installed runtime
   restart and conversation/preference continuity remain NOT_RUN under the
   user's headless-only constraint.
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
candidate and silent upgrade are recorded below; commit/push and publication
remain pending.

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

A one-off attempt to start the installed app on an isolated invisible Windows
desktop exited before exposing a WebView debug endpoint. No visible app was
opened, but project, preference and conversation recovery after the installed
upgrade remain **NOT_RUN**. Do not claim installed runtime continuity from the
silent install. The native development smoke also had one intermittent generic
record lookup failure; five direct CLI reads succeeded and subsequent native
smokes passed, but the cause is not established. Keep this as a known alpha
limitation, not a resolved bug. Publication awaits maintainer acceptance of
`RELEASE_NOTES-0.1.1.md` and downloadable-artifact verification.
