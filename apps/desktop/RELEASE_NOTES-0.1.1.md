# Forge Desktop 0.1.1 alpha — approved local candidate

**Status: maintainer approved this exact candidate for publication on
2026-09-27; public upload and downloaded-byte verification are still pending.**
Earlier local `0.1.1` installers are archived as **superseded**. The current
Windows x64 candidate is
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.1_x64-setup.exe`
(4,619,155 bytes; SHA-256
`F8E04F44E314DB7D530AD251731ACA69C76C6E239FF0FF0E3D461D2E55E70568`).
It was silently installed over the prior local `0.1.1` and tested in an
isolated desktop, including a real Codex reply without a CLI override and
actual Windows folder/file dialogs. This is not a public download or a claim
of whole-product readiness.

## What this source delivers

- Portuguese Home, Explore, My Projects and conversation screens, with eight
  illustrated themes, approved botanical Home/Explore accents, a chat-led
  project board, light/dark/contrast preferences, and local shortcuts to
  folders previously opened in this app.
- One folder action for a new, existing or previously unlinked project. Choosing
  a folder alone changes nothing; **Continuar nesta pasta** asks Forge core to
  prepare or reopen it and, when the core explicitly requests it, initialize
  its workflow record. Existing files are preserved. A damaged Forge link or
  missing linked state is not silently recreated.
- Codex conversation in the chosen project: send, interrupt, explicit prior-chat
  selection, read-only resume and bounded long-history loading. Recovery does
  not resend a previous message or replay completed actions by itself.
- Read-only Forge record: recorded stage, activity and next step, current
  accepted direction and its history, pending-decision count, and current
  decision suggestions with options and consequences. Suggestions are not
  accepted decisions; conversation actions only prepare unsent drafts.
- Local result review for project text, Markdown, supported images and
  scriptless static HTML. A completed Codex reply can link to a real project
  file; the native preview validates it before opening. The user can prepare
  an unsent change request from the preview.

## Requirements and honest limits

- Requires a separately installed `forge-core` and an installed, authenticated,
  compatible Codex CLI. Neither is bundled in this installer. The app first
  honors an explicit `FORGE_CODEX_EXE`; otherwise it looks for the current
  Codex Desktop CLI on this device, then falls back to the npm CLI location.
  This Codex Desktop internal path is best-effort and may change in future app
  versions. The npm CLI installed on this test machine was too old for a real
  reply in the default-model test. The app uses Codex's existing login and
  conversation history on this device.
- Windows x64, unsigned NSIS current-user installer. No automatic in-app
  updater: install a newer package over the previous alpha. No public download
  exists until the maintainer approves publication and the uploaded file is
  downloaded and hash-checked.
- Codex can execute commands and change local files under the current access
  policy. Use trusted projects and review the agent's work. Interactive approval
  forms are not yet exposed in this UI; the agent asks in ordinary conversation.
- The Forge record is a read-only snapshot and may lag behind chat. An agent
  reply, a locally opened file and a recorded stage do not prove the product is
  finished or published. Historical decision questions/alternatives are not
  reconstructed from current suggestions.
- Static HTML preview blocks scripts, network access and interaction. Relative
  local CSS/images work within the selected site directory; root-absolute
  `/assets` references, dynamic sites and external resources may not render.
  The enlarged preview can reveal more of a long page in bounded steps but
  is not a full interactive browser or complete page inspection.
  Text and image previews are bounded in size and format.
- Silent installation over the prior local `0.1.1` and native app readback
  passed. One real Codex turn in the installed candidate produced a reply
  without `FORGE_CODEX_EXE`; both sides survived WebView reload without
  resending. A separate disposable-profile test created one real Codex turn
  in the previous local candidate (SHA-256 prefix `3F7EA430`), installed this
  exact candidate over it, and restored the same user/reply pair without
  sending another turn. This proves continuity for these two local builds,
  not every upgrade. An exact `0.1.0` → current `0.1.1` upgrade and native
  rendering of a nonempty real Forge decision suggestion remain **NOT_RUN**.

## Candidate gate before publication

The candidate identified above passed 43 desktop Rust tests, strict Clippy,
formatting, seven Node unit tests, the browser UI suite, silent install,
isolated native smoke, and a populated conversation upgrade from the previous
local `0.1.1` candidate. Its installer hash was rechecked after those tests.
The maintainer approved this exact file and its limits on 2026-09-27. Any
subsequent application-source edit supersedes it and requires a new candidate.
Publication is complete only after the uploaded file is downloaded, its hash
matches this candidate, and that downloaded file is installed and checked.
