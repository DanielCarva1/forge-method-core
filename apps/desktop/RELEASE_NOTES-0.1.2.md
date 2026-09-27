# Forge Desktop 0.1.2 alpha — published prerelease

Published prerelease:
<https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.2-alpha.1>.
The unsigned Windows x64 NSIS file was built locally at:
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.2_x64-setup.exe`
(4,623,866 bytes; SHA-256
`F8C57185E3CDD96C5329A8D70C5DC426EAD793BB5DFEF894E02EB93A3B251EA1`).
It was rebuilt after the conversation-viewport, new-idea, and **Abrir outro
projeto** project-boundary fixes. The earlier files with hashes
`C88219654ED0525C49DB6A6D6CDADB1386607B59587774D1B6390C176AD42B5A`,
`75F9DCBFB1CAF09D97DDD5ADCFCC79C45CF313788952CF10420292313662180C`
(and the older `D86273A6...` file) are superseded. This exact new candidate
was installed silently over `0.1.1` and passed the installed-binary checks
below. The maintainer approved publication. The public download was checked
and installed afterward.

## What changes from 0.1.1

- A slow read of the Forge record or direction history now gets a bounded
  longer wait and a plain-language status after six seconds. Project identity
  and setup keep their shorter timeout. A new project's empty record clearly
  says no work has been recorded; it does not invent progress.
- My Projects, the workspace and the Codex connection use the folder name a
  person recognizes, while retaining the full path and Forge ID for identity.
  The cards use the Forge icon. Approved botanical artwork now frames the
  project list and conversation workspace without supplying fake project data.
- A message awaiting Codex's native send acknowledgement leaves a thread-only
  local marker before Send. If the app closes or Send fails, reopening blocks
  replay and a new thread until the same conversation is explicitly resumed
  for review. If the marker cannot be saved, the message stays a draft and is
  **not sent**. This is a safety guard, not a promise of exactly-once delivery.
- In a confirmed project, the conversation invitation, bounded message history,
  composer, access warning and Send fit in the default 1180 x 820 desktop
  window without covering one another. The optional history controls retain
  their explanation in one disclosure instead of occupying the message area.
- Choosing a fresh idea from Explore or Home while another project is open now
  returns to folder choice with the idea draft intact, instead of silently
  sending it to the previous project. An active Codex session is disconnected
  first; a running turn requires confirmation, and cancelling keeps the old
  project and draft. The old transcript is not displayed in the new-project
  setup. Ordinary **Minha conversa** still returns to the current project.
- **Abrir outro projeto** now also clears the previous folder, confirmed-project
  display and transcript after an allowed switch. Send stays disabled until a
  new folder is confirmed; declining interruption leaves the current project
  intact. The UI no longer looks like a new project while still addressing the
  old conversation.

## Current alpha limits

- The app still requires separately installed `forge-core` and an authenticated,
  compatible Codex CLI. It has no in-app automatic updater. The installer is
  Windows x64, current-user and unsigned.
- Codex may run commands and change files under the current access policy;
  interactive approval controls are not implemented in this UI. Use trusted
  projects and review the agent's work.
- The Forge record is a read-only snapshot and may lag behind chat. Suggested
  questions are not accepted human decisions. Historical decision details
  unavailable from the projected record are not reconstructed or invented.
- Local HTML preview is scriptless and noninteractive; dynamic sites,
  root-absolute assets and external resources may not render. No complete
  accessibility audit or whole-product visual acceptance has occurred. The
  new viewport layout is a targeted improvement, not approval of every screen.
- A cold read longer than the new 85-second per-attempt budget may still fail.
  The previously measured 64-second cold read was not reproduced against this
  candidate. A real Codex message caught mid-delivery during a process crash
  has not been tested; the native restart guard used a controlled bridge.

## Evidence and pending gate

**PASS for this candidate:** 43 desktop Rust tests, formatting, strict desktop
Clippy, eight Node tests, the complete controlled browser UI suite at
1180 x 820 and 1280 x 844 (plus keyboard, mobile, enlarged text and long
history), NSIS build and SHA-256 readback. The rebuilt release executable
passed hidden native WebView smoke with real Forge project/record readback,
onboarding, preview and appearance. A separate hidden full-process restart
confirmed the uncertain-send guard against a controlled Codex bridge. The
native viewport capture confirms Send and the empty-chat invitation are visible
together after a selected-idea draft is preserved. An earlier release payload
sent a real Codex turn and restored it after reload and restart. Separately,
installed `0.1.1` created a disposable real Codex turn and this candidate's
local release executable resumed the same user/reply pair without resending.
That is not an installed-upgrade check. A focused hidden release-WebView check
at an emulated 590 x 410 CSS viewport kept a long unsent draft, made Send
reachable without sideways page overflow, and preserved that pair. This is
not an actual OS zoom or virtual-keyboard test. The broad hidden native smoke
was rerun successfully without the experimental viewport override; transient
CSS-load and onboarding waits in diagnostic iterations prompted test-harness
hardening, not a claimed product fix. A further complete hidden native smoke
passed with the actual Windows folder and file dialogs: folder cancellation,
linked and unlinked folder selection, explicit onboarding without changing an
existing file, and local file preview. After the new-idea correction, the
controlled browser suite (including connected/running-turn guards), eight Node
tests, a fresh debug build and a release NSIS build passed. The release
executable passed hidden native smoke with real Windows dialogs and the
new-idea flow; the latter was visually inspected in a native screenshot.
It also resumed the existing real Codex user/reply pair from installed 0.1.1
without a new send. A separate hidden native smoke on this release executable
sent one new no-tools turn to real Codex, displayed the reply, and restored
both sides after WebView reload without sending again. This does not install
the candidate or prove a full-process restart. Another hidden native run
followed **Explore → fresh folder → real Forge onboarding → real Codex reply**
with exactly one user/agent pair in the new project. The test replaced the
suggested artistic draft with a safe no-tools prompt before Send; the suggested
draft itself was not delivered. After the **Abrir outro projeto** correction,
the controlled browser suite first failed on retained old-folder state, then
passed. A fresh debug build and a new release NSIS build passed; hidden native
smoke of both the debug and release executables verified that this route clears
the old folder/transcript and blocks Send until confirmation. This route did
not send a new real Codex turn in those two smokes. The current release
executable also resumed the real `0.1.1` user/reply pair in the retained
disposable profile with no new Send and no extra turn. That remains a
**pre-install** readback check, not an installed-upgrade result. Separately,
the exact current local release executable sent one fresh no-tools prompt to
real Codex in a disposable Forge project, received a reply, and restored the
user/reply pair after WebView reload without sending again. This was one real
Codex turn, not a measured count of internal model requests; model-specific
tokens and BRL cost are UNKNOWN.

An additional real Codex artifact attempt on this exact local executable
created a disposable `site/index.html`, but its broad runner was interrupted
before the original reply completed. A focused hidden-native continuation
resumed the same conversation, obtained a relative file link in one bounded
no-tools follow-up, opened the actual local HTML preview from that reply, and
prepared a change request in the same unsent composer. This is **PASS** for
the resumed artifact-to-preview-to-change path, not for an uninterrupted
original artifact turn. Model-specific tokens and BRL cost are UNKNOWN.

**PASS for the installed update:** the exact candidate installer exited 0 when
installed silently over published `0.1.1`. The installed executable reports
product/file version `0.1.2` and SHA-256
`E21FFB1CFF1710A49197F651D66D449EC1D7FE2DB579F885A78ED20B36C40774`.
Using that installed executable, the retained isolated project/profile restored
the real Codex user/reply pair created under installed `0.1.1`, with no new
Send or extra turn. A separate full hidden native smoke of the installed
executable passed project onboarding, real Windows folder/file dialogs, Forge
record readback, local preview, project switching, and appearance persistence.
It did **not** send another real Codex turn.

**PASS for publication:** tag `desktop-v0.1.2-alpha.1` resolves to source
commit `31594f953c611f5a6be8d651dbde623ba4e1b610`. The GitHub prerelease
contains the installer and matching `.sha256` sidecar. An unauthenticated
download to a fresh path returned 4,623,866 bytes with the same installer
SHA-256 as above; the sidecar content matched. That downloaded file exited 0
when installed silently. The installed executable again reported `0.1.2` and
the same hash. A hidden resume of the retained real Codex pair passed again,
without sending a new turn. No GitHub CI rebuild was used for this desktop
prerelease; the tested prebuilt file was uploaded.

**Still untested:** a real Codex message caught mid-delivery during a process
crash, full-product visual/accessibility acceptance, and all possible upgrade
paths. Model-specific token counts and BRL cost remain UNKNOWN.
