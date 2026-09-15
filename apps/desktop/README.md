# Forge desktop shell

Independent Tauri application, currently a development shell, not a published
user release. It does not require Codex Desktop. A Codex CLI adapter supports
conversation in an explicitly confirmed project. The native identity check alone
is not an agent connection.

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
server, CDN or runtime download. This is a small first slice, not a commitment
against using a frontend framework when component complexity warrants it.

## Boundaries

- `app_info` is an identity read with no project access.
- Rust also exposes `inspect_project`: a read-only call to the existing
  `forge-core project resolve` command. The backend remains the link/state owner;
  the UI never infers progress from its compatibility phase field.
- Project selection currently accepts an absolute folder path; a native folder
  picker is not implemented. Only already-linked projects
  with available state are identified. Failed lookups hide earlier results.
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
not claim persistence. Very large histories exceeding the existing 1 MiB frame
limit fail visibly without truncation; users can continue through Codex CLI.
Bounded history pagination remains follow-up work before claiming unrestricted
long-running conversation recovery. Codex may not persist an empty thread until
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
