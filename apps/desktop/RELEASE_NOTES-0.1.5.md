# Forge Desktop 0.1.5 alpha — candidate notes

This Windows x64 alpha makes the existing project-and-conversation journey
clearer. It does not add another project registry or claim that the whole Forge
backend has a screen.

## Change from 0.1.4

- The empty result preview is smaller, so the project record begins in the
  first desktop viewport rather than being pushed below a large placeholder.
  Once a file is selected, its existing local preview remains available.
- The project record now makes an existing Forge direction discoverable without
  displaying a long technical objective by default. The original wording,
  constraints and origin remain available on demand. A button prepares a
  Portuguese explanation request in the current conversation without sending
  it or changing the Forge record. A Forge agent's record is not presented as
  independent proof of human approval.
- Screen-reader group semantics and reply heading levels were corrected for
  the current navigation, project record and conversation UI.

## Alpha limits

- `forge-core` and a compatible authenticated Codex CLI remain separate
  installations. The installer is unsigned and current-user only; there is no
  in-app automatic updater.
- The preview is local, read-only, scriptless and noninteractive; it is not
  proof of publication or production behavior.
- The Forge record can lag behind the chat. Full plain-language history of
  resolved decisions remains incomplete because the current core audit does
  not supply immutable user-facing question and choice wording. The app does
  not invent this from the chat.
- A real process crash during an in-flight Codex Send has not been validated
  for exactly-once behavior. The app does not automatically replay an
  unconfirmed Send.
- Automated accessibility checks do not replace manual contrast, keyboard
  and screen-reader acceptance across the complete UI.

## Verification

The desktop-only source gate passed offline locked `cargo check`, all 44
desktop Rust tests, eight Node tests, formatting, strict Clippy, the controlled
browser suite and `git diff --check`. A headless axe-core scan reported no
violations in the checked empty, loaded-record, loaded-preview and active-chat
states; gradient backgrounds left automated color-contrast checks incomplete.

One NSIS candidate was built at
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.5_x64-setup.exe`
(4,620,348 bytes; SHA-256
`F4793ABBEA90B9A951A94E48572FDDEFC2B7EC6B980C1B380D8EC0C24B9922D9`).
Its release executable passed a hidden native journey using actual Windows
folder and file dialogs, Forge onboarding and record readback, a real Codex
page-creation turn, local preview, a follow-up edit in the same conversation,
and WebView reload without resending.

The exact installer hash was rechecked and it installed silently over the
previously installed 0.1.4 (exit code 0). Installed 0.1.5 has executable
SHA-256 `37B0BCE7D3DFD88CDEEF506C905D7BB49DBE7824ADCA94ED2339186CE886D402`.
A disposable real Codex user/reply pair was restored before and after the
upgrade with no new Send. A separate hidden native smoke on the installed
binary passed project onboarding, real folder/file dialogs, record readback
and local preview. A new real Codex turn was not sent on the installed binary;
the real artifact journey ran on the release executable from the same build.

Commit, tag, public upload, fresh download and verification of downloaded
bytes are NOT_RUN at this candidate stage. Model-specific token use and BRL
cost remain UNKNOWN.
