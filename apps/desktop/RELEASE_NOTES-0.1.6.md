# Forge Desktop 0.1.6 alpha — published

This Windows x64 alpha improves the local result-to-change loop. It does not
claim that every Forge backend capability has a screen.

## Change from 0.1.5

- After a successful Codex turn, an already opened local result refreshes
  automatically through the existing project-bound native file read. The app
  does not guess which file to show from the agent's reply or refresh after a
  failed/interrupted turn.
- If the larger preview is open when Codex finishes, it remains open. The file
  refreshes after the person closes it, so reading is not interrupted.
- A recovered pending decision whose original question is unavailable in the
  current Forge record is labeled as such instead of implying that the app
  knows the missing wording.

## Alpha limits

- `forge-core` and a compatible authenticated Codex CLI remain separate
  installations. The installer is unsigned and current-user only; there is
  no in-app automatic updater.
- The preview is local, read-only, scriptless and noninteractive; it does not
  prove publication or production behavior. A result must first be selected
  or opened from a safe local-file action in the Codex reply. If the Codex
  turn creates a different file, the app does not switch files automatically.
- The Forge record can lag behind the chat. Historical decision wording is
  incomplete where the core audit retains only references and digests; the
  Desktop does not reconstruct it from today's policy or chat text.
- A real process crash during an in-flight Codex Send has not been validated
  for exactly-once behavior. The app does not automatically replay an
  unconfirmed Send.
- Automated accessibility checks do not replace manual visual, contrast,
  keyboard and screen-reader acceptance across the complete UI.

## Verification

The desktop-only source gate passed locked offline `cargo check`, all 44 Rust
tests, eight Node tests, formatting, strict Clippy, the controlled browser
suite, and `git diff --check`. The browser suite covered normal and
enlarged-preview refresh paths using simulated IPC. No core workspace-wide
build or GitHub CI was run.

One NSIS candidate was built at
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.6_x64-setup.exe`
(4,623,315 bytes; SHA-256
`CD50E95002A5D9E923E206AD86DB864732612DEE53424E711F91313A6AC0BD9D`).
The release executable passed a hidden native WebView journey with real Codex:
it created a local HTML page, opened its isolated preview, changed the page
in the same conversation, and showed the new title without a manual refresh
click. The conversation was restored after WebView reload without sending
again. A separate native run verified that an enlarged preview stays open
until the person closes it, then refreshes.

The exact candidate hash was rechecked before installation. Silent NSIS
installation over local 0.1.5 exited 0. Installed 0.1.6 reports product
version 0.1.6 and executable SHA-256
`3625F44B812D17529A23F369D52E0240BB57692A0F5ED39467EA5B7A4ED6ACFA`.
A disposable real Codex user/reply pair created with installed 0.1.5 was
restored on installed 0.1.6 without a new Send. A separate installed-binary
native smoke passed Forge onboarding and readback, local preview, and the
actual Windows folder and file dialogs. A new real Codex artifact turn was
not sent from the installed binary; it passed on the release executable from
the same build.

Manual screen-reader/complete contrast acceptance and a real in-flight
process crash remain NOT_RUN. Model-specific token usage and BRL cost remain
UNKNOWN.

## Publication readback

The [0.1.6 alpha prerelease](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.6-alpha.1)
was published from commit `814f47f8b1fbc520c9daa10397cf420c94d3dd30`.
Its public installer is 4,623,315 bytes and has SHA-256
`CD50E95002A5D9E923E206AD86DB864732612DEE53424E711F91313A6AC0BD9D`.
An unauthenticated fresh download matched that hash and its published sidecar.
The downloaded file was installed over the local 0.1.6 installation; silent
install exited 0, and the pre-upgrade real Codex conversation was restored
without a new Send. The installer was not rebuilt for publication. GitHub CI
was not manually triggered.
